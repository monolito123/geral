// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests of the `nott-tools` binary: real files, real exit codes.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TOOL: &str = env!("CARGO_BIN_EXE_nott-tools");

/// A scratch directory that is removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!("nott-tools-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create scratch directory");
        Self(path)
    }

    fn file(&self, name: &str, content: &[u8]) -> String {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().expect("has a parent")).expect("create parent");
        fs::write(&path, content).expect("write file");
        text(&path)
    }

    fn path(&self, name: &str) -> String {
        text(&self.0.join(name))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn text(path: &Path) -> String {
    path.to_str().expect("UTF-8 path").to_owned()
}

fn run(arguments: &[&str]) -> (i32, String, String) {
    let Output {
        status,
        stdout,
        stderr,
    } = Command::new(TOOL)
        .args(arguments)
        .output()
        .expect("run nott-tools");
    (
        status.code().expect("exit code"),
        String::from_utf8(stdout).expect("UTF-8 output"),
        String::from_utf8(stderr).expect("UTF-8 output"),
    )
}

#[test]
fn manifest_detects_every_kind_of_tampering() {
    let scratch = Scratch::new("manifest");
    let system = scratch.file("images/system.img", b"system contents");
    let boot = scratch.file("images/boot.img", b"boot contents");
    let key = scratch.file("key.pub", b"public key");
    let manifest = scratch.path("manifest.txt");
    let images = scratch.path("images");

    let (code, stdout, _) = run(&[
        "manifest",
        "create",
        "--version",
        "0.1.0",
        "--build-id",
        "TEST.1",
        "--security-patch",
        "2026-11-05",
        "--target",
        "test",
        "--vb-key",
        &key,
        "--image",
        &format!("system.img={system}"),
        "--image",
        &format!("boot.img={boot}"),
        "--out",
        &manifest,
    ]);
    assert_eq!(code, 0, "{stdout}");
    let written = fs::read_to_string(&manifest).expect("manifest written");
    assert!(written.starts_with("nottos-build-manifest 1\nversion: 0.1.0\n"));

    let verify = |extra: &[&str]| {
        let mut arguments = vec!["manifest", "verify", &manifest, "--dir", &images];
        arguments.extend_from_slice(extra);
        run(&arguments)
    };
    let (code, stdout, _) = verify(&["--vb-key", &key]);
    assert_eq!(code, 0, "{stdout}");
    assert!(stdout.contains("ok   boot.img\nok   system.img\nok   verified-boot key\nPASS"));

    // Same size, one byte different.
    fs::write(&system, b"system contentz").expect("tamper");
    let (code, stdout, _) = verify(&[]);
    assert_eq!(code, 1);
    assert!(stdout.contains("FAIL system.img: sha256 "), "{stdout}");
    assert!(stdout.contains("ok   boot.img"));

    // Different size.
    fs::write(&system, b"short").expect("truncate");
    let (code, stdout, _) = verify(&[]);
    assert_eq!(code, 1);
    assert!(
        stdout.contains("FAIL system.img: size 5, manifest says 15"),
        "{stdout}"
    );

    // Missing image.
    fs::write(&system, b"system contents").expect("restore");
    fs::remove_file(&boot).expect("remove");
    let (code, stdout, _) = verify(&[]);
    assert_eq!(code, 1);
    assert!(stdout.contains("FAIL boot.img: "), "{stdout}");

    // Wrong verified-boot key.
    fs::write(&boot, b"boot contents").expect("restore");
    let other_key = scratch.file("other.pub", b"another key");
    let (code, stdout, _) = verify(&["--vb-key", &other_key]);
    assert_eq!(code, 1);
    assert!(stdout.contains("FAIL verified-boot key: "), "{stdout}");

    // A manifest that was edited is not canonical any more or lists other
    // digests; either way verification cannot pass.
    fs::write(&manifest, written.replace("TEST.1", "TEST.1 ")).expect("edit");
    let (code, _, stderr) = verify(&[]);
    assert_eq!(code, 2);
    assert!(stderr.contains("line 3: invalid build-id"), "{stderr}");
}

#[test]
fn repro_compare_and_size_budget() {
    let scratch = Scratch::new("repro");
    scratch.file("a/bin/tool", b"binary");
    scratch.file("a/notes.txt", b"same");
    scratch.file("b/bin/tool", b"binary");
    scratch.file("b/notes.txt", b"same");
    let (a, b) = (scratch.path("a"), scratch.path("b"));

    let (code, stdout, _) = run(&["repro", "compare", &a, &b]);
    assert_eq!(code, 0);
    assert_eq!(stdout, "PASS: identical (2 file(s), 3 entries)\n");

    scratch.file("b/bin/tool", b"binarz");
    scratch.file("b/extra", b"");
    let (code, stdout, _) = run(&["repro", "compare", &a, &b]);
    assert_eq!(code, 1);
    assert!(stdout.contains("differs:        bin/tool (6 bytes, sha256 "));
    assert!(stdout.contains("only in second: extra\n"));
    assert!(stdout.ends_with("FAIL: 2 difference(s)\n"), "{stdout}");

    let (code, _, stderr) = run(&["repro", "compare", &a, &scratch.path("missing")]);
    assert_eq!(code, 2);
    assert!(stderr.contains("missing"), "{stderr}");

    // Size budget, using a manifest over the same files.
    let key = scratch.file("key", b"k");
    let manifest = scratch.path("manifest.txt");
    let (code, ..) = run(&[
        "manifest",
        "create",
        "--version",
        "1",
        "--build-id",
        "B",
        "--security-patch",
        "2026-11-05",
        "--target",
        "t",
        "--vb-key",
        &key,
        "--image",
        &format!("system={}", scratch.path("a/bin/tool")),
        "--image",
        &format!("product={}", scratch.path("a/notes.txt")),
        "--out",
        &manifest,
    ]);
    assert_eq!(code, 0);
    let size = |budget: &str, images: &str| {
        run(&[
            "size",
            "check",
            &manifest,
            "--budget-bytes",
            budget,
            "--images",
            images,
        ])
    };
    let (code, stdout, _) = size("10", "system,product");
    assert_eq!(code, 0);
    assert!(
        stdout.contains("total: 10 bytes\nbudget: 10 bytes\nPASS"),
        "{stdout}"
    );
    let (code, stdout, _) = size("9", "system,product");
    assert_eq!(code, 1);
    assert!(
        stdout.ends_with("FAIL: over budget by 1 bytes\n"),
        "{stdout}"
    );
    let (code, _, stderr) = size("9", "system,vendor");
    assert_eq!(code, 2);
    assert!(
        stderr.contains("image `vendor` is not in the manifest"),
        "{stderr}"
    );
}

#[test]
fn tier_check_of_the_emulator_descriptor() {
    let descriptor =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../device/cuttlefish/capability.toml");
    let (code, stdout, stderr) = run(&["tier", "check", &text(&descriptor)]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.starts_with(
        "Device: cuttlefish (emulator, x86_64)\n\
         Tier: emulator (development target, no hardware guarantees)\n"
    ));
    assert!(stdout.contains("complete the device audit: some mechanisms are unknown"));
    assert!(!stdout.contains("Missing for Tier A"));
}

#[test]
fn tier_check_fails_for_an_excluded_device() {
    let scratch = Scratch::new("tier");
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../device/cuttlefish/capability.toml");
    let descriptor = fs::read_to_string(source)
        .expect("descriptor readable")
        .replace("kind = \"emulator\"", "kind = \"phone\"")
        .replace(
            "verified_boot_bypass_known = \"unknown\"",
            "verified_boot_bypass_known = \"yes\"",
        );
    let path = scratch.file("capability.toml", descriptor.as_bytes());
    let (code, stdout, _) = run(&["tier", "check", &path]);
    assert_eq!(code, 1);
    assert!(stdout.contains("Tier: excluded (may not carry the NottOS name)"));
    assert!(stdout.contains("a public, unpatchable bypass of verified boot exists"));
}

#[test]
fn storage_check_applies_the_exception_list() {
    let scratch = Scratch::new("storage");
    let report = scratch.file(
        "report.txt",
        b"[mounts]\n\
          /dev/mapper/system / erofs ro,relatime 0 0\n\
          /dev/mapper/data /data ext4 rw,relatime 0 0\n\
          /dev/vdd /scratch ext4 rw,nosuid,nodev,noexec 0 0\n\
          [verity]\n/dev/mapper/system\n\
          [encrypted]\n/dev/mapper/data\n",
    );
    let exceptions = scratch.file("exceptions.txt", b"# test scratch\n/scratch\n");

    let (code, stdout, _) = run(&["storage", "check", &report, "--exceptions", &exceptions]);
    assert_eq!(code, 0, "{stdout}");
    assert!(stdout.ends_with("PASS: 3 mount(s) checked\n"));

    let (code, stdout, _) = run(&["storage", "check", &report]);
    assert_eq!(code, 1);
    assert!(stdout.contains("FAIL /scratch (ext4 on /dev/vdd): neither verified"));
    assert!(stdout.ends_with("FAIL: 1 mount(s) violate the policy\n"));
}

#[test]
fn usage_and_unknown_commands() {
    let (code, stdout, _) = run(&[]);
    assert_eq!(code, 0);
    assert!(stdout.starts_with("usage: nott-tools <command>"));

    let (code, _, stderr) = run(&["frobnicate"]);
    assert_eq!(code, 2);
    assert!(stderr.starts_with("nott-tools: unknown command"));

    let (code, _, stderr) = run(&["repro", "compare", "only-one"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("expected 2 positional argument(s), got 1"));
}

#[test]
fn signed_manifest_end_to_end() {
    use std::os::unix::fs::PermissionsExt as _;

    let scratch = Scratch::new("signed");
    let system = scratch.file("images/system.img", b"system contents");
    let roothash = scratch.file("images/system.roothash", b"0123\n");
    let images = scratch.path("images");
    let manifest = scratch.path("manifest.txt");
    let signature = scratch.path("manifest.sig");
    let (secret, public) = (
        scratch.path("release.secret"),
        scratch.path("release.public"),
    );
    let (other_secret, other_public) = (scratch.path("other.secret"), scratch.path("other.public"));

    // Keys.
    let (code, stdout, stderr) = run(&[
        "key",
        "generate",
        "--out-secret",
        &secret,
        "--out-public",
        &public,
    ]);
    assert_eq!(code, 0, "{stderr}");
    let key_id = stdout
        .lines()
        .find_map(|line| line.strip_prefix("key-id: "))
        .expect("key id printed")
        .to_owned();
    assert_eq!(key_id.len(), 64);
    let mode = fs::metadata(&secret)
        .expect("secret exists")
        .permissions()
        .mode();
    assert_eq!(
        mode & 0o777,
        0o600,
        "secret key must be private to its owner"
    );
    let (code, stdout, _) = run(&["key", "id", &public]);
    assert_eq!(code, 0);
    assert_eq!(
        stdout,
        format!("suite: ed25519+ml-dsa-87\nkey-id: {key_id}\n")
    );
    // An existing secret key is never overwritten.
    let (code, _, stderr) = run(&[
        "key",
        "generate",
        "--out-secret",
        &secret,
        "--out-public",
        &public,
    ]);
    assert_eq!(code, 2);
    assert!(stderr.contains("release.secret"), "{stderr}");
    let (code, ..) = run(&[
        "key",
        "generate",
        "--out-secret",
        &other_secret,
        "--out-public",
        &other_public,
    ]);
    assert_eq!(code, 0);

    // Manifest and signature.
    let (code, ..) = run(&[
        "manifest",
        "create",
        "--version",
        "0.1.0",
        "--build-id",
        "TEST.1",
        "--security-patch",
        "2026-11-05",
        "--target",
        "test",
        "--vb-key",
        &public,
        "--image",
        &format!("system.img={system}"),
        "--image",
        &format!("system.roothash={roothash}"),
        "--out",
        &manifest,
    ]);
    assert_eq!(code, 0);
    let sign = |key: &str, context: &str, out: &str| {
        run(&[
            "sign",
            &manifest,
            "--key",
            key,
            "--context",
            context,
            "--out",
            out,
        ])
    };
    let (code, stdout, stderr) = sign(&secret, "build-manifest", &signature);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains(&format!("key-id: {key_id}")));

    let verify = |key: &str, signature: &str, extra: &[&str]| {
        let mut arguments = vec![
            "manifest",
            "verify",
            &manifest,
            "--dir",
            &images,
            "--public-key",
            key,
            "--signature",
            signature,
        ];
        arguments.extend_from_slice(extra);
        run(&arguments)
    };

    // Authentic and intact.
    let (code, stdout, _) = verify(&public, &signature, &["--vb-key", &public]);
    assert_eq!(code, 0, "{stdout}");
    assert!(stdout.starts_with(&format!("ok   signature by key {key_id}\n")));
    assert!(stdout.ends_with("PASS: every listed item matches the manifest\n"));

    // A subset, as an early boot stage checks it.
    let (code, stdout, _) = verify(&public, &signature, &["--only", "system.roothash"]);
    assert_eq!(code, 0);
    assert!(stdout.ends_with("PASS: 1 of 2 listed items checked, all match\n"));
    assert!(!stdout.contains("system.img"));

    // The generic verify command agrees.
    let generic = |context: &str| {
        run(&[
            "verify",
            &manifest,
            "--public-key",
            &public,
            "--signature",
            &signature,
            "--context",
            context,
        ])
    };
    assert_eq!(generic("build-manifest").0, 0);
    let (code, stdout, _) = generic("update-metadata");
    assert_eq!(code, 1);
    assert_eq!(stdout, "FAIL: signature was made for a different purpose\n");

    // Signed by a key other than the trusted one.
    let other_signature = scratch.path("other.sig");
    assert_eq!(sign(&other_secret, "build-manifest", &other_signature).0, 0);
    let (code, stdout, _) = verify(&public, &other_signature, &[]);
    assert_eq!(code, 1);
    assert_eq!(
        stdout,
        "FAIL signature: signature was made by a different key\n\
         FAIL: the manifest is not authentic\n"
    );

    // Signed by the right key for another purpose.
    let wrong_purpose = scratch.path("purpose.sig");
    assert_eq!(sign(&secret, "update-metadata", &wrong_purpose).0, 0);
    let (code, stdout, _) = verify(&public, &wrong_purpose, &[]);
    assert_eq!(code, 1);
    assert!(stdout.starts_with("FAIL signature: signature was made for a different purpose\n"));

    // Only one of the two options.
    let (code, _, stderr) = run(&[
        "manifest",
        "verify",
        &manifest,
        "--dir",
        &images,
        "--public-key",
        &public,
    ]);
    assert_eq!(code, 2);
    assert!(stderr.contains("--public-key and --signature go together"));

    // The manifest is edited after signing: still canonical, no longer
    // authentic. Its content is not reported.
    let original = fs::read_to_string(&manifest).expect("manifest readable");
    fs::write(&manifest, original.replace("TEST.1", "TEST.2")).expect("edit");
    let (code, stdout, _) = verify(&public, &signature, &[]);
    assert_eq!(code, 1);
    assert_eq!(
        stdout,
        "FAIL signature: signature is not valid for this key and message\n\
         FAIL: the manifest is not authentic\n"
    );

    // An image changes after signing: authentic manifest, mismatching image.
    fs::write(&manifest, original).expect("restore");
    fs::write(&roothash, b"9999\n").expect("tamper");
    let (code, stdout, _) = verify(&public, &signature, &["--only", "system.roothash"]);
    assert_eq!(code, 1);
    assert!(stdout.contains("FAIL system.roothash: sha256 "), "{stdout}");
}

#[test]
fn root_suite_keys_sign_and_verify() {
    let scratch = Scratch::new("root-suite");
    let (secret, public) = (scratch.path("root.secret"), scratch.path("root.public"));
    let (code, stdout, stderr) = run(&[
        "key",
        "generate",
        "--suite",
        "ed25519+slh-dsa-sha2-256s",
        "--out-secret",
        &secret,
        "--out-public",
        &public,
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("suite: ed25519+slh-dsa-sha2-256s\n"));
    let (_, stdout, _) = run(&["key", "id", &public]);
    assert!(stdout.starts_with("suite: ed25519+slh-dsa-sha2-256s\n"));

    let message = scratch.file("root.txt", b"root metadata stand-in\n");
    let signature = scratch.path("root.sig");
    let (code, _, stderr) = run(&[
        "sign",
        &message,
        "--key",
        &secret,
        "--context",
        "root-metadata",
        "--out",
        &signature,
    ]);
    assert_eq!(code, 0, "{stderr}");
    let verify = |context: &str| {
        run(&[
            "verify",
            &message,
            "--public-key",
            &public,
            "--signature",
            &signature,
            "--context",
            context,
        ])
    };
    let (code, stdout, _) = verify("root-metadata");
    assert_eq!(code, 0, "{stdout}");
    assert!(stdout.starts_with("PASS: valid ed25519+slh-dsa-sha2-256s signature"));
    assert_eq!(verify("build-manifest").0, 1);

    let (code, _, stderr) = run(&[
        "key",
        "generate",
        "--suite",
        "rsa",
        "--out-secret",
        &scratch.path("x.secret"),
        "--out-public",
        &scratch.path("x.public"),
    ]);
    assert_eq!(code, 2);
    assert!(
        stderr.contains("unknown suite `rsa`; known: ed25519+ml-dsa-87, ed25519+slh-dsa-sha2-256s")
    );
}

#[test]
fn update_metadata_end_to_end() {
    let scratch = Scratch::new("update");
    let key = |name: &str, suite: &str| {
        let (secret, public) = (
            scratch.path(&format!("{name}.secret")),
            scratch.path(&format!("{name}.public")),
        );
        let (code, _, stderr) = run(&[
            "key",
            "generate",
            "--suite",
            suite,
            "--out-secret",
            &secret,
            "--out-public",
            &public,
        ]);
        assert_eq!(code, 0, "{stderr}");
        (secret, public)
    };
    let roots: Vec<_> = (0..3)
        .map(|i| key(&format!("root{i}"), "ed25519+slh-dsa-sha2-256s"))
        .collect();
    let releases: Vec<_> = (0..2)
        .map(|i| key(&format!("release{i}"), "ed25519+ml-dsa-87"))
        .collect();
    let sign = |file: &str, secret: &str, context: &str, out: &str| {
        let (code, _, stderr) = run(&[
            "sign",
            file,
            "--key",
            secret,
            "--context",
            context,
            "--out",
            out,
        ]);
        assert_eq!(code, 0, "{stderr}");
        out.to_owned()
    };
    let combine = |signatures: &[&str], out: &str| {
        let mut arguments = vec!["sigset"];
        arguments.extend_from_slice(signatures);
        arguments.extend_from_slice(&["--out", out]);
        run(&arguments)
    };

    // Root version 1: 2 of 3 root keys, 2 of 2 release keys.
    let root = scratch.path("root.txt");
    let (code, stdout, stderr) = run(&[
        "update",
        "root-create",
        "--version",
        "1",
        "--expires",
        "2027-11-13T00:00:00Z",
        "--root-threshold",
        "2",
        "--root-key",
        &roots[0].1,
        "--root-key",
        &roots[1].1,
        "--root-key",
        &roots[2].1,
        "--release-threshold",
        "2",
        "--release-key",
        &releases[0].1,
        "--release-key",
        &releases[1].1,
        "--out",
        &root,
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        stdout,
        format!("wrote {root}: root version 1, 3 root key(s), 2 release key(s)\n")
    );
    let root_sig0 = sign(
        &root,
        &roots[0].0,
        "root-metadata",
        &scratch.path("root.sig0"),
    );
    let root_sig1 = sign(
        &root,
        &roots[1].0,
        "root-metadata",
        &scratch.path("root.sig1"),
    );
    let root_set = scratch.path("root.sigs");
    assert_eq!(combine(&[&root_sig1, &root_sig0], &root_set).0, 0);
    let root_one = scratch.path("root.one");
    assert_eq!(combine(&[&root_sig0], &root_one).0, 0);
    let (code, _, stderr) = combine(&[&root_sig0, &root_sig0], &scratch.path("dup"));
    assert_eq!(code, 2);
    assert!(stderr.contains("strictly ascending"), "{stderr}");

    // Update metadata for release 7, signed by both release keys.
    let manifest = scratch.file("manifest.txt", b"nottos-build-manifest 1\n(stand-in)\n");
    let metadata = scratch.path("update.txt");
    let (code, stdout, stderr) = run(&[
        "update",
        "metadata-create",
        "--channel",
        "stable",
        "--target",
        "mini_x86_64",
        "--sequence",
        "7",
        "--issued",
        "2026-11-13T00:00:00Z",
        "--expires",
        "2026-12-13T00:00:00Z",
        "--manifest",
        &manifest,
        "--out",
        &metadata,
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, format!("wrote {metadata}: stable release 7\n"));
    let update_sig0 = sign(
        &metadata,
        &releases[0].0,
        "update-metadata",
        &scratch.path("u.sig0"),
    );
    let update_sig1 = sign(
        &metadata,
        &releases[1].0,
        "update-metadata",
        &scratch.path("u.sig1"),
    );
    let update_set = scratch.path("update.sigs");
    assert_eq!(combine(&[&update_sig0, &update_sig1], &update_set).0, 0);
    let update_one = scratch.path("update.one");
    assert_eq!(combine(&[&update_sig1], &update_one).0, 0);

    let verify = |root_signatures: &str, signatures: &str, manifest: &str, extra: &[&str]| {
        let mut arguments = vec![
            "update",
            "verify",
            "--root",
            &root,
            "--root-signatures",
            root_signatures,
            "--metadata",
            &metadata,
            "--signatures",
            signatures,
            "--manifest",
            manifest,
            "--channel",
            "stable",
            "--target",
            "mini_x86_64",
        ];
        arguments.extend_from_slice(extra);
        run(&arguments)
    };
    let now = ["--now", "2026-11-20T00:00:00Z"];

    let (code, stdout, stderr) = verify(&root_set, &update_set, &manifest, &now);
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert_eq!(
        stdout,
        format!(
            "ok   root version 1 from {root}\n\
             ok   update metadata: stable release 7 for mini_x86_64, expires 2026-12-13T00:00:00Z\n\
             ok   build manifest {manifest}\n\
             PASS: install release 7; then keep root version 1, last-sequence 7, \
             time-floor 2026-11-13T00:00:00Z\n"
        )
    );

    let failure = |result: (i32, String, String)| {
        assert_eq!(result.0, 1, "{}{}", result.1, result.2);
        result.1.lines().last().expect("a verdict").to_owned()
    };
    assert_eq!(
        failure(verify(&root_one, &update_set, &manifest, &now)),
        "FAIL: 1 valid signature(s) by root keys, 2 required"
    );
    assert_eq!(
        failure(verify(&root_set, &update_one, &manifest, &now)),
        "FAIL: 1 valid signature(s) by release keys, 2 required"
    );
    let mut installed = now.to_vec();
    installed.extend_from_slice(&["--last-sequence", "7"]);
    assert_eq!(
        failure(verify(&root_set, &update_set, &manifest, &installed)),
        "FAIL: release 7 is not newer than installed release 7"
    );
    assert_eq!(
        failure(verify(
            &root_set,
            &update_set,
            &manifest,
            &["--now", "2026-12-13T00:00:00Z"]
        )),
        "FAIL: the update metadata expired at 2026-12-13T00:00:00Z (now 2026-12-13T00:00:00Z)"
    );
    // The clock was set back, but the device remembers a later time.
    let mut floored = now.to_vec();
    floored.extend_from_slice(&["--time-floor", "2027-01-01T00:00:00Z"]);
    assert_eq!(
        failure(verify(&root_set, &update_set, &manifest, &floored)),
        "FAIL: the update metadata expired at 2026-12-13T00:00:00Z (now 2027-01-01T00:00:00Z)"
    );
    let other_manifest = scratch.file("other.txt", b"nottos-build-manifest 1\n(other)\n");
    assert_eq!(
        failure(verify(&root_set, &update_set, &other_manifest, &now)),
        "FAIL: the build manifest does not match the update metadata"
    );
    // Metadata edited after signing.
    let original = fs::read_to_string(&metadata).expect("metadata readable");
    fs::write(&metadata, original.replace("sequence: 7", "sequence: 8")).expect("edit");
    assert_eq!(
        failure(verify(&root_set, &update_set, &manifest, &now)),
        "FAIL: 0 valid signature(s) by release keys, 2 required"
    );
    fs::write(&metadata, original).expect("restore");

    // Usage errors.
    let (code, _, stderr) = run(&[
        "update",
        "root-create",
        "--version",
        "1",
        "--expires",
        "2027-11-13T00:00:00Z",
        "--root-threshold",
        "4",
        "--root-key",
        &roots[0].1,
        "--release-threshold",
        "1",
        "--release-key",
        &releases[0].1,
        "--out",
        &scratch.path("x"),
    ]);
    assert_eq!(code, 2);
    assert!(
        stderr.contains("--root-key must name 1 to 16 distinct keys"),
        "{stderr}"
    );
    let (code, _, stderr) = verify(&root_set, &update_set, &manifest, &["--now", "yesterday"]);
    assert_eq!(code, 2);
    assert!(
        stderr.contains("--now must look like 2026-11-13T00:00:00Z"),
        "{stderr}"
    );
}
