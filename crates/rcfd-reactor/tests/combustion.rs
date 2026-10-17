//! ADR 0053 Part 2 (F-129): mixture-fraction tables, beta PDF, Burke-Schumann, steady flamelets and the premixed progress variable.
//! The limits are those registered in the ADR before any of these checks was run. No comparison with a published flame is attempted.

use rcfd_reactor::equil::{equilibrium, Constraint};
use rcfd_reactor::flamelet::*;
use rcfd_reactor::mech::{Mechanism, P_STD, R_GAS};
use rcfd_reactor::mixfrac::*;
use rcfd_reactor::progress::*;
use rcfd_reactor::reactors::*;
use rcfd_reactor::toy;
use std::f64::consts::PI;
use std::path::PathBuf;

fn gri() -> Option<Mechanism> {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../validation-data/data/gri-mech-30");
    let m = std::fs::read_to_string(d.join("berkeley-grimech30.dat")).ok()?;
    let t = std::fs::read_to_string(d.join("berkeley-thermo30.dat")).ok()?;
    Some(Mechanism::from_chemkin(&m, Some(&t)).unwrap_or_else(|e| panic!("GRI-Mech read failed: {e}")))
}

// ------------------------------------------------------------------------------------------------ the Burke-Schumann toy

const Q: f64 = 1.2e7; // J per kg of fuel
const W_F: f64 = 0.024;
const W_O: f64 = 0.032;
const W_P: f64 = 0.028;

/// F = FU (X2, 24 g/mol), O = OX (Y2, 32), P = PR (XY, 28), inert IN (Z2, 28); `FU + OX -> 2 PR` with constant mass `cp = 1000`.
fn bs_mech(rate_line: &str) -> Mechanism {
    let a1 = |w: f64| 1000.0 * w / R_GAS;
    let h0_p = -Q * W_F / (2.0 * R_GAS);
    let mech = format!("ELEMENTS X/12.0/ Y/16.0/ Z/14.0/ END\nSPECIES FU OX PR IN END\nREACTIONS\n{rate_line}\nEND\n");
    let e = vec![
        ("FU", vec![("X", 2.0)], a1(W_F), 0.0, 4.0),
        ("OX", vec![("Y", 2.0)], a1(W_O), 0.0, 4.0),
        ("PR", vec![("X", 1.0), ("Y", 1.0)], a1(W_P), h0_p, 4.0 + 15.0),
        ("IN", vec![("Z", 2.0)], a1(W_P), 0.0, 4.0),
    ];
    Mechanism::from_chemkin(&mech, Some(&toy::thermo_text(&e))).expect("BS toy mechanism")
}

fn bs_streams(m: &Mechanism) -> (Stream, Stream) {
    let fuel = Stream::from_moles(m, &[("FU", 1.0)], 300.0).unwrap();
    // oxidiser: Y_O = 0.25, Y_I = 0.75 by mass
    let ox = Stream::from_moles(m, &[("OX", 0.25 / W_O), ("IN", 0.75 / W_P)], 300.0).unwrap();
    (fuel, ox)
}

fn bs_closed() -> BurkeSchumann {
    BurkeSchumann {
        s: W_O / W_F,
        yf1: 1.0,
        yo2: 0.25,
        cp: 1000.0,
        q: Q,
        t_o: 300.0,
        t_f: 300.0,
    }
}

fn zgrid_with(n: usize, extra: f64) -> Vec<f64> {
    let mut z: Vec<f64> = (0..=n).map(|i| i as f64 / n as f64).collect();
    if !z.iter().any(|v| (v - extra).abs() < 1e-14) {
        z.push(extra);
        z.sort_by(|a, b| a.partial_cmp(b).unwrap());
    }
    z
}

// ------------------------------------------------------------------------------------------------ B1 .. B7

#[test]
fn b1_burke_schumann_versus_equilibrium() {
    let m = bs_mech("FU + OX <=> 2PR  1.0E9 0.0 0.0");
    let (fuel, ox) = bs_streams(&m);
    let bs = bs_closed();
    let zst = bs.z_st();
    assert!((zst - 3.0 / 19.0).abs() < 1e-14);
    let z = zgrid_with(300, zst);
    let tab = equilibrium_table(&m, &fuel, &ox, P_STD, &z).unwrap();
    let mut dt = 0.0_f64;
    let mut dy = 0.0_f64;
    for (i, &zz) in z.iter().enumerate() {
        dt = dt.max((tab.t[i] - bs.t(zz)).abs());
        let yb = [bs.y_f(zz), bs.y_o(zz), bs.y_p(zz), 0.75 * (1.0 - zz)];
        for k in 0..4 {
            dy = dy.max((tab.y[k][i] - yb[k]).abs());
        }
    }
    let ist = z.iter().position(|v| (v - zst).abs() < 1e-14).unwrap();
    let tst_err = (tab.t[ist] - 2194.7368421).abs();
    // piecewise linear with one kink: slope differences across interior nodes
    let mut worst_curv = 0.0_f64;
    for i in 1..z.len() - 1 {
        if (i as isize - ist as isize).abs() <= 1 {
            continue;
        }
        let sl = (tab.t[i] - tab.t[i - 1]) / (z[i] - z[i - 1]);
        let sr = (tab.t[i + 1] - tab.t[i]) / (z[i + 1] - z[i]);
        worst_curv = worst_curv.max((sr - sl).abs() * 0.5 * (z[i + 1] - z[i - 1]));
    }
    println!(
        "B1 {} nodes: max|T_eq - T_BS| {dt:.3e} K (limit 1e-3); max|Y_eq - Y_BS| {dy:.3e} (1e-8); T_st {:.7} (2194.7368 +- 1e-3, err {tst_err:.2e}); max slope-jump*h away from the kink {worst_curv:.3e} K (1e-3)",
        z.len(),
        tab.t[ist]
    );
    assert!(dt <= 1e-3);
    assert!(dy <= 1e-8);
    assert!(tst_err <= 1e-3);
    assert!(worst_curv <= 1e-3);
}

