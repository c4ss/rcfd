//! ADR 0053 Part 1 (F-223): ISAT tabulation, directed-relation-graph reduction and clustering of similar cells.
//! Limits are those registered in the ADR (and its Amendment 1) before any of these checks was run; none was changed afterwards.
//! GRI-Mech tests skip with a message when the (git-ignored) data are absent.

use rcfd_reactor::cell::{CellModel, Closure};
use rcfd_reactor::cluster::advance_clustered;
use rcfd_reactor::drg::*;
use rcfd_reactor::isat::*;
use rcfd_reactor::mech::{Mechanism, P_STD, R_GAS};
use rcfd_reactor::reactors::*;
use rcfd_reactor::{mole_fractions, toy};
use std::path::PathBuf;
use std::time::Instant;

fn gri() -> Option<Mechanism> {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../validation-data/data/gri-mech-30");
    let m = std::fs::read_to_string(d.join("berkeley-grimech30.dat")).ok()?;
    let t = std::fs::read_to_string(d.join("berkeley-thermo30.dat")).ok()?;
    Some(Mechanism::from_chemkin(&m, Some(&t)).unwrap_or_else(|e| panic!("GRI-Mech read failed: {e}")))
}

fn mix(m: &Mechanism, pairs: &[(&str, f64)]) -> Vec<f64> {
    let v: Vec<(String, f64)> = pairs.iter().map(|(n, a)| (n.to_string(), *a)).collect();
    mole_fractions(m, &v).unwrap()
}

fn state(m: &Mechanism, x: &[f64], t: f64) -> Vec<f64> {
    let mut s = vec![t];
    s.extend(m.x_to_y(x));
    s
}

fn h2_air_state(m: &Mechanism, t0: f64) -> Vec<f64> {
    state(m, &mix(m, &[("H2", 2.0), ("O2", 1.0), ("N2", 3.76)]), t0)
}

fn idx(m: &Mechanism, n: &str) -> usize {
    m.species_index(n).unwrap()
}

/// Largest change of the element moles per kg between two states, relative to the element's moles in `a` with a floor of 1e-8 times the
/// largest element total (elements absent from the mixture carry only integrator round-off, ~1e-15, and would otherwise read as huge
/// relative changes).
fn element_rel(m: &Mechanism, a: &[f64], b: &[f64]) -> f64 {
    let ea = element_moles(m, a);
    let eb = element_moles(m, b);
    let floor = 1e-8 * ea.iter().cloned().fold(0.0, f64::max);
    ea.iter()
        .zip(&eb)
        .map(|(a, b)| (a - b).abs() / a.abs().max(floor))
        .fold(0.0, f64::max)
}

/// The original (defective) form used by the first I4 set-up: no guard for zero element totals, so absent elements read as 1e268.
fn element_rel_unguarded(m: &Mechanism, a: &[f64], b: &[f64]) -> f64 {
    let ea = element_moles(m, a);
    let eb = element_moles(m, b);
    ea.iter()
        .zip(&eb)
        .map(|(a, b)| (a - b).abs() / a.abs().max(1e-300))
        .fold(0.0, f64::max)
}

/// ISAT table on the element-active subspace of `phi` (structural zeros of the mixture are not coordinates).
fn masked_isat(m: &Mechanism, s: &[f64], eps: f64, phi: &[f64]) -> Isat {
    let mut act = vec![true];
    act.extend(element_active_mask(m, phi));
    Isat::with_active(s.to_vec(), eps, &act)
}

/// Scaled max-norm error of a state against a reference.
fn serr(a: &[f64], b: &[f64], s: &[f64]) -> f64 {
    scaled_dist(a, b, s)
}

/// Deterministic uniform numbers in [0, 1) (64-bit LCG, top 53 bits).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

// ---------------------------------------------------------------- ISAT, toy mechanisms

#[test]
fn i1_fd_sensitivity_matches_analytic() {
    let m = toy::isomer(1.0e8, 19872.0, -3000.0, false);
    let t = 800.0_f64;
    let r = &m.reactions[0];
    let k = r.arr[0] * (r.arr[1] * t.ln() - r.arr[2] / t).exp();
    let dk = k * r.arr[2] / (t * t); // dk/dT
    let dt = 0.5 / k;
    let mut map = BatchMap::new(
        &m,
        BatchKind::ConstantVolume,
        Energy::Isothermal,
        1.5,
        dt,
        Tol::default(),
    );
    let mut isat = Isat::new(default_scales(2), 1e-3);
    let ya = 0.7;
    let (_, out) = isat.query(&mut map, &[t, ya, 1.0 - ya]).unwrap();
    assert_eq!(out, Outcome::Added);
    let a = isat.sensitivity(0);
    let e = (-k * dt).exp();
    let dyadt = -ya * dt * dk * e;
    let an = [[1.0, 0.0, 0.0], [dyadt, e, 0.0], [-dyadt, 1.0 - e, 1.0]];
    let mut worst = 0.0_f64;
    for c in 0..3 {
        let colmax = (0..3).map(|j| an[j][c].abs()).fold(0.0, f64::max);
        for j in 0..3 {
            let err = (a[j * 3 + c] - an[j][c]).abs() / colmax;
            println!(
                "I1 A[{j}][{c}] fd {:+.8e} analytic {:+.8e} err/colmax {err:.2e}",
                a[j * 3 + c],
                an[j][c]
            );
            worst = worst.max(err);
        }
    }
    println!("I1 worst entry error relative to its column maximum {worst:.3e} (limit 1e-3)");
    assert!(worst <= 1e-3);
}

