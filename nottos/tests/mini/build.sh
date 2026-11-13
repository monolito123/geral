#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
# Builds the minimal Linux test target. See README.md in this directory.
#
# usage: build.sh OUT_DIR NOTT_TOOLS
#
# Produces in OUT_DIR:
#   vmlinuz               distribution kernel, unmodified
#   initramfs.cpio.gz     the "boot image": stage 1, modules, nott-tools and
#                         the trusted release public key
#   system.img            read-only EROFS system image
#   system.verity         dm-verity hash tree for system.img
#   system.roothash       dm-verity root hash of system.img
#   manifest              build manifest listing the three files above
#   meta.img              boot metadata: manifest, its signature by the
#                         release key, and the root hash
#   meta-otherkey.img     the same, signed by a key the boot image does not
#                         trust
#   keys/                 test keys, generated afresh by every build
#   data.img              empty; encrypted and formatted on first boot
#   scratch.img           plain ext4; the deliberate, listed exception
#   noncritical.sha256    digest of a file in system.img that boot ignores
set -eu

HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
[ $# -eq 2 ] || {
    echo "usage: build.sh OUT_DIR NOTT_TOOLS" >&2
    exit 2
}
mkdir -p "$1/cache"
OUT=$(CDPATH='' cd -- "$1" && pwd)
TOOLS=$(CDPATH='' cd -- "$(dirname -- "$2")" && pwd)/$(basename -- "$2")
[ -x "$TOOLS" ] || {
    echo "mini: not executable: $2" >&2
    exit 2
}
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
# the tree at $1. Programs are given by name or by path and go to $1/sbin.
install_programs() {
    tree=$1
    shift
    mkdir -p "$tree/sbin"
    for program in "$@"; do
        path=$(command -v "$program")
        cp "$path" "$tree/sbin/$(basename "$program")"
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
# A file that boot does not need. Exactly 64 KiB (2048 lines of 32 bytes),
# so that it occupies whole blocks of its own and tampering with it leaves
# boot-critical blocks alone. Its digest is recorded for the tests.
awk 'BEGIN { for (i = 0; i < 2048; i++) print "NOTT-MINI-NONCRITICAL-DATA-LINE" }' \
    >"$SYSTEM/etc/nott/noncritical.txt"
[ "$(wc -c <"$SYSTEM/etc/nott/noncritical.txt")" -eq 65536 ] || {
    echo "mini: noncritical.txt is not 64 KiB" >&2
    exit 1
}
sha256sum <"$SYSTEM/etc/nott/noncritical.txt" | cut -d' ' -f1 \
    >"$OUT/noncritical.sha256"

mkfs.erofs --quiet --all-root -T 0 \
    -U 6e6f7474-6d69-6e69-0000-000000000001 "$OUT/system.img" "$SYSTEM"
veritysetup format \
    --salt 6e6f74746d696e692d73797374656d2d73616c74000000000000000000000001 \
    --uuid 6e6f7474-6d69-6e69-0000-000000000002 \
    "$OUT/system.img" "$OUT/system.verity" |
    sed -n 's/^Root hash:[[:space:]]*//p' >"$OUT/system.roothash"
[ "$(wc -c <"$OUT/system.roothash")" -eq 65 ] || {
    echo "mini: could not read the dm-verity root hash" >&2
    exit 1
}

# --- signed boot metadata ---------------------------------------------------
# Test keys, new on every build. "release" is the key the boot image trusts;
# "other" is a key it does not.
rm -rf "$OUT/keys"
mkdir -p "$OUT/keys"
for name in release other; do
    "$TOOLS" key generate --out-secret "$OUT/keys/$name.secret" \
        --out-public "$OUT/keys/$name.public" >/dev/null
done
"$TOOLS" manifest create --version 0.1.0 --build-id MINI \
    --security-patch "$(date -u -d "@${SOURCE_DATE_EPOCH:-$(date +%s)}" +%Y-%m-%d)" \
    --target mini_x86_64 --vb-key "$OUT/keys/release.public" \
    --image "system.img=$OUT/system.img" \
    --image "system.verity=$OUT/system.verity" \
    --image "system.roothash=$OUT/system.roothash" \
    --out "$OUT/manifest" >/dev/null
for name in release other; do
    "$TOOLS" sign "$OUT/manifest" --key "$OUT/keys/$name.secret" \
        --context build-manifest --out "$WORK/manifest.$name.sig" >/dev/null
done

# The metadata disk has no filesystem and no archive format, so that the
# boot stage parses nothing before the signature is checked: three slots of
# 64 KiB each, holding the manifest, its signature and the root hash, each
# padded with zero bytes.
SLOT=65536
slot() {
    size=$(wc -c <"$1")
    [ "$size" -le "$SLOT" ] || {
        echo "mini: $1 does not fit a metadata slot" >&2
        exit 1
    }
    cat "$1"
    head -c $((SLOT - size)) /dev/zero
}
for name in release other; do
    {
        slot "$OUT/manifest"
        slot "$WORK/manifest.$name.sig"
        slot "$OUT/system.roothash"
    } >"$WORK/meta.$name.img"
done
mv "$WORK/meta.release.img" "$OUT/meta.img"
mv "$WORK/meta.other.img" "$OUT/meta-otherkey.img"

# --- boot image (initramfs) -------------------------------------------------
BOOT="$WORK/initramfs"
mkdir -p "$BOOT/etc/nott" "$BOOT/modules" "$BOOT/dev" "$BOOT/proc" \
    "$BOOT/sys" "$BOOT/run" "$BOOT/system"
install_busybox "$BOOT"
install_programs "$BOOT" veritysetup cryptsetup "$TOOLS"
# The trust anchor: the only key whose signature on boot metadata counts.
cp "$OUT/keys/release.public" "$BOOT/etc/nott/release.public"
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

(cd "$BOOT" && find . | LC_ALL=C sort |
    cpio --quiet -o -H newc --owner=0:0) | gzip -n >"$OUT/initramfs.cpio.gz"

# --- data and scratch -------------------------------------------------------
rm -f "$OUT/data.img" "$OUT/scratch.img"
truncate -s 64M "$OUT/data.img"
truncate -s 16M "$OUT/scratch.img"
mke2fs -q -t ext4 -F -E root_owner=0:0 "$OUT/scratch.img"

rm -rf "$WORK"
echo "mini: built in $OUT (kernel $KERNEL, root hash $(cut -c1-16 "$OUT/system.roothash")...)"
