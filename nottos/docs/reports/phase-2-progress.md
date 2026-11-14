# Phase 2 progress report — signatures and update verification

- **Date:** 2026-11-14
- **State:** Phase 1 is still blocked on the AOSP build host. Work moved on
  to the parts of Phase 2 that are product code and can be fully tested on
  a host: hybrid signatures, signed boot metadata on the test target, and
  the verification half of the update system. **No Phase 2 acceptance
  criterion is fully met**; one is met for signatures only (2.10).

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

Implementations used, all pinned to exact versions: `libcrux-ed25519`
0.0.10, `libcrux-ml-dsa` 0.0.11, `fips205` 0.5.0. Second implementations,
in tests only: RustCrypto `ml-dsa` 0.1.2 and `slh-dsa` 0.1.0.

## 2. What was verified, and how

All of the following ran in this development environment; the commits
also ran on GitHub-hosted runners (CI runs 5 to 9 of the `nottos`
workflow).

| # | Claim | How it was checked | Result |
|---|---|---|---|
| V1 | The code is tested | `./nott check`: 128 tests | Pass |
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
| V13 | Builds stay reproducible | `./nott repro` | Identical across paths and Cargo homes. For commit `7ce3ae9` the binary digest was also identical between this machine and a GitHub runner. |
| V14 | The SBOM is exact | Compared with Cargo's own list of compiled packages; SPDX 2.3 schema | Identical (55 packages); valid |

Measured on this machine (x86-64, no SIMD paths): SLH-DSA-SHA2-256s
public-key derivation about 35 ms, signing about 0.6 s, verification under
1 ms.

## 3. Phase 2 acceptance criteria

| ID | Criterion (short) | State | What exists toward it |
|---|---|---|---|
| 2.1 | No permissive SELinux domain | Open | Needs AOSP |
| 2.2 | Syscall filter for every process class | Open | Needs AOSP |
| 2.3 | No writable-executable memory | Open | Needs AOSP |
| 2.4 | Kernel hardening checker passes | Open | Not started |
| 2.5 | CTS still passes with hardening | Open | Needs AOSP |
| 2.6 | Keys evicted on lock and reboot | Open | Not started |
| 2.7 | Duress credential makes data unrecoverable | Open | Not started |
| 2.8 | Valid update installs; bad classical signature, bad post-quantum signature or lower version refused | Open | The refusals are tested in the verifier (V7, V10). Installation needs `update_engine`. |
| 2.9 | Failed new slot reverts | Open | Needs AOSP and a bootloader |
| 2.10 | Hybrid constructions pass standard vectors and interoperate with a second implementation | **Met for signatures** | V2 to V6. Hybrid key encapsulation (X25519 + ML-KEM-1024) is not built, so the criterion is not met as a whole. |
| 2.11 | Integrity folder cannot be modified | Open | Not started |
| 2.12 | Verify system passes on a genuine image and fails on a tampered manifest; an external tool reproduces the hashes | Open | The external half exists: `nott-tools manifest verify` with signature. The on-device app does not. |

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

## 7. Owner actions that block progress

Unchanged from the Phase 1 report: an AOSP build host, a dedicated
organisation for the repositories, three release-key holders, and partner
outreach with a candidate Tier A device.

## 8. Next steps that need none of those

1. **Integrity folder.** Generate `/nott/attest` (release keys, signed
   manifest, verification README) in the build, place it on the test
   target's verified system image, and add an in-system check that
   recomputes the system partition hashes, as an analogue of criterion
   2.12. ADR: integrity folder.
2. **Kernel hardening checker.** A tool that reads a kernel configuration
   and reports every option of the hardening profile that is missing.
   Running it on the test target's distribution kernel will show, honestly,
   how far an unmodified kernel is from the profile. ADR: kernel hardening
   profile.
3. **Hybrid key encapsulation** (X25519 + ML-KEM-1024), with known-answer
   tests and a second implementation, as the base for encrypted backups.
