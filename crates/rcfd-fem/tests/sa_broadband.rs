//! F-566 (ADR 0497): broadband (Dirlik) fatigue damage rate from the spectral moments. Checks C1 to C9 of the registration; every
//! value is printed.

use rcfd_core::rng::Rng;
use rcfd_fem::builders::hex_box;
use rcfd_fem::fatigue::{
    broadband_damage_rate, broadband_damage_rate_curve, dirlik_coefficients, miner, narrow_band_damage_rate,
    rainflow_periodic, spectral_rate, MeanStressCorrection, SnCurve,
};
use rcfd_fem::harmonic::{HarmonicOptions, ModalBasis, ModalDamping};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::random::{
    spectral_response, Excitation, ModalChannels, Moments, RandomRequest, RandomSolver, SpectralOptions, Spectrum,
};
use rcfd_fem::{Constraint, FemError, Material, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;

const SEED: u64 = 20_261_114;

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    /// `value <= limit`, the value finite.
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
    /// `value > limit`, the value finite.
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value > limit;
        println!(
            "{name}: {value:.6e} (expected > {limit:e}) {}",
            if ok { "as expected" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
        }
    }
    /// `value >= limit`, the value finite.
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

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

fn refused(r: rcfd_fem::Result<f64>, contains: &[&str]) -> (bool, String) {
    match r {
        Ok(v) => (false, format!("accepted: {v:e}")),
        Err(e) => {
            let s = format!("{e}");
            let key_ok = matches!(&e, FemError::InvalidValue { key, .. } if *key == "broadband");
            (key_ok && contains.iter().all(|c| s.contains(c)), s)
        }
    }
}

// ----------------------------------------------------------------------------------------------------- the systems

const FN: f64 = 50.0;
const MASS: f64 = 2.0;

/// The one-mode system S as channels (force gain `1/m`), flat `G = 1e-2` on `[0, 500]`, displacement moments.
fn one_mode_moments(zeta: f64) -> Moments {
    let w = 2.0 * PI * FN;
    let ch = ModalChannels::new(
        vec![w * w],
        vec![2.0 * zeta * w],
        vec![vec![vec![1.0 / MASS]]],
        vec![vec![0.0]],
    )
    .unwrap();
    let spec = [Spectrum::flat(0.0, 500.0, 1e-2).unwrap()];
    spectral_response(&ch, &spec, None, &SpectralOptions::default())
        .unwrap()
        .moments[0]
}

const F1: f64 = 50.0;
const F2: f64 = 150.0;
const ZT: f64 = 0.02;
const U2: f64 = 5.196_152_422_706_632; // 3^1.5
const FBAND: f64 = 600.0;

/// The two-mode system T (gains 1 and 3^1.5, unit modal masses), flat `G = 1` on `[0, 600]`.
fn two_mode_moments() -> Moments {
    let (w1, w2) = (2.0 * PI * F1, 2.0 * PI * F2);
    let ch = ModalChannels::new(
        vec![w1 * w1, w2 * w2],
        vec![2.0 * ZT * w1, 2.0 * ZT * w2],
        vec![vec![vec![1.0, U2]]],
        vec![vec![0.0]],
    )
    .unwrap();
    let spec = [Spectrum::flat(0.0, FBAND, 1.0).unwrap()];
    spectral_response(&ch, &spec, None, &SpectralOptions::default())
        .unwrap()
        .moments[0]
}

/// The density of the displacement of T at `f` (Hz).
fn two_mode_density(f: f64) -> f64 {
    let h = |fr: f64, u: f64| {
        let (w, wr) = (2.0 * PI * f, 2.0 * PI * fr);
        let (re, im) = (wr * wr - w * w, 2.0 * ZT * wr * w);
        let d = re * re + im * im;
        (u * re / d, -u * im / d)
    };
    let (a, b) = (h(F1, 1.0), h(F2, U2));
    let (re, im) = (a.0 + b.0, a.1 + b.1);
    re * re + im * im
}

fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = 2.0 * PI / len as f64;
        let mut i = 0;
        while i < n {
            for k in 0..len / 2 {
                let (cr, ci) = ((ang * k as f64).cos(), (ang * k as f64).sin());
                let (a, b) = (i + k, i + k + len / 2);
                let (tr, ti) = (re[b] * cr - im[b] * ci, re[b] * ci + im[b] * cr);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
            i += len;
        }
        len <<= 1;
    }
}

