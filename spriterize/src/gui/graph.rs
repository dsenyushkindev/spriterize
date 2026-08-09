//! The generator node graph: a visual front-end onto the same artlib calls as
//! the DSL.
//!
//! A graph is a set of nodes wired together; each node is an artlib operation and
//! each wire carries an artlib value (a shape [`Field`], a noise [`Grid`], a
//! [`Shader`], or an in-progress [`Canvas`]). Evaluating from the single `Output`
//! node produces the layer's pixels — the same result a script produces, through
//! the same engine.
//!
//! This module is the editor adapter and evaluator. The nodes are plain,
//! serialized project data, and evaluation turns that data into artlib
//! values on demand. The interactive editor (a `SnarlViewer`) and the per-layer
//! window are built on top of this.

use artlib::fields::Field;
use artlib::raster::{self, Canvas, Rgba, Shader};
use artlib::texture::{self, Grid};
use egui_snarl::ui::{PinInfo, SnarlViewer};
use egui_snarl::{InPin, InPinId, NodeId, OutPin, Snarl};
use lapix::{
    GeneratorGraph, GeneratorGraphNode, GeneratorGraphWire, GeneratorNode as Node,
    GeneratorNoiseSource, GeneratorWorleyFeature,
};
use std::collections::{HashMap, HashSet};

/// Values supplied by the layer for named parameter nodes.
pub type KnobValues = HashMap<String, KnobValue>;

/// A named graph parameter's current value.
#[derive(Clone, Debug, PartialEq)]
pub enum KnobValue {
    Float(f32),
    Int(i64),
    Color(Rgba),
    Bool(bool),
}

/// What a wire carries — used both to type-check the evaluator and (later) to
/// colour the editor's pins and refuse mismatched connections.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Socket {
    Float,
    Int,
    Color,
    Bool,
    Shape,
    Grid,
    Shader,
    Canvas,
}

trait NodeExt {
    fn title(&self) -> &'static str;
    fn inputs(&self) -> &'static [Socket];
    fn input_label(&self, index: usize) -> Option<&'static str>;
    fn output(&self) -> Option<Socket>;
}

impl NodeExt for Node {
    fn title(&self) -> &'static str {
        match self {
            Node::Float { .. } => "Number",
            Node::Int { .. } => "Integer",
            Node::Bool { .. } => "Boolean",
            Node::FloatKnob { .. } => "Number parameter",
            Node::IntKnob { .. } => "Integer parameter",
            Node::ColorKnob { .. } => "Color parameter",
            Node::BoolKnob { .. } => "Boolean parameter",
            Node::Color { .. } => "Color",
            Node::Disk { .. } => "Disk",
            Node::Ellipse { .. } => "Ellipse",
            Node::Ring { .. } => "Ring",
            Node::Rect { .. } => "Rect",
            Node::HalfPlane { .. } => "Half plane",
            Node::Diamond { .. } => "Diamond",
            Node::Capsule { .. } => "Capsule",
            Node::Sector { .. } => "Sector",
            Node::ChamferedRect { .. } => "Chamfered rect",
            Node::Hexagon { .. } => "Hexagon",
            Node::Polyline { .. } => "Polyline",
            Node::Polygon { .. } => "Polygon",
            Node::Everywhere => "Everywhere",
            Node::ValueNoise { .. } => "Value noise",
            Node::Perlin { .. } => "Perlin",
            Node::Worley { .. } => "Worley",
            Node::Fbm { .. } => "FBM",
            Node::Ridged { .. } => "Ridged",
            Node::Stripes { .. } => "Stripes",
            Node::ConstantGrid { .. } => "Constant grid",
            Node::Union => "Union",
            Node::Intersect => "Intersect",
            Node::Subtract => "Subtract",
            Node::Invert => "Invert",
            Node::Expand { .. } => "Expand",
            Node::Outline { .. } => "Outline",
            Node::Translate { .. } => "Translate",
            Node::Rotate { .. } => "Rotate",
            Node::Scale { .. } => "Scale",
            Node::Mirror4 { .. } => "Mirror four",
            Node::PolarArray { .. } => "Polar array",
            Node::FieldX => "X coordinate",
            Node::FieldY => "Y coordinate",
            Node::FieldConstant { .. } => "Constant field",
            Node::FieldAdd => "Field add",
            Node::FieldSubtract => "Field subtract",
            Node::FieldMultiply => "Field multiply",
            Node::FieldDivide => "Field divide",
            Node::FieldMinimum => "Field minimum",
            Node::FieldMaximum => "Field maximum",
            Node::FieldAbsolute => "Field absolute",
            Node::FieldSine => "Field sine",
            Node::FieldPower { .. } => "Field power",
            Node::FieldClamp { .. } => "Field clamp",
            Node::FieldHypot => "Field hypot",
            Node::FieldSmoothstep { .. } => "Field smoothstep",
            Node::FieldSelect => "Field select",
            Node::HeightProfile { .. } => "Height profile",
            Node::GridToField => "Grid to field",
            Node::GridNormalize => "Grid normalize",
            Node::GridClamp { .. } => "Grid clamp",
            Node::GridGain { .. } => "Grid gain",
            Node::GridRemap { .. } => "Grid remap",
            Node::GridQuantize { .. } => "Grid quantize",
            Node::GridLerp { .. } => "Grid lerp",
            Node::GridBlur { .. } => "Grid blur",
            Node::GridHighpass { .. } => "Grid high-pass",
            Node::GridWarp { .. } => "Grid warp",
            Node::GridRelief { .. } => "Grid relief",
            Node::GridMask { .. } => "Grid mask",
            Node::GridAdd => "Grid add",
            Node::GridSubtract => "Grid subtract",
            Node::GridMultiply => "Grid multiply",
            Node::GridScale { .. } => "Grid scale",
            Node::GridOffset { .. } => "Grid offset",
            Node::GridNegate => "Grid negate",
            Node::GridAbsolute => "Grid absolute",
            Node::AlphaColor { .. } => "Set alpha",
            Node::ShadeColor { .. } => "Shade color",
            Node::MixColor { .. } => "Mix colors",
            Node::Solid { .. } => "Solid",
            Node::Vertical { .. } => "Vertical gradient",
            Node::Horizontal { .. } => "Horizontal gradient",
            Node::Radial { .. } => "Radial gradient",
            Node::Elliptical { .. } => "Elliptical gradient",
            Node::FromField { .. } => "From field",
            Node::FromGrid { .. } => "From grid",
            Node::AlphaField { .. } => "Alpha from field",
            Node::RgbaFields => "RGBA fields",
            Node::Paint { .. } => "Paint",
            Node::Stamp { .. } => "Stamp",
            Node::Fill => "Fill",
            Node::Modulate => "Modulate",
            Node::Output => "Output",
        }
    }

    fn inputs(&self) -> &'static [Socket] {
        match self {
            Node::Float { .. }
            | Node::Int { .. }
            | Node::Bool { .. }
            | Node::FloatKnob { .. }
            | Node::IntKnob { .. }
            | Node::ColorKnob { .. }
            | Node::BoolKnob { .. }
            | Node::Color { .. }
            | Node::Everywhere
            | Node::FieldX
            | Node::FieldY => &[],
            Node::Disk { .. } => &[Socket::Float, Socket::Float, Socket::Float],
            Node::Ellipse { .. } | Node::Ring { .. } => {
                &[Socket::Float, Socket::Float, Socket::Float, Socket::Float]
            }
            Node::Rect { .. } => &[Socket::Float, Socket::Float, Socket::Float, Socket::Float],
            Node::HalfPlane { .. } | Node::Diamond { .. } => {
                &[Socket::Float, Socket::Float, Socket::Float]
            }
            Node::Capsule { .. } | Node::ChamferedRect { .. } => &[
                Socket::Float,
                Socket::Float,
                Socket::Float,
                Socket::Float,
                Socket::Float,
            ],
            Node::Sector { .. } => &[Socket::Float, Socket::Float, Socket::Float, Socket::Float],
            Node::Hexagon { .. } => &[Socket::Float, Socket::Float, Socket::Float, Socket::Bool],
            Node::Polyline { .. } => &[Socket::Float],
            Node::Polygon { .. } => &[],
            Node::Perlin { .. } | Node::ValueNoise { .. } => {
                &[Socket::Int, Socket::Int, Socket::Int]
            }
            Node::Worley { .. } => &[Socket::Int, Socket::Int, Socket::Int, Socket::Float],
            Node::Fbm { .. } => &[
                Socket::Int,
                Socket::Int,
                Socket::Int,
                Socket::Int,
                Socket::Float,
            ],
            Node::Ridged { .. } => &[Socket::Int, Socket::Int, Socket::Int, Socket::Int],
            Node::Stripes { .. } => &[Socket::Int, Socket::Int, Socket::Int, Socket::Float],
            Node::ConstantGrid { .. } => &[Socket::Int, Socket::Float],
            Node::Union
            | Node::Intersect
            | Node::Subtract
            | Node::FieldAdd
            | Node::FieldSubtract
            | Node::FieldMultiply
            | Node::FieldDivide
            | Node::FieldMinimum
            | Node::FieldMaximum
            | Node::FieldHypot => &[Socket::Shape, Socket::Shape],
            Node::Invert | Node::FieldAbsolute | Node::FieldSine => &[Socket::Shape],
            Node::Expand { .. } => &[Socket::Shape, Socket::Float],
            Node::Outline { .. } => &[Socket::Shape, Socket::Float, Socket::Float],
            Node::Translate { .. } => &[Socket::Shape, Socket::Float, Socket::Float],
            Node::Rotate { .. } | Node::Scale { .. } => {
                &[Socket::Shape, Socket::Float, Socket::Float, Socket::Float]
            }
            Node::Mirror4 { .. } => &[Socket::Shape, Socket::Float, Socket::Float],
            Node::PolarArray { .. } => &[
                Socket::Shape,
                Socket::Int,
                Socket::Float,
                Socket::Float,
                Socket::Float,
            ],
            Node::FieldConstant { .. } => &[Socket::Float],
            Node::FieldPower { .. } => &[Socket::Shape, Socket::Float],
            Node::FieldClamp { .. } | Node::FieldSmoothstep { .. } => {
                &[Socket::Shape, Socket::Float, Socket::Float]
            }
            Node::FieldSelect => &[Socket::Shape, Socket::Shape, Socket::Shape],
            Node::HeightProfile { .. } => &[Socket::Float, Socket::Float],
            Node::GridToField | Node::GridNormalize | Node::GridNegate | Node::GridAbsolute => {
                &[Socket::Grid]
            }
            Node::GridClamp { .. } | Node::GridRemap { .. } => {
                &[Socket::Grid, Socket::Float, Socket::Float]
            }
            Node::GridGain { .. }
            | Node::GridQuantize { .. }
            | Node::GridScale { .. }
            | Node::GridOffset { .. } => &[Socket::Grid, Socket::Float],
            Node::GridLerp { .. } => &[Socket::Grid, Socket::Grid, Socket::Float],
            Node::GridBlur { .. } => &[Socket::Grid, Socket::Int, Socket::Int],
            Node::GridHighpass { .. } => &[Socket::Grid, Socket::Int],
            Node::GridWarp { .. } => &[Socket::Grid, Socket::Grid, Socket::Grid, Socket::Float],
            Node::GridRelief { .. } => &[Socket::Grid, Socket::Float, Socket::Float, Socket::Float],
            Node::GridMask { .. } => &[Socket::Grid, Socket::Float, Socket::Float, Socket::Float],
            Node::GridAdd | Node::GridSubtract | Node::GridMultiply => {
                &[Socket::Grid, Socket::Grid]
            }
            Node::AlphaColor { .. } => &[Socket::Color, Socket::Int],
            Node::ShadeColor { .. } => &[Socket::Color, Socket::Float],
            Node::MixColor { .. } => &[Socket::Color, Socket::Color, Socket::Float],
            Node::Solid { .. } => &[Socket::Color],
            Node::Vertical { .. } | Node::Horizontal { .. } => {
                &[Socket::Color, Socket::Color, Socket::Float, Socket::Float]
            }
            Node::Radial { .. } => &[
                Socket::Float,
                Socket::Float,
                Socket::Float,
                Socket::Color,
                Socket::Color,
            ],
            Node::Elliptical { .. } => &[
                Socket::Float,
                Socket::Float,
                Socket::Float,
                Socket::Float,
                Socket::Color,
                Socket::Color,
            ],
            Node::FromField { .. } => &[
                Socket::Shape,
                Socket::Color,
                Socket::Color,
                Socket::Float,
                Socket::Float,
            ],
            Node::FromGrid { .. } => &[
                Socket::Grid,
                Socket::Color,
                Socket::Color,
                Socket::Float,
                Socket::Float,
            ],
            Node::AlphaField { .. } => {
                &[Socket::Shape, Socket::Color, Socket::Float, Socket::Float]
            }
            Node::RgbaFields => &[Socket::Shape, Socket::Shape, Socket::Shape, Socket::Shape],
            Node::Paint { .. } => &[
                Socket::Canvas,
                Socket::Shape,
                Socket::Shader,
                Socket::Bool,
                Socket::Float,
            ],
            Node::Stamp { .. } => &[Socket::Canvas, Socket::Shape, Socket::Shader, Socket::Bool],
            Node::Fill => &[Socket::Canvas, Socket::Shader],
            Node::Modulate => &[Socket::Canvas, Socket::Shape, Socket::Shape],
            Node::Output => &[Socket::Canvas],
        }
    }

    /// The label of an input socket, in the same order as [`Node::inputs`].
    fn input_label(&self, index: usize) -> Option<&'static str> {
        let labels: &[&str] = match self {
            Node::Float { .. }
            | Node::Int { .. }
            | Node::Bool { .. }
            | Node::FloatKnob { .. }
            | Node::IntKnob { .. }
            | Node::ColorKnob { .. }
            | Node::BoolKnob { .. }
            | Node::Color { .. }
            | Node::Everywhere
            | Node::FieldX
            | Node::FieldY
            | Node::Polygon { .. } => &[],
            Node::Disk { .. } => &["center x", "center y", "radius"],
            Node::Ellipse { .. } => &["center x", "center y", "radius x", "radius y"],
            Node::Ring { .. } => &["center x", "center y", "inner", "outer"],
            Node::Rect { .. } => &["left", "top", "right", "bottom"],
            Node::HalfPlane { .. } => &["normal x", "normal y", "distance"],
            Node::Diamond { .. } => &["center x", "center y", "radius"],
            Node::Capsule { .. } => &["x0", "y0", "x1", "y1", "radius"],
            Node::Sector { .. } => &["center x", "center y", "from", "to"],
            Node::ChamferedRect { .. } => &["left", "top", "right", "bottom", "cut"],
            Node::Hexagon { .. } => &["center x", "center y", "radius", "flat top"],
            Node::Polyline { .. } => &["radius"],
            Node::Perlin { .. } | Node::ValueNoise { .. } => &["size", "period", "seed"],
            Node::Worley { .. } => &["size", "period", "seed", "jitter"],
            Node::Fbm { .. } => &["size", "seed", "octaves", "period", "falloff"],
            Node::Ridged { .. } => &["size", "seed", "octaves", "period"],
            Node::Stripes { .. } => &["size", "cycles x", "cycles y", "phase"],
            Node::ConstantGrid { .. } => &["size", "value"],
            Node::Union
            | Node::Intersect
            | Node::Subtract
            | Node::FieldAdd
            | Node::FieldSubtract
            | Node::FieldMultiply
            | Node::FieldDivide
            | Node::FieldMinimum
            | Node::FieldMaximum
            | Node::FieldHypot => &["a", "b"],
            Node::Invert | Node::FieldAbsolute | Node::FieldSine => &["field"],
            Node::Expand { .. } => &["shape", "radius"],
            Node::Outline { .. } => &["shape", "weight", "inset"],
            Node::Translate { .. } => &["shape", "dx", "dy"],
            Node::Rotate { .. } => &["shape", "degrees", "center x", "center y"],
            Node::Scale { .. } => &["shape", "factor", "center x", "center y"],
            Node::Mirror4 { .. } => &["shape", "width", "height"],
            Node::PolarArray { .. } => &["shape", "count", "center x", "center y", "phase"],
            Node::FieldConstant { .. } => &["value"],
            Node::FieldPower { .. } => &["field", "exponent"],
            Node::FieldClamp { .. } => &["field", "low", "high"],
            Node::FieldSmoothstep { .. } => &["field", "edge 0", "edge 1"],
            Node::FieldSelect => &["condition (<= 0)", "true", "false"],
            Node::HeightProfile { .. } => &["crest", "foot"],
            Node::GridToField | Node::GridNormalize | Node::GridNegate | Node::GridAbsolute => {
                &["grid"]
            }
            Node::GridClamp { .. } | Node::GridRemap { .. } => &["grid", "low", "high"],
            Node::GridGain { .. } => &["grid", "power"],
            Node::GridQuantize { .. } => &["grid", "steps"],
            Node::GridScale { .. } => &["grid", "factor"],
            Node::GridOffset { .. } => &["grid", "amount"],
            Node::GridLerp { .. } => &["a", "b", "amount"],
            Node::GridBlur { .. } => &["grid", "radius", "passes"],
            Node::GridHighpass { .. } => &["grid", "radius"],
            Node::GridWarp { .. } => &["grid", "dx", "dy", "amount"],
            Node::GridRelief { .. } => &["grid", "azimuth", "strength", "ambient"],
            Node::GridMask { .. } => &["grid", "low", "high", "softness"],
            Node::GridAdd | Node::GridSubtract | Node::GridMultiply => &["a", "b"],
            Node::AlphaColor { .. } => &["color", "alpha"],
            Node::ShadeColor { .. } => &["color", "factor"],
            Node::MixColor { .. } => &["a", "b", "amount"],
            Node::Solid { .. } => &["color"],
            Node::Vertical { .. } => &["top", "bottom", "y0", "y1"],
            Node::Horizontal { .. } => &["left", "right", "x0", "x1"],
            Node::Radial { .. } => &["center x", "center y", "radius", "inner", "outer"],
            Node::Elliptical { .. } => &[
                "center x", "center y", "radius x", "radius y", "inner", "outer",
            ],
            Node::FromField { .. } => &["field", "low", "high", "lo", "hi"],
            Node::FromGrid { .. } => &["grid", "low", "high", "lo", "hi"],
            Node::AlphaField { .. } => &["field", "color", "lo", "hi"],
            Node::RgbaFields => &["red", "green", "blue", "alpha"],
            Node::Paint { .. } => &["canvas", "shape", "shader", "antialias", "opacity"],
            Node::Stamp { .. } => &["canvas", "shape", "shader", "antialias"],
            Node::Fill => &["canvas", "shader"],
            Node::Modulate => &["canvas", "factors", "restrict (optional)"],
            Node::Output => &["canvas"],
        };
        labels.get(index).copied()
    }

    /// The type of the output socket, if the node has one.
    fn output(&self) -> Option<Socket> {
        match self {
            Node::FloatKnob { .. } | Node::Float { .. } => Some(Socket::Float),
            Node::IntKnob { .. } | Node::Int { .. } => Some(Socket::Int),
            Node::ColorKnob { .. }
            | Node::Color { .. }
            | Node::AlphaColor { .. }
            | Node::ShadeColor { .. }
            | Node::MixColor { .. } => Some(Socket::Color),
            Node::BoolKnob { .. } | Node::Bool { .. } => Some(Socket::Bool),
            Node::Disk { .. }
            | Node::Ellipse { .. }
            | Node::Ring { .. }
            | Node::Rect { .. }
            | Node::HalfPlane { .. }
            | Node::Diamond { .. }
            | Node::Capsule { .. }
            | Node::Sector { .. }
            | Node::ChamferedRect { .. }
            | Node::Hexagon { .. }
            | Node::Polyline { .. }
            | Node::Polygon { .. }
            | Node::Everywhere
            | Node::Union
            | Node::Intersect
            | Node::Subtract
            | Node::Invert
            | Node::Expand { .. }
            | Node::Outline { .. }
            | Node::Translate { .. }
            | Node::Rotate { .. }
            | Node::Scale { .. }
            | Node::Mirror4 { .. }
            | Node::PolarArray { .. }
            | Node::FieldX
            | Node::FieldY
            | Node::FieldConstant { .. }
            | Node::FieldAdd
            | Node::FieldSubtract
            | Node::FieldMultiply
            | Node::FieldDivide
            | Node::FieldMinimum
            | Node::FieldMaximum
            | Node::FieldAbsolute
            | Node::FieldSine
            | Node::FieldPower { .. }
            | Node::FieldClamp { .. }
            | Node::FieldHypot
            | Node::FieldSmoothstep { .. }
            | Node::FieldSelect
            | Node::HeightProfile { .. }
            | Node::GridToField
            | Node::GridMask { .. } => Some(Socket::Shape),
            Node::Perlin { .. }
            | Node::ValueNoise { .. }
            | Node::Worley { .. }
            | Node::Fbm { .. }
            | Node::Ridged { .. }
            | Node::Stripes { .. }
            | Node::ConstantGrid { .. }
            | Node::GridNormalize
            | Node::GridClamp { .. }
            | Node::GridGain { .. }
            | Node::GridRemap { .. }
            | Node::GridQuantize { .. }
            | Node::GridLerp { .. }
            | Node::GridBlur { .. }
            | Node::GridHighpass { .. }
            | Node::GridWarp { .. }
            | Node::GridRelief { .. }
            | Node::GridAdd
            | Node::GridSubtract
            | Node::GridMultiply
            | Node::GridScale { .. }
            | Node::GridOffset { .. }
            | Node::GridNegate
            | Node::GridAbsolute => Some(Socket::Grid),
            Node::Solid { .. }
            | Node::Vertical { .. }
            | Node::Horizontal { .. }
            | Node::Radial { .. }
            | Node::Elliptical { .. }
            | Node::FromField { .. }
            | Node::FromGrid { .. }
            | Node::AlphaField { .. }
            | Node::RgbaFields => Some(Socket::Shader),
            Node::Paint { .. } | Node::Stamp { .. } | Node::Fill | Node::Modulate => {
                Some(Socket::Canvas)
            }
            Node::Output => None,
        }
    }
}

