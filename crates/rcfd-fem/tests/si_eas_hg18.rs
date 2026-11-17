//! F-796 (ADR 0718): the end of the subcritical branch of rule L's 16^3 punch path at amplitude 2.2e-4. ADR 0711's
//! amplitude-controlled trace rebuilt to its last state (rung 5); there, element 3720's mode tangent (the mode block of the
//! extended element at its converged modes) and the iterates of the five failing attempts re-run (the library's record of
//! the local iteration and a test-side replica of it); the branch approached from rung 4 with smaller amplitude steps (the
//! fine trace, D4) with the lowest pairs, `dlambda/da` and the element's mode tangent at each state, and the registered
//! reading: a fold of the branch or a breakdown of the local mode problem. The fast test runs the same fine trace and reading
//! on a closed-form fold with a regular local problem (PL-F), on a toy whose local problem loses its root before the fold
//! (PL-B) and the replica on one element (E1). The helpers are copies of those of `si_eas_hg17.rs` (existing tests are not
//! edited). Every aggregated value is required finite before it is compared.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{prescribed_equations, reduce, Prescribed};
use rcfd_fem::element::ElementState;
use rcfd_fem::material::{HyperelasticEnergy, PointContext};
use rcfd_fem::nl_incompatible::{
    enhanced_force_and_tangent_with, mode_iteration_record_from, point_gradients, IncompatibleModeUpdate,
    IncompatibleStabilisation, ModeIterate,
};
use rcfd_fem::nonlinear::{
    initial_states, internal_force, solve_from, solve_increment, solve_with, tangent, Contribution, DriverExtensions,
    LoadStep, NewtonOptions, StepControl,
};
use rcfd_fem::solve::DirectFactor;
use rcfd_fem::{BlockProperty, Constraint, Material, SolidFormulation, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{
    lowest_eigenpairs, symmetric_eigen, Factorisation, GeneralizedEigen, ResidualTest, Shift, SubspaceOptions,
};
use rcfd_linalg::skyline::PivotRule;
use rcfd_linalg::sparse_chol::{SparseOptions, SparseOrdering, SupernodalLdlt};
use rcfd_linalg::CsrMatrix;
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed. A value that is
/// not finite misses.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn equals(&mut self, name: &str, value: usize, expected: usize) {
        let ok = value == expected;
        println!(
            "{name}: {value} (expected {expected}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value} != {expected}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool) {
        println!("{name}: {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(name.to_string());
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

fn norm(v: impl Iterator<Item = f64>) -> f64 {
    v.map(|x| x * x).sum::<f64>().sqrt()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn max_abs(v: impl Iterator<Item = f64>) -> f64 {
    v.fold(0.0f64, |w, x| if x.is_finite() { w.max(x.abs()) } else { f64::NAN })
}

// ============================================================================ the materials and the block (ADR 0694)

const MU: f64 = 1.0e6;
const NU: f64 = 0.49;
const INC: SolidFormulation = SolidFormulation::Incompatible;
const LIN: IncompatibleStabilisation = IncompatibleStabilisation::LinearStress;

fn kappa_of(nu: f64) -> f64 {
    2.0 * MU * (1.0 + nu) / (3.0 * (1.0 - 2.0 * nu))
}

/// The neo-Hookean material of ADR 0636 (density 1000).
fn neo_hookean() -> Material {
    Material::hyperelastic(HyperelasticEnergy::neo_hookean(MU, kappa_of(NU)).unwrap(), 1000.0).unwrap()
}

#[derive(Clone, Copy)]
struct Kind {
    name: &'static str,
    formulation: SolidFormulation,
    rule: IncompatibleStabilisation,
}

const K_L: Kind = Kind {
    name: "L",
    formulation: INC,
    rule: LIN,
};
const SUPERNODAL: DirectFactor = DirectFactor::SupernodalNestedDissection;

fn with_formulation(mut mesh: SolidMesh, f: SolidFormulation) -> SolidMesh {
    if f != SolidFormulation::Full {
        mesh.set_block_property(0, BlockProperty::solid(f)).unwrap();
    }
    mesh
}

fn model_of(mesh: SolidMesh, kind: Kind) -> SolidModel {
    let mut m = SolidModel::new(with_formulation(mesh, kind.formulation));
    m.set_incompatible_stabilisation(kind.rule);
    m.add_material("all", neo_hookean()).unwrap();
    m
}

const DELTA: f64 = 0.1;
const TOL: f64 = 1e-9;

fn nodes_where(mesh: &SolidMesh, set: &str, keep: impl Fn(&Vec3) -> bool) -> Vec<usize> {
    mesh.node_set(set)
        .unwrap()
        .iter()
        .copied()
        .filter(|&n| keep(&mesh.nodes()[n]))
        .collect()
}

/// The punch and corner sets (by coordinates).
fn add_sets(mesh: &mut SolidMesh) {
    let punch = nodes_where(mesh, "zmax", |p| p[0] <= 0.5 + TOL && p[1] <= 0.5 + TOL);
    let corner = nodes_where(mesh, "zmax", |p| (p[0] - 1.0).abs() < TOL && (p[1] - 1.0).abs() < TOL);
    mesh.add_node_set("punch", punch).unwrap();
    mesh.add_node_set("corner", corner).unwrap();
}

fn block_mesh(n: usize) -> SolidMesh {
    let mut mesh = hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap();
    add_sets(&mut mesh);
    mesh
}

/// The model of the block with the punch at `-0.1` (the die constraints of ADR 0678 reading c1).
fn block_model(mesh: &SolidMesh, kind: Kind) -> SolidModel {
    let mut m = model_of(mesh.clone(), kind);
    for (set, comp, value) in [
        ("xmin", 0, 0.0),
        ("xmax", 0, 0.0),
        ("ymin", 1, 0.0),
        ("ymax", 1, 0.0),
        ("zmin", 2, 0.0),
        ("punch", 2, -DELTA),
    ] {
        m.add_constraint(Constraint::displacement(set, comp, value)).unwrap();
    }
    m
}

fn options(steps: usize) -> NewtonOptions {
    NewtonOptions::new(steps)
        .threads(4)
        .record_active_set(true)
        .factor(SUPERNODAL)
}

/// `R = -sum_punch f_z / mu` and `w_c` of a state of the block.
fn block_measures(m: &SolidModel, u: &[f64], fint: &[f64]) -> (f64, f64) {
    let dofs = m.dof_map();
    let punch = m.mesh().node_set("punch").unwrap();
    let rz: f64 = punch.iter().map(|&n| fint[dofs.equation(n, 2).unwrap()]).sum();
    let corner = m.mesh().node_set("corner").unwrap()[0];
    (-rz / MU, u[dofs.equation(corner, 2).unwrap()])
}

/// The error text of the EC1 solve of ADR 0678 (`f747-scratch/run3-heavy.log`, line 23).
const ADR0678_TEXT: &str = "solver newton: load step 10 of 10 (load factor 9.625e-1 to 9.656250000000001e-1) after 5 \
                            halvings: singular tangent: pivot -4.651826504388123e6 at node 4768 component 0 (diagonal \
                            2.8253460477227545e6)";

// ============================================================================ the observer (ADR 0688 D1)

struct AttemptLog {
    committed: f64,
    attempted: f64,
    converged: Option<bool>,
    iterates: Vec<Vec<f64>>,
}

#[derive(Default)]
struct Log {
    attempts: Vec<AttemptLog>,
}

struct Watch<'a> {
    log: &'a RefCell<Log>,
}

impl StepControl for Watch<'_> {
    fn name(&self) -> &str {
        "watch"
    }
    fn load_factor(&mut self, committed: f64, proposed: f64, _step: usize, _halvings: usize) -> rcfd_fem::Result<f64> {
        self.log.borrow_mut().attempts.push(AttemptLog {
            committed,
            attempted: proposed,
            converged: None,
            iterates: Vec::new(),
        });
        Ok(proposed)
    }
    fn converged(&mut self, _increment: &LoadStep) {
        if let Some(a) = self.log.borrow_mut().attempts.last_mut() {
            a.converged = Some(true);
        }
    }
    fn failed(&mut self, _committed: f64, _attempted: f64) {
        if let Some(a) = self.log.borrow_mut().attempts.last_mut() {
            a.converged = Some(false);
        }
    }
}

struct Observer<'a> {
    log: &'a RefCell<Log>,
}

impl Contribution for Observer<'_> {
    fn name(&self) -> &str {
        "observer"
    }
    fn add(
        &self,
        u: &[f64],
        _load_factor: f64,
        _force: &mut [f64],
        _tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        if let Some(a) = self.log.borrow_mut().attempts.last_mut() {
            if a.converged.is_none() {
                a.iterates.push(u.to_vec());
            }
        }
        Ok(())
    }
    fn add_trial_force(&self, _u: &[f64], _load_factor: f64, _force: &mut [f64]) -> rcfd_fem::Result<()> {
        Ok(())
    }
}

/// The solve of the block with the observer: the error text (empty if it passes) and the log.
fn solve_observed(mesh: &SolidMesh, tag: &str, kind: Kind, o: &NewtonOptions) -> (String, Log) {
    let m = block_model(mesh, kind);
    let log = RefCell::new(Log::default());
    let obs = Observer { log: &log };
    let mut watch = Watch { log: &log };
    let t0 = Instant::now();
    let out = solve_with(
        &m,
        &[],
        &[],
        o,
        DriverExtensions {
            contributions: vec![&obs],
            step_control: Some(&mut watch),
        },
    );
    let secs = t0.elapsed().as_secs_f64();
    let text = match out {
        Ok(_) => {
            println!("{tag} observed ({}): passes, {secs:.1} s", kind.name);
            String::new()
        }
        Err(e) => {
            println!("{tag} observed ({}): solve failed: {e}, {secs:.1} s", kind.name);
            e.to_string()
        }
    };
    let log = log.into_inner();
    let att: Vec<String> = log
        .attempts
        .iter()
        .map(|a| {
            format!(
                "{:.6}->{:.6}{}",
                a.committed,
                a.attempted,
                if a.converged == Some(true) { "" } else { " x" }
            )
        })
        .collect();
    println!("{tag} observed attempts: [{}]", att.join(", "));
    (text, log)
}

// ============================================================================ the tangent, its inertia and its lowest pairs

/// SY: the symmetric part `(K + K^T) / 2` (ADR 0694 `symmetrise`).
fn symmetrise(a: &CsrMatrix) -> CsrMatrix {
    let mut t = Vec::with_capacity(2 * a.val.len());
    for r in 0..a.n {
        for p in a.row_ptr[r]..a.row_ptr[r + 1] {
            let c = a.col[p];
            t.push((r, c, 0.5 * a.val[p]));
            t.push((c, r, 0.5 * a.val[p]));
        }
    }
    CsrMatrix::from_triplets(a.n, t)
}

/// The supernodal `L D L^T` with the `NonZero` rule (ADR 0694 `inertia`).
fn factor_nonzero(a: &CsrMatrix) -> Option<SupernodalLdlt> {
    SupernodalLdlt::factor_with(
        a,
        &SparseOptions {
            ordering: SparseOrdering::NestedDissection,
            pivots: PivotRule::NonZero,
            symmetry_tolerance: 1e-12,
        },
    )
    .ok()
}

fn inertia(a: &CsrMatrix) -> Option<usize> {
    factor_nonzero(a).map(|f| f.inertia().0)
}

fn identity(n: usize) -> CsrMatrix {
    CsrMatrix::from_triplets(n, (0..n).map(|i| (i, i, 1.0)).collect())
}

fn points_of(m: &SolidModel, free: &[usize]) -> Vec<[f64; 3]> {
    let dofs = m.dof_map();
    free.iter()
        .map(|&e| {
            let (n, _) = dofs.node_component(e).unwrap();
            let p = m.mesh().nodes()[n];
            [p[0], p[1], p[2]]
        })
        .collect()
}

/// The four lowest pairs of `(K, M)` (`Shift::Auto`, the supernodal factor, tolerance `1e-8`, 500 iterations: ADR 0694).
fn lowest(k: &CsrMatrix, mm: &CsrMatrix, points: &[[f64; 3]], test: ResidualTest) -> Option<GeneralizedEigen> {
    let opts = SubspaceOptions {
        shift: Shift::Auto,
        tolerance: 1e-8,
        residual_test: test,
        max_iterations: 500,
        factorisation: Factorisation::Supernodal,
        points: Some(points.to_vec()),
        ..SubspaceOptions::default()
    };
    let t0 = Instant::now();
    match lowest_eigenpairs(k, mm, 4, &opts) {
        Ok(e) => {
            println!(
                "lowest pairs: values {:?}, residuals {:?}, iterations {}, shift {:e}, {:.1} s",
                e.values,
                e.residuals,
                e.iterations,
                e.shift,
                t0.elapsed().as_secs_f64()
            );
            Some(e)
        }
        Err(err) => {
            println!("lowest pairs failed: {err:?}");
            None
        }
    }
}

