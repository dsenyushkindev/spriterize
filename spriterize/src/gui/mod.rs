use crate::{Effect, UiEvent, UiState};
use lapix::{Position, Size, Tool};
use macroquad::prelude::*;

mod layers;
mod menu;
mod palette;
mod preview;
mod status;
mod toolbar;

use layers::LayersPanel;
use menu::MenuBar;
use palette::Palette;
use preview::Preview;
use status::StatusBar;
use toolbar::Toolbar;

#[derive(Debug, Clone)]
pub struct GuiSyncParams {
    pub main_color: [u8; 4],
    pub num_layers: usize,
    pub active_layer: usize,
    pub layers_vis: Vec<bool>,
    pub layers_alpha: Vec<u8>,
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
}

pub struct Gui {
    toolbar: Toolbar,
    layers_panel: LayersPanel,
    preview: Preview,
    palette: Palette,
    status_bar: StatusBar,
    menu: MenuBar,
    mouse_on_canvas: bool,
    selected_tool: Tool,
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
            mouse_on_canvas: false,
            selected_tool: Tool::Brush,
        }
    }

    pub fn sync(&mut self, params: GuiSyncParams) {
        self.mouse_on_canvas = params.is_on_canvas;

        self.selected_tool = params.selected_tool;
        self.layers_panel.sync(
            params.num_layers,
            params.active_layer,
            params.layers_vis.clone(),
            params.layers_alpha.clone(),
        );
        self.preview.sync(
            params.spritesheet,
            params.canvas_size,
            params.layers_vis.clone(),
            params.layers_alpha.clone(),
        );
        self.palette.sync(params.palette.clone(), params.main_color);
        self.menu.sync(
            params.canvas_size,
            params.spritesheet,
            params.can_undo,
            params.can_redo,
        );
        self.status_bar.sync(params);
    }

    pub fn update(&mut self) -> Vec<Effect> {
        let mut events = Vec::new();

        egui_macroquad::ui(|egui_ctx| {
            crate::theme::apply_egui_visuals(egui_ctx);

            let mut palette_events = self.palette.update(egui_ctx);
            events.append(&mut palette_events);

            let mut toolbar_events = self.toolbar.update(egui_ctx, self.selected_tool);
            events.append(&mut toolbar_events);

            let mut layers_events = self.layers_panel.update(egui_ctx);
            events.append(&mut layers_events);

            let mut menu_events = self.menu.update(egui_ctx);
            events.append(&mut menu_events);

            self.preview.update(egui_ctx);
            self.status_bar.update(egui_ctx);

            let mut canvas_panel_events = self.update_canvas_panel(egui_ctx);
            events.append(&mut canvas_panel_events);

            if self.mouse_on_canvas {
                egui_ctx.output_mut(|o| o.cursor_icon = egui::CursorIcon::None);
            }
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
