//! Narrow desktop outcomes for the bundled dashboard. No arbitrary file, shell,
//! path or clipboard payload is accepted from the WebView.
use mem_map::{
    application::{Action, Application, View},
    model::Snapshot,
    preferences::{self, Preferences},
};
use std::{
    io,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{State, WebviewWindow, WebviewWindowBuilder};
use windows_sys::Win32::{
    Foundation::{GlobalFree, HWND},
    System::{
        DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
        Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
    },
};

struct Desktop {
    application: Arc<Mutex<Application>>,
    preferences_path: Option<PathBuf>,
}
impl Desktop {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Application>, String> {
        self.application
            .lock()
            .map_err(|_| "Dashboard state is unavailable".into())
    }
}

fn local_url(url: &tauri::Url) -> bool {
    url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && (url.scheme() == "tauri" && url.host_str() == Some("localhost")
            || url.scheme() == "http" && url.host_str() == Some("tauri.localhost"))
}
fn authorize(window: &WebviewWindow) -> Result<HWND, String> {
    if window.label() != "main" || !local_url(&window.url().map_err(|e| e.to_string())?) {
        return Err("Only the bundled dashboard can use desktop actions".into());
    }
    window
        .hwnd()
        .map(|h| h.0 as HWND)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_view(window: WebviewWindow, state: State<'_, Desktop>) -> Result<View, String> {
    authorize(&window)?;
    Ok(state.lock()?.view())
}
#[tauri::command]
fn dispatch(
    window: WebviewWindow,
    state: State<'_, Desktop>,
    action: Action,
) -> Result<View, String> {
    authorize(&window)?;
    let mut app = state.lock()?;
    let before = app.preferences();
    app.dispatch(action)?;
    let after = app.preferences();
    if after != before
        && let Some(path) = &state.preferences_path
    {
        after
            .save(path)
            .map_err(|e| format!("Settings applied, but could not be saved: {e}"))?;
    }
    Ok(app.view())
}
#[tauri::command]
fn copy_paths(window: WebviewWindow, state: State<'_, Desktop>) -> Result<String, String> {
    let hwnd = authorize(&window)?;
    let paths = state.lock()?.selected_paths()?;
    copy_clipboard(hwnd, &paths.join("\r\n")).map_err(|e| e.to_string())?;
    Ok(format!(
        "Copied {} executable path{}",
        paths.len(),
        if paths.len() == 1 { "" } else { "s" }
    ))
}
#[tauri::command]
fn copy_snapshot(window: WebviewWindow, state: State<'_, Desktop>) -> Result<String, String> {
    let hwnd = authorize(&window)?;
    // Capture the displayed, filtered snapshot under the application lock.
    // No clipboard payload is accepted from the WebView.
    let csv = String::from_utf8(state.lock()?.export_csv()?).map_err(|e| e.to_string())?;
    copy_clipboard(hwnd, &csv).map_err(|e| format!("Copy failed: {e}"))?;
    Ok("Copied snapshot as CSV".into())
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn copy_clipboard(hwnd: HWND, text: &str) -> io::Result<()> {
    // SAFETY: live owned window, movable allocation and terminated UTF-16.
    // Windows takes ownership only after successful SetClipboardData.
    unsafe {
        let mut opened = false;
        for _ in 0..5 {
            if OpenClipboard(hwnd) != 0 {
                opened = true;
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        if !opened {
            return Err(io::Error::last_os_error());
        }
        let result = (|| {
            let encoded = wide(text);
            let memory = GlobalAlloc(GMEM_MOVEABLE, encoded.len() * 2);
            if memory.is_null() {
                return Err(io::Error::last_os_error());
            }
            let target = GlobalLock(memory);
            if target.is_null() {
                GlobalFree(memory);
                return Err(io::Error::last_os_error());
            }
            std::ptr::copy_nonoverlapping(encoded.as_ptr(), target.cast(), encoded.len());
            GlobalUnlock(memory);
            if EmptyClipboard() == 0 || SetClipboardData(13, memory).is_null() {
                let error = io::Error::last_os_error();
                GlobalFree(memory);
                return Err(error);
            }
            Ok(())
        })();
        CloseClipboard();
        result
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let preferences_path = std::env::var_os("MEMMAP_PREFERENCES_PATH")
        .map(PathBuf::from)
        .or_else(preferences::preferences_path);
    let prefs = preferences_path
        .as_deref()
        .map(Preferences::load)
        .transpose()?
        .unwrap_or_default();
    let application = Arc::new(Mutex::new(Application::new(prefs)));
    let data = application.clone();
    let (stop, stopped) = mpsc::channel::<()>();
    let collector = thread::Builder::new()
        .name("mem-map-collector".into())
        .spawn(move || {
            struct Health(Arc<Mutex<Application>>);
            impl Drop for Health {
                fn drop(&mut self) {
                    if let Ok(mut app) = self.0.lock() {
                        app.mark_collector_stopped();
                    }
                }
            }
            let _health = Health(data.clone());
            let started = Instant::now();
            loop {
                let at = started.elapsed().as_secs().min(u32::MAX as u64) as u32;
                let snapshot = mem_map::sampler::capture(at).unwrap_or_else(|error| Snapshot {
                    at,
                    complete_scan: false,
                    collection_error: Some(error.to_string()),
                    system_error: Some(error.to_string()),
                    captured_unix_ms: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                    ..Snapshot::default()
                });
                let Ok(mut app) = data.lock() else {
                    break;
                };
                app.ingest(snapshot);
                drop(app);
                match stopped.recv_timeout(Duration::from_secs(2)) {
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    _ => break,
                }
            }
        })?;
    let result = tauri::Builder::default()
        .manage(Desktop {
            application,
            preferences_path,
        })
        .invoke_handler(tauri::generate_handler![
            get_view,
            dispatch,
            copy_paths,
            copy_snapshot
        ])
        .setup(|app| {
            let mut builder = WebviewWindowBuilder::from_config(app, &app.config().app.windows[0])?
                .on_navigation(local_url);
            if let Some(path) = std::env::var_os("MEMMAP_WEBVIEW_DATA_DIR") {
                builder = builder.data_directory(PathBuf::from(path));
            }
            builder.build()?;
            Ok(())
        })
        .run(tauri::generate_context!());
    drop(stop);
    collector
        .join()
        .map_err(|_| io::Error::other("Collection worker stopped unexpectedly"))?;
    result?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::local_url;
    #[test]
    fn desktop_actions_accept_only_the_exact_bundled_origin() {
        for allowed in [
            "tauri://localhost/index.html",
            "http://tauri.localhost/index.html",
        ] {
            assert!(local_url(&tauri::Url::parse(allowed).unwrap()), "{allowed}");
        }
        for denied in [
            "http://tauri.localhost:3123/index.html",
            "http://user@tauri.localhost/index.html",
            "https://tauri.localhost/index.html",
            "http://localhost/index.html",
            "https://example.com/",
            "file:///C:/index.html",
            "tauri://other/index.html",
        ] {
            assert!(!local_url(&tauri::Url::parse(denied).unwrap()), "{denied}");
        }
    }
}
