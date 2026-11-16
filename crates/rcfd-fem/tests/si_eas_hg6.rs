//! F-732 (ADR 0655): the inverted iterate of the stabilised incompatible-modes Hex8 (rule L of ADR 0630) at the punch edge of
//! the distorted `8^3` block of ADR 0649 DB4: the cause located by the compatible field against the mode field of element 476
//! along the path (CA), the driver's line search (LS) and the opt-in backtracking of the mode update of ADR 0655 D1 (BT, with
//! DB4b, SLb, T-FD, BA), the negative controls NC1 to NC3, and the plain element on the `4^3` block reported (c6). Every
//! aggregated value is required finite before it is compared; a failed solve is a non-finite measure. The helpers are copies
//! of those of `si_eas_hg5.rs` (existing tests are not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::element::ElementState;
use rcfd_fem::material::{HyperelasticEnergy, PointContext};
use rcfd_fem::nl_incompatible::IncompatibleStabilisation;
use rcfd_fem::nl_incompatible::{
    condensed_force_and_tangent, condensed_force_and_tangent_backtracked, enhanced_force_and_tangent_with,
    modes_and_strains_with, point_gradients,
};
use rcfd_fem::nonlinear::{solve, tangent, NewtonOptions, NonlinearResult};
use rcfd_fem::{BlockProperty, Constraint, Material, SolidFormulation, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{count_below, lowest_eigenpairs, symmetric_eigen, SubspaceOptions};
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
const K_INC: Kind = Kind {
    name: "Inc",
    formulation: INC,
    rule: OFF,
};

fn model_of(mesh: SolidMesh, kind: Kind) -> SolidModel {
    let mut m = SolidModel::new(with_formulation(mesh, kind.formulation));
    m.set_incompatible_stabilisation(kind.rule);
    m.add_material("all", material(NU)).unwrap();
    m
}

fn newton() -> NewtonOptions {
    NewtonOptions::new(10).threads(4)
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

/// SL: `(negative eigenvalues of K_ff, lowest eigenvalue / largest diagonal (sparse), the same by the dense Jacobi solver
/// when asked)`; NaN entries where a step fails.
fn stability(run: &Run, dense: bool) -> (f64, f64, f64) {
    let Some(kff) = free_tangent(run) else {
        return (f64::NAN, f64::NAN, f64::NAN);
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
    let low_dense = if dense {
        let mut a = vec![0.0; n * n];
        for r in 0..n {
            for p in kff.row_ptr[r]..kff.row_ptr[r + 1] {
                a[r * n + kff.col[p]] = kff.val[p];
            }
        }
        symmetric_eigen(&a, n)
            .map(|e| least(e.values.iter().copied()) / dmax)
            .unwrap_or(f64::NAN)
    } else {
        f64::NAN
    };
    (neg, low, low_dense)
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

// ============================================================================ DB: the punch block (ADR 0636 reading c1)

const DELTA: f64 = 0.1;

fn block_seed(n: usize) -> u64 {
    match n {
        4 => 0xF719_0004,
        8 => 0xF719_0008,
        _ => unreachable!(),
    }
}

/// The block mesh on `n^3` with the punch and corner sets, moved at amplitude `amp` (0: the regular mesh of ADR 0636).
fn block_mesh(n: usize, amp: f64) -> (SolidMesh, usize) {
    let mut mesh = hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap();
    let tol = 1e-9;
    let punch = nodes_where(&mesh, "zmax", |p| p[0] <= 0.5 + tol && p[1] <= 0.5 + tol);
    let corner = nodes_where(&mesh, "zmax", |p| (p[0] - 1.0).abs() < tol && (p[1] - 1.0).abs() < tol);
    mesh.add_node_set("punch", punch).unwrap();
    mesh.add_node_set("corner", corner).unwrap();
    if amp == 0.0 {
        return (mesh, 0);
    }
    distort(&mesh, [1.0, 1.0, 1.0], [n, n, n], amp, block_seed(n))
}

// ============================================================================ F-732 (ADR 0655): the solves

/// The element of ADR 0649 that inverts on `8^3` at `a = 0.25` (`i = 4, j = 3, k = 7`), and on `4^3` (`i = 1, j = 1, k = 3`).
const E476: usize = 476;
const E53: usize = 53;

/// The options of a solve of ADR 0655: the driver's line search (c3) and the backtracking of the modes (c4).
#[derive(Clone, Copy)]
struct Opts {
    line_search: bool,
    backtracking: bool,
}

const DEFAULT: Opts = Opts {
    line_search: false,
    backtracking: false,
};
const LS: Opts = Opts {
    line_search: true,
    backtracking: false,
};
const BT: Opts = Opts {
    line_search: false,
    backtracking: true,
};
const LS_BT: Opts = Opts {
    line_search: true,
    backtracking: true,
};

impl Opts {
    fn name(&self) -> &'static str {
        match (self.line_search, self.backtracking) {
            (false, false) => "default",
            (true, false) => "line search",
            (false, true) => "backtracking",
            (true, true) => "line search + backtracking",
        }
    }
}

/// The block of ADR 0649 (reading c1) on a mesh, the punch at `delta`, 10 increments; `(R, w_c)` or NaN with the error.
fn run_block(mesh: &SolidMesh, tag: &str, delta: f64, kind: Kind, opts: Opts) -> Run {
    let mut m = model_of(mesh.clone(), kind);
    m.set_incompatible_mode_backtracking(opts.backtracking);
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
    let options = newton().line_search(opts.line_search);
    let (result, a, b, error) = match solve(&m, &[], &[], &options) {
        Ok(r) => {
            let punch = m.mesh().node_set("punch").unwrap().to_vec();
            let corner = m.mesh().node_set("corner").unwrap()[0];
            let rz = -reaction_sum(&r, &punch, 2) / MU;
            let wc = r.result.displacement(corner)[2];
            let halved = r.steps.iter().filter(|s| s.halvings > 0).count();
            let shortest = r
                .steps
                .iter()
                .flat_map(|s| s.step_lengths.iter().copied())
                .fold(f64::INFINITY, f64::min);
            let searched = r
                .steps
                .iter()
                .flat_map(|s| s.step_lengths.iter())
                .filter(|&&l| l < 1.0)
                .count();
            println!(
                "{tag} delta {delta} {} {}: R/mu {rz:.8}, w_c {wc:+.8}, increments {}, halved {halved}, step lengths < 1: \
                 {searched} (shortest {shortest:.4e})",
                kind.name,
                opts.name(),
                r.steps.len()
            );
            (Some(r), rz, wc, None)
        }
        Err(e) => {
            println!("{tag} delta {delta} {} {}: solve failed: {e}", kind.name, opts.name());
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

/// The load factor at the start of the failing increment named in a driver error (`load factor X to Y`), NaN if none.
fn failing_load_factor(error: &str) -> f64 {
    error
        .split("(load factor ")
        .nth(1)
        .and_then(|t| t.split(" to ").next())
        .and_then(|t| t.trim().parse::<f64>().ok())
        .unwrap_or(f64::NAN)
}

// ============================================================================ the measures of reading c2

fn det3(m: &[[f64; 3]; 3]) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// `v > 0` (false for NaN).
fn positive(v: f64) -> bool {
    v > 0.0
}

fn frobenius(m: &[[f64; 3]; 3]) -> f64 {
    m.iter().flatten().map(|v| v * v).sum::<f64>().sqrt()
}

/// The smallest `det(I + H_c + F_m)` over the points at the modes `alpha`; with `compatible_only` the mode part left out.
fn min_det(x: &[Vec3], ue: &[f64], alpha: &[f64; 9], compatible_only: bool) -> f64 {
    let g = point_gradients(x, ue, alpha).unwrap();
    least(g.iter().map(|(hc, fm)| {
        let f: [[f64; 3]; 3] = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                (if i == j { 1.0 } else { 0.0 }) + hc[i][j] + if compatible_only { 0.0 } else { fm[i][j] }
            })
        });
        det3(&f)
    }))
}

/// The largest `|F_m| / |H_c|` over the points at the modes `alpha`.
fn mode_ratio(x: &[Vec3], ue: &[f64], alpha: &[f64; 9]) -> f64 {
    let g = point_gradients(x, ue, alpha).unwrap();
    worst(g.iter().map(|(hc, fm)| frobenius(fm) / frobenius(hc)))
}

/// Gaussian elimination with partial pivoting of a 9 x 9 system.
fn solve9(mut a: [[f64; 9]; 9], mut b: [f64; 9]) -> [f64; 9] {
    for c in 0..9 {
        let p = (c..9).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs())).unwrap();
        a.swap(c, p);
        b.swap(c, p);
        for r in c + 1..9 {
            let l = a[r][c] / a[c][c];
            for k in c..9 {
                a[r][k] -= l * a[c][k];
            }
            b[r] -= l * b[c];
        }
    }
    let mut x = [0.0; 9];
    for r in (0..9).rev() {
        let mut t = b[r];
        for k in r + 1..9 {
            t -= a[r][k] * x[k];
        }
        x[r] = t / a[r][r];
    }
    x
}