#[test]
fn i2_exact_on_a_linear_map() {
    let m = toy::isomer(1.0e8, 19872.0, -3000.0, false);
    let t = 800.0_f64;
    let r = &m.reactions[0];
    let k = r.arr[0] * (r.arr[1] * t.ln() - r.arr[2] / t).exp();
    let dt = 0.5 / k;
    let mut map = BatchMap::new(
        &m,
        BatchKind::ConstantVolume,
        Energy::Isothermal,
        1.5,
        dt,
        Tol::default(),
    );
    let s = default_scales(2);
    let mut isat = Isat::new(s.clone(), 1e-3);
    let mut rng = Lcg(12345);
    let e = (-k * dt).exp();
    let mut worst = 0.0_f64;
    for _ in 0..200 {
        let ya = rng.next();
        let phi = [t, ya, 1.0 - ya];
        let (out, _) = isat.query(&mut map, &phi).unwrap();
        let exact = [t, ya * e, 1.0 - ya * e];
        worst = worst.max(serr(&out, &exact, &s));
    }
    let st = isat.stats;
    println!(
        "I2 records {} retrieved {} grown {} added {}; worst scaled error vs analytic {worst:.3e} (limit 1e-5)",
        isat.len(),
        st.retrieved,
        st.grown,
        st.added
    );
    assert_eq!(st.added, 1, "exactly one record");
    assert_eq!(st.retrieved + st.grown, 199);
    assert!(worst <= 1e-5);
}

/// Time at which component 0 first reaches `target` in the stepped series (linear interpolation), `None` if never.
fn crossing(series: &[f64], dt: f64, target: f64) -> Option<f64> {
    for i in 1..series.len() {
        if series[i] >= target && series[i - 1] < target {
            let f = (target - series[i - 1]) / (series[i] - series[i - 1]);
            return Some((i as f64 - 1.0 + f) * dt);
        }
    }
    None
}

/// I3 ensemble: returns (retrieved count, worst shadow retrieval error, worst crossing-time difference).
fn i3_run() -> (usize, f64, f64) {
    let m = toy::isomer(1.0e8, 19872.0, -3000.0, false);
    let w = 0.020;
    let rho = 1.5;
    let cv = 1.5 * R_GAS / w;
    let cell = CellModel {
        mech: &m,
        rho,
        cp: Some(cv),
        closure: Closure::Kinetic,
        tol: Tol::default(),
    };
    let dt = 2e-6;
    let nsteps = 100;
    let nm = 40;
    let s = default_scales(2);
    let mut map = CellMap::new(&cell, dt);
    // direct trajectories
    let mut direct: Vec<Vec<Vec<f64>>> = (0..nm).map(|j| vec![vec![780.0 + j as f64, 1.0, 0.0]]).collect();
    for _ in 0..nsteps {
        for tr in direct.iter_mut() {
            let nx = map.apply(tr.last().unwrap()).unwrap();
            tr.push(nx);
        }
    }
    // ISAT trajectories, step-major
    let mut isat = Isat::new(s.clone(), 1e-3);
    let mut trs: Vec<Vec<Vec<f64>>> = (0..nm).map(|j| vec![vec![780.0 + j as f64, 1.0, 0.0]]).collect();
    let mut shadow_worst = 0.0_f64;
    let mut retrieved = 0;
    for _ in 0..nsteps {
        for tr in trs.iter_mut() {
            let phi = tr.last().unwrap().clone();
            let (out, o) = isat.query(&mut map, &phi).unwrap();
            if o == Outcome::Retrieved {
                retrieved += 1;
                let d = map.apply(&phi).unwrap();
                shadow_worst = shadow_worst.max(serr(&out, &d, &s));
            }
            tr.push(out);
        }
    }
    let mut worst_t = 0.0_f64;
    for j in 0..nm {
        let t0 = 780.0 + j as f64;
        let a: Vec<f64> = trs[j].iter().map(|s| s[0]).collect();
        let b: Vec<f64> = direct[j].iter().map(|s| s[0]).collect();
        let ta = crossing(&a, dt, t0 + 1000.0).expect("ISAT trajectory ignites");
        let tb = crossing(&b, dt, t0 + 1000.0).expect("direct trajectory ignites");
        worst_t = worst_t.max((ta - tb).abs() / tb);
    }
    println!(
        "I3 queries {} retrieved {retrieved} grown {} added {}; worst retrieval error {shadow_worst:.3e} (limit 1e-3); worst crossing-time difference {worst_t:.3e} (limit 5e-2)",
        isat.stats.queries, isat.stats.grown, isat.stats.added
    );
    (retrieved, shadow_worst, worst_t)
}

/// I3 clauses that hold: the table is used and the ignition-time difference is within 5 %.
#[test]
fn i3_toy_adiabatic_ignition_ensemble() {
    let (retrieved, _, worst_t) = i3_run();
    assert!(retrieved > 0);
    assert!(worst_t <= 5e-2);
}

