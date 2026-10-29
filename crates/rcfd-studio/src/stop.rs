//! Stop in Run's place (ADR 0161, job 48AA). While the selected case has a live run that the toolbar's Run started, the
//! toolbar shows Stop where Run was ([`crate::stop_ui`] draws it; Ctrl+. is its shortcut). Stop is the runner's existing
//! graceful stop, the request ADR 0133 maps Cancel onto: `Client::cancel` writes `runs/<id>/cancel`, which is also the
//! solver's `--stop-file`; the solver stops between two steps, its field store writes the state of that step as the
//! last snapshot and closes the store (`fieldstore::Hook::finish`), and the runner records the run `cancelled` with the
//! solver's exit code 7 (interrupted). The Studio shows such a run as "stopped" ([`stopped_cleanly`]); `status.json`
//! keeps `cancelled`. A run still in the queue is taken out of it (the runner's "cancelled before launch").
//!
//! Stop asks on a short-lived control thread (a file write, never on the UI thread) and is a no-op, without a message,
//! for a run that has already ended (also when it ends between the click and the request). Until the run has ended
//! the toolbar shows "Stopping..." and Run (and F5) stay refused for that case. No UI code here.

use crate::runs::RunRow;
use crate::studio::Studio;
use rcfd_project::RunState;
use rcfd_runner::client::Client;
use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Instant;

/// The state label of a run that stopped cleanly on request.
pub const STOPPED: &str = "stopped";

/// The solver's exit code of a run stopped through its stop file (`rcfd_api::runctl::exit::INTERRUPTED`).
pub const INTERRUPTED: i32 = rcfd_api::runctl::exit::INTERRUPTED as i32;

/// The start of the status-bar notice of a Stop request.
pub const ASKED: &str = "stop requested for run";

/// The start of the message of a Stop request that failed.
pub const FAILED: &str = "Stop: ";

/// True for a run that stopped cleanly on request (Stop, or Cancel of the strip and the Runs panel): `cancelled` with
/// the solver's exit code 7. A run cancelled before launch (no exit code) or killed after the runner's grace (no
/// finish event) is not.
pub fn stopped_cleanly(r: &RunRow) -> bool {
    r.state == RunState::Cancelled && r.exit_code == Some(INTERRUPTED)
}

/// Negative-control switches (tests only; all off in the app).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Switches {
    /// The route before ADR 0161: no Stop; the toolbar keeps Run (and F5 starts another run) while a run runs.
    pub no_stop: bool,
    /// Run is drawn and F5 is not refused while a run of the selected case is stopping.
    pub run_while_stopping: bool,
}

/// A control thread's answer: `Ok(true)` the stop was requested, `Ok(false)` the run had already ended.
struct Reply {
    gen: u64,
    id: String,
    result: Result<bool, String>,
}

/// The Stop model, a field of [`Studio`].
pub struct StopState {
    /// Negative-control switches (tests only).
    pub switches: Switches,
    /// Runs Stop asked to stop and when, until they have ended.
    pub stopping: BTreeMap<String, Instant>,
    /// The runs the toolbar's Run queued, with their case, oldest first.
    started: Vec<(String, String)>,
    gen: u64,
    tx: Sender<Reply>,
    rx: Receiver<Reply>,
}

impl Default for StopState {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        StopState {
            switches: Switches::default(),
            stopping: BTreeMap::new(),
            started: Vec::new(),
            gen: 0,
            tx,
            rx,
        }
    }
}

impl StopState {
    /// Forget the open project's runs; what a request still in flight answers later is dropped.
    pub fn close(&mut self) {
        self.gen += 1;
        self.stopping.clear();
        self.started.clear();
    }
}

/// What the toolbar shows in Run's place for the selected case.
#[derive(Debug, Clone, PartialEq)]
pub enum View {
    /// Run, as before.
    Run,
    /// A start of the selected case is in flight and its run is not queued yet: Stop, greyed out.
    Preparing {
        /// The case.
        case: String,
        /// What the start is doing.
        stage: String,
    },
    /// The selected case's newest live run that the toolbar started: Stop.
    Live {
        /// Run id.
        id: String,
        /// The case.
        case: String,
        /// The run is queued or held (not launched): Stop takes it out of the queue.
        queued: bool,
    },
    /// That run was asked to stop and has not ended: "Stopping...", greyed out.
    Stopping {
        /// Run id.
        id: String,
        /// The case.
        case: String,
        /// When Stop was pressed.
        since: Instant,
    },
}

fn live(row: Option<&RunRow>) -> bool {
    // a run the worker does not list yet (just queued) is live
    row.is_none_or(|r| !r.state.is_terminal())
}

/// What the toolbar shows in Run's place now.
pub fn view(st: &Studio) -> View {
    if st.stop.switches.no_stop {
        return View::Run;
    }
    let Some(case) = crate::quick_run::selected_case(st) else {
        return View::Run;
    };
    // the toolbar's runs of the case, newest first (the last queued one may not be recorded yet)
    let last = st.quick.run.as_ref().map(|(id, p)| (id.as_str(), p.case.as_str()));
    let recorded = st.stop.started.iter().rev().map(|(i, c)| (i.as_str(), c.as_str()));
    let run = last
        .into_iter()
        .chain(recorded)
        .filter(|(_, c)| *c == case)
        .find(|(id, _)| live(st.runs.row(id)));
    if let Some((id, _)) = run {
        if let Some(since) = st.stop.stopping.get(id) {
            if st.stop.switches.run_while_stopping {
                return View::Run;
            }
            return View::Stopping {
                id: id.to_string(),
                case,
                since: *since,
            };
        }
        // a held queued run is `queued` with a pause request; a paused solver is `paused`
        let queued = st.runs.row(id).is_none_or(|r| r.state == RunState::Queued);
        return View::Live {
            id: id.to_string(),
            case,
            queued,
        };
    }
    match &st.quick.starting {
        Some(s) if s.case == case => View::Preparing {
            case,
            stage: s.stage.clone(),
        },
        _ => View::Run,
    }
}

