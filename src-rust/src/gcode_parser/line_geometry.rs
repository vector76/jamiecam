//! Conversion from G-code motion segments into [`LineGeometryData`] for
//! 3D viewport rendering.
//!
//! The integer type convention matches the existing `get_toolpath_geometry`
//! command (see `commands/toolpath.rs`):
//!
//! | Value | Meaning               |
//! |-------|-----------------------|
//! | `0`   | Linking / rapid move  |
//! | `1`   | Cutting / feed move   |
//!
//! Arc segments are tessellated into short linear approximations before being
//! added to the output.

use std::collections::HashMap;

use crate::types::LineGeometryData;

use super::types::{MotionSegment, Plane};

/// Number of line segments used to approximate one full circle.
/// Arcs are a fraction of a circle, so actual count will be proportional to
/// the swept angle.
const ARC_SEGMENTS_PER_CIRCLE: usize = 64;

/// Rapid-move colour: mid-grey.
const RAPID_COLOUR: (f32, f32, f32) = (0.5, 0.5, 0.5);

/// Fallback feed colour for segments that precede any tool assignment (tool
/// number 0 or absent). Distinct from rapid grey.
const FALLBACK_FEED_COLOUR: (f32, f32, f32) = (1.0, 1.0, 1.0);

/// Per-tool colour palette (cycled when there are more tools than colours).
const TOOL_PALETTE: [(f32, f32, f32); 6] = [
    (1.0, 0.3, 0.0), // orange
    (0.0, 0.7, 1.0), // cyan
    (0.6, 0.0, 1.0), // violet
    (0.0, 0.9, 0.3), // green
    (1.0, 0.8, 0.0), // yellow
    (1.0, 0.0, 0.5), // pink
];

