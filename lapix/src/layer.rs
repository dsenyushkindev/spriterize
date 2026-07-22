use crate::color::TRANSPARENT;
use crate::{Bitmap, Canvas, Color, Filter, Point, Rect, Size};
use serde::{Deserialize, Serialize};
use std::cell::{Ref, RefCell};

/// A layer's image as it should be seen: either the pixels it stores, when it
/// has nothing to apply, or the result of running its filters over them.
///
/// Borrowing the source directly in the common case keeps unfiltered layers
/// from paying for a copy on every edit.
pub enum Rendered<'a, IMG> {
    Source(&'a IMG),
    Filtered(Ref<'a, IMG>),
}

impl<IMG> std::ops::Deref for Rendered<'_, IMG> {
    type Target = IMG;

    fn deref(&self) -> &IMG {
        match self {
            Self::Source(img) => img,
            Self::Filtered(img) => img,
        }
    }
}

/// Name given to the nth layer when one is created without the user naming it.
fn default_layer_name(n: usize) -> String {
    format!("Layer {n}")
}

/// An ordered collection of [`Layer`]s. There is always one active layer.
#[derive(Debug, Serialize, Deserialize)]
pub struct Layers<IMG> {
    inner: Vec<Layer<IMG>>,
    active: usize,
    /// Whether filters are applied at all. Turning them off shows the pixels as
    /// they are stored, which is a view setting rather than part of the
    /// drawing, so it isn't saved with the project.
    #[serde(skip, default = "enabled")]
    filters_enabled: bool,
    /// All the layers flattened into one image, kept until anything changes.
    ///
    /// An adjustment layer filters what is beneath it, and filters look at
    /// neighbouring pixels, so the result can't be worked out one pixel at a
    /// time — the stack has to be flattened before anything can be read from
    /// it.
    #[serde(skip, default = "empty_cache")]
    composite: RefCell<Option<IMG>>,
}

fn enabled() -> bool {
    true
}

impl<IMG: Bitmap> Layers<IMG> {
    /// Creates a new set of layers
    pub fn new(size: Size<i32>) -> Self {
        Self {
            inner: vec![Layer::new(size, default_layer_name(1))],
            active: 0,
            filters_enabled: enabled(),
            composite: empty_cache(),
        }
    }

    /// Throw away the flattened image, so it is built again on the next read.
    fn invalidate_composite(&mut self) {
        *self.composite.get_mut() = None;
    }

    /// Every layer flattened into one image, as it should be seen.
    ///
    /// Kept until something changes, so this is cheap to call repeatedly.
    pub fn composite(&self, palette: &[Color]) -> Ref<'_, IMG> {
        if self.composite.borrow().is_none() {
            let flattened = self.flatten(palette);

            *self.composite.borrow_mut() = Some(flattened);
        }

