use crate::gui::GuiSyncParams;
use lapix::{Color, Position, Size, Tool};
use std::path::PathBuf;

pub struct StatusBar {
    current_file: Option<PathBuf>,
    mouse_canvas: Position<i32>,
    is_mouse_on_canvas: bool,
    selected_tool: Tool,
    visible_pixel_on_mouse: Option<[u8; 4]>,
    canvas_size: Size<i32>,
    zoom: f32,
    fps: f32,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            current_file: None,
            mouse_canvas: Position::ZERO,
            is_mouse_on_canvas: false,
            selected_tool: Tool::Brush,
            visible_pixel_on_mouse: None,
            canvas_size: Size::ZERO,
            zoom: 1.,
            fps: 60.,
        }
    }

    pub fn sync(&mut self, params: GuiSyncParams) {
        self.current_file = params.current_file;
        self.mouse_canvas = params.mouse_canvas;
        self.is_mouse_on_canvas = params.is_on_canvas;
        self.selected_tool = params.selected_tool;
        self.visible_pixel_on_mouse = params.visible_pixel_on_mouse;
        self.canvas_size = params.canvas_size;
        self.zoom = params.zoom;
        self.fps = params.fps;
    }

    pub fn update(&mut self, egui_ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("my_panel").show(egui_ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{:.1} FPS", self.fps))
                    .on_hover_text("frames per second");
                ui.separator();
                ui.label(format!("{}x{}", self.canvas_size.x, self.canvas_size.y))
                    .on_hover_text("canvas size");
                ui.separator();
                ui.label(format!("{:.0}%", self.zoom * 100.))
                    .on_hover_text("zoom level");
                ui.separator();
                ui.label(self.selected_tool.to_string())
                    .on_hover_text("current tool");
                ui.separator();

                // miniquad can't retitle the window after it's created, so the
                // open file is shown here instead.
                match &self.current_file {
                    Some(path) => {
                        let name = path
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.to_string_lossy().into_owned());

                        ui.label(name).on_hover_text(path.to_string_lossy());
                    }
                    None => {
                        ui.weak("unsaved").on_hover_text("no file opened or saved yet");
                    }
                }

                if self.is_mouse_on_canvas {
                    ui.separator();
                    ui.label(format!(
                        "{},{}",
                        self.mouse_canvas.x + 1,
                        self.mouse_canvas.y + 1
                    ))
                    .on_hover_text("cursor position in canvas");

                    if let Some(color) = self.visible_pixel_on_mouse {
                        ui.separator();
                        ui.label(Color::from(color).hex())
                            .on_hover_text("color under cursor");
                    }
                }
            });
        });
    }
}
