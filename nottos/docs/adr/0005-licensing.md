# ADR-0005: Licensing of NottOS code

- **Status:** Accepted (2026-11-12; the brief asks for a choice with a
  justification)
- **Date:** 2026-11-12
- **Decider:** project owner (delegated)

## Context

The brief requires that the GPL obligations of the kernel and the licence of
every reused component be respected, and that a licence for new NottOS code
be chosen and justified.

Most of AOSP's user space is under Apache-2.0; the Linux kernel is
GPL-2.0-only `[K]`. NottOS will exchange code with AOSP in both directions
and hopes to work with a hardware partner.

## Options considered

### Option A — Apache-2.0 for new user-space code

- Same licence as most of AOSP, so code can move both ways without
  relicensing. Explicit patent grant with a retaliation clause. Acceptable
  to hardware vendors.
- Others may ship modified NottOS without publishing their changes.

### Option B — a copyleft licence (GPL-3.0 or MPL-2.0)

- Modified versions must share their source (file-level for MPL).
- GPL-3.0 code cannot be mixed into Apache-licensed AOSP components that
  are distributed as one work, and vendors avoid it. MPL-2.0 is workable
  but unusual in this ecosystem and adds friction for every contribution
  upstream.

### Option C — MIT or BSD

- Simplest terms; no patent grant.

## Decision

| Code | Licence |
|---|---|
| New NottOS user-space code: libraries, daemons, tools, apps, tests, build scripts | **Apache-2.0** |
| Kernel patches and kernel modules | **GPL-2.0-only**, as the kernel requires |
| Changes to an existing project | That project's licence |
| Documentation | **CC BY 4.0** |

Rules:

1. Every source file names its licence in an `SPDX-License-Identifier`
   line near the top. `nott check` enforces this.
2. Contributions are accepted under the licence of the file they change.
3. Reused components keep their licences. Each one is listed in the SBOM
   with its declared licence, and the choice of any component under a
   copyleft or non-standard licence is recorded in an ADR.
4. Copyleft applications may ship as separate packages (for example an
   eSIM manager or keyboard under the GPL). They are separate works; their
   source is published as their licence requires.
5. For every release, the exact kernel source and configuration used are
   published, which meets the kernel's GPL obligations.

**Why Apache-2.0 and not copyleft:** NottOS's protection against a
closed, modified copy is not the licence. It is verified boot with the
NottOS key, reproducible builds and public attestation fingerprints: a
modified build cannot pass itself off as NottOS. Given that, the ability to
exchange code freely with AOSP and to work with a hardware partner is worth
more than forcing source disclosure.

## Consequences

- **Positive:** no licence barrier between NottOS and AOSP; patent grant;
  simple rule for contributors.
- **Negative:** a vendor may build a closed product from NottOS code. It may
  not call it NottOS in any way that verification would accept.
- **Open point:** the NottOS name and logo are a trademark question, outside
  this ADR.

## Verification

`nott check` fails when a source file lacks an SPDX identifier (tested by
removing one). The SBOM lists the declared licence of every dependency of
the host tools.
