//! Dynamic mesh layering (F-230, ADR 0090): a stack of planar cell layers along x whose last plane (the piston) moves.
//! Only the layer next to the piston deforms; when its height falls below `a_min h0` it is merged with its neighbour, and
//! when it exceeds `a_max h0` it is split into a layer of the ideal height `h0` next to the static stack and the rest.
//! Every change comes with a cell source map, so cell totals (mass, momentum, energy, volume) are carried over exactly.
//!
//! shortcut: one moving x plane per stack, planar layers only. [`LayerStack`] is a structured box stack (ADR 0090);
//! [`ExtrudedStack`] stacks prisms over any planar [`Section`] (for example triangles) and maps the face flux through an
//! event for the incompressible solver (F-257, ADR 0213). Local remeshing is not implemented.

use crate::gen::{axis_coords, box_mesh, warp, Grading};
use crate::{BoundarySpec, Mesh, PatchKind};
use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use std::collections::HashMap;

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
        apply_thresholds(
            &mut self.xs,
            self.h0,
            self.a_min,
            self.a_max,
            self.cross[0] * self.cross[1],
        )
    }
}

/// ADR 0090 D3 on the plane coordinates `xs` of a stack with `ncol` cells per layer (cell `i + n col`): apply the
/// thresholds once and return the event with its cell source map, or `None` when the piston layer is within them. The
/// events of [`LayerStack`] and [`ExtrudedStack`] are this function (ADR 0213 S1).
fn apply_thresholds(xs: &mut Vec<f64>, h0: f64, a_min: f64, a_max: f64, ncol: usize) -> Option<LayerChange> {
    let n = xs.len() - 1;
    let h = xs[n] - xs[n - 1];
    let (event, new_n) = if h < a_min * h0 && n > 2 {
        (LayerEvent::Removed, n - 1)
    } else if h > a_max * h0 {
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
                LayerEvent::Added if i == n - 1 => vec![(old(n - 1), h0 / h)],
                LayerEvent::Added if i == n => vec![(old(n - 1), (h - h0) / h)],
                _ => vec![(old(i), 1.0)],
            };
        }
    }
    match event {
        LayerEvent::Removed => {
            xs.remove(n - 1);
        }
        LayerEvent::Added => {
            let x = xs[n - 1] + h0;
            xs.insert(n, x);
        }
    }
    Some(LayerChange { event, sources })
}

/// A planar cross-section of an [`ExtrudedStack`] in the `(y, z)` plane (F-257, ADR 0213 M1): points, cells as point
/// loops (triangles, quadrilaterals or other convex polygons, any orientation) and the boundary edges of every named patch.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    /// Point coordinates `(y, z)`.
    pub points: Vec<[f64; 2]>,
    /// Cells as point loops.
    pub cells: Vec<Vec<usize>>,
    /// Patch name and its boundary edges (point pairs); every boundary edge must be in one patch.
    pub patches: Vec<(String, Vec<[usize; 2]>)>,
}

impl Section {
    /// `ny x nz` rectangles on `[0, ly] x [0, lz]`: point `(j, k)` is `j + (ny + 1) k`, cell `(j, k)` is `j + ny k`;
    /// patches `ymin`, `ymax`, `zmin`, `zmax`.
    pub fn rect(cross: [usize; 2], size: [f64; 2]) -> Result<Self> {
        let (points, patches) = rect_grid(cross, size)?;
        let [ny, nz] = cross;
        let pid = |j: usize, k: usize| j + (ny + 1) * k;
        let mut cells = Vec::with_capacity(ny * nz);
        for k in 0..nz {
            for j in 0..ny {
                cells.push(vec![pid(j, k), pid(j + 1, k), pid(j + 1, k + 1), pid(j, k + 1)]);
            }
        }
        Ok(Self { points, cells, patches })
    }

