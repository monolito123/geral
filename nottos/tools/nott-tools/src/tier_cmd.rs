// SPDX-License-Identifier: Apache-2.0

//! `tier check`: which tier can a device reach, and what is its posture?
//!
//! First version of the tier checklist tool (ADR-0002, rule 5). It reads a
//! capability descriptor. Filling a descriptor in from a live device is the
//! job of the on-device probe, which does not exist yet.

use crate::args::Args;
use crate::util::read_text;
use crate::{CommandResult, Outcome};
use nott_policy::tier::{Arch, Kind};
use nott_policy::{assess, Assessment, Descriptor, Tier};
use std::path::Path;

const MAX_DESCRIPTOR_BYTES: u64 = 64 * 1024;

pub(crate) fn check(raw: &[&str]) -> CommandResult {
    let args = Args::parse(raw, &[])?;
    let [path] = args.positionals()?;
    let text = read_text(Path::new(path), MAX_DESCRIPTOR_BYTES)?;
    let descriptor = Descriptor::from_toml(&text).map_err(|error| format!("{path}: {error}"))?;
    let assessment = assess(&descriptor);
    let report = render(&descriptor, &assessment);
    if assessment.tier == Tier::Excluded {
        Ok(Outcome::Fail(report))
    } else {
        Ok(Outcome::Pass(report))
    }
}

fn render(descriptor: &Descriptor, assessment: &Assessment) -> String {
    let kind = match descriptor.kind {
        Kind::Phone => "phone",
        Kind::Tablet => "tablet",
        Kind::Emulator => "emulator",
    };
    let arch = match descriptor.arch {
        Arch::Arm64 => "arm64",
        Arch::X86_64 => "x86_64",
        Arch::Riscv64 => "riscv64",
    };
    let mut report = format!(
        "Device: {} ({kind}, {arch})\nTier: {}\n",
        descriptor.name,
        assessment.tier.label()
    );
    let mut section = |title: &str, lines: &[&str]| {
        if !lines.is_empty() {
            report.push_str(&format!("\n{title}\n"));
            for line in lines {
                report.push_str(&format!("  - {line}\n"));
            }
        }
    };
    section("Excluded because:", &assessment.exclusions);
    section("Missing for Tier A:", &assessment.missing_for_tier_a);
    section(
        "Before this device can be listed as supported:",
        &assessment.open_items,
    );
    report.push_str("\nPosture:\n");
    for item in &assessment.posture {
        report.push_str(&format!(
            "  {:<8} {}: {}\n",
            item.state.label(),
            item.mechanism,
            item.detail
        ));
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESCRIPTOR: &str = r#"
schema = 1
name = "example"
kind = "phone"
arch = "arm64"
[boot]
relock_with_custom_key = "yes"
rollback_protection = "yes"
fingerprint_on_boot_screen = "truncated"
verified_boot_bypass_known = "no"
[keystore]
tee = "hardware"
secure_element = "no"
strongbox = "no"
weaver = "no"
[isolation]
radio_iommu = "unconfirmed"
[firmware]
security_updates = "timely"
[memory]
mte = "no"
[usb]
controller_data_disable = "no"
[kernel]
hardening_profile = "limited"
[telephony]
emergency_call = "untested"
"#;

    #[test]
    fn report_lists_tier_gaps_and_posture() {
        let descriptor = Descriptor::from_toml(DESCRIPTOR).expect("valid");
        let report = render(&descriptor, &assess(&descriptor));
        let expected_start = "\
Device: example (phone, arm64)
Tier: B, boot verified (reduced guarantees)

Missing for Tier A:
  - secure element with StrongBox and unlock throttling
  - IOMMU isolation of radios, confirmed by audit

Before this device can be listed as supported:
  - verify emergency calling
  - complete the device audit: some mechanisms are unknown

Posture:
  full     Verified boot with the NottOS key: bootloader relocks with the NottOS key
";
        assert!(report.starts_with(expected_start), "report was:\n{report}");
        assert!(report.contains("  reduced  USB data off while locked: kernel only\n"));
        assert!(report.contains("  absent   Secure element: none; keys rely on the TEE alone\n"));
        assert!(report.contains("  unknown  Emergency calling: not tested\n"));
        assert!(!report.contains("Excluded because:"));
    }
}
