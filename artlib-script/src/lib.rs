//! A small sandboxed text DSL over [`artlib`], on the Rune language.
//!
//! This is the "layer era" authoring surface: compose artlib's shapes, noise and
//! compositing in text and get pixels back, before the node graph exists. The
//! graph, when it comes, is a second front-end onto the identical calls.
//!
//! A script defines `pub fn main(w, h)` and returns a `Canvas`:
//!
//! ```text
//! pub fn main(w, h) {
//!     let c = Canvas::new(w, h);
//!     let plate = chamfered_rect(0.0, 0.0, 64.0, 64.0, 9.0);
//!     c.paint(plate, vertical(rgb(150, 160, 175), rgb(60, 66, 80), 0.0, 63.0));
//!     c.paint(outline(plate, 1.0, 1.0), solid(rgb(25, 28, 36)));
//!     c
//! }
//! ```
//!
//! Numbers: geometry (coordinates, radii, weights, angles) is floating point;
//! counts (sizes, periods, seeds, octaves) are integers; colours are packed into
//! one integer by [`rgb`]/[`rgba`]. A field bound to a name may be reused freely
//! — the binding functions take fields by reference.
//!
//! The core [`artlib`] crate stays dependency-free; the Rune binding lives only
//! here. Scripts are **sandboxed**: the only functions installed are artlib's,
//! plus Rune's own pure default modules (arithmetic, `let`, arrays, tuples). No
//! file, network or process capability is ever registered — those live in the
//! separate `rune-modules` crate, which is not a dependency.

use rune::{Any, Context, Diagnostics, Module, Source, Sources, Vm};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Value types: thin Rune-visible wrappers over artlib's, since `#[derive(Any)]`
// needs the type to live in a crate that depends on Rune (artlib does not).
// ---------------------------------------------------------------------------

/// A shape or surface: artlib's [`Field`](artlib::fields::Field).
#[derive(Any, Clone)]
struct Field {
    inner: artlib::fields::Field,
}

/// A float-per-pixel surface: artlib's [`Grid`](artlib::texture::Grid).
#[derive(Any, Clone)]
struct Grid {
    inner: artlib::texture::Grid,
}

/// A position-dependent colour: artlib's [`Shader`](artlib::raster::Shader).
#[derive(Any, Clone)]
struct Shader {
    inner: artlib::raster::Shader,
}

/// The image being built.
#[derive(Any)]
struct Canvas {
    inner: artlib::raster::Canvas,
}

impl Field {
    fn wrap(inner: artlib::fields::Field) -> Field {
        Field { inner }
    }
}
impl Grid {
    fn wrap(inner: artlib::texture::Grid) -> Grid {
        Grid { inner }
    }
}
impl Shader {
    fn wrap(inner: artlib::raster::Shader) -> Shader {
        Shader { inner }
    }
}

// ---------------------------------------------------------------------------
// Colour, packed into one i64 so the DSL passes colours as plain values.
// ---------------------------------------------------------------------------

fn pack(c: artlib::raster::Rgba) -> i64 {
    ((c[0] as i64) << 24) | ((c[1] as i64) << 16) | ((c[2] as i64) << 8) | (c[3] as i64)
}

fn unpack(color: i64) -> artlib::raster::Rgba {
    [
        ((color >> 24) & 0xff) as u8,
        ((color >> 16) & 0xff) as u8,
        ((color >> 8) & 0xff) as u8,
        (color & 0xff) as u8,
    ]
}

/// An opaque colour from its 0..255 channels.
#[rune::function]
fn rgb(r: i64, g: i64, b: i64) -> i64 {
    pack([r as u8, g as u8, b as u8, 255])
}

/// A colour with an explicit alpha.
#[rune::function]
fn rgba(r: i64, g: i64, b: i64, a: i64) -> i64 {
    pack([r as u8, g as u8, b as u8, a as u8])
}

/// Lighten (`>1`) or darken (`<1`) a colour, keeping alpha.
#[rune::function]
fn shade(color: i64, factor: f64) -> i64 {
    pack(artlib::raster::shade(unpack(color), factor))
}

