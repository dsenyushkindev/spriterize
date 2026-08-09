use crate::gui::layout::{self, PanelLayout};
use crate::gui::picture::Picture;
use lapix::Size;
use std::time::{SystemTime, UNIX_EPOCH};

const MS_PER_FRAME: u128 = 100;

/// A live preview of the drawing, at a chosen scale.
///
/// When the canvas is divided into a spritesheet, the preview plays through its
/// cells, so it doubles as a look at the animation held within a single frame.
/// It shows the composited active frame, so filters, opacity and adjustment
/// layers are all reflected.
pub struct Preview {
    spritesheet: Size<u8>,
    canvas_size: Size<i32>,
    picture: Picture,
    scale: String,
}

impl Preview {
    pub fn new() -> Self {
        Self {
            spritesheet: (1, 1).into(),
            canvas_size: (0, 0).into(),
            picture: Picture::new(),
            scale: "1".to_owned(),
        }
    }

    pub fn sync(&mut self, spritesheet: Size<u8>, canvas_size: Size<i32>) {
        self.spritesheet = spritesheet;
        self.canvas_size = canvas_size;
    }

    /// Replace the previewed pixels. Called only when the composited image
    /// changes, not every frame.
    pub fn set_image(&mut self, width: usize, height: usize, rgba: &[u8]) {
        self.picture.set(width, height, rgba);
    }

    pub fn update(&mut self, egui_ctx: &egui::Context, layout: &PanelLayout) {
        layout.show(egui_ctx, layout::PREVIEW, |ui| {
            ui.horizontal(|ui| {
                let label = ui.label("scale:");
                ui.add(egui::widgets::TextEdit::singleline(&mut self.scale).desired_width(30.0))
                    .labelled_by(label.id);
            });

            let (nx, ny) = (self.spritesheet.x.max(1), self.spritesheet.y.max(1));
            let cell = self.current_cell(nx, ny);
            let cell_size = egui::vec2(
                self.canvas_size.x as f32 / nx as f32,
                self.canvas_size.y as f32 / ny as f32,
            );
            let scale = self.scale.parse().unwrap_or(1.0_f32).max(0.01);

            egui::ScrollArea::both().show(ui, |ui| {
                self.picture.image(ui, cell_size * scale, cell);
            });
        });
    }

    /// The texture sub-rectangle for the spritesheet cell showing now, cycled
    /// over time so the preview animates.
    fn current_cell(&self, nx: u8, ny: u8) -> egui::Rect {
        let frames = nx as u128 * ny as u128;
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let frame = (elapsed / MS_PER_FRAME % frames) as u8;
        let (col, row) = (frame % nx, frame / nx);
        let (w, h) = (1.0 / nx as f32, 1.0 / ny as f32);

        egui::Rect::from_min_size(egui::pos2(col as f32 * w, row as f32 * h), egui::vec2(w, h))
    }
}
