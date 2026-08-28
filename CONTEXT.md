# pawflash — Domain Glossary

## Terms

### Platform

A `Platform` implementation encapsulates all OS-varying behavior for pawflash. The codebase has one compile-time selected instance (`platform::CURRENT`). Consumers never use `#[cfg]` directly for platform logic — they call trait methods on `CURRENT`.

Implementations:
- `platform::linux::Impl` — Linux (serial `/dev/ttyACM*`, udev rules, `$XDG_DATA_HOME`)
- `platform::windows::Impl` — Windows (serial `COM*`, USBDK, `%LOCALAPPDATA%`)

### FlashTransport

An async trait abstracting the fastboot USB protocol. Three implementations: `NusbFastBoot` (real device), `MockTransport` (unit tests), `SimulatedTransport` (simulate mode). The `FlashExecutor<T: FlashTransport>` is generic over this trait.

### FlashExecutor

The core flash orchestrator. Generic over `FlashTransport`, defaulting to `NusbFastBoot`. Connects to a device, reads device vars, and executes flash plans partition-by-partition.

### DA (Download Agent)

A MediaTek-specific protocol for direct partition access without fastboot. Operated via two backends: the frozen mtkclient bridge (Python subprocess) and the penumbra library (native Rust).

### Scatter File

A MediaTek flash manifest (XML or text) describing partition layout, regions, and image mappings. Parsed by `scatter_parser` into a `FlashPlan`.

### Force-Fastboot

The process of transitioning a MediaTek device from preloader mode to fastboot mode via serial handshake. Involves: waiting for preloader serial port, opening it, writing `FASTBOOT` commands, and detecting the mode switch via USB.

### Preloader

The initial USB serial port a MediaTek device exposes before entering fastboot or DA mode. Typically `/dev/ttyACM0` on Linux, `COM*` on Windows.

### USBDK

A Windows USB driver/library required by mtkclient's Python backend for direct USB access. Not needed on Linux (which uses usbfs directly).

### WinUSB

The Windows USB driver that nusb uses for fastboot device communication. Must be installed via Zadig or Device Manager for fastboot operations on Windows.
