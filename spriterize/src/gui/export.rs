//! The export options both export windows share.

use lapix::{ExportOptions, Scale};

/// Scale ratios offered as buttons, alongside the custom fields.
const PRESETS: [(&str, u32, u32); 4] = [("1/2", 1, 2), ("1x", 1, 1), ("2x", 2, 1), ("4x", 4, 1)];

/// The options, and the half typed state of the fields that make them up.
pub struct ExportSettings {
    pub options: ExportOptions,
    padding_str: String,
    up_str: String,
    down_str: String,
}

impl ExportSettings {
    pub fn new() -> Self {
        Self {
            options: ExportOptions::default(),
            padding_str: "0".to_owned(),
            up_str: "1".to_owned(),
            down_str: "1".to_owned(),
        }
    }

    /// Shows the controls, and reports the size a canvas of `size` would come
    /// out as.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.options.crop, "Trim empty edges")
            .on_hover_text("cut away the fully transparent rows and columns around what is drawn");

        ui.horizontal(|ui| {
            ui.label("Padding:");

            if ui
                .add(egui::TextEdit::singleline(&mut self.padding_str).desired_width(40.))
                .on_hover_text("transparent pixels to add on every side")
                .changed()
            {
                if let Ok(padding) = self.padding_str.parse() {
                    self.options.padding = padding;
                }
            }

            ui.weak("px on each side");
        });

        ui.horizontal(|ui| {
            ui.label("Scale:");

            for (label, up, down) in PRESETS {
                let selected = self.options.scale == Scale::new(up, down);

                if ui.selectable_label(selected, label).clicked() {
                    self.set_scale(up, down);
                }
            }
        });

        ui.horizontal(|ui| {
            ui.weak("custom:");

            let up = ui
                .add(egui::TextEdit::singleline(&mut self.up_str).desired_width(30.))
                .on_hover_text("multiply by");
            ui.weak("/");
            let down = ui
                .add(egui::TextEdit::singleline(&mut self.down_str).desired_width(30.))
                .on_hover_text("divide by");

            if up.changed() || down.changed() {
                if let (Ok(up), Ok(down)) = (self.up_str.parse(), self.down_str.parse()) {
                    self.options.scale = Scale::new(up, down);
                }
            }
        });

        ui.checkbox(&mut self.options.power_of_two, "Round up to a power of two")
            .on_hover_text(
                "grow the width and height to the next power of two, added at the right and \
                 bottom so nothing shifts",
            );
    }

    /// What the exported image will measure, given the size it starts at.
    ///
    /// Trimming depends on what is actually drawn, so with it on the real size
    /// can only be smaller than this.
    pub fn resulting_size(&self, size: lapix::Size<i32>) -> lapix::Size<i32> {
        let padding = self.options.padding as i32 * 2;
        let scale = self.options.scale;
        let scaled = |side: i32| {
            let up = side.saturating_add(padding) * scale.up as i32;
            let down = scale.down as i32;

            // Matches the export: partial blocks are padded out, not split.
            (up + down - 1) / down
        };
        let (mut w, mut h) = (scaled(size.x), scaled(size.y));

        if self.options.power_of_two {
            w = lapix::export::next_power_of_two(w);
            h = lapix::export::next_power_of_two(h);
        }

        lapix::Size::new(w, h)
    }

    fn set_scale(&mut self, up: u32, down: u32) {
        self.options.scale = Scale::new(up, down);
        self.up_str = up.to_string();
        self.down_str = down.to_string();
    }
}