/// The local iteration of the modes replicated (ADR 0655 D2): from `alpha = 0`, the full Newton step, stopped at
/// `|delta|_inf <= 1e-12 max(1, |alpha|_inf)`, 25 steps or the first inverted iterate; with `backtracking` an inverted
/// iterate halves its step as ADR 0655 D1.
struct Trace {
    /// The smallest `det F` of each iterate (iterate 0 is `alpha = 0`).
    dets: Vec<f64>,
    /// The index of the first inverted iterate of the plain iteration.
    inverted_at: Option<usize>,
    converged: bool,
    alpha: [f64; 9],
    halvings: usize,
}

fn local_iteration(x: &[Vec3], ue: &[f64], rule: IncompatibleStabilisation, backtracking: bool) -> Trace {
    let mat = material(NU);
    let ctx = PointContext::default();
    let st = ElementState::new(Vec::new());
    let mut alpha = [0.0; 9];
    let mut step: Option<([f64; 9], [f64; 9])> = None;
    let mut t = Trace {
        dets: Vec::new(),
        inverted_at: None,
        converged: false,
        alpha,
        halvings: 0,
    };
    for it in 0..=25 {
        let mut d = min_det(x, ue, &alpha, false);
        if backtracking {
            let mut m = 0;
            while !positive(d) && m < 20 {
                let Some((base, delta)) = step else { break };
                m += 1;
                let s = 0.5f64.powi(m);
                alpha = std::array::from_fn(|k| base[k] - s * delta[k]);
                d = min_det(x, ue, &alpha, false);
            }
            t.halvings += m as usize;
        }
        t.dets.push(d);
        if !positive(d) {
            t.inverted_at = Some(it);
            break;
        }
        let Ok((f, k)) = enhanced_force_and_tangent_with(x, &mat, ue, &alpha, &st, &ctx, rule) else {
            break;
        };
        let kv = k.values();
        let a: [[f64; 9]; 9] = std::array::from_fn(|i| std::array::from_fn(|j| kv[(24 + i) * 33 + 24 + j]));
        let r: [f64; 9] = std::array::from_fn(|i| f[24 + i]);
        let delta = solve9(a, r);
        let amax = alpha.iter().fold(1.0f64, |w, v| w.max(v.abs()));
        if delta.iter().fold(0.0f64, |w, v| w.max(v.abs())) <= 1e-12 * amax {
            t.converged = true;
            break;
        }
        step = Some((alpha, delta));
        for (ak, dk) in alpha.iter_mut().zip(&delta) {
            *ak -= dk;
        }
    }
    t.alpha = alpha;
    t
}

