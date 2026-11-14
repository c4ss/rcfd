//! F-513 (ADR 0426): beams on the transformation path. Checks J1 to J4, K1, K2, L1 to L3, M1 to M3, Z1 to Z6, S2 and the
//! negative controls N1, N3, N4 of the registration (R1 is the retired-row copy in `si_beam.rs`; B1 to B3 are the existing
//! tests).

use rcfd_core::vec3::{cross, norm, Vec3};
use rcfd_fem::assemble::stiffness;
use rcfd_fem::beam::{self, rectangle_section};
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::free_motions::free_rigid_motions;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::mpc::{solve_static_mpc, MpcSolution, ReferenceLoad};
use rcfd_fem::solve::{solve_static, CgPreconditioner, StaticOptions, StaticSolver};
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, ElasticSupport, ElementKind, FemError, LinearTerm, Load, Material,
    SolidMesh, SolidModel,
};
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.4e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn max_abs(v: impl IntoIterator<Item = f64>) -> f64 {
    v.into_iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

fn vdiff(a: Vec3, b: Vec3) -> f64 {
    max_abs(sub(a, b))
}

fn rel(meas: f64, exact: f64) -> f64 {
    ((meas - exact) / exact).abs()
}

// ------------------------------------------------------------------------------------------------ data (arithmetic 3)

const E1: f64 = 2.1e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const L: f64 = 1.0;
const F_TIP: Vec3 = [1200.0, -2500.0, 3700.0];
const M_TIP: Vec3 = [450.0, 0.0, 0.0];
const ORIENT: Vec3 = [0.0, 1.0, 0.0];

fn section1() -> BeamSection {
    BeamSection::new(0.012, 3.0e-6, 5.0e-6, 4.0e-6, 0.85, 0.75).unwrap()
}

fn mat1() -> Material {
    Material::isotropic_elastic(E1, NU, RHO, 0.0).unwrap()
}

fn g1() -> f64 {
    E1 / (2.0 * (1.0 + NU))
}

/// The cantilever closed form of ADR 0417 arithmetic 6 in the global axes (local = global for the orientation `(0, 1, 0)`
/// along `x`): tip translation and rotation under the tip force `F_TIP` and torque `M_TIP[0]`.
fn cantilever() -> (Vec3, Vec3) {
    let (a, iy, iz, j) = (0.012, 3.0e-6, 5.0e-6, 4.0e-6);
    let g = g1();
    let f = F_TIP;
    let u = [
        f[0] * L / (E1 * a),
        f[1] * (L.powi(3) / (3.0 * E1 * iz) + L / (0.85 * g * a)),
        f[2] * (L.powi(3) / (3.0 * E1 * iy) + L / (0.75 * g * a)),
    ];
    let t = [
        M_TIP[0] * L / (g * j),
        -f[2] * L * L / (2.0 * E1 * iy),
        f[1] * L * L / (2.0 * E1 * iz),
    ];
    (u, t)
}

/// A Hex8 box and a chain of Beam2 elements through `beam_nodes` (none of them a box node), in one mesh: the box nodes
/// first (the same indices as in `hex`), then the beam nodes. Sets: the box's node and surface sets, "block" (every box
/// node), "solid" and "beam" (elements), "root" and "tip" (first and last beam node), "joint" (`xmax` and the root).
fn block_and_beam(
    hex: &SolidMesh,
    beam_nodes: &[Vec3],
    section: BeamSection,
    block_mat: Material,
    beam_mat: Material,
) -> (SolidModel, usize, usize) {
    let nh = hex.nodes().len();
    let mut nodes = hex.nodes().to_vec();
    nodes.extend_from_slice(beam_nodes);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hex.blocks()[0].connectivity().to_vec())
        .unwrap();
    let ne_h = hex.blocks()[0].connectivity().len() / 8;
    let nb = beam_nodes.len();
    let conn: Vec<usize> = (0..nb - 1).flat_map(|i| [nh + i, nh + i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    mesh.set_block_property(1, BlockProperty::beam(section, ORIENT).unwrap())
        .unwrap();
    mesh.add_element_set("solid", (0..ne_h).collect()).unwrap();
    mesh.add_element_set("beam", (ne_h..ne_h + nb - 1).collect()).unwrap();
    for name in BOX_FACES {
        mesh.add_node_set(name, hex.node_set(name).unwrap().to_vec()).unwrap();
        mesh.add_surface_set(name, hex.surface_set(name).unwrap().to_vec())
            .unwrap();
    }
    mesh.add_node_set("block", (0..nh).collect()).unwrap();
    let (root, tip) = (nh, nh + nb - 1);
    mesh.add_node_set("root", vec![root]).unwrap();
    mesh.add_node_set("tip", vec![tip]).unwrap();
    let mut joint = hex.node_set("xmax").unwrap().to_vec();
    joint.push(root);
    mesh.add_node_set("joint", joint).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("solid", block_mat).unwrap();
    m.add_material("beam", beam_mat).unwrap();
    (m, root, tip)
}

fn block_mat() -> Material {
    Material::isotropic_elastic(5.0e9, 0.3, 1200.0, 0.0).unwrap()
}

fn block_mesh() -> SolidMesh {
    hex_box([0.0, -0.1, -0.1], [0.2, 0.2, 0.2], [3, 3, 3]).unwrap()
}

const X_ROOT: Vec3 = [0.2, 0.0, 0.0];

/// The joined model of arithmetic 3: the block (clamped on `xmin` if `clamp`), the beam from `X_ROOT` along `x` in 4
/// elements, the rigid set "joint" with the reference point `reference`, the tip loads. Returns the model, the root and
/// tip nodes and the index of the rigid set.
fn joined(reference: Vec3, clamp: bool) -> (SolidModel, usize, usize, usize) {
    let beam_nodes: Vec<Vec3> = (0..=4).map(|i| [0.2 + L * i as f64 / 4.0, 0.0, 0.0]).collect();
    let (mut m, root, tip) = block_and_beam(&block_mesh(), &beam_nodes, section1(), block_mat(), mat1());
    if clamp {
        for c in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
    }
    let ci = m.constraints().len();
    m.add_constraint(Constraint::rigid_set("joint", reference)).unwrap();
    m.add_load(Load::nodal_force("tip", F_TIP)).unwrap();
    m.add_load(Load::nodal_moment("tip", M_TIP)).unwrap();
    (m, root, tip, ci)
}

fn split6(d: &[f64]) -> (Vec3, Vec3) {
    assert_eq!(d.len(), 6);
    ([d[0], d[1], d[2]], [d[3], d[4], d[5]])
}

fn mpc(m: &SolidModel, loads: &[ReferenceLoad]) -> MpcSolution {
    solve_static_mpc(m, &[], &[], loads, &StaticOptions::default()).unwrap()
}

// ------------------------------------------------------------------------------------------------ J1 to J4, N1

#[test]
fn j1_to_j4_n1_beam_joined_to_a_block_by_a_rigid_set() {
    let mut c = Checks::new();
    let (model, _root, tip, ci) = joined(X_ROOT, true);
    let joint = mpc(&model, &[]);
    let (ut, rt) = split6(joint.result.displacement(tip));
    // The block's compliance, independently: the solid-only block with the same rigid set, loaded by the root force and
    // moment.
    let mut solid = SolidModel::new(block_mesh());
    solid.add_material("all", block_mat()).unwrap();
    for comp in 0..3 {
        solid
            .add_constraint(Constraint::displacement("xmin", comp, 0.0))
            .unwrap();
    }
    solid.add_constraint(Constraint::rigid_set("xmax", X_ROOT)).unwrap();
    let x_tip = [0.2 + L, 0.0, 0.0];
    let m_root = add(M_TIP, cross(sub(x_tip, X_ROOT), F_TIP));
    let alone = mpc(&solid, &[ReferenceLoad::new(3, F_TIP, m_root)]);
    let (t, w) = (alone.references[0].translation, alone.references[0].rotation);
    let (u_cant, th_cant) = cantilever();
    let u_pred = add(add(t, cross(w, sub(x_tip, X_ROOT))), u_cant);
    let th_pred = add(w, th_cant);
    println!("tip: joined u {ut:?} theta {rt:?}");
    println!("tip: predicted u {u_pred:?} theta {th_pred:?}; block t {t:?} w {w:?}");
    c.at_most(
        "J1 tip translation against beam + block compliance",
        vdiff(ut, u_pred) / max_abs(ut),
        1e-8,
    );
    c.at_most(
        "J1 tip rotation against beam + block compliance",
        vdiff(rt, th_pred) / max_abs(rt),
        1e-8,
    );
    // J2: the block's nodes and the reference motion.
    let nh = block_mesh().nodes().len();
    let mut d = 0.0f64;
    let mut big = 0.0f64;
    for a in 0..nh {
        let (x, y) = (joint.result.displacement(a), alone.result.displacement(a));
        assert_eq!((x.len(), y.len()), (3, 3));
        for k in 0..3 {
            d = d.max((x[k] - y[k]).abs());
            big = big.max(y[k].abs());
        }
    }
    c.at_most("J2 block nodes, joined against solid-only", d / big, 1e-8);
    let rj = joint.references.iter().find(|r| r.constraint == ci).copied().unwrap();
    c.at_most("J2 reference translation", vdiff(rj.translation, t) / max_abs(t), 1e-8);
    c.at_most("J2 reference rotation", vdiff(rj.rotation, w) / max_abs(w), 1e-8);
    // J3: the reference point at a corner of the face.
    let (m3, _, tip3, _) = joined([0.2, 0.1, 0.1], true);
    let s3 = mpc(&m3, &[]);
    let (u3, r3) = split6(s3.result.displacement(tip3));
    c.at_most(
        "J3 reference at the corner: tip translation",
        vdiff(u3, ut) / max_abs(ut),
        1e-8,
    );
    c.at_most(
        "J3 reference at the corner: tip rotation",
        vdiff(r3, rt) / max_abs(rt),
        1e-8,
    );
    // J4: the reactions at xmin balance the tip loads.
    let reactions = joint.result.reactions().unwrap();
    let dofs = model.dof_map();
    let mut rf = [0.0; 3];
    let mut rm = [0.0; 3];
    for &a in model.mesh().node_set("xmin").unwrap() {
        let e = dofs.equation(a, 0).unwrap();
        let ra = [reactions[e], reactions[e + 1], reactions[e + 2]];
        rf = add(rf, ra);
        rm = add(rm, cross(model.mesh().nodes()[a], ra));
    }
    let fmax = max_abs(F_TIP);
    c.at_most("J4 reaction forces balance", norm(add(rf, F_TIP)) / fmax, 1e-10);
    c.at_most(
        "J4 reaction moments balance (about the origin)",
        norm(add(add(rm, cross(x_tip, F_TIP)), M_TIP)) / (norm(x_tip) * fmax),
        1e-10,
    );
    // N1: the beam's closed form alone misses the block's share.
    c.above(
        "N1 tip translation against the cantilever alone",
        vdiff(ut, u_cant) / max_abs(ut),
        0.1,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------ K1, K2, L1 to L3

/// A beam along `x` from the origin in `n` elements of total length `L`; sets "all", "root", "tip" (nodes), "beam"
/// (elements).
fn beam_line(n: usize, section: BeamSection, mat: Material) -> SolidModel {
    let nodes: Vec<Vec3> = (0..=n).map(|i| [L * i as f64 / n as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, (0..n).flat_map(|i| [i, i + 1]).collect())
        .unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section, ORIENT).unwrap())
        .unwrap();
    mesh.add_element_set("beam", (0..n).collect()).unwrap();
    mesh.add_node_set("all", (0..=n).collect()).unwrap();
    mesh.add_node_set("root", vec![0]).unwrap();
    mesh.add_node_set("tip", vec![n]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("beam", mat).unwrap();
    m
}

#[test]
fn k1_k2_l1_to_l3_springs_and_linear_constraints_on_rotations() {
    let mut c = Checks::new();
    let iz = 5.0e-6;
    let k_theta = E1 * iz / L;
    let mm = 300.0;
    for (name, at_tip) in [("K1 moment at the root", false), ("K2 moment at the tip", true)] {
        let mut m = beam_line(2, section1(), mat1());
        for comp in 0..5 {
            m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
        }
        m.add_support(ElasticSupport::nodal_spring("root", 5, k_theta)).unwrap();
        m.add_load(Load::nodal_moment(if at_tip { "tip" } else { "root" }, [0.0, 0.0, mm]))
            .unwrap();
        let s = mpc(&m, &[]);
        let th_root = s.result.displacement(0)[5];
        c.at_most(
            &format!("{name}: |k_theta theta - M| / |M|"),
            (k_theta * th_root - mm).abs() / mm,
            1e-10,
        );
        if at_tip {
            let d = s.result.displacement(2);
            c.at_most(
                "K2 tip rotation M / k + M L / (E I_z)",
                rel(d[5], mm / k_theta + mm * L / (E1 * iz)),
                1e-10,
            );
            c.at_most(
                "K2 tip deflection M L / k + M L^2 / (2 E I_z)",
                rel(d[1], mm * L / k_theta + mm * L * L / (2.0 * E1 * iz)),
                1e-10,
            );
        } else {
            let e = m.dof_map().equation(0, 5).unwrap();
            c.at_most(
                "K1 spring force |S + M| / |M|",
                (s.support_forces[e] + mm).abs() / mm,
                1e-10,
            );
        }
    }
    // L1: the seeded one-element cantilever clamped by six Linear rows against the elimination path.
    let (lin, clamped) = l1_models();
    let rl = mpc(&lin, &[]);
    let rc = solve_static(&clamped, &StaticOptions::default()).unwrap();
    for (field, range) in [("translations", 0..3), ("rotations", 3..6)] {
        let mut d = 0.0f64;
        let mut big = 0.0f64;
        for node in 0..2 {
            let (a, b) = (rl.result.displacement(node), rc.displacement(node));
            for k in range.clone() {
                d = d.max((a[k] - b[k]).abs());
                big = big.max(b[k].abs());
            }
        }
        c.at_most(
            &format!("L1 Linear clamp against the elimination path, {field}"),
            d / big,
            1e-12,
        );
    }
    // L2: a guided tip by Linear theta_z(tip) = 0 under F_y.
    let fy = -2500.0;
    let g = g1();
    let mut m = beam_line(2, section1(), mat1());
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::linear(vec![LinearTerm::new(2, 5, 1.0)], 0.0))
        .unwrap();
    m.add_load(Load::nodal_force("tip", [0.0, fy, 0.0])).unwrap();
    let s = mpc(&m, &[]);
    let e = m.dof_map().equation(2, 5).unwrap();
    c.at_most(
        "L2 guided tip deflection F (L^3 / (12 E I) + L / (kappa G A))",
        rel(
            s.result.displacement(2)[1],
            fy * (L.powi(3) / (12.0 * E1 * iz) + L / (0.85 * g * 0.012)),
        ),
        1e-10,
    );
    c.at_most(
        "L2 constraint moment -F L / 2",
        rel(s.constraint_forces[e], -fy * L / 2.0),
        1e-10,
    );
    // L3: Linear theta_z(tip) = theta_0, no load.
    let th0 = 1e-3;
    let mut m = beam_line(2, section1(), mat1());
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::linear(vec![LinearTerm::new(2, 5, 1.0)], th0))
        .unwrap();
    let s = mpc(&m, &[]);
    c.at_most(
        "L3 tip deflection theta_0 L / 2",
        rel(s.result.displacement(2)[1], th0 * L / 2.0),
        1e-10,
    );
    c.at_most(
        "L3 constraint moment E I theta_0 / L",
        rel(s.constraint_forces[e], E1 * iz * th0 / L),
        1e-10,
    );
    c.finish();
}

/// xorshift64* with the seed of ADR 0417 (`0x0417_0391`): the seeded one-element cantilever of its arithmetic 6.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        let r = x.wrapping_mul(0x2545_f491_4f6c_dd1d);
        2.0 * ((r >> 11) as f64 / (1u64 << 53) as f64) - 1.0
    }
    fn vec(&mut self) -> Vec3 {
        [self.next(), self.next(), self.next()]
    }
}

