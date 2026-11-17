//! F-774 (ADR 0694): the corner element of rule L at the punch corner and the equilibrium past the predictor. On the punch
//! block of ADR 0688 (rule L, neo-Hookean, `16^3`), along the lift and the predictor line of the failing attempt `0.9625 ->
//! 0.965625`: `det F` at the points of element 3959 and its 17 neighbours and their local mode solution, cold, warm and
//! continued (D1: CR, PB, AS, CT, WL, reading JR); the equilibrium at `0.965625` by the opt-in Newton iteration that accepts
//! an indefinite tangent (`NewtonOptions::indefinite_tangent`, D2), with its inertia, against F-bar at the same load (IN-A,
//! EQ); the opt-in backtracking of the predictor (`NewtonOptions::predictor_backtracking`, D3) on the attempt and on the whole
//! solve; the fast controls (D4: the `8^3` block, the column, the saddle on the ADR 0628 patch; D7: the controls of the
//! surviving mutations of ADR 0688). The helpers are copies of those of `si_eas_hg13.rs` (existing tests are not edited),
//! the eigen tool at 500 iterations in both branches. Every aggregated value is required finite before it is compared.

use rcfd_core::vec3::Vec3;
use rcfd_fem::arclength::{trace, ArcLengthOptions};
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::constraint::{prescribed_equations, reduce, Prescribed};
use rcfd_fem::element::{element_for, ElementState};
use rcfd_fem::material::{HyperelasticEnergy, PointContext};
use rcfd_fem::nl_incompatible::{
    condensed_force_and_tangent, mode_iteration_record, mode_iteration_record_from, modes_and_strains_with,
    point_gradients, IncompatibleModeUpdate, IncompatibleStabilisation,
};
use rcfd_fem::nonlinear::{
    initial_states, internal_force, solve, solve_from, solve_increment, solve_with, tangent, Contribution,
    DriverExtensions, LoadStep, NewtonOptions, NonlinearResult, StepControl,
};
use rcfd_fem::solve::DirectFactor;
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, Material, SolidFormulation, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{lowest_eigenpairs, symmetric_eigen, Factorisation, GeneralizedEigen, Shift, SubspaceOptions};
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
    /// The displacement and the states at full load of a solve that passes.
    end: Option<(Vec<f64>, Vec<ElementState>)>,
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
    let end = out
        .as_ref()
        .ok()
        .map(|r| (r.result.displacement_values().to_vec(), r.states.clone()));
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
    Hooked { outcome, last, end }
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

