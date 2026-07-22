//! Positions of the floating tool windows.
//!
//! The windows are stacked automatically: the tool column down the left edge
//! and the preview in the bottom right corner. They can be dragged around, but
//! only for the session — the arrangement isn't saved, and is redone whenever
//! the drawing area changes size.

use std::cell::RefCell;
use std::collections::HashMap;

/// Titles of the windows being positioned.
pub const PALETTE: &str = "Palette";
pub const TOOLBOX: &str = "Toolbox";
pub const LAYERS: &str = "Layers";
pub const PREVIEW: &str = "Preview";

/// The windows stacked down the left edge, in order.
const COLUMN: [&str; 3] = [PALETTE, TOOLBOX, LAYERS];

/// Width the stacked tool windows are laid out to, so they line up as a column
/// of equal-width panels. Individual widgets can still be narrower than this.
///
/// Wide enough for a row of the layers table, which is the panel with the most
/// in it. Shortening its column headings would let this come down.
pub const PANEL_WIDTH: f32 = 330.;

const MARGIN: f32 = 15.;
const GAP: f32 = 10.;

/// Most frames to spend arranging before giving up on the positions settling.
///
/// Stacking the windows needs their sizes, which aren't known until they have
/// been shown once, and a window can change height when it moves somewhere with
/// more room. So the arrangement is recomputed until it stops changing, and
/// this only bounds how long that can take.
const MAX_ARRANGE_FRAMES: u8 = 8;

/// Sizes assumed for a window that hasn't been shown yet. Only used for the
/// first arrange pass, and corrected on the frame after.
fn fallback_size(title: &str) -> (f32, f32) {
    match title {
        PALETTE => (290., 430.),
        TOOLBOX => (200., 95.),
        LAYERS => (240., 115.),
        _ => (200., 200.),
    }
}

#[derive(Debug, PartialEq)]
enum Phase {
    /// Recomputing and forcing positions until they stop changing.
    Arranging(u8),
    /// Leaving the windows alone, so they can be dragged.
    Idle,
}

pub struct PanelLayout {
    positions: HashMap<&'static str, (f32, f32)>,
    /// Where each window actually was last frame, taken from what egui returned
    /// when it was shown.
    measured: RefCell<HashMap<&'static str, egui::Rect>>,
    phase: Phase,
    /// Size of the drawing area the current arrangement was made for.
    arranged_for: Option<(f32, f32)>,
}

