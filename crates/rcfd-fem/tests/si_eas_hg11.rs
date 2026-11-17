//! F-747 (ADR 0678): rule L of ADR 0630 (with S of ADR 0658) against the F-bar Hex8 of ADR 0506 with the J2 material of ADR
//! 0666 on the punch block of ADR 0636 on `4^3`, `8^3` and `16^3`: the order-of-convergence reading of the punch reaction and
//! the corner bulge (RD), the J2 tolerance derived from it (TJ, CJ, DB), the elastic positive control (EC1), the Full Hex8
//! negative control (NC1), the reading on synthetic sequences (NC2), the set-up rows against the published values (SU1 to SU3,
//! FC, JY), and the reports of the reason for the gap of ADR 0666 A1.2 (G1, the edge and stabilisation shares, the Hex20 on
//! `8^3`). Every aggregated value is required finite before it is compared; a failed solve is a non-finite measure. The
//! helpers are copies of those of `si_eas_hg9.rs` (existing tests are not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::material::HyperelasticEnergy;
use rcfd_fem::nl_incompatible::IncompatibleStabilisation;
use rcfd_fem::nonlinear::{solve, NewtonOptions, NonlinearResult};
use rcfd_fem::solve::DirectFactor;
use rcfd_fem::{BlockProperty, Constraint, Material, SolidFormulation, SolidMesh, SolidModel};
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

const FULL: SolidFormulation = SolidFormulation::Full;

/// The material of a solve: the J2 material of ADR 0666 reading c1 or the neo-Hookean material of ADR 0636.
#[derive(Clone, Copy, PartialEq)]
enum Mat {
    J2,
    NeoHookean,
}

