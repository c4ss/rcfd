//! ADR 0138 (wave 48G1) UI-thread costs of the mesh preview of a case (release builds, no GPU): G3 (frames while a large
//! mesh builds) and G8 (frames with the 2D overlay at its edge limit). Frame time is wall time around `run_ui` plus
//! tessellation, as the window spends it.

mod pv;

use rcfd_studio::preview::Status;
use rcfd_studio::viewport::Input;
use rcfd_studio::StudioApp;
use std::time::{Duration, Instant};

const MEDIAN_MS: f64 = 8.0;
const MAX_MS: f64 = 250.0;

/// G3: while the preview of a 500 x 500 box (250,000 cells; ADR 0138 amendment 3, the maintainer's question 8: was 1000
/// x 1000) builds, the median frame is at most 8 ms and no frame takes more than 250 ms (at least 20 frames, the build
/// at least 0.5 s). Negative control: building on the UI thread gives a frame of more than 250 ms.
#[test]
fn g3_frames_while_a_large_mesh_builds() {
    if cfg!(debug_assertions) {
        eprintln!("SKIP g3 (crates/rcfd-studio/tests/preview_cost_g1.rs): registered for release builds");
        return;
    }
    for control in [false, true] {
        let root = pv::project("g3", &[("cases/big.toml", pv::box_case(500, 500).as_bytes())]);
        let mut app = StudioApp::new(pv::studio(&root), None);
        app.st.preview.switches.build_on_ui_thread = control;
        let ctx = egui::Context::default();
        for _ in 0..5 {
            let _ = pv::timed_frame(&mut app, &ctx, Vec::new());
        }
        app.st.open_case("cases/big.toml");
        let t0 = Instant::now();
        let mut times = Vec::new();
        loop {
            let (_, ms) = pv::timed_frame(&mut app, &ctx, Vec::new());
            times.push(ms);
            if app.st.preview.case.is_some() && pv::settled(&app.st) {
                break;
            }
            assert!(t0.elapsed() < pv::WAIT, "G3: the build did not end");
            std::thread::sleep(Duration::from_millis(5));
        }
        let b = pv::built(&app.st);
        let max = times.iter().copied().fold(0.0, f64::max);
        let med = pv::median(&times);
        eprintln!(
            "G3{}: {} frames over {:.2} s while building {} cells ({:.2} s in build); median {med:.3} ms, max {max:.1} ms; \
             status {}",
            if control { " control" } else { "" },
            times.len(),
            t0.elapsed().as_secs_f64(),
            b.cells,
            b.build_s,
            match &app.st.preview.status {
                Status::TooLarge { rebuild: false, .. } => "surface over the triangle limit (not shown)",
                Status::Ready => "ready",
                _ => "other",
            }
        );
        assert!(b.build_s >= 0.5, "set-up: the build took {:.3} s", b.build_s);
        if control {
            assert!(max > MAX_MS, "control: max {max:.1} ms");
        } else {
            assert!(times.len() >= 20, "set-up: {} frames", times.len());
            assert!(med <= MEDIAN_MS, "median {med:.3} ms");
            assert!(max <= MAX_MS, "max {max:.1} ms");
        }
    }
}

fn overlay_vertices(app: &mut StudioApp, ctx: &egui::Context) -> usize {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
        ..Default::default()
    };
    let out = ctx.run_ui(input, |ui| app.ui(ui));
    fn walk(s: &egui::Shape) -> usize {
        match s {
            egui::Shape::Mesh(m) => m.vertices.len(),
            egui::Shape::Vec(v) => v.iter().map(walk).sum(),
            _ => 0,
        }
    }
    out.shapes.iter().map(|c| walk(&c.shape)).max().unwrap_or(0)
}

/// G8: with the overlay of a 2D case at its edge limit (a 100 x 99 box: 19,999 cell edges) redrawn every frame (the
/// camera pans 1 px per frame), the median of 100 frames is at most 8 ms. Negative control: 1,001,500 edges (a 1000 x
/// 500 box with the limits raised) give a median of more than 8 ms over 10 frames.
#[test]
fn g8_overlay_frame_cost_at_the_edge_limit() {
    if cfg!(debug_assertions) {
        eprintln!("SKIP g8 (crates/rcfd-studio/tests/preview_cost_g1.rs): registered for release builds");
        return;
    }
    let root = pv::project(
        "g8",
        &[
            ("cases/cap.toml", pv::box_case(100, 99).as_bytes()),
            ("cases/over.toml", pv::box_case(1000, 500).as_bytes()),
        ],
    );
    for (rel, edges, frames, control) in [
        ("cases/cap.toml", 19_999usize, 100usize, false),
        ("cases/over.toml", 1_001_500, 10, true),
    ] {
        let mut st = pv::studio(&root);
        if control {
            st.preview.limits.overlay_edges = 2_000_000;
        }
        let s = pv::show_case(&mut st, rel);
        if control {
            assert!(matches!(s, Status::TooLarge { rebuild: false, .. }), "{s:?}");
            st.preview.anyway();
            assert!(st.drain_until(pv::WAIT, |s| s.viewport.surface.is_some()));
        } else {
            assert_eq!(s, Status::Ready);
        }
        let b = pv::built(&st);
        assert_eq!((b.n_edges, b.edges.len()), (edges, edges), "set-up: edges kept");
        let mut app = StudioApp::new(st, None);
        let ctx = egui::Context::default();
        for _ in 0..5 {
            let _ = pv::timed_frame(&mut app, &ctx, Vec::new());
        }
        let verts = overlay_vertices(&mut app, &ctx);
        assert!(
            verts >= 4 * (edges + b.outlines.len()) * 9 / 10,
            "set-up: the overlay mesh has {verts} vertices"
        );
        let mut times = Vec::with_capacity(frames);
        for _ in 0..frames {
            app.st.viewport.input(Input::Pan(1.0, 0.0));
            let (_, ms) = pv::timed_frame(&mut app, &ctx, Vec::new());
            times.push(ms);
        }
        let med = pv::median(&times);
        eprintln!(
            "G8{}: {edges} cell edges and {} outline segments redrawn every frame; median {med:.3} ms over {frames} \
             frames (max {:.1} ms); overlay mesh {verts} vertices",
            if control { " control" } else { "" },
            b.outlines.len(),
            times.iter().copied().fold(0.0, f64::max)
        );
        if control {
            assert!(med > MEDIAN_MS, "control: median {med:.3} ms");
        } else {
            assert!(med <= MEDIAN_MS, "median {med:.3} ms");
        }
    }
}
