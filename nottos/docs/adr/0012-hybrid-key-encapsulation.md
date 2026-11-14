# ADR-0012: Hybrid key encapsulation

- **Status:** Accepted (2026-11-14; decided under the owner's delegation)
- **Date:** 2026-11-14
- **Decider:** project owner (delegated)

## Context

The brief (section 9) requires hybrid key encapsulation wherever an
asymmetric algorithm protects persistent data: X25519 with ML-KEM-1024
(FIPS 203), so that security holds if either is broken. Its first users
are key wrapping for backups and profile keys, and device-to-device
transfer.

Facts `[V:F-34]`:

- The IETF has two drafts for hybrid ML-KEM, both still drafts: X-Wing
  (X25519 with ML-KEM-768) and Composite ML-KEM (several pairings). Neither
  pairs X25519 with ML-KEM-1024.
- Both combine the secrets the same way: SHA3-256 over the ML-KEM shared
  secret, the classical shared secret, the classical ciphertext, the
  classical public key, and a label. The classical ciphertext and public
  key are included because X25519 alone does not bind them; the ML-KEM
  ciphertext is not.
- Rust implementations: `libcrux-ml-kem` (formally verified with hax and
  F\*; constant-time patterns checked on generated assembly, by its own
  statement); `libcrux-curve25519` ("formally verified curve25519 ECDH",
  code from HACL\*); RustCrypto `ml-kem` (states it has never been
  independently audited); `x25519-dalek`.

## Options considered

| Option | Assessment |
|---|---|
| X-Wing as specified (ML-KEM-768) | Interoperable once published, but below the brief's ML-KEM-1024 |
| **X25519 + ML-KEM-1024 with the drafts' combiner shape and a NottOS label** | Meets the brief; reuses the analysed construction; a different label keeps it from being mistaken for either draft |
| Concatenate both secrets and hash them alone | Rejected: does not bind the X25519 ciphertext and public key |

## Decision

```text
ss = SHA3-256(ml_kem_ss || x25519_ss || x25519_ct || x25519_pk || "NottOS-hybrid-kem-v1")
```

- Public key: X25519 public key (32 bytes) followed by the ML-KEM-1024
  encapsulation key (1,568 bytes). The ML-KEM half must pass the FIPS 203
  input check.
- Ciphertext: the sender's ephemeral X25519 public key followed by the
  ML-KEM-1024 ciphertext (1,600 bytes in all).
- Secret key: a 32-byte X25519 secret and the 64-byte ML-KEM seed `d || z`.
- An all-zero X25519 shared secret (a low-order point) is refused, as RFC
  7748 permits.
- A modified ML-KEM ciphertext still decapsulates, to an unrelated secret
  (FIPS 203 implicit rejection); whatever the secret protects must be
  authenticated, which the backup format will do.
- Implementations: `libcrux-ml-kem` 0.0.11 and `libcrux-curve25519` 0.0.9,
  pinned; RustCrypto `ml-kem` and `x25519-dalek` in tests only.

File formats for keys, ciphertexts and wrapped keys are decided with the
backup format, their first user.

## Consequences

- **Positive:** the shared secret survives a break of either algorithm;
  the strongest verification evidence available for both halves.
- **Negative:** not interoperable with X-Wing or Composite ML-KEM; 1,600
  bytes per encapsulation.
- **Risk:** the drafts change their combiner before publication. This
  construction would then stay as suite 1 and a new suite could follow the
  published form.

## Verification

`core/crypto/tests/kem.rs`, 17 tests:

- RFC 7748 X25519 vectors (sections 5.2 and 6.1); low-order points
  refused.
- NIST ACVP ML-KEM-1024 vectors: 25 key generation, 4 encapsulation, 4
  decapsulation (2 valid, 2 implicit rejection), 10 encapsulation-key
  checks (5 valid, 5 invalid), the last also through the hybrid key.
- Second implementations: same ML-KEM key from the same seed, identical
  ciphertext and secret for the same randomness, mutual decapsulation,
  identical implicit-rejection secrets; X25519 identical to
  `x25519-dalek`; the hybrid secret equals the combiner recomputed in the
  test from the formula above with both halves from the second
  implementations.
- Behaviour: round trip; byte forms; any altered half changes the secret;
  a different X25519 recipient key changes the secret even when the
  ML-KEM halves are identical; a pinned vector.
- Every combiner input and every check was removed once on purpose (8
  changes); each removal made a test fail.