/// The four lowest pairs of `(K, M)` (`Shift::Auto`, the supernodal factor, tolerance `1e-8`, 500 iterations: ADR 0694).
fn lowest(k: &CsrMatrix, mm: &CsrMatrix, points: &[[f64; 3]]) -> Option<GeneralizedEigen> {
    let opts = SubspaceOptions {
        shift: Shift::Auto,
        tolerance: 1e-8,
        max_iterations: 500,
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

/// The reduced (interior) tangent of the patch at `u`, symmetrised, and the interior equations.
fn patch_tangent(m: &SolidModel, u: &[f64]) -> Option<(CsrMatrix, Vec<usize>)> {
    let k = tangent(m, u, &initial_states(m).unwrap(), Some(4)).ok()?;
    let sys = reduce(&k, &vec![0.0; k.n], &boundary_fixed(m)).ok()?;
    let free = sys.free().to_vec();
    Some((symmetrise(&sys.matrix).1, free))
}

// ============================================================================ D1: the corner record

const MODES: usize = 9;

fn det3(f: &[[f64; 3]; 3]) -> f64 {
    f[0][0] * (f[1][1] * f[2][2] - f[1][2] * f[2][1]) - f[0][1] * (f[1][0] * f[2][2] - f[1][2] * f[2][0])
        + f[0][2] * (f[1][0] * f[2][1] - f[1][1] * f[2][0])
}

/// `det F` at the 8 points of an Incompatible Hex8 for the modes `a` (zero modes: the compatible field).
fn point_dets(coords: &[Vec3], ue: &[f64], a: &[f64; MODES]) -> [f64; 8] {
    let pg = point_gradients(coords, ue, a).unwrap();
    std::array::from_fn(|q| {
        let (h, fm) = pg[q];
        let f: [[f64; 3]; 3] =
            std::array::from_fn(|i| std::array::from_fn(|j| h[i][j] + fm[i][j] + if i == j { 1.0 } else { 0.0 }));
        det3(&f)
    })
}

fn min8(d: &[f64; 8]) -> f64 {
    d.iter().fold(f64::INFINITY, |w, &v| w.min(v))
}

/// The outcome of one local iteration of the modes: the root, or the failure text and the accepted iterates before it.
#[derive(Clone)]
enum Local {
    Root([f64; MODES]),
    Fail(String, usize),
}

impl Local {
    fn root(&self) -> Option<&[f64; MODES]> {
        match self {
            Local::Root(a) => Some(a),
            Local::Fail(..) => None,
        }
    }
    /// C (converged), I0 (refused at the start), Ij (a local iterate j inverted), N (not converged), S (singular), E.
    fn class(&self) -> String {
        match self {
            Local::Root(_) => "C".to_string(),
            Local::Fail(t, j) if t.contains("inverted") => format!("I{j}"),
            Local::Fail(t, _) if t.contains("did not converge") => "N".to_string(),
            Local::Fail(t, _) if t.contains("singular") => "S".to_string(),
            Local::Fail(..) => "E".to_string(),
        }
    }
    fn text(&self) -> &str {
        match self {
            Local::Root(_) => "",
            Local::Fail(t, _) => t,
        }
    }
}

/// The local iteration of rule L with the default `Plain` update, cold (`start` None) or from `start`.
fn local(coords: &[Vec3], ue: &[f64], state: &ElementState, start: Option<&[f64; MODES]>) -> Local {
    let mat = neo_hookean();
    let ctx = PointContext::default();
    let rec = match start {
        None => mode_iteration_record(coords, &mat, ue, state, &ctx, LIN, IncompatibleModeUpdate::Plain),
        Some(s) => mode_iteration_record_from(
            coords,
            &mat,
            ue,
            state,
            &ctx,
            LIN,
            IncompatibleModeUpdate::Plain,
            Some(s),
        ),
    };
    match rec {
        Ok((_, Ok(a))) => Local::Root(a),
        Ok((its, Err(e))) => Local::Fail(e.to_string(), its.len()),
        Err(e) => Local::Fail(e.to_string(), 0),
    }
}

/// A point of the path of D1.
struct PathPoint {
    label: String,
    u: Vec<f64>,
}

/// The path of D1: the lift `L(t)`, `t = i / 16`, `i = 0..15`, then the predictor line `P(beta) = u_L + beta du`, `beta = j
/// / 64`, `j = 0..64` (`P(0) = L(1) = u_L`).
fn path(m: &SolidModel, uc: &[f64], free: &[usize], du: &[f64], lc: f64, lt: f64) -> Vec<PathPoint> {
    let fixed = prescribed_equations(m, &[]).unwrap();
    let mut out = Vec::new();
    for i in 0..16 {
        let t = i as f64 / 16.0;
        let mut x = uc.to_vec();
        for &(eq, v) in &fixed {
            x[eq] = if i == 0 { lc * v } else { (lc + t * (lt - lc)) * v };
        }
        out.push(PathPoint {
            label: format!("lift t {t:.4}"),
            u: x,
        });
    }
    let mut ul = uc.to_vec();
    for &(eq, v) in &fixed {
        ul[eq] = lt * v;
    }
    for j in 0..=64 {
        let beta = j as f64 / 64.0;
        let mut x = ul.clone();
        for (&q, d) in free.iter().zip(du) {
            x[q] += beta * d;
        }
        out.push(PathPoint {
            label: format!("beta {beta:.6}"),
            u: x,
        });
    }
    out
}

/// The index in the path of `beta = j / 64`.
fn at_beta(j: usize) -> usize {
    16 + j
}

/// The neighbourhood of D1: `(i, j, k)` with `i, j` within one of the centre and `k` the centre's layer and the one below.
fn neighbourhood(n: usize, c: (usize, usize, usize)) -> Vec<usize> {
    let mut out = Vec::new();
    for k in c.2 - 1..=c.2 {
        for j in c.1 - 1..=c.1 + 1 {
            for i in c.0 - 1..=c.0 + 1 {
                out.push(i + n * (j + n * k));
            }
        }
    }
    out.sort_unstable();
    out
}

/// The record of one element along the path.
struct Track {
    e: usize,
    cold: Vec<Local>,
    warm: Vec<Local>,
    cont: Vec<Local>,
    detc: Vec<[f64; 8]>,
    ratio: f64,
    switches: usize,
    cont_all: bool,
    min_det_cont: f64,
}

fn track(m: &SolidModel, states: &[ElementState], points: &[PathPoint], e: usize) -> Track {
    let coords = element_coords(m.mesh(), e);
    let ue0 = element_u(m, &points[0].u, e);
    let ac = local(&coords, &ue0, &states[e], None).root().copied();
    let (mut cold, mut warm, mut cont, mut detc) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut prev = ac;
    let (mut ratio, mut switches, mut cont_all, mut min_det_cont) = (0.0f64, 0usize, true, f64::INFINITY);
    let mut last: Option<([f64; MODES], Vec<f64>)> = None;
    for p in points {
        let ue = element_u(m, &p.u, e);
        let c = local(&coords, &ue, &states[e], None);
        let w = match &ac {
            Some(a) => local(&coords, &ue, &states[e], Some(a)),
            None => Local::Fail("no root at u_c".to_string(), 0),
        };
        let k = match &prev {
            Some(a) => local(&coords, &ue, &states[e], Some(a)),
            None => Local::Fail("no root to continue from".to_string(), 0),
        };
        detc.push(point_dets(&coords, &ue, &[0.0; MODES]));
        match k.root() {
            Some(a) => {
                min_det_cont = min_det_cont.min(min8(&point_dets(&coords, &ue, a)));
                if let Some((a0, u0)) = &last {
                    let du = norm(ue.iter().zip(u0).map(|(x, y)| x - y));
                    if du > 0.0 {
                        ratio = ratio.max(norm(a.iter().zip(a0).map(|(x, y)| x - y)) / du);
                    }
                }
                if let Some(b) = c.root() {
                    let diff = norm(a.iter().zip(b).map(|(x, y)| x - y));
                    if diff > 1e-6 * norm(a.iter().copied()).max(1e-12) {
                        switches += 1;
                    }
                }
                last = Some((*a, ue.clone()));
                prev = Some(*a);
            }
            None => cont_all = false,
        }
        cold.push(c);
        warm.push(w);
        cont.push(k);
    }
    Track {
        e,
        cold,
        warm,
        cont,
        detc,
        ratio,
        switches,
        cont_all,
        min_det_cont,
    }
}

fn fmt8(d: &[f64; 8]) -> String {
    d.iter().map(|v| format!("{v:.4e}")).collect::<Vec<_>>().join(" ")
}

/// The print of the focus element at a point.
fn report_point(m: &SolidModel, points: &[PathPoint], t: &Track, i: usize) {
    let coords = element_coords(m.mesh(), t.e);
    let ue = element_u(m, &points[i].u, t.e);
    let dets = |l: &Local| {
        l.root()
            .map(|a| fmt8(&point_dets(&coords, &ue, a)))
            .unwrap_or_else(|| "-".to_string())
    };
    println!(
        "DF element {} at {}: det F_c [{}]; cold {} {} [{}]; warm {} [{}]; continued {} [{}]",
        t.e,
        points[i].label,
        fmt8(&t.detc[i]),
        t.cold[i].class(),
        t.cold[i].text(),
        dets(&t.cold[i]),
        t.warm[i].class(),
        dets(&t.warm[i]),
        t.cont[i].class(),
        dets(&t.cont[i]),
    );
}

fn report_track(t: &Track, n: usize) {
    let fails: Vec<String> = t
        .cold
        .iter()
        .enumerate()
        .filter(|(_, c)| c.root().is_none())
        .map(|(i, c)| format!("{i}:{}", c.class()))
        .collect();
    let wfails = t.warm.iter().filter(|w| w.root().is_none()).count();
    let mindc = t.detc.iter().map(min8).fold(f64::INFINITY, f64::min);
    println!(
        "CT element {} {:?}: cold fails at {} points [{}]; warm fails at {wfails}; continued at every point {}; continuity \
         ratio {:.4e}; root switches {}; min det F_c {mindc:.4e}; min det F of the continued root {:.4e}",
        t.e,
        grid(t.e, n),
        fails.len(),
        fails.join(" "),
        t.cont_all,
        t.ratio,
        t.switches,
        t.min_det_cont
    );
}

/// The probe's texts of ADR 0688 (`f767-scratch/probe1.log`) at `beta = 0, 1/16, 1/8` (the index `j` of `j / 64`).
const PROBE: [(usize, &str, &str); 3] = [
    (0, "det F = -5.964573875984064e-2 at point 4", "element 3959: key 'deformation gradient': det F = -5.964573875984064e-2 at point 4 of an Incompatible Hex8 element (inverted)"),
    (4, "det F = -7.824161087854067e-1 at point 0", "element 3959: key 'deformation gradient': det F = -7.824161087854067e-1 at point 0 of an Incompatible Hex8 element (inverted)"),
    (8, "det F = -1.8630342959045024e1 at point 0", "element 3959: key 'deformation gradient': det F = -1.8630342959045024e1 at point 0 of an Incompatible Hex8 element (inverted)"),
];
/// The probe's admissible points (`j` of `j / 64`): `1/32, 1/4, 1/2, 1`.
const PROBE_OK: [usize; 4] = [2, 16, 32, 64];

/// The warm states of D1: the committed states with the cold roots of every element at `u_c` after them (ADR 0660 D1).
fn warm_states(m: &SolidModel, uc: &[f64], states: &[ElementState]) -> Option<Vec<ElementState>> {
    let modes = element_modes(m, uc, states);
    states
        .iter()
        .zip(modes)
        .map(|(s, a)| {
            a.map(|a| {
                let mut v = s.values().to_vec();
                v.extend_from_slice(&a);
                ElementState::new(v)
            })
        })
        .collect()
}

// ============================================================================ D4: the column and its replica

fn nh(mu: f64) -> Material {
    Material::hyperelastic(
        HyperelasticEnergy::neo_hookean(mu, 2.0 * mu * (1.0 + NU) / (3.0 * (1.0 - 2.0 * NU))).unwrap(),
        1000.0,
    )
    .unwrap()
}

const COLUMN_PUNCH: f64 = -1.2;

/// The column of D4: two Full Hex8 on `[0, 1]^2 x [0, 2]`, `mu = 1e6` below and `99e6` above, uniaxial strain, the top at
/// `-1.2`.
fn column_model() -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [1.0, 1.0, 2.0], [1, 1, 2]).unwrap();
    let every: Vec<usize> = (0..mesh.nodes().len()).collect();
    mesh.add_node_set("every", every).unwrap();
    mesh.add_element_set("lower", vec![0]).unwrap();
    mesh.add_element_set("upper", vec![1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("lower", nh(1e6)).unwrap();
    m.add_material("upper", nh(99e6)).unwrap();
    for (set, comp, value) in [
        ("every", 0, 0.0),
        ("every", 1, 0.0),
        ("zmin", 2, 0.0),
        ("zmax", 2, COLUMN_PUNCH),
    ] {
        m.add_constraint(Constraint::displacement(set, comp, value)).unwrap();
    }
    m
}

/// The part of a failure text the library's text must contain (an inversion from `det F`, a pivot, a non-convergence).
fn failure_core(t: &str) -> String {
    if let Some(i) = t.find("det F") {
        let end = t[i..].find(" (inverted)").map(|k| i + k + 11).unwrap_or(t.len());
        t[i..end].to_string()
    } else if let Some(i) = t.find("pivot ") {
        t[i..].split(" at ").next().unwrap_or("").to_string()
    } else {
        t.to_string()
    }
}

/// The test's replica of the backtracked attempt from rest to the full value (D4): `beta`, and the converged `u` or the
/// failure text.
fn column_replica(m: &SolidModel, states: &[ElementState]) -> (f64, Result<Vec<f64>, String>) {
    let fixed = prescribed_equations(m, &[]).unwrap();
    let u0 = vec![0.0; m.dof_map().n_equations()];
    let s0 = replicate_step(m, states, &u0, 0.0, 1.0, 0);
    let Some(du) = s0.du.clone() else {
        return (f64::NAN, Err("the predictor was refused".to_string()));
    };
    let admissible = |x: &[f64]| -> bool {
        if internal_force(m, x, states).is_err() {
            return false;
        }
        let Ok(kt) = tangent(m, x, states, Some(4)) else {
            return false;
        };
        let zero: Vec<(usize, f64)> = fixed.iter().map(|&(eq, _)| (eq, 0.0)).collect();
        let sys = reduce(&kt, &vec![0.0; x.len()], &zero).unwrap();
        driver_factor(&sys.matrix).is_none()
    };
    let trial = |b: f64| -> Vec<f64> {
        let mut x = u0.clone();
        for (&q, d) in s0.free.iter().zip(&du) {
            x[q] += b * d;
        }
        for &(eq, v) in &fixed {
            x[eq] = v;
        }
        x
    };
    let mut beta = 1.0;
    let mut found = false;
    for _ in 0..=10 {
        if admissible(&trial(beta)) {
            found = true;
            break;
        }
        beta *= 0.5;
    }
    if !found {
        beta = 1.0;
    }
    let mut u = u0.clone();
    for (&q, d) in s0.free.iter().zip(&du) {
        if beta == 1.0 {
            u[q] += d;
        } else {
            u[q] += beta * d;
        }
    }
    for &(eq, v) in &fixed {
        u[eq] = v;
    }
    let (e_first, mut e_last) = (s0.energy, s0.energy);
    for k in 1..=20 {
        if let Err(e) = internal_force(m, &u, states) {
            return (beta, Err(e.to_string()));
        }
        let s = replicate_step(m, states, &u, 0.0, 1.0, k);
        if s.r <= 1e-10 && e_last <= 1e-10 * e_first {
            return (beta, Ok(u));
        }
        if k == 20 {
            return (beta, Err("not converged in 20 iterations".to_string()));
        }
        match (s.next, s.refusal) {
            (Some(next), _) => {
                e_last = s.energy;
                u = next;
            }
            (None, Some((_, _, d))) => return (beta, Err(format!("pivot {d:e}"))),
            (None, None) => return (beta, Err("the replica failed".to_string())),
        }
    }
    unreachable!()
}

/// The backtracked step length of a driver outcome: `step_lengths[0]` of a converged attempt, or the note of a failure (1
/// when there is none).
fn beta_of(r: &Result<rcfd_fem::nonlinear::Equilibrium, FemError>) -> f64 {
    match r {
        Ok(eq) => eq.step_lengths.first().copied().unwrap_or(f64::NAN),
        Err(e) => {
            let t = e.to_string();
            match t.find("backtracked to beta = ") {
                Some(i) => t[i + 22..]
                    .split(' ')
                    .next()
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(f64::NAN),
                None => 1.0,
            }
        }
    }
}

// ============================================================================ the measures of the block

/// `R = -sum_punch f_z / mu` and `w_c` of a state of the block.
fn block_measures(m: &SolidModel, u: &[f64], fint: &[f64]) -> (f64, f64) {
    let dofs = m.dof_map();
    let punch = m.mesh().node_set("punch").unwrap();
    let rz: f64 = punch.iter().map(|&n| fint[dofs.equation(n, 2).unwrap()]).sum();
    let corner = m.mesh().node_set("corner").unwrap()[0];
    (-rz / MU, u[dofs.equation(corner, 2).unwrap()])
}

fn inertia_at(m: &SolidModel, u: &[f64], states: &[ElementState]) -> Option<usize> {
    let (k, _) = free_tangent(m, u, states);
    inertia(&symmetrise(&k).1)
}

const K_FBAR: Kind = Kind {
    name: "Fbar",
    formulation: FBAR,
    rule: OFF,
};

// ============================================================================ the fast test

#[test]
#[ignore = "ADR 0694: the registered fast run, known miss (a defective set-up, amendment 1): the column's committed displacement had +0.0 where lc v = -0.0, so solve_increment refused the three column attempts at the input (key 'displacement'); BTC missed, DC met only vacuously; every other row met; f774_fast_amended runs the corrected column"]
fn f774_fast_known_miss() {
    fast(false);
}

/// The corrected fast test of ADR 0694 amendment 1: the column's committed displacement carries `0 x v` at the prescribed
/// equations, and DC requires an inverted point.
#[test]
fn f774_fast_amended() {
    fast(true);
}

fn fast(corrected: bool) {
    let mut ch = Checks::new();
    let t_all = Instant::now();
    let mesh8 = block_mesh(8);
    let m8 = block_model(&mesh8, K_L, DEFAULT, -DELTA);
    let states8 = initial_states(&m8).unwrap();

    // IN8, BT8, IB8.
    let mut bt8 = (f64::NAN, f64::NAN);
    for (tag, o) in [
        ("IN8", options(10).indefinite_tangent(true)),
        ("BT8", options(10).predictor_backtracking(true)),
        ("IB8", options(10).indefinite_tangent(true).predictor_backtracking(true)),
    ] {
        let h = solve_hooked(&mesh8, &format!("{tag} block 8^3"), K_L, DEFAULT, &o);
        let (r, w) = h.outcome.unwrap_or((f64::NAN, f64::NAN));
        if tag == "BT8" {
            bt8 = (r, w);
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

    // INX: the last observed 8^3 attempt, default and indefinite.
    let (_, log8) = solve_observed(&mesh8, "block 8^3", K_L, &options(10));
    let last = log8.attempts.last().unwrap();
    let zero8 = vec![0.0; m8.dof_map().n_equations()];
    let run = |o: &NewtonOptions| {
        solve_increment(
            &m8,
            &[],
            &zero8,
            &last.iterates[0],
            &states8,
            last.committed,
            last.attempted,
            o,
            &[],
        )
    };
    let (d, i) = (run(&options(10)), run(&options(10).indefinite_tangent(true)));
    let inx = match (&d, &i) {
        (Ok(a), Ok(b)) => {
            let umax = a.u.iter().fold(0.0f64, |w, v| w.max(v.abs()));
            let dmax = a.u.iter().zip(&b.u).fold(0.0f64, |w, (x, y)| w.max((x - y).abs()));
            println!("INX: bit-equal {}", bits_equal(&a.u, &b.u));
            dmax / umax
        }
        _ => f64::NAN,
    };
    ch.at_most("INX: max |du| / max |u|, default against indefinite", inx, 1e-12);

    // BT1: one increment with the backtracking.
    let b1 = solve_hooked(
        &mesh8,
        "BT1 block 8^3",
        K_L,
        DEFAULT,
        &options(1).predictor_backtracking(true),
    );
    println!("BT1: converges {}", b1.outcome.is_ok());
    if let Ok((r, w)) = b1.outcome {
        ch.at_most("BT1: R against BT8's", (r / bt8.0 - 1.0).abs(), 1e-6);
        ch.at_most("BT1: w_c against BT8's", (w / bt8.1 - 1.0).abs(), 1e-6);
    }

    // RF2: the refusals.
    let key_rows: Vec<(&str, Result<NonlinearResult, FemError>, &str)> = vec![
        (
            "indefinite with unsymmetric",
            solve(
                &m8,
                &[],
                &[],
                &NewtonOptions::new(1).indefinite_tangent(true).unsymmetric(true),
            ),
            "indefinite_tangent",
        ),
        (
            "backtracking with unsymmetric",
            solve(
                &m8,
                &[],
                &[],
                &NewtonOptions::new(1).predictor_backtracking(true).unsymmetric(true),
            ),
            "predictor_backtracking",
        ),
        (
            "backtracking with damping",
            solve(
                &m8,
                &[],
                &[],
                &NewtonOptions::new(1)
                    .predictor_backtracking(true)
                    .predictor_damping(true),
            ),
            "predictor_backtracking",
        ),
    ];
    for (tag, r, key) in &key_rows {
        ch.holds(&format!("RF2 {tag}: {:?}", r.as_ref().err()), key_of(r) == Some(*key));
    }
    for (tag, o, key) in [
        (
            "arc length, indefinite",
            NewtonOptions::new(1).indefinite_tangent(true),
            "indefinite_tangent",
        ),
        (
            "arc length, backtracking",
            NewtonOptions::new(1).predictor_backtracking(true),
            "predictor_backtracking",
        ),
    ] {
        let r = trace(&m8, &[], &[], &ArcLengthOptions::new(0.1).newton(o), &[], &mut |_| true);
        ch.holds(&format!("RF2 {tag}: {:?}", r.as_ref().err()), key_of(&r) == Some(key));
    }
    let log = RefCell::new(Log::default());
    let obs = Observer { log: &log };
    let rf = solve_with(
        &m8,
        &[],
        &[],
        &options(1).predictor_backtracking(true),
        DriverExtensions {
            contributions: vec![&obs],
            step_control: None,
        },
    );
    let t = rf.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
    ch.holds(
        &format!("RF2 backtracking with a contribution: {t}"),
        t.contains("predictor_backtracking"),
    );

    // INC, NC-I: the saddle on the ADR 0628 patch.
    let h6 = 1.0 / 6.0;
    let fc49 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.93]];
    let mut saddle = None;
    for (tag, rule) in [("INC plain 0.93", OFF), ("NC-I rule L 0.93", LIN)] {
        let m = patch(6, INC, rule);
        let uh = homogeneous_f(&m, &fc49);
        let states = initial_states(&m).unwrap();
        let boundary = boundary_nodes(m.mesh());
        let dofs = m.dof_map();
        let mut pres = Vec::new();
        for &n in &boundary {
            for c in 0..3 {
                pres.push(Prescribed::new(n, c, uh[dofs.equation(n, c).unwrap()]));
            }
        }
        let mut start = uh.clone();
        let mut rng = Rng::new(0xF774_0694);
        for n in 0..m.mesh().nodes().len() {
            if !boundary.contains(&n) {
                for c in 0..3 {
                    start[dofs.equation(n, c).unwrap()] += 1e-4 * h6 * (2.0 * rng.uniform() - 1.0);
                }
            }
        }
        let zero = vec![0.0; uh.len()];
        let def = solve_from(&m, &pres, &zero, &start, &states, &options(1), &[]);
        let ind = solve_from(
            &m,
            &pres,
            &zero,
            &start,
            &states,
            &options(1).indefinite_tangent(true),
            &[],
        );
        println!(
            "{tag}: default {:?}; indefinite {:?}",
            def.as_ref().map(|e| e.residuals.clone()).map_err(|e| e.to_string()),
            ind.as_ref().map(|e| e.residuals.clone()).map_err(|e| e.to_string())
        );
        if rule == OFF {
            let refused = def
                .as_ref()
                .err()
                .is_some_and(|e| e.to_string().contains("singular tangent"));
            ch.holds(&format!("{tag}: the default refuses (singular tangent)"), refused);
            match &ind {
                Ok(eq) => {
                    let dist = eq.u.iter().zip(&uh).fold(0.0f64, |w, (a, b)| w.max((a - b).abs())) / h6;
                    ch.at_most(&format!("{tag}: |u - u_hom|_inf / h"), dist, 1e-6);
                    let nu = patch_tangent(&m, &eq.u)
                        .and_then(|(k, _)| inertia(&k))
                        .unwrap_or(usize::MAX);
                    ch.equals(&format!("{tag}: the inertia at the result"), nu, 21);
                }
                Err(e) => ch.holds(&format!("{tag}: the indefinite iteration converges ({e})"), false),
            }
            saddle = Some((m, uh));
        } else {
            match (&def, &ind) {
                (Ok(a), Ok(b)) => ch.holds(
                    &format!("{tag}: default and indefinite bit-equal"),
                    bits_equal(&a.u, &b.u),
                ),
                _ => ch.holds(&format!("{tag}: default and indefinite both converge"), false),
            }
        }
    }

    // NC-S3, SS on the saddle; DG.
    let (ms, uh) = saddle.unwrap();
    let (k, free) = patch_tangent(&ms, &uh).unwrap();
    let lap = laplacian_sparse(&ms, &free);
    let points = points_of(&ms, &free);
    let e1 = lowest_any(&k, &identity(free.len()), &points);
    let e2 = lowest_any(&k, &lap, &points);
    match &e2.pairs {
        Some(p) => {
            let phi = &p.vectors[0];
            let mut rng = Rng::new(0xF774_0694);
            let x: Vec<f64> = (0..free.len()).map(|_| 2.0 * rng.uniform() - 1.0).collect();
            let mut lphi = vec![0.0; lap.n];
            lap.matvec(phi, &mut lphi);
            let c = lphi.iter().zip(&x).map(|(a, b)| a * b).sum::<f64>()
                / lphi.iter().zip(phi).map(|(a, b)| a * b).sum::<f64>();
            let d: Vec<f64> = x.iter().zip(phi).map(|(a, b)| a - c * b).collect();
            println!("NC-S3: Euclidean share of d {:.3e}", share_euclid(phi, &d));
            ch.at_most(
                "NC-S3: share_l(phi, d) of the L-orthogonal complement",
                share_l(&lap, phi, &d),
                1e-20,
            );
            ch.at_least("NC-S3: share_l(phi, phi)", share_l(&lap, phi, phi), 1.0 - 1e-10);
        }
        None => ch.holds("NC-S3: the saddle's pencil pairs", false),
    }
    let ss = |a: &AnyLowest| match (a.bracket, a.pairs.as_ref()) {
        (Some((_, hi)), Some(e)) => (e.shift / (1.05 * hi) - 1.0).abs(),
        _ => f64::NAN,
    };
    ch.at_most("SS saddle (K, I): the shift against 1.05 s_hi", ss(&e1), 1e-15);
    let n = 40;
    let dg: Vec<f64> = (0..n)
        .map(|i| match i {
            0 => -2.0,
            1 => -1.0,
            _ => 0.5 + 0.1 * (i - 2) as f64,
        })
        .collect();
    let kd = CsrMatrix::from_triplets(n, dg.iter().enumerate().map(|(i, &v)| (i, i, v)).collect());
    let pd: Vec<[f64; 3]> = (0..n).map(|i| [i as f64, 0.0, 0.0]).collect();
    let ed = lowest_any(&kd, &identity(n), &pd);
    ch.equals("DG: nu", ed.nu.unwrap_or(usize::MAX), 2);
    ch.at_most("SS DG: the shift against 1.05 s_hi", ss(&ed), 1e-15);
    ch.holds("DG: SB", sb_holds(&ed));
    for (j, want) in [-2.0, -1.0, 0.5, 0.6].into_iter().enumerate() {
        let got = ed.pairs.as_ref().map(|e| e.values[j]).unwrap_or(f64::NAN);
        ch.at_most(&format!("DG: value {j} against {want}"), (got / want - 1.0).abs(), 1e-8);
    }

    // BTC, DC: the column.
    let mc = column_model();
    let sc = initial_states(&mc).unwrap();
    let mut zc = vec![0.0; mc.dof_map().n_equations()];
    if corrected {
        for (eq, v) in prescribed_equations(&mc, &[]).unwrap() {
            zc[eq] = 0.0 * v;
        }
    }
    let (beta_r, rep) = column_replica(&mc, &sc);
    println!(
        "BTC replica: beta {beta_r}, {:?}",
        rep.as_ref().map(|_| "converged").map_err(|e| e.clone())
    );
    ch.holds("BTC: the replica's beta = 1/2", beta_r == 0.5);
    let lib = solve_increment(
        &mc,
        &[],
        &zc,
        &zc,
        &sc,
        0.0,
        1.0,
        &options(10).predictor_backtracking(true),
        &[],
    );
    println!(
        "BTC library: {:?}",
        lib.as_ref()
            .map(|e| (e.residuals.clone(), e.step_lengths.clone()))
            .map_err(|e| e.to_string())
    );
    let btc = match (&lib, &rep) {
        (Ok(eq), Ok(u)) => bits_equal(&eq.u, u) && eq.step_lengths.first() == Some(&0.5),
        (Err(e), Err(t)) => {
            let text = e.to_string();
            text.contains(&failure_core(t)) && text.contains("beta = 5e-1")
        }
        _ => false,
    };
    ch.holds("BTC: the library's backtracked attempt equals the replica's", btc);
    ch.holds("BTC: beta of the library = 1/2", beta_of(&lib) == 0.5);
    let dflt = solve_increment(&mc, &[], &zc, &zc, &sc, 0.0, 1.0, &options(10), &[]);
    let damp = solve_increment(
        &mc,
        &[],
        &zc,
        &zc,
        &sc,
        0.0,
        1.0,
        &options(10).predictor_damping(true),
        &[],
    );
    let (td, tp) = (
        dflt.as_ref().err().map(|e| e.to_string()),
        damp.as_ref().err().map(|e| e.to_string()),
    );
    println!("DC: default {td:?}; damped {tp:?}");
    ch.holds(
        "DC: the default and the damped attempts fail with the same text",
        td.is_some() && td == tp && (!corrected || td.as_deref().is_some_and(|t| t.contains("inverted"))),
    );

    // CR8: the corner record on the last 8^3 attempt.
    let s0 = replicate_step(&m8, &states8, &last.iterates[0], last.committed, last.attempted, 0);
    let du = s0.du.clone().unwrap();
    let pts = path(&m8, &last.iterates[0], &s0.free, &du, last.committed, last.attempted);
    ch.holds("CR8: L(0) bit-equal to u_c", bits_equal(&pts[0].u, &last.iterates[0]));
    ch.holds(
        "CR8: P(1) bit-equal to u_1",
        bits_equal(&pts[at_beta(64)].u, &last.iterates[1]),
    );
    let els = neighbourhood(8, (3, 3, 7));
    let mut rt = 0;
    let mut wc = true;
    for &e in &els {
        let t = track(&m8, &states8, &pts, e);
        if let Some(a) = t.cold[0].root() {
            rt += 1;
            wc &= t.warm[0].root().is_some_and(|b| bits_equal(a, b));
        } else {
            wc = false;
        }
        report_track(&t, 8);
        if e == 475 {
            report_point(&m8, &pts, &t, 0);
            report_point(&m8, &pts, &t, at_beta(64));
        }
    }
    ch.equals("CR8: the cold iteration converges at u_c in the 18 elements", rt, 18);
    ch.holds("CR8: the warm iteration from the root returns it bit for bit", wc);
    println!("f774_fast: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}

// ============================================================================ the heavy records

#[test]
#[ignore = "ADR 0694: the heavy record of the corner field on 16^3 (D1), run once with --ignored (59 s, every row met): cold 3959 fails at 7 of 81 points, warm and continued converge at all 81 in the 18 elements with no root switch; reading JR: a jump of the local solution; f774_fast_amended covers the same helpers on 8^3"]
fn f774_nh16_corner_heavy() {
    let mut ch = Checks::new();
    let t_all = Instant::now();
    let mesh16 = block_mesh(16);
    let m = block_model(&mesh16, K_L, DEFAULT, -DELTA);
    let states = initial_states(&m).unwrap();

    let (outcome, log) = solve_observed(&mesh16, "block 16^3", K_L, &options(10));
    let text = outcome.as_ref().err().cloned().unwrap_or_default();
    ch.holds("OB: the error text of ADR 0678", text == ADR0678_TEXT);
    let Some(fa) = failing_attempt(&log) else {
        ch.holds("OB: the failing attempt from 0.9625 observed", false);
        ch.finish();
        return;
    };
    let att = &log.attempts[fa];
    let (lc, lt) = (att.committed, att.attempted);
    let uc = &att.iterates[0];
    let s0 = replicate_step(&m, &states, uc, lc, lt, 0);
    let du = s0.du.clone().unwrap();
    let pts = path(&m, uc, &s0.free, &du, lc, lt);
    ch.holds("PL: L(0) bit-equal to u_c", bits_equal(&pts[0].u, uc));
    ch.holds(
        "PL: P(1) bit-equal to u_1",
        att.iterates.len() >= 2 && bits_equal(&pts[at_beta(64)].u, &att.iterates[1]),
    );
    println!("the path: {} points, {:.1} s", pts.len(), t_all.elapsed().as_secs_f64());

    // RT, WC, PB, DF, CT over the 18 elements.
    let els = neighbourhood(16, (7, 7, 15));
    let (mut rt, mut wc) = (0usize, true);
    let mut focus = None;
    for &e in &els {
        let t = track(&m, &states, &pts, e);
        if let Some(a) = t.cold[0].root() {
            rt += 1;
            wc &= t.warm[0].root().is_some_and(|b| bits_equal(a, b));
        } else {
            wc = false;
        }
        report_track(&t, 16);
        if e == 3959 || e == 3720 {
            for i in [0, 8, 15]
                .into_iter()
                .chain([0, 2, 4, 8, 16, 32, 48, 64].into_iter().map(at_beta))
            {
                report_point(&m, &pts, &t, i);
            }
        }
        if e == 3959 {
            focus = Some(t);
        }
    }
    ch.equals("RT: the cold iteration converges at u_c in the 18 elements", rt, 18);
    ch.holds("WC: the warm iteration from a_c returns a_c bit for bit", wc);
    let t = focus.unwrap();
    for (j, core, _) in PROBE {
        let c = &t.cold[at_beta(j)];
        ch.holds(
            &format!(
                "PB: 3959 cold at beta {j}/64 fails with the probe's '{core}' (class {}, {})",
                c.class(),
                c.text()
            ),
            c.root().is_none() && c.text().contains(core),
        );
    }
    for j in PROBE_OK {
        ch.holds(
            &format!("PB: 3959 cold converges at beta {j}/64"),
            t.cold[at_beta(j)].root().is_some(),
        );
    }
    ch.finite("CT 3959: continuity ratio", t.ratio);
    // The reading JR.
    let reads: Vec<&str> = PROBE
        .iter()
        .map(|&(j, _, _)| {
            let i = at_beta(j);
            let cold_fails = t.cold[i].root().is_none();
            let cont = t.cont[i].root().is_some();
            if cold_fails && cont && t.ratio <= 10.0 && t.cont_all {
                "a jump of the local solution"
            } else if !cont && t.warm[i].root().is_none() {
                "an inversion of the root"
            } else {
                "between"
            }
        })
        .collect();
    let jr = if reads.iter().all(|r| *r == "a jump of the local solution") {
        "a jump of the local solution"
    } else if reads.iter().all(|r| *r == "an inversion of the root") {
        "an inversion of the root"
    } else {
        "between"
    };
    println!("READING JR: per probe point {reads:?}; overall: {jr}");

    // AS: the whole model along the path.
    let mut fails = Vec::new();
    for (i, p) in pts.iter().enumerate() {
        let r = internal_force(&m, &p.u, &states);
        if let Err(e) = &r {
            fails.push(format!("{i} ({}): {e}", p.label));
        }
        for &(j, _, full) in &PROBE {
            if i == at_beta(j) {
                let got = r.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
                ch.holds(&format!("AS at beta {j}/64: the probe's text ({got})"), got == full);
            }
        }
        if PROBE_OK.iter().any(|&j| i == at_beta(j)) {
            ch.holds(&format!("AS at {}: the evaluation succeeds", p.label), r.is_ok());
        }
    }
    println!(
        "AS: the whole-model evaluation fails at {} of {} points:",
        fails.len(),
        pts.len()
    );
    for f in &fails {
        println!("  AS {f}");
    }

    // WL: the warm model along the predictor line.
    let mut mw = block_model(&mesh16, K_L, DEFAULT, -DELTA);
    mw.set_incompatible_mode_warm_start(true);
    match warm_states(&m, uc, &states) {
        Some(sw) => {
            for j in 0..=16 {
                let x = &pts[at_beta(4 * j)].u;
                match internal_force(&mw, x, &sw) {
                    Ok(f) => {
                        let s: f64 = s0.free.iter().zip(&du).map(|(&q, d)| d * f[q]).sum();
                        let (k, _) = free_tangent(&mw, x, &sw);
                        let nu = inertia(&symmetrise(&k).1);
                        let df = driver_factor(&k).is_none();
                        println!(
                            "WL beta {}/16: slope {s:.6e}, nu {nu:?}, the driver's factor passes {df}",
                            j
                        );
                    }
                    Err(e) => println!("WL beta {}/16: the warm evaluation fails: {e}", j),
                }
            }
        }
        None => ch.holds("WL: the cold roots at u_c of every element", false),
    }
    println!("f774_nh16_corner_heavy: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}

#[test]
#[ignore = "ADR 0694: the heavy record of the indefinite Newton, the F-bar comparison and the backtracking on 16^3 (D2, D3), run once with --ignored (189 s, every row met): the indefinite Newton converges at 0.965625 with nu 1, 3.9 per cent from F-bar (reading EQ: an unstable equilibrium on the branch of u_c); IN-F and IB-F solve to full load, BT-F fails; f774_fast_amended covers the opt-ins on 8^3, the column and the saddle"]
fn f774_nh16_paths_heavy() {
    let mut ch = Checks::new();
    let t_all = Instant::now();
    let mesh16 = block_mesh(16);
    let m = block_model(&mesh16, K_L, DEFAULT, -DELTA);
    let states = initial_states(&m).unwrap();
    let ndof = m.dof_map().n_equations();
    let zero = vec![0.0; ndof];
    let h = 1.0 / 16.0;

    let (outcome, log) = solve_observed(&mesh16, "block 16^3", K_L, &options(10));
    let text = outcome.as_ref().err().cloned().unwrap_or_default();
    ch.holds("OB: the error text of ADR 0678", text == ADR0678_TEXT);
    let Some(fa) = failing_attempt(&log) else {
        ch.holds("OB: the failing attempt from 0.9625 observed", false);
        ch.finish();
        return;
    };
    let att = &log.attempts[fa];
    let (lc, lt) = (att.committed, att.attempted);
    let uc = att.iterates[0].clone();
    let u1 = att.iterates.get(1).cloned().unwrap_or_default();
    let fc = internal_force(&m, &uc, &states).unwrap();
    let (rc, wcc) = block_measures(&m, &uc, &fc);
    println!(
        "u_c: R/mu {rc:.8}, w_c {wcc:+.8}, {:.1} s",
        t_all.elapsed().as_secs_f64()
    );

    // FB: F-bar at the same loads.
    let mut fb = Vec::new();
    for lf in [lc, lt] {
        let mf = block_model(&mesh16, K_FBAR, DEFAULT, -DELTA * lf);
        let t0 = Instant::now();
        match solve(&mf, &[], &[], &options(10)) {
            Ok(r) => {
                let (rr, ww) = measures(&mf, &r);
                println!(
                    "FB F-bar at {lf:.6}: R/mu {rr:.8}, w_c {ww:+.8}, {:.1} s",
                    t0.elapsed().as_secs_f64()
                );
                fb.push((rr, ww));
            }
            Err(e) => {
                println!("FB F-bar at {lf:.6}: fails: {e}");
                fb.push((f64::NAN, f64::NAN));
            }
        }
    }
    ch.holds(
        "FB: the F-bar solves at 0.9625 and 0.965625 converge",
        fb.iter().all(|p| p.0.is_finite()),
    );
    let gap_c = (rc / fb[0].0 - 1.0, wcc / fb[0].1 - 1.0);
    println!("FB gaps at u_c: g_R {:+.6e}, g_w {:+.6e}", gap_c.0, gap_c.1);

    // IN-A: the indefinite Newton iteration from u_c, observed.
    let ilog = RefCell::new(Log::default());
    ilog.borrow_mut().attempts.push(AttemptLog {
        committed: lc,
        attempted: lt,
        converged: None,
        iterates: Vec::new(),
    });
    let obs = Observer { log: &ilog };
    let t0 = Instant::now();
    let ina = solve_increment(
        &m,
        &[],
        &zero,
        &uc,
        &states,
        lc,
        lt,
        &options(10).indefinite_tangent(true),
        &[&obs],
    );
    println!(
        "IN-A {lc:.6} -> {lt:.6}: {}, {:.1} s",
        match &ina {
            Ok(eq) => format!("converges: residuals {:?}", eq.residuals),
            Err(e) => format!("fails: {e}"),
        },
        t0.elapsed().as_secs_f64()
    );
    let iterates = ilog.into_inner().attempts.pop().unwrap().iterates;
    for (k, x) in iterates.iter().enumerate() {
        let nu = if internal_force(&m, x, &states).is_ok() {
            inertia_at(&m, x, &states)
        } else {
            None
        };
        println!(
            "IN-A iterate {k}: nu {nu:?}, |u_k - u_c|_inf {:.3e}, |u_k - u_1|_inf {:.3e}",
            x.iter().zip(&uc).fold(0.0f64, |w, (a, b)| w.max((a - b).abs())),
            x.iter().zip(&u1).fold(0.0f64, |w, (a, b)| w.max((a - b).abs()))
        );
    }
    let mut ustar = None;
    let reading = match &ina {
        Ok(eq) => {
            let nu = inertia_at(&m, &eq.u, &eq.states).unwrap_or(usize::MAX);
            let (rs, ws) = block_measures(&m, &eq.u, &eq.internal_force);
            let gap = (rs / fb[1].0 - 1.0, ws / fb[1].1 - 1.0);
            println!(
                "EQ u*: nu* {nu}, R*/mu {rs:.8}, w_c* {ws:+.8}; gaps g_R {:+.6e}, g_w {:+.6e}; |u* - u_1|_inf {:.3e}, |u* - \
                 u_c|_inf {:.3e}",
                gap.0,
                gap.1,
                eq.u.iter().zip(&u1).fold(0.0f64, |w, (a, b)| w.max((a - b).abs())),
                eq.u.iter().zip(&uc).fold(0.0f64, |w, (a, b)| w.max((a - b).abs()))
            );
            ch.at_most("EQ: |R* / R_Fb - 1| at 0.965625", (rs / fb[1].0 - 1.0).abs(), 0.15);
            ch.at_most("EQ: |w* / w_Fb - 1| at 0.965625", (ws / fb[1].1 - 1.0).abs(), 0.15);
            let continuous = (gap.0 - gap_c.0).abs() <= 0.01 && (gap.1 - gap_c.1).abs() <= 0.01;
            println!(
                "EQ gap changes: R {:.3e}, w_c {:.3e}; continuous {continuous}",
                (gap.0 - gap_c.0).abs(),
                (gap.1 - gap_c.1).abs()
            );
            // EI*: the lowest pairs at u*.
            let (k, free) = free_tangent(&m, &eq.u, &eq.states);
            let ks = symmetrise(&k).1;
            let points = points_of(&m, &free);
            let lap = laplacian_sparse(&m, &free);
            let tangents = element_tangents(&m, K_L, &eq.u, &eq.states);
            let edge = edge_elements(m.mesh());
            for (name, mm) in [("(K_ff, I)", identity(free.len())), ("pencil", lap)] {
                let a = lowest_any(&ks, &mm, &points);
                ch.at_most(&format!("EI* {name}: largest residual"), worst_residual(&a.pairs), 1e-8);
                ch.equals(
                    &format!("EI* {name}: negatives among four = min(nu*, 4)"),
                    negatives_among(&a.pairs),
                    nu.min(4),
                );
                ch.holds(&format!("EI* {name}: SB"), sb_holds(&a));
                if let Some(e) = &a.pairs {
                    let scale = if name == "pencil" { MU } else { MU * h };
                    println!(
                        "EI* {name}: lowest four / scale {:?}",
                        e.values.iter().map(|v| v / scale).collect::<Vec<_>>()
                    );
                    let loc = locate(
                        &m,
                        &tangents,
                        &expand(&free, ndof, &e.vectors[0]),
                        e.values[0] < 0.0,
                        &edge,
                        &eq.u,
                    );
                    report_location(&format!("EI* {name} lowest mode"), &loc, 16, &edge);
                }
            }
            ustar = Some(eq.u.clone());
            if !continuous {
                "another stationary point"
            } else if nu == 0 {
                "a stable equilibrium past the predictor"
            } else {
                "an unstable equilibrium on the branch of u_c"
            }
        }
        Err(_) => "not reached",
    };
    println!("READING EQ: {reading}");

    // BT-A, IB-A.
    for (tag, o) in [
        ("BT-A", options(10).predictor_backtracking(true)),
        (
            "IB-A",
            options(10).predictor_backtracking(true).indefinite_tangent(true),
        ),
    ] {
        let t0 = Instant::now();
        let r = solve_increment(&m, &[], &zero, &uc, &states, lc, lt, &o, &[]);
        let beta = beta_of(&r);
        match &r {
            Ok(eq) => {
                let nu = inertia_at(&m, &eq.u, &eq.states);
                let (rs, ws) = block_measures(&m, &eq.u, &eq.internal_force);
                println!(
                    "{tag}: converges: residuals {:?}, step lengths {:?}, nu {nu:?}, R/mu {rs:.8}, w_c {ws:+.8}, {:.1} s",
                    eq.residuals,
                    eq.step_lengths,
                    t0.elapsed().as_secs_f64()
                );
                if let (Some(us), "IB-A") = (&ustar, tag) {
                    let umax = us.iter().fold(0.0f64, |w, v| w.max(v.abs()));
                    let d = eq.u.iter().zip(us).fold(0.0f64, |w, (a, b)| w.max((a - b).abs()));
                    println!("IB-A against IN-A: |u_IB - u*|_inf / |u*|_inf {:.3e}", d / umax);
                }
            }
            Err(e) => println!("{tag}: fails: {e}, {:.1} s", t0.elapsed().as_secs_f64()),
        }
        ch.holds(&format!("{tag}: beta_0 = 0.5 (measured {beta})"), beta == 0.5);
    }

    // BT-F, IN-F, IB-F.
    let mut passes = Vec::new();
    for (tag, o) in [
        ("BT-F", options(10).predictor_backtracking(true)),
        ("IN-F", options(10).indefinite_tangent(true)),
        (
            "IB-F",
            options(10).predictor_backtracking(true).indefinite_tangent(true),
        ),
    ] {
        let hk = solve_hooked(&mesh16, &format!("{tag} block 16^3"), K_L, DEFAULT, &o);
        match hk.outcome {
            Ok((r, w)) => {
                let nu = hk.end.as_ref().and_then(|(u, s)| inertia_at(&m, u, s));
                println!("{tag}: solves to full load: R/mu {r:.8}, w_c {w:+.8}, nu at full load {nu:?}");
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
                if nu == Some(0) {
                    passes.push((tag, r, w));
                }
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
    println!("f774_nh16_paths_heavy: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}

/// The failing attempt of the observed `16^3` solve: its index in the log.
fn failing_attempt(log: &Log) -> Option<usize> {
    log.attempts
        .iter()
        .rposition(|a| a.committed == 0.9625 && a.converged == Some(false))
}
