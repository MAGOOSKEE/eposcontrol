//! Windows service mode (`eposcontrol.exe --service`).
//!
//! The EPOS audio driver (APO) reads its settings from a block of shared memory in the
//! Global namespace. Creating Global objects needs service privileges, so this service
//! owns the memory, mutex and event for the GSX 300 and keeps them alive. The tray app
//! then opens them and writes settings, exactly as it did with the official service.

use std::ffi::OsString;
use std::sync::mpsc;
use std::time::Duration;
use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, FILE_MAP_READ, FILE_MAP_WRITE, PAGE_READWRITE,
};
use windows::Win32::System::Threading::{CreateEventW, CreateMutexW};
use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::{define_windows_service, service_dispatcher};

pub const SERVICE_NAME: &str = "EposControlHost";

define_windows_service!(ffi_service_main, service_main);

pub fn run() {
    let _ = service_dispatcher::start(SERVICE_NAME, ffi_service_main);
}

fn status(state: ServiceState, accept: ServiceControlAccept) -> ServiceStatus {
    ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: accept,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    }
}

fn service_main(_args: Vec<OsString>) {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let handler = move |event| match event {
        ServiceControl::Stop | ServiceControl::Shutdown => {
            let _ = stop_tx.send(());
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    };
    let Ok(handle) = service_control_handler::register(SERVICE_NAME, handler) else { return };
    let _ = handle.set_service_status(status(ServiceState::Running, ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN));

    let objects = unsafe { create_objects() };
    let _ = stop_rx.recv();
    if let Some(o) = objects {
        unsafe {
            let _ = CloseHandle(o.0);
            let _ = CloseHandle(o.1);
            let _ = CloseHandle(o.2);
        }
    }
    let _ = handle.set_service_status(status(ServiceState::Stopped, ServiceControlAccept::empty()));
}

/// Create the shared memory, mutex and event the audio driver listens on.
unsafe fn create_objects() -> Option<(HANDLE, HANDLE, HANDLE)> {
    let mut psd = PSECURITY_DESCRIPTOR::default();
    ConvertStringSecurityDescriptorToSecurityDescriptorW(
        w!("D:(A;OICI;FA;;;BU)(A;OICI;FA;;;LS)(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)"),
        SDDL_REVISION_1,
        &mut psd,
        None,
    )
    .ok()?;
    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: psd.0,
        bInheritHandle: false.into(),
    };
    let map = CreateFileMappingW(
        INVALID_HANDLE_VALUE,
        Some(&sa),
        PAGE_READWRITE,
        0,
        4096,
        w!("Global\\CF4B411F-BE2B-4D84-8106-EC27CA0F8F05_1395_0098_memory"),
    )
    .ok()?;
    let view = MapViewOfFile(map, FILE_MAP_READ | FILE_MAP_WRITE, 0, 0, 4096);
    if !view.Value.is_null() {
        // Same starting state as the official service: version 2, enabled EQ and noise reduction.
        let base = view.Value as *mut u8;
        std::ptr::write_volatile(base as *mut u32, 2);
        std::ptr::write_volatile(base.add(4) as *mut u32, 4);
        std::ptr::write_volatile(base.add(0x41), 1);
        std::ptr::write_volatile(base.add(0x43), 1);
        std::ptr::write_volatile(base.add(0xB4) as *mut u32, 2);
    }
    let mutex = CreateMutexW(
        Some(&sa),
        false,
        w!("Global\\CF4B411F-BE2B-4D84-8106-EC27CA0F8F05_1395_0098_mutex"),
    )
    .ok()?;
    let event = CreateEventW(
        Some(&sa),
        false,
        false,
        w!("Global\\CF4B411F-BE2B-4D84-8106-EC27CA0F8F05_1395_0098_event"),
    )
    .ok()?;
    Some((map, mutex, event))
}
