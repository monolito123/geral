# Architecture Decision Records

Every major NottOS choice is recorded here: context, options considered,
decision, consequences.

## Index

| ADR | Title | Status |
|---|---|---|
| [0001](0001-base-architecture.md) | Base architecture: hardened AOSP derivative on GKI Linux | Proposed |
| [0002](0002-device-support-tiers.md) | Device support tiers | Proposed |

## Planned ADRs

These decisions are known to be needed. They are listed so that nobody reads
their absence as a decision. Numbers are assigned when the ADR is written.

| Topic | Why it needs its own ADR |
|---|---|
| 32-bit app support | Owner requirement; depends on SoC support for AArch32 |
| Process spawning model | exec-based spawning vs. cold-start and RAM budgets on the 3 GB class |
| Run-from-RAM design | Pinning vs. RAM-backed image, verification model, per-class budgets |
| Locked-state enforcement | Where Sealed is enforced for network, USB and power, and how unsealing is bound to authentication |
| Storage key hierarchy | Key classes, evict-on-lock, KDF parameters per device class |
| Cryptographic suite and formats | Hybrid constructions, algorithm identifiers, crypto-agility |
| Update system | Signing, delta format, anti-rollback, transparency log |
| Owner-authenticated root mode | Boot-state change, indicator, effect on attestation |
| Integrity folder and Verify app | Final path, manifest format, external verification |
| Nott Reader libraries | Parser choices, licences, sandbox profile |
| Kernel hardening profile | KMI-neutral vs. KMI-affecting options per tier |
| Sandboxed Google Play | Compatibility layer scope and maintenance |
| Certification track | CDD/CTS gap list, hardware partner prerequisites |

## Process

1. Copy [0000-template.md](0000-template.md) to `NNNN-short-title.md`.
2. Status starts as **Proposed**. It becomes **Accepted** only when the project
   owner approves it. Later it may become **Superseded by NNNN** or
   **Deprecated**; ADRs are never deleted or rewritten after acceptance.
3. An ADR must list at least two real options and say what is given up.
4. Claims about fast-moving facts cite [../fact-check.md](../fact-check.md).
