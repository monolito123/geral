// SPDX-License-Identifier: Apache-2.0

//! Hybrid key encapsulation: standard vectors for both halves, agreement
//! with second implementations, and the behaviour of the combination.

use libcrux_ml_kem::mlkem1024::{
    self, MlKem1024Ciphertext, MlKem1024PrivateKey, MlKem1024PublicKey,
};
use nott_crypto::kem::{
    encapsulate, encapsulate_with_randomness, KemCiphertext, KemError, KemPublicKey, KemSecretKey,
    CIPHERTEXT_LEN, LABEL, PUBLIC_KEY_LEN, X25519_LEN,
};
use nott_format::{hex, Sha256Digest};
use sha3::{Digest as _, Sha3_256};
use std::collections::BTreeMap;

type Fields = BTreeMap<String, String>;

fn load(name: &str) -> Vec<(String, Fields)> {
    let path = format!("{}/tests/vectors/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
    let mut tests: Vec<(String, Fields)> = Vec::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(id) = line
            .strip_prefix("[test ")
            .and_then(|rest| rest.strip_suffix(']'))
        {
            tests.push((id.to_owned(), Fields::new()));
            continue;
        }
        let (key, value) = line.split_once(" = ").expect("a `key = value` line");
        let (_, fields) = tests.last_mut().expect("a field belongs to a test");
        fields.insert(key.to_owned(), value.to_owned());
    }
    tests
}

fn bytes(fields: &Fields, key: &str) -> Vec<u8> {
    let text = &fields[key];
    let mut out = vec![0u8; text.len() / 2];
    assert!(hex::decode_into(text, &mut out), "field {key} is not hex");
    out
}

fn array<const N: usize>(fields: &Fields, key: &str) -> [u8; N] {
    bytes(fields, key)
        .try_into()
        .unwrap_or_else(|_| panic!("field {key} is not {N} bytes"))
}

fn h<const N: usize>(text: &str) -> [u8; N] {
    hex::decode(text).expect("valid hex")
}

fn sha256_hex(data: &[u8]) -> String {
    Sha256Digest::of_bytes(data).to_hex()
}

// --- X25519, RFC 7748 ------------------------------------------------------

#[test]
fn x25519_rfc7748_section_5_2() {
    let vectors = [
        (
            "a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4",
            "e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c",
            "c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552",
        ),
        (
            "4b66e9d4d1b4673c5ad22691957d6af5c11b6421e0ea01d42ca4169e7918ba0d",
            "e5210f12786811d3f4b7959d0538ae2c31dbe7106fc03c3efc4cd549c715a493",
            "95cbde9476e8907d7aade45cb4b873f88b595a68799fa152e6f8f7647aac7957",
        ),
    ];
    for (scalar, u, expected) in vectors {
        let mut out = [0u8; 32];
        assert!(libcrux_curve25519::ecdh(&mut out, &h(u), &h(scalar)).is_ok());
        assert_eq!(out, h::<32>(expected));
    }
}

