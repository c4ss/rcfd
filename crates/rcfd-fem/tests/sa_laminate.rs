//! F-428 (ADR 0496): the laminated Shell4. Checks BT1 to BT5, L1 to L4, PL1, PL2, ST1 to ST3, CP1, CP2, TW1, TW2, FC1, RF1 to
//! RF4, MS1, RX2 and the negative controls N1 to N6 of the registration (B0 is `sa_laminate_default_bits.rs`; RX1, X1 and N2b
//! are the copies in `sa_scaffold.rs` and `si_shell_thermal.rs`, ADR 0496 D10).

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::geometric_struct::prestress_from_static;
use rcfd_fem::laminate::{
    self, check_plies, failure_index, layup_response, ply_constants, section_stiffness, Criterion, PlyStrengths,
};
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::mass::{consistent, mass_properties};
use rcfd_fem::material::{HyperelasticEnergy, MaterialAxes, PronyTerm};
use rcfd_fem::modal::{modal_analysis_with, ModalOptions};
use rcfd_fem::nonlinear::{solve, NewtonOptions};
use rcfd_fem::shell::{self, homogeneous_section, ShearStiffness};
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, Load, Material, Ply, SolidMesh, SolidModel};
use std::f64::consts::PI;

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    /// `value <= limit` with `value` finite (ADR 0496 b6).
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

/// The largest of finite values; NaN if any is not finite (ADR 0496 b6).
fn max_finite(v: impl IntoIterator<Item = f64>) -> f64 {
    let mut m = 0.0f64;
    for x in v {
        if !x.is_finite() {
            return f64::NAN;
        }
        m = m.max(x);
    }
    m
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

// ------------------------------------------------------------------------------------------------ data (arithmetic of D0)

const E1: f64 = 10.0e9;
const E2: f64 = 4.0e9;
const NU12: f64 = 0.35;
const G12: f64 = 1.6e9;
const A1: f64 = 2.0e-5;
const A2: f64 = 6.0e-5;
const RHO: f64 = 1400.0;

fn ortho_with(density: f64, alpha: [f64; 2]) -> Material {
    let axes = MaterialAxes::fixed([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]).unwrap();
    Material::orthotropic(
        [E1, E2, 4.0e9],
        [NU12, 0.35, 0.45],
        [G12, 1.6e9, 1.4e9],
        density,
        [alpha[0], alpha[1], alpha[1]],
        axes,
    )
    .unwrap()
}

fn ortho() -> Material {
    ortho_with(RHO, [A1, A2])
}

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 1.2e-5).unwrap()
}

fn aluminium() -> Material {
    Material::isotropic_elastic(7.0e10, 0.33, 2700.0, 2.3e-5).unwrap()
}

/// `Q` of the registered ply from its constants, in the test (ADR 0496 D1).
fn q_hand() -> (f64, f64, f64, f64) {
    let nu21 = NU12 * E2 / E1;
    let d = 1.0 - NU12 * nu21;
    (E1 / d, NU12 * E2 / d, E2 / d, G12)
}

fn rad(deg: f64) -> f64 {
    deg * PI / 180.0
}

/// Plies of the material at the angles (degrees), each of thickness `t / n`.
fn plies_of(material: &Material, angles: &[f64], t: f64) -> Vec<Ply> {
    let tk = t / angles.len() as f64;
    angles
        .iter()
        .map(|&a| Ply::new(material.clone(), tk, rad(a)).unwrap())
        .collect()
}

/// A grid of `nx x ny` elements on `[0, a] x [0, b]` in the plane `z = 0`, element nodes counter-clockwise.
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

