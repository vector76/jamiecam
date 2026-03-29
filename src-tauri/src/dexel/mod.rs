pub mod convert;
pub mod grid;
pub mod types;

pub use convert::toolpath_to_segments;
pub use grid::DexelGrid;
pub use types::{DexelColumn, MotionSegment, ZSpan};
