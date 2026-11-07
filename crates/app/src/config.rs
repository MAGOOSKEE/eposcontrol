use epos_device::Led;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    /// Smart button action, index into `actions::NAMES`.
    pub button: i32,
    /// Knob light: 0 auto (blue in 2.0, red in 7.1), 1 off, 2 red, 3 blue, 4 violet.
    pub led_mode: i32,
    pub notifications: bool,
    pub eq: [f32; 9],
    /// 0..3 built in, 4.. custom, -1 for an edited curve.
    pub preset: i32,
    pub customs: Vec<(String, [f32; 9])>,
    pub surround: bool,
    pub reverb: f32,
    pub mic_eq: [f32; 9],
    pub gate: f32,
    /// Noise cancellation: 0 off, 1 low, 2 high.
    pub nc: i32,
    /// Side tone 0..1, mapped to -57..0 dB.
    pub sidetone: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            button: 0,
            led_mode: 0,
            notifications: true,
            eq: [0.0; 9],
            preset: 0,
            customs: Vec::new(),
            surround: false,
            reverb: 0.0,
            mic_eq: [0.0; 9],
            gate: 0.39,
            nc: 1,
            sidetone: 0.0,
        }
    }
}

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(base).join("EposControl").join("config.txt"))
}

fn parse_eq(s: &str) -> Option<[f32; 9]> {
    let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
    <[f32; 9]>::try_from(v).ok()
}

fn fmt_eq(eq: &[f32; 9]) -> String {
    eq.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(",")
}

impl Config {
    pub fn load() -> Self {
        let mut c = Config::default();
        let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else {
            return c;
        };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "button" => c.button = v.parse().unwrap_or(c.button).clamp(0, 2),
                "ledmode" => c.led_mode = v.parse().unwrap_or(c.led_mode),
                "notifications" => c.notifications = v != "0",
                "eq" => c.eq = parse_eq(v).unwrap_or(c.eq),
                "mic_eq" => c.mic_eq = parse_eq(v).unwrap_or(c.mic_eq),
                "preset" => c.preset = v.parse().unwrap_or(c.preset),
                "surround" => c.surround = v == "1",
                "reverb" => c.reverb = v.parse().unwrap_or(c.reverb),
                "gate" => c.gate = v.parse().unwrap_or(c.gate),
                "nc" => c.nc = v.parse().unwrap_or(c.nc).clamp(0, 2),
                "sidetone" => c.sidetone = v.parse().unwrap_or(c.sidetone),
                "custom" => {
                    if let Some((name, eq)) = v.split_once('|') {
                        if let Some(eq) = parse_eq(eq) {
                            c.customs.push((name.to_string(), eq));
                        }
                    }
                }
                _ => {}
            }
        }
        c
    }

    pub fn save(&self) {
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut out = format!(
            "button={}\nledmode={}\nnotifications={}\neq={}\nmic_eq={}\npreset={}\nsurround={}\nreverb={}\ngate={}\nnc={}\nsidetone={}\n",
            self.button,
            self.led_mode,
            self.notifications as u8,
            fmt_eq(&self.eq),
            fmt_eq(&self.mic_eq),
            self.preset,
            self.surround as u8,
            self.reverb,
            self.gate,
            self.nc,
            self.sidetone
        );
        for (n, eq) in &self.customs {
            out.push_str(&format!("custom={}|{}\n", n.replace(['|', '\n', '\r'], " "), fmt_eq(eq)));
        }
        let _ = std::fs::write(p, out);
    }

    /// The light the knob should show right now.
    pub fn effective_led(&self) -> Led {
        match self.led_mode {
            1 => Led::Off,
            2 => Led::Red,
            3 => Led::Blue,
            4 => Led::Purple,
            _ => {
                if self.surround {
                    Led::Red
                } else {
                    Led::Blue
                }
            }
        }
    }

    /// Side tone as the device value in dB (-57 off .. 0 max).
    pub fn sidetone_db(&self) -> i8 {
        (-57.0 + 57.0 * self.sidetone.clamp(0.0, 1.0)).round() as i8
    }
}
