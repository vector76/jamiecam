//! Application-level error type returned by all Tauri command handlers.
//!
//! `AppError` is serialized to `{ kind, message }` JSON payloads so the
//! TypeScript frontend can pattern-match on a stable `kind` string.

use crate::geometry::GeometryError;

/// Top-level error returned by Tauri command handlers.
///
/// Serialized with serde's adjacently-tagged representation:
/// `{ "kind": "<variant>", "message": "<human-readable text>" }`
///
/// The TypeScript counterpart is:
/// ```ts
/// type AppError = { kind: string; message: string };
/// ```
#[derive(Debug, thiserror::Error, serde::Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    /// A required file path does not exist on disk.
    #[error("file not found")]
    FileNotFound,

    /// A geometry import failed; the inner message comes from [`GeometryError`].
    #[error("{0}")]
    GeometryImport(String),

    /// A generic I/O error; the inner [`std::io::Error`] is converted to a
    /// string at the system boundary so it remains serializable.
    #[error("{0}")]
    Io(String),

    /// The project file could not be loaded or parsed.
    #[error("{0}")]
    ProjectLoad(String),

    /// The project file could not be written.
    #[error("{0}")]
    ProjectSave(String),

    /// The file extension is not supported by any importer.
    #[error("{0}")]
    UnsupportedFormat(String),

    /// A requested resource (tool, operation, etc.) was not found.
    #[error("{0}")]
    NotFound(String),
}

impl From<GeometryError> for AppError {
    /// Convert a [`GeometryError`] into an [`AppError::GeometryImport`].
    ///
    /// The geometry error is stringified here so that the enum variant stores
    /// a plain `String`, keeping the serialized shape as `{ kind, message }`.
    fn from(e: GeometryError) -> Self {
        Self::GeometryImport(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    /// Convert an [`std::io::Error`] into an [`AppError::Io`].
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_error_serializes_to_kind_message() {
        let err = AppError::Io("disk full".to_string());
        let value = serde_json::to_value(&err).expect("serialize AppError::Io");
        assert_eq!(value["kind"], "Io");
        assert_eq!(value["message"], "disk full");
    }

    #[test]
    fn project_load_error_serializes_to_kind_message() {
        let err = AppError::ProjectLoad("invalid JSON".to_string());
        let value = serde_json::to_value(&err).expect("serialize AppError::ProjectLoad");
        assert_eq!(value["kind"], "ProjectLoad");
        assert_eq!(value["message"], "invalid JSON");
    }

    #[test]
    fn geometry_import_error_serializes_to_kind_message() {
        let err = AppError::GeometryImport("tessellation failed".to_string());
        let value = serde_json::to_value(&err).expect("serialize AppError::GeometryImport");
        assert_eq!(value["kind"], "GeometryImport");
        assert_eq!(value["message"], "tessellation failed");
    }

    #[test]
    fn file_not_found_serializes_with_kind() {
        let err = AppError::FileNotFound;
        let value = serde_json::to_value(&err).expect("serialize AppError::FileNotFound");
        assert_eq!(value["kind"], "FileNotFound");
    }

    #[test]
    fn unsupported_format_serializes_to_kind_message() {
        let err = AppError::UnsupportedFormat(".xyz".to_string());
        let value = serde_json::to_value(&err).expect("serialize AppError::UnsupportedFormat");
        assert_eq!(value["kind"], "UnsupportedFormat");
        assert_eq!(value["message"], ".xyz");
    }

    #[test]
    fn from_geometry_error_produces_geometry_import_variant() {
        let geo_err = GeometryError::ImportFailed {
            message: "bad shape".to_string(),
        };
        let app_err = AppError::from(geo_err);
        assert!(matches!(app_err, AppError::GeometryImport(_)));
        let value = serde_json::to_value(&app_err).expect("serialize");
        assert_eq!(value["kind"], "GeometryImport");
    }

    #[test]
    fn from_io_error_produces_io_variant() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let app_err = AppError::from(io_err);
        assert!(matches!(app_err, AppError::Io(_)));
        let value = serde_json::to_value(&app_err).expect("serialize");
        assert_eq!(value["kind"], "Io");
    }

    #[test]
    fn not_found_error_serializes_to_kind_message() {
        let err = AppError::NotFound("tool abc123 not found".to_string());
        let value = serde_json::to_value(&err).expect("serialize AppError::NotFound");
        assert_eq!(value["kind"], "NotFound");
        assert_eq!(value["message"], "tool abc123 not found");
    }

    #[test]
    fn app_error_display_is_human_readable() {
        assert_eq!(AppError::FileNotFound.to_string(), "file not found");
        assert_eq!(
            AppError::Io("access denied".to_string()).to_string(),
            "access denied"
        );
        assert_eq!(
            AppError::ProjectSave("write failed".to_string()).to_string(),
            "write failed"
        );
    }
}
