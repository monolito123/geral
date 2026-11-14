// SPDX-License-Identifier: Apache-2.0

//! Hybrid key encapsulation: X25519 (RFC 7748) with ML-KEM-1024 (FIPS 203).
//!
//! The shared secret stays secret as long as either X25519 or ML-KEM-1024
//! holds. This module implements no primitive: X25519 and ML-KEM-1024 come
//! from `libcrux` (the X25519 code from HACL\*), SHA3-256 from `sha3`.
//!
//! # Construction
//!
//! The recipient's public key is an X25519 public key and an ML-KEM-1024
//! encapsulation key. To encapsulate, the sender makes an ephemeral X25519
//! key pair and an ML-KEM-1024 encapsulation, and combines both shared
//! secrets:
//!
//! ```text
//! ss = SHA3-256(ml_kem_ss || x25519_ss || x25519_ct || x25519_pk || label)
//!
//! x25519_ct = the sender's ephemeral X25519 public key
//! x25519_pk = the recipient's X25519 public key
//! label     = "NottOS-hybrid-kem-v1"
//! ```
//!
//! This is the combiner shape of both IETF drafts for hybrid ML-KEM
//! (X-Wing and Composite ML-KEM): the classical ciphertext and public key
//! are hashed in because X25519 alone does not bind them; the ML-KEM
//! ciphertext is not, as in both drafts. Neither draft defines ML-KEM-1024
//! with X25519, so the label is NottOS's own (`docs/adr/0012`).
//!
//! The ciphertext is `x25519_ct || ml_kem_ct` (1,600 bytes). A shared X25519
//! secret of all zeros, which a low-order point produces, is refused.

use libcrux_ml_kem::mlkem1024::{self, MlKem1024Ciphertext, MlKem1024PublicKey};
use sha3::{Digest as _, Sha3_256};
use std::fmt;
use zeroize::{Zeroize as _, Zeroizing};

/// Length of an X25519 key or shared secret.
pub const X25519_LEN: usize = 32;
/// Length of an ML-KEM-1024 encapsulation key (FIPS 203).
pub const ML_KEM_1024_PUBLIC_KEY_LEN: usize = 1568;
/// Length of an ML-KEM-1024 ciphertext (FIPS 203).
pub const ML_KEM_1024_CIPHERTEXT_LEN: usize = 1568;
/// Length of a hybrid public key.
pub const PUBLIC_KEY_LEN: usize = X25519_LEN + ML_KEM_1024_PUBLIC_KEY_LEN;
/// Length of a hybrid ciphertext.
pub const CIPHERTEXT_LEN: usize = X25519_LEN + ML_KEM_1024_CIPHERTEXT_LEN;
/// Length of the shared secret.
pub const SHARED_SECRET_LEN: usize = 32;
/// Length of the ML-KEM-1024 key-generation seed (`d || z`).
pub const ML_KEM_SEED_LEN: usize = 64;

/// Last input of the combiner.
pub const LABEL: &[u8] = b"NottOS-hybrid-kem-v1";

/// A shared secret. Wiped when dropped.
pub type SharedSecret = Zeroizing<[u8; SHARED_SECRET_LEN]>;

/// Why an encapsulation or decapsulation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KemError {
    /// The operating system did not provide randomness.
    Randomness,
    /// The ML-KEM-1024 encapsulation key fails the FIPS 203 input check, or
    /// a key or ciphertext has the wrong length.
    InvalidInput,
    /// An X25519 public key or ciphertext is a low-order point: the X25519
    /// shared secret would be all zeros.
    LowOrderPoint,
}

impl fmt::Display for KemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Randomness => "the operating system provided no randomness",
            Self::InvalidInput => "malformed public key or ciphertext",
            Self::LowOrderPoint => "X25519 low-order point",
        })
    }
}

impl std::error::Error for KemError {}

/// A hybrid public key.
#[derive(Clone, PartialEq, Eq)]
pub struct KemPublicKey {
    x25519: [u8; X25519_LEN],
    ml_kem: Box<[u8; ML_KEM_1024_PUBLIC_KEY_LEN]>,
}

