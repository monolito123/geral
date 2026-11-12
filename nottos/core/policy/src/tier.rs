// SPDX-License-Identifier: Apache-2.0

//! Device capability descriptor and tier assessment.
//!
//! Implements `docs/adr/0002-device-support-tiers.md`. A device directory
//! carries a capability descriptor: a declaration of what the hardware and
//! its firmware provide. [`assess`] turns a descriptor into the tier the
//! device can reach and the state of each protection mechanism, which is
//! what the posture screen shows.
//!
//! The descriptor records what an audit found. `unknown` is a valid answer
//! and is never treated as `yes`.

use serde::Deserialize;
use std::fmt;

/// The descriptor schema version this crate reads.
pub const SCHEMA_VERSION: u32 = 1;

/// A yes/no fact that an audit may not have established yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    /// Established to be true.
    Yes,
    /// Established to be false.
    No,
    /// Not established.
    Unknown,
}

/// What kind of device this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A phone. Emergency calling must be verified.
    Phone,
    /// A tablet, with or without telephony.
    Tablet,
    /// A virtual device. It has no hardware guarantees.
    Emulator,
}

/// CPU architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Arch {
    /// 64-bit Arm.
    #[serde(rename = "arm64")]
    Arm64,
    /// 64-bit x86.
    #[serde(rename = "x86_64")]
    X86_64,
    /// 64-bit RISC-V.
    #[serde(rename = "riscv64")]
    Riscv64,
}

/// How the bootloader shows the verified-boot key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fingerprint {
    /// The complete fingerprint.
    Full,
    /// A shortened fingerprint.
    Truncated,
    /// Nothing.
    #[serde(rename = "none")]
    Absent,
    /// Not established.
    Unknown,
}

/// What backs the keystore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backing {
    /// A trusted execution environment in hardware.
    Hardware,
    /// A software implementation.
    Software,
    /// No keystore isolation at all.
    #[serde(rename = "none")]
    Absent,
    /// Not established.
    Unknown,
}

/// IOMMU confinement of radio chips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Iommu {
    /// Present and confirmed by a device audit.
    Audited,
    /// Believed present, not audited.
    Unconfirmed,
    /// Not present.
    Absent,
    /// Not established.
    Unknown,
}

/// Firmware security update status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Updates {
    /// The vendor ships security patches promptly.
    Timely,
    /// Patches arrive late.
    Lagging,
    /// The vendor has stopped.
    Ended,
    /// Not established.
    Unknown,
}

/// Which kernel hardening profile the port can apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    /// NottOS builds the kernel and vendor modules.
    Full,
    /// Limited to what the vendor's kernel allows.
    Limited,
    /// Not established.
    Unknown,
}

/// Emergency calling status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Emergency {
    /// Shown to work by the approved test procedure.
    Verified,
    /// Shown not to work.
    Failed,
    /// Not tested yet.
    Untested,
    /// The device has no telephony.
    NotApplicable,
}

/// `[boot]` section.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boot {
    /// The bootloader can be relocked with a key the owner sets.
    pub relock_with_custom_key: Answer,
    /// Rollback indexes are enforced from tamper-evident storage.
    pub rollback_protection: Answer,
    /// How the verified-boot key is shown at boot.
    pub fingerprint_on_boot_screen: Fingerprint,
    /// A public, unpatchable way around verified boot exists.
    pub verified_boot_bypass_known: Answer,
}

/// `[keystore]` section.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keystore {
    /// What backs the main keystore.
    pub tee: Backing,
    /// A discrete secure element is present.
    pub secure_element: Answer,
    /// The secure element offers a StrongBox keystore.
    pub strongbox: Answer,
    /// The secure element throttles unlock attempts (Weaver).
    pub weaver: Answer,
}

/// `[isolation]` section.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Isolation {
    /// IOMMU confinement of baseband, Wi-Fi, Bluetooth and NFC.
    pub radio_iommu: Iommu,
}

/// `[firmware]` section.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Firmware {
    /// Vendor security update status.
    pub security_updates: Updates,
}

/// `[memory]` section.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Memory {
    /// Hardware memory tagging is usable.
    pub mte: Answer,
}

