use crate::settings::Settings;
use crate::{Effect, UiEvent, UiState};
use lapix::{Filter, Position, Size, Tool};
use macroquad::prelude::*;
use std::path::PathBuf;

mod layers;
mod layout;
mod menu;
mod palette;
mod preview;
mod settings;
mod status;
mod toolbar;

use layers::LayersPanel;
use layout::PanelLayout;
use menu::MenuBar;
use palette::Palette;
use preview::Preview;
use settings::SettingsWindow;
use status::StatusBar;
use toolbar::Toolbar;

#[derive(Debug, Clone)]
pub struct GuiSyncParams {
    pub main_color: [u8; 4],
    pub num_layers: usize,
    pub active_layer: usize,
    pub layers_vis: Vec<bool>,
    pub layers_alpha: Vec<u8>,
    pub layers_names: Vec<String>,
    pub layers_filters: Vec<Vec<Filter>>,
    pub layers_adjustment: Vec<bool>,
    pub filters_enabled: bool,
    pub palette: Vec<[u8; 4]>,
    pub mouse_canvas: Position<i32>,
    pub is_on_canvas: bool,
    pub selected_tool: Tool,
    pub visible_pixel_on_mouse: Option<[u8; 4]>,
    pub canvas_size: Size<i32>,
    pub spritesheet: Size<u8>,
    pub zoom: f32,
    pub fps: f32,
    pub can_undo: bool,
    pub can_redo: bool,
    pub recent_files: Vec<PathBuf>,
    pub current_file: Option<PathBuf>,
    pub new_project_requested: bool,
    pub brush_radius: u8,
    pub settings: Settings,
    /// Framebuffer pixels per interface point, applied to egui each frame.
    pub ui_scale: f32,
    pub dpi_scale: f32,
}

pub struct Gui {
    toolbar: Toolbar,
    layers_panel: LayersPanel,
    preview: Preview,
    palette: Palette,
    status_bar: StatusBar,
    menu: MenuBar,
    settings_window: SettingsWindow,
    layout: PanelLayout,
    mouse_on_canvas: bool,
    selected_tool: Tool,
    brush_radius: u8,
    /// Whether egui has an animation in flight (hover fades, tooltips, the text
    /// cursor) and needs another frame to finish it.
    wants_repaint: bool,
    ui_scale: f32,
}

impl Gui {
    pub fn new() -> Self {
        Self {
            toolbar: Toolbar::new(),
            layers_panel: LayersPanel::new(),
            preview: Preview::new(),
            palette: Palette::new(),
            status_bar: StatusBar::new(),
            menu: MenuBar::new(),
            settings_window: SettingsWindow::new(),
            layout: PanelLayout::new(),
            mouse_on_canvas: false,
            selected_tool: Tool::Brush,
            brush_radius: 0,
            wants_repaint: false,
            ui_scale: 1.,
        }
    }

    pub fn wants_repaint(&self) -> bool {
        self.wants_repaint
    }

    pub fn open_settings(&mut self) {
        self.settings_window.open();
    }

    pub fn reset_layout(&mut self) {
        self.layout.reset();
    }

    /// Whether the tool windows are still being positioned, and so another
    /// frame is needed even if nothing else is happening.
    pub fn is_arranging(&self) -> bool {
        self.layout.is_arranging()
    }

    pub fn sync(&mut self, params: GuiSyncParams) {
        self.mouse_on_canvas = params.is_on_canvas;

        self.selected_tool = params.selected_tool;
        self.brush_radius = params.brush_radius;
        self.layers_panel.sync(
            params.num_layers,
            params.active_layer,
            params.layers_vis.clone(),
            params.layers_alpha.clone(),
            params.layers_names.clone(),
            params.layers_filters.clone(),
            params.layers_adjustment.clone(),
            params.filters_enabled,
        );
        self.preview.sync(
            params.spritesheet,
            params.canvas_size,
            params.layers_vis.clone(),
            params.layers_alpha.clone(),
        );
        self.ui_scale = params.ui_scale;
        self.settings_window
            .sync(params.settings.clone(), params.dpi_scale);
        self.palette.sync(params.palette.clone(), params.main_color);
        self.menu.sync(
            params.canvas_size,
            params.spritesheet,
            params.can_undo,
            params.can_redo,
            params.recent_files.clone(),
            params.new_project_requested,
            params.filters_enabled,
        );
        self.status_bar.sync(params);
    }

    pub fn update(&mut self) -> Vec<Effect> {
        let mut events = Vec::new();

        egui_macroquad::ui(|egui_ctx| {
            // Has to be set every frame: egui-miniquad 0.16 computes the
            // display's scaling but never hands it to egui, which otherwise
            // lays the whole interface out as though the screen were unscaled.
            egui_ctx.set_pixels_per_point(self.ui_scale);
            crate::theme::apply_egui_visuals(egui_ctx);

            let mut palette_events = self.palette.update(egui_ctx, &self.layout);
            events.append(&mut palette_events);

            let mut toolbar_events = self.toolbar.update(
                egui_ctx,
                &self.layout,
                self.selected_tool,
                self.brush_radius,
            );
            events.append(&mut toolbar_events);

            let mut layers_events = self.layers_panel.update(egui_ctx, &self.layout);
            events.append(&mut layers_events);

            let mut menu_events = self.menu.update(egui_ctx);
            events.append(&mut menu_events);

            self.preview.update(egui_ctx, &self.layout);
            self.status_bar.update(egui_ctx);

            let mut settings_events = self.settings_window.update(egui_ctx);
            events.append(&mut settings_events);

            let mut canvas_panel_events = self.update_canvas_panel(egui_ctx);
            events.append(&mut canvas_panel_events);

            if self.mouse_on_canvas {
                egui_ctx.output_mut(|o| o.cursor_icon = egui::CursorIcon::None);
            }

            self.wants_repaint = egui_ctx.has_requested_repaint();
            self.layout.update(egui_ctx);
        });

        events
    }

    pub fn draw_preview(&self, state: &UiState) {
        self.preview.draw(state);
    }

    fn update_canvas_panel(&mut self, egui_ctx: &egui::Context) -> Vec<Effect> {
        let mut events = Vec::new();

        if egui_ctx.is_pointer_over_area() {
            events.push(Effect::UiEvent(UiEvent::MouseOverGui));
        }

        events
    }
}