        Ref::map(self.composite.borrow(), |cached| {
            cached.as_ref().expect("just built")
        })
    }

    /// Blends the stack bottom to top, letting each adjustment layer filter
    /// everything that has accumulated beneath it.
    fn flatten(&self, palette: &[Color]) -> IMG {
        let mut result = IMG::new(self.canvas_at(0).size(), TRANSPARENT);

        for i in 0..self.count() {
            let layer = self.get(i);

            if !layer.visible() {
                continue;
            }

            if layer.is_adjustment() {
                self.apply_adjustment(&mut result, layer, palette);
                continue;
            }

            let rendered = self.rendered(i, palette);

            for x in 0..result.width() {
                for y in 0..result.height() {
                    let p = Point::new(x, y);
                    let color = rendered.pixel(p).with_multiplied_alpha(layer.opacity());

                    result.set_pixel(p, color.blend_over(result.pixel(p)));
                }
            }
        }

        result
    }

    /// Runs an adjustment layer's filters over what is below it, mixed in by
    /// its opacity so the effect can be dialled back.
    fn apply_adjustment(&self, result: &mut IMG, layer: &Layer<IMG>, palette: &[Color]) {
        if !self.filters_enabled || layer.filters().is_empty() {
            return;
        }

        let mut filtered = result.clone();

        for filter in layer.filters() {
            filter.apply(&mut filtered, palette);
        }

        for x in 0..result.width() {
            for y in 0..result.height() {
                let p = Point::new(x, y);
                let color = filtered.pixel(p).with_multiplied_alpha(layer.opacity());

                result.set_pixel(p, color.blend_over(result.pixel(p)));
            }
        }
    }

    /// Whether layer filters are being applied
    pub fn filters_enabled(&self) -> bool {
        self.filters_enabled
    }

    /// Show the layers with their filters applied, or as they are stored
    pub fn set_filters_enabled(&mut self, enabled: bool) {
        self.invalidate_composite();
        self.filters_enabled = enabled;
    }

    /// The image of a layer as it should be seen: filtered, unless filters are
    /// switched off.
    ///
    /// Everything that displays, exports or samples a layer goes through this,
    /// so what is picked up by the eyedropper always matches what is on screen.
    pub fn rendered(&self, index: usize, palette: &[Color]) -> Rendered<'_, IMG> {
        if !self.filters_enabled {
            return Rendered::Source(self.canvas_at(index).inner());
        }

        self.inner[index].rendered(palette)
    }

    /// Replace a layer's filter chain, returning the one it had
    pub fn set_filters(&mut self, index: usize, filters: Vec<Filter>) -> Vec<Filter> {
        self.invalidate_composite();

        self.inner[index].set_filters(filters)
    }

    /// Make a layer filter what is below it, or go back to being drawn on.
    /// Returns what it was.
    pub fn set_adjustment(&mut self, index: usize, adjustment: bool) -> bool {
        self.invalidate_composite();

        self.inner[index].set_adjustment(adjustment)
    }

    /// Drop every layer's filtered image, so they are worked out again. Needed
    /// when something outside the layers changes the result, such as the
    /// palette a filter maps onto.
    pub fn invalidate_filters(&mut self) {
        self.invalidate_composite();
        for layer in &mut self.inner {
            layer.invalidate();
        }
    }

    /// Get the active [`Layer`]
    pub fn active(&self) -> &Layer<IMG> {
        &self.inner[self.active]
    }

    /// Get the index of the active [`Layer`]
    pub fn active_index(&self) -> usize {
        self.active
    }

    /// Get the number of [`Layer`]s
    pub fn count(&self) -> usize {
        self.inner.len()
    }

    /// Get the [`Canvas`] of the [`Layer`] at the specified index
    pub fn canvas_at(&self, index: usize) -> &Canvas<IMG> {
        self.inner[index].canvas()
    }

    /// Get the [`Canvas`] of the active [`Layer`]
    pub fn active_canvas(&self) -> &Canvas<IMG> {
        self.canvas_at(self.active)
    }

    /// Get a [`Layer`] by its index
    pub fn get(&self, index: usize) -> &Layer<IMG> {
        &self.inner[index]
    }

    /// Get an image of all the [`Layer`]s blended together
    pub fn blended(&self, palette: &[Color]) -> IMG {
        self.composite(palette).clone()
    }

    /// Get an image of an area (determined by a rectangle) of all [`Layer`]s
    /// blended together
    pub fn blended_area(&self, r: Rect<i32>, palette: &[Color]) -> IMG {
        let composite = self.composite(palette);
        let mut result = IMG::new((r.w, r.h).into(), TRANSPARENT);

        for i in 0..r.w {
            for j in 0..r.h {
                let ij = Point::new(i, j);

                result.set_pixel(ij, composite.pixel(ij + r.pos()));
            }
        }

        result
    }

    /// Get a mutable reference to a [`Layer`] by its index
    pub fn get_mut(&mut self, index: usize) -> &mut Layer<IMG> {
        self.invalidate_composite();
        &mut self.inner[index]
    }

    /// Get a mutable reference to the [`Canvas`] of the [`Layer`] at a certain
    /// index
    pub fn canvas_at_mut(&mut self, index: usize) -> &mut Canvas<IMG> {
        self.invalidate_composite();
        self.inner[index].canvas_mut()
    }

    /// Get a mutable reference to the [`Canvas`] of the active [`Layer`]
    pub fn active_canvas_mut(&mut self) -> &mut Canvas<IMG> {
        self.invalidate_composite();
        self.inner[self.active].canvas_mut()
    }

    /// Resize all [`Layer`]s, returning the images that were there before the
    /// resizing (used for undoing)
    pub fn resize_all(&mut self, size: Size<i32>) -> Vec<IMG> {
        self.invalidate_composite();
        let mut imgs = Vec::new();
        for layer in self.inner.iter_mut() {
            let img = layer.resize(size);
            imgs.push(img);
        }

        imgs
    }

    /// Set the active [`Layer`] to the specified index
    pub fn switch_to(&mut self, index: usize) {
        self.invalidate_composite();
        self.active = index;
    }

    /// Add a new [`Layer`] above all layers
    pub fn add_new_above(&mut self) {
        self.invalidate_composite();
        let layer = Layer::new(self.active_canvas().size(), self.unused_default_name());
        self.inner.push(layer);
    }

    /// Rename the [`Layer`] at the specified index
    pub fn set_name(&mut self, index: usize, name: impl Into<String>) {
        self.invalidate_composite();
        self.inner[index].set_name(name);
    }

    /// The lowest numbered default name no layer is using, so adding layers
    /// after deleting some doesn't produce two with the same name.
    fn unused_default_name(&self) -> String {
        (1..)
            .map(default_layer_name)
            .find(|name| !self.inner.iter().any(|layer| layer.name() == name))
            .expect("there is always an unused name")
    }

    /// Add a new [`Layer`] at the specified index
    pub fn add_at(&mut self, index: usize, layer: Layer<IMG>) {
        self.invalidate_composite();
        self.inner.insert(index, layer);
    }

    /// Delete the [`Layer`] at the specified index
    pub fn delete(&mut self, index: usize) -> Layer<IMG> {
        self.invalidate_composite();
        let layer = self.inner.remove(index);
        self.active = self.active.clamp(0, self.count() - 1);

        layer
    }

    /// Set whether the [`Layer`] at the specified index is visible or not
    pub fn set_visibility(&mut self, index: usize, visible: bool) {
        self.invalidate_composite();
        self.inner[index].set_visibility(visible);
    }

    /// Set the opacity (alpha) of the [`Layer`] at the specified index
    pub fn set_opacity(&mut self, index: usize, opacity: u8) {
        self.invalidate_composite();
        self.inner[index].set_opacity(opacity);
    }

    /// Swap the positions of two [`Layer`]s
    pub fn swap(&mut self, first: usize, second: usize) {
        self.invalidate_composite();
        self.inner.swap(first, second);
    }

    // TODO: maybe Canvas is a better name for Layers than for that type, since
    // the canvas is a combination of all layers, not a single layer's image
    /// Get the color of the visible pixel at a certain [`Point`] in the canvas,
    /// considering the blended result of all layers with their visibility and
    /// opacity settings
    pub fn visible_pixel(&self, p: Point<i32>, palette: &[Color]) -> Color {
        self.composite(palette).pixel(p)
    }
}

