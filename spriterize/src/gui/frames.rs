use crate::gui::layout::{self, PanelLayout};
use crate::Effect;
use lapix::Event;

/// The animation frames: a row of them, plus controls to add, duplicate and
/// delete.
///
/// Frames share the project's layers — a layer exists on every frame — so this
/// only chooses which frame's pixels are being edited and shown.
pub struct FramesPanel {
    frame_count: usize,
    active_frame: usize,
}

impl FramesPanel {
    pub fn new() -> Self {
        Self {
            frame_count: 1,
            active_frame: 0,
        }
    }

    pub fn sync(&mut self, frame_count: usize, active_frame: usize) {
        self.frame_count = frame_count;
        self.active_frame = active_frame;
    }

    pub fn update(&mut self, egui_ctx: &egui::Context, layout: &PanelLayout) -> Vec<Effect> {
        let mut events = Vec::new();

        layout.show(egui_ctx, layout::FRAMES, |ui| {
            ui.horizontal(|ui| {
                if ui.button("+").on_hover_text("add a blank frame").clicked() {
                    events.push(Event::AddFrame.into());
                }
                if ui
                    .button("dup")
                    .on_hover_text("duplicate the current frame")
                    .clicked()
                {
                    events.push(Event::DuplicateFrame(self.active_frame).into());
                }
                if ui
                    .add_enabled(self.frame_count > 1, egui::Button::new("x"))
                    .on_hover_text("delete the current frame")
                    .clicked()
                {
                    events.push(Event::DeleteFrame(self.active_frame).into());
                }
            });

            ui.separator();

            // A numbered button per frame, the current one selected. Wraps so a
            // long animation doesn't push the panel wide.
            ui.horizontal_wrapped(|ui| {
                for frame in 0..self.frame_count {
                    let selected = frame == self.active_frame;

                    if ui
                        .selectable_label(selected, format!("{}", frame + 1))
                        .clicked()
                    {
                        events.push(Event::SwitchFrame(frame).into());
                    }
                }
            });
        });

        events
    }
}
