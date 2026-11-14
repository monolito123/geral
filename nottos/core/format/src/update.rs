// SPDX-License-Identifier: Apache-2.0

//! Metadata of the update system (`docs/adr/0009-update-system.md`).
//!
//! Two documents, both signed by a threshold of keys:
//!
//! - **Root metadata** says which keys may sign root metadata and update
//!   metadata, and how many of them must. It is signed by the root keys.
//! - **Update metadata** says which build is current on a channel. It is
//!   signed by the release keys the root names.
//!
//! # Root metadata, format version 1
//!
//! ```text
//! nottos-root 1
//! version: 1
//! expires: 2027-11-13T00:00:00Z
//! root-threshold: 2
//! root-key: <suite name> <ed25519 hex> <post-quantum hex>
//! ...
//! release-threshold: 2
//! release-key: <suite name> <ed25519 hex> <post-quantum hex>
//! ...
//! ```
//!
//! # Update metadata, format version 1
//!
//! ```text
//! nottos-update 1
//! channel: stable
//! target: mini_x86_64
//! sequence: 7
//! issued: 2026-11-13T00:00:00Z
//! expires: 2026-12-13T00:00:00Z
//! manifest-sha256: <64 hex digits>
//! manifest-size: 1234
//! ```
//!
//! Keys in a root are listed in strictly ascending order of key-id. Both
//! formats follow the rules of [`crate::text`]: one canonical byte form.

use crate::digest::Sha256Digest;
use crate::hex;
use crate::signature::{PublicKey, Suite};
use crate::text::{parse_decimal, Lines, Problem};
use crate::time::Timestamp;
use std::fmt;

/// Most keys a role may have.
pub const MAX_ROLE_KEYS: usize = 16;

const ROOT_MAGIC: &str = "nottos-root";
const UPDATE_MAGIC: &str = "nottos-update";
const FORMAT_VERSION: u32 = 1;
const MAX_ROOT_BYTES: usize = 256 * 1024;
const MAX_UPDATE_BYTES: usize = 4 * 1024;
const MAX_TOKEN_LEN: usize = 64;

/// Why a document was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateFormatError {
    /// Not in canonical form up to the named point.
    Envelope(Problem),
    /// A field value is malformed or out of range.
    BadValue {
        /// One-based line number, or `None` for a value passed to a
        /// constructor.
        line: Option<usize>,
        /// Which field.
        field: &'static str,
    },
}

impl fmt::Display for UpdateFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Envelope(Problem::TooLarge) => f.write_str("file is too large"),
            Self::Envelope(Problem::InvalidCharacter { line }) => {
                write!(f, "line {line}: character outside printable ASCII")
            }
            Self::Envelope(Problem::MissingFinalNewline) => {
                f.write_str("file does not end with a line feed")
            }
            Self::Envelope(Problem::BadHeader) => f.write_str("unrecognised first line"),
            Self::Envelope(Problem::UnsupportedVersion(version)) => {
                write!(f, "unsupported format version {version}")
            }
            Self::Envelope(Problem::UnexpectedLine { line, expected }) => {
                write!(f, "line {line}: expected `{}`", expected.trim_end())
            }
            Self::BadValue {
                line: Some(line),
                field,
            } => write!(f, "line {line}: invalid {field}"),
            Self::BadValue { line: None, field } => write!(f, "invalid {field}"),
        }
    }
}

impl std::error::Error for UpdateFormatError {}

impl From<Problem> for UpdateFormatError {
    fn from(problem: Problem) -> Self {
        Self::Envelope(problem)
    }
}

fn bad(line: Option<usize>, field: &'static str) -> UpdateFormatError {
    UpdateFormatError::BadValue { line, field }
}

/// A set of keys and how many of them must sign.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Role {
    threshold: usize,
    keys: Vec<PublicKey>,
}

impl Role {
    /// Builds a role. Keys are put in ascending order of key-id; the same
    /// key twice, no key, more than [`MAX_ROLE_KEYS`] keys, or a threshold
    /// outside `1..=keys` is an error.
    pub fn new(threshold: usize, mut keys: Vec<PublicKey>) -> Result<Self, UpdateFormatError> {
        keys.sort_by_key(PublicKey::key_id);
        Self::checked(threshold, keys, None, "role")
    }

