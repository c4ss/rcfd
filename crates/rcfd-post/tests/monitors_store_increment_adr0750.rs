//! F-854 (ADR 0750; ADR 0353 amendment 4): what storing an increment of 100 samples per series costs a `MonitorStore`
//! that holds 1e5. I2 of F-456 in its corrected set-up, a store with spare capacity (limit 2 ms on the median), and the
//! increment that crosses the end of the capacity, which is reported without a limit.

use rcfd_post::monitors::MonitorStore;
use std::time::Instant;

const NAMES: [&str; 4] = ["a", "b", "c", "d"];

/// Four series of `len` samples each, built by pushes.
fn grown(len: u64) -> MonitorStore {
    let mut st = MonitorStore::new("iteration");
    for n in NAMES {
        for i in 1..=len {
            st.push(n, "", i as f64, 1.0 / i as f64);
        }
    }
    st
}

/// The capacities of the eight vectors (abscissa and ordinate of each series).
fn capacities(st: &MonitorStore) -> [usize; 8] {
    let mut c = [0; 8];
    for (k, n) in NAMES.iter().enumerate() {
        let s = st.get(n).unwrap();
        c[2 * k] = s.x.capacity();
        c[2 * k + 1] = s.y.capacity();
    }
    c
}

fn assert_lengths(st: &MonitorStore, len: usize) {
    for n in NAMES {
        let s = st.get(n).unwrap();
        assert_eq!((s.x.len(), s.y.len()), (len, len), "series {n}");
    }
}

/// Samples 100,001 to 100,100 of each of the four series: the timed increment, in milliseconds.
fn timed_increment(st: &mut MonitorStore) -> f64 {
    let t = Instant::now();
    for i in 100_001..=100_100u64 {
        for n in NAMES {
            st.push(n, "", i as f64, 1.0);
        }
    }
    t.elapsed().as_secs_f64() * 1e3
}

/// I2 (ADR 0750 D1): the store holds 1e5 samples per series and its vectors have spare capacity, as those of a store
/// that grew by pushes have in all but one increment per doubling. The reallocation that the clone forces is made by
/// the push of sample 100,000, outside the timer. Median of 200 repeats, limit 2 ms.
#[test]
fn f854_i2_storing_an_increment_of_100_samples_at_1e5_within_the_capacity() {
    let base = grown(99_999);
    let mut ms = Vec::new();
    for _ in 0..200 {
        let mut st = base.clone();
        for n in NAMES {
            st.push(n, "", 100_000.0, 1.0);
        }
        assert_lengths(&st, 100_000);
        let before = capacities(&st);
        assert!(
            before.iter().all(|&c| c >= 100_100),
            "the increment would cross the end of a capacity: {before:?}"
        );
        ms.push(timed_increment(&mut st));
        assert_lengths(&st, 100_100);
        assert_eq!(capacities(&st), before, "a vector was reallocated inside the timer");
    }
    ms.sort_by(f64::total_cmp);
    let (best, med, max) = (ms[0], ms[99], ms[199]);
    println!(
        "I2 (ADR 0750) push of 100 samples x 4 series at 1e5, within the capacity: best {best:.4} ms, median {med:.4} ms, max {max:.4} ms"
    );
    assert!(med <= 2.0, "I2: median {med} ms above 2 ms");
}

/// Reported, no limit (ADR 0750 D2): the same increment onto a clone, whose eight vectors are full, so each of them is
/// reallocated inside the timer. This is the set-up of the old I2 of `monitors_filtered_f456.rs` and the worst case of
/// an increment that crosses the end of the capacity. No time is asserted, because the time is not that of
/// `MonitorStore::push`: about 99 percent of it is the system allocator handing out eight blocks of 1.6 MB, and it rose
/// fourfold under a memory load with the code unchanged (ADR 0749, D-I2 and B-I2). By the maintainer's answer of
/// 2026-11-20 (ADR 0353) the crossing is reported and the limit of 2 ms is on the test above.
#[test]
fn f854_i2x_the_increment_that_crosses_the_capacity_at_1e5_is_reported() {
    let base = grown(100_000);
    let mut ms = Vec::new();
    for _ in 0..200 {
        let mut st = base.clone();
        assert_lengths(&st, 100_000);
        let before = capacities(&st);
        ms.push(timed_increment(&mut st));
        assert_lengths(&st, 100_100);
        let after = capacities(&st);
        assert!(
            before.iter().zip(&after).all(|(b, a)| a != b),
            "this test no longer measures a crossing of the capacity: {before:?} then {after:?}"
        );
    }
    ms.sort_by(f64::total_cmp);
    let (best, med, max) = (ms[0], ms[99], ms[199]);
    println!(
        "I2x (ADR 0750, no limit) push of 100 samples x 4 series at 1e5, across the end of the capacity: best {best:.4} ms, median {med:.4} ms, max {max:.4} ms"
    );
}
