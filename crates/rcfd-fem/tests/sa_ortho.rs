//! F-409 (ADR 0452): orthotropic elasticity with material axes and orthotropic expansion. Checks O1 to O9, K1, P1, R1 to R3
//! and the negative controls N1, N2, N4 of the registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::geometric::{prestressed_modes, Prestress};
use rcfd_fem::hex20::{pressure_loads, traction_loads, upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::mat_ortho::{
    compliance, cylindrical_axes, material_elasticity, rotate_elasticity, FOLLOW_UP, PENDING as ORTHO_PENDING,
};
use rcfd_fem::material::{isotropic_elasticity, MaterialAxes, Matrix6, PointContext};
use rcfd_fem::modal::{modal_analysis_with, ModalOptions};
use rcfd_fem::mpc::{modal_analysis_mpc, solve_static_mpc};
use rcfd_fem::nonlinear::{solve, NewtonOptions};
use rcfd_fem::recover::{nodal_average, IntegrationPointField};
use rcfd_fem::solve::{solve_static, solve_static_with, CgPreconditioner, StaticOptions, StaticSolver};
use rcfd_fem::transient::{Integrator, TransientOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel, SolidResult};
use std::f64::consts::PI;

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {} ({detail})", if ok { "met" } else { "MISSED" });
        if !ok {
            self.failed.push(name.to_string());
        }
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.4e} (expected > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{name} = {value:e} <= {limit:e}"));
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "missed: {:?}", self.failed);
    }
}

// ------------------------------------------------------------------------------------------------------- helpers

const E_O: [f64; 3] = [1.4e11, 1.0e10, 1.0e10];
const NU_O: [f64; 3] = [0.3, 0.3, 0.45];
const G_O: [f64; 3] = [5.0e9, 5.0e9, 3.5e9];
const ALPHA: [f64; 3] = [-1e-6, 3e-5, 2e-5];
const RHO: f64 = 1600.0;

type Axes = [[f64; 3]; 3];

/// Rodrigues rotation matrix (rows orthonormal, right-handed): the `rot` of `sa_scaffold.rs`.
fn rot(axis: Vec3, angle: f64) -> Axes {
    let n = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let k = [axis[0] / n, axis[1] / n, axis[2] / n];
    let (s, co) = angle.sin_cos();
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let kk = [[0.0, -k[2], k[1]], [k[2], 0.0, -k[0]], [-k[1], k[0], 0.0]][i][j];
            r[i][j] = (if i == j { co } else { 0.0 }) + s * kk + (1.0 - co) * k[i] * k[j];
        }
    }
    r
}

fn about_z(deg: f64) -> Axes {
    let (s, c) = (deg * PI / 180.0).sin_cos();
    [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]]
}

fn ortho(axes: MaterialAxes) -> Material {
    Material::orthotropic(E_O, NU_O, G_O, RHO, ALPHA, axes).unwrap()
}

fn fixed(r: Axes) -> MaterialAxes {
    MaterialAxes::fixed(r).unwrap()
}

/// The compliance of F1 written again in the test (independent of the library).
fn test_compliance(e: [f64; 3], nu: [f64; 3], g: [f64; 3]) -> [[f64; 6]; 6] {
    let mut s = [[0.0; 6]; 6];
    s[0][0] = 1.0 / e[0];
    s[1][1] = 1.0 / e[1];
    s[2][2] = 1.0 / e[2];
    s[0][1] = -nu[0] / e[0];
    s[1][0] = s[0][1];
    s[0][2] = -nu[1] / e[0];
    s[2][0] = s[0][2];
    s[1][2] = -nu[2] / e[1];
    s[2][1] = s[1][2];
    s[3][3] = 1.0 / g[0];
    s[4][4] = 1.0 / g[2];
    s[5][5] = 1.0 / g[1];
    s
}

/// Gauss-Jordan inverse with partial pivoting.
fn inverse6(a: &[[f64; 6]; 6]) -> [[f64; 6]; 6] {
    let mut m = *a;
    let mut inv = [[0.0; 6]; 6];
    for (i, row) in inv.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    for c in 0..6 {
        let p = (c..6).max_by(|&i, &j| m[i][c].abs().total_cmp(&m[j][c].abs())).unwrap();
        m.swap(c, p);
        inv.swap(c, p);
        let d = m[c][c];
        for j in 0..6 {
            m[c][j] /= d;
            inv[c][j] /= d;
        }
        for r in 0..6 {
            if r != c {
                let f = m[r][c];
                for j in 0..6 {
                    m[r][j] -= f * m[c][j];
                    inv[r][j] -= f * inv[c][j];
                }
            }
        }
    }
    inv
}

