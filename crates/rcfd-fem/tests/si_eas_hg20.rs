//! F-841 (ADR 0723): the subcritical branch of rule L's 16^3 punch path past amplitude 1.65e-4 with warm-started local
//! modes. ADR 0711's rungs rebuilt on the plain model (ADR 0718 D1); from rung 4, ADR 0718's fine trace repeated on the model
//! with the warm start of the incompatible modes (ADR 0660 D1), each accepted state committed (every element's state replaced
//! by its trial state: the material state and the converged modes) before it is analysed, to its fold or to the registered
//! cap of 16 steps: `dlambda/da`, the four lowest pairs and element 3720's mode tangent at its committed modes along it, the
//! one-root reading and whether ADR 0711's rung 5 is reached on the same local root. The fast test runs the same fine trace,
//! commit and reading on a toy whose plain local iteration from zero fails while its root exists (PW-P, PW-W, PW-F) and the
//! root classifier (RC). The helpers are copies of those of `si_eas_hg18.rs` (existing tests are not edited). Every
//! aggregated value is required finite before it is compared.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{prescribed_equations, reduce, Prescribed};
use rcfd_fem::element::ElementState;
use rcfd_fem::material::{HyperelasticEnergy, PointContext};
use rcfd_fem::nl_incompatible::{
    condensed_force_and_tangent_warm, enhanced_force_and_tangent_with, mode_iteration_record_from, point_gradients,
    IncompatibleModeUpdate, IncompatibleStabilisation, ModeIterate,
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

/// A path with a fixed-load Newton under an external force on the free equations.
trait Branch: Case {
    /// Newton at the fixed load `p` from `start` (its prescribed values replaced by those at `p`) with the external force
    /// `eta g` on the free equations (`g` indexed by the free equations).
    fn fixed(&self, start: &St, p: f64, eta: f64, g: &[f64]) -> Result<St, String>;
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
    free: Vec<usize>,
}

impl<'a> FemBranch<'a> {
    fn new(m: &'a SolidModel, m2: &'a SolidModel) -> FemBranch<'a> {
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
        state: material_part(&states[e]),
    }
}

/// ADR 0723 D1: the material part of an element state (the neo-Hookean material has no state: a warm state of nine values,
/// the committed modes, has an empty material part).
fn material_part(st: &ElementState) -> ElementState {
    if st.values().len() == MODES {
        ElementState::new(Vec::new())
    } else {
        st.clone()
    }
}

/// ADR 0723 D1: the committed modes of a warm element state, if present.
fn committed_modes(st: &ElementState) -> Option<[f64; MODES]> {
    let v = st.values();
    (v.len() == MODES).then(|| std::array::from_fn(|k| v[k]))
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
/// ADR 0723 D2: the commit of an accepted state (the identity for the plain trace).
type CommitOf<'a> = &'a dyn Fn(St) -> St;

/// ADR 0718 D4 (ADR 0723 D2: with the commit): the fine trace from `from` over the targets `a_0 + j da`, `j = 1..=steps`; a failure halved toward its target,
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
    commit: CommitOf,
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
                    // ADR 0723 D2: the accepted state committed before it is analysed.
                    let s = commit(s);
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
    // ADR 0723 D4: the cap reached without a sign change of sigma or of l_min.
    if t.stop == "steps" {
        return "no fold up to the cap".to_string();
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
/// The element named by an element error (`element <e>: ...`).
fn element_in(text: &str) -> Option<usize> {
    text.strip_prefix("element ")?.split(':').next()?.parse().ok()
}

// ============================================================================ ADR 0723 D2: the lowest pairs with a nearby bracket

/// ADR 0723 D2: the four lowest pairs of `(K, M)` with the shift bracketed near the previous lowest value `mu_p`: `K + s M`
/// with zero inertia at `s = 1.1 |mu_p|` and non-zero inertia at `0.9 |mu_p|`, 8 bisections, the shift `1.05 hi`, the
/// options of `lowest_any`; `lowest_any` itself if either end fails or there is no negative `mu_p`; `lowest` with `nu = 0`.
fn lowest_near(
    k: &CsrMatrix,
    mm: &CsrMatrix,
    points: &[[f64; 3]],
    test: ResidualTest,
    mu_p: Option<f64>,
) -> Option<GeneralizedEigen> {
    let nu = inertia(k);
    if nu == Some(0) {
        return lowest(k, mm, points, test);
    }
    let Some(m) = mu_p.filter(|m| m.is_finite() && *m < 0.0) else {
        return lowest_any(k, mm, points, test);
    };
    let zero_at = |s: f64| inertia(&combine(k, mm, s)) == Some(0);
    let (mut lo, mut hi) = (0.9 * m.abs(), 1.1 * m.abs());
    if !(zero_at(hi) && !zero_at(lo)) {
        println!("lowest_near: no bracket in [{lo:e}, {hi:e}]: lowest_any");
        return lowest_any(k, mm, points, test);
    }
    for _ in 0..8 {
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
                "lowest pairs near (nu {nu:?}, bracket ({lo:e}, {hi:e}]): values {:?}, residuals {:?}, iterations {}, shift {:e}, \
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
            println!("lowest pairs near (nu {nu:?}) failed: {err:?}");
            None
        }
    }
}

// ============================================================================ ADR 0723 D3: the same local root

/// `|a - b|_inf / |a|_inf`.
fn rel_dist(a: &[f64], b: &[f64]) -> f64 {
    max_abs(a.iter().zip(b).map(|(x, y)| x - y)) / max_abs(a.iter().copied())
}

/// ADR 0723 D3: the root classifier on `d` (the relative distance of the modes) and `dl` (the load difference).
fn root_reading(d: f64, dl: f64) -> &'static str {
    if !(d.is_finite() && dl.is_finite()) {
        "undecided"
    } else if d <= 1e-6 && dl <= 1e-8 {
        "same root"
    } else if d >= 1e-3 || dl >= 1e-6 {
        "another root"
    } else {
        "undecided"
    }
}