/// The measures of reading c2 at one element state.
struct Measures {
    dc: f64,
    de: f64,
    d1: f64,
    dmin: f64,
    q: f64,
    amax: f64,
    plain: Trace,
}

fn measures(tag: &str, x: &[Vec3], ue: &[f64], rule: IncompatibleStabilisation) -> Measures {
    let mat = material(NU);
    let ctx = PointContext::default();
    let st = ElementState::new(Vec::new());
    let zero = [0.0; 9];
    let dc = min_det(x, ue, &zero, true);
    let lib = modes_and_strains_with(x, &mat, ue, &st, &ctx, rule).map(|(a, _)| a);
    let plain = local_iteration(x, ue, rule, false);
    let (de, q, amax) = match &lib {
        Ok(a) => (
            min_det(x, ue, a, false),
            mode_ratio(x, ue, a),
            a.iter().fold(0.0f64, |w, v| w.max(v.abs())),
        ),
        Err(_) => (f64::NAN, f64::NAN, f64::NAN),
    };
    let d1 = plain.dets.get(1).copied().unwrap_or(f64::NAN);
    let dmin = plain.dets.iter().copied().fold(f64::INFINITY, f64::min);
    let replica = match &lib {
        Ok(a) if plain.converged => {
            let s = a.iter().fold(0.0f64, |w, v| w.max(v.abs()));
            worst((0..9).map(|k| (plain.alpha[k] - a[k]).abs() / s))
        }
        _ => f64::NAN,
    };
    println!(
        "C2 {tag}: dc {dc:.4e}, de {de:.4e}, d1 {d1:.4e}, dmin {dmin:.4e}, q {q:.4e}, |alpha|_inf {amax:.4e}, iterates {}, \
         first inverted {:?}, replica converged {}, |replica - library| / |alpha| {replica:.2e}, library {}",
        plain.dets.len(),
        plain.inverted_at,
        plain.converged,
        match &lib {
            Ok(_) => "converged".to_string(),
            Err(e) => format!("failed: {e}"),
        }
    );
    println!("C2 {tag}: min det F per iterate {:?}", plain.dets);
    Measures {
        dc,
        de,
        d1,
        dmin,
        q,
        amax,
        plain,
    }
}

