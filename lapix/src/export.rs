//! Reshaping an image on its way out: trimming, padding, scaling and sizing to
//! a power of two.

use crate::color::TRANSPARENT;
use crate::{Bitmap, Color, Point, Rect, Size};
use serde::{Deserialize, Serialize};

/// How much bigger or smaller to make an exported image, as a ratio.
///
/// Kept as a ratio rather than a float so the result can be guaranteed to land
/// on whole pixels: scaling up replicates each pixel `up` times, and scaling
/// down folds each `down` by `down` block into one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scale {
    pub up: u32,
    pub down: u32,
}

impl Default for Scale {
    fn default() -> Self {
        Self { up: 1, down: 1 }
    }
}

impl Scale {
    pub fn new(up: u32, down: u32) -> Self {
        Self {
            up: up.max(1),
            down: down.max(1),
        }
    }

    pub fn is_identity(&self) -> bool {
        self.up == self.down
    }
}

/// What to do to an image before writing it out.
///
/// Applied in the order the fields are listed, which is the only order that
/// holds together: trimming changes the size, so padding has to follow it;
/// scaling changes it again, so sizing to a power of two has to come last or
/// the result wouldn't be one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ExportOptions {
    /// Trim fully transparent rows and columns from the edges
    pub crop: bool,
    /// Add this many pixels on every side
    pub padding: u32,
    /// Scale by a whole ratio
    pub scale: Scale,
    /// Grow the width and height to the next power of two
    pub power_of_two: bool,
}

impl ExportOptions {
    /// Whether these options would leave an image exactly as it is
    pub fn is_identity(&self) -> bool {
        !self.crop && self.padding == 0 && self.scale.is_identity() && !self.power_of_two
    }
}

/// The smallest rectangle holding every pixel that isn't fully transparent.
///
/// `None` when the image is entirely transparent, since there is nothing to
/// crop to.
pub fn content_bounds<IMG: Bitmap>(image: &IMG) -> Option<Rect<i32>> {
    let (mut left, mut top) = (i32::MAX, i32::MAX);
    let (mut right, mut bottom) = (i32::MIN, i32::MIN);

    for x in 0..image.width() {
        for y in 0..image.height() {
            if image.pixel(Point::new(x, y)).a == 0 {
                continue;
            }

            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
        }
    }

    (left <= right).then(|| Rect::new(left, top, right - left + 1, bottom - top + 1))
}

/// The union of what several images cover, for trimming a set of them to a
/// common rectangle so they stay aligned with each other.
pub fn shared_bounds<IMG: Bitmap>(images: &[IMG]) -> Option<Rect<i32>> {
    images.iter().filter_map(content_bounds).reduce(|a, b| {
        let (left, top) = (a.x.min(b.x), a.y.min(b.y));
        let right = (a.x + a.w).max(b.x + b.w);
        let bottom = (a.y + a.h).max(b.y + b.h);

        Rect::new(left, top, right - left, bottom - top)
    })
}

/// Copies out the part of an image inside a rectangle. Anything outside the
/// image comes out transparent.
pub fn crop<IMG: Bitmap>(image: &IMG, area: Rect<i32>) -> IMG {
    let mut result = IMG::new((area.w.max(1), area.h.max(1)).into(), TRANSPARENT);

    for x in 0..area.w {
        for y in 0..area.h {
            let from = Point::new(area.x + x, area.y + y);

            if from.x < 0 || from.y < 0 || from.x >= image.width() || from.y >= image.height() {
                continue;
            }

            result.set_pixel(Point::new(x, y), image.pixel(from));
        }
    }

    result
}

/// Adds transparent pixels around an image.
pub fn pad<IMG: Bitmap>(image: &IMG, left: i32, top: i32, right: i32, bottom: i32) -> IMG {
    if left == 0 && top == 0 && right == 0 && bottom == 0 {
        return image.clone();
    }

    let size = Size::new(image.width() + left + right, image.height() + top + bottom);
    let mut result = IMG::new(size, TRANSPARENT);

    for x in 0..image.width() {
        for y in 0..image.height() {
            let to = Point::new(x + left, y + top);

            if to.x < 0 || to.y < 0 || to.x >= size.x || to.y >= size.y {
                continue;
            }

            result.set_pixel(to, image.pixel(Point::new(x, y)));
        }
    }

    result
}

