//! F-783 (ADR 0700): the bifurcated branch of rule L's 16^3 punch path. At the symmetry-breaking bifurcation of ADR 0697 on
//! the punch block (rule L, neo-Hookean, `16^3`): the cubic coefficient of the reduced bifurcation equation from the forced
//! response along the critical mode on the stable side (D2); a branch switch along `+-phi` with the indefinite-tangent Newton
//! at fixed loads, on the side the reading puts the branch, and on the other side (D3); the trace of the bifurcated branch with
//! its inertia, `R/mu` and `w_c` against the symmetric branch and F-bar (D4). The procedures are written once over a branch
//! case and run by the fast test on closed-form super- and subcritical pitchforks, on a fold whose Newton converges to a
//! distant root (the continuity test of ADR 0697) and on the stable `8^3` block (D5). The helpers are copies of those of
//! `si_eas_hg15.rs` (existing tests are not edited). Every aggregated value is required finite before it is compared.

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

/// PT: `r_0 = (1 - p) u_0 + b3 u_0^3 + 2 c u_0 u_1`, `r_1 = 2 u_1 - 0.1 p + c u_0^2`, `r_i = k_i u_i - p b_i` (`i = 2..7`).
struct Pitch {
    b3: f64,
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
                0 => (1.0 - p) * u[0] + self.b3 * u[0].powi(3) + 2.0 * PT_C * u[0] * u[1],
                1 => 2.0 * u[1] - 0.1 * p + PT_C * u[0] * u[0],
                _ => k_i(i) * u[i] - p * b_i(i),
            })
            .collect()
    }
    fn dense(&self, u: &[f64], p: f64) -> Vec<f64> {
        let mut a = vec![0.0; TOY_N * TOY_N];
        a[0] = 1.0 - p + 3.0 * self.b3 * u[0] * u[0] + 2.0 * PT_C * u[1];
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
                _ => b_i(i),
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

const TOY_N: usize = 8;

/// C5: `r_0 = u_0 e^-u_0 + u_0 / 20 - p`, `r_i = k_i u_i - p b_i`: a fold with a distant root of the same inertia.
struct Dist;

impl Dist {
    fn g(u: f64) -> f64 {
        u * (-u).exp() + u / 20.0
    }
    fn dg(u: f64) -> f64 {
        (1.0 - u) * (-u).exp() + 1.0 / 20.0
    }
    fn residual(u: &[f64], p: f64) -> Vec<f64> {
        (0..TOY_N)
            .map(|i| match i {
                0 => Dist::g(u[0]) - p,
                _ => k_i(i) * u[i] - p * b_i(i),
            })
            .collect()
    }
    fn diag(u: &[f64]) -> Vec<f64> {
        (0..TOY_N)
            .map(|i| match i {
                0 => Dist::dg(u[0]),
                _ => k_i(i),
            })
            .collect()
    }
    /// The fold: the root of `g'` on `[0.5, 2]` by 200 bisections, and `g` there.
    fn fold() -> f64 {
        let (mut lo, mut hi) = (0.5, 2.0);
        for _ in 0..200 {
            let m = 0.5 * (lo + hi);
            if Dist::dg(m) > 0.0 {
                lo = m;
            } else {
                hi = m;
            }
        }
        Dist::g(lo)
    }
}

impl Case for Dist {
    fn solve(&self, from: &St, to: f64) -> Result<St, String> {
        let mut u = from.u.clone();
        for it in 0..=20 {
            let r = Dist::residual(&u, to);
            if r.iter().chain(&u).any(|v| !v.is_finite()) {
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
            let d = Dist::diag(&u);
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
        let d = Dist::diag(&s.u);
        Sys {
            k: CsrMatrix::from_triplets(TOY_N, d.iter().enumerate().map(|(i, &v)| (i, i, v)).collect()),
            f: (0..TOY_N).map(|i| if i == 0 { 1.0 } else { b_i(i) }).collect(),
            free: (0..TOY_N).collect(),
        }
    }
    fn lowest_pair(&self, s: &Sys, _nu: usize) -> Option<Pair> {
        dense_lowest(&s.k)
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

struct Level {
    eta: f64,
    a_plus: f64,
    a_minus: f64,
    a: f64,
    ks: f64,
    z: f64,
    beta: f64,
    window: bool,
    /// The symmetric part `(u_+ + u_-) / 2 - u_s` on the free equations.
    sym: Vec<f64>,
}

struct Ladder {
    k0: f64,
    levels: Vec<Level>,
    window: usize,
    beta: f64,
    w2: Option<Vec<f64>>,
    reading: &'static str,
    zmax: f64,
}

const LADDER_A0: f64 = 2e-4;
const LADDER_LEVELS: usize = 9;

/// D2: the forced response `r(u, p) = eta phi` at the fixed load of `us`, `eta = +- mu a0 2^j`, `j < levels`.
fn forced<B: Branch>(case: &B, tag: &str, us: &St, phi: &[f64], mu: f64, a0: f64, levels: usize) -> Ladder {
    let free = case.free();
    let base = amp(phi, &free, &us.u);
    let mut sides: Vec<Vec<St>> = Vec::new();
    for sign in [1.0, -1.0] {
        let mut cur = us.clone();
        let mut got = Vec::new();
        for j in 0..levels {
            let eta = sign * mu * a0 * 2f64.powi(j as i32);
            let t0 = Instant::now();
            match case.fixed(&cur, us.p, eta, phi) {
                Ok(s) => {
                    println!(
                        "{tag} forced eta {eta:+.6e}: a {:+.9e}, {:.1} s",
                        amp(phi, &free, &s.u) - base,
                        t0.elapsed().as_secs_f64()
                    );
                    cur = s.clone();
                    got.push(s);
                }
                Err(e) => {
                    println!("{tag} forced eta {eta:+.6e}: fails: {e}");
                    break;
                }
            }
        }
        sides.push(got);
    }
    let mut lv: Vec<Level> = sides[0]
        .iter()
        .zip(&sides[1])
        .enumerate()
        .map(|(j, (sp, sm))| {
            let eta = mu * a0 * 2f64.powi(j as i32);
            let a_plus = amp(phi, &free, &sp.u) - base;
            let a_minus = amp(phi, &free, &sm.u) - base;
            let a = 0.5 * (a_plus - a_minus);
            let sym = free.iter().map(|&q| 0.5 * (sp.u[q] + sm.u[q]) - us.u[q]).collect();
            Level {
                eta,
                a_plus,
                a_minus,
                a,
                ks: eta / a,
                z: (a_plus + a_minus).abs() / (a_plus - a_minus).abs(),
                beta: f64::NAN,
                window: false,
                sym,
            }
        })
        .collect();
    let k0 = if lv.len() >= 2 {
        let (a0m, a1m) = (lv[0].a * lv[0].a, lv[1].a * lv[1].a);
        (lv[0].ks * a1m - lv[1].ks * a0m) / (a1m - a0m)
    } else {
        f64::NAN
    };
    for l in lv.iter_mut() {
        l.beta = (l.ks - k0) / (l.a * l.a);
        let dev = (l.ks / k0 - 1.0).abs();
        l.window = (1e-3..=0.5).contains(&dev);
    }
    let win: Vec<&Level> = lv.iter().filter(|l| l.window).collect();
    let reading = if win.len() >= 2 && win.iter().all(|l| l.beta > 0.0) {
        "stiffening"
    } else if win.len() >= 2 && win.iter().all(|l| l.beta < 0.0) {
        "softening"
    } else {
        "undecided"
    };
    let beta = win.first().map(|l| l.beta).unwrap_or(f64::NAN);
    let w2 = win.first().map(|l| l.sym.iter().map(|s| s / (l.a * l.a)).collect());
    let zmax = lv
        .iter()
        .fold(0.0f64, |w, l| if l.z.is_finite() { w.max(l.z) } else { f64::NAN });
    for l in &lv {
        println!(
            "{tag} level eta {:.6e}: a_+ {:+.9e}, a_- {:+.9e}, a {:.9e}, k_s {:.9e}, z {:.3e}, beta {:+.6e}, window {}",
            l.eta, l.a_plus, l.a_minus, l.a, l.ks, l.z, l.beta, l.window
        );
    }
    println!(
        "{tag} READING CF: {reading}; mu_1 {mu:.9e}, k_0 {k0:.9e}, beta^ {beta:+.6e}, {} window levels, z max {zmax:.3e}",
        win.len()
    );
    Ladder {
        k0,
        window: win.len(),
        beta,
        w2,
        reading,
        zmax,
        levels: lv,
    }
}

// ============================================================================ D3: the switch

#[derive(Clone, Copy, PartialEq, Debug)]
enum Outcome {
    B,
    S,
    F,
}

struct Attempted {
    eps: f64,
    second: bool,
    outcome: Outcome,
    a: f64,
    state: Option<St>,
}

/// One switch from `us + sign eps phi (+ eps^2 w2)` at the fixed load of `us`.
fn switch_one<B: Branch>(
    case: &B,
    tag: &str,
    us: &St,
    phi: &[f64],
    w2: Option<&[f64]>,
    eps: f64,
    sign: f64,
) -> Attempted {
    let free = case.free();
    let mut start = us.clone();
    for (k, &q) in free.iter().enumerate() {
        start.u[q] += sign * eps * phi[k] + w2.map_or(0.0, |w| eps * eps * w[k]);
    }
    let t0 = Instant::now();
    let (outcome, a, state) = match case.fixed(&start, us.p, 0.0, phi) {
        Ok(s) => {
            let a = amp(phi, &free, &s.u) - amp(phi, &free, &us.u);
            (if a.abs() > 1e-3 * eps { Outcome::B } else { Outcome::S }, a, Some(s))
        }
        Err(e) => {
            println!("{tag} switch fails: {e}");
            (Outcome::F, f64::NAN, None)
        }
    };
    println!(
        "{tag} switch at {:.12} from {} eps {:+.6e} ({}): {outcome:?}, a {a:+.9e}, {:.1} s",
        us.p,
        if sign > 0.0 { "+" } else { "-" },
        eps,
        if w2.is_some() { "second-order" } else { "plain" },
        t0.elapsed().as_secs_f64()
    );
    Attempted {
        eps,
        second: w2.is_some(),
        outcome,
        a,
        state,
    }
}

/// The registered `eps` ladder of D3 (factors of `a*_pred`).
const EPS_LADDER: [f64; 3] = [1.0, 2.0, 4.0];

struct Switch {
    a_pred: f64,
    /// The rung (index of the `eps` factor) and the attempt that found the branch, with its mirror switch.
    found: Option<(usize, Attempted, Attempted)>,
    mirror: f64,
    other: Vec<Attempted>,
    branch_hi: bool,
    cr: &'static str,
}

/// D3: the switch on the branch side by the reading CF, the mirror, the other side, the reading CR.
fn switch<B: Branch>(case: &B, tag: &str, lad: &Ladder, lo: (&St, f64), hi: (&St, f64), phi: &[f64]) -> Switch {
    let mut out = Switch {
        a_pred: f64::NAN,
        found: None,
        mirror: f64::NAN,
        other: Vec::new(),
        branch_hi: false,
        cr: "undecided",
    };
    if lad.reading == "undecided" {
        println!("{tag} READING CR: undecided (CF undecided, no switch)");
        return out;
    }
    let stiff = lad.reading == "stiffening";
    out.branch_hi = stiff;
    let ((bs, bmu), (os, _)) = if out.branch_hi { (hi, lo) } else { (lo, hi) };
    out.a_pred = (bmu.abs() / lad.beta.abs()).sqrt();
    println!(
        "{tag} branch side {:.12} (mu_1^sym {bmu:+.6e}): a*_pred {:.9e}",
        bs.p, out.a_pred
    );
    let w2 = lad.w2.as_deref();
    'rungs: for (r, f) in EPS_LADDER.iter().enumerate() {
        let eps = f * out.a_pred;
        for second in [false, true] {
            let w = if second { w2 } else { None };
            if second && w.is_none() {
                continue;
            }
            let at = switch_one(case, tag, bs, phi, w, eps, 1.0);
            if at.outcome == Outcome::B {
                let minus = switch_one(case, tag, bs, phi, w, eps, -1.0);
                if let (Some(sp), Some(sm)) = (&at.state, &minus.state) {
                    let psp = case.mirror(&sp.u);
                    out.mirror = max_abs(sm.u.iter().zip(&psp).map(|(x, y)| x - y)) / max_abs(sp.u.iter().copied());
                }
                out.found = Some((r, at, minus));
                break 'rungs;
            }
        }
    }
    let eps1 = EPS_LADDER[0] * out.a_pred;
    for second in [false, true] {
        let w = if second { w2 } else { None };
        if second && w.is_none() {
            continue;
        }
        out.other
            .push(switch_one(case, &format!("{tag} other side"), os, phi, w, eps1, 1.0));
    }
    let other_ok = out.other.iter().all(|a| a.outcome != Outcome::B);
    out.cr = match (stiff, out.found.is_some() && other_ok) {
        (true, true) => "supercritical",
        (false, true) => "subcritical",
        _ => "undecided",
    };
    println!(
        "{tag} READING CR: {}; found {:?}, mirror {:.3e}, other side {:?}",
        out.cr,
        out.found.as_ref().map(|(r, a, _)| (*r, a.eps, a.second, a.a)),
        out.mirror,
        out.other.iter().map(|a| a.outcome).collect::<Vec<_>>()
    );
    out
}

// ============================================================================ D4: the trace

struct Trace {
    points: Vec<(St, Info)>,
    lost: Option<f64>,
}

const TRACE_HALVINGS: usize = 4;
const TRACE_CAP: usize = 28;

/// D4: the branch from `from` through the grid loads (upward by the case's solve, downward by the fixed-load Newton), each
/// step accepted if it converges and is continuous, halved at most `TRACE_HALVINGS` times per grid load, `TRACE_CAP` solves.
fn trace<B: Branch>(case: &B, tag: &str, from: (St, Info), grid: &[f64], up: bool) -> Trace {
    let free = case.free();
    let zero = vec![0.0; free.len()];
    let mut cur = from;
    let mut points = Vec::new();
    let mut solves = 0;
    for &target in grid {
        let mut tgt = target;
        let mut halvings = 0;
        loop {
            if solves >= TRACE_CAP {
                println!("{tag} trace: the cap of {TRACE_CAP} solves reached at {:.12}", cur.0.p);
                return Trace {
                    points,
                    lost: Some(cur.0.p),
                };
            }
            solves += 1;
            let t0 = Instant::now();
            let res = if up {
                case.solve(&cur.0, tgt)
            } else {
                case.fixed(&cur.0, tgt, 0.0, &zero)
            };
            let (acc, text) = match res {
                Ok(s) => match cur.1.vt.as_ref() {
                    Some(vt) if continuous(&cur.0, &free, vt, &s) => (Some(s), "accepted".to_string()),
                    _ => (None, "discontinuous".to_string()),
                },
                Err(e) => (None, format!("fails: {e}")),
            };
            println!(
                "{tag} trace solve {solves}: {:.12} -> {tgt:.12}: {text}, {:.1} s",
                cur.0.p,
                t0.elapsed().as_secs_f64()
            );
            match acc {
                Some(s) => {
                    let info = analyse(case, &s);
                    cur = (s, info);
                    if tgt == target {
                        points.push(cur.clone());
                        break;
                    }
                    tgt = target;
                }
                None => {
                    if halvings == TRACE_HALVINGS {
                        println!("{tag} trace: lost after {:.12}", cur.0.p);
                        return Trace {
                            points,
                            lost: Some(cur.0.p),
                        };
                    }
                    halvings += 1;
                    tgt = cur.0.p + 0.5 * (tgt - cur.0.p);
                }
            }
        }
    }
    Trace { points, lost: None }
}

// ============================================================================ the fast test

/// The symmetric state of PT at `p`.
fn pt_sym(t: &Pitch, p: f64) -> St {
    t.solve(
        &St {
            p,
            u: vec![0.0; TOY_N],
            states: Vec::new(),
        },
        p,
    )
    .unwrap()
}

/// PT-S (`b3 = 2`, `beta = +1`) or PT-U (`b3 = 0.5`, `beta = -0.5`).
fn pt_case(ch: &mut Checks, tag: &str, b3: f64) {
    let t = Pitch { b3 };
    let beta = b3 - PT_C * PT_C;
    let sup = beta > 0.0;
    let (lf, ap, lt) = (1.0, 1.11, 1.2);
    let (sf, sa, sh) = (pt_sym(&t, lf), pt_sym(&t, ap), pt_sym(&t, lt));
    let (_, pf) = mode_at(&t, tag, &sf);
    let (_, pa) = mode_at(&t, tag, &sa);
    let (_, ph) = mode_at(&t, tag, &sh);
    let (Some(pf), Some(pa), Some(ph)) = (pf, pa, ph) else {
        ch.holds(&format!("{tag}: the lowest pairs"), false);
        return;
    };
    ch.at_least(&format!("{tag}: |phi_f,0|"), pf.phi[0].abs(), 1.0 - 1e-12);
    let alpha = (pa.mu - pf.mu) / (ap - lf);
    let lc = ap + pa.mu / alpha.abs();
    ch.at_most(&format!("{tag}: alpha^ against -0.9"), (alpha / -0.9 - 1.0).abs(), 1e-9);
    ch.at_most(&format!("{tag}: lambda_c^ against 1/0.9"), (lc - 1.0 / 0.9).abs(), 1e-9);
    let lad = forced(&t, tag, &sf, &pf.phi, pf.mu, LADDER_A0, LADDER_LEVELS);
    ch.at_most(&format!("{tag}: k_0 against 0.1"), (lad.k0 / 0.1 - 1.0).abs(), 1e-9);
    ch.at_most(&format!("{tag}: z max"), lad.zmax, 1e-12);
    ch.holds(&format!("{tag}: window levels >= 2 ({})", lad.window), lad.window >= 2);
    ch.at_most(
        &format!("{tag}: beta^ against {beta}"),
        (lad.beta / beta - 1.0).abs(),
        1e-6,
    );
    let cf = if sup { "stiffening" } else { "softening" };
    ch.holds(&format!("{tag}: CF '{cf}' ({})", lad.reading), lad.reading == cf);
    let sw = switch(&t, tag, &lad, (&sf, pf.mu), (&sh, ph.mu), &pf.phi);
    let (ps, cf_a2) = if sup {
        (lt, (0.9 * lt - 1.0) / beta)
    } else {
        (lf, (0.9 * lf - 1.0) / beta)
    };
    match &sw.found {
        Some((r, at, _)) => {
            ch.equals(&format!("{tag}: B at the first eps (rung)"), *r, 0);
            let a = at.a.abs();
            ch.at_most(
                &format!("{tag}: a_s against the closed form at {ps}"),
                (a / cf_a2.sqrt() - 1.0).abs(),
                1e-10,
            );
            ch.at_most(&format!("{tag}: the mirror"), sw.mirror, 1e-12);
            let ratio = a / sw.a_pred;
            ch.holds(
                &format!("{tag}: a_s / a*_pred {ratio:.6} in [0.5, 2]"),
                (0.5..=2.0).contains(&ratio),
            );
            let st = at.state.as_ref().unwrap();
            let info = analyse(&t, st);
            ch.equals(
                &format!("{tag}: nu on the branch"),
                info.nu.unwrap_or(usize::MAX),
                if sup { 0 } else { 1 },
            );
            let grid: Vec<f64> = if sup {
                vec![1.25, 1.3, 1.35, 1.4]
            } else {
                vec![0.95, 0.9, 0.85, 0.8]
            };
            let tr = trace(&t, tag, (st.clone(), info), &grid, sup);
            ch.holds(
                &format!("{tag}: the trace, every step accepted ({} points)", tr.points.len()),
                tr.lost.is_none() && tr.points.len() == grid.len(),
            );
            let sgn = at.a.signum();
            for (s, i) in &tr.points {
                let a = sgn * s.u[0];
                let exact = ((0.9 * s.p - 1.0) / beta).sqrt();
                ch.at_most(
                    &format!("{tag}: a({}) against the closed form", s.p),
                    (a / exact - 1.0).abs(),
                    1e-10,
                );
                ch.equals(
                    &format!("{tag}: nu at {}", s.p),
                    i.nu.unwrap_or(usize::MAX),
                    if sup { 0 } else { 1 },
                );
            }
        }
        None => ch.holds(&format!("{tag}: the switch finds the branch"), false),
    }
    ch.holds(
        &format!(
            "{tag}: the other side S or F ({:?})",
            sw.other.iter().map(|a| a.outcome).collect::<Vec<_>>()
        ),
        !sw.other.is_empty() && sw.other.iter().all(|a| a.outcome != Outcome::B),
    );
    let cr = if sup { "supercritical" } else { "subcritical" };
    ch.holds(&format!("{tag}: CR '{cr}' ({})", sw.cr), sw.cr == cr);
}

#[test]
fn f783_fast() {
    let mut ch = Checks::new();
    let t_all = Instant::now();

    // PT-S, PT-U: the closed-form pitchforks.
    pt_case(&mut ch, "PT-S", 2.0);
    pt_case(&mut ch, "PT-U", 0.5);

    // C5: the continuity test against a distant root.
    let lo = Dist
        .solve(
            &St {
                p: 0.2,
                u: vec![0.0; TOY_N],
                states: Vec::new(),
            },
            0.2,
        )
        .unwrap();
    let br = bracket(&Dist, "C5", lo, 0.7, 20);
    let rd = read(&Dist, "C5", &br);
    println!(
        "C5: nu at the lower end {:?}; cos(a) {:.6e}; the upper end {}; DS {:e}",
        br.nu0,
        rd.ea.cos,
        if rd.eb.is_some() { "measured" } else { "lost" },
        rd.ds
    );
    let u0 = br.hi0.as_ref().map(|h| h.0.u[0]).unwrap_or(f64::NAN);
    ch.holds(
        &format!("C5: the first attempt converged and rejected by the continuity test (u_0 {u0:.6})"),
        br.hi0.is_some() && !br.hi0_accepted,
    );
    ch.at_least("C5: |u_0| of the first attempt", u0.abs(), 5.0);
    let pc = Dist::fold();
    ch.holds(&format!("C5: p_c {pc:.12} in [a, b]"), br.a.p <= pc && pc <= br.b_p);
    ch.at_most("C5: b - a", br.b_p - br.a.p, 0.5 / 2f64.powi(20) * (1.0 + 1e-9));
    ch.holds("C5: the upper end lost", br.b.is_none());
    ch.holds(
        &format!("C5: reading 'limit point' ({})", rd.text),
        rd.text == "limit point",
    );

    // N8: the FEM plumbing on the stable 8^3 block.
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
            let (_, pair) = mode_at(&fb8, "N8", &s1);
            match pair {
                Some(p) => {
                    let n = s1.u.len();
                    let full = expand(&fb8.free, n, &p.phi);
                    let pf = fb8.mirror(&full);
                    let sigma = dot(&full, &pf) / dot(&full, &full);
                    println!("N8: sigma(phi) {sigma:+.12e}");
                    let lad = forced(&fb8, "N8", &s1, &p.phi, p.mu, LADDER_A0, 2);
                    ch.at_most("N8: k_0 against mu_1", (lad.k0 / p.mu - 1.0).abs(), 1e-3);
                    if (sigma + 1.0).abs() <= 1e-6 {
                        ch.at_most("N8: z max", lad.zmax, 1e-6);
                    }
                    let at = switch_one(&fb8, "N8", &s1, &p.phi, None, 1e-3, 1.0);
                    ch.holds(
                        &format!("N8: the switch returns S ({:?})", at.outcome),
                        at.outcome == Outcome::S,
                    );
                }
                None => ch.holds("N8: the lowest pair at 1.0", false),
            }
        }
        Err(e) => ch.holds(&format!("N8: the solve to 1.0 ({e})"), false),
    }
    println!("f783_fast: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}

// ============================================================================ the heavy record

const LF: f64 = 0.9647;
const A_P: f64 = 0.964899444580078;
const B_P: f64 = 0.9649002075195312;

#[test]
#[ignore = "ADR 0700: the heavy record of the bifurcated branch on 16^3 (D2 to D4), run once with --ignored"]
fn f783_nh16_branch_heavy() {
    let mut ch = Checks::new();
    let t_all = Instant::now();
    let mesh16 = block_mesh(16);
    let m = block_model(&mesh16, K_L);
    let m2 = block_model_free_punch(&mesh16, K_L);
    let states = initial_states(&m).unwrap();
    let ndof = m.dof_map().n_equations();
    let fb = FemBranch::new(&m, &m2, 16);
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
    let att = &log.attempts[fa];
    let (lc, lt) = (att.committed, att.attempted);
    let uc = St {
        p: lc,
        u: att.iterates[0].clone(),
        states: states.clone(),
    };
    let mut grid = vec![0.963, 0.9635, 0.964, 0.9645, LF, A_P, lt];
    grid.extend([0.97, 0.975, 0.98, 0.985, 0.99, 0.995, 1.0]);

    // HS1 to HS4: the symmetric chain.
    let mut sym: Vec<(St, Info)> = vec![(uc.clone(), analyse(&fb, &uc))];
    let mut hs1 = true;
    for &p in &grid {
        let t0 = Instant::now();
        match fb.solve(&sym.last().unwrap().0, p) {
            Ok(s) => {
                let i = analyse(&fb, &s);
                let (r, w) = at(&m, &s);
                println!(
                    "HS symmetric at {p:.12}: R/mu {r:.8}, w_c {w:+.8}, nu {:?}, |u - P u|_inf / |u|_inf {:.3e}, {:.1} s",
                    i.nu,
                    asym(&s.u),
                    t0.elapsed().as_secs_f64()
                );
                sym.push((s, i));
            }
            Err(e) => {
                println!("HS symmetric at {p:.12}: fails: {e}");
                hs1 = false;
                break;
            }
        }
    }
    ch.holds("HS1: the symmetric chain converges at every grid load", hs1);
    let find = |v: &[(St, Info)], p: f64| v.iter().position(|(s, _)| s.p == p);
    let nu_of = |v: &[(St, Info)], p: f64| find(v, p).and_then(|k| v[k].1.nu).unwrap_or(usize::MAX);
    ch.equals("HS1: nu(lambda_f)", nu_of(&sym, LF), 0);
    ch.equals("HS1: nu(A)", nu_of(&sym, A_P), 0);
    ch.equals("HS1: nu(lt)", nu_of(&sym, lt), 1);
    if let Some(k) = find(&sym, lt) {
        let (r, w) = at(&m, &sym[k].0);
        ch.at_most("HS2: R at lt against 0.32765165", (r / 0.32765165 - 1.0).abs(), 1e-7);
        ch.at_most("HS2: w_c at lt against 0.06039238", (w / 0.06039238 - 1.0).abs(), 1e-7);
    } else {
        ch.holds("HS2: the symmetric state at lt", false);
    }
    if let Some(k) = find(&sym, 1.0) {
        let (r, w) = at(&m, &sym[k].0);
        ch.at_most("HS3: R at 1.0 against 0.33937627", (r / 0.33937627 - 1.0).abs(), 1e-6);
        ch.at_most("HS3: w_c at 1.0 against 0.06245308", (w / 0.06245308 - 1.0).abs(), 1e-6);
    } else {
        ch.holds("HS3: the symmetric state at 1.0", false);
    }
    let asym_max = sym[1..].iter().fold(0.0f64, |w, (s, _)| w.max(asym(&s.u)));
    ch.at_most(
        "HS4: |u - P u|_inf / |u|_inf, largest over the symmetric grid states",
        asym_max,
        1e-9,
    );
    let (Some(kf), Some(ka), Some(kt)) = (find(&sym, LF), find(&sym, A_P), find(&sym, lt)) else {
        ch.holds("HM: the symmetric states at lambda_f, A, lt", false);
        ch.finish();
        return;
    };

    // HM: the mode.
    let (_, pf) = mode_at(&fb, "HM", &sym[kf].0);
    let (_, pa) = mode_at(&fb, "HM", &sym[ka].0);
    let (_, ph) = mode_at(&fb, "HM", &sym[kt].0);
    let (Some(pf), Some(pa), Some(ph)) = (pf, pa, ph) else {
        ch.holds("HM: the lowest pairs at lambda_f, A, lt", false);
        ch.finish();
        return;
    };
    let full = expand(&fb.free, ndof, &pf.phi);
    let sigma = dot(&full, &fb.mirror(&full)) / dot(&full, &full);
    ch.at_most("HM: |sigma(phi_f) + 1|", (sigma + 1.0).abs(), 1e-6);
    ch.at_least("HM: |phi_f . phi_A|", dot(&pf.phi, &pa.phi).abs(), 0.99);
    ch.holds(&format!("HM: mu_1(lambda_f) {:e} > 0", pf.mu), pf.mu > 0.0);
    ch.holds(
        &format!("HM: mu_1(A) {:e} in (0, 10]", pa.mu),
        pa.mu > 0.0 && pa.mu <= 10.0,
    );
    ch.at_most(
        "HM: the backward residuals of the pairs",
        [pf.residual, pa.residual, ph.residual]
            .iter()
            .fold(0.0f64, |w, &r| w.max(r)),
        1e-8,
    );
    let alpha = (pa.mu - pf.mu) / (A_P - LF);
    let lcs = A_P + pa.mu / alpha.abs();
    println!(
        "HM: alpha^ {alpha:.6e}, lambda_c^ {lcs:.12}, mu_1^sym(lt) {:.6e}",
        ph.mu
    );
    ch.holds(
        &format!("HM: lambda_c^ {lcs:.12} in [A - 1e-7, B + 1e-7]"),
        (A_P - 1e-7..=B_P + 1e-7).contains(&lcs),
    );

    // HF: the forced response at lambda_f.
    let lad = forced(&fb, "HF", &sym[kf].0, &pf.phi, pf.mu, LADDER_A0, LADDER_LEVELS);
    ch.at_most("HF: k_0 against mu_1(lambda_f)", (lad.k0 / pf.mu - 1.0).abs(), 1e-3);
    let zl = lad
        .levels
        .iter()
        .fold(0.0f64, |w, l| if l.z.is_finite() { w.max(l.z) } else { f64::NAN });
    ch.at_most("HF: z, largest over the levels", zl, 1e-6);
    ch.holds(&format!("HF: window levels >= 2 ({})", lad.window), lad.window >= 2);
    ch.holds(
        &format!("HF: CF not undecided ({})", lad.reading),
        lad.reading != "undecided",
    );

    // HW: the switch.
    let sw = switch(&fb, "HW", &lad, (&sym[kf].0, pf.mu), (&sym[kt].0, ph.mu), &pf.phi);
    ch.holds(&format!("HW: CR not undecided ({})", sw.cr), sw.cr != "undecided");
    ch.holds(
        &format!(
            "HW: the other side S or F ({:?})",
            sw.other.iter().map(|a| a.outcome).collect::<Vec<_>>()
        ),
        !sw.other.is_empty() && sw.other.iter().all(|a| a.outcome != Outcome::B),
    );
    let Some((rung, found, _)) = &sw.found else {
        ch.holds("HW: the switch finds the branch", false);
        println!("f783_nh16_branch_heavy: {:.1} s", t_all.elapsed().as_secs_f64());
        ch.finish();
        return;
    };
    println!(
        "HW: found at rung {rung} (eps {:.6e}, second-order {})",
        found.eps, found.second
    );
    ch.at_most("HW: the mirror", sw.mirror, 1e-6);
    let a_s = found.a.abs();
    let ratio = a_s / sw.a_pred;
    ch.holds(
        &format!("HW: a_s / a*_pred {ratio:.6} in [0.5, 2]"),
        (0.5..=2.0).contains(&ratio),
    );
    let s0 = found.state.clone().unwrap();
    let (i0, _) = mode_at(&fb, "HW branch", &s0);
    let sup = sw.cr == "supercritical";
    ch.equals(
        "HW: nu on the branch",
        i0.nu.unwrap_or(usize::MAX),
        if sup { 0 } else { 1 },
    );

    // HT: the trace.
    let tgrid: Vec<f64> = if sw.branch_hi {
        vec![0.97, 0.975, 0.98, 0.985, 0.99, 0.995, 1.0]
    } else {
        vec![0.9645, 0.964, 0.9635, 0.963, lc]
    };
    let tr = trace(&fb, "HT", (s0.clone(), i0.clone()), &tgrid, sw.branch_hi);
    ch.holds(
        &format!(
            "HT: every grid load of the trace reached ({} of {})",
            tr.points.len(),
            tgrid.len()
        ),
        tr.lost.is_none() && tr.points.len() == tgrid.len(),
    );
    let mut branch: Vec<(St, Info)> = vec![(s0, i0)];
    branch.extend(tr.points.iter().cloned());
    let sgn = found.a.signum();
    let mut stays = true;
    let mut rows = Vec::new();
    for (s, i) in &branch {
        let Some(k) = find(&sym, s.p) else { continue };
        let a = sgn * (amp(&pf.phi, &fb.free, &s.u) - amp(&pf.phi, &fb.free, &sym[k].0.u));
        stays &= a >= 0.5 * a_s;
        let (r, w) = at(&m, s);
        let (rs, ws) = at(&m, &sym[k].0);
        println!(
            "HT branch at {:.12}: nu {:?}, a {a:+.9e}, |u - P u|_inf / |u|_inf {:.3e}, R/mu {r:.8}, w_c {w:+.8}; symmetric \
             nu {:?}, R/mu {rs:.8}, w_c {ws:+.8}",
            s.p,
            i.nu,
            asym(&s.u),
            sym[k].1.nu
        );
        rows.push((s.p, i.nu, a, r, w, sym[k].1.nu, rs, ws));
    }
    ch.holds("HT: |a| >= 0.5 |a_s| at every traced load", stays);
    let _ = mode_at(&fb, "HT last", &branch.last().unwrap().0);

    // HFB: F-bar at the same loads.
    let mf = block_model(&mesh16, K_FBAR);
    let mf2 = block_model_free_punch(&mesh16, K_FBAR);
    let fbf = FemBranch::new(&mf, &mf2, 16);
    let (ftext, flog) = solve_observed(&mesh16, "F-bar 16^3", K_FBAR, &options(10));
    ch.holds("HFB: the F-bar ten-increment solve passes", ftext.is_empty());
    let flast = flog.attempts.last().unwrap();
    let mut fst = St {
        p: flast.committed,
        u: flast.iterates[0].clone(),
        states: initial_states(&mf).unwrap(),
    };
    let mut fbar: Vec<(St, Info)> = Vec::new();
    let mut hfb = true;
    for &p in std::iter::once(&lc).chain(&grid) {
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
    match (find(&fbar, lc), find(&fbar, 1.0)) {
        (Some(k0), Some(k1)) => {
            let (r0, w0) = at(&mf, &fbar[k0].0);
            let (r1, w1) = at(&mf, &fbar[k1].0);
            ch.at_most(
                "HFB: R at 0.9625 against 0.31425746",
                (r0 / 0.31425746 - 1.0).abs(),
                1e-6,
            );
            ch.at_most(
                "HFB: w_c at 0.9625 against 0.05849960",
                (w0 / 0.05849960 - 1.0).abs(),
                1e-6,
            );
            ch.at_most("HFB: R at 1.0 against 0.32599497", (r1 / 0.32599497 - 1.0).abs(), 1e-6);
            ch.at_most(
                "HFB: w_c at 1.0 against 0.06060900",
                (w1 / 0.06060900 - 1.0).abs(),
                1e-6,
            );
        }
        _ => ch.holds("HFB: the F-bar states at 0.9625 and 1.0", false),
    }
    let fnu: Vec<Option<usize>> = fbar.iter().map(|(_, i)| i.nu).collect();
    println!("HFB: F-bar nu along {fnu:?}");

    // HG: the comparisons.
    for (p, nu, a, r, w, nus, rs, ws) in &rows {
        let Some(k) = find(&fbar, *p) else {
            ch.holds(&format!("HG: the F-bar state at {p}"), false);
            continue;
        };
        let (rf, wf) = at(&mf, &fbar[k].0);
        println!(
            "TABLE {p:.12}: bifurcated nu {nu:?} a {a:+.6e} R/mu {r:.8} w_c {w:+.8} | symmetric nu {nus:?} R/mu {rs:.8} w_c \
             {ws:+.8} | F-bar nu {:?} R/mu {rf:.8} w_c {wf:+.8} | g_R bif {:+.6e} sym {:+.6e}, g_w bif {:+.6e} sym {:+.6e}",
            fbar[k].1.nu,
            r / rf - 1.0,
            rs / rf - 1.0,
            w / wf - 1.0,
            ws / wf - 1.0
        );
        ch.at_most(&format!("HG at {p}: |R_bif / R_Fb - 1|"), (r / rf - 1.0).abs(), 0.15);
        ch.at_most(&format!("HG at {p}: |w_bif / w_Fb - 1|"), (w / wf - 1.0).abs(), 0.15);
        ch.at_most(&format!("HG at {p}: |R_sym / R_Fb - 1|"), (rs / rf - 1.0).abs(), 0.15);
        ch.at_most(&format!("HG at {p}: |w_sym / w_Fb - 1|"), (ws / wf - 1.0).abs(), 0.15);
    }
    println!(
        "READING CR 16^3: {}; the branch {} ({} loads traced)",
        sw.cr,
        if tr.lost.is_none() {
            "traced through its grid"
        } else {
            "lost"
        },
        tr.points.len()
    );
    println!("f783_nh16_branch_heavy: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}