fn linspace(n: usize) -> Vec<f64> {
    (0..=n).map(|i| i as f64 / n as f64).collect()
}

#[test]
fn b2_beta_pdf() {
    // (a) linear function and (b) normalisation
    let z = linspace(100);
    let lin: Vec<f64> = z.iter().map(|&x| 2.0 + 3.0 * x).collect();
    let one = vec![1.0; z.len()];
    let pairs = [
        (0.5, 0.05),
        (0.3, 0.02),
        (0.1, 0.01),
        (0.9, 0.05),
        (0.02, 0.005),
        (0.98, 0.005),
        (0.5, 0.2),
        (0.2, 0.15),
        (0.7, 0.1),
        (0.05, 0.04),
        (0.95, 0.04),
        (0.4, 0.01),
    ];
    let mut wl = 0.0_f64;
    let mut wn = 0.0_f64;
    let mut ushape = 0;
    for &(zm, v) in &pairs {
        let (a, b) = beta_params(zm, v);
        if a < 1.0 || b < 1.0 {
            ushape += 1;
        }
        let r = integrate_beta(&z, &lin, zm, v).unwrap();
        wl = wl.max((r - (2.0 + 3.0 * zm)).abs() / (2.0 + 3.0 * zm));
        wn = wn.max((integrate_beta(&z, &one, zm, v).unwrap() - 1.0).abs());
    }
    println!("B2a linear function worst relative error {wl:.3e} (1e-12), {ushape} U-shaped pairs; B2b normalisation {wn:.3e} (1e-12)");
    assert!(ushape >= 2);
    assert!(wl <= 1e-12);
    assert!(wn <= 1e-12);

    // BS table on a grid containing the kink
    let bs = bs_closed();
    let zst = bs.z_st();
    let zb = zgrid_with(400, zst);
    let tb: Vec<f64> = zb.iter().map(|&x| bs.t(x)).collect();
    let tmin = tb.iter().cloned().fold(f64::INFINITY, f64::min);
    let tmax = tb.iter().cloned().fold(0.0, f64::max);
    let span = tmax - tmin;
    // (c) delta limits
    let zm = 0.6;
    let d0 = integrate_beta(&zb, &tb, zm, 0.0).unwrap();
    assert_eq!(d0.to_bits(), interp(&zb, &tb, zm).to_bits());
    let zm2 = zst + 0.1;
    let d1 = integrate_beta(&zb, &tb, zm2, 1e-8).unwrap();
    let e1 = (d1 - interp(&zb, &tb, zm2)).abs();
    let zm3 = zst - 0.07;
    let e1b = (integrate_beta(&zb, &tb, zm3, 1e-8).unwrap() - interp(&zb, &tb, zm3)).abs();
    println!(
        "B2c v = 0 bitwise; v = 1e-8 difference {e1:.3e} K / {e1b:.3e} K (limit {:.3e})",
        1e-6 * span
    );
    assert!(e1 <= 1e-6 * span && e1b <= 1e-6 * span);
    // (d) maximum variance
    let mut ed = 0.0_f64;
    for &zm in &[0.2, 0.5, 0.8] {
        let v = (1.0 - 1e-6) * zm * (1.0 - zm);
        let r = integrate_beta(&zb, &tb, zm, v).unwrap();
        let ex = (1.0 - zm) * tb[0] + zm * tb[tb.len() - 1];
        ed = ed.max((r - ex).abs());
    }
    println!(
        "B2d maximum-variance limit worst difference {ed:.3e} K (limit {:.3e})",
        1e-4 * span
    );
    assert!(ed <= 1e-4 * span);
    // (e) independent quadrature, split at the kink: 2 x 100 panels of 5-point Gauss-Legendre
    let (zm, v) = (0.3, 0.02);
    let (a, b) = beta_params(zm, v);
    assert!((a - 2.85).abs() < 1e-12 && (b - 6.65).abs() < 1e-12);
    let lb = ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b);
    let pdf = |x: f64| (lb + (a - 1.0) * x.ln() + (b - 1.0) * (1.0 - x).ln()).exp();
    let gl_x = [
        0.0,
        0.5384693101056831,
        -0.5384693101056831,
        0.906_179_845_938_664,
        -0.906_179_845_938_664,
    ];
    let gl_w = [
        0.5688888888888889,
        0.4786286704993665,
        0.4786286704993665,
        0.2369268850561891,
        0.2369268850561891,
    ];
    let quad = |lo: f64, hi: f64, panels: usize, f: &dyn Fn(f64) -> f64| -> f64 {
        let h = (hi - lo) / panels as f64;
        let mut s = 0.0;
        for p in 0..panels {
            let c = lo + (p as f64 + 0.5) * h;
            for q in 0..5 {
                s += gl_w[q] * 0.5 * h * f(c + 0.5 * h * gl_x[q]);
            }
        }
        s
    };
    let tq = quad(0.0, zst, 100, &|x| bs.t(x) * pdf(x)) + quad(zst, 1.0, 100, &|x| bs.t(x) * pdf(x));
    let nq = quad(0.0, zst, 100, &|x| pdf(x)) + quad(zst, 1.0, 100, &|x| pdf(x));
    let tb_mean = integrate_beta(&zb, &tb, zm, v).unwrap();
    // the piecewise-linear table of the BS curve (kink on a node) is exact, so the two integrals agree
    let rel = (tb_mean - tq).abs() / tq;
    println!(
        "B2e beta mean {tb_mean:.9} quadrature {tq:.9} relative difference {rel:.3e} (1e-8); quadrature norm {nq:.12}"
    );
    assert!(rel <= 1e-8);
    // (f) mean and variance via a fine table
    let zf = linspace(4000);
    let zz = zf.clone();
    let z2: Vec<f64> = zf.iter().map(|x| x * x).collect();
    let mean = integrate_beta(&zf, &zz, zm, v).unwrap();
    let m2 = integrate_beta(&zf, &z2, zm, v).unwrap();
    println!(
        "B2f mean {mean:.15} (zm {zm}) diff {:.2e} (1e-12); <Z^2> {m2:.10} vs zm^2+v {:.10} diff {:.2e} (1e-6)",
        (mean - zm).abs(),
        zm * zm + v,
        (m2 - zm * zm - v).abs()
    );
    assert!((mean - zm).abs() <= 1e-12);
    assert!((m2 - (zm * zm + v)).abs() <= 1e-6);
}

