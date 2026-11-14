//! F-522 (ADR 0435): shells on the transformation path and body loads on shells. Checks G1 to G5, G3m, H1, H1b, H2, H2m,
//! H3, H3r, K1, K2, Z1 to Z7, C1, C1b, C2, S3, S3c and the negative controls N1 to N4 of the registration (R1 to R3 are the
//! retired-row copy in `si_shell.rs`; B1 to B3 are the existing tests).

use rcfd_core::vec3::{cross, dot, norm, Vec3};
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::free_motions::free_rigid_motions;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::mpc::{modal_analysis_mpc, solve_static_mpc, ReferenceLoad};
use rcfd_fem::shell;
use rcfd_fem::solve::{solve_static, CgPreconditioner, StaticOptions, StaticSolver};
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, ElasticSupport, ElementKind, FemError, LinearTerm, Load, Material,
    SolidMesh, SolidModel,
};
use rcfd_linalg::eigen::ResidualTest;

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

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;

fn mat(nu: f64) -> Material {
    Material::isotropic_elastic(E, nu, RHO, 0.0).unwrap()
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(s: f64, a: Vec3) -> Vec3 {
    [s * a[0], s * a[1], s * a[2]]
}

fn max_abs(v: impl IntoIterator<Item = f64>) -> f64 {
    v.into_iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

fn rel(meas: f64, exact: f64) -> f64 {
    ((meas - exact) / exact).abs()
}

/// A grid of `nx x ny` elements on `[0, a] x [0, b]` at z = 0, nodes in rows of constant y (`j (nx + 1) + i`).
fn grid(a: f64, b: f64, nx: usize, ny: usize) -> (Vec<Vec3>, Vec<[usize; 4]>) {
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            nodes.push([a * i as f64 / nx as f64, b * j as f64 / ny as f64, 0.0]);
        }
    }
    let mut el = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            el.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    (nodes, el)
}

/// One Shell4 block, element set "all", node set "all".
fn shell_mesh(nodes: Vec<Vec3>, elements: &[[usize; 4]], thickness: f64) -> SolidMesh {
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = elements.iter().flatten().copied().collect();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(thickness).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..elements.len()).collect()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    mesh
}

fn model_of(mesh: SolidMesh, m: Material) -> SolidModel {
    let mut model = SolidModel::new(mesh);
    model.add_material("all", m).unwrap();
    model
}

/// The rotation of `angle` about `axis` (Rodrigues), as rows.
fn rotation(axis: Vec3, angle: f64) -> [[f64; 3]; 3] {
    let n = norm(axis);
    let a = [axis[0] / n, axis[1] / n, axis[2] / n];
    let (s, c) = angle.sin_cos();
    let k = [[0.0, -a[2], a[1]], [a[2], 0.0, -a[0]], [-a[1], a[0], 0.0]];
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let kk: f64 = (0..3).map(|m| k[i][m] * k[m][j]).sum();
            r[i][j] = f64::from(u8::from(i == j)) + s * k[i][j] + (1.0 - c) * kk;
        }
    }
    r
}

fn apply(q: &[[f64; 3]; 3], v: Vec3) -> Vec3 {
    [dot(q[0], v), dot(q[1], v), dot(q[2], v)]
}

// ------------------------------------------------------------------------------- the distorted patch (arithmetic 2, 5)

const PATCH: [[f64; 2]; 16] = [
    [0.0, 0.0],
    [1.0 / 3.0, 0.0],
    [2.0 / 3.0, 0.0],
    [1.0, 0.0],
    [0.0, 1.0 / 3.0],
    [0.28, 0.40],
    [0.71, 0.30],
    [1.0, 1.0 / 3.0],
    [0.0, 2.0 / 3.0],
    [0.36, 0.62],
    [0.62, 0.74],
    [1.0, 2.0 / 3.0],
    [0.0, 1.0],
    [1.0 / 3.0, 1.0],
    [2.0 / 3.0, 1.0],
    [1.0, 1.0],
];
const PATCH_T: f64 = 0.1;
const G_ACC: Vec3 = [0.5, -1.5, -9.81];

fn patch_elements() -> Vec<[usize; 4]> {
    let mut el = Vec::new();
    for j in 0..3 {
        for i in 0..3 {
            el.push([4 * j + i, 4 * j + i + 1, 4 * (j + 1) + i + 1, 4 * (j + 1) + i]);
        }
    }
    el
}

struct Patch {
    model: SolidModel,
    q: [[f64; 3]; 3],
    origin: Vec3,
}

impl Patch {
    fn place(&self, p: [f64; 2]) -> Vec3 {
        add(self.origin, apply(&self.q, [p[0], p[1], 0.0]))
    }
}

fn patch() -> Patch {
    let q = rotation([1.0, -2.0, 2.0], 0.7);
    let origin = [0.3, -0.2, 0.5];
    let nodes: Vec<Vec3> = PATCH
        .iter()
        .map(|p| add(origin, apply(&q, [p[0], p[1], 0.0])))
        .collect();
    let mut mesh = shell_mesh(nodes, &patch_elements(), PATCH_T);
    let boundary: Vec<usize> = (0..16)
        .filter(|&k| {
            let p = PATCH[k];
            p[0] == 0.0 || p[0] == 1.0 || p[1] == 0.0 || p[1] == 1.0
        })
        .collect();
    mesh.add_node_set("boundary", boundary).unwrap();
    Patch {
        model: model_of(mesh, mat(0.3)),
        q,
        origin,
    }
}

