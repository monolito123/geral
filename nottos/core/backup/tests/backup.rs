// SPDX-License-Identifier: Apache-2.0

//! Backups: round trips; the construction of ADR-0013 recomputed with
//! second implementations; refusal of forged, altered, cut, reordered and
//! extended files; cost limits.

use chacha20poly1305::aead::{Aead as _, KeyInit as _, Payload};
use ml_kem::array::Array;
use ml_kem::kem::Decapsulate as _;
use ml_kem::{DecapsulationKey, MlKem1024, Seed};
use nott_backup::{
    check_kdf, encrypt_with_randomness, BackupError, BackupKey, Decryptor, Kdf, KeyFile,
    Randomness, RecoveryKeys, CHUNK_LEN,
};
use nott_format::backup::{BackupFormatError, BackupHeader};
use nott_format::text::Problem;
use nott_format::time::Timestamp;
use nott_format::{hex, Sha256Digest};
use sha3::digest::{ExtendableOutput as _, Update as _, XofReader as _};
use sha3::{Digest as _, Sha3_256};
use std::sync::OnceLock;
use zeroize::Zeroizing;

const SECRET: &[u8] = b"correct horse battery staple";
const TAG_LEN: usize = 16;
const SEALED_CHUNK_LEN: usize = CHUNK_LEN + TAG_LEN;

/// RFC 9106's second recommended option: the cheapest accepted.
fn kdf() -> Kdf {
    Kdf {
        memory_kib: 65_536,
        iterations: 3,
        lanes: 4,
        salt: [0x42; 16],
    }
}

/// Argon2id is slow on purpose; derive once for the whole test run.
struct World {
    key: BackupKey,
    keys: RecoveryKeys,
}

fn world() -> &'static World {
    static WORLD: OnceLock<World> = OnceLock::new();
    WORLD.get_or_init(|| World {
        key: BackupKey::derive(SECRET, kdf()).expect("backup key"),
        keys: RecoveryKeys::derive(SECRET, &kdf()).expect("recovery keys"),
    })
}

fn created() -> Timestamp {
    Timestamp::parse("2026-11-14T12:00:00Z").unwrap()
}

fn randomness(tag: u8) -> Randomness {
    Randomness {
        data_key: [tag; 32],
        x25519_ephemeral: [tag ^ 0x55; 32],
        ml_kem_m: [tag ^ 0xaa; 32],
    }
}

fn data(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 31 + i / 251) as u8).collect()
}

fn make(key: &BackupKey, plaintext: &[u8], tag: u8) -> Vec<u8> {
    let mut file = Vec::new();
    let written =
        encrypt_with_randomness(key, created(), randomness(tag), plaintext, &mut file).unwrap();
    assert_eq!(written, plaintext.len() as u64);
    file
}

fn restore(file: &[u8]) -> Result<Vec<u8>, BackupError> {
    let decryptor = Decryptor::open_with(&world().keys, file)?;
    let mut plaintext = Vec::new();
    let read = decryptor.decrypt_to(&mut plaintext)?;
    assert_eq!(read, plaintext.len() as u64);
    Ok(plaintext)
}

/// Offset just after the `n`th line feed.
fn after_line(file: &[u8], n: usize) -> usize {
    let (index, _) = file
        .iter()
        .enumerate()
        .filter(|(_, byte)| **byte == b'\n')
        .nth(n - 1)
        .expect("enough lines");
    index + 1
}

fn header_len(file: &[u8]) -> usize {
    after_line(file, 7)
}

/// Replaces the value of the header line starting with `prefix`.
fn edit_line(file: &[u8], prefix: &str, edit: impl Fn(&str) -> String) -> Vec<u8> {
    let end = header_len(file);
    let header = std::str::from_utf8(&file[..end]).unwrap();
    let mut changed = String::new();
    let mut found = false;
    for line in header.split_inclusive('\n') {
        match line.strip_prefix(prefix) {
            Some(value) => {
                found = true;
                changed.push_str(prefix);
                changed.push_str(&edit(value.trim_end_matches('\n')));
                changed.push('\n');
            }
            None => changed.push_str(line),
        }
    }
    assert!(found, "{prefix}");
    [changed.as_bytes(), &file[end..]].concat()
}