#[test]
fn b3_special_functions() {
    let rel = |a: f64, b: f64| (a - b).abs() / b.abs();
    let mut w = 0.0_f64;
    w = w.max(rel(ln_gamma(5.0), 24.0_f64.ln()));
    w = w.max(rel(ln_gamma(0.5).exp(), PI.sqrt()));
    // Gamma(21/2) = 19!!/2^10 sqrt(pi) = 654729075 sqrt(pi)/1024 (the ADR text printed 945 sqrt(pi)/1024, a transcription slip: that is Gamma(11/2)/32)
    w = w.max(rel(ln_gamma(10.5).exp(), 654729075.0 * PI.sqrt() / 1024.0));
    println!("B3 ln Gamma worst relative error {w:.3e} (1e-13)");
    assert!(w <= 1e-13);
    let mut wi = 0.0_f64;
    for &x in &[0.05, 0.3, 0.5, 0.77, 0.95] {
        wi = wi.max((reg_inc_beta(x, 1.0, 1.0) - x).abs());
        wi = wi.max((reg_inc_beta(x, 2.0, 1.0) - x * x).abs());
        wi = wi.max((reg_inc_beta(x, 1.0, 3.0) - (1.0 - (1.0 - x).powi(3))).abs());
        wi = wi.max((reg_inc_beta(x, 0.5, 0.5) - 2.0 / PI * x.sqrt().asin()).abs());
        for &(a, b) in &[(2.3, 4.1), (0.7, 5.5), (12.0, 3.0), (0.2, 0.4)] {
            wi = wi.max((reg_inc_beta(x, a, b) - (1.0 - reg_inc_beta(1.0 - x, b, a))).abs());
        }
    }
    println!("B3 incomplete beta worst absolute error {wi:.3e} (1e-12)");
    assert!(wi <= 1e-12);
    let e1 = rel(erf(1.0), 0.8427007929497149);
    let mut we = 0.0_f64;
    for &y in &[1e-8, 1e-4, 0.01, 0.1, 0.4, 0.9, 1.0, 1.3, 1.9, 1.999] {
        let x = erfc_inv(y);
        we = we.max(rel(erfc(x), y));
    }
    println!("B3 erf(1) relative error {e1:.3e} (1e-12); erfc round trip worst {we:.3e} (1e-12)");
    assert!(e1 <= 1e-12);
    assert!(we <= 1e-12);
}

/// Element-mole error of every table row against `Z e_F + (1 - Z) e_O`, relative to the largest element value of the streams.
fn element_check(m: &Mechanism, tab: &MixtureTable, fuel: &Stream, ox: &Stream) -> (f64, f64) {
    let ef = m.element_mass_fractions(&fuel.y);
    let eo = m.element_mass_fractions(&ox.y);
    let scale = ef.iter().chain(&eo).cloned().fold(0.0, f64::max);
    let mut we = 0.0_f64;
    let mut ws = 0.0_f64;
    for (i, &z) in tab.z.iter().enumerate() {
        let y = tab.y_at(i);
        let e = m.element_mass_fractions(&y);
        for k in 0..e.len() {
            we = we.max((e[k] - (z * ef[k] + (1.0 - z) * eo[k])).abs() / scale);
        }
        ws = ws.max((y.iter().sum::<f64>() - 1.0).abs());
    }
    (we, ws)
}

