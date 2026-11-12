//! `manifest create` and `manifest verify`.

use crate::args::Args;
use crate::util::{read_text, write_text};
use crate::{CommandResult, Outcome};
use nott_format::manifest::MAX_MANIFEST_BYTES;
use nott_format::{BuildManifest, ImageEntry, Sha256Digest};
use std::path::Path;

fn hash_file(path: &Path) -> Result<(Sha256Digest, u64), String> {
    Sha256Digest::of_file(path).map_err(|error| format!("{}: {error}", path.display()))
}

/// Reads and parses a manifest file.
pub(crate) fn load(path: &Path) -> Result<BuildManifest, String> {
    let text = read_text(path, MAX_MANIFEST_BYTES as u64)?;
    BuildManifest::parse(&text).map_err(|error| format!("{}: {error}", path.display()))
}

pub(crate) fn create(raw: &[&str]) -> CommandResult {
    let args = Args::parse(
        raw,
        &[
            "version",
            "build-id",
            "security-patch",
            "target",
            "vb-key",
            "image",
            "out",
        ],
    )?;
    let [] = args.positionals()?;

    let mut images = Vec::new();
    for spec in args.repeated("image") {
        let (name, path) = spec
            .split_once('=')
            .ok_or_else(|| format!("--image expects NAME=FILE, got `{spec}`"))?;
        let (digest, size) = hash_file(Path::new(path))?;
        let entry = ImageEntry::new(name, size, digest)
            .map_err(|error| format!("image `{name}`: {error}"))?;
        images.push(entry);
    }
    let (vb_key, _) = hash_file(Path::new(args.required("vb-key")?))?;
    let manifest = BuildManifest::new(
        args.required("version")?,
        args.required("build-id")?,
        args.required("security-patch")?,
        args.required("target")?,
        vb_key,
        images,
    )
    .map_err(|error| error.to_string())?;

    let out = args.required("out")?;
    write_text(Path::new(out), &manifest.to_string())?;
    Ok(Outcome::Pass(format!(
        "wrote {out}: {} image(s)\n",
        manifest.images().len()
    )))
}

pub(crate) fn verify(raw: &[&str]) -> CommandResult {
    let args = Args::parse(raw, &["dir", "vb-key"])?;
    let [manifest_path] = args.positionals()?;
    let manifest = load(Path::new(manifest_path))?;
    let dir = Path::new(args.required("dir")?);

    let mut report = format!(
        "manifest: version {} build {} patch {} target {}\n",
        manifest.version(),
        manifest.build_id(),
        manifest.security_patch(),
        manifest.target()
    );
    let mut failures = 0usize;
    let mut fail = |report: &mut String, what: &str, why: String| {
        failures += 1;
        report.push_str(&format!("FAIL {what}: {why}\n"));
    };

    for image in manifest.images() {
        match Sha256Digest::of_file(&dir.join(image.name())) {
            Err(error) => fail(&mut report, image.name(), error.to_string()),
            Ok((_, size)) if size != image.size() => fail(
                &mut report,
                image.name(),
                format!("size {size}, manifest says {}", image.size()),
            ),
            Ok((digest, _)) if digest != image.sha256() => fail(
                &mut report,
                image.name(),
                format!("sha256 {digest}, manifest says {}", image.sha256()),
            ),
            Ok(_) => report.push_str(&format!("ok   {}\n", image.name())),
        }
    }
    if let Some(key_path) = args.optional("vb-key")? {
        let (digest, _) = hash_file(Path::new(key_path))?;
        if digest == manifest.verified_boot_key_sha256() {
            report.push_str("ok   verified-boot key\n");
        } else {
            fail(
                &mut report,
                "verified-boot key",
                format!(
                    "sha256 {digest}, manifest says {}",
                    manifest.verified_boot_key_sha256()
                ),
            );
        }
    }

    if failures == 0 {
        report.push_str("PASS: every listed item matches the manifest\n");
        Ok(Outcome::Pass(report))
    } else {
        report.push_str(&format!("FAIL: {failures} item(s) do not match\n"));
        Ok(Outcome::Fail(report))
    }
}
