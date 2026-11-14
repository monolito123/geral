// SPDX-License-Identifier: Apache-2.0

//! Authenticated encryption: ChaCha20-Poly1305 (RFC 8439).
//!
//! The implementation is `libcrux-chacha20poly1305`, whose code comes from
//! HACL\*. It checks the tag, in constant time, before it decrypts anything.
//!
//! A key must never be used twice with the same nonce. Callers in NottOS
//! use each key for one message, or for one stream whose nonces count up
//! (`docs/adr/0013-backup-format.md`).

use std::fmt;
use zeroize::Zeroizing;

/// Length of a key.
pub const KEY_LEN: usize = libcrux_chacha20poly1305::KEY_LEN;
/// Length of a nonce.
pub const NONCE_LEN: usize = libcrux_chacha20poly1305::NONCE_LEN;
/// Length of the authentication tag appended to every ciphertext.
pub const TAG_LEN: usize = libcrux_chacha20poly1305::TAG_LEN;

/// Why sealing or opening failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AeadError {
    /// An input is longer than the implementation accepts (2³² − 1 bytes).
    TooLarge,
    /// The ciphertext, nonce or associated data is not what was sealed under
    /// this key.
    Inauthentic,
}

impl fmt::Display for AeadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TooLarge => "input too large for authenticated encryption",
            Self::Inauthentic => "authentication failed",
        })
    }
}

impl std::error::Error for AeadError {}

/// Encrypts `plaintext` and authenticates it together with `aad`. Returns
/// the ciphertext followed by the tag.
pub fn seal(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, AeadError> {
    let mut sealed = vec![0u8; plaintext.len() + TAG_LEN];
    libcrux_chacha20poly1305::encrypt(key, plaintext, &mut sealed, aad, nonce)
        .map_err(|_| AeadError::TooLarge)?;
    Ok(sealed)
}

/// Checks the tag of `sealed` (ciphertext followed by tag) against `aad`,
/// and only then decrypts. The plaintext is wiped when dropped.
pub fn open(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    sealed: &[u8],
) -> Result<Zeroizing<Vec<u8>>, AeadError> {
    let Some(plaintext_len) = sealed.len().checked_sub(TAG_LEN) else {
        return Err(AeadError::Inauthentic);
    };
    let mut plaintext = Zeroizing::new(vec![0u8; plaintext_len]);
    match libcrux_chacha20poly1305::decrypt(key, &mut plaintext, sealed, aad, nonce) {
        Ok(_) => Ok(plaintext),
        Err(libcrux_chacha20poly1305::AeadError::InvalidCiphertext) => Err(AeadError::Inauthentic),
        Err(_) => Err(AeadError::TooLarge),
    }
}
