//! ADR 0623 BD1 to BD7: the reduced-order fan as a `ledger::Model` and its tolerance bands through `uq::run`.

use rcfd_api::design::fan_rom_eval::{
    fan_rom_bands, fan_rom_metric_unit, FanRomEvaluator, FanRomModel, RomCondition, FAN_ROM_PARAMS,
};
use rcfd_api::design::ledger::{Ledger, Model, Quantity, Verdict};
use rcfd_api::design::sampling;
use rcfd_api::design::uq::{self, clopper_pearson, UqResult};
use rcfd_mesh::fan::SectionFamily;
use rcfd_mesh::fan_design::{Dist, Tolerances};
use rcfd_post::fan_rom::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

const Q_D: f64 = 0.045;
const BAND_METRICS: [&str; 4] = ["q", "dp_s", "power", "tone1_pa"];

fn op() -> Operating {
    Operating {
        rpm: 3000.0,
        rho: 1.2,
        nu: 1.5e-5,
    }
}

/// Set-up C of ADR 0119 as in `fan_rom_design_w46b.rs`.
fn set_up() -> (RomFan, f64) {
    let s = |radius: f64| RomSection {
        radius,
        chord: 0.030,
        stagger: 0.0,
        camber: 0.04,
        camber_pos: 0.5,
    };
    let base = RomFan {
        n_blades: 7,
        r_hub: 0.030,
        r_tip: 0.058,
        tip_gap: 0.001,
        family: SectionFamily::Naca4,
        sections: vec![s(0.030), s(0.058)],
    };
    let m = RomModel::default();
    let fan = free_vortex_design(&base, &m, &op(), Q_D, 45.0 / 1.2 / op().omega()).unwrap();
    let k = FanRom::new(fan.clone(), m).unwrap().point(&op(), Q_D).unwrap().dp_s / (Q_D * Q_D);
    (fan, k)
}

fn cond(k: f64) -> RomCondition {
    RomCondition {
        op: op(),
        system: SystemCurve { k, l: 0.0 },
        c0: 343.0,
        observer_r: 1.0,
        observer_theta: std::f64::consts::FRAC_PI_2,
        n_scan: 100,
    }
}

fn model_with(m: RomModel) -> FanRomModel {
    let (fan, k) = set_up();
    FanRomModel {
        fan,
        model: m,
        cond: cond(k),
    }
}

fn evaluator(m: &FanRomModel) -> FanRomEvaluator {
    FanRomEvaluator {
        geometry: Box::new(|_x: &[f64]| Err("unused".into())),
        model: m.model,
        base: m.cond,
    }
}

/// Tolerances T3 of ADR 0623 (set-up T of ADR 0118 for the three representable fields).
fn t3() -> Tolerances {
    let tn = |mean, sd, lo, hi| Dist::TruncatedNormal { mean, sd, lo, hi };
    Tolerances {
        stagger_common_deg: tn(0.0, 0.5, -1.0, 1.0),
        chord_rel: tn(0.0, 0.01, -0.015, 0.02),
        tip_gap_delta: tn(0.0, 1e-4, -2e-4, 2e-4),
        ..Tolerances::default()
    }
}

