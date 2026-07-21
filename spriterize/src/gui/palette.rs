use crate::wrapped_image::WrappedImage;
use crate::Effect;
use egui::ecolor::Hsva;
use egui::widgets::color_picker::{self, Alpha};
use lapix::{Bitmap, Color, Event};
use macroquad::prelude::Image as MqImage;

const BTN_SIZE: i32 = 20;
/// Width of the color picker's bars and square. egui's default slider width is
/// meant for popups and is too cramped for a picker that's always on screen.
const PICKER_WIDTH: f32 = 200.;

pub struct Palette {
    colors: Vec<[u8; 4]>,
    images: Vec<MqImage>,
    egui_images: Vec<egui::ColorImage>,
    textures: Vec<Option<egui::TextureHandle>>,
    /// The color being edited. Held as `Hsva` rather than being derived from
    /// the main color each frame because hue and saturation aren't recoverable
    /// from RGB once the color reaches black or gray, and dragging through
    /// those would otherwise reset the picker.
    color: Hsva,
    hex: String,
}

impl Palette {
    pub fn new() -> Self {
        Self {
            colors: Vec::new(),
            images: Vec::new(),
            egui_images: Vec::new(),
            textures: Vec::new(),
            color: Hsva::from_srgba_unmultiplied([0, 0, 0, 255]),
            hex: Color::new(0, 0, 0, 255).hex(),
        }
    }

    // TODO: this is a copy and paste of the sync fn in `Preview`, DRY
    pub fn sync(&mut self, colors: Vec<[u8; 4]>, main_color: [u8; 4]) {
        if !colors.is_empty() {
            self.colors = colors;
            self.images = self
                .colors
                .iter()
                .map(|c| WrappedImage::new((BTN_SIZE, BTN_SIZE).into(), (*c).into()).0)
                .collect();
            self.textures = (0..self.images.len()).map(|_| None).collect();
            self.egui_images = Vec::new();

            for image in &self.images {
                let w = image.width();
                let h = image.height();
                let img = egui::ColorImage::from_rgba_unmultiplied([w, h], &image.bytes);
                self.egui_images.push(img);
            }
        }

        // Adopt the main color only when it changed somewhere else — the
        // eyedropper, or a palette swatch. If it already matches the picker,
        // overwriting would throw away the hue the picker is holding onto.
        if self.color.to_srgba_unmultiplied() != main_color {
            self.set_color(main_color);
        }
    }

    fn set_color(&mut self, color: [u8; 4]) {
        self.color = Hsva::from_srgba_unmultiplied(color);
        self.hex = Color::from(color).hex();
    }

    fn current_color(&self) -> [u8; 4] {
        self.color.to_srgba_unmultiplied()
    }

