# Architecture Decision Records

Every major NottOS choice is recorded here: context, options considered,
decision, consequences.

## Index

| ADR | Title | Status |
|---|---|---|
| [0001](0001-base-architecture.md) | Base architecture: hardened AOSP derivative on GKI Linux | Accepted |
| [0002](0002-device-support-tiers.md) | Device support tiers | Accepted |
| [0003](0003-64-bit-only.md) | 64-bit only; no 32-bit app support | Accepted |
| [0004](0004-source-management.md) | Source management | Accepted |
| [0005](0005-licensing.md) | Licensing of NottOS code | Accepted |
| [0006](0006-build-reproducibility.md) | Build reproducibility | Accepted |
| [0007](0007-minimal-linux-test-target.md) | Minimal Linux test target | Accepted |
| [0008](0008-signature-suite.md) | Hybrid signature suites and formats | Accepted |
| [0009](0009-update-system.md) | Update metadata and trust roots | Accepted |
| [0010](0010-integrity-folder.md) | Integrity folder | Accepted |
| [0011](0011-kernel-hardening-profile.md) | Kernel hardening profile | Accepted as a proposal |

## Planned ADRs

These decisions are known to be needed. They are listed so that nobody reads
their absence as a decision. Numbers are assigned when the ADR is written.

| Topic | Phase | Why it needs its own ADR |
|---|---|---|
| Hybrid key encapsulation | 5 | X25519 with ML-KEM-1024, decided with the backup and transfer format |
| Storage key hierarchy | 2 | Key classes, evict-on-lock, KDF parameters, inline engine vs. per-file keys |
| Update installation | 2 | Writing the inactive slot, delta payloads, boot-time rollback, persistent trust state (with AOSP `update_engine`) |
| Owner-authenticated root mode | 2 | Boot-state change, indicator, effect on attestation, what "write access" means on a verified image |
| Verify system app | 5 | User interface and the on-device checks of ADR-0010 |
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
