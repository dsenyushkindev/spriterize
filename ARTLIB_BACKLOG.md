# artlib vocabulary backlog

The Rust port of `artlib` (the `artlib` crate) and its Rune DSL (the `artlib-script`
crate) are **capability-complete**: every field primitive, algebra op, transform,
noise source, shader and compositing operation is present in both, and the
deterministic half is byte-parity-verified against the Python original (see
`artlib/tests/parity.rs`, 21 golden scenes; 20 reproduced from the DSL in
`artlib-script/tests/parity.rs`).

What remains is a short list of **additive** convenience/parameter items — each is
a few extra function registrations or trait impls, no architecture change, and
each can land anytime (including after the graph editor, which is just a second
front-end onto the same calls). This file is the running list.

Status legend: **[gap]** missing capability · **[conv]** convenience/ergonomics ·
**[design]** needs a small design decision · **[wontfix]** deliberate omission.

---

## Engine — `artlib` crate

### [gap] Grid × Grid multiply (and the other Grid operators)
`artlib/src/texture.rs` implements only `impl Mul<f64> for Grid` and
`impl Add<Grid> for Grid`. Python's `Grid` also has `__mul__` with a Grid
(elementwise), `__sub__` (Grid and scalar), `__neg__`, `__abs__`, `__add__` with a
scalar, and `__rmul__`/`__radd__` (scalar-on-left).
- The real capability gap is **Grid × Grid** (multiplicative blend of two surfaces,
  e.g. `form * crack`).
- The rest (`Sub`, `Neg`, `Abs`, scalar `Add`, scalar-on-left) are conveniences.
- **Do:** add the missing `std::ops` impls beside the existing two. Elementwise,
  assert equal sizes, mirror the existing pattern.
- **Workaround today:** additive blends compose (`a*0.7` then `.add(b*0.3)`).

### [wontfix] PNG in/out (`write_png` / `read_png`)
Python's `raster.py` hand-rolls PNG encode/decode with `zlib`. Not ported: the
editor owns all pixel and PNG handling (`spriterize` via the `image` crate and
`WrappedImage`), and the `artlib` core is deliberately dependency-free. Only
revisit if a standalone artlib CLI/export is ever wanted — it would need a deflate
dependency (`miniz_oxide`/`flate2`).

### [conv] `rgba()` colour normalizer
Python's `rgba(color, alpha)` coerces 3- or 4-tuples. Folded into the typed Rust
API (`Rgba = [u8; 4]`, `alpha(color, a)`); no separate helper needed.

---

## DSL — `artlib-script` crate

All of these are `#[rune::function]` registrations in `artlib-script/src/lib.rs`
plus a line in `artlib_module()`.

### [gap] `fbm` / `ridged` source and falloff
DSL `fbm(size, seed, octaves, period)` and `ridged(size, seed, octaves, period)`
hard-code `source = perlin` and (for fbm) `falloff = 0.5`. The engine already takes
both (`artlib::texture::fbm(.., source, falloff)`).
- **Do:** expose them. Rune can't pass a Rust `fn` pointer, so accept a source
  **name** — e.g. `fbm_of(size, seed, octaves, period, "worley", falloff)` — and map
  `"perlin"|"value"|"worley"` to the `NoiseSource`. Blocks fbm-of-worley (weathered
  stone) and fbm-of-value from a script.

### [gap] `worley` f2 and jitter
DSL exposes only `worley` (f1) and `worley_cracks` (f2f1). The engine has
`worley_with(size, period, seed, feature, jitter)` with `Feature::F2` and a jitter
knob.
- **Do:** add `worley_f2(...)` and/or a `worley_jitter(...)` wrapper (or a
  feature-string variant).

### [gap] Grid × Grid multiply in the DSL
DSL `Grid::mul(scalar)` is scalar-only. Once the engine has `Mul<Grid>` (above),
add a `grid.mul_grid(other)` method (or overload).

### [gap] Coordinate-field primitive `[design]`
There is no way to express an arbitrary `|x, y| ...` field in the DSL, so the one
golden scene not reproducible from text is `modulate` (Python
`lambda x, y: 0.4 + 0.9 * x / 63`).
- **Do (design):** pick a primitive — a `linear_gradient(x0, y0, x1, y1)` field, or
  generic `coord_x()` / `coord_y()` fields that compose with arithmetic, or a tiny
  expression. `coord_x`/`coord_y` + the field algebra is the most general.

### [conv] Colour `mix` / `alpha`
`artlib::raster::mix(a, b, t)` and `alpha(color, a)` exist but aren't registered in
the DSL (which has `rgb` / `rgba` / `shade`).
- **Do:** register `mix(a, b, t) -> i64` and `alpha(color, a) -> i64` (packed).

### [conv] Grid arithmetic as operators
Grid math is method-form (`a.mul(0.7).add(b.mul(0.3))`). Rune supports operator
protocols; registering `ADD`/`SUB`/`MUL` on the `Grid` wrapper would allow
`a * 0.7 + b * 0.3`. Nice-to-have, not required.

---

## Known editor-integration notes (not vocabulary, but related)

- A generator **writes** a layer's pixels (destructive-but-re-runnable) because the
  core can't run Rune during compositing; the persisted recipe is what allows
  re-tuning. See `lapix::State::set_layer_generator`.
- Dragging a generator knob records **one undo entry per frame**, same as the
  existing filter controls. A debounce would be a general improvement to both.
