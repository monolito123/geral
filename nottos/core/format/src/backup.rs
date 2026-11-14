// SPDX-License-Identifier: Apache-2.0

//! Formats of the backup system (`docs/adr/0013-backup-format.md`).
//!
//! # Backup header, format version 1
//!
//! ```text
//! nottos-backup 1
//! suite: x25519+ml-kem-1024+chacha20-poly1305
//! created: 2026-11-14T00:00:00Z
//! kdf: argon2id <memory KiB> <iterations> <lanes> <salt, 32 hex digits>
//! recovery-key-id: <64 hex digits>
//! kem-ciphertext: <3,200 hex digits>
//! wrapped-key: <96 hex digits>
//! ```
//!
//! The encrypted payload follows the last line feed directly. The wrapped
//! key authenticates every line before its own.
//!
//! # Backup key, format version 1
//!
//! The device keeps this; it can make backups but not read them.
//!
//! ```text
//! nottos-backup-key 1
//! suite: x25519+ml-kem-1024+chacha20-poly1305
//! kdf: argon2id <memory KiB> <iterations> <lanes> <salt, 32 hex digits>
//! public-key: <3,200 hex digits>
//! authentication-key: <64 hex digits>
//! ```
//!
//! Both formats follow the rules of [`crate::text`]: one canonical byte
//! form. Neither checks cryptographic validity or cost limits; the backup
//! component does.

use crate::digest::Sha256Digest;
use crate::hex;
use crate::text::{parse_decimal, Lines, Problem};
use crate::time::Timestamp;
use std::fmt;
use zeroize::Zeroizing;

/// The only suite of format version 1: hybrid key encapsulation (ADR-0012)
/// and ChaCha20-Poly1305.
pub const SUITE: &str = "x25519+ml-kem-1024+chacha20-poly1305";
/// The only key-derivation function of format version 1.
pub const KDF_NAME: &str = "argon2id";
/// Length of the Argon2id salt.
pub const SALT_LEN: usize = 16;
/// Length of the recovery public key: X25519 and ML-KEM-1024.
pub const PUBLIC_KEY_LEN: usize = 1600;
/// Length of the key-encapsulation ciphertext.
pub const CIPHERTEXT_LEN: usize = 1600;
/// Length of the data key that encrypts the payload.
pub const DATA_KEY_LEN: usize = 32;
/// Length of the wrapped data key: the key and a 16-byte tag.
pub const WRAPPED_KEY_LEN: usize = DATA_KEY_LEN + 16;
/// Length of the authentication key.
pub const AUTHENTICATION_KEY_LEN: usize = 32;
/// Number of lines in a backup header.
pub const HEADER_LINES: usize = 7;
/// Upper bound on the size of a backup header.
pub const MAX_HEADER_BYTES: usize = 4096;

const HEADER_MAGIC: &str = "nottos-backup";
const KEY_MAGIC: &str = "nottos-backup-key";
const FORMAT_VERSION: u32 = 1;
const MAX_KEY_BYTES: usize = 4096;

const FIELD_SUITE: &str = "suite: ";
const FIELD_CREATED: &str = "created: ";
const FIELD_KDF: &str = "kdf: ";
const FIELD_RECOVERY_KEY_ID: &str = "recovery-key-id: ";
const FIELD_KEM_CIPHERTEXT: &str = "kem-ciphertext: ";
const FIELD_WRAPPED_KEY: &str = "wrapped-key: ";
const FIELD_PUBLIC_KEY: &str = "public-key: ";
const FIELD_AUTHENTICATION_KEY: &str = "authentication-key: ";

/// Why a backup header or backup key was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupFormatError {
    /// Not in canonical form up to the named point.
    Envelope(Problem),
    /// A field value is malformed.
    BadValue {
        /// One-based line number.
        line: usize,
        /// Which field.
        field: &'static str,
    },
}

impl fmt::Display for BackupFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Envelope(problem) => problem.fmt(f),
            Self::BadValue { line, field } => write!(f, "line {line}: invalid {field}"),
        }
    }
}

impl std::error::Error for BackupFormatError {}

