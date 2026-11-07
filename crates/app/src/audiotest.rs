//! Built in sound test signal. A short spatial demo for checking surround and the EQ.

use std::sync::OnceLock;
use windows::core::PCWSTR;
use windows::Win32::Media::Audio::{
    PlaySoundW, SND_ASYNC, SND_FILENAME, SND_MEMORY, SND_NODEFAULT, SND_PURGE,
};

pub const SOUND_TEST_SECS: f32 = 10.0;
const RATE: u32 = 48_000;
const TAU: f32 = std::f32::consts::TAU;

struct Mix {
    l: Vec<f32>,
    r: Vec<f32>,
    seed: u32,
}

impl Mix {
    fn noise(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / 8_388_608.0 - 1.0
    }

    /// Add a mono sample at time index `i` with a position from -1 (left) to 1 (right).
    /// The far ear gets a short delay and a duller tone, which gives a stronger sense of direction.
    fn add_pos(&mut self, i: usize, s: f32, pan: f32, lp_state: &mut [f32; 2]) {
        let a = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
        let (gl, gr) = (a.cos(), a.sin());
        let delay = (pan.abs() * 0.0007 * RATE as f32) as usize;
        let k = 0.35 + 0.65 * (1.0 - pan.abs()); // far ear low pass amount
        let (near_l, near_r) = if pan < 0.0 { (true, false) } else { (false, true) };
        for (ch, g) in [(0usize, gl), (1usize, gr)] {
            let near = if ch == 0 { near_l } else { near_r };
            let (idx, v) = if near || pan == 0.0 {
                (i, s * g)
            } else {
                lp_state[ch] += k * (s * g - lp_state[ch]);
                (i + delay, lp_state[ch])
            };
            let buf = if ch == 0 { &mut self.l } else { &mut self.r };
            if idx < buf.len() {
                buf[idx] += v;
            }
        }
    }
}

fn build_wav() -> Vec<u8> {
    let n = (RATE as f32 * SOUND_TEST_SECS) as usize;
    let mut m = Mix { l: vec![0.0; n], r: vec![0.0; n], seed: 0x9E37_79B9 };

    // 0 to 4 s: a bell phrase that moves around the listener, one note every 0.25 s.
    let notes = [392.0f32, 523.25, 659.25, 783.99, 659.25, 523.25, 440.0, 587.33];
    for k in 0..16usize {
        let start = (k as f32 * 0.25 * RATE as f32) as usize;
        let f = notes[k % notes.len()] * if k >= 8 { 1.5 } else { 1.0 };
        let pan = (k as f32 * 0.7).sin();
        let mut lp = [0.0f32; 2];
        for j in 0..(0.9 * RATE as f32) as usize {
            let t = j as f32 / RATE as f32;
            let env = (-t * 5.0).exp() * (1.0 - (-t * 400.0).exp());
            let s = ((TAU * f * t).sin() + 0.4 * (TAU * 2.0 * f * t).sin() + 0.15 * (TAU * 3.0 * f * t).sin()) * 0.16 * env;
            m.add_pos(start + j, s, pan, &mut lp);
        }
    }

    // 4 to 7 s: a fly by. Band passed noise with a rising and falling pitch, swinging across.
    let (f0, f1) = ((4.0 * RATE as f32) as usize, (7.0 * RATE as f32) as usize);
    let mut lp = [0.0f32; 2];
    let (mut b1, mut b2) = (0.0f32, 0.0f32);
    let mut ph = 0.0f32;
    for i in f0..f1 {
        let u = (i - f0) as f32 / (f1 - f0) as f32;
        let pan = (u * 2.0 - 1.0) * 0.95;
        let env = (std::f32::consts::PI * u).sin().powi(2);
        let center = 300.0 + 1700.0 * (std::f32::consts::PI * u).sin();
        let w = TAU * center / RATE as f32;
        let n = m.noise();
        b1 += w * (n - b1 - 0.4 * b2);
        b2 += w * b1;
        ph += TAU * (180.0 - 60.0 * u) / RATE as f32;
        let s = (b1 * 0.5 + ph.sin() * 0.12) * env * 0.5;
        m.add_pos(i, s, pan, &mut lp);
    }

    // 7 to 10 s: low kicks on the beat with hats panned left and right (checks bass EQ and reverb tail).
    for k in 0..6usize {
        let start = ((7.0 + k as f32 * 0.5) * RATE as f32) as usize;
        let mut lp = [0.0f32; 2];
        for j in 0..(0.45 * RATE as f32) as usize {
            let t = j as f32 / RATE as f32;
            let f = 52.0 + 90.0 * (-t * 28.0).exp();
            let s = (TAU * f * t).sin() * (-t * 7.0).exp() * 0.55;
            m.add_pos(start + j, s, 0.0, &mut lp);
        }
        let hat = start + (0.25 * RATE as f32) as usize;
        let side = if k % 2 == 0 { -0.8 } else { 0.8 };
        let mut hp = 0.0f32;
        let mut lp2 = [0.0f32; 2];
        for j in 0..(0.06 * RATE as f32) as usize {
            let t = j as f32 / RATE as f32;
            let nz = m.noise();
            let hp_out = nz - hp;
            hp += 0.5 * hp_out;
            m.add_pos(hat + j, hp_out * (-t * 70.0).exp() * 0.22, side, &mut lp2);
        }
    }

    // Master fade and conversion.
    let fade = (0.03 * RATE as f32) as usize;
    let mut pcm = Vec::with_capacity(n * 2);
    for i in 0..n {
        let g = (i.min(n - 1 - i) as f32 / fade as f32).min(1.0);
        for v in [m.l[i], m.r[i]] {
            pcm.push(((v * g).clamp(-1.0, 1.0) * 32_000.0) as i16);
        }
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

/// Play a WAV file (used for the microphone test).
pub fn play_file(path: &std::path::Path) {
    let wide: Vec<u16> = path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = PlaySoundW(PCWSTR(wide.as_ptr()), None, SND_FILENAME | SND_ASYNC | SND_NODEFAULT);
    }
}
