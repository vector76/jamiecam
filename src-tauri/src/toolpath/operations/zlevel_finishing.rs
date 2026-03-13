//! Z-Level finishing algorithm.
//!
//! Generates wall-following contour passes at each Z level. Unlike roughing,
//! which fills the interior with concentric offsets, finishing produces a
//! single contour per Z level offset inward by the tool radius plus a
//! finishing allowance.

use crate::error::AppError;
use crate::geometry::{poly_offset, shape_section_at_z, GeometryError, OcctShape};
use crate::models::operation::ZLevelFinishingParams;
use crate::models::{StockDefinition, Vec3};
use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind};

/// Data from a prior roughing operation, used for rest machining.
pub struct RoughingData {
    /// Cutting-only passes from the roughing operation.
    pub passes: Vec<Pass>,
    /// Roughing tool diameter.
    pub tool_diameter: f64,
}

/// Generate finishing passes for a Z-level finishing operation.
///
/// At each Z level, the boundary is derived from `shape` (if provided) or the
/// stock extents. The boundary is offset inward by `tool_diameter / 2 +
/// finishing_allowance` to produce a single contour pass. If `spring_pass` is
/// enabled, a second pass offset by `tool_diameter / 2` only is also emitted.
///
/// # Errors
/// - [`AppError::InvalidInput`] if any params are out of range.
/// - [`AppError::GeometryImport`] if the geometry kernel reports an unexpected error.
pub fn zlevel_finishing_passes(
    stock: &StockDefinition,
    params: &ZLevelFinishingParams,
    tool_diameter: f64,
    shape: Option<&OcctShape>,
    roughing_data: Option<&RoughingData>,
) -> Result<Vec<Pass>, AppError> {
    if params.depth <= 0.0 {
        return Err(AppError::InvalidInput(
            "depth must be greater than zero".into(),
        ));
    }
    if params.stepdown <= 0.0 {
        return Err(AppError::InvalidInput(
            "stepdown must be greater than zero".into(),
        ));
    }
    if params.finishing_allowance < 0.0 {
        return Err(AppError::InvalidInput(
            "finishing allowance must not be negative".into(),
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

    let floor_z = stock_top_z - params.depth;
    let mut passes = Vec::new();
    let mut n = 1usize;

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

        // Rest machining: skip Z levels fully covered by roughing
        if let Some(rd) = roughing_data {
            // Compute target boundary (shape offset inward by finishing tool radius)
            let target_delta = -(tool_diameter / 2.0);
            let target_boundary = match poly_offset(&boundary, target_delta, 0.01) {
                Ok(tb) => tb,
                Err(_) => {
                    // Target boundary collapsed — skip this Z level
                    if z <= floor_z {
                        break;
                    }
                    n += 1;
                    continue;
                }
            };

            // Collect roughing contours at this Z level
            const Z_TOL: f64 = 1e-6;
            let roughing_contours_at_z: Vec<Vec<(f64, f64)>> = rd
                .passes
                .iter()
                .filter(|pass| {
                    pass.cuts
                        .first()
                        .map(|c| (c.position.z - z).abs() < Z_TOL)
                        .unwrap_or(false)
                })
                .map(|pass| {
                    pass.cuts
                        .iter()
                        .map(|c| (c.position.x, c.position.y))
                        .collect()
                })
                .collect();

            let rest = crate::toolpath::rest::compute_rest_region(
                &target_boundary,
                &roughing_contours_at_z,
                rd.tool_diameter / 2.0,
            )
            .map_err(AppError::from)?;

            if rest.is_empty() {
                // Roughing fully covers this Z level — no finishing needed
                if z <= floor_z {
                    break;
                }
                n += 1;
                continue;
            }
        }

        // Finishing contour: offset inward by tool radius + finishing allowance
        let finishing_delta = -(tool_diameter / 2.0 + params.finishing_allowance);
        match poly_offset(&boundary, finishing_delta, 0.01) {
            Ok(contour) => {
                passes.push(contour_pass(&contour, z, PassKind::Cutting));
            }
            Err(_) => {
                // Offset collapsed — skip this Z level
                if z <= floor_z {
                    break;
                }
                n += 1;
                continue;
            }
        }

        // Optional spring pass: offset by tool radius only (zero finishing allowance)
        if params.spring_pass {
            let spring_delta = -(tool_diameter / 2.0);
            if let Ok(contour) = poly_offset(&boundary, spring_delta, 0.01) {
                passes.push(contour_pass(&contour, z, PassKind::SpringPass));
            }
        }

        if z <= floor_z {
            break;
        }
        n += 1;
    }

    Ok(passes)
}

fn contour_pass(contour: &[(f64, f64)], z: f64, kind: PassKind) -> Pass {
    Pass {
        kind,
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
    use crate::models::operation::ZLevelFinishingParams;
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

    fn make_params(
        depth: f64,
        stepdown: f64,
        finishing_allowance: f64,
        spring_pass: bool,
    ) -> ZLevelFinishingParams {
        ZLevelFinishingParams {
            depth,
            stepdown,
            finishing_allowance,
            spring_pass,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
            rest_machining: false,
            rest_machining_reference_id: None,
        }
    }

    // --- Ungated parameter validation tests ---

    #[test]
    fn rejects_non_positive_depth() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(0.0, 1.0, 0.1, false);
        assert!(matches!(
            zlevel_finishing_passes(&stock, &params, 5.0, None, None),
            Err(AppError::InvalidInput(_))
        ));
        let params_neg = make_params(-1.0, 1.0, 0.1, false);
        assert!(matches!(
            zlevel_finishing_passes(&stock, &params_neg, 5.0, None, None),
            Err(AppError::InvalidInput(_))
        ));
    }

    #[test]
    fn rejects_non_positive_stepdown() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(5.0, 0.0, 0.1, false);
        assert!(matches!(
            zlevel_finishing_passes(&stock, &params, 5.0, None, None),
            Err(AppError::InvalidInput(_))
        ));
        let params_neg = make_params(5.0, -1.0, 0.1, false);
        assert!(matches!(
            zlevel_finishing_passes(&stock, &params_neg, 5.0, None, None),
            Err(AppError::InvalidInput(_))
        ));
    }

    #[test]
    fn rejects_negative_finishing_allowance() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(5.0, 1.0, -0.1, false);
        assert!(matches!(
            zlevel_finishing_passes(&stock, &params, 5.0, None, None),
            Err(AppError::InvalidInput(_))
        ));
    }

    // --- Gated algorithm tests (require geometry bindings) ---

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn single_z_level_produces_one_pass() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(2.0, 2.0, 0.1, false);
        let passes =
            zlevel_finishing_passes(&stock, &params, 5.0, None, None).expect("should succeed");
        assert_eq!(
            passes.len(),
            1,
            "depth==stepdown should produce exactly 1 pass"
        );
        assert_eq!(passes[0].kind, PassKind::Cutting);
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn multiple_z_levels_correct_count() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        // depth=6, stepdown=2, stock_top=10, floor=4
        // Finishing starts at n=1: z=8 (n=1), z=6 (n=2), z=4 (n=3, floor) → 3 passes
        let params = make_params(6.0, 2.0, 0.1, false);
        let passes =
            zlevel_finishing_passes(&stock, &params, 5.0, None, None).expect("should succeed");
        assert_eq!(passes.len(), 3, "expected 3 Cutting passes");
        for pass in &passes {
            assert_eq!(pass.kind, PassKind::Cutting);
        }
        // Verify Z heights
        let z_values: Vec<i64> = passes
            .iter()
            .map(|p| (p.cuts[0].position.z * 1000.0) as i64)
            .collect();
        assert!(z_values.contains(&8000)); // z=8
        assert!(z_values.contains(&6000)); // z=6
        assert!(z_values.contains(&4000)); // z=4 (floor)
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn spring_pass_doubles_count() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(2.0, 2.0, 0.1, true);
        let passes =
            zlevel_finishing_passes(&stock, &params, 5.0, None, None).expect("should succeed");
        // 1 Z level, spring_pass=true → 2 passes (Cutting + SpringPass)
        assert_eq!(passes.len(), 2);
        assert_eq!(passes[0].kind, PassKind::Cutting);
        assert_eq!(passes[1].kind, PassKind::SpringPass);
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn zero_finishing_allowance_offsets_by_tool_radius() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(2.0, 2.0, 0.0, false);
        let tool_diameter = 6.0;
        let passes = zlevel_finishing_passes(&stock, &params, tool_diameter, None, None)
            .expect("should succeed");
        assert_eq!(passes.len(), 1);
        // With finishing_allowance=0, offset = tool_diameter/2 = 3.0
        // Stock boundary is (0,0)-(50,0)-(50,50)-(0,50)
        // After inward offset of 3.0: (3,3)-(47,3)-(47,47)-(3,47)
        let cuts = &passes[0].cuts;
        for cut in cuts {
            assert!(
                cut.position.x >= 3.0 - 0.1 && cut.position.x <= 47.0 + 0.1,
                "x={} out of expected range",
                cut.position.x
            );
            assert!(
                cut.position.y >= 3.0 - 0.1 && cut.position.y <= 47.0 + 0.1,
                "y={} out of expected range",
                cut.position.y
            );
        }
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn floor_z_always_machined() {
        let stock = make_box_stock(50.0, 50.0, 10.0);
        // depth=5, stepdown=2: floor=5. z=8,6,5 → floor is machined
        let params = make_params(5.0, 2.0, 0.1, false);
        let passes =
            zlevel_finishing_passes(&stock, &params, 5.0, None, None).expect("should succeed");
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
    fn collapsed_section_skipped() {
        let stock = make_box_stock(10.0, 10.0, 5.0);
        // tool_diameter=50 → offset of 25+0.1 > stock size → collapses
        let params = make_params(5.0, 2.0, 0.1, false);
        let passes =
            zlevel_finishing_passes(&stock, &params, 50.0, None, None).expect("should succeed");
        assert!(
            passes.is_empty(),
            "expected empty passes when offset collapses"
        );
    }

    // --- Gated geometry tests (use shape: Some) ---

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn geometry_uses_shape_sections() {
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
        let params = make_params(5.0, 2.0, 0.1, false);
        let passes = zlevel_finishing_passes(&stock, &params, 6.0, Some(&shape), None).unwrap();
        assert!(!passes.is_empty());
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
    fn geometry_collapses_gracefully() {
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
        // Tool diameter much larger than shape → should collapse gracefully
        let params = make_params(5.0, 2.0, 0.1, false);
        let passes = zlevel_finishing_passes(&stock, &params, 500.0, Some(&shape), None).unwrap();
        assert!(
            passes.is_empty(),
            "expected no passes when tool is too large for shape"
        );
    }

    // --- Rest machining tests ---

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn rest_machining_full_coverage_no_passes() {
        // Roughing covers all Z levels → no finishing passes needed.
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(4.0, 2.0, 0.1, false);
        let finishing_tool_diameter = 6.0;
        let roughing_tool_diameter = 10.0;

        // stock_top=10, floor=6, Z levels: 8, 6
        // Create roughing passes that fully cover each Z level.
        // Roughing contour covers the entire stock boundary at each Z.
        let roughing_passes = vec![
            contour_pass(
                &[(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
                8.0,
                PassKind::Cutting,
            ),
            contour_pass(
                &[(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
                6.0,
                PassKind::Cutting,
            ),
        ];
        let rd = RoughingData {
            passes: roughing_passes,
            tool_diameter: roughing_tool_diameter,
        };

        let passes =
            zlevel_finishing_passes(&stock, &params, finishing_tool_diameter, None, Some(&rd))
                .expect("should succeed");
        assert!(
            passes.is_empty(),
            "expected no passes when roughing fully covers all Z levels, got {}",
            passes.len()
        );
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn rest_machining_partial_coverage() {
        // Roughing covers only one Z level → passes emitted at uncovered levels.
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(4.0, 2.0, 0.1, false);
        let finishing_tool_diameter = 6.0;
        let roughing_tool_diameter = 10.0;

        // stock_top=10, floor=6, Z levels: 8, 6
        // Only provide roughing at Z=8, not at Z=6.
        let roughing_passes = vec![contour_pass(
            &[(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
            8.0,
            PassKind::Cutting,
        )];
        let rd = RoughingData {
            passes: roughing_passes,
            tool_diameter: roughing_tool_diameter,
        };

        let passes =
            zlevel_finishing_passes(&stock, &params, finishing_tool_diameter, None, Some(&rd))
                .expect("should succeed");
        // Z=8 is fully covered by roughing → skipped.
        // Z=6 has no roughing → finishing pass emitted.
        assert_eq!(
            passes.len(),
            1,
            "expected 1 pass at uncovered Z level, got {}",
            passes.len()
        );
        let z = passes[0].cuts[0].position.z;
        assert!((z - 6.0).abs() < 1e-6, "expected pass at z=6.0, got z={z}");
    }

    #[cfg(cam_geometry_bindings)]
    #[test]
    fn rest_machining_no_reference_acts_as_full() {
        // roughing_data: None → all Z levels get passes (same as without rest machining).
        let stock = make_box_stock(50.0, 50.0, 10.0);
        let params = make_params(4.0, 2.0, 0.1, false);
        let finishing_tool_diameter = 6.0;

        let passes_none =
            zlevel_finishing_passes(&stock, &params, finishing_tool_diameter, None, None)
                .expect("should succeed");
        // stock_top=10, floor=6, Z levels: 8, 6 → 2 passes
        assert_eq!(
            passes_none.len(),
            2,
            "expected 2 passes with no roughing data, got {}",
            passes_none.len()
        );
    }
}