// ------------------------------------------------------------------------------------------------------------ C1, C2

#[test]
fn c1_one_mode_reduces_to_narrow_band() {
    let mut c = Checks::new();
    let mo = one_mode_moments(0.01);
    for (m, name, reference) in [(3.0, "C1", 0.993_398_434_973_984_2), (5.0, "C1b", 0.0)] {
        let b = mo.broadband_damage_rate(m, 1.0).unwrap();
        let n = mo.narrow_band_damage_rate(m, 1.0).unwrap();
        println!(
            "{name}: m = {m}, broadband {b:.9e}, narrow band {n:.9e}, ratio {:.9}",
            b / n
        );
        c.at_most(&format!("{name} |broadband/narrow band - 1|"), rel(b, n), 1e-2);
        if reference > 0.0 {
            c.at_most("C1x ratio against 0.9933984349739842", rel(b / n, reference), 1e-5);
        }
    }
    c.finish();
}

fn bar() -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [40, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2e11, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

/// Ignored record (ADR 0497 amendment 1): a defective set-up, the basis is all 40 modes of the bar, not one; measured ratio
/// 0.941825219777 against 0.9888264677733556 (rel 4.75e-2). Continued by `c2_c6_one_mode_solver_moments_after_amendment_1`.
#[test]
#[ignore]
fn c2_c6_solver_moments_feed_it_unchanged() {
    let mut c = Checks::new();
    let m = bar();
    let basis = ModalBasis::all_modes(&m, Some(1)).unwrap();
    let f1 = basis.eigenvalues()[0].sqrt() / (2.0 * PI);
    let opts = HarmonicOptions::new(ModalDamping::Ratio(0.02))
        .static_correction(false)
        .threads(1);
    let solver = RandomSolver::new(&m, basis, &opts).unwrap();
    let dofs = m.dof_map();
    let eqs: Vec<usize> = m
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&p| dofs.equation(p, 0).unwrap())
        .collect();
    let mut f = vec![0.0; dofs.n_equations()];
    for &e in &eqs {
        f[e] = 0.25;
    }
    let spec = Spectrum::flat(0.0, 10.0 * f1, 1.0).unwrap();
    let r = solver
        .response(&m, &Excitation::force(f, spec), &RandomRequest::new(vec![eqs[0]]))
        .unwrap();
    let mo = r.moments[0];
    println!("C2: f_1 {f1:.6} Hz, moments {mo:?}");
    let (ex, cc) = (3.0, 1.0);
    let b = mo.broadband_damage_rate(ex, cc).unwrap();
    let n = mo.narrow_band_damage_rate(ex, cc).unwrap();
    println!("C2: ratio {:.12}", b / n);
    c.at_most(
        "C2 ratio against 0.9888264677733556",
        rel(b / n, 0.988_826_467_773_355_6),
        1e-5,
    );
    // C6: bit equality with the free functions on the moments as given.
    let direct = broadband_damage_rate(mo.m0, mo.m1, mo.m2, mo.m4, ex, cc).unwrap();
    let (rms, nu0) = spectral_rate(mo.m0, mo.m2).unwrap();
    let direct_n = narrow_band_damage_rate(nu0, rms, ex, cc).unwrap();
    c.holds(
        "C6 RandomSolver moments, broadband bits",
        b.to_bits() == direct.to_bits(),
        &format!("{b:e} vs {direct:e}"),
    );
    c.holds(
        "C6 RandomSolver moments, narrow-band bits",
        n.to_bits() == direct_n.to_bits(),
        &format!("{n:e} vs {direct_n:e}"),
    );
    let t = two_mode_moments();
    let tb = t.broadband_damage_rate(3.0, 1.0).unwrap();
    let td = broadband_damage_rate(t.m0, t.m1, t.m2, t.m4, 3.0, 1.0).unwrap();
    c.holds(
        "C6b channel moments, broadband bits",
        tb.to_bits() == td.to_bits(),
        &format!("{tb:e} vs {td:e}"),
    );
    c.finish();
}

