use crate::{Effect, UiEvent};
use std::path::PathBuf;

pub struct Launcher {
    recent: Vec<PathBuf>,
}

impl Launcher {
    pub fn new() -> Self {
        Self { recent: Vec::new() }
    }

    pub fn sync(&mut self, recent: Vec<PathBuf>) {
        self.recent = recent;
    }

    pub fn update(&mut self, ctx: &egui::Context) -> Vec<Effect> {
        let mut effects = Vec::new();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space((ui.available_height() * 0.10).min(80.0));
            ui.vertical_centered(|ui| {
                ui.heading("Spriterize");
                ui.label("Pixel art and procedural asset collections");
                ui.add_space(24.0);

                let width = ui.available_width().min(720.0);
                ui.allocate_ui(egui::vec2(width, 190.0), |ui| {
                    ui.columns(2, |columns| {
                        columns[0].heading("Create");
                        columns[0].add_space(8.0);
                        if columns[0]
                            .add_sized(
                                [columns[0].available_width(), 42.0],
                                egui::Button::new("New Asset Collection…"),
                            )
                            .clicked()
                        {
                            effects.push(UiEvent::CreateCollection.into());
                        }
                        columns[0].label(
                            "A reusable collection of editable asset projects, generators, and outputs.",
                        );
                        columns[0].add_space(12.0);
                        if columns[0]
                            .add_sized(
                                [columns[0].available_width(), 36.0],
                                egui::Button::new("New Image Project"),
                            )
                            .clicked()
                        {
                            effects.push(UiEvent::NewProject.into());
                        }

                        columns[1].heading("Open");
                        columns[1].add_space(8.0);
                        if columns[1]
                            .add_sized(
                                [columns[1].available_width(), 42.0],
                                egui::Button::new("Open Collection, Project, or Image…"),
                            )
                            .clicked()
                        {
                            effects.push(UiEvent::OpenDocument.into());
                        }
                        columns[1].label(
                            "Open a collection catalog, an editable project, or an existing image.",
                        );
                    });
                });

                if !self.recent.is_empty() {
                    ui.add_space(24.0);
                    ui.separator();
                    ui.add_space(12.0);
                    ui.heading("Recent");
                    ui.add_space(6.0);

                    for path in &self.recent {
                        let name = path
                            .file_name()
                            .map(|name| name.to_string_lossy())
                            .unwrap_or_else(|| path.to_string_lossy());
                        if ui
                            .add_sized([width, 30.0], egui::Button::new(name.as_ref()))
                            .on_hover_text(path.display().to_string())
                            .clicked()
                        {
                            effects.push(UiEvent::OpenRecent(path.clone()).into());
                        }
                    }
                }
            });
        });

        effects
    }
}
