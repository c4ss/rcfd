//! ADR 0025 checks M1-M6 (F-144): log mapping, decimation, stream ingest, polling without blocking, large frames. M7 (ADR 0095):
//! the plot series grows and frames draw while a cavity run is in progress.

use rcfd_gui::assistant_setup::AssistantSetup;
use rcfd_gui::controller::Controller;
use rcfd_gui::layout::{self, Panel};
use rcfd_gui::monitors::{decimate_minmax, MonitorStore, Series};
use rcfd_gui::state::AppState;
use rcfd_gui::theme::Theme;
use rcfd_gui::RcfdApp;
use serde_json::json;
use std::time::{Duration, Instant};

fn lcg(s: &mut u64) -> f64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*s >> 11) as f64 / (1u64 << 53) as f64
}

fn store_with(name: &str, x: Vec<f64>, y: Vec<f64>) -> MonitorStore {
    let mut m = MonitorStore::default();
    m.series.insert(name.into(), Series { x, y });
    m
}

#[test]
fn m1_log_mapping_is_exact() {
    let mut s = 7u64;
    let (mut xs, mut ys) = (vec![], vec![]);
    let mut i = 0.0;
    for k in -30..=30 {
        for _ in 0..20 {
            xs.push(i);
            i += 1.0;
            ys.push((1.0 + 8.999 * lcg(&mut s)) * 10f64.powi(k));
        }
        xs.push(i);
        i += 1.0;
        ys.push(10f64.powi(k));
    }
    let m = store_with("res_u", xs.clone(), ys.clone());
    let pts = m.plot_points("res_u", true);
    assert_eq!(pts.len(), ys.len());
    let mut worst: f64 = 0.0;
    for (p, y) in pts.iter().zip(&ys) {
        assert_eq!(p[1].to_bits(), y.log10().to_bits());
        worst = worst.max(((10f64.powf(p[1])) / y - 1.0).abs());
    }
    eprintln!("m1: worst inverse relative error {worst:.3e}");
    assert!(worst <= 1e-12);
    for k in -30..=30 {
        let m = store_with("a", vec![0.0], vec![10f64.powi(k)]);
        let v = m.plot_points("a", true)[0][1];
        assert!((v - f64::from(k)).abs() < 1e-12, "decade {k}: {v}");
    }
}

