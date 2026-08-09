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
use egui_snarl::{InPinId, NodeId, Snarl};
use std::collections::HashSet;

/// One node: an artlib operation and its own parameters. Inputs and outputs are
/// wires, not stored here. Plain data, so a graph is serializable.
#[derive(Clone, Debug)]
pub enum Node {
    // Shape sources (no inputs).
    Disk { cx: f32, cy: f32, r: f32 },
    Rect { x0: f32, y0: f32, x1: f32, y1: f32 },
    // Noise source (no inputs) → a Grid, which is also a shape.
    Perlin { period: i64, seed: i64 },
    // Shape algebra.
    Union,
    Outline { weight: f32, inset: f32 },
    // Shaders.
    Solid { color: Rgba },
    FromGrid { low: Rgba, high: Rgba },
    // Compositing: paint a shape through a shader onto the canvas.
    Paint,
    // The terminal: its single Canvas input is the finished image.
    Output,
}

/// What a wire carries — used both to type-check the evaluator and (later) to
/// colour the editor's pins and refuse mismatched connections.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Socket {
    Shape,
    Grid,
    Shader,
    Canvas,
}

impl Node {
    /// The label shown on the node.
    pub fn title(&self) -> &'static str {
        match self {
            Node::Disk { .. } => "Disk",
            Node::Rect { .. } => "Rect",
            Node::Perlin { .. } => "Perlin",
            Node::Union => "Union",
            Node::Outline { .. } => "Outline",
            Node::Solid { .. } => "Solid",
            Node::FromGrid { .. } => "From grid",
            Node::Paint => "Paint",
            Node::Output => "Output",
        }
    }

    /// The type of each input socket, in order.
    pub fn inputs(&self) -> &'static [Socket] {
        match self {
            Node::Disk { .. } | Node::Rect { .. } | Node::Perlin { .. } | Node::Solid { .. } => &[],
            Node::Union => &[Socket::Shape, Socket::Shape],
            Node::Outline { .. } => &[Socket::Shape],
            Node::FromGrid { .. } => &[Socket::Grid],
            Node::Paint => &[Socket::Canvas, Socket::Shape, Socket::Shader],
            Node::Output => &[Socket::Canvas],
        }
    }

    /// The type of the output socket, if the node has one.
    pub fn output(&self) -> Option<Socket> {
        match self {
            Node::Disk { .. } | Node::Rect { .. } | Node::Union | Node::Outline { .. } => {
                Some(Socket::Shape)
            }
            Node::Perlin { .. } => Some(Socket::Grid),
            Node::Solid { .. } | Node::FromGrid { .. } => Some(Socket::Shader),
            Node::Paint => Some(Socket::Canvas),
            Node::Output => None,
        }
    }
}

/// A value produced while evaluating the graph.
enum Value {
    Shape(Field),
    Grid(Grid),
    Shader(Shader),
    Canvas(Canvas),
}

/// Evaluate the graph to `w * h` RGBA8 pixels.
///
/// There must be exactly one `Output` node; its Canvas input is the result. An
/// unconnected `Paint` canvas input starts from a transparent canvas, so a chain
/// of paints composites bottom-up.
pub fn evaluate(snarl: &Snarl<Node>, w: usize, h: usize) -> Result<Vec<u8>, String> {
    let mut output = None;
    for id in snarl.node_ids() {
        if matches!(snarl[id], Node::Output) {
            if output.is_some() {
                return Err("more than one Output node".to_owned());
            }
            output = Some(id);
        }
    }
    let output = output.ok_or("no Output node")?;

    let mut visiting = HashSet::new();
    match eval(snarl, output, w, h, &mut visiting)? {
        Value::Canvas(canvas) => Ok(canvas.to_rgba8()),
        _ => Err("the Output node did not produce a canvas".to_owned()),
    }
}

/// Evaluate one node's output value, guarding against cycles.
fn eval(
    snarl: &Snarl<Node>,
    node_id: NodeId,
    w: usize,
    h: usize,
    visiting: &mut HashSet<NodeId>,
) -> Result<Value, String> {
    if !visiting.insert(node_id) {
        return Err("the graph has a cycle".to_owned());
    }
    let result = eval_node(snarl, node_id, w, h, visiting);
    visiting.remove(&node_id);
    result
}

