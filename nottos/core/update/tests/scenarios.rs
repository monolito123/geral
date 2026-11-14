// SPDX-License-Identifier: Apache-2.0

//! The update system against the attacks it exists to stop: forged,
//! replayed, rolled-back, frozen, retargeted and mixed metadata; root
//! rotation done wrong; and keys that were rotated out.

use nott_crypto::{context, PublicKey, SecretKey, Signature, SignatureSet, Suite};
use nott_format::time::Timestamp;
use nott_format::update::{Role, Root, UpdateFields, UpdateMetadata};
use nott_format::Sha256Digest;
use nott_update::{
    check_initial_root, check_manifest, rotate_root, valid_signatures, verify_update, Device,
    TrustState, UpdateError,
};
use std::sync::OnceLock;

const DEVICE: Device<'static> = Device {
    channel: "stable",
    target: "mini_x86_64",
};

fn time(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("valid time")
}

fn root_key(tag: u8) -> SecretKey {
    SecretKey::from_seeds(Suite::Ed25519SlhDsaSha2_256s, [tag; 32], &[tag; 96]).expect("seeds")
}

fn release_key(tag: u8) -> SecretKey {
    SecretKey::from_seeds(Suite::Ed25519MlDsa87, [tag; 32], &[tag; 32]).expect("seeds")
}

fn publics(keys: &[&SecretKey]) -> Vec<PublicKey> {
    keys.iter().map(|key| key.public_key()).collect()
}

fn set(signatures: Vec<Signature>) -> SignatureSet {
    SignatureSet::new(signatures).expect("distinct signers")
}

/// Keys, roots and root signatures. Root keys sign with SLH-DSA, which
/// takes about a second per signature, so all of this is built once.
struct World {
    r: [SecretKey; 4],
    k: [SecretKey; 4],
    /// Version 1: root keys R0 R1 R2 (2 of 3), release keys K0 K1 K2 (2 of 3).
    root_v1: String,
    /// Signatures on `root_v1` by R0 and by R1.
    v1_by: [Signature; 2],
    /// Version 2: root keys R1 R2 R3 (2 of 3), release key K3 alone (1 of
    /// 1): K0 to K2 are rotated out.
    root_v2: String,
    /// Signatures on `root_v2` by R0, R1, R2 and R3.
    v2_by: [Signature; 4],
    /// Version 3 with the keys of version 1, signed by R0 and R1: a valid
    /// root that skips version 2.
    root_v3: String,
    v3_by: [Signature; 2],
}

fn world() -> &'static World {
    static WORLD: OnceLock<World> = OnceLock::new();
    WORLD.get_or_init(|| {
        let r = [root_key(10), root_key(11), root_key(12), root_key(13)];
        let k = [
            release_key(20),
            release_key(21),
            release_key(22),
            release_key(23),
        ];
        let root = |version,
                    root_keys: &[&SecretKey],
                    threshold,
                    release_keys: &[&SecretKey],
                    release_threshold| {
            Root::new(
                version,
                time("2027-11-13T00:00:00Z"),
                Role::new(threshold, publics(root_keys)).expect("valid role"),
                Role::new(release_threshold, publics(release_keys)).expect("valid role"),
            )
            .expect("valid root")
            .to_string()
        };
        let root_v1 = root(1, &[&r[0], &r[1], &r[2]], 2, &[&k[0], &k[1], &k[2]], 2);
        let root_v2 = root(2, &[&r[1], &r[2], &r[3]], 2, &[&k[3]], 1);
        let root_v3 = root(3, &[&r[0], &r[1], &r[2]], 2, &[&k[0], &k[1], &k[2]], 2);
        let sign = |key: &SecretKey, text: &str| {
            key.sign(context::ROOT_METADATA, text.as_bytes())
                .expect("signs")
        };
        let v1_by = [sign(&r[0], &root_v1), sign(&r[1], &root_v1)];
        let v2_by = [
            sign(&r[0], &root_v2),
            sign(&r[1], &root_v2),
            sign(&r[2], &root_v2),
            sign(&r[3], &root_v2),
        ];
        let v3_by = [sign(&r[0], &root_v3), sign(&r[1], &root_v3)];
        World {
            r,
            k,
            root_v1,
            v1_by,
            root_v2,
            v2_by,
            root_v3,
            v3_by,
        }
    })
}

fn state_v1() -> TrustState {
    let world = world();
    let root = check_initial_root(&world.root_v1, &set(world.v1_by.to_vec())).expect("trusted");
    TrustState {
        root,
        last_sequence: 6,
        time_floor: Timestamp::EPOCH,
    }
}

const MANIFEST: &[u8] = b"nottos-build-manifest 1\n(stand-in)\n";

