#![cfg(cam_geometry_bindings)]

//! Golden tests for the `generate_2d_gcode` IPC command (Mode 2 — 2D Profiling).
//!
//! These tests exercise the full pipeline:
//! parse SVG → populate Project state → call `generate_2d_gcode_inner` →
//! verify G-code output and store golden NC files.

use jamiecam_lib::commands::twod::generate_2d_gcode_inner;
use jamiecam_lib::error::AppError;
use jamiecam_lib::models::operation::{
    CacheState, CutType, MillingDirection, OperationParams, Profile2dParams,
};
use jamiecam_lib::models::stock::BoxDimensions;
use jamiecam_lib::models::tool::ToolType;
use jamiecam_lib::models::twod::{Curve2d, LoadedArtwork, UnitSystem};
use jamiecam_lib::models::{Operation, StockDefinition, Tool, Vec3};
use jamiecam_lib::state::Project;
use std::path::PathBuf;
use std::sync::RwLock;
use uuid::Uuid;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/integration/twod")
}

fn load_rect_curves() -> Vec<Curve2d> {
    let path = fixture_dir().join("rect.svg");
    let bytes = std::fs::read(&path).expect("read rect.svg fixture");
    jamiecam_lib::models::twod::parse_svg(&bytes, UnitSystem::Mm).expect("parse rect.svg")
}

fn make_flat_endmill(id: Uuid, diameter: f64) -> Tool {
    Tool {
        id,
        name: format!("{diameter}mm Flat Endmill"),
        tool_type: ToolType::FlatEndmill,
        material: Some("carbide".to_string()),
        diameter,
        flute_count: Some(4),
        default_spindle_speed: Some(8000),
        default_feed_rate: Some(1000.0),
        cutting_length: 18.0,
        shank_diameter: diameter,
        corner_radius: None,
        included_angle: None,
        point_angle: None,
        pilot_diameter: None,
        pilot_length: None,
        thread_pitch: None,
        min_bore_diameter: None,
        taper_half_angle: None,
        overall_length: Some(54.0),
    }
}

/// Build a minimal Project populated with:
/// - SVG artwork from rect.svg
/// - one Tool of the given diameter
/// - one Profile2d Operation targeting the first closed curve in the artwork
fn make_project_with_outside_op(
    tool_diameter: f64,
    top_of_cut: f64,
    depth_of_cut: f64,
    step_down: f64,
    artwork_origin: [f64; 2],
    stock: Option<StockDefinition>,
) -> (RwLock<Project>, Uuid /* curve_id */) {
    let curves = load_rect_curves();
    let closed_curve = curves
        .iter()
        .find(|c| c.is_closed)
        .expect("rect.svg must have at least one closed curve");
    let curve_id = closed_curve.id;

    let tool_id = Uuid::new_v4();
    let op_id = Uuid::new_v4();

    let mut project = Project::default();
    project.source_2d_artwork = Some(LoadedArtwork {
        file_path: fixture_dir().join("rect.svg").to_string_lossy().into(),
        unit_system: UnitSystem::Mm,
        curves,
        import_warnings: Vec::new(),
    });
    project
        .tools
        .push(make_flat_endmill(tool_id, tool_diameter));
    project.operations.push(Operation {
        id: op_id,
        name: "Rect outside".to_string(),
        enabled: true,
        tool_id,
        spindle_speed_override: None,
        feed_rate_override: None,
        workpiece_material: None,
        params: OperationParams::Profile2d(Profile2dParams {
            curve_id,
            cut_type: CutType::Outside,
            direction: MillingDirection::Climb,
            top_of_cut,
            depth_of_cut,
            step_down,
            feed_rate: 1000.0,
        }),
        cache: CacheState::default(),
    });
    project.artwork_origin = artwork_origin;
    project.stock = stock;

    (RwLock::new(project), curve_id)
}

// ── Golden: rectangular outside cut ──────────────────────────────────────────

