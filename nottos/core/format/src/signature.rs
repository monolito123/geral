// SPDX-License-Identifier: Apache-2.0

//! Containers for hybrid public keys and detached signatures.
//!
//! This module only stores and parses bytes. Creating and checking
//! signatures is the job of `nott-crypto`, so that code which merely moves
//! keys and signatures around does not depend on cryptographic libraries.
//!
//! # Public key, format version 1
//!
//! ```text
//! nottos-public-key 1
//! suite: ed25519+ml-dsa-87
//! ed25519: <64 hex digits>
//! ml-dsa-87: <5184 hex digits>
//! ```
//!
//! # Signature, format version 1
//!
//! ```text
//! nottos-signature 1
//! suite: ed25519+ml-dsa-87
//! context: build-manifest
//! key-id: <64 hex digits>
//! ed25519: <128 hex digits>
//! ml-dsa-87: <9254 hex digits>
//! ```
//!
//! `key-id` is the SHA-256 digest of the signer's public key file. `context`
//! names what the signature is for; a signature made for one purpose does
//! not verify for another.
//!
//! Both formats follow the rules of [`crate::text`]: one canonical byte form.
//! The suite name selects the algorithms, so a new algorithm combination is
//! a new suite, not a new interpretation of an old file.

use crate::digest::Sha256Digest;
use crate::hex;
use crate::text::{Lines, Problem};
use std::fmt;

/// Length of an Ed25519 public key in bytes.
pub const ED25519_PUBLIC_KEY_LEN: usize = 32;
/// Length of an Ed25519 signature in bytes.
pub const ED25519_SIGNATURE_LEN: usize = 64;
/// Length of an ML-DSA-87 public key in bytes (FIPS 204).
pub const ML_DSA_87_PUBLIC_KEY_LEN: usize = 2592;
/// Length of an ML-DSA-87 signature in bytes (FIPS 204).
pub const ML_DSA_87_SIGNATURE_LEN: usize = 4627;

/// Longest context name.
pub const MAX_CONTEXT_LEN: usize = 32;

const PUBLIC_KEY_MAGIC: &str = "nottos-public-key";
const SIGNATURE_MAGIC: &str = "nottos-signature";
const FORMAT_VERSION: u32 = 1;
const MAX_PUBLIC_KEY_BYTES: usize = 8 * 1024;
const MAX_SIGNATURE_BYTES: usize = 16 * 1024;

const FIELD_SUITE: &str = "suite: ";
const FIELD_CONTEXT: &str = "context: ";
const FIELD_KEY_ID: &str = "key-id: ";
const FIELD_ED25519: &str = "ed25519: ";
const FIELD_ML_DSA_87: &str = "ml-dsa-87: ";

/// A combination of signature algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suite {
    /// Ed25519 (RFC 8032) together with ML-DSA-87 (FIPS 204). A signature
    /// is valid only if both components are.
    Ed25519MlDsa87,
}

impl Suite {
    /// The name used in files.
    pub fn name(self) -> &'static str {
        match self {
            Self::Ed25519MlDsa87 => "ed25519+ml-dsa-87",
        }
    }

    /// The number bound into every signature of this suite.
    pub fn id(self) -> u16 {
        match self {
            Self::Ed25519MlDsa87 => 1,
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        (name == Self::Ed25519MlDsa87.name()).then_some(Self::Ed25519MlDsa87)
    }
}

/// Why a key or signature file was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// The file is not in canonical form up to the named point.
    Envelope(Problem),
    /// A field value is malformed.
    BadValue {
        /// One-based line number, or `None` for a value passed to a
        /// constructor.
        line: Option<usize>,
        /// Which field.
        field: &'static str,
    },
    /// The suite is not one this version knows.
    UnknownSuite(String),
}

impl fmt::Display for FormatError {
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
            Self::UnknownSuite(name) => write!(f, "unknown signature suite `{name}`"),
        }
    }
}

impl std::error::Error for FormatError {}

impl From<Problem> for FormatError {
    fn from(problem: Problem) -> Self {
        Self::Envelope(problem)
    }
}

