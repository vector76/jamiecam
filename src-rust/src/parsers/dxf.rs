//! DXF parser: produces [`Polyline`]s in millimetres from a DXF document
//! (the underlying `dxf` crate accepts both ASCII DXF and the AutoCAD binary
//! DXF / DXB variants).
//!
//! Supported entities: `LINE`, `CIRCLE`, `ARC`, `LWPOLYLINE`, `POLYLINE` (2D
//! only — 3D polylines, polygon meshes, and polyface meshes are skipped with
//! a warning), and `SPLINE` (both fit-point and control-point splines,
//! including rational weighted NURBS).
//!
//! Curves (circles, arcs, polyline bulges, splines) are flattened to within
//! [`super::DEFLECTION_TOLERANCE_MM`] of the analytic curve.
//!
//! Units are normalised by reading the `$INSUNITS` header variable. If it is
//! missing or set to `Unitless`, the parser assumes millimetres and emits a
//! [`ParseWarning`] so the UI can surface the assumption.

use std::io::Cursor;

use dxf::{
    entities::{
        Arc as DxfArc, Circle as DxfCircle, EntityType, Line as DxfLine, LwPolyline,
        Polyline as DxfPolyline, Spline as DxfSpline,
    },
    enums::Units,
    Drawing,
};

use crate::error::{AppError, ParseFailure};
use crate::geometry2d::{Point2, Polyline};
use crate::parse_warning::ParseWarning;

use super::DEFLECTION_TOLERANCE_MM;

/// Result of parsing a DXF document: the extracted 2D paths plus any
/// non-fatal warnings (e.g. missing `$INSUNITS`, skipped 3D entity).
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDxf {
    pub paths: Vec<Polyline>,
    pub warnings: Vec<ParseWarning>,
}

/// Parse a DXF document (ASCII or binary) into [`Polyline`]s in millimetres.
///
/// Returns [`AppError::ParseFailure`] only when the document cannot be read at
/// all (malformed structure, unsupported version). Per-entity issues become
/// warnings.
pub fn parse_dxf(bytes: &[u8]) -> Result<ParsedDxf, AppError> {
    let mut cursor = Cursor::new(bytes);
    let drawing = Drawing::load(&mut cursor).map_err(|e| {
        AppError::ParseFailure(ParseFailure {
            source: "dxf".into(),
            message: format!("failed to load DXF: {e}"),
            line: None,
        })
    })?;

    let mut warnings = Vec::new();
    let scale = match unit_scale_mm(drawing.header.default_drawing_units) {
        Some(s) => s,
        None => {
            warnings.push(ParseWarning {
                line: None,
                message: "DXF $INSUNITS missing or unsupported; assuming millimetres".into(),
            });
            1.0
        }
    };

    let mut paths = Vec::new();
    let push_path = |paths: &mut Vec<Polyline>, p: Polyline| {
        if !p.is_empty() {
            paths.push(p);
        }
    };
    for entity in drawing.entities() {
        match &entity.specific {
            EntityType::Line(line) => push_path(&mut paths, line_to_polyline(line, scale)),
            EntityType::Circle(c) => push_path(
                &mut paths,
                circle_to_polyline(c, scale, DEFLECTION_TOLERANCE_MM),
            ),
            EntityType::Arc(a) => push_path(
                &mut paths,
                arc_to_polyline(a, scale, DEFLECTION_TOLERANCE_MM),
            ),
            EntityType::LwPolyline(p) => push_path(
                &mut paths,
                lw_polyline_to_polyline(p, scale, DEFLECTION_TOLERANCE_MM),
            ),
            EntityType::Polyline(p) => {
                if p.is_3d_polyline() || p.is_3d_polygon_mesh() || p.is_polyface_mesh() {
                    warnings.push(ParseWarning {
                        line: None,
                        message: "skipped non-2D POLYLINE (3D polyline, polygon mesh, or polyface mesh); Mode 2 imports 2D only".into(),
                    });
                    continue;
                }
                push_path(
                    &mut paths,
                    polyline_to_polyline(p, scale, DEFLECTION_TOLERANCE_MM),
                );
            }
            EntityType::Spline(s) => match spline_to_polyline(s, scale, DEFLECTION_TOLERANCE_MM) {
                Ok(pl) => push_path(&mut paths, pl),
                Err(msg) => warnings.push(ParseWarning {
                    line: None,
                    message: format!("spline skipped: {msg}"),
                }),
            },
            _ => {}
        }
    }

    Ok(ParsedDxf { paths, warnings })
}