/// The element displacement of a converged run.
fn run_element_u(run: &Run, e: usize) -> Option<Vec<f64>> {
    let r = run.result.as_ref()?;
    Some(element_u(&run.model, r.result.displacement_values(), e))
}

/// The path of reading c2 for element `e` and its classification (H1, H2, H3; ADR 0655 Checks).
fn cause(mesh: &SolidMesh, tag: &str, e: usize, kind: Kind) -> Vec<&'static str> {
    let x = element_coords(mesh, e);
    let deltas = [0.05, 0.06, 0.065, 0.067];
    let runs: Vec<Run> = deltas.iter().map(|&d| run_block(mesh, tag, d, kind, DEFAULT)).collect();
    let mut path = Vec::new();
    for (d, run) in deltas.iter().zip(&runs) {
        match run_element_u(run, e) {
            Some(ue) => {
                let m = measures(
                    &format!("{tag} {} element {e} delta {d}", kind.name),
                    &x,
                    &ue,
                    kind.rule,
                );
                path.push(Some(m));
            }
            None => {
                println!("C2 {tag} element {e} delta {d}: no converged state");
                path.push(None);
            }
        }
    }
    // The trial u_T: u(0.067) + (0.0678 - 0.067) / (0.067 - 0.065) (u(0.067) - u(0.065)).
    let trial = match (run_element_u(&runs[3], e), run_element_u(&runs[2], e)) {
        (Some(a), Some(b)) => {
            let s = (0.0678 - 0.067) / (0.067 - 0.065);
            Some(a.iter().zip(&b).map(|(p, q)| p + s * (p - q)).collect::<Vec<f64>>())
        }
        _ => None,
    };
    let mut out = Vec::new();
    let Some(ut) = trial else {
        println!("C2 {tag} element {e}: no trial u_T (a path state failed)");
        return out;
    };
    let mt = measures(
        &format!("{tag} {} element {e} trial u_T", kind.name),
        &x,
        &ut,
        kind.rule,
    );
    let bt = local_iteration(&x, &ut, kind.rule, true);
    let de_bt = if bt.converged {
        min_det(&x, &ut, &bt.alpha, false)
    } else {
        f64::NAN
    };
    let mat = material(NU);
    let ctx = PointContext::default();
    let st = ElementState::new(Vec::new());
    let lib_bt = condensed_force_and_tangent_backtracked(&x, &mat, &ut, &st, &ctx, kind.rule);
    let lib_plain = condensed_force_and_tangent(&x, &mat, &ut, &st, &ctx, kind.rule);
    println!(
        "C2 {tag} element {e} trial u_T backtracked replica: converged {}, de {de_bt:.4e}, halvings {}, iterates {}, min det \
         F per iterate {:?}; library backtracked {}, library plain {}",
        bt.converged,
        bt.halvings,
        bt.dets.len(),
        bt.dets,
        if lib_bt.is_ok() { "converged".to_string() } else { format!("{:?}", lib_bt.err()) },
        if lib_plain.is_ok() { "converged".to_string() } else { format!("failed: {}", lib_plain.err().unwrap()) },
    );
    let all = path.iter().all(|m| m.is_some());
    let dc_path_pos = all && path.iter().flatten().all(|m| m.dc > 0.0);
    let de_path_pos = all && path.iter().flatten().all(|m| m.de > 0.0);
    let h1 = dc_path_pos && de_path_pos && mt.dc > 0.0 && mt.dmin <= 0.0 && bt.converged && de_bt > 0.0;
    let h2 = !positive(mt.dc) || path.iter().flatten().any(|m| !positive(m.dc));
    let de_drop = match (&path[0], &path[3]) {
        (Some(a), Some(b)) => b.de <= 0.05 * a.de,
        _ => false,
    };
    let h3 = mt.dc > 0.0 && dc_path_pos && (!bt.converged || !positive(de_bt) || de_drop);
    println!(
        "C2 {tag} element {e} along the path: dc {:?}; de {:?}; d1 {:?}; q {:?}; |alpha|_inf {:?}",
        path.iter().map(|m| m.as_ref().map(|m| m.dc)).collect::<Vec<_>>(),
        path.iter().map(|m| m.as_ref().map(|m| m.de)).collect::<Vec<_>>(),
        path.iter().map(|m| m.as_ref().map(|m| m.d1)).collect::<Vec<_>>(),
        path.iter().map(|m| m.as_ref().map(|m| m.q)).collect::<Vec<_>>(),
        path.iter().map(|m| m.as_ref().map(|m| m.amax)).collect::<Vec<_>>(),
    );
    let _ = (mt.d1, mt.q, mt.amax, mt.de, mt.plain.converged);
    for (h, name) in [(h1, "H1"), (h2, "H2"), (h3, "H3")] {
        if h {
            out.push(name);
        }
    }
    println!("C2 {tag} element {e}: hypotheses that hold {out:?}");
    out
}

