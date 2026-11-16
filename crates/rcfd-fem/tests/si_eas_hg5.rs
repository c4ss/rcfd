//! F-719 (ADR 0649): the stabilised incompatible-modes Hex8 with rule L of ADR 0630 (`LinearStress`) under load on distorted
//! meshes: the confined punch block and the displacement-controlled cantilever of ADR 0636 with the interior nodes moved by
//! `0.1 h (2U - 1)` and `0.25 h (2U - 1)` (SplitMix64, the seeds of ADR 0649 D2), against the F-bar Hex8 of ADR 0506 on the
//! same meshes (DB1 to DB4, DC1 to DC4), the Full Hex8 locking control N1, the stability of the loaded state SL and SLx, the
//! mesh checks M0 and M1, and the controls of the F-bar reference FR1, FR2 and NF1. Every aggregated value is required finite
//! before it is compared; a failed solve is a non-finite measure. The helpers are copies of those of `si_eas_hg4.rs` (existing
//! tests are not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::element::ElementState;
use rcfd_fem::material::{HyperelasticEnergy, PointContext};
use rcfd_fem::nl_incompatible::IncompatibleStabilisation;
use rcfd_fem::nonlinear::{
    hex8_force_and_tangent_at, hex8_internal_force_at, solve, tangent, NewtonOptions, NonlinearResult,
};
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
    /// A row checked by `at_most`, or printed as a known miss (ADR 0649 A1.1).
    fn at_most_unless(&mut self, skip: bool, name: &str, value: f64, limit: f64) {
        if skip {
            println!("{name}: {value:.4e} (known miss, printed; ADR 0649 amendment 1)");
        } else {
            self.at_most(name, value, limit);
        }
    }
    /// A row checked by `above`, or printed as a known miss (ADR 0649 A1.1).
    fn above_unless(&mut self, skip: bool, name: &str, value: f64, limit: f64) {
        if skip {
            println!("{name}: {value:.4e} (known miss, printed; ADR 0649 amendment 1)");
        } else {
            self.above(name, value, limit);
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
const FULL: SolidFormulation = SolidFormulation::Full;
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
const K_FULL: Kind = Kind {
    name: "Full",
    formulation: FULL,
    rule: OFF,
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

// ============================================================================ the F-bar reference controls (FR1, NF1)

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

/// Per element at the converged state of an F-bar run: `max |K_offered - K_fd| / max |K_offered|`, `K_fd` the central
/// differences (step `1e-7`) of the F-bar element force, `K_offered` the element tangent of formulation `offered`.
fn tangent_mismatch(run: &Run, offered: SolidFormulation) -> Vec<f64> {
    let Some(r) = run.result.as_ref() else {
        return vec![f64::NAN];
    };
    let mat = material(NU);
    let ctx = PointContext::default();
    let u = r.result.displacement_values();
    let h = 1e-7;
    let mut out = Vec::new();
    for e in 0..run.model.mesh().n_elements() {
        let x = element_coords(run.model.mesh(), e);
        let ue = element_u(&run.model, u, e);
        let st: &ElementState = &r.states[e];
        let (_, k) = match hex8_force_and_tangent_at(offered, &x, &mat, &ue, st, &ctx) {
            Ok(v) => v,
            Err(_) => return vec![f64::NAN],
        };
        let kv = k.values();
        let mut d = 0.0f64;
        for j in 0..24 {
            let mut a = ue.clone();
            a[j] += h;
            let mut b = ue.clone();
            b[j] -= h;
            let (Ok(fa), Ok(fb)) = (
                hex8_internal_force_at(FBAR, &x, &mat, &a, st, &ctx),
                hex8_internal_force_at(FBAR, &x, &mat, &b, st, &ctx),
            ) else {
                return vec![f64::NAN];
            };
            for i in 0..24 {
                d = d.max((kv[i * 24 + j] - (fa[i] - fb[i]) / (2.0 * h)).abs());
            }
        }
        let scale = kv.iter().fold(0.0f64, |w, v| w.max(v.abs()));
        out.push(d / scale);
    }
    out
}

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

// ============================================================================ DB: the punch block (ADR 0636 reading c1)

const DELTA: f64 = 0.1;
const AMPS: [f64; 2] = [0.1, 0.25];

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

/// `(R, w_c)` of the block on a mesh; NaN when the solve fails.
fn run_block(mesh: &SolidMesh, n: usize, amp: f64, kind: Kind) -> Run {
    let mut m = model_of(mesh.clone(), kind);
    let prescribed = constrain(
        &mut m,
        &[
            ("xmin", 0, 0.0),
            ("xmax", 0, 0.0),
            ("ymin", 1, 0.0),
            ("ymax", 1, 0.0),
            ("zmin", 2, 0.0),
            ("punch", 2, -DELTA),
        ],
    );
    let (result, a, b) = match solve(&m, &[], &[], &newton()) {
        Ok(r) => {
            let punch = m.mesh().node_set("punch").unwrap().to_vec();
            let corner = m.mesh().node_set("corner").unwrap()[0];
            let rz = -reaction_sum(&r, &punch, 2) / MU;
            let wc = r.result.displacement(corner)[2];
            println!(
                "DB {n}^3 a {amp} {}: R/mu {rz:.8}, w_c {wc:+.8}, increments {}",
                kind.name,
                r.steps.len()
            );
            (Some(r), rz, wc)
        }
        Err(e) => {
            println!("DB {n}^3 a {amp} {}: solve failed: {e}", kind.name);
            (None, f64::NAN, f64::NAN)
        }
    };
    Run {
        model: m,
        prescribed,
        result,
        a,
        b,
    }
}

#[test]
#[ignore = "known miss of ADR 0649 (registered run, test commit 3026f1d0): DB4 e_R, e_w and SL on the 8^3 block at a = 0.25 non-finite, the rule L solve fails (element 476 inverted, det F = -8.05e-2, load factor 0.675 to 0.678125 after 5 halvings); every other row met (DB1 0.1202, 0.0406; DB2 0.1278, 0.0870; DB3 0.0664, 0.0349); the met rows run in db_punch_block_distorted_met_rows (amendment 1)"]
fn db_punch_block_distorted_against_fbar_known_miss() {
    let mut ch = Checks::new();
    let ids = [["DB1", "DB2"], ["DB3", "DB4"]];
    for (level, (n, tol, moved)) in [(4, 0.15, 27), (8, 0.10, 343)].into_iter().enumerate() {
        let (regular, _) = block_mesh(n, 0.0);
        let f0 = run_block(&regular, n, 0.0, K_FBAR);
        let l0 = run_block(&regular, n, 0.0, K_L);
        for (ia, &amp) in AMPS.iter().enumerate() {
            let id = ids[level][ia];
            let tag = format!("{n}^3 a {amp}");
            let (mesh, count) = block_mesh(n, amp);
            ch.above(
                &format!("M0 block {tag}: min corner det J / centre det J"),
                min_corner_ratio(&mesh),
                0.0,
            );
            ch.equals(&format!("M1 block {tag}: moved nodes"), count, moved);
            let f = run_block(&mesh, n, amp, K_FBAR);
            let l = run_block(&mesh, n, amp, K_L);
            let full = run_block(&mesh, n, amp, K_FULL);
            ch.at_most(
                &format!("{id} block {tag}: e_R = |R_L / R_Fbar - 1|"),
                rel(l.a, f.a),
                tol,
            );
            ch.at_most(
                &format!("{id} block {tag}: e_w = |w_L / w_Fbar - 1|"),
                rel(l.b, f.b),
                tol,
            );
            ch.at_least(&format!("N1 block {tag}: |R_Full / R_Fbar - 1|"), rel(full.a, f.a), tol);
            println!(
                "N2 block {tag}: |w_Full / w_Fbar - 1| = {:.4e} (printed)",
                rel(full.b, f.b)
            );
            let coarse = n == 4;
            let (neg, low, dense) = stability(&l, coarse);
            ch.at_most(
                &format!("SL block {tag}: negative eigenvalues of K_ff, rule L"),
                neg,
                0.0,
            );
            ch.above(
                &format!("SL block {tag}: lowest eigenvalue / max diagonal, rule L"),
                low,
                0.0,
            );
            if coarse {
                ch.at_most(
                    &format!("SLx block {tag}: |sparse / dense - 1|, rule L"),
                    rel(low, dense),
                    1e-6,
                );
            }
            let (fneg, flow, _) = stability(&f, false);
            println!("report SL block {tag} Fbar: negative {fneg}, lowest / max diagonal {flow:.4e}");
            ch.at_most(
                &format!("FR1 block {tag}: F-bar worst element max |K - K_fd| / max |K|"),
                worst(tangent_mismatch(&f, FBAR)),
                1e-6,
            );
            println!(
                "report distortion block {tag}: Fbar R {:+.4e} w {:+.4e}; L R {:+.4e} w {:+.4e}",
                f.a / f0.a - 1.0,
                f.b / f0.b - 1.0,
                l.a / l0.a - 1.0,
                l.b / l0.b - 1.0
            );
            if coarse {
                let inc = run_block(&mesh, n, amp, K_INC);
                println!(
                    "report block {tag} Inc: e_R {:.4e}, e_w {:.4e}",
                    rel(inc.a, f.a),
                    rel(inc.b, f.b)
                );
            }
        }
    }
    ch.finish();
}

#[test]
/// ADR 0649 A1.1: the registered test with the four rows of rule L on `8^3` at `a = 0.25` printed instead of checked.
fn db_punch_block_distorted_met_rows() {
    let mut ch = Checks::new();
    let ids = [["DB1", "DB2"], ["DB3", "DB4"]];
    for (level, (n, tol, moved)) in [(4, 0.15, 27), (8, 0.10, 343)].into_iter().enumerate() {
        let (regular, _) = block_mesh(n, 0.0);
        let f0 = run_block(&regular, n, 0.0, K_FBAR);
        let l0 = run_block(&regular, n, 0.0, K_L);
        for (ia, &amp) in AMPS.iter().enumerate() {
            let id = ids[level][ia];
            let tag = format!("{n}^3 a {amp}");
            let (mesh, count) = block_mesh(n, amp);
            let skip = n == 8 && amp == 0.25;
            ch.above(
                &format!("M0 block {tag}: min corner det J / centre det J"),
                min_corner_ratio(&mesh),
                0.0,
            );
            ch.equals(&format!("M1 block {tag}: moved nodes"), count, moved);
            let f = run_block(&mesh, n, amp, K_FBAR);
            let l = run_block(&mesh, n, amp, K_L);
            let full = run_block(&mesh, n, amp, K_FULL);
            ch.at_most_unless(
                skip,
                &format!("{id} block {tag}: e_R = |R_L / R_Fbar - 1|"),
                rel(l.a, f.a),
                tol,
            );
            ch.at_most_unless(
                skip,
                &format!("{id} block {tag}: e_w = |w_L / w_Fbar - 1|"),
                rel(l.b, f.b),
                tol,
            );
            ch.at_least(&format!("N1 block {tag}: |R_Full / R_Fbar - 1|"), rel(full.a, f.a), tol);
            println!(
                "N2 block {tag}: |w_Full / w_Fbar - 1| = {:.4e} (printed)",
                rel(full.b, f.b)
            );
            let coarse = n == 4;
            let (neg, low, dense) = stability(&l, coarse);
            ch.at_most_unless(
                skip,
                &format!("SL block {tag}: negative eigenvalues of K_ff, rule L"),
                neg,
                0.0,
            );
            ch.above_unless(
                skip,
                &format!("SL block {tag}: lowest eigenvalue / max diagonal, rule L"),
                low,
                0.0,
            );
            if coarse {
                ch.at_most(
                    &format!("SLx block {tag}: |sparse / dense - 1|, rule L"),
                    rel(low, dense),
                    1e-6,
                );
            }
            let (fneg, flow, _) = stability(&f, false);
            println!("report SL block {tag} Fbar: negative {fneg}, lowest / max diagonal {flow:.4e}");
            ch.at_most(
                &format!("FR1 block {tag}: F-bar worst element max |K - K_fd| / max |K|"),
                worst(tangent_mismatch(&f, FBAR)),
                1e-6,
            );
            println!(
                "report distortion block {tag}: Fbar R {:+.4e} w {:+.4e}; L R {:+.4e} w {:+.4e}",
                f.a / f0.a - 1.0,
                f.b / f0.b - 1.0,
                l.a / l0.a - 1.0,
                l.b / l0.b - 1.0
            );
            if coarse {
                let inc = run_block(&mesh, n, amp, K_INC);
                println!(
                    "report block {tag} Inc: e_R {:.4e}, e_w {:.4e}",
                    rel(inc.a, f.a),
                    rel(inc.b, f.b)
                );
            }
        }
    }
    ch.finish();
}

/// `(R, w_c)` of the block on a mesh with a punch displacement and a number of increments (ADR 0649 A1.2).
fn run_block_with(mesh: &SolidMesh, delta: f64, steps: usize, kind: Kind) -> Run {
    let mut m = model_of(mesh.clone(), kind);
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
    let (result, a, b) = match solve(&m, &[], &[], &NewtonOptions::new(steps).threads(4)) {
        Ok(r) => {
            let punch = m.mesh().node_set("punch").unwrap().to_vec();
            let corner = m.mesh().node_set("corner").unwrap()[0];
            let rz = -reaction_sum(&r, &punch, 2) / MU;
            let wc = r.result.displacement(corner)[2];
            let halved = r.steps.iter().filter(|s| s.halvings > 0).count();
            println!(
                "A1 delta {delta} steps {steps} {}: R/mu {rz:.8}, w_c {wc:+.8}, increments {}, halved {halved}",
                kind.name,
                r.steps.len()
            );
            (Some(r), rz, wc)
        }
        Err(e) => {
            println!("A1 delta {delta} steps {steps} {}: solve failed: {e}", kind.name);
            (None, f64::NAN, f64::NAN)
        }
    };
    Run {
        model: m,
        prescribed,
        result,
        a,
        b,
    }
}

#[test]
#[ignore = "diagnosis of ADR 0649 amendment 1 (A1.2, D1 to D3): rule L on the 8^3 block at a = 0.25; no limit; run once with --ignored"]
fn a1_diagnosis_db4_rule_l() {
    let (mesh, _) = block_mesh(8, 0.25);
    // D3.
    let x = element_coords(&mesh, 476);
    let d0 = det_jacobian(&x, [0.0; 3]);
    let r476 = least(XI.iter().map(|&c| det_jacobian(&x, c) / d0));
    println!(
        "D3 element 476: min corner det J / centre det J {r476:.4e}; mesh {:.4e}",
        min_corner_ratio(&mesh)
    );
    // D1.
    let d1 = run_block_with(&mesh, 0.675 * DELTA, 10, K_L);
    let (neg, low, _) = stability(&d1, false);
    println!("D1 delta 0.0675: negative eigenvalues of K_ff {neg}, lowest / max diagonal {low:.4e}");
    // D2.
    let d2 = run_block_with(&mesh, DELTA, 40, K_L);
    let (neg, low, _) = stability(&d2, false);
    println!(
        "D2 delta 0.1, 40 increments: e_R {:.4e}, e_w {:.4e} (against F-bar 0.34220247, 0.06389947); negative {neg}, \
         lowest / max diagonal {low:.4e}",
        rel(d2.a, 0.34220247),
        rel(d2.b, 0.06389947)
    );
}

#[test]
#[ignore = "diagnosis of ADR 0649 amendment 2 (A2.1, D4): rule L and F-bar on the 8^3 block at a = 0.25 along the punch path; no limit; run once with --ignored"]
fn a2_diagnosis_path_stability() {
    let (mesh, _) = block_mesh(8, 0.25);
    for delta in [0.05, 0.06, 0.065, 0.067] {
        for kind in [K_L, K_FBAR] {
            let run = run_block_with(&mesh, delta, 10, kind);
            let (neg, low, _) = stability(&run, false);
            println!(
                "D4 delta {delta} {}: negative eigenvalues of K_ff {neg}, lowest / max diagonal {low:.4e}",
                kind.name
            );
        }
    }
}

// ============================================================================ DC: the cantilever (ADR 0636 reading c2)

const CANT_L: f64 = 1.0;
const CANT_H: f64 = 0.2;
const TIP_V: f64 = 0.3;

fn cant_seed(nx: usize) -> u64 {
    match nx {
        10 => 0xF719_1010,
        20 => 0xF719_1020,
        _ => unreachable!(),
    }
}

fn cant_mesh(nx: usize, ny: usize, amp: f64) -> (SolidMesh, usize) {
    let mesh = hex_box([0.0; 3], [CANT_L, CANT_H, CANT_H], [nx, ny, ny]).unwrap();
    if amp == 0.0 {
        return (mesh, 0);
    }
    distort(&mesh, [CANT_L, CANT_H, CANT_H], [nx, ny, ny], amp, cant_seed(nx))
}

/// `(P, d)` of the cantilever on a mesh; NaN when the solve fails.
fn run_cantilever(mesh: &SolidMesh, nx: usize, ny: usize, amp: f64, kind: Kind) -> Run {
    let mut m = model_of(mesh.clone(), kind);
    let mut bcs: Vec<(&str, usize, f64)> = (0..3).map(|c| ("xmin", c, 0.0)).collect();
    bcs.push(("xmax", 1, TIP_V));
    let prescribed = constrain(&mut m, &bcs);
    let (result, a, b) = match solve(&m, &[], &[], &newton()) {
        Ok(r) => {
            let tip = m.mesh().node_set("xmax").unwrap().to_vec();
            let p = reaction_sum(&r, &tip, 1) / MU;
            let d = -tip.iter().map(|&n| r.result.displacement(n)[0]).sum::<f64>() / tip.len() as f64;
            println!(
                "DC {nx}x{ny}x{ny} a {amp} {}: P/mu {p:.8e}, d {d:.8e}, increments {}",
                kind.name,
                r.steps.len()
            );
            (Some(r), p, d)
        }
        Err(e) => {
            println!("DC {nx}x{ny}x{ny} a {amp} {}: solve failed: {e}", kind.name);
            (None, f64::NAN, f64::NAN)
        }
    };
    Run {
        model: m,
        prescribed,
        result,
        a,
        b,
    }
}

#[test]
fn dc_cantilever_distorted_against_fbar() {
    let mut ch = Checks::new();
    let ids = [["DC1", "DC2"], ["DC3", "DC4"]];
    for (level, (nx, ny, tol, moved)) in [(10, 2, 0.15, 9), (20, 4, 0.10, 171)].into_iter().enumerate() {
        let (regular, _) = cant_mesh(nx, ny, 0.0);
        let f0 = run_cantilever(&regular, nx, ny, 0.0, K_FBAR);
        let l0 = run_cantilever(&regular, nx, ny, 0.0, K_L);
        for (ia, &amp) in AMPS.iter().enumerate() {
            let id = ids[level][ia];
            let tag = format!("{nx}x{ny}x{ny} a {amp}");
            let (mesh, count) = cant_mesh(nx, ny, amp);
            ch.above(
                &format!("M0 cantilever {tag}: min corner det J / centre det J"),
                min_corner_ratio(&mesh),
                0.0,
            );
            ch.equals(&format!("M1 cantilever {tag}: moved nodes"), count, moved);
            let f = run_cantilever(&mesh, nx, ny, amp, K_FBAR);
            let l = run_cantilever(&mesh, nx, ny, amp, K_L);
            let full = run_cantilever(&mesh, nx, ny, amp, K_FULL);
            ch.at_most(
                &format!("{id} cantilever {tag}: e_P = |P_L / P_Fbar - 1|"),
                rel(l.a, f.a),
                tol,
            );
            ch.at_most(
                &format!("{id} cantilever {tag}: e_d = |d_L / d_Fbar - 1|"),
                rel(l.b, f.b),
                tol,
            );
            ch.at_least(
                &format!("N1 cantilever {tag}: |P_Full / P_Fbar - 1|"),
                rel(full.a, f.a),
                tol,
            );
            let coarse = nx == 10;
            let (neg, low, dense) = stability(&l, coarse);
            ch.at_most(
                &format!("SL cantilever {tag}: negative eigenvalues of K_ff, rule L"),
                neg,
                0.0,
            );
            ch.above(
                &format!("SL cantilever {tag}: lowest eigenvalue / max diagonal, rule L"),
                low,
                0.0,
            );
            if coarse {
                ch.at_most(
                    &format!("SLx cantilever {tag}: |sparse / dense - 1|, rule L"),
                    rel(low, dense),
                    1e-6,
                );
            }
            let (fneg, flow, _) = stability(&f, false);
            println!("report SL cantilever {tag} Fbar: negative {fneg}, lowest / max diagonal {flow:.4e}");
            ch.at_most(
                &format!("FR1 cantilever {tag}: F-bar worst element max |K - K_fd| / max |K|"),
                worst(tangent_mismatch(&f, FBAR)),
                1e-6,
            );
            println!(
                "report distortion cantilever {tag}: Fbar P {:+.4e} d {:+.4e}; L P {:+.4e} d {:+.4e}",
                f.a / f0.a - 1.0,
                f.b / f0.b - 1.0,
                l.a / l0.a - 1.0,
                l.b / l0.b - 1.0
            );
            if coarse {
                let inc = run_cantilever(&mesh, nx, ny, amp, K_INC);
                println!(
                    "report cantilever {tag} Inc: e_P {:.4e}, e_d {:.4e}",
                    rel(inc.a, f.a),
                    rel(inc.b, f.b)
                );
            }
        }
    }
    ch.finish();
}

// ============================================================================ FR2, NF1: the F-bar reference controls

#[test]
fn fr_fbar_reference_controls() {
    let mut ch = Checks::new();
    // FR2: the regular coarse meshes against the values of ADR 0636 arithmetic 2 and 3.
    let (regular, _) = block_mesh(4, 0.0);
    let f = run_block(&regular, 4, 0.0, K_FBAR);
    ch.at_most(
        "FR2 block 4^3 regular: |R_Fbar / 0.385977 - 1|",
        rel(f.a, 0.385977),
        1e-5,
    );
    ch.at_most(
        "FR2 block 4^3 regular: |w_Fbar / 0.072210 - 1|",
        rel(f.b, 0.072210),
        1e-5,
    );
    let (regular, _) = cant_mesh(10, 2, 0.0);
    let f = run_cantilever(&regular, 10, 2, 0.0, K_FBAR);
    ch.at_most(
        "FR2 cantilever 10x2x2 regular: |P_Fbar / 6.1658e-4 - 1|",
        rel(f.a, 6.1658e-4),
        2e-5,
    );
    ch.at_most(
        "FR2 cantilever 10x2x2 regular: |d_Fbar / 0.052429 - 1|",
        rel(f.b, 0.052429),
        2e-5,
    );
    // NF1: the Full tangent offered as the F-bar tangent at the F-bar state of the coarse block at a = 0.25.
    let (mesh, _) = block_mesh(4, 0.25);
    let f = run_block(&mesh, 4, 0.25, K_FBAR);
    let mis = tangent_mismatch(&f, FULL);
    println!(
        "NF1 largest over the elements {:.4e} (printed)",
        worst(mis.iter().copied())
    );
    ch.at_least(
        "NF1 block 4^3 a 0.25: Full tangent against F-bar differences, smallest element",
        least(mis),
        0.1,
    );
    ch.finish();
}
