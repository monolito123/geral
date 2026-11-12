# Roadmap and acceptance criteria

- **Version:** 0.1 (Phase 0 draft, awaiting owner approval)
- **Date:** 2026-11-12

The phases and their order are the owner's (brief, section 15). This document
adds, for each phase, the deliverables, the acceptance criteria, how each
criterion is verified, what the phase needs, and what will still be
UNVERIFIED when it ends.

An acceptance criterion counts as met only when the stated check was actually
run and its output is attached to the phase report.

## What every phase report contains

1. What was built.
2. What was verified, and how (command, environment, result).
3. What is still UNVERIFIED.
4. Known risks.
5. Plan for the next phase.

The phase then stops until the owner approves.

## Prerequisites that are not yet met

| Need | For | Status |
|---|---|---|
| Build host: 64-bit x86, at least 64 GB RAM and 400 GB free disk `[V:F-22]`, hardware virtualisation for the emulator `[V:F-25]` | Phase 1 onward | **Missing.** The current environment has 15 GB RAM and 30 GB disk. |
| A place for many repositories (organisation) | Phase 1 onward | Missing; owner decision (Q6) |
| Larger build capacity for the browser engine | Phase 5 | Missing |
| Real devices: one Tier A, one per performance class; USB protocol analyser; power monitor | Phase 6 | Missing |
| Offline signing hardware and key holders | Phase 7 (test keys until then) | Missing; owner decision (Q5) |
| Independent auditor | Phase 7 | Not engaged |

Without the build host, Phases 1 to 5 can only deliver host-side Rust
components with unit tests and fuzzing. Their emulator-based criteria would
stay UNVERIFIED.

## Phase 0 — Architecture (this phase)

| # | Deliverable (brief, section 15) | Where |
|---|---|---|
| 1 | Conflicts and impossibilities, with proposed resolutions | [requirements-review.md](requirements-review.md) |
| 2 | Up to seven questions | [requirements-review.md](requirements-review.md), top |
| 3 | ADR-0001 and the threat model | [adr/0001-base-architecture.md](adr/0001-base-architecture.md), [threat-model.md](threat-model.md) |
| 4 | High-level architecture | [architecture.md](architecture.md) |
| 5 | Performance budgets per device class | [performance-budgets.md](performance-budgets.md) |
| 6 | Repository layout and this roadmap | [repository-layout.md](repository-layout.md), this file |

Exit: owner approval.

## Phase 1 — Build system and emulator target

**Goal.** The system boots to a shell on the emulator with verified boot and
encrypted storage.

**Deliverables.**

- Manifest pinned to an Android 17 release tag; NottOS product targets for
  Cuttlefish x86_64 and arm64.
- `nott build`: one command from a clean checkout.
- Verified boot with NottOS test keys; compressed read-only system images
  under dm-verity; file-based and metadata encryption on user data.
- Reproducibility check and SBOM generation.
- CI stage A (build, unit tests, boot test, image-size budget).
- First version of `tiercheck`, run against the emulator.
- ADRs: 32-bit app support; source management (forks or patch series);
  licensing; build reproducibility approach.

**Acceptance criteria.**

| ID | Criterion | Verified by |
|---|---|---|
| 1.1 | A clean checkout builds with one command on the documented host | CI log of a clean build |
| 1.2 | The emulator boots to a shell; verified-boot state is locked with the NottOS key; verity is enforcing | Boot test reading the boot properties |
| 1.3 | A system image with one flipped byte does not reach a shell | Negative boot test |
| 1.4 | A system image signed with a different key does not boot | Negative boot test |
| 1.5 | User data is encrypted: a known string written to a file, and the file's name, are absent from the raw storage image after shutdown | Search of the raw image |
| 1.6 | Every mounted filesystem is verity-protected read-only, tmpfs or encrypted, apart from an enumerated exception list (INV-02) | Mount-table check |
| 1.7 | Two independent clean builds produce identical images | Comparison tool; differences listed as bugs |
| 1.8 | The SBOM lists every component with version, hash and licence | Schema validation and spot check |
| 1.9 | System image size is within budget | CI gate |

**Still UNVERIFIED after this phase.** Everything hardware-rooted: the
emulator's bootloader and key storage are software. Reproducibility across
different build machines, unless a second host is available.

## Phase 2 — Core security

**Goal.** Access-control policy, hardening, key management, update system,
integrity folder, Verify system.

**Deliverables.**