fn cross2(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

/// Area and area centroid of the patch by two triangles per element (patch coordinates), and per node the closed form
/// `sum_e (J_0 + (J_1 xi_i + J_2 eta_i) / 3)` of `int N_i dA` and the quarter split `sum_e A_e / 4` (arithmetic 2).
fn patch_integrals() -> (f64, [f64; 2], Vec<f64>, Vec<f64>) {
    let corners = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
    let mut area = 0.0;
    let mut moment = [0.0; 2];
    let mut cons = vec![0.0; 16];
    let mut lump = vec![0.0; 16];
    for e in patch_elements() {
        let x: Vec<[f64; 2]> = e.iter().map(|&k| PATCH[k]).collect();
        let comb = |s: [f64; 4]| -> [f64; 2] {
            let mut r = [0.0; 2];
            for (a, xa) in x.iter().enumerate() {
                r[0] += 0.25 * s[a] * xa[0];
                r[1] += 0.25 * s[a] * xa[1];
            }
            r
        };
        let a1 = comb([-1.0, 1.0, 1.0, -1.0]);
        let a2 = comb([-1.0, -1.0, 1.0, 1.0]);
        let a12 = comb([1.0, -1.0, 1.0, -1.0]);
        let (j0, j1, j2) = (cross2(a1, a2), cross2(a1, a12), cross2(a12, a2));
        for (a, &k) in e.iter().enumerate() {
            cons[k] += j0 + (j1 * corners[a][0] + j2 * corners[a][1]) / 3.0;
            lump[k] += j0;
        }
        for tri in [[0usize, 1, 2], [0, 2, 3]] {
            let (p, q, r) = (x[tri[0]], x[tri[1]], x[tri[2]]);
            let at = 0.5 * cross2([q[0] - p[0], q[1] - p[1]], [r[0] - p[0], r[1] - p[1]]);
            area += at;
            moment[0] += at * (p[0] + q[0] + r[0]) / 3.0;
            moment[1] += at * (p[1] + q[1] + r[1]) / 3.0;
        }
    }
    (area, [moment[0] / area, moment[1] / area], cons, lump)
}

fn node_force_moment(model: &SolidModel, f: &[f64], node: usize) -> (Vec3, Vec3) {
    let s = model.dof_map().equation(node, 0).unwrap();
    ([f[s], f[s + 1], f[s + 2]], [f[s + 3], f[s + 4], f[s + 5]])
}

// --------------------------------------------------------------------------------------------- G1, G2, G3, G3m, N1

#[test]
fn g1_to_g3_gravity_on_a_distorted_patch_in_space() {
    let mut c = Checks::new();
    let p = patch();
    let (area, centroid, cons, lump) = patch_integrals();
    let rta = RHO * PATCH_T * area;
    let total = scale(rta, G_ACC);
    let xc = p.place(centroid);
    let mut m = p.model.clone();
    m.add_load(Load::gravity(G_ACC)).unwrap();
    let f = nodal_load_vector(&m, &[]).unwrap();
    let nodes = m.mesh().nodes().to_vec();
    let xmax = max_abs(nodes.iter().flatten().copied());
    // G1: resultant and moment of the nodal loads.
    let mut sf = [0.0; 3];
    let mut sm = [0.0; 3];
    for (k, x) in nodes.iter().enumerate() {
        let (fk, mk) = node_force_moment(&m, &f, k);
        sf = add(sf, fk);
        sm = add(sm, add(cross(*x, fk), mk));
    }
    c.at_most(
        "G1 sum of the nodal forces against rho t A g",
        norm(sub(sf, total)) / norm(total),
        1e-12,
    );
    c.at_most(
        "G1 moment of the nodal loads against A x_c x rho t g",
        norm(sub(sm, cross(xc, total))) / (xmax * norm(total)),
        1e-12,
    );
    // G2, N1: each node against the consistent load of the bilinear translations, and against the quarter split.
    let fmax = max_abs(cons.iter().map(|w| RHO * PATCH_T * w * norm(G_ACC)));
    let (mut dcons, mut dmom, mut dlump) = (0.0f64, 0.0f64, 0.0f64);
    for k in 0..16 {
        let (fk, mk) = node_force_moment(&m, &f, k);
        let exact = scale(RHO * PATCH_T * cons[k], G_ACC);
        let quarter = scale(RHO * PATCH_T * lump[k], G_ACC);
        dcons = dcons.max(max_abs(sub(fk, exact)));
        dlump = dlump.max(max_abs(sub(fk, quarter)));
        dmom = dmom.max(max_abs(mk));
    }
    c.at_most(
        "G2 nodal forces against the consistent closed form",
        dcons / fmax,
        1e-12,
    );
    c.at_most("G2 nodal moments of the flat mesh (per 1 m)", dmom / fmax, 1e-12);
    c.above("N1 nodal forces against the quarter split", dlump / fmax, 1e-3);
    // G3, G3m: the reactions of the patch clamped on its boundary.
    let mut s = p.model.clone();
    for comp in 0..6 {
        s.add_constraint(Constraint::displacement("boundary", comp, 0.0))
            .unwrap();
    }
    s.add_load(Load::gravity(G_ACC)).unwrap();
    let r = solve_static(&s, &StaticOptions::default()).unwrap();
    let reactions = r.reactions().unwrap();
    let mut rf = [0.0; 3];
    let mut rm = [0.0; 3];
    for (k, x) in nodes.iter().enumerate() {
        let (fk, mk) = node_force_moment(&s, reactions, k);
        rf = add(rf, fk);
        rm = add(rm, add(cross(*x, fk), mk));
    }
    c.at_most(
        "G3 sum of the reactions against -rho t A g",
        norm(add(rf, total)) / norm(total),
        1e-12,
    );
    c.at_most(
        "G3m moment of the reactions against -A x_c x rho t g",
        norm(add(rm, cross(xc, total))) / (xmax * norm(total)),
        1e-12,
    );
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------- G4, G5

#[test]
fn g4_g5_solid_bits_and_a_warped_facet() {
    let mut c = Checks::new();
    // G4: a Hex8 box with a separate shell plate: the box's entries are the box's alone, bit for bit.
    let hex = hex_box([0.0; 3], [1.0, 1.0, 1.0], [2, 2, 2]).unwrap();
    let nh = hex.nodes().len();
    let (pn, pe) = grid(1.0, 1.0, 2, 2);
    let mut nodes = hex.nodes().to_vec();
    nodes.extend(pn.iter().map(|x| [x[0], x[1], 2.0]));
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hex.blocks()[0].connectivity().to_vec())
        .unwrap();
    let conn: Vec<usize> = pe.iter().flatten().map(|&k| nh + k).collect();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(1, BlockProperty::shell(0.02).unwrap()).unwrap();
    mesh.add_element_set("solid", (0..8).collect()).unwrap();
    mesh.add_element_set("shell", (8..12).collect()).unwrap();
    let mut mixed = SolidModel::new(mesh);
    mixed.add_material("solid", mat(0.3)).unwrap();
    mixed.add_material("shell", mat(0.3)).unwrap();
    let box_alone = model_of(hex, mat(0.3));
    let loads = [
        ("gravity", Load::gravity(G_ACC)),
        ("centrifugal", Load::centrifugal([0.0, 0.0, 1.0], [0.3, 0.2, 0.0], 40.0)),
    ];
    for (name, load) in loads {
        let mut a = mixed.clone();
        a.add_load(load.clone()).unwrap();
        let mut b = box_alone.clone();
        b.add_load(load).unwrap();
        let fa = nodal_load_vector(&a, &[]).unwrap();
        let fb = nodal_load_vector(&b, &[]).unwrap();
        let same = fa[..fb.len()].iter().zip(&fb).all(|(x, y)| x.to_bits() == y.to_bits());
        c.holds(
            &format!("G4 {name}: the box's entries bit for bit"),
            same,
            &format!("{} entries", fb.len()),
        );
    }
    // G5: one warped facet (arithmetic 9).
    let x: Vec<Vec3> = vec![[0.0, 0.0, 0.0], [1.2, 0.0, 0.0], [1.0, 0.8, 0.12], [0.1, 1.0, 0.0]];
    let mut cen = [0.0; 3];
    for xi in &x {
        cen = add(cen, scale(0.25, *xi));
    }
    let n = cross(sub(x[2], x[0]), sub(x[3], x[1]));
    let e3 = scale(1.0 / norm(n), n);
    let p: Vec<Vec3> = x.iter().map(|xi| sub(*xi, scale(dot(sub(*xi, cen), e3), e3))).collect();
    let h = dot(sub(x[0], cen), e3);
    let warp = h.abs() / (0.5 * norm(n)).sqrt();
    c.holds(
        "G5 set-up: warp ratio below the limit",
        warp < shell::WARP_LIMIT,
        &format!("{warp:.4e}"),
    );
    // Area, first and second moments of the projected quadrilateral by two triangles.
    let (mut ap, mut first, mut second) = (0.0, [0.0; 3], [[0.0; 3]; 3]);
    for tri in [[0usize, 1, 2], [0, 2, 3]] {
        let v = [p[tri[0]], p[tri[1]], p[tri[2]]];
        let at = 0.5 * norm(cross(sub(v[1], v[0]), sub(v[2], v[0])));
        let s = add(add(v[0], v[1]), v[2]);
        ap += at;
        first = add(first, scale(at / 3.0, s));
        for i in 0..3 {
            for j in 0..3 {
                let vv: f64 = v.iter().map(|q| q[i] * q[j]).sum();
                second[i][j] += at / 12.0 * (vv + s[i] * s[j]);
            }
        }
    }
    let t = 0.1;
    let warped = model_of(shell_mesh(x.clone(), &[[0, 1, 2, 3]], t), mat(0.3));
    let xm = max_abs(x.iter().flatten().copied());
    let moments = |m: &SolidModel| -> (Vec3, Vec3) {
        let f = nodal_load_vector(m, &[]).unwrap();
        let (mut sf, mut sm) = ([0.0; 3], [0.0; 3]);
        for (k, xk) in x.iter().enumerate() {
            let (fk, mk) = node_force_moment(m, &f, k);
            sf = add(sf, fk);
            sm = add(sm, add(cross(*xk, fk), mk));
        }
        (sf, sm)
    };
    let mut g = warped.clone();
    g.add_load(Load::gravity(G_ACC)).unwrap();
    let (sf, sm) = moments(&g);
    let res = scale(RHO * t * ap, G_ACC);
    let xp = scale(1.0 / ap, first);
    c.at_most("G5 gravity resultant", norm(sub(sf, res)) / norm(res), 1e-12);
    c.at_most(
        "G5 gravity moment",
        norm(sub(sm, cross(xp, res))) / (xm * norm(res)),
        1e-12,
    );
    let (axis, o, om) = ([0.0, 0.0, 1.0], [0.0; 3], 50.0);
    let mut cf = warped.clone();
    cf.add_load(Load::centrifugal(axis, o, om)).unwrap();
    let (sf, sm) = moments(&cf);
    let la = norm(axis);
    let a = scale(1.0 / la, axis);
    let pm = |i: usize, j: usize| f64::from(u8::from(i == j)) - a[i] * a[j];
    let perp = |v: Vec3| -> Vec3 { std::array::from_fn(|i| (0..3).map(|j| pm(i, j) * v[j]).sum()) };
    let s = RHO * t * om * om;
    let res = scale(s, perp(sub(first, scale(ap, o))));
    // int x x P x dA: (x x P x)_k = eps_kij x_i (P x)_j, so int = eps_kij (S P^T)_ij.
    let mut sp = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            sp[i][j] = (0..3).map(|l| second[i][l] * pm(j, l)).sum();
        }
    }
    let xpx = [sp[1][2] - sp[2][1], sp[2][0] - sp[0][2], sp[0][1] - sp[1][0]];
    let mexact = scale(s, sub(xpx, cross(first, perp(o))));
    c.at_most("G5 centrifugal resultant", norm(sub(sf, res)) / norm(res), 1e-12);
    c.at_most("G5 centrifugal moment", norm(sub(sm, mexact)) / (xm * norm(res)), 1e-12);
    c.finish();
}

