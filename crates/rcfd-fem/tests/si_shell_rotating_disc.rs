//! F-524 (ADR 0439): the rotating plane-stress annular disc of Shell4 facets against the closed form. Checks U1 to U3, SR1
//! to SR3, ST1 to ST3, O1, O2, P1, K1 to K6 and the negative controls N1, N2 of the registration.

use rcfd_core::vec3::{dot, Vec3};
use rcfd_fem::free_motions::free_rigid_motions;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::shell;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
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

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const A: f64 = 0.1;
const B: f64 = 0.3;
const T: f64 = 0.005;
const OMEGA: f64 = 300.0;

/// `K = (3 + nu) rho omega^2 / 8` (D1).
fn k_disc() -> f64 {
    (3.0 + NU) * RHO * OMEGA * OMEGA / 8.0
}

/// The closed form of D1: radial stress.
fn sigma_r(r: f64) -> f64 {
    k_disc() * (A * A + B * B - A * A * B * B / (r * r) - r * r)
}

/// The closed form of D1: hoop stress.
fn sigma_t(r: f64) -> f64 {
    k_disc() * (A * A + B * B + A * A * B * B / (r * r) - (1.0 + 3.0 * NU) / (3.0 + NU) * r * r)
}

/// The closed form of D1: radial displacement.
fn u_r(r: f64) -> f64 {
    r * (sigma_t(r) - NU * sigma_r(r)) / E
}

/// `max u` over `[a, b]` (on a fine sampling; D1: `u(a)`).
fn u_max() -> f64 {
    (0..=200_000)
        .map(|k| u_r(A + (B - A) * k as f64 / 200_000.0))
        .fold(0.0f64, f64::max)
}

/// `max sigma_r` over `[a, b]` (D1: `K (b - a)^2`).
fn sigma_r_max() -> f64 {
    (0..=200_000)
        .map(|k| sigma_r(A + (B - A) * k as f64 / 200_000.0))
        .fold(0.0f64, f64::max)
}

#[derive(Clone, Copy, PartialEq)]
enum Supports {
    /// D3: six point zeros on three bore nodes.
    Registered,
    /// N1: the bore held in-plane, `w` at the three nodes of D3.
    BoreClamped,
}

struct Disc {
    model: SolidModel,
    nodes: Vec<Vec3>,
    radii: Vec<f64>,
    elements: usize,
    supported: Vec<(usize, usize)>,
}

