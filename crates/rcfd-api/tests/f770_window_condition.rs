//! ADR 0689 (F-770): a fast check of the window condition of the repetition trigger of ADR 0680. The observations of ADR 0686
//! (d = 2) with the default exclusion, so that the plain pick lies inside the incumbent's basin; the last of the d + 1 window
//! points is moved far outside the basin (W1): the repetition restart must make no restart pick. With the point next to the
//! incumbent (W2, the twin) it makes one. Both assert their preconditions from the library's own plain pick first.

use rcfd_api::design::bayes::{self, BayesOptions, Observation, Pick, PickKind, Restart, Status, Trigger};

const RHO: f64 = 2.0;
const B: [(f64, f64); 2] = [(0.0, 1.0); 2];

fn halton(i: usize, base: usize) -> f64 {
    let (mut r, mut fr, mut n) = (0.0, 1.0 / base as f64, i);
    while n > 0 {
        r += fr * (n % base) as f64;
        n /= base;
        fr /= base as f64;
    }
    r
}

fn value(x: &[f64]) -> f64 {
    let r = ((x[0] - 0.5).powi(2) + (x[1] - 0.5).powi(2)).sqrt();
    if r < 1e-9 {
        0.0
    } else if r < 0.03 {
        1e-3 * (1.0 + x[0])
    } else {
        0.05 + 0.04 * (37.0 * x[0] + 5.0 * x[1]).sin() * (41.0 * x[1]).cos()
    }
}

/// 14 far points, the incumbent (0.5, 0.5), then the window of three points, the last one at `last`.
fn observations(last: [f64; 2]) -> Vec<Observation> {
    let mut pts: Vec<Vec<f64>> = (1..=14).map(|i| vec![halton(i, 2), halton(i, 3)]).collect();
    pts.push(vec![0.5, 0.5]);
    pts.extend([vec![0.52, 0.5], vec![0.5, 0.52], last.to_vec()]);
    pts.into_iter()
        .map(|x| {
            let f = value(&x);
            Observation {
                x,
                status: Status::Ok { f, c: vec![] },
            }
        })
        .collect()
}

fn options(exclusion: f64, restart: Option<Restart>) -> BayesOptions {
    BayesOptions {
        seed: 1,
        n0: Some(10),
        exclusion,
        restart,
        ..Default::default()
    }
}

fn rep() -> Restart {
    Restart {
        trigger: Trigger::Repetition { window: None },
        ..Restart::default()
    }
}

fn delta(u: &[f64], star: &[f64], l: &[f64]) -> f64 {
    u.iter()
        .zip(star)
        .zip(l)
        .map(|((a, c), s)| ((a - c) / s).powi(2))
        .sum::<f64>()
        .sqrt()
}

fn pick(obs: &[Observation], o: &BayesOptions) -> Pick {
    let mut p = bayes::propose(&B, obs, o).unwrap().picks;
    assert_eq!(p.len(), 1);
    p.remove(0)
}

fn bits(x: &[f64]) -> Vec<u64> {
    x.iter().map(|v| v.to_bits()).collect()
}

/// The preconditions from the library's own plain pick: (plain pick, its delta, the three window deltas, points outside).
fn preconditions(obs: &[Observation]) -> (Pick, f64, Vec<f64>, usize) {
    assert_eq!(obs.len(), 18);
    let plain = pick(obs, &options(BayesOptions::default().exclusion, None));
    assert_eq!(
        plain.kind,
        PickKind::Ei,
        "the plain pick is an expected-improvement pick"
    );
    assert_eq!(plain.incumbent, Some(0.0), "the incumbent is (0.5, 0.5), value 0");
    let star = [0.5, 0.5];
    let l = &plain.length_scales;
    let dp = delta(&plain.x, &star, l);
    let win: Vec<f64> = obs[15..].iter().map(|o| delta(&o.x, &star, l)).collect();
    let outside = obs.iter().filter(|o| delta(&o.x, &star, l) >= RHO).count();
    (plain, dp, win, outside)
}

#[test]
fn w1_window_point_outside_the_basin_no_restart_pick() {
    let obs = observations([0.9, 0.9]);
    let (plain, dp, win, outside) = preconditions(&obs);
    println!(
        "W1: plain delta {dp:.3}, window deltas {win:.3?}, outside {outside}, plain x {:?}",
        plain.x
    );
    assert!(dp < RHO / 2.0, "W1a plain pick inside the basin: {dp}");
    assert!(
        win[0] < RHO / 2.0 && win[1] < RHO / 2.0,
        "W1b first two window points inside"
    );
    assert!(
        win[2] >= RHO + 1.0,
        "W1c last window point outside by a margin: {}",
        win[2]
    );
    assert!(outside >= 4, "W1d points outside the basin: {outside}");
    let r = pick(&obs, &options(BayesOptions::default().exclusion, Some(rep())));
    println!("W1: repetition pick kind {:?}, x {:?}", r.kind, r.x);
    assert_eq!(r.kind, PickKind::Ei, "W1e no restart pick");
    assert_eq!(bits(&r.x), bits(&plain.x), "W1e the plain pick stands");
}

#[test]
fn w2_window_inside_the_basin_restart_pick() {
    let obs = observations([0.48, 0.48]);
    let (plain, dp, win, outside) = preconditions(&obs);
    println!(
        "W2: plain delta {dp:.3}, window deltas {win:.3?}, outside {outside}, plain x {:?}",
        plain.x
    );
    assert!(dp < RHO / 2.0, "W2a plain pick inside the basin: {dp}");
    assert!(win.iter().all(|w| *w < RHO / 2.0), "W2a window inside the basin");
    assert!(outside >= 4, "W2 points outside the basin: {outside}");
    let r = pick(&obs, &options(BayesOptions::default().exclusion, Some(rep())));
    let dr = delta(&r.x, &[0.5, 0.5], &r.length_scales);
    println!("W2: repetition pick kind {:?}, x {:?}, delta {dr:.3}", r.kind, r.x);
    assert_eq!(r.kind, PickKind::Restart, "W2b restart pick");
    assert!(dr >= RHO, "W2b the restart pick lies outside the basin: {dr}");
}
