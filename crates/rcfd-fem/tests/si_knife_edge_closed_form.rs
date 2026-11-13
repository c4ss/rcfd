//! F-492 (ADR 0394): the buckling factors of ADR 0390's knife-edge Hex20 beam against the closed form of a Timoshenko column
//! whose initial-stress term also works on the rotating sections, `(r^2 k^2/Q) P^2 - (1 + a + r^2 k^2) P + P_E,n = 0`
//! (ADR 0394 D1). Checks A1 to A3, C1 to C5 and the negative controls N1 to N3 of the registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::buckling::{factors, BucklingOptions, BucklingResult};
use rcfd_fem::builders::hex_box;
use rcfd_fem::geometric::Prestress;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, Load, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

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
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{name}: {value:.6} (limit >= {limit}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value} < {limit}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
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

// ------------------------------------------------------------------------------------------------- set-ups (D2)

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const H: f64 = 0.1;
const B: f64 = 0.1;
const AREA: f64 = H * B;
const I: f64 = B * H * H * H / 12.0;
const KAPPA: f64 = 5.0 / 6.0;
/// The reference compression of the buckling runs (Pa); `|S_REF| A` converts a factor to a force.
const S_REF: f64 = -1e6;
const TOL: f64 = 1e-9;

fn material() -> Material {
    Material::isotropic_elastic(E, 0.0, RHO, 0.0).unwrap()
}

fn nodes_where(mesh: &SolidMesh, f: impl Fn(Vec3) -> bool) -> Vec<usize> {
    mesh.nodes()
        .iter()
        .enumerate()
        .filter(|(_, &p)| f(p))
        .map(|(i, _)| i)
        .collect()
}