#[test]
fn m2_log_filtering_counts_are_exact() {
    let mut y: Vec<f64> = (1..=100).map(|i| f64::from(i) * 1e-3).collect();
    y.extend([0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1e-300]);
    let x: Vec<f64> = (0..y.len()).map(|i| i as f64).collect();
    let m = store_with("c", x, y);
    let log = m.plot_points("c", true);
    assert_eq!(log.len(), 100);
    // linear: finite values only: 100 + 0.0 + -1.0 + -1e-300
    assert_eq!(m.plot_points("c", false).len(), 103);
    for p in log.iter().chain(&m.plot_points("c", false)) {
        assert!(p[0].is_finite() && p[1].is_finite());
    }
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

#[test]
fn m4_stream_ingest_drops_nothing() {
    let n = 100_000usize;
    let val = |name: usize, i: usize| -> Option<f64> {
        if i % 1000 == 7 {
            None
        } else {
            Some((i as f64 + name as f64).sin() * 1e-3 + name as f64)
        }
    };
    let mut m = MonitorStore::default();
    let mut s = 31u64;
    let mut sent = 0usize;
    let mut added = 0usize;
    while sent < n {
        let chunk = 1 + (lcg(&mut s) * 499.0) as usize;
        let from = sent.saturating_sub(10);
        let to = (sent + chunk).min(n);
        let mut series = serde_json::Map::new();
        for (k, name) in ["res_u", "res_p", "continuity"].iter().enumerate() {
            let x: Vec<f64> = (from..to).map(|i| (i + 1) as f64).collect();
            let y: Vec<serde_json::Value> = (from..to).map(|i| json!(val(k, i))).collect();
            series.insert((*name).into(), json!({"x": x, "y": y}));
        }
        added += m.merge(&json!({"series": series}));
        sent = to;
    }
    assert_eq!(added, 3 * n);
    for (k, name) in ["res_u", "res_p", "continuity"].iter().enumerate() {
        let sr = &m.series[*name];
        assert_eq!(sr.x.len(), n);
        for i in 0..n {
            assert_eq!(sr.x[i], (i + 1) as f64);
            match val(k, i) {
                Some(v) => assert_eq!(sr.y[i].to_bits(), v.to_bits()),
                None => assert!(sr.y[i].is_nan()),
            }
        }
    }
}

#[test]
fn m5_polling_a_running_case_does_not_block_and_loses_no_iteration() {
    let dir = std::env::temp_dir().join("rcfd-gui-w17c-m5");
    let mut c = Controller::new(None, dir);
    c.new_from_template().unwrap();
    assert!(c.edit_text("solver.max_iterations", "60"));
    assert!(c.run_start());
    let mut worst = Duration::ZERO;
    let mut saw_running_partial = false;
    let t0 = Instant::now();
    loop {
        let t = Instant::now();
        c.poll(false);
        worst = worst.max(t.elapsed());
        if c.status.state == "running" && c.status.iteration < 60 {
            saw_running_partial = true;
        }
        if c.status.state != "running" && c.status.state != "paused" && c.status.iteration > 0 {
            break;
        }
        if t0.elapsed() > Duration::from_secs(120) {
            panic!("run did not finish");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    c.wait_idle(60_000);
    c.poll(true);
    eprintln!(
        "m5: worst poll {:?}, saw running partial: {saw_running_partial}, iteration {}, state {}",
        worst, c.status.iteration, c.status.state
    );
    assert!(worst < Duration::from_secs(1), "poll took {worst:?}");
    assert!(
        saw_running_partial,
        "run finished before any poll saw it running (non-vacuous check failed)"
    );
    let s = &c.monitors.series["res_u"];
    assert_eq!(s.x.len() as u64, c.status.iteration);
    assert!(s.x.windows(2).all(|w| w[1] > w[0]));
}

/// M7 (ADR 0095, F-144): a long cavity run driven only by headless egui frames of the app with the Monitors panel open (the
/// frame's own 250 ms poll); while the state is `running` the res_u series must grow strictly over at least three polls and every
/// frame must finish within 1 s. The run is stopped once the evidence is collected.
#[test]
fn m7_plot_series_grows_and_frames_draw_while_running() {
    let dir = std::env::temp_dir().join("rcfd-gui-w39d-m7");
    let mut c = Controller::new(None, dir);
    c.new_from_template().unwrap();
    assert!(c.edit_text("solver.max_iterations", "200000"));
    assert!(c.edit_text("solver.tolerance_u", "1e-14"));
    assert!(c.edit_text("solver.tolerance_p", "1e-14"));
    assert!(c.run_start());
    let mut app = RcfdApp::new(AppState::new(c, AssistantSetup::in_memory(), Theme::Light));
    layout::float(&mut app.dock, Panel::Monitors);
    let ctx = egui::Context::default();
    let mut lens: Vec<usize> = Vec::new();
    let (mut frames_running, mut worst, mut shapes_min) = (0usize, Duration::ZERO, usize::MAX);
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(20));
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
            ..Default::default()
        };
        let t = Instant::now();
        let out = ctx.run(input, |ctx| app.ui(ctx));
        let dt = t.elapsed();
        let ctrl = &app.st.ctrl;
        if !ctrl.status.is_running() {
            continue;
        }
        frames_running += 1;
        worst = worst.max(dt);
        shapes_min = shapes_min.min(out.shapes.len());
        let n = ctrl.monitors.series.get("res_u").map_or(0, |s| s.x.len());
        if lens.last() != Some(&n) {
            lens.push(n);
        }
        if lens.len() >= 5 {
            break;
        }
    }
    let final_state = app.st.ctrl.status.state.clone();
    app.st.ctrl.run_stop();
    app.st.ctrl.wait_idle(60_000);
    eprintln!(
        "m7: series lengths while running {lens:?}, frames in running state {frames_running}, worst frame {worst:?}, min shapes {shapes_min}, state at end of loop {final_state}"
    );
    // the length before the first poll is 0 and does not count
    let grown: Vec<usize> = lens.into_iter().filter(|&n| n > 0).collect();
    assert!(
        grown.len() >= 4,
        "fewer than four distinct series lengths while running: {grown:?}"
    );
    assert!(
        grown.windows(2).all(|w| w[1] > w[0]),
        "series did not grow strictly: {grown:?}"
    );
    assert!(*grown.last().unwrap() < 200_000, "the run was not mid-way");
    assert!(frames_running >= 3, "{frames_running} frames in state running");
    assert!(worst < Duration::from_secs(1), "frame took {worst:?}");
    assert!(shapes_min > 0, "a running frame drew nothing");
}

#[test]
fn m6_large_series_are_decimated_and_the_frame_draws() {
    let n = 200_000usize;
    let dir = std::env::temp_dir().join("rcfd-gui-w17c-m6");
    let mut ctrl = Controller::new(None, dir);
    ctrl.new_from_template().unwrap();
    let x: Vec<f64> = (1..=n).map(|i| i as f64).collect();
    for (k, name) in ["res_u", "res_p", "continuity"].iter().enumerate() {
        let y: Vec<f64> = (1..=n).map(|i| 10f64.powf(-(i as f64) / 40_000.0 - k as f64)).collect();
        ctrl.monitors.series.insert((*name).into(), Series { x: x.clone(), y });
        let d = ctrl.monitors.plot_points_decimated(name, true, 4000);
        assert!(d.len() <= 4000 && d.len() > 100, "{name}: {}", d.len());
    }
    let mut app = RcfdApp::new(AppState::new(ctrl, AssistantSetup::in_memory(), Theme::Light));
    layout::float(&mut app.dock, Panel::Monitors);
    let ctx = egui::Context::default();
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.ui(ctx));
    }
}
