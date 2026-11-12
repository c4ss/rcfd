//! F-375 (ADR 0276): surface and body loads. Checks R1 to R6, D1 to D3, C1 to C4, B1 to B3, the extensions E1, E2, X1 to
//! X3 and the negative controls N1 to N3 of the registration (seed 0xF375_0276).

use rcfd_core::vec3::{cross, dot, norm, sub, Vec3};
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::loads::{body_force_loads, face_shape_functions, nodal_load_vector, NodalForceAt};
use rcfd_fem::mesh::{ElementKind, SolidMesh};
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidModel, SolidResult};
use std::collections::BTreeSet;
use std::f64::consts::FRAC_PI_2;

const SEED: u64 = 0xF375_0276;
const KINDS: [ElementKind; 3] = [ElementKind::Tet4, ElementKind::Hex8, ElementKind::Tet10];
const E_MOD: f64 = 2.0e11;
const RHO: f64 = 7850.0;

// Resultant box (ADR 0276 D3).
const B_ORIGIN: Vec3 = [0.1, -0.2, 0.3];
const B_LENGTHS: Vec3 = [1.2, 1.0, 0.8];
const B_DIV: [usize; 3] = [4, 3, 3];
const P: f64 = 2.5e5;
const T: Vec3 = [1e5, -2e5, 3e5];
const G_BOX: Vec3 = [0.3, -9.81, 1.2];
const AXIS: Vec3 = [1.0, 2.0, 2.0];
const ORIGIN_AXIS: Vec3 = [-0.5, 0.3, 0.1];
const OMEGA: f64 = 300.0;
const B0: Vec3 = [1e3, -2e3, 5e2];
const GRAD: [[f64; 3]; 3] = [[1e3, 0.5e3, -0.2e3], [0.3e3, -1e3, 0.4e3], [0.2e3, 0.1e3, 0.8e3]];

// Bars and ring (ADR 0276 D3).
const BAR_L: f64 = 1.0;
const BAR_B: f64 = 0.1;
const G_BAR: f64 = 9.81;
const RING_R: f64 = 1.0;
const RING_T: f64 = 0.02;
const RING_H: f64 = 0.02;

/// SplitMix64, a deterministic source only (ADR 0273 D6).
struct Rng(u64);

impl Rng {
    fn new(tag: u64) -> Rng {
        Rng(SEED.wrapping_add(tag))
    }
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

/// A list of measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.3e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn more_than(&mut self, name: &str, value: f64, bound: f64) {
        let ok = value > bound;
        println!(
            "{name}: {value:.3e} (control, must be > {bound:e}) {}",
            if ok { "holds" } else { "FAILED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {bound:e}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

fn kind_index(kind: ElementKind) -> u64 {
    match kind {
        ElementKind::Tet4 => 0,
        ElementKind::Hex8 => 1,
        _ => 2,
    }
}

/// The same blocks and sets on a new node list.
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

/// A box of the kind (Tet10 from the Tet4 box, straight edges); with `perturb`, the interior grid nodes move by `0.1 h_k
/// (2U - 1)` per coordinate before the upgrade.
fn box_of(kind: ElementKind, origin: Vec3, lengths: Vec3, div: [usize; 3], perturb: Option<u64>) -> SolidMesh {
    let base = match kind {
        ElementKind::Hex8 => hex_box(origin, lengths, div).unwrap(),
        _ => tet_box(origin, lengths, div).unwrap(),
    };
    let base = match perturb {
        None => base,
        Some(tag) => {
            let mut rng = Rng::new(tag);
            let mut boundary = BTreeSet::new();
            for f in BOX_FACES {
                boundary.extend(base.node_set(f).unwrap().iter().copied());
            }
            let h = [
                lengths[0] / div[0] as f64,
                lengths[1] / div[1] as f64,
                lengths[2] / div[2] as f64,
            ];
            let mut nodes = base.nodes().to_vec();
            for (i, p) in nodes.iter_mut().enumerate() {
                if !boundary.contains(&i) {
                    for k in 0..3 {
                        p[k] += 0.1 * h[k] * (2.0 * rng.uniform() - 1.0);
                    }
                }
            }
            rebuild(&base, nodes)
        }
    };
    match kind {
        ElementKind::Tet10 => upgrade_to_tet10(&base).unwrap().0,
        _ => base,
    }
}

fn model_of(mesh: SolidMesh, nu: f64) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_MOD, nu, RHO, 0.0).unwrap())
        .unwrap();
    m
}

