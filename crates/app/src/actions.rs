//! Actions the smart button can be mapped to.

use std::os::windows::process::CommandExt;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eCapture, eCommunications, IMMDeviceEnumerator, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY,
};

pub const NAMES: [&str; 9] = [
    "Do nothing",
    "Toggle 2.0 and 7.1",
    "Media play or pause",
    "Next track",
    "Previous track",
    "Mute speakers",
    "Toggle microphone mute",
    "Launch program or open link",
    "Press keyboard shortcut",
];

pub const TOGGLE_SURROUND: i32 = 1;

/// True when the action needs a text parameter.
pub fn needs_param(action: i32) -> bool {
    action >= 7
}

pub fn param_hint(action: i32) -> &'static str {
    match action {
        7 => "Program path or link, for example C:\\Tools\\app.exe or https://example.com",
        8 => "Shortcut, for example ctrl+alt+m or win+shift+s or f13",
        _ => "",
    }
}

/// Run every action except the LED toggle, which the device thread handles.
pub fn run(action: i32, param: &str) {
    match action {
        2 => tap(&[0xB3]),
        3 => tap(&[0xB0]),
        4 => tap(&[0xB1]),
        5 => tap(&[0xAD]),
        6 => toggle_mic_mute(),
        7 => launch(param),
        8 => {
            if let Some(keys) = parse_combo(param) {
                tap(&keys);
            }
        }
        _ => {}
    }
}

fn key_input(vk: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Press all keys in order, then release them in reverse order.
fn tap(keys: &[u16]) {
    let mut inputs: Vec<INPUT> = keys.iter().map(|k| key_input(*k, false)).collect();
    inputs.extend(keys.iter().rev().map(|k| key_input(*k, true)));
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn launch(target: &str) {
    let target = target.trim();
    if target.is_empty() {
        return;
    }
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", target])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

fn toggle_mic_mute() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let Ok(en) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
        else {
            return;
        };
        let Ok(dev) = en.GetDefaultAudioEndpoint(eCapture, eCommunications) else { return };
        let Ok(vol) = dev.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) else { return };
        if let Ok(muted) = vol.GetMute() {
            let _ = vol.SetMute(!muted.as_bool(), std::ptr::null());
        }
    }
}

/// Parse `ctrl+alt+m` style shortcuts into virtual key codes (modifiers first).
fn parse_combo(s: &str) -> Option<Vec<u16>> {
    let mut mods = Vec::new();
    let mut key = None;
    for tok in s.split('+').map(|t| t.trim().to_ascii_lowercase()) {
        let vk = match tok.as_str() {
            "ctrl" | "control" => {
                mods.push(0x11);
                continue;
            }
            "alt" => {
                mods.push(0x12);
                continue;
            }
            "shift" => {
                mods.push(0x10);
                continue;
            }
            "win" | "meta" | "super" => {
                mods.push(0x5B);
                continue;
            }
            "space" => 0x20,
            "enter" | "return" => 0x0D,
            "tab" => 0x09,
            "esc" | "escape" => 0x1B,
            "backspace" => 0x08,
            "delete" | "del" => 0x2E,
            "insert" | "ins" => 0x2D,
            "home" => 0x24,
            "end" => 0x23,
            "pageup" | "pgup" => 0x21,
            "pagedown" | "pgdn" => 0x22,
            "left" => 0x25,
            "up" => 0x26,
            "right" => 0x27,
            "down" => 0x28,
            t if t.len() == 1 && t.as_bytes()[0].is_ascii_alphanumeric() => {
                t.as_bytes()[0].to_ascii_uppercase() as u16
            }
            t if t.starts_with('f') => match t[1..].parse::<u16>() {
                Ok(n) if (1..=24).contains(&n) => 0x70 + n - 1,
                _ => return None,
            },
            _ => return None,
        };
        key = Some(vk);
    }
    let mut keys = mods;
    keys.push(key?);
    Some(keys)
}
