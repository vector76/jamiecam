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
