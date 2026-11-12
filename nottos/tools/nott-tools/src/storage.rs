//! `storage check`: invariant INV-02 of the threat model.
//!
//! Every mounted filesystem must be one of: verified read-only, volatile
//! (RAM), a kernel interface, or encrypted. Anything else must be on an
//! explicit exception list and mounted without execute, device or set-uid
//! permission.
//!
//! The input is a report collected on the device:
//!
//! ```text
//! [mounts]
//! <the lines of /proc/mounts>
//! [verity]
//! <one dm-verity block device per line>
//! [encrypted]
//! <one encrypted block device per line>
//! ```

use crate::args::Args;
use crate::util::read_text;
use crate::{CommandResult, Outcome};
use std::collections::BTreeSet;
use std::path::Path;

const MAX_INPUT_BYTES: u64 = 1024 * 1024;

/// Filesystems whose contents live in RAM and vanish at power-off.
const VOLATILE: &[&str] = &["devtmpfs", "ramfs", "rootfs", "tmpfs"];

/// Kernel interfaces that store nothing.
const KERNEL_INTERFACES: &[&str] = &[
    "binder",
    "bpf",
    "cgroup",
    "cgroup2",
    "configfs",
    "devpts",
    "proc",
    "securityfs",
    "selinuxfs",
    "sysfs",
    "tracefs",
];

/// Mount options an excepted filesystem must carry.
const EXCEPTION_OPTIONS: &[&str] = &["nodev", "noexec", "nosuid"];

#[derive(Debug, PartialEq, Eq)]
struct Mount {
    device: String,
    mount_point: String,
    filesystem: String,
    options: Vec<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Report {
    mounts: Vec<Mount>,
    verity: BTreeSet<String>,
    encrypted: BTreeSet<String>,
}

pub(crate) fn check(raw: &[&str]) -> CommandResult {
    let args = Args::parse(raw, &["exceptions"])?;
    let [report_path] = args.positionals()?;
    let report = parse_report(&read_text(Path::new(report_path), MAX_INPUT_BYTES)?)
        .map_err(|error| format!("{report_path}: {error}"))?;
    let exceptions = match args.optional("exceptions")? {
        Some(path) => parse_exceptions(&read_text(Path::new(path), MAX_INPUT_BYTES)?),
        None => BTreeSet::new(),
    };

    let mut text = String::new();
    let mut violations = 0usize;
    for mount in &report.mounts {
        let verdict = classify(mount, &report, &exceptions);
        let status = if verdict.is_ok() { "ok  " } else { "FAIL" };
        let (Ok(reason) | Err(reason)) = verdict;
        if verdict.is_err() {
            violations += 1;
        }
        text.push_str(&format!(
            "{status} {} ({} on {}): {reason}\n",
            mount.mount_point, mount.filesystem, mount.device
        ));
    }
    if violations == 0 {
        text.push_str(&format!("PASS: {} mount(s) checked\n", report.mounts.len()));
        Ok(Outcome::Pass(text))
    } else {
        text.push_str(&format!("FAIL: {violations} mount(s) violate the policy\n"));
        Ok(Outcome::Fail(text))
    }
}

fn parse_report(text: &str) -> Result<Report, String> {
    #[derive(Clone, Copy)]
    enum Section {
        Mounts,
        Verity,
        Encrypted,
    }
    let mut report = Report::default();
    let mut section = None;
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let line = line.trim_end();
        match (line, section) {
            ("", _) => {}
            ("[mounts]", _) => section = Some(Section::Mounts),
            ("[verity]", _) => section = Some(Section::Verity),
            ("[encrypted]", _) => section = Some(Section::Encrypted),
            (_, None) => return Err(format!("line {number}: text before the first section")),
            (_, Some(_)) if line.starts_with('[') => {
                return Err(format!("line {number}: unknown section {line}"))
            }
            (_, Some(Section::Mounts)) => {
                let fields: Vec<&str> = line.split(' ').collect();
                let &[device, mount_point, filesystem, options, _, _] = fields.as_slice() else {
                    return Err(format!("line {number}: not a /proc/mounts line"));
                };
                report.mounts.push(Mount {
                    device: device.to_owned(),
                    mount_point: mount_point.to_owned(),
                    filesystem: filesystem.to_owned(),
                    options: options.split(',').map(str::to_owned).collect(),
                });
            }
            (_, Some(Section::Verity)) => {
                report.verity.insert(line.to_owned());
            }
            (_, Some(Section::Encrypted)) => {
                report.encrypted.insert(line.to_owned());
            }
        }
    }
    if report.mounts.is_empty() {
        return Err("no mounts listed".to_owned());
    }
    Ok(report)
}

