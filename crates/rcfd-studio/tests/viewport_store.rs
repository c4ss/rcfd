//! ADR 0132 integration checks on a run's field store: V4 (field from the store, picks), V5 (time lookup), V8 (legend and
//! range), V10 (the slider uses the scalar-only update), V11 (the plot marker). GPU parts run on every adapter with the
//! SKIP rule of ADR 0125.

#[macro_use]
mod vp;

use rcfd_render::{Field, SurfaceMesh};
use rcfd_studio::layout::{self, Panel};
use rcfd_studio::viewport::{RangeMode, StoreView, ViewGpu};
use rcfd_studio::StudioApp;
use rcfd_tsstore::{Mode, Pick, Reader};
use std::sync::Arc;

const V4_SNAPS: [(u64, f64); 3] = [(10, 0.1), (20, 0.2), (30, 0.3)];

fn mapped(s: &SurfaceMesh, cells: &[f64]) -> Vec<f64> {
    s.face_values_from_cells(cells).unwrap()
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn centroid(s: &SurfaceMesh, f: usize) -> [f64; 3] {
    let l = s.face(f);
    let mut c = [0.0; 3];
    for &p in l {
        for d in 0..3 {
            c[d] += s.points[p][d] / l.len() as f64;
        }
    }
    c
}

/// V4: the shown values are the store's, mapped through the owner cells, bit for bit; per-face picks return the face and
/// the stored value (f32 in the pick pass, f64 in the readout). Negative control: the next snapshot's values differ.
#[test]
fn v4_field_from_the_store_and_picks() {
    let (root, id, dir) = vp::store_project("v4", &V4_SNAPS, vp::p_v4);
    let mut st = vp::studio_showing(&root, &id);
    let surface: Arc<SurfaceMesh> = st.viewport.surface.clone().unwrap();
    assert!(st.viewport.two_d, "the cavity is a 2D case");
    assert_eq!(surface.n_faces(), 64);
    let mut owners: Vec<usize> = surface.cell.iter().map(|c| c.unwrap()).collect();
    owners.sort();
    assert_eq!(owners, (0..64).collect::<Vec<_>>(), "distinct owner cells 0..63");
    let mut r = Reader::open(&dir).unwrap();
    let expected: Vec<Vec<f64>> = (0..3)
        .map(|k| mapped(&surface, &r.read_field(k, "p").unwrap()))
        .collect();
    let mut controls = 0;
    for k in 0..3 {
        vp::slide(&mut st, V4_SNAPS[k].1);
        let f = st.viewport.frame.clone().unwrap();
        assert_eq!((f.index, f.field.as_str()), (k, "p"));
        assert_eq!(bits(&f.faces), bits(&expected[k]), "snapshot {k}: shown values");
        if k + 1 < 3 {
            assert_ne!(
                bits(&f.faces),
                bits(&expected[k + 1]),
                "control: snapshot {k} vs {}",
                k + 1
            );
            controls += 1;
        }
    }
    eprintln!("V4 model: 3 snapshots x 64 faces bit-equal; control: {controls} of 2 next-snapshot comparisons differ");
    let adapters = adapters_or_skip!();
    for a in adapters {
        st.viewport.set_gpu(ViewGpu::from_adapter(a));
        st.viewport.set_size(400, 400, false);
        let mut picks = 0;
        for k in 0..3 {
            vp::show(&mut st, k);
            st.viewport.prepare();
            assert!(st.viewport.error.is_none(), "{:?}", st.viewport.error);
            for f in 0..64 {
                let (x, y) = st.viewport.pixel_of(centroid(&surface, f)).unwrap();
                let h = st
                    .viewport
                    .hover_at(x.floor() as u32, y.floor() as u32)
                    .unwrap_or_else(|| panic!("{}: no face at the centroid of face {f}", vp::adapter_name(a)));
                let stored = expected[k][f];
                assert_eq!(h.face, f, "{}: snapshot {k}", vp::adapter_name(a));
                assert_eq!(
                    h.picked.to_bits(),
                    (stored as f32).to_bits(),
                    "{}: face {f}",
                    vp::adapter_name(a)
                );
                assert_eq!(h.value.to_bits(), stored.to_bits());
                assert_eq!(h.cell, surface.cell[f]);
                picks += 1;
            }
        }
        let g = st.viewport.gpu().unwrap();
        eprintln!(
            "V4 picks on {}: {picks} of 192 exact (uploads {}, scalar updates {}, renders {})",
            vp::adapter_name(a),
            g.uploads,
            g.scalar_updates,
            g.renders
        );
    }
}

fn lcg(s: &mut u64) -> f64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*s >> 11) as f64 / (1u64 << 53) as f64
}

