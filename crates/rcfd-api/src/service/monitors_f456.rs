//! F-456 (ADR 0353): `monitors.get` replies are byte for byte what the unchanged code gave.
//!
//! The reference is the old algorithm written out here: the whole document by `MonitorStore::to_json`, the series kept by
//! `names`, the samples kept by `since` (`MonitorStore::since`). The requests are a fixed list of edge cases and a seeded
//! random set, on a store that a seeded generator fills (not the solver, so no solver change can move these bytes). The
//! constant `E3H` was recorded by running this file on the tree before the change.

use super::*;
use crate::sim::tests_support::CAVITY_TOML;
use rcfd_post::monitors::MonitorStore;

/// Seeded generator (64-bit LCG of Knuth's MMIX constants, high bits used).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 11
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn unit(&mut self) -> f64 {
        (self.next() as f64) / ((1u64 << 53) as f64)
    }
}

const SPECIAL: [f64; 10] = [
    f64::NAN,
    f64::INFINITY,
    f64::NEG_INFINITY,
    -0.0,
    0.0,
    5e-324,
    1e308,
    -1e308,
    1.0 / 3.0,
    123_456_789.123_456_79,
];

fn y_value(g: &mut Lcg) -> f64 {
    match g.below(8) {
        0 => SPECIAL[g.below(SPECIAL.len() as u64) as usize],
        1 => f64::from_bits(g.next() << 11 | g.below(2048)),
        _ => (g.unit() - 0.5) * 10f64.powi(g.below(30) as i32 - 15),
    }
}

/// A store of `n_series` series (names with JSON escapes and non-ASCII, one empty, one unsorted) of up to `max_len` samples.
fn recorded_store(seed: u64, n_series: usize, max_len: u64) -> MonitorStore {
    let mut g = Lcg(seed);
    let pool = ["res_u", "res_p", "a\"b\\c\n\t", "\u{fc}n\u{ef}", "", "force x", "zz"];
    let doc = json!({"format": "rcfd-monitors", "version": 1, "x_label": "iteration \"x\"",
        "series": {"empty": {"unit": "u\u{b5}", "x": [], "y": []}}});
    let mut st = MonitorStore::from_json(&doc).unwrap();
    for (k, name) in pool.iter().take(n_series).enumerate() {
        let unit = if k % 2 == 0 { "" } else { "m/s \"q\"" };
        let len = 1 + g.below(max_len);
        let mut x = 0.0f64;
        for _ in 0..len {
            x = if k == 3 {
                // unsorted, with NaN: the old code is defined on any input
                if g.below(9) == 0 {
                    f64::NAN
                } else {
                    g.unit() * 100.0
                }
            } else {
                x + g.below(3) as f64 * 0.5 // ties and gaps
            };
            let y = y_value(&mut g);
            st.push(name, unit, x, y);
        }
    }
    st
}

/// The old arm of `monitors.get`, verbatim in effect.
fn reference(store: &MonitorStore, p: &Value) -> Value {
    let since = p.get("since").and_then(Value::as_f64);
    let names: Option<Vec<&str>> = p
        .get("names")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect());
    let mut doc = store.to_json();
    if let (Some(series), true) = (doc["series"].as_object_mut(), names.is_some() || since.is_some()) {
        series.retain(|k, _| names.as_ref().is_none_or(|n| n.contains(&k.as_str())));
        if let Some(after) = since {
            for (k, v) in series.iter_mut() {
                if let Some(sl) = store.since(k, after) {
                    *v = json!({"unit": sl.unit, "x": sl.x, "y": sl.y});
                }
            }
        }
    }
    doc
}

fn fnv1a(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h ^= u64::from(*b);
        *h = h.wrapping_mul(0x100000001b3);
    }
}

fn session_with(store: MonitorStore) -> (Arc<Service>, String) {
    let svc = Service::new(ServiceConfig::default());
    let r = svc.call("session.create", &json!({"toml": CAVITY_TOML})).unwrap();
    let id = r["session"].as_str().unwrap().to_string();
    svc.call("run.initialize", &json!({"session": id})).unwrap();
    let s = svc.session(&json!({"session": id})).unwrap();
    let sim = svc.sim(&s).unwrap();
    lock(&sim).monitors = store;
    (svc, id)
}

