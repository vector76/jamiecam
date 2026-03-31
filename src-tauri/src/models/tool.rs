//! Tool data model for the project-local tool library.
//!
//! [`Tool`] is the in-memory and on-disk representation of a cutting tool.
//! It maps to the `tools` array in `project.json` inside a `.jcam` archive.
//! See `docs/project-file-format.md` for the full JSON schema.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The type of cutting tool.
///
/// Serialized as a snake_case string (e.g. `"flat_endmill"`, `"ball_nose"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolType {
    FlatEndmill,
    BallNose,
    BullNose,
    VBit,
    Drill,
    CenterDrill,
    Tap,
    Reamer,
    BoringBar,
    ThreadMill,
}

/// A cutting tool entry in the project-local tool library.
///
/// Fields are serialized with camelCase keys so the TypeScript frontend
/// receives a consistent naming convention.
///
/// The `type` field uses `#[serde(rename = "type")]` because the JSON schema
/// specifies `"type"` as the discriminant key, while the Rust field is named
/// `tool_type` to avoid the reserved keyword `type`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    /// Unique identifier for this tool entry.
    pub id: Uuid,
    /// Human-readable tool name (e.g. `"10mm 4F Flat Endmill"`).
    pub name: String,
    /// Tool geometry type.
    #[serde(rename = "type")]
    pub tool_type: ToolType,
    /// Tool body material (e.g. `"carbide"`, `"hss"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    /// Cutting diameter in project units (mm or inch).
    pub diameter: f64,
    /// Number of flutes (cutting edges).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flute_count: Option<u32>,
    /// Default spindle speed in RPM, if specified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_spindle_speed: Option<u32>,
    /// Default feed rate in mm/min (or inch/min), if specified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_feed_rate: Option<f64>,
    /// Length of the cutting flutes in project units.
    /// A value of 0.0 is a sentinel meaning "not yet set".
    #[serde(default)]
    pub cutting_length: f64,
    /// Diameter of the shank (non-cutting portion) in project units.
    /// A value of 0.0 is a sentinel meaning "not yet set".
    #[serde(default)]
    pub shank_diameter: f64,
    /// Overall length of the tool in project units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overall_length: Option<f64>,

    // -- Type-specific geometry fields --
    // Each is meaningful only for certain ToolType variants.
    // Fields irrelevant to a tool's type remain `None`.
    /// BullNose: radius of the corner rounding (mm).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<f64>,
    /// VBit: full angle between cutting edges (degrees).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub included_angle: Option<f64>,
    /// Drill / CenterDrill: full cone angle at the tip (degrees).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point_angle: Option<f64>,
    /// CenterDrill: diameter of the pilot section (mm).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pilot_diameter: Option<f64>,
    /// CenterDrill: total length of the pilot portion including cone (mm).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pilot_length: Option<f64>,
    /// Tap / ThreadMill: distance between threads (mm).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_pitch: Option<f64>,
    /// BoringBar: minimum bore the bar fits into (mm).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_bore_diameter: Option<f64>,
    /// FlatEndmill / BullNose: half-angle of taper (degrees).
    /// `None` means straight (no taper). Not defaulted by resolve_defaults().
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taper_half_angle: Option<f64>,
}