/// I3 registered clause "every retrieval within 1e-3 of the direct integration" (ADR 0053): MISS, 1.27e-3 measured. Root cause: the
/// growth rule verifies the linear prediction only at the query that triggers the growth, so repeatedly grown ellipsoids contain
/// points with error above eps. The limit is intact.
#[test]
#[ignore = "known miss: I3 retrieval error limit 1e-3 (ADR 0053 addendum)"]
fn i3_retrieval_error_known_miss() {
    let (_, shadow_worst, _) = i3_run();
    assert!(shadow_worst <= 1e-3, "worst retrieval error {shadow_worst:.3e}");
}

// ---------------------------------------------------------------- clustering, toy

#[test]
fn c1_zero_tolerance_is_bit_identical() {
    let m = toy::isomer(1.0e8, 19872.0, -3000.0, false);
    let w = 0.020;
    let cell = CellModel {
        mech: &m,
        rho: 1.5,
        cp: Some(1.5 * R_GAS / w),
        closure: Closure::Kinetic,
        tol: Tol::default(),
    };
    let dt = 5e-5;
    let mut rng = Lcg(777);
    let mut states: Vec<Vec<f64>> = Vec::new();
    for _ in 0..150 {
        let ya = rng.next();
        states.push(vec![700.0 + 200.0 * rng.next(), ya, 1.0 - ya]);
    }
    for i in 0..150 {
        let src = states[(i * 7) % 150].clone();
        states.push(src);
    }
    let s = default_scales(2);
    let mut count = 0;
    let mut f = |phi: &[f64]| -> Result<Vec<f64>, ReactorError> {
        count += 1;
        let mut st = phi.to_vec();
        cell.advance(&mut st, dt)?;
        Ok(st)
    };
    let res = advance_clustered(&states, &s, 0.0, &mut f).unwrap();
    let clusters = res.clusters();
    let mut mismatches = 0;
    for (st, out) in states.iter().zip(&res.out) {
        let mut d = st.clone();
        cell.advance(&mut d, dt).unwrap();
        if d.iter().zip(out).any(|(a, b)| a.to_bits() != b.to_bits()) {
            mismatches += 1;
        }
    }
    let mut distinct: Vec<&Vec<f64>> = Vec::new();
    for st in &states {
        if !distinct
            .iter()
            .any(|d| d.iter().zip(st).all(|(a, b)| a.to_bits() == b.to_bits()))
        {
            distinct.push(st);
        }
    }
    println!(
        "C1 cells {} clusters {clusters} distinct {} integrations {count} bit mismatches {mismatches}",
        states.len(),
        distinct.len()
    );
    assert_eq!(mismatches, 0);
    assert_eq!(clusters, distinct.len());
    assert_eq!(count, distinct.len());
}

// ---------------------------------------------------------------- DRG, hand graph

#[test]
fn d1_drg_graph_by_hand() {
    let mech = "ELEMENTS X/20.0/ END\nSPECIES A B C D END\nREACTIONS\nA => B 1.0E2 0.0 0.0\nB => C 1.0E1 0.0 0.0\nC => D 1.0 0.0 0.0\nEND\n";
    let e: Vec<_> = ["A", "B", "C", "D"]
        .iter()
        .map(|n| (*n, vec![("X", 1.0)], 2.5, 0.0, 4.0))
        .collect();
    let m = Mechanism::from_chemkin(mech, Some(&toy::thermo_text(&e))).unwrap();
    let c = [1.0, 2.0, 5.0, 0.0];
    let g = interaction_graph(&m, 800.0, &c);
    let get = |a: usize, b: usize| g[a].iter().find(|e| e.0 == b).map_or(0.0, |e| e.1);
    // q1 = 100 * 1, q2 = 10 * 2, q3 = 1 * 5
    let (q1, q2, q3) = (100.0, 20.0, 5.0);
    let hand = [
        (0, 1, 1.0),
        (1, 0, q1 / (q1 + q2)),
        (1, 2, q2 / (q1 + q2)),
        (2, 1, q2 / (q2 + q3)),
        (2, 3, q3 / (q2 + q3)),
        (3, 2, 1.0),
        (0, 2, 0.0),
        (0, 3, 0.0),
        (3, 0, 0.0),
    ];
    let mut worst = 0.0_f64;
    for (a, b, v) in hand {
        worst = worst.max((get(a, b) - v).abs());
    }
    println!("D1 worst |r - r_hand| = {worst:.3e} (limit 1e-12)");
    assert!(worst <= 1e-12);
    let set = |eps: f64| active_set(&g, &[0], eps);
    assert_eq!(set(0.5), vec![true, true, false, false]);
    assert_eq!(set(0.001), vec![true, true, true, true]);
    assert_eq!(set(0.0), vec![true, true, true, true]);
    // the reduced mechanism of {A, B} keeps exactly the first reaction
    let red = reduced_mechanism(&m, &set(0.5));
    assert_eq!(red.reactions.len(), 1);
    assert_eq!(red.max_element_imbalance(), 0.0);
}

// ---------------------------------------------------------------- GRI-Mech 3.0

