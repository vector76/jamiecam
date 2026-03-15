//! Toolpath planner — entry-point for turning an [`Operation`] into a [`Toolpath`].

use crate::error::AppError;
use crate::geometry::OcctShape;
use crate::models::operation::OperationParams;
use crate::models::{Operation, StockDefinition, Tool};
use crate::toolpath::operations::zlevel_finishing::RoughingData;
use crate::toolpath::{operations, types, types::ToolpathStats};

/// Generate unlinked cutting passes and [`ToolpathStats`] for the given operation.
///
/// Returns raw passes without linking moves applied. For Drill operations,
/// `drill_passes` handles its own linking internally. For Pocket, Profile,
/// ZLevelRoughing, and ZLevelFinishing operations, the caller is responsible for calling
/// [`linking::link_passes`] and assembling the final [`Toolpath`].
pub fn plan(
    operation: &Operation,
    tool: &Tool,
    stock: &StockDefinition,
    shape: Option<&OcctShape>,
    roughing_data: Option<&RoughingData>,
) -> Result<(Vec<types::Pass>, ToolpathStats), AppError> {
    // Step 1: Compute clearance height and stock boundary.
    let StockDefinition::Box(b) = stock;
    let stock_boundary: Vec<(f64, f64)> = vec![
        (b.origin.x, b.origin.y),
        (b.origin.x + b.width, b.origin.y),
        (b.origin.x + b.width, b.origin.y + b.depth),
        (b.origin.x, b.origin.y + b.depth),
    ];

    // Step 1b: Resolve boundary — use geometry selection if present, else stock.
    let boundary: Vec<(f64, f64)> = match &operation.params {
        OperationParams::Pocket(p) if p.geometry.is_some() => {
            resolve_geometry_boundary(shape, p.geometry.as_deref().unwrap())?
        }
        OperationParams::Profile(p) if p.geometry.is_some() => {
            resolve_geometry_boundary(shape, p.geometry.as_deref().unwrap())?
        }
        _ => stock_boundary,
    };

    // Step 2: Generate cutting passes based on operation type.
    // Pocket, Profile, ZLevelRoughing, and ZLevelFinishing return unlinked cutting passes; Drill handles its own linking.
    let passes = match &operation.params {
        OperationParams::Pocket(params) => {
            operations::pocket::pocket_passes(stock, params, tool.diameter, &boundary)?
        }
        OperationParams::Profile(params) => {
            operations::profile::profile_passes(stock, params, tool.diameter, &boundary)?
        }
        OperationParams::Drill(params) => operations::drill::drill_passes(stock, params)?,
        OperationParams::ZLevelRoughing(params) => {
            operations::zlevel_roughing::zlevel_roughing_passes(
                stock,
                params,
                tool.diameter,
                shape,
            )?
        }
        OperationParams::ZLevelFinishing(params) => {
            operations::zlevel_finishing::zlevel_finishing_passes(
                stock,
                params,
                tool.diameter,
                shape,
                roughing_data,
            )?
        }
        OperationParams::AdaptiveClearing(params) => {
            let base_feed = operation
                .feed_rate_override
                .or(tool.default_feed_rate)
                .unwrap_or(500.0);
            operations::adaptive_clearing::adaptive_clearing_passes(
                stock,
                params,
                tool.diameter,
                shape,
                base_feed,
            )?
        }
        OperationParams::ParallelFinishing(params) => {
            operations::parallel_finishing::parallel_finishing_passes(
                stock,
                params,
                tool.diameter,
                shape,
            )?
        }
        OperationParams::ScallopFinishing(params) => {
            operations::scallop_finishing::scallop_finishing_passes(
                stock,
                params,
                tool.diameter,
                shape,
            )?
        }
        OperationParams::FlowlineFinishing(_) => {
            return Err(crate::error::AppError::InvalidInput(
                "flowline finishing toolpath generation is not yet implemented".to_string(),
            ));
        }
        OperationParams::PencilMilling(_) => {
            return Err(crate::error::AppError::InvalidInput(
                "pencil milling toolpath generation is not yet implemented".to_string(),
            ));
        }
    };

    // Step 3: Compute stats over the returned passes.
    let total_pass_count = passes.len();
    let total_point_count: usize = passes.iter().map(|p| p.cuts.len()).sum();
    let total_path_length_mm: f64 = passes
        .iter()
        .flat_map(|p| p.cuts.windows(2))
        .map(|pair| {
            let a = &pair[0].position;
            let b = &pair[1].position;
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let dz = b.z - a.z;
            (dx * dx + dy * dy + dz * dz).sqrt()
        })
        .sum();

    let stats = ToolpathStats {
        total_pass_count,
        total_point_count,
        total_path_length_mm,
    };

    Ok((passes, stats))
}

