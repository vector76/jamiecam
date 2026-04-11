pub mod commands;
pub mod dexel;
pub mod error;
pub mod feed_library;
pub mod gcode_parser;
pub mod geometry;
pub mod menu;
pub mod models;
pub mod postprocessor;
pub mod project;
pub mod state;
pub mod toolpath;

use state::AppState;
use tauri::Emitter;

/// Menu item ID constants.
const MENU_NEW_PROJECT: &str = "new-project";
const MENU_OPEN_PROJECT: &str = "open-project";
const MENU_OPEN_MODEL: &str = "open-model";
const MENU_SAVE: &str = "save";
const MENU_SAVE_AS: &str = "save-as";
const MENU_TOOL_EDITOR: &str = "tool-editor";

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
        .menu(|handle| {
            use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};

            // ── File submenu ────────────────────────────────────────────
            let new_project = MenuItemBuilder::with_id(MENU_NEW_PROJECT, "New Project")
                .accelerator("CmdOrCtrl+N")
                .build(handle)?;
            let open_project = MenuItemBuilder::with_id(MENU_OPEN_PROJECT, "Open Project")
                .accelerator("CmdOrCtrl+O")
                .build(handle)?;
            let open_model = MenuItemBuilder::with_id(MENU_OPEN_MODEL, "Open Model")
                .accelerator("CmdOrCtrl+Shift+O")
                .build(handle)?;
            let save = MenuItemBuilder::with_id(MENU_SAVE, "Save")
                .accelerator("CmdOrCtrl+S")
                .enabled(false)
                .build(handle)?;
            let save_as = MenuItemBuilder::with_id(MENU_SAVE_AS, "Save As...")
                .accelerator("CmdOrCtrl+Shift+S")
                .build(handle)?;

            let file_submenu = SubmenuBuilder::new(handle, "File")
                .item(&new_project)
                .item(&open_project)
                .item(&open_model)
                .separator()
                .item(&save)
                .item(&save_as)
                .build()?;

            // ── Tools submenu ───────────────────────────────────────────
            let tool_editor_item =
                MenuItemBuilder::with_id(MENU_TOOL_EDITOR, "Tool Editor...").build(handle)?;
            let tools_submenu = SubmenuBuilder::new(handle, "Tools")
                .item(&tool_editor_item)
                .build()?;

            MenuBuilder::new(handle)
                .item(&file_submenu)
                .item(&tools_submenu)
                .build()
        })
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            match id {
                MENU_TOOL_EDITOR => open_tool_editor_window(app),
                _ => {
                    let _ = app.emit("menu:action", id);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::file::open_model,
            commands::file::save_project,
            commands::file::save_project_current,
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
            commands::gcode_viewer::load_gcode_for_viewer,
            commands::gcode_viewer::simulate_gcode_viewer,
            commands::gcode_viewer::get_sample_gcode_path,
            commands::dexel::get_simulation_mesh,
            commands::global_tools::list_global_tools,
            commands::global_tools::add_global_tool,
            commands::global_tools::edit_global_tool,
            commands::global_tools::delete_global_tool,
            commands::global_tools::import_from_library,
            commands::global_tools::export_to_library,
            commands::twod::load_2d_file,
            commands::twod::get_2d_curves,
            commands::twod::set_safe_height,
            commands::twod::get_safe_height,
            commands::twod::set_artwork_origin,
            commands::twod::get_artwork_origin,
            commands::twod::generate_2d_gcode,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Open the tool editor window, or focus it if it already exists.
///
/// Window creation is dispatched to a new thread to avoid a deadlock on
/// Windows where WebviewWindowBuilder::build() blocks inside synchronous
/// event handlers (Webview2 limitation).
fn open_tool_editor_window(app: &tauri::AppHandle) {
    use tauri::Manager;

    if let Some(win) = app.get_webview_window("tool-editor") {
        let _ = win.set_focus();
        return;
    }

    let handle = app.clone();
    std::thread::spawn(move || {
        if let Ok(win) = tauri::WebviewWindowBuilder::new(
            &handle,
            "tool-editor",
            tauri::WebviewUrl::App("/".into()),
        )
        .title("Tool Editor")
        .inner_size(900.0, 650.0)
        .build()
        {
            let _ = win.remove_menu();
        }
    });
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
