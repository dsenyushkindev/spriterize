use super::{InputEvent, InputMapper, KeyBindings, KeyboardKey, KeyboardModifier, MouseButton};
use crate::Effect;
use lapix::Position;
use macroquad::prelude as mq;

const TRACKED_MOUSE_BUTTONS: [mq::MouseButton; 3] = [
    mq::MouseButton::Left,
    mq::MouseButton::Right,
    mq::MouseButton::Middle,
];

#[derive(Debug)]
pub struct InputManager {
    keys_to_track: Vec<KeyboardKey>,
    mapper: InputMapper,
    prev_mouse_canvas: Position<i32>,
    mouse_canvas: Position<i32>,
    mouse: Position<f32>,
    prev_mouse: Position<f32>,
}

impl InputManager {
    pub fn new(keys_to_track: Vec<KeyboardKey>) -> Self {
        Self {
            keys_to_track,
            mapper: InputMapper,
            prev_mouse_canvas: Default::default(),
            mouse_canvas: Default::default(),
            mouse: Default::default(),
            prev_mouse: Default::default(),
        }
    }

    pub fn sync(&mut self, mouse_pos: Position<f32>, mouse_canvas_pos: Position<i32>) {
        self.prev_mouse_canvas = self.mouse_canvas;
        self.mouse_canvas = mouse_canvas_pos;
        self.prev_mouse = self.mouse;
        self.mouse = mouse_pos;
    }

    pub fn update(&self, key_bindings: &KeyBindings) -> Vec<Effect> {
        let input_events = self.get_input_events();

        self.mapper.map(key_bindings, input_events)
    }

    fn get_input_events(&self) -> Vec<InputEvent> {
        let mut events = Vec::new();

        // mouse

        for button in TRACKED_MOUSE_BUTTONS {
            if mq::is_mouse_button_pressed(button) {
                events.push(InputEvent::MouseButtonPress(MouseButton(button)));
            }
            if mq::is_mouse_button_down(button) {
                events.push(InputEvent::MouseButtonDown(MouseButton(button)));
            }
            if mq::is_mouse_button_released(button) {
                events.push(InputEvent::MouseButtonRelease(MouseButton(button)));
            }
        }

        if self.prev_mouse_canvas != self.mouse_canvas {
            events.push(InputEvent::MouseCanvasMove(
                self.mouse_canvas - self.prev_mouse_canvas,
            ));
        }

        if self.prev_mouse != self.mouse {
            events.push(InputEvent::MouseRealMove(
                (self.mouse - self.prev_mouse).into(),
            ));
        }

        let scroll = mq::mouse_wheel().1;
        if scroll > 0. {
            events.push(InputEvent::MouseScrollUp);
        } else if scroll < 0. {
            events.push(InputEvent::MouseScrollDown);
        }

        // keyboard

        for key in &self.keys_to_track {
            if mq::is_key_pressed(key.0) {
                events.push(InputEvent::KeyPress(*key));
            }
            if mq::is_key_down(key.0) {
                events.push(InputEvent::KeyDown(*key));
            }
            if mq::is_key_released(key.0) {
                events.push(InputEvent::KeyRelease(*key));
            }
        }

        if mq::is_key_down(mq::KeyCode::RightShift) || mq::is_key_down(mq::KeyCode::LeftShift) {
            events.push(InputEvent::KeyModifier(KeyboardModifier::Shift));
        }
        if mq::is_key_down(mq::KeyCode::RightControl) || mq::is_key_down(mq::KeyCode::LeftControl) {
            events.push(InputEvent::KeyModifier(KeyboardModifier::Control));
        }
        if mq::is_key_down(mq::KeyCode::RightAlt) || mq::is_key_down(mq::KeyCode::LeftAlt) {
            events.push(InputEvent::KeyModifier(KeyboardModifier::Alt));
        }
        if mq::is_key_down(mq::KeyCode::RightSuper) || mq::is_key_down(mq::KeyCode::LeftSuper) {
            events.push(InputEvent::KeyModifier(KeyboardModifier::Super));
        }

        events
    }
}
