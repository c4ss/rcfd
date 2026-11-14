//! F-554 (ADR 0484): the zero-speed split frequencies of ADR 0472 against the Rayleigh quotients of the F-396 rest modes, and the
//! per-mode report of the difference between F-396's reported rest eigenvalues and those quotients. Checks Q1 to Q5, D1, X1 and
//! the controls N1, N2 of the registration. The set-ups are those of `sa_gyro.rs` (ADR 0472), copied.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::cyclic::full_rotor;
use rcfd_fem::hex20::{upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::modal::{ModalOptions, ModalResult};
use rcfd_fem::rotating::{RotatingOptions, RotatingPart, SpeedModes};
use rcfd_fem::{Constraint, ElasticSupport, FemError, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn le(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:.4e} (limit {limit:.0e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    fn gt(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{id}: {value:.4e} (expected > {limit:.0e}) {}",
            if ok { "as expected" } else { "NOT AS EXPECTED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {value:e} <= {limit:e}"));
        }
    }
    fn holds(&mut self, id: &str, ok: bool, what: &str) {
        println!("{id}: {what}: {}", if ok { "holds" } else { "FAILS" });
        if !ok {
            self.failed.push(format!("{id}: {what}"));
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "failed: {:#?}", self.failed);
    }
}
const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const AXIS: Vec3 = [0.0, 0.0, 1.0];
const ORIGIN: Vec3 = [0.0; 3];
const W0: f64 = 1000.0;
const H: f64 = 0.1;

fn key_of(e: FemError) -> &'static str {
    match e {
        FemError::InvalidValue { key, .. } => key,
        other => panic!("unexpected error {other:?}"),
    }
}

