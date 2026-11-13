//! F-498 (ADR 0408): the flap-lag identity of a rotating solid beam restated as `1 - 2 f_x`, and the radial bar's clause.
//! Checks S1 to S3, B1 to B3 and the controls N1 to N4 of the registration. Test code only; the set-ups are copies of those
//! of `si_rotating.rs` (ADR 0403).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::modal::{ModalOptions, ModalResult};
use rcfd_fem::rotating::{RotatingOptions, RotatingPart, SpeedModes};
use rcfd_fem::{Constraint, Material, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::CsrMatrix;

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
            "{name}: {value:.4e} (expected < {limit:e}) {}",
            if ok { "as expected" } else { "NOT AS EXPECTED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} >= {limit:e}"));
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
    #[allow(dead_code)]
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

// set-ups

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const AXIS: Vec3 = [0.0, 0.0, 1.0];
const ORIGIN: Vec3 = [0.0; 3];

fn material() -> Material {
    Material::isotropic_elastic(E, 0.0, RHO, 0.0).unwrap()
}

/// B: the radial bar, axial motion only.
fn bar() -> SolidModel {
    let mesh = hex_box([0.0, -0.025, -0.025], [1.0, 0.05, 0.05], [40, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m
}

/// F (h_y = h_z = 0.05) and R (h_y = 0.06, h_z = 0.05): the radial beam, Hex20, clamped at x = 0.
fn beam(hy: f64, hz: f64) -> SolidModel {
    let base = hex_box([0.0, -hy / 2.0, -hz / 2.0], [1.0, hy, hz], [20, 2, 2]).unwrap();
    let mesh = upgrade_to_hex20(&base).unwrap().0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

fn beam_options() -> RotatingOptions {
    RotatingOptions::new(4).modal(
        ModalOptions::new(4)
            .residual_test(ResidualTest::Backward)
            .tolerance(1e-12),
    )
}

/// `phi^T M_c phi / phi^T M phi`: the share of the kinetic energy in component `c` (M is diagonal in the components).
fn share(model: &SolidModel, m: &CsrMatrix, phi: &[f64], c: usize) -> f64 {
    let dofs = model.dof_map();
    let (mut part, mut all) = (0.0, 0.0);
    for r in 0..m.n {
        let rc = dofs.node_component(r).unwrap().1;
        for p in m.row_ptr[r]..m.row_ptr[r + 1] {
            let v = m.val[p] * phi[r] * phi[m.col[p]];
            all += v;
            if rc == c {
                part += v;
            }
        }
    }
    part / all
}

fn modes_of(s: SpeedModes) -> ModalResult {
    match s {
        SpeedModes::Modes(r) => r,
        SpeedModes::LossOfStiffness(l) => panic!("unexpected loss of stiffness {l:?}"),
    }
}

/// (lambda_flap, lambda_lag, flap mode, lag mode) of a result: the lowest mode with a z share above 0.5 and the lowest with
/// a y share above 0.5.
fn flap_lag(model: &SolidModel, m: &CsrMatrix, r: &ModalResult) -> (f64, f64, Vec<f64>, Vec<f64>) {
    let pick = |c: usize| {
        (0..r.modes.len())
            .find(|&i| share(model, m, &r.modes[i], c) > 0.5)
            .unwrap_or_else(|| panic!("no mode with a share above 0.5 in component {c}"))
    };
    let (f, l) = (pick(2), pick(1));
    (
        r.eigenvalues[f],
        r.eigenvalues[l],
        r.modes[f].clone(),
        r.modes[l].clone(),
    )
}
/// `(lambda_flap - lambda_lag)/Omega^2`, `f_x,h` and `Omega` of F at `0.2 omega_0` (as `flap_lag_at` of `si_rotating.rs`).
fn flap_lag_at(options: &RotatingOptions) -> (f64, f64, f64) {
    let f = beam(0.05, 0.05);
    let part = RotatingPart::new(&f, AXIS, ORIGIN, options).unwrap();
    let rest = modes_of(part.modes(0.0).unwrap());
    let omega = 0.2 * rest.eigenvalues[0].sqrt();
    let r = modes_of(part.modes(omega).unwrap());
    let (lf, ll, phi_f, _) = flap_lag(&f, part.mass(), &r);
    let fx = share(&f, part.mass(), &phi_f, 0);
    ((lf - ll) / (omega * omega), fx, omega)
}

#[test]
fn s1_s2_s3_n1_to_n3_flap_lag_identity_of_the_solid_beam() {
    let mut ck = Checks::new();
    let (d, fx, omega) = flap_lag_at(&beam_options());
    println!(
        "Omega = {omega:.6} rad/s, d = {d:.10}, f_x,h = {fx:.6e}, 1 - 2 f_x,h = {:.10}",
        1.0 - 2.0 * fx
    );
    ck.at_most("S1 |d - (1 - 2 f_x,h)|", (d - (1.0 - 2.0 * fx)).abs(), 1e-4);
    ck.at_most("S2 |d - 1| against 3 f_x,h", (d - 1.0).abs(), 3.0 * fx);
    ck.at_most("S3 |f_x,h/9.683e-4 - 1|", (fx / 9.683e-4 - 1.0).abs(), 5e-2);
    ck.above(
        "N1 |d - 1| (the Euler-Bernoulli limit 1e-4 is missed)",
        (d - 1.0).abs(),
        1e-3,
    );
    let (d2, fx2, _) = flap_lag_at(&beam_options().spin_softening(false));
    println!("  spin softening off: d = {d2:.8e}, f_x,h = {fx2:.4e}");
    ck.above(
        "N2 |d - (1 - 2 f_x,h)| without spin softening",
        (d2 - (1.0 - 2.0 * fx2)).abs(),
        0.5,
    );
    let (d3, fx3, _) = flap_lag_at(&beam_options().stress_stiffening(false));
    println!("  stress stiffening off: d = {d3:.8}, f_x,h = {fx3:.4e}");
    ck.above(
        "N3 |d - (1 - 2 f_x,h)| without stress stiffening",
        (d3 - (1.0 - 2.0 * fx3)).abs(),
        5e-4,
    );
    ck.finish();
}

#[test]
fn b1_b2_b3_n4_radial_bar_clause() {
    let mut ck = Checks::new();
    let b = bar();
    let omega = (1e-4 * E / RHO).sqrt();
    let w2 = omega * omega;
    let off = RotatingOptions::new(2).stress_stiffening(false);
    let full = RotatingOptions::new(2);
    let p_off = RotatingPart::new(&b, AXIS, ORIGIN, &off).unwrap();
    let p_full = RotatingPart::new(&b, AXIS, ORIGIN, &full).unwrap();
    let rest = modes_of(p_full.modes(0.0).unwrap());
    let r_off = modes_of(p_off.modes(omega).unwrap());
    let r_full = modes_of(p_full.modes(omega).unwrap());
    let length = 1.0;
    for n in 0..2 {
        let l0 = rest.eigenvalues[n];
        let ratio = (l0 - r_off.eigenvalues[n]) / w2;
        ck.at_most(
            &format!("B1 n={} |(w0^2 - w^2)/W^2 - 1|", n + 1),
            (ratio - 1.0).abs(),
            1e-3,
        );
        let qh = p_full.stiffening_quotient(&rest.modes[n]);
        let slope = (r_full.eigenvalues[n] - l0) / w2;
        println!("  n={}: Q_n,h = {qh:.8}, slope {slope:.8}", n + 1);
        ck.at_most(
            &format!("B2 n={} |slope - (Q_n,h - 1)|", n + 1),
            (slope - (qh - 1.0)).abs(),
            1e-3,
        );
        let k = (2.0 * (n as f64) + 1.0) * std::f64::consts::PI / (2.0 * length);
        let q_closed = k * k * length * length / 3.0 + 0.25;
        ck.at_most(
            &format!("B3 n={} |Q_n,h/(k^2 L^2/3 + 1/4) - 1|", n + 1),
            (qh / q_closed - 1.0).abs(),
            1e-2,
        );
        if n == 0 {
            ck.below(
                "N4 (w0^2 - w^2)/W^2 of the full operator, n=1",
                (l0 - r_full.eigenvalues[0]) / w2,
                0.0,
            );
        }
    }
    ck.finish();
}
