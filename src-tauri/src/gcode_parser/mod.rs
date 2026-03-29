//! G-code parser: reads ISO 6983 G-code text and produces structured motion data.

pub(crate) mod modal;
pub(crate) mod state;
pub mod types;

pub use types::{
    FeedMode, MotionSegment, ParseWarning, ParsedProgram, Plane, ProgramMetadata, SegmentMetadata,
    SpindleDir, ToolChange, Units,
};