/// Scales by a whole ratio, without ever landing between pixels.
///
/// Scaling up repeats each pixel. Scaling down averages each block into one,
/// weighting colors by their alpha so that fading into transparency doesn't
/// darken the result — and padding first if the size doesn't divide evenly, so
/// no block is ever partial.
pub fn scale<IMG: Bitmap>(image: &IMG, scale: Scale) -> IMG {
    let enlarged = enlarge(image, scale.up as i32);

    reduce(&enlarged, scale.down as i32)
}

fn enlarge<IMG: Bitmap>(image: &IMG, factor: i32) -> IMG {
    if factor <= 1 {
        return image.clone();
    }

    let size = Size::new(image.width() * factor, image.height() * factor);
    let mut result = IMG::new(size, TRANSPARENT);

    for x in 0..image.width() {
        for y in 0..image.height() {
            let color = image.pixel(Point::new(x, y));

            for dx in 0..factor {
                for dy in 0..factor {
                    result.set_pixel(Point::new(x * factor + dx, y * factor + dy), color);
                }
            }
        }
    }

    result
}

fn reduce<IMG: Bitmap>(image: &IMG, factor: i32) -> IMG {
    if factor <= 1 {
        return image.clone();
    }

    // Rounded up to a whole number of blocks first, so every block is complete
    // and no output pixel is an average of a partial one.
    let padded = pad(
        image,
        0,
        0,
        remainder_to(image.width(), factor),
        remainder_to(image.height(), factor),
    );
    let size = Size::new(padded.width() / factor, padded.height() / factor);
    let mut result = IMG::new(size, TRANSPARENT);

    for x in 0..size.x {
        for y in 0..size.y {
            let mut premultiplied = [0_u32; 3];
            let mut alpha = 0;

            for dx in 0..factor {
                for dy in 0..factor {
                    let color = padded.pixel(Point::new(x * factor + dx, y * factor + dy));
                    let a = color.a as u32;

                    premultiplied[0] += color.r as u32 * a;
                    premultiplied[1] += color.g as u32 * a;
                    premultiplied[2] += color.b as u32 * a;
                    alpha += a;
                }
            }

            let count = (factor * factor) as u32;
            let color = if alpha == 0 {
                TRANSPARENT
            } else {
                let channel = |sum: u32| ((sum + alpha / 2) / alpha).min(255) as u8;

                Color::new(
                    channel(premultiplied[0]),
                    channel(premultiplied[1]),
                    channel(premultiplied[2]),
                    ((alpha + count / 2) / count) as u8,
                )
            };

            result.set_pixel(Point::new(x, y), color);
        }
    }

    result
}

/// How much to add to reach the next whole multiple of `factor`.
fn remainder_to(value: i32, factor: i32) -> i32 {
    match value % factor {
        0 => 0,
        left_over => factor - left_over,
    }
}

/// The next power of two at or above `value`, never below one.
pub fn next_power_of_two(value: i32) -> i32 {
    if value <= 1 {
        return 1;
    }

    (value as u32).next_power_of_two() as i32
}

/// Grows an image so both sides are powers of two, adding to the right and
/// bottom so what is already drawn keeps its position.
pub fn fit_to_power_of_two<IMG: Bitmap>(image: &IMG) -> IMG {
    let width = next_power_of_two(image.width());
    let height = next_power_of_two(image.height());

    pad(image, 0, 0, width - image.width(), height - image.height())
}

/// Reshapes an image for export.
///
/// `bounds` overrides what cropping trims to, for exporting several images that
/// have to stay aligned with one another.
pub fn prepare<IMG: Bitmap>(
    image: &IMG,
    options: &ExportOptions,
    bounds: Option<Rect<i32>>,
) -> IMG {
    let mut result = if options.crop {
        match bounds.or_else(|| content_bounds(image)) {
            Some(area) => crop(image, area),
            // Nothing drawn: trimming to nothing would give an empty file, so
            // it is left as it is.
            None => image.clone(),
        }
    } else {
        image.clone()
    };

    if options.padding > 0 {
        let padding = options.padding as i32;
        result = pad(&result, padding, padding, padding, padding);
    }

    if !options.scale.is_identity() {
        result = scale(&result, options.scale);
    }

    if options.power_of_two {
        result = fit_to_power_of_two(&result);
    }

    result
}
