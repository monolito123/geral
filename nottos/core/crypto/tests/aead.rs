// SPDX-License-Identifier: Apache-2.0

//! ChaCha20-Poly1305: RFC 8439 vectors, agreement with RustCrypto's
//! `chacha20poly1305` (tests only), and refusal of every altered input.
//!
//! The vectors were extracted programmatically from the RFC text (see
//! `vectors/README.md`).

use chacha20poly1305::aead::{Aead as _, KeyInit as _, Payload};
use nott_crypto::aead::{open, seal, AeadError, KEY_LEN, NONCE_LEN, TAG_LEN};
use nott_crypto::hash::shake256;
use nott_format::hex;

fn bytes(text: &str) -> Vec<u8> {
    let mut out = vec![0u8; text.len() / 2];
    assert!(hex::decode_into(text, &mut out), "bad hex in test");
    out
}

fn array<const N: usize>(text: &str) -> [u8; N] {
    bytes(text).try_into().expect("length")
}

/// Deterministic test input: SHAKE256 of a label.
fn stream(label: &str, len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    shake256(&[b"nott-crypto aead test", label.as_bytes()], &mut out);
    out
}

#[test]
fn rfc8439_section_2_8_2() {
    let key: [u8; KEY_LEN] =
        array("808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f");
    // 32-bit fixed-common part, then the 64-bit IV.
    let nonce: [u8; NONCE_LEN] = array("070000004041424344454647");
    let aad = bytes("50515253c0c1c2c3c4c5c6c7");
    let plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
    let ciphertext = bytes(concat!(
        "d31a8d34648e60db7b86afbc53ef7ec2a4aded51296e08fea9e2b5a736ee62d6",
        "3dbea45e8ca9671282fafb69da92728b1a71de0a9e060b2905d6a5b67ecd3b36",
        "92ddbd7f2d778b8c9803aee328091b58fab324e4fad675945585808b4831d7bc",
        "3ff4def08e4b7a9de576d26586cec64b6116",
    ));
    let tag = bytes("1ae10b594f09e26a7e902ecbd0600691");
    let sealed = seal(&key, &nonce, &aad, plaintext).unwrap();
    assert_eq!(sealed, [ciphertext, tag].concat());
    assert_eq!(
        &open(&key, &nonce, &aad, &sealed).unwrap()[..],
        &plaintext[..]
    );
}

#[test]
fn rfc8439_appendix_a_5() {
    let key: [u8; KEY_LEN] =
        array("1c9240a5eb55d38af333888604f6b5f0473917c1402b80099dca5cbc207075c0");
    let nonce: [u8; NONCE_LEN] = array("000000000102030405060708");
    let aad = bytes("f33388860000000000004e91");
    let ciphertext = bytes(concat!(
        "64a0861575861af460f062c79be643bd5e805cfd345cf389f108670ac76c8cb2",
        "4c6cfc18755d43eea09ee94e382d26b0bdb7b73c321b0100d4f03b7f355894cf",
        "332f830e710b97ce98c8a84abd0b948114ad176e008d33bd60f982b1ff37c855",
        "9797a06ef4f0ef61c186324e2b3506383606907b6a7c02b0f9f6157b53c867e4",
        "b9166c767b804d46a59b5216cde7a4e99040c5a40433225ee282a1b0a06c523e",
        "af4534d7f83fa1155b0047718cbc546a0d072b04b3564eea1b422273f548271a",
        "0bb2316053fa76991955ebd63159434ecebb4e466dae5a1073a6727627097a10",
        "49e617d91d361094fa68f0ff77987130305beaba2eda04df997b714d6c6f2c29",
        "a6ad5cb4022b02709b",
    ));
    let tag = bytes("eead9d67890cbb22392336fea1851f38");
    let plaintext = bytes(concat!(
        "496e7465726e65742d4472616674732061726520647261667420646f63756d65",
        "6e74732076616c696420666f722061206d6178696d756d206f6620736978206d",
        "6f6e74687320616e64206d617920626520757064617465642c207265706c6163",
        "65642c206f72206f62736f6c65746564206279206f7468657220646f63756d65",
        "6e747320617420616e792074696d652e20497420697320696e617070726f7072",
        "6961746520746f2075736520496e7465726e65742d4472616674732061732072",
        "65666572656e6365206d6174657269616c206f7220746f206369746520746865",
        "6d206f74686572207468616e206173202fe2809c776f726b20696e2070726f67",
        "726573732e2fe2809d",
    ));
    let sealed = [ciphertext, tag].concat();
    assert_eq!(
        &open(&key, &nonce, &aad, &sealed).unwrap()[..],
        &plaintext[..]
    );
    assert_eq!(seal(&key, &nonce, &aad, &plaintext).unwrap(), sealed);
}