#[test]
fn b4_element_conservation_of_the_table() {
    let m = bs_mech("FU + OX <=> 2PR  1.0E9 0.0 0.0");
    let (fuel, ox) = bs_streams(&m);
    let z = zgrid_with(100, 3.0 / 19.0);
    let tab = equilibrium_table(&m, &fuel, &ox, P_STD, &z).unwrap();
    let (we, ws) = element_check(&m, &tab, &fuel, &ox);
    let n = z.len();
    println!(
        "B4 toy: element error {we:.3e} (1e-12 relative to the largest stream element value), sum Y - 1 {ws:.3e} (1e-12); T(0) {} T(1) {}",
        tab.t[0],
        tab.t[n - 1]
    );
    assert!(we <= 1e-12);
    assert!(ws <= 1e-12);
    assert!((tab.t[0] - ox.t).abs() <= 1e-6 && (tab.t[n - 1] - fuel.t).abs() <= 1e-6);
    if let Some(g) = gri() {
        let fuel = Stream::from_moles(&g, &[("CH4", 1.0)], 300.0).unwrap();
        let ox = Stream::from_moles(&g, &[("O2", 1.0), ("N2", 3.76)], 300.0).unwrap();
        let zst = stoichiometric_z(&g, &fuel, &ox).unwrap();
        let z = zgrid_with(40, zst);
        let tab = equilibrium_table(&g, &fuel, &ox, P_STD, &z).unwrap();
        let (we, ws) = element_check(&g, &tab, &fuel, &ox);
        println!("B4 GRI CH4/air: element error {we:.3e} (1e-9), sum Y - 1 {ws:.3e} (1e-9)");
        assert!(we <= 1e-9);
        assert!(ws <= 1e-9);
    } else {
        println!("B4 GRI part skipped: GRI-Mech data not fetched");
    }
}

#[test]
fn b5_interpolation_order() {
    let f = |x: f64| x * x;
    let mut errs = Vec::new();
    for &n in &[20usize, 40] {
        let z = linspace(n);
        let t: Vec<f64> = z.iter().map(|&x| f(x)).collect();
        let h = 1.0 / n as f64;
        let mut emax = 0.0_f64;
        let mut dev = 0.0_f64;
        for i in 0..n {
            let mid = 0.5 * (z[i] + z[i + 1]);
            let e = (interp(&z, &t, mid) - f(mid)).abs();
            dev = dev.max((e - h * h / 4.0).abs());
            emax = emax.max(e);
        }
        println!("B5 n {n}: max midpoint error {emax:.6e}, deviation from h^2/4 {dev:.3e} (1e-12)");
        assert!(dev <= 1e-12);
        errs.push(emax);
    }
    let ratio = errs[0] / errs[1];
    println!("B5 error ratio {ratio:.6} (within [3.9, 4.1])");
    assert!((3.9..=4.1).contains(&ratio));
    let z = linspace(13);
    let t: Vec<f64> = z.iter().map(|&x| 1.5 - 2.0 * x).collect();
    let mut w = 0.0_f64;
    for i in 0..500 {
        let x = i as f64 / 499.0;
        w = w.max((interp(&z, &t, x) - (1.5 - 2.0 * x)).abs());
    }
    println!("B5 linear function reproduced to {w:.3e} (1e-13)");
    assert!(w <= 1e-13);
}

#[test]
fn b6_located_errors() {
    let z = linspace(10);
    let f = vec![1.0; 11];
    let e = |r: Result<f64, MixError>| r.unwrap_err().to_string();
    assert!(
        e(integrate_beta(&z, &f, 1.5, 0.01)).contains("zm"),
        "{}",
        e(integrate_beta(&z, &f, 1.5, 0.01))
    );
    assert!(e(integrate_beta(&z, &f, -0.1, 0.01)).contains("zm"));
    assert!(e(integrate_beta(&z, &f, 0.5, -1e-3)).contains("variance v"));
    let msg = e(integrate_beta(&z, &f, 0.5, 0.3));
    assert!(msg.contains("variance v") && msg.contains("3e-1"), "{msg}");
    let mut bad = z.clone();
    bad[4] = bad[3];
    let msg = e(integrate_beta(&bad, &f, 0.5, 0.01));
    assert!(msg.contains("z grid") && msg.contains("index 4"), "{msg}");
    let msg = e(integrate_beta(&z, &f[..9], 0.5, 0.01));
    assert!(msg.contains("table length mismatch"), "{msg}");
    let m = bs_mech("FU + OX <=> 2PR  1.0E9 0.0 0.0");
    let msg = Stream::from_moles(&m, &[("NOPE", 1.0)], 300.0).unwrap_err().to_string();
    assert!(msg.contains("NOPE"), "{msg}");
    let msg = Stream::from_moles(&m, &[("FU", 0.0)], 300.0).unwrap_err().to_string();
    assert!(msg.contains("sums to zero"), "{msg}");
    let msg = MixtureTable::new(z.clone(), vec![0.0; 11], vec![vec![0.0; 9]])
        .unwrap_err()
        .to_string();
    assert!(msg.contains("row count mismatch"), "{msg}");
    println!("B6 all seven located error cases return text naming the argument");
}

