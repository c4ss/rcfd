//! ADR 0645 (F-716): a stall pre-check of the reduced-order model before each evaluation of the surrogate loop
//! (`bayes::run_prechecked`, `fan_objective::StallPreCheck`). Checks A (the acceptance on the F-698 R1 set-up), NC (controls),
//! F (fast paths), L (resume), D (workers) and B1 (default bits) as registered in ADR 0645. The heavy record (A1 to A6, NC1,
//! NC2) is `#[ignore]`d and run once; the fast tests cover the same code paths. The F3 set-up is copied from
//! `tests/f698_surrogate.rs` (ADR 0621), which is not edited.

use rcfd_api::design::bayes::{self, BayesOptions, BayesParam, BayesSpec, PreCheck, PreCheckScope, PreCheckedResult};
use rcfd_api::design::fan_objective::*;
use rcfd_api::design::fan_rom_eval::RomCondition;
use rcfd_api::design::ledger::{Entry, Fidelity, FnModel, Ledger, Model, Outcome, Quantity, Verdict};
use rcfd_api::design::study::{parallel_map, ParamDef, Sense};
use rcfd_mesh::fan::SectionFamily;
use rcfd_mesh::fan_design::{ComputerFan, ControlPoint, Tolerances};
use rcfd_post::fan_rom::*;
use std::f64::consts::PI;
use std::path::PathBuf;
use std::time::Instant;

const CLOCK: u64 = 1_700_000_000_000;
/// ADR 0621 addendum 1: the lowest feasible value of the 21^3 grid on F3 [W].
const P_REF: f64 = 1.455477;

// ---------------------------------------------------------------- F3 (copied from tests/f698_surrogate.rs)

fn op3000() -> Operating {
    Operating {
        rpm: 3000.0,
        rho: 1.2,
        nu: 1.5e-5,
    }
}

fn base_cond() -> RomCondition {
    RomCondition {
        op: op3000(),
        system: SystemCurve { k: 1.0e4, l: 0.0 },
        c0: 343.0,
        observer_r: 1.0,
        observer_theta: std::f64::consts::FRAC_PI_2,
        n_scan: 100,
    }
}

/// ADR 0620 set-up F.
fn fan_f() -> ComputerFan {
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
    let fv = free_vortex_design(&base, &m, &op3000(), 0.045, 45.0 / 1.2 / op3000().omega()).unwrap();
    let span = |r: f64| (r - fv.r_hub) / (fv.r_tip - fv.r_hub);
    let cp = |sp: f64, sec: &RomSection| ControlPoint {
        span: sp,
        chord: sec.chord,
        stagger_deg: sec.stagger.to_degrees(),
        camber: sec.camber,
        camber_pos: sec.camber_pos,
        thickness: 0.08,
        sweep: 0.0,
        skew_deg: 0.0,
    };
    let mut st = vec![cp(0.0, &fv.sections[0])];
    for sec in &fv.sections {
        st.push(cp(span(sec.radius), sec));
    }
    st.push(cp(1.0, fv.sections.last().unwrap()));
    ComputerFan {
        n_blades: 7,
        r_hub: 0.030,
        r_tip: 0.058,
        tip_gap: 0.001,
        stations: st,
        ..ComputerFan::default()
    }
}

fn target_f3() -> FanRomTarget {
    let fan = fan_f();
    FanRomTarget {
        geometry: Box::new(move |x: &[f64]| {
            let mut f = fan.clone();
            for s in &mut f.stations {
                s.stagger_deg += x[0];
                s.chord *= x[1];
                s.camber *= x[2];
            }
            Ok(f)
        }),
        tolerances: Tolerances::default(),
        model: RomModel::default(),
        base: base_cond(),
    }
}

const F3_BOX: [(&str, &str, f64, f64); 3] = [
    ("stagger_offset_deg", "deg", -3.0, 3.0),
    ("chord_scale", "1", 0.8, 1.2),
    ("camber_scale", "1", 0.5, 1.5),
];