fn unit(v: Vec3) -> Vec3 {
    let n = norm(v);
    [v[0] / n, v[1] / n, v[2] / n]
}

/// The seeded cantilever (ADR 0417 arithmetic 6, length 0.8, its tip force `(1200, -2500, 3700)` N and torque 450 N m in
/// local axes) with the root clamped by six `Linear` rows of coefficient 2, and the same model clamped by
/// `Constraint::displacement`.
fn l1_models() -> (SolidModel, SolidModel) {
    let mut rng = Rng(0x0417_0391);
    let x0 = rng.vec();
    let e1 = unit(rng.vec());
    let mut v = unit(rng.vec());
    while norm(cross(e1, v)) < 0.3 {
        v = unit(rng.vec());
    }
    let x1 = add(x0, [0.8 * e1[0], 0.8 * e1[1], 0.8 * e1[2]]);
    // The frame of ADR 0417 arithmetic 1: e3 = e1 x v / |e1 x v|, e2 = e3 x e1.
    let e3 = unit(cross(e1, v));
    let e2 = cross(e3, e1);
    let global = |l: Vec3| -> Vec3 { std::array::from_fn(|i| l[0] * e1[i] + l[1] * e2[i] + l[2] * e3[i]) };
    let (f, t) = (global(F_TIP), global(M_TIP));
    let base = || {
        let mut mesh = SolidMesh::new(vec![x0, x1]).unwrap();
        mesh.add_block(ElementKind::Beam2, vec![0, 1]).unwrap();
        mesh.set_block_property(0, BlockProperty::beam(section1(), v).unwrap())
            .unwrap();
        mesh.add_element_set("all", vec![0]).unwrap();
        mesh.add_node_set("root", vec![0]).unwrap();
        mesh.add_node_set("tip", vec![1]).unwrap();
        let mut m = SolidModel::new(mesh);
        m.add_material("all", mat1()).unwrap();
        m.add_load(Load::nodal_force("tip", f)).unwrap();
        m.add_load(Load::nodal_moment("tip", t)).unwrap();
        m
    };
    let mut lin = base();
    let mut clamped = base();
    for comp in 0..6 {
        lin.add_constraint(Constraint::linear(vec![LinearTerm::new(0, comp, 2.0)], 0.0))
            .unwrap();
        clamped
            .add_constraint(Constraint::displacement("root", comp, 0.0))
            .unwrap();
    }
    (lin, clamped)
}

