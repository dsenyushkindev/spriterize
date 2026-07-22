//! Filters that change how a layer looks without changing the pixels stored in
//! it.
//!
//! A layer keeps an ordered list of [`Filter`]s. What is drawn, exported and
//! sampled is the source image with the list applied in turn; the source itself
//! is only ever changed by the drawing tools.
//!
//! A filter is a [`FilterKind`] registered under an id, plus the settings to
//! run it with. Kinds declare their settings, so the interface can build
//! controls for a filter it knows nothing about, and a saved filter is just an
//! id and some named values — which means adding a kind, or a setting to an
//! existing kind, doesn't invalidate saved projects.

use crate::color::{BLACK, TRANSPARENT};
use crate::{Bitmap, Color, ColorF32, Point, Size};
use serde::{Deserialize, Serialize};
use std::sync::{LazyLock, RwLock};

/// Full strength, for the settings that dial an effect back.
pub const FULL_STRENGTH: i32 = 255;
/// Most times a filter is allowed to run over the image in one step.
pub const MAX_PASSES: i32 = 8;

/// The pixel access a filter needs.
///
/// Filters are used through a trait object so they can be registered and looked
/// up by id, which rules out being generic over the image type — this is the
/// narrow, object safe view of an image that they get instead.
pub trait Surface {
    fn size(&self) -> Size<i32>;
    fn pixel(&self, p: Point<i32>) -> Color;
    fn set_pixel(&mut self, p: Point<i32>, color: Color);
}

impl<T: Bitmap> Surface for T {
    fn size(&self) -> Size<i32> {
        Bitmap::size(self)
    }

    fn pixel(&self, p: Point<i32>) -> Color {
        Bitmap::pixel(self, p)
    }

    fn set_pixel(&mut self, p: Point<i32>, color: Color) {
        Bitmap::set_pixel(self, p, color);
    }
}

/// A value one of a filter's settings can hold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Int(i32),
    Color(Color),
    Bool(bool),
}

/// What a setting means, and so what control to offer for it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParamKind {
    /// A number within a range
    Int {
        min: i32,
        max: i32,
    },
    /// A number from none to full, shown as a percentage
    Ratio,
    Color,
    Bool,
}

/// One of a filter's settings: what it is called, what it holds, and what it
/// starts as.
#[derive(Debug, Clone)]
pub struct ParamSpec {
    /// Stable across versions: this is what ends up in saved projects
    pub id: &'static str,
    /// Shown next to the control
    pub label: &'static str,
    pub kind: ParamKind,
    pub default: Value,
    /// Shown when hovering the control
    pub help: &'static str,
}

/// The settings a filter was given, by id.
///
/// A setting that isn't here falls back to its default, so a filter that gains
/// one keeps working on projects saved before it existed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Params(Vec<(String, Value)>);

impl Params {
    /// The settings a kind starts with
    pub fn defaults(kind: &dyn FilterKind) -> Self {
        Self(
            kind.params()
                .iter()
                .map(|spec| (spec.id.to_owned(), spec.default.clone()))
                .collect(),
        )
    }

    pub fn get(&self, id: &str) -> Option<&Value> {
        self.0
            .iter()
            .find(|(key, _)| key == id)
            .map(|(_, value)| value)
    }

    pub fn set(&mut self, id: &str, value: Value) {
        match self.0.iter_mut().find(|(key, _)| key == id) {
            Some((_, held)) => *held = value,
            None => self.0.push((id.to_owned(), value)),
        }
    }

    pub fn int(&self, id: &str, fallback: i32) -> i32 {
        match self.get(id) {
            Some(Value::Int(v)) => *v,
            _ => fallback,
        }
    }

    pub fn color(&self, id: &str, fallback: Color) -> Color {
        match self.get(id) {
            Some(Value::Color(c)) => *c,
            _ => fallback,
        }
    }

