//! Drill cycle algorithm.
//!
//! Generates Linking and Cutting passes for each hole in a drill operation,
//! with optional peck drilling support.

use crate::error::AppError;
use crate::models::operation::{DrillParams, DrillPoint};
use crate::models::stock::BoxDimensions;
use crate::models::{StockDefinition, Vec3};
use crate::toolpath::types::{CutPoint, MoveKind, Pass, PassKind, DEFAULT_CLEARANCE_OFFSET};

/// Generate linking and cutting passes for a drill operation.
///
/// For each point in `params.points`, one `PassKind::Linking` pass and one
/// `PassKind::Cutting` pass are produced. The linking pass positions the tool
/// at clearance height above the hole; the cutting pass performs the plunge
/// (with optional peck increments).
///
/// Returns `Err(AppError::GeometryImport(...))` if `params.points` is empty
/// or if `params.peck_depth` is `Some` but less than or equal to zero.
pub fn drill_passes(stock: &StockDefinition, params: &DrillParams) -> Result<Vec<Pass>, AppError> {
    if params.points.is_empty() {
        return Err(AppError::GeometryImport(
            "drill points list is empty".to_string(),
        ));
    }
    if let Some(peck) = params.peck_depth {
        if peck <= 0.0 {
            return Err(AppError::GeometryImport(
                "peck_depth must be positive".to_string(),
            ));
        }
    }

    let StockDefinition::Box(BoxDimensions { origin, height, .. }) = stock;

    let stock_top_z = origin.z + height;
    let clearance_z = stock_top_z + DEFAULT_CLEARANCE_OFFSET;
    let drill_z = stock_top_z - params.depth;

    // Nearest-neighbor sort: start with the first point, repeatedly select
    // the closest unvisited point by Euclidean XY distance.
    let sorted_points: Vec<DrillPoint> = {
        let mut remaining: Vec<DrillPoint> = params.points.clone();
        let mut ordered = Vec::with_capacity(remaining.len());
        ordered.push(remaining.remove(0));
        while !remaining.is_empty() {
            let last = ordered.last().unwrap();
            let nearest_idx = remaining
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    let da = (a.x - last.x).powi(2) + (a.y - last.y).powi(2);
                    let db = (b.x - last.x).powi(2) + (b.y - last.y).powi(2);
                    da.total_cmp(&db)
                })
                .map(|(i, _)| i)
                .unwrap();
            ordered.push(remaining.swap_remove(nearest_idx));
        }
        ordered
    };

    let mut passes = Vec::with_capacity(sorted_points.len() * 2);

    for (i, point) in sorted_points.iter().enumerate() {
        // --- Linking pass ---
        let linking_cuts = if i == 0 {
            vec![rapid(point.x, point.y, clearance_z)]
        } else {
            let prev = &sorted_points[i - 1];
            vec![
                rapid(prev.x, prev.y, clearance_z),
                rapid(point.x, point.y, clearance_z),
            ]
        };
        passes.push(Pass {
            kind: PassKind::Linking,
            cuts: linking_cuts,
        });

        // --- Cutting pass ---
        let mut cuts = vec![rapid(point.x, point.y, clearance_z)];

        match params.peck_depth {
            None => {
                cuts.push(feed(point.x, point.y, drill_z));
                cuts.push(rapid(point.x, point.y, clearance_z));
            }
            Some(peck) => {
                let mut current_z = stock_top_z;
                loop {
                    let step_z = (current_z - peck).max(drill_z);
                    cuts.push(feed(point.x, point.y, step_z));
                    cuts.push(rapid(point.x, point.y, clearance_z));
                    if (step_z - drill_z).abs() < 1e-9 {
                        break;
                    }
                    current_z = step_z;
                }
            }
        }

        passes.push(Pass {
            kind: PassKind::Cutting,
            cuts,
        });
    }

    Ok(passes)
}