/// One mount point per line; `#` starts a comment.
fn parse_exceptions(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim())
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Returns why the mount is acceptable, or why it is not.
fn classify(
    mount: &Mount,
    report: &Report,
    exceptions: &BTreeSet<String>,
) -> Result<&'static str, &'static str> {
    let has = |option: &str| mount.options.iter().any(|present| present == option);
    let filesystem = mount.filesystem.as_str();

    if VOLATILE.contains(&filesystem) {
        Ok("volatile, in RAM")
    } else if KERNEL_INTERFACES.contains(&filesystem) {
        Ok("kernel interface")
    } else if report.verity.contains(&mount.device) {
        if has("ro") {
            Ok("verified, read-only")
        } else {
            Err("verified device is mounted writable")
        }
    } else if report.encrypted.contains(&mount.device) {
        Ok("encrypted")
    } else if exceptions.contains(&mount.mount_point) {
        if EXCEPTION_OPTIONS.iter().all(|option| has(option)) {
            Ok("listed exception, mounted nosuid,nodev,noexec")
        } else {
            Err("listed exception lacks nosuid, nodev or noexec")
        }
    } else {
        Err("neither verified, volatile nor encrypted, and not a listed exception")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = "\
[mounts]
/dev/mapper/system / erofs ro,relatime 0 0
proc /proc proc rw,relatime 0 0
tmpfs /run tmpfs rw,nosuid,nodev 0 0
/dev/mapper/data /data ext4 rw,relatime 0 0
/dev/vdd /scratch ext4 rw,nosuid,nodev,noexec,relatime 0 0

[verity]
/dev/mapper/system
[encrypted]
/dev/mapper/data
";

    fn verdicts(report: &str, exceptions: &str) -> Vec<Result<&'static str, &'static str>> {
        let report = parse_report(report).expect("report parses");
        let exceptions = parse_exceptions(exceptions);
        report
            .mounts
            .iter()
            .map(|mount| classify(mount, &report, &exceptions))
            .collect()
    }

    #[test]
    fn accepts_a_compliant_system() {
        let exceptions = "# test-only scratch partition\n/scratch  # control data\n";
        assert_eq!(
            verdicts(REPORT, exceptions),
            vec![
                Ok("verified, read-only"),
                Ok("kernel interface"),
                Ok("volatile, in RAM"),
                Ok("encrypted"),
                Ok("listed exception, mounted nosuid,nodev,noexec"),
            ]
        );
    }

    #[test]
    fn unlisted_plain_storage_is_a_violation() {
        assert_eq!(
            verdicts(REPORT, "")[4],
            Err("neither verified, volatile nor encrypted, and not a listed exception")
        );
    }

    #[test]
    fn exception_must_be_mounted_restrictively() {
        let report = REPORT.replace("rw,nosuid,nodev,noexec,relatime", "rw,nosuid,nodev");
        assert_eq!(
            verdicts(&report, "/scratch")[4],
            Err("listed exception lacks nosuid, nodev or noexec")
        );
    }

    #[test]
    fn verified_device_must_be_read_only() {
        let report = REPORT.replace("erofs ro,relatime", "erofs rw,relatime");
        assert_eq!(
            verdicts(&report, "/scratch")[0],
            Err("verified device is mounted writable")
        );
    }

    #[test]
    fn unknown_filesystems_are_not_trusted() {
        let report = REPORT.replace(
            "proc /proc proc rw,relatime 0 0",
            "debugfs /sys/kernel/debug debugfs rw 0 0",
        );
        assert!(verdicts(&report, "/scratch")[1].is_err());
    }

    #[test]
    fn rejects_malformed_reports() {
        assert_eq!(
            parse_report("/dev/vda / ext4 rw 0 0\n"),
            Err("line 1: text before the first section".to_owned())
        );
        assert_eq!(
            parse_report("[mounts]\n/dev/vda / ext4 rw\n"),
            Err("line 2: not a /proc/mounts line".to_owned())
        );
        assert_eq!(
            parse_report("[mounts]\n[other]\n"),
            Err("line 2: unknown section [other]".to_owned())
        );
        assert_eq!(
            parse_report("[verity]\n/dev/dm-0\n"),
            Err("no mounts listed".to_owned())
        );
    }
}
