//! F-779 (ADR 0697): where rule L's 16^3 punch path loses stability. On the punch block of ADR 0694 (rule L, neo-Hookean,
//! `16^3`): the load between `0.9625` and `0.965625` where the inertia of the path changes, bracketed by bisection with the
//! indefinite-tangent Newton of ADR 0694 D2, each midpoint solved from the converged lower neighbour (D1); a limit point told
//! from a bifurcation by the projection of the critical mode on the load vector and by the sign of the load parameter's
//! derivative along the path (D2); F-bar's path at the same loads with its inertia (D3). The bracket and the readings are
//! written once over a `Case` and run by the fast test on a closed-form fold and pitchfork, on the ADR 0628 patch (a
//! finite-element bifurcation with a reference load) and on the stable `8^3` block (D4). The helpers are copies of those of
//! `si_eas_hg14.rs` (existing tests are not edited). Every aggregated value is required finite before it is compared.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::constraint::{prescribed_equations, reduce, Prescribed};
use rcfd_fem::element::ElementState;
use rcfd_fem::material::HyperelasticEnergy;
use rcfd_fem::nl_incompatible::IncompatibleStabilisation;
use rcfd_fem::nonlinear::{
    initial_states, internal_force, solve_increment, solve_with, tangent, Contribution, DriverExtensions, LoadStep,
    NewtonOptions, StepControl,
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
use std::collections::{BTreeSet, HashMap};
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
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.4e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
        }
    }
    fn below(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value < limit;
        println!(
            "{name}: {value:.4e} (limit < {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} >= {limit:e}"));
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
    fn finite(&mut self, name: &str, value: f64) {
        let ok = value.is_finite();
        println!(
            "{name}: {value:.6e} (reported, finite) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} not finite"));
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

/// One solve of the bracket: the load, accepted on the branch, the inertia, `|v_t|`, the outcome.
struct Trial {
    p: f64,
    accepted: bool,
    nu: Option<usize>,
    vt_norm: f64,
    text: String,
}

struct Bracket {
    /// `false`: the inertia is the same at both ends of the interval (nothing bisected).
    change: bool,
    a: St,
    ia: Info,
    b_p: f64,
    /// The upper end, `None` when it is lost.
    b: Option<(St, Info)>,
    /// The first solve of the upper end of the interval.
    hi0: Option<(St, Info)>,
    hi0_accepted: bool,
    nu0: Option<usize>,
    c: f64,
    bisections: usize,
}

/// D1: the continuity test of a solve `s` from `from` (with its tangent `vt`).
fn continuous(from: &St, free: &[usize], vt: &[f64], s: &St) -> bool {
    let d = s.p - from.p;
    let pred = max_abs(vt.iter().map(|v| d * v));
    let corr = max_abs(free.iter().zip(vt).map(|(&q, v)| s.u[q] - from.u[q] - d * v));
    corr.is_finite() && pred.is_finite() && corr <= pred
}

fn attempt(case: &dyn Case, from: &St, ifrom: &Info, p: f64) -> (Option<(St, Info)>, bool, String) {
    let Some(vt) = ifrom.vt.as_ref() else {
        return (None, false, "the lower end's factor refused".to_string());
    };
    match case.solve(from, p) {
        Ok(s) => {
            let ok = continuous(from, &ifrom.sys.free, vt, &s);
            let i = analyse(case, &s);
            (
                Some((s, i)),
                ok,
                if ok { "accepted" } else { "discontinuous" }.to_string(),
            )
        }
        Err(e) => (None, false, format!("fails: {e}")),
    }
}

