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

`--serial <sn>` verifies the connected device matches; `--simulate` runs every command against a mock device (real disk I/O, realistic timing) — use it for safe end-to-end runs without hardware (see `crates/pawflash/src/cli/simulate.rs`).

## Project structure

```
pawflash/
├── Cargo.toml                        → workspace: core, pawflash, src-tauri
├── crates/pawflash-core/             → domain: flash/, force_fastboot/,
│                                        scatter_parser/, output/
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

Affected commands: `force_fastboot`, `disable_vbmeta`, `execute_plan`, `flash_raw_image`. Omitting `on_event` causes silent runtime errors.

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

Shared setup: `.github/actions/setup/`. Linux build deps via `deps: all`: `libudev-dev` (CLI), `libwebkit2gtk-4.1-dev` + `patchelf` (GUI).

## Vendored dep notes

- `fastboot-rs` fork adds: `Flashing(s)`, `SetActive(s)`, `ResizeLogicalPartition`, `SnapshotUpdate`, `split.rs` (sparse image chunking). Bugfix: `Verify` formats as `"verify:"` not `"verity:"`.
- No generated code, no migrations, no codegen.
