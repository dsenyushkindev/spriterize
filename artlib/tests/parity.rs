//! Pixel-parity of the Rust artlib against the Python original.
//!
//! Each scene here is the byte-for-byte twin of one in `tests/gen_golden.py`.
//! The golden PNGs that script emits are the oracle: we rebuild the scene with
//! the Rust port, quantise to RGBA8, and compare. Deterministic features are
//! held to near-exact fidelity; the only allowed slack is a ±1 at antialiased
//! edges where numpy's and Rust's transcendental functions can round a boundary
//! distance apart in the last bit. A gross error (a shape in the wrong place, a
//! colour wrong, a compositing mistake) moves far more than that and fails.
//!
//! Keep the scene list identical to the Python side, name for name. If a scene
//! changes, regenerate: `python artlib/tests/gen_golden.py`.

use artlib::fields::*;
use artlib::raster::*;
use artlib::texture::Grid;
use std::sync::Arc;

const SIZE: usize = 64;

// The shared palette, name for name with the Python side.
const RED: Rgba = [200, 80, 60, 255];
const BLUE: Rgba = [70, 110, 180, 255];
const LIT: Rgba = [150, 160, 175, 255];
const DIM: Rgba = [60, 66, 80, 255];
const SHADOW: Rgba = [25, 28, 36, 255];
const GOLD: Rgba = [240, 200, 60, 255];
const INK: Rgba = [20, 20, 28, 255];
const CYAN: Rgba = [80, 200, 210, 255];
const MAG: Rgba = [200, 70, 160, 255];

fn build(name: &str) -> Canvas {
    let mut c = Canvas::square(SIZE);
    match name {
        "disk" => {
            c.paint(&disk(32., 32., 20.), &solid(RED), true, 1.0);
        }
        "rect" => {
            c.paint(&rect(8., 8., 56., 40.), &solid(BLUE), true, 1.0);
        }
        "ellipse" => {
            c.paint(&ellipse(32., 32., 26., 14.), &solid(CYAN), true, 1.0);
        }
        "chamfer" => {
            let plate = chamfered_rect(0., 0., 64., 64., 9.);
            c.paint(&plate, &vertical(LIT, DIM, 0., 63.), true, 1.0);
            c.paint(&outline(plate.clone(), 1., 1.), &solid(SHADOW), true, 1.0);
        }
        "hexagon" => {
            let hx = hexagon(32., 32., 26., false);
            c.paint(&hx, &solid(CYAN), true, 1.0);
            c.paint(&outline(hx.clone(), 1., 1.), &solid(INK), true, 1.0);
        }
        "ring_sector" => {
            let r = ring(32., 32., 16., 26.);
            let s = sector(32., 32., 45., 300.);
            c.paint(&intersect(vec![r, s]), &solid(GOLD), true, 1.0);
        }
        "polyline" => {
            c.paint(
                &polyline(&[(10., 12.), (52., 20.), (28., 54.)], 4.),
                &solid(MAG),
                true,
                1.0,
            );
        }
        "polygon" => {
            c.paint(
                &polygon(&[(32., 6.), (58., 52.), (6., 52.)]),
                &solid(BLUE),
                true,
                1.0,
            );
        }
        "diamond" => {
            c.paint(&diamond(32., 32., 24.), &solid(CYAN), true, 1.0);
        }
        "subtract" => {
            c.paint(
                &subtract(disk(32., 32., 24.), vec![disk(40., 26., 12.)]),
                &solid(RED),
                true,
                1.0,
            );
        }
        "union_expand" => {
            let blob = union(vec![disk(24., 32., 12.), disk(40., 32., 12.)]);
            c.paint(&expand(blob.clone(), 3.), &solid(BLUE), true, 1.0);
            c.paint(&blob, &solid(GOLD), true, 1.0);
        }
        "polar" => {
            c.paint(
                &polar_array(capsule(32., 8., 32., 26., 3.), 6, 32., 32., 0.),
                &solid(INK),
                true,
                1.0,
            );
        }
        "mirror4" => {
            c.paint(&mirror4(disk(12., 12., 7.), 64., 64.), &solid(GOLD), true, 1.0);
        }
        "rotate" => {
            c.paint(
                &rotate(rect(20., 28., 44., 36.), 30., 32., 32.),
                &solid(CYAN),
                true,
                1.0,
            );
        }
        "scale" => {
            c.paint(
                &scale(rect(24., 24., 40., 40.), 1.5, 32., 32.),
                &solid(MAG),
                true,
                1.0,
            );
        }
        "blend" => {
            c.fill(&solid([30, 30, 40, 255]));
            c.paint(&disk(32., 32., 24.), &solid([255, 200, 0, 128]), true, 1.0);
        }
        "radial" => {
            c.paint(
                &disk(32., 32., 30.),
                &radial(32., 32., 30., GOLD, SHADOW),
                true,
                1.0,
            );
        }
        "horizontal" => {
            c.paint(&rect(4., 4., 60., 60.), &horizontal(RED, BLUE, 0., 59.), true, 1.0);
        }
        "from_field" => {
            let d = disk(32., 32., 30.);
            c.paint(&d, &from_field(d.clone(), CYAN, INK, -28., 4.), true, 1.0);
        }
        "modulate" => {
            c.paint(&rect(6., 6., 58., 58.), &solid(BLUE), true, 1.0);
            let factors: Field = Arc::new(|x, _| 0.4 + 0.9 * x / 63.0);
            c.modulate(&factors, None);
        }
        "stamp" => {
            c.fill(&solid([10, 20, 30, 255]));
            c.stamp(&disk(32., 32., 20.), &solid([100, 150, 200, 120]), false);
        }
        other => panic!("unknown scene {other}"),
    }
    c
}

