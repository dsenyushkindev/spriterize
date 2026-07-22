//! Functions to calculate graphics like lines, rectangles, etc. in a discrete
//! 2D space

use std::collections::HashSet;

use crate::{Point, Rect};

/// Get the distance between two [`Point`]s
pub fn distance(p1: Point<i32>, p2: Point<i32>) -> f32 {
    ((((p1.x - p2.x) as i64).pow(2) + ((p1.y - p2.y) as i64).pow(2)) as f64).sqrt() as f32
}

/// Get the set of [`Point`]s needed to draw a line between two points
pub fn line(p1: Point<i32>, p2: Point<i32>) -> Vec<Point<i32>> {
    let mut line = Vec::new();
    let diff = p2 - p1;
    let dist = distance(p1, p2);
    let dx = if dist < 0.1 { 0. } else { diff.x as f32 / dist };
    let dy = if dist < 0.1 { 0. } else { diff.y as f32 / dist };

    for i in 0..=dist.round() as usize {
        let x = (p1.x as f32 + (i as f32 * dx)).round() as i32;
        let y = (p1.y as f32 + (i as f32 * dy)).round() as i32;

        if let Some(Point { x: x0, y: y0 }) = line.last() {
            if x == *x0 && y == *y0 {
                continue;
            }
        }

        line.push((x, y).into());
    }

    line
}

/// Largest brush radius that makes sense to offer. Beyond this a stamp covers
/// most of a typical sprite in one click.
pub const MAX_BRUSH_RADIUS: u8 = 16;

/// Offsets covered by a brush of the given radius, as a filled disc.
///
/// Radius 0 is a single pixel, 1 a plus shape, and so on: a pixel belongs to
/// the stamp when its centre is within `radius` of the middle, which keeps the
/// familiar chunky circles of pixel art rather than a smooth outline. The
/// diameter is always `2 * radius + 1`, so a stamp is symmetric around the
/// pixel under the cursor.
pub fn brush_offsets(radius: u8) -> Vec<Point<i32>> {
    let radius = radius.min(MAX_BRUSH_RADIUS) as i32;
    let limit = radius * radius;
    let mut offsets = Vec::new();

    for y in -radius..=radius {
        for x in -radius..=radius {
            if x * x + y * y <= limit {
                offsets.push(Point::new(x, y));
            }
        }
    }

    offsets
}

/// Moves `p` onto the nearest of the eight directions 45 degrees apart from
/// `p0`: the four axes and the four diagonals.
///
/// The angle of the drag is rounded to the nearest step, then the cursor is
/// projected onto that ray, so the end of the line slides along the direction
/// rather than jumping to a fixed length. A diagonal comes out at exactly 45
/// degrees, which is what makes it a clean staircase in pixels.
pub fn snap_to_direction(p0: Point<i32>, p: Point<i32>) -> Point<i32> {
    const STEP: f32 = std::f32::consts::FRAC_PI_4;

    let (dx, dy) = ((p.x - p0.x) as f32, (p.y - p0.y) as f32);
    let angle = (dy.atan2(dx) / STEP).round() * STEP;
    let (cos, sin) = (angle.cos(), angle.sin());
    // Distance along the chosen ray, never behind its start.
    let reach = (dx * cos + dy * sin).max(0.);

    Point::new(
        p0.x + (reach * cos).round() as i32,
        p0.y + (reach * sin).round() as i32,
    )
}

/// Moves `p` so the box it forms with `p0` is square, keeping the direction of
/// the drag. Turns the rectangle tool into a square and the ellipse into a
/// circle.
pub fn snap_to_square(p0: Point<i32>, p: Point<i32>) -> Point<i32> {
    let (dx, dy) = (p.x - p0.x, p.y - p0.y);
    // The longer side wins, so the shape always covers the drag.
    let side = dx.abs().max(dy.abs());
    // A drag straight along an axis has no sign to preserve on the other one,
    // so it grows right and down.
    let sign = |d: i32| if d < 0 { -1 } else { 1 };

    Point::new(p0.x + side * sign(dx), p0.y + side * sign(dy))
}