// ------------------------------------------------------------------------------------------- the plate of H (arithmetic 3)

const HT: f64 = 0.05;

/// The plate `[0, 1]^2`, 4 x 4 elements, nodes `5 j + i`; sets "edge" (x = 0), "tip" (x = 1), "corner" (node 0). With
/// `offset` nodes before it (a block) and its elements in the element set "shell".
fn plate_nodes() -> (Vec<Vec3>, Vec<[usize; 4]>) {
    grid(1.0, 1.0, 4, 4)
}

fn plate(pressure: bool) -> SolidModel {
    let (nodes, el) = plate_nodes();
    let mut mesh = shell_mesh(nodes, &el, HT);
    mesh.add_node_set("edge", (0..5).map(|j| 5 * j).collect()).unwrap();
    mesh.add_node_set("tip", (0..5).map(|j| 5 * j + 4).collect()).unwrap();
    mesh.add_node_set("corner", vec![0]).unwrap();
    let mut m = model_of(mesh, mat(0.3));
    if pressure {
        m.add_load(Load::shell_pressure("all", 1000.0)).unwrap();
    }
    m
}

fn clamp(m: &mut SolidModel, set: &str) {
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement(set, comp, 0.0)).unwrap();
    }
}

/// The mixed model of H2: the block `hex_box([-0.2, 0, -0.1], [0.2, 1, 0.2], [1, 2, 1])` with every node prescribed, the
/// plate's nodes after the block's; sets "edge" (the plate's x = 0), "joint" (the block's xmax and the plate's edge).
fn mixed() -> (SolidModel, usize) {
    let hex = hex_box([-0.2, 0.0, -0.1], [0.2, 1.0, 0.2], [1, 2, 1]).unwrap();
    let nh = hex.nodes().len();
    let (pn, pe) = plate_nodes();
    let mut nodes = hex.nodes().to_vec();
    nodes.extend_from_slice(&pn);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hex.blocks()[0].connectivity().to_vec())
        .unwrap();
    let ne_h = hex.blocks()[0].connectivity().len() / 8;
    let conn: Vec<usize> = pe.iter().flatten().map(|&k| nh + k).collect();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(1, BlockProperty::shell(HT).unwrap()).unwrap();
    mesh.add_element_set("solid", (0..ne_h).collect()).unwrap();
    mesh.add_element_set("shell", (ne_h..ne_h + pe.len()).collect())
        .unwrap();
    mesh.add_node_set("block", (0..nh).collect()).unwrap();
    let edge: Vec<usize> = (0..5).map(|j| nh + 5 * j).collect();
    mesh.add_node_set("edge", edge.clone()).unwrap();
    let mut joint = hex.node_set("xmax").unwrap().to_vec();
    joint.extend_from_slice(&edge);
    mesh.add_node_set("joint", joint).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("solid", mat(0.3)).unwrap();
    m.add_material("shell", mat(0.3)).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("block", comp, 0.0)).unwrap();
    }
    m.add_load(Load::shell_pressure("shell", 1000.0)).unwrap();
    (m, nh)
}