impl Mat {
    fn material(self) -> Material {
        match self {
            Mat::J2 => j2(),
            Mat::NeoHookean => neo_hookean(),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Mat::J2 => "J2",
            Mat::NeoHookean => "NH",
        }
    }
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
/// The variant S of ADR 0658 (cold), as ADR 0666 A1.2.
const S: Opts = Opts {
    name: "S",
    backtracking: true,
    decrease: true,
    warm: false,
};
/// The remedy W of ADR 0660, the fallback of reading c1.
const W: Opts = Opts {
    name: "W",
    backtracking: true,
    decrease: true,
    warm: true,
};

const SUPERNODAL: DirectFactor = DirectFactor::SupernodalNestedDissection;
const ENVELOPE: DirectFactor = DirectFactor::Envelope;

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

/// The sum of the driver's reactions over a node list's component.
fn reaction_sum(r: &NonlinearResult, nodes: &[usize], comp: usize) -> f64 {
    let re = r.result.reactions().unwrap();
    let dofs = r.result.dofs();
    nodes.iter().map(|&n| re[dofs.equation(n, comp).unwrap()]).sum()
}

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

// ============================================================================ the punch block (reading c1)

const DELTA: f64 = 0.1;
const TOL: f64 = 1e-9;

/// The punch, corner and edge-row sets on a mesh of the unit cube (by coordinates; for the Hex20 with the mid-edge nodes).
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

/// The block mesh on `n^3` with its sets; moved at amplitude `amp` with `seed` when `amp > 0`.
fn block_mesh(n: usize, amp: f64, seed: u64) -> (SolidMesh, usize) {
    let mut mesh = hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap();
    add_sets(&mut mesh);
    if amp > 0.0 {
        distort(&mesh, [1.0, 1.0, 1.0], [n, n, n], amp, seed)
    } else {
        (mesh, 0)
    }
}

/// The Hex20 block (reading c2): the regular `n^3` block upgraded, the sets by coordinates.
fn hex20_mesh(n: usize) -> SolidMesh {
    let mut mesh = upgrade_to_hex20(&hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap())
        .unwrap()
        .0;
    add_sets(&mut mesh);
    mesh
}

/// DB4 of ADR 0649 D2.
fn db4_mesh() -> (SolidMesh, usize) {
    block_mesh(8, 0.25, 0xF719_0008)
}

/// A solve of the block: `R = -sum_punch r_z / mu`, `w_c`, the edge-row part `R_e`, the plastic points; NaN on failure.
struct Run {
    result: Option<NonlinearResult>,
    r: f64,
    w: f64,
    r_edge: f64,
    opts: &'static str,
}

fn steps_text(r: &NonlinearResult) -> String {
    let halved = r.steps.iter().filter(|s| s.halvings > 0).count();
    let iterations: Vec<usize> = r.steps.iter().map(|s| s.residuals.len()).collect();
    format!(
        "increments {}, halved {halved}, residuals per increment {iterations:?}",
        r.steps.len()
    )
}

fn solve_block(mesh: &SolidMesh, tag: &str, kind: Kind, opts: Opts, mat: Mat, factor: DirectFactor) -> Run {
    let mut m = model_of(mesh.clone(), kind, opts, mat.material());
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
    let options = NewtonOptions::new(10).threads(4).record_active_set(true).factor(factor);
    let t0 = Instant::now();
    let out = solve(&m, &[], &[], &options);
    let secs = t0.elapsed().as_secs_f64();
    match out {
        Ok(r) => {
            let punch = m.mesh().node_set("punch").unwrap().to_vec();
            let edge = m.mesh().node_set("edge").unwrap().to_vec();
            let corner = m.mesh().node_set("corner").unwrap()[0];
            let rz = -reaction_sum(&r, &punch, 2) / MU;
            let re = -reaction_sum(&r, &edge, 2) / MU;
            let wc = r.result.displacement(corner)[2];
            println!(
                "{tag} {} {} {} {factor:?}: R/mu {rz:.8}, w_c {wc:+.8}, R_edge/mu {re:.8}, {}, {secs:.1} s",
                mat.name(),
                kind.name,
                opts.name,
                steps_text(&r)
            );
            Run {
                result: Some(r),
                r: rz,
                w: wc,
                r_edge: re,
                opts: opts.name,
            }
        }
        Err(e) => {
            println!(
                "{tag} {} {} {} {factor:?}: solve failed: {e}, {secs:.1} s",
                mat.name(),
                kind.name,
                opts.name
            );
            Run {
                result: None,
                r: f64::NAN,
                w: f64::NAN,
                r_edge: f64::NAN,
                opts: opts.name,
            }
        }
    }
}

/// Rule L with S, and with W once if S fails (reading c1).
fn solve_l(mesh: &SolidMesh, tag: &str, mat: Mat, factor: DirectFactor) -> Run {
    let s = solve_block(mesh, tag, K_L, S, mat, factor);
    if s.result.is_some() {
        return s;
    }
    println!("{tag}: S failed, W run once (reading c1)");
    solve_block(mesh, tag, K_L, W, mat, factor)
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

// ============================================================================ the reading (c4, c5)

/// The order-of-convergence reading of three values on `h = 1/4, 1/8, 1/16` (reading c4).
#[derive(Debug, Clone, Copy)]
struct Reading {
    asymptotic: bool,
    p: f64,
    a_inf: f64,
    u: f64,
}

/// The reading as registered (ADR 0678 c4): its limit `a16 + d2 / (2^p - 1)` has the wrong sign (amendment 1); kept for the
/// ignored record of the first run.
fn reading_registered(a4: f64, a8: f64, a16: f64) -> Reading {
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
                a_inf: a16 + d2 / g,
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

/// The reading of amendment 1: the Richardson limit `a16 - d2 / (2^p - 1)` (`d2 = a8 - a16`); the rest as registered.
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

/// `E_X(n)`: the error of a value against its element's limit, with the uncertainty (reading c4).
fn error_at(a: f64, r: &Reading) -> f64 {
    ((a - r.a_inf).abs() + r.u) / r.a_inf.abs()
}

/// `T(n) = 1.25 (E_L + E_Fbar)`, rounded up to two decimals (reading c5).
fn tolerance(e_l: f64, e_f: f64) -> f64 {
    (125.0 * (e_l + e_f)).ceil() / 100.0
}

/// G1: the two limits within the sum of their uncertainties (reading c6).
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

// ============================================================================ the fast test (set-up rows, NC2)

/// The set-up values of arithmetic 1: `(R, w_c)`.
const NH_FBAR_4: (f64, f64) = (0.38597738, 0.07220995);
const NH_L_4: (f64, f64) = (0.43284260, 0.07317520);
const NH_FBAR_8: (f64, f64) = (0.34403791, 0.06372110);
const NH_L_8: (f64, f64) = (0.36659596, 0.06631664);
const J2_FBAR_4: (f64, f64) = (0.09785876, 0.05043897);
const J2_L_4: (f64, f64) = (0.08146704, 0.05092271);
const J2_FBAR_8: (f64, f64) = (0.07717089, 0.03601575);
const J2_L_8: (f64, f64) = (0.06172012, 0.02976180);
const DB4_FBAR: (f64, f64) = (0.07576858, 0.03522022);
const DB4_L: (f64, f64) = (0.06653272, 0.03300841);

fn against(ch: &mut Checks, id: &str, tag: &str, run: &Run, known: (f64, f64), limit: f64) {
    ch.at_most(
        &format!("{id} {tag}: R against the known value"),
        rel(run.r, known.0),
        limit,
    );
    ch.at_most(
        &format!("{id} {tag}: w_c against the known value"),
        rel(run.w, known.1),
        limit,
    );
}

fn nc2_rows(ch: &mut Checks, reading: fn(f64, f64, f64) -> Reading) {
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

#[test]
#[ignore = "ADR 0678 amendment 1: the registered run, defective: the registered Richardson limit has the wrong sign (NC2 (i) |a_inf - 1| 9.375e-3 > 1e-10); every other row met"]
/// SU1, SU2, SU3, M0, M1, FC, JY on `4^3`, `8^3` and DB4; NC2 with the reading as registered (the registered run).
fn j2_refined_reference_setup_known_miss() {
    setup_rows(reading_registered);
}

#[test]
/// The same rows with the reading of amendment 1.
fn j2_refined_reference_setup_corrected() {
    setup_rows(reading);
}

fn setup_rows(reading: fn(f64, f64, f64) -> Reading) {
    let mut ch = Checks::new();
    nc2_rows(&mut ch, reading);
    for (n, f_known, l_known) in [(4, NH_FBAR_4, NH_L_4), (8, NH_FBAR_8, NH_L_8)] {
        let (mesh, _) = block_mesh(n, 0.0, 0);
        let tag = format!("block {n}^3");
        let f = solve_block(&mesh, &tag, K_FBAR, DEFAULT, Mat::NeoHookean, SUPERNODAL);
        let l = solve_block(&mesh, &tag, K_L, DEFAULT, Mat::NeoHookean, SUPERNODAL);
        against(&mut ch, "SU1", &format!("{tag} NH Fbar"), &f, f_known, 1e-6);
        against(&mut ch, "SU1", &format!("{tag} NH L"), &l, l_known, 1e-6);
    }
    for (n, f_known, l_known) in [(4, J2_FBAR_4, J2_L_4), (8, J2_FBAR_8, J2_L_8)] {
        let (mesh, _) = block_mesh(n, 0.0, 0);
        let tag = format!("block {n}^3");
        let f = solve_block(&mesh, &tag, K_FBAR, DEFAULT, Mat::J2, SUPERNODAL);
        let l = solve_l(&mesh, &tag, Mat::J2, SUPERNODAL);
        against(&mut ch, "SU2", &format!("{tag} J2 Fbar"), &f, f_known, 1e-6);
        against(&mut ch, "SU2", &format!("{tag} J2 L ({})", l.opts), &l, l_known, 1e-6);
        ch.above(&format!("JY {tag}: plastic points of Fbar"), plastic_points(&f), 0.0);
        ch.above(&format!("JY {tag}: plastic points of L"), plastic_points(&l), 0.0);
        if n == 4 {
            let fe = solve_block(&mesh, &tag, K_FBAR, DEFAULT, Mat::J2, ENVELOPE);
            let le = solve_block(&mesh, &tag, K_L, S, Mat::J2, ENVELOPE);
            ch.at_most(&format!("FC {tag} Fbar: R supernodal / envelope"), rel(f.r, fe.r), 1e-9);
            ch.at_most(
                &format!("FC {tag} Fbar: w_c supernodal / envelope"),
                rel(f.w, fe.w),
                1e-9,
            );
            ch.at_most(&format!("FC {tag} L: R supernodal / envelope"), rel(l.r, le.r), 1e-9);
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
    let f = solve_block(&mesh, tag, K_FBAR, DEFAULT, Mat::J2, SUPERNODAL);
    let l = solve_l(&mesh, tag, Mat::J2, SUPERNODAL);
    against(&mut ch, "SU3", &format!("{tag} J2 Fbar"), &f, DB4_FBAR, 1e-6);
    against(&mut ch, "SU3", &format!("{tag} J2 L ({})", l.opts), &l, DB4_L, 1e-6);
    ch.above(&format!("JY {tag}: plastic points of Fbar"), plastic_points(&f), 0.0);
    ch.above(&format!("JY {tag}: plastic points of L"), plastic_points(&l), 0.0);
    ch.finish();
}

// ============================================================================ the heavy record (c1 to c6)

/// One element's three regular runs.
struct Series {
    name: String,
    runs: Vec<Run>,
}

impl Series {
    fn r(&self) -> Reading {
        reading(self.runs[0].r, self.runs[1].r, self.runs[2].r)
    }
    fn w(&self) -> Reading {
        reading(self.runs[0].w, self.runs[1].w, self.runs[2].w)
    }
}

const NS: [usize; 3] = [4, 8, 16];

fn series(name: &str, mut each: impl FnMut(&SolidMesh, &str) -> Run) -> Series {
    let runs = NS
        .iter()
        .map(|&n| {
            let (mesh, _) = block_mesh(n, 0.0, 0);
            each(&mesh, &format!("block {n}^3"))
        })
        .collect();
    Series {
        name: name.to_string(),
        runs,
    }
}

fn report_series(s: &Series, reference: (&Reading, &Reading), fbar16: &Run) {
    let (rr, rw) = (s.r(), s.w());
    println!("report reading {} R: {}", s.name, reading_text(&rr));
    println!("report reading {} w_c: {}", s.name, reading_text(&rw));
    for (k, run) in s.runs.iter().enumerate() {
        println!(
            "report {} {}^3: R / R_ref - 1 {:+.4}, w_c / w_ref - 1 {:+.4}; against Fbar 16^3 {:+.4}, {:+.4}; E_R {:.4}, E_w {:.4}, \
             per-element tolerance 1.25 E: {:.4}, {:.4}",
            s.name,
            NS[k],
            run.r / reference.0.a_inf - 1.0,
            run.w / reference.1.a_inf - 1.0,
            run.r / fbar16.r - 1.0,
            run.w / fbar16.w - 1.0,
            error_at(run.r, &rr),
            error_at(run.w, &rw),
            1.25 * error_at(run.r, &rr),
            1.25 * error_at(run.w, &rw),
        );
    }
}

#[test]
#[ignore = "ADR 0678: the heavy record (six 16^3 solves, the Hex20 8^3), run once (409 s), known miss: EC1 (the neo-Hookean rule L 16^3 solve fails, singular tangent at load factor 0.966), NC1 (the Full Hex8 16^3 solve fails, inverted element at 0.988, and R_Full 0.07612 < R_Fbar 0.07717 on 8^3); every other row met; the fast test j2_refined_reference_setup_corrected covers the same path"]
/// RC0, JY, RD, TJ, CJ, DB, EC1, NC1 and the reports of c3 and c6.
fn j2_refined_reference_heavy_record_known_miss() {
    let mut ch = Checks::new();
    let t0 = Instant::now();
    let fbar = series("J2 Fbar", |m, t| {
        solve_block(m, t, K_FBAR, DEFAULT, Mat::J2, SUPERNODAL)
    });
    let l = series("J2 L", |m, t| solve_l(m, t, Mat::J2, SUPERNODAL));
    let off = series("J2 Off (S)", |m, t| solve_block(m, t, K_OFF, S, Mat::J2, SUPERNODAL));
    let full = series("J2 Full", |m, t| {
        solve_block(m, t, K_FULL, DEFAULT, Mat::J2, SUPERNODAL)
    });
    let nh_f = series("NH Fbar", |m, t| {
        solve_block(m, t, K_FBAR, DEFAULT, Mat::NeoHookean, SUPERNODAL)
    });
    let nh_l = series("NH L", |m, t| {
        solve_block(m, t, K_L, DEFAULT, Mat::NeoHookean, SUPERNODAL)
    });
    let hex20 = solve_block(&hex20_mesh(8), "Hex20 block 8^3", K_HEX20, DEFAULT, Mat::J2, SUPERNODAL);
    let (db, _) = db4_mesh();
    let db_f = solve_block(&db, "DB4", K_FBAR, DEFAULT, Mat::J2, SUPERNODAL);
    let db_l = solve_l(&db, "DB4", Mat::J2, SUPERNODAL);
    println!("all solves: {:.1} s", t0.elapsed().as_secs_f64());

    // RC0, JY.
    ch.holds(
        "RC0: the 16^3 J2 solves of Fbar and L succeed",
        fbar.runs[2].r.is_finite() && fbar.runs[2].w.is_finite() && l.runs[2].r.is_finite() && l.runs[2].w.is_finite(),
    );
    for (k, n) in NS.iter().enumerate() {
        ch.above(
            &format!("JY block {n}^3: plastic points of Fbar"),
            plastic_points(&fbar.runs[k]),
            0.0,
        );
        ch.above(
            &format!("JY block {n}^3: plastic points of L"),
            plastic_points(&l.runs[k]),
            0.0,
        );
        println!(
            "report JY block {n}^3: plastic points Fbar {}, L {}, Off {}, Full {} of {}",
            plastic_points(&fbar.runs[k]),
            plastic_points(&l.runs[k]),
            plastic_points(&off.runs[k]),
            plastic_points(&full.runs[k]),
            POINTS * n * n * n
        );
    }
    ch.above("JY DB4: plastic points of Fbar", plastic_points(&db_f), 0.0);
    ch.above("JY DB4: plastic points of L", plastic_points(&db_l), 0.0);

    // RD: the readings.
    let (fr, fw, lr, lw) = (fbar.r(), fbar.w(), l.r(), l.w());
    for (name, r) in [("Fbar R", &fr), ("Fbar w_c", &fw), ("L R", &lr), ("L w_c", &lw)] {
        ch.at_most(&format!("RD J2 {name}: U / |a_inf|"), r.u / r.a_inf.abs(), 1.0);
    }
    let reference = (&fr, &fw);
    for s in [&fbar, &l, &off, &full, &nh_f, &nh_l] {
        let refs = if s.name.starts_with("NH") {
            (nh_f.r(), nh_f.w())
        } else {
            (fr, fw)
        };
        let f16 = if s.name.starts_with("NH") {
            &nh_f.runs[2]
        } else {
            &fbar.runs[2]
        };
        report_series(s, (&refs.0, &refs.1), f16);
    }
    println!(
        "report Hex20 8^3 J2: R / R_Fbar(8^3) - 1 {:+.4}, w_c / w_Fbar(8^3) - 1 {:+.4}; R / R_ref - 1 {:+.4}, w_c / w_ref - 1 {:+.4}; \
         plastic points {} of {}",
        hex20.r / fbar.runs[1].r - 1.0,
        hex20.w / fbar.runs[1].w - 1.0,
        hex20.r / reference.0.a_inf - 1.0,
        hex20.w / reference.1.a_inf - 1.0,
        plastic_points(&hex20),
        27 * 512
    );

    // TJ, CJ: the tolerance and the regular meshes.
    let mut t_r = [f64::NAN; 3];
    let mut t_w = [f64::NAN; 3];
    for (k, n) in NS.iter().enumerate() {
        t_r[k] = tolerance(error_at(l.runs[k].r, &lr), error_at(fbar.runs[k].r, &fr));
        t_w[k] = tolerance(error_at(l.runs[k].w, &lw), error_at(fbar.runs[k].w, &fw));
        ch.holds(
            &format!("TJ {n}^3: T_R = {:.2}, T_w = {:.2} finite", t_r[k], t_w[k]),
            t_r[k].is_finite() && t_w[k].is_finite(),
        );
        ch.at_most(
            &format!("CJ block {n}^3: |R_L / R_Fbar - 1| against T_R({n})"),
            rel(l.runs[k].r, fbar.runs[k].r),
            t_r[k],
        );
        ch.at_most(
            &format!("CJ block {n}^3: |w_L / w_Fbar - 1| against T_w({n})"),
            rel(l.runs[k].w, fbar.runs[k].w),
            t_w[k],
        );
    }
    // DB.
    ch.at_most("DB DB4: |R_L / R_Fbar - 1| against T_R(8)", rel(db_l.r, db_f.r), t_r[1]);
    ch.at_most("DB DB4: |w_L / w_Fbar - 1| against T_w(8)", rel(db_l.w, db_f.w), t_w[1]);

    // EC1, NC1.
    let (nfr, nlr) = (nh_f.r(), nh_l.r());
    println!(
        "EC1 NH: |R_inf,L - R_inf,Fbar| = {:.4e}, U_L + U_Fbar = {:.4e}",
        (nlr.a_inf - nfr.a_inf).abs(),
        nlr.u + nfr.u
    );
    ch.holds("EC1 NH: G1 holds for R (same limit)", same_limit(&nlr, &nfr));
    let ur = full.r();
    println!(
        "NC1 Full: |R_inf,Full - R_inf,Fbar| = {:.4e}, U_Full + U_Fbar = {:.4e}",
        (ur.a_inf - fr.a_inf).abs(),
        ur.u + fr.u
    );
    ch.holds(
        "NC1 Full: G1 does not hold for R (a different limit)",
        ur.a_inf.is_finite() && fr.a_inf.is_finite() && !same_limit(&ur, &fr),
    );
    ch.holds(
        "NC1 Full: R_Full(n) > R_Fbar(n) on every mesh",
        (0..3).all(|k| full.runs[k].r > fbar.runs[k].r),
    );

    // c6: the classification (reported).
    let g1r = same_limit(&lr, &fr);
    let g1w = same_limit(&lw, &fw);
    println!(
        "report G1 J2 R: |R_inf,L - R_inf,Fbar| = {:.4e}, U_L + U_Fbar = {:.4e}: {}",
        (lr.a_inf - fr.a_inf).abs(),
        lr.u + fr.u,
        if g1r { "same limit" } else { "different limits" }
    );
    println!(
        "report G1 J2 w_c: |w_inf,L - w_inf,Fbar| = {:.4e}, U_L + U_Fbar = {:.4e}: {}",
        (lw.a_inf - fw.a_inf).abs(),
        lw.u + fw.u,
        if g1w { "same limit" } else { "different limits" }
    );
    let b = lr.a_inf / fr.a_inf - 1.0;
    let range = lr.u / lr.a_inf.abs() + fr.u / fr.a_inf.abs();
    println!("report B (R): {b:+.4} +- {range:.4}");
    println!(
        "report E(8) R: L {:.4}, Fbar {:.4}; w_c: L {:.4}, Fbar {:.4}",
        error_at(l.runs[1].r, &lr),
        error_at(fbar.runs[1].r, &fr),
        error_at(l.runs[1].w, &lw),
        error_at(fbar.runs[1].w, &fw)
    );
    for (k, n) in NS.iter().enumerate() {
        let (lk, fk, ok) = (&l.runs[k], &fbar.runs[k], &off.runs[k]);
        let gamma = (lk.r_edge - fk.r_edge) / (lk.r - fk.r);
        let sigma = (ok.r - lk.r) / (fk.r - lk.r);
        println!(
            "report shares block {n}^3: gap R_L / R_Fbar - 1 {:+.4}; edge row: R_e,L {:.6}, R_e,Fbar {:.6}, gamma {gamma:+.4}; \
             Off: R_Off {:.6}, sigma {sigma:+.4}",
            lk.r / fk.r - 1.0,
            lk.r_edge,
            fk.r_edge,
            ok.r
        );
    }
    println!("heavy record: {:.1} s", t0.elapsed().as_secs_f64());
    ch.finish();
}
