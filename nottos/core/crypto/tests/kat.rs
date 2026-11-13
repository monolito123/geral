// SPDX-License-Identifier: Apache-2.0

//! Known-answer tests for the algorithms `nott-crypto` builds on, called
//! the way `nott-crypto` calls them. Sources are listed in
//! `tests/vectors/README.md`.

use libcrux_ml_dsa::ml_dsa_87::{
    self, MLDSA87Signature, MLDSA87SigningKey, MLDSA87VerificationKey,
};
use nott_format::{hex, Sha256Digest};
use std::collections::BTreeMap;

type Fields = BTreeMap<String, String>;

/// Reads a vector file: `[test N]` headers followed by `key = value` lines.
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

fn sha256_hex(data: &[u8]) -> String {
    Sha256Digest::of_bytes(data).to_hex()
}

/// RFC 8032, section 7.1, tests 1 to 3: secret key, public key, message,
/// signature.
const ED25519_RFC8032: [[&str; 4]; 3] = [
    [
        "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
        "",
        "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e06522490155\
         5fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
    ],
    [
        "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
        "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
        "72",
        "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da\
         085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
    ],
    [
        "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
        "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
        "af82",
        "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac\
         18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a",
    ],
];

#[test]
fn ed25519_rfc8032() {
    for [secret, public, message, signature] in ED25519_RFC8032 {
        let secret: [u8; 32] = hex::decode(secret).expect("hex");
        let public: [u8; 32] = hex::decode(public).expect("hex");
        let signature: [u8; 64] = hex::decode(signature).expect("hex");
        let mut message_bytes = vec![0u8; message.len() / 2];
        assert!(hex::decode_into(message, &mut message_bytes));

        let mut derived = [0u8; 32];
        libcrux_ed25519::secret_to_public(&mut derived, &secret);
        assert_eq!(derived, public);
        assert_eq!(
            libcrux_ed25519::sign(&message_bytes, &secret).expect("signs"),
            signature
        );
        assert!(libcrux_ed25519::verify(&message_bytes, &public, &signature).is_ok());

        let mut other_message = message_bytes.clone();
        other_message.push(0);
        assert!(libcrux_ed25519::verify(&other_message, &public, &signature).is_err());
        let mut other_signature = signature;
        other_signature[0] ^= 1;
        assert!(libcrux_ed25519::verify(&message_bytes, &public, &other_signature).is_err());
    }
}

#[test]
fn ml_dsa_87_key_generation_acvp() {
    let tests = load("ml-dsa-87-keygen.txt");
    assert_eq!(tests.len(), 25);
    for (id, fields) in tests {
        let pair = ml_dsa_87::generate_key_pair(array(&fields, "seed"));
        assert_eq!(
            sha256_hex(pair.verification_key.as_slice()),
            fields["pk-sha256"],
            "test {id}: public key"
        );
        assert_eq!(
            sha256_hex(pair.signing_key.as_slice()),
            fields["sk-sha256"],
            "test {id}: private key"
        );
    }
}

#[test]
fn ml_dsa_87_deterministic_signing_acvp() {
    let tests = load("ml-dsa-87-siggen.txt");
    assert_eq!(tests.len(), 2);
    for (id, fields) in tests {
        let signing_key = MLDSA87SigningKey::new(array(&fields, "sk"));
        // The deterministic variant of FIPS 204 uses all-zero randomness.
        let signature = ml_dsa_87::sign(
            &signing_key,
            &bytes(&fields, "message"),
            &bytes(&fields, "context"),
            [0u8; 32],
        )
        .unwrap_or_else(|_| panic!("test {id}: signing failed"));
        assert_eq!(
            sha256_hex(signature.as_slice()),
            fields["signature-sha256"],
            "test {id}"
        );
    }
}

#[test]
fn ml_dsa_87_verification_acvp() {
    let tests = load("ml-dsa-87-sigver.txt");
    assert_eq!(tests.len(), 6);
    let mut accepted = 0;
    for (id, fields) in tests {
        let expected = fields["passed"] == "true";
        let outcome = ml_dsa_87::verify(
            &MLDSA87VerificationKey::new(array(&fields, "pk")),
            &bytes(&fields, "message"),
            &bytes(&fields, "context"),
            &MLDSA87Signature::new(array(&fields, "signature")),
        );
        assert_eq!(
            outcome.is_ok(),
            expected,
            "test {id} ({})",
            fields["reason"]
        );
        accepted += usize::from(expected);
    }
    assert_eq!(accepted, 2, "two of the six vectors are valid signatures");
}

