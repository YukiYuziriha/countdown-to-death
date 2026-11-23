fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_state",
            "save_state",
            "get_status",
            "quit_app",
        ]),
    ))
    .expect("could not build Tauri application metadata");
}
