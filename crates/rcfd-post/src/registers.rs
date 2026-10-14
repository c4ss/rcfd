//! Cell registers (regions) for post-processing, source restriction and adaption-style marking (F-124, ADR 0011
//! section 3).
//!
//! A [`Register`] is a boolean cell mask over a mesh. Constructors select cells by a box, by a value range of a cell
//! field (both limits inclusive), by an arbitrary function of the cell centre, by adjacency to boundary patches or by
//! a mesh cell zone; registers combine with union, intersection, difference, complement, grow (face-neighbour
//! dilation) and shrink (face-neighbour erosion; boundary faces do not erode). Statistics per register
//! ([`stats`], [`stats_component`]) give count, volume, minimum, maximum, volume-weighted mean and volume integral;
//! an empty register yields `None`, never NaN. [`Register::patch_field`] overwrites a field inside a register and
//! leaves every other cell bit identical. [`mark_by_jump`] flags cells whose largest absolute face jump of a field
//! reaches a tolerance (adaption-style marking), optionally restricted to a register.
//!
//! Errors are [`rcfd_core::Error::InvalidValue`] whose `path` names the offending argument (`box_min`, `field`,
//! `layers`, `patches`, ...) so that callers can prefix the settings path.

use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;

fn bad<T>(path: &str, reason: impl Into<String>) -> Result<T> {
    Err(Error::InvalidValue {
        path: path.into(),
        reason: reason.into(),
    })
}

/// Boolean cell mask of a mesh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Register {
    mask: Vec<bool>,
}

/// Statistics of a field over a register.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionStats {
    /// Number of cells.
    pub count: usize,
    /// Total volume of the cells.
    pub volume: f64,
    /// Smallest cell value.
    pub min: f64,
    /// Largest cell value.
    pub max: f64,
    /// Volume-weighted mean.
    pub mean: f64,
    /// Volume integral `sum v_c V_c`.
    pub integral: f64,
}

/// Result of [`mark_by_jump`].
#[derive(Debug, Clone, PartialEq)]
pub struct Marks {
    /// Marked cells.
    pub cells: Vec<bool>,
    /// Number of marked cells.
    pub count: usize,
    /// Marked volume divided by the total mesh volume.
    pub volume_fraction: f64,
}

impl Register {
    /// Register from an explicit mask.
    pub fn from_mask(mask: Vec<bool>) -> Register {
        Register { mask }
    }

    /// Register containing every cell.
    pub fn all(mesh: &Mesh) -> Register {
        Register {
            mask: vec![true; mesh.n_cells()],
        }
    }

    /// The mask (one flag per cell).
    pub fn mask(&self) -> &[bool] {
        &self.mask
    }

    /// Number of cells in the register.
    pub fn count(&self) -> usize {
        self.mask.iter().filter(|&&m| m).count()
    }

    /// True when no cell is selected.
    pub fn is_empty(&self) -> bool {
        !self.mask.iter().any(|&m| m)
    }

    /// Total volume of the register.
    pub fn volume(&self, mesh: &Mesh) -> f64 {
        mesh.cell_volumes()
            .iter()
            .zip(&self.mask)
            .filter(|(_, &m)| m)
            .map(|(v, _)| v)
            .sum()
    }

    /// Cells whose centre lies in `[lo, hi]` (inclusive) in every direction. `lo[k] >= hi[k]`, or a non-finite
    /// limit, is an error (`box_min` / `box_max`).
    pub fn in_box(mesh: &Mesh, lo: Vec3, hi: Vec3) -> Result<Register> {
        for k in 0..3 {
            if !lo[k].is_finite() {
                return bad("box_min", format!("component {k} is not finite"));
            }
            if !hi[k].is_finite() {
                return bad("box_max", format!("component {k} is not finite"));
            }
            if lo[k] >= hi[k] {
                return bad(
                    "box_max",
                    format!("box_max[{k}] = {} must exceed box_min[{k}] = {}", hi[k], lo[k]),
                );
            }
        }
        Ok(Register {
            mask: mesh
                .cell_centres()
                .iter()
                .map(|c| (0..3).all(|k| c[k] >= lo[k] && c[k] <= hi[k]))
                .collect(),
        })
    }

