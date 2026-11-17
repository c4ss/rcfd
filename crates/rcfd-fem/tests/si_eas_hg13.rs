//! F-767 (ADR 0688): the failing Newton iterate of rule L at the punch corner. The failing attempt of ADR 0682 (rule L,
//! neo-Hookean, the punch block on `16^3`, from the converged state at load factor `0.9625` to `0.965625`) recorded iterate
//! by iterate by a bit-neutral observer (a contribution adding nothing, D1) and rebuilt step by step (BI16); at every
//! iterate the lowest eigenvalue of the free tangent and of the ADR 0628 pencil located by element (EI, LCI, LOCI), the
//! predictor's share in the isolated pencil mode of ADR 0682 (SP), the tangent consistency (TC) and the jump of the modes;
//! the ADR 0628 sweep at the state of the punch-corner element (SH); the line search of ADR 0607 and the damping of the
//! predictor (`NewtonOptions::predictor_damping`, ADR 0688 D3) on the same attempt (`solve_increment`, D2) and on the
//! whole solve; the fast controls on `8^3` (OB8, BI8, BX8, PD8, LS8, PD1, RF, NC-S, TC8) and the positive control on the
//! ADR 0628 patch below its zero (PC, NC-P). The helpers are copies of those of `si_eas_hg12.rs` and `si_eas_hg3.rs`
//! (existing tests are not edited). Every aggregated value is required finite before it is compared.

use rcfd_core::vec3::Vec3;
use rcfd_fem::arclength::{trace, ArcLengthOptions};
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::constraint::{prescribed_equations, reduce};
use rcfd_fem::element::{element_for, ElementState};
use rcfd_fem::material::{HyperelasticEnergy, PointContext};
use rcfd_fem::nl_incompatible::{
    condensed_force_and_tangent, enhanced_force_and_tangent_with, modes_and_strains_with, IncompatibleStabilisation,
};
use rcfd_fem::nonlinear::{
    initial_states, internal_force, solve, solve_increment, solve_with, tangent, Contribution, DriverExtensions,
    LoadStep, NewtonOptions, NonlinearResult, StepControl,
};
use rcfd_fem::solve::DirectFactor;
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, Material, SolidFormulation, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{
    generalized_eigen_dense, lowest_eigenpairs, symmetric_eigen, Factorisation, GeneralizedEigen, Shift,
    SubspaceOptions,
};
use rcfd_linalg::skyline::{PivotRule, SkylineError};
use rcfd_linalg::sparse_chol::{SparseOptions, SparseOrdering, SupernodalAnalysis, SupernodalLdlt};
use rcfd_linalg::CsrMatrix;
use std::cell::RefCell;
use std::collections::BTreeSet;
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

const K_L: Kind = Kind {
    name: "L",
    formulation: INC,
    rule: LIN,
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
    last: f64,
}

fn solve_hooked(mesh: &SolidMesh, tag: &str, kind: Kind, opts: Opts, o: &NewtonOptions) -> Hooked {
    let m = block_model(mesh, kind, opts, -DELTA);
    let mut rec = Recorder::default();
    let t0 = Instant::now();
    let steps = o.steps;
    let out = solve_with(
        &m,
        &[],
        &[],
        o,
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
            let lengths: Vec<String> = r
                .steps
                .iter()
                .map(|s| {
                    format!(
                        "{:?}",
                        s.step_lengths.iter().map(|a| format!("{a:.4}")).collect::<Vec<_>>()
                    )
                })
                .collect();
            println!(
                "{tag}: R/mu {rz:.8}, w_c {wc:+.8}, {secs:.1} s; step lengths per increment {}",
                lengths.join(" ")
            );
            Ok((rz, wc))
        }
        Err(e) => {
            println!("{tag}: solve failed: {e}; last converged load factor {last:.6}, {secs:.1} s");
            Err(e.to_string())
        }
    };
    Hooked { outcome, last }
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
    let f = centre_f(coords, ue);
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

fn centre_f(coords: &[Vec3], ue: &[f64]) -> [[f64; 3]; 3] {
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
    f
}

fn key_of<T>(r: &Result<T, FemError>) -> Option<&'static str> {
    match r {
        Err(FemError::InvalidValue { key, .. }) => Some(key),
        _ => None,
    }
}

