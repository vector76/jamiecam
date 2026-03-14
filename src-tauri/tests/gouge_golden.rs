#![cfg(cam_geometry_bindings)]

use jamiecam_lib::geometry::OcctShape;
use jamiecam_lib::models::operation::ParallelFinishingParams;
use jamiecam_lib::models::stock::BoxDimensions;
use jamiecam_lib::models::{StockDefinition, Vec3};
use jamiecam_lib::toolpath::gouge::{auto_lift_gouges, check_gouges};
use jamiecam_lib::toolpath::operations::parallel_finishing::parallel_finishing_passes;
use jamiecam_lib::toolpath::types::Pass;
use std::path::{Path, PathBuf};

fn sphere_step_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/sphere.step")
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures")
        .join(name)
}

fn load_shape(path: &Path) -> (OcctShape, StockDefinition) {
    let shape = OcctShape::load_step(path).expect("load shape");
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
    (shape, stock)
}

fn sphere_params() -> ParallelFinishingParams {
    ParallelFinishingParams {
        stepover: 5.0,
        direction_angle_deg: 0.0,
        allowance: 0.0,
        geometry: None,
        arc_lead_in_radius: None,
        arc_lead_out_radius: None,
        helical_entry_radius: None,
        helical_entry_pitch: None,
        ramp_entry_angle_deg: None,
    }
}

const TOOL_DIAMETER: f64 = 6.0;
const TOOL_TYPE: &str = "ball_nose";
const ALLOWANCE: f64 = 0.0;

/// Generate parallel finishing passes on the sphere fixture.
fn generate_sphere_passes() -> (Vec<Pass>, OcctShape) {
    let (shape, stock) = load_shape(&sphere_step_path());
    let params = sphere_params();
    let passes = parallel_finishing_passes(&stock, &params, TOOL_DIAMETER, Some(&shape))
        .expect("should succeed");
    (passes, shape)
}

// ---------------------------------------------------------------------------
// Test (a): Golden snapshot — check_gouges on clean parallel finishing passes
// ---------------------------------------------------------------------------

#[test]
fn gouge_check_golden_matches() {
    let (passes, shape) = generate_sphere_passes();

    let result =
        check_gouges(&passes, &shape, TOOL_TYPE, TOOL_DIAMETER, ALLOWANCE).expect("check_gouges");

    // A correctly computed path should not gouge.
    assert!(
        result.passed,
        "expected no gouge violations on clean passes, got {} violations",
        result.violations.len()
    );
    assert!(
        result.violations.is_empty(),
        "expected zero violations, got {}",
        result.violations.len()
    );

    let json = serde_json::to_string_pretty(&result).expect("serialize GougeCheckResult");

    let fixture = fixture_path("gouge_check_golden.json");
    if !fixture.exists() {
        std::fs::write(&fixture, &json).expect("write golden fixture");
        panic!("Golden fixture written to {fixture:?}. Inspect output, then re-run to compare.");
    }

    let golden = std::fs::read_to_string(&fixture).unwrap_or_else(|e| panic!("read fixture: {e}"));
    assert_eq!(
        json, golden,
        "gouge check output does not match golden file"
    );
}

// ---------------------------------------------------------------------------
// Test (b): Negative integration — artificially lowered Z values
// ---------------------------------------------------------------------------

#[test]
fn gouge_check_detects_lowered_passes_and_auto_lift_fixes() {
    let (passes, shape) = generate_sphere_passes();

    // Clone and lower all Z values by 1.0mm.
    let mut lowered: Vec<Pass> = passes
        .iter()
        .map(|p| {
            let mut cloned = p.clone();
            for pt in cloned.cuts.iter_mut() {
                pt.position.z -= 1.0;
            }
            cloned
        })
        .collect();

    // Gouges should now be detected.
    let result =
        check_gouges(&lowered, &shape, TOOL_TYPE, TOOL_DIAMETER, ALLOWANCE).expect("check_gouges");
    assert!(
        !result.passed,
        "expected gouge violations after lowering Z by 1.0mm"
    );
    assert!(
        !result.violations.is_empty(),
        "expected non-empty violations after lowering Z"
    );

    // All gouge depths should be positive and at most ~1.0mm (the amount we lowered).
    // Some points were originally above the surface, so depths vary.
    for v in &result.violations {
        assert!(
            v.gouge_depth > 0.0,
            "expected positive gouge depth, got {:.4}mm",
            v.gouge_depth
        );
        assert!(
            v.gouge_depth <= 1.0 + 1e-6,
            "gouge depth {:.4}mm exceeds the 1.0mm we lowered by",
            v.gouge_depth
        );
    }

    // Auto-lift should fix the gouges. On curved surfaces, lifting can shift
    // the projection, so iterate until convergence.
    let mut total_corrections = 0usize;
    for _ in 0..50 {
        let corrections =
            auto_lift_gouges(&mut lowered, &shape, TOOL_TYPE, TOOL_DIAMETER, ALLOWANCE)
                .expect("auto_lift_gouges");
        total_corrections += corrections;
        if corrections == 0 {
            break;
        }
    }
    assert!(
        total_corrections > 0,
        "expected auto_lift to make corrections on lowered passes"
    );

    // Re-check: should now pass.
    let result_after = check_gouges(&lowered, &shape, TOOL_TYPE, TOOL_DIAMETER, ALLOWANCE)
        .expect("check_gouges after lift");
    assert!(
        result_after.passed,
        "expected zero violations after auto-lift, got {} violations",
        result_after.violations.len()
    );
}

// ---------------------------------------------------------------------------
// Test (c): Structural — auto-lift never lowers a point
// ---------------------------------------------------------------------------

#[test]
fn auto_lift_never_lowers_points() {
    let (passes, shape) = generate_sphere_passes();

    // Clone and lower all Z values by 1.0mm.
    let mut lowered: Vec<Pass> = passes
        .iter()
        .map(|p| {
            let mut cloned = p.clone();
            for pt in cloned.cuts.iter_mut() {
                pt.position.z -= 1.0;
            }
            cloned
        })
        .collect();

    // Record pre-lift (lowered) Z values.
    let pre_lift_z: Vec<Vec<f64>> = lowered
        .iter()
        .map(|p| p.cuts.iter().map(|pt| pt.position.z).collect())
        .collect();

    for _ in 0..50 {
        let corrections =
            auto_lift_gouges(&mut lowered, &shape, TOOL_TYPE, TOOL_DIAMETER, ALLOWANCE)
                .expect("auto_lift_gouges");
        if corrections == 0 {
            break;
        }
    }

    // After auto-lift, every Z must be >= the pre-lift (lowered) Z value.
    for (pi, pass) in lowered.iter().enumerate() {
        for (ci, pt) in pass.cuts.iter().enumerate() {
            assert!(
                pt.position.z >= pre_lift_z[pi][ci] - 1e-9,
                "auto-lift lowered point [{pi}][{ci}]: {:.6} < pre-lift {:.6}",
                pt.position.z,
                pre_lift_z[pi][ci]
            );
        }
    }

    // Verify auto-lift moved points upward from their lowered positions:
    // at least some points should have been lifted.
    let mut lifted_count = 0usize;
    for (pi, pass) in lowered.iter().enumerate() {
        for (ci, pt) in pass.cuts.iter().enumerate() {
            if pt.position.z > pre_lift_z[pi][ci] + 1e-9 {
                lifted_count += 1;
            }
        }
    }
    assert!(
        lifted_count > 0,
        "expected auto-lift to raise at least some points"
    );
}