/// Represents a layer of the canvas. Layers are stacked on top of each other to
/// make a final image, blending colors with transparency. Layers can be moved
/// up or down relative to each other, can be made invisible or have a level of
/// transparency (opacity).
#[derive(Debug, Serialize, Deserialize)]
pub struct Layer<IMG> {
    canvas: Canvas<IMG>,
    visible: bool,
    opacity: u8,
    /// What the user calls this layer. Also the file name it gets when layers
    /// are exported separately.
    name: String,
    /// Applied in order to produce what is shown, leaving `canvas` untouched.
    filters: Vec<Filter>,
    /// When set, this layer draws nothing of its own: its filters apply to
    /// everything stacked below it instead.
    adjustment: bool,
    /// The result of the filters, kept until something invalidates it. Not
    /// saved: it can always be worked out again from the pixels and the chain.
    #[serde(skip, default = "empty_cache")]
    cache: RefCell<Option<IMG>>,
}

/// Spelled out rather than derived, so layers don't need their image type to
/// implement `Default` just to have somewhere to keep the filtered result.
fn empty_cache<IMG>() -> RefCell<Option<IMG>> {
    RefCell::new(None)
}

impl<IMG: Bitmap> Layer<IMG> {
    /// Create a new layer with a specified size and name
    pub fn new(size: Size<i32>, name: impl Into<String>) -> Self {
        Self {
            canvas: Canvas::new(size),
            visible: true,
            opacity: 255,
            name: name.into(),
            filters: Vec::new(),
            adjustment: false,
            cache: RefCell::new(None),
        }
    }

