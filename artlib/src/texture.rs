//! TEXTURE: surfaces as arithmetic on grids.
//!
//! [`fields`](crate::fields) answers "what SHAPE is this"; this answers "what is
//! it MADE of". A surface has no boundary to be inside or outside of — it is a
//! value at every point — so the unit here is a [`Grid`]: one float per pixel,
//! with arithmetic. A `Grid` is also a field (it is callable at a position), so
//! it can be coloured by [`from_field`](crate::raster::from_field) or turned
//! into a shape with [`Grid::mask`], and the two halves of artlib meet there.
//!
//! EVERYTHING TILES. Each source wraps its lattice, `warp` samples with wrap and
//! `relief` takes its differences with wrap, so tiling survives composition.
//!
//! A port of the Python `artlib.texture`. The noise here reproduces the Python
//! *algorithms* — value, gradient, cellular, fbm, ridged, stripes — but draws
//! its lattices from [`Prng`](crate::Prng) rather than replicating CPython's
//! random stream; a seed gives a stable texture of the same character, not the
//! same bytes.

use crate::fields::Field;
use crate::prng::Prng;
use std::f64::consts::TAU;
use std::sync::Arc;

fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

fn smoother(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// The signature every source shares, so the whole vocabulary layers through
/// [`fbm`] and [`ridged`].
pub type NoiseSource = fn(usize, usize, u64) -> Grid;

/// A float per pixel, with arithmetic. Stored row-major: index `y * size + x`.
///
/// Every operation returns a NEW grid: a surface is built by naming intermediate
/// steps, and mutating in place makes those names lie.
#[derive(Debug, Clone)]
pub struct Grid {
    pub size: usize,
    pub v: Vec<f64>,
}

impl Grid {
    /// A grid of zeros.
    pub fn zeros(size: usize) -> Self {
        Self {
            size,
            v: vec![0.0; size * size],
        }
    }

    /// A grid from `size * size` values, row-major.
    pub fn from_vec(size: usize, v: Vec<f64>) -> Self {
        assert_eq!(v.len(), size * size, "grid values must be size * size");
        Self { size, v }
    }

    // -- as a field --------------------------------------------------------

    /// Nearest sample with wrap: what makes a Grid a field. Positions are
    /// truncated toward zero to an integer cell, then wrapped, matching numpy's
    /// `astype(int64) % size`.
    pub fn call(&self, x: f64, y: f64) -> f64 {
        let s = self.size as i64;
        let xi = (x as i64).rem_euclid(s);
        let yi = (y as i64).rem_euclid(s);
        self.v[(yi * s + xi) as usize]
    }

    /// Bilinear sample with wrap, for reading at a fractional position — what a
    /// domain warp does on every pixel.
    pub fn at(&self, x: f64, y: f64) -> f64 {
        let s = self.size as i64;
        let x0f = x.floor();
        let y0f = y.floor();
        let fx = x - x0f;
        let fy = y - y0f;
        let x0 = (x0f as i64).rem_euclid(s);
        let y0 = (y0f as i64).rem_euclid(s);
        let x1 = (x0 + 1).rem_euclid(s);
        let y1 = (y0 + 1).rem_euclid(s);
        let at = |yy: i64, xx: i64| self.v[(yy * s + xx) as usize];
        let top = at(y0, x0) + (at(y0, x1) - at(y0, x0)) * fx;
        let bot = at(y1, x0) + (at(y1, x1) - at(y1, x0)) * fx;
        top + (bot - top) * fy
    }

    /// This grid as a field, so a surface can be handed anywhere a shape can.
    pub fn field(&self) -> Field {
        let g = self.clone();
        Arc::new(move |x, y| g.call(x, y))
    }

    // -- reshaping the VALUES ----------------------------------------------

    pub fn clamp(&self, lo: f64, hi: f64) -> Grid {
        Grid::from_vec(self.size, self.v.iter().map(|x| x.clamp(lo, hi)).collect())
    }

    /// Stretch to fill `0..1`. Worth doing before any threshold, since worley
    /// and ridged noise do not naturally span the range.
    pub fn normalize(&self) -> Grid {
        let lo = self.v.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = self.v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let span = if hi - lo == 0.0 { 1.0 } else { hi - lo };
        Grid::from_vec(self.size, self.v.iter().map(|x| (x - lo) / span).collect())
    }

    /// Push values toward one end. `power > 1` darkens the middle (fewer,
    /// sharper features); `power < 1` opens it up.
    pub fn gain(&self, power: f64) -> Grid {
        Grid::from_vec(
            self.size,
            self.v
                .iter()
                .map(|&x| if x > 0.0 { x.powf(power) } else { 0.0 })
                .collect(),
        )
    }

    pub fn remap(&self, lo: f64, hi: f64) -> Grid {
        Grid::from_vec(
            self.size,
            self.v.iter().map(|x| lo + (hi - lo) * x).collect(),
        )
    }

    /// Snap to discrete levels — sedimentary banding.
    pub fn quantize(&self, steps: f64) -> Grid {
        Grid::from_vec(
            self.size,
            self.v.iter().map(|x| (x * steps).floor() / steps).collect(),
        )
    }

    pub fn lerp(&self, other: &Grid, t: f64) -> Grid {
        assert_eq!(self.size, other.size, "lerp needs matching sizes");
        Grid::from_vec(
            self.size,
            self.v
                .iter()
                .zip(&other.v)
                .map(|(a, b)| a + (b - a) * t)
                .collect(),
        )
    }

    // -- frequency ---------------------------------------------------------

    /// Box blur, separable and WRAPPED. On its own it softens; its real use is
    /// as the other half of [`highpass`](Grid::highpass).
    pub fn blur(&self, radius: i64, passes: u32) -> Grid {
        let s = self.size;
        let n = (2 * radius + 1) as f64;
        let mut v = self.v.clone();
        for _ in 0..passes {
            let mut tmp = vec![0.0; s * s];
            for y in 0..s {
                for x in 0..s {
                    let mut acc = 0.0;
                    for k in -radius..=radius {
                        let xx = (x as i64 + k).rem_euclid(s as i64) as usize;
                        acc += v[y * s + xx];
                    }
                    tmp[y * s + x] = acc / n;
                }
            }
            let mut out = vec![0.0; s * s];
            for y in 0..s {
                for x in 0..s {
                    let mut acc = 0.0;
                    for k in -radius..=radius {
                        let yy = (y as i64 + k).rem_euclid(s as i64) as usize;
                        acc += tmp[yy * s + x];
                    }
                    out[y * s + x] = acc / n;
                }
            }
            v = out;
        }
        Grid::from_vec(s, v)
    }

    /// Keep the fine detail, throw away the broad shape — which is what hides a
    /// repeat, since the eye tracks only low frequencies. Centred on `0.5` so
    /// the result is still a `0..1` surface.
    pub fn highpass(&self, radius: i64) -> Grid {
        let soft = self.blur(radius, 1);
        Grid::from_vec(
            self.size,
            self.v
                .iter()
                .zip(&soft.v)
                .map(|(a, b)| a - b + 0.5)
                .collect(),
        )
    }

    // -- turning a surface into a shape ------------------------------------

    /// The band `lo..hi` as a FIELD, so it can be painted like any other shape.
    /// Returns signed distance in value units (negative inside the band);
    /// `softness` widens the transition.
    pub fn mask(&self, lo: f64, hi: f64, softness: f64) -> Field {
        let mid = (lo + hi) / 2.0;
        let half = (hi - lo) / 2.0;
        let scale = if softness > 0.0 { 1.0 / softness } else { 1e6 };
        let g = self.clone();
        Arc::new(move |x, y| ((g.call(x, y) - mid).abs() - half) * scale)
    }

    // -- the three that matter ---------------------------------------------

    /// Sample this grid at a position pushed by two other grids: bland noise
    /// becomes flow, strata and erosion. Offsets are centred on zero and
    /// sampling wraps, so the result still tiles.
    pub fn warp(&self, dx: &Grid, dy: &Grid, amount: f64) -> Grid {
        let s = self.size;
        let mut out = vec![0.0; s * s];
        for (idx, slot) in out.iter_mut().enumerate() {
            let x = (idx % s) as f64;
            let y = (idx / s) as f64;
            let ox = (dx.call(x, y) - 0.5) * 2.0 * amount;
            let oy = (dy.call(x, y) - 0.5) * 2.0 * amount;
            *slot = self.at(x + ox, y + oy);
        }
        Grid::from_vec(s, out)
    }

    /// Light this grid as a height map; returns brightness MULTIPLIERS near 1,
    /// for [`Canvas::modulate`](crate::raster::Canvas::modulate). Differences are
    /// taken with WRAP, so a lit texture tiles as well as the height it came
    /// from.
    pub fn relief(&self, azimuth: f64, strength: f64, ambient: f64) -> Grid {
        let s = self.size;
        let rad = azimuth.to_radians();
        let (mut lx, mut ly, mut lz) = (rad.cos(), rad.sin(), 0.85);
        let ln = (lx * lx + ly * ly + lz * lz).sqrt();
        lx /= ln;
        ly /= ln;
        lz /= ln;

        let g = |y: usize, x: usize| self.v[y * s + x];
        let mut out = vec![0.0; s * s];
        for y in 0..s {
            for x in 0..s {
                let xr = g(y, (x + 1) % s);
                let xl = g(y, (x + s - 1) % s);
                let yd = g((y + 1) % s, x);
                let yu = g((y + s - 1) % s, x);
                let gx = (xr - xl) * strength;
                let gy = (yd - yu) * strength;
                let n = (gx * gx + gy * gy + 1.0).sqrt();
                let lam = (-gx * lx - gy * ly + lz) / n;
                out[y * s + x] = ambient + (1.0 - ambient) * lam.max(0.0) * 2.0;
            }
        }
        Grid::from_vec(s, out)
    }
}

impl std::ops::Mul<f64> for Grid {
    type Output = Grid;
    fn mul(self, rhs: f64) -> Grid {
        Grid::from_vec(self.size, self.v.iter().map(|x| x * rhs).collect())
    }
}

impl std::ops::Add<Grid> for Grid {
    type Output = Grid;
    fn add(self, rhs: Grid) -> Grid {
        assert_eq!(self.size, rhs.size, "grid add needs matching sizes");
        Grid::from_vec(
            self.size,
            self.v.iter().zip(&rhs.v).map(|(a, b)| a + b).collect(),
        )
    }
}

// ---------------------------------------------------------------------------
// Sources. Every one tiles over `size`.
// ---------------------------------------------------------------------------

/// For each axis position, the two lattice cells straddling it and the fraction
/// between — precomputed once and shared by both axes, as numpy broadcasts them.
fn lattice_axes(
    size: usize,
    period: usize,
    smoothing: fn(f64) -> f64,
) -> (Vec<usize>, Vec<usize>, Vec<f64>) {
    let step = size as f64 / period as f64;
    let mut i0 = vec![0usize; size];
    let mut i1 = vec![0usize; size];
    let mut frac = vec![0.0; size];
    for a in 0..size {
        let axis = a as f64 / step;
        let cell = axis as i64; // trunc toward zero; axis >= 0
        let c0 = cell.rem_euclid(period as i64) as usize;
        i0[a] = c0;
        i1[a] = (c0 + 1) % period; // wraps: the whole trick
        frac[a] = smoothing(axis - cell as f64);
    }
    (i0, i1, frac)
}

/// Smooth noise on a lattice of random VALUES. Soft and blobby — the cheapest
/// form.
pub fn value_noise(size: usize, period: usize, seed: u64) -> Grid {
    let mut rng = Prng::new(seed);
    // The lattice is drawn one value at a time, row-major — that order is what
    // makes a texture the same texture.
    let mut lat = vec![vec![0.0; period]; period];
    for row in lat.iter_mut() {
        for cell in row.iter_mut() {
            *cell = rng.random();
        }
    }
    let (i0, i1, frac) = lattice_axes(size, period, smooth);

    let mut v = vec![0.0; size * size];
    for row in 0..size {
        let (y0, y1, fy) = (i0[row], i1[row], frac[row]);
        for col in 0..size {
            let (x0, x1, fx) = (i0[col], i1[col], frac[col]);
            let top = lat[y0][x0] + (lat[y0][x1] - lat[y0][x0]) * fx;
            let bot = lat[y1][x0] + (lat[y1][x1] - lat[y1][x0]) * fx;
            v[row * size + col] = top + (bot - top) * fy;
        }
    }
    Grid::from_vec(size, v)
}

/// Gradient noise: a lattice of random DIRECTIONS rather than values. Zero on
/// the lattice points and peaking between, so it reads as terrain rather than as
/// a quilt.
// `0.7071` is the rounded literal the Python original uses to bring 2D gradient
// noise back to roughly 0..1; it is NOT `FRAC_1_SQRT_2` (0.70710678…), and using
// the exact constant would break byte-parity with the golden images.
#[allow(clippy::approx_constant)]
pub fn perlin(size: usize, period: usize, seed: u64) -> Grid {
    let mut rng = Prng::new(seed);
    let mut gx = vec![0.0; period * period];
    let mut gy = vec![0.0; period * period];
    for i in 0..period * period {
        let a = rng.random() * TAU;
        gx[i] = a.cos();
        gy[i] = a.sin();
    }

    // Raw fractions (for the dot products) and their smoother-curved versions
    // (for the interpolation weights).
    let step = size as f64 / period as f64;
    let mut i0 = vec![0usize; size];
    let mut i1 = vec![0usize; size];
    let mut raw = vec![0.0; size];
    for a in 0..size {
        let axis = a as f64 / step;
        let cell = axis as i64;
        let c0 = cell.rem_euclid(period as i64) as usize;
        i0[a] = c0;
        i1[a] = (c0 + 1) % period;
        raw[a] = axis - cell as f64;
    }

    let dot = |row: usize, col: usize, ox: f64, oy: f64, tx: f64, ty: f64| {
        let at = row * period + col;
        gx[at] * (tx - ox) + gy[at] * (ty - oy)
    };

    let mut v = vec![0.0; size * size];
    for row in 0..size {
        let (y0, y1, ty) = (i0[row], i1[row], raw[row]);
        let fy = smoother(ty);
        for col in 0..size {
            let (x0, x1, tx) = (i0[col], i1[col], raw[col]);
            let fx = smoother(tx);
            let n00 = dot(y0, x0, 0.0, 0.0, tx, ty);
            let n10 = dot(y0, x1, 1.0, 0.0, tx, ty);
            let n01 = dot(y1, x0, 0.0, 1.0, tx, ty);
            let n11 = dot(y1, x1, 1.0, 1.0, tx, ty);
            let top = n00 + (n10 - n00) * fx;
            let bot = n01 + (n11 - n01) * fx;
            v[row * size + col] = (top + (bot - top) * fy) * 0.7071 + 0.5;
        }
    }
    Grid::from_vec(size, v)
}

/// Which distance a [`worley`] cell measures, and so the whole character of the
/// result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    /// Distance to the NEAREST point. Blobs, pebbles, cell interiors.
    F1,
    /// Distance to the second nearest.
    F2,
    /// The gap between them — near zero on the BOUNDARY between two points, a
    /// crack network. Nothing else here can draw an edge.
    F2F1,
}