fn combine(k: &CsrMatrix, m: &CsrMatrix, s: f64) -> CsrMatrix {
    let mut t = Vec::with_capacity(k.val.len() + m.val.len());
    for r in 0..k.n {
        for p in k.row_ptr[r]..k.row_ptr[r + 1] {
            t.push((r, k.col[p], k.val[p]));
        }
    }
    for r in 0..m.n {
        for p in m.row_ptr[r]..m.row_ptr[r + 1] {
            t.push((r, m.col[p], s * m.val[p]));
        }
    }
    CsrMatrix::from_triplets(k.n, t)
}

/// The four lowest pairs of `(K, M)` whatever the inertia (ADR 0694 `lowest_any`, the shift `1.05 s_hi`).
fn lowest_any(k: &CsrMatrix, mm: &CsrMatrix, points: &[[f64; 3]], test: ResidualTest) -> Option<GeneralizedEigen> {
    let nu = inertia(k);
    if nu == Some(0) {
        return lowest(k, mm, points, test);
    }
    let rho = k.diag().iter().map(|v| v.abs()).sum::<f64>() / mm.diag().iter().sum::<f64>();
    let zero_at = |s: f64| inertia(&combine(k, mm, s)) == Some(0);
    let mut found = None;
    for j in 0..=60 {
        let s = 1e-6 * rho * 2f64.powi(j);
        if zero_at(s) {
            let lo = if j == 0 { 0.0 } else { 1e-6 * rho * 2f64.powi(j - 1) };
            found = Some((lo, s));
            break;
        }
    }
    let Some((mut lo, mut hi)) = found else {
        println!("lowest_any: no shift with zero inertia");
        return None;
    };
    for _ in 0..10 {
        let mid = 0.5 * (lo + hi);
        if zero_at(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let opts = SubspaceOptions {
        shift: Shift::Value(1.05 * hi),
        tolerance: 1e-8,
        residual_test: test,
        max_iterations: 500,
        factorisation: Factorisation::Supernodal,
        points: Some(points.to_vec()),
        ..SubspaceOptions::default()
    };
    let t0 = Instant::now();
    match lowest_eigenpairs(k, mm, 4, &opts) {
        Ok(e) => {
            println!(
                "lowest pairs (nu {nu:?}, bracket ({lo:e}, {hi:e}]): values {:?}, residuals {:?}, iterations {}, shift {:e}, \
                 {:.1} s",
                e.values,
                e.residuals,
                e.iterations,
                e.shift,
                t0.elapsed().as_secs_f64()
            );
            Some(e)
        }
        Err(err) => {
            println!("lowest pairs (nu {nu:?}) failed: {err:?}");
            None
        }
    }
}

// ============================================================================ D1, D2: the case, the bracket, the readings

/// A converged state of a path at the load parameter `p`.
#[derive(Clone)]
struct St {
    p: f64,
    u: Vec<f64>,
    states: Vec<ElementState>,
}

/// The symmetrised reduced tangent `K`, the load vector `f = -dr_F/dp` and the free equations at a state.
#[derive(Clone)]
struct Sys {
    k: CsrMatrix,
    f: Vec<f64>,
    free: Vec<usize>,
}

/// The lowest pair of `(K, I)`: the value, the vector and the residual.
struct Pair {
    mu: f64,
    phi: Vec<f64>,
    residual: f64,
}

trait Case {
    /// The solve from a converged state to the load `to` (`to > from.p`).
    fn solve(&self, from: &St, to: f64) -> Result<St, String>;
    fn system(&self, s: &St) -> Sys;
    fn lowest_pair(&self, s: &Sys, nu: usize) -> Option<Pair>;
}

/// The inertia and the path tangent `v_t = K^{-1} f` at a state (one factor).
#[derive(Clone)]
struct Info {
    nu: Option<usize>,
    vt: Option<Vec<f64>>,
    sys: Sys,
}

fn analyse(case: &dyn Case, s: &St) -> Info {
    let sys = case.system(s);
    let (nu, vt) = match factor_nonzero(&sys.k) {
        Some(fac) => (Some(fac.inertia().0), Some(fac.solve(&sys.f))),
        None => (None, None),
    };
    Info { nu, vt, sys }
}

// ============================================================================ the finite-element case

/// The FEM case: `solve_increment` with the indefinite tangent from the converged state; the prescribed list `pres` (the
/// model's own constraints and these, scaled by the load factor).
struct FemCase<'a> {
    m: &'a SolidModel,
    pres: Vec<Prescribed>,
    opts: NewtonOptions,
    /// The residual measure of the lowest pairs (ADR 0697 amendment 1: `Backward` in the amended tests).
    test: ResidualTest,
}

impl Case for FemCase<'_> {
    fn solve(&self, from: &St, to: f64) -> Result<St, String> {
        let zero = vec![0.0; self.m.dof_map().n_equations()];
        solve_increment(
            self.m,
            &self.pres,
            &zero,
            &from.u,
            &from.states,
            from.p,
            to,
            &self.opts,
            &[],
        )
        .map(|eq| St {
            p: to,
            u: eq.u,
            states: eq.states,
        })
        .map_err(|e| e.to_string())
    }
    fn system(&self, s: &St) -> Sys {
        let kt = tangent(self.m, &s.u, &s.states, Some(4)).unwrap();
        let fixed = prescribed_equations(self.m, &self.pres).unwrap();
        let sys = reduce(&kt, &vec![0.0; kt.n], &fixed).unwrap();
        let free = sys.free().to_vec();
        Sys {
            k: symmetrise(&sys.matrix),
            f: sys.rhs,
            free,
        }
    }
    fn lowest_pair(&self, s: &Sys, nu: usize) -> Option<Pair> {
        let points = points_of(self.m, &s.free);
        let id = identity(s.free.len());
        let e = if nu == 0 {
            lowest(&s.k, &id, &points, self.test)
        } else {
            lowest_any(&s.k, &id, &points, self.test)
        }?;
        Some(Pair {
            mu: e.values[0],
            phi: e.vectors[0].clone(),
            residual: e.residuals.iter().fold(0.0f64, |w, &r| w.max(r)),
        })
    }
}

// ============================================================================ the reflection of the block

/// The reflection `P: (x, y, z) -> (y, x, z)` of the block's equations: `(P x)_{n, c} = x_{P(n), s(c)}`.
fn reflection(m: &SolidModel, n: usize) -> Vec<usize> {
    let dofs = m.dof_map();
    let key = |p: &Vec3| -> (i64, i64, i64) {
        let s = n as f64;
        (
            (p[0] * s).round() as i64,
            (p[1] * s).round() as i64,
            (p[2] * s).round() as i64,
        )
    };
    let index: HashMap<(i64, i64, i64), usize> =
        m.mesh().nodes().iter().enumerate().map(|(i, p)| (key(p), i)).collect();
    let mut map = vec![usize::MAX; dofs.n_equations()];
    for (i, p) in m.mesh().nodes().iter().enumerate() {
        let (a, b, c) = key(p);
        let j = index[&(b, a, c)];
        for comp in 0..3 {
            let sc = [1, 0, 2][comp];
            map[dofs.equation(i, comp).unwrap()] = dofs.equation(j, sc).unwrap();
        }
    }
    map
}

/// The failing attempt of the observed `16^3` solve: its index in the log.
fn failing_attempt(log: &Log) -> Option<usize> {
    log.attempts
        .iter()
        .rposition(|a| a.committed == 0.9625 && a.converged == Some(false))
}

// ============================================================================ D6: the branch case on the block

/// The block model without its punch constraint (the punch is given as a prescribed list in the fixed-load solves, D6).
fn block_model_free_punch(mesh: &SolidMesh, kind: Kind) -> SolidModel {
    let mut m = model_of(mesh.clone(), kind);
    for (set, comp) in [("xmin", 0), ("xmax", 0), ("ymin", 1), ("ymax", 1), ("zmin", 2)] {
        m.add_constraint(Constraint::displacement(set, comp, 0.0)).unwrap();
    }
    m
}

/// A path with a fixed-load Newton under an external force on the free equations and a reflection.
trait Branch: Case {
    /// Newton at the fixed load `p` from `start` (its prescribed values replaced by those at `p`) with the external force
    /// `eta g` on the free equations (`g` indexed by the free equations).
    fn fixed(&self, start: &St, p: f64, eta: f64, g: &[f64]) -> Result<St, String>;
    /// The reflection of a full vector.
    fn mirror(&self, u: &[f64]) -> Vec<f64>;
    /// The free equations.
    fn free(&self) -> Vec<usize>;
}

/// D6: the branch case of the block: `solve_increment` and the tangent on the block model, `solve_from` on the model
/// without the punch constraint with the punch prescribed at `p v`.
struct FemBranch<'a> {
    base: FemCase<'a>,
    m2: &'a SolidModel,
    fixed_eqs: Vec<(usize, f64)>,
    punch: Vec<(usize, usize, f64)>,
    refl: Vec<usize>,
    free: Vec<usize>,
}

impl<'a> FemBranch<'a> {
    fn new(m: &'a SolidModel, m2: &'a SolidModel, n: usize) -> FemBranch<'a> {
        let fixed_eqs = prescribed_equations(m, &[]).unwrap();
        let values: HashMap<usize, f64> = fixed_eqs.iter().copied().collect();
        let dofs = m.dof_map();
        let punch = m
            .mesh()
            .node_set("punch")
            .unwrap()
            .iter()
            .map(|&nd| {
                let eq = dofs.equation(nd, 2).unwrap();
                (nd, 2, values[&eq])
            })
            .collect();
        let mut is_fixed = vec![false; dofs.n_equations()];
        for &(eq, _) in &fixed_eqs {
            is_fixed[eq] = true;
        }
        let free = (0..dofs.n_equations()).filter(|&q| !is_fixed[q]).collect();
        FemBranch {
            base: FemCase {
                m,
                pres: Vec::new(),
                opts: options(10).indefinite_tangent(true),
                test: ResidualTest::Backward,
            },
            m2,
            fixed_eqs,
            punch,
            refl: reflection(m, n),
            free,
        }
    }
}

impl Case for FemBranch<'_> {
    fn solve(&self, from: &St, to: f64) -> Result<St, String> {
        self.base.solve(from, to)
    }
    fn system(&self, s: &St) -> Sys {
        self.base.system(s)
    }
    fn lowest_pair(&self, s: &Sys, nu: usize) -> Option<Pair> {
        self.base.lowest_pair(s, nu)
    }
}

impl Branch for FemBranch<'_> {
    fn fixed(&self, start: &St, p: f64, eta: f64, g: &[f64]) -> Result<St, String> {
        let mut u0 = start.u.clone();
        for &(eq, v) in &self.fixed_eqs {
            u0[eq] = p * v;
        }
        let pres: Vec<Prescribed> = self
            .punch
            .iter()
            .map(|&(nd, c, v)| Prescribed::new(nd, c, p * v))
            .collect();
        let mut load = vec![0.0; u0.len()];
        for (k, &q) in self.free.iter().enumerate() {
            load[q] = eta * g[k];
        }
        solve_from(self.m2, &pres, &load, &u0, &start.states, &self.base.opts, &[])
            .map(|eq| St {
                p,
                u: eq.u,
                states: eq.states,
            })
            .map_err(|e| e.to_string())
    }
    fn mirror(&self, u: &[f64]) -> Vec<f64> {
        (0..u.len()).map(|q| u[self.refl[q]]).collect()
    }
    fn free(&self) -> Vec<usize> {
        self.free.clone()
    }
}

// ============================================================================ D5: the closed-form cases

const PT_C: f64 = 1.0;
const TOY_N: usize = 8;

fn k_i(i: usize) -> f64 {
    1.0 + i as f64
}

fn b_i(i: usize) -> f64 {
    0.1 * i as f64
}

