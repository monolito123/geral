// SPDX-License-Identifier: Apache-2.0

//! Behaviour of the hybrid signature as a whole, for every suite.

use nott_crypto::{
    context, verify, CryptoError, PublicKey, SecretKey, Signature, Suite, VerifyError,
};
use nott_format::Sha256Digest;
use std::sync::OnceLock;

const MESSAGE: &[u8] = b"nottos-build-manifest 1\n...";

fn seed_len(suite: Suite) -> usize {
    match suite {
        Suite::Ed25519MlDsa87 => 32,
        Suite::Ed25519SlhDsaSha2_256s => 96,
    }
}

fn key(suite: Suite, tag: u8) -> SecretKey {
    SecretKey::from_seeds(
        suite,
        [tag; 32],
        &vec![tag.wrapping_add(1); seed_len(suite)],
    )
    .expect("valid seeds")
}

/// Signs with fixed randomness so that tests are repeatable.
fn sign(key: &SecretKey, context: &str, message: &[u8]) -> Signature {
    key.sign_with_randomness(context, message, [0u8; 32])
        .expect("signs")
}

/// Keys and signatures shared by the tests. SLH-DSA signing takes about a
/// second, so each suite's signatures are made once.
struct Fixture {
    suite: Suite,
    public: PublicKey,
    /// Over `MESSAGE`.
    good: Signature,
    /// Same key, over another message.
    other_message: Signature,
    /// Another key, over `MESSAGE`.
    stranger: Signature,
}

fn fixture(suite: Suite) -> &'static Fixture {
    static FIXTURES: [OnceLock<Fixture>; 2] = [OnceLock::new(), OnceLock::new()];
    let slot = match suite {
        Suite::Ed25519MlDsa87 => &FIXTURES[0],
        Suite::Ed25519SlhDsaSha2_256s => &FIXTURES[1],
    };
    slot.get_or_init(|| {
        let key = key(suite, 1);
        Fixture {
            suite,
            public: key.public_key(),
            good: sign(&key, context::BUILD_MANIFEST, MESSAGE),
            other_message: sign(&key, context::BUILD_MANIFEST, b"another message"),
            stranger: sign(&self::key(suite, 9), context::BUILD_MANIFEST, MESSAGE),
        }
    })
}

#[test]
fn a_fresh_key_signs_and_verifies() {
    for suite in Suite::ALL {
        let key = SecretKey::generate(suite).expect("randomness available");
        assert_eq!(key.suite(), suite);
        let public = key.public_key();
        let signature = key.sign(context::BUILD_MANIFEST, MESSAGE).expect("signs");
        assert_eq!(
            verify(&public, context::BUILD_MANIFEST, MESSAGE, &signature),
            Ok(())
        );
        assert_eq!(signature.key_id(), public.key_id());
        assert_eq!(signature.suite(), suite);

        let other = SecretKey::generate(suite).expect("randomness available");
        assert_ne!(other.public_key().key_id(), public.key_id());
    }
}

#[test]
fn signatures_survive_their_file_formats() {
    for suite in Suite::ALL {
        let fixture = fixture(suite);
        let public = PublicKey::parse(&fixture.public.to_string()).expect("key parses");
        let signature = Signature::parse(&fixture.good.to_string()).expect("signature parses");
        assert_eq!(
            verify(&public, context::BUILD_MANIFEST, MESSAGE, &signature),
            Ok(())
        );
    }
}

#[test]
fn rejects_wrong_message_context_and_key() {
    for suite in Suite::ALL {
        let Fixture { public, good, .. } = fixture(suite);
        let mut altered = MESSAGE.to_vec();
        altered[0] ^= 1;
        assert_eq!(
            verify(public, context::BUILD_MANIFEST, &altered, good),
            Err(VerifyError::BadSignature)
        );
        assert_eq!(
            verify(public, context::BUILD_MANIFEST, b"", good),
            Err(VerifyError::BadSignature)
        );
        assert_eq!(
            verify(public, context::UPDATE_METADATA, MESSAGE, good),
            Err(VerifyError::ContextMismatch)
        );
        assert_eq!(
            verify(
                &key(suite, 7).public_key(),
                context::BUILD_MANIFEST,
                MESSAGE,
                good
            ),
            Err(VerifyError::KeyMismatch)
        );
    }
}