// ------------------------------------------------------------------------------------------------ M1 to M3

/// The roots of `cos x cosh x = -1` (clamped-free Euler-Bernoulli), by bisection.
fn cantilever_root(n: usize) -> f64 {
    let f = |x: f64| x.cos() * x.cosh() + 1.0;
    let (mut lo, mut hi) = if n == 1 {
        (1.0, 3.0)
    } else {
        let c = (n as f64 - 0.5) * PI;
        (c - 0.5, c + 0.5)
    };
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(lo) * f(mid) <= 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    0.5 * (lo + hi)
}

#[test]
fn m1_m2_m3_modes_of_a_beam_held_by_a_rigid_set() {
    let mut c = Checks::new();
    let (b, h, jt) = (0.01, 0.02, 4.58e-9);
    let section = rectangle_section(b, h, jt, 5.0 / 6.0).unwrap();
    let steel = Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap();
    let n = 20;
    let options = ModalOptions::new(6)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward);
    // The held beam: the root in a rigid set with the xmax face of a fixed Hex8 block.
    let hex = hex_box([-0.1, -0.05, -0.05], [0.1, 0.1, 0.1], [1, 1, 1]).unwrap();
    let beam_nodes: Vec<Vec3> = (0..=n).map(|i| [L * i as f64 / n as f64, 0.0, 0.0]).collect();
    let (mut held, _, _) = block_and_beam(&hex, &beam_nodes, section, steel.clone(), steel.clone());
    for comp in 0..3 {
        held.add_constraint(Constraint::displacement("block", comp, 0.0))
            .unwrap();
    }
    held.add_constraint(Constraint::rigid_set("joint", [0.0; 3])).unwrap();
    let mh = modal_analysis(&held, &options).unwrap();
    // The equivalent clamped beam on the elimination path.
    let mut clamped = beam_line(n, section, steel);
    for comp in 0..6 {
        clamped
            .add_constraint(Constraint::displacement("root", comp, 0.0))
            .unwrap();
    }
    let mc = modal_analysis(&clamped, &options).unwrap();
    println!("held:    {:?}", mh.frequencies);
    println!("clamped: {:?}", mc.frequencies);
    for i in 0..6 {
        c.at_most(
            &format!("M1 f_{} held against clamped", i + 1),
            rel(mh.frequencies[i], mc.frequencies[i]),
            1e-8,
        );
    }
    // M2: Euler-Bernoulli cantilever, both planes, the five lowest.
    let a = b * h;
    let mut eb = Vec::new();
    for i in [h * b.powi(3) / 12.0, b * h.powi(3) / 12.0] {
        for k in 1..=4 {
            let lam = cantilever_root(k);
            eb.push(lam * lam / (2.0 * PI * L * L) * (2.0e11 * i / (7850.0 * a)).sqrt());
        }
    }
    eb.sort_by(f64::total_cmp);
    println!("Euler-Bernoulli: {:?}", &eb[..5]);
    for i in 0..5 {
        c.at_most(
            &format!("M2 f_{} held against Euler-Bernoulli", i + 1),
            rel(mh.frequencies[i], eb[i]),
            5e-3,
        );
    }
    // M3.
    c.holds("M3 rigid_free false", !mh.rigid_free, &format!("{}", mh.rigid_free));
    let count = free_rigid_motions(&held, &[]).unwrap().count;
    c.holds(
        "M3 free rigid motions of the held beam",
        count == 0,
        &format!("{count}"),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------ Z1 to Z6

#[test]
fn z1_to_z6_free_rigid_motions_with_rotations() {
    let mut c = Checks::new();
    let count = |m: &SolidModel| free_rigid_motions(m, &[]).unwrap().count;
    let free = beam_line(4, section1(), mat1());
    let z1 = count(&free);
    c.holds("Z1 free beam", z1 == 6, &format!("{z1}"));
    let mut m = beam_line(4, section1(), mat1());
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
    }
    let mut z2m = m.clone();
    z2m.add_support(ElasticSupport::nodal_spring("root", 5, 1e6)).unwrap();
    let z2 = count(&z2m);
    c.holds("Z2 root translations, spring on theta_z", z2 == 2, &format!("{z2}"));
    let mut z3m = m.clone();
    z3m.add_constraint(Constraint::linear(vec![LinearTerm::new(0, 3, 1.0)], 0.0))
        .unwrap();
    z3m.add_constraint(Constraint::displacement("root", 4, 0.0)).unwrap();
    let z3 = count(&z3m);
    c.holds(
        "Z3 root translations, Linear theta_x, prescribed theta_y",
        z3 == 1,
        &format!("{z3}"),
    );
    let z4 = count(&joined(X_ROOT, false).0);
    c.holds("Z4 joined model without the clamp", z4 == 6, &format!("{z4}"));
    let z5 = count(&joined(X_ROOT, true).0);
    c.holds("Z5 joined model", z5 == 0, &format!("{z5}"));
    // Z6: the free fields of Z1 are in the null space of the beam stiffness.
    let frm = free_rigid_motions(&free, &[]).unwrap();
    let k = stiffness(&free).unwrap();
    let kmax = max_abs(k.val.iter().copied());
    let mut worst = 0.0f64;
    for x in frm.fields(&free) {
        let mut kx = vec![0.0; x.len()];
        k.matvec(&x, &mut kx);
        worst = worst.max(max_abs(kx) / (kmax * max_abs(x.iter().copied())));
    }
    c.at_most("Z6 K X c of the free fields", worst, 1e-12);
    c.finish();
}

