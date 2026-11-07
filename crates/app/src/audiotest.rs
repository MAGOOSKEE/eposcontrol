//! Built in sound test signal and a simple microphone record and play test.

use std::sync::OnceLock;
use windows::core::{w, PCWSTR};
use windows::Win32::Media::Audio::{
    PlaySoundW, SND_ASYNC, SND_FILENAME, SND_MEMORY, SND_NODEFAULT, SND_PURGE,
};
use windows::Win32::Media::Multimedia::mciSendStringW;

pub const SOUND_TEST_SECS: f32 = 8.0;
pub const MIC_TEST_MAX_SECS: f32 = 15.0;

const RATE: u32 = 44_100;

fn build_wav() -> Vec<u8> {
    let n = (RATE as f32 * SOUND_TEST_SECS) as usize;
    let mut pcm: Vec<i16> = Vec::with_capacity(n * 2);
    let mut seed: u32 = 0x1234_5678;
    let mut lp = 0.0f32;
    let sweep_secs = 4.0f32;
    let mut phase = 0.0f32;
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let (mut l, mut r) = (0.0f32, 0.0f32);
        if t < sweep_secs {
            // Exponential sweep 80 Hz to 12 kHz, panned left to right and back.
            let f = 80.0 * (12_000.0f32 / 80.0).powf(t / sweep_secs);
            phase += std::f32::consts::TAU * f / RATE as f32;
            let pan = 0.5 + 0.5 * (std::f32::consts::TAU * t / sweep_secs).sin();
            let fade = (t / 0.05).min(1.0) * ((sweep_secs - t) / 0.05).min(1.0);
            let s = phase.sin() * 0.25 * fade;
            l = s * (1.0 - pan);
            r = s * pan;
        } else {
            // Short soft noise bursts alternating left and right (shows surround and reverb).
            let tt = t - sweep_secs;
            let slot = (tt / 0.5) as usize;
            let within = tt - slot as f32 * 0.5;
            if within < 0.12 {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let noise = (seed >> 8) as f32 / 8_388_608.0 - 1.0;
                lp += 0.25 * (noise - lp);
                let env = (within / 0.01).min(1.0) * ((0.12 - within) / 0.03).min(1.0);
                let s = lp * 0.9 * env;
                if slot % 2 == 0 {
                    l = s;
                } else {
                    r = s;
                }
            }
        }
        pcm.push((l * 32_767.0) as i16);
        pcm.push((r * 32_767.0) as i16);
    }
    let data_len = (pcm.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 4).to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

static WAV: OnceLock<Vec<u8>> = OnceLock::new();

/// Start the sound test through the default playback device.
pub fn play_sound_test() {
    let wav = WAV.get_or_init(build_wav);
    unsafe {
        let _ = PlaySoundW(PCWSTR(wav.as_ptr() as *const u16), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
    }
}

pub fn stop_playback() {
    unsafe {
        let _ = PlaySoundW(PCWSTR::null(), None, SND_PURGE);
    }
}

fn mci(cmd: &str) {
    let wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = mciSendStringW(PCWSTR(wide.as_ptr()), None, None);
    }
}

fn mic_path() -> std::path::PathBuf {
    std::env::temp_dir().join("eposcontrol-mic-test.wav")
}

/// Start recording from the default input device.
pub fn mic_record_start() {
    mci("close mictest");
    mci("open new type waveaudio alias mictest");
    mci("set mictest time format ms bitspersample 16 channels 1 samplespersec 44100");
    mci("record mictest");
}

pub fn mic_record_stop() {
    mci("stop mictest");
    mci(&format!("save mictest \"{}\"", mic_path().display()));
    mci("close mictest");
}

pub fn mic_play() {
    let wide: Vec<u16> = mic_path().to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = PlaySoundW(PCWSTR(wide.as_ptr()), None, SND_FILENAME | SND_ASYNC | SND_NODEFAULT);
    }
    let _ = w!("");
}
