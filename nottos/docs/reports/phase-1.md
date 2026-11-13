# Phase 1 report — build system and emulator target

- **Date:** 2026-11-13
- **State:** host side complete and verified; emulator side not started.
  **No Phase 1 acceptance criterion is met.**

Phase 1's goal is a NottOS system image that boots on the Cuttlefish
emulator with verified boot and encrypted storage. That needs an AOSP build
host (at least 64 GB of RAM, 400 GB of disk, hardware virtualisation), which
the project does not have. What could be done without it was done: the host
tools the phase calls for, continuous integration, and a small Linux test
target on which the phase's checks run for real. The test target is not
NottOS and its results do not count toward the acceptance criteria.

## 1. What was built

| Item | What it is | Where |
|---|---|---|
| `nott-format` | Build manifest, format version 1: one canonical byte form per manifest, strict parser | `core/format` |
| `nott-policy` | Device capability descriptor and tier assessment per ADR-0002 | `core/policy` |
| `nott-tools` | One binary, seven checks: manifest create and verify, reproducibility comparison, image-size budget, SPDX bill of materials, tier check, storage policy check (INV-02) | `tools/nott-tools` |
| `nott` | Single entry point: `build host`, `build mini`, `check`, `repro`, `sbom`, `mini test`, `mini shell` | `nott` |
| Emulator descriptor | Capability descriptor for Cuttlefish, every answer "unknown" because nothing has been observed there | `device/cuttlefish` |
| Minimal Linux test target | Distribution kernel, two-stage init, EROFS system image under dm-verity, encrypted data disk, test runner | `tests/mini` |
| Continuous integration | Two jobs on every change under `nottos/`; the third-party action is pinned by commit hash | `.github/workflows/nottos.yml` in the repository root |
| Decisions | ADR-0003 (64-bit only), 0004 (source management), 0005 (licensing), 0006 (reproducibility), 0007 (test target); ADR-0001 and 0002 accepted; the seven questions recorded as decided | `docs/adr`, `docs/requirements-review.md` |
| Licence | Apache-2.0 text, notice, and an SPDX identifier in every source file | `LICENSE`, `NOTICE` |

## 2. What was verified, and how