#[test]
fn x25519_rfc7748_section_6_1() {
    let alice = h::<32>("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
    let bob = h::<32>("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb");
    let (mut alice_public, mut bob_public) = ([0u8; 32], [0u8; 32]);
    libcrux_curve25519::secret_to_public(&mut alice_public, &alice);
    libcrux_curve25519::secret_to_public(&mut bob_public, &bob);
    assert_eq!(
        alice_public,
        h::<32>("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a")
    );
    assert_eq!(
        bob_public,
        h::<32>("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f")
    );
    let (mut at_alice, mut at_bob) = ([0u8; 32], [0u8; 32]);
    assert!(libcrux_curve25519::ecdh(&mut at_alice, &bob_public, &alice).is_ok());
    assert!(libcrux_curve25519::ecdh(&mut at_bob, &alice_public, &bob).is_ok());
    let shared = h::<32>("4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742");
    assert_eq!((at_alice, at_bob), (shared, shared));
}

#[test]
fn x25519_refuses_low_order_points() {
    // The all-zero u-coordinate and u = 1 are points of small order.
    let scalar = [0x42u8; 32];
    for point in [[0u8; 32], {
        let mut one = [0u8; 32];
        one[0] = 1;
        one
    }] {
        let mut out = [0u8; 32];
        assert!(libcrux_curve25519::ecdh(&mut out, &point, &scalar).is_err());
    }
}

// --- ML-KEM-1024, NIST ACVP ------------------------------------------------

#[test]
fn ml_kem_1024_key_generation_acvp() {
    let tests = load("ml-kem-1024-keygen.txt");
    assert_eq!(tests.len(), 25);
    for (id, fields) in tests {
        let mut seed = bytes(&fields, "d");
        seed.extend_from_slice(&bytes(&fields, "z"));
        let pair = mlkem1024::generate_key_pair(seed.try_into().expect("64 bytes"));
        assert_eq!(sha256_hex(pair.pk()), fields["ek-sha256"], "test {id}: ek");
        assert_eq!(sha256_hex(pair.sk()), fields["dk-sha256"], "test {id}: dk");
    }
}

#[test]
fn ml_kem_1024_encapsulation_acvp() {
    let tests = load("ml-kem-1024-encap.txt");
    assert_eq!(tests.len(), 4);
    for (id, fields) in tests {
        let public = MlKem1024PublicKey::from(array::<1568>(&fields, "ek"));
        let (ciphertext, secret) = mlkem1024::encapsulate(&public, array(&fields, "m"));
        assert_eq!(
            sha256_hex(ciphertext.as_slice()),
            fields["c-sha256"],
            "test {id}"
        );
        assert_eq!(secret.to_vec(), bytes(&fields, "k"), "test {id}");
    }
}

#[test]
fn ml_kem_1024_decapsulation_acvp() {
    let tests = load("ml-kem-1024-decap.txt");
    assert_eq!(tests.len(), 4);
    let mut rejected = 0;
    for (id, fields) in tests {
        let private = MlKem1024PrivateKey::from(array::<3168>(&fields, "dk"));
        let ciphertext = MlKem1024Ciphertext::from(array::<1568>(&fields, "c"));
        let secret = mlkem1024::decapsulate(&private, &ciphertext);
        assert_eq!(
            secret.to_vec(),
            bytes(&fields, "k"),
            "test {id} ({})",
            fields["reason"]
        );
        rejected += usize::from(fields["reason"] == "modified ciphertext");
    }
    assert_eq!(rejected, 2, "two vectors exercise implicit rejection");
}

#[test]
fn ml_kem_1024_encapsulation_key_check_acvp() {
    let tests = load("ml-kem-1024-ek-check.txt");
    assert_eq!(tests.len(), 10);
    let mut x25519 = vec![0x09u8];
    x25519.resize(X25519_LEN, 0);
    for (id, fields) in tests {
        let expected = fields["passed"] == "true";
        let ek = array::<1568>(&fields, "ek");
        assert_eq!(
            mlkem1024::validate_public_key(&MlKem1024PublicKey::from(ek)),
            expected,
            "test {id} ({})",
            fields["reason"]
        );
        // The hybrid public key applies the same check.
        let mut hybrid = x25519.clone();
        hybrid.extend_from_slice(&ek);
        assert_eq!(
            KemPublicKey::from_bytes(&hybrid).is_ok(),
            expected,
            "test {id}"
        );
    }
}

// --- second implementations ------------------------------------------------

mod cross {
    use super::*;
    use ml_kem::array::Array;
    use ml_kem::kem::Decapsulate as _;
    use ml_kem::{DecapsulationKey, EncapsulationKey, MlKem1024, Seed, B32};

    const SEED: [u8; 64] = [0x5c; 64];

    #[test]
    fn ml_kem_1024_agrees_with_rustcrypto() {
        let ours = mlkem1024::generate_key_pair(SEED);
        let theirs = DecapsulationKey::<MlKem1024>::from_seed(Seed::from(SEED));
        let their_public = theirs.encapsulation_key();
        let their_public_bytes: Vec<u8> = ml_kem::KeyExport::to_bytes(their_public).to_vec();
        assert_eq!(
            ours.pk().to_vec(),
            their_public_bytes,
            "same encapsulation key"
        );

        for m in [[0u8; 32], [0xa7; 32]] {
            // Same randomness, same ciphertext and secret.
            let (our_ct, our_ss) = mlkem1024::encapsulate(ours.public_key(), m);
            let (their_ct, their_ss) = their_public.encapsulate_deterministic(&B32::from(m));
            assert_eq!(our_ct.as_slice().to_vec(), their_ct.to_vec());
            assert_eq!(our_ss.to_vec(), their_ss.to_vec());
            // Each decapsulates the other's ciphertext.
            assert_eq!(theirs.decapsulate(&their_ct).to_vec(), our_ss.to_vec());
            let their_ct_as_ours = MlKem1024Ciphertext::from(
                <[u8; 1568]>::try_from(their_ct.as_slice()).expect("ciphertext length"),
            );
            assert_eq!(
                mlkem1024::decapsulate(ours.private_key(), &their_ct_as_ours).to_vec(),
                their_ss.to_vec()
            );
            // A modified ciphertext gives the same implicit-rejection secret
            // in both.
            let mut damaged = our_ct.as_slice().to_owned();
            damaged[100] ^= 1;
            let ours_rejected =
                mlkem1024::decapsulate(ours.private_key(), &MlKem1024Ciphertext::from(damaged));
            let theirs_rejected = theirs.decapsulate(&Array::from(damaged));
            assert_eq!(ours_rejected.to_vec(), theirs_rejected.to_vec());
            assert_ne!(ours_rejected.to_vec(), our_ss.to_vec());
        }
        let _ = EncapsulationKey::<MlKem1024>::new;
    }

    #[test]
    fn x25519_agrees_with_dalek() {
        for (a, b) in [([0x11u8; 32], [0x22u8; 32]), ([0xf0; 32], [0x0f; 32])] {
            let (mut a_pub, mut b_pub) = ([0u8; 32], [0u8; 32]);
            libcrux_curve25519::secret_to_public(&mut a_pub, &a);
            libcrux_curve25519::secret_to_public(&mut b_pub, &b);
            let dalek_a = x25519_dalek::StaticSecret::from(a);
            let dalek_b = x25519_dalek::StaticSecret::from(b);
            assert_eq!(x25519_dalek::PublicKey::from(&dalek_a).to_bytes(), a_pub);
            let mut ours = [0u8; 32];
            assert!(libcrux_curve25519::ecdh(&mut ours, &b_pub, &a).is_ok());
            let theirs = dalek_a.diffie_hellman(&x25519_dalek::PublicKey::from(&dalek_b));
            assert_eq!(theirs.to_bytes(), ours);
        }
    }

    /// The hybrid secret equals the combiner rebuilt in the test from the
    /// documented formula, with both halves computed by the second
    /// implementations.
    #[test]
    fn hybrid_matches_the_documented_combiner() {
        let x25519_secret = [0x31u8; 32];
        let key = KemSecretKey::from_seeds(x25519_secret, SEED);
        let public = key.public_key();
        let ephemeral = [0x77u8; 32];
        let m = [0x99u8; 32];
        let (ciphertext, secret) =
            encapsulate_with_randomness(&public, ephemeral, m).expect("encapsulates");
        let ct = ciphertext.to_bytes();

        let dalek_ephemeral = x25519_dalek::StaticSecret::from(ephemeral);
        let x25519_ct = x25519_dalek::PublicKey::from(&dalek_ephemeral).to_bytes();
        assert_eq!(ct[..32], x25519_ct);
        let x25519_pk =
            x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from(x25519_secret));
        let x25519_ss = dalek_ephemeral.diffie_hellman(&x25519_pk).to_bytes();
        let theirs = DecapsulationKey::<MlKem1024>::from_seed(Seed::from(SEED));
        let (ml_kem_ct, ml_kem_ss) = theirs
            .encapsulation_key()
            .encapsulate_deterministic(&B32::from(m));
        assert_eq!(ct[32..], ml_kem_ct.to_vec()[..]);

        let mut hasher = Sha3_256::new();
        hasher.update(ml_kem_ss);
        hasher.update(x25519_ss);
        hasher.update(x25519_ct);
        hasher.update(x25519_pk.to_bytes());
        hasher.update(b"NottOS-hybrid-kem-v1");
        let expected: [u8; 32] = hasher.finalize().into();
        assert_eq!(*secret, expected);
        assert_eq!(LABEL, b"NottOS-hybrid-kem-v1");
    }
}

