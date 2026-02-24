/// JamieCam Tauri application library entry point.
///
/// All Tauri builder setup lives here so it can be tested and referenced
/// by the thin `main.rs` binary wrapper.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
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