// ============================================================================ the checks

#[test]
/// CA (c2): the cause of the inverted point of element 476 on the `8^3` block at `a = 0.25`, rule L.
fn ca_cause_of_the_inverted_point() {
    let mut ch = Checks::new();
    let (mesh, _) = block_mesh(8, 0.25);
    let held = cause(&mesh, "CA 8^3 a 0.25", E476, K_L);
    ch.equals("CA: hypotheses that hold (exactly one)", held.len(), 1);
    println!("CA: located as {held:?}");
    ch.finish();
}

/// The checks of a remedy that converged (c5): DB4 and SL rows; true if all met.
fn remedy_rows(ch: &mut Checks, id: &str, run: &Run, f: &Run) -> bool {
    let before = ch.0.len();
    ch.at_most(&format!("DB4{id} e_R = |R / R_Fbar - 1|"), rel(run.a, f.a), 0.10);
    ch.at_most(&format!("DB4{id} e_w = |w / w_Fbar - 1|"), rel(run.b, f.b), 0.10);
    let (neg, low, _) = stability(run, false);
    ch.at_most(&format!("SL{id} negative eigenvalues of K_ff"), neg, 0.0);
    ch.above(&format!("SL{id} lowest eigenvalue / max diagonal"), low, 0.0);
    ch.0.len() == before
}