    /// [`rect`](Self::rect) with every rectangle `q = j + ny k` cut into the triangles `2q` and `2q + 1`: by the diagonal
    /// from `(j, k)` to `(j + 1, k + 1)` when `j + k` is even, from `(j + 1, k)` to `(j, k + 1)` otherwise.
    pub fn rect_triangles(cross: [usize; 2], size: [f64; 2]) -> Result<Self> {
        let (points, patches) = rect_grid(cross, size)?;
        let [ny, nz] = cross;
        let pid = |j: usize, k: usize| j + (ny + 1) * k;
        let mut cells = Vec::with_capacity(2 * ny * nz);
        for k in 0..nz {
            for j in 0..ny {
                let (a, b, c, d) = (pid(j, k), pid(j + 1, k), pid(j + 1, k + 1), pid(j, k + 1));
                if (j + k) % 2 == 0 {
                    cells.push(vec![a, b, c]);
                    cells.push(vec![a, c, d]);
                } else {
                    cells.push(vec![a, b, d]);
                    cells.push(vec![b, c, d]);
                }
            }
        }
        Ok(Self { points, cells, patches })
    }
}

/// Points and boundary patches of the rectangular grid of [`Section::rect`].
type Grid = (Vec<[f64; 2]>, Vec<(String, Vec<[usize; 2]>)>);

fn rect_grid(cross: [usize; 2], size: [f64; 2]) -> Result<Grid> {
    let ys = axis_coords(cross[0], size[0], Grading::Uniform)?;
    let zs = axis_coords(cross[1], size[1], Grading::Uniform)?;
    let [ny, nz] = cross;
    let pid = |j: usize, k: usize| j + (ny + 1) * k;
    let mut points = Vec::with_capacity((ny + 1) * (nz + 1));
    for z in &zs {
        for y in &ys {
            points.push([*y, *z]);
        }
    }
    let patches = vec![
        (
            "ymin".to_string(),
            (0..nz).map(|k| [pid(0, k), pid(0, k + 1)]).collect(),
        ),
        (
            "ymax".to_string(),
            (0..nz).map(|k| [pid(ny, k), pid(ny, k + 1)]).collect(),
        ),
        (
            "zmin".to_string(),
            (0..ny).map(|j| [pid(j, 0), pid(j + 1, 0)]).collect(),
        ),
        (
            "zmax".to_string(),
            (0..ny).map(|j| [pid(j, nz), pid(j + 1, nz)]).collect(),
        ),
    ];
    Ok((points, patches))
}

/// A layer stack whose cells are prisms over the cells of a [`Section`] (F-257, ADR 0213 M1): planes `x = xs[k]`, point
/// `j + np k` at `(xs[k], section point j)` (`np` section points), cell `i + n c` over section cell `c` between planes `i`
/// and `i + 1`; patches `xmin` (plane 0), `xmax` (plane `n`, the piston), then the section's patches, all walls. The
/// piston and the events are those of [`LayerStack`] (ADR 0090 D3).
#[derive(Debug, Clone, PartialEq)]
pub struct ExtrudedStack {
    /// Plane x coordinates (increasing); the last one is the piston.
    pub xs: Vec<f64>,
    /// Ideal layer height.
    pub h0: f64,
    /// Removal threshold: merge when the piston layer is lower than `a_min h0`.
    pub a_min: f64,
    /// Addition threshold: split when the piston layer is higher than `a_max h0`.
    pub a_max: f64,
    /// The cross-section.
    pub section: Section,
}

/// Where the value of a face of the mesh after a layer event comes from (ADR 0213 M3).
#[derive(Debug, Clone, PartialEq)]
pub enum FaceSource {
    /// The weighted sum of old face values `(old face, weight)`; the old faces have the orientation of the new one.
    Sum(Vec<(usize, f64)>),
    /// A face of the plane inserted by an addition: its value closes the balance of its owner cell.
    Closure,
}

/// Identity of a face of an extruded stack (ADR 0213 M2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum FaceTag {
    /// Plane index and section cell.
    Plane(usize, usize),
    /// Layer index and section edge (sorted point pair).
    Side(usize, [usize; 2]),
}

