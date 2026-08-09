//! A layer's procedural generator recipe.
//!
//! The frontend executes either an artlib script or a visual artlib graph and
//! hands the resulting pixels to `State::set_layer_generator`. Lapix stores the
//! definition and its current named parameter values so both representations
//! are saved, undoable, and re-runnable without depending on either executor.

use crate::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GenValue {
    Float(f32),
    Int(i64),
    Color(Color),
    Bool(bool),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GeneratorDefinition {
    Script(String),
    Graph(GeneratorGraph),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GeneratorGraph {
    pub nodes: Vec<GeneratorGraphNode>,
    pub wires: Vec<GeneratorGraphWire>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratorGraphNode {
    pub id: u64,
    pub position: [f32; 2],
    pub node: GeneratorNode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratorGraphWire {
    pub from_node: u64,
    pub from_output: usize,
    pub to_node: u64,
    pub to_input: usize,
}

/// Serializable artlib graph vocabulary. Scalar fields are fallback values
/// used when their corresponding input socket is not connected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GeneratorNode {
    FloatKnob {
        id: String,
        default: f32,
        min: f32,
        max: f32,
    },
    IntKnob {
        id: String,
        default: i64,
        min: i64,
        max: i64,
    },
    ColorKnob {
        id: String,
        default: [u8; 4],
    },
    BoolKnob {
        id: String,
        default: bool,
    },
    Disk {
        cx: f32,
        cy: f32,
        r: f32,
    },
    Rect {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    },
    Perlin {
        period: i64,
        seed: i64,
    },
    Union,
    Outline {
        weight: f32,
        inset: f32,
    },
    Solid {
        color: [u8; 4],
    },
    FromGrid {
        low: [u8; 4],
        high: [u8; 4],
    },
    Paint {
        antialias: bool,
        opacity: f32,
    },
    Output,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Generator {
    pub definition: GeneratorDefinition,
    values: Vec<(String, GenValue)>,
}

impl Generator {
    pub fn new(script: String) -> Self {
        Self::with_definition(GeneratorDefinition::Script(script), Vec::new())
    }

    pub fn graph(graph: GeneratorGraph) -> Self {
        Self::with_definition(GeneratorDefinition::Graph(graph), Vec::new())
    }

    pub fn with_values(script: String, values: Vec<(String, GenValue)>) -> Self {
        Self::with_definition(GeneratorDefinition::Script(script), values)
    }

    pub fn with_definition(
        definition: GeneratorDefinition,
        values: Vec<(String, GenValue)>,
    ) -> Self {
        Self { definition, values }
    }

    pub fn script(&self) -> Option<&str> {
        match &self.definition {
            GeneratorDefinition::Script(script) => Some(script),
            GeneratorDefinition::Graph(_) => None,
        }
    }

    pub fn graph_definition(&self) -> Option<&GeneratorGraph> {
        match &self.definition {
            GeneratorDefinition::Graph(graph) => Some(graph),
            GeneratorDefinition::Script(_) => None,
        }
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

    pub fn values(&self) -> &[(String, GenValue)] {
        &self.values
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_generator_round_trips_through_project_encoding() {
        let graph = GeneratorGraph {
            nodes: vec![GeneratorGraphNode {
                id: 7,
                position: [12.0, 34.0],
                node: GeneratorNode::Output,
            }],
            wires: Vec::new(),
        };
        let mut generator = Generator::graph(graph);
        generator.set("radius", GenValue::Float(12.0));

        let bytes = bincode::serialize(&generator).unwrap();
        let decoded: Generator = bincode::deserialize(&bytes).unwrap();
        assert_eq!(decoded, generator);
    }
}
