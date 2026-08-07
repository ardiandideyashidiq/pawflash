# Design Spike: GUI as a First-Class Core Consumer

Status: **SPEC** (design only — no production code in this document).
Planned at commit `7fc689c`, 2026-08-07. Depends on plans 017, 018, 020, 021
(which have landed; this spec reflects the codebase **after** those plans).

## 1. Problem statement

The GUI (Tauri backend, `src-tauri/`) re-implements or bypasses core
capabilities that the CLI consumes directly. The result is two independently
evolving implementations of device-touching behavior, with the GUI silently
drifting from the CLI's semantics. The audit found four concrete instances
(all now partially addressed by plans 017-021, but the *architecture* that
caused them is unchanged):

1. **Connect/wait** — the CLI waits up to 60s for a device to appear
   (`crates/pawflash/src/cli/flash/scatter.rs:85-89`); the GUI now does too,
   but via its own `AnyExecutor::connect_wait` wrapper
   (`src-tauri/src/sim.rs:61-75`) instead of a shared core entry.
2. **Handshake** — plan 020 consolidated the FASTBOOT loop into core
   (`crates/pawflash-core/src/force_fastboot/handshake.rs`); both UIs now call
   it. This is the model the rest should follow.
3. **Error mapping** — plan 021 gave the boundary a typed `AppError` DTO, but
   the mapping lives in `src-tauri/src/lib.rs` and only covers
   `FlashError::*` — the CLI's `miette`-based rendering is a parallel surface.
4. **Serial pinning** — the CLI has `--serial`
   (`crates/pawflash/src/main.rs:11-14`, `executor/mod.rs:19-21`); the GUI has
   none (plan 007 made ambiguity a hard error instead).

## 2. Seam map (current state, post-plans-017-021)

| Operation | CLI implementation | GUI implementation | Shared core? |
|-----------|--------------------|--------------------|--------------|
| Connect (fail fast) | `FlashExecutor::connect()` | `AnyExecutor::connect` → `FlashExecutor::connect` | partial |
| Connect (wait 60s) | `FlashExecutor::wait_for_device` (`flash/scatter.rs:85-89`, core `connect.rs:102`) | `AnyExecutor::connect_wait` → `wait_for_device` (`sim.rs:61-75`) | wait is core, wrapper is GUI |
| FASTBOOT handshake | `handshake::handshake` (`cli/force_fastboot.rs:54`) | `handshake::handshake` (`lib.rs:334`) | **YES** (plan 020) |
| Fastbootd-mode guard | `disable_vbmeta.rs:43` inline | `lib.rs:509` inline | **NO** — duplicated |
| Slot-target resolution | `cli/flash/raw.rs:79-93` | `lib.rs:734` | **NO** — duplicated |
| Simulated device vars | `simulated_vars()` (core, plan 015) | same | **YES** (plan 015) |
| Error boundary | `miette` diagnostic | `AppError` DTO (`lib.rs:79`, plan 021) | **NO** — two surfaces |
| Serial pinning | `--serial` global flag | none | **NO** |
| Single-flight guard | n/a (single process) | `OpGuard` in lib.rs (plan 017) | GUI-only |
| Cancel token for wait | `CancellationToken` per call | `CancelState.cancel_token` (plan 018) | GUI-only plumbing |

**Key observation**: the two remaining genuine duplications are the
fastbootd-mode guard and slot-target resolution. The handshake (plan 020) and
sim-vars (plan 015) show the pattern that works: **put the operation in core,
have both UIs call it and render the result differently**.

## 3. Proposed core API additions

Add these to `pawflash-core`, each consumed by both UIs. Signatures are
proposals; exact error types should follow the `AppError` DTO pattern from
plan 021 where the GUI needs error classes.