impl Tool {
    /// Returns the Z clearance at radial distance `r` from the tool axis.
    ///
    /// Returns `None` if `r` is outside the tool's cutting envelope.
    /// This is a closed-form function giving exact results per tool type.
    /// Only describes the cutting portion — not the shank.
    ///
    /// All angles stored in degrees; converted internally with `.to_radians()`.
    pub fn z_clearance(&self, r: f64) -> Option<f64> {
        const EPS: f64 = 1e-10;
        let big_r = self.diameter / 2.0;

        match self.tool_type {
            ToolType::FlatEndmill
            | ToolType::Tap
            | ToolType::Reamer
            | ToolType::BoringBar
            | ToolType::ThreadMill => match self.taper_half_angle {
                Some(angle) => {
                    let tan_a = angle.to_radians().tan();
                    let r_top = big_r + self.cutting_length * tan_a;
                    if r <= big_r + EPS {
                        Some(0.0)
                    } else if r <= r_top + EPS {
                        Some((r - big_r) / tan_a)
                    } else {
                        None
                    }
                }
                None => {
                    if r <= big_r + EPS {
                        Some(0.0)
                    } else {
                        None
                    }
                }
            },

            ToolType::BallNose => {
                if r <= big_r + EPS {
                    let r_clamped = r.min(big_r);
                    Some(big_r - (big_r * big_r - r_clamped * r_clamped).sqrt())
                } else {
                    None
                }
            }

            ToolType::BullNose => {
                let cr = self.corner_radius.unwrap_or(0.0);
                let flat_r = big_r - cr;

                match self.taper_half_angle {
                    Some(angle) => {
                        let tan_a = angle.to_radians().tan();
                        let r_top = big_r + (self.cutting_length - cr) * tan_a;
                        if r <= flat_r + EPS {
                            Some(0.0)
                        } else if r <= big_r + EPS {
                            let r_clamped = r.min(big_r).max(flat_r);
                            let dr = r_clamped - flat_r;
                            Some(cr - (cr * cr - dr * dr).sqrt())
                        } else if r <= r_top + EPS {
                            Some(cr + (r - big_r) / tan_a)
                        } else {
                            None
                        }
                    }
                    None => {
                        if r <= flat_r + EPS {
                            Some(0.0)
                        } else if r <= big_r + EPS {
                            let r_clamped = r.min(big_r).max(flat_r);
                            let dr = r_clamped - flat_r;
                            Some(cr - (cr * cr - dr * dr).sqrt())
                        } else {
                            None
                        }
                    }
                }
            }

            ToolType::VBit => {
                if r <= big_r + EPS {
                    let half_angle = (self.included_angle.unwrap_or(90.0) / 2.0).to_radians();
                    Some(r / half_angle.tan())
                } else {
                    None
                }
            }

            ToolType::Drill => {
                if r <= big_r + EPS {
                    let half_angle = (self.point_angle.unwrap_or(118.0) / 2.0).to_radians();
                    Some(r / half_angle.tan())
                } else {
                    None
                }
            }

            ToolType::CenterDrill => {
                let pilot_r = self.pilot_diameter.unwrap_or(self.diameter * 0.3) / 2.0;
                if r <= pilot_r + EPS {
                    let half_angle = (self.point_angle.unwrap_or(60.0) / 2.0).to_radians();
                    Some(r / half_angle.tan())
                } else {
                    None
                }
            }
        }
    }

