//! FIELDS: shapes as mathematics rather than as loops.
//!
//! A field is a function `(x, y) -> f64` returning SIGNED DISTANCE in pixels:
//! negative inside the shape, zero on its boundary, positive outside. That one
//! convention is what lets shapes compose as arithmetic on the distance:
//!
//! ```text
//! union(a, b)      min(a, b)        both shapes
//! intersect(a, b)  max(a, b)        only where they overlap
//! subtract(a, b)   max(a, -b)       a with b bitten out of it
//! expand(f, r)     f - r            every point moved r outward
//! outline(f, w)    abs(f) - w/2     the boundary, as a shape in its own right
//! ```
//!
//! This is a direct port of the Python `artlib.fields`. Python evaluates each
//! field over a whole image of positions at once with numpy; here a field is a
//! plain `Fn(f64, f64) -> f64` evaluated per pixel. The arithmetic is identical,
//! so the result is the same — the vectorisation was a speed decision, not part
//! of what a distance function *is*, and native per-pixel closures are already
//! fast enough at pixel-art sizes.
//!
//! Distance also buys ANTIALIASING for free: the rasteriser reads a field's
//! value as coverage (`clamp(0.5 - d, 0, 1)`), so an edge half a pixel inside
//! the boundary comes out half lit. See [`crate::raster::Canvas::paint`].

use std::f64::consts::PI;
use std::sync::Arc;

/// A shape or surface as a function of position: signed distance in pixels.
///
/// `Arc` rather than `Box` so a field can be shared into several composites
/// (a `union` holds its parts, a transform wraps its source) without cloning
/// the closure, and `Send + Sync` so a generator can run one off the UI thread.
pub type Field = Arc<dyn Fn(f64, f64) -> f64 + Send + Sync>;