fn objective_f3() -> FanObjective {
    let pt = |label: &str, k: f64, weight: f64| ImpedancePoint {
        label: label.into(),
        system: SystemCurve { k, l: 0.0 },
        weight,
    };
    FanObjective {
        name: "f698_rom".into(),
        params: F3_BOX
            .iter()
            .map(|(n, u, lo, hi)| {
                (
                    ParamDef {
                        name: n.to_string(),
                        lo: *lo,
                        hi: *hi,
                        init: None,
                        targets: vec![],
                    },
                    u.to_string(),
                )
            })
            .collect(),
        points: vec![pt("R1", 5.0e3, 0.5), pt("R2", 1.5e4, 0.3), pt("R3", 2.5e4, 0.2)],
        terms: vec![Term {
            metric: "power".into(),
            sense: Sense::Min,
            tol: TolStat::Nominal,
            over: Over::Mean,
            weight: 1.0,
        }],
        limits: vec![],
        plan: None,
        workers: 1,
    }
}

fn spec_f3(seed: u64, budget: usize) -> BayesSpec {
    BayesSpec {
        study: "f698_rom".into(),
        params: F3_BOX
            .iter()
            .map(|(n, u, lo, hi)| BayesParam {
                name: n.to_string(),
                unit: u.to_string(),
                lo: *lo,
                hi: *hi,
            })
            .collect(),
        objective: "power@nominal@mean".into(),
        sense: Sense::Min,
        constraints: vec![],
        budget,
        options: BayesOptions {
            seed,
            ..Default::default()
        },
        clock_ms: Some(CLOCK),
    }
}

fn failed_entries_located(entries: &[Entry]) -> bool {
    entries.iter().filter(|e| e.verdict != Verdict::Ok).all(|e| {
        e.verdict == Verdict::Diverged
            && ["R1", "R2", "R3"]
                .iter()
                .any(|p| e.reason.contains(&format!("point '{p}'")))
    })
}

// ---------------------------------------------------------------- helpers

fn pc_f3<'a>(o: &'a FanObjective, tg: &'a FanRomTarget) -> StallPreCheck<'a, FanRomTarget> {
    StallPreCheck {
        objective: o,
        model: tg,
        points: vec!["R3".into()],
    }
}

fn pre_of(m: &dyn Model, scope: PreCheckScope) -> PreCheck<'_> {
    PreCheck {
        model: m,
        scope,
        max_rejected: 400,
        max_per_step: 20,
        progress: None,
    }
}

fn names_r3(e: &Entry) -> bool {
    e.reason.contains("point 'R3'")
}

/// Points outside the box or within 1e-6 (unit cube) of an earlier point.
fn e2_violations(xs: &[Vec<f64>], b: &[(f64, f64)]) -> usize {
    let u: Vec<Vec<f64>> = xs
        .iter()
        .map(|x| x.iter().zip(b).map(|(v, (lo, hi))| (v - lo) / (hi - lo)).collect())
        .collect();
    let mut bad = 0;
    for (i, p) in u.iter().enumerate() {
        let outside = p.iter().any(|v| !(0.0..=1.0).contains(v));
        let close = u[..i]
            .iter()
            .any(|q| q.iter().zip(p).map(|(a, c)| (a - c) * (a - c)).sum::<f64>().sqrt() < 1e-6);
        bad += usize::from(outside || close);
    }
    bad
}

fn f3_box() -> Vec<(f64, f64)> {
    F3_BOX.iter().map(|(_, _, lo, hi)| (*lo, *hi)).collect()
}

/// Every evaluated point of a run in ledger order (pre-checks before the evaluation of their step).
fn all_points(entries: &[Entry]) -> Vec<Vec<f64>> {
    entries
        .iter()
        .filter(|e| e.kind == "evaluation")
        .map(|e| e.params.iter().map(|q| q.value).collect())
        .collect()
}

/// The proposals of a pre-checked run in ledger order (ADR 0645 amendment 1): every evaluation of the objective model and
/// every failed pre-check (a passed pre-check is the same proposal as the evaluation that follows it).
fn proposals(entries: &[Entry], pre_id: &str) -> Vec<Vec<f64>> {
    entries
        .iter()
        .filter(|e| e.kind == "evaluation" && (e.evaluator != pre_id || e.verdict != Verdict::Ok))
        .map(|e| e.params.iter().map(|q| q.value).collect())
        .collect()
}