    pub fn bool(&self, id: &str, fallback: bool) -> bool {
        match self.get(id) {
            Some(Value::Bool(v)) => *v,
            _ => fallback,
        }
    }
}

/// One kind of filter: what it is called, what it can be told to do, and how it
/// does it.
///
/// Implement this and hand it to [`register`] to add a filter; everything else
/// — the menu entry, the controls, saving and loading — follows from what is
/// declared here.
pub trait FilterKind: Send + Sync {
    /// Identifies this kind in saved projects, so it must not change
    fn id(&self) -> &'static str;

    /// What to call it in the interface
    fn name(&self) -> &'static str;

    /// The settings it takes, in the order they should be shown
    fn params(&self) -> &'static [ParamSpec] {
        &[]
    }

    /// Whether the result depends on the palette, and so is stale when the
    /// palette changes
    fn uses_palette(&self) -> bool {
        false
    }

    fn apply(&self, surface: &mut dyn Surface, params: &Params, palette: &[Color]);
}

/// A filter as a layer holds it: which kind, and the settings to run it with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Filter {
    pub id: String,
    pub params: Params,
}

impl Filter {
    /// A filter of the given kind, at its default settings
    pub fn new(kind: &dyn FilterKind) -> Self {
        Self {
            id: kind.id().to_owned(),
            params: Params::defaults(kind),
        }
    }

    /// The registered kind this refers to, if it is still known. A project may
    /// name a filter this build doesn't have.
    pub fn kind(&self) -> Option<&'static dyn FilterKind> {
        kind(&self.id)
    }

    /// What to call it in the interface
    pub fn name(&self) -> String {
        match self.kind() {
            Some(kind) => kind.name().to_owned(),
            None => format!("Unknown ({})", self.id),
        }
    }

    pub fn uses_palette(&self) -> bool {
        self.kind().is_some_and(|kind| kind.uses_palette())
    }

    /// Runs the filter. An unknown kind leaves the image alone rather than
    /// failing, so a project using a filter this build lacks still opens.
    pub fn apply<IMG: Bitmap>(&self, image: &mut IMG, palette: &[Color]) {
        if let Some(kind) = self.kind() {
            kind.apply(image, &self.params, palette);
        }
    }
}

/// Every registered kind. Boxes are leaked on registration: a filter kind lives
/// as long as the program does, and handing out `'static` references keeps
/// callers from having to hold the lock.
static REGISTRY: LazyLock<RwLock<Vec<&'static dyn FilterKind>>> = LazyLock::new(|| {
    RwLock::new(vec![
        &Smooth as &'static dyn FilterKind,
        &ApplyPalette,
        &Silhouette,
    ])
});

/// Adds a filter kind. Its id must be unique; registering an id that is already
/// taken replaces nothing and returns `false`.
pub fn register(kind: impl FilterKind + 'static) -> bool {
    let mut registry = REGISTRY.write().expect("filter registry is not poisoned");

    if registry.iter().any(|known| known.id() == kind.id()) {
        return false;
    }

    registry.push(Box::leak(Box::new(kind)));

    true
}

/// Looks a kind up by the id saved with a filter
pub fn kind(id: &str) -> Option<&'static dyn FilterKind> {
    REGISTRY
        .read()
        .expect("filter registry is not poisoned")
        .iter()
        .find(|kind| kind.id() == id)
        .copied()
}

/// Every registered kind, for offering the list of filters that can be added
pub fn kinds() -> Vec<&'static dyn FilterKind> {
    REGISTRY
        .read()
        .expect("filter registry is not poisoned")
        .clone()
}

// --- built in kinds ---------------------------------------------------------

/// Weights of the 3x3 neighbourhood used when smoothing. Leaning on the centre
/// keeps the result close to the original rather than washing it out.
const SMOOTH_KERNEL: [[u32; 3]; 3] = [[1, 2, 1], [2, 4, 2], [1, 2, 1]];

pub struct Smooth;

