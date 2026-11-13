// SPDX-License-Identifier: Apache-2.0

//! Containers for hybrid public keys and detached signatures.
//!
//! This module only stores and parses bytes. Creating and checking
//! signatures is the job of `nott-crypto`, so that code which merely moves
//! keys and signatures around does not depend on cryptographic libraries.
//!
//! Every suite pairs Ed25519 with one post-quantum algorithm
//! (`docs/adr/0008-signature-suite.md`, `docs/adr/0009-update-system.md`):
//!
//! | Suite | Name | Post-quantum half | Used for |
//! |---|---|---|---|
//! | 1 | `ed25519+ml-dsa-87` | ML-DSA-87 (FIPS 204) | release and other working keys |
//! | 2 | `ed25519+slh-dsa-sha2-256s` | SLH-DSA-SHA2-256s (FIPS 205) | long-lived root keys |
//!
//! # Public key, format version 1
//!
//! ```text
//! nottos-public-key 1
//! suite: <suite name>
//! ed25519: <64 hex digits>
//! <post-quantum name>: <hex>
//! ```
//!
//! # Signature, format version 1
//!
//! ```text
//! nottos-signature 1
//! suite: <suite name>
//! context: build-manifest
//! key-id: <64 hex digits>
//! ed25519: <128 hex digits>
//! <post-quantum name>: <hex>
//! ```
//!
//! `key-id` is the SHA-256 digest of the signer's public key file. `context`
//! names what the signature is for; a signature made for one purpose does
//! not verify for another.
//!
//! # Signature set, format version 1
//!
//! One or more signatures over the same bytes, concatenated, in strictly
//! ascending order of key-id. Used where a threshold of keys must sign.
//!
//! All formats follow the rules of [`crate::text`]: one canonical byte form.
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
/// Length of an SLH-DSA-SHA2-256s public key in bytes (FIPS 205).
pub const SLH_DSA_SHA2_256S_PUBLIC_KEY_LEN: usize = 64;
/// Length of an SLH-DSA-SHA2-256s signature in bytes (FIPS 205).
pub const SLH_DSA_SHA2_256S_SIGNATURE_LEN: usize = 29792;

/// Longest context name.
pub const MAX_CONTEXT_LEN: usize = 32;
/// Most signatures a signature set may hold.
pub const MAX_SET_SIGNATURES: usize = 16;

const PUBLIC_KEY_MAGIC: &str = "nottos-public-key";
const SIGNATURE_MAGIC: &str = "nottos-signature";
const FORMAT_VERSION: u32 = 1;
const MAX_PUBLIC_KEY_BYTES: usize = 8 * 1024;
const MAX_SIGNATURE_BYTES: usize = 128 * 1024;
const MAX_SET_BYTES: usize = MAX_SET_SIGNATURES * MAX_SIGNATURE_BYTES;

const FIELD_SUITE: &str = "suite: ";
const FIELD_CONTEXT: &str = "context: ";
const FIELD_KEY_ID: &str = "key-id: ";
const FIELD_ED25519: &str = "ed25519: ";
const FIELD_ML_DSA_87: &str = "ml-dsa-87: ";
const FIELD_SLH_DSA_SHA2_256S: &str = "slh-dsa-sha2-256s: ";

/// A combination of signature algorithms. A signature is valid only if both
/// of its component signatures are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Suite {
    /// Ed25519 (RFC 8032) with ML-DSA-87 (FIPS 204).
    Ed25519MlDsa87,
    /// Ed25519 (RFC 8032) with SLH-DSA-SHA2-256s (FIPS 205).
    Ed25519SlhDsaSha2_256s,
}

impl Suite {
    /// Every suite this version knows.
    pub const ALL: [Suite; 2] = [Suite::Ed25519MlDsa87, Suite::Ed25519SlhDsaSha2_256s];