// ---------------------------------------------------------------------------
// Fields: primitives.
// ---------------------------------------------------------------------------

#[rune::function]
fn disk(cx: f64, cy: f64, r: f64) -> Field {
    Field::wrap(artlib::fields::disk(cx, cy, r))
}

#[rune::function]
fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Field {
    Field::wrap(artlib::fields::ellipse(cx, cy, rx, ry))
}

#[rune::function]
fn ring(cx: f64, cy: f64, r_inner: f64, r_outer: f64) -> Field {
    Field::wrap(artlib::fields::ring(cx, cy, r_inner, r_outer))
}

#[rune::function]
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Field {
    Field::wrap(artlib::fields::rect(x0, y0, x1, y1))
}

#[rune::function]
fn half_plane(nx: f64, ny: f64, d: f64) -> Field {
    Field::wrap(artlib::fields::half_plane(nx, ny, d))
}

#[rune::function]
fn diamond(cx: f64, cy: f64, r: f64) -> Field {
    Field::wrap(artlib::fields::diamond(cx, cy, r))
}

#[rune::function]
fn capsule(x0: f64, y0: f64, x1: f64, y1: f64, r: f64) -> Field {
    Field::wrap(artlib::fields::capsule(x0, y0, x1, y1, r))
}

#[rune::function]
fn sector(cx: f64, cy: f64, from: f64, to: f64) -> Field {
    Field::wrap(artlib::fields::sector(cx, cy, from, to))
}

#[rune::function]
fn chamfered_rect(x0: f64, y0: f64, x1: f64, y1: f64, cut: f64) -> Field {
    Field::wrap(artlib::fields::chamfered_rect(x0, y0, x1, y1, cut))
}

#[rune::function]
fn hexagon(cx: f64, cy: f64, radius: f64, flat_top: bool) -> Field {
    Field::wrap(artlib::fields::hexagon(cx, cy, radius, flat_top))
}

/// A thick path through `[(x, y), ...]` control points.
#[rune::function]
fn polyline(points: Vec<(f64, f64)>, radius: f64) -> Field {
    Field::wrap(artlib::fields::polyline(&points, radius))
}

/// A filled polygon from its `[(x, y), ...]` vertices.
#[rune::function]
fn polygon(points: Vec<(f64, f64)>) -> Field {
    Field::wrap(artlib::fields::polygon(&points))
}

// ---------------------------------------------------------------------------
// Fields: algebra and transforms.
// ---------------------------------------------------------------------------

fn unwrap_fields(fields: Vec<Field>) -> Vec<artlib::fields::Field> {
    fields.into_iter().map(|f| f.inner).collect()
}

#[rune::function]
fn union(fields: Vec<Field>) -> Field {
    Field::wrap(artlib::fields::union(unwrap_fields(fields)))
}

#[rune::function]
fn intersect(fields: Vec<Field>) -> Field {
    Field::wrap(artlib::fields::intersect(unwrap_fields(fields)))
}

#[rune::function]
fn subtract(field: &Field, cutters: Vec<Field>) -> Field {
    Field::wrap(artlib::fields::subtract(
        field.inner.clone(),
        unwrap_fields(cutters),
    ))
}

#[rune::function]
fn invert(field: &Field) -> Field {
    Field::wrap(artlib::fields::invert(field.inner.clone()))
}

#[rune::function]
fn expand(field: &Field, r: f64) -> Field {
    Field::wrap(artlib::fields::expand(field.inner.clone(), r))
}

#[rune::function]
fn outline(field: &Field, weight: f64, inset: f64) -> Field {
    Field::wrap(artlib::fields::outline(field.inner.clone(), weight, inset))
}

#[rune::function]
fn everywhere() -> Field {
    Field::wrap(artlib::fields::everywhere())
}

#[rune::function]
fn translate(field: &Field, dx: f64, dy: f64) -> Field {
    Field::wrap(artlib::fields::translate(field.inner.clone(), dx, dy))
}

