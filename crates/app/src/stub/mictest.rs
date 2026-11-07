//! Stand-ins for the microphone record test and live level monitor (Windows only).

pub fn wav_path() -> std::path::PathBuf {
    std::env::temp_dir().join("eposcontrol-mic-test.wav")
}

pub struct Recorder;

impl Recorder {
    pub fn start() -> Self {
        Recorder
    }
    pub fn finish(self) -> Option<f32> {
        None
    }
}

pub struct Monitor;

impl Monitor {
    pub fn start() -> Self {
        Monitor
    }
    pub fn running(&self) -> bool {
        true
    }
    pub fn take_peak(&self) -> f32 {
        0.0
    }
}