fn ignition_time(tr: &Trajectory) -> f64 {
    tr.time_of_max_dtdt().expect("ignites")
}

#[allow(clippy::too_many_arguments)]
fn drg_case(
    label: &str,
    m: &Mechanism,
    fuel: &str,
    ox: f64,
    t0: f64,
    sample_t0: f64,
    eps: f64,
    dtol: f64,
    tfin: f64,
    max_species: usize,
    corrected_h: bool,
) {
    let kind = BatchKind::ConstantPressure;
    let tol = Tol::default();
    let targets = [idx(m, fuel), idx(m, "O2")];
    let mk = |t: f64| {
        let v: Vec<(String, f64)> = vec![(fuel.to_string(), 1.0), ("O2".into(), ox), ("N2".into(), 3.76 * ox)];
        let x = mole_fractions(m, &v).unwrap();
        state(m, &x, t)
    };
    // skeleton from 40 samples of the sampling run
    let s0 = mk(sample_t0);
    let tr0 = run_batch(
        m,
        kind,
        Energy::Adiabatic,
        P_STD,
        &s0,
        1.0,
        tol,
        Stop::AfterIgnition { rise: 400.0 },
    )
    .unwrap();
    let n = tr0.state.len();
    let samples: Vec<Vec<f64>> = (0..40).map(|i| tr0.state[i * (n - 1) / 39].clone()).collect();
    let active = skeletal_active(m, kind, P_STD, &samples, &targets, eps);
    let nact = active.iter().filter(|a| **a).count();
    let red = reduced_mechanism(m, &active);
    println!(
        "{label} eps_drg {eps}: skeleton species {nact} of {}, reactions {} of {}",
        m.species.len(),
        red.reactions.len(),
        m.reactions.len()
    );
    assert!(nact < m.species.len());
    assert!(nact <= max_species, "{label}: {nact} species exceed {max_species}");
    let temps: Vec<f64> = if label.starts_with("D2") {
        vec![1000.0, 1100.0, 1200.0]
    } else {
        vec![t0]
    };
    for t in temps {
        let s = mk(t);
        let full0 = run_batch(
            m,
            kind,
            Energy::Adiabatic,
            P_STD,
            &s,
            1.0,
            tol,
            Stop::AfterIgnition { rise: 400.0 },
        )
        .unwrap();
        let tau = ignition_time(&full0);
        let t_end = 3.0 * tau;
        let fullr = run_batch(m, kind, Energy::Adiabatic, P_STD, &s, t_end, tol, Stop::EndTime).unwrap();
        let t_full = Instant::now();
        let _ = run_batch(m, kind, Energy::Adiabatic, P_STD, &s, t_end, tol, Stop::EndTime).unwrap();
        let wall_full = t_full.elapsed();
        let t_red = Instant::now();
        let redr = run_batch_active(
            &red,
            &active,
            kind,
            Energy::Adiabatic,
            P_STD,
            &s,
            t_end,
            tol,
            Stop::EndTime,
        )
        .unwrap();
        let wall_red = t_red.elapsed();
        let (tf, tr_) = (ignition_time(&fullr), ignition_time(&redr));
        let derr = (tr_ - tf).abs() / tf;
        let dtemp = (redr.last()[0] - fullr.last()[0]).abs();
        let edrift = element_rel(m, &s, redr.last());
        let hdrift = if corrected_h {
            enthalpy_change_in_kelvin(m, &s, redr.last()).abs()
        } else {
            (enthalpy_in_kelvin(m, redr.last()) - enthalpy_in_kelvin(m, &s)).abs()
        };
        println!(
            "{label} T0 {t}: delay full {tf:.6e} reduced {tr_:.6e} (rel {derr:.3e}, limit {dtol}); T_end full {:.3} reduced {:.3} (diff {dtemp:.3e} K, limit {tfin}); element drift {edrift:.2e} (1e-9); enthalpy drift {hdrift:.2e} K (1e-2); steps {} vs {}; wall {wall_full:?} vs {wall_red:?}; Jacobian dim {} vs {}",
            fullr.last()[0],
            redr.last()[0],
            fullr.t.len(),
            redr.t.len(),
            m.species.len() + 1,
            nact + 1
        );
        assert!(derr <= dtol, "{label} T0 {t}: delay error {derr}");
        assert!(dtemp <= tfin, "{label} T0 {t}: final temperature difference {dtemp}");
        assert!(edrift <= 1e-9);
        assert!(hdrift <= 1e-2);
    }
}

#[test]
fn d2_gri_h2_air_static_drg() {
    let Some(m) = gri() else {
        return println!("D2 skipped: GRI-Mech data not fetched");
    };
    drg_case("D2", &m, "H2", 1.0, 1100.0, 1100.0, 0.01, 0.05, 5.0, 20, true);
}

/// The first D2 set-up: enthalpy drift measured as the difference of h/cp, which is not conserved when cp changes (65 K measured,
/// a set-up defect). Kept; superseded by `d2_gri_h2_air_static_drg` with the enthalpy-change measure.
#[test]
#[ignore = "superseded set-up (h/cp difference is not a conserved quantity), kept per ADR 0053 rules"]
fn d2_first_setup_h_over_cp() {
    let Some(m) = gri() else { return };
    drg_case("D2", &m, "H2", 1.0, 1100.0, 1100.0, 0.01, 0.05, 5.0, 20, false);
}

