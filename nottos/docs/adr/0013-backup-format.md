# ADR-0013: Backup encryption format

- **Status:** Accepted (2026-11-14; decided under the owner's delegation)
- **Date:** 2026-11-14
- **Decider:** project owner (delegated)

## Context

The brief (section 9) asks for hybrid key wrapping of backups, authenticated
encryption, and a recovery passphrase stretched with a memory-hard function
(Argon2id or scrypt). The architecture (section 5.3) adds: the backup key
pair is a hybrid KEM whose private half is protected by a recovery
passphrase; each backup has a random 256-bit data key wrapped to it;
symmetric strength is 256 bits everywhere; every container starts with a
format version and a suite identifier; NottOS does not invent a
construction. Roadmap criterion 5.10: a backup restores on a second device,
and a backup with a damaged wrapper is refused.

Backups leave the device: a computer, removable media, a cloud service.
Whoever stores them may read, change, replace, cut, extend or replay them.
Two properties follow:

1. **Confidentiality** against everyone without the recovery secret,
   including after a quantum computer exists (hence the hybrid KEM,
   ADR-0012).
2. **Authenticity.** Restoring writes apps and data onto a device; a backup
   made by someone else must be refused. With public-key encryption alone,
   anyone who knows the public key can make a file that decrypts. The
   recovery public key is not secret, so encryption alone is not enough.

And one operational need: the device must make backups unattended, without
holding anything that can read them (keys are evicted on lock, ADR to come
on the storage key hierarchy).

Facts `[V:F-35]` `[V:F-36]` `[V:F-37]`.

## Options considered

### Option A — age v1 with its post-quantum recipient

The age format is published and analysed. Its post-quantum recipient uses
ML-KEM-768, below the brief's ML-KEM-1024. Its file key is 128 bits, below
"256 bits everywhere". A passphrase stanza must be the only stanza. And
recipients are not authenticated: anyone with the public key can make a file
the owner's identity opens. **Rejected**: three requirements unmet.

### Option B — age v1 file structure with a NottOS recipient

A custom stanza (X25519 with ML-KEM-1024) through the Rust `age` crate's
recipient interface. Still a 128-bit file key, still no authentication
(another layer would be needed), and the files would be unreadable by other
age tools anyway, so the main benefit of age, interoperability, is lost.
It would bring the `age` crate, including plugin machinery that NottOS does
not use. **Rejected.**

### Option C — HPKE (RFC 9180) in pre-shared-key mode

HPKE's PSK mode authenticates the sender by a shared key, which fits. But no
HPKE KEM identifier exists for X25519 with ML-KEM-1024 (neither hybrid draft
defines that pairing `[V:F-34]`), so this would be HPKE in name only, with a
private KEM. **Rejected for now**; revisit if such a KEM is registered.

### Option D — a NottOS container on age's payload layout

Age's payload layout unchanged (64 KiB chunks, ChaCha20-Poly1305, counter
and last-chunk flag in the nonce, the same rules for the final chunk). A
256-bit data key, wrapped once under a key derived from the hybrid KEM
secret and an authentication key. Both recovery keys derived from the
recovery secret with Argon2id. **Chosen.**

What is given up: interoperability with age tools, and the benefit of
published analysis for the key schedule. The payload layout is age's; the
wrapping and derivation are NottOS's, built only from standard parts
(Argon2id, SHAKE256, SHA3-256, ChaCha20-Poly1305, ADR-0012).

## Decision

### Keys

```text
recovery secret --Argon2id(salt, m, t, p, 32-byte output)--> seed
SHAKE256("NottOS-backup-recovery-v1" || seed) -> 128 bytes:
    X25519 secret (32) || ML-KEM-1024 seed d || z (64) || authentication key (32)
recovery public key = public half of that hybrid KEM key (ADR-0012)
recovery key id     = SHA-256(recovery public key)
```

- The salt is 16 random bytes, chosen when the backup key is made.
- The **backup key**, kept on the device, holds the parameters, the salt,
  the recovery public key and the authentication key. It can make backups;
  it cannot read them. The recovery secret is not stored anywhere.
- The recovery secret is taken as bytes: 1 to 1,024 of them.

### Each backup

```text
data key K   = 32 random bytes
(ct, ss)     = hybrid KEM encapsulation to the recovery public key
wrap key     = SHA3-256("NottOS-backup-wrap-v1" || ss || authentication key)
wrapped key  = ChaCha20-Poly1305(wrap key, nonce 0^12,
                 aad = header up to and including the kem-ciphertext line,
                 plaintext K)
payload key  = SHA3-256("NottOS-backup-payload-v1" || K)
chunk i      = ChaCha20-Poly1305(payload key,
                 nonce = i as 11 bytes big endian || last (1 byte),
                 aad = "", 64 KiB of plaintext; the final chunk may be shorter,
                 and is empty only if the whole payload is)
```

Every input to SHA3-256 and SHAKE256 has a fixed length, so concatenation is
unambiguous. The wrap key is used once (the KEM secret is fresh), so the
fixed nonce is safe; the payload key is used once per chunk number.

### File

```text
nottos-backup 1
suite: x25519+ml-kem-1024+chacha20-poly1305
created: 2026-11-14T00:00:00Z
kdf: argon2id <memory KiB> <iterations> <lanes> <salt, hex>
recovery-key-id: <hex>
kem-ciphertext: <hex, 1,600 bytes>
wrapped-key: <hex, 48 bytes>
<payload>
```

The header follows the rules of the other NottOS text formats: one canonical
byte form, fixed field order. The backup key file is the same style
(`nottos-backup-key 1`).

### Reading

