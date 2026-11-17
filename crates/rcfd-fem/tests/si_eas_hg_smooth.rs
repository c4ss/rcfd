//! F-762 (ADR 0683): a J2 punch reference without the edge jump. The block of ADR 0636 with the J2 material of ADR 0666
//! under a smoothed dead pressure (the quintic step over the registered width `0.5`, two `4^3` element widths) on `4^3`,
//! `8^3` and `16^3`, rule L of ADR 0630 (with S of ADR 0658) and the F-bar Hex8 of ADR 0506, the Full Hex8 as the negative
//! control and the Hex20 on `8^3`: the three-mesh reading of ADR 0678 (amendment 1) of the load-weighted settlement `S` and
//! the corner bulge `w_c` (RU), the J2 tolerance derived from it (TJ, CJ, DB) and its separation of the Full Hex8 (NC1a to
//! NC1c), the checks of the load (LS1 to LS3, SM), the reading on synthetic sequences (NC2), and the classification X1
//! (whether F-bar's edge excess of ADR 0678 disappears). Every aggregated value is required finite before it is compared; a
//! failed solve is a non-finite measure. The helpers are copies of those of `si_eas_hg11.rs` (existing tests are not edited).

use rcfd_core::vec3::{cross, norm, Vec3};
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::loads::{face_shape_functions, nodal_load_vector, NodalForceAt};
use rcfd_fem::nl_incompatible::IncompatibleStabilisation;
use rcfd_fem::nonlinear::{solve, NewtonOptions, NonlinearResult};
use rcfd_fem::solve::DirectFactor;
use rcfd_fem::{BlockProperty, Constraint, Load, Material, SolidFormulation, SolidMesh, SolidModel};
use std::collections::{BTreeMap, BTreeSet};
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

// ============================================================================ the distorted mesh (ADR 0649 D2)

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
const FULL: SolidFormulation = SolidFormulation::Full;
const OFF: IncompatibleStabilisation = IncompatibleStabilisation::Off;
const LIN: IncompatibleStabilisation = IncompatibleStabilisation::LinearStress;
/// Young's modulus of the shear modulus `MU` at `NU` (ADR 0678 c1).
const E_MOD: f64 = 2.0 * MU * (1.0 + NU);
/// The J2 constants of ADR 0678 c1.
const SIGMA_Y: f64 = 6.0e4;
const H_ISO: f64 = 3.0e5;
/// Values per point of the J2 state, its equivalent plastic strain.
const J2_SIZE: usize = rcfd_fem::mat_j2::STATE_SIZE;