/// Returns the mm-per-unit scale for a `$INSUNITS` value, or `None` for
/// `Unitless` and the handful of astronomical/typographic units that don't
/// make sense for a CNC drawing.
fn unit_scale_mm(u: Units) -> Option<f64> {
    match u {
        Units::Millimeters => Some(1.0),
        Units::Centimeters => Some(10.0),
        Units::Decimeters => Some(100.0),
        Units::Meters => Some(1_000.0),
        Units::Decameters => Some(10_000.0),
        Units::Hectometers => Some(100_000.0),
        Units::Kilometers => Some(1_000_000.0),
        Units::Inches => Some(25.4),
        Units::Feet => Some(304.8),
        Units::Yards => Some(914.4),
        Units::Miles => Some(1_609_344.0),
        Units::Microinches => Some(25.4e-6),
        Units::Mils => Some(0.0254),
        Units::Microns => Some(0.001),
        Units::Nanometers => Some(1e-6),
        Units::Angstroms => Some(1e-7),
        Units::USSurveyInch => Some(25.400_050_8),
        Units::USSurveyFeet => Some(304.800_609_6),
        Units::USSurveyYard => Some(914.401_828_8),
        Units::USSurveyMile => Some(1_609_347.218_69),
        _ => None,
    }
}

fn line_to_polyline(line: &DxfLine, scale: f64) -> Polyline {
    Polyline::open(vec![
        Point2::new(line.p1.x * scale, line.p1.y * scale),
        Point2::new(line.p2.x * scale, line.p2.y * scale),
    ])
}

fn circle_to_polyline(c: &DxfCircle, scale: f64, tol: f64) -> Polyline {
    let r = c.radius * scale;
    let cx = c.center.x * scale;
    let cy = c.center.y * scale;
    let n = arc_segment_count(r, std::f64::consts::TAU, tol);
    let mut pts = Vec::with_capacity(n);
    for i in 0..n {
        let t = (i as f64) / (n as f64) * std::f64::consts::TAU;
        pts.push(Point2::new(cx + r * t.cos(), cy + r * t.sin()));
    }
    Polyline::closed(pts)
}

fn arc_to_polyline(a: &DxfArc, scale: f64, tol: f64) -> Polyline {
    let r = a.radius * scale;
    let cx = a.center.x * scale;
    let cy = a.center.y * scale;
    let start = a.start_angle.to_radians();
    let end = a.end_angle.to_radians();
    // DXF arcs sweep CCW from start_angle to end_angle. If end <= start, the
    // arc wraps through the +x axis.
    let mut sweep = end - start;
    while sweep <= 0.0 {
        sweep += std::f64::consts::TAU;
    }
    let n = arc_segment_count(r, sweep, tol).max(1);
    let mut pts = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let t = start + sweep * (i as f64) / (n as f64);
        pts.push(Point2::new(cx + r * t.cos(), cy + r * t.sin()));
    }
    Polyline::open(pts)
}

/// Number of chord segments needed to approximate an arc of `radius` and
/// `sweep_rad` to within `tol` (deflection between chord and arc).
///
/// Derivation: for a chord subtending angle `θ` on a circle of radius `r`,
/// the maximum deflection (sagitta) is `r·(1 − cos(θ/2))`. Setting that equal
/// to `tol` gives `θ = 2·acos(1 − tol/r)`. Number of segments is the sweep
/// divided by `θ`, rounded up.
fn arc_segment_count(radius: f64, sweep_rad: f64, tol: f64) -> usize {
    if radius <= 0.0 || tol <= 0.0 || sweep_rad <= 0.0 {
        return 1;
    }
    let ratio = (1.0 - tol / radius).clamp(-1.0, 1.0);
    let max_segment_angle = 2.0 * ratio.acos();
    if max_segment_angle <= f64::EPSILON {
        // `tol / radius` was smaller than f64 precision, so 1.0 - x rounded
        // to 1.0 and acos(1.0) = 0. The curve is indistinguishable from a
        // straight line at this precision — one segment is correct.
        return 1;
    }
    let n = (sweep_rad / max_segment_angle).ceil() as usize;
    n.clamp(1, 4096)
}

