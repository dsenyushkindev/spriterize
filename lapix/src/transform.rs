//! Functions that can be applied to an image, modifying it

use crate::Color;
use crate::{color, Bitmap, Point};
use serde::{Deserialize, Serialize};

/// Weights of the 3x3 neighbourhood used when smoothing, and their total.
/// Leaning on the centre keeps the result close to the original rather than
/// washing it out.
const SMOOTH_KERNEL: [[u32; 3]; 3] = [[1, 2, 1], [2, 4, 2], [1, 2, 1]];

/// The color a pixel takes when smoothed: its 3x3 neighbourhood averaged.
///
/// Colors are weighted by their alpha and the result divided back out. Without
/// that, the fully transparent pixels around a sprite — which are usually
/// transparent *black* — would drag a dark fringe into its edges. Neighbours
/// beyond the edge of the image are left out rather than counted as
/// transparent, so the border doesn't fade away.
pub fn smoothed_pixel<IMG: Bitmap>(image: &IMG, p: Point<i32>) -> Color {
    let mut weighted_alpha = 0;
    let mut premultiplied = [0_u32; 3];
    let mut total_weight = 0;

    for (dj, row) in SMOOTH_KERNEL.iter().enumerate() {
        for (di, weight) in row.iter().enumerate() {
            let (x, y) = (p.x + di as i32 - 1, p.y + dj as i32 - 1);

            if x < 0 || y < 0 || x >= image.width() || y >= image.height() {
                continue;
            }

            let color = image.pixel((x, y).into());
            let alpha = color.a as u32;

            premultiplied[0] += color.r as u32 * alpha * weight;
            premultiplied[1] += color.g as u32 * alpha * weight;
            premultiplied[2] += color.b as u32 * alpha * weight;
            weighted_alpha += alpha * weight;
            total_weight += weight;
        }
    }

    if weighted_alpha == 0 {
        return color::TRANSPARENT;
    }

    let channel = |sum: u32| ((sum + weighted_alpha / 2) / weighted_alpha).min(255) as u8;

    Color::new(
        channel(premultiplied[0]),
        channel(premultiplied[1]),
        channel(premultiplied[2]),
        ((weighted_alpha + total_weight / 2) / total_weight) as u8,
    )
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Transform {
    Identity,
    Silhouete,
    ApplyPalette,
    /// Softens the steps between neighbouring colors
    Smooth,
}

impl Transform {
    /// The filter this applies in one go, at its default settings. `None` for
    /// the transform that does nothing.
    ///
    /// Sharing the implementations means a one-off transform and the filter of
    /// the same name can never drift apart.
    fn as_filter(&self) -> Option<crate::Filter> {
        use crate::Filter;

        match self {
            Self::Identity => None,
            Self::Silhouete => Some(Filter::silhouette()),
            Self::ApplyPalette => Some(Filter::apply_palette()),
            Self::Smooth => Some(Filter::smooth()),
        }
    }

    pub fn apply<IMG: Bitmap>(&self, image: &mut IMG, palette: Vec<Color>) {
        if let Some(filter) = self.as_filter() {
            filter.apply(image, &palette);
        }
    }
}