#[test]
fn b7_ch4_air_equilibrium_table_report() {
    let Some(g) = gri() else {
        return println!("B7 skipped: GRI-Mech data not fetched");
    };
    let fuel = Stream::from_moles(&g, &[("CH4", 1.0)], 300.0).unwrap();
    let ox = Stream::from_moles(&g, &[("O2", 1.0), ("N2", 3.76)], 300.0).unwrap();
    let zst = stoichiometric_z(&g, &fuel, &ox).unwrap();
    // probe the grid: report the nodes at which the library equilibrium could not be computed
    let mut z = vec![0.0];
    let mut failed = Vec::new();
    for zz in zgrid_with(200, zst).into_iter().skip(1) {
        let one = [0.0, zz];
        if equilibrium_table(&g, &fuel, &ox, P_STD, &one).is_ok() || zz == 1.0 {
            z.push(zz);
        } else {
            failed.push(zz);
        }
    }
    println!("B7 nodes without a converged equilibrium (omitted from the table): {failed:?}");
    let tab = equilibrium_table(&g, &fuel, &ox, P_STD, &z).unwrap();
    let imax = (0..z.len())
        .max_by(|&a, &b| tab.t[a].partial_cmp(&tab.t[b]).unwrap())
        .unwrap();
    let t_lean = tab.lookup(0.5 * zst).0;
    let t_rich = tab.lookup(2.0 * zst).0;
    let ist = z.iter().position(|v| (v - zst).abs() < 1e-14).unwrap();
    println!(
        "B7 CH4/air (GRI-Mech 3.0 thermodynamics, 300 K, 1 atm): Z_st {zst:.6} (element balance); T_max {:.1} K at Z {:.5}; T(Z_st) {:.1} K; T(Z_st/2) {t_lean:.1} K; T(2 Z_st) {t_rich:.1} K (report only)",
        tab.t[imax], z[imax], tab.t[ist]
    );
    // report only, but a sanity guard on the element balance value: 1 / (1 + 17.2) for methane / air by mass is about 0.055
    assert!((zst - 0.055).abs() < 0.002);
}

// ------------------------------------------------------------------------------------------------ flamelets

#[test]
fn f5_chi_profile() {
    let zst = 3.0 / 19.0;
    let c = chi_profile(zst, 7.5, zst);
    let mut sym = 0.0_f64;
    let mut peak_ok = true;
    let cmax = chi_profile(0.5, 1.0, zst);
    for i in 1..100 {
        let z = i as f64 / 100.0;
        sym = sym.max((chi_profile(z, 2.0, zst) - chi_profile(1.0 - z, 2.0, zst)).abs());
        if chi_profile(z, 1.0, zst) > cmax {
            peak_ok = false;
        }
    }
    println!("F5 chi(Z_st) {c} (7.5 exact); symmetry error {sym:.3e} (1e-12); maximum at 0.5: {peak_ok}");
    assert!((c - 7.5).abs() <= 1e-12 * 7.5);
    assert!(sym <= 1e-12);
    assert!(peak_ok);
    assert_eq!(chi_profile(0.0, 1.0, zst), 0.0);
    assert_eq!(chi_profile(1.0, 1.0, zst), 0.0);
}

fn perturbed_mixing_line(fuel: &Stream, ox: &Stream, z: &[f64]) -> Vec<Vec<f64>> {
    z.iter()
        .enumerate()
        .map(|(i, &zz)| {
            let bump = 0.02 * (PI * zz).sin() * (1.0 + 0.3 * ((7 * i) % 5) as f64);
            fuel.y
                .iter()
                .zip(&ox.y)
                .enumerate()
                .map(|(k, (f, o))| {
                    (zz * f + (1.0 - zz) * o)
                        + if k == 0 {
                            bump
                        } else {
                            -bump / (fuel.y.len() - 1) as f64
                        }
                })
                .collect()
        })
        .collect()
}

#[test]
fn f1_flamelet_without_reaction_is_the_mixing_line() {
    // inert: same streams, no reactive path (the reaction needs both FU and OX, but the fuel stream has no OX and vice versa; set the rate to zero)
    let m = bs_mech("FU + OX => 2PR  0.0 0.0 0.0");
    let (fuel, ox) = bs_streams(&m);
    let z = linspace(100);
    let chi: Vec<f64> = z.iter().map(|&x| chi_profile(x, 1.0, 3.0 / 19.0)).collect();
    let guess = perturbed_mixing_line(&fuel, &ox, &z);
    let sol = solve(
        &m,
        P_STD,
        &fuel,
        &ox,
        &z,
        &chi,
        Some(&guess),
        &FlameletOptions::default(),
    )
    .unwrap();
    let mut w = 0.0_f64;
    for (i, &zz) in z.iter().enumerate() {
        for k in 0..4 {
            w = w.max((sol.y[i][k] - (zz * fuel.y[k] + (1.0 - zz) * ox.y[k])).abs());
        }
    }
    println!(
        "F1 max deviation from the mixing line {w:.3e} (1e-12); Newton iterations {} (<= 2)",
        sol.newton_iters
    );
    assert!(w <= 1e-12);
    assert!(sol.newton_iters <= 2);
}

