//! ADR 0632 (F-700) checks A1 to A8 and A11 to A13: optional ledger fields, the measured tier and the privacy split, the
//! fidelity pair record, the pairing and training-set queries. A9 and A10 are the existing tests (rcfd-mcp goldens, design::).

use super::*;
use crate::design::{pareto, study};

/// The ADR 0121 ledger as of c4424c9b, verbatim (lines 1 to 854 of `ledger.rs`), as the base writer and reader.
#[allow(dead_code, clippy::all)]
#[path = "ledger_base_c4424c9b.rs"]
mod base;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f700_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d.join("ledger.jsonl")
}

fn fixed() -> u64 {
    1_700_000_000_000
}

fn ledger(path: &Path) -> Ledger {
    let mut l = Ledger::new(path);
    l.clock = fixed;
    l
}

fn base_ledger(path: &Path) -> base::Ledger {
    let mut l = base::Ledger::new(path);
    l.clock = fixed;
    l
}

fn bq(q: &[Quantity]) -> Vec<base::Quantity> {
    q.iter()
        .map(|x| base::Quantity::new(&x.name, x.value, &x.unit))
        .collect()
}

/// The base fields of an entry as a base entry (public tiers only).
fn to_base(e: &Entry) -> base::Entry {
    base::Entry {
        kind: e.kind.clone(),
        study: e.study.clone(),
        variant: e.variant.clone(),
        params: bq(&e.params),
        evaluator: e.evaluator.clone(),
        evaluator_version: e.evaluator_version.clone(),
        solver_version: e.solver_version.clone(),
        geometry_hash: e.geometry_hash.clone(),
        settings: e.settings.clone(),
        metrics: bq(&e.metrics),
        verdict: base::Verdict::parse(e.verdict.as_str()).unwrap(),
        reason: e.reason.clone(),
        fidelity: base::Fidelity::parse(e.fidelity.as_str()).unwrap(),
        recorded_at_ms: e.recorded_at_ms,
        wall_s: e.wall_s,
        seed: e.seed,
        provenance: e.provenance.clone(),
    }
}

fn stamped(e: &Entry) -> Entry {
    let mut e = e.clone();
    if e.solver_version.is_empty() {
        e.solver_version = solver_version();
    }
    if e.recorded_at_ms == 0 {
        e.recorded_at_ms = fixed();
    }
    e
}

/// Bits of every number of an entry (NaN kept as its bits).
fn bits(e: &Entry) -> Vec<u64> {
    e.params
        .iter()
        .chain(&e.metrics)
        .chain(&e.uncertainty)
        .map(|q| q.value.to_bits())
        .chain(e.wall_s.map(f64::to_bits))
        .collect()
}

fn base_bits(e: &base::Entry) -> Vec<u64> {
    e.params
        .iter()
        .chain(&e.metrics)
        .map(|q| q.value.to_bits())
        .chain(e.wall_s.map(f64::to_bits))
        .collect()
}

/// FNV-1a 64 written out again in the test (independent of the library's).
fn fnv(text: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in text.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{h:016x}")
}

fn lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap()
        .split('\n')
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

// ---------------------------------------------------------------- fixtures (ADR 0632 "Set-ups")

/// The six entries of check L1 of ADR 0121.
fn l1_set() -> Vec<Entry> {
    let xs = [0.1, 1.0 / 3.0, 2f64.sqrt(), 1e-300, 123456.789, -0.0];
    let mut want: Vec<Entry> = xs
        .iter()
        .enumerate()
        .map(|(k, &x)| super::tests::sample_entry(k, x))
        .collect();
    want[2].verdict = Verdict::Diverged;
    want[2].reason = "PIMPLE divergence guard: |U| is 1e30 at step 4, cell 17".into();
    want[2].metrics[1].value = f64::NAN;
    want[3].verdict = Verdict::ConstraintRejected;
    want[3].reason = "tip gap 0.1 mm below the minimum 0.3 mm".into();
    want[4].wall_s = Some(12.5);
    want
}

