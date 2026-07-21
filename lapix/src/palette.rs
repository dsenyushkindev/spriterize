use crate::{util, Color, Error, Result};
use serde::{Deserialize, Serialize};

const MAX_PALETTE: usize = 200;

/// First line of a GIMP palette file.
const GPL_HEADER: &str = "GIMP Palette";
/// Layout hint written to saved palettes. Only affects how other editors
/// display the palette, not how it is read back.
const GPL_COLUMNS: usize = 8;
/// Extension recognized as a GIMP palette rather than an image.
const GPL_EXTENSION: &str = "gpl";

/// Whether a path names a GIMP palette, as opposed to an image to sample.
fn is_gpl_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(GPL_EXTENSION))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Palette(Vec<Color>);

impl Default for Palette {
    fn default() -> Self {
        Self(vec![
            Color::new(0, 0, 0, 255),       // BLACK
            Color::new(255, 255, 255, 255), // WHITE
            Color::new(255, 0, 0, 255),     // RED
            Color::new(255, 127, 0, 255),   // RED + YELLOW = ORANGE
            Color::new(255, 255, 0, 255),   // YELLOW
            Color::new(127, 255, 0, 255),   // GREEN + YELLOW
            Color::new(0, 255, 0, 255),     // GREEN
            Color::new(0, 255, 127, 255),   // GREEN + CYAN
            Color::new(0, 255, 255, 255),   // CYAN
            Color::new(0, 127, 255, 255),   // BLUE + CYAN
            Color::new(0, 0, 255, 255),     // BLUE
            Color::new(127, 0, 255, 255),   // BLUE + MAGENTA
            Color::new(255, 0, 255, 255),   // MAGENTA
            Color::new(255, 0, 127, 255),   // RED + MAGENTA
        ])
    }
}

impl Palette {
    /// Load a palette from a file.
    ///
    /// A `.gpl` file is read as a GIMP palette. Anything else is treated as an
    /// image, and its distinct colors become the palette.
    pub fn from_file(path: &str) -> Result<Self> {
        if is_gpl_path(path) {
            return Self::from_gpl(&std::fs::read_to_string(path)?);
        }

        let img = util::load_img_from_file(path)?;

        Ok(Self::from_image(img))
    }

    /// Write this palette to a file as a GIMP palette (`.gpl`).
    pub fn save_to_file(&self, path: &str) -> Result<()> {
        let name = std::path::Path::new(path)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_owned());

        std::fs::write(path, self.to_gpl(&name))?;