    /// Replace sentinel (zero) geometry values with heuristic defaults.
    ///
    /// Resolution order matters: universal fields are resolved first because
    /// type-specific defaults (e.g. `pilot_length`) may depend on them.
    /// `taper_half_angle` is truly optional — `None` means no taper, so it
    /// is never defaulted.
    pub fn resolve_defaults(&mut self) {
        // --- Universal fields (must come first) ---
        if self.cutting_length == 0.0 {
            self.cutting_length = self.diameter * 3.0;
        }
        if self.shank_diameter == 0.0 {
            self.shank_diameter = self.diameter;
        }

        // --- Type-specific fields ---
        match self.tool_type {
            ToolType::BullNose => {
                if self.corner_radius.is_none() {
                    self.corner_radius = Some(self.diameter * 0.1);
                }
            }
            ToolType::VBit => {
                if self.included_angle.is_none() {
                    self.included_angle = Some(90.0);
                }
            }
            ToolType::Drill => {
                if self.point_angle.is_none() {
                    self.point_angle = Some(118.0);
                }
            }
            ToolType::CenterDrill => {
                if self.point_angle.is_none() {
                    self.point_angle = Some(60.0);
                }
                if self.pilot_diameter.is_none() {
                    self.pilot_diameter = Some(self.diameter * 0.3);
                }
                if self.pilot_length.is_none() {
                    self.pilot_length = Some(self.cutting_length / 3.0);
                }
            }
            ToolType::Tap | ToolType::ThreadMill => {
                if self.thread_pitch.is_none() {
                    self.thread_pitch = Some(1.0);
                }
            }
            ToolType::BoringBar => {
                if self.min_bore_diameter.is_none() {
                    self.min_bore_diameter = Some(self.diameter * 1.5);
                }
            }
            // FlatEndmill, BallNose, Reamer: no type-specific defaults
            // (taper_half_angle is never defaulted)
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tool() -> Tool {
        Tool {
            id: Uuid::parse_str("7f3c1a00-0000-0000-0000-000000000001").unwrap(),
            name: "10mm 4F Flat Endmill".to_string(),
            tool_type: ToolType::FlatEndmill,
            material: Some("carbide".to_string()),
            diameter: 10.0,
            flute_count: Some(4),
            default_spindle_speed: Some(15000),
            default_feed_rate: Some(2400.0),
            cutting_length: 30.0,
            shank_diameter: 10.0,
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
    fn tool_serde_round_trip() {
        let original = make_tool();
        let json = serde_json::to_string(&original).expect("serialize Tool");
        let recovered: Tool = serde_json::from_str(&json).expect("deserialize Tool");
        assert_eq!(original, recovered);
    }

    #[test]
    fn tool_type_field_serializes_as_type_key() {
        let tool = make_tool();
        let value = serde_json::to_value(&tool).expect("to_value");
        assert_eq!(value["type"], "flat_endmill");
        assert!(
            value.get("toolType").is_none(),
            "must not have toolType key"
        );
    }

    #[test]
    fn tool_fields_are_camel_case() {
        let tool = make_tool();
        let value = serde_json::to_value(&tool).expect("to_value");
        assert!(value.get("fluteCount").is_some());
        assert!(value.get("flute_count").is_none());
        assert!(value.get("defaultSpindleSpeed").is_some());
        assert!(value.get("defaultFeedRate").is_some());
    }

    #[test]
    fn tool_optional_fields_absent_when_none() {
        let tool = Tool {
            id: Uuid::new_v4(),
            name: "Drill".to_string(),
            tool_type: ToolType::Drill,
            material: None,
            diameter: 6.0,
            flute_count: Some(2),
            default_spindle_speed: None,
            default_feed_rate: None,
            cutting_length: 18.0,
            shank_diameter: 6.0,
            overall_length: None,
            corner_radius: None,
            included_angle: None,
            point_angle: None,
            pilot_diameter: None,
            pilot_length: None,
            thread_pitch: None,
            min_bore_diameter: None,
            taper_half_angle: None,
        };
        let value = serde_json::to_value(&tool).expect("to_value");
        assert!(value.get("defaultSpindleSpeed").is_none());
        assert!(value.get("defaultFeedRate").is_none());
        assert!(value.get("material").is_none());
        assert!(value.get("overallLength").is_none());
    }

    #[test]
    fn backward_compat_deserialize_then_resolve_defaults() {
        // JSON without the new geometry fields — simulates an old .jcam file.
        let json = r#"{
            "id": "7f3c1a00-0000-0000-0000-000000000001",
            "name": "10mm 4F Flat Endmill",
            "type": "flat_endmill",
            "material": "carbide",
            "diameter": 10.0,
            "fluteCount": 4
        }"#;
        let mut tool: Tool = serde_json::from_str(json).expect("deserialize");
        assert_eq!(tool.cutting_length, 0.0);
        assert_eq!(tool.shank_diameter, 0.0);
        assert!(tool.overall_length.is_none());

        tool.resolve_defaults();
        assert_eq!(tool.cutting_length, 30.0); // diameter * 3
        assert_eq!(tool.shank_diameter, 10.0); // diameter
        assert!(tool.overall_length.is_none()); // no longer auto-filled
    }

    #[test]
    fn geometry_fields_round_trip() {
        let mut original = make_tool();
        original.overall_length = Some(90.0);
        assert_eq!(original.cutting_length, 30.0);
        assert_eq!(original.shank_diameter, 10.0);
        assert_eq!(original.overall_length, Some(90.0));

        let json = serde_json::to_string(&original).expect("serialize");
        let recovered: Tool = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, recovered);
    }

    #[test]
    fn resolve_defaults_chain_uses_resolved_cutting_length() {
        let mut tool = Tool {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            tool_type: ToolType::FlatEndmill,
            material: Some("carbide".to_string()),
            diameter: 10.0,
            flute_count: Some(4),
            default_spindle_speed: None,
            default_feed_rate: None,
            cutting_length: 20.0, // explicit, non-zero
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
        };
        tool.resolve_defaults();
        // cutting_length kept as-is (already non-zero)
        assert_eq!(tool.cutting_length, 20.0);
        // shank_diameter defaults to diameter
        assert_eq!(tool.shank_diameter, 10.0);
        // overall_length stays None (no longer auto-filled)
        assert!(tool.overall_length.is_none());
    }

    #[test]
    fn all_tool_types_round_trip() {
        let types = [
            ToolType::FlatEndmill,
            ToolType::BallNose,
            ToolType::BullNose,
            ToolType::VBit,
            ToolType::Drill,
            ToolType::CenterDrill,
            ToolType::Tap,
            ToolType::Reamer,
            ToolType::BoringBar,
            ToolType::ThreadMill,
        ];
        for tt in &types {
            let json = serde_json::to_string(tt).expect("serialize ToolType");
            let recovered: ToolType = serde_json::from_str(&json).expect("deserialize ToolType");
            assert_eq!(tt, &recovered);
        }
    }

    // ---- Type-specific geometry tests ----

    /// Helper: build a minimal old-format JSON string (no type-specific fields)
    /// for any tool type.
    fn old_format_json(tool_type_str: &str, diameter: f64) -> String {
        format!(
            r#"{{
                "id": "7f3c1a00-0000-0000-0000-000000000001",
                "name": "test",
                "type": "{}",
                "material": "carbide",
                "diameter": {},
                "fluteCount": 2
            }}"#,
            tool_type_str, diameter
        )
    }

