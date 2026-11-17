//! F-834 follow-up: two engine checks that went with the deleted egui crates (F-449), restored on the workspace's code.
//! ADR 0025 M3 on `rcfd_workspace::decimate_minmax` (copied from `rcfd-gui/src/monitors.rs`), and the engine part of
//! ADR 0152 DV1 and DV5 (job 48R) on the workspace's start plan (`rcfd_workspace::plan`, what `start_run` applies).
//! The Studio's route before ADR 0152 (`PlanRoute::default_store_keys_set`), its `CaseExtra::case_store` key list and
//! its dialog lines had no engine counterpart and are not ported; DV2 to DV4 drove the Studio and are not ported either.

mod f834;

use rcfd_api::case::Case;
use rcfd_workspace::decimate_minmax;
use rcfd_workspace::plan::{parse_case, Plan, INTERVAL_KEYS};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------- M3: decimation

fn lcg(s: &mut u64) -> f64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*s >> 11) as f64 / (1u64 << 53) as f64
}

fn noisy(n: usize) -> Vec<[f64; 2]> {
    let mut s = 99u64;
    let mut v: Vec<[f64; 2]> = (0..n)
        .map(|i| [i as f64, (i as f64 * 0.01).sin() + 0.1 * lcg(&mut s)])
        .collect();
    if n > 20 {
        v[n / 3][1] = 50.0;
        v[2 * n / 3][1] = -50.0;
        v[n / 7][1] = 49.0;
    }
    v
}

/// Replaces `crates/rcfd-gui/tests/monitors_plot.rs::m3_decimation_preserves_extrema_and_order` (ADR 0025 M3, ported
/// unchanged to `crates/rcfd-studio/tests/monitors_plot.rs`), with the same series, sizes and assertions, on the copy in
/// `crates/rcfd-workspace/src/series.rs`: at most `max` points; a series that fits is returned whole; first and last
/// kept; global minimum and maximum kept bit for bit; x strictly increasing; a subset of the input; the minimum and
/// maximum of every interior bucket kept.
#[test]
fn m3_decimation_preserves_extrema_and_order() {
    let max = 2000usize;
    for n in [5usize, 1999, 2000, 2001, 10_000, 1_000_000] {
        let p = noisy(n);
        let d = decimate_minmax(&p, max);
        assert!(d.len() <= max, "n={n}: len {}", d.len());
        if n <= max {
            assert_eq!(d, p, "n={n}");
            continue;
        }
        assert_eq!(d.first(), p.first());
        assert_eq!(d.last(), p.last());
        let (gmin, gmax) = p.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |a, q| {
            (a.0.min(q[1]), a.1.max(q[1]))
        });
        let (dmin, dmax) = d.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |a, q| {
            (a.0.min(q[1]), a.1.max(q[1]))
        });
        assert_eq!(dmin.to_bits(), gmin.to_bits());
        assert_eq!(dmax.to_bits(), gmax.to_bits());
        assert!(d.windows(2).all(|w| w[1][0] > w[0][0]), "x not strictly increasing");
        // subset: x is the index, so the point must be identical to p[x]
        assert!(d.iter().all(|q| p[q[0] as usize] == *q));
        // per-bucket extrema (same bucketing as documented: interior split into (max-2)/2 buckets by index)
        let nb = (max - 2) / 2;
        let inner = n - 2;
        let have: std::collections::HashSet<u64> = d.iter().map(|q| q[0] as u64).collect();
        let mut bad = 0;
        for b in 0..nb {
            let (s, e) = (1 + b * inner / nb, 1 + (b + 1) * inner / nb);
            if s >= e {
                continue;
            }
            let mut imin = s;
            let mut imax = s;
            for i in s..e {
                if p[i][1] < p[imin][1] {
                    imin = i;
                }
                if p[i][1] > p[imax][1] {
                    imax = i;
                }
            }
            bad += usize::from(!have.contains(&(imin as u64))) + usize::from(!have.contains(&(imax as u64)));
        }
        assert_eq!(bad, 0, "n={n}: bucket extrema missing");
    }
}

// ---------------------------------------------------------------- DV1, DV5: store keys at their defaults

const OWN: &str = "the case sets its own field store (output.store_*)";
const PRECISION_KEY: &str = "output.store_precision";

/// The cavity (`common::RUN_CASE`) with an `[output]` table of `lines` (`saved_store_r.rs::cavity_with`).
fn cavity_with(lines: &str) -> String {
    format!("{}\n[output]\n{lines}\n", f834::RUN_CASE)
}

/// SF4 (b)'s piso cavity (`saved_store_r.rs::piso`).
fn piso() -> String {
    let t = f834::RUN_CASE.replace(
        "algorithm = \"simplec\"",
        "algorithm = \"piso\"\ndt = 0.0025\nend_time = 0.5",
    );
    assert!(t.contains("piso"));
    t
}

fn piso_with(lines: &str) -> String {
    format!("{}\n[output]\n{lines}\n", piso())
}

