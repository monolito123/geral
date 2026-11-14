// SPDX-License-Identifier: Apache-2.0

//! NottOS hybrid signatures.
//!
//! A hybrid signature combines a classical algorithm with a post-quantum
//! one and is valid only if **both** component signatures are. Forging one
//! therefore requires breaking both algorithms.
//!
//! This crate implements no cryptographic primitive. It composes Ed25519
//! (RFC 8032) and ML-DSA-87 (FIPS 204) from the `libcrux` implementations,
//! SLH-DSA-SHA2-256s (FIPS 205) from `fips205`, and SHA-512 from `sha2`. The
//! construction and the choice of implementations are recorded in
//! `docs/adr/0008-signature-suite.md` and `docs/adr/0009-update-system.md`.
//!
//! # Construction, every suite
//!
//! To sign message `M` for purpose `context`, both algorithms sign the same
//! *representative*:
//!
//! ```text
//! R = "NottOS-hybrid-signature-v1" || 0x00
//!     || suite id (2 bytes, big endian)
//!     || length of context (1 byte) || context
//!     || SHA-512(M)
//! ```
//!
//! - classical half: `Ed25519.Sign(sk1, R)`
//! - post-quantum half: `ML-DSA-87.Sign(sk2, R, ctx = "")` (suite 1) or
//!   `SLH-DSA-SHA2-256s.Sign(sk2, R, ctx = "")` (suite 2), both hedged
//!
//! Because `R` names the construction, the suite and the purpose, a
//! component signature taken out of a hybrid signature is not a signature on
//! `M` under that algorithm alone, a signature made for one purpose does not
//! verify for another, and a signature made under one suite does not verify
//! under another.

pub mod kem;
mod rng;
mod secret;

pub use nott_format::signature::{FormatError, PublicKey, Signature, SignatureSet, Suite};
pub use secret::SecretKey;

use fips205::slh_dsa_sha2_256s;
use fips205::traits::{SerDes as _, Verifier as _};
use libcrux_ml_dsa::ml_dsa_87::{self, MLDSA87Signature, MLDSA87VerificationKey};
use nott_format::signature::is_context;
use sha2::{Digest as _, Sha512};
use std::fmt;

/// Purposes a signature can be made for. Each protocol uses its own.
pub mod context {
    /// A release build manifest (`nott_format::manifest`).
    pub const BUILD_MANIFEST: &str = "build-manifest";
    /// Boot metadata: the root hashes of the verified partitions, checked
    /// before any of them is used (what `vbmeta` is on Android).
    pub const BOOT_METADATA: &str = "boot-metadata";
    /// Root metadata of the update system (`nott_format::update`).
    pub const ROOT_METADATA: &str = "root-metadata";
    /// Update metadata (`nott_format::update`).
    pub const UPDATE_METADATA: &str = "update-metadata";
}

/// First bytes of every representative, including the terminating zero.
const LABEL: &[u8] = b"NottOS-hybrid-signature-v1\0";

/// The post-quantum algorithm's own context string. Always empty: the
/// purpose is already bound into the representative, for both halves alike.
const PQ_CONTEXT: &[u8] = b"";

/// Why a signing operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoError {
    /// The operating system did not provide randomness.
    Randomness,
    /// The context is not a valid context name.
    InvalidContext,
    /// A seed does not have the length the suite requires.
    InvalidSeed,
    /// A component algorithm reported failure.
    Signing,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Randomness => "the operating system provided no randomness",
            Self::InvalidContext => "invalid signature context name",
            Self::InvalidSeed => "seed has the wrong length for this suite",
            Self::Signing => "signing failed",
        })
    }
}

impl std::error::Error for CryptoError {}

/// Why a signature was not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyError {
    /// Key and signature belong to different suites.
    SuiteMismatch,
    /// The signature was made for another purpose.
    ContextMismatch,
    /// The signature names another key.
    KeyMismatch,
    /// At least one component signature is invalid for this key and message.
    BadSignature,
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SuiteMismatch => "key and signature use different suites",
            Self::ContextMismatch => "signature was made for a different purpose",
            Self::KeyMismatch => "signature was made by a different key",
            Self::BadSignature => "signature is not valid for this key and message",
        })
    }
}

impl std::error::Error for VerifyError {}

/// Builds the byte string that both component algorithms sign. Returns
/// `None` if `context` is not a valid context name.
fn representative(suite: Suite, context: &str, message: &[u8]) -> Option<Vec<u8>> {
    if !is_context(context) {
        return None;
    }
    let context_len = u8::try_from(context.len()).ok()?;
    let mut bytes = Vec::with_capacity(LABEL.len() + 3 + context.len() + 64);
    bytes.extend_from_slice(LABEL);
    bytes.extend_from_slice(&suite.id().to_be_bytes());
    bytes.push(context_len);
    bytes.extend_from_slice(context.as_bytes());
    bytes.extend_from_slice(&Sha512::digest(message));
    Some(bytes)
}

