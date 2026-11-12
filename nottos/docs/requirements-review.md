# Requirements review — questions, conflicts and limits

- **Version:** 0.2 (Phase 0, awaiting owner approval)
- **Date:** 2026-11-12
- **Change from 0.1:** the complete brief (sections 9 to 15) was received.
  Tiers now follow section 13. Items C-18 to C-24 were added from sections
  11 to 14. Open decisions are consolidated into seven questions.

The brief asks that no requirement be dropped silently or pretended into
existence. Part 1 holds the questions whose answers change the architecture.
Part 2 lists every requirement found to be impossible as written, unsafe, or
in conflict with another, with the closest achievable design. Where no
question is asked, the stated assumption is used unless the owner objects.

Evidence tags are explained in the [README](../README.md).

## Part 1 — Seven questions for the owner

Each has a recommendation, which is also the default if there is no answer.

### Q1 — Should the main profile's keys be evicted at every lock?

**Why it matters.** It decides the storage key hierarchy (C-05).

- **Option A, graded (recommended).** At lock, evict: keystore keys marked
  unlocked-only; profiles and the private space set to end at lock; a new
  opt-in storage class for apps that ask for it. The main profile's storage
  key stays until auto-reboot. Apps keep working while locked.
- **Option B, strict.** Evict the main profile's key at every lock. Stronger
  against an attacker with a locked device, but alarms, incoming messages,
  music and background sync stop, and many apps misbehave, because Android
  apps assume their files stay readable after the first unlock.

### Q2 — Under Sealed, do Bluetooth audio and hearing devices stay connected?

**Why it matters.** It decides where Bluetooth is enforced (C-06). A full
radio block is enforceable in the kernel. Keeping selected devices connected
means the decision is made per link in the Bluetooth stack, which is a
larger and weaker enforcement point.

- **Option A (recommended).** Disconnect everything at lock by default. A
  clearly labelled setting keeps audio and hearing devices connected, and is
  offered at setup when a hearing aid is paired.
- **Option B.** Keep audio and hearing devices connected by default.

### Q3 — Who operates the lost-device service, and how does it reach a sealed phone?

**Why it matters.** It decides a protocol, a server and the one deliberate
hole in the seal (C-06).

- **Option A (recommended).** The phone polls an endpoint the owner
  configures: a relay run by the NottOS project by default, replaceable by a
  self-hosted one. Commands are signed by the owner's key, so the relay can
  neither forge nor read them. The owner chooses whether polling may use
  mobile data, known Wi-Fi networks, or both.
- **Option B.** No server. Commands arrive by signed SMS. Needs cellular
  voice and SMS allowed while locked, and exposes the baseband.
- **Option C.** No lost-device function in the first release.

### Q4 — How faithful must Nott Reader be for office documents?

**Why it matters.** It decides the Reader's architecture and the system
image size (C-20).

- **Option A (recommended).** Built-in Rust converters give a *readable*
  rendering of DOCX, XLSX, PPTX, ODF and RTF: text, tables, images and basic
  styling, reflowed, not page-exact. An optional "high-fidelity office
  pack", built on an existing office engine confined in the same sandbox,
  can be installed from the NottOS repository by users who need exact
  layout.
- **Option B.** Ship the office engine in the base image. Exact layout for
  everyone, at a cost of a large, complex C++ component and a substantial
  share of the image budget.
- **Option C.** Readable rendering only; no high-fidelity path.

### Q5 — How many independent people will hold release keys?

**Why it matters.** It decides the key ceremony and whether one compromised
person or machine can sign a malicious release (threat model, A8).

- **Option A (recommended).** At least three holders with a two-of-three
  rule for release signing, and the offline root split the same way.
- **Option B.** A single holder (the owner). Simpler, but theft or coercion
  of one person compromises every device. The transparency log and
  reproducible builds would then be the only check.

### Q6 — Will the project get a build host and its own repositories?