/// A value produced while evaluating the graph.
#[derive(Clone)]
enum Value {
    Float(f32),
    Int(i64),
    Color(Rgba),
    Bool(bool),
    Shape(Field),
    Grid(Grid),
    Shader(Shader),
    Canvas(Canvas),
}

pub fn to_recipe(snarl: &Snarl<Node>) -> GeneratorGraph {
    GeneratorGraph {
        nodes: snarl
            .nodes_pos_ids()
            .map(|(id, position, node)| GeneratorGraphNode {
                id: id.0 as u64,
                position: [position.x, position.y],
                node: node.clone(),
            })
            .collect(),
        wires: snarl
            .wires()
            .map(|(from, to)| GeneratorGraphWire {
                from_node: from.node.0 as u64,
                from_output: from.output,
                to_node: to.node.0 as u64,
                to_input: to.input,
            })
            .collect(),
    }
}

pub fn from_recipe(recipe: &GeneratorGraph) -> Result<Snarl<Node>, String> {
    let mut snarl = Snarl::new();
    let mut ids = HashMap::new();
    for held in &recipe.nodes {
        let id = snarl.insert_node(
            egui::pos2(held.position[0], held.position[1]),
            held.node.clone(),
        );
        if ids.insert(held.id, id).is_some() {
            return Err(format!("duplicate graph node id {}", held.id));
        }
    }
    for wire in &recipe.wires {
        let from = *ids
            .get(&wire.from_node)
            .ok_or_else(|| format!("wire starts at missing node {}", wire.from_node))?;
        let to = *ids
            .get(&wire.to_node)
            .ok_or_else(|| format!("wire ends at missing node {}", wire.to_node))?;
        let source = &snarl[from];
        let target = &snarl[to];
        let Some(output_socket) = source.output().filter(|_| wire.from_output == 0) else {
            return Err(format!(
                "{} has no output {}",
                source.title(),
                wire.from_output
            ));
        };
        let Some(&input_socket) = target.inputs().get(wire.to_input) else {
            return Err(format!("{} has no input {}", target.title(), wire.to_input));
        };
        if !sockets_compatible(output_socket, input_socket) {
            return Err(format!(
                "cannot connect {output_socket:?} to {input_socket:?}"
            ));
        }
        snarl.connect(
            egui_snarl::OutPinId {
                node: from,
                output: wire.from_output,
            },
            InPinId {
                node: to,
                input: wire.to_input,
            },
        );
    }
    Ok(snarl)
}

pub fn default_recipe() -> GeneratorGraph {
    let mut snarl = Snarl::new();
    let radius = snarl.insert_node(
        egui::pos2(20.0, 80.0),
        Node::FloatKnob {
            id: "radius".into(),
            default: 20.0,
            min: 2.0,
            max: 40.0,
        },
    );
    let disk = snarl.insert_node(
        egui::pos2(220.0, 80.0),
        Node::Disk {
            cx: 32.0,
            cy: 32.0,
            r: 20.0,
        },
    );
    let solid = snarl.insert_node(
        egui::pos2(220.0, 260.0),
        Node::Solid {
            color: [220, 120, 60, 255],
        },
    );
    let paint = snarl.insert_node(
        egui::pos2(440.0, 140.0),
        Node::Paint {
            antialias: true,
            opacity: 1.0,
        },
    );
    let output = snarl.insert_node(egui::pos2(660.0, 140.0), Node::Output);
    snarl.connect(
        egui_snarl::OutPinId {
            node: radius,
            output: 0,
        },
        InPinId {
            node: disk,
            input: 2,
        },
    );
    snarl.connect(
        egui_snarl::OutPinId {
            node: disk,
            output: 0,
        },
        InPinId {
            node: paint,
            input: 1,
        },
    );
    snarl.connect(
        egui_snarl::OutPinId {
            node: solid,
            output: 0,
        },
        InPinId {
            node: paint,
            input: 2,
        },
    );
    snarl.connect(
        egui_snarl::OutPinId {
            node: paint,
            output: 0,
        },
        InPinId {
            node: output,
            input: 0,
        },
    );
    to_recipe(&snarl)
}

pub struct GraphViewer;

#[allow(refining_impl_trait)]
impl SnarlViewer<Node> for GraphViewer {
    fn title(&mut self, node: &Node) -> String {
        node.title().to_owned()
    }