    fn checked(
        threshold: usize,
        keys: Vec<PublicKey>,
        line: Option<usize>,
        field: &'static str,
    ) -> Result<Self, UpdateFormatError> {
        let ascending = keys
            .windows(2)
            .all(|pair| pair[0].key_id() < pair[1].key_id());
        if keys.is_empty() || keys.len() > MAX_ROLE_KEYS || !ascending {
            return Err(bad(line, field));
        }
        if threshold == 0 || threshold > keys.len() {
            return Err(bad(line, field));
        }
        Ok(Self { threshold, keys })
    }

    /// How many distinct keys must sign.
    pub fn threshold(&self) -> usize {
        self.threshold
    }

    /// The keys, in ascending order of key-id.
    pub fn keys(&self) -> &[PublicKey] {
        &self.keys
    }

    /// Looks a key up by its key-id.
    pub fn key(&self, key_id: Sha256Digest) -> Option<&PublicKey> {
        self.keys
            .binary_search_by(|key| key.key_id().cmp(&key_id))
            .ok()
            .map(|index| &self.keys[index])
    }
}

/// Root metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    version: u64,
    expires: Timestamp,
    root: Role,
    release: Role,
}

const FIELD_VERSION: &str = "version: ";
const FIELD_EXPIRES: &str = "expires: ";
const FIELD_ROOT_THRESHOLD: &str = "root-threshold: ";
const FIELD_ROOT_KEY: &str = "root-key: ";
const FIELD_RELEASE_THRESHOLD: &str = "release-threshold: ";
const FIELD_RELEASE_KEY: &str = "release-key: ";

impl Root {
    /// Builds root metadata. `version` starts at 1.
    pub fn new(
        version: u64,
        expires: Timestamp,
        root: Role,
        release: Role,
    ) -> Result<Self, UpdateFormatError> {
        if version == 0 {
            return Err(bad(None, "version"));
        }
        Ok(Self {
            version,
            expires,
            root,
            release,
        })
    }

    /// Parses the canonical text form.
    pub fn parse(text: &str) -> Result<Self, UpdateFormatError> {
        let mut lines = Lines::open(text, MAX_ROOT_BYTES, ROOT_MAGIC, FORMAT_VERSION)?;
        let version = parse_number(&mut lines, FIELD_VERSION, "version")?;
        if version == 0 {
            return Err(bad(Some(lines.number()), "version"));
        }
        let expires = parse_time(&mut lines, FIELD_EXPIRES, "expires")?;
        let root = parse_role(
            &mut lines,
            FIELD_ROOT_THRESHOLD,
            FIELD_ROOT_KEY,
            FIELD_RELEASE_THRESHOLD,
            "root role",
        )?;
        let release = parse_role(
            &mut lines,
            FIELD_RELEASE_THRESHOLD,
            FIELD_RELEASE_KEY,
            "",
            "release role",
        )?;
        lines.finish()?;
        Ok(Self {
            version,
            expires,
            root,
            release,
        })
    }

    /// Increases by exactly one with every new root.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// After this time the root is no longer trusted.
    pub fn expires(&self) -> Timestamp {
        self.expires
    }

    /// The keys that sign root metadata.
    pub fn root_role(&self) -> &Role {
        &self.root
    }

    /// The keys that sign update metadata.
    pub fn release_role(&self) -> &Role {
        &self.release
    }
}

impl fmt::Display for Root {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{ROOT_MAGIC} {FORMAT_VERSION}")?;
        writeln!(f, "{FIELD_VERSION}{}", self.version)?;
        writeln!(f, "{FIELD_EXPIRES}{}", self.expires)?;
        write_role(f, FIELD_ROOT_THRESHOLD, FIELD_ROOT_KEY, &self.root)?;
        write_role(f, FIELD_RELEASE_THRESHOLD, FIELD_RELEASE_KEY, &self.release)
    }
}

