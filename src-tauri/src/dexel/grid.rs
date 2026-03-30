use crate::dexel::types::{DexelColumn, MotionSegment, ZSpan};
use crate::models::stock::Vec3;
use crate::models::StockDefinition;

/// A 2D grid of dexel columns representing a workpiece.
///
/// Columns are stored in row-major order (`nx` columns per row, `ny` rows).
#[derive(Debug, Clone)]
pub struct DexelGrid {
    pub origin_x: f64,
    pub origin_y: f64,
    pub resolution: f64,
    pub nx: usize,
    pub ny: usize,
    pub columns: Vec<DexelColumn>,
    pub floor_z: f64,
}

impl DexelGrid {
    /// Create a dexel grid from a stock definition.
    ///
    /// Each grid cell is filled with a single span covering the full stock height.
    pub fn from_stock(stock: &StockDefinition, resolution: f64) -> Self {
        let StockDefinition::Box(dims) = stock;

        let nx = (dims.width / resolution).ceil() as usize;
        let ny = (dims.depth / resolution).ceil() as usize;
        let floor_z = dims.origin.z;
        let span = ZSpan {
            z_min: floor_z,
            z_max: floor_z + dims.height,
        };
        let column = DexelColumn { spans: vec![span] };
        let columns = vec![column; nx * ny];

        DexelGrid {
            origin_x: dims.origin.x,
            origin_y: dims.origin.y,
            resolution,
            nx,
            ny,
            columns,
            floor_z,
        }
    }

    /// Total material volume across all columns.
    pub fn volume(&self) -> f64 {
        let cell_area = self.resolution * self.resolution;
        self.columns
            .iter()
            .flat_map(|col| &col.spans)
            .map(|span| (span.z_max - span.z_min) * cell_area)
            .sum()
    }

    /// Return the top Z of the highest span at world coordinates `(x, y)`,
    /// or `None` if the point is outside the grid or the cell is empty.
    pub fn height_at(&self, x: f64, y: f64) -> Option<f64> {
        let col_f = (x - self.origin_x) / self.resolution;
        let row_f = (y - self.origin_y) / self.resolution;

        if col_f < 0.0 || row_f < 0.0 {
            return None;
        }

        let col = col_f.floor() as usize;
        let row = row_f.floor() as usize;

        if col >= self.nx || row >= self.ny {
            return None;
        }

        let idx = row * self.nx + col;
        self.columns[idx].spans.last().map(|span| span.z_max)
    }

    /// Maximum top Z across all columns, or `None` if every column is empty.
    pub fn max_height(&self) -> Option<f64> {
        self.columns
            .iter()
            .flat_map(|col| col.spans.last())
            .map(|span| span.z_max)
            .fold(None, |acc, z| match acc {
                None => Some(z),
                Some(prev) => Some(prev.max(z)),
            })
    }

    /// Deep-clone this grid.
    pub fn snapshot(&self) -> DexelGrid {
        self.clone()
    }

    /// Volume removed compared to a previous snapshot: `previous.volume() - self.volume()`.
    pub fn removed_volume_since(&self, previous: &DexelGrid) -> f64 {
        previous.volume() - self.volume()
    }

