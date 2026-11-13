// SPDX-License-Identifier: Apache-2.0

//! Hybrid secret keys.
//!
//! A secret key is a set of seeds: 32 bytes for Ed25519 and, for the
//! post-quantum half, 32 bytes (ML-DSA-87, suite 1) or 96 bytes
//! (SLH-DSA-SHA2-256s, suite 2: `SK.seed || SK.prf || PK.seed`). Working keys
//! are derived from the seeds when needed and wiped afterwards.
//!
//! # Secret key file, format version 1
//!
//! ```text
//! nottos-secret-key 1
//! suite: <suite name>
//! ed25519-seed: <64 hex digits>
//! <post-quantum name>-seed: <hex>
//! ```
//!
//! This file format exists for test and development keys. Release and root
//! keys are meant to live in offline hardware and never appear in this form.

use crate::rng::GivenBytes;
use crate::{representative, CryptoError, PQ_CONTEXT};
use fips205::slh_dsa_sha2_256s;
use fips205::traits::{KeyGen as _, SerDes as _, Signer as _};
use libcrux_ml_dsa::ml_dsa_87;
use nott_format::hex;
use nott_format::signature::{FormatError, PublicKey, Signature, Suite};
use nott_format::text::Lines;
use std::fmt;
use zeroize::{Zeroize as _, Zeroizing};

const ED25519_SEED_LEN: usize = 32;
/// Length of the randomness one signature consumes.
const SIGNING_RANDOMNESS_LEN: usize = 32;
const SLH_N: usize = slh_dsa_sha2_256s::N;

const MAGIC: &str = "nottos-secret-key";
const FORMAT_VERSION: u32 = 1;
const MAX_BYTES: usize = 1024;
const FIELD_SUITE: &str = "suite: ";
const FIELD_ED25519_SEED: &str = "ed25519-seed: ";

fn seed_len(suite: Suite) -> usize {
    match suite {
        Suite::Ed25519MlDsa87 => ml_dsa_87_seed_len(),
        Suite::Ed25519SlhDsaSha2_256s => 3 * SLH_N,
    }
}

const fn ml_dsa_87_seed_len() -> usize {
    libcrux_ml_dsa::KEY_GENERATION_RANDOMNESS_SIZE
}

fn seed_field(suite: Suite) -> &'static str {
    match suite {
        Suite::Ed25519MlDsa87 => "ml-dsa-87-seed: ",
        Suite::Ed25519SlhDsaSha2_256s => "slh-dsa-sha2-256s-seed: ",
    }
}

fn seed_label(suite: Suite) -> &'static str {
    match suite {
        Suite::Ed25519MlDsa87 => "ml-dsa-87 seed",
        Suite::Ed25519SlhDsaSha2_256s => "slh-dsa-sha2-256s seed",
    }
}

/// A hybrid secret key. The seeds are wiped from memory when the value is
/// dropped.
pub struct SecretKey {
    suite: Suite,
    ed25519_seed: Zeroizing<[u8; ED25519_SEED_LEN]>,
    post_quantum_seed: Zeroizing<Vec<u8>>,
}

impl SecretKey {
    /// Creates a new key of the given suite from operating-system
    /// randomness.
    pub fn generate(suite: Suite) -> Result<Self, CryptoError> {
        let mut ed25519_seed = Zeroizing::new([0u8; ED25519_SEED_LEN]);
        fill_random(&mut ed25519_seed[..])?;
        let mut post_quantum_seed = Zeroizing::new(vec![0u8; seed_len(suite)]);
        fill_random(&mut post_quantum_seed)?;
        Ok(Self {
            suite,
            ed25519_seed,
            post_quantum_seed,
        })
    }

    /// Rebuilds a key from its seeds. Fails if the post-quantum seed does
    /// not have the length the suite requires (32 bytes for suite 1, 96 for
    /// suite 2).
    pub fn from_seeds(
        suite: Suite,
        ed25519_seed: [u8; ED25519_SEED_LEN],
        post_quantum_seed: &[u8],
    ) -> Result<Self, CryptoError> {
        if post_quantum_seed.len() != seed_len(suite) {
            return Err(CryptoError::InvalidSeed);
        }
        Ok(Self {
            suite,
            ed25519_seed: Zeroizing::new(ed25519_seed),
            post_quantum_seed: Zeroizing::new(post_quantum_seed.to_vec()),
        })
    }

    /// Parses the secret key file format.
    pub fn parse(text: &str) -> Result<Self, FormatError> {
        let mut lines = Lines::open(text, MAX_BYTES, MAGIC, FORMAT_VERSION)?;
        let name = lines.field(FIELD_SUITE)?;
        let suite =
            Suite::from_name(name).ok_or_else(|| FormatError::UnknownSuite(name.to_owned()))?;
        let field = lines.field(FIELD_ED25519_SEED)?;
        let ed25519_seed = hex::decode(field)
            .map(Zeroizing::new)
            .ok_or(FormatError::BadValue {
                line: Some(lines.number()),
                field: "ed25519 seed",
            })?;
        let field = lines.field(seed_field(suite))?;
        let mut post_quantum_seed = Zeroizing::new(vec![0u8; seed_len(suite)]);
        if !hex::decode_into(field, &mut post_quantum_seed) {
            return Err(FormatError::BadValue {
                line: Some(lines.number()),
                field: seed_label(suite),
            });
        }
        lines.finish()?;
        Ok(Self {
            suite,
            ed25519_seed,
            post_quantum_seed,
        })
    }

