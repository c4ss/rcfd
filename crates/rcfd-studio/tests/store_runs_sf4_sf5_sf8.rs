//! ADR 0140 SF4 (the plan's snapshot count and size against real stores), SF5 (the store is where the viewport reads it)
//! and SF8 (re-run with fields stored), through the Studio's start path and `rcfd-runner serve` started directly.

mod common;

use common::{studio_on, RunnerProject, RUN_CASE};
use rcfd_project::RunState;
use rcfd_studio::run_fields::EmptyKind;
use rcfd_studio::runs::StartPlan;
use rcfd_studio::store_plan;
use rcfd_studio::viewport::{Status, StoreView};
use rcfd_studio::Studio;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn piso_case() -> String {
    RUN_CASE.replace(
        "algorithm = \"simplec\"",
        "algorithm = \"piso\"\ndt = 0.0025\nend_time = 0.5",
    )
}

fn sod_case() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cases/sod/sod.toml")).unwrap()
}

/// Bytes of every file in a folder.
fn dir_bytes(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().metadata().unwrap().len())
        .sum()
}

/// The plan the start dialog gets for `rel` (through the run worker), with ADR 0133's test settings.
fn dialog_plan(st: &mut Studio, rel: &str) -> StartPlan {
    st.plan_start(rel).unwrap();
    assert!(st.drain_until(
        Duration::from_secs(30),
        |s| matches!(&s.runs.plan, Some(Ok(p)) if p.case == rel)
    ));
    let Some(Ok(mut p)) = st.runs.plan.clone() else {
        unreachable!()
    };
    p.mem_mb = Some(10);
    p
}

/// Start `plan` and return the new run id.
fn start(st: &mut Studio, p: &RunnerProject, plan: StartPlan) -> String {
    let before = p.ids();
    st.start_run(plan).unwrap();
    assert!(st.drain_until(Duration::from_secs(60), |_| p.ids().len() > before.len()));
    p.ids().into_iter().find(|i| !before.contains(i)).unwrap()
}

fn wait_finished(st: &mut Studio, p: &RunnerProject, id: &str) {
    assert!(
        st.drain_until(Duration::from_secs(300), |s| s
            .runs
            .row(id)
            .is_some_and(|r| r.state.is_terminal())),
        "run {id} did not end"
    );
    assert_eq!(p.status(id).unwrap().state, RunState::Finished, "{:?}", p.status(id));
}

/// Select `id` and wait for the viewport to show its last snapshot.
fn show_last(st: &mut Studio, id: &str) {
    st.select_run(id);
    assert!(
        st.drain_until(Duration::from_secs(30), |s| s.viewport.run.as_deref() == Some(id)
            && s.viewport.status == Status::Ready
            && s.viewport
                .frame
                .as_ref()
                .is_some_and(|f| f.index + 1 == s.viewport.snaps.len())
            && !s.viewport.pending()),
        "{id}: {:?}",
        st.viewport.status
    );
}

#[test]
fn sf4_sf5_estimates_and_the_viewport_location() {
    let mut p = RunnerProject::new("sf4", 3);
    std::fs::write(p.dir.join("cases/piso.toml"), piso_case()).unwrap();
    std::fs::write(p.dir.join("cases/sod.toml"), sod_case()).unwrap();
    p.serve();
    let mut st = studio_on(&p.dir, &p.runner);
    let mut runs: Vec<(&str, String, StartPlan)> = Vec::new();
    for (name, rel) in [
        ("(a) steady cavity", "cases/cavity.toml"),
        ("(b) piso cavity", "cases/piso.toml"),
        ("(c) sod", "cases/sod.toml"),
    ] {
        let plan = dialog_plan(&mut st, rel);
        eprintln!(
            "SF4 {name}: set {:?}; {:?}; {:?}",
            plan.set(),
            store_plan::store_line(&plan),
            store_plan::size_line(&plan)
        );
        let id = start(&mut st, &p, plan.clone());
        runs.push((name, id, plan));
    }
    for (_, id, _) in &runs {
        wait_finished(&mut st, &p, id);
    }
    let mut actual: Vec<(usize, u64)> = Vec::new();
    for (name, id, plan) in &runs {
        let dir = p.run_dir(id).join("fields");
        let n = rcfd_tsstore::Reader::open(&dir).unwrap().snapshots().len();
        let bytes = dir_bytes(&dir);
        let c = store_plan::count(&store_plan::effective(plan)).unwrap();
        let est = store_plan::estimate(plan).unwrap();
        let ratio = bytes as f64 / est as f64;
        eprintln!(
            "SF4 {name}: {n} snapshots (plan {} {:?}), {bytes} bytes against the estimate {est} (actual / estimate {ratio:.4})",
            c.n, c.kind
        );
        if name.starts_with("(c)") {
            assert!(n >= 1 && n as u64 <= c.n, "{name}: {n} snapshots");
            assert!(bytes as f64 <= 1.10 * est as f64, "{name}");
        } else {
            assert_eq!(n as u64, c.n, "{name}");
            assert!((ratio - 1.0).abs() <= 0.10, "{name}: ratio {ratio}");
        }
        actual.push((n, bytes));
    }
    // negative control: R10's data-only estimate misses (a) by more than 10 %
    let old = runs[0].2.estimated_bytes().unwrap();
    let miss = old as f64 / actual[0].1 as f64 - 1.0;
    eprintln!(
        "SF4 control: estimated_bytes() {old} against {} bytes: {:+.1} %",
        actual[0].1,
        100.0 * miss
    );
    assert!(miss.abs() > 0.10);
    // SF5: each run in the viewport, at the contract location, values bit-equal to the store
    for ((name, id, _), (n, _)) in runs.iter().zip(&actual) {
        show_last(&mut st, id);
        let dir = p.run_dir(id).join("fields");
        let sv = StoreView::open(&dir).unwrap();
        let mut r = rcfd_tsstore::Reader::open(&dir).unwrap();
        let want = sv
            .surface
            .face_values_from_cells(&r.read_field(n - 1, "p").unwrap())
            .unwrap();
        let f = st.viewport.frame.as_ref().unwrap();
        let equal = f.faces.len() == want.len() && f.faces.iter().zip(&want).all(|(a, b)| a.to_bits() == b.to_bits());
        eprintln!(
            "SF5 {name}: Ready, {} snapshots, note {:?}, last p on {} faces bit-equal: {equal}",
            st.viewport.snaps.len(),
            st.viewport.store_note,
            want.len()
        );
        assert_eq!(st.viewport.snaps.len(), *n);
        assert_eq!(st.viewport.store_note, None);
        assert!(!p.run_dir(id).join("output").join("fields").exists());
        assert!(equal, "{name}");
    }
    // SF5 negative control: (a)'s store moved away
    let (_, a, _) = &runs[0];
    std::fs::rename(p.run_dir(a).join("fields"), p.run_dir(a).join("fields.moved")).unwrap();
    st.select_run(&runs[1].1);
    st.select_run(a);
    assert!(st.drain_until(Duration::from_secs(30), |s| matches!(
        &s.viewport.status,
        Status::NoStore(_)
    )));
    let e = st.viewport.empty.clone().unwrap();
    eprintln!("SF5 control: {:?}: {}", e.kind, e.text);
    assert_eq!(e.kind, EmptyKind::Missing);
    assert_eq!(
        e.text,
        format!("Run {a} asked for stored fields, but no store was found.")
    );
}

