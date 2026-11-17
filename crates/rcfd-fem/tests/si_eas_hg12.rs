//! F-763 (ADR 0682): rule L of ADR 0630 loses positive definiteness on the refined neo-Hookean punch block of ADR 0636 (the
//! EC1 solve of ADR 0678 on `16^3`). The failing solve reproduced (RP0, RP1), the last converged state rebuilt by the
//! replicated path P (RP2), the lowest eigenvalue of the free tangent (E1) and of the ADR 0628 pencil (E2) located by element
//! (LC1, the located set, the edge share, the hourglass share), the reference element at the same state (CF), rule L on
//! `8^3` (C8), the inertia along the path (ON), the modes block of every element (KA), the classification of the registered
//! criterion (CL); S, W and finer increments (SW, FI); the fast controls on `8^3` and on synthetic fields (F1 to F7, NC-H,
//! NC-L) and the plain element as the positive control (PC1). The helpers are copies of those of `si_eas_hg11.rs` and
//! `si_eas_hg3.rs` (existing tests are not edited). Every aggregated value is required finite before it is compared.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{prescribed_equations, reduce};
use rcfd_fem::element::{element_for, ElementState};
use rcfd_fem::material::{HyperelasticEnergy, PointContext};
use rcfd_fem::nl_incompatible::{
    condensed_force_and_tangent, enhanced_force_and_tangent_with, modes_and_strains_with, IncompatibleStabilisation,
};
use rcfd_fem::nonlinear::{
    initial_states, solve, solve_from, solve_with, tangent, DriverExtensions, LoadStep, NewtonOptions, NonlinearResult,
    StepControl,
};
use rcfd_fem::solve::DirectFactor;
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Material, SolidFormulation, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{lowest_eigenpairs, symmetric_eigen, Factorisation, GeneralizedEigen, Shift, SubspaceOptions};
use rcfd_linalg::skyline::{PivotRule, SkylineError};
use rcfd_linalg::sparse_chol::{SparseOptions, SparseOrdering, SupernodalAnalysis, SupernodalLdlt};
use rcfd_linalg::CsrMatrix;
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

fn element_coords(mesh: &SolidMesh, e: usize) -> Vec<Vec3> {
    mesh.element_nodes(e).iter().map(|&n| mesh.nodes()[n]).collect()
}

/// The element unknowns of `u` (node-major, three per node).
fn element_u(model: &SolidModel, u: &[f64], e: usize) -> Vec<f64> {
    let dofs = model.dof_map();
    model
        .mesh()
        .element_nodes(e)
        .iter()
        .flat_map(|&n| (0..3).map(move |c| (n, c)))
        .map(|(n, c)| u[dofs.equation(n, c).unwrap()])
        .collect()
}

/// The reference corners of the Hex8 (node order of `hex_box`).
const XI: [[f64; 3]; 8] = [
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
    [1.0, 1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, 1.0],
    [-1.0, 1.0, 1.0],
];

// ============================================================================ the materials and the elements (ADR 0678)

const MU: f64 = 1.0e6;
const NU: f64 = 0.49;
const INC: SolidFormulation = SolidFormulation::Incompatible;
const FBAR: SolidFormulation = SolidFormulation::BBar;
const FULL: SolidFormulation = SolidFormulation::Full;
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

const K_FBAR: Kind = Kind {
    name: "Fbar",
    formulation: FBAR,
    rule: OFF,
};
const K_L: Kind = Kind {
    name: "L",
    formulation: INC,
    rule: LIN,
};
const K_OFF: Kind = Kind {
    name: "Off",
    formulation: INC,
    rule: OFF,
};
const K_FULL: Kind = Kind {
    name: "Full",
    formulation: FULL,
    rule: OFF,
};

/// The opt-ins of the incompatible-mode update (ADR 0655 D1, ADR 0658 D1, ADR 0660 D1).
#[derive(Clone, Copy)]
struct Opts {
    name: &'static str,
    backtracking: bool,
    decrease: bool,
    warm: bool,
}

const DEFAULT: Opts = Opts {
    name: "default",
    backtracking: false,
    decrease: false,
    warm: false,
};
/// The variant S of ADR 0658 (cold).
const S: Opts = Opts {
    name: "S",
    backtracking: true,
    decrease: true,
    warm: false,
};
/// The remedy W of ADR 0660.
const W: Opts = Opts {
    name: "W",
    backtracking: true,
    decrease: true,
    warm: true,
};

const SUPERNODAL: DirectFactor = DirectFactor::SupernodalNestedDissection;

fn with_formulation(mut mesh: SolidMesh, f: SolidFormulation) -> SolidMesh {
    if f != SolidFormulation::Full {
        mesh.set_block_property(0, BlockProperty::solid(f)).unwrap();
    }
    mesh
}

fn model_of(mesh: SolidMesh, kind: Kind, opts: Opts) -> SolidModel {
    let mut m = SolidModel::new(with_formulation(mesh, kind.formulation));
    m.set_incompatible_stabilisation(kind.rule);
    m.set_incompatible_mode_backtracking(opts.backtracking);
    m.set_incompatible_mode_decrease(opts.decrease);
    m.set_incompatible_mode_warm_start(opts.warm);
    m.add_material("all", neo_hookean()).unwrap();
    m
}

// ============================================================================ the punch block (ADR 0678 reading c1)

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

/// The punch, corner and edge-row sets (by coordinates).
fn add_sets(mesh: &mut SolidMesh) {
    let punch = nodes_where(mesh, "zmax", |p| p[0] <= 0.5 + TOL && p[1] <= 0.5 + TOL);
    let edge: Vec<usize> = punch
        .iter()
        .copied()
        .filter(|&n| {
            let p = mesh.nodes()[n];
            (p[0] - 0.5).abs() < TOL || (p[1] - 0.5).abs() < TOL
        })
        .collect();
    let corner = nodes_where(mesh, "zmax", |p| (p[0] - 1.0).abs() < TOL && (p[1] - 1.0).abs() < TOL);
    mesh.add_node_set("punch", punch).unwrap();
    mesh.add_node_set("edge", edge).unwrap();
    mesh.add_node_set("corner", corner).unwrap();
}

fn block_mesh(n: usize) -> SolidMesh {
    let mut mesh = hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap();
    add_sets(&mut mesh);
    mesh
}

