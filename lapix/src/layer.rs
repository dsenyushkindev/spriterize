use crate::color::TRANSPARENT;
use crate::{Bitmap, Canvas, Color, Filter, Generator, Point, Rect, Size};
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
    /// The frame being edited and shown. Every layer holds a cel for it.
    #[serde(default)]
    active_frame: usize,
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
            inner: vec![Layer::new(size, default_layer_name(1), 1)],
            active: 0,
            active_frame: 0,
            filters_enabled: enabled(),
            composite: empty_cache(),
        }
    }

    /// How many frames the project has. Every layer has this many cels.
    pub fn frame_count(&self) -> usize {
        self.inner[0].frame_count()
    }

    /// The frame being edited and shown
    pub fn active_frame(&self) -> usize {
        self.active_frame
    }

    /// The size a cel has, taken from the first layer's first frame
    fn cel_size(&self) -> Size<i32> {
        self.inner[0].canvas(0).size()
    }

    /// Switch to a different frame
    pub fn switch_frame(&mut self, frame: usize) {
        if frame < self.frame_count() {
            self.invalidate_composite();
            self.active_frame = frame;
        }
    }

    /// Add a blank frame after the last one, and switch to it. Returns its
    /// index.
    pub fn add_frame(&mut self) -> usize {
        self.invalidate_composite();
        let size = self.cel_size();

        for layer in &mut self.inner {
            layer.add_cel(size);
        }

        self.active_frame = self.frame_count() - 1;

        self.active_frame
    }

    /// Insert a frame's cels back at an index, for undoing a deletion. Each
    /// layer takes its cel from the list, in layer order.
    pub fn insert_frame(&mut self, frame: usize, cels: Vec<Cel<IMG>>) {
        self.invalidate_composite();

        for (layer, cel) in self.inner.iter_mut().zip(cels) {
            layer.insert_cel(frame, cel);
        }

        self.active_frame = frame.min(self.frame_count() - 1);
    }

    /// Insert a copy of a frame right after it, and switch to the copy. Returns
    /// the new frame's index.
    pub fn duplicate_frame(&mut self, frame: usize) -> usize {
        self.invalidate_composite();

        for layer in &mut self.inner {
            let copy = layer.duplicate_cel(frame);
            layer.insert_cel(frame + 1, copy);
        }

        self.active_frame = frame + 1;

        self.active_frame
    }

    /// Remove a frame, returning each layer's cel for it in layer order, so the
    /// deletion can be undone. Does nothing and returns `None` if it is the
    /// only frame.
    pub fn remove_frame(&mut self, frame: usize) -> Option<Vec<Cel<IMG>>> {
        if self.frame_count() <= 1 {
            return None;
        }

        self.invalidate_composite();
        let cels = self
            .inner
            .iter_mut()
            .map(|layer| layer.remove_cel(frame))
            .collect();

        self.active_frame = self.active_frame.min(self.frame_count() - 1);

        Some(cels)
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

    /// Blends the active frame bottom to top, letting each adjustment layer
    /// filter everything that has accumulated beneath it.
    fn flatten(&self, palette: &[Color]) -> IMG {
        self.flatten_frame(self.active_frame, palette)
    }

    /// Blends one frame's stack into a single image. The active frame's result
    /// is what [`composite`](Self::composite) caches; any frame can be flattened
    /// this way for export or playback.
    pub fn flatten_frame(&self, frame: usize, palette: &[Color]) -> IMG {
        let mut result = IMG::new(self.cel_size(), TRANSPARENT);

        for i in 0..self.count() {
            let layer = self.get(i);

            if !layer.visible() {
                continue;
            }

            if layer.is_adjustment() {
                self.apply_adjustment(&mut result, layer, palette);
                continue;
            }

            let rendered = if self.filters_enabled {
                layer.rendered(frame, palette)
            } else {
                Rendered::Source(layer.canvas(frame).inner())
            };

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

        self.inner[index].rendered(self.active_frame, palette)
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

    /// Replace a layer's generator recipe, returning the one it had. Metadata
    /// only — the pixels are set separately, so nothing to invalidate here.
    pub fn set_generator(
        &mut self,
        index: usize,
        generator: Option<Generator>,
    ) -> Option<Generator> {
        self.inner[index].set_generator(generator)
    }

    /// Drop every layer's filtered image, so they are worked out again. Needed
    /// when something outside the layers changes the result, such as the
    /// palette a filter maps onto.
    pub fn invalidate_filters(&mut self) {
        self.invalidate_composite();
        for layer in &mut self.inner {
            layer.invalidate_all();
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

    /// Get the active frame's [`Canvas`] for the [`Layer`] at the specified
    /// index
    pub fn canvas_at(&self, index: usize) -> &Canvas<IMG> {
        self.inner[index].canvas(self.active_frame)
    }

    /// Get the active frame's [`Canvas`] for the active [`Layer`]
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

    /// Get a mutable reference to the active frame's [`Canvas`] for the
    /// [`Layer`] at a certain index
    pub fn canvas_at_mut(&mut self, index: usize) -> &mut Canvas<IMG> {
        self.invalidate_composite();
        let frame = self.active_frame;
        self.inner[index].canvas_mut(frame)
    }

    /// Get a mutable reference to a specific frame's [`Canvas`] for a layer,
    /// for replaying an edit onto the frame it was made on rather than whichever
    /// is active now
    pub fn cel_mut(&mut self, index: usize, frame: usize) -> &mut Canvas<IMG> {
        self.invalidate_composite();
        self.inner[index].canvas_mut(frame)
    }

    /// Replace a frame's image for a layer, returning the one it had
    pub fn set_cel_img(&mut self, index: usize, frame: usize, img: IMG) -> IMG {
        self.invalidate_composite();
        let old = self.inner[index].take_img(frame);
        self.inner[index].canvas_mut(frame).set_img(img);

        old
    }

    /// Get a mutable reference to the active frame's [`Canvas`] for the active
    /// [`Layer`]
    pub fn active_canvas_mut(&mut self) -> &mut Canvas<IMG> {
        self.invalidate_composite();
        let (active, frame) = (self.active, self.active_frame);
        self.inner[active].canvas_mut(frame)
    }

    /// Resize every frame of every [`Layer`], returning the images that were
    /// there before, tagged with their layer and frame (used for undoing)
    pub fn resize_all(&mut self, size: Size<i32>) -> Vec<(usize, usize, IMG)> {
        self.invalidate_composite();
        let mut imgs = Vec::new();

        for (layer_index, layer) in self.inner.iter_mut().enumerate() {
            for (frame, img) in layer.resize(size).into_iter().enumerate() {
                imgs.push((layer_index, frame, img));
            }
        }

        imgs
    }

    /// Set the active [`Layer`] to the specified index
    pub fn switch_to(&mut self, index: usize) {
        self.invalidate_composite();
        self.active = index;
    }

    /// Add a new [`Layer`] above all layers, with a cel for every frame
    pub fn add_new_above(&mut self) {
        self.invalidate_composite();
        let layer = Layer::new(
            self.cel_size(),
            self.unused_default_name(),
            self.frame_count(),
        );
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

/// One layer's pixels for one frame, together with the cached result of running
/// the layer's filters over them.
///
/// A layer holds one of these per frame. The filtered result is cached per cel
/// because each frame's pixels differ; it is not saved, since it can always be
/// worked out again from the pixels and the chain.
#[derive(Debug, Serialize, Deserialize)]
pub struct Cel<IMG> {
    canvas: Canvas<IMG>,
    #[serde(skip, default = "empty_cache")]
    cache: RefCell<Option<IMG>>,
}

impl<IMG: Bitmap> Cel<IMG> {
    fn new(size: Size<i32>) -> Self {
        Self::from_canvas(Canvas::new(size))
    }

    fn from_canvas(canvas: Canvas<IMG>) -> Self {
        Self {
            canvas,
            cache: RefCell::new(None),
        }
    }

    /// A separate cel with the same pixels, for duplicating a frame.
    fn duplicate(&self) -> Self {
        let mut canvas = Canvas::new(self.canvas.size());
        canvas.set_img(self.canvas.inner().clone());

        Self::from_canvas(canvas)
    }

    fn invalidate(&mut self) {
        *self.cache.get_mut() = None;
    }
}

/// Spelled out rather than derived, so cels don't need their image type to
/// implement `Default` just to have somewhere to keep the filtered result.
fn empty_cache<IMG>() -> RefCell<Option<IMG>> {
    RefCell::new(None)
}

/// A layer of the canvas: a stack of these, blended by transparency, makes the
/// picture. Layers can be reordered, hidden, given an opacity and a filter
/// chain, or turned into adjustment layers.
///
/// A layer's pixels are stored per frame as [`Cel`]s: the name, order,
/// visibility, opacity and filters are shared across every frame, while each
/// frame has its own pixels. Every layer in a project holds the same number of
/// cels.
#[derive(Debug, Serialize, Deserialize)]
pub struct Layer<IMG> {
    cels: Vec<Cel<IMG>>,
    visible: bool,
    opacity: u8,
    /// What the user calls this layer. Also the file name it gets when layers
    /// are exported separately.
    name: String,
    /// Applied in order to produce what is shown, leaving the pixels untouched.
    filters: Vec<Filter>,
    /// When set, this layer draws nothing of its own: its filters apply to
    /// everything stacked below it instead.
    adjustment: bool,
    /// The recipe that produced this layer's pixels, if any — kept so it can be
    /// re-run and re-tuned. `default` so projects saved before generators load.
    #[serde(default)]
    generator: Option<Generator>,
}

impl<IMG: Bitmap> Layer<IMG> {
    /// Create a new layer with a specified size, name and number of frames
    pub fn new(size: Size<i32>, name: impl Into<String>, frames: usize) -> Self {
        Self {
            cels: (0..frames.max(1)).map(|_| Cel::new(size)).collect(),
            visible: true,
            opacity: 255,
            name: name.into(),
            filters: Vec::new(),
            adjustment: false,
            generator: None,
        }
    }

    /// How many frames this layer holds a cel for
    pub fn frame_count(&self) -> usize {
        self.cels.len()
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
        // The chain is shared by every frame, so every cel's cached result is
        // now stale.
        self.invalidate_all();

        std::mem::replace(&mut self.filters, filters)
    }

    /// The generator recipe that fills this layer, if it has one.
    pub fn generator(&self) -> Option<&Generator> {
        self.generator.as_ref()
    }

    /// Replace this layer's generator recipe, returning the one it had. The
    /// recipe is only metadata; the pixels it produced are set separately.
    pub fn set_generator(&mut self, generator: Option<Generator>) -> Option<Generator> {
        std::mem::replace(&mut self.generator, generator)
    }

    /// This layer's image for one frame as it should be seen, with its filters
    /// applied.
    ///
    /// The result is kept until the pixels or the chain change, so this is
    /// cheap to call repeatedly — for every pixel of a composite, say.
    pub fn rendered(&self, frame: usize, palette: &[Color]) -> Rendered<'_, IMG> {
        let cel = &self.cels[frame];

        if self.filters.is_empty() {
            return Rendered::Source(cel.canvas.inner());
        }

        if cel.cache.borrow().is_none() {
            let mut img = cel.canvas.inner().clone();

            for filter in &self.filters {
                filter.apply(&mut img, palette);
            }

            *cel.cache.borrow_mut() = Some(img);
        }

        Rendered::Filtered(Ref::map(cel.cache.borrow(), |cached| {
            cached.as_ref().expect("just computed")
        }))
    }

    /// Throw away every cel's filtered image, so they are worked out again.
    pub fn invalidate_all(&mut self) {
        for cel in &mut self.cels {
            cel.invalidate();
        }
    }

    /// The name of this layer
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Rename this layer
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// Get the [`Canvas`] holding this layer's pixels for a frame
    pub fn canvas(&self, frame: usize) -> &Canvas<IMG> {
        &self.cels[frame].canvas
    }

    /// Get a mutable reference to a frame's [`Canvas`]
    ///
    /// Handing out the canvas for writing means its filtered image can no
    /// longer be trusted, so it is dropped here rather than trying to catch
    /// every individual edit.
    pub fn canvas_mut(&mut self, frame: usize) -> &mut Canvas<IMG> {
        self.cels[frame].invalidate();

        &mut self.cels[frame].canvas
    }

    /// Whether this layer is visible
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Get the opacity level (alpha) of this layer, a value from 0-255
    pub fn opacity(&self) -> u8 {
        self.opacity
    }

    /// Take the image of a frame's [`Canvas`], leaving a dummy empty one in its
    /// place
    pub fn take_img(&mut self, frame: usize) -> IMG {
        self.cels[frame].invalidate();
        self.cels[frame].canvas.take_inner()
    }

    /// Resize every frame's [`Canvas`], returning the previous images in frame
    /// order (for undoing)
    pub fn resize(&mut self, size: Size<i32>) -> Vec<IMG> {
        self.cels
            .iter_mut()
            .map(|cel| {
                cel.invalidate();
                cel.canvas.resize(size)
            })
            .collect()
    }

    /// Set whether this layer is visible
    pub fn set_visibility(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Set the opacity of this layer
    pub fn set_opacity(&mut self, opacity: u8) {
        self.opacity = opacity;
    }

    /// Add a blank cel for a new frame
    fn add_cel(&mut self, size: Size<i32>) {
        self.cels.push(Cel::new(size));
    }

    /// Insert a cel at a frame index, for restoring a removed frame
    fn insert_cel(&mut self, frame: usize, cel: Cel<IMG>) {
        self.cels.insert(frame, cel);
    }

    /// A copy of a frame's cel, for duplicating a frame
    fn duplicate_cel(&self, frame: usize) -> Cel<IMG> {
        self.cels[frame].duplicate()
    }

    /// Remove and return a frame's cel
    fn remove_cel(&mut self, frame: usize) -> Cel<IMG> {
        self.cels.remove(frame)
    }
}
