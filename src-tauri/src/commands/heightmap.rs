//! Heightmap import IPC command for 3D mode.
//!
//! The initial slice accepts a grayscale PNG or TIFF and tessellates it into
//! a `MeshData` plane with per-pixel Z displacement. Physical footprint and
//! Z range are hardcoded for v1; controls will be added in a later slice.

use crate::error::AppError;
use crate::geometry::safe::FaceGroup;
use crate::geometry::MeshData;

/// Physical footprint width (X axis) applied to every heightmap in v1.
pub const DEFAULT_PHYSICAL_WIDTH_MM: f64 = 100.0;
/// Physical footprint depth (Y axis) applied to every heightmap in v1.
pub const DEFAULT_PHYSICAL_DEPTH_MM: f64 = 100.0;
/// Full-scale Z displacement (white pixel → `DEFAULT_Z_RANGE_MM`, black → 0).
pub const DEFAULT_Z_RANGE_MM: f64 = 10.0;

/// Testable inner logic for [`load_heightmap`].
///
/// Reads an image from disk, converts it to 16-bit grayscale, and builds a
/// tessellated plane mesh. Pixel (i, j) maps to world (x, y, z) with:
///
/// - `x = i * width / (W - 1)`
/// - `y = (H - 1 - j) * depth / (H - 1)`  (flip so image top = +Y)
/// - `z = luminance/65535 * z_range`
///
/// Normals are computed by central-difference gradient, falling back to
/// one-sided differences at the image edges.
pub fn load_heightmap_inner(path: &str) -> Result<MeshData, AppError> {
    if !std::path::Path::new(path).exists() {
        return Err(AppError::FileNotFound);
    }

    let img = image::ImageReader::open(path)
        .map_err(|e| AppError::Io(format!("could not open image: {e}")))?
        .with_guessed_format()
        .map_err(|e| AppError::Io(format!("could not read image header: {e}")))?
        .decode()
        .map_err(|e| AppError::GeometryImport(format!("failed to decode image: {e}")))?;

    let gray = img.into_luma16();
    let w = gray.width() as usize;
    let h = gray.height() as usize;

    if w < 2 || h < 2 {
        return Err(AppError::InvalidInput(
            "heightmap must be at least 2×2 pixels".into(),
        ));
    }

    let physical_width = DEFAULT_PHYSICAL_WIDTH_MM;
    let physical_depth = DEFAULT_PHYSICAL_DEPTH_MM;
    let z_range = DEFAULT_Z_RANGE_MM;

    let dx = physical_width / (w as f64 - 1.0);
    let dy = physical_depth / (h as f64 - 1.0);

    // Sample all Z values up-front so the normal pass can read neighbours.
    let mut zs = vec![0.0_f64; w * h];
    for j in 0..h {
        for i in 0..w {
            let lum = gray.get_pixel(i as u32, j as u32).0[0] as f64;
            zs[j * w + i] = (lum / 65535.0) * z_range;
        }
    }

    let vertex_count = w * h;
    let triangle_count = (w - 1) * (h - 1) * 2;
    let mut vertices = Vec::with_capacity(vertex_count * 3);
    let mut normals = Vec::with_capacity(vertex_count * 3);
    let mut indices = Vec::with_capacity(triangle_count * 3);

    for j in 0..h {
        for i in 0..w {
            let x = i as f64 * dx;
            // Flip so image row 0 (top) maps to largest Y.
            let y = (h as f64 - 1.0 - j as f64) * dy;
            let z = zs[j * w + i];
            vertices.extend_from_slice(&[x as f32, y as f32, z as f32]);

            let (zl, zr, step_x) = if i == 0 {
                (zs[j * w + i], zs[j * w + i + 1], dx)
            } else if i == w - 1 {
                (zs[j * w + i - 1], zs[j * w + i], dx)
            } else {
                (zs[j * w + i - 1], zs[j * w + i + 1], 2.0 * dx)
            };
            // Y axis in world space is flipped relative to image j, so swap
            // the "up"/"down" neighbour ordering to keep dz/dy in world units.
            let (zd, zu, step_y) = if j == 0 {
                (zs[j * w + i], zs[(j + 1) * w + i], dy)
            } else if j == h - 1 {
                (zs[(j - 1) * w + i], zs[j * w + i], dy)
            } else {
                (zs[(j + 1) * w + i], zs[(j - 1) * w + i], 2.0 * dy)
            };
            let dz_dx = (zr - zl) / step_x;
            let dz_dy = (zu - zd) / step_y;

            let nx = -dz_dx;
            let ny = -dz_dy;
            let nz = 1.0_f64;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            normals.extend_from_slice(&[
                (nx / len) as f32,
                (ny / len) as f32,
                (nz / len) as f32,
            ]);
        }
    }

    // Triangulate each quad (i, j) — (i+1, j) — (i+1, j+1) — (i, j+1).
    // Note j increases downward in image space but we flipped Y for world
    // coords, so reverse the winding to keep triangles front-facing (+Z).
    for j in 0..(h - 1) {
        for i in 0..(w - 1) {
            let v00 = (j * w + i) as u32;
            let v10 = (j * w + i + 1) as u32;
            let v01 = ((j + 1) * w + i) as u32;
            let v11 = ((j + 1) * w + i + 1) as u32;
            indices.extend_from_slice(&[v00, v11, v10]);
            indices.extend_from_slice(&[v00, v01, v11]);
        }
    }

    Ok(MeshData {
        vertices,
        normals,
        indices,
        face_groups: vec![FaceGroup {
            start_triangle: 0,
            triangle_count: triangle_count as u32,
        }],
    })
}