    fn inputs(&mut self, node: &Node) -> usize {
        node.inputs().len()
    }

    fn outputs(&mut self, node: &Node) -> usize {
        usize::from(node.output().is_some())
    }

    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<Node>,
    ) -> PinInfo {
        let node = &snarl[pin.id.node];
        let socket = node.inputs()[pin.id.input];
        ui.label(node.input_label(pin.id.input).unwrap_or("input"));
        pin_info(socket)
    }

    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<Node>,
    ) -> PinInfo {
        let socket = snarl[pin.id.node].output().expect("output pin exists");
        ui.label(socket_name(socket));
        pin_info(socket)
    }

    fn has_body(&mut self, node: &Node) -> bool {
        !matches!(node, Node::Union | Node::Output)
    }

    fn show_body(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<Node>,
    ) {
        show_node_properties(&mut snarl[node], ui);
    }

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<Node>) {
        let Some(output) = snarl[from.id.node].output() else {
            return;
        };
        let Some(&input) = snarl[to.id.node].inputs().get(to.id.input) else {
            return;
        };
        if sockets_compatible(output, input) {
            for remote in to.remotes.to_vec() {
                snarl.disconnect(remote, to.id);
            }
            snarl.connect(from.id, to.id);
        }
    }

    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<Node>) -> bool {
        true
    }

    fn show_graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<Node>,
    ) {
        ui.label("Add node");
        add_node_menu(pos, ui, snarl);
    }

    fn has_node_menu(&mut self, _node: &Node) -> bool {
        true
    }

    fn show_node_menu(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<Node>,
    ) {
        if ui.button("Delete").clicked() {
            snarl.remove_node(node);
            ui.close_menu();
        }
    }
}

fn pin_info(socket: Socket) -> PinInfo {
    let color = match socket {
        Socket::Float => egui::Color32::from_rgb(90, 180, 240),
        Socket::Int => egui::Color32::from_rgb(80, 130, 220),
        Socket::Color => egui::Color32::from_rgb(235, 130, 190),
        Socket::Bool => egui::Color32::from_rgb(220, 100, 90),
        Socket::Shape => egui::Color32::from_rgb(100, 210, 130),
        Socket::Grid => egui::Color32::from_rgb(100, 190, 180),
        Socket::Shader => egui::Color32::from_rgb(235, 190, 80),
        Socket::Canvas => egui::Color32::from_rgb(170, 130, 235),
    };
    PinInfo::circle().with_fill(color)
}

fn socket_name(socket: Socket) -> &'static str {
    match socket {
        Socket::Float => "number",
        Socket::Int => "integer",
        Socket::Color => "color",
        Socket::Bool => "boolean",
        Socket::Shape => "shape",
        Socket::Grid => "grid",
        Socket::Shader => "shader",
        Socket::Canvas => "canvas",
    }
}

fn show_node_properties(node: &mut Node, ui: &mut egui::Ui) {
    match node {
        Node::Float { value } => {
            ui.add(egui::DragValue::new(value));
        }
        Node::Int { value } => {
            ui.add(egui::DragValue::new(value));
        }
        Node::Bool { value } => {
            ui.checkbox(value, "value");
        }
        Node::FloatKnob {
            id,
            default,
            min,
            max,
        } => {
            ui.text_edit_singleline(id);
            ui.horizontal(|ui| {
                ui.label("value");
                ui.add(egui::DragValue::new(default));
            });
            ui.horizontal(|ui| {
                ui.label("range");
                ui.add(egui::DragValue::new(min));
                ui.add(egui::DragValue::new(max));
            });
        }
        Node::IntKnob {
            id,
            default,
            min,
            max,
        } => {
            ui.text_edit_singleline(id);
            ui.horizontal(|ui| {
                ui.label("value");
                ui.add(egui::DragValue::new(default));
            });
            ui.horizontal(|ui| {
                ui.label("range");
                ui.add(egui::DragValue::new(min));
                ui.add(egui::DragValue::new(max));
            });
        }
        Node::ColorKnob { id, default } => {
            ui.text_edit_singleline(id);
            ui.color_edit_button_srgba_unmultiplied(default);
        }
        Node::Color { value } => {
            ui.color_edit_button_srgba_unmultiplied(value);
        }
        Node::BoolKnob { id, default } => {
            ui.text_edit_singleline(id);
            ui.checkbox(default, "default");
        }
        Node::Disk { cx, cy, r } => {
            ui.add(egui::DragValue::new(cx).prefix("x "));
            ui.add(egui::DragValue::new(cy).prefix("y "));
            ui.add(egui::DragValue::new(r).prefix("r "));
        }
        Node::Rect { x0, y0, x1, y1 } => {
            for (name, value) in [("x0 ", x0), ("y0 ", y0), ("x1 ", x1), ("y1 ", y1)] {
                ui.add(egui::DragValue::new(value).prefix(name));
            }
        }
        Node::Polyline { points, radius } => {
            ui.add(egui::DragValue::new(radius).prefix("radius "));
            show_points(points, ui);
        }
        Node::Polygon { points } => show_points(points, ui),
        Node::HeightProfile {
            values,
            crest,
            foot,
        } => {
            ui.add(egui::DragValue::new(crest).prefix("crest "));
            ui.add(egui::DragValue::new(foot).prefix("foot "));
            ui.label(format!("{} samples", values.len()));
        }
        Node::Perlin { size, period, seed } => {
            ui.add(egui::DragValue::new(size).prefix("size "));
            ui.add(egui::DragValue::new(period).prefix("period "));
            ui.add(egui::DragValue::new(seed).prefix("seed "));
        }
        Node::Worley {
            size,
            period,
            seed,
            feature,
            jitter,
        } => {
            for (name, value) in [("size ", size), ("period ", period), ("seed ", seed)] {
                ui.add(egui::DragValue::new(value).prefix(name));
            }
            ui.add(egui::DragValue::new(jitter).prefix("jitter "));
            egui::ComboBox::from_label("feature")
                .selected_text(format!("{feature:?}"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(feature, GeneratorWorleyFeature::F1, "F1");
                    ui.selectable_value(feature, GeneratorWorleyFeature::F2, "F2");
                    ui.selectable_value(feature, GeneratorWorleyFeature::F2F1, "F2-F1");
                });
        }
        Node::Fbm { source, .. } | Node::Ridged { source, .. } => {
            egui::ComboBox::from_label("source")
                .selected_text(format!("{source:?}"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(source, GeneratorNoiseSource::Value, "Value noise");
                    ui.selectable_value(source, GeneratorNoiseSource::Perlin, "Perlin");
                    ui.selectable_value(source, GeneratorNoiseSource::Worley, "Worley");
                });
        }
        Node::Outline { weight, inset } => {
            ui.add(egui::DragValue::new(weight).prefix("weight "));
            ui.add(egui::DragValue::new(inset).prefix("inset "));
        }
        Node::Solid { color } => {
            ui.color_edit_button_srgba_unmultiplied(color);
        }
        Node::FromGrid { low, high, lo, hi } => {
            ui.label("low");
            ui.color_edit_button_srgba_unmultiplied(low);
            ui.label("high");
            ui.color_edit_button_srgba_unmultiplied(high);
            ui.add(egui::DragValue::new(lo).prefix("lo "));
            ui.add(egui::DragValue::new(hi).prefix("hi "));
        }
        Node::Paint { antialias, opacity } => {
            ui.checkbox(antialias, "antialias");
            ui.add(egui::Slider::new(opacity, 0.0..=1.0).text("opacity"));
        }
        _ => {}
    }
}

fn show_points(points: &mut Vec<[f32; 2]>, ui: &mut egui::Ui) {
    let mut remove = None;
    for (index, point) in points.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(index.to_string());
            ui.add(egui::DragValue::new(&mut point[0]).prefix("x "));
            ui.add(egui::DragValue::new(&mut point[1]).prefix("y "));
            if ui.small_button("−").clicked() {
                remove = Some(index);
            }
        });
    }
    if let Some(index) = remove {
        points.remove(index);
    }
    if ui.small_button("+ point").clicked() {
        points.push(points.last().copied().unwrap_or([0.0, 0.0]));
    }
}