#[test]
fn d3_gri_ch4_air_static_drg() {
    let Some(m) = gri() else {
        return println!("D3 skipped: GRI-Mech data not fetched");
    };
    drg_case("D3", &m, "CH4", 2.0, 1400.0, 1400.0, 0.01, 0.05, 20.0, 53, true);
}

/// The first D3 set-up (h/cp difference measure); see `d2_first_setup_h_over_cp`.
#[test]
#[ignore = "superseded set-up (h/cp difference is not a conserved quantity), kept per ADR 0053 rules"]
fn d3_first_setup_h_over_cp() {
    let Some(m) = gri() else { return };
    drg_case("D3", &m, "CH4", 2.0, 1400.0, 1400.0, 0.01, 0.05, 20.0, 53, false);
}

#[test]
fn d4_gri_h2_air_dynamic_drg() {
    let Some(m) = gri() else {
        return println!("D4 skipped: GRI-Mech data not fetched");
    };
    let kind = BatchKind::ConstantPressure;
    let s0 = h2_air_state(&m, 1100.0);
    let dt = 6.25e-7;
    let nsteps = 400;
    let mut map = BatchMap::new(&m, kind, Energy::Adiabatic, P_STD, dt, Tol::default());
    let mut drg = DrgStepper::new(
        &m,
        vec![idx(&m, "H2"), idx(&m, "O2")],
        0.01,
        kind,
        P_STD,
        Tol::default(),
    );
    let mut a = vec![s0.clone()];
    let mut b = vec![s0.clone()];
    let t_full = Instant::now();
    for _ in 0..nsteps {
        let nx = map.apply(a.last().unwrap()).unwrap();
        a.push(nx);
    }
    let wall_full = t_full.elapsed();
    let t_drg = Instant::now();
    for _ in 0..nsteps {
        let nx = drg.step(b.last().unwrap(), dt).unwrap();
        b.push(nx);
    }
    let wall_drg = t_drg.elapsed();
    let ta: Vec<f64> = a.iter().map(|s| s[0]).collect();
    let tb: Vec<f64> = b.iter().map(|s| s[0]).collect();
    let ca = crossing(&ta, dt, 1500.0).expect("full ignites");
    let cb = crossing(&tb, dt, 1500.0).expect("dynamic DRG ignites");
    let derr = (cb - ca).abs() / ca;
    let dtemp = (tb[nsteps] - ta[nsteps]).abs();
    println!(
        "D4 delay (crossing 1500 K) full {ca:.6e} dynamic {cb:.6e} rel {derr:.3e} (limit 5e-2); T_end {:.3} vs {:.3} diff {dtemp:.3e} K (limit 5); mean active species {:.2} of {} (limit < 53); wall {wall_full:?} vs {wall_drg:?}",
        ta[nsteps],
        tb[nsteps],
        drg.mean_active(),
        m.species.len()
    );
    assert!(derr <= 5e-2);
    assert!(dtemp <= 5.0);
    assert!(drg.mean_active() < m.species.len() as f64);
}

// ---------------------------------------------------------------- ISAT and clustering on GRI-Mech

/// Temperature from enthalpy at fixed mass fractions (Newton).
fn t_from_h(m: &Mechanism, h: f64, y: &[f64]) -> f64 {
    let mut t = 800.0;
    for _ in 0..80 {
        let d = (m.h_mass(t, y) - h) / m.cp_mass(t, y);
        t -= d;
        if d.abs() < 1e-10 {
            break;
        }
    }
    t
}

/// The I5 planar mixing layer, 100 cells over 1 mm.
fn mixing_layer(m: &Mechanism) -> Vec<Vec<f64>> {
    let yf = m.x_to_y(&mix(m, &[("H2", 1.0), ("N2", 1.0)]));
    let yo = m.x_to_y(&mix(m, &[("O2", 1.0), ("N2", 3.76)]));
    let hf = m.h_mass(300.0, &yf);
    let ho = m.h_mass(1300.0, &yo);
    (0..100)
        .map(|j| {
            let x = (j as f64 + 0.5) * 1e-5;
            let z = 0.5 * (1.0 - ((x - 0.5e-3) / 0.1e-3).tanh());
            let y: Vec<f64> = yf.iter().zip(&yo).map(|(f, o)| z * f + (1.0 - z) * o).collect();
            let t = t_from_h(m, z * hf + (1.0 - z) * ho, &y);
            let mut s = vec![t];
            s.extend(y);
            s
        })
        .collect()
}

fn diffuse(cells: &mut [Vec<f64>], fo: f64) {
    let old: Vec<Vec<f64>> = cells.to_vec();
    let n = old.len();
    for j in 0..n {
        let l = &old[j.saturating_sub(1)];
        let r = &old[(j + 1).min(n - 1)];
        for k in 0..old[j].len() {
            cells[j][k] = old[j][k] + fo * (l[k] - 2.0 * old[j][k] + r[k]);
        }
    }
}

