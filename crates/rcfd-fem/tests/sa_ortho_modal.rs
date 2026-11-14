//! F-537 (ADR 0457): the orthotropic and the tabulated material of ADR 0452 on the modal analysis, the pre-stressed modes
//! and the transformation path. Checks C1 to C2tp, Q1, Q1t, Z1, Z1t, P1, P1t, T1, T3, R1 (the copy of ADR 0452's R1 and
//! R2), R3 and the negative controls N1 to N3 of the registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::damped::{damped_modes, DampedOptions};
use rcfd_fem::geometric::{geometric_stiffness, prestressed_modes, Prestress};
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::mass::consistent;
use rcfd_fem::mat_ortho::FOLLOW_UP;
use rcfd_fem::mat_table::interpolate;
use rcfd_fem::material::MaterialAxes;
use rcfd_fem::modal::{modal_analysis, modal_analysis_with, ModalOptions, ModalResult};
use rcfd_fem::model::{ElasticSupport, LinearTerm};
use rcfd_fem::mpc::{modal_analysis_mpc, solve_static_mpc};
use rcfd_fem::nonlinear::{solve, NewtonOptions};
use rcfd_fem::rotating::{RotatingOptions, RotatingPart};
use rcfd_fem::solve::{solve_static, solve_static_with, CgPreconditioner, StaticOptions, StaticSolver};
use rcfd_fem::transient::{Integrator, TransientOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;

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
    fn below(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value < limit;
        println!(
            "{name}: {value:.4e} (expected < {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{name} = {value:e} >= {limit:e}"));
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "missed: {:?}", self.failed);
    }
}

// ------------------------------------------------------------------------------------------------------- helpers

type Axes = [[f64; 3]; 3];

const I3: Axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
const E_ISO: f64 = 2.0e11;
const NU_ISO: f64 = 0.3;
const RHO_ISO: f64 = 7850.0;
const E_O: [f64; 3] = [1.4e11, 1.0e10, 1.0e10];
const NU_O: [f64; 3] = [0.3, 0.3, 0.45];
const G_O: [f64; 3] = [5.0e9, 5.0e9, 3.5e9];
const RHO_O: f64 = 1600.0;
const TABLE_T: [f64; 2] = [200.0, 600.0];
const TABLE_E: [f64; 2] = [2.0e11, 1.6e11];
const T_REF: f64 = 293.0;
const S_XX: f64 = 2.0e8;

/// Rodrigues rotation matrix (rows orthonormal, right-handed): the `rot` of `sa_ortho.rs`.
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

fn fixed(r: Axes) -> MaterialAxes {
    MaterialAxes::fixed(r).unwrap()
}

fn iso_e(e: f64) -> Material {
    Material::isotropic_elastic(e, NU_ISO, RHO_ISO, 0.0).unwrap()
}

/// `ISO_O`: isotropic constants in an orthotropic material, `G` times `g_factor` (1 for C2, `1 + 1e-6` for N3).
fn iso_o(r: Axes, g_factor: f64) -> Material {
    let g = E_ISO / (2.0 * (1.0 + NU_ISO));
    Material::orthotropic(
        [E_ISO; 3],
        [NU_ISO; 3],
        [g * g_factor, g, g],
        RHO_ISO,
        [0.0; 3],
        fixed(r),
    )
    .unwrap()
}

fn ortho(axes: MaterialAxes) -> Material {
    Material::orthotropic(E_O, NU_O, G_O, RHO_O, [0.0; 3], axes).unwrap()
}

fn cyl() -> Material {
    ortho(MaterialAxes::cylindrical([0.5, -1.0, 0.0], [0.0, 0.0, 1.0]).unwrap())
}

fn tab() -> Material {
    Material::tabulated(
        iso_e(E_ISO),
        TABLE_T.to_vec(),
        vec![("e".to_string(), TABLE_E.to_vec())],
    )
    .unwrap()
}

