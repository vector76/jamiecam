use super::PostProcessorError;
use crate::models::Vec3;

/// Returns the IJK arc-center offsets: `(I, J, K) = center − start`.
///
/// In G-code, I, J, K are the signed offsets from the arc start point to the
/// arc center point along the X, Y, Z axes respectively.
pub fn ijk_from_arc(start: &Vec3, center: &Vec3) -> (f64, f64, f64) {
    (center.x - start.x, center.y - start.y, center.z - start.z)
}

/// Computes the sweep angle (in degrees) traversed by an arc from `start` to
/// `end` around `center` in the XY plane, in the specified direction.
///
/// Returns a value in the range `(0°, 360°]`. A result of `360°` indicates a
/// full circle (start and end coincide angularly around the center).
pub fn arc_sweep_degrees(start: &Vec3, center: &Vec3, end: &Vec3, clockwise: bool) -> f64 {
    let angle_start = (start.y - center.y).atan2(start.x - center.x);
    let angle_end = (end.y - center.y).atan2(end.x - center.x);

    let diff = if clockwise {
        angle_start - angle_end
    } else {
        angle_end - angle_start
    };

    let sweep_deg = diff.to_degrees().rem_euclid(360.0);

    // rem_euclid returns 0.0 when start and end angles are equal (full circle).
    if sweep_deg == 0.0 {
        360.0
    } else {
        sweep_deg
    }
}