#[test]
fn f2_flamelet_first_order_decay() {
    // isothermal A -> B with k = m^2 chi / 2, m = 2, constant chi
    let chi0 = 3.0;
    let mm = 2.0_f64;
    let k = mm * mm * chi0 / 2.0;
    let mech = toy::isomer(k, 0.0, 0.0, false);
    let fuel = Stream::from_moles(&mech, &[("A", 1.0)], 500.0).unwrap();
    let ox = Stream::from_moles(&mech, &[("B", 1.0)], 500.0).unwrap();
    let n = 100;
    let z = linspace(n);
    let chi = vec![chi0; n + 1];
    let guess: Vec<Vec<f64>> = z.iter().map(|&x| vec![x, 1.0 - x]).collect();
    let sol = solve(
        &mech,
        P_STD,
        &fuel,
        &ox,
        &z,
        &chi,
        Some(&guess),
        &FlameletOptions::default(),
    )
    .unwrap();
    // exact discrete solution: 2 (cosh(mu h) - 1)/h^2 = m^2
    let h = 1.0 / n as f64;
    let mu = ((1.0 + 0.5 * mm * mm * h * h).acosh()) / h;
    let mut wd = 0.0_f64;
    let mut wc = 0.0_f64;
    for (i, &zz) in z.iter().enumerate() {
        wd = wd.max((sol.y[i][0] - (mu * zz).sinh() / mu.sinh()).abs());
        wc = wc.max((sol.y[i][0] - (mm * zz).sinh() / mm.sinh()).abs());
    }
    println!(
        "F2 vs exact discrete solution {wd:.3e} (1e-9); vs continuous sinh(mZ)/sinh(m) {wc:.3e} (2e-4); iterations {}",
        sol.newton_iters
    );
    assert!(wd <= 1e-9);
    assert!(wc <= 2e-4);
}

fn t_leak(sol: &FlameletSolution, dts: f64, tst: f64) -> f64 {
    (sol.t_max() - tst).abs() / dts
}

#[test]
fn f3_flamelet_fast_chemistry_limit() {
    let m = bs_mech("FU + OX <=> 2PR  1.0E14 0.0 0.0");
    let (fuel, ox) = bs_streams(&m);
    let bs = bs_closed();
    let zst = bs.z_st();
    let z = zgrid_with(800, zst);
    let eq = equilibrium_table(&m, &fuel, &ox, P_STD, &z).unwrap();
    let dts = bs.t_st() - 300.0;
    let mut leaks = Vec::new();
    let mut worst_lin = 0.0_f64;
    let mut worst_res = 0.0_f64;
    for &c in &[1e-1, 1e-2, 1e-3] {
        let chi: Vec<f64> = z.iter().map(|&x| chi_profile(x, c, zst)).collect();
        let guess = guess_from_table(&eq, &z);
        let sol = solve(
            &m,
            P_STD,
            &fuel,
            &ox,
            &z,
            &chi,
            Some(&guess),
            &FlameletOptions::default(),
        )
        .unwrap();
        let leak = t_leak(&sol, dts, bs.t_st());
        // element X mass fraction linear in Z: X is in FU (X2: 24 g/mol -> all mass) and PR (XY: 12/28)
        let mut lin = 0.0_f64;
        for (i, &zz) in z.iter().enumerate() {
            let xm = sol.y[i][0] + sol.y[i][2] * 12.0 / 28.0;
            lin = lin.max((xm - zz).abs());
        }
        println!(
            "F3 chi_st {c:e}: T_max {:.2} (BS peak {:.2}) relative deviation {leak:.4e}; X-element linearity error {lin:.3e} (1e-9); residual {:.2e}; Newton {} transient {}",
            sol.t_max(),
            bs.t_st(),
            sol.residual,
            sol.newton_iters,
            sol.transient_steps
        );
        leaks.push(leak);
        worst_lin = worst_lin.max(lin);
        worst_res = worst_res.max(sol.residual);
    }
    assert!(leaks[0] > leaks[1] && leaks[1] > leaks[2], "not monotone: {leaks:?}");
    assert!(leaks[2] <= 0.1);
    assert!(worst_res <= 1e-8);
    assert!(worst_lin <= 1e-9);
}

/// First F4 set-up: chi_st list with a factor of 4 between values, too coarse for the burning branch (only 2 values burn before the
/// flamelet extinguishes; set-up defect). Kept; superseded by `f4_flamelet_extinction_trend`.
#[test]
#[ignore = "superseded set-up (chi_st list too coarse), kept per ADR 0053 rules"]
fn f4_first_setup_coarse_chi_list() {
    // irreversible one-step Arrhenius, activation temperature 15000 K (29807 cal/mol)
    let m = bs_mech("FU + OX => 2PR  3.0E12 0.0 29807.0");
    let (fuel, ox) = bs_streams(&m);
    let bs = bs_closed();
    let zst = bs.z_st();
    let z = zgrid_with(400, zst);
    let eq = equilibrium_table(&m, &fuel, &ox, P_STD, &z).unwrap();
    let chis: Vec<f64> = (0..14).map(|i| 1.0 * 4.0_f64.powi(i) * 0.25).collect();
    let (tab, sols) = build_table(&m, P_STD, &fuel, &ox, &z, zst, &chis, &eq, &FlameletOptions::default()).unwrap();
    let tm: Vec<f64> = sols.iter().map(|s| s.t_max()).collect();
    for (c, t) in tab.chi_st.iter().zip(&tm) {
        println!("F4 chi_st {c:.4e}: T_max {t:.2}");
    }
    println!(
        "F4 converged {} of {} chi_st values; first non-converging chi_st (extinction): {:?}",
        tab.chi_st.len(),
        chis.len(),
        tab.extinction_chi
    );
    // strictly decreasing over at least 6 values (stop at the first value where the cold solution is reached)
    let mut count = 1;
    for i in 1..tm.len() {
        if tm[i] < tm[i - 1] {
            count += 1;
        } else {
            break;
        }
    }
    println!("F4 strictly decreasing T_max over the first {count} values");
    assert!(count >= 6);
}