**Why it matters.** It decides what the next phases can verify (C-15).
Building AOSP needs at least 64 GB of RAM and 400 GB of disk `[V:F-22]`; the
current environment has 15 GB and 30 GB. An AOSP derivative also needs many
repositories, and this session can only write to an existing general one.

- **Option A (recommended).** The owner provides a build machine or cloud
  budget with hardware virtualisation, and a dedicated organisation for the
  NottOS repositories, before Phase 1 starts.
- **Option B.** Continue in this environment. Phases 1 to 5 then deliver
  host-side Rust components with tests, and every emulator-based acceptance
  criterion stays UNVERIFIED.

### Q7 — Is a hardware-partner track started now, and may hardware work begin before Phase 6?

**Why it matters.** It decides how late security patches arrive (C-14),
whether a certification path exists (C-03), and how long hardware-dependent
designs stay untested (roadmap, risks).

- **Option A (recommended).** Start looking for a partner now, in parallel.
  Run a small hardware spike alongside Phase 2: boot NottOS with its own key
  on the candidate Tier A device, to find hardware surprises early.
- **Option B.** Keep the phase order strictly; first hardware contact in
  Phase 6; no partner track.

## Part 2 — Conflicts and limits

| ID | Topic | Kind | Status |
|---|---|---|---|
| C-01 | Device breadth vs. full security guarantees | Conflict | Resolved by section 13; one addition in ADR-0002 |
| C-02 | "Best at all four goals" vs. the cost of hardening | Conflict | Assumption |
| C-03 | Play Integrity verdicts | Not achievable by an OS | Informational |
| C-04 | Post-quantum signatures in the boot chain | Limited by hardware | Assumption |
| C-05 | Evicting keys on every lock | Conflict with the app model | **Q1** |
| C-06 | Sealed vs. Bluetooth devices, locate, emergency | Conflict | **Q2, Q3** |
| C-07 | "Nothing outside user data is writable" | Impossible as written | Assumption |
| C-08 | Run-from-RAM on the 3 GB floor | Conflict | Assumption |
| C-09 | Enforcement in USB and radio firmware | Limited by hardware | Informational |
| C-10 | All keys in a secure element | Limited by hardware | Assumption |
| C-11 | Kernel hardening vs. prebuilt vendor modules | Limited by hardware | Assumption |
| C-12 | Syscall filters for vendor HALs | Costly per device | Assumption |
| C-13 | Hand-written assembly for cryptography | Unsafe as written | Assumption |
| C-14 | Monthly, prompt security patches without partner access | External dependency | **Q7** |
| C-15 | Building and testing an OS in this environment | Tooling limit | **Q6** |
| C-16 | Integrity folder "marked immutable" | Wording | Assumption |
| C-17 | Sections missing from the brief | Input | Resolved |
| C-18 | System apps "sandboxed like any third-party app" | Impossible for some apps | Assumption |
| C-19 | Only three default network connections | Too few in practice | Assumption |
| C-20 | Nott Reader: breadth, fidelity, size and licences | Conflict | **Q4** |
| C-21 | Per-file keys vs. the inline encryption engine | Conflict | Assumption |
| C-22 | Tests that "prove" the locked-state policy | Wording; hardware arrives late | Assumption |
| C-23 | eSIM, VoLTE and VoWiFi need closed components | External dependency | Assumption |
| C-24 | "Small" system apps; offline speech and captions | Limits | Assumption |

### C-01 — Device breadth vs. full security guarantees

**Asked for.** Maximum compatibility and maximum security, handled with
honest tiers A, B and C (section 13).

**Remaining problem.** As defined, Tier B holds both devices that verify
their boot chain with the NottOS key and devices whose bootloader cannot be
relocked. The second kind has no protection against tampering and cannot be
attested.