fn param_bits(e: &Entry) -> Vec<u64> {
    e.params.iter().map(|q| q.value.to_bits()).collect()
}

/// Every evaluated entry follows (in ledger order) a passed pre-check at the same parameters.
fn evaluated_after_passed(entries: &[Entry], pre_id: &str, model_id: &str) -> bool {
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.evaluator == model_id && e.kind == "evaluation")
        .all(|(i, e)| {
            e.provenance["precheck"] == "passed"
                && entries[..i]
                    .iter()
                    .any(|p| p.evaluator == pre_id && p.verdict == Verdict::Ok && param_bits(p) == param_bits(e))
        })
}

struct LedgerCounts {
    pre: usize,
    pre_failed: usize,
    objective: usize,
    fidelity_ok: bool,
    summary_ok: bool,
}

fn counts(entries: &[Entry], r: &PreCheckedResult, pre_id: &str, model_id: &str, fid: Fidelity) -> LedgerCounts {
    let pre: Vec<&Entry> = entries
        .iter()
        .filter(|e| e.kind == "evaluation" && e.evaluator == pre_id)
        .collect();
    let objective = entries
        .iter()
        .filter(|e| e.kind == "evaluation" && e.evaluator == model_id)
        .count();
    let summary_ok = entries.iter().find(|e| e.kind == "surrogate_summary").is_some_and(|s| {
        s.metric("prechecks").map(|q| q.value) == Some(r.prechecks.len() as f64)
            && s.metric("precheck_failed").map(|q| q.value) == Some(r.rejected as f64)
    });
    LedgerCounts {
        pre: pre.len(),
        pre_failed: pre.iter().filter(|e| e.verdict != Verdict::Ok).count(),
        objective,
        fidelity_ok: pre.iter().all(|e| e.fidelity == fid),
        summary_ok,
    }
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("f716");
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join(name);
    let _ = std::fs::remove_file(&p);
    p
}

fn evaluation_lines(p: &PathBuf) -> Vec<String> {
    std::fs::read_to_string(p)
        .unwrap()
        .lines()
        .filter(|l| l.contains("\"kind\":\"evaluation\""))
        .map(str::to_string)
        .collect()
}

// ---------------------------------------------------------------- BH: Branin with a failure disc

fn branin(x: &[f64]) -> f64 {
    let (b, c, t) = (5.1 / (4.0 * PI * PI), 5.0 / PI, 1.0 / (8.0 * PI));
    (x[1] - b * x[0] * x[0] + c * x[0] - 6.0).powi(2) + 10.0 * (1.0 - t) * x[0].cos() + 10.0
}

/// The F1 disc of ADR 0621: unit-cube radius 0.15 around (pi, 2.275).
fn in_hole(x: &[f64]) -> bool {
    let u = [(x[0] + 5.0) / 15.0, x[1] / 15.0];
    let c = [(PI + 5.0) / 15.0, 2.275 / 15.0];
    ((u[0] - c[0]).powi(2) + (u[1] - c[1]).powi(2)).sqrt() < 0.15
}

fn hole_outcome() -> Outcome {
    Outcome {
        verdict: Verdict::Diverged,
        reason: "point 'hole': inside the failure disc".into(),
        ..Default::default()
    }
}

/// The F-698 L1 objective (no disc).
fn branin_model() -> FnModel<impl Fn(&[Quantity]) -> Outcome + Sync> {
    FnModel {
        id: "test.branin".into(),
        version: "1".into(),
        fidelity: Fidelity::Analytic,
        f: |p: &[Quantity]| Outcome::ok(vec![Quantity::new("f", branin(&[p[0].value, p[1].value]), "1")]),
    }
}