    #[test]
    fn backward_compat_flat_endmill_defaults() {
        let json = old_format_json("flat_endmill", 10.0);
        let mut tool: Tool = serde_json::from_str(&json).expect("deserialize");
        tool.resolve_defaults();
        // No type-specific fields should be set for FlatEndmill.
        assert_eq!(tool.corner_radius, None);
        assert_eq!(tool.included_angle, None);
        assert_eq!(tool.point_angle, None);
        assert_eq!(tool.pilot_diameter, None);
        assert_eq!(tool.pilot_length, None);
        assert_eq!(tool.thread_pitch, None);
        assert_eq!(tool.min_bore_diameter, None);
        assert_eq!(tool.taper_half_angle, None);
    }

    #[test]
    fn backward_compat_ball_nose_defaults() {
        let json = old_format_json("ball_nose", 10.0);
        let mut tool: Tool = serde_json::from_str(&json).expect("deserialize");
        tool.resolve_defaults();
        // BallNose has no type-specific defaults.
        assert_eq!(tool.corner_radius, None);
        assert_eq!(tool.included_angle, None);
        assert_eq!(tool.point_angle, None);
    }

    #[test]
    fn backward_compat_bull_nose_defaults() {
        let json = old_format_json("bull_nose", 10.0);
        let mut tool: Tool = serde_json::from_str(&json).expect("deserialize");
        tool.resolve_defaults();
        assert_eq!(tool.corner_radius, Some(1.0)); // diameter * 0.1
        assert_eq!(tool.taper_half_angle, None); // truly optional
    }

    #[test]
    fn backward_compat_vbit_defaults() {
        let json = old_format_json("v_bit", 10.0);
        let mut tool: Tool = serde_json::from_str(&json).expect("deserialize");
        tool.resolve_defaults();
        assert_eq!(tool.included_angle, Some(90.0));
    }

    #[test]
    fn backward_compat_drill_defaults() {
        let json = old_format_json("drill", 10.0);
        let mut tool: Tool = serde_json::from_str(&json).expect("deserialize");
        tool.resolve_defaults();
        assert_eq!(tool.point_angle, Some(118.0));
    }

