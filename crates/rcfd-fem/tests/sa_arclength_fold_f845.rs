//! F-845 (ADR 0728): the library arc-length driver through rule L's 16^3 fold with warm-started local modes. The heavy
//! record: ADR 0711's rungs rebuilt on the plain model and rung 5 committed on the model with the warm start of the
//! incompatible modes (ADR 0724 D1; the helpers are copies of those of `si_eas_hg21.rs`, existing tests are not edited), then
//! `arclength::trace_from` (ADR 0726) from rung 5 on the warm model, the committed element states carried by the driver,
//! through the fold computed by ADR 0724: the fold read from the load factor and the inertia, the steps and the local
//! fallbacks, the states against a bordered reference on the warm model, the constraint with a floor for short arcs. The fast
//! test: ADR 0726's closed-form snap-back under a prescribed displacement with a local unknown at the apex (ADR 0723's PW
//! local energy) traced by `trace_from` through both closed-form folds with the warm start and the commit, its negative
//! control without the warm start, a fallback control, and the driver's carry of the warm modes on a `4^3` block. Each
//! registered run is made once; a miss stays as an ignored `*_known_miss` test with its limit. Every aggregated value is
//! required finite before it is compared.

use rcfd_core::vec3::Vec3;
use rcfd_fem::arclength::{trace_from, ArcLengthOptions, PathPoint, PathStart};
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
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, Material, SolidFormulation, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{
    lowest_eigenpairs, symmetric_eigen, Factorisation, GeneralizedEigen, ResidualTest, Shift, SubspaceOptions,
};
use rcfd_linalg::skyline::PivotRule;
use rcfd_linalg::sparse_chol::{SparseOptions, SparseOrdering, SupernodalLdlt};
use rcfd_linalg::CsrMatrix;
use std::cell::{Cell, RefCell};
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
/// state followed by the converged modes (four threads; the result does not depend on them); CW on the element `watched`.
fn commit_fem(m: &SolidModel, mat: &Material, s: &St, watched: usize) -> Commit {
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
        if e == watched {
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
    let el = element_of(m, watched, &s.u, &s.states);
    out.cw = condensed_force_and_tangent_warm(
        &el.coords,
        mat,
        &el.ue,
        &s.states[watched],
        &PointContext::default(),
        LIN,
        IncompatibleModeUpdate::Plain,
    )
    .map(|w| {
        let c = out.s.states[watched].values();
        w.3.len() == c.len() && w.3.iter().zip(c).all(|(x, y)| x.to_bits() == y.to_bits())
    })
    .unwrap_or(false);
    out
}

/// Whether two residuals are equal bit for bit.
fn same_bits(a: &Result<(Vec<f64>, f64), String>, b: &Result<(Vec<f64>, f64), String>) -> bool {
    match (a, b) {
        (Ok((x, _)), Ok((y, _))) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| p.to_bits() == q.to_bits()),
        _ => false,
    }
}

/// `|a - b|_inf / |a|_inf`.
fn rel_dist(a: &[f64], b: &[f64]) -> f64 {
    max_abs(a.iter().zip(b).map(|(x, y)| x - y)) / max_abs(a.iter().copied())
}

/// Whether two lists of element states are equal bit for bit.
fn same_states(a: &[ElementState], b: &[ElementState]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            x.values().len() == y.values().len()
                && x.values()
                    .iter()
                    .zip(y.values())
                    .all(|(p, q)| p.to_bits() == q.to_bits())
        })
}

fn expand(free: &[usize], n: usize, x: &[f64]) -> Vec<f64> {
    let mut v = vec![0.0; n];
    for (&e, &xi) in free.iter().zip(x) {
        v[e] = xi;
    }
    v
}

// ============================================================================ D5: the constraint with a floor

/// D5: at every point, from the previous one (`start` first), the measure `e_j` of the constraint `|Du_F|^2 + c Dlambda^2 =
/// Ds^2` and its limit `1e-10 + 4 (k_j + 1) eps (|u_j|_F,2 + sqrt(c) |lambda_j|) / Ds_j`.
fn constraint_floor(free: &[usize], start: (&[f64], f64), points: &[PathPoint], c: f64) -> Vec<(f64, f64)> {
    let (mut pu, mut pl) = (start.0.to_vec(), start.1);
    let mut out = Vec::with_capacity(points.len());
    for p in points {
        let du2: f64 = free
            .iter()
            .map(|&q| (p.displacement[q] - pu[q]) * (p.displacement[q] - pu[q]))
            .sum();
        let dl = p.load_factor - pl;
        let ds2 = p.length * p.length;
        let e = (du2 + c * dl * dl - ds2).abs() / ds2;
        let un = norm(free.iter().map(|&q| p.displacement[q]));
        let k = p.residuals.len().saturating_sub(1) as f64;
        let limit = 1e-10 + 4.0 * (k + 1.0) * f64::EPSILON * (un + c.sqrt() * p.load_factor.abs()) / p.length;
        out.push((e, limit));
        pu = p.displacement.clone();
        pl = p.load_factor;
    }
    out
}

/// Whether every point carries `lambda v` on the prescribed equations (`f64` equality).
fn prescribed_follow(fixed: &[(usize, f64)], points: &[PathPoint]) -> bool {
    points
        .iter()
        .all(|p| fixed.iter().all(|&(eq, v)| p.displacement[eq] == p.load_factor * v))
}

fn free_of(m: &SolidModel, fixed: &[(usize, f64)]) -> Vec<usize> {
    let n = m.dof_map().n_equations();
    let mut is_fixed = vec![false; n];
    for &(eq, _) in fixed {
        is_fixed[eq] = true;
    }
    (0..n).filter(|&q| !is_fixed[q]).collect()
}

// ============================================================================ D3: the reading of a fold

/// A point of a path for the reading: the amplitude, the load factor, `dlambda/da` and the inertia.
#[derive(Clone, Copy, Debug)]
struct RPoint {
    a: f64,
    l: f64,
    sigma: f64,
    nu: Option<usize>,
}