/// Returns the R-format radius for a G-code arc.
///
/// * Minor arcs (sweep < 180°) → positive R.
/// * Major arcs (sweep > 180°) → negative R.
/// * Exactly 180° arcs → [`Err`]: the R format is ambiguous for a semicircle;
///   use IJK format instead.
///
/// The radius is the 3-D distance from `center` to `start`.
pub fn r_from_arc(
    start: &Vec3,
    end: &Vec3,
    center: &Vec3,
    clockwise: bool,
) -> Result<f64, PostProcessorError> {
    let radius = ((center.x - start.x).powi(2)
        + (center.y - start.y).powi(2)
        + (center.z - start.z).powi(2))
    .sqrt();

    let sweep = arc_sweep_degrees(start, center, end, clockwise);

    const HALF_CIRCLE: f64 = 180.0;
    const EPSILON: f64 = 1e-9;

    if (sweep - HALF_CIRCLE).abs() < EPSILON {
        return Err(PostProcessorError::ArcError(
            "180\u{b0} arc is ambiguous in R format; use IJK instead".to_string(),
        ));
    }

    if sweep > HALF_CIRCLE {
        Ok(-radius)
    } else {
        Ok(radius)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    // -------------------------------------------------------------------------
    // ijk_from_arc — quadrant signs
    // -------------------------------------------------------------------------

    #[test]
    fn ijk_quadrant_positive_x_start() {
        // Start at (+x, 0, 0), center at origin → I negative
        let (i, j, k) = ijk_from_arc(&v(10.0, 0.0, 0.0), &v(0.0, 0.0, 0.0));
        assert_eq!(i, -10.0);
        assert_eq!(j, 0.0);
        assert_eq!(k, 0.0);
    }

    #[test]
    fn ijk_quadrant_positive_y_start() {
        // Start at (0, +y, 0), center at origin → J negative
        let (i, j, k) = ijk_from_arc(&v(0.0, 10.0, 0.0), &v(0.0, 0.0, 0.0));
        assert_eq!(i, 0.0);
        assert_eq!(j, -10.0);
        assert_eq!(k, 0.0);
    }

    #[test]
    fn ijk_quadrant_negative_x_start() {
        // Start at (-x, 0, 0), center at origin → I positive
        let (i, j, k) = ijk_from_arc(&v(-10.0, 0.0, 0.0), &v(0.0, 0.0, 0.0));
        assert_eq!(i, 10.0);
        assert_eq!(j, 0.0);
        assert_eq!(k, 0.0);
    }

    #[test]
    fn ijk_quadrant_negative_y_start() {
        // Start at (0, -y, 0), center at origin → J positive
        let (i, j, k) = ijk_from_arc(&v(0.0, -10.0, 0.0), &v(0.0, 0.0, 0.0));
        assert_eq!(i, 0.0);
        assert_eq!(j, 10.0);
        assert_eq!(k, 0.0);
    }

    // -------------------------------------------------------------------------
    // arc_sweep_degrees
    // -------------------------------------------------------------------------

    #[test]
    fn sweep_quarter_ccw() {
        // (+x,0) → (0,+y) CCW = 90°
        let s = arc_sweep_degrees(
            &v(10.0, 0.0, 0.0),
            &v(0.0, 0.0, 0.0),
            &v(0.0, 10.0, 0.0),
            false,
        );
        assert!((s - 90.0).abs() < 1e-9, "expected 90°, got {s}");
    }

    #[test]
    fn sweep_quarter_cw() {
        // (+x,0) → (0,-y) CW = 90°
        let s = arc_sweep_degrees(
            &v(10.0, 0.0, 0.0),
            &v(0.0, 0.0, 0.0),
            &v(0.0, -10.0, 0.0),
            true,
        );
        assert!((s - 90.0).abs() < 1e-9, "expected 90°, got {s}");
    }

    #[test]
    fn sweep_half_ccw() {
        // (+x,0) → (-x,0) CCW = 180°
        let s = arc_sweep_degrees(
            &v(10.0, 0.0, 0.0),
            &v(0.0, 0.0, 0.0),
            &v(-10.0, 0.0, 0.0),
            false,
        );
        assert!((s - 180.0).abs() < 1e-9, "expected 180°, got {s}");
    }

    #[test]
    fn sweep_half_cw() {
        // (+x,0) → (-x,0) CW = 180°
        let s = arc_sweep_degrees(
            &v(10.0, 0.0, 0.0),
            &v(0.0, 0.0, 0.0),
            &v(-10.0, 0.0, 0.0),
            true,
        );
        assert!((s - 180.0).abs() < 1e-9, "expected 180°, got {s}");
    }

    #[test]
    fn sweep_three_quarter_ccw() {
        // (+x,0) → (0,-y) CCW = 270°
        let s = arc_sweep_degrees(
            &v(10.0, 0.0, 0.0),
            &v(0.0, 0.0, 0.0),
            &v(0.0, -10.0, 0.0),
            false,
        );
        assert!((s - 270.0).abs() < 1e-9, "expected 270°, got {s}");
    }

    #[test]
    fn sweep_three_quarter_cw() {
        // (+x,0) → (0,+y) CW = 270°
        let s = arc_sweep_degrees(
            &v(10.0, 0.0, 0.0),
            &v(0.0, 0.0, 0.0),
            &v(0.0, 10.0, 0.0),
            true,
        );
        assert!((s - 270.0).abs() < 1e-9, "expected 270°, got {s}");
    }

    #[test]
    fn sweep_full_circle_ccw() {
        // start == end → 360°
        let s = arc_sweep_degrees(
            &v(10.0, 0.0, 0.0),
            &v(0.0, 0.0, 0.0),
            &v(10.0, 0.0, 0.0),
            false,
        );
        assert!((s - 360.0).abs() < 1e-9, "expected 360°, got {s}");
    }

    #[test]
    fn sweep_full_circle_cw() {
        // start == end → 360° regardless of direction
        let s = arc_sweep_degrees(
            &v(10.0, 0.0, 0.0),
            &v(0.0, 0.0, 0.0),
            &v(10.0, 0.0, 0.0),
            true,
        );
        assert!((s - 360.0).abs() < 1e-9, "expected 360°, got {s}");
    }

    // -------------------------------------------------------------------------
    // r_from_arc
    // -------------------------------------------------------------------------

    #[test]
    fn r_quarter_arc_ccw_positive() {
        // 90° CCW → minor arc → positive R = 10
        let r = r_from_arc(
            &v(10.0, 0.0, 0.0),
            &v(0.0, 10.0, 0.0),
            &v(0.0, 0.0, 0.0),
            false,
        )
        .expect("90° CCW should not err");
        assert!((r - 10.0).abs() < 1e-9, "expected R=10, got {r}");
    }

    #[test]
    fn r_quarter_arc_cw_positive() {
        // 90° CW → minor arc → positive R = 10
        let r = r_from_arc(
            &v(10.0, 0.0, 0.0),
            &v(0.0, -10.0, 0.0),
            &v(0.0, 0.0, 0.0),
            true,
        )
        .expect("90° CW should not err");
        assert!((r - 10.0).abs() < 1e-9, "expected R=10, got {r}");
    }

    #[test]
    fn r_half_arc_ccw_returns_err() {
        // Exactly 180° CCW → ambiguous → Err
        let result = r_from_arc(
            &v(10.0, 0.0, 0.0),
            &v(-10.0, 0.0, 0.0),
            &v(0.0, 0.0, 0.0),
            false,
        );
        assert!(result.is_err(), "180° CCW arc must return Err");
    }

    #[test]
    fn r_half_arc_cw_returns_err() {
        // Exactly 180° CW → ambiguous → Err
        let result = r_from_arc(
            &v(10.0, 0.0, 0.0),
            &v(-10.0, 0.0, 0.0),
            &v(0.0, 0.0, 0.0),
            true,
        );
        assert!(result.is_err(), "180° CW arc must return Err");
    }

    #[test]
    fn r_three_quarter_arc_ccw_negative() {
        // 270° CCW → major arc → negative R = -10
        let r = r_from_arc(
            &v(10.0, 0.0, 0.0),
            &v(0.0, -10.0, 0.0),
            &v(0.0, 0.0, 0.0),
            false,
        )
        .expect("270° CCW should not err");
        assert!(r < 0.0, "major arc R must be negative, got {r}");
        assert!((r + 10.0).abs() < 1e-9, "expected R=-10, got {r}");
    }

    #[test]
    fn r_three_quarter_arc_cw_negative() {
        // 270° CW → major arc → negative R = -10
        let r = r_from_arc(
            &v(10.0, 0.0, 0.0),
            &v(0.0, 10.0, 0.0),
            &v(0.0, 0.0, 0.0),
            true,
        )
        .expect("270° CW should not err");
        assert!(r < 0.0, "major arc R must be negative, got {r}");
        assert!((r + 10.0).abs() < 1e-9, "expected R=-10, got {r}");
    }
}
