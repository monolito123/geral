// SPDX-License-Identifier: Apache-2.0

//! Behaviour of the hybrid signature as a whole.

use nott_crypto::{context, verify, CryptoError, PublicKey, SecretKey, Signature, VerifyError};
use nott_format::Sha256Digest;

const MESSAGE: &[u8] = b"nottos-build-manifest 1\n...";

fn key(tag: u8) -> SecretKey {
    SecretKey::from_seeds([tag; 32], [tag.wrapping_add(1); 32])
}

/// Signs with fixed randomness so that tests are repeatable.
fn sign(key: &SecretKey, context: &str, message: &[u8]) -> Signature {
    key.sign_with_randomness(context, message, [0u8; 32])
        .expect("signs")
}

#[test]
fn a_fresh_key_signs_and_verifies() {
    let key = SecretKey::generate().expect("randomness available");
    let public = key.public_key();
    let signature = key.sign(context::BUILD_MANIFEST, MESSAGE).expect("signs");
    assert_eq!(
        verify(&public, context::BUILD_MANIFEST, MESSAGE, &signature),
        Ok(())
    );
    assert_eq!(signature.key_id(), public.key_id());
    assert_eq!(signature.context(), context::BUILD_MANIFEST);

    // Two fresh keys differ.
    let other = SecretKey::generate().expect("randomness available");
    assert_ne!(other.public_key().key_id(), public.key_id());
}

#[test]
fn signatures_survive_their_file_formats() {
    let key = key(1);
    let public = PublicKey::parse(&key.public_key().to_string()).expect("key parses");
    let signature = Signature::parse(&sign(&key, context::BUILD_MANIFEST, MESSAGE).to_string())
        .expect("signature parses");
    assert_eq!(
        verify(&public, context::BUILD_MANIFEST, MESSAGE, &signature),
        Ok(())
    );
}

#[test]
fn rejects_wrong_message_context_and_key() {
    let key = key(1);
    let public = key.public_key();
    let signature = sign(&key, context::BUILD_MANIFEST, MESSAGE);

    let mut altered = MESSAGE.to_vec();
    altered[0] ^= 1;
    assert_eq!(
        verify(&public, context::BUILD_MANIFEST, &altered, &signature),
        Err(VerifyError::BadSignature)
    );
    assert_eq!(
        verify(&public, context::BUILD_MANIFEST, b"", &signature),
        Err(VerifyError::BadSignature)
    );
    assert_eq!(
        verify(&public, "update-metadata", MESSAGE, &signature),
        Err(VerifyError::ContextMismatch)
    );
    assert_eq!(
        verify(
            &self::key(7).public_key(),
            context::BUILD_MANIFEST,
            MESSAGE,
            &signature
        ),
        Err(VerifyError::KeyMismatch)
    );
}

#[test]
fn a_signature_for_one_purpose_fails_for_another_even_if_relabelled() {
    let key = key(1);
    let public = key.public_key();
    let signature = sign(&key, context::BUILD_MANIFEST, MESSAGE);
    // An attacker rewrites the context field and keeps the components.
    let relabelled = Signature::new(
        "update-metadata",
        signature.key_id(),
        *signature.ed25519(),
        Box::new(*signature.ml_dsa_87()),
    )
    .expect("valid context");
    assert_eq!(
        verify(&public, "update-metadata", MESSAGE, &relabelled),
        Err(VerifyError::BadSignature)
    );
}