const PAIRS: [(usize, usize); 6] = [(0, 0), (1, 1), (2, 2), (0, 1), (1, 2), (2, 0)];

fn tensor_of_strain(v: &[f64; 6]) -> [[f64; 3]; 3] {
    let mut t = [[0.0; 3]; 3];
    for (k, &(i, j)) in PAIRS.iter().enumerate() {
        let x = if i == j { v[k] } else { v[k] / 2.0 };
        t[i][j] = x;
        t[j][i] = x;
    }
    t
}

fn tensor_of_stress(v: &[f64; 6]) -> [[f64; 3]; 3] {
    let mut t = [[0.0; 3]; 3];
    for (k, &(i, j)) in PAIRS.iter().enumerate() {
        t[i][j] = v[k];
        t[j][i] = v[k];
    }
    t
}

/// `R t R^T` (to material axes).
fn to_material(r: &Axes, t: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut o = [[0.0; 3]; 3];
    for a in 0..3 {
        for b in 0..3 {
            o[a][b] = (0..3)
                .flat_map(|i| (0..3).map(move |j| (i, j)))
                .map(|(i, j)| r[a][i] * t[i][j] * r[b][j])
                .sum();
        }
    }
    o
}

/// `R^T t R` (to global axes).
fn to_global(r: &Axes, t: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut o = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = (0..3)
                .flat_map(|a| (0..3).map(move |b| (a, b)))
                .map(|(a, b)| r[a][i] * t[a][b] * r[b][j])
                .sum();
        }
    }
    o
}

fn voigt_strain(t: &[[f64; 3]; 3]) -> [f64; 6] {
    std::array::from_fn(|k| {
        let (i, j) = PAIRS[k];
        if i == j {
            t[i][j]
        } else {
            2.0 * t[i][j]
        }
    })
}

fn voigt_stress(t: &[[f64; 3]; 3]) -> [f64; 6] {
    std::array::from_fn(|k| {
        let (i, j) = PAIRS[k];
        t[i][j]
    })
}

/// The global strain of a unit `sigma_xx` by the test's own tensor rotation of the material compliance.
fn strain_of_unit_sigma_xx(e: [f64; 3], nu: [f64; 3], g: [f64; 3], r: &Axes) -> [f64; 6] {
    let mut sg = [[0.0; 3]; 3];
    sg[0][0] = 1.0;
    let sm = voigt_stress(&to_material(r, &sg));
    let s = test_compliance(e, nu, g);
    let em: [f64; 6] = std::array::from_fn(|i| (0..6).map(|j| s[i][j] * sm[j]).sum());
    voigt_strain(&to_global(r, &tensor_of_strain(&em)))
}

fn max_abs(v: impl IntoIterator<Item = f64>) -> f64 {
    v.into_iter().fold(0.0, |a: f64, x| a.max(x.abs()))
}

fn rel_matrix(a: &Matrix6, b: &Matrix6) -> f64 {
    let scale = max_abs(b.iter().flatten().copied());
    max_abs(a.iter().flatten().zip(b.iter().flatten()).map(|(x, y)| x - y)) / scale
}

fn node_at(mesh: &SolidMesh, x: Vec3) -> usize {
    mesh.nodes()
        .iter()
        .position(|p| (0..3).all(|i| (p[i] - x[i]).abs() <= 1e-12))
        .expect("node")
}

/// The unit cube of one Hex8 with the node sets of the three-two-one support (`p0` (0,0,0), `p1` (1,0,0), `p2` (0,1,0))
/// and `every`.
fn cube() -> SolidMesh {
    let mut mesh = hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap();
    let p0 = node_at(&mesh, [0.0, 0.0, 0.0]);
    let p1 = node_at(&mesh, [1.0, 0.0, 0.0]);
    let p2 = node_at(&mesh, [0.0, 1.0, 0.0]);
    let n = mesh.nodes().len();
    mesh.add_node_set("p0", vec![p0]).unwrap();
    mesh.add_node_set("p1", vec![p1]).unwrap();
    mesh.add_node_set("p2", vec![p2]).unwrap();
    mesh.add_node_set("every", (0..n).collect()).unwrap();
    mesh
}

