//! F-591 (ADR 0502): the laminated Shell4 under body loads and pre-stress. Checks BB1 to BB3, G1, G2, C1, C2, NB1, MF1, MF2,
//! F0 to F2, SS1, SS2, BK1 to BK3, RX3 and the negative controls NG, NF1, NBK, NS of the registration (B0 is
//! `sa_laminate_body_default_bits.rs`; RX4 is the copy in `sa_laminate.rs`, ADR 0502 D9). Each registered run is made once;
//! every value is printed.

use rcfd_core::vec3::{cross, dot, Vec3};
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::geometric_struct::prestress_from_static;
use rcfd_fem::laminate;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mass::consistent;
use rcfd_fem::material::MaterialAxes;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::nonlinear::{solve, NewtonOptions};
use rcfd_fem::rotating::{spin_softening_mass, RotatingOptions, RotatingPart, SpeedModes};
use rcfd_fem::shell;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, Load, Material, Ply, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::CsrMatrix;
use std::collections::HashMap;
use std::f64::consts::PI;

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    /// `value <= limit` with `value` finite (ADR 0502 b7).
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
            "{name}: {value:.4e} (expected > {limit:e}) {}",
            if ok { "as expected" } else { "NOT AS EXPECTED" }
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

/// The largest of finite values; NaN if any is not finite (ADR 0502 b7).
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

fn norm(v: Vec3) -> f64 {
    dot(v, v).sqrt()
}

/// `max_k |a_k - b_k| / |b|`.
fn vec_error(a: Vec3, b: Vec3) -> f64 {
    max_finite((0..3).map(|k| (a[k] - b[k]).abs())) / norm(b)
}

// ------------------------------------------------------------------------------------------------ data (ADR 0496)

const E1: f64 = 10.0e9;
const E2: f64 = 4.0e9;
const NU12: f64 = 0.35;
const G12: f64 = 1.6e9;
const RHO: f64 = 1400.0;
const AXIS: Vec3 = [0.2, 0.3, 1.0];
const ORIGIN: Vec3 = [0.1, 0.05, -0.1];
const OMEGA: f64 = 50.0;
const GRAV: Vec3 = [1.0, -2.0, -9.81];
const BODY: Vec3 = [100.0, 200.0, -300.0];
/// The Southwell coefficient of arithmetic 3.
const SOUTHWELL: f64 = 1.1933364;
const BETA1: f64 = 1.8751040687;

fn ortho_with(density: f64) -> Material {
    let axes = MaterialAxes::fixed([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]).unwrap();
    Material::orthotropic(
        [E1, E2, 4.0e9],
        [NU12, 0.35, 0.45],
        [G12, 1.6e9, 1.4e9],
        density,
        [2.0e-5, 6.0e-5, 6.0e-5],
        axes,
    )
    .unwrap()
}

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 1.2e-5).unwrap()
}

fn aluminium() -> Material {
    Material::isotropic_elastic(7.0e10, 0.33, 2700.0, 2.3e-5).unwrap()
}

fn q_hand() -> (f64, f64, f64, f64) {
    let nu21 = NU12 * E2 / E1;
    let d = 1.0 - NU12 * nu21;
    (E1 / d, NU12 * E2 / d, E2 / d, G12)
}

/// The hand `D11, D22, D12, D66` of `[0/90]s` of thickness `t` (ADR 0496 arithmetic 3).
fn d_cross(t: f64) -> (f64, f64, f64, f64) {
    let (q11, q12, q22, q66) = q_hand();
    let t3 = t * t * t;
    (
        t3 * (7.0 * q11 + q22) / 96.0,
        t3 * (q11 + 7.0 * q22) / 96.0,
        q12 * t3 / 12.0,
        q66 * t3 / 12.0,
    )
}

fn rad(deg: f64) -> f64 {
    deg * PI / 180.0
}

fn cross_ply(t: f64) -> Vec<Ply> {
    [0.0, 90.0, 90.0, 0.0]
        .iter()
        .map(|&a| Ply::new(ortho_with(RHO), t / 4.0, rad(a)).unwrap())
        .collect()
}

/// The two-density layup of arithmetic 1: two plies of 1 mm at 0 and 90 degrees, densities 1400 and 2800.
fn two_density() -> Vec<Ply> {
    vec![
        Ply::new(ortho_with(1400.0), 1.0e-3, 0.0).unwrap(),
        Ply::new(ortho_with(2800.0), 1.0e-3, rad(90.0)).unwrap(),
    ]
}

