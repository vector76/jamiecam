//! Non-fatal warning produced by parsers (G-code, SVG, DXF, …).
//!
//! Promoted out of the G-code parser so every parser can emit a uniform
//! warning shape. `line` is optional because some formats (e.g. DXF binary)
//! have no meaningful source-line concept.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseWarning {
    pub line: Option<u32>,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_camel_case_with_line() {
        let w = ParseWarning {
            line: Some(42),
            message: "bad token".into(),
        };
        let v = serde_json::to_value(&w).unwrap();
        assert_eq!(v["line"], 42);
        assert_eq!(v["message"], "bad token");
    }

    #[test]
    fn serializes_null_line_when_absent() {
        let w = ParseWarning {
            line: None,
            message: "format-level issue".into(),
        };
        let v = serde_json::to_value(&w).unwrap();
        assert!(v["line"].is_null());
    }

    #[test]
    fn round_trips_via_json() {
        let w = ParseWarning {
            line: Some(7),
            message: "duplicate header".into(),
        };
        let json = serde_json::to_string(&w).unwrap();
        let back: ParseWarning = serde_json::from_str(&json).unwrap();
        assert_eq!(back, w);
    }
}