/// Whether `text` is a valid context name: 1 to 32 characters from
/// `a-z 0-9 -`.
pub fn is_context(text: &str) -> bool {
    (1..=MAX_CONTEXT_LEN).contains(&text.len())
        && text
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// A hybrid public key.
#[derive(Clone, PartialEq, Eq)]
pub struct PublicKey {
    suite: Suite,
    ed25519: [u8; ED25519_PUBLIC_KEY_LEN],
    ml_dsa_87: Box<[u8; ML_DSA_87_PUBLIC_KEY_LEN]>,
}

impl PublicKey {
    /// Assembles a key of suite [`Suite::Ed25519MlDsa87`] from its two
    /// component keys.
    pub fn new(
        ed25519: [u8; ED25519_PUBLIC_KEY_LEN],
        ml_dsa_87: Box<[u8; ML_DSA_87_PUBLIC_KEY_LEN]>,
    ) -> Self {
        Self {
            suite: Suite::Ed25519MlDsa87,
            ed25519,
            ml_dsa_87,
        }
    }

    /// Parses the canonical text form.
    pub fn parse(text: &str) -> Result<Self, FormatError> {
        let mut lines = Lines::open(text, MAX_PUBLIC_KEY_BYTES, PUBLIC_KEY_MAGIC, FORMAT_VERSION)?;
        let suite = parse_suite(&mut lines)?;
        let ed25519 = parse_hex(&mut lines, FIELD_ED25519, "ed25519 key", hex::decode)?;
        let ml_dsa_87 = parse_hex(
            &mut lines,
            FIELD_ML_DSA_87,
            "ml-dsa-87 key",
            hex::decode_boxed,
        )?;
        lines.finish()?;
        Ok(Self {
            suite,
            ed25519,
            ml_dsa_87,
        })
    }

    /// The suite this key belongs to.
    pub fn suite(&self) -> Suite {
        self.suite
    }

    /// The Ed25519 component.
    pub fn ed25519(&self) -> &[u8; ED25519_PUBLIC_KEY_LEN] {
        &self.ed25519
    }

    /// The ML-DSA-87 component.
    pub fn ml_dsa_87(&self) -> &[u8; ML_DSA_87_PUBLIC_KEY_LEN] {
        &self.ml_dsa_87
    }

    /// The SHA-256 digest of this key's canonical text form. It identifies
    /// the key in signatures and is what people compare by eye.
    pub fn key_id(&self) -> Sha256Digest {
        Sha256Digest::of_bytes(self.to_string().as_bytes())
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{PUBLIC_KEY_MAGIC} {FORMAT_VERSION}")?;
        writeln!(f, "{FIELD_SUITE}{}", self.suite.name())?;
        writeln!(f, "{FIELD_ED25519}{}", hex::encode(&self.ed25519))?;
        writeln!(f, "{FIELD_ML_DSA_87}{}", hex::encode(&self.ml_dsa_87[..]))
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({}, {})", self.suite.name(), self.key_id())
    }
}

/// A detached hybrid signature.
#[derive(Clone, PartialEq, Eq)]
pub struct Signature {
    suite: Suite,
    context: String,
    key_id: Sha256Digest,
    ed25519: [u8; ED25519_SIGNATURE_LEN],
    ml_dsa_87: Box<[u8; ML_DSA_87_SIGNATURE_LEN]>,
}

impl Signature {
    /// Assembles a signature of suite [`Suite::Ed25519MlDsa87`]. Fails if
    /// `context` is not a valid context name.
    pub fn new(
        context: &str,
        key_id: Sha256Digest,
        ed25519: [u8; ED25519_SIGNATURE_LEN],
        ml_dsa_87: Box<[u8; ML_DSA_87_SIGNATURE_LEN]>,
    ) -> Result<Self, FormatError> {
        if !is_context(context) {
            return Err(FormatError::BadValue {
                line: None,
                field: "context",
            });
        }
        Ok(Self {
            suite: Suite::Ed25519MlDsa87,
            context: context.to_owned(),
            key_id,
            ed25519,
            ml_dsa_87,
        })
    }

    /// Parses the canonical text form.
    pub fn parse(text: &str) -> Result<Self, FormatError> {
        let mut lines = Lines::open(text, MAX_SIGNATURE_BYTES, SIGNATURE_MAGIC, FORMAT_VERSION)?;
        let suite = parse_suite(&mut lines)?;
        let context = lines.field(FIELD_CONTEXT)?;
        if !is_context(context) {
            return Err(FormatError::BadValue {
                line: Some(lines.number()),
                field: "context",
            });
        }
        let key_id = parse_hex(&mut lines, FIELD_KEY_ID, "key-id", |text| {
            Sha256Digest::from_hex(text)
        })?;
        let ed25519 = parse_hex(&mut lines, FIELD_ED25519, "ed25519 signature", hex::decode)?;
        let ml_dsa_87 = parse_hex(
            &mut lines,
            FIELD_ML_DSA_87,
            "ml-dsa-87 signature",
            hex::decode_boxed,
        )?;
        lines.finish()?;
        Ok(Self {
            suite,
            context: context.to_owned(),
            key_id,
            ed25519,
            ml_dsa_87,
        })
    }

    /// The suite this signature was made with.
    pub fn suite(&self) -> Suite {
        self.suite
    }

    /// What the signature is for.
    pub fn context(&self) -> &str {
        &self.context
    }

    /// The identifier of the key that signed.
    pub fn key_id(&self) -> Sha256Digest {
        self.key_id
    }

    /// The Ed25519 component.
    pub fn ed25519(&self) -> &[u8; ED25519_SIGNATURE_LEN] {
        &self.ed25519
    }

    /// The ML-DSA-87 component.
    pub fn ml_dsa_87(&self) -> &[u8; ML_DSA_87_SIGNATURE_LEN] {
        &self.ml_dsa_87
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{SIGNATURE_MAGIC} {FORMAT_VERSION}")?;
        writeln!(f, "{FIELD_SUITE}{}", self.suite.name())?;
        writeln!(f, "{FIELD_CONTEXT}{}", self.context)?;
        writeln!(f, "{FIELD_KEY_ID}{}", self.key_id)?;
        writeln!(f, "{FIELD_ED25519}{}", hex::encode(&self.ed25519))?;
        writeln!(f, "{FIELD_ML_DSA_87}{}", hex::encode(&self.ml_dsa_87[..]))
    }
}

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Signature({}, context {}, key {})",
            self.suite.name(),
            self.context,
            self.key_id
        )
    }
}

