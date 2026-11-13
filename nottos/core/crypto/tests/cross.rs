// SPDX-License-Identifier: Apache-2.0

//! Cross-checks against a second, independent ML-DSA implementation
//! (RustCrypto `ml-dsa`). The two implementations share no code, so
//! agreement on keys and deterministic signatures, and acceptance of each
//! other's signatures, is evidence that both follow FIPS 204 and that
//! NottOS signatures can be verified by other software.

use libcrux_ml_dsa::ml_dsa_87::{self, MLDSA87Signature, MLDSA87VerificationKey};
use ml_dsa::{EncodedSignature, EncodedVerifyingKey, ExpandedSigningKey, MlDsa87, Seed};
use nott_crypto::{context, SecretKey, Suite};
use sha2::{Digest as _, Sha512};

type OtherSignature = ml_dsa::Signature<MlDsa87>;
type OtherVerifyingKey = ml_dsa::VerifyingKey<MlDsa87>;

fn other_signing_key(seed: [u8; 32]) -> ExpandedSigningKey<MlDsa87> {
    ExpandedSigningKey::<MlDsa87>::from_seed(&Seed::from(seed))
}

fn other_verifying_key(bytes: &[u8]) -> OtherVerifyingKey {
    let encoded = EncodedVerifyingKey::<MlDsa87>::try_from(bytes).expect("key length");
    OtherVerifyingKey::decode(&encoded)
}

fn other_signature(bytes: &[u8]) -> OtherSignature {
    let encoded = EncodedSignature::<MlDsa87>::try_from(bytes).expect("signature length");
    OtherSignature::decode(&encoded).expect("well-formed signature")
}

const SEEDS: [[u8; 32]; 3] = [[0x00; 32], [0x5a; 32], [0xff; 32]];

fn cases() -> Vec<(Vec<u8>, Vec<u8>)> {
    vec![
        (b"".to_vec(), b"".to_vec()),
        (b"NottOS".to_vec(), b"build-manifest".to_vec()),
        (vec![0xa5; 4096], vec![0x3c; 255]),
    ]
}

#[test]
fn same_seed_gives_the_same_key_pair() {
    for seed in SEEDS {
        let ours = ml_dsa_87::generate_key_pair(seed);
        let theirs = other_signing_key(seed).verifying_key().encode();
        assert_eq!(ours.verification_key.as_slice(), theirs.as_slice());
    }
}

#[test]
fn deterministic_signatures_are_identical() {
    for seed in SEEDS {
        let ours = ml_dsa_87::generate_key_pair(seed);
        let theirs = other_signing_key(seed);
        for (message, context) in cases() {
            let our_signature =
                ml_dsa_87::sign(&ours.signing_key, &message, &context, [0u8; 32]).expect("signs");
            let their_signature = theirs
                .sign_deterministic(&message, &context)
                .expect("signs")
                .encode();
            assert_eq!(our_signature.as_slice(), their_signature.as_slice());
        }
    }
}

#[test]
fn each_implementation_accepts_the_other() {
    let seed = SEEDS[1];
    let ours = ml_dsa_87::generate_key_pair(seed);
    let theirs = other_signing_key(seed);
    let their_key = other_verifying_key(ours.verification_key.as_slice());
    for (message, context) in cases() {
        // Ours, hedged with non-zero randomness, checked by theirs.
        let signature =
            ml_dsa_87::sign(&ours.signing_key, &message, &context, [0x77; 32]).expect("signs");
        assert!(their_key.verify_with_context(
            &message,
            &context,
            &other_signature(signature.as_slice())
        ));
        let mut other_message = message.clone();
        other_message.push(1);
        assert!(!their_key.verify_with_context(
            &other_message,
            &context,
            &other_signature(signature.as_slice())
        ));

        // Theirs, checked by ours.
        let signature = theirs
            .sign_deterministic(&message, &context)
            .expect("signs")
            .encode();
        let signature =
            MLDSA87Signature::new(signature.as_slice().try_into().expect("signature length"));
        assert!(ml_dsa_87::verify(&ours.verification_key, &message, &context, &signature).is_ok());
        assert!(
            ml_dsa_87::verify(&ours.verification_key, &other_message, &context, &signature)
                .is_err()
        );
    }
}

/// Rebuilds the representative from the documented construction, without
/// using the library's own function, and checks both components of a NottOS
/// hybrid signature against it: ML-DSA with the second implementation,
/// Ed25519 with a direct call.
#[test]
fn hybrid_signature_matches_the_documented_construction() {
    let key = SecretKey::from_seeds([0x11; 32], [0x22; 32]);
    let public = key.public_key();
    let message = b"the quick brown fox";
    let signature = key.sign(context::BUILD_MANIFEST, message).expect("signs");

    let mut representative = b"NottOS-hybrid-signature-v1\0".to_vec();
    representative.extend_from_slice(&Suite::Ed25519MlDsa87.id().to_be_bytes());
    representative.push(context::BUILD_MANIFEST.len() as u8);
    representative.extend_from_slice(context::BUILD_MANIFEST.as_bytes());
    representative.extend_from_slice(&Sha512::digest(message));

    assert!(other_verifying_key(public.ml_dsa_87()).verify_with_context(
        &representative,
        b"",
        &other_signature(signature.ml_dsa_87())
    ));
    assert!(
        libcrux_ed25519::verify(&representative, public.ed25519(), signature.ed25519()).is_ok()
    );

    // Neither component is a signature on the bare message.
    assert!(
        !other_verifying_key(public.ml_dsa_87()).verify_with_context(
            message,
            b"",
            &other_signature(signature.ml_dsa_87())
        )
    );
    assert!(libcrux_ed25519::verify(message, public.ed25519(), signature.ed25519()).is_err());

    // The public key's ML-DSA half is what the second implementation derives
    // from the same seed.
    let derived = other_signing_key([0x22; 32]).verifying_key().encode();
    assert_eq!(public.ml_dsa_87().as_slice(), derived.as_slice());
    let _ = MLDSA87VerificationKey::new(*public.ml_dsa_87());
}