const PAIRS: [(usize, usize); 24] = [
    (0, 36),
    (2, 78),
    (8, 64),
    (9, 11),
    (15, 93),
    (17, 79),
    (18, 26),
    (24, 12),
    (31, 41),
    (37, 27),
    (38, 14),
    (40, 56),
    (46, 42),
    (47, 85),
    (53, 71),
    (60, 4),
    (62, 86),
    (67, 33),
    (69, 19),
    (75, 5),
    (76, 48),
    (82, 34),
    (89, 63),
    (91, 49),
];
const DUPLICATES: [usize; 4] = [51, 70, 73, 92];

fn xy(k: usize) -> (f64, f64) {
    ((k as f64 - 23.0) * 0.125, (k as f64).sqrt())
}

fn f96_entry(fidelity: Fidelity, evaluator: &str, version: &str, params: Vec<Quantity>, q: f64, k: usize) -> Entry {
    Entry {
        study: "f96".into(),
        variant: format!("v{k}"),
        params,
        evaluator: evaluator.into(),
        evaluator_version: version.into(),
        metrics: vec![Quantity::new("q", q, "m3/s")],
        verdict: Verdict::Ok,
        fidelity,
        ..Default::default()
    }
}

fn low(k: usize) -> Entry {
    let (x, y) = xy(k);
    let p = vec![Quantity::new("x", x, "m"), Quantity::new("y", y, "1")];
    f96_entry(Fidelity::ReducedOrder, "rom.fan", "1", p, x + y, k)
}

fn high_of(x: f64, y: f64, k: usize) -> Entry {
    let p = vec![Quantity::new("y", y, "1"), Quantity::new("x", x, "m")];
    f96_entry(Fidelity::Cfd, "cfd.fan", "3", p, x + y + 0.01, k)
}

fn high(k: usize) -> Entry {
    let (x, y) = xy(k);
    high_of(x, y, k)
}

/// F96 in generation order (g = 0..95).
fn f96_gen() -> Vec<Entry> {
    let paired: Vec<usize> = (0..24).map(|j| (7 * j) % 44).collect();
    let mut g: Vec<Entry> = (0..44).map(low).collect();
    g.extend(paired.iter().map(|&k| high(k)));
    for k in [2, 4, 6, 9] {
        let (x, y) = xy(k);
        g.push(high_of(f64::from_bits(x.to_bits() + 1), y, k));
    }
    for k in [11, 13, 16, 18] {
        let (x, y) = xy(k);
        g.push(high_of(x, f64::from_bits(y.to_bits() + 1), k));
    }
    g.push(high_of(-0.0, xy(23).1, 23));
    for k in [20, 25, 27] {
        let mut e = high(k);
        e.params.push(Quantity::new("z", 1.0, "1"));
        g.push(e);
    }
    for k in [30, 32, 34, 36] {
        let mut e = high(k);
        e.verdict = Verdict::Diverged;
        e.reason = "synthetic divergence at step 3".into();
        e.metrics[0].value = f64::NAN;
        g.push(e);
    }
    g.extend(paired[0..4].iter().map(|&k| high(k)));
    for &k in &paired[4..8] {
        let mut e = high(k);
        e.kind = "uq_summary".into();
        g.push(e);
    }
    for &k in &paired[8..12] {
        let mut e = low(k);
        e.fidelity = Fidelity::Analytic;
        e.evaluator = "analytic.fan".into();
        g.push(e);
    }
    assert_eq!(g.len(), 96);
    g
}

fn place(g: Vec<Entry>) -> Vec<Entry> {
    let mut out = vec![Entry::default(); 96];
    for (i, e) in g.into_iter().enumerate() {
        out[(37 * i) % 96] = e;
    }
    out
}

fn f96() -> Vec<Entry> {
    place(f96_gen())
}

fn t14() -> Vec<Entry> {
    let mut v: Vec<Entry> = (0..14)
        .map(|k| {
            let a = 0.125 * k as f64;
            let b = (1 + k % 3) as f64;
            let ok = k < 12;
            Entry {
                study: "t14".into(),
                variant: format!("t{k}"),
                params: vec![Quantity::new("a", a, "m"), Quantity::new("b", b, "1")],
                evaluator: "rom.fan".into(),
                evaluator_version: "2".into(),
                metrics: vec![
                    Quantity::new("f", if ok { a * a + b } else { f64::NAN }, "Pa"),
                    Quantity::new("g", if ok { 0.1 * k as f64 } else { f64::NAN }, "W"),
                ],
                verdict: if ok { Verdict::Ok } else { Verdict::Diverged },
                reason: if ok { String::new() } else { "synthetic failure".into() },
                fidelity: Fidelity::ReducedOrder,
                ..Default::default()
            }
        })
        .collect();
    v.push(Entry {
        kind: "uq_summary".into(),
        study: "t14".into(),
        variant: "t14#summary".into(),
        evaluator: "rom.fan".into(),
        evaluator_version: "2".into(),
        verdict: Verdict::Ok,
        fidelity: Fidelity::ReducedOrder,
        ..Default::default()
    });
    v
}

