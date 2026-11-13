//! F-399 (ADR 0391): harmonic response by mode superposition, modal damping, static correction, effective mass, force
//! harmonics. Checks A1 to A10, C1 to C8 and the negative controls N1 to N4 of the registration.

use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::prescribed_equations;
use rcfd_fem::harmonic::{
    force_harmonics, participation, wrap_phase, HarmonicLoad, HarmonicOptions, HarmonicSolver, ModalBasis, ModalDamping,
};
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::solve::{solve_static_with, StaticOptions};
use rcfd_fem::transient::{Integrator, LoadHistory, LoadTable, Rayleigh, TransientOptions};
use rcfd_fem::{Constraint, ElasticSupport, FemError, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

const E_MOD: f64 = 2e11;
const RHO: f64 = 7850.0;
const B: f64 = 0.04;

/// A list of measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.6e} (expected > {limit:e}) {}",
            if ok { "as expected" } else { "MISSED" }
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
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

fn key_of(e: &FemError) -> Option<&'static str> {
    match e {
        FemError::InvalidValue { key, .. } | FemError::FieldLength { key, .. } => Some(key),
        _ => None,
    }
}

fn refused_with<T>(r: rcfd_fem::Result<T>, key: &str) -> (bool, String) {
    match r {
        Ok(_) => (false, "accepted".to_string()),
        Err(e) => (key_of(&e) == Some(key), format!("{e}")),
    }
}

fn mv(m: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; m.n];
    m.matvec(x, &mut y);
    y
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn inf_norm(x: &[f64]) -> f64 {
    x.iter().fold(0.0, |m, v| m.max(v.abs()))
}

fn steel() -> Material {
    Material::isotropic_elastic(E_MOD, 0.0, RHO, 0.0).unwrap()
}

fn c_wave() -> f64 {
    (E_MOD / RHO).sqrt()
}

// --------------------------------------------------------------------------------------------------- bars B40, B10

fn bar_model(cells: usize) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, B, B], [cells, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

/// The tip load: 0.25 N along x on each of the four nodes of `xmax` (F = 1 N).
fn tip_load(model: &SolidModel) -> Vec<f64> {
    let dofs = model.dof_map();
    let mut f = vec![0.0; dofs.n_equations()];
    for &a in model.mesh().node_set("xmax").unwrap() {
        f[dofs.equation(a, 0).unwrap()] = 0.25;
    }
    f
}

fn tip_equation(model: &SolidModel) -> usize {
    let node = model.mesh().node_set("xmax").unwrap()[0];
    model.dof_map().equation(node, 0).unwrap()
}

fn forces_of(model: &SolidModel, f: &[f64]) -> Vec<NodalForceAt> {
    let dofs = model.dof_map();
    (0..model.mesh().nodes().len())
        .map(|a| {
            let c = |k: usize| dofs.equation(a, k).map(|e| f[e]).unwrap_or(0.0);
            NodalForceAt::new(a, [c(0), c(1), c(2)])
        })
        .collect()
}

fn static_of(model: &SolidModel, f: &[f64]) -> Vec<f64> {
    solve_static_with(model, &[], &forces_of(model, f), &StaticOptions::default())
        .unwrap()
        .displacement_values()
        .to_vec()
}

fn b40_options(threads: Option<usize>) -> ModalOptions {
    let o = ModalOptions::new(10)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward);
    match threads {
        Some(t) => o.threads(t),
        None => o,
    }
}

struct Bar {
    model: SolidModel,
    basis: ModalBasis,
    tip: usize,
    lambda1: f64,
}

fn b40() -> Bar {
    let model = bar_model(40);
    let modal = modal_analysis(&model, &b40_options(None)).unwrap();
    let basis = ModalBasis::from_result(&modal).unwrap();
    Bar {
        tip: tip_equation(&model),
        lambda1: basis.eigenvalues()[0],
        model,
        basis,
    }
}

fn closed_tip(omega: f64) -> f64 {
    let k = omega / c_wave();
    (k * 1.0).tan() / (E_MOD * B * B * k)
}

// ------------------------------------------------------------------------------------------------- A1, A1v, A2, A3

