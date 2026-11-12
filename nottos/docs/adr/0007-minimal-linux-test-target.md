# ADR-0007: Minimal Linux test target

- **Status:** Accepted (2026-11-12; decided under the owner's delegation,
  see [0001](0001-base-architecture.md))
- **Date:** 2026-11-12
- **Decider:** project owner (delegated)

## Context

The first "device" in the roadmap is the Cuttlefish emulator. Building it
needs an AOSP build host with at least 64 GB of RAM and 400 GB of disk
`[V:F-22]`, which the project does not have yet. Without it, nothing that
boots can be tested.

A good part of what NottOS adds does not depend on Android at all. It lives
in or next to the kernel: verified read-only images, encrypted storage, the
storage policy check, and later the kernel seal hook, USB policy, the update
verifier and the integrity verifier. Those can be exercised on any Linux
kernel.

Measured in this environment: QEMU without hardware virtualisation boots a
distribution kernel with a small initramfs to a shell in about five seconds.

## Options considered

### Option A — wait for Cuttlefish

Test nothing that boots until a build host exists.

- No extra code to maintain and no risk of confusing targets.
- Every boot-level check stays unverified for an unknown time, and the test
  harnesses are written blind.

### Option B — a minimal Linux system in QEMU, from distribution packages

A distribution kernel, busybox, a two-stage init, and images built with
standard tools. Lives under `tests/mini/`.

- Runs anywhere, including CI, in seconds.
- The kernel is not the Android kernel and is not hardened. Nothing
  hardware-rooted exists. Results do not transfer to Cuttlefish or a phone.

### Option C — the same, with a custom-built kernel and root filesystem

As B, but NottOS builds its own kernel and user space.

- Needed eventually to test kernel patches such as the seal hook.
- Adds a kernel build and its sources before there is anything to patch.

### Option D — containers

Run checks in containers on the CI host.

- Cannot load kernel modules or create device-mapper targets without
  privileges CI does not grant, so it cannot test what matters here.

## Decision

**Option B now; grow into Option C when the first kernel patch exists.**

Rules:

1. The target is never called NottOS and makes no product claim. Its
   documentation says so first.
2. Roadmap acceptance criteria stay tied to Cuttlefish and to hardware.
   Results on this target are reported separately, as analogues, and never
   mark a criterion as met.
3. Product code does not depend on anything under `tests/mini/`. The
   dependency goes one way.
4. Every check that expects something to be absent has a control showing
   that the check can fail.
5. The target's scripts are test scaffolding. They are shell, linted, and
   kept small.

## Consequences

- **Positive:** boot-level tests exist today and run in CI; the harnesses
  for tamper tests, raw-disk scans and the storage policy are written
  against something that executes; later phases get a place to develop
  kernel-adjacent components before the build host arrives.
- **Negative:** one more target to keep working; a distribution kernel can
  change under the tests; the encryption analogue uses ext4's legacy
  key-management tool because the distribution offers no packaged
  alternative.
- **Risk:** a passing run is read as progress on the Android system.
  Mitigation: rules 1 and 2, and the wording of every phase report.

## Revisit when

- Cuttlefish runs in CI. Decide then whether this target still earns its
  keep.
- The first kernel patch is written (move to Option C).

## Verification

`./nott mini test`. First run recorded in the Phase 1 report.