| Operation | Proposed core function | CLI caller becomes | GUI caller becomes |
|-----------|------------------------|--------------------|--------------------|
| Fastbootd guard | `core::flash::ensure_bootloader_mode(executor) -> Result<(), FlashError>` | `cli/disable_vbmeta.rs` calls it; renders `miette` | `lib.rs` calls it; maps to `AppError` + `Error` event |
| Slot resolution | `core::flash::resolve_flash_target(device_vars, name) -> String` | `cli/flash/raw.rs:79-93` | `lib.rs:615-637` |
| Serial pinning | `core::flash::FlashExecutor::set_expected_serial` (exists) + GUI passes serial per-command | `--serial` flag | new `get_device_info`-returned serial → threaded into command args |
| Wait-with-cancel | extend `wait_for_device` to accept an `AtomicBool` probe (already has `CancellationToken`; add the bool form for the GUI's cancel pattern) | unchanged | `AnyExecutor::connect_wait` delegates |

The single-flight guard (`OpGuard`) and the cancel-token plumbing in
`CancelState` are **Tauri-app state**, not core concerns — they stay in
`src-tauri/`. Do not pull them into core.

## 4. `src-tauri/src/lib.rs` decomposition

Currently 858 lines bundling: `init_logging` (26-38), `AppError` DTO
(79-131), `CancelState` + `OpGuard` (122-180), `ScatterCache` (175-237), all
13 commands (239-784), and `run()` (785-811). Proposed split:

```
src-tauri/src/
├── lib.rs        → builder + command registration only (~60 lines)
├── commands/
│   ├── mod.rs    → re-exports + shared helpers (fresh_cancel_token, send_progress)
│   ├── device.rs → get_device_info, reboot, lock, unlock, set_active, get_var
│   ├── flash.rs  → execute_plan, flash_raw_image, disable_vbmeta, cancel_flash
│   ├── force.rs  → force_fastboot (+ simulated variant)
│   └── scatter.rs→ parse_scatter, build_plan, classify_partition
├── state.rs      → CancelState, OpGuard, fresh_cancel_token
├── cache.rs      → ScatterCache
├── error.rs      → AppError + From impls
├── events.rs     → ProgressEvent, DeviceInfo
└── sim.rs        → AnyExecutor (kept)
```

Each module maps to a coherent slice of the current 858 lines; `lib.rs`
retains only the `run()` builder. This is mechanical and behavior-preserving.

## 5. Sequencing for a future build plan

1. **`core::flash::resolve_flash_target`** — smallest win; both callers exist,
   logic is ~15 lines each, pure function over `device_vars`. De-dup first.
2. **`core::flash::ensure_bootloader_mode`** — the fastbootd guard; move the
   `is-userspace` check into core, both UIs call it.
3. **`wait_for_device` bool-cancel variant** — unifies the GUI's cancel
   plumbing with the core wait.
4. **Serial pinning in the GUI** — pick a device in `get_device_info`, pass
   the serial to `execute_plan`/`flash_raw_image`/etc. via command args; core
   `set_expected_serial` already enforces.
5. **Split `lib.rs`** — mechanical, do after 1-4 so the split happens once.

## 6. Open questions (with recommended answers)

1. **Should the GUI support multiple devices with serial selection, or is the
   plan-007 hard error enough?** Recommended: hard error stays; serial
   selection is a future UX feature. The API should accept a serial today so
   the UI doesn't need a breaking change later.
2. **Does core need a `--serial`-style config for the GUI, or per-command
   serial args?** Recommended: per-command serial argument, defaulting to the
   connected device's serial from `get_device_info`. Keeps commands stateless.
3. **Should `get_device_info`'s fast-fail (no wait) be preserved?** Yes — the
   sidebar must reflect live connectivity without 60s hangs; only destructive
   ops wait.
4. **Should the simulated-vars profile become a full `SimulatedDevice` in
   core?** Recommended: yes, long-term — a single struct with vars + command
   recording would remove the `AnyExecutor` enum's two-variant duplication.
   Defer; the enum is acceptable for now.

## 7. Non-goals

- Moving the single-flight guard or cancel-token state into core.
- Making the CLI render through `AppError` (the CLI's `miette` surface is
  fine; they share *producers*, not *rendering*).
- A GUI device picker UI (that's a product feature, not this consolidation).
