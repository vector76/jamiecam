pub mod arcs;
pub mod block;
pub mod config;
pub mod formatter;
pub mod modal;
pub mod program;

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

#[allow(dead_code)]
pub(crate) const FANUC_0I_TOML: &str = include_str!("builtins/fanuc-0i.toml");
#[allow(dead_code)]
pub(crate) const LINUXCNC_TOML: &str = include_str!("builtins/linuxcnc.toml");
#[allow(dead_code)]
pub(crate) const MACH4_TOML: &str = include_str!("builtins/mach4.toml");
#[allow(dead_code)]
pub(crate) const GRBL_TOML: &str = include_str!("builtins/grbl.toml");

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
}
