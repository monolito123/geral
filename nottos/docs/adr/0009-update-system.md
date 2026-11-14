# ADR-0009: Update metadata and trust roots

- **Status:** Accepted (2026-11-13; decided under the owner's delegation)
- **Date:** 2026-11-13
- **Decider:** project owner (delegated)

## Context

The brief requires signed, atomic A/B updates with automatic rollback on a
failed boot, delta updates, anti-rollback counters, and hybrid
post-quantum signatures with SLH-DSA for long-lived roots. The threat model
(A8) adds: release signing by more than one person, a client that enforces
signature, version and anti-rollback regardless of the server, and no
device identifier sent to the server.

This ADR covers the part that decides **whether an update is authentic and
current**. Writing the update to the inactive slot, delta payloads and
boot-time rollback belong to AOSP's `update_engine` and the bootloader, and
need the AOSP build host.

The attacks a client must resist are well catalogued by The Update
Framework (TUF) `[K]`: arbitrary software (forged metadata), rollback (an
older release offered as new), indefinite freeze (stale metadata replayed
for ever), mix-and-match (pieces of different releases combined), endless
data, and compromise of a single key.

## Options considered

### Option A — a single signed manifest

The release key signs the build manifest; the device installs anything
newer.

- Simple. No defence against freeze; one key compromise is total; no way
  to replace a compromised key without reflashing devices.

### Option B — TUF itself, with an existing library

- A specification with years of public review and real deployments `[K]`.
- Existing Rust implementations support Ed25519, ECDSA and RSA, not hybrid
  post-quantum signatures. Using one would mean either dropping the
  post-quantum requirement or maintaining a fork of its key and signature
  handling, which is the most security-sensitive part.
- TUF's four roles (root, targets, snapshot, timestamp) exist to manage
  many independently changing files. A NottOS release is one unit, bound
  by one build manifest.

### Option C — TUF's design, reduced to what one release needs, in NottOS formats

Keep TUF's rules where they defend against the attacks above; keep its
role separation where it matters (offline root, separate release keys);
implement it in NottOS's canonical formats with the hybrid suites of
ADR-0008.

## Decision

**Option C.** Two roles and two documents.

| Document | Signed by | Context | Says |
|---|---|---|---|
| Root metadata (`nottos-root 1`) | A threshold of root keys | `root-metadata` | Version, expiry, the root keys and their threshold, the release keys and their threshold |
| Update metadata (`nottos-update 1`) | A threshold of the release keys the current root names | `update-metadata` | Channel, target, release sequence number, issue and expiry times, digest and size of the build manifest |

The build manifest (ADR-0008, `build-manifest`) lists every image by size
and digest, so the update metadata binds the whole release.

### Keys

| Role | Suite | Threshold (default) | Held |
|---|---|---|---|
| Root | 2: Ed25519 + SLH-DSA-SHA2-256s | 2 of 3 | Offline, by three people |
| Release | 1: Ed25519 + ML-DSA-87 | 2 of 3 | Offline signing hardware |

The client refuses a root whose roles use any other suite. Changing suites
is a new policy version, not a silent acceptance.

### Rules the client enforces

1. **Shipped root.** The root on the verified system image is trusted, but
   must be signed by a threshold of its own root keys.
2. **Root rotation** (TUF section 5.3). A new root must have exactly the
   next version and be signed by a threshold of the *current* root keys and
   a threshold of the *new* root keys. A key the old root never trusted
   cannot introduce itself; a root nobody can sign for cannot be adopted.
   Intermediate roots may have expired; only the final one must be current.
3. **Authenticate, then interpret.** Update metadata is parsed only after a
   threshold of release keys has signed its exact bytes. Signatures by keys
   outside the role, invalid signatures and signatures for another purpose
   count for nothing.
4. **Freshness.** Update metadata and the root carry expiry times. The
   device clock is not believed below a *time floor*: the latest `issued`
   time of any update metadata the device has accepted. Setting the clock
   back cannot make stale metadata current.
5. **Anti-rollback.** The release sequence number must be strictly greater
   than the installed one. On a device this is backed by the bootloader's
   tamper-evident rollback index, which is the final authority.
6. **Binding.** The channel and target must be the device's own. The build
   manifest must have the digest the metadata names; the downloader stops
   reading at the size the metadata names.
7. **Nothing identifies the device.** The client fetches public files by
   name. Whether an update applies is decided on the device.

### Mapping to TUF's attacks

| Attack | Defence here |
|---|---|
| Arbitrary software | Threshold of release keys over the exact bytes; manifest digest; image digests |
| Rollback | Strictly increasing sequence number; bootloader rollback index |
| Indefinite freeze | Expiry of update metadata and of the root; time floor |
| Mix-and-match | One document binds one manifest, which binds every image |
| Endless data | Sizes in the metadata and the manifest |
| One compromised key | Thresholds; root rotation removes keys |
| Compromised root key | Threshold of root keys; rotation needs old and new thresholds |

## What is deliberately left out, and the cost

- **No timestamp role.** In TUF an online key re-signs a short-lived
  timestamp file so that release keys can stay offline while the freeze
  window stays short. Here the freeze window is the update metadata's
  lifetime, which release signers must renew by hand. Proposed lifetime:
  30 days `[P]`. Adding a timestamp role later is a new document type and
  does not change the two above.
- **Fast-forward.** An attacker who holds a threshold of release keys can
  sign a very large sequence number. Devices that install it can then not
  be offered a lower one, even after the keys are rotated. TUF handles this
  by resetting version state when the relevant keys rotate; that reset
  needs care so that it does not reopen rollback, and is left to the
  implementation of the on-device client. Until then, the threshold is the
  defence.
- **Transparency log** (threat model A8): planned with the release
  infrastructure, Phase 7.

## Consequences

- **Positive:** the client's decision is a pure function of trusted state,
  documents and time, so every attack above has a test; release keys can
  be replaced without reflashing; the root's post-quantum assumption is
  independent of the release keys'.
- **Negative:** a custom implementation of a published design, so its
  correctness rests on NottOS's own tests and review rather than on TUF's
  implementations. Mitigations: the scenario tests, mutation testing of
  the verifier, and the planned independent audit.
- **Negative:** root signatures are large (about 60 KB each as files).
  Root rotations are rare.

## Verification

`core/update/tests/scenarios.rs`, 14 scenario tests. Every check in the
verifier was deliberately broken once to confirm that a test fails. One
such change (ignoring the manifest size) was not caught because the digest
comparison already decides; that redundant check was removed. Command-line
coverage: `tools/nott-tools/tests/cli.rs`.

Not verified: anything on a device. There is no on-device client yet, no
persistent storage of the trust state, and no connection to the
bootloader's rollback index.