fn three_two_one(m: &mut SolidModel) {
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("p0", c, 0.0)).unwrap();
    }
    for c in 1..3 {
        m.add_constraint(Constraint::displacement("p1", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::displacement("p2", 2, 0.0)).unwrap();
}

fn field_of(m: &SolidModel, r: &SolidResult) -> IntegrationPointField {
    IntegrationPointField::compute(m, r.displacement_values()).unwrap()
}

/// O4's block: the cube with `+-sigma` on `xmax`/`xmin` and the three-two-one support.
fn off_axis_block(mat: Material) -> SolidModel {
    let mut m = SolidModel::new(cube());
    m.add_material("all", mat).unwrap();
    three_two_one(&mut m);
    m.add_load(Load::traction("xmax", [SIGMA, 0.0, 0.0])).unwrap();
    m.add_load(Load::traction("xmin", [-SIGMA, 0.0, 0.0])).unwrap();
    m
}

const SIGMA: f64 = 1e6;

// ------------------------------------------------------------------------------------------------- O1 to O3

#[test]
fn o1_to_o3_constants() {
    let mut c = Checks::new();
    c.holds("X mat_ortho landed", ORTHO_PENDING.is_none(), "");
    // O1.
    let e = 2.0e11;
    for nu in [0.0, 0.3, 0.45, -0.2] {
        let g = e / (2.0 * (1.0 + nu));
        let iso = isotropic_elasticity(e, nu);
        let m = Material::orthotropic([e; 3], [nu; 3], [g; 3], RHO, [0.0; 3], fixed(about_z(0.0))).unwrap();
        let d0 = m.elasticity_at(&PointContext::default()).unwrap();
        c.at_most(&format!("O1 nu = {nu} global axes"), rel_matrix(&d0, &iso), 1e-13);
        let r = rot([1.0, 2.0, 3.0], 0.5);
        let dr = m.elasticity_at(&PointContext::default().with_axes(r).unwrap()).unwrap();
        c.at_most(&format!("O1 nu = {nu} rotated axes"), rel_matrix(&dr, &iso), 1e-13);
    }
    // O2.
    let eq = [1.0e10; 3];
    let gq = [4.0e9; 3];
    let refused = |nu: [f64; 3], e: [f64; 3]| -> Option<&'static str> {
        match Material::orthotropic(e, nu, gq, RHO, [0.0; 3], fixed(about_z(0.0))) {
            Err(FemError::InvalidMaterial { key, .. }) => Some(key),
            _ => None,
        }
    };
    for (nu, key) in [
        ([1.05, 0.2, 0.2], "nu_12"),
        ([0.2, 1.05, 0.2], "nu_13"),
        ([0.2, 0.2, 1.05], "nu_23"),
        ([0.5, 0.5, 0.5], "nu_12, nu_13, nu_23"),
    ] {
        let got = refused(nu, eq);
        c.holds(
            &format!("O2 {nu:?} refused naming {key}"),
            got == Some(key),
            &format!("{got:?}"),
        );
    }
    let ok1 = Material::orthotropic(eq, [0.499; 3], gq, RHO, [0.0; 3], fixed(about_z(0.0))).is_ok();
    let ok2 = Material::orthotropic(E_O, NU_O, G_O, RHO, ALPHA, fixed(about_z(0.0))).is_ok();
    let ok3 = Material::orthotropic(E_O, [-0.5, 0.3, 0.45], G_O, RHO, ALPHA, fixed(about_z(0.0))).is_ok();
    c.holds(
        "O2 accepted: nu 0.499, ORTHO, nu_12 = -0.5",
        ok1 && ok2 && ok3,
        &format!("{ok1} {ok2} {ok3}"),
    );
    // O3.
    let s = compliance(E_O, NU_O, G_O);
    let sym = (0..6).all(|i| (0..6).all(|j| s[i][j].to_bits() == s[j][i].to_bits()));
    c.holds("O3 compliance symmetric bit for bit", sym, "");
    let pairs = [
        (-s[0][1] * E_O[1], NU_O[0] * E_O[1] / E_O[0]),
        (-s[0][2] * E_O[2], NU_O[1] * E_O[2] / E_O[0]),
        (-s[1][2] * E_O[2], NU_O[2] * E_O[2] / E_O[1]),
    ];
    let recip = max_abs(pairs.iter().map(|(a, b)| (a - b) / b));
    c.at_most("O3 reciprocity nu_ji = nu_ij E_j/E_i", recip, 1e-15);
    let d = rotate_elasticity(&material_elasticity(E_O, NU_O, G_O), &rot([1.0, -2.0, 0.5], 1.1));
    let dsym = (0..6).all(|i| (0..6).all(|j| d[i][j].to_bits() == d[j][i].to_bits()));
    c.holds("O3 rotated elasticity symmetric bit for bit", dsym, "");
    c.finish();
}

