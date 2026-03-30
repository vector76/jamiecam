pub mod commands;
pub mod dexel;
pub mod error;
pub mod feed_library;
pub mod gcode_parser;
pub mod geometry;
pub mod models;
pub mod postprocessor;
pub mod project;
pub mod state;
pub mod toolpath;

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
    let log_dir = dirs::data_local_dir().unwrap_or_default().join("jamiecam");

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
    let mut state = AppState::default();

    // Load the global tool library from the user's data directory.
    let global_library_path = log_dir.join("tools.json");
    let global_library = state::GlobalToolLibrary::load(&global_library_path);
    state.global_tool_library = std::sync::RwLock::new(global_library);
    state.global_library_path = global_library_path;

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
            commands::project::is_project_open,
            commands::tools::add_tool,
            commands::tools::edit_tool,
            commands::tools::delete_tool,
            commands::tools::list_tools,
            commands::stock::set_stock,
            commands::stock::get_stock,
            commands::stock::set_wcs,
            commands::stock::get_wcs,
            commands::operations::add_operation,
            commands::operations::edit_operation,
            commands::operations::delete_operation,
            commands::operations::reorder_operations,
            commands::operations::list_operations,
            commands::toolpath::list_post_processors,
            commands::toolpath::calculate_toolpath,
            commands::toolpath::get_toolpath_geometry,
            commands::toolpath::check_gouge,
            commands::toolpath::auto_lift,
            commands::toolpath::get_gcode_preview,
            commands::file::export_gcode,
            commands::geometry::get_model_faces,
            commands::geometry::detect_holes,
            commands::feeds::list_materials,
            commands::feeds::lookup_feeds,
            commands::gcode_parser::parse_gcode,
            commands::dexel::get_simulation_mesh,
            commands::global_tools::list_global_tools,
            commands::global_tools::add_global_tool,
            commands::global_tools::edit_global_tool,
            commands::global_tools::delete_global_tool,
            commands::global_tools::import_from_library,
            commands::global_tools::export_to_library,
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
        let recovered: serde_json::Value = serde_json::from_str(&serialised).expect("deserialise");
        assert_eq!(original, recovered);
    }
}