/// The knife-edge beam of ADR 0390 D7 with length `length` and `nx` elements along: Hex20, knife edges on the neutral axis
/// (`ke0` pin, `ke1` roller), `u_z = 0` everywhere, the balanced end tractions of an axial stress `s`.
fn beam(length: f64, nx: usize, s: f64) -> SolidModel {
    let base = hex_box([0.0; 3], [length, H, B], [nx, 2, 1]).unwrap();
    let mut mesh = upgrade_to_hex20(&base).unwrap().0;
    let ke0 = nodes_where(&mesh, |p| p[0].abs() < TOL && (p[1] - H / 2.0).abs() < TOL);
    let ke1 = nodes_where(&mesh, |p| (p[0] - length).abs() < TOL && (p[1] - H / 2.0).abs() < TOL);
    let every = (0..mesh.nodes().len()).collect();
    mesh.add_node_set("ke0", ke0).unwrap();
    mesh.add_node_set("ke1", ke1).unwrap();
    mesh.add_node_set("every", every).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m.add_constraint(Constraint::displacement("ke0", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ke0", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ke1", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("every", 2, 0.0)).unwrap();
    m.add_load(Load::traction("xmax", [s, 0.0, 0.0])).unwrap();
    m.add_load(Load::traction("xmin", [-s, 0.0, 0.0])).unwrap();
    m
}

/// Static pre-stress at `S_REF` and the two lowest buckling factors.
fn buckle(name: &str, length: f64, nx: usize) -> (SolidModel, BucklingResult) {
    let model = beam(length, nx, S_REF);
    let st = solve_static(&model, &StaticOptions::default()).expect("static solve");
    let pre = Prestress::from_static(&model, &st).expect("pre-stress");
    let r = factors(&model, &pre, &BucklingOptions::new(2)).expect("buckling");
    println!(
        "{name}: factors {:?}, iterations {}, residuals {:?}, Sturm {:?}, exhausted {:?}",
        r.factors, r.iterations, r.residuals, r.sturm, r.exhausted
    );
    (model, r)
}

// ------------------------------------------------------------------------------------------------- closed form (D1)

/// The lower root `P_n` (N) of `(r^2 k^2/Q) P^2 - (1 + a + r^2 k^2) P + P_E,n = 0`, `k = n pi/L`, `P_E,n = E I k^2`, `a =
/// P_E,n/Q`, `Q = kappa G A`, `r^2 = I/A`; `rotating = false` drops the rotating-section term (`r^2 k^2 = 0`: Engesser,
/// `P_E,n/(1 + a)`).
fn lower_root(length: f64, n: usize, rotating: bool) -> f64 {
    let k = n as f64 * PI / length;
    let q = KAPPA * (E / 2.0) * AREA;
    let pe = E * I * k * k;
    let rk = if rotating { I / AREA * k * k } else { 0.0 };
    let c1 = 1.0 + pe / q + rk;
    let c2 = rk / q;
    2.0 * pe / (c1 + (c1 * c1 - 4.0 * c2 * pe).sqrt())
}

/// `|lambda |S_REF| A / p - 1|`.
fn off(lambda: f64, p: f64) -> f64 {
    (lambda * S_REF.abs() * AREA / p - 1.0).abs()
}

/// The correlation of ADR 0390 D8 of `u_y` on the neutral line `y = H/2, z = 0` with `sin(n pi x/L)`.
fn identification(model: &SolidModel, mode: &[f64], length: f64, n: usize) -> f64 {
    let dofs = model.dof_map();
    let mesh = model.mesh();
    let k = n as f64 * PI / length;
    let (mut uf, mut uu, mut ff) = (0.0, 0.0, 0.0);
    for node in nodes_where(mesh, |p| (p[1] - H / 2.0).abs() < TOL && p[2].abs() < TOL) {
        let u = mode[dofs.equation(node, 1).unwrap()];
        let g = (k * mesh.nodes()[node][0]).sin();
        uf += u * g;
        uu += u * u;
        ff += g * g;
    }
    uf.abs() / (uu * ff).sqrt()
}

fn two_factors(c: &mut Checks, name: &str, r: &BucklingResult) {
    c.holds(
        &format!("C5 {name} two factors, Sturm"),
        r.factors.len() == 2 && r.sturm.counted == 2 && r.sturm.expected == 2,
        &format!(
            "{} factors, counted {}, expected {}",
            r.factors.len(),
            r.sturm.counted,
            r.sturm.expected
        ),
    );
}

// ------------------------------------------------------------------------------------------------- the checks

#[test]
fn a1_a2_c1_c2_n1_knife_edge_beam_l_over_h_20() {
    let mut c = Checks::new();
    let length = 2.0;
    let (model, r) = buckle("B20", length, 40);
    two_factors(&mut c, "B20", &r);
    let p1 = lower_root(length, 1, true);
    let p2 = lower_root(length, 2, true);
    println!("closed form: P_1 = {p1:.6e} N, P_2 = {p2:.6e} N");
    c.at_most("A1 B20 |lambda_1 |s| A/P_1 - 1|", off(r.factors[0], p1), 0.01);
    c.at_most("A2 B20 |lambda_2 |s| A/P_2 - 1|", off(r.factors[1], p2), 0.01);
    c.at_most("C1 B20 n = 1", off(r.factors[0], p1), 1e-3);
    c.at_most("C1 B20 n = 2", off(r.factors[1], p2), 1e-3);
    for n in 1..=2 {
        let corr = identification(&model, &r.modes[n - 1], length, n);
        c.at_least(&format!("C2 B20 mode {n}"), corr, 0.99);
    }
    // N1: Engesser alone is seen by the sharper check.
    for n in 1..=2 {
        let pe = lower_root(length, n, false);
        c.above(&format!("N1 B20 n = {n} against Engesser"), off(r.factors[n - 1], pe), 1e-3);
    }
    // N2: the clause of ADR 0390 A9 is not met at L/h = 20.
    c.above(
        "N2 B20 |lambda_2/(4 lambda_1) - 1|",
        (r.factors[1] / (4.0 * r.factors[0]) - 1.0).abs(),
        0.02,
    );
    // N3: the quadratic without the mode index (k = pi/L) for n = 2.
    c.above("N3 B20 n = 2 at k = pi/L", off(r.factors[1], lower_root(length, 1, true)), 0.01);
    c.finish();
}

#[test]
fn a3_c1_c2_c3_c4_knife_edge_beam_l_over_h_40() {
    let mut c = Checks::new();
    let length = 4.0;
    let (model, r) = buckle("B40", length, 80);
    two_factors(&mut c, "B40", &r);
    let ratio = r.factors[1] / (4.0 * r.factors[0]);
    c.at_most("A3 B40 |lambda_2/(4 lambda_1) - 1|", (ratio - 1.0).abs(), 0.02);
    let p1 = lower_root(length, 1, true);
    let p2 = lower_root(length, 2, true);
    println!(
        "closed form: P_1 = {p1:.6e} N, P_2 = {p2:.6e} N, P_2/(4 P_1) = {:.6}",
        p2 / (4.0 * p1)
    );
    c.at_most("C1 B40 n = 1", off(r.factors[0], p1), 1e-3);
    c.at_most("C1 B40 n = 2", off(r.factors[1], p2), 1e-3);
    for n in 1..=2 {
        let corr = identification(&model, &r.modes[n - 1], length, n);
        c.at_least(&format!("C2 B40 mode {n}"), corr, 0.99);
    }
    c.at_most(
        "C3 B40 |lambda_2/(4 lambda_1) - P_2/(4 P_1)|",
        (ratio - p2 / (4.0 * p1)).abs(),
        1e-3,
    );
    // C4: B40's second factor has the k of B20's first.
    let (_, r20) = buckle("B20 (for C4)", 2.0, 40);
    two_factors(&mut c, "B20 (for C4)", &r20);
    c.at_most(
        "C4 |lambda_2,B40/lambda_1,B20 - 1|",
        (r.factors[1] / r20.factors[0] - 1.0).abs(),
        1e-3,
    );
    c.finish();
}
