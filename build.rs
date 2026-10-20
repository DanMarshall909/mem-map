fn main() {
    if std::env::var_os("CARGO_FEATURE_DESKTOP").is_some() {
        let attributes = tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new().commands(&[
                "get_view",
                "dispatch",
                "copy_paths",
                "copy_snapshot",
            ]),
        );
        tauri_build::try_build(attributes).expect("build the local Tauri dashboard");
    }
}