#[rune::function]
fn rotate(field: &Field, degrees: f64, cx: f64, cy: f64) -> Field {
    Field::wrap(artlib::fields::rotate(field.inner.clone(), degrees, cx, cy))
}

#[rune::function]
fn scale(field: &Field, factor: f64, cx: f64, cy: f64) -> Field {
    Field::wrap(artlib::fields::scale(field.inner.clone(), factor, cx, cy))
}

#[rune::function]
fn mirror4(field: &Field, w: f64, h: f64) -> Field {
    Field::wrap(artlib::fields::mirror4(field.inner.clone(), w, h))
}

#[rune::function]
fn polar_array(field: &Field, count: i64, cx: f64, cy: f64, phase: f64) -> Field {
    Field::wrap(artlib::fields::polar_array(
        field.inner.clone(),
        count as usize,
        cx,
        cy,
        phase,
    ))
}

// ---------------------------------------------------------------------------
// Texture: noise sources (returning grids) and grid operations.
// ---------------------------------------------------------------------------

#[rune::function]
fn value_noise(size: i64, period: i64, seed: i64) -> Grid {
    Grid::wrap(artlib::texture::value_noise(
        size as usize,
        period as usize,
        seed as u64,
    ))
}

#[rune::function]
fn perlin(size: i64, period: i64, seed: i64) -> Grid {
    Grid::wrap(artlib::texture::perlin(
        size as usize,
        period as usize,
        seed as u64,
    ))
}

/// Cellular noise, `f1` — blobs and cell interiors.
#[rune::function]
fn worley(size: i64, period: i64, seed: i64) -> Grid {
    Grid::wrap(artlib::texture::worley(
        size as usize,
        period as usize,
        seed as u64,
    ))
}

/// Cellular noise, `f2 - f1` — the crack network between cells.
#[rune::function]
fn worley_cracks(size: i64, period: i64, seed: i64) -> Grid {
    Grid::wrap(artlib::texture::worley_with(
        size as usize,
        period as usize,
        seed as u64,
        artlib::texture::Feature::F2F1,
        1.0,
    ))
}

/// Layered gradient noise: form, then grain on it.
#[rune::function]
fn fbm(size: i64, seed: i64, octaves: i64, period: i64) -> Grid {
    Grid::wrap(artlib::texture::fbm(
        size as usize,
        seed as u64,
        octaves as u32,
        period as usize,
        artlib::texture::perlin,
        0.5,
    ))
}

/// Folded noise: creases instead of blobs — veins, ridges, fracture lines.
#[rune::function]
fn ridged(size: i64, seed: i64, octaves: i64, period: i64) -> Grid {
    Grid::wrap(artlib::texture::ridged(
        size as usize,
        seed as u64,
        octaves as u32,
        period as usize,
        artlib::texture::perlin,
    ))
}

/// Straight bands, stated as whole cycles across the texture.
#[rune::function]
fn stripes(size: i64, cycles_x: i64, cycles_y: i64, phase: f64) -> Grid {
    Grid::wrap(artlib::texture::stripes(
        size as usize,
        cycles_x,
        cycles_y,
        phase,
    ))
}

/// A flat surface of one value.
#[rune::function]
fn constant(size: i64, value: f64) -> Grid {
    Grid::wrap(artlib::texture::constant(size as usize, value))
}

impl Grid {
    /// This surface as a field, so it can be coloured or painted.
    #[rune::function]
    fn field(&self) -> Field {
        Field::wrap(self.inner.field())
    }

    /// Stretch to fill `0..1`.
    #[rune::function]
    fn normalize(&self) -> Grid {
        Grid::wrap(self.inner.normalize())
    }

    #[rune::function]
    fn clamp(&self, lo: f64, hi: f64) -> Grid {
        Grid::wrap(self.inner.clamp(lo, hi))
    }

    /// Push values toward one end (`power > 1` sharpens, `< 1` opens up).
    #[rune::function]
    fn gain(&self, power: f64) -> Grid {
        Grid::wrap(self.inner.gain(power))
    }

    #[rune::function]
    fn remap(&self, lo: f64, hi: f64) -> Grid {
        Grid::wrap(self.inner.remap(lo, hi))
    }

