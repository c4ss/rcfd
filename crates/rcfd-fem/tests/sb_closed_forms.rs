//! F-379 (ADR 0433, the registration of ADR 0280): closed-form benchmarks. The cantilever of slenderness 20 with Tet10, Hex8
//! and the Tet4 locking control on the same nodes, with the support of beam theory at the root and the full clamp recorded
//! beside it (checks C1 to C4, K0, K1, R1, N1); Lame's thick-walled cylinder in plane strain on a quarter model, graded Hex8
//! levels and curved Tet10 (checks T1 to T4, K2, K3, N2, N3).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::recover::{nodal_average, IntegrationPointField};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, Load, Material, SolidMesh, SolidModel, SolidResult};
use std::collections::BTreeSet;
use std::f64::consts::PI;

const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;

// Cantilever (ADR 0433 D1 to D5).
const C_L: f64 = 2.0;
const C_H: f64 = 0.1;
const C_P: f64 = 1000.0;

// Cylinder (ADR 0433 D6 to D8).
const A_IN: f64 = 1.0;
const B_OUT: f64 = 2.0;
const H_AX: f64 = 0.1;
const PRESS: f64 = 1.0e6;
const BETA: f64 = 2.0;

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
    fn below(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value < limit;
        println!(
            "{name}: {value:.4e} (limit < {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} >= {limit:e}"));
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
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{name}: {value:.4e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
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

fn rel(x: f64, y: f64) -> f64 {
    (x / y - 1.0).abs()
}

fn model_of(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_MOD, NU, RHO, 0.0).unwrap())
        .unwrap();
    m
}

/// The same blocks and sets on a new node list (as `sb_static.rs`).
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

fn reaction(res: &SolidResult, node: usize, component: usize) -> f64 {
    let eq = res.dofs().equation(node, component).unwrap();
    res.reactions().unwrap()[eq]
}