#[test]
fn sf8_rerun_with_fields_stored() {
    let mut p = RunnerProject::new("sf8", 3);
    p.serve();
    let mut st = studio_on(&p.dir, &p.runner);
    let plan = p.plan(400, false);
    assert_eq!(plan.set(), ["solver.max_iterations=400"]);
    let old = start(&mut st, &p, plan);
    wait_finished(&mut st, &p, &old);
    st.select_run(&old);
    assert!(st.drain_until(Duration::from_secs(30), |s| matches!(
        &s.viewport.status,
        Status::NoStore(_)
    ) && s.viewport.empty.is_some()));
    let e = st.viewport.empty.clone().unwrap();
    eprintln!("SF8: {:?}: {}; {:?}", e.kind, e.text, e.rerun_line);
    assert_eq!(e.kind, EmptyKind::NotStored);
    assert!(e.rerun);
    assert_eq!(
        e.rerun_line.as_deref(),
        Some("Starts a new run of cases/cavity.toml (unchanged since this run) with the field store on.")
    );
    st.rerun_with_fields(&old).unwrap();
    assert!(st.drain_until(Duration::from_secs(60), |_| p.ids().len() == 2));
    let new = p.ids().into_iter().find(|i| *i != old).unwrap();
    let rec = |id: &str| -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(p.run_dir(id).join("run.json")).unwrap()).unwrap()
    };
    let (ro, rn) = (rec(&old), rec(&new));
    eprintln!("SF8: re-run {new}: set {}, case {}", rn["settings"]["set"], rn["case"]);
    assert_eq!(
        rn["settings"]["set"],
        serde_json::json!([
            "output.store_interval=4",
            "output.store_precision=f32",
            "solver.max_iterations=400"
        ])
    );
    assert_eq!(rn["case"], ro["case"]);
    let snap = |id: &str| std::fs::read(p.run_dir(id).join("case.snapshot")).unwrap();
    assert_eq!(snap(&new), snap(&old));
    wait_finished(&mut st, &p, &new);
    show_last(&mut st, &new);
    eprintln!("SF8: the re-run shows {} snapshots", st.viewport.snaps.len());
    assert_eq!(st.viewport.snaps.len(), 100);
    // negative control: the case changed since the run; refused with the registered text, no run created
    let case: PathBuf = p.dir.join("cases/cavity.toml");
    std::fs::write(&case, format!("{RUN_CASE}# edited\n")).unwrap();
    let said = st.messages.len();
    st.rerun_with_fields(&old).unwrap();
    let want =
        "the run was not started: cases/cavity.toml changed since this run, so it is not re-run; start a new run \
                with Runs > Start run... (\"Store fields\" is on by default)."
            .to_string();
    assert!(
        st.drain_until(Duration::from_secs(30), |s| s.messages[said..].contains(&want)),
        "{:?}",
        &st.messages[said..]
    );
    std::thread::sleep(Duration::from_millis(500));
    eprintln!("SF8 control: refused ({want}); runs {}", p.ids().len());
    assert_eq!(p.ids().len(), 2);
}