/// Irreversible one-step Arrhenius flamelets along an ascending `chi_st` list (burning branch continuation).
fn f4_run(factor: f64, n: usize) -> (FlameletTable, Vec<f64>, f64) {
    let m = bs_mech("FU + OX => 2PR  3.0E12 0.0 29807.0"); // activation temperature 15000 K
    let (fuel, ox) = bs_streams(&m);
    let bs = bs_closed();
    let zst = bs.z_st();
    let z = zgrid_with(400, zst);
    let eq = equilibrium_table(&m, &fuel, &ox, P_STD, &z).unwrap();
    let chis: Vec<f64> = (0..n).map(|i| 0.05 * factor.powi(i as i32)).collect();
    let (tab, sols) = build_table(&m, P_STD, &fuel, &ox, &z, zst, &chis, &eq, &FlameletOptions::default()).unwrap();
    let tm: Vec<f64> = sols.iter().map(|s| s.t_max()).collect();
    (tab, tm, bs.t_st() - 300.0)
}

#[test]
fn f4_flamelet_extinction_trend() {
    let (tab, tm, dts) = f4_run(1.25, 22);
    for (c, t) in tab.chi_st.iter().zip(&tm) {
        println!("F4 chi_st {c:.4e}: T_max {t:.2}");
    }
    // burning values: T_max above 300 + 20 % of the adiabatic rise; extinction = first value below it (or the first non-converging one)
    let burning = tm.iter().take_while(|&&t| t - 300.0 > 0.2 * dts).count();
    let ext = if burning < tab.chi_st.len() {
        Some(tab.chi_st[burning])
    } else {
        tab.extinction_chi
    };
    println!(
        "F4 converged {} of 22 chi_st values, {burning} burning; first extinguished or non-converging chi_st: {ext:?}",
        tab.chi_st.len()
    );
    let decreasing = (1..burning).all(|i| tm[i] < tm[i - 1]);
    println!("F4 T_max strictly decreasing over the {burning} burning values: {decreasing}");
    assert!(burning >= 6);
    assert!(decreasing);
}

#[test]
fn f6_flamelet_table_lookup() {
    // a table built from analytic rows: T(z, chi) = 300 + 100 z + 10 ln chi (linear in both coordinates), y = z
    let z = linspace(10);
    let chis = [0.1, 1.0, 10.0];
    let rows: Vec<FlameletSolution> = chis
        .iter()
        .map(|&c| FlameletSolution {
            z: z.clone(),
            t: z.iter().map(|&x| 300.0 + 100.0 * x + 10.0 * f64::ln(c)).collect(),
            y: z.iter().map(|&x| vec![x, 1.0 - x]).collect(),
            newton_iters: 0,
            transient_steps: 0,
            residual: 0.0,
        })
        .collect();
    let tab = FlameletTable::from_rows(z.clone(), chis.to_vec(), &rows).unwrap();
    // exact nodes
    let (t, y) = tab.lookup(z[3], chis[1]);
    assert!((t - (300.0 + 30.0)).abs() <= 1e-12 && (y[0] - 0.3).abs() <= 1e-15);
    // a linear field at off-node points
    let mut w = 0.0_f64;
    for &(zz, c) in &[(0.123, 0.3), (0.77, 3.3), (0.5, 0.11), (0.31, 9.0)] {
        let (t, _) = tab.lookup(zz, c);
        w = w.max((t - (300.0 + 100.0 * zz + 10.0 * f64::ln(c))).abs());
    }
    // v = 0 mean equals the lookup
    let (tm, _) = tab.mean(0.37, 0.0, 2.5).unwrap();
    let wm = (tm - tab.lookup(0.37, 2.5).0).abs();
    println!("F6 linear field error {w:.3e} (1e-10); PDF mean with v = 0 minus lookup {wm:.3e} (0)");
    assert!(w <= 1e-10);
    assert_eq!(wm, 0.0);
    // PDF-integrated value of the linear-in-z field equals the lookup at the mean
    let (tp, _) = tab.mean(0.37, 0.02, 2.5).unwrap();
    assert!((tp - tab.lookup(0.37, 2.5).0).abs() <= 1e-10);
}

// ------------------------------------------------------------------------------------------------ progress variable

