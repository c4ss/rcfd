//! Solution-adaptive steady solves (F-102, ADR 0242): a loop of SIMPLE solves on an [`AmrMesh`], each followed by an indicator
//! of the solution, fixed-fraction marks, a refinement step and the transfer of `U` and `p` to the new mesh.
//!
//! Nothing outside this module calls it; every existing path is unchanged.
//!
//! F-343 (ADR 0247) adds opt-in criteria: target cell count ([`target_count_marks`], [`target_schedule`],
//! [`run_adaptive_simple_target`]), edge-length weighting ([`edge_length_weighted_indicator`],
//! [`velocity_edge_length_indicator`]), registers ([`Register`], [`register_marks`], [`restrict_marks`]) and the loop with any
//! marks ([`run_adaptive_simple_marked`]). Anisotropic (directional) refinement is not available: [`AmrMesh`] keeps one level per
//! leaf for every axis.

use crate::field::{ScalarField, VectorField};
use crate::geometry::FvGeometry;
use crate::gradient::least_squares;
use crate::simple::{SimpleSettings, SimpleSolver};
use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use rcfd_mesh::adapt::{cell_lengths, transfer_field, AmrMesh, Mark, RefineTransfer};
use rcfd_mesh::Mesh;
use std::sync::Arc;

/// Settings of [`run_adaptive_simple`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdaptiveSettings {
    /// Adaptation steps (the loop makes `cycles + 1` solves).
    pub cycles: usize,
    /// Fraction of the cells marked for refinement per step ([`fixed_fraction_marks`]).
    pub refine_fraction: f64,
    /// 2:1 balance of the refinement ([`AmrMesh::adapt`]).
    pub balance: bool,
    /// How refined cells take their values ([`transfer_field`]).
    pub transfer: RefineTransfer,
    /// Indicator `|grad U| h` instead of `|grad U|` ([`velocity_gradient_indicator`]).
    pub scale_by_length: bool,
    /// Iterations per solve ([`SimpleSolver::run`]).
    pub max_iter: usize,
    /// Convergence tolerance per solve ([`SimpleSolver::run`]).
    pub tol: f64,
}

/// One solve of the loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleRecord {
    /// Cells of the mesh of this solve.
    pub cells: usize,
    /// Iterations of the solve.
    pub iterations: usize,
    /// Whether the solve met its tolerance.
    pub converged: bool,
    /// Cells split by the step that made this mesh (0 for the first solve).
    pub refined: usize,
    /// Cells marked `Refine` at the finest level by that step (0 for the first solve).
    pub ignored_at_max_level: usize,
}

/// Result of [`run_adaptive_simple`].
pub struct AdaptiveRun {
    /// The final lattice; `solver`'s mesh is its [`AmrMesh::to_mesh`].
    pub amr: AmrMesh,
    /// The solver of the last solve.
    pub solver: SimpleSolver,
    /// One record per solve.
    pub cycles: Vec<CycleRecord>,
}

/// Per cell `|grad U|_F` (times the cell length of [`cell_lengths`] when `scale_by_length`), the gradient of each component
/// being the least-squares gradient with the boundary values of `U`.
pub fn velocity_gradient_indicator(solver: &SimpleSolver, axes: [bool; 3], scale_by_length: bool) -> Result<Vec<f64>> {
    let g = &solver.geom;
    let n = g.n_cells();
    if solver.u.values.len() != n {
        return Err(Error::Other(format!(
            "velocity_gradient_indicator: U has {} cells, the mesh {n}",
            solver.u.values.len()
        )));
    }
    let bu = solver.u.boundary_values(g);
    let mut sq = vec![0.0; n];
    for k in 0..3 {
        let cells: Vec<f64> = solver.u.values.iter().map(|v| v[k]).collect();
        let bv: Vec<f64> = bu.iter().map(|v| v[k]).collect();
        let gk = least_squares(g, &cells, &bv);
        for (s, v) in sq.iter_mut().zip(&gk) {
            for a in 0..3 {
                if axes[a] {
                    *s += v[a] * v[a];
                }
            }
        }
    }
    let h = if scale_by_length {
        cell_lengths(&g.mesh, axes)
    } else {
        vec![1.0; n]
    };
    let out: Vec<f64> = sq.iter().zip(&h).map(|(s, h)| s.sqrt() * h).collect();
    if let Some(c) = out.iter().position(|v| !v.is_finite()) {
        return Err(Error::Other(format!(
            "velocity_gradient_indicator: non-finite indicator in cell {c}"
        )));
    }
    Ok(out)
}