/// The model of the block with the punch at `punch` (the die constraints of reading c1).
fn block_model(mesh: &SolidMesh, kind: Kind, opts: Opts, punch: f64) -> SolidModel {
    let mut m = model_of(mesh.clone(), kind, opts);
    for (set, comp, value) in [
        ("xmin", 0, 0.0),
        ("xmax", 0, 0.0),
        ("ymin", 1, 0.0),
        ("ymax", 1, 0.0),
        ("zmin", 2, 0.0),
        ("punch", 2, punch),
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

/// `R = -sum_punch r_z / mu` and `w_c` of a driver result.
fn measures(m: &SolidModel, r: &NonlinearResult) -> (f64, f64) {
    let re = r.result.reactions().unwrap();
    let dofs = r.result.dofs();
    let punch = m.mesh().node_set("punch").unwrap();
    let rz: f64 = punch.iter().map(|&n| re[dofs.equation(n, 2).unwrap()]).sum();
    let corner = m.mesh().node_set("corner").unwrap()[0];
    (-rz / MU, r.result.displacement(corner)[2])
}

/// The error text of the EC1 solve of ADR 0678 (`f747-scratch/run3-heavy.log`, line 23).
const ADR0678_TEXT: &str = "solver newton: load step 10 of 10 (load factor 9.625e-1 to 9.656250000000001e-1) after 5 \
                            halvings: singular tangent: pivot -4.651826504388123e6 at node 4768 component 0 (diagonal \
                            2.8253460477227545e6)";
const ADR0678_PIVOT: f64 = -4.651826504388123e6;

/// RP0: the solve of ADR 0678 (`solve_block` of `si_eas_hg11.rs` with ten increments): the error text, or the measures.
fn solve_plain(mesh: &SolidMesh, tag: &str, kind: Kind, opts: Opts) -> Result<(f64, f64), String> {
    let m = block_model(mesh, kind, opts, -DELTA);
    let t0 = Instant::now();
    let out = solve(&m, &[], &[], &options(10));
    let secs = t0.elapsed().as_secs_f64();
    match out {
        Ok(r) => {
            let (rz, wc) = measures(&m, &r);
            println!(
                "{tag} NH {} {} plain: R/mu {rz:.8}, w_c {wc:+.8}, {secs:.1} s",
                kind.name, opts.name
            );
            Ok((rz, wc))
        }
        Err(e) => {
            println!(
                "{tag} NH {} {} plain: solve failed: {e}, {secs:.1} s",
                kind.name, opts.name
            );
            Err(e.to_string())
        }
    }
}

/// The recording hook of reading c1: returns the driver's proposed load factor unchanged and records every attempt.
#[derive(Default)]
struct Recorder {
    pending: (f64, f64),
    /// `(committed, attempted, converged)` of every attempt in order.
    log: Vec<(f64, f64, bool)>,
}

impl StepControl for Recorder {
    fn name(&self) -> &str {
        "recorder"
    }
    fn load_factor(&mut self, committed: f64, proposed: f64, _step: usize, _halvings: usize) -> rcfd_fem::Result<f64> {
        self.pending = (committed, proposed);
        Ok(proposed)
    }
    fn converged(&mut self, increment: &LoadStep) {
        self.log.push((self.pending.0, increment.load_factor, true));
    }
    fn failed(&mut self, committed: f64, attempted: f64) {
        self.log.push((committed, attempted, false));
    }
}

/// A hooked solve: the outcome (measures or error text), the converged load factors in order and the last converged one.
struct Hooked {
    outcome: Result<(f64, f64), String>,
    converged: Vec<f64>,
    last: f64,
}

fn solve_hooked(mesh: &SolidMesh, tag: &str, kind: Kind, opts: Opts, steps: usize) -> Hooked {
    let m = block_model(mesh, kind, opts, -DELTA);
    let mut rec = Recorder::default();
    let t0 = Instant::now();
    let out = solve_with(
        &m,
        &[],
        &[],
        &options(steps),
        DriverExtensions {
            contributions: Vec::new(),
            step_control: Some(&mut rec),
        },
    );
    let secs = t0.elapsed().as_secs_f64();
    let converged: Vec<f64> = rec.log.iter().filter(|a| a.2).map(|a| a.1).collect();
    let last = converged.last().copied().unwrap_or(0.0);
    let text: Vec<String> = rec
        .log
        .iter()
        .map(|(c, a, ok)| format!("{c:.6}->{a:.6}{}", if *ok { "" } else { " x" }))
        .collect();
    println!(
        "{tag} NH {} {} {steps} steps hooked: attempts [{}]",
        kind.name,
        opts.name,
        text.join(", ")
    );
    let outcome = match out {
        Ok(r) => {
            let (rz, wc) = measures(&m, &r);
            println!("{tag}: R/mu {rz:.8}, w_c {wc:+.8}, {secs:.1} s");
            Ok((rz, wc))
        }
        Err(e) => {
            println!("{tag}: solve failed: {e}; last converged load factor {last:.6}, {secs:.1} s");
            Err(e.to_string())
        }
    };
    Hooked {
        outcome,
        converged,
        last,
    }
}

/// A converged state of the replicated path P.
struct State {
    load: f64,
    u: Vec<f64>,
    states: Vec<ElementState>,
    fint: Vec<f64>,
}

/// One attempt of P by `solve_from` from `prev` to the load factor `l` (the lifted start of reading c1).
fn attempt(mesh: &SolidMesh, kind: Kind, opts: Opts, prev: &State, l: f64) -> Option<State> {
    let m = block_model(mesh, kind, opts, l * -DELTA);
    let dofs = m.dof_map();
    let mut start = prev.u.clone();
    for &n in m.mesh().node_set("punch").unwrap() {
        start[dofs.equation(n, 2).unwrap()] = l * -DELTA;
    }
    let load = vec![0.0; dofs.n_equations()];
    match solve_from(&m, &[], &load, &start, &prev.states, &options(10), &[]) {
        Ok(eq) => Some(State {
            load: l,
            u: eq.u,
            states: eq.states,
            fint: eq.internal_force,
        }),
        Err(e) => {
            println!("P {} at {l:.6}: attempt failed: {e}", kind.name);
            None
        }
    }
}

/// The replicated path P on the converged load factors: every converged state (the last first failing ends the path).
fn replicate(mesh: &SolidMesh, kind: Kind, opts: Opts, loads: &[f64]) -> Vec<State> {
    let m = block_model(mesh, kind, opts, 0.0);
    let n = m.dof_map().n_equations();
    let mut path = vec![State {
        load: 0.0,
        u: vec![0.0; n],
        states: initial_states(&m).unwrap(),
        fint: vec![0.0; n],
    }];
    let t0 = Instant::now();
    for &l in loads {
        let prev = path.last().unwrap();
        let next = attempt(mesh, kind, opts, prev, l).or_else(|| {
            let mid = prev.load + 0.5 * (l - prev.load);
            println!("P {} at {l:.6}: retried in two halves (reading c1)", kind.name);
            let half = attempt(mesh, kind, opts, prev, mid)?;
            attempt(mesh, kind, opts, &half, l)
        });
        match next {
            Some(s) => path.push(s),
            None => break,
        }
    }
    println!(
        "P {} on {} elements: reached {:.6} of {:.6}, {:.1} s",
        kind.name,
        mesh.n_elements(),
        path.last().unwrap().load,
        loads.last().copied().unwrap_or(0.0),
        t0.elapsed().as_secs_f64()
    );
    path.remove(0);
    path
}

/// `R`, `w_c` of a state of P (the reactions from the internal force: no external load).
fn state_measures(m: &SolidModel, s: &State) -> (f64, f64) {
    let dofs = m.dof_map();
    let punch = m.mesh().node_set("punch").unwrap();
    let rz: f64 = punch.iter().map(|&n| s.fint[dofs.equation(n, 2).unwrap()]).sum();
    let corner = m.mesh().node_set("corner").unwrap()[0];
    (-rz / MU, s.u[dofs.equation(corner, 2).unwrap()])
}

// ============================================================================ the free tangent and its spectrum

/// The reduced (free) tangent of a model at a state, as the driver reduces it; and its free equations.
fn free_tangent(m: &SolidModel, u: &[f64], states: &[ElementState]) -> (CsrMatrix, Vec<usize>) {
    let k = tangent(m, u, states, Some(4)).unwrap();
    let fixed = prescribed_equations(m, &[]).unwrap();
    let sys = reduce(&k, &vec![0.0; k.n], &fixed).unwrap();
    let free = sys.free().to_vec();
    (sys.matrix, free)
}

/// SY: `max |k_ij - k_ji| / sqrt(|k_ii k_jj|)`, and the symmetric part `(K + K^T) / 2`.
fn symmetrise(a: &CsrMatrix) -> (f64, CsrMatrix) {
    let d = a.diag();
    let mut worst = 0.0f64;
    let mut t = Vec::with_capacity(2 * a.val.len());
    for r in 0..a.n {
        for p in a.row_ptr[r]..a.row_ptr[r + 1] {
            let c = a.col[p];
            let s = a.col[a.row_ptr[c]..a.row_ptr[c + 1]]
                .binary_search(&r)
                .map(|q| a.val[a.row_ptr[c] + q])
                .unwrap_or(0.0);
            worst = worst.max((a.val[p] - s).abs() / (d[r].abs() * d[c].abs()).sqrt());
            t.push((r, c, 0.5 * a.val[p]));
            t.push((c, r, 0.5 * a.val[p]));
        }
    }
    (worst, CsrMatrix::from_triplets(a.n, t))
}

/// The inertia: the number of negative pivots of the supernodal `L D L^T` with the `NonZero` rule (Sylvester); `None` if
/// the factor refuses a pivot.
fn inertia(a: &CsrMatrix) -> Option<usize> {
    SupernodalLdlt::factor_with(
        a,
        &SparseOptions {
            ordering: SparseOrdering::NestedDissection,
            pivots: PivotRule::NonZero,
            symmetry_tolerance: 1e-12,
        },
    )
    .ok()
    .map(|f| f.inertia().0)
}

/// The driver's first factor of a reduced tangent (`factor_solve` of `nonlinear.rs`): `None` if it passes, else the free
/// row and the pivot of the first refusal.
fn driver_factor(a: &CsrMatrix) -> Option<(usize, f64)> {
    let analysis = SupernodalAnalysis::nested_dissection(a).unwrap();
    let diag = a.diag();
    match SupernodalLdlt::factor_analysed(a, &analysis) {
        Err(SkylineError::Pivot { row, pivot, .. }) => Some((row, pivot)),
        Err(e) => panic!("driver factor: {e:?}"),
        Ok(f) => f
            .order()
            .iter()
            .zip(f.pivots())
            .find(|(&row, &d)| d.is_nan() || d <= 1e-10 * diag[row])
            .map(|(&row, &d)| (row, d)),
    }
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

fn identity(n: usize) -> CsrMatrix {
    CsrMatrix::from_triplets(n, (0..n).map(|i| (i, i, 1.0)).collect())
}

/// The vector Laplacian of the Full Hex8 (2 x 2 x 2 rule) on the free block (`laplacian` of `si_eas_hg3.rs`, sparse).
fn laplacian_sparse(m: &SolidModel, free: &[usize]) -> CsrMatrix {
    let mut pos = vec![usize::MAX; m.dof_map().n_equations()];
    for (i, &e) in free.iter().enumerate() {
        pos[e] = i;
    }
    let el = element_for(ElementKind::Hex8);
    let dofs = m.dof_map();
    let mut t = Vec::new();
    for e in 0..m.mesh().n_elements() {
        let nodes = m.mesh().element_nodes(e);
        let coords = element_coords(m.mesh(), e);
        for ip in el.integration_points(&coords, el.stiffness_rule()).unwrap() {
            let g = &ip.geometry.grad;
            for (a, &na) in nodes.iter().enumerate() {
                for (b, &nb) in nodes.iter().enumerate() {
                    let v = ip.weight * (g[a][0] * g[b][0] + g[a][1] * g[b][1] + g[a][2] * g[b][2]);
                    for c in 0..3 {
                        let (r, s) = (pos[dofs.equation(na, c).unwrap()], pos[dofs.equation(nb, c).unwrap()]);
                        if r != usize::MAX && s != usize::MAX {
                            t.push((r, s, v));
                        }
                    }
                }
            }
        }
    }
    CsrMatrix::from_triplets(free.len(), t)
}

/// The four lowest pairs of `(K, M)` (reading c3: `Shift::Auto`, the supernodal factor, tolerance `1e-8`, 300 iterations).
fn lowest(k: &CsrMatrix, mm: &CsrMatrix, points: &[[f64; 3]]) -> Option<GeneralizedEigen> {
    let opts = SubspaceOptions {
        shift: Shift::Auto,
        tolerance: 1e-8,
        max_iterations: 300,
        factorisation: Factorisation::Supernodal,
        points: Some(points.to_vec()),
        ..SubspaceOptions::default()
    };
    let t0 = Instant::now();
    let out = lowest_eigenpairs(k, mm, 4, &opts);
    match out {
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

fn worst_residual(e: &Option<GeneralizedEigen>) -> f64 {
    e.as_ref()
        .map(|e| {
            e.residuals
                .iter()
                .fold(0.0f64, |w, &r| if r.is_finite() { w.max(r) } else { f64::NAN })
        })
        .unwrap_or(f64::NAN)
}

fn negatives_among(e: &Option<GeneralizedEigen>) -> usize {
    e.as_ref()
        .map(|e| e.values.iter().filter(|&&v| v < 0.0).count())
        .unwrap_or(usize::MAX)
}

fn quadratic(a: &CsrMatrix, x: &[f64]) -> f64 {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    x.iter().zip(&y).map(|(p, q)| p * q).sum()
}

fn expand(free: &[usize], n: usize, x: &[f64]) -> Vec<f64> {
    let mut v = vec![0.0; n];
    for (&e, &xi) in free.iter().zip(x) {
        v[e] = xi;
    }
    v
}

// ============================================================================ located by element

/// The hourglass share `h_e` of an element vector (24 values, node-major; reading "located by element").
fn hourglass_share(x: &[f64]) -> f64 {
    let (mut num, mut den) = (0.0, 0.0);
    for c in 0..3 {
        let v: [f64; 8] = std::array::from_fn(|a| x[3 * a + c]);
        let mean = v.iter().sum::<f64>() / 8.0;
        let coef: [f64; 3] = std::array::from_fn(|k| (0..8).map(|a| XI[a][k] * v[a]).sum::<f64>() / 8.0);
        for a in 0..8 {
            let lin = mean + coef[0] * XI[a][0] + coef[1] * XI[a][1] + coef[2] * XI[a][2];
            num += (v[a] - lin).powi(2);
            den += (v[a] - mean).powi(2);
        }
    }
    if den > 0.0 {
        num / den
    } else {
        0.0
    }
}

/// `|x_e - P_0 x_e|^2` of an element vector.
fn spread(x: &[f64]) -> f64 {
    (0..3)
        .map(|c| {
            let mean = (0..8).map(|a| x[3 * a + c]).sum::<f64>() / 8.0;
            (0..8).map(|a| (x[3 * a + c] - mean).powi(2)).sum::<f64>()
        })
        .sum()
}

/// The punch-edge elements `E`: the elements with a node of the edge row (all on the top face).
fn edge_elements(mesh: &SolidMesh) -> Vec<bool> {
    let edge = mesh.node_set("edge").unwrap();
    (0..mesh.n_elements())
        .map(|e| mesh.element_nodes(e).iter().any(|n| edge.contains(n)))
        .collect()
}

/// The element tangents (condensed for the incompatible element) of a model at a state.
fn element_tangents(m: &SolidModel, kind: Kind, u: &[f64], states: &[ElementState]) -> Vec<Vec<f64>> {
    let mat = neo_hookean();
    (0..m.mesh().n_elements())
        .map(|e| {
            let coords = element_coords(m.mesh(), e);
            let ue = element_u(m, u, e);
            let (_, k, _) =
                condensed_force_and_tangent(&coords, &mat, &ue, &states[e], &PointContext::default(), kind.rule)
                    .unwrap();
            (0..576).map(|p| k.get(p / 24, p % 24)).collect()
        })
        .collect()
}

/// The location of a mode (reading "located by element").
struct Location {
    sum: f64,
    sum_abs: f64,
    set: usize,
    first: usize,
    edge_share: f64,
    hourglass: f64,
    stretch: f64,
}

fn grid(e: usize, n: usize) -> (usize, usize, usize) {
    (e % n, (e / n) % n, e / (n * n))
}

/// The smallest principal stretch at the centre of an element (the compatible gradient of the trilinear field).
fn centre_stretch(coords: &[Vec3], ue: &[f64]) -> f64 {
    let h: [f64; 3] = std::array::from_fn(|k| coords[6][k] - coords[0][k]);
    let mut f = [[0.0; 3]; 3];
    for (a, s) in XI.iter().enumerate() {
        for i in 0..3 {
            for j in 0..3 {
                f[i][j] += ue[3 * a + i] * s[j] / (4.0 * h[j]);
            }
        }
    }
    for (i, row) in f.iter_mut().enumerate() {
        row[i] += 1.0;
    }
    let c: Vec<f64> = (0..9)
        .map(|p| (0..3).map(|k| f[k][p / 3] * f[k][p % 3]).sum())
        .collect();
    symmetric_eigen(&c, 3).unwrap().values[0].sqrt()
}

fn locate(m: &SolidModel, tangents: &[Vec<f64>], x: &[f64], negative: bool, edge: &[bool], u: &[f64]) -> Location {
    let ne = m.mesh().n_elements();
    let mut energies = vec![0.0; ne];
    for (e, ke) in tangents.iter().enumerate() {
        let xe = element_u(m, x, e);
        let mut s = 0.0;
        for i in 0..24 {
            for j in 0..24 {
                s += xe[i] * ke[24 * i + j] * xe[j];
            }
        }
        energies[e] = s;
    }
    let w: Vec<f64> = energies
        .iter()
        .map(|&v| if negative { (-v).max(0.0) } else { v.abs() })
        .collect();
    let total: f64 = w.iter().sum();
    let mut order: Vec<usize> = (0..ne).collect();
    order.sort_by(|&a, &b| w[b].total_cmp(&w[a]).then(a.cmp(&b)));
    let (mut acc, mut set) = (0.0, 0);
    let (mut hw, mut ww) = (0.0, 0.0);
    for &e in &order {
        if acc >= 0.5 * total {
            break;
        }
        acc += w[e];
        set += 1;
        hw += w[e] * hourglass_share(&element_u(m, x, e));
        ww += w[e];
    }
    let edge_w: f64 = (0..ne).filter(|&e| edge[e]).map(|e| w[e]).sum();
    let first = order[0];
    Location {
        sum: energies.iter().sum(),
        sum_abs: energies.iter().map(|v| v.abs()).sum(),
        set,
        first,
        edge_share: edge_w / total,
        hourglass: if ww > 0.0 { hw / ww } else { f64::NAN },
        stretch: centre_stretch(&element_coords(m.mesh(), first), &element_u(m, u, first)),
    }
}

fn report_location(tag: &str, l: &Location, n: usize, edge: &[bool]) {
    let (i, j, k) = grid(l.first, n);
    println!(
        "{tag}: sum e_e {:.6e} (sum |e_e| {:.6e}), located set |A| {}, first element {} at ({i}, {j}, {k}){}, edge share \
         s_E {:.4}, hourglass share H {:.4}, smallest centre stretch {:.4}",
        l.sum,
        l.sum_abs,
        l.set,
        l.first,
        if edge[l.first] { " (punch edge)" } else { "" },
        l.edge_share,
        l.hourglass,
        l.stretch
    );
}

fn form(h: f64) -> &'static str {
    if h >= 0.5 {
        "hourglass form"
    } else if h <= 0.2 {
        "smooth"
    } else {
        "mixed"
    }
}

/// KA: `min_e lambda_min(K_aa,e) / (mu h^3)` of rule L at its converged modes, and the element.
fn kaa_min(m: &SolidModel, u: &[f64], states: &[ElementState], n: usize) -> (f64, usize) {
    let mat = neo_hookean();
    let ctx = PointContext::default();
    let mut lo = (f64::INFINITY, usize::MAX);
    for e in 0..m.mesh().n_elements() {
        let coords = element_coords(m.mesh(), e);
        let ue = element_u(m, u, e);
        let (modes, _) = modes_and_strains_with(&coords, &mat, &ue, &states[e], &ctx, LIN).unwrap();
        let (_, k) = enhanced_force_and_tangent_with(&coords, &mat, &ue, &modes, &states[e], &ctx, LIN).unwrap();
        let block: Vec<f64> = (0..81).map(|p| k.get(24 + p / 9, 24 + p % 9)).collect();
        let v = symmetric_eigen(&block, 9).unwrap().values[0];
        if v < lo.0 {
            lo = (v, e);
        }
    }
    (lo.0 / (MU / (n * n * n) as f64), lo.1)
}

/// The classification of the registered criterion (reading c6): `fi_pass` whether a finer-increment solve passed (`None`:
/// not known in this record).
fn classify(l1: f64, rho_f: f64, lambda_f: f64, fi_pass: Option<bool>) -> String {
    if !(l1.is_finite() && rho_f.is_finite() && lambda_f.is_finite()) {
        return "bounded (a measure is missing)".to_string();
    }
    if l1 < 0.0 && rho_f > 0.0 && lambda_f > 0.0 {
        "I-S (identified: a spurious instability of rule L)".to_string()
    } else if l1 < 0.0 && rho_f <= 0.0 && lambda_f <= 0.0 {
        "I-P (identified: an instability of the discretised body)".to_string()
    } else if l1 > 0.0 {
        match fi_pass {
            Some(true) => "I-T (identified: a Newton iterate)".to_string(),
            Some(false) => "bounded (stable converged state, no finer solve passes)".to_string(),
            None => "I-T or bounded (by the finer-increment record)".to_string(),
        }
    } else {
        "bounded (the reference element's two readings disagree)".to_string()
    }
}

// ============================================================================ the analysis of one converged state

/// Everything of the readings c2, c3 and of the reference element at a state of P.
struct Analysis {
    nu: Option<usize>,
    sy: f64,
    e1: Option<GeneralizedEigen>,
    e2: Option<GeneralizedEigen>,
    lc1: (f64, f64),
    loc1: Option<Location>,
    loc2: Option<Location>,
    l1: f64,
    m1: f64,
    rho_f: f64,
    rho_full: f64,
}

fn analyse(mesh: &SolidMesh, n: usize, kind: Kind, s: &State, pencil: bool) -> Analysis {
    let m = block_model(mesh, kind, DEFAULT, s.load * -DELTA);
    let (k, free) = free_tangent(&m, &s.u, &s.states);
    let (sy, ks) = symmetrise(&k);
    let nu = inertia(&ks);
    let points = points_of(&m, &free);
    println!(
        "analysis {} {n}^3 at {:.6}: SY {sy:.3e}, inertia nu {nu:?}",
        kind.name, s.load
    );
    let e1 = lowest(&ks, &identity(free.len()), &points);
    let e2 = if pencil {
        lowest(&ks, &laplacian_sparse(&m, &free), &points)
    } else {
        None
    };
    let edge = edge_elements(m.mesh());
    let tangents = element_tangents(&m, kind, &s.u, &s.states);
    let ndof = m.dof_map().n_equations();
    let mut lc1 = (f64::NAN, f64::NAN);
    let mut locs = [None, None];
    for (slot, e) in [&e1, &e2].into_iter().enumerate() {
        let Some(e) = e else { continue };
        let x = &e.vectors[0];
        let full = expand(&free, ndof, x);
        let loc = locate(&m, &tangents, &full, e.values[0] < 0.0, &edge, &s.u);
        let lc = (loc.sum - quadratic(&ks, x)).abs() / loc.sum_abs;
        if slot == 0 {
            lc1.0 = lc;
        } else {
            lc1.1 = lc;
        }
        report_location(
            &format!(
                "located {} mode {}",
                if slot == 0 { "free-tangent" } else { "pencil" },
                kind.name
            ),
            &loc,
            n,
            &edge,
        );
        locs[slot] = Some(loc);
    }
    let [loc1, loc2] = locs;
    let l1 = e1.as_ref().map(|e| e.values[0] / (MU / n as f64)).unwrap_or(f64::NAN);
    let m1 = e2.as_ref().map(|e| e.values[0] / MU).unwrap_or(f64::NAN);
    // The reference element at the same configuration (reading "the reference element at the same state").
    let (rho_f, rho_full) = match &e1 {
        Some(e) => {
            let x = &e.vectors[0];
            let xx: f64 = x.iter().map(|v| v * v).sum();
            let rho = |kd: Kind| {
                let mr = block_model(mesh, kd, DEFAULT, s.load * -DELTA);
                let (kr, fr) = free_tangent(&mr, &s.u, &initial_states(&mr).unwrap());
                assert_eq!(fr, free, "the same free equations");
                quadratic(&kr, x) / xx / (MU / n as f64)
            };
            (rho(K_FBAR), rho(K_FULL))
        }
        None => (f64::NAN, f64::NAN),
    };
    println!(
        "analysis {} {n}^3 at {:.6}: lambda_1 / (mu h) {l1:.6e}, m_1 {m1:.6e}, rho_F / (mu h) {rho_f:.6e}, rho_Full / (mu \
         h) {rho_full:.6e}",
        kind.name, s.load
    );
    Analysis {
        nu,
        sy,
        e1,
        e2,
        lc1,
        loc1,
        loc2,
        l1,
        m1,
        rho_f,
        rho_full,
    }
}

/// `lambda_F` (or any element's `lambda_1`) over `mu h` at the last state of its own P, with the residual of its pairs.
fn own_lowest(mesh: &SolidMesh, n: usize, kind: Kind, loads: &[f64]) -> (f64, f64, f64) {
    let path = replicate(mesh, kind, DEFAULT, loads);
    let Some(s) = path
        .last()
        .filter(|s| (s.load - loads.last().copied().unwrap_or(0.0)).abs() < 1e-15)
    else {
        return (f64::NAN, f64::NAN, f64::NAN);
    };
    let m = block_model(mesh, kind, DEFAULT, s.load * -DELTA);
    let (k, free) = free_tangent(&m, &s.u, &s.states);
    let (_, ks) = symmetrise(&k);
    println!(
        "own {} {n}^3 at {:.6}: inertia nu {:?}",
        kind.name,
        s.load,
        inertia(&ks)
    );
    let e = lowest(&ks, &identity(free.len()), &points_of(&m, &free));
    let l = e.as_ref().map(|e| e.values[0] / (MU / n as f64)).unwrap_or(f64::NAN);
    (l, worst_residual(&e), s.load)
}

// ============================================================================ the fast test

#[test]
fn f763_fast_setup() {
    let mut ch = Checks::new();
    let mesh8 = block_mesh(8);
    let mesh16 = block_mesh(16);

    // F6: the punch-edge elements.
    ch.equals(
        "F6 |E| on 8^3",
        edge_elements(&mesh8).iter().filter(|&&b| b).count(),
        16,
    );
    ch.equals(
        "F6 |E| on 16^3",
        edge_elements(&mesh16).iter().filter(|&&b| b).count(),
        32,
    );

    // NC-H: the hourglass share on synthetic fields (16^3 mesh, no solve).
    let pattern: Vec<f64> = (0..24)
        .map(|p| if p % 3 == 0 { XI[p / 3][0] * XI[p / 3][1] } else { 0.0 })
        .collect();
    ch.at_least("NC-H (i) xi eta in u_x: h_e", hourglass_share(&pattern), 1.0 - 1e-12);
    let m16 = block_model(&mesh16, K_L, DEFAULT, -DELTA);
    let dofs16 = m16.dof_map();
    let field = |f: &dyn Fn(&Vec3) -> [f64; 3]| {
        let mut u = vec![0.0; dofs16.n_equations()];
        for (nd, p) in m16.mesh().nodes().iter().enumerate() {
            let v = f(p);
            for c in 0..3 {
                u[dofs16.equation(nd, c).unwrap()] = v[c];
            }
        }
        u
    };
    let lin = field(&|p| [0.1 * p[0] + 0.2 * p[1], 0.3 * p[2], -0.1 * p[0] + 0.4 * p[2]]);
    let worst_lin = (0..m16.mesh().n_elements())
        .map(|e| hourglass_share(&element_u(&m16, &lin, e)))
        .fold(0.0f64, f64::max);
    ch.at_most("NC-H (ii) linear field: largest h_e", worst_lin, 1e-12);
    let pi = std::f64::consts::PI;
    let sine = field(&|p| {
        let v = (pi * p[0]).sin() * (pi * p[1]).sin() * (pi * p[2]).sin();
        [v, v, v]
    });
    let (mut hw, mut ww) = (0.0, 0.0);
    for e in 0..m16.mesh().n_elements() {
        let xe = element_u(&m16, &sine, e);
        hw += spread(&xe) * hourglass_share(&xe);
        ww += spread(&xe);
    }
    ch.at_most("NC-H (iii) sine field: H_mesh", hw / ww, 0.05);

    // F7: the hooked solve against the plain one, rule L 8^3.
    let plain = solve_plain(&mesh8, "block 8^3", K_L, DEFAULT);
    let hooked = solve_hooked(&mesh8, "block 8^3", K_L, DEFAULT, 10);
    let bits = match (&plain, &hooked.outcome) {
        (Ok(a), Ok(b)) => a.0.to_bits() == b.0.to_bits() && a.1.to_bits() == b.1.to_bits(),
        _ => false,
    };
    ch.holds("F7 8^3 rule L: hooked R, w_c bit-equal to the plain solve", bits);

    // F1: the path P on 8^3 to load factor 1 against ADR 0678.
    let path = replicate(&mesh8, K_L, DEFAULT, &hooked.converged);
    let m8 = block_model(&mesh8, K_L, DEFAULT, -DELTA);
    let end = path.last().filter(|s| s.load == 1.0);
    let (r1, w1) = end.map(|s| state_measures(&m8, s)).unwrap_or((f64::NAN, f64::NAN));
    println!("F1 P 8^3 rule L at 1: R/mu {r1:.8}, w_c {w1:+.8}");
    ch.at_most("F1 P 8^3: R against 0.36659596", (r1 / 0.36659596 - 1.0).abs(), 1e-6);
    ch.at_most("F1 P 8^3: w_c against 0.06631664", (w1 / 0.06631664 - 1.0).abs(), 1e-6);

    // F2: the inertia and the driver's factor at the states 0.1 .. 0.9.
    let mut f2_nu = 0usize;
    let mut f2_pass = true;
    let mut counted = 0;
    for s in path.iter().filter(|s| s.load < 0.95) {
        let m = block_model(&mesh8, K_L, DEFAULT, s.load * -DELTA);
        let (k, _) = free_tangent(&m, &s.u, &s.states);
        let nu = inertia(&symmetrise(&k).1);
        let df = driver_factor(&k);
        println!("F2 8^3 at {:.6}: nu {nu:?}, driver factor {df:?}", s.load);
        f2_nu = f2_nu.max(nu.unwrap_or(usize::MAX));
        f2_pass &= df.is_none();
        counted += 1;
    }
    ch.equals("F2 8^3 states counted (0.1 .. 0.9)", counted, 9);
    ch.equals("F2 8^3: largest nu at 0.1 .. 0.9", f2_nu, 0);
    ch.holds("F2 8^3: the driver's factor passes at 0.1 .. 0.9", f2_pass);

    // F3, F4, F5 at load factor 1 on 8^3; NC-L.
    match end {
        Some(s) => {
            let a = analyse(&mesh8, 8, K_L, s, true);
            let nu = a.nu.unwrap_or(usize::MAX);
            ch.at_most("F3 8^3 (K_ff, I): largest residual", worst_residual(&a.e1), 1e-8);
            ch.equals(
                "F3 8^3: negative values among four = min(nu, 4)",
                negatives_among(&a.e1),
                nu.min(4),
            );
            ch.at_most("F4 8^3 (K_ff, L_ff): largest residual", worst_residual(&a.e2), 1e-8);
            ch.equals(
                "F4 8^3: negative values among four = min(nu, 4)",
                negatives_among(&a.e2),
                nu.min(4),
            );
            ch.at_most("F5 (LC1) 8^3 free-tangent mode", a.lc1.0, 1e-10);
            ch.at_most("F5 (LC1) 8^3 pencil mode", a.lc1.1, 1e-10);
            // NC-L: one nodal value at the interior node (4, 4, 4).
            let n0 = 4 + 9 * 4 + 81 * 4;
            let m = block_model(&mesh8, K_L, DEFAULT, -DELTA);
            let mut x = vec![0.0; m.dof_map().n_equations()];
            x[m.dof_map().equation(n0, 0).unwrap()] = 1.0;
            let tangents = element_tangents(&m, K_L, &s.u, &s.states);
            let edge = edge_elements(m.mesh());
            let loc = locate(&m, &tangents, &x, false, &edge, &s.u);
            report_location("NC-L", &loc, 8, &edge);
            // The located set's elements: those of the order whose weights reach half; recompute membership.
            let ne = m.mesh().n_elements();
            let w: Vec<f64> = tangents
                .iter()
                .enumerate()
                .map(|(e, ke)| {
                    let xe = element_u(&m, &x, e);
                    (0..576).map(|p| xe[p / 24] * ke[p] * xe[p % 24]).sum::<f64>().abs()
                })
                .collect();
            let mut order: Vec<usize> = (0..ne).collect();
            order.sort_by(|&a, &b| w[b].total_cmp(&w[a]).then(a.cmp(&b)));
            let inside = order[..loc.set]
                .iter()
                .all(|&e| m.mesh().element_nodes(e).contains(&n0));
            ch.holds("NC-L: |A| <= 8", loc.set <= 8);
            ch.holds("NC-L: every element of A contains n0", inside);
        }
        None => ch.holds("F3, F4, F5, NC-L: the 8^3 state at 1 exists", false),
    }

    // PC1: the plain element on 8^3.
    let off = solve_hooked(&mesh8, "PC1 block 8^3", K_OFF, DEFAULT, 10);
    ch.holds("PC1 Off 8^3: the hooked solve fails", off.outcome.is_err());
    let path = replicate(&mesh8, K_OFF, DEFAULT, &off.converged);
    let at = path.last().filter(|s| s.load == off.last && !off.converged.is_empty());
    let (l1, rho_f) = match at {
        Some(s) => {
            let a = analyse(&mesh8, 8, K_OFF, s, false);
            println!(
                "PC1 Off 8^3 at {:.6}: nu {:?}, form {}",
                s.load,
                a.nu,
                a.loc1.as_ref().map(|l| form(l.hourglass)).unwrap_or("none")
            );
            (a.l1, a.rho_f)
        }
        None => (f64::NAN, f64::NAN),
    };
    let (lambda_f, res_f, _) = own_lowest(&mesh8, 8, K_FBAR, &off.converged);
    println!("PC1: lambda_F / (mu h) {lambda_f:.6e} (residual {res_f:.2e})");
    let class = classify(l1, rho_f, lambda_f, None);
    println!("PC1 classification: {class}");
    ch.holds("PC1 Off 8^3: I-S", class.starts_with("I-S"));
    ch.finish();
}

// ============================================================================ the heavy records

#[test]
#[ignore = "ADR 0682: the heavy record of the 16^3 tangent (reproduction, P, the eigen analyses, F-bar, 8^3), run once"]
fn f763_nh16_tangent_heavy() {
    let mut ch = Checks::new();
    let mesh16 = block_mesh(16);
    let mesh8 = block_mesh(8);

    // RP0, RP1.
    let rp0 = solve_plain(&mesh16, "block 16^3", K_L, DEFAULT);
    let t0 = rp0.as_ref().err().cloned().unwrap_or_default();
    ch.holds("RP0: the error text of ADR 0678", t0 == ADR0678_TEXT);
    let rp1 = solve_hooked(&mesh16, "block 16^3", K_L, DEFAULT, 10);
    let t1 = rp1.outcome.as_ref().err().cloned().unwrap_or_default();
    ch.holds("RP1: the hooked error text equals RP0's", t1 == t0 && !t0.is_empty());
    ch.at_most("RP1: |lambda_c - 0.9625|", (rp1.last - 0.9625).abs(), 0.0);
    let lambda_c = rp1.last;

    // P, ON, RP2.
    let path = replicate(&mesh16, K_L, DEFAULT, &rp1.converged);
    let mut onset = (0.0f64, f64::NAN);
    for s in &path {
        let m = block_model(&mesh16, K_L, DEFAULT, s.load * -DELTA);
        let (k, _) = free_tangent(&m, &s.u, &s.states);
        let nu = inertia(&symmetrise(&k).1);
        let df = driver_factor(&k);
        println!("ON 16^3 at {:.6}: nu {nu:?}, driver factor {df:?}", s.load);
        match nu {
            Some(0) => onset.0 = s.load,
            Some(_) if onset.1.is_nan() => onset.1 = s.load,
            _ => {}
        }
    }
    println!(
        "ON: the last state with nu = 0 at {:.6}, the first with nu >= 1 at {:.6}",
        onset.0, onset.1
    );
    ch.finite("ON: the last load factor with nu = 0", onset.0);
    let Some(uc) = path.last().filter(|s| s.load == lambda_c) else {
        ch.holds("RP2: P reaches lambda_c", false);
        ch.finish();
        return;
    };
    let m = block_model(&mesh16, K_L, DEFAULT, lambda_c * -DELTA);
    let (k, free) = free_tangent(&m, &uc.u, &uc.states);
    let rp2 = driver_factor(&k).map(|(row, d)| (m.dof_map().node_component(free[row]).unwrap(), d));
    println!("RP2: the driver's first factor at u_c: {rp2:?}");
    let (at, d) = rp2.unwrap_or(((usize::MAX, usize::MAX), f64::NAN));
    ch.holds("RP2: the first refusal at node 4768 component 0", at == (4768, 0));
    ch.at_most("RP2: |d / d_ADR0678 - 1|", (d / ADR0678_PIVOT - 1.0).abs(), 1e-3);
    let (rz, wc) = state_measures(&m, uc);
    println!("u_c: R/mu {rz:.8}, w_c {wc:+.8}");

    // SY, E1, E2, LC1, LOC.
    let a = analyse(&mesh16, 16, K_L, uc, true);
    let nu = a.nu.unwrap_or(usize::MAX);
    ch.at_most("SY", a.sy, 1e-10);
    ch.at_most("E1 (K_ff, I): largest residual", worst_residual(&a.e1), 1e-8);
    ch.equals(
        "E1: negative values among four = min(nu, 4)",
        negatives_among(&a.e1),
        nu.min(4),
    );
    ch.at_most("E2 (K_ff, L_ff): largest residual", worst_residual(&a.e2), 1e-8);
    ch.equals(
        "E2: negative values among four = min(nu, 4)",
        negatives_among(&a.e2),
        nu.min(4),
    );
    ch.at_most("LC1 free-tangent mode", a.lc1.0, 1e-10);
    ch.at_most("LC1 pencil mode", a.lc1.1, 1e-10);
    for (name, l) in [("free tangent", &a.loc1), ("pencil", &a.loc2)] {
        let l = l.as_ref();
        ch.finite(&format!("LOC {name}: s_E"), l.map(|l| l.edge_share).unwrap_or(f64::NAN));
        ch.finite(&format!("LOC {name}: H"), l.map(|l| l.hourglass).unwrap_or(f64::NAN));
        ch.finite(
            &format!("LOC {name}: stretch"),
            l.map(|l| l.stretch).unwrap_or(f64::NAN),
        );
    }
    if let Some(e) = &a.e1 {
        let close = (e.values[1] - e.values[0]).abs() <= 1e-6 * e.values[0].abs();
        println!("E1: the two lowest within 1e-6 relative: {close}");
    }
    println!(
        "nu {nu}, lambda_1 / (mu h) {:.6e}, m_1 {:.6e}, rho_F {:.6e}, rho_Full {:.6e}",
        a.l1, a.m1, a.rho_f, a.rho_full
    );

    // KA.
    let (ka, ke) = kaa_min(&m, &uc.u, &uc.states, 16);
    let (i, j, kk) = grid(ke, 16);
    println!(
        "KA: min lambda(K_aa) / (mu h^3) {ka:.6e} at element {ke} ({i}, {j}, {kk}){}",
        if edge_elements(&mesh16)[ke] {
            " (punch edge)"
        } else {
            ""
        }
    );
    ch.finite("KA", ka);

    // CF: rho_F (in analyse) and lambda_F.
    ch.finite("CF rho_F", a.rho_f);
    ch.finite("CF rho_Full", a.rho_full);
    let (lambda_f, res_f, at_f) = own_lowest(&mesh16, 16, K_FBAR, &rp1.converged);
    println!("CF: F-bar 16^3 at {at_f:.6}: lambda_F / (mu h) {lambda_f:.6e}");
    ch.finite("CF lambda_F", lambda_f);
    ch.at_most("CF: F-bar pairs, largest residual", res_f, 1e-8);

    // C8.
    let (l8, res8, at8) = own_lowest(&mesh8, 8, K_L, &rp1.converged);
    println!("C8: rule L 8^3 at {at8:.6}: lambda_1 / (mu h) {l8:.6e} (residual {res8:.2e})");
    ch.finite("C8 lambda_1 (8^3)", l8);

    // CL.
    let class = classify(a.l1, a.rho_f, lambda_f, None);
    let f1 = a.loc1.as_ref().map(|l| l.hourglass).unwrap_or(f64::NAN);
    let f2 = a.loc2.as_ref().map(|l| l.hourglass).unwrap_or(f64::NAN);
    let place = a.loc1.as_ref().map(|l| l.edge_share >= 0.5).unwrap_or(false);
    println!(
        "CL: {class}; form {} (free tangent H {f1:.4}) and {} (pencil H {f2:.4}): {}; {}",
        form(f1),
        form(f2),
        if form(f1) == form(f2) {
            "the same side"
        } else {
            "undetermined"
        },
        if place {
            "at the punch edge"
        } else {
            "not at the punch edge"
        }
    );
    ch.finish();
}

#[test]
#[ignore = "ADR 0682: the heavy record of S, W and the finer increments on 16^3, run once"]
fn f763_nh16_variants_heavy() {
    let mesh16 = block_mesh(16);
    let mut ch = Checks::new();
    let mut rows = Vec::new();
    for (tag, opts, steps) in [
        ("SW S", S, 10),
        ("SW W", W, 10),
        ("FI 20", DEFAULT, 20),
        ("FI 40", DEFAULT, 40),
    ] {
        let h = solve_hooked(&mesh16, &format!("{tag} block 16^3"), K_L, opts, steps);
        let classified = match &h.outcome {
            Ok((r, w)) => r.is_finite() && w.is_finite(),
            Err(_) => !h.converged.is_empty(),
        };
        ch.holds(&format!("{tag}: outcome classified"), classified);
        println!(
            "{tag}: {}; last converged load factor {:.6}; within 0.025 of 0.9625: {}",
            if h.outcome.is_ok() { "passes" } else { "fails" },
            h.last,
            (h.last - 0.9625).abs() <= 0.025
        );
        rows.push((tag, h.outcome.is_ok(), h.last));
    }
    // Reading c5: both finer solves fail and their last converged factors and lambda_c = 0.9625 lie within 0.025.
    let fi: Vec<_> = rows.iter().filter(|r| r.0.starts_with("FI")).collect();
    let lasts: Vec<f64> = fi.iter().map(|r| r.2).chain([0.9625]).collect();
    let spread =
        lasts.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)) - lasts.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let path = fi.iter().all(|r| !r.1) && spread <= 0.025;
    println!("FI: spread of the last converged factors with lambda_c {spread:.6}");
    println!(
        "FI reading: {}; a finer solve passes: {}",
        if path {
            "a property of the path"
        } else {
            "increment-dependent"
        },
        fi.iter().any(|r| r.1)
    );
    ch.finish();
}
