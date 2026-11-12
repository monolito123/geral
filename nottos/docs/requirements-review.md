# Requirements review — conflicts, limits and owner decisions

- **Version:** 0.1 (Phase 0 draft)
- **Date:** 2026-11-12

The project brief asks that no requirement be dropped silently or pretended
into existence. This document lists every requirement found to be impossible
as written, unsafe, or in conflict with another, with the closest achievable
design. Items marked **Decision needed** change the architecture and wait for
the owner. For the rest, the stated assumption is used unless the owner
objects.

Evidence tags are explained in the [README](../README.md).

## Summary

| ID | Topic | Kind | Owner decision |
|---|---|---|---|
| C-01 | Device breadth vs. full security guarantees | Conflict | **Needed** |
| C-02 | "Best at all four goals" vs. the cost of hardening | Conflict | Assumed |
| C-03 | Play Integrity verdicts | Impossible as an OS feature | Informational |
| C-04 | Post-quantum signatures in the boot chain | Limited by hardware | Assumed |
| C-05 | Evicting keys on every lock | Conflict with the Android app model | **Needed** |
| C-06 | Sealed by default vs. Bluetooth devices, locate, emergency | Conflict | **Needed** (one sub-point) |
| C-07 | "Nothing outside user data is writable" | Impossible as written | Assumed |
| C-08 | Run-from-RAM on the 3 GB floor | Conflict | Assumed |
| C-09 | Enforcement in USB and radio firmware | Limited by hardware | Informational |
| C-10 | All keys in a secure element; SE throttling | Limited by hardware | Assumed |
| C-11 | Kernel hardening vs. prebuilt vendor modules | Limited by hardware | Assumed |
| C-12 | seccomp for every process class, including vendor HALs | Costly per device | Assumed |
| C-13 | Hand-written assembly for cryptography | Unsafe as written | Assumed |
| C-14 | Timely security patches without partner access | External dependency | **Needed** |
| C-15 | Building and testing an OS in this environment | Tooling limit | **Needed** |
| C-16 | Integrity folder "marked immutable" | Wording | Assumed |
| C-17 | Sections missing from the brief | Input | **Needed** |

## C-01 — Device breadth vs. full security guarantees

**Asked for.** The widest practical device compatibility, and on every device
a relocked bootloader with NottOS keys, secure-element throttling, MTE, USB
disabled at the controller and IOMMU-isolated radios.

**Problem.** Those are hardware properties. Few phones have all of them:
GrapheneOS, with a similar list, supports only Pixels, and even a flagship
partner's current devices fall short `[V:F-08]`. Custom-key verified boot is
implemented correctly by only some vendors `[V:F-19]`. A phone at the 3 GB /
eMMC floor with the full list has not been identified and may not exist
`[K]`.

**Closest achievable design.** Three explicit tiers with published per-tier
guarantees and an on-device posture screen
([ADR-0002](adr/0002-device-support-tiers.md)).

**Decision needed.** Accept tiers, or restrict NottOS to full-guarantee
devices only. And: should the unverified-boot Tier 3 exist at all?
Recommendation: accept tiers; define Tier 3 but ship it late, under a
different name.

## C-02 — "Best at all four goals" vs. the cost of hardening

**Asked for.** The best available system at security, speed, low resource
use and compatibility at once.

**Problem.** The mandated hardening has a price that an unhardened system
does not pay. Exec-based spawning costs about 200 ms per cold start on
flagships and more on low-end hardware, plus memory `[V:F-09]`. A hardened
allocator, zero-on-free, synchronous MTE and CFI each cost CPU or RAM `[K]`.

**Closest achievable design.** The priority order already resolves this:
security wins. NottOS aims to be the fastest and leanest system *at this
security level*, held to explicit budgets
([performance-budgets.md](performance-budgets.md)) and a no-regression gate
against stock AOSP on the same device, with a 10 % allowance on cold start.

**Assumption.** Where a budget and a mandated mitigation collide on class L,
the mitigation stays, the measured cost is reported, and the owner is asked
before any mitigation is relaxed.

## C-03 — Play Integrity verdicts