impl KemPublicKey {
    /// Reads `x25519 || ml_kem`. The ML-KEM key must pass the FIPS 203
    /// encapsulation-key check.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, KemError> {
        if bytes.len() != PUBLIC_KEY_LEN {
            return Err(KemError::InvalidInput);
        }
        let (x25519, ml_kem) = bytes.split_at(X25519_LEN);
        let x25519: [u8; X25519_LEN] = x25519.try_into().map_err(|_| KemError::InvalidInput)?;
        let ml_kem: Box<[u8; ML_KEM_1024_PUBLIC_KEY_LEN]> = ml_kem
            .to_vec()
            .into_boxed_slice()
            .try_into()
            .map_err(|_| KemError::InvalidInput)?;
        if !mlkem1024::validate_public_key(&MlKem1024PublicKey::from(*ml_kem)) {
            return Err(KemError::InvalidInput);
        }
        Ok(Self { x25519, ml_kem })
    }

    /// `x25519 || ml_kem`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.x25519.to_vec();
        bytes.extend_from_slice(&self.ml_kem[..]);
        bytes
    }

    /// The X25519 half.
    pub fn x25519(&self) -> &[u8; X25519_LEN] {
        &self.x25519
    }

    /// The ML-KEM-1024 half.
    pub fn ml_kem(&self) -> &[u8; ML_KEM_1024_PUBLIC_KEY_LEN] {
        &self.ml_kem
    }
}

impl fmt::Debug for KemPublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "KemPublicKey({})",
            nott_format::Sha256Digest::of_bytes(&self.to_bytes())
        )
    }
}

/// A hybrid ciphertext.
#[derive(Clone, PartialEq, Eq)]
pub struct KemCiphertext {
    x25519: [u8; X25519_LEN],
    ml_kem: Box<[u8; ML_KEM_1024_CIPHERTEXT_LEN]>,
}

impl KemCiphertext {
    /// Reads `x25519_ct || ml_kem_ct`.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, KemError> {
        if bytes.len() != CIPHERTEXT_LEN {
            return Err(KemError::InvalidInput);
        }
        let (x25519, ml_kem) = bytes.split_at(X25519_LEN);
        Ok(Self {
            x25519: x25519.try_into().map_err(|_| KemError::InvalidInput)?,
            ml_kem: ml_kem
                .to_vec()
                .into_boxed_slice()
                .try_into()
                .map_err(|_| KemError::InvalidInput)?,
        })
    }

    /// `x25519_ct || ml_kem_ct`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.x25519.to_vec();
        bytes.extend_from_slice(&self.ml_kem[..]);
        bytes
    }
}

impl fmt::Debug for KemCiphertext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "KemCiphertext({})",
            nott_format::Sha256Digest::of_bytes(&self.to_bytes())
        )
    }
}

/// A hybrid secret key: an X25519 secret and an ML-KEM-1024 seed. Wiped
/// when dropped.
pub struct KemSecretKey {
    x25519: Zeroizing<[u8; X25519_LEN]>,
    ml_kem_seed: Zeroizing<[u8; ML_KEM_SEED_LEN]>,
}

impl KemSecretKey {
    /// Creates a new key from operating-system randomness.
    pub fn generate() -> Result<Self, KemError> {
        let mut x25519 = Zeroizing::new([0u8; X25519_LEN]);
        let mut ml_kem_seed = Zeroizing::new([0u8; ML_KEM_SEED_LEN]);
        fill_random(&mut x25519[..])?;
        fill_random(&mut ml_kem_seed[..])?;
        Ok(Self {
            x25519,
            ml_kem_seed,
        })
    }

    /// Rebuilds a key from its X25519 secret and ML-KEM seed `d || z`.
    pub fn from_seeds(x25519: [u8; X25519_LEN], ml_kem_seed: [u8; ML_KEM_SEED_LEN]) -> Self {
        Self {
            x25519: Zeroizing::new(x25519),
            ml_kem_seed: Zeroizing::new(ml_kem_seed),
        }
    }

    /// The matching public key.
    pub fn public_key(&self) -> KemPublicKey {
        let mut x25519 = [0u8; X25519_LEN];
        libcrux_curve25519::secret_to_public(&mut x25519, &self.x25519);
        let (private, public) = mlkem1024::generate_key_pair(*self.ml_kem_seed).into_parts();
        let mut private: [u8; mlkem1024::MlKem1024PrivateKey::len()] = private.into();
        private.zeroize();
        KemPublicKey {
            x25519,
            ml_kem: Box::new(public.into()),
        }
    }

