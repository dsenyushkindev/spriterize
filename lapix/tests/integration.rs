#[cfg(feature = "test-utils")]
use lapix::TestImage;

use lapix::color::{BLACK, TRANSPARENT};
use lapix::{Color, Event, Point, Size, State};

#[cfg(feature = "test-utils")]
fn smooth_with(strength: i32, passes: i32) -> lapix::Filter {
    use lapix::filter::Value;

    let mut filter = lapix::Filter::smooth();
    filter.params.set("strength", Value::Int(strength));
    filter.params.set("passes", Value::Int(passes));

    filter
}

#[cfg(feature = "test-utils")]
fn silhouette_with(color: Color, threshold: i32) -> lapix::Filter {
    use lapix::filter::Value;

    let mut filter = lapix::Filter::silhouette();
    filter.params.set("color", Value::Color(color));
    filter.params.set("threshold", Value::Int(threshold));

    filter
}

#[cfg(feature = "test-utils")]
#[test]
fn empty_canvas() {
    let side = 10;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

    for i in 0..side {
        for j in 0..side {
            assert_eq!(state.canvas().pixel(Point::new(i, j)), TRANSPARENT);
        }
    }
}

#[cfg(feature = "test-utils")]
#[test]
fn draw_line() {
    let side = 10;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);
    state.execute(Event::LineStart(Point::new(0, 0)));
    state.execute(Event::LineEnd(Point::new(side - 1, side - 1)));

    for i in 0..side {
        for j in 0..side {
            let color = if i == j { BLACK } else { TRANSPARENT };

            assert_eq!(state.canvas().pixel(Point::new(i, j)), color);
        }
    }
}

#[cfg(feature = "test-utils")]
#[test]
fn draw_red_line() {
    let side = 10;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);
    let red = Color::new(255, 0, 0, 255);
    state.execute(Event::SetMainColor(red));
    state.execute(Event::LineStart(Point::new(0, 0)));
    state.execute(Event::LineEnd(Point::new(side - 1, side - 1)));

    for i in 0..side {
        for j in 0..side {
            let color = if i == j { red } else { TRANSPARENT };

            assert_eq!(state.canvas().pixel(Point::new(i, j)), color);
        }
    }
}

#[cfg(feature = "test-utils")]
#[test]
fn draw_line_then_clear_canvas() {
    let side = 10;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);
    state.execute(Event::LineStart(Point::new(0, 0)));
    state.execute(Event::LineEnd(Point::new(side - 1, side - 1)));
    state.execute(Event::ClearCanvas);

    for i in 0..side {
        for j in 0..side {
            assert_eq!(state.canvas().pixel(Point::new(i, j)), TRANSPARENT);
        }
    }
}

#[cfg(feature = "test-utils")]
#[test]
fn bucket() {
    let side = 10;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);
    state.execute(Event::Bucket(Point::new(0, 0)));

    for i in 0..side {
        for j in 0..side {
            assert_eq!(state.canvas().pixel(Point::new(i, j)), BLACK);
        }
    }
}

#[cfg(feature = "test-utils")]
#[test]
fn undo_restores_previous_canvas() {
    let side = 10;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);
    state.execute(Event::LineStart(Point::new(0, 0)));
    state.execute(Event::LineEnd(Point::new(side - 1, side - 1)));

    assert!(state.can_undo());
    assert!(!state.can_redo());

    state.execute(Event::Undo);

    for i in 0..side {
        for j in 0..side {
            assert_eq!(state.canvas().pixel(Point::new(i, j)), TRANSPARENT);
        }
    }
    assert!(state.can_redo());
}

#[cfg(feature = "test-utils")]
#[test]
fn redo_reapplies_undone_action() {
    let side = 10;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);
    state.execute(Event::LineStart(Point::new(0, 0)));
    state.execute(Event::LineEnd(Point::new(side - 1, side - 1)));
    state.execute(Event::Undo);
    state.execute(Event::Redo);

    for i in 0..side {
        for j in 0..side {
            let color = if i == j { BLACK } else { TRANSPARENT };

            assert_eq!(state.canvas().pixel(Point::new(i, j)), color);
        }
    }
    assert!(!state.can_redo());
    assert!(state.can_undo());
}

// Guards the ordering of the inverse actions: with two overlapping strokes the
// canvas only ends up correct if redo replays them in their original order.
#[cfg(feature = "test-utils")]
#[test]
fn undo_redo_round_trip_with_overlapping_strokes() {
    let side = 10;
    let red = Color::new(255, 0, 0, 255);
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

    state.execute(Event::Bucket(Point::new(0, 0)));
    state.execute(Event::SetMainColor(red));
    state.execute(Event::LineStart(Point::new(0, 0)));
    state.execute(Event::LineEnd(Point::new(side - 1, side - 1)));

    let expected: Vec<Color> = (0..side)
        .flat_map(|i| (0..side).map(move |j| if i == j { red } else { BLACK }))
        .collect();

    state.execute(Event::Undo);
    state.execute(Event::Redo);

    let actual: Vec<Color> = (0..side)
        .flat_map(|i| (0..side).map(move |j| (i, j)))
        .map(|(i, j)| state.canvas().pixel(Point::new(i, j)))
        .collect();

    assert_eq!(actual, expected);
}

#[cfg(feature = "test-utils")]
#[test]
fn new_action_clears_the_redo_stack() {
    let side = 10;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);
    state.execute(Event::LineStart(Point::new(0, 0)));
    state.execute(Event::LineEnd(Point::new(side - 1, side - 1)));
    state.execute(Event::Undo);

    assert!(state.can_redo());

    state.execute(Event::Bucket(Point::new(0, 0)));

    assert!(!state.can_redo());
}

#[cfg(feature = "test-utils")]
#[test]
fn cannot_undo_or_redo_on_a_fresh_state() {
    let side = 10;
    let state = State::<TestImage>::new(Size::new(side, side), None, None);

    assert!(!state.can_undo());
    assert!(!state.can_redo());
}

