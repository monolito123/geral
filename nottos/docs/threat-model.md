# NottOS threat model

- **Version:** 0.1 (Phase 0 draft, awaiting owner approval)
- **Date:** 2026-11-12
- **Depends on:** [ADR-0001](adr/0001-base-architecture.md) (hardened AOSP on
  Linux), [ADR-0002](adr/0002-device-support-tiers.md) (device tiers)

Nothing in this document has been implemented or tested. Mitigations are
design intent. Evidence tags are explained in the [README](../README.md).

## 1. What is protected

| Asset | Why it matters |
|---|---|
| **User data at rest** — files, messages, photos, app data, credentials stored by apps | Primary asset. Must stay confidential against anyone without the owner's credential, including with full physical possession. |
| **Keys** — disk-encryption keys, profile keys, keystore keys, backup keys | Compromise of a key is compromise of everything under it. |
| **The owner's credential** — PIN or passphrase, biometrics | Gate to every key. |
| **System integrity** — the code that runs, from bootloader hand-off to apps | If the system is modified, every other guarantee is void. |
| **Data in use and in transit** — what apps display, send and receive | Confidentiality and integrity against the network and other apps. |
| **Metadata** — location, identifiers, contact graph, which hosts are contacted, when the device is active | Enables tracking even when content is protected. |
| **Availability of safety functions** — emergency calls, thermal and battery protection | Legal and safety obligation; never traded for security. |
| **Owner control** — ability to locate, lock and wipe; ability to verify the system | The owner, not the holder, controls the device. |

## 2. Device states

Guarantees differ by state, and attackers choose the state they attack in.

| State | Meaning | Keys in RAM |
|---|---|---|
| **Off** | Powered down | None |
| **BFU** (Before First Unlock) | Booted, credential not yet entered since boot | Only device-bound keys needed to reach the lock screen. No user data keys. |
| **AFU, unlocked** | In use | All keys of the active profile |
| **AFU, locked** | Locked after having been unlocked | Keys of data that must stay available while locked. Evict-on-lock classes are gone. |

Boot states follow Android Verified Boot: locked with the NottOS key (the
"yellow" state, which is NottOS's normal state on Tier 1 and 2), unlocked
("orange", Tier 3 or development), verification failed ("red", does not boot)
`[K]`. Owner-enabled root is a distinct, visible boot state defined in a later
ADR.

## 3. Trust assumptions

NottOS **trusts**, because it has no alternative:

- The SoC boot ROM and the hardware root of trust.
- The vendor bootloader chain up to the point where it verifies NottOS.
- The TEE OS and, where present, the secure element, for key storage,
  attestation and unlock throttling.
- The CPU's isolation mechanisms.

NottOS **does not trust**, and designs to contain:

- The network, including the mobile operator and any Wi-Fi access point.
- Apps, including Google Play services.
- Files from any source.
- USB peripherals and chargers.
- Baseband, Wi-Fi, Bluetooth, NFC, UWB and GNSS firmware.
- The SIM or eSIM profile.
- Vendor HAL processes (necessary, but confined).
- NottOS's own update servers and mirrors (integrity comes from signatures,
  not from the transport or the server).

How much the trusted parts deserve that trust depends on the device tier.
On Tier 3 the bootloader does not verify NottOS at all.

## 4. Adversaries

Each adversary is described by capability, goal, the mitigations planned, and
what remains. Mitigations name the mechanism; the mechanism's design is left
to its ADR.

### A1 — Remote network attacker

**Capability.** Sends arbitrary traffic from the internet; controls the local
network (rogue access point, compromised router); operates a rogue base
station; sends SMS, MMS, calls and push messages; is in radio range for
Bluetooth, NFC, UWB and Wi-Fi management frames.

**Goal.** Remote code execution, interception, tracking.

**Mitigations.**

- No listening network services by default. Every default outbound connection
  is documented and can be disabled (updates, time, connectivity check).
- **Sealed locked state**: while the device is locked, the network attack
  surface is closed (section 6). A phone spends most of its life locked.
