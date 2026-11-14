# Phase 2 progress report — signatures and update verification

- **Date:** 2026-11-14
- **State:** Phase 1 is still blocked on the AOSP build host. Work moved on
  to the parts of Phase 2 that are product code and can be fully tested on
  a host. **One Phase 2 acceptance criterion is met (2.10, hybrid
  cryptography).** The others need AOSP, a device, or components not yet
  built.

## 1. What was built

| Item | What it is | Where |
|---|---|---|
| Suite 1, `ed25519+ml-dsa-87` | Hybrid signature for release and working keys; both halves sign one representative that names the construction, suite and purpose | `core/crypto`, ADR-0008 |
| Suite 2, `ed25519+slh-dsa-sha2-256s` | Hybrid signature for root keys, whose post-quantum half rests only on the hash function | `core/crypto`, ADR-0008 (amended) |
| Key, signature and signature-set formats | Canonical text, versioned, one byte form per value; a suite selects its algorithms | `core/format/src/signature.rs` |
| Timestamps | `YYYY-MM-DDTHH:MM:SSZ`, strict | `core/format/src/time.rs` |
| Root and update metadata formats | Roles with thresholds; channel, target, sequence, issue and expiry times, manifest digest | `core/format/src/update.rs` |
| Update verifier | Shipped-root check, root rotation, update verification, manifest check; pure functions | `core/update`, ADR-0009 |
| Host tool commands | `key generate --suite`, `key id`, `sign`, `verify`, `sigset`, `update root-create`, `update metadata-create`, `update verify`; `manifest verify` with signature and `--only` | `tools/nott-tools` |
| Signed boot on the test target | The boot image carries only the release public key; stage 1 accepts a dm-verity root hash only from a signed build manifest | `tests/mini` |
| SBOM correction | The SBOM now lists exactly what the build compiles | `nott`, `tools/nott-tools/src/sbom.rs` |
| Authenticated encryption and passphrase stretching | ChaCha20-Poly1305 and Argon2id behind `nott-crypto` | `core/crypto/src/aead.rs`, `password.rs`, ADR-0013 |
| Backup encryption | A byte stream encrypted to a recovery key derived from the recovery secret; random 256-bit data key wrapped with the hybrid KEM; header and every chunk authenticated; the device can make backups but not read them | `core/backup`, `core/format/src/backup.rs`, ADR-0013 |
| Host tool commands | `backup key-create`, `backup encrypt`, `backup decrypt` | `tools/nott-tools` |

Implementations used, all pinned to exact versions: `libcrux-ed25519`
0.0.10, `libcrux-ml-dsa` 0.0.11, `fips205` 0.5.0, `libcrux-ml-kem` 0.0.11,
`libcrux-curve25519` 0.0.9, `libcrux-chacha20poly1305` 0.0.10, `argon2`
0.6.0. Second implementations, in tests only: RustCrypto `ml-dsa` 0.1.2,
`slh-dsa` 0.1.0, `ml-kem` 0.3.4, `chacha20poly1305` 0.10.1, `x25519-dalek`
3.0.0 and `rust-argon2` 3.0.0.

## 2. What was verified, and how

All of the following ran in this development environment; the commits
also ran on GitHub-hosted runners (CI runs 5 to 14 of the `nottos`
workflow).