#[test]
fn rect_outside_golden() {
    let golden = fixture_dir().join("rect_outside.nc");

    let (lock, _) = make_project_with_outside_op(
        6.0, // tool diameter
        1.0, // top_of_cut
        5.0, // depth_of_cut  → bottom_z = -4.0
        2.5, // step_down     → 2 passes: Z-1.5, Z-4.0
        [0.0, 0.0],
        None,
    );

    let result =
        generate_2d_gcode_inner("fanuc-0i", &lock).expect("generate_2d_gcode_inner must succeed");

    // Sanity: 2 Z-level cutting passes should appear
    assert!(
        result.gcode.contains("Z-1.5"),
        "expected Z-1.5 pass in G-code; got:\n{}",
        result.gcode
    );
    assert!(
        result.gcode.contains("Z-4"),
        "expected Z-4 pass in G-code; got:\n{}",
        result.gcode
    );
    // Feed moves must be present
    assert!(
        result.gcode.contains("G01"),
        "expected G01 feed moves; got:\n{}",
        result.gcode
    );
    // Stats: at least 2 passes (one per Z level)
    assert!(
        result.stats.total_pass_count >= 2,
        "expected at least 2 passes, got {}",
        result.stats.total_pass_count
    );

    if !golden.exists() {
        std::fs::write(&golden, &result.gcode).expect("write golden file");
        panic!(
            "Golden fixture written to {:?}. \
             Inspect: verify 2 Z levels (Z-1.5 and Z-4), outside contour offset by 3 mm, \
             climb-milling winding. Re-run to lock.",
            golden
        );
    }

    let expected =
        std::fs::read_to_string(&golden).unwrap_or_else(|e| panic!("read golden {golden:?}: {e}"));
    assert_eq!(
        result.gcode, expected,
        "rect_outside G-code does not match golden file"
    );
}

// ── Golden: artwork origin offset ─────────────────────────────────────────────

#[test]
fn rect_outside_offset_golden() {
    let golden = fixture_dir().join("rect_outside_offset.nc");

    let (lock, _) = make_project_with_outside_op(
        6.0,
        1.0,
        5.0,
        2.5,
        [10.0, 0.0], // artwork_origin shifted +10 in X
        None,
    );

    let result = generate_2d_gcode_inner("fanuc-0i", &lock)
        .expect("generate_2d_gcode_inner must succeed with offset");

    // With +10 X offset the cut contour should not touch X=0 territory.
    // All cutting X coordinates must be ≥ 10 − tool_radius (= 10 − 3 = 7 for outside cut).
    // We verify by checking that an absolute X0 does not appear in feed moves.
    assert!(
        !result.gcode.contains("X0 ") && !result.gcode.contains("X0\r"),
        "X0 should not appear after a +10 artwork origin offset; got:\n{}",
        result.gcode
    );

    if !golden.exists() {
        std::fs::write(&golden, &result.gcode).expect("write golden file");
        panic!(
            "Golden fixture written to {:?}. \
             Inspect: verify all X coordinates are shifted by +10 compared to rect_outside.nc. \
             Re-run to lock.",
            golden
        );
    }

    let expected =
        std::fs::read_to_string(&golden).unwrap_or_else(|e| panic!("read golden {golden:?}: {e}"));
    assert_eq!(
        result.gcode, expected,
        "rect_outside_offset G-code does not match golden file"
    );
}

// ── Error: inside cut with tool too large ─────────────────────────────────────

#[test]
fn inside_cut_tool_too_large_returns_error() {
    // A 50 mm-radius circle curve with a 60 mm tool cannot be inside-cut.
    // We synthesise a small closed circle to make the inside-cut invalid.
    let circle_id = Uuid::new_v4();
    let circle_points: Vec<[f64; 2]> = (0..64)
        .map(|i| {
            let angle = 2.0 * std::f64::consts::PI * i as f64 / 64.0;
            [25.0 * angle.cos(), 25.0 * angle.sin()] // radius 25 mm
        })
        .collect();

    let tool_id = Uuid::new_v4();
    let op_id = Uuid::new_v4();
    let mut project = Project::default();

    project.source_2d_artwork = Some(LoadedArtwork {
        file_path: String::new(),
        unit_system: UnitSystem::Mm,
        curves: vec![Curve2d {
            id: circle_id,
            is_closed: true,
            points: circle_points,
            layer: None,
        }],
        import_warnings: Vec::new(),
    });
    project.tools.push(make_flat_endmill(tool_id, 120.0)); // diameter=120, radius=60
    project.operations.push(Operation {
        id: op_id,
        name: "Inside too large".to_string(),
        enabled: true,
        tool_id,
        spindle_speed_override: None,
        feed_rate_override: None,
        workpiece_material: None,
        params: OperationParams::Profile2d(Profile2dParams {
            curve_id: circle_id,
            cut_type: CutType::Inside,
            direction: MillingDirection::Climb,
            top_of_cut: 0.0,
            depth_of_cut: 5.0,
            step_down: 5.0,
            feed_rate: 500.0,
        }),
        cache: CacheState::default(),
    });

    let lock = RwLock::new(project);
    let err = generate_2d_gcode_inner("fanuc-0i", &lock)
        .expect_err("expected error for tool-too-large inside cut");

    assert!(
        matches!(err, AppError::InvalidInput(_)),
        "expected InvalidInput, got {err:?}"
    );
}

