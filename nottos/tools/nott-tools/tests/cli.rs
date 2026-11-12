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