**Design.** The brief's tiers, with Tier B divided into B-verified and
B-unverified as a headline attribute
([ADR-0002](adr/0002-device-support-tiers.md)). Approved or rejected with
that ADR.

**Also note.** Tier A hardware is rare `[V:F-08]`, and a Tier A phone at the
3 GB floor has not been identified `[K]`. The low-end class will probably be
Tier B.

### C-02 — "Best at all four goals" vs. the cost of hardening

**Problem.** The mandated hardening has a price an unhardened system does
not pay. Exec-based spawning costs about 200 ms per cold start on flagships
and more on low-end hardware, plus memory `[V:F-09]`. A hardened allocator,
zero-on-free, synchronous MTE and control-flow integrity each cost CPU or
RAM `[K]`.

**Design.** The priority order resolves it: security wins. NottOS aims to be
the fastest and leanest system *at this security level*, held to explicit
budgets and a no-regression gate against stock AOSP on the same device, with
a 10 % allowance on cold start
([performance-budgets.md](performance-budgets.md)).

**Assumption.** Where a budget and a mandated mitigation collide on the
low-end class, the mitigation stays, the measured cost is reported, and the
owner is asked before anything is relaxed.

### C-03 — Play Integrity verdicts

From Google's current definitions `[V:F-05]`:

| Verdict | Requires | NottOS |
|---|---|---|
| `MEETS_BASIC_INTEGRITY` | On Android 13+, "only that the attestation root of trust is provided by Google" | Plausibly obtainable on hardware with Google-rooted attestation keys. UNVERIFIED until tested. Returned only to apps installed through Google Play. |
| `MEETS_DEVICE_INTEGRITY` | "a genuine and certified Android device [...] the loaded Android OS is a certified device manufacturer image" | **Not obtainable.** |
| `MEETS_STRONG_INTEGRITY` | Device integrity plus recent patches on all partitions | **Not obtainable.** |

Apps that demand device or strong integrity, which includes many banking
apps, will refuse to run, as they do on GrapheneOS `[V:F-10]`. Nothing
inside NottOS changes that. NottOS will not spoof verdicts, fingerprints or
attestation.

**What NottOS does.** Full hardware key attestation with verified boot; a
published, signed list of verified-boot key fingerprints; a developer guide
for allowlisting NottOS directly `[V:F-10]`.

**Separate track.** Certification needs compliance with the compatibility
definition, passing CTS and a commercial agreement with Google `[V:F-21]`,
which in practice needs a hardware partner (Q7). Several NottOS features are
likely deviations and would have to be optional in a certified build; the
gap list is a Phase 7 deliverable.

Google's developer-verification rules, now enforced in Brazil, apply to
certified devices `[V:F-14]`. They do not apply to NottOS.

### C-04 — Post-quantum signatures in the boot chain

**Problem.** The bootloader verifies the first NottOS image, and NottOS
cannot change the bootloader of an existing phone. Android 17's
verified-boot library is gaining ML-DSA `[V:F-07]`, but Google has not said
that existing bootloaders will be updated, and the supported parameter sets
could not be confirmed `[V:F-20]`. A post-quantum check added later in the
boot is itself protected only by the classical signature the bootloader
checked.

**Design.**

- Updates, repository metadata, backups and device transfer: fully hybrid
  from the first release. NottOS controls both ends.
- Boot chain on existing devices: the strongest classical algorithm the
  bootloader accepts, plus a hybrid-signed build manifest checked by the
  updater before installation and by Verify system.
- Boot chain where the bootloader verifies ML-DSA: use it.
- SLH-DSA for the offline root keys.

**Why acceptable.** Forging a boot signature needs a quantum computer at the
time of the attack, and physical access. Nothing recorded today can be
attacked later through this path. The update channel is reachable remotely
and gets post-quantum signatures.

### C-05 — Evicting keys on every lock

