//! SVG parser: produces [`Polyline`]s in millimetres from an SVG document.
//!
//! We lean on `usvg` to do the SVG-specific heavy lifting (units, viewBox,
//! nested transforms, presentation styles, `<rect>`/`<circle>`/`<ellipse>`/
//! `<line>`/`<polyline>`/`<polygon>` → path conversion). The output of `usvg`
//! is a flat tree of [`usvg::Path`] nodes whose `data()` is a tiny-skia path
//! containing only `MoveTo`/`LineTo`/`QuadTo`/`CubicTo`/`Close` segments and
//! whose `abs_transform()` is the cumulative transform from group ancestors.
//!
//! We then walk the tree, apply each path's absolute transform up-front to
//! the segment control points, and flatten quadratic and cubic Béziers
//! adaptively to within [`super::DEFLECTION_TOLERANCE_MM`] of the analytic
//! curve. Adaptive midpoint subdivision works on the *transformed* points so
//! the deviation budget is spent in the same canvas (mm) space the caller
//! receives.
//!
//! # Unit normalisation
//!
//! `usvg::Options::dpi` controls how physical units (`mm`, `cm`, `in`, `pt`,
//! `pc`) are resolved into the internal "px" space; setting it to `25.4`
//! makes that space coincident with millimetres. With viewBox in play this
//! still works: viewBox maps user units onto the SVG's intrinsic size, which
//! is now expressed in millimetres, so the resulting `abs_transform` carries
//! user-space coordinates into millimetres regardless of how viewBox is
//! scaled.
//!
//! An SVG whose `width`/`height` are unitless (or absent) is interpreted as
//! "user units == millimetres" — the same convention DXF gets when its
//! `$INSUNITS` header is missing — and a [`ParseWarning`] is emitted so the
//! UI can surface the assumption.
//!
//! # Note on Y axis
//!
//! SVG uses a y-down coordinate system and `usvg` does not flip it. The
//! polylines this parser emits therefore use SVG's native y-down convention.
//! Mode 2's downstream pipeline (planner, G-code emitter) is agnostic to
//! handedness; if a future UI surface wants y-up display it can flip at the
//! presentation boundary.

use usvg::tiny_skia_path::PathSegment;
use usvg::{Group, Node, Options, Transform, Tree};

use crate::error::{AppError, ParseFailure};
use crate::geometry2d::{Point2, Polyline};
use crate::parse_warning::ParseWarning;

use super::DEFLECTION_TOLERANCE_MM;

/// Result of parsing an SVG document: the extracted 2D paths plus any
/// non-fatal warnings (e.g. unitless dimensions).
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSvg {
    pub paths: Vec<Polyline>,
    pub warnings: Vec<ParseWarning>,
}

/// Parse an SVG document into [`Polyline`]s in millimetres.
///
/// Returns [`AppError::ParseFailure`] only when the document cannot be parsed
/// at all (malformed XML, unsupported SVG structure). Per-document concerns
/// like missing units become warnings.
pub fn parse_svg(bytes: &[u8]) -> Result<ParsedSvg, AppError> {
    // Make usvg's internal coordinate space coincide with millimetres
    // (see module docs). With this, a width of "100mm" resolves to 100 size
    // units, "1in" to 25.4, and any abs_transform we read out below maps
    // user-space coordinates directly to millimetres.
    let options = Options {
        dpi: 25.4,
        ..Options::default()
    };

    let tree = Tree::from_data(bytes, &options).map_err(|e| {
        AppError::ParseFailure(ParseFailure {
            source: "svg".into(),
            message: format!("failed to parse SVG: {e}"),
            line: None,
        })
    })?;

    let mut warnings = Vec::new();
    if has_unitless_dimensions(bytes) {
        warnings.push(ParseWarning {
            line: None,
            message: "SVG width/height are unitless; assuming millimetres".into(),
        });
    }

    let mut paths = Vec::new();
    walk_group(tree.root(), DEFLECTION_TOLERANCE_MM, &mut paths);

    Ok(ParsedSvg { paths, warnings })
}

fn walk_group(group: &Group, tol: f64, out: &mut Vec<Polyline>) {
    for node in group.children() {
        match node {
            Node::Group(g) => walk_group(g, tol, out),
            Node::Path(p) => append_path_polylines(p, tol, out),
            // Images and text aren't 2D vector geometry for the planner;
            // skip silently. (If we want to surface them later, this is the
            // place to push a ParseWarning.)
            Node::Image(_) | Node::Text(_) => {}
        }
    }
}

