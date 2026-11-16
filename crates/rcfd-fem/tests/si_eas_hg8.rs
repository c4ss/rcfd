//! F-740 (ADR 0660): the opt-in warm start of the local mode iteration of the stabilised incompatible-modes Hex8 (rule L of
//! ADR 0630) from the modes of the committed state, with the update S of ADR 0658 (the remedy W), on the eleven set-ups of
//! ADR 0658 and three more seeds of the `8^3` block at `0.25 h`, against the F-bar Hex8 (WB, WC, WR, SLW, FR3); the opt-in
//! trust-region (Levenberg-Marquardt) mode step measured beside it from `alpha = 0` (T); the local records at the stall of
//! seed `0xF739_0003` (NCL, LW1, LW3, TR1, TR2); the committed modes (LW0, LW2); the negative controls NCS2, NCW0, NCT,
//! NCW1. Every aggregated value is required finite before it is compared; a failed solve is a non-finite measure. The helpers
//! are copies of those of `si_eas_hg7.rs` (existing tests are not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::element::ElementState;
use rcfd_fem::material::{HyperelasticEnergy, PointContext};
use rcfd_fem::nl_incompatible::{
    condensed_force_and_tangent_warm, mode_iteration_record, mode_iteration_record_from, trust_region_record,
    IncompatibleModeUpdate, IncompatibleStabilisation, TrustRegionIterate, TRUST_REGION_ACCEPT,
};
use rcfd_fem::nonlinear::{solve, tangent, NewtonOptions, NonlinearResult};
use rcfd_fem::{BlockProperty, Constraint, Material, SolidFormulation, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{count_below, lowest_eigenpairs, SubspaceOptions};
use rcfd_linalg::skyline::SkylineOrdering;
use rcfd_linalg::CsrMatrix;
use std::collections::BTreeSet;

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
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value > limit;
        println!(
            "{name}: {value:.4e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
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
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

/// The largest of finite values; NaN if any is not finite or there is none.
fn worst<I: IntoIterator<Item = f64>>(values: I) -> f64 {
    let mut w = f64::NEG_INFINITY;
    for v in values {
        if !v.is_finite() {
            return f64::NAN;
        }
        w = w.max(v);
    }
    if w == f64::NEG_INFINITY {
        f64::NAN
    } else {
        w
    }
}

/// The smallest of finite values; NaN if any is not finite or there is none.
fn least<I: IntoIterator<Item = f64>>(values: I) -> f64 {
    let mut w = f64::INFINITY;
    for v in values {
        if !v.is_finite() {
            return f64::NAN;
        }
        w = w.min(v);
    }
    if w == f64::INFINITY {
        f64::NAN
    } else {
        w
    }
}

/// SplitMix64 from a seed.
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

fn with_formulation(mut mesh: SolidMesh, f: SolidFormulation) -> SolidMesh {
    if f != SolidFormulation::Full {
        mesh.set_block_property(0, BlockProperty::solid(f)).unwrap();
    }
    mesh
}

fn element_coords(mesh: &SolidMesh, e: usize) -> Vec<Vec3> {
    mesh.element_nodes(e).iter().map(|&n| mesh.nodes()[n]).collect()
}

fn boundary_nodes(mesh: &SolidMesh) -> BTreeSet<usize> {
    let mut s = BTreeSet::new();
    for f in BOX_FACES {
        s.extend(mesh.node_set(f).unwrap().iter().copied());
    }
    s
}

// ============================================================================ the distorted meshes (ADR 0649 D2)

/// The interior nodes (on none of the six box faces) moved by `amp h_c (2U - 1)` per component, `h_c = lengths[c] / div[c]`,
/// three draws per interior node in node order; returns the mesh and the number of moved nodes.
fn distort(mesh: &SolidMesh, lengths: Vec3, div: [usize; 3], amp: f64, seed: u64) -> (SolidMesh, usize) {
    let boundary = boundary_nodes(mesh);
    let mut rng = Rng::new(seed);
    let h: [f64; 3] = std::array::from_fn(|c| lengths[c] / div[c] as f64);
    let mut nodes = mesh.nodes().to_vec();
    let mut moved = 0;
    for (i, p) in nodes.iter_mut().enumerate() {
        if !boundary.contains(&i) {
            moved += 1;
            for (c, v) in p.iter_mut().enumerate() {
                *v += amp * h[c] * (2.0 * rng.uniform() - 1.0);
            }
        }
    }
    (rebuild(mesh, nodes), moved)
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

/// `det J` of the trilinear map of an element at a reference point.
fn det_jacobian(x: &[Vec3], xi: [f64; 3]) -> f64 {
    let mut j = [[0.0; 3]; 3];
    for (a, s) in XI.iter().enumerate() {
        let f: [f64; 3] = std::array::from_fn(|k| 1.0 + s[k] * xi[k]);
        let d = [
            s[0] * f[1] * f[2] / 8.0,
            f[0] * s[1] * f[2] / 8.0,
            f[0] * f[1] * s[2] / 8.0,
        ];
        for r in 0..3 {
            for c in 0..3 {
                j[r][c] += x[a][r] * d[c];
            }
        }
    }
    j[0][0] * (j[1][1] * j[2][2] - j[1][2] * j[2][1]) - j[0][1] * (j[1][0] * j[2][2] - j[1][2] * j[2][0])
        + j[0][2] * (j[1][0] * j[2][1] - j[1][1] * j[2][0])
}

/// M0: the smallest corner `det J` over the element centre's, over the mesh.
fn min_corner_ratio(mesh: &SolidMesh) -> f64 {
    least((0..mesh.n_elements()).flat_map(|e| {
        let x = element_coords(mesh, e);
        let d0 = det_jacobian(&x, [0.0; 3]);
        XI.iter().map(move |&c| det_jacobian(&x, c) / d0).collect::<Vec<_>>()
    }))
}

// ============================================================================ the material and the elements

const MU: f64 = 1.0e6;
const NU: f64 = 0.49;
const INC: SolidFormulation = SolidFormulation::Incompatible;
const FBAR: SolidFormulation = SolidFormulation::BBar;
const OFF: IncompatibleStabilisation = IncompatibleStabilisation::Off;
const LIN: IncompatibleStabilisation = IncompatibleStabilisation::LinearStress;

fn kappa_of(nu: f64) -> f64 {
    2.0 * MU * (1.0 + nu) / (3.0 * (1.0 - 2.0 * nu))
}

fn material(nu: f64) -> Material {
    Material::hyperelastic(HyperelasticEnergy::neo_hookean(MU, kappa_of(nu)).unwrap(), 1000.0).unwrap()
}

/// An element of the comparison (ADR 0636 D2).
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

/// The options of a solve: the driver's line search (ADR 0607 D1), the backtracking of the modes (ADR 0655 D1), its
/// sufficient-decrease test (ADR 0658 D1), the warm start (ADR 0660 D1) and the trust-region step (ADR 0660 D2).
#[derive(Clone, Copy)]
struct Opts {
    name: &'static str,
    line_search: bool,
    backtracking: bool,
    decrease: bool,
    warm: bool,
    trust: bool,
}

const DEFAULT: Opts = Opts {
    name: "default",
    line_search: false,
    backtracking: false,
    decrease: false,
    warm: false,
    trust: false,
};
/// The remedy R of ADR 0658 (the states of the local records, as ADR 0658 E1).
const R: Opts = Opts {
    name: "R (line search + backtracking)",
    line_search: true,
    backtracking: true,
    decrease: false,
    warm: false,
    trust: false,
};
/// The variant S of ADR 0658 (cold; NCS2, NCW1).
const S: Opts = Opts {
    name: "S (backtracking + decrease)",
    line_search: false,
    backtracking: true,
    decrease: true,
    warm: false,
    trust: false,
};
/// The registered remedy W (reading c3).
const W: Opts = Opts {
    name: "W (warm start + backtracking + decrease)",
    line_search: false,
    backtracking: true,
    decrease: true,
    warm: true,
    trust: false,
};
/// The variant T (reading c4): the trust-region step from alpha = 0.
const T: Opts = Opts {
    name: "T (trust region)",
    line_search: false,
    backtracking: false,
    decrease: false,
    warm: false,
    trust: true,
};

fn model_of(mesh: SolidMesh, kind: Kind, opts: Opts) -> SolidModel {
    let mut m = SolidModel::new(with_formulation(mesh, kind.formulation));
    m.set_incompatible_stabilisation(kind.rule);
    m.set_incompatible_mode_backtracking(opts.backtracking);
    m.set_incompatible_mode_decrease(opts.decrease);
    m.set_incompatible_mode_warm_start(opts.warm);
    m.set_incompatible_mode_trust_region(opts.trust);
    m.add_material("all", material(NU)).unwrap();
    m
}

fn newton(opts: Opts) -> NewtonOptions {
    NewtonOptions::new(10).threads(4).line_search(opts.line_search)
}

/// The nodes of a set whose coordinates pass a filter.
fn nodes_where(mesh: &SolidMesh, set: &str, keep: impl Fn(&Vec3) -> bool) -> Vec<usize> {
    mesh.node_set(set)
        .unwrap()
        .iter()
        .copied()
        .filter(|&n| keep(&mesh.nodes()[n]))
        .collect()
}

/// The sum of the driver's reactions over a node set's component.
fn reaction_sum(r: &NonlinearResult, nodes: &[usize], comp: usize) -> f64 {
    let re = r.result.reactions().unwrap();
    let dofs = r.result.dofs();
    nodes.iter().map(|&n| re[dofs.equation(n, comp).unwrap()]).sum()
}

/// A model with its prescribed equations, its solve and the two measures of the load case.
struct Run {
    model: SolidModel,
    prescribed: BTreeSet<usize>,
    result: Option<NonlinearResult>,
    a: f64,
    b: f64,
    error: Option<String>,
}

fn constrain(m: &mut SolidModel, bcs: &[(&str, usize, f64)]) -> BTreeSet<usize> {
    let mut prescribed = BTreeSet::new();
    for &(set, comp, value) in bcs {
        m.add_constraint(Constraint::displacement(set, comp, value)).unwrap();
        for &nd in m.mesh().node_set(set).unwrap() {
            prescribed.insert(m.dof_map().equation(nd, comp).unwrap());
        }
    }
    prescribed
}

// ============================================================================ the stability of the loaded state (SL)

/// The free block of the tangent at the converged state, as a CSR matrix.
fn free_tangent(run: &Run) -> Option<CsrMatrix> {
    let r = run.result.as_ref()?;
    let k = tangent(&run.model, r.result.displacement_values(), &r.states, Some(4)).ok()?;
    let free: Vec<usize> = (0..k.n).filter(|e| !run.prescribed.contains(e)).collect();
    let mut pos = vec![usize::MAX; k.n];
    for (i, &e) in free.iter().enumerate() {
        pos[e] = i;
    }
    let mut t = Vec::new();
    for (row, &e) in free.iter().enumerate() {
        for p in k.row_ptr[e]..k.row_ptr[e + 1] {
            let c = pos[k.col[p]];
            if c != usize::MAX {
                t.push((row, c, k.val[p]));
            }
        }
    }
    Some(CsrMatrix::from_triplets(free.len(), t))
}

/// SL: `(negative eigenvalues of K_ff, lowest eigenvalue / largest diagonal)`; NaN where a step fails.
fn stability(run: &Run) -> (f64, f64) {
    let Some(kff) = free_tangent(run) else {
        return (f64::NAN, f64::NAN);
    };
    let n = kff.n;
    let dmax = kff.diag().iter().fold(0.0f64, |w, v| w.max(v.abs()));
    let id = CsrMatrix::from_triplets(n, (0..n).map(|i| (i, i, 1.0)).collect());
    let neg = count_below(&kff, &id, 0.0, &SkylineOrdering::Rcm)
        .map(|c| c as f64)
        .unwrap_or(f64::NAN);
    let low = lowest_eigenpairs(&kff, &id, 1, &SubspaceOptions::default())
        .map(|e| e.values[0] / dmax)
        .unwrap_or(f64::NAN);
    (neg, low)
}

/// The unknowns of element `e` from the global field.
fn element_u(m: &SolidModel, u: &[f64], e: usize) -> Vec<f64> {
    let dofs = m.dof_map();
    let mut out = Vec::with_capacity(24);
    for &n in m.mesh().element_nodes(e) {
        for c in 0..3 {
            out.push(u[dofs.equation(n, c).unwrap()]);
        }
    }
    out
}

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

/// The bitwise difference count of two results' displacements and reactions; `usize::MAX` if one is missing.
fn bit_differences(a: Option<&NonlinearResult>, b: Option<&NonlinearResult>) -> usize {
    let (Some(a), Some(b)) = (a, b) else {
        return usize::MAX;
    };
    let ua = a.result.displacement_values();
    let ub = b.result.displacement_values();
    let ra = a.result.reactions().unwrap();
    let rb = b.result.reactions().unwrap();
    if ua.len() != ub.len() || ra.len() != rb.len() {
        return usize::MAX;
    }
    ua.iter().zip(ub).filter(|(x, y)| x.to_bits() != y.to_bits()).count()
        + ra.iter().zip(rb).filter(|(x, y)| x.to_bits() != y.to_bits()).count()
}

// ============================================================================ the load cases (ADR 0636 readings c1, c2)

const DELTA: f64 = 0.1;
const CANT_L: f64 = 1.0;
const CANT_H: f64 = 0.2;
const TIP_V: f64 = 0.3;

/// The block mesh on `n^3` with the punch and corner sets, moved at amplitude `amp` with `seed`.
fn block_mesh(n: usize, amp: f64, seed: u64) -> (SolidMesh, usize) {
    let mut mesh = hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap();
    let tol = 1e-9;
    let punch = nodes_where(&mesh, "zmax", |p| p[0] <= 0.5 + tol && p[1] <= 0.5 + tol);
    let corner = nodes_where(&mesh, "zmax", |p| (p[0] - 1.0).abs() < tol && (p[1] - 1.0).abs() < tol);
    mesh.add_node_set("punch", punch).unwrap();
    mesh.add_node_set("corner", corner).unwrap();
    distort(&mesh, [1.0, 1.0, 1.0], [n, n, n], amp, seed)
}

/// The cantilever mesh `nx x ny x ny` moved at amplitude `amp` with `seed`.
fn cant_mesh(nx: usize, ny: usize, amp: f64, seed: u64) -> (SolidMesh, usize) {
    let mesh = hex_box([0.0; 3], [CANT_L, CANT_H, CANT_H], [nx, ny, ny]).unwrap();
    distort(&mesh, [CANT_L, CANT_H, CANT_H], [nx, ny, ny], amp, seed)
}

/// The step record of a solve, printed.
fn steps_text(r: &NonlinearResult) -> String {
    let halved = r.steps.iter().filter(|s| s.halvings > 0).count();
    let searched = r
        .steps
        .iter()
        .flat_map(|s| s.step_lengths.iter())
        .filter(|&&l| l < 1.0)
        .count();
    let shortest = r
        .steps
        .iter()
        .flat_map(|s| s.step_lengths.iter().copied())
        .fold(f64::INFINITY, f64::min);
    format!(
        "increments {}, halved {halved}, step lengths < 1: {searched} (shortest {shortest:.4e})",
        r.steps.len()
    )
}

/// The block of ADR 0636 reading c1 on a mesh, the punch at `delta`, 10 increments; `(R, w_c)` or NaN with the error.
fn run_block(mesh: &SolidMesh, tag: &str, delta: f64, kind: Kind, opts: Opts) -> Run {
    let mut m = model_of(mesh.clone(), kind, opts);
    let prescribed = constrain(
        &mut m,
        &[
            ("xmin", 0, 0.0),
            ("xmax", 0, 0.0),
            ("ymin", 1, 0.0),
            ("ymax", 1, 0.0),
            ("zmin", 2, 0.0),
            ("punch", 2, -delta),
        ],
    );
    let (result, a, b, error) = match solve(&m, &[], &[], &newton(opts)) {
        Ok(r) => {
            let punch = m.mesh().node_set("punch").unwrap().to_vec();
            let corner = m.mesh().node_set("corner").unwrap()[0];
            let rz = -reaction_sum(&r, &punch, 2) / MU;
            let wc = r.result.displacement(corner)[2];
            println!(
                "{tag} delta {delta} {} {}: R/mu {rz:.8}, w_c {wc:+.8}, {}",
                kind.name,
                opts.name,
                steps_text(&r)
            );
            (Some(r), rz, wc, None)
        }
        Err(e) => {
            println!("{tag} delta {delta} {} {}: solve failed: {e}", kind.name, opts.name);
            (None, f64::NAN, f64::NAN, Some(e.to_string()))
        }
    };
    Run {
        model: m,
        prescribed,
        result,
        a,
        b,
        error,
    }
}

/// The cantilever of ADR 0636 reading c2 on a mesh; `(P, d)` or NaN with the error.
fn run_cantilever(mesh: &SolidMesh, tag: &str, kind: Kind, opts: Opts) -> Run {
    let mut m = model_of(mesh.clone(), kind, opts);
    let mut bcs: Vec<(&str, usize, f64)> = (0..3).map(|c| ("xmin", c, 0.0)).collect();
    bcs.push(("xmax", 1, TIP_V));
    let prescribed = constrain(&mut m, &bcs);
    let (result, a, b, error) = match solve(&m, &[], &[], &newton(opts)) {
        Ok(r) => {
            let tip = m.mesh().node_set("xmax").unwrap().to_vec();
            let p = reaction_sum(&r, &tip, 1) / MU;
            let d = -tip.iter().map(|&n| r.result.displacement(n)[0]).sum::<f64>() / tip.len() as f64;
            println!(
                "{tag} {} {}: P/mu {p:.8e}, d {d:.8e}, {}",
                kind.name,
                opts.name,
                steps_text(&r)
            );
            (Some(r), p, d, None)
        }
        Err(e) => {
            println!("{tag} {} {}: solve failed: {e}", kind.name, opts.name);
            (None, f64::NAN, f64::NAN, Some(e.to_string()))
        }
    };
    Run {
        model: m,
        prescribed,
        result,
        a,
        b,
        error,
    }
}

impl Checks {
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
}

/// The rows of W on one set-up (WB, WC, WR and SLW) against F-bar. `names` are the two measures.
fn w_rows(ch: &mut Checks, id: &str, tag: &str, tol: f64, names: [&str; 2], f: &Run, w: &Run) {
    ch.at_most(&format!("{id} {tag} W: {}", names[0]), rel(w.a, f.a), tol);
    ch.at_most(&format!("{id} {tag} W: {}", names[1]), rel(w.b, f.b), tol);
    let (neg, low) = stability(w);
    ch.at_most(&format!("SLW {tag} W: negative eigenvalues of K_ff"), neg, 0.0);
    ch.above(&format!("SLW {tag} W: lowest eigenvalue / max diagonal"), low, 0.0);
}

/// A variant measured on a set-up (T, or S where reported): outcome, the two errors, SL, and whether it passes W's limits.
fn measured(label: &str, tag: &str, tol: f64, names: [&str; 2], f: &Run, v: &Run) -> bool {
    let (ea, eb) = (rel(v.a, f.a), rel(v.b, f.b));
    let (neg, low) = stability(v);
    let pass = ea.is_finite() && ea <= tol && eb.is_finite() && eb <= tol && neg == 0.0 && low > 0.0;
    println!(
        "{label} {tag} (measured): converged {}, {} {ea:.4e}, {} {eb:.4e} (W's limit {tol}), negative {neg}, lowest / max \
         diagonal {low:.4e}: {}{}",
        v.result.is_some(),
        names[0],
        names[1],
        if pass { "passes" } else { "does not pass" },
        v.error.as_ref().map(|e| format!(" ({e})")).unwrap_or_default()
    );
    pass
}

/// One variant against another where both converge (reported).
fn against(tag: &str, label: &str, a: &Run, b: &Run) {
    if a.result.is_some() && b.result.is_some() {
        println!(
            "report {tag}: {label}: |a / a_other - 1| {:.4e}, |b / b_other - 1| {:.4e}",
            rel(a.a, b.a),
            rel(a.b, b.b)
        );
    } else {
        println!(
            "report {tag}: {label}: not both converged ({}, {})",
            a.result.is_some(),
            b.result.is_some()
        );
    }
}

/// Bit differences as a measure: NaN if a solve is missing.
fn bits(a: &Run, b: &Run) -> f64 {
    match bit_differences(a.result.as_ref(), b.result.as_ref()) {
        usize::MAX => f64::NAN,
        n => n as f64,
    }
}

/// LW0 and LW2 at W's result: the elements whose committed state is not nine values, and the elements whose warm record
/// (update S) from the committed modes at the result's displacement has another number of iterates than one (or fails).
fn committed_modes(w: &Run) -> (f64, f64) {
    let Some(r) = w.result.as_ref() else {
        return (f64::NAN, f64::NAN);
    };
    let mat = material(NU);
    let ctx = PointContext::default();
    let empty = ElementState::new(Vec::new());
    let u = r.result.displacement_values();
    let mesh = w.model.mesh();
    let (mut not_nine, mut not_one) = (0usize, 0usize);
    let mut counts = std::collections::BTreeMap::new();
    for (e, st) in r.states.iter().enumerate() {
        let v = st.values();
        if v.len() != 9 {
            not_nine += 1;
        }
        let start: Option<[f64; 9]> = (v.len() == 9).then(|| std::array::from_fn(|k| v[k]));
        let x = element_coords(mesh, e);
        let ue = element_u(&w.model, u, e);
        let n = match mode_iteration_record_from(
            &x,
            &mat,
            &ue,
            &empty,
            &ctx,
            LIN,
            IncompatibleModeUpdate::BacktrackingWithDecrease,
            start.as_ref(),
        ) {
            Ok((rec, Ok(_))) => rec.len(),
            _ => usize::MAX,
        };
        *counts.entry(n).or_insert(0usize) += 1;
        if n != 1 {
            not_one += 1;
        }
    }
    println!("LW0 iterates of the warm records (count: elements): {counts:?}");
    (not_nine as f64, not_one as f64)
}

// ============================================================================ the tests

const BLOCK_NAMES: [&str; 2] = ["e_R", "e_w"];
const CANT_NAMES: [&str; 2] = ["e_P", "e_d"];

#[test]
/// WB1 to WB4, SLW, FR3, M0, M1 on the four block meshes of ADR 0649; T measured; LW0, LW2 on DB4.
fn w_and_t_on_the_four_blocks() {
    let mut ch = Checks::new();
    let setups = [
        ("WB1", 4, 0.1, 0xF719_0004, 0.15, 27, (0.38349042, 0.07137650)),
        ("WB2", 4, 0.25, 0xF719_0004, 0.15, 27, (0.38316202, 0.07044819)),
        ("WB3", 8, 0.1, 0xF719_0008, 0.10, 343, (0.34263132, 0.06370837)),
        ("WB4", 8, 0.25, 0xF719_0008, 0.10, 343, (0.34220247, 0.06389947)),
    ];
    let mut t_passes = Vec::new();
    for (id, n, amp, seed, tol, moved, recorded) in setups {
        let tag = format!("block {n}^3 a {amp}");
        let (mesh, count) = block_mesh(n, amp, seed);
        ch.above(
            &format!("M0 {tag}: min corner det J / centre det J"),
            min_corner_ratio(&mesh),
            0.0,
        );
        ch.equals(&format!("M1 {tag}: moved nodes"), count, moved);
        let f = run_block(&mesh, &tag, DELTA, K_FBAR, DEFAULT);
        ch.at_most(
            &format!("FR3 {tag}: |R_Fbar / {} - 1|", recorded.0),
            rel(f.a, recorded.0),
            1e-6,
        );
        ch.at_most(
            &format!("FR3 {tag}: |w_Fbar / {} - 1|", recorded.1),
            rel(f.b, recorded.1),
            1e-6,
        );
        let (fneg, flow) = stability(&f);
        println!("report SL {tag} Fbar: negative {fneg}, lowest / max diagonal {flow:.4e}");
        let w = run_block(&mesh, &tag, DELTA, K_L, W);
        w_rows(&mut ch, id, &tag, tol, BLOCK_NAMES, &f, &w);
        let t = run_block(&mesh, &tag, DELTA, K_L, T);
        t_passes.push((tag.clone(), measured("T", &tag, tol, BLOCK_NAMES, &f, &t)));
        against(&tag, "T against W", &t, &w);
        if id == "WB4" {
            let (not_nine, not_one) = committed_modes(&w);
            ch.at_most(
                "LW2 DB4 W: elements whose committed state is not nine values",
                not_nine,
                0.0,
            );
            ch.at_most(
                "LW0 DB4 W: elements whose warm record from the committed modes has another count than one iterate",
                not_one,
                0.0,
            );
        }
    }
    println!("T on the blocks: {t_passes:?}");
    ch.finish();
}

#[test]
/// WC1 to WC4, SLW, FR3, M0, M1 on the four cantilever meshes of ADR 0649; T measured; NCW0, NCT, NCW1.
fn w_and_t_on_the_four_cantilevers() {
    let mut ch = Checks::new();
    let setups = [
        ("WC1", 10, 2, 0.1, 0xF719_1010, 0.15, 9, (6.17271412e-4, 5.24022571e-2)),
        ("WC2", 10, 2, 0.25, 0xF719_1010, 0.15, 9, (6.19924741e-4, 5.23693020e-2)),
        (
            "WC3",
            20,
            4,
            0.1,
            0xF719_1020,
            0.10,
            171,
            (5.72841645e-4, 5.23430902e-2),
        ),
        (
            "WC4",
            20,
            4,
            0.25,
            0xF719_1020,
            0.10,
            171,
            (5.75783694e-4, 5.23261081e-2),
        ),
    ];
    let mut t_passes = Vec::new();
    for (id, nx, ny, amp, seed, tol, moved, recorded) in setups {
        let tag = format!("cantilever {nx}x{ny}x{ny} a {amp}");
        let (mesh, count) = cant_mesh(nx, ny, amp, seed);
        ch.above(
            &format!("M0 {tag}: min corner det J / centre det J"),
            min_corner_ratio(&mesh),
            0.0,
        );
        ch.equals(&format!("M1 {tag}: moved nodes"), count, moved);
        let f = run_cantilever(&mesh, &tag, K_FBAR, DEFAULT);
        ch.at_most(
            &format!("FR3 {tag}: |P_Fbar / {:e} - 1|", recorded.0),
            rel(f.a, recorded.0),
            1e-6,
        );
        ch.at_most(
            &format!("FR3 {tag}: |d_Fbar / {:e} - 1|", recorded.1),
            rel(f.b, recorded.1),
            1e-6,
        );
        let (fneg, flow) = stability(&f);
        println!("report SL {tag} Fbar: negative {fneg}, lowest / max diagonal {flow:.4e}");
        let w = run_cantilever(&mesh, &tag, K_L, W);
        w_rows(&mut ch, id, &tag, tol, CANT_NAMES, &f, &w);
        let t = run_cantilever(&mesh, &tag, K_L, T);
        t_passes.push((tag.clone(), measured("T", &tag, tol, CANT_NAMES, &f, &t)));
        against(&tag, "T against W", &t, &w);
        if id == "WC2" {
            let d = run_cantilever(&mesh, &tag, K_L, DEFAULT);
            let s = run_cantilever(&mesh, &tag, K_L, S);
            let with_values = d
                .result
                .as_ref()
                .map(|r| r.states.iter().filter(|st| !st.values().is_empty()).count() as f64)
                .unwrap_or(f64::NAN);
            ch.at_most(
                "NCW0 cantilever 10x2x2 a 0.25 default: elements whose committed state has a value",
                with_values,
                0.0,
            );
            ch.above(
                "NCT cantilever 10x2x2 a 0.25: values differing in bits, T against the default",
                bits(&t, &d),
                0.0,
            );
            ch.above(
                "NCW1 cantilever 10x2x2 a 0.25: values differing in bits, W against S",
                bits(&w, &s),
                0.0,
            );
            against(&tag, "W against S", &w, &s);
        }
    }
    println!("T on the cantilevers: {t_passes:?}");
    ch.finish();
}

/// The first three seeds from `first` on whose `8^3` mesh at `0.25 h` passes M0 (ADR 0658 reading c3, ADR 0660 reading c1).
fn seeds_from(first: u64) -> Vec<(u64, SolidMesh, usize, f64)> {
    let mut out = Vec::new();
    let mut seed = first;
    while out.len() < 3 {
        let (mesh, count) = block_mesh(8, 0.25, seed);
        let m0 = min_corner_ratio(&mesh);
        if m0 > 0.0 {
            out.push((seed, mesh, count, m0));
        } else {
            println!("seed {seed:#X}: M0 = {m0:e}, not a valid mesh; replaced by the next seed (reading c1)");
        }
        seed += 1;
    }
    out
}

#[test]
/// WR1 to WR3, SLW, FR3, M0, M1 on the `8^3` block at `0.25 h` with the seeds of ADR 0658; T measured; NCS2.
fn w_and_t_on_the_seeds_of_adr_0658() {
    let mut ch = Checks::new();
    let recorded = [
        (0.34387169, 0.06336271),
        (0.34915749, 0.06430598),
        (0.34360196, 0.06344073),
    ];
    let mut t_passes = Vec::new();
    for (i, (seed, mesh, count, m0)) in seeds_from(0xF739_0001).into_iter().enumerate() {
        let id = format!("WR{}", i + 1);
        let tag = format!("block 8^3 a 0.25 seed {seed:#X}");
        ch.above(&format!("M0 {tag}: min corner det J / centre det J"), m0, 0.0);
        ch.equals(&format!("M1 {tag}: moved nodes"), count, 343);
        let f = run_block(&mesh, &tag, DELTA, K_FBAR, DEFAULT);
        ch.at_most(
            &format!("FR3 {tag}: |R_Fbar / {} - 1|", recorded[i].0),
            rel(f.a, recorded[i].0),
            1e-6,
        );
        ch.at_most(
            &format!("FR3 {tag}: |w_Fbar / {} - 1|", recorded[i].1),
            rel(f.b, recorded[i].1),
            1e-6,
        );
        let (fneg, flow) = stability(&f);
        println!("report SL {tag} Fbar: negative {fneg}, lowest / max diagonal {flow:.4e}");
        let w = run_block(&mesh, &tag, DELTA, K_L, W);
        w_rows(&mut ch, &id, &tag, 0.10, BLOCK_NAMES, &f, &w);
        let t = run_block(&mesh, &tag, DELTA, K_L, T);
        t_passes.push((tag.clone(), measured("T", &tag, 0.10, BLOCK_NAMES, &f, &t)));
        against(&tag, "T against W", &t, &w);
        if i > 0 {
            let s = run_block(&mesh, &tag, DELTA, K_L, S);
            ch.equals(
                &format!("NCS2 {tag}: S (cold) fails (1 = yes)"),
                s.result.is_none() as usize,
                1,
            );
        }
    }
    println!("T on the seeds of ADR 0658: {t_passes:?}");
    ch.finish();
}

#[test]
/// WR4 to WR6, SLW, M0, M1 on the `8^3` block at `0.25 h` with three more seeds (from `0xF740_0001`); T measured; S
/// reported.
fn w_and_t_on_three_more_seeds() {
    let mut ch = Checks::new();
    let (mut t_passes, mut s_passes) = (Vec::new(), Vec::new());
    for (i, (seed, mesh, count, m0)) in seeds_from(0xF740_0001).into_iter().enumerate() {
        let id = format!("WR{}", i + 4);
        let tag = format!("block 8^3 a 0.25 seed {seed:#X}");
        ch.above(&format!("M0 {tag}: min corner det J / centre det J"), m0, 0.0);
        ch.equals(&format!("M1 {tag}: moved nodes"), count, 343);
        let f = run_block(&mesh, &tag, DELTA, K_FBAR, DEFAULT);
        let (fneg, flow) = stability(&f);
        println!("report SL {tag} Fbar: negative {fneg}, lowest / max diagonal {flow:.4e}");
        let w = run_block(&mesh, &tag, DELTA, K_L, W);
        w_rows(&mut ch, &id, &tag, 0.10, BLOCK_NAMES, &f, &w);
        let t = run_block(&mesh, &tag, DELTA, K_L, T);
        t_passes.push((tag.clone(), measured("T", &tag, 0.10, BLOCK_NAMES, &f, &t)));
        against(&tag, "T against W", &t, &w);
        let s = run_block(&mesh, &tag, DELTA, K_L, S);
        s_passes.push((tag.clone(), measured("S", &tag, 0.10, BLOCK_NAMES, &f, &s)));
        against(&tag, "W against S", &w, &s);
    }
    println!("T on the three more seeds: {t_passes:?}");
    println!("S on the three more seeds: {s_passes:?}");
    ch.finish();
}

fn print_trust(tag: &str, rec: &[TrustRegionIterate]) {
    let norms: Vec<String> = rec.iter().map(|i| format!("{:.3e}", i.residual_norm)).collect();
    let rej: Vec<usize> = rec.iter().map(|i| i.rejections).collect();
    let lam: Vec<String> = rec.iter().map(|i| format!("{:.1e}", i.damping)).collect();
    let rho: Vec<String> = rec.iter().map(|i| format!("{:.3}", i.gain)).collect();
    println!(
        "{tag}: {} iterates, |r|_2 {norms:?}, rejections {rej:?}, lambda {lam:?}, rho {rho:?}",
        rec.len()
    );
}

/// TR1's measures over the accepted steps of a record: the largest `|r_{j+1}|_2 / |r_j|_2` and the smallest `rho`.
fn trust_measures(rec: &[TrustRegionIterate]) -> (f64, f64) {
    (
        worst(rec.windows(2).map(|w| w[1].residual_norm / w[0].residual_norm)),
        least(rec.iter().skip(1).map(|i| i.gain)),
    )
}

fn outcome<T>(o: &rcfd_fem::Result<T>) -> String {
    o.as_ref()
        .map(|_| "converged".to_string())
        .unwrap_or_else(|e| e.to_string())
}

fn inf_rel(a: &[f64], b: &[f64]) -> f64 {
    let amax = b.iter().fold(0.0f64, |w, v| w.max(v.abs()));
    a.iter().zip(b).fold(0.0f64, |w, (p, q)| w.max((p - q).abs())) / amax
}

#[test]
/// NCL, LW1, LW3, TR1, TR2 at the trial state `u_T` and the state `u_b` of element 476 of seed `0xF739_0003` (ADR 0658
/// A1.2 E1, E3).
fn local_records_at_the_stall_of_seed_3() {
    let mut ch = Checks::new();
    let mat = material(NU);
    let ctx = PointContext::default();
    let st = ElementState::new(Vec::new());
    let (seed, e, pa, pb, pt) = (0xF739_0003u64, 476usize, 0.0850, 0.0855, 0.0858);
    let tag = format!("seed {seed:#X} element {e}");
    let (mesh, _) = block_mesh(8, 0.25, seed);
    let x = element_coords(&mesh, e);
    let ra = run_block(&mesh, &format!("E1 {tag}"), pa, K_L, R);
    let rb = run_block(&mesh, &format!("E1 {tag}"), pb, K_L, R);
    let ua = ra
        .result
        .as_ref()
        .map(|r| element_u(&ra.model, r.result.displacement_values(), e));
    let ub = rb
        .result
        .as_ref()
        .map(|r| element_u(&rb.model, r.result.displacement_values(), e));
    let ut: Option<Vec<f64>> = match (&ua, &ub) {
        (Some(a), Some(b)) => Some(
            b.iter()
                .zip(a)
                .map(|(q, p)| q + (pt - pb) / (pb - pa) * (q - p))
                .collect(),
        ),
        _ => {
            println!("{tag}: a solve of R failed; no trial state");
            None
        }
    };
    let dec = IncompatibleModeUpdate::BacktrackingWithDecrease;
    // alpha_b: the converged modes of the cold backtracked record at u_b (ADR 0658 E3).
    let alpha_b = ub.as_ref().and_then(|b| {
        match mode_iteration_record(&x, &mat, b, &st, &ctx, LIN, IncompatibleModeUpdate::Backtracking) {
            Ok((_, Ok(a))) => Some(a),
            _ => None,
        }
    });
    println!("{tag}: alpha_b {alpha_b:?}");
    // NCL: the cold record with update S at u_T fails.
    let cold = ut
        .as_ref()
        .and_then(|u| mode_iteration_record_from(&x, &mat, u, &st, &ctx, LIN, dec, None).ok());
    if let Some((rec, o)) = cold.as_ref() {
        let norms: Vec<String> = rec.iter().map(|i| format!("{:.3e}", i.residual_norm)).collect();
        println!(
            "NCL cold record S at u_T: {} iterates, |r|_2 {norms:?}: {}",
            rec.len(),
            outcome(o)
        );
    }
    ch.equals(
        "NCL cold record S at u_T fails (1 = yes)",
        cold.as_ref().map(|(_, o)| o.is_err() as usize).unwrap_or(0),
        1,
    );
    // LW1: the warm record with update S from alpha_b at u_T converges within 25 iterates.
    let warm = match (ut.as_ref(), alpha_b.as_ref()) {
        (Some(u), Some(a)) => mode_iteration_record_from(&x, &mat, u, &st, &ctx, LIN, dec, Some(a)).ok(),
        _ => None,
    };
    if let Some((rec, o)) = warm.as_ref() {
        let norms: Vec<String> = rec.iter().map(|i| format!("{:.3e}", i.residual_norm)).collect();
        let halvings: Vec<usize> = rec.iter().map(|i| i.halvings).collect();
        println!(
            "LW1 warm record S at u_T from alpha_b: {} iterates, |r|_2 {norms:?}, halvings {halvings:?}: {}",
            rec.len(),
            outcome(o)
        );
        if let (Ok(m), Some(a)) = (o, alpha_b.as_ref()) {
            println!(
                "report LW1: |alpha_T - alpha_b|_inf / |alpha_b|_inf {:.4e}",
                inf_rel(m, a)
            );
        }
    }
    let warm_modes = warm.as_ref().and_then(|(_, o)| o.as_ref().ok().copied());
    ch.equals(
        "LW1 warm record S at u_T from alpha_b converged (1 = yes)",
        warm_modes.is_some() as usize,
        1,
    );
    // LW3: the driver's warm evaluation from the state alpha_b at u_T.
    let lw3 = match (ut.as_ref(), alpha_b.as_ref()) {
        (Some(u), Some(a)) => {
            condensed_force_and_tangent_warm(&x, &mat, u, &ElementState::new(a.to_vec()), &ctx, LIN, dec)
        }
        _ => Err(rcfd_fem::FemError::InvalidValue {
            key: "test",
            reason: "no trial state".into(),
        }),
    };
    if let Err(er) = lw3.as_ref() {
        println!("LW3 warm evaluation at u_T: {er}");
    }
    ch.equals(
        "LW3 warm evaluation at u_T from the state alpha_b succeeds (1 = yes)",
        lw3.is_ok() as usize,
        1,
    );
    let stored = match (lw3.as_ref(), warm_modes.as_ref()) {
        (Ok((_, _, _, s)), Some(m)) if s.len() == 9 => inf_rel(s, m),
        _ => f64::NAN,
    };
    ch.at_most(
        "LW3 the trial state's nine values against the LW1 modes, |.|_inf relative",
        stored,
        1e-12,
    );
    // TR1, TR2: the trust-region records from alpha = 0 at u_T and at u_b.
    let tr_t = ut
        .as_ref()
        .and_then(|u| trust_region_record(&x, &mat, u, &st, &ctx, LIN, None).ok());
    let tr_b = ub
        .as_ref()
        .and_then(|b| trust_region_record(&x, &mat, b, &st, &ctx, LIN, None).ok());
    for (label, rec) in [("u_T", &tr_t), ("u_b", &tr_b)] {
        match rec.as_ref() {
            Some((r, o)) => {
                print_trust(&format!("T record at {label}"), r);
                println!("T record at {label}: {}", outcome(o));
                let (rise, rho) = trust_measures(r);
                ch.below(
                    &format!("TR1 T record at {label}: largest |r_(j+1)|_2 / |r_j|_2 over the accepted steps"),
                    rise,
                    1.0,
                );
                ch.above(
                    &format!("TR1 T record at {label}: smallest rho of an accepted step"),
                    rho,
                    TRUST_REGION_ACCEPT,
                );
            }
            None => {
                ch.below(&format!("TR1 T record at {label}: no record"), f64::NAN, 1.0);
            }
        }
    }
    let tr2 = match (tr_b.as_ref(), alpha_b.as_ref()) {
        (Some((_, Ok(m))), Some(a)) => inf_rel(m, a),
        _ => f64::NAN,
    };
    ch.at_most(
        "TR2 T record at u_b converged, its modes against alpha_b, |.|_inf relative",
        tr2,
        1e-7,
    );
    ch.finish();
}
