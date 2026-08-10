//! Script and graph editors for a layer generator.

use crate::collection::ElementResource;
use crate::gui::graph::{from_recipe, to_recipe, GraphPreviewCache, GraphViewer};
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
    target: Option<Target>,
    editor: Option<Editor>,
    error: Option<String>,
    elements: Vec<ElementResource>,
    previews: GraphPreviewCache,
    canvas_size: (usize, usize),
    element_preview_size: (usize, usize),
    show_previews: bool,
    node_filter: String,
}

enum Target {
    Layer(usize),
    Element { id: String, name: String },
}

impl GeneratorWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            target: None,
            editor: None,
            error: None,
            elements: Vec::new(),
            previews: GraphPreviewCache::new(),
            canvas_size: (64, 64),
            element_preview_size: (80, 80),
            show_previews: true,
            node_filter: String::new(),
        }
    }

    pub fn open_for(&mut self, layer: usize, generator: Generator) {
        self.open = true;
        self.target = Some(Target::Layer(layer));
        self.error = None;
        self.previews.clear();
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

    pub fn sync_context(&mut self, elements: Vec<ElementResource>, canvas_size: (usize, usize)) {
        if self.elements != elements || self.canvas_size != canvas_size {
            self.previews.clear();
        }
        self.elements = elements;
        self.canvas_size = canvas_size;
    }

    pub fn open_element(&mut self, element: ElementResource) {
        self.open = true;
        self.error = None;
        self.previews.clear();
        self.element_preview_size = (80, 80);
        self.target = Some(Target::Element {
            id: element.id,
            name: element.name,
        });
        self.editor = Some(match from_recipe(&element.graph) {
            Ok(graph) => Editor::Graph(graph),
            Err(error) => {
                self.error = Some(error);
                Editor::Graph(Snarl::new())
            }
        });
    }

    pub fn set_error(&mut self, error: Option<String>) {
        self.error = error;
    }

    pub fn update(&mut self, egui_ctx: &egui::Context) -> Vec<Effect> {
        let mut events = Vec::new();
        let (Some(target), Some(editor)) = (self.target.as_mut(), self.editor.as_mut()) else {
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
        let title = match target {
            Target::Layer(layer) => format!("Generator {kind} — layer {}", *layer + 1),
            Target::Element { name, .. } => format!("Element — {name}"),
        };
        egui::Window::new(title)
            .open(&mut open)
            .default_size(match editor {
                Editor::Script(_) => egui::vec2(480.0, 420.0),
                Editor::Graph(_) => egui::vec2(820.0, 620.0),
            })
            .show(egui_ctx, |ui| {
                if ui.button("Apply").clicked() {
                    match target {
                        Target::Layer(layer) => {
                            let definition = match editor {
                                Editor::Script(script) => GeneratorDefinition::Script(script.clone()),
                                Editor::Graph(graph) => GeneratorDefinition::Graph(to_recipe(graph)),
                            };
                            events.push(Effect::UiEvent(UiEvent::SetGeneratorDefinition {
                                layer: *layer,
                                definition,
                            }));
                        }
                        Target::Element { id, name } => {
                            let Editor::Graph(graph) = editor else { unreachable!() };
                            events.push(Effect::UiEvent(UiEvent::SetCollectionElement {
                                id: id.clone(),
                                name: name.trim().to_owned(),
                                graph: to_recipe(graph),
                            }));
                        }
                    }
                }

                if let Target::Element { name, .. } = target {
                    ui.horizontal(|ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(name);
                    });
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
                        let defining_element = match target {
                            Target::Element { id, .. } => Some(id.as_str()),
                            Target::Layer(_) => None,
                        };
                        ui.horizontal(|ui| {
                            ui.weak(if defining_element.is_some() {
                                "Right-click to add typed element ports, operations, or other elements. Apply updates every caller."
                            } else {
                                "Right-click the graph to add nodes or reusable collection elements; right-click a node to delete it."
                            });
                            ui.separator();
                            ui.checkbox(&mut self.show_previews, "Node previews");
                            if defining_element.is_some() && self.show_previews {
                                ui.separator();
                                ui.label("Preview");
                                let width = ui.add(
                                    egui::DragValue::new(&mut self.element_preview_size.0)
                                        .range(16..=512)
                                        .prefix("W "),
                                );
                                let height = ui.add(
                                    egui::DragValue::new(&mut self.element_preview_size.1)
                                        .range(16..=512)
                                        .prefix("H "),
                                );
                                if width.changed() || height.changed() {
                                    self.previews.clear();
                                }
                            }
                        });
                        self.previews.prepare(graph);
                        let preview_size = if defining_element.is_some() {
                            self.element_preview_size
                        } else {
                            self.canvas_size
                        };
                        let mut viewer = GraphViewer {
                            elements: &self.elements,
                            defining_element,
                            previews: &mut self.previews,
                            preview_size,
                            show_previews: self.show_previews,
                            node_filter: &mut self.node_filter,
                        };
                        graph.show(
                            &mut viewer,
                            &SnarlStyle::new(),
                            ("generator-graph", defining_element.unwrap_or("layer")),
                            ui,
                        );
                    }
                }
            });
        self.open = open;
        events
    }
}
