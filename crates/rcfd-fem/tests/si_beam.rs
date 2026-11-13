//! F-391 (ADR 0417): six unknowns per node and the two-node Timoshenko beam Beam2. Checks D1, C1 to C9, R1, A1, A2, F1 to
//! F4, S1, K1, K2, M1 and the negative controls N1 to N4 of the registration (B1 to B3 are the existing tests).

use rcfd_core::vec3::{cross, dot, norm, Vec3};
use rcfd_fem::assemble::stiffness;
use rcfd_fem::beam::{self, SectionForces};
use rcfd_fem::builders::hex_box;
use rcfd_fem::mass::{consistent, mass_properties};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::solve::{solve_static, CgPreconditioner, StaticOptions, StaticSolver};
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, DofMap, ElasticSupport, ElementKind, FemError, Load, Material, SolidMesh,
    SolidModel, SolidResult,
};
use rcfd_linalg::amg_elastic::ElasticAmgSettings;
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

/// xorshift64* (the seed of the registration: `0x0417_0391`).
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

/// The frame of the registration (arithmetic 1), computed here independently of the library.
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

/// A beam model: nodes, element node pairs, one block with the section and orientation, the material on "all".
fn beam_model(nodes: Vec<Vec3>, elements: &[[usize; 2]], section: BeamSection, v: Vec3, mat: Material) -> SolidModel {
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
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    m
}

fn clamp_root(m: &mut SolidModel) {
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("root", c, 0.0)).unwrap();
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

// ------------------------------------------------------------------------------------------------ the one-element case

const SEED: u64 = 0x0417_0391;
const L1: f64 = 0.8;
const E1: f64 = 2.1e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const F_LOCAL: Vec3 = [1200.0, -2500.0, 3700.0];
const T_LOCAL: f64 = 450.0;

fn section1() -> BeamSection {
    BeamSection::new(0.012, 3.0e-6, 5.0e-6, 4.0e-6, 0.85, 0.75).unwrap()
}

fn mat1() -> Material {
    Material::isotropic_elastic(E1, NU, RHO, 0.0).unwrap()
}

/// The seeded geometry of arithmetic 6: `(x0, x1, v)`.
fn seeded() -> (Vec3, Vec3, Vec3) {
    let mut rng = Rng(SEED);
    let x0 = rng.vec();
    let e1 = unit(rng.vec());
    let mut v = unit(rng.vec());
    while norm(cross(e1, v)) < 0.3 {
        v = unit(rng.vec());
    }
    (x0, add(x0, scale(L1, e1)), v)
}

/// The clamped one-element model with the tip loads of arithmetic 6 (global vectors), and its axes.
fn one_element() -> (SolidModel, [Vec3; 3], Vec3, Vec3) {
    let (x0, x1, v) = seeded();
    let ax = axes(x0, x1, v);
    let mut m = beam_model(vec![x0, x1], &[[0, 1]], section1(), v, mat1());
    clamp_root(&mut m);
    let f = to_global(&ax, F_LOCAL);
    let mm = to_global(&ax, [T_LOCAL, 0.0, 0.0]);
    m.add_load(Load::nodal_force("tip", f)).unwrap();
    m.add_load(Load::nodal_moment("tip", mm)).unwrap();
    (m, ax, f, mm)
}

fn rel(meas: f64, exact: f64) -> f64 {
    ((meas - exact) / exact).abs()
}