/// Fixed-fraction marking: among the cells with `level < max_level`, the `ceil(fraction * n_cells)` cells with the largest
/// indicator (every eligible cell if there are fewer) are marked `Refine`, the others `Keep`; ties go to the lower index.
pub fn fixed_fraction_marks(indicator: &[f64], levels: &[u8], max_level: u8, fraction: f64) -> Result<Vec<Mark>> {
    if !(fraction.is_finite() && (0.0..=1.0).contains(&fraction)) {
        return Err(Error::Other(format!(
            "fixed_fraction_marks: fraction must be finite and in [0, 1], got {fraction}"
        )));
    }
    if indicator.len() != levels.len() {
        return Err(Error::Other(format!(
            "fixed_fraction_marks: indicator has {} entries but levels has {}",
            indicator.len(),
            levels.len()
        )));
    }
    if let Some(i) = indicator.iter().position(|v| !v.is_finite()) {
        return Err(Error::Other(format!(
            "fixed_fraction_marks: indicator[{i}] is not finite"
        )));
    }
    let n = indicator.len();
    let k = (fraction * n as f64).ceil() as usize;
    let mut eligible: Vec<usize> = (0..n).filter(|&c| levels[c] < max_level).collect();
    eligible.sort_by(|&a, &b| indicator[b].total_cmp(&indicator[a]).then(a.cmp(&b)));
    let mut marks = vec![Mark::Keep; n];
    for &c in eligible.iter().take(k) {
        marks[c] = Mark::Refine;
    }
    Ok(marks)
}

fn in_cycle<T>(k: usize, r: Result<T>) -> Result<T> {
    r.map_err(|e| Error::Other(format!("adaptive cycle {k}: {e}")))
}

/// [`run_adaptive_simple_with`] with [`velocity_gradient_indicator`] (`settings.scale_by_length`, the lattice's refined axes).
pub fn run_adaptive_simple<F>(amr: AmrMesh, setup: F, settings: &AdaptiveSettings) -> Result<AdaptiveRun>
where
    F: FnMut(&Arc<FvGeometry>) -> Result<(VectorField, ScalarField, SimpleSettings)>,
{
    let axes = amr.refined_axes();
    let scale = settings.scale_by_length;
    run_adaptive_simple_with(amr, setup, settings, move |s: &SimpleSolver| {
        velocity_gradient_indicator(s, axes, scale)
    })
}

/// The adaptation loop. `setup` makes the fields (with their boundary conditions) and the solver settings on a mesh; cycle 0
/// solves on `amr` as given; every later cycle computes `indicator` of the last solution, marks with [`fixed_fraction_marks`],
/// adapts, calls `setup` on the new mesh, replaces the cell values of `U` and `p` by the transferred last solution and solves.
/// A solve that does not converge is recorded, not an error.
pub fn run_adaptive_simple_with<F, I>(
    amr: AmrMesh,
    setup: F,
    settings: &AdaptiveSettings,
    mut indicator: I,
) -> Result<AdaptiveRun>
where
    F: FnMut(&Arc<FvGeometry>) -> Result<(VectorField, ScalarField, SimpleSettings)>,
    I: FnMut(&SimpleSolver) -> Result<Vec<f64>>,
{
    let fraction = settings.refine_fraction;
    run_adaptive_simple_marked(
        amr,
        setup,
        settings,
        move |solver: &SimpleSolver, amr: &AmrMesh, _k: usize| {
            let ind = indicator(solver)?;
            if ind.len() != amr.n_cells() {
                return Err(Error::Other(format!(
                    "indicator has {} entries, the mesh has {} cells",
                    ind.len(),
                    amr.n_cells()
                )));
            }
            fixed_fraction_marks(&ind, &amr.levels(), amr.max_level(), fraction)
        },
    )
}

