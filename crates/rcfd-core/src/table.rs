//! Lookup tables and profile data (F-105).
//!
//! A [`Table`] maps one to three input coordinates to one or more output
//! columns. It is read from a small CSV text:
//!
//! ```text
//! # comment lines start with '#'
//! T [K], nu [m^2/s]
//! 273.15, 1.79e-6
//! 293.15, 1.00e-6
//! ```
//!
//! * the first line names the columns; a unit may follow the name in square
//!   brackets and values are converted to SI when the table is built (so
//!   `degC` columns work); a column without a unit keeps its raw numbers and
//!   has no declared dimension (the case layer then assumes the SI unit of the
//!   quantity a profile boundary needs, and refuses to use an input column
//!   without unit inside a dimension-checked expression);
//! * the delimiter is a comma, semicolon or tab;
//! * one column per input coordinate (as named by the caller) and any number
//!   of output columns.
//!
//! Interpolation is multilinear on a rectilinear grid (a one-input table is
//! piecewise linear; a grid needs every combination of the axis values exactly
//! once, in any row order). Rows that do not form a grid are treated as
//! scattered points and interpolated by inverse-distance weighting of the
//! nearest points (power 2); this is a smooth, parameter-free profile
//! interpolant, not a high-order one. Queries outside the data range either
//! return NaN ([`Extrapolation::Error`], the default; callers turn it into an
//! error) or the nearest edge value ([`Extrapolation::Clamp`]).
//!
//! Everything here is original code written from textbook definitions of
//! linear interpolation and Shepard's inverse-distance weighting.

use crate::units::{Dimension, Unit};
use std::fmt;

/// What to do for queries outside the range of the data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Extrapolation {
    /// Return NaN so the caller can report the missing data (default).
    #[default]
    Error,
    /// Use the value at the nearest edge.
    Clamp,
}

impl Extrapolation {
    /// Parse `"error"` or `"clamp"`.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "error" => Some(Extrapolation::Error),
            "clamp" => Some(Extrapolation::Clamp),
            _ => None,
        }
    }
}

/// Problem found while reading or building a table.
#[derive(Debug, Clone, PartialEq)]
pub struct TableError(pub String);

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TableError {}

fn err<T>(m: impl Into<String>) -> Result<T, TableError> {
    Err(TableError(m.into()))
}

/// Description of how to read a CSV text.
#[derive(Debug, Clone, Default)]
pub struct CsvSpec {
    /// Names of the input (coordinate) columns, in argument order (1 to 3).
    pub inputs: Vec<String>,
    /// Output columns to keep (empty = every other column).
    pub outputs: Vec<String>,
    /// Units by column name for columns that carry none in the header.
    pub units: Vec<(String, String)>,
    /// Out-of-range behaviour.
    pub extrapolation: Extrapolation,
}

/// A named column with its (optional) dimension.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    /// Column name.
    pub name: String,
    /// Dimension when a unit was declared.
    pub dim: Option<Dimension>,
}

#[derive(Debug, Clone, PartialEq)]
enum Data {
    /// Rectilinear grid: sorted axes, `vals[output][flat]` with the last axis fastest.
    Grid { axes: Vec<Vec<f64>>, vals: Vec<Vec<f64>> },
    /// Scattered points: `pts[i]` has one coordinate per input.
    Scatter {
        pts: Vec<Vec<f64>>,
        vals: Vec<Vec<f64>>,
        lo: Vec<f64>,
        hi: Vec<f64>,
    },
}

/// An interpolation table (values in SI where units were declared).
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    /// Name of the table (for messages).
    pub name: String,
    /// Input columns.
    pub inputs: Vec<Column>,
    /// Output columns.
    pub outputs: Vec<Column>,
    extrapolation: Extrapolation,
    data: Data,
}

