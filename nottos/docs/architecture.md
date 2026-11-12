# NottOS high-level architecture

- **Version:** 0.1 (Phase 0 draft, awaiting owner approval)
- **Date:** 2026-11-12
- **Depends on:** [ADR-0001](adr/0001-base-architecture.md),
  [ADR-0002](adr/0002-device-support-tiers.md),
  [threat-model.md](threat-model.md)

This is design intent. No component described here exists yet. Component
names are working names. Evidence tags are explained in the
[README](../README.md).

## 1. Principles

1. **Policy is enforced low; the framework only asks.** Lock-state, network,
   USB and power policy are enforced in the kernel, the TEE and small
   daemons. system_server presents UI and makes requests.
2. **Fail closed.** The default at boot, after a crash and after a reset is
   the most restrictive state.
3. **Device-specific code is quarantined** behind stable interfaces and
   declarative descriptors (section 3).
4. **New code is small, Rust, and narrowly privileged.** One job per
   component, one SELinux domain each.
5. **Stay close to upstream.** Every change to AOSP is a recurring merge
   cost and a possible CTS deviation, so new behaviour goes into separate
   components where it can.
6. **Verifiable from outside.** Anything the system says about itself can be
   checked by something that is not the system.

## 2. Components

### 2.1 Layers

```
APPS -- each app: own UID, SELinux domain, seccomp filter, fresh exec
  user apps
  sandboxed Google Play (optional, user-installed, no privilege)
  NottOS system apps (section 2.3)
  Nott Reader UI ---> disposable parser sandboxes
                      (no network, no files, minimal syscalls)

ANDROID FRAMEWORK -- AOSP plus NottOS patches
  system_server: activities, packages, permissions, lock-screen UI
  NottOS patches: Network and Sensors permissions, Storage and
    Contact Scopes, per-app controls, exec-based app spawning
  ART (ahead-of-time compiled; no JIT in system components)
  Bionic with hardened allocator

NATIVE SERVICES
  AOSP: init, vold, keystore2, netd, update_engine, SurfaceFlinger,
        media codecs (sandboxed)
  NottOS, in Rust (section 2.2): seald, updated, verifyd, ramd,
        beacond (opt-in)
  Vendor HALs -- closed, confined, listed per device:
        radio, Wi-Fi, Bluetooth, NFC, GNSS, camera, graphics, audio,
        USB, KeyMint, Weaver, health, thermal

KERNEL -- Android Common Kernel (GKI) plus vendor modules
  access control: SELinux, seccomp
  storage: dm-verity, fscrypt, metadata encryption
  locked-state hooks: packet filter, USB authorisation, power gate
  hardening: CFI, shadow call stack, PAC/BTI, MTE, zero-init,
             lockdown, no module loading after boot
  IOMMU confinement of radios and other peripherals

FIRMWARE AND HARDWARE -- not controlled by NottOS
  boot ROM, bootloader (verified boot), TEE OS, secure element
  baseband, Wi-Fi/Bluetooth/NFC/UWB chips
  USB-C port controller, power-management chip
```

### 2.2 NottOS components

