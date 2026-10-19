#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod ui;

#[cfg(windows)]
fn main() {
    if let Err(error) = ui::run() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MessageBoxW};
        let message: Vec<_> = format!("{error}\0").encode_utf16().collect();
        let title: Vec<_> = "Mem Map\0".encode_utf16().collect();
        // SAFETY: both strings are terminated and live throughout the call.
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                message.as_ptr(),
                title.as_ptr(),
                MB_ICONERROR,
            );
        }
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Mem Map currently requires Windows 10/11.");
}