fn split_cells(line: &str) -> Vec<&str> {
    let d = if line.contains(',') {
        ','
    } else if line.contains(';') {
        ';'
    } else {
        '\t'
    };
    line.split(d).map(str::trim).collect()
}

/// `"T [K]"` -> (`"T"`, `Some("K")`).
fn split_header(cell: &str) -> (String, Option<String>) {
    match (cell.find('['), cell.rfind(']')) {
        (Some(a), Some(b)) if b > a => (cell[..a].trim().to_string(), Some(cell[a + 1..b].trim().to_string())),
        _ => (cell.trim().to_string(), None),
    }
}

impl Table {
    /// Read a table from CSV text. `name` is used in error messages.
    pub fn from_csv(name: &str, text: &str, spec: &CsvSpec) -> Result<Table, TableError> {
        let w = |m: String| TableError(format!("table '{name}': {m}"));
        if spec.inputs.is_empty() || spec.inputs.len() > 3 {
            return err(format!("table '{name}': 1 to 3 input columns are required"));
        }
        let mut lines = text
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l.trim()))
            .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'));
        let Some((_, header)) = lines.next() else {
            return Err(w("the file has no header line".into()));
        };
        let cols: Vec<(String, Option<String>)> = split_cells(header).into_iter().map(split_header).collect();
        for (i, (c, _)) in cols.iter().enumerate() {
            if c.is_empty() {
                return Err(w(format!("header column {} has no name", i + 1)));
            }
            if cols[..i].iter().any(|(o, _)| o == c) {
                return Err(w(format!("duplicate column '{c}'")));
            }
        }
        let idx_of = |n: &str| cols.iter().position(|(c, _)| c == n);
        let mut in_idx = Vec::new();
        for n in &spec.inputs {
            match idx_of(n) {
                Some(i) => in_idx.push(i),
                None => {
                    let have: Vec<&str> = cols.iter().map(|c| c.0.as_str()).collect();
                    return Err(w(format!(
                        "input column '{n}' not found (columns: {})",
                        have.join(", ")
                    )));
                }
            }
        }
        let out_names: Vec<String> = if spec.outputs.is_empty() {
            cols.iter()
                .map(|c| c.0.clone())
                .filter(|c| !spec.inputs.contains(c))
                .collect()
        } else {
            spec.outputs.clone()
        };
        if out_names.is_empty() {
            return Err(w("there is no output column".into()));
        }
        let mut out_idx = Vec::new();
        for n in &out_names {
            match idx_of(n) {
                Some(i) if !in_idx.contains(&i) => out_idx.push(i),
                Some(_) => return Err(w(format!("column '{n}' is an input and cannot also be an output"))),
                None => return Err(w(format!("output column '{n}' not found"))),
            }
        }
        // units: header first, then the spec
        let unit_for = |col: usize| -> Result<(Option<Unit>, Option<Dimension>), TableError> {
            let fallback = spec.units.iter().find(|(n, _)| *n == cols[col].0).map(|(_, u)| u);
            let u = cols[col].1.as_ref().or(fallback).filter(|s| !s.is_empty());
            match u {
                None => Ok((None, None)),
                Some(s) => {
                    let unit =
                        Unit::parse(s).map_err(|e| w(format!("column '{}': bad unit '{s}': {e}", cols[col].0)))?;
                    Ok((Some(unit), Some(unit.dim)))
                }
            }
        };
        let mut in_units = Vec::new();
        for &i in &in_idx {
            in_units.push(unit_for(i)?);
        }
        let mut out_units = Vec::new();
        for &i in &out_idx {
            out_units.push(unit_for(i)?);
        }
        // rows
        let nin = in_idx.len();
        let mut rows: Vec<(Vec<f64>, Vec<f64>)> = Vec::new();
        for (ln, l) in lines {
            let cells = split_cells(l);
            if cells.len() != cols.len() {
                return Err(w(format!(
                    "line {ln}: expected {} values, found {}",
                    cols.len(),
                    cells.len()
                )));
            }
            let num = |c: usize, u: &Option<Unit>| -> Result<f64, TableError> {
                let v: f64 = cells[c].parse().map_err(|_| {
                    w(format!(
                        "line {ln}: '{}' in column '{}' is not a number",
                        cells[c], cols[c].0
                    ))
                })?;
                if !v.is_finite() {
                    return Err(w(format!("line {ln}: column '{}' is not finite", cols[c].0)));
                }
                Ok(u.map_or(v, |u| u.to_si(v)))
            };
            let mut x = Vec::with_capacity(nin);
            for (k, &c) in in_idx.iter().enumerate() {
                x.push(num(c, &in_units[k].0)?);
            }
            let mut y = Vec::with_capacity(out_idx.len());
            for (k, &c) in out_idx.iter().enumerate() {
                y.push(num(c, &out_units[k].0)?);
            }
            rows.push((x, y));
        }
        if rows.len() < 2 {
            return Err(w(format!("at least 2 data rows are required, found {}", rows.len())));
        }
        let nout = out_idx.len();
        let data = build_data(name, nin, nout, rows)?;
        Ok(Table {
            name: name.to_string(),
            inputs: spec
                .inputs
                .iter()
                .zip(&in_units)
                .map(|(n, u)| Column {
                    name: n.clone(),
                    dim: u.1,
                })
                .collect(),
            outputs: out_names
                .iter()
                .zip(&out_units)
                .map(|(n, u)| Column {
                    name: n.clone(),
                    dim: u.1,
                })
                .collect(),
            extrapolation: spec.extrapolation,
            data,
        })
    }

    /// Number of inputs.
    pub fn n_inputs(&self) -> usize {
        self.inputs.len()
    }

    /// Index of an output column by name.
    pub fn output_index(&self, name: &str) -> Option<usize> {
        self.outputs.iter().position(|c| c.name == name)
    }

    /// `(min, max)` of every input coordinate.
    pub fn range(&self) -> Vec<(f64, f64)> {
        match &self.data {
            Data::Grid { axes, .. } => axes.iter().map(|a| (a[0], a[a.len() - 1])).collect(),
            Data::Scatter { lo, hi, .. } => lo.iter().copied().zip(hi.iter().copied()).collect(),
        }
    }

    /// Whether the data form a rectilinear grid (otherwise scattered points).
    pub fn is_grid(&self) -> bool {
        matches!(self.data, Data::Grid { .. })
    }

    /// Number of data rows.
    pub fn len(&self) -> usize {
        match &self.data {
            Data::Grid { vals, .. } => vals[0].len(),
            Data::Scatter { pts, .. } => pts.len(),
        }
    }

    /// Always false: a table has at least two rows.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Whether `x` lies inside the data range (within a tiny tolerance).
    pub fn contains(&self, x: &[f64]) -> bool {
        self.range().iter().zip(x).all(|(&(lo, hi), &v)| {
            let eps = 1e-9 * (hi - lo).abs() + 1e-12;
            v.is_finite() && v >= lo - eps && v <= hi + eps
        })
    }

    /// Interpolated value of output `out` at `x` (SI). NaN when `x` is outside the range and the
    /// policy is [`Extrapolation::Error`].
    pub fn eval(&self, out: usize, x: &[f64]) -> f64 {
        if x.len() != self.inputs.len() || x.iter().any(|v| !v.is_finite()) {
            return f64::NAN;
        }
        let inside = self.contains(x);
        if !inside && self.extrapolation == Extrapolation::Error {
            return f64::NAN;
        }
        let rng = self.range();
        let q: Vec<f64> = x.iter().zip(&rng).map(|(&v, &(lo, hi))| v.clamp(lo, hi)).collect();
        match &self.data {
            Data::Grid { axes, vals } => grid_eval(axes, &vals[out], &q),
            Data::Scatter { pts, vals, .. } => scatter_eval(pts, &vals[out], &q),
        }
    }
}