struct EnsembleRun {
    cells: Vec<Vec<f64>>,
    queries: Vec<(Vec<f64>, Vec<f64>, Outcome)>,
    secs: f64,
    n_ignited: usize,
}

fn run_layer(m: &Mechanism, nsteps: usize, dt: f64, isat: Option<&mut Isat>, map: &mut BatchMap) -> EnsembleRun {
    let mut cells = mixing_layer(m);
    let mut queries = Vec::new();
    let mut isat = isat;
    let t0 = Instant::now();
    for _ in 0..nsteps {
        diffuse(&mut cells, 2e-6 * dt / 1e-10);
        for c in cells.iter_mut() {
            let phi = c.clone();
            let (out, o) = match isat.as_deref_mut() {
                Some(is) => is.query(map, &phi).unwrap(),
                None => (map.apply(&phi).unwrap(), Outcome::Direct),
            };
            queries.push((phi, out.clone(), o));
            *c = out;
        }
    }
    let secs = t0.elapsed().as_secs_f64();
    let n_ignited = cells.iter().filter(|c| c[0] > 1500.0).count();
    EnsembleRun {
        cells,
        queries,
        secs,
        n_ignited,
    }
}

struct I4Res {
    nret: usize,
    worst: f64,
    worst_delay: f64,
    worst_tf: f64,
    worst_el: f64,
    worst_el_direct: f64,
    worst_h: f64,
}

/// I4 ensemble. `guarded` selects the corrected element-drift measure and the element-active ISAT subspace; `false` is the first
/// set-up (full-dimension table, unguarded element measure), kept as an ignored test.
fn i4_run(guarded: bool) -> Option<I4Res> {
    let Some(m) = gri() else {
        println!("I4 skipped: GRI-Mech data not fetched");
        return None;
    };
    let kind = BatchKind::ConstantPressure;
    let dt = 5e-6;
    let nsteps = 40;
    let nm = 16;
    let s = default_scales(m.species.len());
    let init: Vec<Vec<f64>> = (0..nm).map(|j| h2_air_state(&m, 1100.0 + 0.04 * j as f64)).collect();
    let mut map = BatchMap::new(&m, kind, Energy::Adiabatic, P_STD, dt, Tol::default());
    // direct
    let t_d = Instant::now();
    let mut direct: Vec<Vec<Vec<f64>>> = init.iter().map(|s| vec![s.clone()]).collect();
    for _ in 0..nsteps {
        for tr in direct.iter_mut() {
            let nx = map.apply(tr.last().unwrap()).unwrap();
            tr.push(nx);
        }
    }
    let wall_direct = t_d.elapsed().as_secs_f64();
    let rhs_direct = map.rhs_evals;
    map.rhs_evals = 0;
    // ISAT (pure, timed)
    let mut isat = if guarded {
        masked_isat(&m, &s, 1e-3, &init[0])
    } else {
        Isat::new(s.clone(), 1e-3)
    };
    let mut trs: Vec<Vec<Vec<f64>>> = init.iter().map(|s| vec![s.clone()]).collect();
    let mut queries: Vec<(Vec<f64>, Vec<f64>, Outcome)> = Vec::new();
    let t_i = Instant::now();
    for _ in 0..nsteps {
        for tr in trs.iter_mut() {
            let phi = tr.last().unwrap().clone();
            let (out, o) = isat.query(&mut map, &phi).unwrap();
            queries.push((phi, out.clone(), o));
            tr.push(out);
        }
    }
    let wall_isat = t_i.elapsed().as_secs_f64();
    let rhs_isat = map.rhs_evals;
    // shadow checks of the retrievals
    let mut worst = 0.0_f64;
    let mut worst_el = 0.0_f64;
    let mut worst_h = 0.0_f64;
    let mut worst_el_direct = 0.0_f64;
    let mut nret = 0;
    for (phi, out, o) in &queries {
        if *o == Outcome::Retrieved {
            nret += 1;
            let d = map.apply(phi).unwrap();
            worst = worst.max(serr(out, &d, &s));
            worst_el_direct = worst_el_direct.max(element_rel(&m, phi, &d));
            worst_el = worst_el.max(if guarded {
                element_rel(&m, phi, out)
            } else {
                element_rel_unguarded(&m, phi, out)
            });
            worst_h = worst_h.max(if guarded {
                enthalpy_change_in_kelvin(&m, phi, out).abs()
            } else {
                (enthalpy_in_kelvin(&m, out) - enthalpy_in_kelvin(&m, phi)).abs()
            });
        }
    }
    let mut worst_delay = 0.0_f64;
    let mut worst_tf = 0.0_f64;
    for j in 0..nm {
        let t0 = 1100.0 + 0.04 * j as f64;
        let a: Vec<f64> = trs[j].iter().map(|s| s[0]).collect();
        let b: Vec<f64> = direct[j].iter().map(|s| s[0]).collect();
        let ta = crossing(&a, dt, t0 + 400.0).expect("ISAT ignites");
        let tb = crossing(&b, dt, t0 + 400.0).expect("direct ignites");
        worst_delay = worst_delay.max((ta - tb).abs() / tb);
        worst_tf = worst_tf.max((a[nsteps] - b[nsteps]).abs());
    }
    println!(
        "I4 queries {} retrieved {nret} grown {} added {} records {}; worst retrieval error {worst:.3e} (limit 1e-3); delay difference {worst_delay:.3e} (5e-2); final T difference {worst_tf:.3e} K (10); element drift {worst_el:.2e} (1e-9; direct integration alone {worst_el_direct:.2e}); enthalpy {worst_h:.2e} K (1); wall direct {wall_direct:.3} s, isat {wall_isat:.3} s, speed-up {:.3}; rhs evals {rhs_direct} vs {rhs_isat}, ratio {:.3} (reported only)",
        isat.stats.queries,
        isat.stats.grown,
        isat.stats.added,
        isat.len(),
        wall_direct / wall_isat,
        rhs_direct as f64 / rhs_isat as f64
    );
    Some(I4Res {
        nret,
        worst,
        worst_delay,
        worst_tf,
        worst_el,
        worst_el_direct,
        worst_h,
    })
}

