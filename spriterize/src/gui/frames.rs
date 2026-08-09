use crate::gui::layout::{self, PanelLayout};
use crate::playback::{MAX_FPS, MIN_FPS};
use crate::{Effect, UiEvent};
use lapix::Event;

/// The animation frames: a row of them, controls to add, duplicate and delete,
/// and playback.
///
/// Frames share the project's layers — a layer exists on every frame — so this
/// only chooses which frame's pixels are being edited and shown.
pub struct FramesPanel {
    frame_count: usize,
    active_frame: usize,
    is_playing: bool,
    fps: f32,
}

impl FramesPanel {
    pub fn new() -> Self {
        Self {
            frame_count: 1,
            active_frame: 0,
            is_playing: false,
            fps: 12.0,
        }
    }

    pub fn sync(&mut self, frame_count: usize, active_frame: usize, is_playing: bool, fps: f32) {
        self.frame_count = frame_count;
        self.active_frame = active_frame;
        self.is_playing = is_playing;
        self.fps = fps;
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

            // Playback. Only useful with more than one frame, and while playing
            // the whole canvas follows along.
            ui.add_enabled_ui(self.frame_count > 1, |ui| {
                ui.horizontal(|ui| {
                    let play = if self.is_playing { "⏸" } else { "▶" };

                    if ui
                        .button(play)
                        .on_hover_text("play or pause the animation (Enter)")
                        .clicked()
                    {
                        events.push(UiEvent::TogglePlayback.into());
                    }
                    if ui
                        .button("⏹")
                        .on_hover_text("stop and return to the first frame")
                        .clicked()
                    {
                        events.push(UiEvent::StopPlayback.into());
                    }

                    ui.label("fps:");

                    let mut fps = self.fps;
                    if ui
                        .add(egui::Slider::new(&mut fps, MIN_FPS..=MAX_FPS).integer())
                        .on_hover_text("playback speed")
                        .changed()
                    {
                        events.push(UiEvent::SetPlaybackFps(fps).into());
                    }
                });
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
