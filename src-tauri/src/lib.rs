pub mod commands;
pub mod error;
pub mod geometry;
pub mod project;
pub mod state;

use state::AppState;

/// JamieCam Tauri application library entry point.
///
/// All Tauri builder setup lives here so it can be tested and referenced
/// by the thin `main.rs` binary wrapper.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // ── Tracing setup (must happen before anything else) ────────────────────
    //
    // Logs are written to a rolling-never (single) file in the OS data dir:
    //   Linux    ~/.local/share/jamiecam/jamiecam.log
    //   macOS    ~/Library/Application Support/jamiecam/jamiecam.log
    //   Windows  %LOCALAPPDATA%\jamiecam\jamiecam.log
    //
    // Log level is controlled by the RUST_LOG environment variable;
    // defaults to INFO when the variable is absent.
    let log_dir = dirs::data_local_dir()
        .unwrap_or_default()
        .join("jamiecam");

    // Ensure the log directory exists before handing it to the appender.
    // tracing_appender::rolling::never panics if it cannot open the log file,
    // so we create the directory tree first.  Failure is silently ignored —
    // on systems where the directory cannot be created the appender will still
    // attempt to open the file and will panic, but that scenario (unwritable
    // home directory) is already a fatal environment misconfiguration.
    let _ = std::fs::create_dir_all(&log_dir);

    let file_appender = tracing_appender::rolling::never(&log_dir, "jamiecam.log");
    let (non_blocking, _tracing_guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(non_blocking)
        .init();

    tracing::info!("JamieCam starting");

    // ── Application state ────────────────────────────────────────────────────
    let state = AppState::default();

    // ── Tauri builder ────────────────────────────────────────────────────────
    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            commands::file::open_model,
            commands::file::save_project,
            commands::file::load_project,
            commands::file::new_project,
            commands::project::get_project_snapshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    /// Sanity check: the library compiles and basic arithmetic works.
    #[test]
    fn sanity() {
        assert_eq!(2 + 2, 4);
    }

    /// Verify that serde serialisation round-trips a simple value.
    #[test]
    fn serde_round_trip() {
        let original = serde_json::json!({ "name": "JamieCam", "version": 1 });
        let serialised = serde_json::to_string(&original).expect("serialise");
        let recovered: serde_json::Value =
            serde_json::from_str(&serialised).expect("deserialise");
        assert_eq!(original, recovered);
    }
}
