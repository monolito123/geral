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
#   manifest              build manifest: system.img, system.verity and
#                         initramfs.cpio.gz, signed by the release key
#   attest/               the integrity folder, as mounted at /nott/attest
#   attest.img            the integrity folder as a read-only EROFS image
#   attest.verity         dm-verity hash tree for attest.img
#   attest.roothash       dm-verity root hash of attest.img
#   boot-metadata         the two root hashes, as a manifest of their own
#   meta.img              boot metadata disk: boot-metadata, its signature
#                         by the release key, and the two root hashes
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
    e4crypt gpg gpgconf gzip ldd mke2fs mkfs.erofs openssl veritysetup zstd; do
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

# verity_image NAME: dm-verity hash tree and root hash for $OUT/NAME.img.
verity_image() {
    veritysetup format \
        --salt "$(printf 'nott-mini-%s' "$1" | sha256sum | cut -d' ' -f1)" \
        "$OUT/$1.img" "$OUT/$1.verity" |
        sed -n 's/^Root hash:[[:space:]]*//p' >"$OUT/$1.roothash"
    [ "$(wc -c <"$OUT/$1.roothash")" -eq 65 ] || {
        echo "mini: could not read the dm-verity root hash of $1" >&2
        exit 1
    }
}

# --- system image -----------------------------------------------------------
SYSTEM="$WORK/system"
mkdir -p "$SYSTEM/etc/nott" "$SYSTEM/dev" "$SYSTEM/proc" "$SYSTEM/sys" \
    "$SYSTEM/run" "$SYSTEM/tmp" "$SYSTEM/data" "$SYSTEM/scratch" \
    "$SYSTEM/nott/attest"
install_busybox "$SYSTEM"
install_programs "$SYSTEM" dmsetup e4crypt mke2fs "$TOOLS"
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
verity_image system

# --- keys -------------------------------------------------------------------
# Test keys, new on every build. "release" is the key the boot image trusts;
# "other" is a key it does not. The OpenPGP key is for people who verify the
# integrity folder by hand with standard tools.
rm -rf "$OUT/keys"
mkdir -p "$OUT/keys"
for name in release other; do
    "$TOOLS" key generate --out-secret "$OUT/keys/$name.secret" \
        --out-public "$OUT/keys/$name.public" >/dev/null
done
GNUPGHOME="$WORK/gnupg"
export GNUPGHOME
mkdir -m 700 "$GNUPGHOME"
gpg --quiet --batch --pinentry-mode loopback --passphrase '' \
    --quick-gen-key 'NottOS mini test release key' ed25519 sign never 2>/dev/null
gpg --quiet --armor --export >"$OUT/keys/release.asc"
PGP_FINGERPRINT=$(gpg --with-colons --list-keys | awk -F: '/^fpr:/ { print $10; exit }')

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

# --- build manifest ---------------------------------------------------------
PATCH_LEVEL=$(date -u -d "@${SOURCE_DATE_EPOCH:-$(date +%s)}" +%Y-%m-%d)
"$TOOLS" manifest create --version 0.1.0 --build-id MINI \
    --security-patch "$PATCH_LEVEL" \
    --target mini_x86_64 --vb-key "$OUT/keys/release.public" \
    --image "system.img=$OUT/system.img" \
    --image "system.verity=$OUT/system.verity" \
    --image "initramfs.cpio.gz=$OUT/initramfs.cpio.gz" \
    --out "$OUT/manifest" >/dev/null

# --- integrity folder (/nott/attest) ----------------------------------------
# On its own verified, read-only image: a manifest stored inside the system
# image could not list that image's own digest (docs/adr/0010).
ATTEST="$OUT/attest"
rm -rf "$ATTEST"
mkdir -p "$ATTEST"
cp "$OUT/manifest" "$ATTEST/manifest"
cp "$OUT/keys/release.public" "$ATTEST/release.public"
cp "$OUT/keys/release.asc" "$ATTEST/release.asc"
"$TOOLS" sign "$ATTEST/manifest" --key "$OUT/keys/release.secret" \
    --context build-manifest --out "$ATTEST/manifest.sig" >/dev/null
