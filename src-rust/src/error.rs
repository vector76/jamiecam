//! Application-level error type returned by wasm entry points.
//!
//! Serialized with serde's adjacently-tagged representation:
//! `{ "kind": "<variant>", "message": "<content>" }`. The `message` payload is
//! a string for simple variants and a struct for richer ones (e.g.
//! [`ParseFailure`]).
//!
//! The TypeScript counterpart is a discriminated union on `kind` — see
//! `src/api/types.ts`.

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    #[error("{0}")]
    Io(String),

    #[error("{0}")]
    InvalidInput(String),

    #[error("{}: {}", .0.source, .0.message)]
    ParseFailure(ParseFailure),

    #[error("missing setup: {id}")]
    MissingSetup { id: String },

    #[error("missing tool: {id}")]
    MissingTool { id: String },
}

/// Detail payload for [`AppError::ParseFailure`]. Used when a parser cannot
/// produce any structured output (as opposed to recoverable warnings, which
/// are reported via [`crate::parse_warning::ParseWarning`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseFailure {
    /// Short identifier of the parser that failed (e.g. "gcode", "svg", "dxf").
    pub source: String,
    /// Human-readable failure description.
    pub message: String,
    /// Source line number, when applicable.
    pub line: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_input_serializes_to_kind_message() {
        let err = AppError::InvalidInput("bad value".to_string());
        let value = serde_json::to_value(&err).unwrap();
        assert_eq!(value["kind"], "InvalidInput");
        assert_eq!(value["message"], "bad value");
    }

    #[test]
    fn io_error_serializes_to_kind_message() {
        let err = AppError::Io("read failed".to_string());
        let value = serde_json::to_value(&err).unwrap();
        assert_eq!(value["kind"], "Io");
        assert_eq!(value["message"], "read failed");
    }

    #[test]
    fn parse_failure_serializes_with_struct_payload() {
        let err = AppError::ParseFailure(ParseFailure {
            source: "svg".into(),
            message: "unexpected token".into(),
            line: Some(12),
        });
        let value = serde_json::to_value(&err).unwrap();
        assert_eq!(value["kind"], "ParseFailure");
        assert_eq!(value["message"]["source"], "svg");
        assert_eq!(value["message"]["message"], "unexpected token");
        assert_eq!(value["message"]["line"], 12);
    }

    #[test]
    fn parse_failure_serializes_null_line_when_absent() {
        let err = AppError::ParseFailure(ParseFailure {
            source: "dxf".into(),
            message: "binary header rejected".into(),
            line: None,
        });
        let value = serde_json::to_value(&err).unwrap();
        assert!(value["message"]["line"].is_null());
    }

    #[test]
    fn parse_failure_round_trips_via_json() {
        let err = AppError::ParseFailure(ParseFailure {
            source: "gcode".into(),
            message: "malformed token".into(),
            line: Some(3),
        });
        let json = serde_json::to_string(&err).unwrap();
        let back: AppError = serde_json::from_str(&json).unwrap();
        match back {
            AppError::ParseFailure(detail) => {
                assert_eq!(detail.source, "gcode");
                assert_eq!(detail.message, "malformed token");
                assert_eq!(detail.line, Some(3));
            }
            other => panic!("expected ParseFailure, got {other:?}"),
        }
    }

    #[test]
    fn missing_setup_serializes_with_id_payload() {
        let err = AppError::MissingSetup {
            id: "setup-uuid-1".into(),
        };
        let value = serde_json::to_value(&err).unwrap();
        assert_eq!(value["kind"], "MissingSetup");
        assert_eq!(value["message"]["id"], "setup-uuid-1");
    }

    #[test]
    fn missing_tool_serializes_with_id_payload() {
        let err = AppError::MissingTool {
            id: "tool-uuid-1".into(),
        };
        let value = serde_json::to_value(&err).unwrap();
        assert_eq!(value["kind"], "MissingTool");
        assert_eq!(value["message"]["id"], "tool-uuid-1");
    }

    #[test]
    fn missing_setup_round_trips_via_json() {
        let err = AppError::MissingSetup { id: "abc".into() };
        let json = serde_json::to_string(&err).unwrap();
        let back: AppError = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, AppError::MissingSetup { id } if id == "abc"));
    }

    #[test]
    fn missing_tool_round_trips_via_json() {
        let err = AppError::MissingTool { id: "xyz".into() };
        let json = serde_json::to_string(&err).unwrap();
        let back: AppError = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, AppError::MissingTool { id } if id == "xyz"));
    }

    #[test]
    fn simple_variant_round_trips_via_json() {
        let err = AppError::InvalidInput("nope".to_string());
        let json = serde_json::to_string(&err).unwrap();
        let back: AppError = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, AppError::InvalidInput(s) if s == "nope"));
    }
}
