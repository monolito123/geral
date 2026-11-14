#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
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
TIMEOUT=${NOTT_MINI_TIMEOUT:-120}

ACCEL=tcg
if [ -w /dev/kvm ]; then ACCEL=kvm; fi

# The network tests give the machine one interface on QEMU's user-mode
# network, isolated from the host's own network, with this address.
GUEST_MAC=52:54:00:12:34:56
# Host port forwarded to the machine's port 8080, for probes from outside.
PROBE_PORT=$((20000 + $$ % 20000))
# Capture file for the next boot; empty for a boot without a network.
CAPTURE=
# File that holds the next boot's RAM, readable by the host while the machine
# runs; empty for ordinary RAM.
RAMFILE=

# boot LOG SYSTEM_IMAGE METADATA_IMAGE KERNEL_ARGUMENTS [ATTEST_IMAGE]
# Uses $RUN/data.img and $RUN/scratch.img as the writable disks. With
# $CAPTURE set, adds the network interface; QEMU records every frame on it,
# in both directions, from before the machine starts.
boot() {
    log=$1
    set -- -accel "$ACCEL" -cpu max -m 1024 -smp 2 \
        -nodefaults -no-reboot -display none -serial "file:$1" \
        -kernel "$MINI/vmlinuz" -initrd "$MINI/initramfs.cpio.gz" \
        -append "console=ttyS0 loglevel=4 panic=-1 $4" \
        -drive "file=$2,if=virtio,format=raw,readonly=on" \
        -drive "file=$MINI/system.verity,if=virtio,format=raw,readonly=on" \
        -drive "file=$RUN/data.img,if=virtio,format=raw" \
        -drive "file=$RUN/scratch.img,if=virtio,format=raw" \
        -drive "file=$3,if=virtio,format=raw,readonly=on" \
        -drive "file=${5:-$MINI/attest.img},if=virtio,format=raw,readonly=on" \
        -drive "file=$MINI/attest.verity,if=virtio,format=raw,readonly=on"
    if [ -n "$RAMFILE" ]; then
        set -- "$@" \
            -object "memory-backend-file,id=ram0,size=1024M,mem-path=$RAMFILE,share=on" \
            -machine memory-backend=ram0
    fi
    if [ -n "$CAPTURE" ]; then
        set -- "$@" \
            -netdev "user,id=net0,restrict=on,hostfwd=tcp:127.0.0.1:$PROBE_PORT-:8080" \
            -device "virtio-net-pci,netdev=net0,mac=$GUEST_MAC" \
            -object "filter-dump,id=dump0,netdev=net0,file=$CAPTURE"
    fi
    timeout "$TIMEOUT" qemu-system-x86_64 "$@" || true
    # Serial output uses CR LF line endings.
    tr -d '\r' <"$log" >"$log.txt"
}