    #[test]
    fn backward_compat_center_drill_defaults() {
        let json = old_format_json("center_drill", 10.0);
        let mut tool: Tool = serde_json::from_str(&json).expect("deserialize");
        tool.resolve_defaults();
        assert_eq!(tool.point_angle, Some(60.0));
        assert_eq!(tool.pilot_diameter, Some(3.0)); // diameter * 0.3
        assert_eq!(tool.pilot_length, Some(10.0)); // cutting_length(30) / 3
    }

    #[test]
    fn backward_compat_tap_defaults() {
        let json = old_format_json("tap", 10.0);
        let mut tool: Tool = serde_json::from_str(&json).expect("deserialize");
        tool.resolve_defaults();
        assert_eq!(tool.thread_pitch, Some(1.0));
    }

    #[test]
    fn backward_compat_reamer_defaults() {
        let json = old_format_json("reamer", 10.0);
        let mut tool: Tool = serde_json::from_str(&json).expect("deserialize");
        tool.resolve_defaults();
        // Reamer has no type-specific defaults.
        assert_eq!(tool.corner_radius, None);
        assert_eq!(tool.thread_pitch, None);
    }

    #[test]
    fn backward_compat_boring_bar_defaults() {
        let json = old_format_json("boring_bar", 10.0);
        let mut tool: Tool = serde_json::from_str(&json).expect("deserialize");
        tool.resolve_defaults();
        assert_eq!(tool.min_bore_diameter, Some(15.0)); // diameter * 1.5
    }

    #[test]
    fn backward_compat_thread_mill_defaults() {
        let json = old_format_json("thread_mill", 10.0);
        let mut tool: Tool = serde_json::from_str(&json).expect("deserialize");
        tool.resolve_defaults();
        assert_eq!(tool.thread_pitch, Some(1.0));
    }

    #[test]
    fn bull_nose_corner_radius_round_trip() {
        let mut tool = make_tool();
        tool.tool_type = ToolType::BullNose;
        tool.corner_radius = Some(2.5);
        let json = serde_json::to_string(&tool).expect("serialize");
        let recovered: Tool = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(recovered.corner_radius, Some(2.5));
    }

    #[test]
    fn taper_half_angle_absent_is_none() {
        let json = old_format_json("flat_endmill", 10.0);
        let tool: Tool = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(tool.taper_half_angle, None);
    }

    #[test]
    fn taper_half_angle_present_round_trips() {
        let mut tool = make_tool();
        tool.taper_half_angle = Some(3.0);
        let json = serde_json::to_string(&tool).expect("serialize");
        let recovered: Tool = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(recovered.taper_half_angle, Some(3.0));
    }

    #[test]
    fn taper_half_angle_not_defaulted_by_resolve() {
        let json = old_format_json("flat_endmill", 10.0);
        let mut tool: Tool = serde_json::from_str(&json).expect("deserialize");
        tool.resolve_defaults();
        assert_eq!(tool.taper_half_angle, None);

        let json = old_format_json("bull_nose", 10.0);
        let mut tool: Tool = serde_json::from_str(&json).expect("deserialize");
        tool.resolve_defaults();
        assert_eq!(tool.taper_half_angle, None);
    }

    #[test]
    fn cross_type_irrelevance_flat_endmill() {
        let json = old_format_json("flat_endmill", 10.0);
        let mut tool: Tool = serde_json::from_str(&json).expect("deserialize");
        tool.resolve_defaults();
        assert_eq!(tool.corner_radius, None);
        assert_eq!(tool.included_angle, None);
        assert_eq!(tool.point_angle, None);
        assert_eq!(tool.pilot_diameter, None);
        assert_eq!(tool.pilot_length, None);
        assert_eq!(tool.thread_pitch, None);
        assert_eq!(tool.min_bore_diameter, None);
        assert_eq!(tool.taper_half_angle, None);
    }

