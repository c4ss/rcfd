//! F-396 (ADR 0403): modes of a rotating part, stress stiffening by the centrifugal pre-stress, spin softening, mode
//! tracking and the Campbell table. Checks A1 to A11 and the negative controls N1 to N4 of the registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::modal::{modal_analysis, ModalOptions, ModalResult};
use rcfd_fem::rotating::{engine_order_crossings, mac, CampbellTable, RotatingOptions, RotatingPart, SpeedModes};
use rcfd_fem::{Constraint, ElasticSupport, FemError, Material, SolidModel};
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

// ------------------------------------------------------------------------------------------------------ set-ups

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const AXIS: Vec3 = [0.0, 0.0, 1.0];
const ORIGIN: Vec3 = [0.0; 3];
/// The continuum Rayleigh quotient of the centrifugal tension with the first cantilever mode (ADR 0403 arithmetic 4).
const Q_EB: f64 = 1.193336;

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

/// K: the block on a radial foundation, held in y and z.
fn block() -> SolidModel {
    let mesh = hex_box([0.1, -0.05, -0.05], [0.1, 0.1, 0.1], [4, 2, 2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_support(ElasticSupport::foundation("xmin", 2.0e9)).unwrap();
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

fn same_bits(a: &ModalResult, b: &ModalResult) -> bool {
    let v = |x: &[f64], y: &[f64]| x.len() == y.len() && x.iter().zip(y).all(|(p, q)| p.to_bits() == q.to_bits());
    v(&a.eigenvalues, &b.eigenvalues)
        && v(&a.frequencies, &b.frequencies)
        && v(&a.residuals, &b.residuals)
        && a.modes.len() == b.modes.len()
        && a.modes.iter().zip(&b.modes).all(|(p, q)| v(p, q))
        && a.iterations == b.iterations
        && a.shift.to_bits() == b.shift.to_bits()
        && a.sturm == b.sturm
        && a.unknowns == b.unknowns
        && a.rigid_free == b.rigid_free
}

// --------------------------------------------------------------------------------------------------------- A1

#[test]
fn a1_zero_speed_has_the_bits_of_the_modal_analysis() {
    let mut ck = Checks::new();
    let f = beam(0.05, 0.05);
    let opts = beam_options();
    let part = RotatingPart::new(&f, AXIS, ORIGIN, &opts).unwrap();
    let r0 = modes_of(part.modes(0.0).unwrap());
    let reference = modal_analysis(&f, &opts.modal).unwrap();
    ck.holds(
        "A1 F",
        same_bits(&r0, &reference),
        "speed 0 against modal_analysis, every field by to_bits",
    );
    let table = part.campbell(&[0.0, 10.0]).unwrap();
    let column: Vec<f64> = table.tracks.iter().map(|t| t.eigenvalues[0].unwrap()).collect();
    ck.holds(
        "A1b F",
        column
            .iter()
            .zip(&reference.eigenvalues)
            .all(|(a, b)| a.to_bits() == b.to_bits()),
        "the speed-0 column of the table against modal_analysis",
    );
    let k = block();
    let ko = RotatingOptions::new(2);
    let kp = RotatingPart::new(&k, AXIS, ORIGIN, &ko).unwrap();
    let kr = modes_of(kp.modes(0.0).unwrap());
    let kref = modal_analysis(&k, &ko.modal).unwrap();
    ck.holds(
        "A1 K",
        same_bits(&kr, &kref),
        "speed 0 on the transformation path, every field by to_bits",
    );
    ck.finish();
}

// --------------------------------------------------------------------------------------------------------- A2

#[test]
fn a2_radial_bar_axial_modes() {
    let mut ck = Checks::new();
    let b = bar();
    let omega = (1e-4 * E / RHO).sqrt();
    println!("Omega_B = {omega:.6} rad/s");
    let off = RotatingOptions::new(2).stress_stiffening(false);
    let full = RotatingOptions::new(2);
    let p_off = RotatingPart::new(&b, AXIS, ORIGIN, &off).unwrap();
    let p_full = RotatingPart::new(&b, AXIS, ORIGIN, &full).unwrap();
    let rest = modes_of(p_full.modes(0.0).unwrap());
    let r_off = modes_of(p_off.modes(omega).unwrap());
    let r_full = modes_of(p_full.modes(omega).unwrap());
    let w2 = omega * omega;
    let q_cf = [1.0724670, 7.6522033];
    for n in 0..2 {
        let l0 = rest.eigenvalues[n];
        let ratio = (l0 - r_off.eigenvalues[n]) / w2;
        ck.at_most(
            &format!("A2a n={} |(w0^2 - w^2)/W^2 - 1| (softening only)", n + 1),
            (ratio - 1.0).abs(),
            1e-3,
        );
        let qh = p_full.stiffening_quotient(&rest.modes[n]);
        let slope = (r_full.eigenvalues[n] - l0) / w2;
        println!("  n={}: Q_n,h = {qh:.8}, measured slope {slope:.8}", n + 1);
        ck.at_most(
            &format!("A2b n={} |slope - (Q_n,h - 1)|", n + 1),
            (slope - (qh - 1.0)).abs(),
            1e-3,
        );
        ck.at_most(
            &format!("A2c n={} |Q_n,h/Q_n - 1|", n + 1),
            (qh / q_cf[n] - 1.0).abs(),
            1e-2,
        );
        if n == 0 {
            let literal = (l0 - r_full.eigenvalues[0]) / w2;
            ck.below("N4 (w0^2 - w^2)/W^2 of the full operator, n=1", literal, 0.0);
        }
    }
    let smax = p_full
        .unit_prestress()
        .stress()
        .iter()
        .flat_map(|s| s.iter())
        .fold(0.0f64, |a, v| a.max(v.abs()));
    ck.below("A2d largest centrifugal stress at Omega_B / E", smax * w2 / E, 1e-4);
    // A9b: softening only, beyond the first axial frequency.
    let w_loss = 1.1 * rest.eigenvalues[0].sqrt();
    let loss = p_off.modes(w_loss).unwrap();
    ck.holds(
        "A9b",
        matches!(loss, SpeedModes::LossOfStiffness(l) if l.negative == Some(1) && l.speed.to_bits() == w_loss.to_bits()),
        &format!("{loss:?}"),
    );
    ck.finish();
}

// --------------------------------------------------------------------------------------------------------- A3

/// F at 0.2 omega_0: the flap-lag identity with both effects (and the part's mass), for A3, A3b.
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
#[ignore = "known miss of A3 (ADR 0403 addendum 1): |(w_flap^2 - w_lag^2)/W^2 - 1| = 1.9220e-3 against the limit 1e-4, predicted 1.94e-3 in the registration (arithmetic 3): the section rotation of both bending modes is softened (f_x = 9.61e-4) and the lateral centrifugal stress stiffens the lag mode by as much; A3b met at 1.1e-8"]
fn a3_flap_lag_identity_known_miss() {
    let mut ck = Checks::new();
    let (d, fx, omega) = flap_lag_at(&beam_options());
    println!("Omega = {omega:.6} rad/s, (w_flap^2 - w_lag^2)/W^2 = {d:.8}, f_x,h = {fx:.4e}");
    ck.at_most("A3 |(w_flap^2 - w_lag^2)/W^2 - 1|", (d - 1.0).abs(), 1e-4);
    ck.finish();
}

#[test]
fn a3b_n1_flap_lag_diagnosis() {
    let mut ck = Checks::new();
    let (d, fx, _) = flap_lag_at(&beam_options());
    println!(
        "(w_flap^2 - w_lag^2)/W^2 = {d:.8}, f_x,h = {fx:.4e}, 1 - 2 f_x,h = {:.8}",
        1.0 - 2.0 * fx
    );
    ck.at_most(
        "A3b |(w_flap^2 - w_lag^2)/W^2 - (1 - 2 f_x,h)|",
        (d - (1.0 - 2.0 * fx)).abs(),
        5e-4,
    );
    let (d1, _, _) = flap_lag_at(&beam_options().spin_softening(false));
    ck.below("N1 |(w_flap^2 - w_lag^2)/W^2| without spin softening", d1.abs(), 1e-2);
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------ A4, A5

/// The rest flap mode of F: in the degenerate pair, the M-normalised combination with the largest z share.
fn rest_flap(model: &SolidModel, m: &CsrMatrix, rest: &ModalResult) -> Vec<f64> {
    let (p, q) = (&rest.modes[0], &rest.modes[1]);
    let dofs = model.dof_map();
    let zdot = |a: &[f64], b: &[f64]| {
        let mut s = 0.0;
        for r in 0..m.n {
            if dofs.node_component(r).unwrap().1 == 2 {
                for k in m.row_ptr[r]..m.row_ptr[r + 1] {
                    s += m.val[k] * a[r] * b[m.col[k]];
                }
            }
        }
        s
    };
    let (a, b, c) = (zdot(p, p), zdot(p, q), zdot(q, q));
    // Largest eigenvector of [[a, b], [b, c]].
    let theta = 0.5 * (2.0 * b).atan2(a - c);
    let (cs, sn) = (theta.cos(), theta.sin());
    p.iter().zip(q).map(|(x, y)| cs * x + sn * y).collect()
}

#[test]
fn a4_a5_stiffening_of_the_flap_mode() {
    let mut ck = Checks::new();
    let f = beam(0.05, 0.05);
    let opts = beam_options();
    let part = RotatingPart::new(&f, AXIS, ORIGIN, &opts).unwrap();
    let rest = modes_of(part.modes(0.0).unwrap());
    let l0 = rest.eigenvalues[0];
    let w0 = l0.sqrt();
    println!(
        "omega_0 = {w0:.6} rad/s, lambda_0,1 = {l0:.10e}, lambda_0,2 = {:.10e}",
        rest.eigenvalues[1]
    );
    let phi_f = rest_flap(&f, part.mass(), &rest);
    let qh = part.stiffening_quotient(&phi_f);
    let slope = part.slope(&phi_f);
    println!(
        "Q_h = {qh:.8}, slope(phi_f,0) = {slope:.8}, z share {:.6}",
        share(&f, part.mass(), &phi_f, 2)
    );
    ck.at_most("A4b |Q_h/1.193336 - 1|", (qh / Q_EB - 1.0).abs(), 2e-2);
    let s_at = |ratio: f64, p: &RotatingPart| {
        let w = ratio * w0;
        let r = modes_of(p.modes(w).unwrap());
        let (lf, _, _, _) = flap_lag(&f, p.mass(), &r);
        (lf - l0) / (w * w)
    };
    let s001 = s_at(0.01, &part);
    ck.at_most("A4c |S(0.01 w0) - slope(phi_f,0)|", (s001 - slope).abs(), 1e-3);
    for ratio in [0.2, 0.5, 1.0, 2.0, 3.0, 5.0] {
        let s = s_at(ratio, &part);
        if ratio == 0.2 {
            ck.at_most("A4 |S(0.2 w0)/1.193336 - 1|", (s / Q_EB - 1.0).abs(), 2e-2);
        }
        ck.holds(
            &format!("A5 Omega = {ratio} w0"),
            (1.0..=qh).contains(&s),
            &format!("S = {s:.8} in [1, Q_h = {qh:.8}]"),
        );
    }
    let off = RotatingPart::new(&f, AXIS, ORIGIN, &beam_options().stress_stiffening(false)).unwrap();
    let s_off = s_at(0.2, &off);
    ck.below("N2 |S(0.2 w0)| without stress stiffening", s_off.abs(), 1e-2);
    ck.finish();
}

// --------------------------------------------------------------------------------------------------------- A6

#[test]
fn a6_synthetic_family_crossings() {
    let mut ck = Checks::new();
    let speeds: Vec<f64> = (0..=40).map(|j| 10.0 * j as f64).collect();
    for (w0, s) in [(100.0f64, 0.19f64), (100.0, 0.5), (250.0, -0.3)] {
        let lambda: Vec<Option<f64>> = speeds.iter().map(|w| Some(w0 * w0 + s * w * w)).collect();
        for k in 1..=4u32 {
            let c = engine_order_crossings(&speeds, &lambda, k);
            let exact = w0 / (f64::from(k * k) - s).sqrt();
            ck.holds(
                &format!("A6 ({w0}, {s}) k={k} count"),
                c.len() == 1,
                &format!("{} crossing(s)", c.len()),
            );
            if let Some(&x) = c.first() {
                ck.at_most(
                    &format!("A6 ({w0}, {s}) k={k} relative error"),
                    (x / exact - 1.0).abs(),
                    1e-8,
                );
            }
        }
    }
    ck.finish();
}

// ------------------------------------------------------------------------------------------------- A7, A10

/// The crossing of the flap (`c = 2`) or lag (`c = 1`) branch of F with order `k` by a direct solve: regula falsi
/// (Illinois) on `lambda(Omega) - k^2 Omega^2` in `[a, b]`, to 1e-10 relative in `Omega`.
fn direct_crossing(f: &SolidModel, part: &RotatingPart, c: usize, k: u32, mut a: f64, mut b: f64) -> f64 {
    let k2 = f64::from(k * k);
    let h = |w: f64| {
        let r = modes_of(part.modes(w).unwrap());
        let (lf, ll, _, _) = flap_lag(f, part.mass(), &r);
        (if c == 2 { lf } else { ll }) - k2 * w * w
    };
    let (mut fa, mut fb) = (h(a), h(b));
    assert!(fa * fb < 0.0, "no sign change in [{a}, {b}]");
    for _ in 0..100 {
        let x = b - fb * (b - a) / (fb - fa);
        let fx = h(x);
        if fx * fb < 0.0 {
            a = b;
            fa = fb;
        } else {
            fa *= 0.5;
        }
        b = x;
        fb = fx;
        if (b - a).abs() <= 1e-10 * b.abs() || fx == 0.0 {
            return x;
        }
    }
    panic!("regula falsi did not converge");
}

#[test]
fn a7_a10_beam_campbell_table() {
    let mut ck = Checks::new();
    let f = beam(0.05, 0.05);
    let part = RotatingPart::new(&f, AXIS, ORIGIN, &beam_options()).unwrap();
    let rest = modes_of(part.modes(0.0).unwrap());
    let w0 = rest.eigenvalues[0].sqrt();
    let speeds: Vec<f64> = (0..=12).map(|j| 0.1 * j as f64 * w0).collect();
    let table = part.campbell(&speeds).unwrap();
    // Track identity by the share at the second speed (the first is degenerate).
    let kind = |t: usize| {
        let phi = table.tracks[t].modes[1].as_ref().unwrap();
        if share(&f, part.mass(), phi, 2) > 0.5 {
            2
        } else {
            1
        }
    };
    let flap = (0..2).find(|&t| kind(t) == 2).expect("a flap track");
    let lag = (0..2).find(|&t| kind(t) == 1).expect("a lag track");
    for (track, c, name, orders) in [(flap, 2usize, "flap", 1..=4u32), (lag, 1usize, "lag", 1..=4u32)] {
        for k in orders {
            let cr: Vec<f64> = table
                .crossings(k)
                .iter()
                .filter(|x| x.track == track)
                .map(|x| x.speed)
                .collect();
            if c == 2 && k == 1 {
                ck.holds(
                    "A7 flap order 1",
                    cr.is_empty(),
                    &format!("{} crossing(s) (none expected)", cr.len()),
                );
                continue;
            }
            ck.holds(
                &format!("A7 {name} k={k} count"),
                cr.len() == 1,
                &format!("{} crossing(s)", cr.len()),
            );
            let Some(&x) = cr.first() else { continue };
            let j = speeds.iter().position(|&w| w > x).expect("inside the table");
            let reference = direct_crossing(&f, &part, c, k, speeds[j - 1], speeds[j]);
            println!(
                "  {name} k={k}: table {:.8} w0, direct {:.8} w0",
                x / w0,
                reference / w0
            );
            ck.at_most(&format!("A7 {name} k={k} relative"), (x / reference - 1.0).abs(), 1e-2);
        }
    }
    // A10: the CSV parses back to the bits of the table.
    let csv = table.to_csv();
    let mut ok = true;
    for (j, line) in csv.lines().skip(1).enumerate() {
        let cells: Vec<&str> = line.split(',').collect();
        ok &= cells[0].parse::<f64>().unwrap().to_bits() == speeds[j].to_bits();
        for (t, tr) in table.tracks.iter().enumerate() {
            ok &= cells[2 + t].parse::<f64>().unwrap().to_bits() == tr.frequencies[j].unwrap().to_bits();
        }
    }
    ck.holds(
        "A10 CSV",
        ok && csv.lines().count() == speeds.len() + 1,
        "parses back to the table by to_bits",
    );
    ck.finish();
}

// --------------------------------------------------------------------------------------------------------- A8

#[test]
fn a8_n3_tracking_through_a_flap_lag_crossing() {
    let mut ck = Checks::new();
    let r = beam(0.06, 0.05);
    let part = RotatingPart::new(&r, AXIS, ORIGIN, &beam_options()).unwrap();
    let rest = modes_of(part.modes(0.0).unwrap());
    let w0 = rest.eigenvalues[0].sqrt();
    println!(
        "omega_0f = {w0:.6} rad/s, lag/flap at rest {:.6}",
        (rest.eigenvalues[1] / rest.eigenvalues[0]).sqrt()
    );
    let speeds: Vec<f64> = (0..=12).map(|j| 0.1 * j as f64 * w0).collect();
    let table: CampbellTable = part.campbell(&speeds).unwrap();
    let m = part.mass();
    let (t0, t1) = (&table.tracks[0], &table.tracks[1]);
    let z_min = (0..speeds.len())
        .map(|j| share(&r, m, t0.modes[j].as_ref().unwrap(), 2))
        .fold(1.0f64, f64::min);
    let y_min = (0..speeds.len())
        .map(|j| share(&r, m, t1.modes[j].as_ref().unwrap(), 1))
        .fold(1.0f64, f64::min);
    ck.holds("A8 flap track z share", z_min > 0.9, &format!("min {z_min:.6} > 0.9"));
    ck.holds("A8 lag track y share", y_min > 0.9, &format!("min {y_min:.6} > 0.9"));
    let mac_min = (1..speeds.len())
        .flat_map(|j| [t0.mac[j].unwrap(), t1.mac[j].unwrap()])
        .fold(1.0f64, f64::min);
    ck.holds(
        "A8 MAC with the previous mode",
        mac_min > 0.9,
        &format!("min {mac_min:.6} > 0.9"),
    );
    let (f06, f07) = (6, 7);
    let order_before = t0.eigenvalues[f06].unwrap() < t1.eigenvalues[f06].unwrap();
    let order_after = t0.eigenvalues[f07].unwrap() > t1.eigenvalues[f07].unwrap();
    ck.holds(
        "A8 crossing inside the table",
        order_before && order_after,
        &format!(
            "flap/lag at 0.6: {:.6}, at 0.7: {:.6}",
            t0.eigenvalues[f06].unwrap() / t1.eigenvalues[f06].unwrap(),
            t0.eigenvalues[f07].unwrap() / t1.eigenvalues[f07].unwrap()
        ),
    );
    let a = modes_of(part.modes(speeds[f06]).unwrap());
    let b = modes_of(part.modes(speeds[f07]).unwrap());
    ck.below(
        "N3 MAC of mode 1 at 0.6 with mode 1 at 0.7 (by frequency order)",
        mac(m, &a.modes[0], &b.modes[0]),
        0.1,
    );
    ck.finish();
}

// --------------------------------------------------------------------------------------------------------- A9

#[test]
fn a9_loss_of_stiffness_with_the_speed() {
    let mut ck = Checks::new();
    let k = block();
    let part = RotatingPart::new(&k, AXIS, ORIGIN, &RotatingOptions::new(2)).unwrap();
    let rest = modes_of(part.modes(0.0).unwrap());
    let wr = rest.eigenvalues[0].sqrt();
    println!(
        "omega_r0 = {wr:.6} rad/s, lambda_2/lambda_1 at rest = {:.4}",
        rest.eigenvalues[1] / rest.eigenvalues[0]
    );
    let speeds: Vec<f64> = [0.5, 0.9, 1.1, 1.5].iter().map(|r| r * wr).collect();
    for (j, &w) in speeds.iter().enumerate() {
        let s = part.modes(w).unwrap();
        match (j < 2, &s) {
            (true, SpeedModes::Modes(r)) => {
                let ok = r.eigenvalues.iter().all(|l| l.is_finite() && *l > 0.0)
                    && r.frequencies.iter().all(|f| f.is_finite());
                ck.holds(
                    &format!("A9 speed {:.1} w_r0", w / wr),
                    ok,
                    &format!("eigenvalues {:?}", r.eigenvalues),
                );
            }
            (false, SpeedModes::LossOfStiffness(l)) => ck.holds(
                &format!("A9 speed {:.1} w_r0", w / wr),
                l.speed.to_bits() == w.to_bits() && l.negative.is_some_and(|n| n >= 1),
                &format!("{l:?}"),
            ),
            _ => ck.holds(&format!("A9 speed {:.1} w_r0", w / wr), false, &format!("{s:?}")),
        }
    }
    let table = part.campbell(&speeds).unwrap();
    let finite = table.speeds.iter().all(|w| w.is_finite())
        && table.tracks.iter().all(|t| {
            t.eigenvalues.iter().flatten().all(|v| v.is_finite())
                && t.frequencies.iter().flatten().all(|v| v.is_finite())
                && t.mac.iter().flatten().all(|v| v.is_finite())
        });
    let nones = table.tracks.iter().all(|t| {
        t.frequencies[0].is_some()
            && t.frequencies[1].is_some()
            && t.frequencies[2].is_none()
            && t.frequencies[3].is_none()
    });
    let listed = table.losses.len() == 2
        && table.losses[0].speed.to_bits() == speeds[2].to_bits()
        && table.losses[1].speed.to_bits() == speeds[3].to_bits();
    ck.holds(
        "A9 table",
        finite && nones && listed,
        &format!("losses {:?}", table.losses),
    );
    let csv = table.to_csv();
    let json = table.to_json(&[1, 2]);
    let bad = |s: &str| s.contains("NaN") || s.contains("inf");
    ck.holds(
        "A9 CSV and JSON finite",
        !bad(&csv) && !bad(&json),
        "no NaN or inf in the text",
    );
    let mut ok = true;
    for t in &table.tracks {
        let expected = format!(
            "\"frequencies_hz\": [{}, {}, null, null]",
            t.frequencies[0].unwrap(),
            t.frequencies[1].unwrap()
        );
        ok &= json.contains(&expected);
    }
    ck.holds("A10 JSON nulls", ok, "null exactly at the two losses of every track");
    println!("{csv}{json}");
    ck.finish();
}

// -------------------------------------------------------------------------------------------------------- A11

#[test]
fn a11_refusals() {
    let mut ck = Checks::new();
    let b = bar();
    let part = RotatingPart::new(&b, AXIS, ORIGIN, &RotatingOptions::new(2)).unwrap();
    let key = |r: Result<CampbellTable, FemError>| match r {
        Err(FemError::InvalidValue { key, .. }) => key.to_string(),
        other => format!("{other:?}"),
    };
    ck.holds(
        "A11 decreasing",
        key(part.campbell(&[0.0, 20.0, 10.0])) == "speeds",
        "refused, key speeds",
    );
    ck.holds(
        "A11 negative",
        key(part.campbell(&[-1.0, 10.0])) == "speeds",
        "refused, key speeds",
    );
    ck.holds(
        "A11 NaN",
        key(part.campbell(&[0.0, f64::NAN])) == "speeds",
        "refused, key speeds",
    );
    let zero_axis = RotatingPart::new(&b, [0.0; 3], ORIGIN, &RotatingOptions::new(2));
    ck.holds(
        "A11 zero axis",
        matches!(zero_axis, Err(FemError::InvalidValue { key: "axis", .. })),
        "refused, key axis",
    );
    ck.holds(
        "A11 negative speed",
        matches!(part.modes(-1.0), Err(FemError::InvalidValue { key: "speed", .. })),
        "refused, key speed",
    );
    ck.finish();
}
