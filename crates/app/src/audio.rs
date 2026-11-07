//! Windows audio endpoint helpers for the GSX 300: volume, mic gain, mic level.
//! All functions must be called from a thread that initialised COM.

use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::Endpoints::{IAudioEndpointVolume, IAudioMeterInformation};
use windows::Win32::Media::Audio::{
    eCapture, eConsole, eRender, EDataFlow, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
    DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_APARTMENTTHREADED, STGM_READ,
};
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;

pub fn init_com() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
}

fn enumerator() -> Option<IMMDeviceEnumerator> {
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok() }
}

fn friendly_name(dev: &IMMDevice) -> Option<String> {
    unsafe {
        let store = dev.OpenPropertyStore(STGM_READ).ok()?;
        let pv = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
        let p = PropVariantToStringAlloc(&pv as *const _).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        s
    }
}

/// The GSX 300 endpoint for the given direction, or the default endpoint if not found.
fn gsx_endpoint(flow: EDataFlow) -> Option<IMMDevice> {
    let en = enumerator()?;
    unsafe {
        if let Ok(list) = en.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE) {
            for i in 0..list.GetCount().unwrap_or(0) {
                if let Ok(dev) = list.Item(i) {
                    if friendly_name(&dev).is_some_and(|n| n.contains("GSX 300")) {
                        return Some(dev);
                    }
                }
            }
        }
        en.GetDefaultAudioEndpoint(flow, eConsole).ok()
    }
}

pub fn device_name(capture: bool) -> Option<String> {
    gsx_endpoint(if capture { eCapture } else { eRender }).and_then(|d| friendly_name(&d))
}

fn volume_iface(flow: EDataFlow) -> Option<IAudioEndpointVolume> {
    let dev = gsx_endpoint(flow)?;
    unsafe { dev.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None).ok() }
}

fn get(flow: EDataFlow) -> Option<f32> {
    unsafe { volume_iface(flow)?.GetMasterVolumeLevelScalar().ok() }
}

fn set(flow: EDataFlow, v: f32) {
    if let Some(vol) = volume_iface(flow) {
        unsafe {
            let _ = vol.SetMasterVolumeLevelScalar(v.clamp(0.0, 1.0), std::ptr::null());
        }
    }
}

pub fn render_volume() -> Option<f32> {
    get(eRender)
}
pub fn set_render_volume(v: f32) {
    set(eRender, v)
}
pub fn capture_volume() -> Option<f32> {
    get(eCapture)
}
pub fn set_capture_volume(v: f32) {
    set(eCapture, v)
}

pub fn capture_meter() -> Option<IAudioMeterInformation> {
    let dev = gsx_endpoint(eCapture)?;
    unsafe { dev.Activate::<IAudioMeterInformation>(CLSCTX_ALL, None).ok() }
}

pub fn peak(meter: &IAudioMeterInformation) -> Option<f32> {
    unsafe { meter.GetPeakValue().ok() }
}
