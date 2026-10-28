//! Unregistered (ADR 0132 integration addendum): a live run's store, written while the viewport shows it, extends the
//! slider through the worker's `refresh()`; a run without a store says where it looked.

#[macro_use]
mod vp;

use rcfd_project::RunState;
use rcfd_studio::viewport::Status;
use rcfd_studio::Studio;
use rcfd_tsstore::{Location, Precision, WriteOptions, Writer};

fn studio_on(root: &std::path::Path, id: &str) -> Studio {
    let mut st = Studio::in_memory();
    st.poll = None;
    st.run_poll = None;
    st.open_project(root);
    assert!(st.drain_until(vp::WAIT, |s| s.browser().is_some()));
    st.select_run(id);
    st
}

#[test]
fn live_store_extends_the_slider() {
    let (root, id, dir) = vp::run_project("live", RunState::Running, (0, 0.0));
    let mut st = studio_on(&root, &id);
    assert!(st.drain_until(vp::WAIT, |s| matches!(s.viewport.status, Status::NoStore(_))));
    let Status::NoStore(m) = &st.viewport.status else {
        unreachable!()
    };
    assert!(m.contains("fields") && m.contains("store.rts"), "{m}");
    let mesh = vp::cavity8();
    std::fs::create_dir_all(&dir).unwrap();
    let mut w = Writer::create(&dir, &mesh, WriteOptions::default()).unwrap();
    w.flush().unwrap();
    assert!(st.drain_until(vp::WAIT, |s| s.viewport.status == Status::Ready));
    let n = mesh.n_cells();
    for (k, (step, time)) in [(1u64, 0.1), (2, 0.2), (3, 0.3)].into_iter().enumerate() {
        let p: Vec<f64> = (0..n).map(|c| vp::p_v4(c, k)).collect();
        let mut s = w.snapshot(step, time).unwrap();
        s.field("p", "unknown", Location::Cell, &p, Precision::F64).unwrap();
        s.commit().unwrap();
        w.flush().unwrap();
        assert!(
            st.drain_until(vp::WAIT, |s| s.viewport.snaps.len() == k + 1
                && s.viewport.frame.as_ref().is_some_and(|f| f.index == k)
                && !s.viewport.pending()),
            "snapshot {k} not followed: {:?}",
            st.viewport.status
        );
        assert_eq!(st.plot_marker, Some(step as f64));
    }
    w.finish().unwrap();
}

/// "Save PNG" reads back the same pixels as the texture egui shows (first adapter).
#[test]
fn saved_image_equals_the_texture() {
    let a = &adapters_or_skip!()[0];
    let (root, id, _) = vp::store_project("save-png", &[(10, 0.1), (20, 0.2)], vp::p_v4);
    let mut app = rcfd_studio::StudioApp::new(vp::studio_showing(&root, &id), None);
    app.st.viewport.set_gpu(rcfd_studio::viewport::ViewGpu::from_adapter(a));
    let ctx = egui::Context::default();
    let _ = vp::frame(&mut app, &ctx);
    let img = app.st.viewport.snapshot_image().unwrap();
    let g = app.st.viewport.gpu().unwrap();
    let t = g.texture().unwrap();
    assert_eq!((img.width, img.height), (t.width() as usize, t.height() as usize));
    let tex = vp::read_back(g.renderer().device(), g.renderer().queue(), t);
    assert!(tex == img.rgba, "the saved image differs from the texture");
    assert!(img.to_png().unwrap().starts_with(b"\x89PNG"));
}
