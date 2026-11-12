//! Solution-adaptive steady solves (F-102, ADR 0242): a loop of SIMPLE solves on an [`AmrMesh`], each followed by an indicator
//! of the solution, fixed-fraction marks, a refinement step and the transfer of `U` and `p` to the new mesh.
//!
//! Nothing outside this module calls it; every existing path is unchanged.

use crate::field::{ScalarField, VectorField};
use crate::geometry::FvGeometry;
use crate::gradient::least_squares;
use crate::simple::{SimpleSettings, SimpleSolver};
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
    mut setup: F,
    settings: &AdaptiveSettings,
    mut indicator: I,
) -> Result<AdaptiveRun>
where
    F: FnMut(&Arc<FvGeometry>) -> Result<(VectorField, ScalarField, SimpleSettings)>,
    I: FnMut(&SimpleSolver) -> Result<Vec<f64>>,
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
        let ind = in_cycle(k, indicator(&solver))?;
        if ind.len() != amr.n_cells() {
            return Err(Error::Other(format!(
                "adaptive cycle {k}: indicator has {} entries, the mesh has {} cells",
                ind.len(),
                amr.n_cells()
            )));
        }
        let marks = in_cycle(
            k,
            fixed_fraction_marks(&ind, &amr.levels(), amr.max_level(), settings.refine_fraction),
        )?;
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
