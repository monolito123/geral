# ADR-0014: Network seal: layers, loading order, and how it is tested

- **Status:** Accepted (2026-11-14; decided under the owner's delegation)
- **Date:** 2026-11-14
- **Decider:** project owner (delegated)

## Context

While the device is sealed, no packet may leave or enter it on any
interface but loopback (INV-05), and that must hold from the first instant
after a reset, before any interface is brought up (INV-08). The architecture
(section 6.2) puts the enforcement at a kernel packet hook that drops by
default from boot: built into the kernel on Tier A, loaded by the first
NottOS-controlled init stage on Tier B. Phase 4 criteria 4.1 and 4.2 ask
for zero frames, measured from outside, over a matrix of scenarios.

This ADR decides the mechanism and the test method, and proves both on the
minimal Linux test target (ADR-0007). It does not decide how `seald`
controls the seal, how unsealing is bound to authentication, or what
happens to radios, USB and power. Those belong to the locked-state
enforcement ADR of Phase 4.

The Android generic kernel image (GKI) has iptables, ip6tables, arptables,
tc with a match-all classifier and a drop action, and BPF. It does **not**
have nftables `[V:F-38]`. A seal must use what that kernel offers, or its
evidence does not carry over to phones.

## Options considered

### Option A — nftables

One atomic transaction for every layer; the `netdev` family's egress hook
also sees frames that programs write to raw packet sockets. Built first and
tested: on the test target the sealed boot (T8a) passed with it. **Rejected**
because GKI has no nftables: the evidence would be about a mechanism no
phone kernel offers.

### Option B — BPF programs, as Android's own network daemon uses

Flexible, and present in GKI. It needs a BPF build and a loader, and the
per-application cgroup filters do not see packets the kernel generates
itself or frames written below IP `[K]`. **Deferred** to Phase 4, where
`seald` may need it for the Allowlist level.

### Option C — tc match-all drop, iptables and ip6tables, arptables

Three layers, all present in GKI. No single transaction, so the order of
application has to guarantee that there is never a gap. **Chosen.**

### Option D — keep the interfaces down

A process allowed to configure the network can bring an interface up, and
the kernel then speaks on it. Useful in addition for radios (the
architecture already powers them down and blocks them), not as the seal.

## Decision

### Layers, in the order they are applied

1. **Device layer.** On each interface but loopback, a `clsact` queueing
   discipline with a match-all filter whose action is drop, on ingress and
   on egress. This is the lowest layer software reaches. It also drops
   frames written to raw packet sockets, which bypass IP and ARP (the DHCP
   client and `arping` use them).
2. **IP layer.** The IPv4 and IPv6 filter tables, replaced in one step each:
   policy DROP on input, output and forwarding; only loopback accepted. This
   layer covers interfaces created after the seal.
3. **ARP layer.** The ARP filter table: policy DROP everywhere.

Each layer alone blocks what it covers, and the device layer goes first, so
each step only adds to what is already blocked. Unsealing removes the
layers in the reverse order.

### Loading order and failure

- The packet-filter modules load first and alone, before any other driver.
  The seal is applied at once. Only then do the other drivers load, and the
  seal is applied again, so that the device layer also covers any interface
  those drivers created. Re-applying is idempotent: a drop already in
  place stays.
- After applying it, the seal checks that every part is in place. If a
  module fails to load or any part is missing, the boot stops. Nothing that
  could bring an interface up has started by then.
- An interface created later is covered by the IP and ARP layers at once.
  The device layer covers it only once the seal is applied again. On
  Android, `seald` must do that on every new interface (Phase 4).

### Test method

- **Measured from outside.** The emulator records every frame on the
  virtual link, in both directions, from before the machine starts. A
  frame whose Ethernet source address is the machine's was sent by it.
  `nott-tools capture check` reads the capture.
- **Sealed throughout** (T8a): with IPv6 on, the machine tries DHCP, ARP,
  ICMP, DNS, TCP and IPv6 while the host tries to connect in. Every attempt
  must fail, the seal's counters must show drops at the IP layer and at the
  device layer in both directions, and the capture must hold no frame sent
  by the machine and at least one sent to it.
- **Open, then sealed** (T8b): while open, the machine sends every kind of
  frame, and the capture must hold each kind. This positive control proves
  the rig sees them. The last frame before sealing carries a marker name.
  No frame may follow it. The host starts its connection attempts only
  once the machine reports that it sealed itself again.
- **Fail closed** (T8c): with one packet-filter module blocked, the boot
  stops and no frame is sent.
- **The checker can fail**: given T8b's capture as if the machine had been
  sealed throughout, it must report the frames.

The first version of T8b let the host probe during the open phase as well.
The capture then held a reset the machine sent about a second after the
marker, and nothing could tell whether it went out just before or just
after sealing. The test was ambiguous, so it was changed; it says nothing
about the seal either way.

## Consequences

- **Positive:** the mechanism exists in every GKI kernel, so this evidence
  applies to the Tier B design; the test method (outside capture, positive
  controls, a checker that can fail) carries over to Phase 4 on the
  emulator.
- **Negative:** no single transaction across layers; the ordering argument
  replaces it. The boot image carries `tc` and the legacy iptables and
  arptables programs.
- **Risks:**
  - *Raw packet sockets that bypass the queueing layer*
    (`PACKET_QDISC_BYPASS`) skip tc egress `[K]`; only nftables' egress hook
    would see them, and GKI lacks it. On Android, raw packet sockets need a
    capability that SELinux grants to a few system daemons `[K]`. Phase 4
    must close this or show it closed. UNVERIFIED.
  - *Another program with network privileges removes the seal.* On the
    test target, root can. On Android, SELinux must allow these changes to
    `seald` alone. Phase 4.
  - *A new interface before re-application* (above).
  - Sockets open at the moment of sealing are not yet destroyed (criterion
    4.9; the kernels have the facility, `INET_DIAG_DESTROY` `[V:F-38]`).

## Revisit when

- GKI gains nftables, or NottOS builds its own kernel for Tier A (then a
  kernel-built-in hook can cover every interface from boot).
- Phase 4 designs `seald` and the Allowlist level.

## Verification

- `tests/mini`: T8a, T8b, T8c and the checker control; T1 shows the seal is
  on in every boot.
- `tools/nott-tools/src/capture.rs`: unit tests of the capture reader
  (both byte orders, cut-off captures, other link types) and of the
  verdicts (sealed throughout, sealed after a marker, missing control
  kinds, too few received frames).