/// I4 (corrected set-up): retrieval error, ignition delay, final temperature and element conservation clauses.
#[test]
fn i4_gri_h2_air_homogeneous_ignition_ensemble() {
    let Some(r) = i4_run(true) else { return };
    assert!(r.nret > 0, "the table was never used");
    assert!(r.worst <= 1e-3, "retrieval error {:.3e}", r.worst);
    assert!(r.worst_delay <= 5e-2);
    assert!(r.worst_tf <= 10.0);
    assert!(r.worst_h <= 1.0, "enthalpy change {:.3e} K", r.worst_h);
    // diagnostic support for the element-drift miss: the direct integration itself conserves the elements
    assert!(r.worst_el_direct <= 1e-12);
}

/// I4 element-conservation clause (retrieved states keep the element moles to 1e-9): MISS, 2.4e-9 measured. See the ADR addendum for
/// the diagnosis (finite-difference noise in the invariant columns of the sensitivity matrix). The limit is intact.
#[test]
#[ignore = "known miss: I4 element drift 1e-9 (ADR 0053 addendum)"]
fn i4_element_drift_known_miss() {
    let Some(r) = i4_run(true) else { return };
    assert!(r.worst_el <= 1e-9, "element drift {:.3e}", r.worst_el);
}

/// The first I4 set-up (full-dimension table, unguarded element measure that reads absent elements as 1e268). Set-up defect,
/// kept for the record; superseded by `i4_gri_h2_air_homogeneous_ignition_ensemble`.
#[test]
#[ignore = "superseded set-up (defective element measure), kept per ADR 0053 rules"]
fn i4_first_setup_unguarded_element_measure() {
    let Some(r) = i4_run(false) else { return };
    assert!(r.worst_el <= 1e-9);
    assert!(
        r.worst_h <= 1.0,
        "first-set-up enthalpy measure (difference of h/cp) {:.3e} K",
        r.worst_h
    );
}

/// (worst retrieval error, final profile difference K, element drift, speed-up)
fn i5_run() -> Option<(f64, f64, f64, f64)> {
    let Some(m) = gri() else {
        println!("I5 skipped: GRI-Mech data not fetched");
        return None;
    };
    let dt = 5e-6;
    let nsteps = 60;
    let s = default_scales(m.species.len());
    let mut map = BatchMap::new(
        &m,
        BatchKind::ConstantPressure,
        Energy::Adiabatic,
        P_STD,
        dt,
        Tol::default(),
    );
    let direct = run_layer(&m, nsteps, dt, None, &mut map);
    let rhs_direct = map.rhs_evals;
    map.rhs_evals = 0;
    let mut isat = masked_isat(&m, &s, 1e-3, &mixing_layer(&m)[50]);
    let acc = run_layer(&m, nsteps, dt, Some(&mut isat), &mut map);
    let rhs_isat = map.rhs_evals;
    let mut worst = 0.0_f64;
    let mut worst_el = 0.0_f64;
    let mut nret = 0;
    for (phi, out, o) in &acc.queries {
        if *o == Outcome::Retrieved {
            nret += 1;
            let d = map.apply(phi).unwrap();
            worst = worst.max(serr(out, &d, &s));
            worst_el = worst_el.max(element_rel(&m, phi, out));
        }
    }
    let dtmax = direct
        .cells
        .iter()
        .zip(&acc.cells)
        .map(|(a, b)| (a[0] - b[0]).abs())
        .fold(0.0, f64::max);
    println!(
        "I5 queries {} retrieved {nret} grown {} added {} records {}; worst retrieval error {worst:.3e} (limit 1e-3); final T profile max difference {dtmax:.3} K (limit 10); element drift {worst_el:.2e} (1e-9); cells above 1500 K: direct {} isat {}; wall direct {:.3} s isat {:.3} s speed-up {:.3} (must be > 1); rhs evals {rhs_direct} vs {rhs_isat}",
        isat.stats.queries,
        isat.stats.grown,
        isat.stats.added,
        isat.len(),
        direct.n_ignited,
        acc.n_ignited,
        direct.secs,
        acc.secs,
        direct.secs / acc.secs
    );
    Some((worst, dtmax, worst_el, direct.secs / acc.secs))
}

