//! ADR 0742 (F-262): the order of the final writes of a run. A reader takes the terminal `status.json` as the sign that
//! the run folder is done (ADR 0123 S7), so at the instant that status is written the last state event, `metrics.json`
//! and the absence of the request files must already be on disk. The store of these tests looks at the folder from
//! inside each write, so nothing here depends on timing or on the load of the machine.
//!
//! - D1: a held run that is cancelled (the failure of S7). D2: a run cancelled before its launch. D3: a finished run.
//!   D4: a failed run. D5: a cancelled run whose earlier runner died between the removal of `cancel` and the status.
//! - B1: the records of a run that is not cancelled (D3, D4) are the bytes of the formulae before ADR 0742.

use super::*;
use std::cell::RefCell;

/// What the folder of a run held at one write of its end.
#[derive(Debug, Clone, PartialEq)]
enum Seen {
    /// At the write of a terminal status: the `to` of the last state event, the state of `metrics.json`, the request
    /// files present.
    Status {
        state: RunState,
        last_event: Value,
        metrics: Value,
        requests: Vec<&'static str>,
    },
    /// At the append of a terminal state event: whether `status.json` was terminal already.
    Event { state: Value, status_terminal: bool },
}

/// A store that records the folder at each terminal write and then writes as [`FsStore`] does.
struct Watch {
    fs: FsStore,
    seen: RefCell<Vec<Seen>>,
}

impl Watch {
    fn new(fs: &FsStore) -> Watch {
        Watch {
            fs: fs.clone(),
            seen: RefCell::new(Vec::new()),
        }
    }
}

fn is_terminal_name(v: &Value) -> bool {
    ["finished", "failed", "cancelled"].iter().any(|s| v == s)
}

fn last_state_event(fs: &FsStore, id: &str) -> Value {
    events_of(fs, id)
        .iter()
        .rev()
        .find(|v| event_kind(v) == Some("state"))
        .map_or(Value::Null, |v| v["to"].clone())
}

fn requests_present(fs: &FsStore, id: &str) -> Vec<&'static str> {
    [extra::PAUSE, extra::PAUSE_ACK, extra::CANCEL]
        .into_iter()
        .filter(|f| fs.run_dir(id).join(f).exists())
        .collect()
}

impl RunStore for Watch {
    fn project(&self) -> &Path {
        self.fs.project()
    }
    fn list_runs(&self) -> io::Result<Vec<String>> {
        self.fs.list_runs()
    }
    fn create_run(&self, case: &Path, settings: &RunSettings, v: Option<String>) -> io::Result<String> {
        self.fs.create_run(case, settings, v)
    }
    fn read_record(&self, id: &str) -> io::Result<RunRecord> {
        self.fs.read_record(id)
    }
    fn read_status(&self, id: &str) -> io::Result<RunStatus> {
        self.fs.read_status(id)
    }
    fn write_status(&self, id: &str, status: &RunStatus) -> io::Result<()> {
        if status.state.is_terminal() {
            let metrics: Value = std::fs::read_to_string(self.fs.run_dir(id).join("metrics.json"))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or(Value::Null);
            self.seen.borrow_mut().push(Seen::Status {
                state: status.state,
                last_event: last_state_event(&self.fs, id),
                metrics: metrics["state"].clone(),
                requests: requests_present(&self.fs, id),
            });
        }
        self.fs.write_status(id, status)
    }
    fn append_event(&self, id: &str, event: Value) -> io::Result<()> {
        if event_kind(&event) == Some("state") && is_terminal_name(&event["to"]) {
            self.seen.borrow_mut().push(Seen::Event {
                state: event["to"].clone(),
                status_terminal: self.fs.read_status(id).is_ok_and(|s| s.state.is_terminal()),
            });
        }
        self.fs.append_event(id, event)
    }
    fn append_residual(&self, id: &str, line: &Value) -> io::Result<()> {
        self.fs.append_residual(id, line)
    }
    fn append_residuals(&self, id: &str, lines: &[Value]) -> io::Result<()> {
        self.fs.append_residuals(id, lines)
    }
    fn write_metrics(&self, id: &str, metrics: &Value) -> io::Result<()> {
        self.fs.write_metrics(id, metrics)
    }
}