/// Widens a set of [`Point`]s by stamping a brush of the given radius over each
/// of them, so a one pixel outline becomes a thick one.
///
/// Duplicates are removed, which matters when the caller records how to undo the
/// change: a pixel set twice would otherwise be reverted to a color from earlier
/// in the same stroke.
pub fn thicken(points: impl IntoIterator<Item = Point<i32>>, radius: u8) -> Vec<Point<i32>> {
    if radius == 0 {
        return points.into_iter().collect();
    }

    let offsets = brush_offsets(radius);
    let mut seen = HashSet::new();
    let mut thickened = Vec::new();

    for point in points {
        for offset in &offsets {
            let p = point + *offset;

            if seen.insert(p) {
                thickened.push(p);
            }
        }
    }

    thickened
}

/// Get the set of [`Point`]s needed to draw a rectangle between two points
pub fn rectangle(p1: Point<i32>, p2: Point<i32>) -> Vec<Point<i32>> {
    let l1 = line((p1.x, p1.y).into(), (p1.x, p2.y).into());
    let l2 = line((p1.x, p1.y).into(), (p2.x, p1.y).into());
    let l3 = line((p2.x, p1.y).into(), (p2.x, p2.y).into());
    let l4 = line((p1.x, p2.y).into(), (p2.x, p2.y).into());

    vec![l1, l2, l3, l4].into_iter().flatten().collect()
}

