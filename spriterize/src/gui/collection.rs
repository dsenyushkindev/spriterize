use crate::gui::CollectionSync;
use crate::{Effect, UiEvent};

pub struct CollectionWindow {
    open: bool,
    name: String,
    assets: Vec<(String, bool)>,
    projects: Vec<(String, String, bool)>,
    active_project: Option<String>,
    elements: Vec<(String, String)>,
    adding_project: bool,
    adding_element: bool,
    new_name: String,
    new_width: String,
    new_height: String,
    new_element_name: String,
}

impl CollectionWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            name: String::new(),
            assets: Vec::new(),
            projects: Vec::new(),
            active_project: None,
            elements: Vec::new(),
            adding_project: false,
            adding_element: false,
            new_name: "New Asset".into(),
            new_width: "64".into(),
            new_height: "64".into(),
            new_element_name: "New Element".into(),
        }
    }

    pub fn sync(&mut self, collection: Option<CollectionSync>) {
        let Some(collection) = collection else {
            self.open = false;
            self.name.clear();
            self.assets.clear();
            self.projects.clear();
            self.active_project = None;
            self.elements.clear();
            return;
        };
        self.name = collection.name;
        self.active_project = collection.active_project;
        self.elements = collection
            .elements
            .iter()
            .map(|element| (element.id.clone(), element.name.clone()))
            .collect();
        sync_checks(&mut self.assets, collection.assets);

        let old = std::mem::take(&mut self.projects);
        self.projects = collection
            .projects
            .into_iter()
            .map(|(id, name)| {
                let selected = old
                    .iter()
                    .find(|(old_id, _, _)| old_id == &id)
                    .map(|(_, _, selected)| *selected)
                    .unwrap_or(true);
                (id, name, selected)
            })
            .collect();
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn update(&mut self, ctx: &egui::Context) -> Vec<Effect> {
        let mut effects = Vec::new();
        if !self.open {
            return effects;
        }
        egui::Window::new(format!("Asset Collection — {}", self.name))
            .open(&mut self.open)
            .default_size(egui::vec2(460.0, 600.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("+ New Project").clicked() {
                        self.adding_project = true;
                    }
                    if ui.button("+ New Element").clicked() {
                        self.adding_element = true;
                    }
                    if ui.button("Select all").clicked() {
                        set_all(&mut self.assets, true);
                        for (_, _, selected) in &mut self.projects {
                            *selected = true;
                        }
                    }
                    if ui.button("Select none").clicked() {
                        set_all(&mut self.assets, false);
                        for (_, _, selected) in &mut self.projects {
                            *selected = false;
                        }
                    }
                });

                if self.adding_project {
                    ui.group(|ui| {
                        ui.label("New editable project");
                        ui.horizontal(|ui| {
                            ui.label("Name");
                            ui.text_edit_singleline(&mut self.new_name);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Size");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.new_width).desired_width(55.0),
                            );
                            ui.label("×");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.new_height)
                                    .desired_width(55.0),
                            );
                        });
                        let dimensions = self
                            .new_width
                            .parse::<u16>()
                            .ok()
                            .zip(self.new_height.parse::<u16>().ok())
                            .filter(|(width, height)| *width > 0 && *height > 0);
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    !self.new_name.trim().is_empty() && dimensions.is_some(),
                                    egui::Button::new("Create and edit"),
                                )
                                .clicked()
                            {
                                let (width, height) = dimensions.unwrap();
                                effects.push(
                                    UiEvent::CreateCollectionProject {
                                        name: self.new_name.trim().to_owned(),
                                        width,
                                        height,
                                    }
                                    .into(),
                                );
                                self.adding_project = false;
                            }
                            if ui.button("Cancel").clicked() {
                                self.adding_project = false;
                            }
                        });
                    });
                }

                if self.adding_element {
                    ui.group(|ui| {
                        ui.label("New reusable graph element");
                        ui.horizontal(|ui| {
                            ui.label("Name");
                            ui.text_edit_singleline(&mut self.new_element_name);
                        });
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    !self.new_element_name.trim().is_empty(),
                                    egui::Button::new("Create and edit"),
                                )
                                .clicked()
                            {
                                effects.push(
                                    UiEvent::CreateCollectionElement(
                                        self.new_element_name.trim().to_owned(),
                                    )
                                    .into(),
                                );
                                self.adding_element = false;
                            }
                            if ui.button("Cancel").clicked() {
                                self.adding_element = false;
                            }
                        });
                    });
                }

                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(420.0)
                    .show(ui, |ui| {
                        ui.heading("Editable projects");
                        if self.projects.is_empty() {
                            ui.weak("No projects yet. Create one to start authoring an asset.");
                        }
                        for (id, name, selected) in &mut self.projects {
                            ui.horizontal(|ui| {
                                ui.checkbox(selected, "");
                                let active = self.active_project.as_deref() == Some(id.as_str());
                                if ui.selectable_label(active, name.as_str()).clicked() && !active {
                                    effects
                                        .push(UiEvent::SwitchCollectionProject(id.clone()).into());
                                }
                                if active {
                                    ui.weak("editing");
                                }
                            });
                        }

                        ui.add_space(12.0);
                        ui.heading("Reusable elements");
                        if self.elements.is_empty() {
                            ui.weak("No shared elements yet.");
                        }
                        for (id, name) in &self.elements {
                            if ui.button(name).clicked() {
                                effects.push(UiEvent::EditCollectionElement(id.clone()).into());
                            }
                        }

                        if !self.assets.is_empty() {
                            ui.add_space(12.0);
                            ui.heading("Generated outputs");
                            for (id, selected) in &mut self.assets {
                                ui.checkbox(selected, id.as_str());
                            }
                        }
                    });
                ui.separator();
                let selected = selected_ids(&self.assets, &self.projects);
                let total = self.assets.len() + self.projects.len();
                ui.horizontal(|ui| {
                    ui.label(format!("{}/{} selected", selected.len(), total));
                    if ui
                        .add_enabled(!selected.is_empty(), egui::Button::new("Export selected…"))
                        .clicked()
                    {
                        effects.push(UiEvent::ExportCollectionSelected(selected).into());
                    }
                    if ui
                        .add_enabled(total > 0, egui::Button::new("Export all…"))
                        .clicked()
                    {
                        effects.push(UiEvent::ExportCollection.into());
                    }
                });
            });
        effects
    }
}

fn sync_checks(values: &mut Vec<(String, bool)>, ids: Vec<String>) {
    let old = std::mem::take(values);
    *values = ids
        .into_iter()
        .map(|id| {
            let selected = old
                .iter()
                .find(|(old_id, _)| old_id == &id)
                .map(|(_, selected)| *selected)
                .unwrap_or(true);
            (id, selected)
        })
        .collect();
}

fn set_all(values: &mut [(String, bool)], selected: bool) {
    for (_, value) in values {
        *value = selected;
    }
}

fn selected_ids(assets: &[(String, bool)], projects: &[(String, String, bool)]) -> Vec<String> {
    assets
        .iter()
        .filter(|(_, selected)| *selected)
        .map(|(id, _)| id.clone())
        .chain(
            projects
                .iter()
                .filter(|(_, _, selected)| *selected)
                .map(|(id, _, _)| id.clone()),
        )
        .collect()
}