#[cfg(feature = "test-utils")]
#[test]
fn a_wider_brush_paints_a_disc() {
    let side = 11;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);
    let centre = Point::new(5, 5);

    state.execute(Event::SetBrushRadius(2)).unwrap();
    state.execute(Event::BrushStart).unwrap();
    state.execute(Event::BrushStroke(centre)).unwrap();
    state.execute(Event::BrushEnd).unwrap();

    for i in 0..side {
        for j in 0..side {
            let (dx, dy) = (i - centre.x, j - centre.y);
            let inside = dx * dx + dy * dy <= 4;
            let color = if inside { BLACK } else { TRANSPARENT };

            assert_eq!(
                state.canvas().pixel(Point::new(i, j)),
                color,
                "at {i},{j} with radius 2"
            );
        }
    }
}

#[cfg(feature = "test-utils")]
#[test]
fn undo_reverts_a_whole_wide_stroke() {
    // Overlapping stamps set some pixels more than once, so this catches
    // reversals that record a color from earlier in the same stroke.
    let side = 12;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

    state.execute(Event::SetBrushRadius(2)).unwrap();
    state.execute(Event::BrushStart).unwrap();
    state.execute(Event::BrushStroke(Point::new(2, 2))).unwrap();
    state.execute(Event::BrushStroke(Point::new(9, 9))).unwrap();
    state.execute(Event::BrushEnd).unwrap();
    state.execute(Event::Undo).unwrap();

    for i in 0..side {
        for j in 0..side {
            assert_eq!(
                state.canvas().pixel(Point::new(i, j)),
                TRANSPARENT,
                "at {i},{j} after undo"
            );
        }
    }
}

#[cfg(feature = "test-utils")]
#[test]
fn selection_preview_matches_the_selection_it_makes() {
    let side = 10;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);
    let (from, to) = (Point::new(2, 3), Point::new(6, 8));

    state.execute(Event::StartSelection(from)).unwrap();
    let preview = state.selection_in_progress(to);

    state.execute(Event::EndSelection(to)).unwrap();
    let selection = match state.selection() {
        Some(lapix::Selection::Canvas(rect)) => rect,
        other => panic!("expected a canvas selection, got {other:?}"),
    };

    assert_eq!(preview, Some(selection));
}

#[cfg(feature = "test-utils")]
#[test]
fn there_is_no_selection_preview_when_nothing_is_being_dragged() {
    let side = 10;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

    assert_eq!(state.selection_in_progress(Point::new(1, 1)), None);

    state.execute(Event::BrushStart).unwrap();
    assert_eq!(state.selection_in_progress(Point::new(1, 1)), None);
}

#[cfg(feature = "test-utils")]
#[test]
fn a_wider_stroke_thickens_a_line() {
    let side = 11;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

    state.execute(Event::SetBrushRadius(1)).unwrap();
    state.execute(Event::LineStart(Point::new(5, 2))).unwrap();
    state.execute(Event::LineEnd(Point::new(5, 8))).unwrap();

    // A radius of one turns the single pixel wide line into a three wide one.
    for y in 2..=8 {
        for x in 4..=6 {
            assert_eq!(
                state.canvas().pixel(Point::new(x, y)),
                BLACK,
                "at {x},{y} on a thick line"
            );
        }
    }

    // ...and no further.
    assert_eq!(state.canvas().pixel(Point::new(3, 5)), TRANSPARENT);
    assert_eq!(state.canvas().pixel(Point::new(7, 5)), TRANSPARENT);
}

#[cfg(feature = "test-utils")]
#[test]
fn a_wider_stroke_thickens_rectangles_and_ellipses() {
    let side = 16;

    for event in [
        Event::RectEnd(Point::new(12, 12)),
        Event::EllipseEnd(Point::new(12, 12)),
    ] {
        let start = match event {
            Event::RectEnd(_) => Event::RectStart(Point::new(3, 3)),
            _ => Event::EllipseStart(Point::new(3, 3)),
        };
        let mut thin = State::<TestImage>::new(Size::new(side, side), None, None);
        thin.execute(start.clone()).unwrap();
        thin.execute(event.clone()).unwrap();

        let mut thick = State::<TestImage>::new(Size::new(side, side), None, None);
        thick.execute(Event::SetBrushRadius(2)).unwrap();
        thick.execute(start).unwrap();
        thick.execute(event.clone()).unwrap();

        let painted = |state: &State<TestImage>| {
            (0..side)
                .flat_map(|i| (0..side).map(move |j| (i, j)))
                .filter(|(i, j)| state.canvas().pixel(Point::new(*i, *j)) != TRANSPARENT)
                .count()
        };

        assert!(
            painted(&thick) > painted(&thin),
            "a wider stroke should cover more for {event:?}"
        );
    }
}

#[cfg(feature = "test-utils")]
#[test]
fn a_thick_preview_is_not_clipped_at_its_corners() {
    // The preview image has to grow by the radius on every side, or the ends of
    // a thick stroke would be cut off.
    let radius = 3;
    let thin =
        lapix::FreeImage::<TestImage>::line_preview(Point::new(4, 4), Point::new(8, 8), BLACK, 0);
    let thick = lapix::FreeImage::<TestImage>::line_preview(
        Point::new(4, 4),
        Point::new(8, 8),
        BLACK,
        radius,
    );

    assert_eq!(thick.rect.w, thin.rect.w + 2 * radius as i32);
    assert_eq!(thick.rect.h, thin.rect.h + 2 * radius as i32);
    assert_eq!(thick.rect.x, thin.rect.x - radius as i32);
    assert_eq!(thick.rect.y, thin.rect.y - radius as i32);
}

#[cfg(feature = "test-utils")]
#[test]
fn smoothing_softens_a_hard_edge() {
    use lapix::{Bitmap, Transform};

    // A black half and a white half meeting down the middle.
    let white = Color::new(255, 255, 255, 255);
    let mut img = TestImage::new(Size::new(6, 3), white);

    for j in 0..3 {
        for i in 0..3 {
            img.set_pixel(Point::new(i, j), BLACK);
        }
    }

    Transform::Smooth.apply(&mut img, Vec::new());

    let left = img.pixel(Point::new(2, 1));
    let right = img.pixel(Point::new(3, 1));

    // The two pixels either side of the seam become intermediate shades.
    assert!(left.r > 0 && left.r < 255, "left of the seam: {left:?}");
    assert!(right.r > 0 && right.r < 255, "right of the seam: {right:?}");
    assert!(left.r < right.r, "the gradient should run dark to light");

    // Away from the seam the original colors survive.
    assert_eq!(img.pixel(Point::new(0, 1)), BLACK);
    assert_eq!(img.pixel(Point::new(5, 1)), white);
}