**Asked for.** Keep every integrity signal that can be kept legitimately.

**What is and is not achievable**, from Google's current definitions
`[V:F-05]`:

| Verdict | Requires | NottOS |
|---|---|---|
| `MEETS_BASIC_INTEGRITY` | On Android 13+, "only that the attestation root of trust is provided by Google" | Plausibly obtainable on hardware with Google-rooted attestation keys. UNVERIFIED until tested. Returned only to apps installed through Google Play. |
| `MEETS_DEVICE_INTEGRITY` | "a genuine and certified Android device [...] the loaded Android OS is a certified device manufacturer image" | **Not obtainable.** NottOS is not a certified manufacturer image. |
| `MEETS_STRONG_INTEGRITY` | Device integrity plus recent patches on all partitions | **Not obtainable**, for the same reason. |

Apps that demand device or strong integrity (many banking apps) will refuse
to run, as they do on GrapheneOS `[V:F-10]`. No engineering inside NottOS
changes that. NottOS will not spoof verdicts, fingerprints or attestation.

**What NottOS does instead.** Full hardware key attestation with verified
boot, a published and signed list of verified-boot key fingerprints, and a
developer guide so that apps can allowlist NottOS directly, as GrapheneOS
does `[V:F-10]`.

**Separate track.** Certification means CDD compliance, passing CTS and a
commercial agreement with Google `[V:F-21]`, which in practice needs a
hardware partner. Several NottOS features are likely CDD deviations and
would have to be optional or absent in a certified build; that gap list is a
later deliverable.

One side effect worth knowing: Google's developer-verification rules, now
enforced in Brazil, apply to certified devices `[V:F-14]`. They do not apply
to NottOS.

## C-04 — Post-quantum signatures in the boot chain

**Asked for.** Hybrid classical plus post-quantum signatures on verified-boot
metadata "where the bootloader permits", and SLH-DSA for long-lived roots.

