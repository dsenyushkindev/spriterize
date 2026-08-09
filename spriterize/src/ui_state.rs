use crate::bg::Background;
use crate::files::{self, RecentFiles};
use crate::graphics::DrawContext;
use crate::gui::{Gui, GuiSyncParams};
use crate::input::bindings::KeyBindings;
use crate::input::manager::InputManager;
use crate::mouse::{CursorType, MouseManager};
use crate::project;
use crate::settings::{Settings, MAX_UI_SCALE, MIN_UI_SCALE};
use crate::wrapped_image::WrappedImage;
use crate::{graphics, Result, Timer};
use lapix::primitives::*;
use lapix::{
    Canvas, CanvasEffect, Event, ExportOptions, Layer, LoadProject, SaveProject, Selection, State,
    Tool,
};
use macroquad::prelude::Color as MqColor;
use macroquad::prelude::{FilterMode, Texture2D};
use std::default::Default;
use std::path::PathBuf;
use std::time::SystemTime;

pub const WINDOW_W: i32 = 1200;
pub const WINDOW_H: i32 = 820;
const DEFAULT_WINDOW_POS: (u32, u32) = (40, 40);
pub const CANVAS_W: u16 = 64;
pub const CANVAS_H: u16 = 64;
const LEFT_TOOLBAR_W: u16 = 300;
const CAMERA_SPEED: f32 = 12.;
const BG_COLOR: MqColor = crate::theme::CANVAS_SURROUND;
const GUI_REST_MS: u64 = 100;
const FPS_INTERVAL: usize = 15;
const DEFAULT_ZOOM_LEVEL: f32 = 8.;
pub const MIN_ZOOM: f32 = 0.125;
pub const MAX_ZOOM: f32 = 1024.;

/// Size of the drawing area, in framebuffer pixels.
fn screen_size() -> Size<f32> {
    (
        macroquad::prelude::screen_width(),
        macroquad::prelude::screen_height(),
    )
        .into()
}

/// Scaling the display itself asks for. 1.0 unless we're on a HiDPI screen,
/// where the framebuffer macroquad draws into is larger than the window's
/// logical size because of `high_dpi: true`.
fn dpi_scale() -> f32 {
    macroquad::window::screen_dpi_scale()
}

#[derive(Debug, Clone)]
pub enum Effect {
    Event(Event),
    UiEvent(UiEvent),
}

impl From<Event> for Effect {
    fn from(val: Event) -> Self {
        Self::Event(val)
    }
}

impl From<UiEvent> for Effect {
    fn from(val: UiEvent) -> Self {
        Self::UiEvent(val)
    }
}

// TODO remove this
// TODO maybe this deserves its own module
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum UiEvent {
    ZoomIn,
    ZoomOut,
    ResetZoom,
    ZoomAdd(f32),
    ZoomMul(f32),
    ToggleGrid,
    ToggleFilters,
    NextFrame,
    PreviousFrame,
    SetUiScale(f32),
    OpenSettings,
    ResetLayout,
    MoveCamera(Direction),
    MoveCameraExact(Point<i32>),
    MouseOverGui,
    Paste,
    Exit,
    NewProject,
    /// Ask the menu to put up its "discard the current project?" confirmation
    RequestNewProject,
    OpenProject,
    SaveProject,
    SaveProjectAs,
    /// Ask the menu to put up the export options for the flattened image
    ExportImage,
    /// The flattened image, reshaped by the given options
    ExportImageAs(ExportOptions),
    /// Ask the menu to put up the export options for the layers
    ExportLayers,
    /// One image per layer, into a folder
    ExportLayersSeparately(ExportOptions),
    /// All the layers tiled into one image, the given number of cells across
    /// and down
    ExportLayerSheet(u8, u8, ExportOptions),
    ImportImage,
    OpenRecent(PathBuf),
    ClearRecent,
    GuiInteraction,
    SetZoom100,
    SetCursor(CursorType),
    ToggleCursor(CursorType),
    SetPreviousCursor,
    ToolStart,
    ToolStroke,
    ToolEnd,
    BlockCanvas,
    UnblockCanvas,
}

impl UiEvent {
    pub fn is_gui_interaction(&self) -> bool {
        matches!(self, Self::MouseOverGui | Self::GuiInteraction)
    }
}