fn ident(tag: &str) -> Identity {
    Identity {
        id: format!("{tag}.id"),
        version: "1".into(),
        hash: "fnv1a64:0123456789abcdef".into(),
    }
}

fn computed_source() -> Source {
    Source {
        id: "rom-run-7".into(),
        kind: "computed".into(),
        citation: Some("ADR 0119".into()),
        file_sha256: Some("ab".repeat(32)),
    }
}

/// Set X: one entry per new field, one with all six, and a fidelity pair of F96 pair (0, 36).
fn x_set() -> Vec<Entry> {
    let s = |k: usize| super::tests::sample_entry(k, 0.25 + k as f64);
    let mut v = Vec::new();
    let mut e = s(0);
    e.parents = vec![EntryRef {
        index: 3,
        hash: Some("fnv1a64:00000000deadbeef".into()),
    }];
    v.push(e);
    let mut e = s(1);
    e.source = Some(computed_source());
    v.push(e);
    let mut e = s(2);
    e.fidelity = Fidelity::Cfd;
    e.fidelity_level = Some("coarse".into());
    v.push(e);
    let mut e = s(3);
    e.uncertainty = vec![Quantity::new("flow", 1e-3, "m3/s")];
    v.push(e);
    let mut e = s(4);
    e.kind = "objective_score".into();
    e.objective = Some(ident("objective"));
    v.push(e);
    let mut e = s(5);
    e.kind = "surrogate_fit".into();
    e.fitted = Some(ident("gp"));
    v.push(e);
    let mut e = s(6);
    e.fidelity = Fidelity::ReducedOrder;
    e.parents = vec![EntryRef { index: 0, hash: None }, EntryRef { index: 1, hash: None }];
    e.source = Some(computed_source());
    e.fidelity_level = Some("r1".into());
    e.uncertainty = vec![
        Quantity::new("noise", 0.5, "dB(A)"),
        Quantity::new("flow", 2e-3, "m3/s"),
    ];
    e.objective = Some(ident("objective"));
    e.fitted = Some(ident("correction"));
    v.push(e);
    let f = f96();
    v.push(fidelity_pair(&f, 0, 36, &["q"], &[1e-3], None).unwrap());
    v
}

// ---------------------------------------------------------------- A1, A2

#[test]
fn a1_no_new_field_gives_the_base_bytes() {
    let sets: [(&str, Vec<Entry>); 3] = [("l1", l1_set()), ("f96", f96()), ("t14", t14())];
    for (tag, set) in sets {
        let (pn, pb) = (tmp(&format!("a1n_{tag}")), tmp(&format!("a1b_{tag}")));
        let (ln, lb) = (ledger(&pn), base_ledger(&pb));
        let based: Vec<base::Entry> = set.iter().map(to_base).collect();
        let cut = set.len() / 2;
        ln.append(&set[..cut]).unwrap();
        ln.append(&set[cut..]).unwrap();
        lb.append(&based[..cut]).unwrap();
        lb.append(&based[cut..]).unwrap();
        let (bn, bb) = (std::fs::read(&pn).unwrap(), std::fs::read(&pb).unwrap());
        let differing = bn.iter().zip(&bb).filter(|(a, b)| a != b).count() + bn.len().abs_diff(bb.len());
        println!(
            "A1 {tag}: {} entries, {} bytes, {differing} differing bytes",
            set.len(),
            bn.len()
        );
        assert_eq!(differing, 0, "{tag}");
        for (e, b) in set.iter().zip(&based) {
            assert_eq!(e.to_json().to_string(), b.to_json().to_string(), "{tag}");
        }
    }
}