fn parse_suite(lines: &mut Lines<'_>) -> Result<Suite, FormatError> {
    let name = lines.field(FIELD_SUITE)?;
    Suite::from_name(name).ok_or_else(|| FormatError::UnknownSuite(name.to_owned()))
}

fn parse_hex<T>(
    lines: &mut Lines<'_>,
    prefix: &'static str,
    field: &'static str,
    decode: impl Fn(&str) -> Option<T>,
) -> Result<T, FormatError> {
    let text = lines.field(prefix)?;
    decode(text).ok_or(FormatError::BadValue {
        line: Some(lines.number()),
        field,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_key() -> PublicKey {
        let mut ml_dsa_87 = Box::new([0u8; ML_DSA_87_PUBLIC_KEY_LEN]);
        for (index, byte) in ml_dsa_87.iter_mut().enumerate() {
            *byte = (index % 251) as u8;
        }
        PublicKey::new([7u8; ED25519_PUBLIC_KEY_LEN], ml_dsa_87)
    }

    fn sample_signature() -> Signature {
        let mut ml_dsa_87 = Box::new([0u8; ML_DSA_87_SIGNATURE_LEN]);
        for (index, byte) in ml_dsa_87.iter_mut().enumerate() {
            *byte = (index % 241) as u8;
        }
        Signature::new(
            "build-manifest",
            sample_key().key_id(),
            [9u8; ED25519_SIGNATURE_LEN],
            ml_dsa_87,
        )
        .expect("valid context")
    }

    #[test]
    fn public_key_round_trip() {
        let key = sample_key();
        let text = key.to_string();
        assert!(text.starts_with("nottos-public-key 1\nsuite: ed25519+ml-dsa-87\ned25519: 0707"));
        assert_eq!(text.lines().count(), 4);
        let parsed = PublicKey::parse(&text).expect("parses");
        assert_eq!(parsed, key);
        assert_eq!(parsed.to_string(), text);
        assert_eq!(parsed.key_id(), Sha256Digest::of_bytes(text.as_bytes()));
        assert_eq!(parsed.suite().id(), 1);
    }

    #[test]
    fn signature_round_trip() {
        let signature = sample_signature();
        let text = signature.to_string();
        assert_eq!(text.lines().count(), 6);
        let parsed = Signature::parse(&text).expect("parses");
        assert_eq!(parsed, signature);
        assert_eq!(parsed.to_string(), text);
        assert_eq!(parsed.context(), "build-manifest");
        assert_eq!(parsed.key_id(), sample_key().key_id());
    }

    #[test]
    fn key_id_depends_on_every_component() {
        let base = sample_key();
        let mut other_ed = *base.ed25519();
        other_ed[0] ^= 1;
        let changed_ed = PublicKey::new(other_ed, Box::new(*base.ml_dsa_87()));
        let mut other_ml = Box::new(*base.ml_dsa_87());
        other_ml[ML_DSA_87_PUBLIC_KEY_LEN - 1] ^= 1;
        let changed_ml = PublicKey::new(*base.ed25519(), other_ml);
        assert_ne!(base.key_id(), changed_ed.key_id());
        assert_ne!(base.key_id(), changed_ml.key_id());
        assert_ne!(changed_ed.key_id(), changed_ml.key_id());
    }

    #[test]
    fn context_names() {
        for good in ["a", "build-manifest", "update-metadata-2", &"x".repeat(32)] {
            assert!(is_context(good), "{good:?}");
        }
        for bad in ["", "Build", "a b", "a_b", "a.b", &"x".repeat(33)] {
            assert!(!is_context(bad), "{bad:?}");
        }
        assert_eq!(
            Signature::new(
                "Bad Context",
                sample_key().key_id(),
                [0u8; ED25519_SIGNATURE_LEN],
                Box::new([0u8; ML_DSA_87_SIGNATURE_LEN])
            ),
            Err(FormatError::BadValue {
                line: None,
                field: "context"
            })
        );
    }

    #[test]
    fn rejects_malformed_files() {
        let key = sample_key().to_string();
        let signature = sample_signature().to_string();
        let key_cases: Vec<(String, FormatError)> = vec![
            (
                key.replacen("ed25519+ml-dsa-87", "ed25519", 1),
                FormatError::UnknownSuite("ed25519".to_owned()),
            ),
            (
                key.replacen("ed25519: 07", "ed25519: 0", 1),
                FormatError::BadValue {
                    line: Some(3),
                    field: "ed25519 key",
                },
            ),
            (
                key.replacen("ml-dsa-87: 00", "ml-dsa-87: 0G", 1),
                FormatError::BadValue {
                    line: Some(4),
                    field: "ml-dsa-87 key",
                },
            ),
            (
                format!("{key}extra: 1\n"),
                FormatError::Envelope(Problem::UnexpectedLine {
                    line: 5,
                    expected: "end of file",
                }),
            ),
            (
                key.replacen("nottos-public-key 1", "nottos-public-key 2", 1),
                FormatError::Envelope(Problem::UnsupportedVersion("2".to_owned())),
            ),
            (signature.clone(), FormatError::Envelope(Problem::TooLarge)),
        ];
        for (text, expected) in key_cases {
            assert_eq!(PublicKey::parse(&text), Err(expected));
        }

        let signature_cases: Vec<(String, FormatError)> = vec![
            (
                signature.replacen("context: build-manifest", "context: Build", 1),
                FormatError::BadValue {
                    line: Some(3),
                    field: "context",
                },
            ),
            (
                signature.replacen("key-id: ", "key-id: 0", 1),
                FormatError::BadValue {
                    line: Some(4),
                    field: "key-id",
                },
            ),
            (
                signature.replacen("ed25519: 09", "ed25519: 9", 1),
                FormatError::BadValue {
                    line: Some(5),
                    field: "ed25519 signature",
                },
            ),
            (
                signature.replacen("context: build-manifest\n", "", 1),
                FormatError::Envelope(Problem::UnexpectedLine {
                    line: 3,
                    expected: "context: ",
                }),
            ),
            (key.clone(), FormatError::Envelope(Problem::BadHeader)),
        ];
        for (text, expected) in signature_cases {
            assert_eq!(Signature::parse(&text), Err(expected));
        }
    }

    /// No other byte string parses to the sample signature. Checked for
    /// single-bit changes of every byte of the short fields and of a spread
    /// of positions in the long last field.
    #[test]
    fn bit_flips_are_rejected_or_change_the_signature() {
        let original = sample_signature();
        let text = original.to_string();
        let bytes = text.as_bytes();
        let positions = (0..600)
            .chain((600..bytes.len()).step_by(53))
            .chain(bytes.len() - 16..bytes.len());
        for index in positions {
            for bit in 0..8 {
                let mut mutated = bytes.to_vec();
                mutated[index] ^= 1 << bit;
                let Ok(mutated) = String::from_utf8(mutated) else {
                    continue;
                };
                if let Ok(parsed) = Signature::parse(&mutated) {
                    assert_ne!(parsed, original, "byte {index}, bit {bit}");
                    assert_eq!(parsed.to_string(), mutated, "byte {index}, bit {bit}");
                }
            }
        }
    }
}
