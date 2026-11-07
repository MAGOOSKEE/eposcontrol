//! Detects and disables the official EPOS Gaming Suite so it cannot fight EposControl.
//!
//! `check()` is cheap and needs no admin rights. `disable()` and `restore()` need admin
//! and are run through `eposcontrol.exe --disable-official` / `--restore-official`.
//! Everything that is changed is reversible: the service is set to disabled (not removed),
//! startup shortcuts are renamed, and Run values are saved to a file before being removed.

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, ERROR_SUCCESS};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegEnumValueW, RegOpenKeyExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS, KEY_READ, KEY_WOW64_32KEY,
    KEY_WOW64_64KEY, REG_SZ,
};
use windows_service::service::{ServiceAccess, ServiceStartType, ServiceState};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

const SERVICE: &str = "EPOSGamingSuiteService";
const UI_EXE: &str = "EPOSGamingSuite.exe";
const SERVICE_EXE: &str = "EPOSGamingSuiteService.exe";
const RUN_KEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";
const DISABLED_SUFFIX: &str = ".eposcontrol-disabled";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Default, Clone)]
pub struct Status {
    pub installed: bool,
    pub ui_running: bool,
    pub service_running: bool,
    pub service_enabled: bool,
    pub startup_entries: Vec<String>,
}

impl Status {
    /// Human readable list of what is still active. Empty when the official software is out of the way.
    pub fn issues(&self) -> Vec<String> {
        let mut v = Vec::new();
        if self.ui_running {
            v.push("the app is running".to_string());
        }
        if self.service_running || self.service_enabled {
            v.push("its background service is enabled".to_string());
        }
        if !self.startup_entries.is_empty() {
            v.push("it starts with Windows".to_string());
        }
        v
    }

    pub fn summary(&self) -> String {
        if !self.installed {
            return String::new();
        }
        let issues = self.issues();
        if issues.is_empty() {
            return String::new();
        }
        let mut s = String::from("Official EPOS Gaming Suite: ");
        s.push_str(&issues.join(", "));
        s
    }
}

fn startup_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(pd) = std::env::var_os("ProgramData") {
        dirs.push(PathBuf::from(pd).join(r"Microsoft\Windows\Start Menu\Programs\StartUp"));
    }
    if let Some(ad) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(ad).join(r"Microsoft\Windows\Start Menu\Programs\Startup"));
    }
    dirs
}

fn is_epos_shortcut(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".lnk") && (n.contains("epos gaming suite") || n.contains("eposgamingsuite"))
}

fn startup_shortcuts() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for d in startup_dirs() {
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                if is_epos_shortcut(&e.file_name().to_string_lossy()) {
                    out.push(e.path());
                }
            }
        }
    }
    out
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// (name, data) of all Run values in a key and registry view.
fn run_values(root: HKEY, view: u32) -> Vec<(String, String)> {
    let mut out = Vec::new();
    unsafe {
        let mut key = HKEY::default();
        let path = wide(RUN_KEY);
        if RegOpenKeyExW(root, PCWSTR(path.as_ptr()), Some(0), KEY_READ | windows::Win32::System::Registry::REG_SAM_FLAGS(view), &mut key)
            != ERROR_SUCCESS
        {
            return out;
        }
        for i in 0.. {
            let mut name = [0u16; 512];
            let mut name_len = name.len() as u32;
            let mut data = [0u8; 2048];
            let mut data_len = data.len() as u32;
            let mut ty = 0u32;
            let r = RegEnumValueW(
                key,
                i,
                Some(windows::core::PWSTR(name.as_mut_ptr())),
                &mut name_len,
                None,
                Some(&mut ty),
                Some(data.as_mut_ptr()),
                Some(&mut data_len),
            );
            if r != ERROR_SUCCESS {
                break;
            }
            let n = String::from_utf16_lossy(&name[..name_len as usize]);
            let words: Vec<u16> = data[..data_len as usize]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            let d = String::from_utf16_lossy(&words).trim_end_matches('\0').to_string();
            out.push((n, d));
        }
        let _ = RegCloseKey(key);
    }
    out
}

fn is_epos_run_value(name: &str, data: &str) -> bool {
    let (n, d) = (name.to_ascii_lowercase(), data.to_ascii_lowercase());
    if d.contains("eposcontrol") || n.contains("eposcontrol") {
        return false; // that is us
    }
    n.contains("epos gaming suite") || n.contains("eposgamingsuite") || d.contains("eposgamingsuite")
}

/// All matching Run values as (hive name, view, name, data).
fn epos_run_values() -> Vec<(&'static str, u32, String, String)> {
    let mut out = Vec::new();
    for (hive, root, view) in [
        ("HKLM", HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY.0),
        ("HKLM", HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY.0),
        ("HKCU", HKEY_CURRENT_USER, KEY_WOW64_64KEY.0),
    ] {
        for (n, d) in run_values(root, view) {
            if is_epos_run_value(&n, &d) {
                out.push((hive, view, n, d));
            }
        }
    }
    out
}

