//! ADR 0686 (F-766): a fast check of the plain-pick condition of the repetition trigger of ADR 0680. One hand-built
//! observation sequence (d = 2) whose last d + 1 points lie in the incumbent's basin: with a large exclusion radius the plain
//! pick of the step lies outside the basin and the repetition restart must make no restart pick (N1); with the default
//! exclusion the plain pick lies inside and it must make one (P1, the twin). Both assert their preconditions first.

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

/// 14 far points, the incumbent (0.5, 0.5), then the window of three points next to it.
fn observations() -> Vec<Observation> {
    let mut pts: Vec<Vec<f64>> = (1..=14).map(|i| vec![halton(i, 2), halton(i, 3)]).collect();
    pts.push(vec![0.5, 0.5]);
    pts.extend([vec![0.52, 0.5], vec![0.5, 0.52], vec![0.48, 0.48]]);
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

/// The step's preconditions, from the library's own plain pick: (plain pick, incumbent, delta of the plain pick, window deltas,
/// number of points outside the basin).
fn preconditions(exclusion: f64) -> (Pick, f64, Vec<f64>, usize) {
    let obs = observations();
    assert_eq!(obs.len(), 18);
    let plain = pick(&obs, &options(exclusion, None));
    assert_eq!(
        plain.kind,
        PickKind::Ei,
        "the plain pick is an expected-improvement pick"
    );
    let star = [0.5, 0.5];
    // the incumbent of the sequence is (0.5, 0.5): the only observation of value 0
    let best = obs
        .iter()
        .filter_map(|o| match &o.status {
            Status::Ok { f, .. } => Some(*f),
            _ => None,
        })
        .fold(f64::INFINITY, f64::min);
    assert_eq!(best, 0.0);
    assert_eq!(plain.incumbent, Some(0.0));
    let l = &plain.length_scales;
    let dp = delta(&plain.x, &star, l);
    let win: Vec<f64> = obs[15..].iter().map(|o| delta(&o.x, &star, l)).collect();
    let outside = obs.iter().filter(|o| delta(&o.x, &star, l) >= RHO).count();
    (plain, dp, win, outside)
}

#[test]
fn n1_plain_pick_outside_the_basin_no_restart_pick() {
    let (plain, dp, win, outside) = preconditions(0.25);
    println!(
        "N1: plain delta {dp:.3}, window deltas {win:.3?}, outside {outside}, plain x {:?}",
        plain.x
    );
    // N1a to N1c: the precondition, so that the assertion below cannot pass vacuously
    assert!(win.iter().all(|w| *w < RHO / 2.0), "N1a window inside the basin");
    assert!(dp >= RHO + 1.0, "N1b plain pick outside the basin by a margin: {dp}");
    assert!(outside >= 4, "N1c points outside the basin: {outside}");
    // N1d
    let r = pick(&observations(), &options(0.25, Some(rep())));
    println!("N1: repetition pick kind {:?}, x {:?}", r.kind, r.x);
    assert_eq!(r.kind, PickKind::Ei, "N1d no restart pick");
    assert_eq!(bits(&r.x), bits(&plain.x), "N1d the plain pick stands");
}

#[test]
fn p1_plain_pick_inside_the_basin_restart_pick() {
    let (plain, dp, win, outside) = preconditions(BayesOptions::default().exclusion);
    println!(
        "P1: plain delta {dp:.3}, window deltas {win:.3?}, outside {outside}, plain x {:?}",
        plain.x
    );
    assert!(win.iter().all(|w| *w < RHO / 2.0), "P1a window inside the basin");
    assert!(dp < RHO / 2.0, "P1a plain pick inside the basin: {dp}");
    assert!(outside >= 4, "P1 points outside the basin: {outside}");
    let r = pick(
        &observations(),
        &options(BayesOptions::default().exclusion, Some(rep())),
    );
    let dr = delta(&r.x, &[0.5, 0.5], &r.length_scales);
    println!("P1: repetition pick kind {:?}, x {:?}, delta {dr:.3}", r.kind, r.x);
    assert_eq!(r.kind, PickKind::Restart, "P1b restart pick");
    assert!(dr >= RHO, "P1b the restart pick lies outside the basin: {dr}");
}