/// The mesh of D2 and the supports.
fn disc(nr: usize, nt: usize, supports: Supports) -> Disc {
    let idx = |i: usize, j: usize| (j % nt) * (nr + 1) + i;
    let radii: Vec<f64> = (0..=nr).map(|i| A + (B - A) * i as f64 / nr as f64).collect();
    let mut nodes = Vec::new();
    for j in 0..nt {
        let th = 2.0 * PI * j as f64 / nt as f64;
        for &r in &radii {
            nodes.push([r * th.cos(), r * th.sin(), 0.0]);
        }
    }
    let mut conn = Vec::new();
    for j in 0..nt {
        for i in 0..nr {
            conn.extend_from_slice(&[idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let elements = nr * nt;
    let mut mesh = SolidMesh::new(nodes.clone()).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(T).unwrap()).unwrap();
    mesh.add_element_set("all", (0..elements).collect()).unwrap();
    let (p0, p90, p180) = (idx(0, 0), idx(0, nt / 4), idx(0, nt / 2));
    mesh.add_node_set("p0", vec![p0]).unwrap();
    mesh.add_node_set("p90", vec![p90]).unwrap();
    mesh.add_node_set("p180", vec![p180]).unwrap();
    mesh.add_node_set("bore", (0..nt).map(|j| idx(0, j)).collect()).unwrap();
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap())
        .unwrap();
    let mut supported = Vec::new();
    match supports {
        Supports::Registered => {
            for (set, node, comp) in [("p0", p0, 1), ("p90", p90, 0), ("p180", p180, 1)] {
                model.add_constraint(Constraint::displacement(set, comp, 0.0)).unwrap();
                supported.push((node, comp));
            }
        }
        Supports::BoreClamped => {
            for comp in 0..2 {
                model
                    .add_constraint(Constraint::displacement("bore", comp, 0.0))
                    .unwrap();
            }
        }
    }
    for (set, node) in [("p0", p0), ("p90", p90), ("p180", p180)] {
        model.add_constraint(Constraint::displacement(set, 2, 0.0)).unwrap();
        supported.push((node, 2));
    }
    model
        .add_load(Load::centrifugal([0.0, 0.0, 1.0], [0.0; 3], OMEGA))
        .unwrap();
    let radii_of_nodes = (0..nt).flat_map(|_| radii.iter().copied()).collect();
    Disc {
        model,
        nodes,
        radii: radii_of_nodes,
        elements,
        supported,
    }
}

struct Measured {
    e_u: f64,
    e_sr: f64,
    e_st: f64,
    u_theta: f64,
    bending: f64,
    theta_z: f64,
    reaction: f64,
    free: usize,
    on_circle: f64,
    counts: (usize, usize),
}

/// Solve and measure as the readings of the registration say.
fn measure(nr: usize, nt: usize, supports: Supports) -> Measured {
    let d = disc(nr, nt, supports);
    let r = solve_static(&d.model, &StaticOptions::default()).unwrap();
    let umax = u_max();
    let mut e_u = 0.0f64;
    let mut u_theta = 0.0f64;
    let mut bending = 0.0f64;
    let mut theta_z = 0.0f64;
    let mut on_circle = 0.0f64;
    for (k, x) in d.nodes.iter().enumerate() {
        let rk = x[0].hypot(x[1]);
        on_circle = on_circle.max((rk - d.radii[k]).abs());
        let (c, s) = (x[0] / rk, x[1] / rk);
        let u = r.displacement(k);
        let ur = u[0] * c + u[1] * s;
        let ut = -u[0] * s + u[1] * c;
        e_u = e_u.max((ur - u_r(d.radii[k])).abs() / umax);
        u_theta = u_theta.max(ut.abs() / umax);
        bending = bending.max(u[2].abs().max(A * u[3].abs()).max(A * u[4].abs()) / umax);
        theta_z = theta_z.max(A * u[5].abs() / umax);
    }
    let srmax = sigma_r_max();
    let stmax = sigma_t(A);
    let mut e_sr = 0.0f64;
    let mut e_st = 0.0f64;
    let st = shell::stresses(&d.model, r.displacement_values()).unwrap();
    for (e, s) in &st {
        let en = d.model.mesh().element_nodes(*e);
        let mut xc = [0.0; 3];
        for &n in en {
            for q in 0..3 {
                xc[q] += 0.25 * d.nodes[n][q];
            }
        }
        let rc = xc[0].hypot(xc[1]);
        let er = [xc[0] / rc, xc[1] / rc, 0.0];
        let et = [-er[1], er[0], 0.0];
        // the membrane stress tensor in the local axes e_1, e_2
        let [nxx, nyy, nxy] = s.membrane;
        let turn = |v: Vec3| -> f64 {
            let (p, q) = (dot(v, s.axes[0]), dot(v, s.axes[1]));
            (p * p * nxx + q * q * nyy + 2.0 * p * q * nxy) / T
        };
        e_sr = e_sr.max((turn(er) - sigma_r(rc)).abs() / srmax);
        e_st = e_st.max((turn(et) - sigma_t(rc)).abs() / stmax);
    }
    let f = nodal_load_vector(&d.model, &[]).unwrap();
    let fmax = f.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    let reactions = r.reactions().unwrap();
    let dofs = d.model.dof_map();
    let reaction = d
        .supported
        .iter()
        .map(|&(n, c)| reactions[dofs.equation(n, c).unwrap()].abs())
        .fold(0.0f64, f64::max)
        / fmax;
    let free = free_rigid_motions(&d.model, &[]).unwrap().count;
    Measured {
        e_u,
        e_sr,
        e_st,
        u_theta,
        bending,
        theta_z,
        reaction,
        free,
        on_circle,
        counts: (d.nodes.len(), d.elements),
    }
}

const LEVELS: [(usize, usize); 3] = [(4, 24), (8, 48), (16, 96)];
/// Limits of U, SR, ST per level (registration, D4).
const LIMITS: [[f64; 3]; 3] = [
    [4.2e-2, 6.6e-2, 2.2e-2],
    [1.2e-2, 1.8e-2, 5.6e-3],
    [2.9e-3, 4.5e-3, 1.5e-3],
];
/// The prototype's values (precise.log), for P1.
const PROTOTYPE: [[f64; 3]; 3] = [
    [2.7369781392e-2, 4.3880312115e-2, 1.4444232806e-2],
    [7.3480083461e-3, 1.1595371467e-2, 3.7068596673e-3],
    [1.8749513223e-3, 2.9721749953e-3, 9.3471196775e-04],
];

#[test]
fn rotating_disc_against_the_closed_form() {
    let mut c = Checks::new();
    let names = ["U", "SR", "ST"];
    let mut errors = Vec::new();
    for (l, &(nr, nt)) in LEVELS.iter().enumerate() {
        let m = measure(nr, nt, Supports::Registered);
        println!("level {} ({nr} x {nt})", l + 1);
        let e = [m.e_u, m.e_sr, m.e_st];
        for q in 0..3 {
            c.at_most(&format!("{}{}", names[q], l + 1), e[q], LIMITS[l][q]);
        }
        for q in 0..3 {
            c.at_most(
                &format!("P1 {}{} against the prototype", names[q], l + 1),
                (e[q] / PROTOTYPE[l][q] - 1.0).abs(),
                1e-6,
            );
        }
        c.at_most(&format!("K1 level {} u_theta", l + 1), m.u_theta, 1e-9);
        c.at_most(&format!("K2 level {} w, a theta_x, a theta_y", l + 1), m.bending, 1e-9);
        c.at_most(&format!("K3 level {} a theta_z", l + 1), m.theta_z, 1e-6);
        c.at_most(&format!("K4 level {} reactions / max f", l + 1), m.reaction, 1e-9);
        c.holds(
            &format!("K5 level {} free rigid motions", l + 1),
            m.free == 0,
            &format!("count {}", m.free),
        );
        c.at_most(
            &format!("K6 level {} nodes on their circles (m)", l + 1),
            m.on_circle,
            1e-15,
        );
        c.holds(
            &format!("K6 level {} counts", l + 1),
            m.counts == ((nr + 1) * nt, nr * nt),
            &format!("{:?}", m.counts),
        );
        errors.push(e);
    }
    for k in 0..2 {
        for q in 0..3 {
            let order = (errors[k][q] / errors[k + 1][q]).log2();
            c.at_least(&format!("O{} order of {}", k + 1, names[q]), order, 1.8);
        }
    }
    c.finish();
}

#[test]
fn n1_bore_clamped() {
    let mut c = Checks::new();
    let m = measure(16, 96, Supports::BoreClamped);
    println!("N1 e_sr {:.4e}, e_st {:.4e}", m.e_sr, m.e_st);
    c.at_least("N1 e_u with the bore held", m.e_u, 0.5);
    c.finish();
}

#[test]
fn n2_chord_control() {
    let mut c = Checks::new();
    let coarse = measure(16, 24, Supports::Registered);
    let fine = measure(16, 96, Supports::Registered);
    println!("N2 16 x 24: e_u {:.4e}, e_sr {:.4e}", coarse.e_u, coarse.e_sr);
    c.at_least("N2 e_st at 16 x 24", coarse.e_st, 1e-2);
    let ratio = coarse.e_st / fine.e_st;
    c.holds(
        "N2 e_st(16 x 24) / e_st(16 x 96) in [10, 40]",
        (10.0..=40.0).contains(&ratio),
        &format!("{ratio:.4}"),
    );
    c.finish();
}
