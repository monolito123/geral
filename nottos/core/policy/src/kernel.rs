// SPDX-License-Identifier: Apache-2.0

//! Kernel hardening profile and the check of a kernel configuration
//! against it (`docs/adr/0011-kernel-hardening-profile.md`).
//!
//! The profile is a list of rules, each requiring an exact value for an
//! option or forbidding it. The configuration is a kernel `.config` file.
//! An option that does not appear in the configuration is unset.

use std::collections::BTreeMap;
use std::fmt;

/// CPU architectures a profile section can apply to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelArch {
    /// 64-bit Arm.
    Arm64,
    /// 64-bit x86.
    X86_64,
}

impl KernelArch {
    /// Parses `arm64` or `x86_64`.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "arm64" => Some(Self::Arm64),
            "x86_64" => Some(Self::X86_64),
            _ => None,
        }
    }
}

/// What a rule demands of one option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Requirement {
    /// The option must be set to exactly this value.
    Equals(String),
    /// The option must not be set.
    Unset,
}

/// One rule of the profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// The option, including its `CONFIG_` prefix.
    pub option: String,
    /// What it must be.
    pub requirement: Requirement,
    /// Why, as written in the profile. May be empty.
    pub reason: String,
    /// `None` for every architecture.
    pub arch: Option<KernelArch>,
}

/// A parsed profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    rules: Vec<Rule>,
}

/// Why a profile or configuration was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// One-based line number.
    pub line: usize,
    /// What is wrong.
    pub message: &'static str,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Kconfig symbol names: letters, digits and underscores. Real
/// configurations contain lower-case letters (for example
/// `CONFIG_MTD_NETtel`).
fn is_option(name: &str) -> bool {
    name.strip_prefix("CONFIG_").is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    })
}

impl Profile {
    /// Parses the profile format described in `kernel/hardening-profile.txt`.
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let mut rules: Vec<Rule> = Vec::new();
        let mut section: Option<Option<KernelArch>> = None;
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            let error = |message| ParseError { line, message };
            let content = raw.trim();
            if content.is_empty() || content.starts_with('#') {
                continue;
            }
            if let Some(name) = content
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
            {
                section = Some(match name {
                    "all" => None,
                    other => Some(KernelArch::from_name(other).ok_or(error("unknown section"))?),
                });
                continue;
            }
            let arch = section.ok_or(error("rule before the first section"))?;
            let mut words = content.split_whitespace();
            let (verb, target) = (words.next(), words.next());
            let reason = words.collect::<Vec<_>>().join(" ");
            let (option, requirement) = match (verb, target) {
                (Some("require"), Some(target)) => {
                    let (option, value) = target
                        .split_once('=')
                        .ok_or(error("require needs OPTION=VALUE"))?;
                    if value.is_empty() {
                        return Err(error("require needs a value"));
                    }
                    (option, Requirement::Equals(value.to_owned()))
                }
                (Some("forbid"), Some(target)) => (target, Requirement::Unset),
                _ => return Err(error("expected `require` or `forbid` and an option")),
            };
            if !is_option(option) {
                return Err(error("not a CONFIG_ option name"));
            }
            if rules.iter().any(|rule| {
                rule.option == option
                    && (rule.arch == arch || rule.arch.is_none() || arch.is_none())
            }) {
                return Err(error("option listed twice for the same architecture"));
            }
            rules.push(Rule {
                option: option.to_owned(),
                requirement,
                reason,
                arch,
            });
        }
        if rules.is_empty() {
            return Err(ParseError {
                line: 0,
                message: "profile has no rules",
            });
        }
        Ok(Self { rules })
    }

    /// The rules that apply to `arch`, in profile order.
    pub fn rules_for(&self, arch: KernelArch) -> impl Iterator<Item = &Rule> {
        self.rules
            .iter()
            .filter(move |rule| rule.arch.is_none_or(|only| only == arch))
    }
}

/// A kernel configuration: option name to value. Unset options are absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelConfig {
    values: BTreeMap<String, String>,
}

