//! F-742 (ADR 0666): the warm start of the local mode iteration of the stabilised incompatible-modes Hex8 (the remedy W of
//! ADR 0660) with a J2 material on the punch block of ADR 0636 and on DB4 of ADR 0649 against the F-bar Hex8 (JB1, JB2), its
//! active-set record of ADR 0515 against the cold S (AS1, AS2, NCA), the committed modes (JL0, JL2); on the nonlinear
//! transient path with the energy record from the committed modes (TT1, TT2, TE1, TE2, NCE) and the restore of a warm state
//! (TS1, NCR); on the rotating path (RO1). Every aggregated value is required finite before it is compared; a failed solve is
//! a non-finite measure. The helpers are copies of those of `si_eas_hg8.rs` (existing tests are not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::element::{element_for, ElementState};
use rcfd_fem::material::{HyperelasticEnergy, PointContext};
use rcfd_fem::nl_incompatible::{
    mode_iteration_record_from, point_gradients, stabilisation_energy_with, IncompatibleModeUpdate,
    IncompatibleStabilisation,
};
use rcfd_fem::nl_transient::{Integrator, NlTransientOptions, NlTransientState};
use rcfd_fem::nonlinear::{solve, tangent, NewtonOptions, NonlinearResult};
use rcfd_fem::rotating::{RotatingOptions, RotatingPart, SpeedModes};
use rcfd_fem::transient::{LoadHistory, LoadTable, Rayleigh};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Material, SolidFormulation, SolidMesh, SolidModel};
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

// ============================================================================ the materials and the elements

const MU: f64 = 1.0e6;
const NU: f64 = 0.49;
const INC: SolidFormulation = SolidFormulation::Incompatible;
const FBAR: SolidFormulation = SolidFormulation::BBar;
const OFF: IncompatibleStabilisation = IncompatibleStabilisation::Off;
const LIN: IncompatibleStabilisation = IncompatibleStabilisation::LinearStress;
/// Young's modulus of the shear modulus `MU` at `NU` (reading c1).
const E_MOD: f64 = 2.0 * MU * (1.0 + NU);
/// The J2 constants of reading c1.
const SIGMA_Y: f64 = 6.0e4;
const H_ISO: f64 = 3.0e5;
/// Values per point of the J2 state, its equivalent plastic strain.
const J2_SIZE: usize = rcfd_fem::mat_j2::STATE_SIZE;
const POINTS: usize = 8;

fn kappa_of(nu: f64) -> f64 {
    2.0 * MU * (1.0 + nu) / (3.0 * (1.0 - 2.0 * nu))
}

fn energy_of() -> HyperelasticEnergy {
    HyperelasticEnergy::neo_hookean(MU, kappa_of(NU)).unwrap()
}

/// The neo-Hookean material of ADR 0636 (density 1000).
fn neo_hookean() -> Material {
    Material::hyperelastic(energy_of(), 1000.0).unwrap()
}

fn elastic() -> Material {
    Material::isotropic_elastic(E_MOD, NU, 1000.0, 0.0).unwrap()
}

/// The J2 material of reading c1.
fn j2() -> Material {
    Material::j2(elastic(), SIGMA_Y, H_ISO, 0.0).unwrap()
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

/// The opt-ins of a solve: the backtracking of the modes (ADR 0655 D1), its decrease test (ADR 0658 D1), the warm start
/// (ADR 0660 D1). No line search anywhere.
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
    name: "S (backtracking + decrease)",
    backtracking: true,
    decrease: true,
    warm: false,
};
/// The remedy W of ADR 0660 reading c3.
const W: Opts = Opts {
    name: "W (warm start + backtracking + decrease)",
    backtracking: true,
    decrease: true,
    warm: true,
};

