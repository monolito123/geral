// SPDX-License-Identifier: Apache-2.0

//! Argon2id: RFC 9106 and reference-implementation vectors, and agreement
//! with a second implementation (`rust-argon2`, tests only).
//!
//! Sources are listed in `vectors/README.md`.

use nott_crypto::password::{argon2id, Argon2idParams, KdfError};
use nott_format::hex;

fn params(memory_kib: u32, iterations: u32, lanes: u32) -> Argon2idParams {
    Argon2idParams {
        memory_kib,
        iterations,
        lanes,
    }
}

/// RFC 9106, section 5.3. The vector uses a secret key and associated
/// data, which NottOS never passes, so it is checked against the pinned
/// implementation directly; the vectors below go through the wrapper.
#[test]
fn rfc9106_section_5_3_on_the_pinned_implementation() {
    let params = argon2::ParamsBuilder::new()
        .m_cost(32)
        .t_cost(3)
        .p_cost(4)
        .data(argon2::AssociatedData::new(&[0x04; 12]).unwrap())
        .output_len(32)
        .build()
        .unwrap();
    let hasher = argon2::Argon2::new_with_secret(
        &[0x03; 8],
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        params,
    )
    .unwrap();
    let mut tag = [0u8; 32];
    hasher
        .hash_password_into(&[0x01; 32], &[0x02; 16], &mut tag)
        .unwrap();
    assert_eq!(
        hex::encode(&tag),
        "0d640df58d78766c08c037a34a8b53c9d01ef0452d75b65eb52520e96b01e659"
    );
}

/// `src/test.c` of the reference implementation (P-H-C/phc-winner-argon2),
/// every Argon2id case, through NottOS's wrapper.
#[test]
fn reference_implementation_vectors() {
    let cases = [
        (
            2,
            16,
            1,
            "password",
            "somesalt",
            "09316115d5cf24ed5a15a31a3ba326e5cf32edc24702987c02b6566f61913cf7",
        ),
        (
            2,
            18,
            1,
            "password",
            "somesalt",
            "78fe1ec91fb3aa5657d72e710854e4c3d9b9198c742f9616c2f085bed95b2e8c",
        ),
        (
            2,
            8,
            1,
            "password",
            "somesalt",
            "9dfeb910e80bad0311fee20f9c0e2b12c17987b4cac90c2ef54d5b3021c68bfe",
        ),
        (
            2,
            8,
            2,
            "password",
            "somesalt",
            "6d093c501fd5999645e0ea3bf620d7b8be7fd2db59c20d9fff9539da2bf57037",
        ),
        (
            1,
            16,
            1,
            "password",
            "somesalt",
            "f6a5adc1ba723dddef9b5ac1d464e180fcd9dffc9d1cbf76cca2fed795d9ca98",
        ),
        (
            4,
            16,
            1,
            "password",
            "somesalt",
            "9025d48e68ef7395cca9079da4c4ec3affb3c8911fe4f86d1a2520856f63172c",
        ),
        (
            2,
            16,
            1,
            "differentpassword",
            "somesalt",
            "0b84d652cf6b0c4beaef0dfe278ba6a80df6696281d7e0d2891b817d8c458fde",
        ),
        (
            2,
            16,
            1,
            "password",
            "diffsalt",
            "bdf32b05ccc42eb15d58fd19b1f856b113da1e9a5874fdcc544308565aa8141c",
        ),
    ];
    for (iterations, log_memory, lanes, password, salt, expected) in cases {
        let key = argon2id(
            password.as_bytes(),
            salt.as_bytes(),
            params(1 << log_memory, iterations, lanes),
        )
        .unwrap();
        assert_eq!(
            hex::encode(&key[..]),
            expected,
            "t={iterations} m=2^{log_memory} p={lanes}"
        );
    }
}

#[test]
fn agrees_with_a_second_implementation() {
    let cases = [
        (8, 1, 1, &b"x"[..], [0u8; 16]),
        (1024, 1, 4, b"correct horse battery staple", [1; 16]),
        (4096, 2, 8, b"\xc3\xa7\xc3\xa3o", [0xff; 16]),
        (65_536, 3, 4, &[0x55; 1024], [0x5a; 16]),
    ];
    for (memory_kib, iterations, lanes, secret, salt) in cases {
        let ours = argon2id(secret, &salt, params(memory_kib, iterations, lanes)).unwrap();
        let config = second_argon2::Config {
            mem_cost: memory_kib,
            time_cost: iterations,
            lanes,
            hash_length: 32,
            ..second_argon2::Config::rfc9106_low_mem()
        };
        let theirs = second_argon2::hash_raw(secret, &salt, &config).unwrap();
        assert_eq!(
            &ours[..],
            &theirs[..],
            "m={memory_kib} t={iterations} p={lanes}"
        );
    }
}

#[test]
fn refuses_what_argon2_does_not_define() {
    let good = params(64, 1, 1);
    assert_eq!(
        argon2id(b"secret", b"short", good),
        Err(KdfError::Parameters)
    );
    assert_eq!(
        argon2id(b"secret", b"long enough", params(7, 1, 1)),
        Err(KdfError::Parameters)
    );
    assert_eq!(
        argon2id(b"secret", b"long enough", params(64, 0, 1)),
        Err(KdfError::Parameters)
    );
    assert_eq!(
        argon2id(b"secret", b"long enough", params(64, 1, 0)),
        Err(KdfError::Parameters)
    );
    assert!(argon2id(b"secret", b"long enough", good).is_ok());
}
