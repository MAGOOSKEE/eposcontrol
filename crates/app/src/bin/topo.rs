//! Research tool: dump the audio topology of the GSX 300 endpoints to find the side tone node.

use windows::core::Interface;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eCapture, eRender, EDataFlow, IAudioVolumeLevel, IConnector, IDeviceTopology, IMMDevice,
    IMMDeviceEnumerator, IPart, MMDeviceEnumerator, DEVICE_STATE,
};
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_APARTMENTTHREADED, STGM_READ,
};

fn name_of(dev: &IMMDevice) -> String {
    unsafe {
        let store = dev.OpenPropertyStore(STGM_READ).unwrap();
        let pv = store.GetValue(&PKEY_Device_FriendlyName).unwrap();
        let p = PropVariantToStringAlloc(&pv as *const _).unwrap();
        let s = p.to_string().unwrap_or_default();
        CoTaskMemFree(Some(p.0 as *const _));
        s
    }
}

fn walk(part: &IPart, depth: usize, seen: &mut Vec<u32>, incoming: bool) {
    unsafe {
        let id = part.GetLocalId().unwrap_or(0);
        if seen.contains(&id) {
            return;
        }
        seen.push(id);
        let name = part.GetName().ok().and_then(|p| p.to_string().ok()).unwrap_or_default();
        let sub = part.GetSubType().ok();
        let vol = {
            let mut raw: *mut std::ffi::c_void = std::ptr::null_mut();
            match part.Activate(CLSCTX_ALL.0, &IAudioVolumeLevel::IID, Some(&mut raw)) {
                Ok(()) if !raw.is_null() => Some(IAudioVolumeLevel::from_raw(raw)),
                _ => None,
            }
        };
        let mut extra = String::new();
        if let Some(v) = &vol {
            let ch = v.GetChannelCount().unwrap_or(0);
            let (mut mn, mut mx, mut st) = (0f32, 0f32, 0f32);
            let _ = v.GetLevelRange(0, &mut mn, &mut mx, &mut st);
            let cur = v.GetLevel(0).unwrap_or(f32::NAN);
            extra = format!(" VOLUME ch={ch} range=[{mn},{mx}] step={st} level={cur}");
        }
        println!("{}[{id}] '{name}' subtype={:?}{extra}", "  ".repeat(depth), sub);
        let list = if incoming { part.EnumPartsIncoming() } else { part.EnumPartsOutgoing() };
        if let Ok(list) = list {
            for i in 0..list.GetCount().unwrap_or(0) {
                if let Ok(p) = list.GetPart(i) {
                    walk(&p, depth + 1, seen, incoming);
                }
            }
        }
    }
}

fn dump(en: &IMMDeviceEnumerator, flow: EDataFlow, label: &str) {
    unsafe {
        let list = en.EnumAudioEndpoints(flow, DEVICE_STATE(0xF)).unwrap();
        for i in 0..list.GetCount().unwrap() {
            let dev = list.Item(i).unwrap();
            let name = name_of(&dev);
            let st = dev.GetState().map(|s| s.0).unwrap_or(0);
            println!("endpoint '{name}' state={st:#x}");
            if !name.contains("GSX") && !name.contains("EPOS") {
                continue;
            }
            println!("=== {label}: {name}");
            if let Ok(v) = dev.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
                println!("endpoint volume scalar = {:?}", v.GetMasterVolumeLevelScalar());
            }
            let topo: IDeviceTopology = dev.Activate(CLSCTX_ALL, None).unwrap();
            for c in 0..topo.GetConnectorCount().unwrap_or(0) {
                let conn: IConnector = topo.GetConnector(c).unwrap();
                let other: IConnector = match conn.GetConnectedTo() { Ok(o) => o, Err(e) => { println!("connector {c}: GetConnectedTo failed {e}"); continue; } };
                let part: IPart = other.cast().unwrap();
                println!("-- connector {c} walking {}", if flow == eCapture { "outgoing" } else { "incoming" });
                let mut seen = Vec::new();
                walk(&part, 0, &mut seen, flow == eRender);
                // Also walk the other direction for completeness.
                let mut seen2 = Vec::new();
                println!("-- connector {c} walking opposite direction");
                walk(&part, 0, &mut seen2, flow != eRender);
            }
        }
    }
}

fn main() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).unwrap();
        dump(&en, eRender, "RENDER");
        dump(&en, eCapture, "CAPTURE");
    }
}
