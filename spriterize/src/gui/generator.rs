//! Script and graph editors for a layer generator.

use crate::gui::graph::{from_recipe, to_recipe, GraphViewer};
use crate::{Effect, UiEvent};
use egui_snarl::ui::SnarlStyle;
use egui_snarl::Snarl;
use lapix::{Generator, GeneratorDefinition, GeneratorNode};

pub const DEFAULT_SCRIPT: &str = "\
// Fills this layer. `p` declares knobs, edited in the Layers panel.
pub fn main(w, h, p) {
    let radius = p.num(\"radius\", 20.0, 2.0, 40.0);
    let fill = p.color(\"fill\", rgb(220, 120, 60));
    let c = Canvas::new(w, h);
    c.paint(disk(w as f64 / 2.0, h as f64 / 2.0, radius), solid(fill));
    c
}
";

enum Editor {
    Script(String),
    Graph(Snarl<GeneratorNode>),
}

pub struct GeneratorWindow {
    open: bool,
    layer: Option<usize>,
    editor: Option<Editor>,
    error: Option<String>,
}

impl GeneratorWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            layer: None,
            editor: None,
            error: None,
        }
    }

    pub fn open_for(&mut self, layer: usize, generator: Generator) {
        self.open = true;
        self.layer = Some(layer);
        self.error = None;
        self.editor = Some(match generator.definition {
            GeneratorDefinition::Script(script) => Editor::Script(script),
            GeneratorDefinition::Graph(recipe) => match from_recipe(&recipe) {
                Ok(graph) => Editor::Graph(graph),
                Err(error) => {
                    self.error = Some(error);
                    Editor::Graph(Snarl::new())
                }
            },
        });
    }

    pub fn set_error(&mut self, error: Option<String>) {
        self.error = error;
    }

    pub fn update(&mut self, egui_ctx: &egui::Context) -> Vec<Effect> {
        let mut events = Vec::new();
        let (Some(layer), Some(editor)) = (self.layer, self.editor.as_mut()) else {
            return events;
        };
        if !self.open {
            return events;
        }

        let mut open = self.open;
        let kind = match editor {
            Editor::Script(_) => "script",
            Editor::Graph(_) => "graph",
        };
        egui::Window::new(format!("Generator {kind} — layer {}", layer + 1))
            .open(&mut open)
            .default_size(match editor {
                Editor::Script(_) => egui::vec2(480.0, 420.0),
                Editor::Graph(_) => egui::vec2(820.0, 620.0),
            })
            .show(egui_ctx, |ui| {
                if ui.button("Apply").clicked() {
                    let definition = match editor {
                        Editor::Script(script) => GeneratorDefinition::Script(script.clone()),
                        Editor::Graph(graph) => GeneratorDefinition::Graph(to_recipe(graph)),
                    };
                    events.push(Effect::UiEvent(UiEvent::SetGeneratorDefinition {
                        layer,
                        definition,
                    }));
                }

                if let Some(error) = &self.error {
                    ui.colored_label(egui::Color32::from_rgb(220, 90, 90), error);
                }
                ui.separator();

                match editor {
                    Editor::Script(script) => {
                        egui::ScrollArea::vertical()
                            .max_height(340.0)
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(script)
                                        .code_editor()
                                        .desired_rows(18)
                                        .desired_width(f32::INFINITY),
                                );
                            });
                    }
                    Editor::Graph(graph) => {
                        ui.weak(
                            "Right-click the graph to add nodes; right-click a node to delete it.",
                        );
                        graph.show(
                            &mut GraphViewer,
                            &SnarlStyle::new(),
                            ("generator-graph", layer),
                            ui,
                        );
                    }
                }
            });
        self.open = open;
        events
    }
}
