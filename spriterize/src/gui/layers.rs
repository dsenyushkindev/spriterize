use crate::gui::generator::DEFAULT_SCRIPT;
use crate::gui::graph::default_recipe;
use crate::gui::layout::{self, PanelLayout};
use crate::{Effect, UiEvent};
use artlib_script::{Knob, KnobKind, KnobValue};
use lapix::filter::{ParamKind, Value};
use lapix::{Event, Filter, GenValue, Generator, GeneratorDefinition, GeneratorNode};
use std::collections::HashMap;

/// The settings of one filter. Returns the filter with them applied, if any
/// were changed.
///
/// Built from what the filter declares rather than from a match on known
/// filters, so a newly registered one gets working controls for free.
fn filter_properties(ui: &mut egui::Ui, filter: &Filter) -> Option<Filter> {
    let Some(kind) = filter.kind() else {
        ui.weak("this filter isn't available in this build");
        return None;
    };
    let specs = kind.params();

    if specs.is_empty() {
        ui.weak("nothing to adjust");
        return None;
    }

    let mut params = filter.params.clone();
    let mut changed = false;

    for spec in specs {
        ui.horizontal(|ui| {
            ui.label(format!("{}:", spec.label));

            let response = match spec.kind {
                ParamKind::Ratio => {
                    let mut value = params.int(spec.id, default_int(spec));
                    let response = ui.add(
                        egui::Slider::new(&mut value, 0..=lapix::filter::FULL_STRENGTH)
                            .custom_formatter(|v, _| {
                                format!("{:.0}%", v / lapix::filter::FULL_STRENGTH as f64 * 100.)
                            }),
                    );

                    if response.changed() {
                        params.set(spec.id, Value::Int(value));
                    }

                    response
                }
                ParamKind::Int { min, max } => {
                    let mut value = params.int(spec.id, default_int(spec));
                    let response = ui.add(egui::Slider::new(&mut value, min..=max));

                    if response.changed() {
                        params.set(spec.id, Value::Int(value));
                    }

                    response
                }
                ParamKind::Color => {
                    let color = params.color(spec.id, default_color(spec));
                    let mut rgba = [color.r, color.g, color.b, color.a];
                    let response = ui.color_edit_button_srgba_unmultiplied(&mut rgba);

                    if response.changed() {
                        let color = lapix::Color::new(rgba[0], rgba[1], rgba[2], rgba[3]);
                        params.set(spec.id, Value::Color(color));
                    }

                    response
                }
                ParamKind::Bool => {
                    let mut value = params.bool(spec.id, default_bool(spec));
                    let response = ui.checkbox(&mut value, "");

                    if response.changed() {
                        params.set(spec.id, Value::Bool(value));
                    }

                    response
                }
            };

            changed |= response.changed();
            response.on_hover_text(spec.help);
        });
    }

    changed.then(|| Filter {
        id: filter.id.clone(),
        params,
    })
}

fn default_int(spec: &lapix::filter::ParamSpec) -> i32 {
    match spec.default {
        Value::Int(v) => v,
        _ => 0,
    }
}

fn default_color(spec: &lapix::filter::ParamSpec) -> lapix::Color {
    match spec.default {
        Value::Color(c) => c,
        _ => lapix::color::BLACK,
    }
}

fn default_bool(spec: &lapix::filter::ParamSpec) -> bool {
    match spec.default {
        Value::Bool(v) => v,
        _ => false,
    }
}

/// Render one generator knob, given its declaration and current value. Returns
/// the new value if it changed. Mirrors the filter param controls, with a float
/// slider added for `KnobKind::Float`.
fn render_gen_knob(ui: &mut egui::Ui, knob: &Knob, current: Option<&GenValue>) -> Option<GenValue> {
    match &knob.kind {
        KnobKind::Float { min, max } => {
            let mut value = match current {
                Some(GenValue::Float(v)) => *v,
                _ => gen_float_default(knob),
            };
            let response = ui.add(egui::Slider::new(&mut value, (*min as f32)..=(*max as f32)));
            response.changed().then_some(GenValue::Float(value))
        }
        KnobKind::Int { min, max } => {
            let mut value = match current {
                Some(GenValue::Int(v)) => *v,
                _ => gen_int_default(knob),
            };
            let response = ui.add(egui::Slider::new(&mut value, *min..=*max));
            response.changed().then_some(GenValue::Int(value))
        }
        KnobKind::Color => {
            let mut rgba = match current {
                Some(GenValue::Color(c)) => [c.r, c.g, c.b, c.a],
                _ => gen_color_default(knob),
            };
            let response = ui.color_edit_button_srgba_unmultiplied(&mut rgba);
            response
                .changed()
                .then(|| GenValue::Color(lapix::Color::new(rgba[0], rgba[1], rgba[2], rgba[3])))
        }
        KnobKind::Bool => {
            let mut value = match current {
                Some(GenValue::Bool(v)) => *v,
                _ => gen_bool_default(knob),
            };
            let response = ui.checkbox(&mut value, "");
            response.changed().then_some(GenValue::Bool(value))
        }
    }
}