/// Cellular noise with an explicit feature and jitter. [`worley`] is the
/// `Feature::F1`, full-jitter shorthand that matches the [`NoiseSource`]
/// signature.
pub fn worley_with(size: usize, period: usize, seed: u64, feature: Feature, jitter: f64) -> Grid {
    let mut rng = Prng::new(seed);
    let step = size as f64 / period as f64;
    let mut px = vec![0.0; period * period];
    let mut py = vec![0.0; period * period];
    for cy in 0..period {
        for cx in 0..period {
            let jx = 0.5 + (rng.random() - 0.5) * jitter;
            let jy = 0.5 + (rng.random() - 0.5) * jitter;
            let at = cy * period + cx;
            px[at] = (cx as f64 + jx) * step;
            py[at] = (cy as f64 + jy) * step;
        }
    }

    let per = period as i64;
    let mut out = vec![0.0; size * size];
    for (idx, slot) in out.iter_mut().enumerate() {
        let x = (idx % size) as f64;
        let y = (idx / size) as f64;
        let cx = (x / step) as i64;
        let cy = (y / step) as i64;
        let mut best = 1e9;
        let mut second = 1e9;
        for oy in -1..=1 {
            let n_y = cy + oy;
            for ox in -1..=1 {
                let n_x = cx + ox;
                let at = (n_y.rem_euclid(per) * per + n_x.rem_euclid(per)) as usize;
                // A neighbour off the edge is the point from the opposite side,
                // measured at its near copy — shift it a full width out. This is
                // the line that makes worley tile.
                let mut ppx = px[at];
                let mut ppy = py[at];
                if n_x < 0 {
                    ppx -= size as f64;
                }
                if n_x >= per {
                    ppx += size as f64;
                }
                if n_y < 0 {
                    ppy -= size as f64;
                }
                if n_y >= per {
                    ppy += size as f64;
                }
                let d = (x - ppx).hypot(y - ppy);
                // Nearest and runner-up, updated together and in that order.
                if d < best {
                    second = best;
                    best = d;
                } else if d < second {
                    second = d;
                }
            }
        }
        *slot = match feature {
            Feature::F2F1 => second - best,
            Feature::F2 => second,
            Feature::F1 => best,
        };
    }
    Grid::from_vec(size, out).normalize()
}

