use crate::color::TRANSPARENT;
use crate::{graphics, Bitmap, Color, FreeImage, Point, Rect, Size};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Effects that certain actions can have on the canvas
#[derive(Debug, Clone, Copy)]
pub enum CanvasEffect {
    /// Action does not affect the canvas at all
    None,
    /// Action forces the canvas to be updated
    Update,
    /// Action forces the canvas to be recreated
    New,
    /// Action forces the layers to be updated
    Layer,
}

/// The canvas is the area where drawing can take place. Each layer has a
/// canvas, and the canvas in turn holds an image internally to represent the
/// drawing on it.
#[derive(Debug, Serialize, Deserialize)]
pub struct Canvas<IMG> {
    inner: IMG,
}

impl<IMG: Bitmap> Canvas<IMG> {
    /// Create a new blank canvas with a specified size
    pub fn new(size: Size<i32>) -> Self {
        Self {
            inner: IMG::new(size, TRANSPARENT),
        }
    }

    /// Get the inner image held by this canvas
    pub fn inner(&self) -> &IMG {
        &self.inner
    }

    /// Get the size of the canvas in pixels. This is the amount of pixels of
    /// the underlying image, not the real screen-size that the canvas takes up
    /// when displayed.
    pub fn size(&self) -> Size<i32> {
        self.inner.size()
    }

    /// Get the width of the canvas in pixels (width of the underlying image,
    /// not real screen-size)
    pub fn width(&self) -> i32 {
        self.inner.width()
    }

    /// Get the height of the canvas in pixels (height of the underlying image)
    pub fn height(&self) -> i32 {
        self.inner.height()
    }

    /// Get the rectangle representing the canvas dimensions, starting from the
    /// origin at (0, 0).
    pub fn rect(&self) -> Rect<i32> {
        Rect::new(0, 0, self.width(), self.height())
    }

    /// Get a representation of the inner image as a slice of bytes
    pub fn bytes(&self) -> &[u8] {
        self.inner.bytes()
    }

    /// Check whether a point is inside the canvas
    pub fn is_in_bounds(&self, p: Point<i32>) -> bool {
        p.x >= 0 && p.y >= 0 && p.x < self.width() && p.y < self.height()
    }

    /// Set the image of the canvas with a predefined one
    pub fn set_img(&mut self, img: IMG) {
        self.inner = img;
    }

    /// Take the inner image replacing it with a dummy empty one.
    pub fn take_inner(&mut self) -> IMG {
        std::mem::replace(&mut self.inner, IMG::new(Size::new(0, 0), TRANSPARENT))
    }

    /// Set all pixels of the canvas to transparent
    pub fn clear(&mut self) -> IMG {
        let size = self.size();
        std::mem::replace(&mut self.inner, IMG::new(size, TRANSPARENT))
    }

    /// Resize the canvas, keeping the content that is in bounds and in case of
    /// an increase in one of the dimensions, set new pixels to transparent.
    pub fn resize(&mut self, size: Size<i32>) -> IMG {
        let new_img = IMG::new(size, TRANSPARENT);
        let old_img = std::mem::replace(&mut self.inner, new_img);
        self.inner.set_from(&old_img);

        old_img
    }

    /// Get the color of a pixel in a certain position in the canvas
    pub fn pixel(&self, p: Point<i32>) -> Color {
        self.inner.pixel(p)
    }

    /// Set the color of a pixel in a certain position in the canvas. If there
    /// was an actual change, return the data needed for a reversal, that is,
    /// which point needs to be set to which color to reverse the action.
    pub fn set_pixel(&mut self, p: Point<i32>, color: Color) -> Option<(Point<i32>, Color)> {
        if self.is_in_bounds(p) {
            let old = self.inner.pixel(p);

            if color == old {
                return None;
            }

            self.inner.set_pixel(p, color);
            return Some((p, old));
        }

        None
    }

    /// Stamp a brush of the given radius centred on a point. Returns a set of
    /// reversals (points and the colors they need to be set to in order to
    /// reverse the action).
    pub fn brush(&mut self, p: Point<i32>, color: Color, radius: u8) -> Vec<(Point<i32>, Color)> {
        let mut reversals = Vec::new();

        for offset in graphics::brush_offsets(radius) {
            if let Some(action) = self.set_pixel(p + offset, color) {
                reversals.push(action);
            }
        }

        reversals
    }

    /// Drag a brush of the given radius from one point to another, stamping
    /// along the way. Returns a set of reversals (points and the colors they
    /// need to be set to in order to reverse the action).
    pub fn brush_line(
        &mut self,
        p1: Point<i32>,
        p2: Point<i32>,
        color: Color,
        radius: u8,
    ) -> Vec<(Point<i32>, Color)> {
        // A single pixel brush is the common case, and going straight to `line`
        // avoids stamping the same pixel repeatedly along the way.
        if radius == 0 {
            return self.line(p1, p2, color);
        }

        let offsets = graphics::brush_offsets(radius);
        let mut painted = HashSet::new();
        let mut reversals = Vec::new();

        for centre in graphics::line(p1, p2) {
            for offset in &offsets {
                let p = centre + *offset;

                // Consecutive stamps overlap heavily; setting a pixel twice
                // would record a reversal back to a color from this same
                // stroke, which would leave the undo incomplete.
                if !painted.insert(p) {
                    continue;
                }

                if let Some(action) = self.set_pixel(p, color) {
                    reversals.push(action);
                }
            }
        }

        reversals
    }