        Ok(())
    }

    /// Render this palette in the GIMP palette format.
    ///
    /// That format has no alpha channel, so colors are written as RGB only and
    /// come back fully opaque.
    pub fn to_gpl(&self, name: &str) -> String {
        let mut out = format!("{GPL_HEADER}\nName: {name}\nColumns: {GPL_COLUMNS}\n#\n");

        for color in &self.0 {
            out.push_str(&format!(
                "{:3} {:3} {:3}\t#{:02X}{:02X}{:02X}\n",
                color.r, color.g, color.b, color.r, color.g, color.b
            ));
        }

        out
    }

    /// Parse the GIMP palette format.
    ///
    /// Lines that aren't three numbers are metadata (`Name:`, `Columns:`),
    /// comments or blank, and are skipped. Real-world `.gpl` files vary enough
    /// that being lenient here is worth more than rejecting odd lines; a file
    /// with no colors at all is still an error.
    fn from_gpl(text: &str) -> Result<Self> {
        let mut lines = text.lines().map(str::trim);

        if !lines.next().is_some_and(|l| l.starts_with(GPL_HEADER)) {
            return Err(Error::InvalidPalette(format!(
                "expected it to start with \"{GPL_HEADER}\""
            )));
        }

        let mut colors: Vec<Color> = Vec::new();

        for line in lines {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let mut fields = line.split_whitespace();
            let mut channel = || fields.next().and_then(|f| f.parse::<u8>().ok());

            if let (Some(r), Some(g), Some(b)) = (channel(), channel(), channel()) {
                let color = Color::new(r, g, b, 255);

                if !colors.contains(&color) {
                    colors.push(color);
                }

                if colors.len() >= MAX_PALETTE {
                    break;
                }
            }
        }

        if colors.is_empty() {
            return Err(Error::InvalidPalette("it has no colors".to_owned()));
        }

        let mut palette = Self(colors);
        palette.sort();

        Ok(palette)
    }

    fn from_image(img: image::RgbaImage) -> Self {
        let mut palette = Vec::new();

        for (_, _, pixel) in img.enumerate_pixels() {
            let color = Color::new(pixel.0[0], pixel.0[1], pixel.0[2], pixel.0[3]);

            if !palette.contains(&color) {
                palette.push(color);
            }

            if palette.len() >= MAX_PALETTE {
                break;
            }
        }
        let mut palette = Self(palette);
        palette.sort();

        palette
    }

    pub fn add_color(&mut self, color: Color) {
        if !self.0.contains(&color) {
            self.0.push(color)
        }
        self.sort();
    }

    pub fn remove_color(&mut self, color: Color) {
        self.0.retain(|c| *c != color);
    }

    pub fn colors(&self) -> &[Color] {
        &self.0
    }

    pub fn sort(&mut self) {
        fn sort_val(color: &Color) -> i32 {
            (color.hue() as i32) * 1_000_000
                + (color.saturation() * 10_000.) as i32
                + (color.value() * 10_000.) as i32
        }
        self.0.sort_by(|a, b| sort_val(a).cmp(&sort_val(b)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from_bytes(bytes: Vec<u8>) -> Palette {
        let len = bytes.len() as u32 / 4;
        let img = image::RgbaImage::from_raw(1, len, bytes).unwrap();
        Palette::from_image(img)
    }

    #[test]
    fn create_from_img() {
        let bytes = vec![0, 0, 0, 255];
        let palette = from_bytes(bytes);
        assert!(palette.colors().contains(&Color::new(0, 0, 0, 255)));
        assert_eq!(palette.colors().len(), 1);

        let bytes = vec![0, 0, 0, 255, 0, 0, 0, 255];
        let palette = from_bytes(bytes);
        assert!(palette.colors().contains(&Color::new(0, 0, 0, 255)));
        assert_eq!(palette.colors().len(), 1);

        let bytes = vec![0, 0, 0, 255, 255, 0, 0, 255];
        let palette = from_bytes(bytes);
        assert!(palette.colors().contains(&Color::new(0, 0, 0, 255)));
        assert!(palette.colors().contains(&Color::new(255, 0, 0, 255)));
        assert_eq!(palette.colors().len(), 2);
    }

    #[test]
    fn add_and_remove_from_default() {
        let mut palette = Palette::default();

        let dark = Color::new(10, 10, 10, 255);
        palette.add_color(dark);
        assert!(palette.colors().contains(&dark));

        palette.remove_color(dark);
        assert!(!palette.colors().contains(&dark));
    }

    #[test]
    fn add_one() {
        let bytes = vec![0, 0, 0, 255];
        let mut palette = from_bytes(bytes);

        let color = Color::new(0, 1, 2, 3);
        palette.add_color(color);
        assert!(palette.colors().contains(&color));
        assert_eq!(palette.colors().len(), 2);
    }

    #[test]
    fn gpl_round_trip() {
        let palette = Palette::default();
        let parsed = Palette::from_gpl(&palette.to_gpl("test")).unwrap();

        assert_eq!(parsed.colors(), palette.colors());
    }

    #[test]
    fn gpl_skips_metadata_and_comments() {
        let text = "GIMP Palette\n\
                    Name: Example\n\
                    Columns: 4\n\
                    #\n\
                    # a comment\n\
                    \n\
                      0   0   0\tBlack\n\
                    255 255 255\tWhite\n";
        let palette = Palette::from_gpl(text).unwrap();

        assert_eq!(palette.colors().len(), 2);
        assert!(palette.colors().contains(&Color::new(0, 0, 0, 255)));
        assert!(palette.colors().contains(&Color::new(255, 255, 255, 255)));
    }

    #[test]
    fn gpl_colors_are_opaque_and_deduplicated() {
        let text = "GIMP Palette\n1 2 3\n1 2 3\n";
        let palette = Palette::from_gpl(text).unwrap();

        assert_eq!(palette.colors(), &[Color::new(1, 2, 3, 255)]);
    }

    #[test]
    fn gpl_rejects_files_without_a_header() {
        assert!(Palette::from_gpl("0 0 0\n").is_err());
    }

    #[test]
    fn gpl_rejects_files_without_colors() {
        assert!(Palette::from_gpl("GIMP Palette\nName: Empty\n#\n").is_err());
    }

    #[test]
    fn remove_one() {
        let bytes = vec![0, 0, 0, 255, 1, 1, 1, 255];
        let mut palette = from_bytes(bytes);

        let black = Color::new(0, 0, 0, 255);
        palette.remove_color(black);
        assert!(!palette.colors().contains(&black));
        assert_eq!(palette.colors().len(), 1);
    }
}
