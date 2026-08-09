use crate::layer::Cel;
use crate::{Bitmap, CanvasEffect, Color, Filter, Generator, Layer, Layers, Point};
use std::fmt::Debug;

pub type LayerIndex = usize;
pub type FrameIndex = usize;

pub struct Action<IMG>(Vec<AtomicAction<IMG>>);

impl<IMG> Default for Action<IMG> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<IMG> Debug for Action<IMG> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str("Action([")?;

        for action in self.0.iter() {
            f.write_fmt(format_args!("{:?}, ", action))?;
        }

        f.write_str("])")
    }
}

impl<IMG> From<Vec<AtomicAction<IMG>>> for Action<IMG> {
    fn from(actions: Vec<AtomicAction<IMG>>) -> Self {
        Self(actions)
    }
}

impl<IMG: Bitmap> Action<IMG> {
    pub fn push(&mut self, action: AtomicAction<IMG>) {
        self.0.push(action);
    }

    pub fn append(&mut self, actions: Vec<AtomicAction<IMG>>) {
        for action in actions {
            self.push(action);
        }
    }

    /// Apply every atomic action, in the reverse order they were added, and
    /// return the [`Action`] that reverses this one.
    ///
    /// Applying the returned action puts the layers back the way they were
    /// before this call, which is what makes redo possible: undoing pushes the
    /// returned action onto the redo stack, and redoing pushes its own return
    /// value back onto the undo stack.
    pub fn apply(mut self, layers: &mut Layers<IMG>) -> (CanvasEffect, Action<IMG>) {
        let mut effect = CanvasEffect::None;
        let mut inverse = Action::default();

        // Atomic actions are applied back to front, so their inverses come out
        // in reverse order. That is exactly the order this method expects, so
        // applying `inverse` replays the original actions front to back.
        while let Some(action) = self.0.pop() {
            let (action_effect, action_inverse) = action.apply(layers);
            effect = action_effect;

            if let Some(action_inverse) = action_inverse {
                inverse.push(action_inverse);
            }
        }

        (effect, inverse)
    }

    /// Whether this action has no atomic actions to apply
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub enum AtomicAction<IMG> {
    // Pixel edits name the frame they happened on, so undoing one applies to
    // that frame even if a different one is active by then.
    SetPixel(LayerIndex, FrameIndex, Point<i32>, Color),
    DestroyLayer(LayerIndex),
    CreateLayer(LayerIndex, Layer<IMG>),
    SetLayerCanvas(LayerIndex, FrameIndex, IMG),
    SetLayerFilters(LayerIndex, Vec<Filter>),
    SetLayerGenerator(LayerIndex, Option<Generator>),
    SetLayerAdjustment(LayerIndex, bool),
    // A frame's worth of cels, one per layer in layer order.
    RemoveFrame(FrameIndex),
    InsertFrame(FrameIndex, Vec<Cel<IMG>>),
}

impl<IMG> Debug for AtomicAction<IMG> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::SetPixel(i, frame, p, c) => f
                .debug_tuple("SetPixel")
                .field(&i)
                .field(&frame)
                .field(&p)
                .field(&c)
                .finish(),
            Self::DestroyLayer(i) => f.debug_tuple("DestroyLayer").field(&i).finish(),
            Self::CreateLayer(i, _) => f.debug_tuple("CreateLayer").field(&i).finish(),
            Self::SetLayerCanvas(i, frame, _) => f
                .debug_tuple("SetLayerCanvas")
                .field(&i)
                .field(&frame)
                .finish(),
            Self::SetLayerFilters(i, filters) => f
                .debug_tuple("SetLayerFilters")
                .field(&i)
                .field(&filters)
                .finish(),
            Self::SetLayerGenerator(i, _) => f.debug_tuple("SetLayerGenerator").field(&i).finish(),
            Self::SetLayerAdjustment(i, adjustment) => f
                .debug_tuple("SetLayerAdjustment")
                .field(&i)
                .field(&adjustment)
                .finish(),
            Self::RemoveFrame(frame) => f.debug_tuple("RemoveFrame").field(&frame).finish(),
            Self::InsertFrame(frame, _) => f.debug_tuple("InsertFrame").field(&frame).finish(),
        }
    }
}

impl<IMG: Bitmap> AtomicAction<IMG> {
    pub fn set_pixel_vec(
        i: LayerIndex,
        frame: FrameIndex,
        values: Vec<(Point<i32>, Color)>,
    ) -> Vec<Self> {
        values
            .into_iter()
            .map(|(p, c)| AtomicAction::SetPixel(i, frame, p, c))
            .collect()
    }

    /// Apply this action, returning the atomic action that reverses it.
    ///
    /// The inverse is `None` when applying was a no-op (setting a pixel to the
    /// color it already had, or one that is out of bounds), since there is
    /// nothing to reverse in that case.
    pub fn apply(self, layers: &mut Layers<IMG>) -> (CanvasEffect, Option<AtomicAction<IMG>>) {
        let inverse = match self {
            Self::SetPixel(i, frame, p, color) => layers
                .cel_mut(i, frame)
                .set_pixel(p, color)
                .map(|(p, old_color)| Self::SetPixel(i, frame, p, old_color)),
            Self::DestroyLayer(i) => Some(Self::CreateLayer(i, layers.delete(i))),
            Self::CreateLayer(i, layer) => {
                layers.add_at(i, layer);
                Some(Self::DestroyLayer(i))
            }
            Self::SetLayerCanvas(i, frame, img) => {
                let old_img = layers.set_cel_img(i, frame, img);
                Some(Self::SetLayerCanvas(i, frame, old_img))
            }
            Self::SetLayerFilters(i, filters) => {
                Some(Self::SetLayerFilters(i, layers.set_filters(i, filters)))
            }
            Self::SetLayerGenerator(i, generator) => Some(Self::SetLayerGenerator(
                i,
                layers.set_generator(i, generator),
            )),
            Self::SetLayerAdjustment(i, adjustment) => Some(Self::SetLayerAdjustment(
                i,
                layers.set_adjustment(i, adjustment),
            )),
            Self::RemoveFrame(frame) => layers
                .remove_frame(frame)
                .map(|cels| Self::InsertFrame(frame, cels)),
            Self::InsertFrame(frame, cels) => {
                layers.insert_frame(frame, cels);
                Some(Self::RemoveFrame(frame))
            }
        };

        (CanvasEffect::Layer, inverse)
    }
}
