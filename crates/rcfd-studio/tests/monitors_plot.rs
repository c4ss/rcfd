//! Ported from `crates/rcfd-gui/tests/monitors_plot.rs` at main 15b00ef (ADR 0131 W48-9 (a), limits unchanged): ADR 0025
//! checks M1-M4 (log mapping, log filtering, decimation, stream ingest) on the copied monitor store, M6 (a large series is
//! decimated and the frame draws, here the Studio's Plots panel), and the frozen logic test
//! `monitors_merge_is_idempotent_and_incremental`. M5 and M7 start a run of the in-process solver, which the Studio never
//! does (ADR 0133 D1); their Studio form is ADR 0133 R1, R3 and R4 (48D).

use rcfd_studio::layout::{self, Panel};
use rcfd_studio::monitors::{decimate_minmax, MonitorStore, Series};
use rcfd_studio::{Studio, StudioApp};
use serde_json::json;

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
fn m6_large_series_are_decimated_and_the_frame_draws() {
    let n = 200_000usize;
    let mut st = Studio::in_memory();
    let x: Vec<f64> = (1..=n).map(|i| i as f64).collect();
    for (k, name) in ["res_u", "res_p", "continuity"].iter().enumerate() {
        let y: Vec<f64> = (1..=n).map(|i| 10f64.powf(-(i as f64) / 40_000.0 - k as f64)).collect();
        st.plots.series.insert((*name).into(), Series { x: x.clone(), y });
        let d = st.plots.plot_points_decimated(name, true, 4000);
        assert!(d.len() <= 4000 && d.len() > 100, "{name}: {}", d.len());
    }
    let mut app = StudioApp::new(st, None);
    layout::float(&mut app.st.dock, Panel::Plots);
    let ctx = egui::Context::default();
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
            ..Default::default()
        };
        let _ = ctx.run_ui(input, |ui| app.ui(ui));
    }
}

#[test]
fn monitors_merge_is_idempotent_and_incremental() {
    let doc = json!({"series": {"res_u": {"x": [1.0, 2.0], "y": [0.1, 0.01]}}});
    let mut m = MonitorStore::default();
    assert_eq!(m.merge(&doc), 2);
    assert_eq!(m.merge(&doc), 0);
    assert_eq!(m.since(), Some(2.0));
    let pts = m.plot_points("res_u", true);
    assert_eq!(pts.len(), 2);
    assert!((pts[1][1] + 2.0).abs() < 1e-12);
}