/// Walk one [`usvg::Path`] and append one or more [`Polyline`]s to `out`,
/// splitting on each `MoveTo` (each SVG subpath becomes one polyline). The
/// path's absolute transform is baked into the points up front so that
/// adaptive flattening of cubics/quadratics happens in canvas (mm) space and
/// the tolerance budget is meaningful.
fn append_path_polylines(path: &usvg::Path, tol: f64, out: &mut Vec<Polyline>) {
    let xform = path.abs_transform();
    let mut current: Vec<Point2> = Vec::new();
    let mut closed = false;
    // Last point in *user* (untransformed) space — we need it to seed the
    // next QuadTo/CubicTo's p0.
    let mut last_user: (f64, f64) = (0.0, 0.0);

    for seg in path.data().segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                flush(&mut current, &mut closed, out);
                let q = map_point(&xform, p.x as f64, p.y as f64);
                current.push(Point2::new(q.0, q.1));
                last_user = (p.x as f64, p.y as f64);
            }
            PathSegment::LineTo(p) => {
                let q = map_point(&xform, p.x as f64, p.y as f64);
                current.push(Point2::new(q.0, q.1));
                last_user = (p.x as f64, p.y as f64);
            }
            PathSegment::QuadTo(c, p) => {
                let p0 = map_point(&xform, last_user.0, last_user.1);
                let p1 = map_point(&xform, c.x as f64, c.y as f64);
                let p2 = map_point(&xform, p.x as f64, p.y as f64);
                flatten_quad(p0, p1, p2, tol, &mut current, 0);
                last_user = (p.x as f64, p.y as f64);
            }
            PathSegment::CubicTo(c1, c2, p) => {
                let p0 = map_point(&xform, last_user.0, last_user.1);
                let p1 = map_point(&xform, c1.x as f64, c1.y as f64);
                let p2 = map_point(&xform, c2.x as f64, c2.y as f64);
                let p3 = map_point(&xform, p.x as f64, p.y as f64);
                flatten_cubic(p0, p1, p2, p3, tol, &mut current, 0);
                last_user = (p.x as f64, p.y as f64);
            }
            PathSegment::Close => {
                // SVG's Z draws a line back to the subpath start. Mark the
                // polyline closed; if the previous segment happened to land
                // on the start point we drop the duplicate so geometry2d's
                // implicit-closure convention holds.
                if current.len() >= 2 {
                    let first = current[0];
                    let last = current[current.len() - 1];
                    if (first.x - last.x).abs() < 1e-9 && (first.y - last.y).abs() < 1e-9 {
                        current.pop();
                    }
                }
                closed = true;
                flush(&mut current, &mut closed, out);
                last_user = (0.0, 0.0);
            }
        }
    }
    flush(&mut current, &mut closed, out);
}

fn flush(current: &mut Vec<Point2>, closed: &mut bool, out: &mut Vec<Polyline>) {
    if current.is_empty() {
        *closed = false;
        return;
    }
    let pts = std::mem::take(current);
    let pl = if *closed {
        Polyline::closed(pts)
    } else {
        Polyline::open(pts)
    };
    out.push(pl);
    *closed = false;
}

fn map_point(t: &Transform, x: f64, y: f64) -> (f64, f64) {
    // usvg's Transform uses column-major-column-vector notation: a point
    // (x, y) maps to (sx·x + kx·y + tx, ky·x + sy·y + ty).
    let nx = x * (t.sx as f64) + y * (t.kx as f64) + (t.tx as f64);
    let ny = x * (t.ky as f64) + y * (t.sy as f64) + (t.ty as f64);
    (nx, ny)
}

/// Cap recursion so a pathological curve (cusps, near-zero chord with
/// distant control points) cannot blow the stack.
const MAX_SUBDIVIDE_DEPTH: u32 = 20;

/// Adaptive midpoint subdivision for a quadratic Bézier. We push the
/// endpoint (`p2`) — never `p0` — so chained segments don't duplicate joins.
fn flatten_quad(
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    tol: f64,
    out: &mut Vec<Point2>,
    depth: u32,
) {
    let dev = point_to_line_distance(p1, p0, p2);
    if depth >= MAX_SUBDIVIDE_DEPTH || dev <= tol {
        out.push(Point2::new(p2.0, p2.1));
        return;
    }
    let m01 = midpoint(p0, p1);
    let m12 = midpoint(p1, p2);
    let mid = midpoint(m01, m12);
    flatten_quad(p0, m01, mid, tol, out, depth + 1);
    flatten_quad(mid, m12, p2, tol, out, depth + 1);
}