**Problem.** The bootloader verifies the first NottOS image, and NottOS
cannot change the bootloader of an existing phone. Android 17's verified-boot
library is gaining ML-DSA `[V:F-07]`, but Google has not said that existing
devices' bootloaders will be updated, and the supported parameter sets could
not be confirmed `[V:F-20]`. A post-quantum check added later in the boot
(inside NottOS's own early init) is itself only protected by the classical
signature the bootloader checked, so it does not stop an attacker who can
forge that classical signature.

**Closest achievable design.**

- **Updates, repository metadata, backups, device transfer:** fully hybrid
  from day one. NottOS controls both ends.
- **Boot chain on existing devices:** the strongest classical algorithm the
  bootloader accepts, plus a hybrid-signed build manifest verified by the
  updater before installation and by the Verify app.
- **Boot chain on devices whose bootloader verifies ML-DSA:** use it.
- **SLH-DSA** for the offline root keys that certify update-signing keys.

**Why this is acceptable.** Forging a boot signature needs a quantum computer
*at the time of the attack* and physical access. Recorded ciphertext cannot be
attacked later through this path. The update channel, which is reachable
remotely, is the one that most needs post-quantum signatures, and it gets
them.

## C-05 — Evicting keys on every lock

**Asked for.** "On lock, evict keys for data that does not need to be
available while locked."

**Problem.** Android has two storage classes: available after boot, and
available after first unlock. Apps assume that after the first unlock their
files stay readable while the screen is locked: alarms, messaging, music
playback, background sync `[K]`. Evicting the main profile's keys at every
lock would break most apps. There is no upstream storage class that means
"only while unlocked".

**Closest achievable design.** Evict-on-lock is offered where it is safe:

1. **Keystore keys** marked as requiring an unlocked device are unusable
   while locked (upstream behaviour `[K]`).
2. **Whole profiles and the private space** can be set to end their session
   at lock, which evicts their keys. The cost is that their apps stop.
3. **A new opt-in storage class for apps that ask for it**, with a per-app
   key evicted at lock. Existing apps would not use it without changes.
4. **Auto-reboot** returns everything to BFU after the configured locked
   time.

**Decision needed.** Is this graded model acceptable, or does the owner want
the main profile's keys evicted at lock by default, accepting that most apps
lose background function while locked?

## C-06 — Sealed by default vs. Bluetooth devices, locate, emergency

**Asked for.** Sealed as the default: nothing passes on any radio while
locked; plus emergency calls in every mode, and an owner-authenticated
locate/lock/wipe path that works under Sealed.

**Problems.**

1. **Locating a lost phone requires a network connection.** A truly sealed
   device cannot be reached.
2. **Emergency calls on 4G and 5G are IP traffic** (IMS). On some devices the
   IMS stack runs on the application processor `[K]`, so a blanket IP drop
   would block the call. Wi-Fi calling emergency calls need a tunnel over
   Wi-Fi.
3. **Bluetooth is mostly not IP.** A literal "drop IP" rule leaves audio,
   watches, hearing aids and car links untouched; a literal "sealed box"
   disconnects them all at every lock, including music with the screen off
   and hearing aids.

**Closest achievable design.**

- Sealed covers IP *and* the link layer: Wi-Fi, Bluetooth, NFC and UWB are
  powered down or blocked, not just filtered.
- **Emergency exception:** starting an emergency call opens only the bearer
  or tunnel that call needs, for its duration, in every mode.
- **Locate exception (opt-in):** one system service, one pinned endpoint,
  outbound polling only, commands signed by the owner's key. The owner
  chooses the channel (mobile data, known Wi-Fi) and accepts that it is a
  deliberate hole in the seal.
- **Bluetooth devices connected before lock** follow the same setting as USB
  accessories: default disconnect on lock.

**Decision needed (one point).** Under Sealed, should Bluetooth audio and
hearing devices that were connected before lock stay connected by default?
Recommendation: disconnect by default for consistency with the "sealed box"
goal, with a clearly labelled setting to keep audio and hearing devices, and
that setting pre-enabled when a hearing aid is paired.

**Stated consequence of the Sealed default.** No notifications, messages or
internet calls arrive while locked. The owner has already chosen this; it is
recorded here so that it reaches the setup screen in plain words.

## C-07 — "Nothing outside the encrypted user-data partition is writable"

**Problem.** Real devices have small writable partitions outside user data
that the hardware and Android itself need: the partition that holds
encrypted key blobs and update state, the bootloader's control block, vendor
calibration data, and modem state `[K]`. They cannot be made read-only
without breaking boot, updates or the radio.

**Closest achievable design.** Enumerate them per device; mount them
`nosuid,nodev,noexec`; allow writes only from the specific domains that own
them; forbid executable content; audit what is stored. All volatile *system*
state stays in tmpfs. This is invariant INV-02 in the threat model.

## C-08 — Run-from-RAM on the 3 GB floor

**Problem.** Loading the system image into RAM on a 3 GB phone would take
about 40 % of memory from apps. The brief anticipated this.

**Closest achievable design.** Three levels (pinned hot set, extended pin,
Full RAM mode) selected by a per-class RAM budget; Full RAM and amnesic modes
only from 12 GB (amnesic optionally at 8 GB with a small cap)
([performance-budgets.md](performance-budgets.md), sections 3 and 4).

## C-09 — Enforcement in USB controller and radio firmware

**Limit.** That firmware is closed. NottOS can only use the controls the
vendor exposes: a USB HAL call or port-controller register to disable data;
power-down or block switches for radios. Where such a control exists it is
used; where it does not, enforcement is in the kernel only, and the device's
posture screen says so. Whether a powered-down radio is really silent can be
checked in a lab with RF measurement, not proven from software.

## C-10 — All keys in a secure element; throttling by the secure element

**Limit.** Many phones, including most of class L, have a TEE but no secure
element `[K]`. There, throttling is done by the TEE: still outside the OS,
but weaker against physical attack. Irreversible key destruction for the
duress PIN is also strongest with a secure element.

**Assumption.** Tier 1 requires a secure element. Tier 2 without one requires
a passphrase rather than a short PIN at setup, and says why.

## C-11 — Kernel hardening vs. prebuilt vendor modules

**Limit.** Vendor kernel modules are built against a fixed kernel module
interface. Hardening options that change it need the modules rebuilt from
source, which is possible only when the vendor publishes usable source.
"No module loading after boot" also requires loading every module at boot,
including ones the vendor loads on demand, at some memory cost `[K]`.

**Assumption.** Two hardening profiles (ADR-0001, D2). Devices that cannot
take the full profile are capped at Tier 2 and the gap is shown.

## C-12 — seccomp for every process class

**Limit.** Upstream does not apply syscall filters to most vendor HAL
processes. Doing so needs a per-device trace of what each closed binary
calls, and a wrong filter breaks hardware.

**Assumption.** Filters for all NottOS and AOSP process classes from the
start. Vendor HAL filters are generated per device from traces, start in a
logging mode on engineering builds only, and are enforced in production once
stable. A device's posture screen lists any HAL still unfiltered.

## C-13 — Hand-written assembly for cryptographic primitives

**Asked for.** Assembly for crypto primitives, with constant-time behaviour
and known-answer tests; and, separately, "never design new primitives" and
"use vetted implementations, formally verified where available".

**Problem.** Writing new assembly for AES, SHA or ML-KEM contradicts the
second requirement. New crypto assembly is where timing leaks and carry bugs
come from, and the existing vetted libraries already contain reviewed,
hardware-accelerated assembly.

**Closest achievable design.** For cryptography, NottOS *uses* the assembly
in vetted libraries and in the kernel's crypto subsystem, and adds none of
its own unless no vetted implementation exists for a needed primitive and
platform. The owner's assembly policy (reference implementation, differential
tests, measured gain) applies fully to non-crypto routines such as pixel
loops, checksums and compression. No assembly at all is written before a
device lab exists to measure it.

