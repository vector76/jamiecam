//! Dexel simulation IPC command handlers.
//!
//! All handlers follow the `_inner` + `#[tauri::command]` wrapper pattern:
//! - `_inner` functions take `&RwLock<Project>` and contain the business logic.
//!   They are synchronous and directly testable without Tauri.
//! - `#[tauri::command]` wrappers extract managed state and delegate to `_inner`.

use std::sync::RwLock;

use uuid::Uuid;

use crate::dexel::{clearance_for_tool, toolpath_to_segments, DexelGrid};
use crate::error::AppError;
use crate::geometry::MeshData;
use crate::state::{AppState, Project};

use super::read_project;

// ── get_simulation_mesh ──────────────────────────────────────────────────────

/// Testable inner logic for [`get_simulation_mesh`].
///
/// Builds a dexel grid from the project stock, applies toolpath segments for
/// the requested operations, and returns the resulting triangle mesh.
pub(crate) fn get_simulation_mesh_inner(
    resolution: f64,
    operation_ids: Option<Vec<Uuid>>,
    up_to_segment: Option<usize>,
    project_lock: &RwLock<Project>,
) -> Result<MeshData, AppError> {
    // (a) Validate resolution range.
    if !(0.01..=5.0).contains(&resolution) {
        return Err(AppError::InvalidInput(
            "resolution must be between 0.01 and 5.0".into(),
        ));
    }

    // (b) Acquire read lock and clone the data we need, then release.
    let (stock, operations, tools, toolpaths) = {
        let project = read_project(project_lock)?;

        let stock = project.stock.clone();
        let operations = project.operations.clone();
        let tools = project.tools.clone();
        let toolpaths = project.toolpaths.clone();

        (stock, operations, tools, toolpaths)
    };

    // (c) Require stock.
    let stock = stock.ok_or_else(|| AppError::InvalidInput("no stock defined".into()))?;

    // (d) Resolve which operations to simulate.
    let ops = match operation_ids {
        Some(ids) => {
            let mut resolved = Vec::with_capacity(ids.len());
            for id in &ids {
                let op = operations
                    .iter()
                    .find(|op| op.id == *id)
                    .ok_or_else(|| AppError::NotFound(format!("operation {id} not found")))?;
                resolved.push(op.clone());
            }
            resolved
        }
        None => operations.into_iter().filter(|op| op.enabled).collect(),
    };

    // (e) Initialize dexel grid from stock.
    let mut grid = DexelGrid::from_stock(&stock, resolution);

    // (f) Apply segments for each operation, carrying tool position across.
    let mut segments_applied: usize = 0;
    let mut last_position: Option<crate::models::Vec3> = None;

    for op in &ops {
        // Find the tool for this operation.
        let tool = tools
            .iter()
            .find(|t| t.id == op.tool_id)
            .ok_or_else(|| AppError::NotFound(format!("tool {} not found", op.tool_id)))?;

        let (tool_radius, z_clearance) = clearance_for_tool(tool);

        // Find the toolpath for this operation.
        let toolpath = toolpaths.get(&op.id).ok_or_else(|| {
            AppError::NotFound(format!("toolpath for operation {} not found", op.id))
        })?;

        let segments = toolpath_to_segments(toolpath, last_position.as_ref());

        // Apply segments, respecting the budget if set.
        let to_apply = if let Some(budget) = up_to_segment {
            let remaining = budget.saturating_sub(segments_applied);
            &segments[..remaining.min(segments.len())]
        } else {
            &segments[..]
        };

        grid.apply_segments(to_apply, tool_radius, &z_clearance);
        segments_applied += to_apply.len();

        // Track the last position for continuity with the next operation.
        if let Some(last_seg) = to_apply.last() {
            last_position = Some(match last_seg {
                crate::dexel::MotionSegment::Linear { end, .. } => end.clone(),
                crate::dexel::MotionSegment::Arc { end, .. } => end.clone(),
            });
        }

        // Break if budget exhausted.
        if let Some(budget) = up_to_segment {
            if segments_applied >= budget {
                break;
            }
        }
    }

    // (g) Extract and return mesh.
    Ok(grid.extract_mesh())
}