fn lw_polyline_to_polyline(p: &LwPolyline, scale: f64, tol: f64) -> Polyline {
    let verts: Vec<(f64, f64, f64)> = p
        .vertices
        .iter()
        .map(|v| (v.x * scale, v.y * scale, v.bulge))
        .collect();
    build_polyline(&verts, p.is_closed(), tol)
}

fn polyline_to_polyline(p: &DxfPolyline, scale: f64, tol: f64) -> Polyline {
    let verts: Vec<(f64, f64, f64)> = p
        .vertices()
        .map(|v| (v.location.x * scale, v.location.y * scale, v.bulge))
        .collect();
    build_polyline(&verts, p.is_closed(), tol)
}

/// Build a [`Polyline`] from a list of `(x, y, bulge)` vertices in
/// millimetres, with the same closure semantics as the two DXF polyline
/// flavours.
///
/// If `closed` is true and the input has a redundant trailing vertex that
/// coincides with the first (some DXF emitters duplicate the closing point
/// even though the closed flag already makes closure implicit), it is
/// dropped to match `geometry2d`'s implicit-closure convention.
fn build_polyline(verts: &[(f64, f64, f64)], closed: bool, tol: f64) -> Polyline {
    let n = verts.len();
    if n == 0 {
        return if closed {
            Polyline::closed(Vec::new())
        } else {
            Polyline::open(Vec::new())
        };
    }
    // Drop a redundant duplicated closing vertex before computing edges,
    // otherwise the closing edge from the duplicate back to vertex 0 would
    // be zero-length but still emit a bogus point.
    let effective_n = if closed && n >= 2 && coincident(verts[0], verts[n - 1]) {
        n - 1
    } else {
        n
    };
    if effective_n == 0 {
        return Polyline::closed(Vec::new());
    }
    let mut pts = Vec::with_capacity(effective_n);
    let edge_count = if closed { effective_n } else { effective_n - 1 };
    for i in 0..edge_count {
        let (x0, y0, b0) = verts[i];
        let (x1, y1, _) = verts[(i + 1) % effective_n];
        pts.push(Point2::new(x0, y0));
        if b0.abs() > f64::EPSILON {
            flatten_bulge((x0, y0), (x1, y1), b0, tol, &mut pts);
        }
    }
    if !closed {
        let (x, y, _) = verts[effective_n - 1];
        pts.push(Point2::new(x, y));
    }
    if closed {
        Polyline::closed(pts)
    } else {
        Polyline::open(pts)
    }
}