/// Fixed edge cases and seeded random requests (without the session member, added by the caller).
fn requests(store: &MonitorStore, seed: u64, n_random: usize) -> Vec<Value> {
    let xs: Vec<f64> = store
        .names()
        .iter()
        .flat_map(|n| store.get(n).unwrap().x.clone())
        .filter(|x| x.is_finite())
        .collect();
    let (lo, hi) = xs.iter().fold((f64::MAX, f64::MIN), |(a, b), &x| (a.min(x), b.max(x)));
    let mut out = vec![
        json!({}),
        json!({"names": []}),
        json!({"names": ["res_u"]}),
        json!({"names": ["nope"]}),
        json!({"names": [1, "res_u", null, ["res_p"]]}),
        json!({"names": "res_u"}),
        json!({"names": null}),
        json!({"names": {"res_u": 1}}),
        json!({"since": 0}),
        json!({"since": -1}),
        json!({"since": 1e300}),
        json!({"since": -1e300}),
        json!({"since": "5"}),
        json!({"since": null}),
        json!({"since": true}),
        json!({"since": [1]}),
        json!({"since": 3}),
        json!({"since": 2.5}),
        json!({"since": hi}),
        json!({"since": lo}),
        json!({"since": hi + 1.0}),
        json!({"since": lo - 1.0}),
        json!({"names": ["res_u", "res_p"], "since": 2}),
        json!({"names": [], "since": 2}),
        json!({"names": ["nope"], "since": 2}),
        json!({"names": ["empty"]}),
        json!({"names": ["empty"], "since": 1}),
        json!({"names": ["a\"b\\c\n\t", "\u{fc}n\u{ef}", ""]}),
        json!({"names": ["a\"b\\c\n\t", "\u{fc}n\u{ef}", ""], "since": 7.25}),
        json!({"names": ["res_u"], "since": 4, "other": 1, "x": [1]}),
        json!({"other": 1}),
        json!({"names": ["RES_U"]}),
        json!({"names": ["res_u", "res_u", "res_u"]}),
        json!({"since": 0, "names": ["zz"]}),
    ];
    let pool: Vec<String> = store.names().iter().map(|s| s.to_string()).collect();
    let mut g = Lcg(seed);
    for _ in 0..n_random {
        let mut p = serde_json::Map::new();
        if g.below(3) != 0 {
            let mut names: Vec<Value> = pool.iter().filter(|_| g.below(2) == 0).map(|n| json!(n)).collect();
            if g.below(4) == 0 {
                names.push(json!("unknown"));
            }
            if g.below(6) == 0 {
                names.push(json!(g.below(5)));
            }
            p.insert("names".into(), Value::Array(names));
        }
        match g.below(5) {
            0 => {}
            1 => {
                p.insert("since".into(), json!(xs[g.below(xs.len() as u64) as usize]));
            }
            2 => {
                p.insert("since".into(), json!(lo + (hi - lo) * g.unit()));
            }
            3 => {
                p.insert("since".into(), json!((lo + (hi - lo) * g.unit()).floor()));
            }
            _ => {
                p.insert("since".into(), json!(if g.below(2) == 0 { lo - 5.0 } else { hi + 5.0 }));
            }
        }
        out.push(Value::Object(p));
    }
    out
}

fn with_session(p: &Value, id: &str) -> Value {
    let mut q = p.clone();
    if let Some(m) = q.as_object_mut() {
        m.insert("session".into(), json!(id));
    }
    q
}

/// The line `serve_stream` writes for the envelope `req`.
fn wire(svc: &Arc<Service>, req: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    svc.serve_stream(format!("{req}\n").as_bytes(), &mut out).unwrap();
    out
}

fn result_line(id: &Value, result: &Value) -> Vec<u8> {
    let mut t = json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string();
    t.push('\n');
    t.into_bytes()
}

/// FNV-1a 64 of the concatenated wire lines of E3, recorded on the tree before the change.
const E3H: u64 = 0xa918e7df5af7a64a;