impl<'a> From<&'a UiState> for GuiSyncParams {
    fn from(state: &'a UiState) -> Self {
        let n_layers = state.inner.layers().count();
        let (x, y) = macroquad::prelude::mouse_position();
        let (x, y) = state.screen_to_canvas(x, y);
        let p = (x, y).into();
        let in_canvas = state.canvas().is_in_bounds(p);
        let visible_pixel = if in_canvas {
            Some(state.visible_pixel(p))
        } else {
            None
        };

        Self {
            main_color: state.inner.main_color().into(),
            num_layers: n_layers,
            active_layer: state.inner.layers().active_index(),
            layers_vis: (0..n_layers)
                .map(|i| state.inner.layers().get(i).visible())
                .collect(),
            layers_alpha: (0..n_layers)
                .map(|i| state.inner.layers().get(i).opacity())
                .collect(),
            layers_names: (0..n_layers)
                .map(|i| state.inner.layers().get(i).name().to_owned())
                .collect(),
            layers_filters: (0..n_layers)
                .map(|i| state.inner.layers().get(i).filters().to_vec())
                .collect(),
            layers_adjustment: (0..n_layers)
                .map(|i| state.inner.layers().get(i).is_adjustment())
                .collect(),
            filters_enabled: state.inner.filters_enabled(),
            frame_count: state.inner.frame_count(),
            active_frame: state.inner.active_frame(),
            palette: state.inner.palette().iter().map(|c| (*c).into()).collect(),
            mouse_canvas: (x, y).into(),
            is_on_canvas: in_canvas,
            selected_tool: state.selected_tool(),
            visible_pixel_on_mouse: visible_pixel,
            canvas_size: state.canvas().size(),
            spritesheet: state.inner.spritesheet(),
            zoom: state.zoom,
            fps: state.fps,
            can_undo: state.inner.can_undo(),
            can_redo: state.inner.can_redo(),
            recent_files: state.recent.paths().to_vec(),
            current_file: state.current_file.clone(),
            new_project_requested: state.new_project_requested,
            export_layers_requested: state.export_layers_requested,
            export_image_requested: state.export_image_requested,
            brush_radius: state.inner.brush_radius(),
            settings: state.settings.clone(),
            ui_scale: state.ui_scale(),
            dpi_scale: dpi_scale(),
        }
    }
}

pub struct UiState {
    inner: State<WrappedImage>,
    gui: Gui,
    camera: Position<f32>,
    canvas_pos: Position<f32>,
    zoom: f32,
    settings: Settings,
    /// Screen size as of the last time the canvas was centered. Starts at zero
    /// so that the first frame always centers.
    last_screen_size: Size<f32>,
    /// The whole stack flattened into one texture.
    ///
    /// One texture rather than one per layer because an adjustment layer
    /// filters everything below it, which the GPU can't express by blending
    /// layers separately.
    canvas_texture: Texture2D,
    input: InputManager,
    mouse: MouseManager,
    mouse_over_gui: bool,
    key_bindings: KeyBindings,
    gui_interaction_rest: Timer,
    manual_canvas_block: bool,
    free_image_tex: Option<Texture2D>,
    must_exit: bool,
    t0: SystemTime,
    fps: f32,
    bg: Background,
    prev_cursor: CursorType,
    /// The file this project was last opened from or saved to.
    current_file: Option<PathBuf>,
    recent: RecentFiles,
    /// Set by the New Project shortcut, and consumed by the menu on the next
    /// frame to raise its confirmation window.
    new_project_requested: bool,
    /// Set by the Export Layers shortcut, and consumed by the menu on the next
    /// frame to raise its options window.
    export_layers_requested: bool,
    /// Set by the Export Image shortcut, and consumed by the menu on the next
    /// frame to raise its options window.
    export_image_requested: bool,
    /// Where a line, rectangle or ellipse being dragged started. Kept here
    /// because constraining the shape with shift needs the anchor as well as
    /// the cursor.
    shape_start: Option<Point<i32>>,
}

impl Default for UiState {
    fn default() -> Self {
        let state = State::<WrappedImage>::new(
            (CANVAS_W as i32, CANVAS_H as i32).into(),
            Some(LoadProject(project::load)),
            Some(SaveProject(project::save)),
        );
        let drawing = Texture2D::from_image(&state.canvas().inner().0);
        drawing.set_filter(FilterMode::Nearest);

        let key_bindings = KeyBindings::new();

        // TODO: keys_to_track should be defined by the shortcuts in use
        let input = InputManager::new(key_bindings.used_keys());

        Self {
            inner: state,
            gui: Gui::new(),
            camera: Position::ZERO_F32,
            // Both are set by the first `sync_screen_size`, once the real size
            // of the drawing area is known.
            canvas_pos: Position::ZERO_F32,
            zoom: DEFAULT_ZOOM_LEVEL,
            settings: Settings::load(),
            last_screen_size: Size::ZERO_F32,
            canvas_texture: drawing,
            input,
            mouse: MouseManager::new(),
            mouse_over_gui: false,
            key_bindings,
            gui_interaction_rest: Timer::new(),
            free_image_tex: None,
            must_exit: false,
            t0: SystemTime::now(),
            fps: 60.,
            bg: Background::new(),
            prev_cursor: CursorType::Tool(Tool::Brush),
            manual_canvas_block: false,
            current_file: None,
            recent: RecentFiles::load(),
            new_project_requested: false,
            export_layers_requested: false,
            export_image_requested: false,
            shape_start: None,
        }
    }
}