    /// Recovers the shared secret from a ciphertext.
    ///
    /// A ciphertext whose ML-KEM half was altered still decapsulates, to an
    /// unrelated secret (FIPS 203 implicit rejection); the caller notices
    /// when the secret fails to open what it protects.
    pub fn decapsulate(&self, ciphertext: &KemCiphertext) -> Result<SharedSecret, KemError> {
        let public = self.public_key();
        let mut x25519_ss = Zeroizing::new([0u8; X25519_LEN]);
        libcrux_curve25519::ecdh(&mut x25519_ss, &ciphertext.x25519, &self.x25519)
            .map_err(|_| KemError::LowOrderPoint)?;
        let (private, _public) = mlkem1024::generate_key_pair(*self.ml_kem_seed).into_parts();
        let ml_kem_ss = Zeroizing::new(mlkem1024::decapsulate(
            &private,
            &MlKem1024Ciphertext::from(*ciphertext.ml_kem),
        ));
        let mut private: [u8; mlkem1024::MlKem1024PrivateKey::len()] = private.into();
        private.zeroize();
        Ok(combine(
            &ml_kem_ss,
            &x25519_ss,
            &ciphertext.x25519,
            &public.x25519,
        ))
    }
}

impl fmt::Debug for KemSecretKey {
    /// Never prints key material.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KemSecretKey(<redacted>)")
    }
}

/// Makes a shared secret for `recipient`, with fresh operating-system
/// randomness.
pub fn encapsulate(recipient: &KemPublicKey) -> Result<(KemCiphertext, SharedSecret), KemError> {
    let mut ephemeral = Zeroizing::new([0u8; X25519_LEN]);
    let mut m = Zeroizing::new([0u8; 32]);
    fill_random(&mut ephemeral[..])?;
    fill_random(&mut m[..])?;
    encapsulate_with_randomness(recipient, *ephemeral, *m)
}

/// Encapsulates with caller-supplied randomness: the ephemeral X25519
/// secret and the ML-KEM message `m`. For tests and reproduction only;
/// reusing randomness destroys security. Prefer [`encapsulate`].
pub fn encapsulate_with_randomness(
    recipient: &KemPublicKey,
    ephemeral_x25519: [u8; X25519_LEN],
    ml_kem_m: [u8; 32],
) -> Result<(KemCiphertext, SharedSecret), KemError> {
    let ephemeral_x25519 = Zeroizing::new(ephemeral_x25519);
    let mut x25519_ct = [0u8; X25519_LEN];
    libcrux_curve25519::secret_to_public(&mut x25519_ct, &ephemeral_x25519);
    let mut x25519_ss = Zeroizing::new([0u8; X25519_LEN]);
    libcrux_curve25519::ecdh(&mut x25519_ss, &recipient.x25519, &ephemeral_x25519)
        .map_err(|_| KemError::LowOrderPoint)?;
    let (ml_kem_ct, ml_kem_ss) =
        mlkem1024::encapsulate(&MlKem1024PublicKey::from(*recipient.ml_kem), ml_kem_m);
    let ml_kem_ss = Zeroizing::new(ml_kem_ss);
    let secret = combine(&ml_kem_ss, &x25519_ss, &x25519_ct, &recipient.x25519);
    let ciphertext = KemCiphertext {
        x25519: x25519_ct,
        ml_kem: Box::new(ml_kem_ct.into()),
    };
    Ok((ciphertext, secret))
}

/// The combiner documented at the top of this module.
fn combine(
    ml_kem_ss: &[u8; 32],
    x25519_ss: &[u8; X25519_LEN],
    x25519_ct: &[u8; X25519_LEN],
    x25519_pk: &[u8; X25519_LEN],
) -> SharedSecret {
    let mut hasher = Sha3_256::new();
    hasher.update(ml_kem_ss);
    hasher.update(x25519_ss);
    hasher.update(x25519_ct);
    hasher.update(x25519_pk);
    hasher.update(LABEL);
    Zeroizing::new(hasher.finalize().into())
}

fn fill_random(buffer: &mut [u8]) -> Result<(), KemError> {
    getrandom::fill(buffer).map_err(|_| KemError::Randomness)
}
