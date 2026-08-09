//! Properties the noise sources must hold.
//!
//! Noise is exempt from byte-parity with Python (see `src/prng.rs`): a seed
//! gives a stable texture of the same *character*, not the same bytes. So it is
//! checked here for what actually matters — determinism, seed sensitivity,
//! staying in range, and tiling — rather than against a golden image.

use artlib::texture::{
    constant, fbm, perlin, ridged, stripes, value_noise, worley, worley_with, Feature, Grid,
};

const N: usize = 64;

fn range(g: &Grid) -> (f64, f64) {
    let lo = g.v.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = g.v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    (lo, hi)
}

fn max_abs_diff(a: &Grid, b: &Grid) -> f64 {
    a.v.iter()
        .zip(&b.v)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
}

#[test]
fn every_source_is_deterministic() {
    assert_eq!(value_noise(N, 4, 7).v, value_noise(N, 4, 7).v);
    assert_eq!(perlin(N, 4, 7).v, perlin(N, 4, 7).v);
    assert_eq!(worley(N, 5, 7).v, worley(N, 5, 7).v);
    assert_eq!(
        fbm(N, 7, 4, 4, perlin, 0.5).v,
        fbm(N, 7, 4, 4, perlin, 0.5).v
    );
    assert_eq!(ridged(N, 7, 3, 4, perlin).v, ridged(N, 7, 3, 4, perlin).v);
    assert_eq!(stripes(N, 3, 1, 0.0).v, stripes(N, 3, 1, 0.0).v);
}

#[test]
fn different_seeds_give_different_noise() {
    assert!(max_abs_diff(&perlin(N, 4, 7), &perlin(N, 4, 8)) > 0.05);
    assert!(max_abs_diff(&value_noise(N, 4, 7), &value_noise(N, 4, 8)) > 0.05);
    assert!(max_abs_diff(&worley(N, 5, 7), &worley(N, 5, 8)) > 0.05);
}

#[test]
fn normalized_sources_fill_the_unit_interval() {
    // worley, ridged and fbm all end in `.normalize()`, so min is 0 and max 1.
    for g in [
        worley(N, 5, 3),
        worley_with(N, 5, 3, Feature::F2F1, 1.0),
        ridged(N, 3, 3, 4, perlin),
    ] {
        let (lo, hi) = range(&g);
        assert!(lo.abs() < 1e-9, "min {lo} not 0");
        assert!((hi - 1.0).abs() < 1e-9, "max {hi} not 1");
    }
}

#[test]
fn unnormalized_sources_stay_in_range() {
    // value_noise interpolates lattice values in [0,1); perlin is centred on
    // 0.5 and stripes is a shifted sine — all comfortably within [0,1].
    for g in [value_noise(N, 4, 1), perlin(N, 4, 1), stripes(N, 2, 1, 0.0)] {
        let (lo, hi) = range(&g);
        assert!(
            lo >= -1e-9 && hi <= 1.0 + 1e-9,
            "range {lo}..{hi} escapes [0,1]"
        );
    }
}

#[test]
fn worley_features_differ() {
    // f1 is cell interiors; f2f1 is the boundaries between cells — they must not
    // be the same surface.
    let f1 = worley_with(N, 5, 4, Feature::F1, 1.0);
    let f2f1 = worley_with(N, 5, 4, Feature::F2F1, 1.0);
    assert!(max_abs_diff(&f1, &f2f1) > 0.2);
}

#[test]
fn stripes_tile_at_whole_cycles() {
    // Two cycles across 64 px repeat every 32; the surface must match one period
    // apart, exactly.
    let s = stripes(N, 2, 0, 0.0);
    for y in 0..N {
        for x in 0..(N - 32) {
            let a = s.v[y * N + x];
            let b = s.v[y * N + x + 32];
            assert!((a - b).abs() < 1e-12, "seam at ({x},{y}): {a} vs {b}");
        }
    }
}

#[test]
fn constant_is_flat() {
    let g = constant(N, 0.3);
    assert!(g.v.iter().all(|&v| v == 0.3));
}

#[test]
fn grid_arithmetic_matches_elementwise() {
    let a = constant(4, 0.2);
    let b = constant(4, 0.5);
    assert!((a.clone() + b.clone())
        .v
        .iter()
        .all(|&v| (v - 0.7).abs() < 1e-12));
    assert!((a * 3.0).v.iter().all(|&v| (v - 0.6).abs() < 1e-12));
}

#[test]
fn normalize_then_quantize_bands() {
    let g = perlin(N, 4, 2).normalize().quantize(4.0);
    // Snapped to floor(v*4)/4, so every value is one of 0, .25, .5, .75, 1.
    for &v in &g.v {
        let bucket = (v * 4.0).round() / 4.0;
        assert!((v - bucket).abs() < 1e-9, "{v} not on a quarter step");
    }
}

#[test]
fn relief_returns_brightness_near_one() {
    // Lighting a smooth height map gives multipliers bounded by ambient below
    // and ambient + 2*(1-ambient) above.
    let height = fbm(N, 5, 4, 4, perlin, 0.5);
    let lit = height.relief(135.0, 2.0, 0.55);
    let (lo, hi) = range(&lit);
    assert!(lo >= 0.55 - 1e-9, "relief dipped below ambient: {lo}");
    assert!(hi <= 0.55 + 2.0 * 0.45 + 1e-9, "relief too bright: {hi}");
}
