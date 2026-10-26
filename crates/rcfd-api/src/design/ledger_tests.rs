//! ADR 0121 checks L1-L6 of the design ledger.

use super::*;
use std::sync::Arc;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_w46d_ledger_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d.join("ledger.jsonl")
}

fn fixed() -> u64 {
    1_700_000_000_000
}

pub(crate) fn sample_entry(k: usize, x: f64) -> Entry {
    Entry {
        study: "s".into(),
        variant: format!("v{k}"),
        params: vec![Quantity::new("chord", x, "m"), Quantity::new("blades", 7.0, "1")],
        evaluator: "analytic.test".into(),
        evaluator_version: "1".into(),
        geometry_hash: format!("{:016x}", (x * 1e6) as u64),
        settings: json!({"rpm": 2000.0, "note": "a \"quoted\", text\nwith a newline"}),
        metrics: vec![
            Quantity::new("flow", 1.0 / (1.0 + x), "m3/s"),
            Quantity::new("noise", x * x, "dB(A)"),
        ],
        verdict: Verdict::Ok,
        fidelity: Fidelity::Analytic,
        seed: Some(k as u64),
        provenance: json!({"k": k}),
        ..Default::default()
    }
}

fn ledger(path: &Path) -> Ledger {
    let mut l = Ledger::new(path);
    l.clock = fixed;
    l
}

#[test]
fn l1_round_trip_is_exact_and_validation_is_located() {
    let p = tmp("l1");
    let l = ledger(&p);
    let xs = [0.1, 1.0 / 3.0, 2f64.sqrt(), 1e-300, 123456.789, -0.0];
    let mut want: Vec<Entry> = xs.iter().enumerate().map(|(k, &x)| sample_entry(k, x)).collect();
    want[2].verdict = Verdict::Diverged;
    want[2].reason = "PIMPLE divergence guard: |U| is 1e30 at step 4, cell 17".into();
    want[2].metrics[1].value = f64::NAN;
    want[3].verdict = Verdict::ConstraintRejected;
    want[3].reason = "tip gap 0.1 mm below the minimum 0.3 mm".into();
    want[4].wall_s = Some(12.5);
    l.append(&want[..3]).unwrap();
    l.append(&want[3..]).unwrap();
    let got = l.read().unwrap();
    assert!(got.skipped.is_empty(), "{:?}", got.skipped);
    assert_eq!(got.entries.len(), want.len());
    for (g, w) in got.entries.iter().zip(&want) {
        let mut w = w.clone();
        w.solver_version = solver_version();
        w.recorded_at_ms = fixed();
        // NaN != NaN: compare the bit patterns of every number
        let bits = |e: &Entry| -> Vec<u64> { e.params.iter().chain(&e.metrics).map(|q| q.value.to_bits()).collect() };
        let nan_free = |e: &Entry| {
            let mut e = e.clone();
            for q in e.metrics.iter_mut().chain(e.params.iter_mut()) {
                q.value = 0.0;
            }
            e
        };
        let wb: Vec<u64> = bits(&w)
            .into_iter()
            .map(|b| {
                if f64::from_bits(b).is_nan() {
                    f64::NAN.to_bits()
                } else {
                    b
                }
            })
            .collect();
        assert_eq!(bits(g), wb);
        assert_eq!(nan_free(g), nan_free(&w));
    }
    // located validation errors; nothing is written for a bad batch
    let bad = |f: &dyn Fn(&mut Entry)| {
        let mut e = sample_entry(9, 0.5);
        f(&mut e);
        l.append(&[sample_entry(8, 0.5), e]).unwrap_err()
    };
    assert!(bad(&|e| e.params[0].unit.clear()).contains("params 'chord': empty unit"));
    assert!(bad(&|e| e.metrics[1].name = "flow".into()).contains("metrics 'flow': given twice"));
    assert!(bad(&|e| e.metrics[0].value = f64::INFINITY).contains("metrics 'flow'"));
    assert!(bad(&|e| e.verdict = Verdict::Diverged).contains("needs a located reason"));
    assert!(bad(&|e| e.evaluator_version.clear()).contains("evaluator_version"));
    assert!(bad(&|e| e.params[0].value = f64::NAN).contains("params 'chord'"));
    assert_eq!(l.read().unwrap().entries.len(), want.len());
    // negative control: a line of another format is not an entry
    let text = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, text.replacen(FORMAT, "rcfd-ledger-0", 1)).unwrap();
    let c = l.read().unwrap();
    assert_eq!(c.entries.len(), want.len() - 1);
    assert_eq!(c.skipped.len(), 1);
    assert!(c.skipped[0].reason.contains("format"), "{:?}", c.skipped);
}