/// BH: Branin failing inside the disc.
fn bh_model() -> FnModel<impl Fn(&[Quantity]) -> Outcome + Sync> {
    FnModel {
        id: "test.branin".into(),
        version: "1".into(),
        fidelity: Fidelity::Analytic,
        f: |p: &[Quantity]| {
            let x = [p[0].value, p[1].value];
            if in_hole(&x) {
                hole_outcome()
            } else {
                Outcome::ok(vec![Quantity::new("f", branin(&x), "1")])
            }
        },
    }
}

fn fn_pre(id: &str, pass: fn(&[f64]) -> bool) -> FnModel<impl Fn(&[Quantity]) -> Outcome + Sync> {
    FnModel {
        id: id.into(),
        version: "1".into(),
        fidelity: Fidelity::Analytic,
        f: move |p: &[Quantity]| {
            if pass(&[p[0].value, p[1].value]) {
                Outcome::ok(vec![Quantity::new("ok", 1.0, "1")])
            } else {
                hole_outcome()
            }
        },
    }
}

fn branin_spec(budget: usize, seed: u64) -> BayesSpec {
    BayesSpec {
        study: "l1".into(),
        params: vec![
            BayesParam {
                name: "x1".into(),
                unit: "1".into(),
                lo: -5.0,
                hi: 10.0,
            },
            BayesParam {
                name: "x2".into(),
                unit: "1".into(),
                lo: 0.0,
                hi: 15.0,
            },
        ],
        objective: "f".into(),
        sense: Sense::Min,
        constraints: vec![],
        budget,
        options: BayesOptions {
            seed,
            n0: Some(10),
            ..Default::default()
        },
        clock_ms: Some(CLOCK),
    }
}

fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// ---------------------------------------------------------------- B1: default bits

/// The digest of the same two runs at d40cd665 (this function on a `git archive` of d40cd665; ADR 0645 addendum 1).
const B1_BASE_DIGEST: u64 = 0xbe4b_c4aa_0063_bc33;

fn b1_digest() -> u64 {
    let m = branin_model();
    let p = tmp("b1.jsonl");
    let led = Ledger::new(&p);
    bayes::run(&branin_spec(25, 698_901), &m, Some(&led)).unwrap();
    let mut text = std::fs::read_to_string(&p).unwrap();
    let tg = target_f3();
    let o = objective_f3();
    let fm = ObjectiveModel {
        objective: &o,
        model: &tg,
    };
    let r = bayes::run(&spec_f3(1, 18), &fm, None).unwrap();
    for e in r.entries.iter().chain(std::iter::once(&r.summary)) {
        text.push_str(&format!("{e:?}\n"));
    }
    for k in &r.picks {
        text.push_str(&format!(
            "{:?} {:?}\n",
            k.x.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            k.kind
        ));
    }
    text.push_str(&format!(
        "{:?} {:?} {}\n",
        r.best,
        r.best_value.map(f64::to_bits),
        r.failed
    ));
    fnv(text.as_bytes())
}

#[test]
fn b1_default_run_bits() {
    let d = b1_digest();
    println!("B1 digest {d:016x}, base {B1_BASE_DIGEST:016x}");
    assert_eq!(d, B1_BASE_DIGEST, "B1");
}

// ---------------------------------------------------------------- F: fast paths