/// The largest difference of the translations and of the rotations of `nodes`, each relative to the largest value of that
/// field in `b`.
fn field_diff(
    a: &rcfd_fem::SolidResult,
    b: &rcfd_fem::SolidResult,
    nodes: impl Iterator<Item = usize> + Clone,
) -> (f64, f64) {
    let mut out = [0.0; 2];
    for (f, range) in [(0usize, 0..3), (1, 3..6)] {
        let mut d = 0.0f64;
        let mut big = 0.0f64;
        for n in nodes.clone() {
            for k in range.clone() {
                d = d.max((a.displacement(n)[k] - b.displacement(n)[k]).abs());
                big = big.max(b.displacement(n)[k].abs());
            }
        }
        out[f] = d / big;
    }
    (out[0], out[1])
}

fn modal_options() -> ModalOptions {
    ModalOptions::new(4)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward)
}

// --------------------------------------------------------------------------------------- H1, H1b, H2, H2m, H3, H3r

#[test]
fn h1_to_h3_a_plate_held_by_a_rigid_set_on_an_edge() {
    let mut c = Checks::new();
    let mut clamped = plate(true);
    clamp(&mut clamped, "edge");
    let rc = solve_static(&clamped, &StaticOptions::default()).unwrap();
    for (name, reference) in [("H1", [0.0, 0.5, 0.0]), ("H1b", [0.0, 0.0, 0.0])] {
        let mut m = plate(true);
        m.add_constraint(Constraint::rigid_set("edge", reference)).unwrap();
        clamp(&mut m, "corner");
        let r = solve_static(&m, &StaticOptions::default()).unwrap();
        let (dt, dr) = field_diff(&r, &rc, 0..25);
        c.at_most(&format!("{name} translations against the clamped plate"), dt, 1e-10);
        c.at_most(&format!("{name} rotations against the clamped plate"), dr, 1e-10);
    }
    // H2: joined to a fixed block.
    let (base, nh) = mixed();
    let mut mc = base.clone();
    clamp(&mut mc, "edge");
    let rmc = solve_static(&mc, &StaticOptions::default()).unwrap();
    let mut mj = base.clone();
    mj.add_constraint(Constraint::rigid_set("joint", [0.0, 0.5, 0.0]))
        .unwrap();
    let rmj = solve_static(&mj, &StaticOptions::default()).unwrap();
    let (dt, dr) = field_diff(&rmj, &rmc, nh..nh + 25);
    c.at_most("H2 translations against the clamped plate", dt, 1e-10);
    c.at_most("H2 rotations against the clamped plate", dr, 1e-10);
    // H2m: the four lowest frequencies.
    let fc = modal_analysis(&mc, &modal_options()).unwrap().frequencies;
    let fj = modal_analysis(&mj, &modal_options()).unwrap().frequencies;
    for i in 0..4 {
        c.at_most(
            &format!("H2m frequency {} ({:.6} Hz)", i + 1, fc[i]),
            rel(fj[i], fc[i]),
            1e-8,
        );
    }
    // H3: a rigid set on the free edge of the cantilever plate, loaded at its reference point.
    let mut m = plate(false);
    clamp(&mut m, "edge");
    let x_r = [1.0, 0.5, 0.0];
    m.add_constraint(Constraint::rigid_set("tip", x_r)).unwrap();
    let (force, moment) = ([150.0, 300.0, 500.0], [100.0, -40.0, 25.0]);
    let s = solve_static_mpc(
        &m,
        &[],
        &[],
        &[ReferenceLoad::new(6, force, moment)],
        &StaticOptions::default(),
    )
    .unwrap();
    let (t, w) = (s.references[0].translation, s.references[0].rotation);
    let nodes = m.mesh().nodes().to_vec();
    let tip: Vec<usize> = (0..5).map(|j| 5 * j + 4).collect();
    let (mut du, mut dth, mut umax) = (0.0f64, 0.0f64, 0.0f64);
    for &a in &tip {
        let d = s.result.displacement(a);
        let rigid = add(t, cross(w, sub(nodes[a], x_r)));
        du = du.max(max_abs(sub([d[0], d[1], d[2]], rigid)));
        dth = dth.max(max_abs(sub([d[3], d[4], d[5]], w)));
        umax = umax.max(max_abs([d[0], d[1], d[2]]));
    }
    c.at_most("H3 tip translations u_a - t - w x d_a", du / umax, 1e-12);
    c.at_most("H3 tip rotations theta_a - w", dth / norm(w), 1e-12);
    let reactions = s.result.reactions().unwrap();
    let (mut rf, mut rm) = ([0.0; 3], [0.0; 3]);
    for j in 0..5 {
        let a = 5 * j;
        let (fa, ma) = node_force_moment(&m, reactions, a);
        rf = add(rf, fa);
        rm = add(rm, add(cross(nodes[a], fa), ma));
    }
    c.at_most(
        "H3r root reaction forces balance the reference force",
        norm(add(rf, force)) / norm(force),
        1e-10,
    );
    c.at_most(
        "H3r root reaction moments balance (about the origin)",
        norm(add(add(rm, cross(x_r, force)), moment)) / (norm(x_r) * norm(force) + norm(moment)),
        1e-10,
    );
    c.finish();
}

// -------------------------------------------------------------------------------------------- K1, K2 (arithmetic 4)

const KB: f64 = 0.2;
const KT: f64 = 0.05;

/// The strip `[0, 1] x [0, 0.2]`, 4 x 1 elements, `t = 0.05`, `nu = 0`; sets "root" (0, 5), "tip" (4, 9), "root0" (0).
fn strip() -> SolidModel {
    let (nodes, el) = grid(1.0, KB, 4, 1);
    let mut mesh = shell_mesh(nodes, &el, KT);
    mesh.add_node_set("root", vec![0, 5]).unwrap();
    mesh.add_node_set("tip", vec![4, 9]).unwrap();
    mesh.add_node_set("root0", vec![0]).unwrap();
    model_of(mesh, mat(0.0))
}