/// The adaptation loop with any marks: as [`run_adaptive_simple_with`], but every cycle `k >= 1` takes its marks from
/// `marker(last solver, current lattice, k)` (one mark per cell). `settings.refine_fraction` is not used.
pub fn run_adaptive_simple_marked<F, M>(
    amr: AmrMesh,
    mut setup: F,
    settings: &AdaptiveSettings,
    mut marker: M,
) -> Result<AdaptiveRun>
where
    F: FnMut(&Arc<FvGeometry>) -> Result<(VectorField, ScalarField, SimpleSettings)>,
    M: FnMut(&SimpleSolver, &AmrMesh, usize) -> Result<Vec<Mark>>,
{
    let axes = amr.refined_axes();
    let mut amr = amr;
    let mut records = Vec::with_capacity(settings.cycles + 1);
    let mesh = in_cycle(0, amr.to_mesh())?;
    let geom = Arc::new(in_cycle(0, FvGeometry::new(Arc::new(mesh)))?);
    let (u, p, s) = in_cycle(0, setup(&geom))?;
    let mut solver = SimpleSolver::new(geom, u, p, s);
    let sum = solver.run(settings.max_iter, settings.tol);
    records.push(CycleRecord {
        cells: solver.geom.n_cells(),
        iterations: sum.iterations,
        converged: sum.converged,
        refined: 0,
        ignored_at_max_level: 0,
    });
    for k in 1..=settings.cycles {
        let marks = in_cycle(k, marker(&solver, &amr, k))?;
        let (next, tr, rep) = in_cycle(k, amr.adapt(&marks, settings.balance))?;
        let mesh: Mesh = in_cycle(k, next.to_mesh())?;
        let old: &Mesh = &solver.geom.mesh;
        let mut moved: [Vec<f64>; 4] = [vec![], vec![], vec![], vec![]];
        for (c, m) in moved.iter_mut().enumerate().take(3) {
            let comp: Vec<f64> = solver.u.values.iter().map(|v| v[c]).collect();
            *m = in_cycle(k, transfer_field(old, &mesh, &tr, &comp, settings.transfer, axes))?;
        }
        moved[3] = in_cycle(
            k,
            transfer_field(old, &mesh, &tr, &solver.p.values, settings.transfer, axes),
        )?;
        let geom = Arc::new(in_cycle(k, FvGeometry::new(Arc::new(mesh)))?);
        let (mut u, mut p, s) = in_cycle(k, setup(&geom))?;
        if u.values.len() != geom.n_cells() || p.values.len() != geom.n_cells() {
            return Err(Error::Other(format!(
                "adaptive cycle {k}: setup returned fields of the wrong size"
            )));
        }
        for (c, v) in u.values.iter_mut().enumerate() {
            *v = [moved[0][c], moved[1][c], moved[2][c]];
        }
        p.values.clone_from(&moved[3]);
        amr = next;
        solver = SimpleSolver::new(geom, u, p, s);
        let sum = solver.run(settings.max_iter, settings.tol);
        records.push(CycleRecord {
            cells: solver.geom.n_cells(),
            iterations: sum.iterations,
            converged: sum.converged,
            refined: rep.refined,
            ignored_at_max_level: rep.ignored_at_max_level,
        });
    }
    Ok(AdaptiveRun {
        amr,
        solver,
        cycles: records,
    })
}

// -------------------------------------------------------------------------------------------------------------------
// F-343 (ADR 0247): target cell count
// -------------------------------------------------------------------------------------------------------------------

/// Cells made by one split: `2^d`, `d` the number of refined axes.
pub fn children_per_split(axes: [bool; 3]) -> usize {
    1 << axes.iter().filter(|&&a| a).count()
}

/// Marks `Refine` on the `k` largest indicators among the cells below `max_level` (ties to the lower index), `Keep` elsewhere.
fn mark_largest(indicator: &[f64], levels: &[u8], max_level: u8, k: usize) -> Vec<Mark> {
    let n = indicator.len();
    let mut eligible: Vec<usize> = (0..n).filter(|&c| levels[c] < max_level).collect();
    eligible.sort_by(|&a, &b| indicator[b].total_cmp(&indicator[a]).then(a.cmp(&b)));
    let mut marks = vec![Mark::Keep; n];
    for &c in eligible.iter().take(k) {
        marks[c] = Mark::Refine;
    }
    marks
}

