//! Rectangles and 2-D vectors in logical pixels. `Rect` has "cut" helpers so
//! layouts are derived from shared rects instead of magic coordinates.

use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn len(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn dist(self, o: Vec2) -> f32 {
        (self - o).len()
    }
    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }
    pub fn from_angle(a: f32, r: f32) -> Self {
        Self::new(a.cos() * r, a.sin() * r)
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}
impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, s: f32) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}
impl AddAssign for Vec2 {
    fn add_assign(&mut self, o: Vec2) {
        self.x += o.x;
        self.y += o.y;
    }
}
impl SubAssign for Vec2 {
    fn sub_assign(&mut self, o: Vec2) {
        self.x -= o.x;
        self.y -= o.y;
    }
}

/// Axis-aligned rectangle: top-left corner + size, logical px.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const ZERO: Rect = Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 };

    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn from_min_max(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self::new(x0, y0, x1 - x0, y1 - y0)
    }
    pub fn from_center(c: Vec2, w: f32, h: f32) -> Self {
        Self::new(c.x - w * 0.5, c.y - h * 0.5, w, h)
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn min(&self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }
    pub fn max(&self) -> Vec2 {
        Vec2::new(self.right(), self.bottom())
    }
    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.w * 0.5, self.y + self.h * 0.5)
    }
    pub fn size(&self) -> Vec2 {
        Vec2::new(self.w, self.h)
    }
    pub fn is_empty(&self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.y >= self.y && p.x < self.right() && p.y < self.bottom()
    }
    /// Shrink by `d` on every side (negative grows).
    pub fn inset(&self, d: f32) -> Rect {
        self.inset_xy(d, d)
    }
    pub fn inset_xy(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, (self.w - 2.0 * dx).max(0.0), (self.h - 2.0 * dy).max(0.0))
    }
    pub fn translate(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }
    pub fn scaled(&self, s: f32) -> Rect {
        Rect::new(self.x * s, self.y * s, self.w * s, self.h * s)
    }
    pub fn intersect(&self, o: &Rect) -> Rect {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = self.right().min(o.right());
        let y1 = self.bottom().min(o.bottom());
        if x1 <= x0 || y1 <= y0 { Rect::new(x0, y0, 0.0, 0.0) } else { Rect::from_min_max(x0, y0, x1, y1) }
    }
    /// A `w`×`h` rect centered inside this one.
    pub fn centered(&self, w: f32, h: f32) -> Rect {
        Rect::from_center(self.center(), w, h)
    }
    /// Largest square centered inside this rect.
    pub fn square(&self) -> Rect {
        let s = self.w.min(self.h);
        self.centered(s, s)
    }

    // ── cuts: remove a strip from one edge, return it, keep the rest ──
    pub fn cut_top(&mut self, h: f32) -> Rect {
        let h = h.min(self.h);
        let r = Rect::new(self.x, self.y, self.w, h);
        self.y += h;
        self.h -= h;
        r
    }
    pub fn cut_bottom(&mut self, h: f32) -> Rect {
        let h = h.min(self.h);
        self.h -= h;
        Rect::new(self.x, self.y + self.h, self.w, h)
    }
    pub fn cut_left(&mut self, w: f32) -> Rect {
        let w = w.min(self.w);
        let r = Rect::new(self.x, self.y, w, self.h);
        self.x += w;
        self.w -= w;
        r
    }
    pub fn cut_right(&mut self, w: f32) -> Rect {
        let w = w.min(self.w);
        self.w -= w;
        Rect::new(self.x + self.w, self.y, w, self.h)
    }
    /// `n` equal columns with `gap` between them.
    pub fn columns(&self, n: usize, gap: f32) -> impl Iterator<Item = Rect> + '_ {
        let n = n.max(1);
        let w = (self.w - gap * (n as f32 - 1.0)) / n as f32;
        (0..n).map(move |i| Rect::new(self.x + i as f32 * (w + gap), self.y, w, self.h))
    }
    /// `n` equal rows with `gap` between them.
    pub fn rows(&self, n: usize, gap: f32) -> impl Iterator<Item = Rect> + '_ {
        let n = n.max(1);
        let h = (self.h - gap * (n as f32 - 1.0)) / n as f32;
        (0..n).map(move |i| Rect::new(self.x, self.y + i as f32 * (h + gap), self.w, h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_partition_the_rect() {
        let mut r = Rect::new(0.0, 0.0, 100.0, 50.0);
        let top = r.cut_top(10.0);
        assert_eq!(top, Rect::new(0.0, 0.0, 100.0, 10.0));
        assert_eq!(r, Rect::new(0.0, 10.0, 100.0, 40.0));
        let right = r.cut_right(30.0);
        assert_eq!(right, Rect::new(70.0, 10.0, 30.0, 40.0));
        assert_eq!(r, Rect::new(0.0, 10.0, 70.0, 40.0));
    }

    #[test]
    fn columns_share_edges_with_gap() {
        let r = Rect::new(0.0, 0.0, 110.0, 10.0);
        let c: Vec<Rect> = r.columns(3, 10.0).collect();
        assert_eq!(c[0].w, 30.0);
        assert_eq!(c[1].x, 40.0);
        assert_eq!(c[2].right(), 110.0);
    }

    #[test]
    fn contains_and_intersect() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert!(a.contains(Vec2::new(9.9, 0.0)));
        assert!(!a.contains(Vec2::new(10.0, 0.0)));
        assert_eq!(a.intersect(&b), Rect::new(5.0, 5.0, 5.0, 5.0));
        assert!(a.intersect(&Rect::new(20.0, 20.0, 1.0, 1.0)).is_empty());
    }
}
