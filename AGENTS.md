# pawflash — AGENTS.md

## Commands

```sh
cargo build -p pawflash-core            # core lib only
cargo build -p pawflash                 # CLI (debug)
cargo build --release -p pawflash       # matches CI
cargo build -p pawflash-gui             # Tauri (Rust side, src-tauri crate)
cargo test --workspace                  # all tests
cargo test -p pawflash-core <name>      # single test
cargo clippy --all-targets --all-features --locked -- -D warnings

pnpm lint                                # eslint
pnpm lint:tsc                            # tsc --noEmit
pnpm build                               # tsc && vite build (before tauri build)
pnpm tauri dev                           # Tauri dev server
```

**Order:** `pnpm build` before `cargo build -p pawflash-gui`.

## CLI global flags

`--serial <sn>` verifies the connected device matches; `--simulate` runs every command against a mock device (real disk I/O, realistic timing) — use it for safe end-to-end runs without hardware. Mock runners live in `crates/pawflash-core/src/flash/simulate.rs` and `penumbra/ops/simulate.rs` (plus `SimulatedMtkRunner` in `mtk/ops.rs`).

## DA-mode subcommands

- `pawflash mtkclient` — DA ops via the frozen Python bridge sidecar (`crates/pawflash-core/src/mtk/`).
- `pawflash penumbra` — native DA ops via the in-process `penumbra` crate (fork `ardiandideyashidiq/penumbra`, git dep pinned by `rev`). DA files (`DA/<brand>/<chipset>.bin`) are resolved from the fork's `DA/manifest.json` by device name; the last selection is persisted to `base_data_dir()/penumbra/state.json`. Core: `crates/pawflash-core/src/penumbra/`; CLI: `crates/pawflash/src/cli/penumbra.rs`; GUI: `PenumbraTab.tsx` + `penumbra_*` commands. `base_data_dir()` lives in `penumbra/platform.rs` — Linux `~/.local/share/pawflash`, Windows `%LOCALAPPDATA%\pawflash`, overridable via `$PAWFLASH_DATA_DIR` (used by tests); mtk and penumbra share the device lock.

## Project structure

```
pawflash/
├── Cargo.toml                        → workspace: core, pawflash, src-tauri
├── crates/pawflash-core/             → domain: flash/, force_fastboot/,
│                                        scatter_parser/, output/, mtk/, penumbra/, udev/
├── src-tauri/                        → Tauri v2 backend (lib.rs has commands, ProgressEvent)
├── src/                              → React 19 + Tailwind v4 frontend
│   ├── components/{console,layout,tabs,ui}/
│   ├── types/                        → api.ts, progress.ts
│   └── index.css                     → Tailwind v4 @theme tokens, copper palette
└── vendor/
    └── fastboot-rs/                  → fork of boardswarm/fastboot-rs (+split.rs, +commands)
```

## Critical Tauri wiring

Commands accepting `on_event: Channel<ProgressEvent>` **must** get a frontend-created channel:

```ts
const channel = new Channel<ProgressEvent>();
channel.onmessage = (event) => addProgressEvent(event);
await invoke("force_fastboot", { onEvent: channel });
```

Affected commands: `force_fastboot`, `disable_vbmeta`, `execute_plan`, `flash_raw_image`, `mtk_*`, `penumbra_*`. Omitting `on_event` causes silent runtime errors.

Most device commands also take a `simulate: bool` arg — the frontend threads it through the `useSimulation()` hook (`src/hooks/useSimulation.tsx`), so GUI-only changes must keep that param on every affected `invoke`.

`ProgressEvent` uses `#[serde(tag = "event", content = "data")]` — TS discriminated union mirrored in `src/types/progress.ts`.

## Framework quirks

- **Tailwind v4** — tokens in `@theme` / `@theme inline` in index.css, **no** `tailwind.config.ts`.
- **shadcn/ui base-nova** — uses `@base-ui/react` (NOT Radix).
- **React 19 eslint strictness:**
  - `useRef(Date.now())` flagged — use `useState(() => Date.now())`.
  - Reading `ref.current` during render forbidden — do it inside callbacks only.
  - `react-refresh/only-export-components` — named hook exports in separate files.

## Rust conventions

- No `#[allow]` anywhere; exactly one `#[expect]` (see below) — otherwise fix the lint.
- Max ~400 lines per file; split into directory submodules.
- `tracing` with fields always: `info!(field = value, "msg")` — never format! in log calls.
- CLI prints via `output::status::*` helpers (`data()`, `ok()`/`warn()`/`fail()`, etc.) — never raw println/eprintln.
- Edition 2024, MSRV 1.85. Release profile (`.cargo/config.toml`): `lto = true`, `panic = "abort"`, `strip = "symbols"`.
- Clippy: `all`+`pedantic` = warn; `perf`, `cast_lossless`, `cast_precision_loss`, `cast_sign_loss`, `cargo_common_metadata`, `doc_markdown`, `large_enum_variant`, `missing_const_for_fn`, `needless_pass_by_value`, `redundant_clone` = deny. Workspace rust lints: `unused`, `dead_code` = deny.
- Exactly one `#[expect]` in the repo: `clippy::implicit_hasher` in `output/tables.rs` (tabled-derive workaround, documented inline). No `#[allow]`.
- All tests in-module (`#[cfg(test)]`) in `pawflash-core`; no `tests/` integration dirs. `crates/pawflash` (CLI) declares `assert_cmd`/`predicates` dev-deps but has **no tests yet**.

## CI & release

Single 2-phase workflow (matrix build → one combined release):

| Workflow | Trigger | Build targets | Release tag |
|----------|---------|---------------|-------------|
| `release.yml` | push to main | CLI `pawflash` linux + windows, Tauri bundles linux + windows | `release-YYYYMMDD-HHMMSS` |

Everything is inlined in `release.yml` (no composite setup action, no separate check job — CI builds only, it does not run tests/clippy/lint/typecheck). Linux build deps: `libwebkit2gtk-4.1-dev libgtk-3-dev libglib2.0-dev libxdo-dev libssl-dev librsvg2-dev patchelf libudev-dev`.

## Vendored dep notes

- `fastboot-rs` fork adds: `Flashing(s)`, `SetActive(s)`, `ResizeLogicalPartition`, `SnapshotUpdate`, `split.rs` (sparse image chunking). Bugfix: `Verify` formats as `"verify:"` not `"verity:"`.
- No generated code, no migrations, no codegen.
