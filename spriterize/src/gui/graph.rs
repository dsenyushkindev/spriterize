//! The generator node graph: a visual front-end onto the same artlib calls as
//! the DSL.
//!
//! A graph is a set of nodes wired together; each node is an artlib operation and
//! each wire carries an artlib value (a shape [`Field`], a noise [`Grid`], a
//! [`Shader`], or an in-progress [`Canvas`]). Evaluating from the single `Output`
//! node produces the layer's pixels — the same result a script produces, through
//! the same engine.
//!
//! This module is the model and the evaluator. The nodes are plain data (so the
//! graph can be serialized later), and evaluation turns that data into artlib
//! values on demand. The interactive editor (a `SnarlViewer`) and the per-layer
//! window are built on top of this.

use artlib::fields::Field;
use artlib::raster::{self, Canvas, Rgba, Shader};
use artlib::texture::{self, Grid};
use egui_snarl::ui::{PinInfo, SnarlViewer};
use egui_snarl::{InPin, InPinId, NodeId, OutPin, Snarl};
use lapix::{GeneratorGraph, GeneratorGraphNode, GeneratorGraphWire, GeneratorNode as Node};
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
    /// The label shown on the node.
    fn title(&self) -> &'static str {
        match self {
            Node::FloatKnob { .. } => "Number parameter",
            Node::IntKnob { .. } => "Integer parameter",
            Node::ColorKnob { .. } => "Color parameter",
            Node::BoolKnob { .. } => "Boolean parameter",
            Node::Disk { .. } => "Disk",
            Node::Rect { .. } => "Rect",
            Node::Perlin { .. } => "Perlin",
            Node::Union => "Union",
            Node::Outline { .. } => "Outline",
            Node::Solid { .. } => "Solid",
            Node::FromGrid { .. } => "From grid",
            Node::Paint { .. } => "Paint",
            Node::Output => "Output",
        }
    }

    /// The type of each input socket, in order.
    fn inputs(&self) -> &'static [Socket] {
        match self {
            Node::FloatKnob { .. }
            | Node::IntKnob { .. }
            | Node::ColorKnob { .. }
            | Node::BoolKnob { .. } => &[],
            Node::Disk { .. } => &[Socket::Float, Socket::Float, Socket::Float],
            Node::Rect { .. } => &[Socket::Float, Socket::Float, Socket::Float, Socket::Float],
            Node::Perlin { .. } => &[Socket::Int, Socket::Int],
            Node::Union => &[Socket::Shape, Socket::Shape],
            Node::Outline { .. } => &[Socket::Shape, Socket::Float, Socket::Float],
            Node::Solid { .. } => &[Socket::Color],
            Node::FromGrid { .. } => &[Socket::Grid, Socket::Color, Socket::Color],
            Node::Paint { .. } => &[
                Socket::Canvas,
                Socket::Shape,
                Socket::Shader,
                Socket::Bool,
                Socket::Float,
            ],
            Node::Output => &[Socket::Canvas],
        }
    }

    /// The label of an input socket, in the same order as [`Node::inputs`].
    fn input_label(&self, index: usize) -> Option<&'static str> {
        let labels: &[&str] = match self {
            Node::FloatKnob { .. }
            | Node::IntKnob { .. }
            | Node::ColorKnob { .. }
            | Node::BoolKnob { .. } => &[],
            Node::Disk { .. } => &["center x", "center y", "radius"],
            Node::Rect { .. } => &["left", "top", "right", "bottom"],
            Node::Perlin { .. } => &["period", "seed"],
            Node::Union => &["a", "b"],
            Node::Outline { .. } => &["shape", "weight", "inset"],
            Node::Solid { .. } => &["color"],
            Node::FromGrid { .. } => &["grid", "low", "high"],
            Node::Paint { .. } => &["canvas", "shape", "shader", "antialias", "opacity"],
            Node::Output => &["canvas"],
        };
        labels.get(index).copied()
    }

    /// The type of the output socket, if the node has one.
    fn output(&self) -> Option<Socket> {
        match self {
            Node::FloatKnob { .. } => Some(Socket::Float),
            Node::IntKnob { .. } => Some(Socket::Int),
            Node::ColorKnob { .. } => Some(Socket::Color),
            Node::BoolKnob { .. } => Some(Socket::Bool),
            Node::Disk { .. } | Node::Rect { .. } | Node::Union | Node::Outline { .. } => {
                Some(Socket::Shape)
            }
            Node::Perlin { .. } => Some(Socket::Grid),
            Node::Solid { .. } | Node::FromGrid { .. } => Some(Socket::Shader),
            Node::Paint { .. } => Some(Socket::Canvas),
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
        Node::Perlin { period, seed } => {
            ui.add(egui::DragValue::new(period).prefix("period "));
            ui.add(egui::DragValue::new(seed).prefix("seed "));
        }
        Node::Outline { weight, inset } => {
            ui.add(egui::DragValue::new(weight).prefix("weight "));
            ui.add(egui::DragValue::new(inset).prefix("inset "));
        }
        Node::Solid { color } => {
            ui.color_edit_button_srgba_unmultiplied(color);
        }
        Node::FromGrid { low, high } => {
            ui.label("low");
            ui.color_edit_button_srgba_unmultiplied(low);
            ui.label("high");
            ui.color_edit_button_srgba_unmultiplied(high);
        }
        Node::Paint { antialias, opacity } => {
            ui.checkbox(antialias, "antialias");
            ui.add(egui::Slider::new(opacity, 0.0..=1.0).text("opacity"));
        }
        Node::Union | Node::Output => {}
    }
}