#[test]
#[ignore = "record of the first registered run (ADR 0645 amendment 1): E2 over every evaluation line counts each evaluation against its own passed pre-check; measured 18 violations"]
fn f1_fan_fast_path_defective_e2_record() {
    let tg = target_f3();
    let o = objective_f3();
    let m = ObjectiveModel {
        objective: &o,
        model: &tg,
    };
    let pc = pc_f3(&o, &tg);
    let p = tmp("f1.jsonl");
    let led = Ledger::new(&p);
    let r = bayes::run_prechecked(&spec_f3(1, 18), &m, &pre_of(&pc, PreCheckScope::Every), Some(&led)).unwrap();
    let entries = led.read().unwrap().entries;
    let c = counts(&entries, &r, STALL_PRECHECK_ID, m.id(), Fidelity::ReducedOrder);
    let rejected: Vec<&Entry> = r.prechecks.iter().filter(|e| e.verdict != Verdict::Ok).collect();
    let pre_located = rejected.iter().all(|e| names_r3(e));
    let exact = rejected.iter().all(|e| {
        let out = m.evaluate(&e.params);
        out.verdict != Verdict::Ok && out.reason.contains("point 'R3'")
    });
    let located = failed_entries_located(&r.run.entries);
    let e2 = e2_violations(&all_points(&entries), &f3_box());
    let after = evaluated_after_passed(&entries, STALL_PRECHECK_ID, m.id());
    println!(
        "F1: {} evaluations, failed {}, pre-checks {} (ledger {}), rejected {} (ledger {}), objective entries {}, \
         fidelity {}, summary {}, pre-check reasons R3 {pre_located}, exact {exact}, located {located}, E2 {e2}, \
         after passed {after}; best {:?}",
        r.run.entries.len(),
        r.run.failed,
        r.prechecks.len(),
        c.pre,
        r.rejected,
        c.pre_failed,
        c.objective,
        c.fidelity_ok,
        c.summary_ok,
        r.run.best_value
    );
    assert_eq!(r.run.entries.len(), 18, "F1 evaluations");
    assert!(
        c.pre == r.prechecks.len() && c.pre_failed == r.rejected && c.objective == 18,
        "F1 ledger counts"
    );
    assert!(c.fidelity_ok && c.summary_ok, "F1 ledger fields");
    assert!(pre_located && exact && located, "F1 A4 A5");
    assert_eq!(e2, 0, "F1 E2");
    assert!(after, "F1 evaluated only after a passed pre-check");
}

#[test]
fn f1b_fan_fast_path() {
    let tg = target_f3();
    let o = objective_f3();
    let m = ObjectiveModel {
        objective: &o,
        model: &tg,
    };
    let pc = pc_f3(&o, &tg);
    let p = tmp("f1.jsonl");
    let led = Ledger::new(&p);
    let r = bayes::run_prechecked(&spec_f3(1, 18), &m, &pre_of(&pc, PreCheckScope::Every), Some(&led)).unwrap();
    let entries = led.read().unwrap().entries;
    let c = counts(&entries, &r, STALL_PRECHECK_ID, m.id(), Fidelity::ReducedOrder);
    let rejected: Vec<&Entry> = r.prechecks.iter().filter(|e| e.verdict != Verdict::Ok).collect();
    let pre_located = rejected.iter().all(|e| names_r3(e));
    let exact = rejected.iter().all(|e| {
        let out = m.evaluate(&e.params);
        out.verdict != Verdict::Ok && out.reason.contains("point 'R3'")
    });
    let located = failed_entries_located(&r.run.entries);
    let e2 = e2_violations(&proposals(&entries, STALL_PRECHECK_ID), &f3_box());
    let after = evaluated_after_passed(&entries, STALL_PRECHECK_ID, m.id());
    println!(
        "F1b: {} evaluations, failed {}, pre-checks {} (ledger {}), rejected {} (ledger {}), objective entries {}, \
         fidelity {}, summary {}, pre-check reasons R3 {pre_located}, exact {exact}, located {located}, E2 {e2}, \
         after passed {after}; best {:?}",
        r.run.entries.len(),
        r.run.failed,
        r.prechecks.len(),
        c.pre,
        r.rejected,
        c.pre_failed,
        c.objective,
        c.fidelity_ok,
        c.summary_ok,
        r.run.best_value
    );
    assert_eq!(r.run.entries.len(), 18, "F1b evaluations");
    assert!(
        c.pre == r.prechecks.len() && c.pre_failed == r.rejected && c.objective == 18,
        "F1b ledger counts"
    );
    assert!(c.fidelity_ok && c.summary_ok, "F1b ledger fields");
    assert!(pre_located && exact && located, "F1b A4 A5");
    assert_eq!(e2, 0, "F1b E2");
    assert!(after, "F1b evaluated only after a passed pre-check");
}

fn entry_key(e: &Entry) -> (String, Vec<u64>, Vec<(String, u64)>, Verdict, String) {
    (
        e.variant.clone(),
        param_bits(e),
        e.metrics.iter().map(|q| (q.name.clone(), q.value.to_bits())).collect(),
        e.verdict,
        e.reason.clone(),
    )
}

