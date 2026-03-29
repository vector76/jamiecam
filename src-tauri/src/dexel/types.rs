use crate::models::Vec3;

/// A vertical interval of material within a dexel column.
#[derive(Debug, Clone, PartialEq)]
pub struct ZSpan {
    pub z_min: f64,
    pub z_max: f64,
}

/// A single vertical column of material, represented as sorted, non-overlapping [`ZSpan`]s.
#[derive(Debug, Clone, PartialEq)]
pub struct DexelColumn {
    pub spans: Vec<ZSpan>,
}

impl DexelColumn {
    /// Remove all material above `z_floor`.
    ///
    /// - Spans entirely above `z_floor` are removed.
    /// - Spans straddling `z_floor` are truncated (`z_max` set to `z_floor`).
    /// - Spans entirely below `z_floor` are unchanged.
    pub fn remove_above(&mut self, z_floor: f64) {
        self.spans.retain_mut(|span| {
            if span.z_min >= z_floor {
                // Entirely above — remove.
                false
            } else if span.z_max > z_floor {
                // Straddles — truncate.
                span.z_max = z_floor;
                true
            } else {
                // Entirely below — keep.
                true
            }
        });
    }
}

/// A segment of tool motion used by the dexel material removal engine.
#[derive(Debug, Clone, PartialEq)]
pub enum MotionSegment {
    Linear {
        start: Vec3,
        end: Vec3,
    },
    Arc {
        start: Vec3,
        end: Vec3,
        center: Vec3,
        clockwise: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(spans: &[(f64, f64)]) -> DexelColumn {
        DexelColumn {
            spans: spans
                .iter()
                .map(|&(z_min, z_max)| ZSpan { z_min, z_max })
                .collect(),
        }
    }

    #[test]
    fn remove_above_truncation() {
        let mut c = col(&[(0.0, 50.0)]);
        c.remove_above(30.0);
        assert_eq!(c, col(&[(0.0, 30.0)]));
    }

    #[test]
    fn remove_above_noop_floor_above_span() {
        let mut c = col(&[(0.0, 50.0)]);
        c.remove_above(60.0);
        assert_eq!(c, col(&[(0.0, 50.0)]));
    }

    #[test]
    fn remove_above_full_removal() {
        let mut c = col(&[(10.0, 50.0)]);
        c.remove_above(5.0);
        assert_eq!(c, col(&[]));
    }

    #[test]
    fn remove_above_exact_boundary_at_z_min() {
        let mut c = col(&[(10.0, 50.0)]);
        c.remove_above(10.0);
        assert_eq!(c, col(&[]));
    }

    #[test]
    fn remove_above_multi_span_column() {
        let mut c = col(&[(0.0, 10.0), (20.0, 30.0), (40.0, 50.0)]);
        c.remove_above(25.0);
        assert_eq!(c, col(&[(0.0, 10.0), (20.0, 25.0)]));
    }

    #[test]
    fn remove_above_empty_column() {
        let mut c = col(&[]);
        c.remove_above(10.0);
        assert_eq!(c, col(&[]));
    }
}
