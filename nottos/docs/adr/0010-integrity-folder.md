# ADR-0010: Integrity folder

- **Status:** Accepted (2026-11-14; decided under the owner's delegation)
- **Date:** 2026-11-14
- **Decider:** project owner (delegated)

## Context

The brief (section 10) asks for a protected directory, proposed as
`/nott/attest`, holding what a user needs to check that the system is
genuine: the release public keys (OpenPGP and hybrid post-quantum), the
signed build manifest with the hashes of every system partition and of the
verified-boot key, detached signatures, and a README. It must live on the
verified read-only system image, be unwritable by every production domain,
change only through a signed update, and be read by a "Verify system" app.
The brief also asks that the limitation be stated: a compromised system can
lie about itself, so verification from outside must exist.

**The requirement as written is impossible** (requirements review, C-25).
A manifest stored inside the system image cannot contain that image's own
digest: writing the digest into the image changes the image. The same
holds for any image that contains the manifest.

## Options considered

### Option A — folder on the system image; the manifest omits that image

- Keeps the brief's location.
- The one partition that matters most is the one the folder cannot vouch
  for. The Verify app would have to trust the running kernel's word for the
  system image's root hash.

### Option B — folder on its own small verified partition

`/nott/attest` is the mount point of a dedicated read-only partition
(`nott_attest`), verified by the boot chain like every other partition. The
manifest inside lists every other partition. The folder's own partition is
covered by the boot metadata (`vbmeta` on Android), which the bootloader
verifies with the verified-boot key, whose digest the manifest records.

- No circularity: the manifest vouches for the system image; the boot
  metadata vouches for the folder; the bootloader vouches for the boot
  metadata.
- On Android a new logical partition in the dynamic partition layout
  `[K]`: a small device-configuration change, available on devices that
  launched with Android 10 or later.

### Option C — the manifest outside the device only

- Simple, and external verification is the trustworthy kind anyway.
- Drops the brief's on-device folder and the Verify app's self-check.

## Decision

**Option B.**

| Item | Decision |
|---|---|
| Path | `/nott/attest`, final |
| Storage | A dedicated read-only partition, EROFS under dm-verity, mounted `ro,nosuid,nodev,noexec` |
| Covered by | The boot metadata, signed by the release key for the purpose `boot-metadata` and verified before the partition is used |
| Written by | Only the build. A new folder arrives with a signed update as a new partition image; there is no write path on a running system |

Contents:

| File | What it is |
|---|---|
| `manifest` | Build manifest: version, build ID, security patch level, size and SHA-256 of every image except the folder's own partition and the boot metadata, SHA-256 of the verified-boot key |
| `manifest.sig` | Hybrid signature of the manifest (suite 1, purpose `build-manifest`) |
| `manifest.asc` | OpenPGP detached signature of the manifest |
| `release.public` | Hybrid release public key |
| `release.asc` | OpenPGP release public key |
| `README.txt` | Verification steps in Brazilian Portuguese and English; the key id and OpenPGP fingerprint; the limitation |

On a release device the folder will also hold the update root metadata and
its signatures (ADR-0009), so that the update trust anchor can be checked
the same way.

**OpenPGP is a convenience, not a guarantee.** OpenPGP signatures here are
classical (Ed25519) and exist so that anyone can check a release with
standard tools. The hybrid signature is the authoritative one.

Protection, in layers:

1. The partition is verified at boot and on every read (dm-verity). A
   changed block cannot be read back; it causes an I/O error.
2. The filesystem is read-only by design (EROFS); it cannot be remounted
   writable.
3. On Android: a dedicated SELinux type for the folder with no write rule
   for any production domain, checked by a neverallow rule in the policy
   build (INV-14). Not yet implemented.
4. Owner-enabled root (planned ADR) cannot write the folder in place
   either. What root mode can do, and how it shows, is decided there.

What a "Verify system" app checks, and what it cannot:

- It verifies the manifest signature against the key in the folder, then
  recomputes the digest of every partition the manifest lists, and shows
  pass or fail with the build fingerprint.
- It cannot detect a compromise of the running system, which can show a
  false result. The README says so in both languages and lists the checks
  made from outside: the key fingerprint at boot, hardware attestation
  from a second device, and rebuilding from source.

## Consequences

- **Positive:** every partition has exactly one signed statement covering
  it; the folder is as tamper-evident as the system itself; checks work
  both on the device and from another computer, with NottOS tools or with
  standard OpenPGP tools.
- **Negative:** one more partition per device; a deviation from the
  brief's wording, recorded in the requirements review.
- **Negative:** the boot image and similar partitions loaded by the
  bootloader may not be readable from the running system on every device;
  the Verify app then reports them as "not checked" rather than "passed".

## Verification

On the minimal Linux test target (`tests/mini`, ADR-0007), where the
initramfs stands in for the bootloader:

- T4d: boot metadata whose folder root hash was replaced is refused.
- T7a: in the running system the folder is mounted read-only from the
  verified image; eight write attempts by root (create, append, delete,
  rename, make a directory, remount writable, write the verity device,
  write the disk) are all refused; the in-system check passes for the
  system image and its hash tree.
- T7b: from the host, the NottOS tool verifies every image including the
  boot image, and GnuPG verifies the OpenPGP signature; an edited copy of
  the manifest fails both.
- T7c: one flipped bit in the folder's manifest makes the in-system check
  fail with an I/O error, and the kernel reports dm-verity corruption.

Not verified: SELinux protection, the Verify app itself, and anything on
Android or a device.