    /// Cells where `lo <= values[c] <= hi`. The field must have one value per cell and no NaN (the first offending
    /// cell is named); `lo > hi` is an error.
    pub fn threshold(mesh: &Mesh, values: &[f64], lo: f64, hi: f64) -> Result<Register> {
        if values.len() != mesh.n_cells() {
            return bad("field", format!("{} values for {} cells", values.len(), mesh.n_cells()));
        }
        if lo.is_nan() || hi.is_nan() || lo > hi {
            return bad("above", format!("the range [{lo}, {hi}] is empty or not a number"));
        }
        if let Some(i) = values.iter().position(|v| v.is_nan()) {
            return bad("field", format!("the field is NaN in cell {i}"));
        }
        Ok(Register {
            mask: values.iter().map(|&v| v >= lo && v <= hi).collect(),
        })
    }

    /// Cells for which `f(centre)` is true (used for expressions of the cell centre).
    pub fn from_centre_fn(mesh: &Mesh, mut f: impl FnMut(Vec3) -> bool) -> Register {
        Register {
            mask: mesh.cell_centres().iter().map(|&c| f(c)).collect(),
        }
    }

    /// Cells that own a face on one of `patches`, grown by `layers - 1` face-neighbour steps. `layers < 1`, an empty
    /// patch list or an unknown patch is an error.
    pub fn boundary_adjacent(mesh: &Mesh, patches: &[&str], layers: usize) -> Result<Register> {
        if layers < 1 {
            return bad("layers", "layers must be at least 1");
        }
        if patches.is_empty() {
            return bad("patches", "at least one patch is required");
        }
        let mut mask = vec![false; mesh.n_cells()];
        for name in patches {
            let Some(p) = mesh.patch(name) else {
                let known: Vec<&str> = mesh.patches().iter().map(|p| p.name.as_str()).collect();
                return bad(
                    "patches",
                    format!("the mesh has no patch '{name}' (patches: {})", known.join(", ")),
                );
            };
            for f in p.faces() {
                mask[mesh.owner()[f]] = true;
            }
        }
        let mut r = Register { mask };
        for _ in 1..layers {
            r = r.grow(mesh, 1);
        }
        Ok(r)
    }

    /// Cells of a mesh cell zone.
    pub fn cell_zone(mesh: &Mesh, name: &str) -> Result<Register> {
        match mesh.cell_zone(name) {
            Some(z) => {
                let mut mask = vec![false; mesh.n_cells()];
                for &i in &z.ids {
                    if i >= mask.len() {
                        return bad("cell_zone", format!("zone '{name}' lists cell {i} of {}", mask.len()));
                    }
                    mask[i] = true;
                }
                Ok(Register { mask })
            }
            None => {
                let known: Vec<&str> = mesh.cell_zones().iter().map(|z| z.name.as_str()).collect();
                bad(
                    "cell_zone",
                    format!("the mesh has no cell zone '{name}' (zones: {})", known.join(", ")),
                )
            }
        }
    }

    fn zip(&self, other: &Register, f: impl Fn(bool, bool) -> bool) -> Result<Register> {
        if self.mask.len() != other.mask.len() {
            return bad(
                "operands",
                format!(
                    "registers of {} and {} cells cannot be combined",
                    self.mask.len(),
                    other.mask.len()
                ),
            );
        }
        Ok(Register {
            mask: self.mask.iter().zip(&other.mask).map(|(&a, &b)| f(a, b)).collect(),
        })
    }

    /// Union.
    pub fn union(&self, other: &Register) -> Result<Register> {
        self.zip(other, |a, b| a || b)
    }

