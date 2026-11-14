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

| Phase | State |
|---|---|
| 0 — Architecture | Approved by the owner on 2026-11-12 |
| 1 — Build system and emulator target | **Host side done; emulator side blocked.** Host tools, CI and a minimal Linux test target exist and are tested. No Android system image has been built: that needs an AOSP build host the project does not have. None of the Phase 1 acceptance criteria is met yet. See [docs/reports/phase-1.md](docs/reports/phase-1.md). |
| 2 — Core security | **In progress, host side.** Hybrid signatures and key encapsulation, update verification, the integrity folder and a kernel hardening checker are built and tested; signed boot metadata runs on the test target. One acceptance criterion is met (2.10). See [docs/reports/phase-2-progress.md](docs/reports/phase-2-progress.md). |
| 3 to 7 | Not started |

Every performance number in this repository is still a *proposed target*,
not a measurement.

## Building and testing

Needs a Linux host with `rustup`, `shellcheck` and, for the boot tests, the
packages listed in [tests/mini/README.md](tests/mini/README.md).

```sh
./nott check       # formatting, lints, tests, shell lint, licence headers
./nott build host  # host tools into out/host/
./nott repro       # build twice under different paths; outputs must match
./nott sbom        # SPDX bill of materials for the host tools
./nott mini test   # boot tests on the minimal Linux test target (QEMU)
```

The minimal Linux test target is a test vehicle. It is not NottOS
([ADR-0007](docs/adr/0007-minimal-linux-test-target.md)).

## Documents

The six Phase 0 deliverables (brief, section 15):

| # | Deliverable | Document |
|---|---|---|
| 1 | Conflicts and impossibilities, with proposed resolutions | [docs/requirements-review.md](docs/requirements-review.md), part 2 |
| 2 | Up to seven questions for the owner | [docs/requirements-review.md](docs/requirements-review.md), part 1 |
| 3 | ADR-0001 and the threat model | [docs/adr/0001-base-architecture.md](docs/adr/0001-base-architecture.md), [docs/threat-model.md](docs/threat-model.md) |
| 4 | High-level architecture: components, boot flow, key hierarchy, locked-state enforcement points | [docs/architecture.md](docs/architecture.md) |
| 5 | Performance budgets per device class | [docs/performance-budgets.md](docs/performance-budgets.md) |
| 6 | Repository layout and roadmap with acceptance criteria | [docs/repository-layout.md](docs/repository-layout.md), [docs/roadmap.md](docs/roadmap.md) |

Supporting documents:

| Document | Purpose |
|---|---|
| [docs/adr/0002-device-support-tiers.md](docs/adr/0002-device-support-tiers.md) | Tiers A, B and C made testable |
| [docs/fact-check.md](docs/fact-check.md) | Fast-moving facts checked on 2026-11-12, with sources and confidence |
| [docs/adr/README.md](docs/adr/README.md) | ADR index, planned ADRs, process and template |

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
