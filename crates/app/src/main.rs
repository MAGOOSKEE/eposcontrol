#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod apo;
mod audio;
mod config;
mod presets;

use apo::Apo;
use config::Config;
use epos_device::{Gsx300, Led};
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

slint::include_modules!();

type Shared = Arc<Mutex<Config>>;

enum Cmd {
    SetLed(Led),
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

fn tray_icon_image() -> Icon {
    const N: u32 = 32;
    let mut px = vec![0u8; (N * N * 4) as usize];
    let c = (N as f32 - 1.0) / 2.0;
    for y in 0..N {
        for x in 0..N {
            let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt();
            let i = ((y * N + x) * 4) as usize;
            if d <= 15.5 {
                let ring = (10.0..13.0).contains(&d) && y < 18;
                let cup = (x < 9 || x > 22) && y > 16 && y < 27 && d < 15.0;
                let (r, g, b) = if ring || cup { (255, 255, 255) } else { (30, 150, 140) };
                px[i..i + 4].copy_from_slice(&[r, g, b, 255]);
            }
        }
    }
    Icon::from_rgba(px, N, N).expect("icon")
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
        let push = |connected: bool, led: Led| {
            let ui = ui.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui.upgrade() {
                    let be = ui.global::<Backend>();
                    be.set_connected(connected);
                    be.set_led_index(led.index());
                }
            });
        };
        let current_led = || Led::from_index(cfg.lock().unwrap().led);
        let mut dev: Option<Gsx300> = None;
        loop {
            let Some(d) = dev.as_ref() else {
                match Gsx300::open() {
                    Ok(d) => {
                        let _ = d.set_led(current_led());
                        dev = Some(d);
                        push(true, current_led());
                    }
                    Err(_) => {
                        push(false, current_led());
                        let _ = rx.recv_timeout(Duration::from_secs(2));
                    }
                }
                continue;
            };
            let mut lost = false;
            while let Ok(Cmd::SetLed(l)) = rx.try_recv() {
                lost |= d.set_led(l).is_err();
            }
            match d.read_raw(250) {
                Ok(r) => {
                    if Gsx300::parse(&r).is_some() {
                        let (action, param) = {
                            let c = cfg.lock().unwrap();
                            (c.action, c.param.clone())
                        };
                        if action == actions::TOGGLE_LED {
                            let next = if current_led() == Led::Red { Led::Blue } else { Led::Red };
                            {
                                let mut c = cfg.lock().unwrap();
                                c.led = next.index();
                                c.save();
                            }
                            lost |= d.set_led(next).is_err();
                            push(true, next);
                        } else {
                            actions::run(action, &param);
                        }
                    }
                }
                Err(_) => lost = true,
            }
            if lost {
                dev = None;
                push(false, current_led());
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

fn preset_names(c: &Config) -> Vec<SharedString> {
    presets::BUILTIN
        .iter()
        .map(|p| p.name.to_string())
        .chain(c.customs.iter().map(|(n, _)| n.clone()))
        .map(SharedString::from)
        .collect()
}

fn eq_array(m: &VecModel<f32>) -> [f32; 9] {
    let mut a = [0.0; 9];
    for (i, v) in a.iter_mut().enumerate() {
        *v = slint::Model::row_data(m, i).unwrap_or(0.0);
    }
    a
}

fn main() {
    let guard = single_instance::SingleInstance::new("EposControl-single-instance").ok();
    if guard.as_ref().is_some_and(|g| !g.is_single()) {
        return;
    }
    let minimized = std::env::args().any(|a| a == "--minimized");
    audio::init_com();

    let cfg: Shared = Arc::new(Mutex::new(Config::load()));
    let ui = MainWindow::new().expect("window");
    let hw = Rc::new(Hw(RefCell::new(None)));

    // Debounced save so dragging sliders does not write the file continuously.
    let save_timer = Rc::new(Timer::default());
    let schedule_save = {
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
    let spectrum_model = Rc::new(VecModel::from(vec![-120.0f32; 15]));
    let names_model: Rc<VecModel<SharedString>> = Rc::new(VecModel::default());
    let action_names: Vec<SharedString> =
        actions::NAMES.iter().map(|s| SharedString::from(*s)).collect();

    let be = ui.global::<Backend>();
    be.set_eq(ModelRc::from(eq_model.clone()));
    be.set_spectrum(ModelRc::from(spectrum_model.clone()));
    be.set_preset_names(ModelRc::from(names_model.clone()));
    be.set_action_names(ModelRc::new(VecModel::from(action_names)));
    be.set_version(env!("CARGO_PKG_VERSION").into());
    be.set_playback_name(audio::device_name(false).unwrap_or_else(|| "Not found".into()).into());
    be.set_recording_name(audio::device_name(true).unwrap_or_else(|| "Not found".into()).into());
    be.set_gain(audio::capture_volume().unwrap_or(1.0));
    be.set_volume(audio::render_volume().unwrap_or(0.5));

    let refresh_presets = {
        let cfg = cfg.clone();
        let names = names_model.clone();
        let w = ui.as_weak();
        Rc::new(move || {
            let c = cfg.lock().unwrap();
            names.set_vec(preset_names(&c));
            if let Some(ui) = w.upgrade() {
                let be = ui.global::<Backend>();
                be.set_preset(c.preset);
                be.set_preset_label(preset_label(&c).into());
            }
        })
    };

    {
        let c = cfg.lock().unwrap();
        eq_model.set_vec(c.eq.to_vec());
        be.set_surround(c.surround);
        be.set_reverb(c.reverb);
        be.set_voice(c.voice);
        be.set_gate(c.gate);
        be.set_nc(c.nc);
        be.set_action_index(c.action);
        be.set_action_param(c.param.clone().into());
        be.set_led_index(c.led);
        be.set_param_visible(actions::needs_param(c.action));
        be.set_param_hint(actions::param_hint(c.action).into());
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
            a.set_mic_eq(&presets::VOICE[c.voice.clamp(0, 2) as usize], c.voice > 0);
        });
    }

    let al = autolaunch();
    be.set_autostart(al.as_ref().and_then(|a| a.is_enabled().ok()).unwrap_or(false));
    be.on_autostart_toggled(move |on| {
        if let Some(a) = &al {
            let _ = if on { a.enable() } else { a.disable() };
        }
    });

    // Output page.
    {
        let (cfg, hw, eq_model, save, rp) =
            (cfg.clone(), hw.clone(), eq_model.clone(), schedule_save.clone(), refresh_presets.clone());
        be.on_eq_moved(move |_, _| {
            let eq = eq_array(&eq_model);
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
        let (cfg, hw, eq_model, save, rp, w) = (
            cfg.clone(),
            hw.clone(),
            eq_model.clone(),
            schedule_save.clone(),
            refresh_presets.clone(),
            ui.as_weak(),
        );
        be.on_preset_selected(move |i| {
            let mut c = cfg.lock().unwrap();
            if (0..4).contains(&i) {
                let p = &presets::BUILTIN[i as usize];
                c.eq = p.eq;
                c.surround = p.surround;
                c.reverb = p.reverb;
            } else if let Some((_, eq)) = c.customs.get((i - 4) as usize) {
                c.eq = *eq;
            } else {
                return;
            }
            c.preset = i;
            let c2 = c.clone();
            drop(c);
            eq_model.set_vec(c2.eq.to_vec());
            if let Some(ui) = w.upgrade() {
                let be = ui.global::<Backend>();
                be.set_surround(c2.surround);
                be.set_reverb(c2.reverb);
            }
            hw.with(|a| {
                a.set_speaker_eq(&c2.eq);
                a.set_surround(c2.surround);
                a.set_reverb(c2.reverb);
            });
            rp();
            save();
        });
    }
    {
        let (cfg, save, rp) = (cfg.clone(), schedule_save.clone(), refresh_presets.clone());
        be.on_preset_add(move || {
            {
                let mut c = cfg.lock().unwrap();
                let name = format!("CUSTOM {}", c.customs.len() + 1);
                let eq = c.eq;
                c.customs.push((name, eq));
                c.preset = 3 + c.customs.len() as i32;
            }
            rp();
            save();
        });
    }
    {
        let (cfg, hw, save) = (cfg.clone(), hw.clone(), schedule_save.clone());
        be.on_surround_set(move |on| {
            cfg.lock().unwrap().surround = on;
            hw.with(|a| a.set_surround(on));
            save();
        });
    }
    {
        let (cfg, hw, save) = (cfg.clone(), hw.clone(), schedule_save.clone());
        be.on_reverb_set(move |v| {
            cfg.lock().unwrap().reverb = v;
            hw.with(|a| a.set_reverb(v));
            save();
        });
    }

    // Mic page.
    {
        let (cfg, hw, save) = (cfg.clone(), hw.clone(), schedule_save.clone());
        be.on_voice_set(move |i| {
            cfg.lock().unwrap().voice = i;
            hw.with(|a| a.set_mic_eq(&presets::VOICE[i.clamp(0, 2) as usize], i > 0));
            save();
        });
    }
    be.on_gain_set(move |v| audio::set_capture_volume(v));
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
    {
        let (hw, sm) = (hw.clone(), spectrum_model.clone());
        be.on_poll_spectrum(move || {
            if let Some(s) = hw.with(|a| a.spectrum()) {
                sm.set_vec(s.to_vec());
            }
        });
    }
    {
        let hw = hw.clone();
        be.on_page_changed(move |p| {
            hw.with(|a| a.set_meters(p == 1));
        });
    }

    // Volume popup.
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
    let (tx, rx) = mpsc::channel();
    {
        let (cfg, save) = (cfg.clone(), schedule_save.clone());
        be.on_led_selected(move |i| {
            cfg.lock().unwrap().led = i;
            let _ = tx.send(Cmd::SetLed(Led::from_index(i)));
            save();
        });
    }
    {
        let (cfg, save, w) = (cfg.clone(), schedule_save.clone(), ui.as_weak());
        be.on_action_selected(move |i| {
            cfg.lock().unwrap().action = i;
            if let Some(ui) = w.upgrade() {
                let be = ui.global::<Backend>();
                be.set_param_visible(actions::needs_param(i));
                be.set_param_hint(actions::param_hint(i).into());
            }
            save();
        });
    }
    {
        let (cfg, save) = (cfg.clone(), schedule_save.clone());
        be.on_param_edited(move |t| {
            cfg.lock().unwrap().param = t.to_string();
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
        let hw = hw.clone();
        be.on_close(move || {
            hw.with(|a| a.set_meters(false));
            if let Some(ui) = w.upgrade() {
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
    let _tray = TrayIconBuilder::new()
        .with_tooltip("EposControl")
        .with_icon(tray_icon_image())
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .expect("tray");

    if !minimized {
        ui.show().expect("show");
    }
    slint::run_event_loop_until_quit().expect("event loop");
    cfg.lock().unwrap().save();
}
