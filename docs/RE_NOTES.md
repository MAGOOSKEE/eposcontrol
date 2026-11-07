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