/// Gaussian elimination with partial pivoting of a dense row-major `n x n` system.
fn dense_solve(mut a: Vec<f64>, mut b: Vec<f64>, n: usize) -> Option<Vec<f64>> {
    for c in 0..n {
        let piv = (c..n).max_by(|&x, &y| a[x * n + c].abs().total_cmp(&a[y * n + c].abs()))?;
        if a[piv * n + c] == 0.0 {
            return None;
        }
        if piv != c {
            for k in 0..n {
                a.swap(piv * n + k, c * n + k);
            }
            b.swap(piv, c);
        }
        for r in c + 1..n {
            let f = a[r * n + c] / a[c * n + c];
            for k in c..n {
                a[r * n + k] -= f * a[c * n + k];
            }
            b[r] -= f * b[c];
        }
    }
    let mut x = vec![0.0; n];
    for r in (0..n).rev() {
        let s: f64 = (r + 1..n).map(|k| a[r * n + k] * x[k]).sum();
        x[r] = (b[r] - s) / a[r * n + r];
    }
    Some(x)
}

fn dense_lowest(k: &CsrMatrix) -> Option<Pair> {
    let n = k.n;
    let mut dense = vec![0.0; n * n];
    for r in 0..n {
        for q in k.row_ptr[r]..k.row_ptr[r + 1] {
            dense[n * r + k.col[q]] = k.val[q];
        }
    }
    let e = symmetric_eigen(&dense, n).ok()?;
    Some(Pair {
        mu: e.values[0],
        phi: e.vectors[0].clone(),
        residual: 0.0,
    })
}

fn csr_of_dense(a: &[f64], n: usize) -> CsrMatrix {
    let mut t = Vec::new();
    for r in 0..n {
        for c in 0..n {
            if a[r * n + c] != 0.0 {
                t.push((r, c, a[r * n + c]));
            }
        }
    }
    CsrMatrix::from_triplets(n, t)
}

// ============================================================================ D2: the mode and the forced response

fn amp(phi: &[f64], free: &[usize], u: &[f64]) -> f64 {
    phi.iter().zip(free).map(|(p, &q)| p * u[q]).sum()
}

/// The lowest pair at a state with its vector of unit 2-norm, the sign fixed by its largest component (positive).
fn mode_at(case: &dyn Case, tag: &str, s: &St) -> (Info, Option<Pair>) {
    let info = analyse(case, s);
    let pair = info.nu.and_then(|nu| case.lowest_pair(&info.sys, nu)).map(|mut p| {
        let n = norm(p.phi.iter().copied());
        let big = p.phi.iter().fold(0.0f64, |w, &x| if x.abs() > w.abs() { x } else { w });
        let s = if big < 0.0 { -1.0 / n } else { 1.0 / n };
        p.phi.iter_mut().for_each(|x| *x *= s);
        p
    });
    println!(
        "{tag} mode at {:.12}: nu {:?}, mu_1 {:?}, residual {:?}",
        s.p,
        info.nu,
        pair.as_ref().map(|p| p.mu),
        pair.as_ref().map(|p| p.residual)
    );
    (info, pair)
}

// ============================================================================ D3: the bordered Newton

/// The branch case with the residual, its convergence measure and the load setting of the bordered Newton (ADR 0711 D3).
trait Bordered: Branch {
    /// The free residual at a state (indexed by the free equations) and its convergence measure.
    fn residual(&self, s: &St) -> Result<(Vec<f64>, f64), String>;
    /// The tolerance of the measure and the energy tolerance (`None`: no energy test).
    fn tolerances(&self) -> (f64, Option<f64>);
    /// The state `s` at the load `p` (the prescribed values `p v`).
    fn at_load(&self, s: &St, p: f64) -> St;
}

impl Bordered for FemBranch<'_> {
    fn residual(&self, s: &St) -> Result<(Vec<f64>, f64), String> {
        let fint = internal_force(self.base.m, &s.u, &s.states).map_err(|e| e.to_string())?;
        let r: Vec<f64> = self.free.iter().map(|&q| fint[q]).collect();
        let reference = norm(fint.iter().copied());
        let rn = norm(r.iter().copied());
        Ok((r, if reference > 0.0 { rn / reference } else { rn }))
    }
    fn tolerances(&self) -> (f64, Option<f64>) {
        (1e-10, Some(1e-10))
    }
    fn at_load(&self, s: &St, p: f64) -> St {
        let mut u = s.u.clone();
        for &(eq, v) in &self.fixed_eqs {
            u[eq] = p * v;
        }
        St {
            p,
            u,
            states: s.states.clone(),
        }
    }
}

/// The linear constraint `g . u_F + h lambda = c` of the bordered Newton.
struct Control {
    g: Vec<f64>,
    h: f64,
    c: f64,
}

/// D3: the bordered Newton from `start` under the constraint `ctl`: `K y_1 = -R`, `K y_2 = f`, `dl = (c - g . u_F - h lambda -
/// g . y_1) / (g . y_2 + h)`, `du = y_1 + dl y_2`; the converged state and the number of iterations.
fn bordered<B: Bordered>(case: &B, start: &St, ctl: &Control) -> Result<(St, usize), String> {
    let free = case.free();
    let (tol, etol) = case.tolerances();
    let mut s = start.clone();
    let (mut e_first, mut e_last) = (0.0f64, 0.0f64);
    for k in 0..=20 {
        let (r, m) = case.residual(&s)?;
        if !m.is_finite() {
            return Err(format!("non-finite residual at iteration {k}"));
        }
        let gu: f64 = ctl.g.iter().zip(&free).map(|(g, &q)| g * s.u[q]).sum();
        let cres = gu + ctl.h * s.p - ctl.c;
        let c_ok = cres.abs() <= 1e-9 * ctl.c.abs().max(1e-300);
        let e_ok = etol.map_or(true, |t| e_last <= t * e_first);
        if k >= 1 && m <= tol && e_ok && c_ok {
            return Ok((s, k));
        }
        if k == 20 {
            break;
        }
        let sys = case.system(&s);
        let fac = factor_nonzero(&sys.k).ok_or_else(|| format!("the factor refused at iteration {k}"))?;
        let mr: Vec<f64> = r.iter().map(|x| -x).collect();
        let y1 = fac.solve(&mr);
        let y2 = fac.solve(&sys.f);
        let den = dot(&ctl.g, &y2) + ctl.h;
        let dl = (-cres - dot(&ctl.g, &y1)) / den;
        if !dl.is_finite() {
            return Err(format!("non-finite load correction at iteration {k}"));
        }
        let du: Vec<f64> = y1.iter().zip(&y2).map(|(a, b)| a + dl * b).collect();
        let e = du
            .iter()
            .zip(sys.f.iter().zip(&r))
            .map(|(d, (f, ri))| d * (dl * f - ri))
            .sum::<f64>()
            .abs();
        if k == 0 {
            e_first = e;
        }
        e_last = e;
        for (&q, d) in free.iter().zip(&du) {
            s.u[q] += d;
        }
        s = case.at_load(&s, s.p + dl);
    }
    Err("not converged in 20 iterations".to_string())
}

// ============================================================================ D3: the amplitude-controlled trace

/// An accepted state of the D3 trace.
#[derive(Clone)]
struct APoint {
    s: St,
    info: Info,
    a: f64,
    /// `dlambda / da = 1 / (phi . v_t)`.
    sigma: f64,
    /// The rung (0: the start; `None`: a halved state).
    rung: Option<usize>,
}

struct Fold {
    /// The index of the second state of the bracket.
    k: usize,
    lo: f64,
    hi: f64,
    a_est: f64,
    l_est: f64,
}

struct ATrace {
    points: Vec<APoint>,
    /// The stop: "fold + 2", "rungs", "range", "lost".
    stop: String,
    reading: String,
    fold: Option<Fold>,
}

const AMP_HALVINGS: usize = 4;
const AMP_CAP: usize = 40;

fn apoint<B: Bordered>(case: &B, s: St, info: Info, phi: &[f64], rung: Option<usize>) -> APoint {
    let free = case.free();
    let a = amp(phi, &free, &s.u);
    let sigma = info.vt.as_ref().map(|v| 1.0 / dot(phi, v)).unwrap_or(f64::NAN);
    APoint {
        s,
        info,
        a,
        sigma,
        rung,
    }
}

/// D3: the fold between two consecutive states.
fn fold_between(p0: &APoint, p1: &APoint) -> Fold {
    let (a0, l0, s0) = (p0.a, p0.s.p, p0.sigma);
    let (a1, l1, s1) = (p1.a, p1.s.p, p1.sigma);
    let ax = (l1 - l0 + s0 * a0 - s1 * a1) / (s0 - s1);
    let lo = l0 + s0 * (ax - a0);
    let a_est = a0 - s0 * (a1 - a0) / (s1 - s0);
    let l_est = l0 + s0 * (a_est - a0) / 2.0;
    Fold {
        k: 0,
        lo,
        hi: l0.min(l1),
        a_est,
        l_est,
    }
}

/// D3: the trace of the branch from `from` under amplitude control over the rungs `a_b 2^(k/2)`, `k = 1..=rungs`, the load
/// kept in `range`.
fn amp_trace<B: Bordered>(
    case: &B,
    tag: &str,
    from: (St, Info),
    phi: &[f64],
    rungs: usize,
    range: (f64, f64),
) -> ATrace {
    let free = case.free();
    let first = apoint(case, from.0, from.1, phi, Some(0));
    let a_b = first.a;
    println!(
        "{tag} amplitude trace: start {:.12}, a {a_b:+.9e}, nu {:?}, dlambda/da {:+.6e}",
        first.s.p, first.info.nu, first.sigma
    );
    let mut out = ATrace {
        points: vec![first],
        stop: "rungs".to_string(),
        reading: String::new(),
        fold: None,
    };
    let mut solves = 0;
    let mut after = 0;
    let mut undecided = false;
    'rungs: for k in 1..=rungs {
        let target = a_b * 2f64.powf(k as f64 / 2.0);
        let mut tgt = target;
        let mut halvings = 0;
        loop {
            let cur = out.points.last().unwrap().clone();
            if solves >= AMP_CAP {
                println!(
                    "{tag} amplitude trace: the cap of {AMP_CAP} solves reached at a {:+.6e}",
                    cur.a
                );
                out.stop = "lost".to_string();
                break 'rungs;
            }
            solves += 1;
            let t0 = Instant::now();
            let accepted = match cur.info.vt.as_ref() {
                None => Err("the previous factor refused".to_string()),
                Some(vt) => {
                    let dp = (tgt - cur.a) / dot(phi, vt);
                    let mut up = cur.s.clone();
                    for (&q, v) in free.iter().zip(vt) {
                        up.u[q] += dp * v;
                    }
                    let pred = case.at_load(&up, cur.s.p + dp);
                    let ctl = Control {
                        g: phi.to_vec(),
                        h: 0.0,
                        c: tgt,
                    };
                    match bordered(case, &pred, &ctl) {
                        Ok((s, its)) => {
                            let corr = max_abs(free.iter().map(|&q| s.u[q] - pred.u[q]));
                            let step = max_abs(free.iter().map(|&q| pred.u[q] - cur.s.u[q]));
                            if corr.is_finite() && step.is_finite() && corr <= step {
                                Ok((s, its, corr, step))
                            } else {
                                Err(format!("discontinuous ({corr:.4e} > {step:.4e}), {its} iterations"))
                            }
                        }
                        Err(e) => Err(format!("fails: {e}")),
                    }
                }
            };
            match accepted {
                Ok((s, its, corr, step)) => {
                    let info = analyse(case, &s);
                    let rung = if tgt == target { Some(k) } else { None };
                    let pt = apoint(case, s, info, phi, rung);
                    println!(
                        "{tag} amplitude solve {solves} (rung {k}{}): a {:+.9e} -> {:+.9e}: accepted, lambda {:.12}, nu {:?}, \
                         dlambda/da {:+.6e}, {its} iterations, continuity {corr:.3e} <= {step:.3e}, {:.1} s",
                        if rung.is_none() { ", halved" } else { "" },
                        cur.a,
                        pt.a,
                        pt.s.p,
                        pt.info.nu,
                        pt.sigma,
                        t0.elapsed().as_secs_f64()
                    );
                    out.points.push(pt);
                    let n = out.points.len();
                    let (p0, p1) = (&out.points[n - 2], &out.points[n - 1]);
                    if out.fold.is_none() && !undecided && p0.sigma < 0.0 && p1.sigma >= 0.0 {
                        let one = match (p0.info.nu, p1.info.nu) {
                            (Some(x), Some(y)) => x.abs_diff(y) == 1,
                            _ => false,
                        };
                        if one {
                            let mut f = fold_between(p0, p1);
                            f.k = n - 1;
                            out.fold = Some(f);
                        } else {
                            undecided = true;
                        }
                    }
                    let lam = out.points[n - 1].s.p;
                    if !(range.0..=range.1).contains(&lam) {
                        out.stop = "range".to_string();
                        break 'rungs;
                    }
                    if tgt == target {
                        break;
                    }
                    tgt = target;
                }
                Err(e) => {
                    println!(
                        "{tag} amplitude solve {solves} (rung {k}): a {:+.9e} -> {tgt:+.9e}: rejected, {e}, {:.1} s",
                        cur.a,
                        t0.elapsed().as_secs_f64()
                    );
                    if halvings == AMP_HALVINGS {
                        println!("{tag} amplitude trace: lost after a {:+.9e}", cur.a);
                        out.stop = "lost".to_string();
                        break 'rungs;
                    }
                    halvings += 1;
                    tgt = cur.a + 0.5 * (tgt - cur.a);
                }
            }
        }
        if out.fold.is_some() && out.fold.as_ref().unwrap().k < out.points.len() - 1 {
            after += 1;
        } else if out.fold.is_some() {
            after = 0;
        }
        if out.fold.is_some() && after == 2 {
            out.stop = "fold + 2".to_string();
            break;
        }
    }
    let last = out.points.last().unwrap();
    out.reading = if undecided {
        "undecided".to_string()
    } else if let Some(f) = &out.fold {
        let (p0, p1) = (&out.points[f.k - 1], &out.points[f.k]);
        format!(
            "fold: a in [{:.9e}, {:.9e}], lambda_f in [{:.12}, {:.12}], estimate a {:.9e} lambda {:.12}, nu {:?} -> {:?}",
            p0.a, p1.a, f.lo, f.hi, f.a_est, f.l_est, p0.info.nu, p1.info.nu
        )
    } else if out.stop == "lost" {
        format!(
            "lost at a = {:.9e}, lambda = {:.12} (dlambda/da {:+.6e})",
            last.a, last.s.p, last.sigma
        )
    } else {
        format!("no fold up to a = {:.9e} (lambda {:.12})", last.a, last.s.p)
    };
    println!(
        "{tag} READING AF: {}; stop {}, {} states, {solves} solves",
        out.reading,
        out.stop,
        out.points.len()
    );
    out
}

