# ADR-0003: 64-bit only; no 32-bit app support

- **Status:** Accepted (2026-11-12; the brief delegates this decision to an
  ADR)
- **Date:** 2026-11-12
- **Decider:** project owner (delegated)

## Context

The brief asks for native 64-bit ARM app support and for an ADR on whether
32-bit apps are supported and on which devices. The answer fixes the build
targets, so it is needed before Phase 1's product definitions.

Facts:

- Android has supported 64-bit-only builds since Android 12. Pixel 7 and
  7 Pro were the first 64-bit-only phones; Google reported up to 150 MB of
  RAM saved and better security from dropping 32-bit code `[V:F-28]`.
- Arm's Cortex-A cores have been 64-bit only since the 2023 generation,
  little cores included `[V:F-28]`. On current and future high-end and
  mid-range SoCs, 32-bit apps cannot run at all.
- Google Play has required 64-bit versions of apps since August 2019 and
  stopped serving 32-bit-only apps to 64-bit-capable devices in August 2021
  `[V:F-28]`. Apps that are still 32-bit only are old and unmaintained.
- GrapheneOS requires 64-bit-only device code `[V:F-08]`.
- 32-bit processes have a much smaller address space, so address-space
  randomisation is weaker, and the AArch32 execution state has no pointer
  authentication, branch target identification or memory tagging `[K]`.

## Options considered

### Option A — 64-bit only on every device

One runtime, one set of system libraries, one app ABI.

- Smaller system image and less RAM; every hardening feature applies to
  every process; half the test matrix.
- 32-bit-only APKs do not install. The user sees a clear message.

### Option B — 32-bit support where the SoC allows it

A second runtime and library set on devices whose cores still implement
AArch32, which are mostly older low-end phones.

- Old 32-bit-only apps run on those devices.
- Costs RAM and storage exactly on the class that has least; those
  processes run with weaker mitigations; behaviour differs between
  devices; a second ABI must be built, tested and patched.

### Option C — 32-bit apps through translation or a virtual machine

- Large new component for a shrinking set of apps. Not considered further.

## Decision

**Option A.** NottOS is 64-bit only: system, runtime and apps, on every tier
and every device class. Installing an APK with only 32-bit native code is
refused with a message that says why.

## Consequences

- **Positive:** fits the security priority (stronger ASLR, PAC, BTI and MTE
  everywhere) and the footprint priority (image size and idle RAM budgets);
  simpler builds and tests.
- **Negative:** a deliberate deviation from app compatibility for
  32-bit-only apps. It is recorded as such in the deviation registry.
- **Risk:** an app important to users in Brazil turns out to be 32-bit only.
  Mitigation: the Phase 3 app corpus records the ABI of every app tested,
  so the size of the problem is measured, not guessed.

## Revisit when

The Phase 3 app corpus shows a material share of wanted apps that are
32-bit only, on devices that could run them.

## Verification

Phase 1: the product definitions build no 32-bit runtime. Phase 3: an
install test with a 32-bit-only APK. Both UNVERIFIED until an AOSP build
host exists.