- SELinux policy with no permissive domain; syscall filters for every NottOS
  and AOSP process class; filter coverage report.
- Kernel hardening profiles and a checker that fails the build if a required
  option is missing.
- Hardened allocator integrated.
- Key management: memory-hard credential stretching, evict-on-lock classes,
  auto-reboot, duress credential.
- `nott-crypto` and `nott-format`: hybrid signatures and key encapsulation
  over vetted primitives, versioned containers.
- `updated`: hybrid-signed A/B updates with delta support, anti-rollback and
  automatic revert.
- Integrity folder, `verifyd`, the Verify system app, and an external
  verification tool that runs on another computer.
- ADRs: cryptographic suite; storage key hierarchy; update system; kernel
  hardening profile; owner-authenticated root mode; integrity folder.

**Acceptance criteria.**

| ID | Criterion | Verified by |
|---|---|---|
| 2.1 | No permissive domain; enforcement cannot be disabled at run time (INV-03) | Build-time policy analysis; run-time attempt fails |
| 2.2 | Every NottOS and AOSP process class has a syscall filter (INV-04) | Generated coverage report equals the process list |
| 2.3 | No system component can map writable-executable memory or run code from outside verified partitions (INV-11) | Policy neverallow rules; run-time probes |
| 2.4 | The hardening checker passes on the emulator kernel | Checker output |
| 2.5 | CTS modules covering Bionic, the runtime and security still pass with the hardened allocator and policy; every failure is classified | CTS report with a deviation list |
| 2.6 | After lock, evict-on-lock keys are absent from memory; after reboot, all user keys are absent (INV-09) | Memory inspection on an engineering build |
| 2.7 | The duress credential leaves user data unrecoverable | Test: correct credential fails afterwards; storage keys cannot be derived |
| 2.8 | A valid update installs; an update with a bad classical signature, a bad post-quantum signature, or a lower version is refused (INV-15) | Four update tests |
| 2.9 | A new slot that fails to boot is reverted automatically | Fault-injection test |
| 2.10 | Hybrid constructions pass known-answer tests from the standards and interoperate with a second implementation | Test vectors; cross-check |
| 2.11 | Nothing in production can write, rename or delete in the integrity folder (INV-14) | Policy neverallow; run-time attempts from every domain class |
| 2.12 | Verify system reports pass on a genuine image and fail on a tampered manifest; the external tool reproduces the same hashes | Three test cases |

**Still UNVERIFIED after this phase.** Secure-element throttling (INV-10),
irreversible key destruction in hardware, kernel hardening on a real vendor
kernel, MTE.

## Phase 3 — Android compatibility and app sandbox controls

**Goal.** First APKs running, with the sandbox and permission controls.

**Deliverables.**

- Exec-based app spawning with the spare-process pool.
- Network and Sensors permissions; Storage Scopes; Contact Scopes; per-app
  controls; hardware-identifier removal; clipboard and location controls.
- Profiles with separate keys, end-session, private space.
- Connection log and firewall.
- Repository client with signed, expiring metadata and signing-key pinning.
- App corpus for compatibility testing; full CTS and VTS runs on the
  emulator.
- ADRs: process spawning model; sandboxed Google Play approach (design only;
  implementation in Phase 5).

**Acceptance criteria.**

| ID | Criterion | Verified by |
|---|---|---|
| 3.1 | CTS and VTS run to completion on the emulator; every failure is a filed bug or a recorded deliberate deviation | Reports plus deviation registry |
| 3.2 | Each app in a pinned corpus of open-source APKs installs and reaches its first screen | Automated launch test |
| 3.3 | No app process shares an address-space layout with another; none is forked from a shared template | Inspection of process ancestry and mapping addresses across apps |
| 3.4 | With Network revoked, an app causes no packet to leave the device by any route, and sees the network as down rather than crashing (INV-17) | Capture on the emulator interface; test app exercising direct sockets, the download service and name resolution |
| 3.5 | With Sensors revoked, an app receives no sensor data | Test app |
| 3.6 | Storage Scopes and Contact Scopes expose only what was selected | Test app |
| 3.7 | No third-party app can read a hardware identifier (INV-16) | Test app trying every known route |
| 3.8 | Ending a session evicts that profile's keys | As 2.6, per profile |
| 3.9 | The repository client refuses expired, rolled-back or mis-signed metadata, and an app update signed by a different key | Four tests |
| 3.10 | Cold-start cost of exec spawning measured against a zygote baseline on the emulator | Benchmark; relative only |

