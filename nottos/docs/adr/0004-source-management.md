# ADR-0004: Source management

- **Status:** Accepted (2026-11-12; decided under the owner's delegation)
- **Date:** 2026-11-12
- **Decider:** project owner (delegated)

## Context

AOSP is about a thousand Git repositories pinned together by a manifest
`[K]`. NottOS will change a small number of them and add its own code.
Today the whole project lives in one directory of the owner's general
repository, and this development environment cannot create repositories.

## Options considered

### Option A — forked repositories and a manifest

Each AOSP project that NottOS changes is forked. A NottOS manifest pins the
AOSP release tag for unmodified projects and the NottOS branch for forks.

- The common practice among AOSP derivatives; rebasing onto a new upstream
  tag is ordinary Git work; history per project stays readable.
- Needs an organisation with many repositories.

### Option B — patch series kept in one repository

AOSP is checked out unmodified; a script applies NottOS patches.

- Works with a single repository.
- Patches break silently when upstream moves; reviewing a change to a patch
  means reading a diff of a diff; tooling expects real commits.

### Option C — import AOSP into one repository

- Hundreds of gigabytes of history. Not workable.

## Decision

**Option A**, with these rules:

1. **Until the organisation exists, no AOSP project is modified.** NottOS's
   own code lives in this tree. This avoids inventing an interim patch
   format that would have to be migrated.
2. Forks carry one branch per Android release, **rebased** onto each
   upstream tag, never merged, so the NottOS delta is always a short list
   of commits on top of upstream.
3. One topic per commit. Subjects start with `NottOS:`.
4. Every change to an AOSP project has an entry in the **deviation
   registry** under `platform/`: what changed, why, and which CTS tests it
   affects. A change without an entry fails review.
5. The manifest pins exact revisions. Moving a pin is a reviewed commit.

## Consequences

- **Positive:** standard tooling; small, reviewable delta; a clear list of
  every deviation from Android.
- **Negative:** the owner must create and administer an organisation; many
  repositories to mirror and protect.
- **Blocked until then:** all work inside AOSP projects (Phases 2 and 3 in
  large part).

## Owner action required

Create a dedicated organisation for NottOS repositories.

## Revisit when

The number of patched projects turns out to be very small (a handful). A
patch series might then be simpler after all.

## Verification

The manifest checks out and builds on the build host. UNVERIFIED.
