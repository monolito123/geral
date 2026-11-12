# ADR-0001: Base architecture — hardened AOSP derivative on GKI Linux

- **Status:** Proposed (awaiting owner approval)
- **Date:** 2026-11-12
- **Decider:** project owner

## Context

NottOS must satisfy requirements that pull in different directions:

| ID | Requirement |
|---|---|
| R1 | Run unmodified APKs and app bundles at the current stable API level (Android 17, API 37 `[V:F-01]`) and aim to pass CTS for everything not deliberately restricted |
| R2 | Support the widest practical range of existing phones |
| R3 | Keep every integrity signal that can be kept legitimately: verified boot and hardware-backed key attestation |
| R4 | Security first, including against physical attackers |
| R5 | Stay usable on 3 GB of RAM and eMMC storage |
| R6 | Reuse audited open-source components; rewrite only for a measurable gain |

Four facts about phones constrain every option:

1. **Drivers exist only for Linux and Android.** GPU, modem, camera, codec,
   Wi-Fi/Bluetooth and TEE client drivers ship as Linux kernel modules plus
   closed userspace HAL binaries linked against Bionic and speaking Binder
   `[K]`. There is no second driver ecosystem.
2. **App compatibility is framework behaviour, not a syscall ABI.** An APK
   depends on ART, the framework services, the permission model, the keystore,
   and hundreds of behaviours that CTS pins down.
3. **Hardware attestation describes what the bootloader booted.** The keystore
   in the TEE or secure element attests the verified-boot state and key of the
   OS image the bootloader loaded. An OS that lives inside a container or VM
   is not what the hardware attests.
4. **The highest exception level is usually taken.** On most shipping phones
   the SoC vendor's hypervisor occupies EL2 (Qualcomm Gunyah `[V:F-16]`;
   MediaTek and Samsung have their own `[K]`). Only some devices give the OS
   kernel EL2 for pKVM. A custom hypervisor or microkernel cannot assume it
   gets EL2.

## Options considered

### Option A — hardened AOSP derivative

Linux (Android Common Kernel, GKI) with vendor drivers behind Treble HALs, and
an AOSP-derived userspace that is trimmed and hardened. This is the GrapheneOS
model. New NottOS components are written in Rust.

- **R1:** native. Same ART, Bionic, Binder and framework as upstream, so CTS
  is reachable and each deviation is a deliberate, listable patch.
- **R2:** inherits Treble and GKI. Any device with an unlockable bootloader
  and a usable vendor image is a candidate; the quality of security then
  depends on the hardware (ADR-0002).
- **R3:** intact. The bootloader verifies NottOS directly; key attestation
  reports `SelfSigned` with the NottOS key, which apps can allowlist
  `[V:F-10]`.
- **R4:** the trusted computing base contains the Linux kernel and vendor
  code. This is the weakness of the option. It is mitigated (attack-surface
  reduction, CFI, MTE, sandboxing), not removed.
- **R5:** one kernel, one userspace. Feasible on 3 GB; upstream runs on less.
- **R6:** maximal reuse. A small team can maintain it, as GrapheneOS,
  CalyxOS and LineageOS demonstrate `[K]`.

### Option B1 — minimal custom userspace on Linux, Android in a container

A small Rust userspace owns the device; Android runs in a namespace container
on the same kernel (the Waydroid or Sailfish AppSupport model `[K]`).

- **R1:** partial. Ordinary apps run. Hardware-backed keystore and attestation,
  DRM, and parts of telephony, camera and NFC do not behave as CTS requires
  because the container does not own the hardware.
- **R2:** worse than A. The host userspace needs a graphics and radio stack.
  Either it uses the vendor's Android HAL binaries through a compatibility
  shim (libhybris), which puts Android's vendor stack back into the host, or
  it relies on mainline open drivers that exist for few SoCs `[K]`.
- **R3:** broken. Attestation describes the host, not the Android instance
  the app runs in.
- **R4:** no gain against the main risk. Container and host share one kernel,
  so a kernel exploit from an app defeats both. Existing container setups run
  Android without its SELinux policy enforced `[K]`, which makes the app
  sandbox weaker than stock Android.
- **R5:** acceptable; a container adds little memory.
- **R6:** medium. The host userspace, compositor and all hardware shims are
  new code.

### Option B2 — minimal custom userspace on Linux, Android in a virtual machine

As B1, but Android runs as a guest under KVM/pKVM with crosvm.

- **R1:** partial, and slower. Every device class must be paravirtualised:
  GPU, camera, codecs, modem, sensors, NFC, secure element. Vendor GPU drivers
  cannot be passed through to a guest on phone SoCs `[K]`, so graphics goes
  through virtio-gpu with a translation layer.