#[cfg(feature = "test-utils")]
#[test]
fn smoothing_does_not_darken_edges_against_transparency() {
    use lapix::{Bitmap, Transform};

    // Transparent pixels are transparent black, so averaging colors without
    // weighting them by alpha would pull a dark fringe into the red square.
    let red = Color::new(255, 0, 0, 255);
    let mut img = TestImage::new(Size::new(5, 5), TRANSPARENT);

    for j in 1..4 {
        for i in 1..4 {
            img.set_pixel(Point::new(i, j), red);
        }
    }

    Transform::Smooth.apply(&mut img, Vec::new());

    for j in 1..4 {
        for i in 1..4 {
            let color = img.pixel(Point::new(i, j));

            assert_eq!(
                (color.r, color.g, color.b),
                (255, 0, 0),
                "at {i},{j} the hue should stay red, only the alpha softens"
            );
        }
    }

    // The edge does become partly transparent, which is the softening.
    assert!(img.pixel(Point::new(1, 1)).a < 255);
}

#[cfg(feature = "test-utils")]
#[test]
fn the_smooth_tool_only_touches_what_it_is_dragged_over() {
    use lapix::Bitmap;

    let white = Color::new(255, 255, 255, 255);
    let side = 12;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

    // A hard vertical seam down the middle.
    state.execute(Event::SetMainColor(BLACK)).unwrap();
    state.execute(Event::RectStart(Point::new(0, 0))).unwrap();
    state.execute(Event::SetMainColor(white)).unwrap();

    let mut img = TestImage::new(Size::new(side, side), white);
    for j in 0..side {
        for i in 0..side / 2 {
            img.set_pixel(Point::new(i, j), BLACK);
        }
    }
    state.execute(Event::ClearCanvas).unwrap();
    state.canvas_mut().set_img(img);

    // Smooth only near the top of the seam.
    state.execute(Event::SetBrushRadius(1)).unwrap();
    state.execute(Event::SmoothStart).unwrap();
    state
        .execute(Event::SmoothStroke(Point::new(6, 1)))
        .unwrap();
    state.execute(Event::SmoothEnd).unwrap();

    let touched = state.canvas().pixel(Point::new(6, 1));
    let untouched = state.canvas().pixel(Point::new(6, 9));

    assert!(
        touched.r > 0 && touched.r < 255,
        "the pixel under the brush should soften: {touched:?}"
    );
    assert_eq!(
        untouched, white,
        "the same seam further down should be untouched"
    );
}

#[cfg(feature = "test-utils")]
#[test]
fn smoothing_leaves_a_flat_image_alone() {
    use lapix::{Bitmap, Transform};

    let blue = Color::new(0, 0, 255, 255);
    let mut img = TestImage::new(Size::new(4, 4), blue);

    Transform::Smooth.apply(&mut img, Vec::new());

    for j in 0..4 {
        for i in 0..4 {
            assert_eq!(img.pixel(Point::new(i, j)), blue, "at {i},{j}");
        }
    }
}

#[cfg(feature = "test-utils")]
#[test]
fn a_filter_changes_what_is_seen_but_not_what_is_stored() {
    use lapix::{Bitmap, Filter};

    let side = 8;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

    // A single dot, so it has edges to soften against.
    state.execute(Event::BrushStart).unwrap();
    state.execute(Event::BrushStroke(Point::new(4, 4))).unwrap();
    state.execute(Event::BrushEnd).unwrap();
    state
        .execute(Event::SetLayerFilters(0, vec![Filter::smooth()]))
        .unwrap();

    // The stored pixel is untouched...
    assert_eq!(state.canvas().pixel(Point::new(4, 4)), BLACK);
    // ...but what is shown has softened into its surroundings.
    assert!(state.rendered_layer(0).pixel(Point::new(4, 4)).a < 255);
    assert!(state.rendered_layer(0).pixel(Point::new(3, 4)).a > 0);
}

#[cfg(feature = "test-utils")]
#[test]
fn turning_filters_off_shows_the_pixels_as_drawn() {
    use lapix::{Bitmap, Filter};

    let side = 8;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

    state.execute(Event::Bucket(Point::new(0, 0))).unwrap();
    state
        .execute(Event::SetLayerFilters(0, vec![Filter::smooth()]))
        .unwrap();
    state.execute(Event::SetFiltersEnabled(false)).unwrap();

    assert!(!state.filters_enabled());
    assert_eq!(state.rendered_layer(0).pixel(Point::new(0, 0)), BLACK);
    // What the eyedropper reads follows the same switch.
    assert_eq!(state.visible_pixel(Point::new(0, 0)), BLACK);
}

#[cfg(feature = "test-utils")]
#[test]
fn filter_changes_can_be_undone() {
    use lapix::Filter;

    let side = 8;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

    state
        .execute(Event::SetLayerFilters(0, vec![Filter::smooth()]))
        .unwrap();
    assert_eq!(state.layers().get(0).filters(), &[Filter::smooth()]);

    state
        .execute(Event::SetLayerFilters(
            0,
            vec![Filter::smooth(), Filter::silhouette()],
        ))
        .unwrap();
    assert_eq!(state.layers().get(0).filters().len(), 2);

    state.execute(Event::Undo).unwrap();
    assert_eq!(state.layers().get(0).filters(), &[Filter::smooth()]);

    state.execute(Event::Undo).unwrap();
    assert!(state.layers().get(0).filters().is_empty());

    state.execute(Event::Redo).unwrap();
    assert_eq!(state.layers().get(0).filters(), &[Filter::smooth()]);
}

