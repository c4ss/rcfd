//! ADR 0143 (job 48I) N1 and N2: the runner started by the Studio's Run path. N1: the first Run of a session starts the
//! solver within 1.5 s of the click (release builds). N2: the runner survives the exit of the Studio process that
//! started it. Run this binary with its output going to a file (ADR 0123 S5a): the direct start hands the runner the
//! caller's inheritable handles (ADR 0143 decision 1). The drop guard of `RunnerProject` shuts the runner down and kills
//! only the run's solver.

mod common;
mod g4;

use g4::{app_on, cavity, click, project};
use rcfd_project::RunState;
use rcfd_studio::bins::Search;
use rcfd_studio::Studio;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// N1: the solver started (`status.json` running with a pid, or terminal) within this of the click.
const START_S: f64 = 1.5;

/// N2: the child's environment (the project folder and the runner).
const N2_DIR: &str = "RCFD_W48I_N2_DIR";
const N2_RUNNER: &str = "RCFD_W48I_N2_RUNNER";

/// N1: the first Run of a session, no runner alive, through the drawn app.
#[test]
#[cfg_attr(debug_assertions, ignore = "timing criterion is for release builds (ADR 0143 N1)")]
fn n1_first_run_of_a_session_starts_the_solver_within_1_5_s() {
    let p = project("n1", "live.toml", &cavity(2000), false);
    let ctx = egui::Context::default();
    let mut app = app_on(&p, "live.toml", &ctx);
    assert!(!p.client.runner_running(), "a runner is alive before the click");
    let dot = p.dir.join(".rcfd");
    let t0 = click(&mut app, &ctx, "Run");
    let since = |t: Instant| t.duration_since(t0).as_secs_f64();
    let (mut queued, mut runner, mut check, mut started, mut shown) = (None, None, None, None, None);
    let mut id: Option<String> = None;
    let mut first = None;
    let end = t0 + Duration::from_secs(60);
    while started.is_none() || shown.is_none() {
        let _ = common::frame(&mut app, &ctx);
        let now = Instant::now();
        if id.is_none() {
            id = p.ids().into_iter().next();
        }
        if let Some(rid) = id.clone() {
            queued.get_or_insert(since(now));
            if runner.is_none() && dot.join("runner.json").is_file() {
                runner = Some(since(now));
            }
            if check.is_none() && p.run_dir(&rid).join("check.json").is_file() {
                check = Some(since(now));
            }
            if started.is_none() {
                if let Some(s) = p
                    .status(&rid)
                    .filter(|s| (s.state == RunState::Running && s.pid.is_some()) || s.state.is_terminal())
                {
                    started = Some(since(now));
                    first = Some(s);
                }
            }
            let row = app.st.runs.row(&rid).map(|r| r.state);
            if shown.is_none() && row.is_some_and(|x| x == RunState::Running || x.is_terminal()) {
                shown = Some(since(now));
            }
        }
        assert!(
            now < end,
            "N1: no start within 60 s: queued {queued:?}, runner {runner:?}, check {check:?}, started {started:?}, \
             shown {shown:?}; start error {:?}, runner error {:?}",
            app.st.quick.error,
            app.st.quick.runner_error
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    let rid = id.unwrap();
    let first = first.unwrap();
    let end_state = p.client.wait(&rid, Duration::from_secs(120)).unwrap();
    eprintln!(
        "N1: after the click: run folder {queued:?} s, runner.json {runner:?} s, check.json {check:?} s, status.json \
         {:?} (pid {:?}) {started:?} s, shown by the model {shown:?} s; end {:?} step {}; runner error {:?}",
        first.state, first.pid, end_state.state, end_state.step, app.st.quick.runner_error
    );
    assert_eq!(
        (end_state.state, end_state.step),
        (RunState::Finished, 2000),
        "{end_state:?}"
    );
    let dt = started.unwrap();
    assert!(
        dt <= START_S,
        "the solver started {dt:.3} s after the click (limit {START_S} s)"
    );
}

fn status_of(dir: &Path, id: &str) -> Option<rcfd_project::RunStatus> {
    std::fs::read(dir.join("runs").join(id).join("status.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
}

/// N2's child: the Studio's Run path in a process of its own, which ends as soon as the run is running. Without the
/// parent's environment it does nothing (a documented child entry point).
#[test]
fn n2_child_entry() {
    let (Some(dir), Some(runner)) = (std::env::var_os(N2_DIR), std::env::var_os(N2_RUNNER)) else {
        return;
    };
    let (dir, runner) = (PathBuf::from(dir), PathBuf::from(runner));
    let mut st = Studio::in_memory();
    st.runner_exe = runner.clone();
    st.quick.search = Search::explicit(&runner);
    st.open_project(&dir);
    assert!(
        st.drain_until(g4::WAIT, |s| s.browser().is_some() && s.runs.last_tick.is_some()),
        "N2 child: the project did not open"
    );
    st.open_case("cases/live.toml");
    assert!(
        st.drain_until(g4::WAIT, |s| s.editor.is_some()),
        "N2 child: the case did not load"
    );
    rcfd_studio::quick_run::run_selected_case(&mut st).unwrap();
    let ok = st.drain_until(Duration::from_secs(60), |s| {
        s.quick.starting.is_none()
            && s.quick
                .run
                .as_ref()
                .and_then(|(id, _)| status_of(&dir, id))
                .is_some_and(|x| x.state == RunState::Running)
    });
    assert!(
        ok && st.quick.error.is_none() && st.quick.runner_error.is_none(),
        "N2 child: not running (start error {:?}, runner error {:?})",
        st.quick.error,
        st.quick.runner_error
    );
    let id = st.quick.run.as_ref().unwrap().0.clone();
    println!("N2 RUN {id}");
    std::io::stdout().flush().unwrap();
    std::process::exit(0);
}

/// N2: the runner survives the exit of the Studio process that started it; negative control: the lock probe reads no
/// runner before the start and after the shutdown.
#[test]
fn n2_the_runner_survives_the_studio_process() {
    let p = project("n2", "live.toml", &cavity(15_000), false);
    let before = p.client.runner_running();
    let tmp = p.dir.parent().unwrap().to_path_buf();
    let (out, err) = (tmp.join("n2-child-out.txt"), tmp.join("n2-child-err.txt"));
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "n2_child_entry", "--nocapture", "--test-threads", "1"])
        .env(N2_DIR, &p.dir)
        .env(N2_RUNNER, &p.runner)
        .stdin(Stdio::null())
        .stdout(std::fs::File::create(&out).unwrap())
        .stderr(std::fs::File::create(&err).unwrap())
        .spawn()
        .unwrap();
    let read = |f: &Path| std::fs::read_to_string(f).unwrap_or_default();
    let t0 = Instant::now();
    let code = loop {
        if let Some(c) = child.try_wait().unwrap() {
            break c;
        }
        if t0.elapsed() > Duration::from_secs(120) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("N2: the child did not end within 120 s; its stderr: {}", read(&err));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let alive_after_exit = p.client.runner_running();
    let text = read(&out);
    let id = text
        .lines()
        .find_map(|l| l.find("N2 RUN ").map(|i| l[i + 7..].trim().to_string()));
    let state_after_exit = id.as_ref().and_then(|i| p.status(i)).map(|s| s.state);
    eprintln!(
        "N2: lock probe before the start {before}; child exit {code:?} after {:.3} s, run {id:?}; after the exit: \
         runner {alive_after_exit}, run {state_after_exit:?}",
        t0.elapsed().as_secs_f64()
    );
    assert!(
        !before,
        "negative control: the lock probe reads a runner before the start"
    );
    assert!(
        code.success(),
        "N2: the child failed ({code:?}); stdout: {text}; stderr: {}",
        read(&err)
    );
    let id = id.expect("N2: the child printed no run id");
    assert!(alive_after_exit, "the runner died with the Studio process");
    assert!(
        state_after_exit.is_some_and(|s| !s.is_terminal()),
        "set-up: the run was not running when the Studio process ended ({state_after_exit:?})"
    );
    let s = p.client.wait(&id, Duration::from_secs(300)).unwrap();
    let alive_after_run = p.client.runner_running();
    p.client.shutdown(Duration::from_secs(10)).unwrap();
    let after_shutdown = p.client.runner_running();
    eprintln!(
        "N2: run {:?} step {}; runner after the run {alive_after_run}; lock probe after the shutdown {after_shutdown}",
        s.state, s.step
    );
    assert_eq!((s.state, s.step), (RunState::Finished, 15_000), "{s:?}");
    assert!(alive_after_run, "the runner did not outlive the run");
    assert!(
        !after_shutdown,
        "negative control: the lock probe reads a runner after the shutdown"
    );
}
