//! Toolpath planner — entry-point for turning an [`Operation`] into a [`Toolpath`].

use crate::error::AppError;
use crate::models::operation::OperationParams;
use crate::models::{Operation, StockDefinition, Tool};
use crate::toolpath::{
    linking, operations,
    types::{Toolpath, ToolpathStats},
};

/// Generate a [`Toolpath`] and [`ToolpathStats`] for the given operation.
pub fn plan(
    operation: &Operation,
    tool: &Tool,
    stock: &StockDefinition,
) -> Result<(Toolpath, ToolpathStats), AppError> {
    // Step 1: Compute clearance height and stock boundary.
    let StockDefinition::Box(b) = stock;
    let stock_top_z = b.origin.z + b.height;
    let stock_boundary: Vec<(f64, f64)> = vec![
        (b.origin.x, b.origin.y),
        (b.origin.x + b.width, b.origin.y),
        (b.origin.x + b.width, b.origin.y + b.depth),
        (b.origin.x, b.origin.y + b.depth),
    ];

    // Step 2: Generate cutting passes based on operation type.
    let linked_passes = match &operation.params {
        OperationParams::Pocket(params) => {
            let passes =
                operations::pocket::pocket_passes(stock, params, tool.diameter, &stock_boundary)?;
            linking::link_passes(passes, tool.diameter, stock_top_z + 5.0)
        }
        OperationParams::Profile(params) => {
            let passes =
                operations::profile::profile_passes(stock, params, tool.diameter, &stock_boundary)?;
            linking::link_passes(passes, tool.diameter, stock_top_z + 5.0)
        }
        OperationParams::Drill(params) => operations::drill::drill_passes(stock, params)?,
    };

    // Step 3: Compute stats.
    let total_pass_count = linked_passes.len();
    let total_point_count: usize = linked_passes.iter().map(|p| p.cuts.len()).sum();
    let total_path_length_mm: f64 = linked_passes
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

    // Step 4: Assemble Toolpath.
    let spindle_speed = operation
        .spindle_speed_override
        .map(|v| v as f64)
        .or_else(|| tool.default_spindle_speed.map(|v| v as f64))
        .unwrap_or(8000.0);
    let feed_rate = operation
        .feed_rate_override
        .or(tool.default_feed_rate)
        .unwrap_or(500.0);
    let toolpath = Toolpath {
        operation_id: operation.id,
        tool_number: 1,
        spindle_speed,
        feed_rate,
        passes: linked_passes,
    };

    let stats = ToolpathStats {
        total_pass_count,
        total_point_count,
        total_path_length_mm,
    };

    Ok((toolpath, stats))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::operation::{
        CacheState, CompensationSide, DrillParams, DrillPoint, OperationParams, PocketParams,
        ProfileParams,
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
            }),
            cache: CacheState::default(),
        };
        let tool = make_tool_10mm();
        let stock = make_stock_50x50x10();
        let (_, stats) = plan(&operation, &tool, &stock).expect("pocket plan should succeed");
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
                stepdown: 2.5,
                compensation_side: CompensationSide::Left,
            }),
            cache: CacheState::default(),
        };
        let tool = make_tool_10mm();
        let stock = make_stock_50x50x10();
        let (_, stats) = plan(&operation, &tool, &stock).expect("profile plan should succeed");
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
                stepdown: 2.0,
                compensation_side: CompensationSide::Left,
            }),
            cache: CacheState::default(),
        };
        let tool = make_tool_10mm();
        let stock = make_stock_50x50x10();
        assert!(plan(&operation, &tool, &stock).is_err());
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
    fn plan_uses_spindle_speed_override_when_set() {
        let operation = make_drill_operation(Some(12000), None);
        let tool = Tool {
            default_spindle_speed: Some(8000),
            ..make_tool_10mm()
        };
        let stock = make_stock_50x50x10();
        let (toolpath, _) = plan(&operation, &tool, &stock).expect("drill plan should succeed");
        assert_eq!(toolpath.spindle_speed, 12000.0);
    }

    #[test]
    fn plan_uses_tool_default_when_no_spindle_override() {
        let operation = make_drill_operation(None, None);
        let tool = Tool {
            default_spindle_speed: Some(9000),
            ..make_tool_10mm()
        };
        let stock = make_stock_50x50x10();
        let (toolpath, _) = plan(&operation, &tool, &stock).expect("drill plan should succeed");
        assert_eq!(toolpath.spindle_speed, 9000.0);
    }

    #[test]
    fn plan_uses_hardcoded_spindle_fallback_when_neither_set() {
        let operation = make_drill_operation(None, None);
        let tool = make_tool_10mm(); // default_spindle_speed: None
        let stock = make_stock_50x50x10();
        let (toolpath, _) = plan(&operation, &tool, &stock).expect("drill plan should succeed");
        assert_eq!(toolpath.spindle_speed, 8000.0);
    }

    #[test]
    fn plan_uses_feed_rate_override_when_set() {
        let operation = make_drill_operation(None, Some(800.0));
        let tool = Tool {
            default_feed_rate: Some(500.0),
            ..make_tool_10mm()
        };
        let stock = make_stock_50x50x10();
        let (toolpath, _) = plan(&operation, &tool, &stock).expect("drill plan should succeed");
        assert_eq!(toolpath.feed_rate, 800.0);
    }

    #[test]
    fn plan_uses_tool_default_feed_rate_when_no_override() {
        let operation = make_drill_operation(None, None);
        let tool = Tool {
            default_feed_rate: Some(300.0),
            ..make_tool_10mm()
        };
        let stock = make_stock_50x50x10();
        let (toolpath, _) = plan(&operation, &tool, &stock).expect("drill plan should succeed");
        assert_eq!(toolpath.feed_rate, 300.0);
    }

    #[test]
    fn plan_uses_hardcoded_feed_rate_fallback_when_neither_set() {
        let operation = make_drill_operation(None, None);
        let tool = make_tool_10mm(); // default_feed_rate: None
        let stock = make_stock_50x50x10();
        let (toolpath, _) = plan(&operation, &tool, &stock).expect("drill plan should succeed");
        assert_eq!(toolpath.feed_rate, 500.0);
    }
}