fn bits_equal(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn norm(v: impl Iterator<Item = f64>) -> f64 {
    v.map(|x| x * x).sum::<f64>().sqrt()
}

// ============================================================================ D1: the observer

/// One attempt of the driver as the observer saw it.
struct AttemptLog {
    committed: f64,
    attempted: f64,
    /// `None` while running; then converged or failed.
    converged: Option<bool>,
    /// The iterates whose force the driver evaluated, `u_0` the committed state.
    iterates: Vec<Vec<f64>>,
}

#[derive(Default)]
struct Log {
    attempts: Vec<AttemptLog>,
}

/// The step-control half of the observer: returns the proposed load factor unchanged and opens an attempt.
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

/// The contribution half of the observer: adds nothing, records the iterate of every evaluation of a running attempt.
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

/// The solve of the block with the observer: the outcome (measures or error text) and the log.
fn solve_observed(mesh: &SolidMesh, tag: &str, kind: Kind, o: &NewtonOptions) -> (Result<(f64, f64), String>, Log) {
    let m = block_model(mesh, kind, DEFAULT, -DELTA);
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
    let outcome = match out {
        Ok(r) => {
            let (rz, wc) = measures(&m, &r);
            println!("{tag} observed: R/mu {rz:.8}, w_c {wc:+.8}, {secs:.1} s");
            Ok((rz, wc))
        }
        Err(e) => {
            println!("{tag} observed: solve failed: {e}, {secs:.1} s");
            Err(e.to_string())
        }
    };
    let log = log.into_inner();
    let text: Vec<String> = log
        .attempts
        .iter()
        .map(|a| {
            format!(
                "{:.6}->{:.6} ({} iterates){}",
                a.committed,
                a.attempted,
                a.iterates.len(),
                if a.converged == Some(true) { "" } else { " x" }
            )
        })
        .collect();
    println!("{tag} observed attempts: [{}]", text.join(", "));
    (outcome, log)
}

// ============================================================================ the driver's step rebuilt

/// The driver's factor and solve of a reduced system with its pivot test; the free row and pivot of a refusal.
fn driver_factor_solve(a: &CsrMatrix, b: &[f64]) -> Result<Vec<f64>, (usize, f64)> {
    let analysis = SupernodalAnalysis::nested_dissection(a).map_err(|_| (usize::MAX, f64::NAN))?;
    let diag = a.diag();
    let f = match SupernodalLdlt::factor_analysed(a, &analysis) {
        Ok(f) => f,
        Err(SkylineError::Pivot { row, pivot, .. }) => return Err((row, pivot)),
        Err(_) => return Err((usize::MAX, f64::NAN)),
    };
    if let Some((&row, &d)) = f
        .order()
        .iter()
        .zip(f.pivots())
        .find(|(&row, &d)| d.is_nan() || d <= 1e-10 * diag[row])
    {
        return Err((row, d));
    }
    Ok(f.solve(b))
}

/// Iteration `k` of the driver's attempt from `lc` to `lt` rebuilt at the iterate `u` (BI): the relative residual, the
/// correction (free part) and the next iterate, or the refusal (node, component, pivot).
struct Step {
    r: f64,
    free: Vec<usize>,
    du: Option<Vec<f64>>,
    next: Option<Vec<f64>>,
    refusal: Option<(usize, usize, f64)>,
    energy: f64,
}

fn replicate_step(m: &SolidModel, states: &[ElementState], u: &[f64], lc: f64, lt: f64, k: usize) -> Step {
    let fixed = prescribed_equations(m, &[]).unwrap();
    let dofs = m.dof_map();
    let fint = match internal_force(m, u, states) {
        Ok(f) => f,
        Err(_) => {
            return Step {
                r: f64::NAN,
                free: Vec::new(),
                du: None,
                next: None,
                refusal: None,
                energy: f64::NAN,
            }
        }
    };
    let kt = tangent(m, u, states, Some(4)).unwrap();
    let rfull: Vec<f64> = fint.iter().map(|g| lt * 0.0 - g).collect();
    let increment: Vec<(usize, f64)> = fixed
        .iter()
        .map(|&(eq, v)| (eq, if k == 0 { (lt - lc) * v } else { 0.0 }))
        .collect();
    let sys = reduce(&kt, &rfull, &increment).unwrap();
    let free = sys.free().to_vec();
    let rn = norm(free.iter().map(|&i| rfull[i]));
    let reference = norm(free.iter().map(|_| lt * 0.0)).max(norm(fint.iter().copied()));
    let r = if reference > 0.0 { rn / reference } else { rn };
    match driver_factor_solve(&sys.matrix, &sys.rhs) {
        Ok(du) => {
            let energy = du.iter().zip(&sys.rhs).map(|(a, b)| a * b).sum::<f64>().abs();
            let mut next = u.to_vec();
            for (&q, d) in free.iter().zip(&du) {
                next[q] += d;
            }
            if k == 0 {
                for &(eq, v) in &fixed {
                    next[eq] = lt * v;
                }
            }
            Step {
                r,
                free,
                du: Some(du),
                next: Some(next),
                refusal: None,
                energy,
            }
        }
        Err((row, d)) => {
            let at = if row == usize::MAX {
                (usize::MAX, usize::MAX)
            } else {
                dofs.node_component(free[row]).unwrap()
            };
            Step {
                r,
                free,
                du: None,
                next: None,
                refusal: Some((at.0, at.1, d)),
                energy: f64::NAN,
            }
        }
    }
}

/// BI over the attempts of a log: the number of pairs compared and whether every replicated next iterate is bit-equal.
fn bi_of_log(m: &SolidModel, states: &[ElementState], log: &Log, from: f64) -> (usize, bool) {
    let (mut pairs, mut all) = (0usize, true);
    for a in log.attempts.iter().filter(|a| a.committed >= from) {
        for k in 0..a.iterates.len().saturating_sub(1) {
            let s = replicate_step(m, states, &a.iterates[k], a.committed, a.attempted, k);
            let ok = s.next.as_deref().is_some_and(|n| bits_equal(n, &a.iterates[k + 1]));
            if !ok {
                println!(
                    "BI: attempt {:.6}->{:.6} iterate {k}: the replicated next iterate differs",
                    a.committed, a.attempted
                );
            }
            all &= ok;
            pairs += 1;
        }
    }
    (pairs, all)
}

// ============================================================================ D5: the lowest eigenvalue at any iterate

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

/// The four lowest pairs of `(K, M)` whatever the inertia (D5), with the bracket `(s_lo, s_hi]` of `-lambda_1` when `nu
/// >= 1`.
struct AnyLowest {
    nu: Option<usize>,
    pairs: Option<GeneralizedEigen>,
    bracket: Option<(f64, f64)>,
}

fn lowest_any(k: &CsrMatrix, mm: &CsrMatrix, points: &[[f64; 3]]) -> AnyLowest {
    let nu = inertia(k);
    if nu == Some(0) {
        return AnyLowest {
            nu,
            pairs: lowest(k, mm, points),
            bracket: None,
        };
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
        return AnyLowest {
            nu,
            pairs: None,
            bracket: None,
        };
    };
    for _ in 0..10 {
        let mid = 0.5 * (lo + hi);
        if zero_at(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let sigma = 1.05 * hi;
    let opts = SubspaceOptions {
        shift: Shift::Value(sigma),
        tolerance: 1e-8,
        max_iterations: 500,
        factorisation: Factorisation::Supernodal,
        points: Some(points.to_vec()),
        ..SubspaceOptions::default()
    };
    let t0 = Instant::now();
    let pairs = match lowest_eigenpairs(k, mm, 4, &opts) {
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
    };
    AnyLowest {
        nu,
        pairs,
        bracket: Some((lo, hi)),
    }
}

/// SB: `-s_hi <= lambda_1 <= -s_lo` up to `1e-6 |lambda_1|` (true when there is no bracket).
fn sb_holds(a: &AnyLowest) -> bool {
    match (a.bracket, a.pairs.as_ref()) {
        (None, _) => true,
        (Some((lo, hi)), Some(e)) => {
            let l1 = e.values[0];
            let tol = 1e-6 * l1.abs();
            -hi - tol <= l1 && l1 <= -lo + tol
        }
        (Some(_), None) => false,
    }
}

// ============================================================================ D6, D7, D8

/// `share(d) = (phi^T L d)^2 / ((phi^T L phi) (d^T L d))` (D6).
fn share_l(lap: &CsrMatrix, phi: &[f64], d: &[f64]) -> f64 {
    let mut lphi = vec![0.0; lap.n];
    lap.matvec(phi, &mut lphi);
    let pd: f64 = lphi.iter().zip(d).map(|(a, b)| a * b).sum();
    let pp: f64 = lphi.iter().zip(phi).map(|(a, b)| a * b).sum();
    pd * pd / (pp * quadratic(lap, d))
}

fn share_euclid(phi: &[f64], d: &[f64]) -> f64 {
    let pd: f64 = phi.iter().zip(d).map(|(a, b)| a * b).sum();
    pd * pd / (phi.iter().map(|v| v * v).sum::<f64>() * d.iter().map(|v| v * v).sum::<f64>())
}

/// `TC(u, d)` of D7 on the free equations (`d` the free part).
fn tc(m: &SolidModel, states: &[ElementState], u: &[f64], free: &[usize], d: &[f64], h: f64) -> f64 {
    let dmax = d.iter().fold(0.0f64, |w, v| w.max(v.abs()));
    if dmax.is_nan() || dmax <= 0.0 {
        return f64::NAN;
    }
    let eps = 1e-5 * h;
    let (mut up, mut um) = (u.to_vec(), u.to_vec());
    for (&q, &di) in free.iter().zip(d) {
        up[q] += eps * di / dmax;
        um[q] -= eps * di / dmax;
    }
    let (Ok(fp), Ok(fm)) = (internal_force(m, &up, states), internal_force(m, &um, states)) else {
        return f64::NAN;
    };
    let kt = tangent(m, u, states, Some(4)).unwrap();
    let dh = expand(free, u.len(), &d.iter().map(|v| v / dmax).collect::<Vec<_>>());
    let mut kd = vec![0.0; u.len()];
    kt.matvec(&dh, &mut kd);
    let num = norm(free.iter().map(|&q| (fp[q] - fm[q]) / (2.0 * eps) - kd[q]));
    let den = norm(free.iter().map(|&q| kd[q]));
    num / den
}

/// The modes of every element (rule L, the default cold update) at `u`.
fn element_modes(m: &SolidModel, u: &[f64], states: &[ElementState]) -> Vec<Option<[f64; 9]>> {
    let mat = neo_hookean();
    let ctx = PointContext::default();
    (0..m.mesh().n_elements())
        .map(|e| {
            modes_and_strains_with(
                &element_coords(m.mesh(), e),
                &mat,
                &element_u(m, u, e),
                &states[e],
                &ctx,
                LIN,
            )
            .ok()
            .map(|(a, _)| a)
        })
        .collect()
}

/// D8: `(max Delta a_e, its element, max Delta a_e / Delta u_e, its element)`.
fn mode_jump(
    m: &SolidModel,
    u0: &[f64],
    a0: &[Option<[f64; 9]>],
    u: &[f64],
    states: &[ElementState],
) -> (f64, usize, f64, usize) {
    let a = element_modes(m, u, states);
    let mut out = (0.0f64, usize::MAX, 0.0f64, usize::MAX);
    for e in 0..a.len() {
        let (Some(x), Some(y)) = (a[e], a0[e]) else {
            return (f64::NAN, e, f64::NAN, e);
        };
        let da = norm(x.iter().zip(&y).map(|(p, q)| p - q));
        let ue = element_u(m, u, e);
        let ue0 = element_u(m, u0, e);
        let du = norm(ue.iter().zip(&ue0).map(|(p, q)| p - q));
        if da > out.0 {
            out.0 = da;
            out.1 = e;
        }
        if du > 0.0 && da / du > out.2 {
            out.2 = da / du;
            out.3 = e;
        }
    }
    out
}

// ============================================================================ D9, D10: the ADR 0628 patch

/// SplitMix64 from a seed (`si_eas_hg3.rs`).
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed)
    }
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
    let mut rng = Rng::new(HG_SEED);
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

/// The homogeneous field `u = (F - I) X` at every node.
fn homogeneous_f(m: &SolidModel, f: &[[f64; 3]; 3]) -> Vec<f64> {
    let dofs = m.dof_map();
    let mut u = vec![0.0; dofs.n_equations()];
    for (n, x) in m.mesh().nodes().iter().enumerate() {
        for c in 0..3 {
            let mut v = 0.0;
            for j in 0..3 {
                v += (f[c][j] - if c == j { 1.0 } else { 0.0 }) * x[j];
            }
            u[dofs.equation(n, c).unwrap()] = v;
        }
    }
    u
}

/// The boundary equations of the patch (value 0: the increments of `reduce`).
fn boundary_fixed(m: &SolidModel) -> Vec<(usize, f64)> {
    let boundary = boundary_nodes(m.mesh());
    let dofs = m.dof_map();
    let mut out = Vec::new();
    for &n in &boundary {
        for c in 0..3 {
            out.push((dofs.equation(n, c).unwrap(), 0.0));
        }
    }
    out.sort_by_key(|p| p.0);
    out
}

fn to_dense(a: &CsrMatrix) -> Vec<f64> {
    let n = a.n;
    let mut out = vec![0.0; n * n];
    for r in 0..n {
        for p in a.row_ptr[r]..a.row_ptr[r + 1] {
            out[r * n + a.col[p]] = a.val[p];
        }
    }
    out
}

/// The reduced (interior) tangent of the patch at `u`, symmetrised, and the interior equations.
fn patch_tangent(m: &SolidModel, u: &[f64]) -> Option<(CsrMatrix, Vec<usize>)> {
    let k = tangent(m, u, &initial_states(m).unwrap(), Some(4)).ok()?;
    let sys = reduce(&k, &vec![0.0; k.n], &boundary_fixed(m)).ok()?;
    let free = sys.free().to_vec();
    Some((symmetrise(&sys.matrix).1, free))
}

/// The dense pencil measure `m = lambda_min(K_ff, L_ff) / mu` of the patch at the homogeneous state `F` (D9), and the
/// interior equilibrium residual.
fn patch_m(m: &SolidModel, f: &[[f64; 3]; 3]) -> (f64, f64) {
    let u = homogeneous_f(m, f);
    let Some((k, free)) = patch_tangent(m, &u) else {
        return (f64::NAN, f64::NAN);
    };
    let lap = laplacian_sparse(m, &free);
    let v = generalized_eigen_dense(&to_dense(&k), &to_dense(&lap), free.len())
        .map(|e| e.values[0] / MU)
        .unwrap_or(f64::NAN);
    let fint = internal_force(m, &u, &initial_states(m).unwrap()).ok();
    let eq = fint
        .map(|fi| {
            let fmax = fi.iter().fold(0.0f64, |w, x| w.max(x.abs()));
            free.iter().fold(0.0f64, |w, &q| w.max(fi[q].abs())) / fmax
        })
        .unwrap_or(f64::NAN);
    (v, eq)
}

/// D9 at one element and state: the four measures, the equilibrium and the ray of rule L and the plain element.
fn corner_sweep(tag: &str, f: &[[f64; 3]; 3]) -> (f64, f64) {
    let lin = patch(6, INC, LIN);
    let off = patch(6, INC, OFF);
    let full = patch(6, FULL, OFF);
    let fbar = patch(6, FBAR, OFF);
    let (ml, eq) = patch_m(&lin, f);
    let (mi, _) = patch_m(&off, f);
    let (mf, _) = patch_m(&full, f);
    let (mb, _) = patch_m(&fbar, f);
    println!(
        "SH {tag}: F {f:?}; m_L {ml:.6}, m_Inc {mi:+.6}, m_Full {mf:.6}, m_Fbar {mb:.6}, interior equilibrium {eq:.3e}"
    );
    let (mut zl, mut zi) = (f64::NAN, f64::NAN);
    let mut row = Vec::new();
    for j in 0..=15 {
        let t = j as f64 / 10.0;
        let ft: [[f64; 3]; 3] = std::array::from_fn(|a| {
            std::array::from_fn(|b| (if a == b { 1.0 } else { 0.0 }) + t * (f[a][b] - if a == b { 1.0 } else { 0.0 }))
        });
        let l = patch_m(&lin, &ft).0;
        let i = patch_m(&off, &ft).0;
        if zl.is_nan() && l <= 0.0 {
            zl = t;
        }
        if zi.is_nan() && i <= 0.0 {
            zi = t;
        }
        row.push(format!("{t:.1}: {l:.4}/{i:+.4}"));
    }
    println!(
        "SH {tag} ray t: m_L/m_Inc [{}]; first t with m_L <= 0: {zl}, with m_Inc <= 0: {zi}",
        row.join(", ")
    );
    (ml, mi)
}

// ============================================================================ the analysis of one iterate

/// The free-tangent and pencil analysis of an iterate of the block (D5, located as ADR 0682).
struct IterateAnalysis {
    nu: Option<usize>,
    e1: AnyLowest,
    e2: AnyLowest,
    lc1: (f64, f64),
    loc1: Option<Location>,
    loc2: Option<Location>,
}

fn analyse_iterate(m: &SolidModel, n: usize, u: &[f64], states: &[ElementState], lap: &CsrMatrix) -> IterateAnalysis {
    let (k, free) = free_tangent(m, u, states);
    let (sy, ks) = symmetrise(&k);
    let points = points_of(m, &free);
    let e1 = lowest_any(&ks, &identity(free.len()), &points);
    let e2 = lowest_any(&ks, lap, &points);
    let nu = e1.nu;
    println!("  SY {sy:.3e}, inertia nu {nu:?}");
    let edge = edge_elements(m.mesh());
    let tangents = element_tangents(m, K_L, u, states);
    let ndof = m.dof_map().n_equations();
    let mut lc1 = (f64::NAN, f64::NAN);
    let mut locs = [None, None];
    for (slot, a) in [&e1, &e2].into_iter().enumerate() {
        let Some(e) = &a.pairs else { continue };
        let x = &e.vectors[0];
        let full = expand(&free, ndof, x);
        let loc = locate(m, &tangents, &full, e.values[0] < 0.0, &edge, u);
        let lc = (loc.sum - quadratic(&ks, x)).abs() / loc.sum_abs;
        if slot == 0 {
            lc1.0 = lc;
        } else {
            lc1.1 = lc;
        }
        report_location(
            &format!("  located {} mode", if slot == 0 { "free-tangent" } else { "pencil" }),
            &loc,
            n,
            &edge,
        );
        println!("  form: {}", form(loc.hourglass));
        locs[slot] = Some(loc);
    }
    let [loc1, loc2] = locs;
    IterateAnalysis {
        nu,
        e1,
        e2,
        lc1,
        loc1,
        loc2,
    }
}

fn in_corner(e: usize) -> bool {
    let (i, j, k) = grid(e, 16);
    (6..=9).contains(&i) && (6..=9).contains(&j) && (13..=15).contains(&k)
}

fn largest_node(m: &SolidModel, free: &[usize], d: &[f64]) -> (f64, usize) {
    let (mut w, mut at) = (0.0f64, usize::MAX);
    for (&q, &v) in free.iter().zip(d) {
        if v.abs() > w {
            w = v.abs();
            at = q;
        }
    }
    let node = if at == usize::MAX {
        usize::MAX
    } else {
        m.dof_map().node_component(at).unwrap().0
    };
    (w, node)
}

// ============================================================================ the fast test

#[test]
#[ignore = "ADR 0688: the registered fast run, known miss: NC-P plain 0.97, the pencil pairs by the ADR 0682 tool (Shift::Auto, 300 iterations) did not converge (residuals 6.9e-8 to 2.8e-6 against 1e-8; the dense value 0.955471 mu); every other row met; f767_fast_rows runs the met rows"]
fn f767_fast_known_miss() {
    fast_rows(true);
}

/// The default fast test after the registered run: every row of `f767_fast_known_miss` but the pencil rows of NC-P plain
/// 0.97 (recorded in the known miss).
#[test]
fn f767_fast_rows() {
    fast_rows(false);
}

fn fast_rows(full: bool) {
    let mut ch = Checks::new();
    let mesh8 = block_mesh(8);
    let m8 = block_model(&mesh8, K_L, DEFAULT, -DELTA);
    let states8 = initial_states(&m8).unwrap();

    // OB8: the observer is bit-neutral.
    let plain = solve_plain(&mesh8, "block 8^3", K_L, DEFAULT);
    let (observed, log8) = solve_observed(&mesh8, "block 8^3", K_L, &options(10));
    let bits = match (&plain, &observed) {
        (Ok(a), Ok(b)) => a.0.to_bits() == b.0.to_bits() && a.1.to_bits() == b.1.to_bits(),
        _ => false,
    };
    ch.holds("OB8 8^3 rule L: observed R, w_c bit-equal to the plain solve", bits);

    // BI8: every observed iterate rebuilt.
    let (pairs, all) = bi_of_log(&m8, &states8, &log8, 0.0);
    println!("BI8: {pairs} pairs compared");
    ch.holds("BI8: every replicated next iterate bit-equal", all);
    ch.holds("BI8: at least 20 pairs", pairs >= 20);

    // BX8: solve_increment of the last attempt.
    let last = log8.attempts.last().unwrap();
    let bx = solve_increment(
        &m8,
        &[],
        &vec![0.0; m8.dof_map().n_equations()],
        &last.iterates[0],
        &states8,
        last.committed,
        last.attempted,
        &options(10),
        &[],
    );
    match &bx {
        Ok(eq) => {
            println!(
                "BX8: {} residuals, observed {} iterates; residuals {:?}",
                eq.residuals.len(),
                last.iterates.len(),
                eq.residuals
            );
            ch.holds(
                "BX8: u bit-equal to the observed last iterate",
                bits_equal(&eq.u, last.iterates.last().unwrap()),
            );
            ch.equals(
                "BX8: residual count = observed iterates",
                eq.residuals.len(),
                last.iterates.len(),
            );
        }
        Err(e) => ch.holds(&format!("BX8: solve_increment converges ({e})"), false),
    }

    // TC8: the tangent consistency at the start of the last attempt along its predictor.
    let s0 = replicate_step(&m8, &states8, &last.iterates[0], last.committed, last.attempted, 0);
    let tc8 = s0
        .du
        .as_ref()
        .map(|d| tc(&m8, &states8, &last.iterates[0], &s0.free, d, 1.0 / 8.0))
        .unwrap_or(f64::NAN);
    ch.at_most("TC8 at the start of the last attempt along the predictor", tc8, 1e-6);

    // NC-S: the share of the pencil modes at load factor 1.
    let u1 = last.iterates.last().unwrap();
    let (k, free) = free_tangent(&m8, u1, &states8);
    let ks = symmetrise(&k).1;
    let lap8 = laplacian_sparse(&m8, &free);
    let e = lowest_any(&ks, &lap8, &points_of(&m8, &free));
    match &e.pairs {
        Some(p) => {
            let s1 = share_l(&lap8, &p.vectors[0], &p.vectors[0]);
            let s2 = share_l(&lap8, &p.vectors[0], &p.vectors[1]);
            println!(
                "NC-S: share(phi_1) {s1:.15}, share(phi_2) {s2:.3e}, Euclidean share(phi_2) {:.3e}",
                share_euclid(&p.vectors[0], &p.vectors[1])
            );
            ch.at_least("NC-S share(phi_1)", s1, 1.0 - 1e-10);
            ch.at_most("NC-S share(phi_2)", s2, 1e-8);
        }
        None => ch.holds("NC-S: the 8^3 pencil pairs exist", false),
    }

    // PD8, LS8: the opt-ins on the 8^3 solve.
    let mut pd8 = (f64::NAN, f64::NAN);
    for (tag, o) in [
        ("PD8", options(10).predictor_damping(true)),
        ("LS8", options(10).line_search(true)),
    ] {
        let h = solve_hooked(&mesh8, &format!("{tag} block 8^3"), K_L, DEFAULT, &o);
        let (r, w) = h.outcome.unwrap_or((f64::NAN, f64::NAN));
        if tag == "PD8" {
            pd8 = (r, w);
        }
        ch.at_most(
            &format!("{tag}: R against 0.36659596"),
            (r / 0.36659596 - 1.0).abs(),
            1e-6,
        );
        ch.at_most(
            &format!("{tag}: w_c against 0.06631664"),
            (w / 0.06631664 - 1.0).abs(),
            1e-6,
        );
    }

    // PD1: one increment, default and damped.
    let d1 = solve_hooked(&mesh8, "PD1 default block 8^3", K_L, DEFAULT, &options(1));
    let p1 = solve_hooked(
        &mesh8,
        "PD1 damped block 8^3",
        K_L,
        DEFAULT,
        &options(1).predictor_damping(true),
    );
    println!("PD1: default {:?}; damped {:?}", d1.outcome.is_ok(), p1.outcome.is_ok());
    if let Ok((r, w)) = p1.outcome {
        ch.at_most("PD1 damped: R against PD8's", (r / pd8.0 - 1.0).abs(), 1e-6);
        ch.at_most("PD1 damped: w_c against PD8's", (w / pd8.1 - 1.0).abs(), 1e-6);
    }

    // RF: the refusals.
    let rf1 = solve(
        &m8,
        &[],
        &[],
        &NewtonOptions::new(1).predictor_damping(true).unsymmetric(true),
    );
    ch.holds(
        &format!("RF unsymmetric: {:?}", rf1.as_ref().err()),
        key_of(&rf1) == Some("predictor_damping"),
    );
    let ao = ArcLengthOptions::new(0.1).newton(NewtonOptions::new(1).predictor_damping(true));
    let rf2 = trace(&m8, &[], &[], &ao, &[], &mut |_| true);
    ch.holds(
        &format!("RF arc length: {:?}", rf2.as_ref().err()),
        key_of(&rf2) == Some("predictor_damping"),
    );
    let log = RefCell::new(Log::default());
    let obs = Observer { log: &log };
    let rf3 = solve_with(
        &m8,
        &[],
        &[],
        &options(1).predictor_damping(true),
        DriverExtensions {
            contributions: vec![&obs],
            step_control: None,
        },
    );
    let t3 = rf3.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
    ch.holds(
        &format!("RF with a contribution: {t3}"),
        t3.contains("predictor_damping"),
    );

    // PC, NC-P: the ADR 0628 patch, C49.
    pc_rows(&mut ch, full);
    ch.finish();
}

/// PC and NC-P (D10).
fn pc_rows(ch: &mut Checks, all: bool) {
    for (tag, rule, lambda, negative) in [
        ("PC plain 0.93", OFF, 0.93, true),
        ("NC-P plain 0.97", OFF, 0.97, false),
        ("NC-P rule L 0.93", LIN, 0.93, false),
    ] {
        let m = patch(6, INC, rule);
        let f = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, lambda]];
        let u = homogeneous_f(&m, &f);
        let Some((k, free)) = patch_tangent(&m, &u) else {
            ch.holds(&format!("{tag}: the patch tangent"), false);
            continue;
        };
        let lap = laplacian_sparse(&m, &free);
        let points = points_of(&m, &free);
        let e1 = lowest_any(&k, &identity(free.len()), &points);
        let e2 = lowest_any(&k, &lap, &points);
        let d1 = symmetric_eigen(&to_dense(&k), free.len())
            .map(|e| e.values[0])
            .unwrap_or(f64::NAN);
        let d2 = generalized_eigen_dense(&to_dense(&k), &to_dense(&lap), free.len())
            .map(|e| e.values[0])
            .unwrap_or(f64::NAN);
        let l1 = e1.pairs.as_ref().map(|e| e.values[0]).unwrap_or(f64::NAN);
        let l2 = e2.pairs.as_ref().map(|e| e.values[0]).unwrap_or(f64::NAN);
        let nu = e1.nu.unwrap_or(usize::MAX);
        let refused = driver_factor(&k).is_some();
        println!(
            "{tag}: nu {nu}, (K, I) lowest {l1:.6e} (dense {d1:.6e}), pencil {:.6} mu (dense {:.6}), driver factor refuses \
             {refused}",
            l2 / MU,
            d2 / MU
        );
        ch.at_most(&format!("{tag}: (K, I) against dense"), (l1 / d1 - 1.0).abs(), 1e-6);
        let pencil_rows = all || tag != "NC-P plain 0.97";
        if pencil_rows {
            ch.at_most(&format!("{tag}: pencil against dense"), (l2 / d2 - 1.0).abs(), 1e-6);
        }
        if negative {
            ch.holds(&format!("{tag}: nu >= 1"), nu >= 1 && nu != usize::MAX);
            ch.holds(&format!("{tag}: (K, I) lambda_1 < 0"), l1 < 0.0);
            ch.holds(&format!("{tag}: pencil lambda_1 < 0"), l2 < 0.0);
            ch.holds(&format!("{tag}: SB (K, I)"), sb_holds(&e1));
            ch.holds(&format!("{tag}: SB pencil"), sb_holds(&e2));
            ch.holds(&format!("{tag}: the driver's factor refuses"), refused);
            // LC1 of the negative pencil mode.
            let ndof = m.dof_map().n_equations();
            let tangents = element_tangents(
                &m,
                Kind {
                    name: "patch",
                    formulation: INC,
                    rule,
                },
                &u,
                &initial_states(&m).unwrap(),
            );
            let edge = vec![false; m.mesh().n_elements()];
            match &e2.pairs {
                Some(p) => {
                    let loc = locate(&m, &tangents, &expand(&free, ndof, &p.vectors[0]), true, &edge, &u);
                    report_location(&format!("{tag} pencil mode"), &loc, 6, &edge);
                    let lc = (loc.sum - quadratic(&k, &p.vectors[0])).abs() / loc.sum_abs;
                    ch.at_most(&format!("{tag}: LC1 pencil mode"), lc, 1e-10);
                }
                None => ch.holds(&format!("{tag}: pencil pairs"), false),
            }
        } else {
            ch.equals(&format!("{tag}: nu"), nu, 0);
            ch.holds(&format!("{tag}: (K, I) lambda_1 > 0"), l1 > 0.0);
            if pencil_rows {
                ch.holds(&format!("{tag}: pencil lambda_1 > 0"), l2 > 0.0);
            }
            ch.holds(&format!("{tag}: the driver's factor passes"), !refused);
        }
    }
}