#[test]
fn c1_to_c9_n4_one_element_any_orientation() {
    let mut c = Checks::new();
    let (model, ax, f, mm) = one_element();
    let (x0, x1, _) = seeded();
    println!("seeded: x0 {x0:?}, e1 {:?}, e2 {:?}, e3 {:?}", ax[0], ax[1], ax[2]);
    let r = solve_static(&model, &StaticOptions::default()).unwrap();
    let (ut, ur) = node6(&r, 1);
    let (ul, rl) = (to_local(&ax, ut), to_local(&ax, ur));
    let s = section1();
    let g = E1 / (2.0 * (1.0 + NU));
    let (l, a) = (L1, s.area());
    let [fx, fy, fz] = F_LOCAL;
    let v_exact = fy * (l.powi(3) / (3.0 * E1 * s.i_z()) + l / (s.shear_y() * g * a));
    let w_exact = fz * (l.powi(3) / (3.0 * E1 * s.i_y()) + l / (s.shear_z() * g * a));
    c.at_most("C1 tip deflection v", rel(ul[1], v_exact), 1e-10);
    c.at_most("C2 tip deflection w", rel(ul[2], w_exact), 1e-10);
    c.at_most(
        "C3 tip rotation theta_z",
        rel(rl[2], fy * l * l / (2.0 * E1 * s.i_z())),
        1e-10,
    );
    c.at_most(
        "C4 tip rotation theta_y",
        rel(rl[1], -fz * l * l / (2.0 * E1 * s.i_y())),
        1e-10,
    );
    c.at_most("C5 twist theta_x", rel(rl[0], T_LOCAL * l / (g * s.torsion())), 1e-10);
    c.at_most("C6 elongation u", rel(ul[0], fx * l / (E1 * a)), 1e-10);
    // N4: the closed form without the shear term.
    c.above(
        "N4 v against P L^3 / (3 E I_z) only",
        rel(ul[1], fy * l.powi(3) / (3.0 * E1 * s.i_z())),
        1e-3,
    );
    c.above(
        "N4 w against P L^3 / (3 E I_y) only",
        rel(ul[2], fz * l.powi(3) / (3.0 * E1 * s.i_y())),
        1e-3,
    );
    // C7: section forces.
    let sf = beam::section_forces(&model, r.displacement_values()).unwrap();
    assert_eq!(sf.len(), 1);
    let SectionForces { start, end } = sf[0].1;
    let fmax = max_abs(F_LOCAL);
    let root = [fx, fy, fz, T_LOCAL, -l * fz, l * fy];
    let tip = [fx, fy, fz, T_LOCAL, 0.0, 0.0];
    let err = |got: &[f64; 6], want: &[f64; 6]| {
        let ef = max_abs((0..3).map(|i| got[i] - want[i])) / fmax;
        let em = max_abs((3..6).map(|i| got[i] - want[i])) / (l * fmax);
        ef.max(em)
    };
    println!("section forces start {start:?}, end {end:?}");
    c.at_most("C7 root section forces", err(&start, &root), 1e-10);
    c.at_most("C7 tip section forces", err(&end, &tip), 1e-10);
    // C8: reactions balance the loads.
    let (rf, rm) = reaction6(&r, 0);
    let ef = max_abs((0..3).map(|i| rf[i] + f[i])) / fmax;
    let arm = cross(sub(x1, x0), f);
    let em = max_abs((0..3).map(|i| rm[i] + arm[i] + mm[i])) / (l * fmax);
    c.at_most("C8 reaction forces + loads", ef, 1e-10);
    c.at_most("C8 reaction moments + moments of the loads about the root", em, 1e-10);
    let s1 = r.dofs().equation(1, 0).unwrap();
    c.holds(
        "C8 reactions at the free equations",
        r.reactions().unwrap()[s1..s1 + 6].iter().all(|&x| x == 0.0),
        "exactly 0",
    );
    // C9: Jacobi CG on the same model.
    let cg = solve_static(
        &model,
        &StaticOptions::with_solver(StaticSolver::ConjugateGradient {
            tolerance: 1e-13,
            max_iterations: 100,
        }),
    )
    .unwrap();
    let umax = max_abs(r.displacement_values().iter().copied());
    let d = max_abs(
        r.displacement_values()
            .iter()
            .zip(cg.displacement_values())
            .map(|(a, b)| a - b),
    ) / umax;
    c.at_most("C9 CG against the direct solve", d, 1e-8);
    c.finish();
}

// -------------------------------------------------------------------------------------------------- K1, K2, M1, D1

