//! RASTER: colours, the shaders that place them, and the canvas fields are
//! painted onto.
//!
//! PAINTING IS COMPOSITING, not assignment. Assigning a translucent colour would
//! replace what was under it — so a soft glow drawn over a part would punch a
//! hole in it rather than glow on it. [`Canvas::paint`] over-composites;
//! [`Canvas::stamp`] still overwrites, for masks where the alpha channel is the
//! artwork.
//!
//! Row 0 is the TOP of the image, so every [`vertical`] ramp and "lit from
//! above" decision reads the same as the Python original.
//!
//! A port of the Python `artlib.raster`. Python says each operation to a whole
//! array of pixels at once with numpy; here each is a per-pixel loop with the
//! identical arithmetic and the identical integer rounding — truncation toward
//! zero everywhere except the composited alpha, which rounds half up — so the
//! bytes match.

use crate::fields::Field;
use std::sync::Arc;

/// An RGBA colour, 0..255 per channel — how colours are stated at call sites.
pub type Rgba = [u8; 4];

/// Fully transparent.
pub const CLEAR: Rgba = [0, 0, 0, 0];
/// Opaque white.
pub const WHITE: Rgba = [255, 255, 255, 255];

/// A colour that depends on WHERE it is. Returns channels as floats that always
/// hold integer values (0..255), matching numpy's integer colour arrays.
pub type Shader = Arc<dyn Fn(f64, f64) -> [f64; 4] + Send + Sync>;

// ---------------------------------------------------------------------------
// Colour.
// ---------------------------------------------------------------------------

/// Lighten (`>1`) or darken (`<1`), keeping alpha.
pub fn shade(color: Rgba, factor: f64) -> Rgba {
    let ch = |c: u8| (c as f64 * factor).clamp(0.0, 255.0) as i64 as u8;
    [ch(color[0]), ch(color[1]), ch(color[2]), color[3]]
}

/// Blend two colours, `t = 0` giving `a`. Clamped, because callers compute `t`
/// from a distance and a distance runs past the end of what it measures.
/// Channels are truncated to integers, as in the original.
pub fn mix(a: Rgba, b: Rgba, t: f64) -> [f64; 4] {
    let t = t.clamp(0.0, 1.0);
    let mut out = [0.0; 4];
    for c in 0..4 {
        let low = a[c] as f64;
        let high = b[c] as f64;
        out[c] = (low + (high - low) * t).trunc();
    }
    out
}

/// The same colour at a stated opacity.
pub fn alpha(color: Rgba, a: u8) -> Rgba {
    [color[0], color[1], color[2], a]
}

// ---------------------------------------------------------------------------
// Shaders.
// ---------------------------------------------------------------------------

/// One colour everywhere.
pub fn solid(color: Rgba) -> Shader {
    let c = [
        color[0] as f64,
        color[1] as f64,
        color[2] as f64,
        color[3] as f64,
    ];
    Arc::new(move |_, _| c)
}

/// A ramp down the image between rows `y0` and `y1` — THE lighting model, since
/// everything is lit by the same dead sky and so is brighter at the top.
pub fn vertical(top: Rgba, bottom: Rgba, y0: f64, y1: f64) -> Shader {
    let span = if y1 - y0 == 0.0 { 1.0 } else { y1 - y0 };
    Arc::new(move |_, y| mix(top, bottom, (y - y0) / span))
}

/// A ramp across the image between columns `x0` and `x1`.
pub fn horizontal(left: Rgba, right: Rgba, x0: f64, x1: f64) -> Shader {
    let span = if x1 - x0 == 0.0 { 1.0 } else { x1 - x0 };
    Arc::new(move |x, _| mix(left, right, (x - x0) / span))
}

/// A ramp outward from a point: a hot core darkening toward its rim.
pub fn radial(cx: f64, cy: f64, r: f64, inner: Rgba, outer: Rgba) -> Shader {
    let denom = if r == 0.0 { 1.0 } else { r };
    Arc::new(move |x, y| mix(inner, outer, (x - cx).hypot(y - cy) / denom))
}

/// Colour by the VALUE of a field — how a noise grid becomes rock.
pub fn from_field(field: Field, low: Rgba, high: Rgba, lo: f64, hi: f64) -> Shader {
    let span = if hi - lo == 0.0 { 1.0 } else { hi - lo };
    Arc::new(move |x, y| mix(low, high, (field(x, y) - lo) / span))
}

// ---------------------------------------------------------------------------
// The canvas.
// ---------------------------------------------------------------------------

/// One RGBA value per pixel, stored as floats (row-major, `y * w + x`) so
/// compositing arithmetic stays exact until the final quantisation in
/// [`Canvas::to_rgba8`].
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    px: Vec<[f64; 4]>,
}

