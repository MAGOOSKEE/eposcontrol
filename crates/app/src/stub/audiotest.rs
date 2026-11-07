//! Stand-ins for the built in sound test (Windows only).

pub const SOUND_TEST_SECS: f32 = 10.0;

pub fn play_sound_test() {}
pub fn stop_playback() {}
pub fn play_file(_path: &std::path::Path) {}
