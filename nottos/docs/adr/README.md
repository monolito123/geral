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

| Topic | Phase | Why it needs its own ADR |
|---|---|---|
| 32-bit app support | 1 | Owner requirement; depends on SoC support for AArch32 and fixes the build targets |
| Source management | 1 | Forked repositories or patch series for changes to AOSP |
| Licensing of NottOS code | 1 | Owner requirement: choose and justify |
| Build reproducibility | 1 | How bit-identical builds are achieved and checked |
| Cryptographic suite and formats | 2 | Hybrid constructions, algorithm identifiers, crypto-agility |
| Storage key hierarchy | 2 | Key classes, evict-on-lock, KDF parameters, inline engine vs. per-file keys |
| Update system | 2 | Signing, delta format, anti-rollback, transparency log |
| Kernel hardening profile | 2 | Interface-neutral vs. interface-changing options per tier |
| Owner-authenticated root mode | 2 | Boot-state change, indicator, effect on attestation, what "write access" means on a verified image |
| Integrity folder and Verify system | 2 | Final path, manifest format, external verification |
| Process spawning model | 3 | exec-based spawning and the spare-process pool vs. cold-start and RAM budgets |
| Sandboxed Google Play | 3 | Compatibility layer scope, reuse and maintenance |
| Locked-state enforcement | 4 | Where Sealed is enforced for network, USB and power, and how unsealing is bound to authentication |
| Nott Reader libraries | 5 | Parser choices, licences, sandbox profile, office fidelity |
| Browser and WebView base | 5 | Which hardened Chromium build to track |
| Keyboard, text-to-speech, eSIM manager | 5 | Component choices and licences |
| Lost-device protocol | 5 | Relay, command signing, the exception in the seal |
| Factory-reset protection | 5 | Owner-bound protection without a Google account |
| Backup and transfer format | 5 | Hybrid wrapping, recovery passphrase |
| Run-from-RAM design | 6 | Pinning vs. RAM-backed image, verification model, per-class budgets |
| Certification track | 7 | Compatibility gap list, hardware partner prerequisites |

## Process

1. Copy [0000-template.md](0000-template.md) to `NNNN-short-title.md`.
2. Status starts as **Proposed**. It becomes **Accepted** only when the project
   owner approves it. Later it may become **Superseded by NNNN** or
   **Deprecated**; ADRs are never deleted or rewritten after acceptance.
3. An ADR must list at least two real options and say what is given up.
4. Claims about fast-moving facts cite [../fact-check.md](../fact-check.md).
