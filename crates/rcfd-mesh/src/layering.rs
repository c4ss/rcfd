//! Dynamic mesh layering (F-230, ADR 0090): a stack of planar cell layers along x whose last plane (the piston) moves.
//! Only the layer next to the piston deforms; when its height falls below `a_min h0` it is merged with its neighbour, and
//! when it exceeds `a_max h0` it is split into a layer of the ideal height `h0` next to the static stack and the rest.
//! Every change comes with a cell source map, so cell totals (mass, momentum, energy, volume) are carried over exactly.
//!
//! shortcut: structured box stacks with one moving x plane only; general layering of unstructured prism layers and local
//! remeshing are not implemented (ADR 0090 D1).

use crate::gen::{box_mesh, warp, Grading};
use crate::Mesh;
use rcfd_core::{Error, Result};

/// What a layering check did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerEvent {
    /// The piston layer was merged with its neighbour.
    Removed,
    /// The piston layer was split.
    Added,
}

/// A topology change: for every cell of the new mesh, the old cells and the fraction of each old cell's total it receives.
#[derive(Debug, Clone)]
pub struct LayerChange {
    /// What happened.
    pub event: LayerEvent,
    /// `sources[new_cell] = [(old_cell, fraction)]`; the fractions of every old cell sum to one.
    pub sources: Vec<Vec<(usize, f64)>>,
}

/// Planar layer stack on `[xs[0], xs[n]] x [0, ly] x [0, lz]` with `ny x nz` cells per layer (patches of [`box_mesh`]).
#[derive(Debug, Clone, PartialEq)]
pub struct LayerStack {
    /// Plane x coordinates (increasing); the last one is the piston.
    pub xs: Vec<f64>,
    /// Ideal layer height.
    pub h0: f64,
    /// Removal threshold: merge when the piston layer is lower than `a_min h0`.
    pub a_min: f64,
    /// Addition threshold: split when the piston layer is higher than `a_max h0`.
    pub a_max: f64,
    /// Cells across the layer in y and z.
    pub cross: [usize; 2],
    /// Extent in y and z.
    pub size: [f64; 2],
}

impl LayerStack {
    /// `n` uniform layers of height `h0 = length / n`. Requires `1 + a_min <= a_max` so that one event cannot undo another.
    pub fn uniform(n: usize, length: f64, cross: [usize; 2], size: [f64; 2], a_min: f64, a_max: f64) -> Result<Self> {
        if n < 2 || length <= 0.0 || !(a_min > 0.0 && 1.0 + a_min <= a_max) {
            return Err(Error::Mesh(
                "layering: need n >= 2, length > 0, 0 < a_min, 1 + a_min <= a_max".into(),
            ));
        }
        Ok(Self {
            xs: (0..=n).map(|i| length * i as f64 / n as f64).collect(),
            h0: length / n as f64,
            a_min,
            a_max,
            cross,
            size,
        })
    }

    /// Number of layers.
    pub fn n_layers(&self) -> usize {
        self.xs.len() - 1
    }

    /// Height of the piston layer.
    pub fn piston_height(&self) -> f64 {
        let n = self.n_layers();
        self.xs[n] - self.xs[n - 1]
    }

    /// The hexahedral mesh; cell `(i, j, k)` (layer `i`) has index `i + n (j + ny k)`.
    pub fn mesh(&self) -> Result<Mesh> {
        let n = self.n_layers();
        // integer x coordinates on the generator, so that the plane index is exact
        let m = box_mesh(
            [n, self.cross[0], self.cross[1]],
            [n as f64, self.size[0], self.size[1]],
            [Grading::Uniform; 3],
        )?;
        warp(&m, |p| [self.xs[p[0].round() as usize], p[1], p[2]])
    }

    /// Move the piston plane; it must stay beyond the last static plane.
    pub fn set_piston(&mut self, x: f64) -> Result<()> {
        let n = self.n_layers();
        if x <= self.xs[n - 1] {
            return Err(Error::Mesh(format!(
                "layering: piston at {x} is not beyond the last static plane {}",
                self.xs[n - 1]
            )));
        }
        self.xs[n] = x;
        Ok(())
    }