// ── Tauri command wrapper ────────────────────────────────────────────────────

/// Compute a simulation mesh by applying toolpath segments to the stock via the
/// dexel engine.
#[tauri::command]
pub async fn get_simulation_mesh(
    resolution: f64,
    operation_ids: Option<Vec<Uuid>>,
    up_to_segment: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<MeshData, AppError> {
    get_simulation_mesh_inner(resolution, operation_ids, up_to_segment, &state.project)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::operation::{CacheState, CompensationSide, OperationParams, ProfileParams};
    use crate::models::stock::{BoxDimensions, Vec3};
    use crate::models::{Operation, StockDefinition, Tool, ToolType};
    use crate::state::AppState;
    use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind, Toolpath};

    /// Assert that a `MeshData` is structurally valid:
    /// - non-empty vertices/normals/indices
    /// - all indices in bounds
    /// - all normals are unit-length (within tolerance)
    /// - no degenerate (zero-area) triangles
    fn assert_mesh_valid(mesh: &MeshData) {
        let vertex_count = mesh.vertices.len() / 3;
        assert!(vertex_count > 0, "mesh has no vertices");
        assert_eq!(mesh.vertices.len() % 3, 0, "vertices not a multiple of 3");
        assert_eq!(
            mesh.normals.len(),
            mesh.vertices.len(),
            "normals/vertices length mismatch"
        );
        assert!(mesh.indices.len() >= 3, "mesh has no triangles");
        assert_eq!(mesh.indices.len() % 3, 0, "indices not a multiple of 3");

        // All indices in bounds.
        for (i, &idx) in mesh.indices.iter().enumerate() {
            assert!(
                (idx as usize) < vertex_count,
                "index[{i}] = {idx} out of bounds (vertex_count={vertex_count})"
            );
        }

        // All normals are unit-length.
        for i in 0..vertex_count {
            let nx = mesh.normals[i * 3] as f64;
            let ny = mesh.normals[i * 3 + 1] as f64;
            let nz = mesh.normals[i * 3 + 2] as f64;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            assert!(
                (len - 1.0).abs() < 0.01,
                "normal[{i}] = ({nx},{ny},{nz}) has length {len}, expected ~1.0"
            );
        }

        // No degenerate triangles (zero-area).
        for tri in 0..(mesh.indices.len() / 3) {
            let i0 = mesh.indices[tri * 3] as usize;
            let i1 = mesh.indices[tri * 3 + 1] as usize;
            let i2 = mesh.indices[tri * 3 + 2] as usize;

            let v0 = [
                mesh.vertices[i0 * 3],
                mesh.vertices[i0 * 3 + 1],
                mesh.vertices[i0 * 3 + 2],
            ];
            let v1 = [
                mesh.vertices[i1 * 3],
                mesh.vertices[i1 * 3 + 1],
                mesh.vertices[i1 * 3 + 2],
            ];
            let v2 = [
                mesh.vertices[i2 * 3],
                mesh.vertices[i2 * 3 + 1],
                mesh.vertices[i2 * 3 + 2],
            ];

            // Edge vectors.
            let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
            let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];

            // Cross product magnitude = 2 * triangle area.
            let cx = e1[1] * e2[2] - e1[2] * e2[1];
            let cy = e1[2] * e2[0] - e1[0] * e2[2];
            let cz = e1[0] * e2[1] - e1[1] * e2[0];
            let area2 = (cx * cx + cy * cy + cz * cz).sqrt();
            assert!(
                area2 > 1e-12,
                "triangle {tri} (indices {i0},{i1},{i2}) is degenerate (area~0)"
            );
        }
    }

    fn make_stock() -> StockDefinition {
        StockDefinition::Box(BoxDimensions {
            origin: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            width: 20.0,
            depth: 20.0,
            height: 10.0,
        })
    }

    fn make_tool() -> Tool {
        Tool {
            id: Uuid::new_v4(),
            name: "Test Endmill".to_string(),
            tool_type: ToolType::FlatEndmill,
            material: Some("carbide".to_string()),
            diameter: 10.0,
            flute_count: Some(4),
            default_spindle_speed: None,
            default_feed_rate: None,
            cutting_length: 30.0,
            shank_diameter: 10.0,
            overall_length: Some(90.0),
            corner_radius: None,
            included_angle: None,
            point_angle: None,
            pilot_diameter: None,
            pilot_length: None,
            thread_pitch: None,
            min_bore_diameter: None,
            taper_half_angle: None,
        }
    }

    fn make_operation(tool_id: Uuid) -> Operation {
        Operation {
            id: Uuid::new_v4(),
            name: "Test Op".to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            workpiece_material: None,
            params: OperationParams::Profile(ProfileParams {
                depth: 5.0,
                stepdown: Some(2.5),
                compensation_side: CompensationSide::Left,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        }
    }

    fn make_toolpath(operation_id: Uuid) -> Toolpath {
        Toolpath {
            operation_id,
            tool_number: 1,
            spindle_speed: 10000.0,
            feed_rate: 1000.0,
            passes: vec![Pass {
                kind: PassKind::Cutting,
                cuts: vec![
                    CutPoint {
                        position: Vec3 {
                            x: 5.0,
                            y: 5.0,
                            z: 10.0,
                        },
                        move_kind: MoveKind::Feed,
                        tool_orientation: None,
                        feed_rate_override: None,
                    },
                    CutPoint {
                        position: Vec3 {
                            x: 15.0,
                            y: 5.0,
                            z: 5.0,
                        },
                        move_kind: MoveKind::Feed,
                        tool_orientation: None,
                        feed_rate_override: None,
                    },
                ],
            }],
        }
    }

    #[test]
    fn error_invalid_resolution_too_low() {
        let state = AppState::default();
        let result = get_simulation_mesh_inner(0.001, None, None, &state.project);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::InvalidInput(_)));
    }

    #[test]
    fn error_invalid_resolution_too_high() {
        let state = AppState::default();
        let result = get_simulation_mesh_inner(10.0, None, None, &state.project);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::InvalidInput(_)));
    }

    #[test]
    fn error_missing_stock() {
        let state = AppState::default();
        // No stock set — should fail.
        let result = get_simulation_mesh_inner(1.0, None, None, &state.project);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::InvalidInput(_)));
    }

    #[test]
    fn error_unknown_operation_id() {
        let state = AppState::default();
        {
            let mut project = state.project.write().unwrap();
            project.stock = Some(make_stock());
        }
        let random_id = Uuid::new_v4();
        let result = get_simulation_mesh_inner(1.0, Some(vec![random_id]), None, &state.project);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::NotFound(_)));
    }

    #[test]
    fn error_missing_tool() {
        let state = AppState::default();
        let nonexistent_tool_id = Uuid::new_v4();
        let op = make_operation(nonexistent_tool_id);
        let op_id = op.id;
        {
            let mut project = state.project.write().unwrap();
            project.stock = Some(make_stock());
            project.operations.push(op);
            project.toolpaths.insert(op_id, make_toolpath(op_id));
            // No tool added — should fail.
        }
        let result = get_simulation_mesh_inner(1.0, Some(vec![op_id]), None, &state.project);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::NotFound(_)));
    }

    #[test]
    fn error_missing_toolpath() {
        let state = AppState::default();
        let tool = make_tool();
        let op = make_operation(tool.id);
        let op_id = op.id;
        {
            let mut project = state.project.write().unwrap();
            project.stock = Some(make_stock());
            project.tools.push(tool);
            project.operations.push(op);
            // No toolpath inserted — should fail.
        }
        let result = get_simulation_mesh_inner(1.0, Some(vec![op_id]), None, &state.project);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::NotFound(_)));
    }

    // ── Integration helpers ─────────────────────────────────────────────

    fn make_large_stock() -> StockDefinition {
        StockDefinition::Box(BoxDimensions {
            origin: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            width: 50.0,
            depth: 50.0,
            height: 10.0,
        })
    }

    /// Build a rectangular pocket toolpath: a series of Feed moves at constant
    /// Z across the stock, forming a raster pattern.
    fn make_pocket_toolpath(operation_id: Uuid, z_depth: f64) -> Toolpath {
        let mut cuts = Vec::new();
        // Raster passes at 2mm Y spacing across X=[10,40], Y=[10,40].
        // The first CutPoint establishes the start position (no segment
        // generated from it alone), subsequent points create segments.
        let mut y = 10.0;
        let mut forward = true;
        while y <= 40.0 {
            let (x_start, x_end) = if forward { (10.0, 40.0) } else { (40.0, 10.0) };
            cuts.push(CutPoint {
                position: Vec3 {
                    x: x_start,
                    y,
                    z: z_depth,
                },
                move_kind: MoveKind::Feed,
                tool_orientation: None,
                feed_rate_override: None,
            });
            cuts.push(CutPoint {
                position: Vec3 {
                    x: x_end,
                    y,
                    z: z_depth,
                },
                move_kind: MoveKind::Feed,
                tool_orientation: None,
                feed_rate_override: None,
            });
            y += 2.0;
            forward = !forward;
        }

        Toolpath {
            operation_id,
            tool_number: 1,
            spindle_speed: 10000.0,
            feed_rate: 1000.0,
            passes: vec![Pass {
                kind: PassKind::Cutting,
                cuts,
            }],
        }
    }

    // ── Integration tests ───────────────────────────────────────────────

    #[test]
    fn integration_full_pipeline_produces_valid_mesh() {
        let state = AppState::default();
        let tool = make_tool(); // diameter=10 flat endmill
        let op = make_operation(tool.id);
        let op_id = op.id;
        let toolpath = make_pocket_toolpath(op_id, 5.0);
        {
            let mut project = state.project.write().unwrap();
            project.stock = Some(make_large_stock());
            project.tools.push(tool);
            project.operations.push(op);
            project.toolpaths.insert(op_id, toolpath);
        }

        let mesh = get_simulation_mesh_inner(1.0, None, None, &state.project).unwrap();
        assert_mesh_valid(&mesh);
    }

    #[test]
    fn integration_up_to_segment_partial_removal() {
        let state = AppState::default();
        let tool = make_tool();
        let op = make_operation(tool.id);
        let op_id = op.id;
        let toolpath = make_pocket_toolpath(op_id, 5.0);
        {
            let mut project = state.project.write().unwrap();
            project.stock = Some(make_large_stock());
            project.tools.push(tool);
            project.operations.push(op);
            project.toolpaths.insert(op_id, toolpath);
        }

        // Full cut.
        let mesh_full = get_simulation_mesh_inner(1.0, None, None, &state.project).unwrap();
        // Partial cut — only 1 segment applied.
        let mesh_partial = get_simulation_mesh_inner(1.0, None, Some(1), &state.project).unwrap();

        assert_mesh_valid(&mesh_full);
        assert_mesh_valid(&mesh_partial);

        // Partial cut (1 segment = one raster pass) removes less material
        // than the full raster, producing a different mesh.
        assert_ne!(
            mesh_full.vertices, mesh_partial.vertices,
            "full and partial meshes should have different geometry"
        );
    }

    #[test]
    fn integration_operation_ids_filtering() {
        let state = AppState::default();
        let tool_a = make_tool();
        let mut tool_b = make_tool();
        tool_b.diameter = 6.0;
        tool_b.name = "Small Endmill".to_string();

        let op_a = make_operation(tool_a.id);
        let op_b = make_operation(tool_b.id);
        let op_a_id = op_a.id;
        let op_b_id = op_b.id;

        // Op A: deep pocket at Z=3.
        let toolpath_a = make_pocket_toolpath(op_a_id, 3.0);
        // Op B: shallow pocket at Z=8.
        let toolpath_b = make_pocket_toolpath(op_b_id, 8.0);

        {
            let mut project = state.project.write().unwrap();
            project.stock = Some(make_large_stock());
            project.tools.push(tool_a);
            project.tools.push(tool_b);
            project.operations.push(op_a);
            project.operations.push(op_b);
            project.toolpaths.insert(op_a_id, toolpath_a);
            project.toolpaths.insert(op_b_id, toolpath_b);
        }

        // Request only op_a.
        let mesh_a_only =
            get_simulation_mesh_inner(1.0, Some(vec![op_a_id]), None, &state.project).unwrap();
        // Request only op_b.
        let mesh_b_only =
            get_simulation_mesh_inner(1.0, Some(vec![op_b_id]), None, &state.project).unwrap();
        // Request both.
        let mesh_both =
            get_simulation_mesh_inner(1.0, Some(vec![op_a_id, op_b_id]), None, &state.project)
                .unwrap();

        assert_mesh_valid(&mesh_a_only);
        assert_mesh_valid(&mesh_b_only);
        assert_mesh_valid(&mesh_both);

        // Op A (Z=3) cuts deeper than Op B (Z=8), producing different
        // geometry at different Z levels.
        assert_ne!(
            mesh_a_only.vertices, mesh_b_only.vertices,
            "A-only and B-only meshes should have different vertex data"
        );
    }

    // ── End-to-end golden test via G-code fixture ───────────────────────

    #[test]
    fn end_to_end_gcode_fixture() {
        use crate::dexel::{self, flat_endmill_clearance, DexelGrid};
        use crate::gcode_parser;

        // Parse the adaptive clearing NC file.
        let nc_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/adaptive_clearing_golden.nc"
        );
        let nc_text = std::fs::read_to_string(nc_path).expect("read NC fixture");
        let parsed = gcode_parser::parse_gcode(&nc_text);
        assert!(
            !parsed.segments.is_empty(),
            "NC fixture should produce motion segments"
        );

        // Convert gcode_parser::MotionSegment → dexel::MotionSegment.
        let dexel_segments: Vec<dexel::MotionSegment> = parsed
            .segments
            .iter()
            .filter_map(|seg| match seg {
                gcode_parser::MotionSegment::Rapid { start, end, .. } => {
                    Some(dexel::MotionSegment::Linear {
                        start: start.clone(),
                        end: end.clone(),
                    })
                }
                gcode_parser::MotionSegment::Linear { start, end, .. } => {
                    Some(dexel::MotionSegment::Linear {
                        start: start.clone(),
                        end: end.clone(),
                    })
                }
                gcode_parser::MotionSegment::Arc {
                    start,
                    end,
                    center,
                    clockwise,
                    ..
                } => Some(dexel::MotionSegment::Arc {
                    start: start.clone(),
                    end: end.clone(),
                    center: center.clone(),
                    clockwise: *clockwise,
                }),
                gcode_parser::MotionSegment::Dwell { .. } => None,
            })
            .collect();

        assert!(
            !dexel_segments.is_empty(),
            "should have non-dwell segments to apply"
        );

        // Set up stock and tool.
        let stock = StockDefinition::Box(BoxDimensions {
            origin: Vec3 {
                x: -5.0,
                y: -5.0,
                z: 0.0,
            },
            width: 30.0,
            depth: 30.0,
            height: 15.0,
        });
        let tool_radius = 5.0;
        let z_clearance = flat_endmill_clearance(tool_radius);

        let mut grid = DexelGrid::from_stock(&stock, 1.0);
        let initial_volume = grid.volume();

        grid.apply_segments(&dexel_segments, tool_radius, &z_clearance);

        let final_volume = grid.volume();
        let removed = initial_volume - final_volume;

        // Verify material was removed.
        assert!(
            removed > 0.0,
            "should remove material; initial={initial_volume}, final={final_volume}"
        );
        // Verify plausible amount (some but not all stock removed).
        assert!(
            final_volume > 0.0,
            "should not remove all material; final_volume={final_volume}"
        );

        // Extract mesh and validate.
        let mesh = grid.extract_mesh();
        assert_mesh_valid(&mesh);
    }
}
