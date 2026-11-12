# NottOS

NottOS is a security-first mobile operating system for smartphones that runs
unmodified Android apps (APKs). Its goals, in priority order when they
conflict:

1. Security and integrity of user data
2. Correctness and stability
3. Android app compatibility
4. Performance and battery life
5. Memory and storage footprint
6. Breadth of device support
7. Extra features

Legal and safety obligations sit above all of these: emergency calls always
work, thermal and battery protection is never disabled, and radio behaviour
stays within regulatory limits.

## Status

**Phase 0 — architecture and threat model. Design documents only.**

- No NottOS code exists yet.
- Nothing has been compiled, booted, tested or measured. Every performance
  number in this repository is a *proposed target*, not a result.
- Phase 0 is awaiting owner approval. No later phase has started.

## Documents

| Document | Purpose |
|---|---|
| [docs/adr/0001-base-architecture.md](docs/adr/0001-base-architecture.md) | Decision zero: hardened AOSP derivative vs. container/VM vs. microkernel |
| [docs/adr/0002-device-support-tiers.md](docs/adr/0002-device-support-tiers.md) | How "widest device support" and "full security guarantees" are reconciled |
| [docs/threat-model.md](docs/threat-model.md) | Assets, adversaries, mitigations, residual risk, testable invariants |
| [docs/performance-budgets.md](docs/performance-budgets.md) | Device classes, proposed budgets, RAM budget for run-from-RAM, how CI will measure |
| [docs/requirements-review.md](docs/requirements-review.md) | Requirements that are impossible, unsafe or in conflict, with the closest achievable design and the decisions the owner must take |
| [docs/fact-check.md](docs/fact-check.md) | Fast-moving facts checked on 2026-11-12, with sources and confidence |
| [docs/adr/README.md](docs/adr/README.md) | ADR index, process and template |

## Evidence tags

Claims that depend on fast-moving or hardware-specific facts carry a tag:

| Tag | Meaning |
|---|---|
| `[V:F-nn]` | Checked in this project against a source, recorded as entry `F-nn` in [docs/fact-check.md](docs/fact-check.md) |
| `[K]` | From the author's background knowledge (cutoff June 2026); **not** re-checked. Treat as UNVERIFIED until a fact-check entry exists |
| `[P]` | A proposal or target chosen by this project. Not a measurement |
| `UNVERIFIED` | Explicitly not built, booted, tested or confirmed |

## Conventions

- Code, identifiers, comments, commit messages and technical documents are in
  English. Discussion with the project owner is in Brazilian Portuguese.
- Every major choice is an Architecture Decision Record under `docs/adr/`.
- New system code is Rust by default; C only where an existing interface
  requires it; assembly only with a reference implementation, differential
  tests and a measured gain (see the owner's language policy, restated in
  ADR-0001).