/// Target cell count (ADR 0247 D2): with `n` cells and `children` cells per split, the `k = ceil((n_target - n) /
/// (children - 1))` cells with the largest indicator below `max_level` are marked `Refine` (none when `n_target <= n`; every
/// eligible cell when fewer than `k` are eligible); ties go to the lower index. Without balance the adapted mesh then has
/// `n + k (children - 1)` cells, in `[n_target, n_target + children - 2]` whenever enough cells are eligible.
pub fn target_count_marks(
    indicator: &[f64],
    levels: &[u8],
    max_level: u8,
    n_target: usize,
    children: usize,
) -> Result<Vec<Mark>> {
    if children < 2 {
        return Err(Error::Other(format!(
            "target_count_marks: children must be at least 2, got {children}"
        )));
    }
    if indicator.len() != levels.len() {
        return Err(Error::Other(format!(
            "target_count_marks: indicator has {} entries but levels has {}",
            indicator.len(),
            levels.len()
        )));
    }
    if let Some(i) = indicator.iter().position(|v| !v.is_finite()) {
        return Err(Error::Other(format!(
            "target_count_marks: indicator[{i}] is not finite"
        )));
    }
    let n = indicator.len();
    let k = if n_target > n {
        (n_target - n).div_ceil(children - 1)
    } else {
        0
    };
    Ok(mark_largest(indicator, levels, max_level, k))
}

/// Intermediate target of cycle `k` of `cycles` (ADR 0247 D2): `round(n0 (n_target / n0)^(k / cycles))` for `0 < k < cycles`,
/// `n0` for `k = 0` and `n_target` for `k >= cycles`.
pub fn target_schedule(n0: usize, n_target: usize, cycles: usize, k: usize) -> usize {
    if k == 0 {
        n0
    } else if k >= cycles || n0 == 0 {
        n_target
    } else {
        let ratio = n_target as f64 / n0 as f64;
        (n0 as f64 * ratio.powf(k as f64 / cycles as f64)).round() as usize
    }
}

/// The loop with target-count marks: cycle `k` aims at [`target_schedule`]`(N_0, n_target, settings.cycles, k)` from the
/// current count, ranking by `indicator`; `N_0` is the cell count of `amr`. `settings.refine_fraction` is not used.
pub fn run_adaptive_simple_target<F, I>(
    amr: AmrMesh,
    setup: F,
    settings: &AdaptiveSettings,
    n_target: usize,
    mut indicator: I,
) -> Result<AdaptiveRun>
where
    F: FnMut(&Arc<FvGeometry>) -> Result<(VectorField, ScalarField, SimpleSettings)>,
    I: FnMut(&SimpleSolver) -> Result<Vec<f64>>,
{
    let n0 = amr.n_cells();
    let cycles = settings.cycles;
    let children = children_per_split(amr.refined_axes());
    run_adaptive_simple_marked(
        amr,
        setup,
        settings,
        move |solver: &SimpleSolver, amr: &AmrMesh, k: usize| {
            let ind = indicator(solver)?;
            target_count_marks(
                &ind,
                &amr.levels(),
                amr.max_level(),
                target_schedule(n0, n_target, cycles, k),
                children,
            )
        },
    )
}

// -------------------------------------------------------------------------------------------------------------------
// F-343 (ADR 0247): edge-length weighting
// -------------------------------------------------------------------------------------------------------------------

/// Per cell the extent of its vertices along each axis (the edge lengths of a box cell).
pub fn cell_extents(mesh: &Mesh) -> Vec<Vec3> {
    let pts = mesh.points();
    (0..mesh.n_cells())
        .map(|c| {
            let mut lo = [f64::INFINITY; 3];
            let mut hi = [f64::NEG_INFINITY; 3];
            for &f in mesh.cell_faces(c) {
                for &p in mesh.face_points(f) {
                    for a in 0..3 {
                        lo[a] = lo[a].min(pts[p][a]);
                        hi[a] = hi[a].max(pts[p][a]);
                    }
                }
            }
            [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]]
        })
        .collect()
}

