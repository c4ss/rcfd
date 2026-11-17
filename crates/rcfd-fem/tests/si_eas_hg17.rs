//! F-789 (ADR 0711): the subcritical branch of rule L's 16^3 punch path past 0.9645. From the state of the bifurcated branch
//! of ADR 0700 at 0.9645 (reproduced by the switch of ADR 0700 D3): the load-controlled trace toward lower loads with the
//! continuity test of ADR 0697 D1 and a bound on the change of the amplitude along the critical mode (D2); the
//! amplitude-controlled trace by a bordered Newton (the substitute for the arc-length driver, D1, D3) to its fold if it folds,
//! with the fold bracketed and bounded; the inertia, `R/mu` and `w_c` along both against the symmetric branch and F-bar. The
//! procedures are written once over a branch case and run by the fast test on the closed-form subcritical pitchfork of ADR
//! 0700, on a step that falls back onto the symmetric branch, on a closed-form subcritical branch with a fold and on the stable
//! `8^3` block (D5). The helpers are copies of those of `si_eas_hg16.rs` (existing tests are not edited). Every aggregated
//! value is required finite before it is compared.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{prescribed_equations, reduce, Prescribed};
use rcfd_fem::element::ElementState;
use rcfd_fem::material::HyperelasticEnergy;
use rcfd_fem::nl_incompatible::IncompatibleStabilisation;
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
const FBAR: SolidFormulation = SolidFormulation::BBar;
const OFF: IncompatibleStabilisation = IncompatibleStabilisation::Off;
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
const K_FBAR: Kind = Kind {
    name: "Fbar",
    formulation: FBAR,
    rule: OFF,
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

/// D1: the continuity test of a solve `s` from `from` (with its tangent `vt`).
fn continuous(from: &St, free: &[usize], vt: &[f64], s: &St) -> bool {
    let d = s.p - from.p;
    let pred = max_abs(vt.iter().map(|v| d * v));
    let corr = max_abs(free.iter().zip(vt).map(|(&q, v)| s.u[q] - from.u[q] - d * v));
    corr.is_finite() && pred.is_finite() && corr <= pred
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

fn expand(free: &[usize], n: usize, x: &[f64]) -> Vec<f64> {
    let mut v = vec![0.0; n];
    for (&e, &xi) in free.iter().zip(x) {
        v[e] = xi;
    }
    v
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

/// PT of ADR 0700 D5 generalised (ADR 0711 D5): `r_0 = (1 - p) u_0 + b3 u_0^3 + b5 u_0^5 + 2 c u_0 u_1`, `r_1 = 2 u_1 - 0.1 p
/// + c u_0^2`, `r_i = k_i u_i - p sb b_i` (`i = 2..7`). PT-U: `b3 = 0.5`, `b5 = 0`, `sb = 1` (ADR 0700's subcritical toy).
struct Pitch {
    b3: f64,
    b5: f64,
    sb: f64,
}

fn k_i(i: usize) -> f64 {
    1.0 + i as f64
}

fn b_i(i: usize) -> f64 {
    0.1 * i as f64
}

impl Pitch {
    fn residual(&self, u: &[f64], p: f64) -> Vec<f64> {
        (0..TOY_N)
            .map(|i| match i {
                0 => (1.0 - p) * u[0] + self.b3 * u[0].powi(3) + self.b5 * u[0].powi(5) + 2.0 * PT_C * u[0] * u[1],
                1 => 2.0 * u[1] - 0.1 * p + PT_C * u[0] * u[0],
                _ => k_i(i) * u[i] - p * self.sb * b_i(i),
            })
            .collect()
    }
    fn dense(&self, u: &[f64], p: f64) -> Vec<f64> {
        let mut a = vec![0.0; TOY_N * TOY_N];
        a[0] = 1.0 - p + 3.0 * self.b3 * u[0] * u[0] + 5.0 * self.b5 * u[0].powi(4) + 2.0 * PT_C * u[1];
        a[1] = 2.0 * PT_C * u[0];
        a[TOY_N] = 2.0 * PT_C * u[0];
        a[TOY_N + 1] = 2.0;
        for i in 2..TOY_N {
            a[TOY_N * i + i] = k_i(i);
        }
        a
    }
    /// Newton at the fixed load `p` with the force `eta g` from `start` (20 iterations, `|r - eta g|_inf <= 1e-14`).
    fn newton(&self, start: &[f64], p: f64, eta: f64, g: &[f64]) -> Result<Vec<f64>, String> {
        let mut u = start.to_vec();
        for it in 0..=20 {
            let r: Vec<f64> = self
                .residual(&u, p)
                .iter()
                .zip(g)
                .map(|(ri, gi)| ri - eta * gi)
                .collect();
            if r.iter().chain(&u).any(|v| !v.is_finite()) {
                return Err(format!("not finite at iteration {it}"));
            }
            if max_abs(r.iter().copied()) <= 1e-14 {
                return Ok(u);
            }
            if it == 20 {
                break;
            }
            let du = dense_solve(self.dense(&u, p), r.iter().map(|v| -v).collect(), TOY_N)
                .ok_or_else(|| "singular".to_string())?;
            for (ui, d) in u.iter_mut().zip(&du) {
                *ui += d;
            }
        }
        Err("not converged in 20 iterations".to_string())
    }
    /// The branch state at `p` from the closed form (the small root of `mu + beta a^2 + b5 a^4 = 0`, `a > 0`), polished by the
    /// toy's Newton.
    fn branch(&self, p: f64) -> St {
        let beta = self.b3 - PT_C * PT_C;
        let mu = 1.0 - 0.9 * p;
        let a2 = if self.b5 == 0.0 {
            -mu / beta
        } else {
            (-beta - (beta * beta - 4.0 * self.b5 * mu).sqrt()) / (2.0 * self.b5)
        };
        let u: Vec<f64> = (0..TOY_N)
            .map(|i| match i {
                0 => a2.sqrt(),
                1 => (0.1 * p - PT_C * a2) / 2.0,
                _ => p * self.sb * b_i(i) / k_i(i),
            })
            .collect();
        St {
            p,
            u: self.newton(&u, p, 0.0, &[0.0; TOY_N]).unwrap(),
            states: Vec::new(),
        }
    }
    /// The closed-form load of the branch at the amplitude `a`: `(1 + beta a^2 + b5 a^4) / 0.9`.
    fn load_of(&self, a: f64) -> f64 {
        (1.0 + (self.b3 - PT_C * PT_C) * a * a + self.b5 * a.powi(4)) / 0.9
    }
}

impl Case for Pitch {
    fn solve(&self, from: &St, to: f64) -> Result<St, String> {
        self.newton(&from.u, to, 0.0, &[0.0; TOY_N]).map(|u| St {
            p: to,
            u,
            states: Vec::new(),
        })
    }
    fn system(&self, s: &St) -> Sys {
        let f = (0..TOY_N)
            .map(|i| match i {
                0 => s.u[0],
                1 => 0.1,
                _ => self.sb * b_i(i),
            })
            .collect();
        Sys {
            k: csr_of_dense(&self.dense(&s.u, s.p), TOY_N),
            f,
            free: (0..TOY_N).collect(),
        }
    }
    fn lowest_pair(&self, s: &Sys, _nu: usize) -> Option<Pair> {
        dense_lowest(&s.k)
    }
}

impl Branch for Pitch {
    fn fixed(&self, start: &St, p: f64, eta: f64, g: &[f64]) -> Result<St, String> {
        self.newton(&start.u, p, eta, g).map(|u| St {
            p,
            u,
            states: Vec::new(),
        })
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

impl Bordered for Pitch {
    fn residual(&self, s: &St) -> Result<(Vec<f64>, f64), String> {
        let r = Pitch::residual(self, &s.u, s.p);
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

// ============================================================================ D2: the amended continuity test and the trace

const THETA_A: f64 = 0.5;

/// D2: the two tests of a step `s` from `from` (with its tangent `vt`): the test of ADR 0697 D1, the amplitude bound, `Da`,
/// `Da_pred`.
fn amended(from: &St, free: &[usize], vt: &[f64], phi: &[f64], s: &St) -> (bool, bool, f64, f64) {
    let old = continuous(from, free, vt, s);
    let d = s.p - from.p;
    let da = amp(phi, free, &s.u) - amp(phi, free, &from.u);
    let da_pred = d * dot(phi, vt);
    let ok = (da - da_pred).abs() <= THETA_A * da_pred.abs();
    (old, ok && da.is_finite() && da_pred.is_finite(), da, da_pred)
}

/// One attempt of the D2 trace.
struct LAttempt {
    to: f64,
    converged: bool,
    a: f64,
    old: bool,
    bound: bool,
}

struct LTrace {
    points: Vec<(St, Info)>,
    attempts: Vec<LAttempt>,
    lost: Option<f64>,
}

const TRACE_HALVINGS: usize = 4;
const TRACE_CAP: usize = 28;

/// D2: the branch from `from` down the grid by the fixed-load Newton, each step accepted if it converges and passes both
/// tests, halved at most `TRACE_HALVINGS` times per grid load, `TRACE_CAP` solves.
fn trace_down<B: Branch>(case: &B, tag: &str, from: (St, Info), grid: &[f64], phi: &[f64]) -> LTrace {
    let free = case.free();
    let zero = vec![0.0; free.len()];
    let mut cur = from;
    let mut out = LTrace {
        points: Vec::new(),
        attempts: Vec::new(),
        lost: None,
    };
    let mut solves = 0;
    for &target in grid {
        let mut tgt = target;
        let mut halvings = 0;
        loop {
            if solves >= TRACE_CAP {
                println!("{tag} trace: the cap of {TRACE_CAP} solves reached at {:.12}", cur.0.p);
                out.lost = Some(cur.0.p);
                return out;
            }
            solves += 1;
            let t0 = Instant::now();
            let res = case.fixed(&cur.0, tgt, 0.0, &zero);
            let (acc, text, att) = match (res, cur.1.vt.as_ref()) {
                (Ok(s), Some(vt)) => {
                    let (old, bound, da, dap) = amended(&cur.0, &free, vt, phi, &s);
                    let a = amp(phi, &free, &s.u);
                    let text = format!(
                        "a {a:+.9e}, D1 {old}, bound {bound} (Da {da:+.6e}, Da_pred {dap:+.6e}, |Da - Da_pred| / |Da_pred| \
                         {:.4e})",
                        (da - dap).abs() / dap.abs()
                    );
                    let att = LAttempt {
                        to: tgt,
                        converged: true,
                        a,
                        old,
                        bound,
                    };
                    (if old && bound { Some(s) } else { None }, text, att)
                }
                (Ok(_), None) => (
                    None,
                    "the previous factor refused".to_string(),
                    LAttempt {
                        to: tgt,
                        converged: true,
                        a: f64::NAN,
                        old: false,
                        bound: false,
                    },
                ),
                (Err(e), _) => (
                    None,
                    format!("fails: {e}"),
                    LAttempt {
                        to: tgt,
                        converged: false,
                        a: f64::NAN,
                        old: false,
                        bound: false,
                    },
                ),
            };
            println!(
                "{tag} trace solve {solves}: {:.12} -> {tgt:.12}: {} {text}, {:.1} s",
                cur.0.p,
                if acc.is_some() { "accepted" } else { "rejected" },
                t0.elapsed().as_secs_f64()
            );
            out.attempts.push(att);
            match acc {
                Some(s) => {
                    let info = analyse(case, &s);
                    cur = (s, info);
                    if tgt == target {
                        out.points.push(cur.clone());
                        break;
                    }
                    tgt = target;
                }
                None => {
                    if halvings == TRACE_HALVINGS {
                        println!("{tag} trace: lost after {:.12}", cur.0.p);
                        out.lost = Some(cur.0.p);
                        return out;
                    }
                    halvings += 1;
                    tgt = cur.0.p + 0.5 * (tgt - cur.0.p);
                }
            }
        }
    }
    out
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

/// D3: the cubic Hermite interpolant of `lambda(a)` through the accepted states, at `a` (`None` outside their range).
fn hermite(points: &[APoint], a: f64) -> Option<f64> {
    points.windows(2).find_map(|w| {
        let (p0, p1) = (&w[0], &w[1]);
        if !(p0.a <= a && a <= p1.a) {
            return None;
        }
        let h = p1.a - p0.a;
        let t = (a - p0.a) / h;
        let (t2, t3) = (t * t, t * t * t);
        Some(
            (2.0 * t3 - 3.0 * t2 + 1.0) * p0.s.p
                + (t3 - 2.0 * t2 + t) * h * p0.sigma
                + (-2.0 * t3 + 3.0 * t2) * p1.s.p
                + (t3 - t2) * h * p1.sigma,
        )
    })
}

// ============================================================================ the fast test

#[test]
fn f789_fast() {
    let mut ch = Checks::new();
    let t_all = Instant::now();

    // PU: the subcritical pitchfork of ADR 0700.
    let pu = Pitch {
        b3: 0.5,
        b5: 0.0,
        sb: 1.0,
    };
    let phi_e0: Vec<f64> = (0..TOY_N).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();
    let b1 = pu.branch(1.0);
    let ib1 = analyse(&pu, &b1);
    let lt = trace_down(&pu, "PU-a", (b1.clone(), ib1.clone()), &[0.95, 0.9, 0.85, 0.8], &phi_e0);
    ch.holds(
        &format!("PU-a: every step accepted ({} of 4)", lt.points.len()),
        lt.lost.is_none() && lt.points.len() == 4 && lt.attempts.len() == 4,
    );
    let mut prev = b1.u[0];
    for (s, i) in &lt.points {
        let exact = ((1.0 - 0.9 * s.p) / 0.5).sqrt();
        ch.at_most(
            &format!("PU-a: a({}) against the closed form", s.p),
            (s.u[0] / exact - 1.0).abs(),
            1e-10,
        );
        ch.equals(&format!("PU-a: nu at {}", s.p), i.nu.unwrap_or(usize::MAX), 1);
        ch.holds(&format!("PU-a: a increasing at {}", s.p), s.u[0] > prev);
        prev = s.u[0];
    }
    let at = amp_trace(
        &pu,
        "PU-b",
        (b1.clone(), ib1.clone()),
        &phi_e0,
        2,
        (f64::NEG_INFINITY, f64::INFINITY),
    );
    ch.holds(
        &format!("PU-b: both rungs accepted ({} states)", at.points.len()),
        at.points.len() == 3 && at.points[1..].iter().all(|p| p.rung.is_some()),
    );
    for p in &at.points[1..] {
        ch.at_most(
            &format!("PU-b: lambda at a {:.6} against the closed form", p.a),
            (p.s.p - pu.load_of(p.a)).abs(),
            1e-12,
        );
        ch.equals(&format!("PU-b: nu at a {:.6}", p.a), p.info.nu.unwrap_or(usize::MAX), 1);
    }
    ch.holds(
        &format!("PU-b: reading 'no fold' ({})", at.reading),
        at.reading.starts_with("no fold"),
    );
    let mut inside = 0;
    for (s, _) in &lt.points {
        if let Some(lh) = hermite(&at.points, s.u[0]) {
            inside += 1;
            ch.at_most(
                &format!("PU-c: |lambda_H - lambda| at {}", s.p),
                (lh - s.p).abs(),
                1e-12,
            );
        }
    }
    ch.holds(&format!("PU-c: at least 3 states inside ({inside})"), inside >= 3);

    // PF: the fall-back.
    let pf = Pitch {
        b3: 0.5,
        b5: 0.0,
        sb: 100.0,
    };
    let f1 = pf.branch(1.0);
    let if1 = analyse(&pf, &f1);
    match (pf.fixed(&f1, 0.8, 0.0, &[0.0; TOY_N]), if1.vt.as_ref()) {
        (Ok(s), Some(vt)) => {
            let (old, bound, da, dap) = amended(&f1, &pf.free(), vt, &phi_e0, &s);
            println!(
                "PF-a: the step to 0.8: u_0 {:+.6e}, D1 {old}, bound {bound}, Da {da:+.6e}, Da_pred {dap:+.6e}",
                s.u[0]
            );
            ch.at_most("PF-a: |u_0| of the step (the symmetric root)", s.u[0].abs(), 1e-12);
            ch.holds("PF-a: the test of ADR 0697 D1 accepts it", old);
            ch.holds("PF-a: the amended test rejects it", !bound);
        }
        _ => ch.holds("PF-a: the step to 0.8 converged", false),
    }
    let ft = trace_down(&pf, "PF-b", (f1, if1), &[0.8], &phi_e0);
    let rejected = ft.attempts.iter().filter(|a| !(a.old && a.bound)).count();
    ch.holds("PF-b: 0.8 reached", ft.lost.is_none() && ft.points.len() == 1);
    ch.holds(&format!("PF-b: rejected attempts >= 1 ({rejected})"), rejected >= 1);
    match ft.points.last() {
        Some((s, i)) if s.p == 0.8 => {
            ch.at_most(
                "PF-b: a(0.8) against sqrt(0.56)",
                (s.u[0] / 0.56f64.sqrt() - 1.0).abs(),
                1e-10,
            );
            ch.equals("PF-b: nu at 0.8", i.nu.unwrap_or(usize::MAX), 1);
        }
        _ => ch.holds("PF-b: the state at 0.8", false),
    }
    for a in &ft.attempts {
        println!(
            "PF-b attempt to {:.6}: converged {}, a {:+.6e}, D1 {}, bound {}",
            a.to, a.converged, a.a, a.old, a.bound
        );
    }

    // P5: a closed-form fold.
    let p5 = Pitch {
        b3: 0.5,
        b5: 0.25,
        sb: 1.0,
    };
    let c1 = p5.branch(1.0);
    let ic1 = analyse(&p5, &c1);
    let a5 = amp_trace(&p5, "P5", (c1, ic1), &phi_e0, 5, (f64::NEG_INFINITY, f64::INFINITY));
    ch.holds(
        &format!("P5: all 5 rungs accepted ({} states)", a5.points.len()),
        a5.points.len() == 6 && a5.points.iter().all(|p| p.rung.is_some()),
    );
    for p in &a5.points[1..] {
        ch.at_most(
            &format!("P5: lambda at a {:.6} against the closed form", p.a),
            (p.s.p - p5.load_of(p.a)).abs(),
            1e-12,
        );
    }
    ch.holds(
        &format!("P5: reading 'fold' ({})", a5.reading),
        a5.reading.starts_with("fold"),
    );
    let pfold = 0.75 / 0.9;
    match &a5.fold {
        Some(f) => {
            let (p0, p1) = (&a5.points[f.k - 1], &a5.points[f.k]);
            ch.holds(
                &format!("P5: a = 1 in [{:.6}, {:.6}]", p0.a, p1.a),
                p0.a <= 1.0 && 1.0 <= p1.a,
            );
            ch.holds(
                &format!("P5: p_f {pfold:.9} in [L, U] = [{:.9}, {:.9}]", f.lo, f.hi),
                f.lo <= pfold && pfold <= f.hi,
            );
            ch.at_most("P5: |lambda_f^ - p_f|", (f.l_est - pfold).abs(), 5e-3);
            ch.equals("P5: nu before the fold", p0.info.nu.unwrap_or(usize::MAX), 1);
            ch.equals("P5: nu after the fold", p1.info.nu.unwrap_or(usize::MAX), 0);
        }
        None => ch.holds("P5: the fold bracketed", false),
    }

    // N8: the bordered Newton on the stable 8^3 block.
    let mesh8 = block_mesh(8);
    let m8 = block_model(&mesh8, K_L);
    let m8f = block_model_free_punch(&mesh8, K_L);
    let fb8 = FemBranch::new(&m8, &m8f, 8);
    let (_, log8) = solve_observed(&mesh8, "block 8^3", K_L, &options(10));
    let last = log8.attempts.last().unwrap();
    let lo = St {
        p: last.committed,
        u: last.iterates[0].clone(),
        states: initial_states(&m8).unwrap(),
    };
    match fb8.solve(&lo, last.attempted) {
        Ok(s1) => {
            let zero = vec![0.0; fb8.free.len()];
            match fb8.fixed(&s1, 0.99, 0.0, &zero) {
                Ok(sf) => {
                    let scale = max_abs(sf.u.iter().copied());
                    let load = Control {
                        g: vec![0.0; fb8.free.len()],
                        h: 1.0,
                        c: 0.99,
                    };
                    match bordered(&fb8, &s1, &load) {
                        Ok((s, its)) => {
                            println!("N8a: load control, {its} iterations, lambda {:.17}", s.p);
                            ch.at_most(
                                "N8a: |u - u_fixed|_inf / |u_fixed|_inf",
                                max_abs(s.u.iter().zip(&sf.u).map(|(x, y)| x - y)) / scale,
                                1e-8,
                            );
                            ch.at_most("N8a: |lambda - 0.99|", (s.p - 0.99).abs(), 1e-14);
                        }
                        Err(e) => ch.holds(&format!("N8a: the load-controlled Newton ({e})"), false),
                    }
                    let corner = m8.mesh().node_set("corner").unwrap()[0];
                    let eq = m8.dof_map().equation(corner, 2).unwrap();
                    let kq = fb8.free.iter().position(|&q| q == eq).unwrap();
                    let mut g = vec![0.0; fb8.free.len()];
                    g[kq] = 1.0;
                    let wc = Control { g, h: 0.0, c: sf.u[eq] };
                    match bordered(&fb8, &s1, &wc) {
                        Ok((s, its)) => {
                            println!("N8b: corner control, {its} iterations, lambda {:.17}", s.p);
                            ch.at_most(
                                "N8b: |u - u_fixed|_inf / |u_fixed|_inf",
                                max_abs(s.u.iter().zip(&sf.u).map(|(x, y)| x - y)) / scale,
                                1e-8,
                            );
                            ch.at_most("N8b: |lambda - 0.99|", (s.p - 0.99).abs(), 1e-8);
                        }
                        Err(e) => ch.holds(&format!("N8b: the corner-controlled Newton ({e})"), false),
                    }
                }
                Err(e) => ch.holds(&format!("N8: the fixed-load reference at 0.99 ({e})"), false),
            }
        }
        Err(e) => ch.holds(&format!("N8: the solve to 1.0 ({e})"), false),
    }
    println!("f789_fast: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}

// ============================================================================ the heavy record

const LF: f64 = 0.9647;
const LB: f64 = 0.9645;
/// ADR 0700's `a*_pred` (the `Debug` form of its log line 81).
const EPS_SWITCH: f64 = 2.7468713663317314e-5;
const LAMBDA_C: f64 = 0.964900029325;
const ALPHA: f64 = -4.367713e6;
const BETA: f64 = -1.157900e12;

fn a_red(lambda: f64) -> f64 {
    (ALPHA.abs() * (LAMBDA_C - lambda) / BETA.abs()).sqrt()
}

/// The fixed-load Newton from `from` to `p` in equal substeps of at most `0.02`.
fn chain_to<B: Branch>(case: &B, from: &St, p: f64) -> Result<St, String> {
    let zero = vec![0.0; case.free().len()];
    let n = ((p - from.p).abs() / 0.02).ceil().max(1.0) as usize;
    let mut cur = from.clone();
    for j in 1..=n {
        let q = if j == n {
            p
        } else {
            from.p + (p - from.p) * j as f64 / n as f64
        };
        cur = case.fixed(&cur, q, 0.0, &zero)?;
    }
    Ok(cur)
}

#[test]
#[ignore = "ADR 0711: the heavy record of the subcritical branch on 16^3 (D2 to D4), run once with --ignored"]
fn f789_nh16_subcritical_heavy() {
    let mut ch = Checks::new();
    let t_all = Instant::now();
    let mesh16 = block_mesh(16);
    let m = block_model(&mesh16, K_L);
    let m2 = block_model_free_punch(&mesh16, K_L);
    let states = initial_states(&m).unwrap();
    let ndof = m.dof_map().n_equations();
    let fb = FemBranch::new(&m, &m2, 16);
    let free = fb.free.clone();
    let asym = |u: &[f64]| max_abs(u.iter().zip(fb.mirror(u)).map(|(x, y)| x - y)) / max_abs(u.iter().copied());
    let at = |mm: &SolidModel, s: &St| {
        let fint = internal_force(mm, &s.u, &s.states).unwrap();
        block_measures(mm, &s.u, &fint)
    };

    // OB.
    let (text, log) = solve_observed(&mesh16, "block 16^3", K_L, &options(10));
    ch.holds("OB: the error text of ADR 0678", text == ADR0678_TEXT);
    let Some(fa) = failing_attempt(&log) else {
        ch.holds("OB: the failing attempt from 0.9625 observed", false);
        ch.finish();
        return;
    };
    let lc = log.attempts[fa].committed;
    let uc = St {
        p: lc,
        u: log.attempts[fa].iterates[0].clone(),
        states: states.clone(),
    };

    // R0: the symmetric chain (ADR 0700's prefix).
    let mut sym: Vec<(St, Info)> = vec![(uc.clone(), analyse(&fb, &uc))];
    let mut r0 = true;
    for p in [0.963, 0.9635, 0.964, LB, LF] {
        let t0 = Instant::now();
        match fb.solve(&sym.last().unwrap().0, p) {
            Ok(s) => {
                let i = analyse(&fb, &s);
                let (r, w) = at(&m, &s);
                println!(
                    "R0 symmetric at {p:.12}: R/mu {r:.8}, w_c {w:+.8}, nu {:?}, asymmetry {:.3e}, {:.1} s",
                    i.nu,
                    asym(&s.u),
                    t0.elapsed().as_secs_f64()
                );
                sym.push((s, i));
            }
            Err(e) => {
                println!("R0 symmetric at {p:.12}: fails: {e}");
                r0 = false;
                break;
            }
        }
    }
    ch.holds("R0: the symmetric chain converges", r0);
    let find = |v: &[(St, Info)], p: f64| v.iter().position(|(s, _)| s.p == p);
    let (Some(kb), Some(kf)) = (find(&sym, LB), find(&sym, LF)) else {
        ch.holds("R0: the symmetric states at 0.9645 and 0.9647", false);
        ch.finish();
        return;
    };
    let (rsb, _) = at(&m, &sym[kb].0);
    ch.at_most(
        "R0: R/mu at 0.9645 against 0.32726801",
        (rsb / 0.32726801 - 1.0).abs(),
        1e-7,
    );
    let (_, pf) = mode_at(&fb, "R0", &sym[kf].0);
    let Some(pf) = pf else {
        ch.holds("R0: the lowest pair at 0.9647", false);
        ch.finish();
        return;
    };
    let phi = pf.phi.clone();
    ch.at_most(
        "R0: mu_1(0.9647) against 873.6706402833074",
        (pf.mu / 873.6706402833074 - 1.0).abs(),
        1e-9,
    );
    let full = expand(&free, ndof, &phi);
    let sigma = dot(&full, &fb.mirror(&full)) / dot(&full, &full);
    ch.at_most("R0: |sigma(phi_f) + 1|", (sigma + 1.0).abs(), 1e-6);
    ch.at_most("R0: |phi_f . u_s(0.9645)|", amp(&phi, &free, &sym[kb].0.u).abs(), 1e-9);

    // R1: the switch at 0.9647 and the step to 0.9645 (ADR 0700 D3, D4).
    let zero = vec![0.0; free.len()];
    let mut start = sym[kf].0.clone();
    for (k, &q) in free.iter().enumerate() {
        start.u[q] += EPS_SWITCH * phi[k];
    }
    let t0 = Instant::now();
    let Ok(ssw) = fb.fixed(&start, LF, 0.0, &phi) else {
        ch.holds("R1: the switch at 0.9647 converges", false);
        ch.finish();
        return;
    };
    let a_sw = amp(&phi, &free, &ssw.u) - amp(&phi, &free, &sym[kf].0.u);
    println!(
        "R1 switch at 0.9647: a {a_sw:+.12e}, {:.1} s",
        t0.elapsed().as_secs_f64()
    );
    ch.at_most(
        "R1: a at 0.9647 against 2.694982374354194e-5",
        (a_sw / 2.694982374354194e-5 - 1.0).abs(),
        1e-9,
    );
    let Ok(sb) = fb.fixed(&ssw, LB, 0.0, &zero) else {
        ch.holds("R1: the step to 0.9645 converges", false);
        ch.finish();
        return;
    };
    let ib = analyse(&fb, &sb);
    let a_b_rel = amp(&phi, &free, &sb.u) - amp(&phi, &free, &sym[kb].0.u);
    let (rb, wb) = at(&m, &sb);
    println!(
        "R1 the branch at 0.9645: a {a_b_rel:+.12e}, nu {:?}, R/mu {rb:.8}, w_c {wb:+.8}, asymmetry {:.3e}",
        ib.nu,
        asym(&sb.u)
    );
    ch.at_most(
        "R1: a at 0.9645 against 3.827194476e-5",
        (a_b_rel / 3.827194476e-5 - 1.0).abs(),
        1e-8,
    );
    ch.at_most(
        "R1: R/mu at 0.9645 against 0.32726797",
        (rb / 0.32726797 - 1.0).abs(),
        1e-7,
    );
    ch.equals("R1: nu at 0.9645", ib.nu.unwrap_or(usize::MAX), 1);
    let _ = mode_at(&fb, "R1 branch", &sb);

    // F-bar (HFB).
    let mf = block_model(&mesh16, K_FBAR);
    let mf2 = block_model_free_punch(&mesh16, K_FBAR);
    let fbf = FemBranch::new(&mf, &mf2, 16);
    let (ftext, flog) = solve_observed(&mesh16, "F-bar 16^3", K_FBAR, &options(10));
    let flast = flog.attempts.last().unwrap();
    let mut fbar: Vec<(St, Info)> = Vec::new();
    let mut hfb = ftext.is_empty();
    let mut fst = St {
        p: flast.committed,
        u: flast.iterates[0].clone(),
        states: initial_states(&mf).unwrap(),
    };
    for p in [lc, 0.963, 0.9635, 0.964, LB] {
        let t0 = Instant::now();
        match fbf.solve(&fst, p) {
            Ok(s) => {
                let i = analyse(&fbf, &s);
                let (r, w) = at(&mf, &s);
                println!(
                    "HFB F-bar at {p:.12}: R/mu {r:.8}, w_c {w:+.8}, nu {:?}, {:.1} s",
                    i.nu,
                    t0.elapsed().as_secs_f64()
                );
                fst = s.clone();
                fbar.push((s, i));
            }
            Err(e) => {
                println!("HFB F-bar at {p:.12}: fails: {e}");
                hfb = false;
                break;
            }
        }
    }
    ch.holds("HFB: the F-bar chain converges", hfb);
    match find(&fbar, LB) {
        Some(k) => {
            let (r, _) = at(&mf, &fbar[k].0);
            ch.at_most(
                "HFB: R/mu at 0.9645 against 0.31488787",
                (r / 0.31488787 - 1.0).abs(),
                1e-7,
            );
        }
        None => ch.holds("HFB: the F-bar state at 0.9645", false),
    }

    // HL: the load-controlled trace with the amended test.
    let lgrid = [0.964, 0.9635, 0.963, lc];
    let lt = trace_down(&fb, "HL", (sb.clone(), ib.clone()), &lgrid, &phi);
    match lt.attempts.first() {
        Some(a0) => {
            println!(
                "HL0: the first attempt to {:.6}: converged {}, a {:+.6e}, D1 {}, bound {}",
                a0.to, a0.converged, a0.a, a0.old, a0.bound
            );
            ch.holds(
                &format!(
                    "HL0: the first attempt converged to the symmetric root (|a| {:.3e})",
                    a0.a.abs()
                ),
                a0.converged && a0.to == 0.964 && a0.a.abs() <= 1e-3 * a_b_rel,
            );
            ch.holds("HL0: the test of ADR 0697 D1 accepts it", a0.old);
            ch.holds("HL0: the amended test rejects it", !a0.bound);
        }
        None => ch.holds("HL0: the first attempt", false),
    }
    ch.holds(
        &format!("HL1: every grid load reached ({} of 4)", lt.points.len()),
        lt.lost.is_none() && lt.points.len() == 4,
    );
    let mut prev = a_b_rel;
    for (s, i) in &lt.points {
        let Some(k) = find(&sym, s.p) else {
            ch.holds(&format!("HL: the symmetric state at {}", s.p), false);
            continue;
        };
        let a = amp(&phi, &free, &s.u) - amp(&phi, &free, &sym[k].0.u);
        let (r, w) = at(&m, s);
        let (rs, ws) = at(&m, &sym[k].0);
        ch.equals(&format!("HL2: nu at {}", s.p), i.nu.unwrap_or(usize::MAX), 1);
        ch.holds(&format!("HL2: a {a:.6e} larger than {prev:.6e} at {}", s.p), a > prev);
        let ratio = a / a_red(s.p);
        ch.holds(
            &format!("HL2: a / a_red {ratio:.6} in [0.5, 2] at {}", s.p),
            (0.5..=2.0).contains(&ratio),
        );
        prev = a;
        ch.at_most(
            &format!("HL3: |R_bif / R_sym - 1| at {}", s.p),
            (r / rs - 1.0).abs(),
            1e-5,
        );
        match find(&fbar, s.p) {
            Some(kf2) => {
                let (rf, wf) = at(&mf, &fbar[kf2].0);
                println!(
                    "TABLE HL {:.12}: bifurcated nu {:?} a {a:+.6e} R/mu {r:.8} w_c {w:+.8} asym {:.3e} | symmetric nu {:?} \
                     R/mu {rs:.8} w_c {ws:+.8} | F-bar nu {:?} R/mu {rf:.8} w_c {wf:+.8} | g_R bif {:+.6e} sym {:+.6e}",
                    s.p,
                    i.nu,
                    asym(&s.u),
                    sym[k].1.nu,
                    fbar[kf2].1.nu,
                    r / rf - 1.0,
                    rs / rf - 1.0
                );
                for (name, x, y) in [
                    ("R_bif / R_Fb", r, rf),
                    ("w_bif / w_Fb", w, wf),
                    ("R_sym / R_Fb", rs, rf),
                    ("w_sym / w_Fb", ws, wf),
                ] {
                    ch.at_most(&format!("HL3 at {}: |{name} - 1|", s.p), (x / y - 1.0).abs(), 0.15);
                }
            }
            None => ch.holds(&format!("HL3: the F-bar state at {}", s.p), false),
        }
    }

    // AT0: the bordered Newton at the start.
    let a_b = amp(&phi, &free, &sb.u);
    let st0 = fb.at_load(&sb, 0.96451);
    match bordered(
        &fb,
        &st0,
        &Control {
            g: phi.clone(),
            h: 0.0,
            c: a_b,
        },
    ) {
        Ok((s, its)) => {
            println!("AT0: {its} iterations, lambda {:.15}", s.p);
            ch.at_most("AT0: |lambda - 0.9645|", (s.p - LB).abs(), 1e-9);
            ch.at_most(
                "AT0: |u - u_b|_inf / |u_b|_inf",
                max_abs(s.u.iter().zip(&sb.u).map(|(x, y)| x - y)) / max_abs(sb.u.iter().copied()),
                1e-8,
            );
        }
        Err(e) => ch.holds(&format!("AT0: the bordered Newton at the start ({e})"), false),
    }

    // AT: the amplitude-controlled trace.
    let atr = amp_trace(&fb, "AT", (sb.clone(), ib.clone()), &phi, 18, (0.5, 1.2));
    match atr.points.get(1) {
        Some(p) if p.rung == Some(1) => {
            let ratio = (LAMBDA_C - p.s.p) * ALPHA.abs() / (BETA.abs() * p.a * p.a);
            ch.holds(
                &format!("AT1: the first rung, (lambda_c - lambda) |alpha| / (|beta| a^2) {ratio:.6} in [0.25, 4]"),
                (0.25..=4.0).contains(&ratio),
            );
        }
        _ => ch.holds("AT1: the first rung accepted", false),
    }
    let cmax = atr.points[1..]
        .iter()
        .filter_map(|p| p.rung.map(|k| ((p.a - a_b * 2f64.powf(k as f64 / 2.0)) / p.a).abs()))
        .fold(0.0f64, |w, x| if x.is_finite() { w.max(x) } else { f64::NAN });
    ch.at_most("AT2: the constraint at the rung states, relative", cmax, 1e-9);
    println!("AT2: the stop of the trace: {}", atr.stop);
    let mut inside = 0;
    for (s, _) in &lt.points {
        let a = amp(&phi, &free, &s.u);
        if let Some(lh) = hermite(&atr.points, a) {
            inside += 1;
            ch.at_most(
                &format!("AT3: |lambda_H - lambda| / (lambda_c - lambda) at {}", s.p),
                (lh - s.p).abs() / (LAMBDA_C - s.p),
                1e-2,
            );
        }
    }
    println!("AT3: {inside} states of the load-controlled trace inside the amplitude range");
    ch.holds(
        &format!("AF: the reading not undecided ({})", atr.reading),
        atr.reading != "undecided",
    );
    let mut look = vec![atr.points.len() - 1];
    if let Some(f) = &atr.fold {
        look.extend([f.k - 1, f.k]);
    }
    look.sort_unstable();
    look.dedup();
    for k in look {
        let _ = mode_at(&fb, &format!("AF state {k}"), &atr.points[k].s);
    }

    // AC: the comparisons at the rung states.
    let mut sc = sym[kb].0.clone();
    let mut fc = find(&fbar, LB).map(|k| fbar[k].0.clone());
    for p in atr.points[1..].iter().filter(|p| p.rung.is_some()) {
        let lam = p.s.p;
        if !(0.5..=1.2).contains(&lam) {
            continue;
        }
        let t0 = Instant::now();
        let ss = chain_to(&fb, &sc, lam);
        let ff = fc.as_ref().map(|f| chain_to(&fbf, f, lam));
        let (Ok(ss), Some(Ok(ff))) = (ss, ff) else {
            ch.holds(&format!("AC: the symmetric and F-bar states at {lam:.12}"), false);
            break;
        };
        let is = analyse(&fb, &ss);
        let iff = analyse(&fbf, &ff);
        let (r, w) = at(&m, &p.s);
        let (rs, ws) = at(&m, &ss);
        let (rf, wf) = at(&mf, &ff);
        println!(
            "TABLE AT rung {:?} lambda {lam:.12}: bifurcated nu {:?} a {:+.6e} dlambda/da {:+.6e} R/mu {r:.8} w_c {w:+.8} asym \
             {:.3e} | symmetric nu {:?} R/mu {rs:.8} w_c {ws:+.8} | F-bar nu {:?} R/mu {rf:.8} w_c {wf:+.8} | g_R bif {:+.6e} \
             sym {:+.6e}, {:.1} s",
            p.rung,
            p.info.nu,
            p.a,
            p.sigma,
            asym(&p.s.u),
            is.nu,
            iff.nu,
            r / rf - 1.0,
            rs / rf - 1.0,
            t0.elapsed().as_secs_f64()
        );
        ch.at_most(
            &format!("AC at {lam:.9}: the symmetric state's asymmetry"),
            asym(&ss.u),
            1e-9,
        );
        for (name, x, y) in [
            ("R_bif / R_Fb", r, rf),
            ("w_bif / w_Fb", w, wf),
            ("R_sym / R_Fb", rs, rf),
            ("w_sym / w_Fb", ws, wf),
        ] {
            ch.at_most(&format!("AC at {lam:.9}: |{name} - 1|"), (x / y - 1.0).abs(), 0.15);
        }
        sc = ss;
        fc = Some(ff);
    }
    println!("f789_nh16_subcritical_heavy: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}