impl KernelConfig {
    /// Parses a kernel `.config` file: `CONFIG_X=value` lines and
    /// `# CONFIG_X is not set` lines; other comments and blank lines are
    /// ignored. An option given twice is an error.
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let mut values = BTreeMap::new();
        let mut unset = std::collections::BTreeSet::new();
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            let error = |message| ParseError { line, message };
            let content = raw.trim_end();
            if content.is_empty() {
                continue;
            }
            if let Some(comment) = content.strip_prefix('#') {
                if let Some(option) = comment
                    .trim_start()
                    .strip_suffix(" is not set")
                    .filter(|option| is_option(option))
                {
                    if values.contains_key(option) || !unset.insert(option.to_owned()) {
                        return Err(error("option given twice"));
                    }
                }
                continue;
            }
            let (option, value) = content
                .split_once('=')
                .ok_or(error("expected CONFIG_NAME=VALUE"))?;
            if !is_option(option) {
                return Err(error("not a CONFIG_ option name"));
            }
            if unset.contains(option)
                || values.insert(option.to_owned(), value.to_owned()).is_some()
            {
                return Err(error("option given twice"));
            }
        }
        Ok(Self { values })
    }

    /// The option's value, or `None` if it is unset.
    pub fn get(&self, option: &str) -> Option<&str> {
        self.values.get(option).map(String::as_str)
    }

    /// Number of options that are set.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether no option is set.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// The outcome of one rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The configuration satisfies the rule.
    Ok,
    /// A required option is unset.
    Missing,
    /// A required option has another value.
    Wrong {
        /// The value found.
        found: String,
    },
    /// A forbidden option is set.
    Present {
        /// The value found.
        found: String,
    },
}

