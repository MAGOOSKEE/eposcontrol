#![allow(dead_code)]
//! Control of the EPOS APO audio driver through its shared memory block.
//!
//! Layout and objects are documented in docs/RE_NOTES.md. Writers take the
//! shared mutex, write, release, then signal the event so the APO reloads.

use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Memory::{
    MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_READ, FILE_MAP_WRITE,
};
use windows::Win32::System::Threading::{
    OpenEventW, OpenMutexW, ReleaseMutex, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE,
    MUTEX_MODIFY_STATE, SYNCHRONIZATION_SYNCHRONIZE,
};

const OFF_SURROUND_A: usize = 0x40;
const OFF_MIC_EQ_ON: usize = 0x42;
const OFF_GATE_ON: usize = 0x44;
const OFF_SURROUND_B: usize = 0x45;
const OFF_METERS_ON: usize = 0x46;
const OFF_SPEAKER_EQ: usize = 0x4C;
const OFF_MIC_EQ: usize = 0x70;
const OFF_REVERB: usize = 0x94;
const OFF_GATE: usize = 0x98;
const OFF_NC: usize = 0xA0;
const OFF_SPECTRUM: usize = 0x440;

pub struct Apo {
    map: HANDLE,
    view: *mut u8,
    mutex: HANDLE,
    event: HANDLE,
}

impl Apo {
    /// Open the shared objects of the GSX 300 driver (VID 1395, PID 0098).
    pub fn open() -> Option<Self> {
        unsafe {
            let map = OpenFileMappingW(
                (FILE_MAP_READ | FILE_MAP_WRITE).0,
                false,
                w!("Global\\CF4B411F-BE2B-4D84-8106-EC27CA0F8F05_1395_0098_memory"),
            )
            .ok()?;
            let view = MapViewOfFile(map, FILE_MAP_READ | FILE_MAP_WRITE, 0, 0, 0);
            if view.Value.is_null() {
                let _ = CloseHandle(map);
                return None;
            }
            let mutex = OpenMutexW(
                SYNCHRONIZATION_SYNCHRONIZE | MUTEX_MODIFY_STATE,
                false,
                w!("Global\\CF4B411F-BE2B-4D84-8106-EC27CA0F8F05_1395_0098_mutex"),
            )
            .ok()?;
            let event = OpenEventW(
                SYNCHRONIZATION_SYNCHRONIZE | EVENT_MODIFY_STATE,
                false,
                w!("Global\\CF4B411F-BE2B-4D84-8106-EC27CA0F8F05_1395_0098_event"),
            )
            .ok()?;
            Some(Self { map, view: view.Value as *mut u8, mutex, event })
        }
    }

    fn locked(&self, f: impl FnOnce(*mut u8)) {
        unsafe {
            WaitForSingleObject(self.mutex, 1000);
            f(self.view);
            let _ = ReleaseMutex(self.mutex);
            let _ = SetEvent(self.event);
        }
    }

    fn put_f32(base: *mut u8, off: usize, v: f32) {
        unsafe { std::ptr::write_volatile(base.add(off) as *mut f32, v) }
    }

    fn put_u8(base: *mut u8, off: usize, v: bool) {
        unsafe { std::ptr::write_volatile(base.add(off), v as u8) }
    }

    pub fn set_surround(&self, on: bool) {
        self.locked(|b| {
            Self::put_u8(b, OFF_SURROUND_A, on);
            Self::put_u8(b, OFF_SURROUND_B, on);
        });
    }

    pub fn set_speaker_eq(&self, gains_db: &[f32; 9]) {
        self.locked(|b| {
            for (i, g) in gains_db.iter().enumerate() {
                Self::put_f32(b, OFF_SPEAKER_EQ + i * 4, g.clamp(-6.0, 6.0));
            }
        });
    }

    pub fn set_mic_eq(&self, gains_db: &[f32; 9], enabled: bool) {
        self.locked(|b| {
            for (i, g) in gains_db.iter().enumerate() {
                Self::put_f32(b, OFF_MIC_EQ + i * 4, g.clamp(-6.0, 6.0));
            }
            Self::put_u8(b, OFF_MIC_EQ_ON, enabled);
        });
    }

    pub fn set_reverb(&self, level: f32) {
        self.locked(|b| Self::put_f32(b, OFF_REVERB, level.clamp(0.0, 1.0)));
    }

    pub fn set_noise_gate(&self, level: f32) {
        self.locked(|b| {
            Self::put_f32(b, OFF_GATE, level.clamp(0.0, 1.0));
            Self::put_u8(b, OFF_GATE_ON, level > 0.0);
        });
    }

    pub fn set_noise_cancellation(&self, preset: u32) {
        self.locked(|b| unsafe { std::ptr::write_volatile(b.add(OFF_NC) as *mut u32, preset.min(2)) });
    }

    pub fn set_meters(&self, on: bool) {
        self.locked(|b| Self::put_u8(b, OFF_METERS_ON, on));
    }

    /// Live mic spectrum in dB (15 bands), written by the APO.
    pub fn spectrum(&self) -> [f32; 15] {
        let mut out = [-120.0f32; 15];
        unsafe {
            for (i, v) in out.iter_mut().enumerate() {
                *v = std::ptr::read_volatile(self.view.add(OFF_SPECTRUM + i * 4) as *const f32);
            }
        }
        out
    }
}

impl Drop for Apo {
    fn drop(&mut self) {
        unsafe {
            let _ = UnmapViewOfFile(windows::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.view as *mut _,
            });
            let _ = CloseHandle(self.map);
            let _ = CloseHandle(self.mutex);
            let _ = CloseHandle(self.event);
        }
    }
}
