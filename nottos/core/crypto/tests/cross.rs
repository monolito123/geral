// SPDX-License-Identifier: Apache-2.0

//! Cross-checks against second, independent implementations: RustCrypto
//! `ml-dsa` for ML-DSA-87 and RustCrypto `slh-dsa` for SLH-DSA-SHA2-256s.
//! Neither shares code with the implementation `nott-crypto` uses, so
//! agreement on keys and deterministic signatures, and acceptance of each
//! other's signatures, is evidence that both follow the standards and that
//! NottOS signatures can be verified by other software.

use nott_crypto::{context, SecretKey, Suite};
use sha2::{Digest as _, Sha512};

/// Rebuilds the representative from the construction documented in
/// `nott-crypto`, without using the library's own function.
fn documented_representative(suite: Suite, context: &str, message: &[u8]) -> Vec<u8> {
    let mut representative = b"NottOS-hybrid-signature-v1\0".to_vec();
    representative.extend_from_slice(&suite.id().to_be_bytes());
    representative.push(u8::try_from(context.len()).expect("short context"));
    representative.extend_from_slice(context.as_bytes());
    representative.extend_from_slice(&Sha512::digest(message));
    representative
}

fn cases() -> Vec<(Vec<u8>, Vec<u8>)> {
    vec![
        (b"".to_vec(), b"".to_vec()),
        (b"NottOS".to_vec(), b"build-manifest".to_vec()),
        (vec![0xa5; 4096], vec![0x3c; 255]),
    ]
}

mod ml_dsa_87 {
    use super::{cases, context, documented_representative, SecretKey, Suite};
    use libcrux_ml_dsa::ml_dsa_87::{self as ours, MLDSA87Signature};
    use ml_dsa::{EncodedSignature, EncodedVerifyingKey, ExpandedSigningKey, MlDsa87, Seed};

    type TheirSignature = ml_dsa::Signature<MlDsa87>;
    type TheirVerifyingKey = ml_dsa::VerifyingKey<MlDsa87>;

    fn their_signing_key(seed: [u8; 32]) -> ExpandedSigningKey<MlDsa87> {
        ExpandedSigningKey::<MlDsa87>::from_seed(&Seed::from(seed))
    }

    fn their_verifying_key(bytes: &[u8]) -> TheirVerifyingKey {
        let encoded = EncodedVerifyingKey::<MlDsa87>::try_from(bytes).expect("key length");
        TheirVerifyingKey::decode(&encoded)
    }

    fn their_signature(bytes: &[u8]) -> TheirSignature {
        let encoded = EncodedSignature::<MlDsa87>::try_from(bytes).expect("signature length");
        TheirSignature::decode(&encoded).expect("well-formed signature")
    }

    const SEEDS: [[u8; 32]; 3] = [[0x00; 32], [0x5a; 32], [0xff; 32]];

    #[test]
    fn same_seed_gives_the_same_key_pair() {
        for seed in SEEDS {
            let pair = ours::generate_key_pair(seed);
            let theirs = their_signing_key(seed).verifying_key().encode();
            assert_eq!(pair.verification_key.as_slice(), theirs.as_slice());
        }
    }