    /// Snap to discrete levels — sedimentary banding.
    #[rune::function]
    fn quantize(&self, steps: f64) -> Grid {
        Grid::wrap(self.inner.quantize(steps))
    }

    #[rune::function]
    fn lerp(&self, other: &Grid, t: f64) -> Grid {
        Grid::wrap(self.inner.lerp(&other.inner, t))
    }

    #[rune::function]
    fn blur(&self, radius: i64, passes: i64) -> Grid {
        Grid::wrap(self.inner.blur(radius, passes.max(0) as u32))
    }

    /// Keep fine detail, drop the broad shape — hides a repeat.
    #[rune::function]
    fn highpass(&self, radius: i64) -> Grid {
        Grid::wrap(self.inner.highpass(radius))
    }

    /// Sample this surface pushed around by two others — flow and erosion.
    #[rune::function]
    fn warp(&self, dx: &Grid, dy: &Grid, amount: f64) -> Grid {
        Grid::wrap(self.inner.warp(&dx.inner, &dy.inner, amount))
    }

    /// Light this surface as a height map; returns brightness factors near 1
    /// for [`Canvas::modulate`].
    #[rune::function]
    fn relief(&self, azimuth: f64, strength: f64, ambient: f64) -> Grid {
        Grid::wrap(self.inner.relief(azimuth, strength, ambient))
    }

    /// The band `lo..hi` as a field, so it can be painted like a shape.
    #[rune::function]
    fn mask(&self, lo: f64, hi: f64, softness: f64) -> Field {
        Field::wrap(self.inner.mask(lo, hi, softness))
    }

    #[rune::function]
    fn add(&self, other: &Grid) -> Grid {
        Grid::wrap(self.inner.clone() + other.inner.clone())
    }

    #[rune::function]
    fn sub(&self, other: &Grid) -> Grid {
        // a - b, elementwise, via a + (b * -1).
        Grid::wrap(self.inner.clone() + (other.inner.clone() * -1.0))
    }

    /// Multiply every value by a scalar.
    #[rune::function]
    fn mul(&self, scalar: f64) -> Grid {
        Grid::wrap(self.inner.clone() * scalar)
    }
}

// ---------------------------------------------------------------------------
// Shaders.
// ---------------------------------------------------------------------------

#[rune::function]
fn solid(color: i64) -> Shader {
    Shader::wrap(artlib::raster::solid(unpack(color)))
}

/// A ramp down the image between rows `y0` and `y1`.
#[rune::function]
fn vertical(top: i64, bottom: i64, y0: f64, y1: f64) -> Shader {
    Shader::wrap(artlib::raster::vertical(unpack(top), unpack(bottom), y0, y1))
}

/// A ramp across the image between columns `x0` and `x1`.
#[rune::function]
fn horizontal(left: i64, right: i64, x0: f64, x1: f64) -> Shader {
    Shader::wrap(artlib::raster::horizontal(
        unpack(left),
        unpack(right),
        x0,
        x1,
    ))
}

/// A ramp outward from a point.
#[rune::function]
fn radial(cx: f64, cy: f64, r: f64, inner: i64, outer: i64) -> Shader {
    Shader::wrap(artlib::raster::radial(
        cx,
        cy,
        r,
        unpack(inner),
        unpack(outer),
    ))
}

/// Colour by the value of a field over the range `lo..hi`.
#[rune::function]
fn from_field(field: &Field, low: i64, high: i64, lo: f64, hi: f64) -> Shader {
    Shader::wrap(artlib::raster::from_field(
        field.inner.clone(),
        unpack(low),
        unpack(high),
        lo,
        hi,
    ))
}

/// Colour by the value of a surface — how a noise grid becomes rock.
#[rune::function]
fn from_grid(grid: &Grid, low: i64, high: i64, lo: f64, hi: f64) -> Shader {
    Shader::wrap(artlib::raster::from_field(
        grid.inner.field(),
        unpack(low),
        unpack(high),
        lo,
        hi,
    ))
}

// ---------------------------------------------------------------------------
// The canvas.
// ---------------------------------------------------------------------------

