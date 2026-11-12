# NottOS performance and memory budgets

- **Version:** 0.1 (Phase 0 draft, awaiting owner approval)
- **Date:** 2026-11-12

**Every number in this document is a proposed target `[P]`.** Nothing has
been measured. No NottOS build exists and this project has no device lab yet.
The targets are engineering judgement about what a trimmed, hardened AOSP can
reach on each hardware class; the first measurement of a stock AOSP baseline
on real devices will show whether they are too soft or too hard, and they
will be revised then, in the open.

A budget counts as met only when it is measured on a real device of the class
by the method in section 6.

## 1. Device classes

| | **L — floor** | **M — mid-range** | **H — high-end** |
|---|---|---|---|
| RAM | 3–4 GB | 6–8 GB | 12–16 GB |
| Storage | eMMC 5.1 | UFS 2.2–3.1 | UFS 4.x |
| CPU | 8 little cores, or 2 older big + 6 little | 2–4 big + little (ARMv8.2–ARMv9.0) | prime + big cores (ARMv9.2 or later) |
| Display | 720p–1080p, 60–90 Hz | 1080p, 90–120 Hz | 1080p–1440p, 120 Hz |
| Typical security hardware | TEE only | TEE, sometimes a secure element | Secure element; MTE on some |
| Likely tier (ADR-0002) | B | B, a few A | A or B |

The reference configuration for budget tables is 3 GB for L, 8 GB for M and
12 GB for H.

## 2. Budgets

Each budget has an **absolute target** and a **relative gate**. The relative
gate compares NottOS with a stock AOSP build of the same Android version on
the same unit, because hardware varies more than software within a class.

### 2.1 Cold boot to lock screen

From kernel entry to the first lock-screen frame that accepts input, in BFU.
Bootloader and firmware time before kernel entry is recorded separately; it
is vendor code that NottOS cannot change.

| | L | M | H |
|---|---|---|---|
| Absolute target | ≤ 25 s | ≤ 14 s | ≤ 10 s |
| Relative gate | ≤ 100 % of AOSP baseline | ≤ 100 % | ≤ 100 % |

Background preloading for run-from-RAM must not delay this; it starts after
the lock screen is interactive.

### 2.2 Cold app start

Time from launch intent to first frame (time to initial display), process not
running and the app's files not in the page cache. Median and 90th percentile
over at least 10 launches.

| Reference workload | L (p50 / p90) | M (p50 / p90) | H (p50 / p90) |
|---|---|---|---|
| NottOS Settings (light system app) | 700 / 1000 ms | 350 / 500 ms | 220 / 320 ms |
| Nott Reader, 10-page PDF, first page drawn (includes spawning the parser sandbox) | 900 / 1300 ms | 450 / 650 ms | 300 / 420 ms |
| Third-party corpus: 10 open-source apps pinned by hash, median app | 1200 / 1700 ms | 600 / 850 ms | 400 / 550 ms |
| Relative gate, all workloads | ≤ 110 % of AOSP baseline | ≤ 110 % | ≤ 110 % |

The relative gate allows 10 % because NottOS spawns apps with a fresh `exec`
instead of forking a zygote. GrapheneOS reports that this costs roughly
200 ms on flagships and more on low-end hardware `[V:F-09]`. Section 4.3
describes how NottOS intends to win most of that back without sharing
address-space layout. If the gate cannot be met on class L, the owner will be
shown the measured cost and asked to choose (see
[requirements-review.md](requirements-review.md), C-02).

### 2.3 Frame time

At the panel's native refresh rate, with frame period `T` (16.67 ms at 60 Hz,
11.11 ms at 90 Hz, 8.33 ms at 120 Hz). Scenarios: home-screen page swipe,
notification shade open and close, Settings list fling, Nott Reader scrolling
a 200-page PDF, app-switch animation.