fn add_node_menu(pos: egui::Pos2, ui: &mut egui::Ui, snarl: &mut Snarl<Node>) {
    type NodeFactory = fn() -> Node;
    fn entries(
        ui: &mut egui::Ui,
        pos: egui::Pos2,
        snarl: &mut Snarl<Node>,
        values: &[(&str, NodeFactory)],
    ) {
        for (label, make) in values {
            if ui.button(*label).clicked() {
                snarl.insert_node(pos, make());
                ui.close_menu();
            }
        }
    }
    ui.menu_button("Parameters and values", |ui| {
        entries(
            ui,
            pos,
            snarl,
            &[
                ("Number", || Node::Float { value: 0.0 }),
                ("Integer", || Node::Int { value: 0 }),
                ("Boolean", || Node::Bool { value: true }),
                ("Number parameter", || Node::FloatKnob {
                    id: "value".into(),
                    default: 0.0,
                    min: 0.0,
                    max: 100.0,
                }),
                ("Integer parameter", || Node::IntKnob {
                    id: "value".into(),
                    default: 0,
                    min: 0,
                    max: 100,
                }),
                ("Color parameter", || Node::ColorKnob {
                    id: "color".into(),
                    default: [255, 255, 255, 255],
                }),
                ("Boolean parameter", || Node::BoolKnob {
                    id: "enabled".into(),
                    default: true,
                }),
                ("Color", || Node::Color { value: [255; 4] }),
                ("Set alpha", || Node::AlphaColor { alpha: 255 }),
                ("Shade color", || Node::ShadeColor { factor: 1.0 }),
                ("Mix colors", || Node::MixColor { amount: 0.5 }),
            ],
        )
    });
    ui.menu_button("Shapes", |ui| {
        entries(
            ui,
            pos,
            snarl,
            &[
                ("Disk", || Node::Disk {
                    cx: 32.0,
                    cy: 32.0,
                    r: 16.0,
                }),
                ("Ellipse", || Node::Ellipse {
                    cx: 32.0,
                    cy: 32.0,
                    rx: 20.0,
                    ry: 12.0,
                }),
                ("Ring", || Node::Ring {
                    cx: 32.0,
                    cy: 32.0,
                    inner: 12.0,
                    outer: 16.0,
                }),
                ("Rectangle", || Node::Rect {
                    x0: 8.0,
                    y0: 8.0,
                    x1: 56.0,
                    y1: 56.0,
                }),
                ("Half plane", || Node::HalfPlane {
                    nx: 1.0,
                    ny: 0.0,
                    d: 32.0,
                }),
                ("Diamond", || Node::Diamond {
                    cx: 32.0,
                    cy: 32.0,
                    r: 16.0,
                }),
                ("Capsule", || Node::Capsule {
                    x0: 12.0,
                    y0: 32.0,
                    x1: 52.0,
                    y1: 32.0,
                    r: 4.0,
                }),
                ("Sector", || Node::Sector {
                    cx: 32.0,
                    cy: 32.0,
                    from: 0.0,
                    to: 90.0,
                }),
                ("Chamfered rectangle", || Node::ChamferedRect {
                    x0: 8.0,
                    y0: 8.0,
                    x1: 56.0,
                    y1: 56.0,
                    cut: 6.0,
                }),
                ("Hexagon", || Node::Hexagon {
                    cx: 32.0,
                    cy: 32.0,
                    radius: 24.0,
                    flat_top: false,
                }),
                ("Polyline", || Node::Polyline {
                    points: vec![[8.0, 32.0], [56.0, 32.0]],
                    radius: 2.0,
                }),
                ("Polygon", || Node::Polygon {
                    points: vec![[32.0, 8.0], [56.0, 56.0], [8.0, 56.0]],
                }),
                ("Everywhere", || Node::Everywhere),
            ],
        )
    });
    ui.menu_button("Shape operations", |ui| {
        entries(
            ui,
            pos,
            snarl,
            &[
                ("Union", || Node::Union),
                ("Intersect", || Node::Intersect),
                ("Subtract", || Node::Subtract),
                ("Invert", || Node::Invert),
                ("Expand", || Node::Expand { radius: 1.0 }),
                ("Outline", || Node::Outline {
                    weight: 2.0,
                    inset: 1.0,
                }),
                ("Translate", || Node::Translate { dx: 0.0, dy: 0.0 }),
                ("Rotate", || Node::Rotate {
                    degrees: 0.0,
                    cx: 0.0,
                    cy: 0.0,
                }),
                ("Scale", || Node::Scale {
                    factor: 1.0,
                    cx: 0.0,
                    cy: 0.0,
                }),
                ("Mirror four", || Node::Mirror4 {
                    width: 64.0,
                    height: 64.0,
                }),
                ("Polar array", || Node::PolarArray {
                    count: 6,
                    cx: 32.0,
                    cy: 32.0,
                    phase: 0.0,
                }),
            ],
        )
    });
    ui.menu_button("Field math", |ui| {
        entries(
            ui,
            pos,
            snarl,
            &[
                ("X coordinate", || Node::FieldX),
                ("Y coordinate", || Node::FieldY),
                ("Constant", || Node::FieldConstant { value: 0.0 }),
                ("Add", || Node::FieldAdd),
                ("Subtract", || Node::FieldSubtract),
                ("Multiply", || Node::FieldMultiply),
                ("Divide", || Node::FieldDivide),
                ("Minimum", || Node::FieldMinimum),
                ("Maximum", || Node::FieldMaximum),
                ("Absolute", || Node::FieldAbsolute),
                ("Sine", || Node::FieldSine),
                ("Power", || Node::FieldPower { exponent: 2.0 }),
                ("Clamp", || Node::FieldClamp {
                    low: 0.0,
                    high: 1.0,
                }),
                ("Hypot", || Node::FieldHypot),
                ("Smoothstep", || Node::FieldSmoothstep {
                    edge0: 0.0,
                    edge1: 1.0,
                }),
                ("Select", || Node::FieldSelect),
                ("Height profile", || Node::HeightProfile {
                    values: vec![0.5; 64],
                    crest: 8.0,
                    foot: 48.0,
                }),
            ],
        )
    });
    ui.menu_button("Texture sources", |ui| {
        entries(
            ui,
            pos,
            snarl,
            &[
                ("Value noise", || Node::ValueNoise {
                    size: 0,
                    period: 4,
                    seed: 1,
                }),
                ("Perlin", || Node::Perlin {
                    size: 0,
                    period: 4,
                    seed: 1,
                }),
                ("Worley", || Node::Worley {
                    size: 0,
                    period: 4,
                    seed: 1,
                    feature: GeneratorWorleyFeature::F1,
                    jitter: 1.0,
                }),
                ("FBM", || Node::Fbm {
                    size: 0,
                    seed: 1,
                    octaves: 3,
                    period: 4,
                    source: GeneratorNoiseSource::Perlin,
                    falloff: 0.5,
                }),
                ("Ridged", || Node::Ridged {
                    size: 0,
                    seed: 1,
                    octaves: 3,
                    period: 4,
                    source: GeneratorNoiseSource::Perlin,
                }),
                ("Stripes", || Node::Stripes {
                    size: 0,
                    cycles_x: 0,
                    cycles_y: 1,
                    phase: 0.0,
                }),
                ("Constant grid", || Node::ConstantGrid {
                    size: 0,
                    value: 0.0,
                }),
            ],
        )
    });
    ui.menu_button("Texture operations", |ui| {
        entries(
            ui,
            pos,
            snarl,
            &[
                ("To field", || Node::GridToField),
                ("Normalize", || Node::GridNormalize),
                ("Clamp", || Node::GridClamp {
                    low: 0.0,
                    high: 1.0,
                }),
                ("Gain", || Node::GridGain { power: 1.0 }),
                ("Remap", || Node::GridRemap {
                    low: 0.0,
                    high: 1.0,
                }),
                ("Quantize", || Node::GridQuantize { steps: 4.0 }),
                ("Lerp", || Node::GridLerp { amount: 0.5 }),
                ("Blur", || Node::GridBlur {
                    radius: 1,
                    passes: 1,
                }),
                ("High-pass", || Node::GridHighpass { radius: 4 }),
                ("Warp", || Node::GridWarp { amount: 4.0 }),
                ("Relief", || Node::GridRelief {
                    azimuth: 135.0,
                    strength: 2.0,
                    ambient: 0.55,
                }),
                ("Mask", || Node::GridMask {
                    low: 0.5,
                    high: 1.0,
                    softness: 0.0,
                }),
                ("Add", || Node::GridAdd),
                ("Subtract", || Node::GridSubtract),
                ("Multiply", || Node::GridMultiply),
                ("Scale", || Node::GridScale { factor: 1.0 }),
                ("Offset", || Node::GridOffset { amount: 0.0 }),
                ("Negate", || Node::GridNegate),
                ("Absolute", || Node::GridAbsolute),
            ],
        )
    });
    ui.menu_button("Shaders", |ui| {
        entries(
            ui,
            pos,
            snarl,
            &[
                ("Solid", || Node::Solid {
                    color: [220, 120, 60, 255],
                }),
                ("Vertical", || Node::Vertical {
                    top: [255; 4],
                    bottom: [0, 0, 0, 255],
                    y0: 0.0,
                    y1: 64.0,
                }),
                ("Horizontal", || Node::Horizontal {
                    left: [255; 4],
                    right: [0, 0, 0, 255],
                    x0: 0.0,
                    x1: 64.0,
                }),
                ("Radial", || Node::Radial {
                    cx: 32.0,
                    cy: 32.0,
                    radius: 24.0,
                    inner: [255; 4],
                    outer: [0, 0, 0, 0],
                }),
                ("Elliptical", || Node::Elliptical {
                    cx: 32.0,
                    cy: 32.0,
                    rx: 24.0,
                    ry: 16.0,
                    inner: [255; 4],
                    outer: [0, 0, 0, 0],
                }),
                ("From field", || Node::FromField {
                    low: [0, 0, 0, 255],
                    high: [255; 4],
                    lo: 0.0,
                    hi: 1.0,
                }),
                ("From grid", || Node::FromGrid {
                    low: [30, 30, 40, 255],
                    high: [210, 210, 220, 255],
                    lo: 0.0,
                    hi: 1.0,
                }),
                ("Alpha from field", || Node::AlphaField {
                    color: [255; 4],
                    lo: 0.0,
                    hi: 255.0,
                }),
                ("RGBA fields", || Node::RgbaFields),
            ],
        )
    });
    ui.menu_button("Canvas", |ui| {
        entries(
            ui,
            pos,
            snarl,
            &[
                ("Paint", || Node::Paint {
                    antialias: true,
                    opacity: 1.0,
                }),
                ("Stamp", || Node::Stamp { antialias: false }),
                ("Fill", || Node::Fill),
                ("Modulate", || Node::Modulate),
                ("Output", || Node::Output),
            ],
        )
    });
}

/// Evaluate the graph to `w * h` RGBA8 pixels.
///
/// There must be exactly one `Output` node; its Canvas input is the result. An
/// unconnected `Paint` canvas input starts from a transparent canvas, so a chain
/// of paints composites bottom-up.
pub fn evaluate(snarl: &Snarl<Node>, w: usize, h: usize) -> Result<Vec<u8>, String> {
    evaluate_with_values(snarl, w, h, &KnobValues::new())
}

/// Evaluate with current values for the graph's named parameter nodes.
pub fn evaluate_with_values(
    snarl: &Snarl<Node>,
    w: usize,
    h: usize,
    knobs: &KnobValues,
) -> Result<Vec<u8>, String> {
    let output = validate(snarl)?;
    let mut visiting = HashSet::new();
    let mut cache = HashMap::new();
    match eval(snarl, output, w, h, knobs, &mut visiting, &mut cache)? {
        Value::Canvas(canvas) => Ok(canvas.to_rgba8()),
        _ => Err("the Output node did not produce a canvas".to_owned()),
    }
}

/// Validate graph structure before evaluating any artlib operation.
fn validate(snarl: &Snarl<Node>) -> Result<NodeId, String> {
    let mut output = None;
    let mut knob_ids = HashSet::new();

    for (id, node) in snarl.node_ids() {
        if matches!(node, Node::Output) && output.replace(id).is_some() {
            return Err("more than one Output node".to_owned());
        }

        if let Some(knob_id) = knob_id(node) {
            if knob_id.trim().is_empty() {
                return Err("a parameter node has an empty name".to_owned());
            }
            if !knob_ids.insert(knob_id) {
                return Err(format!("duplicate parameter name `{knob_id}`"));
            }
        }
        validate_node_parameters(node)?;
    }

    let mut connected_inputs = HashSet::new();
    for (from, to) in snarl.wires() {
        let source = snarl
            .get_node(from.node)
            .ok_or_else(|| format!("wire starts at missing node {}", from.node.0))?;
        let target = snarl
            .get_node(to.node)
            .ok_or_else(|| format!("wire ends at missing node {}", to.node.0))?;
        let Some(output_socket) = source.output().filter(|_| from.output == 0) else {
            return Err(format!("{} has no output {}", source.title(), from.output));
        };
        let Some(&input_socket) = target.inputs().get(to.input) else {
            return Err(format!("{} has no input {}", target.title(), to.input));
        };
        if !sockets_compatible(output_socket, input_socket) {
            let label = target.input_label(to.input).unwrap_or("input");
            return Err(format!(
                "cannot connect {output_socket:?} to {}'s {label} ({input_socket:?})",
                target.title()
            ));
        }
        if !connected_inputs.insert(to) {
            let label = target.input_label(to.input).unwrap_or("input");
            return Err(format!(
                "{}'s {label} has more than one wire",
                target.title()
            ));
        }
    }

    output.ok_or_else(|| "no Output node".to_owned())
}

fn sockets_compatible(output: Socket, input: Socket) -> bool {
    output == input || (output == Socket::Grid && input == Socket::Shape)
}

fn knob_id(node: &Node) -> Option<&str> {
    match node {
        Node::FloatKnob { id, .. }
        | Node::IntKnob { id, .. }
        | Node::ColorKnob { id, .. }
        | Node::BoolKnob { id, .. } => Some(id),
        _ => None,
    }
}

fn validate_node_parameters(node: &Node) -> Result<(), String> {
    let finite = |name: &str, values: &[f32]| {
        if values.iter().all(|v| v.is_finite()) {
            Ok(())
        } else {
            Err(format!("{} has a non-finite {name}", node.title()))
        }
    };

    match node {
        Node::Float { value } => finite("value", &[*value])?,
        Node::FloatKnob {
            default, min, max, ..
        } => {
            finite("range", &[*default, *min, *max])?;
            if min > max {
                return Err("number parameter minimum is greater than its maximum".to_owned());
            }
            if default < min || default > max {
                return Err("number parameter default is outside its range".to_owned());
            }
        }
        Node::IntKnob {
            default, min, max, ..
        } => {
            if min > max {
                return Err("integer parameter minimum is greater than its maximum".to_owned());
            }
            if default < min || default > max {
                return Err("integer parameter default is outside its range".to_owned());
            }
        }
        Node::Disk { cx, cy, r } => finite("parameter", &[*cx, *cy, *r])?,
        Node::Rect { x0, y0, x1, y1 } => finite("parameter", &[*x0, *y0, *x1, *y1])?,
        Node::Polyline { points, radius } => {
            finite("radius", &[*radius])?;
            if points.len() < 2 {
                return Err("Polyline needs at least two points".into());
            }
            for point in points {
                finite("point", point)?;
            }
        }
        Node::Polygon { points } => {
            if points.len() < 3 {
                return Err("Polygon needs at least three points".into());
            }
            for point in points {
                finite("point", point)?;
            }
        }
        Node::HeightProfile {
            values,
            crest,
            foot,
        } => {
            if values.is_empty() {
                return Err("Height profile needs at least one sample".into());
            }
            finite("profile", values)?;
            finite("bounds", &[*crest, *foot])?;
        }
        Node::Outline { weight, inset } => finite("parameter", &[*weight, *inset])?,
        Node::Paint { opacity, .. } => finite("opacity", &[*opacity])?,
        _ => {}
    }
    Ok(())
}

/// Evaluate one node's output value, guarding against cycles.
fn eval(
    snarl: &Snarl<Node>,
    node_id: NodeId,
    w: usize,
    h: usize,
    knobs: &KnobValues,
    visiting: &mut HashSet<NodeId>,
    cache: &mut HashMap<NodeId, Value>,
) -> Result<Value, String> {
    if let Some(value) = cache.get(&node_id) {
        return Ok(value.clone());
    }
    if !visiting.insert(node_id) {
        return Err("the graph has a cycle".to_owned());
    }
    let result = eval_node(snarl, node_id, w, h, knobs, visiting, cache);
    visiting.remove(&node_id);
    if let Ok(value) = &result {
        cache.insert(node_id, value.clone());
    }
    result
}

