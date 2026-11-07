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

## Assets and licences

- Fonts: Barlow, Barlow Semi Condensed and IBM Plex Mono (SIL Open Font License), in `assets/fonts`.
- Icons: Material Symbols (Apache 2.0), in `assets/icons`.
- The "E" mark used for the app and tray icon was extracted from the owner's own installed copy of the official software. Keep this repository private unless the logo's usage terms have been checked.
- `assets/gsx300.svg` is a stand-in drawing. To use a photo, save it as `assets/gsx300.png`, point the `@image-url` in `crates/app/ui/main.slint` at it, and set `Art.knob-fx`, `Art.knob-fy` and `Art.ring-r` so the glow sits on the knob.

## Knob light

Auto mode shows blue in 2.0 and red in 7.1. It can also be set manually to off, red, blue or violet. The whole UI accent follows the knob colour.
