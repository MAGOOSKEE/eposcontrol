# EposControl

A fast, light, open replacement for EPOS Gaming Suite. Target hardware: EPOS GSX 300 (USB VID 0x1395, PID 0x0098). Windows first, cross platform later.

## Goals

- Starts silently in the system tray on boot (user can disable this).
- Tiny memory and CPU footprint, instant startup, no background service bloat.
- Clean, standard layout with the Material Symbols icon set. No emojis.
- Signed installer for releases.

## Stack

Rust backend (HID and audio control) with a Tauri 2 shell and a small web UI.

## Status

Early reverse engineering. See docs/RE_NOTES.md.