impl Canvas {
    /// A transparent canvas of the given size.
    #[rune::function(path = Self::new)]
    fn new(w: i64, h: i64) -> Canvas {
        Canvas {
            inner: artlib::raster::Canvas::new(w as usize, h as usize, artlib::raster::CLEAR),
        }
    }

    /// Composite a shape through a shader, source-over, antialiased.
    #[rune::function]
    fn paint(&mut self, field: &Field, shader: &Shader) {
        self.inner.paint(&field.inner, &shader.inner, true, 1.0);
    }

    /// Paint at reduced opacity.
    #[rune::function]
    fn paint_opacity(&mut self, field: &Field, shader: &Shader, opacity: f64) {
        self.inner.paint(&field.inner, &shader.inner, true, opacity);
    }

    /// Paint with a hard (non-antialiased) edge.
    #[rune::function]
    fn paint_hard(&mut self, field: &Field, shader: &Shader) {
        self.inner.paint(&field.inner, &shader.inner, false, 1.0);
    }

    /// Replace every covered pixel, alpha included.
    #[rune::function]
    fn stamp(&mut self, field: &Field, shader: &Shader) {
        self.inner.stamp(&field.inner, &shader.inner, false);
    }

    /// Paint every pixel through a shader.
    #[rune::function]
    fn fill(&mut self, shader: &Shader) {
        self.inner.fill(&shader.inner);
    }

    /// Multiply what is already painted by a per-pixel brightness field.
    #[rune::function]
    fn modulate(&mut self, factors: &Field) {
        self.inner.modulate(&factors.inner, None);
    }

    /// Modulate only within a shape.
    #[rune::function]
    fn modulate_in(&mut self, factors: &Field, restrict: &Field) {
        self.inner.modulate(&factors.inner, Some(&restrict.inner));
    }
}

// ---------------------------------------------------------------------------
// Knobs: parameters a script declares, so it can be tweaked without editing it.
//
// A script asks the host for each knob at the point of use — `p.num("r", ...)` —
// which both DECLARES the knob (so the editor can build a control) and returns
// its CURRENT value. The host runs the script once with no values to collect the
// declarations, then again with the user's values whenever a control moves.
// ---------------------------------------------------------------------------

/// What kind of control a knob wants, and its bounds.
#[derive(Debug, Clone, PartialEq)]
pub enum KnobKind {
    Float { min: f64, max: f64 },
    Int { min: i64, max: i64 },
    Color,
    Bool,
}

/// A knob's value, in whichever type it holds.
#[derive(Debug, Clone, PartialEq)]
pub enum KnobValue {
    Float(f64),
    Int(i64),
    Color([u8; 4]),
    Bool(bool),
}

/// One parameter a script declared: a stable id, what control it wants, and the
/// value to start from.
#[derive(Debug, Clone, PartialEq)]
pub struct Knob {
    pub id: String,
    pub kind: KnobKind,
    pub default: KnobValue,
}

/// The current value of each knob, by id — what the editor hands back as the
/// user turns the controls.
pub type KnobValues = HashMap<String, KnobValue>;

/// What a script run produced: the pixels, and the knobs it declared (in the
/// order it asked for them).
pub struct Generated {
    pub pixels: Vec<u8>,
    pub knobs: Vec<Knob>,
}

/// The shared state behind a [`Params`]: the values fed in, and the declarations
/// collected as the script runs.
#[derive(Default)]
struct ParamState {
    values: KnobValues,
    declared: Vec<Knob>,
}

impl ParamState {
    /// Record a declaration the first time its id is seen, so re-asking for the
    /// same knob (e.g. in a loop) doesn't list it twice.
    fn declare(&mut self, knob: Knob) {
        if !self.declared.iter().any(|k| k.id == knob.id) {
            self.declared.push(knob);
        }
    }
}

/// The `p` a script's `main(w, h, p)` receives: it declares knobs and reads
/// their current values. Cheap to clone (a shared handle), so the host keeps one
/// to read the declarations back after the run.
#[derive(Any, Clone)]
struct Params {
    state: Arc<Mutex<ParamState>>,
}