    /// Apply the thresholds once; `None` when the piston layer is within them.
    pub fn adjust(&mut self) -> Option<LayerChange> {
        let n = self.n_layers();
        let ncol = self.cross[0] * self.cross[1];
        let h = self.piston_height();
        let (event, new_n) = if h < self.a_min * self.h0 && n > 2 {
            (LayerEvent::Removed, n - 1)
        } else if h > self.a_max * self.h0 {
            (LayerEvent::Added, n + 1)
        } else {
            return None;
        };
        let mut sources = vec![Vec::new(); new_n * ncol];
        for col in 0..ncol {
            for i in 0..new_n {
                let old = |l: usize| l + n * col;
                sources[i + new_n * col] = match event {
                    LayerEvent::Removed if i == n - 2 => vec![(old(n - 2), 1.0), (old(n - 1), 1.0)],
                    LayerEvent::Added if i == n - 1 => vec![(old(n - 1), self.h0 / h)],
                    LayerEvent::Added if i == n => vec![(old(n - 1), (h - self.h0) / h)],
                    _ => vec![(old(i), 1.0)],
                };
            }
        }
        match event {
            LayerEvent::Removed => {
                self.xs.remove(n - 1);
            }
            LayerEvent::Added => {
                let x = self.xs[n - 1] + self.h0;
                self.xs.insert(n, x);
            }
        }
        Some(LayerChange { event, sources })
    }
}

/// Carry cell densities `u` (per unit volume) through a change: new total `= sum fraction * old total`, divided by the new
/// cell volume. Cell totals are conserved to round-off.
pub fn remap<const N: usize>(change: &LayerChange, u: &[[f64; N]], old_vol: &[f64], new_vol: &[f64]) -> Vec<[f64; N]> {
    change
        .sources
        .iter()
        .zip(new_vol)
        .map(|(src, v)| {
            let mut t = [0.0; N];
            for &(o, w) in src {
                for k in 0..N {
                    t[k] += w * old_vol[o] * u[o][k];
                }
            }
            t.map(|x| x / v)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_and_add_conserve_volume_and_totals() {
        let mut s = LayerStack::uniform(5, 1.0, [2, 1], [0.1, 0.1], 0.4, 1.6).unwrap();
        let m0 = s.mesh().unwrap();
        let u0: Vec<[f64; 2]> = (0..m0.n_cells()).map(|c| [1.0 + c as f64, 0.5 * c as f64]).collect();
        let tot = |m: &Mesh, u: &[[f64; 2]]| -> [f64; 3] {
            let v = m.cell_volumes();
            [
                v.iter().sum(),
                u.iter().zip(v).map(|(a, b)| a[0] * b).sum(),
                u.iter().zip(v).map(|(a, b)| a[1] * b).sum(),
            ]
        };
        // compress: the piston layer collapses and is merged
        s.set_piston(0.85).unwrap();
        let m1 = s.mesh().unwrap();
        let ch = s.adjust().unwrap();
        assert_eq!(ch.event, LayerEvent::Removed);
        let m2 = s.mesh().unwrap();
        assert_eq!(m2.n_cells(), 4 * 2);
        let u2 = remap(&ch, &u0, m1.cell_volumes(), m2.cell_volumes());
        let (a, b) = (tot(&m1, &u0), tot(&m2, &u2));
        for k in 0..3 {
            assert!(
                (a[k] - b[k]).abs() <= 1e-14 * a[k].abs(),
                "remove {k}: {} {}",
                a[k],
                b[k]
            );
        }
        assert!(s.adjust().is_none());
        // expand: the piston layer grows and is split
        s.set_piston(1.2).unwrap();
        let m3 = s.mesh().unwrap();
        let ch = s.adjust().unwrap();
        assert_eq!(ch.event, LayerEvent::Added);
        let m4 = s.mesh().unwrap();
        let u4 = remap(&ch, &u2, m3.cell_volumes(), m4.cell_volumes());
        let (a, b) = (tot(&m3, &u2), tot(&m4, &u4));
        for k in 0..3 {
            assert!((a[k] - b[k]).abs() <= 1e-14 * a[k].abs(), "add {k}: {} {}", a[k], b[k]);
        }
        assert!((s.piston_height() - (1.2 - 0.6 - 0.2)).abs() < 1e-14);
    }
}
