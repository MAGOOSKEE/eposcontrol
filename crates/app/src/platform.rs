//! Platform specific pieces: the tray icon, overlay window placement and admin elevation.
//! Only Windows has these. Other systems get no-op versions (the app is a normal window there).

#[cfg(windows)]
mod imp {
    use crate::{knob_rgb, ToastWindow};
    use epos_device::Led;
    use slint::winit_030::WinitWindowAccessor;
    use slint::ComponentHandle;
    use tray_icon::menu::{Menu, MenuEvent, MenuItem};
    use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SystemParametersInfoW, GWL_EXSTYLE, SPI_GETWORKAREA,
        SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };

    pub const HAS_TRAY: bool = true;

    pub struct Tray(TrayIcon);

    /// The EPOS mark for the tray, tinted with the current knob colour.
    fn icon(led: Led) -> Icon {
        let (r, g, b) = knob_rgb(led);
        let mut rgba = include_bytes!("../../../assets/tray-32.rgba").to_vec();
        for px in rgba.chunks_exact_mut(4) {
            px[0] = r;
            px[1] = g;
            px[2] = b;
        }
        Icon::from_rgba(rgba, 32, 32).expect("icon")
    }

    impl Tray {
        pub fn new(led: Led, show: impl Fn() + Clone + Send + Sync + 'static) -> Tray {
            let open = MenuItem::new("Open EposControl", true, None);
            let quit = MenuItem::new("Quit", true, None);
            let menu = Menu::new();
            let _ = menu.append_items(&[&open, &quit]);
            let (open_id, quit_id) = (open.id().clone(), quit.id().clone());
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
            Tray(
                TrayIconBuilder::new()
                    .with_tooltip("EposControl")
                    .with_icon(icon(led))
                    .with_menu(Box::new(menu))
                    .with_menu_on_left_click(false)
                    .build()
                    .expect("tray"),
            )
        }

        pub fn set_icon(&self, led: Led) {
            let _ = self.0.set_icon(Some(icon(led)));
        }
    }

    /// Stop the overlay from taking focus (so it never steals it from a game) and hide it from Alt+Tab.
    pub fn style_toast(win: &ToastWindow) {
        win.window().with_winit_window(|w| {
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
    }

    /// Put the overlay in the bottom left corner of the work area.
    pub fn position_toast(win: &ToastWindow) {
        let mut work = RECT::default();
        unsafe {
            let _ = SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some(&mut work as *mut RECT as *mut _),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            );
        }
        let size = win.window().size();
        let scale = win.window().scale_factor();
        let margin = (24.0 * scale) as i32;
        win.window().set_position(slint::PhysicalPosition::new(
            work.left + margin,
            work.bottom - size.height as i32 - margin,
        ));
    }

    /// Run this exe again with admin rights (UAC prompt) and wait for it to finish.
    pub fn run_elevated_and_wait(arg: &str) {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::WaitForSingleObject;
        use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
        let Ok(exe) = std::env::current_exe() else { return };
        let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
        let (file, params, verb) = (wide(&exe.to_string_lossy()), wide(arg), wide("runas"));
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            lpVerb: PCWSTR(verb.as_ptr()),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(params.as_ptr()),
            nShow: 0,
            ..Default::default()
        };
        unsafe {
            if ShellExecuteExW(&mut info).is_ok() && !info.hProcess.is_invalid() {
                WaitForSingleObject(info.hProcess, 60_000);
                let _ = CloseHandle(info.hProcess);
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use crate::ToastWindow;
    use epos_device::Led;

    pub const HAS_TRAY: bool = false;

    pub struct Tray;

    impl Tray {
        pub fn new(_led: Led, _show: impl Fn() + Clone + Send + Sync + 'static) -> Tray {
            Tray
        }
        pub fn set_icon(&self, _led: Led) {}
    }

    pub fn style_toast(_win: &ToastWindow) {}
    pub fn position_toast(_win: &ToastWindow) {}
    pub fn run_elevated_and_wait(_arg: &str) {}
}

pub use imp::*;
