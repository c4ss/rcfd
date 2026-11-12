//! Solution-adaptive steady solves (F-102, ADR 0242): a loop of SIMPLE solves on an [`AmrMesh`], each followed by an indicator
//! of the solution, fixed-fraction marks, a refinement step and the transfer of `U` and `p` to the new mesh.
//!
//! Nothing outside this module calls it; every existing path is unchanged.
//!
//! F-343 (ADR 0247) adds opt-in criteria: target cell count ([`target_count_marks`], [`target_schedule`],
//! [`run_adaptive_simple_target`]), edge-length weighting ([`edge_length_weighted_indicator`],
//! [`velocity_edge_length_indicator`]), registers ([`Register`], [`register_marks`], [`restrict_marks`]) and the loop with any
//! marks ([`run_adaptive_simple_marked`]). [`AmrMesh`] keeps one level per leaf for every axis.
//!
//! F-348 (ADR 0267) adds directional refinement on the second lattice [`AnisoMesh`] (one level per axis), opt-in: the per-axis
//! variations ([`axis_variations`], [`velocity_axis_variations`]), the choice of the split axes ([`directional_axes`]), target
//! marks with a split size per cell ([`directional_target_marks`]) and the loop ([`run_adaptive_simple_directional`],
//! [`run_adaptive_simple_directional_target`]). The functions above are not changed by it.
//!
//! F-441 (ADR 0343) adds box coarsening inside the directional loop, opt-in: an axis whose variation is below `phi` times the
//! cell's largest is coarsened ([`directional_coarsen_axes`], [`directional_coarsen_marks`], `phi <= theta / 4` for the
//! hysteresis); each cycle coarsens with [`Coarsening::Box`] and then refines on the coarsened lattice
//! ([`directional_coarsen_refine_step`], [`merged_gradients`]); the loops are [`run_adaptive_simple_directional_coarsening`]
//! and [`run_adaptive_simple_directional_coarsen_target`]. Nothing above is changed by it.

use crate::field::{ScalarField, VectorField};
use crate::geometry::FvGeometry;
use crate::gradient::least_squares;
use crate::simple::{SimpleSettings, SimpleSolver};
use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use rcfd_mesh::adapt::aniso::{AnisoMesh, AxisMark};
use rcfd_mesh::adapt::{cell_lengths, transfer_field, AmrMesh, Mark, RefineTransfer};
use rcfd_mesh::Mesh;
use std::sync::Arc;

use rcfd_mesh::adapt::aniso::{AxisLeaf, Coarsening};
use rcfd_mesh::adapt::{AdaptReport, Source, Transfer};

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

// -------------------------------------------------------------------------------------------------------------------
// F-348 (ADR 0267): directional refinement
// -------------------------------------------------------------------------------------------------------------------

/// Result of [`run_adaptive_simple_directional`].
pub struct DirectionalRun {
    /// The final lattice; `solver`'s mesh is its [`AnisoMesh::to_mesh`].
    pub amr: AnisoMesh,
    /// The solver of the last solve.
    pub solver: SimpleSolver,
    /// One record per solve.
    pub cycles: Vec<CycleRecord>,
}

/// Per-axis variation (ADR 0267 D5): `E_{c,a} = (sum_k g_k[c][a]^2)^(1/2) h_{c,a}` on the active axes (zero elsewhere), `g_k`
/// the gradient of field component `k` and `h_{c,a}` the [`cell_extents`]: the first-order change of the field across the cell
/// along its edge of axis `a`.
pub fn axis_variations(mesh: &Mesh, grads: &[Vec<Vec3>], axes: [bool; 3]) -> Result<Vec<Vec3>> {
    let n = mesh.n_cells();
    if !axes.contains(&true) {
        return Err(Error::Other("axis_variations: no active axis".into()));
    }
    for (k, g) in grads.iter().enumerate() {
        if g.len() != n {
            return Err(Error::Other(format!(
                "axis_variations: grads[{k}] has {} entries, the mesh has {n} cells",
                g.len()
            )));
        }
        if let Some(c) = g.iter().position(|v| v.iter().any(|x| !x.is_finite())) {
            return Err(Error::Other(format!("axis_variations: grads[{k}][{c}] is not finite")));
        }
    }
    let ext = cell_extents(mesh);
    Ok((0..n)
        .map(|c| {
            let mut e = [0.0; 3];
            for a in 0..3 {
                if !axes[a] {
                    continue;
                }
                let sq: f64 = grads.iter().map(|g| g[c][a] * g[c][a]).sum();
                e[a] = sq.sqrt() * ext[c][a];
            }
            e
        })
        .collect())
}