// ------------------------------------------------------------------------------------------------ S2, N3, N4

#[test]
fn s2_n3_n4_refusals() {
    let mut c = Checks::new();
    // S2: a beam node shared with the block, on the transformation path.
    let hex = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let mut nodes = hex.nodes().to_vec();
    let shared = nodes.iter().position(|y| *y == [2.0, 0.0, 0.0]).unwrap();
    nodes.push([3.0, 0.0, 0.0]);
    let free_end = nodes.len() - 1;
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hex.blocks()[0].connectivity().to_vec())
        .unwrap();
    mesh.add_block(ElementKind::Beam2, vec![free_end, shared]).unwrap();
    mesh.set_block_property(1, BlockProperty::beam(section1(), [0.0, 0.0, 1.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    mesh.add_element_set("structural", vec![2]).unwrap();
    mesh.add_node_set("xmin", hex.node_set("xmin").unwrap().to_vec())
        .unwrap();
    mesh.add_node_set("xmax", hex.node_set("xmax").unwrap().to_vec())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    let steel = Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap();
    m.add_material("all", steel.clone()).unwrap();
    m.add_material("structural", steel).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::rigid_set("xmax", [2.0, 0.5, 0.5]))
        .unwrap();
    let names = |r: &Result<(), FemError>| {
        matches!(r, Err(e @ FemError::BeamSolidNode { node, .. })
            if *node == shared && e.to_string().contains(&format!("node {shared}")))
    };
    let rs = solve_static(&m, &StaticOptions::default()).map(|_| ());
    c.holds("S2 solve_static (transformation)", names(&rs), &format!("{rs:?}"));
    let rm = modal_analysis(&m, &ModalOptions::new(2)).map(|_| ());
    c.holds("S2 modal_analysis (transformation)", names(&rm), &format!("{rm:?}"));
    // Amendment 1: the entry functions of the transformation path called directly.
    let rs = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    c.holds("S2 solve_static_mpc", names(&rs), &format!("{rs:?}"));
    let rm = rcfd_fem::mpc::modal_analysis_mpc(&m, &[], &ModalOptions::new(2)).map(|_| ());
    c.holds("S2 modal_analysis_mpc", names(&rm), &format!("{rm:?}"));
    // N3: a beam model with a cyclic constraint.
    let hex = hex_box([0.0; 3], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
    let (mut m, _, _) = block_and_beam(
        &hex,
        &[[2.0, 0.0, 0.0], [3.0, 0.0, 0.0]],
        section1(),
        block_mat(),
        mat1(),
    );
    m.add_constraint(Constraint::cyclic("ymin", "ymax", [0.0, 0.0, 1.0], [0.0; 3], 4))
        .unwrap();
    let cyclic = |r: &Result<(), FemError>| matches!(r, Err(FemError::NotYetSupported { variant: "Cyclic", package, .. }) if *package == beam::FOLLOW_UP);
    let rs = solve_static(&m, &StaticOptions::default()).map(|_| ());
    c.holds("N3 cyclic with a beam: solve_static", cyclic(&rs), &format!("{rs:?}"));
    let rm = modal_analysis(&m, &ModalOptions::new(2)).map(|_| ());
    c.holds("N3 cyclic with a beam: modal_analysis", cyclic(&rm), &format!("{rm:?}"));
    // N4: the joined model on paths that do not take beams.
    let (jm, _, _, _) = joined(X_ROOT, true);
    let r = rcfd_fem::nonlinear::initial_states(&jm).map(|_| ());
    let ok =
        matches!(&r, Err(FemError::NotYetSupported { variant: "Beam2", package, .. }) if *package == beam::FOLLOW_UP);
    c.holds("N4 nonlinear initial states", ok, &format!("{r:?}"));
    let pcg = StaticOptions::with_solver(StaticSolver::PreconditionedCg {
        preconditioner: CgPreconditioner::BlockJacobi,
        tolerance: 1e-10,
        max_iterations: 100,
    });
    let r = solve_static(&jm, &pcg).map(|_| ());
    c.holds(
        "N4 preconditioned CG on the transformation",
        matches!(&r, Err(FemError::InvalidValue { key: "solver", .. })),
        &format!("{r:?}"),
    );
    c.finish();
}
