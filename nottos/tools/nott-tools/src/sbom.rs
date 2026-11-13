// SPDX-License-Identifier: Apache-2.0

//! `sbom`: software bill of materials for the host tools, in SPDX 2.3 JSON.
//!
//! The inputs are the output of `cargo metadata --format-version 1 --locked`,
//! the workspace's `Cargo.lock`, and the list of packages `cargo tree`
//! reports for normal and build dependencies on the build platform. Only
//! packages on that list are written: Cargo's metadata also names packages
//! that exist for tests or for other platforms, and those are not part of
//! what ships. The output is deterministic for the same inputs.

use crate::args::Args;
use crate::util::{read_text, write_text};
use crate::{CommandResult, Outcome};
use nott_format::Sha256Digest;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;
const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

pub(crate) fn run(raw: &[&str]) -> CommandResult {
    let args = Args::parse(
        raw,
        &["metadata", "lock", "packages", "created", "name", "out"],
    )?;
    let [] = args.positionals()?;
    let metadata_path = args.required("metadata")?;
    let metadata: Value =
        serde_json::from_str(&read_text(Path::new(metadata_path), MAX_INPUT_BYTES)?)
            .map_err(|error| format!("{metadata_path}: {error}"))?;
    let lock = read_text(Path::new(args.required("lock")?), MAX_INPUT_BYTES)?;
    let shipped = parse_package_list(&read_text(
        Path::new(args.required("packages")?),
        MAX_INPUT_BYTES,
    )?)?;
    let document = build(
        &metadata,
        &lock,
        &shipped,
        args.required("created")?,
        args.required("name")?,
    )?;
    let count = document["packages"].as_array().map_or(0, Vec::len);
    let mut text = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("cannot serialise the SBOM: {error}"))?;
    text.push('\n');
    let out = args.required("out")?;
    write_text(Path::new(out), &text)?;
    Ok(Outcome::Pass(format!("wrote {out}: {count} package(s)\n")))
}

struct Package<'a> {
    name: &'a str,
    version: &'a str,
    license: Option<&'a str>,
    source: Option<&'a str>,
}

/// Parses lines of the form `name version`.
fn parse_package_list(text: &str) -> Result<BTreeSet<(String, String)>, String> {
    let mut packages = BTreeSet::new();
    for (index, line) in text.lines().enumerate() {
        let fields: Vec<&str> = line.split(' ').collect();
        let &[name, version] = fields.as_slice() else {
            return Err(format!(
                "package list, line {}: expected `name version`",
                index + 1
            ));
        };
        packages.insert((name.to_owned(), version.to_owned()));
    }
    if packages.is_empty() {
        return Err("package list is empty".to_owned());
    }
    Ok(packages)
}