/// The control lookup: nearest, but the later snapshot at a tie.
fn round_half_up(times: &[f64], t: f64) -> usize {
    let k = times.partition_point(|&x| x <= t);
    match (k.checked_sub(1), times.get(k)) {
        (Some(f), Some(n)) => {
            if n - t <= t - times[f] {
                k
            } else {
                f
            }
        }
        (Some(f), None) => f,
        (None, _) => 0,
    }
}

/// V5: slider positions in both modes against `Reader::by_time(Nearest)` and `field_at_time(Linear)`, through the
/// worker's sampling code for every position and through the drawn app once per position class. Negative control: a
/// round-half-up lookup differs at a tie.
#[test]
fn v5_time_lookup_in_both_modes() {
    let times = [0.0, 0.1, 0.35, 0.4, 0.9, 1.6];
    let snaps: Vec<(u64, f64)> = times.iter().enumerate().map(|(i, &t)| (i as u64 + 1, t)).collect();
    let (root, id, dir) = vp::store_project("v5", &snaps, vp::p_v4);
    let mut sv = StoreView::open(&dir).unwrap();
    let surface = sv.surface.clone();
    let mut r = Reader::open(&dir).unwrap();
    let mut pos: Vec<(&str, f64)> = times.iter().map(|&t| ("snapshot", t)).collect();
    pos.extend(times.windows(2).map(|w| ("midpoint", 0.5 * (w[0] + w[1]))));
    pos.extend([("end", 0.0), ("end", 1.6)]);
    let mut s = 48005u64;
    pos.extend((0..200).map(|_| ("random", 1.6 * lcg(&mut s))));
    let (mut n_near, mut n_lin, mut n_interp) = (0, 0, 0);
    for &(class, t) in &pos {
        let idx = r.by_time(t, Pick::Nearest).unwrap();
        let f = sv.sample("p", t, false).unwrap();
        assert_eq!(f.index, idx, "{class} t = {t}");
        assert!(!f.interpolated);
        assert_eq!(
            bits(&f.faces),
            bits(&mapped(&surface, &r.read_field(idx, "p").unwrap())),
            "{class} t = {t}"
        );
        n_near += 1;
        let want = r.field_at_time("p", t, Mode::Linear).unwrap();
        let f = sv.sample("p", t, true).unwrap();
        assert_eq!(
            bits(&f.faces),
            bits(&mapped(&surface, &want.values)),
            "linear {class} t = {t}"
        );
        assert_eq!(f.interpolated, want.interpolated, "linear {class} t = {t}");
        if class == "snapshot" {
            assert!(!f.interpolated, "exactly at a snapshot time: stored values");
        }
        n_interp += usize::from(f.interpolated);
        n_lin += 1;
    }
    let differ = times
        .windows(2)
        .map(|w| 0.5 * (w[0] + w[1]))
        .filter(|&t| round_half_up(&times, t) != r.by_time(t, Pick::Nearest).unwrap())
        .count();
    assert!(
        differ >= 1,
        "control: the round-half-up lookup agrees at every midpoint"
    );
    // through the drawn app, once per position class and mode
    let mut app = StudioApp::new(vp::studio_showing(&root, &id), None);
    let ctx = egui::Context::default();
    let picks = [
        ("snapshot", 0.35),
        ("midpoint", 0.225),
        ("random", pos[pos.len() - 1].1),
    ];
    for (class, t) in picks {
        for linear in [false, true] {
            app.st.viewport.set_linear(linear);
            vp::slide(&mut app.st, t);
            let f = app.st.viewport.frame.clone().unwrap();
            let want = if linear {
                let w = r.field_at_time("p", t, Mode::Linear).unwrap();
                assert_eq!(f.interpolated, w.interpolated);
                mapped(&surface, &w.values)
            } else {
                let i = r.by_time(t, Pick::Nearest).unwrap();
                assert_eq!(f.index, i);
                mapped(&surface, &r.read_field(i, "p").unwrap())
            };
            assert_eq!(bits(&f.faces), bits(&want), "app {class} linear {linear}");
            let drawn = vp::texts(&vp::frame(&mut app, &ctx).shapes);
            let shown = drawn.iter().any(|s| s.contains("interpolated"));
            assert_eq!(
                shown, f.interpolated,
                "app {class} linear {linear}: the flag is drawn iff set"
            );
        }
    }
    eprintln!(
        "V5: {} positions, {n_near} nearest and {n_lin} linear samples bit-equal ({n_interp} interpolated); control: \
         round-half-up differs at {differ} of 5 midpoints; app: 3 classes x 2 modes",
        pos.len()
    );
}