**Still UNVERIFIED after this phase.** Absolute performance; behaviour of
real vendor HALs; apps that need Google Play.

## Phase 4 — Locked-state policy

**Goal.** USB, network and power policy while locked, proven by tests.

**Deliverables.**

- `seald`, the kernel seal hook, and the unseal path bound to TEE-backed
  authentication.
- Three levels: Sealed, Allowlist, Standard. Emergency-call exception.
- USB policy; authenticated power controls; auto-reboot integration.
- Capture rigs for the emulator and a scenario matrix.
- ADR: locked-state enforcement.

**Acceptance criteria.**

| ID | Criterion | Verified by |
|---|---|---|
| 4.1 | Locked and Sealed: zero frames on every emulator network interface over the full scenario matrix (apps sending, inbound probes, address renewal, pending notifications) (INV-05) | Host-side capture for the whole locked interval |
| 4.2 | From the first instant after a reset until first unlock: zero frames (INV-08) | Capture started before the virtual machine |
| 4.3 | Locked: a virtual USB peripheral attached is not enumerated; the device does not enumerate to a virtual host (INV-06) | Emulated USB attach in both roles |
| 4.4 | Locked: shutdown, reboot, airplane mode and radio toggles fail without authentication, from the UI and from every process domain (INV-07) | UI test and per-domain probes |
| 4.5 | A simulated thermal or battery emergency shuts the device down in every state (INV-18) | Fault injection |
| 4.6 | An emergency call can be placed in every state and level, and opens nothing beyond that call | Emulator telephony test with capture |
| 4.7 | A process with system_server's privileges cannot unseal without a fresh authentication | Test harness acting as a compromised framework |
| 4.8 | Allowlist passes only the chosen apps; Standard passes all | Capture |
| 4.9 | A transfer in flight at lock fails with an error within a bounded time and completes after unlock; a scheduled job is deferred, not lost | Test app |
| 4.10 | Seal and unseal latencies within budget on the emulator | Timing test; relative only |

**What "proven" means here.** Tests show that nothing passed in the
scenarios tested, and the design argues that nothing can: default drop,
single owner of the controls, unseal only with hardware-backed proof. That
is strong evidence, not a mathematical proof. Proof on hardware needs a USB
protocol analyser and RF measurement on a real device, which is Phase 6.

**Still UNVERIFIED after this phase.** Everything about real radios and real
USB controllers; that a powered-down radio is silent; port-controller data
disable.

## Phase 5 — System apps, Nott Reader, sandboxed Google Play

**Goal.** The complete set of essential functions.

**Deliverables.**

- Every system app and service in section 12 of the brief, in Brazilian
  Portuguese and English.
- Nott Reader with its sandbox, fuzzers and published compatibility matrix.
- Hardened browser and WebView.
- Sandboxed Google Play compatibility layer.
- Lost-device service (opt-in); backup and restore with hybrid wrapping;
  encrypted removable storage.
- ADRs: Nott Reader libraries and licences; browser base; keyboard,
  text-to-speech and eSIM components; lost-device protocol; factory-reset
  protection; backup format.

**Acceptance criteria.**

| ID | Criterion | Verified by |
|---|---|---|
| 5.1 | Every app in section 12 is present, starts offline, and holds exactly the privileges in its published table and no others (INV-21) | Manifest audit against the table |
| 5.2 | The keyboard has no network permission; apps that need none have none | Manifest audit |
| 5.3 | A fresh install makes only the documented default connections (INV-13) | Capture from first boot |
| 5.4 | Each Reader parser process has no network, no file access beyond its input, and only allowlisted syscalls (INV-19) | Probes from inside the sandbox |
| 5.5 | A crashing or hung parser does not take down the Reader UI and is discarded | Fault injection |
| 5.6 | Every parser has a fuzz harness running in CI with a coverage report | CI |
| 5.7 | The compatibility matrix states a fidelity level per format, backed by a test corpus | Corpus run |
| 5.8 | No macro, script or remote fetch is ever executed by the Reader | Test documents containing each |
| 5.9 | Google Play installs, updates an app and delivers a push message while holding no privileged permission and running in the ordinary app domain | Permission dump; process domain check |
| 5.10 | A backup restores on a second emulator instance; a backup with a damaged wrapper is refused | Round-trip and negative test |
| 5.11 | All user-facing text exists in Brazilian Portuguese and English | String coverage check |
| 5.12 | Reader open-time budgets met relative to baseline on the emulator | Benchmark; relative only |