#[test]
fn p1_p2_zeldovich_front() {
    let gamma = 2.0;
    let k = 5.0;
    let f = |c: f64| k * c * c * (1.0 - c);
    let front = front_speed(gamma, &f, -k).unwrap();
    let exact = (gamma * k / 2.0).sqrt();
    let rel = (front.m - exact).abs() / exact;
    println!(
        "P1 front flux {:.8} vs sqrt(Gamma k / 2) = {exact:.8}: relative error {rel:.3e} (1e-4)",
        front.m
    );
    assert!(rel <= 1e-4);
    let a = (k / (2.0 * gamma)).sqrt();
    let mut w = 0.0_f64;
    for i in 1..100 {
        let c = i as f64 / 100.0;
        w = w.max((front.g_at(c) / a - c * (1.0 - c)).abs());
    }
    println!("P2 profile g/a versus the logistic c(1-c): worst absolute difference {w:.3e} (1e-4)");
    assert!(w <= 1e-4);
}

struct P3Run {
    t_u: f64,
    t_b: f64,
    y_b_h2o: f64,
    traj: Vec<Vec<f64>>,
    ih2o: usize,
}

fn p3_run(g: &Mechanism) -> P3Run {
    let comp = [("H2", 2.0), ("O2", 1.0), ("N2", 3.76)];
    let v: Vec<(String, f64)> = comp.iter().map(|(n, a)| (n.to_string(), *a)).collect();
    let x = rcfd_reactor::mole_fractions(g, &v).unwrap();
    let t0 = 1100.0;
    let eq = equilibrium(g, &x, t0, P_STD, Constraint::HP).unwrap();
    let mut y0 = vec![t0];
    y0.extend(g.x_to_y(&x));
    let tr = run_batch(
        g,
        BatchKind::ConstantPressure,
        Energy::Adiabatic,
        P_STD,
        &y0,
        3e-4,
        Tol::default(),
        Stop::EndTime,
    )
    .unwrap();
    let ih2o = g.species_index("H2O").unwrap();
    P3Run {
        t_u: t0,
        t_b: eq.t,
        y_b_h2o: eq.y[ih2o],
        traj: tr.state,
        ih2o,
    }
}

#[test]
fn p3_progress_variable_from_equilibrium() {
    let Some(g) = gri() else {
        return println!("P3 skipped: GRI-Mech data not fetched");
    };
    let r = p3_run(&g);
    assert_eq!(progress(r.t_u, r.t_u, r.t_b).unwrap(), 0.0);
    assert!((progress(r.t_b, r.t_u, r.t_b).unwrap() - 1.0).abs() <= 1e-15);
    let cmin = r
        .traj
        .iter()
        .map(|s| progress(s[0], r.t_u, r.t_b).unwrap())
        .fold(f64::INFINITY, f64::min);
    let cend = progress(r.traj.last().unwrap()[0], r.t_u, r.t_b).unwrap();
    let src = SourceTable::from_fn(50, &|c| 3.0 - 4.0 * c);
    let mut wl = 0.0_f64;
    for &(cm, v) in &[(0.3, 0.02), (0.6, 0.05), (0.1, 0.01)] {
        wl = wl.max((src.mean(cm, v).unwrap() - (3.0 - 4.0 * cm)).abs());
    }
    println!(
        "P3 T_b (HP equilibrium) {:.2} K from {} K; lowest c along the ignition trajectory {cmin:.3e}; c at 3e-4 s {cend:.6}; linear-source PDF mean error {wl:.3e} (1e-12)",
        r.t_b, r.t_u
    );
    assert!(wl <= 1e-12);
    assert!((0.0..=1.0 + 1e-6).contains(&cend));
}

/// P3 registered clause "c from T is monotone along a constant-pressure ignition trajectory": MISS. The induction period of H2/air is
/// slightly endothermic (chain initiation), so T, hence c, dips below its initial value before the ignition. The limit is intact; the
/// monotone progress variable is `p3b_*`.
#[test]
#[ignore = "known miss: temperature-based c is not monotone in the H2/air induction period (ADR 0053 addendum)"]
fn p3_temperature_progress_monotone_known_miss() {
    let Some(g) = gri() else { return };
    let r = p3_run(&g);
    let mut prev = -1.0;
    let mut worst = 0.0_f64;
    for s in &r.traj {
        let c = progress(s[0], r.t_u, r.t_b).unwrap();
        worst = worst.max(prev - c);
        prev = c;
    }
    assert!(worst <= 1e-12, "largest decrease of c between steps {worst:.3e}");
}

/// P3 with the water mass fraction as the progress variable (new test: the variable is a better choice than T for this induction period).
#[test]
fn p3b_water_progress_variable_is_monotone() {
    let Some(g) = gri() else {
        return println!("P3b skipped: GRI-Mech data not fetched");
    };
    let r = p3_run(&g);
    let mut prev = -1.0;
    let mut worst = 0.0_f64;
    for s in &r.traj {
        let c = progress(s[1 + r.ih2o], 0.0, r.y_b_h2o).unwrap();
        worst = worst.max(prev - c);
        prev = c;
    }
    let cend = progress(r.traj.last().unwrap()[1 + r.ih2o], 0.0, r.y_b_h2o).unwrap();
    println!("P3b c from Y_H2O: largest decrease between steps {worst:.3e} (must be <= 1e-12); c at 3e-4 s {cend:.6}");
    assert!(worst <= 1e-12);
}
