// SPDX-License-Identifier: Apache-2.0

//! `sigset`, `update root-create`, `update metadata-create` and
//! `update verify` (`docs/adr/0009-update-system.md`).

use crate::args::Args;
use crate::sign_cmd::{load_public_key, load_signature};
use crate::util::{read_text, write_text};
use crate::{CommandResult, Outcome};
use nott_crypto::SignatureSet;
use nott_format::text::parse_decimal;
use nott_format::time::Timestamp;
use nott_format::update::{Role, Root, UpdateFields, UpdateMetadata};
use nott_format::Sha256Digest;
use nott_update::{
    check_initial_root, check_manifest, rotate_root, verify_update, Device, TrustState,
};
use std::path::Path;

const MAX_DOCUMENT_BYTES: u64 = 4 * 1024 * 1024;

fn read(path: &str) -> Result<String, String> {
    read_text(Path::new(path), MAX_DOCUMENT_BYTES)
}

fn load_set(path: &str) -> Result<SignatureSet, String> {
    SignatureSet::parse(&read(path)?).map_err(|error| format!("{path}: {error}"))
}

fn number(args: &Args<'_>, name: &str) -> Result<u64, String> {
    parse_decimal(args.required(name)?)
        .ok_or_else(|| format!("--{name} must be a whole number without leading zeros"))
}

fn time(args: &Args<'_>, name: &str) -> Result<Timestamp, String> {
    let text = args.required(name)?;
    Timestamp::parse(text).ok_or_else(|| format!("--{name} must look like 2026-11-13T00:00:00Z"))
}

fn role(args: &Args<'_>, threshold_option: &str, key_option: &str) -> Result<Role, String> {
    let threshold = usize::try_from(number(args, threshold_option)?)
        .map_err(|_| format!("--{threshold_option} is too large"))?;
    let keys = args
        .repeated(key_option)
        .iter()
        .map(|path| load_public_key(path))
        .collect::<Result<Vec<_>, _>>()?;
    Role::new(threshold, keys).map_err(|_| {
        format!(
            "--{key_option} must name 1 to 16 distinct keys and --{threshold_option} \
             must be between 1 and their number"
        )
    })
}

pub(crate) fn sigset(raw: &[&str]) -> CommandResult {
    let args = Args::parse(raw, &["out"])?;
    let paths: Vec<&str> = args.rest().to_vec();
    if paths.is_empty() {
        return Err("sigset needs at least one signature file".to_owned());
    }
    let signatures = paths
        .iter()
        .map(|path| load_signature(path))
        .collect::<Result<Vec<_>, _>>()?;
    let set = SignatureSet::new(signatures).map_err(|error| error.to_string())?;
    let out = args.required("out")?;
    write_text(Path::new(out), &set.to_string())?;
    Ok(Outcome::Pass(format!(
        "wrote {out}: {} signature(s)\n",
        set.signatures().len()
    )))
}

pub(crate) fn root_create(raw: &[&str]) -> CommandResult {
    let args = Args::parse(
        raw,
        &[
            "version",
            "expires",
            "root-threshold",
            "root-key",
            "release-threshold",
            "release-key",
            "out",
        ],
    )?;
    let [] = args.positionals()?;
    let root = Root::new(
        number(&args, "version")?,
        time(&args, "expires")?,
        role(&args, "root-threshold", "root-key")?,
        role(&args, "release-threshold", "release-key")?,
    )
    .map_err(|error| error.to_string())?;
    let out = args.required("out")?;
    write_text(Path::new(out), &root.to_string())?;
    Ok(Outcome::Pass(format!(
        "wrote {out}: root version {}, {} root key(s), {} release key(s)\n",
        root.version(),
        root.root_role().keys().len(),
        root.release_role().keys().len()
    )))
}

