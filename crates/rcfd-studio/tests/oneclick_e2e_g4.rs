//! ADR 0141 G1: one click from a case to a picture, end to end through the drawn app with the real `rcfd-runner` and
//! `rcfd`, and no runner alive before the click (the click starts it through `ensure_runner`, the direct start of ADR
//! 0143, as ADR 0133 R9). Run this binary with its output going to a file (ADR 0123 S5a): the direct start hands the
//! runner the caller's inheritable handles. The drop guard of `RunnerProject` shuts the runner down and kills only the
//! run's solver.

mod common;
mod g4;
#[macro_use]
mod vp;

use g4::{app_on, cavity, click, project, steps, store_snapshots};
use rcfd_project::RunState;
use rcfd_studio::layout::{self, Panel};
use rcfd_studio::quick_run::strip_view;
use std::time::{Duration, Instant};

/// (a) the run selected and listed within this of the click.
const LISTED_S: f64 = 5.0;
/// (b) the run shown running within this of the click: R9's 15 s runner start plus the pre-launch check and launch.
const RUNNING_S: f64 = 20.0;
/// (c) the first snapshot shown within this of its commit (4 viewport poll periods of 250 ms).
const FIRST_S: f64 = 1.0;
/// (d) the newest snapshot shown within this of the last one being written (the same 4 periods).
const LAST_S: f64 = 1.0;

#[derive(Default, Debug)]
struct Seen {
    listed: Option<f64>,
    running: Option<f64>,
    store_first: Option<f64>,
    frame_first: Option<f64>,
    lengths: Vec<usize>,
    lengths_running: Vec<usize>,
    indices: Vec<usize>,
    marker_bad: Vec<String>,
    focus_after_listed: Option<(bool, bool, bool)>,
    rendered_first: Option<(u64, Option<String>)>,
    newest_after: Option<f64>,
    /// GPU uploads and renders in the frame where the run is first listed (ADR 0141 amendment 3: (g) counts from there,
    /// not the Mesh side's upload of the case's preview before it).
    gpu_at_listed: Option<(u64, u64)>,
}

/// G1 missed (h) in its registered run (amendment 1): the reopened Viewport took the Plots' place. Kept with the
/// reopening it measured (`reopen_in_focused_leaf`) and its limits intact.
#[test]
#[ignore = "ADR 0141 amendment 1: G1 missed limit (h) (Plots not in front after Run reopened the Viewport); G1b checks the change"]
fn g1_one_click_from_case_to_picture_known_miss() {
    g1("g1", true);
}

/// G1b (amendment 1): G1's fixture, procedure and limits with a closed Viewport reopened above the Plots.
#[test]
fn g1b_one_click_from_case_to_picture() {
    g1("g1b", false);
}

