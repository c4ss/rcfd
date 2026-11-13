//! F-485 (ADR 0385): the Hex20 cantilever with the support of beam theory at the root (the full clamp beside it) and the
//! bore hoop stress of the thick cylinder on the radially refined 8 x 8 mesh. Checks C2, R1, R2, K1 to K3, S2, T2a, T2b and
//! the negative controls N1 to N3 of the registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::hex20::{pressure_loads, traction_loads, upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::recover::{nodal_average, IntegrationPointField};
use rcfd_fem::solve::{solve_static_with, StaticOptions};
use rcfd_fem::{Constraint, FemError, Material, SolidMesh, SolidModel, SolidResult};
use std::f64::consts::PI;

const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const C_L: f64 = 2.0;
const C_H: f64 = 0.1;
const C_P: f64 = 1000.0;
/// The ratio of the full clamp recorded by ADR 0292 amendment 1 (six printed digits).
const CLAMP_RECORD: f64 = 0.981949;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
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

fn model_of(mesh: SolidMesh, nu: f64) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_MOD, nu, RHO, 0.0).unwrap())
        .unwrap();
    m
}

// ---------------------------------------------------------------------------------------------------------------------
// The cantilever (ADR 0385 D1)

#[derive(Clone, Copy, PartialEq, Debug)]
enum Support {
    /// D1: `u_x` on the root face, `u_z` at both neutral-line nodes, `u_y` at `n0`.
    Beam,
    /// R2: as `Beam` with `u_y` at both neutral-line nodes.
    BeamBothY,
    /// N1: as `Beam` with `u_z` at `n0` only.
    BeamOneZ,
    /// R1: the full clamp of ADR 0292 (`xmin`, three components).
    Clamp,
}

struct Cantilever {
    /// The nodes of `xmin` and the neutral-line nodes `n0` (y = 0), `n1` (y = h).
    root: Vec<usize>,
    neutral: Vec<usize>,
    model: SolidModel,
    loads: Vec<NodalForceAt>,
}

fn cantilever_setup(support: Support) -> Cantilever {
    let base = hex_box([0.0; 3], [C_L, C_H, C_H], [10, 1, 1]).unwrap();
    let mut mesh = upgrade_to_hex20(&base).unwrap().0;
    let root: Vec<usize> = mesh.node_set("xmin").unwrap().to_vec();
    let mut neutral: Vec<usize> = root
        .iter()
        .copied()
        .filter(|&n| (mesh.nodes()[n][2] - 0.5 * C_H).abs() <= 1e-12)
        .collect();
    neutral.sort_by(|&a, &b| mesh.nodes()[a][1].total_cmp(&mesh.nodes()[b][1]));
    let loads = traction_loads(&mesh, "xmax", [0.0, 0.0, -C_P / (C_H * C_H)]).unwrap();
    if support != Support::Clamp {
        let z_nodes = if support == Support::BeamOneZ {
            vec![neutral[0]]
        } else {
            neutral.clone()
        };
        let y_nodes = if support == Support::BeamBothY {
            neutral.clone()
        } else {
            vec![neutral[0]]
        };
        mesh.add_node_set("root_neutral", z_nodes).unwrap();
        mesh.add_node_set("root_neutral_y", y_nodes).unwrap();
    }
    let mut model = model_of(mesh, NU);
    match support {
        Support::Clamp => {
            for c in 0..3 {
                model.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
            }
        }
        _ => {
            model.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
            model
                .add_constraint(Constraint::displacement("root_neutral", 2, 0.0))
                .unwrap();
            model
                .add_constraint(Constraint::displacement("root_neutral_y", 1, 0.0))
                .unwrap();
        }
    }
    Cantilever {
        root,
        neutral,
        model,
        loads,
    }
}

fn delta_t() -> f64 {
    let i = C_H.powi(4) / 12.0;
    let g = E_MOD / (2.0 * (1.0 + NU));
    let kappa = 5.0 / 6.0;
    C_P * C_L.powi(3) / (3.0 * E_MOD * i) * (1.0 + 3.0 * E_MOD * i / (kappa * g * C_H * C_H * C_L * C_L))
}

fn solve(c: &Cantilever) -> Result<SolidResult, FemError> {
    solve_static_with(&c.model, &[], &c.loads, &StaticOptions::default())
}

/// `delta / delta_T` with `delta = sum_a f_az u_az / P` (ADR 0292 amendment 1).
fn ratio(c: &Cantilever, res: &SolidResult, tag: &str) -> f64 {
    let delta: f64 = c
        .loads
        .iter()
        .map(|f| f.force[2] * res.displacement(f.node)[2])
        .sum::<f64>()
        / C_P;
    let dt = delta_t();
    println!(
        "{tag}: unknowns {}, delta {delta:.6e} m, delta_T {dt:.6e} m, ratio {:.6}",
        res.stats().unwrap().unknowns(),
        delta / dt
    );
    delta / dt
}