| | L | M | H |
|---|---|---|---|
| p50 frame time | ≤ 0.5 T | ≤ 0.5 T | ≤ 0.5 T |
| p90 | ≤ 0.9 T | ≤ 0.8 T | ≤ 0.8 T |
| p99 | ≤ 2.0 T | ≤ 1.5 T | ≤ 1.0 T |
| Frames that miss their deadline | ≤ 3 % | ≤ 1.5 % | ≤ 0.5 % |
| Relative gate (missed frames) | ≤ AOSP baseline | ≤ AOSP baseline | ≤ AOSP baseline |

### 2.4 Idle RAM after boot

`MemTotal − MemAvailable`, five minutes after first unlock, no user-installed
apps, home screen visible, default (not Full RAM) mode.

| | L (3 GB) | M (8 GB) | H (12 GB) |
|---|---|---|---|
| Absolute target | ≤ 1.15 GiB | ≤ 1.65 GiB | ≤ 2.40 GiB |
| Relative gate | ≤ 90 % of AOSP baseline | ≤ 100 % | no gate |

M and H deliberately spend more RAM than L on pinned code (section 4). H has
no relative gate because NottOS chooses to use RAM there to remove storage
latency.

### 2.5 System image size

Sum of the read-only images NottOS builds (system, system_ext, product and
NottOS APEX modules), as compressed EROFS. Vendor, ODM and firmware images
come from the device and are excluded.

| | All classes |
|---|---|
| Absolute target | ≤ 1.2 GiB |
| Typical monthly delta update | ≤ 150 MiB |

This is deterministic, so it is gated on every change in CI without a device.

### 2.6 Idle battery drain

Locked, screen off, stationary, 25 °C, battery between 40 % and 80 %, after a
30-minute settle, averaged over at least 4 hours. Reported as mean battery
current; the percentage per hour is for a 5000 mAh reference battery.

| Mode | Target | On a 5000 mAh battery |
|---|---|---|
| Sealed, all radios off | ≤ 5 mA | ≤ 0.10 % per hour |
| Sealed, cellular voice standby allowed | ≤ 12 mA | ≤ 0.24 % per hour |
| Standard: Wi-Fi associated, cellular standby, push delivery, no user apps | ≤ 20 mA | ≤ 0.40 % per hour |
| Relative gate, each mode | ≤ stock firmware in the equivalent state on the same unit | |

The absolute figures assume a modern modem and SoC `[K]`; on older hardware
the relative gate is the meaningful one. The same targets apply to all
classes.

### 2.7 Security-path latencies

These exist because NottOS adds work to the lock and unlock paths.

| Path | L | M | H |
|---|---|---|---|
| Lock event to network sealed (all interfaces dropping) | ≤ 50 ms | ≤ 50 ms | ≤ 50 ms |
| Lock event to USB data off | ≤ 100 ms | ≤ 100 ms | ≤ 100 ms |
| Correct credential to first home frame, AFU | ≤ 1000 ms | ≤ 600 ms | ≤ 400 ms |
| Correct credential to first home frame, first unlock after boot | ≤ 4.0 s | ≤ 2.5 s | ≤ 1.5 s |
| Unseal to network usable (excludes re-association with the access point or cell, which the far side dominates) | ≤ 150 ms | ≤ 150 ms | ≤ 150 ms |

Unlock time is dominated by the memory-hard key derivation and the
secure-element round trip. Android 17 adds secure-element pre-warming that
can save up to 200 ms `[V:F-02]`.

## 3. Why "load everything into RAM" needs a budget

Android already runs hot code from RAM: file pages read from the system image
stay in the page cache. What the page cache does not give is a *guarantee*.
Under memory pressure the kernel evicts those pages and the next use waits
for storage, which on eMMC is the main source of stutter.

There are three levels of commitment, each costing more RAM:

| Level | What it does | RAM cost | Gain |
|---|---|---|---|
| **Pinned hot set** | Locks the pages that common paths touch (framework code, system_server, system UI, launcher, core libraries) so they are never evicted | Only the pinned pages | Removes storage stalls on common paths |
| **Extended pin** | Also pins the AOT-compiled code of the user's most-used apps and less common system paths | Several hundred MiB | Removes stalls on most paths |
| **Full RAM mode** | Copies the whole compressed system image into RAM; storage is not read again for system code until reboot | The whole image, about 1.2 GiB, plus decompressed hot pages | No system-code storage access at all; fewer storage wake-ups |

Loading a 1.2 GiB image on a 3 GB phone would take 40 % of memory away from
apps, force constant app kills and make the device slower, not faster. So the
level is chosen per device from a budget:

**Rule `[P]`:** pinned memory plus RAM-loaded image may not exceed 6 % of RAM
on devices under 6 GB, 10 % from 6 GB to under 12 GB, and 20 % from 12 GB up.

## 4. Memory plan per class

### 4.1 Idle memory budget

Figures in GiB, for the reference configuration of each class. "Usable" is
what the kernel reports as `MemTotal` after firmware reservations `[K]`.

| Consumer | L (3 GB) | M (8 GB) | H (12 GB) |
|---|---|---|---|
| Usable RAM (assumed) | 2.80 | 7.50 | 11.30 |
| Kernel, page tables, slab | 0.28 | 0.40 | 0.55 |
| Core system processes (system_server, compositor, HALs, daemons) | 0.42 | 0.55 | 0.70 |
| Launcher, system UI, keyboard | 0.22 | 0.28 | 0.33 |
| Pinned hot set | 0.15 | 0.30 | 0.60 |
| tmpfs volatile state, typical (hard cap) | 0.02 (0.05) | 0.04 (0.10) | 0.06 (0.20) |
| Spare app-process pool | 0.03 (1 process) | 0.06 (2) | 0.14 (4) |
| **Idle total** (targets in 2.4: 1.15 / 1.65 / 2.40) | **1.12** | **1.63** | **2.38** |
| **Left for apps and cache** | **≈ 1.7** | **≈ 5.9** | **≈ 8.9** |

### 4.2 Modes offered per class

| | L | M | H |
|---|---|---|---|
| Pinned hot set | Yes, ≤ 0.15 GiB | Yes, ≤ 0.30 GiB | Yes, ≤ 0.60 GiB |
| Extended pin | No | Optional at 8 GB, ≤ 0.8 GiB total | Yes |
| Full RAM mode | No | No | Optional from 12 GB, ≤ 20 % of RAM |
| Amnesic session (session data only in RAM, gone at shutdown) | No | Optional at 8 GB, session capped at 1.5 GiB, with a warning | Optional, session capped at 25 % of RAM |
| Compressed RAM swap (zram) size | 1.5 GiB | 3 GiB | 4 GiB |
| Kernel page size | 4 KB | 4 KB; 16 KB where the device ships it | 16 KB where the device ships it |
| Multi-size huge pages for anonymous and file memory | Off | Measure first | On, if measured gain |
| Cached background apps | Strict cap, frozen when cached | Moderate | Generous |

Notes:

- **zram, not disk swap.** Swap on flash wears the storage and would put
  session memory on disk. zram keeps it in RAM. At a typical compression
  ratio near 2.5:1 `[K]`, a full 1.5 GiB zram device on class L occupies
  about 0.6 GiB and yields a net gain of about 0.9 GiB.
- **Page size.** 16 KB pages give measured gains (boot about 8 % faster, app
  launch about 3 % faster under memory pressure) at the cost of "slightly
  more memory" `[V:F-15]`. Page size is fixed when the kernel and vendor
  modules are built, so NottOS can only use what the device supports. Class L
  stays on 4 KB because memory is its scarcest resource.
- **Background limits are strict but fair:** cached apps are frozen rather
  than killed where possible; foreground services the user can see are not
  limited; the limits are per class, visible in Settings and overridable per
  app.
