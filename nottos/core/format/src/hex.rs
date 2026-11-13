// SPDX-License-Identifier: Apache-2.0

//! Lower-case hexadecimal, strict: one textual form per byte string.

/// Encodes bytes as lower-case hexadecimal.
pub fn encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(DIGITS[usize::from(byte >> 4)] as char);
        text.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    text
}

/// Decodes exactly `out.len()` bytes. Returns `false`, leaving `out` in an
/// unspecified state, unless `text` is exactly twice that many lower-case
/// hexadecimal digits.
pub fn decode_into(text: &str, out: &mut [u8]) -> bool {
    let text = text.as_bytes();
    if text.len() != out.len() * 2 {
        return false;
    }
    for (byte, pair) in out.iter_mut().zip(text.chunks_exact(2)) {
        let (Some(high), Some(low)) = (value(pair[0]), value(pair[1])) else {
            return false;
        };
        *byte = (high << 4) | low;
    }
    true
}

/// Decodes exactly `N` bytes.
pub fn decode<const N: usize>(text: &str) -> Option<[u8; N]> {
    let mut bytes = [0u8; N];
    decode_into(text, &mut bytes).then_some(bytes)
}

/// Decodes exactly `N` bytes onto the heap, for arrays too large to pass
/// around on the stack comfortably.
pub fn decode_boxed<const N: usize>(text: &str) -> Option<Box<[u8; N]>> {
    let mut bytes = vec![0u8; N].into_boxed_slice();
    if !decode_into(text, &mut bytes) {
        return None;
    }
    bytes.try_into().ok()
}

fn value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let bytes: Vec<u8> = (0..=255).collect();
        let text = encode(&bytes);
        assert_eq!(text.len(), 512);
        assert!(text.starts_with("000102") && text.ends_with("fdfeff"));
        assert_eq!(decode::<256>(&text).map(Vec::from), Some(bytes.clone()));
        assert_eq!(
            decode_boxed::<256>(&text).map(|boxed| boxed.to_vec()),
            Some(bytes)
        );
    }

    #[test]
    fn strict() {
        assert_eq!(decode::<2>("00ff"), Some([0x00, 0xff]));
        assert_eq!(decode::<0>(""), Some([]));
        for bad in ["00FF", "00f", "00fff", "00fg", " 0ff", "0x00"] {
            assert_eq!(decode::<2>(bad), None, "{bad:?}");
            assert!(decode_boxed::<2>(bad).is_none(), "{bad:?}");
        }
    }
}
