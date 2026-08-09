//! The DSL proves artlib works from text: each script here is the golden scene
//! from `../artlib/tests/gen_golden.py` written in the Rune DSL, and its output
//! must match the same golden PNG byte-for-byte.
//!
//! Since the DSL calls the exact artlib functions the golden images were built
//! from, and those are already pixel-identical to Python, the bar here is
//! *exact* — a difference means the DSL binding mistranslated something.
//!
//! (`modulate` is the one golden scene omitted: it uses an arbitrary per-pixel
//! `lambda x, y: ...` factor, which the DSL has no primitive for yet — the
//! Rust-side parity test already covers `modulate` itself.)

const SIZE: usize = 64;

/// The DSL source that reproduces golden scene `name`.
fn script(name: &str) -> String {
    let body = match name {
        "disk" => "c.paint(disk(32.0,32.0,20.0), solid(rgb(200,80,60)));",
        "rect" => "c.paint(rect(8.0,8.0,56.0,40.0), solid(rgb(70,110,180)));",
        "ellipse" => "c.paint(ellipse(32.0,32.0,26.0,14.0), solid(rgb(80,200,210)));",
        "chamfer" => {
            "let plate = chamfered_rect(0.0,0.0,64.0,64.0,9.0);
             c.paint(plate, vertical(rgb(150,160,175), rgb(60,66,80), 0.0, 63.0));
             c.paint(outline(plate,1.0,1.0), solid(rgb(25,28,36)));"
        }
        "hexagon" => {
            "let hx = hexagon(32.0,32.0,26.0,false);
             c.paint(hx, solid(rgb(80,200,210)));
             c.paint(outline(hx,1.0,1.0), solid(rgb(20,20,28)));"
        }
        "ring_sector" => {
            "c.paint(intersect([ring(32.0,32.0,16.0,26.0), sector(32.0,32.0,45.0,300.0)]), solid(rgb(240,200,60)));"
        }
        "polyline" => {
            "c.paint(polyline([(10.0,12.0),(52.0,20.0),(28.0,54.0)], 4.0), solid(rgb(200,70,160)));"
        }
        "polygon" => {
            "c.paint(polygon([(32.0,6.0),(58.0,52.0),(6.0,52.0)]), solid(rgb(70,110,180)));"
        }
        "diamond" => "c.paint(diamond(32.0,32.0,24.0), solid(rgb(80,200,210)));",
        "subtract" => {
            "c.paint(subtract(disk(32.0,32.0,24.0), [disk(40.0,26.0,12.0)]), solid(rgb(200,80,60)));"
        }
        "union_expand" => {
            "let blob = union([disk(24.0,32.0,12.0), disk(40.0,32.0,12.0)]);
             c.paint(expand(blob,3.0), solid(rgb(70,110,180)));
             c.paint(blob, solid(rgb(240,200,60)));"
        }
        "polar" => {
            "c.paint(polar_array(capsule(32.0,8.0,32.0,26.0,3.0), 6, 32.0, 32.0, 0.0), solid(rgb(20,20,28)));"
        }
        "mirror4" => "c.paint(mirror4(disk(12.0,12.0,7.0),64.0,64.0), solid(rgb(240,200,60)));",
        "rotate" => {
            "c.paint(rotate(rect(20.0,28.0,44.0,36.0),30.0,32.0,32.0), solid(rgb(80,200,210)));"
        }
        "scale" => {
            "c.paint(scale(rect(24.0,24.0,40.0,40.0),1.5,32.0,32.0), solid(rgb(200,70,160)));"
        }
        "blend" => {
            "c.fill(solid(rgb(30,30,40)));
             c.paint(disk(32.0,32.0,24.0), solid(rgba(255,200,0,128)));"
        }
        "radial" => {
            "c.paint(disk(32.0,32.0,30.0), radial(32.0,32.0,30.0, rgb(240,200,60), rgb(25,28,36)));"
        }
        "horizontal" => {
            "c.paint(rect(4.0,4.0,60.0,60.0), horizontal(rgb(200,80,60), rgb(70,110,180), 0.0, 59.0));"
        }
        "from_field" => {
            "let d = disk(32.0,32.0,30.0);
             c.paint(d, from_field(d, rgb(80,200,210), rgb(20,20,28), -28.0, 4.0));"
        }
        "stamp" => {
            "c.fill(solid(rgb(10,20,30)));
             c.stamp(disk(32.0,32.0,20.0), solid(rgba(100,150,200,120)));"
        }
        other => panic!("unknown scene {other}"),
    };
    // `p` is the knob object every script's main receives; these scenes declare
    // no knobs and ignore it.
    format!("pub fn main(w, h, p) {{ let c = Canvas::new(w, h); {body} c }}")
}

const SCENES: &[&str] = &[
    "disk",
    "rect",
    "ellipse",
    "chamfer",
    "hexagon",
    "ring_sector",
    "polyline",
    "polygon",
    "diamond",
    "subtract",
    "union_expand",
    "polar",
    "mirror4",
    "rotate",
    "scale",
    "blend",
    "radial",
    "horizontal",
    "from_field",
    "stamp",
];

fn golden(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../artlib/tests/golden/{}.png",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    image::open(&path)
        .unwrap_or_else(|e| panic!("open golden {path}: {e} (run: python artlib/tests/gen_golden.py)"))
        .to_rgba8()
        .into_raw()
}

fn max_channel_diff(a: &[u8], b: &[u8]) -> i32 {
    assert_eq!(a.len(), b.len(), "size mismatch");
    a.iter()
        .zip(b)
        .map(|(x, y)| (*x as i32 - *y as i32).abs())
        .max()
        .unwrap_or(0)
}

#[test]
fn dsl_scenes_match_golden() {
    // Run once to warm up (compilation), then time nothing — just correctness.
    let mut failures = Vec::new();
    for &name in SCENES {
        let px = artlib_script::run_script(&script(name), SIZE, SIZE)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let d = max_channel_diff(&px, &golden(name));
        println!("{name:>14}: max channel diff {d}  {}", if d == 0 { "exact" } else { "!!" });
        if d != 0 {
            failures.push(format!("{name}: max channel diff {d}"));
        }
    }
    assert!(failures.is_empty(), "DSL parity failures:\n{}", failures.join("\n"));
}
