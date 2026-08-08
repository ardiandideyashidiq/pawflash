# pawflash

MTK device flashing toolkit — force fastboot via the preloader serial handshake, parse scatter manifests, plan & execute full flashes, disable vbmeta, and control devices over fastboot.

## Install

```sh
sudo apt install libudev-dev   # Linux only
cargo build --release          # requires Rust 1.85+
```

Prebuilt binaries for Linux (x86_64) and Windows (x86_64) are published on [releases](https://github.com/ardiandideyashidiq/pawflash/releases). The GUI is a Tauri v2 desktop app; `pnpm tauri dev` runs it in development.

## Usage

```
pawflash force-fastboot

pawflash flash scatter <scatter-path> [--show] [--dry-run] [--json]
        [--storage auto|all|ufs|emmc] [--firmware-dir <dir>]
        [--exclude <name>]... [--check-images] [--include-preloader]
        [--image-search] [--allow-incomplete-slots]

pawflash flash <partition> <image> [--slot a|b] [--both] [--force]
pawflash disable-vbmeta

pawflash device info
pawflash device reboot [system|bootloader|fastbootd|recovery]
pawflash device lock | unlock
pawflash device set-active <a|b>
pawflash device get-var <var-name>

pawflash mtkclient status | download | doctor | read | write | erase
pawflash penumbra status | doctor | da download
pawflash penumbra download | write | read | erase | format <partition> --file <path>
```

Global flags: `-v`/`-vv`/`-vvv` (log verbosity), `--serial <sn>`, `--simulate`.

**Flash policy** — always **full**: flash every safe firmware and Android partition from the scatter, skipping identity/calibration and dangerous partitions (e.g. `nvram`, `nvdata`, `pgpt`, `gpt`). `--include-preloader` opts the preloader in; `--exclude` narrows the set. Storage selection: `auto` (default, prefers UFS), `all`, `ufs`, `emmc`.

## Credits

- **[fastboot-rs](https://github.com/boardswarm/fastboot-rs)** by boardswarm — Rust fastboot protocol library powering pawflash's fastboot engine (vendored fork).
- **[penumbra](https://github.com/shomykohai/penumbra/)** by shomykohai — MTK flash tool in Rust; native in-process DA driver for MediaTek devices (used via the [pawflash fork](https://github.com/ardiandideyashidiq/penumbra)).
- **[mtkclient](https://github.com/bkerler/mtkclient)** by bkerler — MediaTek Flash and Repair Utility; low-level BROM/DA-mode tool (used via a frozen Python bridge from the [pawflash fork](https://github.com/ardiandideyashidiq/mtkclient)).

## License

GPL-3.0-or-later.