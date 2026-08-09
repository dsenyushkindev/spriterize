//! A layer's procedural generator: the recipe that fills its pixels.
//!
//! Like a [`Filter`](crate::Filter), a generator is data a layer carries and the
//! project saves — but where a filter changes how stored pixels *look*, a
//! generator *produces* them. It holds the artlib script that fills the layer
//! and the current value of each knob that script declares, so it can be re-run
//! and re-tuned any number of times, across sessions.
//!
//! The script is run by the frontend (the core has no scripting engine); lapix
//! only stores the recipe and takes the resulting pixels through
//! [`State::set_layer_generator`](crate::State::set_layer_generator). The knob
//! *declarations* (kind, range) come from running the script, so only each
//! knob's current *value* is stored here.

use crate::Color;
use serde::{Deserialize, Serialize};

/// The stored value of one knob.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GenValue {
    Float(f32),
    Int(i64),
    Color(Color),
    Bool(bool),
}

/// A layer's generator: the script that fills it, plus each knob's value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Generator {
    /// The artlib DSL source, defining `pub fn main(w, h, p)`.
    pub script: String,
    /// Knob values by id, in the order the script declares them. A value that
    /// isn't here falls back to the script's declared default, so a script that
    /// gains a knob keeps working on projects saved before it existed.
    values: Vec<(String, GenValue)>,
}

impl Generator {
    /// A generator that runs `script` with every knob at its declared default.
    pub fn new(script: String) -> Self {
        Self {
            script,
            values: Vec::new(),
        }
    }

    /// A generator with a script and a set of knob values already in hand — for
    /// re-editing the script while keeping the values.
    pub fn with_values(script: String, values: Vec<(String, GenValue)>) -> Self {
        Self { script, values }
    }

    pub fn get(&self, id: &str) -> Option<&GenValue> {
        self.values
            .iter()
            .find(|(key, _)| key == id)
            .map(|(_, value)| value)
    }

    pub fn set(&mut self, id: &str, value: GenValue) {
        match self.values.iter_mut().find(|(key, _)| key == id) {
            Some((_, held)) => *held = value,
            None => self.values.push((id.to_owned(), value)),
        }
    }

    /// Every stored knob value, in declaration order.
    pub fn values(&self) -> &[(String, GenValue)] {
        &self.values
    }
}
