# RustyMCU

> Bevy + egui desktop toolkit for embedded board bring-up — serial, USB,
> network, MCU flash/debug, and disk-image writing in one window.

RustyMCU (binary: `board-tools`) is a cross-platform desktop application for
working with microcontrollers and single-board computers. It puts the common
hardware bring-up tasks — watching a serial console, inspecting USB devices,
poking a network endpoint, flashing firmware, and writing disk images — behind
a single native GUI instead of a pile of separate CLI tools.

It is built on the [Bevy](https://bevyengine.org/) engine with
[`bevy_egui`](https://github.com/vladbat00/bevy_egui) for the UI and
`egui_plot` for live charts, and runs as a normal windowed app with a system
tray icon.

## Features

- **Serial console** — open a port, stream output, and send input
  (`serialport`), with a **live plot** panel for numeric telemetry
  (`egui_plot`).
- **defmt decode** — decode [`defmt`](https://defmt.ferrous-systems.com/)-encoded
  log frames from embedded targets into readable messages.
- **USB explorer** — enumerate and inspect attached USB devices (`nusb`).
- **Network panel** — basic network endpoint tooling over async I/O (`tokio`).
- **Flash / image write** — flash MCUs and write disk images via
  [`probe-rs`](https://probe.rs/), with native file pickers (`rfd`) and
  gzip handling (`flate2`) for compressed images.
- **Export** — save captured serial/log/plot data out of the app.
- **Tray icon** — keep the tool resident with a native system-tray entry.

## Build & run

Requires a recent stable Rust toolchain.

```bash
cargo run --release       # launch the GUI (binary name: board-tools)
cargo build --release     # -> target/release/board-tools
```

Bevy needs the usual platform GPU/windowing prerequisites; see the
[Bevy setup guide](https://bevyengine.org/learn/quick-start/getting-started/setup/)
for OS-specific dependencies.

### Platform notes

- **Serial:** on Linux your user typically needs access to the serial device
  (e.g. membership in the `dialout` group) to open a port without root.
- **Flash/debug:** `probe-rs` requires the appropriate debug-probe drivers/udev
  rules to be installed for your probe.
- **USB:** raw USB access may require udev rules (Linux) or the right driver
  (Windows) for the target device.

## Project layout

```
src/
  main.rs            app entry / Bevy app wiring
  state.rs           shared application state
  plugins/           feature plugins
    serial.rs        serial port I/O
    usb.rs           USB enumeration
    network.rs       network tooling
    flash.rs         MCU flash + image write (probe-rs)
    defmt_decode.rs  defmt log decoding
    tray.rs          system tray integration
  ui/                egui panels (serial, plot, usb, network, flash, export,
                     sidebar, theme)
```

## Status

Early but functional desktop tooling, developed as part of the Rustcor
hardware/embedded workflow. Interfaces and panels are still evolving.

## License

Licensed under either of:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
