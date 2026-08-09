//! The Generator window: fill the active layer from an artlib script.
//!
//! A script composes artlib's shapes, noise and compositing in text (see the
//! `artlib-script` crate) and returns a canvas of pixels. It can declare
//! **knobs** — `p.num(...)`, `p.color(...)` — which show up here as controls, so
//! a generated layer can be re-tweaked parametrically without opening the
//! script.
//!
//! Running the script and its knob state live here in the window; the resulting
//! pixels are handed to [`UiState`](crate::UiState) through a single
//! [`UiEvent::SetGeneratedImage`], which drops them into the active cel as one
//! undoable step.

use crate::{Effect, UiEvent};
use artlib_script::{generate, Knob, KnobKind, KnobValue, KnobValues};
use lapix::Size;

/// A starter script, shown the first time the window opens. Declares a few knobs
/// so the parameter controls are visible immediately.
const DEFAULT_SCRIPT: &str = "\
// Fills the active layer. `p` declares knobs — tweak them below without
// editing this. Returns a Canvas of w x h pixels.
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
    script: String,
    /// The knobs the last run declared, in the order the script asked for them.
    knobs: Vec<Knob>,
    /// The current value of each knob, by id.
    values: KnobValues,
    /// The last compile or run error, shown until the next successful run.
    error: Option<String>,
    /// The canvas size to generate at, so the output matches the cel it fills.
    canvas_size: (usize, usize),
}

impl GeneratorWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            script: DEFAULT_SCRIPT.to_owned(),
            knobs: Vec::new(),
            values: KnobValues::new(),
            error: None,
            canvas_size: (64, 64),
        }
    }

    pub fn sync(&mut self, canvas_size: Size<i32>) {
        self.canvas_size = (canvas_size.x.max(1) as usize, canvas_size.y.max(1) as usize);
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn update(&mut self, egui_ctx: &egui::Context) -> Vec<Effect> {
        let mut events = Vec::new();

        if !self.open {
            return events;
        }

        let mut open = self.open;
        // Set when the script should be (re)run this frame: the Generate button,
        // or any knob moving.
        let mut run = false;

        egui::Window::new("Generator")
            .open(&mut open)
            .default_width(380.)
            .show(egui_ctx, |ui| {
                let (w, h) = self.canvas_size;
                ui.label(
                    egui::RichText::new(format!("Fills the active layer ({w}×{h})."))
                        .weak()
                        .small(),
                );

                egui::ScrollArea::vertical()
                    .max_height(220.)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.script)
                                .code_editor()
                                .desired_rows(12)
                                .desired_width(f32::INFINITY),
                        );
                    });

                ui.horizontal(|ui| {
                    if ui
                        .button("Generate")
                        .on_hover_text("run the script into the active layer")
                        .clicked()
                    {
                        run = true;
                    }
                    if !self.knobs.is_empty() {
                        ui.label(
                            egui::RichText::new("· knobs update live")
                                .weak()
                                .small(),
                        );
                    }
                });

                if !self.knobs.is_empty() {
                    ui.separator();
                    ui.label("Parameters");

                    for knob in &self.knobs {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}:", knob.id));
                            if render_knob(ui, knob, &mut self.values) {
                                run = true;
                            }
                        });
                    }
                }

                if let Some(error) = &self.error {
                    ui.separator();
                    ui.colored_label(egui::Color32::from_rgb(220, 90, 90), error);
                }
            });

        self.open = open;

        if run {
            let (w, h) = self.canvas_size;
            match generate(&self.script, w, h, self.values.clone()) {
                Ok(result) => {
                    self.knobs = result.knobs;
                    self.error = None;
                    events.push(Effect::UiEvent(UiEvent::SetGeneratedImage {
                        width: w,
                        height: h,
                        pixels: result.pixels,
                    }));
                }
                Err(message) => self.error = Some(message),
            }
        }

        events
    }
}

/// Render one knob's control, editing `values`. Returns whether it changed.
/// Mirrors the filter `ParamSpec` controls, with a float slider added.
fn render_knob(ui: &mut egui::Ui, knob: &Knob, values: &mut KnobValues) -> bool {
    let response = match &knob.kind {
        KnobKind::Float { min, max } => {
            let mut value = match values.get(&knob.id) {
                Some(KnobValue::Float(v)) => *v,
                _ => float_default(knob),
            };
            let response = ui.add(egui::Slider::new(&mut value, *min..=*max));
            if response.changed() {
                values.insert(knob.id.clone(), KnobValue::Float(value));
            }
            response
        }
        KnobKind::Int { min, max } => {
            let mut value = match values.get(&knob.id) {
                Some(KnobValue::Int(v)) => *v,
                _ => int_default(knob),
            };
            let response = ui.add(egui::Slider::new(&mut value, *min..=*max));
            if response.changed() {
                values.insert(knob.id.clone(), KnobValue::Int(value));
            }
            response
        }
        KnobKind::Color => {
            let mut rgba = match values.get(&knob.id) {
                Some(KnobValue::Color(c)) => *c,
                _ => color_default(knob),
            };
            let response = ui.color_edit_button_srgba_unmultiplied(&mut rgba);
            if response.changed() {
                values.insert(knob.id.clone(), KnobValue::Color(rgba));
            }
            response
        }
        KnobKind::Bool => {
            let mut value = match values.get(&knob.id) {
                Some(KnobValue::Bool(v)) => *v,
                _ => bool_default(knob),
            };
            let response = ui.checkbox(&mut value, "");
            if response.changed() {
                values.insert(knob.id.clone(), KnobValue::Bool(value));
            }
            response
        }
    };

    response.changed()
}

fn float_default(knob: &Knob) -> f64 {
    match knob.default {
        KnobValue::Float(v) => v,
        _ => 0.0,
    }
}

fn int_default(knob: &Knob) -> i64 {
    match knob.default {
        KnobValue::Int(v) => v,
        _ => 0,
    }
}

fn color_default(knob: &Knob) -> [u8; 4] {
    match knob.default {
        KnobValue::Color(c) => c,
        _ => [0, 0, 0, 255],
    }
}

fn bool_default(knob: &Knob) -> bool {
    match knob.default {
        KnobValue::Bool(v) => v,
        _ => false,
    }
}