**Problem.** Android has two storage classes: available after boot, and
available after the first unlock `[V:F-23]`. Apps assume their files stay
readable while the screen is locked. Evicting the main profile's keys at
every lock breaks most of them. No upstream class means "only while
unlocked".

**Design.** Graded eviction; see **Q1**.

### C-06 — Sealed vs. Bluetooth devices, locate, emergency

**Problems.**

1. Locating a lost phone needs a network connection; a sealed phone has
   none.
2. Emergency calls on 4G and 5G are IP traffic. On some devices that stack
   runs on the application processor `[K]`, so a blanket IP drop would block
   the call.
3. Bluetooth is mostly not IP. "Drop IP" leaves audio, watches and hearing
   aids untouched; "sealed box" disconnects them all at every lock.

**Design.**

- Sealed covers the link layer too: Wi-Fi, Bluetooth, NFC and UWB are
  powered down or blocked.
- Emergency exception: starting an emergency call opens only what that call
  needs, for its duration, in every mode.
- Locate exception, opt-in: one service, one endpoint, outbound polling,
  owner-signed commands. See **Q3**.
- Bluetooth devices connected before lock: see **Q2**.

**Stated consequence of the Sealed default.** No notifications, messages or
internet calls arrive while locked. The setup wizard must say so in plain
words.

### C-07 — "Nothing outside the encrypted user-data partition is writable"

**Problem.** Real devices have small writable partitions outside user data
that the hardware and Android need: encrypted key blobs and update state,
the bootloader's control block, vendor calibration data, modem state `[K]`.

**Design.** Enumerate them per device; mount them without execute, device or
set-uid permissions; allow writes only from the domains that own them; audit
what is stored. All volatile system state stays in tmpfs (INV-02).

### C-08 — Run-from-RAM on the 3 GB floor

**Design.** Three levels (pinned hot set, extended pin, Full RAM mode)
selected by a per-class RAM budget; Full RAM and amnesic modes only from
12 GB, amnesic optionally at 8 GB with a small cap
([performance-budgets.md](performance-budgets.md), sections 3 and 4).

### C-09 — Enforcement in USB controller and radio firmware

**Limit.** That firmware is closed. NottOS uses the controls the vendor
exposes and enforces in the kernel everywhere. Whether a powered-down radio
is really silent can be measured in a lab, not proven from software.

### C-10 — All keys in a secure element

**Limit.** Many phones have a TEE but no secure element `[K]`. There,
throttling is done by the TEE: still outside the OS, but weaker against
physical attack. Irreversible key destruction is also strongest with a
secure element.

**Assumption.** Tier A requires a secure element (as section 13 says).
Tier B devices without one must use a passphrase rather than a short PIN,
and are told why.

### C-11 — Kernel hardening vs. prebuilt vendor modules

**Limit.** Hardening options that change the kernel module interface need
vendor modules rebuilt from source. "No module loading after boot" requires
every module to be loaded at boot, at some memory cost `[K]`.

**Assumption.** Full profile on Tier A; on Tier B, only what the vendor's
kernel allows, shown on the posture screen.

### C-12 — Syscall filters for vendor HALs

**Limit.** Upstream does not filter most vendor HAL processes. Doing so
needs a per-device trace of what each closed binary calls.

**Assumption.** Filters for every NottOS and AOSP process class from the
start. Vendor HAL filters are generated per device from traces and enforced
once stable; the vendor binary manifest shows which are enforced.

### C-13 — Hand-written assembly for cryptography

**Problem.** Section 4 asks for assembly in cryptographic primitives.
Section 9 says "use vetted implementations, formally verified where
available, and never design new primitives". New cryptographic assembly is
where timing leaks come from, and the vetted libraries already contain
reviewed, hardware-accelerated assembly.

**Design.** For cryptography, NottOS uses the assembly inside vetted
libraries and the kernel's crypto subsystem and writes none of its own,
unless no vetted implementation exists for a needed primitive and platform.
The assembly policy applies in full to non-cryptographic routines. No
assembly is written before a device lab exists to measure it (Phase 6).