| # | Claim | How it was checked | Result |
|---|---|---|---|
| V1 | The code is tested | `./nott check`: 128 tests when first written, 184 now | Pass |
| V2 | Ed25519 is called correctly | RFC 8032 section 7.1, tests 1 to 3 | Pass |
| V3 | ML-DSA-87 is called correctly | NIST ACVP: 25 key generation, 2 deterministic signing, 6 verification (2 valid, 4 invalid) | Pass |
| V4 | SLH-DSA-SHA2-256s is called correctly | NIST ACVP: 10 key generation, 2 deterministic signing, 8 verification (2 valid, 4 invalid by content, 2 by length) | Pass |
| V5 | Implementations agree with independent ones | Same seeds give the same public keys; deterministic signatures are byte-identical; each accepts the other's signatures and rejects a changed message or context | Pass, for both ML-DSA-87 and SLH-DSA-SHA2-256s |
| V6 | The hybrid construction is what ADR-0008 says | Tests rebuild the representative from the ADR's text and verify each half against it, the post-quantum half with the second implementation | Pass, both suites |
| V7 | Both halves are required | Each half broken alone, halves from different messages, halves from different keys, a relabelled purpose, a signature under the other suite: all rejected | Pass, both suites |
| V8 | Formats are canonical | Bit-flip tests on signatures and update metadata; round trips of generated documents; every day from 1970 to 2500 for timestamps | Pass |
| V9 | Adding suite 2 did not change suite 1 | The pinned suite-1 test vector (public key and signature digests) is unchanged | Pass |
| V10 | The update verifier stops the attacks it is for | 14 scenario tests: forged and edited metadata, missing threshold, keys outside the role, wrong purpose, replay, rollback, expiry, clock set back, expired root, wrong channel or target, wrong manifest, rotation with only old or only new keys, version skipping, keys rotated out | Pass |
| V11 | Those tests can fail | Each check in the verifier was broken once on purpose (11 changes) | 10 caught at once. One missing test was found and added (a root skipping a version). One check was redundant (manifest size, already decided by the digest) and was removed. |
| V12 | The test target refuses unauthentic boot metadata | Boot tests T4a (untrusted key), T4b (manifest edited after signing), T4c (root hash replaced): stage 1 refuses each, with the reason in the log | Pass, here and in CI |
| V13 | Builds stay reproducible | `./nott repro`, here and on GitHub runners | Identical across paths and Cargo homes. Identical between this machine and a GitHub runner for commit `7ce3ae9` (before the cryptography crates) and for commit `54832f7` (with them): `10e4864a32c1df9a095435911f9b2ae1c333a24ad7248a9c72165a2840f69369`; and for commit `26f18b8` (with the backup crates): `d08b9f0495edd09b921d033131a81067cd83cef41dcde956cb72f0f4f86e5475`. |
| V14 | The SBOM is exact | Compared with Cargo's own list of compiled packages; SPDX 2.3 schema | Identical (55 packages when first written, 70 now); valid |
| V15 | ChaCha20-Poly1305 and Argon2id are called correctly | RFC 8439 (2.8.2, A.5); RFC 9106 (5.3); the eight Argon2id cases of the reference implementation; agreement with RustCrypto `chacha20poly1305` and `rust-argon2` | Pass |
| V16 | The backup construction is what ADR-0013 says | A backup made by `nott-backup` is opened by a test that rebuilds every step from the ADR with the second implementations only | Pass |
| V17 | Backups stop the attacks they are for | 17 tests: a backup made with the public parts but not the authentication key; every header byte changed; cut, extended, reordered, dropped and foreign chunks; an empty final chunk; wrong secret; cost limits | Pass |
| V18 | Those tests can fail | Each of 20 checks and derivation inputs broken once on purpose | 19 caught at once; the memory minimum was masked by the work minimum, so a test was added |

Measured on this machine (x86-64, no SIMD paths): SLH-DSA-SHA2-256s
public-key derivation about 35 ms, signing about 0.6 s, verification under
1 ms.

## 3. Phase 2 acceptance criteria

| ID | Criterion (short) | State | What exists toward it |
|---|---|---|---|
| 2.1 | No permissive SELinux domain | Open | Needs AOSP |
| 2.2 | Syscall filter for every process class | Open | Needs AOSP |
| 2.3 | No writable-executable memory | Open | Needs AOSP |
| 2.4 | Kernel hardening checker passes | Open | The profile and checker exist (ADR-0011). There is no Android kernel to pass it; the test target's distribution kernel fails 28 of 84 rules, as expected. |
| 2.5 | CTS still passes with hardening | Open | Needs AOSP |
| 2.6 | Keys evicted on lock and reboot | Open | Not started |
| 2.7 | Duress credential makes data unrecoverable | Open | Not started |
| 2.8 | Valid update installs; bad classical signature, bad post-quantum signature or lower version refused | Open | The refusals are tested in the verifier (V7, V10). Installation needs `update_engine`. |
| 2.9 | Failed new slot reverts | Open | Needs AOSP and a bootloader |
| 2.10 | Hybrid constructions pass standard vectors and interoperate with a second implementation | **Met** | Signatures: V2 to V6. Key encapsulation (X25519 + ML-KEM-1024, ADR-0012): RFC 7748 and NIST ACVP vectors, agreement with RustCrypto `ml-kem` and `x25519-dalek`, combiner recomputed independently. The criterion is about the constructions on a host; their use on a device is covered by other criteria. |
| 2.11 | Integrity folder cannot be modified | Open | Analogue on the test target: T7a (eight write attempts by root refused) and T7c (a changed block cannot be read). The SELinux layer needs AOSP. |
| 2.12 | Verify system passes on a genuine image and fails on a tampered manifest; an external tool reproduces the hashes | Open | Analogue on the test target: the in-system check passes (T7a) and fails on a damaged manifest (T7c); the external tool and GnuPG verify from the host (T7b). The app does not exist. |

## 4. What is still UNVERIFIED

- Everything on a device or on Cuttlefish.
- That the cryptographic implementations are free of side channels on real
  hardware: `libcrux` and `fips205` make claims; nothing here measured
  them.
- That the implementations are correct beyond the vectors and the
  agreement tests: neither ML-DSA nor SLH-DSA implementation has been
  audited, and `libcrux` is formally verified only in parts.
- Persistent trust state on a device, its connection to the bootloader's
  rollback index, and the downloader that must honour `manifest-size`.
- Release signing in offline hardware, and whether such hardware supports
  ML-DSA and SLH-DSA.
- How long Argon2id takes on a phone. The default (512 MiB, three passes)
  took 1.76 s on this machine; nothing was measured on a device.
- That a recovery passphrase typed on two devices gives the same bytes:
  Unicode normalisation belongs to the setup and restore screens, which do
  not exist.