fn rapid(x: f64, y: f64, z: f64) -> CutPoint {
    CutPoint {
        position: Vec3 { x, y, z },
        move_kind: MoveKind::Rapid,
        tool_orientation: None,
        feed_rate_override: None,
    }
}

fn feed(x: f64, y: f64, z: f64) -> CutPoint {
    CutPoint {
        position: Vec3 { x, y, z },
        move_kind: MoveKind::Feed,
        tool_orientation: None,
        feed_rate_override: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::operation::DrillPoint;
    use crate::models::stock::BoxDimensions;
    use crate::models::Vec3;

    fn make_stock(height: f64) -> StockDefinition {
        StockDefinition::Box(BoxDimensions {
            origin: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            width: 100.0,
            depth: 100.0,
            height,
        })
    }

    fn point(x: f64, y: f64) -> DrillPoint {
        DrillPoint { x, y }
    }

    #[test]
    fn drill_zero_peck_depth_returns_error() {
        let stock = make_stock(10.0);
        let params = DrillParams {
            depth: 5.0,
            points: vec![point(0.0, 0.0)],
            peck_depth: Some(0.0),
        };
        let result = drill_passes(&stock, &params);
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::GeometryImport(_) => {}
            other => panic!("expected GeometryImport, got {:?}", other),
        }
    }

    #[test]
    fn drill_negative_peck_depth_returns_error() {
        let stock = make_stock(10.0);
        let params = DrillParams {
            depth: 5.0,
            points: vec![point(0.0, 0.0)],
            peck_depth: Some(-1.0),
        };
        let result = drill_passes(&stock, &params);
        assert!(result.is_err());
    }

    #[test]
    fn drill_empty_points_returns_error() {
        let stock = make_stock(10.0);
        let params = DrillParams {
            depth: 5.0,
            points: vec![],
            peck_depth: None,
        };
        let result = drill_passes(&stock, &params);
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::GeometryImport(_) => {}
            other => panic!("expected GeometryImport, got {:?}", other),
        }
    }

    #[test]
    fn drill_single_nonpeck_hole() {
        // stock height=10, origin z=0 → stock_top_z=10, clearance_z=15, drill_z=10-8=2
        let stock = make_stock(10.0);
        let params = DrillParams {
            depth: 8.0,
            points: vec![point(5.0, 7.0)],
            peck_depth: None,
        };
        let passes = drill_passes(&stock, &params).expect("should succeed");
        assert_eq!(passes.len(), 2);

        assert_eq!(passes[0].kind, PassKind::Linking);
        assert_eq!(passes[1].kind, PassKind::Cutting);

        let cutting = &passes[1];
        assert_eq!(cutting.cuts.len(), 3);
        assert_eq!(cutting.cuts[0].move_kind, MoveKind::Rapid); // approach
        assert_eq!(cutting.cuts[1].move_kind, MoveKind::Feed); // plunge
        assert_eq!(cutting.cuts[2].move_kind, MoveKind::Rapid); // retract

        let clearance_z = 10.0 + DEFAULT_CLEARANCE_OFFSET;
        let drill_z = 2.0_f64;
        assert!((cutting.cuts[0].position.z - clearance_z).abs() < 1e-9);
        assert!((cutting.cuts[1].position.z - drill_z).abs() < 1e-9);
        assert!((cutting.cuts[2].position.z - clearance_z).abs() < 1e-9);
    }

    #[test]
    fn drill_peck_hole() {
        // stock height=10 → stock_top_z=10, clearance_z=15, drill_z=10-9=1
        // peck=3.0: pecks at z=7, z=4, z=1 (3 pairs)
        let stock = make_stock(10.0);
        let params = DrillParams {
            depth: 9.0,
            points: vec![point(0.0, 0.0)],
            peck_depth: Some(3.0),
        };
        let passes = drill_passes(&stock, &params).expect("should succeed");
        assert_eq!(passes.len(), 2);

        let cutting = &passes[1];
        assert_eq!(cutting.kind, PassKind::Cutting);

        // 1 approach rapid + 3 pecks × 2 (feed+rapid) = 7 cut points
        assert_eq!(
            cutting.cuts.len(),
            7,
            "expected 1 approach + 3 peck pairs = 7, got {}",
            cutting.cuts.len()
        );

        // approach
        assert_eq!(cutting.cuts[0].move_kind, MoveKind::Rapid);
        // peck 1: feed to z=7, rapid to clearance
        assert_eq!(cutting.cuts[1].move_kind, MoveKind::Feed);
        assert!((cutting.cuts[1].position.z - 7.0).abs() < 1e-9);
        assert_eq!(cutting.cuts[2].move_kind, MoveKind::Rapid);
        // peck 2: feed to z=4
        assert_eq!(cutting.cuts[3].move_kind, MoveKind::Feed);
        assert!((cutting.cuts[3].position.z - 4.0).abs() < 1e-9);
        assert_eq!(cutting.cuts[4].move_kind, MoveKind::Rapid);
        // peck 3: feed to z=1
        assert_eq!(cutting.cuts[5].move_kind, MoveKind::Feed);
        assert!((cutting.cuts[5].position.z - 1.0).abs() < 1e-9);
        assert_eq!(cutting.cuts[6].move_kind, MoveKind::Rapid);
    }

    #[test]
    fn test_sort_single() {
        let stock = make_stock(10.0);
        let params = DrillParams {
            depth: 5.0,
            points: vec![point(7.0, 3.0)],
            peck_depth: None,
        };
        let passes = drill_passes(&stock, &params).expect("should succeed");
        // One hole → one linking + one cutting pass; position unchanged
        assert_eq!(passes.len(), 2);
        assert!((passes[1].cuts[0].position.x - 7.0).abs() < 1e-9);
        assert!((passes[1].cuts[0].position.y - 3.0).abs() < 1e-9);
    }

    #[test]
    fn test_sort_grid() {
        // Collinear points in an order that differs from nearest-neighbor.
        // Input: (0,0), (40,0), (10,0), (30,0)
        // Expected NN tour from (0,0): (0,0)→(10,0)→(30,0)→(40,0)
        let stock = make_stock(10.0);
        let params = DrillParams {
            depth: 5.0,
            points: vec![
                point(0.0, 0.0),
                point(40.0, 0.0),
                point(10.0, 0.0),
                point(30.0, 0.0),
            ],
            peck_depth: None,
        };
        let passes = drill_passes(&stock, &params).expect("should succeed");
        // 4 holes × 2 passes each = 8 passes
        assert_eq!(passes.len(), 8);
        // Extract X positions from the approach cut of each cutting pass
        let xs: Vec<f64> = passes
            .iter()
            .filter(|p| p.kind == PassKind::Cutting)
            .map(|p| p.cuts[0].position.x)
            .collect();
        assert_eq!(xs, vec![0.0, 10.0, 30.0, 40.0]);
    }

    #[test]
    fn drill_two_holes_order_and_linking() {
        let stock = make_stock(10.0);
        let params = DrillParams {
            depth: 5.0,
            points: vec![point(10.0, 20.0), point(30.0, 40.0)],
            peck_depth: None,
        };
        let passes = drill_passes(&stock, &params).expect("should succeed");
        assert_eq!(passes.len(), 4);

        assert_eq!(passes[0].kind, PassKind::Linking);
        assert_eq!(passes[1].kind, PassKind::Cutting);
        assert_eq!(passes[2].kind, PassKind::Linking);
        assert_eq!(passes[3].kind, PassKind::Cutting);

        // Second linking pass must have exactly 2 cut points
        assert_eq!(
            passes[2].cuts.len(),
            2,
            "second linking pass must have 2 cut points"
        );
        // Both linking cut points should be at clearance height
        let clearance_z = 10.0 + DEFAULT_CLEARANCE_OFFSET;
        for cut in &passes[2].cuts {
            assert!((cut.position.z - clearance_z).abs() < 1e-9);
        }
    }
}
