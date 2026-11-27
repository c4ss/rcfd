//! ADR 0827 (F-879): check H's class of ADR 0183 read from the committed records of ADR 0824 (F-885), with the torque and
//! the total-to-static pressure rise in time: mean, half range, frequency by upward crossings (ADR 0766 D-5) and by the
//! periodogram of the signal less its least-squares line, their decay over the six flow-through times, the comparison of
//! dt and dt/2, of the steady B2 run of ADR 0762 and of FAN-01's measured design point. No solve: the tests read files.

use std::f64::consts::PI;
use std::path::{Path, PathBuf};

/// Time step of H(dt) [s] (ADR 0183 D5).
const H_DT: f64 = 4.605e-5;
/// Steps of H(dt).
const H_STEPS: usize = 5498;
/// The window is the rows with `t` above this [s] (ADR 0766).
const WINDOW_START: f64 = 0.1266;
/// STEADY threshold of the site amplitude [m/s] (ADR 0183, 0.01 Omega r_hub).
const A_STEADY: f64 = 0.193;
/// Outer-iteration rule of ADR 0183 as written (ADR 0767 E-1).
const OUTER_RATIO: f64 = 1e-2;
const OUTER_SHARE: f64 = 0.99;
/// The tolerance of the state (ADR 0762): T1, T2, T3.
const STATE_TOL: f64 = 0.01;
/// The frequency band of ADR 0183, check H (row TF).
const F_BAND: (f64, f64) = (0.9, 1.1);
/// The steady B2 run of ADR 0762 (R1).
const B2_TORQUE: f64 = 2.028692;
const B2_DP: f64 = 64.2329;
/// The numerical uncertainty of the B1/B2/B3 family (ADR 0762, the band arithmetic): T4.
const U_TORQUE: f64 = 0.21190;
const U_DP: f64 = 29.75;
/// FAN-01's design point (`docs/validation-data.md`): T5, reported only.
const MEAS_DP: f64 = 126.46;
const MEAS_POWER: f64 = 333.30;
const RPM: f64 = 1486.0;
/// G2: the same files read twice.
const REC_TOL: f64 = 1e-6;

/// The rows as first computed (G2): per run, `key(..)`. `None` before the registered run.
const RECORDED: Option<[[f64; KEYS]; 2]> = None;
const KEYS: usize = 14;

fn omega() -> f64 {
    RPM * 2.0 * PI / 60.0
}

/// One step of a record: the time, the six site speeds, torque, pressure rise and, in ADR 0824's form, `d1 .. d5`.
#[derive(Debug, Clone)]
struct Rec {
    step: usize,
    t: f64,
    w: [f64; 6],
    torque: f64,
    dp: f64,
    dv: Option<[f64; 5]>,
}

fn read(path: &Path, cols: usize) -> Vec<Rec> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("step"))
        .map(|l| {
            let c: Vec<f64> = l
                .split(',')
                .map(|v| v.trim().parse::<f64>().unwrap_or_else(|e| panic!("{l}: {e}")))
                .collect();
            assert_eq!(c.len(), cols, "{l}");
            Rec {
                step: c[0] as usize,
                t: c[1],
                w: [c[2], c[3], c[4], c[5], c[6], c[7]],
                torque: c[9],
                dp: c[10],
                dv: (cols == 21).then(|| [c[15], c[16], c[17], c[18], c[19]]),
            }
        })
        .collect()
}

fn adr0824(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/investigations/f885-kept-gradient-lag")
        .join(name)
}

fn adr0766(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/investigations/f879-fan01-check-h")
        .join(name)
}

/// The measures of one signal over the window (ADR 0827 section 2).
#[derive(Debug, Clone, Copy)]
struct Meas {
    mean: f64,
    amp: f64,
    /// `amp / abs(mean)`
    rel: f64,
    crossings: usize,
    /// `(n - 1) / (t_n - t_1)` of the upward crossings of the mean-removed signal; NaN with fewer than two
    fx: f64,
    /// dominant frequency of the periodogram of the signal less its least-squares line; NaN for a constant
    fp: f64,
    /// share of the largest bin in the sum of the periodogram
    share: f64,
    /// change of the least-squares line across the window
    drift: f64,
}

fn half_range(x: &[f64]) -> f64 {
    let (lo, hi) = x
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| (a.min(v), b.max(v)));
    0.5 * (hi - lo)
}

