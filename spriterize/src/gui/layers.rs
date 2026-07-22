use crate::gui::layout::{self, PanelLayout};
use crate::Effect;
use lapix::Event;

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
            name_width: MIN_NAME_WIDTH,
        }
    }

    pub fn sync(
        &mut self,
        num_layers: usize,
        active_layer: usize,
        layers_vis: Vec<bool>,
        layers_alpha: Vec<u8>,
        layers_names: Vec<String>,
    ) {
        self.active_layer = active_layer;
        self.num_layers = num_layers;
        self.layers_vis = layers_vis;
        self.layers_alpha = layers_alpha.into_iter().map(|x| x.to_string()).collect();
        self.layers_names = layers_names;
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