/// Changes the first hex digit at or after `offset` in a hex value.
fn flip_hex(value: &str, offset: usize) -> String {
    let mut bytes = value.as_bytes().to_vec();
    bytes[offset] = if bytes[offset] == b'0' { b'1' } else { b'0' };
    String::from_utf8(bytes).unwrap()
}

fn is<T>(result: Result<T, BackupError>, check: impl Fn(&BackupError) -> bool) -> bool {
    result.as_ref().err().is_some_and(check)
}

#[test]
fn round_trips_every_size() {
    for len in [
        0,
        1,
        100,
        CHUNK_LEN - 1,
        CHUNK_LEN,
        CHUNK_LEN + 1,
        2 * CHUNK_LEN,
        2 * CHUNK_LEN + 17,
    ] {
        let plaintext = data(len);
        let file = make(&world().key, &plaintext, 1);
        let chunks = len.div_ceil(CHUNK_LEN).max(1);
        assert_eq!(
            file.len(),
            header_len(&file) + len + chunks * TAG_LEN,
            "{len}"
        );
        assert_eq!(restore(&file).unwrap(), plaintext, "{len}");
    }
}

#[test]
fn the_recovery_secret_alone_opens_a_backup() {
    let file = make(&world().key, b"photos and messages", 2);
    let decryptor = Decryptor::open(SECRET, &file[..]).unwrap();
    assert_eq!(decryptor.created(), created());
    assert_eq!(
        decryptor.header().fields().recovery_key_id,
        world().key.recovery_key_id()
    );
    let mut plaintext = Vec::new();
    decryptor.decrypt_to(&mut plaintext).unwrap();
    assert_eq!(plaintext, b"photos and messages");
}

#[test]
fn another_secret_does_not() {
    let file = make(&world().key, b"x", 3);
    assert!(is(
        Decryptor::open(b"Correct horse battery staple", &file[..]),
        |error| { matches!(error, BackupError::WrongSecret) }
    ));
    assert!(is(Decryptor::open(b"", &file[..]), |error| {
        matches!(error, BackupError::SecretLength)
    }));
    assert!(is(Decryptor::open(&[b'a'; 1025], &file[..]), |error| {
        matches!(error, BackupError::SecretLength)
    }));
    let other = RecoveryKeys::derive(
        SECRET,
        &Kdf {
            salt: [0x43; 16],
            ..kdf()
        },
    )
    .unwrap();
    assert!(is(Decryptor::open_with(&other, &file[..]), |error| {
        matches!(error, BackupError::WrongSecret)
    }));
}

/// The parameters and salt are in every header, and the recovery public
/// key is not secret. Someone who has all three but not the authentication
/// key cannot make a backup that is accepted.
#[test]
fn a_backup_made_with_only_the_public_parts_is_refused() {
    let mut forged = KeyFile::parse(&world().key.to_text()).unwrap();
    forged.authentication_key = Zeroizing::new([0x99; 32]);
    let forger = BackupKey::parse(&forged.to_text()).unwrap();
    assert_eq!(forger.recovery_key_id(), world().key.recovery_key_id());
    let file = make(&forger, b"an app that was never installed", 4);
    assert!(is(restore(&file), |error| matches!(
        error,
        BackupError::Inauthentic
    )));
}

#[test]
fn every_byte_of_the_header_is_authenticated() {
    let file = make(&world().key, &data(100), 5);
    for position in 0..header_len(&file) {
        let mut changed = file.clone();
        changed[position] ^= 0x01;
        assert!(restore(&changed).is_err(), "byte {position}");
    }
}