/// Edge-length-weighted indicator (ADR 0247 D3): `eta_c = (sum_a sum_k (g_k[c][a] h_{c,a})^2)^(1/2) (prod_a h_{c,a})^((r -
/// 1) / d)` over the `d` active axes, `g_k` the gradient of field component `k` and `h_{c,a}` the [`cell_extents`]. `r = 1` is
/// the variation across the cell along its edges; `r = 1 + d/2` the cell's contribution to the `L2` norm of that variation.
pub fn edge_length_weighted_indicator(mesh: &Mesh, grads: &[Vec<Vec3>], axes: [bool; 3], r: f64) -> Result<Vec<f64>> {
    let n = mesh.n_cells();
    let d = axes.iter().filter(|&&a| a).count();
    if d == 0 {
        return Err(Error::Other("edge_length_weighted_indicator: no active axis".into()));
    }
    if !(r.is_finite() && r >= 0.0) {
        return Err(Error::Other(format!(
            "edge_length_weighted_indicator: exponent r must be finite and non-negative, got {r}"
        )));
    }
    for (k, g) in grads.iter().enumerate() {
        if g.len() != n {
            return Err(Error::Other(format!(
                "edge_length_weighted_indicator: grads[{k}] has {} entries, the mesh has {n} cells",
                g.len()
            )));
        }
        if let Some(c) = g.iter().position(|v| v.iter().any(|x| !x.is_finite())) {
            return Err(Error::Other(format!(
                "edge_length_weighted_indicator: grads[{k}][{c}] is not finite"
            )));
        }
    }
    let ext = cell_extents(mesh);
    let expo = (r - 1.0) / d as f64;
    let out: Vec<f64> = (0..n)
        .map(|c| {
            let mut sq = 0.0;
            let mut meas = 1.0;
            for a in 0..3 {
                if !axes[a] {
                    continue;
                }
                let h = ext[c][a];
                meas *= h;
                for g in grads {
                    let v = g[c][a] * h;
                    sq += v * v;
                }
            }
            sq.sqrt() * meas.powf(expo)
        })
        .collect();
    if let Some(c) = out.iter().position(|v| !v.is_finite()) {
        return Err(Error::Other(format!(
            "edge_length_weighted_indicator: non-finite indicator in cell {c}"
        )));
    }
    Ok(out)
}

/// [`edge_length_weighted_indicator`] of the velocity, the gradient of each component being the least-squares gradient with the
/// boundary values of `U` (as in [`velocity_gradient_indicator`]).
pub fn velocity_edge_length_indicator(solver: &SimpleSolver, axes: [bool; 3], r: f64) -> Result<Vec<f64>> {
    let g = &solver.geom;
    let n = g.n_cells();
    if solver.u.values.len() != n {
        return Err(Error::Other(format!(
            "velocity_edge_length_indicator: U has {} cells, the mesh {n}",
            solver.u.values.len()
        )));
    }
    let bu = solver.u.boundary_values(g);
    let grads: Vec<Vec<Vec3>> = (0..3)
        .map(|k| {
            let cells: Vec<f64> = solver.u.values.iter().map(|v| v[k]).collect();
            let bv: Vec<f64> = bu.iter().map(|v| v[k]).collect();
            least_squares(g, &cells, &bv)
        })
        .collect();
    edge_length_weighted_indicator(&g.mesh, &grads, axes, r)
}

// -------------------------------------------------------------------------------------------------------------------
// F-343 (ADR 0247): registers
// -------------------------------------------------------------------------------------------------------------------

/// A set of cells (one flag per cell) that drives ([`register_marks`]) or confines ([`restrict_marks`]) the adaptation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Register {
    cells: Vec<bool>,
}

impl Register {
    /// From one flag per cell.
    pub fn from_mask(mask: Vec<bool>) -> Register {
        Register { cells: mask }
    }