/// The kind of fold: a maximum of the load factor (`sigma` from `> 0` to `<= 0`, `nu` 0 to 1) or a minimum (`sigma` from `< 0`
/// to `>= 0`, `nu` 1 to 0).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Extremum {
    Max,
    Min,
}

/// A fold read between the points `j - 1` and `j`: the load factor bracket and ADR 0711's estimate.
#[derive(Clone, Copy, Debug)]
struct Read {
    j: usize,
    lo: f64,
    hi: f64,
    a_est: f64,
    l_est: f64,
}

/// D3: the first sign change of `sigma` of the kind `kind` along `pts`: `Ok` the fold if the inertia changes by the rule,
/// `Err(j)` ("undecided") otherwise; `None` if there is none.
fn read_fold(pts: &[RPoint], kind: Extremum) -> Option<Result<Read, usize>> {
    for j in 1..pts.len() {
        let (p0, p1) = (pts[j - 1], pts[j]);
        let (change, nu_rule) = match kind {
            Extremum::Min => (p0.sigma < 0.0 && p1.sigma >= 0.0, p0.nu == Some(1) && p1.nu == Some(0)),
            Extremum::Max => (p0.sigma > 0.0 && p1.sigma <= 0.0, p0.nu == Some(0) && p1.nu == Some(1)),
        };
        if !change {
            continue;
        }
        if !nu_rule {
            return Some(Err(j));
        }
        let (a0, l0, s0) = (p0.a, p0.l, p0.sigma);
        let (a1, l1, s1) = (p1.a, p1.l, p1.sigma);
        let ax = (l1 - l0 + s0 * a0 - s1 * a1) / (s0 - s1);
        let lx = l0 + s0 * (ax - a0);
        let (lo, hi) = match kind {
            Extremum::Min => (lx, l0.min(l1)),
            Extremum::Max => (l0.max(l1), lx),
        };
        let a_est = a0 - s0 * (a1 - a0) / (s1 - s0);
        let l_est = l0 + s0 * (a_est - a0) / 2.0;
        return Some(Ok(Read {
            j,
            lo,
            hi,
            a_est,
            l_est,
        }));
    }
    None
}

/// D3: the containment of a fold `(a_f, lambda_f)` in a reading, `lambda_f` within `tau` of its bracket.
fn contains(pts: &[RPoint], r: &Read, a_f: f64, l_f: f64, tau: f64) -> (bool, bool) {
    let (a0, a1) = (pts[r.j - 1].a, pts[r.j].a);
    (a0 < a_f && a_f < a1, l_f >= r.lo - tau && l_f <= r.hi + tau)
}

/// The containment tolerance of D3 in the load factor.
const TAU: f64 = 1e-9;

// ============================================================================ D4: the snap-back of ADR 0726 D3 with a local unknown

const E: f64 = 2.0e11;
const L0: f64 = 1.0;
const H: f64 = 0.2;
const S: f64 = 0.1;
/// The effective area of the bipyramid, `V/L0 = s^2/3` (ADR 0480 arithmetic 5).
const A: f64 = S * S / 3.0;
const APEX: usize = 2;
/// The series bar (ADR 0726 D3): its material, length, the prescribed displacement of its far end at load 1.
const E_S: f64 = E / 50.0;
const L_S: f64 = 1.0;
const D_TOP: f64 = 1.0;
const TOP: usize = 3;
/// The local unknown at the apex (D4).
const Z_C: f64 = 200.0;
const Z_D: f64 = 1.0;
const Z_K: f64 = 5.0e4;

fn p_closed(w: f64) -> f64 {
    E * A * (H - w) * (2.0 * H * w - w * w) / L0.powi(3)
}

fn dp_closed(w: f64) -> f64 {
    E * A * (3.0 * (w - H) * (w - H) - H * H) / L0.powi(3)
}

/// D4: the local force at the root, `Q(w) = kappa d w (2 c w^2 - tan(d w^2 / 2))`.
fn q_closed(w: f64) -> f64 {
    Z_K * Z_D * w * (2.0 * Z_C * w * w - (Z_D * w * w / 2.0).tan())
}

fn dq_closed(w: f64) -> f64 {
    let t = Z_D * w * w / 2.0;
    Z_K * Z_D * (6.0 * Z_C * w * w - t.tan() - Z_D * w * w / (t.cos() * t.cos()))
}

/// The root `z*(w) = c w^2 - tan(d w^2 / 2)` of the local equation.
fn z_closed(w: f64) -> f64 {
    Z_C * w * w - (Z_D * w * w / 2.0).tan()
}

/// The stretch `r` of the series bar under the force `P(w) + Q(w)` (Newton from 1, 100 iterations).
fn r_closed(w: f64) -> f64 {
    let p = p_closed(w) + q_closed(w);
    let mut r = 1.0f64;
    for _ in 0..100 {
        let g = E_S * A * r * (1.0 - r * r) / 2.0 - p;
        let dg = E_S * A * (1.0 - 3.0 * r * r) / 2.0;
        r -= g / dg;
    }
    r
}

fn lambda_closed(w: f64) -> f64 {
    (w + L_S * (1.0 - r_closed(w))) / D_TOP
}

fn dlambda_closed(w: f64) -> f64 {
    let r = r_closed(w);
    let kt = -E_S * A * (1.0 - 3.0 * r * r) / 2.0 / L_S;
    (1.0 + (dp_closed(w) + dq_closed(w)) / kt) / D_TOP
}

fn bisect(f: impl Fn(f64) -> f64, mut a: f64, mut b: f64, n: usize) -> f64 {
    for _ in 0..n {
        let m = 0.5 * (a + b);
        if f(a) * f(m) <= 0.0 {
            b = m;
        } else {
            a = m;
        }
    }
    0.5 * (a + b)
}