/// Get the set of [`Point`]s needed to draw an ellipse between two points
/// TODO there are still some imperfections here
pub fn ellipse(p1: Point<i32>, p2: Point<i32>) -> Vec<Point<i32>> {
    let a = (p1.x - p2.x).abs() as f32 / 2.0;
    let b = (p1.y - p2.y).abs() as f32 / 2.0;

    let low_x = std::cmp::min(p1.x, p2.x);
    let low_y = std::cmp::min(p1.y, p2.y);
    let high_x = std::cmp::max(p1.x, p2.x);
    let high_y = std::cmp::max(p1.y, p2.y);
    let bounds = Rect::new(low_x, low_y, high_x - low_x, high_y - low_y);
    let xspan = ((p1.x - p2.x).abs() as f32 / 2.0).round() as i32;
    let yspan = ((p1.y - p2.y).abs() as f32 / 2.0).round() as i32;

    let mut points = HashSet::new();

    let sampling_level = yspan;
    let step = 1. / sampling_level as f32;

    // For each x, we'll find the corresponding y values
    for x in 0..(xspan) {
        for delta in 0..sampling_level {
            let x = x as f32 + (delta as f32 * step);
            // Formula:
            // sqrt(((a2-x2)*b2)/a2)
            let inner = (a.powf(2.0) - x.powf(2.0)) * b.powf(2.0) / a.powf(2.0);
            let root = inner.sqrt();

            let ys = vec![root, -root];

            for y in ys {
                let xx = x.round() as i32 + low_x + xspan;
                let yy = y.round() as i32 + low_y + yspan;

                if bounds.contains(xx, yy) {
                    points.insert(Point::new(xx, yy));
                }

                let xx = -x.round() as i32 + low_x + xspan;
                let yy = y.round() as i32 + low_y + yspan;

                if bounds.contains(xx, yy) {
                    points.insert(Point::new(xx, yy));
                }
            }
        }
    }

    points.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case((0,0), (2,0), vec![(0, 0), (1, 0), (2, 0)])]
    #[test_case((0,0), (0,2), vec![(0, 0), (0, 1), (0, 2)])]
    #[test_case((0,0), (2,2), vec![(0, 0), (1, 1), (2, 2)])]
    #[test_case((2,0), (0,0), vec![(0, 0), (1, 0), (2, 0)])]
    #[test_case((0,0), (3,1), vec![(0, 0), (1, 0), (2, 1), (3, 1)])]
    fn simple_line_cases(p1: (i32, i32), p2: (i32, i32), expected: Vec<(i32, i32)>) {
        let mut l = line(p1.into(), p2.into());
        l.sort();

        assert_eq!(l, expected.into_iter().map(Into::into).collect::<Vec<_>>());
    }

    #[test]
    fn a_zero_radius_brush_is_a_single_pixel() {
        assert_eq!(brush_offsets(0), vec![Point::new(0, 0)]);
    }

    #[test]
    fn a_radius_one_brush_is_a_plus() {
        let mut offsets = brush_offsets(1);
        offsets.sort();

        let mut expected: Vec<Point<i32>> = vec![(0, -1), (-1, 0), (0, 0), (1, 0), (0, 1)]
            .into_iter()
            .map(Into::into)
            .collect();
        expected.sort();

        assert_eq!(offsets, expected);
    }

    #[test]
    fn brushes_are_round_and_symmetric() {
        for radius in 0..=6 {
            let offsets = brush_offsets(radius);
            let r = radius as i32;

            // Every offset is within the radius, and the disc is symmetric
            // under reflection in both axes.
            for p in &offsets {
                assert!(p.x * p.x + p.y * p.y <= r * r);
                assert!(offsets.contains(&Point::new(-p.x, p.y)));
                assert!(offsets.contains(&Point::new(p.x, -p.y)));
            }

            // Corners of the bounding box are cut off for anything but a
            // single pixel, so the stamp reads as a circle rather than a box.
            if radius > 0 {
                assert!(!offsets.contains(&Point::new(r, r)));
            }
        }
    }

    #[test_case((10, 10), (20, 11) => (20, 10) ; "shallow drag flattens to horizontal")]
    #[test_case((10, 10), (11, 20) => (10, 20) ; "steep drag straightens to vertical")]
    #[test_case((10, 10), (0, 11) => (0, 10) ; "horizontal works leftwards")]
    #[test_case((10, 10), (11, 0) => (10, 0) ; "vertical works upwards")]
    // Kept off a half pixel projection, where rounding could fall either way.
    #[test_case((10, 10), (20, 16) => (18, 18) ; "near diagonal snaps to exactly 45 degrees")]
    #[test_case((10, 10), (0, 20) => (0, 20) ; "diagonals work in every quadrant")]
    #[test_case((10, 10), (10, 10) => (10, 10) ; "no drag stays put")]
    fn snapped_direction(p0: (i32, i32), p: (i32, i32)) -> (i32, i32) {
        let snapped = snap_to_direction(p0.into(), p.into());

        (snapped.x, snapped.y)
    }

    #[test]
    fn snapped_diagonals_are_exactly_45_degrees() {
        let p0 = Point::new(8, 8);

        for x in -20..=20 {
            for y in -20..=20 {
                let snapped = snap_to_direction(p0, Point::new(p0.x + x, p0.y + y));
                let (dx, dy) = ((snapped.x - p0.x).abs(), (snapped.y - p0.y).abs());

                // Every result lies on an axis or on a perfect diagonal.
                assert!(
                    dx == 0 || dy == 0 || dx == dy,
                    "({x}, {y}) snapped to ({dx}, {dy}), which is neither"
                );
            }
        }
    }

    #[test_case((5, 5), (15, 8) => (15, 15) ; "grows the short side to match")]
    #[test_case((5, 5), (8, 15) => (15, 15) ; "works whichever side is longer")]
    #[test_case((5, 5), (0, 3) => (0, 0) ; "keeps the direction of the drag")]
    #[test_case((5, 5), (2, 9) => (1, 9) ; "handles mixed directions")]
    #[test_case((5, 5), (5, 12) => (12, 12) ; "an axis drag grows right and down")]
    #[test_case((5, 5), (5, 5) => (5, 5) ; "no drag stays put")]
    fn snapped_square(p0: (i32, i32), p: (i32, i32)) -> (i32, i32) {
        let snapped = snap_to_square(p0.into(), p.into());

        (snapped.x, snapped.y)
    }

    #[test]
    fn snapped_squares_have_equal_sides() {
        let p0 = Point::new(9, 9);

        for x in -15..=15 {
            for y in -15..=15 {
                let snapped = snap_to_square(p0, Point::new(p0.x + x, p0.y + y));

                assert_eq!(
                    (snapped.x - p0.x).abs(),
                    (snapped.y - p0.y).abs(),
                    "({x}, {y}) did not come out square"
                );
            }
        }
    }

    #[test]
    fn brush_radius_is_capped() {
        let huge = brush_offsets(u8::MAX);
        let capped = brush_offsets(MAX_BRUSH_RADIUS);

        assert_eq!(huge.len(), capped.len());
    }

    #[test]
    fn odd_lines() {
        let p1 = (0, 0);
        let p2 = (2, 1);
        let expect = vec![(0, 0), (2, 1)];
        let either = vec![(1, 0), (1, 1)];
        let l = line(p1.into(), p2.into());

        for expected in expect {
            assert!(l.contains(&expected.into()));
        }

        assert!(l.contains(&either[0].into()) || l.contains(&either[1].into()));
    }
}