#[test]
fn header_changes_are_named() {
    let file = make(&world().key, &data(100), 6);
    let inauthentic = |error: &BackupError| matches!(error, BackupError::Inauthentic);
    let wrong_secret = |error: &BackupError| matches!(error, BackupError::WrongSecret);

    let later = edit_line(&file, "created: ", |_| "2026-11-14T12:00:01Z".to_owned());
    assert!(is(restore(&later), inauthentic));
    let x25519 = edit_line(&file, "kem-ciphertext: ", |value| flip_hex(value, 0));
    assert!(is(restore(&x25519), inauthentic));
    let low_order = edit_line(&file, "kem-ciphertext: ", |value| {
        format!("{}{}", "0".repeat(64), &value[64..])
    });
    assert!(is(restore(&low_order), inauthentic));
    let ml_kem = edit_line(&file, "kem-ciphertext: ", |value| flip_hex(value, 1000));
    assert!(is(restore(&ml_kem), inauthentic));
    let wrapped = edit_line(&file, "wrapped-key: ", |value| flip_hex(value, 10));
    assert!(is(restore(&wrapped), inauthentic));
    let key_id = edit_line(&file, "recovery-key-id: ", |value| flip_hex(value, 0));
    assert!(is(restore(&key_id), wrong_secret));
    let salt = edit_line(&file, "kdf: ", |value| flip_hex(value, value.len() - 1));
    assert!(is(restore(&salt), wrong_secret));
    let memory = edit_line(&file, "kdf: ", |value| value.replacen("65536", "65537", 1));
    assert!(is(restore(&memory), wrong_secret));

    // Cost limits are checked before any memory is allocated.
    let cheap = edit_line(&file, "kdf: ", |value| value.replacen("65536", "1024", 1));
    assert!(is(Decryptor::open(SECRET, &cheap[..]), |error| {
        matches!(error, BackupError::KdfParameters)
    }));
    let huge = edit_line(&file, "kdf: ", |value| {
        value.replacen("65536", "4294967295", 1)
    });
    assert!(is(Decryptor::open(SECRET, &huge[..]), |error| {
        matches!(error, BackupError::KdfParameters)
    }));
    let version = [b"nottos-backup 2".as_slice(), &file[15..]].concat();
    assert!(is(restore(&version), |error| matches!(
        error,
        BackupError::Format(BackupFormatError::Envelope(Problem::UnsupportedVersion(_)))
    )));
}

#[test]
fn malformed_headers_are_refused_before_any_work() {
    let keys = &world().keys;
    let long = vec![b'a'; 5000];
    assert!(is(Decryptor::open_with(keys, &long[..]), |error| matches!(
        error,
        BackupError::Format(BackupFormatError::Envelope(Problem::TooLarge))
    )));
    let not_utf8 = b"nottos-backup 1\n\xff\n";
    assert!(is(
        Decryptor::open_with(keys, &not_utf8[..]),
        |error| matches!(
            error,
            BackupError::Format(BackupFormatError::Envelope(Problem::InvalidCharacter {
                line: 2
            }))
        )
    ));
    assert!(is(Decryptor::open_with(keys, &b""[..]), |error| matches!(
        error,
        BackupError::Format(_)
    )));
    let file = make(&world().key, b"x", 7);
    let cut_header = &file[..header_len(&file) - 1];
    assert!(is(
        Decryptor::open_with(keys, cut_header),
        |error| matches!(error, BackupError::Format(_))
    ));
}

#[test]
fn payload_changes_name_the_chunk() {
    let file = make(&world().key, &data(2 * CHUNK_LEN + 17), 8);
    let start = header_len(&file);
    for (index, position) in [
        (0, start),
        (0, start + SEALED_CHUNK_LEN - 1),
        (1, start + SEALED_CHUNK_LEN + 5),
        (2, start + 2 * SEALED_CHUNK_LEN),
        (2, file.len() - 1),
    ] {
        let mut changed = file.clone();
        changed[position] ^= 0x80;
        assert!(
            is(restore(&changed), |error| matches!(
                error,
                BackupError::Chunk { index: found } if *found == index
            )),
            "position {position}"
        );
    }
}

