//! A little picture: some pixels shown as a scaled, nearest-neighbour egui
//! image.
//!
//! Both the frame thumbnails and (in principle) the preview want the same
//! thing — turn an image into something drawable in a panel — so it lives here
//! once. The egui texture is built lazily and cached until the pixels change.

/// Pixels ready to be shown, with the texture cached until they are replaced.
pub struct Picture {
    image: egui::ColorImage,
    texture: Option<egui::TextureHandle>,
}

impl Picture {
    pub fn new() -> Self {
        Self {
            // A transparent pixel, until real content arrives.
            image: egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 0]),
            texture: None,
        }
    }

    /// Replace the pixels, dropping the cached texture so it is rebuilt on the
    /// next show.
    pub fn set(&mut self, width: usize, height: usize, rgba: &[u8]) {
        self.image = egui::ColorImage::from_rgba_unmultiplied([width, height], rgba);
        self.texture = None;
    }

    fn texture(&mut self, ctx: &egui::Context) -> egui::TextureHandle {
        self.texture
            .get_or_insert_with(|| {
                ctx.load_texture("picture", self.image.clone(), egui::TextureOptions::NEAREST)
            })
            .clone()
    }

    /// Show a sub-rectangle of the picture at the given on-screen size, over
    /// the theme's preview backdrop so transparent pixels read as empty.
    ///
    /// `uv` is in 0..1 texture coordinates: the whole picture is
    /// `Rect::from_min_max((0,0), (1,1))`, and a spritesheet cell is a slice of
    /// that.
    pub fn image(&mut self, ui: &mut egui::Ui, size: egui::Vec2, uv: egui::Rect) -> egui::Response {
        let texture = self.texture(ui.ctx());

        ui.add(
            egui::Image::new(egui::load::SizedTexture::new(texture.id(), size))
                .uv(uv)
                .bg_fill(crate::theme::PREVIEW_BG),
        )
    }

    /// Show as a clickable button, framed when `selected`.
    pub fn button(&mut self, ui: &mut egui::Ui, size: egui::Vec2, selected: bool) -> egui::Response {
        let texture = self.texture(ui.ctx());

        ui.add(
            egui::ImageButton::new(egui::load::SizedTexture::new(texture.id(), size))
                .selected(selected),
        )
    }
}