- **Amnesic mode is not just tmpfs.** It must also guarantee that nothing is
  written to any persistent partition during the session. Its design is left
  to the run-from-RAM ADR.

### 4.3 Fast cold start without weakening ASLR

Allowed, because they share file pages but not address-space layout:

1. **Page-cache preloading and pinning.** Each process still maps the pinned
   files at its own random addresses.
2. **Ahead-of-time compilation with profiles.** System and core apps are
   compiled at build time; user apps at install and when idle and charging.
   Profile-guided layout reduces the number of pages a start touches.
3. **A spare-process pool of freshly exec'd processes `[P]`.** A few
   processes are started ahead of time, each by its own `exec`, so each has
   an independent random layout. A launch takes one and specialises it; a
   replacement is started in the background. The exec cost is paid before the
   user taps. Pool size follows the class (table above). This is a NottOS
   proposal, not an existing upstream or GrapheneOS feature; its benefit and
   memory cost are UNVERIFIED.

Excluded, because they make layout predictable across processes:

- Forking apps from a shared zygote, including upstream's pre-forked pool.
- Prelinking or any mechanism that gives a library the same address in every
  process. Upstream's cross-process sharing of the WebView library's
  relocated data needs review against this rule `[K]`.

## 5. Use of hardware

What NottOS relies on upstream for, and where it depends on the device. All
`[K]` until each device is audited.

| Mechanism | Use | Depends on |
|---|---|---|
| Heterogeneous cores | Energy-aware scheduling with utilisation clamping; latency-sensitive threads on big cores | SoC energy model from the vendor |
| GPU | Vulkan-based composition and UI rendering; hardware overlays first | Vendor Vulkan driver quality |
| Video | Hardware codecs through the standard codec HAL; software codecs sandboxed | Vendor codec HAL |
| Storage encryption | Inline encryption engine with hardware-wrapped keys where trustworthy; Adiantum where the CPU has no AES instructions | SoC engine; device audit |
| CPU crypto and vector | AES, SHA-2, polynomial multiply; NEON everywhere; SVE2 where present | Runtime feature detection |
| Memory tagging | MTE synchronous mode for kernel and userspace | Devices that have MTE (some Tier A) |
| Fast storage features | Command queueing on eMMC; write booster and background operations control on UFS | Device firmware |
| NPU and DSP | Only through open, documented interfaces | Few devices qualify |

## 6. How budgets are measured

| Stage | Trigger | Runs on | Checks |
|---|---|---|---|
| A | Every change | Build host and Cuttlefish | Unit and functional tests; image size budget; build-time invariants |
| B | Nightly | One real device per class | Boot time, app start, frame time, idle RAM |
| C | Weekly and before release | Real devices with an external power monitor | Idle drain in each mode |

Method:

- At least 10 iterations per benchmark, first discarded; report median and
  90th percentile with a bootstrap confidence interval.
- Tools: Perfetto traces and frame timeline, launch timing from the activity
  manager, `/proc/meminfo`, boot-progress events, an external power monitor
  for current `[K]`.
- **Regression gate:** a change that worsens a median by more than 5 % or a
  90th percentile by more than 10 % against the 7-day rolling baseline fails
  stage B.
- **Release gate:** any absolute budget exceeded on a supported device blocks
  the release for that device, unless the owner signs a recorded exception.
- Results are published with the device model, build ID and raw data.

The same lab validates assembly routines. Proposed acceptance rule `[P]`: an
assembly path is kept only if it is at least 10 % faster than the portable
reference on the target operation in at least one class, with no regression
in the others, and passes differential and constant-time tests.

## 7. What is needed before any number is real

1. A build host able to build AOSP: at least 64 GB of RAM and 400 GB of disk
   `[V:F-22]`. This development environment cannot.
2. One real device per class, plus a power monitor.
3. A stock AOSP baseline measured on those devices.

Until then every budget above is UNVERIFIED.