#[test]
fn a_signature_does_not_verify_under_another_suite() {
    let ml_dsa = fixture(Suite::Ed25519MlDsa87);
    let slh_dsa = fixture(Suite::Ed25519SlhDsaSha2_256s);
    assert_eq!(
        verify(
            &slh_dsa.public,
            context::BUILD_MANIFEST,
            MESSAGE,
            &ml_dsa.good
        ),
        Err(VerifyError::SuiteMismatch)
    );
    assert_eq!(
        verify(
            &ml_dsa.public,
            context::BUILD_MANIFEST,
            MESSAGE,
            &slh_dsa.good
        ),
        Err(VerifyError::SuiteMismatch)
    );
}

#[test]
fn a_signature_for_one_purpose_fails_for_another_even_if_relabelled() {
    for suite in Suite::ALL {
        let Fixture { public, good, .. } = fixture(suite);
        // An attacker rewrites the context field and keeps the components.
        let relabelled = Signature::new(
            suite,
            context::UPDATE_METADATA,
            good.key_id(),
            *good.ed25519(),
            good.post_quantum().to_vec(),
        )
        .expect("valid fields");
        assert_eq!(
            verify(public, context::UPDATE_METADATA, MESSAGE, &relabelled),
            Err(VerifyError::BadSignature)
        );
    }
}

#[test]
fn both_components_are_required() {
    for suite in Suite::ALL {
        let Fixture {
            public,
            good,
            other_message,
            stranger,
            ..
        } = fixture(suite);
        let rebuild = |ed25519: [u8; 64], post_quantum: &[u8]| {
            Signature::new(
                suite,
                context::BUILD_MANIFEST,
                good.key_id(),
                ed25519,
                post_quantum.to_vec(),
            )
            .expect("valid fields")
        };
        let check =
            |signature: &Signature| verify(public, context::BUILD_MANIFEST, MESSAGE, signature);
        let bad = Err(VerifyError::BadSignature);

        assert_eq!(
            check(&rebuild(*good.ed25519(), good.post_quantum())),
            Ok(())
        );

        // Only the classical half is broken.
        let mut ed25519 = *good.ed25519();
        ed25519[10] ^= 0x40;
        assert_eq!(check(&rebuild(ed25519, good.post_quantum())), bad);

        // Only the post-quantum half is broken, near the start and the end.
        for position in [100, suite.post_quantum_signature_len() - 5] {
            let mut post_quantum = good.post_quantum().to_vec();
            post_quantum[position] ^= 0x40;
            assert_eq!(
                check(&rebuild(*good.ed25519(), &post_quantum)),
                bad,
                "{position}"
            );
        }

        // Each half is valid, but for a different message.
        assert_eq!(
            check(&rebuild(*good.ed25519(), other_message.post_quantum())),
            bad
        );
        assert_eq!(
            check(&rebuild(*other_message.ed25519(), good.post_quantum())),
            bad
        );

        // Each half is valid, but made by a different key.
        assert_eq!(
            check(&rebuild(*good.ed25519(), stranger.post_quantum())),
            bad
        );
        assert_eq!(
            check(&rebuild(*stranger.ed25519(), good.post_quantum())),
            bad
        );
    }
}

/// Checked for suite 1 only: it needs three signatures by one key, which
/// for suite 2 would add seconds without testing anything suite-specific.
#[test]
fn randomness_changes_only_the_post_quantum_half() {
    let key = key(Suite::Ed25519MlDsa87, 1);
    let public = key.public_key();
    let with = |randomness| {
        key.sign_with_randomness(context::BUILD_MANIFEST, MESSAGE, randomness)
            .expect("signs")
    };
    let (first, again, second) = (with([1u8; 32]), with([1u8; 32]), with([2u8; 32]));
    assert_eq!(first, again);
    assert_eq!(first.ed25519(), second.ed25519());
    assert_ne!(first.post_quantum(), second.post_quantum());
    for signature in [&first, &second] {
        assert_eq!(
            verify(&public, context::BUILD_MANIFEST, MESSAGE, signature),
            Ok(())
        );
    }
}

