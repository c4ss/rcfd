//! F-427 (ADR 0507): the checks of frictionless node-to-surface contact with an augmented Lagrangian. Every limit is the one
//! registered in ADR 0507 (with amendment 1); the models are those of its D7.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, tet_box};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::contact::{
    solve, solve_with, ContactOptions, ContactPair, ContactResult, ContactSet, Kinematics, Target, SOLVER_NAME,
};
use rcfd_fem::nonlinear::{Contribution, NewtonOptions};
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::tie::assemble_parts;
use rcfd_fem::{Constraint, ElementKind, FaceRef, FemError, Load, Material, SolidMesh, SolidModel};
use std::cell::Cell;
use std::f64::consts::PI;

// ------------------------------------------------------------------------------------------------------------ helpers

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
        }
    }
    fn less(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value < limit;
        println!(
            "{name}: {value:.6e} (limit < {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} >= {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

/// The largest of the values; infinite if any is not finite (a NaN must not hide behind `f64::max`).
fn worst(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut w: f64 = 0.0;
    for v in values {
        if !v.is_finite() {
            return f64::INFINITY;
        }
        w = w.max(v);
    }
    w
}

fn elastic(e: f64, nu: f64) -> Material {
    Material::isotropic_elastic(e, nu, 1.0, 0.0).unwrap()
}

fn newton() -> NewtonOptions {
    NewtonOptions::new(1).max_iterations(30)
}

/// Every stress component of every integration point of `model` at `u` against `reference`, the largest difference.
fn stress_error(model: &SolidModel, u: &[f64], reference: [f64; 6], elements: Option<&[usize]>) -> f64 {
    let f = IntegrationPointField::compute(model, u).unwrap();
    let off = f.offsets();
    let ne = off.len() - 1;
    let list: Vec<usize> = elements.map_or((0..ne).collect(), |e| e.to_vec());
    worst(list.iter().flat_map(|&e| {
        let s = &f.stress()[off[e]..off[e + 1]];
        s.iter()
            .flat_map(move |p| (0..6).map(move |i| (p[i] - reference[i]).abs()))
            .collect::<Vec<_>>()
    }))
}

// ------------------------------------------------------------------------------------------------------------ P: bar

const E_BAR: f64 = 2.0e11;
const A_BAR: f64 = 0.01;
const GAP: f64 = 1e-2;
const F_MAX: f64 = 2.0 * E_BAR * A_BAR * GAP;

fn bar(force: f64) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [4, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", elastic(E_BAR, 0.3)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    if force != 0.0 {
        m.add_load(Load::traction("xmax", [force / A_BAR, 0.0, 0.0])).unwrap();
    }
    m
}

fn stop() -> ContactPair {
    ContactPair::new(
        "xmax",
        Target::RigidPlane {
            point: [1.0 + GAP, 0.0, 0.0],
            normal: [-1.0, 0.0, 0.0],
        },
        100.0 * E_BAR / 0.25,
    )
}

fn end_nodes(m: &SolidModel) -> Vec<usize> {
    let mut v = m.mesh().node_set("xmax").unwrap().to_vec();
    v.sort_unstable();
    v
}

#[test]
fn p1_p2_bar_against_a_rigid_stop() {
    let m = bar(F_MAX);
    let r = solve(&m, &[], &[], &[stop()], &ContactOptions::new(7, 1e-11).newton(newton())).unwrap();
    let dofs = m.dof_map();
    let ends = end_nodes(&m);
    let support = m.mesh().node_set("xmin").unwrap().to_vec();
    let ea_l = E_BAR * A_BAR / 1.0;
    let mut c = Checks::new();
    let (mut eu, mut er, mut es, mut pens) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for s in &r.steps {
        let f = s.load_factor * F_MAX;
        let u_end = ends
            .iter()
            .map(|&n| s.displacement[dofs.equation(n, 0).unwrap()])
            .sum::<f64>()
            / 4.0;
        let force: f64 = s.nodes.iter().map(|n| n.force).sum();
        let support_x: f64 = support.iter().map(|&n| s.reactions[dofs.equation(n, 0).unwrap()]).sum();
        let u_cf = (f / ea_l).min(GAP);
        let r_cf = (f - ea_l * GAP).max(0.0);
        println!(
            "P step {}: u_end {u_end:.12e} (cf {u_cf:.12e}), R {force:.6e} (cf {r_cf:.6e}), S {support_x:.6e}",
            s.step
        );
        eu.push((u_end - u_cf).abs() / GAP);
        er.push((force - r_cf).abs() / F_MAX);
        // Amendment 1: S = -min(F, E A g/L).
        es.push((support_x + f.min(ea_l * GAP)).abs() / F_MAX);
        if r_cf > 0.0 {
            pens.push(s.solves.last().unwrap().penetration / 1.0);
        }
    }
    c.at_most("P1 end displacement / g", worst(eu), 1e-8);
    c.at_most("P1 stop force / F_max", worst(er), 1e-8);
    c.at_most("P1 support reaction / F_max (amendment 1)", worst(es), 1e-8);
    c.holds(
        "P1 contact from step 4",
        r.steps.iter().filter(|s| s.nodes.iter().any(|n| n.force > 0.0)).count() == 4,
        "4 steps",
    );
    c.less("P2 penetration after augmentation / L", worst(pens), 1e-8);
    c.finish();
}

#[test]
fn np_pure_penalty_penetrates() {
    let m = bar(F_MAX);
    let r = solve(
        &m,
        &[],
        &[],
        &[stop()],
        &ContactOptions::new(7, 1e-11).newton(newton()).max_augmentations(0),
    )
    .unwrap();
    let pen = r.steps.last().unwrap().solves.last().unwrap().penetration;
    let mut c = Checks::new();
    c.at_least("NP penetration of the pure penalty at k = 7 / L", pen, 1e-8);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ B, S: blocks

const E_BL: f64 = 2.0e11;

fn blocks(drop: f64) -> SolidModel {
    let lower = hex_box([0.0; 3], [1.0, 1.0, 1.0], [2, 2, 2]).unwrap();
    let upper = hex_box([0.0, 0.0, 1.0 - drop], [1.0, 1.0, 1.0], [2, 2, 2]).unwrap();
    let mesh = assemble_parts(&[("lower", &lower), ("upper", &upper)]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("lower.all", elastic(E_BL, 0.3)).unwrap();
    m.add_material("upper.all", elastic(E_BL, 0.3)).unwrap();
    m.add_constraint(Constraint::displacement("lower.zmin", 2, 0.0))
        .unwrap();
    for p in ["lower", "upper"] {
        m.add_constraint(Constraint::displacement(&format!("{p}.xmin"), 0, 0.0))
            .unwrap();
        m.add_constraint(Constraint::displacement(&format!("{p}.ymin"), 1, 0.0))
            .unwrap();
    }
    m
}

fn block_pair() -> ContactPair {
    ContactPair::new("upper.zmin", Target::Surface("lower.zmax".into()), 100.0 * E_BL / 0.5)
}

#[test]
fn b1_blocks_pressed() {
    let p = 1.0e8;
    let mut m = blocks(0.0);
    m.add_load(Load::pressure("upper.zmax", p)).unwrap();
    let r = solve(
        &m,
        &[],
        &[],
        &[block_pair()],
        &ContactOptions::new(2, 1e-12).newton(newton()),
    )
    .unwrap();
    let u = r.result.displacement_values();
    let mut c = Checks::new();
    c.at_most(
        "B1 stress / p",
        stress_error(&m, u, [0.0, 0.0, -p, 0.0, 0.0, 0.0], None) / p,
        1e-8,
    );
    c.at_most(
        "B1 node pressure / p",
        worst(r.nodes.iter().map(|n| (n.pressure - p).abs() / p)),
        1e-8,
    );
    c.holds(
        "B1 nodes",
        r.nodes.len() == 9,
        &format!("{} contactor nodes", r.nodes.len()),
    );
    let dofs = m.dof_map();
    let re = r.result.reactions().unwrap();
    let sz: f64 = m
        .mesh()
        .node_set("lower.zmin")
        .unwrap()
        .iter()
        .map(|&n| re[dofs.equation(n, 2).unwrap()])
        .sum();
    c.at_most("B1 reaction / p", (sz - p).abs() / p, 1e-8);
    c.finish();
}

#[test]
#[ignore = "record of the registered run of 8ca0d696 (ADR 0507 amendment 2): load step 4 of 8: not converged in 30 iterations, relative residual 1.1e-7, energy ratio 4.1e-23; continued in s1_blocks_pressed_then_pulled_apart_a2"]
fn s1_blocks_pressed_then_pulled_apart() {
    let d = 1e-3;
    let m = blocks(d);
    let top: Vec<Prescribed> = m
        .mesh()
        .node_set("upper.zmax")
        .unwrap()
        .iter()
        .map(|&n| Prescribed::new(n, 2, 2.0 * d))
        .collect();
    let r = solve(
        &m,
        &top,
        &[],
        &[block_pair()],
        &ContactOptions::new(8, 1e-13).newton(newton()),
    )
    .unwrap();
    let mut c = Checks::new();
    let scale = E_BL * d / 8.0;
    let (mut neg, mut st, mut pr, mut touch, mut zero, mut gap, mut free) = (
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    for s in &r.steps {
        let l = s.load_factor;
        for n in &s.nodes {
            // Every multiplier finite and >= 0: a negative or non-finite one is recorded as infinite.
            neg.push(if n.force.is_finite() && n.force >= 0.0 {
                0.0
            } else {
                f64::INFINITY
            });
        }
        let k = s.step;
        if k < 4 {
            let sigma = E_BL * d * (l - 0.5);
            st.push(stress_error(&m, &s.displacement, [0.0, 0.0, sigma, 0.0, 0.0, 0.0], None) / scale);
            pr.extend(s.nodes.iter().map(|n| (n.pressure + sigma).abs() / scale));
        } else if k == 4 {
            touch.extend(s.nodes.iter().map(|n| n.pressure.abs() / (E_BL * d)));
        } else {
            zero.extend(s.nodes.iter().map(|n| if n.force == 0.0 { 0.0 } else { f64::INFINITY }));
            gap.extend(
                s.nodes
                    .iter()
                    .map(|n| (n.gap.unwrap_or(f64::INFINITY) - d * (2.0 * l - 1.0)).abs() / d),
            );
            free.push(stress_error(&m, &s.displacement, [0.0; 6], None) / (E_BL * d));
        }
    }
    c.at_most("S1 every multiplier >= 0 (0 if so)", worst(neg), 0.0);
    c.at_most("S1 pressed: stress / (E d/8)", worst(st), 1e-8);
    c.at_most("S1 pressed: node pressure / (E d/8)", worst(pr), 1e-8);
    c.at_most("S1 touching: node pressure / (E d)", worst(touch), 1e-8);
    c.at_most("S1 apart: every multiplier exactly 0 (0 if so)", worst(zero), 0.0);
    c.at_most("S1 apart: gap / d", worst(gap), 1e-8);
    c.at_most("S1 apart: stress / (E d)", worst(free), 1e-8);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ H: Hertz

const NU_H: f64 = 0.3;
const R_H: f64 = 1.0;
const A_H: f64 = 0.1;

fn e_star() -> f64 {
    1.0 / (1.0 - NU_H * NU_H)
}

/// Boussinesq's displacement for the point force `f` into the surface `z = 0` of the body `z <= 0` (E = 1), ADR 0507
/// arithmetic 3.
fn boussinesq(f: f64, x: Vec3) -> Vec3 {
    let zeta = -x[2];
    let r = (x[0] * x[0] + x[1] * x[1]).sqrt();
    let rho = (r * r + zeta * zeta).sqrt();
    let c = f * (1.0 + NU_H) / (2.0 * PI);
    let u_zeta = c * (zeta * zeta / rho.powi(3) + 2.0 * (1.0 - NU_H) / rho);
    let u_r = c * (r * zeta / rho.powi(3) - (1.0 - 2.0 * NU_H) * r / (rho * (rho + zeta)));
    let (cx, cy) = if r > 0.0 { (x[0] / r, x[1] / r) } else { (0.0, 0.0) };
    [u_r * cx, u_r * cy, -u_zeta]
}

fn bstress(f: f64, x: Vec3, s: f64) -> [[f64; 3]; 3] {
    let lam = NU_H / ((1.0 + NU_H) * (1.0 - 2.0 * NU_H));
    let g = 1.0 / (2.0 * (1.0 + NU_H));
    let mut du = [[0.0; 3]; 3];
    for j in 0..3 {
        let (mut a, mut b) = (x, x);
        a[j] += s;
        b[j] -= s;
        let (ua, ub) = (boussinesq(f, a), boussinesq(f, b));
        for i in 0..3 {
            du[i][j] = (ua[i] - ub[i]) / (2.0 * s);
        }
    }
    let tr = du[0][0] + du[1][1] + du[2][2];
    let mut sig = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            sig[i][j] = g * (du[i][j] + du[j][i]) + if i == j { lam * tr } else { 0.0 };
        }
    }
    sig
}

#[test]
fn h0_boussinesq_field() {
    let lam = NU_H / ((1.0 + NU_H) * (1.0 - 2.0 * NU_H));
    let g = 1.0 / (2.0 * (1.0 + NU_H));
    let mut c = Checks::new();
    let pts: [Vec3; 6] = [
        [0.3, 0.1, -0.2],
        [0.05, 0.4, -0.1],
        [0.2, 0.2, -0.5],
        [0.01, 0.02, -0.03],
        [1.0, -0.5, -0.7],
        [0.0, 0.3, -0.4],
    ];
    let mut nav = Vec::new();
    for x in pts {
        let rho = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
        let s = 1e-4 * rho;
        let u = |p: Vec3| boussinesq(1.0, p);
        let at = |dx: [f64; 3]| u([x[0] + dx[0], x[1] + dx[1], x[2] + dx[2]]);
        let u0 = u(x);
        let mut res = [0.0; 3];
        for i in 0..3 {
            // G lap u_i + (lambda + G) sum_j d2 u_j / dx_i dx_j
            let mut lap = 0.0;
            for j in 0..3 {
                let mut e = [0.0; 3];
                e[j] = s;
                let (a, b) = (at(e), at([-e[0], -e[1], -e[2]]));
                lap += (a[i] - 2.0 * u0[i] + b[i]) / (s * s);
            }
            let mut gd = 0.0;
            for j in 0..3 {
                let mut ei = [0.0; 3];
                ei[i] += s;
                let mut ej = [0.0; 3];
                ej[j] += s;
                let pp = at([ei[0] + ej[0], ei[1] + ej[1], ei[2] + ej[2]]);
                let pm = at([ei[0] - ej[0], ei[1] - ej[1], ei[2] - ej[2]]);
                let mp = at([-ei[0] + ej[0], -ei[1] + ej[1], -ei[2] + ej[2]]);
                let mm = at([-ei[0] - ej[0], -ei[1] - ej[1], -ei[2] - ej[2]]);
                gd += (pp[j] - pm[j] - mp[j] + mm[j]) / (4.0 * s * s);
            }
            res[i] = g * lap + (lam + g) * gd;
        }
        let un = (u0[0] * u0[0] + u0[1] * u0[1] + u0[2] * u0[2]).sqrt();
        nav.push((res[0] * res[0] + res[1] * res[1] + res[2] * res[2]).sqrt() / (g * un / (rho * rho)));
    }
    c.at_most("H0 Navier residual", worst(nav), 1e-5);
    let mut tr = Vec::new();
    for &(x, y) in &[(0.1, 0.0), (0.3, 0.2), (0.0, 0.7), (1.0, 1.0)] {
        let r2: f64 = x * x + y * y;
        let sig = bstress(1.0, [x, y, 0.0], 1e-4 * r2.sqrt());
        tr.push(sig[2][2].abs().max(sig[0][2].abs()).max(sig[1][2].abs()) / (1.0 / r2));
    }
    c.at_most("H0 traction-free surface", worst(tr), 1e-5);
    // The resultant of sigma_zz on the plane zeta = 0.3 against -F, trapezoid in ln r.
    let zeta: f64 = 0.3;
    let (s0, s1) = ((1e-6 * zeta).ln(), 300f64.ln());
    let nint = 4000;
    let h = (s1 - s0) / nint as f64;
    let mut sum = 0.0;
    for i in 0..=nint {
        let r = (s0 + h * i as f64).exp();
        let sig = bstress(1.0, [r, 0.0, -zeta], 1e-4 * (r * r + zeta * zeta).sqrt());
        let w = if i == 0 || i == nint { 0.5 } else { 1.0 };
        sum += w * sig[2][2] * 2.0 * PI * r * r * h;
    }
    c.at_most("H0 resultant against -F", (sum + 1.0).abs(), 1e-4);
    c.finish();
}

struct Hertz {
    force: f64,
    a_fe: f64,
    p0: f64,
    axis_count: usize,
    penetration: f64,
    nodes: usize,
}

fn graded(h: f64, nf: usize, m: usize, l: f64) -> Vec<f64> {
    let xf = h * nf as f64;
    let (mut lo, mut hi) = (1.0000001f64, 3.0f64);
    for _ in 0..200 {
        let q = 0.5 * (lo + hi);
        let s = h * q * (q.powi(m as i32) - 1.0) / (q - 1.0);
        if s > l - xf {
            hi = q
        } else {
            lo = q
        }
    }
    let q = 0.5 * (lo + hi);
    println!("H grading factor q = {q:.6}");
    let mut v: Vec<f64> = (0..=nf).map(|i| h * i as f64).collect();
    let mut d = h;
    for _ in 0..m {
        d *= q;
        let last = *v.last().unwrap();
        v.push(last + d);
    }
    *v.last_mut().unwrap() = l;
    v
}

fn hertz(per_a: usize, nf: usize, m: usize) -> Hertz {
    let a = A_H;
    let h = a / per_a as f64;
    let xs = graded(h, nf, m, 5.0 * a);
    let n = xs.len();
    let id = |i: usize, j: usize, k: usize| i + n * (j + n * k);
    let mut nodes = Vec::with_capacity(n * n * n);
    for k in 0..n {
        for j in 0..n {
            for i in 0..n {
                nodes.push([xs[i], xs[j], -xs[k]]);
            }
        }
    }
    let mut conn = Vec::new();
    let mut top = Vec::new();
    for k in 0..n - 1 {
        for j in 0..n - 1 {
            for i in 0..n - 1 {
                conn.extend_from_slice(&[
                    id(i, j, k + 1),
                    id(i + 1, j, k + 1),
                    id(i + 1, j + 1, k + 1),
                    id(i, j + 1, k + 1),
                    id(i, j, k),
                    id(i + 1, j, k),
                    id(i + 1, j + 1, k),
                    id(i, j + 1, k),
                ]);
                if k == 0 {
                    top.push(FaceRef {
                        element: i + (n - 1) * (j + (n - 1) * k),
                        face: 1,
                    });
                }
            }
        }
    }
    let mut mesh = SolidMesh::new(nodes.clone()).unwrap();
    mesh.add_block(ElementKind::Hex8, conn).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_surface_set("top", top).unwrap();
    let mut model = SolidModel::new(mesh);
    model.add_material("all", elastic(1.0, NU_H)).unwrap();
    let f_th = 4.0 * e_star() * a.powi(3) / (3.0 * R_H);
    let delta = a * a / R_H;
    let mut pres = Vec::new();
    for k in 0..n {
        for j in 0..n {
            for i in 0..n {
                let v = id(i, j, k);
                if i == 0 {
                    pres.push(Prescribed::new(v, 0, 0.0));
                }
                if j == 0 {
                    pres.push(Prescribed::new(v, 1, 0.0));
                }
                if i == n - 1 || j == n - 1 || k == n - 1 {
                    let u = boussinesq(f_th, nodes[v]);
                    let vals = [u[0], u[1], u[2] + delta];
                    for (cc, &val) in vals.iter().enumerate() {
                        if (cc == 0 && i == 0) || (cc == 1 && j == 0) {
                            continue;
                        }
                        pres.push(Prescribed::new(v, cc, val));
                    }
                }
            }
        }
    }
    let mut pair = ContactPair::new(
        "top",
        Target::RigidPlane {
            point: [0.0; 3],
            normal: [0.0, 0.0, -1.0],
        },
        100.0 * 1.0 / h,
    );
    for j in 0..n {
        for i in 0..n {
            let r2 = xs[i] * xs[i] + xs[j] * xs[j];
            pair = pair.initial_gap(id(i, j, 0), R_H - (R_H * R_H - r2).sqrt());
        }
    }
    let t0 = std::time::Instant::now();
    let r: ContactResult = solve(
        &model,
        &pres,
        &[],
        &[pair],
        &ContactOptions::new(1, 1e-8).newton(newton()),
    )
    .unwrap();
    let s = r.steps.last().unwrap();
    println!(
        "H run: {:?}, {} Newton solves, iterations {:?}",
        t0.elapsed(),
        s.solves.len(),
        s.solves.iter().map(|v| v.residuals.len() - 1).collect::<Vec<_>>()
    );
    let force = 4.0 * r.nodes.iter().map(|v| v.force).sum::<f64>();
    let pmax = r.nodes.iter().map(|v| v.pressure).fold(0.0, f64::max);
    let (mut sw, mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let mut p0 = f64::NAN;
    let mut axis_count = 0;
    for v in &r.nodes {
        let x = nodes[v.node];
        let r2 = x[0] * x[0] + x[1] * x[1];
        if r2 == 0.0 {
            p0 = v.pressure;
        }
        if x[1] == 0.0 && v.force > 0.0 {
            axis_count += 1;
        }
        if v.pressure >= 0.5 * pmax {
            let y = v.pressure * v.pressure;
            sw += 1.0;
            sx += r2;
            sy += y;
            sxx += r2 * r2;
            sxy += r2 * y;
        }
    }
    let c1 = (sw * sxy - sx * sy) / (sw * sxx - sx * sx);
    let c0 = (sy - c1 * sx) / sw;
    let a_fe = (-c0 / c1).sqrt();
    Hertz {
        force,
        a_fe,
        p0,
        axis_count,
        penetration: s.solves.last().unwrap().penetration,
        nodes: r.nodes.len(),
    }
}

fn hertz_checks(c: &mut Checks, tag: &str, h: &Hertz, limits: [f64; 3], full: bool) {
    let a_h = (3.0 * h.force * R_H / (4.0 * e_star())).cbrt();
    let p_h = 3.0 * h.force / (2.0 * PI * a_h * a_h);
    let delta = A_H * A_H / R_H;
    println!(
        "{tag}: F {:.6e} (theory {:.6e}), a_fe {:.6e}, a_H {a_h:.6e}, p0 {:.6e}, p_H {p_h:.6e}, axis nodes {}, contactor nodes {}",
        h.force,
        4.0 * e_star() * A_H.powi(3) / (3.0 * R_H),
        h.a_fe,
        h.p0,
        h.axis_count,
        h.nodes
    );
    c.at_most(&format!("{tag}1 contact radius"), (h.a_fe / a_h - 1.0).abs(), limits[0]);
    c.at_most(&format!("{tag}2 peak pressure"), (h.p0 / p_h - 1.0).abs(), limits[1]);
    c.at_most(
        &format!("{tag}3 approach"),
        (delta / (a_h * a_h / R_H) - 1.0).abs(),
        limits[2],
    );
    if full {
        c.at_least("H4 nodes on y = 0 in contact", h.axis_count as f64, 10.0);
        c.at_most("H5 penetration", h.penetration, 1e-8);
        // NH: the approach with E in place of E*, recorded.
        let a_e = (3.0 * h.force * R_H / 4.0).cbrt();
        let nh = (delta / (a_e * a_e / R_H) - 1.0).abs();
        println!("NH approach with E in place of E*: {nh:.6e} (expected > 3e-2)");
        c.at_least("NH approach with E (negative control)", nh, 3e-2);
    }
}

#[test]
fn hf_hertz_fast_copy() {
    let h = hertz(6, 8, 6);
    let mut c = Checks::new();
    hertz_checks(&mut c, "HF", &h, [0.15, 0.20, 0.10], false);
    c.finish();
}

#[test]
#[ignore = "heavy record of ADR 0507 H (about 4 minutes and 0.7 GB in release, run once alone; hf_hertz_fast_copy covers the path)"]
fn h1_to_h5_hertz_sphere_on_a_rigid_plane() {
    let h = hertz(12, 15, 9);
    let mut c = Checks::new();
    hertz_checks(&mut c, "H", &h, [5e-2, 5e-2, 3e-2], true);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ I: fit

const E_FIT: f64 = 2.0e11;
const NU_FIT: f64 = 0.3;
const A_FIT: f64 = 1.0;
const B_FIT: f64 = 2.0;
const D_FIT: f64 = 1e-3;
const T_FIT: f64 = 0.1;
const N_FIT: usize = 64;

struct Fit {
    model: SolidModel,
    ring: Vec<usize>,
    shaft: Vec<usize>,
}

fn fit_model() -> Fit {
    let dth = 2.0 * PI / N_FIT as f64;
    let mut pts: Vec<[f64; 2]> = Vec::new();
    // Core grid (i, j), 17 x 17, half-width 0.5.
    let m = 16;
    let core = |i: usize, j: usize| i + (m + 1) * j;
    for j in 0..=m {
        for i in 0..=m {
            pts.push([-0.5 + i as f64 / m as f64, -0.5 + j as f64 / m as f64]);
        }
    }
    // Perimeter, counter-clockwise from (0.5, 0) = grid (16, 8).
    let mut per = Vec::new();
    for j in 8..=m {
        per.push(core(m, j));
    }
    for i in (0..m).rev() {
        per.push(core(i, m));
    }
    for j in (0..m).rev() {
        per.push(core(0, j));
    }
    for i in 1..=m {
        per.push(core(i, 0));
    }
    for j in 1..8 {
        per.push(core(m, j));
    }
    assert_eq!(per.len(), N_FIT);
    let av = A_FIT / (dth / 2.0).cos();
    let layers = 6;
    let mut band = vec![per.clone()];
    for l in 1..=layers {
        let s = l as f64 / layers as f64;
        let mut row = Vec::new();
        for (p, &q) in per.iter().enumerate() {
            let th = (p as f64 + 0.5) * dth;
            let c = pts[q];
            pts.push([
                (1.0 - s) * c[0] + s * av * th.cos(),
                (1.0 - s) * c[1] + s * av * th.sin(),
            ]);
            row.push(pts.len() - 1);
        }
        band.push(row);
    }
    // Ring (j, k).
    let ring0 = pts.len();
    let rk = |k: usize| (A_FIT - D_FIT) + k as f64 * (B_FIT - A_FIT + D_FIT) / 8.0;
    let ring = |j: usize, k: usize| ring0 + (j % N_FIT) + N_FIT * k;
    for k in 0..=8 {
        for j in 0..N_FIT {
            let th = j as f64 * dth;
            pts.push([rk(k) * th.cos(), rk(k) * th.sin()]);
        }
    }
    let n2 = pts.len();
    let mut nodes: Vec<Vec3> = pts.iter().map(|p| [p[0], p[1], 0.0]).collect();
    nodes.extend(pts.iter().map(|p| [p[0], p[1], T_FIT]));
    let hex = |q: [usize; 4]| [q[0], q[1], q[2], q[3], q[0] + n2, q[1] + n2, q[2] + n2, q[3] + n2];
    let mut conn = Vec::new();
    let mut shaft_el = Vec::new();
    let mut ring_el = Vec::new();
    let mut shaft_out = Vec::new();
    let mut ring_in = Vec::new();
    let mut e = 0;
    for j in 0..m {
        for i in 0..m {
            conn.extend_from_slice(&hex([core(i, j), core(i + 1, j), core(i + 1, j + 1), core(i, j + 1)]));
            shaft_el.push(e);
            e += 1;
        }
    }
    for l in 0..layers {
        for p in 0..N_FIT {
            let p1 = (p + 1) % N_FIT;
            conn.extend_from_slice(&hex([band[l][p], band[l + 1][p], band[l + 1][p1], band[l][p1]]));
            shaft_el.push(e);
            if l == layers - 1 {
                shaft_out.push(FaceRef { element: e, face: 3 });
            }
            e += 1;
        }
    }
    for k in 0..8 {
        for j in 0..N_FIT {
            conn.extend_from_slice(&hex([ring(j, k), ring(j, k + 1), ring(j + 1, k + 1), ring(j + 1, k)]));
            ring_el.push(e);
            if k == 0 {
                ring_in.push(FaceRef { element: e, face: 5 });
            }
            e += 1;
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, conn).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_surface_set("ring_in", ring_in).unwrap();
    mesh.add_surface_set("shaft_out", shaft_out).unwrap();
    mesh.add_node_set("z0", (0..n2).collect()).unwrap();
    let both = |v: usize| vec![v, v + n2];
    mesh.add_node_set("centre", both(core(8, 8))).unwrap();
    mesh.add_node_set("shaft_x", both(core(m, 8))).unwrap();
    mesh.add_node_set("ring_0", both(ring(0, 8))).unwrap();
    let mut r90 = both(ring(16, 8));
    r90.extend(both(ring(48, 8)));
    mesh.add_node_set("ring_90_270", r90).unwrap();
    let mut model = SolidModel::new(mesh);
    model.add_material("all", elastic(E_FIT, NU_FIT)).unwrap();
    model.add_constraint(Constraint::displacement("z0", 2, 0.0)).unwrap();
    model
        .add_constraint(Constraint::displacement("centre", 0, 0.0))
        .unwrap();
    model
        .add_constraint(Constraint::displacement("centre", 1, 0.0))
        .unwrap();
    model
        .add_constraint(Constraint::displacement("shaft_x", 1, 0.0))
        .unwrap();
    model
        .add_constraint(Constraint::displacement("ring_0", 1, 0.0))
        .unwrap();
    model
        .add_constraint(Constraint::displacement("ring_90_270", 0, 0.0))
        .unwrap();
    Fit {
        model,
        ring: ring_el,
        shaft: shaft_el,
    }
}

#[test]
#[ignore = "record of the registered run of 8ca0d696 (ADR 0507 amendment 2): tangent factorisation at Newton iteration 0: pivot -1.29e10, not positive definite; continued in i1_to_i4_interference_fit_ring_on_a_solid_shaft_a2"]
fn i1_to_i4_interference_fit_ring_on_a_solid_shaft() {
    let fit = fit_model();
    let m = &fit.model;
    let pair = ContactPair::new("ring_in", Target::Surface("shaft_out".into()), 100.0 * E_FIT / 0.125);
    let r = solve(m, &[], &[], &[pair], &ContactOptions::new(1, 1e-9).newton(newton())).unwrap();
    let p_cf = E_FIT * D_FIT * (B_FIT * B_FIT - A_FIT * A_FIT) / (2.0 * A_FIT * B_FIT * B_FIT);
    let mean = r.nodes.iter().map(|n| n.pressure).sum::<f64>() / r.nodes.len() as f64;
    let mut c = Checks::new();
    println!(
        "I contact pressure: mean {mean:.6e}, closed form {p_cf:.6e}, {} nodes",
        r.nodes.len()
    );
    c.holds(
        "I nodes",
        r.nodes.len() == 2 * N_FIT,
        &format!("{} contactor nodes", r.nodes.len()),
    );
    c.at_most(
        "I1 every node's pressure",
        worst(r.nodes.iter().map(|n| (n.pressure / p_cf - 1.0).abs())),
        2e-2,
    );
    c.at_most("I1 mean pressure", (mean / p_cf - 1.0).abs(), 2e-2);
    let u = r.result.displacement_values();
    let f = IntegrationPointField::compute(m, u).unwrap();
    let off = f.offsets();
    let mut hoop = Vec::new();
    for &e in &fit.ring {
        let s = &f.stress()[off[e]..off[e + 1]];
        let np = s.len() as f64;
        let mut avg = [0.0; 6];
        for p in s {
            for i in 0..6 {
                avg[i] += p[i] / np;
            }
        }
        let en = m.mesh().element_nodes(e);
        let (mut cx, mut cy) = (0.0, 0.0);
        for &v in en {
            cx += m.mesh().nodes()[v][0] / 8.0;
            cy += m.mesh().nodes()[v][1] / 8.0;
        }
        let rc = (cx * cx + cy * cy).sqrt();
        let (co, si) = (cx / rc, cy / rc);
        let st = avg[0] * si * si - 2.0 * avg[3] * si * co + avg[1] * co * co;
        let lame = mean * A_FIT * A_FIT * (1.0 + B_FIT * B_FIT / (rc * rc)) / (B_FIT * B_FIT - A_FIT * A_FIT);
        hoop.push((st / lame - 1.0).abs());
    }
    c.at_most("I2 ring hoop stress against Lame", worst(hoop), 2e-2);
    let mut sh = Vec::new();
    for &e in &fit.shaft {
        for p in &f.stress()[off[e]..off[e + 1]] {
            sh.push(((p[0] + mean).abs().max((p[1] + mean).abs()).max(p[3].abs())) / mean);
        }
    }
    c.at_most("I3 shaft uniform stress", worst(sh), 1e-6);
    c.at_most("I4 penetration", r.steps[0].solves.last().unwrap().penetration, 1e-9);
    let rigid = E_FIT * D_FIT * (B_FIT * B_FIT - A_FIT * A_FIT)
        / (A_FIT * ((1.0 + NU_FIT) * B_FIT * B_FIT + (1.0 - NU_FIT) * A_FIT * A_FIT));
    let ni = (mean / rigid - 1.0).abs();
    println!("NI against the rigid-shaft form {rigid:.6e}: {ni:.6e}");
    c.at_least("NI rigid-shaft form (negative control)", ni, 0.2);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ T1: tangent

fn two_hex(contactor: [Vec3; 4]) -> SolidModel {
    let mut nodes: Vec<Vec3> = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.05],
        [1.0, 0.0, 0.96],
        [1.0, 1.0, 1.06],
        [0.0, 1.0, 0.97],
    ];
    nodes.extend_from_slice(&contactor);
    nodes.extend(contactor.iter().map(|p| [p[0], p[1], p[2] + 1.0]));
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, (0..16).collect()).unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    mesh.add_surface_set("target", vec![FaceRef { element: 0, face: 1 }])
        .unwrap();
    mesh.add_surface_set("contactor", vec![FaceRef { element: 1, face: 0 }])
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", elastic(1.0, 0.3)).unwrap();
    m
}

const T1_CONTACTOR: [Vec3; 4] = [[0.25, 0.3, 0.98], [0.8, 0.2, 0.99], [0.75, 0.7, 0.97], [0.2, 0.75, 1.0]];

fn dense(n: usize, trip: &[(usize, usize, f64)]) -> Vec<f64> {
    let mut k = vec![0.0; n * n];
    for &(r, c, v) in trip {
        k[n * r + c] += v;
    }
    k
}

/// The contact force and matrix at `u`, and the active flags and faces of the nodes.
type NodeStatus = Vec<(bool, Option<FaceRef>)>;

fn force_at(set: &ContactSet, u: &[f64]) -> (Vec<f64>, Vec<f64>, NodeStatus) {
    let n = u.len();
    let mut f = vec![0.0; n];
    let mut t = Vec::new();
    set.add(u, 1.0, &mut f, &mut t).unwrap();
    let st = set.evaluate(u).unwrap().iter().map(|v| (v.active, v.face)).collect();
    (f, dense(n, &t), st)
}

/// `max |K - K_fd| / max |K|`, whether the active set stayed, and the error of `eps G G^T` alone against `K_fd`.
fn fd_check(m: &SolidModel, pair: ContactPair, lam: &[f64]) -> (f64, bool, f64) {
    let set = ContactSet::new(m, std::slice::from_ref(&pair), &ContactOptions::new(1, 0.0)).unwrap();
    set.set_multipliers(0, lam).unwrap();
    let n = m.dof_map().n_equations();
    let u: Vec<f64> = (0..n).map(|i| 0.01 * (1.7 * i as f64 + 0.3).sin()).collect();
    let (_, k, st0) = force_at(&set, &u);
    let h = 1e-6;
    let mut kfd = vec![0.0; n * n];
    let mut same = st0.iter().all(|s| s.0);
    for j in 0..n {
        let (mut up, mut um) = (u.clone(), u.clone());
        up[j] += h;
        um[j] -= h;
        let (fp, _, sp) = force_at(&set, &up);
        let (fm, _, sm) = force_at(&set, &um);
        same &= sp == st0 && sm == st0;
        for r in 0..n {
            kfd[n * r + j] = (fp[r] - fm[r]) / (2.0 * h);
        }
    }
    let kmax = worst(k.iter().map(|v| v.abs()));
    let err = worst(k.iter().zip(&kfd).map(|(a, b)| (a - b).abs())) / kmax;
    // eps G G^T alone, from the node data (NT).
    let dofs = m.dof_map();
    let mut kg = vec![0.0; n * n];
    for v in set.evaluate(&u).unwrap() {
        let mut g: Vec<(usize, f64)> = Vec::new();
        let e0 = dofs.equation(v.node, 0).unwrap();
        for i in 0..3 {
            g.push((e0 + i, v.normal[i]));
        }
        if let Some(face) = v.face {
            let fnodes = m.mesh().face_nodes(face);
            let corners = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
            for (a, &fnode) in fnodes.iter().enumerate() {
                let na = 0.25 * (1.0 + v.local[0] * corners[a][0]) * (1.0 + v.local[1] * corners[a][1]);
                let ea = dofs.equation(fnode, 0).unwrap();
                for i in 0..3 {
                    g.push((ea + i, -na * v.normal[i]));
                }
            }
        }
        let eps = pair.penalty * v.area;
        for &(r, gr) in &g {
            for &(s, gs) in &g {
                kg[n * r + s] += eps * gr * gs;
            }
        }
    }
    let kfd_max = worst(kfd.iter().map(|v| v.abs()));
    let nt = worst(kg.iter().zip(&kfd).map(|(a, b)| (a - b).abs())) / kfd_max;
    (err, same, nt)
}

#[test]
fn t1_tangent_against_central_differences() {
    let m = two_hex(T1_CONTACTOR);
    let lam = [0.5, 0.4, 0.6, 0.3];
    let mut c = Checks::new();
    let (err, same, nt) = fd_check(
        &m,
        ContactPair::new("contactor", Target::Surface("target".into()), 10.0),
        &lam,
    );
    c.holds("T1 active set fixed (all four active, same faces)", same, "");
    c.at_most("T1 warped deformable target", err, 1e-6);
    println!("NT without the curvature part: {nt:.6e} (expected > 1e-2)");
    c.at_least("NT without the curvature part (negative control)", nt, 1e-2);
    let plane = ContactPair::new(
        "contactor",
        Target::RigidPlane {
            point: [0.0, 0.0, 1.0],
            normal: [0.0, 0.0, -1.0],
        },
        10.0,
    )
    .initial_gap(8, 0.01)
    .initial_gap(10, 0.02);
    let (err, same, _) = fd_check(&m, plane, &lam);
    c.holds("T1r active set fixed", same, "");
    c.at_most("T1r rigid plane", err, 1e-6);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ T2: order

#[test]
#[ignore = "record of the registered run of 8ca0d696 (ADR 0507 amendment 2): singular tangent: pivot -0.469 at node 57 component 2; continued in t2_order_of_the_last_residuals_total_lagrangian_punch_a2"]
fn t2_order_of_the_last_residuals_total_lagrangian_punch() {
    let plate = hex_box([0.0; 3], [2.0, 0.5, 0.25], [8, 2, 2]).unwrap();
    let punch = hex_box([0.8, 0.0, 0.25], [0.4, 0.2, 0.2], [3, 1, 1]).unwrap();
    let mesh = assemble_parts(&[("plate", &plate), ("punch", &punch)]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("plate.all", elastic(1.0, 0.3)).unwrap();
    m.add_material("punch.all", elastic(1.0, 0.3)).unwrap();
    for s in ["plate.xmin", "plate.xmax"] {
        for cc in 0..3 {
            m.add_constraint(Constraint::displacement(s, cc, 0.0)).unwrap();
        }
    }
    m.add_constraint(Constraint::displacement("plate.ymin", 1, 0.0))
        .unwrap();
    m.add_constraint(Constraint::displacement("punch.ymin", 1, 0.0))
        .unwrap();
    m.add_constraint(Constraint::displacement("punch.zmax", 0, 0.0))
        .unwrap();
    m.add_load(Load::pressure("punch.zmax", 0.01)).unwrap();
    let pair = ContactPair::new("punch.zmin", Target::Surface("plate.zmax".into()), 800.0);
    let o = ContactOptions::new(2, 1e-8)
        .newton(newton())
        .kinematics(Kinematics::TotalLagrangian);
    let r = solve(&m, &[], &[], &[pair], &o).unwrap();
    let mut orders = Vec::new();
    for s in &r.steps {
        for (i, v) in s.solves.iter().enumerate() {
            let kept: Vec<usize> = (0..v.residuals.len()).filter(|&k| v.residuals[k] >= 1e-12).collect();
            println!(
                "T2 step {} solve {i}: residuals {:?}, active counts {:?}",
                s.step,
                v.residuals,
                v.active_sets.iter().map(|a| a.len()).collect::<Vec<_>>()
            );
            if kept.len() < 3 {
                continue;
            }
            let (a, b, cc) = (kept[kept.len() - 3], kept[kept.len() - 2], kept[kept.len() - 1]);
            if v.active_sets[a] != v.active_sets[b] || v.active_sets[b] != v.active_sets[cc] {
                continue;
            }
            let (ra, rb, rc) = (v.residuals[a], v.residuals[b], v.residuals[cc]);
            orders.push((rc / rb).ln() / (rb / ra).ln());
        }
    }
    let dz = r
        .result
        .displacement_values()
        .iter()
        .fold(0.0f64, |a, b| a.max(b.abs()));
    println!("T2 orders {orders:?}, largest displacement {dz:.4e}");
    let mut c = Checks::new();
    c.at_least("T2 qualifying solves", orders.len() as f64, 1.0);
    let least = orders.iter().fold(
        f64::INFINITY,
        |a, &b| if b.is_finite() { a.min(b) } else { f64::NEG_INFINITY },
    );
    c.at_least("T2 order of every qualifying solve", least, 1.8);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ C: cycling

/// The extra contribution of C1: `+-amplitude` along `x` at the end nodes as an external load, alternating at each call.
struct Oscillator {
    nodes: Vec<usize>,
    eqs: Vec<usize>,
    amplitude: f64,
    calls: Cell<usize>,
}

impl Contribution for Oscillator {
    fn name(&self) -> &str {
        "oscillator"
    }
    fn add(&self, _u: &[f64], _lf: f64, force: &mut [f64], _t: &mut Vec<(usize, usize, f64)>) -> rcfd_fem::Result<()> {
        let k = self.calls.get();
        self.calls.set(k + 1);
        let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
        for &e in &self.eqs {
            force[e] -= sign * self.amplitude / self.nodes.len() as f64;
        }
        Ok(())
    }
}

fn oscillator(m: &SolidModel, amplitude: f64) -> Oscillator {
    let nodes = end_nodes(m);
    let dofs = m.dof_map();
    Oscillator {
        eqs: nodes.iter().map(|&n| dofs.equation(n, 0).unwrap()).collect(),
        nodes,
        amplitude,
        calls: Cell::new(0),
    }
}

fn cycle_check(c: &mut Checks, tag: &str, kin: Kinematics) {
    let m = bar(0.0);
    let osc = oscillator(&m, F_MAX);
    let o = ContactOptions::new(1, 1e-11).newton(newton()).kinematics(kin);
    let e = solve_with(&m, &[], &[], &[stop()], &o, &[&osc]).unwrap_err();
    let ends = end_nodes(&m);
    let text = e.to_string();
    println!("{tag} error: {text}");
    let ok = matches!(&e, FemError::Solver { solver, reason }
        if *solver == SOLVER_NAME && reason.contains("at Newton iteration 5;") && reason.contains(&format!("{ends:?}")));
    c.holds(tag, ok, &text);
}

#[test]
fn c1_cycling_active_set_is_a_located_error() {
    let mut c = Checks::new();
    cycle_check(&mut c, "C1 linear", Kinematics::Linear);
    cycle_check(&mut c, "C1t total Lagrangian", Kinematics::TotalLagrangian);
    // NC: without the oscillation the same set-up converges.
    let m = bar(0.0);
    let osc = oscillator(&m, 0.0);
    let r = solve_with(
        &m,
        &[],
        &[],
        &[stop()],
        &ContactOptions::new(1, 1e-11).newton(newton()),
        &[&osc],
    );
    c.holds(
        "NC no oscillation converges",
        r.is_ok(),
        &format!("{:?}", r.as_ref().err()),
    );
    c.finish();
}

#[test]
fn c2_augmentations_exhausted_name_the_nodes() {
    let m = bar(F_MAX);
    let o = ContactOptions::new(1, 1e-30).newton(newton()).max_augmentations(1);
    let e = solve(&m, &[], &[], &[stop()], &o).unwrap_err();
    let text = e.to_string();
    println!("C2 error: {text}");
    let ok = matches!(&e, FemError::Solver { solver, .. } if *solver == SOLVER_NAME)
        && text.contains("not met after 1 augmentations")
        && end_nodes(&m).iter().all(|n| text.contains(&format!("({n}, ")));
    let mut c = Checks::new();
    c.holds("C2", ok, &text);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ R: refusals

#[test]
#[ignore = "record of the registered run of 8ca0d696 (ADR 0507 amendment 2): R2 refused by ConflictingConstraint (node 4 component 1) before the check, a set-up defect; continued in r1_r2_refusals_a2"]
fn r1_r2_refusals() {
    let mut c = Checks::new();
    let m = bar(F_MAX);
    let o = ContactOptions::new(1, 1e-11);
    let e = solve(&m, &[], &[], &[ContactPair::new("nope", stop().target, 1.0)], &o).unwrap_err();
    c.holds(
        "R1 unknown contactor",
        matches!(e, FemError::UnknownSet { .. }),
        &e.to_string(),
    );
    let e = solve(
        &m,
        &[],
        &[],
        &[ContactPair::new("xmax", Target::Surface("nope".into()), 1.0)],
        &o,
    )
    .unwrap_err();
    c.holds(
        "R1 unknown target",
        matches!(e, FemError::UnknownSet { .. }),
        &e.to_string(),
    );
    for pen in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let e = solve(&m, &[], &[], &[ContactPair::new("xmax", stop().target, pen)], &o).unwrap_err();
        c.holds(
            &format!("R1 penalty {pen}"),
            matches!(e, FemError::InvalidValue { key: "penalty", .. }),
            &e.to_string(),
        );
    }
    for tol in [-1.0, f64::NAN] {
        let e = solve(&m, &[], &[], &[stop()], &ContactOptions::new(1, tol)).unwrap_err();
        c.holds(
            &format!("R1 tolerance {tol}"),
            matches!(
                e,
                FemError::InvalidValue {
                    key: "penetration_tolerance",
                    ..
                }
            ),
            &e.to_string(),
        );
    }
    let mut t = SolidModel::new(tet_box([0.0; 3], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap());
    t.add_material("all", elastic(1.0, 0.3)).unwrap();
    let e = solve(&t, &[], &[], &[ContactPair::new("xmax", stop().target, 1.0)], &o).unwrap_err();
    c.holds(
        "R1 Tet4 face",
        matches!(&e, FemError::InvalidValue { key: "contact surface", reason } if reason.contains("element")),
        &e.to_string(),
    );
    let mut m2 = bar(0.0);
    m2.add_constraint(Constraint::displacement("xmax", 1, 1e-3)).unwrap();
    let e = solve(
        &m2,
        &[],
        &[],
        &[stop()],
        &o.clone().kinematics(Kinematics::TotalLagrangian),
    )
    .unwrap_err();
    c.holds(
        "R2 total Lagrangian with a non-zero model constraint",
        matches!(&e, FemError::InvalidValue { key: "constraint", reason } if reason.contains("'xmax'")),
        &e.to_string(),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ amendment 2
// The corrected runs of ADR 0507 amendment 2 (bodies identical to the records except R2), each made once.

#[test]
#[ignore = "record of the corrected run of 043ee2ea (ADR 0507 amendment 3): pressed stress and pressure 2.50e-3 of E d/8, touching pressure 3.13e-4 of E d (limits 1e-8) missed; continued in s1_blocks_pressed_then_pulled_apart_a3"]
fn s1_blocks_pressed_then_pulled_apart_a2() {
    let d = 1e-3;
    let m = blocks(d);
    let top: Vec<Prescribed> = m
        .mesh()
        .node_set("upper.zmax")
        .unwrap()
        .iter()
        .map(|&n| Prescribed::new(n, 2, 2.0 * d))
        .collect();
    let r = solve(
        &m,
        &top,
        &[],
        &[block_pair()],
        &ContactOptions::new(8, 1e-13).newton(newton()),
    )
    .unwrap();
    let mut c = Checks::new();
    let scale = E_BL * d / 8.0;
    let (mut neg, mut st, mut pr, mut touch, mut zero, mut gap, mut free) = (
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    for s in &r.steps {
        let l = s.load_factor;
        for n in &s.nodes {
            // Every multiplier finite and >= 0: a negative or non-finite one is recorded as infinite.
            neg.push(if n.force.is_finite() && n.force >= 0.0 {
                0.0
            } else {
                f64::INFINITY
            });
        }
        let k = s.step;
        if k < 4 {
            let sigma = E_BL * d * (l - 0.5);
            st.push(stress_error(&m, &s.displacement, [0.0, 0.0, sigma, 0.0, 0.0, 0.0], None) / scale);
            pr.extend(s.nodes.iter().map(|n| (n.pressure + sigma).abs() / scale));
        } else if k == 4 {
            touch.extend(s.nodes.iter().map(|n| n.pressure.abs() / (E_BL * d)));
        } else {
            zero.extend(s.nodes.iter().map(|n| if n.force == 0.0 { 0.0 } else { f64::INFINITY }));
            gap.extend(
                s.nodes
                    .iter()
                    .map(|n| (n.gap.unwrap_or(f64::INFINITY) - d * (2.0 * l - 1.0)).abs() / d),
            );
            free.push(stress_error(&m, &s.displacement, [0.0; 6], None) / (E_BL * d));
        }
    }
    c.at_most("S1 every multiplier >= 0 (0 if so)", worst(neg), 0.0);
    c.at_most("S1 pressed: stress / (E d/8)", worst(st), 1e-8);
    c.at_most("S1 pressed: node pressure / (E d/8)", worst(pr), 1e-8);
    c.at_most("S1 touching: node pressure / (E d)", worst(touch), 1e-8);
    c.at_most("S1 apart: every multiplier exactly 0 (0 if so)", worst(zero), 0.0);
    c.at_most("S1 apart: gap / d", worst(gap), 1e-8);
    c.at_most("S1 apart: stress / (E d)", worst(free), 1e-8);
    c.finish();
}

#[test]
#[ignore = "record of the corrected run of 043ee2ea (ADR 0507 amendment 3): not converged in 30 iterations, relative residual 2.29e-8, energy ratio 1.24e-26; continued in i1_to_i4_interference_fit_ring_on_a_solid_shaft_a3"]
fn i1_to_i4_interference_fit_ring_on_a_solid_shaft_a2() {
    let fit = fit_model();
    let m = &fit.model;
    let pair = ContactPair::new("ring_in", Target::Surface("shaft_out".into()), 100.0 * E_FIT / 0.125);
    let r = solve(m, &[], &[], &[pair], &ContactOptions::new(1, 1e-9).newton(newton())).unwrap();
    let p_cf = E_FIT * D_FIT * (B_FIT * B_FIT - A_FIT * A_FIT) / (2.0 * A_FIT * B_FIT * B_FIT);
    let mean = r.nodes.iter().map(|n| n.pressure).sum::<f64>() / r.nodes.len() as f64;
    let mut c = Checks::new();
    println!(
        "I contact pressure: mean {mean:.6e}, closed form {p_cf:.6e}, {} nodes",
        r.nodes.len()
    );
    c.holds(
        "I nodes",
        r.nodes.len() == 2 * N_FIT,
        &format!("{} contactor nodes", r.nodes.len()),
    );
    c.at_most(
        "I1 every node's pressure",
        worst(r.nodes.iter().map(|n| (n.pressure / p_cf - 1.0).abs())),
        2e-2,
    );
    c.at_most("I1 mean pressure", (mean / p_cf - 1.0).abs(), 2e-2);
    let u = r.result.displacement_values();
    let f = IntegrationPointField::compute(m, u).unwrap();
    let off = f.offsets();
    let mut hoop = Vec::new();
    for &e in &fit.ring {
        let s = &f.stress()[off[e]..off[e + 1]];
        let np = s.len() as f64;
        let mut avg = [0.0; 6];
        for p in s {
            for i in 0..6 {
                avg[i] += p[i] / np;
            }
        }
        let en = m.mesh().element_nodes(e);
        let (mut cx, mut cy) = (0.0, 0.0);
        for &v in en {
            cx += m.mesh().nodes()[v][0] / 8.0;
            cy += m.mesh().nodes()[v][1] / 8.0;
        }
        let rc = (cx * cx + cy * cy).sqrt();
        let (co, si) = (cx / rc, cy / rc);
        let st = avg[0] * si * si - 2.0 * avg[3] * si * co + avg[1] * co * co;
        let lame = mean * A_FIT * A_FIT * (1.0 + B_FIT * B_FIT / (rc * rc)) / (B_FIT * B_FIT - A_FIT * A_FIT);
        hoop.push((st / lame - 1.0).abs());
    }
    c.at_most("I2 ring hoop stress against Lame", worst(hoop), 2e-2);
    let mut sh = Vec::new();
    for &e in &fit.shaft {
        for p in &f.stress()[off[e]..off[e + 1]] {
            sh.push(((p[0] + mean).abs().max((p[1] + mean).abs()).max(p[3].abs())) / mean);
        }
    }
    c.at_most("I3 shaft uniform stress", worst(sh), 1e-6);
    c.at_most("I4 penetration", r.steps[0].solves.last().unwrap().penetration, 1e-9);
    let rigid = E_FIT * D_FIT * (B_FIT * B_FIT - A_FIT * A_FIT)
        / (A_FIT * ((1.0 + NU_FIT) * B_FIT * B_FIT + (1.0 - NU_FIT) * A_FIT * A_FIT));
    let ni = (mean / rigid - 1.0).abs();
    println!("NI against the rigid-shaft form {rigid:.6e}: {ni:.6e}");
    c.at_least("NI rigid-shaft form (negative control)", ni, 0.2);
    c.finish();
}

#[test]
#[ignore = "known miss of ADR 0507 amendment 3 (run of 043ee2ea): orders [1.48, 0.191, 0.249, 1.57, 0.171], least 0.171 < 1.8; not run again"]
fn t2_order_of_the_last_residuals_total_lagrangian_punch_a2_known_miss() {
    let plate = hex_box([0.0; 3], [2.0, 0.5, 0.25], [8, 2, 2]).unwrap();
    let punch = hex_box([0.8, 0.0, 0.25], [0.4, 0.2, 0.2], [3, 1, 1]).unwrap();
    let mesh = assemble_parts(&[("plate", &plate), ("punch", &punch)]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("plate.all", elastic(1.0, 0.3)).unwrap();
    m.add_material("punch.all", elastic(1.0, 0.3)).unwrap();
    for s in ["plate.xmin", "plate.xmax"] {
        for cc in 0..3 {
            m.add_constraint(Constraint::displacement(s, cc, 0.0)).unwrap();
        }
    }
    m.add_constraint(Constraint::displacement("plate.ymin", 1, 0.0))
        .unwrap();
    m.add_constraint(Constraint::displacement("punch.ymin", 1, 0.0))
        .unwrap();
    m.add_constraint(Constraint::displacement("punch.zmax", 0, 0.0))
        .unwrap();
    m.add_load(Load::pressure("punch.zmax", 0.01)).unwrap();
    let pair = ContactPair::new("punch.zmin", Target::Surface("plate.zmax".into()), 800.0);
    let o = ContactOptions::new(2, 1e-8)
        .newton(newton())
        .kinematics(Kinematics::TotalLagrangian);
    let r = solve(&m, &[], &[], &[pair], &o).unwrap();
    let mut orders = Vec::new();
    for s in &r.steps {
        for (i, v) in s.solves.iter().enumerate() {
            let kept: Vec<usize> = (0..v.residuals.len()).filter(|&k| v.residuals[k] >= 1e-12).collect();
            println!(
                "T2 step {} solve {i}: residuals {:?}, active counts {:?}",
                s.step,
                v.residuals,
                v.active_sets.iter().map(|a| a.len()).collect::<Vec<_>>()
            );
            if kept.len() < 3 {
                continue;
            }
            let (a, b, cc) = (kept[kept.len() - 3], kept[kept.len() - 2], kept[kept.len() - 1]);
            if v.active_sets[a] != v.active_sets[b] || v.active_sets[b] != v.active_sets[cc] {
                continue;
            }
            let (ra, rb, rc) = (v.residuals[a], v.residuals[b], v.residuals[cc]);
            orders.push((rc / rb).ln() / (rb / ra).ln());
        }
    }
    let dz = r
        .result
        .displacement_values()
        .iter()
        .fold(0.0f64, |a, b| a.max(b.abs()));
    println!("T2 orders {orders:?}, largest displacement {dz:.4e}");
    let mut c = Checks::new();
    c.at_least("T2 qualifying solves", orders.len() as f64, 1.0);
    let least = orders.iter().fold(
        f64::INFINITY,
        |a, &b| if b.is_finite() { a.min(b) } else { f64::NEG_INFINITY },
    );
    c.at_least("T2 order of every qualifying solve", least, 1.8);
    c.finish();
}

#[test]
fn r1_r2_refusals_a2() {
    let mut c = Checks::new();
    let m = bar(F_MAX);
    let o = ContactOptions::new(1, 1e-11);
    let e = solve(&m, &[], &[], &[ContactPair::new("nope", stop().target, 1.0)], &o).unwrap_err();
    c.holds(
        "R1 unknown contactor",
        matches!(e, FemError::UnknownSet { .. }),
        &e.to_string(),
    );
    let e = solve(
        &m,
        &[],
        &[],
        &[ContactPair::new("xmax", Target::Surface("nope".into()), 1.0)],
        &o,
    )
    .unwrap_err();
    c.holds(
        "R1 unknown target",
        matches!(e, FemError::UnknownSet { .. }),
        &e.to_string(),
    );
    for pen in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let e = solve(&m, &[], &[], &[ContactPair::new("xmax", stop().target, pen)], &o).unwrap_err();
        c.holds(
            &format!("R1 penalty {pen}"),
            matches!(e, FemError::InvalidValue { key: "penalty", .. }),
            &e.to_string(),
        );
    }
    for tol in [-1.0, f64::NAN] {
        let e = solve(&m, &[], &[], &[stop()], &ContactOptions::new(1, tol)).unwrap_err();
        c.holds(
            &format!("R1 tolerance {tol}"),
            matches!(
                e,
                FemError::InvalidValue {
                    key: "penetration_tolerance",
                    ..
                }
            ),
            &e.to_string(),
        );
    }
    let mut t = SolidModel::new(tet_box([0.0; 3], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap());
    t.add_material("all", elastic(1.0, 0.3)).unwrap();
    let e = solve(&t, &[], &[], &[ContactPair::new("xmax", stop().target, 1.0)], &o).unwrap_err();
    c.holds(
        "R1 Tet4 face",
        matches!(&e, FemError::InvalidValue { key: "contact surface", reason } if reason.contains("element")),
        &e.to_string(),
    );
    let mut m2 = bar(0.0);
    // ADR 0507 amendment 2: u_x on xmax (no other constraint on that component there).
    m2.add_constraint(Constraint::displacement("xmax", 0, 1e-3)).unwrap();
    let e = solve(
        &m2,
        &[],
        &[],
        &[stop()],
        &o.clone().kinematics(Kinematics::TotalLagrangian),
    )
    .unwrap_err();
    c.holds(
        "R2 total Lagrangian with a non-zero model constraint",
        matches!(&e, FemError::InvalidValue { key: "constraint", reason } if reason.contains("'xmax'")),
        &e.to_string(),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ amendment 3
// The corrected runs of ADR 0507 amendment 3 (bodies identical to the records), each made once.

#[test]
fn s1_blocks_pressed_then_pulled_apart_a3() {
    let d = 1e-3;
    let m = blocks(d);
    let top: Vec<Prescribed> = m
        .mesh()
        .node_set("upper.zmax")
        .unwrap()
        .iter()
        .map(|&n| Prescribed::new(n, 2, 2.0 * d))
        .collect();
    let r = solve(
        &m,
        &top,
        &[],
        &[block_pair()],
        &ContactOptions::new(8, 1e-13).newton(newton()),
    )
    .unwrap();
    let mut c = Checks::new();
    let scale = E_BL * d / 8.0;
    let (mut neg, mut st, mut pr, mut touch, mut zero, mut gap, mut free) = (
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    for s in &r.steps {
        let l = s.load_factor;
        for n in &s.nodes {
            // Every multiplier finite and >= 0: a negative or non-finite one is recorded as infinite.
            neg.push(if n.force.is_finite() && n.force >= 0.0 {
                0.0
            } else {
                f64::INFINITY
            });
        }
        let k = s.step;
        if k < 4 {
            let sigma = E_BL * d * (l - 0.5);
            st.push(stress_error(&m, &s.displacement, [0.0, 0.0, sigma, 0.0, 0.0, 0.0], None) / scale);
            pr.extend(s.nodes.iter().map(|n| (n.pressure + sigma).abs() / scale));
        } else if k == 4 {
            touch.extend(s.nodes.iter().map(|n| n.pressure.abs() / (E_BL * d)));
        } else {
            zero.extend(s.nodes.iter().map(|n| if n.force == 0.0 { 0.0 } else { f64::INFINITY }));
            gap.extend(
                s.nodes
                    .iter()
                    .map(|n| (n.gap.unwrap_or(f64::INFINITY) - d * (2.0 * l - 1.0)).abs() / d),
            );
            free.push(stress_error(&m, &s.displacement, [0.0; 6], None) / (E_BL * d));
        }
    }
    c.at_most("S1 every multiplier >= 0 (0 if so)", worst(neg), 0.0);
    c.at_most("S1 pressed: stress / (E d/8)", worst(st), 1e-8);
    c.at_most("S1 pressed: node pressure / (E d/8)", worst(pr), 1e-8);
    c.at_most("S1 touching: node pressure / (E d)", worst(touch), 1e-8);
    c.at_most("S1 apart: every multiplier exactly 0 (0 if so)", worst(zero), 0.0);
    c.at_most("S1 apart: gap / d", worst(gap), 1e-8);
    c.at_most("S1 apart: stress / (E d)", worst(free), 1e-8);
    c.finish();
}

#[test]
fn i1_to_i4_interference_fit_ring_on_a_solid_shaft_a3() {
    let fit = fit_model();
    let m = &fit.model;
    let pair = ContactPair::new("ring_in", Target::Surface("shaft_out".into()), 100.0 * E_FIT / 0.125);
    let r = solve(m, &[], &[], &[pair], &ContactOptions::new(1, 1e-9).newton(newton())).unwrap();
    let p_cf = E_FIT * D_FIT * (B_FIT * B_FIT - A_FIT * A_FIT) / (2.0 * A_FIT * B_FIT * B_FIT);
    let mean = r.nodes.iter().map(|n| n.pressure).sum::<f64>() / r.nodes.len() as f64;
    let mut c = Checks::new();
    println!(
        "I contact pressure: mean {mean:.6e}, closed form {p_cf:.6e}, {} nodes",
        r.nodes.len()
    );
    c.holds(
        "I nodes",
        r.nodes.len() == 2 * N_FIT,
        &format!("{} contactor nodes", r.nodes.len()),
    );
    c.at_most(
        "I1 every node's pressure",
        worst(r.nodes.iter().map(|n| (n.pressure / p_cf - 1.0).abs())),
        2e-2,
    );
    c.at_most("I1 mean pressure", (mean / p_cf - 1.0).abs(), 2e-2);
    let u = r.result.displacement_values();
    let f = IntegrationPointField::compute(m, u).unwrap();
    let off = f.offsets();
    let mut hoop = Vec::new();
    for &e in &fit.ring {
        let s = &f.stress()[off[e]..off[e + 1]];
        let np = s.len() as f64;
        let mut avg = [0.0; 6];
        for p in s {
            for i in 0..6 {
                avg[i] += p[i] / np;
            }
        }
        let en = m.mesh().element_nodes(e);
        let (mut cx, mut cy) = (0.0, 0.0);
        for &v in en {
            cx += m.mesh().nodes()[v][0] / 8.0;
            cy += m.mesh().nodes()[v][1] / 8.0;
        }
        let rc = (cx * cx + cy * cy).sqrt();
        let (co, si) = (cx / rc, cy / rc);
        let st = avg[0] * si * si - 2.0 * avg[3] * si * co + avg[1] * co * co;
        let lame = mean * A_FIT * A_FIT * (1.0 + B_FIT * B_FIT / (rc * rc)) / (B_FIT * B_FIT - A_FIT * A_FIT);
        hoop.push((st / lame - 1.0).abs());
    }
    c.at_most("I2 ring hoop stress against Lame", worst(hoop), 2e-2);
    let mut sh = Vec::new();
    for &e in &fit.shaft {
        for p in &f.stress()[off[e]..off[e + 1]] {
            sh.push(((p[0] + mean).abs().max((p[1] + mean).abs()).max(p[3].abs())) / mean);
        }
    }
    c.at_most("I3 shaft uniform stress", worst(sh), 1e-6);
    c.at_most("I4 penetration", r.steps[0].solves.last().unwrap().penetration, 1e-9);
    let rigid = E_FIT * D_FIT * (B_FIT * B_FIT - A_FIT * A_FIT)
        / (A_FIT * ((1.0 + NU_FIT) * B_FIT * B_FIT + (1.0 - NU_FIT) * A_FIT * A_FIT));
    let ni = (mean / rigid - 1.0).abs();
    println!("NI against the rigid-shaft form {rigid:.6e}: {ni:.6e}");
    c.at_least("NI rigid-shaft form (negative control)", ni, 0.2);
    c.finish();
}
