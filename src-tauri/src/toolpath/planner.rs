//! Toolpath planner — entry-point for turning an [`Operation`] into a [`Toolpath`].

use crate::error::AppError;
use crate::models::operation::OperationParams;
use crate::models::{Operation, StockDefinition, Tool};
use crate::toolpath::{
    linking, operations,
    types::{Toolpath, ToolpathStats},
};

/// Generate a [`Toolpath`] and [`ToolpathStats`] for the given operation.
///
/// Returns [`AppError::NotFound`] for operation types that are not yet
/// supported (Profile, Drill).
pub fn plan(
    operation: &Operation,
    tool: &Tool,
    stock: &StockDefinition,
) -> Result<(Toolpath, ToolpathStats), AppError> {
    // Step 1: Generate cutting passes based on operation type.
    let passes = match &operation.params {
        OperationParams::Pocket(params) => {
            operations::pocket::pocket_passes(stock, params, tool.diameter)?
        }
        OperationParams::Profile(_) | OperationParams::Drill(_) => {
            return Err(AppError::NotFound(
                "operation type not supported".to_string(),
            ));
        }
    };

    // Step 2: Compute clearance height and link passes.
    let stock_top_z = match stock {
        StockDefinition::Box(b) => b.origin.z + b.height,
    };
    let linked_passes = linking::link_passes(passes, tool.diameter, stock_top_z + 5.0);

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
    let toolpath = Toolpath {
        operation_id: operation.id,
        tool_number: 1,
        spindle_speed: tool.default_spindle_speed.unwrap_or(8000) as f64,
        feed_rate: tool.default_feed_rate.unwrap_or(500.0),
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
        CompensationSide, OperationParams, PocketParams, ProfileParams,
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

    #[test]
    fn plan_returns_error_for_profile() {
        let operation = Operation {
            id: Uuid::nil(),
            name: "Profile Op".to_string(),
            enabled: true,
            tool_id: Uuid::nil(),
            params: OperationParams::Profile(ProfileParams {
                depth: 5.0,
                stepdown: 2.0,
                compensation_side: CompensationSide::Center,
            }),
        };
        let tool = make_tool_10mm();
        let stock = make_stock_50x50x10();
        assert!(plan(&operation, &tool, &stock).is_err());
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn plan_stats_are_non_zero_for_pocket() {
        let operation = Operation {
            id: Uuid::nil(),
            name: "Pocket Op".to_string(),
            enabled: true,
            tool_id: Uuid::nil(),
            params: OperationParams::Pocket(PocketParams {
                depth: 10.0,
                stepdown: 2.0,
                stepover_percent: 50.0,
            }),
        };
        let tool = make_tool_10mm();
        let stock = make_stock_50x50x10();
        let (_, stats) = plan(&operation, &tool, &stock).expect("pocket plan should succeed");
        assert!(stats.total_pass_count > 0);
        assert!(stats.total_path_length_mm > 0.0);
    }
}