/// L2: every byte offset of a torn tail; the newline guard keeps the next entry.
#[test]
fn l2_torn_tail_at_every_byte_offset() {
    let p = tmp("l2");
    let l = ledger(&p);
    let es: Vec<Entry> = (0..3).map(|k| sample_entry(k, 0.25 + k as f64)).collect();
    l.append(&es).unwrap();
    let full = std::fs::read(&p).unwrap();
    let ends: Vec<usize> = full
        .iter()
        .enumerate()
        .filter(|(_, &b)| b == b'\n')
        .map(|(i, _)| i)
        .collect();
    let extra = sample_entry(99, 7.5);
    let mut naive_lost = 0usize;
    let mut cuts = 0usize;
    for cut in 0..=full.len() {
        // a complete JSON line without its newline still counts (any proper prefix of a JSON object does not parse)
        let complete = ends.iter().filter(|&&e| e <= cut).count();
        std::fs::write(&p, &full[..cut]).unwrap();
        let c = l.read().unwrap();
        assert_eq!(c.entries.len(), complete, "cut {cut}");
        assert_eq!(c.entries[..], stamped(&es[..complete])[..], "cut {cut}");
        assert!(c.skipped.len() <= 1, "cut {cut}");
        l.append(std::slice::from_ref(&extra)).unwrap();
        let c = l.read().unwrap();
        assert_eq!(
            c.entries.len(),
            complete + 1,
            "cut {cut}: the appended entry must be readable"
        );
        assert_eq!(c.entries[complete].variant, "v99", "cut {cut}");
        assert!(c.skipped.len() <= 1, "cut {cut}");
        // negative control: append without the newline guard
        std::fs::write(&p, &full[..cut]).unwrap();
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        let mut e = extra.clone();
        e.solver_version = solver_version();
        e.recorded_at_ms = fixed();
        f.write_all((e.to_json().to_string() + "\n").as_bytes()).unwrap();
        drop(f);
        if l.read().unwrap().entries.len() != complete + 1 {
            naive_lost += 1;
        }
        cuts += 1;
    }
    eprintln!("L2: {cuts} cuts; the naive append lost the new entry at {naive_lost} cuts");
    assert!(naive_lost > 0, "the negative control must lose entries");
}

fn stamped(es: &[Entry]) -> Vec<Entry> {
    es.iter()
        .map(|e| {
            let mut e = e.clone();
            e.solver_version = solver_version();
            e.recorded_at_ms = fixed();
            e
        })
        .collect()
}

const KILL_ENV: &str = "RCFD_W46D_KILL_CHILD";

fn big_entry(k: usize) -> Entry {
    let mut e = sample_entry(k, k as f64);
    e.provenance = json!({"k": k, "pad": "x".repeat(16 * 1024)});
    e
}

const TEAR_ENV: &str = "RCFD_W46D_TEAR_CHILD";

/// Child process of L3 and L3b: appends until killed (bounded at 20 000 entries; returns at once without the environment
/// variable). With `RCFD_W46D_TEAR_CHILD` set it writes every line in two halves 100 ms apart (raw writes, not `Ledger::append`),
/// so that a kill lands inside a line.
#[test]
fn l3_kill_child() {
    let Ok(p) = std::env::var(KILL_ENV) else {
        return;
    };
    let tear = std::env::var(TEAR_ENV).is_ok();
    let l = Ledger::new(Path::new(&p));
    for k in 0..20_000 {
        if tear {
            let mut e = big_entry(k);
            e.solver_version = solver_version();
            e.recorded_at_ms = 1;
            let line = e.to_json().to_string() + "\n";
            let (a, b) = line.split_at(line.len() / 2);
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&p).unwrap();
            f.write_all(a.as_bytes()).unwrap();
            f.sync_data().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(100));
            f.write_all(b.as_bytes()).unwrap();
            f.sync_data().unwrap();
        } else {
            l.append(&[big_entry(k)]).unwrap();
        }
    }
}