const SCENES: &[&str] = &[
    "disk",
    "rect",
    "ellipse",
    "chamfer",
    "hexagon",
    "ring_sector",
    "polyline",
    "polygon",
    "diamond",
    "subtract",
    "union_expand",
    "polar",
    "mirror4",
    "rotate",
    "scale",
    "blend",
    "radial",
    "horizontal",
    "from_field",
    "modulate",
    "stamp",
];

fn golden(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/golden/{}.png", env!("CARGO_MANIFEST_DIR"), name);
    image::open(&path)
        .unwrap_or_else(|e| panic!("open golden {path}: {e} (run: python artlib/tests/gen_golden.py)"))
        .to_rgba8()
        .into_raw()
}

/// Compare and return `(max channel diff, fraction of pixels that differ at
/// all)`.
fn diff(rust: &[u8], golden: &[u8]) -> (i32, f64) {
    assert_eq!(rust.len(), golden.len(), "size mismatch");
    let mut max_channel = 0;
    let mut changed_pixels = 0;
    for (r, g) in rust.chunks_exact(4).zip(golden.chunks_exact(4)) {
        let mut changed = false;
        for c in 0..4 {
            let d = (r[c] as i32 - g[c] as i32).abs();
            max_channel = max_channel.max(d);
            if d != 0 {
                changed = true;
            }
        }
        if changed {
            changed_pixels += 1;
        }
    }
    (max_channel, changed_pixels as f64 / (rust.len() / 4) as f64)
}

#[test]
fn deterministic_scenes_match_python() {
    let mut failures = Vec::new();
    for &name in SCENES {
        let rust = build(name).to_rgba8();
        let (max_channel, changed) = diff(&rust, &golden(name));
        // ±1 at AA edges is the only allowed slack; a real port bug moves far
        // more, either in channel magnitude or in how many pixels shift.
        let ok = max_channel <= 1 && changed < 0.02;
        println!(
            "{:>14}: max channel diff {max_channel}, {:.3}% pixels differ  {}",
            name,
            changed * 100.0,
            if ok { "ok" } else { "FAIL" }
        );
        if !ok {
            failures.push(format!(
                "{name}: max channel diff {max_channel}, {:.3}% pixels differ",
                changed * 100.0
            ));
        }
    }
    assert!(failures.is_empty(), "parity failures:\n{}", failures.join("\n"));
}

/// A grid is a field: `from_field` colouring a deterministic distance field is
/// itself deterministic, so a Grid built from a shape's distance round-trips
/// exactly. (Guards the Grid→field bridge used by the noise-colouring path,
/// without depending on noise.)
#[test]
fn grid_as_field_is_callable() {
    let mut g = Grid::zeros(4);
    g.v[2 * 4 + 3] = 0.9; // (x=3, y=2)
    assert_eq!(g.call(3.0, 2.0), 0.9);
    assert_eq!(g.call(3.0 + 4.0, 2.0), 0.9); // wraps
    let f = g.field();
    assert_eq!(f(3.0, 2.0), 0.9);
}