/// Checks a configuration against the rules of `profile` for `arch`.
pub fn check<'p>(
    profile: &'p Profile,
    arch: KernelArch,
    config: &KernelConfig,
) -> Vec<(&'p Rule, Verdict)> {
    profile
        .rules_for(arch)
        .map(|rule| {
            let found = config.get(&rule.option);
            let verdict = match (&rule.requirement, found) {
                (Requirement::Equals(want), Some(have)) if want == have => Verdict::Ok,
                (Requirement::Equals(_), Some(have)) => Verdict::Wrong {
                    found: have.to_owned(),
                },
                (Requirement::Equals(_), None) => Verdict::Missing,
                (Requirement::Unset, None) => Verdict::Ok,
                (Requirement::Unset, Some(have)) => Verdict::Present {
                    found: have.to_owned(),
                },
            };
            (rule, verdict)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &str = "\
# comment
[all]
require CONFIG_A=y     first reason
forbid CONFIG_B        second reason
require CONFIG_N=65536
[arm64]
require CONFIG_ARM=y
[x86_64]
forbid CONFIG_X86_MSR
";

    #[test]
    fn parses_sections_rules_and_reasons() {
        let profile = Profile::parse(PROFILE).expect("valid");
        let arm: Vec<&str> = profile
            .rules_for(KernelArch::Arm64)
            .map(|rule| rule.option.as_str())
            .collect();
        assert_eq!(arm, ["CONFIG_A", "CONFIG_B", "CONFIG_N", "CONFIG_ARM"]);
        let x86: Vec<&str> = profile
            .rules_for(KernelArch::X86_64)
            .map(|rule| rule.option.as_str())
            .collect();
        assert_eq!(x86, ["CONFIG_A", "CONFIG_B", "CONFIG_N", "CONFIG_X86_MSR"]);
        let first = profile.rules_for(KernelArch::Arm64).next().expect("a rule");
        assert_eq!(first.reason, "first reason");
        assert_eq!(first.requirement, Requirement::Equals("y".to_owned()));
    }

    #[test]
    fn checks_every_kind_of_outcome() {
        let profile = Profile::parse(PROFILE).expect("valid");
        let config = KernelConfig::parse(
            "\
#
# Automatically generated file; DO NOT EDIT.
CONFIG_A=y
CONFIG_B=m
CONFIG_N=4096
# CONFIG_ARM is not set
CONFIG_X86_MSR=y
",
        )
        .expect("valid");
        let verdicts: Vec<(&str, Verdict)> = check(&profile, KernelArch::Arm64, &config)
            .into_iter()
            .map(|(rule, verdict)| (rule.option.as_str(), verdict))
            .collect();
        assert_eq!(
            verdicts,
            [
                ("CONFIG_A", Verdict::Ok),
                (
                    "CONFIG_B",
                    Verdict::Present {
                        found: "m".to_owned()
                    }
                ),
                (
                    "CONFIG_N",
                    Verdict::Wrong {
                        found: "4096".to_owned()
                    }
                ),
                ("CONFIG_ARM", Verdict::Missing),
            ]
        );
        // On x86_64 the forbidden MSR driver is present; the arm64 rule
        // does not apply.
        let x86 = check(&profile, KernelArch::X86_64, &config);
        assert_eq!(
            x86.last()
                .map(|(rule, verdict)| (rule.option.as_str(), verdict.clone())),
            Some((
                "CONFIG_X86_MSR",
                Verdict::Present {
                    found: "y".to_owned()
                }
            ))
        );
    }

    #[test]
    fn absent_and_not_set_mean_the_same() {
        let profile = Profile::parse("[all]\nforbid CONFIG_B\nrequire CONFIG_A=y\n").expect("ok");
        for config in ["# CONFIG_B is not set\n", ""] {
            let config = KernelConfig::parse(config).expect("valid");
            let verdicts: Vec<Verdict> = check(&profile, KernelArch::Arm64, &config)
                .into_iter()
                .map(|(_, verdict)| verdict)
                .collect();
            assert_eq!(verdicts, [Verdict::Ok, Verdict::Missing]);
        }
    }

    #[test]
    fn rejects_malformed_profiles() {
        let cases = [
            ("require CONFIG_A=y\n", 1, "rule before the first section"),
            ("[mips]\n", 1, "unknown section"),
            ("[all]\nrequire CONFIG_A\n", 2, "require needs OPTION=VALUE"),
            ("[all]\nrequire CONFIG_A=\n", 2, "require needs a value"),
            (
                "[all]\nallow CONFIG_A\n",
                2,
                "expected `require` or `forbid` and an option",
            ),
            ("[all]\nforbid A\n", 2, "not a CONFIG_ option name"),
            ("[all]\nforbid CONFIG_A-B\n", 2, "not a CONFIG_ option name"),
            ("[all]\nforbid CONFIG_\n", 2, "not a CONFIG_ option name"),
            (
                "[all]\nforbid CONFIG_A\n[arm64]\nrequire CONFIG_A=y\n",
                4,
                "option listed twice for the same architecture",
            ),
        ];
        for (text, line, message) in cases {
            assert_eq!(
                Profile::parse(text),
                Err(ParseError { line, message }),
                "{text:?}"
            );
        }
        assert!(Profile::parse("# nothing\n").is_err());
        // The same option may be ruled differently per architecture.
        assert!(Profile::parse("[arm64]\nrequire CONFIG_A=y\n[x86_64]\nforbid CONFIG_A\n").is_ok());
    }

    #[test]
    fn rejects_malformed_configurations() {
        assert_eq!(
            KernelConfig::parse("CONFIG_A=y\nCONFIG_A=n\n"),
            Err(ParseError {
                line: 2,
                message: "option given twice"
            })
        );
        assert_eq!(
            KernelConfig::parse("CONFIG_A=y\n# CONFIG_A is not set\n"),
            Err(ParseError {
                line: 2,
                message: "option given twice"
            })
        );
        assert_eq!(
            KernelConfig::parse("garbage\n"),
            Err(ParseError {
                line: 1,
                message: "expected CONFIG_NAME=VALUE"
            })
        );
        let config = KernelConfig::parse("CONFIG_S=\"a b\"\nCONFIG_MTD_NETtel=m\n").expect("valid");
        assert_eq!(config.get("CONFIG_S"), Some("\"a b\""));
        assert_eq!(config.get("CONFIG_MTD_NETtel"), Some("m"));
        assert_eq!(config.len(), 2);
    }

    #[test]
    fn the_shipped_profile_parses() {
        let text = include_str!("../../../kernel/hardening-profile.txt");
        let profile = Profile::parse(text).expect("shipped profile is valid");
        assert!(profile.rules_for(KernelArch::Arm64).count() > 60);
        assert!(profile.rules_for(KernelArch::X86_64).count() > 60);
        // The rules the brief names explicitly are in it.
        for option in [
            "CONFIG_CFI_CLANG",
            "CONFIG_SHADOW_CALL_STACK",
            "CONFIG_ARM64_PTR_AUTH_KERNEL",
            "CONFIG_ARM64_BTI_KERNEL",
            "CONFIG_ARM64_MTE",
            "CONFIG_INIT_ON_ALLOC_DEFAULT_ON",
            "CONFIG_INIT_ON_FREE_DEFAULT_ON",
            "CONFIG_STRICT_KERNEL_RWX",
            "CONFIG_SECURITY_LOCKDOWN_LSM",
            "CONFIG_MODULE_SIG_FORCE",
        ] {
            assert!(
                profile
                    .rules_for(KernelArch::Arm64)
                    .any(|rule| rule.option == option),
                "{option}"
            );
        }
    }
}