    pub fn update(&mut self, egui_ctx: &egui::Context) -> Vec<Effect> {
        let mut fx = Vec::new();

        egui::Window::new("Palette")
            .default_pos((15., 30.))
            .show(egui_ctx, |ui| {
                let changed = ui
                    .scope(|ui| {
                        ui.spacing_mut().slider_width = PICKER_WIDTH;
                        color_picker::color_picker_hsva_2d(ui, &mut self.color, Alpha::OnlyBlend)
                    })
                    .inner;

                if changed {
                    let color = self.current_color();
                    self.hex = Color::from(color).hex();
                    fx.push(Event::SetMainColor(color.into()).into());
                }

                ui.horizontal(|ui| {
                    let label = ui.label("hex:");
                    let edit = ui
                        .add(egui::TextEdit::singleline(&mut self.hex).desired_width(90.))
                        .labelled_by(label.id);

                    if edit.changed() {
                        if let Some(color) = parse_hex(&self.hex) {
                            self.color = Hsva::from_srgba_unmultiplied(color);
                            fx.push(Event::SetMainColor(color.into()).into());
                        }
                    }

                    if ui
                        .button("+")
                        .on_hover_text("add this color to the palette")
                        .clicked()
                    {
                        fx.push(Event::AddToPalette(self.current_color().into()).into());
                    }
                });

                ui.separator();

                ui.horizontal(|ui| {
                    if ui
                        .button("Load")
                        .on_hover_text("load a .gpl palette, or take the colors from an image")
                        .clicked()
                    {
                        let dialog = rfd::FileDialog::new()
                            .add_filter("Palettes and images", &["gpl", "png", "jpg", "jpeg"])
                            .add_filter("GIMP palette", &["gpl"])
                            .add_filter("All files", &["*"]);

                        if let Some(path) = dialog.pick_file() {
                            fx.push(Event::LoadPalette(path).into());
                        }
                    }

                    if ui
                        .button("Save")
                        .on_hover_text("save this palette as a .gpl file")
                        .clicked()
                    {
                        let dialog = rfd::FileDialog::new()
                            .add_filter("GIMP palette", &["gpl"])
                            .add_filter("All files", &["*"])
                            .set_file_name("palette.gpl");

                        if let Some(path) = dialog.save_file() {
                            fx.push(Event::SavePalette(path).into());
                        }
                    }
                });

                ui.horizontal_wrapped(|ui| {
                    ui.set_max_width(PICKER_WIDTH);
                    ui.spacing_mut().item_spacing = egui::vec2(0., 0.);
                    ui.spacing_mut().button_padding = egui::vec2(1., 1.);

                    for i in 0..self.textures.len() {
                        let tex = &mut self.textures[i];
                        let image = &mut self.egui_images[i];
                        let tex: &egui::TextureHandle = tex.get_or_insert_with(|| {
                            ui.ctx().load_texture("", image.clone(), Default::default())
                        });
                        let tooltip = format!(
                            "Select color {:?} (HSV: {}, {:.3}, {:.3}) (right click to remove from palette)",
                            self.colors[i],
                            Color::from(self.colors[i]).hue(),
                            Color::from(self.colors[i]).saturation(),
                            Color::from(self.colors[i]).value()
                        );

                        let btn = egui::ImageButton::new(tex, tex.size_vec2());
                        let btn = ui.add(btn).on_hover_text(tooltip);
                        if btn.clicked() {
                            fx.push(Event::SetMainColor(self.colors[i].into()).into());
                        }
                        if btn.clicked_by(egui::PointerButton::Secondary) {
                            fx.push(Event::RemoveFromPalette(self.colors[i].into()).into());
                        }
                    }
                });
            });

        fx
    }
}

/// Parses `#RRGGBB` or `#RRGGBBAA`, with or without the leading `#`. Colors
/// written without an alpha channel are taken to be opaque.
fn parse_hex(text: &str) -> Option<[u8; 4]> {
    let text = text.trim().trim_start_matches('#');
    let value = u32::from_str_radix(text, 16).ok()?;

    match text.len() {
        6 => Some([(value >> 16) as u8, (value >> 8) as u8, value as u8, 255]),
        8 => Some([
            (value >> 24) as u8,
            (value >> 16) as u8,
            (value >> 8) as u8,
            value as u8,
        ]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_with_and_without_alpha() {
        assert_eq!(parse_hex("#3A7FD5"), Some([0x3A, 0x7F, 0xD5, 255]));
        assert_eq!(parse_hex("3A7FD5"), Some([0x3A, 0x7F, 0xD5, 255]));
        assert_eq!(parse_hex("#3A7FD580"), Some([0x3A, 0x7F, 0xD5, 0x80]));
        assert_eq!(parse_hex(" #3a7fd5 "), Some([0x3A, 0x7F, 0xD5, 255]));
    }

    #[test]
    fn rejects_malformed_hex() {
        assert_eq!(parse_hex(""), None);
        assert_eq!(parse_hex("#ABC"), None);
        assert_eq!(parse_hex("#GGGGGG"), None);
        assert_eq!(parse_hex("#3A7FD5FF00"), None);
    }
}
