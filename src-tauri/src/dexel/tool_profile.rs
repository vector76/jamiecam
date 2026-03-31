use crate::models::tool::{Tool, ToolType};

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

/// Z clearance for a ball nose endmill: hemisphere profile.
pub fn ball_nose_clearance(tool_radius: f64) -> impl Fn(f64) -> Option<f64> {
    move |r: f64| {
        if r <= tool_radius {
            Some(tool_radius - (tool_radius * tool_radius - r * r).sqrt())
        } else {
            None
        }
    }
}

/// Build a (tool_radius, z_clearance) pair from a `Tool`.
///
/// `FlatEndmill` uses the flat profile, `BallNose` uses the hemisphere profile,
/// and all other tool types fall back to the flat profile.
pub fn clearance_for_tool(tool: &Tool) -> (f64, Box<dyn Fn(f64) -> Option<f64> + Send + Sync>) {
    let tool_radius = tool.diameter / 2.0;
    let closure: Box<dyn Fn(f64) -> Option<f64> + Send + Sync> = match tool.tool_type {
        ToolType::BallNose => Box::new(ball_nose_clearance(tool_radius)),
        ToolType::FlatEndmill => Box::new(flat_endmill_clearance(tool_radius)),
        _ => Box::new(flat_endmill_clearance(tool_radius)),
    };
    (tool_radius, closure)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

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
    fn ball_nose_at_half_radius() {
        let r = 5.0;
        let clearance = ball_nose_clearance(r);
        let half_r = r / 2.0;
        let expected = r - (r * r - half_r * half_r).sqrt();
        let z = clearance(half_r).unwrap();
        assert!(
            (z - expected).abs() < 1e-12,
            "at r/2 expected {expected}, got {z}"
        );
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

    fn make_tool(tool_type: ToolType, diameter: f64) -> Tool {
        Tool {
            id: Uuid::new_v4(),
            name: "test tool".to_string(),
            tool_type,
            material: Some("carbide".to_string()),
            diameter,
            flute_count: Some(4),
            default_spindle_speed: None,
            default_feed_rate: None,
            cutting_length: 0.0,
            shank_diameter: 0.0,
            overall_length: None,
            corner_radius: None,
            included_angle: None,
            point_angle: None,
            pilot_diameter: None,
            pilot_length: None,
            thread_pitch: None,
            min_bore_diameter: None,
            taper_half_angle: None,
        }
    }

    #[test]
    fn clearance_for_flat_endmill() {
        let tool = make_tool(ToolType::FlatEndmill, 10.0);
        let (radius, clearance) = clearance_for_tool(&tool);
        assert!((radius - 5.0).abs() < 1e-12);
        assert_eq!(clearance(3.0), Some(0.0));
        assert_eq!(clearance(6.0), None);
    }

    #[test]
    fn clearance_for_ball_nose() {
        let tool = make_tool(ToolType::BallNose, 10.0);
        let (radius, clearance) = clearance_for_tool(&tool);
        assert!((radius - 5.0).abs() < 1e-12);
        let z = clearance(0.0).unwrap();
        assert!((z - 0.0).abs() < 1e-12);
        assert!(clearance(6.0).is_none());
    }

    #[test]
    fn clearance_fallback_for_other_types() {
        for tool_type in [
            ToolType::VBit,
            ToolType::Drill,
            ToolType::BullNose,
            ToolType::CenterDrill,
            ToolType::Tap,
            ToolType::Reamer,
            ToolType::BoringBar,
            ToolType::ThreadMill,
        ] {
            let tool = make_tool(tool_type.clone(), 10.0);
            let (radius, clearance) = clearance_for_tool(&tool);
            assert!((radius - 5.0).abs() < 1e-12);
            // Fallback is flat endmill
            assert_eq!(clearance(3.0), Some(0.0));
            assert_eq!(clearance(6.0), None);
        }
    }
}