/// The JSON text of a TOML text's tree (only the keys the text sets; `saved_store_r.rs::json_of`).
fn json_of(toml_text: &str) -> String {
    serde_json::to_string_pretty(&toml::from_str::<Value>(toml_text).unwrap()).unwrap()
}

fn plan(text: &str, base: &Path) -> Plan {
    Plan::of(&parse_case(text, base, None).unwrap_or_else(|d| panic!("{d:?}")))
}

/// The engine's reading of a text's store keys: (the case sets its own store interval, the case sets its own
/// precision), the `sets_store` and `sets_precision` of the Studio's `case_facts_route`.
fn read(text: &str) -> (bool, bool) {
    let p = plan(text, Path::new("."));
    (p.every.is_none() && p.note.as_deref() == Some(OWN), p.sets_precision)
}

const ALL_DEFAULTS: &str =
    "store_interval = 0\nstore_time_interval = 0.0\nstore_fields = []\nstore_precision = \"f64\"";

/// Replaces the engine part of `crates/rcfd-studio/tests/saved_store_r.rs::dv1_a_store_key_at_its_schema_default_is_not_set`
/// (ADR 0152 DV1): the schema defaults of the four store keys (`store_plan::store_defaults` read them from the same
/// schema), whether each of the twelve texts sets its own store and precision by ADR 0152's rule, TOML and JSON alike,
/// and that texts (i) to (vii), whose store keys are all at their defaults, plan exactly as the case without them
/// (TOML and JSON). Rows (i), (v), (viii) and (xi) overlap `new_case_f271.rs::plan_values` and `store_plan_f834.rs`
/// SF9 (c), (j), (k), (l) for TOML; they stay so that the table is DV1's and its JSON half is checked. Not ported: the
/// key list of `case_store` (the Studio's `CaseExtra`) and the negative control on the route before ADR 0152, a Studio
/// switch with no engine counterpart.
#[test]
fn dv1_a_store_key_at_its_schema_default_is_not_set() {
    let c = Case::new();
    let d: Vec<(&str, Value)> = INTERVAL_KEYS
        .iter()
        .chain(["output.store_fields", PRECISION_KEY].iter())
        .map(|k| (*k, c.get(k).unwrap()))
        .collect();
    eprintln!("DV1 defaults: {d:?}");
    assert_eq!(INTERVAL_KEYS, ["output.store_interval", "output.store_time_interval"]);
    assert!(d[0].1.is_u64() && d[0].1.as_u64() == Some(0), "{:?}", d[0]);
    assert!(d[1].1.is_f64() && d[1].1.as_f64() == Some(0.0), "{:?}", d[1]);
    assert_eq!(d[2].1, json!([]));
    assert_eq!(d[3].1, json!("f64"));
    let none = (false, false);
    let table: Vec<(&str, String, (bool, bool))> = vec![
        ("(i)", cavity_with("store_interval = 0"), none),
        ("(ii)", piso_with("store_time_interval = 0.0"), none),
        ("(iii)", piso_with("store_time_interval = 0"), none),
        ("(iv)", piso_with("store_time_interval = -0.0"), none),
        ("(v)", cavity_with("store_precision = \"f64\""), none),
        ("(vi)", cavity_with("store_fields = []"), none),
        ("(vii)", cavity_with(ALL_DEFAULTS), none),
        ("(viii)", cavity_with("store_interval = 7"), (true, false)),
        ("(ix)", piso_with("store_time_interval = 0.05"), (true, false)),
        ("(x)", cavity_with("store_precision = \"f32\""), (false, true)),
        ("(xi)", cavity_with("store_fields = [\"p\"]"), none),
        (
            "(xii)",
            piso_with("store_interval = 0\nstore_time_interval = 0.05"),
            (true, false),
        ),
    ];
    for (id, text, want) in &table {
        let got = read(text);
        let got_json = read(&json_of(text));
        eprintln!("DV1 {id}: {got:?}; JSON {got_json:?}");
        assert_eq!(&got, want, "{id}");
        assert_eq!(&got_json, want, "{id} (JSON)");
    }
    // (i)-(vii): exactly the plan of the same case without the store keys
    let here = Path::new(".");
    for (id, text, _) in &table[..7] {
        let bare = if text.contains("piso") {
            piso()
        } else {
            f834::RUN_CASE.to_string()
        };
        let want = plan(&bare, here);
        assert_eq!(
            plan(text, here),
            want,
            "{id}: not the plan of the case without the store keys"
        );
        assert_eq!(
            plan(&json_of(text), here),
            want,
            "{id} (JSON): not the plan of the case without the store keys"
        );
    }
}

/// Whether the tree of a case text names dotted key `key` (the rule before ADR 0152: a key in the text is the case's
/// own, whatever its value).
fn names(tree: &Value, key: &str) -> bool {
    key.split('.').try_fold(tree, |t, k| t.get(k)).is_some()
}