fn gen_float_default(knob: &Knob) -> f32 {
    match &knob.default {
        KnobValue::Float(v) => *v as f32,
        _ => 0.0,
    }
}

fn gen_int_default(knob: &Knob) -> i64 {
    match &knob.default {
        KnobValue::Int(v) => *v,
        _ => 0,
    }
}

fn gen_color_default(knob: &Knob) -> [u8; 4] {
    match &knob.default {
        KnobValue::Color(c) => *c,
        _ => [0, 0, 0, 255],
    }
}

fn gen_bool_default(knob: &Knob) -> bool {
    match &knob.default {
        KnobValue::Bool(v) => *v,
        _ => false,
    }
}

fn graph_knobs(generator: &Generator) -> Result<Vec<Knob>, String> {
    let Some(graph) = generator.graph_definition() else {
        return Ok(Vec::new());
    };
    let mut ids = std::collections::HashSet::new();
    let mut knobs = Vec::new();
    for held in &graph.nodes {
        let knob = match &held.node {
            GeneratorNode::FloatKnob {
                id,
                default,
                min,
                max,
            } => {
                if !default.is_finite() || !min.is_finite() || !max.is_finite() || min > max {
                    return Err(format!("invalid range for parameter `{id}`"));
                }
                Some(Knob {
                    id: id.clone(),
                    kind: KnobKind::Float {
                        min: *min as f64,
                        max: *max as f64,
                    },
                    default: KnobValue::Float(*default as f64),
                })
            }
            GeneratorNode::IntKnob {
                id,
                default,
                min,
                max,
            } => {
                if min > max {
                    return Err(format!("invalid range for parameter `{id}`"));
                }
                Some(Knob {
                    id: id.clone(),
                    kind: KnobKind::Int {
                        min: *min,
                        max: *max,
                    },
                    default: KnobValue::Int(*default),
                })
            }
            GeneratorNode::ColorKnob { id, default } => Some(Knob {
                id: id.clone(),
                kind: KnobKind::Color,
                default: KnobValue::Color(*default),
            }),
            GeneratorNode::BoolKnob { id, default } => Some(Knob {
                id: id.clone(),
                kind: KnobKind::Bool,
                default: KnobValue::Bool(*default),
            }),
            _ => None,
        };
        if let Some(knob) = knob {
            if knob.id.trim().is_empty() {
                return Err("a graph parameter has an empty name".to_owned());
            }
            if !ids.insert(knob.id.clone()) {
                return Err(format!("duplicate parameter name `{}`", knob.id));
            }
            knobs.push(knob);
        }
    }
    Ok(knobs)
}

/// Narrowest the layer name field is allowed to get, whatever the other columns
/// need.
const MIN_NAME_WIDTH: f32 = 120.;
/// Width of the opacity field, enough for three digits.
const ALPHA_WIDTH: f32 = 34.;
/// Column headings. The last is blank: it reserves the column holding the
/// reorder and delete buttons, which needs no label.
const HEADINGS: [&str; 5] = ["Name", "Active", "Visible", "Alpha", ""];
const COLUMNS: usize = HEADINGS.len();

pub struct LayersPanel {
    num_layers: usize,
    active_layer: usize,
    layers_vis: Vec<bool>,
    layers_alpha: Vec<String>,
    layers_names: Vec<String>,
    layers_filters: Vec<Vec<Filter>>,
    layers_adjustment: Vec<bool>,
    layers_generators: Vec<Option<Generator>>,
    filters_enabled: bool,
    /// Which layer's filter chain is expanded, if any.
    editing_filters: Option<usize>,
    /// Which layer's generator is expanded, if any.
    editing_generator: Option<usize>,
    /// Canvas size, to run generators at and to size their output.
    canvas_size: (usize, usize),
    /// The knobs each script declares, cached by script so the controls don't
    /// re-run the script every frame. Holds the error if a script won't compile.
    knob_cache: HashMap<String, Result<Vec<Knob>, String>>,
    /// Width of the name field, set to whatever the other columns leave over.
    name_width: f32,
}

