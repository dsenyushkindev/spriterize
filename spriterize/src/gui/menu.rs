use crate::{Effect, UiEvent};
use lapix::{Event, Size, Transform};
use std::path::PathBuf;

pub struct MenuBar {
    recent_files: Vec<PathBuf>,
    show_resize_window: bool,
    show_spritesheet_window: bool,
    show_confirm_exit_window: bool,
    show_confirm_new_window: bool,
    canvas_size: Size<i32>,
    spritesheet: Size<u8>,
    canvas_size_str: Option<(String, String)>,
    spritesheet_str: Option<(String, String)>,
    can_undo: bool,
    can_redo: bool,
}

impl MenuBar {
    pub fn new() -> Self {
        Self {
            recent_files: Vec::new(),
            show_resize_window: false,
            show_spritesheet_window: false,
            show_confirm_exit_window: false,
            show_confirm_new_window: false,
            canvas_size: Size::ZERO,
            spritesheet: (1, 1).into(),
            canvas_size_str: None,
            spritesheet_str: None,
            can_undo: false,
            can_redo: false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sync(
        &mut self,
        canvas_size: Size<i32>,
        spritesheet: Size<u8>,
        can_undo: bool,
        can_redo: bool,
        recent_files: Vec<PathBuf>,
        new_project_requested: bool,
    ) {
        self.canvas_size = canvas_size;
        self.spritesheet = spritesheet;
        self.can_undo = can_undo;
        self.can_redo = can_redo;
        self.recent_files = recent_files;

        // The New Project shortcut goes through the same confirmation as the
        // menu entry, rather than discarding the project outright.
        if new_project_requested {
            self.show_confirm_new_window = true;
        }
    }

    pub fn update(&mut self, egui_ctx: &egui::Context) -> Vec<Effect> {
        let mut events = self.update_menu(egui_ctx);
        events.append(&mut self.update_resize_window(egui_ctx));
        events.append(&mut self.update_spritesheet_window(egui_ctx));
        events.append(&mut self.update_confirm_exit_window(egui_ctx));
        events.append(&mut self.update_confirm_new_window(egui_ctx));
        events
    }

    fn update_menu(&mut self, egui_ctx: &egui::Context) -> Vec<Effect> {
        let mut events = Vec::new();

        egui::TopBottomPanel::top("menu_bar").show(egui_ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    // The dialogs themselves live in `crate::files`, so that the
                    // keyboard shortcuts can open the same ones.
                    for (label, shortcut, event) in [
                        ("New", "Ctrl+N", UiEvent::RequestNewProject),
                        ("Open Project", "Ctrl+O", UiEvent::OpenProject),
                        ("Save Project", "Ctrl+S", UiEvent::SaveProject),
                        ("Save Project As", "Ctrl+Shift+S", UiEvent::SaveProjectAs),
                        ("Export Image", "Ctrl+E", UiEvent::ExportImage),
                        ("Import Image", "Ctrl+I", UiEvent::ImportImage),
                    ] {
                        if ui
                            .add(egui::Button::new(label).shortcut_text(shortcut))
                            .clicked()
                        {
                            events.push(Effect::UiEvent(event));
                            ui.close_menu();
                        }
                    }

                    ui.menu_button("Open Recent", |ui| {
                        if self.recent_files.is_empty() {
                            ui.add_enabled(false, egui::Button::new("(nothing yet)"));
                            return;
                        }

                        for path in &self.recent_files {
                            let label = path
                                .file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_else(|| path.to_string_lossy().into_owned());

                            if ui
                                .button(label)
                                .on_hover_text(path.to_string_lossy())
                                .clicked()
                            {
                                events.push(Effect::UiEvent(UiEvent::OpenRecent(path.clone())));
                                ui.close_menu();
                            }
                        }

                        ui.separator();

                        if ui.button("Clear").clicked() {
                            events.push(Effect::UiEvent(UiEvent::ClearRecent));
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    if ui.button("Exit").clicked() {
                        self.show_confirm_exit_window = true;
                        ui.close_menu();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui
                        .add_enabled(
                            self.can_undo,
                            egui::Button::new("Undo").shortcut_text("Ctrl+Z"),
                        )
                        .clicked()
                    {
                        events.push(Event::Undo.into());
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            self.can_redo,
                            egui::Button::new("Redo").shortcut_text("Ctrl+Y"),
                        )
                        .clicked()
                    {
                        events.push(Event::Redo.into());
                        ui.close_menu();
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui.button("Zoom in").clicked() {
                        events.push(Effect::UiEvent(UiEvent::ZoomIn));
                        ui.close_menu();
                    }
                    if ui.button("Zoom out").clicked() {
                        events.push(Effect::UiEvent(UiEvent::ZoomOut));
                        ui.close_menu();
                    }
                    if ui.button("Reset zoom to default").clicked() {
                        events.push(Effect::UiEvent(UiEvent::ResetZoom));
                        ui.close_menu();
                    }
                    if ui.button("Set zoom to 100%").clicked() {
                        events.push(Effect::UiEvent(UiEvent::SetZoom100));
                        ui.close_menu();
                    }
                    if ui.button("Toggle pixel grid").clicked() {
                        events.push(Effect::UiEvent(UiEvent::ToggleGrid));
                        ui.close_menu();
                    }
                });
                ui.menu_button("Canvas", |ui| {
                    /*
                    ui.menu_button("Category", |ui| {
                        if ui.button("Item 1").clicked() {
                        }
                        if ui.button("Item 2").clicked() {
                        }
                    });*/
                    if ui.button("Resize Canvas").clicked() {
                        ui.close_menu();
                        self.show_resize_window = true;
                    }
                    if ui.button("Change Spritesheet").clicked() {
                        ui.close_menu();
                        self.show_spritesheet_window = true;
                    }
                    if ui.button("Erase Canvas").clicked() {
                        ui.close_menu();
                        events.push(Event::ClearCanvas.into());
                    }
                });
                ui.menu_button("Transform", |ui| {
                    if ui.button("Silhouete").clicked() {
                        ui.close_menu();
                        events.push(Event::ApplyTransform(Transform::Silhouete).into());
                    }
                    if ui.button("Apply palette").clicked() {
                        ui.close_menu();
                        events.push(Event::ApplyTransform(Transform::ApplyPalette).into());
                    }
                });
            });
        });

        events
    }

    fn update_resize_window(&mut self, egui_ctx: &egui::Context) -> Vec<Effect> {
        let mut events = Vec::new();

        if self.show_resize_window {
            if self.canvas_size_str.is_none() {
                self.canvas_size_str = Some((
                    self.canvas_size.x.to_string(),
                    self.canvas_size.y.to_string(),
                ));
            }

            egui::Window::new("Resize Canvas")
                .default_pos((200., 30.))
                .show(egui_ctx, |ui| {
                    ui.horizontal(|ui| {
                        let label = ui.label("w:");
                        ui.add(
                            egui::widgets::TextEdit::singleline(
                                &mut self.canvas_size_str.as_mut().unwrap().0,
                            )
                            .desired_width(30.0),
                        )
                        .labelled_by(label.id);
                        let label = ui.label("h:");
                        ui.add(
                            egui::widgets::TextEdit::singleline(
                                &mut self.canvas_size_str.as_mut().unwrap().1,
                            )
                            .desired_width(30.0),
                        )
                        .labelled_by(label.id);
                    });

                    ui.horizontal(|ui| {
                        if ui.button("resize").clicked() {
                            if let (Ok(w), Ok(h)) = (
                                self.canvas_size_str.as_ref().unwrap().0.parse(),
                                self.canvas_size_str.as_ref().unwrap().1.parse(),
                            ) {
                                events.push(Event::ResizeCanvas((w, h).into()).into());
                            }
                            self.canvas_size_str = None;
                            self.show_resize_window = false;
                        }
                        if ui.button("cancel").clicked() {
                            self.canvas_size_str = None;
                            self.show_resize_window = false;
                        }
                    });
                });
        }

        events
    }

    fn update_spritesheet_window(&mut self, egui_ctx: &egui::Context) -> Vec<Effect> {
        let mut events = Vec::new();

        if self.show_spritesheet_window {
            if self.spritesheet_str.is_none() {
                self.spritesheet_str = Some((
                    self.spritesheet.x.to_string(),
                    self.spritesheet.y.to_string(),
                ));
            }

            egui::Window::new("Spritesheet")
                .default_pos((200., 30.))
                .show(egui_ctx, |ui| {
                    ui.horizontal(|ui| {
                        let label = ui.label("cols:");
                        ui.add(
                            egui::widgets::TextEdit::singleline(
                                &mut self.spritesheet_str.as_mut().unwrap().0,
                            )
                            .desired_width(30.0),
                        )
                        .labelled_by(label.id);
                        let label = ui.label("rows:");
                        ui.add(
                            egui::widgets::TextEdit::singleline(
                                &mut self.spritesheet_str.as_mut().unwrap().1,
                            )
                            .desired_width(30.0),
                        )
                        .labelled_by(label.id);
                        if ui.button("Ok").clicked() {
                            if let (Ok(w), Ok(h)) = (
                                self.spritesheet_str.as_ref().unwrap().0.parse(),
                                self.spritesheet_str.as_ref().unwrap().1.parse(),
                            ) {
                                events.push(Event::SetSpritesheet((w, h).into()).into());
                            }
                            self.spritesheet_str = None;
                            self.show_spritesheet_window = false;
                        }
                        if ui.button("cancel").clicked() {
                            self.spritesheet_str = None;
                            self.show_spritesheet_window = false;
                        }
                    });
                });
        }

        events
    }

    fn update_confirm_exit_window(&mut self, egui_ctx: &egui::Context) -> Vec<Effect> {
        let mut events = Vec::new();

        if !self.show_confirm_exit_window {
            return events;
        }

        egui::Window::new("Exit")
            .default_pos((200., 30.))
            .show(egui_ctx, |ui| {
                ui.label("Are you sure you want to exit?");
                ui.horizontal(|ui| {
                    if ui.button("Ok").clicked() {
                        events.push(UiEvent::Exit.into());
                    }
                    if ui.button("cancel").clicked() {
                        self.show_confirm_exit_window = false;
                    }
                });
            });

        events
    }

    fn update_confirm_new_window(&mut self, egui_ctx: &egui::Context) -> Vec<Effect> {
        let mut events = Vec::new();

        if !self.show_confirm_new_window {
            return events;
        }

        egui::Window::new("New Project")
            .default_pos((200., 30.))
            .show(egui_ctx, |ui| {
                ui.label(
                    "Are you sure you want to start a new project? \
                    All your unsaved changes will be lost",
                );
                ui.horizontal(|ui| {
                    if ui.button("Ok").clicked() {
                        events.push(UiEvent::NewProject.into());
                    }
                    if ui.button("cancel").clicked() {
                        self.show_confirm_new_window = false;
                    }
                });
            });

        events
    }
}