// ============================================================================ ADR 0723 D5: the toy PW

const PW_C: f64 = 2.84;
const PW_D: f64 = 0.01;
const PW_B3: f64 = 0.5;
const PW_B5: f64 = 0.25;

/// PW (ADR 0723 D5): PT of ADR 0711 (`b3 = 0.5`, `b5 = 0.25`, `sb = 1`) with the local unknown `z` of `W_l = Phi(x) + (d/2)
/// u_0^2 z`, `x = z - c u_0^2`, `Phi(x) = x atan x - ln(1 + x^2) / 2`: `g = atan(x) + d u_0^2 / 2 = 0` solved per evaluation by
/// Newton, from the committed `z` (the state's one value) with the warm start, then from 0 (ADR 0660 D1's rule).
struct Pw {
    warm: bool,
    /// The fallbacks counted by the commits.
    fallbacks: RefCell<usize>,
}

impl Pw {
    fn new(warm: bool) -> Pw {
        Pw {
            warm,
            fallbacks: RefCell::new(0),
        }
    }
    /// The local Newton at `a` from `start` (25 iterations, `|g| <= 1e-14`).
    fn local(&self, a: f64, start: f64) -> Result<f64, String> {
        let mut z = start;
        for it in 0..=25 {
            let x = z - PW_C * a * a;
            let g = x.atan() + PW_D * a * a / 2.0;
            if !(g.is_finite() && z.is_finite()) {
                return Err(format!("local: not finite at iteration {it}"));
            }
            if g.abs() <= 1e-14 {
                return Ok(z);
            }
            if it == 25 {
                break;
            }
            z -= g * (1.0 + x * x);
        }
        Err("local: not converged in 25 iterations".to_string())
    }
    /// The start rule of ADR 0660 D1: from the committed `z` with the warm start, from 0 if there is none or that fails; the
    /// root and whether it fell back.
    fn solve_local(&self, a: f64, committed: Option<f64>) -> (Result<f64, String>, bool) {
        match committed.filter(|_| self.warm) {
            Some(z0) => match self.local(a, z0) {
                Ok(z) => (Ok(z), false),
                Err(_) => (self.local(a, 0.0), true),
            },
            None => (self.local(a, 0.0), false),
        }
    }
    fn committed(s: &St) -> Option<f64> {
        s.states.first().map(|st| st.values()[0])
    }
    fn x_of(a: f64) -> f64 {
        -(PW_D * a * a / 2.0).tan()
    }
    fn z_of(a: f64) -> f64 {
        PW_C * a * a + Pw::x_of(a)
    }
    fn p_of(a: f64) -> f64 {
        (1.0 + (PW_B3 - 1.0) * a * a + PW_B5 * a.powi(4) + 2.0 * PW_C * PW_D * a * a
            - PW_D * (PW_D * a * a / 2.0).tan())
            / 0.9
    }
    fn dp_da(a: f64) -> f64 {
        let e = PW_D * a * a / 2.0;
        (2.0 * (PW_B3 - 1.0) * a + 4.0 * PW_B5 * a.powi(3) + 4.0 * PW_C * PW_D * a
            - PW_D * PW_D * a / (e.cos() * e.cos()))
            / 0.9
    }
    fn eval(&self, u: &[f64], p: f64, committed: Option<f64>) -> Result<(Vec<f64>, f64), String> {
        let z = self.solve_local(u[0], committed).0?;
        let x = z - PW_C * u[0] * u[0];
        let r = (0..TOY_N)
            .map(|i| match i {
                0 => {
                    (1.0 - p) * u[0] + PW_B3 * u[0].powi(3) + PW_B5 * u[0].powi(5) + 2.0 * PT_C * u[0] * u[1]
                        - 2.0 * PW_C * u[0] * x.atan()
                        + PW_D * u[0] * z
                }
                1 => 2.0 * u[1] - 0.1 * p + PT_C * u[0] * u[0],
                _ => k_i(i) * u[i] - p * b_i(i),
            })
            .collect();
        Ok((r, z))
    }
    fn dense(&self, u: &[f64], p: f64, z: f64) -> Vec<f64> {
        let mut a = vec![0.0; TOY_N * TOY_N];
        let x = z - PW_C * u[0] * u[0];
        let w = 1.0 / (1.0 + x * x);
        let wzz = w;
        let wuz = -2.0 * PW_C * u[0] * w + PW_D * u[0];
        let wuu = -2.0 * PW_C * x.atan() + 4.0 * PW_C * PW_C * u[0] * u[0] * w + PW_D * z;
        a[0] = 1.0 - p + 3.0 * PW_B3 * u[0] * u[0] + 5.0 * PW_B5 * u[0].powi(4) + 2.0 * PT_C * u[1] + wuu
            - wuz * wuz / wzz;
        a[1] = 2.0 * PT_C * u[0];
        a[TOY_N] = 2.0 * PT_C * u[0];
        a[TOY_N + 1] = 2.0;
        for i in 2..TOY_N {
            a[TOY_N * i + i] = k_i(i);
        }
        a
    }
    /// The closed-form branch state at the amplitude `a` (not polished, no committed `z`).
    fn guess(&self, a: f64) -> St {
        let p = Pw::p_of(a);
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
    /// ADR 0723 D2: the commit (the warm toy only): the root by the start rule stored in the state; a fallback counted.
    fn commit(&self, s: St) -> St {
        if !self.warm {
            return s;
        }
        let (z, fell) = self.solve_local(s.u[0], Pw::committed(&s));
        if fell {
            *self.fallbacks.borrow_mut() += 1;
        }
        St {
            states: vec![ElementState::new(vec![z.unwrap_or(f64::NAN)])],
            ..s
        }
    }
    /// `g_z = 1 / (1 + x^2)` at the root of a state (the committed `z`, or the plain iteration from 0).
    fn g_z(&self, s: &St) -> f64 {
        let z = match Pw::committed(s) {
            Some(z) => Ok(z),
            None => self.local(s.u[0], 0.0),
        };
        z.map(|z| {
            let x = z - PW_C * s.u[0] * s.u[0];
            1.0 / (1.0 + x * x)
        })
        .unwrap_or(f64::NAN)
    }
}

impl Case for Pw {
    fn solve(&self, _from: &St, _to: f64) -> Result<St, String> {
        Err("not used".to_string())
    }
    fn system(&self, s: &St) -> Sys {
        let z = self.solve_local(s.u[0], Pw::committed(s)).0.unwrap_or(f64::NAN);
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

impl Branch for Pw {
    fn fixed(&self, _start: &St, _p: f64, _eta: f64, _g: &[f64]) -> Result<St, String> {
        Err("not used".to_string())
    }
    fn free(&self) -> Vec<usize> {
        (0..TOY_N).collect()
    }
}

impl Bordered for Pw {
    fn residual(&self, s: &St) -> Result<(Vec<f64>, f64), String> {
        let (r, _) = self.eval(&s.u, s.p, Pw::committed(s))?;
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
            states: s.states.clone(),
        }
    }
}

/// The toy's fine trace from the closed-form state at `a0` polished by the bordered Newton and committed.
fn pw_trace(pw: &Pw, tag: &str, a0: f64, o: &FineOpts, ch: &mut Checks) -> FTrace {
    let phi: Vec<f64> = (0..TOY_N).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();
    let ctl = Control {
        g: phi.clone(),
        h: 0.0,
        c: a0,
    };
    let s0 = bordered(pw, &pw.guess(a0), &ctl)
        .map(|x| x.0)
        .unwrap_or_else(|_| pw.guess(f64::NAN));
    let s0 = pw.commit(s0);
    ch.at_most(
        &format!("{tag}-0: |p - p(a0)| at the start"),
        (s0.p - Pw::p_of(a0)).abs(),
        1e-12,
    );
    let info = analyse(pw, &s0);
    let start = apoint(pw, s0, info, &phi, Some(0));
    let lmin = |s: &St| pw.g_z(s);
    let local = |_: Option<&St>, text: &str| text.starts_with("local");
    let commit = |s: St| pw.commit(s);
    let mut at = |p: &APoint| {
        vec![
            dense_lowest(&p.info.sys.k).map(|x| x.mu).unwrap_or(f64::NAN),
            Pw::committed(&p.s).unwrap_or(f64::NAN),
        ]
    };
    fine_trace(pw, tag, start, &phi, o, &lmin, &local, &commit, &mut at)
}

/// The closed-form checks at every accepted state of a toy trace (PP-1, PW-1).
fn pw_states(tag: &str, t: &FTrace, warm: bool, ch: &mut Checks) {
    let up = |w: f64, x: f64| if x.is_finite() { w.max(x) } else { f64::NAN };
    let (mut ep, mut es, mut eg, mut ez) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for f in &t.points {
        let a = f.pt.a;
        let x = Pw::x_of(a);
        ep = up(ep, (f.pt.s.p - Pw::p_of(a)).abs());
        es = up(es, ((f.pt.sigma - Pw::dp_da(a)) / Pw::dp_da(a)).abs());
        eg = up(eg, (f.lmin - 1.0 / (1.0 + x * x)).abs());
        if warm {
            ez = up(ez, (f.extra[1] - Pw::z_of(a)).abs());
        }
        println!(
            "{tag} TABLE a {a:.12}: p {:.12} (closed form {:.12}), dp/da {:+.9e} (closed form {:+.9e}), g_z {:.9e}, rho {:.6}, \
             nu {:?}, mu_1 {:+.6e}, z committed {:.12} (closed form {:.12})",
            f.pt.s.p,
            Pw::p_of(a),
            f.pt.sigma,
            Pw::dp_da(a),
            f.lmin,
            f.lmin / t.points[0].lmin,
            f.pt.info.nu,
            f.extra[0],
            f.extra[1],
            Pw::z_of(a)
        );
    }
    ch.at_most(
        &format!("{tag}-1: max |p - p(a)| over {} states", t.points.len()),
        ep,
        1e-12,
    );
    ch.at_most(&format!("{tag}-1: max |sigma - dp/da| / |dp/da|"), es, 1e-8);
    ch.at_most(&format!("{tag}-1: max |g_z - 1 / (1 + x*^2)|"), eg, 1e-12);
    if warm {
        ch.at_most(&format!("{tag}-1: max |z_committed - z*(a)|"), ez, 1e-12);
    }
}

// ============================================================================ the fast test

#[test]
fn f841_fast() {
    let mut ch = Checks::new();
    let t_all = Instant::now();
    let plain = Pw::new(false);
    let warm = Pw::new(true);

    // a_c (the plain local Newton's threshold) and a_f (the fold) by bisection.
    ch.holds(
        "PW: the plain local Newton from 0 converges at 0.5 and fails at 1.5",
        plain.local(0.5, 0.0).is_ok() && plain.local(1.5, 0.0).is_err(),
    );
    let (mut lo, mut hi) = (0.5, 1.5);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if plain.local(mid, 0.0).is_ok() {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let a_c = lo;
    let (mut lo, mut hi) = (0.5, 1.5);
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if Pw::dp_da(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let a_f = lo;
    println!(
        "PW: a_c {a_c:.10} (the plain local Newton from 0 fails above), a_f {a_f:.10} (the fold, p_f {:.10})",
        Pw::p_of(a_f)
    );
    let o = FineOpts {
        da: 0.05,
        steps: 16,
        halvings: 6,
        cap: 60,
    };

    // PW-P: the plain trace.
    let tp = pw_trace(&plain, "PP", 0.5, &o, &mut ch);
    pw_states("PP", &tp, false, &mut ch);
    let n = tp.points.len();
    let a_last = tp.points[n - 1].pt.a;
    let tf = t_fail_min(&tp);
    let rd = reading(&tp);
    println!("PP READING: {rd}; {}", companions(&tp));
    ch.holds(&format!("PP-2: the stop lost ({})", tp.stop), tp.stop == "lost");
    ch.holds(
        &format!("PP-2: a_last {a_last:.10} < a_c {a_c:.10} < t_fail,min {tf:.10}"),
        a_last < a_c && a_c < tf,
    );
    ch.at_most("PP-2: t_fail,min - a_last", tf - a_last, 0.05 / 32.0);
    ch.holds(
        &format!("PP-3: the reading local breakdown: the local iteration ({rd})"),
        rd == "local breakdown: the local iteration",
    );
    ch.holds(
        &format!("PP-3: every failed attempt local ({} failures)", tp.fails.len()),
        !tp.fails.is_empty() && tp.fails.iter().all(|f| f.local),
    );
    let (mut gz_min, mut ez) = (f64::INFINITY, 0.0f64);
    for f in &tp.fails {
        let x = Pw::x_of(f.target);
        gz_min = gz_min.min(1.0 / (1.0 + x * x));
        let from = &tp.points[f.from].pt;
        let e = plain
            .local(from.a, 0.0)
            .and_then(|z0| plain.local(f.target, z0))
            .map(|z| (z - Pw::z_of(f.target)).abs())
            .unwrap_or(f64::NAN);
        ez = if e.is_finite() { ez.max(e) } else { f64::NAN };
    }
    ch.holds(
        &format!("PP-3: the root exists at every failed target (smallest g_z {gz_min:.6})"),
        gz_min >= 0.99,
    );
    ch.at_most(
        "PP-3: the local Newton from the last accepted z at every failed target, |z - z*|",
        ez,
        1e-12,
    );

    // PW-W: the warm trace with the commit.
    let tw = pw_trace(&warm, "PW", 0.5, &o, &mut ch);
    pw_states("PW", &tw, true, &mut ch);
    let rd = reading(&tw);
    println!("PW READING: {rd}; {}", companions(&tw));
    ch.equals("PW-2: failed attempts", tw.fails.len(), 0);
    ch.equals("PW-2: fallbacks in the commits", *warm.fallbacks.borrow(), 0);
    ch.holds(&format!("PW-2: the stop fold + 2 ({})", tw.stop), tw.stop == "fold + 2");
    let wl = tw.points[tw.points.len() - 1].pt.a;
    ch.holds(&format!("PW-2: the last a {wl:.6} > a_c"), wl > a_c);
    ch.holds(&format!("PW-3: the reading fold ({rd})"), rd == "fold");
    match tw.sign {
        Some(j) => {
            let (p0, p1) = (&tw.points[j - 1], &tw.points[j]);
            ch.holds(
                &format!("PW-3: a_f in the bracket [{:.6}, {:.6}]", p0.pt.a, p1.pt.a),
                p0.pt.a <= a_f && a_f <= p1.pt.a,
            );
            ch.holds(
                &format!("PW-3: nu {:?} then {:?}", p0.pt.info.nu, p1.pt.info.nu),
                p0.pt.info.nu == Some(1) && p1.pt.info.nu == Some(0),
            );
            ch.holds(
                &format!("PW-3: rho {:.4} and {:.4} >= 0.1", rho_of(&tw, j - 1), rho_of(&tw, j)),
                rho_of(&tw, j - 1) >= RHO_REG && rho_of(&tw, j) >= RHO_REG,
            );
        }
        None => ch.holds("PW-3: a sign change of sigma", false),
    }

    // PW-F: the fallback.
    let (z, fell) = warm.solve_local(0.5, Some(10.0));
    ch.holds("PW-F: the fallback flagged", fell);
    ch.at_most(
        "PW-F: |z - z*(0.5)|",
        z.map(|z| (z - Pw::z_of(0.5)).abs()).unwrap_or(f64::NAN),
        1e-12,
    );

    // RC: the root classifier on the two roots of PL's quadratic local problem at a = 1 (c_l = 1/16).
    let zm = (1.0 - 0.75f64.sqrt()) / 2.0;
    let zp = 1.0 - zm;
    let gq = |z: f64| z - z * z - 1.0 / 16.0;
    println!("RC: z_- {zm:.15} (g {:e}), z_+ {zp:.15} (g {:e})", gq(zm), gq(zp));
    let r1 = root_reading(rel_dist(&[zm], &[zm]), 0.0);
    let r2 = root_reading(rel_dist(&[zm], &[zp]), 0.0);
    let r3 = root_reading(rel_dist(&[zm], &[zm * (1.0 + 1e-4)]), 0.0);
    ch.holds(&format!("RC: (z_-, z_-) same root ({r1})"), r1 == "same root");
    ch.holds(&format!("RC: (z_-, z_+) another root ({r2})"), r2 == "another root");
    ch.holds(
        &format!("RC: (z_-, z_- (1 + 1e-4)) undecided ({r3})"),
        r3 == "undecided",
    );
    println!("f841_fast: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}

// ============================================================================ ADR 0723 D1: the commit on the block

/// The element watched (ADR 0718 X1).
const E_W: usize = 3720;
/// ADR 0718's fine trace at `t_1` (`f796-scratch/run3-heavy.log` line 195).
const LAMBDA_T1: f64 = 0.959749331159;
const SIGMA_T1: f64 = -3.909602e1;
const MU_T1: f64 = -2.365429e4;

/// A commit: the committed state, the elements that fell back, whether the watched one did, the elements whose commit
/// failed, and CW (the watched element's committed state equal to the driver's warm trial state, bit for bit).
struct Commit {
    s: St,
    fallbacks: usize,
    watched: bool,
    failed: usize,
    cw: bool,
}

/// ADR 0723 D1: the commit of `s` on the warm model `m`: every element's local iteration from its committed modes, from `alpha
/// = 0` if there are none or that fails (the start rule of ADR 0660 D1, by the same `converged`), the new state the material
/// state followed by the converged modes (four threads; the result does not depend on them).
fn commit_fem(m: &SolidModel, mat: &Material, s: &St) -> Commit {
    let ne = m.mesh().n_elements();
    let one = |e: usize| -> (Result<[f64; MODES], String>, bool) {
        let el = element_of(m, e, &s.u, &s.states);
        match committed_modes(&s.states[e]) {
            Some(a) => match record_from(mat, &el, Some(&a)).1 {
                Ok(x) => (Ok(x), false),
                Err(_) => (record_from(mat, &el, None).1, true),
            },
            None => (record_from(mat, &el, None).1, false),
        }
    };
    let one = &one;
    let per = ne.div_ceil(4);
    let results: Vec<(Result<[f64; MODES], String>, bool)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..4)
            .map(|c| sc.spawn(move || (c * per..((c + 1) * per).min(ne)).map(one).collect::<Vec<_>>()))
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let mut out = Commit {
        s: s.clone(),
        fallbacks: 0,
        watched: false,
        failed: 0,
        cw: false,
    };
    for (e, (r, fell)) in results.into_iter().enumerate() {
        if fell {
            out.fallbacks += 1;
        }
        if e == E_W {
            out.watched = fell;
        }
        match r {
            Ok(alpha) => {
                let mut v = material_part(&s.states[e]).values().to_vec();
                v.extend_from_slice(&alpha);
                out.s.states[e] = ElementState::new(v);
            }
            Err(t) => {
                out.failed += 1;
                println!("commit: element {e} failed: {t}");
            }
        }
    }
    let el = element_of(m, E_W, &s.u, &s.states);
    out.cw = condensed_force_and_tangent_warm(
        &el.coords,
        mat,
        &el.ue,
        &s.states[E_W],
        &PointContext::default(),
        LIN,
        IncompatibleModeUpdate::Plain,
    )
    .map(|w| {
        let c = out.s.states[E_W].values();
        w.3.len() == c.len() && w.3.iter().zip(c).all(|(x, y)| x.to_bits() == y.to_bits())
    })
    .unwrap_or(false);
    out
}

/// The indices of the diagnostics of an accepted state of the warm trace (`FPoint::extra`).
const X_DELTA: usize = 7;
const X_WFB: usize = 8;
const X_NFB: usize = 9;

#[test]
#[ignore = "ADR 0723: the heavy record of the warm trace on 16^3 (D1 to D4), run once with --ignored"]
fn f841_nh16_warm_trace_heavy() {
    let mut ch = Checks::new();
    let t_all = Instant::now();
    let mesh16 = block_mesh(16);
    let m = block_model(&mesh16, K_L);
    let m2 = block_model_free_punch(&mesh16, K_L);
    let states = initial_states(&m).unwrap();
    let fb = FemBranch::new(&m, &m2);
    let free = fb.free.clone();
    let mat = neo_hookean();

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

    // D1: the symmetric chain, the mode, the switch and the step to 0.9645 (ADR 0711 R0, R1), on the plain model.
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

    // D1: ADR 0711's rungs 1 to 5 (plain).
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
    let Ok(d5) = elem_diag(&mat, &element_of(&m, E_W, &s5.s.u, &s5.s.states)) else {
        ch.holds("S5: element 3720's local problem at S5 converges", false);
        ch.finish();
        return;
    };
    println!(
        "S5: lambda {:.15}, a {:+.12e}, sigma {:+.9e}; element {E_W}: alpha_5 {:?}, K_aa l_min {:+.6e}, asymmetry {:.2e}, {}          local iterations, det F min {:.6}, {:.1} s",
        s5.s.p,
        s5.a,
        s5.sigma,
        d5.alpha,
        d5.eig[0],
        d5.asym,
        d5.its,
        d5.det,
        t_all.elapsed().as_secs_f64()
    );

    // D1: the warm model and the commit of S4.
    let mut mw = block_model(&mesh16, K_L);
    mw.set_incompatible_mode_warm_start(true);
    let mut m2w = block_model_free_punch(&mesh16, K_L);
    m2w.set_incompatible_mode_warm_start(true);
    let fbw = FemBranch::new(&mw, &m2w);
    let c4 = commit_fem(&mw, &mat, &s4.s);
    println!(
        "D1: S4 committed on the warm model: {} fallbacks, {} failed, CW {}, {:.1} s",
        c4.fallbacks,
        c4.failed,
        c4.cw,
        t_all.elapsed().as_secs_f64()
    );
    ch.holds("CW: element 3720's committed state at S4", c4.cw && c4.failed == 0);
    let rp = fb.residual(&s4.s);
    let rw = fbw.residual(&c4.s);
    let diff = match (&rp, &rw) {
        (Ok((a, _)), Ok((b, _))) if a.len() == b.len() => {
            a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count()
        }
        _ => usize::MAX,
    };
    ch.equals("W0: the free residual at S4_W against S4, entries that differ", diff, 0);
    let i4w = analyse(&fbw, &c4.s);
    let s4w = apoint(&fbw, c4.s, i4w, &phi, Some(0));
    ch.holds(
        &format!(
            "W0: sigma at S4_W {:+.15e} bit-identical to S4's {:+.15e}",
            s4w.sigma, s4.sigma
        ),
        s4w.sigma.to_bits() == s4.sigma.to_bits(),
    );
    ch.holds(
        &format!("W0: nu at S4_W {:?} equal to S4's {:?}", s4w.info.nu, s4.info.nu),
        s4w.info.nu == s4.info.nu && s4w.info.nu.is_some(),
    );

    // D2: the warm fine trace from S4_W with the commit.
    let dofs = &m.dof_map();
    let pos: HashMap<usize, usize> = free.iter().enumerate().map(|(k, &q)| (q, k)).collect();
    let e_idx: Vec<usize> = m
        .mesh()
        .element_nodes(E_W)
        .iter()
        .flat_map(|&n| (0..3).map(move |c| (n, c)))
        .filter_map(|(n, c)| pos.get(&dofs.equation(n, c).unwrap()).copied())
        .collect();
    let share = |v: &[f64]| e_idx.iter().map(|&k| v[k] * v[k]).sum::<f64>() / v.iter().map(|x| x * x).sum::<f64>();
    let kaa = |s: &St| -> Option<(M9, [f64; MODES])> {
        let alpha = committed_modes(&s.states[E_W])?;
        let el = element_of(&mw, E_W, &s.u, &s.states);
        mode_block(&mat, &el, &alpha).ok().map(|(_, k)| (k, alpha))
    };
    let lmin = |s: &St| kaa(s).map(|(k, _)| spectrum(&k).1[0]).unwrap_or(f64::NAN);
    let local = |it: Option<&St>, text: &str| match (it, element_in(text)) {
        (Some(s), Some(e)) if e < mw.mesh().n_elements() => {
            let rec = record_from(&mat, &element_of(&mw, e, &s.u, &s.states), None);
            !rec.0.is_empty() && (text.contains("'internal modes'") || text.contains("'deformation gradient'"))
        }
        _ => false,
    };
    let records: RefCell<Vec<(usize, bool, usize, bool)>> = RefCell::new(Vec::new());
    let commit = |s: St| -> St {
        let t0 = Instant::now();
        let c = commit_fem(&mw, &mat, &s);
        println!(
            "commit at a {:+.9e}: {} fallbacks (element {E_W}: {}), {} failed, CW {}, {:.1} s",
            amp(&phi, &free, &c.s.u),
            c.fallbacks,
            c.watched,
            c.failed,
            c.cw,
            t0.elapsed().as_secs_f64()
        );
        records.borrow_mut().push((c.fallbacks, c.watched, c.failed, c.cw));
        c.s
    };
    let prev_mu: RefCell<Option<f64>> = RefCell::new(None);
    let prev_alpha: RefCell<Option<[f64; MODES]>> = RefCell::new(None);
    let mut at = |p: &APoint| -> Vec<f64> {
        let e = lowest_near(&p.info.sys.k, &id, &points, ResidualTest::Backward, *prev_mu.borrow());
        let mu: Vec<f64> = (0..4)
            .map(|k| e.as_ref().and_then(|e| e.values.get(k).copied()).unwrap_or(f64::NAN))
            .collect();
        *prev_mu.borrow_mut() = Some(mu[0]);
        let sh = e.as_ref().map(|e| share(&e.vectors[0])).unwrap_or(f64::NAN);
        let (lmn, lmx, asym, alpha) = match kaa(&p.s) {
            Some((k, a)) => {
                let (asym, eig) = spectrum(&k);
                (eig[0], eig[MODES - 1], asym, Some(a))
            }
            None => (f64::NAN, f64::NAN, f64::NAN, None),
        };
        let delta = match (alpha, *prev_alpha.borrow()) {
            (Some(a), Some(q)) => rel_dist(&a, &q),
            _ => f64::NAN,
        };
        *prev_alpha.borrow_mut() = alpha;
        let (nfb, wfb) = records
            .borrow()
            .last()
            .map(|r| (r.0 as f64, if r.1 { 1.0 } else { 0.0 }))
            .unwrap_or((f64::NAN, f64::NAN));
        let el = element_of(&mw, E_W, &p.s.u, &p.s.states);
        let (cold, cold_text) = match (record_from(&mat, &el, None).1, alpha) {
            (Ok(a0), Some(a)) => (rel_dist(&a, &a0), "converged".to_string()),
            (Err(t), _) => (f64::NAN, t),
            _ => (f64::NAN, "no committed modes".to_string()),
        };
        let fint = internal_force(&mw, &p.s.u, &p.s.states).unwrap();
        let (r, _) = block_measures(&mw, &p.s.u, &fint);
        println!(
            "TABLE FW a {:+.9e} lambda {:.12} dlambda/da {:+.6e} nu {:?} | lowest pairs {:+.6e} {:+.6e} {:+.6e} {:+.6e} | share on \
             element {E_W}: mode 1 {sh:.4e} | K_aa l_min {lmn:+.6e} l_max {lmx:+.6e} l_min/l_max {:+.4e} asymmetry {asym:.2e} | \
             delta {delta:.4e} | fallbacks {nfb} (element {E_W}: {wfb}) | the plain iteration from 0: {cold_text}, distance \
             {cold:.4e} | R/mu {r:.8}",
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
        v.extend([sh, lmx, asym, delta, wfb, nfb, cold, r]);
        v
    };
    let o = FineOpts {
        da: (s5.a - s4.a) / 8.0,
        steps: 16,
        halvings: 6,
        cap: 40,
    };
    let ft = fine_trace(&fbw, "FW", s4w, &phi, &o, &lmin, &local, &commit, &mut at);
    println!("FW: the trace done, {:.1} s", t_all.elapsed().as_secs_f64());

    // LP0: the nearby bracket against lowest_any at S4_W.
    let mu_any = ft.points[0].extra[0];
    let near = lowest_near(
        &ft.points[0].pt.info.sys.k,
        &id,
        &points,
        ResidualTest::Backward,
        Some(mu_any),
    );
    let mu_near = near.as_ref().map(|e| e.values[0]).unwrap_or(f64::NAN);
    ch.at_most(
        &format!("LP0: mu_1 of the nearby bracket {mu_near:+.12e} against lowest_any's {mu_any:+.12e}, relative"),
        (mu_near / mu_any - 1.0).abs(),
        1e-7,
    );

    // CW, FW1, W1.
    let recs = records.borrow();
    ch.holds(
        &format!(
            "CW: element 3720's committed state at every accepted state ({} commits)",
            recs.len()
        ),
        !recs.is_empty() && recs.iter().all(|r| r.3 && r.2 == 0),
    );
    let cmax = ft.points[1..]
        .iter()
        .map(|f| ((f.pt.a - f.target) / f.target).abs())
        .fold(0.0f64, |w, x| if x.is_finite() { w.max(x) } else { f64::NAN });
    ch.at_most("FW1: the constraint at the accepted states, relative", cmax, 1e-9);
    ch.holds(
        "FW1: the continuity test at every accepted state",
        ft.points[1..].iter().all(|f| f.corr <= f.step),
    );
    match ft.points.iter().find(|f| f.pt.rung == Some(1)) {
        Some(f) => {
            ch.at_most("W1: |lambda(t_1) - 0.959749331159|", (f.pt.s.p - LAMBDA_T1).abs(), 1e-9);
            ch.at_most(
                "W1: sigma(t_1) against -3.909602e1",
                (f.pt.sigma / SIGMA_T1 - 1.0).abs(),
                1e-6,
            );
            ch.at_most(
                "W1: mu_1(t_1) against -2.365429e4",
                (f.extra[0] / MU_T1 - 1.0).abs(),
                1e-6,
            );
        }
        None => println!("W1: t_1 not accepted (not compared)"),
    }

    // FW2: the reading; RT: one root; RS: rung 5.
    let rd = reading(&ft);
    println!("FW READING: {rd}; {}", companions(&ft));
    ch.holds(&format!("FW2: the reading not undecided ({rd})"), rd != "undecided");
    // D3: no fallback of the watched element and delta_j <= 0.1 (a NaN delta is not within).
    let within = |x: f64| x <= 0.1;
    let off = ft.points[1..]
        .iter()
        .find(|f| f.extra[X_WFB] != 0.0 || !within(f.extra[X_DELTA]));
    let rt = match off {
        None => format!(
            "one local root of element {E_W} along the trace ({} accepted states, largest delta {:.4e}, fallbacks of all elements \
             {})",
            ft.points.len(),
            ft.points[1..].iter().map(|f| f.extra[X_DELTA]).fold(0.0f64, f64::max),
            ft.points[1..].iter().map(|f| f.extra[X_NFB]).sum::<f64>()
        ),
        Some(f) => format!(
            "the root left at a {:+.9e} (fallback {}, delta {:.4e})",
            f.pt.a, f.extra[X_WFB], f.extra[X_DELTA]
        ),
    };
    println!("RT READING: {rt}");
    let rs = match ft.points.iter().find(|f| f.pt.rung == Some(8)) {
        Some(f) => {
            let aw = committed_modes(&f.pt.s.states[E_W])
                .map(|a| a.to_vec())
                .unwrap_or(vec![f64::NAN; MODES]);
            let d = rel_dist(&aw, &d5.alpha);
            let dl = (f.pt.s.p - s5.s.p).abs();
            let r = root_reading(d, dl);
            println!(
                "RS: at t_8 a {:+.12e} (a_5 {:+.12e}): lambda {:.12} (lambda_5 {:.12}), d {d:.4e}, dl {dl:.4e}, mu_1 {:+.6e} (S5 \
                 {MU_5:+.6e}), sigma {:+.6e} (S5 {:+.6e}): {r}",
                f.pt.a, s5.a, f.pt.s.p, s5.s.p, f.extra[0], f.pt.sigma, s5.sigma
            );
            r.to_string()
        }
        None => format!(
            "rung 5 not reached (stop {}, last a {:+.9e})",
            ft.stop,
            ft.points[ft.points.len() - 1].pt.a
        ),
    };
    println!("RS READING: {rs}");
    ch.holds(
        &format!("RS: the rung-5 reading not undecided ({rs})"),
        rs != "undecided",
    );

    // The failed attempts after the last accepted state: the watched element from its committed modes and from 0.
    let n = ft.points.len();
    let last = &ft.points[n - 1].pt;
    for (k, f) in ft.fails.iter().enumerate() {
        println!(
            "FW failure {k} (from state {}, to {:+.9e}, local {}): {}",
            f.from, f.target, f.local, f.text
        );
        if f.from != n - 1 {
            continue;
        }
        if let (Some(it), Some(e)) = (&f.iterate, element_in(&f.text)) {
            let el = element_of(&mw, e, &it.u, &it.states);
            let warm_out = committed_modes(&last.s.states[e]).map(|a| record_from(&mat, &el, Some(&a)));
            let cold_out = record_from(&mat, &el, None);
            println!(
                "FW failure {k}: element {e} from its committed modes: {}; from 0: {} iterates, {}",
                match &warm_out {
                    Some((rec, Ok(_))) => format!("{} iterates, converged", rec.len()),
                    Some((rec, Err(t))) => format!("{} iterates, {t}", rec.len()),
                    None => "no committed modes".to_string(),
                },
                cold_out.0.len(),
                match &cold_out.1 {
                    Ok(_) => "converged".to_string(),
                    Err(t) => t.clone(),
                }
            );
        }
    }
    println!("f841_nh16_warm_trace_heavy: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}