/// `(m0, m1, m2)` of plies, computed here from the plies (bottom first, mid-plane at the half-thickness).
fn moments(plies: &[Ply]) -> (f64, f64, f64) {
    let t: f64 = plies.iter().map(|p| p.thickness).sum();
    let mut z = -0.5 * t;
    let (mut m0, mut m1, mut m2) = (0.0, 0.0, 0.0);
    for p in plies {
        let rho = p.material.density();
        let zb = z + 0.5 * p.thickness;
        m0 += rho * p.thickness;
        m1 += rho * p.thickness * zb;
        m2 += rho * (p.thickness * zb * zb + p.thickness.powi(3) / 12.0);
        z += p.thickness;
    }
    (m0, m1, m2)
}

/// A grid of `nx x ny` elements on `[0, a] x [y0, y0 + b]`, nodes mapped by `map`; element nodes counter-clockwise.
fn grid_mapped(
    a: f64,
    b: f64,
    y0: f64,
    nx: usize,
    ny: usize,
    map: &dyn Fn(Vec3) -> Vec3,
) -> (Vec<Vec3>, Vec<[usize; 4]>) {
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            nodes.push(map([a * i as f64 / nx as f64, y0 + b * j as f64 / ny as f64, 0.0]));
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

/// The rotation by `angle` about the unit `u` (Rodrigues).
fn rotate(u: Vec3, angle: f64, x: Vec3) -> Vec3 {
    let (c, s) = (angle.cos(), angle.sin());
    let ux = cross(u, x);
    let ud = dot(u, x);
    [
        x[0] * c + ux[0] * s + u[0] * ud * (1.0 - c),
        x[1] * c + ux[1] * s + u[1] * ud * (1.0 - c),
        x[2] * c + ux[2] * s + u[2] * ud * (1.0 - c),
    ]
}

fn unit(v: Vec3) -> Vec3 {
    let l = norm(v);
    [v[0] / l, v[1] / l, v[2] / l]
}

/// The rotated `1 x 0.5` plate of arithmetic 1 (4 x 2 elements) and its map.
fn rotated_map() -> impl Fn(Vec3) -> Vec3 {
    let u = unit([1.0, 1.0, 1.0]);
    move |x: Vec3| {
        let r = rotate(u, 0.4, x);
        [r[0] + 0.3, r[1] - 0.2, r[2] + 0.5]
    }
}

fn perp(a: Vec3, v: Vec3) -> Vec3 {
    let s = dot(a, v);
    [v[0] - s * a[0], v[1] - s * a[1], v[2] - s * a[2]]
}

/// The resultant force and the moment about the origin of a load vector of a shell model.
fn resultant(m: &SolidModel, f: &[f64]) -> (Vec3, Vec3) {
    let dofs = m.dof_map();
    let (mut force, mut moment) = ([0.0; 3], [0.0; 3]);
    for (node, x) in m.mesh().nodes().iter().enumerate() {
        let fi: Vec3 = std::array::from_fn(|k| f[dofs.equation(node, k).unwrap()]);
        let mi: Vec3 = std::array::from_fn(|k| f[dofs.equation(node, 3 + k).unwrap()]);
        let xf = cross(*x, fi);
        for k in 0..3 {
            force[k] += fi[k];
            moment[k] += xf[k] + mi[k];
        }
    }
    (force, moment)
}

/// The nodal vector with `v(x)` on the translations and `w` on the rotations of every node.
fn nodal_field(m: &SolidModel, v: &dyn Fn(Vec3) -> Vec3, w: Vec3) -> Vec<f64> {
    let dofs = m.dof_map();
    let mut q = vec![0.0; dofs.n_equations()];
    for (node, x) in m.mesh().nodes().iter().enumerate() {
        let vx = v(*x);
        for k in 0..3 {
            q[dofs.equation(node, k).unwrap()] = vx[k];
            q[dofs.equation(node, 3 + k).unwrap()] = w[k];
        }
    }
    q
}

fn matvec(m: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; m.n];
    m.matvec(x, &mut y);
    y
}

/// `max |a - b| / max |b|`.
fn rel_vec(a: &[f64], b: &[f64]) -> f64 {
    let scale = max_finite(b.iter().map(|x| x.abs()));
    max_finite(a.iter().zip(b).map(|(x, y)| (x - y).abs())) / scale
}

// ------------------------------------------------------------------------------------------------ BB1 to BB3

/// The warped 3 x 3 plate of ADR 0496 BT2 with a property and material, clamped at `x = 0`.
fn warped_plate(property: BlockProperty, material: Material) -> SolidModel {
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
    m
}

fn varying_field(m: &SolidModel) -> Vec<Vec3> {
    m.mesh()
        .nodes()
        .iter()
        .map(|x| [100.0 + 30.0 * x[0], 200.0 - 50.0 * x[1], -300.0 + 70.0 * x[2]])
        .collect()
}

