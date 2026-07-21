use crate::settings::{Settings, MAX_UI_SCALE, MIN_UI_SCALE};
use crate::{Effect, UiEvent};

/// Scales offered as one-click presets, alongside the slider.
const SCALE_PRESETS: [f32; 5] = [0.75, 1.0, 1.25, 1.5, 2.0];

pub struct SettingsWindow {
    open: bool,
    settings: Settings,
    /// Scale the display itself reports, shown so the user can tell what the
    /// multiplier is being applied to.
    dpi_scale: f32,
}

impl SettingsWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            settings: Settings::default(),
            dpi_scale: 1.0,
        }
    }

    pub fn sync(&mut self, settings: Settings, dpi_scale: f32) {
        self.settings = settings;
        self.dpi_scale = dpi_scale;
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

        egui::Window::new("Settings")
            .open(&mut open)
            .resizable(false)
            .show(egui_ctx, |ui| {
                ui.heading("Interface");

                let mut scale = self.settings.ui_scale;

                ui.horizontal(|ui| {
                    ui.label("Scale:");
                    ui.add(
                        egui::Slider::new(&mut scale, MIN_UI_SCALE..=MAX_UI_SCALE)
                            .fixed_decimals(2)
                            .suffix("x"),
                    );
                });

                ui.horizontal(|ui| {
                    for preset in SCALE_PRESETS {
                        let selected = (self.settings.ui_scale - preset).abs() < f32::EPSILON;

                        if ui
                            .selectable_label(selected, format!("{:.0}%", preset * 100.))
                            .clicked()
                        {
                            scale = preset;
                        }
                    }
                });

                if scale != self.settings.ui_scale {
                    events.push(Effect::UiEvent(UiEvent::SetUiScale(scale)));
                }

                ui.label(
                    egui::RichText::new(format!(
                        "This display reports {:.0}%, so the interface renders at {:.0}%.",
                        self.dpi_scale * 100.,
                        self.dpi_scale * self.settings.ui_scale * 100.
                    ))
                    .weak()
                    .small(),
                )
                .on_hover_text(
                    "Scale multiplies the display's own scaling, so 100% is about the same \
                     physical size on any screen.",
                );

                ui.separator();
                ui.heading("Canvas");

                let mut show_grid = self.settings.show_grid;

                if ui
                    .checkbox(&mut show_grid, "Show pixel grid")
                    .on_hover_text("Ctrl+G. Hidden automatically when zoomed too far out.")
                    .changed()
                {
                    events.push(Effect::UiEvent(UiEvent::ToggleGrid));
                }
            });

        self.open = open;

        events
    }
}