    /// Remove material for one motion segment using the given tool profile.
    ///
    /// `tool_radius` is the maximum cutting radius of the tool.
    /// `z_clearance(r)` returns the Z offset above the tool tip at radial distance `r`,
    /// or `None` if `r` is beyond the tool's reach.
    pub fn apply_segment(
        &mut self,
        segment: &MotionSegment,
        tool_radius: f64,
        z_clearance: &(impl Fn(f64) -> Option<f64> + Sync),
    ) {
        match segment {
            MotionSegment::Linear { start, end } => {
                self.apply_linear(
                    start.x,
                    start.y,
                    start.z,
                    end.x,
                    end.y,
                    end.z,
                    tool_radius,
                    z_clearance,
                );
            }
            MotionSegment::Arc {
                start,
                end,
                center,
                clockwise,
            } => {
                let dx_s = start.x - center.x;
                let dy_s = start.y - center.y;
                let radius = (dx_s * dx_s + dy_s * dy_s).sqrt();

                let start_angle = dy_s.atan2(dx_s);
                let end_angle = (end.y - center.y).atan2(end.x - center.x);

                // Compute angular sweep respecting direction.
                // Using >= / <= so that sweep == 0 (start == end, i.e. full circle)
                // is promoted to a full ±2π sweep rather than collapsing to nothing.
                let mut sweep = end_angle - start_angle;
                if *clockwise {
                    if sweep >= 0.0 {
                        sweep -= 2.0 * std::f64::consts::PI;
                    }
                } else if sweep <= 0.0 {
                    sweep += 2.0 * std::f64::consts::PI;
                }

                // Number of sub-segments based on chord tolerance.
                let chord_tolerance = self.resolution / 2.0;
                let n = if radius <= chord_tolerance {
                    1
                } else {
                    let step_angle = (1.0 - chord_tolerance / radius).acos();
                    (sweep.abs() / step_angle).ceil().max(1.0) as usize
                };

                for i in 0..n {
                    let t0 = i as f64 / n as f64;
                    let t1 = (i + 1) as f64 / n as f64;

                    let angle0 = start_angle + t0 * sweep;
                    let angle1 = start_angle + t1 * sweep;
                    let z0 = start.z + t0 * (end.z - start.z);
                    let z1 = start.z + t1 * (end.z - start.z);

                    let sub = MotionSegment::Linear {
                        start: Vec3 {
                            x: center.x + radius * angle0.cos(),
                            y: center.y + radius * angle0.sin(),
                            z: z0,
                        },
                        end: Vec3 {
                            x: center.x + radius * angle1.cos(),
                            y: center.y + radius * angle1.sin(),
                            z: z1,
                        },
                    };
                    self.apply_segment(&sub, tool_radius, z_clearance);
                }
            }
        }
    }