    /// Whether this layer filters what is below it rather than drawing its own
    /// pixels
    pub fn is_adjustment(&self) -> bool {
        self.adjustment
    }

    /// Make this layer filter what is below it, or go back to being drawn on.
    /// Returns what it was.
    pub fn set_adjustment(&mut self, adjustment: bool) -> bool {
        std::mem::replace(&mut self.adjustment, adjustment)
    }

    /// The filters applied to this layer, in the order they run
    pub fn filters(&self) -> &[Filter] {
        &self.filters
    }

    /// Replace this layer's filter chain, returning the one it had
    pub fn set_filters(&mut self, filters: Vec<Filter>) -> Vec<Filter> {
        self.invalidate();

        std::mem::replace(&mut self.filters, filters)
    }

    /// This layer's image as it should be seen, with its filters applied.
    ///
    /// The result is kept until the pixels or the chain change, so this is
    /// cheap to call repeatedly — for every pixel of a composite, say.
    pub fn rendered(&self, palette: &[Color]) -> Rendered<'_, IMG> {
        if self.filters.is_empty() {
            return Rendered::Source(self.canvas.inner());
        }

        if self.cache.borrow().is_none() {
            let mut img = self.canvas.inner().clone();

            for filter in &self.filters {
                filter.apply(&mut img, palette);
            }

            *self.cache.borrow_mut() = Some(img);
        }

        Rendered::Filtered(Ref::map(self.cache.borrow(), |cached| {
            cached.as_ref().expect("just computed")
        }))
    }

    /// Throw away the filtered image, so the next read works it out again.
    pub fn invalidate(&mut self) {
        *self.cache.get_mut() = None;
    }

    /// The name of this layer
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Rename this layer
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// Get the [`Canvas`] of this layer
    pub fn canvas(&self) -> &Canvas<IMG> {
        &self.canvas
    }

    /// Get a mutable reference to the [`Canvas`] of this layer
    ///
    /// Handing out the canvas for writing means the filtered image can no
    /// longer be trusted, so it is dropped here rather than trying to catch
    /// every individual edit.
    pub fn canvas_mut(&mut self) -> &mut Canvas<IMG> {
        self.invalidate();

        &mut self.canvas
    }

    /// Whether this layer is visible
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Get the opacity level (alpha) of this layer, a value from 0-255
    pub fn opacity(&self) -> u8 {
        self.opacity
    }

    /// Take the image of this layer's [`Canvas`], leaving a dummy empty one in
    /// its place
    pub fn take_img(&mut self) -> IMG {
        self.canvas.take_inner()
    }

    /// Resize this layer, returning the previous image (the image before the
    /// resizing)
    pub fn resize(&mut self, size: Size<i32>) -> IMG {
        self.canvas.resize(size)
    }

    /// Set whether this layer is visible
    pub fn set_visibility(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Set the opacity of this layer
    pub fn set_opacity(&mut self, opacity: u8) {
        self.opacity = opacity;
    }
}
