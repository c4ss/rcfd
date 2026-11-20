//! F-456 (ADR 0353): E1 (`to_json_filtered` and `write_json_filtered` against the old algorithm) and I2 (storing an
//! increment of 100 samples at 1e5). No tolerance: the documents are equal as values and as text.

use rcfd_post::monitors::MonitorStore;
use serde_json::{json, Value};
use std::time::Instant;

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

const POOL: [&str; 8] = [
    "res_u",
    "res_p",
    "a\"b\\c\n\t",
    "\u{fc}n\u{ef}",
    "",
    "force x",
    "zz",
    "\u{1f600}\u{7f}",
];

fn random_store(g: &mut Lcg) -> MonitorStore {
    let mut st = MonitorStore::new(["iteration", "time", "t \"x\"\n"][g.below(3) as usize]);
    let n = g.below(9) as usize; // 0 to 8 series
    for (k, name) in POOL.iter().enumerate().take(n) {
        let unit = ["", "Pa", "m/s \"q\"", "\u{b0}C"][g.below(4) as usize];
        let len = g.below(301);
        let mut x = 0.0f64;
        let unsorted = k == 3;
        for _ in 0..len {
            x = if unsorted {
                if g.below(9) == 0 {
                    f64::NAN
                } else {
                    g.unit() * 100.0
                }
            } else {
                x + g.below(3) as f64 * 0.5
            };
            st.push(name, unit, x, y_value(g));
        }
    }
    st
}

/// The old arm of `monitors.get` (`to_json`, `retain`, `since`).
fn reference(store: &MonitorStore, names: Option<&[&str]>, since: Option<f64>) -> Value {
    let mut doc = store.to_json();
    if let (Some(series), true) = (doc["series"].as_object_mut(), names.is_some() || since.is_some()) {
        series.retain(|k, _| names.map_or(true, |n| n.contains(&k.as_str())));
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

#[test]
fn f456_e1_filtered_documents_equal_the_old_algorithm() {
    let mut g = Lcg(0xF456_0001);
    let mut distinct_sizes = std::collections::BTreeSet::new();
    for case in 0..3000 {
        let st = random_store(&mut g);
        let all: Vec<f64> = st
            .names()
            .iter()
            .flat_map(|n| st.get(n).unwrap().x.clone())
            .filter(|x| x.is_finite())
            .collect();
        let names_owned: Vec<&str> = POOL
            .iter()
            .copied()
            .chain(["unknown"])
            .filter(|_| g.below(2) == 0)
            .collect();
        let names: Option<&[&str]> = if g.below(3) == 0 { None } else { Some(&names_owned) };
        let since = match g.below(5) {
            0 => None,
            1 if !all.is_empty() => Some(all[g.below(all.len() as u64) as usize]),
            2 => Some(g.unit() * 150.0 - 10.0),
            3 => Some(if g.below(2) == 0 { -1e300 } else { 1e300 }),
            _ => Some((g.unit() * 150.0).floor()),
        };
        let expected = reference(&st, names, since);
        let got = st.to_json_filtered(names, since);
        assert_eq!(got, expected, "case {case}: Value");
        assert_eq!(got.to_string(), expected.to_string(), "case {case}: text of the Value");
        let mut bytes = Vec::new();
        st.write_json_filtered(&mut bytes, names, since);
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            expected.to_string(),
            "case {case}: written text"
        );
        distinct_sizes.insert(expected.to_string().len());
    }
    assert!(
        distinct_sizes.len() > 1000,
        "the cases must differ: {}",
        distinct_sizes.len()
    );
    // to_json is unchanged
    let st = random_store(&mut Lcg(7));
    assert_eq!(st.to_json(), st.to_json_filtered(None, None));
}

/// I2: appending 100 samples to each of four series that hold 1e5, median of 200 repeats, limit 2 ms.
#[test]
fn f456_i2_storing_an_increment_of_100_samples_at_1e5() {
    let mut g = Lcg(0xF456_0002);
    let mut base = MonitorStore::new("iteration");
    for k in 0..4 {
        for i in 1..=100_000u64 {
            base.push(["a", "b", "c", "d"][k], "", i as f64, y_value(&mut g));
        }
    }
    let mut ms = Vec::new();
    for _ in 0..200 {
        let mut st = base.clone();
        let t = Instant::now();
        for i in 100_001..=100_100u64 {
            for n in ["a", "b", "c", "d"] {
                st.push(n, "", i as f64, 1.0);
            }
        }
        ms.push(t.elapsed().as_secs_f64() * 1e3);
        assert_eq!(st.get("a").unwrap().x.len(), 100_100);
    }
    ms.sort_by(f64::total_cmp);
    let (med, max) = (ms[99], ms[199]);
    println!("I2 push of 100 samples x 4 series at 1e5: median {med:.4} ms, max {max:.4} ms");
    assert!(med <= 2.0, "I2: median {med} ms above 2 ms");
}