fn write_role(
    f: &mut fmt::Formatter<'_>,
    threshold_field: &str,
    key_field: &str,
    role: &Role,
) -> fmt::Result {
    writeln!(f, "{threshold_field}{}", role.threshold)?;
    for key in &role.keys {
        writeln!(
            f,
            "{key_field}{} {} {}",
            key.suite().name(),
            hex::encode(key.ed25519()),
            hex::encode(key.post_quantum())
        )?;
    }
    Ok(())
}

/// Reads `<threshold field>` and then key lines until the line that starts
/// with `next_field` (or the end of the file when `next_field` is empty).
fn parse_role(
    lines: &mut Lines<'_>,
    threshold_field: &'static str,
    key_field: &'static str,
    next_field: &'static str,
    label: &'static str,
) -> Result<Role, UpdateFormatError> {
    let threshold = parse_number(lines, threshold_field, label)?;
    let threshold_line = lines.number();
    let mut keys: Vec<PublicKey> = Vec::new();
    loop {
        let more = if next_field.is_empty() {
            lines.has_more()
        } else {
            lines.has_more() && !lines.peek_starts_with(next_field)
        };
        if !more {
            break;
        }
        if keys.len() == MAX_ROLE_KEYS {
            return Err(bad(Some(lines.number() + 1), label));
        }
        let value = lines.field(key_field)?;
        let line = Some(lines.number());
        let key = parse_key_line(value).ok_or(bad(line, label))?;
        if keys
            .last()
            .is_some_and(|last| last.key_id() >= key.key_id())
        {
            return Err(bad(line, label));
        }
        keys.push(key);
    }
    let threshold = usize::try_from(threshold).map_err(|_| bad(Some(threshold_line), label))?;
    Role::checked(threshold, keys, Some(threshold_line), label)
}

fn parse_key_line(value: &str) -> Option<PublicKey> {
    let mut parts = value.split(' ');
    let (Some(suite), Some(ed25519), Some(post_quantum), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let suite = Suite::from_name(suite)?;
    let ed25519 = hex::decode(ed25519)?;
    let mut post_quantum_bytes = vec![0u8; suite.post_quantum_public_key_len()];
    if !hex::decode_into(post_quantum, &mut post_quantum_bytes) {
        return None;
    }
    PublicKey::new(suite, ed25519, post_quantum_bytes).ok()
}

/// Update metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateMetadata {
    channel: String,
    target: String,
    sequence: u64,
    issued: Timestamp,
    expires: Timestamp,
    manifest_sha256: Sha256Digest,
    manifest_size: u64,
}

const FIELD_CHANNEL: &str = "channel: ";
const FIELD_TARGET: &str = "target: ";
const FIELD_SEQUENCE: &str = "sequence: ";
const FIELD_ISSUED: &str = "issued: ";
const FIELD_MANIFEST_SHA256: &str = "manifest-sha256: ";
const FIELD_MANIFEST_SIZE: &str = "manifest-size: ";

/// What a build of update metadata says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateFields<'a> {
    /// Release channel, for example `stable`.
    pub channel: &'a str,
    /// Build target.
    pub target: &'a str,
    /// Release sequence number, starting at 1.
    pub sequence: u64,
    /// When the metadata was signed.
    pub issued: Timestamp,
    /// After this time the metadata is stale.
    pub expires: Timestamp,
    /// Digest of the build manifest.
    pub manifest_sha256: Sha256Digest,
    /// Size of the build manifest in bytes.
    pub manifest_size: u64,
}

impl UpdateMetadata {
    /// Builds update metadata. Channel and target are 1 to 64 characters
    /// from `a-z 0-9 _ . -`; `sequence` starts at 1; `issued` must not be
    /// after `expires`.
    pub fn new(fields: UpdateFields<'_>) -> Result<Self, UpdateFormatError> {
        if !is_name(fields.channel) {
            return Err(bad(None, "channel"));
        }
        if !is_name(fields.target) {
            return Err(bad(None, "target"));
        }
        if fields.sequence == 0 {
            return Err(bad(None, "sequence"));
        }
        if fields.issued > fields.expires {
            return Err(bad(None, "expires"));
        }
        Ok(Self {
            channel: fields.channel.to_owned(),
            target: fields.target.to_owned(),
            sequence: fields.sequence,
            issued: fields.issued,
            expires: fields.expires,
            manifest_sha256: fields.manifest_sha256,
            manifest_size: fields.manifest_size,
        })
    }

