//! ADR 0141 G2: follow-latest. A store written by the test while the model shows its run (no runner, as 48C's
//! `viewport_live`): with follow on each new snapshot is shown; a scrub to an older snapshot turns it off and the view
//! stays while the slider extends; the checkbox (`set_follow`) and a scrub to the newest turn it on again. Controls: the
//! on-check with follow off, and the off-check with follow forced on, fail.

#[macro_use]
mod vp;

use rcfd_mesh::Mesh;
use rcfd_project::RunState;
use rcfd_studio::viewport::Status;
use rcfd_studio::{Studio, StudioApp};
use rcfd_tsstore::{Location, Precision, WriteOptions, Writer};
use std::time::{Duration, Instant};

/// Registered: a new snapshot is shown within 4 viewport poll periods (250 ms) of its commit.
const LIMIT: Duration = Duration::from_millis(1000);
/// Registered: with follow off, the view is watched for 2 poll periods after the slider extended.
const HOLD: Duration = Duration::from_millis(500);

struct Live {
    st: Studio,
    w: Writer,
    mesh: Mesh,
    k: usize,
}

impl Live {
    fn new(tag: &str) -> Live {
        let (root, id, dir) = vp::run_project(tag, RunState::Running, (0, 0.0));
        let mut st = Studio::in_memory();
        st.poll = None;
        st.run_poll = None;
        st.open_project(&root);
        assert!(st.drain_until(vp::WAIT, |s| s.browser().is_some()));
        st.select_run(&id);
        assert!(st.drain_until(vp::WAIT, |s| matches!(s.viewport.status, Status::NoStore(_))));
        let mesh = vp::cavity8();
        std::fs::create_dir_all(&dir).unwrap();
        let mut w = Writer::create(&dir, &mesh, WriteOptions::default()).unwrap();
        w.flush().unwrap();
        assert!(st.drain_until(vp::WAIT, |s| s.viewport.status == Status::Ready));
        assert!(st.viewport.follow, "follow is not on for a newly shown run");
        Live { st, w, mesh, k: 0 }
    }

    /// Commit the next snapshot (step k + 1, time 0.1 (k + 1)); returns its index and the commit time.
    fn commit(&mut self) -> (usize, Instant) {
        let k = self.k;
        let p: Vec<f64> = (0..self.mesh.n_cells()).map(|c| vp::p_v4(c, k)).collect();
        let mut s = self.w.snapshot(k as u64 + 1, 0.1 * (k + 1) as f64).unwrap();
        s.field("p", "unknown", Location::Cell, &p, Precision::F64).unwrap();
        s.commit().unwrap();
        self.w.flush().unwrap();
        self.k += 1;
        (k, Instant::now())
    }

    fn shown(&self) -> Option<usize> {
        let v = &self.st.viewport;
        v.frame.as_ref().filter(|_| !v.pending()).map(|f| f.index)
    }

    /// Wait until snapshot `k` is shown; the time from `t` (None when not within `limit`).
    fn shown_within(&mut self, k: usize, t: Instant, limit: Duration) -> Option<f64> {
        self.st
            .drain_until(limit, |s| {
                s.viewport.snaps.len() == k + 1
                    && !s.viewport.pending()
                    && s.viewport.frame.as_ref().is_some_and(|f| f.index == k)
            })
            .then(|| t.elapsed().as_secs_f64())
    }