impl Params {
    fn new(values: KnobValues) -> Self {
        Self {
            state: Arc::new(Mutex::new(ParamState {
                values,
                declared: Vec::new(),
            })),
        }
    }

    /// A floating-point knob shown as a slider over `min..max`.
    #[rune::function]
    fn num(&self, id: &str, default: f64, min: f64, max: f64) -> f64 {
        let mut s = self.state.lock().expect("params not poisoned");
        s.declare(Knob {
            id: id.to_owned(),
            kind: KnobKind::Float { min, max },
            default: KnobValue::Float(default),
        });
        match s.values.get(id) {
            Some(KnobValue::Float(v)) => *v,
            _ => default,
        }
    }

    /// An integer knob shown as a slider over `min..max`.
    #[rune::function]
    fn int(&self, id: &str, default: i64, min: i64, max: i64) -> i64 {
        let mut s = self.state.lock().expect("params not poisoned");
        s.declare(Knob {
            id: id.to_owned(),
            kind: KnobKind::Int { min, max },
            default: KnobValue::Int(default),
        });
        match s.values.get(id) {
            Some(KnobValue::Int(v)) => *v,
            _ => default,
        }
    }

    /// A colour knob. The default and the returned value are packed integers, so
    /// it composes with `solid`, `vertical`, etc.
    #[rune::function]
    fn color(&self, id: &str, default: i64) -> i64 {
        let mut s = self.state.lock().expect("params not poisoned");
        s.declare(Knob {
            id: id.to_owned(),
            kind: KnobKind::Color,
            default: KnobValue::Color(unpack(default)),
        });
        match s.values.get(id) {
            Some(KnobValue::Color(c)) => pack(*c),
            _ => default,
        }
    }

    /// A boolean knob shown as a checkbox.
    #[rune::function]
    fn toggle(&self, id: &str, default: bool) -> bool {
        let mut s = self.state.lock().expect("params not poisoned");
        s.declare(Knob {
            id: id.to_owned(),
            kind: KnobKind::Bool,
            default: KnobValue::Bool(default),
        });
        match s.values.get(id) {
            Some(KnobValue::Bool(v)) => *v,
            _ => default,
        }
    }
}

// ---------------------------------------------------------------------------
// Running a script.
// ---------------------------------------------------------------------------

/// The artlib vocabulary as a Rune module.
fn artlib_module() -> Result<Module, rune::ContextError> {
    let mut m = Module::new();
    m.ty::<Field>()?;
    m.ty::<Grid>()?;
    m.ty::<Shader>()?;
    m.ty::<Canvas>()?;
    m.ty::<Params>()?;

    // knobs
    m.function_meta(Params::num)?;
    m.function_meta(Params::int)?;
    m.function_meta(Params::color)?;
    m.function_meta(Params::toggle)?;

    // colour
    m.function_meta(rgb)?;
    m.function_meta(rgba)?;
    m.function_meta(shade)?;

    // primitives
    m.function_meta(disk)?;
    m.function_meta(ellipse)?;
    m.function_meta(ring)?;
    m.function_meta(rect)?;
    m.function_meta(half_plane)?;
    m.function_meta(diamond)?;
    m.function_meta(capsule)?;
    m.function_meta(sector)?;
    m.function_meta(chamfered_rect)?;
    m.function_meta(hexagon)?;
    m.function_meta(polyline)?;
    m.function_meta(polygon)?;

    // algebra + transforms
    m.function_meta(union)?;
    m.function_meta(intersect)?;
    m.function_meta(subtract)?;
    m.function_meta(invert)?;
    m.function_meta(expand)?;
    m.function_meta(outline)?;
    m.function_meta(everywhere)?;
    m.function_meta(translate)?;
    m.function_meta(rotate)?;
    m.function_meta(scale)?;
    m.function_meta(mirror4)?;
    m.function_meta(polar_array)?;

    // texture
    m.function_meta(value_noise)?;
    m.function_meta(perlin)?;
    m.function_meta(worley)?;
    m.function_meta(worley_cracks)?;
    m.function_meta(fbm)?;
    m.function_meta(ridged)?;
    m.function_meta(stripes)?;
    m.function_meta(constant)?;
    m.function_meta(Grid::field)?;
    m.function_meta(Grid::normalize)?;
    m.function_meta(Grid::clamp)?;
    m.function_meta(Grid::gain)?;
    m.function_meta(Grid::remap)?;
    m.function_meta(Grid::quantize)?;
    m.function_meta(Grid::lerp)?;
    m.function_meta(Grid::blur)?;
    m.function_meta(Grid::highpass)?;
    m.function_meta(Grid::warp)?;
    m.function_meta(Grid::relief)?;
    m.function_meta(Grid::mask)?;
    m.function_meta(Grid::add)?;
    m.function_meta(Grid::sub)?;
    m.function_meta(Grid::mul)?;

    // shaders
    m.function_meta(solid)?;
    m.function_meta(vertical)?;
    m.function_meta(horizontal)?;
    m.function_meta(radial)?;
    m.function_meta(from_field)?;
    m.function_meta(from_grid)?;

    // canvas
    m.function_meta(Canvas::new)?;
    m.function_meta(Canvas::paint)?;
    m.function_meta(Canvas::paint_opacity)?;
    m.function_meta(Canvas::paint_hard)?;
    m.function_meta(Canvas::stamp)?;
    m.function_meta(Canvas::fill)?;
    m.function_meta(Canvas::modulate)?;
    m.function_meta(Canvas::modulate_in)?;

    Ok(m)
}

