use serde::{Deserialize, Serialize};
use usvg::tiny_skia_path::{PathSegment, Point};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitSystem {
    Mm,
    Inches,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Curve2d {
    pub id: Uuid,
    pub is_closed: bool,
    /// Points in mm, Y-up coordinate system
    pub points: Vec<[f64; 2]>,
    pub layer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundingBox2d {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl BoundingBox2d {
    pub fn from_points(points: &[[f64; 2]]) -> Self {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for &[x, y] in points {
            if x < min_x {
                min_x = x;
            }
            if y < min_y {
                min_y = y;
            }
            if x > max_x {
                max_x = x;
            }
            if y > max_y {
                max_y = y;
            }
        }

        BoundingBox2d {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurveSummary {
    pub id: Uuid,
    pub is_closed: bool,
    pub bbox: BoundingBox2d,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedArtwork {
    pub file_path: String,
    pub unit_system: UnitSystem,
    pub curves: Vec<Curve2d>,
    pub import_warnings: Vec<String>,
}

/// Parse DXF bytes into a list of 2-D curves (mm) and the detected unit system.
///
/// * `$INSUNITS` header variable drives the unit→mm scale factor.
/// * Supported entity types: LINE, LWPOLYLINE, POLYLINE, CIRCLE, ARC.
/// * Entities in paper space (`is_in_paper_space = true`) are skipped.
/// * Curves with fewer than 2 points are silently dropped.
pub fn parse_dxf(bytes: &[u8]) -> Result<(Vec<Curve2d>, UnitSystem), String> {
    use dxf::entities::EntityType;
    use dxf::enums::Units;
    use std::io::Cursor;

    let mut cursor = Cursor::new(bytes);
    let drawing = dxf::Drawing::load(&mut cursor).map_err(|e| e.to_string())?;

    // Map $INSUNITS to (UnitSystem, mm_scale_factor).
    let (unit_system, scale) = match drawing.header.default_drawing_units {
        Units::Inches => (UnitSystem::Inches, 25.4),
        Units::Millimeters => (UnitSystem::Mm, 1.0),
        Units::Unitless => (UnitSystem::Mm, 1.0),
        other => {
            tracing::warn!(
                "Unsupported DXF $INSUNITS value {:?}, defaulting to mm",
                other
            );
            (UnitSystem::Mm, 1.0)
        }
    };

    let mut curves = Vec::new();

    for entity in drawing.entities() {
        if entity.common.is_in_paper_space {
            continue;
        }
        let layer = Some(entity.common.layer.clone());

        match &entity.specific {
            EntityType::Line(l) => {
                let points = vec![
                    [l.p1.x * scale, l.p1.y * scale],
                    [l.p2.x * scale, l.p2.y * scale],
                ];
                curves.push(Curve2d {
                    id: Uuid::new_v4(),
                    is_closed: false,
                    points,
                    layer,
                });
            }
            EntityType::LwPolyline(lw) => {
                let points: Vec<[f64; 2]> = lw
                    .vertices
                    .iter()
                    .map(|v| [v.x * scale, v.y * scale])
                    .collect();
                if points.len() >= 2 {
                    curves.push(Curve2d {
                        id: Uuid::new_v4(),
                        is_closed: lw.is_closed(),
                        points,
                        layer,
                    });
                }
            }
            EntityType::Polyline(p) => {
                let points: Vec<[f64; 2]> = p
                    .vertices()
                    .map(|v| [v.location.x * scale, v.location.y * scale])
                    .collect();
                if points.len() >= 2 {
                    curves.push(Curve2d {
                        id: Uuid::new_v4(),
                        is_closed: p.is_closed(),
                        points,
                        layer,
                    });
                }
            }
            EntityType::Circle(c) => {
                const N: usize = 64;
                let points: Vec<[f64; 2]> = (0..N)
                    .map(|i| {
                        let angle = 2.0 * std::f64::consts::PI * (i as f64) / (N as f64);
                        [
                            (c.center.x + c.radius * angle.cos()) * scale,
                            (c.center.y + c.radius * angle.sin()) * scale,
                        ]
                    })
                    .collect();
                curves.push(Curve2d {
                    id: Uuid::new_v4(),
                    is_closed: true,
                    points,
                    layer,
                });
            }
            EntityType::Arc(a) => {
                let r_mm = a.radius * scale;
                let start_rad = a.start_angle.to_radians();
                let mut end_rad = a.end_angle.to_radians();
                if end_rad <= start_rad {
                    end_rad += 2.0 * std::f64::consts::PI;
                }
                let span = end_rad - start_rad;

                const CHORD_TOL_MM: f64 = 0.05;
                let n_segs = if r_mm > CHORD_TOL_MM {
                    let cos_arg = (1.0 - CHORD_TOL_MM / r_mm).max(-1.0);
                    let max_angle = 2.0 * cos_arg.acos();
                    ((span / max_angle).ceil() as usize).max(1)
                } else {
                    1
                };

                let points: Vec<[f64; 2]> = (0..=n_segs)
                    .map(|i| {
                        let angle = start_rad + span * (i as f64) / (n_segs as f64);
                        [
                            (a.center.x + a.radius * angle.cos()) * scale,
                            (a.center.y + a.radius * angle.sin()) * scale,
                        ]
                    })
                    .collect();
                if points.len() >= 2 {
                    curves.push(Curve2d {
                        id: Uuid::new_v4(),
                        is_closed: false,
                        points,
                        layer,
                    });
                }
            }
            _ => {}
        }
    }

    Ok((curves, unit_system))
}

/// SVG user-unit (px at 96 dpi) to millimetres.
const SVG_TO_MM: f64 = 25.4 / 96.0;

/// Squared chord-flatness tolerance in SVG px² (equivalent to 0.05 mm in mm space).
const CHORD_TOL_SQ_PX: f64 = (0.05 / SVG_TO_MM) * (0.05 / SVG_TO_MM);

/// Parse SVG bytes into a list of 2-D curves in the CAM coordinate system (mm, Y-up).
///
/// * Bezier curves are linearised with a chord tolerance of 0.05 mm.
/// * Paths with fewer than 2 points after linearisation are silently skipped.
/// * `unit_system` is accepted for API symmetry but does not change the output;
///   SVG coordinates are always scaled from px (96 dpi) to mm.
pub fn parse_svg(bytes: &[u8], _unit_system: UnitSystem) -> Result<Vec<Curve2d>, String> {
    let tree =
        usvg::Tree::from_data(bytes, &usvg::Options::default()).map_err(|e| e.to_string())?;
    let mut curves = Vec::new();
    collect_paths(tree.root(), &mut curves);
    Ok(curves)
}

fn collect_paths(group: &usvg::Group, curves: &mut Vec<Curve2d>) {
    for child in group.children() {
        match child {
            usvg::Node::Group(g) => collect_paths(g, curves),
            usvg::Node::Path(path) if path.is_visible() => {
                if let Some(curve) = path_to_curve(path) {
                    curves.push(curve);
                }
            }
            _ => {}
        }
    }
}

fn path_to_curve(path: &usvg::Path) -> Option<Curve2d> {
    let ts = path.abs_transform();
    let mut px_pts: Vec<[f64; 2]> = Vec::new();
    let mut is_closed = false;
    let mut cur = Point::from_xy(0.0, 0.0);
    let mut move_to = Point::from_xy(0.0, 0.0);

    for seg in path.data().segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                move_to = p;
                cur = p;
                let mut tp = p;
                ts.map_point(&mut tp);
                px_pts.push([tp.x as f64, tp.y as f64]);
            }
            PathSegment::LineTo(p) => {
                cur = p;
                let mut tp = p;
                ts.map_point(&mut tp);
                px_pts.push([tp.x as f64, tp.y as f64]);
            }
            PathSegment::QuadTo(p1, p2) => {
                let mut p0t = cur;
                let mut p1t = p1;
                let mut p2t = p2;
                ts.map_point(&mut p0t);
                ts.map_point(&mut p1t);
                ts.map_point(&mut p2t);
                linearize_quad(p0t, p1t, p2t, CHORD_TOL_SQ_PX, &mut px_pts, 0);
                cur = p2;
            }
            PathSegment::CubicTo(p1, p2, p3) => {
                let mut p0t = cur;
                let mut p1t = p1;
                let mut p2t = p2;
                let mut p3t = p3;
                ts.map_point(&mut p0t);
                ts.map_point(&mut p1t);
                ts.map_point(&mut p2t);
                ts.map_point(&mut p3t);
                linearize_cubic(p0t, p1t, p2t, p3t, CHORD_TOL_SQ_PX, &mut px_pts, 0);
                cur = p3;
            }
            PathSegment::Close => {
                is_closed = true;
                cur = move_to;
            }
        }
    }

    let mm_pts: Vec<[f64; 2]> = px_pts
        .iter()
        .map(|&[x, y]| [x * SVG_TO_MM, -y * SVG_TO_MM])
        .collect();

    if mm_pts.len() < 2 {
        return None;
    }

    Some(Curve2d {
        id: Uuid::new_v4(),
        is_closed,
        points: mm_pts,
        layer: None,
    })
}

fn midpoint(a: Point, b: Point) -> Point {
    Point::from_xy((a.x + b.x) * 0.5, (a.y + b.y) * 0.5)
}

/// Squared perpendicular distance from `p` to the chord [a, b].
fn chord_dist_sq(p: Point, a: Point, b: Point) -> f64 {
    let dx = (b.x - a.x) as f64;
    let dy = (b.y - a.y) as f64;
    let len_sq = dx * dx + dy * dy;
    if len_sq < f64::EPSILON {
        let ex = (p.x - a.x) as f64;
        let ey = (p.y - a.y) as f64;
        return ex * ex + ey * ey;
    }
    let cross = (p.x - a.x) as f64 * dy - (p.y - a.y) as f64 * dx;
    cross * cross / len_sq
}

fn linearize_cubic(
    p0: Point,
    p1: Point,
    p2: Point,
    p3: Point,
    tol_sq: f64,
    out: &mut Vec<[f64; 2]>,
    depth: u32,
) {
    let flat = chord_dist_sq(p1, p0, p3) <= tol_sq && chord_dist_sq(p2, p0, p3) <= tol_sq;
    if flat || depth >= 20 {
        out.push([p3.x as f64, p3.y as f64]);
        return;
    }
    let m01 = midpoint(p0, p1);
    let m12 = midpoint(p1, p2);
    let m23 = midpoint(p2, p3);
    let m012 = midpoint(m01, m12);
    let m123 = midpoint(m12, m23);
    let m0123 = midpoint(m012, m123);
    linearize_cubic(p0, m01, m012, m0123, tol_sq, out, depth + 1);
    linearize_cubic(m0123, m123, m23, p3, tol_sq, out, depth + 1);
}

fn linearize_quad(
    p0: Point,
    p1: Point,
    p2: Point,
    tol_sq: f64,
    out: &mut Vec<[f64; 2]>,
    depth: u32,
) {
    if chord_dist_sq(p1, p0, p2) <= tol_sq || depth >= 20 {
        out.push([p2.x as f64, p2.y as f64]);
        return;
    }
    let m01 = midpoint(p0, p1);
    let m12 = midpoint(p1, p2);
    let m012 = midpoint(m01, m12);
    linearize_quad(p0, m01, m012, tol_sq, out, depth + 1);
    linearize_quad(m012, m12, p2, tol_sq, out, depth + 1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounding_box_from_points_correct_extents() {
        let points: &[[f64; 2]] = &[[1.0, 2.0], [5.0, -3.0], [0.0, 7.0], [4.0, 4.0]];
        let bbox = BoundingBox2d::from_points(points);
        assert_eq!(bbox.min_x, 0.0);
        assert_eq!(bbox.min_y, -3.0);
        assert_eq!(bbox.max_x, 5.0);
        assert_eq!(bbox.max_y, 7.0);
    }

    #[test]
    fn bounding_box_single_point() {
        let points: &[[f64; 2]] = &[[3.0, 4.0]];
        let bbox = BoundingBox2d::from_points(points);
        assert_eq!(bbox.min_x, 3.0);
        assert_eq!(bbox.min_y, 4.0);
        assert_eq!(bbox.max_x, 3.0);
        assert_eq!(bbox.max_y, 4.0);
    }

    #[test]
    fn unit_system_serializes_to_snake_case() {
        let mm = serde_json::to_string(&UnitSystem::Mm).unwrap();
        assert_eq!(mm, "\"mm\"");

        let inches = serde_json::to_string(&UnitSystem::Inches).unwrap();
        assert_eq!(inches, "\"inches\"");
    }

    #[test]
    fn curve2d_round_trips_through_serde_json() {
        let id = Uuid::new_v4();
        let curve = Curve2d {
            id,
            is_closed: true,
            points: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
            layer: Some("outline".to_string()),
        };

        let json = serde_json::to_string(&curve).unwrap();
        let deserialized: Curve2d = serde_json::from_str(&json).unwrap();

        assert_eq!(curve, deserialized);
    }

    #[test]
    fn curve2d_round_trips_with_no_layer() {
        let id = Uuid::new_v4();
        let curve = Curve2d {
            id,
            is_closed: false,
            points: vec![[1.0, 2.0], [3.0, 4.0]],
            layer: None,
        };

        let json = serde_json::to_string(&curve).unwrap();
        let deserialized: Curve2d = serde_json::from_str(&json).unwrap();

        assert_eq!(curve, deserialized);
    }

    #[test]
    fn parse_svg_finds_two_curves() {
        let bytes = include_bytes!("../../../tests/integration/twod/rect.svg");
        let curves = parse_svg(bytes, UnitSystem::Mm).unwrap();
        assert_eq!(curves.len(), 2, "expected 2 curves, got {}", curves.len());
    }

    #[test]
    fn parse_svg_closed_and_open_flags() {
        let bytes = include_bytes!("../../../tests/integration/twod/rect.svg");
        let curves = parse_svg(bytes, UnitSystem::Mm).unwrap();
        assert!(curves.iter().any(|c| c.is_closed), "no closed curve found");
        assert!(curves.iter().any(|c| !c.is_closed), "no open curve found");
    }

    #[test]
    fn parse_svg_y_flip() {
        let bytes = include_bytes!("../../../tests/integration/twod/rect.svg");
        let curves = parse_svg(bytes, UnitSystem::Mm).unwrap();
        let closed = curves.iter().find(|c| c.is_closed).unwrap();
        assert!(
            closed.points.iter().all(|p| p[1] <= 0.0),
            "expected all Y <= 0.0 after Y-flip; points: {:?}",
            closed.points
        );
    }

    #[test]
    fn parse_dxf_rect_two_curves_closed_open_mm() {
        let bytes = include_bytes!("../../../tests/integration/twod/rect.dxf");
        let (curves, unit_system) = parse_dxf(bytes).unwrap();
        assert_eq!(curves.len(), 2, "expected 2 curves, got {}", curves.len());
        assert!(curves.iter().any(|c| c.is_closed), "no closed curve found");
        assert!(curves.iter().any(|c| !c.is_closed), "no open curve found");
        assert_eq!(unit_system, UnitSystem::Mm);
    }

    #[test]
    fn parse_dxf_circle_is_closed_64_points() {
        let dxf = "\
  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n\
  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n\
  0\nCIRCLE\n  8\n0\n 10\n0.0\n 20\n0.0\n 30\n0.0\n 40\n10.0\n\
  0\nENDSEC\n  0\nEOF\n";
        let (curves, _) = parse_dxf(dxf.as_bytes()).unwrap();
        assert_eq!(curves.len(), 1);
        assert!(curves[0].is_closed, "circle must be closed");
        assert_eq!(curves[0].points.len(), 64);
    }

    #[test]
    fn parse_dxf_inches_to_mm_conversion() {
        // $INSUNITS = 1 (inches); 1×1 closed square LWPOLYLINE
        let dxf = "\
  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n\
  9\n$INSUNITS\n 70\n1\n\
  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n\
  0\nLWPOLYLINE\n  8\n0\n 70\n1\n 90\n4\n\
 10\n0.0\n 20\n0.0\n 10\n1.0\n 20\n0.0\n\
 10\n1.0\n 20\n1.0\n 10\n0.0\n 20\n1.0\n\
  0\nENDSEC\n  0\nEOF\n";
        let (curves, unit_system) = parse_dxf(dxf.as_bytes()).unwrap();
        assert_eq!(unit_system, UnitSystem::Inches);
        assert_eq!(curves.len(), 1);
        let bbox = BoundingBox2d::from_points(&curves[0].points);
        assert!(
            (bbox.max_x - bbox.min_x - 25.4).abs() < 0.001,
            "width should be ~25.4 mm, got {}",
            bbox.max_x - bbox.min_x
        );
        assert!(
            (bbox.max_y - bbox.min_y - 25.4).abs() < 0.001,
            "height should be ~25.4 mm, got {}",
            bbox.max_y - bbox.min_y
        );
    }

    #[test]
    fn parse_svg_scale() {
        let bytes = include_bytes!("../../../tests/integration/twod/rect.svg");
        let curves = parse_svg(bytes, UnitSystem::Mm).unwrap();
        let closed = curves.iter().find(|c| c.is_closed).unwrap();
        let bbox = BoundingBox2d::from_points(&closed.points);
        let expected = 100.0 * 25.4 / 96.0; // ≈ 26.458 mm
        assert!(
            (bbox.max_x - bbox.min_x - expected).abs() < 0.01,
            "width: expected {expected:.4} mm, got {:.4} mm",
            bbox.max_x - bbox.min_x
        );
        assert!(
            (bbox.max_y - bbox.min_y - expected).abs() < 0.01,
            "height: expected {expected:.4} mm, got {:.4} mm",
            bbox.max_y - bbox.min_y
        );
    }
}