| Working name | Kind | Responsibility | Privilege | Phase |
|---|---|---|---|---|
| `seald` | Rust daemon | Lock-state machine. Sole owner of the kernel and HAL controls for network seal, USB data and power requests. Unseals only on TEE-backed proof of authentication (section 6). | Own domain; no network; no access to user data | 4 |
| Kernel seal hook | Small kernel patch (C, GPL-2.0) | Drops all non-loopback packets by default from boot until `seald` opens it. | Kernel | 4 |
| Key management extensions | Patches to vold and keystore2 (Rust and C++), possibly a helper daemon | Evict-on-lock key classes, memory-hard credential stretching, hybrid key wrapping for backup and transfer. | As vold/keystore2 | 2 |
| `updated` | Rust daemon | Fetches signed update metadata, verifies the hybrid signature, version and transparency proof, then hands the payload to update_engine. | Own domain; network to update servers only | 2 |
| `verifyd` and the Verify system app | Rust service plus app | Recomputes partition hashes and checks them against the signed manifest in the integrity folder. | Read-only access to system block devices; no network | 2 |
| App spawner | Patch to the AOSP spawn path | Starts every app with a fresh `exec`; keeps a small pool of freshly exec'd spare processes. | As upstream zygote | 3 |
| `ramd` | Rust daemon | Preloads and pins the hot set within the device's RAM budget; implements Full RAM mode. | May lock memory; no network; no user data | 6 |
| `beacond` | Rust daemon, opt-in | Lost-device client: outbound polling of one pinned endpoint, owner-signed commands only. | Own domain; the only UID allowed through the seal | 5 |
| Reader parser sandboxes | Isolated processes, Rust parsers | One disposable process per document. Proposed baseline: Android isolated-process services with an additional self-applied syscall filter, so the Reader needs no privilege beyond an ordinary app's. Final design in the Reader ADR. | Less than an app | 5 |
| `nott-crypto`, `nott-format` | Rust libraries | Algorithm-suite registry and versioned containers. Compose vetted primitives; implement none. | Library | 1–2 |
| `tiercheck` | Rust host tool plus on-device probe | Reports the tier a device can reach and the state of each mechanism. | Host | 1 onward |
| `nott` build entry point, signing and reproducibility tools | Host tools | One-command build, release signing, bit-for-bit comparison, SBOM. | Host | 1 |

### 2.3 System apps and their privileges

The brief asks that system apps be "sandboxed like any third-party app". That
holds for most of them. A few cannot do their job without privileges no
third-party app can hold (requirements review, C-18). The intended split:

| No privilege beyond a third-party app | Needs platform privilege (each one listed and justified in its ADR) |
|---|---|
| Calculator, clock, calendar, notes, audio recorder, music and video player, gallery, file manager, camera, keyboard (no network permission), Nott Reader, browser | Phone and dialer (telephony, emergency calling); SMS/MMS default handler; contacts provider; settings; system updater; backup and restore; Verify system; setup wizard; repository client (package installation) |

The browser and the system WebView are Chromium-based. They are the
exception to "small": they are the largest and most frequently patched
components in the system.

### 2.4 Outside the device

| Component | Purpose |
|---|---|
| Build infrastructure | Reproducible builds from pinned sources and toolchains |
| Offline root keys and release signing | Hybrid signatures; documented key ceremony |
| Release transparency log | Append-only public record of every release manifest |
| Update servers and mirrors | Untrusted transport for signed updates |
| App repository | Signed, expiring metadata; untrusted transport |
| Independent rebuilders | Confirm published hashes |
| Auditor app on a second device | Checks this device's hardware attestation |
| Lost-device relay (opt-in) | Carries owner-signed commands; learns as little as possible |
| Time, connectivity-check and assistance-data endpoints | Section 7 |

Who operates these services is an open question for the owner
(requirements review, Q3 and Q6).

## 3. Hardware abstraction

Requirement: all device-specific code is isolated.

**Three layers.**

1. **Generic NottOS system image**, one per CPU architecture (arm64, x86_64;
   riscv64 prepared). It contains no device-specific code and is built
   page-size agnostic so that it runs on 4 KB and 16 KB kernels `[K]`.
2. **Vendor side**, supplied by the device maker: kernel modules, HAL
   binaries, firmware. Used unmodified on Tier B; rebuilt where possible on
   Tier A.
3. **Device directory** in the NottOS tree, one per device, containing
   declarations and no logic:
   - partition layout, fstab, HAL compatibility matrix;
   - a **capability descriptor**: which mechanisms the device has and which
     control implements each (for example, which HAL call or kernel control
     disables USB data, which control powers down each radio, whether a
     secure element is present);
   - the vendor binary manifest (ADR-0002, rule 6);
   - SELinux additions and syscall filters for vendor HALs;
   - kernel configuration fragment and pinned-set tuning.

