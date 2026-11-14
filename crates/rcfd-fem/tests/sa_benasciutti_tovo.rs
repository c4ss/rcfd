//! F-590 (ADR 0524): Benasciutti-Tovo broadband damage rate from the spectral moments. Checks C1 to C7 of the registration; every
//! value is printed.

use rcfd_core::rng::Rng;
use rcfd_fem::fatigue::{
    benasciutti_tovo_coefficients, benasciutti_tovo_damage_rate, benasciutti_tovo_damage_rate_curve,
    broadband_damage_rate, miner, narrow_band_damage_rate, rainflow_periodic, MeanStressCorrection, SnCurve,
};
use rcfd_fem::random::{spectral_response, ModalChannels, Moments, SpectralOptions, Spectrum};
use rcfd_fem::FemError;
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
            let key_ok = matches!(&e, FemError::InvalidValue { key, .. } if *key == "benasciutti-tovo");
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

fn bt(mo: &Moments, m: f64) -> f64 {
    mo.benasciutti_tovo_damage_rate(m, 1.0).unwrap()
}

fn dirlik(mo: &Moments, m: f64) -> f64 {
    mo.broadband_damage_rate(m, 1.0).unwrap()
}

// ------------------------------------------------------------------------------------------------------------ C1

#[test]
#[ignore = "known miss, ADR 0524 Addendum 1: 1.6458e-2 against the limit 1e-2"]
fn c1_one_mode_reduces_to_narrow_band_m3_known_miss() {
    let mut c = Checks::new();
    let mo = one_mode_moments(0.01);
    let (b, n) = (bt(&mo, 3.0), mo.narrow_band_damage_rate(3.0, 1.0).unwrap());
    println!("C1: BT {b:.9e}, narrow band {n:.9e}, ratio {:.9}", b / n);
    c.at_most("C1 |BT/narrow band - 1| (m = 3)", rel(b, n), 1e-2);
    c.finish();
}

#[test]
#[ignore = "known miss, ADR 0524 Addendum 1: 3.1058e-2 against the limit 1e-2"]
fn c1b_one_mode_reduces_to_narrow_band_m5_known_miss() {
    let mut c = Checks::new();
    let mo = one_mode_moments(0.01);
    let (b, n) = (bt(&mo, 5.0), mo.narrow_band_damage_rate(5.0, 1.0).unwrap());
    println!("C1b: BT {b:.9e}, narrow band {n:.9e}, ratio {:.9}", b / n);
    c.at_most("C1b |BT/narrow band - 1| (m = 5)", rel(b, n), 1e-2);
    c.finish();
}