## C-14 — Timely security patches without partner access

**Problem.** NottOS's security depends on shipping fixes quickly, but AOSP
source now arrives twice a year `[V:F-12]`, most security fixes are released
quarterly `[V:F-13]`, and device-specific code for the reference phones is no
longer public `[V:F-11]`. Device makers receive fixes under embargo before
publication; an independent project does not.

**Consequence.** Without a partner agreement, NottOS can be at best a few
days behind public release and can never be ahead of it.

**Decision needed.** Whether to pursue a hardware-partner or licensee
relationship as a parallel track from the start. Recommendation: yes; it is
also the prerequisite for the certification track (C-03) and for a device
that would exceed Tier 1.

## C-15 — Building and testing an OS in this environment

**Limit.** The current development environment has 4 CPU cores, 15 GiB of
RAM, about 30 GB of free disk, no emulator and no AArch64 toolchain
([fact-check.md](fact-check.md)). AOSP cannot be built or booted here.

**What can be done here.** Design documents; host-side Rust components with
unit tests and fuzzing (formats, signature verification, policy engines,
parsers, build tooling).

**What cannot.** Building system images, booting Cuttlefish, running CTS,
measuring any budget, anything on a real device.

**Decision needed.** Provide a build host (large disk and memory, hardware
virtualisation) and, later, test devices; or accept that the next phases
deliver only host-side components and that everything else stays UNVERIFIED.

## C-16 — Integrity folder "marked immutable"

**Wording.** The system image is a read-only filesystem format protected by
dm-verity. It has no per-file immutable flag to set, and needs none: no file
on it can be changed by anything, root included, without failing
verification at the next read. The layering asked for is kept (verified
read-only image, dedicated SELinux type, no write rule), and "immutable"
is satisfied by the filesystem rather than by an attribute. The path
`/nott/attest` is feasible as a directory in the root of the system image.

## C-17 — Sections missing from the brief

The brief as received ends in the middle of section 11 (Nott Reader), at a
bullet beginning "Sc". Sections 12 to 15 did not arrive, including the phase
list in section 15 that section 2 refers to.

**Assumption used for Phase 0.** Phase 0 is: ADR-0001, the threat model, the
performance budgets, and this review. That reading is supported by sections
2, 4, 5 and 6 of the brief.

**Decision needed.** Resend the end of section 11 and sections 12 to 15
before Phase 1 is planned.
