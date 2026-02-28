//! Stock and WCS IPC command handlers.
//!
//! All handlers follow the `_inner` + `#[tauri::command]` wrapper pattern:
//! - `_inner` functions take `&RwLock<Project>` and contain the business logic.
//!   They are synchronous and directly testable without Tauri.
//! - `#[tauri::command]` wrappers extract managed state and delegate to `_inner`.

use std::sync::RwLock;

use crate::error::AppError;
use crate::models::{StockDefinition, WorkCoordinateSystem};
use crate::state::{AppState, Project};

use super::{read_project, write_project};

// ── set_stock ─────────────────────────────────────────────────────────────────

/// Testable inner logic for [`set_stock`].
///
/// Replaces (or clears, when `None`) the project's stock definition.
pub(crate) fn set_stock_inner(
    stock: Option<StockDefinition>,
    project_lock: &RwLock<Project>,
) -> Result<(), AppError> {
    let mut project = write_project(project_lock)?;
    project.stock = stock;
    Ok(())
}

// ── get_stock ─────────────────────────────────────────────────────────────────

/// Testable inner logic for [`get_stock`].
///
/// Returns a clone of the current stock definition, or `None` if unset.
pub(crate) fn get_stock_inner(
    project_lock: &RwLock<Project>,
) -> Result<Option<StockDefinition>, AppError> {
    let project = read_project(project_lock)?;
    Ok(project.stock.clone())
}

// ── set_wcs ───────────────────────────────────────────────────────────────────

/// Testable inner logic for [`set_wcs`].
///
/// Replaces the entire WCS list for the project.
pub(crate) fn set_wcs_inner(
    wcs: Vec<WorkCoordinateSystem>,
    project_lock: &RwLock<Project>,
) -> Result<(), AppError> {
    let mut project = write_project(project_lock)?;
    project.wcs = wcs;
    Ok(())
}

// ── get_wcs ───────────────────────────────────────────────────────────────────

/// Testable inner logic for [`get_wcs`].
///
/// Returns a snapshot of the current WCS list.
pub(crate) fn get_wcs_inner(
    project_lock: &RwLock<Project>,
) -> Result<Vec<WorkCoordinateSystem>, AppError> {
    let project = read_project(project_lock)?;
    Ok(project.wcs.clone())
}

// ── Tauri command wrappers ────────────────────────────────────────────────────

/// Set (or clear) the project stock definition.
///
/// Pass `null` from the frontend to clear the stock.
#[tauri::command]
pub async fn set_stock(
    stock: Option<StockDefinition>,
    state: tauri::State<'_, AppState>,
) -> Result<(), AppError> {
    set_stock_inner(stock, &state.project)
}

/// Return the current project stock definition, or `null` if none is set.
#[tauri::command]
pub async fn get_stock(
    state: tauri::State<'_, AppState>,
) -> Result<Option<StockDefinition>, AppError> {
    get_stock_inner(&state.project)
}

/// Replace the project's WCS list.
#[tauri::command]
pub async fn set_wcs(
    wcs: Vec<WorkCoordinateSystem>,
    state: tauri::State<'_, AppState>,
) -> Result<(), AppError> {
    set_wcs_inner(wcs, &state.project)
}

/// Return the project's WCS list.
#[tauri::command]
pub async fn get_wcs(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<WorkCoordinateSystem>, AppError> {
    get_wcs_inner(&state.project)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::stock::{BoxDimensions, Vec3};
    use crate::state::AppState;
    use uuid::Uuid;

    fn make_box_stock() -> StockDefinition {
        StockDefinition::Box(BoxDimensions {
            origin: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            width: 100.0,
            depth: 80.0,
            height: 25.0,
        })
    }

    fn make_wcs() -> WorkCoordinateSystem {
        WorkCoordinateSystem {
            id: Uuid::new_v4(),
            name: "G54".to_string(),
            origin: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            x_axis: Vec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            z_axis: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        }
    }

    #[test]
    fn default_project_has_no_stock() {
        let state = AppState::default();
        let result = get_stock_inner(&state.project).expect("get_stock should succeed");
        assert!(result.is_none());
    }

    #[test]
    fn default_project_has_empty_wcs() {
        let state = AppState::default();
        let result = get_wcs_inner(&state.project).expect("get_wcs should succeed");
        assert!(result.is_empty());
    }

    #[test]
    fn set_stock_then_get_returns_same_value() {
        let state = AppState::default();
        let stock = make_box_stock();
        set_stock_inner(Some(stock.clone()), &state.project).expect("set_stock should succeed");
        let retrieved = get_stock_inner(&state.project)
            .expect("get_stock should succeed")
            .expect("stock should be set");
        assert_eq!(retrieved, stock);
    }

    #[test]
    fn set_stock_none_clears_stock() {
        let state = AppState::default();
        set_stock_inner(Some(make_box_stock()), &state.project).expect("set");
        set_stock_inner(None, &state.project).expect("clear");
        let result = get_stock_inner(&state.project).expect("get");
        assert!(result.is_none());
    }

    #[test]
    fn set_wcs_then_get_returns_same_list() {
        let state = AppState::default();
        let wcs_list = vec![make_wcs(), make_wcs()];
        set_wcs_inner(wcs_list.clone(), &state.project).expect("set_wcs should succeed");
        let retrieved = get_wcs_inner(&state.project).expect("get_wcs should succeed");
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].name, wcs_list[0].name);
        assert_eq!(retrieved[1].name, wcs_list[1].name);
    }

    #[test]
    fn set_wcs_replaces_previous_list() {
        let state = AppState::default();
        set_wcs_inner(vec![make_wcs(), make_wcs(), make_wcs()], &state.project).expect("set 3");
        set_wcs_inner(vec![make_wcs()], &state.project).expect("replace with 1");
        let retrieved = get_wcs_inner(&state.project).expect("get");
        assert_eq!(retrieved.len(), 1);
    }
}