/// `[usb]` section.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usb {
    /// USB data can be disabled at the port controller.
    pub controller_data_disable: Answer,
}

/// `[kernel]` section.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kernel {
    /// Which hardening profile applies.
    pub hardening_profile: Profile,
}

/// `[telephony]` section.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Telephony {
    /// Emergency calling status.
    pub emergency_call: Emergency,
}

/// A device capability descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    /// Must equal [`SCHEMA_VERSION`].
    pub schema: u32,
    /// Device code name.
    pub name: String,
    /// Phone, tablet or emulator.
    pub kind: Kind,
    /// CPU architecture.
    pub arch: Arch,
    /// Boot chain.
    pub boot: Boot,
    /// Key storage.
    pub keystore: Keystore,
    /// Peripheral isolation.
    pub isolation: Isolation,
    /// Firmware support.
    pub firmware: Firmware,
    /// Memory safety hardware.
    pub memory: Memory,
    /// USB controls.
    pub usb: Usb,
    /// Kernel hardening.
    pub kernel: Kernel,
    /// Telephony.
    pub telephony: Telephony,
}

/// Why a descriptor was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptorError {
    /// Not valid TOML, or a field is missing, unknown or has an invalid value.
    Syntax(String),
    /// The `schema` field names a version this crate does not read.
    UnsupportedSchema(u32),
    /// The fields contradict each other.
    Inconsistent(&'static str),
}

impl fmt::Display for DescriptorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(message) => write!(f, "invalid descriptor: {message}"),
            Self::UnsupportedSchema(version) => write!(
                f,
                "descriptor schema {version} is not supported (expected {SCHEMA_VERSION})"
            ),
            Self::Inconsistent(reason) => write!(f, "inconsistent descriptor: {reason}"),
        }
    }
}

impl std::error::Error for DescriptorError {}

impl Descriptor {
    /// Parses and validates a descriptor in TOML form.
    pub fn from_toml(text: &str) -> Result<Self, DescriptorError> {
        let descriptor: Self = toml::from_str(text)
            .map_err(|error| DescriptorError::Syntax(error.message().to_owned()))?;
        if descriptor.schema != SCHEMA_VERSION {
            return Err(DescriptorError::UnsupportedSchema(descriptor.schema));
        }
        if descriptor.name.is_empty() {
            return Err(DescriptorError::Inconsistent("name is empty"));
        }
        if descriptor.kind == Kind::Phone
            && descriptor.telephony.emergency_call == Emergency::NotApplicable
        {
            return Err(DescriptorError::Inconsistent(
                "a phone cannot declare emergency calling not applicable",
            ));
        }
        if descriptor.keystore.secure_element == Answer::No
            && (descriptor.keystore.strongbox == Answer::Yes
                || descriptor.keystore.weaver == Answer::Yes)
        {
            return Err(DescriptorError::Inconsistent(
                "strongbox and weaver require a secure element",
            ));
        }
        Ok(descriptor)
    }
}

/// The support tier a device can reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// Full guarantees.
    A,
    /// Reduced guarantees; the boot chain is verified with the NottOS key.
    BVerified,
    /// Reduced guarantees; the boot chain is not verified.
    BUnverified,
    /// May not carry the NottOS name.
    Excluded,
    /// A virtual development target with no hardware guarantees.
    Emulator,
}

impl Tier {
    /// Short human-readable description.
    pub fn label(self) -> &'static str {
        match self {
            Self::A => "A (full guarantees)",
            Self::BVerified => "B, boot verified (reduced guarantees)",
            Self::BUnverified => "B, boot NOT verified (reduced guarantees)",
            Self::Excluded => "excluded (may not carry the NottOS name)",
            Self::Emulator => "emulator (development target, no hardware guarantees)",
        }
    }
}

/// State of one protection mechanism on a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// In place in its strongest form, enforced by hardware where that
    /// applies.
    Full,
    /// In place in a weaker form, typically enforced by software only.
    Reduced,
    /// Not available.
    Absent,
    /// Not established; needs an audit or a test.
    Unknown,
    /// Does not apply to this device.
    NotApplicable,
}

impl State {
    /// Short human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Reduced => "reduced",
            Self::Absent => "absent",
            Self::Unknown => "unknown",
            Self::NotApplicable => "n/a",
        }
    }
}

