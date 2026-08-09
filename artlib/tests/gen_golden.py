"""Render the deterministic parity scenes with the REAL Python artlib.

These PNGs are the oracle the Rust port is checked against: `tests/parity.rs`
rebuilds each scene with the Rust artlib and asserts the bytes match. Only
deterministic features live here (shapes, algebra, transforms, shaders,
compositing) — noise is exempt from byte-parity and is checked for its
properties instead, in `tests/noise.rs`.

Run from anywhere:  python artlib/tests/gen_golden.py
It writes artlib/tests/golden/*.png. Keep the scene list identical to the Rust
side, name for name.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "..", "artlib_sample"))
GOLDEN = os.path.join(HERE, "golden")

from artlib import (  # noqa: E402
    Canvas,
    disk, ellipse, rect, ring, diamond, capsule, polyline, polygon, sector,
    chamfered_rect, hexagon,
    union, intersect, subtract, outline, expand,
    rotate, scale, mirror4, polar_array,
    solid, vertical, horizontal, radial, from_field,
)

SIZE = 64

# Palette shared with the Rust side, name for name.
RED = (200, 80, 60)
BLUE = (70, 110, 180)
LIT = (150, 160, 175)
DIM = (60, 66, 80)
SHADOW = (25, 28, 36)
GOLD = (240, 200, 60)
INK = (20, 20, 28)
CYAN = (80, 200, 210)
MAG = (200, 70, 160)


def scene_disk():
    c = Canvas(SIZE)
    c.paint(disk(32, 32, 20), solid(RED))
    return c


def scene_rect():
    c = Canvas(SIZE)
    c.paint(rect(8, 8, 56, 40), solid(BLUE))
    return c


def scene_ellipse():
    c = Canvas(SIZE)
    c.paint(ellipse(32, 32, 26, 14), solid(CYAN))
    return c


def scene_chamfer():
    c = Canvas(SIZE)
    plate = chamfered_rect(0, 0, 64, 64, cut=9)
    c.paint(plate, vertical(LIT, DIM, y1=63))
    c.paint(outline(plate, weight=1, inset=1), solid(SHADOW))
    return c


def scene_hexagon():
    c = Canvas(SIZE)
    hx = hexagon(32, 32, 26)
    c.paint(hx, solid(CYAN))
    c.paint(outline(hx, weight=1, inset=1), solid(INK))
    return c


def scene_ring_sector():
    c = Canvas(SIZE)
    r = ring(32, 32, 16, 26)
    s = sector(32, 32, 45, 300)
    c.paint(intersect(r, s), solid(GOLD))
    return c


def scene_polyline():
    c = Canvas(SIZE)
    c.paint(polyline([(10, 12), (52, 20), (28, 54)], radius=4), solid(MAG))
    return c


def scene_polygon():
    c = Canvas(SIZE)
    c.paint(polygon([(32, 6), (58, 52), (6, 52)]), solid(BLUE))
    return c


def scene_diamond():
    c = Canvas(SIZE)
    c.paint(diamond(32, 32, 24), solid(CYAN))
    return c


def scene_subtract():
    c = Canvas(SIZE)
    c.paint(subtract(disk(32, 32, 24), disk(40, 26, 12)), solid(RED))
    return c


def scene_union_expand():
    c = Canvas(SIZE)
    blob = union(disk(24, 32, 12), disk(40, 32, 12))
    c.paint(expand(blob, 3), solid(BLUE))
    c.paint(blob, solid(GOLD))
    return c


def scene_polar():
    c = Canvas(SIZE)
    c.paint(polar_array(capsule(32, 8, 32, 26, 3), 6, 32, 32), solid(INK))
    return c


def scene_mirror4():
    c = Canvas(SIZE)
    c.paint(mirror4(disk(12, 12, 7), 64, 64), solid(GOLD))
    return c


def scene_rotate():
    c = Canvas(SIZE)
    c.paint(rotate(rect(20, 28, 44, 36), 30, 32, 32), solid(CYAN))
    return c


def scene_scale():
    c = Canvas(SIZE)
    c.paint(scale(rect(24, 24, 40, 40), 1.5, 32, 32), solid(MAG))
    return c


def scene_blend():
    c = Canvas(SIZE)
    c.fill((30, 30, 40))
    c.paint(disk(32, 32, 24), solid((255, 200, 0, 128)))
    return c


def scene_radial():
    c = Canvas(SIZE)
    c.paint(disk(32, 32, 30), radial(32, 32, 30, GOLD, SHADOW))
    return c


def scene_horizontal():
    c = Canvas(SIZE)
    c.paint(rect(4, 4, 60, 60), horizontal(RED, BLUE, x1=59))
    return c


def scene_from_field():
    c = Canvas(SIZE)
    d = disk(32, 32, 30)
    c.paint(d, from_field(d, CYAN, INK, lo=-28, hi=4))
    return c


def scene_modulate():
    c = Canvas(SIZE)
    plate = rect(6, 6, 58, 58)
    c.paint(plate, solid(BLUE))
    c.modulate(lambda x, y: 0.4 + 0.9 * x / 63.0)
    return c


def scene_stamp():
    c = Canvas(SIZE)
    c.fill((10, 20, 30))
    c.stamp(disk(32, 32, 20), solid((100, 150, 200, 120)))
    return c


SCENES = {
    "disk": scene_disk,
    "rect": scene_rect,
    "ellipse": scene_ellipse,
    "chamfer": scene_chamfer,
    "hexagon": scene_hexagon,
    "ring_sector": scene_ring_sector,
    "polyline": scene_polyline,
    "polygon": scene_polygon,
    "diamond": scene_diamond,
    "subtract": scene_subtract,
    "union_expand": scene_union_expand,
    "polar": scene_polar,
    "mirror4": scene_mirror4,
    "rotate": scene_rotate,
    "scale": scene_scale,
    "blend": scene_blend,
    "radial": scene_radial,
    "horizontal": scene_horizontal,
    "from_field": scene_from_field,
    "modulate": scene_modulate,
    "stamp": scene_stamp,
}


def main():
    os.makedirs(GOLDEN, exist_ok=True)
    for name, build in SCENES.items():
        build().save(os.path.join(GOLDEN, name + ".png"))
        print("wrote", name + ".png")
    print(f"{len(SCENES)} golden scenes in {GOLDEN}")


if __name__ == "__main__":
    main()