    /// Writes the secret key file format. The returned text is wiped when
    /// dropped.
    pub fn to_text(&self) -> Zeroizing<String> {
        let ed25519 = Zeroizing::new(hex::encode(&self.ed25519_seed[..]));
        let post_quantum = Zeroizing::new(hex::encode(&self.post_quantum_seed));
        Zeroizing::new(format!(
            "{MAGIC} {FORMAT_VERSION}\n{FIELD_SUITE}{}\n{FIELD_ED25519_SEED}{}\n{}{}\n",
            self.suite.name(),
            *ed25519,
            seed_field(self.suite),
            *post_quantum,
        ))
    }

    /// The suite of this key.
    pub fn suite(&self) -> Suite {
        self.suite
    }

    /// The matching public key.
    pub fn public_key(&self) -> PublicKey {
        let mut ed25519 = [0u8; 32];
        libcrux_ed25519::secret_to_public(&mut ed25519, &self.ed25519_seed);
        let post_quantum = match self.suite {
            Suite::Ed25519MlDsa87 => {
                let mut pair = ml_dsa_87::generate_key_pair(self.ml_dsa_87_seed());
                pair.signing_key.as_mut_slice().zeroize();
                pair.verification_key.as_slice().to_vec()
            }
            Suite::Ed25519SlhDsaSha2_256s => {
                let (public, _secret) = self.slh_dsa_keys();
                public.into_bytes().to_vec()
            }
        };
        PublicKey::new(self.suite, ed25519, post_quantum)
            .expect("derived keys have the lengths their suite requires")
    }

    /// Signs `message` for the purpose `context`.
    ///
    /// The post-quantum half is hedged with fresh operating-system
    /// randomness, as FIPS 204 and FIPS 205 recommend.
    pub fn sign(&self, context: &str, message: &[u8]) -> Result<Signature, CryptoError> {
        let mut randomness = Zeroizing::new([0u8; SIGNING_RANDOMNESS_LEN]);
        fill_random(&mut randomness[..])?;
        self.sign_with_randomness(context, message, *randomness)
    }

    /// Signs with caller-supplied randomness for the post-quantum half.
    /// Intended for tests and for reproducing a signature; prefer
    /// [`SecretKey::sign`].
    pub fn sign_with_randomness(
        &self,
        context: &str,
        message: &[u8],
        randomness: [u8; SIGNING_RANDOMNESS_LEN],
    ) -> Result<Signature, CryptoError> {
        let representative =
            representative(self.suite, context, message).ok_or(CryptoError::InvalidContext)?;
        let randomness = Zeroizing::new(randomness);

        let ed25519 = libcrux_ed25519::sign(&representative, &self.ed25519_seed)
            .map_err(|_| CryptoError::Signing)?;

        let post_quantum = match self.suite {
            Suite::Ed25519MlDsa87 => {
                let mut pair = ml_dsa_87::generate_key_pair(self.ml_dsa_87_seed());
                let signature =
                    ml_dsa_87::sign(&pair.signing_key, &representative, PQ_CONTEXT, *randomness);
                pair.signing_key.as_mut_slice().zeroize();
                signature
                    .map_err(|_| CryptoError::Signing)?
                    .as_slice()
                    .to_vec()
            }
            Suite::Ed25519SlhDsaSha2_256s => {
                let (_public, secret) = self.slh_dsa_keys();
                let mut rng = GivenBytes::new(&randomness[..SLH_N]);
                secret
                    .try_sign_with_rng(&mut rng, &representative, PQ_CONTEXT, true)
                    .map_err(|_| CryptoError::Signing)?
                    .to_vec()
            }
        };

        Signature::new(
            self.suite,
            context,
            self.public_key().key_id(),
            ed25519,
            post_quantum,
        )
        .map_err(|_| CryptoError::Signing)
    }

    fn ml_dsa_87_seed(&self) -> [u8; ml_dsa_87_seed_len()] {
        self.post_quantum_seed[..]
            .try_into()
            .expect("seed length is checked when the key is built")
    }

    /// Derives the SLH-DSA key pair. `fips205` wipes its private key when
    /// it is dropped.
    fn slh_dsa_keys(&self) -> (slh_dsa_sha2_256s::PublicKey, slh_dsa_sha2_256s::PrivateKey) {
        let part = |index: usize| -> Zeroizing<[u8; SLH_N]> {
            Zeroizing::new(
                self.post_quantum_seed[index * SLH_N..(index + 1) * SLH_N]
                    .try_into()
                    .expect("seed length is checked when the key is built"),
            )
        };
        slh_dsa_sha2_256s::KG::keygen_from_seed(&part(0), &part(1), &part(2))
    }
}

impl fmt::Debug for SecretKey {
    /// Never prints key material.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretKey({}, <redacted>)", self.suite.name())
    }
}

fn fill_random(buffer: &mut [u8]) -> Result<(), CryptoError> {
    getrandom::fill(buffer).map_err(|_| CryptoError::Randomness)
}