#[test]
fn a2_base_reader_keeps_every_line_with_new_fields() {
    let p = tmp("a2");
    let l = ledger(&p);
    let set = x_set();
    l.append(&set).unwrap();
    let text = std::fs::read_to_string(&p).unwrap();
    let got = base::parse_text(&text);
    println!(
        "A2: {} lines, base reader {} entries, {} skipped",
        set.len(),
        got.entries.len(),
        got.skipped.len()
    );
    assert!(got.skipped.is_empty(), "{:?}", got.skipped);
    assert_eq!(got.entries.len(), 8);
    for (b, e) in got.entries.iter().zip(&set) {
        let want = to_base(&stamped(e));
        assert_eq!(base_bits(b), base_bits(&want));
        assert_eq!(*b, want);
    }
    // and the new reader gives the new fields back
    let new = l.read().unwrap();
    assert!(new.skipped.is_empty(), "{:?}", new.skipped);
    for (g, e) in new.entries.iter().zip(&set) {
        assert_eq!(*g, stamped(e));
        assert_eq!(bits(g), bits(e));
    }
}

// ---------------------------------------------------------------- A3, A4, A13

#[test]
fn a3_pairing_returns_the_registered_24_pairs() {
    let p = tmp("a3");
    ledger(&p).append(&f96()).unwrap();
    let c = ledger(&p).read().unwrap();
    assert!(c.skipped.is_empty());
    assert_eq!(c.entries.len(), 96);
    let all: Vec<usize> = (0..96).collect();
    let r = pair_by_params(
        &c.entries,
        &all,
        &Level::tier(Fidelity::ReducedOrder),
        &Level::tier(Fidelity::Cfd),
    )
    .unwrap();
    println!(
        "A3: {} pairs {:?}, duplicates {:?}",
        r.pairs.len(),
        r.pairs,
        r.duplicates
    );
    assert_eq!(r.pairs, PAIRS.to_vec());
    assert_eq!(r.duplicates, DUPLICATES.to_vec());
}

#[test]
fn a4_pairing_refuses_mixed_units() {
    let all: Vec<usize> = (0..96).collect();
    let (lo, hi) = (Level::tier(Fidelity::ReducedOrder), Level::tier(Fidelity::Cfd));
    for (g, name, unit, other) in [(44 + 5, "x", "mm", "m"), (1, "y", "rad", "1")] {
        let mut gen = f96_gen();
        gen[g].params.iter_mut().find(|q| q.name == name).unwrap().unit = unit.into();
        let es = place(gen);
        let err = pair_by_params(&es, &all, &lo, &hi).unwrap_err();
        println!("A4 {name}: {err}");
        assert!(err.contains(&format!("parameter '{name}'")), "{err}");
        assert!(
            err.contains(&format!("'{unit}'")) && err.contains(&format!("'{other}'")),
            "{err}"
        );
        assert_eq!(err.matches("entry ").count(), 2, "{err}");
    }
}

#[test]
fn a13_finer_levels_pair_and_equal_levels_are_refused() {
    let mk = |lvl: &str, x: f64| {
        let mut e = high(0);
        e.params[1].value = x;
        e.fidelity_level = Some(lvl.into());
        e
    };
    let es = vec![mk("coarse", 0.5), mk("fine", 0.5), mk("coarse", 0.75), mk("fine", 0.75)];
    let lv = |l: &str| Level {
        fidelity: Fidelity::Cfd,
        level: Some(l.into()),
    };
    let r = pair_by_params(&es, &[0, 1, 2, 3], &lv("coarse"), &lv("fine")).unwrap();
    assert_eq!(r.pairs, vec![(0, 1), (2, 3)]);
    let err = pair_by_params(&es, &[0, 1, 2, 3], &lv("fine"), &lv("fine")).unwrap_err();
    println!("A13: {:?}; equal levels: {err}", r.pairs);
}

// ---------------------------------------------------------------- A5