fn eval_node(
    snarl: &Snarl<Node>,
    node_id: NodeId,
    w: usize,
    h: usize,
    visiting: &mut HashSet<NodeId>,
) -> Result<Value, String> {
    match &snarl[node_id] {
        Node::Disk { cx, cy, r } => Ok(Value::Shape(artlib::fields::disk(
            *cx as f64, *cy as f64, *r as f64,
        ))),
        Node::Rect { x0, y0, x1, y1 } => Ok(Value::Shape(artlib::fields::rect(
            *x0 as f64, *y0 as f64, *x1 as f64, *y1 as f64,
        ))),
        Node::Perlin { period, seed } => {
            // Noise is square; the largest side covers a non-square canvas, and
            // sampling wraps so it tiles.
            let size = w.max(h).max(1);
            Ok(Value::Grid(texture::perlin(
                size,
                (*period).max(1) as usize,
                *seed as u64,
            )))
        }
        Node::Union => {
            let a = need_shape(input(snarl, node_id, 0, w, h, visiting)?, "Union")?;
            let b = need_shape(input(snarl, node_id, 1, w, h, visiting)?, "Union")?;
            Ok(Value::Shape(artlib::fields::union(vec![a, b])))
        }
        Node::Outline { weight, inset } => {
            let shape = need_shape(input(snarl, node_id, 0, w, h, visiting)?, "Outline")?;
            Ok(Value::Shape(artlib::fields::outline(
                shape,
                *weight as f64,
                *inset as f64,
            )))
        }
        Node::Solid { color } => Ok(Value::Shader(raster::solid(*color))),
        Node::FromGrid { low, high } => {
            let grid = need_grid(input(snarl, node_id, 0, w, h, visiting)?, "From grid")?;
            Ok(Value::Shader(raster::from_field(
                grid.field(),
                *low,
                *high,
                0.0,
                1.0,
            )))
        }
        Node::Paint => {
            let mut canvas = match input(snarl, node_id, 0, w, h, visiting)? {
                Some(Value::Canvas(canvas)) => canvas,
                Some(_) => return Err("Paint's canvas input must be a canvas".to_owned()),
                None => Canvas::new(w, h, raster::CLEAR),
            };
            let shape = need_shape(input(snarl, node_id, 1, w, h, visiting)?, "Paint")?;
            let shader = need_shader(input(snarl, node_id, 2, w, h, visiting)?, "Paint")?;
            canvas.paint(&shape, &shader, true, 1.0);
            Ok(Value::Canvas(canvas))
        }
        Node::Output => match input(snarl, node_id, 0, w, h, visiting)? {
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
    visiting: &mut HashSet<NodeId>,
) -> Result<Option<Value>, String> {
    let pin = snarl.in_pin(InPinId {
        node: node_id,
        input: index,
    });
    match pin.remotes.first() {
        Some(out) => Ok(Some(eval(snarl, out.node, w, h, visiting)?)),
        None => Ok(None),
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

    #[test]
    fn a_disk_graph_paints_a_disk() {
        // Disk -> Paint.shape, Solid -> Paint.shader, Paint -> Output.
        let mut snarl = Snarl::new();
        let red = [200, 80, 60, 255];
        let disk = snarl.insert_node(pos(), Node::Disk { cx: 32., cy: 32., r: 20. });
        let solid = snarl.insert_node(pos(), Node::Solid { color: red });
        let paint = snarl.insert_node(pos(), Node::Paint);
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
        assert_eq!(px, canvas.to_rgba8(), "graph output must match direct paint");
    }

    #[test]
    fn union_composites_two_shapes() {
        let mut snarl = Snarl::new();
        let gold = [240, 200, 60, 255];
        let a = snarl.insert_node(pos(), Node::Disk { cx: 24., cy: 32., r: 12. });
        let b = snarl.insert_node(pos(), Node::Disk { cx: 40., cy: 32., r: 12. });
        let union = snarl.insert_node(pos(), Node::Union);
        let solid = snarl.insert_node(pos(), Node::Solid { color: gold });
        let paint = snarl.insert_node(pos(), Node::Paint);
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
        let rect = snarl.insert_node(pos(), Node::Rect { x0: 0., y0: 0., x1: 64., y1: 64. });
        let blue_s = snarl.insert_node(pos(), Node::Solid { color: blue });
        let paint1 = snarl.insert_node(pos(), Node::Paint);
        let disk = snarl.insert_node(pos(), Node::Disk { cx: 32., cy: 32., r: 10. });
        let red_s = snarl.insert_node(pos(), Node::Solid { color: red });
        let paint2 = snarl.insert_node(pos(), Node::Paint);
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
        let paint = snarl.insert_node(pos(), Node::Paint);
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
        snarl.insert_node(pos(), Node::Disk { cx: 8., cy: 8., r: 4. });
        assert!(evaluate(&snarl, 16, 16).is_err());
    }
}
