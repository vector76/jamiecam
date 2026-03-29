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
    pub material: String,
    /// Cutting diameter in project units (mm or inch).
    pub diameter: f64,
    /// Number of flutes (cutting edges).
    pub flute_count: u32,
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
    /// A value of 0.0 is a sentinel meaning "not yet set".
    #[serde(default)]
    pub overall_length: f64,
}

impl Tool {
    /// Replace sentinel (zero) geometry values with heuristic defaults.
    ///
    /// Resolution order matters: `overall_length` depends on the resolved
    /// `cutting_length`, not the raw value.
    pub fn resolve_defaults(&mut self) {
        if self.cutting_length == 0.0 {
            self.cutting_length = self.diameter * 3.0;
        }
        if self.shank_diameter == 0.0 {
            self.shank_diameter = self.diameter;
        }
        if self.overall_length == 0.0 {
            self.overall_length = self.cutting_length * 3.0;
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
            material: "carbide".to_string(),
            diameter: 10.0,
            flute_count: 4,
            default_spindle_speed: Some(15000),
            default_feed_rate: Some(2400.0),
            cutting_length: 30.0,
            shank_diameter: 10.0,
            overall_length: 90.0,
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
            material: "hss".to_string(),
            diameter: 6.0,
            flute_count: 2,
            default_spindle_speed: None,
            default_feed_rate: None,
            cutting_length: 18.0,
            shank_diameter: 6.0,
            overall_length: 54.0,
        };
        let value = serde_json::to_value(&tool).expect("to_value");
        assert!(value.get("defaultSpindleSpeed").is_none());
        assert!(value.get("defaultFeedRate").is_none());
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
        assert_eq!(tool.overall_length, 0.0);

        tool.resolve_defaults();
        assert_eq!(tool.cutting_length, 30.0); // diameter * 3
        assert_eq!(tool.shank_diameter, 10.0); // diameter
        assert_eq!(tool.overall_length, 90.0); // cutting_length * 3
    }

    #[test]
    fn geometry_fields_round_trip() {
        let original = make_tool();
        assert_eq!(original.cutting_length, 30.0);
        assert_eq!(original.shank_diameter, 10.0);
        assert_eq!(original.overall_length, 90.0);

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
            material: "carbide".to_string(),
            diameter: 10.0,
            flute_count: 4,
            default_spindle_speed: None,
            default_feed_rate: None,
            cutting_length: 20.0, // explicit, non-zero
            shank_diameter: 0.0,
            overall_length: 0.0,
        };
        tool.resolve_defaults();
        // cutting_length kept as-is (already non-zero)
        assert_eq!(tool.cutting_length, 20.0);
        // shank_diameter defaults to diameter
        assert_eq!(tool.shank_diameter, 10.0);
        // overall_length uses the *resolved* cutting_length (20), not diameter*3 (30)
        assert_eq!(tool.overall_length, 60.0); // 20 * 3
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
}