/// Checks a hybrid signature.
///
/// Succeeds only if the signature was made by `public_key`, for `context`,
/// over exactly `message`, and both component signatures verify.
pub fn verify(
    public_key: &PublicKey,
    context: &str,
    message: &[u8],
    signature: &Signature,
) -> Result<(), VerifyError> {
    if signature.suite() != public_key.suite() {
        return Err(VerifyError::SuiteMismatch);
    }
    if signature.context() != context {
        return Err(VerifyError::ContextMismatch);
    }
    if signature.key_id() != public_key.key_id() {
        return Err(VerifyError::KeyMismatch);
    }
    let suite = public_key.suite();
    let representative =
        representative(suite, context, message).ok_or(VerifyError::ContextMismatch)?;
    // Both halves are always evaluated; which one failed is not reported.
    let classical_valid =
        libcrux_ed25519::verify(&representative, public_key.ed25519(), signature.ed25519()).is_ok();
    let post_quantum_valid = verify_post_quantum(
        suite,
        public_key.post_quantum(),
        &representative,
        signature.post_quantum(),
    );
    if classical_valid && post_quantum_valid {
        Ok(())
    } else {
        Err(VerifyError::BadSignature)
    }
}

/// Checks the post-quantum half. Lengths have already been enforced by the
/// formats; a mismatch here is still treated as an invalid signature.
fn verify_post_quantum(suite: Suite, key: &[u8], representative: &[u8], signature: &[u8]) -> bool {
    match suite {
        Suite::Ed25519MlDsa87 => {
            let (Ok(key), Ok(signature)) = (key.try_into(), signature.try_into()) else {
                return false;
            };
            ml_dsa_87::verify(
                &MLDSA87VerificationKey::new(key),
                representative,
                PQ_CONTEXT,
                &MLDSA87Signature::new(signature),
            )
            .is_ok()
        }
        Suite::Ed25519SlhDsaSha2_256s => {
            let Ok(key) = <[u8; slh_dsa_sha2_256s::PK_LEN]>::try_from(key) else {
                return false;
            };
            let Ok(signature) = <&[u8; slh_dsa_sha2_256s::SIG_LEN]>::try_from(signature) else {
                return false;
            };
            let Ok(key) = slh_dsa_sha2_256s::PublicKey::try_from_bytes(key) else {
                return false;
            };
            key.verify(representative, signature, PQ_CONTEXT)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn representative_layout() {
        let bytes =
            representative(Suite::Ed25519MlDsa87, "build-manifest", b"abc").expect("valid context");
        let mut expected = b"NottOS-hybrid-signature-v1\0".to_vec();
        expected.extend_from_slice(&[0x00, 0x01]);
        expected.push(14);
        expected.extend_from_slice(b"build-manifest");
        // SHA-512("abc"), FIPS 180-4.
        expected.extend_from_slice(
            &nott_format::hex::decode::<64>(
                "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
                 2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
            )
            .expect("valid hex"),
        );
        assert_eq!(bytes, expected);
        assert_eq!(bytes.len(), 27 + 2 + 1 + 14 + 64);
    }

    #[test]
    fn representative_separates_suites_contexts_and_messages() {
        let of = |suite, context, message: &[u8]| {
            representative(suite, context, message).expect("valid context")
        };
        let suite = Suite::Ed25519MlDsa87;
        assert_ne!(of(suite, "a", b"m"), of(suite, "b", b"m"));
        assert_ne!(of(suite, "a", b"m"), of(suite, "a", b"n"));
        assert_ne!(
            of(Suite::Ed25519MlDsa87, "a", b"m"),
            of(Suite::Ed25519SlhDsaSha2_256s, "a", b"m")
        );
        // The length byte keeps a context from sliding into the digest.
        assert_ne!(of(suite, "ab", b"m")[..30], of(suite, "a", b"m")[..30]);
    }

    #[test]
    fn representative_rejects_invalid_contexts() {
        for context in ["", "Upper", "has space", &"x".repeat(33)] {
            assert!(representative(Suite::Ed25519MlDsa87, context, b"m").is_none());
        }
    }

    #[test]
    fn wrong_lengths_never_verify() {
        for suite in Suite::ALL {
            let key = vec![0u8; suite.post_quantum_public_key_len()];
            let signature = vec![0u8; suite.post_quantum_signature_len()];
            assert!(!verify_post_quantum(suite, &key[1..], b"r", &signature));
            assert!(!verify_post_quantum(suite, &key, b"r", &signature[1..]));
            assert!(!verify_post_quantum(suite, &key, b"r", &signature));
        }
    }
}