    /// The name used in files.
    pub fn name(self) -> &'static str {
        match self {
            Self::Ed25519MlDsa87 => "ed25519+ml-dsa-87",
            Self::Ed25519SlhDsaSha2_256s => "ed25519+slh-dsa-sha2-256s",
        }
    }

    /// The number bound into every signature of this suite.
    pub fn id(self) -> u16 {
        match self {
            Self::Ed25519MlDsa87 => 1,
            Self::Ed25519SlhDsaSha2_256s => 2,
        }
    }

    /// The name of the post-quantum algorithm, as used for its field.
    pub fn post_quantum_name(self) -> &'static str {
        self.post_quantum_field().trim_end_matches(": ")
    }

    /// Length of the post-quantum half of a public key, in bytes.
    pub fn post_quantum_public_key_len(self) -> usize {
        match self {
            Self::Ed25519MlDsa87 => ML_DSA_87_PUBLIC_KEY_LEN,
            Self::Ed25519SlhDsaSha2_256s => SLH_DSA_SHA2_256S_PUBLIC_KEY_LEN,
        }
    }

    /// Length of the post-quantum half of a signature, in bytes.
    pub fn post_quantum_signature_len(self) -> usize {
        match self {
            Self::Ed25519MlDsa87 => ML_DSA_87_SIGNATURE_LEN,
            Self::Ed25519SlhDsaSha2_256s => SLH_DSA_SHA2_256S_SIGNATURE_LEN,
        }
    }

    /// Looks a suite up by its file name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|suite| suite.name() == name)
    }

    fn post_quantum_field(self) -> &'static str {
        match self {
            Self::Ed25519MlDsa87 => FIELD_ML_DSA_87,
            Self::Ed25519SlhDsaSha2_256s => FIELD_SLH_DSA_SHA2_256S,
        }
    }

    fn post_quantum_key_label(self) -> &'static str {
        match self {
            Self::Ed25519MlDsa87 => "ml-dsa-87 key",
            Self::Ed25519SlhDsaSha2_256s => "slh-dsa-sha2-256s key",
        }
    }

    fn post_quantum_signature_label(self) -> &'static str {
        match self {
            Self::Ed25519MlDsa87 => "ml-dsa-87 signature",
            Self::Ed25519SlhDsaSha2_256s => "slh-dsa-sha2-256s signature",
        }
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
    /// A signature set is empty, too large, or not in strictly ascending
    /// order of key-id.
    BadSet(&'static str),
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
            Self::BadSet(reason) => write!(f, "invalid signature set: {reason}"),
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
    post_quantum: Box<[u8]>,
}

impl PublicKey {
    /// Assembles a key from its two component keys. Fails if the
    /// post-quantum key does not have the length the suite requires.
    pub fn new(
        suite: Suite,
        ed25519: [u8; ED25519_PUBLIC_KEY_LEN],
        post_quantum: Vec<u8>,
    ) -> Result<Self, FormatError> {
        if post_quantum.len() != suite.post_quantum_public_key_len() {
            return Err(FormatError::BadValue {
                line: None,
                field: suite.post_quantum_key_label(),
            });
        }
        Ok(Self {
            suite,
            ed25519,
            post_quantum: post_quantum.into_boxed_slice(),
        })
    }