    #[test]
    fn deterministic_signatures_are_identical() {
        for seed in SEEDS {
            let pair = ours::generate_key_pair(seed);
            let theirs = their_signing_key(seed);
            for (message, context) in cases() {
                let our_signature =
                    ours::sign(&pair.signing_key, &message, &context, [0u8; 32]).expect("signs");
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
        let pair = ours::generate_key_pair(seed);
        let theirs = their_signing_key(seed);
        let their_key = their_verifying_key(pair.verification_key.as_slice());
        for (message, context) in cases() {
            let mut other_message = message.clone();
            other_message.push(1);

            // Ours, hedged with non-zero randomness, checked by theirs.
            let signature =
                ours::sign(&pair.signing_key, &message, &context, [0x77; 32]).expect("signs");
            let decoded = their_signature(signature.as_slice());
            assert!(their_key.verify_with_context(&message, &context, &decoded));
            assert!(!their_key.verify_with_context(&other_message, &context, &decoded));

            // Theirs, checked by ours.
            let signature = theirs
                .sign_deterministic(&message, &context)
                .expect("signs")
                .encode();
            let signature =
                MLDSA87Signature::new(signature.as_slice().try_into().expect("signature length"));
            let key = &pair.verification_key;
            assert!(ours::verify(key, &message, &context, &signature).is_ok());
            assert!(ours::verify(key, &other_message, &context, &signature).is_err());
        }
    }

    /// Both halves of a NottOS hybrid signature verify against a
    /// representative rebuilt in the test, the ML-DSA half with the second
    /// implementation.
    #[test]
    fn hybrid_signature_matches_the_documented_construction() {
        let suite = Suite::Ed25519MlDsa87;
        let key = SecretKey::from_seeds(suite, [0x11; 32], &[0x22; 32]).expect("valid seeds");
        let public = key.public_key();
        let message = b"the quick brown fox";
        let signature = key.sign(context::BUILD_MANIFEST, message).expect("signs");
        let representative = documented_representative(suite, context::BUILD_MANIFEST, message);

        let their_key = their_verifying_key(public.post_quantum());
        let decoded = their_signature(signature.post_quantum());
        assert!(their_key.verify_with_context(&representative, b"", &decoded));
        assert!(
            libcrux_ed25519::verify(&representative, public.ed25519(), signature.ed25519()).is_ok()
        );

        // Neither half is a signature on the bare message.
        assert!(!their_key.verify_with_context(message, b"", &decoded));
        assert!(libcrux_ed25519::verify(message, public.ed25519(), signature.ed25519()).is_err());

        // The public key's post-quantum half is what the second
        // implementation derives from the same seed.
        let derived = their_signing_key([0x22; 32]).verifying_key().encode();
        assert_eq!(public.post_quantum(), derived.as_slice());
    }
}

mod slh_dsa_sha2_256s {
    use super::NoRandomness;
    use super::{context, documented_representative, SecretKey, Suite};
    use fips205::slh_dsa_sha2_256s::{PublicKey as OurPublicKey, KG, SIG_LEN};
    use fips205::traits::{KeyGen as _, SerDes as _, Signer as _, Verifier as _};
    use slh_dsa::{Sha2_256s, Signature as TheirSignature, SigningKey, VerifyingKey};

    const SK_SEED: [u8; 32] = [0x31; 32];
    const SK_PRF: [u8; 32] = [0x32; 32];
    const PK_SEED: [u8; 32] = [0x33; 32];
    const MESSAGE: &[u8] = b"NottOS root metadata";
    const CONTEXT: &[u8] = b"cross-check";

    fn theirs() -> SigningKey<Sha2_256s> {
        SigningKey::<Sha2_256s>::slh_keygen_internal(&SK_SEED, &SK_PRF, &PK_SEED)
    }

    fn their_verifying_key(bytes: &[u8]) -> VerifyingKey<Sha2_256s> {
        VerifyingKey::<Sha2_256s>::try_from(bytes).expect("key length")
    }

    fn their_signature(bytes: &[u8]) -> TheirSignature<Sha2_256s> {
        TheirSignature::<Sha2_256s>::try_from(bytes).expect("signature length")
    }

    fn sig_array(bytes: &[u8]) -> &[u8; SIG_LEN] {
        bytes.try_into().expect("signature length")
    }

    /// One deterministic signature from each implementation over the same
    /// message, shared by the tests below. SLH-DSA signing takes about a
    /// second, so it is done once.
    fn signatures() -> &'static (Vec<u8>, Vec<u8>) {
        static SIGNATURES: std::sync::OnceLock<(Vec<u8>, Vec<u8>)> = std::sync::OnceLock::new();
        SIGNATURES.get_or_init(|| {
            let (_public, secret) = KG::keygen_from_seed(&SK_SEED, &SK_PRF, &PK_SEED);
            let ours = secret
                .try_sign_with_rng(&mut NoRandomness, MESSAGE, CONTEXT, false)
                .expect("signs")
                .to_vec();
            let theirs = theirs()
                .try_sign_with_context(MESSAGE, CONTEXT, None)
                .expect("signs")
                .to_vec();
            (ours, theirs)
        })
    }

    #[test]
    fn same_seeds_give_the_same_public_key() {
        let (public, _secret) = KG::keygen_from_seed(&SK_SEED, &SK_PRF, &PK_SEED);
        let ours = public.into_bytes();
        let signing_key = theirs();
        let theirs: &VerifyingKey<Sha2_256s> = signing_key.as_ref();
        assert_eq!(ours.to_vec(), theirs.to_vec());
    }

    #[test]
    fn deterministic_signatures_are_identical() {
        let (ours, theirs) = signatures();
        assert_eq!(ours.len(), SIG_LEN);
        assert_eq!(ours, theirs);
    }

    #[test]
    fn each_implementation_accepts_the_other() {
        let (ours, theirs) = signatures();
        let (public, _secret) = KG::keygen_from_seed(&SK_SEED, &SK_PRF, &PK_SEED);
        let their_key = their_verifying_key(&public.clone().into_bytes());

        assert!(their_key
            .try_verify_with_context(MESSAGE, CONTEXT, &their_signature(ours))
            .is_ok());
        assert!(public.verify(MESSAGE, sig_array(theirs), CONTEXT));

        // And both reject a changed message or context.
        assert!(their_key
            .try_verify_with_context(b"other", CONTEXT, &their_signature(ours))
            .is_err());
        assert!(!public.verify(MESSAGE, sig_array(theirs), b"other"));
        let mut damaged = theirs.clone();
        damaged[SIG_LEN / 2] ^= 1;
        assert!(!public.verify(MESSAGE, sig_array(&damaged), CONTEXT));
    }

    /// Both halves of a NottOS suite-2 signature verify against a
    /// representative rebuilt in the test, the SLH-DSA half with the second
    /// implementation.
    #[test]
    fn hybrid_signature_matches_the_documented_construction() {
        let suite = Suite::Ed25519SlhDsaSha2_256s;
        let mut seed = Vec::new();
        for part in [SK_SEED, SK_PRF, PK_SEED] {
            seed.extend_from_slice(&part);
        }
        let key = SecretKey::from_seeds(suite, [0x44; 32], &seed).expect("valid seeds");
        let public = key.public_key();
        let signature = key.sign(context::ROOT_METADATA, MESSAGE).expect("signs");
        let representative = documented_representative(suite, context::ROOT_METADATA, MESSAGE);

        let their_key = their_verifying_key(public.post_quantum());
        let decoded = their_signature(signature.post_quantum());
        assert!(their_key
            .try_verify_with_context(&representative, b"", &decoded)
            .is_ok());
        assert!(
            libcrux_ed25519::verify(&representative, public.ed25519(), signature.ed25519()).is_ok()
        );
        assert!(their_key
            .try_verify_with_context(MESSAGE, b"", &decoded)
            .is_err());

        // The public key's post-quantum half is the standard encoding of
        // the key both implementations derive from these seeds.
        let ours =
            OurPublicKey::try_from_bytes(public.post_quantum().try_into().expect("key length"))
                .expect("valid key");
        assert_eq!(ours.into_bytes().to_vec(), public.post_quantum());
        let signing_key = self::theirs();
        let theirs: &VerifyingKey<Sha2_256s> = signing_key.as_ref();
        assert_eq!(theirs.to_vec(), public.post_quantum());
    }
}

/// A generator that fails every request: deterministic signing must not
/// draw randomness.
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
