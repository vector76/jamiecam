//! IPC commands for the feed/speed library.

use crate::error::AppError;
use crate::feed_library::{FeedEntry, FeedLibrary, MaterialMeta};
use crate::state::AppState;

// ── Inner functions ───────────────────────────────────────────────────────────

fn list_materials_inner(feed_library: &FeedLibrary) -> Vec<MaterialMeta> {
    feed_library.materials().to_vec()
}

fn lookup_feeds_inner(
    material_id: &str,
    tool_material: &str,
    operation_category: &str,
    feed_library: &FeedLibrary,
) -> Result<FeedEntry, AppError> {
    feed_library
        .lookup(material_id, tool_material, operation_category)
        .cloned()
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Return the list of workpiece materials in the bundled feed library.
#[tauri::command]
pub async fn list_materials(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<MaterialMeta>, AppError> {
    Ok(list_materials_inner(&state.feed_library))
}

/// Look up feed/speed parameters for a material × tool_material × operation combination.
#[tauri::command]
pub async fn lookup_feeds(
    material_id: String,
    tool_material: String,
    operation_category: String,
    state: tauri::State<'_, AppState>,
) -> Result<FeedEntry, AppError> {
    lookup_feeds_inner(
        &material_id,
        &tool_material,
        &operation_category,
        &state.feed_library,
    )
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feed_library::FEEDS_TOML;

    fn library() -> FeedLibrary {
        FeedLibrary::from_toml(FEEDS_TOML).unwrap()
    }

    #[test]
    fn list_materials_inner_returns_at_least_four() {
        let lib = library();
        let materials = list_materials_inner(&lib);
        assert!(materials.len() >= 4);
    }

    #[test]
    fn lookup_feeds_inner_returns_correct_entry_for_known_triple() {
        let lib = library();
        let entry = lookup_feeds_inner("aluminum-6061", "carbide", "roughing", &lib)
            .expect("entry must exist");
        assert_eq!(entry.spindle_speed_rpm, 10000);
    }

    #[test]
    fn lookup_feeds_inner_returns_not_found_for_unknown_material() {
        let lib = library();
        let err = lookup_feeds_inner("unobtanium-99", "carbide", "roughing", &lib).unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }
}