**Interfaces between generic and device code** are only the stable ones AOSP
already defines (versioned HAL interfaces, the kernel's user-space ABI, the
boot control interface) plus the capability descriptor. Generic code never
branches on a device name; it reads the descriptor. The descriptor also
feeds the posture screen and `tiercheck`.

**Architecture portability.** New code is portable Rust or C. Any
architecture-specific routine lives in an `arch/` directory beside its
portable reference implementation and is selected at run time by CPU feature
detection. NottOS's own crates are built for arm64, x86_64 and riscv64 in CI
from the start, so RISC-V support is not blocked later by an assumption made
now.

## 4. Boot flow

### 4.1 Normal boot

| # | Stage | What it does | On failure |
|---|---|---|---|
| 0 | Boot ROM | Immutable. Verifies the first bootloader stage against keys fused in the SoC. | Does not boot |
| 1 | Vendor bootloader | Verifies its own later stages; starts the TEE and secure element. | Does not boot |
| 2 | Verified boot | On a locked bootloader: verifies the boot metadata against the **NottOS key** set as the user root of trust `[V:F-19]`; checks the rollback index against tamper-evident storage; verifies the boot images; shows the key fingerprint; passes the boot state and key hash to the TEE, which binds them into every key it later releases `[V:F-23]`. | Tries the other A/B slot if it is bootable; otherwise stops with a verification error |
| 3 | Kernel start | Kernel hardening and lockdown active; IOMMU on. On Tier A the kernel seal hook is built in and already dropping packets. | Panic and reset |
| 4 | First-stage init | Loads the vendor modules needed for storage; sets up dm-verity for every system partition; mounts them read-only. | Reboot; slot marked unbootable after repeated failures |
| 5 | SELinux | Policy loaded from the verified image; enforcing. | Does not continue |
| 6 | Second-stage init | Starts `seald` in the state SEALED-BFU; on Tier B, where NottOS does not build the kernel, the drop-all packet policy is loaded here, before any network service starts. Starts HALs. vold unwraps the metadata key through a TEE key bound to the boot state and mounts user data. Device-encrypted storage becomes available. After the last boot-time module is loaded, module loading is disabled. | Reboot |
| 7 | Framework, direct-boot mode | Only components that work without user credentials. Lock screen appears. **Budget point: cold boot to lock screen.** | — |
| 8 | First unlock | Credential is stretched, checked by the secure element or TEE, and yields the keys for credential-encrypted storage (section 5). `seald` receives TEE-backed proof and moves to OPEN. | Throttling by hardware |
| 9 | After boot | The slot is marked good after health checks, and only then is the stored rollback index advanced `[K]`. `ramd` begins preloading. App compilation runs when idle and charging. | Failed health check: revert to the previous slot |

### 4.2 Other paths

| Path | Behaviour |
|---|---|
| **Hardware forced reset** | Identical to a cold boot. The device arrives in SEALED-BFU with no user keys in memory. Software cannot prevent the reset; it makes it useless for data access. |
| **Auto-reboot** | `seald` triggers a reboot after the configured locked time. Same result. |
| **Update** | The new system is written to the inactive slot. On the next boot the bootloader tries it; after repeated failures it reverts. |
| **Recovery** | Accepts only updates that pass the same hybrid signature check, and factory reset. Never mounts user data. |
| **Fastboot and vendor download modes** | Locked bootloader refuses to flash or boot unsigned images. Unlocking must first be enabled from the authenticated OS and wipes user data. Vendor-specific low-level modes are outside NottOS's control; devices where they bypass verified boot are excluded (ADR-0002, rule 3). |
| **Owner-enabled root** | A distinct, visibly different boot state with a persistent indicator. Defined in its own ADR. |
| **Factory reset by a thief** | Open design point. Upstream ties reset protection to a Google account. NottOS needs an equivalent bound to the owner's credential or secure element. Planned ADR. |

### 4.3 On the emulator

Cuttlefish follows the same stages with its own virtual bootloader and a
software implementation of the TEE services `[K]`. The structure of the flow
can be developed and tested there. Nothing that depends on a hardware root
of trust, a secure element or a real TEE can be verified there. Those
properties stay UNVERIFIED until the first hardware port.

