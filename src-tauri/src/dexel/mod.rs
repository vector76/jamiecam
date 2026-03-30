pub mod convert;
pub mod grid;
pub mod mesh;
pub mod tool_profile;
pub mod types;

pub use convert::toolpath_to_segments;
pub use grid::DexelGrid;
pub use tool_profile::{ball_nose_clearance, clearance_for_tool, flat_endmill_clearance};
pub use types::{DexelColumn, MotionSegment, ZSpan};