fn metadata(sequence: u64, channel: &str, target: &str, expires: &str) -> String {
    UpdateMetadata::new(UpdateFields {
        channel,
        target,
        sequence,
        issued: time("2026-11-13T00:00:00Z"),
        expires: time(expires),
        manifest_sha256: Sha256Digest::of_bytes(MANIFEST),
        manifest_size: MANIFEST.len() as u64,
    })
    .expect("valid metadata")
    .to_string()
}

fn current_metadata() -> String {
    metadata(7, "stable", "mini_x86_64", "2026-12-13T00:00:00Z")
}

fn signed(text: &str, keys: &[&SecretKey]) -> SignatureSet {
    set(keys
        .iter()
        .map(|key| {
            key.sign(context::UPDATE_METADATA, text.as_bytes())
                .expect("signs")
        })
        .collect())
}

const NOW: &str = "2026-11-20T00:00:00Z";

fn check(
    state: &TrustState,
    text: &str,
    signatures: &SignatureSet,
    now: &str,
) -> Result<u64, UpdateError> {
    verify_update(state, DEVICE, text, signatures, time(now))
        .map(|(metadata, _)| metadata.sequence())
}

// --- the shipped root ------------------------------------------------------

#[test]
fn shipped_root_needs_a_threshold_of_its_own_keys() {
    let world = world();
    assert!(check_initial_root(&world.root_v1, &set(world.v1_by.to_vec())).is_ok());
    assert_eq!(
        check_initial_root(&world.root_v1, &set(vec![world.v1_by[0].clone()])),
        Err(UpdateError::ThresholdNotMet {
            role: "root",
            needed: 2,
            valid: 1
        })
    );
    // Signatures on another root do not count.
    assert_eq!(
        check_initial_root(&world.root_v1, &set(world.v2_by[1..3].to_vec())),
        Err(UpdateError::ThresholdNotMet {
            role: "root",
            needed: 2,
            valid: 0
        })
    );
}

#[test]
fn roles_may_only_use_their_suite() {
    let world = world();
    let any_signatures = set(vec![world.v1_by[0].clone()]);
    // Release keys as root keys, and root keys as release keys.
    let swapped = Root::new(
        1,
        time("2027-11-13T00:00:00Z"),
        Role::new(1, publics(&[&world.k[0]])).expect("role"),
        Role::new(1, publics(&[&world.r[0]])).expect("role"),
    )
    .expect("root")
    .to_string();
    assert_eq!(
        check_initial_root(&swapped, &any_signatures),
        Err(UpdateError::SuiteNotAllowed { role: "root" })
    );
    let release_as_root_suite = Root::new(
        1,
        time("2027-11-13T00:00:00Z"),
        Role::new(1, publics(&[&world.r[0]])).expect("role"),
        Role::new(1, publics(&[&world.r[1]])).expect("role"),
    )
    .expect("root")
    .to_string();
    assert_eq!(
        check_initial_root(&release_as_root_suite, &any_signatures),
        Err(UpdateError::SuiteNotAllowed { role: "release" })
    );
}

// --- update metadata -------------------------------------------------------

#[test]
fn accepts_a_valid_update_and_advances_the_state() {
    let world = world();
    let state = state_v1();
    let text = current_metadata();
    let (metadata, next) = verify_update(
        &state,
        DEVICE,
        &text,
        &signed(&text, &[&world.k[0], &world.k[2]]),
        time(NOW),
    )
    .expect("accepted");
    assert_eq!(metadata.sequence(), 7);
    assert_eq!(next.last_sequence, 7);
    assert_eq!(next.time_floor, time("2026-11-13T00:00:00Z"));
    assert_eq!(next.root, state.root);
    assert_eq!(check_manifest(&metadata, MANIFEST), Ok(()));
}

#[test]
fn requires_a_threshold_of_release_keys() {
    let world = world();
    let state = state_v1();
    let text = current_metadata();
    let needed = |valid| {
        Err(UpdateError::ThresholdNotMet {
            role: "release",
            needed: 2,
            valid,
        })
    };
    assert_eq!(
        check(&state, &text, &signed(&text, &[&world.k[1]]), NOW),
        needed(1)
    );
    // A key outside the role counts for nothing, even if its signature is
    // valid.
    assert_eq!(
        check(
            &state,
            &text,
            &signed(&text, &[&world.k[1], &world.k[3]]),
            NOW
        ),
        needed(1)
    );
    // Root keys do not sign updates.
    let by_root_key = set(vec![
        world.r[0]
            .sign(context::UPDATE_METADATA, text.as_bytes())
            .expect("signs"),
        world.k[1]
            .sign(context::UPDATE_METADATA, text.as_bytes())
            .expect("signs"),
    ]);
    assert_eq!(check(&state, &text, &by_root_key, NOW), needed(1));
    // A signature made for another purpose does not count.
    let wrong_purpose = set(vec![
        world.k[0]
            .sign(context::BUILD_MANIFEST, text.as_bytes())
            .expect("signs"),
        world.k[1]
            .sign(context::UPDATE_METADATA, text.as_bytes())
            .expect("signs"),
    ]);
    assert_eq!(check(&state, &text, &wrong_purpose, NOW), needed(1));
    assert_eq!(
        valid_signatures(
            state.root.release_role(),
            context::UPDATE_METADATA,
            text.as_bytes(),
            &signed(&text, &[&world.k[0], &world.k[1], &world.k[2]])
        ),
        3
    );
}