#[test]
fn f2_pass_through_equals_run() {
    let m = bh_model();
    let pass = fn_pre("test.pass", |_| true);
    let spec = branin_spec(25, 698_901);
    let a = bayes::run(&spec, &m, None).unwrap();
    let b = bayes::run_prechecked(&spec, &m, &pre_of(&pass, PreCheckScope::Every), None).unwrap();
    let eq = a.entries.len() == b.run.entries.len()
        && a.entries
            .iter()
            .zip(&b.run.entries)
            .all(|(x, y)| entry_key(x) == entry_key(y));
    println!(
        "F2: entries equal {eq}; pre-checks {}, rejected {}",
        b.prechecks.len(),
        b.rejected
    );
    assert!(eq, "F2 equal");
    assert_eq!((b.prechecks.len(), b.rejected), (25, 0), "F2 counts");
}

#[test]
fn f3c_cap_refuses() {
    let m = bh_model();
    let never = fn_pre("test.never", |_| false);
    let p = tmp("f3c.jsonl");
    let led = Ledger::new(&p);
    let pre = PreCheck {
        max_rejected: 5,
        ..pre_of(&never, PreCheckScope::Every)
    };
    let r = bayes::run_prechecked(&branin_spec(25, 1), &m, &pre, Some(&led));
    let entries = led.read().unwrap().entries;
    let n_pre = entries.iter().filter(|e| e.evaluator == "test.never").count();
    let n_obj = entries.iter().filter(|e| e.evaluator == "test.branin").count();
    println!(
        "F3c: {:?}; ledger pre-checks {n_pre}, objective {n_obj}",
        r.as_ref().err()
    );
    assert!(r.is_err_and(|e| e.contains("max_rejected = 5")), "F3c refusal");
    assert_eq!((n_pre, n_obj), (6, 0), "F3c ledger");
}

#[test]
fn f4_refusals() {
    let m = bh_model();
    let same = fn_pre("test.branin", |_| true);
    let pass = fn_pre("test.pass", |_| true);
    let spec = branin_spec(5, 1);
    let a = bayes::run_prechecked(&spec, &m, &pre_of(&same, PreCheckScope::Every), None);
    let b = bayes::run_prechecked(
        &spec,
        &m,
        &PreCheck {
            max_per_step: 0,
            ..pre_of(&pass, PreCheckScope::Every)
        },
        None,
    );
    println!("F4: {:?}; {:?}", a.as_ref().err(), b.as_ref().err());
    assert!(a.is_err_and(|e| e.contains("test.branin")), "F4 id");
    assert!(b.is_err_and(|e| e.contains("max_per_step")), "F4 per step");
}

#[test]
fn f5_initial_scope() {
    let m = bh_model();
    let hole = fn_pre("test.hole", |x| !in_hole(x));
    let r = bayes::run_prechecked(&branin_spec(25, 1), &m, &pre_of(&hole, PreCheckScope::Initial), None).unwrap();
    let all_free = r
        .prechecks
        .iter()
        .all(|e| matches!(e.provenance["acquisition"].as_str(), Some("initial" | "filling")));
    // the model-free picks tried: the evaluated ones and the rejected ones
    let tried = r.run.picks.iter().filter(|k| k.kind.is_model_free()).count() + r.rejected;
    println!(
        "F5: pre-checks {}, all model-free {all_free}, model-free picks tried {tried}, rejected {}",
        r.prechecks.len(),
        r.rejected
    );
    assert!(all_free && r.prechecks.len() == tried, "F5");
}

// ---------------------------------------------------------------- L1, D1

fn bh_prechecked(budget: usize, workers: usize, led: Option<&Ledger>) -> PreCheckedResult {
    let m = bh_model();
    let hole = fn_pre("test.hole", |x| !in_hole(x));
    let mut spec = branin_spec(budget, 1);
    spec.options.workers = workers;
    bayes::run_prechecked(&spec, &m, &pre_of(&hole, PreCheckScope::Every), led).unwrap()
}