#[test]
fn a_cut_backup_is_refused() {
    let file = make(&world().key, &data(2 * CHUNK_LEN + 17), 9);
    let start = header_len(&file);
    let truncated = |error: &BackupError| matches!(error, BackupError::Truncated);
    assert!(is(restore(&file[..start]), truncated));
    assert!(is(restore(&file[..start + 10]), truncated));
    assert!(is(restore(&file[..start + SEALED_CHUNK_LEN]), truncated));
    assert!(is(
        restore(&file[..start + 2 * SEALED_CHUNK_LEN]),
        truncated
    ));
    assert!(is(restore(&file[..file.len() - 1]), |error| matches!(
        error,
        BackupError::Chunk { index: 2 }
    )));
}

#[test]
fn data_after_the_end_is_refused() {
    let partial = make(&world().key, &data(CHUNK_LEN + 17), 10);
    assert!(is(restore(&[partial.clone(), vec![0]].concat()), |error| {
        matches!(error, BackupError::Chunk { index: 1 })
    }));
    // When the final chunk is full, appended data is recognised as such.
    let full = make(&world().key, &data(2 * CHUNK_LEN), 11);
    let trailing = |error: &BackupError| matches!(error, BackupError::TrailingData);
    assert!(is(restore(&[full.clone(), vec![0]].concat()), trailing));
    let start = header_len(&full);
    let repeated = [full.clone(), full[start..start + SEALED_CHUNK_LEN].to_vec()].concat();
    assert!(is(restore(&repeated), trailing));
}

#[test]
fn chunks_cannot_be_reordered_dropped_or_moved_between_backups() {
    let file = make(&world().key, &data(2 * CHUNK_LEN + 17), 12);
    let start = header_len(&file);
    let chunk = |n: usize| {
        &file[start + n * SEALED_CHUNK_LEN..(start + (n + 1) * SEALED_CHUNK_LEN).min(file.len())]
    };
    let swapped = [&file[..start], chunk(1), chunk(0), chunk(2)].concat();
    assert!(is(restore(&swapped), |error| matches!(
        error,
        BackupError::Chunk { index: 0 }
    )));
    let dropped = [&file[..start], chunk(0), chunk(2)].concat();
    assert!(is(restore(&dropped), |error| matches!(
        error,
        BackupError::Chunk { index: 1 }
    )));
    let other = make(&world().key, &data(2 * CHUNK_LEN + 17), 13);
    let spliced = [&other[..header_len(&other)], &file[start..]].concat();
    assert!(is(restore(&spliced), |error| matches!(
        error,
        BackupError::Chunk { index: 0 }
    )));
}

/// What the ADR says, computed with other implementations: Argon2id from
/// `rust-argon2`, SHAKE256 and SHA3-256 from `sha3`, X25519 from
/// `x25519-dalek`, ML-KEM-1024 from RustCrypto `ml-kem`, ChaCha20-Poly1305
/// from RustCrypto `chacha20poly1305`.
struct Independent {
    recovery_key_id: Sha256Digest,
    payload_key: [u8; 32],
}