fn matvec(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

fn dot(x: &[f64], y: &[f64]) -> f64 {
    x.iter().zip(y).map(|(p, q)| p * q).sum()
}

fn modes_of(s: SpeedModes) -> ModalResult {
    match s {
        SpeedModes::Modes(r) => r,
        SpeedModes::LossOfStiffness(l) => panic!("defective set-up: loss of stiffness {l:?}"),
    }
}
fn cube(ky: f64) -> SolidModel {
    let mesh = hex_box([-H / 2.0; 3], [H; 3], [1, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, 0.3, RHO, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    let kn = RHO * H * H * H * W0 * W0 / 8.0;
    m.add_support(ElasticSupport::nodal_spring("all", 0, kn)).unwrap();
    m.add_support(ElasticSupport::nodal_spring("all", 1, ky * kn)).unwrap();
    m
}

const RR: f64 = 1.0;
const HR: f64 = 0.05;
const BR: f64 = 0.05;
const NS: usize = 64;

fn ring_material() -> Material {
    Material::isotropic_elastic(E, 0.0, RHO, 0.0).unwrap()
}

fn ring_sector_mesh() -> SolidMesh {
    let alpha = 2.0 * PI / NS as f64;
    let b = hex_box([RR - HR / 2.0, 0.0, -BR / 2.0], [HR, alpha, BR], [1, 1, 1]).unwrap();
    upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0
}

fn ring_sector(clamped: bool) -> SolidModel {
    let mut m = SolidModel::new(ring_sector_mesh());
    m.add_material("all", ring_material()).unwrap();
    if clamped {
        for c in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
    }
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, ORIGIN, NS))
        .unwrap();
    m
}

/// The inextensional ring frequency of mode `n`, rad/s (ADR 0472 set-up R).
fn ring_closed_form(n: f64) -> f64 {
    let ei_rho_a = E * HR * HR / (12.0 * RHO);
    (ei_rho_a / RR.powi(4) * n * n * (n * n - 1.0).powi(2) / (n * n + 1.0)).sqrt()
}

fn ring() -> SolidModel {
    let rotor = full_rotor(&ring_sector(false)).unwrap();
    let mesh = rotor.mesh().clone();
    let nodes = mesh.nodes().len();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", ring_material()).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    let mass = RHO * 2.0 * PI * RR * HR * BR;
    let kn = mass * (0.2 * ring_closed_form(2.0)).powi(2) / nodes as f64;
    m.add_support(ElasticSupport::nodal_spring("all", 0, kn)).unwrap();
    m.add_support(ElasticSupport::nodal_spring("all", 1, kn)).unwrap();
    m
}

fn ring_options(coriolis: bool) -> RotatingOptions {
    RotatingOptions::new(9)
        .modal(
            ModalOptions::new(9)
                .tolerance(1e-10)
                .residual_test(ResidualTest::Backward),
        )
        .coriolis(coriolis)
}

fn beam() -> SolidModel {
    let base = hex_box([0.0, -0.025, -0.025], [1.0, 0.05, 0.05], [20, 2, 2]).unwrap();
    let mesh = upgrade_to_hex20(&base).unwrap().0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

fn beam_options(modes: usize, threads: Option<usize>) -> RotatingOptions {
    let mut mo = ModalOptions::new(modes)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-12);
    if let Some(t) = threads {
        mo = mo.threads(t);
    }
    RotatingOptions::new(modes).modal(mo).coriolis(true)
}

/// The test's own quotient `phi^T K phi / phi^T B phi` (`B = M`, or the identity when `b` is `None`).
fn quotient(k: &CsrMatrix, b: Option<&CsrMatrix>, phi: &[f64]) -> f64 {
    let num = dot(phi, &matvec(k, phi));
    let den = match b {
        Some(m) => dot(phi, &matvec(m, phi)),
        None => dot(phi, phi),
    };
    num / den
}

fn eigen_bits(r: &ModalResult) -> (Vec<u64>, Vec<Vec<u64>>) {
    (
        r.eigenvalues.iter().map(|v| v.to_bits()).collect(),
        r.modes
            .iter()
            .map(|m| m.iter().map(|v| v.to_bits()).collect())
            .collect(),
    )
}

/// Q1 to Q5, D1, N1, N2 on one part built with the option on. Returns nothing; failures are collected.
fn run(ck: &mut Checks, what: &str, on: &RotatingPart, off: &RotatingPart, q4_envelope: f64, n1_limit: Option<f64>) {
    let before = eigen_bits(&modes_of(on.modes(0.0).unwrap()));
    let r = modes_of(on.modes(0.0).unwrap());
    let s = on.split_modes(0.0).unwrap();
    let report = on.rest_quotients().unwrap();
    // D1: the helper and the split solve leave the F-396 rest modes bit for bit; the option does not change them
    let after = eigen_bits(&modes_of(on.modes(0.0).unwrap()));
    let off_bits = eigen_bits(&modes_of(off.modes(0.0).unwrap()));
    ck.holds(
        &format!("D1 {what} F-396 rest eigenvalues and modes before = after the helper; option on = off"),
        before == after && before.0 == off_bits.0,
        "bits",
    );
    // Q1
    ck.holds(
        &format!("Q1 {what} counts split = F-396 = report"),
        s.modes.len() == r.eigenvalues.len() && report.len() == r.eigenvalues.len(),
        &format!("{} / {} / {}", s.modes.len(), r.eigenvalues.len(), report.len()),
    );
    // the test's own quotients
    let k = on.operator(0.0);
    let m = on.mass();
    let mut rho: Vec<f64> = r.modes.iter().map(|p| quotient(&k, Some(m), p)).collect();
    let q3 = report
        .iter()
        .zip(&rho)
        .map(|(h, q)| (h.quotient / q - 1.0).abs())
        .fold(0.0, f64::max);
    ck.le(&format!("Q3 {what} helper quotient against the test's own"), q3, 1e-13);
    rho.sort_by(|a, b| a.partial_cmp(b).unwrap());
    // Q2
    let q2 = s
        .modes
        .iter()
        .zip(&rho)
        .map(|(x, q)| (x.omega / q.sqrt() - 1.0).abs())
        .fold(0.0, f64::max);
    ck.le(
        &format!("Q2 {what} max |omega_split/sqrt(rho) - 1| at zero speed"),
        q2,
        1e-10,
    );
    // Q4: the report per mode
    println!("  {what} per-mode report (index, F-396 lambda, Rayleigh quotient, rho/lambda - 1):");
    for h in &report {
        println!(
            "    {:3} {:.12e} {:.12e} {:+.3e}",
            h.index, h.eigenvalue, h.quotient, h.relative_difference
        );
    }
    let worst = report.iter().map(|h| h.relative_difference.abs()).fold(0.0, f64::max);
    ck.le(
        &format!("Q4 {what} largest |rho/lambda - 1| of the report"),
        worst,
        q4_envelope,
    );
    // Q5
    ck.holds(
        &format!("Q5 {what} report: eigenvalue bits, index order, quotient > 0"),
        report
            .iter()
            .enumerate()
            .all(|(i, h)| h.index == i && h.eigenvalue.to_bits() == r.eigenvalues[i].to_bits() && h.quotient > 0.0),
        "each",
    );
    // N1: the reference matters (ADR 0472 Z1)
    let old = s
        .modes
        .iter()
        .zip(&r.eigenvalues)
        .map(|(x, l)| (x.omega / l.sqrt() - 1.0).abs())
        .fold(0.0, f64::max);
    println!("  N1 {what}: old comparison against sqrt(lambda_F396): {old:.4e}");
    if let Some(lim) = n1_limit {
        ck.gt(&format!("N1 {what} split against sqrt(lambda_F396)"), old, lim);
    }
}

#[test]
fn q_zero_speed_quotients_cube() {
    let mut ck = Checks::new();
    let cb = cube(1.0);
    let on = RotatingPart::new(&cb, AXIS, ORIGIN, &RotatingOptions::new(3).coriolis(true)).unwrap();
    let off = RotatingPart::new(&cb, AXIS, ORIGIN, &RotatingOptions::new(3)).unwrap();
    run(&mut ck, "CB", &on, &off, 1e-11, None);
    // N2: no mass weighting
    let r = modes_of(on.modes(0.0).unwrap());
    let s = on.split_modes(0.0).unwrap();
    let k = on.operator(0.0);
    let mut wrong: Vec<f64> = r.modes.iter().map(|p| quotient(&k, None, p)).collect();
    wrong.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n2 = s
        .modes
        .iter()
        .zip(&wrong)
        .map(|(x, q)| (x.omega / q.sqrt() - 1.0).abs())
        .fold(0.0, f64::max);
    ck.gt(
        "N2 CB quotient without the mass weighting against the split frequencies",
        n2,
        1e-3,
    );
    ck.finish();
}

#[test]
fn q_zero_speed_quotients_ring_cyclic_refusal() {
    let mut ck = Checks::new();
    let r = ring();
    let on = RotatingPart::new(&r, AXIS, ORIGIN, &ring_options(true)).unwrap();
    let off = RotatingPart::new(&r, AXIS, ORIGIN, &ring_options(false)).unwrap();
    run(&mut ck, "R", &on, &off, 5e-9, Some(1e-10));
    // X1: the clamped cyclic sector of R (ADR 0472 X1)
    let sector = ring_sector(true);
    let part = RotatingPart::new(&sector, AXIS, ORIGIN, &ring_options(true)).unwrap();
    let key = part.rest_quotients().map(|_| "none").unwrap_or_else(key_of);
    ck.holds("X1 rest_quotients on a cyclic model is refused", key == "cyclic", key);
    ck.finish();
}

#[test]
fn q_zero_speed_quotients_beam() {
    let mut ck = Checks::new();
    let f = beam();
    let on = RotatingPart::new(&f, AXIS, ORIGIN, &beam_options(14, None)).unwrap();
    let off = RotatingPart::new(&f, AXIS, ORIGIN, &beam_options(14, None).coriolis(false)).unwrap();
    run(&mut ck, "F", &on, &off, 1e-9, None);
    ck.finish();
}