/// Run a DSL script and return its `w * h` RGBA8 pixels together with the knobs
/// it declared.
///
/// The script must define `pub fn main(w, h, p)` returning a `Canvas`, where `p`
/// is the knob object (`p.num(...)`, `p.color(...)`, …). `values` supplies the
/// current setting of each knob by id; pass an empty map to run at the declared
/// defaults and discover what knobs exist.
pub fn generate(
    source: &str,
    w: usize,
    h: usize,
    values: KnobValues,
) -> Result<Generated, String> {
    let module = artlib_module().map_err(|e| e.to_string())?;
    let mut context = Context::with_default_modules().map_err(|e| e.to_string())?;
    context.install(&module).map_err(|e| e.to_string())?;
    let runtime = Arc::new(context.runtime().map_err(|e| e.to_string())?);

    let mut sources = Sources::new();
    sources
        .insert(Source::memory(source).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    let mut diagnostics = Diagnostics::new();
    let unit = rune::prepare(&mut sources)
        .with_context(&context)
        .with_diagnostics(&mut diagnostics)
        .build()
        .map_err(|e| format!("compile error: {e}"))?;

    // Keep a handle to the shared param state so the declarations the script
    // records can be read back after it returns.
    let params = Params::new(values);
    let collected = params.state.clone();

    let mut vm = Vm::new(runtime, Arc::new(unit));
    let output = vm
        .call(["main"], (w as i64, h as i64, params))
        .map_err(|e| format!("run error: {e}"))?;
    let canvas: Canvas = rune::from_value(output).map_err(|e| e.to_string())?;

    let knobs = collected.lock().expect("params not poisoned").declared.clone();
    Ok(Generated {
        pixels: canvas.inner.to_rgba8(),
        knobs,
    })
}

/// Run a script at its default knob values and return just the pixels — the
/// convenience used by tests and by callers that don't drive knobs.
pub fn run_script(source: &str, w: usize, h: usize) -> Result<Vec<u8>, String> {
    generate(source, w, h, KnobValues::new()).map(|g| g.pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(px: &[u8], w: usize, x: usize, y: usize) -> [u8; 4] {
        let i = (y * w + x) * 4;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    }

    #[test]
    fn runs_a_disk_script() {
        let src = r#"
            pub fn main(w, h, p) {
                let c = Canvas::new(w, h);
                c.paint(disk(32.0, 32.0, 20.0), solid(rgb(200, 80, 60)));
                c
            }
        "#;
        let px = run_script(src, 64, 64).unwrap();
        assert_eq!(pixel(&px, 64, 32, 32), [200, 80, 60, 255]);
        assert_eq!(pixel(&px, 64, 2, 2), [0, 0, 0, 0]);
    }

    #[test]
    fn reuse_a_field_binding() {
        let src = r#"
            pub fn main(w, h, p) {
                let c = Canvas::new(w, h);
                let plate = chamfered_rect(0.0, 0.0, 64.0, 64.0, 9.0);
                c.paint(plate, solid(rgb(70, 110, 180)));
                c.paint(outline(plate, 1.0, 1.0), solid(rgb(20, 20, 28)));
                c
            }
        "#;
        assert!(run_script(src, 64, 64).is_ok());
    }

    #[test]
    fn noise_and_grid_ops_run() {
        // Grid arithmetic, a warp, colouring by a surface, and lighting.
        let src = r#"
            pub fn main(w, h, p) {
                let c = Canvas::new(w, h);
                let form = fbm(64, 11, 4, 4);
                let crack = worley_cracks(64, 5, 11);
                let rock = form.mul(0.7).add(crack.mul(0.3)).normalize();
                c.fill(from_grid(rock, rgb(60, 60, 70), rgb(180, 180, 190), 0.0, 1.0));
                c.modulate(rock.relief(135.0, 2.0, 0.55).field());
                c
            }
        "#;
        let a = run_script(src, 64, 64).unwrap();
        let b = run_script(src, 64, 64).unwrap();
        assert_eq!(a, b, "same script must be deterministic");
        assert_eq!(a.len(), 64 * 64 * 4);
    }

    #[test]
    fn points_lists_work() {
        let src = r#"
            pub fn main(w, h, p) {
                let c = Canvas::new(w, h);
                c.paint(polygon([(32.0, 6.0), (58.0, 52.0), (6.0, 52.0)]), solid(rgb(70, 110, 180)));
                c.paint(polyline([(10.0, 12.0), (52.0, 20.0), (28.0, 54.0)], 4.0), solid(rgb(200, 70, 160)));
                c
            }
        "#;
        assert!(run_script(src, 64, 64).is_ok());
    }

    #[test]
    fn declares_and_reads_knobs() {
        let src = r#"
            pub fn main(w, h, p) {
                let r = p.num("radius", 20.0, 4.0, 30.0);
                let col = p.color("fill", rgb(200, 80, 60));
                let c = Canvas::new(w, h);
                c.paint(disk(32.0, 32.0, r), solid(col));
                c
            }
        "#;
        // Discovery run: no values → declarations surface with their defaults.
        let g = generate(src, 64, 64, KnobValues::new()).unwrap();
        assert_eq!(g.knobs.len(), 2);
        assert_eq!(g.knobs[0].id, "radius");
        assert!(matches!(g.knobs[0].kind, KnobKind::Float { min, max } if min == 4.0 && max == 30.0));
        assert_eq!(g.knobs[0].default, KnobValue::Float(20.0));
        assert_eq!(g.knobs[1].id, "fill");
        assert!(matches!(g.knobs[1].kind, KnobKind::Color));
        assert_eq!(pixel(&g.pixels, 64, 32, 32), [200, 80, 60, 255]);

        // Drive the radius knob down: the centre stays filled, a pixel that was
        // inside the default disk is now outside the small one.
        let mut values = KnobValues::new();
        values.insert("radius".into(), KnobValue::Float(3.0));
        let g2 = generate(src, 64, 64, values).unwrap();
        assert_eq!(pixel(&g2.pixels, 64, 32, 32), [200, 80, 60, 255]);
        assert_eq!(pixel(&g2.pixels, 64, 32, 45), [0, 0, 0, 0]);
    }

    #[test]
    fn filesystem_is_not_reachable() {
        // Nothing file/network/process related is installed, so a script cannot
        // even name such a function — it fails to compile.
        let src = r#"
            pub fn main(w, h, p) {
                std::fs::read_to_string("secret.txt")
            }
        "#;
        assert!(run_script(src, 64, 64).is_err(), "fs must be unreachable");
    }
}