/// The writes of an end as the contract wants them: one terminal state event, appended while the status was not
/// terminal, and one terminal status, written onto a folder that is already final.
fn final_folder(state: RunState) -> Vec<Seen> {
    let name = serde_json::to_value(state).unwrap();
    vec![
        Seen::Event {
            state: name.clone(),
            status_terminal: false,
        },
        Seen::Status {
            state,
            last_event: name.clone(),
            metrics: name,
            requests: Vec::new(),
        },
    ]
}

const STEPS: u64 = 5;

/// A solver stream of [`STEPS`] steps and, when given, a finish event.
fn stream(fs: &FsStore, id: &str, finish: Option<&Value>) {
    let mut text = String::new();
    for i in 1..=STEPS {
        text.push_str(&format!(
            "{{\"event\":\"iteration\",\"index\":{i},\"time\":{i}.0,\"res_u\":1e-3,\"res_p\":1e-3,\"continuity\":1e-3}}\n"
        ));
    }
    if let Some(f) = finish {
        text.push_str(&format!("{f}\n"));
    }
    std::fs::write(fs.run_dir(id).join(extra::SOLVER_STREAM), text).unwrap();
}

fn interrupted() -> Value {
    json!({"event": "finish", "seq": 1, "state": "interrupted", "exit_code": 7, "reason": "stop file"})
}

fn converged() -> Value {
    json!({"event": "finish", "seq": 1, "state": "converged", "exit_code": 0, "reason": "tolerance reached"})
}

/// An adopted run that a held solver left: status `paused` with the pid and its state event.
fn hold(fs: &FsStore, id: &str) {
    let mut st = RunStatus::new(RunState::Paused);
    st.pid = Some(ADOPTED_PID);
    fs.write_status(id, &st).unwrap();
    let ev = state_event(
        Some(RunState::Running),
        RunState::Paused,
        "held by the solver between two steps",
    );
    fs.append_event(id, ev).unwrap();
}

/// Adopt the run and let its solver be found gone (two missed probes): what the store saw, and the end status.
fn end_adopted(fs: &FsStore, id: &str) -> (Vec<Seen>, RunStatus) {
    let mut r = runner(Watch::new(fs));
    r.probe = scripted;
    PROBE.with(|q| q.borrow_mut().extend([false, false]));
    r.scan();
    r.tick_active();
    r.tick_active();
    PROBE.with(|q| q.borrow_mut().clear());
    let seen = r.store.seen.borrow().clone();
    (seen, fs.read_status(id).unwrap())
}