#[test]
/// NC1, NC3, LS (c3), BT (c4) with DB4b, SLb, T-FD, BA, P; both together reported.
fn db4_line_search_and_backtracking() {
    let mut ch = Checks::new();
    let (mesh, _) = block_mesh(8, 0.25);
    let tag = "DB4 8^3 a 0.25";
    println!(
        "report the mesh of ADR 0649 (M0 0.0762): min corner det J / centre det J {:.4e}",
        min_corner_ratio(&mesh)
    );
    let f = run_block(&mesh, tag, DELTA, K_FBAR, DEFAULT);
    ch.at_most("NC3 F-bar |R / 0.34220247 - 1|", rel(f.a, 0.34220247), 1e-6);
    ch.at_most("NC3 F-bar |w_c / 0.06389947 - 1|", rel(f.b, 0.06389947), 1e-6);
    let (fneg, flow, _) = stability(&f, false);
    println!("report SL F-bar: negative {fneg}, lowest / max diagonal {flow:.4e}");
    // NC1: the default fails as in ADR 0649.
    let d = run_block(&mesh, tag, DELTA, K_L, DEFAULT);
    let err = d.error.clone().unwrap_or_default();
    let lf = failing_load_factor(&err);
    ch.equals("NC1 default: solve failed (1 = yes)", d.result.is_none() as usize, 1);
    ch.equals(
        "NC1 default: error names element 476 (1 = yes)",
        err.starts_with("element 476:") as usize,
        1,
    );
    ch.at_least("NC1 default: failing load factor >= 0.67", lf, 0.67);
    ch.at_most("NC1 default: failing load factor <= 0.68", lf, 0.68);
    // LS (c3).
    let ls = run_block(&mesh, tag, DELTA, K_L, LS);
    let ls_pass = match &ls.result {
        Some(_) => remedy_rows(&mut ch, "l", &ls, &f),
        None => {
            println!(
                "LS line search: did not pass (measured): failing load factor {:.6}, {}",
                failing_load_factor(ls.error.as_deref().unwrap_or("")),
                ls.error.as_deref().unwrap_or("")
            );
            false
        }
    };
    // BT (c4).
    let bt = run_block(&mesh, tag, DELTA, K_L, BT);
    let bt_pass = match &bt.result {
        Some(_) => remedy_rows(&mut ch, "b", &bt, &f),
        None => {
            println!(
                "BT backtracking: did not pass (measured): {}",
                bt.error.as_deref().unwrap_or("")
            );
            false
        }
    };
    ch.equals(
        "P: one remedy passes with its DB4 and SL rows (1 = yes)",
        (ls_pass || bt_pass) as usize,
        1,
    );
    // T-FD and BA at the loaded state of BT, element 476.
    let mat = material(NU);
    let ctx = PointContext::default();
    let st = ElementState::new(Vec::new());
    let x = element_coords(&mesh, E476);
    let (tfd, ba) = match run_element_u(&bt, E476) {
        Some(ue) => {
            let fd = match condensed_force_and_tangent_backtracked(&x, &mat, &ue, &st, &ctx, LIN) {
                Ok((_, k, _)) => {
                    let kv = k.values();
                    let h = 1e-7;
                    let mut dmax = 0.0f64;
                    let mut ok = true;
                    for j in 0..24 {
                        let mut a = ue.clone();
                        a[j] += h;
                        let mut b = ue.clone();
                        b[j] -= h;
                        match (
                            condensed_force_and_tangent_backtracked(&x, &mat, &a, &st, &ctx, LIN),
                            condensed_force_and_tangent_backtracked(&x, &mat, &b, &st, &ctx, LIN),
                        ) {
                            (Ok((fa, _, _)), Ok((fb, _, _))) => {
                                for i in 0..24 {
                                    dmax = dmax.max((kv[i * 24 + j] - (fa[i] - fb[i]) / (2.0 * h)).abs());
                                }
                            }
                            _ => ok = false,
                        }
                    }
                    let scale = kv.iter().fold(0.0f64, |w, v| w.max(v.abs()));
                    if ok {
                        dmax / scale
                    } else {
                        f64::NAN
                    }
                }
                Err(_) => f64::NAN,
            };
            let plain = condensed_force_and_tangent(&x, &mat, &ue, &st, &ctx, LIN);
            if let Err(e) = &plain {
                println!("BA element 476 without the backtracking: {e}");
            }
            let inverted = matches!(&plain, Err(e) if e.to_string().contains("inverted"));
            let _ = measures("BT loaded state element 476", &x, &ue, LIN);
            (fd, inverted as usize)
        }
        None => (f64::NAN, 0),
    };
    ch.at_most("T-FD element 476 at the BT state: max |K - K_fd| / max |K|", tfd, 1e-6);
    ch.equals(
        "BA element 476 at the BT state without the backtracking: inverted (1 = yes)",
        ba,
        1,
    );
    // Both together (reported).
    let both = run_block(&mesh, tag, DELTA, K_L, LS_BT);
    if both.result.is_some() {
        let (neg, low, _) = stability(&both, false);
        println!(
            "report LS+BT: e_R {:.4e}, e_w {:.4e}, negative {neg}, lowest / max diagonal {low:.4e}",
            rel(both.a, f.a),
            rel(both.b, f.b)
        );
    }
    ch.finish();
}

