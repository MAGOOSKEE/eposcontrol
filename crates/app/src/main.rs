#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod apo;
mod audio;
mod audiotest;
mod config;
mod mictest;
mod presets;
mod service;

use apo::Apo;
use config::Config;
use epos_device::{Gsx300, Led};
use slint::winit_030::WinitWindowAccessor;
use slint::{Color, ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, SystemParametersInfoW, GWL_EXSTYLE, SPI_GETWORKAREA,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

slint::include_modules!();

type Shared = Arc<Mutex<Config>>;

const MIC_TEST_MAX_SECS: f32 = 15.0;
const WAVE_POINTS: usize = 160;

enum Cmd {
    SetLed(Led),
    SetSidetone(i8),
}

/// Lazily connected handle to the audio driver block (UI thread only).
struct Hw(RefCell<Option<Apo>>);

impl Hw {
    fn with<R>(&self, f: impl FnOnce(&Apo) -> R) -> Option<R> {
        let mut a = self.0.borrow_mut();
        if a.is_none() {
            *a = Apo::open();
        }
        a.as_ref().map(f)
    }
}

fn knob_rgb(led: Led) -> (u8, u8, u8) {
    match led {
        Led::Off => (0x9a, 0xa0, 0xaa),
        Led::Red => (0xff, 0x45, 0x3a),
        Led::Blue => (0x3b, 0x8c, 0xff),
        Led::Purple => (0xb0, 0x5c, 0xff),
    }
}

fn knob_color(led: Led) -> Color {
    let (r, g, b) = knob_rgb(led);
    Color::from_rgb_u8(r, g, b)
}

/// The EPOS mark for the tray, tinted with the current knob colour.
fn tray_icon_image(led: Led) -> Icon {
    let (r, g, b) = knob_rgb(led);
    let mut rgba = include_bytes!("../../../assets/tray-32.rgba").to_vec();
    for px in rgba.chunks_exact_mut(4) {
        px[0] = r;
        px[1] = g;
        px[2] = b;
    }
    Icon::from_rgba(rgba, 32, 32).expect("icon")
}

fn fmt_time(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{:02}:{:02}", s / 60, s % 60)
}

fn autolaunch() -> Option<auto_launch::AutoLaunch> {
    let exe = std::env::current_exe().ok()?;
    auto_launch::AutoLaunchBuilder::new()
        .set_app_name("EposControl")
        .set_app_path(exe.to_str()?)
        .set_args(&["--minimized"])
        .build()
        .ok()
}

fn spawn_device_thread(ui: slint::Weak<MainWindow>, rx: mpsc::Receiver<Cmd>, cfg: Shared) {
    std::thread::spawn(move || {
        let push_connected = |connected: bool| {
            let ui = ui.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui.upgrade() {
                    ui.global::<Backend>().set_connected(connected);
                }
            });
        };
        let mut dev: Option<Gsx300> = None;
        loop {
            let Some(d) = dev.as_ref() else {
                match Gsx300::open() {
                    Ok(d) => {
                        {
                            let c = cfg.lock().unwrap();
                            let _ = d.set_led(c.effective_led());
                            let _ = d.set_sidetone_db(c.sidetone_db());
                        }
                        dev = Some(d);
                        push_connected(true);
                    }
                    Err(_) => {
                        push_connected(false);
                        let _ = rx.recv_timeout(Duration::from_secs(2));
                    }
                }
                continue;
            };
            let mut lost = false;
            while let Ok(cmd) = rx.try_recv() {
                lost |= match cmd {
                    Cmd::SetLed(l) => d.set_led(l).is_err(),
                    Cmd::SetSidetone(db) => d.set_sidetone_db(db).is_err(),
                };
            }
            match d.read_raw(250) {
                Ok(r) => {
                    if Gsx300::parse(&r).is_some() {
                        let ui = ui.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui.upgrade() {
                                ui.global::<Backend>().invoke_button_pressed();
                            }
                        });
                    }
                }
                Err(_) => lost = true,
            }
            if lost {
                dev = None;
                push_connected(false);
            }
        }
    });
}

