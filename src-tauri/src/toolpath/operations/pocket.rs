//! Pocket clearing algorithm.
//!
//! Generates cutting passes for a rectangular stock pocket operation by
//! computing concentric contour offsets at each depth step.

use crate::error::AppError;
use crate::geometry::poly_offset;
use crate::models::operation::PocketParams;
use crate::models::{StockDefinition, Vec3};
use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind};

/// Generate cutting passes for a pocket operation.
///
/// The stock boundary is offset inward by the tool radius to produce the first
/// cutting contour, then repeatedly offset inward by the stepover distance until
/// the polygon collapses. This is repeated for each Z depth level.
///
/// Returns `Err(AppError::GeometryImport(...))` if the initial inward offset
/// collapses entirely (e.g. the tool diameter is too large for the stock).
pub fn pocket_passes(
    stock: &StockDefinition,
    params: &PocketParams,
    tool_diameter: f64,
    boundary: &[(f64, f64)],
) -> Result<Vec<Pass>, AppError> {
    let StockDefinition::Box(b) = stock;
    let stock_top_z = b.origin.z + b.height;
    let floor_z = stock_top_z - params.depth;

    let stepover = tool_diameter * params.stepover_percent / 100.0;
    let mut passes = Vec::new();
    let mut n = 0usize;

    loop {
        let current_z = (stock_top_z - n as f64 * params.stepdown).max(floor_z);

        // First contour: inward offset by tool radius. The `?` propagates
        // collapse as AppError::GeometryImport via From<GeometryError>.
        let first_contour = poly_offset(boundary, -(tool_diameter / 2.0), 0.01)?;
        passes.push(contour_pass(&first_contour, current_z));

        // Subsequent concentric contours: offset inward by stepover until
        // the polygon collapses. Err is the expected termination signal here,
        // not a fatal error, so we do NOT use `?`.
        let mut current = first_contour;
        while let Ok(next) = poly_offset(&current, -stepover, 0.01) {
            passes.push(contour_pass(&next, current_z));
            current = next;
        }

        if current_z <= floor_z {
            break;
        }
        n += 1;
    }

    Ok(passes)
}

fn contour_pass(contour: &[(f64, f64)], z: f64) -> Pass {
    Pass {
        kind: PassKind::Cutting,
        cuts: contour
            .iter()
            .map(|&(x, y)| CutPoint {
                position: Vec3 { x, y, z },
                move_kind: MoveKind::Feed,
                tool_orientation: None,
                feed_rate_override: None,
            })
            .collect(),
    }
}

#[cfg(test)]
#[cfg(cam_geometry_bindings)]
mod tests {
    use super::*;
    use crate::models::operation::PocketParams;
    use crate::models::stock::{BoxDimensions, StockDefinition};
    use crate::models::Vec3;

    fn make_box_stock(width: f64, depth: f64, height: f64) -> StockDefinition {
        StockDefinition::Box(BoxDimensions {
            origin: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            width,
            depth,
            height,
        })
    }

    fn make_boundary(stock: &StockDefinition) -> Vec<(f64, f64)> {
        let StockDefinition::Box(b) = stock;
        vec![
            (b.origin.x, b.origin.y),
            (b.origin.x + b.width, b.origin.y),
            (b.origin.x + b.width, b.origin.y + b.depth),
            (b.origin.x, b.origin.y + b.depth),
        ]
    }

    #[test]
    fn pocket_z_levels_count() {
        // stock 50×50×10, depth=10, stepdown=2 → at least 5 distinct Z levels
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = PocketParams {
            depth: 10.0,
            stepdown: 2.0,
            stepover_percent: 50.0,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let boundary = make_boundary(&stock);
        let passes =
            pocket_passes(&stock, &params, 5.0, &boundary).expect("pocket_passes should succeed");

        let mut z_set = std::collections::HashSet::new();
        for pass in &passes {
            for cut in &pass.cuts {
                z_set.insert((cut.position.z * 1000.0) as i64);
            }
        }
        assert!(
            z_set.len() >= 5,
            "expected at least 5 Z levels, got {}",
            z_set.len()
        );
    }

    #[test]
    fn pocket_non_empty_for_small_tool() {
        // stock 50×50×10, depth=2, stepdown=2, tool 10mm, stepover 50%
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = PocketParams {
            depth: 2.0,
            stepdown: 2.0,
            stepover_percent: 50.0,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let boundary = make_boundary(&stock);
        let passes =
            pocket_passes(&stock, &params, 10.0, &boundary).expect("pocket_passes should succeed");
        assert!(!passes.is_empty(), "expected non-empty Vec<Pass>");
    }

    #[test]
    fn pocket_collapses_when_tool_too_large() {
        // stock 10×10×5, tool diameter 20mm → tool radius 10 > half-width 5
        let stock = make_box_stock(10.0, 10.0, 5.0);
        let params = PocketParams {
            depth: 5.0,
            stepdown: 2.0,
            stepover_percent: 50.0,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let boundary = make_boundary(&stock);
        let result = pocket_passes(&stock, &params, 20.0, &boundary);
        assert!(
            result.is_err(),
            "expected Err when tool diameter exceeds stock"
        );
    }
}
