//! F-843 (ADR 0724): the fold of rule L's 16^3 punch branch by an extended system with warm-started local modes. ADR 0711's
//! rungs rebuilt on the plain model (ADR 0723 D1); rung 5 committed on the model with the warm start of the incompatible modes
//! (ADR 0660 D1), the lower end of ADR 0723's fold bracket; its upper end reproduced by one step; from the lower end a
//! test-only Moore-Spence system (`R_F = 0`, `K psi = 0`, `l . psi = 1`) solved by Newton with block elimination and central
//! differences of the tangent, the result committed: `lambda_f`, `a_f`, the critical vector's share on element 3720, the null
//! vector's residual. The fast test solves the same system on ADR 0723's toy PW (a closed-form fold with a local unknown)
//! from both ends of its bracket, with a negative control on the toy without the warm start. The helpers are copies of those
//! of `si_eas_hg20.rs` (existing tests are not edited). Every aggregated value is required finite before it is compared.

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
#[allow(dead_code)] // copied from si_eas_hg20.rs; the fields are read there
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
#[allow(dead_code)] // copied from si_eas_hg20.rs; the fields are read there
struct FPoint {
    pt: APoint,
    target: f64,
    lmin: f64,
    corr: f64,
    step: f64,
    extra: Vec<f64>,
}

/// A failed attempt: its target, its text, whether it is local, the failing iterate, the index of the state it started from.
#[allow(dead_code)] // copied from si_eas_hg20.rs; the fields are read there
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

// ============================================================================ ADR 0723 D3: the same local root