/// The cantilever of ADR 0636 reading c2 (ADR 0649 DC2's set-up on `10 x 2 x 2` at `a = 0.25`).
fn run_cantilever(mesh: &SolidMesh, kind: Kind, opts: Opts) -> Option<NonlinearResult> {
    let mut m = model_of(mesh.clone(), kind);
    m.set_incompatible_mode_backtracking(opts.backtracking);
    let mut bcs: Vec<(&str, usize, f64)> = (0..3).map(|c| ("xmin", c, 0.0)).collect();
    bcs.push(("xmax", 1, 0.3));
    constrain(&mut m, &bcs);
    match solve(&m, &[], &[], &newton()) {
        Ok(r) => Some(r),
        Err(e) => {
            println!("NC2 cantilever {}: solve failed: {e}", opts.name());
            None
        }
    }
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

#[test]
/// NC2 (c7): with the opt-in on set-ups that do not need it, the results are bitwise those of the default.
fn nc2_backtracking_changes_no_bit_where_not_needed() {
    let mut ch = Checks::new();
    let (mesh, _) = block_mesh(8, 0.1);
    let off = run_block(&mesh, "NC2 8^3 a 0.1", DELTA, K_L, DEFAULT);
    let on = run_block(&mesh, "NC2 8^3 a 0.1", DELTA, K_L, BT);
    ch.equals(
        "NC2 block 8^3 a 0.1 rule L: values differing in bits, backtracking against default",
        bit_differences(off.result.as_ref(), on.result.as_ref()),
        0,
    );
    let (cm, _) = distort(
        &hex_box([0.0; 3], [1.0, 0.2, 0.2], [10, 2, 2]).unwrap(),
        [1.0, 0.2, 0.2],
        [10, 2, 2],
        0.25,
        0xF719_1010,
    );
    let off = run_cantilever(&cm, K_L, DEFAULT);
    let on = run_cantilever(&cm, K_L, BT);
    ch.equals(
        "NC2 cantilever 10x2x2 a 0.25 rule L: values differing in bits, backtracking against default",
        bit_differences(off.as_ref(), on.as_ref()),
        0,
    );
    ch.finish();
}

#[test]
/// c6: the plain element (`Off`) on the `4^3` block at `a = 0.25`: the default, the line search, the backtracking against
/// F-bar, and the measures of reading c2 for element 53. Reported, no limit.
fn plain_element_4_cubed_reported() {
    let (mesh, _) = block_mesh(4, 0.25);
    let tag = "C6 4^3 a 0.25";
    let f = run_block(&mesh, tag, DELTA, K_FBAR, DEFAULT);
    for opts in [DEFAULT, LS, BT, LS_BT] {
        let run = run_block(&mesh, tag, DELTA, K_INC, opts);
        if run.result.is_some() {
            let (neg, low, _) = stability(&run, true);
            println!(
                "report C6 Inc {}: e_R {:.4e}, e_w {:.4e}, negative {neg}, lowest / max diagonal {low:.4e}",
                opts.name(),
                rel(run.a, f.a),
                rel(run.b, f.b)
            );
        }
    }
    let held = cause(&mesh, tag, E53, K_INC);
    println!("report C6 element 53: hypotheses that hold {held:?}");
}
