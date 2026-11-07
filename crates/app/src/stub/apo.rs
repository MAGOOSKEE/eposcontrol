//! Stand-in for the EPOS audio driver control on systems without that driver.
//! The driver (an audio processing object) only exists on Windows, so nothing is applied here.

pub struct Apo;

#[allow(dead_code)]
impl Apo {
    pub fn open() -> Option<Self> {
        None
    }
    pub fn set_surround(&self, _on: bool) {}
    pub fn set_speaker_eq(&self, _gains_db: &[f32; 9]) {}
    pub fn set_mic_eq(&self, _gains_db: &[f32; 9], _enabled: bool) {}
    pub fn set_reverb(&self, _level: f32) {}
    pub fn set_noise_gate(&self, _level: f32) {}
    pub fn set_noise_cancellation(&self, _preset: u32) {}
}
