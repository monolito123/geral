//! `repro compare`: are two build output trees identical?
//!
//! Used by the reproducibility check
//! (`docs/adr/0006-build-reproducibility.md`): the same source is built twice
//! in different directories and the outputs must match bit for bit.

use crate::args::Args;
use crate::{CommandResult, Outcome};
use nott_format::Sha256Digest;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// How many differences are printed before the rest are only counted.
const MAX_REPORTED: usize = 50;

#[derive(Debug, PartialEq, Eq)]
enum Entry {
    Directory,
    File { size: u64, sha256: Sha256Digest },
    Symlink { target: String },
}

type Tree = BTreeMap<String, Entry>;

pub(crate) fn compare(raw: &[&str]) -> CommandResult {
    let args = Args::parse(raw, &[])?;
    let [left, right] = args.positionals()?;
    let left_tree = scan(Path::new(left))?;
    let right_tree = scan(Path::new(right))?;
    let differences = diff(&left_tree, &right_tree);

    if differences.is_empty() {
        let files = left_tree
            .values()
            .filter(|entry| matches!(entry, Entry::File { .. }))
            .count();
        return Ok(Outcome::Pass(format!(
            "PASS: identical ({files} file(s), {} entries)\n",
            left_tree.len()
        )));
    }
    let mut report = String::new();
    for line in differences.iter().take(MAX_REPORTED) {
        report.push_str(line);
        report.push('\n');
    }
    if differences.len() > MAX_REPORTED {
        report.push_str(&format!(
            "... and {} more\n",
            differences.len() - MAX_REPORTED
        ));
    }
    report.push_str(&format!("FAIL: {} difference(s)\n", differences.len()));
    Ok(Outcome::Fail(report))
}

/// Records every entry below `root`, keyed by its `/`-separated relative
/// path.
fn scan(root: &Path) -> Result<Tree, String> {
    let metadata = fs::metadata(root).map_err(|error| format!("{}: {error}", root.display()))?;
    if !metadata.is_dir() {
        return Err(format!("{}: not a directory", root.display()));
    }
    let mut tree = Tree::new();
    scan_into(root, "", &mut tree)?;
    Ok(tree)
}

fn scan_into(directory: &Path, prefix: &str, tree: &mut Tree) -> Result<(), String> {
    let describe = |error: std::io::Error| format!("{}: {error}", directory.display());
    for item in fs::read_dir(directory).map_err(describe)? {
        let item = item.map_err(describe)?;
        let path = item.path();
        let name = item
            .file_name()
            .into_string()
            .map_err(|_| format!("{}: file name is not UTF-8", path.display()))?;
        let relative = format!("{prefix}{name}");
        let describe = |error: std::io::Error| format!("{}: {error}", path.display());
        let kind = fs::symlink_metadata(&path).map_err(describe)?.file_type();
        let entry = if kind.is_symlink() {
            let target = fs::read_link(&path).map_err(describe)?;
            Entry::Symlink {
                target: target.to_string_lossy().into_owned(),
            }
        } else if kind.is_dir() {
            scan_into(&path, &format!("{relative}/"), tree)?;
            Entry::Directory
        } else if kind.is_file() {
            let (sha256, size) = Sha256Digest::of_file(&path).map_err(describe)?;
            Entry::File { size, sha256 }
        } else {
            return Err(format!("{}: unsupported file type", path.display()));
        };
        tree.insert(relative, entry);
    }
    Ok(())
}

fn diff(left: &Tree, right: &Tree) -> Vec<String> {
    let mut differences = Vec::new();
    for (path, entry) in left {
        match right.get(path) {
            None => differences.push(format!("only in first:  {path}")),
            Some(other) if other != entry => differences.push(format!(
                "differs:        {path} ({} vs {})",
                describe(entry),
                describe(other)
            )),
            Some(_) => {}
        }
    }
    for path in right.keys().filter(|path| !left.contains_key(*path)) {
        differences.push(format!("only in second: {path}"));
    }
    differences
}

fn describe(entry: &Entry) -> String {
    match entry {
        Entry::Directory => "directory".to_owned(),
        Entry::File { size, sha256 } => format!("{size} bytes, sha256 {sha256}"),
        Entry::Symlink { target } => format!("symlink to {target}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(content: &[u8]) -> Entry {
        Entry::File {
            size: content.len() as u64,
            sha256: Sha256Digest::of_bytes(content),
        }
    }

    fn tree(entries: Vec<(&str, Entry)>) -> Tree {
        entries
            .into_iter()
            .map(|(path, entry)| (path.to_owned(), entry))
            .collect()
    }

    #[test]
    fn identical_trees_have_no_differences() {
        let build = || {
            tree(vec![
                ("bin", Entry::Directory),
                ("bin/tool", file(b"binary")),
                (
                    "link",
                    Entry::Symlink {
                        target: "bin/tool".to_owned(),
                    },
                ),
            ])
        };
        assert!(diff(&build(), &build()).is_empty());
    }

    #[test]
    fn reports_every_kind_of_difference() {
        let left = tree(vec![
            ("a", file(b"same")),
            ("b", file(b"left")),
            ("c", file(b"gone")),
            ("d", Entry::Directory),
        ]);
        let right = tree(vec![
            ("a", file(b"same")),
            ("b", file(b"right")),
            ("d", file(b"now a file")),
            ("e", file(b"new")),
        ]);
        let differences = diff(&left, &right);
        assert_eq!(differences.len(), 4);
        assert!(differences[0].starts_with("differs:        b (4 bytes, sha256 "));
        assert_eq!(differences[1], "only in first:  c");
        assert!(differences[2].starts_with("differs:        d (directory vs 10 bytes"));
        assert_eq!(differences[3], "only in second: e");
    }
}
