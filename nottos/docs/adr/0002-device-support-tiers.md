# ADR-0002: Device support tiers

- **Status:** Proposed (awaiting owner approval)
- **Date:** 2026-11-12
- **Decider:** project owner

## Context

Two requirements conflict directly:

- **Widest practical device compatibility** (priority 6).
- **Full security guarantees** (priority 1): verified boot with NottOS keys on
  a relocked bootloader, rollback protection, secure-element throttling of
  unlock attempts, MTE, USB data disabled at the controller, radios isolated
  by the IOMMU.

The second set is a property of the *hardware and its firmware*, not of the
operating system. NottOS cannot add a secure element, enable memory tagging
the SoC vendor left off, or make a bootloader accept a custom verified-boot
key.

What the hardware landscape looks like:

- The mechanism for a user-set verified-boot key exists in AOSP, but only some
  vendors implement it correctly `[V:F-19]`.
- GrapheneOS, which demands roughly the same hardware properties, supports
  only Pixel 6 and later. Its first non-Pixel device is expected in 2027, and
  current flagships from that partner "lack MTE, adequate secure element
  integration and other features" `[V:F-08]`. Meeting the full list is rare
  even among flagships.
- Android 17 no longer supports the 5.10 kernel branches after QPR1
  `[V:F-03]`, which excludes many older low-end devices.
- The 3 GB / eMMC floor and the full hardware list almost never appear in the
  same device `[K]`. No such device has been identified; treat the existence
  of a full-guarantee low-end phone as UNVERIFIED.

If NottOS presents one undifferentiated "supported" list, either the list is
very short or the security claims are false on most of it.

## Options considered

### Option A — strict: only devices that meet every hardware requirement

One tier. Every NottOS guarantee holds on every supported device. This is
GrapheneOS's position `[V:F-08]`.

- Gives honest, uniform guarantees and the smallest maintenance load.
- Gives up priority 6 almost entirely, and gives up the 3 GB floor in
  practice.

### Option B — broad: any device that boots the system image

One tier, defined by "it runs". Maximum breadth.

- On most devices the bootloader stays unlocked, so verified boot,
  evil-maid resistance and meaningful attestation are absent. Presenting such
  a device as NottOS would break priority 1 and mislead users.

### Option C — explicit tiers with per-tier guarantees

Devices are classified by what their hardware can enforce. Each tier has a
published guarantee set, and the device itself shows its tier and the status
of each mechanism.

- Keeps breadth without diluting claims.
- Costs: more test configurations, and the risk that users do not read the
  tier.

## Decision

**Adopt Option C** with three tiers. The names are part of the product so
that the difference is visible.

| | **Tier 1 — Full** | **Tier 2 — Verified** | **Tier 3 — Compatibility** |
|---|---|---|---|
| Bootloader | Relocked with NottOS key, rollback protection | Relocked with NottOS key, rollback protection | Unlocked, or locked to a key NottOS does not control |
| Verified-boot fingerprint at boot | Shown in full by the bootloader | Shown, possibly truncated | Not available |
| Key storage | Secure element (StrongBox) plus TEE | TEE; secure element if present | TEE, not attestable to NottOS |
| Unlock throttling | Secure element (Weaver) | TEE (Gatekeeper), or Weaver if present | TEE |
| Memory tagging | MTE, synchronous | Not available | Not available |
| USB data off while locked | At the port controller and in the kernel | Kernel; controller where the USB HAL supports it | Kernel |
| Radio isolation by IOMMU | Confirmed by device audit | Unconfirmed or partial | Unknown |
| Kernel hardening | Full profile (vendor modules rebuilt) | Full where modules can be rebuilt, otherwise interface-neutral only | Interface-neutral only |
| Hardware attestation of NottOS | Yes (`SelfSigned`, NottOS key) | Yes | No (reports unlocked) |
| Firmware security patches | Current | Current | May be end-of-life |
| Product label | NottOS | NottOS, with a posture screen listing what is missing | "NottOS Compatibility build" with a warning at setup and in Settings |

Rules that apply to all tiers:

1. **No silent downgrade.** Settings shows a "Security posture" screen with
   every mechanism from the threat model and its state on this device:
   enforced by hardware, enforced by software only, or absent.
2. **Hard exclusions.** A device is not supported in any tier if its boot ROM
   or a download mode has a public, unpatchable exploit that bypasses verified
   boot `[K]`, or if emergency calling cannot be made to work.
3. **Tier 3 never carries the unqualified NottOS name**, never claims
   protection against physical attackers, and is excluded from the published
   attestation allowlist.
4. **A device enters a tier only after a written device audit** against a
   checklist derived from the threat model. Until audited, a device's tier is
   UNVERIFIED.

Development targets `[P]`:

| Purpose | Target |
|---|---|
| Functional development and CI | Cuttlefish arm64 and x86_64, including a 3 GB RAM configuration |
| First Tier 1 device | A current Pixel with MTE, a secure element and long remaining support. The exact model is chosen after an audit of what is still buildable without published device trees `[V:F-11]`. |
| Low-end performance class | To be identified by device audit. A Tier 1 or Tier 2 device at 3–4 GB and eMMC may not exist (see Context). |

## Consequences

- **Positive:** security claims stay true per device; breadth is still
  available to users who accept the trade; app developers get an allowlist
  that contains only devices with real verified boot.
- **Negative:** three test matrices; documentation and UI must explain tiers
  clearly; some users will run Tier 3 and believe they are protected, despite
  the warnings.
- **Risk:** Tier 3 damages the project's reputation if its limits are not
  communicated. Mitigation: distinct name, setup-time warning, posture screen,
  separate download page.

## Owner decision required

Whether Tier 3 exists at all. The recommendation is to define it now but ship
it only after Tier 1 is solid, as a separately named build. The alternative,
Option A, is fully consistent with the priority order and is the lower-risk
choice if device breadth matters less to the owner than this ADR assumes.

## Revisit when

- A hardware partner commits to a device; the tier list may gain a tier above
  Tier 1 (NottOS controls the bootloader, PQC verified boot, hardware radio
  switches).
- A vendor ships correct custom-key verified boot across a low-end line.

## Verification

Per-device audit reports and the posture screen's self-test results. None
exist yet. UNVERIFIED.
