//! F-414, ADR 0450: damped and complex modes on a modal subspace (checks R1, R2, B1 to B3, L1, T1, N1, S2).

use rcfd_fem::builders::hex_box;
use rcfd_fem::damped::{damped_modes, DampedModes, DampedOptions};
use rcfd_fem::harmonic_direct::{Dashpot, LossFactor, ModulusTable};
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::rotating::gyroscopic_state_eigen_dense;
use rcfd_fem::{Constraint, LinearTerm, Material, SolidModel};
use rcfd_linalg::complex::C64;
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::eigen_general::quadratic_eigen;
use std::f64::consts::PI;

const E_MOD: f64 = 2.0e11;
const RHO: f64 = 7850.0;

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {} ({detail})", if ok { "met" } else { "MISSED" });
        if !ok {
            self.failed.push(name.to_string());
        }
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:e} (limit {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(name.to_string());
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "missed: {:?}", self.failed);
    }
}

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(E_MOD, nu, RHO, 0.0).unwrap()
}

fn modal(m: usize) -> ModalOptions {
    ModalOptions::new(m)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward)
}

/// The cantilever of R1: `[0, 1] x [0, 0.1]^2`, Hex8 10 x 2 x 2, `nu = 0.3`, `xmin` held.
fn cantilever() -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [1.0, 0.1, 0.1], [10, 2, 2]).unwrap());
    m.add_material("all", steel(0.3)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

/// The bar of B1, B2: `[0, 1] x [0, 0.01]^2`, Hex8 100 x 1 x 1, `nu = 0`, y and z held, x held at `xmin`.
fn bar() -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [1.0, 0.01, 0.01], [100, 1, 1]).unwrap());
    m.add_material("all", steel(0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

fn impedance() -> f64 {
    1e-4 * (E_MOD * RHO).sqrt()
}

fn c0() -> f64 {
    (E_MOD / RHO).sqrt()
}

/// The bar's damped modes with a tip dashpot `c` (`c/4` on each `xmax` node along x), `modes` basis modes.
fn bar_modes(c: f64, modes: usize) -> DampedModes {
    let model = bar();
    let mut o = DampedOptions::with_modal(modal(modes));
    for &a in model.mesh().node_set("xmax").unwrap() {
        o = o.dashpot(Dashpot::ground(a, 0, c / 4.0));
    }
    damped_modes(&model, &o).unwrap()
}

/// The relative distance of the reported root nearest `exact`.
fn nearest(d: &DampedModes, exact: C64, oscillating: bool) -> f64 {
    d.modes
        .iter()
        .filter(|m| m.oscillating == oscillating)
        .map(|m| (m.eigenvalue - exact).abs() / exact.abs())
        .fold(f64::INFINITY, f64::min)
}

fn csqrt(z: C64) -> C64 {
    let r = z.abs();
    C64::new(((r + z.re) / 2.0).sqrt(), ((r - z.re) / 2.0).sqrt().copysign(z.im))
}

fn print_modes(d: &DampedModes, count: usize) {
    for m in d.modes.iter().take(count) {
        println!(
            "  lambda {} zeta {:.6e} osc {} residual {:e}",
            m.eigenvalue, m.damping_ratio, m.oscillating, m.residual
        );
    }
    println!(
        "  reported {} (osc {}, real {}), notes {:?}, defects {}",
        d.modes.len(),
        d.oscillating,
        d.non_oscillating,
        d.notes,
        d.defects.len()
    );
}

/// `x^H M x` of a complex shape.
fn mass_norm(m: &rcfd_linalg::CsrMatrix, x: &[C64]) -> f64 {
    let re: Vec<f64> = x.iter().map(|v| v.re).collect();
    let im: Vec<f64> = x.iter().map(|v| v.im).collect();
    let mut yr = vec![0.0; m.n];
    let mut yi = vec![0.0; m.n];
    m.matvec(&re, &mut yr);
    m.matvec(&im, &mut yi);
    re.iter().zip(&yr).map(|(a, b)| a * b).sum::<f64>() + im.iter().zip(&yi).map(|(a, b)| a * b).sum::<f64>()
}

fn rayleigh_options(threads: usize) -> (DampedOptions, Vec<f64>, f64, f64) {
    let model = cantilever();
    let u = modal_analysis(&model, &modal(12)).unwrap();
    let w: Vec<f64> = u.eigenvalues.iter().map(|l| l.sqrt()).collect();
    let a = 0.2 * w[0];
    let b = 0.5 / w[11];
    (
        DampedOptions::with_modal(modal(12)).rayleigh(a, b).threads(threads),
        w,
        a,
        b,
    )
}

#[test]
fn r1_r2_rayleigh_on_a_solid_model() {
    let mut c = Checks::new();
    let model = cantilever();
    let (o, w, a, b) = rayleigh_options(4);
    let d = damped_modes(&model, &o).unwrap();
    print_modes(&d, 12);
    // R1: each w_j takes the reported mode of nearest |lambda|
    let mut taken = vec![false; d.modes.len()];
    let mut err: f64 = 0.0;
    for &wj in &w {
        let mut best = (usize::MAX, f64::INFINITY);
        for (i, m) in d.modes.iter().enumerate() {
            let dist = (m.eigenvalue.abs() - wj).abs();
            if !taken[i] && dist < best.1 {
                best = (i, dist);
            }
        }
        if best.0 == usize::MAX {
            err = f64::INFINITY;
            break;
        }
        taken[best.0] = true;
        err = err.max((d.modes[best.0].damping_ratio - (a / wj + b * wj) / 2.0).abs());
    }
    c.at_most("R1 max |zeta_j - (a/w_j + b w_j)/2|", err, 1e-8);
    let mass = consistent(&model).unwrap();
    let reported_once = d.modes.len() == 12
        && d.non_oscillating == 0
        && d.modes.iter().all(|m| {
            m.oscillating && m.eigenvalue.im > 0.0 && m.damping_ratio == -m.eigenvalue.re / m.eigenvalue.abs()
        });
    c.holds(
        "R2 12 modes reported once, oscillating, Im > 0, zeta = -Re/|lambda|",
        reported_once,
        &format!("{} reported, {} real", d.modes.len(), d.non_oscillating),
    );
    let mn = d
        .modes
        .iter()
        .map(|m| (mass_norm(&mass, &m.shape) - 1.0).abs())
        .fold(0.0, f64::max);
    c.at_most("R2 shapes |x^H M x - 1|", mn, 1e-10);
    c.finish();
}

#[test]
fn t1_threads_bit_identical() {
    let model = cantilever();
    let d1 = damped_modes(&model, &rayleigh_options(1).0).unwrap();
    let d4 = damped_modes(&model, &rayleigh_options(4).0).unwrap();
    let bits = |d: &DampedModes| -> Vec<u64> {
        let mut v = Vec::new();
        for m in &d.modes {
            v.push(m.eigenvalue.re.to_bits());
            v.push(m.eigenvalue.im.to_bits());
            for s in &m.shape {
                v.push(s.re.to_bits());
                v.push(s.im.to_bits());
            }
        }
        v
    };
    let same = bits(&d1) == bits(&d4);
    println!(
        "T1 threads 1 and 4 bit-identical: {} ({} words)",
        if same { "met" } else { "MISSED" },
        bits(&d1).len()
    );
    assert!(same);
}

#[test]
fn b1_b2_b3_bar_with_a_tip_dashpot() {
    let mut c = Checks::new();
    let (z, s) = (impedance(), c0());
    println!("Z = {z} N s/m, c0 = {s} m/s");
    let at = (0.5f64).atanh();
    // B1: c = Z/2
    let d = bar_modes(0.5 * z, 40);
    print_modes(&d, 6);
    let mut e1 = Vec::new();
    for j in 1..=3 {
        let exact = C64::new(-at, (2 * j - 1) as f64 * PI / 2.0).scale(s);
        let e = nearest(&d, exact, true);
        println!("  B1 j {j}: exact {exact}, error {e:e}");
        e1.push(e);
    }
    c.at_most(
        "B1 c = Z/2, first three roots, relative",
        e1.iter().copied().fold(0.0, f64::max),
        2e-2,
    );
    let b1_real = d.non_oscillating;
    // B2: c = 2 Z
    let d2 = bar_modes(2.0 * z, 40);
    print_modes(&d2, 6);
    let real_smallest = d2
        .modes
        .iter()
        .filter(|m| !m.oscillating)
        .min_by(|p, q| p.eigenvalue.abs().total_cmp(&q.eigenvalue.abs()));
    let exact0 = C64::from_real(-at * s);
    let e0 = real_smallest.map_or(f64::INFINITY, |m| (m.eigenvalue - exact0).abs() / exact0.abs());
    println!("  B2 j 0: exact {exact0}, error {e0:e}");
    let mut e2 = vec![e0];
    for j in 1..=3 {
        let exact = C64::new(-at, j as f64 * PI).scale(s);
        let e = nearest(&d2, exact, true);
        println!("  B2 j {j}: exact {exact}, error {e:e}");
        e2.push(e);
    }
    c.at_most(
        "B2 c = 2 Z, real root and first three oscillating, relative",
        e2.iter().copied().fold(0.0, f64::max),
        2e-2,
    );
    let reals_flagged = d2
        .modes
        .iter()
        .filter(|m| !m.oscillating)
        .all(|m| m.eigenvalue.im == 0.0 && (m.damping_ratio - 1.0).abs() <= 1e-12);
    let says = d2.notes.iter().any(|n| n.contains("non-oscillating"));
    c.holds(
        "B2b real roots reported non-oscillating, ratio 1, said in notes; none at c = Z/2",
        d2.non_oscillating >= 1 && reals_flagged && says && b1_real == 0,
        &format!(
            "c = 2Z: {} real, notes {:?}; c = Z/2: {b1_real} real",
            d2.non_oscillating, d2.notes
        ),
    );
    // B3
    let exact1 = C64::new(-at, PI / 2.0).scale(s);
    let errs: Vec<f64> = [10, 20]
        .iter()
        .map(|&m| nearest(&bar_modes(0.5 * z, m), exact1, true))
        .collect();
    println!(
        "B3 errors of lambda_1 on 10, 20, 40 modes: {:e}, {:e}, {:e}",
        errs[0], errs[1], e1[0]
    );
    c.holds("B3 decreasing 10 -> 20 -> 40", errs[0] > errs[1] && errs[1] > e1[0], "");
    c.finish();
}

#[test]
fn l1_loss_factor() {
    let mut c = Checks::new();
    let model = cantilever();
    let u = modal_analysis(&model, &modal(12)).unwrap();
    let d = damped_modes(
        &model,
        &DampedOptions::with_modal(modal(12)).loss_factor("all", LossFactor::Constant(0.04)),
    )
    .unwrap();
    print_modes(&d, 12);
    let root = csqrt(C64::new(1.0, 0.04));
    let mut taken = vec![false; d.modes.len()];
    let mut err: f64 = 0.0;
    for l in &u.eigenvalues {
        let exact = C64::I * root.scale(l.sqrt());
        let mut best = (usize::MAX, f64::INFINITY);
        for (i, m) in d.modes.iter().enumerate() {
            let dist = (m.eigenvalue - exact).abs();
            if !taken[i] && dist < best.1 {
                best = (i, dist);
            }
        }
        if best.0 == usize::MAX {
            err = f64::INFINITY;
            break;
        }
        taken[best.0] = true;
        err = err.max(best.1 / exact.abs());
    }
    c.holds(
        "L1 12 roots reported",
        d.modes.len() == 12,
        &format!("{}", d.modes.len()),
    );
    c.at_most("L1 |lambda - i w sqrt(1 + i eta)| / |lambda|", err, 1e-10);
    c.finish();
}

#[test]
fn n1_refusals() {
    let mut c = Checks::new();
    let model = cantilever();
    let table = ModulusTable::new(vec![0.0, 1e5], vec![E_MOD, E_MOD], vec![0.02, 0.02]).unwrap();
    let cases: Vec<(&str, SolidModel, DampedOptions)> = vec![
        (
            "modulus table",
            cantilever(),
            DampedOptions::new(4).loss_factor("all", LossFactor::Table(table)),
        ),
        (
            "unknown set",
            cantilever(),
            DampedOptions::new(4).loss_factor("nope", LossFactor::Constant(0.01)),
        ),
        (
            "negative dashpot",
            cantilever(),
            DampedOptions::new(4).dashpot(Dashpot::ground(5, 0, -1.0)),
        ),
        (
            "transformation path",
            {
                let mut m = cantilever();
                let node = m.mesh().node_set("xmax").unwrap()[0];
                m.add_constraint(Constraint::linear(vec![LinearTerm::new(node, 1, 1.0)], 0.0))
                    .unwrap();
                m
            },
            DampedOptions::new(4),
        ),
    ];
    let _ = model;
    for (what, m, o) in cases {
        let r = damped_modes(&m, &o);
        let msg = match &r {
            Err(e) => e.to_string(),
            Ok(_) => "Ok".to_string(),
        };
        c.holds(
            &format!("N1 {what} refused"),
            matches!(r, Err(rcfd_fem::FemError::InvalidValue { .. })),
            &msg,
        );
    }
    c.finish();
}

#[test]
fn s2_skew_coupling_against_the_whirl_solver() {
    let mut c = Checks::new();
    for (k1, k2, g) in [(4.0, 4.0, 1.0), (1.0, 4.0, 0.5)] {
        let m = [1.0, 0.0, 0.0, 1.0];
        let gm = [0.0, g, -g, 0.0];
        let k = [k1, 0.0, 0.0, k2];
        let q = quadratic_eigen(&m, &gm, &k, 2).unwrap();
        let mut ours: Vec<f64> = q.values.iter().filter(|v| v.im > 0.0).map(|v| v.im).collect();
        ours.sort_by(f64::total_cmp);
        let w = gyroscopic_state_eigen_dense(&k, &m, &gm, 2).unwrap();
        let mut theirs: Vec<f64> = w.omegas.clone();
        theirs.sort_by(f64::total_cmp);
        println!("k1 {k1} k2 {k2} g {g}: ours {ours:?}, whirl {theirs:?}");
        let err = if ours.len() == theirs.len() {
            ours.iter()
                .zip(&theirs)
                .map(|(a, b)| (a - b).abs() / b)
                .fold(0.0, f64::max)
        } else {
            f64::INFINITY
        };
        c.at_most(
            &format!("S2 k1 {k1} k2 {k2} g {g}: against gyroscopic_state_eigen_dense"),
            err,
            1e-12,
        );
    }
    c.finish();
}

// ADR 0450 amendment 1: no spurious defect on a model with independent modes.
#[test]
fn q4b_no_spurious_defects() {
    let model = cantilever();
    let d = damped_modes(&model, &rayleigh_options(4).0).unwrap();
    let ok = d.defects.is_empty() && !d.notes.iter().any(|n| n.contains("defective"));
    println!(
        "Q4b R1 cantilever: no defect: {} ({:?}, notes {:?})",
        if ok { "met" } else { "MISSED" },
        d.defects,
        d.notes
    );
    assert!(ok);
}
