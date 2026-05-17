use crate::dexel::grid::DexelGrid;
use crate::types::{FaceGroup, MeshData};

/// Append a quad (4 vertices, 2 triangles) to the mesh buffers.
///
/// Vertex winding must produce the desired normal via the right-hand rule:
/// `(v1 - v0) × (v2 - v0)` should point in the `normal` direction.
fn emit_quad(
    verts: &mut Vec<f64>,
    norms: &mut Vec<f64>,
    indices: &mut Vec<u32>,
    positions: [(f64, f64, f64); 4],
    normal: (f64, f64, f64),
) {
    let base = (verts.len() / 3) as u32;
    for (x, y, z) in positions {
        verts.extend_from_slice(&[x, y, z]);
        norms.extend_from_slice(&[normal.0, normal.1, normal.2]);
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

impl DexelGrid {
    /// Return the top Z of the column at grid position `(col, row)`,
    /// or `None` if the column is empty.
    fn cell_top_z(&self, col: usize, row: usize) -> Option<f64> {
        self.columns[row * self.nx + col]
            .spans
            .last()
            .map(|s| s.z_max)
    }

    /// Convert the dexel grid into a triangle mesh for 3D visualization.
    ///
    /// Produces a staircase mesh with top surfaces, vertical walls, and
    /// bottom surfaces. Assumes single-span columns (3-axis top-down cutting).
    pub fn extract_mesh(&self) -> MeshData {
        let mut verts = Vec::<f64>::new();
        let mut norms = Vec::<f64>::new();
        let mut indices = Vec::<u32>::new();

        let res = self.resolution;
        let floor = self.floor_z;

        // ── Top and bottom surfaces ──────────────────────────────────────
        for row in 0..self.ny {
            for col in 0..self.nx {
                let tz = match self.cell_top_z(col, row) {
                    Some(z) => z,
                    None => continue,
                };

                let x0 = self.origin_x + col as f64 * res;
                let y0 = self.origin_y + row as f64 * res;
                let x1 = x0 + res;
                let y1 = y0 + res;

                // Top — normal +Z
                emit_quad(
                    &mut verts,
                    &mut norms,
                    &mut indices,
                    [(x0, y0, tz), (x1, y0, tz), (x1, y1, tz), (x0, y1, tz)],
                    (0.0, 0.0, 1.0),
                );

                // Bottom — normal -Z
                emit_quad(
                    &mut verts,
                    &mut norms,
                    &mut indices,
                    [
                        (x0, y0, floor),
                        (x0, y1, floor),
                        (x1, y1, floor),
                        (x1, y0, floor),
                    ],
                    (0.0, 0.0, -1.0),
                );
            }
        }

        // ── X-aligned vertical walls ─────────────────────────────────────
        // Walk every vertical edge at x = origin_x + col * resolution,
        // col in 0..=nx. Left cell is (col-1, row), right cell is (col, row).
        for col in 0..=self.nx {
            for row in 0..self.ny {
                let left_z = if col > 0 {
                    self.cell_top_z(col - 1, row)
                } else {
                    None
                };
                let right_z = if col < self.nx {
                    self.cell_top_z(col, row)
                } else {
                    None
                };

                if left_z == right_z {
                    continue;
                }

                let x = self.origin_x + col as f64 * res;
                let y0 = self.origin_y + row as f64 * res;
                let y1 = y0 + res;

                match (left_z, right_z) {
                    (Some(lz), Some(rz)) => {
                        let (z_lo, z_hi) = (lz.min(rz), lz.max(rz));
                        if lz > rz {
                            // Left taller → outward normal = +X
                            emit_quad(
                                &mut verts,
                                &mut norms,
                                &mut indices,
                                [(x, y0, z_lo), (x, y1, z_lo), (x, y1, z_hi), (x, y0, z_hi)],
                                (1.0, 0.0, 0.0),
                            );
                        } else {
                            // Right taller → outward normal = -X
                            emit_quad(
                                &mut verts,
                                &mut norms,
                                &mut indices,
                                [(x, y1, z_lo), (x, y0, z_lo), (x, y0, z_hi), (x, y1, z_hi)],
                                (-1.0, 0.0, 0.0),
                            );
                        }
                    }
                    (Some(lz), None) => {
                        // Left has material, right empty → normal +X
                        emit_quad(
                            &mut verts,
                            &mut norms,
                            &mut indices,
                            [(x, y0, floor), (x, y1, floor), (x, y1, lz), (x, y0, lz)],
                            (1.0, 0.0, 0.0),
                        );
                    }
                    (None, Some(rz)) => {
                        // Right has material, left empty → normal -X
                        emit_quad(
                            &mut verts,
                            &mut norms,
                            &mut indices,
                            [(x, y1, floor), (x, y0, floor), (x, y0, rz), (x, y1, rz)],
                            (-1.0, 0.0, 0.0),
                        );
                    }
                    (None, None) => {}
                }
            }
        }

        // ── Y-aligned vertical walls ─────────────────────────────────────
        // Walk every horizontal edge at y = origin_y + row * resolution,
        // row in 0..=ny. Below cell is (col, row-1), above cell is (col, row).
        for row in 0..=self.ny {
            for col in 0..self.nx {
                let below_z = if row > 0 {
                    self.cell_top_z(col, row - 1)
                } else {
                    None
                };
                let above_z = if row < self.ny {
                    self.cell_top_z(col, row)
                } else {
                    None
                };

                if below_z == above_z {
                    continue;
                }

                let y = self.origin_y + row as f64 * res;
                let x0 = self.origin_x + col as f64 * res;
                let x1 = x0 + res;

                match (below_z, above_z) {
                    (Some(bz), Some(az)) => {
                        let (z_lo, z_hi) = (bz.min(az), bz.max(az));
                        if bz > az {
                            // Below taller → outward normal = +Y
                            emit_quad(
                                &mut verts,
                                &mut norms,
                                &mut indices,
                                [(x1, y, z_lo), (x0, y, z_lo), (x0, y, z_hi), (x1, y, z_hi)],
                                (0.0, 1.0, 0.0),
                            );
                        } else {
                            // Above taller → outward normal = -Y
                            emit_quad(
                                &mut verts,
                                &mut norms,
                                &mut indices,
                                [(x0, y, z_lo), (x1, y, z_lo), (x1, y, z_hi), (x0, y, z_hi)],
                                (0.0, -1.0, 0.0),
                            );
                        }
                    }
                    (Some(bz), None) => {
                        // Below has material, above empty → normal +Y
                        emit_quad(
                            &mut verts,
                            &mut norms,
                            &mut indices,
                            [(x1, y, floor), (x0, y, floor), (x0, y, bz), (x1, y, bz)],
                            (0.0, 1.0, 0.0),
                        );
                    }
                    (None, Some(az)) => {
                        // Above has material, below empty → normal -Y
                        emit_quad(
                            &mut verts,
                            &mut norms,
                            &mut indices,
                            [(x0, y, floor), (x1, y, floor), (x1, y, az), (x0, y, az)],
                            (0.0, -1.0, 0.0),
                        );
                    }
                    (None, None) => {}
                }
            }
        }

        // ── Assemble MeshData ────────────────────────────────────────────
        let total_triangles = (indices.len() / 3) as u32;
        MeshData {
            vertices: verts.iter().map(|&v| v as f32).collect(),
            normals: norms.iter().map(|&n| n as f32).collect(),
            indices,
            face_groups: vec![FaceGroup {
                start_triangle: 0,
                triangle_count: total_triangles,
            }],
        }
    }
}

#[cfg(test)]
// Row-major index formulas (`row * stride + col`) are written literally even
// when row is 1; the pattern is self-documenting and preferred over
// collapsing to the reduced form.
#[allow(clippy::identity_op)]
mod tests {
    use super::*;
    use crate::types::{BoxDimensions, StockDefinition, Vec3};

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

    fn triangle_count(mesh: &MeshData) -> u32 {
        (mesh.indices.len() / 3) as u32
    }

    // ── Triangle count tests ─────────────────────────────────────────

    #[test]
    fn single_cell_produces_12_triangles() {
        let grid = DexelGrid::from_stock(&box_stock(1.0, 1.0, 1.0), 1.0);
        let mesh = grid.extract_mesh();
        // 6 faces × 2 triangles each = 12
        assert_eq!(triangle_count(&mesh), 12);
    }

    #[test]
    fn untouched_3x3_produces_expected_triangles() {
        let grid = DexelGrid::from_stock(&box_stock(3.0, 3.0, 5.0), 1.0);
        let mesh = grid.extract_mesh();
        // Top: 9×2=18, Bottom: 9×2=18, Walls: 4 sides × 3 cells × 2 tri = 24
        // Total = 60
        assert_eq!(triangle_count(&mesh), 60);
    }

    #[test]
    fn lowered_interior_cell_adds_wall_quads() {
        let mut grid = DexelGrid::from_stock(&box_stock(3.0, 3.0, 5.0), 1.0);
        // Lower center cell (col=1, row=1) from z=5 to z=3
        let idx = 1 * 3 + 1;
        grid.columns[idx].remove_above(3.0);
        let mesh = grid.extract_mesh();
        // Untouched = 60, plus 4 interior walls × 2 tri = 8
        assert_eq!(triangle_count(&mesh), 68);
    }

    // ── Normal validation ────────────────────────────────────────────

    #[test]
    fn all_normals_are_unit_length() {
        let mut grid = DexelGrid::from_stock(&box_stock(3.0, 3.0, 5.0), 1.0);
        grid.columns[1 * 3 + 1].remove_above(3.0);
        let mesh = grid.extract_mesh();

        for i in (0..mesh.normals.len()).step_by(3) {
            let nx = mesh.normals[i] as f64;
            let ny = mesh.normals[i + 1] as f64;
            let nz = mesh.normals[i + 2] as f64;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            assert!(
                (len - 1.0).abs() < 1e-6,
                "Normal at vertex {} has length {len}",
                i / 3
            );
        }
    }

    // ── Degeneracy and winding check ────────────────────────────────

    #[test]
    fn no_degenerate_triangles_and_normals_agree_with_winding() {
        let mut grid = DexelGrid::from_stock(&box_stock(3.0, 3.0, 5.0), 1.0);
        grid.columns[1 * 3 + 1].remove_above(3.0);
        let mesh = grid.extract_mesh();

        for tri in 0..(mesh.indices.len() / 3) {
            let i0 = mesh.indices[tri * 3] as usize;
            let i1 = mesh.indices[tri * 3 + 1] as usize;
            let i2 = mesh.indices[tri * 3 + 2] as usize;

            let v0 = [
                mesh.vertices[i0 * 3] as f64,
                mesh.vertices[i0 * 3 + 1] as f64,
                mesh.vertices[i0 * 3 + 2] as f64,
            ];
            let v1 = [
                mesh.vertices[i1 * 3] as f64,
                mesh.vertices[i1 * 3 + 1] as f64,
                mesh.vertices[i1 * 3 + 2] as f64,
            ];
            let v2 = [
                mesh.vertices[i2 * 3] as f64,
                mesh.vertices[i2 * 3 + 1] as f64,
                mesh.vertices[i2 * 3 + 2] as f64,
            ];

            let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
            let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];

            let cross = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            let cross_len =
                (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
            assert!(
                cross_len > 1e-12,
                "Triangle {tri} is degenerate (cross product magnitude {cross_len})"
            );

            // The geometric normal from winding should agree with the stored normal.
            let stored = [
                mesh.normals[i0 * 3] as f64,
                mesh.normals[i0 * 3 + 1] as f64,
                mesh.normals[i0 * 3 + 2] as f64,
            ];
            let dot = cross[0] * stored[0] + cross[1] * stored[1] + cross[2] * stored[2];
            assert!(
                dot > 0.0,
                "Triangle {tri}: winding normal disagrees with stored normal \
                 (dot = {dot}, cross = {cross:?}, stored = {stored:?})"
            );
        }
    }

    // ── Index bounds check ───────────────────────────────────────────

    #[test]
    fn all_indices_within_vertex_count() {
        let mut grid = DexelGrid::from_stock(&box_stock(3.0, 3.0, 5.0), 1.0);
        grid.columns[1 * 3 + 1].remove_above(3.0);
        let mesh = grid.extract_mesh();

        let vertex_count = (mesh.vertices.len() / 3) as u32;
        for (i, &idx) in mesh.indices.iter().enumerate() {
            assert!(
                idx < vertex_count,
                "Index {idx} at position {i} >= vertex count {vertex_count}"
            );
        }
    }

    // ── Face group covers all triangles ──────────────────────────────

    #[test]
    fn single_face_group_covers_all() {
        let grid = DexelGrid::from_stock(&box_stock(2.0, 2.0, 3.0), 1.0);
        let mesh = grid.extract_mesh();
        assert_eq!(mesh.face_groups.len(), 1);
        assert_eq!(mesh.face_groups[0].start_triangle, 0);
        assert_eq!(mesh.face_groups[0].triangle_count, triangle_count(&mesh));
    }

    // ── Empty grid produces empty mesh ───────────────────────────────

    #[test]
    fn empty_grid_produces_empty_mesh() {
        let mut grid = DexelGrid::from_stock(&box_stock(2.0, 2.0, 3.0), 1.0);
        // Clear all columns
        for col in &mut grid.columns {
            col.spans.clear();
        }
        let mesh = grid.extract_mesh();
        assert!(mesh.vertices.is_empty());
        assert!(mesh.normals.is_empty());
        assert!(mesh.indices.is_empty());
        assert_eq!(mesh.face_groups[0].triangle_count, 0);
    }
}
