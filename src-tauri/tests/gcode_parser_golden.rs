use jamiecam_lib::gcode_parser::{self, MotionSegment, Plane};
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures")
        .join(name)
}

/// Euclidean distance in the arc plane between two Vec3 points.
fn arc_plane_dist(
    a: &jamiecam_lib::models::Vec3,
    b: &jamiecam_lib::models::Vec3,
    plane: &Plane,
) -> f64 {
    match plane {
        Plane::Xy => ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt(),
        Plane::Xz => ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt(),
        Plane::Yz => ((a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt(),
    }
}

/// Validate a single golden NC fixture against the parser.
fn validate_golden(filename: &str, expected_tool: u32, expected_spindle: f64) {
    let path = fixture_path(filename);
    let contents =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read fixture {filename}: {e}"));

    let result = gcode_parser::parse_gcode(&contents);

    // Segments must be non-empty
    assert!(
        !result.segments.is_empty(),
        "{filename}: expected non-empty segments"
    );

    // Segment count should be reasonable relative to non-blank line count
    let non_blank_lines = contents
        .lines()
        .filter(|l| {
            let trimmed = l.trim();
            !trimmed.is_empty() && trimmed != "%"
        })
        .count();
    assert!(
        result.segments.len() >= non_blank_lines / 10,
        "{filename}: segment count ({}) is less than 10% of non-blank lines ({non_blank_lines})",
        result.segments.len()
    );

    // All Arc segments must have valid centers
    for seg in &result.segments {
        if let MotionSegment::Arc {
            start,
            end,
            center,
            plane,
            metadata,
            ..
        } = seg
        {
            let r_start = arc_plane_dist(start, center, plane);
            let r_end = arc_plane_dist(end, center, plane);
            let diff = (r_start - r_end).abs();
            assert!(
                diff < 0.05,
                "{filename} line {}: arc radius mismatch: |{r_start:.4} - {r_end:.4}| = {diff:.4} >= 0.05",
                metadata.source_line
            );
        }
    }

    // Tool number and spindle speed in segment metadata must match expected values
    // (check segments after the first tool change / spindle start)
    let has_expected_tool = result.segments.iter().any(|seg| {
        let meta = match seg {
            MotionSegment::Rapid { metadata, .. } => metadata,
            MotionSegment::Linear { metadata, .. } => metadata,
            MotionSegment::Arc { metadata, .. } => metadata,
            MotionSegment::Dwell { metadata, .. } => metadata,
        };
        meta.tool_number == expected_tool && meta.spindle_speed == expected_spindle
    });
    assert!(
        has_expected_tool,
        "{filename}: no segment found with tool_number={expected_tool} and spindle_speed={expected_spindle}"
    );

    // No warnings that indicate structural parse failures.
    // Unrecognized G/M codes, G28 home-position warnings, cutter comp,
    // and subprogram warnings are acceptable.
    let structural_failures: Vec<_> = result
        .warnings
        .iter()
        .filter(|w| {
            w.message.contains("line skipped")
                || w.message.contains("no center specified")
                || w.message.contains("radius too small")
                || w.message.contains("full circle with R format")
        })
        .collect();
    assert!(
        structural_failures.is_empty(),
        "{filename}: structural parse failure warnings: {structural_failures:?}"
    );
}

#[test]
fn golden_adaptive_clearing() {
    validate_golden("adaptive_clearing_golden.nc", 1, 10000.0);
}

#[test]
fn golden_parallel_finishing() {
    validate_golden("parallel_finishing_golden.nc", 1, 10000.0);
}

#[test]
fn golden_scallop_finishing() {
    validate_golden("scallop_finishing_golden.nc", 1, 10000.0);
}

#[test]
fn golden_flowline_finishing() {
    validate_golden("flowline_finishing_golden.nc", 1, 10000.0);
}

#[test]
fn golden_pencil_milling() {
    validate_golden("pencil_milling_golden.nc", 1, 10000.0);
}
