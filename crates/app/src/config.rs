use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    /// Index into `actions::NAMES`.
    pub action: i32,
    /// Program path, URL or key combo, depending on the action.
    pub param: String,
    /// Index into `epos_device::Led::ALL`.
    pub led: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self { action: 1, param: String::new(), led: 1 }
    }
}

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(base).join("EposControl").join("config.txt"))
}

impl Config {
    pub fn load() -> Self {
        let mut c = Config::default();
        let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else {
            return c;
        };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "action" => c.action = v.trim().parse().unwrap_or(c.action),
                "led" => c.led = v.trim().parse().unwrap_or(c.led),
                "param" => c.param = v.trim().to_string(),
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
        let param = self.param.replace(['\r', '\n'], " ");
        let _ = std::fs::write(
            p,
            format!("action={}\nled={}\nparam={}\n", self.action, self.led, param),
        );
    }
}
