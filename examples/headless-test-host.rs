//! Test-only NDJSON host. No Tauri runtime, window, clipboard or save dialog.
use mem_map::{
    application::{Action, Application},
    preferences::Preferences,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
    time::Instant,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: u64,
    command: String,
    args: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Dispatch {
    action: Action,
}

fn invoke(
    app: &mut Application,
    request: &Request,
    prefs_path: &std::path::Path,
) -> Result<Value, String> {
    if request.command != "dispatch" && request.args != json!({}) {
        return Err("Expected empty command arguments".into());
    }
    match request.command.as_str() {
        "get_view" => Ok(json!({"value": app.view()})),
        "dispatch" => {
            let args: Dispatch =
                serde_json::from_value(request.args.clone()).map_err(|e| e.to_string())?;
            let before = app.preferences();
            app.dispatch(args.action)?;
            let after = app.preferences();
            if before != after {
                after.save(prefs_path).map_err(|e| e.to_string())?;
            }
            Ok(json!({"value": app.view()}))
        }
        "copy_paths" => {
            let paths = app.selected_paths()?;
            Ok(
                json!({"value": format!("Copied {} executable path{}", paths.len(), if paths.len() == 1 { "" } else { "s" }),
                "effect": {"kind": "copy", "text": paths.join("\r\n")}}),
            )
        }
        "copy_snapshot" => {
            let csv = String::from_utf8(app.export_csv()?).map_err(|e| e.to_string())?;
            Ok(
                json!({"value": "Copied snapshot as CSV", "effect": {"kind": "snapshot-copy", "text": csv}}),
            )
        }
        _ => Err("Unsupported test command".into()),
    }
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The caller must supply a test-owned path; never load/save the user's preferences.
    let prefs_path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Test preference path is required")?,
    );
    let mut app = Application::new(Preferences::load(&prefs_path)?);
    let started = Instant::now();
    let mut last_capture = None;
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    for line in io::stdin().lock().lines() {
        let line = line?;
        let request: Request = serde_json::from_str(&line)?;
        let at = started.elapsed().as_secs().min(u32::MAX as u64) as u32;
        // The browser polls continuously, including during a paused presentation.
        // Sampling is demand-driven here; production's collection worker is unchanged.
        if last_capture.is_none_or(|previous| at >= previous + 2) {
            app.ingest(mem_map::sampler::capture(at)?);
            last_capture = Some(at);
        }
        let response = match invoke(&mut app, &request, &prefs_path) {
            Ok(result) => json!({"id": request.id, "result": result}),
            Err(error) => json!({"id": request.id, "error": error}),
        };
        serde_json::to_writer(&mut stdout, &response)?;
        writeln!(stdout)?;
        stdout.flush()?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("The live sampler headless test host requires Windows");
    std::process::exit(1);
}