    /// Intersection.
    pub fn intersection(&self, other: &Register) -> Result<Register> {
        self.zip(other, |a, b| a && b)
    }

    /// Cells of `self` that are not in `other`.
    pub fn difference(&self, other: &Register) -> Result<Register> {
        self.zip(other, |a, b| a && !b)
    }

    /// Complement.
    pub fn complement(&self) -> Register {
        Register {
            mask: self.mask.iter().map(|&m| !m).collect(),
        }
    }

    /// Dilation by `steps` layers of face neighbours (internal faces).
    pub fn grow(&self, mesh: &Mesh, steps: usize) -> Register {
        let mut mask = self.mask.clone();
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        for _ in 0..steps {
            let prev = mask.clone();
            for (f, &n) in nei.iter().enumerate() {
                let o = own[f];
                if prev[o] {
                    mask[n] = true;
                }
                if prev[n] {
                    mask[o] = true;
                }
            }
        }
        Register { mask }
    }

    /// Dilation by `steps` layers of point neighbours (cells sharing at least one vertex). On a hexahedral grid this
    /// adds the full shell (a 10^3 block grows to 12^3), where the face-neighbour [`Register::grow`] adds only the
    /// six face-adjacent slabs (1000 -> 1600).
    pub fn grow_points(&self, mesh: &Mesh, steps: usize) -> Register {
        let mut mask = self.mask.clone();
        for _ in 0..steps {
            let mut pt = vec![false; mesh.n_points()];
            for (c, &m) in mask.iter().enumerate() {
                if m {
                    for &f in mesh.cell_faces(c) {
                        for &p in mesh.face_points(f) {
                            pt[p] = true;
                        }
                    }
                }
            }
            for (c, m) in mask.iter_mut().enumerate() {
                if !*m {
                    *m = mesh
                        .cell_faces(c)
                        .iter()
                        .any(|&f| mesh.face_points(f).iter().any(|&p| pt[p]));
                }
            }
        }
        Register { mask }
    }

    /// Erosion by `steps` layers of point neighbours: a cell is removed when it shares a vertex with a cell outside
    /// the register (vertices on the domain boundary do not erode).
    pub fn shrink_points(&self, mesh: &Mesh, steps: usize) -> Register {
        self.complement().grow_points(mesh, steps).complement()
    }

    /// Erosion by `steps` layers: a cell is removed when an internal face connects it to a cell outside the
    /// register. Boundary faces never erode.
    pub fn shrink(&self, mesh: &Mesh, steps: usize) -> Register {
        self.complement().grow(mesh, steps).complement()
    }

    /// Sum of the areas of the internal faces between a register cell and a non-register cell.
    pub fn interface_area(&self, mesh: &Mesh) -> f64 {
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        nei.iter()
            .enumerate()
            .filter(|&(f, &n)| self.mask[own[f]] != self.mask[n])
            .map(|(f, _)| mesh.face_area(f))
            .sum()
    }

    /// Copy of `values` with `value` inside the register and every other cell unchanged (bit identical). The field
    /// must have one value per cell.
    pub fn patch_field(&self, values: &[f64], value: f64) -> Result<Vec<f64>> {
        if values.len() != self.mask.len() {
            return bad(
                "field",
                format!("{} values for {} cells", values.len(), self.mask.len()),
            );
        }
        Ok(values
            .iter()
            .zip(&self.mask)
            .map(|(&v, &m)| if m { value } else { v })
            .collect())
    }

    /// Like [`Register::patch_field`] for a vector field.
    pub fn patch_vector_field(&self, values: &[Vec3], value: Vec3) -> Result<Vec<Vec3>> {
        if values.len() != self.mask.len() {
            return bad(
                "field",
                format!("{} values for {} cells", values.len(), self.mask.len()),
            );
        }
        Ok(values
            .iter()
            .zip(&self.mask)
            .map(|(&v, &m)| if m { value } else { v })
            .collect())
    }
}

