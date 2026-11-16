//! ADR 0670: checks of the held-tolerance floor over a registered seeded sample of the design (F-749). Synthetic evaluators with
//! closed forms only; no CFD, no person, no language model.

use rcfd_api::design::hypothesis::*;
use rcfd_api::design::ledger::{Entry, Fidelity, Ledger, Model, Outcome, Quantity};
use rcfd_core::hash::fnv64;
use serde_json::Value;
use std::path::{Path, PathBuf};

const CLOCK: u64 = 1_700_000_000_000;
const TOL_Q: f64 = 1e-6;
const TOL_N: f64 = 1e-5;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f749_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ledger_at(p: &Path) -> Ledger {
    let mut l = Ledger::new(p);
    l.clock = || CLOCK;
    l
}

fn entries(l: &Ledger) -> Vec<Entry> {
    let c = l.read().unwrap();
    assert!(c.skipped.is_empty(), "skipped lines: {:?}", c.skipped);
    c.entries
}

fn lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

fn write_lines(path: &Path, ls: &[String]) {
    std::fs::write(path, ls.iter().map(|s| format!("{s}\n")).collect::<String>()).unwrap();
}

fn read_bytes(p: &Path) -> Option<Vec<u8>> {
    std::fs::read(p).ok()
}

fn val(p: &[Quantity], n: &str) -> f64 {
    p.iter().find(|q| q.name == n).map_or(f64::NAN, |q| q.value)
}

fn rel(a: f64, b: f64) -> f64 {
    ((a - b) / b).abs()
}

// ---------------------------------------------------------------- WC and WQ (the smoke's evaluator, verbatim)

struct Wf {
    quad: bool,
}

const WC: Wf = Wf { quad: false };
const WQ: Wf = Wf { quad: true };

impl Model for Wf {
    fn id(&self) -> &str {
        if self.quad {
            "synthetic.wq"
        } else {
            "synthetic.wc"
        }
    }
    fn version(&self) -> &str {
        "1"
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::Analytic
    }
    fn evaluate(&self, p: &[Quantity]) -> Outcome {
        let (s, c, a, b) = (val(p, "s"), val(p, "c"), val(p, "a"), val(p, "b"));
        let gg = if self.quad { 1.0 + 3.0 * c * c } else { 1.0 };
        let (aa, bb, cc) = (1.0 + 0.5 * s, 1.0 + 0.3 * s, 1.0 + s);
        let n = a * bb / (gg * (1.0 + 0.2 * b));
        let q = b * n * aa;
        let y = a * b * cc;
        Outcome::ok(vec![
            Quantity::new("q", q, "m3/s"),
            Quantity::new("n", n, "1/s"),
            Quantity::new("power", y, "W"),
        ])
    }
}

fn g_of(m: &Wf, c: f64) -> f64 {
    if m.quad {
        1.0 + 3.0 * c * c
    } else {
        1.0
    }
}

/// Closed-form sensitivities (S_q, S_n) of the power along the hold at (s, c) and held values (q, n) (ADR 0665 with G(c)).
fn sens(m: &Wf, s: f64, c: f64, q: f64, n: f64) -> (f64, f64) {
    let (aa, bb, cc) = (1.0 + 0.5 * s, 1.0 + 0.3 * s, 1.0 + s);
    let k = g_of(m, c) * cc / bb;
    (
        k * (1.0 / aa + 0.4 * q / (n * aa * aa)),
        -k * 0.2 * q * q / (n * n * aa * aa),
    )
}

fn phi(m: &Wf, s: f64, c: f64, q: f64, n: f64) -> f64 {
    let (sq, sn) = sens(m, s, c, q, n);
    sq.abs() * TOL_Q + sn.abs() * TOL_N
}

/// Closed-form floor of a point with value targets (1, 50).
fn f_value(m: &Wf, c: f64) -> f64 {
    phi(m, 0.0, c, 1.0, 50.0) + phi(m, 1.0, c, 1.0, 50.0)
}