/// Everything BB1 to BB3 compare, as named bit vectors.
fn body_record(m: &SolidModel) -> Vec<(&'static str, Vec<u64>)> {
    let mut out = Vec::new();
    let with = |l: Load| {
        let mut c = m.clone();
        c.add_load(l).unwrap();
        c
    };
    let g = with(Load::gravity(GRAV));
    let c = with(Load::centrifugal(AXIS, ORIGIN, OMEGA));
    let b = with(Load::body_force_nodal(varying_field(m)));
    out.push(("BB1 gravity vector", bits(&nodal_load_vector(&g, &[]).unwrap())));
    out.push(("BB1 centrifugal vector", bits(&nodal_load_vector(&c, &[]).unwrap())));
    out.push(("BB1 body-force vector", bits(&nodal_load_vector(&b, &[]).unwrap())));
    let mut all = m.clone();
    all.add_load(Load::gravity(GRAV)).unwrap();
    all.add_load(Load::centrifugal(AXIS, ORIGIN, OMEGA)).unwrap();
    all.add_load(Load::body_force_nodal(varying_field(m))).unwrap();
    let s = solve_static(&all, &StaticOptions::default()).unwrap();
    let mut v = s.displacement_values().to_vec();
    v.extend_from_slice(s.reactions().unwrap());
    out.push(("BB1 static", bits(&v)));
    // BB2.
    let mut e = m.clone();
    e.add_load(Load::nodal_force("x1", [-1.0e4, 0.0, 0.0])).unwrap();
    let s = solve_static(&e, &StaticOptions::default()).unwrap();
    let pre = prestress_from_static(&e, &s).unwrap();
    let pv: Vec<f64> = pre.stress().iter().flatten().copied().collect();
    out.push(("BB2 pre-stress", bits(&pv)));
    let f = factors(&e, &pre, &BucklingOptions::new(2)).unwrap();
    let mut fv = f.factors.clone();
    for mode in &f.modes {
        fv.extend_from_slice(mode);
    }
    out.push(("BB2 buckling factors and modes", bits(&fv)));
    // BB3.
    let part = RotatingPart::new(m, AXIS, ORIGIN, &RotatingOptions::new(4).modal(ModalOptions::new(4))).unwrap();
    let up: Vec<f64> = part.unit_prestress().stress().iter().flatten().copied().collect();
    out.push(("BB3 unit pre-stress", bits(&up)));
    out.push(("BB3 K_sigma,1", bits(&part.unit_geometric_stiffness().val)));
    out.push(("BB3 M_perp", bits(&part.spin_softening().val)));
    let fr = match part.modes(100.0).unwrap() {
        SpeedModes::Modes(r) => r.frequencies,
        SpeedModes::LossOfStiffness(_) => vec![f64::NAN],
    };
    println!("BB3 frequencies at 100 rad/s {fr:?}");
    out.push(("BB3 frequencies at 100 rad/s", bits(&fr)));
    out
}

#[test]
#[ignore = "record: ADR 0502 amendment 1, its BB3 frequency row was at a speed above the loss of stiffness (vacuous); continued in bb1_bb2_bb3_one_isotropic_ply_bits_after_amendment_1 (ADR 0291 pattern, maintainer standing decision)"]
fn bb1_bb2_bb3_one_isotropic_ply_bits() {
    let mut c = Checks::new();
    let twin = warped_plate(BlockProperty::shell(0.01).unwrap(), steel());
    let lay = warped_plate(
        BlockProperty::layup(vec![Ply::new(steel(), 0.01, 0.3).unwrap()]).unwrap(),
        aluminium(),
    );
    for ((name, a), (_, b)) in body_record(&lay).into_iter().zip(body_record(&twin)) {
        c.holds(
            name,
            a == b && !a.is_empty(),
            &format!(
                "{} values, one-ply layup (aluminium block) against the steel Shell twin",
                a.len()
            ),
        );
    }
    c.finish();
}