fn build_data(name: &str, nin: usize, nout: usize, rows: Vec<(Vec<f64>, Vec<f64>)>) -> Result<Data, TableError> {
    let w = |m: String| TableError(format!("table '{name}': {m}"));
    // distinct sorted axes
    let mut axes: Vec<Vec<f64>> = (0..nin)
        .map(|k| {
            let mut a: Vec<f64> = rows.iter().map(|r| r.0[k]).collect();
            a.sort_by(f64::total_cmp);
            a.dedup();
            a
        })
        .collect();
    let cells: usize = axes.iter().map(Vec::len).product();
    if nin == 1 {
        if axes[0].len() != rows.len() {
            return Err(w(
                "the input column has repeated values; a 1-D table needs strictly increasing inputs".into(),
            ));
        }
        if axes[0].len() < 2 {
            return Err(w("at least 2 distinct input values are required".into()));
        }
    }
    let is_grid = axes.iter().all(|a| a.len() >= 2) && cells == rows.len();
    if is_grid {
        let mut vals = vec![vec![f64::NAN; cells]; nout];
        let mut seen = vec![false; cells];
        for (x, y) in &rows {
            let mut flat = 0usize;
            for k in 0..nin {
                let i = axes[k]
                    .binary_search_by(|v| v.total_cmp(&x[k]))
                    .expect("axis built from rows");
                flat = flat * axes[k].len() + i;
            }
            if seen[flat] {
                // cannot happen when cells == rows.len() and every cell is hit, but a duplicate means a missing one
                return Err(w(format!("duplicate grid point {x:?}")));
            }
            seen[flat] = true;
            for (o, v) in y.iter().enumerate() {
                vals[o][flat] = *v;
            }
        }
        if seen.iter().any(|s| !s) {
            return Err(w("grid data are incomplete".into()));
        }
        return Ok(Data::Grid { axes, vals });
    }
    if nin == 1 {
        return Err(w("at least 2 distinct input values are required".into()));
    }
    // scattered: duplicates are an error (ambiguous values)
    let mut pts: Vec<Vec<f64>> = rows.iter().map(|r| r.0.clone()).collect();
    let mut order: Vec<usize> = (0..pts.len()).collect();
    order.sort_by(|&a, &b| {
        pts[a]
            .iter()
            .zip(&pts[b])
            .map(|(p, q)| p.total_cmp(q))
            .find(|o| o.is_ne())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for pair in order.windows(2) {
        if pts[pair[0]] == pts[pair[1]] {
            return Err(w(format!("duplicate point {:?}", pts[pair[0]])));
        }
    }
    let lo: Vec<f64> = axes.iter_mut().map(|a| a[0]).collect();
    let hi: Vec<f64> = axes.iter().map(|a| a[a.len() - 1]).collect();
    let vals: Vec<Vec<f64>> = (0..nout).map(|o| rows.iter().map(|r| r.1[o]).collect()).collect();
    pts.shrink_to_fit();
    Ok(Data::Scatter { pts, vals, lo, hi })
}

fn grid_eval(axes: &[Vec<f64>], vals: &[f64], q: &[f64]) -> f64 {
    let n = axes.len();
    // lower index and fraction per axis
    let mut lo = [0usize; 3];
    let mut fr = [0.0f64; 3];
    for k in 0..n {
        let a = &axes[k];
        let i = match a.binary_search_by(|v| v.total_cmp(&q[k])) {
            Ok(i) => i.min(a.len() - 2),
            Err(i) => i.saturating_sub(1).min(a.len() - 2),
        };
        lo[k] = i;
        fr[k] = (q[k] - a[i]) / (a[i + 1] - a[i]);
    }
    let mut sum = 0.0;
    for corner in 0..(1usize << n) {
        let mut wgt = 1.0;
        let mut flat = 0usize;
        for k in 0..n {
            let up = (corner >> k) & 1;
            wgt *= if up == 1 { fr[k] } else { 1.0 - fr[k] };
            flat = flat * axes[k].len() + lo[k] + up;
        }
        if wgt != 0.0 {
            sum += wgt * vals[flat];
        }
    }
    sum
}

fn scatter_eval(pts: &[Vec<f64>], vals: &[f64], q: &[f64]) -> f64 {
    let k = if q.len() == 2 { 6 } else { 8 }.min(pts.len());
    let mut d: Vec<(f64, usize)> = pts
        .iter()
        .enumerate()
        .map(|(i, p)| (p.iter().zip(q).map(|(a, b)| (a - b) * (a - b)).sum::<f64>(), i))
        .collect();
    d.sort_by(|a, b| a.0.total_cmp(&b.0));
    if d[0].0 == 0.0 {
        return vals[d[0].1];
    }
    let (mut num, mut den) = (0.0, 0.0);
    for &(d2, i) in d.iter().take(k) {
        let w = 1.0 / d2;
        num += w * vals[i];
        den += w;
    }
    num / den
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(inputs: &[&str]) -> CsvSpec {
        CsvSpec {
            inputs: inputs.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn one_d_linear_with_units_and_comments() {
        let t = Table::from_csv(
            "nu",
            "# water\nT [degC], nu [m^2/s]\n0, 1.79e-6\n20, 1.00e-6\n40, 0.66e-6\n",
            &spec(&["T"]),
        )
        .unwrap();
        assert_eq!(t.inputs[0].dim, Some(Dimension::TEMPERATURE));
        assert_eq!(t.outputs[0].dim, Some(Dimension::KINEMATIC_VISCOSITY));
        // 10 degC = 283.15 K, halfway between 1.79e-6 and 1.00e-6
        let v = t.eval(0, &[283.15]);
        assert!((v - 1.395e-6).abs() < 1e-15, "{v}");
        assert_eq!(t.eval(0, &[293.15]), 1.00e-6);
        assert!(t.eval(0, &[250.0]).is_nan());
        assert!(t.eval(0, &[400.0]).is_nan());
    }

    #[test]
    fn clamp_policy_and_edges() {
        let mut s = spec(&["x"]);
        s.extrapolation = Extrapolation::Clamp;
        let t = Table::from_csv("a", "x,y\n0,1\n1,3\n", &s).unwrap();
        assert_eq!(t.eval(0, &[-5.0]), 1.0);
        assert_eq!(t.eval(0, &[9.0]), 3.0);
        assert_eq!(t.eval(0, &[0.5]), 2.0);
        // exactly at the ends is inside even with the error policy
        let t = Table::from_csv("a", "x,y\n0,1\n1,3\n", &spec(&["x"])).unwrap();
        assert_eq!(t.eval(0, &[1.0]), 3.0);
        assert_eq!(t.eval(0, &[0.0]), 1.0);
    }

    #[test]
    fn unsorted_rows_and_two_outputs() {
        let t = Table::from_csv("a", "x,p,q\n2,20,200\n0,0,0\n1,10,100\n", &spec(&["x"])).unwrap();
        assert_eq!(t.outputs.len(), 2);
        assert_eq!(t.eval(t.output_index("q").unwrap(), &[1.5]), 150.0);
    }

    #[test]
    fn bilinear_grid_reproduces_a_bilinear_function() {
        // f = 1 + 2x + 3y + 4xy, rows in scrambled order
        let mut csv = String::from("x,y,f\n");
        for &(x, y) in &[(1.0, 1.0), (0.0, 0.0), (0.0, 1.0), (1.0, 0.0), (2.0, 0.0), (2.0, 1.0)] {
            csv.push_str(&format!("{x},{y},{}\n", 1.0 + 2.0 * x + 3.0 * y + 4.0 * x * y));
        }
        let t = Table::from_csv("g", &csv, &spec(&["x", "y"])).unwrap();
        assert!(t.is_grid());
        let (x, y) = (1.3, 0.7);
        let want = 1.0 + 2.0 * x + 3.0 * y + 4.0 * x * y;
        assert!((t.eval(0, &[x, y]) - want).abs() < 1e-12);
        assert!(t.eval(0, &[2.5, 0.5]).is_nan());
    }

    #[test]
    fn trilinear_grid() {
        let mut csv = String::from("x,y,z,f\n");
        for x in [0.0, 1.0] {
            for y in [0.0, 2.0] {
                for z in [0.0, 4.0] {
                    csv.push_str(&format!("{x},{y},{z},{}\n", x + 2.0 * y + 3.0 * z));
                }
            }
        }
        let t = Table::from_csv("g", &csv, &spec(&["x", "y", "z"])).unwrap();
        let v = t.eval(0, &[0.25, 0.5, 1.0]);
        assert!((v - (0.25 + 1.0 + 3.0)).abs() < 1e-12, "{v}");
    }

    #[test]
    fn scattered_points_interpolate_and_hit_data_exactly() {
        let t = Table::from_csv(
            "s",
            "x,y,f\n0,0,1\n1,0,2\n0,1,3\n0.5,0.7,9\n1,1.2,4\n",
            &spec(&["x", "y"]),
        )
        .unwrap();
        assert!(!t.is_grid());
        assert_eq!(t.eval(0, &[0.5, 0.7]), 9.0);
        let v = t.eval(0, &[0.4, 0.5]);
        assert!(v > 1.0 && v < 9.0);
        assert!(t.eval(0, &[3.0, 0.0]).is_nan());
    }

    #[test]
    fn bad_input_is_reported() {
        let e = |csv: &str, s: &CsvSpec| Table::from_csv("t", csv, s).unwrap_err().0;
        assert!(e("", &spec(&["x"])).contains("no header"));
        assert!(e("x,y\n1,2\n", &spec(&["x"])).contains("at least 2 data rows"));
        assert!(e("x,y\n1,2\n2,a\n", &spec(&["x"])).contains("not a number"));
        assert!(e("x,y\n1,2\n2\n", &spec(&["x"])).contains("expected 2 values"));
        assert!(e("x,y\n1,2\n1,3\n", &spec(&["x"])).contains("strictly increasing"));
        assert!(e("x,y\n1,2\n2,3\n", &spec(&["z"])).contains("input column 'z' not found"));
        assert!(e("x,y [furlong]\n1,2\n2,3\n", &spec(&["x"])).contains("bad unit"));
        assert!(e("x,y\n1,NaN\n2,3\n", &spec(&["x"])).contains("not finite"));
        assert!(e("x,y\n1,2\n2,3\n", &spec(&[])).contains("input columns"));
        assert!(e("x,x\n1,2\n2,3\n", &spec(&["x"])).contains("duplicate column"));
        // incomplete grid falls back to scatter only when 2-D; 1-D duplicate caught above
        let s = Table::from_csv("t", "x,y,f\n0,0,1\n1,0,2\n0,1,3\n", &spec(&["x", "y"])).unwrap();
        assert!(!s.is_grid());
        assert!(e("x,y,f\n0,0,1\n0,0,2\n1,1,3\n", &spec(&["x", "y"])).contains("duplicate point"));
    }

    #[test]
    fn unit_from_spec_when_header_has_none() {
        let mut s = spec(&["y"]);
        s.units = vec![("y".into(), "mm".into()), ("u".into(), "km/h".into())];
        let t = Table::from_csv("p", "y,u\n0,0\n10,36\n", &s).unwrap();
        assert!((t.eval(0, &[0.005]) - 5.0).abs() < 1e-12); // 5 mm -> 18 km/h = 5 m/s
        assert_eq!(t.inputs[0].dim, Some(Dimension::LENGTH));
    }
}