- New parsers and network-facing daemons in Rust. Hardened allocator and,
  on Tier 1, MTE for the remaining C and C++.
- Each network-facing service in its own SELinux domain with a seccomp filter.
  Media decoding stays in sandboxed codec processes.
- Hardened browser and WebView with site isolation; JIT off by default.
- OS services authenticate servers with pinned keys. Updates and repository
  metadata are verified by signature, so a TLS compromise cannot inject code.
- Per-network MAC randomisation, no stable identifiers on the air, encrypted
  DNS, 2G disabled, optional 4G/5G-only mode, no auto-join of open networks,
  no automatic MMS retrieval.

**Residual risk.**

- Zero-day vulnerabilities in the kernel network stack and in vendor Wi-Fi and
  Bluetooth drivers while the device is unlocked or in Standard/Allowlist
  mode.
- Baseband compromise from the cellular side (continues as A7).
- Cellular-layer tracking by the operator or a rogue base station. An OS
  cannot prevent a registered modem from being located.
- Traffic analysis of encrypted traffic.

### A2 — Malicious app

**Capability.** Runs arbitrary code in the app sandbox, possibly with
permissions the user granted. May collude with other apps, load code
dynamically, probe every reachable kernel and Binder interface, fingerprint
the device, and deceive the user through the UI.

**Goal.** Escape the sandbox, read other apps' data, track the user,
exfiltrate data beyond what was granted, persist.

**Mitigations.**

- Per-app UID, SELinux domain with per-app categories, and seccomp filter.
  SELinux ioctl allowlists on device nodes an app can reach, GPU included
  where the driver permits.
- Exec-based spawning: no address-space layout shared between apps.
- Hardened allocator and MTE (Tier 1) for native app code, with a per-app
  compatibility switch.
- Network and Sensors permissions; Storage Scopes and Contact Scopes; global
  camera and microphone switches; clipboard, background, location-precision,
  dynamic-code-loading and native-debugging controls per app.
- No hardware identifiers and no advertising ID exposed to apps.
- Google Play services run as ordinary sandboxed apps.
- Signature verification with key pinning at install; fs-verity on installed
  code so that tampering with app code at rest is detected.
- Profiles with separate keys; private space; end-session evicts keys.
- Connection log and per-app firewall.

**Residual risk.**

- **Kernel privilege escalation through a vendor driver** (GPU first). This
  is the most realistic sandbox escape and the code is not ours. Reduced by
  ioctl filtering, MTE in the kernel on Tier 1 and fast patching; not
  eliminated.