/// Cellular noise: distance to the nearest scattered feature point, one per
/// lattice cell. The [`NoiseSource`]-shaped default; use [`worley_with`] for
/// `f2`/`f2f1` cracks or reduced jitter.
pub fn worley(size: usize, period: usize, seed: u64) -> Grid {
    worley_with(size, period, seed, Feature::F1, 1.0)
}

/// Layered noise: each octave twice as fine and `falloff` as loud. Form, then
/// grain on it. `source` is any [`NoiseSource`], so the whole vocabulary layers.
pub fn fbm(
    size: usize,
    seed: u64,
    octaves: u32,
    period: usize,
    source: NoiseSource,
    falloff: f64,
) -> Grid {
    let mut total = Grid::zeros(size);
    let mut amp = 1.0;
    let mut norm = 0.0;
    let mut per = period;
    for o in 0..octaves {
        total = total + source(size, per, seed + o as u64 * 977) * amp;
        norm += amp;
        amp *= falloff;
        per *= 2;
    }
    total * (1.0 / norm)
}

/// Noise folded at its midline and inverted: creases instead of blobs — the
/// standard way to get veins, ridges and fracture lines out of smooth noise.
pub fn ridged(size: usize, seed: u64, octaves: u32, period: usize, source: NoiseSource) -> Grid {
    let folded = fbm(size, seed, octaves, period, source, 0.5).v;
    Grid::from_vec(
        size,
        folded.iter().map(|f| 1.0 - (f * 2.0 - 1.0).abs()).collect(),
    )
    .normalize()
}

/// Straight bands, stated as WHOLE CYCLES across the texture, so the tiling
/// condition cannot be stated wrongly: the angle is the ratio of the two counts,
/// and any pair of integers tiles.
pub fn stripes(size: usize, cycles_x: i64, cycles_y: i64, phase: f64) -> Grid {
    let mut v = vec![0.0; size * size];
    for (idx, slot) in v.iter_mut().enumerate() {
        let x = (idx % size) as f64;
        let y = (idx / size) as f64;
        let t = (x * cycles_x as f64 + y * cycles_y as f64) / size as f64 + phase;
        *slot = 0.5 + 0.5 * (t * TAU).sin();
    }
    Grid::from_vec(size, v)
}

/// A flat surface of one value.
pub fn constant(size: usize, value: f64) -> Grid {
    Grid::from_vec(size, vec![value; size * size])
}