/// V8: legend end labels and the range modes; the over-run range is exact. Negative control: per-snapshot ranges differ
/// from the over-run range at snapshots 0 and 2.
#[test]
fn v8_legend_and_range() {
    let (root, id, dir) = vp::store_project("v8", &V4_SNAPS, |c, k| 0.5 + 0.01 * c as f64 + k as f64);
    let mut app = StudioApp::new(vp::studio_showing(&root, &id), None);
    let ctx = egui::Context::default();
    let surface = app.st.viewport.surface.clone().unwrap();
    let mut r = Reader::open(&dir).unwrap();
    let per: Vec<(f64, f64)> = (0..3)
        .map(|k| {
            let v = mapped(&surface, &r.read_field(k, "p").unwrap());
            v.iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |a, &x| (a.0.min(x), a.1.max(x)))
        })
        .collect();
    let run = per.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |a, &(x, y)| {
        (a.0.min(x), a.1.max(y))
    });
    let check = |app: &mut StudioApp, ctx: &egui::Context, lo: f64, hi: f64, what: &str| {
        let l = app.st.viewport.legend().unwrap();
        assert_eq!(
            (l.lo.to_bits(), l.hi.to_bits()),
            (lo.to_bits(), hi.to_bits()),
            "{what}: range"
        );
        assert_eq!(l.labels[0], format!("{lo:.4e}"), "{what}");
        assert_eq!(l.labels[l.labels.len() - 1], format!("{hi:.4e}"), "{what}");
        let drawn = vp::texts(&vp::frame(app, ctx).shapes);
        for s in [&l.labels[0], &l.labels[l.labels.len() - 1]] {
            assert!(drawn.contains(s), "{what}: label {s} not drawn");
        }
        l.labels
    };
    for k in 0..3 {
        vp::show(&mut app.st, k);
        check(
            &mut app,
            &ctx,
            per[k].0,
            per[k].1,
            &format!("snapshot {k}, per snapshot"),
        );
    }
    app.st.viewport.set_range_mode(RangeMode::Run).unwrap();
    assert!(app.st.drain_until(vp::WAIT, |s| s.viewport.run_range.is_some()));
    let mut labels = Vec::new();
    for k in 0..3 {
        vp::show(&mut app.st, k);
        labels = check(&mut app, &ctx, run.0, run.1, &format!("snapshot {k}, over the run"));
    }
    assert_ne!(per[0], run, "control: snapshot 0");
    assert_ne!(per[2], run, "control: snapshot 2");
    eprintln!("V8: per-snapshot ranges {per:?}; over the run {run:?}, labels {labels:?}");
}