// ============================================================================ the heavy records

/// The failing attempt of the observed `16^3` solve: its index in the log.
fn failing_attempt(log: &Log) -> Option<usize> {
    log.attempts
        .iter()
        .rposition(|a| a.committed == 0.9625 && a.converged == Some(false))
}

#[test]
#[ignore = "ADR 0688: the heavy record of the failing attempt on 16^3, iterate by iterate, run once (257 s), every limit met: the attempt 0.9625 -> 0.965625 fails at the predictor's iterate (K = 1); the predictor's share in the isolated pencil mode 4.1e-27; at u_1 nu = 1, lambda_1 / (mu h) -0.0761 and m_1 -0.0291 in the single element 3720 (H 0.93); TC 1.4e-7; the corner elements' homogeneous states stable (m_L 1.057, 1.060); f767_fast_rows covers the same path on 8^3"]
fn f767_nh16_iterates_heavy() {
    let mut ch = Checks::new();
    let t_all = Instant::now();
    let mesh16 = block_mesh(16);
    let m = block_model(&mesh16, K_L, DEFAULT, -DELTA);
    let states = initial_states(&m).unwrap();
    let ndof = m.dof_map().n_equations();
    let h = 1.0 / 16.0;

    // OB16.
    let (outcome, log) = solve_observed(&mesh16, "block 16^3", K_L, &options(10));
    let text = outcome.as_ref().err().cloned().unwrap_or_default();
    ch.holds("OB16: the error text of ADR 0678", text == ADR0678_TEXT);
    let Some(fa) = failing_attempt(&log) else {
        ch.holds("OB16: the failing attempt from 0.9625 observed", false);
        ch.finish();
        return;
    };
    let att = &log.attempts[fa];
    let (lc, lt) = (att.committed, att.attempted);
    let it = &att.iterates;
    let kk = it.len() - 1;
    println!(
        "the failing attempt {lc:.6} -> {lt:.6}: {} iterates (K = {kk})",
        it.len()
    );

    // OT: the other attempts from 0.95 and 0.9625, by their residuals.
    for a in log.attempts.iter().filter(|a| a.committed >= 0.95) {
        let r: Vec<String> = (0..a.iterates.len())
            .map(|k| {
                format!(
                    "{:.3e}",
                    replicate_step(&m, &states, &a.iterates[k], a.committed, a.attempted, k).r
                )
            })
            .collect();
        println!(
            "OT {:.6} -> {:.6} ({}): {} iterates, residuals [{}]",
            a.committed,
            a.attempted,
            if a.converged == Some(true) {
                "converged"
            } else {
                "failed"
            },
            a.iterates.len(),
            r.join(", ")
        );
    }

    // BI16 and the corrections.
    let mut steps = Vec::new();
    let mut bi = true;
    for k in 0..=kk {
        let s = replicate_step(&m, &states, &it[k], lc, lt, k);
        if k < kk {
            let ok = s.next.as_deref().is_some_and(|nx| bits_equal(nx, &it[k + 1]));
            if !ok {
                println!("BI16: iterate {k}: the replicated next iterate differs");
            }
            bi &= ok;
        } else {
            println!("BI16: the refusal at u_K: {:?}", s.refusal);
            let ok = s
                .refusal
                .is_some_and(|(n, c, d)| n == 4768 && c == 0 && d.to_bits() == ADR0678_PIVOT.to_bits());
            ch.holds(
                "BI16: the driver's factor at u_K refuses with the pivot of ADR 0678 at node 4768 component 0",
                ok,
            );
        }
        steps.push(s);
    }
    ch.holds("BI16: every replicated next iterate bit-equal", bi);

    // BX16.
    let bx = solve_increment(&m, &[], &vec![0.0; ndof], &it[0], &states, lc, lt, &options(10), &[]);
    let bx_text = bx.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
    let tail = &ADR0678_TEXT[ADR0678_TEXT.find("singular tangent").unwrap()..];
    println!("BX16: {bx_text}");
    ch.holds(
        "BX16: solve_increment fails with the failure text of ADR 0678",
        bx_text.contains(tail),
    );

    // The isolated pencil mode at u_c (I0) and the analyses at every iterate.
    let (_, free) = free_tangent(&m, &it[0], &states);
    let lap = laplacian_sparse(&m, &free);
    let a0_modes = element_modes(&m, &it[0], &states);
    let mut phi: Option<Vec<f64>> = None;
    let mut last_first = (usize::MAX, f64::NAN);
    for k in 0..=kk {
        let t0 = Instant::now();
        println!("iterate {k} of {kk}:");
        let a = analyse_iterate(&m, 16, &it[k], &states, &lap);
        let nu = a.nu.unwrap_or(usize::MAX);
        let (kf, _) = free_tangent(&m, &it[k], &states);
        let df = driver_factor(&kf).map(|(row, d)| (m.dof_map().node_component(free[row]).unwrap(), d));
        let l1 = a.e1.pairs.as_ref().map(|e| e.values[0] / (MU * h)).unwrap_or(f64::NAN);
        let m1 = a.e2.pairs.as_ref().map(|e| e.values[0] / MU).unwrap_or(f64::NAN);
        if k == 0 {
            phi = a.e2.pairs.as_ref().map(|e| e.vectors[0].clone());
            ch.equals("I0: nu at u_c", nu, 0);
            ch.holds("I0: the driver's factor passes at u_c", df.is_none());
            ch.at_most("I0: m_1 against 6.286570e-2", (m1 / 6.286570e-2 - 1.0).abs(), 1e-4);
            ch.equals(
                "I0: the first element of the pencil mode",
                a.loc2.as_ref().map(|l| l.first).unwrap_or(usize::MAX),
                3720,
            );
        }
        for (name, e) in [("(K_ff, I)", &a.e1), ("pencil", &a.e2)] {
            ch.at_most(
                &format!("EI {k} {name}: largest residual"),
                worst_residual(&e.pairs),
                1e-8,
            );
            ch.equals(
                &format!("EI {k} {name}: negatives among four = min(nu, 4)"),
                negatives_among(&e.pairs),
                nu.min(4),
            );
            ch.holds(&format!("EI {k} {name}: SB"), sb_holds(e));
        }
        ch.at_most(&format!("LCI {k} free-tangent mode"), a.lc1.0, 1e-10);
        ch.at_most(&format!("LCI {k} pencil mode"), a.lc1.1, 1e-10);
        let (ka, kae) = kaa_min(&m, &it[k], &states, 16);
        let (da, dae, ratio, re) = mode_jump(&m, &it[0], &a0_modes, &it[k], &states);
        let s = &steps[k];
        let (dmax, dnode) =
            s.du.as_ref()
                .map(|d| largest_node(&m, &s.free, d))
                .unwrap_or((f64::NAN, usize::MAX));
        let (sh_d, se_d) = match (&phi, &s.du) {
            (Some(p), Some(d)) => (share_l(&lap, p, d), share_euclid(p, d)),
            _ => (f64::NAN, f64::NAN),
        };
        let total: Vec<f64> = free.iter().map(|&q| it[k][q] - it[0][q]).collect();
        let sh_t = match &phi {
            Some(p) if k > 0 => share_l(&lap, p, &total),
            _ => f64::NAN,
        };
        let tck = if k < kk {
            s.du.as_ref()
                .map(|d| tc(&m, &states, &it[k], &s.free, d, h))
                .unwrap_or(f64::NAN)
        } else if kk >= 1 {
            steps[kk - 1]
                .du
                .as_ref()
                .map(|d| tc(&m, &states, &it[k], &s.free, d, h))
                .unwrap_or(f64::NAN)
        } else {
            f64::NAN
        };
        if k == 0 {
            ch.at_most("TC0 at u_c along d_0", tck, 1e-6);
            ch.finite("SP: S_P = share(d_0)", sh_d);
        }
        let first1 = a.loc1.as_ref().map(|l| l.first).unwrap_or(usize::MAX);
        let first2 = a.loc2.as_ref().map(|l| l.first).unwrap_or(usize::MAX);
        println!(
            "LOCI {k}: r {:.3e}, energy {:.3e}, |d|_inf {dmax:.3e} at node {dnode}; nu {nu}, driver factor {df:?}; \
             lambda_1/(mu h) {l1:.6e} (first element {first1} {:?}{}), m_1 {m1:.6e} (first element {first2} {:?}{}); \
             KA {ka:.4e} at {kae} {:?}; modes jump max {da:.3e} at {dae}, ratio {ratio:.3e} at {re} {:?}; share(d_k) \
             {sh_d:.4e} (Euclidean {se_d:.4e}), share(u_k - u_c) {sh_t:.4e}; TC {tck:.3e}; {:.1} s",
            s.r,
            s.energy,
            grid(first1.min(4095), 16),
            if in_corner(first1.min(4095)) { ", in N" } else { "" },
            grid(first2.min(4095), 16),
            if in_corner(first2.min(4095)) { ", in N" } else { "" },
            grid(kae.min(4095), 16),
            grid(re.min(4095), 16),
            t0.elapsed().as_secs_f64()
        );
        for v in [l1, m1, ka, da, ratio, s.r] {
            ch.finite(&format!("LOCI {k}"), v);
        }
        if k < kk {
            ch.finite(&format!("TCI {k}"), tck);
        }
        if k == kk {
            last_first = (first1, a.loc1.as_ref().map(|l| l.hourglass).unwrap_or(f64::NAN));
            println!("JM at u_K: the largest mode ratio {ratio:.3e} at element {re}");
        }
    }

    // SH: the corner sweep.
    let fc = |e: usize, u: &[f64]| centre_f(&element_coords(m.mesh(), e), &element_u(&m, u, e));
    let ep = 3720;
    let mut sh = Vec::new();
    for (tag, e, k) in [
        ("E_c u_c", 3959, 0),
        ("E_c u_K", 3959, kk),
        ("E_p u_c", ep, 0),
        ("E_p u_K", ep, kk),
    ] {
        sh.push(corner_sweep(&format!("{tag} (element {e})"), &fc(e, &it[k])));
    }
    if last_first.0 != 3959 && last_first.0 != ep && last_first.0 < 4096 {
        corner_sweep(
            &format!("E_K u_K (element {})", last_first.0),
            &fc(last_first.0, &it[kk]),
        );
    }
    for (ml, mi) in &sh {
        ch.finite("SH m_L", *ml);
        ch.finite("SH m_Inc", *mi);
    }
    let sh_reading = if sh[1].0 <= 0.0 || sh[3].0 <= 0.0 {
        "the corner element's homogeneous state is unstable for rule L"
    } else if sh.iter().all(|s| s.0 >= 0.5) {
        "not a homogeneous-state property"
    } else {
        "between"
    };
    println!("SH reading: {sh_reading}");

    // The readings.
    let rs: Vec<f64> = steps.iter().map(|s| s.r).collect();
    let form_k = if kk == 1 {
        "the predictor's iterate".to_string()
    } else {
        let conv = (2..=kk).all(|k| rs[k] <= 0.5 * rs[k - 1]);
        let div = rs[kk] > rs[1];
        format!(
            "a later iterate, {}",
            if conv {
                "converging"
            } else if div {
                "diverging"
            } else {
                "stalling"
            }
        )
    };
    let sp = steps[0]
        .du
        .as_ref()
        .zip(phi.as_ref())
        .map(|(d, p)| share_l(&lap, p, d))
        .unwrap_or(f64::NAN);
    let cp = if sp >= 0.5 && in_corner(last_first.0.min(4095)) && last_first.0 < 4096 {
        "confirmed"
    } else if sp <= 0.1 {
        "refuted"
    } else {
        "undetermined"
    };
    println!(
        "READING: the failing iterate K = {kk}: {form_k}; residuals {rs:?}; CP {cp} (S_P {sp:.4e}, first element at u_K {} \
         {:?}, H {:.4}); total {:.1} s",
        last_first.0,
        grid(last_first.0.min(4095), 16),
        last_first.1,
        t_all.elapsed().as_secs_f64()
    );
    ch.finish();
}

