use macroquad::prelude::*;

mod bg;
mod error;
mod files;
mod graphics;
mod gui;
mod input;
mod mouse;
mod project;
mod resource;
mod settings;
mod theme;
mod ui_state;
mod util;
mod wrapped_image;

const VERSION: &str = env!("CARGO_PKG_VERSION");

use error::Result;
use resource::Resources;
use ui_state::{Effect, UiEvent, UiState, WINDOW_H, WINDOW_W};
use util::*;

fn window_conf() -> macroquad::conf::Conf {
    macroquad::conf::Conf {
        miniquad_conf: miniquad::conf::Conf {
            window_title: format!("Spriterize {VERSION}: Pixel Art and 2D Sprite Editor"),
            window_width: WINDOW_W,
            window_height: WINDOW_H,
            high_dpi: true,
            platform: miniquad::conf::Platform {
                // Idle at zero CPU rather than redrawing forever. Anything that
                // animates asks for the next frame itself, via
                // `UiState::schedule_next_frame`.
                blocking_event_loop: true,
                ..Default::default()
            },
            ..Default::default()
        },
        // Input that should wake the editor up and produce a frame.
        update_on: Some(macroquad::conf::UpdateTrigger {
            key_down: true,
            mouse_down: true,
            mouse_up: true,
            mouse_motion: true,
            mouse_wheel: true,
            touch: true,
            specific_key: None,
        }),
        // Pixel art: never interpolate.
        default_filter_mode: FilterMode::Nearest,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut state = UiState::default();
    state.apply_startup_window_size();

    let mut frame = 0;

    loop {
        if let Err(e) = state.update(frame) {
            eprintln!("ERROR: {e}");
        }

        if let Err(e) = state.draw() {
            eprintln!("ERROR: {e}");
        }

        next_frame().await;

        if state.must_exit() {
            // Window size and position are tracked in memory as they change, so
            // this is where they actually reach the disk.
            state.save_settings();
            break;
        }

        frame += 1;
    }
}