Everything below was run in this development environment (Ubuntu 24.04,
Rust 1.97.0, QEMU 8.2.2 without hardware virtualisation) and again on
GitHub-hosted runners. CI runs:
[43317805047](https://github.com/monolito123/geral/actions/runs/43317805047),
[43361781483](https://github.com/monolito123/geral/actions/runs/43361781483),
[43362195043](https://github.com/monolito123/geral/actions/runs/43362195043).

| # | Claim | How it was checked | Result |
|---|---|---|---|
| V1 | The code is tested | `./nott check`: 50 tests (15 format, 10 policy, 19 tool unit tests, 6 end-to-end tests that run the binary on real files) | Pass, here and in CI |
| V2 | A manifest has exactly one byte form | Every single-bit change of a sample manifest is rejected or parses to a different manifest; 300 generated manifests survive a round trip byte for byte | Pass |
| V3 | Verification detects tampering | End-to-end test: an image with one byte changed, a truncated image, a missing image, a wrong verified-boot key and an edited manifest each make `manifest verify` fail | Pass |
| V4 | Code quality gates hold | Formatting, lints with warnings as errors, shell lint, licence-header check | Pass. The licence check was shown to fail when a header is removed. |
| V5 | Builds are reproducible across paths | `./nott repro`: two builds with different source paths and different Cargo homes | Identical output |
| V6 | The reproducibility check can fail | Same procedure without path remapping | Different binaries (1,108,448 and 1,110,688 bytes) |
| V7 | Builds are reproducible across machines | The digest printed by `./nott repro` here and on a GitHub runner, for commit `7ce3ae9` | Identical: `5f47b223f3a999e46a350dd41405c11c0339daa7c0244660d449013481d2eb3f` |
| V8 | The bill of materials is well-formed | `./nott sbom`, then validation against the official SPDX 2.3 JSON schema; generated twice | Valid; 33 packages; byte-identical on regeneration |
| V9 | The test target boots from a verified image and protects stored data | `./nott mini test`: 21 checks over 8 boots, table below | Pass, here and in CI |

### Boot tests on the minimal Linux test target

| Test | What was done | What happened |
|---|---|---|
| T1 | Booted the untampered images | Reached the shell marker from the dm-verity-protected system image; a 64 KiB file read back with the expected digest |
| T2 | Flipped one bit in a block that boot reads | The kernel logged dm-verity corruption; the second stage could not start; the shell marker never appeared |
| T3 | Flipped one bit in a block that boot does not read | Booted normally; the kernel logged the corrupted block and reading the affected file failed with an I/O error; the tampered data was never returned |
| T4 | Booted with a boot image carrying a different root hash | The system image could not be mounted (I/O error on its first block); the shell marker never appeared |
| T5 | Wrote secrets, powered off, searched the raw disk; then booted without the passphrase, with a wrong one, with the right one | File content, file name and unprotected content were all absent from the raw image. Without the right passphrase the directory showed one entry whose name was a long string of ciphertext, and the content could not be read. With it the content was intact. |
| T6 | Collected the mount table and ran `nott-tools storage check` | All 8 mounts classified as verified read-only, volatile, kernel interface, encrypted, or the one listed exception |

Controls, so that a pass means something: the raw-disk search found the
plaintext canary on the deliberately unencrypted scratch disk, and the
storage check failed when the exception list was withheld.

A boot takes about five seconds in pure emulation; the whole suite takes
about a minute. Both machines used kernel 7.0.0-43-generic from the
distribution.

The test target's own images are not reproducible across machines and are
not meant to be: they contain copies of the host's tools and libraries. The
root hash differed between this environment and CI.

## 3. Acceptance criteria

All nine are **open**. Each is about the Cuttlefish target.

| ID | Criterion (short) | State | What exists toward it |
|---|---|---|---|
| 1.1 | One command builds from a clean checkout | Open | `nott build` works for the host tools and the test target; there is no OS image target |
| 1.2 | Emulator boots, locked to the NottOS key, verity enforcing | Open | Analogue: T1 |
| 1.3 | Tampered system image is caught | Open | Analogue: T2 and T3. The criterion was reworded (section 5). |
| 1.4 | Image signed with a different key does not boot | Open | No analogue. Nothing here verifies a signature yet. |
| 1.5 | User data and file names absent from raw storage | Open | Analogue: T5 |
| 1.6 | Every mount verified, volatile or encrypted | Open | The checker is implemented and tested; analogue: T6 |
| 1.7 | Two clean builds produce identical images | Open | The method is implemented and verified for the host tools (V5 to V7) |
| 1.8 | SBOM lists every component | Open | Done for the host tools only (V8) |
| 1.9 | System image size within budget | Open | The gate is implemented and tested; there is no image to measure |

Other Phase 1 deliverables:

| Deliverable | State |
|---|---|
| Manifest pinned to an Android 17 tag; Cuttlefish product targets | Not started (needs the build host and the organisation, ADR-0004) |
| CI stage A | Running for the host side |
| First version of the tier checklist tool | Works on descriptors; has not been run against a live emulator |
| ADRs: 32-bit support, source management, licensing, reproducibility | Written and accepted (0003 to 0006) |

## 4. What is still UNVERIFIED

- **Everything about Android.** No AOSP checkout, build, boot or CTS run has
  happened.
- **Verified boot in the real sense.** The test target has no bootloader, no
  signature check and no root of trust; QEMU loads whatever boot image it is
  given.
- **Key protection.** The test target's disk key sits in its boot image. On
  a phone it is held by the TEE.
- **Kernel hardening.** The test target runs an unmodified distribution
  kernel.
- **Reproducibility beyond one distribution release.** Both machines in V7
  run Ubuntu 24.04 with the same Rust version. Other hosts are untested,
  and dependencies are fetched (with checksum verification) rather than
  vendored.
- **The tier tool against a real device or emulator.**
- **Every performance budget.**

## 5. Corrections to earlier work

Found while doing the work, and fixed:

1. **Roadmap criterion 1.3 was imprecise.** It said one flipped byte in the
   system image prevents boot. dm-verity checks a block when it is read, so
   a change in a block boot never reads does not stop the boot; it makes
   that read fail. The criterion now states both cases.
2. **The first reproducibility check could not fail.** Both builds shared
   one Cargo home, and a control run without path remapping still produced
   identical binaries. The check now also varies the Cargo home, and the
   control produces different binaries (V6).
3. **ADR-0002's table required port-controller USB disable for Tier A.** The
   brief's definition of Tier A does not. It is now a posture item.

## 6. Known risks

| Risk | Effect | Response |
|---|---|---|
| No build host | Phase 1 cannot be completed; later phases can only advance on the host side | Owner action (below) |
| The test target is mistaken for progress on Android | False confidence | ADR-0007 rules; this report's wording; analogues never close a criterion |
| The test target follows the distribution's newest kernel | A kernel update could break the tests for reasons unrelated to NottOS | The kernel version is recorded on every run; pin it if this happens |
| The encryption analogue uses ext4's legacy key tool | Not the mechanism Android uses | Replace when the storage work of Phase 2 gives a reason to |
| 30 third-party crates in the host tools' dependency closure | Supply-chain exposure | All pinned with checksums and listed in the SBOM; no audit tooling yet |
| CI runs on GitHub-hosted machines | Results depend on a third party's infrastructure | Acceptable for tests; release builds will not happen there |

## 7. Owner actions that block progress

1. **A build host**: 64-bit x86, at least 64 GB of RAM and 400 GB of free
   disk, with hardware virtualisation. Without it Phase 1 stays open.
2. **A dedicated organisation** for the NottOS repositories (ADR-0004).
   Without it no AOSP project can be patched.
3. **Three key holders** for release signing, before Phase 7.
4. **Partner outreach and a candidate Tier A device** for the early
   hardware test.

## 8. Plan for the next step

Two tracks, in this order of preference.

**When the build host exists:** complete Phase 1 on Cuttlefish. The tools
for criteria 1.5 to 1.9 are ready; the work is the manifest, the product
targets, the signing set-up and running the checks.

**Until then:** the parts of Phase 2 that are real product code and fully
testable on a host and on the test target:

1. `nott-crypto`: hybrid signatures (Ed25519 with ML-DSA) and hybrid key
   encapsulation (X25519 with ML-KEM-1024), composed from vetted
   implementations and checked against the standards' test vectors. ADR:
   cryptographic suite.
2. Signed build manifests, and the external verification tool that checks a
   release from another computer.
3. Update metadata verification: both signatures, expiry, version
   monotonicity. ADR: update system.
4. On the test target: a boot stage that verifies the signed manifest
   before trusting the root hash. This gives criterion 1.4 its analogue and
   is the first NottOS product code to run during a boot.

None of that closes a Phase 1 or Phase 2 acceptance criterion on its own.
It removes risk from them and produces code that ships.
