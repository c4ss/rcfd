//! ADR 0647 (F-706): `Ledger::private` refuses a path under `.hg`, `.svn` or the project manifest, with a located message.
//! Checks B1 to B7 of the registration; B3w is in `rcfd-workspace`.

use rcfd_api::design::fidelity::{
    self, Candidates, CorrectedMetric, FidelitySpec, HighSource, Observation, ParamBox, Privacy,
};
use rcfd_api::design::ledger::{
    repository_ancestor, repository_marker, Entry, Fidelity, FnModel, Ledger, Outcome, Quantity, Source, Verdict,
};
use rcfd_api::design::study::Sense;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f706_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    // N0: the temp directory lies outside every marker (else the set-up is defective)
    assert_eq!(
        repository_marker(&d.join("probe.jsonl")),
        None,
        "N0: {} is inside a marker",
        d.display()
    );
    d
}

fn mkdir(p: &Path) {
    std::fs::create_dir_all(p).unwrap();
}

fn touch(p: &Path, text: &str) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn canon(p: &Path) -> PathBuf {
    p.canonicalize().unwrap()
}

/// B1 to B3: both paths refused, the message located, nothing created.
fn refused_under(root: &Path, marker: &str, paths: &[PathBuf]) {
    let want = canon(root);
    for p in paths {
        assert_eq!(repository_marker(p), Some((want.clone(), marker)), "{}", p.display());
        assert_eq!(repository_ancestor(p), Some(want.clone()));
        let err = Ledger::private(p).unwrap_err();
        println!("{marker}: {err}");
        assert!(err.contains("inside the repository tree"), "{err}");
        assert!(err.contains(&want.display().to_string()), "{err}");
        assert!(err.contains(&format!("(marker {marker})")), "{err}");
        assert!(!p.exists(), "{} created", p.display());
    }
    assert!(!root.join("a").exists());
}

#[test]
fn b1_hg_refused_located() {
    let t = tmp("b1");
    let r = t.join("hgrepo");
    mkdir(&r.join(".hg"));
    refused_under(
        &r,
        ".hg",
        &[r.join("ledger.jsonl"), r.join("a").join("b").join("ledger.jsonl")],
    );
}

#[test]
fn b2_svn_refused_located() {
    let t = tmp("b2");
    let r = t.join("svnrepo");
    mkdir(&r.join(".svn"));
    refused_under(
        &r,
        ".svn",
        &[r.join("ledger.jsonl"), r.join("a").join("b").join("ledger.jsonl")],
    );
}

#[test]
fn b3_project_manifest_refused_located() {
    let t = tmp("b3");
    let r = t.join("proj");
    touch(&r.join("rcfd-project.json"), "{}");
    refused_under(
        &r,
        "rcfd-project.json",
        &[r.join("ledger.jsonl"), r.join("designs").join("x").join("ledger.jsonl")],
    );
    assert!(!r.join("designs").exists());
}

#[test]
fn b4_nearest_marker_named() {
    let t = tmp("b4");
    let outer = t.join("outer");
    mkdir(&outer.join(".hg"));
    touch(&outer.join("inner").join("rcfd-project.json"), "{}");
    let p = outer.join("inner").join("x").join("ledger.jsonl");
    let inner = canon(&outer.join("inner"));
    assert_eq!(repository_marker(&p), Some((inner.clone(), "rcfd-project.json")));
    let err = Ledger::private(&p).unwrap_err();
    println!("B4 (a): {err}");
    assert!(
        err.contains(&inner.display().to_string()) && err.contains("rcfd-project.json"),
        "{err}"
    );
    assert!(!err.contains(".hg"), "{err}");

    let o2 = t.join("o2");
    touch(&o2.join("rcfd-project.json"), "{}");
    mkdir(&o2.join("i2").join(".svn"));
    let p2 = o2.join("i2").join("ledger.jsonl");
    let i2 = canon(&o2.join("i2"));
    assert_eq!(repository_marker(&p2), Some((i2.clone(), ".svn")));
    let err2 = Ledger::private(&p2).unwrap_err();
    println!("B4 (b): {err2}");
    assert!(
        err2.contains(&i2.display().to_string()) && err2.contains("(marker .svn)"),
        "{err2}"
    );
}

#[test]
fn b5_git_first_in_one_directory() {
    let t = tmp("b5");
    let both = t.join("both");
    mkdir(&both.join(".git"));
    mkdir(&both.join(".hg"));
    assert_eq!(
        repository_marker(&both.join("ledger.jsonl")),
        Some((canon(&both), ".git"))
    );
}

