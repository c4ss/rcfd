//! F-514 (ADR 0422): uniform line loads and gravity on Beam2 elements. Checks L1 to L6, G1 to G3, P1 to P4, B4 and the
//! negative controls N1 to N4 of the registration (B1 to B3 are the existing tests).

use rcfd_core::vec3::{cross, dot, norm, Vec3};
use rcfd_fem::beam::{self, SectionForces};
use rcfd_fem::builders::hex_box;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, ElementKind, FemError, Load, Material, SetKind, SolidMesh, SolidModel,
    SolidResult,
};

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

/// xorshift64*, as `si_beam.rs` (the seed of the registration: `0x0422_0514`).
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

fn scale(a: f64, v: Vec3) -> Vec3 {
    [a * v[0], a * v[1], a * v[2]]
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

/// The frame of ADR 0417 arithmetic 1, computed here independently of the library.
fn axes(x0: Vec3, x1: Vec3, v: Vec3) -> [Vec3; 3] {
    let e1 = unit(sub(x1, x0));
    let e3 = unit(cross(e1, v));
    [e1, cross(e3, e1), e3]
}

fn to_local(ax: &[Vec3; 3], g: Vec3) -> Vec3 {
    [dot(ax[0], g), dot(ax[1], g), dot(ax[2], g)]
}

fn to_global(ax: &[Vec3; 3], l: Vec3) -> Vec3 {
    add(add(scale(l[0], ax[0]), scale(l[1], ax[1])), scale(l[2], ax[2]))
}

fn rel(meas: f64, exact: f64) -> f64 {
    ((meas - exact) / exact).abs()
}

/// A beam mesh: nodes, element node pairs, one block with the section and orientation; element set "all", node sets "all",
/// "root" (node 0), "tip" (last node), "ends" (first and last).
fn beam_mesh(nodes: Vec<Vec3>, elements: &[[usize; 2]], section: BeamSection, v: Vec3) -> SolidMesh {
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, elements.iter().flatten().copied().collect())
        .unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section, v).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..elements.len()).collect()).unwrap();
    let n = mesh.nodes().len();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("root", vec![0]).unwrap();
    mesh.add_node_set("tip", vec![n - 1]).unwrap();
    mesh.add_node_set("ends", vec![0, n - 1]).unwrap();
    mesh
}

fn beam_model(nodes: Vec<Vec3>, elements: &[[usize; 2]], section: BeamSection, v: Vec3, mat: Material) -> SolidModel {
    let mut m = SolidModel::new(beam_mesh(nodes, elements, section, v));
    m.add_material("all", mat).unwrap();
    m
}