    /// Parses the canonical text form.
    pub fn parse(text: &str) -> Result<Self, UpdateFormatError> {
        let mut lines = Lines::open(text, MAX_UPDATE_BYTES, UPDATE_MAGIC, FORMAT_VERSION)?;
        let channel = lines.field(FIELD_CHANNEL)?;
        if !is_name(channel) {
            return Err(bad(Some(lines.number()), "channel"));
        }
        let target = lines.field(FIELD_TARGET)?;
        if !is_name(target) {
            return Err(bad(Some(lines.number()), "target"));
        }
        let sequence = parse_number(&mut lines, FIELD_SEQUENCE, "sequence")?;
        if sequence == 0 {
            return Err(bad(Some(lines.number()), "sequence"));
        }
        let issued = parse_time(&mut lines, FIELD_ISSUED, "issued")?;
        let expires = parse_time(&mut lines, FIELD_EXPIRES, "expires")?;
        if issued > expires {
            return Err(bad(Some(lines.number()), "expires"));
        }
        let digest = lines.field(FIELD_MANIFEST_SHA256)?;
        let manifest_sha256 =
            Sha256Digest::from_hex(digest).ok_or(bad(Some(lines.number()), "manifest-sha256"))?;
        let manifest_size = parse_number(&mut lines, FIELD_MANIFEST_SIZE, "manifest-size")?;
        lines.finish()?;
        Ok(Self {
            channel: channel.to_owned(),
            target: target.to_owned(),
            sequence,
            issued,
            expires,
            manifest_sha256,
            manifest_size,
        })
    }

    /// Release channel.
    pub fn channel(&self) -> &str {
        &self.channel
    }

    /// Build target.
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Release sequence number.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// When the metadata was signed.
    pub fn issued(&self) -> Timestamp {
        self.issued
    }

    /// After this time the metadata is stale.
    pub fn expires(&self) -> Timestamp {
        self.expires
    }

    /// Digest of the build manifest.
    pub fn manifest_sha256(&self) -> Sha256Digest {
        self.manifest_sha256
    }

    /// Size of the build manifest in bytes.
    pub fn manifest_size(&self) -> u64 {
        self.manifest_size
    }
}

impl fmt::Display for UpdateMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{UPDATE_MAGIC} {FORMAT_VERSION}")?;
        writeln!(f, "{FIELD_CHANNEL}{}", self.channel)?;
        writeln!(f, "{FIELD_TARGET}{}", self.target)?;
        writeln!(f, "{FIELD_SEQUENCE}{}", self.sequence)?;
        writeln!(f, "{FIELD_ISSUED}{}", self.issued)?;
        writeln!(f, "{FIELD_EXPIRES}{}", self.expires)?;
        writeln!(f, "{FIELD_MANIFEST_SHA256}{}", self.manifest_sha256)?;
        writeln!(f, "{FIELD_MANIFEST_SIZE}{}", self.manifest_size)
    }
}

fn is_name(text: &str) -> bool {
    (1..=MAX_TOKEN_LEN).contains(&text.len())
        && text.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'.' | b'-')
        })
}

fn parse_number(
    lines: &mut Lines<'_>,
    prefix: &'static str,
    field: &'static str,
) -> Result<u64, UpdateFormatError> {
    let text = lines.field(prefix)?;
    parse_decimal(text).ok_or(bad(Some(lines.number()), field))
}