/// The signal less its least-squares line in the index, and the line's change across the signal.
fn detrend(x: &[f64]) -> (Vec<f64>, f64) {
    let n = x.len() as f64;
    let im = (n - 1.0) / 2.0;
    let xm = x.iter().sum::<f64>() / n;
    let (mut sxy, mut sxx) = (0.0f64, 0.0f64);
    for (i, &v) in x.iter().enumerate() {
        let d = i as f64 - im;
        sxy += d * (v - xm);
        sxx += d * d;
    }
    let b = sxy / sxx;
    let r = x
        .iter()
        .enumerate()
        .map(|(i, &v)| v - xm - b * (i as f64 - im))
        .collect();
    (r, b * (n - 1.0))
}

/// The index `k*` of the largest `P_k`, `k = 1 .. n / 2`, and its share of the sum; `(0, NaN)` for a zero signal.
fn periodogram_peak(y: &[f64]) -> (usize, f64) {
    let n = y.len();
    let (mut best, mut kbest, mut sum) = (0.0f64, 0usize, 0.0f64);
    for k in 1..=n / 2 {
        let w = 2.0 * PI * k as f64 / n as f64;
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, &v) in y.iter().enumerate() {
            let (s, c) = (w * i as f64).sin_cos();
            re += v * c;
            im -= v * s;
        }
        let p = re * re + im * im;
        sum += p;
        if p > best {
            best = p;
            kbest = k;
        }
    }
    (kbest, if kbest == 0 { f64::NAN } else { best / sum })
}

fn measure(t: &[f64], x: &[f64], dt: f64) -> Meas {
    let n = x.len();
    assert!(n >= 4 && t.len() == n);
    let mean = x.iter().sum::<f64>() / n as f64;
    let mut cross: Vec<f64> = Vec::new();
    for i in 1..n {
        let (a, b) = (x[i - 1] - mean, x[i] - mean);
        if a < 0.0 && b >= 0.0 {
            cross.push(t[i - 1] + (t[i] - t[i - 1]) * (-a) / (b - a));
        }
    }
    let fx = if cross.len() >= 2 {
        (cross.len() - 1) as f64 / (cross[cross.len() - 1] - cross[0])
    } else {
        f64::NAN
    };
    let (y, drift) = detrend(x);
    let (k, share) = periodogram_peak(&y);
    let length = n as f64 * dt;
    let amp = half_range(x);
    Meas {
        mean,
        amp,
        rel: amp / mean.abs(),
        crossings: cross.len(),
        fx,
        fp: if k == 0 { f64::NAN } else { k as f64 / length },
        share,
        drift,
    }
}

/// T1, T2: the signal has no oscillation above the tolerance of the state.
fn steady_to_state(m: &Meas) -> bool {
    m.rel <= STATE_TOL
}

fn window(r: &[Rec]) -> Vec<&Rec> {
    r.iter().filter(|x| x.t > WINDOW_START).collect()
}

/// Half range of `f` over each flow-through time `j = 1 .. 6` of the whole run (step in `((j - 1) N / 6, j N / 6]`).
fn decay(r: &[Rec], steps: usize, f: impl Fn(&Rec) -> f64) -> [f64; 6] {
    let mut out = [0.0; 6];
    for (j, o) in out.iter_mut().enumerate() {
        let seg: Vec<f64> = r
            .iter()
            .filter(|x| (6 * x.step).div_ceil(steps) == j + 1)
            .map(&f)
            .collect();
        *o = half_range(&seg);
    }
    out
}

/// One run of check H over its window.
#[derive(Debug, Clone)]
struct Run {
    dt: f64,
    rows: usize,
    complete: bool,
    window: usize,
    share: f64,
    cell: usize,
    site: Meas,
    torque: Meas,
    dp: Meas,
    decay_torque: [f64; 6],
    decay_dp: [f64; 6],
}