# start_probes [LOG]: connection attempts from the host to the machine, once
# a second, for as long as they run. With LOG, they start only once the
# machine reports that it sealed itself again after being unsealed: probes
# answered while it is open would be indistinguishable in the capture from
# frames sent just after sealing.
start_probes() {
    (
        if [ -n "${1:-}" ]; then
            until [ -f "$1" ] && tr -d '\r' <"$1" |
                sed -n '/^NOTT-MINI: seal: off$/,$p' | grep -q -x 'NOTT-MINI: seal: on'; do
                sleep 0.2
            done
        fi
        while :; do
            busybox nc -w 1 127.0.0.1 "$PROBE_PORT" </dev/null >/dev/null 2>&1 || true
            sleep 1
        done
    ) &
    PROBES=$!
}
stop_probes() {
    kill "$PROBES" 2>/dev/null || true
    wait "$PROBES" 2>/dev/null || true
}
# watch_memory LOG RAM OUT: at each "evict: POINT" line the machine logs,
# counts the lines of its RAM that contain the key and the content, and
# appends "POINT key=N content=N" to OUT. Runs until stopped.
watch_memory() {
    (
        for point in unlocked busy locked; do
            until [ -f "$1" ] && tr -d '\r' <"$1" | grep -q -x "NOTT-MINI: evict: $point"; do
                sleep 0.2
            done
            key=$(LC_ALL=C grep -c -a -F "$EVICT_KEY" "$2" || true)
            content=$(LC_ALL=C grep -c -a -F "$EVICT_CONTENT" "$2" || true)
            echo "$point key=$key content=$content" >>"$3"
        done
    ) &
    WATCHER=$!
}
# found OUT POINT WHAT: the count recorded for WHAT at POINT.
found() {
    sed -n "s/^$2 .*$3=\([0-9]*\).*/\1/p" "$1"
}
# capture_check NAME ARGUMENT...: runs the capture checker on $RUN/NAME.pcap.
capture_check() {
    name=$1
    shift
    "$TOOLS" capture check "$RUN/$name.pcap" --device-mac "$GUEST_MAC" "$@" \
        >"$RUN/$name-capture.txt" 2>&1
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
        -drive "file=$RUN/scratch.img,if=virtio,format=raw" \
        -drive "file=$MINI/meta.img,if=virtio,format=raw,readonly=on" \
        -drive "file=$MINI/attest.img,if=virtio,format=raw,readonly=on" \
        -drive "file=$MINI/attest.verity,if=virtio,format=raw,readonly=on"
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
# logged LOG TEXT: some line of the log contains TEXT.
logged() {
    if grep -q -F "$2" "$1.txt"; then pass "logged: $2"; else fail "not logged: $2"; fi
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
    grep -o -b -a -F "$2" "$1" | sed -n '1s/:.*//p'
}

rm -rf "$RUN"
mkdir -p "$RUN"
echo "mini: kernel $(cat "$MINI/kernel-version"), accelerator $ACCEL"

# --- T1: verified boot to a shell ------------------------------------------
echo "T1 boot from an untampered system image"
fresh_disks
boot "$RUN/t1.log" "$MINI/system.img" "$MINI/meta.img" "nott.test=boot"
expect "$RUN/t1.log" "shell ready"
expect "$RUN/t1.log" "seal: on"
logged "$RUN/t1.log" "ok   signature by key $(sha256sum <"$MINI/keys/release.public" | cut -d' ' -f1)"
expect "$RUN/t1.log" "noncritical: read ok sha256=$(cat "$MINI/noncritical.sha256")"

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
boot "$RUN/t2.log" "$RUN/system-critical.img" "$MINI/meta.img" "nott.test=boot"
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
boot "$RUN/t3.log" "$RUN/system-noncritical.img" "$MINI/meta.img" "nott.test=boot"
expect "$RUN/t3.log" "shell ready"
expect "$RUN/t3.log" "noncritical: read error"
if [ "$(count "$RUN/t3.log.txt" 'noncritical: read ok')" -eq 0 ]; then
    pass "tampered data was never returned"
else
    fail "tampered data was returned"
fi

# --- T4: boot metadata that the trusted key did not sign -------------------
echo "T4a boot metadata signed by a key the boot image does not trust"
fresh_disks
boot "$RUN/t4a.log" "$MINI/system.img" "$MINI/meta-otherkey.img" "nott.test=boot"
refuse "$RUN/t4a.log" "shell ready"
expect "$RUN/t4a.log" "FAIL stage1: boot metadata is not authentic"
logged "$RUN/t4a.log" "signature was made by a different key"

echo "T4b manifest changed after it was signed"
fresh_disks
cp "$MINI/meta.img" "$RUN/meta-edited.img"
# Turns build-id "MINI" into "LINI": still a well-formed manifest.
flip_byte "$RUN/meta-edited.img" $(($(offset_of "$RUN/meta-edited.img" "build-id: ") + 10))
boot "$RUN/t4b.log" "$MINI/system.img" "$RUN/meta-edited.img" "nott.test=boot"
refuse "$RUN/t4b.log" "shell ready"
expect "$RUN/t4b.log" "FAIL stage1: boot metadata is not authentic"
logged "$RUN/t4b.log" "signature is not valid for this key and message"

echo "T4c root hash replaced; manifest and signature untouched"
fresh_disks
cp "$MINI/meta.img" "$RUN/meta-roothash.img"
# The root hash is the third 64 KiB slot. Changing the lowest bit of a
# hexadecimal digit gives another hexadecimal digit.
flip_byte "$RUN/meta-roothash.img" 131072
boot "$RUN/t4c.log" "$MINI/system.img" "$RUN/meta-roothash.img" "nott.test=boot"
refuse "$RUN/t4c.log" "shell ready"
expect "$RUN/t4c.log" "FAIL stage1: boot metadata is not authentic"
logged "$RUN/t4c.log" "FAIL system.roothash: sha256"

echo "T4d integrity-folder root hash replaced"
fresh_disks
cp "$MINI/meta.img" "$RUN/meta-attest.img"
flip_byte "$RUN/meta-attest.img" 196608
boot "$RUN/t4d.log" "$MINI/system.img" "$RUN/meta-attest.img" "nott.test=boot"
refuse "$RUN/t4d.log" "shell ready"
expect "$RUN/t4d.log" "FAIL stage1: boot metadata is not authentic"
logged "$RUN/t4d.log" "FAIL attest.roothash: sha256"

# --- T7: the integrity folder ----------------------------------------------
echo "T7a integrity folder in the running system"
fresh_disks
boot "$RUN/t7a.log" "$MINI/system.img" "$MINI/meta.img" "nott.test=attest"
expect "$RUN/t7a.log" "attest: mounted read-only from the verified image"
for operation in "touch /nott/attest/new-file" \
    "sh -c echo extra >>/nott/attest/manifest" \
    "rm -f /nott/attest/manifest" \
    "mv -f /nott/attest/manifest /nott/attest/manifest.old" \
    "mkdir /nott/attest/dir" \
    "mount -o remount,rw /nott/attest" \
    "dd if=/dev/zero of=/dev/mapper/attest bs=512 count=1 conv=notrunc" \
    "dd if=/dev/zero of=/dev/vdf bs=512 count=1 conv=notrunc"; do
    expect "$RUN/t7a.log" "attest: refused: $operation"
done
if grep -q "attest: FAIL" "$RUN/t7a.log.txt"; then
    fail "an integrity-folder check failed: $(grep "attest: FAIL" "$RUN/t7a.log.txt" | head -n 1)"
else
    pass "no integrity-folder check failed"
fi
expect "$RUN/t7a.log" "attest: verify PASS"
logged "$RUN/t7a.log" "PASS: 2 of 3 listed items checked, all match"

echo "T7b integrity folder checked from outside, with NottOS tools and with OpenPGP"
external() {
    "$TOOLS" manifest verify "$1/manifest" --dir "$MINI" \
        --vb-key "$MINI/attest/release.public" \
        --public-key "$MINI/attest/release.public" --signature "$1/manifest.sig"
}
openpgp() {
    home=$(mktemp -d)
    GNUPGHOME="$home" gpg --quiet --batch --import "$MINI/attest/release.asc" 2>/dev/null
    GNUPGHOME="$home" gpg --batch --verify "$1/manifest.asc" "$1/manifest" 2>/dev/null
    status=$?
    GNUPGHOME="$home" gpgconf --kill gpg-agent 2>/dev/null || true
    rm -rf "$home"
    return "$status"
}
if external "$MINI/attest" >"$RUN/t7b-external.txt"; then
    pass "every image matches the signed manifest, including the boot image"
else
    fail "external verification failed (see $RUN/t7b-external.txt)"
fi
if openpgp "$MINI/attest"; then pass "OpenPGP signature verifies"; else fail "OpenPGP signature does not verify"; fi
# Controls: an edited copy of the folder fails both checks.
rm -rf "$RUN/attest-edited"
cp -r "$MINI/attest" "$RUN/attest-edited"
sed -i 's/^build-id: MINI$/build-id: EVIL/' "$RUN/attest-edited/manifest"
if external "$RUN/attest-edited" >/dev/null; then
    fail "control: an edited manifest passed the NottOS check"
else
    pass "control: an edited manifest fails the NottOS check"
fi
if openpgp "$RUN/attest-edited"; then
    fail "control: an edited manifest passed the OpenPGP check"
else
    pass "control: an edited manifest fails the OpenPGP check"
fi

echo "T7c one flipped bit in the integrity folder's manifest"
fresh_disks
cp "$MINI/attest.img" "$RUN/attest-damaged.img"
flip_byte "$RUN/attest-damaged.img" \
    $(($(offset_of "$RUN/attest-damaged.img" "build-id: MINI") + 10))
boot "$RUN/t7c.log" "$MINI/system.img" "$MINI/meta.img" "nott.test=attest" \
    "$RUN/attest-damaged.img"
expect "$RUN/t7c.log" "attest: verify FAIL"
refuse "$RUN/t7c.log" "attest: verify PASS"
if grep -q -i 'verity.*corrupt' "$RUN/t7c.log.txt"; then
    pass "kernel reported dm-verity corruption"
else
    fail "no dm-verity corruption report in the kernel log"
fi

# --- T5: encrypted storage --------------------------------------------------
echo "T5 encrypted storage"
CONTENT=NOTT-CANARY-CONTENT-7f3a9c1e5b2d8046
NAME=NOTT-CANARY-NAME-1c9e4b7a2f6d8053
DEVICE=NOTT-CANARY-DEVICE-55aa33cc0f1e2d3c
PLAIN=NOTT-CANARY-PLAIN-9b8a7c6d5e4f3021
CANARIES="nott.content=$CONTENT nott.name=$NAME nott.device=$DEVICE nott.plain=$PLAIN"
fresh_disks
boot "$RUN/t5-provision.log" "$MINI/system.img" "$MINI/meta.img" \
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

boot "$RUN/t5-locked.log" "$MINI/system.img" "$MINI/meta.img" \
    "nott.test=locked $CANARIES"
expect "$RUN/t5-locked.log" "locked: entries=1"
expect "$RUN/t5-locked.log" "locked: name hidden"
expect "$RUN/t5-locked.log" "locked: read denied"
expect "$RUN/t5-locked.log" "locked: device file readable"

boot "$RUN/t5-wrongpass.log" "$MINI/system.img" "$MINI/meta.img" \
    "nott.test=wrongpass nott.pass=wrong-horse $CANARIES"
expect "$RUN/t5-wrongpass.log" "wrongpass: read denied"

boot "$RUN/t5-unlock.log" "$MINI/system.img" "$MINI/meta.img" \
    "nott.test=unlock nott.pass=correct-horse $CANARIES"
expect "$RUN/t5-unlock.log" "unlock: content ok"

# --- T8: network seal (INV-05, INV-08) -------------------------------------
echo "T8a sealed from the first instant of boot; never unsealed"
fresh_disks
start_probes
CAPTURE="$RUN/t8a.pcap"
boot "$RUN/t8a.log" "$MINI/system.img" "$MINI/meta.img" "nott.test=sealed"
CAPTURE=
stop_probes
expect "$RUN/t8a.log" "seal: on, 1 interface(s) at the device layer"
for result in "dhcp no lease" "arp no reply" "ping no reply" "dns no answer" \
    "tcp failed" "ping6 no reply" "no inbound data"; do
    expect "$RUN/t8a.log" "sealed: $result"
done
for layer in ip-output device-egress device-ingress; do
    expect "$RUN/t8a.log" "seal counted drops at $layer"
done
if capture_check t8a --min-received 1; then
    pass "capture: no frame sent from the first instant on, probes received"
else
    fail "capture: see $RUN/t8a-capture.txt"
fi

echo "T8b unlocked, used, locked again"
fresh_disks
start_probes "$RUN/t8b.log"
CAPTURE="$RUN/t8b.pcap"
boot "$RUN/t8b.log" "$MINI/system.img" "$MINI/meta.img" "nott.test=lockcycle"
CAPTURE=
stop_probes
expect "$RUN/t8b.log" "seal: off"
expect "$RUN/t8b.log" "open: dhcp lease"
expect "$RUN/t8b.log" "open: arp reply"
expect "$RUN/t8b.log" "seal: on"
for result in "dhcp no lease" "arp no reply" "ping no reply" "dns no answer" \
    "tcp failed"; do
    expect "$RUN/t8b.log" "locked: $result"
done
# Every kind the open phase sent must be in the capture: the rig sees them.
if capture_check t8b --seal-marker open-end --expect-sent arp \
    --expect-sent dhcp --expect-sent icmp --expect-sent dns --expect-sent tcp \
    --min-received 1; then
    pass "capture: frames of every kind while open, none after locking"
else
    fail "capture: see $RUN/t8b-capture.txt"
fi
# Control: the same capture judged as sealed from the start must fail.
if "$TOOLS" capture check "$RUN/t8b.pcap" --device-mac "$GUEST_MAC" >/dev/null 2>&1; then
    fail "control: the checker passed a capture with frames sent"
else
    pass "control: the checker flags frames sent while sealed"
fi

echo "T8c no packet filter, no boot"
fresh_disks
CAPTURE="$RUN/t8c.pcap"
boot "$RUN/t8c.log" "$MINI/system.img" "$MINI/meta.img" \
    "nott.test=sealed module_blacklist=ip_tables"
CAPTURE=
expect "$RUN/t8c.log" "FAIL stage1: insmod ip_tables.ko"
refuse "$RUN/t8c.log" "seal: on"
refuse "$RUN/t8c.log" "sealed: dhcp no lease"
if capture_check t8c; then
    pass "capture: no frame sent"
else
    fail "capture: see $RUN/t8c-capture.txt"
fi

# --- T9: evicting a storage class key at lock (INV-09) ---------------------
EVICT_SEEDS="nott.keyseed=nott-mini-key-$$ nott.canaryseed=nott-mini-content-$$"
EVICT_KEY=$(printf '%s' "nott-mini-key-$$" | sha512sum | cut -c1-64)
EVICT_CONTENT=$(printf '%s' "nott-mini-content-$$" | sha256sum | cut -c1-64)
# evict_boot NAME KERNEL_ARGUMENTS: one eviction boot, with memory searched
# from outside at each point.
evict_boot() {
    fresh_disks
    : >"$RUN/$1.scan"
    watch_memory "$RUN/$1.log" "$RUN/$1.ram" "$RUN/$1.scan"
    RAMFILE="$RUN/$1.ram"
    # This boot waits at three points; give it a minute more.
    saved_timeout=$TIMEOUT
    TIMEOUT=$((TIMEOUT + 60))
    boot "$RUN/$1.log" "$MINI/system.img" "$MINI/meta.img" \
        "nott.test=evict $EVICT_SEEDS $2"
    TIMEOUT=$saved_timeout
    RAMFILE=
    kill "$WATCHER" 2>/dev/null || true
    wait "$WATCHER" 2>/dev/null || true
    rm -f "$RUN/$1.ram"
    sed 's/^/        memory: /' "$RUN/$1.scan"
}

echo "T9a key removed at lock, memory wiped on free (init_on_free=1)"
evict_boot t9a init_on_free=1
logged "$RUN/t9a.log" "evict: status Present"
expect "$RUN/t9a.log" "evict: status Incompletely removed"
expect "$RUN/t9a.log" "evict: status Absent"
expect "$RUN/t9a.log" "evict: content inaccessible"
# Controls: while unlocked, the search finds the key and the content.
if [ "$(found "$RUN/t9a.scan" unlocked key)" -gt 0 ] 2>/dev/null &&
    [ "$(found "$RUN/t9a.scan" unlocked content)" -gt 0 ] 2>/dev/null; then
    pass "control: while unlocked, memory holds the key and the content"
else
    fail "control: the memory search found nothing while unlocked"
fi
# A file still open keeps its content in memory: why seald stops holders.
if [ "$(found "$RUN/t9a.scan" busy content)" -gt 0 ] 2>/dev/null; then
    pass "a file still open at lock keeps its content in memory"
else
    fail "expected the open file's content in memory while removal was incomplete"
fi
if [ "$(found "$RUN/t9a.scan" locked key)" = 0 ] &&
    [ "$(found "$RUN/t9a.scan" locked content)" = 0 ]; then
    pass "after lock, neither the key nor the content is anywhere in memory"
else
    fail "after lock: $(grep '^locked' "$RUN/t9a.scan" || echo 'no search')"
fi

echo "T9b control: the same, with the kernel's default (no init_on_free)"
evict_boot t9b ""
expect "$RUN/t9b.log" "evict: status Absent"
# The kernel documents that removing a key frees decrypted contents without
# wiping them. If this boot left nothing either, T9a would not show that
# init_on_free=1 is what clears memory.
if [ "$(found "$RUN/t9b.scan" locked content)" -gt 0 ] 2>/dev/null; then
    pass "control: without init_on_free=1 the content survives in freed memory"
else
    fail "control: expected the content in freed memory without init_on_free=1"
fi

echo
if [ "$FAILED" -eq 0 ]; then
    echo "mini: PASS (logs in $RUN)"
else
    echo "mini: FAIL, $FAILED check(s) failed (logs in $RUN)"
    exit 1
fi