- **R2:** worse than A. Needs EL2 with protected-VM support (constraint 4) and
  per-device paravirtual HALs.
- **R3:** broken, as in B1. The guest also loses hardware DRM levels.
- **R4:** real gain in one direction: a compromised Android guest does not
  directly own the host. But the host must still run the vendor HAL processes
  (Android binaries) to drive the hardware, so the host is, in practice,
  Android again.
- **R5:** fails. Two kernels, two page caches and two memory managers do not
  fit the 3 GB class, and split power management costs battery.
- **R6:** large.

### Option C — custom microkernel with an Android compatibility VM

A small, ideally formally verified kernel (seL4 class) hosts native NottOS
services and a VM that runs Android on Linux.

- **R1, R3, R5:** the same problems as B2.
- **R2:** collapses. Native drivers for closed phone hardware do not exist.
  The realistic design runs Linux as a driver VM, so Linux and vendor drivers
  stay on the I/O path. Bring-up needs EL2 or EL1 ownership, TrustZone
  conventions and bootloader cooperation per device, which in practice means
  one device built with a hardware partner.
- **R4:** the best isolation of any option for the components that live
  outside the Linux VM (key handling, policy enforcement). This is the real
  attraction and it is not available in A.
- **R6:** a multi-year programme with almost no reuse for the native side.
- **Precedent:** the one microkernel phone OS shipped at scale dropped native
  APK compatibility `[K]`.

### Comparison

Ordered by the project's priority list. "Yes" means the option can meet the
requirement; it is not a score.

| Priority | Requirement | A: hardened AOSP | B1: container | B2: VM | C: microkernel + VM |
|---|---|---|---|---|---|
| 1 | Verified boot and attestation of the OS the apps run on | Yes | No | No | No |
| 1 | App sandbox at least as strong as stock Android | Yes, stronger | No (weaker today) | Yes | Yes |
| 1 | Small trusted computing base | No (Linux + vendor code) | No | Partly | Yes, for native services |
| 2 | Proven stability path (CTS/VTS, upstream testing) | Yes | No | No | No |
| 3 | Unmodified APKs including keystore, DRM, telephony | Yes | Partly | Partly | Partly |
| 4 | Native GPU, codec and power management | Yes | Yes | No | No |
| 5 | Fits 3 GB RAM | Yes | Yes | No | No |
| 6 | Many existing devices | Yes (tiered) | Few | Few | About one, with a partner |
| — | Buildable by a small team | Yes | Hard | Very hard | No |
| — | Any path to Google certification | Yes (long) | No | No | No |

## Decision

**Adopt Option A**: NottOS is a hardened, trimmed AOSP derivative on a GKI
Linux kernel, with new and security-critical components written in Rust.

Option A is the only one that meets R1, R3 and R5 at all. Its weakness is the
size of the trusted computing base, and that weakness is shared by B1 and
largely by B2. Option C would be better on priority 1 for a subset of
components but fails priorities 3, 5 and 6 outright, and the project's
priority order does not allow buying isolation at the price of not running
APKs on the target hardware.

Concrete commitments:

| # | Commitment |
|---|---|
| D1 | **Base:** AOSP `android17-release` (API 37) `[V:F-01]`, following quarterly platform releases and security patches. |
| D2 | **Kernel:** the Android Common Kernel GKI branch each device's vendor modules require (`android17-6.18`, `android16-6.12`, and older supported branches `[V:F-03]`). Hardening is applied in two layers: options that do not change the kernel module interface (config, command line, sysctl, LSM policy) on every device; options that change it only where vendor modules can be rebuilt from source. |
| D3 | **Hardware abstraction:** unmodified Treble/VINTF HAL interfaces. The vendor partition is treated as necessary but untrusted: verified at boot, confined by SELinux and seccomp, never given more privilege than upstream gives it. |
| D4 | **Core userspace:** ART, Bionic, Binder and the framework are kept, so that CTS behaviour is preserved. Hardening follows the GrapheneOS approach (hardened allocator, exec-based spawning, stricter SELinux and seccomp). Where GrapheneOS has already published a suitable component under a compatible licence, reuse it instead of rewriting; licence and suitability are checked per component. |
| D5 | **New components in Rust.** Candidates, each needing its own ADR and a measured justification: the locked-state enforcement daemon, USB policy, the update verifier, the integrity verifier, the repository client, backup and transfer cryptography, and the Nott Reader parsers. Upstream already ships Rust in comparable places (Keystore2, the Binder kernel driver `[V:F-04]`, the virtualization stack `[V:F-16]`). |
| D6 | **Not rewritten:** ART, Bionic, SurfaceFlinger, system_server. Rewriting them would break compatibility for no measurable security gain that hardening does not already give. |
| D7 | **Trimming:** remove system apps and services NottOS does not need. Every removal is checked against CTS and recorded as a deliberate deviation if a test depends on it. |
| D8 | **Google Play:** not preinstalled and never privileged. Optional, user-installed, sandboxed. |
| D9 | **Virtualization:** AVF/pKVM is used as an *optional* extra isolation tier on devices that support it. No NottOS guarantee may depend on it, because most devices cannot provide it `[V:F-16]`. |
| D10 | **Reference target:** Cuttlefish (arm64 and x86_64), which is AOSP's own reference device `[V:F-11]`, for development and functional CI. It is the first "device", so development needs no hardware. Physical devices follow ADR-0002. Performance numbers from Cuttlefish never count as device results. |
| D11 | **Architectures:** AArch64 first; x86-64 for emulators and tablets; RISC-V kept buildable for NottOS's own code. Device-specific code is isolated as described in [../architecture.md](../architecture.md), section 3. |