    /// Draw a line between two points in the canvas with a certain color.
    /// Returns a set of reversals (points and the colors they need to be set to
    /// in order to reverse the action).
    pub fn line(
        &mut self,
        p1: Point<i32>,
        p2: Point<i32>,
        color: Color,
    ) -> Vec<(Point<i32>, Color)> {
        let line = graphics::line(p1, p2);
        let mut reversals = Vec::new();

        for p in line {
            if let Some(action) = self.set_pixel(p, color) {
                reversals.push(action);
            }
        }
        reversals
    }

    /// Draw a rectangle (outline) between two points in the canvas with a
    /// certain color. Returns a set of reversals (points and the colors they
    /// need to be set to in order to reverse the action).
    pub fn rectangle(
        &mut self,
        p1: Point<i32>,
        p2: Point<i32>,
        color: Color,
    ) -> Vec<(Point<i32>, Color)> {
        let rect = graphics::rectangle(p1, p2);
        let mut reversals = Vec::new();

        for p in rect {
            if let Some(action) = self.set_pixel(p, color) {
                reversals.push(action);
            }
        }
        reversals
    }

    /// Draw an ellipse (outline) between two points in the canvas with a
    /// certain color. Returns a set of reversals (points and the colors they
    /// need to be set to in order to reverse the action).
    pub fn ellipse(
        &mut self,
        p1: Point<i32>,
        p2: Point<i32>,
        color: Color,
    ) -> Vec<(Point<i32>, Color)> {
        let rect = graphics::ellipse(p1, p2);
        let mut reversals = Vec::new();

        for p in rect {
            if let Some(action) = self.set_pixel(p, color) {
                reversals.push(action);
            }
        }
        reversals
    }

    /// Set an area of the canvas (determined by a rectangle) to a certain
    /// color. Returns a set of reversals (points and colors they need to be set
    /// to in order to reverse the action).
    pub fn set_area(&mut self, area: Rect<i32>, color: Color) -> Vec<(Point<i32>, Color)> {
        let mut reversals = Vec::new();

        for i in 0..area.w {
            for j in 0..area.h {
                if let Some(action) = self.set_pixel((i + area.x, j + area.y).into(), color) {
                    reversals.push(action);
                }
            }
        }

        reversals
    }

    /// Paste a free image into the canvas, overriding the contents that existed
    /// below that area. Returns a set of reversals (points and colors they need
    /// to be set to in order to reverse the action).
    pub fn paste_obj(&mut self, obj: &FreeImage<IMG>) -> Vec<(Point<i32>, Color)> {
        let mut reversals = Vec::new();
        for i in 0..obj.rect.w {
            for j in 0..obj.rect.h {
                let ij = Point::new(i, j);
                let color = obj.texture.pixel(ij);
                let p = ij + obj.rect.pos();

                if self.is_in_bounds(p) {
                    let blended = color.blend_over(self.pixel(p));
                    if let Some(action) = self.set_pixel(p, blended) {
                        reversals.push(action);
                    }
                }
            }
        }

        reversals
    }

    /// Paint an enclosed area with a certain color. Returns a set of reversals
    /// (points and colors they need to be set to in order to reverse the
    /// action).
    pub fn bucket(&mut self, p: Point<i32>, color: Color) -> Vec<(Point<i32>, Color)> {
        let old_color = self.inner.pixel(p);

        if color == old_color {
            return Vec::new();
        }

        let w = self.inner.width() as usize;
        let h = self.inner.height() as usize;

        let mut marked = vec![false; w * h];
        let mut visit = vec![(p.x, p.y)];

        let mut reversals = Vec::new();

        loop {
            if visit.is_empty() {
                break;
            }

            let mut new_visit = Vec::new();
            while let Some((vx, vy)) = visit.pop() {
                marked[(vy as usize) * w + vx as usize] = true;

                if let Some(action) = self.set_pixel((vx, vy).into(), color) {
                    reversals.push(action);
                }

                for (nx, ny) in self.neighbors(vx, vy).into_iter().flatten() {
                    let ind = (ny as usize) * w + nx as usize;
                    if self.inner.pixel((nx, ny).into()) == old_color && !marked[ind] {
                        new_visit.push((nx, ny));
                        marked[ind] = true;
                    }
                }
            }

            visit.append(&mut new_visit);
        }

        reversals
    }

    fn neighbors(&self, x: i32, y: i32) -> [Option<(i32, i32)>; 4] {
        let mut neighbors = [None; 4];
        let w = self.inner.width();
        let h = self.inner.height();

        if x + 1 < w {
            neighbors[0] = Some((x + 1, y));
        }
        if x > 0 {
            neighbors[1] = Some((x - 1, y));
        }
        if y + 1 < h {
            neighbors[2] = Some((x, y + 1));
        }
        if y > 0 {
            neighbors[3] = Some((x, y - 1));
        }

        neighbors
    }

    /// Get an image from a certain area of the canvas (determined by a
    /// rectangle).
    pub fn img_from_area(&self, area: Rect<i32>) -> IMG {
        let mut img = IMG::new((area.w, area.h).into(), TRANSPARENT);

        for i in 0..area.w {
            for j in 0..area.h {
                let ij = Point::new(i, j);
                let p = area.pos() + ij;

                if p.x < self.width() && p.y < self.height() {
                    let color = self.pixel(p);
                    img.set_pixel(ij, color);
                }
            }
        }

        img
    }
}
