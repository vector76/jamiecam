//! Deterministic SHA-256 cache key computation for toolpath operations.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

/// Compute a deterministic SHA-256 cache key for a toolpath operation.
///
/// The key covers operation geometry, tool geometry (excluding display-only
/// fields: `id`, `name`, `default_spindle_speed`, `default_feed_rate`), stock
/// definition, an optional model content hash, and an engine version string.
///
/// Fields that affect only execution behaviour (`spindle_speed_override`,
/// `feed_rate_override`) are intentionally excluded so that changing override
/// values alone does not invalidate a cached toolpath.
///
/// Returns a string of the form `"sha256:<lowercase hex digest>"`.
pub fn compute_cache_key(
    operation: &crate::models::Operation,
    tool: &crate::models::Tool,
    stock: &crate::models::StockDefinition,
    model_sha: Option<&str>,
    engine_version: &str,
) -> String {
    // Tool subset: stable geometry fields only.
    // Exclude id, name, default_spindle_speed, default_feed_rate.
    let mut tool_map: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
    tool_map.insert("diameter", serde_json::json!(tool.diameter));
    tool_map.insert("flute_count", serde_json::json!(tool.flute_count));
    tool_map.insert("material", serde_json::json!(tool.material));
    tool_map.insert(
        "type",
        serde_json::to_value(&tool.tool_type).expect("serialize ToolType"),
    );
    // Universal geometry
    tool_map.insert("cuttingLength", serde_json::json!(tool.cutting_length));
    tool_map.insert("shankDiameter", serde_json::json!(tool.shank_diameter));
    tool_map.insert("overallLength", serde_json::json!(tool.overall_length));
    // Type-specific geometry
    tool_map.insert("cornerRadius", serde_json::json!(tool.corner_radius));
    tool_map.insert("includedAngle", serde_json::json!(tool.included_angle));
    tool_map.insert("pointAngle", serde_json::json!(tool.point_angle));
    tool_map.insert("pilotDiameter", serde_json::json!(tool.pilot_diameter));
    tool_map.insert("pilotLength", serde_json::json!(tool.pilot_length));
    tool_map.insert("threadPitch", serde_json::json!(tool.thread_pitch));
    tool_map.insert("minBoreDiameter", serde_json::json!(tool.min_bore_diameter));
    tool_map.insert("taperHalfAngle", serde_json::json!(tool.taper_half_angle));

    // Operation value: serialize then strip override and future cache fields.
    let mut op_val = serde_json::to_value(operation).expect("serialize Operation");
    if let Some(obj) = op_val.as_object_mut() {
        obj.remove("spindleSpeedOverride");
        obj.remove("feedRateOverride");
        obj.remove("cache"); // excluded for future compatibility
    }

    // Canonical top-level map (BTreeMap guarantees alphabetical key order).
    let mut map: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
    map.insert("engine_version", serde_json::json!(engine_version));
    map.insert("model_sha", serde_json::json!(model_sha));
    map.insert("operation", op_val);
    map.insert(
        "stock",
        serde_json::to_value(stock).expect("serialize StockDefinition"),
    );
    map.insert(
        "tool",
        serde_json::to_value(&tool_map).expect("serialize tool map"),
    );

    let canonical = serde_json::to_vec(&map).expect("serialize canonical map");
    let hash = Sha256::digest(&canonical);
    let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
    format!("sha256:{hex}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        operation::{CacheState, CompensationSide, Operation, OperationParams, ProfileParams},
        stock::{BoxDimensions, StockDefinition, Vec3},
        tool::{Tool, ToolType},
    };
    use uuid::Uuid;

    fn make_tool() -> Tool {
        Tool {
            id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
            name: "10mm Flat Endmill".to_string(),
            tool_type: ToolType::FlatEndmill,
            material: Some("carbide".to_string()),
            diameter: 10.0,
            flute_count: Some(4),
            default_spindle_speed: None,
            default_feed_rate: None,
            cutting_length: 30.0,
            shank_diameter: 10.0,
            overall_length: Some(90.0),
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

    fn make_operation() -> Operation {
        Operation {
            id: Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap(),
            name: "Outer Profile".to_string(),
            enabled: true,
            tool_id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
            spindle_speed_override: None,
            feed_rate_override: None,
            workpiece_material: None,
            params: OperationParams::Profile(ProfileParams {
                depth: 10.0,
                stepdown: Some(2.5),
                compensation_side: CompensationSide::Left,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        }
    }

    fn make_stock() -> StockDefinition {
        StockDefinition::Box(BoxDimensions {
            origin: Vec3::zero(),
            width: 100.0,
            depth: 100.0,
            height: 20.0,
        })
    }

    #[test]
    fn same_inputs_produce_same_key() {
        let tool = make_tool();
        let op = make_operation();
        let stock = make_stock();
        let k1 = compute_cache_key(&op, &tool, &stock, Some("abc123"), "v1");
        let k2 = compute_cache_key(&op, &tool, &stock, Some("abc123"), "v1");
        assert_eq!(k1, k2);
    }

    #[test]
    fn changed_tool_diameter_changes_key() {
        let op = make_operation();
        let stock = make_stock();
        let tool1 = make_tool();
        let mut tool2 = make_tool();
        tool2.diameter = 12.0;
        let k1 = compute_cache_key(&op, &tool1, &stock, Some("abc123"), "v1");
        let k2 = compute_cache_key(&op, &tool2, &stock, Some("abc123"), "v1");
        assert_ne!(k1, k2);
    }

    #[test]
    fn changed_operation_params_changes_key() {
        let tool = make_tool();
        let stock = make_stock();
        let op1 = make_operation();
        let mut op2 = make_operation();
        op2.params = OperationParams::Profile(ProfileParams {
            depth: 15.0, // changed
            stepdown: Some(2.5),
            compensation_side: CompensationSide::Left,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        });
        let k1 = compute_cache_key(&op1, &tool, &stock, Some("abc123"), "v1");
        let k2 = compute_cache_key(&op2, &tool, &stock, Some("abc123"), "v1");
        assert_ne!(k1, k2);
    }

    #[test]
    fn none_model_sha_vs_real_hash_gives_different_keys() {
        let tool = make_tool();
        let op = make_operation();
        let stock = make_stock();
        let k1 = compute_cache_key(&op, &tool, &stock, None, "v1");
        let k2 = compute_cache_key(&op, &tool, &stock, Some("deadbeef1234"), "v1");
        assert_ne!(k1, k2);
    }

    #[test]
    fn changed_cutting_length_changes_key() {
        let op = make_operation();
        let stock = make_stock();
        let tool1 = make_tool();
        let mut tool2 = make_tool();
        tool2.cutting_length = 50.0;
        let k1 = compute_cache_key(&op, &tool1, &stock, Some("abc123"), "v1");
        let k2 = compute_cache_key(&op, &tool2, &stock, Some("abc123"), "v1");
        assert_ne!(k1, k2);
    }

    // ── Optional-field tests ─────────────────────────────────────────────────

    #[test]
    fn cache_key_with_none_fields_is_deterministic() {
        let op = make_operation();
        let stock = make_stock();
        let mut tool = make_tool();
        tool.material = None;
        tool.flute_count = None;
        tool.overall_length = None;

        let k1 = compute_cache_key(&op, &tool, &stock, Some("abc123"), "v1");
        let k2 = compute_cache_key(&op, &tool, &stock, Some("abc123"), "v1");
        assert_eq!(k1, k2, "cache key with None fields must be deterministic");
    }

    #[test]
    fn cache_key_changes_when_optional_field_goes_from_none_to_some() {
        let op = make_operation();
        let stock = make_stock();

        let mut tool_none = make_tool();
        tool_none.material = None;

        let mut tool_some = make_tool();
        tool_some.material = Some("carbide".to_string());

        let k_none = compute_cache_key(&op, &tool_none, &stock, Some("abc123"), "v1");
        let k_some = compute_cache_key(&op, &tool_some, &stock, Some("abc123"), "v1");
        assert_ne!(
            k_none, k_some,
            "cache key must differ when material changes from None to Some"
        );
    }

    #[test]
    fn changed_name_does_not_change_key() {
        let op = make_operation();
        let stock = make_stock();
        let tool1 = make_tool();
        let mut tool2 = make_tool();
        tool2.name = "Completely Different Name".to_string();
        let k1 = compute_cache_key(&op, &tool1, &stock, Some("abc123"), "v1");
        let k2 = compute_cache_key(&op, &tool2, &stock, Some("abc123"), "v1");
        assert_eq!(k1, k2);
    }
}