impl UiState {
    pub fn must_exit(&self) -> bool {
        self.must_exit
    }

    /// The flattened canvas, ready to draw
    pub fn canvas_texture(&self) -> &Texture2D {
        &self.canvas_texture
    }

    pub fn update(&mut self, frame: usize) -> Result<()> {
        if frame % FPS_INTERVAL == (FPS_INTERVAL - 1) {
            let elapsed_ms = self.t0.elapsed().unwrap().as_millis();
            self.fps = FPS_INTERVAL as f32 / (elapsed_ms as f32 / 1000.);
            self.t0 = SystemTime::now();
        }

        self.mouse_over_gui = false;
        self.sync_screen_size();
        self.sync_window_pos();
        self.handle_dropped_files()?;

        self.gui.sync((&*self).into());
        // The menu has now seen these and raised its windows, so they mustn't be
        // raised again on the following frames.
        self.new_project_requested = false;
        self.export_layers_requested = false;
        self.export_image_requested = false;
        let fx = self.gui.update();
        self.process_fx(fx)?;

        let (x, y) = macroquad::prelude::mouse_position();
        let sp = (x, y).into();
        let (cx, cy) = self.screen_to_canvas(x, y);
        let cp = (cx, cy).into();
        self.input.sync(sp, cp);
        let fx = self.input.update(&self.key_bindings);
        self.process_fx(fx)?;

        self.sync_mouse();

        Ok(())
    }

    pub fn apply_startup_window_size(&self) {
        let (width, height) = match self.settings.window_size {
            Some(size) => size,
            None => {
                let scale = dpi_scale();

                (
                    (WINDOW_W as f32 * scale) as u32,
                    (WINDOW_H as f32 * scale) as u32,
                )
            }
        };

        macroquad::window::request_new_screen_size(width as f32, height as f32);

        // Without this the window keeps wherever the OS first put it, which
        // combined with the resize above can leave most of it off screen.
        let (x, y) = self.settings.window_pos.unwrap_or(DEFAULT_WINDOW_POS);
        macroquad::miniquad::window::set_window_position(x, y);
    }