Language policy, restated from the owner's brief so that it is binding on
later ADRs: Rust for new system code; C only where an existing interface
requires it, with full hardening; Kotlin/Java only in the framework
compatibility layer; assembly only with a portable reference implementation,
differential tests, runtime feature detection with fallback, constant-time
behaviour where secrets are involved, and a benchmark on real hardware that
shows a gain.

## Consequences

**Positive**

- APK compatibility, verified boot and hardware attestation work the way app
  developers and the hardware expect.
- One memory manager and one scheduler: the 3 GB class and battery targets are
  reachable.
- Security work starts from a known, public baseline (stock AOSP plus the
  hardening GrapheneOS has shown to be practical `[V:F-09]`).
- A certification track remains possible, because what would be certified is
  an AOSP-compatible device image `[V:F-21]`.

**Negative**

- **The Linux kernel and vendor drivers stay in the trusted computing base.**
  A kernel exploit, most plausibly through a vendor GPU or radio driver,
  defeats the app sandbox. NottOS reduces the probability and the reachable
  surface; it cannot make this class of bug impossible.
- **Closed vendor code** (firmware, TEE OS, HAL binaries, bootloader) cannot be
  audited or fixed by NottOS. Guarantees that depend on it are only as good as
  the vendor.
- **NottOS depends on Google's release process.** Source is published twice a
  year in 2026 `[V:F-12]`, most security fixes ship quarterly `[V:F-13]`, and
  Pixel device trees are no longer published `[V:F-11]`. Without partner
  access to embargoed patches, NottOS can never ship a fix before upstream
  publishes it, and device bring-up is harder than it was before 2025.
- **Every divergence from AOSP is a recurring merge cost.** Patches must stay
  small and modular, and new functionality should live in separate components
  rather than in framework edits.
- **"Fastest" is bounded by Android's architecture.** NottOS can be the
  fastest system *at its security level* and is measured against budgets
  (performance-budgets.md), not against an unhardened system.

**Risks and mitigations**

| Risk | Mitigation |
|---|---|
| Upstream closes further (source, kernels, device support) | Keep the device abstraction clean; seek a hardware partner; track GrapheneOS and other AOSP derivatives for shared infrastructure |
| Hardening breaks apps | Per-app compatibility toggles, as GrapheneOS does `[V:F-09]`; CTS and an app-compatibility corpus in CI |
| Vendor kernel modules cannot be rebuilt | Device is limited to interface-neutral hardening and to Tier B (ADR-0002) |
| Security patches arrive late | Automated rebase pipeline; publish patch level and age honestly in the Verify app; pursue partner access |

## What is taken from the rejected options

- From B2 and C: protected VMs (AVF/pKVM) for specific high-risk workloads on
  capable hardware (D9), evaluated per workload in later ADRs.
- From C: the principle that policy enforcement and key handling live in the
  smallest possible components, separate from the framework. In Option A this
  means small Rust daemons with narrow SELinux domains and, where possible,
  enforcement in the kernel and in the TEE rather than in system_server.

## Revisit when

- A hardware partner offers a device where NottOS controls EL2 and the
  bootloader. Option C, or A with a much stronger hypervisor-enforced core,
  becomes realistic for that device.
- AOSP source publication stops or becomes unusable for independent builds.
- Protected VMs with device passthrough become standard on mid-range phones.

## Verification

- CTS and VTS results on Cuttlefish and on each supported device, with every
  failure classified as a bug or a recorded deliberate deviation.
- The testable invariants in [../threat-model.md](../threat-model.md).
- The budgets in [../performance-budgets.md](../performance-budgets.md),
  measured on real devices.

None of this has been run. Status of all verification: UNVERIFIED.