// ============================================================================ ADR 0718 D2, D3: the element's local problem

const MODES: usize = 9;
const NODES: usize = 24;
const ALL: usize = NODES + MODES;

type M9 = [[f64; MODES]; MODES];

/// An element at a state: its coordinates, its nodal displacement (node-major) and its state.
struct Elem {
    coords: Vec<Vec3>,
    ue: Vec<f64>,
    state: ElementState,
}

fn element_of(m: &SolidModel, e: usize, u: &[f64], states: &[ElementState]) -> Elem {
    let mesh = m.mesh();
    let dofs = &m.dof_map();
    let nodes = mesh.element_nodes(e);
    Elem {
        coords: nodes.iter().map(|&n| mesh.nodes()[n]).collect(),
        ue: nodes
            .iter()
            .flat_map(|&n| (0..3).map(move |c| u[dofs.equation(n, c).unwrap()]))
            .collect(),
        state: states[e].clone(),
    }
}

/// The force (33 values) and the mode block `K_aa` of the extended element at the modes `alpha` (D2).
fn mode_block(mat: &Material, el: &Elem, alpha: &[f64; MODES]) -> Result<(Vec<f64>, M9), String> {
    let (f, k) =
        enhanced_force_and_tangent_with(&el.coords, mat, &el.ue, alpha, &el.state, &PointContext::default(), LIN)
            .map_err(|e| e.to_string())?;
    let v = k.values();
    Ok((
        f,
        std::array::from_fn(|i| std::array::from_fn(|j| v[(NODES + i) * ALL + NODES + j])),
    ))
}

/// The asymmetry `max |K - K^T| / max |K|` and the eigenvalues (ascending) of the symmetric part.
fn spectrum(k: &M9) -> (f64, Vec<f64>) {
    let amax = k.iter().flatten().fold(0.0f64, |w, x| w.max(x.abs()));
    let mut asym = 0.0f64;
    for i in 0..MODES {
        for j in 0..MODES {
            asym = asym.max((k[i][j] - k[j][i]).abs());
        }
    }
    let dense: Vec<f64> = k.iter().flatten().copied().collect();
    let eig = symmetric_eigen(&dense, MODES)
        .map(|e| e.values)
        .unwrap_or_else(|_| vec![f64::NAN; MODES]);
    (asym / amax, eig)
}

fn det3(f: &[[f64; 3]; 3]) -> f64 {
    f[0][0] * (f[1][1] * f[2][2] - f[1][2] * f[2][1]) - f[0][1] * (f[1][0] * f[2][2] - f[1][2] * f[2][0])
        + f[0][2] * (f[1][0] * f[2][1] - f[1][1] * f[2][0])
}

/// The smallest `det F` over the points at the modes `alpha`.
fn det_min(el: &Elem, alpha: &[f64; MODES]) -> f64 {
    match point_gradients(&el.coords, &el.ue, alpha) {
        Ok(g) => g
            .iter()
            .map(|(h, fm)| {
                let f: [[f64; 3]; 3] = std::array::from_fn(|i| {
                    std::array::from_fn(|j| h[i][j] + fm[i][j] + if i == j { 1.0 } else { 0.0 })
                });
                det3(&f)
            })
            .fold(f64::INFINITY, f64::min),
        Err(_) => f64::NAN,
    }
}

/// D2: the element's local problem at a converged state.
struct ElemDiag {
    alpha: [f64; MODES],
    asym: f64,
    eig: Vec<f64>,
    its: usize,
    det: f64,
}

fn elem_diag(mat: &Material, el: &Elem) -> Result<ElemDiag, String> {
    let (rec, out) = record_from(mat, el, None);
    let alpha = out?;
    let (_, k) = mode_block(mat, el, &alpha)?;
    let (asym, eig) = spectrum(&k);
    Ok(ElemDiag {
        alpha,
        asym,
        eig,
        its: rec.len().saturating_sub(1),
        det: det_min(el, &alpha),
    })
}

/// D3 (b): the LU with partial pivoting of the mode block and its solve, a copy of `ModeFactor` of `nl_incompatible.rs` (the
/// same order of operations and the same singular-pivot test).
fn mode_lu_solve(k: &M9, b: &[f64; MODES]) -> Result<[f64; MODES], String> {
    let mut lu = *k;
    let amax = lu.iter().flatten().fold(0.0f64, |w, v| w.max(v.abs()));
    let mut perm: [usize; MODES] = std::array::from_fn(|i| i);
    for c in 0..MODES {
        let p = (c..MODES)
            .max_by(|&a, &b| lu[a][c].abs().total_cmp(&lu[b][c].abs()))
            .expect("a row");
        let pivot = lu[p][c];
        if pivot.is_nan() || pivot.abs() <= 1e-14 * amax || !pivot.is_finite() {
            return Err(format!(
                "key 'internal modes': the mode matrix of an Incompatible Hex8 is singular (pivot {pivot:e} at column {c}, \
                 largest entry {amax:e})"
            ));
        }
        lu.swap(c, p);
        perm.swap(c, p);
        for r in c + 1..MODES {
            let l = lu[r][c] / pivot;
            lu[r][c] = l;
            for j in c + 1..MODES {
                lu[r][j] -= l * lu[c][j];
            }
        }
    }
    let mut x: [f64; MODES] = std::array::from_fn(|i| b[perm[i]]);
    for i in 0..MODES {
        for j in 0..i {
            x[i] -= lu[i][j] * x[j];
        }
    }
    for i in (0..MODES).rev() {
        for j in i + 1..MODES {
            x[i] -= lu[i][j] * x[j];
        }
        x[i] /= lu[i][i];
    }
    Ok(x)
}

/// One iterate of the replica.
struct RepIt {
    alpha: [f64; MODES],
    r2: f64,
    det: f64,
    lmin: f64,
    lmax: f64,
}

/// D3 (b): the replica of the plain local Newton from `alpha = 0`: `n` evaluations (the record's length), and one more if
/// `extra` (the library's next evaluation was refused); the iterates and the refusal text, if any.
fn replica(mat: &Material, el: &Elem, n: usize, extra: bool) -> (Vec<RepIt>, Option<String>) {
    let mut alpha = [0.0; MODES];
    let mut out = Vec::new();
    let evals = if extra { n + 1 } else { n };
    for it in 0..evals {
        let (f, k) = match mode_block(mat, el, &alpha) {
            Ok(x) => x,
            Err(e) => return (out, Some(e)),
        };
        let r: [f64; MODES] = std::array::from_fn(|i| f[NODES + i]);
        let (_, eig) = spectrum(&k);
        out.push(RepIt {
            alpha,
            r2: r.iter().map(|v| v * v).sum::<f64>().sqrt(),
            det: det_min(el, &alpha),
            lmin: eig[0],
            lmax: eig[MODES - 1],
        });
        if it + 1 == evals {
            break;
        }
        let delta = match mode_lu_solve(&k, &r) {
            Ok(d) => d,
            Err(e) => return (out, Some(e)),
        };
        for (a, d) in alpha.iter_mut().zip(&delta) {
            *a -= d;
        }
    }
    (out, None)
}

fn inf9(a: &[f64; MODES]) -> f64 {
    a.iter()
        .fold(0.0f64, |w, x| if x.is_finite() { w.max(x.abs()) } else { f64::NAN })
}

/// The element's record of the plain local iteration from `start` (`None`: `alpha = 0`): the iterates and the outcome.
type Record = (Vec<ModeIterate>, Result<[f64; MODES], String>);

fn record_from(mat: &Material, el: &Elem, start: Option<&[f64; MODES]>) -> Record {
    match mode_iteration_record_from(
        &el.coords,
        mat,
        &el.ue,
        &el.state,
        &PointContext::default(),
        LIN,
        IncompatibleModeUpdate::Plain,
        start,
    ) {
        Ok((rec, out)) => (rec, out.map_err(|e| e.to_string())),
        Err(e) => (Vec::new(), Err(e.to_string())),
    }
}

/// The element `u_e(t) = u_0 + t (u_1 - u_0)`.
fn on_segment(e0: &Elem, e1: &Elem, t: f64) -> Elem {
    Elem {
        coords: e0.coords.clone(),
        ue: e0.ue.iter().zip(&e1.ue).map(|(a, b)| a + t * (b - a)).collect(),
        state: e0.state.clone(),
    }
}