fn result_box(kind: ElementKind) -> SolidMesh {
    box_of(kind, B_ORIGIN, B_LENGTHS, B_DIV, Some(10 + kind_index(kind)))
}

fn load_vector(mesh: &SolidMesh, loads: &[Load]) -> Vec<f64> {
    let mut m = model_of(mesh.clone(), 0.3);
    for l in loads {
        m.add_load(l.clone()).unwrap();
    }
    nodal_load_vector(&m, &[]).unwrap()
}

fn force_sum(f: &[f64]) -> Vec3 {
    let mut s = [0.0; 3];
    for c in f.chunks(3) {
        for i in 0..3 {
            s[i] += c[i];
        }
    }
    s
}

fn moment_sum(mesh: &SolidMesh, f: &[f64], about: Vec3) -> Vec3 {
    let mut s = [0.0; 3];
    for (x, c) in mesh.nodes().iter().zip(f.chunks(3)) {
        let m = cross(sub(*x, about), [c[0], c[1], c[2]]);
        for i in 0..3 {
            s[i] += m[i];
        }
    }
    s
}

fn set_area(mesh: &SolidMesh, set: &str) -> f64 {
    mesh.surface_set(set).unwrap().iter().map(|&f| mesh.face_area(f)).sum()
}

fn diff(a: Vec3, b: Vec3) -> f64 {
    norm(sub(a, b))
}

fn max_rel(a: &[f64], b: &[f64]) -> f64 {
    let m = b.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    a.iter().zip(b).fold(0.0f64, |d, (x, y)| d.max((x - y).abs())) / m
}

fn unit_axis() -> Vec3 {
    let l = norm(AXIS);
    [AXIS[0] / l, AXIS[1] / l, AXIS[2] / l]
}

fn project(v: Vec3) -> Vec3 {
    let a = unit_axis();
    let s = dot(v, a);
    [v[0] - s * a[0], v[1] - s * a[1], v[2] - s * a[2]]
}

fn box_centre() -> Vec3 {
    [
        B_ORIGIN[0] + B_LENGTHS[0] / 2.0,
        B_ORIGIN[1] + B_LENGTHS[1] / 2.0,
        B_ORIGIN[2] + B_LENGTHS[2] / 2.0,
    ]
}

fn linear_field(x: Vec3) -> Vec3 {
    let d = sub(x, box_centre());
    let mut b = B0;
    for i in 0..3 {
        for j in 0..3 {
            b[i] += GRAD[i][j] * d[j];
        }
    }
    b
}

fn to_vector(n: usize, forces: &[NodalForceAt]) -> Vec<f64> {
    let mut f = vec![0.0; 3 * n];
    for p in forces {
        for i in 0..3 {
            f[3 * p.node + i] += p.force[i];
        }
    }
    f
}

// --------------------------------------------------------------------------------------------------------- resultants

