use crate::{Bitmap, CanvasEffect, Color, Filter, Layer, Layers, Point};
use std::fmt::Debug;

pub type LayerIndex = usize;

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
    SetPixel(LayerIndex, Point<i32>, Color),
    DestroyLayer(LayerIndex),
    CreateLayer(LayerIndex, Layer<IMG>),
    SetLayerCanvas(LayerIndex, IMG),
    SetLayerFilters(LayerIndex, Vec<Filter>),
    SetLayerAdjustment(LayerIndex, bool),
}

impl<IMG> Debug for AtomicAction<IMG> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::SetPixel(i, p, c) => f
                .debug_tuple("SetPixel")
                .field(&i)
                .field(&p)
                .field(&c)
                .finish(),
            Self::DestroyLayer(i) => f.debug_tuple("DestroyLayer").field(&i).finish(),
            Self::CreateLayer(i, _) => f.debug_tuple("CreateLayer").field(&i).finish(),
            Self::SetLayerCanvas(i, _) => f.debug_tuple("SetLayerCanvas").field(&i).finish(),
            Self::SetLayerFilters(i, filters) => f
                .debug_tuple("SetLayerFilters")
                .field(&i)
                .field(&filters)
                .finish(),
            Self::SetLayerAdjustment(i, adjustment) => f
                .debug_tuple("SetLayerAdjustment")
                .field(&i)
                .field(&adjustment)
                .finish(),
        }
    }
}

impl<IMG: Bitmap> AtomicAction<IMG> {
    pub fn set_pixel_vec(i: LayerIndex, values: Vec<(Point<i32>, Color)>) -> Vec<Self> {
        values
            .into_iter()
            .map(|(p, c)| AtomicAction::SetPixel(i, p, c))
            .collect()
    }

    /// Apply this action, returning the atomic action that reverses it.
    ///
    /// The inverse is `None` when applying was a no-op (setting a pixel to the
    /// color it already had, or one that is out of bounds), since there is
    /// nothing to reverse in that case.
    pub fn apply(self, layers: &mut Layers<IMG>) -> (CanvasEffect, Option<AtomicAction<IMG>>) {
        let inverse = match self {
            Self::SetPixel(i, p, color) => layers
                .canvas_at_mut(i)
                .set_pixel(p, color)
                .map(|(p, old_color)| Self::SetPixel(i, p, old_color)),
            Self::DestroyLayer(i) => Some(Self::CreateLayer(i, layers.delete(i))),
            Self::CreateLayer(i, layer) => {
                layers.add_at(i, layer);
                Some(Self::DestroyLayer(i))
            }
            Self::SetLayerCanvas(i, img) => {
                let old_img = layers.canvas_at_mut(i).take_inner();
                layers.canvas_at_mut(i).set_img(img);
                Some(Self::SetLayerCanvas(i, old_img))
            }
            Self::SetLayerFilters(i, filters) => {
                Some(Self::SetLayerFilters(i, layers.set_filters(i, filters)))
            }
            Self::SetLayerAdjustment(i, adjustment) => Some(Self::SetLayerAdjustment(
                i,
                layers.set_adjustment(i, adjustment),
            )),
        };

        (CanvasEffect::Layer, inverse)
    }
}