#[test]
fn a5_fidelity_pair_round_trips_and_names_both_entries() {
    let p = tmp("a5");
    let l = ledger(&p);
    let mut f = f96();
    f[78].uncertainty = vec![Quantity::new("q", 0.002, "m3/s")];
    l.append(&f).unwrap();
    let c = l.read().unwrap();
    assert!(c.skipped.is_empty());
    let pred = c.entries[2].metric("q").unwrap().value + 0.005;
    let pair = fidelity_pair(&c.entries, 2, 78, &["q"], &[1e-3], Some(&[pred])).unwrap();
    l.append(std::slice::from_ref(&pair)).unwrap();
    let c2 = l.read().unwrap();
    assert!(c2.skipped.is_empty());
    assert_eq!(c2.entries.len(), 97);
    let got = &c2.entries[96];
    let want = stamped(&pair);
    assert_eq!(*got, want);
    assert_eq!(bits(got), bits(&want));
    let raw = lines(&p);
    assert_eq!(raw.len(), 97);
    let hs = [fnv(&raw[2]), fnv(&raw[78])];
    assert_eq!(
        got.parents,
        vec![
            EntryRef {
                index: 2,
                hash: Some(hs[0].clone())
            },
            EntryRef {
                index: 78,
                hash: Some(hs[1].clone())
            }
        ]
    );
    assert_eq!(resolve(&c2.entries, &got.parents[0]), Ok(2));
    assert_eq!(resolve(&c2.entries, &got.parents[1]), Ok(78));
    let (lq, hq) = (
        c.entries[2].metric("q").unwrap().value,
        c.entries[78].metric("q").unwrap().value,
    );
    let diff = hq - lq;
    assert_eq!(got.metric("q.diff").unwrap().value.to_bits(), diff.to_bits());
    assert_eq!(
        got.metric("q.rel").unwrap().value.to_bits(),
        (diff / hq.abs().max(1e-3)).to_bits()
    );
    assert_eq!(got.metric("q.pred").unwrap().value.to_bits(), pred.to_bits());
    assert_eq!(got.uncertainty, vec![Quantity::new("q.high", 0.002, "m3/s")]);
    // a stale reference is reported, not followed
    let stale = EntryRef {
        index: 3,
        hash: Some(hs[0].clone()),
    };
    assert!(resolve(&c2.entries, &stale).is_err());
    println!("A5: parents {:?}, diff {diff:?}, line {}", got.parents, raw[96]);
}

// ---------------------------------------------------------------- A6, A7

const MATRIX: [[f64; 4]; 12] = [
    [1.0, 0.0, 0.0, 1.0],
    [2.0, 0.125, 0.1, 2.015625],
    [3.0, 0.25, 0.2, 3.0625],
    [1.0, 0.375, 0.30000000000000004, 1.140625],
    [2.0, 0.5, 0.4, 2.25],
    [3.0, 0.625, 0.5, 3.390625],
    [1.0, 0.75, 0.6000000000000001, 1.5625],
    [2.0, 0.875, 0.7000000000000001, 2.765625],
    [3.0, 1.0, 0.8, 4.0],
    [1.0, 1.125, 0.9, 2.265625],
    [2.0, 1.25, 1.0, 3.5625],
    [3.0, 1.375, 1.1, 4.890625],
];

#[test]
fn a6_training_set_is_the_registered_matrix() {
    let p = tmp("a6");
    ledger(&p).append(&t14()).unwrap();
    let c = ledger(&p).read().unwrap();
    assert_eq!(c.entries.len(), 15);
    let all: Vec<usize> = (0..15).collect();
    let ts = training_set(&c.entries, &all, &["b", "a"], &["g", "f"]).unwrap();
    assert_eq!(ts.rows, (0..12).collect::<Vec<_>>());
    assert_eq!(
        ts.params,
        vec![("b".to_string(), "1".to_string()), ("a".to_string(), "m".to_string())]
    );
    assert_eq!(
        ts.metrics,
        vec![("g".to_string(), "W".to_string()), ("f".to_string(), "Pa".to_string())]
    );
    assert_eq!(ts.failed, vec![12, 13]);
    let mut n = 0;
    for (r, row) in MATRIX.iter().enumerate() {
        let k = r as f64;
        let a = 0.125 * k;
        let b = (1 + r % 3) as f64;
        let formula = [b, a, 0.1 * k, a * a + b];
        let got = [ts.x[r][0], ts.x[r][1], ts.y[r][0], ts.y[r][1]];
        for c in 0..4 {
            assert_eq!(got[c].to_bits(), row[c].to_bits(), "row {r} column {c}");
            assert_eq!(got[c].to_bits(), formula[c].to_bits(), "row {r} column {c}");
            n += 1;
        }
    }
    assert_eq!((ts.evaluator.as_str(), ts.evaluator_version.as_str()), ("rom.fan", "2"));
    assert_eq!(ts.refs(&c.entries).len(), 12);
    println!("A6: {n} values bit-equal, rows {:?}, failed {:?}", ts.rows, ts.failed);
}

