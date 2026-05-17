//! Application-level error type returned by wasm entry points.
//!
//! Serialized with serde's adjacently-tagged representation:
//! `{ "kind": "<variant>", "message": "<human-readable text>" }`
//!
//! The TypeScript counterpart is:
//! ```ts
//! type AppError = { kind: string; message: string };
//! ```

#[derive(Debug, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    #[error("{0}")]
    Io(String),

    #[error("{0}")]
    InvalidInput(String),
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
}
