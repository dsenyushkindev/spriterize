use crate::color::{BLACK, TRANSPARENT};
use crate::util::{LoadProject, SaveProject};
use crate::{
    export, graphics, util, Action, AtomicAction, Bitmap, Canvas, CanvasEffect, Color, Error,
    Event, ExportOptions, FreeImage, Generator, Layers, Palette, Point, Position, Rect, Result,
    Size, Tool,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// A single pixel, the brush size pixel art is usually drawn at.
fn default_brush_radius() -> u8 {
    0
}

/// How many past events to keep. Only the most recent is ever read, so a short
/// tail is plenty; the margin is there in case some future continuity check
/// wants to look back a few.
const MAX_EVENT_HISTORY: usize = 16;

/// Characters that can't appear in a file name on Windows, and are worth
/// avoiding elsewhere too.
const RESERVED_CHARS: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// Turns a layer name into something safe to use as a file name, falling back
/// to the layer's position when nothing usable is left.
fn file_name_from(layer_name: &str, index: usize) -> String {
    let cleaned: String = layer_name
        .chars()
        .map(|c| {
            if RESERVED_CHARS.contains(&c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();

    // Windows also refuses names ending in a dot or a space.
    let cleaned = cleaned.trim().trim_end_matches('.').trim();

    if cleaned.is_empty() {
        format!("layer_{}", index + 1)
    } else {
        cleaned.to_owned()
    }
}

/// Keeps exported file names distinct, so two layers sharing a name don't
/// silently overwrite each other.
fn unique_file_name(taken: &mut HashSet<String>, layer_name: &str, index: usize) -> String {
    let base = file_name_from(layer_name, index);

    if taken.insert(base.to_lowercase()) {
        return base;
    }

    for suffix in 2.. {
        let candidate = format!("{base}_{suffix}");

        if taken.insert(candidate.to_lowercase()) {
            return candidate;
        }
    }

    unreachable!("a free name is always found")
}

/// Represents a selection
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Selection {
    /// A selected portion of the canvas, based on a rectangular area
    Canvas(Rect<i32>),
    // TODO: maybe this should contain the FreeImage
    /// A selected free image that is not part of the canvas until it's
    /// *anchored*
    FreeImage,
}

/// The state of the image editor's core. Most importantly, this contains all
/// the layers and images that are being drawn. This state can be modified
/// externally mainly by sending [`Event`]s via the [`execute`] method.
///
/// [`execute`]: State::execute
#[derive(Debug, Serialize, Deserialize)]
pub struct State<IMG> {
    layers: Layers<IMG>,
    #[serde(skip)]
    events: Vec<Event>,
    tool: Tool,
    main_color: Color,
    /// Radius of the stroke the drawing tools lay down, in pixels: the brush
    /// and eraser, and the line, rectangle and ellipse outlines. A tool setting
    /// rather than part of the drawing, so it is deliberately left out of saved
    /// projects — which also keeps the format readable by older versions.
    #[serde(skip, default = "default_brush_radius")]
    brush_radius: u8,
    spritesheet: Size<u8>,
    palette: Palette,
    selection: Option<Selection>,
    free_image: Option<FreeImage<IMG>>,
    clipboard: Option<IMG>,
    #[serde(skip, default = "Vec::new")]
    reversals: Vec<Action<IMG>>,
    #[serde(skip, default = "Vec::new")]
    redos: Vec<Action<IMG>>,
    #[serde(skip, default = "Option::default")]
    cur_reversal: Option<Action<IMG>>,
    #[serde(skip, default = "Option::default")]
    load_project_fn: Option<LoadProject>,
    #[serde(skip, default = "Option::default")]
    save_project_fn: Option<SaveProject>,
}

impl<IMG: Bitmap + Serialize + for<'de> Deserialize<'de>> State<IMG> {
    /// Create a new default state for the editor, with a starting canvas size
    pub fn new(
        size: Size<i32>,
        load_project_fn: Option<LoadProject>,
        save_project_fn: Option<SaveProject>,
    ) -> Self {
        Self {
            layers: Layers::new(size),
            events: Vec::new(),
            tool: Tool::Brush,
            main_color: BLACK,
            brush_radius: default_brush_radius(),
            spritesheet: Size::new(1, 1),
            palette: Palette::default(),
            selection: None,
            free_image: None,
            clipboard: None,
            reversals: Vec::new(),
            redos: Vec::new(),
            cur_reversal: None,
            load_project_fn,
            save_project_fn,
        }
    }

    fn start_action(&mut self) {
        self.cur_reversal = Some(Action::default());
    }

    fn add_to_action(&mut self, actions: Vec<AtomicAction<IMG>>) -> Result<()> {
        if self.cur_reversal.is_none() {
            self.start_action();
        }
        self.cur_reversal
            .as_mut()
            .ok_or(Error::ReversalNotSet)?
            .append(actions);

        Ok(())
    }

    fn end_action(&mut self) {
        if let Some(action) = self.cur_reversal.take() {
            self.reversals.push(action);
            // A new edit invalidates anything that was undone before it.
            self.redos.clear();
        }
    }

    fn single_action(&mut self, action: Action<IMG>) {
        self.end_action();
        self.reversals.push(action);
        self.redos.clear();
    }

    fn add_to_pixels_action(&mut self, actions: Vec<(Point<i32>, Color)>) -> Result<()> {
        let (layer, frame) = (self.layers.active_index(), self.layers.active_frame());
        let actions = AtomicAction::set_pixel_vec(layer, frame, actions);

        self.add_to_action(actions)
    }

    fn single_pixels_action(&mut self, actions: Vec<(Point<i32>, Color)>) {
        let (layer, frame) = (self.layers.active_index(), self.layers.active_frame());
        let actions = AtomicAction::set_pixel_vec(layer, frame, actions);
        self.single_action(actions.into());
    }

    /// Execute an [`Event`]. This is the main way of changing the editor's
    /// state, and probably the most central method of this library. A
    /// [`CanvasEffect`] is returned to communicate to the caller what kind of
    /// visual updates must be made.
    pub fn execute(&mut self, event: Event) -> Result<CanvasEffect> {
        if let Some(prev_event) = self.events.last() {
            if (prev_event == &event && !event.repeatable())
                || (event.same_variant(prev_event) && !event.type_repeatable())
            {
                return Ok(CanvasEffect::None);
            }
        }

        dbg!(&event);
        let t0 = std::time::SystemTime::now();

        if event.triggers_anchoring() {
            self.anchor()?;
        }

        let mut skip_event = false;

        match event.clone() {
            Event::ClearCanvas => {
                let (layer, frame) = (self.layers.active_index(), self.layers.active_frame());
                let img = self.canvas_mut().clear();
                let reversal = AtomicAction::SetLayerCanvas(layer, frame, img);
                self.start_action();
                self.add_to_action(vec![reversal])?;
                self.end_action();
            }
            Event::ResizeCanvas(size) => {
                self.start_action();
                // Resizing changes every frame of every layer, so each cel's
                // old image is a separate reversal.
                let imgs = self.resize_canvas(size);
                self.add_to_action(
                    imgs.into_iter()
                        .map(|(layer, frame, img)| AtomicAction::SetLayerCanvas(layer, frame, img))
                        .collect(),
                )?;
                self.end_action();
            }
            Event::AddFrame => {
                let frame = self.layers.add_frame();
                self.start_action();
                // The new frame is blank, so its reversal just removes it.
                self.add_to_action(vec![AtomicAction::RemoveFrame(frame)])?;
                self.end_action();
            }
            Event::DuplicateFrame(frame) => {
                let added = self.layers.duplicate_frame(frame);
                self.start_action();
                self.add_to_action(vec![AtomicAction::RemoveFrame(added)])?;
                self.end_action();
            }
            Event::DeleteFrame(frame) => {
                if let Some(cels) = self.layers.remove_frame(frame) {
                    self.start_action();
                    self.add_to_action(vec![AtomicAction::InsertFrame(frame, cels)])?;
                    self.end_action();
                } else {
                    skip_event = true;
                }
            }
            Event::SwitchFrame(frame) => self.layers.switch_frame(frame),
            Event::LineStart(_) | Event::RectStart(_) | Event::EllipseStart(_) => (),
            Event::BrushStart | Event::EraseStart | Event::SmoothStart => self.start_action(),
            Event::BrushEnd | Event::EraseEnd | Event::SmoothEnd => self.end_action(),
            Event::SmoothStroke(p) => {
                let radius = self.brush_radius;
                let last_event = self.events.last();

                let reversals = match last_event {
                    // Softening along the whole segment, so a quick drag doesn't
                    // leave gaps between stamps.
                    Some(Event::SmoothStroke(p0)) => {
                        let p0 = *p0;

                        graphics::line(p0, p)
                            .into_iter()
                            .flat_map(|centre| self.canvas_mut().smooth(centre, radius))
                            .collect()
                    }
                    Some(Event::SmoothStart) => self.canvas_mut().smooth(p, radius),
                    _ => Vec::new(),
                };
                self.add_to_pixels_action(reversals)?;
            }
            Event::LineEnd(p) => {
                let last_event = self.events.last();
                let p0 = match last_event {
                    Some(Event::LineStart(p0)) => *p0,
                    _ => return Err(Error::DrawingNotStarted),
                };
                let color = self.main_color;
                let radius = self.brush_radius;
                let reversals = self.canvas_mut().line(p0, p, color, radius);
                self.single_pixels_action(reversals);
                self.free_image = None;
            }
            Event::RectEnd(p) => {
                let last_event = self.events.last();
                let p0: Point<i32> = match last_event {
                    Some(Event::RectStart(p0)) => *p0,
                    _ => return Err(Error::DrawingNotStarted),
                };
                let color = self.main_color;
                let radius = self.brush_radius;
                let reversals = self.canvas_mut().rectangle(p0, p, color, radius);
                self.single_pixels_action(reversals);
                self.free_image = None;
            }
            Event::EllipseEnd(p) => {
                let last_event = self.events.last();
                let p0: Point<i32> = match last_event {
                    Some(Event::EllipseStart(p0)) => *p0,
                    _ => return Err(Error::DrawingNotStarted),
                };
                let color = self.main_color;
                let radius = self.brush_radius;
                let reversals = self.canvas_mut().ellipse(p0, p, color, radius);
                self.single_pixels_action(reversals);
                self.free_image = None;
            }
            Event::BrushStroke(p) => {
                let last_event = self.events.last();

                let radius = self.brush_radius;

                let reversals = match last_event {
                    Some(Event::BrushStroke(p0)) => {
                        let color = self.main_color;
                        let p0 = *p0;
                        self.canvas_mut().line(p0, p, color, radius)
                    }
                    Some(Event::BrushStart) => {
                        let color = self.main_color;
                        self.canvas_mut().brush(p, color, radius)
                    }
                    _ => Vec::new(),
                };
                self.add_to_pixels_action(reversals)?;
            }
            Event::Erase(p) => {
                let last_event = self.events.last();

                let radius = self.brush_radius;

                let reversals = match last_event {
                    Some(Event::Erase(p0)) => {
                        let p0 = *p0;
                        self.canvas_mut().line(p0, p, TRANSPARENT, radius)
                    }
                    Some(Event::EraseStart) => self.canvas_mut().brush(p, TRANSPARENT, radius),
                    _ => Vec::new(),
                };
                self.add_to_pixels_action(reversals)?;
            }
            Event::SetTool(tool) => self.tool = tool,
            Event::SetMainColor(color) => self.main_color = color,
            Event::SetBrushRadius(radius) => {
                self.brush_radius = radius.min(graphics::MAX_BRUSH_RADIUS)
            }
            Event::Save(path, options) => {
                self.save_image(path.to_string_lossy().as_ref(), &options)?
            }
            Event::ExportLayers(path, options) => self.export_layers(&path, &options)?,
            Event::ExportLayerSheet(path, cells, options) => {
                self.export_layer_sheet(&path, cells, &options)?
            }
            Event::ExportFrames(path, options) => self.export_frames(&path, &options)?,
            Event::ExportFrameSheet(path, cells, options) => {
                self.export_frame_sheet(&path, cells, &options)?
            }
            Event::OpenFile(path) => self.import_image(path.to_string_lossy().as_ref())?,
            Event::SaveProject(path) => {
                if let Some(f) = &self.save_project_fn {
                    let bytes = bincode::serialize(&self)?;
                    (f.0)(path, bytes);
                } else {
                    eprintln!("Bug: Missing save project function");
                }
            }
            Event::LoadProject(path) => {
                if let Some(f) = &self.load_project_fn {
                    let bytes = (f.0)(path);
                    let (save_fn, load_fn) =
                        (self.save_project_fn.take(), self.load_project_fn.take());
                    *self = bincode::deserialize(&bytes)?;
                    self.save_project_fn = save_fn;
                    self.load_project_fn = load_fn;
                } else {
                    eprintln!("Bug: Missing load project function");
                }
            }
            Event::LoadPalette(path) => {
                self.palette = Palette::from_file(path.to_string_lossy().as_ref())?;
                // A filter can map onto the palette, so its result is stale now.
                self.layers.invalidate_filters();
            }
            Event::SavePalette(path) => {
                self.palette.save_to_file(path.to_string_lossy().as_ref())?
            }
            Event::AddToPalette(color) => {
                self.palette.add_color(color);
                self.layers.invalidate_filters();
            }
            Event::RemoveFromPalette(color) => {
                self.palette.remove_color(color);
                self.layers.invalidate_filters();
            }
            Event::Bucket(p) => {
                if self.canvas().is_in_bounds(p) {
                    let color = self.main_color;
                    let reversals = self.canvas_mut().bucket(p, color);
                    self.single_pixels_action(reversals);
                }
            }
            Event::ClearSelection => (),
            Event::StartSelection(_) => (),
            Event::EndSelection(p) => {
                let last_event = self.events.last();

                if let Some(Event::StartSelection(p0)) = last_event {
                    let size = p.abs_diff(*p0);
                    let corner = p.rect_min_corner(*p0);
                    let rect = Rect::new(corner.x, corner.y, size.x + 1, size.y + 1);
                    let r = rect.clip_to(self.canvas().rect());
                    self.set_selection(Some(Selection::Canvas(r)))?;
                }
            }
            Event::Copy => match self.selection {
                Some(Selection::Canvas(rect)) => {
                    self.clipboard = Some(self.canvas().img_from_area(rect))
                }
                Some(Selection::FreeImage) => {
                    self.clipboard = Some(
                        self.free_image
                            .as_ref()
                            .ok_or(Error::MissingFreeImage)?
                            .texture
                            .clone(),
                    )
                }
                None => (),
            },
            Event::DeleteSelection => match self.selection {
                Some(Selection::Canvas(rect)) => {
                    let reversals = self.canvas_mut().set_area(rect, TRANSPARENT);
                    self.single_pixels_action(reversals);
                }
                Some(Selection::FreeImage) => {
                    self.free_image = None;
                    self.set_selection(None)?;
                }
                _ => (),
            },
            Event::MoveStart(p) => match self.selection {
                Some(Selection::Canvas(_)) => {
                    self.free_image_from_selection(Some(p));
                }
                Some(Selection::FreeImage) => {
                    if let Some(free_image) = self.free_image.as_mut() {
                        free_image.pivot = Some(p - free_image.rect.pos());
                    }
                }
                None => skip_event = true,
            },
            Event::MoveEnd(p) => {
                let last_event = self.events.last();

                if let Some(Event::MoveStart(_)) = last_event {
                    self.move_free_image(p)?;
                } else {
                    skip_event = true;
                }
            }
            Event::Paste(p) => {
                if let Some(img) = self.clipboard.as_ref().cloned() {
                    let img = FreeImage::new(p, img);
                    self.free_image = Some(img);
                    self.set_selection(Some(Selection::FreeImage))?;
                }
            }
            Event::FlipHorizontal => {
                if let Some(Selection::Canvas(_)) = self.selection {
                    self.free_image_from_selection(None);
                }
                if let Some(free_img) = self.free_image.as_mut() {
                    free_img.flip_horizontally();
                }
            }
            Event::FlipVertical => {
                if let Some(Selection::Canvas(_)) = self.selection {
                    self.free_image_from_selection(None);
                }
                if let Some(free_img) = self.free_image.as_mut() {
                    free_img.flip_vertically();
                }
            }
            Event::ApplyTransform(t) => {
                if let Some(Selection::Canvas(_)) = self.selection {
                    self.free_image_from_selection(None);
                }

                let palette = self.palette().to_vec();
                if let Some(free_img) = self.free_image.as_mut() {
                    t.apply(&mut free_img.texture, palette);
                }
            }
            Event::NewLayerAbove => {
                self.layers.add_new_above();
                self.end_action();
                self.cur_reversal = Some(Action::default());
                let i = self.layers.count() - 1;
                self.cur_reversal
                    .as_mut()
                    .ok_or(Error::ReversalNotSet)?
                    .push(AtomicAction::DestroyLayer(i));
                self.end_action();
            }
            Event::NewLayerBelow => todo!(),
            Event::SwitchLayer(i) => self.layers.switch_to(i),
            Event::ChangeLayerVisibility(i, visible) => self.layers.set_visibility(i, visible),
            Event::ChangeLayerOpacity(i, alpha) => self.layers.set_opacity(i, alpha),
            Event::RenameLayer(i, name) => self.layers.set_name(i, name),
            Event::SetLayerFilters(i, filters) => {
                // The chain the layer had is what reverses this, and recording
                // the whole chain as one action makes any change to it — adding,
                // removing, reordering — a single step to undo.
                let previous = self.layers.set_filters(i, filters);

                self.start_action();
                self.add_to_action(vec![AtomicAction::SetLayerFilters(i, previous)])?;
                self.end_action();
            }
            Event::SetFiltersEnabled(enabled) => self.layers.set_filters_enabled(enabled),
            Event::SetLayerAdjustment(i, adjustment) => {
                let previous = self.layers.set_adjustment(i, adjustment);

                self.start_action();
                self.add_to_action(vec![AtomicAction::SetLayerAdjustment(i, previous)])?;
                self.end_action();
            }
            // TODO: this should not only remove it, as we need to be able to
            // undo this
            Event::DeleteLayer(i) => {
                let img = self.layers.delete(i);
                self.end_action();
                self.cur_reversal = Some(Action::default());
                self.cur_reversal
                    .as_mut()
                    .ok_or(Error::ReversalNotSet)?
                    .push(AtomicAction::CreateLayer(i, img));
                self.end_action();
            }
            Event::MoveLayerDown(i) => self.layers.swap(i, i - 1),
            Event::MoveLayerUp(i) => self.layers.swap(i, i + 1),
            Event::SetSpritesheet(size) => self.set_spritesheet(size),
            Event::Undo => {
                // TODO: we should add UNDO to the events list
                #[allow(unused_must_use)]
                {
                    dbg!(t0.elapsed());
                }
                return Ok(self.undo());
            }
            Event::Redo => {
                #[allow(unused_must_use)]
                {
                    dbg!(t0.elapsed());
                }
                return Ok(self.redo());
            }
        }

        if event.clears_selection() {
            self.clear_selection()?;
        }

        #[allow(unused_must_use)]
        {
            dbg!(t0.elapsed());
        }

        if skip_event {
            println!("Event skipped");
            Ok(CanvasEffect::None)
        } else {
            let effect = event.canvas_effect();
            self.events.push(event);

            // The history is only ever read as `.last()` — to know what the
            // previous event was — so keep just a short tail. Without this it
            // grows without bound: a long drawing session appends every stroke,
            // and playing an animation appends a frame switch on every tick.
            if self.events.len() > MAX_EVENT_HISTORY {
                let excess = self.events.len() - MAX_EVENT_HISTORY;
                self.events.drain(0..excess);
            }

            Ok(effect)
        }
    }

    /// Resize every frame of every layer, returning the old images tagged with
    /// their layer and frame.
    fn resize_canvas(&mut self, size: Size<i32>) -> Vec<(usize, usize, IMG)> {
        self.layers.resize_all(size)
    }

    /// Get a mutable reference to the active [`Layer`]'s [`Canvas`]
    ///
    /// [`Layer`]: crate::Layer
    pub fn canvas_mut(&mut self) -> &mut Canvas<IMG> {
        self.layers.active_canvas_mut()
    }

    /// Get a reference to the active [`Layer`]'s [`Canvas`]
    ///
    /// [`Layer`]: crate::Layer
    pub fn canvas(&self) -> &Canvas<IMG> {
        self.layers.active_canvas()
    }

    /// Replace the active layer's active-frame pixels with `img`, undoably.
    ///
    /// The drawing tools build their edits pixel by pixel; this is the way to
    /// drop in a whole image produced elsewhere — a procedural generator's
    /// output, say — as one reversible step. `img` is expected to match the
    /// canvas size.
    ///
    /// Returns [`CanvasEffect::Layer`] so the caller rebuilds the layer's
    /// texture, exactly as for any edit that replaces a cel wholesale. Undo is a
    /// single self-inverse [`SetLayerCanvas`](crate::AtomicAction) that restores
    /// the previous image.
    pub fn set_active_cel_image(&mut self, img: IMG) -> Result<CanvasEffect> {
        let (layer, frame) = (self.layers.active_index(), self.layers.active_frame());
        let old = self.layers.set_cel_img(layer, frame, img);
        let reversal = AtomicAction::SetLayerCanvas(layer, frame, old);
        self.start_action();
        self.add_to_action(vec![reversal])?;
        self.end_action();

        Ok(CanvasEffect::Layer)
    }

    /// Set (or clear) a layer's generator recipe and, optionally, replace its
    /// active-frame pixels with `img` — as one undoable step.
    ///
    /// The frontend runs the script (the core has no scripting engine) and hands
    /// back both the recipe to remember and the pixels it produced. Adjusting a
    /// knob is the same call with a new recipe and new pixels; removing the
    /// generator is `None`, `None`. Undo restores the previous recipe and, if
    /// pixels were replaced, the previous image.
    pub fn set_layer_generator(
        &mut self,
        layer: usize,
        generator: Option<Generator>,
        img: Option<IMG>,
    ) -> Result<CanvasEffect> {
        self.start_action();
        let mut reversals = Vec::new();

        if let Some(img) = img {
            let frame = self.layers.active_frame();
            let old_img = self.layers.set_cel_img(layer, frame, img);
            reversals.push(AtomicAction::SetLayerCanvas(layer, frame, old_img));
        }

        let old_generator = self.layers.set_generator(layer, generator);
        reversals.push(AtomicAction::SetLayerGenerator(layer, old_generator));

        self.add_to_action(reversals)?;
        self.end_action();

        Ok(CanvasEffect::Layer)
    }

    /// Get a reference to the collection of [`Layers`]
    pub fn layers(&self) -> &Layers<IMG> {
        &self.layers
    }

    /// Get the currently selected [`Tool`]
    pub fn selected_tool(&self) -> Tool {
        self.tool
    }

    /// Get the main (selected) color. This is the color used by most tools
    /// when drawing
    pub fn main_color(&self) -> Color {
        self.main_color
    }

    /// Get the spritesheet dimensions (number of horizontal and vertical
    /// frames). For a static image (not an animation) it will be `(1, 1)`.
    pub fn spritesheet(&self) -> Size<u8> {
        self.spritesheet
    }

    /// How many frames the project has
    pub fn frame_count(&self) -> usize {
        self.layers.frame_count()
    }

    /// The frame being edited and shown
    pub fn active_frame(&self) -> usize {
        self.layers.active_frame()
    }

    /// Set the spritesheet dimensions (number of horizontal and vertical
    /// frames). For a static image (not an animation) it will be `(1, 1)`.
    fn set_spritesheet(&mut self, size: Size<u8>) {
        if self.canvas().width() % size.x as i32 != 0 || self.canvas().height() % size.y as i32 != 0
        {
            // TODO: relax this requirement
            eprintln!("WARN: Canvas size should be a multiple of the spritesheet size");
            return;
        }

        self.spritesheet = size;
    }

    /// Get the colors of the palette
    pub fn palette(&self) -> &[Color] {
        self.palette.colors()
    }

    /// Get the [`Selection`]
    pub fn selection(&self) -> Option<Selection> {
        self.selection
    }

    /// Get the [`FreeImage`]
    pub fn free_image(&self) -> Option<&FreeImage<IMG>> {
        self.free_image.as_ref()
    }

    /// Clear the [`Selection`]
    fn clear_selection(&mut self) -> Result<()> {
        self.set_selection(None)
    }

    /// Set the [`Selection`]
    fn set_selection(&mut self, selection: Option<Selection>) -> Result<()> {
        match selection {
            None => self.selection = None,
            s @ Some(Selection::Canvas(_)) => self.selection = s,
            s @ Some(Selection::FreeImage) => {
                if self.free_image.is_none() {
                    return Err(Error::MissingFreeImage);
                }
                self.selection = s;
            }
        }

        Ok(())
    }

    /// Anchor the [`FreeImage`] into the canvas.
    fn anchor(&mut self) -> Result<()> {
        if let Some(free_image) = self.free_image.take() {
            println!("Anchoring");
            let reversals = self.canvas_mut().paste_obj(&free_image);
            self.single_pixels_action(reversals);
            self.set_selection(Some(Selection::Canvas(
                free_image.rect.clip_to(self.canvas().rect()),
            )))?;
        }

        Ok(())
    }

    /// Undo the last undoable action. Returns the [`CanvasEffect`] to signal to
    /// the caller what needs to be updated visually
    fn undo(&mut self) -> CanvasEffect {
        if let Some(action) = self.reversals.pop() {
            let (effect, inverse) = action.apply(&mut self.layers);
            // An action that changed nothing has nothing to redo, and pushing it
            // would leave the redo stack looking non-empty to `can_redo`.
            if !inverse.is_empty() {
                self.redos.push(inverse);
            }

            return effect;
        }

        CanvasEffect::None
    }

    /// Redo the last undone action. Returns the [`CanvasEffect`] to signal to
    /// the caller what needs to be updated visually
    fn redo(&mut self) -> CanvasEffect {
        if let Some(action) = self.redos.pop() {
            let (effect, inverse) = action.apply(&mut self.layers);
            // Pushed directly instead of via `single_action`, which would clear
            // the redo stack we're walking back up.
            if !inverse.is_empty() {
                self.reversals.push(inverse);
            }

            return effect;
        }

        CanvasEffect::None
    }

    /// Whether there is an action available to [`Event::Undo`]
    pub fn can_undo(&self) -> bool {
        !self.reversals.is_empty()
    }

    /// Whether there is an action available to [`Event::Redo`]
    pub fn can_redo(&self) -> bool {
        !self.redos.is_empty()
    }

    /// When drawing lines, rectangles, etc. or moving things, there are visible
    /// effects (e.g. a preview of the line or of the image being moved) that
    /// are not immediately represented in the canvas, but are stored as a
    /// [`FreeImage`] instead. This method must be called as often as possible
    /// whenever the mouse moves, in order to update this preview image.
    pub fn update_free_image(&mut self, mouse_canvas: Position<i32>) -> Result<()> {
        match self.events.last() {
            Some(Event::MoveStart(_)) => self.move_free_image(mouse_canvas)?,
            Some(Event::LineStart(p)) => self.update_line_preview(*p, mouse_canvas),
            Some(Event::RectStart(p)) => self.update_rect_preview(*p, mouse_canvas),
            Some(Event::EllipseStart(p)) => self.update_ellipse_preview(*p, mouse_canvas),
            _ => (),
        }

        Ok(())
    }

    fn move_free_image(&mut self, new: Position<i32>) -> Result<()> {
        if let Some(free_image) = self.free_image.as_mut() {
            free_image.move_by_pivot(new);
            self.set_selection(Some(Selection::FreeImage))?;
        }

        Ok(())
    }

    fn free_image_from_selection(&mut self, mouse_pos: Option<Point<i32>>) {
        if let Some(Selection::Canvas(rect)) = self.selection {
            self.free_image = Some(FreeImage::from_canvas_area(
                self.canvas(),
                rect,
                mouse_pos.map(|p| p - rect.pos()),
            ));
            let reversals = self.canvas_mut().set_area(rect, TRANSPARENT);
            self.single_pixels_action(reversals);
            self.selection = Some(Selection::FreeImage);
        }
    }

    fn update_line_preview(&mut self, p0: Point<i32>, p: Point<i32>) {
        self.free_image = Some(FreeImage::line_preview(
            p0,
            p,
            self.main_color(),
            self.brush_radius,
        ));
    }

    fn update_rect_preview(&mut self, p0: Point<i32>, p: Point<i32>) {
        self.free_image = Some(FreeImage::rect_preview(
            p0,
            p,
            self.main_color(),
            self.brush_radius,
        ));
    }

    fn update_ellipse_preview(&mut self, p0: Point<i32>, p: Point<i32>) {
        self.free_image = Some(FreeImage::ellipse_preview(
            p0,
            p,
            self.main_color(),
            self.brush_radius,
        ));
    }

    fn save_image(&self, path: &str, options: &ExportOptions) -> Result<()> {
        let blended = self.layers.blended(self.palette.colors());

        util::save_image(export::prepare(&blended, options, None), path)
    }

    /// The color visible at a point, with layer filters, visibility and opacity
    /// all taken into account: what the eyedropper would pick up.
    pub fn visible_pixel(&self, p: Point<i32>) -> Color {
        self.layers.visible_pixel(p, self.palette.colors())
    }

    /// The image of a layer as it is shown, with its filters applied
    pub fn rendered_layer(&self, index: usize) -> crate::Rendered<'_, IMG> {
        self.layers.rendered(index, self.palette.colors())
    }

    /// Every layer flattened into one image, as it should be seen: filters,
    /// visibility, opacity and adjustment layers all accounted for.
    pub fn composite(&self) -> std::cell::Ref<'_, IMG> {
        self.layers.composite(self.palette.colors())
    }

    /// Whether layer filters are being applied
    pub fn filters_enabled(&self) -> bool {
        self.layers.filters_enabled()
    }

    /// Export every layer into a directory as its own PNG, named after the
    /// layer.
    ///
    /// A drawing built up as one layer per part yields those parts separately
    /// this way, without having to hide and export each in turn.
    fn export_layers(&self, dir: &Path, options: &ExportOptions) -> Result<()> {
        let mut taken = HashSet::new();

        for i in 0..self.layers.count() {
            let name = unique_file_name(&mut taken, self.layers.get(i).name(), i);
            let out = dir.join(format!("{name}.png"));
            // Each file stands alone, so each is trimmed to its own content.
            let image = export::prepare(&self.layer_image(i), options, None);

            util::save_image(image, out.to_string_lossy().as_ref())?;
        }

        Ok(())
    }

    /// Export every layer into one image, tiled left to right and top to
    /// bottom into a grid `cells` across and down.
    ///
    /// Layers are all the same size, so they tile exactly. A drawing built up
    /// as one layer per part comes out as a ready made sprite sheet.
    fn export_layer_sheet(
        &self,
        path: &Path,
        cells: Size<u8>,
        options: &ExportOptions,
    ) -> Result<()> {
        let images = (0..self.layers.count()).map(|i| self.layer_image(i)).collect();

        self.save_sheet(path, images, cells, options)
    }

    /// Export the composited image of each frame into a directory as its own
    /// PNG, numbered in frame order.
    fn export_frames(&self, dir: &Path, options: &ExportOptions) -> Result<()> {
        for frame in 0..self.frame_count() {
            let out = dir.join(format!("frame_{}.png", frame + 1));
            let image = export::prepare(&self.frame_image(frame), options, None);

            util::save_image(image, out.to_string_lossy().as_ref())?;
        }

        Ok(())
    }

    /// Export every frame's composited image, tiled into a grid `cells` across
    /// and down: the classic sprite sheet of an animation.
    fn export_frame_sheet(
        &self,
        path: &Path,
        cells: Size<u8>,
        options: &ExportOptions,
    ) -> Result<()> {
        let images = (0..self.frame_count()).map(|f| self.frame_image(f)).collect();

        self.save_sheet(path, images, cells, options)
    }

    /// Tile a list of same-size images into one sheet and save it.
    ///
    /// Shared by the layer and frame sheet exports, since both are "lay these
    /// images out in a grid". Cropping trims every image to one shared
    /// rectangle so the cells stay aligned, and sizing to a power of two is held
    /// back for the finished sheet rather than each cell.
    fn save_sheet(
        &self,
        path: &Path,
        images: Vec<IMG>,
        cells: Size<u8>,
        options: &ExportOptions,
    ) -> Result<()> {
        let (cols, rows) = (cells.x.max(1) as i32, cells.y.max(1) as i32);

        if (cols * rows) < images.len() as i32 {
            return Err(Error::SheetTooSmall {
                cols: cols as u32,
                rows: rows as u32,
                count: images.len() as u32,
            });
        }

        let bounds = options
            .crop
            .then(|| export::shared_bounds(&images))
            .flatten();
        let per_cell = ExportOptions {
            power_of_two: false,
            ..options.clone()
        };
        let prepared: Vec<IMG> = images
            .iter()
            .map(|image| export::prepare(image, &per_cell, bounds))
            .collect();

        let cell = prepared
            .first()
            .map(|image| Size::new(image.width(), image.height()))
            .unwrap_or(Size::new(1, 1));
        let mut sheet = IMG::new((cell.x * cols, cell.y * rows).into(), TRANSPARENT);

        for (index, image) in prepared.iter().enumerate() {
            let index = index as i32;
            let origin = Point::new(index % cols * cell.x, index / cols * cell.y);

            for x in 0..cell.x {
                for y in 0..cell.y {
                    let p = Point::new(x, y);

                    sheet.set_pixel(origin + p, image.pixel(p));
                }
            }
        }

        if options.power_of_two {
            sheet = export::fit_to_power_of_two(&sheet);
        }

        util::save_image(sheet, path.to_string_lossy().as_ref())
    }

    /// The fully composited image of a frame: every visible layer blended, with
    /// filters, opacity and adjustment layers applied.
    pub fn frame_image(&self, frame: usize) -> IMG {
        self.layers.flatten_frame(frame, self.palette.colors())
    }

    /// The image of a single layer as it appears on screen, with the layer's
    /// opacity baked into the alpha channel.
    fn layer_image(&self, index: usize) -> IMG {
        // Exported as shown, filters included.
        let rendered = self.rendered_layer(index);
        let opacity = self.layers.get(index).opacity() as u16;
        let mut img = IMG::new(rendered.size(), TRANSPARENT);

        for i in 0..rendered.width() {
            for j in 0..rendered.height() {
                let p = Point::new(i, j);
                let mut color = rendered.pixel(p);
                color.a = (color.a as u16 * opacity / 255) as u8;

                img.set_pixel(p, color);
            }
        }

        img
    }

    /// The area a selection being dragged covers right now, with the mouse at
    /// `mouse`. `None` when no selection is being dragged.
    ///
    /// Mirrors what [`Event::EndSelection`] would produce, so the outline drawn
    /// during the drag matches the selection that results from it.
    pub fn selection_in_progress(&self, mouse: Point<i32>) -> Option<Rect<i32>> {
        let Some(Event::StartSelection(p0)) = self.events.last() else {
            return None;
        };

        let size = mouse.abs_diff(*p0);
        let corner = mouse.rect_min_corner(*p0);
        let rect = Rect::new(corner.x, corner.y, size.x + 1, size.y + 1);

        Some(rect.clip_to(self.canvas().rect()))
    }

    /// Radius of the brush and eraser, in pixels
    pub fn brush_radius(&self) -> u8 {
        self.brush_radius
    }

    fn import_image(&mut self, path: &str) -> Result<()> {
        let img = util::load_img_from_file(path)?;

        if img.width() as i32 > self.canvas().width()
            || img.height() as i32 > self.canvas().height()
        {
            self.resize_canvas((img.width() as i32, img.height() as i32).into());
        }

        let img: IMG = util::img_from_raw(img);
        let img = FreeImage::new(Point::ZERO, img);
        self.free_image = Some(img);
        self.set_selection(Some(Selection::FreeImage))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_usable_layer_names_as_they_are() {
        assert_eq!(file_name_from("Head", 0), "Head");
        assert_eq!(file_name_from("left arm", 0), "left arm");
        assert_eq!(file_name_from("arm-02", 0), "arm-02");
    }

    #[test]
    fn replaces_characters_a_file_name_cannot_hold() {
        assert_eq!(file_name_from("torso/legs", 0), "torso_legs");
        assert_eq!(file_name_from("a:b*c?d", 0), "a_b_c_d");
        assert_eq!(file_name_from("q\"w<e>r|t\\y", 0), "q_w_e_r_t_y");
    }

    #[test]
    fn falls_back_to_the_layer_position_when_nothing_usable_is_left() {
        assert_eq!(file_name_from("", 0), "layer_1");
        assert_eq!(file_name_from("   ", 4), "layer_5");
        assert_eq!(file_name_from("...", 1), "layer_2");
    }

    #[test]
    fn trims_trailing_dots_and_spaces_windows_rejects() {
        assert_eq!(file_name_from("  head  ", 0), "head");
        assert_eq!(file_name_from("head.", 0), "head");
    }

    #[test]
    fn distinguishes_layers_that_share_a_name() {
        let mut taken = HashSet::new();

        assert_eq!(unique_file_name(&mut taken, "arm", 0), "arm");
        assert_eq!(unique_file_name(&mut taken, "arm", 1), "arm_2");
        assert_eq!(unique_file_name(&mut taken, "arm", 2), "arm_3");
        assert_eq!(unique_file_name(&mut taken, "leg", 3), "leg");
    }

    #[test]
    fn treats_names_differing_only_in_case_as_the_same_file() {
        // Windows and macOS file systems are usually case insensitive, so
        // these would otherwise overwrite one another.
        let mut taken = HashSet::new();

        assert_eq!(unique_file_name(&mut taken, "Arm", 0), "Arm");
        assert_eq!(unique_file_name(&mut taken, "arm", 1), "arm_2");
    }
}