#[test]
fn a7_training_set_refuses_mixed_versions_and_units() {
    let mut v = t14();
    let mut e = v[0].clone();
    e.evaluator_version = "3".into();
    v.push(e);
    let all: Vec<usize> = (0..v.len()).collect();
    let err = training_set(&v, &all, &["b", "a"], &["g", "f"]).unwrap_err();
    println!("A7 version: {err}");
    assert!(
        err.contains("'2'") && err.contains("'3'") && err.contains("entry 0") && err.contains("entry 15"),
        "{err}"
    );
    let mut u = t14();
    u[4].metrics[0].unit = "kPa".into();
    let all: Vec<usize> = (0..u.len()).collect();
    let err = training_set(&u, &all, &["b", "a"], &["g", "f"]).unwrap_err();
    println!("A7 units: {err}");
    assert!(
        err.contains("'f'") && err.contains("'Pa'") && err.contains("'kPa'"),
        "{err}"
    );
}

// ---------------------------------------------------------------- A8

fn measured_set() -> Vec<Entry> {
    let src = Source {
        id: "rig-synthetic".into(),
        kind: "measured".into(),
        citation: None,
        file_sha256: None,
    };
    let lo = f96_entry(
        Fidelity::ReducedOrder,
        "rom.fan",
        "1",
        vec![Quantity::new("x", 0.5, "m")],
        0.123456789,
        0,
    );
    let mut m = lo.clone();
    m.fidelity = Fidelity::Measured;
    m.evaluator = "rig.synthetic".into();
    m.metrics[0].value = 0.135792468;
    m.source = Some(src.clone());
    let mut c = lo.clone();
    c.fidelity = Fidelity::Cfd;
    c.evaluator = "cfd.fan".into();
    c.source = Some(src);
    let pair = fidelity_pair(&[lo, m.clone()], 0, 1, &["q"], &[1e-3], None).unwrap();
    assert_eq!(pair.fidelity, Fidelity::Measured);
    vec![m, c, pair]
}

#[test]
fn a8_measured_only_in_a_private_ledger() {
    let set = measured_set();
    let secret: Vec<String> = set
        .iter()
        .flat_map(|e| e.metrics.iter().map(|q| format!("{:?}", q.value)))
        .collect();
    // public: each refused, nothing written
    let pp = tmp("a8_public");
    let pl = ledger(&pp);
    for (i, e) in set.iter().enumerate() {
        let err = pl.append(std::slice::from_ref(e)).unwrap_err();
        println!("A8 public refusal {i}: {err}");
        assert!(err.contains("private ledger"), "{err}");
        assert!(secret.iter().all(|s| !err.contains(s.as_str())), "a value in: {err}");
    }
    assert!(!pp.exists(), "the public ledger file was created");
    // private, outside the repository
    let qp = tmp("a8_private");
    let mut ql = Ledger::private(&qp).unwrap();
    ql.clock = fixed;
    assert!(ql.is_private());
    ql.append(&set).unwrap();
    let c = ql.read().unwrap();
    assert!(c.skipped.is_empty(), "{:?}", c.skipped);
    assert_eq!(c.entries, set.iter().map(stamped).collect::<Vec<_>>());
    // public readers of the private file
    let pub_read = ledger(&qp).read().unwrap();
    let text = std::fs::read_to_string(&qp).unwrap();
    let base_read = base::parse_text(&text);
    println!(
        "A8: private {} entries; public reader {} entries {} skipped; base reader {} entries {} skipped",
        c.entries.len(),
        pub_read.entries.len(),
        pub_read.skipped.len(),
        base_read.entries.len(),
        base_read.skipped.len()
    );
    assert_eq!((pub_read.entries.len(), pub_read.skipped.len()), (0, 3));
    assert_eq!((base_read.entries.len(), base_read.skipped.len()), (1, 2));
    // a private path inside the repository tree
    let inside = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/f700_private/ledger.jsonl");
    let err = Ledger::private(&inside).unwrap_err();
    println!("A8 repository refusal: {err}");
    assert!(err.contains("inside the repository tree"), "{err}");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    assert_eq!(repository_ancestor(&inside), Some(root.clone()));
    assert!(err.contains(&root.display().to_string()), "{err}");
    assert!(!inside.exists());
}