#[test]
fn k1_k2_m1_element_matrices() {
    let mut c = Checks::new();
    let s = section1();
    let mat = mat1();
    let g = E1 / (2.0 * (1.0 + NU));
    let kl = beam::local_stiffness(L1, &s, &mat);
    let kmax = max_abs(kl.values().iter().copied());
    // K1: the bending blocks against the energy of the interdependent shape functions.
    let gauss = [
        (-0.861_136_311_594_052_6, 0.347_854_845_137_453_9),
        (-0.339_981_043_584_856_3, 0.652_145_154_862_546_1),
        (0.339_981_043_584_856_3, 0.652_145_154_862_546_1),
        (0.861_136_311_594_052_6, 0.347_854_845_137_453_9),
    ];
    for (name, ei, ks, idx, sign) in [
        ("x-y", E1 * s.i_z(), s.shear_y() * g * s.area(), [1usize, 5, 7, 11], 1.0),
        ("x-z", E1 * s.i_y(), s.shear_z() * g * s.area(), [2, 4, 8, 10], -1.0),
    ] {
        let l = L1;
        let phi = 12.0 * ei / (ks * l * l);
        let cc = beam::plane_shape_coefficients(phi);
        let sc = [1.0, l, 1.0, l];
        let mut ke = [[0.0; 4]; 4];
        for (x, w) in gauss {
            let xi = 0.5 * (x + 1.0);
            let wl = 0.5 * w * l;
            let mut dpsi = [0.0; 4];
            let mut gam = [0.0; 4];
            for q in 0..4 {
                let b: Vec<f64> = (0..4).map(|j| cc[j][q] * sc[q]).collect();
                dpsi[q] = (2.0 * b[2] + 6.0 * b[3] * xi) / (l * l);
                gam[q] = -b[3] * phi / (2.0 * l);
            }
            for i in 0..4 {
                for j in 0..4 {
                    ke[i][j] += wl * (ei * dpsi[i] * dpsi[j] + ks * gam[i] * gam[j]);
                }
            }
        }
        let sg = [1.0, sign, 1.0, sign];
        let mut worst: f64 = 0.0;
        for i in 0..4 {
            for j in 0..4 {
                worst = worst.max((kl.get(idx[i], idx[j]) - sg[i] * sg[j] * ke[i][j]).abs());
            }
        }
        c.at_most(&format!("K1 {name} plane against the energy"), worst / kmax, 1e-12);
    }
    let symmetric = (0..12).all(|i| (0..12).all(|j| kl.get(i, j).to_bits() == kl.get(j, i).to_bits()));
    c.holds("K1 K_l symmetric", symmetric, "bit for bit");
    // K2: the global element stiffness annihilates the rigid motions.
    let (x0, x1, v) = seeded();
    let kg = beam::element_stiffness(&[x0, x1], &s, v, &mat).unwrap();
    let kgmax = max_abs(kg.values().iter().copied());
    let mut worst: f64 = 0.0;
    for k in 0..6 {
        let mut r = vec![0.0; 12];
        for (a, x) in [x0, x1].iter().enumerate() {
            if k < 3 {
                r[6 * a + k] = 1.0;
            } else {
                let mut w = [0.0; 3];
                w[k - 3] = 1.0;
                let t = cross(w, sub(*x, x0));
                r[6 * a..6 * a + 3].copy_from_slice(&t);
                r[6 * a + k] = 1.0;
            }
        }
        let rmax = max_abs(r.iter().copied());
        worst = worst.max(max_abs(kg.matvec(&r)) / (kgmax * rmax));
    }
    c.at_most("K2 max |K r| / (max |K| max |r|)", worst, 1e-12);
    // M1: mass properties of the seeded element.
    let model = beam_model(vec![x0, x1], &[[0, 1]], s, v, mat);
    let m = consistent(&model).unwrap();
    let p = mass_properties(&model, &m).unwrap();
    let mass = RHO * s.area() * L1;
    c.at_most("M1 mass", rel(p.mass, mass), 1e-12);
    let mid = scale(0.5, add(x0, x1));
    c.at_most("M1 centre", max_abs((0..3).map(|i| p.centre[i] - mid[i])) / L1, 1e-12);
    let ax = axes(x0, x1, v);
    let dl = [
        RHO * (s.i_y() + s.i_z()) * L1,
        mass * L1 * L1 / 12.0 + RHO * s.i_y() * L1,
        mass * L1 * L1 / 12.0 + RHO * s.i_z() * L1,
    ];
    let mut want = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            want[i][j] = (0..3).map(|k| dl[k] * ax[k][i] * ax[k][j]).sum();
        }
    }
    let imax = max_abs(want.iter().flatten().copied());
    let ierr = max_abs((0..9).map(|q| p.inertia[q / 3][q % 3] - want[q / 3][q % 3])) / imax;
    c.at_most("M1 inertia about the centre", ierr, 1e-12);
    c.finish();
}