impl PanelLayout {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            measured: RefCell::new(HashMap::new()),
            phase: Phase::Arranging(MAX_ARRANGE_FRAMES),
            arranged_for: None,
        }
    }

    /// Stacks the windows again, undoing any dragging.
    pub fn reset(&mut self) {
        self.phase = Phase::Arranging(MAX_ARRANGE_FRAMES);
        self.arranged_for = None;
    }

    /// Whether the windows are still being positioned, and so another frame is
    /// needed even if nothing else is happening.
    pub fn is_arranging(&self) -> bool {
        matches!(self.phase, Phase::Arranging(_))
    }

    /// Shows a tool window at its place in the layout, and notes where it ended
    /// up so the next arrange can use its real size.
    pub fn show<R>(
        &self,
        egui_ctx: &egui::Context,
        title: &'static str,
        add: impl FnOnce(&mut egui::Ui) -> R,
    ) -> Option<R> {
        let window = egui::Window::new(title);
        // Only the stacked column shares a width; the preview is sized by the
        // sprite it shows. Applied to the window rather than its contents:
        // `Ui::set_min_width` widens the content region but doesn't reliably
        // widen the window around it.
        let window = if COLUMN.contains(&title) {
            window.min_width(PANEL_WIDTH)
        } else {
            window
        };
        let window = match self.positions.get(title) {
            // Forced while arranging, so the window moves to where it has been
            // stacked; only a starting point afterwards, so it stays draggable.
            Some(&pos) if self.is_arranging() => window.current_pos(pos),
            Some(&pos) => window.default_pos(pos),
            None => window,
        };

        let response = window.show(egui_ctx, add)?;

        self.measured
            .borrow_mut()
            .insert(title, response.response.rect);

        response.inner
    }

    /// Call once per frame, after the windows have been shown.
    pub fn update(&mut self, egui_ctx: &egui::Context) {
        // The area panels leave free, so the stack clears the menu and status
        // bars without having to assume their heights.
        let area = egui_ctx.available_rect();
        let size = (area.width(), area.height());

        if self.arranged_for != Some(size) {
            self.phase = Phase::Arranging(MAX_ARRANGE_FRAMES);
        }

        let Phase::Arranging(frames_left) = self.phase else {
            return;
        };

        let arranged = self.arrange(area);
        self.arranged_for = Some(size);

        // Stop once recomputing stops changing anything.
        self.phase = if arranged == self.positions || frames_left <= 1 {
            Phase::Idle
        } else {
            Phase::Arranging(frames_left - 1)
        };
        self.positions = arranged;
    }

    /// Stacks the column down the left edge of `area` and puts the preview in
    /// its bottom right corner.
    ///
    /// On a window too short for the whole column, it continues into a second
    /// one rather than running off the bottom.
    fn arrange(&self, area: egui::Rect) -> HashMap<&'static str, (f32, f32)> {
        let mut positions = HashMap::new();
        let mut x = area.left() + MARGIN;
        let mut y = area.top() + MARGIN;
        let mut column_width: f32 = 0.;

        for title in COLUMN {
            let (width, height) = self.size_of(title);

            // Start a new column when this one would overflow, unless nothing
            // has been placed in it yet and moving on wouldn't help.
            if y + height > area.bottom() - MARGIN && y > area.top() + MARGIN {
                x += column_width + GAP;
                y = area.top() + MARGIN;
                column_width = 0.;
            }

            positions.insert(title, (x, y));

            y += height + GAP;
            column_width = column_width.max(width);
        }

        let (width, height) = self.size_of(PREVIEW);

        positions.insert(
            PREVIEW,
            (
                (area.right() - width - MARGIN).max(area.left() + MARGIN),
                (area.bottom() - height - MARGIN).max(area.top() + MARGIN),
            ),
        );

        positions
    }

    fn size_of(&self, title: &'static str) -> (f32, f32) {
        self.measured
            .borrow()
            .get(title)
            .map(|rect| (rect.width(), rect.height()))
            .unwrap_or_else(|| fallback_size(title))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0., 22.), egui::vec2(1200., 700.))
    }

    fn layout_with(sizes: &[(&'static str, f32, f32)]) -> PanelLayout {
        let layout = PanelLayout::new();

        for &(title, w, h) in sizes {
            layout.measured.borrow_mut().insert(
                title,
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h)),
            );
        }

        layout
    }

    #[test]
    fn stacks_the_column_without_overlapping() {
        let layout = layout_with(&[
            (PALETTE, 290., 400.),
            (TOOLBOX, 200., 95.),
            (LAYERS, 240., 115.),
            (PREVIEW, 200., 200.),
        ]);
        let positions = layout.arrange(area());

        let palette = positions[PALETTE];
        let toolbox = positions[TOOLBOX];
        let layers = positions[LAYERS];

        assert_eq!(palette, (15., 37.));
        assert_eq!(toolbox, (15., 37. + 400. + GAP));
        assert_eq!(layers, (15., 37. + 400. + GAP + 95. + GAP));
        assert!(toolbox.1 > palette.1 && layers.1 > toolbox.1);
    }

    #[test]
    fn puts_the_preview_in_the_bottom_right() {
        let layout = layout_with(&[(PREVIEW, 200., 150.)]);
        let positions = layout.arrange(area());

        assert_eq!(positions[PREVIEW], (1200. - 200. - 15., 722. - 150. - 15.));
    }

    #[test]
    fn wraps_into_a_second_column_when_out_of_room() {
        // A short window: the palette alone nearly fills it.
        let short = egui::Rect::from_min_size(egui::pos2(0., 22.), egui::vec2(1200., 500.));
        let layout = layout_with(&[
            (PALETTE, 290., 430.),
            (TOOLBOX, 200., 95.),
            (LAYERS, 240., 115.),
        ]);
        let positions = layout.arrange(short);

        assert_eq!(positions[PALETTE], (15., 37.));
        // No room under the palette, so the toolbox starts a second column.
        assert_eq!(positions[TOOLBOX], (15. + 290. + GAP, 37.));
        assert_eq!(positions[LAYERS], (15. + 290. + GAP, 37. + 95. + GAP));
    }

    /// Whether any two windows would overlap, which is the property the
    /// arrangement exists to guarantee.
    fn any_overlap(layout: &PanelLayout, positions: &HashMap<&'static str, (f32, f32)>) -> bool {
        let rects: Vec<egui::Rect> = COLUMN
            .iter()
            .map(|title| {
                let (x, y) = positions[title];
                let (w, h) = layout.size_of(title);

                egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h))
            })
            .collect();

        rects.iter().enumerate().any(|(i, a)| {
            rects
                .iter()
                .skip(i + 1)
                .any(|b| a.intersects(*b) && a.intersect(*b).area() > 0.)
        })
    }

    #[test]
    fn falls_back_to_assumed_sizes_before_anything_is_measured() {
        let layout = PanelLayout::new();
        let positions = layout.arrange(area());

        assert!(!any_overlap(&layout, &positions));
    }

    #[test]
    fn windows_never_overlap_however_short_the_area() {
        let layout = layout_with(&[
            (PALETTE, 290., 430.),
            (TOOLBOX, 200., 95.),
            (LAYERS, 240., 115.),
        ]);

        for height in [300., 400., 500., 600., 700., 900.] {
            let area = egui::Rect::from_min_size(egui::pos2(0., 22.), egui::vec2(1200., height));
            let positions = layout.arrange(area);

            assert!(
                !any_overlap(&layout, &positions),
                "windows overlap in a {height}pt tall area"
            );
        }
    }
}