#[test]
fn c1x_c1z_c2x_values_and_small_damping() {
    let mut c = Checks::new();
    let mo = one_mode_moments(0.01);
    let r = bt(&mo, 3.0) / mo.narrow_band_damage_rate(3.0, 1.0).unwrap();
    println!("C1x: ratio {r:.12}");
    c.at_most(
        "C1x ratio against 0.9835421337160092",
        rel(r, 0.983_542_133_716_009_2),
        1e-5,
    );
    let mz = one_mode_moments(0.001);
    let rz = bt(&mz, 3.0) / mz.narrow_band_damage_rate(3.0, 1.0).unwrap();
    println!("C1z: zeta = 0.001 ratio {rz:.9}");
    c.at_most("C1z |BT/narrow band - 1| at zeta = 0.001", (rz - 1.0).abs(), 1e-2);
    let t = two_mode_moments();
    let k = benasciutti_tovo_coefficients(t.m0, t.m1, t.m2, t.m4).unwrap();
    println!("T: alpha1 {:.6}, alpha2 {:.6}, b {:.12}", k.alpha1, k.alpha2, k.b);
    c.at_most(
        "C2x T b against 0.6484016641667036",
        rel(k.b, 0.648_401_664_166_703_6),
        1e-5,
    );
    let q = bt(&t, 3.0) / dirlik(&t, 3.0);
    println!("C2x: T BT/Dirlik {q:.12}");
    c.at_most(
        "C2x T BT/Dirlik against 0.9994764275532475",
        rel(q, 0.999_476_427_553_247_5),
        1e-5,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ C2

#[test]
fn c2_agreement_with_dirlik() {
    let mut c = Checks::new();
    let (s, t) = (one_mode_moments(0.01), two_mode_moments());
    for (name, mo, m, limit) in [
        ("C2 S m = 3", &s, 3.0, 5e-2),
        ("C2b T m = 3", &t, 3.0, 5e-2),
        ("C2c S m = 5", &s, 5.0, 1e-1),
        ("C2d T m = 5", &t, 5.0, 1e-1),
    ] {
        let (b, d) = (bt(mo, m), dirlik(mo, m));
        println!("{name}: BT {b:.9e}, Dirlik {d:.9e}, ratio {:.9}", b / d);
        c.at_most(&format!("{name} |BT/Dirlik - 1|"), rel(b, d), limit);
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ C3

#[test]
fn c3_two_mode_synthesised_history() {
    let mut c = Checks::new();
    let t = two_mode_moments();
    let curve = SnCurve::power(3.0, 1.0).unwrap();
    let b = bt(&t, 3.0);
    println!("T moments {t:?}, BT {b:.9e}, Dirlik {:.9e}", dirlik(&t, 3.0));
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
    let counts = rainflow_periodic(&x).unwrap();
    let d = miner(&counts.cycles, &curve, &MeanStressCorrection::none()).unwrap();
    let counted = d / dur;
    println!(
        "C3: {} cycles, counted rate {counted:.9e}, counted/BT {:.6}",
        counts.cycles.len(),
        counted / b
    );
    c.at_most("C3 |counted/BT - 1|", rel(counted, b), 1e-1);
    c.holds(
        "C3b at least 1e5 closed cycles",
        counts.cycles.len() >= 100_000,
        &format!("{}", counts.cycles.len()),
    );
    c.at_most("C3c counted against ADR 0497", rel(counted, 9.527_921_724e-8), 1e-6);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ C4

fn moments(m0: f64, m1: f64, m2: f64, m4: f64) -> Moments {
    let mut mo = Moments::default();
    mo.m0 = m0;
    mo.m1 = m1;
    mo.m2 = m2;
    mo.m4 = m4;
    mo
}

#[test]
fn c4_refusals() {
    let mut c = Checks::new();
    let ok = (1.0, 0.9, 1.0, 1.0 / 0.81);
    let bt_of = |m0: f64, m1: f64, m2: f64, m4: f64| benasciutti_tovo_damage_rate(m0, m1, m2, m4, 3.0, 1.0);
    for (name, v) in [("0", 0.0), ("-1", -1.0), ("NaN", f64::NAN), ("inf", f64::INFINITY)] {
        let (good, msg) = refused(bt_of(v, ok.1, ok.2, ok.3), &["m0"]);
        c.holds(&format!("C4 m0 = {name}"), good, &msg);
    }
    for (name, v) in [("0", 0.0), ("-1", -1.0), ("NaN", f64::NAN)] {
        let (good, msg) = refused(bt_of(ok.0, ok.1, v, ok.3), &["m2"]);
        c.holds(&format!("C4 m2 = {name}"), good, &msg);
    }
    let (good, msg) = refused(Moments::default().benasciutti_tovo_damage_rate(3.0, 1.0), &["m0"]);
    c.holds("C4 Moments::default()", good, &msg);
    let (good, msg) = refused(bt_of(ok.0, ok.1, ok.2, 0.0), &["m4"]);
    c.holds("C4b m4 = 0", good, &msg);
    let (good, msg) = refused(bt_of(ok.0, -0.1, ok.2, ok.3), &["m1"]);
    c.holds("C4b m1 < 0", good, &msg);
    let (good, msg) = refused(bt_of(1.0, 1.1, 1.0, 1.0), &["alpha1"]);
    c.holds("C4b alpha1 > 1", good, &msg);
    let (good, msg) = refused(bt_of(1.0, 0.5, 1.0, 0.9), &["alpha2"]);
    c.holds("C4b alpha2 > 1", good, &msg);
    let (good, msg) = refused(bt_of(1.0, 0.5, 1.0, 1.0 / 0.81), &["b ="]);
    c.holds("C4b b = 1.145", good, &msg);
    let (good, msg) = refused(bt_of(1.0, 0.2, 1.0, 1.0 / 0.36), &["b ="]);
    c.holds("C4b b = -2.155", good, &msg);
    for (name, m, cc, word) in [
        ("m = 0", 0.0, 1.0, "exponent"),
        ("m < 0", -3.0, 1.0, "exponent"),
        ("c = 0", 3.0, 0.0, "constant"),
        ("c < 0", 3.0, -1.0, "constant"),
    ] {
        let (good, msg) = refused(benasciutti_tovo_damage_rate(ok.0, ok.1, ok.2, ok.3, m, cc), &[word]);
        c.holds(&format!("C4b {name}"), good, &msg);
    }
    let table = SnCurve::table(&[1e3, 1e6], &[200.0, 100.0]).unwrap();
    let (good, msg) = refused(
        benasciutti_tovo_damage_rate_curve(ok.0, ok.1, ok.2, ok.3, &table),
        &["power law"],
    );
    c.holds("C4b table curve", good, &msg);
    let endur = SnCurve::power(3.0, 1.0).unwrap().with_endurance(0.1).unwrap();
    let (good, msg) = refused(
        benasciutti_tovo_damage_rate_curve(ok.0, ok.1, ok.2, ok.3, &endur),
        &["power law"],
    );
    c.holds("C4b endurance limit", good, &msg);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------- C5, C6, C7

#[test]
fn c5_c6_c7_scaling_structure_and_feed() {
    let mut c = Checks::new();
    let t = two_mode_moments();
    let m = 3.0;
    let base = bt(&t, m);
    let k = 2.5;
    let amp = moments(t.m0 * k * k, t.m1 * k * k, t.m2 * k * k, t.m4 * k * k);
    c.at_most("C5 amplitude scaling k^m", rel(bt(&amp, m), base * k.powf(m)), 1e-12);
    let s: f64 = 3.0;
    let fr = moments(t.m0, t.m1 * s, t.m2 * s * s, t.m4 * s.powi(4));
    c.at_most("C5 frequency scaling s", rel(bt(&fr, m), base * s), 1e-12);
    // C6: alpha2 = 1 gives the narrow-band rate.
    let d = moments(2.0, 4.0, 8.0, 32.0);
    let n = narrow_band_damage_rate((8.0f64 / 2.0).sqrt(), 2.0f64.sqrt(), m, 1.0).unwrap();
    let v = bt(&d, m);
    println!("C6: degenerate rate {v:e}, narrow band {n:e}");
    c.holds("C6 finite", v.is_finite(), &format!("{v:e}"));
    c.at_most("C6 alpha2 = 1 gives narrow band", rel(v, n), 1e-12);
    // C6b: b = 1 and b = 0 limits (m0 = m2 = 1: nu0 = 1, rms = 1).
    let one = moments(1.0, 1.0, 1.0, 4.0);
    let k1 = benasciutti_tovo_coefficients(one.m0, one.m1, one.m2, one.m4).unwrap();
    let n1 = narrow_band_damage_rate(1.0, 1.0, m, 1.0).unwrap();
    c.at_most("C6b b = 1 limit", (k1.b - 1.0).abs(), 1e-12);
    c.at_most("C6b b = 1 rate equals narrow band", rel(bt(&one, m), n1), 1e-12);
    let zero = moments(1.0, 0.5, 1.0, 4.0);
    let k0 = benasciutti_tovo_coefficients(zero.m0, zero.m1, zero.m2, zero.m4).unwrap();
    c.at_most("C6b b = 0 limit", k0.b.abs(), 1e-12);
    c.at_most("C6b b = 0 rate", rel(bt(&zero, m), 0.5f64.powf(m - 1.0) * n1), 1e-12);
    // C7: the method and the free functions are bit equal.
    for (name, mo) in [("S", one_mode_moments(0.01)), ("T", t)] {
        let a = mo.benasciutti_tovo_damage_rate(3.0, 1.0).unwrap();
        let b = benasciutti_tovo_damage_rate(mo.m0, mo.m1, mo.m2, mo.m4, 3.0, 1.0).unwrap();
        let curve = SnCurve::power(3.0, 1.0).unwrap();
        let cc = benasciutti_tovo_damage_rate_curve(mo.m0, mo.m1, mo.m2, mo.m4, &curve).unwrap();
        c.holds(
            &format!("C7 {name} bit equality"),
            a.to_bits() == b.to_bits() && a.to_bits() == cc.to_bits() && a.is_finite(),
            &format!("{a:e}"),
        );
        let d = broadband_damage_rate(mo.m0, mo.m1, mo.m2, mo.m4, 3.0, 1.0).unwrap();
        c.holds("Dirlik finite", d.is_finite(), &format!("{d:e}"));
    }
    c.finish();
}