#[cfg(feature = "test-utils")]
#[test]
fn drawing_on_a_filtered_layer_refreshes_what_is_shown() {
    use lapix::{Bitmap, Filter};

    let side = 8;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

    state
        .execute(Event::SetLayerFilters(0, vec![Filter::silhouette()]))
        .unwrap();
    // Nothing drawn yet, so nothing to see.
    assert_eq!(state.rendered_layer(0).pixel(Point::new(4, 4)), TRANSPARENT);

    let red = Color::new(255, 0, 0, 255);
    state.execute(Event::SetMainColor(red)).unwrap();
    state.execute(Event::BrushStart).unwrap();
    state.execute(Event::BrushStroke(Point::new(4, 4))).unwrap();
    state.execute(Event::BrushEnd).unwrap();

    // The cached result has to be dropped when pixels change, or this would
    // still be transparent.
    assert_eq!(state.rendered_layer(0).pixel(Point::new(4, 4)), BLACK);
}

#[cfg(feature = "test-utils")]
#[test]
fn an_adjustment_layer_filters_the_layers_below_it() {
    use lapix::{Bitmap, Filter};

    let side = 8;
    let red = Color::new(255, 0, 0, 255);
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

    // A red dot on the bottom layer.
    state.execute(Event::SetMainColor(red)).unwrap();
    state.execute(Event::BrushStart).unwrap();
    state.execute(Event::BrushStroke(Point::new(4, 4))).unwrap();
    state.execute(Event::BrushEnd).unwrap();

    // An empty layer above it, turned into an adjustment layer.
    state.execute(Event::NewLayerAbove).unwrap();
    state.execute(Event::SetLayerAdjustment(1, true)).unwrap();
    state
        .execute(Event::SetLayerFilters(1, vec![Filter::silhouette()]))
        .unwrap();

    // The dot below is flattened to black even though the layer holding it has
    // no filters of its own, and its own pixels are untouched.
    assert_eq!(state.layers().canvas_at(0).pixel(Point::new(4, 4)), red);
    assert_eq!(state.visible_pixel(Point::new(4, 4)), BLACK);
}

#[cfg(feature = "test-utils")]
#[test]
fn an_adjustment_layer_leaves_the_layers_above_it_alone() {
    use lapix::Filter;

    let side = 8;
    let red = Color::new(255, 0, 0, 255);
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

    // Bottom layer: nothing. Middle: adjustment. Top: a red dot.
    state.execute(Event::NewLayerAbove).unwrap();
    state.execute(Event::SetLayerAdjustment(1, true)).unwrap();
    state
        .execute(Event::SetLayerFilters(1, vec![Filter::silhouette()]))
        .unwrap();

    state.execute(Event::NewLayerAbove).unwrap();
    state.execute(Event::SwitchLayer(2)).unwrap();
    state.execute(Event::SetMainColor(red)).unwrap();
    state.execute(Event::BrushStart).unwrap();
    state.execute(Event::BrushStroke(Point::new(4, 4))).unwrap();
    state.execute(Event::BrushEnd).unwrap();

    // Sitting above the adjustment layer, it keeps its color.
    assert_eq!(state.visible_pixel(Point::new(4, 4)), red);
}

#[cfg(feature = "test-utils")]
#[test]
fn hiding_an_adjustment_layer_undoes_its_effect() {
    use lapix::Filter;

    let side = 8;
    let red = Color::new(255, 0, 0, 255);
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

    state.execute(Event::SetMainColor(red)).unwrap();
    state.execute(Event::BrushStart).unwrap();
    state.execute(Event::BrushStroke(Point::new(4, 4))).unwrap();
    state.execute(Event::BrushEnd).unwrap();

    state.execute(Event::NewLayerAbove).unwrap();
    state.execute(Event::SetLayerAdjustment(1, true)).unwrap();
    state
        .execute(Event::SetLayerFilters(1, vec![Filter::silhouette()]))
        .unwrap();
    assert_eq!(state.visible_pixel(Point::new(4, 4)), BLACK);

    // Each of these has to drop the flattened image, or the effect would linger.
    state
        .execute(Event::ChangeLayerVisibility(1, false))
        .unwrap();
    assert_eq!(state.visible_pixel(Point::new(4, 4)), red);

    state
        .execute(Event::ChangeLayerVisibility(1, true))
        .unwrap();
    assert_eq!(state.visible_pixel(Point::new(4, 4)), BLACK);

    state.execute(Event::SetFiltersEnabled(false)).unwrap();
    assert_eq!(state.visible_pixel(Point::new(4, 4)), red);
}

#[cfg(feature = "test-utils")]
#[test]
fn making_a_layer_an_adjustment_can_be_undone() {
    let side = 8;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

    state.execute(Event::SetLayerAdjustment(0, true)).unwrap();
    assert!(state.layers().get(0).is_adjustment());

    state.execute(Event::Undo).unwrap();
    assert!(!state.layers().get(0).is_adjustment());

    state.execute(Event::Redo).unwrap();
    assert!(state.layers().get(0).is_adjustment());
}

#[cfg(feature = "test-utils")]
#[test]
fn a_filter_at_zero_strength_changes_nothing() {
    use lapix::{Bitmap, Filter};

    let side = 8;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

    state.execute(Event::BrushStart).unwrap();
    state.execute(Event::BrushStroke(Point::new(4, 4))).unwrap();
    state.execute(Event::BrushEnd).unwrap();

    state
        .execute(Event::SetLayerFilters(0, vec![smooth_with(0, 4)]))
        .unwrap();

    assert_eq!(state.rendered_layer(0).pixel(Point::new(4, 4)), BLACK);
    assert_eq!(
        state.rendered_layer(0).pixel(Point::new(3, 4)),
        TRANSPARENT,
        "nothing should bleed outwards at zero strength"
    );
}

#[cfg(feature = "test-utils")]
#[test]
fn smoothing_strength_controls_how_far_it_goes() {
    use lapix::{Bitmap, Filter};

    let side = 8;
    let dot = Point::new(4, 4);

    let alpha_at = |strength: i32| {
        let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

        state.execute(Event::BrushStart).unwrap();
        state.execute(Event::BrushStroke(dot)).unwrap();
        state.execute(Event::BrushEnd).unwrap();
        state
            .execute(Event::SetLayerFilters(0, vec![smooth_with(strength, 1)]))
            .unwrap();

        // Bound before returning, so the borrow of `state` ends first.
        let alpha = state.rendered_layer(0).pixel(dot).a;

        alpha
    };

    // The more strength, the more the dot spreads out, so the less is left in
    // the middle.
    assert!(alpha_at(64) > alpha_at(128));
    assert!(alpha_at(128) > alpha_at(255));
}