/// Adaptive midpoint subdivision for a cubic Bézier. We bound the chord
/// deviation by the larger of the two control-point–chord distances; that's
/// a conservative upper bound on the maximum curve–chord error and is
/// standard practice (de Casteljau halving + this test converges fast for
/// any non-degenerate cubic).
fn flatten_cubic(
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    tol: f64,
    out: &mut Vec<Point2>,
    depth: u32,
) {
    let d1 = point_to_line_distance(p1, p0, p3);
    let d2 = point_to_line_distance(p2, p0, p3);
    if depth >= MAX_SUBDIVIDE_DEPTH || d1.max(d2) <= tol {
        out.push(Point2::new(p3.0, p3.1));
        return;
    }
    let p01 = midpoint(p0, p1);
    let p12 = midpoint(p1, p2);
    let p23 = midpoint(p2, p3);
    let p012 = midpoint(p01, p12);
    let p123 = midpoint(p12, p23);
    let p0123 = midpoint(p012, p123);
    flatten_cubic(p0, p01, p012, p0123, tol, out, depth + 1);
    flatten_cubic(p0123, p123, p23, p3, tol, out, depth + 1);
}

fn midpoint(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5)
}

/// Perpendicular distance from `p` to the infinite line through `a` and `b`.
/// When `a` and `b` coincide the line degenerates to a point and we return
/// the Euclidean distance instead.
fn point_to_line_distance(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-24 {
        let px = p.0 - a.0;
        let py = p.1 - a.1;
        return (px * px + py * py).sqrt();
    }
    let cross = dx * (a.1 - p.1) - dy * (a.0 - p.0);
    cross.abs() / len_sq.sqrt()
}

/// Returns `true` if the root `<svg>`'s `width` or `height` is unitless
/// (e.g. `width="100"`), absent, or the document isn't usable XML. We use
/// this to decide whether the "assuming millimetres" warning fires.
///
/// Unit detection by trailing-character inspection: SVG's length grammar
/// allows numeric chars (`0-9 . + - e E`) followed by an optional unit
/// suffix (`em ex px in cm mm pt pc %`). A length is unitless iff its
/// trimmed value's last character is a digit, `.`, or empty.
fn has_unitless_dimensions(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return true;
    };
    let Ok(doc) = usvg::roxmltree::Document::parse(text) else {
        return true;
    };
    let root = doc.root_element();
    if root.tag_name().name() != "svg" {
        return true;
    }
    let w = root.attribute("width");
    let h = root.attribute("height");
    match (w, h) {
        (None, _) | (_, None) => true,
        (Some(w), Some(h)) => is_unitless(w) || is_unitless(h),
    }
}

