//! Built in presets, taken from values the official software writes to the driver.

pub struct Preset {
    pub name: &'static str,
    pub eq: [f32; 9],
    pub surround: bool,
    pub reverb: f32,
}

pub const BUILTIN: [Preset; 4] = [
    Preset { name: "FLAT", eq: [0.0; 9], surround: false, reverb: 0.0 },
    Preset {
        name: "MOVIE",
        eq: [6.0, 4.16, 1.28, 0.28, 0.0, 0.0, 0.66, 4.0, 6.0],
        surround: true,
        reverb: 0.0,
    },
    Preset {
        name: "MUSIC",
        eq: [6.0, 3.97, 0.85, -0.43, -1.04, -0.47, 0.66, 4.0, 6.0],
        surround: false,
        reverb: 0.0,
    },
    Preset {
        name: "ESPORT",
        eq: [-6.0, -3.97, -0.85, 0.85, 2.03, 4.2, 6.0, 6.0, 6.0],
        surround: true,
        reverb: 0.30,
    },
];

/// Mic EQ tables for the voice enhancer: off, warm, clear (8 bands, dB).
pub const VOICE: [[f32; 8]; 3] = [
    [0.0; 8],
    [6.0, 6.0, 4.01, -0.52, -3.02, -2.31, 1.04, -3.02],
    [-6.0, -6.0, -3.02, 0.0, 0.0, 1.04, 2.03, 0.0],
];