#[derive(Clone, Copy)]
enum Field {
    None,
    Linear,
    Uniform,
}

fn add_field(m: &mut SolidModel, field: Field) {
    let t: Vec<f64> = match field {
        Field::None => return,
        Field::Linear => m.mesh().nodes().iter().map(|x| T_REF + 250.0 * x[0]).collect(),
        Field::Uniform => vec![443.0; m.mesh().nodes().len()],
    };
    m.add_load(Load::temperature(t, T_REF)).unwrap();
}

fn opts(n: usize) -> ModalOptions {
    ModalOptions::new(n)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-12)
}

fn max_rel(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(x, y)| (x / y - 1.0).abs()).fold(0.0, f64::max)
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// The block of D7: Hex20 4 x 2 x 1 on `1 x 0.4 x 0.25`, every component of `xmin` held.
fn block(mat: Material, field: Field) -> SolidModel {
    let base = hex_box([0.0; 3], [1.0, 0.4, 0.25], [4, 2, 1]).unwrap();
    let mesh = upgrade_to_hex20(&base).unwrap().0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    add_field(&mut m, field);
    m
}

/// The transformation block of D7: Hex8 5 x 2 x 1 on `1 x 0.4 x 0.25`; `u_x = u_y = 0` on `xmin`; `u_z = 0` at the root by
/// one-term linear constraints (`linear`) or as a displacement constraint; springs of 5e7 N/m in z at the `xmax` nodes
/// (`springs`).
fn tr_block(mat: Material, field: Field, linear: bool, springs: bool) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [1.0, 0.4, 0.25], [5, 2, 1]).unwrap();
    let root: Vec<usize> = mesh.node_set("xmin").unwrap().to_vec();
    let tip: Vec<usize> = mesh.node_set("xmax").unwrap().to_vec();
    mesh.add_node_set("tip", tip).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 1, 0.0)).unwrap();
    if linear {
        for &n in &root {
            m.add_constraint(Constraint::linear(vec![LinearTerm::new(n, 2, 1.0)], 0.0))
                .unwrap();
        }
    } else {
        m.add_constraint(Constraint::displacement("xmin", 2, 0.0)).unwrap();
    }
    if springs {
        m.add_support(ElasticSupport::nodal_spring("tip", 2, 5.0e7)).unwrap();
    }
    add_field(&mut m, field);
    m
}

