#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(all(windows, feature = "desktop"))]
mod desktop;

#[cfg(all(windows, feature = "desktop"))]
fn main() {
    if let Err(error) = desktop::run() {
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

#[cfg(not(all(windows, feature = "desktop")))]
fn main() {
    eprintln!("Mem Map requires Windows and the desktop feature.");
}
