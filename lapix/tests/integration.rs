#[cfg(feature = "test-utils")]
use lapix::TestImage;

use lapix::color::{BLACK, TRANSPARENT};
use lapix::{Color, Event, Point, Size, State};

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