mod slh_dsa_sha2_256s_acvp {
    //! FIPS 205 vectors, run against `fips205` the way `nott-crypto` uses
    //! it: seeds in, the external (pure) interface for signing and
    //! verifying.

    use super::{array, bytes, load, sha256_hex};
    use fips205::slh_dsa_sha2_256s::{PrivateKey, PublicKey, KG, N, SIG_LEN, SK_LEN};
    use fips205::traits::{KeyGen as _, SerDes as _, Signer as _, Verifier as _};

    #[test]
    fn key_generation() {
        let tests = load("slh-dsa-sha2-256s-keygen.txt");
        assert_eq!(tests.len(), 10);
        for (id, fields) in tests {
            let (public, secret) = KG::keygen_from_seed(
                &array::<N>(&fields, "sk-seed"),
                &array::<N>(&fields, "sk-prf"),
                &array::<N>(&fields, "pk-seed"),
            );
            assert_eq!(
                public.into_bytes().to_vec(),
                bytes(&fields, "pk"),
                "test {id}"
            );
            assert_eq!(
                sha256_hex(&secret.into_bytes()),
                fields["sk-sha256"],
                "test {id}: private key"
            );
        }
    }

    #[test]
    fn deterministic_signing() {
        let tests = load("slh-dsa-sha2-256s-siggen.txt");
        assert_eq!(tests.len(), 2);
        for (id, fields) in tests {
            let secret = PrivateKey::try_from_bytes(array::<SK_LEN>(&fields, "sk"))
                .unwrap_or_else(|_| panic!("test {id}: private key"));
            // FIPS 205's deterministic variant draws no randomness, so the
            // generator must never be asked.
            let mut no_randomness = super::NoRandomness;
            let signature = secret
                .try_sign_with_rng(
                    &mut no_randomness,
                    &bytes(&fields, "message"),
                    &bytes(&fields, "context"),
                    false,
                )
                .unwrap_or_else(|_| panic!("test {id}: signing failed"));
            assert_eq!(
                sha256_hex(&signature),
                fields["signature-sha256"],
                "test {id}"
            );
        }
    }

    #[test]
    fn verification() {
        let tests = load("slh-dsa-sha2-256s-sigver.txt");
        assert_eq!(tests.len(), 8);
        let (mut accepted, mut wrong_length) = (0, 0);
        for (id, fields) in tests {
            let expected = fields["passed"] == "true";
            if let Some(length) = fields.get("signature-length") {
                // Too short or too long: the fixed-size interface cannot
                // even be called, which is the rejection.
                let length: usize = length.parse().expect("a number");
                assert_ne!(length, SIG_LEN, "test {id}");
                assert!(!expected);
                wrong_length += 1;
                continue;
            }
            let public = PublicKey::try_from_bytes(array(&fields, "pk"))
                .unwrap_or_else(|_| panic!("test {id}: public key"));
            let outcome = public.verify(
                &bytes(&fields, "message"),
                &array::<SIG_LEN>(&fields, "signature"),
                &bytes(&fields, "context"),
            );
            assert_eq!(outcome, expected, "test {id} ({})", fields["reason"]);
            accepted += usize::from(expected);
        }
        assert_eq!((accepted, wrong_length), (2, 2));
    }
}

/// A generator that fails every request, for code paths that must not draw
/// randomness.
struct NoRandomness;

impl rand_core::RngCore for NoRandomness {
    fn next_u32(&mut self) -> u32 {
        panic!("randomness requested")
    }
    fn next_u64(&mut self) -> u64 {
        panic!("randomness requested")
    }
    fn fill_bytes(&mut self, _: &mut [u8]) {
        panic!("randomness requested")
    }
    fn try_fill_bytes(&mut self, _: &mut [u8]) -> Result<(), rand_core::Error> {
        panic!("randomness requested")
    }
}

impl rand_core::CryptoRng for NoRandomness {}