fn clamp(m: &mut SolidModel, set: &str) {
    for c in 0..6 {
        m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
}

fn node6(r: &SolidResult, node: usize) -> (Vec3, Vec3) {
    let d = r.displacement(node);
    assert_eq!(d.len(), 6, "node {node} has six unknowns");
    ([d[0], d[1], d[2]], [d[3], d[4], d[5]])
}

fn reaction6(r: &SolidResult, node: usize) -> (Vec3, Vec3) {
    let start = r.dofs().equation(node, 0).unwrap();
    let v = &r.reactions().unwrap()[start..start + 6];
    ([v[0], v[1], v[2]], [v[3], v[4], v[5]])
}

/// The sum of the reaction forces over every node of a result.
fn reaction_force_sum(r: &SolidResult, n_nodes: usize) -> Vec3 {
    let mut s = [0.0; 3];
    for node in 0..n_nodes {
        for (k, sk) in s.iter_mut().enumerate() {
            *sk += r.reactions().unwrap()[r.dofs().equation(node, k).unwrap()];
        }
    }
    s
}

// ------------------------------------------------------------------------------------------------ the one-element case

const SEED: u64 = 0x0422_0514;
const L1: f64 = 0.8;
const E1: f64 = 2.1e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const Q_LOCAL: Vec3 = [300.0, -800.0, 1100.0];
const GRAVITY: Vec3 = [1.3, -2.1, -9.81];

fn section1() -> BeamSection {
    BeamSection::new(0.012, 3.0e-6, 5.0e-6, 4.0e-6, 0.85, 0.75).unwrap()
}

fn mat1() -> Material {
    Material::isotropic_elastic(E1, NU, RHO, 0.0).unwrap()
}

/// The seeded geometry (drawn as in `si_beam.rs`): `(x0, e1, v)`.
fn seeded() -> (Vec3, Vec3, Vec3) {
    let mut rng = Rng(SEED);
    let x0 = rng.vec();
    let e1 = unit(rng.vec());
    let mut v = unit(rng.vec());
    while norm(cross(e1, v)) < 0.3 {
        v = unit(rng.vec());
    }
    (x0, e1, v)
}

fn g_of(e: f64) -> f64 {
    e / (2.0 * (1.0 + NU))
}

#[test]
fn l1_to_l6_n1_n4_one_element_line_load() {
    let mut c = Checks::new();
    let (x0, e1, v) = seeded();
    let x1 = add(x0, scale(L1, e1));
    let ax = axes(x0, x1, v);
    println!("seeded: x0 {x0:?}, e1 {:?}, e2 {:?}, e3 {:?}", ax[0], ax[1], ax[2]);
    let mut model = beam_model(vec![x0, x1], &[[0, 1]], section1(), v, mat1());
    clamp(&mut model, "root");
    let q = to_global(&ax, Q_LOCAL);
    model.add_load(Load::line_load("all", q)).unwrap();
    let r = solve_static(&model, &StaticOptions::default()).unwrap();
    let (ut, ur) = node6(&r, 1);
    let (ul, rl) = (to_local(&ax, ut), to_local(&ax, ur));
    let s = section1();
    let g = g_of(E1);
    let (l, a) = (L1, s.area());
    let [qx, qy, qz] = Q_LOCAL;
    let v_eb = qy * l.powi(4) / (8.0 * E1 * s.i_z());
    let w_eb = qz * l.powi(4) / (8.0 * E1 * s.i_y());
    let v_exact = v_eb + qy * l * l / (2.0 * s.shear_y() * g * a);
    let w_exact = w_eb + qz * l * l / (2.0 * s.shear_z() * g * a);
    c.at_most("L1 tip deflection v", rel(ul[1], v_exact), 1e-10);
    c.at_most("L2 tip deflection w", rel(ul[2], w_exact), 1e-10);
    c.at_most(
        "L3 tip rotation theta_z",
        rel(rl[2], qy * l.powi(3) / (6.0 * E1 * s.i_z())),
        1e-10,
    );
    c.at_most(
        "L3 tip rotation theta_y",
        rel(rl[1], -qz * l.powi(3) / (6.0 * E1 * s.i_y())),
        1e-10,
    );
    c.at_most("L3 elongation u", rel(ul[0], qx * l * l / (2.0 * E1 * a)), 1e-10);
    c.at_most(
        "L3 twist |theta_x| / max tip rotation",
        rl[0].abs() / rl[1].abs().max(rl[2].abs()),
        1e-10,
    );
    // N4: without the shear term.
    c.above("N4 v against q L^4 / (8 E I_z) only", rel(ul[1], v_eb), 1e-3);
    c.above("N4 w against q L^4 / (8 E I_y) only", rel(ul[2], w_eb), 1e-3);
    // L4: section forces with the element load.
    let sf = beam::section_forces(&model, r.displacement_values()).unwrap();
    assert_eq!(sf.len(), 1);
    let SectionForces { start, end } = sf[0].1;
    let qmax = max_abs(Q_LOCAL);
    let root = [l * qx, l * qy, l * qz, 0.0, -qz * l * l / 2.0, qy * l * l / 2.0];
    let err = |got: &[f64; 6], want: &[f64; 6]| {
        let ef = max_abs((0..3).map(|i| got[i] - want[i])) / (l * qmax);
        let em = max_abs((3..6).map(|i| got[i] - want[i])) / (l * l * qmax);
        ef.max(em)
    };
    println!("section forces start {start:?}, end {end:?}");
    c.at_most("L4 root section forces", err(&start, &root), 1e-10);
    c.at_most("L4 tip section forces", err(&end, &[0.0; 6]), 1e-10);
    // L5: reactions.
    let (rf, rm) = reaction6(&r, 0);
    let qn = norm(q);
    let ef = norm(add(rf, scale(l, q))) / (l * qn);
    let em = norm(add(rm, scale(l * l / 2.0, cross(ax[0], q)))) / (l * l * qn);
    c.at_most("L5 reaction force + L q", ef, 1e-10);
    c.at_most("L5 reaction moment + (L^2 / 2) e_1 x q", em, 1e-10);
    // L6: the library's consistent load against the Gauss integral of the interdependent shape functions.
    for (name, len) in [("seeded element", L1), ("stubby element", 0.05)] {
        let y1 = add(x0, scale(len, e1));
        let fg = beam::line_load_vector(&[x0, y1], q).unwrap();
        let mut fl = [0.0; 12];
        for b in 0..4 {
            let lb = to_local(&ax, [fg[3 * b], fg[3 * b + 1], fg[3 * b + 2]]);
            fl[3 * b..3 * b + 3].copy_from_slice(&lb);
        }
        let mut want = [0.0; 12];
        want[0] = qx * len / 2.0;
        want[6] = qx * len / 2.0;
        // (plane, I, kappa, load, deflection index, rotation index, sign of psi)
        for (ei, kappa, p, d, rr, sg) in [
            (E1 * s.i_z(), s.shear_y(), qy, 1usize, 5usize, 1.0),
            (E1 * s.i_y(), s.shear_z(), qz, 2, 4, -1.0),
        ] {
            let phi = 12.0 * ei / (kappa * g * a * len * len);
            let fp = gauss_plane_load(phi, len, p);
            println!("L6 {name}: phi {phi:.4}, plane load {fp:?}");
            want[d] = fp[0];
            want[rr] = sg * fp[1];
            want[6 + d] = fp[2];
            want[6 + rr] = sg * fp[3];
        }
        let d = max_abs(fl.iter().zip(&want).map(|(x, y)| x - y)) / max_abs(want);
        c.at_most(&format!("L6 {name}: consistent load against Gauss"), d, 1e-13);
    }
    // N1: the lumped load.
    let mut lumped = beam_model(vec![x0, x1], &[[0, 1]], section1(), v, mat1());
    clamp(&mut lumped, "root");
    lumped.add_load(Load::nodal_force("all", scale(l / 2.0, q))).unwrap();
    let rl_ = solve_static(&lumped, &StaticOptions::default()).unwrap();
    let vl = to_local(&ax, node6(&rl_, 1).0)[1];
    c.above("N1 lumped load: tip v", rel(vl, v_exact), 0.1);
    c.finish();
}

/// The consistent load `p int N_d dx` of one bending plane in `(d_0, psi_0, d_1, psi_1)` by four-point Gauss from
/// `plane_shape_coefficients` (ADR 0422 arithmetic 1), independently of `line_load_vector`.
fn gauss_plane_load(phi: f64, l: f64, p: f64) -> [f64; 4] {
    let c = beam::plane_shape_coefficients(phi);
    let xs = [
        -0.861_136_311_594_052_6,
        -0.339_981_043_584_856_3,
        0.339_981_043_584_856_3,
        0.861_136_311_594_052_6,
    ];
    let ws = [
        0.347_854_845_137_453_9,
        0.652_145_154_862_546_1,
        0.652_145_154_862_546_1,
        0.347_854_845_137_453_9,
    ];
    let scale = [1.0, l, 1.0, l];
    let mut f = [0.0; 4];
    for (x, w) in xs.iter().zip(ws) {
        let t = 0.5 * (x + 1.0);
        let pd = [1.0, t, t * t, t * t * t];
        for q in 0..4 {
            let nd: f64 = (0..4).map(|j| pd[j] * c[j][q] * scale[q]).sum();
            f[q] += 0.5 * w * l * p * nd;
        }
    }
    f
}

// ------------------------------------------------------------------------------------------------------- gravity

fn space_frame() -> (Vec<Vec3>, Vec<[usize; 2]>) {
    (
        vec![[0.0, 0.0, 0.0], [0.8, 0.0, 0.0], [0.8, 0.6, 0.0], [0.8, 0.6, 0.5]],
        vec![[0, 1], [1, 2], [2, 3]],
    )
}

/// The pinned-pinned beam of the registration: `n` elements along `e_x`, the rectangle, `u_x, u_y, u_z` held at both ends
/// and `theta_x` at node 0.
fn pinned(n: usize) -> (SolidModel, BeamSection) {
    let b = 0.02 * 12f64.sqrt();
    let s = beam::rectangle_section(b, 0.04, 1e-6, 5.0 / 6.0).unwrap();
    let nodes: Vec<Vec3> = (0..=n).map(|i| [i as f64 / n as f64, 0.0, 0.0]).collect();
    let elements: Vec<[usize; 2]> = (0..n).map(|i| [i, i + 1]).collect();
    let mat = Material::isotropic_elastic(2.0e11, NU, RHO, 0.0).unwrap();
    let mut m = beam_model(nodes, &elements, s, [0.0, 0.0, 1.0], mat);
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("ends", comp, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::displacement("root", 3, 0.0)).unwrap();
    (m, s)
}

/// The Hex8 box `[2, 1, 1]` clamped at `x = 0` and a separate one-element beam from `(3, 0, 0)` to `(4, 0, 0)` clamped at
/// its first node, in one model; also the box alone. Returns (mixed, box alone, the beam's nodes).
fn mixed() -> (SolidModel, SolidModel, [usize; 2]) {
    let hex = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let mut alone = SolidModel::new(hex.clone());
    alone.add_material("all", mat1()).unwrap();
    alone.add_load(Load::gravity(GRAVITY)).unwrap();
    let mut nodes = hex.nodes().to_vec();
    let nb = nodes.len();
    nodes.push([3.0, 0.0, 0.0]);
    nodes.push([4.0, 0.0, 0.0]);
    let xmin: Vec<usize> = (0..nb).filter(|&n| nodes[n][0] == 0.0).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hex.blocks()[0].connectivity().to_vec())
        .unwrap();
    mesh.add_block(ElementKind::Beam2, vec![nb, nb + 1]).unwrap();
    mesh.set_block_property(1, BlockProperty::beam(section1(), [0.0, 0.0, 1.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    mesh.add_element_set("structural", vec![2]).unwrap();
    mesh.add_node_set("xmin", xmin).unwrap();
    mesh.add_node_set("beamroot", vec![nb]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat1()).unwrap();
    m.add_material("structural", mat1()).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
    }
    clamp(&mut m, "beamroot");
    (m, alone, [nb, nb + 1])
}

#[test]
fn g1_g2_g3_b4_gravity_on_beams() {
    let mut c = Checks::new();
    let gn = norm(GRAVITY);
    // (i) the space frame.
    let (nodes, elements) = space_frame();
    let s = section1();
    let mut frame = beam_model(nodes.clone(), &elements, s, [1.0, 2.0, 3.0], mat1());
    clamp(&mut frame, "root");
    frame.add_load(Load::gravity(GRAVITY)).unwrap();
    let r = solve_static(&frame, &StaticOptions::default()).unwrap();
    let ltot: f64 = elements.iter().map(|e| norm(sub(nodes[e[1]], nodes[e[0]]))).sum();
    let w = scale(RHO * s.area() * ltot, GRAVITY);
    let (rf, rm) = reaction6(&r, 0);
    c.at_most(
        "G1 (i) space frame: |sum R + rho A L g| / |W|",
        norm(add(rf, w)) / norm(w),
        1e-12,
    );
    let mut arm = [0.0; 3];
    let mut rmax = 0.0f64;
    for e in &elements {
        let le = norm(sub(nodes[e[1]], nodes[e[0]]));
        let ce = scale(0.5, add(nodes[e[0]], nodes[e[1]]));
        rmax = rmax.max(norm(ce));
        arm = add(arm, cross(ce, scale(RHO * s.area() * le, GRAVITY)));
    }
    c.at_most(
        "G2 (i) space frame: reaction moment + moment of the weight",
        norm(add(rm, arm)) / (norm(w) * rmax),
        1e-12,
    );
    // G3: gravity against the line load rho A g.
    let fg = nodal_load_vector(&frame, &[]).unwrap();
    let mut by_line = beam_model(nodes.clone(), &elements, s, [1.0, 2.0, 3.0], mat1());
    clamp(&mut by_line, "root");
    let ra = RHO * s.area();
    by_line
        .add_load(Load::line_load(
            "all",
            [ra * GRAVITY[0], ra * GRAVITY[1], ra * GRAVITY[2]],
        ))
        .unwrap();
    let fl = nodal_load_vector(&by_line, &[]).unwrap();
    c.at_most(
        "G3 gravity against the line load rho A g",
        max_abs(fg.iter().zip(&fl).map(|(a, b)| a - b)) / max_abs(fl.iter().copied()),
        1e-15,
    );
    // (ii) the pinned-pinned beam, 20 elements.
    let (mut pp, ps) = pinned(20);
    pp.add_load(Load::gravity(GRAVITY)).unwrap();
    let r = solve_static(&pp, &StaticOptions::default()).unwrap();
    let w = scale(RHO * ps.area() * 1.0, GRAVITY);
    let sum = reaction_force_sum(&r, 21);
    c.at_most(
        "G1 (ii) pinned-pinned: |sum R + rho A L g| / |W|",
        norm(add(sum, w)) / norm(w),
        1e-12,
    );
    for node in [0, 20] {
        let (rf, _) = reaction6(&r, node);
        c.at_most(
            &format!("G2 (ii) pinned-pinned: reaction at node {node} + W / 2"),
            norm(add(rf, scale(0.5, w))) / norm(w),
            1e-12,
        );
    }
    // (iii) the box and a separate beam in one model.
    let (mut mx, alone, beam_nodes) = mixed();
    mx.add_load(Load::gravity(GRAVITY)).unwrap();
    let r = solve_static(&mx, &StaticOptions::default()).unwrap();
    let w = scale(RHO * (2.0 + s.area() * 1.0), GRAVITY);
    let n_nodes = beam_nodes[1] + 1;
    let sum = reaction_force_sum(&r, n_nodes);
    c.at_most(
        "G1 (iii) box and beam: |sum R + rho (V + A L) g| / |W|",
        norm(add(sum, w)) / norm(w),
        1e-12,
    );
    // B4: the box entries are those of the box alone, bit for bit.
    let fm = nodal_load_vector(&mx, &[]).unwrap();
    let fa = nodal_load_vector(&alone, &[]).unwrap();
    let dm = mx.dof_map();
    let da = alone.dof_map();
    let same = (0..beam_nodes[0])
        .all(|n| (0..3).all(|k| fm[dm.equation(n, k).unwrap()].to_bits() == fa[da.equation(n, k).unwrap()].to_bits()));
    c.holds(
        "B4 box entries bit for bit",
        same,
        &format!("{} box nodes", beam_nodes[0]),
    );
    println!("gravity |g| {gn}");
    c.finish();
}

// ------------------------------------------------------------------------------------------------- pinned-pinned

#[test]
fn p1_to_p4_n4_pinned_pinned_uniform_load() {
    let mut c = Checks::new();
    let q_l: Vec3 = [0.0, -1000.0, 600.0];
    let (e, l) = (2.0e11, 1.0);
    let g = g_of(e);
    for n in [1usize, 2, 3, 4, 7, 10, 20, 64] {
        let (mut m, s) = pinned(n);
        let ax = axes([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        let q = to_global(&ax, q_l);
        m.add_load(Load::line_load("all", q)).unwrap();
        let r = solve_static(&m, &StaticOptions::default()).unwrap();
        let a = s.area();
        // (plane, load, E I, k_s): x-y with v, theta_z; x-z with w, -theta_y.
        let planes = [
            (q_l[1], e * s.i_z(), s.shear_y() * g * a),
            (q_l[2], e * s.i_y(), s.shear_z() * g * a),
        ];
        let mut ed = [0.0f64; 2];
        let mut ep = [0.0f64; 2];
        let mut mids = [(0.0, 0.0); 2];
        for k in 0..=n {
            let x = l * k as f64 / n as f64;
            let (ut, ur) = node6(&r, k);
            let (ul, rl) = (to_local(&ax, ut), to_local(&ax, ur));
            for (pi, &(p, ei, ks)) in planes.iter().enumerate() {
                let d = p * x * (l.powi(3) - 2.0 * l * x * x + x.powi(3)) / (24.0 * ei) + p * x * (l - x) / (2.0 * ks);
                let psi = p * (l.powi(3) - 6.0 * l * x * x + 4.0 * x.powi(3)) / (24.0 * ei);
                let wmid = 5.0 * p * l.powi(4) / (384.0 * ei) + p * l * l / (8.0 * ks);
                let psi0 = p * l.powi(3) / (24.0 * ei);
                let (dm, rm) = if pi == 0 { (ul[1], rl[2]) } else { (ul[2], -rl[1]) };
                ed[pi] = ed[pi].max(((dm - d) / wmid).abs());
                ep[pi] = ep[pi].max(((rm - psi) / psi0).abs());
                if 2 * k == n {
                    let eb = 5.0 * p * l.powi(4) / (384.0 * ei);
                    mids[pi] = (rel(dm, wmid), rel(dm, eb));
                }
            }
        }
        for (pi, name) in [(0, "x-y v"), (1, "x-z w")] {
            if n % 2 == 0 {
                c.at_most(&format!("P1 n = {n} midspan {name}"), mids[pi].0, 1e-10);
                if n == 2 {
                    c.above(
                        &format!("N4 n = {n} midspan {name} against 5 q L^4 / (384 E I) only"),
                        mids[pi].1,
                        1e-3,
                    );
                }
            }
            c.at_most(&format!("P2 n = {n} nodal deflections {name}"), ed[pi], 1e-10);
            c.at_most(&format!("P2 n = {n} nodal rotations {name}"), ep[pi], 1e-10);
        }
        // P3: section forces at both ends of every element against statics.
        let sf = beam::section_forces(&m, r.displacement_values()).unwrap();
        let qn = max_abs(q_l);
        let mut ef = 0.0f64;
        let mut em = 0.0f64;
        let e1q = cross([1.0, 0.0, 0.0], q_l);
        for (el, forces) in &sf {
            for (x, got) in [
                (l * *el as f64 / n as f64, forces.start),
                (l * (*el + 1) as f64 / n as f64, forces.end),
            ] {
                let sv = scale(l / 2.0 - x, q_l);
                let mv = scale(-x * (l - x) / 2.0, e1q);
                ef = ef.max(max_abs((0..3).map(|i| got[i] - sv[i])) / (qn * l / 2.0));
                em = em.max(max_abs((0..3).map(|i| got[3 + i] - mv[i])) / (qn * l * l / 8.0));
            }
        }
        c.at_most(&format!("P3 n = {n} section forces"), ef, 1e-10);
        c.at_most(&format!("P3 n = {n} section moments"), em, 1e-10);
        // P4: reactions at the ends.
        let mut er = 0.0f64;
        for node in [0, n] {
            let (rf, _) = reaction6(&r, node);
            er = er.max(norm(add(rf, scale(l / 2.0, q))) / (norm(q) * l / 2.0));
        }
        c.at_most(&format!("P4 n = {n} end reactions"), er, 1e-10);
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------- N2, N3

fn solid_routines<T: std::fmt::Debug>(r: &Result<T, FemError>) -> bool {
    matches!(r, Err(FemError::Element { source, .. })
        if matches!(source.as_ref(), FemError::NotYetSupported { package, .. } if *package == beam::SOLID_ROUTINES))
}

#[test]
fn n2_n3_other_loads_and_invalid_line_loads() {
    let mut c = Checks::new();
    let (x0, e1, v) = seeded();
    let x1 = add(x0, scale(L1, e1));
    let base = || {
        let mut m = beam_model(vec![x0, x1], &[[0, 1]], section1(), v, mat1());
        clamp(&mut m, "root");
        m
    };
    let mut m = base();
    m.add_load(Load::centrifugal([0.0, 0.0, 1.0], [0.0; 3], 100.0)).unwrap();
    let r = solve_static(&m, &StaticOptions::default());
    c.holds("N2 centrifugal on a beam", solid_routines(&r), &format!("{r:?}"));
    let mut m = base();
    m.add_load(Load::body_force_nodal(vec![[0.0, 0.0, -1.0e4]; 2])).unwrap();
    let r = solve_static(&m, &StaticOptions::default());
    c.holds("N2 nodal body force on a beam", solid_routines(&r), &format!("{r:?}"));
    let mut m = base();
    m.add_load(Load::temperature(vec![350.0, 360.0], 300.0)).unwrap();
    let r = solve_static(&m, &StaticOptions::default());
    c.holds("N2 temperature on a beam", solid_routines(&r), &format!("{r:?}"));
    // N3: invalid line loads.
    let mut m = base();
    let r = m.add_load(Load::line_load("nope", [0.0, 1.0, 0.0]));
    c.holds(
        "N3 unknown element set",
        matches!(
            &r,
            Err(FemError::UnknownSet {
                kind: SetKind::Element,
                ..
            })
        ),
        &format!("{r:?}"),
    );
    let r = m.add_load(Load::line_load("all", [f64::NAN, 0.0, 0.0]));
    c.holds(
        "N3 non-finite line load",
        matches!(
            &r,
            Err(FemError::InvalidValue {
                key: "force per length",
                ..
            })
        ),
        &format!("{r:?}"),
    );
    let (mut mx, _, _) = mixed();
    let r = mx.add_load(Load::line_load("all", [0.0, 1.0, 0.0]));
    let ok =
        matches!(&r, Err(e @ FemError::InvalidValue { key: "line load", .. }) if e.to_string().contains("element 0"));
    c.holds("N3 line load on a set of Hex8 elements", ok, &format!("{r:?}"));
    c.finish();
}
