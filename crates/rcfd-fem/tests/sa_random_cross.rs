//! F-567 (ADR 0510): complex cross-spectral matrices and frequency-dependent coherence of random loads. Every check, limit
//! and negative control is the one registered in the ADR before the run; every aggregated error is asserted finite.

use rcfd_fem::builders::hex_box;
use rcfd_fem::harmonic::{HarmonicOptions, ModalBasis, ModalDamping};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::random::{
    check_cross_matrix, exponential_coherence, spectral_response, spectral_response_cross, ChannelResponse, Coherence,
    CrossSpectrum, Excitation, ModalChannels, RandomRequest, RandomSolver, SpectralOptions, Spectrum,
};
use rcfd_fem::{Constraint, FemError, Material, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;

const ZETA: f64 = 0.02;
const FN: f64 = 100.0;
const U1: f64 = 1.0;
const U2: f64 = 1.7;

/// A list of measured values against their limits; every value is printed, the test fails at the end if any missed.
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
            self.0.push(format!("{name} = {value:e} > {limit:e} (or not finite)"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.6e} (expected >= {limit:e}) {}",
            if ok { "as expected" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e} (or not finite)"));
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
    (a - b).abs() / b.abs()
}

fn omega_of(f: f64) -> f64 {
    2.0 * PI * f
}

fn mode_terms(f_mode: f64) -> (f64, f64) {
    let w = omega_of(f_mode);
    (w * w, 2.0 * ZETA * w)
}

/// The complex response `1/(lambda - omega^2 + i c omega)` of a mode at the frequency `f`.
fn h_mode(f_mode: f64, f: f64) -> (f64, f64) {
    let (lam, c) = mode_terms(f_mode);
    let om = omega_of(f);
    let a = lam - om * om;
    let b = c * om;
    let d = a * a + b * b;
    (a / d, -b / d)
}

fn cmul(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}

fn norm2(a: (f64, f64)) -> f64 {
    a.0 * a.0 + a.1 * a.1
}

// ------------------------------------------------------------------------------------- the independent quadrature (D8)

/// 8-point Gauss-Legendre nodes and weights by Newton iteration on the Legendre polynomial.
fn gauss_legendre8() -> (Vec<f64>, Vec<f64>) {
    let n = 8usize;
    let mut x = vec![0.0; n];
    let mut w = vec![0.0; n];
    for i in 0..n {
        let mut z = (PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
        let mut dp = 1.0;
        for _ in 0..100 {
            let (mut p0, mut p1) = (1.0, z);
            for k in 2..=n {
                let p2 = ((2 * k - 1) as f64 * z * p1 - (k - 1) as f64 * p0) / k as f64;
                p0 = p1;
                p1 = p2;
            }
            dp = n as f64 * (z * p1 - p0) / (z * z - 1.0);
            let dz = p1 / dp;
            z -= dz;
            if dz.abs() < 1e-16 {
                break;
            }
        }
        x[i] = z;
        w[i] = 2.0 / ((1.0 - z * z) * dp * dp);
    }
    (x, w)
}

/// `int_0^fmax fun df` by composite 8-point Gauss-Legendre on steps of 0.05 Hz.
fn quad(fun: &dyn Fn(f64) -> f64, fmax: f64) -> f64 {
    let (x, w) = gauss_legendre8();
    let steps = (fmax / 0.05).round() as usize;
    let h = fmax / steps as f64;
    let mut sum = 0.0;
    for s in 0..steps {
        let mid = (s as f64 + 0.5) * h;
        for k in 0..8 {
            sum += w[k] * fun(mid + 0.5 * h * x[k]);
        }
    }
    0.5 * h * sum
}

// ------------------------------------------------------------------------------------------------------ the channels

fn one_mode(u: &[f64]) -> ModalChannels {
    let (lam, c) = mode_terms(FN);
    ModalChannels::new(
        vec![lam],
        vec![c],
        u.iter().map(|&u| vec![vec![u]]).collect(),
        vec![vec![0.0]; u.len()],
    )
    .unwrap()
}

/// Two modes (60 and 140 Hz), two patterns, one output (ADR 0510 arithmetic 4): `Y_1 = H_1 + 0.5 H_2`, `Y_2 = 0.4 H_1 - 1.2 H_2`.
fn two_mode(swap: bool) -> ModalChannels {
    let (l1, c1) = mode_terms(60.0);
    let (l2, c2) = mode_terms(140.0);
    let mut gains = vec![vec![vec![1.0, 0.5]], vec![vec![0.4, -1.2]]];
    if swap {
        gains.reverse();
    }
    ModalChannels::new(vec![l1, l2], vec![c1, c2], gains, vec![vec![0.0]; 2]).unwrap()
}

fn y_two_mode(f: f64) -> ((f64, f64), (f64, f64)) {
    let (h1, h2) = (h_mode(60.0, f), h_mode(140.0, f));
    (
        (h1.0 + 0.5 * h2.0, h1.1 + 0.5 * h2.1),
        (0.4 * h1.0 - 1.2 * h2.0, 0.4 * h1.1 - 1.2 * h2.1),
    )
}

fn flat2(fmax: f64) -> Vec<Spectrum> {
    vec![Spectrum::flat(0.0, fmax, 1.0).unwrap(); 2]
}

fn delay_function(tau: f64, sign: f64) -> Coherence {
    Coherence::function(2, move |f, _, _| {
        let p = omega_of(f) * tau;
        (p.cos(), -sign * p.sin())
    })
}

fn m0_cross(ch: &ModalChannels, cross: &CrossSpectrum) -> f64 {
    spectral_response_cross(ch, cross, &SpectralOptions::default())
        .unwrap()
        .moments[0]
        .m0
}

/// The closed form (CF) of ADR 0510 for the infinite band, unit `G`.
fn closed_form(tau: f64, u1: f64, u2: f64) -> f64 {
    let (lam, c) = mode_terms(FN);
    let wn = omega_of(FN);
    let wd = wn * (1.0 - ZETA * ZETA).sqrt();
    let rho = (-ZETA * wn * tau).exp() * ((wd * tau).cos() + ZETA * wn / wd * (wd * tau).sin());
    (u1 * u1 + u2 * u2 + 2.0 * u1 * u2 * rho) / (4.0 * lam * c)
}

fn one_mode_integrand(f: f64, gamma_re: f64) -> f64 {
    norm2(h_mode(FN, f)) * (U1 * U1 + U2 * U2 + 2.0 * U1 * U2 * gamma_re)
}

// ------------------------------------------------------------------------------------------------------ the refusals

fn cross_key_message(e: &FemError) -> Option<String> {
    match e {
        FemError::InvalidValue { key, reason } if *key == "cross spectrum" => Some(reason.clone()),
        _ => None,
    }
}

type Tables = (Vec<f64>, Vec<Vec<Vec<f64>>>, Vec<Vec<Vec<f64>>>);

/// A 2-pattern table at `freqs`: `G_11 = G_22 = 1`, `G_12 = base` everywhere except the entry `at`, where it is `special`;
/// the lower entry is the conjugate unless `broken`.
fn table2(freqs: &[f64], base: (f64, f64), at: usize, special: (f64, f64), broken: bool) -> Tables {
    let mut re = Vec::new();
    let mut im = Vec::new();
    for k in 0..freqs.len() {
        let c = if k == at { special } else { base };
        let lower_im = if k == at && broken { c.1 } else { -c.1 };
        re.push(vec![vec![1.0, c.0], vec![c.0, 1.0]]);
        im.push(vec![vec![0.0, c.1], vec![lower_im, 0.0]]);
    }
    (freqs.to_vec(), re, im)
}

#[test]
fn h1_to_h5_refusals_and_acceptances() {
    let mut c = Checks::new();
    let freqs = [100.0, 125.0, 150.0, 175.0, 200.0];
    // H1: not Hermitian at the entry 2 (150 Hz).
    let (f, re, im) = table2(&freqs, (0.3, 0.0), 2, (0.2, 0.5), true);
    let e = CrossSpectrum::table(f, re, im).unwrap_err();
    let msg = cross_key_message(&e).unwrap_or_default();
    c.holds(
        "H1 non-Hermitian entry refused with the frequency and the pair",
        msg.contains("1.5e2 Hz") && msg.contains("(0, 1)"),
        &msg,
    );
    // H2: the positive semi-definite boundary at the entry 1 (200 Hz of three).
    let f3 = [100.0, 200.0, 300.0];
    let unit = |s: f64| (0.6 * s, 0.8 * s);
    for (name, s, refused) in [
        ("1.1", 1.1, true),
        ("1 (rank one)", 1.0, false),
        ("1 + 1e-13", 1.0 + 1e-13, false),
        ("1 + 1e-10", 1.0 + 1e-10, true),
    ] {
        let (f, re, im) = table2(&f3, (0.2, 0.0), 1, unit(s), false);
        let r = CrossSpectrum::table(f, re, im);
        match (&r, refused) {
            (Err(e), true) => {
                let msg = cross_key_message(e).unwrap_or_default();
                c.holds(
                    &format!("H2 |G_12| = {name} refused naming 2e2 Hz"),
                    msg.contains("2e2 Hz"),
                    &msg,
                );
            }
            (Ok(_), false) => c.holds(&format!("H2 |G_12| = {name} accepted"), true, ""),
            _ => c.holds(
                &format!("H2 |G_12| = {name}"),
                false,
                &format!("refused = {} but expected {refused}", r.is_err()),
            ),
        }
    }
    // H3: further refusals.
    let ok = || table2(&f3, (0.2, 0.0), 9, (0.0, 0.0), false);
    let (f, mut re, im) = ok();
    re[1][0][0] = -1.0;
    let r = CrossSpectrum::table(f, re, im);
    c.holds(
        "H3 negative diagonal refused naming 2e2 Hz",
        r.as_ref()
            .err()
            .and_then(cross_key_message)
            .is_some_and(|m| m.contains("2e2 Hz")),
        "",
    );
    let (f, re, mut im) = ok();
    im[1][1][1] = 1e-6;
    let r = CrossSpectrum::table(f, re, im);
    c.holds(
        "H3 imaginary diagonal refused naming 2e2 Hz",
        r.as_ref()
            .err()
            .and_then(cross_key_message)
            .is_some_and(|m| m.contains("2e2 Hz")),
        "",
    );
    let (f, mut re, im) = ok();
    re[2][0][1] = f64::NAN;
    let r = CrossSpectrum::table(f, re, im);
    c.holds(
        "H3 NaN refused naming 3e2 Hz",
        r.as_ref()
            .err()
            .and_then(cross_key_message)
            .is_some_and(|m| m.contains("3e2 Hz")),
        "",
    );
    let (_, re, im) = ok();
    let r = CrossSpectrum::table(vec![100.0, 100.0, 300.0], re, im);
    c.holds(
        "H3 frequencies not increasing refused with the entry 1",
        r.as_ref()
            .err()
            .and_then(cross_key_message)
            .is_some_and(|m| m.contains("entry 1")),
        "",
    );
    let (f, mut re, im) = ok();
    re.pop();
    c.holds(
        "H3 wrong number of matrices refused",
        CrossSpectrum::table(f, re, im).is_err(),
        "",
    );
    let zero = vec![vec![vec![0.0; 2]; 2]; 3];
    c.holds(
        "H3 an all-zero table accepted",
        CrossSpectrum::table(f3.to_vec(), zero.clone(), zero).is_ok(),
        "",
    );
    // H4: a function coherence that fails above 300 Hz is refused at a node naming the frequency.
    let bad = Coherence::function(2, |f, _, _| if f < 300.0 { (0.5, 0.0) } else { (1.2, 0.0) });
    let cs = CrossSpectrum::coherent(flat2(1000.0), bad).unwrap();
    let r = spectral_response_cross(&one_mode(&[U1, U2]), &cs, &SpectralOptions::default());
    let named = r
        .as_ref()
        .err()
        .and_then(cross_key_message)
        .and_then(|m| {
            let tail = m.split("at ").nth(1)?.to_string();
            tail.split(" Hz").next()?.parse::<f64>().ok()
        })
        .unwrap_or(f64::NAN);
    c.holds(
        "H4 function coherence refused naming a frequency in (300, 310)",
        named > 300.0 && named < 310.0,
        &format!("named {named}"),
    );
    // H5: a rank-2 three-pattern table G = B B^H accepted.
    let b = [
        [(1.0, 0.0), (0.0, 1.0)],
        [(0.5, 0.5), (1.0, -0.3)],
        [(-0.2, 0.7), (0.4, 0.4)],
    ];
    let mut re = Vec::new();
    let mut im = Vec::new();
    for s in [1.0, 2.5] {
        let mut gr = vec![vec![0.0; 3]; 3];
        let mut gi = vec![vec![0.0; 3]; 3];
        for a in 0..3 {
            for bb in 0..3 {
                for k in 0..2 {
                    let p = cmul(b[a][k], (b[bb][k].0, -b[bb][k].1));
                    gr[a][bb] += s * p.0;
                    gi[a][bb] += s * p.1;
                }
            }
        }
        re.push(gr);
        im.push(gi);
    }
    c.holds(
        "H5 rank-2 three-pattern complex table accepted",
        CrossSpectrum::table(vec![50.0, 500.0], re.clone(), im.clone()).is_ok()
            && check_cross_matrix(50.0, &re[0], &im[0]).is_ok(),
        "",
    );
    c.finish();
}

// ----------------------------------------------------------------------------------------------------- bit for bit

fn spectra3() -> Vec<Spectrum> {
    vec![
        Spectrum::flat(10.0, 900.0, 2.0).unwrap(),
        Spectrum::new(vec![20.0, 80.0, 400.0, 700.0], vec![1.0, 3.0, 3.0, 0.5]).unwrap(),
        Spectrum::log_log(vec![30.0, 120.0, 600.0], vec![0.5, 4.0, 1.0]).unwrap(),
    ]
}

fn channels3() -> ModalChannels {
    let freqs = [60.0_f64, 140.0, 310.0];
    let eig: Vec<f64> = freqs.iter().map(|f| omega_of(*f).powi(2)).collect();
    let damp: Vec<f64> = freqs.iter().map(|f| 2.0 * ZETA * omega_of(*f)).collect();
    let gains: Vec<Vec<Vec<f64>>> = (0..3)
        .map(|a| {
            (0..6)
                .map(|o| {
                    (0..3)
                        .map(|r| 1e3 * (0.3 + 0.1 * a as f64 - 0.2 * o as f64 + 0.35 * r as f64 * (a as f64 - 1.0)))
                        .collect()
                })
                .collect()
        })
        .collect();
    let res: Vec<Vec<f64>> = (0..3)
        .map(|a| (0..6).map(|o| 1e-3 * (a as f64 + 0.5 * o as f64)).collect())
        .collect();
    ModalChannels::new(eig, damp, gains, res).unwrap()
}

fn rho3() -> Vec<Vec<f64>> {
    vec![vec![1.0, 0.6, -0.3], vec![0.6, 1.0, 0.2], vec![-0.3, 0.2, 1.0]]
}

fn bits_of(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn same_bits(a: &ChannelResponse, b: &ChannelResponse) -> bool {
    let flat = |r: &ChannelResponse| -> Vec<u64> {
        let mut w = bits_of(&r.frequencies);
        w.extend(bits_of(&r.weights));
        for p in &r.psd {
            w.extend(bits_of(p));
        }
        for m in &r.moments {
            w.extend([m.m0.to_bits(), m.m1.to_bits(), m.m2.to_bits(), m.m4.to_bits()]);
        }
        for c in r.covariance.iter().chain(&r.rate_covariance) {
            w.extend(bits_of(c));
        }
        w
    };
    let (x, y) = (flat(a), flat(b));
    !x.is_empty() && x == y
}

#[test]
fn b1_to_b4_real_coherence_reproduces_f417_bit_for_bit() {
    let mut c = Checks::new();
    let ch = channels3();
    let sp = spectra3();
    let rho = rho3();
    let n3 = 3;
    let const_table = |freqs: Vec<f64>| -> Coherence {
        let re: Vec<Vec<Vec<f64>>> = freqs.iter().map(|_| rho.clone()).collect();
        let im: Vec<Vec<Vec<f64>>> = freqs.iter().map(|_| vec![vec![0.0; n3]; n3]).collect();
        Coherence::table(freqs, re, im).unwrap()
    };
    for group in [1usize, 3] {
        let base = spectral_response(&ch, &sp, Some(&rho), &SpectralOptions::default().group(group)).unwrap();
        let o = SpectralOptions::default().group(group);
        let by_const = spectral_response_cross(
            &ch,
            &CrossSpectrum::coherent(sp.clone(), Coherence::constant(rho.clone()).unwrap()).unwrap(),
            &o,
        )
        .unwrap();
        c.holds(
            &format!("B1 constant coherence, group {group}"),
            same_bits(&base, &by_const),
            "bits equal",
        );
        let tab = CrossSpectrum::coherent(sp.clone(), const_table(vec![20.0, 300.0, 700.0])).unwrap();
        let by_table = spectral_response_cross(&ch, &tab, &o).unwrap();
        c.holds(
            &format!("B2 real constant coherence table (routed), group {group}"),
            same_bits(&base, &by_table),
            "bits equal",
        );
        let off = o.real_route(false);
        let general_const = spectral_response_cross(
            &ch,
            &CrossSpectrum::coherent(sp.clone(), Coherence::constant(rho.clone()).unwrap()).unwrap(),
            &off,
        )
        .unwrap();
        let outside = CrossSpectrum::coherent(sp.clone(), const_table(vec![1.0, 2.0, 5000.0])).unwrap();
        let general_table = spectral_response_cross(&ch, &outside, &off).unwrap();
        c.holds(
            &format!("B3 general loop, constant coherence, group {group}"),
            same_bits(&base, &general_const),
            "bits equal",
        );
        c.holds(
            &format!("B3 general loop, coherence table outside the band, group {group}"),
            same_bits(&base, &general_table),
            "bits equal",
        );
    }
    // B4: a real table G_ab(f_k) from flat spectra and rho (same breaks as the spectra).
    let levels = [1.0, 2.0, 4.0];
    let flat: Vec<Spectrum> = levels
        .iter()
        .map(|&g| Spectrum::flat(10.0, 900.0, g).unwrap())
        .collect();
    let mk = || -> Vec<Vec<f64>> {
        (0..3)
            .map(|a| {
                (0..3)
                    .map(|b| {
                        if a == b {
                            levels[a]
                        } else {
                            rho[a][b] * (levels[a] * levels[b]).sqrt()
                        }
                    })
                    .collect()
            })
            .collect()
    };
    let table = CrossSpectrum::table(vec![10.0, 900.0], vec![mk(), mk()], vec![vec![vec![0.0; 3]; 3]; 2]).unwrap();
    for group in [1usize, 3] {
        let o = SpectralOptions::default().group(group);
        let base = spectral_response(&ch, &flat, Some(&rho), &o).unwrap();
        let t = spectral_response_cross(&ch, &table, &o).unwrap();
        c.holds(
            &format!("B4 real table G_ab(f_k) from flat spectra, group {group}"),
            same_bits(&base, &t),
            "bits equal",
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------ the delay

#[test]
fn d1_to_d6_pure_delay() {
    let mut c = Checks::new();
    let one = one_mode(&[U1, U2]);
    for tau in [1.0 / (8.0 * FN), 3.0 / (8.0 * FN)] {
        let name = if tau < 0.002 { "1/(8 f_n)" } else { "3/(8 f_n)" };
        // D1: closed form on [0, 400 f_n].
        let cs = CrossSpectrum::coherent(flat2(400.0 * FN), delay_function(tau, 1.0)).unwrap();
        let ms = m0_cross(&one, &cs);
        c.at_most(
            &format!("D1 tau = {name}, closed form, band [0, 400 f_n]"),
            rel(ms, closed_form(tau, U1, U2)),
            1e-6,
        );
        // D2: [0, 1000] against the fine quadrature.
        let cs = CrossSpectrum::coherent(flat2(1000.0), delay_function(tau, 1.0)).unwrap();
        let ms = m0_cross(&one, &cs);
        let reference = quad(&|f| one_mode_integrand(f, (omega_of(f) * tau).cos()), 1000.0);
        c.at_most(
            &format!("D2 tau = {name}, fine quadrature, band [0, 1000]"),
            rel(ms, reference),
            1e-6,
        );
        // D6: the density at the nodes.
        let r = spectral_response_cross(&one, &cs, &SpectralOptions::default()).unwrap();
        let worst = r
            .frequencies
            .iter()
            .zip(&r.psd[0])
            .map(|(&f, &s)| rel(s, one_mode_integrand(f, (omega_of(f) * tau).cos())))
            .fold(0.0, f64::max);
        c.at_most(&format!("D6 tau = {name}, density at the nodes"), worst, 1e-12);
    }
    // D3: the table, h = 0.25 Hz, tau = 1/(8 f_n).
    let tau = 1.0 / (8.0 * FN);
    let freqs: Vec<f64> = (0..=4000).map(|k| 0.25 * k as f64).collect();
    let re: Vec<Vec<Vec<f64>>> = freqs
        .iter()
        .map(|&f| {
            let p = (omega_of(f) * tau).cos();
            vec![vec![1.0, p], vec![p, 1.0]]
        })
        .collect();
    let im: Vec<Vec<Vec<f64>>> = freqs
        .iter()
        .map(|&f| {
            let s = (omega_of(f) * tau).sin();
            vec![vec![0.0, -s], vec![s, 0.0]]
        })
        .collect();
    let table = CrossSpectrum::table(freqs, re, im).unwrap();
    let ms = m0_cross(&one, &table);
    let reference = quad(&|f| one_mode_integrand(f, (omega_of(f) * tau).cos()), 1000.0);
    c.at_most("D3 table of 4001 entries, tau = 1/(8 f_n)", rel(ms, reference), 1e-6);
    // D4 and the negative controls N1, N2: two modes, two loads of different shapes.
    let two = two_mode(false);
    for tau in [1.0 / (8.0 * FN), 3.0 / (8.0 * FN)] {
        let name = if tau < 0.002 { "1/(8 f_n)" } else { "3/(8 f_n)" };
        let cs = CrossSpectrum::coherent(flat2(1000.0), delay_function(tau, 1.0)).unwrap();
        let ms = m0_cross(&two, &cs);
        let reference = quad(
            &|f| {
                let (y1, y2) = y_two_mode(f);
                let p = omega_of(f) * tau;
                let z = cmul(y2, (p.cos(), -p.sin()));
                norm2((y1.0 + z.0, y1.1 + z.1))
            },
            1000.0,
        );
        c.at_most(&format!("D4 two modes, tau = {name}"), rel(ms, reference), 1e-6);
        let opposite = quad(
            &|f| {
                let (y1, y2) = y_two_mode(f);
                let p = omega_of(f) * tau;
                let z = cmul(y2, (p.cos(), p.sin()));
                norm2((y1.0 + z.0, y1.1 + z.1))
            },
            1000.0,
        );
        c.at_least(
            &format!("N1 tau = {name}, the opposite sign of the delay differs"),
            rel(opposite, reference),
            1e-2,
        );
        let real_only = quad(
            &|f| {
                let (y1, y2) = y_two_mode(f);
                let p = omega_of(f) * tau;
                let cross = cmul((y1.0, -y1.1), y2);
                norm2(y1) + norm2(y2) + 2.0 * cross.0 * p.cos()
            },
            1000.0,
        );
        c.at_least(
            &format!("N2 tau = {name}, the imaginary part dropped differs"),
            rel(real_only, reference),
            5e-3,
        );
        // D5: exchange the patterns and conjugate the table.
        let swapped = two_mode(true);
        let cs2 = CrossSpectrum::coherent(flat2(1000.0), delay_function(tau, -1.0)).unwrap();
        let ms2 = m0_cross(&swapped, &cs2);
        c.at_most(
            &format!("D5 tau = {name}, patterns exchanged and table conjugated"),
            rel(ms2, ms),
            1e-13,
        );
    }
    // N4: without the cross term the closed form is far away.
    let alone = one_mode(&[U1, 0.0]);
    let cs = CrossSpectrum::coherent(flat2(400.0 * FN), delay_function(1.0 / (8.0 * FN), 1.0)).unwrap();
    c.at_least(
        "N4 no cross term differs from the closed form",
        rel(m0_cross(&alone, &cs), closed_form(1.0 / (8.0 * FN), U1, U2)),
        0.1,
    );
    c.finish();
}

// -------------------------------------------------------------------------------------------- frequency dependence

#[test]
fn f1_f2_e1_frequency_dependent_coherence_and_exponential_matrix() {
    let mut c = Checks::new();
    let one = one_mode(&[U1, U2]);
    // F1: gamma(f) = exp(-f/200).
    let g = Coherence::function(2, |f, _, _| ((-f / 200.0).exp(), 0.0));
    let ms = m0_cross(&one, &CrossSpectrum::coherent(flat2(1000.0), g).unwrap());
    let reference = quad(&|f| one_mode_integrand(f, (-f / 200.0).exp()), 1000.0);
    c.at_most("F1 gamma(f) = exp(-f/200)", rel(ms, reference), 1e-6);
    // F2: a table linear in f, three rows.
    let rows = [(0.0, 0.9), (500.0, 0.5), (1000.0, 0.1)];
    let freqs: Vec<f64> = rows.iter().map(|r| r.0).collect();
    let re: Vec<Vec<Vec<f64>>> = rows.iter().map(|r| vec![vec![1.0, r.1], vec![r.1, 1.0]]).collect();
    let im = vec![vec![vec![0.0; 2]; 2]; 3];
    let tab = Coherence::table(freqs, re, im).unwrap();
    let ms = m0_cross(&one, &CrossSpectrum::coherent(flat2(1000.0), tab).unwrap());
    let gamma = |f: f64| -> f64 {
        let i = if f < 500.0 { 0 } else { 1 };
        let (f0, g0) = rows[i];
        let (f1, g1) = rows[i + 1];
        g0 + (f - f0) / (f1 - f0) * (g1 - g0)
    };
    let reference = quad(&|f| one_mode_integrand(f, gamma(f)), 1000.0);
    c.at_most("F2 coherence table linear in f", rel(ms, reference), 1e-6);
    // E1: the matrix.
    let pos = [[0.0, 0.0, 0.0], [3.0, 4.0, 0.0], [0.0, 0.0, 2.0]];
    let l = 7.0;
    let m = exponential_coherence(&pos, l).unwrap();
    let want = [
        [1.0, (-5.0_f64 / l).exp(), (-2.0_f64 / l).exp()],
        [(-5.0_f64 / l).exp(), 1.0, (-(29.0_f64).sqrt() / l).exp()],
        [(-2.0_f64 / l).exp(), (-(29.0_f64).sqrt() / l).exp(), 1.0],
    ];
    let worst = (0..3)
        .flat_map(|a| (0..3).map(move |b| (a, b)))
        .map(|(a, b)| rel(m[a][b], want[a][b]))
        .fold(0.0, f64::max);
    c.at_most("E1 exponential_coherence entries", worst, 1e-15);
    c.holds(
        "E1 refuses a non-positive length",
        exponential_coherence(&pos, 0.0).is_err(),
        "",
    );
    c.finish();
}

// ----------------------------------------------------------------------------------------------- the solid-model path

fn cantilever_patterns() -> (SolidModel, Vec<f64>, Vec<f64>, Vec<usize>) {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.08], [16, 2, 2]).unwrap();
    let nodes = mesh.nodes().to_vec();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.1e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let dofs = m.dof_map();
    let find = |p: [f64; 3]| nodes.iter().position(|q| *q == p).unwrap();
    let (a, b) = (find([1.0, 0.1, 0.08]), find([1.0, 0.0, 0.0]));
    let n = dofs.n_equations();
    let mut p1 = vec![0.0; n];
    let mut p2 = vec![0.0; n];
    p1[dofs.equation(a, 1).unwrap()] = 1.0;
    p2[dofs.equation(b, 1).unwrap()] = 1.0;
    let eqs: Vec<usize> = [a, b]
        .iter()
        .flat_map(|&v| (0..3).map(move |c| (v, c)))
        .map(|(v, c)| dofs.equation(v, c).unwrap())
        .collect();
    (m, p1, p2, eqs)
}

#[test]
fn b5_solver_path_with_stress_is_bit_identical_for_a_real_coherence() {
    let mut c = Checks::new();
    let (m, p1, p2, eqs) = cantilever_patterns();
    let res = modal_analysis(
        &m,
        &ModalOptions::new(8)
            .tolerance(1e-12)
            .residual_test(ResidualTest::Backward)
            .threads(1),
    )
    .unwrap();
    let solver = RandomSolver::new(
        &m,
        ModalBasis::from_result(&res).unwrap(),
        &HarmonicOptions::new(ModalDamping::Ratio(ZETA)).threads(1),
    )
    .unwrap();
    let g = Spectrum::flat(5.0, 2000.0, 1.0).unwrap();
    let g2 = Spectrum::new(vec![5.0, 300.0, 2000.0], vec![1.0, 4.0, 0.5]).unwrap();
    let rho = vec![vec![1.0, 0.7], vec![0.7, 1.0]];
    let patterns = vec![p1, p2];
    let spectra = vec![g, g2];
    let a = Excitation::forces(patterns.clone(), spectra.clone(), Some(rho.clone())).unwrap();
    let cross = CrossSpectrum::coherent(spectra, Coherence::constant(rho).unwrap()).unwrap();
    let b = Excitation::cross(patterns, cross).unwrap();
    let req = RandomRequest::new(eqs).stress(true);
    let ra = solver.response(&m, &a, &req).unwrap();
    let rb = solver.response(&m, &b, &req).unwrap();
    let mut wa = Vec::new();
    let mut wb = Vec::new();
    for (x, y) in [(&ra, &mut wa), (&rb, &mut wb)] {
        for p in &x.psd {
            y.extend(bits_of(p));
        }
        for mo in &x.moments {
            y.extend([mo.m0.to_bits(), mo.m1.to_bits(), mo.m2.to_bits(), mo.m4.to_bits()]);
        }
        let s = x.stress.as_ref().unwrap();
        for cv in s.covariance.iter().chain(&s.rate_covariance) {
            for row in cv {
                y.extend(bits_of(row));
            }
        }
    }
    c.holds(
        "B5 solver path with stress, cross against forces",
        !wa.is_empty() && wa == wb,
        &format!("{} words", wa.len()),
    );
    c.finish();
}

// --------------------------------------------------------------------------------------------------------- the plate

struct Plate {
    model: SolidModel,
    solver: RandomSolver,
    patterns: Vec<Vec<f64>>,
    positions: Vec<[f64; 3]>,
    output: usize,
}

fn plate() -> Plate {
    let mesh = hex_box([0.0; 3], [0.4, 0.4, 0.01], [8, 8, 1]).unwrap();
    let nodes = mesh.nodes().to_vec();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.1e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let res = modal_analysis(
        &m,
        &ModalOptions::new(12)
            .tolerance(1e-12)
            .residual_test(ResidualTest::Backward)
            .threads(1),
    )
    .unwrap();
    let solver = RandomSolver::new(
        &m,
        ModalBasis::from_result(&res).unwrap(),
        &HarmonicOptions::new(ModalDamping::Ratio(ZETA))
            .static_correction(false)
            .threads(1),
    )
    .unwrap();
    let dofs = m.dof_map();
    let near = |p: [f64; 3]| -> usize {
        nodes
            .iter()
            .position(|q| (0..3).all(|k| (q[k] - p[k]).abs() < 1e-9))
            .expect("plate node")
    };
    let n = dofs.n_equations();
    let mut patterns = Vec::new();
    let mut positions = Vec::new();
    for x in [0.15, 0.25, 0.35] {
        for y in [0.1, 0.2, 0.3] {
            let node = near([x, y, 0.01]);
            let mut p = vec![0.0; n];
            p[dofs.equation(node, 2).unwrap()] = 1.0;
            patterns.push(p);
            positions.push([x, y, 0.01]);
        }
    }
    let output = dofs.equation(near([0.4, 0.2, 0.01]), 2).unwrap();
    Plate {
        model: m,
        solver,
        patterns,
        positions,
        output,
    }
}

struct PlateResult {
    m0: f64,
    vm: Vec<f64>,
}

fn plate_run(p: &Plate, ex: &Excitation) -> PlateResult {
    let r = p
        .solver
        .response(&p.model, ex, &RandomRequest::new(vec![p.output]).stress(true))
        .unwrap();
    let s = r.stress.as_ref().unwrap();
    PlateResult {
        m0: r.moments[0].m0,
        vm: (0..s.covariance.len()).map(|k| s.rms_von_mises(k)).collect(),
    }
}

fn plate_spectra() -> Vec<Spectrum> {
    vec![Spectrum::flat(20.0, 1500.0, 1.0).unwrap(); 9]
}

fn plate_cross(p: &Plate, length: f64) -> Excitation {
    let gamma = exponential_coherence(&p.positions, length).unwrap();
    Excitation::cross(
        p.patterns.clone(),
        CrossSpectrum::coherent(plate_spectra(), Coherence::constant(gamma).unwrap()).unwrap(),
    )
    .unwrap()
}

/// `(relative difference of the mean square, maximum over the points of the relative difference of the rms von Mises stress)`.
fn plate_diff(a: &PlateResult, b: &PlateResult) -> (f64, f64) {
    assert!(
        b.vm.iter().all(|&v| v.is_finite() && v > 0.0),
        "reference stress must be positive"
    );
    let vm = a.vm.iter().zip(&b.vm).map(|(x, y)| rel(*x, *y)).fold(0.0, f64::max);
    (rel(a.m0, b.m0), vm)
}

#[test]
fn c1_to_c5_exponential_coherence_limits_on_a_plate() {
    let mut c = Checks::new();
    let p = plate();
    let side = 0.4;
    let d_min = 0.1;
    let uncorrelated = plate_run(
        &p,
        &Excitation::forces(p.patterns.clone(), plate_spectra(), None).unwrap(),
    );
    let ones = vec![vec![1.0; 9]; 9];
    let full = plate_run(
        &p,
        &Excitation::forces(p.patterns.clone(), plate_spectra(), Some(ones)).unwrap(),
    );
    println!(
        "plate: uncorrelated m0 = {:e}, fully correlated m0 = {:e}",
        uncorrelated.m0, full.m0
    );
    let (d0, v0) = plate_diff(&plate_run(&p, &plate_cross(&p, d_min / 40.0)), &uncorrelated);
    c.at_most("C1 L = d_min/40, mean square against uncorrelated", d0, 1e-8);
    c.at_most("C3 L = d_min/40, von Mises against uncorrelated", v0, 1e-8);
    let (d1, v1) = plate_diff(&plate_run(&p, &plate_cross(&p, 1e10 * side)), &full);
    c.at_most("C2 L = 1e10 side, mean square against fully correlated", d1, 1e-8);
    c.at_most("C3 L = 1e10 side, von Mises against fully correlated", v1, 1e-8);
    // C4
    let (s0, _) = plate_diff(&plate_run(&p, &plate_cross(&p, 1e-6 * side)), &uncorrelated);
    c.at_most("C4 L = 1e-6 side against uncorrelated", s0, 1e-8);
    let (e6, _) = plate_diff(&plate_run(&p, &plate_cross(&p, 1e6 * side)), &full);
    let (e8, _) = plate_diff(&plate_run(&p, &plate_cross(&p, 1e8 * side)), &full);
    println!("C4 recorded: L = 1e6 side, error {e6:e}; L = 1e8 side, error {e8:e}");
    c.at_least("C4 error(1e6 side)/error(1e8 side) at least 90", e6 / e8, 90.0);
    c.at_most("C4 error(1e6 side)/error(1e8 side) at most 110", e6 / e8, 110.0);
    // N3
    let mid = plate_run(&p, &plate_cross(&p, d_min));
    c.at_least(
        "N3 L = d_min differs from uncorrelated",
        rel(mid.m0, uncorrelated.m0),
        1e-3,
    );
    c.at_least("N3 L = d_min differs from fully correlated", rel(mid.m0, full.m0), 1e-3);
    // C5
    let zero = vec![vec![0.0; 9]; 9];
    for l in [d_min / 40.0, d_min, 1e-6 * side, 1e6 * side, 1e10 * side] {
        let g = exponential_coherence(&p.positions, l).unwrap();
        c.holds(
            &format!("C5 L = {l:e} accepted by the Hermitian PSD check"),
            check_cross_matrix(1.0, &g, &zero).is_ok() && Coherence::constant(g).is_ok(),
            "",
        );
    }
    c.finish();
}