/// The root node at `(0, y, z)`.
fn root_node(c: &Cantilever, y: f64, z: f64) -> usize {
    let nodes = c.model.mesh().nodes();
    let found: Vec<usize> = c
        .root
        .iter()
        .copied()
        .filter(|&n| (nodes[n][1] - y).abs() <= 1e-12 && (nodes[n][2] - z).abs() <= 1e-12)
        .collect();
    assert_eq!(found.len(), 1, "root node at y = {y}, z = {z}");
    found[0]
}

/// Width changes `u_y(0, h, z) - u_y(0, 0, z)` of the root face along its top (`z = h`) and bottom (`z = 0`) edges.
fn root_width_changes(c: &Cantilever, res: &SolidResult) -> (f64, f64) {
    let w = |z: f64| res.displacement(root_node(c, C_H, z))[1] - res.displacement(root_node(c, 0.0, z))[1];
    (w(C_H), w(0.0))
}

fn reaction(res: &SolidResult, node: usize, component: usize) -> f64 {
    let eq = res.dofs().equation(node, component).unwrap();
    res.reactions().unwrap()[eq]
}

#[test]
fn c2_k1_to_k3_s2_r2_beam_support() {
    let mut ch = Checks::new();
    let c = cantilever_setup(Support::Beam);
    let nodes = c.model.mesh().nodes().to_vec();
    let k1 = c.root.len() == 8
        && c.neutral.len() == 2
        && nodes[c.neutral[0]][1].abs() <= 1e-12
        && (nodes[c.neutral[1]][1] - C_H).abs() <= 1e-12;
    ch.holds(
        "K1 root face 8 nodes, neutral line 2 nodes at y = 0 and y = h",
        k1,
        &format!(
            "{} root nodes, neutral {:?}",
            c.root.len(),
            c.neutral.iter().map(|&n| nodes[n]).collect::<Vec<Vec3>>()
        ),
    );
    let res = solve(&c);
    ch.holds(
        "K2 support accepted by the rigid-motion check",
        res.is_ok(),
        &format!("{:?}", res.as_ref().err()),
    );
    let res = res.unwrap();
    let r = ratio(&c, &res, "C2 beam support");
    ch.at_most(
        "C2 |delta / delta_T - 1| (support of beam theory)",
        (r - 1.0).abs(),
        0.01,
    );

    // K3: the reactions of a statically determinate support.
    let rz: Vec<f64> = c.neutral.iter().map(|&n| reaction(&res, n, 2)).collect();
    let ry0 = reaction(&res, c.neutral[0], 1);
    let (mut sx, mut mx) = (0.0, 0.0);
    for &n in &c.root {
        let rx = reaction(&res, n, 0);
        sx += rx;
        mx += rx * (nodes[n][2] - 0.5 * C_H);
    }
    println!(
        "K3 reactions: R_z {rz:?}, R_y(n0) {ry0:e}, sum R_x {sx:e}, -sum R_x (z - h/2) {:e}",
        -mx
    );
    ch.at_most("K3 |sum R_z / P - 1|", ((rz[0] + rz[1]) / C_P - 1.0).abs(), 1e-8);
    ch.at_most("K3 |R_z(n0) / (P/2) - 1|", (rz[0] / (0.5 * C_P) - 1.0).abs(), 1e-8);
    ch.at_most("K3 |R_z(n1) / (P/2) - 1|", (rz[1] / (0.5 * C_P) - 1.0).abs(), 1e-8);
    ch.at_most("K3 |R_y(n0)| / P", ry0.abs() / C_P, 1e-8);
    ch.at_most("K3 |sum R_x| / P", sx.abs() / C_P, 1e-8);
    ch.at_most(
        "K3 |-sum R_x (z - h/2) / (P L) - 1|",
        (-mx / (C_P * C_L) - 1.0).abs(),
        1e-8,
    );

    // S2: the Poisson strain of the root section is free.
    let i = C_H.powi(4) / 12.0;
    let beam = NU * (C_P * C_L * 0.5 * C_H / i) / E_MOD * C_H;
    let (top, bottom) = root_width_changes(&c, &res);
    println!("S2 root width change: top {top:.6e} m, bottom {bottom:.6e} m, beam theory -/+ {beam:.6e} m");
    ch.at_most("S2 top |dw / (-beam) - 1|", (top / -beam - 1.0).abs(), 0.1);
    ch.at_most("S2 bottom |dw / beam - 1|", (bottom / beam - 1.0).abs(), 0.1);

    // R2: u_y at both neutral-line nodes.
    let c2 = cantilever_setup(Support::BeamBothY);
    let res2 = solve(&c2).unwrap();
    let r2 = ratio(&c2, &res2, "R2 beam support, u_y at both neutral nodes");
    ch.at_most("R2 |ratio_both - ratio|", (r2 - r).abs(), 1e-3);
    ch.finish();
}