    /// After snapshot `k`'s commit: the slider reaches k + 1 within the limit, then for HOLD the shown snapshot stays
    /// `at`; returns (slider time, the indices shown during the hold).
    fn held(&mut self, k: usize, t: Instant, at: usize) -> (Option<f64>, Vec<usize>) {
        let grew = self
            .st
            .drain_until(LIMIT, |s| s.viewport.snaps.len() == k + 1)
            .then(|| t.elapsed().as_secs_f64());
        let mut seen = Vec::new();
        let end = Instant::now() + HOLD;
        while Instant::now() < end {
            self.st.drain();
            if let Some(i) = self.shown() {
                if seen.last() != Some(&i) {
                    seen.push(i);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if seen.is_empty() {
            seen.push(at); // nothing new arrived: still the snapshot shown before
        }
        (grew, seen)
    }
}

#[test]
fn g2_follow_on_shows_each_new_snapshot() {
    let mut l = Live::new("g2-on");
    let mut worst: f64 = 0.0;
    for _ in 0..6 {
        let (k, t) = l.commit();
        let dt = l
            .shown_within(k, t, LIMIT)
            .unwrap_or_else(|| panic!("snapshot {k} not shown within {LIMIT:?} (shown {:?})", l.shown()));
        assert_eq!(l.st.plot_marker, Some(k as f64 + 1.0), "the plots mark the shown step");
        assert!(l.st.viewport.follow);
        worst = worst.max(dt);
    }
    eprintln!("G2 on: 6 snapshots each shown after its commit, worst {worst:.3} s");
    l.w.finish().unwrap();
}

#[test]
fn g2_control_follow_off_does_not_show_new_snapshots() {
    let mut l = Live::new("g2-on-ctl");
    let (k, t) = l.commit();
    assert!(l.shown_within(k, t, LIMIT).is_some());
    l.st.viewport.set_follow(false);
    let (k, t) = l.commit();
    let got = l.shown_within(k, t, LIMIT);
    eprintln!(
        "G2 on control: follow off, snapshot {k} shown within {LIMIT:?}: {got:?}; shown {:?}, slider {}",
        l.shown(),
        l.st.viewport.snaps.len()
    );
    assert!(got.is_none(), "the on-check passed with follow off");
    assert_eq!(l.st.viewport.snaps.len(), 2, "the slider did not extend");
    l.w.finish().unwrap();
}

/// The off-check: after a scrub to snapshot 0 three more snapshots arrive; `force_on` is the control.
fn off_check(tag: &str, force_on: bool) -> Vec<(usize, Option<f64>, Vec<usize>)> {
    let mut l = Live::new(tag);
    for _ in 0..3 {
        let (k, t) = l.commit();
        assert!(l.shown_within(k, t, LIMIT).is_some(), "snapshot {k} not followed");
    }
    // the user scrubs back (the "|<" button's call)
    l.st.viewport.show_snapshot(0);
    assert!(!l.st.viewport.follow, "a scrub to an older snapshot left follow on");
    if force_on {
        l.st.viewport.follow = true;
    }
    assert!(l.st.drain_until(LIMIT, |s| !s.viewport.pending()
        && s.viewport.frame.as_ref().is_some_and(|f| f.index == 0)));
    let mut out = Vec::new();
    for _ in 0..3 {
        let (k, t) = l.commit();
        let (grew, seen) = l.held(k, t, 0);
        out.push((k, grew, seen));
    }
    if !force_on {
        // the checkbox turns it on again: the newest is shown, and the next snapshot followed
        let t = Instant::now();
        l.st.viewport.set_follow(true);
        let dt = l.shown_within(5, t, LIMIT);
        let (k, t) = l.commit();
        let next = l.shown_within(k, t, LIMIT);
        // "<" (one back) turns it off, ">|" (the newest) on
        l.st.viewport.step(-1);
        let after_back = l.st.viewport.follow;
        l.st.viewport.show_snapshot(l.st.viewport.snaps.len() - 1);
        let after_end = l.st.viewport.follow;
        // the slider: dragged into the middle off, to its right end on
        let (a, b) = l.st.viewport.bounds();
        l.st.viewport.set_pos(0.5 * (a + b));
        let mid = l.st.viewport.follow;
        l.st.viewport.set_pos(b);
        let end = l.st.viewport.follow;
        eprintln!(
            "G2 on again: newest shown after {dt:?} s, next followed after {next:?} s; follow after '<' {after_back}, \
             after '>|' {after_end}, slider middle {mid}, right end {end}"
        );
        assert!(
            dt.is_some_and(|d| d <= LIMIT.as_secs_f64()),
            "set_follow(true) did not show the newest"
        );
        assert!(
            next.is_some_and(|d| d <= LIMIT.as_secs_f64()),
            "the next snapshot was not followed"
        );
        assert!(!after_back && after_end && !mid && end);
    }
    l.w.finish().unwrap();
    out
}

#[test]
fn g2_follow_off_after_a_scrub_keeps_the_view() {
    let out = off_check("g2-off", false);
    eprintln!("G2 off: (snapshot, slider extended after s, indices shown during the hold) {out:?}");
    for (k, grew, seen) in &out {
        assert!(
            grew.is_some_and(|g| g <= LIMIT.as_secs_f64()),
            "the slider did not extend to {}",
            k + 1
        );
        assert_eq!(seen, &vec![0usize], "the view moved after snapshot {k} with follow off");
    }
}

#[test]
fn g2_control_follow_forced_on_moves_the_view() {
    let out = off_check("g2-off-ctl", true);
    eprintln!("G2 off control (follow forced on): {out:?}");
    assert!(
        out.iter().any(|(_, _, seen)| seen.iter().any(|&i| i != 0)),
        "the off-check passed with follow on"
    );
}

/// The time row draws the "follow latest" switch (headless frame, no GPU).
#[test]
fn g2_follow_switch_is_drawn() {
    let mut l = Live::new("g2-drawn");
    let (k, t) = l.commit();
    assert!(l.shown_within(k, t, LIMIT).is_some());
    let mut app = StudioApp::new(l.st, None);
    let ctx = egui::Context::default();
    let drawn = vp::texts(&vp::frame(&mut app, &ctx).shapes);
    assert!(drawn.iter().any(|t| t == "follow latest"), "{drawn:?}");
}