**Still UNVERIFIED after this phase.** Telephony, camera, positioning and
payments on real hardware; text-to-speech and accessibility quality with
real users; which third-party apps refuse to run without Google's device
verdict.

## Phase 6 — Performance and first Tier A hardware

**Goal.** RAM-resident system, assembly hot paths with benchmarks, first
Tier A hardware port.

**Deliverables.**

- Device audit and port of the first Tier A device; its capability
  descriptor and vendor binary manifest.
- `ramd`: pinned hot set, extended pin, Full RAM mode, amnesic session.
- Assembly routines where measurement justifies them.
- Device lab running the budget benchmarks nightly.
- Porting guide.
- ADR: run-from-RAM design.

**Acceptance criteria.**

| ID | Criterion | Verified by |
|---|---|---|
| 6.1 | On the device, a locked bootloader boots NottOS with the NottOS key, refuses a modified image and refuses a downgrade (INV-01) | Three tests on hardware |
| 6.2 | Hardware attestation from the device reports the locked state and the NottOS key, and verifies on a second device | Auditor test |
| 6.3 | Credential guessing is limited by the secure element with the OS bypassed (INV-10) | Test on an engineering build that calls the hardware directly |
| 6.4 | Locked: a USB protocol analyser records no data transaction in either role; a network capture and RF measurement record nothing in Sealed mode | Lab measurement |
| 6.5 | Every hardware function on the device checklist works, including an emergency call verified by an approved test procedure | Checklist with evidence |
| 6.6 | Every budget in performance-budgets.md is measured on the device, with raw data published; each miss is either fixed or accepted by the owner in writing | Lab results |
| 6.7 | Pinned memory stays within the RAM rule; Full RAM mode reads no system code from storage after loading | Memory and storage-access traces |
| 6.8 | Every assembly routine has a reference implementation, a differential test, a constant-time test where secrets are involved, run-time feature detection with fallback, and a benchmark showing the required gain | Per-routine record |
| 6.9 | `tiercheck` on the device reports Tier A and matches the audit | Tool output |
| 6.10 | CTS and VTS on the device; failures classified | Reports |

**Still UNVERIFIED after this phase.** Every device not ported; performance
classes without a lab device; long-term stability.

## Phase 7 — Audit, hardening, documentation, release engineering

**Goal.** A release that can be called stable.

**Deliverables.**

- Independent security audit and fixes.
- Key ceremony performed; release keys in offline hardware.
- Update servers, mirrors and transparency log in operation.
- Vulnerability disclosure policy; security-patch process running.
- User guide in Brazilian Portuguese and English; final porting guide;
  architecture overview.
- Deviation list for CTS and the compatibility definition; gap list for a
  certification track.

**Acceptance criteria.**

| ID | Criterion | Verified by |
|---|---|---|
| 7.1 | The audit report is published; no critical or high finding remains open | Report and tracker |
| 7.2 | The key ceremony record is complete and witnessed | Ceremony log |
| 7.3 | At least one independent party reproduces the release images bit for bit | Their published hashes |
| 7.4 | An end-to-end update from the previous release succeeds on every supported device | Release test run |
| 7.5 | Every supported device was tested on this release; untested devices are not listed | Support list generated from test results |
| 7.6 | All invariants INV-01 to INV-21 have a passing test on each supported device, or a recorded reason why the device's tier does not provide it | Invariant matrix |
| 7.7 | Security patches from the latest upstream bulletin are merged | Patch-level check |
| 7.8 | Documentation set complete in both languages | Review checklist |

## Risks in the plan itself

| Risk | Effect | Suggested response |
|---|---|---|
| **Hardware arrives only in Phase 6.** Secure element, real USB controller, IOMMU, MTE and vendor kernels are untested for five phases. | Late discovery that a design does not work on real hardware | Run a small hardware spike in parallel with Phase 2: boot an unhardened NottOS on the candidate Tier A device with the NottOS key. The owner decides. |
| No build host | Phases 1 to 5 cannot meet their emulator criteria | Q6 |
| The browser engine needs frequent, heavy rebuilds | Stale browser is a security hole | Reuse an existing hardened Chromium build as upstream; budget build capacity before Phase 5 |
| Upstream changes its release process again | Rebase work, delayed patches | Keep the delta small; partner track (Q7) |
| The feature list is large for a small team | Long schedule | Phases are gated; nothing is called done without its checks |

No dates are given. Durations depend on team size and infrastructure, which
are not yet known.