/// A Shell4 model of the nodes and elements with a block property and a block material.
fn shell_model(nodes: Vec<Vec3>, el: &[[usize; 4]], property: BlockProperty, material: Material) -> SolidModel {
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, el.iter().flatten().copied().collect())
        .unwrap();
    mesh.set_block_property(0, property).unwrap();
    mesh.add_element_set("all", (0..el.len()).collect()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("origin", vec![0]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m
}

/// Whether `r` is refused `NotYetSupported` naming `package`, directly or inside `FemError::Element`.
fn refused_with<T>(r: &rcfd_fem::Result<T>, package: &str) -> bool {
    match r {
        Err(FemError::NotYetSupported { package: p, .. }) => *p == package,
        Err(FemError::Element { source, .. }) => {
            matches!(source.as_ref(), FemError::NotYetSupported { package: p, .. } if *p == package)
        }
        _ => false,
    }
}

// ------------------------------------------------------------------------------------------------ BT1 to BT5

/// The warped 3 x 3 plate of `sa_laminate_default_bits.rs` with a property and material, clamped at `x = 0`; `loads` adds a
/// shell pressure, a nodal moment on `x = 1` and a nodal temperature field.
fn warped_plate(property: BlockProperty, material: Material, loads: bool) -> SolidModel {
    let mut nodes = Vec::new();
    for j in 0..=3 {
        for i in 0..=3 {
            nodes.push([i as f64 / 3.0, j as f64 / 3.0, 0.02 * (i * j) as f64]);
        }
    }
    let idx = |i: usize, j: usize| j * 4 + i;
    let mut el = Vec::new();
    for j in 0..3 {
        for i in 0..3 {
            el.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let temps: Vec<f64> = nodes.iter().map(|x| 300.0 + 40.0 * x[0] - 25.0 * x[1] * x[1]).collect();
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, el.iter().flatten().copied().collect())
        .unwrap();
    mesh.set_block_property(0, property).unwrap();
    mesh.add_element_set("all", (0..el.len()).collect()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", (0..=3).map(|j| idx(0, j)).collect()).unwrap();
    mesh.add_node_set("x1", (0..=3).map(|j| idx(3, j)).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    if loads {
        m.add_load(Load::shell_pressure("all", 1.0e3)).unwrap();
        m.add_load(Load::nodal_moment("x1", [0.3, -2.0, 0.1])).unwrap();
        m.add_load(Load::temperature(temps, 290.0)).unwrap();
    }
    m
}

/// Everything BT2 and BT3 compare, as named bit vectors.
fn record(m: &SolidModel, free: &SolidModel) -> Vec<(&'static str, Vec<u64>)> {
    let mut out = Vec::new();
    out.push(("stiffness", bits(&stiffness(m).unwrap().val)));
    out.push(("mass", bits(&consistent(m).unwrap().val)));
    out.push(("load vector", bits(&nodal_load_vector(m, &[]).unwrap())));
    let s = solve_static(m, &StaticOptions::default()).unwrap();
    let mut v = s.displacement_values().to_vec();
    v.extend_from_slice(s.reactions().unwrap());
    out.push(("static", bits(&v)));
    let mut st = Vec::new();
    for (e, x) in shell::stresses(m, s.displacement_values()).unwrap() {
        st.push(e as f64);
        st.extend_from_slice(&x.membrane);
        st.extend_from_slice(&x.bending);
        st.extend_from_slice(&x.shear);
        st.extend_from_slice(&x.top);
        st.extend_from_slice(&x.bottom);
    }
    out.push(("stresses", bits(&st)));
    let r = modal_analysis_with(free, &[], &ModalOptions::new(4)).unwrap();
    out.push(("frequencies", bits(&r.frequencies)));
    out
}

#[test]
fn bt1_to_bt5_one_isotropic_ply() {
    let mut c = Checks::new();
    let t = 0.01;
    // BT1.
    for theta in [0.0, 0.3] {
        let s = section_stiffness(&[Ply::new(steel(), t, theta).unwrap()]).unwrap();
        c.holds(
            &format!("BT1 section of one steel ply at {theta} rad == homogeneous_section"),
            s == homogeneous_section(&steel(), t),
            "",
        );
    }
    // BT2, BT3: the Shell twin against the one-ply layup of the same material; BT4: the layup's block material changed.
    let shell_prop = BlockProperty::shell(t).unwrap();
    let layup_prop = BlockProperty::layup(vec![Ply::new(steel(), t, 0.3).unwrap()]).unwrap();
    let twin = record(
        &warped_plate(shell_prop.clone(), steel(), true),
        &warped_plate(shell_prop, steel(), false),
    );
    for (id, block_material) in [("BT2/BT3", steel()), ("BT4", aluminium())] {
        let lay = record(
            &warped_plate(layup_prop.clone(), block_material.clone(), true),
            &warped_plate(layup_prop.clone(), block_material, false),
        );
        for ((name, a), (_, b)) in twin.iter().zip(&lay) {
            c.holds(
                &format!("{id} {name}: one-ply layup == Shell twin"),
                a == b && !a.is_empty(),
                &format!("{} values", a.len()),
            );
        }
    }
    // BT5: the general sums of one isotropic ply against the homogeneous section.
    let l = laminate::laminate(&[Ply::new(steel(), t, 0.0).unwrap()]).unwrap();
    let h = homogeneous_section(&steel(), t);
    let rel = |x: &[[f64; 3]; 3], y: &[[f64; 3]; 3]| {
        let scale = max_finite(y.iter().flatten().map(|v| v.abs()));
        max_finite(x.iter().flatten().zip(y.iter().flatten()).map(|(a, b)| (a - b).abs())) / scale
    };
    c.at_most(
        "BT5 A of the general sums against the homogeneous",
        rel(&l.a, &h.membrane),
        1e-14,
    );
    c.at_most(
        "BT5 D of the general sums against the homogeneous",
        rel(&l.d, &h.bending),
        1e-14,
    );
    let hs = match h.shear {
        ShearStiffness::Isotropic(v) => v,
        _ => f64::NAN,
    };
    let herr = max_finite([
        (l.h[0][0] - hs).abs(),
        (l.h[1][1] - hs).abs(),
        l.h[0][1].abs(),
        l.h[1][0].abs(),
    ]) / hs;
    c.at_most("BT5 H against 5/6 G t I", herr, 1e-14);
    c.holds(
        "BT5 B == 0 exactly",
        l.b.iter().flatten().all(|&v| v == 0.0),
        &format!("{:?}", l.b),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------ L1 to L4, N1, N2

fn max_abs(m: &[[f64; 3]; 3]) -> f64 {
    max_finite(m.iter().flatten().map(|v| v.abs()))
}

#[test]
fn l1_to_l4_layup_stiffness_n1_n2() {
    let mut c = Checks::new();
    let t = 0.002;
    let m = ortho();
    let uneven = vec![
        Ply::new(m.clone(), 3e-4, rad(30.0)).unwrap(),
        Ply::new(m.clone(), 2e-4, rad(-45.0)).unwrap(),
        Ply::new(m.clone(), 5e-4, 0.0).unwrap(),
        Ply::new(m.clone(), 5e-4, 0.0).unwrap(),
        Ply::new(m.clone(), 2e-4, rad(-45.0)).unwrap(),
        Ply::new(m.clone(), 3e-4, rad(30.0)).unwrap(),
    ];
    let layups: Vec<(&str, Vec<Ply>)> = vec![
        ("[0/90]s", plies_of(&m, &[0.0, 90.0, 90.0, 0.0], t)),
        ("[0/60/-60]s", plies_of(&m, &[0.0, 60.0, -60.0, -60.0, 60.0, 0.0], t)),
        (
            "[0/45/-45/90]s",
            plies_of(&m, &[0.0, 45.0, -45.0, 90.0, 90.0, -45.0, 45.0, 0.0], t),
        ),
        ("[30/-45/0]s unequal", uneven.clone()),
    ];
    // L1.
    for (name, plies) in &layups {
        let l = laminate::laminate(plies).unwrap();
        c.at_most(
            &format!("L1 {name}: max|B| / (max|A| t)"),
            max_abs(&l.b) / (max_abs(&l.a) * l.thickness),
            1e-12,
        );
    }
    // N1.
    let l = laminate::laminate(&plies_of(&m, &[0.0, 90.0], t)).unwrap();
    c.above(
        "N1 [0/90]: max|B| / (max|A| t)",
        max_abs(&l.b) / (max_abs(&l.a) * l.thickness),
        1e-3,
    );
    // L2.
    let (q11, _q12, q22, _q66) = q_hand();
    let l = laminate::laminate(&layups[0].1).unwrap();
    let want = t * (q11 + q22) / 2.0;
    c.at_most(
        "L2 [0/90]s: A11 against t (Q11 + Q22)/2",
        (l.a[0][0] / want - 1.0).abs(),
        1e-12,
    );
    c.at_most(
        "L2 [0/90]s: A22 against t (Q11 + Q22)/2",
        (l.a[1][1] / want - 1.0).abs(),
        1e-12,
    );
    // L3.
    for (name, plies) in &layups[1..3] {
        let a = laminate::laminate(plies).unwrap().a;
        let s = a[0][0];
        c.at_most(
            &format!("L3 {name}: |A11 - A22| / A11"),
            (a[0][0] - a[1][1]).abs() / s,
            1e-12,
        );
        c.at_most(
            &format!("L3 {name}: |A66 - (A11 - A12)/2| / A11"),
            (a[2][2] - 0.5 * (a[0][0] - a[0][1])).abs() / s,
            1e-12,
        );
        c.at_most(&format!("L3 {name}: |A16| / A11"), a[0][2].abs() / s, 1e-12);
        c.at_most(&format!("L3 {name}: |A26| / A11"), a[1][2].abs() / s, 1e-12);
    }
    // N2.
    let a = laminate::laminate(&layups[0].1).unwrap().a;
    c.above(
        "N2 [0/90]s: |A66 - (A11 - A12)/2| / A11",
        (a[2][2] - 0.5 * (a[0][0] - a[0][1])).abs() / a[0][0],
        1e-3,
    );
    let a = laminate::laminate(&uneven).unwrap().a;
    c.above("N2 [30/-45/0]s unequal: |A16| / A11", a[0][2].abs() / a[0][0], 1e-3);
    // L4.
    let tp = 0.01;
    let (q11, q12, q22, q66) = q_hand();
    let d = laminate::laminate(&plies_of(&m, &[0.0, 90.0, 90.0, 0.0], tp))
        .unwrap()
        .d;
    let t3 = tp * tp * tp;
    for (name, got, want) in [
        ("D11", d[0][0], t3 * (7.0 * q11 + q22) / 96.0),
        ("D22", d[1][1], t3 * (q11 + 7.0 * q22) / 96.0),
        ("D12", d[0][1], q12 * t3 / 12.0),
        ("D66", d[2][2], q66 * t3 / 12.0),
    ] {
        c.at_most(
            &format!("L4 [0/90]s {name} against the hand form"),
            (got / want - 1.0).abs(),
            1e-13,
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------ PL1, PL2, N3

/// The hand `D` of the `[0/90]s` plate (arithmetic 3) and the Navier denominators: `(D11 + 2 (D12 + 2 D66) + D22, the wrong
/// D11 + 2 (D12 + D66) + D22)`.
fn plate_denominators(tp: f64) -> (f64, f64) {
    let (q11, q12, q22, q66) = q_hand();
    let t3 = tp * tp * tp;
    let (d11, d22, d12, d66) = (
        t3 * (7.0 * q11 + q22) / 96.0,
        t3 * (q11 + 7.0 * q22) / 96.0,
        q12 * t3 / 12.0,
        q66 * t3 / 12.0,
    );
    (d11 + 2.0 * (d12 + 2.0 * d66) + d22, d11 + 2.0 * (d12 + d66) + d22)
}

/// The simply supported `[0/90]s` plate of arithmetic 3: model, prescribed unknowns (as ADR 0432 arithmetic 11), centre node.
fn ss_layup_plate(tp: f64) -> (SolidModel, Vec<Prescribed>, usize, Vec<Vec3>, Vec<[usize; 4]>) {
    let n = 16;
    let (nodes, el) = grid(1.0, 1.0, n, n);
    let mut pres = Vec::new();
    for (k, x) in nodes.iter().enumerate() {
        for comp in [0, 1, 5] {
            pres.push(Prescribed::new(k, comp, 0.0));
        }
        let onx = x[0] == 0.0 || x[0] == 1.0;
        let ony = x[1] == 0.0 || x[1] == 1.0;
        if onx || ony {
            pres.push(Prescribed::new(k, 2, 0.0));
        }
        if onx {
            pres.push(Prescribed::new(k, 3, 0.0));
        }
        if ony {
            pres.push(Prescribed::new(k, 4, 0.0));
        }
    }
    let prop = BlockProperty::layup(plies_of(&ortho(), &[0.0, 90.0, 90.0, 0.0], tp)).unwrap();
    let m = shell_model(nodes.clone(), &el, prop, steel());
    (m, pres, (n / 2) * (n + 1) + n / 2, nodes, el)
}

#[test]
fn pl1_pl2_n3_simply_supported_cross_ply_plate() {
    let mut c = Checks::new();
    let tp = 0.01;
    let q0 = 1000.0;
    let (m, pres, centre, nodes, el) = ss_layup_plate(tp);
    // The consistent load of q = q0 sin(pi x) sin(pi y) along +z by the 3 x 3 Gauss rule on each element.
    let g = (0.6f64).sqrt();
    let gp = [(-g, 5.0 / 9.0), (0.0, 8.0 / 9.0), (g, 5.0 / 9.0)];
    let mut fz = vec![0.0; nodes.len()];
    for e in &el {
        let x0 = nodes[e[0]];
        let hx = nodes[e[1]][0] - x0[0];
        let hy = nodes[e[3]][1] - x0[1];
        for &(xi, wx) in &gp {
            for &(eta, wy) in &gp {
                let x = x0[0] + 0.5 * (1.0 + xi) * hx;
                let y = x0[1] + 0.5 * (1.0 + eta) * hy;
                let q = q0 * (PI * x).sin() * (PI * y).sin();
                let nn = [
                    0.25 * (1.0 - xi) * (1.0 - eta),
                    0.25 * (1.0 + xi) * (1.0 - eta),
                    0.25 * (1.0 + xi) * (1.0 + eta),
                    0.25 * (1.0 - xi) * (1.0 + eta),
                ];
                for a in 0..4 {
                    fz[e[a]] += wx * wy * 0.25 * hx * hy * nn[a] * q;
                }
            }
        }
    }
    let forces: Vec<NodalForceAt> = fz
        .iter()
        .enumerate()
        .map(|(k, &f)| NodalForceAt::new(k, [0.0, 0.0, f]))
        .collect();
    let r = solve_static_with(&m, &pres, &forces, &StaticOptions::default()).unwrap();
    let w = r.displacement(centre)[2];
    let (den, wrong) = plate_denominators(tp);
    let w_navier = q0 / (PI.powi(4) * den);
    println!(
        "PL1 w_c = {w:.10e}, w_Navier = {w_navier:.10e}, ratio {:.10}",
        w / w_navier
    );
    c.at_most("PL1 SS [0/90]s plate against Navier", (w / w_navier - 1.0).abs(), 1e-2);
    let w_wrong = q0 / (PI.powi(4) * wrong);
    c.above("N3 against the formula with D12 + D66", (w / w_wrong - 1.0).abs(), 5e-2);
    // PL2.
    let modes = modal_analysis_with(&m, &pres, &ModalOptions::new(1)).unwrap();
    let f_ref = PI * PI * (den / (RHO * tp)).sqrt() / (2.0 * PI);
    println!("PL2 f_11 = {:.8} Hz, closed form {f_ref:.8} Hz", modes.frequencies[0]);
    c.at_most(
        "PL2 (1,1) frequency against the closed form",
        (modes.frequencies[0] / f_ref - 1.0).abs(),
        1e-2,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------ ST1 to ST3, N4

/// The heated strip of arithmetic 4 with the ply expansions `alpha`: model and the closed form `(kappa_x, eps, |N_th dT|,
/// |M_th dT|)`.
fn strip(alpha: [f64; 2]) -> (SolidModel, f64, f64, f64, f64) {
    let t = 0.002;
    let dt = 10.0;
    let mat = ortho_with(RHO, alpha);
    let (nodes, el) = grid(0.1, 0.02, 20, 4);
    let n = nodes.len();
    let prop = BlockProperty::layup(plies_of(&mat, &[0.0, 90.0], t)).unwrap();
    let mut m = shell_model(nodes, &el, prop, steel());
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("origin", comp, 0.0)).unwrap();
    }
    m.add_load(Load::temperature(vec![293.15 + dt; n], 293.15)).unwrap();
    let (q11, q12, q22, _) = q_hand();
    let delta = q11 * q22 - q12 * q12;
    let k = 24.0 * delta * (alpha[1] - alpha[0]) * dt / (t * ((q11 - q22).powi(2) + 16.0 * delta));
    let h = t / 2.0;
    let p = q11 * alpha[0] + q12 * alpha[1];
    let q = q12 * alpha[0] + q22 * alpha[1];
    let nn = (p + q) * h * dt;
    let b = (q22 - q11) * h * h / 2.0;
    let eps = (nn - b * k) / ((q11 + q22 + 2.0 * q12) * h);
    let mth = (q - p).abs() * h * h / 2.0 * dt;
    (m, k, eps, nn, mth)
}

#[test]
fn st1_to_st3_n4_heated_unsymmetric_strip() {
    let mut c = Checks::new();
    let (m, k, eps, nth, mth) = strip([A1, A2]);
    println!("closed form kappa_x = {k:.12e}, eps = {eps:.12e}, |N_th dT| = {nth:.6e}, |M_th dT| = {mth:.6e}");
    let r = solve_static(&m, &StaticOptions::default()).unwrap();
    let resp = layup_response(&m, r.displacement_values()).unwrap();
    c.holds(
        "ST layup_response has every element",
        resp.len() == 80,
        &format!("{}", resp.len()),
    );
    let kx = max_finite(resp.iter().map(|x| (x.curvature[0] / k - 1.0).abs()));
    let ky = max_finite(resp.iter().map(|x| (-x.curvature[1] / k - 1.0).abs()));
    println!(
        "element 0: kappa = {:?}, eps = {:?}, dT = {}",
        resp[0].curvature, resp[0].strain, resp[0].delta_t
    );
    c.at_most("ST1 kappa_x against the closed form", kx, 5e-3);
    c.at_most("ST1 -kappa_y against the closed form", ky, 5e-3);
    let st = shell::stresses(&m, r.displacement_values()).unwrap();
    let n_err = max_finite(st.iter().flat_map(|(_, s)| s.membrane.map(f64::abs))) / nth;
    let m_err = max_finite(st.iter().flat_map(|(_, s)| s.bending.map(f64::abs))) / mth;
    c.at_most("ST2 max |N| / |N_th dT|", n_err, 1e-8);
    c.at_most("ST2 max |M| / |M_th dT|", m_err, 1e-8);
    c.at_most("ST3 kappa_x at 1e-8", kx, 1e-8);
    c.at_most("ST3 -kappa_y at 1e-8", ky, 1e-8);
    c.at_most(
        "ST3 |kappa_xy| / kappa_x",
        max_finite(resp.iter().map(|x| x.curvature[2].abs())) / k,
        1e-8,
    );
    c.at_most(
        "ST3 eps_x against the closed form",
        max_finite(resp.iter().map(|x| (x.strain[0] / eps - 1.0).abs())),
        1e-8,
    );
    c.at_most(
        "ST3 eps_y against the closed form",
        max_finite(resp.iter().map(|x| (x.strain[1] / eps - 1.0).abs())),
        1e-8,
    );
    c.at_most(
        "ST3 |gamma_xy| / eps",
        max_finite(resp.iter().map(|x| x.strain[2].abs())) / eps,
        1e-8,
    );
    // N4: no expansion mismatch, no curvature.
    let (m4, _, _, _, _) = strip([A1, A1]);
    let r4 = solve_static(&m4, &StaticOptions::default()).unwrap();
    let resp4 = layup_response(&m4, r4.displacement_values()).unwrap();
    let kmax = max_finite(resp4.iter().flat_map(|x| x.curvature.map(f64::abs)));
    c.at_most(
        "N4 isotropic expansion: max |kappa| t / (alpha dT)",
        kmax * 0.002 / (A1 * 10.0),
        1e-8,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------ CP1, CP2, N5

fn strengths() -> PlyStrengths {
    PlyStrengths::new(150e6, 110e6, 60e6, 90e6, 50e6).unwrap()
}

#[test]
fn cp1_cp2_n5_off_axis_coupon() {
    let mut c = Checks::new();
    let t = 0.002;
    let sigma = 1.0e6;
    let (nx, ny) = (10, 2);
    let st = strengths();
    for deg in [15.0, 30.0, 45.0, 60.0] {
        let (nodes, el) = grid(0.1, 0.02, nx, ny);
        let ply = Ply::new(ortho(), t, rad(deg)).unwrap();
        let constants = ply_constants(&ply, 0).unwrap();
        let prop = BlockProperty::layup(vec![ply]).unwrap();
        let mut pres = Vec::new();
        for (k, x) in nodes.iter().enumerate() {
            for comp in [2, 3, 4] {
                pres.push(Prescribed::new(k, comp, 0.0));
            }
            if x[0] == 0.0 {
                pres.push(Prescribed::new(k, 0, 0.0));
            }
        }
        pres.push(Prescribed::new(0, 1, 0.0));
        let f = sigma * 0.02 * t;
        let forces: Vec<NodalForceAt> = (0..=ny)
            .map(|j| {
                let k = j * (nx + 1) + nx;
                let w = if j == 0 || j == ny { 0.25 } else { 0.5 };
                NodalForceAt::new(k, [w * f, 0.0, 0.0])
            })
            .collect();
        let m = shell_model(nodes, &el, prop, steel());
        let r = solve_static_with(&m, &pres, &forces, &StaticOptions::default()).unwrap();
        let resp = layup_response(&m, r.displacement_values()).unwrap();
        let (s, co) = rad(deg).sin_cos();
        let want = [sigma * co * co, sigma * s * s, -sigma * s * co];
        let err = max_finite(resp.iter().flat_map(|x| {
            x.plies[0]
                .stress
                .iter()
                .flat_map(|v| (0..3).map(move |i| (v[i] - want[i]).abs() / sigma))
                .collect::<Vec<_>>()
        }));
        c.holds("CP coupon has every element", resp.len() == nx * ny, "");
        c.at_most(
            &format!("CP1 {deg} deg: ply stresses against sigma (c^2, s^2, -s c)"),
            err,
            1e-10,
        );
        let (x, y, ss) = (st.xt, st.yt, st.s);
        let c4 = co.powi(4);
        let s4 = s.powi(4);
        let th_ref = (c4 / (x * x) + (1.0 / (ss * ss) - 1.0 / (x * x)) * s * s * co * co + s4 / (y * y)).powf(-0.5);
        let mut th_err = Vec::new();
        let mut ms_ratio = Vec::new();
        for resp_e in &resp {
            for side in 0..2 {
                let p = &resp_e.plies[0];
                let th = failure_index(Criterion::TsaiHill, &st, &constants, p.stress[side], p.strain[side]);
                th_err.push((th.reserve * sigma / th_ref - 1.0).abs());
                let ms = failure_index(Criterion::MaxStress, &st, &constants, p.stress[side], p.strain[side]);
                ms_ratio.push((ms.reserve / th.reserve - 1.0).abs());
            }
        }
        println!(
            "{deg} deg: Tsai-Hill failure stress {:.6e} Pa (closed form {th_ref:.6e})",
            th_ref
        );
        c.at_most(
            &format!("CP2 {deg} deg: Tsai-Hill failure stress against the closed form"),
            max_finite(th_err),
            1e-10,
        );
        let min_ratio = ms_ratio.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        c.above(
            &format!("N5 {deg} deg: max-stress against Tsai-Hill failure stress"),
            min_ratio,
            0.1,
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------ TW1, TW2, FC1

#[test]
fn tw1_tw2_fc1_criteria_on_the_axes() {
    let mut c = Checks::new();
    let st = strengths();
    let ply = Ply::new(ortho(), 0.001, 0.0).unwrap();
    let pc = ply_constants(&ply, 0).unwrap();
    let s = 1.0e6;
    let cases = [
        ("+sigma1", [s, 0.0, 0.0], st.xt),
        ("-sigma1", [-s, 0.0, 0.0], st.xc),
        ("+sigma2", [0.0, s, 0.0], st.yt),
        ("-sigma2", [0.0, -s, 0.0], st.yc),
        ("+tau12", [0.0, 0.0, s], st.s),
        ("-tau12", [0.0, 0.0, -s], st.s),
    ];
    for (id, crit) in [
        ("TW1", Criterion::tsai_wu()),
        ("TW2 f12* = 0", Criterion::tsai_wu_with(0.0).unwrap()),
        ("TW2 f12* = 0.3", Criterion::tsai_wu_with(0.3).unwrap()),
    ] {
        for (name, stress, want) in &cases {
            let fi = failure_index(crit, &st, &pc, *stress, [0.0; 3]);
            c.at_most(
                &format!("{id} Tsai-Wu {name}: failure stress against the strength"),
                (fi.reserve * s / want - 1.0).abs(),
                1e-12,
            );
        }
    }
    for (name, stress, want) in &cases {
        let fi = failure_index(Criterion::MaxStress, &st, &pc, *stress, [0.0; 3]);
        c.at_most(
            &format!("FC1 max stress {name}"),
            (fi.reserve * s / want - 1.0).abs(),
            1e-12,
        );
    }
    let (e1, e2) = (pc.e[0], pc.e[1]);
    let nu21 = NU12 * E2 / E1;
    let strain1 = [s / e1, -NU12 * s / e1, 0.0];
    let strain2 = [-nu21 * s / e2, s / e2, 0.0];
    let fi = failure_index(Criterion::MaxStrain, &st, &pc, [s, 0.0, 0.0], strain1);
    c.at_most(
        "FC1 max strain under sigma1",
        (fi.reserve * s / st.xt - 1.0).abs(),
        1e-12,
    );
    let fi = failure_index(Criterion::MaxStrain, &st, &pc, [0.0, s, 0.0], strain2);
    c.at_most(
        "FC1 max strain under sigma2",
        (fi.reserve * s / st.yt - 1.0).abs(),
        1e-12,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------ RF1 to RF4, N6

fn contains<T>(r: &rcfd_fem::Result<T>, parts: &[&str]) -> bool {
    match r {
        Err(e) => {
            let s = e.to_string();
            parts.iter().all(|p| s.contains(p))
        }
        Ok(_) => false,
    }
}

fn valid_plies() -> Vec<Ply> {
    plies_of(&ortho(), &[0.0, 45.0, 90.0], 0.003)
}

fn small_model(plies: Vec<Ply>) -> SolidModel {
    let (nodes, el) = grid(1.0, 1.0, 2, 2);
    let mut m = shell_model(nodes, &el, BlockProperty::layup(plies).unwrap(), steel());
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("origin", comp, 0.0)).unwrap();
    }
    m.add_load(Load::shell_pressure("all", 10.0)).unwrap();
    m
}

#[test]
fn rf1_to_rf4_n6_refusals() {
    let mut c = Checks::new();
    // RF1.
    for bad in [0.0, -1e-3, f64::NAN] {
        let mut plies = valid_plies();
        plies[1].thickness = bad;
        let r1 = section_stiffness(&plies);
        let r2 = laminate::laminate(&plies);
        let ok = matches!(&r1, Err(FemError::InvalidValue { key: "ply", .. }))
            && contains(&r1, &["ply 1"])
            && matches!(&r2, Err(FemError::InvalidValue { key: "ply", .. }))
            && contains(&r2, &["ply 1"]);
        c.holds(&format!("RF1 thickness {bad} in ply 1"), ok, &format!("{:?}", r1.err()));
    }
    // RF2.
    let iso = steel();
    let terms = vec![PronyTerm::new(0.3, 0.1, 1e-3).unwrap()];
    let others: Vec<(&str, Material)> = vec![
        ("J2", Material::j2(iso.clone(), 2.5e8, 1e9, 5e8).unwrap()),
        (
            "Tabulated",
            Material::tabulated(
                iso.clone(),
                vec![293.0, 373.0],
                vec![("e".to_string(), vec![2e11, 1.9e11])],
            )
            .unwrap(),
        ),
        (
            "Hyperelastic",
            Material::hyperelastic(HyperelasticEnergy::neo_hookean(1e6, 1e9).unwrap(), 1100.0).unwrap(),
        ),
        ("Creep", Material::creep(iso.clone(), 1e-30, 4.0, -0.5).unwrap()),
        (
            "Viscoelastic",
            Material::viscoelastic(iso.clone(), terms, None).unwrap(),
        ),
    ];
    for (name, mat) in &others {
        let mut plies = valid_plies();
        plies[2].material = mat.clone();
        let r = section_stiffness(&plies);
        c.holds(
            &format!("RF2 a {name} ply 2"),
            matches!(&r, Err(FemError::InvalidValue { key: "ply", .. })) && contains(&r, &["ply 2", name]),
            &format!("{:?}", r.err()),
        );
    }
    // RF3.
    let mut plies = valid_plies();
    plies[1].thickness = -1e-3;
    let r = solve_static(&small_model(plies), &StaticOptions::default());
    c.holds(
        "RF3 solve_static, thickness -1e-3 in ply 1",
        contains(&r, &["block 0", "ply 1"]),
        &format!("{:?}", r.err()),
    );
    let mut plies = valid_plies();
    plies[0].material = others[0].1.clone();
    let r = solve_static(&small_model(plies), &StaticOptions::default());
    c.holds(
        "RF3 solve_static, a J2 ply 0",
        contains(&r, &["block 0", "ply 0", "J2"]),
        &format!("{:?}", r.err()),
    );
    // RF4.
    let mut plies = valid_plies();
    plies[0].angle = f64::NAN;
    c.holds("RF4 angle NaN in ply 0", contains(&check_plies(&plies), &["ply 0"]), "");
    c.holds(
        "RF4 empty layup property",
        BlockProperty::layup(Vec::new()).is_err(),
        "",
    );
    c.holds("RF4 empty ply list", section_stiffness(&[]).is_err(), "");
    c.holds(
        "RF4 strengths 0 and -1",
        PlyStrengths::new(0.0, 1.0, 1.0, 1.0, 1.0).is_err() && PlyStrengths::new(1.0, 1.0, 1.0, 1.0, -1.0).is_err(),
        "",
    );
    c.holds(
        "RF4 f12_star 1 and -1",
        Criterion::tsai_wu_with(1.0).is_err() && Criterion::tsai_wu_with(-1.0).is_err(),
        "",
    );
    // N6.
    c.holds("N6 valid layup: section", section_stiffness(&valid_plies()).is_ok(), "");
    let r = solve_static(&small_model(valid_plies()), &StaticOptions::default());
    c.holds("N6 valid layup: solve_static", r.is_ok(), &format!("{:?}", r.err()));
    c.finish();
}

// ------------------------------------------------------------------------------------------------ MS1

#[test]
fn ms1_layup_mass() {
    let mut c = Checks::new();
    let t = 0.002;
    let (rho0, rho1) = (1400.0, 2800.0);
    let plies = vec![
        Ply::new(ortho_with(rho0, [A1, A2]), t / 2.0, 0.0).unwrap(),
        Ply::new(ortho_with(rho1, [A1, A2]), t / 2.0, rad(90.0)).unwrap(),
    ];
    let (nodes, el) = grid(2.0, 1.0, 4, 2);
    let m = shell_model(nodes, &el, BlockProperty::layup(plies).unwrap(), steel());
    let mm = consistent(&m).unwrap();
    let p = mass_properties(&m, &mm).unwrap();
    let total = 2.0 * (rho0 + rho1) * t / 2.0;
    c.at_most(
        "MS1 total mass against area sum rho_k t_k",
        (p.mass / total - 1.0).abs(),
        1e-12,
    );
    let offset = (rho1 - rho0) * t / (4.0 * (rho0 + rho1));
    println!("MS1 centre {:?}, expected offset {offset:e}", p.centre);
    c.at_most(
        "MS1 centre offset along e_3 against m1/m0 (over t)",
        (p.centre[2] - offset).abs() / t,
        1e-12,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------ RX2

#[test]
#[ignore = "record: ADR 0502 (F-591) takes gravity and the buckling of a layup, retiring its two rows; continued in rx2_paths_that_refuse_a_layup_after_f591 (ADR 0291 pattern, maintainer standing decision)"]
fn rx2_paths_that_refuse_a_layup() {
    let mut c = Checks::new();
    let plies = plies_of(&ortho(), &[0.0, 90.0], 0.002);
    let (nodes, el) = grid(1.0, 0.2, 4, 1);
    let base = || {
        let mut m = shell_model(
            nodes.clone(),
            &el,
            BlockProperty::layup(plies.clone()).unwrap(),
            steel(),
        );
        for comp in 0..6 {
            m.add_constraint(Constraint::displacement("origin", comp, 0.0)).unwrap();
        }
        m
    };
    let mut g = base();
    g.add_load(Load::gravity([0.0, 0.0, -9.81])).unwrap();
    let r = solve_static(&g, &StaticOptions::default());
    c.holds(
        "RX2 gravity on a layup",
        refused_with(&r, laminate::FOLLOW_UP),
        &format!("{:?}", r.err()),
    );
    let mut b = base();
    b.add_load(Load::nodal_force("all", [-1.0, 0.0, 0.0])).unwrap();
    let r = solve_static(&b, &StaticOptions::default())
        .and_then(|s| prestress_from_static(&b, &s))
        .and_then(|p| factors(&b, &p, &BucklingOptions::new(1)));
    c.holds(
        "RX2 buckling of a layup",
        refused_with(&r, laminate::FOLLOW_UP),
        &format!("{:?}", r.err()),
    );
    let mut n = base();
    n.add_load(Load::shell_pressure("all", 10.0)).unwrap();
    let r = solve(&n, &[], &[], &NewtonOptions::new(1));
    c.holds(
        "RX2 nonlinear::solve of a layup",
        refused_with(&r, laminate::FOLLOW_UP),
        &format!("{:?}", r.as_ref().err()),
    );
    c.finish();
}

/// RX2 after ADR 0502 (F-591, its D9 and RX4): gravity on the layup solves, the buckling chain of the layup is taken, and
/// `nonlinear::solve` still refuses it naming the laminate's follow-up.
#[test]
fn rx2_paths_that_refuse_a_layup_after_f591() {
    let mut c = Checks::new();
    let plies = plies_of(&ortho(), &[0.0, 90.0], 0.002);
    let (nodes, el) = grid(1.0, 0.2, 4, 1);
    let base = || {
        let mut m = shell_model(
            nodes.clone(),
            &el,
            BlockProperty::layup(plies.clone()).unwrap(),
            steel(),
        );
        for comp in 0..6 {
            m.add_constraint(Constraint::displacement("origin", comp, 0.0)).unwrap();
        }
        m
    };
    let mut g = base();
    g.add_load(Load::gravity([0.0, 0.0, -9.81])).unwrap();
    let r = solve_static(&g, &StaticOptions::default());
    c.holds("RX4 gravity on a layup solves", r.is_ok(), &format!("{:?}", r.err()));
    let mut b = base();
    b.add_load(Load::nodal_force("all", [-1.0, 0.0, 0.0])).unwrap();
    let r = solve_static(&b, &StaticOptions::default())
        .and_then(|s| prestress_from_static(&b, &s))
        .and_then(|p| factors(&b, &p, &BucklingOptions::new(1)));
    c.holds(
        "RX4 buckling of a layup taken",
        r.is_ok(),
        &format!("{:?}", r.as_ref().err()),
    );
    let mut n = base();
    n.add_load(Load::shell_pressure("all", 10.0)).unwrap();
    let r = solve(&n, &[], &[], &NewtonOptions::new(1));
    c.holds(
        "RX4 nonlinear::solve of a layup still refused",
        refused_with(&r, laminate::FOLLOW_UP),
        &format!("{:?}", r.as_ref().err()),
    );
    c.finish();
}