fn uniform(m: &SolidModel, s: f64) -> Prestress {
    Prestress::uniform(m, [s, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap()
}

// ------------------------------------------------------------------------------------------------- C1, C1b, C1c, N1

const C_H: f64 = 0.1;
const E_C: [f64; 3] = [1.4e11, 1.4e10, 1.4e10];
const NU_C: [f64; 3] = [0.3, 0.3, 0.4];
const G_C: [f64; 3] = [7.0e9, 7.0e9, 5.0e9];
const RHO_C: f64 = 1600.0;
const BETA1_L: f64 = 1.875_104_068_711_961_1;

/// The cantilever of D7: Hex20 `n` x 1 x 1 on `L x h x h`, the symmetric support of beam theory; four modes.
fn cantilever_modes(l: f64, n: usize, r: Axes) -> ModalResult {
    let base = hex_box([0.0; 3], [l, C_H, C_H], [n, 1, 1]).unwrap();
    let mut mesh = upgrade_to_hex20(&base).unwrap().0;
    let root: Vec<usize> = mesh.node_set("xmin").unwrap().to_vec();
    let on = |mesh: &SolidMesh, k: usize, c: usize| (mesh.nodes()[k][c] - 0.5 * C_H).abs() <= 1e-12;
    let zs: Vec<usize> = root.iter().copied().filter(|&k| on(&mesh, k, 2)).collect();
    let ys: Vec<usize> = root.iter().copied().filter(|&k| on(&mesh, k, 1)).collect();
    assert_eq!((zs.len(), ys.len()), (2, 2));
    mesh.add_node_set("root_z", zs).unwrap();
    mesh.add_node_set("root_y", ys).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material(
        "all",
        Material::orthotropic(E_C, NU_C, G_C, RHO_C, [0.0; 3], fixed(r)).unwrap(),
    )
    .unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("root_z", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("root_y", 1, 0.0)).unwrap();
    modal_analysis(&m, &opts(4)).unwrap()
}

fn f_eb(l: f64) -> f64 {
    let i = C_H.powi(4) / 12.0;
    let a = C_H * C_H;
    BETA1_L * BETA1_L * (E_C[0] * i / (RHO_C * a * l.powi(4))).sqrt() / (2.0 * std::f64::consts::PI)
}

#[test]
fn c1_n1_orthotropic_cantilever_frequencies() {
    let mut c = Checks::new();
    let r = cantilever_modes(4.0, 20, I3);
    let (eb, timo) = (f_eb(4.0), 9.415729);
    println!("C1 L/h = 40: frequencies {:?} Hz, f_EB {eb:.6} Hz", r.frequencies);
    for k in 0..2 {
        c.at_most(
            &format!("C1 mode {} |f/f_EB - 1|", k + 1),
            (r.frequencies[k] / eb - 1.0).abs(),
            0.01,
        );
        c.at_most(
            &format!("C1b mode {} |f/f_Timoshenko - 1|", k + 1),
            (r.frequencies[k] / timo - 1.0).abs(),
            3e-3,
        );
    }
    let r20 = cantilever_modes(2.0, 10, I3);
    let (eb20, timo20) = (f_eb(2.0), 37.327353);
    println!("C1c L/h = 20: frequencies {:?} Hz, f_EB {eb20:.6} Hz", r20.frequencies);
    for k in 0..2 {
        c.at_most(
            &format!("C1c mode {} |f/f_Timoshenko - 1|", k + 1),
            (r20.frequencies[k] / timo20 - 1.0).abs(),
            5e-3,
        );
        println!(
            "C1c mode {} against f_EB (recorded, no limit): {:+.4e}",
            k + 1,
            r20.frequencies[k] / eb20 - 1.0
        );
    }
    let turned = [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
    let r90 = cantilever_modes(4.0, 20, turned);
    c.below("N1 axes turned 90 degrees: f_1/f_EB", r90.frequencies[0] / eb, 0.5);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- C2, C2p, C2t, C2tp, N3

#[test]
fn c2_n3_isotropic_constants() {
    let mut c = Checks::new();
    let iso = iso_e(E_ISO);
    let o = opts(6);
    let b_iso = block(iso.clone(), Field::None);
    let f_iso = modal_analysis(&b_iso, &o).unwrap().frequencies;
    let p_iso = prestressed_modes(&b_iso, &uniform(&b_iso, S_XX), &o)
        .unwrap()
        .frequencies;
    let t_iso = tr_block(iso.clone(), Field::None, true, true);
    let ft_iso = modal_analysis(&t_iso, &o).unwrap().frequencies;
    let pt_iso = prestressed_modes(&t_iso, &uniform(&t_iso, S_XX), &o)
        .unwrap()
        .frequencies;
    for (r, tag) in [(I3, "axes I"), (rot([1.0, 2.0, 3.0], 0.5), "axes rot((1, 2, 3), 0.5)")] {
        let b = block(iso_o(r, 1.0), Field::None);
        c.at_most(
            &format!("C2 modal, {tag}"),
            max_rel(&modal_analysis(&b, &o).unwrap().frequencies, &f_iso),
            1e-12,
        );
        c.at_most(
            &format!("C2p pre-stressed, {tag}"),
            max_rel(
                &prestressed_modes(&b, &uniform(&b, S_XX), &o).unwrap().frequencies,
                &p_iso,
            ),
            1e-12,
        );
        let t = tr_block(iso_o(r, 1.0), Field::None, true, true);
        c.at_most(
            &format!("C2t transformation modal, {tag}"),
            max_rel(&modal_analysis(&t, &o).unwrap().frequencies, &ft_iso),
            1e-12,
        );
        c.at_most(
            &format!("C2tp transformation pre-stressed, {tag}"),
            max_rel(
                &prestressed_modes(&t, &uniform(&t, S_XX), &o).unwrap().frequencies,
                &pt_iso,
            ),
            1e-12,
        );
    }
    let n3 = block(iso_o(I3, 1.0 + 1e-6), Field::None);
    c.above(
        "N3 G_12 times (1 + 1e-6)",
        max_rel(&modal_analysis(&n3, &o).unwrap().frequencies, &f_iso),
        1e-9,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- Q1, Q1t, Z1, Z1t

/// D8: `max_k |lambda'_k / lambda_k - 1|` over the first three modes, `lambda' = phi^T M phi / phi^T M K^-1 M phi` with the
/// static solve of the same model.
fn identity(m: &SolidModel, r: &ModalResult) -> f64 {
    let mass = consistent(m).unwrap();
    let dofs = r.dofs();
    let mut worst: f64 = 0.0;
    for k in 0..3 {
        let phi = &r.modes[k];
        let mut mphi = vec![0.0; phi.len()];
        mass.matvec(phi, &mut mphi);
        let mut forces = Vec::new();
        for node in 0..m.mesh().nodes().len() {
            let mut f = [0.0; 3];
            for (comp, fc) in f.iter_mut().enumerate() {
                *fc = mphi[dofs.equation(node, comp).unwrap()];
            }
            forces.push(NodalForceAt::new(node, f));
        }
        let u = solve_static_with(m, &[], &forces, &StaticOptions::default()).unwrap();
        let num: f64 = phi.iter().zip(&mphi).map(|(a, b)| a * b).sum();
        let den: f64 = u.displacement_values().iter().zip(&mphi).map(|(a, b)| a * b).sum();
        let lambda = num / den;
        worst = worst.max((lambda / r.eigenvalues[k] - 1.0).abs());
    }
    worst
}

#[test]
fn q1_z1_per_point_stiffness() {
    let mut c = Checks::new();
    let o = opts(6);
    for (mat, field, tag) in [(tab(), Field::Linear, "TAB"), (cyl(), Field::None, "CYL")] {
        let b = block(mat.clone(), field);
        let r = modal_analysis(&b, &o).unwrap();
        println!("Q1 {tag}: frequencies {:?}", r.frequencies);
        c.at_most(&format!("Q1 {tag} block"), identity(&b, &r), 1e-9);
        let z = prestressed_modes(&b, &uniform(&b, 0.0), &o).unwrap();
        c.holds(
            &format!("Z1 {tag} zero pre-stress has the bits of the modal analysis"),
            bits(&z.eigenvalues) == bits(&r.eigenvalues) && bits(&z.modes.concat()) == bits(&r.modes.concat()),
            &format!("{:?} / {:?}", z.eigenvalues, r.eigenvalues),
        );
        let t = tr_block(mat, field, true, true);
        let rt = modal_analysis(&t, &o).unwrap();
        println!("Q1t {tag}: frequencies {:?}", rt.frequencies);
        c.at_most(&format!("Q1t {tag} transformation block"), identity(&t, &rt), 1e-9);
        let zt = prestressed_modes(&t, &uniform(&t, 0.0), &o).unwrap();
        c.at_most(
            &format!("Z1t {tag} zero pre-stress against the modal analysis"),
            max_rel(&zt.eigenvalues, &rt.eigenvalues),
            1e-12,
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- P1, P1t, N2

#[test]
fn p1_n2_tabulated_temperature() {
    let mut c = Checks::new();
    let o = opts(6);
    let e443 = interpolate(&TABLE_T, &TABLE_E, 443.0).unwrap();
    let e293 = interpolate(&TABLE_T, &TABLE_E, T_REF).unwrap();
    println!("P1: E(443) = {e443:e}, E(293) = {e293:e}");
    let b = block(tab(), Field::Uniform);
    let bi = block(iso_e(e443), Field::None);
    let f = modal_analysis(&b, &o).unwrap().frequencies;
    let fi = modal_analysis(&bi, &o).unwrap().frequencies;
    c.at_most("P1 modal against ISO at E(443)", max_rel(&f, &fi), 1e-12);
    let p = prestressed_modes(&b, &uniform(&b, S_XX), &o).unwrap().frequencies;
    let pi = prestressed_modes(&bi, &uniform(&bi, S_XX), &o).unwrap().frequencies;
    c.at_most("P1 pre-stressed against ISO at E(443)", max_rel(&p, &pi), 1e-12);
    let t = tr_block(tab(), Field::Uniform, true, true);
    let ti = tr_block(iso_e(e443), Field::None, true, true);
    let pt = prestressed_modes(&t, &uniform(&t, S_XX), &o).unwrap().frequencies;
    let pti = prestressed_modes(&ti, &uniform(&ti, S_XX), &o).unwrap().frequencies;
    c.at_most(
        "P1t transformation pre-stressed against ISO at E(443)",
        max_rel(&pt, &pti),
        1e-12,
    );
    let b293 = block(iso_e(e293), Field::None);
    let p293 = prestressed_modes(&b293, &uniform(&b293, S_XX), &o).unwrap().frequencies;
    c.above("N2 against ISO at E(T_ref)", max_rel(&p, &p293), 1e-3);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- T1, T3

#[test]
fn t1_t3_transformation_against_elimination() {
    let mut c = Checks::new();
    let o = opts(6);
    let mat = ortho(fixed(rot([1.0, 2.0, 3.0], 0.5)));
    let lin = tr_block(mat.clone(), Field::None, true, false);
    let eli = tr_block(mat, Field::None, false, false);
    let fl = modal_analysis(&lin, &o).unwrap().frequencies;
    let fe = modal_analysis(&eli, &o).unwrap().frequencies;
    println!("T1: frequencies {fl:?}");
    c.at_most(
        "T1 modal, linear constraints against elimination",
        max_rel(&fl, &fe),
        1e-10,
    );
    let tip = lin.mesh().node_set("tip").unwrap().to_vec();
    let forces: Vec<NodalForceAt> = tip.iter().map(|&n| NodalForceAt::new(n, [0.0, 0.0, -1.0e4])).collect();
    let sl = solve_static_mpc(&lin, &[], &forces, &[], &StaticOptions::default()).unwrap();
    let se = solve_static_with(&eli, &[], &forces, &StaticOptions::default()).unwrap();
    let (a, b) = (sl.result.displacement_values(), se.displacement_values());
    let dev =
        a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max) / b.iter().map(|y| y.abs()).fold(0.0, f64::max);
    c.at_most("T3 static, linear constraints against elimination", dev, 1e-10);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- R1 (copy), R3

/// The cantilever of `sa_ortho.rs` (ADR 0452 R1), copied.
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

fn shell_plate(mat: Material) -> (SolidModel, Vec<Prescribed>) {
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
    s.add_material("all", mat).unwrap();
    (s, pres)
}

/// The copy of `sa_ortho::r1_r2_paths` after ADR 0457 (D6): E3, E8, E9, E10 solve.
#[test]
#[ignore = "record of ADR 0457: ADR 0459 (F-542) opened Integrator::new and buckling::factors to Orthotropic and Tabulated, retiring rows E6, E7; continued in r1_r2_paths_after_f542 of sa_ortho_rotating.rs (ADR 0291 pattern, maintainer standing decision)"]
fn r1_r2_paths_after_f537() {
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
            (
                "E3 modal_analysis_with",
                modal_analysis_with(&m, &[], &ModalOptions::new(2)).map(|_| ()),
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
        for (entry, r) in solved {
            c.holds(
                &format!("R1 {name} {entry} solves"),
                r.is_ok(),
                &format!("{:?}", r.err()),
            );
        }
        let refused: Vec<(&str, rcfd_fem::Result<()>)> = vec![
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
    let (mut s, pres) = shell_plate(ortho(fixed(I3)));
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

#[test]
#[ignore = "record of ADR 0457: ADR 0459 (F-542) opened buckling::factors, Integrator::new and RotatingPart::new to Orthotropic and Tabulated, retiring their rows; continued in r3_paths_not_opened_after_f542 of sa_ortho_rotating.rs (ADR 0291 pattern, maintainer standing decision)"]
fn r3_paths_not_opened() {
    let mut c = Checks::new();
    let steel = Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap();
    let twin = cantilever_with(steel.clone(), false);
    let prestress = Prestress::uniform(&twin, [-1e6, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let table = Material::tabulated(steel, vec![200.0, 600.0], vec![("e".to_string(), vec![2.0e11, 1.8e11])]).unwrap();
    for (mat, name) in [(ortho(fixed(I3)), "Orthotropic"), (table, "Tabulated")] {
        let m = cantilever_with(mat, true);
        let refused: Vec<(&str, rcfd_fem::Result<()>)> = vec![
            ("stiffness", stiffness(&m).map(|_| ())),
            (
                "geometric_stiffness",
                geometric_stiffness(&m, &prestress, None).map(|_| ()),
            ),
            (
                "buckling::factors",
                factors(&m, &prestress, &BucklingOptions::new(1)).map(|_| ()),
            ),
            (
                "nonlinear::solve",
                solve(&m, &[], &[], &NewtonOptions::new(1)).map(|_| ()),
            ),
            (
                "Integrator::new",
                Integrator::new(&m, &[], &TransientOptions::new(1e-4)).map(|_| ()),
            ),
            ("damped_modes", damped_modes(&m, &DampedOptions::new(2)).map(|_| ())),
            (
                "RotatingPart::new",
                RotatingPart::new(&m, [1.0, 0.0, 0.0], [0.0; 3], &RotatingOptions::new(2)).map(|_| ()),
            ),
        ];
        for (entry, r) in refused {
            c.holds(
                &format!("R3 {name} {entry} refused"),
                refusal(&r, name),
                &format!("{:?}", r.err()),
            );
        }
    }
    let (s, pres) = shell_plate(ortho(fixed(I3)));
    let rs = modal_analysis_with(&s, &pres, &ModalOptions::new(2));
    let ok = matches!(&rs, Err(FemError::NotYetSupported { item: "material", variant: "Orthotropic", location, package })
        if location.starts_with("element 0") && location.contains("Shell4") && *package == FOLLOW_UP);
    c.holds(
        "R3 Shell4 modal with Orthotropic refused",
        ok,
        &format!("{:?}", rs.as_ref().err()),
    );
    let mut cy = tr_block(ortho(fixed(I3)), Field::None, false, false);
    cy.add_constraint(Constraint::cyclic("ymin", "ymax", [0.0, 0.0, 1.0], [0.0; 3], 4))
        .unwrap();
    let j = cy.constraints().len() - 1;
    let rc = modal_analysis_with(&cy, &[], &ModalOptions::new(2));
    let want = format!("material 0, element set 'all', with the cyclic constraint {j}");
    let ok = matches!(&rc, Err(FemError::NotYetSupported { item: "material", variant: "Orthotropic", location, package })
        if *location == want && *package == FOLLOW_UP);
    c.holds(
        "R3 cyclic constraint with Orthotropic refused",
        ok,
        &format!("{:?}", rc.as_ref().err()),
    );
    let nt = block(tab(), Field::None);
    let rt = modal_analysis(&nt, &ModalOptions::new(2));
    c.holds(
        "R3 Tabulated without a temperature field refused (temperature)",
        matches!(&rt, Err(FemError::InvalidValue { key: "temperature", .. })),
        &format!("{:?}", rt.as_ref().err()),
    );
    c.finish();
}