#[test]
#[ignore = "ADR 0688: the heavy record of the line search and the damping of the predictor on the failing attempt and on the whole 16^3 solve, run once (187 s): LS-A, PD-A, LP-A fail with the text of ADR 0678 (the damping takes beta = 1: the lifted iterate inverts element 3959); LS-F, PD-F, LP-F fail from 0.9625 with a negative pivot at node 4768; f767_fast_rows covers PD8, LS8 on 8^3"]
fn f767_nh16_remedies_heavy() {
    let mut ch = Checks::new();
    let mesh16 = block_mesh(16);
    let m = block_model(&mesh16, K_L, DEFAULT, -DELTA);
    let states = initial_states(&m).unwrap();
    let ndof = m.dof_map().n_equations();
    let (_, log) = solve_observed(&mesh16, "block 16^3", K_L, &options(10));
    let Some(fa) = failing_attempt(&log) else {
        ch.holds("the failing attempt from 0.9625 observed", false);
        ch.finish();
        return;
    };
    let att = &log.attempts[fa];
    let (lc, lt) = (att.committed, att.attempted);
    let uc = &att.iterates[0];
    for (tag, o) in [
        ("LS-A", options(10).line_search(true)),
        ("PD-A", options(10).predictor_damping(true)),
        ("LP-A", options(10).line_search(true).predictor_damping(true)),
    ] {
        let t0 = Instant::now();
        match solve_increment(&m, &[], &vec![0.0; ndof], uc, &states, lc, lt, &o, &[]) {
            Ok(eq) => {
                let punch = m.mesh().node_set("punch").unwrap();
                let rz: f64 = punch
                    .iter()
                    .map(|&n| eq.internal_force[m.dof_map().equation(n, 2).unwrap()])
                    .sum();
                println!(
                    "{tag} {lc:.6} -> {lt:.6}: solves the attempt: residuals {:?}, step lengths {:?}, R/mu {:.8}, {:.1} s",
                    eq.residuals,
                    eq.step_lengths,
                    -rz / MU,
                    t0.elapsed().as_secs_f64()
                );
                ch.finite(&format!("{tag} R"), -rz / MU);
            }
            Err(e) => {
                println!(
                    "{tag} {lc:.6} -> {lt:.6}: fails: {e}, {:.1} s",
                    t0.elapsed().as_secs_f64()
                );
            }
        }
    }
    let mut passes = Vec::new();
    for (tag, o) in [
        ("LS-F", options(10).line_search(true)),
        ("PD-F", options(10).predictor_damping(true)),
        ("LP-F", options(10).line_search(true).predictor_damping(true)),
    ] {
        let hk = solve_hooked(&mesh16, &format!("{tag} block 16^3"), K_L, DEFAULT, &o);
        match hk.outcome {
            Ok((r, w)) => {
                println!("{tag}: solves to full load: R/mu {r:.8}, w_c {w:+.8}");
                ch.at_most(
                    &format!("{tag}: |R / 0.32599497 - 1|"),
                    (r / 0.32599497 - 1.0).abs(),
                    0.15,
                );
                ch.at_most(
                    &format!("{tag}: |w_c / 0.06060900 - 1|"),
                    (w / 0.06060900 - 1.0).abs(),
                    0.15,
                );
                passes.push((tag, r, w));
            }
            Err(e) => println!("{tag}: fails: {e}; last converged load factor {:.6}", hk.last),
        }
    }
    for i in 0..passes.len() {
        for j in i + 1..passes.len() {
            let (a, b) = (passes[i], passes[j]);
            ch.at_most(&format!("{} vs {}: R", a.0, b.0), (a.1 / b.1 - 1.0).abs(), 1e-6);
            ch.at_most(&format!("{} vs {}: w_c", a.0, b.0), (a.2 / b.2 - 1.0).abs(), 1e-6);
        }
    }
    ch.finish();
}