#[cfg(feature = "test-utils")]
#[test]
fn more_passes_spread_a_blur_further() {
    use lapix::{Bitmap, Filter};

    let side = 12;
    let dot = Point::new(6, 6);
    let far = Point::new(8, 6);

    let alpha_after = |passes: i32| {
        let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

        state.execute(Event::BrushStart).unwrap();
        state.execute(Event::BrushStroke(dot)).unwrap();
        state.execute(Event::BrushEnd).unwrap();
        state
            .execute(Event::SetLayerFilters(0, vec![smooth_with(255, passes)]))
            .unwrap();

        let alpha = state.rendered_layer(0).pixel(far).a;

        alpha
    };

    // One pass of a 3x3 kernel can't reach two pixels away; several can.
    assert_eq!(alpha_after(1), 0);
    assert!(alpha_after(3) > 0);
}

#[cfg(feature = "test-utils")]
#[test]
fn silhouette_settings_choose_the_color_and_the_cutoff() {
    use lapix::{Bitmap, Filter};

    let side = 6;
    let faint = Color::new(255, 255, 255, 100);
    let blue = Color::new(0, 0, 255, 255);
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

    state.execute(Event::SetMainColor(faint)).unwrap();
    state.execute(Event::BrushStart).unwrap();
    state.execute(Event::BrushStroke(Point::new(3, 3))).unwrap();
    state.execute(Event::BrushEnd).unwrap();

    // Too faint to reach the default cutoff, so it is left alone.
    state
        .execute(Event::SetLayerFilters(0, vec![silhouette_with(blue, 128)]))
        .unwrap();
    assert_eq!(state.rendered_layer(0).pixel(Point::new(3, 3)), faint);

    // Lower the cutoff and it is filled, with the chosen color.
    state
        .execute(Event::SetLayerFilters(0, vec![silhouette_with(blue, 50)]))
        .unwrap();
    assert_eq!(state.rendered_layer(0).pixel(Point::new(3, 3)), blue);
}

/// A filter that only exists in this test, to show that registering one is
/// enough to make it work everywhere.
#[cfg(feature = "test-utils")]
struct Invert;

#[cfg(feature = "test-utils")]
impl lapix::filter::FilterKind for Invert {
    fn id(&self) -> &'static str {
        "test_invert"
    }

    fn name(&self) -> &'static str {
        "Invert"
    }

    fn params(&self) -> &'static [lapix::filter::ParamSpec] {
        use lapix::filter::{ParamKind, ParamSpec, Value};

        &[ParamSpec {
            id: "alpha_too",
            label: "invert alpha",
            kind: ParamKind::Bool,
            default: Value::Bool(false),
            help: "flip the alpha channel as well",
        }]
    }

    fn apply(
        &self,
        surface: &mut dyn lapix::filter::Surface,
        params: &lapix::filter::Params,
        _palette: &[Color],
    ) {
        let alpha_too = params.bool("alpha_too", false);
        let size = surface.size();

        for i in 0..size.x {
            for j in 0..size.y {
                let p = Point::new(i, j);
                let c = surface.pixel(p);
                let alpha = if alpha_too { 255 - c.a } else { c.a };

                surface.set_pixel(p, Color::new(255 - c.r, 255 - c.g, 255 - c.b, alpha));
            }
        }
    }
}

#[cfg(feature = "test-utils")]
#[test]
fn a_registered_filter_can_be_looked_up_and_run() {
    use lapix::{filter, Bitmap};

    assert!(filter::register(Invert), "should register the first time");
    assert!(
        !filter::register(Invert),
        "an id that is taken should be refused"
    );

    let kind = filter::kind("test_invert").expect("just registered");
    assert_eq!(kind.name(), "Invert");
    assert!(filter::kinds().iter().any(|k| k.id() == "test_invert"));

    // Its declared settings become its starting settings.
    let inverted = lapix::Filter::new(kind);
    assert!(!inverted.params.bool("alpha_too", true));

    let side = 4;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);
    state.execute(Event::Bucket(Point::new(0, 0))).unwrap();
    state
        .execute(Event::SetLayerFilters(0, vec![inverted]))
        .unwrap();

    // Black filled, so inverting shows white.
    let seen = state.rendered_layer(0).pixel(Point::new(1, 1));
    assert_eq!((seen.r, seen.g, seen.b), (255, 255, 255));
}

#[cfg(feature = "test-utils")]
#[test]
fn a_filter_from_an_unknown_kind_is_left_alone() {
    use lapix::filter::Params;
    use lapix::{Bitmap, Filter};

    let side = 4;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);
    state.execute(Event::Bucket(Point::new(0, 0))).unwrap();

    // As a project saved by a build that had a filter this one doesn't.
    let unknown = Filter {
        id: "not_a_real_filter".to_owned(),
        params: Params::default(),
    };
    state
        .execute(Event::SetLayerFilters(0, vec![unknown.clone()]))
        .unwrap();

    assert_eq!(unknown.name(), "Unknown (not_a_real_filter)");
    assert!(unknown.kind().is_none());
    // Opens and draws rather than failing.
    assert_eq!(state.rendered_layer(0).pixel(Point::new(1, 1)), BLACK);
}

#[cfg(feature = "test-utils")]
#[test]
fn a_setting_that_was_never_saved_falls_back_to_its_default() {
    use lapix::filter::Params;
    use lapix::{Bitmap, Filter};

    let side = 6;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

    state.execute(Event::BrushStart).unwrap();
    state.execute(Event::BrushStroke(Point::new(3, 3))).unwrap();
    state.execute(Event::BrushEnd).unwrap();

    // As a project saved before the filter grew its settings.
    let bare = Filter {
        id: "smooth".to_owned(),
        params: Params::default(),
    };
    state
        .execute(Event::SetLayerFilters(0, vec![bare]))
        .unwrap();

    // Full strength, one pass: what the defaults say.
    assert!(state.rendered_layer(0).pixel(Point::new(3, 3)).a < 255);
}

