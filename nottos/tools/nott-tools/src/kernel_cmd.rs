// SPDX-License-Identifier: Apache-2.0

//! `kernel check`: a kernel configuration against the hardening profile
//! (`docs/adr/0011-kernel-hardening-profile.md`).

use crate::args::Args;
use crate::util::read_text;
use crate::{CommandResult, Outcome};
use nott_policy::kernel::{check as check_config, KernelArch, KernelConfig, Profile, Verdict};
use std::path::Path;

const MAX_INPUT_BYTES: u64 = 8 * 1024 * 1024;

pub(crate) fn check(raw: &[&str]) -> CommandResult {
    let args = Args::parse(raw, &["profile", "arch"])?;
    let [config_path] = args.positionals()?;
    let profile_path = args.required("profile")?;
    let arch_name = args.required("arch")?;
    let arch = KernelArch::from_name(arch_name)
        .ok_or_else(|| format!("--arch must be arm64 or x86_64, not `{arch_name}`"))?;
    let profile = Profile::parse(&read_text(Path::new(profile_path), MAX_INPUT_BYTES)?)
        .map_err(|error| format!("{profile_path}: {error}"))?;
    let config = KernelConfig::parse(&read_text(Path::new(config_path), MAX_INPUT_BYTES)?)
        .map_err(|error| format!("{config_path}: {error}"))?;

    let verdicts = check_config(&profile, arch, &config);
    let mut report = String::new();
    let mut failures = 0usize;
    for (rule, verdict) in &verdicts {
        let line = match verdict {
            Verdict::Ok => format!("ok      {}", rule.option),
            Verdict::Missing => format!("MISSING {} (required)", rule.option),
            Verdict::Wrong { found } => format!("WRONG   {}={found}", rule.option),
            Verdict::Present { found } => format!("PRESENT {}={found} (forbidden)", rule.option),
        };
        if *verdict != Verdict::Ok {
            failures += 1;
        }
        report.push_str(&line);
        if *verdict != Verdict::Ok && !rule.reason.is_empty() {
            report.push_str(&format!("  -- {}", rule.reason));
        }
        report.push('\n');
    }
    let total = verdicts.len();
    if failures == 0 {
        report.push_str(&format!(
            "PASS: all {total} rules for {arch_name} satisfied\n"
        ));
        Ok(Outcome::Pass(report))
    } else {
        report.push_str(&format!(
            "FAIL: {failures} of {total} rules for {arch_name} not satisfied\n"
        ));
        Ok(Outcome::Fail(report))
    }
}