/// `|a - b|_inf / |a|_inf`.
fn rel_dist(a: &[f64], b: &[f64]) -> f64 {
    max_abs(a.iter().zip(b).map(|(x, y)| x - y)) / max_abs(a.iter().copied())
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

// ============================================================================ ADR 0723 D1: the commit on the block

/// The element watched (ADR 0718 X1).
const E_W: usize = 3720;

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

// ============================================================================ ADR 0724 D2: the extended system

/// `K x` for a CSR matrix.
fn mv(k: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    (0..k.n)
        .map(|r| (k.row_ptr[r]..k.row_ptr[r + 1]).map(|p| k.val[p] * x[k.col[p]]).sum())
        .collect()
}

fn diag_max(k: &CsrMatrix) -> f64 {
    k.diag().iter().fold(0.0f64, |w, x| w.max(x.abs()))
}

/// ADR 0724 D4: the null residual `nu_r = |K psi|_2 / (max_i |K_ii| |psi|_2)`.
fn null_residual(k: &CsrMatrix, psi: &[f64]) -> f64 {
    norm(mv(k, psi).into_iter()) / (diag_max(k) * norm(psi.iter().copied()))
}

/// ADR 0724 D2: the step of the central differences of the tangent (in the 2-norm of the free displacement).
const H_FD: f64 = 1e-8;

/// ADR 0724 D2: `G(d, t) = D_(u, lambda)[K psi] (d, t)` by central differences, `e = h / |d|_2`, the same states.
fn dir_deriv<B: Bordered>(case: &B, s: &St, psi: &[f64], d: &[f64], t: f64) -> Vec<f64> {
    let nd = norm(d.iter().copied());
    if nd == 0.0 {
        return vec![0.0; psi.len()];
    }
    let free = case.free();
    let e = H_FD / nd;
    let side = |sg: f64| {
        let mut x = s.clone();
        for (&q, v) in free.iter().zip(d) {
            x.u[q] += sg * e * v;
        }
        let x = case.at_load(&x, s.p + sg * e * t);
        mv(&case.system(&x).k, psi)
    };
    let (p, m) = (side(1.0), side(-1.0));
    p.iter().zip(&m).map(|(a, b)| (a - b) / (2.0 * e)).collect()
}

/// ADR 0724 D4: the tolerances of the stop rule and the most iterations.
struct ExtTol {
    r: f64,
    nu: f64,
    l: f64,
    iterations: usize,
}

/// The result of the extended system: the state, the null vector, the system there, the iterations and the measures.
struct ExtOut {
    s: St,
    psi: Vec<f64>,
    sys: Sys,
    its: usize,
    m_r: f64,
    nu_r: f64,
    l_err: f64,
}

/// ADR 0724 D2, D4: Newton on `R_F = 0`, `K psi = 0`, `l . psi = 1` by block elimination with `K` (four solves, one factor)
/// from `start` with `psi_0` (`l = psi_0`); the first iterate `k >= 1` meeting the three tolerances is the result.
fn extended<B: Bordered>(
    case: &B,
    tag: &str,
    start: &St,
    psi0: &[f64],
    phi: &[f64],
    tol: &ExtTol,
) -> Result<ExtOut, String> {
    let free = case.free();
    let l = psi0.to_vec();
    let mut s = start.clone();
    let mut psi = psi0.to_vec();
    let t0 = Instant::now();
    for k in 0..=tol.iterations {
        let (r, m_r) = case.residual(&s)?;
        let sys = case.system(&s);
        let nu_r = null_residual(&sys.k, &psi);
        let l_err = (dot(&l, &psi) - 1.0).abs();
        println!(
            "{tag} extended iterate {k}: a {:+.12e}, lambda {:.15}, m_R {m_r:.4e}, nu_r {nu_r:.4e}, |l psi - 1| {l_err:.3e}, {:.1} s",
            amp(phi, &free, &s.u),
            s.p,
            t0.elapsed().as_secs_f64()
        );
        if !(m_r.is_finite() && nu_r.is_finite() && l_err.is_finite()) {
            return Err(format!("a non-finite measure at iteration {k}"));
        }
        if k >= 1 && m_r <= tol.r && nu_r <= tol.nu && l_err <= tol.l {
            return Ok(ExtOut {
                s,
                psi,
                sys,
                its: k,
                m_r,
                nu_r,
                l_err,
            });
        }
        if k == tol.iterations {
            break;
        }
        let fac = factor_nonzero(&sys.k).ok_or_else(|| format!("the factor refused at iteration {k}"))?;
        let neg = |v: &[f64]| v.iter().map(|x| -x).collect::<Vec<f64>>();
        let y1 = fac.solve(&neg(&r));
        let y2 = fac.solve(&sys.f);
        let w1 = dir_deriv(case, &s, &psi, &y1, 0.0);
        let w2 = dir_deriv(case, &s, &psi, &y2, 1.0);
        let z1 = fac.solve(&neg(&w1));
        let z2 = fac.solve(&neg(&w2));
        let dl = (1.0 - dot(&l, &z1)) / dot(&l, &z2);
        if !dl.is_finite() {
            return Err(format!("a non-finite load correction at iteration {k}"));
        }
        for (j, &q) in free.iter().enumerate() {
            s.u[q] += y1[j] + dl * y2[j];
        }
        s = case.at_load(&s, s.p + dl);
        psi = z1.iter().zip(&z2).map(|(a, b)| a + dl * b).collect();
    }
    Err(format!("not converged in {} iterations", tol.iterations))
}

/// A vector of unit 2-norm with its largest component positive.
fn unit_signed(v: &[f64]) -> Vec<f64> {
    let n = norm(v.iter().copied());
    let big = v.iter().fold(0.0f64, |w, &x| if x.abs() > w.abs() { x } else { w });
    let s = if big < 0.0 { -1.0 / n } else { 1.0 / n };
    v.iter().map(|x| x * s).collect()
}

/// Whether two residuals are equal bit for bit.
fn same_bits(a: &Result<(Vec<f64>, f64), String>, b: &Result<(Vec<f64>, f64), String>) -> bool {
    match (a, b) {
        (Ok((x, _)), Ok((y, _))) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| p.to_bits() == q.to_bits()),
        _ => false,
    }
}

// ============================================================================ the fast test

const TOL_TOY: ExtTol = ExtTol {
    r: 1e-13,
    nu: 1e-11,
    l: 1e-12,
    iterations: 12,
};