/// Kills and reaps the child when the parent test ends, also on a failed assertion.
struct KillOnDrop(std::process::Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn_child(p: &Path, tear: bool) -> KillOnDrop {
    let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
    cmd.args([
        "design::ledger::tests::l3_kill_child",
        "--exact",
        "--nocapture",
        "--test-threads=1",
    ])
    .env(KILL_ENV, p)
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null());
    if tear {
        cmd.env(TEAR_ENV, "1");
    } else {
        cmd.env_remove(TEAR_ENV);
    }
    KillOnDrop(cmd.spawn().unwrap())
}

/// L3: a process killed while appending leaves a readable ledger with every entry it wrote before the last one.
#[test]
fn l3_kill_during_append_leaves_a_readable_ledger() {
    l3(200, "l3");
}

fn l3(kill_after: usize, tag: &str) {
    let p = tmp(tag);
    let child = spawn_child(&p, false);
    let l = Ledger::new(&p);
    let t0 = std::time::Instant::now();
    let mut seen = 0;
    while seen < kill_after {
        assert!(t0.elapsed().as_secs() < 120, "the child wrote only {seen} entries");
        std::thread::sleep(std::time::Duration::from_millis(5));
        seen = l.read().unwrap().entries.len();
    }
    drop(child);
    let c = l.read().unwrap();
    let torn = std::fs::read(&p).unwrap().last() != Some(&b'\n');
    eprintln!(
        "L3: seen {seen} before the kill, {} entries after, {} skipped, torn tail {torn}",
        c.entries.len(),
        c.skipped.len()
    );
    assert!(c.entries.len() >= seen);
    for (k, e) in c.entries.iter().enumerate() {
        assert_eq!(e.variant, format!("v{k}"), "entries must be contiguous");
        assert_eq!(e.provenance, big_entry(k).provenance);
    }
    assert!(c.skipped.len() <= 1);
    let n = c.entries.len();
    l.append(&[sample_entry(n, 0.5)]).unwrap();
    let c = l.read().unwrap();
    assert_eq!(c.entries.len(), n + 1);
    assert_eq!(c.entries[n].variant, format!("v{n}"));
}

/// L3b (ADR 0121 amendment 1): a kill that lands inside a line; the guarded append recovers, a naive one loses the entry.
#[test]
fn l3b_kill_inside_a_line() {
    l3b(10, "l3b");
}