/// Statistics of a scalar cell field over a register; `None` for an empty register or zero volume.
pub fn stats(mesh: &Mesh, reg: &Register, values: &[f64]) -> Result<Option<RegionStats>> {
    if values.len() != mesh.n_cells() || reg.mask.len() != mesh.n_cells() {
        return bad(
            "field",
            format!(
                "{} values and {} flags for {} cells",
                values.len(),
                reg.mask.len(),
                mesh.n_cells()
            ),
        );
    }
    let vol = mesh.cell_volumes();
    let (mut count, mut v, mut integral) = (0usize, 0.0, 0.0);
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for c in 0..values.len() {
        if reg.mask[c] {
            count += 1;
            v += vol[c];
            integral += vol[c] * values[c];
            lo = lo.min(values[c]);
            hi = hi.max(values[c]);
        }
    }
    if count == 0 || v <= 0.0 {
        return Ok(None);
    }
    Ok(Some(RegionStats {
        count,
        volume: v,
        min: lo,
        max: hi,
        mean: integral / v,
        integral,
    }))
}

/// Statistics of component `k` (0..3) of a vector cell field over a register.
pub fn stats_component(mesh: &Mesh, reg: &Register, values: &[Vec3], k: usize) -> Result<Option<RegionStats>> {
    if k > 2 {
        return bad("component", format!("component {k} of a 3-vector"));
    }
    let comp: Vec<f64> = values.iter().map(|v| v[k]).collect();
    stats(mesh, reg, &comp)
}

/// Statistics of the magnitude of a vector cell field over a register.
pub fn stats_magnitude(mesh: &Mesh, reg: &Register, values: &[Vec3]) -> Result<Option<RegionStats>> {
    let mag: Vec<f64> = values.iter().map(|&v| vec3::norm(v)).collect();
    stats(mesh, reg, &mag)
}

/// Largest absolute difference of `values` between each cell and its internal face neighbours (0 for isolated
/// cells).
pub fn face_jump_indicator(mesh: &Mesh, values: &[f64]) -> Result<Vec<f64>> {
    if values.len() != mesh.n_cells() {
        return bad("field", format!("{} values for {} cells", values.len(), mesh.n_cells()));
    }
    if let Some(i) = values.iter().position(|v| !v.is_finite()) {
        return bad("field", format!("the field is not finite in cell {i}"));
    }
    let mut ind = vec![0.0f64; values.len()];
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    for (f, &n) in nei.iter().enumerate() {
        let d = (values[own[f]] - values[n]).abs();
        ind[own[f]] = ind[own[f]].max(d);
        ind[n] = ind[n].max(d);
    }
    Ok(ind)
}

/// Adaption-style marking: cells whose [`face_jump_indicator`] is at least `tau` (inclusive), optionally only
/// inside `within`. `tau` must be positive and finite.
pub fn mark_by_jump(mesh: &Mesh, values: &[f64], tau: f64, within: Option<&Register>) -> Result<Marks> {
    if !(tau.is_finite() && tau > 0.0) {
        return bad(
            "tolerance",
            format!("the marking tolerance must be positive and finite, got {tau}"),
        );
    }
    if let Some(r) = within {
        if r.mask.len() != mesh.n_cells() {
            return bad(
                "register",
                format!("{} flags for {} cells", r.mask.len(), mesh.n_cells()),
            );
        }
    }
    let ind = face_jump_indicator(mesh, values)?;
    let cells: Vec<bool> = ind
        .iter()
        .enumerate()
        .map(|(c, &i)| i >= tau && within.map_or(true, |r| r.mask[c]))
        .collect();
    let vol = mesh.cell_volumes();
    let marked: f64 = cells.iter().zip(vol).filter(|(&m, _)| m).map(|(_, v)| v).sum();
    let total = mesh.total_volume();
    Ok(Marks {
        count: cells.iter().filter(|&&m| m).count(),
        volume_fraction: if total > 0.0 { marked / total } else { 0.0 },
        cells,
    })
}
