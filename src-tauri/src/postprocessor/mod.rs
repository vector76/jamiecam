pub mod arcs;
pub mod block;
pub mod config;
pub mod formatter;

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
