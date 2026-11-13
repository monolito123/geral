// SPDX-License-Identifier: Apache-2.0

//! The random-number interface `fips205` expects (`rand_core` 0.6), fed with
//! bytes chosen by the caller.
//!
//! NottOS draws randomness from the operating system itself and hands
//! exactly the bytes a signature needs to the implementation through this
//! adapter. That keeps one source of randomness for every algorithm and
//! makes signing reproducible in tests.

use rand_core::{CryptoRng, Error, RngCore};
use std::num::NonZeroU32;

/// Yields the given bytes once, then fails.
pub(crate) struct GivenBytes<'a> {
    bytes: &'a [u8],
}

impl<'a> GivenBytes<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
}

/// Error code reported when more bytes are requested than were given.
const EXHAUSTED: u32 = Error::CUSTOM_START;

impl RngCore for GivenBytes<'_> {
    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0u8; 4];
        self.fill_bytes(&mut bytes);
        u32::from_le_bytes(bytes)
    }

    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0u8; 8];
        self.fill_bytes(&mut bytes);
        u64::from_le_bytes(bytes)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        // `fips205` only calls `try_fill_bytes`. Reaching this with too few
        // bytes is a programming error; continuing with predictable bytes
        // would be worse than stopping.
        if self.try_fill_bytes(dest).is_err() {
            panic!("GivenBytes: more random bytes requested than provided");
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        if dest.len() > self.bytes.len() {
            let code = NonZeroU32::new(EXHAUSTED).ok_or_else(|| Error::from(NonZeroU32::MIN))?;
            return Err(Error::from(code));
        }
        let (taken, rest) = self.bytes.split_at(dest.len());
        dest.copy_from_slice(taken);
        self.bytes = rest;
        Ok(())
    }
}

/// The bytes come from the operating system's generator, or from a test
/// that chose them deliberately.
impl CryptoRng for GivenBytes<'_> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yields_exactly_the_given_bytes() {
        let source = [1u8, 2, 3, 4, 5];
        let mut rng = GivenBytes::new(&source);
        let mut first = [0u8; 2];
        let mut second = [0u8; 3];
        assert!(rng.try_fill_bytes(&mut first).is_ok());
        assert!(rng.try_fill_bytes(&mut second).is_ok());
        assert_eq!((first, second), ([1, 2], [3, 4, 5]));
        assert!(rng.try_fill_bytes(&mut [0u8; 1]).is_err());
        assert!(rng.try_fill_bytes(&mut []).is_ok());
    }

    #[test]
    #[should_panic(expected = "more random bytes requested than provided")]
    fn refuses_to_invent_bytes() {
        GivenBytes::new(&[0u8; 3]).fill_bytes(&mut [0u8; 4]);
    }
}