fn eval_node(
    snarl: &Snarl<Node>,
    node_id: NodeId,
    w: usize,
    h: usize,
    knobs: &KnobValues,
    visiting: &mut HashSet<NodeId>,
    cache: &mut HashMap<NodeId, Value>,
) -> Result<Value, String> {
    match &snarl[node_id] {
        Node::Float { value } => Ok(Value::Float(*value)),
        Node::Int { value } => Ok(Value::Int(*value)),
        Node::Bool { value } => Ok(Value::Bool(*value)),
        Node::FloatKnob {
            id,
            default,
            min,
            max,
        } => match knobs.get(id) {
            Some(KnobValue::Float(value)) if value.is_finite() => {
                Ok(Value::Float(value.clamp(*min, *max)))
            }
            Some(KnobValue::Float(_)) => Err(format!("parameter `{id}` is not finite")),
            Some(_) => Err(format!("parameter `{id}` needs a number value")),
            None => Ok(Value::Float(*default)),
        },
        Node::IntKnob {
            id,
            default,
            min,
            max,
        } => match knobs.get(id) {
            Some(KnobValue::Int(value)) => Ok(Value::Int((*value).clamp(*min, *max))),
            Some(_) => Err(format!("parameter `{id}` needs an integer value")),
            None => Ok(Value::Int(*default)),
        },
        Node::ColorKnob { id, default } => match knobs.get(id) {
            Some(KnobValue::Color(value)) => Ok(Value::Color(*value)),
            Some(_) => Err(format!("parameter `{id}` needs a color value")),
            None => Ok(Value::Color(*default)),
        },
        Node::BoolKnob { id, default } => match knobs.get(id) {
            Some(KnobValue::Bool(value)) => Ok(Value::Bool(*value)),
            Some(_) => Err(format!("parameter `{id}` needs a boolean value")),
            None => Ok(Value::Bool(*default)),
        },
        Node::Color { value } => Ok(Value::Color(*value)),
        Node::Disk { cx, cy, r } => {
            let cx = float_input(snarl, node_id, 0, *cx, w, h, knobs, visiting, cache)?;
            let cy = float_input(snarl, node_id, 1, *cy, w, h, knobs, visiting, cache)?;
            let r = float_input(snarl, node_id, 2, *r, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::disk(
                cx as f64, cy as f64, r as f64,
            )))
        }
        Node::Ellipse { cx, cy, rx, ry } => {
            let cx = float_input(snarl, node_id, 0, *cx, w, h, knobs, visiting, cache)?;
            let cy = float_input(snarl, node_id, 1, *cy, w, h, knobs, visiting, cache)?;
            let rx = float_input(snarl, node_id, 2, *rx, w, h, knobs, visiting, cache)?;
            let ry = float_input(snarl, node_id, 3, *ry, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::ellipse(
                cx as f64, cy as f64, rx as f64, ry as f64,
            )))
        }
        Node::Ring {
            cx,
            cy,
            inner,
            outer,
        } => {
            let cx = float_input(snarl, node_id, 0, *cx, w, h, knobs, visiting, cache)?;
            let cy = float_input(snarl, node_id, 1, *cy, w, h, knobs, visiting, cache)?;
            let inner = float_input(snarl, node_id, 2, *inner, w, h, knobs, visiting, cache)?;
            let outer = float_input(snarl, node_id, 3, *outer, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::ring(
                cx as f64,
                cy as f64,
                inner as f64,
                outer as f64,
            )))
        }
        Node::Rect { x0, y0, x1, y1 } => {
            let x0 = float_input(snarl, node_id, 0, *x0, w, h, knobs, visiting, cache)?;
            let y0 = float_input(snarl, node_id, 1, *y0, w, h, knobs, visiting, cache)?;
            let x1 = float_input(snarl, node_id, 2, *x1, w, h, knobs, visiting, cache)?;
            let y1 = float_input(snarl, node_id, 3, *y1, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::rect(
                x0 as f64, y0 as f64, x1 as f64, y1 as f64,
            )))
        }
        Node::HalfPlane { nx, ny, d } => {
            let nx = float_input(snarl, node_id, 0, *nx, w, h, knobs, visiting, cache)?;
            let ny = float_input(snarl, node_id, 1, *ny, w, h, knobs, visiting, cache)?;
            let d = float_input(snarl, node_id, 2, *d, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::half_plane(
                nx as f64, ny as f64, d as f64,
            )))
        }
        Node::Diamond { cx, cy, r } => {
            let cx = float_input(snarl, node_id, 0, *cx, w, h, knobs, visiting, cache)?;
            let cy = float_input(snarl, node_id, 1, *cy, w, h, knobs, visiting, cache)?;
            let r = float_input(snarl, node_id, 2, *r, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::diamond(
                cx as f64, cy as f64, r as f64,
            )))
        }
        Node::Capsule { x0, y0, x1, y1, r } => {
            let x0 = float_input(snarl, node_id, 0, *x0, w, h, knobs, visiting, cache)?;
            let y0 = float_input(snarl, node_id, 1, *y0, w, h, knobs, visiting, cache)?;
            let x1 = float_input(snarl, node_id, 2, *x1, w, h, knobs, visiting, cache)?;
            let y1 = float_input(snarl, node_id, 3, *y1, w, h, knobs, visiting, cache)?;
            let r = float_input(snarl, node_id, 4, *r, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::capsule(
                x0 as f64, y0 as f64, x1 as f64, y1 as f64, r as f64,
            )))
        }
        Node::Sector { cx, cy, from, to } => {
            let cx = float_input(snarl, node_id, 0, *cx, w, h, knobs, visiting, cache)?;
            let cy = float_input(snarl, node_id, 1, *cy, w, h, knobs, visiting, cache)?;
            let from = float_input(snarl, node_id, 2, *from, w, h, knobs, visiting, cache)?;
            let to = float_input(snarl, node_id, 3, *to, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::sector(
                cx as f64,
                cy as f64,
                from as f64,
                to as f64,
            )))
        }
        Node::ChamferedRect {
            x0,
            y0,
            x1,
            y1,
            cut,
        } => {
            let x0 = float_input(snarl, node_id, 0, *x0, w, h, knobs, visiting, cache)?;
            let y0 = float_input(snarl, node_id, 1, *y0, w, h, knobs, visiting, cache)?;
            let x1 = float_input(snarl, node_id, 2, *x1, w, h, knobs, visiting, cache)?;
            let y1 = float_input(snarl, node_id, 3, *y1, w, h, knobs, visiting, cache)?;
            let cut = float_input(snarl, node_id, 4, *cut, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::chamfered_rect(
                x0 as f64, y0 as f64, x1 as f64, y1 as f64, cut as f64,
            )))
        }
        Node::Hexagon {
            cx,
            cy,
            radius,
            flat_top,
        } => {
            let cx = float_input(snarl, node_id, 0, *cx, w, h, knobs, visiting, cache)?;
            let cy = float_input(snarl, node_id, 1, *cy, w, h, knobs, visiting, cache)?;
            let radius = float_input(snarl, node_id, 2, *radius, w, h, knobs, visiting, cache)?;
            let flat_top = bool_input(snarl, node_id, 3, *flat_top, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::hexagon(
                cx as f64,
                cy as f64,
                radius as f64,
                flat_top,
            )))
        }
        Node::Polyline { points, radius } => {
            let radius = float_input(snarl, node_id, 0, *radius, w, h, knobs, visiting, cache)?;
            let points: Vec<_> = points.iter().map(|p| (p[0] as f64, p[1] as f64)).collect();
            Ok(Value::Shape(artlib::fields::polyline(
                &points,
                radius as f64,
            )))
        }
        Node::Polygon { points } => {
            let points: Vec<_> = points.iter().map(|p| (p[0] as f64, p[1] as f64)).collect();
            Ok(Value::Shape(artlib::fields::polygon(&points)))
        }
        Node::Everywhere => Ok(Value::Shape(artlib::fields::everywhere())),
        Node::Perlin { size, period, seed } | Node::ValueNoise { size, period, seed } => {
            let actual_size = int_input(snarl, node_id, 0, *size, w, h, knobs, visiting, cache)?;
            let size = if actual_size <= 0 {
                w.max(h).max(1)
            } else {
                actual_size as usize
            };
            let period = int_input(snarl, node_id, 1, *period, w, h, knobs, visiting, cache)?.max(1)
                as usize;
            let seed = int_input(snarl, node_id, 2, *seed, w, h, knobs, visiting, cache)? as u64;
            let grid = if matches!(&snarl[node_id], Node::Perlin { .. }) {
                texture::perlin(size, period, seed)
            } else {
                texture::value_noise(size, period, seed)
            };
            Ok(Value::Grid(grid))
        }
        Node::Worley {
            size,
            period,
            seed,
            feature,
            jitter,
        } => {
            let size_value = int_input(snarl, node_id, 0, *size, w, h, knobs, visiting, cache)?;
            let size = if size_value <= 0 {
                w.max(h).max(1)
            } else {
                size_value as usize
            };
            let period = int_input(snarl, node_id, 1, *period, w, h, knobs, visiting, cache)?.max(1)
                as usize;
            let seed = int_input(snarl, node_id, 2, *seed, w, h, knobs, visiting, cache)? as u64;
            let jitter =
                float_input(snarl, node_id, 3, *jitter, w, h, knobs, visiting, cache)? as f64;
            let feature = match feature {
                GeneratorWorleyFeature::F1 => texture::Feature::F1,
                GeneratorWorleyFeature::F2 => texture::Feature::F2,
                GeneratorWorleyFeature::F2F1 => texture::Feature::F2F1,
            };
            Ok(Value::Grid(texture::worley_with(
                size, period, seed, feature, jitter,
            )))
        }
        Node::Fbm {
            size,
            seed,
            octaves,
            period,
            source,
            falloff,
        } => {
            let size_value = int_input(snarl, node_id, 0, *size, w, h, knobs, visiting, cache)?;
            let size = if size_value <= 0 {
                w.max(h).max(1)
            } else {
                size_value as usize
            };
            let seed = int_input(snarl, node_id, 1, *seed, w, h, knobs, visiting, cache)? as u64;
            let octaves =
                int_input(snarl, node_id, 2, *octaves, w, h, knobs, visiting, cache)?.max(0) as u32;
            let period = int_input(snarl, node_id, 3, *period, w, h, knobs, visiting, cache)?.max(1)
                as usize;
            let falloff =
                float_input(snarl, node_id, 4, *falloff, w, h, knobs, visiting, cache)? as f64;
            Ok(Value::Grid(texture::fbm(
                size,
                seed,
                octaves,
                period,
                noise_source(*source),
                falloff,
            )))
        }
        Node::Ridged {
            size,
            seed,
            octaves,
            period,
            source,
        } => {
            let size_value = int_input(snarl, node_id, 0, *size, w, h, knobs, visiting, cache)?;
            let size = if size_value <= 0 {
                w.max(h).max(1)
            } else {
                size_value as usize
            };
            let seed = int_input(snarl, node_id, 1, *seed, w, h, knobs, visiting, cache)? as u64;
            let octaves =
                int_input(snarl, node_id, 2, *octaves, w, h, knobs, visiting, cache)?.max(0) as u32;
            let period = int_input(snarl, node_id, 3, *period, w, h, knobs, visiting, cache)?.max(1)
                as usize;
            Ok(Value::Grid(texture::ridged(
                size,
                seed,
                octaves,
                period,
                noise_source(*source),
            )))
        }
        Node::Stripes {
            size,
            cycles_x,
            cycles_y,
            phase,
        } => {
            let size_value = int_input(snarl, node_id, 0, *size, w, h, knobs, visiting, cache)?;
            let size = if size_value <= 0 {
                w.max(h).max(1)
            } else {
                size_value as usize
            };
            let x = int_input(snarl, node_id, 1, *cycles_x, w, h, knobs, visiting, cache)?;
            let y = int_input(snarl, node_id, 2, *cycles_y, w, h, knobs, visiting, cache)?;
            let phase = float_input(snarl, node_id, 3, *phase, w, h, knobs, visiting, cache)?;
            Ok(Value::Grid(texture::stripes(size, x, y, phase as f64)))
        }
        Node::ConstantGrid { size, value } => {
            let size_value = int_input(snarl, node_id, 0, *size, w, h, knobs, visiting, cache)?;
            let size = if size_value <= 0 {
                w.max(h).max(1)
            } else {
                size_value as usize
            };
            let value = float_input(snarl, node_id, 1, *value, w, h, knobs, visiting, cache)?;
            Ok(Value::Grid(texture::constant(size, value as f64)))
        }
        Node::Union | Node::Intersect | Node::Subtract => {
            let a = need_shape(
                input(snarl, node_id, 0, w, h, knobs, visiting, cache)?,
                snarl[node_id].title(),
            )?;
            let b = need_shape(
                input(snarl, node_id, 1, w, h, knobs, visiting, cache)?,
                snarl[node_id].title(),
            )?;
            let shape = match &snarl[node_id] {
                Node::Union => artlib::fields::union(vec![a, b]),
                Node::Intersect => artlib::fields::intersect(vec![a, b]),
                Node::Subtract => artlib::fields::subtract(a, vec![b]),
                _ => unreachable!(),
            };
            Ok(Value::Shape(shape))
        }
        Node::Invert => unary_shape(
            snarl,
            node_id,
            w,
            h,
            knobs,
            visiting,
            cache,
            "Invert",
            artlib::fields::invert,
        ),
        Node::Expand { radius } => {
            let shape = shape_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let r = float_input(snarl, node_id, 1, *radius, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::expand(shape, r as f64)))
        }
        Node::Outline { weight, inset } => {
            let shape = need_shape(
                input(snarl, node_id, 0, w, h, knobs, visiting, cache)?,
                "Outline",
            )?;
            let weight = float_input(snarl, node_id, 1, *weight, w, h, knobs, visiting, cache)?;
            let inset = float_input(snarl, node_id, 2, *inset, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::outline(
                shape,
                weight as f64,
                inset as f64,
            )))
        }
        Node::Translate { dx, dy } => {
            let shape = shape_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let dx = float_input(snarl, node_id, 1, *dx, w, h, knobs, visiting, cache)?;
            let dy = float_input(snarl, node_id, 2, *dy, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::translate(
                shape, dx as f64, dy as f64,
            )))
        }
        Node::Rotate { degrees, cx, cy }
        | Node::Scale {
            factor: degrees,
            cx,
            cy,
        } => {
            let shape = shape_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let amount = float_input(snarl, node_id, 1, *degrees, w, h, knobs, visiting, cache)?;
            let cx = float_input(snarl, node_id, 2, *cx, w, h, knobs, visiting, cache)?;
            let cy = float_input(snarl, node_id, 3, *cy, w, h, knobs, visiting, cache)?;
            let shape = if matches!(&snarl[node_id], Node::Rotate { .. }) {
                artlib::fields::rotate(shape, amount as f64, cx as f64, cy as f64)
            } else {
                artlib::fields::scale(shape, amount as f64, cx as f64, cy as f64)
            };
            Ok(Value::Shape(shape))
        }
        Node::Mirror4 { width, height } => {
            let shape = shape_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let width = float_input(snarl, node_id, 1, *width, w, h, knobs, visiting, cache)?;
            let height = float_input(snarl, node_id, 2, *height, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::mirror4(
                shape,
                width as f64,
                height as f64,
            )))
        }
        Node::PolarArray {
            count,
            cx,
            cy,
            phase,
        } => {
            let shape = shape_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let count =
                int_input(snarl, node_id, 1, *count, w, h, knobs, visiting, cache)?.max(1) as usize;
            let cx = float_input(snarl, node_id, 2, *cx, w, h, knobs, visiting, cache)?;
            let cy = float_input(snarl, node_id, 3, *cy, w, h, knobs, visiting, cache)?;
            let phase = float_input(snarl, node_id, 4, *phase, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::polar_array(
                shape,
                count,
                cx as f64,
                cy as f64,
                phase as f64,
            )))
        }
        Node::FieldX => Ok(Value::Shape(artlib::fields::x())),
        Node::FieldY => Ok(Value::Shape(artlib::fields::y())),
        Node::FieldConstant { value } => {
            let v = float_input(snarl, node_id, 0, *value, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::constant(v as f64)))
        }
        Node::FieldAdd
        | Node::FieldSubtract
        | Node::FieldMultiply
        | Node::FieldDivide
        | Node::FieldMinimum
        | Node::FieldMaximum
        | Node::FieldHypot => {
            let a = shape_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let b = shape_input(snarl, node_id, 1, w, h, knobs, visiting, cache)?;
            let f = match &snarl[node_id] {
                Node::FieldAdd => artlib::fields::add(a, b),
                Node::FieldSubtract => artlib::fields::difference(a, b),
                Node::FieldMultiply => artlib::fields::multiply(a, b),
                Node::FieldDivide => artlib::fields::divide(a, b),
                Node::FieldMinimum => artlib::fields::minimum(a, b),
                Node::FieldMaximum => artlib::fields::maximum(a, b),
                Node::FieldHypot => artlib::fields::hypot(a, b),
                _ => unreachable!(),
            };
            Ok(Value::Shape(f))
        }
        Node::FieldAbsolute | Node::FieldSine => {
            let a = shape_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(
                if matches!(&snarl[node_id], Node::FieldAbsolute) {
                    artlib::fields::absolute(a)
                } else {
                    artlib::fields::sine(a)
                },
            ))
        }
        Node::FieldPower { exponent } => {
            let a = shape_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let exponent = float_input(snarl, node_id, 1, *exponent, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::power(a, exponent as f64)))
        }
        Node::FieldClamp { low, high }
        | Node::FieldSmoothstep {
            edge0: low,
            edge1: high,
        } => {
            let a = shape_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let lo = float_input(snarl, node_id, 1, *low, w, h, knobs, visiting, cache)?;
            let hi = float_input(snarl, node_id, 2, *high, w, h, knobs, visiting, cache)?;
            let field = if matches!(&snarl[node_id], Node::FieldClamp { .. }) {
                artlib::fields::clamp(a, lo as f64, hi as f64)
            } else {
                artlib::fields::smoothstep(a, lo as f64, hi as f64)
            };
            Ok(Value::Shape(field))
        }
        Node::FieldSelect => {
            let condition = shape_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let yes = shape_input(snarl, node_id, 1, w, h, knobs, visiting, cache)?;
            let no = shape_input(snarl, node_id, 2, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::select(condition, yes, no)))
        }
        Node::HeightProfile {
            values,
            crest,
            foot,
        } => {
            let crest = float_input(snarl, node_id, 0, *crest, w, h, knobs, visiting, cache)?;
            let foot = float_input(snarl, node_id, 1, *foot, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::height_profile(
                values.iter().map(|v| *v as f64).collect(),
                crest as f64,
                foot as f64,
            )))
        }
        Node::GridToField => {
            let grid = grid_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(grid.field()))
        }
        Node::GridNormalize | Node::GridNegate | Node::GridAbsolute => {
            let grid = grid_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let out = match &snarl[node_id] {
                Node::GridNormalize => grid.normalize(),
                Node::GridNegate => grid.negate(),
                Node::GridAbsolute => grid.abs(),
                _ => unreachable!(),
            };
            Ok(Value::Grid(out))
        }
        Node::GridClamp { low, high } | Node::GridRemap { low, high } => {
            let grid = grid_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let lo = float_input(snarl, node_id, 1, *low, w, h, knobs, visiting, cache)? as f64;
            let hi = float_input(snarl, node_id, 2, *high, w, h, knobs, visiting, cache)? as f64;
            Ok(Value::Grid(
                if matches!(&snarl[node_id], Node::GridClamp { .. }) {
                    grid.clamp(lo, hi)
                } else {
                    grid.remap(lo, hi)
                },
            ))
        }
        Node::GridGain { power }
        | Node::GridQuantize { steps: power }
        | Node::GridScale { factor: power }
        | Node::GridOffset { amount: power } => {
            let grid = grid_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let amount =
                float_input(snarl, node_id, 1, *power, w, h, knobs, visiting, cache)? as f64;
            let out = match &snarl[node_id] {
                Node::GridGain { .. } => grid.gain(amount),
                Node::GridQuantize { .. } => grid.quantize(amount),
                Node::GridScale { .. } => grid.scale_values(amount),
                Node::GridOffset { .. } => grid.offset(amount),
                _ => unreachable!(),
            };
            Ok(Value::Grid(out))
        }
        Node::GridLerp { amount } => {
            let a = grid_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let b = grid_input(snarl, node_id, 1, w, h, knobs, visiting, cache)?;
            let amount = float_input(snarl, node_id, 2, *amount, w, h, knobs, visiting, cache)?;
            Ok(Value::Grid(a.lerp(&b, amount as f64)))
        }
        Node::GridBlur { radius, passes } => {
            let grid = grid_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let radius =
                int_input(snarl, node_id, 1, *radius, w, h, knobs, visiting, cache)?.max(0);
            let passes =
                int_input(snarl, node_id, 2, *passes, w, h, knobs, visiting, cache)?.max(0);
            Ok(Value::Grid(grid.blur(radius, passes as u32)))
        }
        Node::GridHighpass { radius } => {
            let grid = grid_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let radius =
                int_input(snarl, node_id, 1, *radius, w, h, knobs, visiting, cache)?.max(0);
            Ok(Value::Grid(grid.highpass(radius)))
        }
        Node::GridWarp { amount } => {
            let grid = grid_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let dx = grid_input(snarl, node_id, 1, w, h, knobs, visiting, cache)?;
            let dy = grid_input(snarl, node_id, 2, w, h, knobs, visiting, cache)?;
            let amount = float_input(snarl, node_id, 3, *amount, w, h, knobs, visiting, cache)?;
            Ok(Value::Grid(grid.warp(&dx, &dy, amount as f64)))
        }
        Node::GridRelief {
            azimuth,
            strength,
            ambient,
        } => {
            let grid = grid_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let azimuth = float_input(snarl, node_id, 1, *azimuth, w, h, knobs, visiting, cache)?;
            let strength = float_input(snarl, node_id, 2, *strength, w, h, knobs, visiting, cache)?;
            let ambient = float_input(snarl, node_id, 3, *ambient, w, h, knobs, visiting, cache)?;
            Ok(Value::Grid(grid.relief(
                azimuth as f64,
                strength as f64,
                ambient as f64,
            )))
        }
        Node::GridMask {
            low,
            high,
            softness,
        } => {
            let grid = grid_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let low = float_input(snarl, node_id, 1, *low, w, h, knobs, visiting, cache)?;
            let high = float_input(snarl, node_id, 2, *high, w, h, knobs, visiting, cache)?;
            let softness = float_input(snarl, node_id, 3, *softness, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(grid.mask(
                low as f64,
                high as f64,
                softness as f64,
            )))
        }
        Node::GridAdd | Node::GridSubtract | Node::GridMultiply => {
            let a = grid_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let b = grid_input(snarl, node_id, 1, w, h, knobs, visiting, cache)?;
            let grid = match &snarl[node_id] {
                Node::GridAdd => a.add(&b),
                Node::GridSubtract => a.sub(&b),
                Node::GridMultiply => a.multiply(&b),
                _ => unreachable!(),
            };
            Ok(Value::Grid(grid))
        }
        Node::AlphaColor { alpha } => {
            let color = color_input(snarl, node_id, 0, [255; 4], w, h, knobs, visiting, cache)?;
            let alpha = int_input(snarl, node_id, 1, *alpha, w, h, knobs, visiting, cache)?
                .clamp(0, 255) as u8;
            Ok(Value::Color(raster::alpha(color, alpha)))
        }
        Node::ShadeColor { factor } => {
            let color = color_input(snarl, node_id, 0, [255; 4], w, h, knobs, visiting, cache)?;
            let factor = float_input(snarl, node_id, 1, *factor, w, h, knobs, visiting, cache)?;
            Ok(Value::Color(raster::shade(color, factor as f64)))
        }
        Node::MixColor { amount } => {
            let a = color_input(
                snarl,
                node_id,
                0,
                [0, 0, 0, 255],
                w,
                h,
                knobs,
                visiting,
                cache,
            )?;
            let b = color_input(snarl, node_id, 1, [255; 4], w, h, knobs, visiting, cache)?;
            let amount = float_input(snarl, node_id, 2, *amount, w, h, knobs, visiting, cache)?;
            let mixed = raster::mix(a, b, amount as f64);
            Ok(Value::Color([
                mixed[0] as u8,
                mixed[1] as u8,
                mixed[2] as u8,
                mixed[3] as u8,
            ]))
        }
        Node::Solid { color } => {
            let color = color_input(snarl, node_id, 0, *color, w, h, knobs, visiting, cache)?;
            Ok(Value::Shader(raster::solid(color)))
        }
        Node::Vertical {
            top,
            bottom,
            y0,
            y1,
        }
        | Node::Horizontal {
            left: top,
            right: bottom,
            x0: y0,
            x1: y1,
        } => {
            let a = color_input(snarl, node_id, 0, *top, w, h, knobs, visiting, cache)?;
            let b = color_input(snarl, node_id, 1, *bottom, w, h, knobs, visiting, cache)?;
            let p0 = float_input(snarl, node_id, 2, *y0, w, h, knobs, visiting, cache)?;
            let p1 = float_input(snarl, node_id, 3, *y1, w, h, knobs, visiting, cache)?;
            Ok(Value::Shader(
                if matches!(&snarl[node_id], Node::Vertical { .. }) {
                    raster::vertical(a, b, p0 as f64, p1 as f64)
                } else {
                    raster::horizontal(a, b, p0 as f64, p1 as f64)
                },
            ))
        }
        Node::Radial {
            cx,
            cy,
            radius,
            inner,
            outer,
        } => {
            let cx = float_input(snarl, node_id, 0, *cx, w, h, knobs, visiting, cache)?;
            let cy = float_input(snarl, node_id, 1, *cy, w, h, knobs, visiting, cache)?;
            let radius = float_input(snarl, node_id, 2, *radius, w, h, knobs, visiting, cache)?;
            let inner = color_input(snarl, node_id, 3, *inner, w, h, knobs, visiting, cache)?;
            let outer = color_input(snarl, node_id, 4, *outer, w, h, knobs, visiting, cache)?;
            Ok(Value::Shader(raster::radial(
                cx as f64,
                cy as f64,
                radius as f64,
                inner,
                outer,
            )))
        }
        Node::Elliptical {
            cx,
            cy,
            rx,
            ry,
            inner,
            outer,
        } => {
            let cx = float_input(snarl, node_id, 0, *cx, w, h, knobs, visiting, cache)?;
            let cy = float_input(snarl, node_id, 1, *cy, w, h, knobs, visiting, cache)?;
            let rx = float_input(snarl, node_id, 2, *rx, w, h, knobs, visiting, cache)?;
            let ry = float_input(snarl, node_id, 3, *ry, w, h, knobs, visiting, cache)?;
            let inner = color_input(snarl, node_id, 4, *inner, w, h, knobs, visiting, cache)?;
            let outer = color_input(snarl, node_id, 5, *outer, w, h, knobs, visiting, cache)?;
            Ok(Value::Shader(raster::elliptical(
                cx as f64, cy as f64, rx as f64, ry as f64, inner, outer,
            )))
        }
        Node::FromField { low, high, lo, hi } => {
            let field = shape_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let low = color_input(snarl, node_id, 1, *low, w, h, knobs, visiting, cache)?;
            let high = color_input(snarl, node_id, 2, *high, w, h, knobs, visiting, cache)?;
            let lo = float_input(snarl, node_id, 3, *lo, w, h, knobs, visiting, cache)?;
            let hi = float_input(snarl, node_id, 4, *hi, w, h, knobs, visiting, cache)?;
            Ok(Value::Shader(raster::from_field(
                field, low, high, lo as f64, hi as f64,
            )))
        }
        Node::FromGrid { low, high, lo, hi } => {
            let grid = need_grid(
                input(snarl, node_id, 0, w, h, knobs, visiting, cache)?,
                "From grid",
            )?;
            let low = color_input(snarl, node_id, 1, *low, w, h, knobs, visiting, cache)?;
            let high = color_input(snarl, node_id, 2, *high, w, h, knobs, visiting, cache)?;
            let lo = float_input(snarl, node_id, 3, *lo, w, h, knobs, visiting, cache)?;
            let hi = float_input(snarl, node_id, 4, *hi, w, h, knobs, visiting, cache)?;
            Ok(Value::Shader(raster::from_field(
                grid.field(),
                low,
                high,
                lo as f64,
                hi as f64,
            )))
        }
        Node::AlphaField { color, lo, hi } => {
            let field = shape_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let color = color_input(snarl, node_id, 1, *color, w, h, knobs, visiting, cache)?;
            let lo = float_input(snarl, node_id, 2, *lo, w, h, knobs, visiting, cache)?;
            let hi = float_input(snarl, node_id, 3, *hi, w, h, knobs, visiting, cache)?;
            Ok(Value::Shader(raster::alpha_field(
                field, color, lo as f64, hi as f64,
            )))
        }
        Node::RgbaFields => {
            let red = shape_input(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let green = shape_input(snarl, node_id, 1, w, h, knobs, visiting, cache)?;
            let blue = shape_input(snarl, node_id, 2, w, h, knobs, visiting, cache)?;
            let alpha = shape_input(snarl, node_id, 3, w, h, knobs, visiting, cache)?;
            Ok(Value::Shader(raster::from_channels(
                red, green, blue, alpha,
            )))
        }
        Node::Paint { antialias, opacity } => {
            let mut canvas = match input(snarl, node_id, 0, w, h, knobs, visiting, cache)? {
                Some(Value::Canvas(canvas)) => canvas,
                Some(_) => return Err("Paint's canvas input must be a canvas".to_owned()),
                None => Canvas::new(w, h, raster::CLEAR),
            };
            let shape = need_shape(
                input(snarl, node_id, 1, w, h, knobs, visiting, cache)?,
                "Paint",
            )?;
            let shader = need_shader(
                input(snarl, node_id, 2, w, h, knobs, visiting, cache)?,
                "Paint",
            )?;
            let antialias =
                bool_input(snarl, node_id, 3, *antialias, w, h, knobs, visiting, cache)?;
            let opacity = float_input(snarl, node_id, 4, *opacity, w, h, knobs, visiting, cache)?;
            canvas.paint(&shape, &shader, antialias, opacity.clamp(0.0, 1.0) as f64);
            Ok(Value::Canvas(canvas))
        }
        Node::Stamp { antialias } => {
            let mut canvas =
                canvas_input_or_blank(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let shape = shape_input(snarl, node_id, 1, w, h, knobs, visiting, cache)?;
            let shader = shader_input(snarl, node_id, 2, w, h, knobs, visiting, cache)?;
            let aa = bool_input(snarl, node_id, 3, *antialias, w, h, knobs, visiting, cache)?;
            canvas.stamp(&shape, &shader, aa);
            Ok(Value::Canvas(canvas))
        }
        Node::Fill => {
            let mut canvas =
                canvas_input_or_blank(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let shader = shader_input(snarl, node_id, 1, w, h, knobs, visiting, cache)?;
            canvas.fill(&shader);
            Ok(Value::Canvas(canvas))
        }
        Node::Modulate => {
            let mut canvas =
                canvas_input_or_blank(snarl, node_id, 0, w, h, knobs, visiting, cache)?;
            let factors = shape_input(snarl, node_id, 1, w, h, knobs, visiting, cache)?;
            let restrict = match input(snarl, node_id, 2, w, h, knobs, visiting, cache)? {
                Some(Value::Shape(field)) => Some(field),
                Some(_) => return Err("Modulate restriction must be a shape".into()),
                None => None,
            };
            canvas.modulate(&factors, restrict.as_ref());
            Ok(Value::Canvas(canvas))
        }
        Node::Output => match input(snarl, node_id, 0, w, h, knobs, visiting, cache)? {
            Some(value @ Value::Canvas(_)) => Ok(value),
            Some(_) => Err("Output's input must be a canvas".to_owned()),
            None => Ok(Value::Canvas(Canvas::new(w, h, raster::CLEAR))),
        },
    }
}

/// Evaluate whatever is wired to input `index`, or `None` if it's unconnected.
fn input(
    snarl: &Snarl<Node>,
    node_id: NodeId,
    index: usize,
    w: usize,
    h: usize,
    knobs: &KnobValues,
    visiting: &mut HashSet<NodeId>,
    cache: &mut HashMap<NodeId, Value>,
) -> Result<Option<Value>, String> {
    let pin = snarl.in_pin(InPinId {
        node: node_id,
        input: index,
    });
    match &*pin.remotes {
        [] => Ok(None),
        [out] => Ok(Some(eval(snarl, out.node, w, h, knobs, visiting, cache)?)),
        _ => Err(format!("input {index} has more than one wire")),
    }
}

fn float_input(
    snarl: &Snarl<Node>,
    node: NodeId,
    index: usize,
    fallback: f32,
    w: usize,
    h: usize,
    knobs: &KnobValues,
    visiting: &mut HashSet<NodeId>,
    cache: &mut HashMap<NodeId, Value>,
) -> Result<f32, String> {
    match input(snarl, node, index, w, h, knobs, visiting, cache)? {
        Some(Value::Float(value)) => Ok(value),
        Some(_) => Err(format!("{} needs a number", snarl[node].title())),
        None => Ok(fallback),
    }
}

fn int_input(
    snarl: &Snarl<Node>,
    node: NodeId,
    index: usize,
    fallback: i64,
    w: usize,
    h: usize,
    knobs: &KnobValues,
    visiting: &mut HashSet<NodeId>,
    cache: &mut HashMap<NodeId, Value>,
) -> Result<i64, String> {
    match input(snarl, node, index, w, h, knobs, visiting, cache)? {
        Some(Value::Int(value)) => Ok(value),
        Some(_) => Err(format!("{} needs an integer", snarl[node].title())),
        None => Ok(fallback),
    }
}

fn color_input(
    snarl: &Snarl<Node>,
    node: NodeId,
    index: usize,
    fallback: Rgba,
    w: usize,
    h: usize,
    knobs: &KnobValues,
    visiting: &mut HashSet<NodeId>,
    cache: &mut HashMap<NodeId, Value>,
) -> Result<Rgba, String> {
    match input(snarl, node, index, w, h, knobs, visiting, cache)? {
        Some(Value::Color(value)) => Ok(value),
        Some(_) => Err(format!("{} needs a color", snarl[node].title())),
        None => Ok(fallback),
    }
}

fn bool_input(
    snarl: &Snarl<Node>,
    node: NodeId,
    index: usize,
    fallback: bool,
    w: usize,
    h: usize,
    knobs: &KnobValues,
    visiting: &mut HashSet<NodeId>,
    cache: &mut HashMap<NodeId, Value>,
) -> Result<bool, String> {
    match input(snarl, node, index, w, h, knobs, visiting, cache)? {
        Some(Value::Bool(value)) => Ok(value),
        Some(_) => Err(format!("{} needs a boolean", snarl[node].title())),
        None => Ok(fallback),
    }
}

/// A shape input accepts a shape or a grid (a grid is a field too).
fn need_shape(value: Option<Value>, node: &str) -> Result<Field, String> {
    match value {
        Some(Value::Shape(field)) => Ok(field),
        Some(Value::Grid(grid)) => Ok(grid.field()),
        _ => Err(format!("{node} needs a shape")),
    }
}

fn need_grid(value: Option<Value>, node: &str) -> Result<Grid, String> {
    match value {
        Some(Value::Grid(grid)) => Ok(grid),
        _ => Err(format!("{node} needs a grid")),
    }
}

fn need_shader(value: Option<Value>, node: &str) -> Result<Shader, String> {
    match value {
        Some(Value::Shader(shader)) => Ok(shader),
        _ => Err(format!("{node} needs a shader")),
    }
}

fn noise_source(source: GeneratorNoiseSource) -> texture::NoiseSource {
    match source {
        GeneratorNoiseSource::Value => texture::value_noise,
        GeneratorNoiseSource::Perlin => texture::perlin,
        GeneratorNoiseSource::Worley => texture::worley,
    }
}

fn shape_input(
    snarl: &Snarl<Node>,
    node: NodeId,
    index: usize,
    w: usize,
    h: usize,
    knobs: &KnobValues,
    visiting: &mut HashSet<NodeId>,
    cache: &mut HashMap<NodeId, Value>,
) -> Result<Field, String> {
    need_shape(
        input(snarl, node, index, w, h, knobs, visiting, cache)?,
        snarl[node].title(),
    )
}

fn grid_input(
    snarl: &Snarl<Node>,
    node: NodeId,
    index: usize,
    w: usize,
    h: usize,
    knobs: &KnobValues,
    visiting: &mut HashSet<NodeId>,
    cache: &mut HashMap<NodeId, Value>,
) -> Result<Grid, String> {
    need_grid(
        input(snarl, node, index, w, h, knobs, visiting, cache)?,
        snarl[node].title(),
    )
}

fn shader_input(
    snarl: &Snarl<Node>,
    node: NodeId,
    index: usize,
    w: usize,
    h: usize,
    knobs: &KnobValues,
    visiting: &mut HashSet<NodeId>,
    cache: &mut HashMap<NodeId, Value>,
) -> Result<Shader, String> {
    need_shader(
        input(snarl, node, index, w, h, knobs, visiting, cache)?,
        snarl[node].title(),
    )
}

fn canvas_input_or_blank(
    snarl: &Snarl<Node>,
    node: NodeId,
    index: usize,
    w: usize,
    h: usize,
    knobs: &KnobValues,
    visiting: &mut HashSet<NodeId>,
    cache: &mut HashMap<NodeId, Value>,
) -> Result<Canvas, String> {
    match input(snarl, node, index, w, h, knobs, visiting, cache)? {
        Some(Value::Canvas(canvas)) => Ok(canvas),
        Some(_) => Err(format!("{} needs a canvas", snarl[node].title())),
        None => Ok(Canvas::new(w, h, raster::CLEAR)),
    }
}

fn unary_shape(
    snarl: &Snarl<Node>,
    node: NodeId,
    w: usize,
    h: usize,
    knobs: &KnobValues,
    visiting: &mut HashSet<NodeId>,
    cache: &mut HashMap<NodeId, Value>,
    name: &str,
    operation: fn(Field) -> Field,
) -> Result<Value, String> {
    let shape = need_shape(input(snarl, node, 0, w, h, knobs, visiting, cache)?, name)?;
    Ok(Value::Shape(operation(shape)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_snarl::OutPinId;

    fn pos() -> egui::Pos2 {
        egui::Pos2::ZERO
    }

    fn pixel(px: &[u8], w: usize, x: usize, y: usize) -> [u8; 4] {
        let i = (y * w + x) * 4;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    }

    fn wire(snarl: &mut Snarl<Node>, from: NodeId, output: usize, to: NodeId, input: usize) {
        snarl.connect(OutPinId { node: from, output }, InPinId { node: to, input });
    }

    fn paint() -> Node {
        Node::Paint {
            antialias: true,
            opacity: 1.0,
        }
    }

    #[test]
    fn a_disk_graph_paints_a_disk() {
        // Disk -> Paint.shape, Solid -> Paint.shader, Paint -> Output.
        let mut snarl = Snarl::new();
        let red = [200, 80, 60, 255];
        let disk = snarl.insert_node(
            pos(),
            Node::Disk {
                cx: 32.,
                cy: 32.,
                r: 20.,
            },
        );
        let solid = snarl.insert_node(pos(), Node::Solid { color: red });
        let paint = snarl.insert_node(pos(), paint());
        let output = snarl.insert_node(pos(), Node::Output);

        wire(&mut snarl, disk, 0, paint, 1);
        wire(&mut snarl, solid, 0, paint, 2);
        wire(&mut snarl, paint, 0, output, 0);

        let px = evaluate(&snarl, 64, 64).unwrap();

        // Same engine as the DSL/Python, so this is byte-identical to a direct
        // disk paint: opaque red at the centre, transparent at the corner.
        assert_eq!(pixel(&px, 64, 32, 32), red);
        assert_eq!(pixel(&px, 64, 2, 2), [0, 0, 0, 0]);

        let mut canvas = Canvas::new(64, 64, raster::CLEAR);
        canvas.paint(
            &artlib::fields::disk(32., 32., 20.),
            &raster::solid(red),
            true,
            1.0,
        );
        assert_eq!(
            px,
            canvas.to_rgba8(),
            "graph output must match direct paint"
        );
    }

    #[test]
    fn union_composites_two_shapes() {
        let mut snarl = Snarl::new();
        let gold = [240, 200, 60, 255];
        let a = snarl.insert_node(
            pos(),
            Node::Disk {
                cx: 24.,
                cy: 32.,
                r: 12.,
            },
        );
        let b = snarl.insert_node(
            pos(),
            Node::Disk {
                cx: 40.,
                cy: 32.,
                r: 12.,
            },
        );
        let union = snarl.insert_node(pos(), Node::Union);
        let solid = snarl.insert_node(pos(), Node::Solid { color: gold });
        let paint = snarl.insert_node(pos(), paint());
        let output = snarl.insert_node(pos(), Node::Output);

        wire(&mut snarl, a, 0, union, 0);
        wire(&mut snarl, b, 0, union, 1);
        wire(&mut snarl, union, 0, paint, 1);
        wire(&mut snarl, solid, 0, paint, 2);
        wire(&mut snarl, paint, 0, output, 0);

        let px = evaluate(&snarl, 64, 64).unwrap();
        // Both disk centres are filled.
        assert_eq!(pixel(&px, 64, 24, 32), gold);
        assert_eq!(pixel(&px, 64, 40, 32), gold);
    }

    #[test]
    fn chained_paints_composite() {
        // A blue rect, then a red disk painted over it (Paint feeds Paint).
        let mut snarl = Snarl::new();
        let blue = [70, 110, 180, 255];
        let red = [200, 80, 60, 255];
        let rect = snarl.insert_node(
            pos(),
            Node::Rect {
                x0: 0.,
                y0: 0.,
                x1: 64.,
                y1: 64.,
            },
        );
        let blue_s = snarl.insert_node(pos(), Node::Solid { color: blue });
        let paint1 = snarl.insert_node(pos(), paint());
        let disk = snarl.insert_node(
            pos(),
            Node::Disk {
                cx: 32.,
                cy: 32.,
                r: 10.,
            },
        );
        let red_s = snarl.insert_node(pos(), Node::Solid { color: red });
        let paint2 = snarl.insert_node(pos(), paint());
        let output = snarl.insert_node(pos(), Node::Output);

        wire(&mut snarl, rect, 0, paint1, 1);
        wire(&mut snarl, blue_s, 0, paint1, 2);
        wire(&mut snarl, paint1, 0, paint2, 0); // canvas chain
        wire(&mut snarl, disk, 0, paint2, 1);
        wire(&mut snarl, red_s, 0, paint2, 2);
        wire(&mut snarl, paint2, 0, output, 0);

        let px = evaluate(&snarl, 64, 64).unwrap();
        assert_eq!(pixel(&px, 64, 32, 32), red, "disk on top");
        assert_eq!(pixel(&px, 64, 4, 4), blue, "rect underneath");
    }

    #[test]
    fn a_cycle_is_rejected() {
        let mut snarl = Snarl::new();
        let paint = snarl.insert_node(pos(), paint());
        let output = snarl.insert_node(pos(), Node::Output);
        wire(&mut snarl, paint, 0, output, 0);
        // Feed Paint's canvas input from its own output.
        wire(&mut snarl, paint, 0, paint, 0);

        let result = evaluate(&snarl, 16, 16);
        assert!(result.is_err(), "a cycle must be an error, not a hang");
        assert!(result.unwrap_err().contains("cycle"));
    }

    #[test]
    fn a_graph_without_output_errors() {
        let mut snarl = Snarl::new();
        snarl.insert_node(
            pos(),
            Node::Disk {
                cx: 8.,
                cy: 8.,
                r: 4.,
            },
        );
        assert!(evaluate(&snarl, 16, 16).is_err());
    }

    #[test]
    fn named_parameters_use_defaults_and_supplied_values() {
        let mut snarl = Snarl::new();
        let radius = snarl.insert_node(
            pos(),
            Node::FloatKnob {
                id: "radius".into(),
                default: 4.0,
                min: 1.0,
                max: 20.0,
            },
        );
        let fill = snarl.insert_node(
            pos(),
            Node::ColorKnob {
                id: "fill".into(),
                default: [200, 80, 60, 255],
            },
        );
        let disk = snarl.insert_node(
            pos(),
            Node::Disk {
                cx: 16.,
                cy: 16.,
                r: 1.,
            },
        );
        let solid = snarl.insert_node(
            pos(),
            Node::Solid {
                color: [0, 0, 0, 255],
            },
        );
        let paint = snarl.insert_node(pos(), paint());
        let output = snarl.insert_node(pos(), Node::Output);

        wire(&mut snarl, radius, 0, disk, 2);
        wire(&mut snarl, fill, 0, solid, 0);
        wire(&mut snarl, disk, 0, paint, 1);
        wire(&mut snarl, solid, 0, paint, 2);
        wire(&mut snarl, paint, 0, output, 0);

        let defaults = evaluate(&snarl, 32, 32).unwrap();
        assert_eq!(pixel(&defaults, 32, 16, 16), [200, 80, 60, 255]);
        assert_eq!(pixel(&defaults, 32, 16, 26), [0, 0, 0, 0]);

        let knobs = KnobValues::from([
            ("radius".into(), KnobValue::Float(12.0)),
            ("fill".into(), KnobValue::Color([40, 90, 210, 255])),
        ]);
        let supplied = evaluate_with_values(&snarl, 32, 32, &knobs).unwrap();
        assert_eq!(pixel(&supplied, 32, 16, 26), [40, 90, 210, 255]);
    }

    #[test]
    fn supplied_parameter_values_are_type_checked() {
        let mut snarl = Snarl::new();
        let radius = snarl.insert_node(
            pos(),
            Node::FloatKnob {
                id: "radius".into(),
                default: 4.0,
                min: 1.0,
                max: 20.0,
            },
        );
        let disk = snarl.insert_node(
            pos(),
            Node::Disk {
                cx: 8.,
                cy: 8.,
                r: 4.,
            },
        );
        let solid = snarl.insert_node(
            pos(),
            Node::Solid {
                color: [255, 255, 255, 255],
            },
        );
        let paint = snarl.insert_node(pos(), paint());
        let output = snarl.insert_node(pos(), Node::Output);
        wire(&mut snarl, radius, 0, disk, 2);
        wire(&mut snarl, disk, 0, paint, 1);
        wire(&mut snarl, solid, 0, paint, 2);
        wire(&mut snarl, paint, 0, output, 0);

        let knobs = KnobValues::from([("radius".into(), KnobValue::Bool(true))]);
        assert!(evaluate_with_values(&snarl, 16, 16, &knobs)
            .unwrap_err()
            .contains("needs a number"));
    }

    #[test]
    fn incompatible_connections_are_rejected_before_evaluation() {
        let mut snarl = Snarl::new();
        let solid = snarl.insert_node(
            pos(),
            Node::Solid {
                color: [255, 255, 255, 255],
            },
        );
        let output = snarl.insert_node(pos(), Node::Output);
        wire(&mut snarl, solid, 0, output, 0);

        let error = evaluate(&snarl, 16, 16).unwrap_err();
        assert!(error.contains("cannot connect Shader"));
    }

    #[test]
    fn an_input_cannot_have_multiple_wires() {
        let mut snarl = Snarl::new();
        let a = snarl.insert_node(
            pos(),
            Node::Solid {
                color: [255, 0, 0, 255],
            },
        );
        let b = snarl.insert_node(
            pos(),
            Node::Solid {
                color: [0, 0, 255, 255],
            },
        );
        let disk = snarl.insert_node(
            pos(),
            Node::Disk {
                cx: 8.,
                cy: 8.,
                r: 4.,
            },
        );
        let paint = snarl.insert_node(pos(), paint());
        let output = snarl.insert_node(pos(), Node::Output);
        wire(&mut snarl, disk, 0, paint, 1);
        wire(&mut snarl, a, 0, paint, 2);
        wire(&mut snarl, b, 0, paint, 2);
        wire(&mut snarl, paint, 0, output, 0);

        assert!(evaluate(&snarl, 16, 16)
            .unwrap_err()
            .contains("more than one wire"));
    }

    #[test]
    fn parameter_names_are_unique() {
        let mut snarl = Snarl::new();
        for _ in 0..2 {
            snarl.insert_node(
                pos(),
                Node::BoolKnob {
                    id: "shared".into(),
                    default: true,
                },
            );
        }
        snarl.insert_node(pos(), Node::Output);

        assert!(evaluate(&snarl, 16, 16)
            .unwrap_err()
            .contains("duplicate parameter name"));
    }

    #[test]
    fn every_parameter_type_can_drive_an_operation_input() {
        let mut snarl = Snarl::new();
        let period = snarl.insert_node(
            pos(),
            Node::IntKnob {
                id: "period".into(),
                default: 4,
                min: 1,
                max: 16,
            },
        );
        let seed = snarl.insert_node(
            pos(),
            Node::IntKnob {
                id: "seed".into(),
                default: 7,
                min: 0,
                max: 100,
            },
        );
        let low = snarl.insert_node(
            pos(),
            Node::ColorKnob {
                id: "low".into(),
                default: [20, 30, 40, 255],
            },
        );
        let high = snarl.insert_node(
            pos(),
            Node::ColorKnob {
                id: "high".into(),
                default: [180, 200, 220, 255],
            },
        );
        let aa = snarl.insert_node(
            pos(),
            Node::BoolKnob {
                id: "antialias".into(),
                default: true,
            },
        );
        let opacity = snarl.insert_node(
            pos(),
            Node::FloatKnob {
                id: "opacity".into(),
                default: 1.0,
                min: 0.0,
                max: 1.0,
            },
        );
        let noise = snarl.insert_node(
            pos(),
            Node::Perlin {
                size: 0,
                period: 2,
                seed: 0,
            },
        );
        let shader = snarl.insert_node(
            pos(),
            Node::FromGrid {
                low: [0, 0, 0, 255],
                high: [255, 255, 255, 255],
                lo: 0.0,
                hi: 1.0,
            },
        );
        let rect = snarl.insert_node(
            pos(),
            Node::Rect {
                x0: 0.0,
                y0: 0.0,
                x1: 16.0,
                y1: 16.0,
            },
        );
        let paint = snarl.insert_node(pos(), paint());
        let output = snarl.insert_node(pos(), Node::Output);

        wire(&mut snarl, period, 0, noise, 1);
        wire(&mut snarl, seed, 0, noise, 2);
        wire(&mut snarl, noise, 0, shader, 0);
        wire(&mut snarl, low, 0, shader, 1);
        wire(&mut snarl, high, 0, shader, 2);
        wire(&mut snarl, rect, 0, paint, 1);
        wire(&mut snarl, shader, 0, paint, 2);
        wire(&mut snarl, aa, 0, paint, 3);
        wire(&mut snarl, opacity, 0, paint, 4);
        wire(&mut snarl, paint, 0, output, 0);

        let knobs = KnobValues::from([
            ("period".into(), KnobValue::Int(8)),
            ("seed".into(), KnobValue::Int(11)),
        ]);
        let pixels = evaluate_with_values(&snarl, 16, 16, &knobs).unwrap();
        assert_eq!(pixels.len(), 16 * 16 * 4);
        assert_eq!(pixel(&pixels, 16, 8, 8)[3], 255);
    }

    #[test]
    fn persisted_recipe_round_trip_preserves_evaluation() {
        let recipe = default_recipe();
        let graph = from_recipe(&recipe).unwrap();
        let before = evaluate(&graph, 64, 64).unwrap();
        let restored = from_recipe(&to_recipe(&graph)).unwrap();
        let after = evaluate(&restored, 64, 64).unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn field_math_and_alpha_stamp_express_custom_shaders() {
        let mut snarl = Snarl::new();
        let x = snarl.insert_node(pos(), Node::FieldX);
        let eight_a = snarl.insert_node(pos(), Node::FieldConstant { value: 8.0 });
        let centered = snarl.insert_node(pos(), Node::FieldSubtract);
        let absolute = snarl.insert_node(pos(), Node::FieldAbsolute);
        let eight_b = snarl.insert_node(pos(), Node::FieldConstant { value: 8.0 });
        let fade = snarl.insert_node(pos(), Node::FieldSubtract);
        let alpha = snarl.insert_node(
            pos(),
            Node::AlphaField {
                color: [255; 4],
                lo: 0.0,
                hi: 8.0,
            },
        );
        let everywhere = snarl.insert_node(pos(), Node::Everywhere);
        let stamp = snarl.insert_node(pos(), Node::Stamp { antialias: false });
        let output = snarl.insert_node(pos(), Node::Output);

        wire(&mut snarl, x, 0, centered, 0);
        wire(&mut snarl, eight_a, 0, centered, 1);
        wire(&mut snarl, centered, 0, absolute, 0);
        wire(&mut snarl, eight_b, 0, fade, 0);
        wire(&mut snarl, absolute, 0, fade, 1);
        wire(&mut snarl, fade, 0, alpha, 0);
        wire(&mut snarl, everywhere, 0, stamp, 1);
        wire(&mut snarl, alpha, 0, stamp, 2);
        wire(&mut snarl, stamp, 0, output, 0);

        let pixels = evaluate(&snarl, 16, 16).unwrap();
        assert_eq!(pixel(&pixels, 16, 8, 8), [255, 255, 255, 255]);
        assert_eq!(pixel(&pixels, 16, 0, 8), [0, 0, 0, 0]);
    }

    #[test]
    fn persisted_recipe_rejects_missing_node_references() {
        let recipe = GeneratorGraph {
            nodes: vec![GeneratorGraphNode {
                id: 1,
                position: [0.0, 0.0],
                node: Node::Output,
            }],
            wires: vec![GeneratorGraphWire {
                from_node: 99,
                from_output: 0,
                to_node: 1,
                to_input: 0,
            }],
        };
        assert!(from_recipe(&recipe).unwrap_err().contains("missing node"));
    }
}
