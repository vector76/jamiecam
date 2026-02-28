pub mod arcs;
pub mod block;
pub mod config;
pub mod formatter;
pub mod modal;
pub mod program;

use crate::toolpath::Toolpath;
use serde::Serialize;

/// Internal error type for post-processor failures.
/// The IPC layer maps these to AppError::PostProcessor at the boundary.
#[derive(Debug, thiserror::Error)]
pub enum PostProcessorError {
    #[error("config error: {0}")]
    Config(String),
    #[error("not supported: {0}")]
    NotSupported(String),
    #[error("arc error: {0}")]
    ArcError(String),
    #[error("program assembly error: {0}")]
    Assembly(String),
}

pub(crate) const FANUC_0I_TOML: &str = include_str!("builtins/fanuc-0i.toml");
pub(crate) const LINUXCNC_TOML: &str = include_str!("builtins/linuxcnc.toml");
pub(crate) const MACH4_TOML: &str = include_str!("builtins/mach4.toml");
pub(crate) const GRBL_TOML: &str = include_str!("builtins/grbl.toml");

/// Metadata for a post-processor, returned by `list_builtins()`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostProcessorMeta {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// The post-processor engine. Loaded from a config, used to generate G-code.
pub struct PostProcessor {
    pub(crate) config: config::PostProcessorConfig,
}

impl PostProcessor {
    /// Load a builtin post-processor by its `meta.id` string.
    pub fn builtin(id: &str) -> Result<Self, PostProcessorError> {
        let toml = match id {
            "fanuc-0i" => FANUC_0I_TOML,
            "linuxcnc" => LINUXCNC_TOML,
            "mach4" => MACH4_TOML,
            "grbl" => GRBL_TOML,
            _ => {
                return Err(PostProcessorError::Config(format!(
                    "unknown builtin id: {}",
                    id
                )))
            }
        };
        config::parse(toml).map(|c| Self { config: c })
    }

    /// Load a post-processor from a TOML file on disk.
    pub fn from_file(path: &std::path::Path) -> Result<Self, PostProcessorError> {
        let toml =
            std::fs::read_to_string(path).map_err(|e| PostProcessorError::Config(e.to_string()))?;
        config::parse(&toml).map(|c| Self { config: c })
    }

    /// List all builtin post-processor metadata (id, name, description).
    pub fn list_builtins() -> Vec<PostProcessorMeta> {
        [FANUC_0I_TOML, LINUXCNC_TOML, MACH4_TOML, GRBL_TOML]
            .iter()
            .filter_map(|toml| config::parse(toml).ok())
            .map(|c| PostProcessorMeta {
                id: c.meta.id,
                name: c.meta.name,
                description: c.meta.description,
            })
            .collect()
    }

    /// Generate G-code from the given toolpaths.
    ///
    /// `tool_infos` carries tool library data (diameter, description) used for
    /// template variable substitution in `tool_change.command`. Build it from
    /// `project.tools` before calling. Pass `&[]` if no tool data is needed.
    pub fn generate(
        &self,
        toolpaths: &[Toolpath],
        tool_infos: &[program::ToolInfo],
        options: program::GenerateOptions,
    ) -> Result<String, PostProcessorError> {
        program::assemble(toolpaths, tool_infos, &self.config, &options)
    }
}

/// Re-export so callers can name `ToolInfo` without importing `program` directly.
pub use program::ToolInfo;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fanuc_0i_builtin_parses_without_error() {
        config::parse(FANUC_0I_TOML).unwrap();
    }

    #[test]
    fn linuxcnc_builtin_parses_without_error() {
        config::parse(LINUXCNC_TOML).unwrap();
    }

    #[test]
    fn mach4_builtin_parses_without_error() {
        config::parse(MACH4_TOML).unwrap();
    }

    #[test]
    fn grbl_builtin_parses_without_error() {
        config::parse(GRBL_TOML).unwrap();
    }

    #[test]
    fn list_builtins_returns_four_entries() {
        let builtins = PostProcessor::list_builtins();
        assert_eq!(builtins.len(), 4);
        let ids: Vec<&str> = builtins.iter().map(|b| b.id.as_str()).collect();
        assert!(ids.contains(&"fanuc-0i"));
        assert!(ids.contains(&"linuxcnc"));
        assert!(ids.contains(&"mach4"));
        assert!(ids.contains(&"grbl"));
    }

    #[test]
    fn builtin_fanuc_0i_loads_without_error() {
        PostProcessor::builtin("fanuc-0i").unwrap();
    }

    #[test]
    fn builtin_unknown_id_returns_error() {
        let result = PostProcessor::builtin("nonexistent");
        assert!(matches!(result, Err(PostProcessorError::Config(_))));
    }

    #[test]
    fn generate_returns_gcode_string() {
        use crate::models::Vec3;
        use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind};
        use uuid::Uuid;

        let toolpath = Toolpath {
            operation_id: Uuid::nil(),
            tool_number: 1,
            spindle_speed: 8000.0,
            feed_rate: 500.0,
            passes: vec![Pass {
                kind: PassKind::Cutting,
                cuts: vec![
                    CutPoint {
                        position: Vec3 {
                            x: 0.0,
                            y: 0.0,
                            z: 5.0,
                        },
                        move_kind: MoveKind::Rapid,
                        tool_orientation: None,
                    },
                    CutPoint {
                        position: Vec3 {
                            x: 10.0,
                            y: 0.0,
                            z: 0.0,
                        },
                        move_kind: MoveKind::Feed,
                        tool_orientation: None,
                    },
                ],
            }],
        };

        let result = PostProcessor::builtin("linuxcnc")
            .unwrap()
            .generate(
                &[toolpath],
                &[],
                program::GenerateOptions {
                    program_number: Some(1),
                    include_comments: false,
                },
            )
            .unwrap();

        assert!(
            result.contains("G01") || result.contains("G1"),
            "expected G01 or G1 in output, got:\n{}",
            result
        );
    }
}