fn add_node_menu(pos: egui::Pos2, ui: &mut egui::Ui, snarl: &mut Snarl<Node>) {
    type NodeFactory = fn() -> Node;
    let entries: &[(&str, NodeFactory)] = &[
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
        ("Disk", || Node::Disk {
            cx: 32.0,
            cy: 32.0,
            r: 16.0,
        }),
        ("Rectangle", || Node::Rect {
            x0: 8.0,
            y0: 8.0,
            x1: 56.0,
            y1: 56.0,
        }),
        ("Perlin", || Node::Perlin { period: 4, seed: 1 }),
        ("Union", || Node::Union),
        ("Outline", || Node::Outline {
            weight: 2.0,
            inset: 1.0,
        }),
        ("Solid", || Node::Solid {
            color: [220, 120, 60, 255],
        }),
        ("From grid", || Node::FromGrid {
            low: [30, 30, 40, 255],
            high: [210, 210, 220, 255],
        }),
        ("Paint", || Node::Paint {
            antialias: true,
            opacity: 1.0,
        }),
        ("Output", || Node::Output),
    ];
    for (label, make) in entries {
        if ui.button(*label).clicked() {
            snarl.insert_node(pos, make());
            ui.close_menu();
        }
    }
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
        Node::Disk { cx, cy, r } => {
            let cx = float_input(snarl, node_id, 0, *cx, w, h, knobs, visiting, cache)?;
            let cy = float_input(snarl, node_id, 1, *cy, w, h, knobs, visiting, cache)?;
            let r = float_input(snarl, node_id, 2, *r, w, h, knobs, visiting, cache)?;
            Ok(Value::Shape(artlib::fields::disk(
                cx as f64, cy as f64, r as f64,
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
        Node::Perlin { period, seed } => {
            // Noise is square; the largest side covers a non-square canvas, and
            // sampling wraps so it tiles.
            let size = w.max(h).max(1);
            let period = int_input(snarl, node_id, 0, *period, w, h, knobs, visiting, cache)?;
            let seed = int_input(snarl, node_id, 1, *seed, w, h, knobs, visiting, cache)?;
            Ok(Value::Grid(texture::perlin(
                size,
                period.max(1) as usize,
                seed as u64,
            )))
        }
        Node::Union => {
            let a = need_shape(
                input(snarl, node_id, 0, w, h, knobs, visiting, cache)?,
                "Union",
            )?;
            let b = need_shape(
                input(snarl, node_id, 1, w, h, knobs, visiting, cache)?,
                "Union",
            )?;
            Ok(Value::Shape(artlib::fields::union(vec![a, b])))
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
        Node::Solid { color } => {
            let color = color_input(snarl, node_id, 0, *color, w, h, knobs, visiting, cache)?;
            Ok(Value::Shader(raster::solid(color)))
        }
        Node::FromGrid { low, high } => {
            let grid = need_grid(
                input(snarl, node_id, 0, w, h, knobs, visiting, cache)?,
                "From grid",
            )?;
            let low = color_input(snarl, node_id, 1, *low, w, h, knobs, visiting, cache)?;
            let high = color_input(snarl, node_id, 2, *high, w, h, knobs, visiting, cache)?;
            Ok(Value::Shader(raster::from_field(
                grid.field(),
                low,
                high,
                0.0,
                1.0,
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
        let noise = snarl.insert_node(pos(), Node::Perlin { period: 2, seed: 0 });
        let shader = snarl.insert_node(
            pos(),
            Node::FromGrid {
                low: [0, 0, 0, 255],
                high: [255, 255, 255, 255],
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

        wire(&mut snarl, period, 0, noise, 0);
        wire(&mut snarl, seed, 0, noise, 1);
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
