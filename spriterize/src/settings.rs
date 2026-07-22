//! User settings, persisted between runs.

use crate::files;

const SETTINGS_FILE_NAME: &str = "settings.txt";

/// Smallest and largest UI scale the settings window offers. Below the minimum
/// the menu bar stops being clickable, which would be hard to recover from.
pub const MIN_UI_SCALE: f32 = 0.5;
pub const MAX_UI_SCALE: f32 = 3.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Multiplier on top of the display's own scaling.
    ///
    /// The effective size of the interface is this times the monitor's DPI
    /// scale, so 1.0 means "whatever this display calls 100%" and the UI comes
    /// out about the same physical size on any screen.
    pub ui_scale: f32,
    /// Draw a grid between canvas pixels.
    pub show_grid: bool,
    /// Window size in physical pixels, as last left by the user. `None` on a
    /// first run, when the size is derived from the display's scaling instead.
    pub window_size: Option<(u32, u32)>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ui_scale: 1.0,
            show_grid: true,
            window_size: None,
        }
    }
}

impl Settings {
    /// Reads the settings file, falling back to defaults for anything missing
    /// or malformed. A broken settings file should never stop the editor from
    /// starting.
    pub fn load() -> Self {
        let Some(path) = files::config_path(SETTINGS_FILE_NAME) else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };

        Self::parse(&text)
    }

    fn parse(text: &str) -> Self {
        let mut settings = Self::default();

        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };

            match key.trim() {
                "ui_scale" => {
                    if let Ok(scale) = value.trim().parse::<f32>() {
                        settings.ui_scale = scale.clamp(MIN_UI_SCALE, MAX_UI_SCALE);
                    }
                }
                "show_grid" => {
                    if let Ok(show) = value.trim().parse::<bool>() {
                        settings.show_grid = show;
                    }
                }
                "window_size" => {
                    if let Some((w, h)) = value.trim().split_once('x') {
                        if let (Ok(w), Ok(h)) = (w.trim().parse(), h.trim().parse()) {
                            settings.window_size = Some((w, h));
                        }
                    }
                }
                _ => (),
            }
        }

        settings
    }

    pub fn save(&self) {
        let Some(path) = files::config_path(SETTINGS_FILE_NAME) else {
            return;
        };

        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }

        let mut text = format!("ui_scale={}\nshow_grid={}\n", self.ui_scale, self.show_grid);

        if let Some((w, h)) = self.window_size {
            text.push_str(&format!("window_size={w}x{h}\n"));
        }

        let _ = std::fs::write(path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_known_keys() {
        let settings = Settings::parse("ui_scale=1.25\nshow_grid=false\n");

        assert_eq!(settings.ui_scale, 1.25);
        assert!(!settings.show_grid);
    }

    #[test]
    fn survives_junk_unknown_keys_and_missing_values() {
        let settings =
            Settings::parse("nonsense\nui_scale=not-a-number\nfuture_option=7\nshow_grid=true\n");

        assert_eq!(settings.ui_scale, Settings::default().ui_scale);
        assert!(settings.show_grid);
    }

    #[test]
    fn clamps_absurd_scales() {
        assert_eq!(Settings::parse("ui_scale=99").ui_scale, MAX_UI_SCALE);
        assert_eq!(Settings::parse("ui_scale=0.01").ui_scale, MIN_UI_SCALE);
    }

    #[test]
    fn reads_window_size() {
        assert_eq!(
            Settings::parse("window_size=1600x1000").window_size,
            Some((1600, 1000))
        );
        assert_eq!(Settings::parse("window_size=garbage").window_size, None);
        assert_eq!(Settings::parse("window_size=1600x").window_size, None);
    }

    #[test]
    fn round_trips_through_the_saved_format() {
        let settings = Settings {
            ui_scale: 1.5,
            show_grid: false,
            window_size: Some((1600, 1000)),
        };
        let mut text = format!(
            "ui_scale={}\nshow_grid={}\n",
            settings.ui_scale, settings.show_grid
        );
        let (w, h) = settings.window_size.unwrap();
        text.push_str(&format!("window_size={w}x{h}\n"));

        assert_eq!(Settings::parse(&text), settings);
    }
}