## 5. Key hierarchy

### 5.1 On-device keys

```
HARDWARE ROOTS -- never leave the hardware
  SoC hardware-unique key ......... inside the TEE
  Secure-element secrets .......... Weaver slots, StrongBox keys
  Verified-boot root of trust ..... boot state + NottOS key hash,
                                    bound into every TEE-held key

DEVICE-BOUND KEYS -- available at boot, no credential needed
  Metadata key .................... encrypts all filesystem metadata
  Device-encrypted (DE) key, per profile
    both wrapped by a TEE key that is rollback-resistant and usable
    only in the expected verified-boot state

CREDENTIAL-BOUND KEYS -- per profile
  PIN or passphrase
    -> stretch: scrypt (upstream) + Argon2id (NottOS, proposed)
    -> gate:    Weaver slot in the secure element (Tier A)
                or Gatekeeper in the TEE
                the guess limit is enforced here, not by the OS
    -> synthetic password: random, 256-bit, never stored in clear
         -> Credential-encrypted (CE) key for the profile
         -> Keystore super-keys
              after-first-unlock key ... kept until reboot or
                                         end of session
              unlocked-only key ........ EVICTED AT LOCK
                -> evict-on-lock storage class keys
                   (NottOS, opt-in per app)

FILE KEYS
  software mode ...... per-file key derived from the profile key
  inline-engine mode . one hardware-wrapped contents key per
                       profile class, unique IV per file
                       (requirements review, C-21)
  contents: AES-256-XTS      names: AES-256-HCTR2 or CTS
  Adiantum on CPUs without AES instructions

APP KEYS -- Android Keystore
  generated and held in the TEE or StrongBox
  optional authentication binding; attestable
```

Notes:

- Upstream stretches the credential with scrypt tuned to about 25 ms and
  2 MiB, and relies on Weaver or Gatekeeper for the guess limit
  `[V:F-23]`. The brief asks for a memory-hard KDF so that offline guessing
  is expensive even if the hardware gate is defeated. NottOS adds an
  Argon2id stage with per-class parameters `[P]`. This protects
  *passphrases*. A six-digit PIN has a million possibilities and no KDF
  makes that safe once the hardware gate is gone, which is why Tier B
  devices without a secure element should require a passphrase.
- The duress PIN destroys the Weaver slots and TEE keys. Without them the
  synthetic password cannot be recovered, so all credential-bound data is
  unrecoverable even though the ciphertext remains.
- Profiles have independent trees. Ending a session evicts that profile's CE
  key and super-keys.

### 5.2 Eviction

| Event | Evicted from memory |
|---|---|
| Lock | Unlocked-only super-key and every evict-on-lock class key; CE keys of profiles and private space configured to end at lock |
| End session (profile) | That profile's CE key and super-keys |
| Reboot, auto-reboot, forced reset | Everything |
| Duress PIN | Everything, and the hardware secrets needed to derive it again |

