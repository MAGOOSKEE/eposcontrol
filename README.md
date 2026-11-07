# EposControl

A fast, light, open replacement for EPOS Gaming Suite. Target hardware: EPOS GSX 300 (USB VID 0x1395, PID 0x0098). Windows first, cross platform later.

## Goals

- Starts silently in the system tray on boot (user can disable this).
- Tiny memory and CPU footprint, instant startup, no background service.
- Clean, standard layout with the Material Symbols icon set. No emojis.
- Installer for releases.

## Stack

Pure native Rust. No web view.

- `crates/epos-device`: HID protocol for the GSX 300.
- `crates/app`: Slint UI (software renderer, Material style), tray icon, launch at startup.

## Build

```
cargo build --release -p eposcontrol
```

Run with `--minimized` to start hidden in the tray (this is what the startup entry uses).

## Status

Device detection, smart button handling, tray and startup toggle work. Audio processing (EQ, reverb, mic features) is not implemented yet. See docs/RE_NOTES.md for the protocol and plan.
