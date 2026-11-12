#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
# Builds the minimal Linux test target. See README.md in this directory.
#
# usage: build.sh OUT_DIR
#
# Produces in OUT_DIR:
#   vmlinuz               distribution kernel, unmodified
#   initramfs.cpio.gz     the "boot image": stage 1, modules, root hash, key
#   initramfs-badhash.cpio.gz  same, with one digit of the root hash changed
#   system.img            read-only EROFS system image
#   system.verity         dm-verity hash tree for system.img
#   data.img              empty; encrypted and formatted on first boot
#   scratch.img           plain ext4; the deliberate, listed exception
#   roothash              dm-verity root hash of system.img
set -eu

HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
[ $# -eq 1 ] || {
    echo "usage: build.sh OUT_DIR" >&2
    exit 2
}
mkdir -p "$1/cache"
OUT=$(CDPATH='' cd -- "$1" && pwd)
WORK="$OUT/work"
# Several of the tools below live in sbin, which is not always on PATH.
PATH=$PATH:/usr/sbin:/sbin

for tool in apt-cache apt-get busybox cpio cryptsetup depmod dmsetup dpkg-deb \
    e4crypt gzip ldd mke2fs mkfs.erofs openssl veritysetup zstd; do
    command -v "$tool" >/dev/null || {
        echo "mini: missing tool: $tool" >&2
        exit 2
    }
done
if ldd "$(command -v busybox)" >/dev/null 2>&1; then
    echo "mini: busybox must be statically linked (package busybox-static)" >&2
    exit 2
fi

# --- kernel -----------------------------------------------------------------
# The newest generic kernel the distribution offers, or the one named in
# NOTT_MINI_KERNEL. The version is recorded in OUT_DIR/kernel-version.
KERNEL=${NOTT_MINI_KERNEL:-$(apt-cache search --names-only \
    '^linux-image-[0-9.]+-[0-9]+-generic$' |
    sed 's/ .*//; s/^linux-image-//' | sort -V | tail -n 1)}
[ -n "$KERNEL" ] || {
    echo "mini: no linux-image-*-generic package found" >&2
    exit 2
}
KDIR="$OUT/cache/kernel-$KERNEL"
if [ ! -f "$KDIR/boot/vmlinuz-$KERNEL" ]; then
    rm -rf "$KDIR" "$OUT/cache/debs"
    mkdir -p "$OUT/cache/debs"
    (cd "$OUT/cache/debs" &&
        apt-get download "linux-image-$KERNEL" "linux-modules-$KERNEL")
    for deb in "$OUT"/cache/debs/*.deb; do
        dpkg-deb -x "$deb" "$KDIR"
    done
    rm -rf "$OUT/cache/debs"
fi
MODULES=$(find "$KDIR" -type d -path "*/modules/$KERNEL" | head -n 1)
# The package ships no dependency index; it is generated at install time.
[ -f "$MODULES/modules.dep" ] || depmod -b "${MODULES%/lib/modules/*}" "$KERNEL"
cp "$KDIR/boot/vmlinuz-$KERNEL" "$OUT/vmlinuz"
echo "$KERNEL" >"$OUT/kernel-version"

rm -rf "$WORK"
mkdir -p "$WORK"

# Prints modules in load order, dependencies first. $1 is modules.dep, the
# rest are module paths as they appear in it.
module_order() {
    dep_file=$1
    shift
    for module in "$@"; do
        deps=$(sed -n "s|^$module: *||p" "$dep_file")
        # Dependencies are a space-separated list. The subshell keeps this
        # loop's variables intact.
        # shellcheck disable=SC2086
        [ -z "$deps" ] || (module_order "$dep_file" $deps)
        echo "$module"
    done
}

# Copies a statically linked busybox and one symlink per applet into $1/bin.
install_busybox() {
    mkdir -p "$1/bin"
    cp "$(command -v busybox)" "$1/bin/busybox"
    for applet in $(busybox --list); do
        [ "$applet" = busybox ] || ln -s busybox "$1/bin/$applet"
    done
}

# Copies dynamically linked programs, and the libraries they need, into
# the tree at $1. Programs go to $1/sbin.
install_programs() {
    tree=$1
    shift
    mkdir -p "$tree/sbin"
    for program in "$@"; do
        path=$(command -v "$program")
        cp "$path" "$tree/sbin/$program"
        ldd "$path" | sed -n 's|.* => \(/[^ ]*\) .*|\1|p; s|^[[:space:]]*\(/[^ ]*\) (0x.*|\1|p' |
            while read -r library; do
                mkdir -p "$tree$(dirname "$library")"
                cp -L "$library" "$tree$library"
            done
    done
}