fn measured_entry() -> Entry {
    Entry {
        study: "f706".into(),
        variant: "v0".into(),
        params: vec![Quantity::new("x", 0.5, "m")],
        evaluator: "rig.synthetic".into(),
        evaluator_version: "1".into(),
        solver_version: "test".into(),
        metrics: vec![Quantity::new("q", 0.135792468, "m3/s")],
        verdict: Verdict::Ok,
        fidelity: Fidelity::Measured,
        recorded_at_ms: 1_700_000_000_000,
        source: Some(Source {
            id: "rig-synthetic".into(),
            kind: "measured".into(),
            citation: None,
            file_sha256: None,
        }),
        ..Entry::default()
    }
}

#[test]
fn b6_negative_controls_accepted() {
    let t = tmp("b6");
    let names = t.join("names");
    for f in [
        ".hgignore",
        ".hgtags",
        "hg",
        ".svnignore",
        "svn",
        "rcfd-project.json.bak",
        "rcfd-project.jsonl",
        "project.json",
    ] {
        touch(&names.join(f), "x");
    }
    mkdir(&names.join(".hg_old"));
    mkdir(&names.join("_svn"));
    mkdir(&t.join("sib").join(".hg"));
    touch(&t.join("sib").join("rcfd-project.json"), "{}");
    mkdir(&t.join("near"));
    let sub = t.join("down").join("sub");
    mkdir(&sub.join(".hg"));
    mkdir(&sub.join(".svn"));
    touch(&sub.join("rcfd-project.json"), "{}");
    touch(&t.join("crate").join("Cargo.toml"), "[package]\nname = \"x\"\n");
    mkdir(&t.join("plain"));
    let paths = [
        t.join("plain").join("ledger.jsonl"),
        names.join("ledger.jsonl"),
        t.join("near").join("ledger.jsonl"),
        t.join("down").join("ledger.jsonl"),
        t.join("crate").join("ledger.jsonl"),
    ];
    let mut accepted = 0;
    for p in &paths {
        assert_eq!(repository_marker(p), None, "{}", p.display());
        let l = Ledger::private(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        assert!(l.is_private());
        accepted += 1;
    }
    println!("B6: {accepted} of {} accepted", paths.len());
    let l = Ledger::private(&paths[0]).unwrap();
    let e = measured_entry();
    l.append(std::slice::from_ref(&e)).unwrap();
    let c = l.read().unwrap();
    assert!(c.skipped.is_empty(), "{:?}", c.skipped);
    assert_eq!(c.entries, vec![e]);
}

struct PrivateSource<'a> {
    calls: &'a AtomicUsize,
}

impl HighSource for PrivateSource<'_> {
    fn id(&self) -> &str {
        "rig-synthetic"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::Measured
    }
    fn private(&self) -> bool {
        true
    }
    fn observe(&self, _params: &[Quantity]) -> Observation {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Observation {
            outcome: Outcome::ok(vec![Quantity::new("f", 1.0, "Pa"), Quantity::new("g", 1.0, "W")]),
            uncertainty: Vec::new(),
        }
    }
}

#[test]
fn b7_driver_refuses_before_any_evaluation() {
    let t = tmp("b7");
    let drv = t.join("drv");
    mkdir(&drv.join(".hg"));
    let privp = drv.join("private").join("ledger.jsonl");
    let pubp = t.join("pub").join("ledger.jsonl");
    let low_calls = AtomicUsize::new(0);
    let high_calls = AtomicUsize::new(0);
    let low = FnModel {
        id: "analytic.s1".to_string(),
        version: "1".to_string(),
        fidelity: Fidelity::Analytic,
        f: |_x: &[Quantity]| {
            low_calls.fetch_add(1, Ordering::SeqCst);
            Outcome::ok(vec![Quantity::new("f", 1.0, "Pa"), Quantity::new("g", 1.0, "W")])
        },
    };
    let high = PrivateSource { calls: &high_calls };
    let params = vec![
        ParamBox::new("x1", "m", 0.0, 1.0),
        ParamBox::new("x2", "1", -1.0, 2.0),
        ParamBox::new("x3", "s", 10.0, 20.0),
    ];
    let metrics = ["f", "g"]
        .iter()
        .map(|n| CorrectedMetric {
            name: (*n).into(),
            floor: 1e-3,
        })
        .collect();
    let mut spec = FidelitySpec::new(
        "f706",
        params,
        Candidates::Lhs { n: 20, seed: 706 },
        metrics,
        "f",
        Sense::Min,
    );
    spec.privacy = Some(Privacy::new(&privp, "rig-synthetic"));
    let err = fidelity::run(&spec, &low, &high, &Ledger::new(&pubp)).unwrap_err();
    println!("B7: {err}");
    assert!(err.starts_with("fidelity: private ledger:"), "{err}");
    assert!(
        err.contains(&canon(&drv).display().to_string()) && err.contains("(marker .hg)"),
        "{err}"
    );
    assert_eq!(low_calls.load(Ordering::SeqCst), 0);
    assert_eq!(high_calls.load(Ordering::SeqCst), 0);
    assert!(!privp.exists() && !pubp.exists());
}