fn run(r: &[Rec], dt: f64, steps: usize) -> Run {
    let complete = r.len() == steps && r.iter().enumerate().all(|(i, x)| x.step == i + 1);
    let win = window(r);
    let t: Vec<f64> = win.iter().map(|x| x.t).collect();
    let col = |c: usize| -> Vec<f64> { win.iter().map(|x| x.w[c]).collect() };
    let cell = (0..6)
        .max_by(|&a, &b| half_range(&col(a)).partial_cmp(&half_range(&col(b))).unwrap())
        .unwrap();
    let share = win
        .iter()
        .filter(|x| x.dv.is_some_and(|d| d[4] <= OUTER_RATIO * d[0]))
        .count() as f64
        / win.len() as f64;
    Run {
        dt,
        rows: r.len(),
        complete,
        window: win.len(),
        share,
        cell,
        site: measure(&t, &col(cell), dt),
        torque: measure(&t, &win.iter().map(|x| x.torque).collect::<Vec<f64>>(), dt),
        dp: measure(&t, &win.iter().map(|x| x.dp).collect::<Vec<f64>>(), dt),
        decay_torque: decay(r, steps, |x| x.torque),
        decay_dp: decay(r, steps, |x| x.dp),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    NotEvaluable,
    Steady,
    NotSteady,
}

/// The first two clauses of ADR 0183's class, in order (row G).
fn class(a: &Run, b: &Run) -> Class {
    if !a.complete || !b.complete || a.share < OUTER_SHARE || b.share < OUTER_SHARE {
        Class::NotEvaluable
    } else if a.site.amp <= A_STEADY && b.site.amp <= A_STEADY {
        Class::Steady
    } else {
        Class::NotSteady
    }
}

fn key(r: &Run) -> [f64; KEYS] {
    [
        r.share,
        r.site.amp,
        r.site.fp,
        r.torque.mean,
        r.torque.amp,
        r.torque.fx,
        r.torque.fp,
        r.torque.drift,
        r.dp.mean,
        r.dp.amp,
        r.dp.fx,
        r.dp.fp,
        r.decay_torque[5],
        r.decay_dp[5],
    ]
}

fn print_meas(label: &str, m: &Meas) {
    println!(
        "  {label}: mean {:.7e}; A {:.4e} (peak-to-peak {:.4e}, {:.4e} of the mean); crossings {}; f_x {:.2} Hz; f_p {:.2} Hz (share {:.4}); drift across the window {:.4e}",
        m.mean, m.amp, 2.0 * m.amp, m.rel, m.crossings, m.fx, m.fp, m.share, m.drift
    );
}

fn sci(v: &[f64]) -> String {
    let x: Vec<String> = v.iter().map(|a| format!("{a:.4e}")).collect();
    format!("[{}]", x.join(", "))
}

fn print_run(label: &str, r: &Run) {
    let sites = [7, 57, 107, 157, 621, 1235];
    println!(
        "F879-HC {label}: dt {:e} s; rows {} (complete {}); window {} steps, length {:.6} s, resolution {:.4} Hz, highest {:.1} Hz; outer rule share {:.6}",
        r.dt,
        r.rows,
        r.complete,
        r.window,
        r.window as f64 * r.dt,
        1.0 / (r.window as f64 * r.dt),
        0.5 / r.dt,
        r.share
    );
    print_meas(&format!("site c{} [m/s]", sites[r.cell]), &r.site);
    print_meas("torque [N m]", &r.torque);
    print_meas("dp_ts [Pa]", &r.dp);
    println!("  shaft power {:.4} W", omega() * r.torque.mean);
    println!("  torque A per flow-through time 1..6: {}", sci(&r.decay_torque));
    println!("  dp_ts A per flow-through time 1..6: {}", sci(&r.decay_dp));
    println!(
        "  last flow-through time: torque {:.4e}, dp_ts {:.4e} of the window mean",
        r.decay_torque[5] / r.torque.mean.abs(),
        r.decay_dp[5] / r.dp.mean.abs()
    );
}

/// `x` within half a unit of the last printed digit of `want`.
fn printed(what: &str, x: f64, want: f64, half_unit: f64) {
    println!("F879-HC {what}: {x:.9e} against the printed {want} (half unit {half_unit:e})");
    assert!((x - want).abs() <= half_unit, "{what}: {x} against {want}");
}

/// C1 (ADR 0827): the measures on known signals.
#[test]
fn f879_hc_measures_on_known_signals() {
    let n = 2749;
    let t: Vec<f64> = (0..n).map(|i| i as f64 * H_DT).collect();
    let length = n as f64 * H_DT;
    let sine: Vec<f64> = t.iter().map(|&x| 0.37 * (2.0 * PI * 17.0 * x / length).sin()).collect();
    // (a)
    let a = measure(&t, &sine, H_DT);
    println!(
        "C1 (a): f_p {:.12e} against {:.12e}; A {:.9e} against 0.37",
        a.fp,
        17.0 / length,
        a.amp
    );
    assert!((a.fp / (17.0 / length) - 1.0).abs() <= 1e-12, "(a) f_p {}", a.fp);
    assert!((a.amp / 0.37 - 1.0).abs() <= 1e-3, "(a) A {}", a.amp);
    // (b) the same with a line rising 10 x 0.37 across the window
    let tilted: Vec<f64> = sine
        .iter()
        .enumerate()
        .map(|(i, &v)| v + 3.7 * i as f64 / (n - 1) as f64)
        .collect();
    let b = measure(&t, &tilted, H_DT);
    println!("C1 (b): f_p {:.12e}; drift {:.6}", b.fp, b.drift);
    assert!((b.fp / (17.0 / length) - 1.0).abs() <= 1e-12, "(b) f_p {}", b.fp);
    // (c) the tolerance of the state, and the negative control N1
    let small: Vec<f64> = sine.iter().map(|&v| 2.0 + v * 0.001 / 0.37).collect();
    let large: Vec<f64> = sine.iter().map(|&v| 2.0 + v * 0.05 / 0.37).collect();
    let (cs, cl) = (measure(&t, &small, H_DT), measure(&t, &large, H_DT));
    println!("C1 (c): {:.4e} and {:.4e} of the mean", cs.rel, cl.rel);
    assert!(steady_to_state(&cs), "(c) 0.05 % is steady to the state");
    assert!(!steady_to_state(&cl), "(c) N1: 2.5 % is not steady to the state");
    // (d)
    let d = measure(&t, &vec![2.0; n], H_DT);
    assert_eq!(d.amp, 0.0);
}

/// K1, K2 (ADR 0827): the printed values of ADR 0766 and ADR 0824 from their records.
#[test]
fn f879_hc_reproduces_the_printed_values() {
    // K1: ADR 0766's records, 15 columns
    let want = [
        ("H(dt)", 2.007096, 2.83e-4, 42, 1009.5, 69.6172, 0.0212, 41, 395.1),
        ("H(dt/2)", 2.010156, 3.53e-4, 161, 1307.6, 70.0729, 0.0364, 106, 957.5),
    ];
    for ((name, dt, steps), w) in [("H_dt.csv", H_DT, H_STEPS), ("H_dt2.csv", 0.5 * H_DT, 2 * H_STEPS)]
        .into_iter()
        .zip(want)
    {
        let r = run(&read(&adr0766(name), 15), dt, steps);
        assert!(r.complete);
        printed(&format!("K1 {} torque mean", w.0), r.torque.mean, w.1, 5e-7);
        printed(&format!("K1 {} torque A", w.0), r.torque.amp, w.2, 5e-7);
        printed(
            &format!("K1 {} torque crossings", w.0),
            r.torque.crossings as f64,
            w.3 as f64,
            0.0,
        );
        printed(&format!("K1 {} torque f_x", w.0), r.torque.fx, w.4, 0.05);
        printed(&format!("K1 {} dp_ts mean", w.0), r.dp.mean, w.5, 5e-5);
        printed(&format!("K1 {} dp_ts A", w.0), r.dp.amp, w.6, 5e-5);
        printed(
            &format!("K1 {} dp_ts crossings", w.0),
            r.dp.crossings as f64,
            w.7 as f64,
            0.0,
        );
        printed(&format!("K1 {} dp_ts f_x", w.0), r.dp.fx, w.8, 0.05);
    }
    // K2: ADR 0824's records, 21 columns
    for ((name, dt, steps), (label, amp, power)) in
        [("H_dt.csv", H_DT, H_STEPS), ("H_dt2.csv", 0.5 * H_DT, 2 * H_STEPS)]
            .into_iter()
            .zip([("H(dt)", 1.2557e-3, 313.56), ("H(dt/2)", 2.2775e-3, 313.96)])
    {
        let r = run(&read(&adr0824(name), 21), dt, steps);
        printed(&format!("K2 {label} outer rule share"), r.share, 1.0, 5e-7);
        printed(&format!("K2 {label} site A"), r.site.amp, amp, 5e-8);
        printed(&format!("K2 {label} shaft power"), omega() * r.torque.mean, power, 5e-3);
    }
}

/// A1, G, T1 to T5, TF and G2 (ADR 0827): the rows from the committed records of ADR 0824.
#[test]
fn f879_hc_rows_from_the_committed_records() {
    let a = run(&read(&adr0824("H_dt.csv"), 21), H_DT, H_STEPS);
    let b = run(&read(&adr0824("H_dt2.csv"), 21), 0.5 * H_DT, 2 * H_STEPS);
    print_run("H(dt)", &a);
    print_run("H(dt/2)", &b);
    let c = class(&a, &b);
    println!(
        "F879-HC G: class by ADR 0183 {c:?}; site A {:.4e} and {:.4e} against {A_STEADY}",
        a.site.amp, b.site.amp
    );
    let t3 = [b.torque.mean / a.torque.mean - 1.0, b.dp.mean / a.dp.mean - 1.0];
    let rich = [
        b.torque.mean + (b.torque.mean - a.torque.mean) / 3.0,
        b.dp.mean + (b.dp.mean - a.dp.mean) / 3.0,
    ];
    println!(
        "F879-HC T1 torque {:.4e}, {:.4e}; T2 dp_ts {:.4e}, {:.4e} of the mean (limit {STATE_TOL})",
        a.torque.rel, b.torque.rel, a.dp.rel, b.dp.rel
    );
    println!(
        "F879-HC TF ratios (applies only if T1 or T2 is missed in both runs): torque f_p {:.4}, dp_ts f_p {:.4}",
        b.torque.fp / a.torque.fp,
        b.dp.fp / a.dp.fp
    );
    println!(
        "F879-HC T3 dt/2 against dt: torque {:.4e}, dp_ts {:.4e} (limit {STATE_TOL}); Richardson {:.7} N m, {:.5} Pa",
        t3[0], t3[1], rich[0], rich[1]
    );
    let t4 = [b.torque.mean - B2_TORQUE, b.dp.mean - B2_DP];
    println!(
        "F879-HC T4 dt/2 against steady B2: torque {:+.6} N m ({:+.4} %, limit {U_TORQUE}), dp_ts {:+.4} Pa ({:+.4} %, limit {U_DP})",
        t4[0],
        100.0 * t4[0] / B2_TORQUE,
        t4[1],
        100.0 * t4[1] / B2_DP
    );
    let power = omega() * b.torque.mean;
    println!(
        "F879-HC T5 dt/2 against FAN-01: power {power:.3} W against {MEAS_POWER} ({:+.3} %), dp_ts {:.4} Pa against {MEAS_DP} ({:+.3} %)",
        100.0 * (power / MEAS_POWER - 1.0),
        b.dp.mean,
        100.0 * (b.dp.mean / MEAS_DP - 1.0)
    );
    let (ka, kb) = (key(&a), key(&b));
    println!("F879-HC keys: [{ka:?}, {kb:?}]");

    // A1
    assert!(a.complete && b.complete, "A1");
    assert_eq!((a.rows, b.rows), (H_STEPS, 2 * H_STEPS), "A1");
    assert_eq!((a.window, b.window), (2749, 5498), "A1: the window");
    // G
    assert_eq!(c, Class::Steady, "G");
    // T1, T2 (TF applies only where both runs miss)
    for (what, x, y) in [("T1 torque", &a.torque, &b.torque), ("T2 dp_ts", &a.dp, &b.dp)] {
        match (steady_to_state(x), steady_to_state(y)) {
            (true, true) => {}
            (false, false) => {
                let q = y.fp / x.fp;
                assert!(
                    q >= F_BAND.0 && q <= F_BAND.1,
                    "{what} missed in both runs; TF: f_p ratio {q}"
                );
            }
            _ => panic!("{what}: missed in one run only, not deciding: {} and {}", x.rel, y.rel),
        }
    }
    // T3, T4
    assert!(t3.iter().all(|d| d.abs() <= STATE_TOL), "T3: {t3:?}");
    assert!(t4[0].abs() <= U_TORQUE && t4[1].abs() <= U_DP, "T4: {t4:?}");
    // G2
    match RECORDED {
        None => println!("F879-HC G2: no record yet (the registered run)"),
        Some(rec) => {
            let same = |x: f64, y: f64| (x.is_nan() && y.is_nan()) || x == y || (x / y - 1.0).abs() <= REC_TOL;
            for (got, want) in [(ka, rec[0]), (kb, rec[1])] {
                for (i, (x, y)) in got.iter().zip(want.iter()).enumerate() {
                    assert!(same(*x, *y), "G2 key {i}: {x} against {y}");
                }
            }
        }
    }
}
