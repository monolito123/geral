#!/bin/sh
# Boots the minimal Linux test target in QEMU and checks what it does.
# See README.md in this directory.
#
# usage: run-tests.sh MINI_DIR NOTT_TOOLS [shell]
set -eu

HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
[ $# -ge 2 ] || {
    echo "usage: run-tests.sh MINI_DIR NOTT_TOOLS [shell]" >&2
    exit 2
}
MINI=$(CDPATH='' cd -- "$1" && pwd)
TOOLS=$2
RUN="$MINI/run"
TIMEOUT=${NOTT_MINI_TIMEOUT:-300}

ACCEL=tcg
if [ -w /dev/kvm ]; then ACCEL=kvm; fi

# boot LOG INITRAMFS SYSTEM_IMAGE KERNEL_ARGUMENTS
# Uses $RUN/data.img and $RUN/scratch.img as the writable disks.
boot() {
    timeout "$TIMEOUT" qemu-system-x86_64 \
        -accel "$ACCEL" -cpu max -m 1024 -smp 2 \
        -nodefaults -no-reboot -display none -serial "file:$1" \
        -kernel "$MINI/vmlinuz" -initrd "$2" \
        -append "console=ttyS0 loglevel=4 panic=-1 $4" \
        -drive "file=$3,if=virtio,format=raw,readonly=on" \
        -drive "file=$MINI/system.verity,if=virtio,format=raw,readonly=on" \
        -drive "file=$RUN/data.img,if=virtio,format=raw" \
        -drive "file=$RUN/scratch.img,if=virtio,format=raw" || true
    # Serial output uses CR LF line endings.
    tr -d '\r' <"$1" >"$1.txt"
}

if [ "${3:-}" = shell ]; then
    rm -rf "$RUN"
    mkdir -p "$RUN"
    cp "$MINI/data.img" "$MINI/scratch.img" "$RUN/"
    echo "mini: starting a shell; 'poweroff -f' leaves it"
    exec qemu-system-x86_64 -accel "$ACCEL" -cpu max -m 1024 -smp 2 \
        -nodefaults -no-reboot -display none -serial mon:stdio \
        -kernel "$MINI/vmlinuz" -initrd "$MINI/initramfs.cpio.gz" \
        -append "console=ttyS0 loglevel=4 panic=-1" \
        -drive "file=$MINI/system.img,if=virtio,format=raw,readonly=on" \
        -drive "file=$MINI/system.verity,if=virtio,format=raw,readonly=on" \
        -drive "file=$RUN/data.img,if=virtio,format=raw" \
        -drive "file=$RUN/scratch.img,if=virtio,format=raw"
fi

FAILED=0
pass() {
    echo "  ok    $*"
}
fail() {
    echo "  FAIL  $*"
    FAILED=$((FAILED + 1))
}
# expect LOG TEXT: the log contains a line equal to "NOTT-MINI: TEXT".
expect() {
    if grep -q -x -F "NOTT-MINI: $2" "$1.txt"; then pass "$2"; else fail "missing: $2"; fi
}
# refuse LOG TEXT: the log contains no line equal to "NOTT-MINI: TEXT".
refuse() {
    if grep -q -x -F "NOTT-MINI: $2" "$1.txt"; then fail "unexpected: $2"; else pass "absent: $2"; fi
}
# count FILE TEXT: number of lines of FILE that contain TEXT.
count() {
    grep -c -a -F "$2" "$1" || true
}
fresh_disks() {
    cp "$MINI/data.img" "$MINI/scratch.img" "$RUN/"
}
# flip_byte IMAGE OFFSET: inverts the lowest bit of one byte.
flip_byte() {
    old=$(dd if="$1" bs=1 skip="$2" count=1 2>/dev/null | od -An -tu1 | tr -d ' ')
    new=$((old ^ 1))
    # The byte is written as an octal escape.
    # shellcheck disable=SC2059
    printf "\\$(printf '%03o' "$new")" |
        dd of="$1" bs=1 seek="$2" conv=notrunc 2>/dev/null
}
# offset_of IMAGE TEXT: byte offset of the first occurrence of TEXT.
offset_of() {
    grep -o -b -a -F "$2" "$1" | head -n 1 | cut -d: -f1
}

rm -rf "$RUN"
mkdir -p "$RUN"
echo "mini: kernel $(cat "$MINI/kernel-version"), accelerator $ACCEL"

# --- T1: verified boot to a shell ------------------------------------------
echo "T1 boot from an untampered system image"
fresh_disks
boot "$RUN/t1.log" "$MINI/initramfs.cpio.gz" "$MINI/system.img" "nott.test=boot"
expect "$RUN/t1.log" "shell ready"
expected=$(yes "NOTT-MINI-NONCRITICAL-DATA-LINE" | head -c 65536 | sha256sum | cut -d' ' -f1)
expect "$RUN/t1.log" "noncritical: read ok sha256=$expected"

# --- T6: every mount is verified, volatile or encrypted (INV-02) -----------
echo "T6 storage policy"
sed -n '/^NOTT-MINI: BEGIN storage-report$/,/^NOTT-MINI: END storage-report$/p' \
    "$RUN/t1.log.txt" | grep -v -e '^NOTT-MINI: ' -e '^\[ *[0-9.]*\] ' \
    >"$RUN/storage-report.txt" || true