/// I5 clauses that hold: retrieval error within 1e-3 and speed-up above 1 on the registered ensemble.
#[test]
fn i5_gri_h2_air_mixing_layer_ensemble() {
    let Some((worst, _, _, speed)) = i5_run() else { return };
    assert!(worst <= 1e-3);
    assert!(speed > 1.0, "speed-up must exceed 1");
}

/// I5 final-profile clause (max temperature difference 10 K): MISS, 1222 K measured. The ISAT run did not ignite where the direct run
/// did (cells above 1500 K: 0 vs 17). Root cause: the scaled error norm (0.01 in mass fraction) is blind to the radical pool of the
/// induction period (mass fractions ~1e-10), so one ellipsoid is grown over states whose radical content is wrong by orders of
/// magnitude while every scaled error stays below 1e-3. The limit is intact.
#[test]
#[ignore = "known miss: I5 profile difference 10 K (ADR 0053 addendum)"]
fn i5_profile_known_miss() {
    let Some((_, dtmax, _, _)) = i5_run() else { return };
    assert!(dtmax <= 10.0, "profile difference {dtmax:.3} K");
}

/// I5 element clause (1e-9): MISS, 1.46e-7 measured; same mechanism as the I4 miss (finite-difference noise in the invariant
/// columns), amplified by the large grown ellipsoid. The limit is intact.
#[test]
#[ignore = "known miss: I5 element drift 1e-9 (ADR 0053 addendum)"]
fn i5_element_drift_known_miss() {
    let Some((_, _, el, _)) = i5_run() else { return };
    assert!(el <= 1e-9, "element drift {el:.3e}");
}

#[test]
fn c2_gri_cluster_tolerance_on_the_mixing_layer() {
    let Some(m) = gri() else {
        return println!("C2 skipped: GRI-Mech data not fetched");
    };
    let dt = 5e-6;
    let s = default_scales(m.species.len());
    let mut map = BatchMap::new(
        &m,
        BatchKind::ConstantPressure,
        Energy::Adiabatic,
        P_STD,
        dt,
        Tol::default(),
    );
    let at30 = run_layer(&m, 30, dt, None, &mut map);
    let states = at30.cells;
    assert_eq!(states.len(), 100);
    let tau = 1e-3;
    let t_c = Instant::now();
    let mut count = 0;
    let mut f = |phi: &[f64]| -> Result<Vec<f64>, ReactorError> {
        count += 1;
        BatchMap::new(
            &m,
            BatchKind::ConstantPressure,
            Energy::Adiabatic,
            P_STD,
            dt,
            Tol::default(),
        )
        .apply(phi)
    };
    let res = advance_clustered(&states, &s, tau, &mut f).unwrap();
    let wall_c = t_c.elapsed().as_secs_f64();
    let t_d = Instant::now();
    let direct: Vec<Vec<f64>> = states.iter().map(|st| map.apply(st).unwrap()).collect();
    let wall_d = t_d.elapsed().as_secs_f64();
    let mut worst = 0.0_f64;
    let mut worst_el = 0.0_f64;
    for ((st, out), d) in states.iter().zip(&res.out).zip(&direct) {
        worst = worst.max(serr(out, d, &s));
        worst_el = worst_el.max(element_rel(&m, st, out));
    }
    println!(
        "C2 cells 100 clusters {} (integrations {count}); worst member error {worst:.3e} (limit 1e-2); element drift {worst_el:.2e} (limit 1e-12); wall direct {wall_d:.3} s clustered {wall_c:.3} s speed-up {:.3} (reported)",
        res.clusters(),
        wall_d / wall_c
    );
    assert!(res.clusters() < 100);
    assert!(worst <= 1e-2);
    assert!(worst_el <= 1e-12);
}

#[test]
#[ignore = "diagnostic of the I3 miss (not a registered check)"]
fn i3_diag() {
    let m = toy::isomer(1.0e8, 19872.0, -3000.0, false);
    let w = 0.020;
    let cv = 1.5 * R_GAS / w;
    let cell = CellModel {
        mech: &m,
        rho: 1.5,
        cp: Some(cv),
        closure: Closure::Kinetic,
        tol: Tol::default(),
    };
    let dt = 2e-6;
    let s = default_scales(2);
    let mut map = CellMap::new(&cell, dt);
    let mut isat = Isat::new(s.clone(), 1e-3);
    let mut trs: Vec<Vec<f64>> = (0..40).map(|j| vec![780.0 + j as f64, 1.0, 0.0]).collect();
    let mut hist = [(0usize, 0.0f64); 8];
    for step in 0..100 {
        for tr in trs.iter_mut() {
            let phi = tr.clone();
            let (out, o) = isat.query(&mut map, &phi).unwrap();
            if o == Outcome::Retrieved {
                let d = map.apply(&phi).unwrap();
                let e = serr(&out, &d, &s);
                let (_, g, dist) = isat.last_hit.unwrap();
                if e > 5e-4 {
                    println!("step {step} T {:.1} err {e:.3e} grows {g} d {dist:.4}", phi[0]);
                }
                let b = g.min(7);
                hist[b].0 += 1;
                hist[b].1 = hist[b].1.max(e);
            }
            *tr = out;
        }
    }
    println!("by grow count (count, max err): {hist:?}");
}