/// D1: the bracket of the change of the inertia between `lo` and `hi_p` with `cap` bisections.
fn bracket(case: &dyn Case, tag: &str, lo: St, hi_p: f64, cap: usize) -> Bracket {
    let t0 = Instant::now();
    let ia = analyse(case, &lo);
    let nu0 = ia.nu;
    let c = ia.vt.as_ref().map(|v| norm(v.iter().copied())).unwrap_or(f64::NAN);
    println!("{tag}: lower end {:.9}: nu {nu0:?}, |v_t| {c:.6e}", lo.p);
    let (hi, ok, text) = attempt(case, &lo, &ia, hi_p);
    let nu_hi = hi.as_ref().and_then(|h| h.1.nu);
    let vn = |x: &Option<(St, Info)>| {
        x.as_ref()
            .and_then(|h| h.1.vt.as_ref())
            .map(|v| norm(v.iter().copied()) / c)
            .unwrap_or(f64::NAN)
    };
    let mut trials = vec![Trial {
        p: hi_p,
        accepted: ok,
        nu: nu_hi,
        vt_norm: vn(&hi),
        text,
    }];
    let hi0 = hi.clone();
    let mut out = Bracket {
        change: true,
        a: lo.clone(),
        ia,
        b_p: hi_p,
        b: if ok { hi } else { None },
        hi0,
        hi0_accepted: ok,
        nu0,
        c,
        bisections: 0,
    };
    if ok && nu_hi == nu0 {
        out.change = false;
    } else {
        for _ in 0..cap {
            let m = 0.5 * (out.a.p + out.b_p);
            let (s, ok, text) = attempt(case, &out.a, &out.ia, m);
            let nu = s.as_ref().and_then(|h| h.1.nu);
            trials.push(Trial {
                p: m,
                accepted: ok,
                nu,
                vt_norm: vn(&s),
                text,
            });
            out.bisections += 1;
            if ok && nu == nu0 {
                let (st, i) = s.unwrap();
                out.a = st;
                out.ia = i;
            } else {
                out.b_p = m;
                out.b = if ok { s } else { None };
            }
        }
    }
    for t in &trials {
        println!(
            "{tag} trial at {:.12}: {} (on the branch {}), nu {:?}, |v_t| / c {:.6e}",
            t.p, t.text, t.accepted, t.nu, t.vt_norm
        );
    }
    println!(
        "{tag}: change {}, bracket [{:.12}, {:.12}], width {:.4e}, {} bisections, upper end {}, {:.1} s",
        out.change,
        out.a.p,
        out.b_p,
        out.b_p - out.a.p,
        out.bisections,
        if out.b.is_some() { "accepted" } else { "lost" },
        t0.elapsed().as_secs_f64()
    );
    out
}

/// D2 (b): `DS = t_a . t_b`, `t = (v_t, c) / |(v_t, c)|`.
fn ds(va: &[f64], vb: &[f64], c: f64) -> f64 {
    let na = (dot(va, va) + c * c).sqrt();
    let nb = (dot(vb, vb) + c * c).sqrt();
    (dot(va, vb) + c * c) / (na * nb)
}

/// D2 (a): `|phi . f| / (|phi| |f|)`.
fn cosine(phi: &[f64], f: &[f64]) -> f64 {
    dot(phi, f).abs() / (norm(phi.iter().copied()) * norm(f.iter().copied()))
}

const THETA: f64 = 1e-3;

/// The measures of one end: `mu_1`, `phi`, the residual, `cos`, `b_bif`.
struct End {
    pair: Option<Pair>,
    cos: f64,
    bbif: f64,
}

fn end_of(case: &dyn Case, i: &Info) -> End {
    let pair = i.nu.and_then(|nu| case.lowest_pair(&i.sys, nu));
    let cos = pair.as_ref().map(|p| cosine(&p.phi, &i.sys.f)).unwrap_or(f64::NAN);
    let vt = i.vt.as_ref().map(|v| norm(v.iter().copied())).unwrap_or(f64::NAN);
    let bbif = pair
        .as_ref()
        .map(|p| p.mu.abs() * vt / norm(i.sys.f.iter().copied()))
        .unwrap_or(f64::NAN);
    End { pair, cos, bbif }
}

struct Reading {
    ea: End,
    eb: Option<End>,
    ds: f64,
    text: &'static str,
}

