use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    /// Index into `actions::NAMES`.
    pub action: i32,
    /// Program path, URL or key combo, depending on the action.
    pub param: String,
    /// Index into `epos_device::Led::ALL`.
    pub led: i32,
    pub eq: [f32; 9],
    /// 0..3 built in, 4.. custom, -1 for an edited curve.
    pub preset: i32,
    pub customs: Vec<(String, [f32; 9])>,
    pub surround: bool,
    pub reverb: f32,
    /// 0 off, 1 warm, 2 clear.
    pub voice: i32,
    pub gate: f32,
    pub nc: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            action: 1,
            param: String::new(),
            led: 1,
            eq: [0.0; 9],
            preset: 0,
            customs: Vec::new(),
            surround: false,
            reverb: 0.0,
            voice: 0,
            gate: 0.39,
            nc: 1,
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
                "action" => c.action = v.parse().unwrap_or(c.action),
                "led" => c.led = v.parse().unwrap_or(c.led),
                "param" => c.param = v.to_string(),
                "eq" => c.eq = parse_eq(v).unwrap_or(c.eq),
                "preset" => c.preset = v.parse().unwrap_or(c.preset),
                "surround" => c.surround = v == "1",
                "reverb" => c.reverb = v.parse().unwrap_or(c.reverb),
                "voice" => c.voice = v.parse().unwrap_or(c.voice),
                "gate" => c.gate = v.parse().unwrap_or(c.gate),
                "nc" => c.nc = v.parse().unwrap_or(c.nc),
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
            "action={}\nled={}\nparam={}\neq={}\npreset={}\nsurround={}\nreverb={}\nvoice={}\ngate={}\nnc={}\n",
            self.action,
            self.led,
            self.param.replace(['\r', '\n'], " "),
            fmt_eq(&self.eq),
            self.preset,
            self.surround as u8,
            self.reverb,
            self.voice,
            self.gate,
            self.nc
        );
        for (n, eq) in &self.customs {
            out.push_str(&format!("custom={}|{}\n", n.replace(['|', '\n', '\r'], " "), fmt_eq(eq)));
        }
        let _ = std::fs::write(p, out);
    }
}