fn preset_label(c: &Config) -> String {
    match c.preset {
        i if (0..4).contains(&i) => presets::BUILTIN[i as usize].name.to_string(),
        i if i >= 4 => c
            .customs
            .get((i - 4) as usize)
            .map(|(n, _)| n.clone())
            .unwrap_or_else(|| "CUSTOM".into()),
        _ => "CUSTOM".into(),
    }
}

fn mic_label(c: &Config) -> String {
    match c.mic_preset {
        i if (0..3).contains(&i) => presets::MIC_PROFILES[i as usize].0.to_string(),
        i if i >= 3 => c
            .mic_customs
            .get((i - 3) as usize)
            .map(|(n, _)| n.clone())
            .unwrap_or_else(|| "CUSTOM".into()),
        _ => "CUSTOM".into(),
    }
}

fn profile(name: &str, custom: bool) -> Profile {
    Profile { name: SharedString::from(name), custom }
}

fn preset_items(c: &Config) -> Vec<Profile> {
    presets::BUILTIN
        .iter()
        .map(|p| profile(p.name, false))
        .chain(c.customs.iter().map(|(n, _)| profile(n, true)))
        .collect()
}

fn mic_items(c: &Config) -> Vec<Profile> {
    presets::MIC_PROFILES
        .iter()
        .map(|(n, _)| profile(n, false))
        .chain(c.mic_customs.iter().map(|(n, _)| profile(n, true)))
        .collect()
}

fn model_array(m: &VecModel<f32>) -> [f32; 9] {
    let mut a = [0.0; 9];
    for (i, v) in a.iter_mut().enumerate() {
        *v = slint::Model::row_data(m, i).unwrap_or(0.0);
    }
    a
}

/// Overlay message in the bottom left that never takes focus from a game.
struct Toast {
    win: ToastWindow,
    hide: Timer,
    styled: Cell<bool>,
}

impl Toast {
    fn new() -> Self {
        Self { win: ToastWindow::new().expect("toast"), hide: Timer::default(), styled: Cell::new(false) }
    }

    fn show(&self, message: &str, color: Color) {
        self.win.set_message(message.into());
        self.win.set_accent(color);
        let _ = self.win.show();
        if !self.styled.get() {
            self.win.window().with_winit_window(|w| {
                use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
                if let Ok(h) = w.window_handle() {
                    if let RawWindowHandle::Win32(h32) = h.as_raw() {
                        let hwnd = HWND(h32.hwnd.get() as *mut _);
                        unsafe {
                            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                            SetWindowLongPtrW(
                                hwnd,
                                GWL_EXSTYLE,
                                ex | WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize,
                            );
                        }
                    }
                }
            });
            self.styled.set(true);
        }
        let mut work = RECT::default();
        unsafe {
            let _ = SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some(&mut work as *mut RECT as *mut _),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            );
        }
        let size = self.win.window().size();
        let scale = self.win.window().scale_factor();
        let margin = (24.0 * scale) as i32;
        self.win.window().set_position(slint::PhysicalPosition::new(
            work.left + margin,
            work.bottom - size.height as i32 - margin,
        ));
        let w = self.win.as_weak();
        self.hide.start(TimerMode::SingleShot, Duration::from_millis(2200), move || {
            if let Some(w) = w.upgrade() {
                let _ = w.hide();
            }
        });
    }
}