#[test]
fn c2_c6_one_mode_solver_moments_after_amendment_1() {
    let mut c = Checks::new();
    let m = bar();
    let res = modal_analysis(
        &m,
        &ModalOptions::new(1)
            .tolerance(1e-12)
            .residual_test(ResidualTest::Backward)
            .threads(1),
    )
    .unwrap();
    let basis = ModalBasis::from_result(&res).unwrap();
    assert_eq!(basis.len(), 1);
    let f1 = basis.eigenvalues()[0].sqrt() / (2.0 * PI);
    let opts = HarmonicOptions::new(ModalDamping::Ratio(0.02))
        .static_correction(false)
        .threads(1);
    let solver = RandomSolver::new(&m, basis, &opts).unwrap();
    let dofs = m.dof_map();
    let eqs: Vec<usize> = m
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&p| dofs.equation(p, 0).unwrap())
        .collect();
    let mut f = vec![0.0; dofs.n_equations()];
    for &e in &eqs {
        f[e] = 0.25;
    }
    let spec = Spectrum::flat(0.0, 10.0 * f1, 1.0).unwrap();
    let r = solver
        .response(&m, &Excitation::force(f, spec), &RandomRequest::new(vec![eqs[0]]))
        .unwrap();
    let mo = r.moments[0];
    println!("C2: f_1 {f1:.6} Hz, moments {mo:?}");
    let (ex, cc) = (3.0, 1.0);
    let b = mo.broadband_damage_rate(ex, cc).unwrap();
    let n = mo.narrow_band_damage_rate(ex, cc).unwrap();
    println!("C2: ratio {:.12}", b / n);
    c.at_most(
        "C2 ratio against 0.9888264677733556",
        rel(b / n, 0.988_826_467_773_355_6),
        1e-5,
    );
    // C6: bit equality with the free functions on the moments as given.
    let direct = broadband_damage_rate(mo.m0, mo.m1, mo.m2, mo.m4, ex, cc).unwrap();
    let (rms, nu0) = spectral_rate(mo.m0, mo.m2).unwrap();
    let direct_n = narrow_band_damage_rate(nu0, rms, ex, cc).unwrap();
    c.holds(
        "C6 RandomSolver moments, broadband bits",
        b.to_bits() == direct.to_bits(),
        &format!("{b:e} vs {direct:e}"),
    );
    c.holds(
        "C6 RandomSolver moments, narrow-band bits",
        n.to_bits() == direct_n.to_bits(),
        &format!("{n:e} vs {direct_n:e}"),
    );
    let t = two_mode_moments();
    let tb = t.broadband_damage_rate(3.0, 1.0).unwrap();
    let td = broadband_damage_rate(t.m0, t.m1, t.m2, t.m4, 3.0, 1.0).unwrap();
    c.holds(
        "C6b channel moments, broadband bits",
        tb.to_bits() == td.to_bits(),
        &format!("{tb:e} vs {td:e}"),
    );
    c.finish();
}