impl From<Problem> for BackupFormatError {
    fn from(problem: Problem) -> Self {
        Self::Envelope(problem)
    }
}

/// How the recovery keys are derived from the recovery secret: Argon2id
/// parameters and salt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kdf {
    /// Memory, in KiB.
    pub memory_kib: u32,
    /// Passes over the memory.
    pub iterations: u32,
    /// Lanes.
    pub lanes: u32,
    /// Salt, chosen at random when the backup key is made.
    pub salt: [u8; SALT_LEN],
}

impl Kdf {
    fn parse(value: &str) -> Option<Self> {
        let mut parts = value.split(' ');
        let (Some(KDF_NAME), Some(memory), Some(iterations), Some(lanes), Some(salt), None) = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) else {
            return None;
        };
        let number = |text| parse_decimal(text).and_then(|value| u32::try_from(value).ok());
        Some(Self {
            memory_kib: number(memory)?,
            iterations: number(iterations)?,
            lanes: number(lanes)?,
            salt: hex::decode(salt)?,
        })
    }
}

impl fmt::Display for Kdf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{KDF_NAME} {} {} {} {}",
            self.memory_kib,
            self.iterations,
            self.lanes,
            hex::encode(&self.salt)
        )
    }
}

/// Every field of a backup header except the wrapped key, which
/// authenticates them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderFields {
    /// When the backup was made, by the clock of the device that made it.
    pub created: Timestamp,
    /// How to derive the recovery keys from the recovery secret.
    pub kdf: Kdf,
    /// SHA-256 of the recovery public key.
    pub recovery_key_id: Sha256Digest,
    /// The key-encapsulation ciphertext.
    pub kem_ciphertext: Box<[u8; CIPHERTEXT_LEN]>,
}

impl HeaderFields {
    /// The header up to and including the `kem-ciphertext` line: exactly
    /// the bytes the wrapped key authenticates.
    pub fn authenticated_text(&self) -> String {
        format!(
            "{HEADER_MAGIC} {FORMAT_VERSION}\n\
             {FIELD_SUITE}{SUITE}\n\
             {FIELD_CREATED}{}\n\
             {FIELD_KDF}{}\n\
             {FIELD_RECOVERY_KEY_ID}{}\n\
             {FIELD_KEM_CIPHERTEXT}{}\n",
            self.created,
            self.kdf,
            self.recovery_key_id,
            hex::encode(&self.kem_ciphertext[..]),
        )
    }
}

/// The header of a backup file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupHeader {
    fields: HeaderFields,
    wrapped_key: [u8; WRAPPED_KEY_LEN],
}

impl BackupHeader {
    /// Joins the fields and the wrapped key.
    pub fn new(fields: HeaderFields, wrapped_key: [u8; WRAPPED_KEY_LEN]) -> Self {
        Self {
            fields,
            wrapped_key,
        }
    }

    /// Parses a header in canonical form. Authenticity is not checked here.
    pub fn parse(text: &str) -> Result<Self, BackupFormatError> {
        let mut lines = Lines::open(text, MAX_HEADER_BYTES, HEADER_MAGIC, FORMAT_VERSION)?;
        parse_suite(&mut lines)?;
        let created = lines.field(FIELD_CREATED)?;
        let created = Timestamp::parse(created).ok_or(bad(&lines, "created"))?;
        let kdf = parse_kdf(&mut lines)?;
        let recovery_key_id = lines.field(FIELD_RECOVERY_KEY_ID)?;
        let recovery_key_id =
            Sha256Digest::from_hex(recovery_key_id).ok_or(bad(&lines, "recovery-key-id"))?;
        let kem_ciphertext = lines.field(FIELD_KEM_CIPHERTEXT)?;
        let kem_ciphertext =
            hex::decode_boxed(kem_ciphertext).ok_or(bad(&lines, "kem-ciphertext"))?;
        let wrapped_key = lines.field(FIELD_WRAPPED_KEY)?;
        let wrapped_key = hex::decode(wrapped_key).ok_or(bad(&lines, "wrapped-key"))?;
        lines.finish()?;
        Ok(Self {
            fields: HeaderFields {
                created,
                kdf,
                recovery_key_id,
                kem_ciphertext,
            },
            wrapped_key,
        })
    }