/// One line of the posture screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostureItem {
    /// The mechanism.
    pub mechanism: &'static str,
    /// Its state on this device.
    pub state: State,
    /// What that state means here.
    pub detail: &'static str,
}

/// Result of assessing a descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assessment {
    /// The tier the device can reach.
    pub tier: Tier,
    /// Reasons the device is excluded. Empty unless `tier` is
    /// [`Tier::Excluded`].
    pub exclusions: Vec<&'static str>,
    /// Tier A requirements the device does not meet. Empty for Tier A and
    /// for emulators.
    pub missing_for_tier_a: Vec<&'static str>,
    /// What must be audited or tested before the device may be listed as
    /// supported.
    pub open_items: Vec<&'static str>,
    /// Every mechanism and its state, in a fixed order.
    pub posture: Vec<PostureItem>,
}

/// Assesses a descriptor against ADR-0002.
pub fn assess(descriptor: &Descriptor) -> Assessment {
    let boot = &descriptor.boot;
    let keystore = &descriptor.keystore;

    let mut exclusions = Vec::new();
    if boot.verified_boot_bypass_known == Answer::Yes {
        exclusions.push("a public, unpatchable bypass of verified boot exists");
    }
    if descriptor.telephony.emergency_call == Emergency::Failed {
        exclusions.push("emergency calling does not work");
    }

    // Tier A, as defined in section 13 of the brief. MTE is "ideally" and is
    // therefore a posture item, not a requirement.
    let requirements = [
        (
            boot.relock_with_custom_key == Answer::Yes,
            "bootloader that relocks with a custom key",
        ),
        (
            boot.rollback_protection == Answer::Yes,
            "rollback protection",
        ),
        (
            keystore.tee == Backing::Hardware,
            "hardware-backed keystore",
        ),
        (
            keystore.secure_element == Answer::Yes
                && keystore.strongbox == Answer::Yes
                && keystore.weaver == Answer::Yes,
            "secure element with StrongBox and unlock throttling",
        ),
        (
            descriptor.isolation.radio_iommu == Iommu::Audited,
            "IOMMU isolation of radios, confirmed by audit",
        ),
        (
            descriptor.firmware.security_updates == Updates::Timely,
            "timely firmware security updates",
        ),
    ];
    let missing: Vec<&'static str> = requirements
        .iter()
        .filter(|(met, _)| !met)
        .map(|(_, what)| *what)
        .collect();

    let (tier, missing_for_tier_a) = if descriptor.kind == Kind::Emulator {
        (Tier::Emulator, Vec::new())
    } else if !exclusions.is_empty() {
        (Tier::Excluded, missing)
    } else if missing.is_empty() {
        (Tier::A, missing)
    } else if boot.relock_with_custom_key == Answer::Yes {
        (Tier::BVerified, missing)
    } else {
        (Tier::BUnverified, missing)
    };
    if tier != Tier::Excluded {
        exclusions.clear();
    }

    let posture = posture(descriptor);
    let mut open_items = Vec::new();
    if boot.verified_boot_bypass_known == Answer::Unknown {
        open_items.push("check for known verified-boot bypasses");
    }
    if descriptor.telephony.emergency_call == Emergency::Untested {
        open_items.push("verify emergency calling");
    }
    if posture.iter().any(|item| item.state == State::Unknown) {
        open_items.push("complete the device audit: some mechanisms are unknown");
    }

    Assessment {
        tier,
        exclusions,
        missing_for_tier_a,
        open_items,
        posture,
    }
}

