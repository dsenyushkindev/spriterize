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

/// Serializable value type carried by a graph or reusable-element port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeneratorSocket {
    Float,
    Int,
    Color,
    Bool,
    Shape,
    Grid,
    Shader,
    Canvas,
}

/// A stable, named port on a reusable graph element.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratorElementPort {
    pub id: String,
    pub name: String,
    pub socket: GeneratorSocket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeneratorNoiseSource {
    Value,
    Perlin,
    Worley,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeneratorWorleyFeature {
    F1,
    F2,
    F2F1,
}

/// Serializable artlib graph vocabulary. Scalar fields are fallback values
/// used when their corresponding input socket is not connected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GeneratorNode {
    Float {
        value: f32,
    },
    Int {
        value: i64,
    },
    Bool {
        value: bool,
    },
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
    Color {
        value: [u8; 4],
    },
    Disk {
        cx: f32,
        cy: f32,
        r: f32,
    },
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
    },
    Ring {
        cx: f32,
        cy: f32,
        inner: f32,
        outer: f32,
    },
    Rect {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    },
    HalfPlane {
        nx: f32,
        ny: f32,
        d: f32,
    },
    Diamond {
        cx: f32,
        cy: f32,
        r: f32,
    },
    Capsule {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        r: f32,
    },
    Sector {
        cx: f32,
        cy: f32,
        from: f32,
        to: f32,
    },
    ChamferedRect {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        cut: f32,
    },
    Hexagon {
        cx: f32,
        cy: f32,
        radius: f32,
        flat_top: bool,
    },
    Polyline {
        points: Vec<[f32; 2]>,
        radius: f32,
    },
    Polygon {
        points: Vec<[f32; 2]>,
    },
    Everywhere,
    Perlin {
        size: i64,
        period: i64,
        seed: i64,
    },
    ValueNoise {
        size: i64,
        period: i64,
        seed: i64,
    },
    Worley {
        size: i64,
        period: i64,
        seed: i64,
        feature: GeneratorWorleyFeature,
        jitter: f32,
    },
    Fbm {
        size: i64,
        seed: i64,
        octaves: i64,
        period: i64,
        source: GeneratorNoiseSource,
        falloff: f32,
    },
    Ridged {
        size: i64,
        seed: i64,
        octaves: i64,
        period: i64,
        source: GeneratorNoiseSource,
    },
    Stripes {
        size: i64,
        cycles_x: i64,
        cycles_y: i64,
        phase: f32,
    },
    ConstantGrid {
        size: i64,
        value: f32,
    },
    Union,
    Intersect,
    Subtract,
    Invert,
    Expand {
        radius: f32,
    },
    Outline {
        weight: f32,
        inset: f32,
    },
    Translate {
        dx: f32,
        dy: f32,
    },
    Rotate {
        degrees: f32,
        cx: f32,
        cy: f32,
    },
    Scale {
        factor: f32,
        cx: f32,
        cy: f32,
    },
    Mirror4 {
        width: f32,
        height: f32,
    },
    PolarArray {
        count: i64,
        cx: f32,
        cy: f32,
        phase: f32,
    },
    FieldX,
    FieldY,
    FieldConstant {
        value: f32,
    },
    FieldAdd,
    FieldSubtract,
    FieldMultiply,
    FieldDivide,
    FieldMinimum,
    FieldMaximum,
    FieldAbsolute,
    FieldSine,
    FieldPower {
        exponent: f32,
    },
    FieldClamp {
        low: f32,
        high: f32,
    },
    FieldHypot,
    FieldSmoothstep {
        edge0: f32,
        edge1: f32,
    },
    FieldSelect,
    HeightProfile {
        values: Vec<f32>,
        crest: f32,
        foot: f32,
    },
    GridToField,
    GridNormalize,
    GridClamp {
        low: f32,
        high: f32,
    },
    GridGain {
        power: f32,
    },
    GridRemap {
        low: f32,
        high: f32,
    },
    GridQuantize {
        steps: f32,
    },
    GridLerp {
        amount: f32,
    },
    GridBlur {
        radius: i64,
        passes: i64,
    },
    GridHighpass {
        radius: i64,
    },
    GridWarp {
        amount: f32,
    },
    GridRelief {
        azimuth: f32,
        strength: f32,
        ambient: f32,
    },
    GridMask {
        low: f32,
        high: f32,
        softness: f32,
    },
    GridAdd,
    GridSubtract,
    GridMultiply,
    GridScale {
        factor: f32,
    },
    GridOffset {
        amount: f32,
    },
    GridNegate,
    GridAbsolute,
    AlphaColor {
        alpha: i64,
    },
    ShadeColor {
        factor: f32,
    },
    MixColor {
        amount: f32,
    },
    Solid {
        color: [u8; 4],
    },
    Vertical {
        top: [u8; 4],
        bottom: [u8; 4],
        y0: f32,
        y1: f32,
    },
    Horizontal {
        left: [u8; 4],
        right: [u8; 4],
        x0: f32,
        x1: f32,
    },
    Radial {
        cx: f32,
        cy: f32,
        radius: f32,
        inner: [u8; 4],
        outer: [u8; 4],
    },
    Elliptical {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        inner: [u8; 4],
        outer: [u8; 4],
    },
    FromField {
        low: [u8; 4],
        high: [u8; 4],
        lo: f32,
        hi: f32,
    },
    FromGrid {
        low: [u8; 4],
        high: [u8; 4],
        lo: f32,
        hi: f32,
    },
    AlphaField {
        color: [u8; 4],
        lo: f32,
        hi: f32,
    },
    RgbaFields,
    Paint {
        antialias: bool,
        opacity: f32,
    },
    Stamp {
        antialias: bool,
    },
    Fill,
    Modulate,
    Output,
    /// An invocation of a collection-owned reusable graph element. Ports are
    /// snapshotted so the graph stays readable and type-checkable even when
    /// viewed outside its collection.
    ElementCall {
        element: String,
        name: String,
        inputs: Vec<GeneratorElementPort>,
        outputs: Vec<GeneratorElementPort>,
    },
    /// A public input inside an element definition.
    ElementInput {
        port: GeneratorElementPort,
    },
    /// A public output inside an element definition.
    ElementOutput {
        port: GeneratorElementPort,
    },
    FloatAdd,
    FloatSubtract,
    FloatMultiply,
    FloatDivide,
    FloatToField,
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