#[test]
fn l1_d1_resume_and_workers() {
    let p = tmp("l1.jsonl");
    let led = Ledger::new(&p);
    let r1 = bh_prechecked(25, 1, Some(&led));
    let p2 = tmp("l1_resume.jsonl");
    let led2 = Ledger::new(&p2);
    bh_prechecked(15, 1, Some(&led2));
    bh_prechecked(10, 1, Some(&led2));
    let resume_eq = evaluation_lines(&p) == evaluation_lines(&p2);
    let r8 = bh_prechecked(25, 8, None);
    let key = |r: &PreCheckedResult| {
        r.run
            .entries
            .iter()
            .chain(&r.prechecks)
            .map(|e| (entry_key(e), e.provenance.to_string()))
            .collect::<Vec<_>>()
    };
    let d1 = key(&r1) == key(&r8);
    println!(
        "L1: resume lines equal {resume_eq} ({} lines, rejected {}); D1 workers 1 and 8 equal {d1}",
        evaluation_lines(&p).len(),
        r1.rejected
    );
    assert!(resume_eq, "L1");
    assert!(d1, "D1");
}

// ---------------------------------------------------------------- heavy record: A1 to A6, NC1, NC2

type CaseOut = (Result<PreCheckedResult, String>, Vec<Entry>);

