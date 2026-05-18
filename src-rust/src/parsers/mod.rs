//! Parsers for 2D source formats (DXF, SVG) that produce shared
//! [`crate::geometry2d`] types.
//!
//! All parsers in this module normalise to millimetres at the boundary so the
//! planner and emitter never have to worry about source units.

pub mod dxf;
pub mod svg;

/// Maximum allowed deflection (in millimetres) between an analytic curve and
/// the polyline that approximates it. Shared by every parser in this module so
/// SVG and DXF imports of the same artwork produce comparable polygon density.
///
/// The value (0.05 mm ≈ 0.002") is well below typical CNC step-over and
/// finishing tolerances and is plenty fine for visual inspection in a 2D
/// workspace at any reasonable zoom level.
pub const DEFLECTION_TOLERANCE_MM: f64 = 0.05;

#[cfg(test)]
mod bundled_sample_tests {
    // Guard against accidental rot of the files in `public/samples/` that
    // back the Mode 2 sidebar "Load Sample" dropdown — if either ever stops
    // parsing cleanly the user-visible affordance silently breaks.
    use super::{dxf::parse_dxf, svg::parse_svg};

    const SAMPLE_SVG: &[u8] = include_bytes!("../../../public/samples/sample-profile.svg");
    const SAMPLE_DXF: &[u8] = include_bytes!("../../../public/samples/sample-profile.dxf");

    #[test]
    fn bundled_svg_sample_parses_to_one_closed_path() {
        let parsed = parse_svg(SAMPLE_SVG).expect("bundled SVG sample must parse");
        assert!(
            parsed.warnings.is_empty(),
            "warnings: {:?}",
            parsed.warnings
        );
        assert_eq!(parsed.paths.len(), 1, "expected one polyline");
        assert!(parsed.paths[0].closed, "star outline should be closed");
    }

    #[test]
    fn bundled_dxf_sample_parses_to_one_closed_path() {
        let parsed = parse_dxf(SAMPLE_DXF).expect("bundled DXF sample must parse");
        assert!(
            parsed.warnings.is_empty(),
            "warnings: {:?}",
            parsed.warnings
        );
        assert_eq!(parsed.paths.len(), 1, "expected one polyline");
        assert!(parsed.paths[0].closed, "octagon outline should be closed");
    }
}