#[cfg(feature = "test-utils")]
#[test]
fn a_sheet_needs_a_cell_for_every_layer() {
    let mut state = State::<TestImage>::new(Size::new(4, 4), None, None);

    state.execute(Event::NewLayerAbove).unwrap();
    state.execute(Event::NewLayerAbove).unwrap();

    // Three layers won't fit in two cells.
    let too_small = state.execute(Event::ExportLayerSheet(
        std::path::PathBuf::from("unused.png"),
        Size::new(2, 1),
        lapix::ExportOptions::default(),
    ));

    assert!(
        matches!(too_small, Err(lapix::Error::SheetTooSmall { .. })),
        "expected a sheet size error, got {too_small:?}"
    );
}

#[cfg(feature = "test-utils")]
#[test]
fn a_composited_frame_reflects_all_its_layers() {
    use lapix::Bitmap;

    let red = Color::new(255, 0, 0, 255);
    let blue = Color::new(0, 0, 255, 255);
    let mut state = State::<TestImage>::new(Size::new(4, 4), None, None);

    // Bottom layer red across a new frame, top layer one blue dot.
    state.execute(Event::AddFrame).unwrap();
    state.execute(Event::SetMainColor(red)).unwrap();
    state.execute(Event::Bucket(Point::new(0, 0))).unwrap();
    state.execute(Event::NewLayerAbove).unwrap();
    state.execute(Event::SwitchLayer(1)).unwrap();
    state.execute(Event::SetMainColor(blue)).unwrap();
    state.execute(Event::BrushStart).unwrap();
    state.execute(Event::BrushStroke(Point::new(1, 1))).unwrap();
    state.execute(Event::BrushEnd).unwrap();

    // frame_image blends the stack for that frame, regardless of which is
    // active.
    state.execute(Event::SwitchFrame(0)).unwrap();
    let frame1 = state.frame_image(1);

    assert_eq!(frame1.pixel(Point::new(1, 1)), blue);
    assert_eq!(frame1.pixel(Point::new(0, 0)), red);

    // Frame 0 was never drawn on, so its composite is empty.
    let frame0 = state.frame_image(0);
    assert_eq!(frame0.pixel(Point::new(0, 0)), TRANSPARENT);
}

#[cfg(feature = "test-utils")]
#[test]
fn a_frame_sheet_needs_a_cell_for_every_frame() {
    let mut state = State::<TestImage>::new(Size::new(4, 4), None, None);

    state.execute(Event::AddFrame).unwrap();
    state.execute(Event::AddFrame).unwrap();

    // Three frames won't fit in a 1x1 sheet.
    let too_small = state.execute(Event::ExportFrameSheet(
        std::path::PathBuf::from("unused.png"),
        Size::new(1, 1),
        lapix::ExportOptions::default(),
    ));

    assert!(
        matches!(too_small, Err(lapix::Error::SheetTooSmall { .. })),
        "expected a sheet size error, got {too_small:?}"
    );
}

#[cfg(feature = "test-utils")]
fn dotted(side: i32, at: Point<i32>, color: Color) -> TestImage {
    use lapix::Bitmap;

    let mut img = TestImage::new(Size::new(side, side), TRANSPARENT);
    img.set_pixel(at, color);

    img
}

#[cfg(feature = "test-utils")]
#[test]
fn cropping_trims_to_what_is_drawn() {
    use lapix::{export, Bitmap};

    let img = dotted(16, Point::new(5, 7), BLACK);
    let options = lapix::ExportOptions {
        crop: true,
        ..Default::default()
    };
    let out = export::prepare(&img, &options, None);

    assert_eq!((out.width(), out.height()), (1, 1));
    assert_eq!(out.pixel(Point::new(0, 0)), BLACK);
}

#[cfg(feature = "test-utils")]
#[test]
fn cropping_an_empty_image_leaves_it_alone() {
    use lapix::{export, Bitmap};

    // Trimming to nothing would give a zero sized file.
    let img = TestImage::new(Size::new(8, 8), TRANSPARENT);
    let options = lapix::ExportOptions {
        crop: true,
        ..Default::default()
    };
    let out = export::prepare(&img, &options, None);

    assert_eq!((out.width(), out.height()), (8, 8));
}

#[cfg(feature = "test-utils")]
#[test]
fn padding_surrounds_the_image_on_every_side() {
    use lapix::{export, Bitmap};

    let img = dotted(4, Point::new(0, 0), BLACK);
    let options = lapix::ExportOptions {
        padding: 3,
        ..Default::default()
    };
    let out = export::prepare(&img, &options, None);

    assert_eq!((out.width(), out.height()), (10, 10));
    assert_eq!(out.pixel(Point::new(3, 3)), BLACK);
}

#[cfg(feature = "test-utils")]
#[test]
fn the_steps_run_in_order() {
    use lapix::{export, Bitmap};

    // A single dot in a 16x16 image: crop to 1x1, pad to 3x3, double to 6x6,
    // then round up to 8x8. Any other order gives a different size.
    let img = dotted(16, Point::new(9, 2), BLACK);
    let options = lapix::ExportOptions {
        crop: true,
        padding: 1,
        scale: lapix::Scale::new(2, 1),
        power_of_two: true,
    };
    let out = export::prepare(&img, &options, None);

    assert_eq!((out.width(), out.height()), (8, 8));
    // The doubled dot sits where the padding put it, not at the origin.
    assert_eq!(out.pixel(Point::new(2, 2)), BLACK);
    assert_eq!(out.pixel(Point::new(3, 3)), BLACK);
}

#[cfg(feature = "test-utils")]
#[test]
fn scaling_up_repeats_whole_pixels() {
    use lapix::{export, Bitmap};

    let img = dotted(2, Point::new(0, 0), BLACK);
    let out = export::scale(&img, lapix::Scale::new(3, 1));

    assert_eq!((out.width(), out.height()), (6, 6));

    for x in 0..3 {
        for y in 0..3 {
            assert_eq!(out.pixel(Point::new(x, y)), BLACK, "at {x},{y}");
        }
    }
    assert_eq!(out.pixel(Point::new(3, 0)), TRANSPARENT);
}

