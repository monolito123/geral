# Minimal Linux test target

A very small Linux system that boots in QEMU in a few seconds. It exists so
that NottOS components that live in or next to the kernel can be exercised
on any development machine and in CI, without an AOSP build host.

**It is not NottOS.** It contains no Android code, no NottOS product code
and no hardware root of trust. Passing these tests says nothing about the
Cuttlefish target or about any phone. See
[ADR-0007](../../docs/adr/0007-minimal-linux-test-target.md).

## What it is made of

| Piece | Stands in for | Notes |
|---|---|---|
| Distribution kernel, unmodified | The device kernel | Newest `linux-image-*-generic` from apt; version recorded in `kernel-version` |
| `initramfs.cpio.gz` (stage 1) | The bootloader-verified boot image | Carries the dm-verity root hash. Nothing verifies the initramfs itself: QEMU loads whatever it is given. |
| `system.img` + `system.verity` | The read-only system partition | EROFS under dm-verity |
| `data.img` | The user-data partition | dm-crypt over the whole partition (as metadata encryption does), ext4 with one fscrypt-protected directory (as credential-encrypted storage does) |
| `scratch.img` | A vendor state partition | Plain ext4, mounted `nosuid,nodev,noexec`, on the exception list |

The data-partition key is a fixed test key inside the boot image. On a phone
that key is wrapped by the TEE and bound to the verified-boot state. Here it
protects nothing against someone who holds the boot image.

## Running

```sh
./nott mini test     # build the images and run every test
./nott mini shell    # boot to an interactive shell on the serial console
```

Needs: `qemu-system-x86`, `erofs-utils`, `cryptsetup-bin`, `dmsetup`,
`e2fsprogs`, `kmod`, `cpio`, `zstd`, `busybox-static`, `openssl`, and apt
access to download the kernel packages (about 180 MB, cached under
`out/mini/cache`). KVM is used when `/dev/kvm` is writable; otherwise QEMU
emulates, which takes a few seconds per boot.

## Tests

| Test | What is done | What must happen |
|---|---|---|
| T1 | Boot the untampered images | Reaches the shell marker from the verified system image; a 64 KiB file reads back with the expected digest |
| T2 | Flip one bit in a block that boot reads | Never reaches the shell marker; the kernel logs dm-verity corruption |
| T3 | Flip one bit in a block that boot does not read | Boots; reading the affected file fails with an I/O error; tampered data is never returned |
| T4 | Boot image carries a different root hash | Never reaches the shell marker |
| T5 | Write secrets, power off, inspect the raw disk; then boot without the passphrase, with a wrong one, with the right one | File content and file name are absent from the raw image; without the right passphrase the name is unreadable and the content inaccessible; with it the content is intact |
| T6 | Collect the mount table and check it with `nott-tools storage check` | Every mount is verified read-only, volatile, a kernel interface, encrypted, or the listed exception |

Each test that expects absence has a control that shows the check can fail:
the raw-disk scan must find a plaintext canary on the unencrypted scratch
disk, and the storage check must flag that disk when the exception list is
withheld.

## Relation to the roadmap

T1 to T6 are scaled-down analogues of Phase 1 acceptance criteria 1.2, 1.3,
1.5 and 1.6. They do not satisfy those criteria, which are about the
Cuttlefish target. Criterion 1.4 (an image signed with a different key does
not boot) has no analogue here, because nothing in this target verifies a
signature.