fn coincident(a: (f64, f64, f64), b: (f64, f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
}

/// Flatten a polyline "bulge" arc between two consecutive vertices.
///
/// `bulge = tan(θ/4)` where `θ` is the included arc angle. We follow the
/// convention used by ezdxf and the majority of CAM tooling: a positive
/// bulge puts the arc on the **left** side of the chord when looking from
/// `p0` to `p1`; a negative bulge puts it on the right. The DXF reference
/// describes the sign in CCW/CW terms instead, but the two phrasings are
/// equivalent in practice and the "left of chord" framing is what most
/// drawings produced by real CAD packages assume.
///
/// The first vertex (`p0`) is assumed already pushed by the caller; this
/// function emits only the *intermediate* points and lets the caller push the
/// next vertex.
fn flatten_bulge(p0: (f64, f64), p1: (f64, f64), bulge: f64, tol: f64, out: &mut Vec<Point2>) {
    let abs_bulge = bulge.abs();
    let sign = if bulge >= 0.0 { 1.0 } else { -1.0 };
    let abs_theta = 4.0 * abs_bulge.atan();
    let chord = ((p1.0 - p0.0).powi(2) + (p1.1 - p0.1).powi(2)).sqrt();
    if chord < f64::EPSILON || abs_theta < f64::EPSILON {
        return;
    }
    let half = abs_theta * 0.5;
    let sin_h = half.sin();
    let cos_h = half.cos();
    if sin_h.abs() < f64::EPSILON {
        return;
    }
    let r = (chord * 0.5) / sin_h;
    let mx = (p0.0 + p1.0) * 0.5;
    let my = (p0.1 + p1.1) * 0.5;
    let dx = (p1.0 - p0.0) / chord;
    let dy = (p1.1 - p0.1) / chord;
    // For positive bulge, arc is on the left (chord rotated +90°). The
    // centre is on the opposite (right) side for minor arcs (|θ| < π), on
    // the same (left) side for major arcs (|θ| > π). The signed factor
    // `cos(θ/2) / sin(θ/2)` carries that flip automatically — it is positive
    // when |θ| < π and negative when |θ| > π.
    let right_perp = (dy, -dx);
    let d_signed = sign * (chord * 0.5) * cos_h / sin_h;
    let cx = mx + right_perp.0 * d_signed;
    let cy = my + right_perp.1 * d_signed;
    let a0 = (p0.1 - cy).atan2(p0.0 - cx);
    // Positive bulge → traverse CW around the centre (decreasing angle), so
    // the intermediate samples land on the left side of the chord.
    let dir = -sign;
    let n = arc_segment_count(r, abs_theta, tol);
    for i in 1..n {
        let t = a0 + dir * abs_theta * (i as f64) / (n as f64);
        out.push(Point2::new(cx + r * t.cos(), cy + r * t.sin()));
    }
}

/// Flatten a (possibly rational) B-spline.
///
/// If `fit_points` are supplied, prefer them — the source CAD has already
/// chosen sampling that satisfies its own fit tolerance, and we cannot beat
/// it without re-fitting. Otherwise evaluate via de Boor and subdivide
/// adaptively until the chord deflection is below `tol`.
fn spline_to_polyline(s: &DxfSpline, scale: f64, tol: f64) -> Result<Polyline, String> {
    let closed = s.is_closed();
    if !s.fit_points.is_empty() {
        let mut pts: Vec<_> = s
            .fit_points
            .iter()
            .map(|p| Point2::new(p.x * scale, p.y * scale))
            .collect();
        if closed {
            // Some emitters duplicate the closing fit point; geometry2d's
            // closed convention is implicit, so drop it.
            if pts.len() >= 2 {
                let (first, last) = (pts[0], pts[pts.len() - 1]);
                if (first.x - last.x).abs() < 1e-9 && (first.y - last.y).abs() < 1e-9 {
                    pts.pop();
                }
            }
            return Ok(Polyline::closed(pts));
        }
        return Ok(Polyline::open(pts));
    }
    let degree = s.degree_of_curve as usize;
    if s.control_points.len() <= degree {
        return Err(format!(
            "{} control points cannot define a degree-{} curve",
            s.control_points.len(),
            degree
        ));
    }
    let expected_knots = s.control_points.len() + degree + 1;
    if s.knot_values.len() != expected_knots {
        return Err(format!(
            "knot vector has {} entries but expected {} for {} control points at degree {}",
            s.knot_values.len(),
            expected_knots,
            s.control_points.len(),
            degree
        ));
    }
    let weights: Vec<f64> = if s.weight_values.len() == s.control_points.len() {
        s.weight_values.clone()
    } else {
        vec![1.0; s.control_points.len()]
    };
    let ctl: Vec<(f64, f64)> = s
        .control_points
        .iter()
        .map(|p| (p.x * scale, p.y * scale))
        .collect();
    let knots = &s.knot_values;
    let t0 = knots[degree];
    let t1 = knots[knots.len() - degree - 1];
    let p0 = eval_nurbs(t0, &ctl, &weights, knots, degree);
    let p1 = eval_nurbs(t1, &ctl, &weights, knots, degree);
    let mut pts = vec![Point2::new(p0.0, p0.1)];
    flatten_curve(
        t0, t1, p0, p1, &ctl, &weights, knots, degree, tol, &mut pts, 0,
    );
    if closed {
        if let (Some(first), Some(last)) = (pts.first().copied(), pts.last().copied()) {
            if (first.x - last.x).abs() < 1e-9 && (first.y - last.y).abs() < 1e-9 {
                pts.pop();
            }
        }
        Ok(Polyline::closed(pts))
    } else {
        Ok(Polyline::open(pts))
    }
}

/// Subdivide recursively at parameter midpoints until the curve is within
/// `tol` of its chord. Bounded depth so degenerate inputs cannot recurse
/// forever.
const MAX_SUBDIVIDE_DEPTH: u32 = 20;

#[allow(clippy::too_many_arguments)]
fn flatten_curve(
    t0: f64,
    t1: f64,
    p0: (f64, f64),
    p1: (f64, f64),
    ctl: &[(f64, f64)],
    weights: &[f64],
    knots: &[f64],
    degree: usize,
    tol: f64,
    out: &mut Vec<Point2>,
    depth: u32,
) {
    let tm = (t0 + t1) * 0.5;
    let pm = eval_nurbs(tm, ctl, weights, knots, degree);
    let line_mx = (p0.0 + p1.0) * 0.5;
    let line_my = (p0.1 + p1.1) * 0.5;
    let dev = ((pm.0 - line_mx).powi(2) + (pm.1 - line_my).powi(2)).sqrt();
    if depth >= MAX_SUBDIVIDE_DEPTH || dev < tol {
        out.push(Point2::new(p1.0, p1.1));
    } else {
        flatten_curve(
            t0,
            tm,
            p0,
            pm,
            ctl,
            weights,
            knots,
            degree,
            tol,
            out,
            depth + 1,
        );
        flatten_curve(
            tm,
            t1,
            pm,
            p1,
            ctl,
            weights,
            knots,
            degree,
            tol,
            out,
            depth + 1,
        );
    }
}

/// De Boor's algorithm for a rational B-spline. Works in homogeneous (wx, wy,
/// w) space and projects back at the end so weighted NURBS work for free.
fn eval_nurbs(
    t: f64,
    ctl: &[(f64, f64)],
    weights: &[f64],
    knots: &[f64],
    degree: usize,
) -> (f64, f64) {
    let n = ctl.len() - 1;
    // Knot span (Piegl & Tiller §2.4): the index `span` such that
    // `knots[span] <= t < knots[span+1]`, clamped to [degree, n]. The end of
    // the parameter range (`t == knots[n+1]`) is a special case that always
    // belongs to span `n`.
    let span = if t >= knots[n + 1] {
        n
    } else {
        let mut s = degree;
        while s < n && t >= knots[s + 1] {
            s += 1;
        }
        s
    };
    let mut d: Vec<(f64, f64, f64)> = (0..=degree)
        .map(|j| {
            let i = span - degree + j;
            let w = weights[i];
            (ctl[i].0 * w, ctl[i].1 * w, w)
        })
        .collect();
    for r in 1..=degree {
        for j in (r..=degree).rev() {
            let i = span - degree + j;
            let lo = knots[i];
            let hi = knots[i + degree - r + 1];
            let denom = hi - lo;
            let alpha = if denom.abs() < f64::EPSILON {
                0.0
            } else {
                (t - lo) / denom
            };
            let a = d[j - 1];
            let b = d[j];
            d[j] = (
                (1.0 - alpha) * a.0 + alpha * b.0,
                (1.0 - alpha) * a.1 + alpha * b.1,
                (1.0 - alpha) * a.2 + alpha * b.2,
            );
        }
    }
    let (wx, wy, w) = d[degree];
    if w.abs() < f64::EPSILON {
        (wx, wy)
    } else {
        (wx / w, wy / w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE_MM: &[u8] = include_bytes!("dxf_fixtures/line_mm.dxf");
    const LWPOLY_RECT_INCH: &[u8] = include_bytes!("dxf_fixtures/lwpoly_rect_inch.dxf");
    const CIRCLE_MM: &[u8] = include_bytes!("dxf_fixtures/circle_mm.dxf");
    const ARC_MM: &[u8] = include_bytes!("dxf_fixtures/arc_mm.dxf");
    const ARC_WRAP_MM: &[u8] = include_bytes!("dxf_fixtures/arc_wrap_mm.dxf");
    const LWPOLY_BULGE_MM: &[u8] = include_bytes!("dxf_fixtures/lwpoly_bulge_mm.dxf");
    const LWPOLY_CLOSED_DUPLICATE_MM: &[u8] =
        include_bytes!("dxf_fixtures/lwpoly_closed_duplicate_mm.dxf");
    const LWPOLY_BULGE_VERTICAL_MM: &[u8] =
        include_bytes!("dxf_fixtures/lwpoly_bulge_vertical_mm.dxf");
    const POLYLINE_MM: &[u8] = include_bytes!("dxf_fixtures/polyline_mm.dxf");
    const POLYLINE_CLOSED_MM: &[u8] = include_bytes!("dxf_fixtures/polyline_closed_mm.dxf");
    const SPLINE_FIT_MM: &[u8] = include_bytes!("dxf_fixtures/spline_fit_mm.dxf");
    const SPLINE_CTRL_MM: &[u8] = include_bytes!("dxf_fixtures/spline_ctrl_mm.dxf");
    const NO_UNITS: &[u8] = include_bytes!("dxf_fixtures/no_units.dxf");

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn line_in_mm_round_trips_endpoints() {
        let parsed = parse_dxf(LINE_MM).unwrap();
        assert_eq!(parsed.warnings, Vec::<ParseWarning>::new());
        assert_eq!(parsed.paths.len(), 1);
        let pl = &parsed.paths[0];
        assert!(!pl.is_closed());
        assert_eq!(pl.points.len(), 2);
        assert!(approx_eq(pl.points[0].x, 0.0, 1e-9));
        assert!(approx_eq(pl.points[0].y, 0.0, 1e-9));
        assert!(approx_eq(pl.points[1].x, 10.0, 1e-9));
        assert!(approx_eq(pl.points[1].y, 5.0, 1e-9));
    }

    #[test]
    fn inch_units_scale_to_mm() {
        let parsed = parse_dxf(LWPOLY_RECT_INCH).unwrap();
        assert!(parsed.warnings.is_empty());
        assert_eq!(parsed.paths.len(), 1);
        let pl = &parsed.paths[0];
        assert!(pl.is_closed());
        assert_eq!(pl.points.len(), 4);
        // 1" × 2" rect at origin becomes 25.4 × 50.8 mm rect.
        let max_x = pl.points.iter().map(|p| p.x).fold(f64::MIN, f64::max);
        let max_y = pl.points.iter().map(|p| p.y).fold(f64::MIN, f64::max);
        assert!(approx_eq(max_x, 25.4, 1e-6));
        assert!(approx_eq(max_y, 50.8, 1e-6));
    }

    #[test]
    fn circle_flattens_within_tolerance() {
        let parsed = parse_dxf(CIRCLE_MM).unwrap();
        assert_eq!(parsed.paths.len(), 1);
        let pl = &parsed.paths[0];
        assert!(pl.is_closed());
        assert!(pl.points.len() >= 16);
        for p in &pl.points {
            let r = (p.x.powi(2) + p.y.powi(2)).sqrt();
            assert!(
                approx_eq(r, 10.0, DEFLECTION_TOLERANCE_MM + 1e-6),
                "point {p:?} not within tolerance of r=10"
            );
        }
    }

    #[test]
    fn arc_emits_open_polyline_with_correct_endpoints() {
        let parsed = parse_dxf(ARC_MM).unwrap();
        assert_eq!(parsed.paths.len(), 1);
        let pl = &parsed.paths[0];
        assert!(!pl.is_closed());
        // Arc is r=5, 0° → 90° CCW, centred at origin.
        let first = pl.points.first().unwrap();
        let last = pl.points.last().unwrap();
        assert!(approx_eq(first.x, 5.0, 1e-6));
        assert!(approx_eq(first.y, 0.0, 1e-6));
        assert!(approx_eq(last.x, 0.0, 1e-6));
        assert!(approx_eq(last.y, 5.0, 1e-6));
    }

    #[test]
    fn lwpolyline_bulge_flattens_to_arc() {
        let parsed = parse_dxf(LWPOLY_BULGE_MM).unwrap();
        assert_eq!(parsed.paths.len(), 1);
        let pl = &parsed.paths[0];
        // Bulge = 1.0 means 180° arc: a semicircle from (0,0) to (10,0).
        // Positive bulge puts the arc on the left of the chord direction
        // (+x here), so left = +y, apex at (5, 5).
        assert!(!pl.is_closed());
        assert!(pl.points.len() >= 8);
        let first = pl.points.first().unwrap();
        let last = pl.points.last().unwrap();
        assert!(approx_eq(first.x, 0.0, 1e-6));
        assert!(approx_eq(first.y, 0.0, 1e-6));
        assert!(approx_eq(last.x, 10.0, 1e-6));
        assert!(approx_eq(last.y, 0.0, 1e-6));
        for p in &pl.points[1..pl.points.len() - 1] {
            assert!(p.y > -1e-6, "expected y >= 0, got {p:?}");
        }
        // The apex should approach (5, 5).
        let max_y = pl.points.iter().map(|p| p.y).fold(f64::MIN, f64::max);
        assert!(approx_eq(max_y, 5.0, DEFLECTION_TOLERANCE_MM + 1e-6));
    }

    #[test]
    fn legacy_polyline_parses_to_open_polyline() {
        let parsed = parse_dxf(POLYLINE_MM).unwrap();
        assert_eq!(parsed.paths.len(), 1);
        let pl = &parsed.paths[0];
        assert!(!pl.is_closed());
        assert_eq!(pl.points.len(), 3);
        assert!(approx_eq(pl.points[0].x, 0.0, 1e-9));
        assert!(approx_eq(pl.points[1].x, 5.0, 1e-9));
        assert!(approx_eq(pl.points[2].x, 10.0, 1e-9));
    }

    #[test]
    fn closed_lwpolyline_drops_duplicated_closing_vertex() {
        // Five vertices on a 10x10 square with the closed flag set AND
        // vertex 4 equal to vertex 0. The parser should treat the
        // duplicate as redundant — 4 distinct points, closed.
        let parsed = parse_dxf(LWPOLY_CLOSED_DUPLICATE_MM).unwrap();
        assert_eq!(parsed.paths.len(), 1);
        let pl = &parsed.paths[0];
        assert!(pl.is_closed());
        assert_eq!(pl.points.len(), 4);
    }

    #[test]
    fn legacy_polyline_closed_keeps_implicit_closure() {
        // Closed rectangle vertices: (0,0), (10,0), (10,10), (0,10). The
        // closing edge from (0,10) back to (0,0) must not duplicate the
        // first point per the geometry2d closure convention.
        let parsed = parse_dxf(POLYLINE_CLOSED_MM).unwrap();
        assert_eq!(parsed.paths.len(), 1);
        let pl = &parsed.paths[0];
        assert!(pl.is_closed());
        assert_eq!(pl.points.len(), 4);
        assert!(approx_eq(pl.points[0].x, 0.0, 1e-9));
        assert!(approx_eq(pl.points[2].x, 10.0, 1e-9));
        assert!(approx_eq(pl.points[2].y, 10.0, 1e-9));
    }

    #[test]
    fn spline_with_fit_points_uses_them_directly() {
        let parsed = parse_dxf(SPLINE_FIT_MM).unwrap();
        assert_eq!(parsed.paths.len(), 1);
        let pl = &parsed.paths[0];
        assert_eq!(pl.points.len(), 3);
        assert!(approx_eq(pl.points[1].x, 3.0, 1e-9));
        assert!(approx_eq(pl.points[1].y, 4.0, 1e-9));
    }

    #[test]
    fn spline_control_points_flatten_via_de_boor() {
        let parsed = parse_dxf(SPLINE_CTRL_MM).unwrap();
        assert_eq!(parsed.paths.len(), 1);
        let pl = &parsed.paths[0];
        // Cubic Bezier on (0,0) (0,10) (10,10) (10,0):
        // - starts at (0,0), ends at (10,0)
        // - convex hull bounds y ∈ [0, 10]
        assert!(pl.points.len() >= 4);
        let first = pl.points.first().unwrap();
        let last = pl.points.last().unwrap();
        assert!(approx_eq(first.x, 0.0, 1e-6));
        assert!(approx_eq(first.y, 0.0, 1e-6));
        assert!(approx_eq(last.x, 10.0, 1e-6));
        assert!(approx_eq(last.y, 0.0, 1e-6));
        for p in &pl.points {
            assert!((-1e-6..=10.0 + 1e-6).contains(&p.y));
        }
    }

    #[test]
    fn missing_insunits_emits_warning_and_assumes_mm() {
        let parsed = parse_dxf(NO_UNITS).unwrap();
        assert_eq!(parsed.paths.len(), 1);
        assert_eq!(parsed.warnings.len(), 1);
        assert!(parsed.warnings[0].message.contains("INSUNITS"));
        // Coordinates pass through unchanged at scale = 1.
        let pl = &parsed.paths[0];
        assert!(approx_eq(pl.points[1].x, 7.0, 1e-9));
    }

    #[test]
    fn invalid_dxf_returns_parse_failure() {
        let err = parse_dxf(b"this is not a dxf file").unwrap_err();
        match err {
            AppError::ParseFailure(detail) => assert_eq!(detail.source, "dxf"),
            other => panic!("expected ParseFailure, got {other:?}"),
        }
    }

    #[test]
    fn bulge_works_on_non_axis_aligned_chord() {
        // Chord straight up the +y axis. Positive bulge means arc on the
        // *left* of the chord direction; left of +y is -x. A semicircle
        // (bulge=1) from (0,0) to (0,10) should apex at (-5, 5).
        let parsed = parse_dxf(LWPOLY_BULGE_VERTICAL_MM).unwrap();
        let pl = &parsed.paths[0];
        for p in &pl.points[1..pl.points.len() - 1] {
            assert!(p.x < 1e-6, "expected x <= 0, got {p:?}");
        }
        let min_x = pl.points.iter().map(|p| p.x).fold(f64::MAX, f64::min);
        assert!(approx_eq(min_x, -5.0, DEFLECTION_TOLERANCE_MM + 1e-6));
    }

    #[test]
    fn arc_wrapping_past_zero_degrees_emits_correct_path() {
        // 270° → 90° arc sweeps 180° CCW through 0°/360°. Should pass through
        // (radius, 0) at the wrap, with endpoints (0, -5) and (0, 5).
        let parsed = parse_dxf(ARC_WRAP_MM).unwrap();
        let pl = &parsed.paths[0];
        let first = pl.points.first().unwrap();
        let last = pl.points.last().unwrap();
        assert!(approx_eq(first.x, 0.0, 1e-6));
        assert!(approx_eq(first.y, -5.0, 1e-6));
        assert!(approx_eq(last.x, 0.0, 1e-6));
        assert!(approx_eq(last.y, 5.0, 1e-6));
        // Every sample sits on the right-hand semicircle (x >= 0).
        for p in &pl.points {
            assert!(p.x > -1e-6, "expected x >= 0, got {p:?}");
        }
    }

    #[test]
    fn arc_segment_count_meets_tolerance() {
        // For r=10 and tol=0.05, max segment angle is 2·acos(0.995) ≈ 0.2 rad.
        // A full circle (2π) needs ceil(2π / 0.2) = 32 segments.
        let n = arc_segment_count(10.0, std::f64::consts::TAU, 0.05);
        assert!((31..=33).contains(&n), "got {n}");
    }

    #[test]
    fn arc_segment_count_collapses_to_one_for_huge_radius() {
        // When tol/r is smaller than f64 precision, `1.0 - tol/r` rounds to
        // 1.0 exactly and acos(1.0) = 0. The arc is geometrically straight
        // at this precision, so the result must be one segment, not the
        // safety cap of 4096.
        let n = arc_segment_count(1e20, 1.0, 0.05);
        assert_eq!(n, 1);
    }
}