    #[test]
    fn center_drill_pilot_length_uses_resolved_cutting_length() {
        // Verify that pilot_length depends on *resolved* cutting_length.
        let json = old_format_json("center_drill", 6.0);
        let mut tool: Tool = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(tool.cutting_length, 0.0); // sentinel
        tool.resolve_defaults();
        // cutting_length resolved to 6.0 * 3 = 18.0
        assert_eq!(tool.cutting_length, 18.0);
        // pilot_length = cutting_length / 3 = 6.0
        assert_eq!(tool.pilot_length, Some(6.0));
    }

    #[test]
    fn explicit_type_specific_values_preserved_by_resolve() {
        let mut tool = make_tool();
        tool.tool_type = ToolType::BullNose;
        tool.corner_radius = Some(5.0); // explicit, not the default
        tool.resolve_defaults();
        assert_eq!(tool.corner_radius, Some(5.0)); // preserved, not overwritten
    }

    #[test]
    fn type_specific_fields_absent_in_json_when_none() {
        let tool = make_tool(); // FlatEndmill, all type-specific = None
        let value = serde_json::to_value(&tool).expect("to_value");
        assert!(value.get("cornerRadius").is_none());
        assert!(value.get("includedAngle").is_none());
        assert!(value.get("pointAngle").is_none());
        assert!(value.get("pilotDiameter").is_none());
        assert!(value.get("pilotLength").is_none());
        assert!(value.get("threadPitch").is_none());
        assert!(value.get("minBoreDiameter").is_none());
        assert!(value.get("taperHalfAngle").is_none());
    }

    // ---- z_clearance tests ----

    /// Helper: build a resolved tool of the given type with diameter 10.0.
    fn make_resolved_tool(tool_type: ToolType) -> Tool {
        let mut tool = make_tool();
        tool.tool_type = tool_type;
        tool.resolve_defaults();
        tool
    }

    fn assert_approx(actual: f64, expected: f64, tol: f64, msg: &str) {
        assert!(
            (actual - expected).abs() < tol,
            "{}: expected {}, got {} (diff {})",
            msg,
            expected,
            actual,
            (actual - expected).abs()
        );
    }

    #[test]
    fn z_clearance_zero_for_all_types() {
        // Universal invariant: z_clearance(0.0) == Some(0.0) for all tool types.
        let types = [
            ToolType::FlatEndmill,
            ToolType::BallNose,
            ToolType::BullNose,
            ToolType::VBit,
            ToolType::Drill,
            ToolType::CenterDrill,
            ToolType::Tap,
            ToolType::Reamer,
            ToolType::BoringBar,
            ToolType::ThreadMill,
        ];
        for tt in &types {
            let tool = make_resolved_tool(tt.clone());
            assert_eq!(
                tool.z_clearance(0.0),
                Some(0.0),
                "z_clearance(0) should be Some(0.0) for {:?}",
                tt
            );
        }
    }

    #[test]
    fn z_clearance_flat_endmill_10mm() {
        let tool = make_resolved_tool(ToolType::FlatEndmill);
        // R = 5.0
        assert_eq!(tool.z_clearance(5.0), Some(0.0));
        assert_eq!(tool.z_clearance(5.0 + 1.0), None);
    }

    #[test]
    fn z_clearance_flat_endmill_boundary() {
        let tool = make_resolved_tool(ToolType::FlatEndmill);
        // Just inside boundary
        assert_eq!(tool.z_clearance(4.999), Some(0.0));
        // At boundary
        assert_eq!(tool.z_clearance(5.0), Some(0.0));
        // Just outside (beyond epsilon)
        assert_eq!(tool.z_clearance(5.001), None);
    }

