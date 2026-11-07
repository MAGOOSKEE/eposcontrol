# Reverse engineering notes: EPOS GSX 300

## Device

- USB composite device, VID 0x1395 (5013), PID 0x0098 (152), firmware 44.05.62.62.
- Interfaces: MI_00 USB audio (shows up as "EPOS GSX 300"), MI_03 HID.
- HID collections on MI_03:
  - COL01: Consumer Control (usage page 0x0C), input 35 bytes, output 39 bytes.
  - COL02: vendor defined (usage page 0xFFFF, usage 1), input 2 bytes, output 2 bytes.
- Firmware adapter name in EPOS config: "Boston". Chip family is Conexant (the vendor DLL is CxAudioHidDll, supports CX20562 and CX2070x/2077x/2085x/2089x/2098x/2198x).

## Official software layout (v1.12.2.1185)

- EPOSGamingSuite.exe: WPF UI. GamingSuite.UI.Services.dll: UI services.
- EngineFrameworkCpp.dll, ConfigManager.dll, AgentUtil.dll: native engine, talks to a service over a named pipe (log shows pipe open failures, one source of the flakiness).
- CxAudioHidDll32.dll: Conexant HID control (read/write input reports, EEPROM patching).
- HidDfu.dll, CM6642FWUpdateSDK.dll, blhost.dll: firmware update paths.
- Audio processing is done on the host by an APO driver (eposaudio.sys and EAPO.dll, Equalizer APO based), not in the device.
- Settings model (from preset XML): 5 band EQ, reverb, Audio3D (virtual surround), ECO mode, mic voice enhancer, mic noise gate. The smart button is mapped to SURROUND.
- State lives in C:\ProgramData\EPOS\Gaming Suite\Config and CUSTOMPRESET.

## Open questions

- Exact HID command set (report IDs, payloads) for the smart button, LEDs, and any on-device settings.
- Whether EQ can be applied without the eposaudio driver (plan: own audio processing path).

## Capture findings (USBPcap, official app v1.12.2.1185, bus 2, device address 4)

Raw summaries are in docs/captures. Guide step times are in cap2-markers.csv.

### Hardware control is tiny

Only two things cross USB between the official app and the GSX 300:

1. Button input, interrupt IN endpoint 0x84 on the HID interface (interface 3):
   - `02 01` is sent when the smart button is pressed (report ID 2, value 1).
   - `01 01` and `01 02` are volume up and down from the dial (report ID 1, consumer control). The OS handles these.
   - `01 00` is the idle release value.
2. LED or state output, HID SET_REPORT (class request 0x09, wValue 0x0204, so output report ID 4), 39 bytes:
   - `04 40 01 00 01 XX 00 ... 00` where XX is `c8` or `e0`.
   - After each smart button press the app answers within about 110 ms: first press sent `c8`, second press sent `e0`. The state alternates, so one of these is surround on and the other off (the app started with surround on, so `e0` is likely on and `c8` off). To be confirmed by sending the reports directly.
   - The app also sent report 4 when presets changed (Movie e0, Music c8, Esport e0, Flat c8), so the state is not only tied to the button.

### Everything else is host side DSP

No USB traffic at all occurred while moving the EQ bands, reverb, Audio 3D, voice enhancer, noise gate, or ECO mode. Those are applied on the PC through the eposaudio APO driver. A replacement therefore needs its own audio processing, not more HID commands.

### Windows volume

Volume and mute use the standard USB Audio class feature unit (SET_CUR to interface 5, entity 5). Windows does this itself.

### Not covered by the capture

Mic mute and the physical volume dial produced no distinct HID writes beyond the consumer events above. Firmware update and device detection paths were not exercised.

## Plan for audio DSP

Implement an Audio Processing Object (APO) in Rust, registered per endpoint like Equalizer APO does, so processing stays in the Windows audio engine with minimal latency and CPU. Fallback: WASAPI loopback capture with render to the device (simpler, more latency).