/// Convert a slice of G-code [`MotionSegment`]s into flat-array line geometry
/// suitable for Three.js / WebGL rendering.
///
/// # Segment mapping
///
/// - `Rapid` → type `0` (linking/rapid), grey colour.
/// - `Linear` → type `1` (cutting), per-tool-number colour.
/// - `Arc` → tessellated into short line segments, type `1`, per-tool-number colour.
/// - `Dwell` → skipped (no visual representation).
///
/// Segments whose tool number is `0` (i.e. before any T-word in the program)
/// receive [`FALLBACK_FEED_COLOUR`] rather than a palette colour.
pub fn gcode_segments_to_line_geometry(segments: &[MotionSegment]) -> LineGeometryData {
    // Collect distinct non-zero tool numbers in first-seen order to assign
    // palette indices.
    let mut tool_palette_index: HashMap<u32, usize> = HashMap::new();
    let mut next_palette_idx: usize = 0;

    // Pre-scan to build the palette mapping (keeps colour assignment stable
    // regardless of the output order).
    for seg in segments {
        let tool_num = tool_number_of(seg);
        if tool_num != 0 && !tool_palette_index.contains_key(&tool_num) {
            tool_palette_index.insert(tool_num, next_palette_idx % TOOL_PALETTE.len());
            next_palette_idx += 1;
        }
    }

    let mut positions: Vec<f32> = Vec::new();
    let mut colours: Vec<f32> = Vec::new();
    let mut types: Vec<u8> = Vec::new();

    for seg in segments {
        match seg {
            MotionSegment::Rapid { start, end, .. } => {
                push_line(
                    &mut positions,
                    &mut colours,
                    &mut types,
                    start.x as f32,
                    start.y as f32,
                    start.z as f32,
                    end.x as f32,
                    end.y as f32,
                    end.z as f32,
                    RAPID_COLOUR,
                    0, // linking/rapid type
                );
            }

            MotionSegment::Linear {
                start,
                end,
                metadata,
                ..
            } => {
                let colour = feed_colour(metadata.tool_number, &tool_palette_index);
                push_line(
                    &mut positions,
                    &mut colours,
                    &mut types,
                    start.x as f32,
                    start.y as f32,
                    start.z as f32,
                    end.x as f32,
                    end.y as f32,
                    end.z as f32,
                    colour,
                    1, // cutting type
                );
            }

            MotionSegment::Arc {
                start,
                end,
                center,
                clockwise,
                plane,
                metadata,
                ..
            } => {
                let colour = feed_colour(metadata.tool_number, &tool_palette_index);
                let tess = tessellate_arc(start, end, center, *clockwise, plane);
                for [ax, ay, az, bx, by, bz] in tess {
                    push_line(
                        &mut positions,
                        &mut colours,
                        &mut types,
                        ax,
                        ay,
                        az,
                        bx,
                        by,
                        bz,
                        colour,
                        1, // cutting type
                    );
                }
            }

            MotionSegment::Dwell { .. } => {
                // No visual output for a dwell.
            }
        }
    }

    LineGeometryData {
        positions,
        colours,
        types,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract tool number from the segment's metadata (0 when there is none).
fn tool_number_of(seg: &MotionSegment) -> u32 {
    match seg {
        MotionSegment::Rapid { metadata, .. } => metadata.tool_number,
        MotionSegment::Linear { metadata, .. } => metadata.tool_number,
        MotionSegment::Arc { metadata, .. } => metadata.tool_number,
        MotionSegment::Dwell { metadata, .. } => metadata.tool_number,
    }
}

/// Resolve the feed colour for a tool number.
fn feed_colour(tool_number: u32, palette: &HashMap<u32, usize>) -> (f32, f32, f32) {
    if tool_number == 0 {
        return FALLBACK_FEED_COLOUR;
    }
    match palette.get(&tool_number) {
        Some(&idx) => TOOL_PALETTE[idx],
        None => FALLBACK_FEED_COLOUR,
    }
}

/// Append one line segment to the three flat arrays.
#[allow(clippy::too_many_arguments)]
fn push_line(
    positions: &mut Vec<f32>,
    colours: &mut Vec<f32>,
    types: &mut Vec<u8>,
    ax: f32,
    ay: f32,
    az: f32,
    bx: f32,
    by: f32,
    bz: f32,
    colour: (f32, f32, f32),
    type_byte: u8,
) {
    positions.extend_from_slice(&[ax, ay, az, bx, by, bz]);
    colours.extend_from_slice(&[colour.0, colour.1, colour.2, colour.0, colour.1, colour.2]);
    types.push(type_byte);
}

/// Tessellate an arc into a sequence of `[ax, ay, az, bx, by, bz]` line
/// segments using the arc's plane, center, start and end points.
///
/// The tessellation divides the swept angle into a number of steps proportional
/// to the fraction of the full circle that the arc represents.
fn tessellate_arc(
    start: &crate::types::Vec3,
    end: &crate::types::Vec3,
    center: &crate::types::Vec3,
    clockwise: bool,
    plane: &Plane,
) -> Vec<[f32; 6]> {
    // Project into the active plane's 2D axes.
    let (start_u, start_v, start_w) = plane_coords(start, plane);
    let (end_u, end_v, end_w) = plane_coords(end, plane);
    let (center_u, center_v, _center_w) = plane_coords(center, plane);

    let r_start = ((start_u - center_u).powi(2) + (start_v - center_v).powi(2)).sqrt();

    let angle_start = (start_v - center_v).atan2(start_u - center_u);
    let mut angle_end = (end_v - center_v).atan2(end_u - center_u);

    // Adjust angle_end so the sweep is in the correct rotational direction.
    if clockwise {
        // CW: angle decreases. Make angle_end < angle_start.
        if angle_end >= angle_start {
            angle_end -= 2.0 * std::f64::consts::PI;
        }
    } else {
        // CCW: angle increases. Make angle_end > angle_start.
        if angle_end <= angle_start {
            angle_end += 2.0 * std::f64::consts::PI;
        }
    }

    let sweep = (angle_end - angle_start).abs();

    // Number of segments proportional to swept angle.
    let n_segs =
        ((sweep / (2.0 * std::f64::consts::PI)) * ARC_SEGMENTS_PER_CIRCLE as f64).ceil() as usize;
    let n_segs = n_segs.max(1);

    let mut result = Vec::with_capacity(n_segs);
    // Snap to the exact start point to avoid floating-point drift from the
    // trig round-trip (center + r*cos(atan2(…)) ≈ start but not bitwise equal).
    let mut prev_point = (start.x, start.y, start.z);

    for i in 1..=n_segs {
        let t = i as f64 / n_segs as f64;
        let angle = angle_start + (angle_end - angle_start) * t;
        let next_point = if i == n_segs {
            // Snap to exact end to avoid floating-point drift.
            (end.x, end.y, end.z)
        } else {
            // Interpolate the depth linearly so helical arcs ramp smoothly.
            let depth = start_w + (end_w - start_w) * t;
            xyz_from_plane(
                center_u + r_start * angle.cos(),
                center_v + r_start * angle.sin(),
                depth,
                plane,
            )
        };

        result.push([
            prev_point.0 as f32,
            prev_point.1 as f32,
            prev_point.2 as f32,
            next_point.0 as f32,
            next_point.1 as f32,
            next_point.2 as f32,
        ]);
        prev_point = next_point;
    }

    result
}

/// Extract (u, v, w) coordinates for a point in the given arc plane.
/// `u` and `v` are the two in-plane axes; `w` is the depth (out-of-plane) axis.
///
/// The axis ordering for each plane matches `arcs::plane_2d` so that the
/// `clockwise` flag (set by the G-code parser) has consistent meaning here:
///
/// | Plane | u  | v  | w  | Normal |
/// |-------|----|----|----|--------|
/// | XY    | X  | Y  | Z  | +Z     |
/// | XZ    | Z  | X  | Y  | +Y  (ZX order, matching arcs.rs) |
/// | YZ    | Y  | Z  | X  | +X     |
fn plane_coords(p: &crate::types::Vec3, plane: &Plane) -> (f64, f64, f64) {
    match plane {
        Plane::Xy => (p.x, p.y, p.z),
        Plane::Xz => (p.z, p.x, p.y), // ZX order — mirrors arcs::plane_2d
        Plane::Yz => (p.y, p.z, p.x),
    }
}

/// Convert in-plane (u, v, w) coordinates back to world (x, y, z).
fn xyz_from_plane(u: f64, v: f64, w: f64, plane: &Plane) -> (f64, f64, f64) {
    match plane {
        Plane::Xy => (u, v, w), // x=u, y=v, z=w
        Plane::Xz => (v, w, u), // x=v, y=w, z=u
        Plane::Yz => (w, u, v), // x=w, y=u, z=v
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcode_parser::types::{FeedMode, SegmentMetadata, SpindleDir};
    use crate::types::Vec3;

    fn meta_tool(tool_number: u32) -> SegmentMetadata {
        SegmentMetadata {
            source_line: 1,
            tool_number,
            spindle_speed: 0.0,
            spindle_dir: SpindleDir::Off,
            feed_mode: FeedMode::PerMinute,
        }
    }

    fn v(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    #[test]
    fn empty_input_returns_empty_geometry() {
        let geo = gcode_segments_to_line_geometry(&[]);
        assert!(geo.positions.is_empty());
        assert!(geo.colours.is_empty());
        assert!(geo.types.is_empty());
    }

    #[test]
    fn rapid_produces_one_segment_with_type_zero() {
        let segs = vec![MotionSegment::Rapid {
            start: v(0.0, 0.0, 0.0),
            end: v(10.0, 0.0, 0.0),
            metadata: meta_tool(0),
        }];
        let geo = gcode_segments_to_line_geometry(&segs);
        assert_eq!(geo.types.len(), 1);
        assert_eq!(geo.types[0], 0);
        assert_eq!(geo.positions.len(), 6);
    }

    #[test]
    fn linear_produces_one_segment_with_type_one() {
        let segs = vec![MotionSegment::Linear {
            start: v(0.0, 0.0, 0.0),
            end: v(5.0, 5.0, 0.0),
            feed_rate: 1000.0,
            metadata: meta_tool(1),
        }];
        let geo = gcode_segments_to_line_geometry(&segs);
        assert_eq!(geo.types.len(), 1);
        assert_eq!(geo.types[0], 1);
    }

    #[test]
    fn dwell_is_skipped() {
        let segs = vec![MotionSegment::Dwell {
            position: v(0.0, 0.0, 0.0),
            seconds: 1.0,
            metadata: meta_tool(1),
        }];
        let geo = gcode_segments_to_line_geometry(&segs);
        assert!(geo.types.is_empty());
    }

    #[test]
    fn rapid_has_different_colour_than_feed() {
        let segs = vec![
            MotionSegment::Rapid {
                start: v(0.0, 0.0, 0.0),
                end: v(10.0, 0.0, 0.0),
                metadata: meta_tool(1),
            },
            MotionSegment::Linear {
                start: v(10.0, 0.0, 0.0),
                end: v(20.0, 0.0, 0.0),
                feed_rate: 1000.0,
                metadata: meta_tool(1),
            },
        ];
        let geo = gcode_segments_to_line_geometry(&segs);
        assert_eq!(geo.types.len(), 2);

        // Rapid colour: positions 0..6 in colours array (RGB × 2 vertices).
        let rapid_r = geo.colours[0];
        let rapid_g = geo.colours[1];
        let rapid_b = geo.colours[2];

        // Feed colour: positions 6..12.
        let feed_r = geo.colours[6];
        let feed_g = geo.colours[7];
        let feed_b = geo.colours[8];

        // Colours must differ.
        assert!(
            rapid_r != feed_r || rapid_g != feed_g || rapid_b != feed_b,
            "rapid and feed colours must differ"
        );
    }

    #[test]
    fn multiple_tool_numbers_get_distinct_colours() {
        let segs = vec![
            MotionSegment::Linear {
                start: v(0.0, 0.0, 0.0),
                end: v(10.0, 0.0, 0.0),
                feed_rate: 1000.0,
                metadata: meta_tool(1),
            },
            MotionSegment::Linear {
                start: v(10.0, 0.0, 0.0),
                end: v(20.0, 0.0, 0.0),
                feed_rate: 1000.0,
                metadata: meta_tool(2),
            },
        ];
        let geo = gcode_segments_to_line_geometry(&segs);
        assert_eq!(geo.types.len(), 2);

        let colour1 = (geo.colours[0], geo.colours[1], geo.colours[2]);
        let colour2 = (geo.colours[6], geo.colours[7], geo.colours[8]);
        assert_ne!(
            colour1, colour2,
            "different tool numbers must produce different colours"
        );
    }

    #[test]
    fn tool_number_zero_uses_fallback_colour() {
        let segs = vec![MotionSegment::Linear {
            start: v(0.0, 0.0, 0.0),
            end: v(10.0, 0.0, 0.0),
            feed_rate: 1000.0,
            metadata: meta_tool(0),
        }];
        let geo = gcode_segments_to_line_geometry(&segs);
        assert_eq!(geo.types.len(), 1);
        // Fallback colour is white (1.0, 1.0, 1.0).
        assert_eq!(geo.colours[0], 1.0);
        assert_eq!(geo.colours[1], 1.0);
        assert_eq!(geo.colours[2], 1.0);
    }

    #[test]
    fn same_tool_number_produces_same_colour() {
        let segs = vec![
            MotionSegment::Linear {
                start: v(0.0, 0.0, 0.0),
                end: v(10.0, 0.0, 0.0),
                feed_rate: 1000.0,
                metadata: meta_tool(3),
            },
            MotionSegment::Linear {
                start: v(10.0, 0.0, 0.0),
                end: v(20.0, 0.0, 0.0),
                feed_rate: 1000.0,
                metadata: meta_tool(3),
            },
        ];
        let geo = gcode_segments_to_line_geometry(&segs);
        assert_eq!(geo.types.len(), 2);
        let colour1 = (geo.colours[0], geo.colours[1], geo.colours[2]);
        let colour2 = (geo.colours[6], geo.colours[7], geo.colours[8]);
        assert_eq!(
            colour1, colour2,
            "same tool number must produce same colour"
        );
    }

    #[test]
    fn arc_tessellation_produces_multiple_segments() {
        // Full semicircle arc on XY plane.
        let segs = vec![MotionSegment::Arc {
            start: v(10.0, 0.0, 0.0),
            end: v(-10.0, 0.0, 0.0),
            center: v(0.0, 0.0, 0.0),
            clockwise: false,
            plane: Plane::Xy,
            feed_rate: 500.0,
            metadata: meta_tool(1),
        }];
        let geo = gcode_segments_to_line_geometry(&segs);
        // A semicircle should produce > 1 output segment.
        assert!(
            geo.types.len() > 1,
            "arc should produce multiple line segments"
        );
        // All arc segments have type 1 (cutting).
        for &t in &geo.types {
            assert_eq!(t, 1);
        }
    }

    #[test]
    fn arc_first_and_last_points_match_start_and_end() {
        // Quarter circle on XY plane: start (10, 0, 5) → end (0, 10, 5), CCW.
        let start = v(10.0, 0.0, 5.0);
        let end = v(0.0, 10.0, 5.0);
        let segs = vec![MotionSegment::Arc {
            start: start.clone(),
            end: end.clone(),
            center: v(0.0, 0.0, 5.0),
            clockwise: false,
            plane: Plane::Xy,
            feed_rate: 500.0,
            metadata: meta_tool(1),
        }];
        let geo = gcode_segments_to_line_geometry(&segs);
        assert!(!geo.types.is_empty());

        // First segment: positions[0..3] should match start.
        let first_ax = geo.positions[0];
        let first_ay = geo.positions[1];
        let first_az = geo.positions[2];
        assert!(
            (first_ax - start.x as f32).abs() < 1e-4,
            "arc start X mismatch"
        );
        assert!(
            (first_ay - start.y as f32).abs() < 1e-4,
            "arc start Y mismatch"
        );
        assert!(
            (first_az - start.z as f32).abs() < 1e-4,
            "arc start Z mismatch"
        );

        // Last segment end point: last 3 positions entries should match end.
        let n = geo.positions.len();
        let last_bx = geo.positions[n - 3];
        let last_by = geo.positions[n - 2];
        let last_bz = geo.positions[n - 1];
        assert!(
            (last_bx - end.x as f32).abs() < 1e-4,
            "arc end X mismatch: got {last_bx}"
        );
        assert!(
            (last_by - end.y as f32).abs() < 1e-4,
            "arc end Y mismatch: got {last_by}"
        );
        assert!(
            (last_bz - end.z as f32).abs() < 1e-4,
            "arc end Z mismatch: got {last_bz}"
        );
    }

    #[test]
    fn helical_arc_interpolates_depth() {
        // Helical quarter-circle on XY plane: Z ramps from 0 to 8 over 90°.
        // Every intermediate Z should be strictly between 0 and 8, not flat.
        let segs = vec![MotionSegment::Arc {
            start: v(10.0, 0.0, 0.0),
            end: v(0.0, 10.0, 8.0),
            center: v(0.0, 0.0, 0.0),
            clockwise: false,
            plane: Plane::Xy,
            feed_rate: 500.0,
            metadata: meta_tool(1),
        }];
        let geo = gcode_segments_to_line_geometry(&segs);
        let n_segs = geo.types.len();
        assert!(
            n_segs > 2,
            "need more than 2 segments to test intermediate depth"
        );

        // All vertex Z values should be in [0, 8], and at least one intermediate
        // vertex should be strictly between 0 and 8 (not flat at 0).
        let mut saw_intermediate_z = false;
        for i in 0..n_segs {
            let z_start = geo.positions[i * 6 + 2];
            let z_end = geo.positions[i * 6 + 5];
            assert!((0.0..=8.0).contains(&z_start), "Z out of range: {z_start}");
            assert!((0.0..=8.0).contains(&z_end), "Z out of range: {z_end}");
            if i > 0 && i < n_segs - 1 && z_start > 0.001 {
                saw_intermediate_z = true;
            }
        }
        assert!(
            saw_intermediate_z,
            "intermediate arc points should have non-zero Z (helical ramp)"
        );
    }

    #[test]
    fn arc_xz_plane_cw_quarter_circle_short_sweep() {
        // G18 CW quarter-circle: (10,0,0) → (0,0,10), center (0,0,0).
        // A CW arc in the XZ plane (viewed from +Y) sweeps 90°, producing
        // ~16 segments (64 × 0.25). Before the ZX-ordering fix this produced
        // a 270° arc (~48 segments) because the axis swap flipped chirality.
        let segs = vec![MotionSegment::Arc {
            start: v(10.0, 0.0, 0.0),
            end: v(0.0, 0.0, 10.0),
            center: v(0.0, 0.0, 0.0),
            clockwise: true,
            plane: Plane::Xz,
            feed_rate: 500.0,
            metadata: meta_tool(1),
        }];
        let geo = gcode_segments_to_line_geometry(&segs);
        // Quarter-circle: ≤ 20 segments; 270° arc would produce ≥ 45.
        assert!(
            geo.types.len() <= 20,
            "XZ CW quarter arc should be ~16 segments, got {}",
            geo.types.len()
        );
        assert!(!geo.types.is_empty());
        // First point must match start.
        assert!((geo.positions[0] - 10.0_f32).abs() < 1e-4);
        assert!((geo.positions[1] - 0.0_f32).abs() < 1e-4);
        assert!((geo.positions[2] - 0.0_f32).abs() < 1e-4);
        // Last point must match end.
        let n = geo.positions.len();
        assert!((geo.positions[n - 3] - 0.0_f32).abs() < 1e-4);
        assert!((geo.positions[n - 2] - 0.0_f32).abs() < 1e-4);
        assert!((geo.positions[n - 1] - 10.0_f32).abs() < 1e-4);
    }

    #[test]
    fn positions_and_colours_have_consistent_lengths() {
        let segs = vec![
            MotionSegment::Rapid {
                start: v(0.0, 0.0, 0.0),
                end: v(5.0, 0.0, 0.0),
                metadata: meta_tool(0),
            },
            MotionSegment::Linear {
                start: v(5.0, 0.0, 0.0),
                end: v(10.0, 0.0, 0.0),
                feed_rate: 500.0,
                metadata: meta_tool(1),
            },
        ];
        let geo = gcode_segments_to_line_geometry(&segs);
        let seg_count = geo.types.len();
        assert_eq!(geo.positions.len(), seg_count * 6);
        assert_eq!(geo.colours.len(), seg_count * 6);
    }
}