### C-14 — Monthly, prompt security patches without partner access

**Asked for.** "Monthly security patches merged promptly" (section 14).

**Problem.** Upstream changed. AOSP source arrives twice a year `[V:F-12]`,
monthly bulletins now carry only high-risk issues with most fixes shipped
quarterly `[V:F-13]`, and device-specific code for the reference phones is
no longer public `[V:F-11]`. Device makers receive fixes under embargo; an
independent project does not.

**Consequence.** NottOS can merge what is published as soon as it is
published. It cannot be ahead of publication, and on Tier B it cannot patch
vendor firmware at all.

**Design.** Automated rebase onto each upstream security release; patch
level and age shown in Verify system; partner track. See **Q7**.

### C-15 — Building and testing an OS in this environment

**Limit.** 4 CPU cores, 15 GiB RAM, about 30 GB of free disk, no emulator,
no AArch64 toolchain ([fact-check.md](fact-check.md)). AOSP needs at least
64 GB RAM and 400 GB disk `[V:F-22]`.

**Possible here.** Design documents; host-side Rust components with unit
tests and fuzzing.

**Not possible here.** Building system images, booting the emulator,
running CTS, measuring any budget, anything on a real device. See **Q6**.

### C-16 — Integrity folder "marked immutable"

**Wording.** The system image is a read-only filesystem format protected by
dm-verity. It has no per-file immutable flag, and needs none: nothing on it
can be changed by anything, root included, without failing verification at
the next read. The layering asked for is kept (verified read-only image,
dedicated SELinux type, no write rule for any production domain).
`/nott/attest` is feasible as a directory in the root of the system image
and is the proposed final path.

Because the image cannot be written in place, "root write access" in an
owner-enabled root mode cannot mean editing that directory on the verified
image. It would mean an overlay or a rebuilt image, with verified boot no
longer attesting the original. The root-mode ADR must say which.

### C-17 — Sections missing from the brief

Resolved. The complete brief was received and Phase 0 now follows
section 15.

### C-18 — System apps "sandboxed like any third-party app"

**Problem.** Some listed apps cannot work without privileges no third-party
app can hold: the dialer and telephony (emergency calling, call control),
the SMS/MMS handler, Settings, the system updater, backup and restore,
Verify system (reads system partitions), the setup wizard, and the
repository client (installs packages).

**Design.** Every system app runs in its own app sandbox and is updated and
confined like any app. Those that need a platform privilege get exactly
that privilege, listed in a published per-app table and checked in CI
(roadmap, criterion 5.1). Apps that need none hold none
([architecture.md](architecture.md), section 2.3).

### C-19 — Only three default network connections

**Asked for.** The only default connections are updates, time and
connectivity check (section 6).

**Problem.** A phone with the functions in section 12 makes a few more:
attestation key provisioning on devices whose keystore requires it;
satellite-positioning assistance data; carrier registration for VoLTE and
VoWiFi; browser security data; DRM provisioning on first protected
playback. GrapheneOS's published list shows the same set `[V:F-24]`.

**Design.** Keep the three as the only connections made by a fresh device
that has no SIM and has not enabled location. Everything else happens only
when the feature that needs it is first used, is listed in Settings with its
endpoint and a switch, and carries no device identifier
([architecture.md](architecture.md), section 7).

**Assumption.** The owner accepts that list. Any item can be struck, with
the consequence shown in that table.

### C-20 — Nott Reader: breadth, fidelity, size and licences

**Problems.**

1. **Office formats.** Page-exact rendering of DOCX, XLSX, PPTX and the
   legacy binary formats needs a full office layout engine. Writing one in
   Rust is years of work. Existing engines are large C++ code bases; they
   can be confined by the sandbox but weigh heavily on the image budget.
   See **Q4**.
