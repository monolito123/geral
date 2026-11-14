// SPDX-License-Identifier: Apache-2.0

//! SHA3-256 and SHAKE256 (FIPS 202), from RustCrypto's `sha3`, for key
//! derivation in the constructions built on this crate.
//!
//! Every caller in NottOS hashes a fixed-length label followed by
//! fixed-length inputs, so plain concatenation is unambiguous.

use sha3::digest::{ExtendableOutput as _, Update as _, XofReader as _};
use sha3::{Digest as _, Sha3_256, Shake256};
use zeroize::Zeroizing;

/// SHA3-256 of the concatenation of `parts`, wiped when dropped.
pub fn sha3_256(parts: &[&[u8]]) -> Zeroizing<[u8; 32]> {
    let mut hasher = Sha3_256::new();
    for part in parts {
        sha3::Digest::update(&mut hasher, part);
    }
    Zeroizing::new(hasher.finalize().into())
}

/// Fills `out` with SHAKE256 output for the concatenation of `parts`.
pub fn shake256(parts: &[&[u8]], out: &mut [u8]) {
    let mut hasher = Shake256::default();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize_xof().read(out);
}