    /// Parses the canonical text form.
    pub fn parse(text: &str) -> Result<Self, FormatError> {
        let mut lines = Lines::open(text, MAX_PUBLIC_KEY_BYTES, PUBLIC_KEY_MAGIC, FORMAT_VERSION)?;
        let suite = parse_suite(&mut lines)?;
        let ed25519 = parse_hex(&mut lines, FIELD_ED25519, "ed25519 key", hex::decode)?;
        let post_quantum = parse_hex(
            &mut lines,
            suite.post_quantum_field(),
            suite.post_quantum_key_label(),
            |text| decode_exact(text, suite.post_quantum_public_key_len()),
        )?;
        lines.finish()?;
        Ok(Self {
            suite,
            ed25519,
            post_quantum,
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

    /// The post-quantum component, exactly
    /// [`Suite::post_quantum_public_key_len`] bytes long.
    pub fn post_quantum(&self) -> &[u8] {
        &self.post_quantum
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
        writeln!(
            f,
            "{}{}",
            self.suite.post_quantum_field(),
            hex::encode(&self.post_quantum)
        )
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
    post_quantum: Box<[u8]>,
}

impl Signature {
    /// Assembles a signature. Fails if `context` is not a valid context
    /// name or the post-quantum signature does not have the length the
    /// suite requires.
    pub fn new(
        suite: Suite,
        context: &str,
        key_id: Sha256Digest,
        ed25519: [u8; ED25519_SIGNATURE_LEN],
        post_quantum: Vec<u8>,
    ) -> Result<Self, FormatError> {
        if !is_context(context) {
            return Err(FormatError::BadValue {
                line: None,
                field: "context",
            });
        }
        if post_quantum.len() != suite.post_quantum_signature_len() {
            return Err(FormatError::BadValue {
                line: None,
                field: suite.post_quantum_signature_label(),
            });
        }
        Ok(Self {
            suite,
            context: context.to_owned(),
            key_id,
            ed25519,
            post_quantum: post_quantum.into_boxed_slice(),
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
        let post_quantum = parse_hex(
            &mut lines,
            suite.post_quantum_field(),
            suite.post_quantum_signature_label(),
            |text| decode_exact(text, suite.post_quantum_signature_len()),
        )?;
        lines.finish()?;
        Ok(Self {
            suite,
            context: context.to_owned(),
            key_id,
            ed25519,
            post_quantum,
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

    /// The post-quantum component, exactly
    /// [`Suite::post_quantum_signature_len`] bytes long.
    pub fn post_quantum(&self) -> &[u8] {
        &self.post_quantum
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{SIGNATURE_MAGIC} {FORMAT_VERSION}")?;
        writeln!(f, "{FIELD_SUITE}{}", self.suite.name())?;
        writeln!(f, "{FIELD_CONTEXT}{}", self.context)?;
        writeln!(f, "{FIELD_KEY_ID}{}", self.key_id)?;
        writeln!(f, "{FIELD_ED25519}{}", hex::encode(&self.ed25519))?;
        writeln!(
            f,
            "{}{}",
            self.suite.post_quantum_field(),
            hex::encode(&self.post_quantum)
        )
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

/// One or more signatures over the same bytes, by different keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureSet {
    signatures: Vec<Signature>,
}

impl SignatureSet {
    /// Builds a set. Signatures are put in ascending order of key-id; two
    /// signatures by the same key are an error.
    pub fn new(mut signatures: Vec<Signature>) -> Result<Self, FormatError> {
        signatures.sort_by_key(Signature::key_id);
        Self::checked(signatures)
    }

    /// Parses the canonical text form: signatures concatenated in strictly
    /// ascending order of key-id.
    pub fn parse(text: &str) -> Result<Self, FormatError> {
        if text.len() > MAX_SET_BYTES {
            return Err(FormatError::Envelope(Problem::TooLarge));
        }
        let header = format!("{SIGNATURE_MAGIC} ");
        if !text.starts_with(&header) {
            return Err(FormatError::Envelope(Problem::BadHeader));
        }
        // Every signature starts with its header at the start of a line. No
        // field of a signature can contain that text, so the split is unique.
        let mut starts: Vec<usize> = text
            .match_indices(&format!("\n{header}"))
            .map(|(index, _)| index + 1)
            .collect();
        starts.insert(0, 0);
        if starts.len() > MAX_SET_SIGNATURES {
            return Err(FormatError::BadSet("too many signatures"));
        }
        starts.push(text.len());
        let signatures = starts
            .windows(2)
            .map(|pair| Signature::parse(&text[pair[0]..pair[1]]))
            .collect::<Result<Vec<_>, _>>()?;
        Self::checked(signatures)
    }

    fn checked(signatures: Vec<Signature>) -> Result<Self, FormatError> {
        if signatures.is_empty() {
            return Err(FormatError::BadSet("no signature"));
        }
        if signatures.len() > MAX_SET_SIGNATURES {
            return Err(FormatError::BadSet("too many signatures"));
        }
        if signatures
            .windows(2)
            .any(|pair| pair[0].key_id() >= pair[1].key_id())
        {
            return Err(FormatError::BadSet(
                "signatures are not in strictly ascending order of key-id",
            ));
        }
        Ok(Self { signatures })
    }

    /// The signatures, in ascending order of key-id.
    pub fn signatures(&self) -> &[Signature] {
        &self.signatures
    }
}

impl fmt::Display for SignatureSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for signature in &self.signatures {
            write!(f, "{signature}")?;
        }
        Ok(())
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

fn decode_exact(text: &str, len: usize) -> Option<Box<[u8]>> {
    let mut bytes = vec![0u8; len].into_boxed_slice();
    hex::decode_into(text, &mut bytes).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(len: usize, modulus: usize) -> Vec<u8> {
        (0..len).map(|index| (index % modulus) as u8).collect()
    }

    fn sample_key(suite: Suite) -> PublicKey {
        PublicKey::new(
            suite,
            [7u8; ED25519_PUBLIC_KEY_LEN],
            pattern(suite.post_quantum_public_key_len(), 251),
        )
        .expect("valid key")
    }

    fn sample_signature(suite: Suite, key_id: Sha256Digest) -> Signature {
        Signature::new(
            suite,
            "build-manifest",
            key_id,
            [9u8; ED25519_SIGNATURE_LEN],
            pattern(suite.post_quantum_signature_len(), 241),
        )
        .expect("valid signature")
    }

    #[test]
    fn suite_names_and_lengths() {
        assert_eq!(
            Suite::from_name("ed25519+ml-dsa-87"),
            Some(Suite::Ed25519MlDsa87)
        );
        assert_eq!(
            Suite::from_name("ed25519+slh-dsa-sha2-256s"),
            Some(Suite::Ed25519SlhDsaSha2_256s)
        );
        assert_eq!(Suite::from_name("ed25519"), None);
        assert_eq!(Suite::Ed25519MlDsa87.post_quantum_name(), "ml-dsa-87");
        assert_eq!(
            Suite::Ed25519SlhDsaSha2_256s.post_quantum_name(),
            "slh-dsa-sha2-256s"
        );
        let ids: Vec<u16> = Suite::ALL.iter().map(|suite| suite.id()).collect();
        assert_eq!(ids, [1, 2]);
    }

    #[test]
    fn public_key_round_trip() {
        for suite in Suite::ALL {
            let key = sample_key(suite);
            let text = key.to_string();
            assert!(text.starts_with(&format!(
                "nottos-public-key 1\nsuite: {}\ned25519: 0707",
                suite.name()
            )));
            assert!(text.contains(&format!("\n{}: 0001", suite.post_quantum_name())));
            assert_eq!(text.lines().count(), 4);
            let parsed = PublicKey::parse(&text).expect("parses");
            assert_eq!(parsed, key);
            assert_eq!(parsed.to_string(), text);
            assert_eq!(parsed.key_id(), Sha256Digest::of_bytes(text.as_bytes()));
        }
    }

    #[test]
    fn signature_round_trip() {
        for suite in Suite::ALL {
            let signature = sample_signature(suite, sample_key(suite).key_id());
            let text = signature.to_string();
            assert_eq!(text.lines().count(), 6);
            let parsed = Signature::parse(&text).expect("parses");
            assert_eq!(parsed, signature);
            assert_eq!(parsed.to_string(), text);
            assert_eq!(parsed.context(), "build-manifest");
            assert_eq!(
                parsed.post_quantum().len(),
                suite.post_quantum_signature_len()
            );
        }
    }

    #[test]
    fn lengths_are_checked() {
        for suite in Suite::ALL {
            let short = suite.post_quantum_public_key_len() - 1;
            assert!(PublicKey::new(suite, [0u8; 32], vec![0u8; short]).is_err());
            let long = suite.post_quantum_signature_len() + 1;
            assert!(Signature::new(
                suite,
                "build-manifest",
                Sha256Digest::of_bytes(b""),
                [0u8; 64],
                vec![0u8; long]
            )
            .is_err());
        }
        // The post-quantum field of one suite is not accepted under another.
        let text = sample_key(Suite::Ed25519MlDsa87).to_string().replacen(
            "ed25519+ml-dsa-87",
            "ed25519+slh-dsa-sha2-256s",
            1,
        );
        assert_eq!(
            PublicKey::parse(&text),
            Err(FormatError::Envelope(Problem::UnexpectedLine {
                line: 4,
                expected: "slh-dsa-sha2-256s: ",
            }))
        );
    }

    #[test]
    fn key_id_depends_on_every_component() {
        let base = sample_key(Suite::Ed25519MlDsa87);
        let mut other_ed = *base.ed25519();
        other_ed[0] ^= 1;
        let changed_ed =
            PublicKey::new(base.suite(), other_ed, base.post_quantum().to_vec()).expect("valid");
        let mut other_pq = base.post_quantum().to_vec();
        other_pq[ML_DSA_87_PUBLIC_KEY_LEN - 1] ^= 1;
        let changed_pq = PublicKey::new(base.suite(), *base.ed25519(), other_pq).expect("valid");
        assert_ne!(base.key_id(), changed_ed.key_id());
        assert_ne!(base.key_id(), changed_pq.key_id());
        assert_ne!(changed_ed.key_id(), changed_pq.key_id());
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
                Suite::Ed25519MlDsa87,
                "Bad Context",
                Sha256Digest::of_bytes(b""),
                [0u8; ED25519_SIGNATURE_LEN],
                vec![0u8; ML_DSA_87_SIGNATURE_LEN]
            ),
            Err(FormatError::BadValue {
                line: None,
                field: "context"
            })
        );
    }

    #[test]
    fn rejects_malformed_files() {
        let key = sample_key(Suite::Ed25519MlDsa87).to_string();
        let signature =
            sample_signature(Suite::Ed25519MlDsa87, Sha256Digest::of_bytes(b"k")).to_string();
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
            // A signature is larger than any public key may be, so the size
            // limit rejects it before the header is read.
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
        let original = sample_signature(Suite::Ed25519MlDsa87, Sha256Digest::of_bytes(b"k"));
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

    fn set_member(tag: u8, suite: Suite) -> Signature {
        sample_signature(suite, Sha256Digest::from_bytes([tag; 32]))
    }

    #[test]
    fn signature_set_round_trip_and_order() {
        let set = SignatureSet::new(vec![
            set_member(3, Suite::Ed25519SlhDsaSha2_256s),
            set_member(1, Suite::Ed25519MlDsa87),
            set_member(2, Suite::Ed25519MlDsa87),
        ])
        .expect("valid set");
        let ids: Vec<u8> = set
            .signatures()
            .iter()
            .map(|signature| signature.key_id().as_bytes()[0])
            .collect();
        assert_eq!(ids, [1, 2, 3]);
        let text = set.to_string();
        assert_eq!(text.matches("nottos-signature 1\n").count(), 3);
        let parsed = SignatureSet::parse(&text).expect("parses");
        assert_eq!(parsed, set);
        assert_eq!(parsed.to_string(), text);

        // A single signature file is a set of one.
        let one = set_member(9, Suite::Ed25519MlDsa87);
        assert_eq!(
            SignatureSet::parse(&one.to_string()).map(|set| set.signatures().len()),
            Ok(1)
        );
    }

    #[test]
    fn signature_set_rejects_disorder_duplicates_and_damage() {
        let (a, b) = (
            set_member(1, Suite::Ed25519MlDsa87),
            set_member(2, Suite::Ed25519MlDsa87),
        );
        let out_of_order = format!("{b}{a}");
        let duplicate = format!("{a}{a}");
        let order_error = Err(FormatError::BadSet(
            "signatures are not in strictly ascending order of key-id",
        ));
        assert_eq!(SignatureSet::parse(&out_of_order), order_error);
        assert_eq!(SignatureSet::parse(&duplicate), order_error);
        assert_eq!(SignatureSet::new(vec![a.clone(), a.clone()]), order_error);
        assert_eq!(
            SignatureSet::new(vec![]),
            Err(FormatError::BadSet("no signature"))
        );
        assert_eq!(
            SignatureSet::parse(""),
            Err(FormatError::Envelope(Problem::BadHeader))
        );
        assert_eq!(
            SignatureSet::parse(&format!("x{a}{b}")),
            Err(FormatError::Envelope(Problem::BadHeader))
        );
        // Text between signatures belongs to the previous one, which is then
        // not canonical.
        assert!(SignatureSet::parse(&format!("{a}junk\n{b}")).is_err());
        let many: String = (0..=MAX_SET_SIGNATURES as u8)
            .map(|tag| set_member(tag, Suite::Ed25519MlDsa87).to_string())
            .collect();
        assert_eq!(
            SignatureSet::parse(&many),
            Err(FormatError::BadSet("too many signatures"))
        );
    }
}