    /// Every field but the wrapped key.
    pub fn fields(&self) -> &HeaderFields {
        &self.fields
    }

    /// The data key, encrypted and authenticated.
    pub fn wrapped_key(&self) -> &[u8; WRAPPED_KEY_LEN] {
        &self.wrapped_key
    }
}

impl fmt::Display for BackupHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.fields.authenticated_text())?;
        writeln!(f, "{FIELD_WRAPPED_KEY}{}", hex::encode(&self.wrapped_key))
    }
}

/// What the device keeps to make backups: the recovery public key and the
/// authentication key. It is secret, because the authentication key is.
#[derive(Clone, PartialEq, Eq)]
pub struct KeyFile {
    /// How the recovery keys were derived.
    pub kdf: Kdf,
    /// The recovery public key.
    pub public_key: Box<[u8; PUBLIC_KEY_LEN]>,
    /// The key that makes a backup authentic.
    pub authentication_key: Zeroizing<[u8; AUTHENTICATION_KEY_LEN]>,
}

impl KeyFile {
    /// Parses a backup key in canonical form.
    pub fn parse(text: &str) -> Result<Self, BackupFormatError> {
        let mut lines = Lines::open(text, MAX_KEY_BYTES, KEY_MAGIC, FORMAT_VERSION)?;
        parse_suite(&mut lines)?;
        let kdf = parse_kdf(&mut lines)?;
        let public_key = lines.field(FIELD_PUBLIC_KEY)?;
        let public_key = hex::decode_boxed(public_key).ok_or(bad(&lines, "public-key"))?;
        let authentication_key = lines.field(FIELD_AUTHENTICATION_KEY)?;
        let mut key = Zeroizing::new([0u8; AUTHENTICATION_KEY_LEN]);
        if !hex::decode_into(authentication_key, &mut key[..]) {
            return Err(bad(&lines, "authentication-key"));
        }
        lines.finish()?;
        Ok(Self {
            kdf,
            public_key,
            authentication_key: key,
        })
    }

    /// The canonical text form. It contains the authentication key and is
    /// wiped when dropped.
    pub fn to_text(&self) -> Zeroizing<String> {
        Zeroizing::new(format!(
            "{KEY_MAGIC} {FORMAT_VERSION}\n\
             {FIELD_SUITE}{SUITE}\n\
             {FIELD_KDF}{}\n\
             {FIELD_PUBLIC_KEY}{}\n\
             {FIELD_AUTHENTICATION_KEY}{}\n",
            self.kdf,
            hex::encode(&self.public_key[..]),
            hex::encode(&self.authentication_key[..]),
        ))
    }
}

impl fmt::Debug for KeyFile {
    /// Never prints the authentication key.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KeyFile({}, <redacted>)", self.kdf)
    }
}

fn bad(lines: &Lines<'_>, field: &'static str) -> BackupFormatError {
    BackupFormatError::BadValue {
        line: lines.number(),
        field,
    }
}

fn parse_suite(lines: &mut Lines<'_>) -> Result<(), BackupFormatError> {
    if lines.field(FIELD_SUITE)? != SUITE {
        return Err(bad(lines, "suite"));
    }
    Ok(())
}

