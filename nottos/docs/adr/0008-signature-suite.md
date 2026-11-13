# ADR-0008: Hybrid signature suite and formats

- **Status:** Accepted (2026-11-13; decided under the owner's delegation).
  Amended the same day: suite 2 for root keys (section "Suite 2").
- **Date:** 2026-11-13
- **Decider:** project owner (delegated)

## Context

The brief requires that wherever an asymmetric algorithm guards code or
persistent data, a classical algorithm is combined with a NIST-standardised
post-quantum one, "so security holds if either is broken": Ed25519 or ECDSA
with ML-DSA (FIPS 204), and SLH-DSA for long-lived roots. It also requires
vetted implementations, formally verified where available, no new
primitives, and formats that carry algorithm versions.

Signatures are needed first: for the build manifest, then for update and
repository metadata.

Facts:

- FIPS 204 is final `[V:F-06]`. Android 17's keystore exposes ML-DSA-65 and
  ML-DSA-87 `[V:F-07]`.
- The IETF's composite-signature specification, which defines how to pair
  ML-DSA with a classical algorithm, is still an Internet-Draft, and its
  pairings for ML-DSA-87 use ECDSA or Ed448, not Ed25519 `[V:F-29]`.
- Rust implementations available today `[V:F-30]`:
  - `libcrux-ed25519`: describes itself as a "formally verified ed25519
    signature library"; the code comes from HACL\*.
  - `libcrux-ml-dsa`: "the portable and AVX2 code for field arithmetic, NTT
    polynomial arithmetic, and serialization is formally verified". The
    rest is not. Version 0.0.x.
  - RustCrypto `ml-dsa`: "has never been independently audited".
  - `aws-lc-rs`: a Rust interface to a C library with FIPS validation.

## Options considered

### Which algorithms

| Option | For | Against |
|---|---|---|
| Ed25519 + ML-DSA-65 | The pairing the IETF draft defines for Ed25519 | Security category 3; the brief elsewhere chooses the highest level (ML-KEM-1024, 256-bit symmetric keys) |
| **Ed25519 + ML-DSA-87** | Category 5 post-quantum component; Ed25519 is deterministic, simple and available in verified form | Not a pairing the IETF draft defines |
| ECDSA P-384 + ML-DSA-87 | A pairing the IETF draft defines | ECDSA depends on per-signature secret numbers and has a long history of implementation failures; no verified implementation at hand |

A hybrid is as strong as its stronger half. Against a quantum attacker that
is ML-DSA-87 whatever the classical half is; against a classical attacker
who has somehow broken ML-DSA, Ed25519's roughly 128-bit strength still
stands.

### How the two signatures are combined

| Option | Assessment |
|---|---|
| Two independent signatures over the message | Rejected. Either half can be detached and presented as an ordinary signature by that algorithm, and nothing ties a signature to its purpose. |
| The IETF composite draft as written | Not final, and no Ed25519 + ML-DSA-87 pairing `[V:F-29]`. |
| **Both algorithms sign one representative that names the construction, suite and purpose, and contains a hash of the message** | Same shape as the IETF draft (prefix, label, context, pre-hash), so a later move to the standard is a new suite, not a redesign. |

### Which implementation

| Option | Assessment |
|---|---|
| **`libcrux` (Ed25519 and ML-DSA)** | Pure Rust, no C toolchain, the most formal verification available today; young version numbers |
| RustCrypto | Pure Rust, widely used building blocks, but its ML-DSA is unaudited by its own statement |
| `aws-lc-rs` | Strong provenance, but a large C library; on a device the natural counterpart is the BoringSSL already in AOSP |

## Decision

**Suite 1, `ed25519+ml-dsa-87`**, implemented with `libcrux`, cross-checked
in tests against RustCrypto `ml-dsa`.

To sign message `M` for purpose `context`:

```text
R = "NottOS-hybrid-signature-v1" || 0x00
    || suite id (2 bytes, big endian; 0x0001)
    || length of context (1 byte) || context
    || SHA-512(M)

ed25519   = Ed25519.Sign(sk1, R)                    RFC 8032
ml-dsa-87 = ML-DSA-87.Sign(sk2, R, ctx = "")        FIPS 204, hedged
```

A signature is valid only if both components verify over `R`.

Rules:

1. **Purpose is part of what is signed.** `context` is a short registered
   name (`build-manifest` first). A signature made for one purpose does not
   verify for another, even if its label is rewritten.
2. **Keys are identified by the SHA-256 of their canonical file**, and a
   signature names the key that made it.
3. **Formats are versioned canonical text** (`nott_format::signature`): a
   header with a format version, a `suite:` line that selects the
   algorithms, lower-case hexadecimal. A different algorithm combination is
   a new suite; old files keep their meaning.
4. **Secret keys are two 32-byte seeds.** Working keys are derived when
   needed and wiped afterwards. The secret-key file format is for test and
   development keys only.
5. **Implementations are pinned to exact versions** and wrapped behind
   `nott-crypto`, so replacing one touches one crate.
6. **Nothing here is a new primitive.** `nott-crypto` builds the
   representative and calls the two algorithms.

Evidence required and in place (`core/crypto/tests`):

- RFC 8032 test vectors for Ed25519.
- NIST ACVP vectors for ML-DSA-87: all 25 key-generation tests, 2
  deterministic signing tests, 6 verification tests (2 valid, 4 invalid).
- Agreement with a second implementation: same seed gives the same key
  pair; deterministic signatures are byte-identical; each accepts the
  other's signatures.
- The hybrid signature's two halves verify against a representative
  rebuilt in the test from the text above, the ML-DSA half by the second
  implementation.
- Each half alone, halves from different messages, and halves from
  different keys are all rejected.
- A pinned test vector for the whole construction.

## Suite 2: `ed25519+slh-dsa-sha2-256s`, for root keys

The brief asks for SLH-DSA (FIPS 205) for long-lived roots such as the
update and verified-boot root keys.

**Why a different post-quantum algorithm at the root.** SLH-DSA's security
rests only on the hash function. ML-DSA rests on lattice problems. If a
weakness is ever found in lattice signatures, release keys under suite 1
would be at risk, but a root under suite 2 would still stand and could
certify replacement keys under a new suite. The root is the recovery
anchor, so it must not share the release keys' assumptions.

**Parameter set.** SLH-DSA-SHA2-256s: security category 5, like ML-DSA-87;
the "s" variants have the smaller signatures (29,792 bytes) at the cost of
slow signing, which suits a key that signs rarely. SHA-2 rather than SHAKE
because SHA-2 instructions are common on both Arm and x86 hosts.

**Construction.** Identical to suite 1, with suite id 2 in the
representative; the post-quantum half is
`SLH-DSA-SHA2-256s.Sign(sk2, R, ctx = "")`, hedged.

**Implementation.** No formally verified SLH-DSA exists in Rust today, and
neither candidate claims an audit `[V:F-32]`:

| Option | Assessment |
|---|---|
| **`fips205` 0.5.0** | Pure Rust, no `unsafe`; implements the final FIPS 205; constant-time key generation and signing claimed at source level and checked with `dudect` measurements; seeds in, fixed-size arrays out |
| RustCrypto `slh-dsa` 0.1.0 | Pure Rust; states that it "has never been independently audited" |

`fips205` is used, pinned to an exact version. RustCrypto `slh-dsa` is used
only in tests, as the second implementation. (Its declared dependency on a
pre-release `signature` crate resolves to an incompatible later
pre-release, so the lock file pins `signature` to `2.3.0-pre.4`. This
affects tests only.)

**Secret key.** Ed25519 seed (32 bytes) and the three SLH-DSA seeds
`SK.seed || SK.prf || PK.seed` (96 bytes).

**Measured cost** on this development machine (x86-64, no SIMD paths):
deriving the public key from seeds about 35 ms, signing about 0.6 s,
verifying under 1 ms. Signing cost is irrelevant for a key used a few
times a year; verification cost is what devices pay.

**Evidence** (`core/crypto/tests`):

- NIST ACVP vectors for SLH-DSA-SHA2-256s: all 10 key-generation tests, 2
  deterministic signing tests, 8 verification tests (2 valid, 4 invalid by
  content, 2 invalid by length).
- The second implementation derives the same public key from the same
  seeds, produces a byte-identical deterministic signature, and each
  accepts the other's signature and rejects a changed message or context.
- A suite-2 hybrid signature's SLH-DSA half verifies, by the second
  implementation, against a representative rebuilt in the test.
- Every behavioural test of the hybrid signature runs for both suites; a
  signature never verifies under the other suite.
- A pinned vector for suite 2. The pinned vector for suite 1 was unchanged
  by the code changes that added suite 2.

## Not decided here

| Topic | Where |
|---|---|
| How a root certifies release keys, thresholds, rotation | Update-system ADR |
| Hybrid key encapsulation (X25519 + ML-KEM-1024) | With the backup and transfer format, its first user |
| The implementation used on the device | When AOSP integration starts; BoringSSL, already in AOSP, is the likely choice |
| How release keys are held in offline hardware, and whether that hardware supports ML-DSA | Key-ceremony document, Phase 7 |

## Consequences

- **Positive:** post-quantum signatures from the first signed artefact;
  the strongest verification evidence available for a Rust implementation;
  no C toolchain in the host tools; a construction small enough to
  re-implement from this page, which the tests do.
- **Negative:** sizes. A suite-1 public key is 2,624 bytes and a signature
  4,691 bytes before encoding (about 5.3 KB and 9.6 KB as files). A suite-2
  public key is only 96 bytes, but a signature is 29,856 bytes (about
  60 KB as a file).
- **Negative:** the pairing is not a registered standard combination, so
  off-the-shelf composite-signature verifiers will not accept it.
- **Risks:**
  - `libcrux` is at version 0.0.x and only partly verified. Mitigation:
    exact pins, the wrapper, the standard vectors and the second
    implementation in CI.
  - Signing in software exposes keys to the host. This is acceptable for
    test keys only.
  - Verification accepts whatever encodings the Ed25519 implementation
    accepts. NottOS never treats a signature as an identifier, and the
    ML-DSA half must verify as well.

## Revisit when

- The IETF composite specification is published. Add its matching pairing
  as a new suite if interoperability is worth more than the current choice.
- A fully verified ML-DSA implementation or an audit becomes available.
- AOSP integration starts (choose the on-device implementation).

## Verification

`cargo test -p nott-crypto`, run by `./nott check` and CI.
