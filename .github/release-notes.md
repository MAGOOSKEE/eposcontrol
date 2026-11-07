First public release of EposControl, an unofficial, lightweight replacement for EPOS Gaming Suite.

## Supported hardware

**Only the EPOS GSX 300 is supported** (USB 1395:0098). Every control was reverse engineered from that one device.

Other EPOS and Sennheiser models that use the same EPOS audio driver might work with small changes, but they have **not been tested**. See the README for what would need to change. Reports and patches for other models are welcome.

## What is in this release

- Headphones: 9 band equalizer, 2.0 and 7.1 audio mode, reverberation, sound test, saved and custom profiles
- Microphone: 9 band equalizer, gain, side tone, noise gate with live input level, noise cancellation, record and play test, saved and custom profiles
- Smart button: toggle audio mode, toggle audio profile, or toggle mute, with an on-screen message
- Knob light: blue in 2.0 and red in 7.1, or set it by hand
- Runs hidden in the tray, starts with Windows if you want, tiny memory and CPU use
- Detects the official EPOS Gaming Suite and turns off its app, service and startup entry (reversible)

## Install

1. Download `EposControl-Setup-0.1.0.exe` below and run it as administrator.
2. The EPOS audio driver must already be installed. Installing the official EPOS Gaming Suite once does that.
3. Keep the "Turn off the official EPOS Gaming Suite" and "Restart the Windows audio service" options ticked the first time.

The installer is not code signed, so Windows SmartScreen will warn you. Choose More info, then Run anyway.

Uninstalling EposControl turns the official EPOS software back on.

## Notes

Unofficial project, not affiliated with or endorsed by EPOS or Sennheiser. It installs a small Windows service that replaces the official background service. Use at your own risk.