fn parse_kdf(lines: &mut Lines<'_>) -> Result<Kdf, BackupFormatError> {
    let kdf = lines.field(FIELD_KDF)?;
    Kdf::parse(kdf).ok_or(bad(lines, "kdf"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_kdf() -> Kdf {
        Kdf {
            memory_kib: 65_536,
            iterations: 3,
            lanes: 4,
            salt: [0x5a; SALT_LEN],
        }
    }

    fn sample_header() -> BackupHeader {
        let mut ciphertext = Box::new([0u8; CIPHERTEXT_LEN]);
        for (index, byte) in ciphertext.iter_mut().enumerate() {
            *byte = (index * 7) as u8;
        }
        BackupHeader::new(
            HeaderFields {
                created: Timestamp::parse("2026-11-14T12:34:56Z").unwrap(),
                kdf: sample_kdf(),
                recovery_key_id: Sha256Digest::of_bytes(b"recovery key"),
                kem_ciphertext: ciphertext,
            },
            [0xa5; WRAPPED_KEY_LEN],
        )
    }

    fn sample_key() -> KeyFile {
        KeyFile {
            kdf: sample_kdf(),
            public_key: Box::new([0x11; PUBLIC_KEY_LEN]),
            authentication_key: Zeroizing::new([0x22; AUTHENTICATION_KEY_LEN]),
        }
    }

    #[test]
    fn header_round_trips() {
        let header = sample_header();
        let text = header.to_string();
        assert_eq!(BackupHeader::parse(&text).unwrap(), header);
        assert_eq!(text.lines().count(), HEADER_LINES);
        assert!(text.len() <= MAX_HEADER_BYTES);
        assert!(text.starts_with(&header.fields().authenticated_text()));
    }

    #[test]
    fn header_layout_is_the_documented_one() {
        let text = sample_header().to_string();
        let prefixes: Vec<&str> = text
            .lines()
            .map(|line| line.split(' ').next().unwrap())
            .collect();
        assert_eq!(
            prefixes,
            [
                "nottos-backup",
                "suite:",
                "created:",
                "kdf:",
                "recovery-key-id:",
                "kem-ciphertext:",
                "wrapped-key:",
            ]
        );
        assert!(text.contains("\nkdf: argon2id 65536 3 4 5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a\n"));
    }

    #[test]
    fn key_round_trips_and_hides_its_secret() {
        let key = sample_key();
        let text = key.to_text();
        assert_eq!(KeyFile::parse(&text).unwrap(), key);
        assert!(!format!("{key:?}").contains("2222"));
    }

    #[test]
    fn every_single_byte_change_to_a_header_is_refused_or_changes_it() {
        // One canonical form: any edit either fails to parse or parses to a
        // different header.
        let header = sample_header();
        let text = header.to_string();
        let replacements = [b'0', b'a', b'f', b'g', b'A', b' ', b'\n', 0x7f];
        for position in 0..text.len() {
            for &replacement in &replacements {
                let mut bytes = text.clone().into_bytes();
                if bytes[position] == replacement {
                    continue;
                }
                bytes[position] = replacement;
                let changed = String::from_utf8(bytes).unwrap();
                if let Ok(parsed) = BackupHeader::parse(&changed) {
                    assert_ne!(parsed, header, "position {position}");
                    assert_eq!(parsed.to_string(), changed, "position {position}");
                }
            }
        }
    }

    #[test]
    fn refuses_what_is_not_canonical() {
        let text = sample_header().to_string();
        let cases = [
            (
                "suite: x25519+ml-kem-1024+chacha20-poly1305",
                "suite: other",
            ),
            ("argon2id 65536", "scrypt 65536"),
            ("argon2id 65536", "argon2id 065536"),
            (" 3 4 ", " 3 4  "),
            (
                "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
                "5A5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
            ),
            (
                "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
                "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
            ),
            ("2026-11-14T12:34:56Z", "2026-11-14T12:34:56+00:00"),
        ];
        for (from, to) in cases {
            assert!(text.contains(from), "{from}");
            let changed = text.replacen(from, to, 1);
            assert!(BackupHeader::parse(&changed).is_err(), "{to}");
        }
        assert!(
            BackupHeader::parse(&text.replacen("nottos-backup 1", "nottos-backup 2", 1))
                .is_err_and(|error| error
                    == BackupFormatError::Envelope(Problem::UnsupportedVersion("2".to_owned())))
        );
        let too_large_memory = text.replacen("argon2id 65536", "argon2id 4294967296", 1);
        assert_eq!(
            BackupHeader::parse(&too_large_memory),
            Err(BackupFormatError::BadValue {
                line: 4,
                field: "kdf"
            })
        );
        let extra_line = format!("{text}x\n");
        assert!(BackupHeader::parse(&extra_line).is_err());
        let key = sample_key().to_text();
        assert!(BackupHeader::parse(&key).is_err());
        assert!(KeyFile::parse(&text).is_err());
    }
}
