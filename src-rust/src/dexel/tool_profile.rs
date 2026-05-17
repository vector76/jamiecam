//! Tool clearance profile functions for the dexel material-removal engine.

/// Z clearance for a flat endmill: zero clearance within the cutting radius.
pub fn flat_endmill_clearance(tool_radius: f64) -> impl Fn(f64) -> Option<f64> {
    move |r: f64| {
        if r <= tool_radius {
            Some(0.0)
        } else {
            None
        }
    }
}

/// Z clearance for a ball-nose endmill: hemisphere profile.
pub fn ball_nose_clearance(tool_radius: f64) -> impl Fn(f64) -> Option<f64> {
    move |r: f64| {
        if r <= tool_radius {
            Some(tool_radius - (tool_radius * tool_radius - r * r).sqrt())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_endmill_inside_radius() {
        let clearance = flat_endmill_clearance(5.0);
        assert_eq!(clearance(0.0), Some(0.0));
        assert_eq!(clearance(3.0), Some(0.0));
        assert_eq!(clearance(5.0), Some(0.0));
    }

    #[test]
    fn flat_endmill_outside_radius() {
        let clearance = flat_endmill_clearance(5.0);
        assert_eq!(clearance(5.01), None);
        assert_eq!(clearance(10.0), None);
    }

    #[test]
    fn ball_nose_at_center() {
        let clearance = ball_nose_clearance(5.0);
        let z = clearance(0.0).unwrap();
        assert!((z - 0.0).abs() < 1e-12, "at center z should be 0, got {z}");
    }

    #[test]
    fn ball_nose_at_full_radius() {
        let r = 5.0;
        let clearance = ball_nose_clearance(r);
        let z = clearance(r).unwrap();
        assert!(
            (z - r).abs() < 1e-12,
            "at full radius z should equal R={r}, got {z}"
        );
    }

    #[test]
    fn ball_nose_outside_radius() {
        let clearance = ball_nose_clearance(5.0);
        assert_eq!(clearance(5.01), None);
    }
}