    #[test]
    fn z_clearance_tapered_flat_endmill() {
        let mut tool = make_tool();
        tool.tool_type = ToolType::FlatEndmill;
        tool.taper_half_angle = Some(5.0); // 5 degrees
        tool.cutting_length = 30.0;
        tool.resolve_defaults();

        let big_r = 5.0;
        let tan_a = 5.0_f64.to_radians().tan();
        let r_top = big_r + 30.0 * tan_a;

        // At R: z = 0
        assert_eq!(tool.z_clearance(big_r), Some(0.0));

        // At R + delta
        let delta = 1.0;
        let r = big_r + delta;
        let expected = delta / tan_a;
        let result = tool.z_clearance(r).unwrap();
        assert_approx(result, expected, 1e-10, "tapered flat endmill at R+1");

        // At R_top: still within domain
        assert!(tool.z_clearance(r_top).is_some());

        // Beyond R_top: None
        assert_eq!(tool.z_clearance(r_top + 1.0), None);
    }

    #[test]
    fn z_clearance_ball_nose() {
        let tool = make_resolved_tool(ToolType::BallNose);
        let big_r = 5.0;

        // r = 0
        assert_eq!(tool.z_clearance(0.0), Some(0.0));

        // r = R/2 = 2.5
        let r: f64 = big_r / 2.0;
        let expected = big_r - (big_r * big_r - r * r).sqrt();
        let result = tool.z_clearance(r).unwrap();
        assert_approx(result, expected, 1e-10, "ball nose at R/2");

        // r = R = 5.0
        let expected_at_r = big_r - (big_r * big_r - big_r * big_r).sqrt(); // = R
        let result = tool.z_clearance(big_r).unwrap();
        assert_approx(result, expected_at_r, 1e-10, "ball nose at R");
        assert_approx(result, big_r, 1e-10, "ball nose at R equals R");

        // Outside
        assert_eq!(tool.z_clearance(big_r + 1.0), None);
    }

    #[test]
    fn z_clearance_bull_nose_boundary() {
        let mut tool = make_tool();
        tool.tool_type = ToolType::BullNose;
        tool.corner_radius = Some(2.0);
        tool.resolve_defaults();

        let big_r = 5.0;
        let cr = 2.0;

        // r = R - cr = 3.0: flat region boundary → Some(0.0)
        assert_eq!(tool.z_clearance(big_r - cr), Some(0.0));

        // Just inside arc at r approaching R:
        // z = cr - sqrt(cr² - (r - (R - cr))²)
        // At r = R = 5.0: z = cr - sqrt(cr² - cr²) = cr = 2.0
        let result = tool.z_clearance(big_r).unwrap();
        assert_approx(result, cr, 1e-10, "bull nose at R");

        // At r = R - cr + cr/2 = 3.0 + 1.0 = 4.0:
        let r = big_r - cr + cr / 2.0;
        let dr = r - (big_r - cr);
        let expected = cr - (cr * cr - dr * dr).sqrt();
        let result = tool.z_clearance(r).unwrap();
        assert_approx(result, expected, 1e-10, "bull nose mid-arc");

        // Outside
        assert_eq!(tool.z_clearance(big_r + 1.0), None);
    }

    #[test]
    fn z_clearance_bull_nose_tapered() {
        let mut tool = make_tool();
        tool.tool_type = ToolType::BullNose;
        tool.corner_radius = Some(2.0);
        tool.taper_half_angle = Some(5.0);
        tool.cutting_length = 30.0;
        tool.resolve_defaults();

        let big_r = 5.0;
        let cr = 2.0;
        let tan_a = 5.0_f64.to_radians().tan();
        let r_top = big_r + (30.0 - cr) * tan_a;

        // Flat region
        assert_eq!(tool.z_clearance(big_r - cr), Some(0.0));

        // Arc region at R
        let result = tool.z_clearance(big_r).unwrap();
        assert_approx(result, cr, 1e-10, "tapered bull nose at R");

        // Taper region: r = R + 1.0
        let r = big_r + 1.0;
        let expected = cr + (r - big_r) / tan_a;
        let result = tool.z_clearance(r).unwrap();
        assert_approx(result, expected, 1e-10, "tapered bull nose taper region");

        // At R_top: still valid
        assert!(tool.z_clearance(r_top).is_some());

        // Beyond R_top
        assert_eq!(tool.z_clearance(r_top + 1.0), None);
    }