#[test]
fn metadata_changed_after_signing_is_refused() {
    let world = world();
    let state = state_v1();
    let text = current_metadata();
    let signatures = signed(&text, &[&world.k[0], &world.k[1]]);
    // Point the update at another manifest, keep the signatures.
    let forged = text.replacen(
        &Sha256Digest::of_bytes(MANIFEST).to_hex(),
        &Sha256Digest::of_bytes(b"malicious").to_hex(),
        1,
    );
    assert_ne!(forged, text);
    assert_eq!(
        check(&state, &forged, &signatures, NOW),
        Err(UpdateError::ThresholdNotMet {
            role: "release",
            needed: 2,
            valid: 0
        })
    );
}

#[test]
fn replay_and_rollback_are_refused() {
    let world = world();
    let state = state_v1();
    for (sequence, installed) in [(6, 6), (3, 6)] {
        let text = metadata(sequence, "stable", "mini_x86_64", "2026-12-13T00:00:00Z");
        assert_eq!(
            check(
                &state,
                &text,
                &signed(&text, &[&world.k[0], &world.k[1]]),
                NOW
            ),
            Err(UpdateError::NotNewer {
                installed,
                offered: sequence
            })
        );
    }
    // Accepting release 7 makes release 7 itself a replay.
    let text = current_metadata();
    let signatures = signed(&text, &[&world.k[0], &world.k[1]]);
    let (_, next) = verify_update(&state, DEVICE, &text, &signatures, time(NOW)).expect("ok");
    assert_eq!(
        check(&next, &text, &signatures, NOW),
        Err(UpdateError::NotNewer {
            installed: 7,
            offered: 7
        })
    );
}

#[test]
fn expired_metadata_is_refused_even_with_the_clock_set_back() {
    let world = world();
    let state = state_v1();
    let text = current_metadata();
    let signatures = signed(&text, &[&world.k[0], &world.k[1]]);
    let expired = Err(UpdateError::MetadataExpired {
        expires: time("2026-12-13T00:00:00Z"),
        now: time("2026-12-13T00:00:00Z"),
    });
    assert_eq!(
        check(&state, &text, &signatures, "2026-12-13T00:00:00Z"),
        expired
    );
    assert_eq!(
        check(&state, &text, &signatures, "2026-12-12T23:59:59Z"),
        Ok(7)
    );

    // The device once accepted metadata issued on 2027-01-01. An attacker
    // sets the clock back to 2026-11-20 to make stale metadata look fresh.
    let floored = TrustState {
        time_floor: time("2027-01-01T00:00:00Z"),
        ..state
    };
    assert_eq!(
        check(&floored, &text, &signatures, NOW),
        Err(UpdateError::MetadataExpired {
            expires: time("2026-12-13T00:00:00Z"),
            now: time("2027-01-01T00:00:00Z"),
        })
    );
}

#[test]
fn an_expired_root_trusts_nothing() {
    let world = world();
    let state = state_v1();
    let text = metadata(7, "stable", "mini_x86_64", "2028-01-01T00:00:00Z");
    assert_eq!(
        check(
            &state,
            &text,
            &signed(&text, &[&world.k[0], &world.k[1]]),
            "2027-11-13T00:00:00Z"
        ),
        Err(UpdateError::RootExpired {
            expires: time("2027-11-13T00:00:00Z"),
            now: time("2027-11-13T00:00:00Z"),
        })
    );
}

#[test]
fn metadata_for_another_channel_or_target_is_refused() {
    let world = world();
    let state = state_v1();
    let beta = metadata(7, "beta", "mini_x86_64", "2026-12-13T00:00:00Z");
    assert_eq!(
        check(
            &state,
            &beta,
            &signed(&beta, &[&world.k[0], &world.k[1]]),
            NOW
        ),
        Err(UpdateError::WrongChannel {
            expected: "stable".to_owned(),
            found: "beta".to_owned()
        })
    );
    let phone = metadata(7, "stable", "some_phone", "2026-12-13T00:00:00Z");
    assert_eq!(
        check(
            &state,
            &phone,
            &signed(&phone, &[&world.k[0], &world.k[1]]),
            NOW
        ),
        Err(UpdateError::WrongTarget {
            expected: "mini_x86_64".to_owned(),
            found: "some_phone".to_owned()
        })
    );
}