#[test]
fn f843_fast() {
    let mut ch = Checks::new();
    let t_all = Instant::now();
    let plain = Pw::new(false);
    let warm = Pw::new(true);

    // The closed-form fold (ADR 0724 D5).
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
    let p_f = Pw::p_of(a_f);
    let share_f = 1.0 / (1.0 + a_f * a_f);
    println!("PW: a_f {a_f:.16}, p_f {p_f:.16}, the share of psi_f on u_0 {share_f:.16}");

    // TB: the bracket of ADR 0723's PW-W trace.
    let o = FineOpts {
        da: 0.05,
        steps: 16,
        halvings: 6,
        cap: 60,
    };
    let tw = pw_trace(&warm, "PW", 0.5, &o, &mut ch);
    let Some(j) = tw.sign else {
        ch.holds("TB: a sign change of sigma on the PW-W trace", false);
        ch.finish();
        return;
    };
    let (lo_pt, hi_pt) = (tw.points[j - 1].pt.clone(), tw.points[j].pt.clone());
    ch.holds(
        &format!(
            "TB: the ends a {:.12} (nu {:?}) and {:.12} (nu {:?}), a_f between",
            lo_pt.a, lo_pt.info.nu, hi_pt.a, hi_pt.info.nu
        ),
        (lo_pt.a - 0.90).abs() <= 1e-9
            && (hi_pt.a - 0.95).abs() <= 1e-9
            && lo_pt.info.nu == Some(1)
            && hi_pt.info.nu == Some(0)
            && lo_pt.a <= a_f
            && a_f <= hi_pt.a,
    );
    let phi: Vec<f64> = (0..TOY_N).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();

    let mut ends = Vec::new();
    for (tag, pt) in [("MS-L", &lo_pt), ("MS-U", &hi_pt)] {
        let psi0 = dense_lowest(&pt.info.sys.k)
            .map(|p| unit_signed(&p.phi))
            .unwrap_or(vec![f64::NAN; TOY_N]);
        let nu0 = null_residual(&pt.info.sys.k, &psi0);
        ch.holds(
            &format!("NC2: nu_r at the bracket's end a {:.2} ({nu0:.4e}) >= 1e-4", pt.a),
            nu0 >= 1e-4,
        );
        let fb0 = *warm.fallbacks.borrow();
        match extended(&warm, tag, &pt.s, &psi0, &phi, &TOL_TOY) {
            Ok(x) => {
                let a = x.s.u[0];
                let share = x.psi[0] * x.psi[0] / dot(&x.psi, &x.psi);
                println!(
                    "{tag}: a {a:.16} lambda {:.16} share {share:.16}, {} iterations, m_R {:.3e}, nu_r {:.3e}, |l psi - 1| {:.3e}",
                    x.s.p, x.its, x.m_r, x.nu_r, x.l_err
                );
                ch.holds(&format!("{tag}1: converged ({} iterations <= 6)", x.its), x.its <= 6);
                ch.at_most(&format!("{tag}2: |a - a_f|"), (a - a_f).abs(), 1e-10);
                ch.at_most(&format!("{tag}2: |lambda - p_f|"), (x.s.p - p_f).abs(), 1e-12);
                ch.at_most(
                    &format!("{tag}3: |share on u_0 - 1 / (1 + a_f^2)|"),
                    (share - share_f).abs(),
                    1e-9,
                );
                ch.at_most(&format!("{tag}4: nu_r at the result"), x.nu_r, 1e-11);
                let c = warm.commit(x.s.clone());
                ch.equals(
                    &format!("{tag}5: fallbacks in the commit"),
                    *warm.fallbacks.borrow() - fb0,
                    0,
                );
                ch.at_most(
                    &format!("{tag}5: |z_committed - z*(a)|"),
                    (Pw::committed(&c).unwrap_or(f64::NAN) - Pw::z_of(a)).abs(),
                    1e-12,
                );
                ch.holds(
                    &format!("{tag}5: the residual after the commit bit-identical"),
                    same_bits(&warm.residual(&c), &warm.residual(&x.s)),
                );
                ends.push(a);
            }
            Err(e) => {
                ch.holds(&format!("{tag}1: converged ({e})"), false);
                ends.push(f64::NAN);
            }
        }
    }
    ch.at_most("MS-LU: |a_L - a_U|", (ends[0] - ends[1]).abs(), 1e-10);

    // NC1: the extended system on the plain toy (no warm start) from the lower end.
    let psi0 = dense_lowest(&lo_pt.info.sys.k)
        .map(|p| unit_signed(&p.phi))
        .unwrap_or(vec![f64::NAN; TOY_N]);
    let nc1 = extended(&plain, "NC1", &lo_pt.s, &psi0, &phi, &TOL_TOY);
    let text = match &nc1 {
        Ok(_) => "converged".to_string(),
        Err(e) => e.clone(),
    };
    ch.holds(
        &format!("NC1: the plain toy fails with a local error ({text})"),
        nc1.is_err() && text.starts_with("local"),
    );
    println!("f843_fast: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}

// ============================================================================ the heavy record

/// ADR 0723's fold bracket (`f841-scratch/run2-heavy.log` lines 76 and 81) and its estimate (ADR 0724 arithmetic (i)).
const A_LO: f64 = 2.164988e-4;
const A_HI: f64 = 2.204620e-4;
const L_LO: f64 = 0.9584711481;
const L_HI: f64 = 0.958472223921;
const UB_LAMBDA: f64 = 0.958478523962;
const UB_A: f64 = 2.204619895e-4;
const UB_SIGMA: f64 = 3.754511;
const A_EST: f64 = 2.16996e-4;
const L_EST: f64 = 0.95847209;

const TOL_BLOCK: ExtTol = ExtTol {
    r: 1e-10,
    nu: 1e-9,
    l: 1e-8,
    iterations: 12,
};

#[test]
#[ignore = "ADR 0724: the heavy record of the extended system at ADR 0723's fold bracket on 16^3, run once with --ignored (63.9 s), every row met: rung 5 rebuilt and committed on the warm model bit-identical to the plain one; the upper end reproduced (lambda to 5.6e-14); the Moore-Spence system from rung 5 converged in 2 iterations to a_f 2.170072e-4, lambda_f 0.958472086948 (inside the bracket), nu_r 3.6e-11, the critical vector's share on element 3720 0.586, the critical value simple and lowest; the commit bit-identical; f843_fast covers the same procedures"]
fn f843_nh16_fold_heavy() {
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
        "S5: lambda {:.15}, a {:+.12e}, sigma {:+.9e}; element {E_W}: K_aa l_min {:+.6e}, {} local iterations, det F min {:.6}, \
         {:.1} s",
        s5.s.p,
        s5.a,
        s5.sigma,
        d5.eig[0],
        d5.its,
        d5.det,
        t_all.elapsed().as_secs_f64()
    );

    // D1, W5: the warm model and the commit of S5.
    let mut mw = block_model(&mesh16, K_L);
    mw.set_incompatible_mode_warm_start(true);
    let mut m2w = block_model_free_punch(&mesh16, K_L);
    m2w.set_incompatible_mode_warm_start(true);
    let fbw = FemBranch::new(&mw, &m2w);
    let c5 = commit_fem(&mw, &mat, &s5.s);
    println!(
        "D1: S5 committed on the warm model: {} fallbacks, {} failed, CW {}, {:.1} s",
        c5.fallbacks,
        c5.failed,
        c5.cw,
        t_all.elapsed().as_secs_f64()
    );
    ch.equals("W5: failed elements in the commit of S5", c5.failed, 0);
    ch.holds("W5: CW at S5", c5.cw);
    ch.holds(
        "W5: the free residual at S5_W bit-identical to S5's",
        same_bits(&fb.residual(&s5.s), &fbw.residual(&c5.s)),
    );
    let i5w = analyse(&fbw, &c5.s);
    let s5w = apoint(&fbw, c5.s, i5w, &phi, Some(0));
    ch.holds(
        &format!(
            "W5: sigma at S5_W {:+.15e} bit-identical to S5's {:+.15e}",
            s5w.sigma, s5.sigma
        ),
        s5w.sigma.to_bits() == s5.sigma.to_bits(),
    );
    ch.holds(
        &format!("W5: nu at S5_W {:?} equal to S5's {:?}", s5w.info.nu, s5.info.nu),
        s5w.info.nu == s5.info.nu && s5w.info.nu.is_some(),
    );

    // UB: the upper end of the bracket from S5_W (ADR 0723's halved step 9), committed.
    let da = (s5.a - s4.a) / 8.0;
    let tgt = s5w.a + 0.5 * ((s4.a + 9.0 * da) - s5w.a);
    let t0 = Instant::now();
    match attempt(&fbw, &s5w, &phi, tgt, false) {
        Ok((s, its, corr, step)) => {
            let c = commit_fem(&mw, &mat, &s);
            let info = analyse(&fbw, &c.s);
            let ub = apoint(&fbw, c.s, info, &phi, None);
            println!(
                "UB: a -> {:+.12e}: accepted, lambda {:.12}, nu {:?}, dlambda/da {:+.6e}, {its} iterations, continuity {corr:.3e} \
                 <= {step:.3e}, commit {} fallbacks {} failed, {:.1} s",
                ub.a,
                ub.s.p,
                ub.info.nu,
                ub.sigma,
                c.fallbacks,
                c.failed,
                t0.elapsed().as_secs_f64()
            );
            ch.at_most("UB: |lambda - 0.958478523962|", (ub.s.p - UB_LAMBDA).abs(), 1e-9);
            ch.at_most("UB: a against 2.204619895e-4", (ub.a / UB_A - 1.0).abs(), 1e-8);
            ch.at_most("UB: sigma against +3.754511", (ub.sigma / UB_SIGMA - 1.0).abs(), 1e-5);
            ch.equals("UB: nu", ub.info.nu.unwrap_or(usize::MAX), 0);
        }
        Err((e, _)) => ch.holds(&format!("UB: the upper end accepted ({e})"), false),
    }

    // MS: the extended system from S5_W.
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
    let Some(psi0) = e5.as_ref().map(|e| unit_signed(&e.vectors[0])) else {
        ch.holds("MS: the lowest pair at S5", false);
        ch.finish();
        return;
    };
    println!(
        "MS: start at S5_W, nu_r {:.4e}, the share of psi_0 on element {E_W} {:.4e}",
        null_residual(&s5w.info.sys.k, &psi0),
        share(&psi0)
    );
    let x = match extended(&fbw, "MS", &s5w.s, &psi0, &phi, &TOL_BLOCK) {
        Ok(x) => x,
        Err(e) => {
            ch.holds(&format!("MS1: the extended system converged ({e})"), false);
            ch.finish();
            return;
        }
    };
    ch.holds(
        &format!("MS1: the extended system converged ({} iterations)", x.its),
        true,
    );
    let a_f = amp(&phi, &free, &x.s.u);
    let l_f = x.s.p;
    ch.holds(
        &format!("MS2: a_f {a_f:.12e} in [{A_LO:e}, {A_HI:e}]"),
        (A_LO..=A_HI).contains(&a_f),
    );
    ch.holds(
        &format!("MS2: lambda_f {l_f:.15} in [{L_LO}, {L_HI}]"),
        (L_LO..=L_HI).contains(&l_f),
    );
    ch.at_most("MS3: nu_r at the result", x.nu_r, 1e-9);
    ch.at_most("MS3: m_R at the result", x.m_r, 1e-10);
    // MS3: the unsymmetrised reduced tangent at the result.
    let kt = tangent(&mw, &x.s.u, &x.s.states, Some(4)).unwrap();
    let fixed = prescribed_equations(&mw, &[]).unwrap();
    let ku = reduce(&kt, &vec![0.0; kt.n], &fixed).unwrap().matrix;
    let nu_u = norm(mv(&ku, &x.psi).into_iter()) / (diag_max(&ku) * norm(x.psi.iter().copied()));
    ch.at_most("MS3: nu_r with the unsymmetrised tangent", nu_u, 1e-8);
    let ks: HashMap<(usize, usize), f64> = (0..x.sys.k.n)
        .flat_map(|r| (x.sys.k.row_ptr[r]..x.sys.k.row_ptr[r + 1]).map(move |p| (r, p)))
        .map(|(r, p)| ((r, x.sys.k.col[p]), x.sys.k.val[p]))
        .collect();
    let (mut asym, mut kmax) = (0.0f64, 0.0f64);
    for r in 0..ku.n {
        for p in ku.row_ptr[r]..ku.row_ptr[r + 1] {
            let v = ku.val[p];
            kmax = kmax.max(v.abs());
            asym = asym.max(2.0 * (v - ks.get(&(r, ku.col[p])).copied().unwrap_or(0.0)).abs());
        }
    }
    // MS4: the share on element 3720.
    let sh = share(&x.psi);
    ch.holds(
        &format!("MS4: the share of psi on element {E_W} {sh:.6} in [0.30, 0.65]"),
        (0.30..=0.65).contains(&sh),
    );
    // LS: the critical value simple and lowest.
    let n_plus = inertia(&combine(&x.sys.k, &id, 1000.0));
    let n_minus = inertia(&combine(&x.sys.k, &id, -1000.0));
    ch.holds(
        &format!("LS: inertia of K_f + 1000 I {n_plus:?} = 0, of K_f - 1000 I {n_minus:?} = 1"),
        n_plus == Some(0) && n_minus == Some(1),
    );
    // MSR: the reported measures.
    let kpsi = mv(&x.sys.k, &x.psi);
    let rq = dot(&x.psi, &kpsi) / dot(&x.psi, &x.psi);
    let pf_n = dot(&x.psi, &x.sys.f) / (norm(x.psi.iter().copied()) * norm(x.sys.f.iter().copied()));
    let ph = unit_signed(&x.psi);
    let along = |sg: f64| {
        let mut y = x.s.clone();
        for (k, &q) in free.iter().enumerate() {
            y.u[q] += sg * H_FD * ph[k];
        }
        fbw.residual(&y).map(|r| r.0)
    };
    let fd_res = match (along(1.0), along(-1.0)) {
        (Ok(p), Ok(q)) => norm(p.iter().zip(&q).map(|(a, b)| (a - b) / (2.0 * H_FD))) / diag_max(&x.sys.k),
        _ => f64::NAN,
    };
    println!(
        "MSR: a_f {a_f:+.12e} lambda_f {l_f:.15} | {} iterations, m_R {:.4e}, nu_r {:.4e}, |l psi - 1| {:.3e} | Rayleigh quotient \
         {rq:+.6e}, max |K_ii| {:.6e}, psi . f / (|psi| |f|) {pf_n:+.6e} | nu_r unsymmetrised {nu_u:.4e}, asymmetry {:.3e} | \
         the residual along psi {fd_res:.4e} | share on element {E_W} {sh:.6} | against ADR 0723's estimate: a {:+.4e} relative, \
         lambda {:+.4e}",
        x.its,
        x.m_r,
        x.nu_r,
        x.l_err,
        diag_max(&x.sys.k),
        asym / kmax,
        a_f / A_EST - 1.0,
        l_f - L_EST
    );

    // CF: the commit of the result.
    let cf = commit_fem(&mw, &mat, &x.s);
    println!(
        "CF: the result committed: {} fallbacks (element {E_W}: {}), {} failed, CW {}",
        cf.fallbacks, cf.watched, cf.failed, cf.cw
    );
    ch.equals("CF: failed elements in the commit of the result", cf.failed, 0);
    ch.holds("CF: CW at the result", cf.cw);
    ch.holds(
        "CF: the free residual at F_W bit-identical to F's",
        same_bits(&fbw.residual(&x.s), &fbw.residual(&cf.s)),
    );
    if let Some(alpha) = committed_modes(&cf.s.states[E_W]) {
        let el = element_of(&mw, E_W, &cf.s.u, &cf.s.states);
        let lmin = mode_block(&mat, &el, &alpha)
            .map(|(_, k)| spectrum(&k).1[0])
            .unwrap_or(f64::NAN);
        let cold = match record_from(&mat, &el, None).1 {
            Ok(a0) => format!("converged, distance {:.4e}", rel_dist(&alpha, &a0)),
            Err(t) => t,
        };
        println!("MSR: element {E_W} at F_W: K_aa l_min {lmin:+.6e}; the plain iteration from 0: {cold}");
    }
    println!("f843_nh16_fold_heavy: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}