#[test]
fn invalid_contexts_and_seeds_are_refused() {
    for suite in Suite::ALL {
        let key = key(suite, 1);
        for context in ["", "Build Manifest", "x_y"] {
            assert_eq!(
                key.sign(context, MESSAGE).map(|_| ()),
                Err(CryptoError::InvalidContext)
            );
        }
        let wrong = seed_len(suite) + 1;
        assert_eq!(
            SecretKey::from_seeds(suite, [0; 32], &vec![0; wrong]).map(|_| ()),
            Err(CryptoError::InvalidSeed)
        );
    }
}

#[test]
fn secret_key_file_round_trip() {
    let expected = [
        format!(
            "nottos-secret-key 1\nsuite: ed25519+ml-dsa-87\ned25519-seed: {}\nml-dsa-87-seed: {}\n",
            "03".repeat(32),
            "04".repeat(32)
        ),
        format!(
            "nottos-secret-key 1\nsuite: ed25519+slh-dsa-sha2-256s\ned25519-seed: {}\n\
             slh-dsa-sha2-256s-seed: {}\n",
            "03".repeat(32),
            "04".repeat(96)
        ),
    ];
    for (suite, expected) in Suite::ALL.into_iter().zip(expected) {
        let key = key(suite, 3);
        let text = key.to_text();
        assert_eq!(*text, expected);
        let parsed = SecretKey::parse(&text).expect("parses");
        assert_eq!(parsed.public_key(), key.public_key());
        assert_eq!(*parsed.to_text(), *text);

        for bad in [
            text.replacen("ed25519-seed: 03", "ed25519-seed: 3", 1),
            text.replacen(suite.name(), "ed25519", 1),
            text.replacen("nottos-secret-key 1", "nottos-secret-key 2", 1),
            text.replacen("-seed: 0404", "-seed: 04", 1),
            format!("{}extra\n", *text),
            text.trim_end().to_owned(),
        ] {
            assert!(SecretKey::parse(&bad).is_err(), "{bad:?}");
        }
    }
}

#[test]
fn debug_output_shows_no_key_material() {
    for suite in Suite::ALL {
        let shown = format!("{:?}", key(suite, 0xab));
        assert_eq!(shown, format!("SecretKey({}, <redacted>)", suite.name()));
        assert!(!shown.contains("abab"));
    }
}

/// Pins the whole construction for each suite: fixed seeds, fixed
/// randomness, fixed message. If one of these changes, signatures made by
/// earlier versions no longer verify, which must never happen silently.
#[test]
fn pinned_test_vectors() {
    let pinned = [
        (
            Suite::Ed25519MlDsa87,
            vec![0x02; 32],
            "055f1225158b2f81d0ac9791e3bea582116353ea34600d6b9e869eb997942c96",
            "df4e13649733675423d028309b215af6d90c2571f1dcdaabae28085f90c070b9",
        ),
        (
            Suite::Ed25519SlhDsaSha2_256s,
            vec![0x02; 96],
            "49a24fc7b56bf0df7308a6ae7892c8e03fb345bd1644be1037029c1a46043a69",
            "ccedf8d495770d689ab2ff67f46f31a3df1e07267d51414b8184ba3576ed02ef",
        ),
    ];
    let digest = |text: String| Sha256Digest::of_bytes(text.as_bytes()).to_hex();
    for (suite, seed, public_key, signature) in pinned {
        let key = SecretKey::from_seeds(suite, [0x01; 32], &seed).expect("valid seeds");
        let made = key
            .sign_with_randomness(context::BUILD_MANIFEST, b"NottOS test vector 1", [0u8; 32])
            .expect("signs");
        assert_eq!(
            digest(key.public_key().to_string()),
            public_key,
            "{suite:?}"
        );
        assert_eq!(digest(made.to_string()), signature, "{suite:?}");
    }
}

#[test]
fn fixtures_are_consistent() {
    for suite in Suite::ALL {
        let fixture = fixture(suite);
        assert_eq!(fixture.suite, suite);
        assert_eq!(fixture.good.suite(), suite);
        assert_eq!(
            fixture.good.post_quantum().len(),
            suite.post_quantum_signature_len()
        );
    }
}
