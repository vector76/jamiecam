//! Feed/Speed library: TOML-backed lookup table for machining parameters.

use std::collections::HashMap;

use crate::error::AppError;

/// Embedded TOML source for the bundled feed/speed data.
pub const FEEDS_TOML: &str = include_str!("data/feeds.toml");

// ── Public data types ─────────────────────────────────────────────────────────

/// Metadata for a workpiece material, used by IPC list endpoints.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialMeta {
    pub id: String,
    pub display_name: String,
}

/// Feed/speed parameters for one material × tool_material × operation combination.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedEntry {
    pub spindle_speed_rpm: u32,
    pub feed_rate_mmpm: f64,
    pub doc_mm: Option<f64>,
}

// ── Library ───────────────────────────────────────────────────────────────────

/// In-memory feed/speed library built from the embedded TOML.
///
/// Constructed programmatically via [`FeedLibrary::from_toml`]; not
/// deserialized directly and never sent over IPC.
#[derive(Debug)]
pub struct FeedLibrary {
    /// Key: (workpiece_material_id, tool_material, operation_category)
    entries: HashMap<(String, String, String), FeedEntry>,
    materials: Vec<MaterialMeta>,
}

impl FeedLibrary {
    /// Parse a TOML string and build the library.
    ///
    /// Returns [`AppError::ProjectLoad`] if the TOML is malformed.
    pub fn from_toml(toml_str: &str) -> Result<Self, AppError> {
        #[derive(serde::Deserialize)]
        struct RawEntry {
            material_id: String,
            display_name: String,
            tool_material: String,
            operation_category: String,
            spindle_speed_rpm: u32,
            feed_rate_mmpm: f64,
            doc_mm: Option<f64>,
        }

        #[derive(serde::Deserialize)]
        struct RawFile {
            entry: Vec<RawEntry>,
        }

        let raw: RawFile = toml::from_str(toml_str)
            .map_err(|e| AppError::ProjectLoad(format!("feed library parse error: {e}")))?;

        let mut entries: HashMap<(String, String, String), FeedEntry> = HashMap::new();
        // Use a Vec + seen set to preserve first-seen order and deduplicate.
        let mut materials: Vec<MaterialMeta> = Vec::new();
        let mut seen_materials: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        for r in raw.entry {
            if seen_materials.insert(r.material_id.clone()) {
                materials.push(MaterialMeta {
                    id: r.material_id.clone(),
                    display_name: r.display_name.clone(),
                });
            }

            let key = (r.material_id, r.tool_material, r.operation_category);
            entries.insert(
                key,
                FeedEntry {
                    spindle_speed_rpm: r.spindle_speed_rpm,
                    feed_rate_mmpm: r.feed_rate_mmpm,
                    doc_mm: r.doc_mm,
                },
            );
        }

        Ok(Self { entries, materials })
    }

    /// Look up feed/speed parameters for a given combination.
    ///
    /// Returns [`AppError::NotFound`] if no entry matches.
    pub fn lookup(
        &self,
        material_id: &str,
        tool_material: &str,
        op_category: &str,
    ) -> Result<&FeedEntry, AppError> {
        let key = (
            material_id.to_string(),
            tool_material.to_string(),
            op_category.to_string(),
        );
        self.entries.get(&key).ok_or_else(|| {
            AppError::NotFound(format!(
                "no feed entry for material={material_id} tool={tool_material} op={op_category}"
            ))
        })
    }

    /// Return the list of known workpiece materials.
    pub fn materials(&self) -> &[MaterialMeta] {
        &self.materials
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> FeedLibrary {
        FeedLibrary::from_toml(FEEDS_TOML).expect("embedded FEEDS_TOML must parse")
    }

    #[test]
    fn from_toml_parses_embedded_data() {
        // Should succeed without panic.
        let lib = library();
        // Sanity: at least one entry loaded.
        assert!(!lib.entries.is_empty());
    }

    #[test]
    fn lookup_aluminum_carbide_roughing_returns_correct_rpm() {
        let lib = library();
        let entry = lib
            .lookup("aluminum-6061", "carbide", "roughing")
            .expect("entry must exist");
        assert_eq!(entry.spindle_speed_rpm, 10000);
    }

    #[test]
    fn lookup_unknown_material_returns_not_found() {
        let lib = library();
        let err = lib
            .lookup("unobtanium-99", "carbide", "roughing")
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[test]
    fn lookup_valid_material_and_tool_invalid_op_returns_not_found() {
        let lib = library();
        let err = lib
            .lookup("aluminum-6061", "carbide", "nonexistent-op")
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[test]
    fn lookup_valid_material_and_op_invalid_tool_returns_not_found() {
        let lib = library();
        let err = lib
            .lookup("aluminum-6061", "unobtanium-tool", "roughing")
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[test]
    fn materials_returns_four_entries() {
        let lib = library();
        assert_eq!(lib.materials().len(), 4);
    }
}
