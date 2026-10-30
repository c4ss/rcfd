//! ADR 0172 (job 49J) item A3, limits LC2 and LC3: the Studio worker reads the adaptive corrector count and cap
//! (`correctors`, `corrector_cap`, ADR 0160 D4) of `residuals.jsonl` into the Monitors plot's store; a run without them
//! gets neither series. No runner: fixture runs made with `rcfd-project`, as ADR 0133 R4 and R7.

mod common;

use common::{temp, CAVITY};
use rcfd_project::{Project, RunState, RunStatus};
use rcfd_studio::monitors::MAX_PLOT_POINTS;
use rcfd_studio::Studio;
use std::path::Path;
use std::time::Duration;

fn line(step: u64, n: Option<(u64, u64)>) -> String {
    let r = 1.0 / (step as f64 + 1.0);
    let extra = n.map_or(String::new(), |(c, cap)| {
        format!(",\"correctors\":{c},\"corrector_cap\":{cap}")
    });
    format!(
        "{{\"step\":{step},\"time\":{},\"residuals\":{{\"u\":{r:e},\"p\":{},\"continuity\":{}}},\"monitors\":{{}}{extra}}}\n",
        step as f64 * 1e-3,
        r * 0.5,
        r * 0.25,
    )
}

fn fixture(root: &Path, text: &str) -> String {
    let p = Project::create(root, "w49j fixture", "rcfd test").unwrap();
    std::fs::write(root.join("cases/a.toml"), CAVITY).unwrap();
    let rec = p
        .create_run("cases/a.toml", None, "rcfd test", serde_json::json!({}))
        .unwrap();
    p.write_status(&rec.id, &RunStatus::new(RunState::Running)).unwrap();
    std::fs::write(root.join("runs").join(&rec.id).join("residuals.jsonl"), text).unwrap();
    rec.id
}

fn studio_showing(root: &Path, id: &str, last_step: f64) -> Studio {
    let mut st = Studio::in_memory();
    st.poll = None;
    st.run_poll = Some(Duration::from_millis(20));
    st.runner_exe = root.join("no-runner-here");
    st.open_project(root);
    assert!(st.drain_until(Duration::from_secs(30), |s| s.browser().is_some()
        && s.runs.last_tick.is_some()));
    st.select_run(id);
    assert!(
        st.drain_until(Duration::from_secs(30), |s| s
            .plots
            .series
            .get("res_u")
            .and_then(|m| m.x.last().copied())
            == Some(last_step)),
        "the residuals were not read"
    );
    st
}

#[test]
fn lc2_lc3_the_corrector_series_reach_the_monitors_store() {
    let counts = [2u64, 2, 3, 3, 4, 4, 3, 3];
    let with: String = counts
        .iter()
        .enumerate()
        .map(|(i, &c)| line(i as u64 + 1, Some((c, i as u64 / 3))))
        .collect();
    let root = temp("w49j-lc2");
    let id = fixture(&root, &with);
    let st = studio_showing(&root, &id, 8.0);
    let c = &st.monitors.series["correctors"];
    assert_eq!(c.x, (1..=8).map(f64::from).collect::<Vec<_>>());
    assert_eq!(c.y, vec![2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 3.0, 3.0]);
    assert_eq!(
        st.monitors.series["corrector_cap"].y,
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0]
    );
    // the Monitors plot draws every series of that store (a linear axis); the residual plot's names are unchanged
    let seg = st.monitors.plot_segments("correctors", false, MAX_PLOT_POINTS);
    assert_eq!(seg.iter().map(Vec::len).sum::<usize>(), 8);
    assert_eq!(st.plots.residual_names(), vec!["continuity", "res_p", "res_u"]);
    assert!(!st.plots.series.contains_key("correctors"));
}

#[test]
fn lc2_a_run_without_them_gets_neither_series() {
    let plain: String = (1..=8).map(|i| line(i, None)).collect();
    let root = temp("w49j-lc2-plain");
    let id = fixture(&root, &plain);
    let st = studio_showing(&root, &id, 8.0);
    assert!(!st.monitors.series.contains_key("correctors"));
    assert!(!st.monitors.series.contains_key("corrector_cap"));
    assert_eq!(st.plots.residual_names(), vec!["continuity", "res_p", "res_u"]);
}