    /// Notes where the window has been moved to. Only kept in memory; the file
    /// is written once on exit, so dragging the window doesn't write it on
    /// every frame of the drag.
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    fn sync_window_pos(&mut self) {
        self.settings.window_pos = Some(macroquad::miniquad::window::get_window_position());
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn sync_window_pos(&mut self) {}

    /// Writes out settings that are tracked in memory while the app runs.
    pub fn save_settings(&self) {
        self.settings.save();
    }

    /// Re-centers the canvas whenever the drawing area changes size, which
    /// covers the first frame, resizing the window and maximizing it.
    ///
    /// The size is compared rather than recomputed unconditionally so that
    /// panning and zooming survive from one frame to the next.
    fn sync_screen_size(&mut self) {
        let screen = screen_size();

        if screen != self.last_screen_size {
            let first_frame = self.last_screen_size == Size::ZERO_F32;

            self.last_screen_size = screen;
            self.center_canvas();

            // Remember the size the user settles on, but don't write back the
            // one we just asked for ourselves. Saved on exit, not here.
            if !first_frame {
                self.settings.window_size = Some((screen.x as u32, screen.y as u32));
            }
        }
    }

    /// Centers the canvas in the area to the right of the tool windows, and
    /// resets the scroll so that what is centered is what's on screen.
    pub fn center_canvas(&mut self) {
        let screen = screen_size();
        let canvas = self.canvas_actual_size();
        // `LEFT_TOOLBAR_W` is a width in egui's points, but the canvas is drawn
        // in framebuffer pixels, so it has to be converted before the two can be
        // used in the same calculation.
        let toolbar = LEFT_TOOLBAR_W as f32 * self.ui_scale();

        self.canvas_pos = (
            toolbar + (screen.x - toolbar - canvas.x) / 2.,
            (screen.y - canvas.y) / 2.,
        )
            .into();
        self.camera = Position::ZERO_F32;
    }

    /// Opens anything dropped onto the window, routing each file the same way
    /// the menu and the recent files list do.
    ///
    /// `get_dropped_files` takes the queue rather than copying it, so files are
    /// only seen once.
    ///
    /// Two quirks of miniquad 0.4 to be aware of:
    ///
    /// - A drop doesn't wake a blocking event loop, so the files are picked up
    ///   on the next frame some other input causes. In practice the pointer is
    ///   over the window and moves immediately afterwards.
    /// - Only X11, Wayland, macOS and web dispatch the drop event. On Windows
    ///   miniquad records the paths but never fires it, so this does nothing
    ///   there until that's fixed upstream.
    fn handle_dropped_files(&mut self) -> Result<()> {
        for file in macroquad::input::get_dropped_files() {
            if let Some(path) = file.path {
                self.open_path(path)?;
            }
        }

        Ok(())
    }

    /// Framebuffer pixels per interface point: what the display asks for, times
    /// the user's own preference.
    ///
    /// Handing this to egui each frame is also what keeps the interface the
    /// right size at all: egui-miniquad 0.16 works out the display scaling but
    /// never passes it on to egui, which then lays everything out as if the
    /// screen were unscaled.
    pub fn ui_scale(&self) -> f32 {
        dpi_scale() * self.settings.ui_scale
    }

    /// Applies the shift constraint to a shape being dragged: lines snap to the
    /// nearest 45 degree direction, rectangles and ellipses become squares and
    /// circles. Leaves the point alone when shift isn't held or no shape is
    /// being dragged.
    fn constrained(&self, p: Point<i32>) -> Point<i32> {
        use macroquad::prelude::{is_key_down, KeyCode};

        let Some(start) = self.shape_start else {
            return p;
        };

        if !is_key_down(KeyCode::LeftShift) && !is_key_down(KeyCode::RightShift) {
            return p;
        }

        match self.selected_tool() {
            Tool::Line => lapix::graphics::snap_to_direction(start, p),
            Tool::Rectangle | Tool::Ellipse => lapix::graphics::snap_to_square(start, p),
            _ => p,
        }
    }

    /// Where to show the brush preview, if it should be shown at all.
    ///
    /// Only for the tools that stamp, and only while the pointer is over the
    /// canvas rather than a tool window.
    fn brush_preview_at(&self, mouse_canvas: Point<i32>) -> Option<Point<i32>> {
        // A line is that dot swept along the drag, and a rectangle is it swept
        // around one, so in both cases the dot shows what the stroke will look
        // like — for the rectangle it even sits exactly where a corner will be.
        // An ellipse is left out because the drag corner isn't on the curve, so
        // a dot there would point at the wrong place.
        //
        // Once a shape is being dragged its own preview shows the whole stroke,
        // and the dot would only be in the way.
        let stamps = match self.selected_tool() {
            Tool::Brush | Tool::Eraser | Tool::Smooth => true,
            Tool::Line | Tool::Rectangle => self.shape_start.is_none(),
            _ => false,
        };

        if !stamps || self.is_canvas_blocked() || !self.canvas().is_in_bounds(mouse_canvas) {
            return None;
        }

        Some(mouse_canvas)
    }

    /// Asks for another frame when something on screen is still moving.
    ///
    /// With `blocking_event_loop` the editor only redraws in response to input,
    /// so every animation has to keep itself alive. Without this the marching
    /// ants around a selection freeze, and the preview stops cycling frames.
    fn schedule_next_frame(&self) {
        let (x, y) = macroquad::prelude::mouse_position();
        let mouse_canvas = self.screen_to_canvas(x, y).into();

        let animating = self.inner.selection().is_some()
            // Marching ants around a selection being dragged.
            || self.inner.selection_in_progress(mouse_canvas).is_some()
            || self.inner.free_image().is_some()
            || self.spritesheet_frames() > 1;

        if animating || self.gui.wants_repaint() || self.gui.is_arranging() {
            macroquad::miniquad::window::schedule_update();
        }
    }

    fn spritesheet_frames(&self) -> u32 {
        let sheet = self.inner.spritesheet();

        sheet.x as u32 * sheet.y as u32
    }

    /// Opens a path, loading it as a project or importing it as an image
    /// depending on its extension. This is what the recent files list replays.
    fn open_path(&mut self, path: PathBuf) -> Result<()> {
        if files::is_project(&path) {
            self.execute(Event::LoadProject(path.clone()))?;
        } else {
            self.execute(Event::OpenFile(path.clone()))?;
            self.execute(Event::SetTool(Tool::Move))?;
        }

        self.set_current_file(path);

        Ok(())
    }

    fn save_project_as(&mut self) -> Result<()> {
        if let Some(path) = files::save_project(self.current_file.as_deref()) {
            self.save_project_to(path)?;
        }

        Ok(())
    }

    fn save_project_to(&mut self, path: PathBuf) -> Result<()> {
        self.execute(Event::SaveProject(path.clone()))?;
        self.set_current_file(path);

        Ok(())
    }

    fn set_current_file(&mut self, path: PathBuf) {
        self.recent.push(path.clone());
        self.current_file = Some(path);
    }

    fn process_fx(&mut self, fx: Vec<Effect>) -> Result<()> {
        for effect in fx {
            match effect {
                Effect::UiEvent(event) => self.process_event(event)?,
                Effect::Event(event) => {
                    self.execute(event)?;
                }
            }
        }

        Ok(())
    }

    fn is_canvas_blocked(&self) -> bool {
        self.manual_canvas_block || self.mouse_over_gui || !self.gui_interaction_rest.expired()
    }

    fn draw_ctx(&self) -> DrawContext {
        DrawContext {
            spritesheet: self.inner.spritesheet(),
            scale: self.zoom(),
            canvas_pos: self.canvas_pos(),
            camera: self.camera(),
            canvas_size: (self.canvas().width() as f32, self.canvas().height() as f32).into(),
            selection: self.inner.selection(),
            show_grid: self.settings.show_grid,
        }
    }

    pub fn draw(&mut self) -> Result<()> {
        macroquad::prelude::clear_background(BG_COLOR);

        let ctx = self.draw_ctx();

        self.bg.draw(ctx);
        graphics::draw_canvas(&*self);
        graphics::draw_grid(ctx);
        graphics::draw_spritesheet_boundaries(ctx);

        let (x, y) = macroquad::prelude::mouse_position();
        let mouse_canvas = self.screen_to_canvas(x, y).into();

        // TODO should be in update method
        self.inner
            .update_free_image(self.constrained(mouse_canvas))?;

        if self.inner.selection().is_some() {
            graphics::draw_selection(ctx, self.inner.free_image());
        }

        if let Some(rect) = self.inner.selection_in_progress(mouse_canvas) {
            graphics::draw_selection_preview(ctx, rect);
        }

        if let Some(centre) = self.brush_preview_at(mouse_canvas) {
            graphics::draw_brush_preview(
                ctx,
                centre,
                self.inner.brush_radius(),
                self.inner.main_color().into(),
            );
        }

        // TODO: most of this logic should be in some update method, not a draw one
        if let Some(img) = self.inner.free_image() {
            // Since macroquad 0.4 a Texture2D frees its GPU memory when dropped,
            // so replacing the previous one here is enough.
            let tex = Texture2D::from_image(&img.texture.0);
            tex.set_filter(FilterMode::Nearest);
            self.free_image_tex = Some(tex);

            graphics::draw_free_image(
                ctx,
                img,
                self.inner.layers().active().opacity(),
                self.free_image_tex.as_ref().unwrap(),
            );
        } else {
            self.free_image_tex = None;
        }

        egui_macroquad::draw();
        self.gui.draw_preview(self);
        self.mouse.draw();

        self.schedule_next_frame();

        Ok(())
    }

    pub fn sync_mouse(&mut self) {
        let (x, y) = macroquad::prelude::mouse_position();
        let (x, y) = self.screen_to_canvas(x, y);
        let p = (x, y).into();
        let in_canvas = self.canvas().is_in_bounds(p);

        self.mouse.sync(in_canvas, self.selected_tool());
    }

    pub fn execute(&mut self, event: Event) -> Result<()> {
        let effect = self.inner.execute(event)?;

        match effect {
            // TODO: Texture2D is copy, so we don't need `drawing_mut` here, but
            // it would be better.
            // Re-uploads the flattened stack, so filters and adjustment layers
            // are included in what gets drawn.
            CanvasEffect::Update => {
                let composite = self.inner.composite();

                self.canvas_texture.update(&composite.0);
            }
            // The canvas may have changed size, so the texture is rebuilt
            // rather than written over.
            CanvasEffect::New | CanvasEffect::Layer => {
                self.sync_canvas_texture();
            }
            CanvasEffect::None => (),
        };

        Ok(())
    }

    /// Rebuilds the canvas texture from the flattened stack.
    pub fn sync_canvas_texture(&mut self) {
        let texture = {
            let composite = self.inner.composite();
            let texture = Texture2D::from_image(&composite.0);
            texture.set_filter(FilterMode::Nearest);

            texture
        };

        self.canvas_texture = texture;
    }

    pub fn process_event(&mut self, event: UiEvent) -> Result<()> {
        if event.is_gui_interaction() {
            self.gui_interaction_rest.start(GUI_REST_MS);
        }
        let (x, y) = macroquad::prelude::mouse_position();
        let (x, y) = self.screen_to_canvas(x, y);
        let p = (x, y).into();

        match event {
            UiEvent::BlockCanvas => self.manual_canvas_block = true,
            UiEvent::UnblockCanvas => self.manual_canvas_block = false,
            UiEvent::ZoomIn => self.zoom_in(),
            UiEvent::ZoomOut => self.zoom_out(),
            UiEvent::ResetZoom => self.reset_zoom(),
            UiEvent::ZoomAdd(n) => self.zoom_add(n),
            UiEvent::ZoomMul(n) => self.zoom_mul(n),
            UiEvent::SetZoom100 => self.set_zoom(1.),
            UiEvent::ToggleGrid => {
                self.settings.show_grid = !self.settings.show_grid;
                self.settings.save();
            }
            UiEvent::SetUiScale(scale) => {
                self.settings.ui_scale = scale.clamp(MIN_UI_SCALE, MAX_UI_SCALE);
                self.settings.save();
                // The tool windows change size with the scale, so the space the
                // canvas is centered in changes too.
                self.center_canvas();
            }
            UiEvent::ToggleFilters => {
                let enabled = self.inner.filters_enabled();

                self.execute(Event::SetFiltersEnabled(!enabled))?;
            }
            // Wrap around, so holding the key cycles the animation.
            UiEvent::NextFrame => {
                let count = self.inner.frame_count();
                let next = (self.inner.active_frame() + 1) % count;

                self.execute(Event::SwitchFrame(next))?;
            }
            UiEvent::PreviousFrame => {
                let count = self.inner.frame_count();
                let previous = (self.inner.active_frame() + count - 1) % count;

                self.execute(Event::SwitchFrame(previous))?;
            }
            UiEvent::OpenSettings => self.gui.open_settings(),
            UiEvent::ResetLayout => self.gui.reset_layout(),
            UiEvent::MoveCamera(dir) => self.move_camera(dir),
            UiEvent::MoveCameraExact(p) => self.move_camera_exact(p),
            UiEvent::MouseOverGui => self.mouse_over_gui = true,
            UiEvent::GuiInteraction => (),
            UiEvent::Paste => {
                self.execute(Event::Paste(p))?;
            }
            UiEvent::Exit => self.must_exit = true,
            UiEvent::NewProject => *self = UiState::default(),
            UiEvent::RequestNewProject => self.new_project_requested = true,
            UiEvent::OpenProject => {
                if let Some(path) = files::open_project(self.current_file.as_deref()) {
                    self.open_path(path)?;
                }
            }
            UiEvent::SaveProject => {
                // Save straight back to the open project; only ask for a path
                // when there isn't one yet.
                match self.current_file.clone().filter(|p| files::is_project(p)) {
                    Some(path) => self.save_project_to(path)?,
                    None => self.save_project_as()?,
                }
            }
            UiEvent::SaveProjectAs => self.save_project_as()?,
            UiEvent::ExportImage => self.export_image_requested = true,
            UiEvent::ExportImageAs(options) => {
                if let Some(path) = files::export_image(self.current_file.as_deref()) {
                    self.execute(Event::Save(path.clone(), options))?;
                    // Recorded as recent so it can be re-imported, but it isn't
                    // made current: the project is still the open document.
                    self.recent.push(path);
                }
            }
            UiEvent::ExportLayers => self.export_layers_requested = true,
            UiEvent::ExportLayersSeparately(options) => {
                // A directory, not a file: each layer is named after itself.
                if let Some(dir) = files::export_layers_dir(self.current_file.as_deref()) {
                    self.execute(Event::ExportLayers(dir, options))?;
                }
            }
            UiEvent::ExportLayerSheet(cols, rows, options) => {
                if let Some(path) = files::export_image(self.current_file.as_deref()) {
                    self.execute(Event::ExportLayerSheet(
                        path.clone(),
                        (cols, rows).into(),
                        options,
                    ))?;
                    self.recent.push(path);
                }
            }
            UiEvent::ImportImage => {
                if let Some(path) = files::import_image(self.current_file.as_deref()) {
                    self.open_path(path)?;
                }
            }
            UiEvent::OpenRecent(path) => self.open_path(path)?,
            UiEvent::ClearRecent => self.recent.clear(),
            UiEvent::SetPreviousCursor => self.mouse.set_cursor(self.prev_cursor),
            UiEvent::SetCursor(c) => {
                self.prev_cursor = self.mouse.cursor();
                self.mouse.set_cursor(c);
            }
            UiEvent::ToggleCursor(c) => {
                if self.mouse.cursor() == c {
                    self.mouse.set_cursor(self.prev_cursor);
                    self.prev_cursor = c;
                } else {
                    self.prev_cursor = self.mouse.cursor();
                    self.mouse.set_cursor(c);
                }
            }
            // TODO: this used to be in mouse.rs, now it's cluttering this
            // module, we should move it somewhere else
            UiEvent::ToolStart => {
                // Anything left over from a drag that was abandoned, by
                // switching tools part way through it, say.
                self.shape_start = None;

                match (self.selected_tool(), self.is_canvas_blocked()) {
                    (Tool::Brush, false) => self.execute(Event::BrushStart)?,
                    (Tool::Eraser, false) => self.execute(Event::EraseStart)?,
                    (Tool::Smooth, false) => self.execute(Event::SmoothStart)?,
                    (Tool::Line, false) => {
                        self.shape_start = Some(p);
                        self.execute(Event::LineStart(p))?
                    }
                    (Tool::Rectangle, false) => {
                        self.shape_start = Some(p);
                        self.execute(Event::RectStart(p))?
                    }
                    (Tool::Ellipse, false) => {
                        self.shape_start = Some(p);
                        self.execute(Event::EllipseStart(p))?
                    }
                    (Tool::Bucket, false) => self.execute(Event::Bucket(p))?,
                    (Tool::Selection, false) => self.execute(Event::StartSelection(p))?,
                    (Tool::Move, false) => self.execute(Event::MoveStart(p))?,
                    (Tool::Eyedropper, false) => {
                        if self.canvas().is_in_bounds(p) {
                            let color = self.visible_pixel(p);
                            self.execute(Event::SetMainColor(color.into()))?;
                            self.execute(Event::SetTool(Tool::Brush))?;
                        }
                    }
                    _ => (),
                }
            }
            UiEvent::ToolStroke => match (self.selected_tool(), self.is_canvas_blocked()) {
                (Tool::Brush, false) => self.execute(Event::BrushStroke(p))?,
                (Tool::Eraser, false) => self.execute(Event::Erase(p))?,
                (Tool::Smooth, false) => self.execute(Event::SmoothStroke(p))?,
                _ => (),
            },
            UiEvent::ToolEnd => match (self.selected_tool(), self.is_canvas_blocked()) {
                (Tool::Brush, false) => self.execute(Event::BrushEnd)?,
                (Tool::Eraser, false) => self.execute(Event::EraseEnd)?,
                (Tool::Smooth, false) => self.execute(Event::SmoothEnd)?,
                // Constrained before the shape is committed, so it lands exactly
                // where the preview showed it.
                (Tool::Line, false) => {
                    let end = self.constrained(p);
                    self.shape_start = None;
                    self.execute(Event::LineEnd(end))?
                }
                (Tool::Rectangle, false) => {
                    let end = self.constrained(p);
                    self.shape_start = None;
                    self.execute(Event::RectEnd(end))?
                }
                (Tool::Ellipse, false) => {
                    let end = self.constrained(p);
                    self.shape_start = None;
                    self.execute(Event::EllipseEnd(end))?
                }
                (Tool::Selection, false) => {
                    self.execute(Event::EndSelection(p))?;
                    self.execute(Event::SetTool(Tool::Move))?;
                }
                (Tool::Move, false) => {
                    if self.is_mouse_on_selection() {
                        self.execute(Event::MoveEnd(p))?;
                    } else {
                        self.execute(Event::ClearSelection)?;
                    }
                }
                _ => (),
            },
        };

        Ok(())
    }

    pub fn visible_pixel(&self, p: Point<i32>) -> [u8; 4] {
        // Goes through the state so filters are taken into account: the
        // eyedropper picks up what is actually on screen.
        self.inner.visible_pixel(p).into()
    }

    pub fn camera(&self) -> Position<f32> {
        self.camera
    }

    pub fn canvas(&self) -> &Canvas<WrappedImage> {
        self.inner.canvas()
    }

    pub fn canvas_pos(&self) -> Position<f32> {
        self.canvas_pos
    }

    pub fn canvas_actual_size(&self) -> Size<f32> {
        (
            self.inner.canvas().width() as f32 * self.zoom,
            self.inner.canvas().height() as f32 * self.zoom,
        )
            .into()
    }

    pub fn selected_tool(&self) -> Tool {
        self.inner.selected_tool()
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    pub fn layer(&self, index: usize) -> &Layer<WrappedImage> {
        self.inner.layers().get(index)
    }

    pub fn num_layers(&self) -> usize {
        self.inner.layers().count()
    }

    pub fn zoom_in(&mut self) {
        self.zoom_mul(2.);
    }

    pub fn zoom_out(&mut self) {
        self.zoom_mul(0.5);
    }

    pub fn zoom_mul(&mut self, val: f32) {
        self.change_zoom(|zoom| zoom * val);
    }

    pub fn zoom_add(&mut self, val: f32) {
        self.change_zoom(|zoom| zoom + val);
    }

    pub fn change_zoom<F: Fn(f32) -> f32>(&mut self, op: F) {
        self.set_zoom((op)(self.zoom));
    }

    pub fn set_zoom_at(&mut self, zoom: f32, anchor: Position<f32>) {
        let new_zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let fac = new_zoom / self.zoom;

        let origin = self.canvas_pos - self.camera;
        let new_origin_x = anchor.x - (anchor.x - origin.x) * fac;
        let new_origin_y = anchor.y - (anchor.y - origin.y) * fac;

        self.camera = (
            self.canvas_pos.x - new_origin_x,
            self.canvas_pos.y - new_origin_y,
        )
            .into();
        self.zoom = new_zoom;
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.set_zoom_at(zoom, self.zoom_anchor());
    }

    fn zoom_anchor(&self) -> Position<f32> {
        let (x, y) = macroquad::prelude::mouse_position();
        let (w, h) = (
            macroquad::prelude::screen_width(),
            macroquad::prelude::screen_height(),
        );
        let on_canvas_area =
            !self.mouse_over_gui && x >= LEFT_TOOLBAR_W as f32 && x < w && y >= 0. && y < h;

        if on_canvas_area {
            (x, y).into()
        } else {
            (
                LEFT_TOOLBAR_W as f32 + (w - LEFT_TOOLBAR_W as f32) / 2.,
                h / 2.,
            )
                .into()
        }
    }

    pub fn reset_zoom(&mut self) {
        self.set_zoom(DEFAULT_ZOOM_LEVEL);
    }

    pub fn move_camera(&mut self, direction: Direction) {
        let speed = CAMERA_SPEED;

        if !self.is_camera_off(direction) {
            match direction {
                Direction::Up => self.camera.y -= speed,
                Direction::Down => self.camera.y += speed,
                Direction::Left => self.camera.x -= speed,
                Direction::Right => self.camera.x += speed,
            }
        }
    }

    pub fn move_camera_exact(&mut self, vector: Point<i32>) {
        let h_dir = if vector.x < 0 {
            Direction::Left
        } else {
            Direction::Right
        };

        let v_dir = if vector.y < 0 {
            Direction::Up
        } else {
            Direction::Down
        };

        if !self.is_camera_off(h_dir) {
            self.camera.x += vector.x as f32;
        }

        if !self.is_camera_off(v_dir) {
            self.camera.y += vector.y as f32;
        }
    }

    fn is_camera_off(&self, direction: Direction) -> bool {
        let buffer = 20.;
        let canvas_size = self.canvas_actual_size();
        let canvas_pos = self.canvas_pos;
        let camera = self.camera;
        let screen = screen_size();
        let win_w = screen.x;
        let win_h = screen.y;

        match direction {
            Direction::Up => canvas_pos.y - camera.y > win_h - buffer,
            Direction::Down => camera.y > canvas_pos.y + canvas_size.y - buffer,
            Direction::Left => canvas_pos.x - camera.x > win_w - buffer,
            Direction::Right => camera.x > canvas_pos.x + canvas_size.x - buffer,
        }
    }

    pub fn screen_to_canvas(&self, x: f32, y: f32) -> (i32, i32) {
        let canvas_x = self.canvas_pos().x - self.camera().x;
        let canvas_y = self.canvas_pos().y - self.camera().y;
        let scale = self.zoom();

        (
            ((x - canvas_x) / scale) as i32,
            ((y - canvas_y) / scale) as i32,
        )
    }

    pub fn is_mouse_on_selection(&self) -> bool {
        let (x, y) = macroquad::prelude::mouse_position();
        let (x, y) = self.screen_to_canvas(x, y);

        let rect = match self.inner.selection() {
            Some(Selection::FreeImage) => self.inner.free_image().unwrap().rect,
            Some(Selection::Canvas(rect)) => rect,
            _ => return false,
        };

        rect.contains(x, y)
    }
}