fn params(x: &[f64]) -> Vec<Quantity> {
    FAN_ROM_PARAMS
        .iter()
        .zip(x)
        .map(|((n, u), &v)| Quantity::new(n, v, u))
        .collect()
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f268r_bd_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ledger(path: PathBuf) -> Ledger {
    let mut l = Ledger::new(&path);
    l.clock = || 1_700_000_000_000;
    l
}

#[test]
fn bd1_the_adapter_equals_the_evaluator_bit_for_bit() {
    let m = model_with(RomModel {
        annulus_drag: 0.024,
        secondary_drag: 0.025,
        ..RomModel::default()
    });
    let ev = evaluator(&m);
    let pts = sampling::scale(
        &sampling::latin_hypercube(64, 3, 623_101),
        &[(-1.0, 1.0), (-0.015, 0.02), (-2e-4, 2e-4)],
    );
    let mut equal = 0;
    let mut first_bad = None;
    for x in &pts {
        // the perturbation written here independently of the adapter
        let mut f = m.fan.clone();
        for s in &mut f.sections {
            s.stagger += x[0].to_radians();
            s.chord *= 1.0 + x[1];
        }
        f.tip_gap += x[2];
        let want = ev.metrics(f, &m.cond);
        let got = m.evaluate(&params(x));
        let same = match &want {
            Ok(w) => {
                got.verdict == Verdict::Ok
                    && got.metrics.len() == w.len()
                    && got.metrics.iter().zip(w).all(|(q, (k, v))| {
                        q.name == *k && q.value.to_bits() == v.to_bits() && q.unit == fan_rom_metric_unit(k)
                    })
            }
            Err(e) => got.verdict != Verdict::Ok && got.reason.contains(e.as_str()),
        };
        if same {
            equal += 1;
        } else if first_bad.is_none() {
            first_bad = Some((x.clone(), want, got));
        }
    }
    eprintln!("BD1: {equal} of 64 points bit for bit; first difference {first_bad:?}");
    assert_eq!(equal, 64);
    assert!(
        pts.iter().all(|x| m.evaluate(&params(x)).verdict == Verdict::Ok),
        "set-up: every point evaluates"
    );
}

fn nominal(m: &FanRomModel) -> BTreeMap<String, f64> {
    evaluator(m).metrics(m.fan.clone(), &m.cond).unwrap()
}

#[test]
fn bd4_zero_width_tolerances_give_sd_zero_and_the_nominal_value() {
    let m = model_with(RomModel::default());
    let nom = nominal(&m);
    let r = fan_rom_bands(&m, "bd4", &Tolerances::default(), 128, 623_104, 4, None).unwrap();
    let mut bad = Vec::new();
    for s in &r.metrics {
        let v = nom[&s.name];
        let ends_ok = s
            .quantiles
            .iter()
            .all(|q| [q.value, q.lo, q.hi].iter().all(|x| x.to_bits() == v.to_bits()));
        if s.sd != 0.0 || !ends_ok {
            bad.push((s.name.clone(), s.sd, s.quantiles.clone()));
        }
    }
    eprintln!(
        "BD4: {} metrics, n_ok {} of {}, sds {:?}; failing {bad:?}",
        r.metrics.len(),
        r.n_ok,
        r.n_samples,
        r.metrics.iter().map(|s| (s.name.as_str(), s.sd)).collect::<Vec<_>>()
    );
    assert_eq!(r.metrics.len(), 8);
    assert_eq!(r.n_ok, 128);
    assert!(bad.is_empty());
}

#[test]
fn bd6_failed_samples_are_counted_and_logged_with_their_reason() {
    let m = model_with(RomModel::default());
    let tol = Tolerances {
        tip_gap_delta: Dist::Uniform { lo: -1.5e-3, hi: 0.0 },
        ..t3()
    };
    let dir = tmp("bd6");
    let l = ledger(dir.join("ledger.jsonl"));
    let r = fan_rom_bands(&m, "bd6", &tol, 120, 623_106, 4, Some(&l)).unwrap();
    let c = l.read().unwrap();
    let evals: Vec<_> = c.entries.iter().filter(|e| e.kind == "evaluation").collect();
    let non_ok = evals.iter().filter(|e| e.verdict != Verdict::Ok).count();
    let with_reason = evals
        .iter()
        .filter(|e| e.verdict != Verdict::Ok && e.reason.contains("tip_gap"))
        .count();
    let failed = r.n_samples - r.n_ok;
    let (lo, hi) = clopper_pearson(failed, r.n_samples, 0.999);
    eprintln!(
        "BD6: {failed} failed of {} (fraction {:.4}, 0.999 interval [{lo:.4}, {hi:.4}]), {non_ok} non-ok entries, {with_reason} \
         naming tip_gap",
        r.n_samples,
        failed as f64 / r.n_samples as f64
    );
    assert_eq!(evals.len(), 120);
    assert_eq!(failed, non_ok);
    assert_eq!(with_reason, non_ok);
    assert!(r.n_ok >= 1 && failed >= 1);
    assert!(lo <= 1.0 / 3.0 && 1.0 / 3.0 <= hi);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bd7_band_ledger_bytes_do_not_depend_on_the_worker_count() {
    let m = model_with(RomModel::default());
    let dir = tmp("bd7");
    let mut bytes = Vec::new();
    for w in [1usize, 4] {
        let l = ledger(dir.join(format!("ledger{w}.jsonl")));
        fan_rom_bands(&m, "bd7", &t3(), 64, 623_107, w, Some(&l)).unwrap();
        bytes.push(std::fs::read(l.path()).unwrap());
    }
    let lines = bytes[0].iter().filter(|&&b| b == b'\n').count();
    eprintln!(
        "BD7: {} and {} bytes, {lines} lines, equal {}",
        bytes[0].len(),
        bytes[1].len(),
        bytes[0] == bytes[1]
    );
    assert_eq!(lines, 65);
    assert_eq!(bytes[0], bytes[1]);
    let _ = std::fs::remove_dir_all(&dir);
}

fn stats<'a>(r: &'a UqResult, name: &str) -> &'a uq::MetricStats {
    r.metrics.iter().find(|m| m.name == name).unwrap()
}