// --- the hybrid KEM --------------------------------------------------------

fn key(tag: u8) -> KemSecretKey {
    KemSecretKey::from_seeds([tag; 32], [tag.wrapping_add(1); 64])
}

#[test]
fn round_trip_with_fresh_randomness() {
    let key = KemSecretKey::generate().expect("randomness");
    let public = key.public_key();
    let (ciphertext, secret) = encapsulate(&public).expect("encapsulates");
    assert_eq!(ciphertext.to_bytes().len(), CIPHERTEXT_LEN);
    assert_eq!(
        *key.decapsulate(&ciphertext).expect("decapsulates"),
        *secret
    );
    // Two encapsulations to the same key differ.
    let (other_ciphertext, other_secret) = encapsulate(&public).expect("encapsulates");
    assert_ne!(other_ciphertext, ciphertext);
    assert_ne!(*other_secret, *secret);
}

#[test]
fn keys_and_ciphertexts_survive_their_byte_forms() {
    let key = key(1);
    let public = KemPublicKey::from_bytes(&key.public_key().to_bytes()).expect("valid key");
    assert_eq!(public.to_bytes().len(), PUBLIC_KEY_LEN);
    let (ciphertext, secret) = encapsulate_with_randomness(&public, [2; 32], [3; 32]).expect("ok");
    let parsed = KemCiphertext::from_bytes(&ciphertext.to_bytes()).expect("valid ciphertext");
    assert_eq!(*key.decapsulate(&parsed).expect("ok"), *secret);
    assert_eq!(
        KemCiphertext::from_bytes(&[0u8; CIPHERTEXT_LEN - 1]),
        Err(KemError::InvalidInput)
    );
    assert_eq!(
        KemPublicKey::from_bytes(&[0u8; PUBLIC_KEY_LEN + 1]).map(|_| ()),
        Err(KemError::InvalidInput)
    );
}