// ── Error: open curve ─────────────────────────────────────────────────────────

#[test]
fn open_curve_returns_error() {
    let curves = load_rect_curves();
    let open_curve = curves
        .iter()
        .find(|c| !c.is_closed)
        .expect("rect.svg must have at least one open curve");
    let open_id = open_curve.id;

    let tool_id = Uuid::new_v4();
    let mut project = Project::default();
    project.source_2d_artwork = Some(LoadedArtwork {
        file_path: String::new(),
        unit_system: UnitSystem::Mm,
        curves,
        import_warnings: Vec::new(),
    });
    project.tools.push(make_flat_endmill(tool_id, 6.0));
    project.operations.push(Operation {
        id: Uuid::new_v4(),
        name: "Open curve op".to_string(),
        enabled: true,
        tool_id,
        spindle_speed_override: None,
        feed_rate_override: None,
        workpiece_material: None,
        params: OperationParams::Profile2d(Profile2dParams {
            curve_id: open_id,
            cut_type: CutType::OnLine,
            direction: MillingDirection::Climb,
            top_of_cut: 0.0,
            depth_of_cut: 5.0,
            step_down: 5.0,
            feed_rate: 500.0,
        }),
        cache: CacheState::default(),
    });

    let lock = RwLock::new(project);
    let err =
        generate_2d_gcode_inner("fanuc-0i", &lock).expect_err("expected error for open curve");

    assert!(
        matches!(err, AppError::InvalidInput(_)),
        "expected InvalidInput, got {err:?}"
    );
}

// ── Error: multiple tools ─────────────────────────────────────────────────────

#[test]
fn multi_tool_returns_error() {
    let curves = load_rect_curves();
    let closed_curve = curves
        .iter()
        .find(|c| c.is_closed)
        .expect("rect.svg must have a closed curve");
    let curve_id = closed_curve.id;

    let tool_a = Uuid::new_v4();
    let tool_b = Uuid::new_v4();
    let mut project = Project::default();
    project.source_2d_artwork = Some(LoadedArtwork {
        file_path: String::new(),
        unit_system: UnitSystem::Mm,
        curves,
        import_warnings: Vec::new(),
    });
    project.tools.push(make_flat_endmill(tool_a, 6.0));
    project.tools.push(make_flat_endmill(tool_b, 10.0));

    for (tool_id, name) in [(tool_a, "Op A"), (tool_b, "Op B")] {
        project.operations.push(Operation {
            id: Uuid::new_v4(),
            name: name.to_string(),
            enabled: true,
            tool_id,
            spindle_speed_override: None,
            feed_rate_override: None,
            workpiece_material: None,
            params: OperationParams::Profile2d(Profile2dParams {
                curve_id,
                cut_type: CutType::OnLine,
                direction: MillingDirection::Climb,
                top_of_cut: 0.0,
                depth_of_cut: 5.0,
                step_down: 5.0,
                feed_rate: 500.0,
            }),
            cache: CacheState::default(),
        });
    }

    let lock = RwLock::new(project);
    let err = generate_2d_gcode_inner("fanuc-0i", &lock)
        .expect_err("expected error for multi-tool setup");

    assert!(
        matches!(err, AppError::InvalidInput(_)),
        "expected InvalidInput, got {err:?}"
    );
    if let AppError::InvalidInput(msg) = err {
        assert!(
            msg.contains("multiple tools"),
            "error message should mention 'multiple tools'; got: {msg}"
        );
    }
}

// ── Warning: top-of-cut at or below stock top ─────────────────────────────────

#[test]
fn top_of_cut_warning_when_at_stock_top() {
    let stock = StockDefinition::Box(BoxDimensions {
        origin: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        width: 100.0,
        depth: 100.0,
        height: 10.0, // stock top = origin.z + height = 0.0 + 10.0 = 10.0
    });

    let (lock, _) = make_project_with_outside_op(
        6.0,
        10.0, // top_of_cut = 10.0 == stock top → warning
        5.0,
        5.0,
        [0.0, 0.0],
        Some(stock),
    );

    let result = generate_2d_gcode_inner("fanuc-0i", &lock)
        .expect("generate_2d_gcode_inner must succeed even when warning is present");

    assert!(
        !result.warnings.is_empty(),
        "expected at least one warning when top_of_cut <= stock top_z; got none"
    );
}