The main profile's CE key stays in memory while locked, because Android apps
depend on it (requirements review, C-05, pending the owner's answer to Q1).

### 5.3 Off-device and owner keys

```
NOTTOS RELEASE KEYS
  Offline root ............ SLH-DSA plus a classical signature; kept
                            in hardware, offline; signs only the keys
                            below; changed rarely
    -> Release signing .... ML-DSA plus Ed25519; signs build
                            manifests and update metadata
    -> Repository roles ... signed, expiring metadata
    -> OpenPGP key ........ for people verifying downloads by hand
  Verified-boot key ....... whatever the bootloader accepts: classical
                            on existing devices, ML-DSA where the
                            bootloader supports it; never used for
                            anything else
  System-app signing keys . one per app; hybrid where the platform
                            supports it

OWNER KEYS
  Backup key pair ......... hybrid KEM (X25519 + ML-KEM-1024); the
                            private half is protected by a recovery
                            passphrase or kept on the device
    each backup ........... random 256-bit data key wrapped to it
  Device-to-device transfer ephemeral hybrid KEM, authenticated by a
                            code the user compares on both screens
  Removable-volume key .... random per volume; wrapped by a keystore
                            key and, optionally, by a passphrase so
                            the volume can move between devices
  Lost-device command key . owner's signing key; the phone stores only
                            the public half
```

Rules for all of the above:

- **Hybrid means both.** A hybrid signature is valid only if the classical
  and the post-quantum signature both verify. A hybrid KEM derives its
  shared secret from both exchanges through a published combiner. The
  specific constructions are chosen in the cryptography ADR; NottOS does not
  invent one.
- **Crypto-agility.** Every wrapped key, signature and container starts with
  a format version and an algorithm-suite identifier. Replacing an algorithm
  means re-wrapping keys, not re-encrypting data.
- **Symmetric strength is 256 bits everywhere.**

## 6. Locked-state enforcement points

### 6.1 States

| From | Event | To | Who decides |
|---|---|---|---|
| any | Boot, forced reset | SEALED-BFU | Kernel default; nothing needs to run |
| SEALED-BFU | First unlock, with TEE-backed proof | OPEN | `seald`, after a keystore operation the TEE allows only with a fresh authentication token |
| OPEN | Lock button, timeout, remote lock, framework crash, `seald` restart | LOCKED at the user's level: Sealed, Allowlist or Standard | `seald`; no proof required |
| LOCKED | Unlock, with TEE-backed proof | OPEN | `seald` |
| LOCKED | Auto-reboot timer expires | Reboot, then SEALED-BFU | `seald` |
| any | Emergency call starts | Same state plus a scoped exception for that call | `seald`, on request from telephony |

Locking is easy and needs no proof. Unlocking needs proof that
system_server cannot forge. A compromised framework can lock the device or
ask to unlock it, but cannot open it.

### 6.2 Where each surface is enforced

"Lowest point" is the layer that enforces even if everything above it is
compromised.

| Surface | Lowest enforcement point | Behaviour when Sealed | Exceptions |
|---|---|---|---|
| IP traffic, every interface | Kernel packet hook, default drop from boot. Built into the kernel on Tier A; loaded by the first NottOS-controlled init stage on Tier B. | Dropped in both directions. Open sockets are destroyed so that apps get an immediate error instead of a hang `[K]`. | Allowlist level: per-app pass rule. Emergency bearer. `beacond`. |
| Wi-Fi | Kernel radio block and interface down; HAL stopped | No scanning, no association | — |
| Bluetooth | Kernel radio block; controller down | No scanning, no connections | Audio and hearing devices, if the owner allows (Q2) |
| NFC, UWB | HAL power-down; kernel radio block where present | Off | — |
| Mobile data | Packet hook on data interfaces; data bearers released | Off | Emergency bearer; `beacond` if the owner chose mobile data |
| Cellular voice and SMS | Radio HAL power state | Registered only if the owner allows voice or SMS while locked; otherwise the modem is powered down | An emergency call may always power the modem up |
| Tethering | Follows the interfaces above | Off | — |
| USB | Port-controller data disable through the USB HAL where the device supports it; kernel deauthorises attached devices and unbinds the device-mode controller | Charging only; no enumeration in either role | Accessories attached before lock, by setting (default: disconnect) |
| Shutdown, reboot | Shutdown request path in init and the kernel | User-initiated requests refused without fresh authentication | Thermal and battery protection always honoured; hardware reset cannot be blocked |
| Airplane mode and radio toggles | `seald` owns the controls; the framework's toggles are requests | Refused without authentication | — |
| Debug bridge | Daemon not running; USB function absent | Unavailable | — |
| Boot-mode selection | Bootloader | Locked bootloader; key-combination boot modes disabled where the bootloader allows | Vendor-dependent |

### 6.3 What apps see

No new API. Unmodified apps already handle these standard signals:

- the platform reports the network as blocked through the connectivity
  callbacks;
- scheduled jobs with a network requirement are deferred until the device is
  unsealed;
- sockets in flight are closed with an error, so a transfer fails cleanly
  and the app's own retry logic resumes it after unlock.

### 6.4 How it is tested

Invariants INV-05 to INV-08 and INV-18 in the threat model. On the emulator:
capture on the virtual network interfaces and the virtual USB link for the
whole locked period, including from the first instant of boot. On hardware:
packet capture at the access point, a USB protocol analyser, and RF
measurement. See the roadmap, Phase 4 and Phase 6.

## 7. Default network connections

The brief allows three default connections: updates, time and connectivity
check. A phone that makes calls, finds its position and attests its keys
needs a few more, some only when a feature is first used. GrapheneOS's
published list is the reference for what an AOSP-based system actually does
`[V:F-24]`. The proposed NottOS list `[P]`:

| Connection | When | Can be turned off | If turned off |
|---|---|---|---|
| System updates | Periodic | Yes | No security updates |
| Network time | Periodic | Yes | Clock drifts; certificate checks may fail |
| Connectivity check | On network change | Yes | No captive-portal detection |
| App repository metadata | When the repository client is used or set to auto-update | Yes | No app updates |
| Attestation key provisioning | Occasionally, on devices whose keystore requires it | Yes | Hardware attestation stops working once keys expire |
| Browser security data | Periodic, if the browser is used | Yes | Weaker certificate checks |
| Satellite-positioning assistance data | Only after location is first enabled | Yes | Slower position fix |
| Carrier services (VoLTE, VoWiFi registration) | With a SIM that needs them | Per feature | No calls over those bearers |
| Private-DNS probe | Only if Private DNS is enabled | With the feature | — |
| DRM provisioning | Only on first protected playback | Yes | No protected playback |
| eSIM download | Only when the user adds an eSIM | — | — |
| Lost-device beacon | Opt-in | Yes | No remote locate, lock or wipe |

Every one is listed in Settings with its endpoint and a switch. None carries
a device identifier. The first three match the brief; the rest are recorded
as requirements review C-19 for the owner to accept or strike.

## 8. App execution and sandbox controls

| Control | Enforced in |
|---|---|
| App isolation | Kernel: UID, SELinux domain and categories, seccomp |
| Spawning | Fresh `exec` per app; no shared address-space layout |
| Network permission | Kernel: per-app packet filter and no socket privilege. Framework reports the network as down. |
| Sensors permission | Sensor service returns no data |
| Storage Scopes, Contact Scopes | Framework providers present only what the user selected |
| Camera and microphone switches | Framework privacy switch plus HAL-level block; hardware switch where the device has one |
| Dynamic code loading, native debugging | SELinux rules per app |
| Clipboard, location precision, background activity | Framework services; cgroup freezer for background limits |
| Hardware identifiers | Framework and SELinux deny access |
| Connection log and firewall | Kernel per-app filter; resolver records names; the log stays on the device, in encrypted storage |
| Profiles and private space | Separate key trees; end-session evicts keys |
| Google Play | Ordinary app domain; compatibility shims in the framework give it no privilege |

## 9. Update and integrity verification

```
build (pinned sources, pinned toolchain)
  -> images + build manifest (hash of every partition, verified-boot
     key hash, version, patch level)
  -> hybrid signature by the release key
  -> entry in the public transparency log
  -> mirrors

device: updated
  -> fetch metadata; check hybrid signature, expiry, version greater
     than installed, and log inclusion
  -> download delta or full payload
  -> update_engine writes the inactive slot and checks every hash
  -> reboot; bootloader verifies; health check; slot marked good;
     rollback index advanced
```

Checking the running system, weakest to strongest:

| Check | Trust it against a compromised OS? |
|---|---|
| Verify system app compares partition hashes with the signed manifest in `/nott/attest` | No. A compromised system can show a false result. |
| Verified-boot key fingerprint on the boot screen | Yes, as far as the bootloader is trusted |
| Hardware attestation checked by the Auditor app on a second device | Yes, as far as the TEE or secure element is trusted |
| Reproducible build: anyone rebuilds and compares with the published hashes | Independent of the device |
