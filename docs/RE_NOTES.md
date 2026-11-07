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

The raw USB captures are not included in the repository; the findings below are what they showed.

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

## Knob lighting (confirmed by manual test)

Output report ID 4, `04 40 01 00 01 XX 00 ...`. Byte 5 (XX) is a bitmask for the knob LED:

- bit 0x20 (value 0x20): red
- bit 0x08 (value 0x08): blue
- both (0x28, and 0xFF which was tested): purple
- neither: off (0x00 tested)
- bits 0x01, 0x02, 0x04, 0x10, 0x40, 0x80 alone: no visible effect

The official app uses 0xE0 (red) and 0xC8 (blue).
Brightness, blink patterns and other bytes of the report are still unexplored.

## Official audio driver (EPOS APO) control interface

The installer ships an audio processing object (EAPO.dll, CLSID {27201018-29C6-446A-9308-AA97260FEED1}) plus a KMDF driver (eposaudio.sys). The settings are not in the registry. The official service (EPOSGamingSuiteService) writes them into a shared memory block, and the APO in audiodg listens on an event and reloads.

Objects (all in the Global namespace, non admin processes can open them read and write):

- `Global\CF4B411F-BE2B-4D84-8106-EC27CA0F8F05_1395_0098_memory` (4096 byte section)
- `Global\CF4B411F-BE2B-4D84-8106-EC27CA0F8F05_1395_0098_mutex`
- `Global\CF4B411F-BE2B-4D84-8106-EC27CA0F8F05_1395_0098_event`

The suffix is VID_PID of the device (1395, 0098 for the GSX 300). To change a setting: wait on the mutex, write, release the mutex, SetEvent. Confirmed on hardware: writing 7.1 on plus reverb at maximum was audible within a second.

### Memory layout (little endian, found by diffing 28 guided snapshots)

| Offset | Type | Meaning |
|---|---|---|
| 0x00 | u32 | 2 (version, constant) |
| 0x04 | u32 | 4 (constant) |
| 0x40 | u8 | 7.1 surround on (1) or 2.0 (0). Flips together with 0x45 |
| 0x41 | u8 | speaker EQ enabled, always 1 |
| 0x42, 0x44, 0x46 | u8 | turned on when the official mic page opens (mic EQ, mic meters), off when it closes |
| 0x43 | u8 | noise reduction enabled, always 1 |
| 0x45 | u8 | surround flag, same value as 0x40 |
| 0x4C to 0x6C | 9 x f32 | speaker EQ gain in dB, range -6 to +6, bands 64, 125, 250, 500, 1k, 2k, 4k, 8k, 16k Hz |
| 0x70 to 0x8C | 8 x f32 | mic EQ gain in dB. The voice enhancer presets are just tables written here |
| 0x94 | f32 | reverb level 0 to 1 (only audible in 7.1 mode) |
| 0x98 | f32 | noise gate level 0 to 1 (official presets use value/255) |
| 0xA0 | u32 | noise cancellation preset 0, 1 or 2 (left, middle, right dot) |
| 0xB4 | u32 | 2, never changed, probably the HRTF preset |
| 0x440 to 0x47C | 16 x f32 | live mic spectrum in dB, written by the APO, read only |

Mic gain and side tone do not touch this block. They go through the Windows endpoint volume (gain) and the device topology side tone node (the official service has a CSideToneGain class).

### Official presets (speaker EQ, dB)

- FLAT: all 0.
- MOVIE: 6, 4.16, 1.28, 0.28, 0, 0, 0.66, 4.0, 6.0 (7.1 on, voice enhancer Warm, gate 0.667)
- MUSIC: 6, 3.97, 0.85, -0.43, -1.04, -0.47, 0.66, 4.0, 6.0 (gate 0.157, reverb 0.16)
- ESPORT: -6, -3.97, -0.85, 0.85, 2.03, 4.2, 6, 6, 6 (7.1 on, reverb 0.30)

### Voice enhancer tables (mic EQ, dB, 8 bands)

- Warm: 6, 6, 4.01, -0.52, -3.02, -2.31, 1.04, -3.02
- Clear: -6, -6, -3.02, 0, 0, 1.04, 2.03, 0
- Off: all 0

### Open points

- Who creates the section when the official service is not running. Creating a Global section normally needs admin or a service. Plan: ship our own small service in the installer that owns these objects, or keep the official service installed and stopped from showing its UI.
- Meaning of the flags at 0x42, 0x44, 0x46, 0x47 and the field at 0xB4.

## Side tone (confirmed from a USB capture)

Side tone is not a Windows audio control for this device. The official service sets it with the same HID output report 4 (SET_REPORT, 39 bytes, consumer collection) used for the knob light:

`04 40 01 12 7B VV 00 ...`

- `40` is the write command, `01` the length, `12 7B` the register address (0x127B), `VV` the value.
- `VV` is a signed byte in dB: 0xC7 (-57 dB) is the minimum (side tone off, also the value at app start), 0x00 (0 dB) is the maximum. Intermediate values seen: 0xC9, 0xFD, 0xEF.
- The knob light uses the same command with register `00 01` (see above).
- Reads use command 0x20 (`04 20 01 00 <addr>`), sent in a burst at startup by the official service. Not needed for control.


## Who creates the shared memory

Tested by stopping EPOSGamingSuiteService and restarting the Windows audio service: the Global objects were gone afterwards, so the APO only opens them and the official service creates them. EposControl therefore ships its own service (`eposcontrol.exe --service`, crates/app/src/service.rs) that creates the same three objects (4096 byte section with the starting values from the official service, mutex, auto reset event) with security `D:(A;OICI;FA;;;BU)(A;OICI;FA;;;LS)(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)`.

Not yet verified on hardware: that the APO attaches to memory created by this service (event reset type is a guess).
