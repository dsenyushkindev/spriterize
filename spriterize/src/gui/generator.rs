//! The generator script editor: a small window bound to one layer.
//!
//! A layer's generator recipe (script + knob values) lives on the layer and is
//! saved with the project; its knobs are edited inline in the Layers panel. This
//! window is only for editing the *script* — the Layers panel's "Edit script…"
//! opens it for a specific layer. Applying emits the new script; the running,
//! the pixel fill and the error all happen in [`UiState`](crate::UiState), which
//! reports any compile error back here.

use crate::{Effect, UiEvent};

/// The script a freshly added generator starts with. Declares a few knobs so the
/// inline controls appear immediately.
pub const DEFAULT_SCRIPT: &str = "\
// Fills this layer. `p` declares knobs, edited in the Layers panel. Returns a
// Canvas of w x h pixels.
pub fn main(w, h, p) {
    let radius = p.num(\"radius\", 20.0, 2.0, 40.0);
    let fill = p.color(\"fill\", rgb(220, 120, 60));
    let edge = p.color(\"edge\", rgb(30, 20, 20));

    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;
    let c = Canvas::new(w, h);
    let body = disk(cx, cy, radius);
    c.paint(body, solid(fill));
    c.paint(outline(body, 2.0, 1.0), solid(edge));
    c
}
";

pub struct GeneratorWindow {
    open: bool,
    /// The layer whose script is being edited.
    layer: Option<usize>,
    script: String,
    /// The last compile/run error for this script, set by `UiState`.
    error: Option<String>,
}

impl GeneratorWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            layer: None,
            script: String::new(),
            error: None,
        }
    }

    /// Open the editor on `layer`'s script.
    pub fn open_for(&mut self, layer: usize, script: String) {
        self.open = true;
        self.layer = Some(layer);
        self.script = script;
        self.error = None;
    }

    /// Show the result of the last apply: `None` cleared the error, `Some` is a
    /// compile or run failure to display.
    pub fn set_error(&mut self, error: Option<String>) {
        self.error = error;
    }

    pub fn update(&mut self, egui_ctx: &egui::Context) -> Vec<Effect> {
        let mut events = Vec::new();

        let Some(layer) = self.layer else {
            return events;
        };
        if !self.open {
            return events;
        }

        let mut open = self.open;

        egui::Window::new(format!("Generator script — layer {}", layer + 1))
            .open(&mut open)
            .default_width(420.)
            .show(egui_ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(320.)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.script)
                                .code_editor()
                                .desired_rows(16)
                                .desired_width(f32::INFINITY),
                        );
                    });

                if ui
                    .button("Apply")
                    .on_hover_text("run the script into the layer and save it")
                    .clicked()
                {
                    events.push(Effect::UiEvent(UiEvent::SetGeneratorScript {
                        layer,
                        script: self.script.clone(),
                    }));
                }

                if let Some(error) = &self.error {
                    ui.separator();
                    ui.colored_label(egui::Color32::from_rgb(220, 90, 90), error);
                }
            });

        self.open = open;

        events
    }
}