fn volume(p: &[Vec3; 4]) -> f64 {
    let d = |a: Vec3, b: Vec3| [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let (u, v, w) = (d(p[0], p[1]), d(p[0], p[2]), d(p[0], p[3]));
    u[0] * (v[1] * w[2] - v[2] * w[1]) - u[1] * (v[0] * w[2] - v[2] * w[0]) + u[2] * (v[0] * w[1] - v[1] * w[0])
}

/// A bar from node `p` to node `q` of length `len`: a bipyramid of four Tet4 around its axis (ADR 0480 D6).
fn bar(nodes: &mut Vec<Vec3>, conn: &mut Vec<usize>, p: usize, q: usize, len: f64) {
    let (pp, pq) = (nodes[p], nodes[q]);
    let a0 = [(pq[0] - pp[0]) / len, (pq[1] - pp[1]) / len];
    let n0 = [-a0[1], a0[0]];
    let m = [(pp[0] + pq[0]) / 2.0, (pp[1] + pq[1]) / 2.0];
    let first = nodes.len();
    for (cn, cz) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        nodes.push([m[0] + cn * S / 2.0 * n0[0], m[1] + cn * S / 2.0 * n0[1], cz * S / 2.0]);
    }
    for i in 0..4 {
        let mut t = [p, q, first + i, first + (i + 1) % 4];
        if volume(&[nodes[t[0]], nodes[t[1]], nodes[t[2]], nodes[t[3]]]) < 0.0 {
            t.swap(2, 3);
        }
        conn.extend_from_slice(&t);
    }
}