/// Coordinate fields and scalar-field arithmetic. Shapes are scalar fields in
/// artlib, so these combinators provide a safe, serializable-front-end-friendly
/// replacement for Python callables without embedding another language in a
/// node graph.
pub fn x() -> Field {
    Arc::new(|x, _| x)
}
pub fn y() -> Field {
    Arc::new(|_, y| y)
}
pub fn constant(value: f64) -> Field {
    Arc::new(move |_, _| value)
}
pub fn add(a: Field, b: Field) -> Field {
    Arc::new(move |x, y| a(x, y) + b(x, y))
}
pub fn difference(a: Field, b: Field) -> Field {
    Arc::new(move |x, y| a(x, y) - b(x, y))
}
pub fn multiply(a: Field, b: Field) -> Field {
    Arc::new(move |x, y| a(x, y) * b(x, y))
}
pub fn divide(a: Field, b: Field) -> Field {
    Arc::new(move |x, y| a(x, y) / b(x, y))
}
pub fn minimum(a: Field, b: Field) -> Field {
    Arc::new(move |x, y| a(x, y).min(b(x, y)))
}
pub fn maximum(a: Field, b: Field) -> Field {
    Arc::new(move |x, y| a(x, y).max(b(x, y)))
}
pub fn absolute(a: Field) -> Field {
    Arc::new(move |x, y| a(x, y).abs())
}
pub fn sine(a: Field) -> Field {
    Arc::new(move |x, y| a(x, y).sin())
}
pub fn power(a: Field, exponent: f64) -> Field {
    Arc::new(move |x, y| a(x, y).powf(exponent))
}
pub fn clamp(a: Field, lo: f64, hi: f64) -> Field {
    Arc::new(move |x, y| a(x, y).clamp(lo, hi))
}
pub fn hypot(a: Field, b: Field) -> Field {
    Arc::new(move |x, y| a(x, y).hypot(b(x, y)))
}
pub fn smoothstep(a: Field, edge0: f64, edge1: f64) -> Field {
    Arc::new(move |x, y| {
        let t = ((a(x, y) - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    })
}
pub fn select(condition: Field, if_true: Field, if_false: Field) -> Field {
    Arc::new(move |x, y| {
        if condition(x, y) <= 0.0 {
            if_true(x, y)
        } else {
            if_false(x, y)
        }
    })
}

/// The area below a sampled height line. Samples wrap horizontally, making the
/// result suitable for reusable horizon strips and other serialized profiles.
pub fn height_profile(values: Vec<f64>, crest: f64, foot: f64) -> Field {
    Arc::new(move |x, y| {
        if values.is_empty() {
            return 1.0;
        }
        let index = (x as i64).rem_euclid(values.len() as i64) as usize;
        let top = crest + (1.0 - values[index]) * (foot - crest);
        top - y
    })
}

pub const SQRT2: f64 = std::f64::consts::SQRT_2;
pub const SQRT3: f64 = 1.732_050_807_568_877_2;

// ---------------------------------------------------------------------------
// Primitives. Each returns a field; none of them touch a pixel.
// ---------------------------------------------------------------------------

/// A filled circle.
pub fn disk(cx: f64, cy: f64, r: f64) -> Field {
    Arc::new(move |x, y| (x - cx).hypot(y - cy) - r)
}

/// A filled ellipse.
///
/// The normalized radial equation gives the exact boundary and sign; multiplying
/// by the smaller radius turns it back into pixel-like distance near that
/// boundary, keeping antialiasing stable when one axis is much longer.
pub fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Field {
    assert!(rx > 0.0 && ry > 0.0, "ellipse radii must be positive");
    let scale = rx.min(ry);
    Arc::new(move |x, y| (((x - cx) / rx).hypot((y - cy) / ry) - 1.0) * scale)
}

/// An annulus: the distance to the ring's centreline minus its half-width.
pub fn ring(cx: f64, cy: f64, r_inner: f64, r_outer: f64) -> Field {
    let mid = (r_inner + r_outer) / 2.0;
    let half = (r_outer - r_inner) / 2.0;
    Arc::new(move |x, y| ((x - cx).hypot(y - cy) - mid).abs() - half)
}

/// An axis-aligned rectangle, exact inside and out.
pub fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Field {
    let cx = (x0 + x1) / 2.0;
    let cy = (y0 + y1) / 2.0;
    let hw = (x1 - x0) / 2.0;
    let hh = (y1 - y0) / 2.0;
    Arc::new(move |x, y| {
        let dx = (x - cx).abs() - hw;
        let dy = (y - cy).abs() - hh;
        // Outside: distance to the nearest corner or edge. Inside: the negative
        // distance to the nearest edge, which is what makes `expand` and
        // `outline` behave on a rectangle.
        dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0)
    })
}

/// Everything on the near side of a line: inside where `nx*x + ny*y <= d`.
///
/// The arguments are the plane equation as written — the normal need not be a
/// unit vector — and both it and `d` are divided by the normal's length so the
/// result is a true distance.
pub fn half_plane(nx: f64, ny: f64, d: f64) -> Field {
    let n = {
        let h = nx.hypot(ny);
        if h == 0.0 {
            1.0
        } else {
            h
        }
    };
    let (nx, ny, d) = (nx / n, ny / n, d / n);
    Arc::new(move |x, y| nx * x + ny * y - d)
}

/// A square turned 45 degrees — the Manhattan disc. Divided by root two so the
/// result is a true distance.
pub fn diamond(cx: f64, cy: f64, r: f64) -> Field {
    Arc::new(move |x, y| ((x - cx).abs() + (y - cy).abs() - r) / SQRT2)
}

/// A thick line segment with round caps: distance to the segment, minus the
/// radius.
pub fn capsule(x0: f64, y0: f64, x1: f64, y1: f64, r: f64) -> Field {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let length2 = dx * dx + dy * dy;
    Arc::new(move |x, y| {
        if length2 == 0.0 {
            return (x - x0).hypot(y - y0) - r;
        }
        let t = (((x - x0) * dx + (y - y0) * dy) / length2).clamp(0.0, 1.0);
        (x - (x0 + t * dx)).hypot(y - (y0 + t * dy)) - r
    })
}

/// A thick path through declarative control points: adjoining capsules whose
/// round caps overlap at each vertex, so the union is continuous without special
/// joins.
pub fn polyline(points: &[(f64, f64)], radius: f64) -> Field {
    assert!(!points.is_empty(), "a polyline needs at least one point");
    if points.len() == 1 {
        return disk(points[0].0, points[0].1, radius);
    }
    let segments = points
        .windows(2)
        .map(|w| capsule(w[0].0, w[0].1, w[1].0, w[1].1, radius))
        .collect();
    union(segments)
}

/// A filled simple polygon, with signed distance to its nearest edge. Winding
/// may be either direction.
pub fn polygon(points: &[(f64, f64)]) -> Field {
    assert!(points.len() >= 3, "a polygon needs at least three points");
    // (start, end) for every edge, wrapping the last vertex back to the first.
    let edges: Vec<((f64, f64), (f64, f64))> = points
        .iter()
        .enumerate()
        .map(|(i, &a)| (a, points[(i + 1) % points.len()]))
        .collect();

    Arc::new(move |x, y| {
        let mut distance2 = f64::INFINITY;
        // Crossings counted as a running parity: inside when an odd number of
        // edges lie to one side.
        let mut inside = false;
        for &((ax, ay), (bx, by)) in &edges {
            let ex = bx - ax;
            let ey = by - ay;
            let length2 = ex * ex + ey * ey;
            let t = if length2 == 0.0 {
                0.0
            } else {
                (((x - ax) * ex + (y - ay) * ey) / length2).clamp(0.0, 1.0)
            };
            let dx = x - (ax + ex * t);
            let dy = y - (ay + ey * t);
            distance2 = distance2.min(dx * dx + dy * dy);

            let straddles = (ay > y) != (by > y);
            let crossing = ax + (y - ay) * ex / if ey == 0.0 { 1.0 } else { ey };
            if straddles && x < crossing {
                inside = !inside;
            }
        }
        let distance = distance2.sqrt();
        if inside {
            -distance
        } else {
            distance
        }
    })
}

/// An angular wedge, for cutting gaps in rings.
///
/// A mask rather than a true distance — an angle has no length — so it reports a
/// large constant either side of its edge. Enough for intersecting with a ring,
/// which supplies the real distance.
pub fn sector(cx: f64, cy: f64, degrees_from: f64, degrees_to: f64) -> Field {
    let span = (degrees_to - degrees_from).rem_euclid(360.0);
    Arc::new(move |x, y| {
        let a = ((y - cy).atan2(x - cx).to_degrees() - degrees_from).rem_euclid(360.0);
        if a <= span {
            -1.0
        } else {
            1.0
        }
    })
}

/// A rectangle with its corners cut at 45 degrees, built by intersecting the
/// rectangle with four diagonal half-planes so the result is a real distance.
///
/// `x1`/`y1` are exclusive, so the far edges are at `x1 - 1` and `y1 - 1`.
pub fn chamfered_rect(x0: f64, y0: f64, x1: f64, y1: f64, cut: f64) -> Field {
    let w = x1 - x0;
    let h = y1 - y0;
    intersect(vec![
        rect(x0, y0, x1, y1),
        half_plane(-1.0, -1.0, -(cut + x0 + y0)), // top-left
        half_plane(1.0, -1.0, w - 1.0 - cut + x0 - y0), // top-right
        half_plane(-1.0, 1.0, h - 1.0 - cut - x0 + y0), // bottom-left
        half_plane(1.0, 1.0, w + h - 2.0 - cut + x0 + y0), // bottom-right
    ])
}

/// A regular hexagon, built by intersecting six half-planes so it outlines and
/// insets like any other shape.
///
/// `radius` is the circumradius (centre to corner). Pointy-top by default —
/// a vertex straight up, flat edges left and right; `flat_top` turns it 30°.
pub fn hexagon(cx: f64, cy: f64, radius: f64, flat_top: bool) -> Field {
    let apothem = radius * SQRT3 / 2.0;
    let phase = if flat_top { PI / 6.0 } else { 0.0 };
    let planes = (0..6)
        .map(|k| {
            let a = phase + k as f64 * PI / 3.0;
            half_plane(a.cos(), a.sin(), apothem + a.cos() * cx + a.sin() * cy)
        })
        .collect();
    intersect(planes)
}

// ---------------------------------------------------------------------------
// Algebra.
// ---------------------------------------------------------------------------

/// Both shapes: the pointwise minimum of every field.
pub fn union(fields: Vec<Field>) -> Field {
    assert!(!fields.is_empty(), "union needs at least one field");
    Arc::new(move |x, y| {
        let mut out = fields[0](x, y);
        for g in &fields[1..] {
            out = out.min(g(x, y));
        }
        out
    })
}

/// Only where they overlap: the pointwise maximum of every field.
pub fn intersect(fields: Vec<Field>) -> Field {
    assert!(!fields.is_empty(), "intersect needs at least one field");
    Arc::new(move |x, y| {
        let mut out = fields[0](x, y);
        for g in &fields[1..] {
            out = out.max(g(x, y));
        }
        out
    })
}

/// `field` with every shape in `cut` removed from it.
pub fn subtract(field: Field, cut: Vec<Field>) -> Field {
    assert!(!cut.is_empty(), "subtract needs at least one cutter");
    Arc::new(move |x, y| {
        let mut out = -cut[0](x, y);
        for g in &cut[1..] {
            out = out.max(-g(x, y));
        }
        field(x, y).max(out)
    })
}

/// The complement: inside becomes outside.
pub fn invert(field: Field) -> Field {
    Arc::new(move |x, y| -field(x, y))
}

/// Grow a shape by `r` pixels in every direction. Negative `r` shrinks it.
pub fn expand(field: Field, r: f64) -> Field {
    Arc::new(move |x, y| field(x, y) - r)
}

/// The boundary of a shape, as a shape. `inset` moves the line inward before
/// taking it, so an outline can sit inside the silhouette.
pub fn outline(field: Field, weight: f64, inset: f64) -> Field {
    Arc::new(move |x, y| (field(x, y) + inset).abs() - weight / 2.0)
}

/// The field that is inside at every point, for painting a whole canvas through
/// the same path as everything else.
pub fn everywhere() -> Field {
    Arc::new(|_, _| -1.0)
}

// ---------------------------------------------------------------------------
// Transforms.
// ---------------------------------------------------------------------------

/// Move a shape by `(dx, dy)`.
pub fn translate(field: Field, dx: f64, dy: f64) -> Field {
    Arc::new(move |x, y| field(x - dx, y - dy))
}

/// Rotate a shape `degrees` about `(cx, cy)`.
pub fn rotate(field: Field, degrees: f64, cx: f64, cy: f64) -> Field {
    let rad = (-degrees).to_radians();
    let (cos, sin) = (rad.cos(), rad.sin());
    Arc::new(move |x, y| {
        let px = x - cx;
        let py = y - cy;
        field(cx + px * cos - py * sin, cy + px * sin + py * cos)
    })
}

/// Uniformly scale a field about `(cx, cy)`.
///
/// Sampling the source at the inverse transform gives the right silhouette;
/// multiplying its result by `factor` keeps the return value a distance in
/// destination pixels, so outlines and antialiasing keep their intended weight.
pub fn scale(field: Field, factor: f64, cx: f64, cy: f64) -> Field {
    assert!(factor > 0.0, "scale factor must be positive");
    Arc::new(move |x, y| field(cx + (x - cx) / factor, cy + (y - cy) / factor) * factor)
}

/// The shape and its reflections in both axes: draw one corner, get four.
pub fn mirror4(field: Field, w: f64, h: f64) -> Field {
    let a = field.clone();
    let b = field.clone();
    let c = field.clone();
    let d = field;
    union(vec![
        Arc::new(move |x, y| a(x, y)),
        Arc::new(move |x, y| b(w - 1.0 - x, y)),
        Arc::new(move |x, y| c(x, h - 1.0 - y)),
        Arc::new(move |x, y| d(w - 1.0 - x, h - 1.0 - y)),
    ])
}

/// One shape repeated around a centre: vanes, fins, spokes, bolts, ports.
pub fn polar_array(field: Field, count: usize, cx: f64, cy: f64, phase: f64) -> Field {
    assert!(count > 0, "polar_array needs at least one copy");
    let step = 360.0 / count as f64;
    let copies = (0..count)
        .map(|i| rotate(field.clone(), phase + i as f64 * step, cx, cy))
        .collect();
    union(copies)
}