/// The J2 material of ADR 0678 c1.
fn j2() -> Material {
    Material::j2(
        Material::isotropic_elastic(E_MOD, NU, 1000.0, 0.0).unwrap(),
        SIGMA_Y,
        H_ISO,
        0.0,
    )
    .unwrap()
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
const K_FULL: Kind = Kind {
    name: "Full",
    formulation: FULL,
    rule: OFF,
};
/// The Hex20 (27 points, `Full`; the mesh is the upgraded one).
const K_HEX20: Kind = Kind {
    name: "Hex20",
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
/// The remedy W of ADR 0660, the fallback.
const W: Opts = Opts {
    name: "W",
    backtracking: true,
    decrease: true,
    warm: true,
};

const SUPERNODAL: DirectFactor = DirectFactor::SupernodalNestedDissection;
const ENVELOPE: DirectFactor = DirectFactor::Envelope;

fn model_of(mesh: SolidMesh, kind: Kind, opts: Opts) -> SolidModel {
    let mut m = SolidModel::new(with_formulation(mesh, kind.formulation));
    m.set_incompatible_stabilisation(kind.rule);
    m.set_incompatible_mode_backtracking(opts.backtracking);
    m.set_incompatible_mode_decrease(opts.decrease);
    m.set_incompatible_mode_warm_start(opts.warm);
    m.add_material("all", j2()).unwrap();
    m
}

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

// ============================================================================ the smoothed pressure (reading c1)

/// The peak pressure `p0` (arithmetic 3).
const P0: f64 = 1.5e5;
const TOL: f64 = 1e-9;
/// The first moment `int x g dx` of the profile (arithmetic 1).
const M1: f64 = 29.0 / 224.0;

/// The quintic smooth step `6 s^5 - 15 s^4 + 10 s^3`.
fn smooth_step(s: f64) -> f64 {
    s * s * s * (10.0 + s * (-15.0 + 6.0 * s))
}

/// `g(t)`: `1` up to `0.25`, `1 - S((t - 0.25) / 0.5)` to `0.75`, `0` beyond (reading c1; width `0.5`).
fn profile(t: f64) -> f64 {
    if t <= 0.25 {
        1.0
    } else if t >= 0.75 {
        0.0
    } else {
        1.0 - smooth_step((t - 0.25) / 0.5)
    }
}

/// The 4-point Gauss-Legendre rule on `[-1, 1]` (exact to degree 7).
const GAUSS4_X: [f64; 4] = [
    -0.861_136_311_594_052_6,
    -0.339_981_043_584_856_3,
    0.339_981_043_584_856_3,
    0.861_136_311_594_052_6,
];
const GAUSS4_W: [f64; 4] = [
    0.347_854_845_137_453_8,
    0.652_145_154_862_546_1,
    0.652_145_154_862_546_1,
    0.347_854_845_137_453_8,
];

/// The consistent magnitudes `F_a = int N_a p0 q(x, y) dA` over the reference `zmax` faces (pressing in `-z`), by node.
fn pressure_forces(mesh: &SolidMesh, p0: f64, q: impl Fn(f64, f64) -> f64) -> BTreeMap<usize, f64> {
    let mut out = BTreeMap::new();
    for &face in mesh.surface_set("zmax").unwrap() {
        let nodes = mesh.face_nodes(face);
        let x: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
        let mut fe = vec![0.0; nodes.len()];
        for (j, wj) in GAUSS4_X.iter().zip(GAUSS4_W) {
            for (i, wi) in GAUSS4_X.iter().zip(GAUSS4_W) {
                let (n, d) = face_shape_functions(nodes.len(), [*i, *j]).unwrap();
                let mut xp = [0.0; 3];
                let mut xu = [0.0; 3];
                let mut xv = [0.0; 3];
                for ((xa, na), da) in x.iter().zip(&n).zip(&d) {
                    for c in 0..3 {
                        xp[c] += xa[c] * na;
                        xu[c] += xa[c] * da[0];
                        xv[c] += xa[c] * da[1];
                    }
                }
                let ja = norm(cross(xu, xv));
                let t = p0 * q(xp[0], xp[1]);
                for (fa, na) in fe.iter_mut().zip(&n) {
                    *fa += wi * wj * ja * t * na;
                }
            }
        }
        for (&node, fa) in nodes.iter().zip(&fe) {
            *out.entry(node).or_insert(0.0) += fa;
        }
    }
    out
}

fn smooth_forces(mesh: &SolidMesh) -> BTreeMap<usize, f64> {
    pressure_forces(mesh, P0, |x, y| profile(x) * profile(y))
}

/// `S = sum_a F_a (-u_z,a) / sum_a F_a` over the nodes of `keep`, divided by the total.
fn settlement_part(forces: &BTreeMap<usize, f64>, uz: impl Fn(usize) -> f64, keep: impl Fn(usize) -> bool) -> f64 {
    let total: f64 = forces.values().sum();
    forces
        .iter()
        .filter(|(&n, _)| keep(n))
        .map(|(&n, &f)| f * -uz(n))
        .sum::<f64>()
        / total
}

fn settlement(forces: &BTreeMap<usize, f64>, uz: impl Fn(usize) -> f64) -> f64 {
    settlement_part(forces, uz, |_| true)
}

/// The block mesh on `n^3`; moved at amplitude `amp` with `seed` when `amp > 0`.
fn block_mesh(n: usize, amp: f64, seed: u64) -> (SolidMesh, usize) {
    let mesh = hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap();
    if amp > 0.0 {
        distort(&mesh, [1.0, 1.0, 1.0], [n, n, n], amp, seed)
    } else {
        (mesh, 0)
    }
}

/// The Hex20 block (reading c2): the regular `n^3` block upgraded.
fn hex20_mesh(n: usize) -> SolidMesh {
    upgrade_to_hex20(&hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap())
        .unwrap()
        .0
}

/// DB4 of ADR 0649 D2.
fn db4_mesh() -> (SolidMesh, usize) {
    block_mesh(8, 0.25, 0xF719_0008)
}

/// The node at a point (within `TOL`).
fn node_at(mesh: &SolidMesh, p: Vec3) -> usize {
    mesh.nodes()
        .iter()
        .position(|q| (0..3).all(|c| (q[c] - p[c]).abs() < TOL))
        .unwrap()
}

/// A solve of the block: `S`, `w_c`, the band part `S_b`, the centre settlement; NaN on failure.
struct Run {
    result: Option<NonlinearResult>,
    s: f64,
    w: f64,
    s_band: f64,
    centre: f64,
    opts: &'static str,
    secs: f64,
}

fn steps_text(r: &NonlinearResult) -> String {
    let halved = r.steps.iter().filter(|s| s.halvings > 0).count();
    let iterations: Vec<usize> = r.steps.iter().map(|s| s.residuals.len()).collect();
    format!(
        "increments {}, halved {halved}, residuals per increment {iterations:?}",
        r.steps.len()
    )
}

fn solve_smooth(mesh: &SolidMesh, tag: &str, kind: Kind, opts: Opts, factor: DirectFactor) -> Run {
    let forces = smooth_forces(mesh);
    let mut m = model_of(mesh.clone(), kind, opts);
    for (set, comp) in [("xmin", 0), ("xmax", 0), ("ymin", 1), ("ymax", 1), ("zmin", 2)] {
        m.add_constraint(Constraint::displacement(set, comp, 0.0)).unwrap();
    }
    let loads: Vec<NodalForceAt> = forces
        .iter()
        .map(|(&n, &f)| NodalForceAt::new(n, [0.0, 0.0, -f]))
        .collect();
    let options = NewtonOptions::new(10).threads(4).record_active_set(true).factor(factor);
    let corner = node_at(mesh, [1.0, 1.0, 1.0]);
    let origin = node_at(mesh, [0.0, 0.0, 1.0]);
    let t0 = Instant::now();
    let out = solve(&m, &[], &loads, &options);
    let secs = t0.elapsed().as_secs_f64();
    match out {
        Ok(r) => {
            let uz = |n: usize| r.result.displacement(n)[2];
            let s = settlement(&forces, uz);
            let s_band = settlement_part(&forces, uz, |n| {
                let p = mesh.nodes()[n];
                p[0].max(p[1]) > 0.25 + TOL
            });
            let w = uz(corner);
            let centre = -uz(origin);
            println!(
                "{tag} J2 {} {} {factor:?}: S {s:.8}, w_c {w:+.8}, S_band {s_band:.8}, centre {centre:.8}, {}, {secs:.1} s",
                kind.name,
                opts.name,
                steps_text(&r)
            );
            Run {
                result: Some(r),
                s,
                w,
                s_band,
                centre,
                opts: opts.name,
                secs,
            }
        }
        Err(e) => {
            println!(
                "{tag} J2 {} {} {factor:?}: solve failed: {e}, {secs:.1} s",
                kind.name, opts.name
            );
            Run {
                result: None,
                s: f64::NAN,
                w: f64::NAN,
                s_band: f64::NAN,
                centre: f64::NAN,
                opts: opts.name,
                secs,
            }
        }
    }
}

/// Rule L with S, and with W once if S fails (reading c1).
fn solve_l(mesh: &SolidMesh, tag: &str, factor: DirectFactor) -> Run {
    let s = solve_smooth(mesh, tag, K_L, S, factor);
    if s.result.is_some() {
        return s;
    }
    println!("{tag}: S failed, W run once (reading c1)");
    solve_smooth(mesh, tag, K_L, W, factor)
}

/// JY: the points with a positive equivalent plastic strain (every J2 chunk of each state); NaN without a result.
fn plastic_points(run: &Run) -> f64 {
    let Some(r) = run.result.as_ref() else {
        return f64::NAN;
    };
    r.states
        .iter()
        .map(|s| {
            s.values()
                .chunks_exact(J2_SIZE)
                .filter(|p| p[J2_SIZE - 1] > 0.0)
                .count()
        })
        .sum::<usize>() as f64
}

// ============================================================================ the reading (ADR 0678 c4, amendment 1)

#[derive(Debug, Clone, Copy)]
struct Reading {
    asymptotic: bool,
    p: f64,
    a_inf: f64,
    u: f64,
}

/// The reading of ADR 0678 amendment 1: the Richardson limit `a16 - d2 / (2^p - 1)` (`d2 = a8 - a16`).
fn reading(a4: f64, a8: f64, a16: f64) -> Reading {
    if !(a4.is_finite() && a8.is_finite() && a16.is_finite()) {
        return Reading {
            asymptotic: false,
            p: f64::NAN,
            a_inf: f64::NAN,
            u: f64::NAN,
        };
    }
    let d1 = a4 - a8;
    let d2 = a8 - a16;
    if d1 * d2 > 0.0 {
        let p = (d1 / d2).log2();
        if (0.5..=4.0).contains(&p) {
            let g = 2f64.powf(p) - 1.0;
            return Reading {
                asymptotic: true,
                p,
                a_inf: a16 - d2 / g,
                u: 1.25 * d2.abs() / g,
            };
        }
        return Reading {
            asymptotic: false,
            p,
            a_inf: a16,
            u: d2.abs() / (2f64.sqrt() - 1.0),
        };
    }
    Reading {
        asymptotic: false,
        p: f64::NAN,
        a_inf: a16,
        u: d1.abs().max(d2.abs()),
    }
}

/// `E_X(n)`: the error of a value against its element's limit, with the uncertainty.
fn error_at(a: f64, r: &Reading) -> f64 {
    ((a - r.a_inf).abs() + r.u) / r.a_inf.abs()
}

/// `T(n) = 1.25 (E_L + E_Fbar)`, rounded up to two decimals.
fn tolerance(e_l: f64, e_f: f64) -> f64 {
    (125.0 * (e_l + e_f)).ceil() / 100.0
}

/// G1: the two limits within the sum of their uncertainties.
fn same_limit(a: &Reading, b: &Reading) -> bool {
    (a.a_inf - b.a_inf).abs() <= a.u + b.u
}

fn reading_text(r: &Reading) -> String {
    format!(
        "{}, p {:.4}, a_inf {:.8}, U {:.4e} (U / |a_inf| {:.4})",
        if r.asymptotic {
            "asymptotic"
        } else {
            "NOT asymptotic (bound)"
        },
        r.p,
        r.a_inf,
        r.u,
        r.u / r.a_inf.abs()
    )
}

// ============================================================================ the fast test

fn nc2_rows(ch: &mut Checks) {
    let f = |h: f64| 1.0 + 0.3 * h.powf(1.5);
    let (a4, a8, a16) = (f(0.25), f(0.125), f(0.0625));
    let r = reading(a4, a8, a16);
    let u = 1.25 * (a8 - a16).abs() / (2f64.powf(1.5) - 1.0);
    ch.holds("NC2 (i) 1 + 0.3 h^1.5: asymptotic", r.asymptotic);
    ch.at_most("NC2 (i): |p - 1.5|", (r.p - 1.5).abs(), 1e-10);
    ch.at_most("NC2 (i): |a_inf - 1|", (r.a_inf - 1.0).abs(), 1e-10);
    ch.at_most("NC2 (i): |U - 1.25 |d2| / (2^1.5 - 1)|", (r.u - u).abs(), 1e-10);
    let r = reading(1.0, 0.9, 0.95);
    ch.holds("NC2 (ii) (1, 0.9, 0.95): not asymptotic", !r.asymptotic);
    ch.at_most("NC2 (ii): |a_inf - 0.95|", (r.a_inf - 0.95).abs(), 1e-12);
    ch.at_most("NC2 (ii): |U - 0.1|", (r.u - 0.1).abs(), 1e-12);
    let r = reading(1.0, 0.5, 0.49);
    println!("NC2 (iii) (1, 0.5, 0.49): {}", reading_text(&r));
    ch.holds("NC2 (iii) (1, 0.5, 0.49): not asymptotic", !r.asymptotic);
    ch.at_most(
        "NC2 (iii): |U - 0.01 / (2^0.5 - 1)|",
        (r.u - 0.01 / (2f64.sqrt() - 1.0)).abs(),
        1e-12,
    );
}

/// LS1, LS2 on one mesh.
fn load_rows(ch: &mut Checks, tag: &str, mesh: &SolidMesh) {
    let f = smooth_forces(mesh);
    let total: f64 = f.values().sum();
    let mx: f64 = f.iter().map(|(&n, &v)| v * mesh.nodes()[n][0]).sum();
    let my: f64 = f.iter().map(|(&n, &v)| v * mesh.nodes()[n][1]).sum();
    ch.at_most(
        &format!("LS1 {tag}: sum F_a against p0 / 4"),
        rel(total, P0 / 4.0),
        1e-12,
    );
    ch.at_most(
        &format!("LS2 {tag}: sum F_a x_a against 29 p0 / 448"),
        rel(mx, P0 * M1 / 2.0),
        1e-12,
    );
    ch.at_most(
        &format!("LS2 {tag}: sum F_a y_a against 29 p0 / 448"),
        rel(my, P0 * M1 / 2.0),
        1e-12,
    );
}

/// LS3: the forces of a uniform profile against the library's consistent loads of `Load::pressure`.
fn uniform_against_library(mesh: &SolidMesh) -> f64 {
    let mine = pressure_forces(mesh, P0, |_, _| 1.0);
    let mut m = SolidModel::new(mesh.clone());
    m.add_material("all", j2()).unwrap();
    m.add_load(Load::pressure("zmax", P0)).unwrap();
    let lib = nodal_load_vector(&m, &[]).unwrap();
    let dofs = m.dof_map();
    let mut largest_force = 0.0f64;
    let mut largest_diff = 0.0f64;
    for n in 0..mesh.nodes().len() {
        let fz = lib[dofs.equation(n, 2).unwrap()];
        let a = -mine.get(&n).copied().unwrap_or(0.0);
        largest_force = largest_force.max(fz.abs());
        largest_diff = largest_diff.max((fz - a).abs());
        for c in 0..2 {
            largest_diff = largest_diff.max(lib[dofs.equation(n, c).unwrap()].abs());
        }
    }
    largest_diff / largest_force
}

#[test]
/// NC2; LS1, LS2, LS3, SM; SG, JY on `4^3`, `8^3` and DB4; FC on `4^3`; M0, M1.
fn j2_smooth_reference_setup() {
    let mut ch = Checks::new();
    nc2_rows(&mut ch);
    for n in [4, 8, 16] {
        load_rows(&mut ch, &format!("Hex8 {n}^3"), &block_mesh(n, 0.0, 0).0);
    }
    let h20 = hex20_mesh(8);
    load_rows(&mut ch, "Hex20 8^3", &h20);
    ch.at_most(
        "LS3 Hex8 4^3: uniform forces against Load::pressure",
        uniform_against_library(&block_mesh(4, 0.0, 0).0),
        1e-12,
    );
    ch.at_most(
        "LS3 Hex20 8^3: uniform forces against Load::pressure",
        uniform_against_library(&h20),
        1e-12,
    );
    let (m4, _) = block_mesh(4, 0.0, 0);
    let f4 = smooth_forces(&m4);
    let sm = settlement(&f4, |n| -(1.0 + m4.nodes()[n][0]));
    ch.at_most(
        "SM 4^3: S of u_z = -(1 + x) against 1 + 29/112",
        (sm - (1.0 + 29.0 / 112.0)).abs(),
        1e-12,
    );
    for n in [4, 8] {
        let (mesh, _) = block_mesh(n, 0.0, 0);
        let tag = format!("block {n}^3");
        let f = solve_smooth(&mesh, &tag, K_FBAR, DEFAULT, SUPERNODAL);
        let l = solve_l(&mesh, &tag, SUPERNODAL);
        sg_jy_rows(&mut ch, &tag, &f, &l);
        if n == 4 {
            let fe = solve_smooth(&mesh, &tag, K_FBAR, DEFAULT, ENVELOPE);
            let le = solve_smooth(&mesh, &tag, K_L, S, ENVELOPE);
            ch.at_most(&format!("FC {tag} Fbar: S supernodal / envelope"), rel(f.s, fe.s), 1e-9);
            ch.at_most(
                &format!("FC {tag} Fbar: w_c supernodal / envelope"),
                rel(f.w, fe.w),
                1e-9,
            );
            ch.at_most(&format!("FC {tag} L: S supernodal / envelope"), rel(l.s, le.s), 1e-9);
            ch.at_most(&format!("FC {tag} L: w_c supernodal / envelope"), rel(l.w, le.w), 1e-9);
        }
    }
    let tag = "DB4 (block 8^3 a 0.25)";
    let (mesh, moved) = db4_mesh();
    ch.above(
        &format!("M0 {tag}: min corner det J / centre det J"),
        min_corner_ratio(&mesh),
        0.0,
    );
    ch.equals(&format!("M1 {tag}: moved nodes"), moved, 343);
    let f = solve_smooth(&mesh, tag, K_FBAR, DEFAULT, SUPERNODAL);
    let l = solve_l(&mesh, tag, SUPERNODAL);
    sg_jy_rows(&mut ch, tag, &f, &l);
    ch.finish();
}

/// SG and JY for the F-bar and rule L solves of one mesh.
fn sg_jy_rows(ch: &mut Checks, tag: &str, f: &Run, l: &Run) {
    for (name, r) in [("Fbar".to_string(), f), (format!("L ({})", l.opts), l)] {
        ch.above(&format!("SG {tag} {name}: S"), r.s, 0.0);
        ch.above(&format!("SG {tag} {name}: w_c"), r.w, 0.0);
        ch.above(&format!("JY {tag} {name}: plastic points"), plastic_points(r), 0.0);
    }
}

// ============================================================================ the heavy record (c1 to c5)

/// One element's three regular runs.
struct Series {
    name: &'static str,
    runs: Vec<Run>,
}

impl Series {
    fn s(&self) -> Reading {
        reading(self.runs[0].s, self.runs[1].s, self.runs[2].s)
    }
    fn w(&self) -> Reading {
        reading(self.runs[0].w, self.runs[1].w, self.runs[2].w)
    }
}

const NS: [usize; 3] = [4, 8, 16];

fn series(name: &'static str, mut each: impl FnMut(&SolidMesh, &str) -> Run) -> Series {
    let runs = NS
        .iter()
        .map(|&n| {
            let (mesh, _) = block_mesh(n, 0.0, 0);
            each(&mesh, &format!("block {n}^3"))
        })
        .collect();
    Series { name, runs }
}

fn report_series(s: &Series, reference: (&Reading, &Reading), fbar16: &Run) {
    let (rs, rw) = (s.s(), s.w());
    println!("report reading {} S: {}", s.name, reading_text(&rs));
    println!("report reading {} w_c: {}", s.name, reading_text(&rw));
    for (k, run) in s.runs.iter().enumerate() {
        println!(
            "report {} {}^3: S / S_ref - 1 {:+.4}, w_c / w_ref - 1 {:+.4}; against Fbar 16^3 {:+.4}, {:+.4}; d_S {:.4}, d_w {:.4}; \
             E_S {:.4}, E_w {:.4}, per-element tolerance 1.25 E: {:.4}, {:.4}; centre {:.6}; {:.1} s",
            s.name,
            NS[k],
            run.s / reference.0.a_inf - 1.0,
            run.w / reference.1.a_inf - 1.0,
            run.s / fbar16.s - 1.0,
            run.w / fbar16.w - 1.0,
            rel(run.s, rs.a_inf),
            rel(run.w, rw.a_inf),
            error_at(run.s, &rs),
            error_at(run.w, &rw),
            1.25 * error_at(run.s, &rs),
            1.25 * error_at(run.w, &rw),
            run.centre,
            run.secs,
        );
    }
}

#[test]
#[ignore = "ADR 0683: the heavy record (three 16^3 solves, the Hex20 8^3), run once; the fast test j2_smooth_reference_setup covers the same path"]
/// RC0, SG, JY, RU, TJ, CJ, DB, NC1a, NC1b, NC1c and the reports of c2, c3, c5.
fn j2_smooth_reference_heavy_record() {
    let mut ch = Checks::new();
    let t0 = Instant::now();
    let fbar = series("J2 Fbar", |m, t| solve_smooth(m, t, K_FBAR, DEFAULT, SUPERNODAL));
    let l = series("J2 L", |m, t| solve_l(m, t, SUPERNODAL));
    let full = series("J2 Full", |m, t| solve_smooth(m, t, K_FULL, DEFAULT, SUPERNODAL));
    let hex20 = solve_smooth(&hex20_mesh(8), "Hex20 block 8^3", K_HEX20, DEFAULT, SUPERNODAL);
    let (db, _) = db4_mesh();
    let db_f = solve_smooth(&db, "DB4", K_FBAR, DEFAULT, SUPERNODAL);
    let db_l = solve_l(&db, "DB4", SUPERNODAL);
    println!("all solves: {:.1} s", t0.elapsed().as_secs_f64());

    // RC0, SG, JY.
    ch.holds(
        "RC0: the 16^3 J2 solves of Fbar and L succeed",
        fbar.runs[2].s.is_finite() && fbar.runs[2].w.is_finite() && l.runs[2].s.is_finite() && l.runs[2].w.is_finite(),
    );
    for (k, n) in NS.iter().enumerate() {
        sg_jy_rows(&mut ch, &format!("block {n}^3"), &fbar.runs[k], &l.runs[k]);
        println!(
            "report JY block {n}^3: plastic points Fbar {}, L {}, Full {} of {}",
            plastic_points(&fbar.runs[k]),
            plastic_points(&l.runs[k]),
            plastic_points(&full.runs[k]),
            8 * n * n * n
        );
    }
    sg_jy_rows(&mut ch, "DB4", &db_f, &db_l);

    // RU: the readings.
    let (fs, fw, ls, lw) = (fbar.s(), fbar.w(), l.s(), l.w());
    ch.at_most("RU J2 Fbar S: U / |a_inf|", fs.u / fs.a_inf.abs(), 0.03);
    ch.at_most("RU J2 L S: U / |a_inf|", ls.u / ls.a_inf.abs(), 0.03);
    ch.at_most("RU J2 Fbar w_c: U / |a_inf|", fw.u / fw.a_inf.abs(), 0.05);
    ch.at_most("RU J2 L w_c: U / |a_inf|", lw.u / lw.a_inf.abs(), 0.05);
    for s in [&fbar, &l, &full] {
        report_series(s, (&fs, &fw), &fbar.runs[2]);
    }
    println!(
        "report Hex20 8^3 J2: S / S_Fbar(8^3) - 1 {:+.4}, w_c / w_Fbar(8^3) - 1 {:+.4}; S / S_ref - 1 {:+.4}, w_c / w_ref - 1 {:+.4}; \
         S / S_inf,L - 1 {:+.4}; plastic points {} of {}",
        hex20.s / fbar.runs[1].s - 1.0,
        hex20.w / fbar.runs[1].w - 1.0,
        hex20.s / fs.a_inf - 1.0,
        hex20.w / fw.a_inf - 1.0,
        hex20.s / ls.a_inf - 1.0,
        plastic_points(&hex20),
        27 * 512
    );

    // TJ, CJ.
    let mut t_s = [f64::NAN; 3];
    let mut t_w = [f64::NAN; 3];
    for (k, n) in NS.iter().enumerate() {
        t_s[k] = tolerance(error_at(l.runs[k].s, &ls), error_at(fbar.runs[k].s, &fs));
        t_w[k] = tolerance(error_at(l.runs[k].w, &lw), error_at(fbar.runs[k].w, &fw));
        ch.holds(
            &format!("TJ {n}^3: T_S = {:.2}, T_w = {:.2} finite", t_s[k], t_w[k]),
            t_s[k].is_finite() && t_w[k].is_finite(),
        );
        ch.at_most(
            &format!("CJ block {n}^3: |S_L / S_Fbar - 1| against T_S({n})"),
            rel(l.runs[k].s, fbar.runs[k].s),
            t_s[k],
        );
        ch.at_most(
            &format!("CJ block {n}^3: |w_L / w_Fbar - 1| against T_w({n})"),
            rel(l.runs[k].w, fbar.runs[k].w),
            t_w[k],
        );
    }
    // DB.
    ch.at_most("DB DB4: |S_L / S_Fbar - 1| against T_S(8)", rel(db_l.s, db_f.s), t_s[1]);
    ch.at_most("DB DB4: |w_L / w_Fbar - 1| against T_w(8)", rel(db_l.w, db_f.w), t_w[1]);

    // NC1a, NC1b, NC1c.
    for (k, n) in NS.iter().enumerate() {
        ch.above(
            &format!("NC1a block {n}^3: |S_Full / S_ref - 1| beyond T_S({n}) (margin)"),
            rel(full.runs[k].s, fs.a_inf) - t_s[k],
            0.0,
        );
        ch.above(
            &format!("NC1b block {n}^3: |S_Full / S_Fbar - 1| beyond T_S({n}) (margin)"),
            rel(full.runs[k].s, fbar.runs[k].s) - t_s[k],
            0.0,
        );
    }
    let us = full.s();
    println!(
        "NC1c Full: |S_inf,Full - S_inf,Fbar| = {:.4e}, U_Full + U_Fbar = {:.4e}",
        (us.a_inf - fs.a_inf).abs(),
        us.u + fs.u
    );
    ch.holds(
        "NC1c Full: G1 does not hold for S (a different limit)",
        us.a_inf.is_finite() && fs.a_inf.is_finite() && !same_limit(&us, &fs),
    );

    // c5: G1, X1, gamma_b (reported).
    let g1s = same_limit(&ls, &fs);
    let g1w = same_limit(&lw, &fw);
    println!(
        "report G1 J2 S: |S_inf,L - S_inf,Fbar| = {:.4e}, U_L + U_Fbar = {:.4e}: {}",
        (ls.a_inf - fs.a_inf).abs(),
        ls.u + fs.u,
        if g1s { "same limit" } else { "different limits" }
    );
    println!(
        "report G1 J2 w_c: |w_inf,L - w_inf,Fbar| = {:.4e}, U_L + U_Fbar = {:.4e}: {}",
        (lw.a_inf - fw.a_inf).abs(),
        lw.u + fw.u,
        if g1w { "same limit" } else { "different limits" }
    );
    let x1 = |name: &str, a: &Reading, b: &Reading, pick: fn(&Run) -> f64| {
        let mut ok = same_limit(b, a);
        for k in [1, 2] {
            let d_f = rel(pick(&fbar.runs[k]), a.a_inf);
            let d_l = rel(pick(&l.runs[k]), b.a_inf);
            println!("report X1 {name} {}^3: d_Fbar {d_f:.4}, d_L {d_l:.4}", NS[k]);
            ok &= d_f <= d_l.max(0.02);
        }
        println!(
            "report X1 {name}: F-bar's edge excess {}",
            if ok { "DISAPPEARS" } else { "PERSISTS" }
        );
    };
    x1("S", &fs, &ls, |r| r.s);
    x1("w_c (reported only)", &fw, &lw, |r| r.w);
    for (k, n) in NS.iter().enumerate() {
        let (lk, fk) = (&l.runs[k], &fbar.runs[k]);
        let gamma = (lk.s_band - fk.s_band) / (lk.s - fk.s);
        println!(
            "report shares block {n}^3: gap S_L / S_Fbar - 1 {:+.4}, w_L / w_Fbar - 1 {:+.4}; band: S_b,L {:.6}, S_b,Fbar {:.6}, \
             gamma_b {gamma:+.4}",
            lk.s / fk.s - 1.0,
            lk.w / fk.w - 1.0,
            lk.s_band,
            fk.s_band,
        );
    }
    println!("heavy record: {:.1} s", t0.elapsed().as_secs_f64());
    ch.finish();
}