/// Load a heightmap image (PNG or TIFF grayscale) and return a tessellated
/// plane mesh with per-pixel Z displacement.
///
/// Physical footprint and Z range are hardcoded to 100×100 mm × 10 mm in the
/// initial implementation; parameterised controls will follow.
#[tauri::command]
pub async fn load_heightmap(path: String) -> Result<MeshData, AppError> {
    load_heightmap_inner(&path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Luma};

    fn write_test_png(tmp: &tempfile::TempDir, name: &str, img: ImageBuffer<Luma<u16>, Vec<u16>>) -> String {
        let path = tmp.path().join(name);
        img.save(&path).expect("save test png");
        path.to_str().unwrap().to_string()
    }

    #[test]
    fn returns_file_not_found_for_missing_path() {
        let err = load_heightmap_inner("/nonexistent/heightmap.png").unwrap_err();
        assert!(matches!(err, AppError::FileNotFound));
    }

    #[test]
    fn rejects_image_smaller_than_2x2() {
        let tmp = tempfile::tempdir().unwrap();
        let img: ImageBuffer<Luma<u16>, Vec<u16>> = ImageBuffer::from_pixel(1, 1, Luma([0u16]));
        let path = write_test_png(&tmp, "tiny.png", img);
        let err = load_heightmap_inner(&path).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn flat_black_image_produces_flat_mesh_at_z_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let img: ImageBuffer<Luma<u16>, Vec<u16>> = ImageBuffer::from_pixel(4, 4, Luma([0u16]));
        let path = write_test_png(&tmp, "black.png", img);
        let mesh = load_heightmap_inner(&path).unwrap();

        // 4×4 vertices, (4-1)*(4-1)*2 = 18 triangles.
        assert_eq!(mesh.vertices.len(), 4 * 4 * 3);
        assert_eq!(mesh.indices.len(), 18 * 3);

        // All Z values should be zero.
        for i in 0..16 {
            assert_eq!(mesh.vertices[i * 3 + 2], 0.0, "vertex {i} z should be 0");
        }

        // Normals of a flat surface should point +Z.
        for i in 0..16 {
            let nz = mesh.normals[i * 3 + 2];
            assert!((nz - 1.0).abs() < 1e-5, "vertex {i} normal z should be +1");
        }
    }

    #[test]
    fn white_pixel_maps_to_full_z_range() {
        let tmp = tempfile::tempdir().unwrap();
        let img: ImageBuffer<Luma<u16>, Vec<u16>> = ImageBuffer::from_pixel(2, 2, Luma([u16::MAX]));
        let path = write_test_png(&tmp, "white.png", img);
        let mesh = load_heightmap_inner(&path).unwrap();

        for i in 0..4 {
            let z = mesh.vertices[i * 3 + 2] as f64;
            assert!(
                (z - DEFAULT_Z_RANGE_MM).abs() < 1e-4,
                "vertex {i} z = {z}, expected {DEFAULT_Z_RANGE_MM}",
            );
        }
    }

    #[test]
    fn footprint_matches_default_physical_size() {
        let tmp = tempfile::tempdir().unwrap();
        let img: ImageBuffer<Luma<u16>, Vec<u16>> = ImageBuffer::from_pixel(3, 3, Luma([0u16]));
        let path = write_test_png(&tmp, "grid.png", img);
        let mesh = load_heightmap_inner(&path).unwrap();

        // Walk all vertices, find XY bounds.
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for v in 0..9 {
            let x = mesh.vertices[v * 3];
            let y = mesh.vertices[v * 3 + 1];
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        assert_eq!(min_x, 0.0);
        assert_eq!(min_y, 0.0);
        assert!((max_x as f64 - DEFAULT_PHYSICAL_WIDTH_MM).abs() < 1e-4);
        assert!((max_y as f64 - DEFAULT_PHYSICAL_DEPTH_MM).abs() < 1e-4);
    }

    #[test]
    fn indices_are_all_in_bounds_and_triangles_non_degenerate() {
        let tmp = tempfile::tempdir().unwrap();
        // Gradient image: left column 0, right column white.
        let mut img: ImageBuffer<Luma<u16>, Vec<u16>> = ImageBuffer::new(5, 5);
        for j in 0..5 {
            for i in 0..5 {
                let v = (i as f64 / 4.0 * u16::MAX as f64) as u16;
                img.put_pixel(i, j, Luma([v]));
            }
        }
        let path = write_test_png(&tmp, "gradient.png", img);
        let mesh = load_heightmap_inner(&path).unwrap();

        let vertex_count = mesh.vertices.len() / 3;
        for &idx in &mesh.indices {
            assert!(
                (idx as usize) < vertex_count,
                "index {idx} out of bounds",
            );
        }

        // No zero-area triangles.
        for tri in 0..(mesh.indices.len() / 3) {
            let i0 = mesh.indices[tri * 3] as usize;
            let i1 = mesh.indices[tri * 3 + 1] as usize;
            let i2 = mesh.indices[tri * 3 + 2] as usize;
            let v0 = [
                mesh.vertices[i0 * 3],
                mesh.vertices[i0 * 3 + 1],
                mesh.vertices[i0 * 3 + 2],
            ];
            let v1 = [
                mesh.vertices[i1 * 3],
                mesh.vertices[i1 * 3 + 1],
                mesh.vertices[i1 * 3 + 2],
            ];
            let v2 = [
                mesh.vertices[i2 * 3],
                mesh.vertices[i2 * 3 + 1],
                mesh.vertices[i2 * 3 + 2],
            ];
            let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
            let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
            let cx = e1[1] * e2[2] - e1[2] * e2[1];
            let cy = e1[2] * e2[0] - e1[0] * e2[2];
            let cz = e1[0] * e2[1] - e1[1] * e2[0];
            let area2 = (cx * cx + cy * cy + cz * cz).sqrt();
            assert!(area2 > 1e-6, "triangle {tri} is degenerate");
        }
    }

    #[test]
    fn face_groups_span_all_triangles() {
        let tmp = tempfile::tempdir().unwrap();
        let img: ImageBuffer<Luma<u16>, Vec<u16>> = ImageBuffer::from_pixel(3, 3, Luma([0u16]));
        let path = write_test_png(&tmp, "fg.png", img);
        let mesh = load_heightmap_inner(&path).unwrap();
        assert_eq!(mesh.face_groups.len(), 1);
        assert_eq!(mesh.face_groups[0].start_triangle, 0);
        assert_eq!(mesh.face_groups[0].triangle_count as usize, mesh.indices.len() / 3);
    }
}