/// Stop: ask the selected case's live toolbar run to stop (the click and Ctrl+.). True when a stop was asked for; false
/// (a no-op, no message) when there is nothing to stop: no such run, its start not queued yet, already stopping, or
/// ended.
pub fn stop(st: &mut Studio) -> bool {
    match view(st) {
        View::Live { id, .. } => request(st, &id),
        _ => false,
    }
}

fn request(st: &mut Studio, id: &str) -> bool {
    let Some(root) = st.browser().map(|b| b.root.clone()) else {
        return false;
    };
    let runner = st.runner_exe.clone();
    st.stop.stopping.insert(id.to_string(), Instant::now());
    st.say(format!(
        "{ASKED} {id}: the solver stops after the step it is on and stores that state as the run's last snapshot"
    ));
    let (tx, wake, gen, id) = (st.stop.tx.clone(), st.wake.clone(), st.stop.gen, id.to_string());
    let _ = std::thread::Builder::new()
        .name("rcfd-studio-stop".into())
        .spawn(move || {
            let client = Client::new(root, runner);
            let ended = |c: &Client| c.status(&id).is_ok_and(|s| s.state.is_terminal());
            let result = if ended(&client) {
                Ok(false)
            } else {
                match client.cancel(&id) {
                    Ok(()) => Ok(true),
                    // it ended between the two reads: nothing to stop
                    Err(_) if ended(&client) => Ok(false),
                    Err(e) => Err(e),
                }
            };
            if tx.send(Reply { gen, id, result }).is_ok() {
                wake();
            }
        });
    true
}

/// Why Run and F5 must not start the selected case now (`None`: they may): the toolbar shows Stop or "Stopping..." for
/// it. Only the toolbar asks; [`crate::quick_run::run_selected_case`] itself is unchanged.
pub fn run_refusal(st: &Studio) -> Option<String> {
    match view(st) {
        View::Live { id, case, .. } => Some(format!(
            "Run: run {id} of {case} is going; press Stop (Ctrl+.) first, or start another run with Options..."
        )),
        View::Stopping { id, case, .. } => Some(format!(
            "Run: run {id} of {case} is stopping; Run is available again once it has stopped"
        )),
        View::Run | View::Preparing { .. } => None,
    }
}

/// The hover text of Stop (and of "Stopping...").
pub fn hover(st: &Studio, v: &View) -> String {
    match v {
        View::Run => String::new(),
        View::Preparing { case, stage } => {
            format!("The run of {case} is being prepared ({stage}); Stop is available once it is queued (Ctrl+.).")
        }
        View::Stopping { id, since, .. } => format!(
            "Run {id} is stopping (asked {:.1} s ago): the solver ends the step it is on and stores its final state. \
             Run is available again once it has stopped.",
            since.elapsed().as_secs_f64()
        ),
        View::Live { id, case, queued: true } => format!(
            "Stop run {id} of {case} (Ctrl+.). It has not started: Stop takes it out of the queue (the Runs panel then \
             lists it as cancelled, before launch); nothing was computed and nothing is stored."
        ),
        View::Live { id, case, .. } => {
            let grace = match &st.runs.config {
                Some(Ok(c)) => c.cancel_grace_ms,
                _ => rcfd_runner::runner::Config::default().cancel_grace_ms,
            } as f64
                / 1000.0;
            let store = st.quick.plan_of(id).and_then(|p| p.store.as_ref()).map(|s| s.on);
            let kept = match store {
                Some(false) => {
                    "This run stores no fields: its residuals, monitors, log and outputs up to that step are kept."
                }
                _ => {
                    "That state is stored as the run's last snapshot; every snapshot stored before is kept, and so are \
                     the residuals, monitors, log and outputs."
                }
            };
            format!(
                "Stop run {id} of {case} (Ctrl+.). The solver stops cleanly after the step it is on. {kept} The run \
                 stays in the Runs panel as \"{STOPPED}\"; select it there to view its snapshots. The case file is not \
                 changed, and the run cannot be continued (no restart file is written). If the solver has not stopped \
                 {} s after Stop, the runner kills it and only the snapshots stored before are kept.",
                crate::viewport::fmt_num(grace)
            )
        }
    }
}

/// Record the run the toolbar's Run queued, apply the control threads' answers and forget runs that have ended; the
/// number of answers. Called at the end of [`Studio::drain`]; never blocks.
pub fn drain(st: &mut Studio) -> usize {
    if let Some((id, plan)) = &st.quick.run {
        if !st.stop.started.iter().any(|(i, _)| i == id) {
            st.stop.started.push((id.clone(), plan.case.clone()));
        }
    }
    let replies: Vec<Reply> = st.stop.rx.try_iter().collect();
    let n = replies.len();
    for r in replies {
        if r.gen != st.stop.gen {
            continue;
        }
        match r.result {
            Ok(true) => {}
            // it had already ended: nothing was asked, nothing to say
            Ok(false) => {
                st.stop.stopping.remove(&r.id);
            }
            Err(e) => {
                st.stop.stopping.remove(&r.id);
                st.say(format!("{FAILED}run {}: {e}", r.id));
            }
        }
    }
    let runs = &st.runs;
    st.stop.stopping.retain(|id, _| live(runs.row(id)));
    n
}