pub(crate) fn metadata_create(raw: &[&str]) -> CommandResult {
    let args = Args::parse(
        raw,
        &[
            "channel", "target", "sequence", "issued", "expires", "manifest", "out",
        ],
    )?;
    let [] = args.positionals()?;
    let manifest_path = args.required("manifest")?;
    let (manifest_sha256, manifest_size) = Sha256Digest::of_file(Path::new(manifest_path))
        .map_err(|error| format!("{manifest_path}: {error}"))?;
    let metadata = UpdateMetadata::new(UpdateFields {
        channel: args.required("channel")?,
        target: args.required("target")?,
        sequence: number(&args, "sequence")?,
        issued: time(&args, "issued")?,
        expires: time(&args, "expires")?,
        manifest_sha256,
        manifest_size,
    })
    .map_err(|error| error.to_string())?;
    let out = args.required("out")?;
    write_text(Path::new(out), &metadata.to_string())?;
    Ok(Outcome::Pass(format!(
        "wrote {out}: {} release {}\n",
        metadata.channel(),
        metadata.sequence()
    )))
}

pub(crate) fn verify(raw: &[&str]) -> CommandResult {
    let args = Args::parse(
        raw,
        &[
            "root",
            "root-signatures",
            "next-root",
            "next-root-signatures",
            "metadata",
            "signatures",
            "manifest",
            "channel",
            "target",
            "now",
            "last-sequence",
            "time-floor",
        ],
    )?;
    let [] = args.positionals()?;
    let mut report = String::new();
    let fail = |report: String, error: &dyn std::fmt::Display| {
        Ok(Outcome::Fail(format!("{report}FAIL: {error}\n")))
    };

    let root_path = args.required("root")?;
    let mut root = match check_initial_root(
        &read(root_path)?,
        &load_set(args.required("root-signatures")?)?,
    ) {
        Ok(root) => root,
        Err(error) => return fail(report, &error),
    };
    report.push_str(&format!(
        "ok   root version {} from {root_path}\n",
        root.version()
    ));

    let (next_roots, next_signatures) = (
        args.repeated("next-root"),
        args.repeated("next-root-signatures"),
    );
    if next_roots.len() != next_signatures.len() {
        return Err("give one --next-root-signatures for each --next-root, in order".to_owned());
    }
    for (path, signatures) in next_roots.iter().zip(next_signatures) {
        root = match rotate_root(&root, &read(path)?, &load_set(signatures)?) {
            Ok(next) => next,
            Err(error) => return fail(report, &error),
        };
        report.push_str(&format!(
            "ok   rotated to root version {} from {path}\n",
            root.version()
        ));
    }

    let state = TrustState {
        root,
        last_sequence: match args.optional("last-sequence")? {
            None => 0,
            Some(_) => number(&args, "last-sequence")?,
        },
        time_floor: match args.optional("time-floor")? {
            None => Timestamp::EPOCH,
            Some(_) => time(&args, "time-floor")?,
        },
    };
    let device = Device {
        channel: args.required("channel")?,
        target: args.required("target")?,
    };
    let (metadata, next) = match verify_update(
        &state,
        device,
        &read(args.required("metadata")?)?,
        &load_set(args.required("signatures")?)?,
        time(&args, "now")?,
    ) {
        Ok(result) => result,
        Err(error) => return fail(report, &error),
    };
    report.push_str(&format!(
        "ok   update metadata: {} release {} for {}, expires {}\n",
        metadata.channel(),
        metadata.sequence(),
        metadata.target(),
        metadata.expires()
    ));

    let manifest_path = args.required("manifest")?;
    let manifest =
        std::fs::read(manifest_path).map_err(|error| format!("{manifest_path}: {error}"))?;
    if let Err(error) = check_manifest(&metadata, &manifest) {
        return fail(report, &error);
    }
    report.push_str(&format!("ok   build manifest {manifest_path}\n"));
    report.push_str(&format!(
        "PASS: install release {}; then keep root version {}, last-sequence {}, time-floor {}\n",
        metadata.sequence(),
        next.root.version(),
        next.last_sequence,
        next.time_floor
    ));
    Ok(Outcome::Pass(report))
}