// ── resolve_geometry_boundary ─────────────────────────────────────────────────

/// Resolve a list of face fingerprints to a 2-D boundary polygon.
///
/// Returns the outer-wire boundary of the selected face(s), unioned together
/// when multiple fingerprints are provided.
fn resolve_geometry_boundary(
    shape: Option<&OcctShape>,
    fingerprints: &[String],
) -> Result<Vec<(f64, f64)>, AppError> {
    let shape = shape.ok_or_else(|| {
        AppError::GeometryImport("no model loaded — cannot resolve geometry selection".into())
    })?;

    #[cfg(cam_geometry_bindings)]
    {
        use crate::geometry::{enumerate_faces, face_boundary, poly_boolean, BoolOp};

        let descriptors = enumerate_faces(shape).map_err(AppError::from)?;

        let mut combined: Option<Vec<(f64, f64)>> = None;
        for fp in fingerprints {
            let desc = descriptors
                .iter()
                .find(|d| &d.fingerprint == fp)
                .ok_or_else(|| {
                    AppError::GeometryImport(format!(
                        "face fingerprint '{}' not found in current model — model may have changed",
                        fp
                    ))
                })?;
            let boundary = face_boundary(shape, desc.face_idx).map_err(AppError::from)?;
            combined = Some(match combined {
                None => boundary,
                Some(acc) => {
                    poly_boolean(&acc, &boundary, BoolOp::Union).map_err(AppError::from)?
                }
            });
        }

        Ok(combined.unwrap_or_default())
    }

    #[cfg(not(cam_geometry_bindings))]
    {
        let _ = (shape, fingerprints);
        Err(AppError::GeometryImport("OCCT not available".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::operation::{
        AdaptiveClearingParams, CacheState, CompensationSide, DrillParams, DrillPoint,
        OperationParams, PocketParams, ProfileParams, ZLevelRoughingParams,
    };
    use crate::models::stock::BoxDimensions;
    use crate::models::tool::ToolType;
    use crate::models::{StockDefinition, Tool, Vec3};
    use uuid::Uuid;

    fn make_stock_50x50x10() -> StockDefinition {
        StockDefinition::Box(BoxDimensions {
            origin: Vec3::zero(),
            width: 50.0,
            depth: 50.0,
            height: 10.0,
        })
    }

    fn make_tool_10mm() -> Tool {
        Tool {
            id: Uuid::nil(),
            name: "10mm Flat Endmill".to_string(),
            tool_type: ToolType::FlatEndmill,
            material: "carbide".to_string(),
            diameter: 10.0,
            flute_count: 4,
            default_spindle_speed: None,
            default_feed_rate: None,
        }
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn plan_stats_are_non_zero_for_pocket() {
        let operation = Operation {
            id: Uuid::nil(),
            name: "Pocket Op".to_string(),
            enabled: true,
            tool_id: Uuid::nil(),
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::Pocket(PocketParams {
                depth: 10.0,
                stepdown: 2.0,
                stepover_percent: 50.0,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };
        let tool = make_tool_10mm();
        let stock = make_stock_50x50x10();
        let (passes, stats) =
            plan(&operation, &tool, &stock, None, None).expect("pocket plan should succeed");
        assert!(!passes.is_empty());
        assert!(stats.total_pass_count > 0);
        assert!(stats.total_path_length_mm > 0.0);
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn plan_stats_are_non_zero_for_profile() {
        let operation = Operation {
            id: Uuid::nil(),
            name: "Profile Op".to_string(),
            enabled: true,
            tool_id: Uuid::nil(),
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::Profile(ProfileParams {
                depth: 10.0,
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
        };
        let tool = make_tool_10mm();
        let stock = make_stock_50x50x10();
        let (passes, stats) =
            plan(&operation, &tool, &stock, None, None).expect("profile plan should succeed");
        assert!(!passes.is_empty());
        assert!(stats.total_pass_count > 0);
        assert!(stats.total_path_length_mm > 0.0);
    }

    #[cfg(not(cam_geometry_bindings))]
    #[test]
    fn plan_profile_returns_error_without_geometry_bindings() {
        let operation = Operation {
            id: Uuid::nil(),
            name: "Profile Op".to_string(),
            enabled: true,
            tool_id: Uuid::nil(),
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::Profile(ProfileParams {
                depth: 5.0,
                stepdown: Some(2.0),
                compensation_side: CompensationSide::Left,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };
        let tool = make_tool_10mm();
        let stock = make_stock_50x50x10();
        assert!(plan(&operation, &tool, &stock, None, None).is_err());
    }

    fn make_drill_operation(
        spindle_speed_override: Option<u32>,
        feed_rate_override: Option<f64>,
    ) -> Operation {
        Operation {
            id: Uuid::nil(),
            name: "Drill Op".to_string(),
            enabled: true,
            tool_id: Uuid::nil(),
            spindle_speed_override,
            feed_rate_override,
            params: OperationParams::Drill(DrillParams {
                depth: 5.0,
                peck_depth: None,
                points: vec![DrillPoint { x: 10.0, y: 10.0 }],
            }),
            cache: CacheState::default(),
        }
    }

    #[test]
    fn plan_drill_with_spindle_speed_override_returns_passes() {
        let operation = make_drill_operation(Some(12000), None);
        let tool = Tool {
            default_spindle_speed: Some(8000),
            ..make_tool_10mm()
        };
        let stock = make_stock_50x50x10();
        let (passes, _) =
            plan(&operation, &tool, &stock, None, None).expect("drill plan should succeed");
        assert!(!passes.is_empty());
    }

    #[test]
    fn plan_drill_with_tool_default_spindle_returns_passes() {
        let operation = make_drill_operation(None, None);
        let tool = Tool {
            default_spindle_speed: Some(9000),
            ..make_tool_10mm()
        };
        let stock = make_stock_50x50x10();
        let (passes, _) =
            plan(&operation, &tool, &stock, None, None).expect("drill plan should succeed");
        assert!(!passes.is_empty());
    }

    #[test]
    fn plan_drill_with_no_spindle_set_returns_passes() {
        let operation = make_drill_operation(None, None);
        let tool = make_tool_10mm(); // default_spindle_speed: None
        let stock = make_stock_50x50x10();
        let (passes, _) =
            plan(&operation, &tool, &stock, None, None).expect("drill plan should succeed");
        assert!(!passes.is_empty());
    }

    #[test]
    fn plan_drill_with_feed_rate_override_returns_passes() {
        let operation = make_drill_operation(None, Some(800.0));
        let tool = Tool {
            default_feed_rate: Some(500.0),
            ..make_tool_10mm()
        };
        let stock = make_stock_50x50x10();
        let (passes, _) =
            plan(&operation, &tool, &stock, None, None).expect("drill plan should succeed");
        assert!(!passes.is_empty());
    }

    #[test]
    fn plan_drill_with_tool_default_feed_rate_returns_passes() {
        let operation = make_drill_operation(None, None);
        let tool = Tool {
            default_feed_rate: Some(300.0),
            ..make_tool_10mm()
        };
        let stock = make_stock_50x50x10();
        let (passes, _) =
            plan(&operation, &tool, &stock, None, None).expect("drill plan should succeed");
        assert!(!passes.is_empty());
    }

    #[test]
    fn plan_drill_with_no_feed_rate_set_returns_passes() {
        let operation = make_drill_operation(None, None);
        let tool = make_tool_10mm(); // default_feed_rate: None
        let stock = make_stock_50x50x10();
        let (passes, _) =
            plan(&operation, &tool, &stock, None, None).expect("drill plan should succeed");
        assert!(!passes.is_empty());
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn plan_pocket_with_geometry_none_uses_stock_boundary() {
        let operation = Operation {
            id: Uuid::nil(),
            name: "Pocket Op".to_string(),
            enabled: true,
            tool_id: Uuid::nil(),
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::Pocket(PocketParams {
                depth: 10.0,
                stepdown: 2.0,
                stepover_percent: 50.0,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };
        let tool = make_tool_10mm();
        let stock = make_stock_50x50x10();
        let (passes, stats) = plan(&operation, &tool, &stock, None, None)
            .expect("pocket with no geometry should use stock boundary");
        assert!(!passes.is_empty());
        assert!(stats.total_pass_count > 0);
        assert!(stats.total_path_length_mm > 0.0);
    }

    #[test]
    fn plan_zlevel_roughing_invalid_params_returns_invalid_input() {
        let operation = Operation {
            id: Uuid::nil(),
            name: "ZLR Op".to_string(),
            enabled: true,
            tool_id: Uuid::nil(),
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::ZLevelRoughing(ZLevelRoughingParams {
                depth: 5.0,
                stepdown: 0.0, // invalid
                stepover: 0.5,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };
        let tool = make_tool_10mm();
        let stock = make_stock_50x50x10();
        let result = plan(&operation, &tool, &stock, None, None);
        assert!(
            matches!(result, Err(AppError::InvalidInput(_))),
            "expected InvalidInput error, got: {result:?}"
        );
    }

    #[test]
    fn plan_pocket_with_geometry_some_and_no_shape_returns_error() {
        let operation = Operation {
            id: Uuid::nil(),
            name: "Pocket Op".to_string(),
            enabled: true,
            tool_id: Uuid::nil(),
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::Pocket(PocketParams {
                depth: 10.0,
                stepdown: 2.0,
                stepover_percent: 50.0,
                geometry: Some(vec!["fp1".into()]),
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };
        let tool = make_tool_10mm();
        let stock = make_stock_50x50x10();
        let result = plan(&operation, &tool, &stock, None, None);
        assert!(
            matches!(result, Err(AppError::GeometryImport(_))),
            "expected GeometryImport error, got: {result:?}"
        );
    }

    #[test]
    fn plan_parallel_finishing_returns_error_without_shape() {
        use crate::models::operation::{CacheState, ParallelFinishingParams};
        let operation = Operation {
            id: uuid::Uuid::nil(),
            name: "PF Op".to_string(),
            enabled: true,
            tool_id: uuid::Uuid::nil(),
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::ParallelFinishing(ParallelFinishingParams {
                stepover: 0.5,
                direction_angle_deg: 0.0,
                allowance: 0.0,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };
        let result = plan(
            &operation,
            &make_tool_10mm(),
            &make_stock_50x50x10(),
            None,
            None,
        );
        assert!(
            result.is_err(),
            "expected error when no shape provided, got: {result:?}"
        );
    }

    #[test]
    fn plan_scallop_finishing_returns_error_without_shape() {
        use crate::models::operation::{CacheState, ScallopFinishingParams};
        let operation = Operation {
            id: uuid::Uuid::nil(),
            name: "SF Op".to_string(),
            enabled: true,
            tool_id: uuid::Uuid::nil(),
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::ScallopFinishing(ScallopFinishingParams {
                target_scallop_height: 0.01,
                min_stepover: 0.1,
                max_stepover: 2.0,
                direction_angle_deg: 0.0,
                allowance: 0.0,
                tool_radius: 5.0,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };
        let result = plan(
            &operation,
            &make_tool_10mm(),
            &make_stock_50x50x10(),
            None,
            None,
        );
        assert!(
            result.is_err(),
            "expected error when no shape provided, got: {result:?}"
        );
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn plan_adaptive_clearing_produces_passes_and_stats() {
        let operation = Operation {
            id: Uuid::nil(),
            name: "Adaptive Clearing Op".to_string(),
            enabled: true,
            tool_id: Uuid::nil(),
            spindle_speed_override: None,
            feed_rate_override: Some(600.0),
            params: OperationParams::AdaptiveClearing(AdaptiveClearingParams {
                depth: 5.0,
                stepdown: 2.5,
                optimal_load: 0.25,
                stepover_percent: 40.0,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };
        let tool = make_tool_10mm();
        let stock = make_stock_50x50x10();
        let (passes, stats) = plan(&operation, &tool, &stock, None, None)
            .expect("adaptive clearing plan should succeed");
        assert!(!passes.is_empty(), "should produce at least one pass");
        assert!(
            stats.total_pass_count > 1,
            "expected multiple passes, got {}",
            stats.total_pass_count
        );
        assert!(stats.total_point_count > 0, "expected non-zero point count");
        assert!(
            stats.total_path_length_mm > 0.0,
            "expected non-zero path length"
        );
    }
}