/// ADR 0726 D3's model: ADR 0480's truss (nodes 0, 1 the supports, 2 the apex) with the series bar from the apex to the top
/// node 3, prescribed to `x = 0`, `y = -d` at load 1.
fn snap_model() -> SolidModel {
    let a = (L0 * L0 - H * H).sqrt();
    let mut nodes: Vec<Vec3> = vec![[-a, 0.0, 0.0], [a, 0.0, 0.0], [0.0, H, 0.0], [0.0, H + L_S, 0.0]];
    let mut conn = Vec::new();
    for b in 0..2usize {
        bar(&mut nodes, &mut conn, b, APEX, L0);
    }
    let n_bars = conn.len() / 4;
    bar(&mut nodes, &mut conn, APEX, TOP, L_S);
    let nn = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Tet4, conn).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("bars", (0..n_bars).collect()).unwrap();
    mesh.add_node_set("pins", vec![0, 1]).unwrap();
    mesh.add_node_set("every", (0..nn).collect()).unwrap();
    mesh.add_element_set("spring", (n_bars..ne).collect()).unwrap();
    mesh.add_node_set("top", vec![TOP]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("bars", Material::isotropic_elastic(E, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("pins", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("pins", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("every", 2, 0.0)).unwrap();
    m.add_material("spring", Material::isotropic_elastic(E_S, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("top", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("top", 1, -D_TOP)).unwrap();
    m
}

/// D4: the local Newton on `g = atan(z - c w^2) + d w^2 / 2` from `z0` (25 iterations, `|g| <= 1e-14`).
fn local_newton(w: f64, z0: f64) -> Result<f64, String> {
    let mut z = z0;
    for k in 0..=25 {
        let x = z - Z_C * w * w;
        let g = x.atan() + Z_D * w * w / 2.0;
        if !(g.is_finite() && z.is_finite()) {
            return Err(format!("local: not finite at iteration {k}"));
        }
        if g.abs() <= 1e-14 {
            return Ok(z);
        }
        if k == 25 {
            break;
        }
        z -= g * (1.0 + x * x);
    }
    Err("local: not converged in 25 iterations".to_string())
}

/// D4: the force on the apex's `y` equation and the condensed tangent at the root `z`.
fn local_force(w: f64, z: f64) -> (f64, f64) {
    let x = z - Z_C * w * w;
    let wzz = 1.0 / (1.0 + x * x);
    let wwz = -2.0 * Z_C * w / (1.0 + x * x) + Z_D * w;
    let www = -2.0 * Z_C * x.atan() + 4.0 * Z_C * Z_C * w * w / (1.0 + x * x) + Z_D * z;
    let dw = -2.0 * Z_C * w * x.atan() + Z_D * w * z;
    (-Z_K * dw, Z_K * (www - wwz * wwz / wzz))
}

/// D4: the local unknown at the apex, a contribution on the apex's `y` equation (`w = -u_y`): the warm start from the
/// committed root and the fallback from 0, or the plain start from 0.
struct LocalZ {
    eq: usize,
    warm: bool,
    committed: Cell<f64>,
    fallbacks: Cell<usize>,
}

impl LocalZ {
    /// The root at `w` by the start rule, and whether the warm iteration fell back.
    fn solve(&self, w: f64) -> Result<(f64, bool), String> {
        if self.warm {
            match local_newton(w, self.committed.get()) {
                Ok(z) => Ok((z, false)),
                Err(_) => local_newton(w, 0.0).map(|z| (z, true)),
            }
        } else {
            local_newton(w, 0.0).map(|z| (z, false))
        }
    }
    /// The commit at a converged displacement: the root by the start rule becomes the committed one; a fallback is counted.
    fn commit(&self, u: &[f64]) -> Result<bool, String> {
        let (z, fell) = self.solve(-u[self.eq])?;
        self.committed.set(z);
        if fell {
            self.fallbacks.set(self.fallbacks.get() + 1);
        }
        Ok(fell)
    }
}

impl Contribution for LocalZ {
    fn name(&self) -> &str {
        "local z"
    }
    fn add(
        &self,
        u: &[f64],
        _load_factor: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        let w = -u[self.eq];
        let (z, _) = self.solve(w).map_err(|reason| FemError::Solver {
            solver: "local z",
            reason,
        })?;
        let (f, k) = local_force(w, z);
        force[self.eq] += f;
        tangent.push((self.eq, self.eq, k));
        Ok(())
    }
}

fn apex_w(model: &SolidModel, u: &[f64]) -> f64 {
    -u[model.dof_map().equation(APEX, 1).unwrap()]
}

/// The analysis of a state of SZ with the local unknown at its committed root: the inertia of the reduced tangent (with the
/// contribution's triplet) and `dlambda/dw = 1 / (-v_t[apex y])`, `v_t = K^-1 (-K_Fp v)`.
fn snap_analyse(m: &SolidModel, fixed: &[(usize, f64)], lz: &LocalZ, u: &[f64]) -> (Option<usize>, f64) {
    let states = initial_states(m).unwrap();
    let Ok(kt) = tangent(m, u, &states, None) else {
        return (None, f64::NAN);
    };
    let (_, k) = local_force(-u[lz.eq], lz.committed.get());
    let mut t = Vec::with_capacity(kt.val.len() + 1);
    for r in 0..kt.n {
        for p in kt.row_ptr[r]..kt.row_ptr[r + 1] {
            t.push((r, kt.col[p], kt.val[p]));
        }
    }
    t.push((lz.eq, lz.eq, k));
    let kc = CsrMatrix::from_triplets(kt.n, t);
    let Ok(sys) = reduce(&kc, &vec![0.0; kc.n], fixed) else {
        return (None, f64::NAN);
    };
    let Some(pos) = sys.free().iter().position(|&q| q == lz.eq) else {
        return (None, f64::NAN);
    };
    match factor_nonzero(&symmetrise(&sys.matrix)) {
        Some(fac) => {
            let vt = fac.solve(&sys.rhs);
            (Some(fac.inertia().0), 1.0 / -vt[pos])
        }
        None => (None, f64::NAN),
    }
}

/// A point of the SZ trace with its commit and analysis.
struct ZRow {
    p: PathPoint,
    w: f64,
    z: f64,
    fell: bool,
    nu: Option<usize>,
}

#[test]
fn f845_fast() {
    let mut ck = Checks::new();
    let model = snap_model();
    let dofs = model.dof_map();
    let fixed = prescribed_equations(&model, &[]).unwrap();
    let free = free_of(&model, &fixed);
    let apex_y = dofs.equation(APEX, 1).unwrap();
    let top_y = dofs.equation(TOP, 1).unwrap();

    // TZ0: the closed form.
    let w_c = bisect(|w| if local_newton(w, 0.0).is_ok() { 1.0 } else { -1.0 }, 1e-4, 0.2, 60);
    let w1 = bisect(dlambda_closed, 0.0, H, 200);
    let w2 = bisect(dlambda_closed, H, 2.0 * H, 200);
    let (l_max, l_min) = (lambda_closed(w1), lambda_closed(w2));
    let r_min = 1.0 / 3f64.sqrt();
    println!(
        "TZ0 closed form: w_c {w_c:.9}, w_1/h {:.9}, w_2/h {:.9}, lambda_max {l_max:.12}, lambda_min {l_min:.12}, r(w_1) {:.6}, \
         r(w_2) {:.6}",
        w1 / H,
        w2 / H,
        r_closed(w1),
        r_closed(w2)
    );
    ck.holds(
        "TZ0: w_c < w_1 < h < w_2 < 1.8 h, r > 1/sqrt(3) at both folds",
        w_c < w1 && w1 < H && H < w2 && w2 < 1.8 * H && r_closed(w1) > r_min && r_closed(w2) > r_min,
    );

    // SZ-W: the warm trace with the commit.
    let options = ArcLengthOptions::new(0.005).max_points(600);
    let start0 = PathStart::unloaded(&model).unwrap();
    let lz = LocalZ {
        eq: apex_y,
        warm: true,
        committed: Cell::new(0.0),
        fallbacks: Cell::new(0),
    };
    let (nu0, sig0) = snap_analyse(&model, &fixed, &lz, &start0.displacement);
    let rows: RefCell<Vec<ZRow>> = RefCell::new(Vec::new());
    let rpts: RefCell<Vec<RPoint>> = RefCell::new(vec![RPoint {
        a: 0.0,
        l: 0.0,
        sigma: sig0,
        nu: nu0,
    }]);
    let commit_error: RefCell<Option<String>> = RefCell::new(None);
    let t0 = Instant::now();
    let mut stop = |p: &PathPoint| {
        let fell = match lz.commit(&p.displacement) {
            Ok(f) => f,
            Err(e) => {
                *commit_error.borrow_mut() = Some(e);
                return true;
            }
        };
        let w = apex_w(&model, &p.displacement);
        let (nu, sigma) = snap_analyse(&model, &fixed, &lz, &p.displacement);
        rows.borrow_mut().push(ZRow {
            p: p.clone(),
            w,
            z: lz.committed.get(),
            fell,
            nu,
        });
        let mut rp = rpts.borrow_mut();
        rp.push(RPoint {
            a: w,
            l: p.load_factor,
            sigma,
            nu,
        });
        match read_fold(&rp, Extremum::Min) {
            Some(Ok(r)) => rp.len() > r.j + 2,
            Some(Err(_)) => true,
            None => false,
        }
    };
    let cw: [&dyn Contribution; 1] = [&lz];
    let warm = trace_from(&model, &[], &[], &options, &cw, &start0, &mut stop);
    let rows = rows.into_inner();
    let rpts = rpts.into_inner();
    println!(
        "SZ-W: {} points, {:.2} s, {}, commit error {:?}",
        rows.len(),
        t0.elapsed().as_secs_f64(),
        match &warm {
            Ok(p) => format!("stopped {}, load scale {:e}", p.stopped, p.load_scale),
            Err(e) => format!("lost: {e}"),
        },
        commit_error.borrow()
    );
    let (mut e_l, mut e_z) = (0.0f64, 0.0f64);
    for r in &rows {
        let dl = (r.p.load_factor - lambda_closed(r.w)).abs();
        let zs = z_closed(r.w);
        let dz = (r.z - zs).abs() / zs.abs().max(1.0);
        e_l = if dl.is_finite() { e_l.max(dl) } else { f64::NAN };
        e_z = if dz.is_finite() { e_z.max(dz) } else { f64::NAN };
    }
    ck.at_most("TZ1: max |lambda_j - lambda(w_j)|", e_l, 1e-9);
    ck.at_most("TZ1: max |z_c - z*(w_j)| / max(1, |z*|)", e_z, 1e-12);
    let halvings: usize = rows.iter().map(|r| r.p.halvings).sum();
    let falls = rows.iter().filter(|r| r.fell).count();
    let w_last = rows.last().map(|r| r.w).unwrap_or(f64::NAN);
    ck.equals("TZ2: halvings over the SZ-W points", halvings, 0);
    ck.equals("TZ2: fallbacks in the SZ-W commits", falls, 0);
    ck.holds(
        &format!(
            "TZ2: SZ-W stopped by the rule within 600 points ({} points), the last w {:.6} > w_2, w_c passed",
            rows.len(),
            w_last
        ),
        matches!(&warm, Ok(p) if p.stopped)
            && commit_error.borrow().is_none()
            && w_last > w2
            && rows.iter().any(|r| r.w > w_c),
    );
    for (tag, kind, wf, lf) in [("TZ3", Extremum::Max, w1, l_max), ("TZ4", Extremum::Min, w2, l_min)] {
        match read_fold(&rpts, kind) {
            Some(Ok(r)) => {
                let (in_a, in_l) = contains(&rpts, &r, wf, lf, TAU);
                println!(
                    "{tag}: fold ({kind:?}) between w {:.9} and {:.9} (nu {:?} -> {:?}), lambda in [{:.12}, {:.12}], estimate w {:.9} \
                     lambda {:.12}; closed form w {wf:.9} lambda {lf:.12}",
                    rpts[r.j - 1].a,
                    rpts[r.j].a,
                    rpts[r.j - 1].nu,
                    rpts[r.j].nu,
                    r.lo,
                    r.hi,
                    r.a_est,
                    r.l_est
                );
                ck.holds(&format!("{tag}: the fold read ({kind:?})"), true);
                ck.holds(&format!("{tag}: the closed-form w strictly inside the bracket"), in_a);
                ck.holds(&format!("{tag}: the closed-form lambda in [lo - tau, hi + tau]"), in_l);
            }
            Some(Err(j)) => ck.holds(
                &format!("{tag}: the fold read ({kind:?}): undecided at point {j}"),
                false,
            ),
            None => ck.holds(&format!("{tag}: the fold read ({kind:?}): none"), false),
        }
    }
    let pts: Vec<PathPoint> = rows.iter().map(|r| r.p.clone()).collect();
    let cf = constraint_floor(
        &free,
        (&start0.displacement, 0.0),
        &pts,
        warm.as_ref().map(|p| p.load_scale).unwrap_or(f64::NAN),
    );
    let worst = cf
        .iter()
        .fold(0.0f64, |w, &(e, l)| if e.is_finite() { w.max(e / l) } else { f64::NAN });
    println!(
        "TZ5: the largest e_j / limit_j {worst:.4e}, the largest e_j {:.4e}",
        cf.iter().fold(0.0f64, |w, &(e, _)| w.max(e))
    );
    ck.holds(
        "TZ5: the constraint within its floor at every point",
        !cf.is_empty() && cf.iter().all(|&(e, l)| e.is_finite() && e <= l),
    );
    ck.holds(
        "TZ6: the top node at -lambda d at every point",
        !pts.is_empty()
            && pts.iter().all(|p| p.displacement[top_y] == p.load_factor * -D_TOP)
            && prescribed_follow(&fixed, &pts),
    );
    let mut bad = Vec::new();
    for (k, r) in rows.iter().enumerate() {
        if (r.w - w1).abs() > 0.02 * H && (r.w - w2).abs() > 0.02 * H && r.nu != Some(r.p.negative_pivots) {
            bad.push(format!(
                "point {} w/h {:.4}: {} against {:?}",
                k + 1,
                r.w / H,
                r.p.negative_pivots,
                r.nu
            ));
        }
    }
    ck.holds(
        &format!("TZ7: the driver's negative pivots equal nu {bad:?}"),
        bad.is_empty(),
    );

    // SZ-P: the negative control without the warm start.
    let lp = LocalZ {
        eq: apex_y,
        warm: false,
        committed: Cell::new(0.0),
        fallbacks: Cell::new(0),
    };
    // Amendment 1: every converged w below w_c, the last within 0.01 of it; lost or at the cap; the cause reported.
    let last_p = Cell::new(f64::NAN);
    let max_p = Cell::new(f64::NEG_INFINITY);
    let n_p = Cell::new(0usize);
    let mut stop_p = |p: &PathPoint| {
        let w = apex_w(&model, &p.displacement);
        last_p.set(w);
        max_p.set(if w.is_finite() { max_p.get().max(w) } else { f64::NAN });
        n_p.set(n_p.get() + 1);
        false
    };
    let cp: [&dyn Contribution; 1] = [&lp];
    let plain = trace_from(&model, &[], &[], &options, &cp, &start0, &mut stop_p);
    let (wl, wm) = (last_p.get(), max_p.get());
    let end = match &plain {
        Err(e) => format!("lost: {e}"),
        Ok(p) => format!("{} points, stopped {}", p.points.len(), p.stopped),
    };
    println!(
        "SZ-P: {} converged points, the last w {wl:.12}, the largest w {wm:.12}, w_c {w_c:.12}; {end}",
        n_p.get()
    );
    ck.holds(
        "TZ-P: the plain trace lost or at the cap of 600 points",
        plain.is_err() || matches!(&plain, Ok(p) if !p.stopped && p.points.len() == 600),
    );
    ck.holds(
        &format!("TZ-P: every converged w below w_c (largest {wm:.9}), the last {wl:.9} within 0.01 of w_c {w_c:.9}"),
        n_p.get() > 0 && wm < w_c && w_c - wl <= 0.01,
    );

    // SZ-F: the fallback.
    let lf = LocalZ {
        eq: apex_y,
        warm: true,
        committed: Cell::new(1e3),
        fallbacks: Cell::new(0),
    };
    let mut uf = vec![0.0; dofs.n_equations()];
    uf[apex_y] = -0.05;
    let fell = lf.commit(&uf);
    println!(
        "SZ-F: {:?}, fallbacks {}, z {:.15}, z* {:.15}",
        fell,
        lf.fallbacks.get(),
        lf.committed.get(),
        z_closed(0.05)
    );
    ck.holds(
        "TZ-F: the warm solve from 1e3 falls back and the fallback is counted",
        fell == Ok(true) && lf.fallbacks.get() == 1 && local_newton(0.05, 1e3).is_err(),
    );
    ck.at_most(
        "TZ-F: |z - z*(0.05)|",
        (lf.committed.get() - z_closed(0.05)).abs(),
        1e-12,
    );

    // FW: the driver's carry of the warm modes on a 4^3 block.
    let mesh4 = block_mesh(4);
    let mut mb = block_model(&mesh4, K_L);
    mb.set_incompatible_mode_warm_start(true);
    let mut mb2 = block_model_free_punch(&mesh4, K_L);
    mb2.set_incompatible_mode_warm_start(true);
    let fb4 = FemBranch::new(&mb, &mb2);
    let s0 = St {
        p: 0.0,
        u: vec![0.0; mb.dof_map().n_equations()],
        states: initial_states(&mb).unwrap(),
    };
    let i0 = analyse(&fb4, &s0);
    match i0.vt.as_ref() {
        Some(vt) => {
            let c = dot(vt, vt);
            let o4 = ArcLengthOptions::new(0.1 * (2.0 * c).sqrt())
                .load_scale(c)
                .max_points(3);
            let mut never = |_: &PathPoint| false;
            match trace_from(&mb, &[], &[], &o4, &[], &PathStart::unloaded(&mb).unwrap(), &mut never) {
                Ok(path) => {
                    let mat = neo_hookean();
                    let mut chain = s0.states.clone();
                    let mut ok = true;
                    for p in &path.points {
                        let cm = commit_fem(
                            &mb,
                            &mat,
                            &St {
                                p: p.load_factor,
                                u: p.displacement.clone(),
                                states: chain.clone(),
                            },
                            0,
                        );
                        ok &= cm.failed == 0;
                        chain = cm.s.states;
                    }
                    println!(
                        "FW: {} points, lambda {:?}, load scale {c:e}",
                        path.points.len(),
                        path.points.iter().map(|p| p.load_factor).collect::<Vec<_>>()
                    );
                    ck.holds(
                        "FW: 3 points; every committed state of nine values",
                        path.points.len() == 3 && path.states.iter().all(|s| s.values().len() == MODES),
                    );
                    ck.holds(
                        "FW: the driver's states at point 3 equal the test-side commit chain (bit for bit)",
                        ok && same_states(&path.states, &chain),
                    );
                }
                Err(e) => ck.holds(&format!("FW: the 4^3 warm trace ({e})"), false),
            }
        }
        None => ck.holds("FW: the tangent at the unloaded 4^3 block", false),
    }
    ck.finish();
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
/// ADR 0724's fold (`f843-scratch/run2-heavy.log`).
const A_F: f64 = 2.170071672061e-4;
const LAMBDA_F: f64 = 0.958472086948445;
/// D2: the step in the amplitude and the cap.
const DA_T: f64 = 2e-7;
const CAP: usize = 12;

/// A point of the heavy trace with its commit, its analysis and element 3720's plain iteration from 0.
struct HRow {
    p: PathPoint,
    pt: APoint,
    fallbacks: usize,
    failed: usize,
    cold: String,
}

#[test]
#[ignore = "ADR 0728: the heavy record on 16^3 (D1 to D3), run once with --ignored (131.8 s), every row met: rung 5 rebuilt and committed on the warm model bit-identical; trace_from from rung 5 on the warm model, 6 points of 4 to 5 iterations, no halving, no local fallback in 4096 elements, the driver's states equal the test-side commit chain bit for bit; the fold read between a 2.167379e-4 (nu 1) and 2.171069e-4 (nu 0), containing ADR 0724's a_f 2.170072e-4 and lambda_f 0.958472086948 (bracket [0.958472072691, 0.958472092233]); the bordered reference on the warm model to 4.7e-14; the constraint 7.7e-10 within its floor 4.6e-8; f845_fast covers the same library path"]
fn f845_nh16_fold_heavy() {
    let mut ch = Checks::new();
    let t_all = Instant::now();
    let mesh16 = block_mesh(16);
    let m = block_model(&mesh16, K_L);
    let m2 = block_model_free_punch(&mesh16, K_L);
    let states = initial_states(&m).unwrap();
    let fb = FemBranch::new(&m, &m2);
    let free = fb.free.clone();
    let ndof = m.dof_map().n_equations();
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
    let Some(vt_b) = ib.vt.clone() else {
        ch.holds("D2: the tangent at the B state", false);
        ch.finish();
        return;
    };

    // D1: ADR 0711's rungs 1 to 5 (plain).
    let atr = amp_trace(&fb, "AT", (sb.clone(), ib.clone()), &phi, 5, (0.5, 1.2));
    let rungs_ok = atr.points.len() == 6 && (1..=5).all(|k| atr.points[k].rung == Some(k));
    ch.holds("S5: rungs 1 to 5 accepted", rungs_ok);
    if !rungs_ok {
        ch.finish();
        return;
    }
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
    let c5 = commit_fem(&mw, &mat, &s5.s, E_W);
    println!(
        "D1: S5 committed on the warm model: {} fallbacks (element {E_W}: {}), {} failed, CW {}, {:.1} s",
        c5.fallbacks,
        c5.watched,
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
    let Some(vt5) = s5w.info.vt.clone() else {
        ch.holds("D1: the tangent at S5_W", false);
        ch.finish();
        return;
    };

    // D1, D2: the start, the load scale and the arc.
    let c = dot(&vt_b, &vt_b);
    let sigma5 = s5w.sigma;
    let w5: Vec<f64> = vt5.iter().map(|x| sigma5 * x).collect();
    let ds = DA_T * (dot(&w5, &w5) + c * sigma5 * sigma5).sqrt();
    println!(
        "D2: c = |v_t(B)|^2 {c:.6e}, |v_t(S5_W)| {:.6e}, |w_5| {:.6e}, sigma_5 {sigma5:+.6e}, Ds {ds:.6e}",
        norm(vt5.iter().copied()),
        norm(w5.iter().copied())
    );
    let mut du0 = expand(&free, ndof, &vt5.iter().map(|x| -x).collect::<Vec<f64>>());
    for &(eq, v) in &fbw.fixed_eqs {
        du0[eq] = -v;
    }
    let start5 = PathStart::new(s5w.s.p, s5w.s.u.clone(), s5w.s.states.clone()).increment(du0, -1.0);
    let o = ArcLengthOptions::new(ds)
        .load_scale(c)
        .max_points(CAP)
        .newton(NewtonOptions::new(1).threads(4));
    let r0 = RPoint {
        a: s5w.a,
        l: s5w.s.p,
        sigma: s5w.sigma,
        nu: s5w.info.nu,
    };
    let rows: RefCell<Vec<HRow>> = RefCell::new(Vec::new());
    let rpts: RefCell<Vec<RPoint>> = RefCell::new(vec![r0]);
    let stop_why: RefCell<String> = RefCell::new(String::new());
    let clock = RefCell::new(Instant::now());
    let mut stop = |p: &PathPoint| {
        let driver_s = clock.borrow().elapsed().as_secs_f64();
        let t1 = Instant::now();
        let prev = rows
            .borrow()
            .last()
            .map(|r| r.pt.s.states.clone())
            .unwrap_or_else(|| s5w.s.states.clone());
        let st = St {
            p: p.load_factor,
            u: p.displacement.clone(),
            states: prev,
        };
        let cm = commit_fem(&mw, &mat, &st, E_W);
        let info = analyse(&fbw, &cm.s);
        let pt = apoint(&fbw, cm.s, info, &phi, None);
        let el = element_of(&mw, E_W, &pt.s.u, &pt.s.states);
        let cold = match (committed_modes(&pt.s.states[E_W]), record_from(&mat, &el, None).1) {
            (Some(alpha), Ok(a0)) => format!("same ({:.1e})", rel_dist(&alpha, &a0)),
            (_, Err(t)) => format!("fails ({t})"),
            (None, Ok(_)) => "no committed modes".to_string(),
        };
        let n = rows.borrow().len() + 1;
        println!(
            "HR point {n}: a {:+.12e}, lambda {:.12}, sigma {:+.6e}, nu {:?}, driver negative pivots {}, length {:.6e}, halvings {}, \
             {} iterations, fallbacks {}, failed {}, element {E_W} plain from 0: {cold}; driver {driver_s:.1} s, test {:.1} s",
            pt.a,
            pt.s.p,
            pt.sigma,
            pt.info.nu,
            p.negative_pivots,
            p.length,
            p.halvings,
            p.residuals.len() - 1,
            cm.fallbacks,
            cm.failed,
            t1.elapsed().as_secs_f64()
        );
        let mut rp = rpts.borrow_mut();
        let pa = rp.last().unwrap().a;
        rp.push(RPoint {
            a: pt.a,
            l: pt.s.p,
            sigma: pt.sigma,
            nu: pt.info.nu,
        });
        let why = match read_fold(&rp, Extremum::Min) {
            Some(Ok(r)) if rp.len() > r.j + 4 => Some("the fourth point after the fold's bracket"),
            Some(Err(_)) => Some("undecided"),
            _ if pt.a <= pa => Some("a fall-back"),
            _ if !(0.95..=0.97).contains(&pt.s.p) => Some("lambda outside [0.95, 0.97]"),
            _ => None,
        };
        rows.borrow_mut().push(HRow {
            p: p.clone(),
            pt,
            fallbacks: cm.fallbacks,
            failed: cm.failed,
            cold,
        });
        *clock.borrow_mut() = Instant::now();
        if let Some(w) = why {
            *stop_why.borrow_mut() = w.to_string();
            return true;
        }
        false
    };
    let t_tr = Instant::now();
    let tr = trace_from(&mw, &[], &[], &o, &[], &start5, &mut stop);
    let rows = rows.into_inner();
    let rpts = rpts.into_inner();
    let end = match &tr {
        Ok(p) if p.stopped => format!("stopped: {}", stop_why.borrow()),
        Ok(p) => format!("the cap of {CAP} points ({} points)", p.points.len()),
        Err(e) => format!("lost: {e}"),
    };
    println!(
        "HR: the end of the trace: {end}; {} points, {:.1} s",
        rows.len(),
        t_tr.elapsed().as_secs_f64()
    );

    // HA0.
    ch.holds(
        "HA0: the first point with a > a_5 and lambda < lambda_5",
        rows.first().is_some_and(|r| r.pt.a > s5w.a && r.pt.s.p < s5w.s.p),
    );

    // HF1 to HF4: the reading.
    let fold = read_fold(&rpts, Extremum::Min);
    let jb = match fold {
        Some(Ok(r)) => {
            let (in_a, in_l) = contains(&rpts, &r, A_F, LAMBDA_F, TAU);
            println!(
                "HR READING: fold between a {:.12e} and {:.12e} (nu {:?} -> {:?}, sigma {:+.6e} -> {:+.6e}), lambda_f in [{:.15}, \
                 {:.15}], estimate a {:.12e} lambda {:.15}; ADR 0724: a_f {A_F:e} lambda_f {LAMBDA_F}; estimate against it: a \
                 {:+.4e} relative, lambda {:+.4e}",
                rpts[r.j - 1].a,
                rpts[r.j].a,
                rpts[r.j - 1].nu,
                rpts[r.j].nu,
                rpts[r.j - 1].sigma,
                rpts[r.j].sigma,
                r.lo,
                r.hi,
                r.a_est,
                r.l_est,
                r.a_est / A_F - 1.0,
                r.l_est - LAMBDA_F
            );
            ch.holds("HF1: the fold read", true);
            ch.holds("HF2: a_f strictly inside the a bracket", in_a);
            ch.holds(
                &format!(
                    "HF3: lambda_f in [lo - tau, hi + tau] (strictly in [lo, hi]: {})",
                    LAMBDA_F >= r.lo && LAMBDA_F <= r.hi
                ),
                in_l,
            );
            Some(r.j)
        }
        Some(Err(j)) => {
            ch.holds(&format!("HF1: the fold read: undecided at point {j}"), false);
            None
        }
        None => {
            ch.holds("HF1: the fold read: none", false);
            None
        }
    };
    let increasing = rpts.windows(2).all(|w| w[1].a > w[0].a);
    let hf4 = jb.is_some_and(|jb| {
        rpts.len() > jb + 3
            && rpts.iter().enumerate().all(|(i, q)| {
                if i < jb {
                    q.sigma < 0.0 && q.nu == Some(1)
                } else {
                    q.sigma > 0.0 && q.nu == Some(0)
                }
            })
    });
    ch.holds(
        "HF4: a strictly increasing; sigma < 0, nu 1 before the bracket's second point, sigma > 0, nu 0 from it on (3 or more after)",
        increasing && hf4,
    );

    // HS1, HL1, HL2, HN.
    ch.holds(
        &format!("HS1: stopped by the rule within {CAP} points ({end})"),
        matches!(&tr, Ok(p) if p.stopped) && stop_why.borrow().as_str() == "the fourth point after the fold's bracket",
    );
    ch.equals(
        "HS1: halvings over the points",
        rows.iter().map(|r| r.p.halvings).sum::<usize>(),
        0,
    );
    let its_max = rows.iter().map(|r| r.p.residuals.len() - 1).max().unwrap_or(usize::MAX);
    ch.holds(
        &format!("HS1: iterations at every point <= 6 (largest {its_max})"),
        its_max <= 6,
    );
    ch.equals(
        "HL1: fallbacks over the commits",
        rows.iter().map(|r| r.fallbacks).sum::<usize>(),
        0,
    );
    ch.equals(
        "HL1: failed elements over the commits",
        rows.iter().map(|r| r.failed).sum::<usize>(),
        0,
    );
    ch.holds(
        "HL2: the driver's committed states at the last point equal the test-side commit chain (bit for bit)",
        match (&tr, rows.last()) {
            (Ok(p), Some(r)) => same_states(&p.states, &r.pt.s.states),
            _ => false,
        },
    );
    ch.holds(
        "HN: the driver's negative pivots equal nu at every point",
        !rows.is_empty() && rows.iter().all(|r| r.pt.info.nu == Some(r.p.negative_pivots)),
    );

    // HB: the bordered reference on the warm model.
    let (mut worst_l, mut worst_u) = (0.0f64, 0.0f64);
    let mut hb_ok = !rows.is_empty();
    for j in 0..rows.len() {
        let prev = if j == 0 { &s5w } else { &rows[j - 1].pt };
        let a_j = rows[j].pt.a;
        let t0 = Instant::now();
        let Some(vt) = prev.info.vt.as_ref() else {
            hb_ok = false;
            println!("HB point {}: the previous tangent refused", j + 1);
            continue;
        };
        let dp = (a_j - prev.a) / dot(&phi, vt);
        let mut up = prev.s.clone();
        for (&q, v) in free.iter().zip(vt) {
            up.u[q] += dp * v;
        }
        let pred = fbw.at_load(&up, prev.s.p + dp);
        let ctl = Control {
            g: phi.clone(),
            h: 0.0,
            c: a_j,
        };
        match bordered(&fbw, &pred, &ctl) {
            Ok((s, its)) => {
                let p = &rows[j].p;
                let dl = (p.load_factor - s.p).abs();
                let du = max_abs(p.displacement.iter().zip(&s.u).map(|(x, y)| x - y)) / max_abs(s.u.iter().copied());
                println!(
                    "HB point {}: a {a_j:+.12e}, lambda {:.15} against the bordered {:.15} ({its} iterations, {:.1} s): |Dlambda| \
                     {dl:.3e}, |Du| / |u| {du:.3e}",
                    j + 1,
                    p.load_factor,
                    s.p,
                    t0.elapsed().as_secs_f64()
                );
                worst_l = if dl.is_finite() { worst_l.max(dl) } else { f64::NAN };
                worst_u = if du.is_finite() { worst_u.max(du) } else { f64::NAN };
            }
            Err(e) => {
                hb_ok = false;
                println!("HB point {}: the bordered reference fails: {e}", j + 1);
            }
        }
    }
    ch.holds("HB: the bordered reference converged at every point", hb_ok);
    ch.at_most("HB: max |lambda_j - lambda^B_j|", worst_l, 1e-8);
    ch.at_most("HB: max |u_j - u^B_j|_inf / |u^B_j|_inf", worst_u, 1e-7);

    // HC, HP.
    let pts: Vec<PathPoint> = rows.iter().map(|r| r.p.clone()).collect();
    let cf = constraint_floor(&free, (&s5w.s.u, s5w.s.p), &pts, c);
    for (j, &(e, l)) in cf.iter().enumerate() {
        println!("HC point {}: e {e:.4e}, limit {l:.4e}", j + 1);
    }
    ch.holds(
        "HC: the constraint within its floor at every point",
        !cf.is_empty() && cf.iter().all(|&(e, l)| e.is_finite() && e <= l),
    );
    ch.holds(
        "HP: the punch equations at lambda v at every point",
        !pts.is_empty() && prescribed_follow(&fbw.fixed_eqs, &pts),
    );
    println!(
        "HR: rows {}, element {E_W} plain from 0: {:?}",
        rows.len(),
        rows.iter().map(|r| r.cold.as_str()).collect::<Vec<_>>()
    );
    println!("f845_nh16_fold_heavy: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}