/// D1: the held run of S7. Before ADR 0742 the status is written first: the last state event is still `paused` and the
/// three request files are there.
#[test]
fn d1_a_cancelled_held_run_is_final_when_its_terminal_status_is_written() {
    let (dir, fs, id) = adopted("f262-d1");
    hold(&fs, &id);
    for f in [extra::PAUSE, extra::PAUSE_ACK, extra::CANCEL] {
        std::fs::write(fs.run_dir(&id).join(f), b"").unwrap();
    }
    stream(&fs, &id, Some(&interrupted()));
    let (seen, st) = end_adopted(&fs, &id);
    let terminal_events = events_of(&fs, &id)
        .iter()
        .filter(|v| event_kind(v) == Some("state") && is_terminal_name(&v["to"]))
        .count();
    eprintln!("D1: {seen:?}; end {:?} at step {}: {:?}", st.state, st.step, st.reason);
    assert_eq!(st.state, RunState::Cancelled, "{st:?}");
    assert_eq!(st.step, STEPS);
    assert_eq!(seen, final_folder(RunState::Cancelled));
    assert_eq!(terminal_events, 1);
    assert!(requests_present(&fs, &id).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// D2: a run that is held and cancelled before its launch.
#[test]
fn d2_a_run_cancelled_before_launch_is_final_when_its_terminal_status_is_written() {
    let dir = project("f262-d2");
    let fs = FsStore::new(&dir);
    let settings = RunSettings {
        mem_mb: Some(10),
        ..RunSettings::default()
    };
    let id = fs.create_run(&dir.join("cases/cavity.toml"), &settings, None).unwrap();
    for f in [extra::PAUSE, extra::CANCEL] {
        std::fs::write(fs.run_dir(&id).join(f), b"").unwrap();
    }
    let mut r = runner(Watch::new(&fs));
    r.scan();
    r.tick_pending();
    let seen = r.store.seen.borrow().clone();
    let st = fs.read_status(&id).unwrap();
    let metrics: Value =
        serde_json::from_str(&std::fs::read_to_string(fs.run_dir(&id).join("metrics.json")).unwrap()).unwrap();
    eprintln!("D2: {seen:?}; end {:?}: {:?}", st.state, st.reason);
    assert_eq!(st.state, RunState::Cancelled, "{st:?}");
    assert_eq!(metrics["launched"], false);
    assert_eq!(seen, final_folder(RunState::Cancelled));
    assert!(requests_present(&fs, &id).is_empty() && r.pending.is_empty() && r.launch_seq == 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// D3: a finished run.
#[test]
fn d3_a_finished_run_is_final_when_its_terminal_status_is_written() {
    let (dir, fs, id) = adopted("f262-d3");
    stream(&fs, &id, Some(&converged()));
    let (seen, st) = end_adopted(&fs, &id);
    eprintln!("D3: {seen:?}; end {:?}: {:?}", st.state, st.reason);
    assert_eq!(st.state, RunState::Finished, "{st:?}");
    assert_eq!(seen, final_folder(RunState::Finished));
    let _ = std::fs::remove_dir_all(&dir);
}

/// D4: a run whose solver is gone without a finish event.
#[test]
fn d4_a_failed_run_is_final_when_its_terminal_status_is_written() {
    let (dir, fs, id) = adopted("f262-d4");
    stream(&fs, &id, None);
    let (seen, st) = end_adopted(&fs, &id);
    eprintln!("D4: {seen:?}; end {:?}: {:?}", st.state, st.reason);
    assert_eq!(st.state, RunState::Failed, "{st:?}");
    assert_eq!(seen, final_folder(RunState::Failed));
    let _ = std::fs::remove_dir_all(&dir);
}

/// D5: the earlier runner appended the `cancelled` state event and removed the request files, and died before the
/// status. The next runner adopts the run and ends it as cancelled, from the `cancel_requested` event. Control: without
/// that event the stopped solver is a failure, as before.
#[test]
fn d5_a_cancel_whose_runner_died_before_the_status_still_ends_cancelled() {
    let outcome = |requested: bool| {
        let (dir, fs, id) = adopted(if requested { "f262-d5" } else { "f262-d5-control" });
        hold(&fs, &id);
        if requested {
            fs.append_event(&id, event_line("cancel_requested", json!({}))).unwrap();
        }
        let ev = state_event(Some(RunState::Paused), RunState::Cancelled, "cancelled on request");
        fs.append_event(&id, ev).unwrap();
        stream(&fs, &id, Some(&interrupted()));
        let (_, st) = end_adopted(&fs, &id);
        let _ = std::fs::remove_dir_all(&dir);
        (st.state, st.reason.unwrap_or_default())
    };
    let (state, reason) = outcome(true);
    let (c_state, c_reason) = outcome(false);
    eprintln!("D5: {state:?}: {reason}; control (no cancel_requested event): {c_state:?}: {c_reason}");
    assert_eq!(state, RunState::Cancelled);
    assert!(reason.starts_with("cancelled on request"), "{reason}");
    assert_eq!(c_state, RunState::Failed, "control");
    assert!(c_reason.contains("'interrupted'"), "{c_reason}");
}

/// The bytes `atomic::write_json` gives for `v` (the writer of `metrics.json`).
fn json_file_bytes(dir: &Path, v: &Value) -> Vec<u8> {
    let p = dir.join("f262-expected.json");
    rcfd_project::atomic::write_json(&p, v).unwrap();
    let bytes = std::fs::read(&p).unwrap();
    std::fs::remove_file(&p).unwrap();
    bytes
}

/// The records of an ended run against the formulae before ADR 0742, copied here: `status.json` (with the file's own
/// `updated`), `metrics.json`, the kinds of `events.jsonl` and its last line (with the line's own `t`), the file names.
fn assert_records(dir: &Path, fs: &FsStore, id: &str, state: RunState, reason: &str, finish: Option<&Value>) {
    let run = fs.run_dir(id);
    let exit_code = finish.and_then(|f| f["exit_code"].as_i64()).map(|c| c as i32);
    let on_disk = fs.read_status(id).unwrap();
    let mut st = RunStatus::new(state);
    st.step = STEPS;
    st.time = STEPS as f64;
    st.reason = Some(reason.to_string());
    st.exit_code = exit_code;
    st.updated = on_disk.updated.clone();
    let mut want = serde_json::to_vec_pretty(&st).unwrap();
    want.push(b'\n');
    assert_eq!(
        String::from_utf8_lossy(&std::fs::read(run.join("status.json")).unwrap()),
        String::from_utf8_lossy(&want),
        "status.json"
    );
    let last = read_last_line(&run.join("residuals.jsonl")).and_then(|l| serde_json::from_str::<Value>(&l).ok());
    assert_eq!(last.as_ref().map(|l| l["step"].clone()), Some(json!(STEPS)));
    let metrics = json!({
        "state": state, "reason": reason, "exit_code": exit_code, "steps": STEPS, "time": STEPS as f64,
        "solver_finish": finish, "last_residuals": last, "reports": {},
    });
    assert_eq!(
        String::from_utf8_lossy(&std::fs::read(run.join("metrics.json")).unwrap()),
        String::from_utf8_lossy(&json_file_bytes(dir, &metrics)),
        "metrics.json"
    );
    let text = std::fs::read_to_string(run.join("events.jsonl")).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let kinds: Vec<String> = lines
        .iter()
        .map(|l| {
            event_kind(&serde_json::from_str::<Value>(l).unwrap())
                .unwrap()
                .to_string()
        })
        .collect();
    let mut want_kinds = vec!["state", "adopted"];
    if finish.is_some() {
        want_kinds.push("solver");
    }
    want_kinds.push("state");
    assert_eq!(kinds, want_kinds, "events.jsonl");
    let last_line = lines.last().unwrap();
    let mut ev = state_event(Some(RunState::Running), state, reason);
    ev["t"] = serde_json::from_str::<Value>(last_line).unwrap()["t"].clone();
    assert_eq!(*last_line, ev.to_string(), "the last line of events.jsonl");
    assert!(text.ends_with('\n'));
    let mut names: Vec<String> = std::fs::read_dir(&run)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "case.snapshot",
            "events.jsonl",
            "metrics.json",
            "residuals.jsonl",
            "run.json",
            "solver-status.jsonl",
            "status.json"
        ],
        "the files of the run folder"
    );
}

/// B1: a run that is not cancelled keeps its records byte for byte (a finished one and a failed one).
#[test]
fn b1_the_records_of_a_run_that_is_not_cancelled_are_unchanged() {
    let (dir, fs, id) = adopted("f262-b1-finished");
    let fin = converged();
    stream(&fs, &id, Some(&fin));
    end_adopted(&fs, &id);
    assert_records(
        &dir,
        &fs,
        &id,
        RunState::Finished,
        "converged: tolerance reached",
        Some(&fin),
    );
    let _ = std::fs::remove_dir_all(&dir);

    let (dir, fs, id) = adopted("f262-b1-failed");
    stream(&fs, &id, None);
    end_adopted(&fs, &id);
    let log = fs.run_dir(&id).join("log.txt");
    let reason = format!(
        "solver process {ADOPTED_PID} ended without a finish event (found not running by the liveness probe); last step \
         {STEPS} at time {}; last log line: '' (log: {})",
        STEPS as f64,
        log.display()
    );
    assert_records(&dir, &fs, &id, RunState::Failed, &reason, None);
    let _ = std::fs::remove_dir_all(&dir);
}