#[test]
fn either_half_altered_changes_the_secret() {
    let key = key(1);
    let public = key.public_key();
    let (ciphertext, secret) = encapsulate_with_randomness(&public, [2; 32], [3; 32]).expect("ok");
    let bytes = ciphertext.to_bytes();
    // X25519 half (byte 5) and ML-KEM half (bytes 40 and the last).
    for position in [5, 40, CIPHERTEXT_LEN - 1] {
        let mut damaged = bytes.clone();
        damaged[position] ^= 0x20;
        let damaged = KemCiphertext::from_bytes(&damaged).expect("well-formed");
        match key.decapsulate(&damaged) {
            Ok(other) => assert_ne!(*other, *secret, "position {position}"),
            Err(error) => assert_eq!(error, KemError::LowOrderPoint, "position {position}"),
        }
    }
    // The wrong recipient gets a different secret.
    assert_ne!(*self::key(9).decapsulate(&ciphertext).expect("ok"), *secret);
}

#[test]
fn low_order_points_are_refused() {
    let key = key(1);
    let public = key.public_key();
    let (ciphertext, _) = encapsulate_with_randomness(&public, [2; 32], [3; 32]).expect("ok");
    let mut bytes = ciphertext.to_bytes();
    bytes[..32].fill(0);
    let low = KemCiphertext::from_bytes(&bytes).expect("well-formed");
    assert_eq!(
        key.decapsulate(&low).map(|_| ()),
        Err(KemError::LowOrderPoint)
    );

    let mut recipient = public.to_bytes();
    recipient[..32].fill(0);
    let recipient = KemPublicKey::from_bytes(&recipient).expect("ML-KEM half is valid");
    assert_eq!(
        encapsulate_with_randomness(&recipient, [2; 32], [3; 32]).map(|_| ()),
        Err(KemError::LowOrderPoint)
    );
}

#[test]
fn the_classical_key_is_bound_into_the_secret() {
    // Same ML-KEM key, same randomness, different X25519 recipient key: the
    // secrets differ even though the ML-KEM halves are identical.
    let a = KemSecretKey::from_seeds([1; 32], [5; 64]);
    let b = KemSecretKey::from_seeds([2; 32], [5; 64]);
    let (ct_a, ss_a) = encapsulate_with_randomness(&a.public_key(), [7; 32], [8; 32]).expect("ok");
    let (ct_b, ss_b) = encapsulate_with_randomness(&b.public_key(), [7; 32], [8; 32]).expect("ok");
    assert_eq!(ct_a.to_bytes()[32..], ct_b.to_bytes()[32..]);
    assert_ne!(*ss_a, *ss_b);
}

#[test]
fn debug_output_shows_no_key_material() {
    assert_eq!(format!("{:?}", key(0xab)), "KemSecretKey(<redacted>)");
}

/// Pins the whole construction. A change here means data encrypted by
/// earlier versions can no longer be opened.
#[test]
fn pinned_test_vector() {
    let key = KemSecretKey::from_seeds([0x01; 32], [0x02; 64]);
    let public = key.public_key();
    let (ciphertext, secret) =
        encapsulate_with_randomness(&public, [0x03; 32], [0x04; 32]).expect("ok");
    assert_eq!(
        sha256_hex(&public.to_bytes()),
        "80de2284fa3067626956349b8d1d3a610aeb62f5d5e3ad40eb6a553153d21a87"
    );
    assert_eq!(
        sha256_hex(&ciphertext.to_bytes()),
        "905393977e24a53debd836afc7f0d2869c94e7fbbd12943decf6f2cf365c2bec"
    );
    assert_eq!(
        hex::encode(&secret[..]),
        "188bd116853f785492f591622112fa498326f76ef42ec7847f1e792edc05dbcd"
    );
}