fn main() {
    if std::env::args().any(|a| a == "--service") {
        service::run();
        return;
    }
    let guard = single_instance::SingleInstance::new("EposControl-single-instance").ok();
    if guard.as_ref().is_some_and(|g| !g.is_single()) {
        return;
    }
    let minimized = std::env::args().any(|a| a == "--minimized");
    audio::init_com();

    let cfg: Shared = Arc::new(Mutex::new(Config::load()));
    let ui = MainWindow::new().expect("window");
    let hw = Rc::new(Hw(RefCell::new(None)));
    let toast = Rc::new(Toast::new());
    let tray: Rc<RefCell<Option<TrayIcon>>> = Rc::new(RefCell::new(None));
    let (tx, rx) = mpsc::channel::<Cmd>();

    // Debounced save so dragging sliders does not write the file continuously.
    let save_timer = Rc::new(Timer::default());
    let schedule_save: Rc<dyn Fn()> = {
        let cfg = cfg.clone();
        let t = save_timer.clone();
        Rc::new(move || {
            let cfg = cfg.clone();
            t.start(TimerMode::SingleShot, Duration::from_millis(500), move || {
                cfg.lock().unwrap().save();
            });
        })
    };

    let eq_model = Rc::new(VecModel::from(vec![0.0f32; 9]));
    let mic_model = Rc::new(VecModel::from(vec![0.0f32; 9]));
    let wave_model = Rc::new(VecModel::from(vec![0.0f32; WAVE_POINTS]));
    let preset_model: Rc<VecModel<Profile>> = Rc::new(VecModel::default());
    let mic_preset_model: Rc<VecModel<Profile>> = Rc::new(VecModel::default());
    let button_names: Vec<SharedString> =
        actions::NAMES.iter().map(|s| SharedString::from(*s)).collect();

    let be = ui.global::<Backend>();
    be.set_eq(ModelRc::from(eq_model.clone()));
    be.set_mic_eq(ModelRc::from(mic_model.clone()));
    be.set_wave(ModelRc::from(wave_model.clone()));
    be.set_preset_items(ModelRc::from(preset_model.clone()));
    be.set_mic_preset_items(ModelRc::from(mic_preset_model.clone()));
    be.set_button_names(ModelRc::new(VecModel::from(button_names)));
    be.set_version(env!("CARGO_PKG_VERSION").into());
    be.set_serial(Gsx300::serial().unwrap_or_else(|| "n/a".into()).into());
    be.set_gain(audio::capture_volume().unwrap_or(1.0));
    be.set_volume(audio::render_volume().unwrap_or(0.5));
    be.set_mic_muted(audio::capture_muted().unwrap_or(false));
    be.set_window_visible(!minimized);

    let refresh_presets = {
        let cfg = cfg.clone();
        let (items, mic_items_model) = (preset_model.clone(), mic_preset_model.clone());
        let w = ui.as_weak();
        Rc::new(move || {
            let c = cfg.lock().unwrap();
            items.set_vec(preset_items(&c));
            mic_items_model.set_vec(mic_items(&c));
            if let Some(ui) = w.upgrade() {
                let be = ui.global::<Backend>();
                be.set_preset_label(preset_label(&c).into());
                be.set_mic_preset_label(mic_label(&c).into());
            }
        })
    };

    // Push the knob light to the device and tint the UI and tray with it.
    let apply_led: Rc<dyn Fn()> = {
        let (cfg, tx, w, tray) = (cfg.clone(), tx.clone(), ui.as_weak(), tray.clone());
        Rc::new(move || {
            let led = cfg.lock().unwrap().effective_led();
            let _ = tx.send(Cmd::SetLed(led));
            if let Some(ui) = w.upgrade() {
                ui.global::<Backend>().set_knob_color(knob_color(led));
            }
            if let Some(t) = tray.borrow().as_ref() {
                let _ = t.set_icon(Some(tray_icon_image(led)));
            }
        })
    };

    // Single place that changes the audio mode: driver, UI, knob light, saved state.
    let apply_surround: Rc<dyn Fn(bool)> = {
        let (cfg, hw, w, save, led) =
            (cfg.clone(), hw.clone(), ui.as_weak(), schedule_save.clone(), apply_led.clone());
        Rc::new(move |on: bool| {
            cfg.lock().unwrap().surround = on;
            hw.with(|a| a.set_surround(on));
            if let Some(ui) = w.upgrade() {
                ui.global::<Backend>().set_surround(on);
            }
            led();
            save();
        })
    };

    // Selecting a headphone profile loads its curve and mode.
    let select_preset: Rc<dyn Fn(i32)> = {
        let (cfg, hw, eq_model, save, rp, w, surround) = (
            cfg.clone(),
            hw.clone(),
            eq_model.clone(),
            schedule_save.clone(),
            refresh_presets.clone(),
            ui.as_weak(),
            apply_surround.clone(),
        );
        Rc::new(move |i: i32| {
            let (eq, surround_on, reverb) = {
                let mut c = cfg.lock().unwrap();
                if (0..4).contains(&i) {
                    let p = &presets::BUILTIN[i as usize];
                    c.eq = p.eq;
                    c.reverb = p.reverb;
                    c.preset = i;
                    (c.eq, Some(p.surround), c.reverb)
                } else if let Some((_, eq)) = c.customs.get((i - 4) as usize).cloned() {
                    c.eq = eq;
                    c.preset = i;
                    (c.eq, None, c.reverb)
                } else {
                    return;
                }
            };
            eq_model.set_vec(eq.to_vec());
            if let Some(ui) = w.upgrade() {
                ui.global::<Backend>().set_reverb(reverb);
            }
            hw.with(|a| {
                a.set_speaker_eq(&eq);
                a.set_reverb(reverb);
            });
            if let Some(on) = surround_on {
                surround(on);
            }
            rp();
            save();
        })
    };

    {
        let c = cfg.lock().unwrap();
        eq_model.set_vec(c.eq.to_vec());
        mic_model.set_vec(c.mic_eq.to_vec());
        be.set_surround(c.surround);
        be.set_reverb(c.reverb);
        be.set_gate(c.gate);
        be.set_nc(c.nc);
        be.set_sidetone(c.sidetone);
        be.set_button_index(c.button);
        be.set_led_mode(c.led_mode);
        be.set_notifications(c.notifications);
        be.set_knob_color(knob_color(c.effective_led()));
    }
    refresh_presets();

    // Push the saved state to the driver at startup.
    {
        let c = cfg.lock().unwrap().clone();
        hw.with(|a| {
            a.set_speaker_eq(&c.eq);
            a.set_surround(c.surround);
            a.set_reverb(c.reverb);
            a.set_noise_gate(c.gate);
            a.set_noise_cancellation(c.nc as u32);
            a.set_mic_eq(&c.mic_eq, c.mic_eq.iter().any(|v| v.abs() > 0.01));
        });
    }

    let al = autolaunch();
    be.set_autostart(al.as_ref().and_then(|a| a.is_enabled().ok()).unwrap_or(false));
    be.on_autostart_toggled(move |on| {
        if let Some(a) = &al {
            let _ = if on { a.enable() } else { a.disable() };
        }
    });

    // Smart button: runs the mapped action and shows an overlay message.
    {
        let (cfg, surround, select, toast, w) =
            (cfg.clone(), apply_surround.clone(), select_preset.clone(), toast.clone(), ui.as_weak());
        be.on_button_pressed(move || {
            let (button, notify, cur_surround, preset, n) = {
                let c = cfg.lock().unwrap();
                (c.button, c.notifications, c.surround, c.preset, 4 + c.customs.len() as i32)
            };
            let accent = || knob_color(cfg.lock().unwrap().effective_led());
            match button {
                actions::TOGGLE_MODE => {
                    surround(!cur_surround);
                    if notify {
                        let m = format!("AUDIO MODE: {}", if !cur_surround { "7.1" } else { "2.0" });
                        toast.show(&m, accent());
                    }
                }
                actions::TOGGLE_PROFILE => {
                    select((preset.max(0) + 1) % n);
                    if notify {
                        let m = format!("AUDIO PROFILE: {}", preset_label(&cfg.lock().unwrap()));
                        toast.show(&m, accent());
                    }
                }
                _ => {
                    if let Some(muted) = audio::toggle_capture_mute() {
                        if let Some(ui) = w.upgrade() {
                            ui.global::<Backend>().set_mic_muted(muted);
                        }
                        if notify {
                            toast.show(if muted { "MICROPHONE: MUTED" } else { "MICROPHONE: LIVE" }, accent());
                        }
                    }
                }
            }
        });
    }

    // Home and headphones pages.
    {
        let (cfg, hw, eq_model, save, rp) =
            (cfg.clone(), hw.clone(), eq_model.clone(), schedule_save.clone(), refresh_presets.clone());
        be.on_eq_moved(move |_, _| {
            let eq = model_array(&eq_model);
            {
                let mut c = cfg.lock().unwrap();
                c.eq = eq;
                c.preset = -1;
            }
            hw.with(|a| a.set_speaker_eq(&eq));
            rp();
            save();
        });
    }
    {
        let select = select_preset.clone();
        be.on_preset_selected(move |i| select(i));
    }
    {
        let (cfg, save, rp) = (cfg.clone(), schedule_save.clone(), refresh_presets.clone());
        be.on_preset_add(move |name| {
            {
                let mut c = cfg.lock().unwrap();
                let mut name = name.trim().to_uppercase();
                if name.is_empty() {
                    name = format!("CUSTOM {}", c.customs.len() + 1);
                }
                let eq = c.eq;
                c.customs.push((name, eq));
                c.preset = 3 + c.customs.len() as i32;
            }
            rp();
            save();
        });
    }
    {
        let (cfg, save, rp) = (cfg.clone(), schedule_save.clone(), refresh_presets.clone());
        be.on_preset_deleted(move |i| {
            {
                let mut c = cfg.lock().unwrap();
                let idx = i - 4;
                if idx < 0 || idx as usize >= c.customs.len() {
                    return;
                }
                c.customs.remove(idx as usize);
                if c.preset == i {
                    c.preset = -1;
                } else if c.preset > i {
                    c.preset -= 1;
                }
            }
            rp();
            save();
        });
    }
    {
        let surround = apply_surround.clone();
        be.on_surround_set(move |on| surround(on));
    }
    {
        let (cfg, hw, save) = (cfg.clone(), hw.clone(), schedule_save.clone());
        be.on_reverb_set(move |v| {
            cfg.lock().unwrap().reverb = v;
            hw.with(|a| a.set_reverb(v));
            save();
        });
    }
    {
        let w = ui.as_weak();
        be.on_toggle_mute(move || {
            if let (Some(muted), Some(ui)) = (audio::toggle_capture_mute(), w.upgrade()) {
                ui.global::<Backend>().set_mic_muted(muted);
            }
        });
    }

    // Sound test through the default playback device.
    {
        let timer = Rc::new(Timer::default());
        let w = ui.as_weak();
        be.on_test_toggle(move || {
            let Some(ui) = w.upgrade() else { return };
            let be = ui.global::<Backend>();
            if be.get_test_playing() {
                audiotest::stop_playback();
                timer.stop();
                be.set_test_playing(false);
                be.set_test_progress(0.0);
                be.set_test_time("00:00".into());
            } else {
                audiotest::play_sound_test();
                be.set_test_playing(true);
                let start = Instant::now();
                let (w2, t2) = (w.clone(), timer.clone());
                timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
                    let Some(ui) = w2.upgrade() else { return };
                    let be = ui.global::<Backend>();
                    let e = start.elapsed().as_secs_f32();
                    if e >= audiotest::SOUND_TEST_SECS {
                        t2.stop();
                        be.set_test_playing(false);
                        be.set_test_progress(0.0);
                        be.set_test_time("00:00".into());
                    } else {
                        be.set_test_progress(e / audiotest::SOUND_TEST_SECS);
                        be.set_test_time(fmt_time(e).into());
                    }
                });
            }
        });
    }

    // Microphone page: profiles.
    let select_mic: Rc<dyn Fn(i32)> = {
        let (cfg, hw, mic_model, save, rp) =
            (cfg.clone(), hw.clone(), mic_model.clone(), schedule_save.clone(), refresh_presets.clone());
        Rc::new(move |i: i32| {
            let curve = {
                let mut c = cfg.lock().unwrap();
                let curve = if (0..3).contains(&i) {
                    presets::MIC_PROFILES[i as usize].1
                } else if let Some((_, eq)) = c.mic_customs.get((i - 3) as usize).cloned() {
                    eq
                } else {
                    return;
                };
                c.mic_eq = curve;
                c.mic_preset = i;
                curve
            };
            mic_model.set_vec(curve.to_vec());
            hw.with(|a| a.set_mic_eq(&curve, curve.iter().any(|v| v.abs() > 0.01)));
            rp();
            save();
        })
    };
    {
        let select = select_mic.clone();
        be.on_mic_preset_selected(move |i| select(i));
    }
    {
        let (cfg, save, rp) = (cfg.clone(), schedule_save.clone(), refresh_presets.clone());
        be.on_mic_preset_add(move |name| {
            {
                let mut c = cfg.lock().unwrap();
                let mut name = name.trim().to_uppercase();
                if name.is_empty() {
                    name = format!("MIC {}", c.mic_customs.len() + 1);
                }
                let eq = c.mic_eq;
                c.mic_customs.push((name, eq));
                c.mic_preset = 2 + c.mic_customs.len() as i32;
            }
            rp();
            save();
        });
    }
    {
        let (cfg, save, rp) = (cfg.clone(), schedule_save.clone(), refresh_presets.clone());
        be.on_mic_preset_deleted(move |i| {
            {
                let mut c = cfg.lock().unwrap();
                let idx = i - 3;
                if idx < 0 || idx as usize >= c.mic_customs.len() {
                    return;
                }
                c.mic_customs.remove(idx as usize);
                if c.mic_preset == i {
                    c.mic_preset = -1;
                } else if c.mic_preset > i {
                    c.mic_preset -= 1;
                }
            }
            rp();
            save();
        });
    }
    {
        let (cfg, hw, mic_model, save, rp) =
            (cfg.clone(), hw.clone(), mic_model.clone(), schedule_save.clone(), refresh_presets.clone());
        be.on_mic_eq_moved(move |_, _| {
            let curve = model_array(&mic_model);
            {
                let mut c = cfg.lock().unwrap();
                c.mic_eq = curve;
                c.mic_preset = -1;
            }
            hw.with(|a| a.set_mic_eq(&curve, curve.iter().any(|v| v.abs() > 0.01)));
            rp();
            save();
        });
    }
    be.on_gain_set(move |v| audio::set_capture_volume(v));
    {
        let (cfg, tx, save) = (cfg.clone(), tx.clone(), schedule_save.clone());
        be.on_sidetone_set(move |v| {
            let db = {
                let mut c = cfg.lock().unwrap();
                c.sidetone = v;
                c.sidetone_db()
            };
            let _ = tx.send(Cmd::SetSidetone(db));
            save();
        });
    }
    {
        let (cfg, hw, save) = (cfg.clone(), hw.clone(), schedule_save.clone());
        be.on_gate_set(move |v| {
            cfg.lock().unwrap().gate = v;
            hw.with(|a| a.set_noise_gate(v));
            save();
        });
    }
    {
        let (cfg, hw, save) = (cfg.clone(), hw.clone(), schedule_save.clone());
        be.on_nc_set(move |i| {
            cfg.lock().unwrap().nc = i;
            hw.with(|a| a.set_noise_cancellation(i as u32));
            save();
        });
    }

    // Live input level for setting the noise gate. The monitor stops itself when polling stops.
    {
        let monitor: Rc<RefCell<Option<mictest::Monitor>>> = Rc::new(RefCell::new(None));
        let ring: Rc<RefCell<Vec<f32>>> = Rc::new(RefCell::new(vec![0.0; WAVE_POINTS]));
        let wm = wave_model.clone();
        be.on_poll_wave(move || {
            let mut m = monitor.borrow_mut();
            if m.as_ref().map_or(true, |x| !x.running()) {
                *m = Some(mictest::Monitor::start());
            }
            let peak = m.as_ref().map_or(0.0, |x| x.take_peak());
            // Level on a -60 dB to 0 dB scale, 0 to 1.
            let level = if peak <= 0.000_001 { 0.0 } else { ((20.0 * peak.log10() + 60.0) / 60.0).clamp(0.0, 1.0) };
            let mut r = ring.borrow_mut();
            r.remove(0);
            r.push(level);
            wm.set_vec(r.clone());
        });
    }
    be.on_page_changed(|_| {});

    // Mic test: record from the GSX 300 input, then play back.
    {
        let timer = Rc::new(Timer::default());
        let recorder: Rc<RefCell<Option<mictest::Recorder>>> = Rc::new(RefCell::new(None));
        let rec_len = Rc::new(Cell::new(0.0f32));

        let finish = {
            let (recorder, rec_len) = (recorder.clone(), rec_len.clone());
            Rc::new(move |be: &Backend| {
                let len = recorder.borrow_mut().take().and_then(|r| r.finish()).unwrap_or(0.0);
                rec_len.set(len);
                be.set_mic_test_progress(0.0);
                be.set_mic_test_time(fmt_time(len).into());
                be.set_mic_test_state(if len > 0.3 { 2 } else { 0 });
            })
        };

        let (w, t, rec, fin) = (ui.as_weak(), timer.clone(), recorder.clone(), finish.clone());
        be.on_mic_test_record(move || {
            let Some(ui) = w.upgrade() else { return };
            let be = ui.global::<Backend>();
            *rec.borrow_mut() = Some(mictest::Recorder::start());
            be.set_mic_test_state(1);
            let start = Instant::now();
            let (w2, t2, fin2) = (w.clone(), t.clone(), fin.clone());
            t.start(TimerMode::Repeated, Duration::from_millis(50), move || {
                let Some(ui) = w2.upgrade() else { return };
                let be = ui.global::<Backend>();
                let e = start.elapsed().as_secs_f32();
                if be.get_mic_test_state() != 1 {
                    t2.stop();
                } else if e >= MIC_TEST_MAX_SECS {
                    t2.stop();
                    fin2(&be);
                } else {
                    be.set_mic_test_progress(e / MIC_TEST_MAX_SECS);
                    be.set_mic_test_time(fmt_time(e).into());
                }
            });
        });

        let (w, t, len, fin) = (ui.as_weak(), timer.clone(), rec_len.clone(), finish.clone());
        be.on_mic_test_stop(move || {
            let Some(ui) = w.upgrade() else { return };
            let be = ui.global::<Backend>();
            t.stop();
            match be.get_mic_test_state() {
                1 => fin(&be),
                3 => {
                    audiotest::stop_playback();
                    be.set_mic_test_state(2);
                    be.set_mic_test_progress(0.0);
                    be.set_mic_test_time(fmt_time(len.get()).into());
                }
                _ => {}
            }
        });

        let (w, t, len) = (ui.as_weak(), timer.clone(), rec_len.clone());
        be.on_mic_test_play(move || {
            let Some(ui) = w.upgrade() else { return };
            let be = ui.global::<Backend>();
            audiotest::play_file(&mictest::wav_path());
            be.set_mic_test_state(3);
            let start = Instant::now();
            let (w2, t2, len2) = (w.clone(), t.clone(), len.clone());
            t.start(TimerMode::Repeated, Duration::from_millis(50), move || {
                let Some(ui) = w2.upgrade() else { return };
                let be = ui.global::<Backend>();
                let e = start.elapsed().as_secs_f32();
                let total = len2.get().max(0.1);
                if e >= total || be.get_mic_test_state() != 3 {
                    t2.stop();
                    if be.get_mic_test_state() == 3 {
                        be.set_mic_test_state(2);
                    }
                    be.set_mic_test_progress(0.0);
                    be.set_mic_test_time(fmt_time(len2.get()).into());
                } else {
                    be.set_mic_test_progress((e / total).min(1.0));
                    be.set_mic_test_time(fmt_time(e).into());
                }
            });
        });
    }

    // Volume popup and home slider.
    be.on_volume_set(move |v| audio::set_render_volume(v));
    {
        let w = ui.as_weak();
        be.on_refresh_volume(move || {
            if let (Some(ui), Some(v)) = (w.upgrade(), audio::render_volume()) {
                ui.global::<Backend>().set_volume(v);
            }
        });
    }

    // Settings page.
    {
        let (cfg, save, led) = (cfg.clone(), schedule_save.clone(), apply_led.clone());
        be.on_led_mode_selected(move |i| {
            cfg.lock().unwrap().led_mode = i;
            led();
            save();
        });
    }
    {
        let (cfg, save) = (cfg.clone(), schedule_save.clone());
        be.on_notifications_toggled(move |on| {
            cfg.lock().unwrap().notifications = on;
            save();
        });
    }
    {
        let (cfg, save) = (cfg.clone(), schedule_save.clone());
        be.on_button_selected(move |i| {
            cfg.lock().unwrap().button = i;
            save();
        });
    }

    // Window chrome.
    {
        let w = ui.as_weak();
        be.on_drag_window(move || {
            if let Some(ui) = w.upgrade() {
                ui.window().with_winit_window(|ww| {
                    let _ = ww.drag_window();
                });
            }
        });
    }
    {
        let w = ui.as_weak();
        be.on_minimize(move || {
            if let Some(ui) = w.upgrade() {
                ui.window().set_minimized(true);
            }
        });
    }
    {
        let w = ui.as_weak();
        be.on_close(move || {
            if let Some(ui) = w.upgrade() {
                ui.global::<Backend>().set_window_visible(false);
                let _ = ui.hide();
            }
        });
    }
    spawn_device_thread(ui.as_weak(), rx, cfg.clone());

    // Tray.
    let open = MenuItem::new("Open EposControl", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let menu = Menu::new();
    let _ = menu.append_items(&[&open, &quit]);
    let (open_id, quit_id) = (open.id().clone(), quit.id().clone());
    let show = {
        let w = ui.as_weak();
        move || {
            let w = w.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = w.upgrade() {
                    ui.global::<Backend>().set_window_visible(true);
                    let _ = ui.show();
                }
            });
        }
    };
    {
        let show = show.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if e.id == open_id {
                show();
            } else if e.id == quit_id {
                let _ = slint::invoke_from_event_loop(|| {
                    let _ = slint::quit_event_loop();
                });
            }
        }));
    }
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = e
        {
            show();
        }
    }));
    let led_now = cfg.lock().unwrap().effective_led();
    *tray.borrow_mut() = Some(
        TrayIconBuilder::new()
            .with_tooltip("EposControl")
            .with_icon(tray_icon_image(led_now))
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()
            .expect("tray"),
    );

    if !minimized {
        ui.show().expect("show");
    }
    if std::env::args().any(|a| a == "--toast-test") {
        toast.show("AUDIO MODE: 7.1", knob_color(Led::Red));
    }
    slint::run_event_loop_until_quit().expect("event loop");
    cfg.lock().unwrap().save();
}
