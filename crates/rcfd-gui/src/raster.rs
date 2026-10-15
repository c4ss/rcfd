//! Data-to-pixel raster functions for the field viewport (no UI types), so the mapping can be checked
//! numerically without a screen. `ui` draws with the same colour map, hover and tick functions.

use crate::viewport::{cell_square_size, colormap, GridData, SliceData};

/// Colour of pixels where no data exists.
pub const BACKGROUND: [u8; 4] = [255, 255, 255, 255];

/// An RGBA image, row 0 at the top.
#[derive(Debug, Clone, PartialEq)]
pub struct Rgba {
    /// Width in pixels.
    pub w: usize,
    /// Height in pixels.
    pub h: usize,
    /// Row-major pixels.
    pub px: Vec<[u8; 4]>,
}

impl Rgba {
    /// Image filled with `c`.
    pub fn new(w: usize, h: usize, c: [u8; 4]) -> Rgba {
        Rgba {
            w,
            h,
            px: vec![c; w * h],
        }
    }

    /// Pixel at column `x`, row `y`.
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        self.px[y * self.w + x]
    }

    /// Encode as PNG file bytes.
    pub fn to_png(&self) -> Result<Vec<u8>, String> {
        let mut out = vec![];
        {
            let mut enc = png::Encoder::new(&mut out, self.w as u32, self.h as u32);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut wr = enc.write_header().map_err(|e| e.to_string())?;
            let flat: Vec<u8> = self.px.iter().flatten().copied().collect();
            wr.write_image_data(&flat).map_err(|e| e.to_string())?;
        }
        Ok(out)
    }
}

/// Rasterise a binned grid: each cell is a `cell_px` square of its colour.
pub fn render_grid(g: &GridData, lo: f64, hi: f64, cell_px: usize) -> Rgba {
    let cp = cell_px.max(1);
    let mut img = Rgba::new(g.cols * cp, g.rows * cp, BACKGROUND);
    for r in 0..g.rows {
        for c in 0..g.cols {
            if let Some(v) = g.values[r * g.cols + c] {
                let [a, b, d] = colormap(v, lo, hi);
                for y in r * cp..(r + 1) * cp {
                    for x in c * cp..(c + 1) * cp {
                        img.px[y * img.w + x] = [a, b, d, 255];
                    }
                }
            }
        }
    }
    img
}

/// Rasterise cell-centre samples as squares of [`cell_square_size`] over `extent`
/// `(x0, x1, y0, y1)`; y points up, so the largest y is in the first row.
pub fn render_cells(s: &SliceData, extent: (f64, f64, f64, f64), w: usize, h: usize, lo: f64, hi: f64) -> Rgba {
    let mut img = Rgba::new(w, h, BACKGROUND);
    let (x0, x1, y0, y1) = extent;
    let sx = w as f64 / (x1 - x0).max(1e-12);
    let sy = h as f64 / (y1 - y0).max(1e-12);
    let half = cell_square_size(extent, s.x.len()) / 2.0;
    for i in 0..s.x.len() {
        let [a, b, d] = colormap(s.values[i], lo, hi);
        let px0 = ((s.x[i] - half - x0) * sx).floor().max(0.0) as usize;
        let px1 = (((s.x[i] + half - x0) * sx).ceil().max(0.0) as usize).min(w);
        let py0 = ((y1 - s.y[i] - half) * sy).floor().max(0.0) as usize;
        let py1 = (((y1 - s.y[i] + half) * sy).ceil().max(0.0) as usize).min(h);
        for y in py0..py1 {
            for x in px0..px1 {
                img.px[y * w + x] = [a, b, d, 255];
            }
        }
    }
    img
}

/// Vertical colour bar: top row is `hi`, bottom row is `lo`.
pub fn render_legend(lo: f64, hi: f64, w: usize, h: usize) -> Rgba {
    let mut img = Rgba::new(w, h, BACKGROUND);
    for y in 0..h {
        let t = if h > 1 { 1.0 - y as f64 / (h - 1) as f64 } else { 1.0 };
        let [a, b, d] = colormap(lo + t * (hi - lo), lo, hi);
        for x in 0..w {
            img.px[y * w + x] = [a, b, d, 255];
        }
    }
    img
}

/// Cell under a position given as fractions `(fx, fy)` of the drawn rectangle (origin top-left).
/// Returns `(column, row, value)`; `None` outside the rectangle or on an undefined cell.
pub fn hover_cell(g: &GridData, fx: f64, fy: f64) -> Option<(usize, usize, f64)> {
    if !(0.0..=1.0).contains(&fx) || !(0.0..=1.0).contains(&fy) {
        return None;
    }
    let c = ((fx * g.cols as f64) as usize).min(g.cols - 1);
    let r = ((fy * g.rows as f64) as usize).min(g.rows - 1);
    g.values[r * g.cols + c].map(|v| (c, r, v))
}

/// Position of `t` within `[lo, hi]` as a fraction (0 at `lo`, 1 at `hi`).
pub fn tick_fraction(lo: f64, hi: f64, t: f64) -> f64 {
    (t - lo) / (hi - lo).max(1e-300)
}