#[test]
fn a1_a2_a3_resonance_of_the_first_mode() {
    let mut ch = Checks::new();
    let bar = b40();
    let zeta = 0.01;
    let omega1 = bar.lambda1.sqrt();
    println!("B40: omega_1 = {omega1:.10e} rad/s, {} free unknowns", bar.basis.len());
    let m = consistent(&bar.model).unwrap();
    let f = mv(&m, &bar.basis.modes()[0]);
    let xs = static_of(&bar.model, &f);
    let solver = HarmonicSolver::new(
        &bar.model,
        bar.basis.clone(),
        &HarmonicOptions::new(ModalDamping::Ratio(zeta)),
    )
    .unwrap();
    let load = HarmonicLoad::real(f).unwrap();
    let x = solver.response(&load, omega1).unwrap();
    let t = bar.tip;
    let amp = x.amplitude(t) / (xs[t].abs() / (2.0 * zeta));
    ch.at_most(
        "A1 amplitude at the tip / (static / 2 zeta) - 1",
        (amp - 1.0).abs(),
        1e-8,
    );
    // X - x_s/(2 i zeta) = X + i x_s/(2 zeta)
    let mut worst: f64 = 0.0;
    for j in 0..xs.len() {
        let dr = x.re[j];
        let di = x.im[j] + xs[j] / (2.0 * zeta);
        worst = worst.max(dr.hypot(di));
    }
    ch.at_most(
        "A1v max |X - x_s/(2 i zeta)| / (max |x_s| / 2 zeta)",
        worst / (inf_norm(&xs) / (2.0 * zeta)),
        1e-8,
    );
    let lag = wrap_phase(0.0f64.atan2(xs[t]) - x.phase(t));
    ch.at_most("A2 |phase lag - pi/2|, rad", (lag - PI / 2.0).abs(), 1e-6);

    // A3: the half-power width of |X_tip(omega)|.
    let amp_at = |w: f64| solver.response(&load, w).unwrap().amplitude(t);
    let g = (5f64.sqrt() - 1.0) / 2.0;
    let (mut a, mut b) = (0.99 * omega1, 1.01 * omega1);
    let mut c = b - g * (b - a);
    let mut d = a + g * (b - a);
    let (mut fc, mut fd) = (amp_at(c), amp_at(d));
    for _ in 0..200 {
        if fc > fd {
            b = d;
            d = c;
            fd = fc;
            c = b - g * (b - a);
            fc = amp_at(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + g * (b - a);
            fd = amp_at(d);
        }
    }
    let wp = 0.5 * (a + b);
    let half = amp_at(wp) / 2f64.sqrt();
    let bisect = |mut lo: f64, mut hi: f64, rising: bool| {
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            let above = amp_at(mid) > half;
            if above == rising {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        0.5 * (lo + hi)
    };
    let w_lo = bisect(0.98 * omega1, wp, true);
    let w_hi = bisect(wp, 1.02 * omega1, false);
    let width = (w_hi - w_lo) / (2.0 * zeta * omega1);
    println!(
        "A3: peak at {:.12} omega_1, half-power points {:.12}, {:.12} omega_1, width / (2 zeta omega_1) = {width:.12}",
        wp / omega1,
        w_lo / omega1,
        w_hi / omega1
    );
    ch.at_most("A3 |width / (2 zeta omega_1) - 1|", (width - 1.0).abs(), 0.01);
    ch.finish();
}

// ------------------------------------------------------------------------------------------- A4, A5, N1, C4, C8

#[test]
fn a4_a5_bar_tip_with_and_without_the_static_correction() {
    let mut ch = Checks::new();
    let bar = b40();
    let omega1 = bar.lambda1.sqrt();
    let load = HarmonicLoad::real(tip_load(&bar.model)).unwrap();
    let with = HarmonicSolver::new(&bar.model, bar.basis.clone(), &HarmonicOptions::default()).unwrap();
    let without = HarmonicSolver::new(
        &bar.model,
        bar.basis.clone(),
        &HarmonicOptions::default().static_correction(false),
    )
    .unwrap();
    for s in [0.5, 1.5] {
        let w = s * omega1;
        let x = with.response(&load, w).unwrap();
        let cf = closed_tip(w);
        println!(
            "s = {s}: tip {:.10e} (im {:e}), closed {cf:.10e}",
            x.re[bar.tip], x.im[bar.tip]
        );
        ch.at_most(
            &format!("A4 s = {s}: |X_tip / closed - 1| with 10 modes and the static correction"),
            (x.re[bar.tip] / cf - 1.0).abs(),
            0.005,
        );
        let y = without.response(&load, w).unwrap();
        let e = (y.re[bar.tip] / cf - 1.0).abs();
        if s == 0.5 {
            ch.above("A5 s = 0.5: error without the static correction", e, 0.01);
        } else {
            ch.above("N1 s = 1.5: error without the static correction", e, 0.005);
        }
    }
    // C4: the sweep has the bits of the single responses.
    let ws: Vec<f64> = [0.2, 0.5, 1.5, 2.5].iter().map(|s| s * omega1).collect();
    let sw = with.sweep(&load, &ws).unwrap();
    let same = ws.iter().zip(&sw).all(|(&w, r)| {
        let x = with.response(&load, w).unwrap();
        x.re.iter().zip(&r.re).all(|(a, b)| a.to_bits() == b.to_bits())
            && x.im.iter().zip(&r.im).all(|(a, b)| a.to_bits() == b.to_bits())
            && x.omega.to_bits() == r.omega.to_bits()
    });
    ch.holds("C4 sweep equals response at each frequency", same, "bit-equal");
    // C8: linearity in the imaginary part.
    let mut f_im = vec![0.0; load.re().len()];
    let n1 = bar.model.mesh().node_set("xmax").unwrap()[1];
    f_im[bar.model.dof_map().equation(n1, 0).unwrap()] = 0.125;
    let complex = HarmonicLoad::new(load.re().to_vec(), f_im.clone()).unwrap();
    let w = 0.5 * omega1;
    let x = with.response(&complex, w).unwrap();
    let r1 = with.response(&load, w).unwrap();
    let r2 = with.response(&HarmonicLoad::real(f_im).unwrap(), w).unwrap();
    let mut diff: f64 = 0.0;
    let mut size: f64 = 0.0;
    for j in 0..x.re.len() {
        let er = x.re[j] - (r1.re[j] - r2.im[j]);
        let ei = x.im[j] - (r1.im[j] + r2.re[j]);
        diff = diff.max(er.hypot(ei));
        size = size.max(x.re[j].hypot(x.im[j]));
    }
    ch.at_most(
        "C8 complex load against the sum of the two real loads",
        diff / size,
        1e-14,
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------------------- A6, C1, C7

#[test]
fn a6_c1_c7_bar_effective_mass() {
    let mut ch = Checks::new();
    let bar = b40();
    let nodes = bar.model.mesh().nodes();
    let dofs = bar.model.dof_map();
    // C1: each mode uniform over each section.
    let mut worst: f64 = 0.0;
    for phi in bar.basis.modes() {
        let scale = inf_norm(phi);
        let mut sections: std::collections::BTreeMap<i64, Vec<f64>> = Default::default();
        for (a, p) in nodes.iter().enumerate() {
            sections
                .entry((p[0] * 1e6).round() as i64)
                .or_default()
                .push(phi[dofs.equation(a, 0).unwrap()]);
        }
        for v in sections.values() {
            let (lo, hi) = v
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &x| (l.min(x), h.max(x)));
            worst = worst.max((hi - lo) / scale);
        }
    }
    ch.at_most("C1 max section variation of the 10 modes / max |phi|", worst, 1e-6);
    let p = participation(&bar.model, &bar.basis, [1.0, 0.0, 0.0], None).unwrap();
    let fr = p.fractions();
    println!("B40 total mass {:.10e}, fractions {:?}", p.total_mass, &fr[..3]);
    let target = 8.0 / (PI * PI);
    ch.at_most("A6 |m_1/m / (8/pi^2) - 1|", (fr[0] / target - 1.0).abs(), 0.005);
    let sum: f64 = fr.iter().sum();
    ch.holds(
        "C7 sum of the 10 fractions in [0.95, 0.99279]",
        (0.95..=0.99279).contains(&sum),
        &format!("{sum:.8}"),
    );
    ch.finish();
}

// ----------------------------------------------------------------------------------------------------------- A7

#[test]
fn a7_cantilever_first_bending_mode_effective_mass() {
    let mut ch = Checks::new();
    let mesh = upgrade_to_hex20(&hex_box([0.0; 3], [1.0, 0.025, 0.05], [40, 1, 1]).unwrap())
        .unwrap()
        .0;
    let mut model = SolidModel::new(mesh);
    model.add_material("all", steel()).unwrap();
    for c in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let modal = modal_analysis(
        &model,
        &ModalOptions::new(3)
            .tolerance(1e-12)
            .residual_test(ResidualTest::Backward),
    )
    .unwrap();
    println!("C40: frequencies {:?} Hz", modal.frequencies);
    let basis = ModalBasis::from_result(&modal).unwrap();
    let py = participation(&model, &basis, [0.0, 1.0, 0.0], None).unwrap();
    let pz = participation(&model, &basis, [0.0, 0.0, 1.0], None).unwrap();
    println!("C40 fractions y {:?}, z {:?}", py.fractions(), pz.fractions());
    let f1 = py.fractions()[0];
    ch.at_most("A7 |m_1/m / 0.6130761 - 1|", (f1 / 0.6130761 - 1.0).abs(), 0.01);
    ch.finish();
}

// --------------------------------------------------------------------------------------------------- A8a, A8b, N3

fn small_model(clamped: bool) -> SolidModel {
    let mesh: SolidMesh = hex_box([0.0; 3], [1.0, 0.5, 0.5], [2, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    if clamped {
        for c in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
    }
    m
}

/// `g^T M_ff^{-1} g` by a dense Cholesky factor.
fn complete_sum(m: &CsrMatrix, free: &[usize], g: &[f64]) -> f64 {
    let n = free.len();
    let mut a = vec![0.0; n * n];
    for (i, &ei) in free.iter().enumerate() {
        for p in m.row_ptr[ei]..m.row_ptr[ei + 1] {
            if let Some(j) = free.iter().position(|&e| e == m.col[p]) {
                a[i * n + j] = m.val[p];
            }
        }
    }
    let mut l = vec![0.0; n * n];
    for j in 0..n {
        let s: f64 = a[j * n + j] - (0..j).map(|k| l[j * n + k] * l[j * n + k]).sum::<f64>();
        l[j * n + j] = s.sqrt();
        for i in j + 1..n {
            let s: f64 = a[i * n + j] - (0..j).map(|k| l[i * n + k] * l[j * n + k]).sum::<f64>();
            l[i * n + j] = s / l[j * n + j];
        }
    }
    // y = L^{-1} g; g^T M^{-1} g = y^T y
    let mut y = g.to_vec();
    for i in 0..n {
        let s: f64 = (0..i).map(|k| l[i * n + k] * y[k]).sum();
        y[i] = (y[i] - s) / l[i * n + i];
    }
    dot(&y, &y)
}

#[test]
fn a8_sum_over_all_modes_of_a_small_model() {
    let mut ch = Checks::new();
    let free_model = small_model(false);
    let basis = ModalBasis::all_modes(&free_model, None).unwrap();
    println!(
        "S-free: {} modes, lowest eigenvalues {:?}",
        basis.len(),
        &basis.eigenvalues()[..7]
    );
    for (name, d) in [("x", [1.0, 0.0, 0.0]), ("y", [0.0, 1.0, 0.0]), ("z", [0.0, 0.0, 1.0])] {
        let p = participation(&free_model, &basis, d, None).unwrap();
        let sum: f64 = p.effective_masses.iter().sum();
        ch.at_most(
            &format!("A8a S-free {name}: |sum m_r / m - 1|"),
            (sum / p.total_mass - 1.0).abs(),
            1e-10,
        );
    }
    let clamped = small_model(true);
    let basis = ModalBasis::all_modes(&clamped, None).unwrap();
    let m = consistent(&clamped).unwrap();
    let fixed = prescribed_equations(&clamped, &[]).unwrap();
    let n = m.n;
    let free: Vec<usize> = (0..n).filter(|e| !fixed.iter().any(|&(f, _)| f == *e)).collect();
    println!("S-clamped: {} modes, {} free unknowns", basis.len(), free.len());
    for (name, d) in [("x", [1.0, 0.0, 0.0]), ("y", [0.0, 1.0, 0.0]), ("z", [0.0, 0.0, 1.0])] {
        let p = participation(&clamped, &basis, d, None).unwrap();
        let dofs = clamped.dof_map();
        let mut r = vec![0.0; n];
        for a in 0..clamped.mesh().nodes().len() {
            for (c, dc) in d.iter().enumerate() {
                r[dofs.equation(a, c).unwrap()] = *dc;
            }
        }
        let mr = mv(&m, &r);
        let g: Vec<f64> = free.iter().map(|&e| mr[e]).collect();
        let reference = complete_sum(&m, &free, &g);
        let sum: f64 = p.effective_masses.iter().sum();
        println!(
            "S-clamped {name}: sum {sum:.14e}, g^T M_ff^-1 g {reference:.14e}, total mass {:.14e}",
            p.total_mass
        );
        ch.at_most(
            &format!("A8b S-clamped {name}: |sum m_r / g^T M_ff^-1 g - 1|"),
            (sum / reference - 1.0).abs(),
            1e-10,
        );
        if name == "x" {
            let big = p.effective_masses.iter().cloned().fold(0.0, f64::max);
            ch.above(
                "N3 the largest m_r left out",
                ((sum - big) / reference - 1.0).abs(),
                1e-3,
            );
        }
    }
    ch.finish();
}

// ----------------------------------------------------------------------------------------------------- A9, A10, N4

const R_T: f64 = 0.01;
const R_N: usize = 96;
const R_T0: f64 = 1.3e-3;
const R_MEAN: f64 = 0.7;
const R_H: [(f64, f64); 3] = [(1.0, 0.3), (0.45, -2.1), (0.2, 2.9)];

fn record_r() -> (Vec<f64>, f64) {
    let dt = 2.0 * R_T / R_N as f64;
    let om = 2.0 * PI / R_T;
    let s = (0..R_N)
        .map(|j| {
            let t = R_T0 + j as f64 * dt;
            R_MEAN
                + R_H
                    .iter()
                    .enumerate()
                    .map(|(h, &(a, p))| a * ((h + 1) as f64 * om * t + p).cos())
                    .sum::<f64>()
        })
        .collect();
    (s, dt)
}

#[test]
fn a9_a10_force_harmonics() {
    let mut ch = Checks::new();
    let (s, dt) = record_r();
    let sp = force_harmonics(&s, R_T0, dt, R_T, 5).unwrap();
    println!("R: mean {:.16}, periods {}, {:?}", sp.mean, sp.periods, sp.harmonics);
    let mut amp: f64 = (sp.mean - R_MEAN).abs();
    let mut ph: f64 = 0.0;
    for (i, h) in sp.harmonics.iter().enumerate() {
        let (a, p) = if i < 3 { R_H[i] } else { (0.0, 0.0) };
        amp = amp.max((h.amplitude - a).abs());
        if i < 3 {
            ph = ph.max(wrap_phase(h.phase - p).abs());
        }
    }
    ch.at_most("A9 amplitudes (and the mean), max abs error", amp, 1e-12);
    ch.at_most("A9 phases of orders 1 to 3, max abs error, rad", ph, 1e-12);
    // N4: the phase reference t0 dropped.
    let sp0 = force_harmonics(&s, 0.0, dt, R_T, 3).unwrap();
    let off = sp0
        .harmonics
        .iter()
        .zip(R_H)
        .map(|(h, (_, p))| wrap_phase(h.phase - p).abs())
        .fold(f64::INFINITY, f64::min);
    ch.above("N4 smallest phase error with t0 dropped", off, 1e-3);
    // A10.
    let span = R_N as f64 * dt;
    let (ok, d) = refused_with(force_harmonics(&s, R_T0, dt, span / 2.5, 3), "force record");
    ch.holds("A10 2.5 periods refused", ok, &d);
    let (ok, d) = refused_with(force_harmonics(&s, R_T0, dt, span / (2.0 + 1e-6), 3), "force record");
    ch.holds("A10 2 + 1e-6 periods refused", ok, &d);
    let r = force_harmonics(&s, R_T0, dt, span / (2.0 * (1.0 + 1e-13)), 3);
    ch.holds(
        "A10 2 (1 + 1e-13) periods accepted",
        r.is_ok(),
        &format!("{:?}", r.err()),
    );
    ch.finish();
}

// ----------------------------------------------------------------------------------------------------------- C2

#[test]
fn c2_static_limit_and_stress() {
    let mut ch = Checks::new();
    let bar = b40();
    let f = tip_load(&bar.model);
    let solver = HarmonicSolver::new(
        &bar.model,
        bar.basis.clone(),
        &HarmonicOptions::new(ModalDamping::Ratio(0.01)),
    )
    .unwrap();
    let x = solver.response(&HarmonicLoad::real(f.clone()).unwrap(), 0.0).unwrap();
    let xs = static_of(&bar.model, &f);
    let d: Vec<f64> = x.re.iter().zip(&xs).map(|(a, b)| a - b).collect();
    ch.at_most(
        "C2 |X_re - x_s| / |x_s| at omega = 0",
        inf_norm(&d) / inf_norm(&xs),
        1e-10,
    );
    ch.holds("C2 X_im exactly 0", x.im.iter().all(|&v| v == 0.0), "all zero");
    let st = x.stress(&bar.model).unwrap();
    let fs = IntegrationPointField::compute(&bar.model, &xs).unwrap();
    let mut diff: f64 = 0.0;
    let mut size: f64 = 0.0;
    for (a, b) in st.re.iter().zip(fs.stress()) {
        diff = diff.max((a[0] - b[0]).abs());
        size = size.max(b[0].abs());
    }
    println!("C2 static sigma_xx max {size:.6e} Pa (F/A = {:.6e})", 1.0 / (B * B));
    ch.at_most(
        "C2 stress sigma_xx of the real part against the static stress",
        diff / size,
        1e-10,
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------- C3, N2

#[test]
fn c3_steady_state_of_the_transient_integrator() {
    let mut ch = Checks::new();
    let model = bar_model(10);
    let basis = ModalBasis::all_modes(&model, None).unwrap();
    let omega1 = basis.eigenvalues()[0].sqrt();
    let ray = Rayleigh::from_ratio(0.1, omega1, omega1).unwrap();
    let solver = HarmonicSolver::new(
        &model,
        basis,
        &HarmonicOptions::new(ModalDamping::Rayleigh {
            mass: ray.mass,
            stiffness: ray.stiffness,
        })
        .static_correction(false),
    )
    .unwrap();
    let f = tip_load(&model);
    let tip = tip_equation(&model);
    let omega = 0.5 * omega1;
    let per = 200usize;
    let dt = 2.0 * PI / (per as f64 * omega);
    let steps = 8000u64;
    let points: Vec<(f64, f64)> = (0..=steps)
        .map(|n| {
            let t = 0.0 + n as f64 * dt;
            (t, (omega * t).cos())
        })
        .collect();
    let history = LoadHistory::new(f.clone(), LoadTable::new(points).unwrap());
    let mut it = Integrator::new(&model, &[history], &TransientOptions::new(dt).damping(ray)).unwrap();
    let n = f.len();
    it.start(0.0, &vec![0.0; n], &vec![0.0; n]).unwrap();
    let first = steps - per as u64;
    it.run(first).unwrap();
    let mut samples = Vec::with_capacity(per);
    for _ in 0..per {
        samples.push(it.displacement().unwrap()[tip]);
        it.step().unwrap();
    }
    let sp = force_harmonics(&samples, first as f64 * dt, dt, 2.0 * PI / omega, 1).unwrap();
    let h = sp.harmonics[0];
    let load = HarmonicLoad::real(f).unwrap();
    let omega_hat = (2.0 / dt) * (omega * dt / 2.0).tan();
    let x = solver.response(&load, omega_hat).unwrap();
    println!(
        "C3: transient amplitude {:.12e} phase {:.12}, harmonic at omega_hat {:.12e} phase {:.12}, mean {:e}",
        h.amplitude,
        h.phase,
        x.amplitude(tip),
        x.phase(tip),
        sp.mean
    );
    ch.at_most(
        "C3 amplitude against the harmonic response at omega_hat",
        (h.amplitude / x.amplitude(tip) - 1.0).abs(),
        1e-8,
    );
    ch.at_most(
        "C3 phase against the harmonic response at omega_hat, rad",
        wrap_phase(h.phase - x.phase(tip)).abs(),
        1e-8,
    );
    let y = solver.response(&load, omega).unwrap();
    ch.above(
        "N2 amplitude against the harmonic response at omega (unwarped)",
        (h.amplitude / y.amplitude(tip) - 1.0).abs(),
        1e-6,
    );
    ch.finish();
}

// ----------------------------------------------------------------------------------------------------------- C5

#[test]
fn c5_threads_bit_identical() {
    let mut ch = Checks::new();
    let model = bar_model(40);
    let run = |t: usize| {
        let modal = modal_analysis(&model, &b40_options(Some(t))).unwrap();
        let basis = ModalBasis::from_result(&modal).unwrap();
        let w = 0.5 * basis.eigenvalues()[0].sqrt();
        let p = participation(&model, &basis, [1.0, 0.0, 0.0], Some(t)).unwrap();
        let s = HarmonicSolver::new(&model, basis, &HarmonicOptions::default().threads(t)).unwrap();
        let x = s.response(&HarmonicLoad::real(tip_load(&model)).unwrap(), w).unwrap();
        (x, p)
    };
    let (x1, p1) = run(1);
    let (x8, p8) = run(8);
    let bits = |a: &[f64], b: &[f64]| a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits());
    ch.holds(
        "C5 response threads 1 and 8",
        bits(&x1.re, &x8.re) && bits(&x1.im, &x8.im),
        "bit-equal",
    );
    ch.holds(
        "C5 participation factors threads 1 and 8",
        bits(&p1.factors, &p8.factors) && p1.total_mass.to_bits() == p8.total_mass.to_bits(),
        "bit-equal",
    );
    ch.finish();
}

// ----------------------------------------------------------------------------------------------------------- C6

#[test]
#[ignore = "record: ADR 0400 retired two of its refusal rows (static correction of a free body, with a nodal spring); see c6_refusals_after_landed"]
fn c6_refusals() {
    let mut ch = Checks::new();
    let bar = b40();
    let load = HarmonicLoad::real(tip_load(&bar.model)).unwrap();
    let s = HarmonicSolver::new(&bar.model, bar.basis.clone(), &HarmonicOptions::default()).unwrap();
    for w in [-1.0, f64::NAN, f64::INFINITY] {
        let (ok, d) = refused_with(s.response(&load, w), "frequency");
        ch.holds(&format!("C6 omega {w}"), ok, &d);
    }
    let w3 = bar.basis.eigenvalues()[2].sqrt();
    let (ok, d) = refused_with(s.response(&load, w3), "frequency");
    ch.holds("C6 undamped resonance of mode 3", ok, &d);
    for (name, damping) in [
        ("Ratio(-0.1)", ModalDamping::Ratio(-0.1)),
        ("Ratio(NaN)", ModalDamping::Ratio(f64::NAN)),
        ("PerMode of 9", ModalDamping::PerMode(vec![0.01; 9])),
    ] {
        let (ok, d) = refused_with(
            HarmonicSolver::new(&bar.model, bar.basis.clone(), &HarmonicOptions::new(damping)),
            "damping",
        );
        ch.holds(&format!("C6 damping {name}"), ok, &d);
    }
    let (ok, d) = refused_with(
        s.response(&HarmonicLoad::real(vec![0.0; 5]).unwrap(), 1.0),
        "harmonic load",
    );
    ch.holds("C6 load of the wrong length", ok, &d);
    let b10 = bar_model(10);
    let b10_basis = ModalBasis::all_modes(&b10, None).unwrap();
    let (ok, d) = refused_with(
        HarmonicSolver::new(&bar.model, b10_basis, &HarmonicOptions::default()),
        "modal basis",
    );
    ch.holds("C6 basis of B10 for B40", ok, &d);
    let free_model = small_model(false);
    let free_basis = ModalBasis::all_modes(&free_model, None).unwrap();
    let (ok, d) = refused_with(
        HarmonicSolver::new(&free_model, free_basis, &HarmonicOptions::default()),
        "static correction",
    );
    ch.holds("C6 static correction of a free body", ok, &d);
    let clamped = small_model(true);
    let clamped_basis = ModalBasis::all_modes(&clamped, None).unwrap();
    let mut sprung = clamped.clone();
    sprung
        .add_support(ElasticSupport::nodal_spring("xmax", 0, 1e6))
        .unwrap();
    let (ok, d) = refused_with(
        HarmonicSolver::new(&sprung, clamped_basis, &HarmonicOptions::default()),
        "static correction",
    );
    ch.holds("C6 static correction with a nodal spring", ok, &d);
    let mut big = SolidModel::new(hex_box([0.0; 3], [10.0, B, B], [200, 1, 1]).unwrap());
    big.add_material("all", steel()).unwrap();
    let (ok, d) = refused_with(ModalBasis::all_modes(&big, None), "modes");
    ch.holds("C6 all_modes of 2412 free unknowns", ok, &d);
    let (rs, dt) = record_r();
    for (name, r) in [
        ("1 sample", force_harmonics(&rs[..1], 0.0, dt, R_T, 1)),
        ("NaN sample", {
            let mut v = rs.clone();
            v[3] = f64::NAN;
            force_harmonics(&v, 0.0, dt, R_T, 1)
        }),
        ("dt 0", force_harmonics(&rs, 0.0, 0.0, R_T, 1)),
        ("period -1", force_harmonics(&rs, 0.0, dt, -1.0, 1)),
        ("orders 0", force_harmonics(&rs, 0.0, dt, R_T, 0)),
        ("order 24 (Nyquist)", force_harmonics(&rs, 0.0, dt, R_T, 24)),
    ] {
        let (ok, d) = refused_with(r, "force record");
        ch.holds(&format!("C6 force record {name}"), ok, &d);
    }
    ch.finish();
}

/// C6 without the two rows retired by ADR 0400 (the static correction of a free body and with a nodal spring are
/// taken since F-491).
#[test]
fn c6_refusals_after_landed() {
    let mut ch = Checks::new();
    let bar = b40();
    let load = HarmonicLoad::real(tip_load(&bar.model)).unwrap();
    let s = HarmonicSolver::new(&bar.model, bar.basis.clone(), &HarmonicOptions::default()).unwrap();
    for w in [-1.0, f64::NAN, f64::INFINITY] {
        let (ok, d) = refused_with(s.response(&load, w), "frequency");
        ch.holds(&format!("C6 omega {w}"), ok, &d);
    }
    let w3 = bar.basis.eigenvalues()[2].sqrt();
    let (ok, d) = refused_with(s.response(&load, w3), "frequency");
    ch.holds("C6 undamped resonance of mode 3", ok, &d);
    for (name, damping) in [
        ("Ratio(-0.1)", ModalDamping::Ratio(-0.1)),
        ("Ratio(NaN)", ModalDamping::Ratio(f64::NAN)),
        ("PerMode of 9", ModalDamping::PerMode(vec![0.01; 9])),
    ] {
        let (ok, d) = refused_with(
            HarmonicSolver::new(&bar.model, bar.basis.clone(), &HarmonicOptions::new(damping)),
            "damping",
        );
        ch.holds(&format!("C6 damping {name}"), ok, &d);
    }
    let (ok, d) = refused_with(
        s.response(&HarmonicLoad::real(vec![0.0; 5]).unwrap(), 1.0),
        "harmonic load",
    );
    ch.holds("C6 load of the wrong length", ok, &d);
    let b10 = bar_model(10);
    let b10_basis = ModalBasis::all_modes(&b10, None).unwrap();
    let (ok, d) = refused_with(
        HarmonicSolver::new(&bar.model, b10_basis, &HarmonicOptions::default()),
        "modal basis",
    );
    ch.holds("C6 basis of B10 for B40", ok, &d);
    let mut big = SolidModel::new(hex_box([0.0; 3], [10.0, B, B], [200, 1, 1]).unwrap());
    big.add_material("all", steel()).unwrap();
    let (ok, d) = refused_with(ModalBasis::all_modes(&big, None), "modes");
    ch.holds("C6 all_modes of 2412 free unknowns", ok, &d);
    let (rs, dt) = record_r();
    for (name, r) in [
        ("1 sample", force_harmonics(&rs[..1], 0.0, dt, R_T, 1)),
        ("NaN sample", {
            let mut v = rs.clone();
            v[3] = f64::NAN;
            force_harmonics(&v, 0.0, dt, R_T, 1)
        }),
        ("dt 0", force_harmonics(&rs, 0.0, 0.0, R_T, 1)),
        ("period -1", force_harmonics(&rs, 0.0, dt, -1.0, 1)),
        ("orders 0", force_harmonics(&rs, 0.0, dt, R_T, 0)),
        ("order 24 (Nyquist)", force_harmonics(&rs, 0.0, dt, R_T, 24)),
    ] {
        let (ok, d) = refused_with(r, "force record");
        ch.holds(&format!("C6 force record {name}"), ok, &d);
    }
    ch.finish();
}