- The network seal on Android, on radios, and against raw packet sockets
  that bypass the queueing layer (ADR-0014). Only the test target, one
  wired virtual interface, was measured.

## 5. Corrections to earlier work

1. **The SBOM listed packages the build does not compile.** Found when the
   cryptography crates pushed Cargo's metadata to 105 packages. Fixed; the
   Phase 1 SBOM had one such extra package, recorded in the Phase 1 report.
2. **One test expectation was wrong** during the suite-2 work (a size limit
   is checked before the header). The code was right; the test was fixed
   to match the earlier, correct expectation.
3. **A test-only dependency resolved to an incompatible pre-release**
   (`signature` 2.3.0-pre.7 for `slh-dsa` 0.1.0). Pinned to the version it
   declares. Nothing that ships is affected.

## 6. Known risks

| Risk | Response |
|---|---|
| Young, partly verified or unaudited cryptographic implementations | Exact pins, one wrapper crate, standard vectors and second implementations in CI; replaceable behind `nott-crypto` |
| The update verifier is a new implementation of a published design | Scenario tests, mutation testing, planned independent audit |
| No timestamp role: the freeze window equals the update metadata lifetime | 30 days proposed; a timestamp role can be added without changing existing documents (ADR-0009) |
| Fast-forward by an attacker holding a threshold of release keys | Threshold signing; on-device reset rule still to be designed (ADR-0009) |
| Test time grows with hash-based signatures | Signatures made once per test run and shared; whole suite about 20 s |
| The backup key schedule is NottOS's own, built from standard parts | Written construction, independent recomputation in tests, mutation testing, planned audit (ADR-0013) |
| Replay of an old backup to a new device | The creation time is authenticated and shown; a new device has no state to do more (ADR-0013) |

## 7. Owner actions that block progress

Unchanged from the Phase 1 report: an AOSP build host, a dedicated
organisation for the repositories, three release-key holders, and partner
outreach with a candidate Tier A device.

## 8. Done since this report was first written

- **Integrity folder** (ADR-0010). Building it exposed a conflict in the
  brief (requirements review, C-25): a manifest inside the system image
  cannot list that image's digest. The folder now has its own verified
  partition. Tests T4d and T7a to T7c on the test target. During this work
  one test hung: `rm` on a write-protected file stopped to ask for
  confirmation on the console. Every write attempt now runs with empty
  input, and the time limit per boot is down from five minutes to two.

- **Kernel hardening profile and checker** (ADR-0011): 95 rules derived from
  the Kernel Self Protection Project's list, each deviation justified;
  `nott-tools kernel check`. Run against the test target's distribution
  kernel: 28 of 84 x86-64 rules not satisfied. Running it on a real
  configuration exposed a parser bug (option names may contain lower-case
  letters), fixed before the result was recorded.

- **Hybrid key encapsulation** (ADR-0012): X25519 with ML-KEM-1024, the
  combiner shape of the IETF drafts with a NottOS label; 17 tests; every
  combiner input and check removed once on purpose, each removal caught.
  This completes criterion 2.10.

- **Backup encryption** (ADR-0013). The age format was the first
  candidate. Its post-quantum recipient uses ML-KEM-768, its file key is
  128 bits, and its recipients are not authenticated, so anyone holding the
  public key could make a backup the owner's device would restore. NottOS
  keeps age's payload layout and adds its own key schedule: the recovery
  keys come from the recovery secret through Argon2id, and the data key is
  wrapped under a key that needs both the hybrid KEM secret and an
  authentication key the device holds. Host analogue of Phase 5 criterion
  5.10: restoring on another machine needs only the recovery secret, and a
  damaged wrapper is refused.

- **Network seal on the test target** (ADR-0014; Phase 4 work that needs
  only a Linux kernel). The first version used nftables and passed the
  sealed-boot test. Then the Android generic kernel's configuration turned
  out to have no nftables `[V:F-38]`, so the seal was rebuilt from what
  that kernel does have: a match-all drop on each interface (tc), the IPv4
  and IPv6 filter tables, and the ARP filter table, applied in that order.
  Stage 1 loads the packet-filter modules before any other driver and
  stops the boot if the seal is incomplete. QEMU records the virtual link
  from before the machine starts:
  - T8a, sealed throughout: no frame sent, while DHCP, ARP, ICMP, DNS,
    TCP and IPv6 were tried and the host probed in.
  - T8b, open then sealed: every kind seen while open, none after
    locking.
  - T8c: with a filter module blocked, the boot stops and nothing is
    sent.

  A first version of T8b was ambiguous: it let the host probe during the
  open phase, so a reply could not be placed before or after the moment of
  sealing. Probes now start only once the machine reports that it is
  sealed. The test target now runs 80 checks.

## 9. Next steps that need none of those

1. **Storage key hierarchy ADR** (planned): key classes, evict-on-lock, and
   where the backup authentication key lives while the device is locked.
2. **Socket destruction on sealing** (criterion 4.9) and **re-sealing new
   interfaces** on the test target: both kernels have the facilities
   (`INET_DIAG_DESTROY`, netlink interface events).