fn model_of(mesh: SolidMesh, kind: Kind, opts: Opts, material: Material) -> SolidModel {
    let mut m = SolidModel::new(with_formulation(mesh, kind.formulation));
    m.set_incompatible_stabilisation(kind.rule);
    m.set_incompatible_mode_backtracking(opts.backtracking);
    m.set_incompatible_mode_decrease(opts.decrease);
    m.set_incompatible_mode_warm_start(opts.warm);
    m.add_material("all", material).unwrap();
    m
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

// ============================================================================ the J2 blocks (readings c1, c2)

const DELTA: f64 = 0.1;

/// The block mesh on `n^3` with the punch and corner sets; moved at amplitude `amp` with `seed` when `amp > 0`.
fn block_mesh(n: usize, amp: f64, seed: u64) -> (SolidMesh, usize) {
    let mut mesh = hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap();
    let tol = 1e-9;
    let punch = nodes_where(&mesh, "zmax", |p| p[0] <= 0.5 + tol && p[1] <= 0.5 + tol);
    let corner = nodes_where(&mesh, "zmax", |p| (p[0] - 1.0).abs() < tol && (p[1] - 1.0).abs() < tol);
    mesh.add_node_set("punch", punch).unwrap();
    mesh.add_node_set("corner", corner).unwrap();
    if amp > 0.0 {
        distort(&mesh, [1.0, 1.0, 1.0], [n, n, n], amp, seed)
    } else {
        (mesh, 0)
    }
}

fn steps_text(r: &NonlinearResult) -> String {
    let halved = r.steps.iter().filter(|s| s.halvings > 0).count();
    let iterations: Vec<usize> = r.steps.iter().map(|s| s.residuals.len()).collect();
    format!(
        "increments {}, halved {halved}, residuals per increment {iterations:?}",
        r.steps.len()
    )
}

/// The block of ADR 0636 reading c1 with the J2 material and the active-set record; `(R, w_c)` or NaN with the error.
fn run_block(mesh: &SolidMesh, tag: &str, kind: Kind, opts: Opts) -> Run {
    let mut m = model_of(mesh.clone(), kind, opts, j2());
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
    let options = NewtonOptions::new(10).threads(4).record_active_set(true);
    let (result, a, b, error) = match solve(&m, &[], &[], &options) {
        Ok(r) => {
            let punch = m.mesh().node_set("punch").unwrap().to_vec();
            let corner = m.mesh().node_set("corner").unwrap()[0];
            let rz = -reaction_sum(&r, &punch, 2) / MU;
            let wc = r.result.displacement(corner)[2];
            println!(
                "{tag} J2 {} {}: R/mu {rz:.8}, w_c {wc:+.8}, {}",
                kind.name,
                opts.name,
                steps_text(&r)
            );
            (Some(r), rz, wc, None)
        }
        Err(e) => {
            println!("{tag} J2 {} {}: solve failed: {e}", kind.name, opts.name);
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

/// JY: the points with a positive equivalent plastic strain (the material part of each state); NaN without a result.
fn plastic_points(run: &Run) -> f64 {
    let Some(r) = run.result.as_ref() else {
        return f64::NAN;
    };
    r.states
        .iter()
        .map(|s| {
            let v = s.values();
            v[..(POINTS * J2_SIZE).min(v.len())]
                .chunks(J2_SIZE)
                .filter(|p| p[J2_SIZE - 1] > 0.0)
                .count()
        })
        .sum::<usize>() as f64
}

/// AS1: the entries of two active-set records that differ (the increments, the iterations per increment, `yielded` and
/// `set_changed`); NaN if a solve is missing.
fn record_differences(a: &Run, b: &Run) -> f64 {
    let (Some(a), Some(b)) = (a.result.as_ref(), b.result.as_ref()) else {
        return f64::NAN;
    };
    let mut d = a.steps.len().abs_diff(b.steps.len());
    for (x, y) in a.steps.iter().zip(&b.steps) {
        d += x.yielded.len().abs_diff(y.yielded.len());
        d += x.set_changed.len().abs_diff(y.set_changed.len());
        d += x.yielded.iter().zip(&y.yielded).filter(|(p, q)| p != q).count();
        d += x.set_changed.iter().zip(&y.set_changed).filter(|(p, q)| p != q).count();
    }
    d as f64
}

/// AS1c (ADR 0666 A1.1): the entries at the converged iterate of every increment (the last `yielded` entry) that differ,
/// and the difference of the numbers of increments; NaN if a solve is missing or an increment has no record.
fn converged_record_differences(a: &Run, b: &Run) -> f64 {
    let (Some(a), Some(b)) = (a.result.as_ref(), b.result.as_ref()) else {
        return f64::NAN;
    };
    let mut d = a.steps.len().abs_diff(b.steps.len());
    for (x, y) in a.steps.iter().zip(&b.steps) {
        match (x.yielded.last(), y.yielded.last()) {
            (Some(p), Some(q)) => d += usize::from(p != q),
            _ => return f64::NAN,
        }
    }
    d as f64
}

/// AS2: the largest `yielded` entry over the number of material points; NaN without a result or a record.
fn largest_yielded_fraction(run: &Run) -> f64 {
    let Some(r) = run.result.as_ref() else {
        return f64::NAN;
    };
    let most = r.steps.iter().flat_map(|s| s.yielded.iter().copied()).max();
    match most {
        Some(m) => m as f64 / (POINTS * run.model.mesh().n_elements()) as f64,
        None => f64::NAN,
    }
}

/// JL0, JL2, NCA at W's result: (elements whose state is not `104 + 9` values, elements whose warm record from the committed
/// modes with the committed material state has another number of iterates than one or fails, elements whose state read in
/// chunks of 13 has another number of chunks than 8).
fn committed_j2_modes(w: &Run) -> (f64, f64, f64) {
    let Some(r) = w.result.as_ref() else {
        return (f64::NAN, f64::NAN, f64::NAN);
    };
    let mat = j2();
    let ctx = PointContext::default();
    let u = r.result.displacement_values();
    let mesh = w.model.mesh();
    let n = POINTS * J2_SIZE;
    let (mut wrong_length, mut not_one, mut naive) = (0usize, 0usize, 0usize);
    let mut counts = std::collections::BTreeMap::new();
    for (e, st) in r.states.iter().enumerate() {
        let v = st.values();
        if v.len() != n + 9 {
            wrong_length += 1;
        }
        if v.chunks(J2_SIZE).count() != POINTS {
            naive += 1;
        }
        let iterates = if v.len() == n + 9 {
            let start: [f64; 9] = std::array::from_fn(|k| v[n + k]);
            let x = element_coords(mesh, e);
            let ue = element_u(&w.model, u, e);
            match mode_iteration_record_from(
                &x,
                &mat,
                &ue,
                &ElementState::new(v[..n].to_vec()),
                &ctx,
                LIN,
                IncompatibleModeUpdate::BacktrackingWithDecrease,
                Some(&start),
            ) {
                Ok((rec, Ok(_))) => rec.len(),
                _ => usize::MAX,
            }
        } else {
            usize::MAX
        };
        *counts.entry(iterates).or_insert(0usize) += 1;
        if iterates != 1 {
            not_one += 1;
        }
    }
    println!("JL0 iterates of the warm records (count: elements): {counts:?}");
    (wrong_length as f64, not_one as f64, naive as f64)
}

/// The rows of one J2 block: JB, JY, AS2, JL0, JL2 (and AS1, NCA where `binding_record`); the reports.
fn j2_rows(ch: &mut Checks, id: &str, tag: &str, tol: f64, mesh: &SolidMesh, binding_record: bool, met_rows: bool) {
    let f = run_block(mesh, tag, K_FBAR, DEFAULT);
    let w = run_block(mesh, tag, K_L, W);
    let s = run_block(mesh, tag, K_L, S);
    if met_rows {
        // ADR 0666 A1.1: printed, not checked (the known miss of the registered run).
        println!(
            "{id} {tag} W: e_R {:.4e} (registered limit <= {tol}, printed)",
            rel(w.a, f.a)
        );
    } else {
        ch.at_most(&format!("{id} {tag} W: e_R"), rel(w.a, f.a), tol);
    }
    ch.at_most(&format!("{id} {tag} W: e_w"), rel(w.b, f.b), tol);
    ch.above(&format!("JY {tag}: plastic points of W"), plastic_points(&w), 0.0);
    ch.above(&format!("JY {tag}: plastic points of Fbar"), plastic_points(&f), 0.0);
    println!(
        "report JY {tag}: plastic points W {}, S {}, Fbar {} of {}",
        plastic_points(&w),
        plastic_points(&s),
        plastic_points(&f),
        POINTS * mesh.n_elements()
    );
    let differences = record_differences(&w, &s);
    if binding_record && !met_rows {
        ch.at_most(
            &format!("AS1 {tag}: entries of W's active-set record that differ from S's"),
            differences,
            0.0,
        );
    } else {
        println!(
            "report AS1 {tag}: entries of W's active-set record that differ from S's: {differences} (S converged: {}{})",
            s.result.is_some(),
            s.error.as_ref().map(|e| format!(", {e}")).unwrap_or_default()
        );
    }
    if met_rows {
        ch.at_most(
            &format!("AS1c {tag}: converged-iterate entries of W's record that differ from S's"),
            converged_record_differences(&w, &s),
            0.0,
        );
    }
    for (name, run) in [("W", &w), ("S", &s)] {
        if let Some(r) = run.result.as_ref() {
            let yielded: Vec<&Vec<usize>> = r.steps.iter().map(|x| &x.yielded).collect();
            println!("report record {tag} {name}: yielded per increment {yielded:?}");
        }
    }
    ch.at_most(
        &format!("AS2 {tag}: largest yielded entry of W / material points"),
        largest_yielded_fraction(&w),
        1.0,
    );
    let (wrong_length, not_one, naive) = committed_j2_modes(&w);
    ch.at_most(
        &format!("JL0 {tag}: elements whose warm record from the committed modes is not one iterate"),
        not_one,
        0.0,
    );
    ch.at_most(
        &format!("JL2 {tag}: elements whose committed state is not 104 + 9 values"),
        wrong_length,
        0.0,
    );
    if binding_record {
        ch.equals(
            &format!("NCA {tag}: elements whose state in chunks of 13 is not 8 chunks"),
            naive as usize,
            mesh.n_elements(),
        );
    }
    if w.result.is_some() && s.result.is_some() {
        println!(
            "report {tag}: W against S: |R_W / R_S - 1| {:.4e}, |w_W / w_S - 1| {:.4e}",
            rel(w.a, s.a),
            rel(w.b, s.b)
        );
    }
    println!(
        "report {tag}: S e_R {:.4e}, e_w {:.4e} against Fbar",
        rel(s.a, f.a),
        rel(s.b, f.b)
    );
    let (neg, low) = stability(&w);
    println!("report SL {tag} W: negative {neg}, lowest / max diagonal {low:.4e}");
}

fn punch_block(met_rows: bool) {
    let mut ch = Checks::new();
    let (mesh, _) = block_mesh(4, 0.0, 0);
    j2_rows(&mut ch, "JB1", "block 4^3", 0.15, &mesh, true, met_rows);
    ch.finish();
}

fn distorted_block_db4(met_rows: bool) {
    let mut ch = Checks::new();
    let tag = "DB4 (block 8^3 a 0.25)";
    let (mesh, moved) = block_mesh(8, 0.25, 0xF719_0008);
    ch.above(
        &format!("M0 {tag}: min corner det J / centre det J"),
        min_corner_ratio(&mesh),
        0.0,
    );
    ch.equals(&format!("M1 {tag}: moved nodes"), moved, 343);
    j2_rows(&mut ch, "JB2", tag, 0.10, &mesh, false, met_rows);
    ch.finish();
}

#[test]
#[ignore = "ADR 0666 amendment 1: the registered run, known miss: JB1 e_R 0.1675 > 0.15 (R_L 0.08147, R_Fbar 0.09786; S the same), AS1 28 > 0 (the records differ at the first iterates of the increments, equal at the converged iterates)"]
/// JB1, JY, AS1, AS2, JL0, JL2, NCA on the punch block of ADR 0636 (`4^3`) with the J2 material (the registered run).
fn j2_w_on_the_punch_block_known_miss() {
    punch_block(false);
}

#[test]
#[ignore = "ADR 0666 amendment 1: the registered run, known miss: JB2 e_R 0.1219 > 0.10 (R_L 0.06653, R_Fbar 0.07577; S the same)"]
/// M0, M1, JB2, JY, AS2, JL0, JL2 on DB4 of ADR 0649 with the J2 material; AS1 reported (the registered run).
fn j2_w_on_the_distorted_block_db4_known_miss() {
    distorted_block_db4(false);
}

#[test]
/// ADR 0666 A1.1: the rows of `j2_w_on_the_punch_block_known_miss` already met, with JB1 e_R and AS1 printed; AS1c.
fn j2_w_on_the_punch_block_met_rows() {
    punch_block(true);
}

#[test]
/// ADR 0666 A1.1: the rows of `j2_w_on_the_distorted_block_db4_known_miss` already met, with JB2 e_R printed; AS1c.
fn j2_w_on_the_distorted_block_db4_met_rows() {
    distorted_block_db4(true);
}

#[test]
#[ignore = "ADR 0666 A1.2: a diagnosis of JB (no limit), run once"]
/// F-bar and rule L (S) with the J2 material on the regular `4^3` and `8^3` blocks: `R` printed.
fn j2_reaction_against_mesh_diagnosis() {
    for n in [4, 8] {
        let (mesh, _) = block_mesh(n, 0.0, 0);
        let tag = format!("diagnosis block {n}^3");
        let f = run_block(&mesh, &tag, K_FBAR, DEFAULT);
        let l = run_block(&mesh, &tag, K_L, S);
        println!("{tag}: R_L / R_Fbar - 1 = {:+.4e}", l.a / f.a - 1.0);
    }
}

// ============================================================================ the transient cantilever (reading c3)

const TIP_FORCE: f64 = 200.0;
const DT: f64 = 0.02;
const STEPS: u64 = 50;
const ALPHA_R: f64 = 22.16;

/// The integrator of reading c3, started from rest.
fn transient(kind: Kind, opts: Opts) -> (SolidModel, Integrator) {
    let mesh = hex_box([0.0; 3], [1.0, 0.2, 0.2], [10, 2, 2]).unwrap();
    let mut m = model_of(mesh, kind, opts, neo_hookean());
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let n = m.dof_map().n_equations();
    let tip = m.mesh().node_set("xmax").unwrap().to_vec();
    let mut pattern = vec![0.0; n];
    for &nd in &tip {
        pattern[m.dof_map().equation(nd, 1).unwrap()] = TIP_FORCE / tip.len() as f64;
    }
    let table = LoadTable::new(vec![(0.0, 0.0), (0.5, 1.0), (1.0, 1.0)]).unwrap();
    let loads = [LoadHistory { pattern, table }];
    let options = NlTransientOptions::new(DT)
        .damping(Rayleigh {
            mass: ALPHA_R,
            stiffness: 0.0,
        })
        .threads(4);
    let mut it = Integrator::new(&m, &loads, &options).unwrap();
    it.start(0.0, &vec![0.0; n], &vec![0.0; n]).unwrap();
    (m, it)
}

/// `(d, W_s)` at the integrator's state: the mean tip `u_y` and the strain energy; NaN where it fails.
fn transient_measures(m: &SolidModel, it: &Integrator) -> (f64, f64) {
    let Ok(u) = it.displacement() else {
        return (f64::NAN, f64::NAN);
    };
    let tip = m.mesh().node_set("xmax").unwrap();
    let d = tip
        .iter()
        .map(|&nd| u[m.dof_map().equation(nd, 1).unwrap()])
        .sum::<f64>()
        / tip.len() as f64;
    let w = it.strain_energy().unwrap_or(f64::NAN);
    (d, w)
}

fn bits_of(a: &[f64], b: &[f64]) -> usize {
    if a.len() != b.len() {
        return usize::MAX;
    }
    a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count()
}

/// The test's own energy of reading c3 (TE2) at the committed modes of every element.
fn own_energy(m: &SolidModel, u: &[f64], states: &[ElementState]) -> f64 {
    let mat = neo_hookean();
    let energy = energy_of();
    let ctx = PointContext::default();
    let el = element_for(ElementKind::Hex8);
    let mut w = 0.0;
    for (e, st) in states.iter().enumerate() {
        let v = st.values();
        if v.len() != 9 {
            return f64::NAN;
        }
        let modes: [f64; 9] = std::array::from_fn(|k| v[k]);
        let x = element_coords(m.mesh(), e);
        let ue = element_u(m, u, e);
        let grads = point_gradients(&x, &ue, &modes).unwrap();
        let ips = el.integration_points(&x, el.stiffness_rule()).unwrap();
        for (ip, (hc, fm)) in ips.iter().zip(&grads) {
            let h: [[f64; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| hc[i][j] + fm[i][j]));
            w += ip.weight * rcfd_fem::mat_hyper::energy(&energy, &voigt_green_lagrange(&h)).unwrap();
        }
        w += stabilisation_energy_with(&x, &mat, &ue, &modes, &ctx, LIN).unwrap();
    }
    w
}

fn voigt_green_lagrange(a: &[[f64; 3]; 3]) -> [f64; 6] {
    let e = |i: usize, j: usize| a[i][j] + a[j][i] + a[0][i] * a[0][j] + a[1][i] * a[1][j] + a[2][i] * a[2][j];
    [0.5 * e(0, 0), 0.5 * e(1, 1), 0.5 * e(2, 2), e(0, 1), e(1, 2), e(2, 0)]
}

#[test]
/// TT1, TT2, TE1, TE2, NCE, TS1, NCR on the transient cantilever of reading c3.
fn w_on_the_transient_cantilever() {
    let mut ch = Checks::new();
    let (mf, mut f) = transient(K_FBAR, DEFAULT);
    let fr = f.run(STEPS);
    println!("transient Fbar: {}", if fr.is_ok() { "ran" } else { "failed" });
    let (fd, fw) = if fr.is_ok() {
        transient_measures(&mf, &f)
    } else {
        (f64::NAN, f64::NAN)
    };
    let (ml, mut l) = transient(K_L, DEFAULT);
    let lr = l.run(STEPS);
    let (ld, lw) = if lr.is_ok() {
        transient_measures(&ml, &l)
    } else {
        (f64::NAN, f64::NAN)
    };
    // W: 25 steps, the state through bytes, one step, the restore, the same step, then the rest (TS1).
    let (mw, mut w) = transient(K_L, W);
    let mut ran = w.run(25).is_ok();
    let mut ts1 = f64::NAN;
    let mut restored = false;
    let mut saved = None;
    if ran {
        let s = w.state().unwrap();
        let bytes = s.to_bytes();
        saved = Some(s);
        ran = w.run(1).is_ok();
        if ran {
            let first = w.state().unwrap();
            match NlTransientState::from_bytes(&bytes).and_then(|back| w.restore(back)) {
                Ok(()) => {
                    restored = true;
                    ran = w.run(1).is_ok();
                    if ran {
                        let again = w.state().unwrap();
                        ts1 = (bits_of(&first.u, &again.u) + bits_of(&first.v, &again.v) + bits_of(&first.a, &again.a))
                            as f64;
                    }
                }
                Err(e) => println!("TS1: restore refused: {e}"),
            }
        }
        if ran {
            ran = w.run(STEPS - 26).is_ok();
        }
    }
    ch.holds("TS1 W: the restore of the state through bytes succeeds", restored);
    ch.at_most(
        "TS1 W: bit differences of u, v, a of the step after the restore",
        ts1,
        0.0,
    );
    let (wd, ww) = if ran {
        transient_measures(&mw, &w)
    } else {
        (f64::NAN, f64::NAN)
    };
    println!("transient at t = 1: Fbar d {fd:.8e}, W_s {fw:.8e}; W d {wd:.8e}, W_s {ww:.8e}; default L d {ld:.8e}, W_s {lw:.8e}");
    ch.at_most("TT1 transient W: e_d", rel(wd, fd), 0.15);
    ch.at_most("TT1 transient W: e_W", rel(ww, fw), 0.15);
    println!(
        "report transient default L against W: |d_L / d_W - 1| {:.4e}, |W_L / W_W - 1| {:.4e}",
        rel(ld, wd),
        rel(lw, ww)
    );
    // TT2, TE1, TE2, NCE at t = 1.
    let state = if ran { w.state().ok() } else { None };
    let (mut tt2, mut te1, mut nce, mut te2) = (f64::NAN, f64::NAN, f64::NAN, f64::NAN);
    if let Some(st) = state.as_ref() {
        tt2 = st.states.iter().filter(|s| s.values().len() != 9).count() as f64;
        let record = w.strain_energy_modes().unwrap();
        let mut differ = 0usize;
        let mut cold_differ = 0usize;
        let mut cold_worst = 0.0f64;
        let mat = neo_hookean();
        let ctx = PointContext::default();
        for (e, (s, r)) in st.states.iter().zip(&record).enumerate() {
            let v = s.values();
            let committed: Option<[f64; 9]> = (v.len() == 9).then(|| std::array::from_fn(|k| v[k]));
            let same = match (committed, r) {
                (Some(c), Some(r)) => bits_of(&c, r) == 0,
                _ => false,
            };
            if !same {
                differ += 1;
            }
            let x = element_coords(mw.mesh(), e);
            let ue = element_u(&mw, &st.u, e);
            let cold = mode_iteration_record_from(
                &x,
                &mat,
                &ue,
                &ElementState::new(Vec::new()),
                &ctx,
                LIN,
                IncompatibleModeUpdate::BacktrackingWithDecrease,
                None,
            );
            match (committed, cold) {
                (Some(c), Ok((_, Ok(a)))) => {
                    if bits_of(&c, &a) != 0 {
                        cold_differ += 1;
                    }
                    let scale = c.iter().fold(0.0f64, |m, x| m.max(x.abs()));
                    let diff = c.iter().zip(&a).fold(0.0f64, |m, (x, y)| m.max((x - y).abs()));
                    if scale > 0.0 {
                        cold_worst = cold_worst.max(diff / scale);
                    }
                }
                _ => cold_differ += 1,
            }
        }
        te1 = differ as f64;
        nce = cold_differ as f64;
        println!("report NCE: largest |alpha_cold - alpha_committed|_inf / |alpha_committed|_inf {cold_worst:.4e}");
        let own = own_energy(&mw, &st.u, &st.states);
        te2 = (ww - own).abs() / own.abs();
        println!("TE2: strain_energy {ww:.15e}, own energy at the committed modes {own:.15e}");
    }
    ch.at_most(
        "TT2 transient W: elements whose committed state is not 9 values",
        tt2,
        0.0,
    );
    ch.at_most(
        "TE1 transient W: elements whose energy-record modes differ in a bit from the committed modes",
        te1,
        0.0,
    );
    ch.at_most(
        "TE2 transient W: strain_energy against the own energy, relative",
        te2,
        1e-10,
    );
    ch.above(
        "NCE transient W: elements whose cold record differs in a bit from the committed modes",
        nce,
        0.0,
    );
    // NCR: the default integrator refuses W's extended state.
    let ncr = match saved {
        Some(s) => {
            let refused = l.restore(s).is_err();
            println!(
                "NCR: the default integrator {} W's state",
                if refused { "refuses" } else { "accepts" }
            );
            refused
        }
        None => false,
    };
    ch.holds("NCR the default integrator refuses W's state with modes", ncr);
    ch.finish();
}

// ============================================================================ the rotating path (reading c9)

fn rotating_frequencies(opts: Opts) -> Vec<f64> {
    let mesh = hex_box([0.0; 3], [1.0, 0.2, 0.2], [10, 2, 2]).unwrap();
    let mut m = model_of(mesh, K_L, opts, elastic());
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let part = RotatingPart::new(&m, [0.0, 0.0, 1.0], [0.0; 3], &RotatingOptions::new(4)).unwrap();
    let mut out = Vec::new();
    for omega in [0.0, 11.08] {
        match part.modes(omega).unwrap() {
            SpeedModes::Modes(r) => out.extend(r.frequencies),
            SpeedModes::LossOfStiffness(l) => panic!("unexpected loss of stiffness {l:?}"),
        }
    }
    out
}

#[test]
/// RO1: the rotating path with W's opt-ins against none.
fn rotating_path_with_w_inert() {
    let mut ch = Checks::new();
    let a = rotating_frequencies(DEFAULT);
    let b = rotating_frequencies(W);
    println!("RO1 frequencies, default {a:?}, W {b:?}");
    ch.equals("RO1 frequencies", b.len(), 8);
    ch.at_most(
        "RO1 bit differences of the frequencies at 0 and 11.08 rad/s",
        match bits_of(&a, &b) {
            usize::MAX => f64::NAN,
            n => n as f64,
        },
        0.0,
    );
    ch.above("RO1 lowest frequency (finite, positive)", least(b.iter().copied()), 0.0);
    ch.finish();
}