// ---------------------------------------------------------------- A11, A12

#[test]
fn a11_malformed_new_fields_are_refused() {
    let base_e = super::tests::sample_entry(1, 0.5);
    let mut cases: Vec<(&str, Entry, (&str, Value))> = Vec::new();
    let mut e = base_e.clone();
    e.uncertainty = vec![Quantity::new("nope", 0.1, "m3/s")];
    cases.push((
        "uncertainty",
        e,
        ("uncertainty", json!([{"name": "nope", "value": 0.1, "unit": "m3/s"}])),
    ));
    let mut e = base_e.clone();
    e.uncertainty = vec![Quantity::new("flow", 0.1, "l/s")];
    cases.push((
        "uncertainty",
        e,
        ("uncertainty", json!([{"name": "flow", "value": 0.1, "unit": "l/s"}])),
    ));
    let mut e = base_e.clone();
    e.uncertainty = vec![Quantity::new("flow", -0.1, "m3/s")];
    cases.push((
        "uncertainty",
        e,
        ("uncertainty", json!([{"name": "flow", "value": -0.1, "unit": "m3/s"}])),
    ));
    let mut e = base_e.clone();
    e.parents = vec![EntryRef {
        index: 0,
        hash: Some("fnv1a64:xyz".into()),
    }];
    cases.push((
        "parents[0].hash",
        e,
        ("parents", json!([{"index": 0, "hash": "fnv1a64:xyz"}])),
    ));
    let mut e = base_e.clone();
    e.source = Some(Source {
        kind: "guess".into(),
        ..computed_source()
    });
    cases.push(("source.kind", e, ("source", json!({"id": "s", "kind": "guess"}))));
    let mut e = base_e.clone();
    e.objective = Some(Identity {
        hash: String::new(),
        ..ident("objective")
    });
    cases.push((
        "objective.hash",
        e,
        ("objective", json!({"id": "o", "version": "1", "hash": ""})),
    ));
    let mut refused = 0;
    let mut skipped = 0;
    for (field, e, (key, val)) in cases {
        let err = e.validate().unwrap_err();
        assert!(err.contains(field), "{field}: {err}");
        refused += 1;
        let mut v = stamped(&base_e).to_json();
        v.as_object_mut().unwrap().insert(key.into(), val);
        let c = parse_text(&v.to_string());
        if c.entries.is_empty() && c.skipped.len() == 1 {
            skipped += 1;
        }
        println!(
            "A11 {field}: validate '{err}', reader {:?}",
            c.skipped.first().map(|s| &s.reason)
        );
    }
    assert_eq!((refused, skipped), (6, 6));
}

#[test]
fn a12_variant_and_condition_filters_and_groups() {
    let mk = |v: &str, c: &str| Entry {
        variant: v.into(),
        settings: json!({"condition": c, "set": {}}),
        ..super::tests::sample_entry(0, 0.5)
    };
    let es = vec![
        mk("a", "c0"),
        mk("a", "c1"),
        mk("b", "c0"),
        mk("a", "c2"),
        mk("b", "c1"),
        mk("b", "c2"),
    ];
    let by_v = Filter {
        variant: Some("a".into()),
        ..Default::default()
    }
    .apply(&es);
    let by_c = Filter {
        condition: Some("c1".into()),
        ..Default::default()
    }
    .apply(&es);
    let all: Vec<usize> = (0..6).collect();
    let groups = group_by_variant(&es, &all);
    println!("A12: variant {by_v:?}, condition {by_c:?}, groups {groups:?}");
    assert_eq!(by_v, vec![0, 1, 3]);
    assert_eq!(by_c, vec![1, 4]);
    assert_eq!(
        groups,
        vec![("a".to_string(), vec![0, 1, 3]), ("b".to_string(), vec![2, 4, 5])]
    );
    // the default filter still matches everything
    assert_eq!(Filter::default().apply(&es), all);
}
