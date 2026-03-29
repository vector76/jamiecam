use crate::dexel::types::{DexelColumn, ZSpan};
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
}