fn l3b(min_entries: usize, tag: &str) {
    let p = tmp(tag);
    let child = spawn_child(&p, true);
    let l = Ledger::new(&p);
    let t0 = std::time::Instant::now();
    loop {
        assert!(
            t0.elapsed().as_secs() < 120,
            "the child did not reach {min_entries} entries with a torn tail"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
        let b = std::fs::read(&p).unwrap_or_default();
        if b.last().is_some_and(|&c| c != b'\n')
            && parse_text(&String::from_utf8_lossy(&b)).entries.len() >= min_entries
        {
            break;
        }
    }
    drop(child);
    let bytes = std::fs::read(&p).unwrap();
    let complete = bytes.iter().filter(|&&c| c == b'\n').count();
    let c = l.read().unwrap();
    eprintln!(
        "L3b: {} bytes, {complete} complete lines, {} entries, {} skipped, last byte newline {}",
        bytes.len(),
        c.entries.len(),
        c.skipped.len(),
        bytes.last() == Some(&b'\n')
    );
    assert_ne!(bytes.last(), Some(&b'\n'), "the kill must leave a torn line");
    assert_eq!(c.entries.len(), complete);
    assert_eq!(c.skipped.len(), 1);
    for (k, e) in c.entries.iter().enumerate() {
        assert_eq!(e.variant, format!("v{k}"));
    }
    // guarded append after the kill: readable at the next index
    l.append(&[sample_entry(complete, 0.5)]).unwrap();
    let after = l.read().unwrap();
    assert_eq!(after.entries.len(), complete + 1);
    assert_eq!(after.entries[complete].variant, format!("v{complete}"));
    assert_eq!(after.skipped.len(), 1);
    // negative control: a naive append to the killed file merges into the torn line and is lost
    std::fs::write(&p, &bytes).unwrap();
    let mut e = sample_entry(complete, 0.5);
    e.solver_version = solver_version();
    e.recorded_at_ms = 1;
    let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
    f.write_all((e.to_json().to_string() + "\n").as_bytes()).unwrap();
    drop(f);
    assert_eq!(
        l.read().unwrap().entries.len(),
        complete,
        "the naive append must lose the entry"
    );
}

/// L4: threads appending through one handle and through two handles of the same file.
#[test]
fn l4_concurrent_appends() {
    l4(8, 250, "l4");
}

/// Rule-d smoke (ignored) of the L3 and L4 set-ups with smaller counts.
#[test]
#[ignore]
fn smoke_w46d_ledger() {
    l3(50, "l3smoke");
    l3b(3, "l3bsmoke");
    l4(4, 50, "l4smoke");
}

fn l4(threads: usize, per: usize, tag: &str) {
    let p = tmp(tag);
    let a = Arc::new(ledger(&p));
    let b = Arc::new(ledger(&p));
    std::thread::scope(|s| {
        for t in 0..threads {
            let l = if t % 2 == 0 { a.clone() } else { b.clone() };
            s.spawn(move || {
                for k in 0..per {
                    let mut e = sample_entry(k, 0.5);
                    e.study = format!("t{t}");
                    e.provenance = json!({"t": t, "k": k, "pad": "y".repeat(100 * (k % 7))});
                    l.append(&[e]).unwrap();
                }
            });
        }
    });
    let c = a.read().unwrap();
    assert!(c.skipped.is_empty(), "{:?}", &c.skipped[..c.skipped.len().min(3)]);
    assert_eq!(c.entries.len(), threads * per);
    for t in 0..threads {
        let ks: Vec<u64> = c
            .entries
            .iter()
            .filter(|e| e.study == format!("t{t}"))
            .map(|e| e.provenance["k"].as_u64().unwrap())
            .collect();
        assert_eq!(
            ks,
            (0..per as u64).collect::<Vec<_>>(),
            "thread {t}: every entry once, in order"
        );
    }
    // negative control: two half lines interleaved by hand are reported, not read as entries
    let e0 = sample_entry(0, 0.5).to_json().to_string();
    let e1 = sample_entry(1, 0.5).to_json().to_string();
    let (h0, t0) = e0.split_at(e0.len() / 2);
    let (h1, t1) = e1.split_at(e1.len() / 2);
    let c = parse_text(&format!("{h0}{h1}\n{t0}{t1}\n"));
    assert!(c.entries.is_empty());
    assert_eq!(c.skipped.len(), 2);
}

fn front_set() -> Vec<Entry> {
    // flow (max) and noise (min) on the curve noise = flow^2 for t in 0..=10, plus dominated points
    let mut out = Vec::new();
    for k in 0..=10 {
        let t = k as f64 / 10.0;
        let mut e = sample_entry(k, t);
        e.metrics = vec![Quantity::new("flow", t, "m3/s"), Quantity::new("noise", t * t, "dB(A)")];
        out.push(e);
        let mut d = sample_entry(100 + k, t);
        d.metrics = vec![
            Quantity::new("flow", t, "m3/s"),
            Quantity::new("noise", t * t + 0.05, "dB(A)"),
        ];
        out.push(d);
    }
    let mut f = sample_entry(200, 2.0);
    f.verdict = Verdict::Diverged;
    f.reason = "diverged at iteration 3".into();
    f.metrics = vec![Quantity::new("flow", 9.0, "m3/s"), Quantity::new("noise", 0.0, "dB(A)")];
    out.push(f);
    out
}

/// L5: filters, Pareto membership, best so far, parameter history, CSV export.
#[test]
fn l5_queries_and_export() {
    let es = front_set();
    let all: Vec<usize> = (0..es.len()).collect();
    let ok = Filter {
        verdict: Some(Verdict::Ok),
        ..Default::default()
    }
    .apply(&es);
    assert_eq!(ok.len(), 22);
    let front = pareto_front(&es, &all, &[("flow".into(), Sense::Max), ("noise".into(), Sense::Min)]).unwrap();
    assert_eq!(
        front,
        (0..=10).map(|k| 2 * k).collect::<Vec<_>>(),
        "the 11 points on the curve, the diverged one excluded"
    );
    let (best, trace) = best_so_far(&es, &all, "noise", Sense::Min).unwrap();
    assert_eq!(best, Some(0));
    assert_eq!(trace.len(), 22);
    let mut run = f64::INFINITY;
    for (i, v) in &trace {
        run = run.min(es[*i].metric("noise").unwrap().value);
        assert_eq!(*v, run);
    }
    let (best_flow, _) = best_so_far(&es, &all, "flow", Sense::Max).unwrap();
    assert_eq!(
        best_flow,
        Some(20),
        "the first entry reaching flow 1 (the diverged entry with flow 9 is excluded)"
    );
    let h = history(&es, &all, "chord");
    assert_eq!(h.len(), es.len());
    assert_eq!(h[3], (3, 0.1, "m".to_string(), Verdict::Ok));
    // negative controls: mixed evaluators and mixed units are refused
    let mut mixed = es.clone();
    mixed[5].evaluator = "rom.bem".into();
    assert!(pareto_front(&mixed, &all, &[("flow".into(), Sense::Max)])
        .unwrap_err()
        .contains("different evaluators"));
    let mut units = es.clone();
    units[7].metrics[1].unit = "dB".into();
    assert!(best_so_far(&units, &all, "noise", Sense::Min)
        .unwrap_err()
        .contains("unit 'dB(A)' in entry 0 and 'dB' in entry 7"));
    // CSV: every number parses back to the same bits; quoting survives commas, quotes and newlines
    let csv = to_csv(&es, &all);
    let head = csv.lines().next().unwrap();
    assert!(head.contains("param:chord [m]") && head.contains("metric:noise [dB(A)]"));
    let row1 = csv.lines().nth(2).unwrap();
    let cols: Vec<&str> = row1.split(',').collect();
    let hc: Vec<&str> = head.split(',').collect();
    let at = |name: &str| cols[hc.iter().position(|h| *h == name).unwrap()];
    assert_eq!(
        at("param:chord [m]").parse::<f64>().unwrap(),
        es[1].param("chord").unwrap().value
    );
    assert_eq!(
        at("metric:noise [dB(A)]").parse::<f64>().unwrap(),
        es[1].metric("noise").unwrap().value
    );
    assert_eq!(csv.lines().count(), es.len() + 1);
}

/// L6: the re-run check reproduces an entry bit for bit and flags a changed evaluator.
#[test]
fn l6_rerun_check() {
    let model = FnModel {
        id: "analytic.test".into(),
        version: "1".into(),
        fidelity: Fidelity::Analytic,
        f: |p: &[Quantity]| {
            let x = p[0].value;
            Outcome::ok(vec![
                Quantity::new("flow", 1.0 / (1.0 + x), "m3/s"),
                Quantity::new("noise", x * x, "dB(A)"),
            ])
        },
    };
    let mut e = entry_of(
        &model,
        vec![Quantity::new("chord", 0.37, "m")],
        model.evaluate(&[Quantity::new("chord", 0.37, "m")]),
    );
    e.geometry_hash.clear();
    let r = rerun_check(&e, &model, 0.0, 0.0);
    assert!(r.pass, "{r:?}");
    assert!(r.metrics.iter().all(|m| m.3 == 0.0));
    // negative controls: a perturbed evaluator, another version, another unit
    let off = FnModel {
        f: |p: &[Quantity]| {
            let x = p[0].value;
            Outcome::ok(vec![
                Quantity::new("flow", 1.0 / (1.0 + x) * (1.0 + 1e-6), "m3/s"),
                Quantity::new("noise", x * x, "dB(A)"),
            ])
        },
        id: model.id.clone(),
        version: "1".into(),
        fidelity: Fidelity::Analytic,
    };
    let r = rerun_check(&e, &off, 1e-9, 0.0);
    assert!(!r.pass);
    assert!(!r.metrics[0].4 && r.metrics[1].4);
    let v2 = FnModel {
        version: "2".into(),
        ..off
    };
    assert!(rerun_check(&e, &v2, 1.0, 1.0).problems[0].contains("entry needs analytic.test 1"));
    let r = compare_metrics(
        &e,
        &[Quantity::new("flow", e.metrics[0].value, "l/s")],
        Verdict::Ok,
        0.0,
        0.0,
    );
    assert!(!r.pass && r.problems.iter().any(|p| p.contains("unit")) && r.problems.iter().any(|p| p.contains("noise")));
}
