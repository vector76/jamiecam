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

    match params.stepdown {
        Some(sd) if sd > 0.0 => {
            let mut n = 1usize;
            loop {
                let current_z = (stock_top_z - n as f64 * sd).max(floor_z);
                passes.push(contour_pass(&contour, current_z));
                if current_z <= floor_z {
                    break;
                }
                n += 1;
            }
        }
        _ => {
            passes.push(contour_pass(&contour, floor_z));
        }
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
            stepdown: Some(2.5),
            compensation_side: CompensationSide::Left,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
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
            stepdown: Some(2.5),
            compensation_side: CompensationSide::Left,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
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
            stepdown: Some(2.0),
            compensation_side: CompensationSide::Left,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
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
            stepdown: Some(5.0),
            compensation_side: CompensationSide::Left,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let params_center = ProfileParams {
            depth: 5.0,
            stepdown: Some(5.0),
            compensation_side: CompensationSide::Center,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
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
            stepdown: Some(5.0),
            compensation_side: CompensationSide::Center,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
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

    fn make_stock_10() -> StockDefinition {
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

    #[test]
    fn profile_none_stepdown_produces_single_pass_at_floor() {
        let stock = make_stock_10();
        let params = ProfileParams {
            depth: 10.0,
            stepdown: None,
            compensation_side: CompensationSide::Center,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let boundary = vec![(0.0_f64, 0.0_f64), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)];
        let passes = profile_passes(&stock, &params, 6.0, &boundary).expect("should succeed");
        assert_eq!(
            passes.len(),
            1,
            "None stepdown must produce exactly one pass"
        );
        // stock origin z=0, height=10 → top=10, floor=top-depth=0.0
        let z = passes[0].cuts[0].position.z;
        assert!(
            (z - 0.0_f64).abs() < 1e-9,
            "single pass must be at floor z=0.0, got {z}"
        );
    }

    #[test]
    fn profile_zero_stepdown_produces_single_pass_at_floor() {
        let stock = make_stock_10();
        let params = ProfileParams {
            depth: 10.0,
            stepdown: Some(0.0),
            compensation_side: CompensationSide::Center,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let boundary = vec![(0.0_f64, 0.0_f64), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)];
        let passes = profile_passes(&stock, &params, 6.0, &boundary).expect("should succeed");
        assert_eq!(
            passes.len(),
            1,
            "zero stepdown must produce exactly one pass"
        );
        let z = passes[0].cuts[0].position.z;
        assert!(
            (z - 0.0_f64).abs() < 1e-9,
            "single pass must be at floor z=0.0, got {z}"
        );
    }

    #[test]
    fn profile_negative_stepdown_produces_single_pass_at_floor() {
        let stock = make_stock_10();
        let params = ProfileParams {
            depth: 10.0,
            stepdown: Some(-1.0),
            compensation_side: CompensationSide::Center,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let boundary = vec![(0.0_f64, 0.0_f64), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)];
        let passes = profile_passes(&stock, &params, 6.0, &boundary).expect("should succeed");
        assert_eq!(
            passes.len(),
            1,
            "negative stepdown must produce exactly one pass"
        );
    }

    #[test]
    fn profile_stepdown_absent_from_json_when_none() {
        use crate::models::operation::ProfileParams;
        let params = ProfileParams {
            depth: 10.0,
            stepdown: None,
            compensation_side: CompensationSide::Center,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let value = serde_json::to_value(&params).expect("to_value");
        assert!(
            value.get("stepdown").is_none(),
            "stepdown must be absent from JSON when None"
        );
    }

    #[test]
    fn profile_stepdown_backward_compat_deserialize() {
        // Old JSON with a numeric stepdown field must still deserialize to Some(v)
        let json = r#"{"depth": 10.0, "stepdown": 2.5, "compensationSide": "center"}"#;
        let params: ProfileParams = serde_json::from_str(json).expect("deserialize");
        assert_eq!(params.stepdown, Some(2.5));
    }

    #[test]
    fn profile_stepdown_none_single_z_level() {
        // stepdown: None → exactly one Z level (single pass at floor depth)
        let stock = make_stock_10();
        let params = ProfileParams {
            depth: 10.0,
            stepdown: None,
            compensation_side: CompensationSide::Center,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let boundary = vec![(0.0_f64, 0.0_f64), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)];
        let passes = profile_passes(&stock, &params, 6.0, &boundary).expect("should succeed");
        let z_set: std::collections::HashSet<i64> = passes
            .iter()
            .flat_map(|p| p.cuts.iter())
            .map(|c| (c.position.z * 1000.0) as i64)
            .collect();
        assert_eq!(
            z_set.len(),
            1,
            "None stepdown must produce exactly 1 Z level, got {}",
            z_set.len()
        );
    }

    #[test]
    fn profile_stepdown_2_depth_8_produces_four_passes() {
        // stock height=10 → stock_top_z=10, depth=8 → floor_z=2
        // stepdown=2.0 → passes at z=8, 6, 4, 2 (exactly 4)
        let stock = make_stock_10();
        let params = ProfileParams {
            depth: 8.0,
            stepdown: Some(2.0),
            compensation_side: CompensationSide::Center,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let boundary = vec![(0.0_f64, 0.0_f64), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)];
        let passes = profile_passes(&stock, &params, 6.0, &boundary).expect("should succeed");
        assert_eq!(
            passes.len(),
            4,
            "stepdown=2, depth=8 must produce exactly 4 passes, got {}",
            passes.len()
        );
        let z_values: Vec<f64> = passes.iter().map(|p| p.cuts[0].position.z).collect();
        let expected = [8.0, 6.0, 4.0, 2.0];
        for (got, exp) in z_values.iter().zip(expected.iter()) {
            assert!((got - exp).abs() < 1e-9, "expected Z={exp}, got Z={got}");
        }
    }

    #[test]
    fn profile_stepdown_3_depth_8_final_pass_at_floor() {
        // stock height=10 → stock_top_z=10, depth=8 → floor_z=2
        // stepdown=3.0 → passes at z=7, 4, then clamped to floor_z=2 (3 passes total)
        let stock = make_stock_10();
        let params = ProfileParams {
            depth: 8.0,
            stepdown: Some(3.0),
            compensation_side: CompensationSide::Center,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let boundary = vec![(0.0_f64, 0.0_f64), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)];
        let passes = profile_passes(&stock, &params, 6.0, &boundary).expect("should succeed");
        assert_eq!(
            passes.len(),
            3,
            "stepdown=3, depth=8 must produce exactly 3 passes, got {}",
            passes.len()
        );
        let last_z = passes.last().unwrap().cuts[0].position.z;
        assert!(
            (last_z - 2.0_f64).abs() < 1e-9,
            "final pass must be at exact floor_z=2.0, got {last_z}"
        );
    }
}
