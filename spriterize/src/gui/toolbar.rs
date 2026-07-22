use crate::gui::layout::{self, PanelLayout};
use crate::{Effect, Resources};
use lapix::graphics::MAX_BRUSH_RADIUS;
use lapix::{Event, Size, Tool};
use macroquad::prelude::*;
use std::collections::HashMap;

const TOOL_BTN_IMG_SIZE: Size<usize> = Size { x: 16, y: 16 };
const TOOLS: [Tool; 10] = [
    Tool::Brush,
    Tool::Bucket,
    Tool::Eraser,
    Tool::Smooth,
    Tool::Eyedropper,
    Tool::Line,
    Tool::Selection,
    Tool::Move,
    Tool::Rectangle,
    Tool::Ellipse,
];

pub struct Toolbar {
    tools: HashMap<Tool, ToolButton>,
}

impl Toolbar {
    pub fn new() -> Self {
        Self {
            tools: TOOLS.iter().map(|t| (*t, ToolButton::new(*t))).collect(),
        }
    }

    pub fn get_mut(&mut self, tool: Tool) -> Option<&mut ToolButton> {
        self.tools.get_mut(&tool)
    }

    pub fn update(
        &mut self,
        egui_ctx: &egui::Context,
        layout: &PanelLayout,
        selected_tool: Tool,
        brush_radius: u8,
    ) -> Vec<Effect> {
        let mut events = Vec::new();

        layout.show(egui_ctx, layout::TOOLBOX, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(layout::PANEL_WIDTH);
                for tool in TOOLS {
                    if let Some(btn) = self.get_mut(tool) {
                        btn.add_to_ui(ui, selected_tool == tool, || {
                            events.push(Event::SetTool(tool).into())
                        });
                    }
                }
            });

            // Only the tools that lay down a stroke use it, so it would be
            // noise for the others.
            if matches!(
                selected_tool,
                Tool::Brush
                    | Tool::Eraser
                    | Tool::Smooth
                    | Tool::Line
                    | Tool::Rectangle
                    | Tool::Ellipse
            ) {
                ui.separator();

                let mut radius = brush_radius;

                ui.horizontal(|ui| {
                    ui.label("size:");
                    ui.add(
                        egui::Slider::new(&mut radius, 0..=MAX_BRUSH_RADIUS)
                            .show_value(false)
                            .custom_formatter(|r, _| format!("{}", 2. * r + 1.)),
                    )
                    .on_hover_text("stroke radius, shared by the drawing tools");
                    // Shown as a diameter, which is what the stamp on screen
                    // measures across.
                    ui.label(format!("{} px", 2 * radius as u16 + 1));
                });

                if radius != brush_radius {
                    events.push(Event::SetBrushRadius(radius).into());
                }
            }
        });

        events
    }
}

pub struct ToolButton {
    tool: Tool,
    image: egui::ColorImage,
    texture: Option<egui::TextureHandle>,
}

impl ToolButton {
    pub fn new(tool: Tool) -> Self {
        let bytes = Resources::tool_icon(tool);
        // The icons are compiled in, so a decode failure is a build problem
        // rather than something to recover from at runtime.
        let mut img =
            Image::from_file_with_format(bytes, None).expect("bundled icon should decode");
        crate::theme::invert_rgb(&mut img.bytes);

        let x = TOOL_BTN_IMG_SIZE.x;
        let y = TOOL_BTN_IMG_SIZE.y;
        let image = egui::ColorImage::from_rgba_unmultiplied([x, y], &img.bytes);

        Self {
            tool,
            image,
            texture: None,
        }
    }

    pub fn add_to_ui<F: FnMut()>(&mut self, ui: &mut egui::Ui, selected: bool, mut action: F) {
        let tooltip: &'static str = self.tooltip();

        let texture: &egui::TextureHandle = self.texture.get_or_insert_with(|| {
            ui.ctx()
                .load_texture("", self.image.clone(), Default::default())
        });
        let prev_bg_fill = ui.style().visuals.widgets.inactive.weak_bg_fill;
        // Highlight the currently selected tool.
        //
        // FIXME: Ui::scope destroys the toolbar's wrapping layout, so we're forced to temporarily
        // set the style and then set back the old style manually after we're done.
        if selected {
            ui.style_mut().visuals.widgets.inactive.weak_bg_fill = crate::theme::ACCENT;
        }
        if ui
            .add(egui::ImageButton::new(
                egui::load::SizedTexture::from_handle(texture),
            ))
            .on_hover_text(tooltip)
            .clicked()
        {
            (action)();
        }
        ui.style_mut().visuals.widgets.inactive.weak_bg_fill = prev_bg_fill;
    }

    // TODO: the shortcut being hardcoded here is a problem since it's
    // configurable
    fn tooltip(&self) -> &'static str {
        match self.tool {
            Tool::Brush => "brush tool (B)",
            Tool::Bucket => "bucket tool (G)",
            Tool::Eraser => "eraser tool (E)",
            Tool::Eyedropper => "eyedropper tool (I)",
            Tool::Line => "line tool (L)",
            Tool::Selection => "selection tool (S)",
            Tool::Move => "move tool (M)",
            Tool::Rectangle => "rectangle tool (R)",
            Tool::Ellipse => "ellipse tool (O)",
            Tool::Smooth => "smooth tool (K): soften edges between colors",
        }
    }
}