/// V10: stepping the slider rewrites only the scalar (no new upload), and the texture equals a fresh upload's image.
#[test]
fn v10_slider_uses_the_scalar_only_update() {
    let (root, id, dir) = vp::store_project("v10", &V4_SNAPS, vp::p_v4);
    let adapters = adapters_or_skip!();
    let mut r = Reader::open(&dir).unwrap();
    for a in adapters {
        let mut app = StudioApp::new(vp::studio_showing(&root, &id), None);
        let ctx = egui::Context::default();
        app.st.viewport.set_gpu(ViewGpu::from_adapter(a));
        let surface = app.st.viewport.surface.clone().unwrap();
        let expected: Vec<Vec<f64>> = (0..3)
            .map(|k| mapped(&surface, &r.read_field(k, "p").unwrap()))
            .collect();
        let _ = vp::frame(&mut app, &ctx);
        let g = app.st.viewport.gpu().unwrap();
        assert_eq!((g.uploads, g.scalar_updates), (1, 0), "first image: one upload");
        assert!(g.renders >= 1);
        let mut controls = 0;
        for (n, k) in [1usize, 2, 0].into_iter().enumerate() {
            let size = app.st.viewport.size;
            let g = app.st.viewport.gpu().unwrap();
            let rd = g.renderer();
            let before = vp::read_back(rd.device(), rd.queue(), g.texture().unwrap());
            app.st.viewport.show_snapshot(k);
            assert!(app.st.drain_until(vp::WAIT, |s| !s.viewport.pending()
                && s.viewport.frame.as_ref().is_some_and(|f| f.index == k)));
            let _ = vp::frame(&mut app, &ctx);
            assert_eq!(app.st.viewport.size, size, "the size did not change");
            // amendment 2: the expected image under the view the model draws the new snapshot with
            let view = app.st.viewport.view().unwrap();
            let g = app.st.viewport.gpu().unwrap();
            assert_eq!(g.uploads, 1, "{}: step to {k} uploaded", vp::adapter_name(a));
            assert_eq!(g.scalar_updates, n as u64 + 1);
            let rd = g.renderer();
            let fresh = rd.upload(&surface, Field::Face(&expected[k])).unwrap();
            let want = rd.render(&fresh, &view).unwrap();
            if before != want.rgba {
                controls += 1;
            }
            let after = vp::read_back(rd.device(), rd.queue(), g.texture().unwrap());
            let diff = after.chunks(4).zip(want.rgba.chunks(4)).filter(|(x, y)| x != y).count();
            assert_eq!(
                diff,
                0,
                "{}: snapshot {k}: {diff} pixels differ from a fresh upload",
                vp::adapter_name(a)
            );
        }
        assert_eq!(
            controls, 3,
            "control: the texture before each step differs from the next image"
        );
        let g = app.st.viewport.gpu().unwrap();
        eprintln!(
            "V10 on {}: 3 steps byte-identical to a fresh upload ({}x{}); uploads {}, scalar updates {}, renders {}; \
             control: {controls} of 3 differ before the step",
            vp::adapter_name(a),
            g.texture().unwrap().width(),
            g.texture().unwrap().height(),
            g.uploads,
            g.scalar_updates,
            g.renders
        );
    }
}

/// V11: the plot marker is the shown snapshot's step (interpolated in linear mode) and is cleared on close.
#[test]
fn v11_plot_marker_follows_the_time_slider() {
    let (root, id, dir) = vp::store_project("v11", &V4_SNAPS, vp::p_v4);
    let mut app = StudioApp::new(vp::studio_showing(&root, &id), None);
    let ctx = egui::Context::default();
    for k in 0..3 {
        vp::show(&mut app.st, k);
        assert_eq!(app.st.plot_marker, Some(V4_SNAPS[k].0 as f64), "snapshot {k}");
        assert_ne!(
            app.st.plot_marker,
            Some(k as f64),
            "control: the marker is not the snapshot index"
        );
    }
    app.st.viewport.set_linear(true);
    vp::slide(&mut app.st, 0.125);
    let w = Reader::open(&dir)
        .unwrap()
        .field_at_time("p", 0.125, Mode::Linear)
        .unwrap()
        .weight;
    let want = 10.0 + w * (20.0 - 10.0);
    assert_eq!(
        app.st.plot_marker.map(f64::to_bits),
        Some(want.to_bits()),
        "linear: {:?}",
        app.st.plot_marker
    );
    app.st.plots.push("res_u", 10.0, 1e-3);
    app.st.plots.push("res_u", 20.0, 1e-4);
    layout::float(&mut app.st.dock, Panel::Plots);
    let _ = vp::frame(&mut app, &ctx);
    app.st.close_project();
    assert_eq!(app.st.plot_marker, None);
    let _ = vp::frame(&mut app, &ctx);
    eprintln!("V11: markers 10, 20, 30; linear at t = 0.125: {want} (w = {w}); None after close");
}
