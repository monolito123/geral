// SPDX-License-Identifier: Apache-2.0

//! `size check`: the system image size budget
//! (`docs/performance-budgets.md`, section 2.5).

use crate::args::Args;
use crate::manifest_cmd::load;
use crate::{CommandResult, Outcome};
use nott_format::BuildManifest;
use std::path::Path;

pub(crate) fn check(raw: &[&str]) -> CommandResult {
    let args = Args::parse(raw, &["budget-bytes", "images"])?;
    let [manifest_path] = args.positionals()?;
    let budget: u64 = args
        .required("budget-bytes")?
        .parse()
        .map_err(|_| "--budget-bytes must be a whole number of bytes".to_owned())?;
    let names: Vec<&str> = args.required("images")?.split(',').collect();
    let manifest = load(Path::new(manifest_path))?;
    let total = total_size(&manifest, &names)?;

    let report = format!(
        "images: {}\ntotal: {total} bytes\nbudget: {budget} bytes\n",
        names.join(", ")
    );
    if total <= budget {
        Ok(Outcome::Pass(format!("{report}PASS: within budget\n")))
    } else {
        Ok(Outcome::Fail(format!(
            "{report}FAIL: over budget by {} bytes\n",
            total - budget
        )))
    }
}

/// Sums the sizes of the named images. Every name must be in the manifest
/// and may be given only once, so that a typo cannot make the check pass.
fn total_size(manifest: &BuildManifest, names: &[&str]) -> Result<u64, String> {
    let mut total: u64 = 0;
    for (index, name) in names.iter().enumerate() {
        if names[..index].contains(name) {
            return Err(format!("image `{name}` named more than once"));
        }
        let image = manifest
            .image(name)
            .ok_or_else(|| format!("image `{name}` is not in the manifest"))?;
        total = total
            .checked_add(image.size())
            .ok_or_else(|| "total size overflows 64 bits".to_owned())?;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nott_format::{ImageEntry, Sha256Digest};

    fn manifest(sizes: &[(&str, u64)]) -> BuildManifest {
        let digest = Sha256Digest::of_bytes(b"");
        let images = sizes
            .iter()
            .map(|(name, size)| ImageEntry::new(name, *size, digest).expect("valid"))
            .collect();
        BuildManifest::new("1", "b", "2026-11-12", "t", digest, images).expect("valid")
    }

    #[test]
    fn sums_only_named_images() {
        let manifest = manifest(&[("system", 700), ("product", 200), ("vendor", 5000)]);
        assert_eq!(total_size(&manifest, &["system", "product"]), Ok(900));
        assert_eq!(total_size(&manifest, &["vendor"]), Ok(5000));
    }

    #[test]
    fn rejects_unknown_repeated_and_overflowing() {
        let manifest = manifest(&[("a", u64::MAX), ("b", 1)]);
        assert_eq!(
            total_size(&manifest, &["a", "missing"]),
            Err("image `missing` is not in the manifest".to_owned())
        );
        assert_eq!(
            total_size(&manifest, &["b", "b"]),
            Err("image `b` named more than once".to_owned())
        );
        assert_eq!(
            total_size(&manifest, &["a", "b"]),
            Err("total size overflows 64 bits".to_owned())
        );
    }
}