// ------------------------------------------------------------------------------------------------- O4, N4, P1

#[test]
fn o4_n4_p1_off_axis_tension() {
    let mut c = Checks::new();
    for deg in [15.0, 30.0, 45.0, 60.0] {
        let r = about_z(deg);
        let m = off_axis_block(ortho(fixed(r)));
        let res = solve_static(&m, &StaticOptions::default()).unwrap();
        let field = field_of(&m, &res);
        let (s, co) = (deg * PI / 180.0).sin_cos();
        let inv_ex =
            co.powi(4) / E_O[0] + s.powi(4) / E_O[1] + (1.0 / G_O[0] - 2.0 * NU_O[0] / E_O[0]) * s * s * co * co;
        let (s11, s22, s12, s66) = (1.0 / E_O[0], 1.0 / E_O[1], -NU_O[0] / E_O[0], 1.0 / G_O[0]);
        let s16 = (2.0 * s11 - 2.0 * s12 - s66) * s * co.powi(3) - (2.0 * s22 - 2.0 * s12 - s66) * s.powi(3) * co;
        let eta = s16 / inv_ex;
        let col = strain_of_unit_sigma_xx(E_O, NU_O, G_O, &r);
        let (mut ea, mut ee, mut ec): (f64, f64, f64) = (0.0, 0.0, 0.0);
        for eps in field.strain() {
            ea = ea.max((eps[0] / (SIGMA * inv_ex) - 1.0).abs());
            ee = ee.max((eps[3] / eps[0] - eta).abs() / eta.abs());
            ec = ec.max(max_abs((0..6).map(|k| eps[k] - SIGMA * col[k])) / eps[0].abs());
        }
        println!("O4 {deg} deg: eta = {eta:.6}, {} points", field.n_points());
        c.at_most(&format!("O4 {deg} deg axial strain sigma/E_x"), ea, 1e-10);
        c.at_most(&format!("O4 {deg} deg shear-extension ratio"), ee, 1e-10);
        c.at_most(
            &format!("O4 {deg} deg six strains against the rotated compliance"),
            ec,
            1e-10,
        );
    }
    // N4: isotropic constants at 30 degrees, no shear-extension coupling.
    let (e, nu) = (2.0e11, 0.3);
    let iso = Material::orthotropic(
        [e; 3],
        [nu; 3],
        [e / (2.0 * (1.0 + nu)); 3],
        RHO,
        [0.0; 3],
        fixed(about_z(30.0)),
    )
    .unwrap();
    let m = off_axis_block(iso);
    let res = solve_static(&m, &StaticOptions::default()).unwrap();
    let field = field_of(&m, &res);
    c.at_most(
        "N4 isotropic constants |gamma_xy/eps_xx|",
        max_abs(field.strain().iter().map(|e| e[3] / e[0])),
        1e-10,
    );
    // P1.
    let r = about_z(30.0);
    let a = solve_static(&off_axis_block(ortho(fixed(r))), &StaticOptions::default()).unwrap();
    let b = solve_static(
        &off_axis_block(ortho(MaterialAxes::per_element(vec![r]).unwrap())),
        &StaticOptions::default(),
    )
    .unwrap();
    let same = a
        .displacement_values()
        .iter()
        .zip(b.displacement_values())
        .all(|(x, y)| x.to_bits() == y.to_bits());
    c.holds("P1 PerElement([R]) bits of Fixed(R)", same, "");
    let two = solve_static(
        &off_axis_block(ortho(MaterialAxes::per_element(vec![r, r]).unwrap())),
        &StaticOptions::default(),
    );
    c.holds(
        "P1 two triads for one element refused (axes)",
        matches!(&two, Err(FemError::InvalidValue { key: "axes", .. })),
        &format!("{:?}", two.as_ref().err()),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- O5

fn apply3(q: &Axes, x: Vec3) -> Vec3 {
    std::array::from_fn(|i| (0..3).map(|j| q[i][j] * x[j]).sum())
}

fn energy_model(mesh: SolidMesh, r: Axes, forces: &[Vec3]) -> (SolidModel, Vec<NodalForceAt>) {
    let mut xmax: Vec<usize> = mesh.node_set("xmax").unwrap().to_vec();
    xmax.sort_unstable();
    let loads: Vec<NodalForceAt> = xmax
        .iter()
        .zip(forces)
        .map(|(&n, &f)| NodalForceAt::new(n, f))
        .collect();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", ortho(fixed(r))).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    (m, loads)
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

#[test]
fn o5_rotation_invariance() {
    let mut c = Checks::new();
    let forces: [Vec3; 4] = [[1e5, 2e5, -3e5], [-2e5, 1e5, 1e5], [3e5, -1e5, 2e5], [1e5, 1e5, 1e5]];
    let r = rot([1.0, -1.0, 2.0], 0.4);
    let q = rot([1.0, 2.0, 3.0], 0.7);
    let base = hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap();
    let (m0, l0) = energy_model(base.clone(), r, &forces);
    let rq: Axes = std::array::from_fn(|a| apply3(&q, r[a]));
    let moved = rebuild(&base, base.nodes().iter().map(|&x| apply3(&q, x)).collect());
    let fq: Vec<Vec3> = forces.iter().map(|&f| apply3(&q, f)).collect();
    let (m1, l1) = energy_model(moved, rq, &fq);
    let work = |m: &SolidModel, l: &[NodalForceAt]| {
        let res = solve_static_with(m, &[], l, &StaticOptions::default()).unwrap();
        let w: f64 = l
            .iter()
            .map(|f| (0..3).map(|i| f.force[i] * res.displacement(f.node)[i]).sum::<f64>())
            .sum::<f64>()
            / 2.0;
        (w, field_of(m, &res).strain_energy())
    };
    let (w0, u0) = work(&m0, &l0);
    let (w1, u1) = work(&m1, &l1);
    println!("O5: W = {w0:.12e}, rotated {w1:.12e}; recovered {u0:.12e}, {u1:.12e}");
    c.at_most("O5 |W'/W - 1|", (w1 / w0 - 1.0).abs(), 1e-12);
    c.at_most(
        "O5 recovered energy of the model against W",
        (u0 / w0 - 1.0).abs(),
        1e-12,
    );
    c.at_most(
        "O5 recovered energy of the twin against W'",
        (u1 / w1 - 1.0).abs(),
        1e-12,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- O6, N1

const C_L: f64 = 2.0;
const C_H: f64 = 0.1;
const C_P: f64 = 1000.0;
const E_C: [f64; 3] = [1.4e11, 1.4e10, 1.4e10];
const NU_C: [f64; 3] = [0.3, 0.3, 0.4];
const G_C: [f64; 3] = [7.0e9, 7.0e9, 5.0e9];

/// The Hex20 cantilever of ADR 0385 D1 (support of beam theory) with an orthotropic material: `delta / delta_T`.
fn cantilever_ratio(r: Axes, tag: &str) -> f64 {
    let base = hex_box([0.0; 3], [C_L, C_H, C_H], [10, 1, 1]).unwrap();
    let mut mesh = upgrade_to_hex20(&base).unwrap().0;
    let root: Vec<usize> = mesh.node_set("xmin").unwrap().to_vec();
    let mut neutral: Vec<usize> = root
        .iter()
        .copied()
        .filter(|&n| (mesh.nodes()[n][2] - 0.5 * C_H).abs() <= 1e-12)
        .collect();
    neutral.sort_by(|&a, &b| mesh.nodes()[a][1].total_cmp(&mesh.nodes()[b][1]));
    assert_eq!(neutral.len(), 2);
    let loads = traction_loads(&mesh, "xmax", [0.0, 0.0, -C_P / (C_H * C_H)]).unwrap();
    mesh.add_node_set("root_neutral", neutral.clone()).unwrap();
    mesh.add_node_set("root_neutral_y", vec![neutral[0]]).unwrap();
    let mut model = SolidModel::new(mesh);
    model
        .add_material(
            "all",
            Material::orthotropic(E_C, NU_C, G_C, RHO, [0.0; 3], fixed(r)).unwrap(),
        )
        .unwrap();
    model.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    model
        .add_constraint(Constraint::displacement("root_neutral", 2, 0.0))
        .unwrap();
    model
        .add_constraint(Constraint::displacement("root_neutral_y", 1, 0.0))
        .unwrap();
    let res = solve_static_with(&model, &[], &loads, &StaticOptions::default()).unwrap();
    let delta: f64 = loads
        .iter()
        .map(|f| f.force[2] * res.displacement(f.node)[2])
        .sum::<f64>()
        / C_P;
    let i = C_H.powi(4) / 12.0;
    let a = C_H * C_H;
    let kappa = 5.0 / 6.0;
    let delta_t = C_P * C_L.powi(3) / (3.0 * E_C[0] * i) + C_P * C_L / (kappa * G_C[1] * a);
    println!(
        "{tag}: delta {delta:.6e} m, delta_T {delta_t:.6e} m, ratio {:.6}",
        delta / delta_t
    );
    delta / delta_t
}

#[test]
fn o6_n1_orthotropic_cantilever() {
    let mut c = Checks::new();
    let r = cantilever_ratio(about_z(0.0), "O6");
    c.at_most("O6 |delta/delta_T - 1|", (r - 1.0).abs(), 0.01);
    let turned = [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
    let r90 = cantilever_ratio(turned, "N1");
    c.above("N1 axes turned 90 degrees: delta/delta_T", r90, 5.0);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- O7, O8

#[test]
fn o7_o8_free_heating_and_full_restraint() {
    let mut c = Checks::new();
    let r = rot([1.0, 2.0, 3.0], 0.5);
    let dt = 80.0;
    let s = test_compliance(E_O, NU_O, G_O);
    let cm = inverse6(&s);
    let a_dt = [ALPHA[0] * dt, ALPHA[1] * dt, ALPHA[2] * dt, 0.0, 0.0, 0.0];
    let c_a: [f64; 6] = std::array::from_fn(|i| (0..6).map(|j| cm[i][j] * a_dt[j]).sum());
    let s_scale = max_abs(c_a);
    let e_scale = max_abs(a_dt);
    // O7.
    let mut m = SolidModel::new(cube());
    m.add_material("all", ortho(fixed(r))).unwrap();
    three_two_one(&mut m);
    m.add_load(Load::temperature(vec![293.0 + dt; 8], 293.0)).unwrap();
    let res = solve_static(&m, &StaticOptions::default()).unwrap();
    let field = field_of(&m, &res);
    let (mut es, mut ss): (f64, f64) = (0.0, 0.0);
    for (eps, sg) in field.strain().iter().zip(field.stress()) {
        let em = voigt_strain(&to_material(&r, &tensor_of_strain(eps)));
        es = es.max(max_abs((0..6).map(|k| em[k] - a_dt[k])) / e_scale);
        ss = ss.max(max_abs(sg.iter().copied()) / s_scale);
    }
    c.at_most("O7 free heating: material-axes strain - alpha dT", es, 1e-10);
    c.at_most("O7 free heating: stress", ss, 1e-10);
    // O8.
    let mut m = SolidModel::new(cube());
    m.add_material("all", ortho(fixed(r))).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("every", comp, 0.0)).unwrap();
    }
    m.add_load(Load::temperature(vec![293.0 + dt; 8], 293.0)).unwrap();
    let res = solve_static(&m, &StaticOptions::default()).unwrap();
    let field = field_of(&m, &res);
    let mut e8: f64 = 0.0;
    for sg in field.stress() {
        let sm = voigt_stress(&to_material(&r, &tensor_of_stress(sg)));
        e8 = e8.max(max_abs((0..6).map(|k| sm[k] + c_a[k])) / s_scale);
    }
    c.at_most("O8 full restraint: material-axes stress + C alpha dT", e8, 1e-10);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- O9, N2, K1, R3

/// The quarter tube of ADR 0385 on `n_r x n_t x 1` Hex20 with cylindrical axes: (max bore hoop error against `s_c`, max
/// bore radial displacement error against `u_c`).
fn tube(n_r: usize, n_t: usize, e: [f64; 3], s_c: f64, u_c: f64, tag: &str) -> (f64, f64) {
    let (a, b, p) = (1.0, 2.0, 1e7);
    let param = hex_box([a, 0.0, 0.0], [b - a, PI / 2.0, 0.2], [n_r, n_t, 1]).unwrap();
    let (mesh, _) = upgrade_to_hex20_mapped(&param, |q| [q[0] * q[1].cos(), q[0] * q[1].sin(), q[2]]).unwrap();
    let bore: Vec<usize> = mesh.node_set("xmin").unwrap().to_vec();
    let loads = pressure_loads(&mesh, "xmin", p).unwrap();
    let mut model = SolidModel::new(mesh);
    let axes = MaterialAxes::cylindrical([0.0; 3], [0.0, 0.0, 1.0]).unwrap();
    model
        .add_material(
            "all",
            Material::orthotropic(e, [0.1, 0.0, 0.0], [5e9; 3], RHO, [0.0; 3], axes).unwrap(),
        )
        .unwrap();
    for (set, comp) in [("zmin", 2), ("zmax", 2), ("ymin", 1), ("ymax", 0)] {
        model.add_constraint(Constraint::displacement(set, comp, 0.0)).unwrap();
    }
    let res = solve_static_with(&model, &[], &loads, &StaticOptions::default()).unwrap();
    let field = field_of(&model, &res);
    let nodal = nodal_average(&model, &field, field.stress(), None).unwrap();
    let mesh = model.mesh();
    let (mut es, mut eu): (f64, f64) = (0.0, 0.0);
    for &n in &bore {
        let x = mesh.nodes()[n];
        let rr = (x[0] * x[0] + x[1] * x[1]).sqrt();
        let (co, s) = (x[0] / rr, x[1] / rr);
        let u = res.displacement(n);
        eu = eu.max(((u[0] * co + u[1] * s) / u_c - 1.0).abs());
        let sg = nodal[n].unwrap();
        let hoop = s * s * sg[0] + co * co * sg[1] - 2.0 * s * co * sg[3];
        es = es.max((hoop / (s_c * p) - 1.0).abs());
    }
    println!(
        "{tag}: {n_r} x {n_t}, {} nodes, {} bore nodes, hoop {s_c:.6} p, u_r(a) {u_c:.6e}",
        mesh.nodes().len(),
        bore.len()
    );
    (es, eu)
}

#[test]
fn o9_n2_polar_orthotropic_tube() {
    let mut c = Checks::new();
    let (a, b, p) = (1.0f64, 2.0f64, 1e7);
    let e: [f64; 3] = [1e10, 4e10, 1e10];
    let k = (e[1] / e[0]).sqrt();
    let s_c = k * (a.powf(2.0 * k) + b.powf(2.0 * k)) / (b.powf(2.0 * k) - a.powf(2.0 * k));
    c.at_most("O9 closed form 34/15", (s_c - 34.0 / 15.0).abs(), 1e-15);
    let u_c = a * (0.1 * p / e[0] + s_c * p / e[1]);
    let (es, eu) = tube(16, 8, e, s_c, u_c, "O9");
    c.at_most("O9 bore hoop stress (16 x 8)", es, 0.01);
    c.at_most("O9b bore radial displacement (16 x 8)", eu, 2e-3);
    // N2: k = 1 gives Lame.
    let lame = (a * a + b * b) / (b * b - a * a);
    let e1 = [1e10; 3];
    let u1 = a * (0.1 * p / e1[0] + lame * p / e1[1]);
    let (es1, _) = tube(16, 8, e1, lame, u1, "N2");
    c.at_most("N2 k = 1 against Lame", es1, 0.01);
    c.above("N2 Lame against the k = 2 value", (lame / s_c - 1.0).abs(), 0.2);
    c.finish();
}

#[test]
fn k1_r3_cylindrical_axes() {
    let mut c = Checks::new();
    let t = 0.7f64;
    let got = cylindrical_axes([0.0; 3], [0.0, 0.0, 2.0], [3.0 * t.cos(), 3.0 * t.sin(), 5.0]).unwrap();
    let want = [[t.cos(), t.sin(), 0.0], [-t.sin(), t.cos(), 0.0], [0.0, 0.0, 1.0]];
    c.at_most(
        "K1 cylindrical axes",
        max_abs(got.iter().flatten().zip(want.iter().flatten()).map(|(x, y)| x - y)),
        1e-15,
    );
    let on = cylindrical_axes([0.0; 3], [0.0, 0.0, 2.0], [0.0, 0.0, 4.0]);
    c.holds(
        "K1 point on the axis refused",
        matches!(on, Err(FemError::InvalidValue { key: "axes", .. })),
        "",
    );
    let cyl = ortho(MaterialAxes::cylindrical([0.0; 3], [0.0, 0.0, 1.0]).unwrap());
    let r3 = cyl.elasticity_at(&PointContext::default());
    c.holds(
        "R3 cylindrical axes without the point refused (axes)",
        matches!(&r3, Err(FemError::InvalidValue { key: "axes", .. })),
        &format!("{:?}", r3.as_ref().err()),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- R1, R2

fn cantilever_with(mat: Material, temperature: bool) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [20, 2, 2]).unwrap();
    let t: Vec<f64> = mesh.nodes().iter().map(|x| 293.0 + 100.0 * x[0]).collect();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("xmax", [0.0, 0.0, -100.0])).unwrap();
    if temperature {
        m.add_load(Load::temperature(t, 293.0)).unwrap();
    }
    m
}

fn refusal(r: &rcfd_fem::Result<()>, variant: &str) -> bool {
    matches!(r, Err(FemError::NotYetSupported { item: "material", variant: v, location: l, package: p })
        if *v == variant && l == "material 0, element set 'all'" && *p == FOLLOW_UP)
}

#[test]
#[ignore = "record of ADR 0452: ADR 0457 (F-537) opened modal_analysis_with, prestressed_modes, solve_static_mpc and modal_analysis_mpc to Orthotropic and Tabulated, retiring rows E3, E8, E9, E10; continued in r1_r2_paths_after_f537 of sa_ortho_modal.rs (ADR 0291 pattern, maintainer standing decision)"]
fn r1_r2_paths() {
    let mut c = Checks::new();
    let steel = Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap();
    let twin = cantilever_with(steel.clone(), false);
    let prestress = Prestress::uniform(&twin, [-1e6, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let table = Material::tabulated(steel, vec![200.0, 600.0], vec![("e".to_string(), vec![2.0e11, 1.8e11])]).unwrap();
    for (mat, name) in [
        (ortho(fixed(rot([0.0, 0.0, 1.0], 0.5))), "Orthotropic"),
        (table, "Tabulated"),
    ] {
        let m = cantilever_with(mat, true);
        let cg = StaticOptions::with_solver(StaticSolver::PreconditionedCg {
            preconditioner: CgPreconditioner::BlockJacobi,
            tolerance: 1e-10,
            max_iterations: 20000,
        });
        let solved: Vec<(&str, rcfd_fem::Result<()>)> = vec![
            (
                "E1 solve_static",
                solve_static(&m, &StaticOptions::default()).map(|_| ()),
            ),
            (
                "E2 solve_static_with CG",
                solve_static_with(&m, &[], &[], &cg).map(|_| ()),
            ),
        ];
        for (entry, r) in solved {
            c.holds(
                &format!("R1 {name} {entry} solves"),
                r.is_ok(),
                &format!("{:?}", r.err()),
            );
        }
        let refused: Vec<(&str, rcfd_fem::Result<()>)> = vec![
            (
                "E3 modal_analysis_with",
                modal_analysis_with(&m, &[], &ModalOptions::new(2)).map(|_| ()),
            ),
            ("E4 stiffness", stiffness(&m).map(|_| ())),
            (
                "E5 nonlinear::solve",
                solve(&m, &[], &[], &NewtonOptions::new(1)).map(|_| ()),
            ),
            (
                "E6 Integrator::new",
                Integrator::new(&m, &[], &TransientOptions::new(1e-4)).map(|_| ()),
            ),
            (
                "E7 buckling::factors",
                factors(&m, &prestress, &BucklingOptions::new(1)).map(|_| ()),
            ),
            (
                "E8 prestressed_modes",
                prestressed_modes(&m, &prestress, &ModalOptions::new(2)).map(|_| ()),
            ),
            (
                "E9 solve_static_mpc",
                solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ()),
            ),
            (
                "E10 modal_analysis_mpc",
                modal_analysis_mpc(&m, &[], &ModalOptions::new(2)).map(|_| ()),
            ),
        ];
        for (entry, r) in refused {
            c.holds(
                &format!("R1 {name} {entry} refused"),
                refusal(&r, name),
                &format!("{:?}", r.err()),
            );
        }
    }
    // R2: a Shell4 plate with the orthotropic material.
    let n = 4;
    let idx = |i: usize, j: usize| j * (n + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            nodes.push([i as f64 / n as f64, j as f64 / n as f64, 0.0]);
        }
    }
    let mut conn = Vec::new();
    for j in 0..n {
        for i in 0..n {
            conn.extend([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let mut pres = Vec::new();
    for (k, x) in nodes.iter().enumerate() {
        if x[0] == 0.0 {
            for comp in 0..6 {
                pres.push(Prescribed::new(k, comp, 0.0));
            }
        }
    }
    let nn = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..n * n).collect()).unwrap();
    mesh.add_node_set("all", (0..nn).collect()).unwrap();
    let mut s = SolidModel::new(mesh);
    s.add_material("all", ortho(fixed(about_z(0.0)))).unwrap();
    s.add_load(Load::shell_pressure("all", 1000.0)).unwrap();
    let r2 = solve_static_with(&s, &pres, &[], &StaticOptions::default());
    let ok = matches!(&r2, Err(FemError::NotYetSupported { item: "material", variant: "Orthotropic", location, package })
        if location.starts_with("element 0") && location.contains("Shell4") && *package == FOLLOW_UP);
    c.holds(
        "R2 Shell4 with Orthotropic refused",
        ok,
        &format!("{:?}", r2.as_ref().err()),
    );
    c.finish();
}