#[test]
#[ignore = "heavy record (ADR 0645 A1 to A6, NC1, NC2), run once: A1 to A6 met; NC1 missed on seed 4 (best / P_ref 0.99351 against the registered [0.9930, 0.9935]; every seed equal to the F-698 log), see ADR 0645 addendum 1"]
fn a_reduced_order_fan_with_stall_precheck_record_nc1_known_miss() {
    let t = Instant::now();
    let tg = target_f3();
    let o = objective_f3();
    let m = ObjectiveModel {
        objective: &o,
        model: &tg,
    };
    let pc = pc_f3(&o, &tg);
    let progress = |line: &str| println!("progress: {line}");
    // case 0: A (scope Every, a ledger file each), 1: NC1 (pre-check off), 2: NC2 (scope Initial)
    let cases: Vec<(u64, usize)> = (0..3).flat_map(|c| (1..=10u64).map(move |s| (s, c))).collect();
    let res: Vec<CaseOut> = parallel_map(&cases, 30, &|&(s, c): &(u64, usize)| -> CaseOut {
        let spec = spec_f3(s, 50);
        match c {
            0 => {
                let p = tmp(&format!("a_seed{s}.jsonl"));
                let led = Ledger::new(&p);
                let pre = PreCheck {
                    progress: Some(&progress),
                    ..pre_of(&pc, PreCheckScope::Every)
                };
                let r = bayes::run_prechecked(&spec, &m, &pre, Some(&led));
                (r, led.read().unwrap().entries)
            }
            1 => {
                let r = bayes::run(&spec, &m, None).map(|run| PreCheckedResult {
                    run,
                    prechecks: Vec::new(),
                    rejected: 0,
                });
                (r, Vec::new())
            }
            _ => {
                let pre = PreCheck {
                    progress: Some(&progress),
                    ..pre_of(&pc, PreCheckScope::Initial)
                };
                (bayes::run_prechecked(&spec, &m, &pre, None), Vec::new())
            }
        }
    });
    // the F-698 record (f698-scratch/run_r1r2.log, ADR 0621 addendum 1)
    let f698_failed = [37, 35, 34, 36, 36, 36, 39, 36, 34, 36];
    let f698_ratio = [
        0.99328, 0.99325, 0.99304, 0.99351, 0.99317, 0.99345, 0.99328, 0.99328, 0.99306, 0.99311,
    ];
    let (mut a1, mut a2, mut a3, mut a4, mut a5, mut a6, mut nc1) = (true, 0, true, true, true, 0, true);
    let mut rom_points = Vec::new();
    for ((s, c), (r, entries)) in cases.iter().zip(&res) {
        let name = ["A", "NC1", "NC2"][*c];
        let r = match r {
            Ok(r) => r,
            Err(e) => {
                println!("{name} seed {s}: refused: {e}");
                match c {
                    0 => a1 = false,
                    1 => nc1 = false,
                    _ => {}
                }
                continue;
            }
        };
        let best = r.run.best_value.unwrap_or(f64::INFINITY);
        let ratio = best / P_REF;
        let at = r
            .run
            .entries
            .iter()
            .position(|e| e.verdict == Verdict::Ok && e.metric("power@nominal@mean").unwrap().value <= P_REF * 1.01)
            .map(|i| i + 1);
        let mut fails_at = std::collections::BTreeMap::new();
        for e in r.run.entries.iter().filter(|e| e.verdict != Verdict::Ok) {
            let p = ["R1", "R2", "R3"]
                .iter()
                .find(|p| e.reason.contains(&format!("point '{p}'")))
                .map_or("other", |p| *p);
            *fails_at.entry(p).or_insert(0) += 1;
        }
        println!(
            "{name} seed {s}: failed {} of {} {fails_at:?}, best {best:.6} W (ratio {ratio:.5}), target at {at:?}, \
             pre-checks {}, rejected {}",
            r.run.failed,
            r.run.entries.len(),
            r.prechecks.len(),
            r.rejected
        );
        match c {
            0 => {
                a1 &= r.run.failed <= 14 && r.run.entries.len() == 50;
                a2 += usize::from(best <= P_REF * 1.01);
                let cn = counts(entries, r, STALL_PRECHECK_ID, m.id(), Fidelity::ReducedOrder);
                let ok3 = cn.pre == r.prechecks.len()
                    && cn.pre_failed == r.rejected
                    && cn.objective == 50
                    && cn.fidelity_ok
                    && cn.summary_ok
                    && entries
                        .iter()
                        .filter(|e| e.evaluator == STALL_PRECHECK_ID)
                        .all(|e| e.study == "f698_rom" && e.kind == "evaluation");
                a3 &= ok3;
                rom_points.push(r.prechecks.len() + 3 * 50);
                let rej: Vec<&Entry> = r.prechecks.iter().filter(|e| e.verdict != Verdict::Ok).collect();
                a4 &= failed_entries_located(&r.run.entries) && rej.iter().all(|e| names_r3(e));
                let exact = parallel_map(&rej, 4, &|e: &&Entry| {
                    let out = m.evaluate(&e.params);
                    out.verdict != Verdict::Ok && out.reason.contains("point 'R3'")
                });
                a5 &= exact.iter().all(|v| *v);
                let v6 = e2_violations(&proposals(entries, STALL_PRECHECK_ID), &f3_box());
                a6 += v6;
                println!(
                    "A seed {s}: ledger counts {ok3} (pre-checks {}, failed {}, objective {}), ROM operating points {}, \
                     exact {}/{}, E2 {v6}",
                    cn.pre,
                    cn.pre_failed,
                    cn.objective,
                    r.prechecks.len() + 150,
                    exact.iter().filter(|v| **v).count(),
                    exact.len()
                );
            }
            1 => {
                let i = (*s - 1) as usize;
                let ok = (34..=39).contains(&r.run.failed) && (0.9930..=0.9935).contains(&ratio);
                nc1 &= ok;
                println!(
                    "NC1 seed {s}: within the ADR 0621 ranges {ok}; F-698 log failed {} ratio {:.5}, same {}",
                    f698_failed[i],
                    f698_ratio[i],
                    r.run.failed == f698_failed[i] && (ratio - f698_ratio[i]).abs() < 6e-6
                );
            }
            _ => {}
        }
    }
    println!(
        "A1 {a1}; A2 {a2} of 10 (limit 9); A3 {a3}, ROM operating points {rom_points:?}; A4 {a4}; A5 {a5}; A6 {a6}; \
         NC1 {nc1}; {:.0} s",
        t.elapsed().as_secs_f64()
    );
    assert!(nc1, "NC1");
    assert!(a1, "A1");
    assert!(a2 >= 9, "A2");
    assert!(a3, "A3");
    assert!(a4 && a5, "A4 A5");
    assert_eq!(a6, 0, "A6");
}