#[test]
fn r1_n2_full_clamp_beside() {
    let mut ch = Checks::new();
    let c = cantilever_setup(Support::Clamp);
    let res = solve(&c).unwrap();
    let r = ratio(&c, &res, "R1 full clamp");
    ch.at_most("R1 |ratio_clamp - 0.981949|", (r - CLAMP_RECORD).abs(), 1e-6);
    ch.above("R1 |ratio_clamp - 1| (the clamp still misses)", (r - 1.0).abs(), 0.01);
    let (top, bottom) = root_width_changes(&c, &res);
    ch.holds(
        "N2 clamp root width changes exactly 0",
        top == 0.0 && bottom == 0.0,
        &format!("top {top:e}, bottom {bottom:e}"),
    );
    ch.finish();
}

#[test]
fn n1_incomplete_support_refused() {
    let mut ch = Checks::new();
    let c = cantilever_setup(Support::BeamOneZ);
    let res = solve(&c);
    let ok = matches!(res, Err(FemError::RigidBodyMotionFree { .. }));
    ch.holds(
        "N1 u_z at n0 only refused as a free rigid motion",
        ok,
        &match &res {
            Ok(_) => "solved".to_string(),
            Err(e) => e.to_string(),
        },
    );
    ch.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// The thick cylinder (ADR 0385 D2)

/// T1 of ADR 0292 on `n_r x n_t x 1`: (bore mid-edge radius error, max bore `u_r` error, max bore hoop-stress error).
fn cylinder(n_r: usize, n_t: usize) -> (f64, f64, f64) {
    let (a, b, p) = (1.0, 2.0, 1e8);
    let param = hex_box([a, 0.0, 0.0], [b - a, PI / 2.0, 0.2], [n_r, n_t, 1]).unwrap();
    let (mesh, mids) = upgrade_to_hex20_mapped(&param, |q| [q[0] * q[1].cos(), q[0] * q[1].sin(), q[2]]).unwrap();
    let bore: Vec<usize> = mesh.node_set("xmin").unwrap().to_vec();
    let mut radius: f64 = 0.0;
    for m in &mids {
        if bore.contains(&m.node) {
            let x = mesh.nodes()[m.node];
            radius = radius.max(((x[0] * x[0] + x[1] * x[1]).sqrt() - a).abs());
        }
    }
    let loads = pressure_loads(&mesh, "xmin", p).unwrap();
    let mut model = model_of(mesh, NU);
    for (set, c) in [("zmin", 2), ("zmax", 2), ("ymin", 1), ("ymax", 0)] {
        model.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
    let res = solve_static_with(&model, &[], &loads, &StaticOptions::default()).unwrap();
    let mesh = model.mesh();
    let big_a = p * a * a / (b * b - a * a);
    let big_b = big_a * b * b;
    let ur_a = (1.0 + NU) / E_MOD * ((1.0 - 2.0 * NU) * big_a * a + big_b / a);
    let s_a = big_a + big_b / (a * a);
    let field = IntegrationPointField::compute(&model, res.displacement_values()).unwrap();
    let nodal = nodal_average(&model, &field, field.stress(), None).unwrap();
    let (mut eu, mut es): (f64, f64) = (0.0, 0.0);
    for &n in &bore {
        let x = mesh.nodes()[n];
        let r = (x[0] * x[0] + x[1] * x[1]).sqrt();
        let (c, s) = (x[0] / r, x[1] / r);
        let u = res.displacement(n);
        eu = eu.max(((u[0] * c + u[1] * s) / ur_a - 1.0).abs());
        let sg = nodal[n].unwrap();
        // t^T sigma t with t = (-s, c, 0); Voigt xx, yy, zz, xy, yz, zx.
        let hoop = s * s * sg[0] + c * c * sg[1] - 2.0 * s * c * sg[3];
        es = es.max((hoop / s_a - 1.0).abs());
    }
    println!(
        "cylinder {n_r} x {n_t}: {} nodes, {} bore nodes, u_r(a) Lame {ur_a:.6e}, hoop(a) Lame {s_a:.6e}",
        mesh.nodes().len(),
        bore.len()
    );
    (radius, eu, es)
}

#[test]
fn t2_refined_bore() {
    let mut ch = Checks::new();
    let (radius, eu, es) = cylinder(8, 8);
    println!("bore mid-edge nodes on the circle (printed, not registered): {radius:.2e}");
    ch.at_most("T2a bore radial displacement (8 x 8)", eu, 0.002);
    ch.at_most("T2b bore hoop stress (8 x 8)", es, 0.01);
    ch.finish();
}

#[test]
fn n3_coarse_bore_misses() {
    let mut ch = Checks::new();
    let (_, _, es) = cylinder(4, 8);
    ch.above("N3 bore hoop stress on 4 x 8 (the ADR 0292 miss)", es, 0.01);
    ch.finish();
}