#[cfg(feature = "test-utils")]
#[test]
fn scaling_down_a_doubled_image_gives_the_original_back() {
    use lapix::{export, Bitmap};

    // Every block is one solid color, so averaging is exact.
    let red = Color::new(255, 0, 0, 255);
    let img = dotted(4, Point::new(1, 2), red);
    let doubled = export::scale(&img, lapix::Scale::new(2, 1));
    let back = export::scale(&doubled, lapix::Scale::new(1, 2));

    assert_eq!((back.width(), back.height()), (4, 4));

    for x in 0..4 {
        for y in 0..4 {
            let p = Point::new(x, y);

            assert_eq!(back.pixel(p), img.pixel(p), "at {x},{y}");
        }
    }
}

#[cfg(feature = "test-utils")]
#[test]
fn scaling_down_an_odd_size_never_splits_a_pixel() {
    use lapix::{export, Bitmap};

    // 5 doesn't halve, so it is padded to 6 first and comes out 3.
    let img = dotted(5, Point::new(0, 0), BLACK);
    let out = export::scale(&img, lapix::Scale::new(1, 2));

    assert_eq!((out.width(), out.height()), (3, 3));
}

#[cfg(feature = "test-utils")]
#[test]
fn sizing_to_a_power_of_two_rounds_each_side_up() {
    use lapix::{export, Bitmap};

    for (from, expected) in [(1, 1), (2, 2), (3, 4), (5, 8), (16, 16), (17, 32)] {
        assert_eq!(export::next_power_of_two(from), expected, "from {from}");
    }

    let img = TestImage::new(Size::new(5, 17), TRANSPARENT);
    let out = export::fit_to_power_of_two(&img);

    assert_eq!((out.width(), out.height()), (8, 32));
}

#[cfg(feature = "test-utils")]
#[test]
fn a_shared_crop_keeps_images_the_same_size() {
    use lapix::{export, Bitmap};

    // Cells of a sheet have to stay aligned, so they are trimmed together.
    let images = vec![
        dotted(16, Point::new(2, 2), BLACK),
        dotted(16, Point::new(9, 6), BLACK),
    ];
    let bounds = export::shared_bounds(&images).expect("both have content");
    let options = lapix::ExportOptions {
        crop: true,
        ..Default::default()
    };

    let sizes: Vec<(i32, i32)> = images
        .iter()
        .map(|image| {
            let out = export::prepare(image, &options, Some(bounds));

            (out.width(), out.height())
        })
        .collect();

    assert_eq!(sizes, vec![(8, 5), (8, 5)]);
}

#[cfg(feature = "test-utils")]
#[test]
fn a_project_starts_with_one_frame() {
    let mut state = State::<TestImage>::new(Size::new(8, 8), None, None);

    assert_eq!(state.frame_count(), 1);
    assert_eq!(state.active_frame(), 0);

    state.execute(Event::AddFrame).unwrap();

    assert_eq!(state.frame_count(), 2);
    // Adding a frame switches to it.
    assert_eq!(state.active_frame(), 1);
}

#[cfg(feature = "test-utils")]
#[test]
fn each_frame_holds_its_own_pixels_but_shares_the_layers() {
    let red = Color::new(255, 0, 0, 255);
    let blue = Color::new(0, 0, 255, 255);
    let dot = Point::new(4, 4);
    let mut state = State::<TestImage>::new(Size::new(8, 8), None, None);

    // Draw red on frame 0.
    state.execute(Event::SetMainColor(red)).unwrap();
    state.execute(Event::BrushStart).unwrap();
    state.execute(Event::BrushStroke(dot)).unwrap();
    state.execute(Event::BrushEnd).unwrap();

    // Draw blue on a new frame 1.
    state.execute(Event::AddFrame).unwrap();
    state.execute(Event::SetMainColor(blue)).unwrap();
    state.execute(Event::BrushStart).unwrap();
    state.execute(Event::BrushStroke(dot)).unwrap();
    state.execute(Event::BrushEnd).unwrap();

    // A layer added while on frame 1 exists on frame 0 too.
    state.execute(Event::NewLayerAbove).unwrap();
    assert_eq!(state.layers().get(0).frame_count(), 2);
    assert_eq!(state.layers().get(1).frame_count(), 2);

    state.execute(Event::SwitchFrame(0)).unwrap();
    assert_eq!(state.visible_pixel(dot), red);

    state.execute(Event::SwitchFrame(1)).unwrap();
    assert_eq!(state.visible_pixel(dot), blue);
}

#[cfg(feature = "test-utils")]
#[test]
fn undo_applies_to_the_frame_the_edit_was_made_on() {
    // The reason atomic actions carry a frame: drawing on one frame, switching
    // away, then undoing must revert the frame that was drawn on, not the one
    // that happens to be active.
    let red = Color::new(255, 0, 0, 255);
    let dot = Point::new(3, 3);
    let mut state = State::<TestImage>::new(Size::new(8, 8), None, None);

    state.execute(Event::AddFrame).unwrap();
    assert_eq!(state.active_frame(), 1);

    // Paint frame 1.
    state.execute(Event::SetMainColor(red)).unwrap();
    state.execute(Event::BrushStart).unwrap();
    state.execute(Event::BrushStroke(dot)).unwrap();
    state.execute(Event::BrushEnd).unwrap();

    // Move to frame 0, then undo the stroke.
    state.execute(Event::SwitchFrame(0)).unwrap();
    state.execute(Event::Undo).unwrap();

    // Frame 1's stroke is gone; frame 0 was never touched.
    state.execute(Event::SwitchFrame(1)).unwrap();
    assert_eq!(state.visible_pixel(dot), TRANSPARENT);
}

#[cfg(feature = "test-utils")]
#[test]
fn a_duplicated_frame_copies_the_pixels_then_diverges() {
    use lapix::Bitmap;

    let red = Color::new(255, 0, 0, 255);
    let dot = Point::new(2, 2);
    let mut state = State::<TestImage>::new(Size::new(8, 8), None, None);

    state.execute(Event::SetMainColor(red)).unwrap();
    state.execute(Event::BrushStart).unwrap();
    state.execute(Event::BrushStroke(dot)).unwrap();
    state.execute(Event::BrushEnd).unwrap();

    state.execute(Event::DuplicateFrame(0)).unwrap();
    assert_eq!(state.frame_count(), 2);
    assert_eq!(state.active_frame(), 1);
    // The copy starts identical.
    assert_eq!(state.visible_pixel(dot), red);

    // Erasing on the copy leaves the original alone.
    state.execute(Event::ClearCanvas).unwrap();
    assert_eq!(state.visible_pixel(dot), TRANSPARENT);
    state.execute(Event::SwitchFrame(0)).unwrap();
    assert_eq!(state.canvas().pixel(dot), red);
}