fn independent(file: &[u8]) -> Independent {
    let config = second_argon2::Config {
        mem_cost: 65_536,
        time_cost: 3,
        lanes: 4,
        hash_length: 32,
        ..second_argon2::Config::rfc9106_low_mem()
    };
    let seed = second_argon2::hash_raw(SECRET, &kdf().salt, &config).unwrap();
    let mut expanded = [0u8; 128];
    let mut xof = sha3::Shake256::default();
    xof.update(b"NottOS-backup-recovery-v1");
    xof.update(&seed);
    xof.finalize_xof().read(&mut expanded);
    let x25519_secret: [u8; 32] = expanded[..32].try_into().unwrap();
    let ml_kem_seed: [u8; 64] = expanded[32..96].try_into().unwrap();
    let authentication_key = &expanded[96..];

    let dalek_secret = x25519_dalek::StaticSecret::from(x25519_secret);
    let x25519_public = x25519_dalek::PublicKey::from(&dalek_secret);
    let decapsulation_key = DecapsulationKey::<MlKem1024>::from_seed(Seed::from(ml_kem_seed));
    let ml_kem_public: Vec<u8> =
        ml_kem::KeyExport::to_bytes(decapsulation_key.encapsulation_key()).to_vec();
    let public = [x25519_public.as_bytes().as_slice(), &ml_kem_public].concat();

    // The header, read without the format code.
    let header = std::str::from_utf8(&file[..header_len(file)]).unwrap();
    let value = |prefix: &str| {
        header
            .lines()
            .find_map(|line| line.strip_prefix(prefix))
            .unwrap()
    };
    let mut ciphertext = [0u8; 1600];
    assert!(hex::decode_into(value("kem-ciphertext: "), &mut ciphertext));
    let mut wrapped = [0u8; 48];
    assert!(hex::decode_into(value("wrapped-key: "), &mut wrapped));
    let authenticated = &file[..after_line(file, 6)];

    let ephemeral: [u8; 32] = ciphertext[..32].try_into().unwrap();
    let x25519_shared = dalek_secret
        .diffie_hellman(&x25519_dalek::PublicKey::from(ephemeral))
        .to_bytes();
    let ml_kem_ciphertext: [u8; 1568] = ciphertext[32..].try_into().unwrap();
    let ml_kem_shared = decapsulation_key.decapsulate(&Array::from(ml_kem_ciphertext));
    let shared = Sha3_256::new()
        .chain_update(ml_kem_shared)
        .chain_update(x25519_shared)
        .chain_update(ephemeral)
        .chain_update(x25519_public.as_bytes())
        .chain_update(b"NottOS-hybrid-kem-v1")
        .finalize();
    let wrap_key: [u8; 32] = Sha3_256::new()
        .chain_update(b"NottOS-backup-wrap-v1")
        .chain_update(shared)
        .chain_update(authentication_key)
        .finalize()
        .into();
    let data_key = chacha20poly1305::ChaCha20Poly1305::new((&wrap_key).into())
        .decrypt(
            (&[0u8; 12]).into(),
            Payload {
                msg: &wrapped,
                aad: authenticated,
            },
        )
        .expect("the wrapped key opens");
    let payload_key = Sha3_256::new()
        .chain_update(b"NottOS-backup-payload-v1")
        .chain_update(&data_key)
        .finalize()
        .into();
    Independent {
        recovery_key_id: Sha256Digest::of_bytes(&public),
        payload_key,
    }
}

/// The chunk nonce, as the ADR states it.
fn chunk_nonce(index: u64, last: bool) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[3..11].copy_from_slice(&index.to_be_bytes());
    nonce[11] = u8::from(last);
    nonce
}

#[test]
fn the_construction_is_the_documented_one() {
    let plaintext = data(CHUNK_LEN + 1000);
    let file = make(&world().key, &plaintext, 14);
    let derived = independent(&file);
    assert_eq!(derived.recovery_key_id, world().key.recovery_key_id());
    let cipher = chacha20poly1305::ChaCha20Poly1305::new((&derived.payload_key).into());
    let payload = &file[header_len(&file)..];
    let count = payload.len().div_ceil(SEALED_CHUNK_LEN);
    let mut opened = Vec::new();
    for (index, chunk) in payload.chunks(SEALED_CHUNK_LEN).enumerate() {
        let nonce = chunk_nonce(index as u64, index + 1 == count);
        let part = cipher
            .decrypt(
                (&nonce).into(),
                Payload {
                    msg: chunk,
                    aad: b"",
                },
            )
            .expect("chunk opens");
        opened.extend_from_slice(&part);
    }
    assert_eq!(opened, plaintext);
}

/// A backup whose last chunk is an empty final chunk after a full one is
/// authentic but not canonical; it is refused. Only the payload key can
/// make one, so the test makes it with the independently derived key.
#[test]
fn an_empty_final_chunk_after_others_is_refused() {
    let file = make(&world().key, &data(CHUNK_LEN), 15);
    let derived = independent(&file);
    let cipher = chacha20poly1305::ChaCha20Poly1305::new((&derived.payload_key).into());
    let start = header_len(&file);
    let first = cipher
        .encrypt(
            (&chunk_nonce(0, false)).into(),
            Payload {
                msg: &data(CHUNK_LEN),
                aad: b"",
            },
        )
        .unwrap();
    let empty = cipher
        .encrypt(
            (&chunk_nonce(1, true)).into(),
            Payload { msg: b"", aad: b"" },
        )
        .unwrap();
    let crafted = [&file[..start], &first, &empty].concat();
    assert!(is(restore(&crafted), |error| matches!(
        error,
        BackupError::EmptyFinalChunk
    )));
}