/// The rows of [`body_record`] with the BB3 frequency row replaced (ADR 0502 amendment 1): the loss of stiffness at 100 rad/s
/// (speed, negative count) and the frequencies and modes at 10 rad/s; and whether the latter are four finite frequencies.
fn body_record_after_amendment_1(m: &SolidModel) -> (Vec<(&'static str, Vec<u64>)>, bool) {
    let mut out = body_record(m);
    let last = out.pop().expect("the BB3 frequency row");
    assert_eq!(last.0, "BB3 frequencies at 100 rad/s");
    let part = RotatingPart::new(m, AXIS, ORIGIN, &RotatingOptions::new(4).modal(ModalOptions::new(4))).unwrap();
    let loss = match part.modes(100.0).unwrap() {
        SpeedModes::LossOfStiffness(l) => vec![l.speed, l.negative.map(|n| n as f64).unwrap_or(-1.0)],
        SpeedModes::Modes(_) => vec![f64::NAN],
    };
    println!("BB3 loss of stiffness at 100 rad/s {loss:?}");
    out.push(("BB3 loss of stiffness at 100 rad/s", bits(&loss)));
    let (modes, ok) = match part.modes(10.0).unwrap() {
        SpeedModes::Modes(r) => {
            println!("BB3 frequencies at 10 rad/s {:?}", r.frequencies);
            let ok = r.frequencies.len() == 4 && r.frequencies.iter().all(|f| f.is_finite() && *f > 0.0);
            let mut v = r.frequencies.clone();
            for phi in &r.modes {
                v.extend_from_slice(phi);
            }
            (v, ok)
        }
        SpeedModes::LossOfStiffness(_) => (vec![f64::NAN], false),
    };
    out.push(("BB3 frequencies and modes at 10 rad/s", bits(&modes)));
    (out, ok)
}

/// BB1 to BB3 after ADR 0502 amendment 1.
#[test]
fn bb1_bb2_bb3_one_isotropic_ply_bits_after_amendment_1() {
    let mut c = Checks::new();
    let twin = warped_plate(BlockProperty::shell(0.01).unwrap(), steel());
    let lay = warped_plate(
        BlockProperty::layup(vec![Ply::new(steel(), 0.01, 0.3).unwrap()]).unwrap(),
        aluminium(),
    );
    let (rl, okl) = body_record_after_amendment_1(&lay);
    let (rt, okt) = body_record_after_amendment_1(&twin);
    for ((name, a), (_, b)) in rl.into_iter().zip(rt) {
        c.holds(
            name,
            a == b && !a.is_empty(),
            &format!(
                "{} values, one-ply layup (aluminium block) against the steel Shell twin",
                a.len()
            ),
        );
    }
    c.holds(
        "BB3 the modes at 10 rad/s are four finite frequencies (both models)",
        okl && okt,
        &format!("layup {okl}, twin {okt}"),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------ G1, G2, C1, C2, NB1, NG

#[test]
fn g1_g2_c1_c2_nb1_ng_layup_body_loads() {
    let mut c = Checks::new();
    let map = rotated_map();
    let (nodes, el) = grid_mapped(1.0, 0.5, 0.0, 4, 2, &map);
    let plies = two_density();
    let (m0, m1, _) = moments(&plies);
    let t_total = 2.0e-3;
    let area = 0.5;
    let base = shell_model(nodes, &el, BlockProperty::layup(plies).unwrap(), steel());
    let mass = consistent(&base).unwrap();
    let centroid = map([0.5, 0.25, 0.0]);
    let o = map([0.0; 3]);
    let ez = map([0.0, 0.0, 1.0]);
    let n = [ez[0] - o[0], ez[1] - o[1], ez[2] - o[2]];
    println!("m0 = {m0:e}, m1 = {m1:e}, m1/m0 = {:e}, n = {n:?}", m1 / m0);
    // G1, G2.
    let mut g = base.clone();
    g.add_load(Load::gravity(GRAV)).unwrap();
    let fg = nodal_load_vector(&g, &[]).unwrap();
    let mg = matvec(&mass, &nodal_field(&base, &|_| GRAV, [0.0; 3]));
    c.at_most("G1 gravity load against M d_g", rel_vec(&fg, &mg), 1e-12);
    let (force, moment) = resultant(&base, &fg);
    let fref: Vec3 = std::array::from_fn(|k| m0 * area * GRAV[k]);
    c.at_most(
        "G2 gravity resultant force against m0 A g",
        vec_error(force, fref),
        1e-12,
    );
    let xcm: Vec3 = std::array::from_fn(|k| centroid[k] + m1 / m0 * n[k]);
    let mref = cross(xcm, fref);
    println!("G2 moment {moment:?}, reference {mref:?}");
    c.at_most(
        "G2 gravity resultant moment against m0 A (x_cm x g)",
        vec_error(moment, mref),
        1e-12,
    );
    c.above(
        "NG the moment against m0 A (c x g) (no first mass moment)",
        vec_error(moment, cross(centroid, fref)),
        1e-6,
    );
    // C1, C2.
    let a = unit(AXIS);
    let w2 = OMEGA * OMEGA;
    let acc = |x: Vec3| -> Vec3 {
        let r = perp(a, [x[0] - ORIGIN[0], x[1] - ORIGIN[1], x[2] - ORIGIN[2]]);
        [w2 * r[0], w2 * r[1], w2 * r[2]]
    };
    let mut cf = base.clone();
    cf.add_load(Load::centrifugal(AXIS, ORIGIN, OMEGA)).unwrap();
    let fc = nodal_load_vector(&cf, &[]).unwrap();
    let mc = matvec(&mass, &nodal_field(&base, &acc, [0.0; 3]));
    c.at_most("C1 centrifugal load against M d_c", rel_vec(&fc, &mc), 1e-12);
    let (force, _) = resultant(&base, &fc);
    let ac = acc(centroid);
    let cref: Vec3 = std::array::from_fn(|k| m0 * area * ac[k]);
    c.at_most(
        "C2 centrifugal resultant force against Omega^2 m0 A P (c - p)",
        vec_error(force, cref),
        1e-12,
    );
    // NB1.
    let mut nb = base.clone();
    nb.add_load(Load::body_force_nodal(vec![BODY; base.mesh().nodes().len()]))
        .unwrap();
    let fb = nodal_load_vector(&nb, &[]).unwrap();
    let (force, moment) = resultant(&base, &fb);
    let bref: Vec3 = std::array::from_fn(|k| t_total * area * BODY[k]);
    c.at_most(
        "NB1 body-force resultant force against t A b",
        vec_error(force, bref),
        1e-12,
    );
    c.at_most(
        "NB1 body-force resultant moment against t A (c x b)",
        vec_error(moment, cross(centroid, bref)),
        1e-12,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------ MF1, MF2, F0 to F2, NF1

const L: f64 = 0.1;
const W: f64 = 0.02;
const T_STRIP: f64 = 0.002;
const NX: usize = 40;
const NY: usize = 4;

/// The strip of arithmetic 2: `[0/90]s`, root clamped, `theta_x = 0` at every node; its root nodes.
fn strip() -> (SolidModel, Vec<usize>) {
    let (nodes, el) = grid_mapped(L, W, -0.5 * W, NX, NY, &|x| x);
    let root: Vec<usize> = (0..=NY).map(|j| j * (NX + 1)).collect();
    let mut m = shell_model(nodes, &el, BlockProperty::layup(cross_ply(T_STRIP)).unwrap(), steel());
    let mut mesh = m.mesh().clone();
    mesh.add_node_set("root", root.clone()).unwrap();
    let mut s = SolidModel::new(mesh);
    s.add_material("all", steel()).unwrap();
    for comp in 0..6 {
        s.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
    }
    s.add_constraint(Constraint::displacement("all", 3, 0.0)).unwrap();
    m = s;
    (m, root)
}

fn share(model: &SolidModel, m: &CsrMatrix, phi: &[f64], comp: usize) -> f64 {
    let dofs = model.dof_map();
    let y = matvec(m, phi);
    let (mut part, mut all) = (0.0, 0.0);
    for r in 0..m.n {
        let v = phi[r] * y[r];
        all += v;
        if dofs.node_component(r).unwrap().1 == comp {
            part += v;
        }
    }
    part / all
}

/// The flap frequency (Hz) and mode of a speed: the mode of the four with the largest kinetic share in `u_z`.
fn flap(part: &RotatingPart, model: &SolidModel, omega: f64) -> (f64, Vec<f64>) {
    let r = match part.modes(omega).unwrap() {
        SpeedModes::Modes(r) => r,
        SpeedModes::LossOfStiffness(l) => panic!("loss of stiffness at {omega}: {l:?}"),
    };
    let mut best = (0usize, -1.0);
    for (k, phi) in r.modes.iter().enumerate() {
        let s = share(model, part.mass(), phi, 2);
        if s > best.1 {
            best = (k, s);
        }
    }
    println!(
        "  speed {omega:.4} rad/s: frequencies {:?}, flap mode {} (u_z share {:.6})",
        r.frequencies, best.0, best.1
    );
    (r.frequencies[best.0], r.modes[best.0].clone())
}

fn strip_opts() -> RotatingOptions {
    RotatingOptions::new(4).modal(
        ModalOptions::new(4)
            .residual_test(ResidualTest::Backward)
            .tolerance(1e-12),
    )
}

#[test]
fn mf1_mf2_f0_f1_f2_nf1_rotating_cross_ply_strip() {
    let mut c = Checks::new();
    let (m, root) = strip();
    let (m0, _, _) = moments(&cross_ply(T_STRIP));
    let (d11, _, _, _) = d_cross(T_STRIP);
    let omega0 = BETA1 * BETA1 * (d11 / (m0 * L.powi(4))).sqrt();
    let f0 = omega0 / (2.0 * PI);
    println!("strip m0 = {m0}, D11 = {d11:.8}, omega_0 = {omega0:.6} rad/s, f_0 = {f0:.6} Hz");
    // MF1, MF2: the static centrifugal state at omega_0.
    let mut s = m.clone();
    s.add_load(Load::centrifugal([0.0, 0.0, 1.0], [0.0; 3], omega0))
        .unwrap();
    let r = solve_static(&s, &StaticOptions::default()).unwrap();
    let st = shell::stresses(&s, r.displacement_values()).unwrap();
    let h = L / NX as f64;
    let w2 = omega0 * omega0;
    let root_value = m0 * w2 * L * L / 2.0;
    let mut errs = Vec::new();
    for i in 0..NX {
        let mean = (0..NY).map(|j| st[j * NX + i].1.membrane[0]).sum::<f64>() / NY as f64;
        let rc = (i as f64 + 0.5) * h;
        let want = m0 * w2 * (L * L - rc * rc - h * h / 12.0) / 2.0;
        if i == 0 || i == NX - 1 || i == NX / 2 {
            println!("  column {i}: mean N_xx {mean:.10e}, closed form {want:.10e}");
        }
        errs.push((mean - want).abs() / root_value);
    }
    c.at_most(
        "MF1 column mean of N_xx against m0 Omega^2 (L^2 - r_c^2 - h^2/12)/2 (over the root value)",
        max_finite(errs),
        1e-8,
    );
    let dofs = s.dof_map();
    let reac = r.reactions().unwrap();
    let rx: f64 = root.iter().map(|&n| reac[dofs.equation(n, 0).unwrap()]).sum();
    let want = -m0 * W * w2 * L * L / 2.0;
    println!("MF2 reaction sum along x {rx:.10e}, expected {want:.10e}");
    c.at_most(
        "MF2 root reaction against -m0 W Omega^2 L^2/2",
        (rx / want - 1.0).abs(),
        1e-10,
    );
    // F0 to F2.
    let part = RotatingPart::new(&m, [0.0, 0.0, 1.0], [0.0; 3], &strip_opts()).unwrap();
    let (fr, phi0) = flap(&part, &m, 0.0);
    println!("F0 rest flap {fr:.8} Hz against {f0:.8} Hz");
    c.at_most("F0 rest flap frequency against f_0 (D11)", (fr / f0 - 1.0).abs(), 1e-2);
    for ratio in [0.5, 1.0] {
        let om = ratio * omega0;
        let (f, _) = flap(&part, &m, om);
        let fs = f0 * (1.0 + SOUTHWELL * ratio * ratio).sqrt();
        println!("F1 Omega = {ratio} omega_0: flap {f:.8} Hz, Southwell {fs:.8} Hz");
        c.at_most(
            &format!("F1 flap at {ratio} omega_0 against the Southwell form"),
            (f / fs - 1.0).abs(),
            2e-2,
        );
    }
    let slope = part.slope(&phi0);
    println!("F2 slope {slope:.8} against S = {SOUTHWELL}");
    c.at_most("F2 first-order slope against S", (slope / SOUTHWELL - 1.0).abs(), 1e-2);
    // NF1.
    let soft = RotatingPart::new(&m, [0.0, 0.0, 1.0], [0.0; 3], &strip_opts().stress_stiffening(false)).unwrap();
    let (f, _) = flap(&soft, &m, omega0);
    let fs = f0 * (1.0 + SOUTHWELL).sqrt();
    c.above(
        "NF1 no stress stiffening at omega_0 against the Southwell form",
        (f / fs - 1.0).abs(),
        0.2,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------ SS1, SS2, NS

fn entries(m: &CsrMatrix) -> HashMap<(usize, usize), f64> {
    let mut out = HashMap::new();
    for r in 0..m.n {
        for k in m.row_ptr[r]..m.row_ptr[r + 1] {
            *out.entry((r, m.col[k])).or_insert(0.0) += m.val[k];
        }
    }
    out
}

#[test]
fn ss1_ss2_ns_layup_spin_softening() {
    let mut c = Checks::new();
    let plies = two_density();
    let (m0, m1, m2) = moments(&plies);
    // SS1: the flat plate in z = 0.
    let (nodes, el) = grid_mapped(1.0, 0.5, 0.0, 4, 2, &|x| x);
    let flat = shell_model(nodes, &el, BlockProperty::layup(plies.clone()).unwrap(), steel());
    let dofs = flat.dof_map();
    let mass = entries(&consistent(&flat).unwrap());
    let mut sum: HashMap<(usize, usize), f64> = HashMap::new();
    for axis in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
        for (k, v) in entries(&spin_softening_mass(&flat, axis, None).unwrap()) {
            *sum.entry(k).or_insert(0.0) += v;
        }
    }
    let scale = max_finite(mass.values().map(|v| v.abs()));
    let mut keys: Vec<(usize, usize)> = mass.keys().chain(sum.keys()).copied().collect();
    keys.sort_unstable();
    keys.dedup();
    let err = max_finite(keys.iter().map(|&(r, col)| {
        let drill = dofs.node_component(r).unwrap().1 == 5 && dofs.node_component(col).unwrap().1 == 5;
        let want = if drill {
            0.0
        } else {
            2.0 * mass.get(&(r, col)).copied().unwrap_or(0.0)
        };
        (sum.get(&(r, col)).copied().unwrap_or(0.0) - want).abs()
    })) / scale;
    c.at_most(
        "SS1 sum_k M_perp(e_k) against 2 M (drilling rotary entries zero)",
        err,
        1e-12,
    );
    // SS2: the rotated plate, the rigid rotation omega about p.
    let map = rotated_map();
    let (nodes, el) = grid_mapped(1.0, 0.5, 0.0, 4, 2, &map);
    let rot = shell_model(nodes, &el, BlockProperty::layup(plies).unwrap(), steel());
    let om = [0.3, -0.5, 0.8];
    let p = ORIGIN;
    let a = unit(AXIS);
    let q = nodal_field(&rot, &|x| cross(om, [x[0] - p[0], x[1] - p[1], x[2] - p[2]]), om);
    let mp = spin_softening_mass(&rot, AXIS, None).unwrap();
    let got = dot_vec(&q, &matvec(&mp, &q));
    let o = map([0.0; 3]);
    let ez = map([0.0, 0.0, 1.0]);
    let n = [ez[0] - o[0], ez[1] - o[1], ez[2] - o[2]];
    let wn = perp(a, cross(om, n));
    // The exact integral by the 3 x 3 Gauss rule on each element of the local grid (the map is a rigid motion).
    let g = (0.6f64).sqrt();
    let gp = [(-g, 5.0 / 9.0), (0.0, 8.0 / 9.0), (g, 5.0 / 9.0)];
    let (hx, hy) = (0.25, 0.25);
    let (mut exact, mut no_coupling) = (0.0, 0.0);
    for j in 0..2 {
        for i in 0..4 {
            for &(xi, wx) in &gp {
                for &(eta, wy) in &gp {
                    let x = map([
                        hx * (i as f64 + 0.5 * (1.0 + xi)),
                        hy * (j as f64 + 0.5 * (1.0 + eta)),
                        0.0,
                    ]);
                    let u = perp(a, cross(om, [x[0] - p[0], x[1] - p[1], x[2] - p[2]]));
                    let w = wx * wy * 0.25 * hx * hy;
                    let a0 = m0 * dot(u, u) + m2 * dot(wn, wn);
                    exact += w * (a0 + 2.0 * m1 * dot(u, wn));
                    no_coupling += w * a0;
                }
            }
        }
    }
    println!("SS2 q^T M_perp q = {got:.15e}, exact {exact:.15e}");
    c.at_most(
        "SS2 rigid-rotation energy against the integral",
        (got / exact - 1.0).abs(),
        1e-12,
    );
    c.above(
        "NS the integral without the coupling term",
        (got / no_coupling - 1.0).abs(),
        1e-9,
    );
    c.finish();
}

fn dot_vec(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

// ------------------------------------------------------------------------------------------------ BK1 to BK3, NBK

#[test]
fn bk1_bk2_bk3_nbk_simply_supported_cross_ply_plate_buckling() {
    let mut c = Checks::new();
    let (a, t, cells, n_ref) = (1.0, 0.01, 16, 1000.0);
    let (nodes, el) = grid_mapped(a, a, 0.0, cells, cells, &|x| x);
    let idx = |i: usize, j: usize| j * (cells + 1) + i;
    let col = |i: usize| (0..=cells).map(|j| idx(i, j)).collect::<Vec<_>>();
    let row = |j: usize| (0..=cells).map(|i| idx(i, j)).collect::<Vec<_>>();
    let mut edges: Vec<usize> = col(0)
        .into_iter()
        .chain(col(cells))
        .chain(row(0))
        .chain(row(cells))
        .collect();
    edges.sort_unstable();
    edges.dedup();
    let base = shell_model(nodes, &el, BlockProperty::layup(cross_ply(t)).unwrap(), steel());
    let mut mesh = base.mesh().clone();
    mesh.add_node_set("edges", edges).unwrap();
    mesh.add_node_set("x0", col(0)).unwrap();
    mesh.add_node_set("c00", vec![idx(0, 0)]).unwrap();
    mesh.add_node_set("xl_end", vec![idx(cells, 0), idx(cells, cells)])
        .unwrap();
    mesh.add_node_set("xl_mid", (1..cells).map(|j| idx(cells, j)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_constraint(Constraint::displacement("edges", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("x0", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("c00", 1, 0.0)).unwrap();
    let hy = a / cells as f64;
    let mut loaded = m.clone();
    loaded
        .add_load(Load::nodal_force("xl_mid", [-n_ref * hy, 0.0, 0.0]))
        .unwrap();
    loaded
        .add_load(Load::nodal_force("xl_end", [-0.5 * n_ref * hy, 0.0, 0.0]))
        .unwrap();
    let r = solve_static(&loaded, &StaticOptions::default()).unwrap();
    let pre = prestress_from_static(&loaded, &r).unwrap();
    let opts = BucklingOptions::new(2)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-12);
    let b = factors(&m, &pre, &opts).unwrap();
    let (d11, d22, d12, d66) = d_cross(t);
    let want1 = PI * PI * (d11 + 2.0 * (d12 + 2.0 * d66) + d22) / (a * a);
    let want2 = PI * PI * (4.0 * d11 + 2.0 * (d12 + 2.0 * d66) + d22 / 4.0) / (a * a);
    let wrong = PI * PI * (d11 + 2.0 * (d12 + d66) + d22) / (a * a);
    println!(
        "BK factors {:?}: N_cr {:.8e}, {:.8e} against m = 1 {want1:.8e}, m = 2 {want2:.8e}",
        b.factors,
        b.factors.first().copied().unwrap_or(f64::NAN) * n_ref,
        b.factors.get(1).copied().unwrap_or(f64::NAN) * n_ref
    );
    let f1 = b.factors.first().copied().unwrap_or(f64::NAN) * n_ref;
    let f2 = b.factors.get(1).copied().unwrap_or(f64::NAN) * n_ref;
    c.at_most(
        "BK1 SS [0/90]s plate, 16 x 16, against pi^2 (D11 + 2 (D12 + 2 D66) + D22)/a^2",
        (f1 / want1 - 1.0).abs(),
        2e-2,
    );
    c.at_most(
        "BK2 second factor against the m = 2 load",
        (f2 / want2 - 1.0).abs(),
        5e-2,
    );
    c.above(
        "NBK BK1 against the form with D12 + D66",
        (f1 / wrong - 1.0).abs(),
        5e-2,
    );
    // BK3: one half-wave each way.
    let dofs = b.dofs();
    let mode = &b.modes[0];
    let w = |node: usize| mode[dofs.equation(node, 2).unwrap()];
    let mid = cells / 2;
    let line_y: Vec<f64> = (1..cells).map(|i| w(idx(i, mid))).collect();
    let line_x: Vec<f64> = (1..cells).map(|j| w(idx(mid, j))).collect();
    let one_sign = |v: &[f64]| v.iter().all(|x| *x > 0.0) || v.iter().all(|x| *x < 0.0);
    c.holds(
        "BK3 first mode: w of one sign along y = a/2 and along x = a/2",
        one_sign(&line_y) && one_sign(&line_x),
        &format!("y = a/2 {line_y:?}; x = a/2 {line_x:?}"),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------ RX3

#[test]
fn rx3_nonlinear_solve_still_refuses_a_layup() {
    let mut c = Checks::new();
    let (nodes, el) = grid_mapped(1.0, 0.2, 0.0, 4, 1, &|x| x);
    let plies = vec![
        Ply::new(ortho_with(RHO), 1.0e-3, 0.0).unwrap(),
        Ply::new(ortho_with(RHO), 1.0e-3, rad(90.0)).unwrap(),
    ];
    let mut m = shell_model(nodes, &el, BlockProperty::layup(plies).unwrap(), steel());
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("origin", comp, 0.0)).unwrap();
    }
    m.add_load(Load::shell_pressure("all", 10.0)).unwrap();
    let r = solve(&m, &[], &[], &NewtonOptions::new(1));
    let refused = match &r {
        Err(FemError::NotYetSupported { package, .. }) => *package == laminate::FOLLOW_UP,
        Err(FemError::Element { source, .. }) => {
            matches!(source.as_ref(), FemError::NotYetSupported { package, .. } if *package == laminate::FOLLOW_UP)
        }
        _ => false,
    };
    c.holds(
        "RX3 nonlinear::solve of a layup refused naming laminate::FOLLOW_UP",
        refused,
        &format!("{:?}", r.as_ref().err()),
    );
    c.finish();
}