#[test]
fn d1_three_or_six_unknowns() {
    let mut c = Checks::new();
    let hex = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let ns = hex.nodes().len();
    c.holds(
        "D1 solid-only mesh",
        DofMap::for_mesh(&hex) == DofMap::uniform(ns, 3),
        &format!("{ns} nodes, uniform 3"),
    );
    let (x0, x1, v) = seeded();
    let bm = beam_model(
        vec![x0, x1, add(x1, [0.1, 0.2, 0.3])],
        &[[0, 1], [1, 2]],
        section1(),
        v,
        mat1(),
    );
    let d = bm.dof_map();
    c.holds(
        "D1 beam-only mesh",
        (0..3).all(|n| d.components(n) == 6) && d.n_equations() == 18,
        &format!("{} unknowns", d.n_equations()),
    );
    // A Hex8 box and a separate beam in one mesh.
    let mut nodes = hex.nodes().to_vec();
    nodes.push([5.0, 0.0, 0.0]);
    nodes.push([6.0, 0.0, 0.0]);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hex.blocks()[0].connectivity().to_vec())
        .unwrap();
    mesh.add_block(ElementKind::Beam2, vec![ns, ns + 1]).unwrap();
    let d = DofMap::for_mesh(&mesh);
    let ok = (0..ns).all(|n| d.components(n) == 3)
        && d.components(ns) == 6
        && d.components(ns + 1) == 6
        && d.n_equations() == 3 * ns + 12;
    c.holds(
        "D1 Hex8 box and a separate beam",
        ok,
        &format!("{} unknowns", d.n_equations()),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ R1

/// A rotation matrix: `angle` about the unit `axis` (Rodrigues).
fn rotation(axis: Vec3, angle: f64) -> [[f64; 3]; 3] {
    let k = unit(axis);
    let (s, co) = angle.sin_cos();
    let mut q = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            q[i][j] = co * f64::from(u8::from(i == j)) + (1.0 - co) * k[i] * k[j];
        }
    }
    q[0][1] -= s * k[2];
    q[0][2] += s * k[1];
    q[1][0] += s * k[2];
    q[1][2] -= s * k[0];
    q[2][0] -= s * k[1];
    q[2][1] += s * k[0];
    q
}

fn apply(q: &[[f64; 3]; 3], v: Vec3) -> Vec3 {
    [dot(q[0], v), dot(q[1], v), dot(q[2], v)]
}

fn apply_t(q: &[[f64; 3]; 3], v: Vec3) -> Vec3 {
    [
        q[0][0] * v[0] + q[1][0] * v[1] + q[2][0] * v[2],
        q[0][1] * v[0] + q[1][1] * v[1] + q[2][1] * v[2],
        q[0][2] * v[0] + q[1][2] * v[1] + q[2][2] * v[2],
    ]
}

type Run = (Vec<Vec3>, Vec<Vec3>, Vec3, Vec3, Vec<SectionForces>);

/// A clamped frame (root node 0, loads at the last node) solved with everything rotated by `q`; results rotated back.
fn frame_run(nodes: &[Vec3], elements: &[[usize; 2]], v: Vec3, f: Vec3, m: Vec3, q: &[[f64; 3]; 3]) -> Run {
    let rn: Vec<Vec3> = nodes.iter().map(|x| apply(q, *x)).collect();
    let mut model = beam_model(rn, elements, section1(), apply(q, v), mat1());
    clamp_root(&mut model);
    model.add_load(Load::nodal_force("tip", apply(q, f))).unwrap();
    model.add_load(Load::nodal_moment("tip", apply(q, m))).unwrap();
    let r = solve_static(&model, &StaticOptions::default()).unwrap();
    let mut ut = Vec::new();
    let mut ur = Vec::new();
    for n in 0..nodes.len() {
        let (t, w) = node6(&r, n);
        ut.push(apply_t(q, t));
        ur.push(apply_t(q, w));
    }
    let (rf, rm) = reaction6(&r, 0);
    let sf = beam::section_forces(&model, r.displacement_values())
        .unwrap()
        .into_iter()
        .map(|(_, s)| s)
        .collect();
    (ut, ur, apply_t(q, rf), apply_t(q, rm), sf)
}