- Abuse of permissions the user really granted.
- Side channels and covert channels between colluding apps.
- UI deception (phishing inside the app's own window).

### A3 — Malicious file

**Capability.** Delivers a crafted document, image, video, audio file, font or
archive through any channel. No code execution to begin with.

**Goal.** Code execution in whatever process parses the file; then A2.

**Mitigations.**

- Nott Reader parses every document in a separate, disposable process: no
  network, no filesystem beyond the one input, minimal syscall filter, memory
  and CPU limits. The UI receives rendered output only.
- No macros, embedded scripts or JavaScript are executed. No remote content is
  fetched. Embedded files are treated as new untrusted input.
- Parsers are Rust, or well-fuzzed libraries inside the sandbox. Continuous
  fuzzing of every parser.
- Thumbnails and previews are generated only in such sandboxes, never in a
  privileged process.
- Software media codecs stay in the upstream sandboxed codec service.

**Residual risk.**

- Hardware video codecs parse untrusted bitstreams in vendor code and
  firmware.
- Logic flaws that mislead rather than exploit: content that renders
  differently from what was signed, or spoofed signature status.
- Resource exhaustion (decompression bombs), bounded by limits but still a
  denial of service for that document.
- Files opened in third-party apps are only as safe as that app's sandbox.

### A4 — Malicious USB peripheral or charger

**Capability.** Acts as a USB host (computer, public charger with data) or as
a device (keyboard that types, storage, network adapter that becomes the
default route, hub). Sends crafted USB Power Delivery messages. Uses
alternate modes and debug-accessory signalling.

**Goal.** Exploit the USB stack, inject input, extract data, intercept
traffic, reach debug interfaces.

**Mitigations.**

- **Locked: data lines off.** At the port controller on Tier 1 and where the
  USB HAL supports it; in the kernel everywhere. No enumeration in either
  role, no alternate modes, no accessory mode. Charging continues.
- Accessories connected before locking follow a user setting; default is
  disconnect on lock.
- Unlocked: a new peripheral is accepted only after a prompt naming its
  class. Input devices and network adapters always prompt.
- ADB off by default, available only after unlock and explicit
  authentication, never while locked. Debug-accessory modes disabled in
  production builds.
- Fastboot, recovery and download modes never expose user data. Unlocking the
  bootloader is refused unless enabled from the authenticated OS, and wipes
  data.

**Residual risk.**

- Bugs in the port controller's Power Delivery firmware, which must stay
  active to charge.
- The vendor bootloader's own USB stack in fastboot or download mode. NottOS
  cannot harden it. Data stays encrypted and, in that state, keys are not in
  RAM.
- On Tier 2 and 3 without controller-level disable, a kernel USB bug reachable
  before the software block applies.

### A5 — Thief or forensic tool with a locked device

**Capability.** Unlimited physical possession. Commercial forensic toolkits
with exploit chains over USB, radios and the lock screen. Credential guessing.
Hardware techniques: flash chip removal and imaging, memory extraction, debug
ports, fault injection. Can isolate the device from the network, remove the
SIM, force a hardware reset, or drain the battery. May have observed the PIN
before taking the phone.

**Goal.** User data; resale after wiping; preventing the owner from locating
the device.

**Mitigations.**

- Sealed locked state closes USB and network surfaces in AFU.
- **Auto-reboot** after a configurable locked period returns the device to
  BFU with all keys evicted. Evict-on-lock key classes leave RAM at lock.
- Unlock attempts are throttled by the secure element (Tier 1), not by the
  OS. Key derivation combines a memory-hard KDF over the credential with a
  hardware-bound secret, so guessing requires this device's hardware.
- Metadata encryption on top of file-based encryption: a flash image reveals
  no file names, sizes or directory structure.
- Memory is zeroed on free in kernel and userspace; firmware-level memory
  clearing on reset where the device provides it.
- Shutdown, reboot, airplane mode and radio toggles require authentication
  while locked. A hardware forced reset boots back into NottOS in BFU with
  Sealed active from early boot.
- Minimal lock-screen surface: no content previews by default, PIN scrambling
  option, second factor option (fingerprint plus PIN), duress PIN that
  destroys keys.
- Opt-in owner-authenticated locate, lock and wipe.

**Residual risk.**

- **AFU exploitation before the auto-reboot fires**, through what must stay
  reachable: the touchscreen and lock-screen UI, the charging controller, and
  the baseband if voice is allowed while locked.
- **A short PIN is only as strong as the throttling hardware.** On Tier 2 and
  3 without a secure element, a compromised TEE or OS turns a 6-digit PIN
  into a quick search. Those tiers should require a passphrase.
- Invasive attacks on the TEE or secure element (out of scope, section 5).
- A thief who knows the PIN. Second factor, a separate private-space
  credential and delays on sensitive setting changes limit the damage; they
  do not remove it.
- Denial of service: destroying the device, draining it, or shielding it
  defeats location. No OS can prevent this.

### A6 — Evil maid

**Capability.** Temporary, unobserved physical access; the device returns to
its owner. Can flash partitions through the bootloader or a low-level
download mode, downgrade the OS, replace the device with a look-alike that
captures the credential, add a hardware implant, or alter encrypted storage
blocks.

**Goal.** Persistent compromise or credential capture without the owner
noticing.

**Mitigations.**

- Verified boot from the hardware root of trust with NottOS keys on a locked
  bootloader; dm-verity on all system partitions; rollback counters.
- The verified-boot key fingerprint is shown at boot.
- **Verification from outside the device:** hardware attestation checked by a
  second device that pinned this device's attestation key on first pairing.
  This detects OS replacement, downgrade and device substitution.
- Reproducible builds and published hashes, so that "genuine" has a public
  definition.
- The integrity folder and Verify app for user-visible self-checks.
- Unlocking the bootloader requires enabling it from the authenticated OS and
  wipes user data.
- Tiering excludes devices whose boot ROM or download mode can bypass
  verified boot (ADR-0002).

**Residual risk.**

- Hardware implants and malicious replacement parts.
- Vulnerabilities in the vendor bootloader.
- **User-data blocks are encrypted but not authenticated.** AES-XTS does not
  detect modification or replay of old ciphertext. An evil maid can corrupt
  data or roll individual blocks back, though not choose plaintext.
  Installed code is covered separately by fs-verity.
- **A compromised system can lie about itself.** The Verify app and the
  integrity folder prove nothing against an attacker who controls the running
  OS. Only the boot-time fingerprint and external attestation do.
- The owner ignoring a changed boot screen.

### A7 — Compromised radio chip

**Capability.** Full control of baseband, Wi-Fi, Bluetooth, NFC, UWB or GNSS
firmware. Sends arbitrary data to the host over the chip's bus, attempts DMA,
sees and alters all traffic through that radio, transmits at will.

**Goal.** Pivot into the application processor; intercept; track.

**Mitigations.**

- Radios confined by the IOMMU so that a chip can reach only its own buffers
  (confirmed per device on Tier 1).
- Host-side drivers and HAL daemons treat radio input as hostile: confined
  domains, seccomp, fuzzing of the message parsers, Rust for new shims.
- End-to-end protection above the radio: TLS everywhere and optional
  always-on VPN, so a radio sees ciphertext.
- Sealed mode powers down Wi-Fi, Bluetooth, NFC and UWB while locked.
- 4G/5G-only mode removes legacy protocol stacks from reach.
- SIM toolkit actions that send messages or open channels are off by default.
- Hardware or firmware radio kill where the device has one.

**Residual risk.**

- **If voice or SMS is allowed while locked, the baseband stays registered**:
  the network knows the device's cell, and the baseband stays attackable from
  the network. This is inherent in being reachable.
- On SoCs with an integrated modem, the modem shares hardware with other
  subsystems, sometimes including the audio path. NottOS cannot verify the
  isolation between them. Whether a compromised baseband can reach the
  microphone depends on the SoC and is UNVERIFIED for every device until its
  audit.
- Vendor radio drivers in the kernel remain the weakest link on the host
  side.
- NottOS cannot attest radio firmware beyond what the SoC's own secure boot
  does.

### A8 — Compromised supply chain or NottOS infrastructure

Not in the owner's original list; added because the update and integrity
requirements imply it.

**Capability.** Controls an update server, mirror or CDN; steals a signing
key; compromises a build machine or a dependency; is a malicious insider.

**Goal.** Ship malicious code to all users or to one targeted user.

**Mitigations.**

- Root signing keys offline in hardware, with hybrid classical plus
  post-quantum signatures and a hash-based scheme for long-lived roots.
  Release signing requires more than one person.
- Update client enforces signature, monotonic version and anti-rollback
  regardless of what the server says. It sends no unique device identifier,
  so a server cannot target one device.
- Reproducible builds with independent rebuilders, and a public append-only
  log of release manifests, so that a release served to only one victim is
  detectable.
- Repository metadata is signed with expiry, which defeats freeze, rollback
  and mix-and-match of old metadata.
- Dependencies pinned and reviewed; no network access during builds.

**Residual risk.**

- Backdoors or vulnerabilities already present in upstream AOSP or in vendor
  binaries.
- A compromised compiler toolchain.
- Simultaneous compromise of enough signing parties.

### A9 — Coercion

The owner is forced to unlock. The duress PIN destroys the keys and is
indistinguishable from a failed unlock at the moment of entry. Beyond that,
coercion is out of scope: an OS cannot protect a user who is compelled to use
the real credential, and using a duress feature can have legal consequences
that the documentation must state.

## 5. Out of scope

| Excluded | Consequence if it happens |
|---|---|
| Compromised SoC boot ROM or hardware root of trust | Total compromise; nothing above it can be trusted |
| Malicious hardware inserted at manufacture | As above |
| Invasive physical attacks on the secure element or TEE (decapsulation, probing, advanced fault injection) | Throttling and hardware-bound secrets can be bypassed; protection falls back to credential entropy alone |
| Vulnerabilities inside the TEE OS or secure-element firmware | As above; NottOS can only ship vendor fixes |
| Electromagnetic and acoustic emanation attacks | Not addressed |
| An attacker using the device while it is unlocked and unattended | Limited by per-app locks and private space only |
| Malware the user installs and grants broad permissions to | Contained to what was granted; not prevented |
| Interception inside the mobile network (lawful or not) of calls and SMS | Use end-to-end encrypted apps |
| Physical destruction, battery drain, RF shielding | Availability only; data stays protected |

## 6. Cross-cutting design: who is allowed to say "unlocked"

Several mitigations depend on the locked state being enforced below the
application framework. The principle, to be detailed in the locked-state ADR:

- **Sealing is fail-closed and easy.** Any of several signals seals the
  device: the framework reporting lock, the display going off past a timeout,
  early boot. Sealing needs no proof.
- **Unsealing requires proof of authentication that the framework cannot
  forge.** A small Rust daemon in its own SELinux domain owns the kernel
  controls for network, USB and power policy. It unseals only after a
  keystore operation that the TEE permits solely with a fresh authentication
  token from the credential or biometric check. A compromised system_server
  can ask, but cannot produce that token.
- **Enforcement points are in the kernel**: packet filtering on every
  interface, loaded with a default-drop policy before any interface comes up;
  USB authorisation and port-controller state; the shutdown path, which
  honours protective requests (thermal, battery) unconditionally and
  user-initiated requests only when unsealed.
- **Exceptions are narrow and enumerated**: an emergency call opens only the
  path that call needs, for its duration; the opt-in lost-device service is
  one process talking to one pinned endpoint.

A compromised system_server while the device is *unlocked* already has the
user's data, so this design does not claim to help there. Its claim is
narrower: once sealed, nothing in userspace short of a kernel compromise
reopens the device without the owner's credential.

## 7. Mitigations by adversary

`●` primary defence, `○` contributes.

| Mechanism | A1 net | A2 app | A3 file | A4 USB | A5 thief | A6 maid | A7 radio | A8 supply |
|---|---|---|---|---|---|---|---|---|
| Verified boot, rollback protection | | ○ | | | ○ | ● | | ○ |
| External hardware attestation | | | | | | ● | | ○ |
| SELinux, no permissive domains | ○ | ● | ● | ○ | ○ | | ● | |
| seccomp per process class | ○ | ● | ● | | | | ○ | |
| Kernel hardening (CFI, SCS, PAC/BTI, MTE, zero-init, lockdown) | ● | ● | ○ | ● | ● | | ● | |
| Hardened allocator, exec spawning | ● | ● | ● | | | | ○ | |
| Rust for new parsers and daemons | ● | ○ | ● | ○ | | | ● | |
| Disposable parser sandbox | | | ● | | | | | |
| Sealed locked state: network | ● | | | | ● | | ● | |
| Sealed locked state: USB | | | | ● | ● | ○ | | |
| Authenticated power controls | | | | | ● | | | |
| Auto-reboot, key eviction | | | | ○ | ● | | | |
| Encryption (FBE, metadata), hardware-bound KDF | | ○ | | | ● | ○ | | |
| Secure-element throttling, duress PIN | | | | | ● | | | |
| IOMMU radio isolation | ○ | | | | | | ● | |
| Per-app permissions, scopes, firewall | | ● | | | | | | |
| Hybrid-signed updates, anti-rollback | ● | | | | | ● | | ● |
| Reproducible builds, transparency log | | | | | | ○ | | ● |

## 8. What holds on which tier

| Guarantee | Tier 1 | Tier 2 | Tier 3 |
|---|---|---|---|
| Modified system image does not boot | Yes | Yes | **No** |
| OS downgrade is refused | Yes | Yes | **No** |
| Apps and a second device can attest NottOS | Yes | Yes | **No** |
| Unlock guessing is limited by hardware the OS cannot override | Secure element | TEE (weaker) | TEE, unattested |
| Memory-corruption bugs trapped by hardware tagging | Yes | No | No |
| USB dead at the controller while locked | Yes | Where the HAL supports it | Kernel only |
| Radio confined by IOMMU | Audited | Unconfirmed | Unknown |
| Full kernel hardening profile | Yes | Depends on vendor modules | No |
| App sandbox, permissions, scopes, firewall | Yes | Yes | Yes |
| Sealed network policy while locked | Yes | Yes | Yes, but a modified OS could remove it |
| Encryption at rest with hardware-bound keys | Yes | Yes | Yes, but boot chain is not verified |

On Tier 3, everything that depends on the system being unmodified is only as
strong as physical custody of the device.

## 9. Testable invariants

Each invariant is a statement a test or audit can falsify. They become the
security test suite in later phases. None is implemented; all are UNVERIFIED.

| ID | Invariant |
|---|---|
| INV-01 | On a locked bootloader, an image whose verified-boot chain fails, or whose rollback index is below the stored one, does not boot. |
| INV-02 | Every mounted filesystem is one of: verity-protected read-only, tmpfs, or encrypted user data. Exceptions (small vendor state partitions the hardware requires) are enumerated per device, carry no executable content, and are mounted `nosuid,nodev,noexec`. |
| INV-03 | No SELinux domain is permissive. Enforcement cannot be turned off at runtime. No production domain has unconfined-style blanket rules. |
| INV-04 | Every process class has a seccomp filter. The list of classes and filters is generated and checked at build time. |
| INV-05 | Locked and Sealed: no packet is transmitted or accepted on any non-loopback interface, and Wi-Fi, Bluetooth, NFC and UWB do not transmit, except through an active enumerated exception. |
| INV-06 | Locked: the device does not enumerate as a USB device and does not enumerate attached peripherals. |
| INV-07 | Locked: no user-initiated shutdown, reboot, airplane-mode or radio toggle succeeds without authentication. |
| INV-08 | After any reset, the drop-all network policy is active before any network interface is brought up. |
| INV-09 | After lock, keys of evict-on-lock classes are absent from kernel and process memory. After reboot, all user keys are absent. |
| INV-10 | The number of credential guesses is bounded by the secure element or TEE irrespective of OS state. |
| INV-11 | No system component maps memory writable and executable, or executes code that is not on a verified partition. |
| INV-12 | The debug bridge is not running unless enabled after authentication, and does not accept connections while locked. |
| INV-13 | A freshly installed system makes no network connection outside the documented list. |
| INV-14 | No production domain can write, rename or delete anything in the integrity folder. |
| INV-15 | An update is installed only if both the classical and the post-quantum signature verify and its version is greater than the installed one. A failed boot reverts to the previous slot. |
| INV-16 | No third-party app can read a hardware identifier. |
| INV-17 | An app whose Network permission is revoked cannot cause any packet to leave the device, directly or through a system service. |
| INV-18 | Thermal and battery protective shutdowns, and emergency calls, work in every lock state and policy level. |
| INV-19 | A document-parser process has no network access, no filesystem access beyond its input, and only the syscalls on its allowlist. |
| INV-20 | Enabling root changes the boot state, shows a persistent indicator, and is reflected in attestation. |

## 10. Limits stated plainly

1. A hardware forced reset and battery exhaustion cannot be blocked by
   software. The design makes them useless for data access, not impossible.
2. If cellular voice is allowed while locked, the device is locatable by the
   network and the baseband is exposed.
3. A system that is already compromised can report that it is intact.
4. The Linux kernel and vendor drivers are a single point of failure for the
   app sandbox.
5. Closed vendor firmware is trusted without the ability to inspect it.
6. Post-quantum protection of the boot chain depends on the bootloader, which
   NottOS does not control on existing devices (see
   [requirements-review.md](requirements-review.md), C-04).
7. Stored user data is confidential but not tamper-evident at the block
   level.