/// sd of a standard normal truncated to `[a, b]`.
fn truncated_sd(a: f64, b: f64) -> f64 {
    let phi = |x: f64| (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let z = uq::norm_cdf(b) - uq::norm_cdf(a);
    let m = (phi(a) - phi(b)) / z;
    (1.0 + (a * phi(a) - b * phi(b)) / z - m * m).sqrt()
}

#[test]
#[ignore = "heavy record (about 1 h with 4 workers): ADR 0623 BD2 and BD3, run once; BD7 is the fast test of the path"]
fn bd2_bd3_bands_against_a_100000_sample_reference_heavy() {
    let m = model_with(RomModel::default());
    let dir = tmp("bd2");
    let l = ledger(dir.join("ledger.jsonl"));
    let t = std::time::Instant::now();
    let r = fan_rom_bands(&m, "bd2", &t3(), 2000, 623_102, 4, Some(&l)).unwrap();
    let t_band = t.elapsed().as_secs_f64();
    let reference = fan_rom_bands(&m, "bd2-reference", &t3(), 100_000, 623_200, 4, None).unwrap();
    let t_all = t.elapsed().as_secs_f64();
    // BD2
    let mut contained = 0;
    let mut shape_ok = true;
    for name in BAND_METRICS {
        let (a, b) = (stats(&r, name), stats(&reference, name));
        for (qa, qb) in a.quantiles.iter().zip(&b.quantiles) {
            let inside = qa.lo <= qb.value && qb.value <= qa.hi;
            contained += usize::from(inside);
            shape_ok &= qa.lo.is_finite() && qa.hi.is_finite() && qa.coverage >= 0.95;
            eprintln!(
                "BD2 {name} p {}: band {:e} [{:e}, {:e}] coverage {:.4}; reference {:e}; contained {inside}",
                qa.p, qa.value, qa.lo, qa.hi, qa.coverage, qb.value
            );
        }
        shape_ok &= a.quantiles.iter().map(|q| q.p).collect::<Vec<_>>() == [0.05, 0.5, 0.95];
    }
    let c = l.read().unwrap();
    eprintln!(
        "BD2: {contained} of 12 intervals contain the reference; n_ok {} of {}, reference n_ok {} of {}; ledger {} entries, {} \
         skipped; {t_band:.0} s band, {t_all:.0} s in all",
        r.n_ok,
        r.n_samples,
        reference.n_ok,
        reference.n_samples,
        c.entries.len(),
        c.skipped.len()
    );
    // BD3: first-order propagation with central differences, step one parent sd
    let sds = [0.5, 0.01, 1e-4];
    let tsd = [
        0.5 * truncated_sd(-2.0, 2.0),
        0.01 * truncated_sd(-1.5, 2.0),
        1e-4 * truncated_sd(-2.0, 2.0),
    ];
    let limit = 0.01 + 4.0 / (2.0f64 * 2000.0).sqrt();
    let mut bd3_ok = true;
    for name in BAND_METRICS {
        let mut var = 0.0;
        for i in 0..3 {
            let mut x = [0.0; 3];
            x[i] = sds[i];
            let up = m.evaluate(&params(&x));
            x[i] = -sds[i];
            let dn = m.evaluate(&params(&x));
            let v = |o: &rcfd_api::design::ledger::Outcome| o.metrics.iter().find(|q| q.name == name).unwrap().value;
            let g = (v(&up) - v(&dn)) / (2.0 * sds[i]);
            var += (g * tsd[i]).powi(2);
        }
        let lin = var.sqrt();
        let sd = stats(&r, name).sd;
        let rel = (sd - lin).abs() / lin;
        bd3_ok &= rel <= limit;
        eprintln!("BD3 {name}: sd {sd:e}, first-order {lin:e}, relative difference {rel:.4} (limit {limit:.4})");
    }
    assert!(shape_ok, "BD2: quantiles and finite intervals of coverage >= 0.95");
    assert_eq!(c.entries.len(), 2001);
    assert!(c.skipped.is_empty());
    assert!(contained >= 10, "BD2: {contained} of 12");
    assert!(bd3_ok, "BD3");
    let _ = std::fs::remove_dir_all(&dir);
}
