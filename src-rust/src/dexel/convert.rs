//! Convert parsed G-code motion segments into dexel motion segments.

use crate::dexel::MotionSegment;
use crate::gcode_parser;

/// Convert a slice of G-code parser [`gcode_parser::MotionSegment`]s into
/// dexel [`MotionSegment`]s suitable for the material-removal engine.
///
/// Mapping rules:
/// - `Rapid { start, end, .. }` → `Linear { start, end }` (rapid traversals still
///   sweep the tool through the model in the dexel simulation).
/// - `Linear { start, end, .. }` → `Linear { start, end }`.
/// - `Arc { start, end, center, clockwise, .. }` → `Arc { start, end, center, clockwise }`.
/// - `Dwell { .. }` → skipped (no positional change).
pub fn gcode_segments_to_dexel(segments: &[gcode_parser::MotionSegment]) -> Vec<MotionSegment> {
    segments
        .iter()
        .filter_map(|seg| match seg {
            gcode_parser::MotionSegment::Rapid { start, end, .. } => Some(MotionSegment::Linear {
                start: start.clone(),
                end: end.clone(),
            }),
            gcode_parser::MotionSegment::Linear { start, end, .. } => Some(MotionSegment::Linear {
                start: start.clone(),
                end: end.clone(),
            }),
            gcode_parser::MotionSegment::Arc {
                start,
                end,
                center,
                clockwise,
                ..
            } => Some(MotionSegment::Arc {
                start: start.clone(),
                end: end.clone(),
                center: center.clone(),
                clockwise: *clockwise,
            }),
            gcode_parser::MotionSegment::Dwell { .. } => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcode_parser::types::{FeedMode, Plane, SegmentMetadata, SpindleDir};
    use crate::types::Vec3;

    fn meta() -> SegmentMetadata {
        SegmentMetadata {
            source_line: 1,
            tool_number: 1,
            spindle_speed: 0.0,
            spindle_dir: SpindleDir::Off,
            feed_mode: FeedMode::PerMinute,
        }
    }

    fn v(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    #[test]
    fn rapid_maps_to_linear() {
        let segs = vec![gcode_parser::MotionSegment::Rapid {
            start: v(0.0, 0.0, 0.0),
            end: v(10.0, 0.0, 0.0),
            metadata: meta(),
        }];
        let result = gcode_segments_to_dexel(&segs);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0],
            MotionSegment::Linear {
                start: v(0.0, 0.0, 0.0),
                end: v(10.0, 0.0, 0.0),
            }
        );
    }

    #[test]
    fn linear_maps_to_linear() {
        let segs = vec![gcode_parser::MotionSegment::Linear {
            start: v(0.0, 0.0, 0.0),
            end: v(5.0, 5.0, 5.0),
            feed_rate: 1000.0,
            metadata: meta(),
        }];
        let result = gcode_segments_to_dexel(&segs);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0],
            MotionSegment::Linear {
                start: v(0.0, 0.0, 0.0),
                end: v(5.0, 5.0, 5.0),
            }
        );
    }

    #[test]
    fn arc_maps_to_arc_with_same_geometry() {
        let segs = vec![gcode_parser::MotionSegment::Arc {
            start: v(10.0, 0.0, 0.0),
            end: v(0.0, 10.0, 0.0),
            center: v(0.0, 0.0, 0.0),
            clockwise: false,
            plane: Plane::Xy,
            feed_rate: 500.0,
            metadata: meta(),
        }];
        let result = gcode_segments_to_dexel(&segs);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0],
            MotionSegment::Arc {
                start: v(10.0, 0.0, 0.0),
                end: v(0.0, 10.0, 0.0),
                center: v(0.0, 0.0, 0.0),
                clockwise: false,
            }
        );
    }

    #[test]
    fn dwell_produces_no_segment() {
        let segs = vec![gcode_parser::MotionSegment::Dwell {
            position: v(0.0, 0.0, 0.0),
            seconds: 1.0,
            metadata: meta(),
        }];
        let result = gcode_segments_to_dexel(&segs);
        assert!(result.is_empty());
    }

    #[test]
    fn empty_input_produces_empty_output() {
        let result = gcode_segments_to_dexel(&[]);
        assert!(result.is_empty());
    }
}