# --- system image -----------------------------------------------------------
SYSTEM="$WORK/system"
mkdir -p "$SYSTEM/etc/nott" "$SYSTEM/dev" "$SYSTEM/proc" "$SYSTEM/sys" \
    "$SYSTEM/run" "$SYSTEM/tmp" "$SYSTEM/data" "$SYSTEM/scratch"
install_busybox "$SYSTEM"
install_programs "$SYSTEM" dmsetup e4crypt mke2fs
cp "$HERE/init-stage2" "$SYSTEM/sbin/init"
chmod 755 "$SYSTEM/sbin/init"
echo "nott-mini 1" >"$SYSTEM/etc/nott-release"
# A file that boot does not need. Exactly 64 KiB, so that it occupies whole
# blocks of its own and tampering with it leaves boot-critical blocks alone.
yes "NOTT-MINI-NONCRITICAL-DATA-LINE" | head -c 65536 \
    >"$SYSTEM/etc/nott/noncritical.txt"

mkfs.erofs --quiet --all-root -T 0 \
    -U 6e6f7474-6d69-6e69-0000-000000000001 "$OUT/system.img" "$SYSTEM"
veritysetup format \
    --salt 6e6f74746d696e692d73797374656d2d73616c74000000000000000000000001 \
    --uuid 6e6f7474-6d69-6e69-0000-000000000002 \
    "$OUT/system.img" "$OUT/system.verity" |
    sed -n 's/^Root hash:[[:space:]]*//p' >"$OUT/roothash"
[ "$(wc -c <"$OUT/roothash")" -eq 65 ] || {
    echo "mini: could not read the dm-verity root hash" >&2
    exit 1
}

# --- boot image (initramfs) -------------------------------------------------
BOOT="$WORK/initramfs"
mkdir -p "$BOOT/etc/nott" "$BOOT/modules" "$BOOT/dev" "$BOOT/proc" \
    "$BOOT/sys" "$BOOT/run" "$BOOT/system"
install_busybox "$BOOT"
install_programs "$BOOT" veritysetup cryptsetup
cp "$HERE/init-stage1" "$BOOT/init"
chmod 755 "$BOOT/init"

# Everything stage 1 needs from the kernel. Each item is either built in
# or a module to load; which one differs between kernel packages.
wanted=
for name in virtio_pci virtio_blk dm-mod dm-verity dm-crypt erofs ext4 xts; do
    module=$(sed -n "s|^\\(.*/$name\\.ko[^:]*\\):.*|\\1|p" "$MODULES/modules.dep")
    if [ -n "$module" ]; then
        wanted="$wanted $module"
    elif ! grep -q "/$name\\.ko\$" "$MODULES/modules.builtin"; then
        echo "mini: kernel $KERNEL has no $name, built in or as a module" >&2
        exit 1
    fi
done
# shellcheck disable=SC2086
module_order "$MODULES/modules.dep" $wanted | awk '!seen[$0]++' |
    while read -r module; do
        name=$(basename "$module" .zst)
        case "$module" in
        *.zst) zstd -q -d -f "$MODULES/$module" -o "$BOOT/modules/$name" ;;
        *) cp "$MODULES/$module" "$BOOT/modules/$name" ;;
        esac
        echo "$name"
    done >"$BOOT/modules/order"

# Test key for the data partition. On a phone this key is wrapped by the
# TEE and bound to the verified-boot state; here it sits in the boot image
# and protects nothing against someone who has that image. It exists so
# that the layering can be exercised.
printf 'nott-mini test metadata key' | openssl dgst -sha512 -binary \
    >"$BOOT/etc/nott/metadata.key"

# pack_boot OUTPUT: archives the boot tree with root ownership.
pack_boot() {
    (cd "$BOOT" && find . | LC_ALL=C sort |
        cpio --quiet -o -H newc --owner=0:0) | gzip -n >"$1"
}
cp "$OUT/roothash" "$BOOT/etc/nott/roothash"
pack_boot "$OUT/initramfs.cpio.gz"
# Same boot image with the first digit of the root hash changed.
first=$(cut -c1 "$OUT/roothash")
if [ "$first" = 0 ]; then other=1; else other=0; fi
echo "$other$(cut -c2- "$OUT/roothash")" >"$BOOT/etc/nott/roothash"
pack_boot "$OUT/initramfs-badhash.cpio.gz"

# --- data and scratch -------------------------------------------------------
rm -f "$OUT/data.img" "$OUT/scratch.img"
truncate -s 64M "$OUT/data.img"
truncate -s 16M "$OUT/scratch.img"
mke2fs -q -t ext4 -F -E root_owner=0:0 "$OUT/scratch.img"

rm -rf "$WORK"
echo "mini: built in $OUT (kernel $KERNEL, root hash $(cut -c1-16 "$OUT/roothash")...)"