    /// Cells with value `>= 0.5` (the 0/1 output of `rcfd_mesh::adapt::region_indicator`).
    pub fn from_indicator(indicator: &[f64]) -> Result<Register> {
        if let Some(i) = indicator.iter().position(|v| !v.is_finite()) {
            return Err(Error::Other(format!(
                "Register::from_indicator: indicator[{i}] is not finite"
            )));
        }
        Ok(Register {
            cells: indicator.iter().map(|&v| v >= 0.5).collect(),
        })
    }

    /// Cells with `lo <= field <= hi`.
    pub fn in_range(field: &[f64], lo: f64, hi: f64) -> Result<Register> {
        if !(lo.is_finite() && hi.is_finite() && lo <= hi) {
            return Err(Error::Other(format!(
                "Register::in_range: lo and hi must be finite with lo <= hi, got {lo} and {hi}"
            )));
        }
        if let Some(i) = field.iter().position(|v| !v.is_finite()) {
            return Err(Error::Other(format!("Register::in_range: field[{i}] is not finite")));
        }
        Ok(Register {
            cells: field.iter().map(|&v| lo <= v && v <= hi).collect(),
        })
    }

    /// Cells with `lo <= level <= hi`.
    pub fn levels_in(levels: &[u8], lo: u8, hi: u8) -> Register {
        Register {
            cells: levels.iter().map(|&l| lo <= l && l <= hi).collect(),
        }
    }

    fn check(&self, who: &str, n: usize) -> Result<()> {
        if self.cells.len() != n {
            return Err(Error::Other(format!(
                "{who}: register has {} cells, expected {n}",
                self.cells.len()
            )));
        }
        Ok(())
    }

    /// Intersection.
    pub fn and(&self, other: &Register) -> Result<Register> {
        other.check("Register::and", self.cells.len())?;
        Ok(Register {
            cells: self.cells.iter().zip(&other.cells).map(|(a, b)| *a && *b).collect(),
        })
    }

    /// Union.
    pub fn or(&self, other: &Register) -> Result<Register> {
        other.check("Register::or", self.cells.len())?;
        Ok(Register {
            cells: self.cells.iter().zip(&other.cells).map(|(a, b)| *a || *b).collect(),
        })
    }

    /// Complement.
    pub fn complement(&self) -> Register {
        Register {
            cells: self.cells.iter().map(|a| !a).collect(),
        }
    }

    /// Adds, `layers` times, every face neighbour of a member.
    pub fn grow(&self, mesh: &Mesh, layers: usize) -> Result<Register> {
        self.check("Register::grow", mesh.n_cells())?;
        let mut cur = self.cells.clone();
        for _ in 0..layers {
            let mut next = cur.clone();
            for f in 0..mesh.n_internal_faces() {
                let (o, n) = (mesh.owner()[f], mesh.neighbour()[f]);
                if cur[o] {
                    next[n] = true;
                }
                if cur[n] {
                    next[o] = true;
                }
            }
            cur = next;
        }
        Ok(Register { cells: cur })
    }

    /// Number of cells the register is defined on.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Whether the register is defined on no cell.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Number of members.
    pub fn count(&self) -> usize {
        self.cells.iter().filter(|&&a| a).count()
    }

    /// Whether cell `c` is a member.
    pub fn contains(&self, c: usize) -> bool {
        self.cells.get(c).copied().unwrap_or(false)
    }

    /// The flags.
    pub fn mask(&self) -> &[bool] {
        &self.cells
    }
}

/// `Refine` for the members below `max_level`, `Keep` elsewhere.
pub fn register_marks(register: &Register, levels: &[u8], max_level: u8) -> Result<Vec<Mark>> {
    register.check("register_marks", levels.len())?;
    Ok(register
        .cells
        .iter()
        .zip(levels)
        .map(|(&m, &l)| if m && l < max_level { Mark::Refine } else { Mark::Keep })
        .collect())
}

/// Confines `marks` to the register: members keep their mark, every other cell gets `Keep`.
pub fn restrict_marks(marks: &[Mark], register: &Register) -> Result<Vec<Mark>> {
    register.check("restrict_marks", marks.len())?;
    Ok(marks
        .iter()
        .zip(&register.cells)
        .map(|(&m, &r)| if r { m } else { Mark::Keep })
        .collect())
}