/// The plan of `text` by the rule before ADR 0152, made from `now` (ADR 0152's plan of the same text): an interval key
/// in the text means no store, and a precision key in the text means no precision entry. Every other part of the rule
/// is the one of [`Plan::of`].
fn before(text: &str, now: &Plan) -> Plan {
    let tree: Value = if text.trim_start().starts_with('{') {
        serde_json::from_str(text).unwrap()
    } else {
        toml::from_str(text).unwrap()
    };
    let sets_precision = names(&tree, PRECISION_KEY);
    if INTERVAL_KEYS.iter().any(|k| names(&tree, k)) {
        return Plan {
            every: None,
            note: Some(OWN.into()),
            sets_precision,
            ..now.clone()
        };
    }
    Plan {
        sets_precision,
        ..now.clone()
    }
}

/// What a run sees of a plan: the `--set` entries, the snapshot count, the size estimate and the note (the engine part
/// of `saved_store_r.rs::Seen`).
type Seen = (Vec<String>, Option<u64>, Option<u64>, Option<String>);

fn seen(p: &Plan) -> Seen {
    (p.entries(), p.snapshots(), p.estimated_bytes(), p.note.clone())
}

/// Replaces the engine part of
/// `crates/rcfd-studio/tests/saved_store_r.rs::dv5_explicit_non_default_store_keys_behave_as_before` (ADR 0152 DV5): a
/// case with explicit non-default store keys plans as it did before ADR 0152. The Studio compared its two routes
/// (`PlanRoute::default()` against `default_store_keys_set`); the engine has one rule, so the route before is written
/// here as the rule it was ([`before`]: a store key named in the text is the case's own) and compared with
/// `rcfd_workspace::plan`: (a) to (f), (g) (the Studio's case-editor save, here `rcfd-api`'s writer `Case::to_toml`, every
/// key spelled out) and (h) the thirteen cases of `example-projects/`, whose whole plan must agree. Negative control as
/// DV5's: the saved cavity (store keys at their defaults) plans a store now and none by the rule before. The Studio's
/// dialog lines, settings, warnings and counts were Studio code, not ported.
#[test]
fn dv5_explicit_non_default_store_keys_behave_as_before() {
    let g = {
        let mut c = Case::parse_toml(f834::RUN_CASE).unwrap();
        c.set("output.store_interval", json!(7)).unwrap();
        c.to_toml()
    };
    let gt: Value = toml::from_str(&g).unwrap();
    assert!(
        gt["output"]["store_interval"].as_u64() == Some(7) && gt["output"]["store_precision"] == json!("f64"),
        "set-up (g): {}",
        gt["output"]
    );
    let mut texts: Vec<(String, String, PathBuf)> = vec![
        ("(a)".into(), cavity_with("store_interval = 7"), ".".into()),
        ("(b)".into(), cavity_with("store_fields = [\"p\"]"), ".".into()),
        ("(c)".into(), cavity_with("store_precision = \"f32\""), ".".into()),
        (
            "(d)".into(),
            cavity_with("store_interval = 5\nstore_time_interval = 0.0"),
            ".".into(),
        ),
        ("(e)".into(), piso_with("store_time_interval = 0.05"), ".".into()),
        (
            "(f)".into(),
            piso_with("store_interval = 0\nstore_time_interval = 0.05"),
            ".".into(),
        ),
        ("(g)".into(), g, ".".into()),
    ];
    let root = f834::repo();
    let mut projects: Vec<PathBuf> = std::fs::read_dir(root.join("example-projects"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("cases").is_dir())
        .collect();
    projects.sort();
    for p in projects {
        for (rel, path) in f834::files_under(&p.join("cases"), "toml") {
            texts.push((
                format!("(h) {rel}"),
                std::fs::read_to_string(&path).unwrap(),
                path.parent().unwrap().to_path_buf(),
            ));
        }
    }
    assert_eq!(texts.len(), 20, "7 texts and the thirteen cases of example-projects/");
    for (id, text, base) in &texts {
        let a = plan(text, base);
        let b = before(text, &a);
        eprintln!("DV5 {id}: {:?}", seen(&a));
        assert_eq!(seen(&a), seen(&b), "{id}: differs from the rule before");
        if id.starts_with("(h)") {
            assert_eq!(a, b, "{id}: the whole plan differs from the rule before");
        }
    }
    // negative control: the saved cavity's entries differ between the rules
    let saved = {
        let mut c = Case::parse_toml(&f834::cavity(30)).unwrap();
        c.set("physics.nu", json!(0.02)).unwrap();
        c.to_toml()
    };
    let a = plan(&saved, Path::new("."));
    let b = before(&saved, &a);
    eprintln!(
        "DV5 control: the saved cavity: entries {:?} against the rule before {:?}",
        a.entries(),
        b.entries()
    );
    assert_ne!(a.entries(), b.entries());
    assert!(b.entries().is_empty());
}
