// SPDX-License-Identifier: Apache-2.0

//! Hybrid secret keys.
//!
//! A secret key is two 32-byte seeds, one per component algorithm. The
//! working keys are derived from the seeds when needed and wiped afterwards.
//!
//! # Secret key file, format version 1
//!
//! ```text
//! nottos-secret-key 1
//! suite: ed25519+ml-dsa-87
//! ed25519-seed: <64 hex digits>
//! ml-dsa-87-seed: <64 hex digits>
//! ```
//!
//! This file format exists for test and development keys. Release keys are
//! meant to live in offline hardware and never appear in this form.

use crate::{representative, CryptoError, ML_DSA_CONTEXT};
use libcrux_ml_dsa::ml_dsa_87;
use nott_format::hex;
use nott_format::signature::{FormatError, PublicKey, Signature, Suite};
use nott_format::text::Lines;
use std::fmt;
use zeroize::{Zeroize as _, Zeroizing};

const SEED_LEN: usize = 32;
const MAGIC: &str = "nottos-secret-key";
const FORMAT_VERSION: u32 = 1;
const MAX_BYTES: usize = 1024;
const FIELD_SUITE: &str = "suite: ";
const FIELD_ED25519_SEED: &str = "ed25519-seed: ";
const FIELD_ML_DSA_87_SEED: &str = "ml-dsa-87-seed: ";

/// A hybrid secret key of suite [`Suite::Ed25519MlDsa87`]. The seeds are
/// wiped from memory when the value is dropped.
pub struct SecretKey {
    ed25519_seed: Zeroizing<[u8; SEED_LEN]>,
    ml_dsa_87_seed: Zeroizing<[u8; SEED_LEN]>,
}

impl SecretKey {
    /// Creates a new key from operating-system randomness.
    pub fn generate() -> Result<Self, CryptoError> {
        let mut key = Self {
            ed25519_seed: Zeroizing::new([0u8; SEED_LEN]),
            ml_dsa_87_seed: Zeroizing::new([0u8; SEED_LEN]),
        };
        fill_random(&mut key.ed25519_seed[..])?;
        fill_random(&mut key.ml_dsa_87_seed[..])?;
        Ok(key)
    }

    /// Rebuilds a key from its two seeds.
    pub fn from_seeds(ed25519_seed: [u8; SEED_LEN], ml_dsa_87_seed: [u8; SEED_LEN]) -> Self {
        Self {
            ed25519_seed: Zeroizing::new(ed25519_seed),
            ml_dsa_87_seed: Zeroizing::new(ml_dsa_87_seed),
        }
    }

    /// Parses the secret key file format.
    pub fn parse(text: &str) -> Result<Self, FormatError> {
        let mut lines = Lines::open(text, MAX_BYTES, MAGIC, FORMAT_VERSION)?;
        let suite = lines.field(FIELD_SUITE)?;
        if suite != Suite::Ed25519MlDsa87.name() {
            return Err(FormatError::UnknownSuite(suite.to_owned()));
        }
        let mut seed = |prefix, field| -> Result<Zeroizing<[u8; SEED_LEN]>, FormatError> {
            let text = lines.field(prefix)?;
            hex::decode(text)
                .map(Zeroizing::new)
                .ok_or(FormatError::BadValue {
                    line: Some(lines.number()),
                    field,
                })
        };
        let ed25519_seed = seed(FIELD_ED25519_SEED, "ed25519 seed")?;
        let ml_dsa_87_seed = seed(FIELD_ML_DSA_87_SEED, "ml-dsa-87 seed")?;
        lines.finish()?;
        Ok(Self {
            ed25519_seed,
            ml_dsa_87_seed,
        })
    }

    /// Writes the secret key file format. The returned text is wiped when
    /// dropped.
    pub fn to_text(&self) -> Zeroizing<String> {
        Zeroizing::new(format!(
            "{MAGIC} {FORMAT_VERSION}\n{FIELD_SUITE}{}\n{FIELD_ED25519_SEED}{}\n{FIELD_ML_DSA_87_SEED}{}\n",
            Suite::Ed25519MlDsa87.name(),
            *Zeroizing::new(hex::encode(&self.ed25519_seed[..])),
            *Zeroizing::new(hex::encode(&self.ml_dsa_87_seed[..])),
        ))
    }

    /// The matching public key.
    pub fn public_key(&self) -> PublicKey {
        let mut ed25519 = [0u8; 32];
        libcrux_ed25519::secret_to_public(&mut ed25519, &self.ed25519_seed);
        let mut pair = ml_dsa_87::generate_key_pair(*self.ml_dsa_87_seed);
        pair.signing_key.as_mut_slice().zeroize();
        PublicKey::new(ed25519, Box::new(*pair.verification_key.as_ref()))
    }

    /// Signs `message` for the purpose `context`.
    ///
    /// ML-DSA signing is hedged with fresh operating-system randomness, as
    /// FIPS 204 recommends.
    pub fn sign(&self, context: &str, message: &[u8]) -> Result<Signature, CryptoError> {
        let mut randomness = Zeroizing::new([0u8; SEED_LEN]);
        fill_random(&mut randomness[..])?;
        self.sign_with_randomness(context, message, *randomness)
    }

    /// Signs with caller-supplied ML-DSA randomness. With all-zero
    /// randomness this is FIPS 204's deterministic variant. Intended for
    /// tests and for reproducing a signature; prefer [`SecretKey::sign`].
    pub fn sign_with_randomness(
        &self,
        context: &str,
        message: &[u8],
        randomness: [u8; SEED_LEN],
    ) -> Result<Signature, CryptoError> {
        let representative = representative(Suite::Ed25519MlDsa87, context, message)
            .ok_or(CryptoError::InvalidContext)?;

        let ed25519 = libcrux_ed25519::sign(&representative, &self.ed25519_seed)
            .map_err(|_| CryptoError::Signing)?;

        let mut pair = ml_dsa_87::generate_key_pair(*self.ml_dsa_87_seed);
        let ml_dsa_87 = ml_dsa_87::sign(
            &pair.signing_key,
            &representative,
            ML_DSA_CONTEXT,
            randomness,
        );
        pair.signing_key.as_mut_slice().zeroize();
        let ml_dsa_87 = ml_dsa_87.map_err(|_| CryptoError::Signing)?;

        Signature::new(
            context,
            self.public_key().key_id(),
            ed25519,
            Box::new(*ml_dsa_87.as_ref()),
        )
        .map_err(|_| CryptoError::InvalidContext)
    }
}

impl fmt::Debug for SecretKey {
    /// Never prints key material.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretKey(<redacted>)")
    }
}

fn fill_random(buffer: &mut [u8]) -> Result<(), CryptoError> {
    getrandom::fill(buffer).map_err(|_| CryptoError::Randomness)
}