impl Canvas {
    /// A canvas filled with `fill`.
    pub fn new(w: usize, h: usize, fill: Rgba) -> Self {
        let fill = [
            fill[0] as f64,
            fill[1] as f64,
            fill[2] as f64,
            fill[3] as f64,
        ];
        Self {
            w,
            h,
            px: vec![fill; w * h],
        }
    }

    /// A square canvas, transparent.
    pub fn square(size: usize) -> Self {
        Self::new(size, size, CLEAR)
    }

    fn coverage(&self, field: &Field, x: f64, y: f64, aa: bool) -> f64 {
        let d = field(x, y);
        if !aa {
            return if d <= 0.0 { 1.0 } else { 0.0 };
        }
        // A pixel whose centre is half a pixel inside the boundary is fully
        // covered; half a pixel outside, not at all. Linear between.
        (0.5 - d).clamp(0.0, 1.0)
    }

    /// Composite a shape onto the canvas through a shader, source-over.
    pub fn paint(&mut self, field: &Field, shader: &Shader, aa: bool, opacity: f64) -> &mut Self {
        for idx in 0..self.w * self.h {
            let x = (idx % self.w) as f64;
            let y = (idx / self.w) as f64;
            let cov = self.coverage(field, x, y, aa);
            if cov <= 0.0 {
                continue;
            }
            let src = shader(x, y);
            let a = (src[3] / 255.0) * cov * opacity;
            if a <= 0.0 {
                continue;
            }

            let dst = self.px[idx];
            // Standard source-over, un-premultiplied. A fully opaque source
            // replaces; a destination that ends up transparent is CLEAR;
            // everything else blends.
            let back = (dst[3] / 255.0) * (1.0 - a);
            let out_a = a + back;
            if out_a <= 0.0 {
                self.px[idx] = [0.0; 4];
                continue;
            }
            let mut blended = [0.0; 4];
            for c in 0..3 {
                blended[c] = ((src[c] * a + dst[c] * back) / out_a).trunc();
            }
            blended[3] = (out_a * 255.0 + 0.5).trunc();
            if a >= 1.0 {
                blended[0] = src[0];
                blended[1] = src[1];
                blended[2] = src[2];
                blended[3] = 255.0;
            }
            self.px[idx] = blended;
        }
        self
    }

    /// REPLACE every covered pixel, alpha included — for masks where the alpha
    /// channel is the artwork rather than an opacity.
    pub fn stamp(&mut self, field: &Field, shader: &Shader, aa: bool) -> &mut Self {
        for idx in 0..self.w * self.h {
            let x = (idx % self.w) as f64;
            let y = (idx / self.w) as f64;
            if self.coverage(field, x, y, aa) > 0.0 {
                self.px[idx] = shader(x, y);
            }
        }
        self
    }

    /// Multiply what is already painted by a per-pixel brightness factor — how a
    /// surface gets LIT. Hand it [`Grid::relief`](crate::texture::Grid::relief),
    /// which returns factors around 1. `restrict`, if given, confines it to that
    /// shape.
    pub fn modulate(&mut self, factors: &Field, restrict: Option<&Field>) -> &mut Self {
        for idx in 0..self.w * self.h {
            if self.px[idx][3] == 0.0 {
                continue;
            }
            let x = (idx % self.w) as f64;
            let y = (idx / self.w) as f64;
            if let Some(f) = restrict {
                if f(x, y) > 0.0 {
                    continue;
                }
            }
            let k = factors(x, y);
            for c in 0..3 {
                self.px[idx][c] = (self.px[idx][c] * k).trunc().clamp(0.0, 255.0);
            }
        }
        self
    }

    /// Paint every pixel through a shader, ignoring what was there.
    pub fn fill(&mut self, shader: &Shader) -> &mut Self {
        for idx in 0..self.w * self.h {
            let x = (idx % self.w) as f64;
            let y = (idx / self.w) as f64;
            self.px[idx] = shader(x, y);
        }
        self
    }

    /// The finished image as 8-bit RGBA bytes, row-major. Channels are asserted
    /// in range then truncated, exactly as the Python `write_png` does before
    /// deflating — a channel outside `0..255` is a compositing bug, not
    /// something to silently wrap.
    pub fn to_rgba8(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.w * self.h * 4];
        for (idx, px) in self.px.iter().enumerate() {
            for c in 0..4 {
                let v = px[c];
                assert!(
                    (0.0..=255.0).contains(&v),
                    "channel out of range at pixel {idx}: {v}"
                );
                out[idx * 4 + c] = v as u8;
            }
        }
        out
    }
}