impl FilterKind for Smooth {
    fn id(&self) -> &'static str {
        "smooth"
    }

    fn name(&self) -> &'static str {
        "Smooth"
    }

    fn params(&self) -> &'static [ParamSpec] {
        &[
            ParamSpec {
                id: "strength",
                label: "strength",
                kind: ParamKind::Ratio,
                default: Value::Int(FULL_STRENGTH),
                help: "how much of the softened result to mix in",
            },
            ParamSpec {
                id: "passes",
                label: "passes",
                kind: ParamKind::Int {
                    min: 1,
                    max: MAX_PASSES,
                },
                default: Value::Int(1),
                help: "run more than once for a wider blur",
            },
        ]
    }

    fn apply(&self, surface: &mut dyn Surface, params: &Params, _palette: &[Color]) {
        let strength = params
            .int("strength", FULL_STRENGTH)
            .clamp(0, FULL_STRENGTH);
        let passes = params.int("passes", 1).clamp(1, MAX_PASSES);

        if strength == 0 {
            return;
        }

        let size = surface.size();

        for _ in 0..passes {
            // Read from an untouched copy, so pixels already smoothed this pass
            // don't feed into their neighbours and smear in one direction.
            let source = Snapshot::of(surface);

            for i in 0..size.x {
                for j in 0..size.y {
                    let p = Point::new(i, j);
                    let softened = smoothed_pixel(&source, p);

                    surface.set_pixel(p, mix(source.pixel(p), softened, strength));
                }
            }
        }
    }
}

pub struct ApplyPalette;

impl FilterKind for ApplyPalette {
    fn id(&self) -> &'static str {
        "apply_palette"
    }

    fn name(&self) -> &'static str {
        "Apply palette"
    }

    fn uses_palette(&self) -> bool {
        true
    }

    fn apply(&self, surface: &mut dyn Surface, _params: &Params, palette: &[Color]) {
        if palette.is_empty() {
            return;
        }

        let size = surface.size();

        for i in 0..size.x {
            for j in 0..size.y {
                let p = Point::new(i, j);
                let color: ColorF32 = surface.pixel(p).into();

                let nearest = palette
                    .iter()
                    .min_by(|a, b| {
                        let (a, b): (ColorF32, ColorF32) = ((**a).into(), (**b).into());

                        a.dist(&color)
                            .partial_cmp(&b.dist(&color))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .expect("palette is not empty");

                surface.set_pixel(p, *nearest);
            }
        }
    }
}

pub struct Silhouette;

impl FilterKind for Silhouette {
    fn id(&self) -> &'static str {
        "silhouette"
    }

    fn name(&self) -> &'static str {
        "Silhouette"
    }

    fn params(&self) -> &'static [ParamSpec] {
        &[
            ParamSpec {
                id: "color",
                label: "color",
                kind: ParamKind::Color,
                default: Value::Color(BLACK),
                help: "what to fill with",
            },
            ParamSpec {
                id: "threshold",
                label: "threshold",
                kind: ParamKind::Int { min: 1, max: 255 },
                default: Value::Int(128),
                help: "how opaque a pixel must be to be filled",
            },
        ]
    }

    fn apply(&self, surface: &mut dyn Surface, params: &Params, _palette: &[Color]) {
        let fill = params.color("color", BLACK);
        let threshold = params.int("threshold", 128).clamp(0, 255) as u8;
        let size = surface.size();

        for i in 0..size.x {
            for j in 0..size.y {
                let p = Point::new(i, j);

                if surface.pixel(p).a >= threshold {
                    surface.set_pixel(p, fill);
                }
            }
        }
    }
}

// --- shared helpers ---------------------------------------------------------

/// A copy of an image's pixels, so a filter can read it as it was while writing
/// over the original.
///
/// Only ever read from: `set_pixel` is deliberately inert, since a snapshot
/// exists precisely to stay unchanged.
struct Snapshot {
    pixels: Vec<Color>,
    size: Size<i32>,
}