#[test]
fn a_different_manifest_is_refused() {
    let world = world();
    let text = current_metadata();
    let (metadata, _) = verify_update(
        &state_v1(),
        DEVICE,
        &text,
        &signed(&text, &[&world.k[0], &world.k[1]]),
        time(NOW),
    )
    .expect("accepted");
    let mut longer = MANIFEST.to_vec();
    longer.push(b'\n');
    let mut same_size = MANIFEST.to_vec();
    same_size[0] ^= 1;
    for manifest in [&longer[..], &same_size[..], b""] {
        assert_eq!(
            check_manifest(&metadata, manifest),
            Err(UpdateError::ManifestMismatch)
        );
    }
}

// --- root rotation ---------------------------------------------------------

#[test]
fn rotation_needs_both_the_old_and_the_new_threshold() {
    let world = world();
    let v1 = state_v1().root;
    let by = |indices: &[usize]| set(indices.iter().map(|&i| world.v2_by[i].clone()).collect());

    // R1 and R2 are in both the old and the new root: enough for both.
    let v2 = rotate_root(&v1, &world.root_v2, &by(&[1, 2])).expect("rotates");
    assert_eq!(v2.version(), 2);

    // R0 and R1 satisfy the old root, but only R1 is in the new one.
    assert_eq!(
        rotate_root(&v1, &world.root_v2, &by(&[0, 1])),
        Err(UpdateError::ThresholdNotMet {
            role: "new root",
            needed: 2,
            valid: 1
        })
    );
    // R3 alone: a key the old root never trusted cannot introduce itself.
    assert_eq!(
        rotate_root(&v1, &world.root_v2, &by(&[3])),
        Err(UpdateError::ThresholdNotMet {
            role: "root",
            needed: 2,
            valid: 0
        })
    );
    // R2 and R3: new root satisfied, old root only once.
    assert_eq!(
        rotate_root(&v1, &world.root_v2, &by(&[2, 3])),
        Err(UpdateError::ThresholdNotMet {
            role: "root",
            needed: 2,
            valid: 1
        })
    );
}

#[test]
fn rotation_must_go_one_version_at_a_time() {
    let world = world();
    let v1 = state_v1().root;
    // A properly signed version 3 cannot skip version 2.
    assert_eq!(
        rotate_root(&v1, &world.root_v3, &set(world.v3_by.to_vec())),
        Err(UpdateError::RootVersion {
            expected: 2,
            found: 3
        })
    );
    let signatures = set(world.v2_by[1..3].to_vec());
    let v2 = rotate_root(&v1, &world.root_v2, &signatures).expect("rotates");
    // Re-applying version 2, or going back to version 1, is refused.
    assert_eq!(
        rotate_root(&v2, &world.root_v2, &signatures),
        Err(UpdateError::RootVersion {
            expected: 3,
            found: 2
        })
    );
    assert_eq!(
        rotate_root(&v2, &world.root_v1, &set(world.v1_by.to_vec())),
        Err(UpdateError::RootVersion {
            expected: 3,
            found: 1
        })
    );
}

#[test]
fn keys_rotated_out_lose_their_power() {
    let world = world();
    let v2 = rotate_root(
        &state_v1().root,
        &world.root_v2,
        &set(world.v2_by[1..3].to_vec()),
    )
    .expect("rotates");
    let state = TrustState {
        root: v2,
        last_sequence: 6,
        time_floor: Timestamp::EPOCH,
    };
    let text = current_metadata();
    // K0, K1 and K2 were enough under version 1 ...
    assert_eq!(
        check(
            &state,
            &text,
            &signed(&text, &[&world.k[0], &world.k[1], &world.k[2]]),
            NOW
        ),
        Err(UpdateError::ThresholdNotMet {
            role: "release",
            needed: 1,
            valid: 0
        })
    );
    // ... and K3 alone is enough under version 2.
    assert_eq!(
        check(&state, &text, &signed(&text, &[&world.k[3]]), NOW),
        Ok(7)
    );
}

#[test]
fn malformed_roots_and_metadata_are_reported_as_such() {
    let world = world();
    let any = set(vec![world.v1_by[0].clone()]);
    assert!(matches!(
        check_initial_root("nottos-root 1\n", &any),
        Err(UpdateError::Format(_))
    ));
    // Metadata is only parsed after its signatures are checked, so a
    // malformed document signed by enough release keys is still caught.
    let state = state_v1();
    let text = "not metadata\n";
    assert!(matches!(
        check(
            &state,
            text,
            &signed(text, &[&world.k[0], &world.k[1]]),
            NOW
        ),
        Err(UpdateError::Format(_))
    ));
}
