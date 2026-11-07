//! Microphone record test using WASAPI shared mode capture on the GSX 300 input.

use crate::audio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use windows::Win32::Media::Audio::{IAudioCaptureClient, IAudioClient, AUDCLNT_SHAREMODE_SHARED};
use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED};

pub struct Recorder {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<Option<f32>>>,
}

pub fn wav_path() -> std::path::PathBuf {
    std::env::temp_dir().join("eposcontrol-mic-test.wav")
}

fn capture(stop: Arc<AtomicBool>) -> Option<f32> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let dev = audio::capture_endpoint()?;
        let client: IAudioClient = dev.Activate(CLSCTX_ALL, None).ok()?;
        let fmt = client.GetMixFormat().ok()?;
        let channels = std::ptr::read_unaligned(std::ptr::addr_of!((*fmt).nChannels)) as usize;
        let rate = std::ptr::read_unaligned(std::ptr::addr_of!((*fmt).nSamplesPerSec));
        let bits = std::ptr::read_unaligned(std::ptr::addr_of!((*fmt).wBitsPerSample));
        client.Initialize(AUDCLNT_SHAREMODE_SHARED, 0, 10_000_000, 0, fmt, None).ok()?;
        let cap: IAudioCaptureClient = client.GetService().ok()?;
        client.Start().ok()?;
        let mut samples: Vec<f32> = Vec::new();
        while !stop.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(10));
            loop {
                let n = cap.GetNextPacketSize().unwrap_or(0);
                if n == 0 {
                    break;
                }
                let mut data: *mut u8 = std::ptr::null_mut();
                let (mut frames, mut flags) = (0u32, 0u32);
                if cap.GetBuffer(&mut data, &mut frames, &mut flags, None, None).is_err() {
                    break;
                }
                for f in 0..frames as usize {
                    let v = if flags & 2 != 0 || data.is_null() {
                        0.0 // silent packet
                    } else if bits == 32 {
                        *(data as *const f32).add(f * channels)
                    } else {
                        *(data as *const i16).add(f * channels) as f32 / 32768.0
                    };
                    samples.push(v);
                }
                let _ = cap.ReleaseBuffer(frames);
            }
        }
        let _ = client.Stop();
        CoTaskMemFree(Some(fmt as *const _));
        write_wav(&samples, rate);
        Some(samples.len() as f32 / rate as f32)
    }
}

fn write_wav(samples: &[f32], rate: u32) {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    let _ = std::fs::write(wav_path(), out);
}

impl Recorder {
    pub fn start() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let handle = std::thread::spawn(move || capture(s));
        Self { stop, handle: Some(handle) }
    }

    /// Stop recording, write the file and return the recorded length in seconds.
    pub fn finish(mut self) -> Option<f32> {
        self.stop.store(true, Ordering::Relaxed);
        self.handle.take()?.join().ok().flatten()
    }
}