impl Snapshot {
    fn of(surface: &dyn Surface) -> Self {
        let size = surface.size();
        let mut pixels = Vec::with_capacity((size.x * size.y).max(0) as usize);

        for j in 0..size.y {
            for i in 0..size.x {
                pixels.push(surface.pixel(Point::new(i, j)));
            }
        }

        Self { pixels, size }
    }
}

impl Surface for Snapshot {
    fn size(&self) -> Size<i32> {
        self.size
    }

    fn pixel(&self, p: Point<i32>) -> Color {
        self.pixels[(p.y * self.size.x + p.x) as usize]
    }

    fn set_pixel(&mut self, _p: Point<i32>, _color: Color) {}
}

/// The color a pixel takes when smoothed: its 3x3 neighbourhood averaged.
///
/// Colors are weighted by their alpha and the result divided back out. Without
/// that, the fully transparent pixels around a sprite — which are usually
/// transparent *black* — would drag a dark fringe into its edges. Neighbours
/// beyond the edge are left out rather than counted as transparent, so the
/// border doesn't fade away.
pub fn smoothed_pixel(surface: &dyn Surface, p: Point<i32>) -> Color {
    let size = surface.size();
    let mut weighted_alpha = 0;
    let mut premultiplied = [0_u32; 3];
    let mut total_weight = 0;

    for (dj, row) in SMOOTH_KERNEL.iter().enumerate() {
        for (di, weight) in row.iter().enumerate() {
            let (x, y) = (p.x + di as i32 - 1, p.y + dj as i32 - 1);

            if x < 0 || y < 0 || x >= size.x || y >= size.y {
                continue;
            }

            let color = surface.pixel(Point::new(x, y));
            let alpha = color.a as u32;

            premultiplied[0] += color.r as u32 * alpha * weight;
            premultiplied[1] += color.g as u32 * alpha * weight;
            premultiplied[2] += color.b as u32 * alpha * weight;
            weighted_alpha += alpha * weight;
            total_weight += weight;
        }
    }

    if weighted_alpha == 0 {
        return TRANSPARENT;
    }

    let channel = |sum: u32| ((sum + weighted_alpha / 2) / weighted_alpha).min(255) as u8;

    Color::new(
        channel(premultiplied[0]),
        channel(premultiplied[1]),
        channel(premultiplied[2]),
        ((weighted_alpha + total_weight / 2) / total_weight) as u8,
    )
}

/// Blends between two colors, `t` running from all of `original` to all of
/// `other`.
///
/// Colors are weighted by their alpha and the result divided back out, so
/// mixing towards a transparent color fades rather than darkening.
fn mix(original: Color, other: Color, t: i32) -> Color {
    if t <= 0 {
        return original;
    }
    if t >= FULL_STRENGTH {
        return other;
    }

    let (t, inverse) = (t as u32, (FULL_STRENGTH - t) as u32);
    let (from_alpha, to_alpha) = (original.a as u32, other.a as u32);
    let alpha = (from_alpha * inverse + to_alpha * t) / 255;

    if alpha == 0 {
        return TRANSPARENT;
    }

    let channel = |from: u8, to: u8| {
        let premultiplied = from as u32 * from_alpha * inverse + to as u32 * to_alpha * t;

        ((premultiplied / 255 + alpha / 2) / alpha).min(255) as u8
    };

    Color::new(
        channel(original.r, other.r),
        channel(original.g, other.g),
        channel(original.b, other.b),
        alpha as u8,
    )
}

/// Convenience constructors for the built in kinds.
impl Filter {
    pub fn smooth() -> Self {
        Self::new(&Smooth)
    }

    pub fn apply_palette() -> Self {
        Self::new(&ApplyPalette)
    }

    pub fn silhouette() -> Self {
        Self::new(&Silhouette)
    }
}
