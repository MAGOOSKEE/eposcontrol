#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod audio;
mod config;

use config::Config;
use epos_device::{Gsx300, Led};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
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
                let (r, g, b) = if ring || cup { (255, 255, 255) } else { (79, 157, 255) };
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
                    ui.set_connected(connected);
                    ui.set_led_index(led.index());
                }
            });
        };
        let current_led = || Led::from_index(cfg.lock().unwrap().led);
        let mut dev: Option<Gsx300> = None;
        loop {
            let Some(d) = dev.as_ref() else {
                match Gsx300::open() {
                    Ok(d) => {
                        // Restore the saved light colour after a (re)connect.
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

fn main() {
    let guard = single_instance::SingleInstance::new("EposControl-single-instance").ok();
    if guard.as_ref().is_some_and(|g| !g.is_single()) {
        return;
    }
    let minimized = std::env::args().any(|a| a == "--minimized");

    let cfg: Shared = Arc::new(Mutex::new(Config::load()));
    let ui = MainWindow::new().expect("window");

    let names: Vec<SharedString> = actions::NAMES.iter().map(|s| SharedString::from(*s)).collect();
    ui.set_action_names(ModelRc::new(VecModel::from(names)));
    {
        let c = cfg.lock().unwrap();
        ui.set_action_index(c.action);
        ui.set_action_param(c.param.clone().into());
        ui.set_led_index(c.led);
        ui.set_param_visible(actions::needs_param(c.action));
        ui.set_param_hint(actions::param_hint(c.action).into());
    }

    let al = autolaunch();
    ui.set_autostart(al.as_ref().and_then(|a| a.is_enabled().ok()).unwrap_or(false));
    ui.on_autostart_toggled(move |on| {
        if let Some(a) = &al {
            let _ = if on { a.enable() } else { a.disable() };
        }
    });

    let (tx, rx) = mpsc::channel();
    {
        let cfg = cfg.clone();
        let ui_weak = ui.as_weak();
        ui.on_led_selected(move |i| {
            {
                let mut c = cfg.lock().unwrap();
                c.led = i;
                c.save();
            }
            let _ = tx.send(Cmd::SetLed(Led::from_index(i)));
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_led_index(i);
            }
        });
    }
    {
        let cfg = cfg.clone();
        let ui_weak = ui.as_weak();
        ui.on_action_selected(move |i| {
            {
                let mut c = cfg.lock().unwrap();
                c.action = i;
                c.save();
            }
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_action_index(i);
                ui.set_param_visible(actions::needs_param(i));
                ui.set_param_hint(actions::param_hint(i).into());
            }
        });
    }
    {
        let cfg = cfg.clone();
        ui.on_param_edited(move |t| {
            let mut c = cfg.lock().unwrap();
            c.param = t.to_string();
            c.save();
        });
    }
    spawn_device_thread(ui.as_weak(), rx, cfg);

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
}