impl ExtrudedStack {
    /// `n` uniform layers of height `h0 = length / n` over `section`; the requirements of [`LayerStack::uniform`].
    pub fn uniform(section: Section, n: usize, length: f64, a_min: f64, a_max: f64) -> Result<Self> {
        if n < 2 || length <= 0.0 || !(a_min > 0.0 && 1.0 + a_min <= a_max) {
            return Err(Error::Mesh(
                "layering: need n >= 2, length > 0, 0 < a_min, 1 + a_min <= a_max".into(),
            ));
        }
        if section.cells.is_empty() || section.cells.iter().any(|c| c.len() < 3) {
            return Err(Error::Mesh(
                "layering: the section needs cells of at least 3 points".into(),
            ));
        }
        let np = section.points.len();
        let in_range = |p: &usize| *p < np;
        if !section.cells.iter().flatten().all(in_range)
            || !section
                .patches
                .iter()
                .flat_map(|(_, e)| e.iter().flatten())
                .all(in_range)
        {
            return Err(Error::Mesh("layering: a section point index is out of range".into()));
        }
        Ok(Self {
            xs: (0..=n).map(|i| length * i as f64 / n as f64).collect(),
            h0: length / n as f64,
            a_min,
            a_max,
            section,
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

    /// Apply the thresholds once (ADR 0090 D3); `None` when the piston layer is within them.
    pub fn adjust(&mut self) -> Option<LayerChange> {
        let ncol = self.section.cells.len();
        apply_thresholds(&mut self.xs, self.h0, self.a_min, self.a_max, ncol)
    }

    /// Point coordinates of the current planes (point `j + np k`).
    pub fn points(&self) -> Vec<Vec3> {
        let mut out = Vec::with_capacity(self.xs.len() * self.section.points.len());
        for x in &self.xs {
            for q in &self.section.points {
                out.push([*x, q[0], q[1]]);
            }
        }
        out
    }

    /// The mesh of the current planes.
    pub fn mesh(&self) -> Result<Mesh> {
        let n = self.n_layers();
        let np = self.section.points.len();
        let pid = |k: usize, j: usize| j + np * k;
        let plane = |k: usize, lp: &[usize]| -> Vec<usize> { lp.iter().map(|&j| pid(k, j)).collect() };
        let side = |i: usize, a: usize, b: usize| vec![pid(i, a), pid(i, b), pid(i + 1, b), pid(i + 1, a)];
        let nc = self.section.cells.len();
        let mut cells = vec![Vec::new(); n * nc];
        for (c, lp) in self.section.cells.iter().enumerate() {
            for i in 0..n {
                let mut faces = vec![plane(i, lp), plane(i + 1, lp)];
                for e in 0..lp.len() {
                    faces.push(side(i, lp[e], lp[(e + 1) % lp.len()]));
                }
                cells[i + n * c] = faces;
            }
        }
        let mut specs = vec![
            BoundarySpec {
                name: "xmin".into(),
                kind: PatchKind::Wall,
                faces: self.section.cells.iter().map(|lp| plane(0, lp)).collect(),
            },
            BoundarySpec {
                name: "xmax".into(),
                kind: PatchKind::Wall,
                faces: self.section.cells.iter().map(|lp| plane(n, lp)).collect(),
            },
        ];
        for (name, edges) in &self.section.patches {
            let mut faces = Vec::with_capacity(n * edges.len());
            for i in 0..n {
                for &[a, b] in edges {
                    faces.push(side(i, a, b));
                }
            }
            specs.push(BoundarySpec {
                name: name.clone(),
                kind: PatchKind::Wall,
                faces,
            });
        }
        let mesh = Mesh::from_cell_faces(self.points(), &cells, &specs)?;
        if mesh.patches().len() != specs.len() {
            return Err(Error::Mesh(
                "layering: a boundary edge of the section is in no patch".into(),
            ));
        }
        Ok(mesh)
    }

    /// The identity of every face of `mesh`, a mesh of this stack with `n` layers (ADR 0213 M2).
    fn tags(&self, mesh: &Mesh, n: usize) -> Result<Vec<FaceTag>> {
        let np = self.section.points.len();
        (0..mesh.n_faces())
            .map(|f| {
                let ids = mesh.face_points(f);
                let lo = ids.iter().map(|p| p / np).min().unwrap_or(0);
                let hi = ids.iter().map(|p| p / np).max().unwrap_or(0);
                if lo == hi {
                    return Ok(FaceTag::Plane(lo, mesh.owner()[f] / n));
                }
                let mut e: Vec<usize> = ids.iter().map(|p| p % np).collect();
                e.sort_unstable();
                e.dedup();
                if hi != lo + 1 || e.len() != 2 {
                    return Err(Error::Mesh(format!("layering: face {f} is not a face of the stack")));
                }
                Ok(FaceTag::Side(lo, [e[0], e[1]]))
            })
            .collect()
    }

    /// The source of every face of `new` (the mesh after the event `change`, of this stack as it is after
    /// [`adjust`](Self::adjust)) in `old` (the mesh before it): ADR 0213 M3.
    pub fn face_sources(&self, change: &LayerChange, old: &Mesh, new: &Mesh) -> Result<Vec<FaceSource>> {
        let n_new = self.n_layers();
        let n_old = match change.event {
            LayerEvent::Removed => n_new + 1,
            LayerEvent::Added => n_new - 1,
        };
        let ncol = self.section.cells.len();
        if old.n_cells() != n_old * ncol || new.n_cells() != n_new * ncol || change.sources.len() != new.n_cells() {
            return Err(Error::Mesh(
                "layering: the meshes do not belong to the stack before and after the event".into(),
            ));
        }
        let index: HashMap<FaceTag, usize> = self
            .tags(old, n_old)?
            .into_iter()
            .enumerate()
            .map(|(f, t)| (t, f))
            .collect();
        let find = |t: FaceTag| -> Result<usize> {
            index
                .get(&t)
                .copied()
                .ok_or_else(|| Error::Mesh(format!("layering: no old face {t:?}")))
        };
        let one = |t: FaceTag| -> Result<FaceSource> { Ok(FaceSource::Sum(vec![(find(t)?, 1.0)])) };
        let tags = self.tags(new, n_new)?;
        let mut out = Vec::with_capacity(tags.len());
        for (f, t) in tags.into_iter().enumerate() {
            out.push(match (change.event, t) {
                (LayerEvent::Removed, FaceTag::Plane(k, c)) => {
                    one(FaceTag::Plane(if k < n_new { k } else { n_old }, c))?
                }
                (LayerEvent::Removed, FaceTag::Side(i, e)) if i + 1 < n_new => one(FaceTag::Side(i, e))?,
                (LayerEvent::Removed, FaceTag::Side(_, e)) => FaceSource::Sum(vec![
                    (find(FaceTag::Side(n_old - 2, e))?, 1.0),
                    (find(FaceTag::Side(n_old - 1, e))?, 1.0),
                ]),
                (LayerEvent::Added, FaceTag::Plane(k, c)) if k < n_old => one(FaceTag::Plane(k, c))?,
                (LayerEvent::Added, FaceTag::Plane(k, _)) if k == n_old => FaceSource::Closure,
                (LayerEvent::Added, FaceTag::Plane(_, c)) => one(FaceTag::Plane(n_old, c))?,
                (LayerEvent::Added, FaceTag::Side(i, e)) if i + 1 < n_old => one(FaceTag::Side(i, e))?,
                (LayerEvent::Added, FaceTag::Side(_, e)) => {
                    // a child of the split layer: the cell fraction of its owner (ADR 0213 M3)
                    let w = match change.sources[new.owner()[f]].as_slice() {
                        [(_, w)] => *w,
                        _ => return Err(Error::Mesh("layering: a split cell has not one source".into())),
                    };
                    FaceSource::Sum(vec![(find(FaceTag::Side(n_old - 1, e))?, w)])
                }
            });
        }
        Ok(out)
    }
}

/// Carry a face flux (positive out of the owner) through a layer event (ADR 0213 M3): every [`FaceSource::Sum`] face
/// takes its weighted sum, then every [`FaceSource::Closure`] face closes the balance of its owner cell in `new`.
pub fn remap_face_flux(sources: &[FaceSource], phi_old: &[f64], new: &Mesh) -> Vec<f64> {
    let mut phi: Vec<f64> = sources
        .iter()
        .map(|s| match s {
            FaceSource::Sum(v) => v.iter().map(|&(g, w)| w * phi_old[g]).sum(),
            FaceSource::Closure => 0.0,
        })
        .collect();
    let own = new.owner();
    for (f, s) in sources.iter().enumerate() {
        if *s == FaceSource::Closure {
            let o = own[f];
            let mut net = 0.0;
            for &g in new.cell_faces(o) {
                if g != f {
                    net += if own[g] == o { phi[g] } else { -phi[g] };
                }
            }
            phi[f] = -net;
        }
    }
    phi
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

    // ---- F-257 (ADR 0213): checks F1, F2, F1n, K1, K2 ----------------------------------------------------------

    use rcfd_core::vec3;

    /// Net outward value of every cell.
    fn net(m: &Mesh, phi: &[f64]) -> Vec<f64> {
        let mut out = vec![0.0; m.n_cells()];
        for (f, &o) in m.owner().iter().enumerate() {
            out[o] += phi[f];
        }
        for (f, &nb) in m.neighbour().iter().enumerate() {
            out[nb] -= phi[f];
        }
        out
    }

    /// Sum of the values over every patch.
    fn patch_sums(m: &Mesh, phi: &[f64]) -> Vec<f64> {
        m.patches().iter().map(|p| p.faces().map(|f| phi[f]).sum()).collect()
    }

    /// A fixed pseudo-random sequence in `[-1, 1]` (linear congruential).
    fn lcg(n: usize, seed: u64) -> Vec<f64> {
        let mut x = seed;
        (0..n)
            .map(|_| {
                x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
            })
            .collect()
    }

    /// One event of the F set-up: the meshes before and after, the change and the face sources.
    struct Event {
        before: Mesh,
        after: Mesh,
        change: LayerChange,
        sources: Vec<FaceSource>,
        n_after: usize,
    }

    fn event(s: &mut ExtrudedStack, piston: f64, want: LayerEvent) -> Event {
        s.set_piston(piston).unwrap();
        let before = s.mesh().unwrap();
        let change = s.adjust().unwrap();
        assert_eq!(change.event, want);
        let after = s.mesh().unwrap();
        let sources = s.face_sources(&change, &before, &after).unwrap();
        Event {
            before,
            after,
            change,
            sources,
            n_after: s.n_layers(),
        }
    }

    /// F1's balance error of `phi_new` (cells and patches), relative to `scale`.
    fn balance_error(ev: &Event, phi_old: &[f64], phi_new: &[f64], scale: f64) -> f64 {
        let (d_old, d_new) = (net(&ev.before, phi_old), net(&ev.after, phi_new));
        let n_old = match ev.change.event {
            LayerEvent::Removed => ev.n_after + 1,
            LayerEvent::Added => ev.n_after - 1,
        };
        let mut err = 0.0f64;
        for (c, src) in ev.change.sources.iter().enumerate() {
            let want = match (ev.change.event, src.as_slice()) {
                (LayerEvent::Added, [_]) if c % ev.n_after == n_old - 1 => 0.0,
                (LayerEvent::Added, [(o, _)]) if c % ev.n_after == n_old => d_old[*o],
                _ => src.iter().map(|&(o, _)| d_old[o]).sum(),
            };
            err = err.max((d_new[c] - want).abs());
        }
        for (a, b) in patch_sums(&ev.before, phi_old)
            .iter()
            .zip(patch_sums(&ev.after, phi_new))
        {
            err = err.max((a - b).abs());
        }
        err / scale
    }

    fn sections() -> Vec<(&'static str, Section)> {
        vec![
            ("rectangles", Section::rect([2, 1], [0.1, 0.1]).unwrap()),
            ("triangles", Section::rect_triangles([2, 2], [0.1, 0.1]).unwrap()),
        ]
    }

    #[test]
    fn f1_f1n_f2_face_flux_through_a_removal_and_an_addition() {
        let u0 = [0.3, -0.2, 0.1];
        let (mut f1, mut f1n, mut f2) = (0.0f64, f64::INFINITY, 0.0f64);
        for (name, sec) in sections() {
            let mut s = ExtrudedStack::uniform(sec, 5, 1.0, 0.4, 1.6).unwrap();
            for (piston, want) in [(0.85, LayerEvent::Removed), (1.2, LayerEvent::Added)] {
                let ev = event(&mut s, piston, want);
                // F1: pseudo-random values
                let phi_old = lcg(ev.before.n_faces(), 17 + piston.to_bits());
                let scale = phi_old.iter().fold(0.0f64, |m, x| m.max(x.abs()));
                let phi_new = remap_face_flux(&ev.sources, &phi_old, &ev.after);
                let e1 = balance_error(&ev, &phi_old, &phi_new, scale);
                // F1n: the second old face of every merged face dropped
                if want == LayerEvent::Removed {
                    let dropped: Vec<FaceSource> = ev
                        .sources
                        .iter()
                        .map(|s| match s {
                            FaceSource::Sum(v) if v.len() == 2 => FaceSource::Sum(vec![v[0]]),
                            other => other.clone(),
                        })
                        .collect();
                    let bad = remap_face_flux(&dropped, &phi_old, &ev.after);
                    let en = balance_error(&ev, &phi_old, &bad, scale);
                    println!("F1n {name}: balance error with the second old face dropped {en:.3e} (>= 1e-2)");
                    f1n = f1n.min(en);
                }
                // F2: the flux of a uniform velocity
                let flux = |m: &Mesh| -> Vec<f64> { m.face_area_vectors().iter().map(|s| vec3::dot(u0, *s)).collect() };
                let q_new = remap_face_flux(&ev.sources, &flux(&ev.before), &ev.after);
                let s_max = ev
                    .after
                    .face_area_vectors()
                    .iter()
                    .map(|s| vec3::norm(*s))
                    .fold(0.0, f64::max);
                let e2 = q_new
                    .iter()
                    .zip(flux(&ev.after))
                    .map(|(a, b)| (a - b).abs())
                    .fold(0.0, f64::max)
                    / (vec3::norm(u0) * s_max);
                println!(
                    "F1 {name} {want:?}: balance error {e1:.3e} (<= 1e-13); F2 uniform-flow error {e2:.3e} (<= 1e-13)"
                );
                f1 = f1.max(e1);
                f2 = f2.max(e2);
            }
        }
        assert!(f1 <= 1e-13, "F1 {f1}");
        assert!(f2 <= 1e-13, "F2 {f2}");
        assert!(f1n >= 1e-2, "F1n {f1n}");
    }

    #[test]
    fn k1_rectangle_stack_matches_the_box_stack() {
        let a = ExtrudedStack::uniform(Section::rect([2, 2], [0.1, 0.1]).unwrap(), 40, 1.0, 0.4, 1.6)
            .unwrap()
            .mesh()
            .unwrap();
        let b = LayerStack::uniform(40, 1.0, [2, 2], [0.1, 0.1], 0.4, 1.6)
            .unwrap()
            .mesh()
            .unwrap();
        let pa: Vec<_> = a.patches().iter().map(|p| (&p.name, &p.kind, p.size)).collect();
        let pb: Vec<_> = b.patches().iter().map(|p| (&p.name, &p.kind, p.size)).collect();
        assert_eq!(pa, pb, "K1 patches");
        assert_eq!(a.n_cells(), b.n_cells(), "K1 cells");
        let ev = a
            .cell_volumes()
            .iter()
            .zip(b.cell_volumes())
            .map(|(x, y)| (x - y).abs() / y)
            .fold(0.0, f64::max);
        let ec = a
            .cell_centres()
            .iter()
            .zip(b.cell_centres())
            .map(|(x, y)| vec3::norm(vec3::sub(*x, *y)))
            .fold(0.0, f64::max);
        println!("K1 cell volumes {ev:.2e} (relative, <= 1e-13), centres {ec:.2e} (<= 1e-13)");
        assert!(ev <= 1e-13 && ec <= 1e-13, "K1 {ev} {ec}");
    }

    #[test]
    fn k2_prism_stack_structure() {
        let s =
            ExtrudedStack::uniform(Section::rect_triangles([2, 2], [0.1, 0.1]).unwrap(), 40, 1.0, 0.4, 1.6).unwrap();
        let m = s.mesh().unwrap();
        assert_eq!(m.n_cells(), 320, "K2 cells");
        for c in 0..m.n_cells() {
            let faces = m.cell_faces(c);
            let tri = faces.iter().filter(|&&f| m.face_points(f).len() == 3).count();
            assert!(
                faces.len() == 5 && tri == 2,
                "K2 cell {c}: {} faces, {tri} triangles",
                faces.len()
            );
        }
        let sizes: Vec<(&str, usize)> = m.patches().iter().map(|p| (p.name.as_str(), p.size)).collect();
        assert_eq!(
            sizes,
            [
                ("xmin", 8),
                ("xmax", 8),
                ("ymin", 80),
                ("ymax", 80),
                ("zmin", 80),
                ("zmax", 80)
            ],
            "K2 patches"
        );
        let n = s.n_layers();
        let mut worst = 0.0f64;
        for i in 0..n {
            let v: f64 = (0..8).map(|c| m.cell_volumes()[i + n * c]).sum();
            worst = worst.max((v / (0.01 * (s.xs[i + 1] - s.xs[i])) - 1.0).abs());
        }
        println!("K2 layer volumes against 0.01 h: {worst:.2e} (relative, <= 1e-13)");
        assert!(worst <= 1e-13, "K2 {worst}");
    }
}
