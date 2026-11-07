//! WASAPI shared mode capture on the GSX 300 input: the record and play test,
//! and the live level monitor used to set the noise gate.

use crate::audio;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows::Win32::Media::Audio::{IAudioCaptureClient, IAudioClient, AUDCLNT_SHAREMODE_SHARED};
use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED};

pub fn wav_path() -> std::path::PathBuf {
    std::env::temp_dir().join("eposcontrol-mic-test.wav")
}

/// Capture mono samples until `stop` is set. Calls `on_samples` for every packet.
/// Returns the sample rate, or None if the device could not be opened.
fn capture_loop(stop: &AtomicBool, mut on_samples: impl FnMut(&[f32])) -> Option<u32> {
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
        let mut buf: Vec<f32> = Vec::new();
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
                buf.clear();
                for f in 0..frames as usize {
                    let v = if flags & 2 != 0 || data.is_null() {
                        0.0 // silent packet
                    } else if bits == 32 {
                        *(data as *const f32).add(f * channels)
                    } else {
                        *(data as *const i16).add(f * channels) as f32 / 32768.0
                    };
                    buf.push(v);
                }
                on_samples(&buf);
                let _ = cap.ReleaseBuffer(frames);
            }
        }
        let _ = client.Stop();
        CoTaskMemFree(Some(fmt as *const _));
        Some(rate)
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

pub struct Recorder {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<Option<f32>>>,
}

impl Recorder {
    pub fn start() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let handle = std::thread::spawn(move || {
            let mut samples: Vec<f32> = Vec::new();
            let rate = capture_loop(&s, |b| samples.extend_from_slice(b))?;
            write_wav(&samples, rate);
            Some(samples.len() as f32 / rate as f32)
        });
        Self { stop, handle: Some(handle) }
    }

    /// Stop recording, write the file and return the recorded length in seconds.
    pub fn finish(mut self) -> Option<f32> {
        self.stop.store(true, Ordering::Relaxed);
        self.handle.take()?.join().ok().flatten()
    }
}

/// Live input level. Runs only while it is being polled, then stops itself.
pub struct Monitor {
    peak: Arc<AtomicU32>,
    last_poll: Arc<std::sync::Mutex<Instant>>,
    handle: JoinHandle<()>,
}

impl Monitor {
    pub fn start() -> Self {
        let peak = Arc::new(AtomicU32::new(0));
        let last_poll = Arc::new(std::sync::Mutex::new(Instant::now()));
        let (p, l) = (peak.clone(), last_poll.clone());
        let handle = std::thread::spawn(move || {
            let idle = AtomicBool::new(false);
            let _ = capture_loop(&idle, |b| {
                let m = b.iter().fold(0.0f32, |a, v| a.max(v.abs()));
                p.fetch_max(m.to_bits(), Ordering::Relaxed);
                if l.lock().unwrap().elapsed() > Duration::from_millis(1500) {
                    idle.store(true, Ordering::Relaxed);
                }
            });
        });
        Self { peak, last_poll, handle }
    }

    pub fn running(&self) -> bool {
        !self.handle.is_finished()
    }

    /// Largest absolute sample since the last call (0 to 1).
    pub fn take_peak(&self) -> f32 {
        *self.last_poll.lock().unwrap() = Instant::now();
        f32::from_bits(self.peak.swap(0, Ordering::Relaxed))
    }
}