fn build(
    metadata: &Value,
    lock: &str,
    shipped: &BTreeSet<(String, String)>,
    created: &str,
    name: &str,
) -> Result<Value, String> {
    if !is_timestamp(created) {
        return Err("--created must look like 2026-11-12T00:00:00Z".to_owned());
    }
    if name.is_empty() {
        return Err("--name must not be empty".to_owned());
    }

    let mut packages = BTreeMap::new();
    for package in array(metadata, "packages")? {
        packages.insert(
            string(package, "id")?,
            Package {
                name: string(package, "name")?,
                version: string(package, "version")?,
                license: package.get("license").and_then(Value::as_str),
                source: package.get("source").and_then(Value::as_str),
            },
        );
    }

    // Dependency edges, without development-only dependencies.
    let resolve = metadata
        .get("resolve")
        .ok_or("cargo metadata: missing `resolve`")?;
    let mut edges: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for node in array(resolve, "nodes")? {
        let mut targets = Vec::new();
        for dependency in array(node, "deps")? {
            let shipped = array(dependency, "dep_kinds")?
                .iter()
                .any(|kind| kind.get("kind").and_then(Value::as_str) != Some("dev"));
            if shipped {
                targets.push(string(dependency, "pkg")?);
            }
        }
        edges.insert(string(node, "id")?, targets);
    }

    let mut members = Vec::new();
    for member in array(metadata, "workspace_members")? {
        members.push(
            member
                .as_str()
                .ok_or("cargo metadata: bad workspace member")?,
        );
    }
    let is_shipped = |id: &str| {
        packages.get(id).is_some_and(|package| {
            shipped.contains(&(package.name.to_owned(), package.version.to_owned()))
        })
    };
    let mut reachable = BTreeSet::new();
    let mut pending = members.clone();
    while let Some(id) = pending.pop() {
        if is_shipped(id) && reachable.insert(id) {
            pending.extend(edges.get(id).into_iter().flatten().copied());
        }
    }
    // The two views of the dependency graph must agree: everything Cargo
    // says is built must have been found in the metadata.
    for (name, version) in shipped {
        let found = reachable.iter().any(|id| {
            packages
                .get(id)
                .is_some_and(|package| package.name == name && package.version == version)
        });
        if !found {
            return Err(format!(
                "package list names {name} {version}, which the metadata graph does not reach"
            ));
        }
    }

    let checksums = lock_checksums(lock)?;
    let mut identifiers: BTreeMap<&str, String> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for id in &reachable {
        let package = packages
            .get(id)
            .ok_or_else(|| format!("cargo metadata: unknown package `{id}`"))?;
        let identifier = spdx_identifier(package.name, package.version);
        if !seen.insert(identifier.clone()) {
            return Err(format!("two packages map to SPDX identifier {identifier}"));
        }
        identifiers.insert(id, identifier);
    }

    let mut package_values = Vec::new();
    for (id, identifier) in &identifiers {
        let package = &packages[id];
        let registry = package.source == Some(CRATES_IO);
        let download = if registry {
            format!(
                "https://crates.io/api/v1/crates/{}/{}/download",
                package.name, package.version
            )
        } else {
            "NOASSERTION".to_owned()
        };
        let mut value = json!({
            "SPDXID": identifier,
            "name": package.name,
            "versionInfo": package.version,
            "downloadLocation": download,
            "filesAnalyzed": false,
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": package
                .license
                .map_or_else(|| "NOASSERTION".to_owned(), |text| text.replace('/', " OR ")),
            "copyrightText": "NOASSERTION",
            "externalRefs": [{
                "referenceCategory": "PACKAGE-MANAGER",
                "referenceType": "purl",
                "referenceLocator": format!("pkg:cargo/{}@{}", package.name, package.version),
            }],
        });
        if registry {
            let checksum = checksums
                .get(&(package.name.to_owned(), package.version.to_owned()))
                .ok_or_else(|| {
                    format!(
                        "Cargo.lock has no checksum for {} {}",
                        package.name, package.version
                    )
                })?;
            value["checksums"] = json!([{"algorithm": "SHA256", "checksumValue": checksum}]);
        }
        package_values.push((identifier.clone(), value));
    }
    package_values.sort_by(|a, b| a.0.cmp(&b.0));

    let mut relationships = BTreeSet::new();
    for member in &members {
        relationships.insert((
            "SPDXRef-DOCUMENT",
            "DESCRIBES",
            identifiers[member].as_str(),
        ));
    }
    for (id, identifier) in &identifiers {
        for target in edges.get(id).into_iter().flatten() {
            if let Some(target) = identifiers.get(target) {
                relationships.insert((identifier.as_str(), "DEPENDS_ON", target.as_str()));
            }
        }
    }

    Ok(json!({
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": name,
        "documentNamespace": format!(
            "https://spdx.org/spdxdocs/{name}-{}",
            Sha256Digest::of_bytes(lock.as_bytes())
        ),
        "creationInfo": {
            "created": created,
            "creators": [format!("Tool: nott-tools-{}", env!("CARGO_PKG_VERSION"))],
        },
        "packages": package_values.into_iter().map(|(_, value)| value).collect::<Vec<_>>(),
        "relationships": relationships
            .into_iter()
            .map(|(from, kind, to)| json!({
                "spdxElementId": from,
                "relationshipType": kind,
                "relatedSpdxElement": to,
            }))
            .collect::<Vec<_>>(),
    }))
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("cargo metadata: missing array `{key}`"))
}

fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("cargo metadata: missing string `{key}`"))
}

/// Maps (name, version) to the SHA-256 checksum recorded in `Cargo.lock`.
fn lock_checksums(lock: &str) -> Result<BTreeMap<(String, String), String>, String> {
    let table: toml::Table =
        toml::from_str(lock).map_err(|error| format!("Cargo.lock: {}", error.message()))?;
    let entries = table
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or("Cargo.lock: missing `package` entries")?;
    let mut checksums = BTreeMap::new();
    for entry in entries {
        let field = |key: &str| entry.get(key).and_then(toml::Value::as_str);
        if let (Some(name), Some(version), Some(checksum)) =
            (field("name"), field("version"), field("checksum"))
        {
            checksums.insert((name.to_owned(), version.to_owned()), checksum.to_owned());
        }
    }
    Ok(checksums)
}