#[test]
fn both_components_are_required() {
    let key = key(1);
    let public = key.public_key();
    let good = sign(&key, context::BUILD_MANIFEST, MESSAGE);
    let rebuild = |ed25519: [u8; 64], ml_dsa_87: Box<[u8; 4627]>| {
        Signature::new(context::BUILD_MANIFEST, good.key_id(), ed25519, ml_dsa_87)
            .expect("valid context")
    };
    let check =
        |signature: &Signature| verify(&public, context::BUILD_MANIFEST, MESSAGE, signature);

    assert_eq!(
        check(&rebuild(*good.ed25519(), Box::new(*good.ml_dsa_87()))),
        Ok(())
    );

    // Only the classical half is broken.
    let mut ed25519 = *good.ed25519();
    ed25519[10] ^= 0x40;
    assert_eq!(
        check(&rebuild(ed25519, Box::new(*good.ml_dsa_87()))),
        Err(VerifyError::BadSignature)
    );

    // Only the post-quantum half is broken.
    let mut ml_dsa_87 = Box::new(*good.ml_dsa_87());
    ml_dsa_87[100] ^= 0x40;
    assert_eq!(
        check(&rebuild(*good.ed25519(), ml_dsa_87)),
        Err(VerifyError::BadSignature)
    );

    // Each half is valid, but for different messages.
    let other = sign(&key, context::BUILD_MANIFEST, b"another message");
    assert_eq!(
        check(&rebuild(*good.ed25519(), Box::new(*other.ml_dsa_87()))),
        Err(VerifyError::BadSignature)
    );
    assert_eq!(
        check(&rebuild(*other.ed25519(), Box::new(*good.ml_dsa_87()))),
        Err(VerifyError::BadSignature)
    );

    // Each half is valid, but made by different keys.
    let stranger = sign(&self::key(9), context::BUILD_MANIFEST, MESSAGE);
    assert_eq!(
        check(&rebuild(*good.ed25519(), Box::new(*stranger.ml_dsa_87()))),
        Err(VerifyError::BadSignature)
    );
    assert_eq!(
        check(&rebuild(*stranger.ed25519(), Box::new(*good.ml_dsa_87()))),
        Err(VerifyError::BadSignature)
    );
}

#[test]
fn randomness_changes_only_the_post_quantum_half() {
    let key = key(1);
    let public = key.public_key();
    let first = key
        .sign_with_randomness(context::BUILD_MANIFEST, MESSAGE, [1u8; 32])
        .expect("signs");
    let again = key
        .sign_with_randomness(context::BUILD_MANIFEST, MESSAGE, [1u8; 32])
        .expect("signs");
    let second = key
        .sign_with_randomness(context::BUILD_MANIFEST, MESSAGE, [2u8; 32])
        .expect("signs");
    assert_eq!(first, again);
    assert_eq!(first.ed25519(), second.ed25519());
    assert_ne!(first.ml_dsa_87()[..], second.ml_dsa_87()[..]);
    for signature in [&first, &second] {
        assert_eq!(
            verify(&public, context::BUILD_MANIFEST, MESSAGE, signature),
            Ok(())
        );
    }
}

#[test]
fn invalid_contexts_are_refused_when_signing() {
    let key = key(1);
    for context in ["", "Build Manifest", "x_y"] {
        assert_eq!(
            key.sign(context, MESSAGE).map(|_| ()),
            Err(CryptoError::InvalidContext)
        );
    }
}

#[test]
fn secret_key_file_round_trip() {
    let key = key(3);
    let text = key.to_text();
    assert_eq!(
        *text,
        format!(
            "nottos-secret-key 1\nsuite: ed25519+ml-dsa-87\ned25519-seed: {}\nml-dsa-87-seed: {}\n",
            "03".repeat(32),
            "04".repeat(32)
        )
    );
    let parsed = SecretKey::parse(&text).expect("parses");
    assert_eq!(parsed.public_key(), key.public_key());
    assert_eq!(*parsed.to_text(), *text);

    for bad in [
        text.replacen("ed25519-seed: 03", "ed25519-seed: 3", 1),
        text.replacen("ed25519+ml-dsa-87", "ed25519", 1),
        text.replacen("nottos-secret-key 1", "nottos-secret-key 2", 1),
        format!("{}extra\n", *text),
        text.trim_end().to_owned(),
    ] {
        assert!(SecretKey::parse(&bad).is_err(), "{bad:?}");
    }
}

#[test]
fn debug_output_shows_no_key_material() {
    let shown = format!("{:?}", key(0xab));
    assert_eq!(shown, "SecretKey(<redacted>)");
}

/// Pins the whole construction: fixed seeds, fixed randomness, fixed
/// message. If this changes, signatures made by earlier versions no longer
/// verify, which must never happen silently.
#[test]
fn pinned_test_vector() {
    let key = SecretKey::from_seeds([0x01; 32], [0x02; 32]);
    let signature = key
        .sign_with_randomness(context::BUILD_MANIFEST, b"NottOS test vector 1", [0u8; 32])
        .expect("signs");
    let digest = |text: String| Sha256Digest::of_bytes(text.as_bytes()).to_hex();
    assert_eq!(
        digest(key.public_key().to_string()),
        "055f1225158b2f81d0ac9791e3bea582116353ea34600d6b9e869eb997942c96"
    );
    assert_eq!(
        digest(signature.to_string()),
        "df4e13649733675423d028309b215af6d90c2571f1dcdaabae28085f90c070b9"
    );
}
