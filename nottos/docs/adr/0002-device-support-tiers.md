# ADR-0002: Device support tiers

- **Status:** Proposed (awaiting owner approval)
- **Date:** 2026-11-12 (revised the same day to adopt the tier names and
  definitions in section 13 of the owner's brief)
- **Decider:** project owner

## Context

The brief asks for maximum compatibility and maximum security and
acknowledges that they pull apart, "because many phones lack a relockable
bootloader or a secure element". It prescribes honest tiers instead of one
weak baseline:

- **Tier A, full guarantees:** relockable bootloader that accepts custom
  keys, hardware keystore with secure element, IOMMU, timely firmware
  updates, ideally MTE.
- **Tier B, reduced guarantees:** supported through generic system images and
  vendor HALs; some protections unavailable, such as bootloader relocking or
  a secure element; the user is shown exactly which.
- **Tier C, community ports:** best effort.

This ADR makes those tiers precise enough to test, and records one addition.

Facts that shape the tiers:

- The security properties in question belong to the hardware and its
  firmware. NottOS cannot add a secure element or make a bootloader accept a
  custom verified-boot key.
- The mechanism for a user-set verified-boot key exists in AOSP, but only
  some vendors implement it correctly `[V:F-19]`.
- GrapheneOS, with a similar hardware list, supports only Pixel 6 and later;
  its first non-Pixel device is expected in 2027 `[V:F-08]`. Tier A hardware
  is rare.
- Android 17 does not support the 5.10 kernel branches after QPR1
  `[V:F-03]`, which excludes many older low-end devices from every tier.
- A phone at the 3 GB / eMMC floor that meets Tier A has not been identified
  and may not exist `[K]`. UNVERIFIED either way.

## Options considered

### Option 1 — the brief's three tiers, taken literally

Tier B then contains two very different kinds of device: one whose
bootloader is relocked with the NottOS key but which lacks a secure element,
and one whose bootloader cannot be relocked at all. On the second kind a
modified system image boots without complaint, hardware attestation reports
an unlocked device, and nothing protects against tampering by someone with
brief physical access. A posture list of "missing protections" would show
this, but as one line among several.

### Option 2 — the brief's three tiers, with Tier B split into two grades

Same tiers. Inside Tier B, whether the boot chain is verified becomes a
headline attribute rather than a list item: **B-verified** and
**B-unverified**. Everything else stays a per-mechanism posture item.

### Option 3 — strict: Tier A only

Uniform guarantees, smallest maintenance load, almost no device breadth. This
is GrapheneOS's position `[V:F-08]`. Rejected by the brief, which asks for
tiers.

## Decision

**Adopt Option 2.**

| | **Tier A — full guarantees** | **Tier B — reduced guarantees** | **Tier C — community port** |
|---|---|---|---|
| Built and tested by | The NottOS project | The NottOS project | The community; the project does not test it |
| How NottOS is installed | Full device build | Generic system image plus device overlay, on the vendor's HALs and kernel | Whatever the porter achieves |
| Bootloader | Relocked with the NottOS key, rollback protection | **B-verified:** relocked with the NottOS key. **B-unverified:** unlocked | Unknown |
| Key storage and unlock throttling | Secure element (StrongBox, Weaver) plus TEE | TEE; secure element if present | Unknown |
| IOMMU isolation of radios | Confirmed by audit | Unconfirmed or partial | Unknown |
| Firmware security updates | Timely | May lag or have ended | Unknown |
| MTE | Used where the device has it ("ideally") | Not expected | Unknown |
| USB data off while locked | Port controller and kernel | Kernel; controller where the USB HAL supports it | Unknown |
| Kernel hardening profile | Full (NottOS builds the kernel and vendor modules) | Limited to what the vendor's kernel allows | Unknown |
| Hardware attestation of NottOS | Yes | B-verified: yes. B-unverified: no | No claim |
| Listed in the published attestation allowlist | Yes | B-verified only | No |
| Claims made | Every guarantee in the threat model | Only the guarantees whose mechanisms the posture screen shows as present | None |

Rules:

1. **Posture screen.** On every tier, Settings lists each mechanism from the
   threat model with its state on this device: enforced by hardware, enforced
   by software only, or absent. This is the brief's "shows the user exactly
   which protections are missing".
2. **B-unverified is announced, not listed.** Setup and the posture screen
   open with a plain statement that this device cannot verify its own system
   software and is not protected against physical tampering.
3. **Hard exclusions, all tiers.** A device does not carry the NottOS name if
   its boot ROM or a download mode has a public, unpatchable bypass of
   verified boot `[K]`, or if emergency calling cannot be shown to work.
4. **Nothing is supported until tested.** A device appears on the support
   list for a release only with a written device audit and a passing test
   run on that release. A device that was not re-tested drops off the list
   for that release.
5. **Tier checklist tool.** A tool reads a device's properties and reports
   the tier it can reach and the state of each mechanism. The posture screen
   and the audit report use the same data.
6. **Vendor binaries are confined and listed.** Each device publishes a
   manifest of every closed binary: path, hash, origin, purpose, SELinux
   domain, whether a syscall filter is enforced, and whether it has network
   access.

Hardware abstraction is described in
[../architecture.md](../architecture.md), section 3.

Targets:

| Purpose | Target |
|---|---|
| First "device" | The Cuttlefish emulator, x86_64 and arm64, including a 3 GB RAM configuration. Development needs no hardware. |
| Architectures | AArch64 first; x86-64 for emulators and tablets; RISC-V kept buildable for NottOS's own code so that a port is not blocked by assumptions |
| First Tier A hardware | Chosen in Phase 6 after a device audit. A current Pixel is the likely candidate; what can still be built without published device trees `[V:F-11]` must be checked first. UNVERIFIED. |
| Low-end performance class | To be identified. Probably Tier B (see Context). |

## Consequences

- **Positive:** claims stay true per device; breadth remains available;
  app developers get an allowlist that contains only devices with real
  verified boot; no device is called supported on faith.
- **Negative:** several test matrices; re-testing every listed device for
  every release costs hardware and time, and the list will be short at
  first.
- **Risk:** users on B-unverified devices believe they have Tier A
  protection. Mitigation: rule 2, and exclusion from the attestation
  allowlist.
- **Limit of Tier B.** A generic-image install keeps the vendor's kernel and
  vendor partition as shipped. NottOS cannot update that firmware; when the
  vendor stops, the device's firmware patch level freezes and the posture
  screen says so.

## Owner decision required

Approve the B-verified / B-unverified split (Option 2), or keep Tier B
undivided (Option 1).

## Revisit when

- A hardware partner commits to a device. A tier above A may be defined:
  NottOS controls the bootloader, post-quantum verified boot, hardware radio
  switches.
- A vendor ships correct custom-key verified boot across a low-end line.

## Verification

Per-device audit reports, tier-checklist output and release test runs. None
exist yet. UNVERIFIED.