#[test]
fn c2b_c3_c9_coefficients_and_density() {
    let mut c = Checks::new();
    let m5 = one_mode_moments(0.05);
    let r = m5.broadband_damage_rate(3.0, 1.0).unwrap() / m5.narrow_band_damage_rate(3.0, 1.0).unwrap();
    println!("C2b: zeta 0.05 ratio {r:.12}");
    c.at_most(
        "C2b ratio against 0.9748167797070537",
        rel(r, 0.974_816_779_707_053_7),
        1e-5,
    );
    let mo = one_mode_moments(0.02);
    let k = dirlik_coefficients(mo.m0, mo.m1, mo.m2, mo.m4).unwrap();
    println!("C3 coefficients {k:?}");
    c.at_most(
        "C9 D1 + D2 + D3 - 1 (S, zeta 0.02)",
        (k.d1 + k.d2 + k.d3 - 1.0).abs(),
        1e-15,
    );
    let t = two_mode_moments();
    let kt = dirlik_coefficients(t.m0, t.m1, t.m2, t.m4).unwrap();
    println!("C3 coefficients of T {kt:?}");
    c.at_most("C9 D1 + D2 + D3 - 1 (T)", (kt.d1 + kt.d2 + kt.d3 - 1.0).abs(), 1e-15);
    let n = 200_000usize;
    let h = 60.0 / n as f64;
    let simpson = |g: &dyn Fn(f64) -> f64| {
        let mut s = g(0.0) + g(60.0);
        for i in 1..n {
            s += g(i as f64 * h) * if i % 2 == 1 { 4.0 } else { 2.0 };
        }
        s * h / 3.0
    };
    c.at_most("C3 int p dZ against 1", (simpson(&|z| k.density(z)) - 1.0).abs(), 1e-8);
    for m in [3.0, 5.0] {
        let num = simpson(&|z| z.powf(m) * k.density(z));
        let cf = k.amplitude_moment(m).unwrap();
        println!("C3 m = {m}: quadrature {num:.12e}, closed form {cf:.12e}");
        c.at_most(
            &format!("C3 int Z^{m} p dZ against the closed form"),
            rel(num, cf),
            1e-8,
        );
    }
    c.finish();
}

// -------------------------------------------------------------------------------------------------------------- C4