gpg --quiet --batch --armor --detach-sign --output "$ATTEST/manifest.asc" \
    "$ATTEST/manifest"
RELEASE_KEY_ID=$("$TOOLS" key id "$ATTEST/release.public" | sed -n 's/^key-id: //p')
cat >"$ATTEST/README.txt" <<EOF
NottOS integrity folder / Pasta de integridade do NottOS
========================================================

Test build of the minimal Linux test target. Not a NottOS release.
Build de teste do alvo mínimo de testes. Não é uma versão do NottOS.

Release key id (hybrid Ed25519 + ML-DSA-87):
  $RELEASE_KEY_ID
OpenPGP key fingerprint (Ed25519, classical only):
  $PGP_FINGERPRINT

Files / Arquivos
  manifest        version, build, patch level, size and SHA-256 of every
                  image, SHA-256 of the verified-boot key
  manifest.sig    hybrid signature of manifest (context build-manifest)
  manifest.asc    OpenPGP signature of manifest
  release.public  hybrid release public key
  release.asc     OpenPGP release public key

Verify on another computer (EN) / Verifique em outro computador (PT)
  1. Compare the key id and fingerprint above with the ones published
     on the project's site. / Compare o id e a impressão digital acima
     com os publicados no site do projeto.
  2. nott-tools manifest verify manifest --dir IMAGES \\
       --public-key release.public --signature manifest.sig
  3. gpg --import release.asc && gpg --verify manifest.asc manifest
  4. Rebuild from source and compare the digests in manifest.
     Recompile a partir do código-fonte e compare os hashes do manifest.

A system that is already compromised can show you a false result. Only
checks made outside the running system (steps 1 to 4, the key
fingerprint at boot, hardware attestation from a second device) are
trustworthy.
Um sistema já comprometido pode mostrar um resultado falso. Só as
verificações feitas fora do sistema em execução são confiáveis.
EOF
gpgconf --kill gpg-agent 2>/dev/null || true
# File data in blocks of its own, so that a test can damage one file's
# content without touching the filesystem's metadata.
mkfs.erofs --quiet --all-root -T 0 -E noinline_data \
    -U 6e6f7474-6d69-6e69-0000-000000000003 "$OUT/attest.img" "$ATTEST"
verity_image attest

# --- signed boot metadata ---------------------------------------------------
# The root hashes of both verified images, as a manifest of their own,
# signed for the purpose "boot-metadata" (what vbmeta is on Android).
"$TOOLS" manifest create --version 0.1.0 --build-id MINI \
    --security-patch "$PATCH_LEVEL" \
    --target mini_x86_64-boot --vb-key "$OUT/keys/release.public" \
    --image "system.roothash=$OUT/system.roothash" \
    --image "attest.roothash=$OUT/attest.roothash" \
    --out "$OUT/boot-metadata" >/dev/null
for name in release other; do
    "$TOOLS" sign "$OUT/boot-metadata" --key "$OUT/keys/$name.secret" \
        --context boot-metadata --out "$WORK/boot-metadata.$name.sig" >/dev/null
done

# The metadata disk has no filesystem and no archive format, so that the
# boot stage parses nothing before the signature is checked: four slots of
# 64 KiB each, holding the boot metadata, its signature and the two root
# hashes, each padded with zero bytes.
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
        slot "$OUT/boot-metadata"
        slot "$WORK/boot-metadata.$name.sig"
        slot "$OUT/system.roothash"
        slot "$OUT/attest.roothash"
    } >"$WORK/meta.$name.img"
done
mv "$WORK/meta.release.img" "$OUT/meta.img"
mv "$WORK/meta.other.img" "$OUT/meta-otherkey.img"

# --- data and scratch -------------------------------------------------------
rm -f "$OUT/data.img" "$OUT/scratch.img"
truncate -s 64M "$OUT/data.img"
truncate -s 16M "$OUT/scratch.img"
mke2fs -q -t ext4 -F -E root_owner=0:0 "$OUT/scratch.img"

rm -rf "$WORK"
echo "mini: built in $OUT (kernel $KERNEL, root hash $(cut -c1-16 "$OUT/system.roothash")...)"
