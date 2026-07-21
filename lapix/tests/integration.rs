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