fn is_unitless(value: &str) -> bool {
    let trimmed = value.trim();
    match trimmed.chars().last() {
        None => true,
        Some(c) => c.is_ascii_digit() || c == '.',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECT_MM: &[u8] = include_bytes!("svg_fixtures/rect_mm.svg");
    const CIRCLE_MM: &[u8] = include_bytes!("svg_fixtures/circle_mm.svg");
    const PATH_CUBIC_MM: &[u8] = include_bytes!("svg_fixtures/path_cubic_mm.svg");
    const GROUP_TRANSFORM_MM: &[u8] = include_bytes!("svg_fixtures/group_transform_mm.svg");
    const RECT_UNITLESS: &[u8] = include_bytes!("svg_fixtures/rect_unitless.svg");

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    fn bbox(points: &[Point2]) -> (f64, f64, f64, f64) {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in points {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
        (min_x, min_y, max_x, max_y)
    }

    #[test]
    fn rect_in_mm_yields_closed_polyline_with_four_corners() {
        let parsed = parse_svg(RECT_MM).unwrap();
        assert!(parsed.warnings.is_empty(), "got {:?}", parsed.warnings);
        assert_eq!(parsed.paths.len(), 1);
        let pl = &parsed.paths[0];
        assert!(pl.is_closed());
        assert_eq!(pl.points.len(), 4);
        let (min_x, min_y, max_x, max_y) = bbox(&pl.points);
        assert!(approx_eq(min_x, 0.0, 1e-6));
        assert!(approx_eq(min_y, 0.0, 1e-6));
        assert!(approx_eq(max_x, 20.0, 1e-6));
        assert!(approx_eq(max_y, 10.0, 1e-6));
    }

    #[test]
    fn circle_flattens_within_tolerance_of_analytic_circle() {
        let parsed = parse_svg(CIRCLE_MM).unwrap();
        assert!(parsed.warnings.is_empty());
        assert_eq!(parsed.paths.len(), 1);
        let pl = &parsed.paths[0];
        assert!(pl.is_closed());
        // usvg approximates <circle> with 4 cubic arcs; after flattening to
        // 0.05 mm tolerance on a radius-5 circle we should see comfortably
        // more than the 4 control-point joins themselves.
        assert!(pl.points.len() >= 16, "got {} points", pl.points.len());
        // Every sample must lie within tolerance of the true circle.
        for p in &pl.points {
            let dx = p.x - 10.0;
            let dy = p.y - 10.0;
            let r = (dx * dx + dy * dy).sqrt();
            assert!(
                approx_eq(r, 5.0, DEFLECTION_TOLERANCE_MM + 1e-6),
                "point {p:?} not within tolerance of r=5"
            );
        }
    }

    #[test]
    fn cubic_path_emits_open_polyline_with_correct_endpoints_and_apex() {
        let parsed = parse_svg(PATH_CUBIC_MM).unwrap();
        assert!(parsed.warnings.is_empty());
        assert_eq!(parsed.paths.len(), 1);
        let pl = &parsed.paths[0];
        assert!(!pl.is_closed());
        let first = pl.points.first().unwrap();
        let last = pl.points.last().unwrap();
        assert!(approx_eq(first.x, 0.0, 1e-6));
        assert!(approx_eq(first.y, 0.0, 1e-6));
        assert!(approx_eq(last.x, 20.0, 1e-6));
        assert!(approx_eq(last.y, 0.0, 1e-6));
        // For a symmetric cubic with control points at y=10, the analytic
        // apex sits at y=7.5 (the value of the Bernstein blend at t=0.5).
        let (_, _, _, max_y) = bbox(&pl.points);
        assert!(
            approx_eq(max_y, 7.5, DEFLECTION_TOLERANCE_MM + 1e-6),
            "apex y was {max_y}"
        );
        // Adaptive flattening of a non-degenerate cubic at 0.05 mm over a
        // 20 mm chord should produce many more than the endpoint pair.
        assert!(pl.points.len() >= 8, "got {} points", pl.points.len());
    }

    #[test]
    fn group_transform_translates_child_path() {
        let parsed = parse_svg(GROUP_TRANSFORM_MM).unwrap();
        assert!(parsed.warnings.is_empty());
        assert_eq!(parsed.paths.len(), 1);
        let pl = &parsed.paths[0];
        assert!(pl.is_closed());
        // 10×5 rect translated by (10, 5) → corners at (10,5)..(20,10).
        let (min_x, min_y, max_x, max_y) = bbox(&pl.points);
        assert!(approx_eq(min_x, 10.0, 1e-6));
        assert!(approx_eq(min_y, 5.0, 1e-6));
        assert!(approx_eq(max_x, 20.0, 1e-6));
        assert!(approx_eq(max_y, 10.0, 1e-6));
    }

    #[test]
    fn unitless_dimensions_emit_warning_and_pass_through_as_mm() {
        let parsed = parse_svg(RECT_UNITLESS).unwrap();
        assert_eq!(parsed.warnings.len(), 1);
        assert!(parsed.warnings[0].message.contains("unitless"));
        assert_eq!(parsed.paths.len(), 1);
        let pl = &parsed.paths[0];
        let (min_x, min_y, max_x, max_y) = bbox(&pl.points);
        // Unitless "20" treated as 20 mm.
        assert!(approx_eq(min_x, 0.0, 1e-6));
        assert!(approx_eq(min_y, 0.0, 1e-6));
        assert!(approx_eq(max_x, 20.0, 1e-6));
        assert!(approx_eq(max_y, 10.0, 1e-6));
    }

    #[test]
    fn invalid_svg_returns_parse_failure() {
        let err = parse_svg(b"not actually an svg document").unwrap_err();
        match err {
            AppError::ParseFailure(detail) => assert_eq!(detail.source, "svg"),
            other => panic!("expected ParseFailure, got {other:?}"),
        }
    }

    #[test]
    fn is_unitless_recognises_common_suffixes() {
        assert!(is_unitless("100"));
        assert!(is_unitless("100.5"));
        assert!(is_unitless(" 42 "));
        assert!(!is_unitless("100mm"));
        assert!(!is_unitless("3.5in"));
        assert!(!is_unitless("50%"));
        assert!(!is_unitless("10pt"));
    }
}