#[test]
fn c4_c5_two_mode_synthesised_history() {
    let mut c = Checks::new();
    let t = two_mode_moments();
    let curve = SnCurve::power(3.0, 1.0).unwrap();
    let bb = t.broadband_damage_rate(3.0, 1.0).unwrap();
    let nb = t.narrow_band_damage_rate(3.0, 1.0).unwrap();
    println!("T moments {t:?}, broadband {bb:.9e}, narrow band {nb:.9e}");
    // The synthesis: random phases and Rayleigh amplitudes on the bins k/T of the one-sided density.
    let fs = 6000.0;
    let n = 1usize << 23;
    let dur = n as f64 / fs;
    let df = 1.0 / dur;
    let kmax = (FBAND * dur).floor() as usize;
    let mut rng = Rng::new(SEED);
    let mut re = vec![0.0; n];
    let mut im = vec![0.0; n];
    for k in 1..=kmax {
        let (u1, u2) = (rng.uniform(), rng.uniform());
        let r = (-2.0 * (1.0 - u1).ln()).sqrt();
        let a = (two_mode_density(k as f64 * df) * df).sqrt() * r;
        let (sn, cs) = (2.0 * PI * u2).sin_cos();
        re[k] = a * cs;
        im[k] = a * sn;
    }
    fft(&mut re, &mut im);
    let x = re;
    drop(im);
    let var = x.iter().map(|v| v * v).sum::<f64>() / n as f64;
    let counts = rainflow_periodic(&x).unwrap();
    let d = miner(&counts.cycles, &curve, &MeanStressCorrection::none()).unwrap();
    let counted = d / dur;
    println!(
        "C4: {} cycles, duration {dur:.3} s, variance {var:.6e} (m0 {:.6e}, ratio {:.5}), counted rate {counted:.9e}",
        counts.cycles.len(),
        t.m0,
        var / t.m0
    );
    println!(
        "C4: counted/broadband {:.6}, narrow/broadband {:.6}, narrow/counted {:.6}",
        counted / bb,
        nb / bb,
        nb / counted
    );
    c.at_most("C4 |counted/broadband - 1|", rel(counted, bb), 0.1);
    c.holds(
        "C4b at least 1e5 closed cycles",
        counts.cycles.len() >= 100_000,
        &format!("{}", counts.cycles.len()),
    );
    c.above("C5 narrow band over broadband", nb / bb, 1.0);
    c.above("C5b narrow band over counted", nb / counted, 1.0);
    c.at_least("C5c narrow band over broadband minus 1", nb / bb - 1.0, 0.10);
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------- C7, C8

#[test]
fn c7_c8_refusals_and_scaling() {
    let mut c = Checks::new();
    let ok = (4.0e-7, 4.0e-5, 5.0e-3, 116.0);
    let rate = |m0: f64, m1: f64, m2: f64, m4: f64| broadband_damage_rate(m0, m1, m2, m4, 3.0, 1.0);
    c.holds(
        "C7 the base moments are accepted",
        rate(ok.0, ok.1, ok.2, ok.3).is_ok(),
        "",
    );
    for (name, v) in [
        ("m0 = 0", 0.0),
        ("m0 < 0", -1.0),
        ("m0 NaN", f64::NAN),
        ("m0 inf", f64::INFINITY),
    ] {
        let (good, msg) = refused(rate(v, ok.1, ok.2, ok.3), &["moment m0"]);
        c.holds(&format!("C7 {name} refused"), good, &msg);
    }
    for (name, v) in [("m2 = 0", 0.0), ("m2 < 0", -1.0), ("m2 NaN", f64::NAN)] {
        let (good, msg) = refused(rate(ok.0, ok.1, v, ok.3), &["moment m2"]);
        c.holds(&format!("C7 {name} refused"), good, &msg);
    }
    let (good, msg) = refused(Moments::default().broadband_damage_rate(3.0, 1.0), &["moment m0"]);
    c.holds("C7 Moments::default refused", good, &msg);
    for (name, v) in [("m4 = 0", 0.0), ("m4 < 0", -3.0)] {
        let (good, msg) = refused(rate(ok.0, ok.1, ok.2, v), &["moment m4"]);
        c.holds(&format!("C7b {name} refused"), good, &msg);
    }
    let (good, msg) = refused(rate(ok.0, -1.0, ok.2, ok.3), &["moment m1"]);
    c.holds("C7b m1 < 0 refused", good, &msg);
    let (good, msg) = refused(rate(ok.0, ok.1, ok.2, 1e-3), &["gamma"]);
    c.holds("C7b m2^2 > m0 m4 refused", good, &msg);
    let (good, msg) = refused(broadband_damage_rate(ok.0, ok.1, ok.2, ok.3, 0.0, 1.0), &["exponent m"]);
    c.holds("C7b m = 0 refused", good, &msg);
    let (good, msg) = refused(broadband_damage_rate(ok.0, ok.1, ok.2, ok.3, 3.0, 0.0), &["constant c"]);
    c.holds("C7b c = 0 refused", good, &msg);
    let table = SnCurve::table(&[1e3, 1e5], &[300.0, 100.0]).unwrap();
    let (good, msg) = refused(
        broadband_damage_rate_curve(ok.0, ok.1, ok.2, ok.3, &table),
        &["power law"],
    );
    c.holds("C7b table curve refused", good, &msg);
    let endurance = SnCurve::power(3.0, 1.0).unwrap().with_endurance(1e-4).unwrap();
    let (good, msg) = refused(
        broadband_damage_rate_curve(ok.0, ok.1, ok.2, ok.3, &endurance),
        &["power law"],
    );
    c.holds("C7b curve with endurance limit refused", good, &msg);
    // C8: scaling, on the moments of T.
    let t = two_mode_moments();
    let base = broadband_damage_rate(t.m0, t.m1, t.m2, t.m4, 3.0, 2.5).unwrap();
    let k = 3.7f64;
    let by_amp = broadband_damage_rate(t.m0 * k * k, t.m1 * k * k, t.m2 * k * k, t.m4 * k * k, 3.0, 2.5).unwrap();
    c.at_most("C8 amplitude scaling k^m", rel(by_amp, base * k.powi(3)), 1e-12);
    let s = 1.9f64;
    let by_freq = broadband_damage_rate(t.m0, t.m1 * s, t.m2 * s * s, t.m4 * s.powi(4), 3.0, 2.5).unwrap();
    c.at_most("C8 frequency scaling s", rel(by_freq, base * s), 1e-12);
    c.finish();
}