// ---------------------------------------------------------------------------------------------------------------------
// The cantilever (ADR 0433 D1 to D5)

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mesh {
    /// `tet_box [40, 2, 2]` upgraded.
    Tet10,
    /// `tet_box [80, 4, 4]`.
    Tet4,
    /// `hex_box [80, 4, 4]`.
    Hex8,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Support {
    /// D2: `u_x` on the root face, `u_z` at both neutral-line nodes, `u_y` at `n0`.
    Beam,
    /// R1: `xmin` fixed in all three components.
    Clamp,
}

fn cantilever_mesh(kind: Mesh) -> SolidMesh {
    let dims = [C_L, C_H, C_H];
    match kind {
        Mesh::Tet10 => {
            upgrade_to_tet10(&tet_box([0.0; 3], dims, [40, 2, 2]).unwrap())
                .unwrap()
                .0
        }
        Mesh::Tet4 => tet_box([0.0; 3], dims, [80, 4, 4]).unwrap(),
        Mesh::Hex8 => hex_box([0.0; 3], dims, [80, 4, 4]).unwrap(),
    }
}

/// K0: every node within 1e-12 m of a distinct point of the grid `(i L/80, j h/4, k h/4)`, 2025 nodes.
fn k0_same_nodes(ch: &mut Checks, kind: Mesh, mesh: &SolidMesh) {
    let (dx, dy) = (C_L / 80.0, C_H / 4.0);
    let mut seen = BTreeSet::new();
    let mut dev = 0.0f64;
    for p in mesh.nodes() {
        let idx = [(p[0] / dx).round(), (p[1] / dy).round(), (p[2] / dy).round()];
        let q = [idx[0] * dx, idx[1] * dy, idx[2] * dy];
        for c in 0..3 {
            dev = dev.max((p[c] - q[c]).abs());
        }
        seen.insert((idx[0] as i64, idx[1] as i64, idx[2] as i64));
    }
    let n = mesh.nodes().len();
    let ok = n == 2025 && seen.len() == 2025 && dev <= 1e-12;
    ch.holds(
        &format!("K0 {kind:?} nodes on the grid L/80 x h/4 x h/4"),
        ok,
        &format!(
            "{n} nodes, {} distinct grid points, largest deviation {dev:.3e} m",
            seen.len()
        ),
    );
}

struct Cantilever {
    model: SolidModel,
    neutral: [usize; 2],
}

/// The root node at `(0, y, h/2)`.
fn neutral_node(mesh: &SolidMesh, y: f64) -> usize {
    let found: Vec<usize> = mesh
        .node_set("xmin")
        .unwrap()
        .iter()
        .copied()
        .filter(|&n| {
            let p = mesh.nodes()[n];
            (p[1] - y).abs() <= 1e-12 && (p[2] - 0.5 * C_H).abs() <= 1e-12
        })
        .collect();
    assert_eq!(found.len(), 1, "root neutral node at y = {y}");
    found[0]
}

fn cantilever(kind: Mesh, support: Support) -> Cantilever {
    let mut mesh = cantilever_mesh(kind);
    let neutral = [neutral_node(&mesh, 0.0), neutral_node(&mesh, C_H)];
    mesh.add_node_set("root_neutral", neutral.to_vec()).unwrap();
    mesh.add_node_set("root_neutral_y", vec![neutral[0]]).unwrap();
    let mut model = model_of(mesh);
    match support {
        Support::Beam => {
            model.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
            model
                .add_constraint(Constraint::displacement("root_neutral", 2, 0.0))
                .unwrap();
            model
                .add_constraint(Constraint::displacement("root_neutral_y", 1, 0.0))
                .unwrap();
        }
        Support::Clamp => {
            for c in 0..3 {
                model.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
            }
        }
    }
    model
        .add_load(Load::traction("xmax", [0.0, 0.0, -C_P / (C_H * C_H)]))
        .unwrap();
    Cantilever { model, neutral }
}

fn delta_t() -> f64 {
    let i = C_H.powi(4) / 12.0;
    let g = E_MOD / (2.0 * (1.0 + NU));
    let kappa = 5.0 / 6.0;
    C_P * C_L.powi(3) / (3.0 * E_MOD * i) * (1.0 + 3.0 * E_MOD * i / (kappa * g * C_H * C_H * C_L * C_L))
}

/// `M c / I` at `x = h` (D4).
fn sigma_beam() -> f64 {
    C_P * (C_L - C_H) * 0.5 * C_H / (C_H.powi(4) / 12.0)
}

/// Solves; returns the result and `delta / delta_T` with `delta = sum_i f_i u_i / P` (D3).
fn run_cantilever(c: &Cantilever, tag: &str) -> (SolidResult, f64) {
    let t0 = std::time::Instant::now();
    let res = solve_static(&c.model, &StaticOptions::default()).unwrap();
    let f = nodal_load_vector(&c.model, &[]).unwrap();
    let delta: f64 = f.iter().zip(res.displacement_values()).map(|(f, u)| f * u).sum::<f64>() / C_P;
    let dt = delta_t();
    println!(
        "{tag}: {} unknowns, solve {:.2} s, delta {delta:.6e} m, delta_T {dt:.6e} m, ratio {:.6}",
        res.stats().unwrap().unknowns(),
        t0.elapsed().as_secs_f64(),
        delta / dt
    );
    (res, delta / dt)
}

/// `sigma_xx` of the nodal average at the nodes `(h, y, z)` of the line `x = h`, `z = z0`, sorted by `y`.
fn section_stress(c: &Cantilever, res: &SolidResult, z0: f64) -> Vec<(f64, f64)> {
    let field = IntegrationPointField::from_result(&c.model, res).unwrap();
    let avg = nodal_average(&c.model, &field, field.stress(), None).unwrap();
    let mut out: Vec<(f64, f64)> = c
        .model
        .mesh()
        .nodes()
        .iter()
        .enumerate()
        .filter(|(_, p)| (p[0] - C_H).abs() <= 1e-12 && (p[2] - z0).abs() <= 1e-12)
        .map(|(n, p)| (p[1], avg[n].unwrap()[0]))
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

#[test]
fn k0_c1_c3_k1_r1_tet10_cantilever() {
    let mut ch = Checks::new();
    for kind in [Mesh::Tet10, Mesh::Tet4, Mesh::Hex8] {
        k0_same_nodes(&mut ch, kind, &cantilever_mesh(kind));
    }
    let c = cantilever(Mesh::Tet10, Support::Beam);
    let (res, ratio) = run_cantilever(&c, "C1 Tet10 40x2x2 beam support");
    ch.at_most("C1 Tet10 |delta / delta_T - 1|", (ratio - 1.0).abs(), 0.01);

    // C3, C3b: bending stress at x = h.
    let sb = sigma_beam();
    for (z0, sign, name) in [(C_H, 1.0, "top"), (0.0, -1.0, "bottom")] {
        let line = section_stress(&c, &res, z0);
        println!(
            "C3 {name} line x = h, z = {z0}: (y, sigma_xx) {line:?}, M c / I {:.6e}",
            sign * sb
        );
        let mid: Vec<f64> = line
            .iter()
            .filter(|(y, _)| (y - 0.5 * C_H).abs() <= 1e-12)
            .map(|&(_, s)| s)
            .collect();
        assert_eq!(mid.len(), 1, "C3 mid-width node on the {name} line");
        assert_eq!(line.len(), 5, "C3b five nodes on the {name} line");
        ch.at_most(
            &format!("C3 Tet10 {name} (h, h/2, {z0}) |sigma_xx / (M c / I) - 1|"),
            rel(mid[0], sign * sb),
            0.03,
        );
        let worst = line.iter().map(|&(_, s)| rel(s, sign * sb)).fold(0.0, f64::max);
        ch.at_most(&format!("C3b Tet10 {name} line, largest error"), worst, 0.03);
    }

    // K1: reactions of the statically determinate support.
    let root = c.model.mesh().node_set("xmin").unwrap().to_vec();
    let rz = [reaction(&res, c.neutral[0], 2), reaction(&res, c.neutral[1], 2)];
    let ry0 = reaction(&res, c.neutral[0], 1);
    let sx: f64 = root.iter().map(|&n| reaction(&res, n, 0)).sum();
    println!("K1 reactions: R_z {rz:?}, R_y(n0) {ry0:e}, sum R_x {sx:e}");
    ch.at_most("K1 |sum R_z / P - 1|", ((rz[0] + rz[1]) / C_P - 1.0).abs(), 1e-8);
    ch.at_most("K1 |R_z(n0) / (P/2) - 1|", (rz[0] / (0.5 * C_P) - 1.0).abs(), 1e-8);
    ch.at_most("K1 |R_z(n1) / (P/2) - 1|", (rz[1] / (0.5 * C_P) - 1.0).abs(), 1e-8);
    ch.at_most("K1 |R_y(n0)| / P", ry0.abs() / C_P, 1e-8);
    ch.at_most("K1 |sum R_x| / P", sx.abs() / C_P, 1e-8);

    // R1: the full clamp beside it.
    let cc = cantilever(Mesh::Tet10, Support::Clamp);
    let (resc, ratio_c) = run_cantilever(&cc, "R1 Tet10 40x2x2 full clamp");
    for (z0, sign, name) in [(C_H, 1.0, "top"), (0.0, -1.0, "bottom")] {
        let line = section_stress(&cc, &resc, z0);
        let mid = line.iter().find(|(y, _)| (y - 0.5 * C_H).abs() <= 1e-12).unwrap().1;
        println!(
            "R1 Tet10 full clamp {name} (h, h/2, {z0}): sigma_xx {mid:.6e}, error {:.4e}",
            rel(mid, sign * sb)
        );
    }
    ch.at_most("R1 Tet10 ratio_clamp - ratio_beam", ratio_c - ratio, 1e-12);
    ch.above("R1 Tet10 ratio_clamp", ratio_c, 0.98);
    ch.finish();
}

#[test]
fn c2_r1_hex8_cantilever_four_through_thickness() {
    let mut ch = Checks::new();
    let c = cantilever(Mesh::Hex8, Support::Beam);
    let (res, ratio) = run_cantilever(&c, "C2 Hex8 80x4x4 beam support");
    ch.at_most("C2 Hex8 |delta / delta_T - 1|", (ratio - 1.0).abs(), 0.05);
    let sb = sigma_beam();
    for (z0, sign, name) in [(C_H, 1.0, "top"), (0.0, -1.0, "bottom")] {
        let line = section_stress(&c, &res, z0);
        let mid = line.iter().find(|(y, _)| (y - 0.5 * C_H).abs() <= 1e-12).unwrap().1;
        println!(
            "record Hex8 beam support {name} (h, h/2, {z0}): sigma_xx {mid:.6e}, error {:.4e}",
            rel(mid, sign * sb)
        );
    }
    let cc = cantilever(Mesh::Hex8, Support::Clamp);
    let (_, ratio_c) = run_cantilever(&cc, "R1 Hex8 80x4x4 full clamp (record)");
    println!("R1 Hex8 ratio_clamp - ratio_beam {:.4e}", ratio_c - ratio);
    ch.finish();
}

#[test]
fn c4_n1_tet4_locking_control_on_the_same_nodes() {
    let mut ch = Checks::new();
    let c = cantilever(Mesh::Tet4, Support::Beam);
    let (_, ratio) = run_cantilever(&c, "C4 Tet4 80x4x4 beam support");
    ch.below("C4 / N1 Tet4 delta / delta_T (locking control)", ratio, 0.9);
    ch.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// The cylinder (ADR 0433 D6 to D8)

/// Lame, plane strain: `u_r(r)`.
fn lame_u(r: f64) -> f64 {
    let a = PRESS * A_IN * A_IN / (B_OUT * B_OUT - A_IN * A_IN);
    let b = a * B_OUT * B_OUT;
    (1.0 + NU) / E_MOD * ((1.0 - 2.0 * NU) * a * r + b / r)
}

/// Lame, plane stress: `u_r(r)` (control N2).
fn lame_u_plane_stress(r: f64) -> f64 {
    let a = PRESS * A_IN * A_IN / (B_OUT * B_OUT - A_IN * A_IN);
    let b = a * B_OUT * B_OUT;
    ((1.0 - NU) * a * r + (1.0 + NU) * b / r) / E_MOD
}

/// Lame: `sigma_theta(a) = A + B / a^2`.
fn lame_hoop_bore() -> f64 {
    let a = PRESS * A_IN * A_IN / (B_OUT * B_OUT - A_IN * A_IN);
    a + a * B_OUT * B_OUT / (A_IN * A_IN)
}

/// The map of D7 from the parameters `(s, t, z)`.
fn polar(p: Vec3, beta: f64) -> Vec3 {
    let r = if beta == 0.0 {
        A_IN + (B_OUT - A_IN) * p[0]
    } else {
        A_IN + (B_OUT - A_IN) * ((beta * p[0]).exp() - 1.0) / (beta.exp() - 1.0)
    };
    let th = 0.5 * PI * p[1];
    [r * th.cos(), r * th.sin(), p[2]]
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Axial {
    /// `u_z = 0` at every node (plane strain).
    PlaneStrain,
    /// N2: `u_z = 0` on `zmin` only.
    Free,
}

fn cylinder_model(mesh: SolidMesh, axial: Axial) -> SolidModel {
    let mut m = model_of(mesh);
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymax", 0, 0.0)).unwrap();
    let set = match axial {
        Axial::PlaneStrain => "all",
        Axial::Free => "zmin",
    };
    m.add_constraint(Constraint::displacement(set, 2, 0.0)).unwrap();
    m.add_load(Load::pressure("xmin", PRESS)).unwrap();
    m
}

fn hex8_cylinder(k: usize, beta: f64) -> SolidMesh {
    let base = hex_box([0.0; 3], [1.0, 1.0, H_AX], [4 * k, 8 * k, 1]).unwrap();
    let nodes = base.nodes().iter().map(|&p| polar(p, beta)).collect();
    rebuild(&base, nodes)
}

fn tet10_cylinder(n_r: usize, n_t: usize) -> SolidMesh {
    let (q, _) = upgrade_to_tet10(&tet_box([0.0; 3], [1.0, 1.0, H_AX], [n_r, n_t, 1]).unwrap()).unwrap();
    let nodes = q.nodes().iter().map(|&p| polar(p, BETA)).collect();
    rebuild(&q, nodes)
}

struct CylinderRun {
    bore_u: f64,
    outer_u: f64,
    bore_u_plane_stress: f64,
    hoop: f64,
    ry: f64,
    rx: f64,
}

fn run_cylinder(model: &SolidModel, tag: &str) -> CylinderRun {
    let t0 = std::time::Instant::now();
    let res = solve_static(model, &StaticOptions::default()).unwrap();
    let mesh = model.mesh();
    let nodes = mesh.nodes();
    let u_r = |n: usize| {
        let (p, u) = (nodes[n], res.displacement(n));
        (u[0] * p[0] + u[1] * p[1]) / p[0].hypot(p[1])
    };
    let bore = mesh.node_set("xmin").unwrap();
    let outer = mesh.node_set("xmax").unwrap();
    let max_over = |set: &[usize], f: &dyn Fn(usize) -> f64| set.iter().map(|&n| f(n)).fold(0.0, f64::max);
    let bore_u = max_over(bore, &|n| rel(u_r(n), lame_u(A_IN)));
    let outer_u = max_over(outer, &|n| rel(u_r(n), lame_u(B_OUT)));
    let bore_u_plane_stress = max_over(bore, &|n| rel(u_r(n), lame_u_plane_stress(A_IN)));
    let field = IntegrationPointField::from_result(model, &res).unwrap();
    let avg = nodal_average(model, &field, field.stress(), None).unwrap();
    let hoop_at = |n: usize| {
        let p = nodes[n];
        let r = p[0].hypot(p[1]);
        let t = [-p[1] / r, p[0] / r];
        let s = avg[n].unwrap();
        s[0] * t[0] * t[0] + s[1] * t[1] * t[1] + 2.0 * s[3] * t[0] * t[1]
    };
    let hoop = max_over(bore, &|n| rel(hoop_at(n), lame_hoop_bore()));
    let ry: f64 = mesh
        .node_set("ymin")
        .unwrap()
        .iter()
        .map(|&n| reaction(&res, n, 1))
        .sum();
    let rx: f64 = mesh
        .node_set("ymax")
        .unwrap()
        .iter()
        .map(|&n| reaction(&res, n, 0))
        .sum();
    let run = CylinderRun {
        bore_u,
        outer_u,
        bore_u_plane_stress,
        hoop,
        ry,
        rx,
    };
    println!(
        "{tag}: {} unknowns, solve {:.2} s; u_r errors bore {:.4e}, outer {:.4e} (bore vs plane stress {:.4e}); \
         bore hoop error {:.4e}; sum R_y(ymin) {:.9e}, sum R_x(ymax) {:.9e}, -p a H {:.9e}",
        res.stats().unwrap().unknowns(),
        t0.elapsed().as_secs_f64(),
        run.bore_u,
        run.outer_u,
        run.bore_u_plane_stress,
        run.hoop,
        run.ry,
        run.rx,
        -PRESS * A_IN * H_AX
    );
    run
}

fn k3_load_totals(ch: &mut Checks, tag: &str, r: &CylinderRun) {
    let pah = PRESS * A_IN * H_AX;
    ch.at_most(
        &format!("K3 {tag} |sum R_y(ymin) / (p a H) + 1|"),
        (r.ry / pah + 1.0).abs(),
        1e-9,
    );
    ch.at_most(
        &format!("K3 {tag} |sum R_x(ymax) / (p a H) + 1|"),
        (r.rx / pah + 1.0).abs(),
        1e-9,
    );
}

#[test]
fn t1_t2_t3_k3_n2_n3_hex8_cylinder() {
    let mut ch = Checks::new();
    let mut e = Vec::new();
    let mut fine = None;
    for k in [1usize, 2, 4, 8] {
        let model = cylinder_model(hex8_cylinder(k, BETA), Axial::PlaneStrain);
        let r = run_cylinder(&model, &format!("Hex8 k={k} ({} x {})", 4 * k, 8 * k));
        e.push(r.bore_u);
        if k == 8 {
            fine = Some(r);
        }
    }
    let orders: Vec<f64> = e.windows(2).map(|w| (w[0] / w[1]).log2()).collect();
    println!("T3 Hex8 bore u_r errors {e:?}, observed orders {orders:?}");
    let r = fine.unwrap();
    ch.at_most("T1 Hex8 k=8 bore |u_r / u_r(a) - 1|", r.bore_u, 0.005);
    ch.at_most("T1 Hex8 k=8 outer |u_r / u_r(b) - 1|", r.outer_u, 0.005);
    ch.at_most("T2 Hex8 k=8 bore |sigma_theta / (5p/3) - 1|", r.hoop, 0.02);
    ch.at_least("T3 Hex8 observed order log2(e_4 / e_8)", orders[2], 1.8);
    k3_load_totals(&mut ch, "Hex8 k=8", &r);

    // N2: no plane-strain constraint.
    let n2 = run_cylinder(
        &cylinder_model(hex8_cylinder(8, BETA), Axial::Free),
        "N2 Hex8 k=8 u_z on zmin only",
    );
    ch.above("N2 bore error against plane-strain Lame", n2.bore_u, 0.02);
    ch.at_most("N2 bore error against plane-stress Lame", n2.bore_u_plane_stress, 0.005);

    // N3: the uniform radius.
    let n3 = run_cylinder(
        &cylinder_model(hex8_cylinder(8, 0.0), Axial::PlaneStrain),
        "N3 Hex8 k=8 uniform radius",
    );
    let q = n3.hoop / r.hoop;
    println!(
        "N3 hoop error uniform {:.4e}, graded {:.4e}, ratio {q:.4}",
        n3.hoop, r.hoop
    );
    ch.holds(
        "N3 e_u / e_T2 in [2, 4.5]",
        (2.0..=4.5).contains(&q),
        &format!("{q:.4}"),
    );
    ch.finish();
}

#[test]
fn t4_k2_k3_curved_tet10_cylinder() {
    let mut ch = Checks::new();
    let mesh = tet10_cylinder(8, 16);
    let mut dr = 0.0f64;
    for (set, radius) in [("xmin", A_IN), ("xmax", B_OUT)] {
        for &n in mesh.node_set(set).unwrap() {
            let p = mesh.nodes()[n];
            dr = dr.max((p[0].hypot(p[1]) - radius).abs());
        }
    }
    ch.at_most("K2 Tet10 wall nodes on their circles, largest |r - R|", dr, 1e-14);
    println!(
        "Tet10 cylinder: {} elements, {} nodes",
        mesh.n_elements(),
        mesh.nodes().len()
    );
    let r = run_cylinder(&cylinder_model(mesh, Axial::PlaneStrain), "T4 Tet10 curved 8 x 16");
    ch.at_most("T4 Tet10 bore |u_r / u_r(a) - 1|", r.bore_u, 0.005);
    ch.at_most("T4 Tet10 outer |u_r / u_r(b) - 1|", r.outer_u, 0.005);
    ch.at_most("T4 Tet10 bore |sigma_theta / (5p/3) - 1|", r.hoop, 0.02);
    k3_load_totals(&mut ch, "Tet10", &r);
    ch.finish();
}