    /// Remove material for a sequence of motion segments.
    pub fn apply_segments(
        &mut self,
        segments: &[MotionSegment],
        tool_radius: f64,
        z_clearance: &(impl Fn(f64) -> Option<f64> + Sync),
    ) {
        for segment in segments {
            self.apply_segment(segment, tool_radius, z_clearance);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_linear(
        &mut self,
        ax: f64,
        ay: f64,
        az: f64,
        bx: f64,
        by: f64,
        bz: f64,
        tool_radius: f64,
        z_clearance: &(impl Fn(f64) -> Option<f64> + Sync),
    ) {
        // XY bounding box of swept tool envelope, clamped to grid.
        let x_min = ax.min(bx) - tool_radius;
        let x_max = ax.max(bx) + tool_radius;
        let y_min = ay.min(by) - tool_radius;
        let y_max = ay.max(by) + tool_radius;

        let col_start = ((x_min - self.origin_x) / self.resolution).floor().max(0.0) as usize;
        let col_end = ((x_max - self.origin_x) / self.resolution)
            .ceil()
            .min(self.nx as f64) as usize;
        let row_start = ((y_min - self.origin_y) / self.resolution).floor().max(0.0) as usize;
        let row_end = ((y_max - self.origin_y) / self.resolution)
            .ceil()
            .min(self.ny as f64) as usize;

        if col_start >= col_end || row_start >= row_end {
            return;
        }

        let dx = bx - ax;
        let dy = by - ay;
        let dz = bz - az;
        let seg_len_sq = dx * dx + dy * dy;

        // Threshold: consider the move "vertical/plunge" when XY displacement < resolution * 0.01
        let is_plunge = seg_len_sq < (self.resolution * 0.01) * (self.resolution * 0.01);

        let tr_sq = tool_radius * tool_radius;

        for row in row_start..row_end {
            let cy = self.origin_y + (row as f64 + 0.5) * self.resolution;
            for col in col_start..col_end {
                let cx = self.origin_x + (col as f64 + 0.5) * self.resolution;

                let floor_z = if is_plunge {
                    // Pure plunge: XY is essentially constant. Distance is from cell center
                    // to the start/end XY position. Tool tip goes to the lowest Z.
                    let rx = cx - ax;
                    let ry = cy - ay;
                    let r = (rx * rx + ry * ry).sqrt();
                    match z_clearance(r) {
                        Some(dz_clear) => az.min(bz) + dz_clear,
                        None => continue,
                    }
                } else {
                    // General case: find the reachable t-range via quadratic solve
                    // and minimize floor_z over that range. This is exact for flat
                    // endmills and accurate for curved profiles.
                    let fz = Self::reachable_floor_z(
                        cx,
                        cy,
                        ax,
                        ay,
                        az,
                        dx,
                        dy,
                        dz,
                        seg_len_sq,
                        tr_sq,
                        z_clearance,
                    );
                    if fz == f64::INFINITY {
                        continue; // cell outside tool reach
                    }
                    fz
                };

                let idx = row * self.nx + col;
                self.columns[idx].remove_above(floor_z);
            }
        }
    }

    /// Find the t-range where the cell is within tool reach and minimize floor_z
    /// over that range. Returns `f64::INFINITY` if the cell is unreachable.
    #[allow(clippy::too_many_arguments)]
    fn reachable_floor_z(
        cx: f64,
        cy: f64,
        ax: f64,
        ay: f64,
        az: f64,
        dx: f64,
        dy: f64,
        dz: f64,
        seg_len_sq: f64,
        tr_sq: f64,
        z_clearance: &(impl Fn(f64) -> Option<f64> + Sync),
    ) -> f64 {
        // Distance squared from cell center to segment point at parameter t:
        //   d²(t) = (cx - ax - t*dx)² + (cy - ay - t*dy)²
        //         = seg_len_sq * t² - 2*((cx-ax)*dx + (cy-ay)*dy)*t + ((cx-ax)² + (cy-ay)²)
        // We need d²(t) ≤ tr_sq, i.e., seg_len_sq * t² - 2*dot*t + (c_dist_sq - tr_sq) ≤ 0
        let vx = cx - ax;
        let vy = cy - ay;
        let dot = vx * dx + vy * dy;
        let c_dist_sq = vx * vx + vy * vy;

        let a_coeff = seg_len_sq;
        let b_coeff = -2.0 * dot;
        let c_coeff = c_dist_sq - tr_sq;

        let discriminant = b_coeff * b_coeff - 4.0 * a_coeff * c_coeff;
        if discriminant < 0.0 {
            return f64::INFINITY;
        }

        let sqrt_disc = discriminant.sqrt();
        let t_lo = ((-b_coeff - sqrt_disc) / (2.0 * a_coeff)).clamp(0.0, 1.0);
        let t_hi = ((-b_coeff + sqrt_disc) / (2.0 * a_coeff)).clamp(0.0, 1.0);

        if t_lo >= t_hi {
            // Cell not within reach for any valid t (or only tangent).
            // Check the single point t_lo = t_hi.
            let t = t_lo;
            let px = ax + t * dx;
            let py = ay + t * dy;
            let r = ((cx - px) * (cx - px) + (cy - py) * (cy - py)).sqrt();
            return match z_clearance(r) {
                Some(dz_clear) => az + t * dz + dz_clear,
                None => f64::INFINITY,
            };
        }

        // Evaluate floor_z at endpoints of the reachable range, and at the
        // interior critical point of floor_z(t) = az + t*dz + z_clearance(d(t)).
        //
        // For a flat endmill (z_clearance = 0), the minimum is simply at the
        // endpoint with the lower Z: min(az + t_lo*dz, az + t_hi*dz).
        // For general tools, we also evaluate at the closest-approach t (where d(t)
        // is minimized and thus z_clearance is minimized).

        let mut best = f64::INFINITY;

        // Helper: evaluate floor_z at a given t
        let eval = |t: f64| -> f64 {
            let px = ax + t * dx;
            let py = ay + t * dy;
            let r = ((cx - px) * (cx - px) + (cy - py) * (cy - py)).sqrt();
            match z_clearance(r) {
                Some(dz_clear) => az + t * dz + dz_clear,
                None => f64::INFINITY,
            }
        };

        // Endpoints of reachable range
        best = best.min(eval(t_lo));
        best = best.min(eval(t_hi));

        // Interior critical point: closest approach in XY (where d(t) is minimized)
        let t_closest = (dot / seg_len_sq).clamp(t_lo, t_hi);
        best = best.min(eval(t_closest));

        // For dz < 0 (downward move), the minimum floor_z tends to be at t_hi.
        // For dz > 0 (upward move), it tends to be at t_lo. Both are already covered
        // by the endpoint evaluations above.

        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::stock::{BoxDimensions, Vec3};

    fn box_stock(width: f64, depth: f64, height: f64) -> StockDefinition {
        StockDefinition::Box(BoxDimensions {
            origin: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            width,
            depth,
            height,
        })
    }

    #[test]
    fn from_stock_grid_dimensions() {
        let stock = box_stock(10.0, 20.0, 5.0);
        let grid = DexelGrid::from_stock(&stock, 1.0);
        assert_eq!(grid.nx, 10);
        assert_eq!(grid.ny, 20);
    }

    #[test]
    fn initial_volume_matches_stock() {
        let stock = box_stock(10.0, 20.0, 5.0);
        let grid = DexelGrid::from_stock(&stock, 1.0);
        let expected = 10.0 * 20.0 * 5.0;
        assert!((grid.volume() - expected).abs() < 1e-9);
    }

    #[test]
    fn height_at_inside_bounds() {
        let stock = box_stock(10.0, 20.0, 5.0);
        let grid = DexelGrid::from_stock(&stock, 1.0);
        assert_eq!(grid.height_at(3.5, 7.2), Some(5.0));
    }

    #[test]
    fn height_at_outside_bounds() {
        let stock = box_stock(10.0, 20.0, 5.0);
        let grid = DexelGrid::from_stock(&stock, 1.0);
        assert_eq!(grid.height_at(-1.0, 5.0), None);
        assert_eq!(grid.height_at(5.0, -1.0), None);
        assert_eq!(grid.height_at(10.0, 5.0), None);
        assert_eq!(grid.height_at(5.0, 20.0), None);
    }

    #[test]
    fn max_height_on_fresh_grid() {
        let stock = box_stock(10.0, 20.0, 5.0);
        let grid = DexelGrid::from_stock(&stock, 1.0);
        assert_eq!(grid.max_height(), Some(5.0));
    }

    #[test]
    fn snapshot_is_independent_copy() {
        let stock = box_stock(10.0, 20.0, 5.0);
        let mut grid = DexelGrid::from_stock(&stock, 1.0);
        let snap = grid.snapshot();

        // Mutate the original grid.
        grid.columns[0].remove_above(2.0);

        // Snapshot should be unchanged.
        assert_eq!(snap.columns[0].spans.len(), 1);
        assert!((snap.columns[0].spans[0].z_max - 5.0).abs() < 1e-9);
        // Original should be modified.
        assert!((grid.columns[0].spans[0].z_max - 2.0).abs() < 1e-9);
    }

    #[test]
    fn removed_volume_since_self_is_zero() {
        let stock = box_stock(10.0, 20.0, 5.0);
        let grid = DexelGrid::from_stock(&stock, 1.0);
        let snap = grid.snapshot();
        assert!((grid.removed_volume_since(&snap)).abs() < 1e-9);
    }

    // --- Material removal tests ---

    /// Flat endmill z_clearance: returns Some(0.0) for r <= radius, None otherwise.
    fn flat_endmill(radius: f64) -> impl Fn(f64) -> Option<f64> + Sync {
        move |r: f64| {
            if r <= radius {
                Some(0.0)
            } else {
                None
            }
        }
    }

    #[test]
    fn horizontal_slot() {
        // 10x10x5 stock, resolution=1.0, flat endmill radius=1.0
        // Linear move at Z=4.0 from (1,5,4) to (9,5,4) — horizontal slot.
        let stock = box_stock(10.0, 10.0, 5.0);
        let mut grid = DexelGrid::from_stock(&stock, 1.0);
        let segment = MotionSegment::Linear {
            start: Vec3 {
                x: 1.0,
                y: 5.0,
                z: 4.0,
            },
            end: Vec3 {
                x: 9.0,
                y: 5.0,
                z: 4.0,
            },
        };
        let tool = flat_endmill(1.0);
        grid.apply_segment(&segment, 1.0, &tool);

        // Cells whose center is within radius 1.0 of the corridor (Y=5.0, X in [1,9])
        // should have top Z=4.0.
        // Cell centers along Y=5: row=5 (center y=5.5), that's 0.5 from Y=5.0 → within radius.
        // Also row=4 (center y=4.5), 0.5 from Y=5.0 → within radius.
        // row=3 (center y=3.5), 1.5 from Y=5.0 → outside.
        for col in 1..9 {
            let cx = 0.0 + (col as f64 + 0.5) * 1.0;
            // Row 4 (cy=4.5) and Row 5 (cy=5.5) should be cut
            for row in [4, 5] {
                let idx = row * 10 + col;
                let z_max = grid.columns[idx].spans.last().map(|s| s.z_max);
                assert!(
                    (z_max.unwrap() - 4.0).abs() < 1e-9,
                    "Cell ({col},{row}) at cx={cx} should have z_max=4.0, got {z_max:?}"
                );
            }
        }

        // Cells outside the corridor should still be at 5.0.
        // Row 0 (cy=0.5) — well outside.
        for col in 0..10 {
            let idx = 0 * 10 + col;
            let z_max = grid.columns[idx].spans.last().map(|s| s.z_max);
            assert!(
                (z_max.unwrap() - 5.0).abs() < 1e-9,
                "Cell ({col},0) should be unchanged at z_max=5.0, got {z_max:?}"
            );
        }
    }

    #[test]
    fn plunge_cut() {
        // 10x10x5 stock, resolution=1.0, flat endmill radius=1.0
        // Vertical move from (5,5,5) to (5,5,2) — plunge cut.
        let stock = box_stock(10.0, 10.0, 5.0);
        let mut grid = DexelGrid::from_stock(&stock, 1.0);
        let segment = MotionSegment::Linear {
            start: Vec3 {
                x: 5.0,
                y: 5.0,
                z: 5.0,
            },
            end: Vec3 {
                x: 5.0,
                y: 5.0,
                z: 2.0,
            },
        };
        let tool = flat_endmill(1.0);
        grid.apply_segment(&segment, 1.0, &tool);

        // Cells within radius 1.0 of (5.0, 5.0) should have top Z=2.0.
        // Cell (4,4) center=(4.5, 4.5), distance=sqrt(0.25+0.25)=0.707 → within radius.
        // Cell (5,5) center=(5.5, 5.5), distance=0.707 → within radius.
        // Cell (4,5) center=(4.5, 5.5), distance=0.707 → within radius.
        // Cell (5,4) center=(5.5, 4.5), distance=0.707 → within radius.
        for (col, row) in [(4, 4), (5, 5), (4, 5), (5, 4)] {
            let idx = row * 10 + col;
            let z_max = grid.columns[idx].spans.last().map(|s| s.z_max);
            assert!(
                (z_max.unwrap() - 2.0).abs() < 1e-9,
                "Cell ({col},{row}) should have z_max=2.0 after plunge, got {z_max:?}"
            );
        }

        // Cell (3,3) center=(3.5, 3.5), distance=sqrt(2.25+2.25)=2.12 → outside.
        let idx = 3 * 10 + 3;
        let z_max = grid.columns[idx].spans.last().map(|s| s.z_max);
        assert!(
            (z_max.unwrap() - 5.0).abs() < 1e-9,
            "Cell (3,3) should be unchanged at z_max=5.0, got {z_max:?}"
        );
    }

    #[test]
    fn sloped_move() {
        // 10x10x10 stock, resolution=1.0, flat endmill radius=0.5
        // Small radius so only cells directly on the path are affected.
        // Diagonal move from (0.5, 5.5, 8) to (9.5, 5.5, 3) — sloped in XZ.
        // dx=9, dz=-5. For a cell on the centerline at cx, the tool covers it
        // from t_lo to t_hi. With a flat endmill (z_clearance=0) and dz<0, the
        // minimum floor_z is at t_hi = min((cx - 0.5 + 0.5) / 9, 1.0) = min(cx/9, 1.0).
        // floor_z = 8 + t_hi * (-5).
        let stock = box_stock(10.0, 10.0, 10.0);
        let mut grid = DexelGrid::from_stock(&stock, 1.0);
        let segment = MotionSegment::Linear {
            start: Vec3 {
                x: 0.5,
                y: 5.5,
                z: 8.0,
            },
            end: Vec3 {
                x: 9.5,
                y: 5.5,
                z: 3.0,
            },
        };
        let tool = flat_endmill(0.5);
        grid.apply_segment(&segment, 0.5, &tool);

        // Check a few cells along the path at row=5 (cy=5.5).
        let check = |col: usize, expected_z: f64| {
            let idx = 5 * 10 + col;
            let z_max = grid.columns[idx].spans.last().map(|s| s.z_max).unwrap();
            assert!(
                (z_max - expected_z).abs() < 0.1,
                "Cell ({col},5) expected z_max≈{expected_z:.2}, got {z_max:.2}"
            );
        };

        // col=0 (cx=0.5): t_hi = 0.5/9 ≈ 0.0556, floor_z = 8 - 5*0.5/9 ≈ 7.72
        check(0, 8.0 - 5.0 * 0.5 / 9.0);
        // col=9 (cx=9.5): t_hi = min(9.5/9, 1) = 1.0, floor_z = 3.0
        check(9, 3.0);
        // col=4 (cx=4.5): t_hi = 4.5/9 = 0.5, floor_z = 8 - 2.5 = 5.5
        check(4, 5.5);

        // Verify Z decreases along the path
        let mut prev_z = f64::INFINITY;
        for col in 0..10 {
            let idx = 5 * 10 + col;
            let z_max = grid.columns[idx].spans.last().map(|s| s.z_max).unwrap();
            assert!(z_max <= prev_z + 1e-9, "Z should decrease along path");
            prev_z = z_max;
        }
    }

    #[test]
    fn tool_outside_stock() {
        // Move entirely outside stock bounds — no cells should be modified.
        let stock = box_stock(10.0, 10.0, 5.0);
        let mut grid = DexelGrid::from_stock(&stock, 1.0);
        let snap = grid.snapshot();
        let segment = MotionSegment::Linear {
            start: Vec3 {
                x: 20.0,
                y: 20.0,
                z: 3.0,
            },
            end: Vec3 {
                x: 30.0,
                y: 20.0,
                z: 3.0,
            },
        };
        let tool = flat_endmill(1.0);
        grid.apply_segment(&segment, 1.0, &tool);

        assert!((grid.removed_volume_since(&snap)).abs() < 1e-9);
    }

    // --- Arc segment tests ---

    #[test]
    fn full_circle_arc_removes_annular_region() {
        // 40x40x10 stock, resolution=0.5, flat endmill radius=1.0.
        // Full circle arc centered at (20,20) with arc radius 8.
        // The tool sweeps an annular ring from r=7 to r=9 from center (20,20).
        let stock = box_stock(40.0, 40.0, 10.0);
        let mut grid = DexelGrid::from_stock(&stock, 0.5);
        let snap = grid.snapshot();

        let arc_radius = 8.0;
        let cx = 20.0;
        let cy = 20.0;
        // Start and end at the same point (full circle), CCW.
        let segment = MotionSegment::Arc {
            start: Vec3 {
                x: cx + arc_radius,
                y: cy,
                z: 5.0,
            },
            end: Vec3 {
                x: cx + arc_radius,
                y: cy,
                z: 5.0,
            },
            center: Vec3 {
                x: cx,
                y: cy,
                z: 0.0,
            },
            clockwise: false,
        };
        let tool_radius = 1.0;
        let tool = flat_endmill(tool_radius);
        grid.apply_segment(&segment, tool_radius, &tool);

        // Verify material was removed (volume decreased).
        let removed = grid.removed_volume_since(&snap);
        assert!(
            removed > 0.0,
            "Full circle arc should remove material, got removed={removed}"
        );

        // Cells well inside the annular region (distance from center ≈ arc_radius)
        // should be cut to z=5.0.
        // Check a point at (cx + arc_radius, cy) — on the arc path.
        let z = grid.height_at(cx + arc_radius, cy);
        assert!(
            z.is_some() && (z.unwrap() - 5.0).abs() < 0.5,
            "Cell on arc path should be cut to ~5.0, got {z:?}"
        );

        // Check a point 90° around the arc (top of circle) — should also be cut.
        let z_top = grid.height_at(cx, cy + arc_radius);
        assert!(
            z_top.is_some() && (z_top.unwrap() - 5.0).abs() < 0.5,
            "Cell at 90° on arc should be cut to ~5.0, got {z_top:?}"
        );

        // Check a point 180° around the arc (left side) — should also be cut.
        let z_left = grid.height_at(cx - arc_radius, cy);
        assert!(
            z_left.is_some() && (z_left.unwrap() - 5.0).abs() < 0.5,
            "Cell at 180° on arc should be cut to ~5.0, got {z_left:?}"
        );

        // Check a point far from the arc (center of stock) — should be unchanged.
        let z_center = grid.height_at(cx, cy);
        assert!(
            z_center.is_some() && (z_center.unwrap() - 10.0).abs() < 1e-9,
            "Cell at center should be untouched at 10.0, got {z_center:?}"
        );
    }

    #[test]
    fn full_circle_arc_clockwise() {
        // Same setup as the CCW test but with clockwise=true.
        // Verifies the CW sweep path produces the same annular cut.
        let stock = box_stock(40.0, 40.0, 10.0);
        let mut grid = DexelGrid::from_stock(&stock, 0.5);

        let arc_radius = 8.0;
        let cx = 20.0;
        let cy = 20.0;
        let segment = MotionSegment::Arc {
            start: Vec3 {
                x: cx + arc_radius,
                y: cy,
                z: 5.0,
            },
            end: Vec3 {
                x: cx + arc_radius,
                y: cy,
                z: 5.0,
            },
            center: Vec3 {
                x: cx,
                y: cy,
                z: 0.0,
            },
            clockwise: true,
        };
        let tool_radius = 1.0;
        let tool = flat_endmill(tool_radius);
        grid.apply_segment(&segment, tool_radius, &tool);

        // Check all four quadrants of the circle are cut.
        for (px, py) in [
            (cx + arc_radius, cy),
            (cx, cy + arc_radius),
            (cx - arc_radius, cy),
            (cx, cy - arc_radius),
        ] {
            let z = grid.height_at(px, py);
            assert!(
                z.is_some() && (z.unwrap() - 5.0).abs() < 0.5,
                "CW full circle: point ({px},{py}) should be cut to ~5.0, got {z:?}"
            );
        }

        // Center should be untouched.
        let z_center = grid.height_at(cx, cy).unwrap();
        assert!(
            (z_center - 10.0).abs() < 1e-9,
            "Center should be untouched at 10.0, got {z_center}"
        );
    }

    // --- Ball nose tests ---

    fn ball_nose(radius: f64) -> impl Fn(f64) -> Option<f64> + Sync {
        move |r: f64| {
            if r <= radius {
                Some(radius - (radius * radius - r * r).sqrt())
            } else {
                None
            }
        }
    }

    #[test]
    fn ball_nose_slot_profile() {
        // 20x20x10 stock, resolution=0.25, ball nose radius=2.0.
        // Linear move along Y=10.0 from X=2 to X=18 at Z=5.0.
        // At the centerline (r=0), z_clearance=0, so floor_z = 5.0.
        // At r=1.0, z_clearance = R - sqrt(R^2 - 1) = 2 - sqrt(3) ≈ 0.268.
        let stock = box_stock(20.0, 20.0, 10.0);
        let mut grid = DexelGrid::from_stock(&stock, 0.25);
        let tool_radius = 2.0;
        let segment = MotionSegment::Linear {
            start: Vec3 {
                x: 2.0,
                y: 10.0,
                z: 5.0,
            },
            end: Vec3 {
                x: 18.0,
                y: 10.0,
                z: 5.0,
            },
        };
        let tool = ball_nose(tool_radius);
        grid.apply_segment(&segment, tool_radius, &tool);

        // Check centerline: cell at (10.0, 10.0), r=0 → floor_z = 5.0
        let z_center = grid.height_at(10.0, 10.0).unwrap();
        assert!(
            (z_center - 5.0).abs() < 0.1,
            "Centerline should be at 5.0, got {z_center}"
        );

        // Check at radial distance 1.0 from centerline: (10.0, 11.0).
        // Expected floor_z = 5.0 + (2.0 - sqrt(4.0 - 1.0)) = 5.0 + 0.268 = 5.268
        let expected_at_r1 = 5.0 + (tool_radius - (tool_radius * tool_radius - 1.0).sqrt());
        let z_at_r1 = grid.height_at(10.0, 11.0).unwrap();
        assert!(
            (z_at_r1 - expected_at_r1).abs() < 0.2,
            "At r=1.0 expected ~{expected_at_r1:.3}, got {z_at_r1:.3}"
        );

        // Outside the tool radius (r=2.5 from centerline): should be unchanged at 10.0.
        let z_outside = grid.height_at(10.0, 12.5).unwrap();
        assert!(
            (z_outside - 10.0).abs() < 1e-9,
            "Outside tool reach should be 10.0, got {z_outside}"
        );
    }

    // --- Resolution convergence tests ---

    #[test]
    fn resolution_convergence_toward_analytical_volume() {
        // Verify that the dexel simulation converges as resolution improves:
        // the difference in removed volume between successive resolutions
        // shrinks, and the finest resolution is close to the analytical value.
        //
        // Setup: flat endmill (radius=5) raster-cutting a pocket on a large
        // stock (100×100×10). Raster passes at Y=25,30,...,75 along X=[25,75].
        // Tool reach extends ±5 beyond path, giving a 60×60 cut area at Z=5
        // (depth=5). All well within stock bounds to avoid boundary effects.
        let resolutions = [2.0, 1.0, 0.5];
        let tool_radius = 5.0;
        let z_cut = 5.0;
        let stock_height = 10.0;
        let clearance = flat_endmill(tool_radius);

        let mut volumes: Vec<f64> = Vec::new();

        for &res in &resolutions {
            let stock = box_stock(100.0, 100.0, stock_height);
            let mut grid = DexelGrid::from_stock(&stock, res);
            let initial_volume = grid.volume();

            let mut y = 25.0;
            let mut forward = true;
            while y <= 75.0 {
                let (xs, xe) = if forward { (25.0, 75.0) } else { (75.0, 25.0) };
                let seg = MotionSegment::Linear {
                    start: Vec3 { x: xs, y, z: z_cut },
                    end: Vec3 { x: xe, y, z: z_cut },
                };
                grid.apply_segment(&seg, tool_radius, &clearance);
                y += tool_radius; // step = radius for full coverage
                forward = !forward;
            }

            let removed = initial_volume - grid.volume();
            volumes.push(removed);
        }

        // Differences between successive resolutions should shrink
        // (volumes converge).
        let diff_01 = (volumes[0] - volumes[1]).abs();
        let diff_12 = (volumes[1] - volumes[2]).abs();
        assert!(
            diff_12 < diff_01 + 1e-6,
            "convergence: |V1-V2|={diff_12:.2} should be < |V0-V1|={diff_01:.2}"
        );

        // The finest resolution should be close to the analytical value.
        // Cut region: X=[20,80], Y=[20,80] = 60×60 area, depth 5 → 18000 mm³.
        let analytical_removed = 60.0 * 60.0 * (stock_height - z_cut);
        let finest_error_pct =
            (volumes.last().unwrap() - analytical_removed).abs() / analytical_removed * 100.0;
        assert!(
            finest_error_pct < 5.0,
            "finest resolution error {finest_error_pct:.1}% should be < 5% (removed={:.1}, analytical={analytical_removed:.1})",
            volumes.last().unwrap()
        );
    }

    // --- Volume accounting tests ---

    #[test]
    fn rectangular_pocket_volume() {
        // Cut a rectangular pocket using multiple passes of a flat endmill.
        // Stock: 20x20x10 at origin, resolution=0.5.
        // Pocket: X in [5, 15], Y in [5, 15], depth of cut = 3.0 (floor at Z=7.0).
        // Tool: flat endmill radius=1.0.
        //
        // Raster passes along X at Y = 5.0, 6.0, ..., 15.0 (spaced < 2*R).
        let stock = box_stock(20.0, 20.0, 10.0);
        let mut grid = DexelGrid::from_stock(&stock, 0.5);
        let snap = grid.snapshot();

        let tool_radius = 1.0;
        let tool = flat_endmill(tool_radius);
        let z_cut = 7.0;

        // Raster at 1.0 spacing (= tool_radius) to get good coverage
        let mut y = 5.0;
        while y <= 15.0 {
            let seg = MotionSegment::Linear {
                start: Vec3 {
                    x: 5.0,
                    y,
                    z: z_cut,
                },
                end: Vec3 {
                    x: 15.0,
                    y,
                    z: z_cut,
                },
            };
            grid.apply_segment(&seg, tool_radius, &tool);
            y += 1.0;
        }

        let removed = grid.removed_volume_since(&snap);
        // The tool envelope extends ~1.0 beyond 5..15 in both X and Y,
        // so actual cut area is roughly 12 x 12, depth 3.0.
        // But the test only checks that it's within a reasonable range of
        // the pocket's nominal volume.
        let nominal = 10.0 * 10.0 * 3.0; // 300.0
                                         // With tool overhang the actual removed volume will be somewhat larger
                                         // than nominal. Check within 50% tolerance (the key thing is order-of-magnitude).
        assert!(
            removed > nominal * 0.8,
            "Removed volume {removed:.1} should be at least 80% of nominal {nominal:.1}"
        );
        assert!(
            removed < nominal * 2.0,
            "Removed volume {removed:.1} should be less than 2x nominal {nominal:.1}"
        );
    }
}