#[test]
fn r1_to_r6_resultants_on_a_box_and_n2() {
    let mut ck = Checks::new();
    let v = B_LENGTHS[0] * B_LENGTHS[1] * B_LENGTHS[2];
    let a_tot = 2.0 * (B_LENGTHS[0] * B_LENGTHS[1] + B_LENGTHS[1] * B_LENGTHS[2] + B_LENGTHS[2] * B_LENGTHS[0]);
    let d = norm(B_LENGTHS);
    let c = box_centre();
    for kind in KINDS {
        let k = kind.name();
        let mesh = result_box(kind);
        let vol: f64 = (0..mesh.n_elements()).map(|e| mesh.element_volume(e)).sum();
        println!(
            "{k}: {} nodes, {} elements, mesh volume {vol} (box {v})",
            mesh.nodes().len(),
            mesh.n_elements()
        );
        // R1: closed surface under uniform pressure.
        let loads: Vec<Load> = BOX_FACES.iter().map(|s| Load::pressure(s, P)).collect();
        let f = load_vector(&mesh, &loads);
        ck.at_most(
            &format!("R1 {k} pressure force / (p A)"),
            norm(force_sum(&f)) / (P * a_tot),
            1e-12,
        );
        ck.at_most(
            &format!("R1 {k} pressure moment / (p A L)"),
            norm(moment_sum(&mesh, &f, c)) / (P * a_tot * d),
            1e-12,
        );
        // R2: pressure on xmax only presses on the solid.
        let ax = set_area(&mesh, "xmax");
        let f = load_vector(&mesh, &[Load::pressure("xmax", P)]);
        ck.at_most(
            &format!("R2 {k} pressure on xmax against -p A e_x"),
            diff(force_sum(&f), [-P * ax, 0.0, 0.0]) / (P * ax),
            1e-12,
        );
        // R3: traction on ymax.
        let ay = set_area(&mesh, "ymax");
        let f = load_vector(&mesh, &[Load::traction("ymax", T)]);
        let want = [T[0] * ay, T[1] * ay, T[2] * ay];
        ck.at_most(
            &format!("R3 {k} traction against t A"),
            diff(force_sum(&f), want) / (norm(T) * ay),
            1e-12,
        );
        // R4: gravity.
        let f = load_vector(&mesh, &[Load::gravity(G_BOX)]);
        let want = [RHO * v * G_BOX[0], RHO * v * G_BOX[1], RHO * v * G_BOX[2]];
        ck.at_most(
            &format!("R4 {k} gravity against rho V g"),
            diff(force_sum(&f), want) / norm(want),
            1e-12,
        );
        // R5: centrifugal about an oblique axis not through the centroid.
        let f = load_vector(&mesh, &[Load::centrifugal(AXIS, ORIGIN_AXIS, OMEGA)]);
        let rc = project(sub(c, ORIGIN_AXIS));
        let s = RHO * OMEGA * OMEGA * v;
        let want = [s * rc[0], s * rc[1], s * rc[2]];
        let got = force_sum(&f);
        ck.at_most(
            &format!("R5 {k} centrifugal against rho w^2 V r_c (vector)"),
            diff(got, want) / norm(want),
            1e-12,
        );
        ck.at_most(
            &format!("R5 {k} centrifugal |sum f| against rho w^2 V r_c"),
            (norm(got) - norm(want)).abs() / norm(want),
            1e-12,
        );
        // N2: the same against the unprojected distance.
        let wrong = sub(c, ORIGIN_AXIS);
        let wrong = [s * wrong[0], s * wrong[1], s * wrong[2]];
        ck.more_than(
            &format!("N2 {k} centrifugal against rho w^2 V (x_c - o)"),
            diff(got, wrong) / norm(want),
            1e-3,
        );
        // R6: nodal linear body force field.
        let values: Vec<Vec3> = mesh.nodes().iter().map(|&x| linear_field(x)).collect();
        let f = load_vector(&mesh, &[Load::body_force_nodal(values)]);
        let want = [v * B0[0], v * B0[1], v * B0[2]];
        ck.at_most(
            &format!("R6 {k} nodal body force against V b0"),
            diff(force_sum(&f), want) / norm(want),
            1e-12,
        );
    }
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------- distributions

/// The consistent load of a uniform face force density `q` (per unit area) on a flat face of the given nodes and area
/// (ADR 0276 arithmetic 4).
fn flat_face_load(f: &mut [f64], nodes: &[usize], area: f64, q: Vec3) {
    let share: Vec<f64> = match nodes.len() {
        3 => vec![area / 3.0; 3],
        4 => vec![area / 4.0; 4],
        6 => vec![0.0, 0.0, 0.0, area / 3.0, area / 3.0, area / 3.0],
        n => panic!("face of {n} nodes"),
    };
    for (&node, s) in nodes.iter().zip(share) {
        for i in 0..3 {
            f[3 * node + i] += s * q[i];
        }
    }
}

#[test]
fn d1_to_d3_load_distributions() {
    let mut ck = Checks::new();
    for kind in KINDS {
        let k = kind.name();
        let mesh = result_box(kind);
        let n = mesh.nodes().len();
        // D1: traction on ymax.
        let lib = load_vector(&mesh, &[Load::traction("ymax", T)]);
        let mut test = vec![0.0; 3 * n];
        for &face in mesh.surface_set("ymax").unwrap() {
            flat_face_load(&mut test, &mesh.face_nodes(face), mesh.face_area(face), T);
        }
        ck.at_most(
            &format!("D1 {k} traction per node against the closed forms"),
            max_rel(&lib, &test),
            1e-13,
        );
        // D2: pressure on all six sets.
        let loads: Vec<Load> = BOX_FACES.iter().map(|s| Load::pressure(s, P)).collect();
        let lib = load_vector(&mesh, &loads);
        let mut test = vec![0.0; 3 * n];
        for s in BOX_FACES {
            for &face in mesh.surface_set(s).unwrap() {
                let a = mesh.face_area_vector(face);
                let area = norm(a);
                let q = [-P * a[0] / area, -P * a[1] / area, -P * a[2] / area];
                flat_face_load(&mut test, &mesh.face_nodes(face), area, q);
            }
        }
        ck.at_most(
            &format!("D2 {k} pressure per node against the closed forms"),
            max_rel(&lib, &test),
            1e-13,
        );
    }
    // D3: Tet4 centrifugal against int N_a x dV = V (sum x_b + x_a)/20.
    let mesh = result_box(ElementKind::Tet4);
    let lib = load_vector(&mesh, &[Load::centrifugal(AXIS, ORIGIN_AXIS, OMEGA)]);
    let mut test = vec![0.0; 3 * mesh.nodes().len()];
    let s = RHO * OMEGA * OMEGA;
    for e in 0..mesh.n_elements() {
        let nodes = mesh.element_nodes(e);
        let v = mesh.element_volume(e);
        let mut sum = [0.0; 3];
        for &b in nodes {
            for i in 0..3 {
                sum[i] += mesh.nodes()[b][i];
            }
        }
        for &a in nodes {
            let xa = mesh.nodes()[a];
            let m = [
                v * (sum[0] + xa[0]) / 20.0 - v * ORIGIN_AXIS[0] / 4.0,
                v * (sum[1] + xa[1]) / 20.0 - v * ORIGIN_AXIS[1] / 4.0,
                v * (sum[2] + xa[2]) / 20.0 - v * ORIGIN_AXIS[2] / 4.0,
            ];
            let pm = project(m);
            for i in 0..3 {
                test[3 * a + i] += s * pm[i];
            }
        }
    }
    ck.at_most(
        "D3 Tet4 centrifugal per node against the closed form",
        max_rel(&lib, &test),
        1e-13,
    );
    ck.finish();
}

// ---------------------------------------------------------------------------------------------- body force cross-checks

#[test]
fn b1_to_b3_body_force_field_and_function() {
    let mut ck = Checks::new();
    for kind in KINDS {
        let k = kind.name();
        let mesh = result_box(kind);
        let n = mesh.nodes().len();
        // B1: nodal rho g against gravity.
        let grav = load_vector(&mesh, &[Load::gravity(G_BOX)]);
        let field = load_vector(
            &mesh,
            &[Load::body_force_nodal(vec![
                [
                    RHO * G_BOX[0],
                    RHO * G_BOX[1],
                    RHO * G_BOX[2]
                ];
                n
            ])],
        );
        ck.at_most(
            &format!("B1 {k} nodal rho g against gravity"),
            max_rel(&field, &grav),
            1e-14,
        );
        // B2: the function helper with the centrifugal field against the centrifugal load.
        let model = model_of(mesh.clone(), 0.3);
        let cent = load_vector(&mesh, &[Load::centrifugal(AXIS, ORIGIN_AXIS, OMEGA)]);
        let s = RHO * OMEGA * OMEGA;
        let fun = body_force_loads(&model, |x| {
            let r = project(sub(x, ORIGIN_AXIS));
            [s * r[0], s * r[1], s * r[2]]
        })
        .unwrap();
        assert_eq!(fun.len(), n);
        ck.at_most(
            &format!("B2 {k} function helper against the centrifugal load"),
            max_rel(&to_vector(n, &fun), &cent),
            1e-14,
        );
        // B3: the function helper with the linear field against the nodal field.
        let values: Vec<Vec3> = mesh.nodes().iter().map(|&x| linear_field(x)).collect();
        let nodal = load_vector(&mesh, &[Load::body_force_nodal(values)]);
        let fun = body_force_loads(&model, linear_field).unwrap();
        ck.at_most(
            &format!("B3 {k} function helper against the nodal linear field"),
            max_rel(&to_vector(n, &fun), &nodal),
            1e-13,
        );
    }
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------- closed forms

fn bar(kind: ElementKind, n: usize) -> SolidModel {
    let mesh = box_of(
        kind,
        [0.0, -BAR_B / 2.0, -BAR_B / 2.0],
        [BAR_L, BAR_B, BAR_B],
        [n, 2, 2],
        None,
    );
    let mut m = model_of(mesh, 0.0);
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

fn tip(model: &SolidModel, r: &SolidResult) -> f64 {
    let tip = model.mesh().node_set("xmax").unwrap();
    tip.iter().map(|&a| r.displacement(a)[0]).sum::<f64>() / tip.len() as f64
}

fn hanging_exact(x: f64) -> f64 {
    RHO * G_BAR * (BAR_L * x - x * x / 2.0) / E_MOD
}

fn hanging_nodal_error(model: &SolidModel, r: &SolidResult) -> f64 {
    let mut e = 0.0f64;
    for (a, x) in model.mesh().nodes().iter().enumerate() {
        let u = r.displacement(a);
        e = e.max(diff([u[0], u[1], u[2]], [hanging_exact(x[0]), 0.0, 0.0]));
    }
    e / hanging_exact(BAR_L)
}

#[test]
fn c1_c2_hanging_bar_and_n1() {
    let mut ck = Checks::new();
    let exact = hanging_exact(BAR_L);
    println!("hanging bar: exact tip rho g L^2/(2E) = {exact:.6e} m");
    for kind in KINDS {
        let k = kind.name();
        let mut m = bar(kind, 8);
        let plain = m.clone();
        m.add_load(Load::gravity([G_BAR, 0.0, 0.0])).unwrap();
        let r = solve_static(&m, &StaticOptions::default()).unwrap();
        let rel_tip = (tip(&m, &r) - exact).abs() / exact;
        println!(
            "{k} [8, 2, 2]: {} unknowns, relative residual {:.2e}, tip {:.6e}",
            r.stats().unwrap().unknowns(),
            r.stats().unwrap().relative_residual(),
            tip(&m, &r)
        );
        if kind == ElementKind::Tet10 {
            ck.at_most(
                "C1 Tet10 hanging bar max nodal error / u(L)",
                hanging_nodal_error(&m, &r),
                1e-9,
            );
            // N1: a lumped load, each element's rho V_e g / 10 on each of its nodes.
            let mesh = plain.mesh();
            let mut lumped = vec![[0.0; 3]; mesh.nodes().len()];
            for e in 0..mesh.n_elements() {
                let share = RHO * mesh.element_volume(e) * G_BAR / 10.0;
                for &a in mesh.element_nodes(e) {
                    lumped[a][0] += share;
                }
            }
            let forces: Vec<NodalForceAt> = lumped
                .iter()
                .enumerate()
                .map(|(a, &f)| NodalForceAt::new(a, f))
                .collect();
            let r = solve_static_with(&plain, &[], &forces, &StaticOptions::default()).unwrap();
            ck.more_than(
                "N1 Tet10 lumped gravity, max nodal error / u(L)",
                hanging_nodal_error(&plain, &r),
                1e-4,
            );
            println!("C1 record: Tet10 relative tip error {rel_tip:.3e}");
        } else {
            ck.at_most(&format!("C2 {k} hanging bar tip [8, 2, 2]"), rel_tip, 1e-2);
        }
    }
    ck.finish();
}

#[test]
fn c3_rotating_bar_and_e1() {
    let mut ck = Checks::new();
    let exact = RHO * OMEGA * OMEGA * BAR_L.powi(3) / (3.0 * E_MOD);
    println!("rotating bar: exact tip rho w^2 L^3/(3E) = {exact:.6e} m");
    for (kind, n, id) in [
        (ElementKind::Tet10, 8, "C3"),
        (ElementKind::Hex8, 16, "C3"),
        (ElementKind::Tet4, 16, "E1"),
    ] {
        let k = kind.name();
        let mut m = bar(kind, n);
        m.add_load(Load::centrifugal([0.0, 0.0, 1.0], [0.0; 3], OMEGA)).unwrap();
        let r = solve_static(&m, &StaticOptions::default()).unwrap();
        let t = tip(&m, &r);
        println!(
            "{k} [{n}, 2, 2]: {} unknowns, relative residual {:.2e}, tip {t:.6e}",
            r.stats().unwrap().unknowns(),
            r.stats().unwrap().relative_residual()
        );
        ck.at_most(
            &format!("{id} {k} rotating bar tip [{n}, 2, 2]"),
            (t - exact).abs() / exact,
            1e-2,
        );
    }
    ck.finish();
}

/// The quarter ring of ADR 0276 arithmetic 7: the builder's box on the parameters (rho, theta, z), Tet10 upgraded before
/// the map, then mapped to (rho cos theta, rho sin theta, z). Returns the mesh and the parameter radius of each node.
fn ring(kind: ElementKind) -> (SolidMesh, Vec<f64>) {
    let origin = [RING_R - RING_T / 2.0, 0.0, 0.0];
    let lengths = [RING_T, FRAC_PI_2, RING_H];
    let div = [2, 16, 1];
    let base = match kind {
        ElementKind::Hex8 => hex_box(origin, lengths, div).unwrap(),
        _ => tet_box(origin, lengths, div).unwrap(),
    };
    let base = match kind {
        ElementKind::Tet10 => upgrade_to_tet10(&base).unwrap().0,
        _ => base,
    };
    let radius: Vec<f64> = base.nodes().iter().map(|p| p[0]).collect();
    let mapped: Vec<Vec3> = base
        .nodes()
        .iter()
        .map(|p| {
            let (c, s) = if p[1] == FRAC_PI_2 {
                (0.0, 1.0)
            } else if p[1] == 0.0 {
                (1.0, 0.0)
            } else {
                (p[1].cos(), p[1].sin())
            };
            [p[0] * c, p[0] * s, p[2]]
        })
        .collect();
    (rebuild(&base, mapped), radius)
}

#[test]
fn c4_thin_rotating_ring_e2_and_n3() {
    let mut ck = Checks::new();
    let exact = RHO * OMEGA * OMEGA * RING_R.powi(3) / E_MOD;
    println!(
        "ring: exact radial displacement rho w^2 R^3/E = {exact:.6e} m (t/R = {})",
        RING_T / RING_R
    );
    for kind in KINDS {
        let k = kind.name();
        let (mesh, radius) = ring(kind);
        let mut m = model_of(mesh, 0.0);
        m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
        m.add_constraint(Constraint::displacement("ymax", 0, 0.0)).unwrap();
        m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
        m.add_load(Load::centrifugal([0.0, 0.0, 1.0], [0.0; 3], OMEGA)).unwrap();
        let r = solve_static(&m, &StaticOptions::default()).unwrap();
        let (mut mid, mut n_mid, mut inner, mut ut, mut uz) = (0.0f64, 0, 0.0f64, 0.0f64, 0.0f64);
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for (a, x) in m.mesh().nodes().iter().enumerate() {
            let u = r.displacement(a);
            let rr = (x[0] * x[0] + x[1] * x[1]).sqrt();
            let ur = (u[0] * x[0] + u[1] * x[1]) / rr;
            ut = ut.max((-u[0] * x[1] + u[1] * x[0]).abs() / rr);
            uz = uz.max(u[2].abs());
            if (radius[a] - RING_R).abs() < 1e-12 {
                n_mid += 1;
                mid = mid.max((ur - exact).abs() / exact);
                lo = lo.min(ur / exact);
                hi = hi.max(ur / exact);
            }
            if (radius[a] - (RING_R - RING_T / 2.0)).abs() < 1e-12 {
                let own = RHO * OMEGA * OMEGA * rr.powi(3) / E_MOD;
                inner = inner.max((ur - own).abs() / own);
            }
        }
        println!(
            "{k}: {} unknowns, relative residual {:.2e}, {n_mid} mid-surface nodes, u_r / exact in [{lo:.6}, {hi:.6}]",
            r.stats().unwrap().unknowns(),
            r.stats().unwrap().relative_residual()
        );
        ck.at_most(&format!("C4 {k} ring mid-surface radial displacement"), mid, 2e-2);
        ck.at_most(&format!("E2 {k} ring max |u_theta| / exact"), ut / exact, 1e-2);
        ck.at_most(&format!("E2 {k} ring max |u_z| / exact"), uz / exact, 1e-2);
        if kind == ElementKind::Hex8 {
            ck.more_than(
                "N3 Hex8 ring inner surface against rho w^2 r^3/E of its radius",
                inner,
                1e-2,
            );
        }
    }
    ck.finish();
}

// ---------------------------------------------------------------------------------------------------------- refusals

#[test]
fn x1_to_x3_refusals() {
    let mesh = hex_box([0.0; 3], [1.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    // X1 (the thermal load refused) removed by the lead on 2026-11-12: ADR 0281 (F-380) builds it.
    // X2: a body load needs a material.
    let mut m = SolidModel::new(mesh.clone());
    m.add_load(Load::gravity([0.0, 0.0, -9.81])).unwrap();
    let e = nodal_load_vector(&m, &[]).unwrap_err();
    println!("X2: {e}");
    assert!(matches!(e, FemError::MaterialUnassigned { element: 0 }));
    // A surface load does not.
    let mut m = SolidModel::new(mesh.clone());
    m.add_load(Load::pressure("xmax", 1.0)).unwrap();
    assert!(nodal_load_vector(&m, &[]).is_ok());
    // X3: a non-finite body force and an unknown face size.
    let m = model_of(mesh, 0.3);
    let e = body_force_loads(&m, |_| [f64::NAN, 0.0, 0.0]).unwrap_err();
    println!("X3: {e}");
    assert!(e.to_string().contains("body force"));
    assert!(matches!(e, FemError::Element { element: 0, .. }));
    let e = face_shape_functions(5, [0.0, 0.0]).unwrap_err();
    println!("X3: {e}");
    assert!(matches!(e, FemError::InvalidValue { key: "face nodes", .. }));
}