impl LayersPanel {
    pub fn new() -> Self {
        Self {
            num_layers: 1,
            active_layer: 0,
            layers_vis: vec![true],
            layers_alpha: vec!["255".to_owned()],
            layers_names: vec!["Layer 1".to_owned()],
            layers_filters: vec![Vec::new()],
            layers_adjustment: vec![false],
            layers_generators: vec![None],
            filters_enabled: true,
            editing_filters: None,
            editing_generator: None,
            canvas_size: (64, 64),
            knob_cache: HashMap::new(),
            name_width: MIN_NAME_WIDTH,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sync(
        &mut self,
        num_layers: usize,
        active_layer: usize,
        layers_vis: Vec<bool>,
        layers_alpha: Vec<u8>,
        layers_names: Vec<String>,
        layers_filters: Vec<Vec<Filter>>,
        layers_adjustment: Vec<bool>,
        layers_generators: Vec<Option<Generator>>,
        filters_enabled: bool,
        canvas_size: (usize, usize),
    ) {
        self.active_layer = active_layer;
        self.num_layers = num_layers;
        self.layers_vis = layers_vis;
        self.layers_alpha = layers_alpha.into_iter().map(|x| x.to_string()).collect();
        self.layers_names = layers_names;
        self.layers_filters = layers_filters;
        self.layers_adjustment = layers_adjustment;
        self.layers_generators = layers_generators;
        self.filters_enabled = filters_enabled;
        self.canvas_size = canvas_size;

        // A layer that was being edited may have been deleted.
        if self.editing_filters.is_some_and(|i| i >= num_layers) {
            self.editing_filters = None;
        }
        if self.editing_generator.is_some_and(|i| i >= num_layers) {
            self.editing_generator = None;
        }
    }

    /// The row of filter controls for one layer, shown under it when expanded.
    ///
    /// Every change sends the whole chain back, so adding, removing and
    /// reordering are all one step to undo.
    fn filter_chain(&self, ui: &mut egui::Ui, layer: usize, events: &mut Vec<Effect>) {
        let filters = &self.layers_filters[layer];
        let mut changed: Option<Vec<Filter>> = None;

        let mut adjustment = self.layers_adjustment[layer];

        if ui
            .checkbox(&mut adjustment, "apply to the layers below")
            .on_hover_text(
                "instead of filtering its own pixels, this layer filters everything \
                 stacked beneath it",
            )
            .changed()
        {
            events.push(Event::SetLayerAdjustment(layer, adjustment).into());
        }

        if filters.is_empty() {
            ui.weak("no filters");
        }

        // One block per filter, stacked, so each has room for its own settings.
        for (i, filter) in filters.iter().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.strong(format!("{}. {}", i + 1, filter.name()));

                    // Pushed to the right so the controls line up down the list.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("x").on_hover_text("remove filter").clicked() {
                            let mut next = filters.clone();
                            next.remove(i);
                            changed = Some(next);
                        }
                        if ui
                            .add_enabled(i + 1 < filters.len(), egui::Button::new("v"))
                            .on_hover_text("run later")
                            .clicked()
                        {
                            let mut next = filters.clone();
                            next.swap(i, i + 1);
                            changed = Some(next);
                        }
                        if ui
                            .add_enabled(i > 0, egui::Button::new("^"))
                            .on_hover_text("run earlier")
                            .clicked()
                        {
                            let mut next = filters.clone();
                            next.swap(i - 1, i);
                            changed = Some(next);
                        }
                    });
                });

                if let Some(adjusted) = filter_properties(ui, filter) {
                    let mut next = filters.clone();
                    next[i] = adjusted;
                    changed = Some(next);
                }
            });
        }

        // Offers whatever is registered, so a new filter shows up here without
        // this needing to know about it.
        ui.menu_button("add filter", |ui| {
            for kind in lapix::filter::kinds() {
                if ui.button(kind.name()).clicked() {
                    let mut next = filters.clone();
                    next.push(Filter::new(kind));
                    changed = Some(next);
                    ui.close_menu();
                }
            }
        });

        if let Some(filters) = changed {
            events.push(Event::SetLayerFilters(layer, filters).into());
        }
    }

    /// The generator controls for one layer, shown under it when expanded: an
    /// Edit-script button, the knobs the script declares, and remove — or, if
    /// the layer has no generator yet, a button to add one.
    fn generator_section(&mut self, ui: &mut egui::Ui, layer: usize, events: &mut Vec<Effect>) {
        // Cloned up front so the knob cache — also on `self` — can be borrowed
        // mutably below without conflicting with the recipe.
        let generator = self.layers_generators[layer].clone();

        let Some(generator) = generator else {
            ui.horizontal(|ui| {
                if ui.button("Add script").clicked() {
                    events.push(Effect::UiEvent(UiEvent::UpdateGenerator {
                        layer,
                        generator: Generator::new(DEFAULT_SCRIPT.to_owned()),
                    }));
                }
                if ui.button("Add graph").clicked() {
                    events.push(Effect::UiEvent(UiEvent::UpdateGenerator {
                        layer,
                        generator: Generator::graph(default_recipe()),
                    }));
                }
            });
            return;
        };

        ui.horizontal(|ui| {
            if ui
                .button(match generator.definition {
                    GeneratorDefinition::Script(_) => "Edit script…",
                    GeneratorDefinition::Graph(_) => "Edit graph…",
                })
                .clicked()
            {
                events.push(Effect::UiEvent(UiEvent::OpenGeneratorEditor { layer }));
            }
            if ui
                .button("remove")
                .on_hover_text("remove the generator (keeps the pixels it made)")
                .clicked()
            {
                events.push(Effect::UiEvent(UiEvent::RemoveGenerator { layer }));
            }
        });

        let (w, h) = self.canvas_size;
        let graph_declared;
        let knobs = match &generator.definition {
            GeneratorDefinition::Script(script) => self
                .knob_cache
                .entry(script.clone())
                .or_insert_with(|| artlib_script::declared_knobs(script, w, h)),
            GeneratorDefinition::Graph(_) => {
                graph_declared = graph_knobs(&generator);
                &graph_declared
            }
        };

        match knobs {
            Ok(knobs) => {
                if knobs.is_empty() {
                    ui.weak("no parameters");
                }

                let mut updated: Option<Generator> = None;
                for knob in knobs.iter() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}:", knob.id));
                        if let Some(value) = render_gen_knob(ui, knob, generator.get(&knob.id)) {
                            let mut next = generator.clone();
                            next.set(&knob.id, value);
                            updated = Some(next);
                        }
                    });
                }

                if let Some(generator) = updated {
                    events.push(Effect::UiEvent(UiEvent::UpdateGenerator {
                        layer,
                        generator,
                    }));
                }
            }
            Err(error) => {
                ui.colored_label(egui::Color32::from_rgb(220, 90, 90), error.clone());
            }
        }
    }

    pub fn update(&mut self, egui_ctx: &egui::Context, layout: &PanelLayout) -> Vec<Effect> {
        let mut events = Vec::new();

        layout.show(egui_ctx, layout::LAYERS, |ui| {
            let btn = ui.button("+");
            if btn.clicked() {
                events.push(Event::NewLayerAbove.into());
                events.push(Event::SwitchLayer(self.num_layers).into());
            }

            // Where each column sits, gathered as the grid is built so the
            // dividers can be drawn in the gaps between columns afterwards.
            // Measuring beats putting separators in the cells, which would push
            // the headings out of line with the controls under them.
            let mut columns: Vec<egui::Rect> = Vec::with_capacity(COLUMNS);
            let mut header = egui::Rect::NOTHING;

            // A grid, so the headings line up with the controls underneath them:
            // its columns are sized to their widest cell.
            egui::Grid::new("layers")
                .num_columns(COLUMNS)
                .striped(true)
                .show(ui, |ui| {
                    for heading in HEADINGS {
                        let rect = ui.label(heading).rect;
                        columns.push(rect);
                        header = header.union(rect);
                    }
                    ui.end_row();

                    // Topmost layer first, matching how they stack on screen.
                    for i in (0..self.num_layers).rev() {
                        // Sized explicitly rather than with `desired_width`: a
                        // grid cell offers only the width its column had last
                        // frame, and a text edit shrinks to fit that, so the
                        // column could never grow past the 40pt egui starts it
                        // at. Allocating the size makes the cell that wide.
                        let name = ui.add_sized(
                            [self.name_width, ui.spacing().interact_size.y],
                            egui::widgets::TextEdit::singleline(&mut self.layers_names[i]),
                        );

                        // The controls are wider than the headings, so they are
                        // what actually decides where each column starts and
                        // ends.
                        if i == self.num_layers - 1 {
                            columns[0] = columns[0].union(name.rect);
                        }

                        if name.changed() {
                            events.push(Event::RenameLayer(i, self.layers_names[i].clone()).into());
                        }

                        name.on_hover_text("layer name, also its file name when exporting layers");

                        let tooltip = format!("select layer {}", i + 1);
                        let active = ui.radio(i == self.active_layer, "").on_hover_text(tooltip);

                        if active.clicked() {
                            events.push(Event::SwitchLayer(i).into());
                        }

                        let tooltip = format!("toggle visibility of layer {}", i + 1);
                        let visible = ui.radio(self.layers_vis[i], "").on_hover_text(tooltip);

                        if visible.clicked() {
                            events
                                .push(Event::ChangeLayerVisibility(i, !self.layers_vis[i]).into());
                        }

                        let text_edit = ui.add(
                            egui::widgets::TextEdit::singleline(&mut self.layers_alpha[i])
                                .desired_width(ALPHA_WIDTH),
                        );

                        if text_edit.changed() {
                            if let Ok(opacity) = self.layers_alpha[i].parse() {
                                events.push(Event::ChangeLayerOpacity(i, opacity).into());
                            }
                        }

                        let buttons = ui.horizontal(|ui| {
                            // Move layer below button
                            ui.add_enabled_ui(i > 0, |ui| {
                                if ui.button("v").on_hover_text("move layer down").clicked() {
                                    events.push(Event::MoveLayerDown(i).into());
                                    events.push(Event::SwitchLayer(i - 1).into());
                                }
                            });
                            // Move layer above button
                            ui.add_enabled_ui(i < self.num_layers - 1, |ui| {
                                if ui.button("^").on_hover_text("move layer up").clicked() {
                                    events.push(Event::MoveLayerUp(i).into());
                                    events.push(Event::SwitchLayer(i + 1).into());
                                }
                            });
                            // Filter chain toggle
                            let count = self.layers_filters[i].len();
                            let label = if count == 0 {
                                "fx".to_owned()
                            } else {
                                format!("fx {count}")
                            };
                            let expanded = self.editing_filters == Some(i);

                            if ui
                                .selectable_label(expanded, label)
                                .on_hover_text("filters applied to this layer")
                                .clicked()
                            {
                                self.editing_filters = if expanded { None } else { Some(i) };
                            }
                            // Generator toggle. A dot marks a layer that has one.
                            let gen_label = if self.layers_generators[i].is_some() {
                                "gen•"
                            } else {
                                "gen"
                            };
                            let gen_expanded = self.editing_generator == Some(i);

                            if ui
                                .selectable_label(gen_expanded, gen_label)
                                .on_hover_text("procedural generator that fills this layer")
                                .clicked()
                            {
                                self.editing_generator = if gen_expanded { None } else { Some(i) };
                            }
                            // Delete layer button
                            ui.add_enabled_ui(self.num_layers > 1, |ui| {
                                if ui.button("x").on_hover_text("delete layer").clicked() {
                                    events.push(Event::DeleteLayer(i).into());

                                    let select_layer = match self.active_layer {
                                        x if i > x => self.active_layer,
                                        x if i == x && i == 0 => 0,
                                        _ => self.active_layer - 1,
                                    };
                                    events.push(Event::SwitchLayer(select_layer).into());
                                }
                            });
                        });

                        if i == self.num_layers - 1 {
                            for (column, rect) in columns.iter_mut().skip(1).zip([
                                active.rect,
                                visible.rect,
                                text_edit.rect,
                                buttons.response.rect,
                            ]) {
                                *column = column.union(rect);
                            }
                        }

                        ui.end_row();
                    }
                });

            if let Some(layer) = self.editing_filters {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.strong(&self.layers_names[layer]);

                    if !self.filters_enabled {
                        ui.weak("(filters are hidden)");
                    }
                });
                self.filter_chain(ui, layer, &mut events);
            }

            if let Some(layer) = self.editing_generator {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.strong(&self.layers_names[layer]);
                    ui.weak("generator");
                });
                self.generator_section(ui, layer, &mut events);
            }

            // Hand the name field whatever width the other columns don't need,
            // so a row fills the panel. Their widths don't depend on the name
            // field, so this settles immediately rather than creeping.
            if let (Some(first_other), Some(last)) = (columns.get(1), columns.last()) {
                let others = last.right() - first_other.left() + ui.spacing().item_spacing.x;

                self.name_width = (layout::PANEL_WIDTH - others).max(MIN_NAME_WIDTH);
            }

            // Dividers between the headings, marking where one column ends and
            // the next begins.
            let stroke = ui.visuals().widgets.noninteractive.bg_stroke;

            for pair in columns.windows(2) {
                let x = (pair[0].right() + pair[1].left()) / 2.;

                ui.painter().vline(x, header.y_range(), stroke);
            }
        });

        events
    }
}