if "$TOOLS" storage check "$RUN/storage-report.txt" \
    --exceptions "$HERE/storage-exceptions.txt" >"$RUN/storage-check.txt"; then
    pass "storage policy holds with the listed exception"
else
    fail "storage policy violated (see $RUN/storage-check.txt)"
fi
# Control: without the exception list the plain scratch disk must be caught.
if "$TOOLS" storage check "$RUN/storage-report.txt" >/dev/null; then
    fail "control: the checker did not flag the unencrypted scratch disk"
else
    pass "control: the checker flags the unencrypted scratch disk"
fi

# --- T2: tampering with a block that boot needs ----------------------------
echo "T2 one flipped bit in a boot-critical block"
fresh_disks
cp "$MINI/system.img" "$RUN/system-critical.img"
flip_byte "$RUN/system-critical.img" \
    "$(offset_of "$RUN/system-critical.img" NOTT-MINI-STAGE2-MARKER)"
boot "$RUN/t2.log" "$MINI/initramfs.cpio.gz" "$RUN/system-critical.img" "nott.test=boot"
refuse "$RUN/t2.log" "shell ready"
if grep -q -i 'verity.*corrupt' "$RUN/t2.log.txt"; then
    pass "kernel reported dm-verity corruption"
else
    fail "no dm-verity corruption report in the kernel log"
fi

# --- T3: tampering with a block that boot does not need --------------------
echo "T3 one flipped bit in a block boot does not read"
fresh_disks
cp "$MINI/system.img" "$RUN/system-noncritical.img"
start=$(offset_of "$RUN/system-noncritical.img" NOTT-MINI-NONCRITICAL-DATA-LINE)
flip_byte "$RUN/system-noncritical.img" $((start + 32768))
boot "$RUN/t3.log" "$MINI/initramfs.cpio.gz" "$RUN/system-noncritical.img" "nott.test=boot"
expect "$RUN/t3.log" "shell ready"
expect "$RUN/t3.log" "noncritical: read error"
if [ "$(count "$RUN/t3.log.txt" 'noncritical: read ok')" -eq 0 ]; then
    pass "tampered data was never returned"
else
    fail "tampered data was returned"
fi

# --- T4: boot image carries a different root hash --------------------------
echo "T4 boot image with a different root hash"
fresh_disks
boot "$RUN/t4.log" "$MINI/initramfs-badhash.cpio.gz" "$MINI/system.img" "nott.test=boot"
refuse "$RUN/t4.log" "shell ready"

# --- T5: encrypted storage --------------------------------------------------
echo "T5 encrypted storage"
CONTENT=NOTT-CANARY-CONTENT-7f3a9c1e5b2d8046
NAME=NOTT-CANARY-NAME-1c9e4b7a2f6d8053
DEVICE=NOTT-CANARY-DEVICE-55aa33cc0f1e2d3c
PLAIN=NOTT-CANARY-PLAIN-9b8a7c6d5e4f3021
CANARIES="nott.content=$CONTENT nott.name=$NAME nott.device=$DEVICE nott.plain=$PLAIN"
fresh_disks
boot "$RUN/t5-provision.log" "$MINI/initramfs.cpio.gz" "$MINI/system.img" \
    "nott.test=provision nott.pass=correct-horse $CANARIES"
expect "$RUN/t5-provision.log" "provision: done"

# Raw disk images, as an attacker with the storage chip would see them.
check_absent() {
    if [ "$(count "$RUN/data.img" "$1")" -eq 0 ]; then
        pass "$2 is absent from the raw data image"
    else
        fail "$2 is present in the raw data image"
    fi
}
check_absent "$CONTENT" "file content"
check_absent "$NAME" "file name"
check_absent "$DEVICE" "content outside the passphrase-protected directory"
if [ "$(count "$RUN/scratch.img" "$PLAIN")" -ge 1 ]; then
    pass "control: the scan finds plaintext on the unencrypted scratch disk"
else
    fail "control: the scan missed plaintext on the unencrypted scratch disk"
fi

boot "$RUN/t5-locked.log" "$MINI/initramfs.cpio.gz" "$MINI/system.img" \
    "nott.test=locked $CANARIES"
expect "$RUN/t5-locked.log" "locked: entries=1"
expect "$RUN/t5-locked.log" "locked: name hidden"
expect "$RUN/t5-locked.log" "locked: read denied"
expect "$RUN/t5-locked.log" "locked: device file readable"

boot "$RUN/t5-wrongpass.log" "$MINI/initramfs.cpio.gz" "$MINI/system.img" \
    "nott.test=wrongpass nott.pass=wrong-horse $CANARIES"
expect "$RUN/t5-wrongpass.log" "wrongpass: read denied"

boot "$RUN/t5-unlock.log" "$MINI/initramfs.cpio.gz" "$MINI/system.img" \
    "nott.test=unlock nott.pass=correct-horse $CANARIES"
expect "$RUN/t5-unlock.log" "unlock: content ok"

echo
if [ "$FAILED" -eq 0 ]; then
    echo "mini: PASS (logs in $RUN)"
else
    echo "mini: FAIL, $FAILED check(s) failed (logs in $RUN)"
    exit 1
fi