#[test]
fn agrees_with_a_second_implementation() {
    for (case, len) in [0, 1, 15, 16, 17, 63, 64, 65, 255, 1000, 65_536]
        .into_iter()
        .enumerate()
    {
        let key: [u8; KEY_LEN] = stream(&format!("key {case}"), KEY_LEN).try_into().unwrap();
        let nonce: [u8; NONCE_LEN] = stream(&format!("nonce {case}"), NONCE_LEN)
            .try_into()
            .unwrap();
        let aad = stream(&format!("aad {case}"), case * 5);
        let plaintext = stream(&format!("plaintext {case}"), len);

        let second = chacha20poly1305::ChaCha20Poly1305::new((&key).into());
        let theirs = second
            .encrypt(
                (&nonce).into(),
                Payload {
                    msg: &plaintext,
                    aad: &aad,
                },
            )
            .unwrap();
        let ours = seal(&key, &nonce, &aad, &plaintext).unwrap();
        assert_eq!(ours, theirs, "case {case}");
        assert_eq!(
            &open(&key, &nonce, &aad, &theirs).unwrap()[..],
            &plaintext[..]
        );
        let opened = second
            .decrypt(
                (&nonce).into(),
                Payload {
                    msg: &ours,
                    aad: &aad,
                },
            )
            .unwrap();
        assert_eq!(opened, plaintext);
    }
}

#[test]
fn refuses_every_altered_input() {
    let key: [u8; KEY_LEN] = stream("key", KEY_LEN).try_into().unwrap();
    let nonce: [u8; NONCE_LEN] = stream("nonce", NONCE_LEN).try_into().unwrap();
    let aad = stream("aad", 20);
    let plaintext = stream("plaintext", 24);
    let sealed = seal(&key, &nonce, &aad, &plaintext).unwrap();
    assert_eq!(sealed.len(), plaintext.len() + TAG_LEN);

    for bit in 0..sealed.len() * 8 {
        let mut changed = sealed.clone();
        changed[bit / 8] ^= 1 << (bit % 8);
        assert_eq!(
            open(&key, &nonce, &aad, &changed),
            Err(AeadError::Inauthentic),
            "ciphertext bit {bit}"
        );
    }
    for bit in 0..aad.len() * 8 {
        let mut changed = aad.clone();
        changed[bit / 8] ^= 1 << (bit % 8);
        assert!(
            open(&key, &nonce, &changed, &sealed).is_err(),
            "aad bit {bit}"
        );
    }
    for bit in 0..NONCE_LEN * 8 {
        let mut changed = nonce;
        changed[bit / 8] ^= 1 << (bit % 8);
        assert!(
            open(&key, &changed, &aad, &sealed).is_err(),
            "nonce bit {bit}"
        );
    }
    for bit in 0..KEY_LEN * 8 {
        let mut changed = key;
        changed[bit / 8] ^= 1 << (bit % 8);
        assert!(
            open(&changed, &nonce, &aad, &sealed).is_err(),
            "key bit {bit}"
        );
    }
    assert!(open(&key, &nonce, &aad[..19], &sealed).is_err());
    assert!(open(&key, &nonce, &aad, &sealed[..sealed.len() - 1]).is_err());
    assert!(open(&key, &nonce, &aad, &[sealed.clone(), vec![0]].concat()).is_err());
    assert_eq!(
        open(&key, &nonce, &aad, &sealed[..TAG_LEN - 1]),
        Err(AeadError::Inauthentic)
    );
    assert_eq!(open(&key, &nonce, &aad, &[]), Err(AeadError::Inauthentic));
}

#[test]
fn empty_plaintext_is_a_tag_over_the_associated_data() {
    let key = [7u8; KEY_LEN];
    let nonce = [0u8; NONCE_LEN];
    let sealed = seal(&key, &nonce, b"header", b"").unwrap();
    assert_eq!(sealed.len(), TAG_LEN);
    assert!(open(&key, &nonce, b"header", &sealed).unwrap().is_empty());
    assert!(open(&key, &nonce, b"Header", &sealed).is_err());
}