fn posture(descriptor: &Descriptor) -> Vec<PostureItem> {
    let boot = &descriptor.boot;
    let keystore = &descriptor.keystore;
    let item = |mechanism, (state, detail)| PostureItem {
        mechanism,
        state,
        detail,
    };
    let yes_no = |answer, yes: &'static str, no: &'static str| match answer {
        Answer::Yes => (State::Full, yes),
        Answer::No => (State::Absent, no),
        Answer::Unknown => (State::Unknown, "not established"),
    };

    vec![
        item(
            "Verified boot with the NottOS key",
            yes_no(
                boot.relock_with_custom_key,
                "bootloader relocks with the NottOS key",
                "bootloader stays unlocked; a modified system boots",
            ),
        ),
        item(
            "Rollback protection",
            yes_no(
                boot.rollback_protection,
                "older system versions are refused",
                "an older, vulnerable system can be installed",
            ),
        ),
        item(
            "Key fingerprint on the boot screen",
            match boot.fingerprint_on_boot_screen {
                Fingerprint::Full => (State::Full, "complete fingerprint shown"),
                Fingerprint::Truncated => (State::Reduced, "shortened fingerprint shown"),
                Fingerprint::Absent => (State::Absent, "nothing shown"),
                Fingerprint::Unknown => (State::Unknown, "not established"),
            },
        ),
        item(
            "Key storage",
            match keystore.tee {
                Backing::Hardware => (State::Full, "hardware-backed (TEE)"),
                Backing::Software => (State::Reduced, "software only"),
                Backing::Absent => (State::Absent, "no isolated key storage"),
                Backing::Unknown => (State::Unknown, "not established"),
            },
        ),
        item(
            "Secure element",
            yes_no(
                keystore.secure_element,
                "present",
                "none; keys rely on the TEE alone",
            ),
        ),
        item(
            "StrongBox keystore",
            yes_no(keystore.strongbox, "present", "not available"),
        ),
        item(
            "Unlock attempt throttling",
            match (keystore.weaver, keystore.tee) {
                (Answer::Yes, _) => (State::Full, "enforced by the secure element"),
                (Answer::Unknown, _) | (Answer::No, Backing::Unknown) => {
                    (State::Unknown, "not established")
                }
                (Answer::No, Backing::Hardware) => (
                    State::Reduced,
                    "enforced by the TEE, not a secure element; use a passphrase",
                ),
                (Answer::No, Backing::Software | Backing::Absent) => (
                    State::Absent,
                    "enforced by software only; use a long passphrase",
                ),
            },
        ),
        item(
            "Radio isolation (IOMMU)",
            match descriptor.isolation.radio_iommu {
                Iommu::Audited => (State::Full, "confirmed by audit"),
                Iommu::Unconfirmed => (State::Unknown, "believed present, not audited"),
                Iommu::Absent => (State::Absent, "radios are not confined"),
                Iommu::Unknown => (State::Unknown, "not established"),
            },
        ),
        item(
            "Firmware security updates",
            match descriptor.firmware.security_updates {
                Updates::Timely => (State::Full, "vendor patches arrive promptly"),
                Updates::Lagging => (State::Reduced, "vendor patches arrive late"),
                Updates::Ended => (State::Absent, "vendor no longer patches firmware"),
                Updates::Unknown => (State::Unknown, "not established"),
            },
        ),
        item(
            "Memory tagging (MTE)",
            yes_no(descriptor.memory.mte, "available", "not available"),
        ),
        item(
            "USB data off while locked",
            match descriptor.usb.controller_data_disable {
                Answer::Yes => (State::Full, "port controller and kernel"),
                Answer::No => (State::Reduced, "kernel only"),
                Answer::Unknown => (State::Unknown, "not established"),
            },
        ),
        item(
            "Kernel hardening profile",
            match descriptor.kernel.hardening_profile {
                Profile::Full => (State::Full, "full profile"),
                Profile::Limited => (State::Reduced, "limited by the vendor's kernel"),
                Profile::Unknown => (State::Unknown, "not established"),
            },
        ),
        item(
            "Emergency calling",
            match descriptor.telephony.emergency_call {
                Emergency::Verified => (State::Full, "verified by test"),
                Emergency::Failed => (State::Absent, "does not work"),
                Emergency::Untested => (State::Unknown, "not tested"),
                Emergency::NotApplicable => (State::NotApplicable, "no telephony"),
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A phone that meets every Tier A requirement.
    const TIER_A: &str = r#"
schema = 1
name = "ideal"
kind = "phone"
arch = "arm64"

[boot]
relock_with_custom_key = "yes"
rollback_protection = "yes"
fingerprint_on_boot_screen = "full"
verified_boot_bypass_known = "no"

[keystore]
tee = "hardware"
secure_element = "yes"
strongbox = "yes"
weaver = "yes"

[isolation]
radio_iommu = "audited"

[firmware]
security_updates = "timely"

[memory]
mte = "no"

[usb]
controller_data_disable = "yes"

[kernel]
hardening_profile = "full"

[telephony]
emergency_call = "verified"
"#;

    fn with(changes: &[(&str, &str)]) -> String {
        let mut text = TIER_A.to_owned();
        for (from, to) in changes {
            assert!(text.contains(from), "base descriptor must contain {from:?}");
            text = text.replacen(from, to, 1);
        }
        text
    }

    fn assess_text(text: &str) -> Assessment {
        assess(&Descriptor::from_toml(text).expect("descriptor parses"))
    }

    fn state_of(assessment: &Assessment, mechanism: &str) -> State {
        assessment
            .posture
            .iter()
            .find(|item| item.mechanism == mechanism)
            .unwrap_or_else(|| panic!("no posture item {mechanism:?}"))
            .state
    }

    #[test]
    fn full_hardware_reaches_tier_a_without_mte() {
        let assessment = assess_text(TIER_A);
        assert_eq!(assessment.tier, Tier::A);
        assert!(assessment.missing_for_tier_a.is_empty());
        assert!(assessment.exclusions.is_empty());
        assert!(assessment.open_items.is_empty());
        // MTE is "ideally": absent, yet still Tier A.
        assert_eq!(state_of(&assessment, "Memory tagging (MTE)"), State::Absent);
        assert_eq!(assessment.posture.len(), 13);
    }

    #[test]
    fn each_tier_a_requirement_is_necessary() {
        let cases = [
            (
                (
                    "rollback_protection = \"yes\"",
                    "rollback_protection = \"no\"",
                ),
                "rollback protection",
            ),
            (
                ("tee = \"hardware\"", "tee = \"software\""),
                "hardware-backed keystore",
            ),
            (
                ("weaver = \"yes\"", "weaver = \"no\""),
                "secure element with StrongBox and unlock throttling",
            ),
            (
                ("strongbox = \"yes\"", "strongbox = \"unknown\""),
                "secure element with StrongBox and unlock throttling",
            ),
            (
                ("radio_iommu = \"audited\"", "radio_iommu = \"unconfirmed\""),
                "IOMMU isolation of radios, confirmed by audit",
            ),
            (
                (
                    "security_updates = \"timely\"",
                    "security_updates = \"lagging\"",
                ),
                "timely firmware security updates",
            ),
        ];
        for (change, missing) in cases {
            let assessment = assess_text(&with(&[change]));
            assert_eq!(assessment.tier, Tier::BVerified, "change {change:?}");
            assert_eq!(assessment.missing_for_tier_a, vec![missing]);
        }
    }

    #[test]
    fn no_secure_element_is_tier_b_verified() {
        let assessment = assess_text(&with(&[
            ("secure_element = \"yes\"", "secure_element = \"no\""),
            ("strongbox = \"yes\"", "strongbox = \"no\""),
            ("weaver = \"yes\"", "weaver = \"no\""),
        ]));
        assert_eq!(assessment.tier, Tier::BVerified);
        assert_eq!(state_of(&assessment, "Secure element"), State::Absent);
        assert_eq!(
            state_of(&assessment, "Unlock attempt throttling"),
            State::Reduced
        );
    }

    #[test]
    fn unlocked_bootloader_is_tier_b_unverified() {
        for answer in ["no", "unknown"] {
            let assessment = assess_text(&with(&[(
                "relock_with_custom_key = \"yes\"",
                &format!("relock_with_custom_key = \"{answer}\""),
            )]));
            assert_eq!(assessment.tier, Tier::BUnverified, "answer {answer}");
            assert!(assessment
                .missing_for_tier_a
                .contains(&"bootloader that relocks with a custom key"));
        }
    }

    #[test]
    fn unknown_is_never_treated_as_yes() {
        let assessment = assess_text(&with(&[
            (
                "rollback_protection = \"yes\"",
                "rollback_protection = \"unknown\"",
            ),
            ("radio_iommu = \"audited\"", "radio_iommu = \"unknown\""),
            (
                "emergency_call = \"verified\"",
                "emergency_call = \"untested\"",
            ),
            (
                "verified_boot_bypass_known = \"no\"",
                "verified_boot_bypass_known = \"unknown\"",
            ),
        ]));
        assert_eq!(assessment.tier, Tier::BVerified);
        assert_eq!(assessment.missing_for_tier_a.len(), 2);
        assert_eq!(
            assessment.open_items,
            vec![
                "check for known verified-boot bypasses",
                "verify emergency calling",
                "complete the device audit: some mechanisms are unknown",
            ]
        );
    }

    #[test]
    fn hard_exclusions() {
        let bypass = assess_text(&with(&[(
            "verified_boot_bypass_known = \"no\"",
            "verified_boot_bypass_known = \"yes\"",
        )]));
        assert_eq!(bypass.tier, Tier::Excluded);
        assert_eq!(
            bypass.exclusions,
            vec!["a public, unpatchable bypass of verified boot exists"]
        );

        let emergency = assess_text(&with(&[(
            "emergency_call = \"verified\"",
            "emergency_call = \"failed\"",
        )]));
        assert_eq!(emergency.tier, Tier::Excluded);
        assert_eq!(
            emergency.exclusions,
            vec!["emergency calling does not work"]
        );
        assert_eq!(state_of(&emergency, "Emergency calling"), State::Absent);
    }

    #[test]
    fn emulator_makes_no_hardware_claim() {
        let assessment = assess_text(&with(&[("kind = \"phone\"", "kind = \"emulator\"")]));
        assert_eq!(assessment.tier, Tier::Emulator);
        assert!(assessment.missing_for_tier_a.is_empty());
    }

    #[test]
    fn tablet_without_telephony() {
        let assessment = assess_text(&with(&[
            ("kind = \"phone\"", "kind = \"tablet\""),
            (
                "emergency_call = \"verified\"",
                "emergency_call = \"not_applicable\"",
            ),
        ]));
        assert_eq!(assessment.tier, Tier::A);
        assert_eq!(
            state_of(&assessment, "Emergency calling"),
            State::NotApplicable
        );
    }

    #[test]
    fn software_only_throttling_is_reported_absent() {
        let assessment = assess_text(&with(&[
            ("tee = \"hardware\"", "tee = \"software\""),
            ("secure_element = \"yes\"", "secure_element = \"no\""),
            ("strongbox = \"yes\"", "strongbox = \"no\""),
            ("weaver = \"yes\"", "weaver = \"no\""),
        ]));
        assert_eq!(
            state_of(&assessment, "Unlock attempt throttling"),
            State::Absent
        );
        assert_eq!(state_of(&assessment, "Key storage"), State::Reduced);
    }

    #[test]
    fn rejects_invalid_descriptors() {
        let syntax = |text: String| {
            matches!(
                Descriptor::from_toml(&text),
                Err(DescriptorError::Syntax(_))
            )
        };
        assert!(syntax(with(&[("mte = \"no\"", "mte = \"maybe\"")])));
        assert!(syntax(with(&[("mte = \"no\"", "mte = \"no\"\nextra = 1")])));
        assert!(syntax(with(&[("[memory]\nmte = \"no\"\n", "")])));
        assert!(syntax(with(&[("arch = \"arm64\"", "arch = \"arm\"")])));
        assert!(syntax("not toml at all [".to_owned()));

        assert_eq!(
            Descriptor::from_toml(&with(&[("schema = 1", "schema = 2")])),
            Err(DescriptorError::UnsupportedSchema(2))
        );
        assert_eq!(
            Descriptor::from_toml(&with(&[("name = \"ideal\"", "name = \"\"")])),
            Err(DescriptorError::Inconsistent("name is empty"))
        );
        assert_eq!(
            Descriptor::from_toml(&with(&[(
                "emergency_call = \"verified\"",
                "emergency_call = \"not_applicable\""
            )])),
            Err(DescriptorError::Inconsistent(
                "a phone cannot declare emergency calling not applicable"
            ))
        );
        assert_eq!(
            Descriptor::from_toml(&with(&[(
                "secure_element = \"yes\"",
                "secure_element = \"no\""
            )])),
            Err(DescriptorError::Inconsistent(
                "strongbox and weaver require a secure element"
            ))
        );
    }
}
