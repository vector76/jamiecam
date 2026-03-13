//! Z-Level roughing algorithm.
//!
//! Generates cutting passes for a Z-level roughing operation by slicing the
//! workpiece at each depth step and computing concentric contour offsets.

use crate::error::AppError;
use crate::geometry::{poly_offset, shape_section_at_z, GeometryError, OcctShape};
use crate::models::operation::ZLevelRoughingParams;
use crate::models::{StockDefinition, Vec3};
use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind};

/// Generate cutting passes for a Z-level roughing operation.
///
/// At each Z level, the boundary is derived from `shape` (if provided) or the
/// stock extents. The boundary is offset inward by the tool radius, then
/// repeatedly offset inward by the stepover distance until the polygon
/// collapses.
///
/// # Errors
/// - [`AppError::InvalidInput`] if any params are out of range.
/// - [`AppError::GeometryImport`] if the initial inward offset collapses or the
///   geometry kernel reports an unexpected error.
pub fn zlevel_roughing_passes(
    stock: &StockDefinition,
    params: &ZLevelRoughingParams,
    tool_diameter: f64,
    shape: Option<&OcctShape>,
) -> Result<Vec<Pass>, AppError> {
    if params.stepdown <= 0.0 {
        return Err(AppError::InvalidInput(
            "stepdown must be greater than zero".into(),
        ));
    }
    if params.depth <= 0.0 {
        return Err(AppError::InvalidInput(
            "depth must be greater than zero".into(),
        ));
    }
    if params.stepover <= 0.0 || params.stepover > 1.0 {
        return Err(AppError::InvalidInput(
            "stepover must be in range (0, 1]".into(),
        ));
    }

    let StockDefinition::Box(b) = stock;
    let stock_top_z = b.origin.z + b.height;

    let stock_boundary: Vec<(f64, f64)> = vec![
        (b.origin.x, b.origin.y),
        (b.origin.x + b.width, b.origin.y),
        (b.origin.x + b.width, b.origin.y + b.depth),
        (b.origin.x, b.origin.y + b.depth),
    ];

    let stepover = tool_diameter * params.stepover;
    let floor_z = stock_top_z - params.depth;
    let mut passes = Vec::new();
    let mut n = 0usize;

    loop {
        let z = (stock_top_z - n as f64 * params.stepdown).max(floor_z);

        let boundary: Vec<(f64, f64)> = if let Some(s) = shape {
            match shape_section_at_z(s, z) {
                Ok(loops) if loops.is_empty() => {
                    if z <= floor_z {
                        break;
                    }
                    n += 1;
                    continue;
                }
                Ok(loops) => loops[0].clone(),
                Err(GeometryError::NotImplemented) => stock_boundary.clone(),
                Err(e) => return Err(AppError::from(e)),
            }
        } else {
            stock_boundary.clone()
        };

        let first_contour = poly_offset(&boundary, -(tool_diameter / 2.0), 0.01)?;
        passes.push(contour_pass(&first_contour, z));

        let mut current = first_contour;
        while let Ok(next) = poly_offset(&current, -stepover, 0.01) {
            passes.push(contour_pass(&next, z));
            current = next;
        }

        if z <= floor_z {
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
mod tests {
    use super::*;
    use crate::models::operation::ZLevelRoughingParams;
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

    #[test]
    fn rejects_zero_stepdown() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = ZLevelRoughingParams {
            depth: 5.0,
            stepdown: 0.0,
            stepover: 0.5,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let result = zlevel_roughing_passes(&stock, &params, 5.0, None);
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn rejects_zero_depth() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = ZLevelRoughingParams {
            depth: 0.0,
            stepdown: 1.0,
            stepover: 0.5,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let result = zlevel_roughing_passes(&stock, &params, 5.0, None);
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn rejects_stepover_out_of_range() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params_zero = ZLevelRoughingParams {
            depth: 5.0,
            stepdown: 1.0,
            stepover: 0.0,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        assert!(matches!(
            zlevel_roughing_passes(&stock, &params_zero, 5.0, None),
            Err(AppError::InvalidInput(_))
        ));

        let params_over = ZLevelRoughingParams {
            depth: 5.0,
            stepdown: 1.0,
            stepover: 1.1,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        assert!(matches!(
            zlevel_roughing_passes(&stock, &params_over, 5.0, None),
            Err(AppError::InvalidInput(_))
        ));
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn produces_passes_for_simple_stock() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = ZLevelRoughingParams {
            depth: 4.0,
            stepdown: 2.0,
            stepover: 0.5,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let passes = zlevel_roughing_passes(&stock, &params, 5.0, None).expect("should succeed");
        assert!(!passes.is_empty(), "expected non-empty passes");
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn z_levels_span_depth() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = ZLevelRoughingParams {
            depth: 6.0,
            stepdown: 2.0,
            stepover: 0.5,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let passes = zlevel_roughing_passes(&stock, &params, 5.0, None).expect("should succeed");

        let mut z_set = std::collections::HashSet::new();
        for pass in &passes {
            for cut in &pass.cuts {
                z_set.insert((cut.position.z * 1000.0) as i64);
            }
        }
        // depth=6, stepdown=2, stock_top_z=10 → z levels: 10, 8, 6, 4 (exactly 4)
        assert!(
            z_set.len() >= 4,
            "expected at least 4 Z levels, got {}",
            z_set.len()
        );
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn floor_z_always_machined_when_depth_not_multiple_of_stepdown() {
        // depth=5, stepdown=2: naive float loop would produce z=10,8,6 and stop
        // (since 10-5=5, and 10-3*2=4 < 5). The floor at z=5 must still be machined.
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = ZLevelRoughingParams {
            depth: 5.0,
            stepdown: 2.0,
            stepover: 0.5,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let passes = zlevel_roughing_passes(&stock, &params, 5.0, None).expect("should succeed");

        let floor_z_millis = ((10.0_f64 - 5.0) * 1000.0) as i64; // 5000
        let z_set: std::collections::HashSet<i64> = passes
            .iter()
            .flat_map(|p| p.cuts.iter())
            .map(|c| (c.position.z * 1000.0) as i64)
            .collect();
        assert!(
            z_set.contains(&floor_z_millis),
            "floor z=5 must be present; found z levels: {z_set:?}"
        );
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn zlr_stock_boundary_produces_correct_z_levels() {
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
        let params = ZLevelRoughingParams {
            depth: 6.0,
            stepdown: 2.0,
            stepover: 0.5,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let passes = zlevel_roughing_passes(&stock, &params, 6.0, None).unwrap();
        assert!(!passes.is_empty());
        // stock_top_z = 10.0; Z levels: 10, 8, 6, 4 (all >= 10 - 6 = 4.0) -> 4 levels
        let unique_zs: std::collections::HashSet<_> = passes
            .iter()
            .map(|p| (p.cuts[0].position.z * 1000.0) as i64)
            .collect();
        assert_eq!(unique_zs.len(), 4);
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn zlr_geometry_uses_section_boundaries() {
        use crate::geometry::OcctShape;
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/box.step"
        ));
        let shape = OcctShape::load_step(path).expect("load box.step");
        let (xmin, ymin, zmin, xmax, ymax, zmax) = shape.bounding_box();
        let stock = StockDefinition::Box(BoxDimensions {
            origin: Vec3 {
                x: xmin,
                y: ymin,
                z: zmin,
            },
            width: xmax - xmin,
            depth: ymax - ymin,
            height: zmax - zmin,
        });
        let params = ZLevelRoughingParams {
            depth: 5.0,
            stepdown: 2.0,
            stepover: 0.4,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let passes = zlevel_roughing_passes(&stock, &params, 6.0, Some(&shape)).unwrap();
        assert!(!passes.is_empty());
        // all cut Z values must lie within [zmax - 5.0, zmax]
        for pass in &passes {
            for cut in &pass.cuts {
                assert!(
                    cut.position.z >= zmax - 5.0 - f64::EPSILON
                        && cut.position.z <= zmax + f64::EPSILON
                );
            }
        }
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn collapses_when_tool_too_large() {
        let stock = make_box_stock(10.0, 10.0, 5.0);
        let params = ZLevelRoughingParams {
            depth: 5.0,
            stepdown: 2.0,
            stepover: 0.5,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let result = zlevel_roughing_passes(&stock, &params, 20.0, None);
        assert!(
            result.is_err(),
            "expected Err when tool diameter exceeds stock"
        );
    }
}