    #[test]
    fn z_clearance_vbit() {
        let mut tool = make_tool();
        tool.tool_type = ToolType::VBit;
        tool.included_angle = Some(90.0);
        tool.resolve_defaults();

        let big_r = 5.0;
        let half_angle = (90.0_f64 / 2.0).to_radians();

        // r = 0
        assert_eq!(tool.z_clearance(0.0), Some(0.0));

        // r = 3.0
        let r = 3.0;
        let expected = r / half_angle.tan();
        let result = tool.z_clearance(r).unwrap();
        assert_approx(result, expected, 1e-10, "vbit at r=3");

        // r = R: z = R / tan(45°) = R
        let result = tool.z_clearance(big_r).unwrap();
        assert_approx(result, big_r, 1e-10, "90° vbit at R: z = R");

        // Outside
        assert_eq!(tool.z_clearance(big_r + 1.0), None);
    }

    #[test]
    fn z_clearance_drill() {
        let mut tool = make_tool();
        tool.tool_type = ToolType::Drill;
        tool.point_angle = Some(118.0);
        tool.resolve_defaults();

        let big_r = 5.0;
        let half_angle = (118.0_f64 / 2.0).to_radians();

        // r = 0
        assert_eq!(tool.z_clearance(0.0), Some(0.0));

        // r = R
        let expected = big_r / half_angle.tan();
        let result = tool.z_clearance(big_r).unwrap();
        assert_approx(result, expected, 1e-10, "drill at R");

        // Outside
        assert_eq!(tool.z_clearance(big_r + 1.0), None);
    }

    #[test]
    fn z_clearance_center_drill_domain() {
        let mut tool = make_tool();
        tool.tool_type = ToolType::CenterDrill;
        tool.point_angle = Some(60.0);
        tool.pilot_diameter = Some(3.0);
        tool.resolve_defaults();

        let pilot_r = 1.5;
        let half_angle = (60.0_f64 / 2.0).to_radians();

        // At pilot_R
        let expected = pilot_r / half_angle.tan();
        let result = tool.z_clearance(pilot_r).unwrap();
        assert_approx(result, expected, 1e-10, "center drill at pilot_R");

        // Beyond pilot_R → None
        assert_eq!(tool.z_clearance(pilot_r + 1.0), None);
    }

    #[test]
    fn z_clearance_outside_returns_none_all_non_tapered() {
        let types = [
            ToolType::FlatEndmill,
            ToolType::BallNose,
            ToolType::BullNose,
            ToolType::VBit,
            ToolType::Drill,
            ToolType::Tap,
            ToolType::Reamer,
            ToolType::BoringBar,
            ToolType::ThreadMill,
        ];
        for tt in &types {
            let tool = make_resolved_tool(tt.clone());
            let big_r = tool.diameter / 2.0;
            assert_eq!(
                tool.z_clearance(big_r + 1.0),
                None,
                "z_clearance outside R should be None for {:?}",
                tt
            );
        }
    }

    #[test]
    fn z_clearance_center_drill_outside_pilot_returns_none() {
        let tool = make_resolved_tool(ToolType::CenterDrill);
        let pilot_r = tool.pilot_diameter.unwrap() / 2.0;
        // Even within the full tool radius, beyond pilot_R → None
        assert_eq!(tool.z_clearance(pilot_r + 1.0), None);
    }

    #[test]
    fn z_clearance_cylindrical_types_match_flat_endmill() {
        // Tap, Reamer, BoringBar, ThreadMill behave identically to straight FlatEndmill
        let flat = make_resolved_tool(ToolType::FlatEndmill);
        let cylindrical_types = [
            ToolType::Tap,
            ToolType::Reamer,
            ToolType::BoringBar,
            ToolType::ThreadMill,
        ];
        let test_radii = [0.0, 2.5, 5.0, 5.001];
        for tt in &cylindrical_types {
            let tool = make_resolved_tool(tt.clone());
            for &r in &test_radii {
                assert_eq!(
                    tool.z_clearance(r),
                    flat.z_clearance(r),
                    "{:?} at r={} should match FlatEndmill",
                    tt,
                    r
                );
            }
        }
    }
}