/// SPDX identifiers may contain only letters, digits, `.` and `-`.
fn spdx_identifier(name: &str, version: &str) -> String {
    let clean = |text: &str| -> String {
        text.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' {
                    c
                } else {
                    '-'
                }
            })
            .collect()
    };
    format!("SPDXRef-Package-{}-{}", clean(name), clean(version))
}

fn is_timestamp(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 20
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            10 => *byte == b'T',
            13 | 16 => *byte == b':',
            19 => *byte == b'Z',
            _ => byte.is_ascii_digit(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = r#"
version = 4

[[package]]
name = "app"
version = "0.1.0"
dependencies = ["lib_one"]

[[package]]
name = "lib_one"
version = "1.2.3+meta"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "aaaa"

[[package]]
name = "build-helper"
version = "2.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "bbbb"

[[package]]
name = "test-only"
version = "9.9.9"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "cccc"
"#;

    fn metadata() -> Value {
        let registry = CRATES_IO;
        json!({
            "packages": [
                {"id": "app-id", "name": "app", "version": "0.1.0",
                 "license": "Apache-2.0", "source": null},
                {"id": "one-id", "name": "lib_one", "version": "1.2.3+meta",
                 "license": "MIT/Apache-2.0", "source": registry},
                {"id": "build-id", "name": "build-helper", "version": "2.0.0",
                 "license": null, "source": registry},
                {"id": "test-id", "name": "test-only", "version": "9.9.9",
                 "license": "MIT", "source": registry},
            ],
            "workspace_members": ["app-id"],
            "resolve": {"nodes": [
                {"id": "app-id", "deps": [
                    {"pkg": "one-id", "dep_kinds": [{"kind": null}]},
                    {"pkg": "test-id", "dep_kinds": [{"kind": "dev"}]},
                ]},
                {"id": "one-id", "deps": [
                    {"pkg": "build-id", "dep_kinds": [{"kind": "build"}]},
                ]},
                {"id": "build-id", "deps": []},
                {"id": "test-id", "deps": []},
            ]},
        })
    }

    const CREATED: &str = "2026-11-12T00:00:00Z";

    fn shipped(packages: &[(&str, &str)]) -> BTreeSet<(String, String)> {
        packages
            .iter()
            .map(|(name, version)| ((*name).to_owned(), (*version).to_owned()))
            .collect()
    }

    fn all_shipped() -> BTreeSet<(String, String)> {
        shipped(&[
            ("app", "0.1.0"),
            ("lib_one", "1.2.3+meta"),
            ("build-helper", "2.0.0"),
        ])
    }

    fn names(document: &Value) -> Vec<&str> {
        document["packages"]
            .as_array()
            .expect("array")
            .iter()
            .map(|package| package["name"].as_str().expect("name"))
            .collect()
    }

    fn relationships(document: &Value) -> Vec<String> {
        document["relationships"]
            .as_array()
            .expect("array")
            .iter()
            .map(|r| {
                format!(
                    "{} {} {}",
                    r["spdxElementId"].as_str().expect("id"),
                    r["relationshipType"].as_str().expect("type"),
                    r["relatedSpdxElement"].as_str().expect("id")
                )
            })
            .collect()
    }

    #[test]
    fn lists_shipped_packages_only() {
        let document = build(&metadata(), LOCK, &all_shipped(), CREATED, "sample").expect("valid");
        assert_eq!(document["spdxVersion"], "SPDX-2.3");
        assert_eq!(document["creationInfo"]["created"], CREATED);
        // The test-only dependency is absent from the metadata walk.
        assert_eq!(names(&document), ["app", "build-helper", "lib_one"]);
        let packages = document["packages"].as_array().expect("array");

        let app = &packages[0];
        assert_eq!(app["SPDXID"], "SPDXRef-Package-app-0.1.0");
        assert_eq!(app["downloadLocation"], "NOASSERTION");
        assert_eq!(app["licenseDeclared"], "Apache-2.0");
        assert!(app.get("checksums").is_none());

        let helper = &packages[1];
        assert_eq!(helper["licenseDeclared"], "NOASSERTION");
        assert_eq!(helper["checksums"][0]["checksumValue"], "bbbb");

        let one = &packages[2];
        assert_eq!(one["SPDXID"], "SPDXRef-Package-lib-one-1.2.3-meta");
        assert_eq!(one["licenseDeclared"], "MIT OR Apache-2.0");
        assert_eq!(
            one["downloadLocation"],
            "https://crates.io/api/v1/crates/lib_one/1.2.3+meta/download"
        );
        assert_eq!(
            one["externalRefs"][0]["referenceLocator"],
            "pkg:cargo/lib_one@1.2.3+meta"
        );
    }

    #[test]
    fn relationships_follow_shipped_edges() {
        let document = build(&metadata(), LOCK, &all_shipped(), CREATED, "sample").expect("valid");
        assert_eq!(
            relationships(&document),
            [
                "SPDXRef-DOCUMENT DESCRIBES SPDXRef-Package-app-0.1.0",
                "SPDXRef-Package-app-0.1.0 DEPENDS_ON SPDXRef-Package-lib-one-1.2.3-meta",
                "SPDXRef-Package-lib-one-1.2.3-meta DEPENDS_ON SPDXRef-Package-build-helper-2.0.0",
            ]
        );
    }

    /// A package the metadata graph reaches but the build does not compile,
    /// for example one that only another platform pulls in, is left out
    /// together with the edge that led to it.
    #[test]
    fn packages_outside_the_build_are_left_out() {
        let built = shipped(&[("app", "0.1.0"), ("lib_one", "1.2.3+meta")]);
        let document = build(&metadata(), LOCK, &built, CREATED, "sample").expect("valid");
        assert_eq!(names(&document), ["app", "lib_one"]);
        assert_eq!(
            relationships(&document),
            [
                "SPDXRef-DOCUMENT DESCRIBES SPDXRef-Package-app-0.1.0",
                "SPDXRef-Package-app-0.1.0 DEPENDS_ON SPDXRef-Package-lib-one-1.2.3-meta",
            ]
        );
    }

    #[test]
    fn output_is_deterministic() {
        let render = || {
            let document =
                build(&metadata(), LOCK, &all_shipped(), CREATED, "sample").expect("valid");
            serde_json::to_string_pretty(&document).expect("serialises")
        };
        assert_eq!(render(), render());
    }

    #[test]
    fn rejects_bad_input() {
        let build_with = |metadata: &Value, lock: &str, created: &str| {
            build(metadata, lock, &all_shipped(), created, "sample").map(|_| ())
        };
        assert_eq!(
            build_with(&metadata(), LOCK, "yesterday"),
            Err("--created must look like 2026-11-12T00:00:00Z".to_owned())
        );
        assert_eq!(
            build_with(
                &metadata(),
                &LOCK.replace("checksum = \"aaaa\"", ""),
                CREATED
            ),
            Err("Cargo.lock has no checksum for lib_one 1.2.3+meta".to_owned())
        );
        assert_eq!(
            build_with(&json!({"packages": []}), LOCK, CREATED),
            Err("cargo metadata: missing `resolve`".to_owned())
        );

        // The package list names something the metadata walk never reaches.
        let mut extra = all_shipped();
        extra.insert(("ghost".to_owned(), "1.0.0".to_owned()));
        assert_eq!(
            build(&metadata(), LOCK, &extra, CREATED, "sample").map(|_| ()),
            Err(
                "package list names ghost 1.0.0, which the metadata graph does not reach"
                    .to_owned()
            )
        );

        // Two packages whose names differ only in a character that SPDX
        // identifiers cannot carry.
        let mut clash = metadata();
        clash["packages"][2]["name"] = json!("lib-one");
        clash["packages"][2]["version"] = json!("1.2.3-meta");
        clash["resolve"]["nodes"][0]["deps"][1]["pkg"] = json!("build-id");
        clash["resolve"]["nodes"][0]["deps"][1]["dep_kinds"][0]["kind"] = json!(null);
        let clashing = shipped(&[
            ("app", "0.1.0"),
            ("lib_one", "1.2.3+meta"),
            ("lib-one", "1.2.3-meta"),
        ]);
        assert_eq!(
            build(&clash, LOCK, &clashing, CREATED, "sample").map(|_| ()),
            Err(
                "two packages map to SPDX identifier SPDXRef-Package-lib-one-1.2.3-meta".to_owned()
            )
        );
    }

    #[test]
    fn package_list_format() {
        assert_eq!(
            parse_package_list("a 1.0.0\nb 2.0.0\na 1.0.0\n"),
            Ok(shipped(&[("a", "1.0.0"), ("b", "2.0.0")]))
        );
        assert_eq!(
            parse_package_list("a 1.0.0 extra\n"),
            Err("package list, line 1: expected `name version`".to_owned())
        );
        assert_eq!(
            parse_package_list(""),
            Err("package list is empty".to_owned())
        );
    }

    #[test]
    fn timestamp_format() {
        assert!(is_timestamp("2026-11-12T23:59:59Z"));
        for bad in [
            "2026-11-12 23:59:59Z",
            "2026-11-12T23:59:59",
            "2026-11-12T23:59:59+0",
            "",
        ] {
            assert!(!is_timestamp(bad), "{bad:?}");
        }
    }
}