/// Closed-form floor of a point with as_low_arm targets (b at 0.02, a at 50): phi of the high arm at the low arm's (q, n).
fn f_aslow(m: &Wf, c: f64) -> f64 {
    let n_l = 50.0 / (g_of(m, c) * (1.0 + 0.2 * 0.02));
    let q_l = 0.02 * n_l;
    phi(m, 1.0, c, q_l, n_l)
}

fn hq(t: HeldTarget) -> Held {
    Held {
        metric: "q".into(),
        unit: "m3/s".into(),
        target: t,
        tolerance: TOL_Q,
        by: "b".into(),
        by_unit: "1".into(),
        bracket: (0.001, 0.1),
    }
}

fn hn(t: HeldTarget, br: (f64, f64)) -> Held {
    Held {
        metric: "n".into(),
        unit: "1/s".into(),
        target: t,
        tolerance: TOL_N,
        by: "a".into(),
        by_unit: "V".into(),
        bracket: br,
    }
}

fn values(br: (f64, f64)) -> Vec<Held> {
    vec![hq(HeldTarget::Value(1.0)), hn(HeldTarget::Value(50.0), br)]
}

fn aslow(br: (f64, f64)) -> Vec<Held> {
    vec![
        hq(HeldTarget::AsLowArm { at: 0.02 }),
        hn(HeldTarget::AsLowArm { at: 50.0 }, br),
    ]
}

fn claim(m: &Wf, seed: u64, min_effect: f64, held: Vec<Held>, sample: Option<usize>) -> Hypothesis {
    let mut h = Hypothesis::new(
        "raising s raises the power at equal flow and speed",
        Factor {
            name: "s".into(),
            unit: "1".into(),
            low: 0.0,
            high: 1.0,
        },
        Response {
            name: "power".into(),
            unit: "W".into(),
            resolution: 0.0,
        },
        Direction::Increases,
        min_effect,
        vec![Bound::new("c", "1", 0.0, 1.0)],
        32,
        seed,
        Scope::of(m, true),
    );
    h.held = held;
    h.floor_sample = sample.map(|points| FloorSample { points });
    h
}

fn nums(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()
}

fn idx(v: &Value) -> Vec<usize> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_u64().unwrap() as usize)
        .collect()
}

// ---------------------------------------------------------------- P0

