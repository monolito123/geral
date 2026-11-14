// SPDX-License-Identifier: Apache-2.0

//! Passphrase stretching: Argon2id, version 0x13 (RFC 9106).
//!
//! The implementation is RustCrypto's `argon2`, with its memory wiped after
//! use. This module accepts any parameters the implementation accepts; what
//! NottOS allows is decided by the caller (`docs/adr/0013-backup-format.md`).

use std::fmt;
use zeroize::Zeroizing;

/// Length of the derived key.
pub const OUTPUT_LEN: usize = 32;

/// Argon2id cost parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Argon2idParams {
    /// Memory, in KiB (RFC 9106: `m`).
    pub memory_kib: u32,
    /// Passes over the memory (RFC 9106: `t`).
    pub iterations: u32,
    /// Lanes (RFC 9106: `p`).
    pub lanes: u32,
}

/// Why a key could not be derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KdfError {
    /// The parameters, passphrase or salt are outside what Argon2 defines
    /// (for example a salt shorter than 8 bytes).
    Parameters,
    /// The memory could not be allocated.
    OutOfMemory,
}

impl fmt::Display for KdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Parameters => "invalid Argon2id parameters",
            Self::OutOfMemory => "not enough memory for Argon2id",
        })
    }
}

impl std::error::Error for KdfError {}

/// Derives a 32-byte key from `passphrase` and `salt`. No secret key and no
/// associated data are used.
pub fn argon2id(
    passphrase: &[u8],
    salt: &[u8],
    params: Argon2idParams,
) -> Result<Zeroizing<[u8; OUTPUT_LEN]>, KdfError> {
    let params = argon2::Params::new(
        params.memory_kib,
        params.iterations,
        params.lanes,
        Some(OUTPUT_LEN),
    )
    .map_err(|_| KdfError::Parameters)?;
    let hasher = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut key = Zeroizing::new([0u8; OUTPUT_LEN]);
    hasher
        .hash_password_into(passphrase, salt, &mut key[..])
        .map_err(|error| match error {
            argon2::Error::OutOfMemory => KdfError::OutOfMemory,
            _ => KdfError::Parameters,
        })?;
    Ok(key)
}