#[test]
fn k1_k2_a_nodal_spring_on_a_shell_rotation() {
    let mut c = Checks::new();
    let ei = E * KB * KT.powi(3) / 12.0;
    let k = 10.0 * ei;
    for (name, at_tip, mm) in [("K1", false, 3000.0), ("K2", true, 1500.0)] {
        let mut m = strip();
        for comp in [0, 1, 2, 3, 5] {
            m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
        }
        m.add_support(ElasticSupport::nodal_spring("root", 4, k)).unwrap();
        m.add_load(Load::nodal_moment(if at_tip { "tip" } else { "root" }, [0.0, mm, 0.0]))
            .unwrap();
        let s = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).unwrap();
        let dofs = m.dof_map();
        let nodes = m.mesh().nodes().to_vec();
        for root in [0usize, 5] {
            let th = s.result.displacement(root)[4];
            c.at_most(
                &format!("{name} node {root}: |k_theta theta - M| / M"),
                (k * th - mm).abs() / mm,
                1e-10,
            );
            if !at_tip {
                let e = dofs.equation(root, 4).unwrap();
                c.at_most(
                    &format!("{name} node {root}: spring force |S + M| / M"),
                    (s.support_forces[e] + mm).abs() / mm,
                    1e-10,
                );
            }
        }
        if at_tip {
            let th_root = mm / k;
            let m_tot = 2.0 * mm;
            for tip in [4usize, 9] {
                let d = s.result.displacement(tip);
                c.at_most(
                    &format!("K2 node {tip}: tip rotation theta_root + M L / (E I)"),
                    rel(d[4], th_root + m_tot / ei),
                    1e-10,
                );
                c.at_most(
                    &format!("K2 node {tip}: tip deflection -(theta_root L + M L^2 / (2 E I))"),
                    rel(d[2], -(th_root + m_tot / (2.0 * ei))),
                    1e-10,
                );
            }
        } else {
            let th = mm / k;
            let (mut dth, mut duz) = (0.0f64, 0.0f64);
            for (n, x) in nodes.iter().enumerate() {
                let d = s.result.displacement(n);
                dth = dth.max((d[4] - th).abs());
                duz = duz.max((d[2] + th * x[0]).abs());
            }
            c.at_most("K1 every node turns rigidly: theta_y", dth / th, 1e-10);
            c.at_most("K1 every node turns rigidly: u_z = -theta x", duz / th, 1e-10);
        }
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------- Z1 to Z7 (arithmetic 6)

#[test]
fn z1_to_z7_free_rigid_motions_with_shell_nodes() {
    let mut c = Checks::new();
    let count = |m: &SolidModel| free_rigid_motions(m, &[]).unwrap().count;
    let free = strip();
    let z1 = count(&free);
    c.holds("Z1 free strip", z1 == 6, &format!("{z1}"));
    let mut z2m = strip();
    for comp in 0..3 {
        z2m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
    }
    let z2 = count(&z2m);
    c.holds("Z2 root translations", z2 == 1, &format!("{z2}"));
    let mut z3m = z2m.clone();
    z3m.add_support(ElasticSupport::nodal_spring("root0", 4, 1e6)).unwrap();
    let z3 = count(&z3m);
    c.holds("Z3 root translations, spring on theta_y", z3 == 0, &format!("{z3}"));
    let mut z4m = strip();
    for comp in 0..4 {
        z4m.add_constraint(Constraint::displacement("root0", comp, 0.0))
            .unwrap();
    }
    z4m.add_constraint(Constraint::linear(vec![LinearTerm::new(0, 5, 1.0)], 0.0))
        .unwrap();
    let z4 = count(&z4m);
    c.holds(
        "Z4 node 0 translations and theta_x, Linear theta_z",
        z4 == 1,
        &format!("{z4}"),
    );
    let mut z5m = plate(true);
    z5m.add_constraint(Constraint::rigid_set("edge", [0.0, 0.5, 0.0]))
        .unwrap();
    let z5 = count(&z5m);
    c.holds("Z5 plate with a rigid set on its edge only", z5 == 6, &format!("{z5}"));
    let mut h1 = z5m.clone();
    clamp(&mut h1, "corner");
    let z6a = count(&h1);
    c.holds("Z6 the H1 model", z6a == 0, &format!("{z6a}"));
    let (mut h2, _) = mixed();
    h2.add_constraint(Constraint::rigid_set("joint", [0.0, 0.5, 0.0]))
        .unwrap();
    let z6b = count(&h2);
    c.holds("Z6 the H2 model", z6b == 0, &format!("{z6b}"));
    // Z7: the free fields of Z1 are in the null space of the shell stiffness.
    let frm = free_rigid_motions(&free, &[]).unwrap();
    let k = stiffness(&free).unwrap();
    let kmax = max_abs(k.val.iter().copied());
    let mut worst = 0.0f64;
    for x in frm.fields(&free) {
        let mut kx = vec![0.0; x.len()];
        k.matvec(&x, &mut kx);
        worst = worst.max(max_abs(kx) / (kmax * max_abs(x.iter().copied())));
    }
    c.at_most("Z7 K X c of the free fields", worst, 1e-12);
    c.finish();
}

// ---------------------------------------------------------------------------------------- C1, C1b, C2 (arithmetic 7)

#[test]
fn c1_c2_centrifugal_load_on_shells() {
    let mut c = Checks::new();
    let (l, b, t, om, nx) = (1.0, 0.1, 0.01, 100.0, 8usize);
    let (nodes, el) = grid(l, b, nx, 1);
    let mut mesh = shell_mesh(nodes.clone(), &el, t);
    mesh.add_node_set("root", vec![0, nx + 1]).unwrap();
    let mut m = model_of(mesh, mat(0.0));
    for comp in 1..6 {
        m.add_constraint(Constraint::displacement("all", comp, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::displacement("root", 0, 0.0)).unwrap();
    m.add_load(Load::centrifugal([0.0, 0.0, 1.0], [0.0; 3], om)).unwrap();
    let r = solve_static(&m, &StaticOptions::default()).unwrap();
    let exact = |x: f64| RHO * om * om * (l * l * x - x * x * x / 3.0) / (2.0 * E);
    let utip = exact(l);
    let mut d = 0.0f64;
    for (n, x) in nodes.iter().enumerate() {
        d = d.max((r.displacement(n)[0] - exact(x[0])).abs());
    }
    c.at_most("C1 u_x at every node against the 1D bar", d / utip, 1e-10);
    // C1b: the nodal loads against the rectangle closed form.
    let f = nodal_load_vector(&m, &[]).unwrap();
    let h = l / nx as f64;
    let s = RHO * t * om * om;
    let mut expected = vec![[0.0; 6]; nodes.len()];
    for i in 0..nx {
        let x0 = i as f64 * h;
        for (row, fy) in [(0usize, b * b / 6.0), (1, b * b / 3.0)] {
            let a = row * (nx + 1) + i;
            expected[a][0] += s * 0.5 * b * h * (0.5 * x0 + h / 6.0);
            expected[a + 1][0] += s * 0.5 * b * h * (0.5 * x0 + h / 3.0);
            expected[a][1] += s * 0.5 * h * fy;
            expected[a + 1][1] += s * 0.5 * h * fy;
        }
    }
    let emax = max_abs(expected.iter().flatten().copied());
    let mut d = 0.0f64;
    for (n, ex) in expected.iter().enumerate() {
        let e0 = m.dof_map().equation(n, 0).unwrap();
        for k in 0..6 {
            d = d.max((f[e0 + k] - ex[k]).abs());
        }
    }
    c.at_most("C1b nodal loads against the rectangle closed form", d / emax, 1e-12);
    // C2: the resultant on the distorted patch in space.
    let p = patch();
    let (area, centroid, _, _) = patch_integrals();
    let (axis, o, om2) = ([1.0, 2.0, -0.5], [0.1, 0.2, -0.3], 50.0);
    let mut mp = p.model.clone();
    mp.add_load(Load::centrifugal(axis, o, om2)).unwrap();
    let f = nodal_load_vector(&mp, &[]).unwrap();
    let mut sf = [0.0; 3];
    for k in 0..16 {
        sf = add(sf, node_force_moment(&mp, &f, k).0);
    }
    let a = scale(1.0 / norm(axis), axis);
    let dxc = sub(p.place(centroid), o);
    let perp = sub(dxc, scale(dot(dxc, a), a));
    let res = scale(RHO * PATCH_T * om2 * om2 * area, perp);
    c.at_most("C2 centrifugal resultant", norm(sub(sf, res)) / norm(res), 1e-12);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- S3, S3c, N2 to N4

#[test]
#[ignore = "record of ADR 0432: ADR 0473 (F-420) takes the centrifugal load on a beam, retiring the N4 refusal row; continued in s3_n2_to_n4_refusals_after_f420 (ADR 0291 pattern, maintainer standing decision)"]
fn s3_n2_to_n4_refusals() {
    let mut c = Checks::new();
    // S3, S3c: the entry refusal before the transformation is built (arithmetic 8).
    let hex = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let at = [[2.0, 0.0, 0.0], [2.0, 1.0, 0.0], [2.0, 1.0, 1.0], [2.0, 0.0, 1.0]];
    let build = |shared: bool| -> (SolidModel, usize) {
        let mut nodes = hex.nodes().to_vec();
        let conn: Vec<usize> = if shared {
            at.iter().map(|p| nodes.iter().position(|q| q == p).unwrap()).collect()
        } else {
            let n0 = nodes.len();
            nodes.extend(at.iter().map(|p| [3.0, p[1], p[2]]));
            (n0..n0 + 4).collect()
        };
        let mut mesh = SolidMesh::new(nodes).unwrap();
        mesh.add_block(ElementKind::Hex8, hex.blocks()[0].connectivity().to_vec())
            .unwrap();
        mesh.add_block(ElementKind::Shell4, conn.clone()).unwrap();
        mesh.set_block_property(1, BlockProperty::shell(0.01).unwrap()).unwrap();
        mesh.add_element_set("solid", vec![0, 1]).unwrap();
        mesh.add_element_set("shell", vec![2]).unwrap();
        mesh.add_node_set("xmin", hex.node_set("xmin").unwrap().to_vec())
            .unwrap();
        let mut m = SolidModel::new(mesh);
        m.add_material("solid", mat(0.3)).unwrap();
        m.add_material("shell", mat(0.3)).unwrap();
        for comp in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
        }
        (m, conn[0])
    };
    let set_a = |m: &mut SolidModel, node: usize| {
        m.add_constraint(Constraint::linear(vec![LinearTerm::new(node, 0, 1.0)], 0.0))
            .unwrap();
        m.add_constraint(Constraint::linear(vec![LinearTerm::new(node, 0, 1.0)], 1e-3))
            .unwrap();
    };
    let set_b = |m: &mut SolidModel, node: usize| {
        m.add_constraint(Constraint::linear(
            vec![LinearTerm::new(node, 0, 1.0), LinearTerm::new(node, 0, -1.0)],
            0.0,
        ))
        .unwrap();
    };
    let (shared, first) = build(true);
    let names = |r: &Result<(), FemError>| -> (bool, String) {
        match r {
            Err(e @ FemError::Element { element: 2, source }) => (
                matches!(source.as_ref(), FemError::InvalidValue { key: "node", .. })
                    && e.to_string().contains(&format!("node {first} ")),
                e.to_string(),
            ),
            other => (false, format!("{other:?}")),
        }
    };
    let mut a = shared.clone();
    set_a(&mut a, first);
    let mut bm = shared.clone();
    set_b(&mut bm, first);
    let r = solve_static_mpc(&a, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    let (ok, msg) = names(&r);
    c.holds("S3 solve_static_mpc, set A", ok, &msg);
    let r = modal_analysis_mpc(&a, &[], &ModalOptions::new(2)).map(|_| ());
    let (ok, msg) = names(&r);
    c.holds("S3 modal_analysis_mpc, set A", ok, &msg);
    let r = modal_analysis_mpc(&bm, &[], &ModalOptions::new(2)).map(|_| ());
    let (ok, msg) = names(&r);
    c.holds("S3 modal_analysis_mpc, set B", ok, &msg);
    let (control, cfirst) = build(false);
    let says = |r: &Result<(), FemError>, text: &str| -> (bool, String) {
        match r {
            Err(e @ FemError::InvalidValue { key: "constraint", .. }) => (e.to_string().contains(text), e.to_string()),
            other => (false, format!("{other:?}")),
        }
    };
    let mut a = control.clone();
    set_a(&mut a, cfirst);
    let mut bm = control.clone();
    set_b(&mut bm, cfirst);
    let r = solve_static_mpc(&a, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    let (ok, msg) = says(&r, "contradicts");
    c.holds("S3c solve_static_mpc, set A", ok, &msg);
    let r = modal_analysis_mpc(&a, &[], &ModalOptions::new(2)).map(|_| ());
    let (ok, msg) = says(&r, "takes zero values only");
    c.holds("S3c modal_analysis_mpc, set A", ok, &msg);
    let r = modal_analysis_mpc(&bm, &[], &ModalOptions::new(2)).map(|_| ());
    let (ok, msg) = says(&r, "cancel to zero");
    c.holds("S3c modal_analysis_mpc, set B", ok, &msg);
    // N2: a shell model with a cyclic constraint.
    let hb = hex_box([0.0, 1.0, 0.0], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
    let nh = hb.nodes().len();
    let (sn, se) = grid(1.0, KB, 4, 1);
    let mut nodes = hb.nodes().to_vec();
    nodes.extend_from_slice(&sn);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hb.blocks()[0].connectivity().to_vec())
        .unwrap();
    let conn: Vec<usize> = se.iter().flatten().map(|&k| nh + k).collect();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(1, BlockProperty::shell(KT).unwrap()).unwrap();
    mesh.add_element_set("solid", vec![0]).unwrap();
    mesh.add_element_set("shell", (1..5).collect()).unwrap();
    for name in BOX_FACES {
        mesh.add_node_set(name, hb.node_set(name).unwrap().to_vec()).unwrap();
        mesh.add_surface_set(name, hb.surface_set(name).unwrap().to_vec())
            .unwrap();
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("solid", mat(0.3)).unwrap();
    m.add_material("shell", mat(0.0)).unwrap();
    m.add_constraint(Constraint::cyclic("ymin", "ymax", [0.0, 0.0, 1.0], [0.0; 3], 4))
        .unwrap();
    let cyclic = |r: &Result<(), FemError>| matches!(r, Err(FemError::NotYetSupported { variant: "Cyclic", package, .. }) if *package == shell::FOLLOW_UP);
    let rs = solve_static(&m, &StaticOptions::default()).map(|_| ());
    c.holds("N2 cyclic with a shell: solve_static", cyclic(&rs), &format!("{rs:?}"));
    let rm = modal_analysis(&m, &ModalOptions::new(2)).map(|_| ());
    c.holds(
        "N2 cyclic with a shell: modal_analysis",
        cyclic(&rm),
        &format!("{rm:?}"),
    );
    // N3: the H1 model on paths that do not take shells.
    let mut h1 = plate(true);
    h1.add_constraint(Constraint::rigid_set("edge", [0.0, 0.5, 0.0]))
        .unwrap();
    clamp(&mut h1, "corner");
    let follow_up = |r: &Result<(), FemError>| matches!(r, Err(FemError::NotYetSupported { variant: "Shell4", package, .. }) if *package == shell::FOLLOW_UP);
    let r = h1.check_supported();
    c.holds("N3 check_supported (the other paths)", follow_up(&r), &format!("{r:?}"));
    let r = rcfd_fem::nonlinear::initial_states(&h1).map(|_| ());
    c.holds("N3 nonlinear initial states", follow_up(&r), &format!("{r:?}"));
    let pcg = StaticOptions::with_solver(StaticSolver::PreconditionedCg {
        preconditioner: CgPreconditioner::BlockJacobi,
        tolerance: 1e-10,
        max_iterations: 100,
    });
    let r = solve_static(&h1, &pcg).map(|_| ());
    c.holds(
        "N3 preconditioned CG on the transformation",
        matches!(&r, Err(FemError::InvalidValue { key: "solver", .. })),
        &format!("{r:?}"),
    );
    // N4: centrifugal on a beam model, unchanged.
    let mut mesh = SolidMesh::new(vec![[0.0; 3], [0.5, 0.0, 0.0], [1.0, 0.0, 0.0]]).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2]).unwrap();
    let section = BeamSection::new(0.012, 3e-6, 5e-6, 4e-6, 0.85, 0.75).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    mesh.add_node_set("root", vec![0]).unwrap();
    let mut bm = model_of(mesh, mat(0.3));
    clamp(&mut bm, "root");
    bm.add_load(Load::centrifugal([0.0, 0.0, 1.0], [0.0; 3], 10.0)).unwrap();
    let r = solve_static(&bm, &StaticOptions::default()).map(|_| ());
    let ok = matches!(&r, Err(FemError::Element { source, .. })
        if matches!(source.as_ref(), FemError::NotYetSupported { package, .. } if *package == rcfd_fem::beam::SOLID_ROUTINES));
    c.holds("N4 centrifugal on a beam (unchanged)", ok, &format!("{r:?}"));
    c.finish();
}

// S3, S3c, N2 to N4 after ADR 0473 (F-420, check D1): N4, the centrifugal load on a beam, now solves and the root
// reactions balance the centrifugal force; every other row as before.
#[test]
fn s3_n2_to_n4_refusals_after_f420() {
    let mut c = Checks::new();
    // S3, S3c: the entry refusal before the transformation is built (arithmetic 8).
    let hex = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let at = [[2.0, 0.0, 0.0], [2.0, 1.0, 0.0], [2.0, 1.0, 1.0], [2.0, 0.0, 1.0]];
    let build = |shared: bool| -> (SolidModel, usize) {
        let mut nodes = hex.nodes().to_vec();
        let conn: Vec<usize> = if shared {
            at.iter().map(|p| nodes.iter().position(|q| q == p).unwrap()).collect()
        } else {
            let n0 = nodes.len();
            nodes.extend(at.iter().map(|p| [3.0, p[1], p[2]]));
            (n0..n0 + 4).collect()
        };
        let mut mesh = SolidMesh::new(nodes).unwrap();
        mesh.add_block(ElementKind::Hex8, hex.blocks()[0].connectivity().to_vec())
            .unwrap();
        mesh.add_block(ElementKind::Shell4, conn.clone()).unwrap();
        mesh.set_block_property(1, BlockProperty::shell(0.01).unwrap()).unwrap();
        mesh.add_element_set("solid", vec![0, 1]).unwrap();
        mesh.add_element_set("shell", vec![2]).unwrap();
        mesh.add_node_set("xmin", hex.node_set("xmin").unwrap().to_vec())
            .unwrap();
        let mut m = SolidModel::new(mesh);
        m.add_material("solid", mat(0.3)).unwrap();
        m.add_material("shell", mat(0.3)).unwrap();
        for comp in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
        }
        (m, conn[0])
    };
    let set_a = |m: &mut SolidModel, node: usize| {
        m.add_constraint(Constraint::linear(vec![LinearTerm::new(node, 0, 1.0)], 0.0))
            .unwrap();
        m.add_constraint(Constraint::linear(vec![LinearTerm::new(node, 0, 1.0)], 1e-3))
            .unwrap();
    };
    let set_b = |m: &mut SolidModel, node: usize| {
        m.add_constraint(Constraint::linear(
            vec![LinearTerm::new(node, 0, 1.0), LinearTerm::new(node, 0, -1.0)],
            0.0,
        ))
        .unwrap();
    };
    let (shared, first) = build(true);
    let names = |r: &Result<(), FemError>| -> (bool, String) {
        match r {
            Err(e @ FemError::Element { element: 2, source }) => (
                matches!(source.as_ref(), FemError::InvalidValue { key: "node", .. })
                    && e.to_string().contains(&format!("node {first} ")),
                e.to_string(),
            ),
            other => (false, format!("{other:?}")),
        }
    };
    let mut a = shared.clone();
    set_a(&mut a, first);
    let mut bm = shared.clone();
    set_b(&mut bm, first);
    let r = solve_static_mpc(&a, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    let (ok, msg) = names(&r);
    c.holds("S3 solve_static_mpc, set A", ok, &msg);
    let r = modal_analysis_mpc(&a, &[], &ModalOptions::new(2)).map(|_| ());
    let (ok, msg) = names(&r);
    c.holds("S3 modal_analysis_mpc, set A", ok, &msg);
    let r = modal_analysis_mpc(&bm, &[], &ModalOptions::new(2)).map(|_| ());
    let (ok, msg) = names(&r);
    c.holds("S3 modal_analysis_mpc, set B", ok, &msg);
    let (control, cfirst) = build(false);
    let says = |r: &Result<(), FemError>, text: &str| -> (bool, String) {
        match r {
            Err(e @ FemError::InvalidValue { key: "constraint", .. }) => (e.to_string().contains(text), e.to_string()),
            other => (false, format!("{other:?}")),
        }
    };
    let mut a = control.clone();
    set_a(&mut a, cfirst);
    let mut bm = control.clone();
    set_b(&mut bm, cfirst);
    let r = solve_static_mpc(&a, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    let (ok, msg) = says(&r, "contradicts");
    c.holds("S3c solve_static_mpc, set A", ok, &msg);
    let r = modal_analysis_mpc(&a, &[], &ModalOptions::new(2)).map(|_| ());
    let (ok, msg) = says(&r, "takes zero values only");
    c.holds("S3c modal_analysis_mpc, set A", ok, &msg);
    let r = modal_analysis_mpc(&bm, &[], &ModalOptions::new(2)).map(|_| ());
    let (ok, msg) = says(&r, "cancel to zero");
    c.holds("S3c modal_analysis_mpc, set B", ok, &msg);
    // N2: a shell model with a cyclic constraint.
    let hb = hex_box([0.0, 1.0, 0.0], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
    let nh = hb.nodes().len();
    let (sn, se) = grid(1.0, KB, 4, 1);
    let mut nodes = hb.nodes().to_vec();
    nodes.extend_from_slice(&sn);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hb.blocks()[0].connectivity().to_vec())
        .unwrap();
    let conn: Vec<usize> = se.iter().flatten().map(|&k| nh + k).collect();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(1, BlockProperty::shell(KT).unwrap()).unwrap();
    mesh.add_element_set("solid", vec![0]).unwrap();
    mesh.add_element_set("shell", (1..5).collect()).unwrap();
    for name in BOX_FACES {
        mesh.add_node_set(name, hb.node_set(name).unwrap().to_vec()).unwrap();
        mesh.add_surface_set(name, hb.surface_set(name).unwrap().to_vec())
            .unwrap();
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("solid", mat(0.3)).unwrap();
    m.add_material("shell", mat(0.0)).unwrap();
    m.add_constraint(Constraint::cyclic("ymin", "ymax", [0.0, 0.0, 1.0], [0.0; 3], 4))
        .unwrap();
    let cyclic = |r: &Result<(), FemError>| matches!(r, Err(FemError::NotYetSupported { variant: "Cyclic", package, .. }) if *package == shell::FOLLOW_UP);
    let rs = solve_static(&m, &StaticOptions::default()).map(|_| ());
    c.holds("N2 cyclic with a shell: solve_static", cyclic(&rs), &format!("{rs:?}"));
    let rm = modal_analysis(&m, &ModalOptions::new(2)).map(|_| ());
    c.holds(
        "N2 cyclic with a shell: modal_analysis",
        cyclic(&rm),
        &format!("{rm:?}"),
    );
    // N3: the H1 model on paths that do not take shells.
    let mut h1 = plate(true);
    h1.add_constraint(Constraint::rigid_set("edge", [0.0, 0.5, 0.0]))
        .unwrap();
    clamp(&mut h1, "corner");
    let follow_up = |r: &Result<(), FemError>| matches!(r, Err(FemError::NotYetSupported { variant: "Shell4", package, .. }) if *package == shell::FOLLOW_UP);
    let r = h1.check_supported();
    c.holds("N3 check_supported (the other paths)", follow_up(&r), &format!("{r:?}"));
    let r = rcfd_fem::nonlinear::initial_states(&h1).map(|_| ());
    c.holds("N3 nonlinear initial states", follow_up(&r), &format!("{r:?}"));
    let pcg = StaticOptions::with_solver(StaticSolver::PreconditionedCg {
        preconditioner: CgPreconditioner::BlockJacobi,
        tolerance: 1e-10,
        max_iterations: 100,
    });
    let r = solve_static(&h1, &pcg).map(|_| ());
    c.holds(
        "N3 preconditioned CG on the transformation",
        matches!(&r, Err(FemError::InvalidValue { key: "solver", .. })),
        &format!("{r:?}"),
    );
    // N4: centrifugal on a beam model, unchanged.
    let mut mesh = SolidMesh::new(vec![[0.0; 3], [0.5, 0.0, 0.0], [1.0, 0.0, 0.0]]).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2]).unwrap();
    let section = BeamSection::new(0.012, 3e-6, 5e-6, 4e-6, 0.85, 0.75).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    mesh.add_node_set("root", vec![0]).unwrap();
    let mut bm = model_of(mesh, mat(0.3));
    clamp(&mut bm, "root");
    bm.add_load(Load::centrifugal([0.0, 0.0, 1.0], [0.0; 3], 10.0)).unwrap();
    // ADR 0473 (check D1): the centrifugal load on a beam is taken; the root reactions balance rho A Omega^2 L x_mid e_x.
    let r = solve_static(&bm, &StaticOptions::default()).unwrap();
    let start = r.dofs().equation(0, 0).unwrap();
    let rf = &r.reactions().unwrap()[start..start + 3];
    let fc = RHO * 0.012 * 10.0 * 10.0 * 1.0 * 0.5;
    c.at_most(
        "N4 centrifugal on a beam (ADR 0473 check D1): |R + F_c| / |F_c|",
        norm([rf[0] + fc, rf[1], rf[2]]) / fc,
        1e-12,
    );
    c.finish();
}