#[test]
fn p0_midpoint_registrations_keep_their_bytes() {
    let dir = tmp("p0");
    let cases: [(&str, &Wf, Hypothesis, usize, usize, &str, &str); 3] = [
        (
            "wc_value",
            &WC,
            claim(&WC, 749_900, 0.01, values((10.0, 100.0)), None),
            66,
            63350,
            "07ff3427fa1da108",
            "fnv1a64:54561c0a1e6d09c4",
        ),
        (
            "wq_value",
            &WQ,
            claim(&WQ, 749_901, 0.01, values((10.0, 250.0)), None),
            66,
            63469,
            "0de2dc24545db9a1",
            "fnv1a64:c9810d4e57d4ccd3",
        ),
        (
            "wc_aslow",
            &WC,
            claim(&WC, 749_902, 0.01, aslow((10.0, 100.0)), None),
            66,
            61909,
            "60277d546d014ba1",
            "fnv1a64:166ddb3ab9838223",
        ),
    ];
    let mut ok = 0;
    for (tag, m, h, n_e, n_b, fnv, rh) in cases {
        let p = dir.join(format!("{tag}.jsonl"));
        let l = ledger_at(&p);
        let r = register_with_floor(&l, &h, m).unwrap();
        run(
            &l,
            &r.id,
            m,
            &RunOptions {
                workers: 1,
                ..RunOptions::default()
            },
        )
        .unwrap();
        let bytes = std::fs::read(&p).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        let n = entries(&l).len();
        let got = format!("{:016x}", fnv64(&bytes));
        let no_key = !text.contains("\"floor_sample\"") && !text.contains("\"sample\"");
        let none = held_floor(&h, m).unwrap().sample.is_none();
        eprintln!(
            "P0 {tag}: {n} entries, {} bytes, fnv {got}, record hash {}; no new key {no_key}; sample None {none}",
            bytes.len(),
            r.hash
        );
        if n == n_e && bytes.len() == n_b && got == fnv && r.hash == rh && no_key && none {
            ok += 1;
        }
    }
    assert_eq!(ok, 3, "P0");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- P1, P2

#[test]
fn p1_p2_sample_floor_against_the_closed_form() {
    let dir = tmp("p1");
    let path = dir.join("l.jsonl");
    let h = claim(&WQ, 749_001, 0.01, values((10.0, 250.0)), Some(24));
    let l = ledger_at(&path);
    let r = register_with_floor(&l, &h, &WQ).unwrap();
    let es = entries(&l);
    let st = &es[0].settings["held_floor"];
    let sm = &st["sample"];
    let hash = es[0].settings["hash"].as_str().unwrap().to_string();
    assert_eq!(hash, h.hash());
    // (a) the pairs
    let fseed = fnv64(format!("{hash}:floor:749001").as_bytes());
    let want = rerun_pairs(32, 24, fseed);
    let pairs = idx(&sm["pairs"]);
    let a_ok = pairs == want && sm["seed"].as_u64() == Some(fseed) && sm["points"].as_u64() == Some(24);
    // (b) each per-pair value
    let pts = design_points(&h, derived_seed(&hash, 749_001));
    let vals = nums(&sm["values"]);
    let (mut b_ok, mut b_worst) = (0, 0.0f64);
    let (mut cmax, mut cprobe) = (f_value(&WQ, 0.5), vec![0.5]);
    for (k, &i) in pairs.iter().enumerate() {
        let c = pts[i][0];
        let fc = f_value(&WQ, c);
        let e = rel(vals[k], fc);
        b_worst = b_worst.max(e);
        if e <= 1e-6 {
            b_ok += 1;
        }
        if fc > cmax {
            cmax = fc;
            cprobe = pts[i].clone();
        }
    }
    // (c) the midpoint
    let mid = sm["midpoint"].as_f64().unwrap();
    let c_err = rel(mid, f_value(&WQ, 0.5));
    // (d) the value and its probe
    let value = st["value"].as_f64().unwrap();
    let d_err = rel(value, cmax);
    let probe = nums(&st["probe"]);
    let probe_ok = probe.len() == cprobe.len() && probe.iter().zip(&cprobe).all(|(a, b)| a.to_bits() == b.to_bits());
    // (e) held_floor bit for bit
    let f = held_floor(&h, &WQ).unwrap();
    let e_ok = f.value.to_bits() == value.to_bits();
    eprintln!(
        "P1: pairs {pairs:?} (a) {a_ok}; (b) {b_ok} of 24 within 1e-6 (worst {b_worst:.2e}); (c) midpoint {mid:.12e} closed {:.12e} \
         rel {c_err:.2e}; (d) value {value:.12e} closed max {cmax:.12e} rel {d_err:.2e}, probe {probe:?} closed {cprobe:?} \
         {probe_ok}, largest {}; (e) {e_ok}",
        f_value(&WQ, 0.5),
        sm["largest"]
    );
    // run and verify
    let rep = run(
        &l,
        &r.id,
        &WQ,
        &RunOptions {
            workers: 1,
            ..RunOptions::default()
        },
    )
    .unwrap();
    let v = verify(&entries(&l), "H-1", Some(&WQ));
    let fin = v.result.as_ref().map(|x| x.finding);
    eprintln!(
        "P1 run: {} evidence; finding {:?}; mean {:?}; problems {:?}; genuine {:?}; evidence_rerun {:?}",
        rep.appended_evidence,
        fin,
        v.result.as_ref().map(|x| x.stats.mean),
        v.problems,
        v.genuine,
        v.evidence_rerun
    );
    // P2
    let ratio = value / mid;
    let at1 = held_floor_at(&h, &WQ, &[1.0]).unwrap();
    let at1_err = rel(at1.value, f_value(&WQ, 1.0));
    let cratio = f_value(&WQ, 1.0) / f_value(&WQ, 0.5);
    eprintln!(
        "P2: (a) sample / midpoint {ratio:.6} (limit 1.5); (b) held_floor_at(1) {:.12e} closed 4 F_WC {:.12e} rel {at1_err:.2e} \
         (4 F_WC with F_WC {:.12e}); (c) closed ratio {cratio:.6} (limit 2)",
        at1.value,
        f_value(&WQ, 1.0),
        f_value(&WC, 0.5)
    );
    assert!(a_ok, "P1 (a)");
    assert_eq!(b_ok, 24, "P1 (b)");
    assert!(c_err <= 1e-6, "P1 (c)");
    assert!(d_err <= 1e-6 && probe_ok, "P1 (d)");
    assert!(e_ok, "P1 (e)");
    assert_eq!(fin, Some(Finding::Confirmed), "P1 run");
    assert!(v.problems.is_empty() && v.genuine.is_some(), "P1 verify");
    assert!(ratio >= 1.5, "P2 (a)");
    assert!(at1_err <= 1e-6, "P2 (b)");
    assert!(cratio >= 2.0, "P2 (c)");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- P3

#[test]
fn p3_refused_below_the_sample_floor() {
    let dir = tmp("p3");
    let path = dir.join("l.jsonl");
    let l = ledger_at(&path);
    let h = claim(&WQ, 749_003, 5e-5, values((10.0, 250.0)), Some(24));
    let before = read_bytes(&path);
    let err = register_with_floor(&l, &h, &WQ).unwrap_err();
    let unchanged = read_bytes(&path) == before;
    let f = held_floor(&h, &WQ).unwrap();
    let sm = f.sample.clone().unwrap();
    let pts = design_points(&h, derived_seed(&h.hash(), h.seed));
    let cmax = sm
        .pairs
        .iter()
        .map(|&i| f_value(&WQ, pts[i][0]))
        .fold(f_value(&WQ, 0.5), f64::max);
    let named = ["min_effect", "0.00005", "held-tolerance floor", "sampled design pairs"]
        .iter()
        .all(|w| err.contains(w))
        && err.contains(&format!("{}", f.value))
        && err.contains(&format!("{}", sm.midpoint));
    let between = 10.0 * sm.midpoint < 5e-5 && 10.0 * f.value > 5e-5;
    let r_err = rel(f.value, cmax);
    let mut hm = h.clone();
    hm.floor_sample = None;
    let acc = register_with_floor(&l, &hm, &WQ);
    eprintln!(
        "P3: refused: {err}\n  named {named}; 10 F_mid {:.6e} < 5e-5 < 10 F_s {:.6e}: {between}; F_s closed {cmax:.12e} rel \
         {r_err:.2e}; file unchanged {unchanged}; midpoint registration {:?}",
        10.0 * sm.midpoint,
        10.0 * f.value,
        acc.as_ref().map(|r| (r.id.clone(), r.slot))
    );
    assert!(named, "P3 message");
    assert!(between, "P3 between");
    assert!(r_err <= 1e-6, "P3 closed form");
    assert!(unchanged, "P3 file");
    let acc = acc.unwrap();
    assert_eq!(acc.slot, 1, "P3 midpoint accepted");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- P4

#[test]
fn p4_as_low_arm_sample() {
    let h = claim(&WQ, 749_004, 0.01, aslow((10.0, 250.0)), Some(8));
    let f = held_floor(&h, &WQ).unwrap();
    let sm = f.sample.clone().unwrap();
    let pts = design_points(&h, derived_seed(&h.hash(), h.seed));
    let mut ok = 0;
    let mut worst = 0.0f64;
    for (k, &i) in sm.pairs.iter().enumerate() {
        let at = held_floor_at(&h, &WQ, &pts[i]).unwrap();
        let one_high = at.arms.len() == 1 && at.arms[0].arm == Arm::High;
        let e = rel(sm.values[k], f_aslow(&WQ, pts[i][0]));
        worst = worst.max(e);
        if one_high && at.value.to_bits() == sm.values[k].to_bits() && e <= 1e-6 {
            ok += 1;
        }
    }
    let mid_err = rel(sm.midpoint, f_aslow(&WQ, 0.5));
    let top = f.arms.len() == 1 && f.arms[0].arm == Arm::High;
    eprintln!(
        "P4: pairs {:?}; {ok} of 8 (one arm high, the values within 1e-6, worst {worst:.2e}); midpoint {:.12e} closed {:.12e} \
         rel {mid_err:.2e}; value {:.12e}; top one arm high {top}",
        sm.pairs,
        sm.midpoint,
        f_aslow(&WQ, 0.5),
        f.value
    );
    assert_eq!(ok, 8, "P4 points");
    assert!(mid_err <= 1e-6 && top, "P4 midpoint");
}

// ---------------------------------------------------------------- P5

#[test]
fn p5_no_variation_no_inflation() {
    let h = claim(&WC, 749_005, 0.01, values((10.0, 100.0)), Some(32));
    let f = held_floor(&h, &WC).unwrap();
    let sm = f.sample.clone().unwrap();
    let infl = f.value / sm.midpoint - 1.0;
    eprintln!(
        "P5: WC, 32 of 32 pairs: sample floor {:.15e}, midpoint {:.15e}, inflation {infl:.3e} (limit 1e-6), largest {:?}",
        f.value, sm.midpoint, sm.largest
    );
    assert_eq!(sm.pairs.len(), 32);
    assert!(infl <= 1e-6, "P5");
}

// ---------------------------------------------------------------- P6

fn edit_floor(src: &Path, dst: &Path, f: impl Fn(&mut Value)) {
    let mut ls = lines(src);
    let mut e: Value = serde_json::from_str(&ls[0]).unwrap();
    f(&mut e["settings"]["held_floor"]);
    ls[0] = e.to_string();
    write_lines(dst, &ls);
}

#[test]
fn p6_guards() {
    let dir = tmp("p6");
    let path = dir.join("l.jsonl");
    let h = claim(&WQ, 749_001, 0.01, values((10.0, 250.0)), Some(24));
    register_with_floor(&ledger_at(&path), &h, &WQ).unwrap();
    let clean = verify(&entries(&ledger_at(&path)), "H-1", Some(&WQ));
    eprintln!("P6: clean registration: problems {:?}", clean.problems);
    assert!(clean.problems.is_empty());
    let orig: Value = serde_json::from_str(&lines(&path)[0]).unwrap();
    let sample = orig["settings"]["held_floor"]["sample"].clone();
    let has = |v: &Verification, w: &str| v.problems.iter().any(|p| p.contains(w));
    // (a) sample removed
    let pa = dir.join("a.jsonl");
    edit_floor(&path, &pa, |f| {
        f.as_object_mut().unwrap().remove("sample");
    });
    let va = verify(&entries(&ledger_at(&pa)), "H-1", None);
    // (b) the first pair replaced by an index not in the list
    let pairs = idx(&sample["pairs"]);
    let other = (0..32).find(|j| !pairs.contains(j)).unwrap();
    let pb = dir.join("b.jsonl");
    edit_floor(&path, &pb, |f| {
        f["sample"]["pairs"][0] = Value::from(other as u64);
    });
    let vb = verify(&entries(&ledger_at(&pb)), "H-1", None);
    // (c) the value replaced by the midpoint value
    let pc = dir.join("c.jsonl");
    let mid = sample["midpoint"].clone();
    edit_floor(&path, &pc, |f| {
        f["value"] = mid.clone();
    });
    let vc = verify(&entries(&ledger_at(&pc)), "H-1", None);
    let vcm = verify(&entries(&ledger_at(&pc)), "H-1", Some(&WQ));
    // (d) a sample added to a midpoint registration (the P3 record without its sample)
    let mut hm = claim(&WQ, 749_003, 5e-5, values((10.0, 250.0)), None);
    hm.floor_sample = None;
    let pm = dir.join("m.jsonl");
    register_with_floor(&ledger_at(&pm), &hm, &WQ).unwrap();
    let pd = dir.join("d.jsonl");
    edit_floor(&pm, &pd, |f| {
        f["sample"] = sample.clone();
    });
    let vd = verify(&entries(&ledger_at(&pd)), "H-1", None);
    let hits = [
        has(&va, "floor sample"),
        has(&vb, "sampled pairs"),
        has(&vc, "not the largest of its sample") && has(&vcm, "not reproduced"),
        has(&vd, "floor sample"),
    ];
    eprintln!(
        "P6: (a) {:?}\n  (b) {:?}\n  (c) {:?} / with the model {:?}\n  (d) {:?}\n  hits {hits:?}",
        va.problems, vb.problems, vc.problems, vcm.problems, vd.problems
    );
    assert_eq!(hits.iter().filter(|x| **x).count(), 4, "P6");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- P7

#[test]
fn p7_refusals() {
    let dir = tmp("p7");
    let path = dir.join("l.jsonl");
    let l = ledger_at(&path);
    // a single hold (n by a), b in the domain, so plain register reaches the floor-sample rule
    let mut one = claim(
        &WQ,
        749_007,
        0.01,
        vec![hn(HeldTarget::Value(50.0), (10.0, 250.0))],
        Some(8),
    );
    one.domain.push(Bound::new("b", "1", 0.005, 0.02));
    let r1 = register(&l, &one).unwrap_err();
    let r2 = register_batch(&l, &[one.clone(), one.clone()]).unwrap_err();
    let r3 = register_with_floor(&l, &claim(&WQ, 749_007, 0.01, values((10.0, 250.0)), Some(0)), &WQ).unwrap_err();
    let r4 = register_with_floor(&l, &claim(&WQ, 749_007, 0.01, values((10.0, 250.0)), Some(33)), &WQ).unwrap_err();
    let narrow = claim(&WQ, 749_007, 0.01, values((10.0, 150.0)), Some(32));
    let r5 = register_with_floor(&l, &narrow, &WQ).unwrap_err();
    let unchanged = read_bytes(&path).is_none();
    let named = [
        r1.contains("floor_sample") && r1.contains("register_with_floor"),
        r2.contains("floor_sample") && r2.contains("register_with_floor"),
        r3.contains("floor_sample"),
        r4.contains("floor_sample"),
        r5.contains("design pair"),
    ];
    let mut mid = narrow.clone();
    mid.floor_sample = None;
    let acc = register_with_floor(&l, &mid, &WQ);
    eprintln!(
        "P7:\n  {r1}\n  {r2}\n  {r3}\n  {r4}\n  {r5}\n  named {named:?}; file unchanged {unchanged}; the [10, 150] record by the \
         midpoint rule {:?}",
        acc.as_ref().map(|r| r.slot)
    );
    assert_eq!(named.iter().filter(|x| **x).count(), 5, "P7");
    assert!(unchanged, "P7 file");
    assert!(acc.is_ok(), "P7 midpoint accepted");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- P8

#[test]
fn p8_bytes_repeat() {
    let dir = tmp("p8");
    let h = claim(&WQ, 749_001, 0.01, values((10.0, 250.0)), Some(24));
    let (pa, pb) = (dir.join("a.jsonl"), dir.join("b.jsonl"));
    register_with_floor(&ledger_at(&pa), &h, &WQ).unwrap();
    register_with_floor(&ledger_at(&pb), &h, &WQ).unwrap();
    let (a, b) = (std::fs::read(&pa).unwrap(), std::fs::read(&pb).unwrap());
    eprintln!("P8: {} and {} bytes, equal {}", a.len(), b.len(), a == b);
    assert_eq!(a, b, "P8");
    let _ = std::fs::remove_dir_all(&dir);
}
