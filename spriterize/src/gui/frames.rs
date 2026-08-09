use crate::gui::layout::{self, PanelLayout};
use crate::gui::picture::Picture;
use crate::playback::{MAX_FPS, MIN_FPS};
use crate::{Effect, UiEvent};
use lapix::Event;

/// One frame's composited pixels: width, height and RGBA bytes.
pub type FrameImage = (usize, usize, Vec<u8>);

/// Longest side a frame thumbnail is shown at, in points. The thumbnail keeps
/// the frame's aspect ratio within this.
const THUMB_MAX: f32 = 48.;

/// The animation frames: a thumbnail of each, controls to add, duplicate and
/// delete, playback, and the onion skin toggle.
///
/// Frames share the project's layers — a layer exists on every frame — so this
/// only chooses which frame's pixels are being edited and shown.
pub struct FramesPanel {
    frame_count: usize,
    active_frame: usize,
    is_playing: bool,
    onion_skin: bool,
    fps: f32,
    /// A thumbnail per frame, rebuilt only when the frames' pixels change.
    thumbnails: Vec<Picture>,
    /// Canvas size, to keep thumbnails in the right proportion.
    canvas_size: (usize, usize),
}

impl FramesPanel {
    pub fn new() -> Self {
        Self {
            frame_count: 1,
            active_frame: 0,
            is_playing: false,
            onion_skin: false,
            fps: 12.0,
            thumbnails: Vec::new(),
            canvas_size: (1, 1),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sync(
        &mut self,
        frame_count: usize,
        active_frame: usize,
        is_playing: bool,
        onion_skin: bool,
        fps: f32,
    ) {
        self.frame_count = frame_count;
        self.active_frame = active_frame;
        self.is_playing = is_playing;
        self.onion_skin = onion_skin;
        self.fps = fps;
    }

    /// Replace the thumbnail images. Called only when the frames' pixels
    /// change, not every frame.
    pub fn set_thumbnails(&mut self, images: Vec<FrameImage>) {
        if let Some((w, h, _)) = images.first() {
            self.canvas_size = (*w, *h);
        }

        // Reuse existing `Picture`s where possible so their textures aren't all
        // dropped when only one frame changed.
        self.thumbnails.resize_with(images.len(), Picture::new);

        for (picture, (w, h, rgba)) in self.thumbnails.iter_mut().zip(&images) {
            picture.set(*w, *h, rgba);
        }
    }

    /// The on-screen size of a thumbnail, keeping the canvas' aspect ratio.
    fn thumb_size(&self) -> egui::Vec2 {
        let (w, h) = self.canvas_size;
        let longest = w.max(h).max(1) as f32;

        egui::vec2(
            THUMB_MAX * w as f32 / longest,
            THUMB_MAX * h as f32 / longest,
        )
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

            let mut onion = self.onion_skin;
            if ui
                .checkbox(&mut onion, "Onion skin")
                .on_hover_text("show a faint ghost of the previous frame while drawing")
                .changed()
            {
                events.push(UiEvent::ToggleOnionSkin.into());
            }

            ui.separator();

            // A thumbnail per frame with its number, the current one framed.
            // Wraps so a long animation doesn't push the panel wide.
            let size = self.thumb_size();
            ui.horizontal_wrapped(|ui| {
                for frame in 0..self.frame_count {
                    let selected = frame == self.active_frame;
                    let clicked = ui
                        .vertical(|ui| {
                            let clicked = match self.thumbnails.get_mut(frame) {
                                Some(picture) => picture.button(ui, size, selected).clicked(),
                                // No thumbnail yet (first frame before the
                                // images arrive): fall back to a plain button.
                                None => ui.selectable_label(selected, "…").clicked(),
                            };
                            ui.label(format!("{}", frame + 1));

                            clicked
                        })
                        .inner;

                    if clicked {
                        events.push(Event::SwitchFrame(frame).into());
                    }
                }
            });
        });

        events
    }
}
