//! ADR 0693: checks of the verifier-drawn re-run sample (F-745): an evaluator that reads the file and is honest exactly on the
//! F-727 sample is caught by pairs drawn from a seed that is not in the file. Synthetic evaluators only; no CFD, no person, no
//! language model.

use rcfd_api::design::hypothesis::*;
use rcfd_api::design::ledger::{Entry, Fidelity, Ledger, Model, Outcome, Quantity};
use rcfd_api::design::rng::Rng;
use rcfd_api::design::study::parallel_map;
use rcfd_core::hash::fnv64;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

const CLOCK: u64 = 1_700_000_000_000;
const K727: usize = 13;
const DRAWN: &str = "not reproduced by a verifier-drawn re-run";

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f745_{tag}_{}", std::process::id()));
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

/// ADR 0624's deterministic standard normal of a point.
fn noise(world: u64, metric: u64, params: &[Quantity]) -> f64 {
    let mut bytes = Vec::with_capacity(8 * params.len());
    for q in params {
        bytes.extend_from_slice(&q.value.to_bits().to_le_bytes());
    }
    let mut r = Rng::new(fnv64(&bytes) ^ world ^ (metric + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let (u1, u2) = (r.uniform(), r.uniform());
    (-2.0 * (1.0 - u1).ln()).sqrt() * (2.0 * PI * u2).cos()
}

fn val(params: &[Quantity], name: &str) -> f64 {
    params.iter().find(|q| q.name == name).map_or(0.0, |q| q.value)
}

// ---------------------------------------------------------------- W5 and the file-aware impostor

#[derive(Clone)]
struct W5 {
    seed: u64,
    id: &'static str,
    version: &'static str,
}

fn w5(seed: u64) -> W5 {
    W5 {
        seed,
        id: "synthetic.w5",
        version: "1",
    }
}

impl Model for W5 {
    fn id(&self) -> &str {
        self.id
    }
    fn version(&self) -> &str {
        self.version
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::Cfd
    }
    fn evaluate(&self, p: &[Quantity]) -> Outcome {
        Outcome::ok(vec![Quantity::new("y", val(p, "x0") + noise(self.seed, 0, p), "1")])
    }
}

/// Same id, version and tier; `y + 0.3` on the high arm of the dishonest points, or `y (1 + rel)` everywhere.
struct Impostor {
    real: W5,
    dishonest: Vec<u64>,
    rel: Option<f64>,
}

impl Model for Impostor {
    fn id(&self) -> &str {
        self.real.id()
    }
    fn version(&self) -> &str {
        self.real.version()
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::Cfd
    }
    fn evaluate(&self, p: &[Quantity]) -> Outcome {
        let y = self.real.evaluate(p).metrics[0].value;
        let y = if let Some(r) = self.rel {
            y * (1.0 + r)
        } else if val(p, "x0") == 0.75 && self.dishonest.contains(&val(p, "x1").to_bits()) {
            y + 0.3
        } else {
            y
        };
        Outcome::ok(vec![Quantity::new("y", y, "1")])
    }
}

fn w5_claim(m: &W5, seed: u64, deterministic: bool) -> Hypothesis {
    let mut h = Hypothesis::new(
        "raising x0 raises y by at least 0.5",
        Factor {
            name: "x0".into(),
            unit: "1".into(),
            low: 0.25,
            high: 0.75,
        },
        Response {
            name: "y".into(),
            unit: "1".into(),
            resolution: 0.0,
        },
        Direction::Increases,
        0.5,
        vec![Bound::new("x1", "1", 0.0, 1.0)],
        64,
        seed,
        Scope::of(m, deterministic),
    );
    if deterministic {
        h.rerun_sample = Some(sample(K727, 0.0));
    }
    h
}

fn sample(k: usize, rtol: f64) -> RerunSample {
    RerunSample {
        pairs: k,
        rtol,
        atol: 0.0,
    }
}

/// Which evaluator writes the evidence of a world ledger.
#[derive(Clone, Copy, PartialEq)]
enum Who {
    /// Dishonest on every pair outside the F-727 sample.
    D1,
    /// Dishonest on 16 pairs outside the F-727 sample.
    D2,
    /// The real model.
    Honest,
    /// `y (1 + 1e-10)` on every arm.
    Rel,
}

struct Built {
    path: PathBuf,
    es: Vec<Entry>,
    s727: Vec<usize>,
    dishonest: Vec<usize>,
    hash: String,
    seed: u64,
}

/// Register the W5 claim (with ADR 0662's k = 13 sample), compute from the registered record only the F-727 sample, and run
/// with the evaluator `who`.
fn build(path: &Path, real: &W5, seed: u64, who: Who) -> Built {
    let _ = std::fs::remove_file(path);
    let l = ledger_at(path);
    let h = w5_claim(real, seed, true);
    let reg = register(&l, &h).unwrap();
    let rec = reg.record.clone().unwrap();
    let s727 = rerun_pairs(rec.n_pairs, K727, rerun_seed(&reg.hash, rec.seed));
    let rest: Vec<usize> = (0..rec.n_pairs).filter(|p| !s727.contains(p)).collect();
    let dishonest = match who {
        Who::D1 => rest,
        Who::D2 => {
            let mut r = rest;
            Rng::new(seed ^ 0x0745_0D02).shuffle(&mut r);
            r.truncate(16);
            r
        }
        Who::Honest | Who::Rel => Vec::new(),
    };
    let pts = design_points(&rec, reg.derived_seed);
    match who {
        Who::Honest => {
            run(&l, &reg.id, real, &RunOptions::default()).unwrap();
        }
        _ => {
            let imp = Impostor {
                real: real.clone(),
                dishonest: dishonest.iter().map(|&p| pts[p][0].to_bits()).collect(),
                rel: (who == Who::Rel).then_some(1e-10),
            };
            run(&l, &reg.id, &imp, &RunOptions::default()).unwrap();
        }
    }
    Built {
        path: path.to_path_buf(),
        es: entries(&l),
        s727,
        dishonest,
        hash: reg.hash,
        seed: rec.seed,
    }
}

fn entry_of(p: &str) -> usize {
    p.trim_start_matches("entry ")
        .split(':')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

fn arm(es: &[Entry], k: usize) -> String {
    es[k].provenance["arm"].as_str().unwrap_or("").to_string()
}

fn pair(es: &[Entry], k: usize) -> usize {
    es[k].provenance["pair"].as_u64().unwrap() as usize
}

/// D3: the number of problems is |drawn ∩ dishonest| (drawn by the recipe), each on a high arm of a drawn dishonest pair.
fn exact(b: &Built, c: &DrawnCheck, k: usize, s: u64) -> bool {
    let drawn = rerun_pairs(64, k, s);
    let inter = drawn.iter().filter(|p| b.dishonest.contains(p)).count();
    c.pairs == drawn
        && c.problems.len() == inter
        && c.problems.iter().all(|p| {
            let e = entry_of(p);
            p.contains(DRAWN) && arm(&b.es, e) == "high" && b.dishonest.contains(&pair(&b.es, e))
        })
        && c.reproduced == (inter == 0)
}

fn f727_rerun_problems(v: &Verification) -> usize {
    v.problems
        .iter()
        .filter(|p| p.contains("not reproduced by a re-run"))
        .count()
}

fn verifier_seed(w: u64) -> u64 {
    fnv64(format!("f745:verifier:{w}").as_bytes())
}

// ---------------------------------------------------------------- D0

#[test]
fn d0_one_world() {
    let dir = tmp("d0");
    let seed = 745_990;
    let real = w5(seed);
    let b = build(&dir.join("d1.jsonl"), &real, seed, Who::D1);
    // (i) the F-727 sample re-runs only honest entries
    let v = verify(&b.es, "H-1", Some(&real));
    eprintln!(
        "D0 (i): F-727 sample {:?}: {} problems, evidence_rerun {:?}",
        v.rerun_sample.as_ref().map(|p| &p.pairs),
        v.problems.len(),
        v.evidence_rerun
    );
    assert!(v.problems.is_empty());
    assert_eq!(v.evidence_rerun, Some(true));
    assert_eq!(v.rerun_sample.unwrap().pairs, b.s727);
    // (ii) drawn k = 3 from a verifier seed
    let s = 1_234_567_891_011_121_314u64;
    let c = check_drawn(&b.es, "H-1", &real, &sample(3, 0.0), s).unwrap();
    let outside = c.pairs.iter().filter(|p| !b.s727.contains(p)).count();
    let on_high = c
        .problems
        .iter()
        .filter(|p| {
            let e = entry_of(p);
            arm(&b.es, e) == "high" && c.pairs.contains(&pair(&b.es, e))
        })
        .count();
    let want_rerun: Vec<usize> = c
        .pairs
        .iter()
        .flat_map(|&q| [v.evidence[2 * q].unwrap(), v.evidence[2 * q + 1].unwrap()])
        .collect();
    eprintln!(
        "D0 (ii): drawn {:?}, {outside} outside the F-727 sample; {} problems, {on_high} on drawn high arms; reproduced {}; re-run {:?}",
        c.pairs,
        c.problems.len(),
        c.reproduced,
        c.rerun
    );
    assert_eq!(c.pairs, rerun_pairs(64, 3, s));
    assert_eq!(c.problems.len(), outside);
    assert_eq!(on_high, outside);
    assert_eq!(c.reproduced, outside == 0);
    assert_eq!(c.rerun.len(), 6);
    assert_eq!(c.rerun, want_rerun);
    assert!(exact(&b, &c, 3, s));
    // (iii) the honest ledger
    let hb = build(&dir.join("h.jsonl"), &real, seed, Who::Honest);
    let ch = check_drawn(&hb.es, "H-1", &real, &sample(3, 0.0), s).unwrap();
    eprintln!(
        "D0 (iii): honest {} problems, reproduced {}",
        ch.problems.len(),
        ch.reproduced
    );
    assert!(ch.problems.is_empty());
    assert!(ch.reproduced);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- worlds

#[derive(Debug, Default, Clone)]
struct World {
    d1_caught: bool,
    d2_caught: bool,
    d1_exact: bool,
    d2_exact: bool,
    nc1_caught: usize,
    nc1_ok: usize,
    nc2_caught: bool,
    nc2_is_s727: bool,
    nc3_caught: bool,
    h1_flagged: usize,
    honest_pairs: Vec<usize>,
    record_ok: bool,
}

fn world_run(w: u64, dir: &Path) -> World {
    let seed = 745_000 + w;
    let real = w5(seed);
    let s = verifier_seed(w);
    let mut out = World::default();
    // D1: verify_drawn, k = 3, the record appended
    let b1 = build(&dir.join(format!("a{w}.jsonl")), &real, seed, Who::D1);
    let v1 = verify(&b1.es, "H-1", Some(&real));
    let l1 = ledger_at(&b1.path);
    let c1 = verify_drawn(&l1, "H-1", &real, &sample(3, 0.0), DrawSeed::Given(s)).unwrap();
    out.d1_caught = !c1.problems.is_empty();
    out.d1_exact = exact(&b1, &c1, 3, s);
    let es1 = entries(&l1);
    out.record_ok = c1.record == Some(b1.es.len())
        && es1.len() == b1.es.len() + 1
        && drawn_record_problems(&es1, b1.es.len(), &real).is_empty();
    // NC2: a "verifier" seed derived from the file
    let c2 = check_drawn(&b1.es, "H-1", &real, &sample(K727, 0.0), rerun_seed(&b1.hash, b1.seed)).unwrap();
    out.nc2_caught = !c2.problems.is_empty();
    out.nc2_is_s727 = c2.pairs == b1.s727;
    // NC3: k = 1
    let c3 = check_drawn(&b1.es, "H-1", &real, &sample(1, 0.0), s).unwrap();
    out.nc3_caught = !c3.problems.is_empty();
    // D2: k = 13
    let b2 = build(&dir.join(format!("b{w}.jsonl")), &real, seed, Who::D2);
    let v2 = verify(&b2.es, "H-1", Some(&real));
    let c4 = check_drawn(&b2.es, "H-1", &real, &sample(13, 0.0), s).unwrap();
    out.d2_caught = !c4.problems.is_empty();
    out.d2_exact = exact(&b2, &c4, 13, s);
    // NC1: the F-727 sample on both impostor ledgers
    for v in [&v1, &v2] {
        out.nc1_caught += usize::from(f727_rerun_problems(v) > 0);
        out.nc1_ok += usize::from(v.evidence_rerun == Some(true));
    }
    // H1, H2: the honest ledger
    let bh = build(&dir.join(format!("h{w}.jsonl")), &real, seed, Who::Honest);
    for k in [3, 13] {
        let c = check_drawn(&bh.es, "H-1", &real, &sample(k, 0.0), s).unwrap();
        out.h1_flagged += usize::from(!c.problems.is_empty() || !c.reproduced);
        if k == 13 {
            out.honest_pairs = c.pairs;
        }
    }
    for t in ["a", "b", "h"] {
        let _ = std::fs::remove_file(dir.join(format!("{t}{w}.jsonl")));
    }
    out
}

fn worlds(ws: &[u64], tag: &str) -> Vec<World> {
    let dir = tmp(tag);
    let out = parallel_map(ws, 4, &|&w: &u64| world_run(w, &dir));
    let _ = std::fs::remove_dir_all(&dir);
    out
}

struct Summary {
    d1: usize,
    d2: usize,
    d3: usize,
    nc1: usize,
    nc1_ok: usize,
    nc2: usize,
    nc2_s727: usize,
    nc3: usize,
    h1: usize,
    records: usize,
}

fn summary(st: &[World]) -> Summary {
    let c = |f: &dyn Fn(&World) -> bool| st.iter().filter(|s| f(s)).count();
    Summary {
        d1: c(&|s| s.d1_caught),
        d2: c(&|s| s.d2_caught),
        d3: c(&|s| s.d1_exact) + c(&|s| s.d2_exact),
        nc1: st.iter().map(|s| s.nc1_caught).sum(),
        nc1_ok: st.iter().map(|s| s.nc1_ok).sum(),
        nc2: c(&|s| s.nc2_caught),
        nc2_s727: c(&|s| s.nc2_is_s727),
        nc3: c(&|s| s.nc3_caught),
        h1: st.iter().map(|s| s.h1_flagged).sum(),
        records: c(&|s| s.record_ok),
    }
}

fn print(tag: &str, n: usize, s: &Summary) {
    eprintln!(
        "{tag} ({n} worlds): D1 caught {} (limit >= 380 of 400), D2 caught {} (limit >= 380 of 400), D3 exact {} of {}; \
         NC1 F-727 sample caught {} of {} (re-run ok {}); NC2 file-derived seed caught {} (draw = S727 in {}); NC3 k = 1 caught {} \
         (287..350 of 400); H1 honest flagged {} of {}; records consistent {}",
        s.d1,
        s.d2,
        s.d3,
        2 * n,
        s.nc1,
        2 * n,
        s.nc1_ok,
        s.nc2,
        s.nc2_s727,
        s.nc3,
        s.h1,
        2 * n,
        s.records
    );
}

#[test]
#[ignore = "heavy registered record (ADR 0693 D1 to D3, NC1 to NC3, H1, H2): 400 worlds, run once"]
fn d_worlds_heavy() {
    let ws: Vec<u64> = (0..400).collect();
    let st = worlds(&ws, "dw");
    let s = summary(&st);
    print("heavy", 400, &s);
    let mut counts = [0usize; 64];
    for w in &st {
        for &p in &w.honest_pairs {
            counts[p] += 1;
        }
    }
    let (cmin, cmax) = (*counts.iter().min().unwrap(), *counts.iter().max().unwrap());
    eprintln!("H2: honest k = 13 pair counts {cmin}..{cmax} (limit 50..113)");
    assert!(s.d1 >= 380, "D1");
    assert!(s.d2 >= 380, "D2");
    assert_eq!(s.d3, 800, "D3");
    assert_eq!(s.nc1, 0, "NC1");
    assert_eq!(s.nc1_ok, 800, "NC1");
    assert_eq!(s.nc2, 0, "NC2");
    assert_eq!(s.nc2_s727, 400, "NC2");
    assert!((287..=350).contains(&s.nc3), "NC3");
    assert_eq!(s.h1, 0, "H1");
    assert!(cmin >= 50 && cmax <= 113, "H2");
}

/// Fast counterpart: worlds 0 to 15, the assertions that hold world by world.
#[test]
fn d_worlds_fast() {
    let ws: Vec<u64> = (0..16).collect();
    let st = worlds(&ws, "dwf");
    let s = summary(&st);
    print("fast", 16, &s);
    assert_eq!(s.d3, 32);
    assert_eq!(s.nc1, 0);
    assert_eq!(s.nc1_ok, 32);
    assert_eq!(s.nc2, 0);
    assert_eq!(s.nc2_s727, 16);
    assert_eq!(s.h1, 0);
    assert_eq!(s.records, 16);
}

// ---------------------------------------------------------------- E1, E2, E3

fn write_entries(path: &Path, es: &[Entry]) {
    let text: String = es.iter().map(|e| format!("{}\n", e.to_json())).collect();
    std::fs::write(path, text).unwrap();
}

fn rows(es: &[Entry], real: &W5) -> String {
    let q = LessonQuery {
        current: vec![("synthetic.w5".into(), "1".into())],
        ..Default::default()
    };
    format!("{:?}", lessons(es, &q, &[real]))
}

#[test]
fn e1_e2_record_after_the_check() {
    let dir = tmp("e1");
    let seed = 745_991;
    let real = w5(seed);
    let b = build(&dir.join("l.jsonl"), &real, seed, Who::D1);
    let before = std::fs::read(&b.path).unwrap();
    let s = fnv64(b"f745:e1");
    let l = ledger_at(&b.path);
    let c = verify_drawn(&l, "H-1", &real, &sample(3, 0.0), DrawSeed::Given(s)).unwrap();
    let after = std::fs::read(&b.path).unwrap();
    let es = entries(&l);
    let idx = b.es.len();
    let rec = &es[idx];
    let text_before = String::from_utf8_lossy(&before);
    let text_after = String::from_utf8_lossy(&after);
    let dec = s.to_string();
    let hex = format!("{s:016x}");
    let recorded: Vec<usize> = rec.settings["pairs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_u64().unwrap() as usize)
        .collect();
    eprintln!(
        "E1: before {} bytes, after {} bytes, prefix {}; new entries {}; kind {}; seed {} ({}), k {}, pairs {:?}; \
         seed text before: dec {} hex {}, after: dec {}; record {:?}, problems {}",
        before.len(),
        after.len(),
        after.starts_with(&before),
        es.len() - b.es.len(),
        rec.kind,
        rec.settings["seed"],
        rec.settings["seed_source"],
        rec.settings["k"],
        recorded,
        text_before.contains(&dec),
        text_before.contains(&hex),
        text_after.contains(&dec),
        c.record,
        c.problems.len()
    );
    assert!(after.starts_with(&before) && after.len() > before.len());
    assert_eq!(es.len(), b.es.len() + 1);
    assert_eq!(rec.kind, KIND_DRAWN);
    assert_eq!(c.record, Some(idx));
    assert_eq!(rec.settings["seed"].as_u64(), Some(s));
    assert_eq!(rec.settings["seed"].as_u64(), Some(c.seed));
    assert_eq!(rec.settings["seed_source"].as_str(), Some("given"));
    assert_eq!(rec.settings["k"].as_u64(), Some(3));
    assert_eq!(recorded, c.pairs);
    assert_eq!(recorded, rerun_pairs(64, 3, s));
    assert!(!text_before.contains(&dec) && !text_before.contains(&hex));
    assert!(text_after.contains(&dec));
    // verify and lessons are blind to the record
    let v0 = verify(&b.es, "H-1", Some(&real));
    let v1 = verify(&es, "H-1", Some(&real));
    assert_eq!(v0, v1, "E1: verify equal before and after");
    assert_eq!(
        rows(&b.es, &real),
        rows(&es, &real),
        "E1: lessons equal before and after"
    );
    let p = drawn_record_problems(&es, idx, &real);
    eprintln!("E1: drawn_record_problems unedited {p:?}");
    assert!(p.is_empty());
    // E2: edits of the file
    let mut edited = 0;
    for tag in ["a", "b", "c"] {
        let mut x = es.clone();
        match tag {
            "a" => x[idx].settings["seed"] = (s.wrapping_add(1)).into(),
            "b" => x[idx].settings["reproduced"] = (!c.reproduced).into(),
            _ => {
                let ev = x[idx].parents[1].index;
                x[ev].metrics[0].value += 1e-3;
            }
        }
        let path = dir.join(format!("e2{tag}.jsonl"));
        write_entries(&path, &x);
        let xr = entries(&ledger_at(&path));
        let p = drawn_record_problems(&xr, idx, &real);
        eprintln!("E2 ({tag}): {p:?}");
        if !p.is_empty() && p.iter().all(|m| m.starts_with(&format!("entry {idx}:"))) {
            edited += 1;
        }
    }
    // the rewrite itself keeps the bytes (so the edits above are the only change)
    let same = dir.join("e2same.jsonl");
    write_entries(&same, &es);
    assert_eq!(std::fs::read(&same).unwrap(), after, "E2: re-serialised file equal");
    assert_eq!(edited, 3, "E2: 3 of 3");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn e3_entropy_seed() {
    let dir = tmp("e3");
    let (a, b) = (entropy_seed(), entropy_seed());
    let seed = 745_992;
    let real = w5(seed);
    let hb = build(&dir.join("h.jsonl"), &real, seed, Who::Honest);
    let l = ledger_at(&hb.path);
    let c = verify_drawn(&l, "H-1", &real, &sample(3, 0.0), DrawSeed::Entropy).unwrap();
    let es = entries(&l);
    let rec = &es[hb.es.len()];
    let p = drawn_record_problems(&es, hb.es.len(), &real);
    eprintln!(
        "E3: entropy seeds {a} {b}; record seed {} ({}), report seed {}, pairs {:?}; problems {p:?}",
        rec.settings["seed"], rec.settings["seed_source"], c.seed, c.pairs
    );
    assert_ne!(a, b);
    assert_eq!(rec.settings["seed_source"].as_str(), Some("entropy"));
    assert_eq!(c.seed_source, "entropy");
    assert_eq!(rec.settings["seed"].as_u64(), Some(c.seed));
    assert_eq!(c.pairs, rerun_pairs(64, 3, c.seed));
    assert!(c.problems.is_empty() && c.reproduced);
    assert!(p.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- DT, DR

#[test]
fn dt_tolerances() {
    let dir = tmp("dt");
    let seed = 745_993;
    let real = w5(seed);
    let b = build(&dir.join("l.jsonl"), &real, seed, Who::Rel);
    let s = fnv64(b"f745:dt");
    let c1 = check_drawn(&b.es, "H-1", &real, &sample(4, 1e-9), s).unwrap();
    let c0 = check_drawn(&b.es, "H-1", &real, &sample(4, 0.0), s).unwrap();
    eprintln!(
        "DT: rtol 1e-9: {} problems, reproduced {}; rtol 0: {} problems, reproduced {}",
        c1.problems.len(),
        c1.reproduced,
        c0.problems.len(),
        c0.reproduced
    );
    assert!(c1.problems.is_empty() && c1.reproduced);
    assert_eq!(c0.problems.len(), 8);
    assert!(!c0.reproduced);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dr_refusals() {
    let dir = tmp("dr");
    let seed = 745_994;
    let real = w5(seed);
    let b = build(&dir.join("l.jsonl"), &real, seed, Who::Honest);
    let l = ledger_at(&b.path);
    // H-2: not declared deterministic; H-3: registered, not run
    register(&l, &w5_claim(&real, seed + 1, false)).unwrap();
    register(&l, &w5_claim(&real, seed + 2, true)).unwrap();
    let before = std::fs::read(&b.path).unwrap();
    let v2 = W5 {
        version: "2",
        ..real.clone()
    };
    let other = W5 {
        id: "synthetic.other",
        ..real.clone()
    };
    let cases: Vec<(&str, &str, &dyn Model, RerunSample)> = vec![
        ("k 0", "H-1", &real, sample(0, 0.0)),
        ("k 65", "H-1", &real, sample(65, 0.0)),
        ("rtol NaN", "H-1", &real, sample(3, f64::NAN)),
        (
            "atol -1",
            "H-1",
            &real,
            RerunSample {
                pairs: 3,
                rtol: 0.0,
                atol: -1.0,
            },
        ),
        ("version 2", "H-1", &v2, sample(3, 0.0)),
        ("other id", "H-1", &other, sample(3, 0.0)),
        ("non-deterministic", "H-2", &real, sample(3, 0.0)),
        ("incomplete", "H-3", &real, sample(3, 0.0)),
        ("unknown id", "H-9", &real, sample(3, 0.0)),
    ];
    let mut refused = 0;
    for (tag, id, m, s) in &cases {
        let r = verify_drawn(&l, id, *m, s, DrawSeed::Given(1));
        eprintln!("DR '{tag}': {:?}", r.as_ref().map(|c| c.problems.len()));
        if r.as_ref().err().is_some_and(|e| e.contains("drawn")) {
            refused += 1;
        }
    }
    let after = std::fs::read(&b.path).unwrap();
    assert_eq!(refused, 9, "DR: 9 of 9");
    assert_eq!(before, after, "DR: file unchanged");
    let _ = std::fs::remove_dir_all(&dir);
}