fn parse_time(
    lines: &mut Lines<'_>,
    prefix: &'static str,
    field: &'static str,
) -> Result<Timestamp, UpdateFormatError> {
    let text = lines.field(prefix)?;
    Timestamp::parse(text).ok_or(bad(Some(lines.number()), field))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(suite: Suite, tag: u8) -> PublicKey {
        PublicKey::new(
            suite,
            [tag; 32],
            vec![tag; suite.post_quantum_public_key_len()],
        )
        .expect("valid key")
    }

    fn time(text: &str) -> Timestamp {
        Timestamp::parse(text).expect("valid time")
    }

    fn sample_root() -> Root {
        let root = Role::new(
            2,
            (1..=3)
                .map(|tag| key(Suite::Ed25519SlhDsaSha2_256s, tag))
                .collect(),
        )
        .expect("valid role");
        let release = Role::new(
            2,
            (4..=6).map(|tag| key(Suite::Ed25519MlDsa87, tag)).collect(),
        )
        .expect("valid role");
        Root::new(1, time("2027-11-13T00:00:00Z"), root, release).expect("valid root")
    }

    fn sample_update() -> UpdateMetadata {
        UpdateMetadata::new(UpdateFields {
            channel: "stable",
            target: "mini_x86_64",
            sequence: 7,
            issued: time("2026-11-13T00:00:00Z"),
            expires: time("2026-12-13T00:00:00Z"),
            manifest_sha256: Sha256Digest::of_bytes(b"manifest"),
            manifest_size: 1234,
        })
        .expect("valid metadata")
    }

    #[test]
    fn root_round_trip() {
        let root = sample_root();
        let text = root.to_string();
        assert!(text.starts_with(
            "nottos-root 1\nversion: 1\nexpires: 2027-11-13T00:00:00Z\nroot-threshold: 2\n\
             root-key: ed25519+slh-dsa-sha2-256s "
        ));
        assert_eq!(text.lines().count(), 1 + 2 + (1 + 3) + (1 + 3));
        let parsed = Root::parse(&text).expect("parses");
        assert_eq!(parsed, root);
        assert_eq!(parsed.to_string(), text);
        assert_eq!(parsed.root_role().threshold(), 2);
        assert_eq!(parsed.release_role().keys().len(), 3);
        let first = &parsed.release_role().keys()[0];
        assert_eq!(parsed.release_role().key(first.key_id()), Some(first));
        assert_eq!(
            parsed.release_role().key(Sha256Digest::of_bytes(b"none")),
            None
        );
    }

    #[test]
    fn role_rules() {
        let keys = |count: u8| -> Vec<PublicKey> {
            (0..count)
                .map(|tag| key(Suite::Ed25519MlDsa87, tag))
                .collect()
        };
        assert!(Role::new(1, keys(1)).is_ok());
        assert!(Role::new(3, keys(3)).is_ok());
        assert!(Role::new(0, keys(3)).is_err());
        assert!(Role::new(4, keys(3)).is_err());
        assert!(Role::new(1, vec![]).is_err());
        assert!(Role::new(1, keys(17)).is_err());
        let mut twice = keys(2);
        twice.push(key(Suite::Ed25519MlDsa87, 0));
        assert!(Role::new(1, twice).is_err());
    }

    #[test]
    fn root_rejects_malformed_text() {
        let text = sample_root().to_string();
        let lines: Vec<&str> = text.lines().collect();
        let rejoin =
            |lines: &[&str]| -> String { lines.iter().map(|line| format!("{line}\n")).collect() };

        // Keys out of order.
        let mut swapped = lines.clone();
        swapped.swap(4, 5);
        assert_eq!(
            Root::parse(&rejoin(&swapped)),
            Err(bad(Some(6), "root role"))
        );
        // A key repeated.
        let mut repeated = lines.clone();
        repeated[5] = lines[4];
        assert_eq!(
            Root::parse(&rejoin(&repeated)),
            Err(bad(Some(6), "root role"))
        );
        // Threshold above the number of keys.
        let high = text.replacen("root-threshold: 2", "root-threshold: 4", 1);
        assert_eq!(Root::parse(&high), Err(bad(Some(4), "root role")));
        let zero = text.replacen("release-threshold: 2", "release-threshold: 0", 1);
        assert_eq!(Root::parse(&zero), Err(bad(Some(8), "release role")));
        // A role without keys.
        let no_keys = rejoin(&[&lines[..4], &lines[7..]].concat());
        assert_eq!(Root::parse(&no_keys), Err(bad(Some(4), "root role")));
        // Version zero, bad time, unknown suite, damaged hex.
        assert_eq!(
            Root::parse(&text.replacen("version: 1", "version: 0", 1)),
            Err(bad(Some(2), "version"))
        );
        assert_eq!(
            Root::parse(&text.replacen("T00:00:00Z", "T00:00:00", 1)),
            Err(bad(Some(3), "expires"))
        );
        assert_eq!(
            Root::parse(&text.replacen("root-key: ed25519+slh-dsa-sha2-256s", "root-key: rsa", 1)),
            Err(bad(Some(5), "root role"))
        );
        assert!(Root::parse(&text.replacen(
            "root-key: ed25519+slh-dsa-sha2-256s 01",
            "root-key: ed25519+slh-dsa-sha2-256s 1",
            1
        ))
        .is_err());
        // Something after the last key.
        assert!(Root::parse(&format!("{text}release-key: x\n")).is_err());
        assert!(Root::parse(&format!("{text}\n")).is_err());
    }

    #[test]
    fn update_round_trip() {
        let update = sample_update();
        let text = update.to_string();
        assert_eq!(
            text,
            format!(
                "nottos-update 1\nchannel: stable\ntarget: mini_x86_64\nsequence: 7\n\
                 issued: 2026-11-13T00:00:00Z\nexpires: 2026-12-13T00:00:00Z\n\
                 manifest-sha256: {}\nmanifest-size: 1234\n",
                Sha256Digest::of_bytes(b"manifest")
            )
        );
        assert_eq!(UpdateMetadata::parse(&text), Ok(update));
    }

    #[test]
    fn update_rejects_malformed_text() {
        let text = sample_update().to_string();
        let cases = [
            (
                text.replacen("stable", "Stable", 1),
                bad(Some(2), "channel"),
            ),
            (
                text.replacen("mini_x86_64", "mini x86", 1),
                bad(Some(3), "target"),
            ),
            (
                text.replacen("sequence: 7", "sequence: 0", 1),
                bad(Some(4), "sequence"),
            ),
            (
                text.replacen("sequence: 7", "sequence: 07", 1),
                bad(Some(4), "sequence"),
            ),
            (
                text.replacen("issued: 2026-11-13", "issued: 2026-12-14", 1),
                bad(Some(6), "expires"),
            ),
            (
                text.replacen("manifest-size: 1234", "manifest-size: -1", 1),
                bad(Some(8), "manifest-size"),
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(UpdateMetadata::parse(&text), Err(expected), "{text}");
        }
        assert!(UpdateMetadata::parse(&format!("{text}extra: 1\n")).is_err());
        let fields = |sequence, channel| UpdateFields {
            channel,
            target: "t",
            sequence,
            issued: Timestamp::EPOCH,
            expires: Timestamp::EPOCH,
            manifest_sha256: Sha256Digest::of_bytes(b""),
            manifest_size: 0,
        };
        assert!(UpdateMetadata::new(fields(1, "stable")).is_ok());
        assert!(UpdateMetadata::new(fields(0, "stable")).is_err());
        assert!(UpdateMetadata::new(fields(1, "")).is_err());
    }

    /// No other byte string parses to the sample metadata.
    #[test]
    fn every_bit_flip_is_rejected_or_changes_the_metadata() {
        let original = sample_update();
        let text = original.to_string();
        let bytes = text.as_bytes();
        for index in 0..bytes.len() {
            for bit in 0..8 {
                let mut mutated = bytes.to_vec();
                mutated[index] ^= 1 << bit;
                let Ok(mutated) = String::from_utf8(mutated) else {
                    continue;
                };
                if let Ok(parsed) = UpdateMetadata::parse(&mutated) {
                    assert_ne!(parsed, original, "byte {index}, bit {bit}");
                    assert_eq!(parsed.to_string(), mutated, "byte {index}, bit {bit}");
                }
            }
        }
    }
}