#[test]
fn f456_e2_e3_replies_equal_the_old_algorithm_byte_for_byte() {
    let store = recorded_store(0xF456_0353, 7, 700);
    let copy = store.clone();
    let (svc, id) = session_with(store);
    let reqs = requests(&copy, 0x0353_F456, 560);
    assert_eq!(reqs.len(), 34 + 560);
    let mut h = 0xcbf29ce484222325u64;
    let (mut n_ok, mut n_empty_series) = (0, 0);
    for (i, p) in reqs.iter().enumerate() {
        let expected = reference(&copy, p);
        if expected["series"].as_object().unwrap().is_empty() {
            n_empty_series += 1;
        }
        let q = with_session(p, &id);
        // E2: the in-process value
        let got = svc.call("monitors.get", &q).unwrap();
        assert_eq!(got.to_string(), expected.to_string(), "E2 request {i}: {p}");
        assert_eq!(got, expected, "E2 request {i}: {p}");
        // E3: the wire line; the id kinds cycle over integer, string, null and fractional
        let rid = match i % 4 {
            0 => json!(i),
            1 => json!(format!("req-{i}")),
            2 => Value::Null,
            _ => json!(i as f64 + 0.5),
        };
        let req = json!({"jsonrpc": "2.0", "id": rid, "method": "monitors.get", "params": q});
        let line = wire(&svc, &req);
        assert_eq!(line, result_line(&rid, &expected), "E3 request {i}: {p}");
        fnv1a(&mut h, &line);
        n_ok += 1;
    }
    // the error paths and the notification: the same bytes as before (covered by the hash), and the right kind
    for (i, params) in [
        json!({}),
        json!({"session": "nope"}),
        json!({"session": 7, "names": ["res_u"]}),
    ]
    .iter()
    .enumerate()
    {
        let req = json!({"jsonrpc": "2.0", "id": 900 + i, "method": "monitors.get", "params": params});
        let line = wire(&svc, &req);
        let v: Value = serde_json::from_slice(&line).unwrap();
        assert!(v.get("error").is_some() && v.get("result").is_none(), "{v}");
        assert!(svc.call("monitors.get", params).is_err());
        fnv1a(&mut h, &line);
    }
    let note = json!({"jsonrpc": "2.0", "method": "monitors.get", "params": {"session": id}});
    assert!(wire(&svc, &note).is_empty(), "a notification gets no reply");
    let notbad = json!({"jsonrpc": "2.0", "method": "monitors.get", "params": {"session": "nope"}});
    assert!(wire(&svc, &notbad).is_empty());
    let wrong = json!({"jsonrpc": "1.0", "id": 5, "method": "monitors.get", "params": {"session": id}});
    fnv1a(&mut h, &wire(&svc, &wrong));
    // the same series are asked for twice on one connection: the lines come one per request
    let two = format!(
        "{}\n{}\n",
        json!({"jsonrpc": "2.0", "id": 1, "method": "monitors.get", "params": {"session": id, "names": ["res_u"]}}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "monitors.get", "params": {"session": id, "since": 3}})
    );
    let mut out = Vec::new();
    svc.serve_stream(two.as_bytes(), &mut out).unwrap();
    assert_eq!(out.iter().filter(|b| **b == b'\n').count(), 2);
    fnv1a(&mut h, &out);
    println!("F456 E3 hash {h:#018x}; {n_ok} requests, {n_empty_series} with no series");
    assert!(
        n_empty_series > 5,
        "the request set must reach the empty-selection case"
    );
    assert_eq!(h, E3H, "the lines differ from the lines of the tree before the change");
}

/// N1 and N2: the comparison used above does see a one-sample and a one-byte difference.
#[test]
fn f456_n1_n2_the_comparison_sees_a_difference() {
    let store = recorded_store(0xF456_0353, 7, 700);
    let reqs = requests(&store, 0x0353_F456, 560);
    let wrong_ref = |p: &Value| {
        // `since` as x >= after
        let mut doc = reference(&store, p);
        if let Some(after) = p.get("since").and_then(Value::as_f64) {
            let names = p.get("names").and_then(Value::as_array);
            for (k, v) in doc["series"].as_object_mut().unwrap().iter_mut() {
                let _ = names;
                let s = store.get(k).unwrap();
                let start = s.x.partition_point(|&v| v < after);
                *v = json!({"unit": s.unit, "x": &s.x[start..], "y": &s.y[start..]});
            }
        }
        doc
    };
    let differ = reqs
        .iter()
        .filter(|p| wrong_ref(p) != reference(&store, p))
        .count();
    assert!(differ > 0, "N1: a reference with x >= since must differ somewhere");
    let line = result_line(&json!(1), &reference(&store, &reqs[2]));
    let mut bad = line.clone();
    bad[line.len() / 2] ^= 1;
    assert_ne!(line, bad, "N2");
}

/// E3b: the full history of four series of 1e5 samples through the wire.
#[test]
fn f456_e3b_four_series_of_1e5_samples_through_the_wire() {
    let mut g = Lcg(0x0353_0456);
    let mut st = MonitorStore::new("iteration");
    for k in 0..4 {
        let name = ["res_p", "res_u", "res_v", "res_w"][k];
        for i in 1..=100_000u64 {
            st.push(name, "", i as f64, y_value(&mut g));
        }
    }
    let copy = st.clone();
    let (svc, id) = session_with(st);
    let names = json!(["res_p", "res_u", "res_v", "res_w"]);
    for p in [
        json!({"names": names}),
        json!({"names": names, "since": 99_900}),
        json!({"since": 0.5}),
    ] {
        let expected = reference(&copy, &p);
        let rid = json!(77);
        let req = json!({"jsonrpc": "2.0", "id": rid, "method": "monitors.get", "params": with_session(&p, &id)});
        let line = wire(&svc, &req);
        assert!(line == result_line(&rid, &expected), "E3b {p}");
    }
}
