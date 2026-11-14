# ADR-0011: Kernel hardening profile

- **Status:** Accepted as a proposal (2026-11-14; decided under the owner's
  delegation). The profile has not been built into an Android kernel; each
  rule's effect on Android compatibility, performance and the GKI module
  interface is UNVERIFIED until it is.
- **Date:** 2026-11-14
- **Decider:** project owner (delegated)

## Context

The brief (section 6) asks for kernel hardening: minimal attack surface,
control-flow integrity, shadow call stack, PAC and BTI, MTE in synchronous
mode on ARMv9 hardware, memory zeroed on allocation and free, strict W^X,
no module loading after boot, and kernel lockdown. ADR-0001 (D2) splits
hardening into what any device can take and what needs a kernel NottOS
builds; ADR-0002 caps devices that cannot take the full profile at Tier B.
Roadmap criterion 2.4 asks for a checker that fails the build when a
required option is missing.

The Kernel Self Protection Project (KSPP) maintains a recommended kernel
configuration `[V:F-33]`. It is the obvious base: public, maintained by
kernel developers, and organised by architecture.

## Options considered

| Option | Assessment |
|---|---|
| Adopt KSPP's list unchanged | Includes settings that conflict with Android (no modules at all), with phones (EFI, SMT, TPM options), or with build hygiene rather than runtime safety |
| **KSPP's list, adapted, with each deviation recorded** | Keeps the reviewed base; makes NottOS-specific choices visible |
| Write a list from scratch | Loses the review KSPP's list has had |

## Decision

The profile is `kernel/hardening-profile.txt`: 95 rules (84 apply to
x86-64, 81 to arm64), each `require OPTION=VALUE` or `forbid OPTION`.
`nott-tools kernel check CONFIG --profile FILE --arch ARCH` checks a full
kernel `.config` against it and fails if any rule is not satisfied.

The brief's demands map to rules as follows:

| Brief | Rules |
|---|---|
| Control-flow integrity | `CFI_CLANG` required, `CFI_PERMISSIVE` forbidden |
| Shadow call stack, PAC, BTI | `SHADOW_CALL_STACK`, `ARM64_PTR_AUTH(_KERNEL)`, `ARM64_BTI(_KERNEL)` |
| MTE | `ARM64_MTE`, `KASAN_HW_TAGS` (synchronous mode is a boot parameter, checked at run time) |
| Memory zeroed on allocation and free | `INIT_ON_ALLOC_DEFAULT_ON`, `INIT_ON_FREE_DEFAULT_ON`, `INIT_STACK_ALL_ZERO` |
| Strict W^X | `STRICT_KERNEL_RWX`, `STRICT_MODULE_RWX`, `DEBUG_WX` |
| Kernel lockdown | `SECURITY_LOCKDOWN_LSM(_EARLY)`, `LOCK_DOWN_KERNEL_FORCE_CONFIDENTIALITY` |
| SELinux without permissive domains | `SECURITY_SELINUX` required; `SECURITY_SELINUX_DEVELOP` and `SECURITY_SELINUX_BOOTPARAM` forbidden |
| Minimal attack surface | `DEVMEM`, `PROC_KCORE`, `KEXEC`, `HIBERNATION`, `BINFMT_MISC`, `MAGIC_SYSRQ`, `COMPAT` and others forbidden |
| Radios isolated by the IOMMU | `IOMMU_DEFAULT_DMA_STRICT` required, `IOMMU_DEFAULT_PASSTHROUGH` forbidden |

Deviations from KSPP:

| KSPP | NottOS | Why |
|---|---|---|
| `MODULES` unset | Modules allowed, `MODULE_SIG_FORCE` required, `MODULE_FORCE_LOAD` forbidden | GKI devices depend on vendor modules `[K]`. The brief's "no module loading after boot" is enforced at run time by setting `kernel.modules_disabled=1` once boot-time modules are loaded |
| `MAGIC_SYSRQ` limited to a safe mask | Forbidden | A phone has no use for it in production; a USB keyboard is one of the attackers in the threat model |
| `COMPAT` unset (arm64) | Same, for every architecture | 64-bit only (ADR-0003) |
| Debug checks `DEBUG_LIST`, `DEBUG_SG`, `DEBUG_VIRTUAL`, `DEBUG_NOTIFIERS`, `DEBUG_CREDENTIALS` | Not required; `LIST_HARDENED`, `BUG_ON_DATA_CORRUPTION` and `SCHED_STACK_END_CHECK` are | Kept to checks intended for production; the cost of the others on phones is unmeasured |
| `RANDSTRUCT_FULL` | Not required | Changes structure layout, so every vendor module would have to be built with the same seed; only possible where NottOS builds all modules. To be reconsidered for Tier A |
| `KSTACK_ERASE` / `GCC_PLUGIN_STACKLEAK`, other GCC plugins | Not required | Depend on compiler support; Android kernels are built with Clang `[K]`. Unverified which apply |
| `WERROR` | Not required | Build hygiene, not run-time protection |
| `SCHED_CORE`, `RESET_ATTACK_MITIGATION`, `HW_RANDOM_TPM`, `RANDOM_TRUST_*`, `EFI_DISABLE_PCI_DMA` | Not required | Specific to PCs (SMT scheduling, EFI, TPM) or to older kernels |
| `STATIC_USERMODEHELPER` | Not required yet | Needs user-space support; to be decided with the AOSP kernel |
| `SECURITY_LANDLOCK` | Not required | NottOS's design does not use it |
| Command-line options and sysctls | Not in this profile | Checked at run time on the device, by a later check of `/proc/cmdline` and `/proc/sys` |

Rules:

1. The checker is a build gate for every kernel NottOS builds (Tier A).
2. For a vendor kernel NottOS cannot rebuild (Tier B), the same check runs
   and its result becomes the device's "kernel hardening profile" posture
   item: every failed rule is listed, not hidden.
3. A rule is removed or changed only by amending this ADR.

## Consequences

- **Positive:** the brief's kernel requirements are a checked list, not
  prose; the gap of any kernel is measured the same way.
- **Negative:** several required options (CFI, shadow call stack, PAC/BTI
  in the kernel, KASAN with hardware tags) need Clang and recent arm64
  hardware; an x86-64 distribution kernel built with GCC cannot meet them.
- **Risk:** a rule turns out to break Android or a vendor module. Then it
  is amended here with the evidence.

## Measured

The distribution kernel used by the minimal Linux test target
(`7.0.0-43-generic`, x86-64, Ubuntu): **28 of 84 rules not satisfied.**
Among them: no CFI and no kernel IBT; SELinux permissive mode and boot-time
disable available; `/dev/mem`, `/proc/kcore`, `kexec`, hibernation, the
magic SysRq key and 32-bit compatibility present; module signatures not
enforced; free-time zeroing off; IOMMU not strict by default.

This is expected for a general-purpose kernel and is the reason the test
target is never called NottOS (ADR-0007). It is a measurement, not a gate:
the test target keeps using that kernel.

Not measured: any Android kernel. The Android Common Kernel's published
`gki_defconfig` lists only options that differ from defaults, so checking
it would report false failures; a full `.config` needs a kernel build.

## Verification

`core/policy/src/kernel.rs` (unit tests, including that the shipped profile
parses and contains every rule the brief names) and
`tools/nott-tools/tests/cli.rs` (`kernel_check_reports_every_rule`).