2. **CBR.** These are RAR archives. The reference RAR decoder's licence is
   not an open-source licence `[K]`. NottOS would use an independent,
   permissively licensed decoder, which may not handle every RAR variant.
3. **HEIF.** Most HEIF images use HEVC, which is patent-encumbered `[K]`. A
   software decoder inside the sandbox raises a licensing question; the
   device's own licensed hardware decoder is closed vendor code handling
   untrusted input. The choice goes in the Reader ADR; until then HEIF
   fidelity is listed as "depends on device".
4. **Digital signatures.** Verifying a signature needs trust anchors and,
   for revocation, network access, which the Reader does not have. The
   Reader can verify integrity and the certificate chain against a bundled
   trust list, and must label revocation as "not checked".

**Design.** A published matrix with a fidelity level per format, never a
blanket "supported": faithful (PDF, images, text, EPUB), readable (office,
RTF, HTML, e-mail), container (archives), device-dependent (HEIF).

### C-21 — Per-file keys vs. the inline encryption engine

**Asked for.** Per-profile and per-file keys, and the SoC's inline
encryption engine where it can be trusted (section 9).

**Problem.** Inline engines have few key slots. Android's mode for them
"derives just one file contents encryption key per CE or DE key, rather than
one per file", with a unique IV per file `[V:F-23]`. The two requests cannot
both hold on the same device.

**Trade-off.** With an inline engine and hardware-wrapped keys, the raw
storage key never exists in kernel memory, which is a real gain against
memory-extraction attacks on a locked device. Per-file keys limit the
damage of a single key's exposure. For AES-XTS with unique IVs, the
cryptographic difference between the two is small.

**Assumption.** Where a device audit trusts the inline engine: wrapped keys,
per-profile contents keys, per-file IVs. Elsewhere: software encryption with
per-file keys. Recorded per device on the posture screen and fixed in the
storage ADR.

### C-22 — Tests that "prove" the locked-state policy

**Asked for.** Tests "must prove that no packet and no USB transaction
passes while the device is locked" (section 14).

**Limit.** A test shows that nothing passed in the scenarios run. Combined
with a design in which the default is drop and one small component owns the
controls, that is strong evidence. It is not a proof in the mathematical
sense, and on the emulator it says nothing about a real USB controller or a
real radio.

**Design.** Emulator capture over a scenario matrix in Phase 4; USB protocol
analyser, access-point capture and RF measurement on hardware in Phase 6.
Phase reports use the word "verified" only for what was measured.

### C-23 — eSIM, VoLTE and VoWiFi need closed components

**Problems.**

- AOSP contains no eSIM profile manager; one must be added as a privileged
  system app. An open-source one exists `[V:F-26]`.
- VoLTE and VoWiFi stacks are closed vendor components tied to carrier
  configuration. On generic-image installs they frequently do not work
  `[K]`.
- As older networks are retired, a phone without working VoLTE may be unable
  to place any call, emergency calls included.

**Design.** Emergency calling is a gating test for every device in every
tier (ADR-0002, rule 3). The eSIM manager and its privilege are recorded in
an ADR. VoLTE and VoWiFi status per device and carrier is published, never
assumed.

### C-24 — "Small" system apps; offline speech and captions

- **Browser and WebView.** A Chromium-based engine is neither small nor
  cheap to maintain: it needs frequent security rebuilds on large build
  machines. NottOS should reuse an existing hardened build as its upstream
  rather than maintain its own patch set from scratch (Reader and browser
  ADRs).
- **Text-to-speech.** Offline voices of good quality for Brazilian
  Portuguese exist under open licences, but the best ones are large and slow
  on low-end hardware `[K]`. The choice, with size and licence, goes in an
  ADR.
- **Captions.** Assumed to mean the system caption support for media that
  carries caption tracks. Automatic captioning of arbitrary audio needs an
  on-device speech-recognition model, which is large; it is treated as a new
  feature the owner would have to request.