fn g1(tag: &str, reopen_in_focused_leaf: bool) {
    let n = steps(3000);
    let p = project(tag, "live.toml", &cavity(n), false);
    let ctx = egui::Context::default();
    let mut app = app_on(&p, "live.toml", &ctx);
    app.st.quick.switches.reopen_in_focused_leaf = reopen_in_focused_leaf;
    let gpu = vp::adapter_list().first();
    match gpu {
        Some(a) => {
            eprintln!("G1 (g) on {}", vp::adapter_name(a));
            app.st.viewport.set_gpu(rcfd_studio::viewport::ViewGpu::from_adapter(a));
        }
        None => eprintln!("SKIP {}:{}: G1 (g): no wgpu adapter (DX12 or Vulkan)", file!(), line!()),
    }
    // the layout a user may have left: the Viewport closed, Outputs in front of Plots and Log
    layout::hide(&mut app.st.dock, Panel::Viewport);
    g4::put_in_front(&mut app, Panel::Outputs);
    assert!(!p.client.runner_running(), "a runner is alive before the click");
    let t0 = click(&mut app, &ctx, "Run");
    let since = |t: Instant| t.duration_since(t0).as_secs_f64();
    let mut s = Seen::default();
    let mut id: Option<String> = None;
    let end = t0 + Duration::from_secs(300);
    let mut final_len_at: Option<Instant> = None;
    loop {
        let _ = common::frame(&mut app, &ctx);
        let now = Instant::now();
        let st = &app.st;
        if id.is_none() {
            id = st.selected_run.clone().filter(|i| st.runs.row(i).is_some());
            if id.is_some() {
                s.listed = Some(since(now));
                s.focus_after_listed = Some((
                    layout::is_open(&st.dock, Panel::Viewport),
                    g4::in_front(&app, Panel::Viewport),
                    g4::in_front(&app, Panel::Plots),
                ));
                s.gpu_at_listed = st.viewport.gpu().map(|g| (g.uploads, g.renders));
            }
        }
        let Some(rid) = id.clone() else {
            assert!(now < t0 + Duration::from_secs(60), "the run was not listed");
            std::thread::sleep(Duration::from_millis(5));
            continue;
        };
        let st = &app.st;
        let row = st.runs.row(&rid).cloned();
        let state = row.as_ref().map(|r| r.state);
        if s.running.is_none() && state.is_some_and(|x| x == RunState::Running || x.is_terminal()) {
            s.running = Some(since(now));
        }
        let store = p.run_dir(&rid).join("fields");
        if s.store_first.is_none() && store_snapshots(&store) > 0 {
            s.store_first = Some(since(Instant::now()));
        }
        let vpm = &st.viewport;
        let len = vpm.snaps.len();
        if s.lengths.last() != Some(&len) {
            s.lengths.push(len);
            if state == Some(RunState::Running) {
                s.lengths_running.push(len);
            }
        }
        // ADR 0141 amendment 3: frames of the run only (the Mesh side's preview of the case has a frame of its own)
        if let Some(f) = vpm
            .frame
            .as_ref()
            .filter(|_| !vpm.pending() && vpm.run.as_deref() == Some(rid.as_str()))
        {
            if s.frame_first.is_none() {
                s.frame_first = Some(since(now));
                s.rendered_first = vpm
                    .gpu()
                    .map(|g| (g.renders, vpm.gpu_error().map(String::from).or(vpm.error.clone())));
            }
            if s.indices.last() != Some(&f.index) {
                s.indices.push(f.index);
            }
            if st.plot_marker != Some(f.step) {
                s.marker_bad
                    .push(format!("marker {:?} at step {}", st.plot_marker, f.step));
            }
        }
        // the end: the run terminal, the slider at the store's final length, and then the newest snapshot shown
        if state.is_some_and(RunState::is_terminal) {
            let total = store_snapshots(&store);
            if total > 0 && len == total {
                let at = *final_len_at.get_or_insert(now);
                let waited = now.duration_since(at).as_secs_f64();
                if vpm.frame.as_ref().is_some_and(|f| f.index + 1 == total) && !vpm.pending() {
                    s.newest_after = Some(waited);
                    break;
                }
                assert!(
                    waited <= LAST_S,
                    "the newest snapshot was not shown within {LAST_S} s of the final slider length"
                );
            }
        }
        assert!(now < end, "the run did not end within 300 s: {s:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
    let rid = id.unwrap();
    // let the final drawing happen (the GPU render of the last frame)
    for _ in 0..3 {
        let _ = common::frame(&mut app, &ctx);
    }
    let disk = p.status(&rid).unwrap();
    let row = app.st.runs.row(&rid).cloned().unwrap();
    let strip = strip_view(&app.st).text();
    let frame = app.st.viewport.frame.clone().unwrap();
    let total = store_snapshots(&p.run_dir(&rid).join("fields"));
    let drawn = g4::texts(&common::frame(&mut app, &ctx));
    eprintln!(
        "G1: listed {:?} s, running {:?} s after the click; first snapshot committed {:?} s, shown {:?} s; slider \
         lengths {:?} (while running {:?}); shown indices {:?}; newest shown {:?} s after the final length; marker \
         problems {}; focus after listing (viewport open, viewport in front, plots in front) {:?}; first frame \
         renders and error {:?}",
        s.listed,
        s.running,
        s.store_first,
        s.frame_first,
        s.lengths,
        s.lengths_running,
        s.indices,
        s.newest_after,
        s.marker_bad.len(),
        s.focus_after_listed,
        s.rendered_first
    );
    eprintln!(
        "G1 end: status.json {:?} step {} time {:e}; model {:?} step {} time {:e}; store {total} snapshots, shown \
         {}/{} step {}; strip: {strip}",
        disk.state,
        disk.step,
        disk.time,
        row.state,
        row.step,
        row.time,
        frame.index + 1,
        app.st.viewport.snaps.len(),
        frame.step
    );
    // (a), (b)
    assert!(s.listed.is_some_and(|t| t <= LISTED_S), "listed {:?}", s.listed);
    assert!(s.running.is_some_and(|t| t <= RUNNING_S), "running {:?}", s.running);
    // (c)
    let (Some(w), Some(f)) = (s.store_first, s.frame_first) else {
        panic!("no first snapshot or no frame: {s:?}")
    };
    assert!(f - w <= FIRST_S, "first snapshot shown {:.3} s after its commit", f - w);
    // (d)
    assert!(
        s.lengths.windows(2).all(|x| x[1] >= x[0]),
        "the slider shrank: {:?}",
        s.lengths
    );
    let distinct_running = s.lengths_running.iter().filter(|&&l| l > 0).count();
    assert!(distinct_running >= 3, "under-powered: {:?}", s.lengths_running);
    assert!(s.indices.len() >= 2, "the view did not move: {:?}", s.indices);
    assert_eq!(app.st.viewport.snaps.len(), total);
    // (e)
    assert!(s.marker_bad.is_empty(), "{:?}", s.marker_bad);
    assert_eq!(app.st.plot_marker, Some(disk.step as f64));
    // (f)
    assert_eq!(disk.state, RunState::Finished);
    assert_eq!(
        (row.state, row.step, row.time.to_bits()),
        (disk.state, disk.step, disk.time.to_bits())
    );
    assert!(
        strip.contains("finished") && strip.contains(&format!("step {}", disk.step)),
        "{strip}"
    );
    assert!(drawn
        .iter()
        .any(|t| t.contains(&format!("step {}", disk.step)) && t.contains("finished")));
    assert_eq!(frame.step, disk.step as f64);
    // (g), counted from the frame where the run is first listed (ADR 0141 amendment 3)
    if gpu.is_some() {
        let (renders, err) = s.rendered_first.clone().unwrap_or((0, Some("no GPU".into())));
        let g = app.st.viewport.gpu().unwrap();
        let (u0, r0) = s.gpu_at_listed.unwrap_or((0, 0));
        eprintln!(
            "G1 (g): renders at the first frame {renders}, at the end {}, error {err:?}; at the listing uploads {u0} \
             renders {r0}, at the end uploads {}",
            g.renders, g.uploads
        );
        assert!(err.is_none(), "{err:?}");
        assert!(
            g.renders > r0 && g.uploads - u0 == 1,
            "since the listing: renders {} uploads {}",
            g.renders - r0,
            g.uploads - u0
        );
    }
    // (h)
    assert_eq!(s.focus_after_listed, Some((true, true, true)), "focus after the click");
}
