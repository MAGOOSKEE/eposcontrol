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
- `assets/gsx300-top.svg` is the top view of the unit, traced from the product quick guide and recoloured. To use a photo instead, point the `@image-url` in `crates/app/ui/main.slint` at it, and set `Art.knob-fx`, `Art.knob-fy` and `Art.ring-r` so the glow sits on the knob.

## Knob light

Auto mode shows blue in 2.0 and red in 7.1. It can also be set manually to off, red, blue or violet. The whole UI accent follows the knob colour.

## Installer

`scripts\build-installer.ps1` builds the release exe and `dist\EposControl-Setup-<version>.exe` with Inno Setup 6 (`winget install JRSoftware.InnoSetup`). Pass `-OfficialMsi <path>` to also bundle the audio driver files from the official installer (private use only).

The installer:

- installs the app and an optional start with Windows entry (hidden in the tray),
- installs `EposControlHost`, a small Windows service (`eposcontrol.exe --service`) that creates the shared memory the EPOS audio driver reads, replacing the official EPOS Gaming Suite service,
- disables the official service (the uninstaller turns it back on),
- can restart the Windows audio service so the driver attaches to the new host.

The audio driver itself (EPOS APO) must be installed. The official Gaming Suite installs it, or tick the driver task when the installer was built with `-OfficialMsi`.