#[cfg(feature = "test-utils")]
#[test]
fn deleting_a_frame_can_be_undone() {
    let red = Color::new(255, 0, 0, 255);
    let dot = Point::new(5, 5);
    let mut state = State::<TestImage>::new(Size::new(8, 8), None, None);

    // Frame 1 gets a red dot.
    state.execute(Event::AddFrame).unwrap();
    state.execute(Event::SetMainColor(red)).unwrap();
    state.execute(Event::BrushStart).unwrap();
    state.execute(Event::BrushStroke(dot)).unwrap();
    state.execute(Event::BrushEnd).unwrap();

    state.execute(Event::DeleteFrame(1)).unwrap();
    assert_eq!(state.frame_count(), 1);

    state.execute(Event::Undo).unwrap();
    assert_eq!(state.frame_count(), 2);
    // Its pixels come back with it.
    state.execute(Event::SwitchFrame(1)).unwrap();
    assert_eq!(state.visible_pixel(dot), red);
}

#[cfg(feature = "test-utils")]
#[test]
fn the_last_frame_cannot_be_deleted() {
    let mut state = State::<TestImage>::new(Size::new(8, 8), None, None);

    state.execute(Event::DeleteFrame(0)).unwrap();

    assert_eq!(state.frame_count(), 1);
    assert!(
        !state.can_undo(),
        "a refused delete records nothing to undo"
    );
}

#[cfg(feature = "test-utils")]
#[test]
fn layers_start_with_distinct_default_names() {
    let mut state = State::<TestImage>::new(Size::new(4, 4), None, None);

    state.execute(Event::NewLayerAbove).unwrap();
    state.execute(Event::NewLayerAbove).unwrap();

    let names: Vec<&str> = (0..state.layers().count())
        .map(|i| state.layers().get(i).name())
        .collect();

    assert_eq!(names, vec!["Layer 1", "Layer 2", "Layer 3"]);
}

#[cfg(feature = "test-utils")]
#[test]
fn a_new_layer_avoids_a_name_already_in_use() {
    let mut state = State::<TestImage>::new(Size::new(4, 4), None, None);

    // Rename the first layer to what the second would be called by default.
    state
        .execute(Event::RenameLayer(0, "Layer 2".to_owned()))
        .unwrap();
    state.execute(Event::NewLayerAbove).unwrap();

    assert_eq!(state.layers().get(1).name(), "Layer 1");
}

#[cfg(feature = "test-utils")]
#[test]
fn layers_can_be_renamed() {
    let mut state = State::<TestImage>::new(Size::new(4, 4), None, None);

    state
        .execute(Event::RenameLayer(0, "head".to_owned()))
        .unwrap();

    assert_eq!(state.layers().get(0).name(), "head");
}

#[cfg(feature = "test-utils")]
#[test]
fn set_active_cel_image_replaces_pixels_undoably() {
    use lapix::Bitmap;

    let side = 4;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);

    // A generator-style whole-image drop-in: every pixel red.
    let red = Color::new(255, 0, 0, 255);
    let bytes: Vec<u8> = (0..side * side).flat_map(|_| [255, 0, 0, 255]).collect();
    let img = TestImage::from_parts(Size::new(side, side), &bytes);

    state.set_active_cel_image(img).unwrap();
    assert_eq!(state.canvas().pixel(Point::new(2, 2)), red);

    // A single reversible step: undo restores the blank canvas, redo brings the
    // image back.
    assert!(state.can_undo());
    state.execute(Event::Undo).unwrap();
    assert_eq!(state.canvas().pixel(Point::new(2, 2)), TRANSPARENT);

    state.execute(Event::Redo).unwrap();
    assert_eq!(state.canvas().pixel(Point::new(2, 2)), red);
}

#[cfg(feature = "test-utils")]
#[test]
fn a_generator_recipe_and_its_pixels_are_set_and_undone_together() {
    use lapix::{Bitmap, GenValue, Generator};

    let side = 4;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);
    assert!(state.layers().get(0).generator().is_none());

    // Apply a generator: its recipe and the pixels it produced land together.
    let mut generator = Generator::new("pub fn main(w, h, p) { }".to_owned());
    generator.set("radius", GenValue::Float(12.0));
    let red = Color::new(255, 0, 0, 255);
    let bytes: Vec<u8> = (0..side * side).flat_map(|_| [255, 0, 0, 255]).collect();
    let img = TestImage::from_parts(Size::new(side, side), &bytes);

    state
        .set_layer_generator(0, Some(generator.clone()), Some(img))
        .unwrap();
    assert_eq!(state.layers().get(0).generator(), Some(&generator));
    assert_eq!(state.canvas().pixel(Point::new(1, 1)), red);

    // One undo reverts both the recipe and the pixels.
    state.execute(Event::Undo).unwrap();
    assert!(state.layers().get(0).generator().is_none());
    assert_eq!(state.canvas().pixel(Point::new(1, 1)), TRANSPARENT);

    // Redo brings both back.
    state.execute(Event::Redo).unwrap();
    assert_eq!(state.layers().get(0).generator(), Some(&generator));
    assert_eq!(state.canvas().pixel(Point::new(1, 1)), red);
}

#[cfg(feature = "test-utils")]
#[test]
fn bucket_then_erase() {
    let side = 10;
    let mut state = State::<TestImage>::new(Size::new(side, side), None, None);
    state.execute(Event::Bucket(Point::new(0, 0)));
    state.execute(Event::EraseStart);
    state.execute(Event::Erase(Point::new(0, 0)));
    state.execute(Event::Erase(Point::new(side - 1, side - 1)));
    state.execute(Event::EraseEnd);

    for i in 0..side {
        for j in 0..side {
            let color = if i == j { TRANSPARENT } else { BLACK };
            assert_eq!(state.canvas().pixel(Point::new(i, j)), color);
        }
    }
}
