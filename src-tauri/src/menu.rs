//! Menu helpers for dynamic menu-item state management.

/// Menu item ID for the Save action (must match the ID used in lib.rs menu setup).
const MENU_SAVE: &str = "save";

/// Enable or disable the Save menu item to reflect the current dirty state.
///
/// Silently ignores errors — the menu may not exist in headless/test environments.
pub(crate) fn update_save_enabled(app: &tauri::AppHandle, dirty: bool) {
    let Some(menu) = app.menu() else { return };
    let Some(item) = menu.get(MENU_SAVE) else {
        return;
    };
    let Some(menu_item) = item.as_menuitem() else {
        return;
    };
    let _ = menu_item.set_enabled(dirty);
}