1. Parse the header (at most 4,096 bytes read).
2. Check the Argon2id cost against the limits below, **before** deriving
   anything, so a hostile header cannot make the reader allocate more.
3. Derive the keys; a different recovery key id means the wrong secret.
4. Decapsulate and open the wrapped key: this authenticates every header
   line. A changed field, a header made without the authentication key, or
   a low-order X25519 point is refused as not authentic.
5. Decrypt chunk by chunk; each is released only after its tag verifies.
   A missing final chunk, data after it, an empty final chunk after others,
   reordered, dropped or foreign chunks are refused. Output must be
   discarded unless the whole payload verified; the host tool writes to a
   `.partial` file and renames it only then.

### Limits and parameters

| | Minimum | Maximum |
|---|---|---|
| Memory | 64 MiB | 2 GiB |
| Passes | 1 | 10 |
| Lanes | 1 | 16 |
| Memory × passes | 64 MiB × 3 | — |

Both of RFC 9106's recommended options meet these limits `[V:F-36]`. Default
for new backup keys: 512 MiB, 3 passes, 4 lanes `[P]`. On this development
machine (Intel Xeon at 2.1 GHz, one thread, since the implementation runs
lanes one after another) deriving the keys took 0.21 s at 64 MiB with 3
passes, 1.76 s at the default, and 3.54 s at 2 GiB with 1 pass. **Not
measured on a phone**; the default is to be set on Tier A hardware.

### Implementations

`libcrux-chacha20poly1305` 0.0.10 (from HACL\*; verifies the tag before
decrypting) and RustCrypto `argon2` 0.6.0 with its memory wiped, pinned.
ChaCha20-Poly1305 rather than AES-256-GCM: constant time in software on
every device, including those without AES instructions, and the formally
verified implementation is already from the library NottOS uses for its
other primitives. In tests only: RustCrypto `chacha20poly1305`,
`rust-argon2`, `ml-kem`, `x25519-dalek`.

## Consequences

- **Positive:** quantum-safe confidentiality; backups made by anyone without
  the authentication key or the recovery secret are refused; the device can
  back up without being able to read its backups; the payload layout is
  age's, already analysed; replacing the KEM later means re-wrapping the
  data key, not re-encrypting the payload (this needs the recovery secret).
- **Negative:** not readable by age tools; a NottOS key schedule to review;
  `argon2` 0.6 brings a second generation of RustCrypto's hashing traits
  (`digest` 0.11 next to 0.10, which `fips205` 0.5 and `sha3` 0.10 need);
  the host tools' bill of materials grew from 57 to 70 packages.
- **Risks and mitigations:**
  - *A compromised device can make backups the owner would accept.* It
    holds the data anyway; restoring shows the creation time.
  - *Replay of an old backup.* A new device has no state to detect it. The
    creation time is authenticated (by the old device's clock) and shown;
    the restore screen must prefer the newest. Residual.
  - *Size and time are visible* to whoever stores the backup. No padding.
  - *The same secret must give the same bytes on every device.* Generated
    recovery codes in ASCII avoid the problem; user-chosen passphrases need
    Unicode normalisation in the restore and setup screens (Phase 5, open).
  - *Weak user passphrases.* Argon2id slows guessing; it does not make a
    weak passphrase strong. Phase 5 should default to a generated code.
  - *Unaudited Argon2 implementation.* Vectors, a second implementation in
    tests, exact pin; replaceable behind `nott-crypto`.

Not built: a mode where the private half is kept on the device instead of
derived from a passphrase (architecture 5.3 mentions it), device-to-device
transfer, the archive layout inside the payload, and the restore user
interface.

## Revisit when

- An HPKE KEM, or an age recipient, with X25519 and ML-KEM-1024 is
  published.
- The hybrid KEM drafts change their combiner (ADR-0012).
- Timings on Tier A devices are available.
- An audit reports on the key schedule or the implementations.

## Verification

- `core/crypto/tests/aead.rs`: RFC 8439 section 2.8.2 and appendix A.5;
  agreement with RustCrypto `chacha20poly1305` for 11 lengths up to 64 KiB;
  every bit of ciphertext, tag, associated data, nonce and key changed once,
  each refused.
- `core/crypto/tests/password.rs`: RFC 9106 section 5.3 on the pinned
  implementation (it needs a secret and associated data, which NottOS does
  not pass); all eight Argon2id cases of the reference implementation's
  `test.c` through NottOS's wrapper; agreement with `rust-argon2`.
- `core/backup/tests/backup.rs`, 17 tests:
  - round trips at sizes 0, 1, 100, 64 KiB − 1, 64 KiB, 64 KiB + 1,
    128 KiB and 128 KiB + 17;
  - **the construction recomputed** from this ADR with the second
    implementations: the recovery key id matches, the wrapped key opens
    with the header bytes as associated data, every chunk opens;
  - a backup made with the public parts and a wrong authentication key is
    refused; every header byte changed once is refused; each kind of header
    change is named; cost limits are enforced before any key is derived;
  - cut at every chunk boundary, cut by one byte, data appended, chunks
    swapped, dropped or taken from another backup, an empty final chunk
    after others (made with the independently derived key): each refused;
  - a pinned vector.
- `core/format/src/backup.rs`: round trips, the documented layout, every
  byte of a header replaced eight ways (each change refused or giving a
  different header), non-canonical forms refused.
- `tools/nott-tools/tests/cli.rs`: key creation (file mode 0600, never
  overwritten), encryption, the wrong secret refused with nothing written,
  a damaged chunk refused with no partial output left.
- Mutation testing: each of the 20 checks and derivation inputs in
  `core/backup/src/lib.rs` was broken once. 19 were caught at once; one (the
  memory minimum, masked by the work minimum) needed a new test, now added.
