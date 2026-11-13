// SPDX-License-Identifier: Apache-2.0

//! SHA-256 digests in canonical lower-case hexadecimal form.
//!
//! The hash itself comes from the `sha2` crate. Nothing here implements a
//! cryptographic primitive.

use crate::hex;
use sha2::{Digest as _, Sha256};
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Length of a SHA-256 digest in bytes.
pub const SHA256_LEN: usize = 32;

const READ_BUFFER_LEN: usize = 64 * 1024;

/// A SHA-256 digest.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Sha256Digest([u8; SHA256_LEN]);

impl Sha256Digest {
    /// Wraps raw digest bytes.
    pub fn from_bytes(bytes: [u8; SHA256_LEN]) -> Self {
        Self(bytes)
    }

    /// Returns the raw digest bytes.
    pub fn as_bytes(&self) -> &[u8; SHA256_LEN] {
        &self.0
    }

    /// Hashes a byte slice.
    pub fn of_bytes(data: &[u8]) -> Self {
        Self(Sha256::digest(data).into())
    }

    /// Hashes everything a reader yields. Returns the digest and the number
    /// of bytes read.
    pub fn of_reader<R: Read>(reader: &mut R) -> io::Result<(Self, u64)> {
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; READ_BUFFER_LEN];
        let mut total: u64 = 0;
        loop {
            let count = match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            };
            hasher.update(&buffer[..count]);
            total += count as u64;
        }
        Ok((Self(hasher.finalize().into()), total))
    }

    /// Hashes a file. Returns the digest and the file length in bytes.
    pub fn of_file(path: &Path) -> io::Result<(Self, u64)> {
        Self::of_reader(&mut File::open(path)?)
    }

    /// Parses exactly 64 lower-case hexadecimal characters.
    ///
    /// Upper-case digits are rejected so that a digest has one textual form.
    pub fn from_hex(text: &str) -> Option<Self> {
        hex::decode(text).map(Self)
    }

    /// Formats the digest as 64 lower-case hexadecimal characters.
    pub fn to_hex(&self) -> String {
        hex::encode(&self.0)
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for Sha256Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sha256Digest({})", self.to_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test vectors from FIPS 180-4 examples.
    const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn known_answers() {
        assert_eq!(Sha256Digest::of_bytes(b"").to_hex(), EMPTY);
        assert_eq!(Sha256Digest::of_bytes(b"abc").to_hex(), ABC);
    }

    #[test]
    fn hex_round_trip() {
        let digest = Sha256Digest::from_hex(ABC).expect("valid hex");
        assert_eq!(digest.to_hex(), ABC);
        assert_eq!(digest.to_string(), ABC);
        assert_eq!(digest, Sha256Digest::of_bytes(b"abc"));
    }

    #[test]
    fn hex_is_strict() {
        assert!(Sha256Digest::from_hex(&ABC.to_uppercase()).is_none());
        assert!(Sha256Digest::from_hex(&ABC[..63]).is_none());
        assert!(Sha256Digest::from_hex(&format!("{ABC}0")).is_none());
        assert!(Sha256Digest::from_hex(&format!("{}g", &ABC[..63])).is_none());
        assert!(Sha256Digest::from_hex(&format!(" {}", &ABC[..63])).is_none());
        assert!(Sha256Digest::from_hex("").is_none());
    }

    #[test]
    fn reader_spans_several_buffers() {
        let data: Vec<u8> = (0..(READ_BUFFER_LEN * 3 + 17))
            .map(|i| (i % 251) as u8)
            .collect();
        let (digest, length) = Sha256Digest::of_reader(&mut data.as_slice()).expect("read");
        assert_eq!(length, data.len() as u64);
        assert_eq!(digest, Sha256Digest::of_bytes(&data));
    }

    #[test]
    fn reader_of_nothing() {
        let (digest, length) = Sha256Digest::of_reader(&mut io::empty()).expect("read");
        assert_eq!(length, 0);
        assert_eq!(digest.to_hex(), EMPTY);
    }
}
