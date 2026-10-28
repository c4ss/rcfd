//! ADR 0144 (job 48J) UI-thread cost of the preview that follows the case editor (release builds, no GPU): J10, frames
//! while keystrokes go into the editor's real text field and the builds they ask for run. Frame time is wall time around
//! `run_ui` plus tessellation (as ADR 0138 G3).

mod lj;
mod pv;

use rcfd_studio::preview::Status;
use rcfd_studio::StudioApp;
use std::time::{Duration, Instant};

const MEDIAN_MS: f64 = 8.0;
const MAX_MS: f64 = 250.0;
const BURSTS: usize = 4;

/// One frame with `events`, timed; drawn text dropped.
fn timed(app: &mut StudioApp, ctx: &egui::Context, events: Vec<egui::Event>, times: &mut Vec<f64>) {
    let (_, ms) = pv::timed_frame(app, ctx, events);
    times.push(ms);
}

/// Frames every 5 ms for `d`, timed.
fn frames_for(app: &mut StudioApp, ctx: &egui::Context, d: Duration, times: &mut Vec<f64>) {
    let end = Instant::now() + d;
    while Instant::now() < end {
        timed(app, ctx, Vec::new(), times);
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// J10: the 500 x 500 box open (its own build not measured), the `mesh.nx` field of the case editor clicked, then four
/// bursts typed into it through egui events (three Backspaces and three digits, `50k`, one key every 50 ms with a frame
/// every 5 ms), each followed by 700 ms of frames, then frames until the builds settle. Limits: median frame at most
/// 8 ms, no frame above 250 ms. Set-up: at least 20 frames; after each burst the editor's draft is `("mesh.nx", "50k")`
/// (the keystrokes reached the model through the field); one build of the editor's text per burst (4); at least one
/// build of at least 0.5 s; never more than 2 build threads. Negative control: building on the UI thread gives a frame
/// above 250 ms.
#[test]
fn j10_typing_into_the_field_while_builds_run() {
    if cfg!(debug_assertions) {
        eprintln!("SKIP j10 (crates/rcfd-studio/tests/live_preview_cost_j.rs): registered for release builds");
        return;
    }
    for control in [false, true] {
        let root = pv::project("j10", &[("cases/big.toml", pv::box_case(500, 500).as_bytes())]);
        let mut app = StudioApp::new(lj::studio(&root, false), None);
        app.st.preview.switches.build_on_ui_thread = control;
        app.ui_state.filter = "mesh.nx".into();
        let ctx = egui::Context::default();
        for _ in 0..5 {
            let _ = lj::frame_with(&mut app, &ctx, Vec::new());
        }
        app.st.open_case("cases/big.toml");
        let t_open = Instant::now();
        while !(lj::settled(&app.st) && app.st.editor.as_ref().is_some_and(|e| e.rel == "cases/big.toml")) {
            let _ = lj::frame_with(&mut app, &ctx, Vec::new());
            assert!(t_open.elapsed() < lj::WAIT, "J10: the case did not open");
            std::thread::sleep(Duration::from_millis(5));
        }
        let _ = lj::frame_with(&mut app, &ctx, Vec::new());
        let field = *app
            .ui_state
            .field_rects
            .get("mesh.nx")
            .expect("set-up: the mesh.nx field was not drawn");
        // the click puts the cursor after the text (the field is wider than "500")
        lj::click_at(&mut app, &ctx, field.right_center() - egui::vec2(6.0, 0.0));
        let e0 = app.st.preview.stats.editor_requests;
        let mut times = Vec::new();
        let mut drafts = Vec::new();
        let t0 = Instant::now();
        for k in 1..=BURSTS {
            let typed = format!("50{k}");
            let mut keys: Vec<Vec<egui::Event>> = (0..3).map(|_| lj::key(egui::Key::Backspace)).collect();
            keys.extend(typed.chars().map(|c| vec![egui::Event::Text(c.to_string())]));
            for ev in keys {
                timed(&mut app, &ctx, ev, &mut times);
                frames_for(&mut app, &ctx, Duration::from_millis(50), &mut times);
            }
            drafts.push(app.st.editor.as_ref().and_then(|e| e.draft.clone()));
            frames_for(&mut app, &ctx, Duration::from_millis(700), &mut times);
        }
        while !(app.st.preview.running() == 0 && lj::settled(&app.st)) {
            timed(&mut app, &ctx, Vec::new(), &mut times);
            assert!(
                t0.elapsed() < Duration::from_secs(180),
                "J10: the builds did not settle"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let wall = t0.elapsed().as_secs_f64();
        let builds = app.st.preview.stats.editor_requests - e0;
        let build_s = app.st.preview.built.as_ref().map_or(0.0, |b| b.build_s);
        let max = times.iter().copied().fold(0.0, f64::max);
        let med = pv::median(&times);
        eprintln!(
            "J10{}: {} frames over {wall:.2} s; median {med:.3} ms, max {max:.1} ms; editor builds {builds}, last build \
             {build_s:.2} s, most threads {}; drafts {drafts:?}; status {}",
            if control { " control" } else { "" },
            times.len(),
            app.st.preview.stats.most_running,
            match &app.st.preview.status {
                Status::TooLarge { rebuild: false, .. } => "over the triangle limit (not shown)",
                Status::Ready => "ready",
                _ => "other",
            }
        );
        let want: Vec<Option<(String, String)>> = (1..=BURSTS)
            .map(|k| Some(("mesh.nx".to_string(), format!("50{k}"))))
            .collect();
        assert_eq!(drafts, want, "set-up: the keystrokes did not reach the editor's draft");
        assert_eq!(builds, BURSTS as u64, "set-up: one build per burst");
        assert!(build_s >= 0.5, "set-up: the last build took {build_s:.3} s");
        if control {
            assert!(max > MAX_MS, "control: max {max:.1} ms");
        } else {
            assert!(times.len() >= 20, "set-up: {} frames", times.len());
            assert!(app.st.preview.stats.most_running <= 2, "more than 2 build threads");
            assert!(med <= MEDIAN_MS, "median {med:.3} ms");
            assert!(max <= MAX_MS, "max {max:.1} ms");
        }
    }
}
