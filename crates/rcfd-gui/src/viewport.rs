//! Field viewport data and colour mapping (no UI code).

use serde_json::Value;

/// A field averaged onto a regular grid (first row is the top).
#[derive(Debug, Clone, PartialEq)]
pub struct GridData {
    /// Field name.
    pub name: String,
    /// Columns.
    pub cols: usize,
    /// Rows.
    pub rows: usize,
    /// Row-major values, `None` where no cell falls in the bin.
    pub values: Vec<Option<f64>>,
    /// Minimum over defined values.
    pub min: f64,
    /// Maximum over defined values.
    pub max: f64,
}

impl GridData {
    /// Parse the result of `field.grid`.
    pub fn from_json(v: &Value) -> Option<GridData> {
        let cols = v["cols"].as_u64()? as usize;
        let rows = v["rows"].as_u64()? as usize;
        let values: Vec<Option<f64>> = v["values"].as_array()?.iter().map(Value::as_f64).collect();
        if cols == 0 || rows == 0 || values.len() != cols * rows {
            return None;
        }
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for x in values.iter().flatten() {
            lo = lo.min(*x);
            hi = hi.max(*x);
        }
        if !lo.is_finite() {
            lo = 0.0;
            hi = 0.0;
        }
        Some(GridData {
            name: v["name"].as_str().unwrap_or_default().to_string(),
            cols,
            rows,
            values,
            min: lo,
            max: hi,
        })
    }
}

/// Cell-centre samples from `field.slice`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SliceData {
    /// x coordinates.
    pub x: Vec<f64>,
    /// y coordinates.
    pub y: Vec<f64>,
    /// Field values.
    pub values: Vec<f64>,
}

impl SliceData {
    /// Parse the result of `field.slice`.
    pub fn from_json(v: &Value) -> Option<SliceData> {
        let arr = |k: &str| -> Option<Vec<f64>> {
            v[k].as_array()
                .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect())
        };
        let (x, y, values) = (arr("x")?, arr("y")?, arr("values")?);
        if x.len() != y.len() || x.len() != values.len() {
            return None;
        }
        Some(SliceData { x, y, values })
    }

    /// Bounding box `(x0, x1, y0, y1)` of the sample positions.
    pub fn extent(&self) -> Option<(f64, f64, f64, f64)> {
        if self.x.is_empty() {
            return None;
        }
        let f = |v: &[f64]| {
            v.iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), x| (l.min(*x), h.max(*x)))
        };
        let (x0, x1) = f(&self.x);
        let (y0, y1) = f(&self.y);
        Some((x0, x1, y0, y1))
    }

    /// Width / height of the sample bounding box (1 when degenerate).
    pub fn aspect(&self) -> f64 {
        match self.extent() {
            Some((x0, x1, y0, y1)) if y1 - y0 > 1e-12 && x1 - x0 > 1e-12 => (x1 - x0) / (y1 - y0),
            _ => 1.0,
        }
    }
}

/// How the viewport draws the field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// Binned raster.
    Raster,
    /// One square per cell centre.
    Cells,
}

/// Colour range policy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Range {
    /// Fit to the data.
    Auto,
    /// User supplied.
    Fixed(f64, f64),
}

/// Viewport settings.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewState {
    /// Field to show.
    pub field: String,
    /// Draw mode.
    pub mode: ViewMode,
    /// Colour range.
    pub range: Range,
    /// Raster resolution (columns).
    pub cols: usize,
}

impl Default for ViewState {
    fn default() -> Self {
        ViewState {
            field: "speed".into(),
            mode: ViewMode::Raster,
            range: Range::Auto,
            cols: 64,
        }
    }
}

impl ViewState {
    /// Rows for the raster given the domain aspect ratio (width / height).
    pub fn rows_for(&self, aspect: f64) -> usize {
        ((self.cols as f64 / aspect.clamp(0.05, 20.0)).round() as usize).clamp(2, 100)
    }

    /// Effective colour range for data spanning `[lo, hi]`.
    pub fn effective_range(&self, lo: f64, hi: f64) -> (f64, f64) {
        match self.range {
            Range::Fixed(a, b) if b > a => (a, b),
            _ => {
                if hi > lo {
                    (lo, hi)
                } else {
                    (lo - 0.5, lo + 0.5)
                }
            }
        }
    }
}

/// Map `v` in `[lo, hi]` to a colour (a perceptually ordered dark-blue to yellow ramp).
pub fn colormap(v: f64, lo: f64, hi: f64) -> [u8; 3] {
    // Control points of a viridis-like ramp.
    const STOPS: [[f64; 3]; 6] = [
        [68.0, 1.0, 84.0],
        [65.0, 68.0, 135.0],
        [42.0, 120.0, 142.0],
        [34.0, 168.0, 132.0],
        [122.0, 209.0, 81.0],
        [253.0, 231.0, 37.0],
    ];
    let t = if hi > lo {
        ((v - lo) / (hi - lo)).clamp(0.0, 1.0)
    } else {
        0.5
    };
    let t = if t.is_nan() { 0.0 } else { t };
    let pos = t * (STOPS.len() - 1) as f64;
    let i = (pos.floor() as usize).min(STOPS.len() - 2);
    let f = pos - i as f64;
    let mut out = [0u8; 3];
    for (k, o) in out.iter_mut().enumerate() {
        *o = (STOPS[i][k] + (STOPS[i + 1][k] - STOPS[i][k]) * f).round() as u8;
    }
    out
}

/// "Nice" tick values for a legend over `[lo, hi]` (about `n` ticks).
pub fn legend_ticks(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    if hi.is_nan() || lo.is_nan() || hi <= lo || n < 2 {
        return vec![lo];
    }
    let raw = (hi - lo) / (n - 1) as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0]
        .iter()
        .map(|m| m * mag)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * mag);
    let mut t = (lo / step).ceil() * step;
    let mut out = vec![];
    while t <= hi + step * 1e-9 {
        out.push(if t.abs() < step * 1e-9 { 0.0 } else { t });
        t += step;
    }
    out
}

/// Side length (in domain units) of the squares drawn for `n` cell centres over `extent`.
pub fn cell_square_size(extent: (f64, f64, f64, f64), n: usize) -> f64 {
    let (x0, x1, y0, y1) = extent;
    let area = ((x1 - x0) * (y1 - y0)).max(1e-30);
    (area / n.max(1) as f64).sqrt()
}

/// Relative luma (Rec. 709 weights) of an 8-bit colour, in 0..=255.
pub fn luminance(c: [u8; 3]) -> f64 {
    0.2126 * f64::from(c[0]) + 0.7152 * f64::from(c[1]) + 0.0722 * f64::from(c[2])
}