fn field_diff(a: &[Vec3], b: &[Vec3]) -> f64 {
    let m = max_abs(a.iter().flatten().copied());
    max_abs(a.iter().flatten().zip(b.iter().flatten()).map(|(x, y)| x - y)) / m
}

#[test]
fn r1_rotation_of_the_whole_model() {
    let mut c = Checks::new();
    let q = rotation([1.0, -2.0, 2.0], 1.1);
    let id = rotation([1.0, 0.0, 0.0], 0.0);
    let (x0, x1, v) = seeded();
    let ax = axes(x0, x1, v);
    let one = (
        vec![x0, x1],
        vec![[0usize, 1usize]],
        v,
        to_global(&ax, F_LOCAL),
        to_global(&ax, [T_LOCAL, 0.0, 0.0]),
    );
    let space = (
        vec![[0.0, 0.0, 0.0], [0.8, 0.0, 0.0], [0.8, 0.6, 0.0], [0.8, 0.6, 0.5]],
        vec![[0, 1], [1, 2], [2, 3]],
        [1.0, 2.0, 3.0],
        [100.0, -200.0, 300.0],
        [10.0, 20.0, -30.0],
    );
    for (name, (nodes, elements, v, f, m)) in [("one element", one), ("space frame", space)] {
        let a = frame_run(&nodes, &elements, v, f, m, &id);
        let b = frame_run(&nodes, &elements, v, f, m, &q);
        c.at_most(&format!("R1 {name}: translations"), field_diff(&a.0, &b.0), 1e-10);
        c.at_most(&format!("R1 {name}: rotations"), field_diff(&a.1, &b.1), 1e-10);
        c.at_most(&format!("R1 {name}: reaction force"), field_diff(&[a.2], &[b.2]), 1e-10);
        c.at_most(
            &format!("R1 {name}: reaction moment"),
            field_diff(&[a.3], &[b.3]),
            1e-10,
        );
        let fa: Vec<f64> = a.4.iter().flat_map(|s| s.start.into_iter().chain(s.end)).collect();
        let fb: Vec<f64> = b.4.iter().flat_map(|s| s.start.into_iter().chain(s.end)).collect();
        let d = max_abs(fa.iter().zip(&fb).map(|(x, y)| x - y)) / max_abs(fa.iter().copied());
        c.at_most(&format!("R1 {name}: section forces"), d, 1e-10);
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ A1, A2

#[test]
fn a1_a2_quarter_circle_arch() {
    let mut c = Checks::new();
    let (r0, a, kappa, p) = (1.0, 0.004, 0.9, 1.0);
    let e = 2.0e11;
    let g = e / (2.0 * (1.0 + NU));
    let mat = Material::isotropic_elastic(e, NU, RHO, 0.0).unwrap();
    let s = beam::circle_section(a, kappa).unwrap();
    let ne = 32;
    let nodes: Vec<Vec3> = (0..=ne)
        .map(|i| {
            let phi = i as f64 * PI / (2.0 * ne as f64);
            [r0 * phi.cos(), r0 * phi.sin(), 0.0]
        })
        .collect();
    let elements: Vec<[usize; 2]> = (0..ne).map(|i| [i, i + 1]).collect();
    let mut model = beam_model(nodes.clone(), &elements, s, [0.0, 0.0, 1.0], mat);
    clamp_root(&mut model);
    model.add_load(Load::nodal_force("tip", [0.0, p, 0.0])).unwrap();
    let r = solve_static(&model, &StaticOptions::default()).unwrap();
    let uy = node6(&r, ne).0[1];
    let i = s.i_z();
    let exact = PI * p * r0.powi(3) / (4.0 * e * i);
    // The polygon of chords by Castigliano (arithmetic 4).
    let tip = nodes[ne];
    let (mut db, mut da, mut ds) = (0.0, 0.0, 0.0);
    for el in &elements {
        let (p0, p1) = (nodes[el[0]], nodes[el[1]]);
        let l = norm(sub(p1, p0));
        let t = unit(sub(p1, p0));
        let (m0, m1) = ((tip[0] - p0[0]) * p, (tip[0] - p1[0]) * p);
        db += l * (m0 * m0 + m0 * m1 + m1 * m1) / (3.0 * e * i);
        da += (p * t[1]).powi(2) * l / (e * s.area());
        ds += (p * t[0]).powi(2) * l / (kappa * g * s.area());
    }
    let polygon = (db + da + ds) / p;
    println!(
        "arch: u_y {uy:.10e}, pi P R^3 / (4 E I) {exact:.10e}, polygon {polygon:.10e} ({:.4e} relative)",
        (polygon - exact) / exact
    );
    c.at_most("A1 tip deflection against pi P R^3 / (4 E I)", rel(uy, exact), 5e-3);
    c.at_most(
        "A2 tip deflection against the polygon closed form",
        rel(uy, polygon),
        1e-6,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------- F1 to F4, N1

#[test]
fn f1_free_beam_six_zero_frequencies() {
    let mut c = Checks::new();
    let dir = unit([1.0, 1.0, 1.0]);
    let nodes: Vec<Vec3> = (0..=10).map(|i| scale(i as f64 / 10.0, dir)).collect();
    let elements: Vec<[usize; 2]> = (0..10).map(|i| [i, i + 1]).collect();
    let mat = Material::isotropic_elastic(2.0e11, NU, RHO, 0.0).unwrap();
    let model = beam_model(
        nodes,
        &elements,
        beam::circle_section(0.05, 0.9).unwrap(),
        [0.0, 0.0, 1.0],
        mat,
    );
    let r = modal_analysis(&model, &ModalOptions::new(7)).unwrap();
    println!("free beam frequencies {:?}", r.frequencies);
    let ratio = max_abs(r.frequencies[..6].iter().copied()) / r.frequencies[6];
    c.at_most("F1 max |f_1..6| / f_7", ratio, 1e-6);
    c.finish();
}

/// The exact Timoshenko pinned-pinned frequency of mode `n` (arithmetic 5).
fn timoshenko(n: usize, l: f64, e: f64, g: f64, rho: f64, a: f64, i: f64, kappa: f64) -> f64 {
    let k = n as f64 * PI / l;
    let s = kappa * g * a;
    let a2 = rho * a * rho * i;
    let a1 = -(rho * a * (e * i * k * k + s) + rho * i * s * k * k);
    let a0 = s * k * k * (e * i * k * k + s) - (s * k).powi(2);
    let w2 = (-a1 - (a1 * a1 - 4.0 * a2 * a0).sqrt()) / (2.0 * a2);
    w2.sqrt() / (2.0 * PI)
}

#[test]
fn f2_f3_f4_n1_pinned_pinned_frequencies() {
    let mut c = Checks::new();
    let (l, e) = (1.0, 2.0e11);
    let g = e / (2.0 * (1.0 + NU));
    let h = 0.02 * 12f64.sqrt();
    let kappa = 5.0 / 6.0;
    let s = beam::rectangle_section(h, h, 0.1406 * h.powi(4), kappa).unwrap();
    let ne = 20;
    let nodes: Vec<Vec3> = (0..=ne).map(|i| [l * i as f64 / ne as f64, 0.0, 0.0]).collect();
    let elements: Vec<[usize; 2]> = (0..ne).map(|i| [i, i + 1]).collect();
    let mat = Material::isotropic_elastic(e, NU, RHO, 0.0).unwrap();
    let base = beam_model(nodes, &elements, s, [0.0, 0.0, 1.0], mat.clone());
    let mut mesh = base.mesh().clone();
    mesh.add_node_set("ends", vec![0, ne]).unwrap();
    let mut model = SolidModel::new(mesh);
    model.add_material("all", mat).unwrap();
    for comp in [0, 2, 3, 4] {
        model
            .add_constraint(Constraint::displacement("all", comp, 0.0))
            .unwrap();
    }
    model.add_constraint(Constraint::displacement("ends", 1, 0.0)).unwrap();
    let r = modal_analysis(
        &model,
        &ModalOptions::new(3)
            .tolerance(1e-12)
            .residual_test(ResidualTest::Backward),
    )
    .unwrap();
    let limits = [5e-3, 1e-2, 2e-2];
    let strong = [5e-5, 5e-4, 2e-3];
    let (a, i) = (s.area(), s.i_z());
    for n in 1..=3 {
        let f_eb = (n as f64 * PI / l).powi(2) * (e * i / (RHO * a)).sqrt() / (2.0 * PI);
        let f_t = timoshenko(n, l, e, g, RHO, a, i, kappa);
        let c_n = f_t / f_eb;
        let f = r.frequencies[n - 1];
        println!("mode {n}: f {f:.6} Hz, f_EB {f_eb:.6}, c_n {c_n:.6}, f_T {f_t:.6}");
        c.at_most(
            &format!("F2 n = {n} against c_n f_n^EB"),
            rel(f, c_n * f_eb),
            limits[n - 1],
        );
        c.at_most(&format!("F3 n = {n} against f_n^T"), rel(f, f_t), strong[n - 1]);
        if n >= 2 {
            c.above(
                &format!("N1 n = {n} against the uncorrected f_n^EB"),
                rel(f, f_eb),
                limits[n - 1],
            );
        } else {
            println!("N1 n = 1 against the uncorrected f_n^EB: {:.4e}", rel(f, f_eb));
        }
        // F4: bending in the x-y plane with n - 1 interior sign changes of u_y.
        let mode = &r.modes[n - 1];
        let dofs = r.dofs();
        let uy: Vec<f64> = (0..=ne).map(|k| mode[dofs.equation(k, 1).unwrap()]).collect();
        let other = max_abs((0..=ne).flat_map(|k| [0, 2].map(|cc| mode[dofs.equation(k, cc).unwrap()])));
        let mut changes = 0;
        let interior: Vec<f64> = uy[1..ne].to_vec();
        for w in interior.windows(2) {
            if w[0] * w[1] < 0.0 {
                changes += 1;
            }
        }
        c.holds(
            &format!("F4 n = {n} bending in x-y"),
            max_abs(uy.iter().copied()) > other && changes == n - 1,
            &format!(
                "{changes} sign changes, max |u_y| {:.3e}, other {other:.3e}",
                max_abs(uy.iter().copied())
            ),
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- S1, N2, N3

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

/// A Hex8 box `[2, 1, 1]` with a beam block from `p` to `q` (positions; a position not on the box adds a node).
fn box_with_beam(p: Vec3, q: Vec3) -> (SolidModel, Vec<usize>) {
    let hex = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let mut nodes = hex.nodes().to_vec();
    let at = |x: Vec3, nodes: &mut Vec<Vec3>| match nodes.iter().position(|y| *y == x) {
        Some(i) => i,
        None => {
            nodes.push(x);
            nodes.len() - 1
        }
    };
    let a = at(p, &mut nodes);
    let b = at(q, &mut nodes);
    let xmin: Vec<usize> = (0..nodes.len()).filter(|&n| nodes[n][0] == 0.0).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hex.blocks()[0].connectivity().to_vec())
        .unwrap();
    mesh.add_block(ElementKind::Beam2, vec![a, b]).unwrap();
    mesh.set_block_property(1, BlockProperty::beam(section1(), [0.0, 0.0, 1.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    mesh.add_element_set("structural", vec![2]).unwrap();
    mesh.add_node_set("xmin", xmin).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_material("structural", steel()).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
    }
    (m, vec![a, b])
}

fn names_node<T: std::fmt::Debug>(r: &Result<T, FemError>, node: usize) -> bool {
    match r {
        Err(e @ FemError::BeamSolidNode { node: n, .. }) => {
            *n == node && e.to_string().contains(&format!("node {node}"))
        }
        _ => false,
    }
}

#[test]
fn s1_beam_node_shared_with_a_solid_is_refused() {
    let mut c = Checks::new();
    for (name, p, q, named) in [
        ("both nodes on the box", [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], 0usize),
        ("second node on the box", [3.0, 0.0, 0.0], [2.0, 0.0, 0.0], 1),
    ] {
        let (m, conn) = box_with_beam(p, q);
        let node = conn[named];
        let rs = solve_static(&m, &StaticOptions::default());
        println!("S1 {name}: {:?}", rs.as_ref().err().map(|e| e.to_string()));
        c.holds(
            &format!("S1 {name}: solve_static"),
            names_node(&rs, node),
            &format!("node {node}"),
        );
        let rm = modal_analysis(&m, &ModalOptions::new(2));
        c.holds(
            &format!("S1 {name}: modal_analysis"),
            names_node(&rm, node),
            &format!("node {node}"),
        );
        let rk = stiffness(&m);
        c.holds(
            &format!("S1 {name}: stiffness"),
            names_node(&rk, node),
            &format!("node {node}"),
        );
    }
    c.finish();
}

fn follow_up<T: std::fmt::Debug>(r: &Result<T, FemError>) -> bool {
    matches!(r, Err(FemError::NotYetSupported { variant: "Beam2", package, .. }) if *package == beam::FOLLOW_UP)
}

#[test]
fn n2_n3_other_paths_and_invalid_data() {
    let mut c = Checks::new();
    // N2: the clamped one-element model on the paths that do not take beams.
    let (base, ax, _, _) = one_element();
    let mut m = base.clone();
    m.add_support(ElasticSupport::nodal_spring("tip", 0, 1e6)).unwrap();
    let r = solve_static(&m, &StaticOptions::default());
    c.holds("N2 nodal spring (transformation)", follow_up(&r), &format!("{r:?}"));
    let r = rcfd_fem::nonlinear::initial_states(&base);
    c.holds("N2 nonlinear initial states", follow_up(&r), &format!("{r:?}"));
    let amg = StaticOptions::with_solver(StaticSolver::PreconditionedCg {
        preconditioner: CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::default()),
        tolerance: 1e-10,
        max_iterations: 100,
    });
    let r = solve_static(&base, &amg);
    c.holds("N2 smoothed-aggregation CG", follow_up(&r), &format!("{r:?}"));
    let mut m = base.clone();
    m.add_load(Load::gravity([0.0, 0.0, -9.81])).unwrap();
    let r = solve_static(&m, &StaticOptions::default());
    let ok = matches!(&r, Err(FemError::Element { source, .. })
        if matches!(source.as_ref(), FemError::NotYetSupported { package, .. } if *package == beam::SOLID_ROUTINES));
    c.holds("N2 gravity on a beam", ok, &format!("{r:?}"));
    // N3: invalid data.
    let (x0, x1, _) = seeded();
    let mut mesh = SolidMesh::new(vec![x0, x1]).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![0, 1]).unwrap();
    mesh.add_element_set("all", vec![0]).unwrap();
    mesh.add_node_set("root", vec![0]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat1()).unwrap();
    clamp_root(&mut m);
    let r = solve_static(&m, &StaticOptions::default());
    c.holds(
        "N3 Beam2 block without property",
        matches!(&r, Err(FemError::InvalidValue { key: "property", .. })),
        &format!("{r:?}"),
    );
    let mut m = beam_model(vec![x0, x1], &[[0, 1]], section1(), scale(-2.0, ax[0]), mat1());
    clamp_root(&mut m);
    let r = solve_static(&m, &StaticOptions::default());
    let ok = matches!(&r, Err(FemError::Element { element: 0, source })
        if matches!(source.as_ref(), FemError::InvalidValue { key: "orientation", .. }));
    c.holds("N3 orientation parallel to the axis", ok, &format!("{r:?}"));
    let hex = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let mut m = SolidModel::new(hex);
    let r = m.add_load(Load::nodal_moment("xmax", [1.0, 0.0, 0.0]));
    c.holds(
        "N3 nodal moment on a three-unknown node",
        matches!(
            &r,
            Err(FemError::ComponentOutOfRange {
                component: 3,
                available: 3,
                ..
            })
        ),
        &format!("{r:?}"),
    );
    c.finish();
}
