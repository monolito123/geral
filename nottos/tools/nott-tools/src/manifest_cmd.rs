// SPDX-License-Identifier: Apache-2.0

//! `manifest create` and `manifest verify`.

use crate::args::Args;
use crate::sign_cmd::{load_public_key, load_signature};
use crate::util::{read_text, write_text};
use crate::{CommandResult, Outcome};
use nott_crypto::context;
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
    let args = Args::parse(raw, &["dir", "vb-key", "only", "public-key", "signature"])?;
    let [manifest_path] = args.positionals()?;
    let text = read_text(Path::new(manifest_path), MAX_MANIFEST_BYTES as u64)?;
    let mut report = String::new();

    // Authenticity first: an unauthentic manifest is not interpreted.
    match (args.optional("public-key")?, args.optional("signature")?) {
        (Some(key_path), Some(signature_path)) => {
            let public = load_public_key(key_path)?;
            let signature = load_signature(signature_path)?;
            if let Err(error) = nott_crypto::verify(
                &public,
                context::BUILD_MANIFEST,
                text.as_bytes(),
                &signature,
            ) {
                return Ok(Outcome::Fail(format!(
                    "FAIL signature: {error}\nFAIL: the manifest is not authentic\n"
                )));
            }
            report.push_str(&format!("ok   signature by key {}\n", public.key_id()));
        }
        (None, None) => report.push_str("note: signature not checked\n"),
        _ => return Err("--public-key and --signature go together".to_owned()),
    }

    let manifest =
        BuildManifest::parse(&text).map_err(|error| format!("{manifest_path}: {error}"))?;
    let dir = Path::new(args.required("dir")?);
    let selected = selected_images(&manifest, args.optional("only")?)?;

    report.push_str(&format!(
        "manifest: version {} build {} patch {} target {}\n",
        manifest.version(),
        manifest.build_id(),
        manifest.security_patch(),
        manifest.target()
    ));
    let mut failures = 0usize;
    let mut fail = |report: &mut String, what: &str, why: String| {
        failures += 1;
        report.push_str(&format!("FAIL {what}: {why}\n"));
    };

    for image in &selected {
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
        let scope = if selected.len() == manifest.images().len() {
            "every listed item matches the manifest".to_owned()
        } else {
            format!(
                "{} of {} listed items checked, all match",
                selected.len(),
                manifest.images().len()
            )
        };
        report.push_str(&format!("PASS: {scope}\n"));
        Ok(Outcome::Pass(report))
    } else {
        report.push_str(&format!("FAIL: {failures} item(s) do not match\n"));
        Ok(Outcome::Fail(report))
    }
}

/// The images to check: all of them, or the ones named in `--only`. Every
/// name must be in the manifest and may appear once.
fn selected_images<'a>(
    manifest: &'a BuildManifest,
    only: Option<&str>,
) -> Result<Vec<&'a ImageEntry>, String> {
    let Some(only) = only else {
        return Ok(manifest.images().iter().collect());
    };
    let mut selected: Vec<&ImageEntry> = Vec::new();
    for name in only.split(',') {
        let image = manifest
            .image(name)
            .ok_or_else(|| format!("--only: image `{name}` is not in the manifest"))?;
        if selected.iter().any(|chosen| chosen.name() == name) {
            return Err(format!("--only: image `{name}` named more than once"));
        }
        selected.push(image);
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> BuildManifest {
        let digest = Sha256Digest::of_bytes(b"");
        let images = ["boot", "system", "vendor"]
            .iter()
            .map(|name| ImageEntry::new(name, 1, digest).expect("valid"))
            .collect();
        BuildManifest::new("1", "b", "2026-11-13", "t", digest, images).expect("valid")
    }

    fn names(images: Vec<&ImageEntry>) -> Vec<&str> {
        images.into_iter().map(ImageEntry::name).collect()
    }

    #[test]
    fn selects_all_or_the_named_images() {
        let manifest = manifest();
        assert_eq!(
            selected_images(&manifest, None).map(names),
            Ok(vec!["boot", "system", "vendor"])
        );
        assert_eq!(
            selected_images(&manifest, Some("vendor,boot")).map(names),
            Ok(vec!["vendor", "boot"])
        );
    }

    #[test]
    fn rejects_unknown_and_repeated_names() {
        let manifest = manifest();
        assert_eq!(
            selected_images(&manifest, Some("boot,missing")).map(names),
            Err("--only: image `missing` is not in the manifest".to_owned())
        );
        assert_eq!(
            selected_images(&manifest, Some("boot,boot")).map(names),
            Err("--only: image `boot` named more than once".to_owned())
        );
        assert!(selected_images(&manifest, Some("")).is_err());
    }
}