fn process_running(exe: &str) -> bool {
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return false };
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut found = false;
        if Process32FirstW(snap, &mut entry).is_ok() {
            loop {
                let len = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(entry.szExeFile.len());
                if String::from_utf16_lossy(&entry.szExeFile[..len]).eq_ignore_ascii_case(exe) {
                    found = true;
                    break;
                }
                if Process32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
        found
    }
}

fn service_state() -> (bool, bool) {
    // (running, enabled)
    let Ok(mgr) = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT) else {
        return (false, false);
    };
    let Ok(svc) = mgr.open_service(SERVICE, ServiceAccess::QUERY_STATUS | ServiceAccess::QUERY_CONFIG) else {
        return (false, false);
    };
    let running = svc.query_status().map(|s| s.current_state != ServiceState::Stopped).unwrap_or(false);
    let enabled = svc.query_config().map(|c| c.start_type != ServiceStartType::Disabled).unwrap_or(false);
    (running, enabled)
}

fn install_dir() -> PathBuf {
    let pf = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)"));
    pf.join(r"EPOS\Gaming Suite")
}

pub fn check() -> Status {
    let (service_running, service_enabled) = service_state();
    let installed = install_dir().join(UI_EXE).exists() || service_running || service_enabled;
    let mut startup_entries: Vec<String> =
        startup_shortcuts().iter().map(|p| p.display().to_string()).collect();
    startup_entries.extend(epos_run_values().into_iter().map(|(h, _, n, _)| format!("{h} Run: {n}")));
    Status {
        installed,
        ui_running: process_running(UI_EXE),
        service_running,
        service_enabled,
        startup_entries,
    }
}

fn backup_path() -> PathBuf {
    let pd = std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
    pd.join("EposControl").join("disabled-official-run.txt")
}

fn run_hidden(program: &str, args: &[&str]) {
    let _ = std::process::Command::new(program).args(args).creation_flags(CREATE_NO_WINDOW).status();
}

/// Turn the official software off. Needs admin rights.
pub fn disable() {
    run_hidden("taskkill", &["/F", "/IM", UI_EXE]);
    run_hidden("sc", &["stop", SERVICE]);
    run_hidden("sc", &["config", SERVICE, "start=", "disabled"]);
    run_hidden("taskkill", &["/F", "/IM", SERVICE_EXE]);

    for p in startup_shortcuts() {
        let mut target = p.clone().into_os_string();
        target.push(DISABLED_SUFFIX);
        let _ = std::fs::rename(&p, PathBuf::from(target));
    }

    // Save Run values to a file, then remove them.
    let mut lines = String::new();
    for (hive, view, name, data) in epos_run_values() {
        let root = if hive == "HKLM" { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER };
        unsafe {
            let mut key = HKEY::default();
            let path = wide(RUN_KEY);
            if RegOpenKeyExW(
                root,
                PCWSTR(path.as_ptr()),
                Some(0),
                KEY_ALL_ACCESS | windows::Win32::System::Registry::REG_SAM_FLAGS(view),
                &mut key,
            ) == ERROR_SUCCESS
            {
                let n = wide(&name);
                if RegDeleteValueW(key, PCWSTR(n.as_ptr())) == ERROR_SUCCESS {
                    lines.push_str(&format!("{hive}|{view}|{name}|{data}\n"));
                }
                let _ = RegCloseKey(key);
            }
        }
    }
    if !lines.is_empty() {
        let bp = backup_path();
        if let Some(dir) = bp.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut existing = std::fs::read_to_string(&bp).unwrap_or_default();
        existing.push_str(&lines);
        let _ = std::fs::write(bp, existing);
    }
}

/// Undo `disable()`. Needs admin rights.
pub fn restore() {
    run_hidden("sc", &["config", SERVICE, "start=", "auto"]);
    run_hidden("sc", &["start", SERVICE]);

    for d in startup_dirs() {
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if let Some(orig) = name.strip_suffix(DISABLED_SUFFIX) {
                    if is_epos_shortcut(orig) {
                        let _ = std::fs::rename(e.path(), Path::new(&d).join(orig));
                    }
                }
            }
        }
    }

    let bp = backup_path();
    if let Ok(text) = std::fs::read_to_string(&bp) {
        for line in text.lines() {
            let parts: Vec<&str> = line.splitn(4, '|').collect();
            if parts.len() != 4 {
                continue;
            }
            let root = if parts[0] == "HKLM" { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER };
            let view: u32 = parts[1].parse().unwrap_or(0);
            unsafe {
                let mut key = HKEY::default();
                let path = wide(RUN_KEY);
                if RegOpenKeyExW(
                    root,
                    PCWSTR(path.as_ptr()),
                    Some(0),
                    KEY_ALL_ACCESS | windows::Win32::System::Registry::REG_SAM_FLAGS(view),
                    &mut key,
                ) == ERROR_SUCCESS
                {
                    let n = wide(parts[2]);
                    let d: Vec<u8> = wide(parts[3]).iter().flat_map(|c| c.to_le_bytes()).collect();
                    let _ = RegSetValueExW(key, PCWSTR(n.as_ptr()), Some(0), REG_SZ, Some(&d));
                    let _ = RegCloseKey(key);
                }
            }
        }
        let _ = std::fs::remove_file(bp);
    }
}