/// [`axis_variations`] of the velocity, the gradient of each component being the least-squares gradient with the boundary
/// values of `U` (as in [`velocity_edge_length_indicator`]).
pub fn velocity_axis_variations(solver: &SimpleSolver, axes: [bool; 3]) -> Result<Vec<Vec3>> {
    let g = &solver.geom;
    let n = g.n_cells();
    if solver.u.values.len() != n {
        return Err(Error::Other(format!(
            "velocity_axis_variations: U has {} cells, the mesh {n}",
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
    axis_variations(&g.mesh, &grads, axes)
}

/// The axes along which a cell is split (ADR 0267 D5): axis `a` when it is active, below its `max_level` and its variation is
/// at least `theta` times the largest variation over the active axes. `theta = 1/2` keeps the per-axis variations within a
/// factor two of each other. With no variation at all every eligible axis is returned; if every axis that passes the test is
/// at its `max_level` none is. `theta` must be finite and in `(0, 1]`.
pub fn directional_axes(
    variation: Vec3,
    levels: [u8; 3],
    max_level: [u8; 3],
    axes: [bool; 3],
    theta: f64,
) -> Result<[bool; 3]> {
    if !(theta.is_finite() && theta > 0.0 && theta <= 1.0) {
        return Err(Error::Other(format!(
            "directional_axes: theta must be finite and in (0, 1], got {theta}"
        )));
    }
    if variation.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return Err(Error::Other(
            "directional_axes: the variations must be finite and non-negative".into(),
        ));
    }
    let top = (0..3).filter(|&a| axes[a]).map(|a| variation[a]).fold(0.0, f64::max);
    Ok([0, 1, 2].map(|a| axes[a] && levels[a] < max_level[a] && variation[a] >= theta * top))
}

/// Directional target-count marks (ADR 0267 D5): cells are taken in the order of decreasing `indicator` (ties to the lower
/// index), each split along its [`directional_axes`] (cells with no axis to split are skipped), until the planned count `n +
/// sum (2^|S| - 1)` reaches `n_target`. Without balance the adapted mesh has between `n_target` and `n_target + 2^d - 2` cells
/// whenever enough cells are eligible.
pub fn directional_target_marks(
    indicator: &[f64],
    variations: &[Vec3],
    levels: &[[u8; 3]],
    max_level: [u8; 3],
    axes: [bool; 3],
    n_target: usize,
    theta: f64,
) -> Result<Vec<AxisMark>> {
    let n = indicator.len();
    if variations.len() != n || levels.len() != n {
        return Err(Error::Other(format!(
            "directional_target_marks: indicator has {n} entries, variations {} and levels {}",
            variations.len(),
            levels.len()
        )));
    }
    if let Some(i) = indicator.iter().position(|v| !v.is_finite()) {
        return Err(Error::Other(format!(
            "directional_target_marks: indicator[{i}] is not finite"
        )));
    }
    let mut split = Vec::with_capacity(n);
    for c in 0..n {
        split.push(directional_axes(variations[c], levels[c], max_level, axes, theta)?);
    }
    let mut order: Vec<usize> = (0..n).filter(|&c| split[c].contains(&true)).collect();
    order.sort_by(|&a, &b| indicator[b].total_cmp(&indicator[a]).then(a.cmp(&b)));
    let mut marks = vec![AxisMark::KEEP; n];
    let mut planned = n;
    for c in order {
        if planned >= n_target {
            break;
        }
        marks[c] = AxisMark::refine(split[c]);
        planned += (1usize << split[c].iter().filter(|&&s| s).count()) - 1;
    }
    Ok(marks)
}

/// The adaptation loop on the directional lattice: [`run_adaptive_simple_marked`] with an [`AnisoMesh`] and [`AxisMark`]s.
/// Every cycle `k >= 1` takes its marks from `marker(last solver, current lattice, k)` (one mark per cell), adapts with
/// `settings.balance`, transfers `U` and `p` with `settings.transfer`, calls `setup` on the new mesh and solves.
/// `settings.refine_fraction` and `settings.scale_by_length` are not used.
pub fn run_adaptive_simple_directional<F, M>(
    amr: AnisoMesh,
    mut setup: F,
    settings: &AdaptiveSettings,
    mut marker: M,
) -> Result<DirectionalRun>
where
    F: FnMut(&Arc<FvGeometry>) -> Result<(VectorField, ScalarField, SimpleSettings)>,
    M: FnMut(&SimpleSolver, &AnisoMesh, usize) -> Result<Vec<AxisMark>>,
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
        let mode = settings.transfer;
        let mut moved: [Vec<f64>; 4] = [vec![], vec![], vec![], vec![]];
        for (c, m) in moved.iter_mut().enumerate().take(3) {
            let comp: Vec<f64> = solver.u.values.iter().map(|v| v[c]).collect();
            *m = in_cycle(k, transfer_field(old, &mesh, &tr, &comp, mode, axes))?;
        }
        moved[3] = in_cycle(k, transfer_field(old, &mesh, &tr, &solver.p.values, mode, axes))?;
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
    Ok(DirectionalRun {
        amr,
        solver,
        cycles: records,
    })
}

/// The directional loop with target-count marks (ADR 0267 D5): cycle `k` aims at [`target_schedule`]`(N_0, n_target,
/// settings.cycles, k)`, ranks the cells by [`velocity_edge_length_indicator`] with exponent `r` and splits each marked cell
/// along its [`directional_axes`] of [`velocity_axis_variations`] with `theta`.
pub fn run_adaptive_simple_directional_target<F>(
    amr: AnisoMesh,
    setup: F,
    settings: &AdaptiveSettings,
    n_target: usize,
    r: f64,
    theta: f64,
) -> Result<DirectionalRun>
where
    F: FnMut(&Arc<FvGeometry>) -> Result<(VectorField, ScalarField, SimpleSettings)>,
{
    let n0 = amr.n_cells();
    let cycles = settings.cycles;
    let axes = amr.refined_axes();
    run_adaptive_simple_directional(
        amr,
        setup,
        settings,
        move |solver: &SimpleSolver, amr: &AnisoMesh, k: usize| {
            let eta = velocity_edge_length_indicator(solver, axes, r)?;
            let var = velocity_axis_variations(solver, axes)?;
            directional_target_marks(
                &eta,
                &var,
                &amr.levels(),
                amr.max_levels(),
                axes,
                target_schedule(n0, n_target, cycles, k),
                theta,
            )
        },
    )
}

// -------------------------------------------------------------------------------------------------------------------
// F-441 (ADR 0343): box coarsening inside the directional loop
// -------------------------------------------------------------------------------------------------------------------

/// The axes along which a cell is coarsened (ADR 0343 D1): axis `a` when it is active, its level is above 0, the largest
/// variation `E_max` over the active axes is positive and `E_a < phi E_max`. The axis of `E_max` is never returned. With
/// `phi <= theta / 4` a just-split axis is not coarsened back and an axis merged by up to two levels is not split again by
/// [`directional_axes`] (for an unchanged gradient). `phi` must be finite and in `[0, 1/4]`; `phi = 0` coarsens nothing.
pub fn directional_coarsen_axes(variation: Vec3, levels: [u8; 3], axes: [bool; 3], phi: f64) -> Result<[bool; 3]> {
    if !(phi.is_finite() && (0.0..=0.25).contains(&phi)) {
        return Err(Error::Other(format!(
            "directional_coarsen_axes: phi must be finite and in [0, 1/4], got {phi}"
        )));
    }
    if variation.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return Err(Error::Other(
            "directional_coarsen_axes: the variations must be finite and non-negative".into(),
        ));
    }
    let top = (0..3).filter(|&a| axes[a]).map(|a| variation[a]).fold(0.0, f64::max);
    Ok([0, 1, 2].map(|a| axes[a] && levels[a] > 0 && top > 0.0 && variation[a] < phi * top))
}

/// Per cell `AxisMark::coarsen(C)` with `C` the [`directional_coarsen_axes`], or [`AxisMark::KEEP`] when `C` is empty.
pub fn directional_coarsen_marks(
    variations: &[Vec3],
    levels: &[[u8; 3]],
    axes: [bool; 3],
    phi: f64,
) -> Result<Vec<AxisMark>> {
    if variations.len() != levels.len() {
        return Err(Error::Other(format!(
            "directional_coarsen_marks: variations has {} entries but levels has {}",
            variations.len(),
            levels.len()
        )));
    }
    let mut out = Vec::with_capacity(levels.len());
    for (v, l) in variations.iter().zip(levels) {
        let c = directional_coarsen_axes(*v, *l, axes, phi)?;
        out.push(if c.contains(&true) {
            AxisMark::coarsen(c)
        } else {
            AxisMark::KEEP
        });
    }
    Ok(out)
}

/// Gradients on an adapted mesh from those of the old mesh (ADR 0343 D2): `Same(i)` and `Descendant(i)` keep cell `i`'s
/// gradient, `Merged(S)` takes the volume-weighted mean over `S` (volumes of `old`). One list per field component.
pub fn merged_gradients(old: &Mesh, tr: &Transfer, grads: &[Vec<Vec3>]) -> Result<Vec<Vec<Vec3>>> {
    let n = old.n_cells();
    let vol = old.cell_volumes();
    let outside = |i: usize| Error::Other(format!("merged_gradients: source {i} outside the old mesh"));
    let mut out = Vec::with_capacity(grads.len());
    for (k, g) in grads.iter().enumerate() {
        if g.len() != n {
            return Err(Error::Other(format!(
                "merged_gradients: grads[{k}] has {} entries, the old mesh has {n} cells",
                g.len()
            )));
        }
        let mut m = Vec::with_capacity(tr.sources.len());
        for s in &tr.sources {
            m.push(match s {
                Source::Same(i) | Source::Descendant(i) => *g.get(*i).ok_or_else(|| outside(*i))?,
                Source::Merged(kids) => {
                    let (mut sv, mut sg) = (0.0, [0.0; 3]);
                    for &i in kids {
                        let gi = g.get(i).ok_or_else(|| outside(i))?;
                        sv += vol[i];
                        for a in 0..3 {
                            sg[a] += vol[i] * gi[a];
                        }
                    }
                    [sg[0] / sv, sg[1] / sv, sg[2] / sv]
                }
            });
        }
        out.push(m);
    }
    Ok(out)
}

/// One adaptation step of the coarsening loop ([`directional_coarsen_refine_step`]).
pub struct DirectionalStep {
    /// The coarsened lattice, its mesh and the transfer to it; `None` when nothing was merged (no coarsen mark, or no merge).
    pub coarsened: Option<(AnisoMesh, Mesh, Transfer)>,
    /// Report of the coarsening call (all zero when no coarsen mark was given).
    pub coarsen_report: AdaptReport,
    /// The refined lattice.
    pub next: AnisoMesh,
    /// Transfer from the coarsened lattice (or from the given one when `coarsened` is `None`) to `next`.
    pub transfer: Transfer,
    /// Report of the refinement call.
    pub report: AdaptReport,
}

/// Coarsen, then refine on the coarsened lattice (ADR 0343 D2). `coarsen_marks` (coarsening only) are applied with
/// [`Coarsening::Box`]; the gradients `grads` of the old mesh `mesh` (one list per field component) are carried to the
/// coarsened mesh by [`merged_gradients`]; the refinement marks are [`directional_target_marks`] of the
/// [`edge_length_weighted_indicator`] (exponent `r`) and the [`axis_variations`] on the coarsened mesh, aiming at `n_target`
/// from its cell count. `balance` is used by both calls.
#[allow(clippy::too_many_arguments)]
pub fn directional_coarsen_refine_step(
    amr: &AnisoMesh,
    mesh: &Mesh,
    grads: &[Vec<Vec3>],
    coarsen_marks: &[AxisMark],
    n_target: usize,
    r: f64,
    theta: f64,
    balance: bool,
) -> Result<DirectionalStep> {
    let axes = amr.refined_axes();
    if coarsen_marks.len() != amr.n_cells() || mesh.n_cells() != amr.n_cells() {
        return Err(Error::Other(format!(
            "directional_coarsen_refine_step: coarsen_marks has {} entries and the mesh {} cells, the lattice has {}",
            coarsen_marks.len(),
            mesh.n_cells(),
            amr.n_cells()
        )));
    }
    if let Some(c) = coarsen_marks.iter().position(|m| m.refine.contains(&true)) {
        return Err(Error::Other(format!(
            "directional_coarsen_refine_step: coarsen_marks[{c}] asks for a refinement"
        )));
    }
    let asks = coarsen_marks.iter().any(|m| (0..3).any(|a| m.coarsen[a] && axes[a]));
    let mut coarsened = None;
    let mut coarsen_report = AdaptReport::default();
    if asks {
        let (mid, t1, rep1) = amr.adapt_with(coarsen_marks, balance, Coarsening::Box)?;
        coarsen_report = rep1;
        if rep1.coarsened > 0 {
            let mid_mesh = mid.to_mesh()?;
            coarsened = Some((mid, mid_mesh, t1));
        }
    }
    let (lat, lmesh, lgrads) = match &coarsened {
        Some((mid, mid_mesh, t1)) => (mid, mid_mesh, merged_gradients(mesh, t1, grads)?),
        None => (amr, mesh, grads.to_vec()),
    };
    let eta = edge_length_weighted_indicator(lmesh, &lgrads, axes, r)?;
    let var = axis_variations(lmesh, &lgrads, axes)?;
    let marks = directional_target_marks(&eta, &var, &lat.levels(), lat.max_levels(), axes, n_target, theta)?;
    let (next, transfer, report) = lat.adapt(&marks, balance)?;
    Ok(DirectionalStep {
        coarsened,
        coarsen_report,
        next,
        transfer,
        report,
    })
}

/// Result of [`run_adaptive_simple_directional_coarsening`].
pub struct DirectionalCoarseningRun {
    /// The final lattice; `solver`'s mesh is its [`AnisoMesh::to_mesh`].
    pub amr: AnisoMesh,
    /// The solver of the last solve.
    pub solver: SimpleSolver,
    /// One record per solve (`refined` and `ignored_at_max_level` of the refinement call).
    pub cycles: Vec<CycleRecord>,
    /// Per solve, the merges of the step that made its mesh (0 for the first solve).
    pub coarsened: Vec<usize>,
    /// Per solve, the leaves of its lattice.
    pub leaves: Vec<Vec<AxisLeaf>>,
}

/// The least-squares gradients of the three components of `U` with its boundary values.
fn velocity_gradients(solver: &SimpleSolver) -> Result<Vec<Vec<Vec3>>> {
    let g = &solver.geom;
    let n = g.n_cells();
    if solver.u.values.len() != n {
        return Err(Error::Other(format!(
            "velocity_gradients: U has {} cells, the mesh {n}",
            solver.u.values.len()
        )));
    }
    let bu = solver.u.boundary_values(g);
    Ok((0..3)
        .map(|k| {
            let cells: Vec<f64> = solver.u.values.iter().map(|v| v[k]).collect();
            let bv: Vec<f64> = bu.iter().map(|v| v[k]).collect();
            least_squares(g, &cells, &bv)
        })
        .collect())
}

/// The directional loop with coarsening (ADR 0343 D2): every cycle `k >= 1` takes the velocity gradients of the last
/// solution, gives their [`axis_variations`] with the current lattice to `coarsen` for the coarsening marks, makes the
/// [`directional_coarsen_refine_step`] towards [`target_schedule`]`(N_0, n_target, settings.cycles, k)` (exponent `r`,
/// `theta`, `settings.balance`), moves `U` and `p` through the coarsening transfer (merged cells: the volume-weighted mean)
/// and then the refinement transfer (`settings.transfer`), calls `setup` on the new mesh and solves.
#[allow(clippy::too_many_arguments)]
pub fn run_adaptive_simple_directional_coarsening<F, C>(
    amr: AnisoMesh,
    mut setup: F,
    settings: &AdaptiveSettings,
    n_target: usize,
    r: f64,
    theta: f64,
    mut coarsen: C,
) -> Result<DirectionalCoarseningRun>
where
    F: FnMut(&Arc<FvGeometry>) -> Result<(VectorField, ScalarField, SimpleSettings)>,
    C: FnMut(&[Vec3], &AnisoMesh, usize) -> Result<Vec<AxisMark>>,
{
    let n0 = amr.n_cells();
    let cycles = settings.cycles;
    let axes = amr.refined_axes();
    let mut amr = amr;
    let mut records = Vec::with_capacity(cycles + 1);
    let mut merges = Vec::with_capacity(cycles + 1);
    let mut history = Vec::with_capacity(cycles + 1);
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
    merges.push(0);
    history.push(amr.leaves().to_vec());
    for k in 1..=cycles {
        let grads = in_cycle(k, velocity_gradients(&solver))?;
        let var = in_cycle(k, axis_variations(&solver.geom.mesh, &grads, axes))?;
        let cmarks = in_cycle(k, coarsen(&var, &amr, k))?;
        let step = in_cycle(
            k,
            directional_coarsen_refine_step(
                &amr,
                &solver.geom.mesh,
                &grads,
                &cmarks,
                target_schedule(n0, n_target, cycles, k),
                r,
                theta,
                settings.balance,
            ),
        )?;
        let mesh: Mesh = in_cycle(k, step.next.to_mesh())?;
        let mode = settings.transfer;
        let mut fields: Vec<Vec<f64>> = (0..3).map(|c| solver.u.values.iter().map(|v| v[c]).collect()).collect();
        fields.push(solver.p.values.clone());
        let mut from: &Mesh = &solver.geom.mesh;
        if let Some((_, mid_mesh, t1)) = &step.coarsened {
            for f in fields.iter_mut() {
                *f = in_cycle(k, transfer_field(from, mid_mesh, t1, f, mode, axes))?;
            }
            from = mid_mesh;
        }
        for f in fields.iter_mut() {
            *f = in_cycle(k, transfer_field(from, &mesh, &step.transfer, f, mode, axes))?;
        }
        let geom = Arc::new(in_cycle(k, FvGeometry::new(Arc::new(mesh)))?);
        let (mut u, mut p, s) = in_cycle(k, setup(&geom))?;
        if u.values.len() != geom.n_cells() || p.values.len() != geom.n_cells() {
            return Err(Error::Other(format!(
                "adaptive cycle {k}: setup returned fields of the wrong size"
            )));
        }
        for (c, v) in u.values.iter_mut().enumerate() {
            *v = [fields[0][c], fields[1][c], fields[2][c]];
        }
        p.values.clone_from(&fields[3]);
        merges.push(step.coarsen_report.coarsened);
        amr = step.next;
        history.push(amr.leaves().to_vec());
        solver = SimpleSolver::new(geom, u, p, s);
        let sum = solver.run(settings.max_iter, settings.tol);
        records.push(CycleRecord {
            cells: solver.geom.n_cells(),
            iterations: sum.iterations,
            converged: sum.converged,
            refined: step.report.refined,
            ignored_at_max_level: step.report.ignored_at_max_level,
        });
    }
    Ok(DirectionalCoarseningRun {
        amr,
        solver,
        cycles: records,
        coarsened: merges,
        leaves: history,
    })
}

/// The coarsening loop with [`directional_coarsen_marks`] (ADR 0343): `phi` must be finite and in `[0, theta / 4]` (the
/// hysteresis of ADR 0343 D1); `phi = 0` asks for no coarsening and gives the run of
/// [`run_adaptive_simple_directional_target`] bit for bit.
#[allow(clippy::too_many_arguments)]
pub fn run_adaptive_simple_directional_coarsen_target<F>(
    amr: AnisoMesh,
    setup: F,
    settings: &AdaptiveSettings,
    n_target: usize,
    r: f64,
    theta: f64,
    phi: f64,
) -> Result<DirectionalCoarseningRun>
where
    F: FnMut(&Arc<FvGeometry>) -> Result<(VectorField, ScalarField, SimpleSettings)>,
{
    if !(theta.is_finite() && theta > 0.0 && theta <= 1.0) {
        return Err(Error::Other(format!(
            "run_adaptive_simple_directional_coarsen_target: theta must be finite and in (0, 1], got {theta}"
        )));
    }
    if !(phi.is_finite() && phi >= 0.0 && phi <= theta / 4.0) {
        return Err(Error::Other(format!(
            "run_adaptive_simple_directional_coarsen_target: phi must be finite and in [0, theta / 4] = [0, {}], got {phi}",
            theta / 4.0
        )));
    }
    let axes = amr.refined_axes();
    run_adaptive_simple_directional_coarsening(
        amr,
        setup,
        settings,
        n_target,
        r,
        theta,
        move |var: &[Vec3], amr: &AnisoMesh, _k: usize| directional_coarsen_marks(var, &amr.levels(), axes, phi),
    )
}
