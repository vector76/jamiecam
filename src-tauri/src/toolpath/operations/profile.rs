//! Profile contouring algorithm.
//!
//! Generates cutting passes for a rectangular stock profile operation by
//! computing a single offset contour at each depth step.

use crate::error::AppError;
use crate::geometry::poly_offset;
use crate::models::operation::{CompensationSide, ProfileParams};
use crate::models::{StockDefinition, Vec3};
use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind};

/// Generate cutting passes for a profile operation.
///
/// The stock boundary is offset by the tool radius according to the
/// compensation side to produce the cutting contour. This single contour
/// is repeated at each Z depth level.
///
/// Returns `Err(AppError::GeometryImport(...))` if the offset collapses
/// entirely (e.g. the tool diameter is too large for the stock). Only
/// applies to `Left` and `Right` compensation; `Center` never calls
/// `poly_offset` and will not fail.
pub fn profile_passes(
    stock: &StockDefinition,
    params: &ProfileParams,
    tool_diameter: f64,
    boundary: &[(f64, f64)],
) -> Result<Vec<Pass>, AppError> {
    let StockDefinition::Box(b) = stock;
    let stock_top_z = b.origin.z + b.height;
    let floor_z = stock_top_z - params.depth;

    let contour: Vec<(f64, f64)> = match params.compensation_side {
        CompensationSide::Left => poly_offset(boundary, -(tool_diameter / 2.0), 0.01)?,
        CompensationSide::Right => poly_offset(boundary, tool_diameter / 2.0, 0.01)?,
        CompensationSide::Center => boundary.to_vec(),
    };

    let mut passes = Vec::new();
    let mut n = 1usize;

    loop {
        let current_z = (stock_top_z - n as f64 * params.stepdown).max(floor_z);
        passes.push(contour_pass(&contour, current_z));

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
            })
            .collect(),
    }
}

#[cfg(test)]
#[cfg(cam_geometry_bindings)]
mod tests {
    use super::*;
    use crate::models::operation::{CompensationSide, ProfileParams};
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
    fn profile_z_levels_count() {
        // stock 50×50×10, depth=10, stepdown=2.5, Left compensation, tool 6mm
        // → assert exactly 4 distinct Z levels
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = ProfileParams {
            depth: 10.0,
            stepdown: 2.5,
            compensation_side: CompensationSide::Left,
        };
        let boundary = make_boundary(&stock);
        let passes =
            profile_passes(&stock, &params, 6.0, &boundary).expect("profile_passes should succeed");

        let mut z_set = std::collections::HashSet::new();
        for pass in &passes {
            for cut in &pass.cuts {
                z_set.insert((cut.position.z * 1000.0) as i64);
            }
        }
        assert_eq!(
            z_set.len(),
            4,
            "expected exactly 4 Z levels, got {}",
            z_set.len()
        );
    }

    #[test]
    fn profile_non_empty_for_left_compensation() {
        // stock 50×50×10, depth=10, stepdown=2.5, Left compensation, tool 6mm
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = ProfileParams {
            depth: 10.0,
            stepdown: 2.5,
            compensation_side: CompensationSide::Left,
        };
        let boundary = make_boundary(&stock);
        let passes =
            profile_passes(&stock, &params, 6.0, &boundary).expect("profile_passes should succeed");
        assert!(!passes.is_empty(), "expected non-empty Vec<Pass>");
    }

    #[test]
    fn profile_collapses_when_tool_too_large() {
        // stock 10×10×5, tool 20mm, Left → tool radius 10 > half-width 5 → collapse
        let stock = make_box_stock(10.0, 10.0, 5.0);
        let params = ProfileParams {
            depth: 5.0,
            stepdown: 2.0,
            compensation_side: CompensationSide::Left,
        };
        let boundary = make_boundary(&stock);
        let result = profile_passes(&stock, &params, 20.0, &boundary);
        assert!(
            result.is_err(),
            "expected Err when tool diameter exceeds stock"
        );
    }

    #[test]
    fn profile_left_and_center_produce_different_contours() {
        // same stock, depth=5, stepdown=5; Left vs Center; first pass first cut x must differ
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params_left = ProfileParams {
            depth: 5.0,
            stepdown: 5.0,
            compensation_side: CompensationSide::Left,
        };
        let params_center = ProfileParams {
            depth: 5.0,
            stepdown: 5.0,
            compensation_side: CompensationSide::Center,
        };
        let boundary = make_boundary(&stock);
        let passes_left =
            profile_passes(&stock, &params_left, 6.0, &boundary).expect("Left should succeed");
        let passes_center =
            profile_passes(&stock, &params_center, 6.0, &boundary).expect("Center should succeed");

        let x_left = passes_left[0].cuts[0].position.x;
        let x_center = passes_center[0].cuts[0].position.x;
        assert!(
            (x_left - x_center).abs() > 1e-9,
            "Left and Center must produce different first cut x: left={x_left}, center={x_center}"
        );
    }
}

#[cfg(test)]
mod tests_no_bindings {
    use super::*;
    use crate::models::operation::{CompensationSide, ProfileParams};
    use crate::models::stock::{BoxDimensions, StockDefinition};
    use crate::models::Vec3;

    #[test]
    fn profile_center_compensation_uses_raw_boundary() {
        let stock = StockDefinition::Box(BoxDimensions {
            origin: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            width: 50.0,
            depth: 50.0,
            height: 10.0,
        });
        let params = ProfileParams {
            depth: 5.0,
            stepdown: 5.0,
            compensation_side: CompensationSide::Center,
        };
        let boundary = vec![(0.0_f64, 0.0_f64), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)];
        let passes = profile_passes(&stock, &params, 6.0, &boundary)
            .expect("Center compensation should never fail");
        assert!(!passes.is_empty());
        // First cut point x must equal raw boundary origin x (0.0), not an offset value.
        let first_x = passes[0].cuts[0].position.x;
        assert!(
            (first_x - 0.0_f64).abs() < 1e-9,
            "Center compensation must use raw boundary x=0.0, got {first_x}"
        );
    }
}
