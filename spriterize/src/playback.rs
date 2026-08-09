//! Playing the animation: advancing through frames on a clock.
//!
//! Playback doesn't render anything of its own. It only decides when to move to
//! the next frame; the frame switch is a normal edit, so the canvas, the
//! preview and everything else update the same way they do when a frame is
//! chosen by hand.

use std::time::{Duration, SystemTime};

const DEFAULT_FPS: f32 = 12.0;
pub const MIN_FPS: f32 = 1.0;
pub const MAX_FPS: f32 = 60.0;

pub struct Playback {
    playing: bool,
    fps: f32,
    /// When the frame currently on screen started showing.
    shown_since: SystemTime,
}

impl Playback {
    pub fn new() -> Self {
        Self {
            playing: false,
            fps: DEFAULT_FPS,
            shown_since: SystemTime::now(),
        }
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn fps(&self) -> f32 {
        self.fps
    }

    pub fn set_fps(&mut self, fps: f32) {
        self.fps = fps.clamp(MIN_FPS, MAX_FPS);
    }

    pub fn play(&mut self) {
        self.playing = true;
        self.shown_since = SystemTime::now();
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn toggle(&mut self) {
        if self.playing {
            self.pause();
        } else {
            self.play();
        }
    }

    /// How many frames to advance since the last check — usually 0, and 1 once
    /// a frame's worth of time has passed.
    ///
    /// The clock is moved forward by whole frame-durations as they are consumed,
    /// so playback keeps time even though the app only redraws in bursts. If it
    /// were suspended for a while, the returned count catches up in one step
    /// rather than replaying every missed frame.
    pub fn advance(&mut self) -> usize {
        if !self.playing {
            return 0;
        }

        let elapsed = self.shown_since.elapsed().unwrap_or_default().as_secs_f32();
        let per_frame = 1.0 / self.fps;

        if elapsed < per_frame {
            return 0;
        }

        let steps = (elapsed / per_frame) as u32;
        self.shown_since += Duration::from_secs_f32(steps as f32 * per_frame);

        steps as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_paused() {
        let mut playback = Playback::new();

        assert!(!playback.is_playing());
        // A paused playback never advances, whatever the clock says.
        assert_eq!(playback.advance(), 0);
    }

    #[test]
    fn toggle_flips_between_playing_and_paused() {
        let mut playback = Playback::new();

        playback.toggle();
        assert!(playback.is_playing());
        playback.toggle();
        assert!(!playback.is_playing());
    }

    #[test]
    fn fps_is_clamped_to_the_allowed_range() {
        let mut playback = Playback::new();

        playback.set_fps(1000.0);
        assert_eq!(playback.fps(), MAX_FPS);

        playback.set_fps(0.0);
        assert_eq!(playback.fps(), MIN_FPS);
    }

    #[test]
    fn a_paused_playback_does_not_advance_after_starting() {
        // Freshly played, no time has passed, so nothing to advance yet.
        let mut playback = Playback::new();
        playback.play();

        assert_eq!(playback.advance(), 0);
    }
}
