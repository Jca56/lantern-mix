//! Rect cut/split/inset/grid, Row/Col with gaps. `Rect` itself (with `cut_*`,
//! `columns`, `rows`, `inset`, `centered`) lives in lmx_gpu::geom.

use crate::Rect;

/// Split `r` horizontally into slots. A width of `0.0` means "flexible": the
/// remaining space is shared equally between flexible slots.
pub fn hstack(r: Rect, widths: &[f32], gap: f32) -> Vec<Rect> {
    let fixed: f32 = widths.iter().filter(|w| **w > 0.0).sum();
    let flex_n = widths.iter().filter(|w| **w <= 0.0).count().max(1) as f32;
    let free = (r.w - fixed - gap * (widths.len().saturating_sub(1)) as f32).max(0.0);
    let mut x = r.x;
    widths
        .iter()
        .map(|w| {
            let w = if *w > 0.0 { *w } else { free / flex_n };
            let out = Rect::new(x, r.y, w, r.h);
            x += w + gap;
            out
        })
        .collect()
}

/// Vertical counterpart of `hstack`.
pub fn vstack(r: Rect, heights: &[f32], gap: f32) -> Vec<Rect> {
    let fixed: f32 = heights.iter().filter(|h| **h > 0.0).sum();
    let flex_n = heights.iter().filter(|h| **h <= 0.0).count().max(1) as f32;
    let free = (r.h - fixed - gap * (heights.len().saturating_sub(1)) as f32).max(0.0);
    let mut y = r.y;
    heights
        .iter()
        .map(|h| {
            let h = if *h > 0.0 { *h } else { free / flex_n };
            let out = Rect::new(r.x, y, r.w, h);
            y += h + gap;
            out
        })
        .collect()
}

/// Uniform grid of cells inside a rect.
#[derive(Clone, Copy, Debug)]
pub struct Grid {
    pub rect: Rect,
    pub cols: usize,
    pub rows: usize,
    pub gap: f32,
}

impl Grid {
    pub fn new(rect: Rect, cols: usize, rows: usize, gap: f32) -> Self {
        Self { rect, cols: cols.max(1), rows: rows.max(1), gap }
    }
    pub fn cell_size(&self) -> (f32, f32) {
        (
            (self.rect.w - self.gap * (self.cols as f32 - 1.0)) / self.cols as f32,
            (self.rect.h - self.gap * (self.rows as f32 - 1.0)) / self.rows as f32,
        )
    }
    pub fn cell(&self, col: usize, row: usize) -> Rect {
        let (w, h) = self.cell_size();
        Rect::new(self.rect.x + col as f32 * (w + self.gap), self.rect.y + row as f32 * (h + self.gap), w, h)
    }
    /// Cell spanning `cols`×`rows` cells from (`col`,`row`).
    pub fn span(&self, col: usize, row: usize, cols: usize, rows: usize) -> Rect {
        let a = self.cell(col, row);
        let b = self.cell(col + cols.max(1) - 1, row + rows.max(1) - 1);
        Rect::from_min_max(a.x, a.y, b.right(), b.bottom())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hstack_flex_shares_remaining() {
        let r = Rect::new(0.0, 0.0, 100.0, 10.0);
        let s = hstack(r, &[20.0, 0.0, 0.0], 10.0);
        assert_eq!(s[0].w, 20.0);
        assert_eq!(s[1].w, 30.0);
        assert_eq!(s[2].right(), 100.0);
    }

    #[test]
    fn grid_cells_tile_exactly() {
        let g = Grid::new(Rect::new(0.0, 0.0, 100.0, 50.0), 4, 2, 4.0);
        assert_eq!(g.cell(3, 1).right(), 100.0);
        assert_eq!(g.cell(3, 1).bottom(), 50.0);
        assert_eq!(g.span(0, 0, 2, 1).right(), g.cell(1, 0).right());
    }
}