/// D3 (d), (e): on the segment from the converged element `e0` to the failing one `e1`: the first `t*` where the plain
/// iteration from 0 fails (30 bisections) and `l_min` at its converged side; the continuation from `alpha_0` in 64 steps: the
/// last `t` reached, the smallest `l_min` along it and `l_min` at the last `t`.
fn segment(mat: &Material, e0: &Elem, e1: &Elem, alpha0: &[f64; MODES]) -> String {
    let ok_at = |t: f64| record_from(mat, &on_segment(e0, e1, t), None).1.is_ok();
    let lmin_at = |t: f64, a: &[f64; MODES]| {
        mode_block(mat, &on_segment(e0, e1, t), a)
            .map(|(_, k)| spectrum(&k).1[0])
            .unwrap_or(f64::NAN)
    };
    let bis = if ok_at(0.0) && !ok_at(1.0) {
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..30 {
            let mid = 0.5 * (lo + hi);
            if ok_at(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let el = on_segment(e0, e1, lo);
        let lm = record_from(mat, &el, None)
            .1
            .map(|a| lmin_at(lo, &a))
            .unwrap_or(f64::NAN);
        format!("t* in [{lo:.9}, {hi:.9}], l_min at the converged side {lm:+.6e}")
    } else {
        format!("no bisection (converges at 0: {}, at 1: {})", ok_at(0.0), ok_at(1.0))
    };
    let mut a = *alpha0;
    let mut last = 0.0;
    let mut smallest = lmin_at(0.0, &a);
    let mut at_last = smallest;
    for s in 1..=64 {
        let t = s as f64 / 64.0;
        match record_from(mat, &on_segment(e0, e1, t), Some(&a)).1 {
            Ok(x) => {
                a = x;
                last = t;
                at_last = lmin_at(t, &a);
                smallest = smallest.min(at_last);
            }
            Err(_) => break,
        }
    }
    format!(
        "{bis}; the continuation from alpha_0 reaches t {last:.6}, the smallest l_min {smallest:+.6e}, l_min at the last t \
         {at_last:+.6e}"
    )
}

// ============================================================================ ADR 0718 D4: the fine trace and the reading

/// The bordered Newton of [`bordered`] (the same arithmetic) that keeps, on a failure, the iterate whose evaluation failed;
/// with `verbose` each iterate is printed.
fn bordered_rec<B: Bordered>(
    case: &B,
    start: &St,
    ctl: &Control,
    verbose: bool,
) -> Result<(St, usize), (String, Option<St>)> {
    let free = case.free();
    let (tol, etol) = case.tolerances();
    let mut s = start.clone();
    let (mut e_first, mut e_last) = (0.0f64, 0.0f64);
    for k in 0..=20 {
        let (r, m) = match case.residual(&s) {
            Ok(x) => x,
            Err(e) => {
                if verbose {
                    println!("    global iterate {k}: lambda {:.15}: the residual fails", s.p);
                }
                return Err((e, Some(s)));
            }
        };
        if !m.is_finite() {
            return Err((format!("non-finite residual at iteration {k}"), Some(s)));
        }
        let gu: f64 = ctl.g.iter().zip(&free).map(|(g, &q)| g * s.u[q]).sum();
        let cres = gu + ctl.h * s.p - ctl.c;
        if verbose {
            println!(
                "    global iterate {k}: lambda {:.15}, residual {m:.4e}, constraint {cres:+.4e}",
                s.p
            );
        }
        let c_ok = cres.abs() <= 1e-9 * ctl.c.abs().max(1e-300);
        let e_ok = etol.map_or(true, |t| e_last <= t * e_first);
        if k >= 1 && m <= tol && e_ok && c_ok {
            return Ok((s, k));
        }
        if k == 20 {
            break;
        }
        let sys = case.system(&s);
        let Some(fac) = factor_nonzero(&sys.k) else {
            return Err((format!("the factor refused at iteration {k}"), None));
        };
        let mr: Vec<f64> = r.iter().map(|x| -x).collect();
        let y1 = fac.solve(&mr);
        let y2 = fac.solve(&sys.f);
        let den = dot(&ctl.g, &y2) + ctl.h;
        let dl = (-cres - dot(&ctl.g, &y1)) / den;
        if !dl.is_finite() {
            return Err((format!("non-finite load correction at iteration {k}"), None));
        }
        let du: Vec<f64> = y1.iter().zip(&y2).map(|(a, b)| a + dl * b).collect();
        let e = du
            .iter()
            .zip(sys.f.iter().zip(&r))
            .map(|(d, (f, ri))| d * (dl * f - ri))
            .sum::<f64>()
            .abs();
        if k == 0 {
            e_first = e;
        }
        e_last = e;
        for (&q, d) in free.iter().zip(&du) {
            s.u[q] += d;
        }
        s = case.at_load(&s, s.p + dl);
    }
    Err(("not converged in 20 iterations".to_string(), None))
}

/// One attempt of ADR 0711 D3 from `cur` to the amplitude `tgt`: the tangent predictor, the bordered Newton, the continuity
/// test; the state, the iterations, the correction and the step, or the text and the failing iterate.
type Attempt = Result<(St, usize, f64, f64), (String, Option<St>)>;

fn attempt<B: Bordered>(case: &B, cur: &APoint, phi: &[f64], tgt: f64, verbose: bool) -> Attempt {
    let free = case.free();
    let Some(vt) = cur.info.vt.as_ref() else {
        return Err(("the previous factor refused".to_string(), None));
    };
    let dp = (tgt - cur.a) / dot(phi, vt);
    let mut up = cur.s.clone();
    for (&q, v) in free.iter().zip(vt) {
        up.u[q] += dp * v;
    }
    let pred = case.at_load(&up, cur.s.p + dp);
    let ctl = Control {
        g: phi.to_vec(),
        h: 0.0,
        c: tgt,
    };
    let (s, its) = bordered_rec(case, &pred, &ctl, verbose)?;
    let corr = max_abs(free.iter().map(|&q| s.u[q] - pred.u[q]));
    let step = max_abs(free.iter().map(|&q| pred.u[q] - cur.s.u[q]));
    if corr.is_finite() && step.is_finite() && corr <= step {
        Ok((s, its, corr, step))
    } else {
        Err((
            format!("discontinuous ({corr:.4e} > {step:.4e}), {its} iterations"),
            None,
        ))
    }
}

/// An accepted state of the fine trace: the state, its target, `l_min` of the watched local tangent, the continuity numbers,
/// and the diagnostics of the case.
struct FPoint {
    pt: APoint,
    target: f64,
    lmin: f64,
    corr: f64,
    step: f64,
    extra: Vec<f64>,
}

/// A failed attempt: its target, its text, whether it is local, the failing iterate, the index of the state it started from.
struct FFail {
    target: f64,
    text: String,
    local: bool,
    iterate: Option<St>,
    from: usize,
}

struct FTrace {
    points: Vec<FPoint>,
    fails: Vec<FFail>,
    /// "fold + 2", "steps", "lost".
    stop: String,
    /// The index of the second state of the first sign change of `sigma`.
    sign: Option<usize>,
    solves: usize,
}

struct FineOpts {
    da: f64,
    steps: usize,
    halvings: usize,
    cap: usize,
}

/// The watched local tangent's smallest eigenvalue at a state.
type LminOf<'a> = &'a dyn Fn(&St) -> f64;
/// Whether a failure (its iterate, its text) is local.
type LocalOf<'a> = &'a dyn Fn(Option<&St>, &str) -> bool;

/// ADR 0718 D4: the fine trace from `from` over the targets `a_0 + j da`, `j = 1..=steps`; a failure halved toward its target,
/// the halvings reset on each acceptance, at most `halvings` in a row, at most `cap` solves.
#[allow(clippy::too_many_arguments)]
fn fine_trace<B: Bordered>(
    case: &B,
    tag: &str,
    from: APoint,
    phi: &[f64],
    o: &FineOpts,
    lmin: LminOf,
    local: LocalOf,
    at_state: &mut dyn FnMut(&APoint) -> Vec<f64>,
) -> FTrace {
    let a0 = from.a;
    let extra = at_state(&from);
    let lm = lmin(&from.s);
    println!(
        "{tag} fine trace: start lambda {:.12}, a {a0:+.9e}, nu {:?}, dlambda/da {:+.6e}, l_min {lm:+.6e}, Da {:.6e}",
        from.s.p, from.info.nu, from.sigma, o.da
    );
    let mut out = FTrace {
        points: vec![FPoint {
            target: a0,
            pt: from,
            lmin: lm,
            corr: 0.0,
            step: 0.0,
            extra,
        }],
        fails: Vec::new(),
        stop: String::new(),
        sign: None,
        solves: 0,
    };
    let t_all = Instant::now();
    'steps: for j in 1..=o.steps {
        let target = a0 + j as f64 * o.da;
        let mut tgt = target;
        let mut h = 0;
        loop {
            if out.solves >= o.cap {
                println!("{tag} fine trace: the cap of {} solves reached", o.cap);
                out.stop = "lost".to_string();
                break 'steps;
            }
            out.solves += 1;
            let t0 = Instant::now();
            let cur = &out.points.last().unwrap().pt;
            match attempt(case, cur, phi, tgt, false) {
                Ok((s, its, corr, step)) => {
                    let info = analyse(case, &s);
                    let rung = if tgt == target { Some(j) } else { None };
                    let pt = apoint(case, s, info, phi, rung);
                    let lm = lmin(&pt.s);
                    let extra = at_state(&pt);
                    println!(
                        "{tag} fine solve {} (step {j}{}): a -> {:+.9e}: accepted, lambda {:.12}, nu {:?}, dlambda/da \
                         {:+.6e}, l_min {lm:+.6e}, {its} iterations, continuity {corr:.3e} <= {step:.3e}, {:.1} s (total {:.1} s)",
                        out.solves,
                        if rung.is_none() { ", halved" } else { "" },
                        pt.a,
                        pt.s.p,
                        pt.info.nu,
                        pt.sigma,
                        t0.elapsed().as_secs_f64(),
                        t_all.elapsed().as_secs_f64()
                    );
                    out.points.push(FPoint {
                        pt,
                        target: tgt,
                        lmin: lm,
                        corr,
                        step,
                        extra,
                    });
                    h = 0;
                    let n = out.points.len();
                    if out.sign.is_none() && out.points[n - 2].pt.sigma < 0.0 && out.points[n - 1].pt.sigma >= 0.0 {
                        out.sign = Some(n - 1);
                    }
                    if let Some(k) = out.sign {
                        if n > k + 2 {
                            out.stop = "fold + 2".to_string();
                            break 'steps;
                        }
                    }
                    if tgt == target {
                        break;
                    }
                    tgt = target;
                }
                Err((text, iterate)) => {
                    let is_local = local(iterate.as_ref(), &text);
                    let ca = cur.a;
                    println!(
                        "{tag} fine solve {} (step {j}): a {ca:+.9e} -> {tgt:+.9e}: rejected (local {is_local}), {text}, \
                         {:.1} s",
                        out.solves,
                        t0.elapsed().as_secs_f64()
                    );
                    out.fails.push(FFail {
                        target: tgt,
                        text,
                        local: is_local,
                        iterate,
                        from: out.points.len() - 1,
                    });
                    if h == o.halvings {
                        println!("{tag} fine trace: lost after a {ca:+.9e}");
                        out.stop = "lost".to_string();
                        break 'steps;
                    }
                    h += 1;
                    tgt = ca + 0.5 * (tgt - ca);
                }
            }
        }
    }
    if out.stop.is_empty() {
        out.stop = "steps".to_string();
    }
    out
}

const RHO_REG: f64 = 0.1;

/// ADR 0718 D4: `rho_j = l_min,j / l_min,0` (NaN if `l_min,0 <= 0`).
fn rho_of(t: &FTrace, j: usize) -> f64 {
    let l0 = t.points[0].lmin;
    if l0 > 0.0 {
        t.points[j].lmin / l0
    } else {
        f64::NAN
    }
}

/// ADR 0718 D4: the reading of the fine trace.
fn reading(t: &FTrace) -> String {
    let p = &t.points;
    let n = p.len();
    let lsign = |j: usize| p[j - 1].lmin * p[j].lmin < 0.0;
    if let Some(j) = t.sign {
        let one = match (p[j - 1].pt.info.nu, p[j].pt.info.nu) {
            (Some(x), Some(y)) => x.abs_diff(y) == 1,
            _ => false,
        };
        let regular = [j - 1, j].iter().all(|&k| p[k].lmin > 0.0 && rho_of(t, k) >= RHO_REG);
        let earlier = (1..=j).any(lsign);
        return if one && regular && !earlier {
            "fold".to_string()
        } else {
            "undecided".to_string()
        };
    }
    if (1..n).any(lsign) {
        return "local breakdown: the mode tangent".to_string();
    }
    if t.stop == "lost" {
        let rho = rho_of(t, n - 1);
        if rho < RHO_REG {
            return "local breakdown: the mode tangent".to_string();
        }
        let last: Vec<&FFail> = t.fails.iter().filter(|f| f.from == n - 1).collect();
        if rho >= RHO_REG && !last.is_empty() && last.iter().all(|f| f.local) {
            return "local breakdown: the local iteration".to_string();
        }
    }
    "undecided".to_string()
}

/// The smallest failed target after the last accepted state.
fn t_fail_min(t: &FTrace) -> f64 {
    let n = t.points.len();
    t.fails
        .iter()
        .filter(|f| f.from == n - 1)
        .map(|f| f.target)
        .fold(f64::INFINITY, f64::min)
}

/// The reported companions of the reading: the extrapolated amplitudes where `sigma` and `l_min` vanish, `|sigma_last| / max
/// |sigma|`, the end interval.
fn companions(t: &FTrace) -> String {
    let p = &t.points;
    let n = p.len();
    let ex = |f: &dyn Fn(&FPoint) -> f64| {
        if n < 2 {
            return f64::NAN;
        }
        let (x0, x1) = (&p[n - 2], &p[n - 1]);
        x1.pt.a - f(x1) * (x1.pt.a - x0.pt.a) / (f(x1) - f(x0))
    };
    let smax = p.iter().fold(0.0f64, |w, x| w.max(x.pt.sigma.abs()));
    format!(
        "a where sigma -> 0 (linear, last two states) {:.6e}; a where l_min -> 0 {:.6e}; |sigma_last| / max |sigma| {:.4e}; \
         the end in [{:.9e}, {:.9e}]; rho_last {:.4e}; {} solves, stop {}",
        ex(&|x| x.pt.sigma),
        ex(&|x| x.lmin),
        p[n - 1].pt.sigma.abs() / smax,
        p[n - 1].pt.a,
        t_fail_min(t),
        rho_of(t, n - 1),
        t.solves,
        t.stop
    )
}

// ============================================================================ ADR 0718 D5: the toy with a local unknown

const K_L_TOY: f64 = 1.0;

/// PL (ADR 0718 D5): PT of ADR 0711 (`sb = 1`) with the local unknown `z` of `W_l = (k_l/2) z^2 - z^3/3 - c_l u_0^2 z`: `g = k_l
/// z - z^2 - c_l u_0^2 = 0` solved per evaluation by Newton from 0, the term `-2 c_l u_0 z` in `r_0`.
struct Pl {
    b3: f64,
    b5: f64,
    cl: f64,
}

impl Pl {
    /// The local Newton at `a` from `start` (25 iterations): the iterates and the root.
    fn local(&self, a: f64, start: f64) -> (Vec<f64>, Result<f64, String>) {
        let mut z = start;
        let mut its = Vec::new();
        for it in 0..=25 {
            let g = K_L_TOY * z - z * z - self.cl * a * a;
            let sc = K_L_TOY * z.abs() + z * z + self.cl * a * a;
            its.push(z);
            if !g.is_finite() {
                return (its, Err(format!("local: not finite at iteration {it}")));
            }
            if g.abs() <= 1e-14 * sc {
                return (its, Ok(z));
            }
            if it == 25 {
                break;
            }
            let gz = K_L_TOY - 2.0 * z;
            z -= g / gz;
        }
        (its, Err("local: not converged in 25 iterations".to_string()))
    }
    fn z_of(&self, a: f64) -> f64 {
        (K_L_TOY - (K_L_TOY * K_L_TOY - 4.0 * self.cl * a * a).sqrt()) / 2.0
    }
    fn p_of(&self, a: f64) -> f64 {
        (1.0 + (self.b3 - 1.0) * a * a + self.b5 * a.powi(4) - 2.0 * self.cl * self.z_of(a)) / 0.9
    }
    fn dp_da(&self, a: f64) -> f64 {
        let zp = 2.0 * self.cl * a / (K_L_TOY - 2.0 * self.z_of(a));
        (2.0 * (self.b3 - 1.0) * a + 4.0 * self.b5 * a.powi(3) - 2.0 * self.cl * zp) / 0.9
    }
    fn a_star(&self) -> f64 {
        K_L_TOY / (2.0 * self.cl.sqrt())
    }
    fn eval(&self, u: &[f64], p: f64) -> Result<(Vec<f64>, f64), String> {
        let z = self.local(u[0], 0.0).1?;
        let r = (0..TOY_N)
            .map(|i| match i {
                0 => {
                    (1.0 - p) * u[0] + self.b3 * u[0].powi(3) + self.b5 * u[0].powi(5) + 2.0 * PT_C * u[0] * u[1]
                        - 2.0 * self.cl * u[0] * z
                }
                1 => 2.0 * u[1] - 0.1 * p + PT_C * u[0] * u[0],
                _ => k_i(i) * u[i] - p * b_i(i),
            })
            .collect();
        Ok((r, z))
    }
    fn dense(&self, u: &[f64], p: f64, z: f64) -> Vec<f64> {
        let mut a = vec![0.0; TOY_N * TOY_N];
        let zp = 2.0 * self.cl * u[0] / (K_L_TOY - 2.0 * z);
        a[0] = 1.0 - p + 3.0 * self.b3 * u[0] * u[0] + 5.0 * self.b5 * u[0].powi(4) + 2.0 * PT_C * u[1]
            - 2.0 * self.cl * z
            - 2.0 * self.cl * u[0] * zp;
        a[1] = 2.0 * PT_C * u[0];
        a[TOY_N] = 2.0 * PT_C * u[0];
        a[TOY_N + 1] = 2.0;
        for i in 2..TOY_N {
            a[TOY_N * i + i] = k_i(i);
        }
        a
    }
    /// The closed-form branch state at the amplitude `a` (not polished).
    fn guess(&self, a: f64) -> St {
        let p = self.p_of(a);
        St {
            p,
            u: (0..TOY_N)
                .map(|i| match i {
                    0 => a,
                    1 => (0.1 * p - PT_C * a * a) / 2.0,
                    _ => p * b_i(i) / k_i(i),
                })
                .collect(),
            states: Vec::new(),
        }
    }
}

impl Case for Pl {
    /// Not used by the fine trace: Newton at the fixed load.
    fn solve(&self, from: &St, to: f64) -> Result<St, String> {
        let mut u = from.u.clone();
        for _ in 0..=20 {
            let (r, z) = self.eval(&u, to)?;
            if max_abs(r.iter().copied()) <= 1e-14 {
                return Ok(St {
                    p: to,
                    u,
                    states: Vec::new(),
                });
            }
            let du = dense_solve(self.dense(&u, to, z), r.iter().map(|v| -v).collect(), TOY_N)
                .ok_or_else(|| "singular".to_string())?;
            for (ui, d) in u.iter_mut().zip(&du) {
                *ui += d;
            }
        }
        Err("not converged in 20 iterations".to_string())
    }
    fn system(&self, s: &St) -> Sys {
        let z = self.local(s.u[0], 0.0).1.unwrap_or(f64::NAN);
        let f = (0..TOY_N)
            .map(|i| match i {
                0 => s.u[0],
                1 => 0.1,
                _ => b_i(i),
            })
            .collect();
        Sys {
            k: csr_of_dense(&self.dense(&s.u, s.p, z), TOY_N),
            f,
            free: (0..TOY_N).collect(),
        }
    }
    fn lowest_pair(&self, s: &Sys, _nu: usize) -> Option<Pair> {
        dense_lowest(&s.k)
    }
}

impl Branch for Pl {
    fn fixed(&self, start: &St, p: f64, _eta: f64, _g: &[f64]) -> Result<St, String> {
        self.solve(start, p)
    }
    fn mirror(&self, u: &[f64]) -> Vec<f64> {
        u.iter()
            .enumerate()
            .map(|(i, &v)| if i == 0 { -v } else { v })
            .collect()
    }
    fn free(&self) -> Vec<usize> {
        (0..TOY_N).collect()
    }
}

impl Bordered for Pl {
    fn residual(&self, s: &St) -> Result<(Vec<f64>, f64), String> {
        let (r, _) = self.eval(&s.u, s.p)?;
        let m = max_abs(r.iter().copied());
        Ok((r, m))
    }
    fn tolerances(&self) -> (f64, Option<f64>) {
        (1e-14, None)
    }
    fn at_load(&self, s: &St, p: f64) -> St {
        St {
            p,
            u: s.u.clone(),
            states: Vec::new(),
        }
    }
}

/// The toy's fine trace from the closed-form state at `a0` polished by the bordered Newton.
fn toy_trace(pl: &Pl, tag: &str, a0: f64, o: &FineOpts, ch: &mut Checks) -> FTrace {
    let phi: Vec<f64> = (0..TOY_N).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();
    let ctl = Control {
        g: phi.clone(),
        h: 0.0,
        c: a0,
    };
    let s0 = bordered(pl, &pl.guess(a0), &ctl)
        .map(|x| x.0)
        .unwrap_or_else(|_| pl.guess(f64::NAN));
    ch.at_most(
        &format!("{tag}-0: |p - p(a0)| at the start"),
        (s0.p - pl.p_of(a0)).abs(),
        1e-12,
    );
    let info = analyse(pl, &s0);
    let start = apoint(pl, s0, info, &phi, Some(0));
    let lmin = |s: &St| pl.local(s.u[0], 0.0).1.map(|z| K_L_TOY - 2.0 * z).unwrap_or(f64::NAN);
    let local = |_: Option<&St>, text: &str| text.starts_with("local");
    let mut at = |p: &APoint| vec![dense_lowest(&p.info.sys.k).map(|x| x.mu).unwrap_or(f64::NAN)];
    fine_trace(pl, tag, start, &phi, o, &lmin, &local, &mut at)
}

/// The closed-form checks at every accepted state of a toy trace (PF-1, PB-1).
#[allow(clippy::too_many_arguments)]
fn toy_states(pl: &Pl, tag: &str, t: &FTrace, lp: f64, ls: f64, lg: f64, nu1: bool, ch: &mut Checks) {
    let up = |w: f64, x: f64| if x.is_finite() { w.max(x) } else { f64::NAN };
    let (mut ep, mut es, mut eg) = (0.0f64, 0.0f64, 0.0f64);
    let mut nus = true;
    for f in &t.points {
        let a = f.pt.a;
        ep = up(ep, (f.pt.s.p - pl.p_of(a)).abs());
        es = up(es, ((f.pt.sigma - pl.dp_da(a)) / pl.dp_da(a)).abs());
        eg = up(eg, (f.lmin - (1.0 - 2.0 * pl.z_of(a))).abs());
        nus &= f.pt.info.nu == Some(1);
        println!(
            "{tag} TABLE a {a:.12}: p {:.12} (closed form {:.12}), dp/da {:+.9e} (closed form {:+.9e}), g_z {:.9e}, rho {:.6}, \
             nu {:?}, mu_1 {:+.6e}",
            f.pt.s.p,
            pl.p_of(a),
            f.pt.sigma,
            pl.dp_da(a),
            f.lmin,
            f.lmin / t.points[0].lmin,
            f.pt.info.nu,
            f.extra[0]
        );
    }
    ch.at_most(
        &format!("{tag}-1: max |p - p(a)| over {} states", t.points.len()),
        ep,
        lp,
    );
    ch.at_most(&format!("{tag}-1: max |sigma - dp/da| / |dp/da|"), es, ls);
    ch.at_most(&format!("{tag}-1: max |g_z - (1 - 2 z(a))|"), eg, lg);
    if nu1 {
        ch.holds(&format!("{tag}-1: nu = 1 at every state"), nus);
    }
}

// ============================================================================ the fast test

#[test]
fn f796_fast() {
    let mut ch = Checks::new();
    let t_all = Instant::now();

    // PL-F: a closed-form fold with a regular local problem.
    let pf = Pl {
        b3: 0.5,
        b5: 0.25,
        cl: 1.0 / 16.0,
    };
    let (mut lo, mut hi) = (0.9, 1.2);
    for _ in 0..200 {
        let m = 0.5 * (lo + hi);
        if pf.dp_da(m) < 0.0 {
            lo = m;
        } else {
            hi = m;
        }
    }
    let a_f = 0.5 * (lo + hi);
    println!("PL-F: the closed-form fold a_f {a_f:.12}, p_f {:.12}", pf.p_of(a_f));
    let of = FineOpts {
        da: 0.05,
        steps: 16,
        halvings: 6,
        cap: 60,
    };
    let tf = toy_trace(&pf, "PF", 0.8, &of, &mut ch);
    toy_states(&pf, "PF", &tf, 1e-12, 1e-8, 1e-12, false, &mut ch);
    ch.holds(
        &format!("PF-2: no failed attempt ({})", tf.fails.len()),
        tf.fails.is_empty(),
    );
    ch.holds(
        &format!("PF-2: the stop 'fold + 2' ({})", tf.stop),
        tf.stop == "fold + 2",
    );
    let rf = reading(&tf);
    println!("PF READING: {rf}; {}", companions(&tf));
    ch.holds(&format!("PF-3: the reading 'fold' ({rf})"), rf == "fold");
    match tf.sign {
        Some(j) => {
            let (p0, p1) = (&tf.points[j - 1], &tf.points[j]);
            ch.holds(
                &format!("PF-3: a_f in [{:.6}, {:.6}]", p0.pt.a, p1.pt.a),
                p0.pt.a <= a_f && a_f <= p1.pt.a,
            );
            ch.equals("PF-3: nu before the fold", p0.pt.info.nu.unwrap_or(usize::MAX), 1);
            ch.equals("PF-3: nu after the fold", p1.pt.info.nu.unwrap_or(usize::MAX), 0);
            for (k, name) in [(j - 1, "before"), (j, "after")] {
                let rho = rho_of(&tf, k);
                ch.holds(&format!("PF-3: rho {name} the fold {rho:.6} >= 0.1"), rho >= RHO_REG);
            }
        }
        None => ch.holds("PF-3: the fold bracketed", false),
    }

    // PL-B: the local problem loses its root before the fold.
    let pb = Pl {
        b3: 0.5,
        b5: 0.25,
        cl: 0.390625,
    };
    let ob = FineOpts {
        da: 0.07,
        steps: 16,
        halvings: 6,
        cap: 60,
    };
    let tb = toy_trace(&pb, "PB", 0.5, &ob, &mut ch);
    toy_states(&pb, "PB", &tb, 1e-10, 1e-6, 1e-8, true, &mut ch);
    ch.holds(&format!("PB-2: the stop 'lost' ({})", tb.stop), tb.stop == "lost");
    let n = tb.points.len();
    let a_last = tb.points[n - 1].pt.a;
    let t_fail = t_fail_min(&tb);
    let a_star = pb.a_star();
    ch.holds(
        &format!("PB-2: a_last {a_last:.12} < a* {a_star} < t_fail,min {t_fail:.12}"),
        a_last < a_star && a_star < t_fail,
    );
    ch.at_most("PB-2: t_fail,min - a_last", t_fail - a_last, 0.07 / 64.0);
    let rb = reading(&tb);
    println!("PB READING: {rb}; {}", companions(&tb));
    ch.holds(
        &format!("PB-3: the reading 'local breakdown: the mode tangent' ({rb})"),
        rb == "local breakdown: the mode tangent",
    );
    let rho = rho_of(&tb, n - 1);
    ch.holds(&format!("PB-3: rho at the last state {rho:.6} < 0.1"), rho < RHO_REG);
    let sr = tb.points[n - 1].pt.sigma.abs() / tb.points[0].pt.sigma.abs();
    ch.holds(&format!("PB-3: |sigma_last| / |sigma_0| {sr:.4} > 10"), sr > 10.0);
    ch.holds(
        &format!("PB-4: every failed attempt local ({} failed)", tb.fails.len()),
        !tb.fails.is_empty() && tb.fails.iter().all(|f| f.local),
    );
    let mut warm_fails = true;
    let mut above = 0;
    for f in tb.fails.iter().filter(|f| f.target > a_star) {
        above += 1;
        let z_last = pb.local(tb.points[f.from].pt.a, 0.0).1.unwrap_or(f64::NAN);
        let (cold, _) = pb.local(f.target, 0.0);
        let (its, out) = pb.local(f.target, z_last);
        println!(
            "PB-4: the failed target {:.9}: the iterates from z = 0 {:?}; from z_last {z_last:.9}: {}",
            f.target,
            cold,
            match &out {
                Ok(z) => format!("converges to {z:.9} in {} iterations", its.len() - 1),
                Err(e) => e.clone(),
            }
        );
        warm_fails &= out.is_err();
    }
    ch.holds(
        &format!("PB-4: above a* ({above} targets) the local Newton from the last accepted z fails as well"),
        above > 0 && warm_fails,
    );

    // E1: the replica of the local iteration on one element.
    let mut mesh1 = hex_box([0.0; 3], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
    mesh1.set_block_property(0, BlockProperty::solid(INC)).unwrap();
    let mut m1 = SolidModel::new(mesh1);
    m1.set_incompatible_stabilisation(LIN);
    m1.add_material("all", neo_hookean()).unwrap();
    let st1 = initial_states(&m1).unwrap();
    let dofs1 = m1.dof_map();
    let mut u1 = vec![0.0; dofs1.n_equations()];
    for (i, p) in m1.mesh().nodes().iter().enumerate() {
        let v = [
            0.02 * (2.0 * p[1] - 1.0) * (2.0 * p[2] - 1.0),
            0.02 * (2.0 * p[0] - 1.0) * (2.0 * p[2] - 1.0),
            -0.15 * p[2],
        ];
        for (c, vc) in v.iter().enumerate() {
            u1[dofs1.equation(i, c).unwrap()] = *vc;
        }
    }
    let mat = neo_hookean();
    let el = element_of(&m1, 0, &u1, &st1);
    let (rec, out) = record_from(&mat, &el, None);
    ch.holds(
        &format!("E1: the record converges ({} iterates)", rec.len()),
        out.is_ok(),
    );
    let (rep, refusal) = replica(&mat, &el, rec.len(), false);
    let dr = rec
        .iter()
        .zip(&rep)
        .map(|(a, b)| ((a.residual_norm - b.r2) / a.residual_norm.max(1e-300)).abs())
        .fold(0.0f64, |w, x| if x.is_finite() { w.max(x) } else { f64::NAN });
    for (k, r) in rep.iter().enumerate() {
        println!(
            "E1 iterate {k}: |r|_2 {:.6e} (record {:.6e}), |alpha|_inf {:.6e}, det F min {:.6}, l_min {:+.6e}, l_max {:+.6e}",
            r.r2,
            rec[k].residual_norm,
            inf9(&r.alpha),
            r.det,
            r.lmin,
            r.lmax
        );
    }
    ch.holds(
        "E1: the replica has the record's iterates and no refusal",
        rep.len() == rec.len() && refusal.is_none(),
    );
    ch.at_most("E1: max |r|_2 replica against the record, relative", dr, 1e-14);
    match (&out, rep.last()) {
        (Ok(alpha), Some(r)) => {
            // The replica's last iterate is the library's converged modes.
            let d = alpha
                .iter()
                .zip(&r.alpha)
                .map(|(x, y)| (x - y).abs())
                .fold(0.0f64, f64::max)
                / inf9(alpha);
            ch.at_most("E1: |alpha_replica - alpha|_inf / |alpha|_inf", d, 1e-14);
            match mode_block(&mat, &el, alpha) {
                Ok((_, k)) => {
                    let (asym, eig) = spectrum(&k);
                    let tr: f64 = (0..MODES).map(|i| k[i][i]).sum();
                    let sum: f64 = eig.iter().sum();
                    let big = eig.iter().fold(0.0f64, |w, x| w.max(x.abs()));
                    println!("E1: the eigenvalues of K_aa {eig:?}");
                    ch.at_most("E1: the asymmetry of K_aa", asym, 1e-12);
                    ch.at_most("E1: |sum l_i - tr K_aa| / max |l_i|", (sum - tr).abs() / big, 1e-12);
                }
                Err(e) => ch.holds(&format!("E1: K_aa ({e})"), false),
            }
        }
        _ => ch.holds("E1: the converged modes", false),
    }
    println!("f796_fast: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}

// ============================================================================ the heavy record

const LF: f64 = 0.9647;
const LB: f64 = 0.9645;
/// ADR 0700's `a*_pred` (the `Debug` form of its log line 81).
const EPS_SWITCH: f64 = 2.7468713663317314e-5;
/// ADR 0711's rung 5 (`f789-scratch/run2-heavy.log` lines 93 and 110).
const LAMBDA_5: f64 = 0.958472223921;
const A_5: f64 = 2.164988001e-4;
const SIGMA_5: f64 = -5.382683e-1;
const MU_5: f64 = -323.40057121476354;
/// ADR 0711's five failing attempts of rung 6 (`run2-heavy.log` lines 94 to 98, after `fails: `).
const ADR0711_TEXTS: [&str; 5] = [
    "element 3720: key 'internal modes': the modes of an Incompatible Hex8 did not converge in 25 local iterations \
     (relative mode residual 3.602825838427964e-5, ADR 0608 D2)",
    "element 3720: key 'internal modes': the modes of an Incompatible Hex8 did not converge in 25 local iterations \
     (relative mode residual 2.333797178293884e-1, ADR 0608 D2)",
    "element 3720: key 'deformation gradient': det F = -1.4423001685690635e0 at point 1 of an Incompatible Hex8 element \
     (inverted)",
    "element 3720: key 'internal modes': the modes of an Incompatible Hex8 did not converge in 25 local iterations \
     (relative mode residual 3.688331482870459e-4, ADR 0608 D2)",
    "element 3720: key 'deformation gradient': det F = -8.366015484721789e-1 at point 1 of an Incompatible Hex8 element \
     (inverted)",
];

/// The element named by an element error (`element <e>: ...`).
fn element_in(text: &str) -> Option<usize> {
    text.strip_prefix("element ")?.split(':').next()?.parse().ok()
}

/// D3: the local diagnostics of a failing iterate in element `e` (the record, the replica, the start from `alpha_ref`, the
/// segment from `el_ref`): the record's outcome text, the replica's agreement and the refusal check.
#[allow(clippy::too_many_arguments)]
fn failing_local(
    tag: &str,
    mat: &Material,
    m: &SolidModel,
    e: usize,
    it: &St,
    el_ref: &Elem,
    alpha_ref: &[f64; MODES],
) -> (Result<[f64; MODES], String>, f64, bool, usize) {
    let elf = element_of(m, e, &it.u, &it.states);
    let du = max_abs(elf.ue.iter().zip(&el_ref.ue).map(|(a, b)| a - b));
    let (rec, out) = record_from(mat, &elf, None);
    let extra = matches!(&out, Err(t) if t.contains("'deformation gradient'"));
    let (rep, refusal) = replica(mat, &elf, rec.len(), extra);
    println!(
        "{tag}: element {e} at the failing iterate (|u_e - u_e,ref|_inf {du:.4e}): {} local iterates, outcome {}",
        rec.len(),
        match &out {
            Ok(_) => "converged".to_string(),
            Err(t) => t.clone(),
        }
    );
    let mut dr = if rep.len() == rec.len() { 0.0f64 } else { f64::NAN };
    for (k, r) in rep.iter().enumerate() {
        let lib = rec.get(k);
        if let Some(l) = lib {
            let x = ((l.residual_norm - r.r2) / l.residual_norm.max(1e-300)).abs();
            dr = if x.is_finite() { dr.max(x) } else { f64::NAN };
        }
        println!(
            "{tag}   local iterate {k}: |r|_2 {:.6e} (record {}), |r|_inf / scale {}, |alpha|_inf {:.6e}, det F min {:+.6}, \
             l_min {:+.6e}, l_max {:+.6e}",
            r.r2,
            lib.map_or("-".to_string(), |l| format!("{:.6e}", l.residual_norm)),
            lib.map_or("-".to_string(), |l| format!("{:.4e}", l.residual_max / l.scale)),
            inf9(&r.alpha),
            r.det,
            r.lmin,
            r.lmax
        );
    }
    let refusal_ok = match (&out, &refusal) {
        (Err(t), Some(r)) if extra => t == r,
        (_, None) => !extra,
        _ => false,
    };
    println!("{tag}   the replica's refusal: {refusal:?}");
    let (wrec, wout) = record_from(mat, &elf, Some(alpha_ref));
    println!(
        "{tag}   the local iteration from the reference modes: {} iterates, {}",
        wrec.len(),
        match &wout {
            Ok(a) => {
                let d = a
                    .iter()
                    .zip(alpha_ref)
                    .map(|(x, y)| (x - y).abs())
                    .fold(0.0f64, f64::max)
                    / inf9(alpha_ref);
                let lm = mode_block(mat, &elf, a)
                    .map(|(_, k)| spectrum(&k).1[0])
                    .unwrap_or(f64::NAN);
                format!("converged, |alpha - alpha_ref|_inf / |alpha_ref|_inf {d:.4e}, l_min {lm:+.6e}")
            }
            Err(t) => t.clone(),
        }
    );
    println!("{tag}   the segment: {}", segment(mat, el_ref, &elf, alpha_ref));
    (out, dr, refusal_ok, rec.len())
}

#[test]
#[ignore = "ADR 0718: the heavy record of the end of the branch on 16^3 (D1 to D4), run once with --ignored"]
fn f796_nh16_end_of_branch_heavy() {
    let mut ch = Checks::new();
    let t_all = Instant::now();
    let mesh16 = block_mesh(16);
    let m = block_model(&mesh16, K_L);
    let m2 = block_model_free_punch(&mesh16, K_L);
    let states = initial_states(&m).unwrap();
    let fb = FemBranch::new(&m, &m2, 16);
    let free = fb.free.clone();
    let mat = neo_hookean();
    let asym = |u: &[f64]| max_abs(u.iter().zip(fb.mirror(u)).map(|(x, y)| x - y)) / max_abs(u.iter().copied());

    // OB.
    let (text, log) = solve_observed(&mesh16, "block 16^3", K_L, &options(10));
    ch.holds("OB: the error text of ADR 0678", text == ADR0678_TEXT);
    let Some(fa) = failing_attempt(&log) else {
        ch.holds("OB: the failing attempt from 0.9625 observed", false);
        ch.finish();
        return;
    };
    let uc = St {
        p: log.attempts[fa].committed,
        u: log.attempts[fa].iterates[0].clone(),
        states: states.clone(),
    };

    // D1: the symmetric chain, the mode, the switch and the step to 0.9645 (ADR 0711 R0, R1).
    let mut cur = uc;
    let mut sym: Vec<St> = Vec::new();
    for p in [0.963, 0.9635, 0.964, LB, LF] {
        match fb.solve(&cur, p) {
            Ok(s) => {
                println!("D1 symmetric at {p:.12}: {:.1} s", t_all.elapsed().as_secs_f64());
                cur = s.clone();
                sym.push(s);
            }
            Err(e) => {
                ch.holds(&format!("D1: the symmetric chain at {p} ({e})"), false);
                ch.finish();
                return;
            }
        }
    }
    let (_, pf) = mode_at(&fb, "D1", &sym[4]);
    let Some(pf) = pf else {
        ch.holds("D1: the lowest pair at 0.9647", false);
        ch.finish();
        return;
    };
    let phi = pf.phi.clone();
    let zero = vec![0.0; free.len()];
    let mut start = sym[4].clone();
    for (k, &q) in free.iter().enumerate() {
        start.u[q] += EPS_SWITCH * phi[k];
    }
    let Ok(ssw) = fb.fixed(&start, LF, 0.0, &phi) else {
        ch.holds("D1: the switch at 0.9647 converges", false);
        ch.finish();
        return;
    };
    let Ok(sb) = fb.fixed(&ssw, LB, 0.0, &zero) else {
        ch.holds("D1: the step to 0.9645 converges", false);
        ch.finish();
        return;
    };
    let ib = analyse(&fb, &sb);
    let a_rel = amp(&phi, &free, &sb.u) - amp(&phi, &free, &sym[3].u);
    ch.at_most(
        "R1: a at 0.9645 against 3.827194476e-5",
        (a_rel / 3.827194476e-5 - 1.0).abs(),
        1e-8,
    );
    ch.equals("R1: nu at 0.9645", ib.nu.unwrap_or(usize::MAX), 1);

    // D1: ADR 0711's rungs 1 to 5.
    let atr = amp_trace(&fb, "AT", (sb.clone(), ib.clone()), &phi, 5, (0.5, 1.2));
    let rungs_ok = atr.points.len() == 6 && (1..=5).all(|k| atr.points[k].rung == Some(k));
    ch.holds("S5: rungs 1 to 5 accepted", rungs_ok);
    if !rungs_ok {
        ch.finish();
        return;
    }
    let s4 = atr.points[4].clone();
    let s5 = atr.points[5].clone();
    ch.at_most("S5: |lambda_5 - 0.958472223921|", (s5.s.p - LAMBDA_5).abs(), 1e-12);
    ch.at_most("S5: a_5 against 2.164988001e-4", (s5.a / A_5 - 1.0).abs(), 1e-9);
    ch.at_most(
        "S5: sigma_5 against -5.382683e-1",
        (s5.sigma / SIGMA_5 - 1.0).abs(),
        1e-6,
    );
    let id = identity(free.len());
    let points = points_of(&m, &free);
    let e5 = lowest_any(&s5.info.sys.k, &id, &points, ResidualTest::Backward);
    let mu5 = e5.as_ref().map(|e| e.values[0]).unwrap_or(f64::NAN);
    ch.at_most("S5: mu_1 against -323.40057121476354", (mu5 / MU_5 - 1.0).abs(), 1e-9);
    println!(
        "S5: lambda {:.15}, a {:+.12e}, sigma {:+.9e}, asymmetry {:.4e}, {:.1} s",
        s5.s.p,
        s5.a,
        s5.sigma,
        asym(&s5.s.u),
        t_all.elapsed().as_secs_f64()
    );

    // X1: the five failing attempts re-run.
    let a_b = atr.points[0].a;
    let mut tgt = a_b * 2f64.powf(6.0 / 2.0);
    let mut failed: Vec<(String, Option<St>)> = Vec::new();
    for (h, want) in ADR0711_TEXTS.iter().enumerate() {
        println!("X1 attempt {} from a {:+.9e} to {tgt:+.9e}:", h + 1, s5.a);
        match attempt(&fb, &s5, &phi, tgt, true) {
            Ok((s, its, _, _)) => {
                println!("X1 attempt {}: converged ({its} iterations, lambda {:.12})", h + 1, s.p);
                ch.holds(&format!("X1: attempt {} fails as in ADR 0711", h + 1), false);
                failed.push(("converged".to_string(), None));
            }
            Err((t, it)) => {
                println!("X1 attempt {}: {t}", h + 1);
                ch.holds(&format!("X1: attempt {} with ADR 0711's text", h + 1), t == *want);
                failed.push((t, it));
            }
        }
        tgt = s5.a + 0.5 * (tgt - s5.a);
    }
    let e_w = element_in(&failed[0].0).unwrap_or(usize::MAX);
    ch.equals("X1: the element of the first failure", e_w, 3720);
    if e_w >= m.mesh().n_elements() {
        ch.finish();
        return;
    }

    // X3: the element's mode tangent at S5 and the rank of the element.
    let el5 = element_of(&m, e_w, &s5.s.u, &s5.s.states);
    let Ok(d5) = elem_diag(&mat, &el5) else {
        ch.holds("X3: the element's local problem at S5 converges", false);
        ch.finish();
        return;
    };
    println!(
        "X3: element {e_w} at S5: the eigenvalues of K_aa {:?}; l_min / l_max {:+.6e}; {} local iterations; det F min {:.6}; \
         alpha {:?}",
        d5.eig,
        d5.eig[0] / d5.eig[MODES - 1],
        d5.its,
        d5.det,
        d5.alpha
    );
    ch.at_most("X3: the asymmetry of K_aa at S5", d5.asym, 1e-12);
    let mut ratios: Vec<(f64, usize)> = (0..m.mesh().n_elements())
        .map(|e| {
            let r = elem_diag(&mat, &element_of(&m, e, &s5.s.u, &s5.s.states))
                .map(|d| d.eig[0] / d.eig[MODES - 1])
                .unwrap_or(f64::NAN);
            (r, e)
        })
        .collect();
    ratios.sort_by(|a, b| a.0.total_cmp(&b.0));
    let rank = ratios.iter().position(|&(_, e)| e == e_w);
    println!(
        "X3: the rank of element {e_w} by l_min / l_max among {} elements: {rank:?}; the five smallest {:?}",
        ratios.len(),
        &ratios[..5]
    );

    // X2, X4: the iterates of the failing attempts.
    for (h, (t, it)) in failed.iter().enumerate() {
        let tag = format!("X2 attempt {}", h + 1);
        let Some(it) = it else {
            ch.holds(&format!("{tag}: the failing iterate kept"), false);
            continue;
        };
        let (out, dr, refusal_ok, n) = failing_local(&tag, &mat, &m, e_w, it, &el5, &d5.alpha);
        ch.holds(
            &format!("{tag}: the record's outcome in the attempt's text"),
            matches!(&out, Err(o) if t.ends_with(o.as_str())),
        );
        ch.at_most(
            &format!("{tag}: replica |r|_2 against the record ({n} iterates)"),
            dr,
            1e-14,
        );
        ch.holds(&format!("{tag}: the replica's refusal"), refusal_ok);
    }

    // D4: the fine trace from S4.
    let dofs = &m.dof_map();
    let pos: HashMap<usize, usize> = free.iter().enumerate().map(|(k, &q)| (q, k)).collect();
    let e_idx: Vec<usize> = m
        .mesh()
        .element_nodes(e_w)
        .iter()
        .flat_map(|&n| (0..3).map(move |c| (n, c)))
        .filter_map(|(n, c)| pos.get(&dofs.equation(n, c).unwrap()).copied())
        .collect();
    let share = |v: &[f64]| e_idx.iter().map(|&k| v[k] * v[k]).sum::<f64>() / v.iter().map(|x| x * x).sum::<f64>();
    let lmin = |s: &St| {
        elem_diag(&mat, &element_of(&m, e_w, &s.u, &s.states))
            .map(|d| d.eig[0])
            .unwrap_or(f64::NAN)
    };
    let local = |it: Option<&St>, text: &str| match (it, element_in(text)) {
        (Some(s), Some(e)) if e < m.mesh().n_elements() => {
            let rec = record_from(&mat, &element_of(&m, e, &s.u, &s.states), None);
            !rec.0.is_empty() && (text.contains("'internal modes'") || text.contains("'deformation gradient'"))
        }
        _ => false,
    };
    let prev: RefCell<Option<[f64; MODES]>> = RefCell::new(None);
    let mut at = |p: &APoint| -> Vec<f64> {
        let e = lowest_any(&p.info.sys.k, &id, &points, ResidualTest::Backward);
        let mu: Vec<f64> = (0..4)
            .map(|k| e.as_ref().and_then(|e| e.values.get(k).copied()).unwrap_or(f64::NAN))
            .collect();
        let sh = e.as_ref().map(|e| share(&e.vectors[0])).unwrap_or(f64::NAN);
        let shv = p.info.vt.as_ref().map(|v| share(v)).unwrap_or(f64::NAN);
        let el = element_of(&m, e_w, &p.s.u, &p.s.states);
        let d = elem_diag(&mat, &el);
        let (lmn, lmx, its, det, alpha) = match &d {
            Ok(d) => (d.eig[0], d.eig[MODES - 1], d.its as f64, d.det, Some(d.alpha)),
            Err(_) => (f64::NAN, f64::NAN, f64::NAN, f64::NAN, None),
        };
        let warm = match (alpha, *prev.borrow()) {
            (Some(a), Some(q)) => record_from(&mat, &el, Some(&q))
                .1
                .map(|w| w.iter().zip(&a).map(|(x, y)| (x - y).abs()).fold(0.0f64, f64::max) / inf9(&a))
                .unwrap_or(f64::NAN),
            _ => f64::NAN,
        };
        *prev.borrow_mut() = alpha;
        let fint = internal_force(&m, &p.s.u, &p.s.states).unwrap();
        let (r, _) = block_measures(&m, &p.s.u, &fint);
        println!(
            "TABLE FT a {:+.9e} lambda {:.12} dlambda/da {:+.6e} nu {:?} | lowest pairs {:+.6e} {:+.6e} {:+.6e} {:+.6e} | share on \
             element {e_w}: mode 1 {sh:.4e}, v_t {shv:.4e} | K_aa l_min {lmn:+.6e} l_max {lmx:+.6e} l_min/l_max {:+.4e} local \
             iterations {its} det F min {det:.6} | warm-start agreement {warm:.3e} | R/mu {r:.8}",
            p.a,
            p.s.p,
            p.sigma,
            p.info.nu,
            mu[0],
            mu[1],
            mu[2],
            mu[3],
            lmn / lmx
        );
        let mut v = mu;
        v.extend([sh, shv, lmx, its, det, warm, r]);
        v
    };
    let o = FineOpts {
        da: (s5.a - s4.a) / 8.0,
        steps: 16,
        halvings: 6,
        cap: 40,
    };
    let ft = fine_trace(&fb, "FT", s4, &phi, &o, &lmin, &local, &mut at);
    match ft.points.iter().find(|f| f.pt.rung == Some(8)) {
        Some(f) => {
            ch.at_most("FT0: |lambda(t_8) - lambda_5|", (f.pt.s.p - s5.s.p).abs(), 1e-8);
            ch.at_most(
                "FT0: mu_1(t_8) against -323.40057121476354",
                (f.extra[0] / MU_5 - 1.0).abs(),
                1e-6,
            );
        }
        None => println!("FT0: t_8 not accepted: S5 not reached by the fine trace (a reading, not a miss)"),
    }
    let cmax = ft.points[1..]
        .iter()
        .map(|f| ((f.pt.a - f.target) / f.target).abs())
        .fold(0.0f64, |w, x| if x.is_finite() { w.max(x) } else { f64::NAN });
    ch.at_most("FT1: the constraint at the accepted states, relative", cmax, 1e-9);
    ch.holds(
        "FT1: the continuity test at every accepted state",
        ft.points[1..].iter().all(|f| f.corr <= f.step),
    );
    let rd = reading(&ft);
    println!("FT READING: {rd}; {}", companions(&ft));
    ch.holds(&format!("FT2: the reading not undecided ({rd})"), rd != "undecided");
    let n = ft.points.len();
    let last = &ft.points[n - 1];
    let el_last = element_of(&m, e_w, &last.pt.s.u, &last.pt.s.states);
    if let Ok(dl) = elem_diag(&mat, &el_last) {
        println!(
            "FT: element {e_w} at the last accepted state: the eigenvalues of K_aa {:?}, {} local iterations",
            dl.eig, dl.its
        );
        for (k, f) in ft.fails.iter().enumerate().filter(|(_, f)| f.from == n - 1) {
            let tag = format!("FT failure {k} (to {:+.9e})", f.target);
            match (&f.iterate, element_in(&f.text)) {
                (Some(it), Some(e)) if e == e_w => {
                    let _ = failing_local(&tag, &mat, &m, e, it, &el_last, &dl.alpha);
                }
                _ => println!("{tag}: {} (no local diagnostics)", f.text),
            }
        }
    }
    println!("f796_nh16_end_of_branch_heavy: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}
