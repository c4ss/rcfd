//! F-739 (ADR 0658): the driver's line search with the opt-in backtracking of the incompatible-mode update of ADR 0655 (the
//! remedy R) for the stabilised incompatible-modes Hex8 (rule L of ADR 0630) on the eight distorted meshes of ADR 0649 and on
//! the `8^3` block at `0.25 h` with three further seeds, against the F-bar Hex8 (RB, RC, RS, SLR, FR3); the opt-in
//! sufficient-decrease test of the mode update of ADR 0658 D1 measured beside it without the line search (S); its record at
//! the trial state of element 475 (DT0, DT1a, DT1b); the negative controls NC1, NCB, NCS, NC4. Every aggregated value is
//! required finite before it is compared; a failed solve is a non-finite measure. The helpers are copies of those of
//! `si_eas_hg5.rs` and `si_eas_hg6.rs` (existing tests are not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::element::ElementState;
use rcfd_fem::material::{HyperelasticEnergy, PointContext};
use rcfd_fem::nl_incompatible::{
    mode_iteration_record, IncompatibleModeUpdate, IncompatibleStabilisation, ModeIterate, DECREASE_FRACTION,
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

/// The options of a solve: the driver's line search (ADR 0607 D1), the backtracking of the modes (ADR 0655 D1) and its
/// sufficient-decrease test (ADR 0658 D1).
#[derive(Clone, Copy)]
struct Opts {
    name: &'static str,
    line_search: bool,
    backtracking: bool,
    decrease: bool,
}

const DEFAULT: Opts = Opts {
    name: "default",
    line_search: false,
    backtracking: false,
    decrease: false,
};
/// The backtracking alone (ADR 0655 c4; NCB, and the states of DT).
const BT: Opts = Opts {
    name: "backtracking",
    line_search: false,
    backtracking: true,
    decrease: false,
};
/// The remedy R (reading c1).
const R: Opts = Opts {
    name: "R (line search + backtracking)",
    line_search: true,
    backtracking: true,
    decrease: false,
};
/// The variant S (reading c6).
const S: Opts = Opts {
    name: "S (backtracking + decrease)",
    line_search: false,
    backtracking: true,
    decrease: true,
};
/// The decrease flag without the backtracking (NC4).
const DEC_ONLY: Opts = Opts {
    name: "decrease flag alone",
    line_search: false,
    backtracking: false,
    decrease: true,
};

fn model_of(mesh: SolidMesh, kind: Kind, opts: Opts) -> SolidModel {
    let mut m = SolidModel::new(with_formulation(mesh, kind.formulation));
    m.set_incompatible_stabilisation(kind.rule);
    m.set_incompatible_mode_backtracking(opts.backtracking);
    m.set_incompatible_mode_decrease(opts.decrease);
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

/// The load factor at the start of the failing increment named in a driver error (`load factor X to Y`), NaN if none.
fn failing_load_factor(error: &str) -> f64 {
    error
        .split("(load factor ")
        .nth(1)
        .and_then(|t| t.split(" to ").next())
        .and_then(|t| t.trim().parse::<f64>().ok())
        .unwrap_or(f64::NAN)
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

/// The rows of R on one set-up (RB, RC, RS and SLR) against F-bar, and S measured beside it against the same limits
/// (printed, no limit). `names` are the two measures (`e_R`, `e_w` or `e_P`, `e_d`).
fn r_rows(ch: &mut Checks, id: &str, tag: &str, tol: f64, names: [&str; 2], f: &Run, r: &Run) {
    ch.at_most(&format!("{id} {tag} R: {}", names[0]), rel(r.a, f.a), tol);
    ch.at_most(&format!("{id} {tag} R: {}", names[1]), rel(r.b, f.b), tol);
    let (neg, low) = stability(r);
    ch.at_most(&format!("SLR {tag} R: negative eigenvalues of K_ff"), neg, 0.0);
    ch.above(&format!("SLR {tag} R: lowest eigenvalue / max diagonal"), low, 0.0);
}

/// S measured on a set-up: outcome, the two errors, SL, and whether it passes R's limits. Returns whether it passes.
fn s_measured(tag: &str, tol: f64, names: [&str; 2], f: &Run, s: &Run) -> bool {
    let (ea, eb) = (rel(s.a, f.a), rel(s.b, f.b));
    let (neg, low) = stability(s);
    let pass = ea.is_finite() && ea <= tol && eb.is_finite() && eb <= tol && neg == 0.0 && low > 0.0;
    println!(
        "S {tag} (measured): converged {}, {} {ea:.4e}, {} {eb:.4e} (R's limit {tol}), negative {neg}, lowest / max diagonal \
         {low:.4e}: {}{}",
        s.result.is_some(),
        names[0],
        names[1],
        if pass { "passes" } else { "does not pass" },
        s.error.as_ref().map(|e| format!(" ({e})")).unwrap_or_default()
    );
    pass
}

/// R against the default rule L where both converge (reported).
fn r_against_default(tag: &str, r: &Run, d: &Run) {
    if d.result.is_some() && r.result.is_some() {
        println!(
            "report {tag}: R against the default rule L: |a_R / a_default - 1| {:.4e}, |b_R / b_default - 1| {:.4e}",
            rel(r.a, d.a),
            rel(r.b, d.b)
        );
    } else {
        println!(
            "report {tag}: the default rule L {}",
            d.error
                .as_deref()
                .map(|e| format!("failed: {e}"))
                .unwrap_or("converged".into())
        );
    }
}

// ============================================================================ the tests

const BLOCK_NAMES: [&str; 2] = ["e_R", "e_w"];
const CANT_NAMES: [&str; 2] = ["e_P", "e_d"];

#[test]
/// RB1 to RB4, SLR, FR3, M0, M1 on the four block meshes of ADR 0649; S measured; NC1, NCB, NCS on the `8^3` block at
/// `0.25 h`.
fn r_and_s_on_the_four_blocks() {
    let mut ch = Checks::new();
    let setups = [
        ("RB1", 4, 0.1, 0xF719_0004, 0.15, 27, (0.38349042, 0.07137650)),
        ("RB2", 4, 0.25, 0xF719_0004, 0.15, 27, (0.38316202, 0.07044819)),
        ("RB3", 8, 0.1, 0xF719_0008, 0.10, 343, (0.34263132, 0.06370837)),
        ("RB4", 8, 0.25, 0xF719_0008, 0.10, 343, (0.34220247, 0.06389947)),
    ];
    let mut s_passes = Vec::new();
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
        let r = run_block(&mesh, &tag, DELTA, K_L, R);
        r_rows(&mut ch, id, &tag, tol, BLOCK_NAMES, &f, &r);
        let s = run_block(&mesh, &tag, DELTA, K_L, S);
        s_passes.push((tag.clone(), s_measured(&tag, tol, BLOCK_NAMES, &f, &s)));
        let d = run_block(&mesh, &tag, DELTA, K_L, DEFAULT);
        r_against_default(&tag, &r, &d);
        if id == "RB4" {
            // NC1: the default fails as in ADR 0649.
            let err = d.error.clone().unwrap_or_default();
            let lf = failing_load_factor(&err);
            ch.equals(
                "NC1 default on DB4: solve failed (1 = yes)",
                d.result.is_none() as usize,
                1,
            );
            ch.equals(
                "NC1 default on DB4: error names element 476 (1 = yes)",
                err.starts_with("element 476:") as usize,
                1,
            );
            ch.at_least("NC1 default on DB4: failing load factor >= 0.67", lf, 0.67);
            ch.at_most("NC1 default on DB4: failing load factor <= 0.68", lf, 0.68);
            // NCB: the backtracking alone fails as in ADR 0655.
            let b = run_block(&mesh, &tag, DELTA, K_L, BT);
            ch.equals(
                "NCB backtracking alone on DB4: solve failed (1 = yes)",
                b.result.is_none() as usize,
                1,
            );
            println!(
                "NCB failing load factor {:.6}",
                failing_load_factor(b.error.as_deref().unwrap_or(""))
            );
            // NCS: S differs from the backtracking alone.
            let differs = s.result.is_some() || s.error != b.error;
            ch.equals(
                "NCS S on DB4 differs from the backtracking alone (1 = yes)",
                differs as usize,
                1,
            );
        }
    }
    println!("S on the blocks: {s_passes:?}");
    ch.finish();
}

#[test]
/// RC1 to RC4, SLR, FR3, M0, M1 on the four cantilever meshes of ADR 0649; S measured; NC4.
fn r_and_s_on_the_four_cantilevers() {
    let mut ch = Checks::new();
    let setups = [
        ("RC1", 10, 2, 0.1, 0xF719_1010, 0.15, 9, (6.17271412e-4, 5.24022571e-2)),
        ("RC2", 10, 2, 0.25, 0xF719_1010, 0.15, 9, (6.19924741e-4, 5.23693020e-2)),
        (
            "RC3",
            20,
            4,
            0.1,
            0xF719_1020,
            0.10,
            171,
            (5.72841645e-4, 5.23430902e-2),
        ),
        (
            "RC4",
            20,
            4,
            0.25,
            0xF719_1020,
            0.10,
            171,
            (5.75783694e-4, 5.23261081e-2),
        ),
    ];
    let mut s_passes = Vec::new();
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
        let r = run_cantilever(&mesh, &tag, K_L, R);
        r_rows(&mut ch, id, &tag, tol, CANT_NAMES, &f, &r);
        let s = run_cantilever(&mesh, &tag, K_L, S);
        s_passes.push((tag.clone(), s_measured(&tag, tol, CANT_NAMES, &f, &s)));
        let d = run_cantilever(&mesh, &tag, K_L, DEFAULT);
        r_against_default(&tag, &r, &d);
        if id == "RC2" {
            let dec = run_cantilever(&mesh, &tag, K_L, DEC_ONLY);
            ch.equals(
                "NC4 cantilever 10x2x2 a 0.25 rule L: values differing in bits, the decrease flag alone against default",
                bit_differences(d.result.as_ref(), dec.result.as_ref()),
                0,
            );
            println!(
                "report cantilever 10x2x2 a 0.25: values differing in bits, S against default: {}",
                bit_differences(d.result.as_ref(), s.result.as_ref())
            );
        }
    }
    println!("S on the cantilevers: {s_passes:?}");
    ch.finish();
}

/// The three further seeds of reading c3: from `0xF739_0001` on, the first three whose `8^3` mesh at `0.25 h` passes M0.
fn further_seeds() -> Vec<(u64, SolidMesh, usize, f64)> {
    let mut out = Vec::new();
    let mut seed = 0xF739_0001u64;
    while out.len() < 3 {
        let (mesh, count) = block_mesh(8, 0.25, seed);
        let m0 = min_corner_ratio(&mesh);
        if m0 > 0.0 {
            out.push((seed, mesh, count, m0));
        } else {
            println!("seed {seed:#X}: M0 = {m0:e}, not a valid mesh; replaced by the next seed (reading c3)");
        }
        seed += 1;
    }
    out
}

#[test]
/// RS1 to RS3, SLR, M0, M1 on the `8^3` block at `0.25 h` with three further seeds; S measured; the default reported.
fn r_and_s_on_three_further_seeds() {
    let mut ch = Checks::new();
    let mut s_passes = Vec::new();
    for (i, (seed, mesh, count, m0)) in further_seeds().into_iter().enumerate() {
        let id = format!("RS{}", i + 1);
        let tag = format!("block 8^3 a 0.25 seed {seed:#X}");
        ch.above(&format!("M0 {tag}: min corner det J / centre det J"), m0, 0.0);
        ch.equals(&format!("M1 {tag}: moved nodes"), count, 343);
        let f = run_block(&mesh, &tag, DELTA, K_FBAR, DEFAULT);
        let (fneg, flow) = stability(&f);
        println!("report SL {tag} Fbar: negative {fneg}, lowest / max diagonal {flow:.4e}");
        let r = run_block(&mesh, &tag, DELTA, K_L, R);
        r_rows(&mut ch, &id, &tag, 0.10, BLOCK_NAMES, &f, &r);
        let s = run_block(&mesh, &tag, DELTA, K_L, S);
        s_passes.push((tag.clone(), s_measured(&tag, 0.10, BLOCK_NAMES, &f, &s)));
        let d = run_block(&mesh, &tag, DELTA, K_L, DEFAULT);
        r_against_default(&tag, &r, &d);
    }
    println!("S on the seeds: {s_passes:?}");
    ch.finish();
}

/// The largest ratio of consecutive `|r|_2` over the accepted iterates of a record (NaN if fewer than two).
fn largest_rise(rec: &[ModeIterate]) -> f64 {
    worst(rec.windows(2).map(|w| w[1].residual_norm / w[0].residual_norm))
}

/// DT1a's measure: the largest `|r_{j+1}|_2^2 / ((1 - 2 c t_j) |r_j|_2^2)`, `t_j = 2^-m` of the step (NaN if fewer than two).
fn largest_decrease_ratio(rec: &[ModeIterate]) -> f64 {
    worst(rec.windows(2).map(|w| {
        let t = 0.5f64.powi(w[1].halvings as i32);
        (w[1].residual_norm * w[1].residual_norm)
            / ((1.0 - 2.0 * DECREASE_FRACTION * t) * w[0].residual_norm * w[0].residual_norm)
    }))
}

fn print_record(tag: &str, rec: &[ModeIterate]) {
    let norms: Vec<String> = rec.iter().map(|i| format!("{:.3e}", i.residual_norm)).collect();
    let halvings: Vec<usize> = rec.iter().map(|i| i.halvings).collect();
    println!("{tag}: {} iterates, |r|_2 {norms:?}, halvings {halvings:?}", rec.len());
}

#[test]
/// DT0, DT1a, DT1b at the trial `u_T` of element 475 (ADR 0655 E1; ADR 0658 arithmetic 6).
fn dt_decrease_record_at_the_trial_of_element_475() {
    let mut ch = Checks::new();
    let (mesh, _) = block_mesh(8, 0.25, 0xF719_0008);
    let e475 = 475;
    let x = element_coords(&mesh, e475);
    let r88 = run_block(&mesh, "DT", 0.088, K_L, BT);
    let r89 = run_block(&mesh, "DT", 0.089, K_L, BT);
    let ut = match (r89.result.as_ref(), r88.result.as_ref()) {
        (Some(a), Some(b)) => {
            let ua = element_u(&r89.model, a.result.displacement_values(), e475);
            let ub = element_u(&r88.model, b.result.displacement_values(), e475);
            Some(ua.iter().zip(&ub).map(|(p, q)| p + 0.2 * (p - q)).collect::<Vec<f64>>())
        }
        _ => {
            println!("DT: a backtracked solve to 0.088 or 0.089 failed; no trial");
            None
        }
    };
    let mat = material(NU);
    let ctx = PointContext::default();
    let st = ElementState::new(Vec::new());
    let record = |update| {
        ut.as_ref()
            .and_then(|u| mode_iteration_record(&x, &mat, u, &st, &ctx, LIN, update).ok())
    };
    let plain = record(IncompatibleModeUpdate::Backtracking);
    let dec = record(IncompatibleModeUpdate::BacktrackingWithDecrease);
    let rise = plain.as_ref().map(|(r, _)| largest_rise(r)).unwrap_or(f64::NAN);
    let ratio = dec.as_ref().map(|(r, _)| largest_decrease_ratio(r)).unwrap_or(f64::NAN);
    let converged = dec.as_ref().map(|(_, o)| o.is_ok()).unwrap_or(false);
    if let Some((r, o)) = plain.as_ref() {
        print_record("DT domain-only record", r);
        println!(
            "DT domain-only outcome: {}",
            o.as_ref()
                .map(|_| "converged".to_string())
                .unwrap_or_else(|e| e.to_string())
        );
    }
    if let Some((r, o)) = dec.as_ref() {
        print_record("DT decrease record", r);
        println!(
            "DT decrease outcome: {}",
            o.as_ref()
                .map(|_| "converged".to_string())
                .unwrap_or_else(|e| e.to_string())
        );
    }
    ch.above("DT0 domain-only record: largest |r_{j+1}|_2 / |r_j|_2", rise, 1.0);
    ch.at_most(
        "DT1a decrease record: largest |r_{j+1}|_2^2 / ((1 - 2 c t_j) |r_j|_2^2)",
        ratio,
        1.0,
    );
    ch.equals("DT1b decrease record converged (1 = yes)", converged as usize, 1);
    if let (Some((_, Ok(a))), Some((_, Ok(b)))) = (plain.as_ref(), dec.as_ref()) {
        let amax = a.iter().fold(0.0f64, |w, v| w.max(v.abs()));
        let d = a.iter().zip(b).fold(0.0f64, |w, (p, q)| w.max((p - q).abs()));
        println!(
            "report DT: max |alpha_domain - alpha_decrease| / |alpha_domain|_inf {:.4e}",
            d / amax
        );
    }
    ch.finish();
}