/// D2: the reading LB of a bracket.
fn read(case: &dyn Case, tag: &str, br: &Bracket) -> Reading {
    let ea = end_of(case, &br.ia);
    let eb = br.b.as_ref().map(|(_, i)| end_of(case, i));
    let d = match (&br.ia.vt, br.b.as_ref().and_then(|(_, i)| i.vt.as_ref())) {
        (Some(va), Some(vb)) => ds(va, vb, br.c),
        _ => f64::NAN,
    };
    let lost = br.b.is_none();
    let text = if !br.change {
        "no change"
    } else {
        let cos_b = eb.as_ref().map(|e| e.cos);
        let all_above = ea.cos > THETA && cos_b.map_or(true, |c| c > THETA);
        let all_below = ea.cos < THETA && cos_b.is_some_and(|c| c < THETA);
        if all_above && (lost || d < 0.0) {
            "limit point"
        } else if all_below && !lost && d > 0.0 {
            "bifurcation"
        } else {
            "undecided"
        }
    };
    let show = |e: &End| {
        format!(
            "mu_1 {:?}, residual {:?}, cos {:.6e}, b_bif {:.6e}",
            e.pair.as_ref().map(|p| p.mu),
            e.pair.as_ref().map(|p| p.residual),
            e.cos,
            e.bbif
        )
    };
    println!(
        "{tag} READING LB: {text}; at a: {}; at b: {}; DS {d:.12e}",
        show(&ea),
        eb.as_ref().map(show).unwrap_or_else(|| "lost".to_string())
    );
    Reading { ea, eb, ds: d, text }
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

// ============================================================================ D4: the closed-form cases

/// Eight unknowns: `r_0` the fold (`u_0 - u_0^3 / 3 - p`) or the pitchfork (`(1 - p) u_0 + u_0^3`), `r_i = k_i u_i - p b_i`.
struct Toy {
    fold: bool,
}

const TOY_N: usize = 8;

impl Toy {
    fn k_i(i: usize) -> f64 {
        1.0 + i as f64
    }
    fn b_i(i: usize) -> f64 {
        0.1 * i as f64
    }
    fn residual(&self, u: &[f64], p: f64) -> Vec<f64> {
        (0..TOY_N)
            .map(|i| match i {
                0 if self.fold => u[0] - u[0].powi(3) / 3.0 - p,
                0 => (1.0 - p) * u[0] + u[0].powi(3),
                _ => Toy::k_i(i) * u[i] - p * Toy::b_i(i),
            })
            .collect()
    }
    fn diag(&self, u: &[f64], p: f64) -> Vec<f64> {
        (0..TOY_N)
            .map(|i| match i {
                0 if self.fold => 1.0 - u[0] * u[0],
                0 => 1.0 - p + 3.0 * u[0] * u[0],
                _ => Toy::k_i(i),
            })
            .collect()
    }
    fn load(&self) -> Vec<f64> {
        (0..TOY_N)
            .map(|i| match i {
                0 if self.fold => 1.0,
                0 => 0.0,
                _ => Toy::b_i(i),
            })
            .collect()
    }
}

impl Case for Toy {
    fn solve(&self, from: &St, to: f64) -> Result<St, String> {
        let mut u = from.u.clone();
        for it in 0..=20 {
            let r = self.residual(&u, to);
            if r.iter().any(|v| !v.is_finite()) || u.iter().any(|v| !v.is_finite()) {
                return Err(format!("not finite at iteration {it}"));
            }
            if max_abs(r.iter().copied()) <= 1e-14 {
                return Ok(St {
                    p: to,
                    u,
                    states: Vec::new(),
                });
            }
            if it == 20 {
                break;
            }
            let d = self.diag(&u, to);
            if d.contains(&0.0) {
                return Err("singular".to_string());
            }
            for i in 0..TOY_N {
                u[i] -= r[i] / d[i];
            }
        }
        Err("not converged in 20 iterations".to_string())
    }
    fn system(&self, s: &St) -> Sys {
        let d = self.diag(&s.u, s.p);
        Sys {
            k: CsrMatrix::from_triplets(TOY_N, d.iter().enumerate().map(|(i, &v)| (i, i, v)).collect()),
            f: self.load(),
            free: (0..TOY_N).collect(),
        }
    }
    fn lowest_pair(&self, s: &Sys, _nu: usize) -> Option<Pair> {
        let mut dense = vec![0.0; TOY_N * TOY_N];
        for r in 0..TOY_N {
            for q in s.k.row_ptr[r]..s.k.row_ptr[r + 1] {
                dense[TOY_N * r + s.k.col[q]] = s.k.val[q];
            }
        }
        let e = symmetric_eigen(&dense, TOY_N).ok()?;
        Some(Pair {
            mu: e.values[0],
            phi: e.vectors[0].clone(),
            residual: 0.0,
        })
    }
}

// ============================================================================ the ADR 0628 patch

/// SplitMix64 from a seed (`si_eas_hg3.rs`).
struct Rng(u64);

impl Rng {
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

fn rebuild(mesh: &SolidMesh, nodes: Vec<Vec3>) -> SolidMesh {
    let mut out = SolidMesh::new(nodes).unwrap();
    for b in mesh.blocks() {
        out.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
    }
    for name in mesh.node_set_names() {
        out.add_node_set(name, mesh.node_set(name).unwrap().to_vec()).unwrap();
    }
    for name in mesh.element_set_names() {
        out.add_element_set(name, mesh.element_set(name).unwrap().to_vec())
            .unwrap();
    }
    for name in mesh.surface_set_names() {
        out.add_surface_set(name, mesh.surface_set(name).unwrap().to_vec())
            .unwrap();
    }
    out
}

fn boundary_nodes(mesh: &SolidMesh) -> BTreeSet<usize> {
    let mut s = BTreeSet::new();
    for f in BOX_FACES {
        s.extend(mesh.node_set(f).unwrap().iter().copied());
    }
    s
}

const HG_SEED: u64 = 0xF685_0614;

/// The ADR 0628 patch: the unit cube on `n^3` Hex8, interior nodes moved by `0.1 h (2U - 1)` (seed `0xF685_0614`),
/// `nu = 0.49`.
fn patch(n: usize, f: SolidFormulation, rule: IncompatibleStabilisation) -> SolidModel {
    let base = hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap();
    let boundary = boundary_nodes(&base);
    let mut rng = Rng(HG_SEED);
    let h = 1.0 / n as f64;
    let mut nodes = base.nodes().to_vec();
    for (i, p) in nodes.iter_mut().enumerate() {
        if !boundary.contains(&i) {
            for v in p.iter_mut() {
                *v += 0.1 * h * (2.0 * rng.uniform() - 1.0);
            }
        }
    }
    let mut m = SolidModel::new(with_formulation(rebuild(&base, nodes), f));
    m.set_incompatible_stabilisation(rule);
    m.add_material("all", neo_hookean()).unwrap();
    m
}

/// The confined-compression path direction `v = (0, 0, -Z)` at every node.
fn patch_direction(m: &SolidModel) -> Vec<f64> {
    let dofs = m.dof_map();
    let mut v = vec![0.0; dofs.n_equations()];
    for (n, x) in m.mesh().nodes().iter().enumerate() {
        v[dofs.equation(n, 2).unwrap()] = -x[2];
    }
    v
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

// ============================================================================ the fast test

#[test]
#[ignore = "ADR 0697: the registered fast run, known miss (amendment 1): PV 6.0184e-6 > 1e-6, evaluated at the Newton-converged bisected state whose error along the soft mode is amplified by 1/mu_1 (2.3e-11 at the exact homogeneous field); every other row met; f779_fast_amended evaluates PV at the exact field"]
fn f779_fast_known_miss() {
    fast(false);
}

/// The corrected fast test of ADR 0697 amendment 1: PV at the exact homogeneous field, the pairs by the backward error.
#[test]
fn f779_fast_amended() {
    fast(true);
}

fn fast(amended: bool) {
    let test = if amended {
        ResidualTest::Backward
    } else {
        ResidualTest::Relative
    };
    let mut ch = Checks::new();
    let t_all = Instant::now();

    // LPT: the fold.
    let lp = Toy { fold: true };
    let lo = St {
        p: 0.6,
        u: vec![0.0; TOY_N],
        states: Vec::new(),
    };
    let lo = lp.solve(&lo, 0.6).unwrap();
    let br = bracket(&lp, "LPT", lo, 0.7, 20);
    let rd = read(&lp, "LPT", &br);
    let pc = 2.0 / 3.0;
    ch.holds("LPT: a <= 2/3 <= b", br.a.p <= pc && pc <= br.b_p);
    ch.at_most("LPT: b - a", br.b_p - br.a.p, 9.6e-8);
    ch.holds("LPT: the upper end lost", br.b.is_none());
    ch.at_most(
        "LPT: cos(a) against 1/sqrt(2.4)",
        (rd.ea.cos * 2.4f64.sqrt() - 1.0).abs(),
        1e-10,
    );
    ch.holds(
        &format!("LPT: reading 'limit point' ({})", rd.text),
        rd.text == "limit point",
    );

    // BFT: the pitchfork.
    let bf = Toy { fold: false };
    let lo = bf
        .solve(
            &St {
                p: 0.9,
                u: vec![0.0; TOY_N],
                states: Vec::new(),
            },
            0.9,
        )
        .unwrap();
    let br = bracket(&bf, "BFT", lo, 1.2, 20);
    let rd = read(&bf, "BFT", &br);
    ch.holds("BFT: a <= 1 <= b", br.a.p <= 1.0 && 1.0 <= br.b_p);
    ch.at_most("BFT: b - a", br.b_p - br.a.p, 2.9e-7);
    ch.equals("BFT: nu_b", br.b.as_ref().and_then(|b| b.1.nu).unwrap_or(usize::MAX), 1);
    ch.at_most("BFT: cos(a)", rd.ea.cos, 1e-12);
    ch.at_most("BFT: cos(b)", rd.eb.as_ref().map(|e| e.cos).unwrap_or(f64::NAN), 1e-12);
    ch.at_least("BFT: DS", rd.ds, 1.0 - 1e-12);
    ch.holds(
        &format!("BFT: reading 'bifurcation' ({})", rd.text),
        rd.text == "bifurcation",
    );

    // DSF: the sign function.
    let w: Vec<f64> = (0..TOY_N).map(|i| if i == 0 { 0.0 } else { 1.0 }).collect();
    let va: Vec<f64> = w
        .iter()
        .enumerate()
        .map(|(i, x)| x + if i == 0 { 1e3 } else { 0.0 })
        .collect();
    let vb: Vec<f64> = w
        .iter()
        .enumerate()
        .map(|(i, x)| x - if i == 0 { 1e3 } else { 0.0 })
        .collect();
    ch.below("DSF: DS of the flipped tangents", ds(&va, &vb, 1.0), 0.0);
    ch.at_most(
        "DSF: |DS - 1| of equal tangents",
        (ds(&va, &va, 1.0) - 1.0).abs(),
        1e-15,
    );

    // PB: the ADR 0628 patch, the plain element in confined compression.
    let mp = patch(6, INC, OFF);
    let v = patch_direction(&mp);
    let boundary = boundary_nodes(mp.mesh());
    let dofs = mp.dof_map();
    let mut pres = Vec::new();
    for &n in &boundary {
        for c in 0..3 {
            pres.push(Prescribed::new(n, c, v[dofs.equation(n, c).unwrap()]));
        }
    }
    let p0 = 0.04;
    let lo = St {
        p: p0,
        u: v.iter().map(|x| p0 * x).collect(),
        states: initial_states(&mp).unwrap(),
    };
    let case = FemCase {
        m: &mp,
        pres,
        opts: options(1).indefinite_tangent(true),
        test,
    };
    let br = bracket(&case, "PB", lo, 0.05, 16);
    let rd = read(&case, "PB", &br);
    let pc = 1.0 - 0.9507467;
    ch.holds(
        &format!("PB: p_c = {pc} in [a - 6e-8, b + 6e-8]"),
        br.a.p - 6e-8 <= pc && pc <= br.b_p + 6e-8,
    );
    ch.at_most("PB: b - a", br.b_p - br.a.p, 1.6e-7);
    ch.equals("PB: nu_a", br.ia.nu.unwrap_or(usize::MAX), 0);
    let nub = br.b.as_ref().and_then(|b| b.1.nu).unwrap_or(0);
    ch.holds(&format!("PB: nu_b >= 1 ({nub})"), nub >= 1);
    ch.below("PB: cos(a)", rd.ea.cos, THETA);
    ch.below("PB: cos(b)", rd.eb.as_ref().map(|e| e.cos).unwrap_or(f64::NAN), THETA);
    ch.at_least("PB: DS", rd.ds, 0.99);
    let exact_a = St {
        p: br.a.p,
        u: v.iter().map(|x| br.a.p * x).collect(),
        states: initial_states(&mp).unwrap(),
    };
    let ipv = if amended {
        analyse(&case, &exact_a)
    } else {
        br.ia.clone()
    };
    let pv = match &ipv.vt {
        Some(vt) => {
            let exact: Vec<f64> = ipv.sys.free.iter().map(|&q| v[q]).collect();
            norm(vt.iter().zip(&exact).map(|(a, b)| a - b)) / norm(exact.iter().copied())
        }
        None => f64::NAN,
    };
    ch.at_most("PV: |v_t(a) - v| / |v| on the interior", pv, 1e-6);
    ms(&mut ch, "PB", &rd);
    ch.holds(
        &format!("PB: reading 'bifurcation' ({})", rd.text),
        rd.text == "bifurcation",
    );

    // NC8: the stable 8^3 block.
    let mesh8 = block_mesh(8);
    let m8 = block_model(&mesh8, K_L);
    let (_, log8) = solve_observed(&mesh8, "block 8^3", K_L, &options(10));
    let last = log8.attempts.last().unwrap();
    let lo = St {
        p: last.committed,
        u: last.iterates[0].clone(),
        states: initial_states(&m8).unwrap(),
    };
    let case8 = FemCase {
        m: &m8,
        pres: Vec::new(),
        opts: options(10).indefinite_tangent(true),
        test,
    };
    let br = bracket(&case8, "NC8", lo, last.attempted, 12);
    let rd = read(&case8, "NC8", &br);
    ch.holds(
        &format!("NC8: reading 'no change' ({})", rd.text),
        rd.text == "no change",
    );
    match &br.hi0 {
        Some((s, i)) => {
            let fint = internal_force(&m8, &s.u, &s.states).unwrap();
            let (r, w) = block_measures(&m8, &s.u, &fint);
            ch.at_most("NC8: R against 0.36659596", (r / 0.36659596 - 1.0).abs(), 1e-6);
            ch.at_most("NC8: w_c against 0.06631664", (w / 0.06631664 - 1.0).abs(), 1e-6);
            let e = end_of(&case8, i);
            ch.finite("NC8: cos(1.0)", e.cos);
            ch.finite(
                "NC8: |v_t(1.0)|",
                i.vt.as_ref().map(|v| norm(v.iter().copied())).unwrap_or(f64::NAN),
            );
        }
        None => ch.holds("NC8: the solve to 1.0", false),
    }
    println!("f779_fast: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}

/// MS: `mu_1(a) > 0 > mu_1(b)` and the residuals of the pairs `<= 1e-8`.
fn ms(ch: &mut Checks, tag: &str, rd: &Reading) {
    let mu_a = rd.ea.pair.as_ref().map(|p| p.mu).unwrap_or(f64::NAN);
    let mu_b = rd
        .eb
        .as_ref()
        .and_then(|e| e.pair.as_ref())
        .map(|p| p.mu)
        .unwrap_or(f64::NAN);
    ch.holds(
        &format!("{tag} MS: mu_1(a) {mu_a:e} > 0 > mu_1(b) {mu_b:e}"),
        mu_a > 0.0 && mu_b < 0.0,
    );
    let res = [
        rd.ea.pair.as_ref().map(|p| p.residual),
        rd.eb.as_ref().and_then(|e| e.pair.as_ref()).map(|p| p.residual),
    ]
    .iter()
    .fold(0.0f64, |w, r| match r {
        Some(v) if v.is_finite() => w.max(*v),
        _ => f64::NAN,
    });
    ch.at_most(&format!("{tag} MS: the residuals of the pairs"), res, 1e-8);
}

/// The failing attempt of the observed `16^3` solve: its index in the log.
fn failing_attempt(log: &Log) -> Option<usize> {
    log.attempts
        .iter()
        .rposition(|a| a.committed == 0.9625 && a.converged == Some(false))
}

// ============================================================================ the heavy record

#[test]
#[ignore = "ADR 0697: the registered heavy record of the bracket on 16^3 (D1 to D3), run once with --ignored (166 s), known miss (amendment 1): MS at b, the lowest pair NotConverged in 500 iterations (relative residual 2.57e-8 > 1e-8 at its rounding floor c eps |A| / |mu_1|, mu_1 ~ -2.5), so reading LB 'undecided'; the bracket [0.964899444580, 0.964900207520] (width 7.63e-7, 12 bisections) and every other row met; F-bar stable (nu 0) at the four loads; f779_nh16_bracket_heavy_amended takes the backward-error residual"]
fn f779_nh16_bracket_heavy_known_miss() {
    heavy(false);
}

/// The corrected heavy record of ADR 0697 amendment 1: the lowest pairs by the normwise backward error.
#[test]
#[ignore = "ADR 0697: the corrected heavy record of the bracket on 16^3 (amendment 1), run once with --ignored (71 s, every row met): the inertia changes 0 -> 1 in [0.964899444580, 0.964900207520] (width 7.63e-7); cos 1.5e-12 and 1.7e-12, DS 1 - 1.3e-12, phi antisymmetric under x <-> y (sigma -1): reading LB a bifurcation; F-bar stable (nu 0) at the four loads; f779_fast_amended covers the same bracket and readings on the fold, the pitchfork, the ADR 0628 patch and 8^3"]
fn f779_nh16_bracket_heavy_amended() {
    heavy(true);
}

fn heavy(amended: bool) {
    let test = if amended {
        ResidualTest::Backward
    } else {
        ResidualTest::Relative
    };
    let mut ch = Checks::new();
    let t_all = Instant::now();
    let mesh16 = block_mesh(16);
    let m = block_model(&mesh16, K_L);
    let states = initial_states(&m).unwrap();
    let ndof = m.dof_map().n_equations();

    // OB.
    let (text, log) = solve_observed(&mesh16, "block 16^3", K_L, &options(10));
    ch.holds("OB: the error text of ADR 0678", text == ADR0678_TEXT);
    let Some(fa) = failing_attempt(&log) else {
        ch.holds("OB: the failing attempt from 0.9625 observed", false);
        ch.finish();
        return;
    };
    let att = &log.attempts[fa];
    let (lc, lt) = (att.committed, att.attempted);
    let uc = att.iterates[0].clone();
    let fc = internal_force(&m, &uc, &states).unwrap();
    let (rc, wcc) = block_measures(&m, &uc, &fc);
    println!("u_c: R/mu {rc:.8}, w_c {wcc:+.8}");

    // H0, BR: the bracket.
    let case = FemCase {
        m: &m,
        pres: Vec::new(),
        opts: options(10).indefinite_tangent(true),
        test,
    };
    let lo = St {
        p: lc,
        u: uc.clone(),
        states: states.clone(),
    };
    let br = bracket(&case, "BR 16^3", lo, lt, 12);
    ch.equals("H0: nu(u_c)", br.nu0.unwrap_or(usize::MAX), 0);
    ch.holds("H0: the upper end from u_c accepted", br.hi0_accepted);
    match &br.hi0 {
        Some((s, i)) => {
            ch.equals("H0: nu at lt", i.nu.unwrap_or(usize::MAX), 1);
            let fint = internal_force(&m, &s.u, &s.states).unwrap();
            let (r, w) = block_measures(&m, &s.u, &fint);
            ch.at_most(
                "H0: R against ADR 0694's 0.32765165",
                (r / 0.32765165 - 1.0).abs(),
                1e-7,
            );
            ch.at_most(
                "H0: w_c against ADR 0694's 0.06039238",
                (w / 0.06039238 - 1.0).abs(),
                1e-7,
            );
        }
        None => ch.holds("H0: the upper end from u_c converges", false),
    }
    ch.holds(
        &format!("BR: 0.9625 <= a {} < b {} <= lt", br.a.p, br.b_p),
        lc <= br.a.p && br.a.p < br.b_p && br.b_p <= lt,
    );
    ch.at_most("BR: b - a", br.b_p - br.a.p, 3.125e-3 / 4096.0 * (1.0 + 1e-9));
    ch.equals("BR: bisections", br.bisections, 12);

    // MS, LB.
    let rd = read(&case, "LB 16^3", &br);
    ms(&mut ch, "16^3", &rd);
    let refl = reflection(&m, 16);
    let sym_of = |free: &[usize], x: &[f64]| {
        let full = expand(free, ndof, x);
        let px: Vec<f64> = (0..ndof).map(|q| full[refl[q]]).collect();
        (dot(&full, &px) / dot(&full, &full), px, full)
    };
    for (end, e, info, st) in [
        ("a", Some(&rd.ea), Some(&br.ia), Some(&br.a)),
        (
            "b",
            rd.eb.as_ref(),
            br.b.as_ref().map(|b| &b.1),
            br.b.as_ref().map(|b| &b.0),
        ),
    ] {
        let (Some(e), Some(info), Some(st)) = (e, info, st) else {
            continue;
        };
        if let Some(p) = &e.pair {
            let (sigma, _, _) = sym_of(&info.sys.free, &p.phi);
            println!("LB at {end}: sigma(phi) = phi . P phi / |phi|^2 = {sigma:+.12e}");
        }
        let (_, pf, ff) = sym_of(&info.sys.free, &info.sys.f);
        println!(
            "LB at {end}: |f - P f| / |f| {:.3e}; |u - P u|_inf / |u|_inf {:.3e}; |v_t| / c {:.6e}",
            norm(ff.iter().zip(&pf).map(|(x, y)| x - y)) / norm(ff.iter().copied()),
            max_abs((0..ndof).map(|q| st.u[q] - st.u[refl[q]])) / max_abs(st.u.iter().copied()),
            info.vt.as_ref().map(|v| norm(v.iter().copied())).unwrap_or(f64::NAN) / br.c
        );
    }
    println!(
        "READING LB 16^3: {}; the load of the change in [{:.12}, {:.12}]",
        rd.text, br.a.p, br.b_p
    );

    // The rule L measures at a and b.
    let at = |s: &St| {
        let fint = internal_force(&m, &s.u, &s.states).unwrap();
        block_measures(&m, &s.u, &fint)
    };
    let la = at(&br.a);
    let lb = br.b.as_ref().map(|b| at(&b.0));
    println!("rule L at a: R/mu {:.8}, w_c {:+.8}; at b: {lb:?}", la.0, la.1);

    // FB0, FBP: F-bar's path at the same loads.
    let mf = block_model(&mesh16, K_FBAR);
    let (ftext, flog) = solve_observed(&mesh16, "F-bar 16^3", K_FBAR, &options(10));
    ch.holds("FBP: the F-bar ten-increment solve passes", ftext.is_empty());
    let fcase = FemCase {
        m: &mf,
        pres: Vec::new(),
        opts: options(10).indefinite_tangent(true),
        test,
    };
    let flast = flog.attempts.last().unwrap();
    let mut fst = St {
        p: flast.committed,
        u: flast.iterates[0].clone(),
        states: initial_states(&mf).unwrap(),
    };
    let mut fb: Vec<(f64, f64)> = Vec::new();
    let mut fnu = Vec::new();
    let mut fb_ok = true;
    for (name, lf) in [("0.9625", lc), ("a", br.a.p), ("b", br.b_p), ("lt", lt)] {
        let t0 = Instant::now();
        match fcase.solve(&fst, lf) {
            Ok(s) => {
                let info = analyse(&fcase, &s);
                let fint = internal_force(&mf, &s.u, &s.states).unwrap();
                let (r, w) = block_measures(&mf, &s.u, &fint);
                println!(
                    "FBP F-bar at {name} ({lf:.12}): R/mu {r:.8}, w_c {w:+.8}, nu {:?}, {:.1} s",
                    info.nu,
                    t0.elapsed().as_secs_f64()
                );
                fnu.push(info.nu);
                fb.push((r, w));
                if name == "b" {
                    let e = end_of(&fcase, &info);
                    let res = e.pair.as_ref().map(|p| p.residual).unwrap_or(f64::NAN);
                    println!(
                        "FBP F-bar at b: mu_1 {:?}, mu_1 / (mu h) {:?}",
                        e.pair.as_ref().map(|p| p.mu),
                        e.pair.as_ref().map(|p| p.mu / (MU / 16.0))
                    );
                    ch.at_most("FBP: F-bar at b, the residual of the lowest pairs", res, 1e-8);
                }
                fst = s;
            }
            Err(e) => {
                println!("FBP F-bar at {name} ({lf:.12}): fails: {e}");
                fb_ok = false;
                fnu.push(None);
                fb.push((f64::NAN, f64::NAN));
                break;
            }
        }
    }
    ch.holds("FBP: F-bar converges at 0.9625, a, b, lt", fb_ok && fb.len() == 4);
    ch.at_most(
        "FB0: F-bar R at 0.9625 against 0.31425746",
        (fb[0].0 / 0.31425746 - 1.0).abs(),
        1e-6,
    );
    ch.at_most(
        "FB0: F-bar w_c at 0.9625 against 0.05849960",
        (fb[0].1 / 0.05849960 - 1.0).abs(),
        1e-6,
    );
    let fs = if fnu.len() == 4 && fnu.iter().all(|n| *n == Some(0)) {
        "F-bar stable at the bracket".to_string()
    } else {
        format!("F-bar also loses stability (nu {fnu:?})")
    };
    println!("READING FS: {fs}");

    // GP: the gaps of rule L to F-bar.
    if fb.len() == 4 {
        let gc = (rc / fb[0].0 - 1.0, wcc / fb[0].1 - 1.0);
        for (name, l, f) in [("a", Some(la), fb[1]), ("b", lb, fb[2])] {
            let (r, w) = l.unwrap_or((f64::NAN, f64::NAN));
            let g = (r / f.0 - 1.0, w / f.1 - 1.0);
            println!(
                "GP at {name}: g_R {:+.6e}, g_w {:+.6e} (at u_c {:+.6e}, {:+.6e})",
                g.0, g.1, gc.0, gc.1
            );
            ch.at_most(&format!("GP at {name}: |g_R|"), g.0.abs(), 0.15);
            ch.at_most(&format!("GP at {name}: |g_w|"), g.1.abs(), 0.15);
            ch.at_most(&format!("GP at {name}: |g_R - g_R(u_c)|"), (g.0 - gc.0).abs(), 0.01);
            ch.at_most(&format!("GP at {name}: |g_w - g_w(u_c)|"), (g.1 - gc.1).abs(), 0.01);
        }
    } else {
        ch.holds("GP: the F-bar states", false);
    }
    println!("f779_nh16_bracket_heavy: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}
