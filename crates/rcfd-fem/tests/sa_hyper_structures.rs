//! F-645 (ADR 0584): hyperelastic materials on Beam2, Shell4 and Shell3 on the linear paths, opted in by
//! `SolidModel::set_hyperelastic_structures`. Checks S, M (static and modes against the isotropic twin of the same small-strain
//! moduli), Bb (beam buckling), the controls C and N1 to N4 of the registration. Set-ups of ADR 0584 D3. Every value is printed;
//! a non-finite value or an error is a miss.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::mat_hyper::{self, LINEAR_FOLLOW_UP};
use rcfd_fem::material::HyperelasticEnergy;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;

/// Measured values against their limits; every value is printed; a non-finite value is a miss; the test fails at the end if
/// any missed.
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
            self.0.push(format!("{name} = {value:e} not <= {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value > limit;
        println!(
            "{name}: {value:.6e} (expected > {limit:e}) {}",
            if ok { "as expected" } else { "NOT AS EXPECTED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not > {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn miss(&mut self, name: &str, detail: &str) {
        println!("{name}: {detail} MISSED");
        self.0.push(format!("{name}: {detail}"));
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

// ----------------------------------------------------------------------------------------------- energies (D3)

const RHO: f64 = 1000.0;

fn nhc() -> HyperelasticEnergy {
    HyperelasticEnergy::neo_hookean(1e6, 1e7).unwrap()
}
fn mrs() -> HyperelasticEnergy {
    HyperelasticEnergy::mooney_rivlin(2.5e5, 1.5e5, 4e6).unwrap()
}
/// NHc with `G (1 + 1e-6)` (the controls C).
fn nhc_perturbed() -> HyperelasticEnergy {
    HyperelasticEnergy::neo_hookean(1e6 * (1.0 + 1e-6), 1e7).unwrap()
}

fn hyper(e: HyperelasticEnergy) -> Material {
    Material::hyperelastic(e, RHO).unwrap()
}

/// The isotropic twin of an energy: `E = 9 K G / s`, `nu = (3K - 2G) / (2 s)`, `s = 3K + G` (ADR 0584 D2, D3).
fn twin(e: &HyperelasticEnergy) -> Material {
    let (g, k) = mat_hyper::small_strain_moduli(e);
    let s = 3.0 * k + g;
    Material::isotropic_elastic(9.0 * k * g / s, (3.0 * k - 2.0 * g) / (2.0 * s), RHO, 0.0).unwrap()
}

// ----------------------------------------------------------------------------------------------- set-ups (D3)

/// Beam B: Beam2 along `x`, `L = 1`, `n` elements, `rectangle_section(0.05, 0.03, 2.6e-7, 5/6)`, orientation `[0, 1, 0]`;
/// node sets `all`, `x0`, `xl`; element set `all`.
fn beam_mesh(n: usize) -> SolidMesh {
    let nodes: Vec<Vec3> = (0..=n).map(|i| [i as f64 / n as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    let section = beam::rectangle_section(0.05, 0.03, 2.6e-7, 5.0 / 6.0).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("all", (0..=n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    mesh
}

/// Beam B clamped at `x0`, with the material; `loads` adds the static loads of D3.
fn beam_b(material: Material, opt_in: bool, loads: bool) -> SolidModel {
    let mut m = SolidModel::new(beam_mesh(10));
    m.add_material("all", material).unwrap();
    m.set_hyperelastic_structures(opt_in);
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    if loads {
        m.add_load(Load::nodal_force("xl", [0.1, 0.2, 0.3])).unwrap();
        m.add_load(Load::nodal_moment("xl", [0.01, 0.02, 0.03])).unwrap();
        m.add_load(Load::gravity([0.0, 0.0, -9.81])).unwrap();
    }
    m
}

/// Plate P4 (`tri = false`) or P3: `[0, 1] x [0, 0.6]`, `10 x 6` cells, `shell(0.01)`, the column `x = 0` clamped, the corner
/// `(1, 0.6)` set `c`; `loads` adds the static loads of D3.
fn plate(material: Material, tri: bool, opt_in: bool, loads: bool) -> SolidModel {
    let (nx, ny, lx, ly) = (10usize, 6usize, 1.0, 0.6);
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            nodes.push([lx * i as f64 / nx as f64, ly * j as f64 / ny as f64, 0.0]);
        }
    }
    let mut conn = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            let (a, b, cc, d) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
            if tri {
                conn.extend_from_slice(&[a, b, cc, a, cc, d]);
            } else {
                conn.extend_from_slice(&[a, b, cc, d]);
            }
        }
    }
    let ne = if tri { 2 * nx * ny } else { nx * ny };
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(if tri { ElementKind::Shell3 } else { ElementKind::Shell4 }, conn)
        .unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("x0", (0..=ny).map(|j| idx(0, j)).collect()).unwrap();
    mesh.add_node_set("c", vec![idx(nx, ny)]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.set_hyperelastic_structures(opt_in);
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    if loads {
        m.add_load(Load::nodal_force("c", [0.1, 0.2, 0.3])).unwrap();
        m.add_load(Load::nodal_moment("c", [0.01, 0.02, 0.005])).unwrap();
        m.add_load(Load::gravity([0.0, 0.0, -9.81])).unwrap();
    }
    m
}

fn modal_opts() -> ModalOptions {
    ModalOptions::new(6).residual_test(ResidualTest::Backward)
}

// ----------------------------------------------------------------------------------------------- measures

/// `max_i |a_i - b_i| / max_i |b_i|`, NaN if any value is not finite.
fn rel(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.iter().chain(b).any(|v| !v.is_finite()) {
        return f64::NAN;
    }
    let d = a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
    let s = b.iter().map(|y| y.abs()).fold(0.0, f64::max);
    d / s
}

/// `max_k |a_k / b_k - 1|`, NaN if any value is not finite or the lengths differ.
fn ratio(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() || a.iter().chain(b).any(|v| !v.is_finite()) {
        return f64::NAN;
    }
    a.iter().zip(b).map(|(x, y)| (x / y - 1.0).abs()).fold(0.0, f64::max)
}

/// `max_k |phi_h,k - s_k phi_t,k|_inf / |phi_t,k|_inf`, `s_k = sign(phi_h,k . phi_t,k)`.
fn shapes(a: &[Vec<f64>], b: &[Vec<f64>]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return f64::NAN;
    }
    let mut worst: f64 = 0.0;
    for (x, y) in a.iter().zip(b) {
        let dot: f64 = x.iter().zip(y).map(|(p, q)| p * q).sum();
        let s = if dot < 0.0 { -1.0 } else { 1.0 };
        let neg: Vec<f64> = y.iter().map(|q| s * q).collect();
        let e = rel(x, &neg);
        if !e.is_finite() {
            return f64::NAN;
        }
        worst = worst.max(e);
    }
    worst
}

/// The static rows: `e_u` of the hyperelastic model against the twin model.
fn static_row(c: &mut Checks, name: &str, h: &SolidModel, t: &SolidModel, limit: f64, above: bool) {
    match (
        solve_static(h, &StaticOptions::default()),
        solve_static(t, &StaticOptions::default()),
    ) {
        (Ok(a), Ok(b)) => {
            let e = rel(a.displacement_values(), b.displacement_values());
            if above {
                c.above(&format!("{name} e_u"), e, limit);
            } else {
                c.at_most(&format!("{name} e_u"), e, limit);
            }
        }
        (a, b) => c.miss(name, &format!("solve_static: {:?} / {:?}", a.err(), b.err())),
    }
}

/// The modal rows: `e_lambda` (and `e_phi` unless a control) of the hyperelastic model against the twin model.
fn modal_row(c: &mut Checks, name: &str, h: &SolidModel, t: &SolidModel, limit: f64, control: bool) {
    match (modal_analysis(h, &modal_opts()), modal_analysis(t, &modal_opts())) {
        (Ok(a), Ok(b)) => {
            println!(
                "{name} iterations {} / {}, residuals max {:.3e} / {:.3e}, lambda {:?}",
                a.iterations,
                b.iterations,
                a.residuals.iter().copied().fold(0.0, f64::max),
                b.residuals.iter().copied().fold(0.0, f64::max),
                a.eigenvalues
            );
            let el = ratio(&a.eigenvalues, &b.eigenvalues);
            if control {
                c.above(&format!("{name} e_lambda"), el, limit);
            } else {
                c.at_most(&format!("{name} e_lambda"), el, limit);
                c.at_most(&format!("{name} e_phi"), shapes(&a.modes, &b.modes), limit);
            }
        }
        (a, b) => c.miss(name, &format!("modal_analysis: {:?} / {:?}", a.err(), b.err())),
    }
}

fn linear_refusal<T>(r: &rcfd_fem::Result<T>, what: &str) -> bool {
    matches!(r, Err(FemError::NotYetSupported { variant, package, location, .. })
        if *variant == "Hyperelastic" && *package == LINEAR_FOLLOW_UP && location.contains(what))
}

// ----------------------------------------------------------------------------------------------- acceptance

#[test]
fn s_m_beam_b() {
    let mut c = Checks::new();
    for (name, en) in [("NH", nhc()), ("MR", mrs())] {
        let h = beam_b(hyper(en), true, true);
        let t = beam_b(twin(&en), false, true);
        static_row(&mut c, &format!("S-B-{name}"), &h, &t, 1e-10, false);
        let h = beam_b(hyper(en), true, false);
        let t = beam_b(twin(&en), false, false);
        modal_row(&mut c, &format!("M-B-{name}"), &h, &t, 1e-10, false);
    }
    c.finish();
}

#[test]
fn s_m_plate_p4() {
    let mut c = Checks::new();
    for (name, en) in [("NH", nhc()), ("MR", mrs())] {
        let h = plate(hyper(en), false, true, true);
        let t = plate(twin(&en), false, false, true);
        static_row(&mut c, &format!("S-P4-{name}"), &h, &t, 1e-10, false);
        let h = plate(hyper(en), false, true, false);
        let t = plate(twin(&en), false, false, false);
        modal_row(&mut c, &format!("M-P4-{name}"), &h, &t, 1e-10, false);
    }
    c.finish();
}

// ----------------------------------------------------------------------------------------------- registered extras

#[test]
fn s_m_plate_p3() {
    let mut c = Checks::new();
    for (name, en) in [("NH", nhc()), ("MR", mrs())] {
        let h = plate(hyper(en), true, true, true);
        let t = plate(twin(&en), true, false, true);
        static_row(&mut c, &format!("S-P3-{name}"), &h, &t, 1e-10, false);
        let h = plate(hyper(en), true, true, false);
        let t = plate(twin(&en), true, false, false);
        modal_row(&mut c, &format!("M-P3-{name}"), &h, &t, 1e-10, false);
    }
    c.finish();
}

/// Beam Bb: 8 elements, planar and pinned (B1 of `sa_geometric_struct.rs`).
fn beam_bb(material: Material, opt_in: bool) -> SolidModel {
    let mut m = SolidModel::new(beam_mesh(8));
    m.add_material("all", material).unwrap();
    m.set_hyperelastic_structures(opt_in);
    for comp in [2, 3, 4] {
        m.add_constraint(Constraint::displacement("all", comp, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::displacement("x0", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("x0", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xl", 1, 0.0)).unwrap();
    m
}

fn buckling_factors(m: &SolidModel) -> rcfd_fem::Result<Vec<f64>> {
    let mut loaded = m.clone();
    loaded.add_load(Load::nodal_force("xl", [-1.0, 0.0, 0.0]))?;
    let r = solve_static(&loaded, &StaticOptions::default())?;
    let pre = Prestress::from_static(&loaded, &r)?;
    Ok(factors(m, &pre, &BucklingOptions::new(2))?.factors)
}

#[test]
fn bb_beam_buckling() {
    let mut c = Checks::new();
    let h = beam_bb(hyper(nhc()), true);
    let t = beam_bb(twin(&nhc()), false);
    match (buckling_factors(&h), buckling_factors(&t)) {
        (Ok(a), Ok(b)) => {
            println!("Bb factors {a:?} / {b:?}");
            c.at_most("Bb-NH e_f", ratio(&a, &b), 1e-10);
        }
        (a, b) => c.miss("Bb-NH", &format!("{:?} / {:?}", a.err(), b.err())),
    }
    c.finish();
}

// ----------------------------------------------------------------------------------------------- controls

#[test]
fn c_perturbed_twin() {
    let mut c = Checks::new();
    let p = nhc_perturbed();
    static_row(
        &mut c,
        "C-B static",
        &beam_b(hyper(nhc()), true, true),
        &beam_b(twin(&p), false, true),
        1e-8,
        true,
    );
    modal_row(
        &mut c,
        "C-B modes",
        &beam_b(hyper(nhc()), true, false),
        &beam_b(twin(&p), false, false),
        1e-8,
        true,
    );
    for (name, tri) in [("C-P4", false), ("C-P3", true)] {
        static_row(
            &mut c,
            &format!("{name} static"),
            &plate(hyper(nhc()), tri, true, true),
            &plate(twin(&p), tri, false, true),
            1e-8,
            true,
        );
        modal_row(
            &mut c,
            &format!("{name} modes"),
            &plate(hyper(nhc()), tri, true, false),
            &plate(twin(&p), tri, false, false),
            1e-8,
            true,
        );
    }
    c.finish();
}

#[test]
fn n1_to_n4_settings_and_refusals() {
    let mut c = Checks::new();
    // N1: without the setting the refusal of ADR 0555 D3 stands.
    let models = [
        ("Beam2", beam_b(hyper(nhc()), false, true)),
        ("Shell4", plate(hyper(nhc()), false, false, true)),
        ("Shell3", plate(hyper(nhc()), true, false, true)),
    ];
    for (name, m) in &models {
        let r = solve_static(m, &StaticOptions::default());
        c.holds(
            &format!("N1 {name} solve_static refused"),
            linear_refusal(&r, "a structural element"),
            &format!("{:?}", r.err()),
        );
        let r = modal_analysis(m, &modal_opts());
        c.holds(
            &format!("N1 {name} modal_analysis refused"),
            linear_refusal(&r, "a structural element"),
            &format!("{:?}", r.err()),
        );
    }
    // N2: with the setting a temperature load stays refused.
    let mut m = beam_b(hyper(nhc()), true, true);
    let nn = m.mesh().nodes().len();
    m.add_load(Load::temperature(vec![303.0; nn], 293.0)).unwrap();
    let r = solve_static(&m, &StaticOptions::default());
    c.holds(
        "N2 Beam2 with the setting and a temperature load refused",
        linear_refusal(&r, "a temperature load"),
        &format!("{:?}", r.err()),
    );
    // N3: the setting does not change an isotropic model.
    let iso = twin(&nhc());
    for (name, on, off) in [
        (
            "Beam2",
            beam_b(iso.clone(), true, true),
            beam_b(iso.clone(), false, true),
        ),
        (
            "Shell4",
            plate(iso.clone(), false, true, true),
            plate(iso.clone(), false, false, true),
        ),
    ] {
        let a = solve_static(&on, &StaticOptions::default()).unwrap();
        let b = solve_static(&off, &StaticOptions::default()).unwrap();
        let same = a
            .displacement_values()
            .iter()
            .zip(b.displacement_values())
            .all(|(x, y)| x.to_bits() == y.to_bits());
        c.holds(&format!("N3 {name} static bits"), same, "");
        let a = modal_analysis(&on, &modal_opts()).unwrap();
        let b = modal_analysis(&off, &modal_opts()).unwrap();
        let same = a
            .eigenvalues
            .iter()
            .zip(&b.eigenvalues)
            .all(|(x, y)| x.to_bits() == y.to_bits());
        c.holds(&format!("N3 {name} eigenvalue bits"), same, "");
    }
    // N4: the default.
    let m = SolidModel::new(beam_mesh(2));
    c.holds("N4 hyperelastic_structures() default", !m.hyperelastic_structures(), "");
    c.finish();
}