#[test]
fn pinned_vector() {
    // Any change to the construction, the format or the implementations
    // changes these digests.
    assert_eq!(
        world().key.recovery_key_id().to_hex(),
        "16c3f055982340a7c597e80026927bbe2e5713e20ce8a3f4d8d33e3b04604038"
    );
    let file = make(&world().key, b"NottOS backup test vector", 16);
    assert_eq!(
        Sha256Digest::of_bytes(&file).to_hex(),
        "b996b56b5153a7c44b0f1906603a2029130104b7dd50672618783c8aab0dca20"
    );
}

#[test]
fn cost_limits() {
    let accepts = |memory_kib, iterations, lanes| {
        check_kdf(&Kdf {
            memory_kib,
            iterations,
            lanes,
            salt: [0; 16],
        })
        .is_ok()
    };
    // RFC 9106's two recommended options.
    assert!(accepts(65_536, 3, 4));
    assert!(accepts(2_097_152, 1, 4));
    assert!(accepts(524_288, 3, 4));
    assert!(accepts(65_536, 10, 16));
    // Too little work, too much, or out of range.
    assert!(!accepts(65_536, 2, 4));
    assert!(!accepts(65_535, 3, 4));
    // Enough work, too little memory.
    assert!(!accepts(32_768, 6, 4));
    assert!(!accepts(65_535, 10, 4));
    assert!(!accepts(2_097_153, 1, 4));
    assert!(!accepts(262_144, 11, 4));
    assert!(!accepts(262_144, 0, 4));
    assert!(!accepts(262_144, 3, 0));
    assert!(!accepts(262_144, 3, 17));
    assert!(matches!(
        BackupKey::derive(
            SECRET,
            Kdf {
                memory_kib: u32::MAX,
                ..kdf()
            }
        ),
        Err(BackupError::KdfParameters)
    ));
}

#[test]
fn backup_keys_round_trip_and_are_checked() {
    let key = &world().key;
    let text = key.to_text();
    let parsed = BackupKey::parse(&text).unwrap();
    assert_eq!(parsed.recovery_key_id(), key.recovery_key_id());
    assert_eq!(*parsed.to_text(), *text);
    assert_eq!(parsed.kdf(), &kdf());
    // Made with the parsed key, opened with the recovery secret.
    assert_eq!(restore(&make(&parsed, b"y", 17)).unwrap(), b"y");

    let file = KeyFile::parse(&text).unwrap();
    let authentication_key = hex::encode(&file.authentication_key[..]);
    assert!(!format!("{key:?}").contains(&authentication_key));
    assert!(!format!("{:?}", world().keys).contains(&authentication_key));

    // An ML-KEM coefficient of 4095 fails the FIPS 203 input check.
    let mut invalid = file.clone();
    invalid.public_key[32] = 0xff;
    invalid.public_key[33] = 0x0f;
    assert!(matches!(
        BackupKey::parse(&invalid.to_text()),
        Err(BackupError::InvalidPublicKey)
    ));
    let mut weak = file;
    weak.kdf.iterations = 1;
    assert!(matches!(
        BackupKey::parse(&weak.to_text()),
        Err(BackupError::KdfParameters)
    ));
}

#[test]
fn header_matches_its_parse() {
    let file = make(&world().key, b"z", 18);
    let text = std::str::from_utf8(&file[..header_len(&file)]).unwrap();
    let header = BackupHeader::parse(text).unwrap();
    assert_eq!(header.to_string(), text);
    assert_eq!(
        header.fields().authenticated_text().as_bytes(),
        &file[..after_line(&file, 6)]
    );
}
