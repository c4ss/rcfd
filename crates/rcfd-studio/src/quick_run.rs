//! The one-click path from a case to a picture (ADR 0141, wave 48G item 4). With a case selected, [`run_selected_case`]
//! (the toolbar's Run button and F5) starts a run of it with the default plan of the "Start run..." dialog
//! ([`StartPlan::new`], field store proposal included), selects the run as soon as it is queued, brings the Viewport
//! (and Plots) to the front, and the viewport follows the run's snapshots as they are written
//! ([`crate::viewport::Viewport::follow`]). [`strip_view`] is the status strip: state, step, time and residuals of the
//! selected run, with Pause, Resume and Cancel ([`control`], the run worker's route of ADR 0133).
//!
//! A start runs on a short-lived control thread: find `rcfd-runner` and `rcfd` together ([`crate::bins`]), refuse a
//! solver older than the runner's flags, check the runner budgets when no runner is alive (`rcfd-runner serve` refuses
//! a bad budgets file; checked here, the start is refused before a run is queued, while a runner that exits on it is
//! reported by `Client::ensure_runner` with its output, ADR 0146, after the run was queued), read the case and make the
//! plan, `Client::submit` (the run id is reported at once), then `Client::ensure_runner`: ADR 0133's start route
//! (`Client::start`) in its two steps, so that neither result is lost. Every failure is a located message. After the
//! start, the run worker's rows say how the run goes; a selected run that ends before its first stored snapshot gets a
//! message naming its state, the reason in its `status.json` and the last line of its log, in the viewport and in the
//! strip, instead of the viewport's generic "no stored fields" text.
//!
//! Run is refused while a field of the case editor is being typed and not committed (ADR 0147, the maintainer's answer
//! to ADR 0144 question 2): the mesh preview shows the typed text, a run would use the file on disk.
//!
//! No UI code here ([`crate::quick_run_ui`] draws); the UI thread does no file I/O (the store check after a run ended
//! runs on a control thread too).

use crate::bins::{self, Found, Search};
use crate::layout::{self, Panel};
use crate::runs::{PlanRoute, RunRow, StartPlan, StoreEvery};
use crate::studio::Studio;
use crate::viewport::Status;
use crate::watch::Wake;
use egui_dock::{DockState, NodePath};
use rcfd_project::records::EventKind;
use rcfd_project::RunState;
use rcfd_runner::client::Client;
use rcfd_runner::runner::Config;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Instant;

/// Test switches for the registered negative controls (ADR 0141); all off in the app.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Switches {
    /// Do not select the queued run.
    pub no_select: bool,
    /// Do not bring the Viewport, Plots or Log panels to the front.
    pub no_focus: bool,
    /// Do not explain a run that ended before its first stored snapshot (the viewport keeps its own text) and leave the
    /// run's reason out of the strip.
    pub no_explain: bool,
    /// Reopen a closed Viewport in the focused place (`layout::show`), as G1's registered run did (its known-miss test,
    /// amendment 1).
    pub reopen_in_focused_leaf: bool,
    /// The route before ADR 0144: a run explained once is not explained again when the viewport opens its store again
    /// (the Mesh / Run toggle), so the viewport keeps ADR 0140's text then.
    pub no_reexplain: bool,
    /// The route before ADR 0147: a field of the case editor being typed does not refuse Run (only committed, unsaved
    /// edits do).
    pub run_while_typing: bool,
    /// The route before ADR 0147: the last lines of `runner.log` are appended to every error of `ensure_runner`, also to
    /// an early-exit report that already carries the runner's output (ADR 0146), so those lines show twice.
    pub tail_always: bool,
    /// The route before ADR 0148: the selected case is the one in the case editor, else the one being loaded into it
    /// (Run, F5, the toolbar label and the Project panel's highlight); the editor keeps its case until another loads
    /// and when another cannot be opened; a failure of any file operation ends the wait for a load; a run that Run
    /// queued always switches the Viewport to it.
    pub selection_from_editor: bool,
    /// The route before ADR 0148 (the maintainer's answer to ADR 0147 question 4): the Start run dialog's Start is
    /// enabled whatever the case editor holds of its case.
    pub start_ignores_editor: bool,
    /// The route before ADR 0149 (ADR 0148's, the maintainer's answer to its question 1): a click on a case keeps the
    /// selected run, a click on a run keeps the selected case, and a run that Run queued after another case was
    /// selected is selected without switching the Viewport (ADR 0148 decision 1 (d)).
    pub case_run_unlinked: bool,
    /// The route before ADR 0149 (ADR 0148 decision 1 (a), the maintainer's answer to its question 2): the unsaved edits
    /// of the case left are dropped at the click, and closing the project drops them without asking.
    pub drop_drafts: bool,
    /// The route before ADR 0149 (the maintainer's answer to ADR 0148 question 6): Run and its plan read a case as TOML
    /// only (a `.json` case is refused with `not TOML: ...`) and Run is not greyed out for a benchmark descriptor.
    pub cases_as_toml: bool,
    /// The route before ADR 0152 (the maintainer's answer to ADR 0149 question 9): Run's plan takes a store key in the
    /// case text as the case's own whatever its value, so a case saved by the Studio (`store_interval = 0` spelled out)
    /// is run without stored fields ([`crate::runs::PlanRoute::default_store_keys_set`]).
    pub default_store_keys_set: bool,
}

/// A start in flight.
#[derive(Debug, Clone, PartialEq)]
pub struct Starting {
    /// Project-relative case.
    pub case: String,
    /// What the control thread is doing.
    pub stage: String,
    /// When the start was asked for.
    pub since: Instant,
}

/// What the control threads report.
#[derive(Debug)]
enum Msg {
    Located(Result<Found, String>),
    Stage {
        gen: u64,
        text: String,
    },
    Queued {
        gen: u64,
        id: String,
        plan: Box<StartPlan>,
    },
    Failed {
        gen: u64,
        text: String,
    },
    Runner {
        gen: u64,
        result: Result<(), String>,
    },
    Probe {
        gen: u64,
        id: String,
        snapshots: Result<usize, String>,
    },
}

/// The one-click model, a field of [`Studio`].
pub struct QuickRun {
    /// Where the runner and the solver are looked for (the process environment; tests point it at their folders).
    pub search: Search,
    /// The last search result (at start-up, after every Run, after "search again").
    pub bins: Option<Result<Found, String>>,
    /// The start in flight.
    pub starting: Option<Starting>,
    /// The located error of the last start (shown until the next start).
    pub error: Option<String>,
    /// `ensure_runner`'s error of the last start (the run stays queued).
    pub runner_error: Option<String>,
    /// The run the last start queued, and its plan.
    pub run: Option<(String, StartPlan)>,
    /// The message given to the viewport for a run that ended before its first stored snapshot: (run id, text).
    pub notice: Option<(String, String)>,
    /// Negative-control switches (tests only).
    pub switches: Switches,
    /// Where the toolbar and the strip drew their buttons last (by label), so tests can click them.
    pub rects: BTreeMap<String, egui::Rect>,
    gen: u64,
    locating: bool,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    probed: BTreeSet<String>,
    failed_seen: BTreeSet<String>,
}

impl Default for QuickRun {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        QuickRun {
            search: Search::from_env(),
            bins: None,
            starting: None,
            error: None,
            runner_error: None,
            run: None,
            notice: None,
            switches: Switches::default(),
            rects: BTreeMap::new(),
            gen: 0,
            locating: false,
            tx,
            rx,
            probed: BTreeSet::new(),
            failed_seen: BTreeSet::new(),
        }
    }
}

impl QuickRun {
    /// Forget the open project's start state; what a start still in flight reports later is dropped.
    pub fn close(&mut self) {
        self.gen += 1;
        self.starting = None;
        self.error = None;
        self.runner_error = None;
        self.run = None;
        self.notice = None;
        self.probed.clear();
        self.failed_seen.clear();
    }

    /// True while a search started by [`QuickRun::locate`] has not reported.
    pub fn locating(&self) -> bool {
        self.locating
    }

    /// Search again for the runner and the solver (file checks, on a control thread).
    pub fn locate(&mut self, wake: &Wake) {
        if self.locating {
            return;
        }
        self.locating = true;
        let (search, tx, wake) = (self.search.clone(), self.tx.clone(), wake.clone());
        spawn("rcfd-studio-locate", move || {
            if tx.send(Msg::Located(search.find())).is_ok() {
                wake();
            }
        });
    }

    /// The plan of the run the last start queued, when `id` is that run.
    pub fn plan_of(&self, id: &str) -> Option<&StartPlan> {
        self.run.as_ref().filter(|(r, _)| r == id).map(|(_, p)| p)
    }
}

fn spawn(name: &str, f: impl FnOnce() + Send + 'static) {
    let _ = std::thread::Builder::new().name(name.into()).spawn(f);
}

/// The case the Run button runs: the selected case ([`Studio::selected_case`], set by the click in the Project panel;
/// ADR 0148). The toolbar's label and the Project panel's highlight show the same case. With
/// [`Switches::selection_from_editor`], the route before: the case in the case editor, else the one being loaded.
pub fn selected_case(st: &Studio) -> Option<String> {
    if st.quick.switches.selection_from_editor {
        return st
            .editor
            .as_ref()
            .map(|e| e.rel.clone())
            .or_else(|| st.pending_case.clone());
    }
    st.selected_case.clone()
}

/// The words of Run's refusal while a field of the case editor is being typed (ADR 0147); the field's path follows.
pub const FINISH_EDITING: &str = "finish editing the field";

/// Why a run of `case` must wait for the case editor (`None`: it need not): the editor of `case` (the open one, or one
/// kept with its unsaved edits while another case is selected, ADR 0149) holds it in conflict, with a field being typed
/// (ADR 0147) or with unsaved edits (ADR 0141); a run uses the file on disk. Run's refusal, and the Start run dialog's
/// for its case (ADR 0148, the maintainer's answer to ADR 0147 question 4).
pub fn editor_blocks(st: &Studio, case: &str) -> Option<String> {
    let ed = st.case_editor(case)?;
    if ed.in_conflict() {
        return Some(format!("{case}: resolve the conflict in the case editor first"));
    }
    // ADR 0147: the preview shows a field's text while it is typed; a run would not use it
    if let Some((path, _)) = ed.draft.as_ref().filter(|_| !st.quick.switches.run_while_typing) {
        return Some(format!(
            "{case}: {FINISH_EDITING} {path} (press Enter or click elsewhere); the mesh shows the typed text, but a run \
             uses the file on disk"
        ));
    }
    if ed.is_dirty() {
        return Some(format!(
            "{case} has unsaved edits: save it first (Case > Save); a run uses the file on disk"
        ));
    }
    None
}

/// Why the Start run dialog's Start cannot start a run of `case` now (`None`: it can): [`editor_blocks`] (ADR 0148);
/// with [`Switches::start_ignores_editor`], never (the route before).
pub fn start_blocked(st: &Studio, case: &str) -> Option<String> {
    if st.quick.switches.start_ignores_editor || case.is_empty() {
        return None;
    }
    editor_blocks(st, case)
}

/// Why Run cannot start now (`None`: it can). The Run button is greyed out with this as its hover text.
pub fn blocked(st: &Studio) -> Option<String> {
    if st.browser().is_none() {
        return Some(if st.project.is_some() {
            "the project is still opening".into()
        } else {
            "open a project first (Project > Open project)".into()
        });
    }
    let Some(case) = selected_case(st) else {
        return Some("select a case in the Project panel first".into());
    };
    if let Some(why) = editor_blocks(st, &case) {
        return Some(why);
    }
    // ADR 0149 (the maintainer's answer to ADR 0148 question 6): a benchmark descriptor, known from its read, is never
    // startable: ADR 0147's one line
    if let Some((_, line)) = st
        .not_a_case
        .as_ref()
        .filter(|(c, _)| *c == case && !st.quick.switches.cases_as_toml)
    {
        return Some(line.clone());
    }
    if let Some(s) = &st.quick.starting {
        return Some(format!("a start of {} is in progress ({})", s.case, s.stage));
    }
    None
}

/// The Run button and F5 (the click-equivalent call): start the selected case with the default start plan.
pub fn run_selected_case(st: &mut Studio) -> Result<(), String> {
    if let Some(why) = blocked(st) {
        return Err(format!("Run: {why}"));
    }
    let (Some(case), Some(root)) = (selected_case(st), st.browser().map(|b| b.root.clone())) else {
        return Err("Run: no case or project".into());
    };
    let q = &mut st.quick;
    q.error = None;
    q.runner_error = None;
    q.starting = Some(Starting {
        case: case.clone(),
        stage: "looking for rcfd-runner and rcfd".into(),
        since: Instant::now(),
    });
    let job = Job {
        gen: q.gen,
        root,
        case,
        search: q.search.clone(),
        tail_always: q.switches.tail_always,
        route: PlanRoute {
            toml_only: q.switches.cases_as_toml,
            default_store_keys_set: q.switches.default_store_keys_set,
        },
    };
    let (tx, wake) = (q.tx.clone(), st.wake.clone());
    spawn("rcfd-studio-quick-start", move || start_job(job, tx, wake));
    Ok(())
}

struct Job {
    gen: u64,
    root: PathBuf,
    case: String,
    search: Search,
    tail_always: bool,
    /// [`Switches::cases_as_toml`] and [`Switches::default_store_keys_set`].
    route: PlanRoute,
}

/// The last lines of `.rcfd/runner.log` (the runner has no console; since ADR 0146 its standard output and error go
/// there too), for a failed runner start.
fn runner_log_tail(root: &Path) -> String {
    let p = rcfd_runner::runner::dot_dir(root).join("runner.log");
    match crate::poll::tail_lines(&p, 3) {
        Ok((_, lines)) if !lines.is_empty() => format!("; last lines of {}: {}", p.display(), lines.join(" | ")),
        _ => String::new(),
    }
}

/// True when an error of `Client::ensure_runner` already reports the runner's output: ADR 0146's early-exit report
/// (`the runner (pid N) exited with ... before it held the project's lock; ` followed by its output's end, or that it
/// wrote nothing, or where its output went).
pub fn reports_runner_output(e: &str) -> bool {
    e.contains(" before it held the project's lock; ")
}

/// The text of a failed runner start: `ensure_runner`'s error with the last lines of `runner.log` appended, unless the
/// error already reports the runner's output (an early exit, ADR 0146), so that those lines show once (ADR 0147). With
/// `always` (the route before), they are appended in every case.
pub fn runner_error(e: &str, root: &Path, always: bool) -> String {
    if !always && reports_runner_output(e) {
        return e.to_string();
    }
    format!("{e}{}", runner_log_tail(root))
}

fn start_job(job: Job, tx: Sender<Msg>, wake: Wake) {
    let send = |m: Msg| {
        if tx.send(m).is_ok() {
            wake();
        }
    };
    let gen = job.gen;
    let stage = |t: &str| send(Msg::Stage { gen, text: t.into() });
    let fail = |text: String| send(Msg::Failed { gen, text });
    let found = job.search.find();
    send(Msg::Located(found.clone()));
    let found = match found {
        Ok(f) => f,
        Err(e) => return fail(format!("{} was not started: {e}", job.case)),
    };
    stage("checking the solver");
    if let Err(e) = bins::check_solver(&found.solver) {
        return fail(format!("{} was not started: {e}", job.case));
    }
    let client = Client::new(&job.root, &found.runner);
    if !client.runner_running() {
        // `rcfd-runner serve` refuses a bad budgets file and exits; `ensure_runner` reports that with its output (ADR
        // 0146), but only after the run was queued: checked here, nothing is queued
        if let Err(e) = Config::load(&job.root) {
            return fail(format!(
                "{} was not started: the runner cannot start with its budgets file: {e} (fix the file or use Runs > \
                 Budgets...)",
                job.case
            ));
        }
    }
    stage("reading the case");
    let plan = job
        .root
        .join(&job.case)
        .canonicalize()
        .map_err(|e| format!("{}: {e}", job.case))
        .and_then(|path| {
            let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            StartPlan::new_route(&job.case, &text, path.parent().unwrap_or(&job.root), job.route)
        });
    let plan = match plan {
        Ok(p) => p,
        Err(e) => return fail(format!("{} was not started: {e}", job.case)),
    };
    stage("queueing the run");
    let id = match client.submit(&job.root.join(&plan.case), &plan.settings()) {
        Ok(id) => id,
        Err(e) => return fail(format!("{} was not queued: {e}", job.case)),
    };
    send(Msg::Queued {
        gen,
        id,
        plan: Box::new(plan),
    });
    let result = client
        .ensure_runner()
        .map_err(|e| runner_error(&e, &job.root, job.tail_always));
    send(Msg::Runner { gen, result });
}

/// Make `p` the shown tab of its place and give that place the focus (shown first when it was hidden).
pub fn bring_to_front(dock: &mut DockState<Panel>, p: Panel) {
    if !layout::is_open(dock, p) {
        layout::show(dock, p);
    }
    if let Some(path) = dock.find_tab(&p) {
        let _ = dock.set_active_tab(path);
        dock.set_focused_node_and_surface(path.node_path());
    }
}

fn place_of(dock: &DockState<Panel>, p: Panel) -> Option<NodePath> {
    dock.find_tab(&p).map(|t| t.node_path())
}

/// Open a closed Viewport in a new place above the Plots' place, as the default layout has it (the Plots keep 0.34 of
/// the height, on their surface, docked or floating); without Plots, where `layout::show` puts a panel. ADR 0141
/// amendment 1: `layout::show` uses the focused place, which is often the Plots' own.
pub fn reopen_viewport(dock: &mut DockState<Panel>) {
    if layout::is_open(dock, Panel::Viewport) {
        return;
    }
    match dock.find_tab(&Panel::Plots) {
        Some(p) => {
            let [_, v] = dock[p.surface].split_above(p.node, 0.34, vec![Panel::Viewport]);
            dock.set_focused_node_and_surface(NodePath {
                surface: p.surface,
                node: v,
            });
        }
        None => layout::show(dock, Panel::Viewport),
    }
}

/// Show `p` in front of its place when it is open and does not share the Viewport's place ("as the layout allows":
/// a hidden panel stays hidden and the viewport stays visible); the focus goes back to the Viewport.
fn front_beside_viewport(dock: &mut DockState<Panel>, p: Panel) {
    let Some(path) = dock.find_tab(&p) else { return };
    if place_of(dock, Panel::Viewport) == Some(path.node_path()) {
        return;
    }
    let _ = dock.set_active_tab(path);
    if let Some(v) = place_of(dock, Panel::Viewport) {
        dock.set_focused_node_and_surface(v);
    }
}

/// Apply what the control threads reported and keep the selected run's viewport text honest; the number of reports.
/// Called at the end of [`Studio::drain`]; never blocks.
pub fn drain(st: &mut Studio) -> usize {
    let msgs: Vec<Msg> = st.quick.rx.try_iter().collect();
    let n = msgs.len();
    for m in msgs {
        apply(st, m);
    }
    follow_selected(st);
    n
}

fn apply(st: &mut Studio, m: Msg) {
    let current = st.quick.gen;
    match m {
        Msg::Located(r) => {
            st.quick.locating = false;
            if let Ok(f) = &r {
                // the run worker of a project opened from now on uses it (ADR 0133 amendment 1 item 7)
                st.runner_exe = f.runner.clone();
            }
            st.quick.bins = Some(r);
        }
        Msg::Stage { gen, text } if gen == current => {
            if let Some(s) = &mut st.quick.starting {
                s.stage = text;
            }
        }
        Msg::Queued { gen, id, plan } if gen == current => {
            let case = plan.case.clone();
            st.say(format!(
                "queued run {id} of {case} (rcfd run --set {})",
                plan.set().join(" ")
            ));
            if let Some(s) = &mut st.quick.starting {
                s.stage = format!("queued run {id}; starting the runner");
            }
            st.quick.run = Some((id.clone(), *plan));
            // ADR 0148: another case selected after the click keeps the Viewport on its mesh. ADR 0149: the run is not
            // selected either, so that the selected run stays one of the selected case (the Run side, the strip and the
            // plots agree with the Project panel); it is listed in the Runs panel and named in the status bar. With
            // `case_run_unlinked`, ADR 0148's rule: the run is selected without a switch of the Viewport.
            let moved_on =
                !st.quick.switches.selection_from_editor && st.selected_case.as_deref() != Some(case.as_str());
            if moved_on {
                if st.quick.switches.case_run_unlinked && !st.quick.switches.no_select {
                    st.select_run_keep_view(&id);
                }
                return;
            }
            if !st.quick.switches.no_select {
                st.select_run(&id);
            }
            if !st.quick.switches.no_focus {
                if !st.quick.switches.reopen_in_focused_leaf {
                    reopen_viewport(&mut st.dock);
                }
                bring_to_front(&mut st.dock, Panel::Viewport);
                front_beside_viewport(&mut st.dock, Panel::Plots);
            }
        }
        Msg::Failed { gen, text } if gen == current => {
            st.quick.starting = None;
            st.quick.error = Some(text.clone());
            st.say(text);
        }
        Msg::Runner { gen, result } if gen == current => {
            st.quick.starting = None;
            match result {
                Ok(()) => st.quick.runner_error = None,
                Err(e) => {
                    let t = format!(
                        "the runner did not start: {e}. The run stays queued until a runner starts (Runs > Start \
                         runner)"
                    );
                    st.say(t.clone());
                    st.quick.runner_error = Some(t);
                }
            }
        }
        Msg::Probe { gen, id, snapshots } if gen == current => explain_end(st, &id, snapshots),
        _ => {}
    }
}

/// The selected run before its first shown snapshot: a waiting text while it is queued or running, and, once it
/// ended, a check of its store on a control thread ([`explain_end`] uses the answer). A run started by the Run button
/// that failed so brings the Log panel (its log tail and events) to the front, once.
fn follow_selected(st: &mut Studio) {
    let Some(id) = st.viewport.run.clone() else { return };
    if st.selected_run.as_deref() != Some(id.as_str()) {
        return;
    }
    if st.viewport.frame.is_some() || !st.viewport.snaps.is_empty() {
        return; // a snapshot is shown or on its way
    }
    let Some(row) = st.runs.row(&id).cloned() else { return };
    if row.state == RunState::Failed && st.quick.plan_of(&id).is_some() && st.quick.failed_seen.insert(id.clone()) {
        st.say(format!(
            "run {id} failed: {}",
            row.reason.as_deref().unwrap_or("no reason recorded")
        ));
        if !st.quick.switches.no_focus {
            front_beside_viewport(&mut st.dock, Panel::Log);
        }
    }
    if st.quick.switches.no_explain {
        return;
    }
    if row.state.is_terminal() {
        let Some(root) = st.browser().map(|b| b.root.clone()) else {
            return;
        };
        if st.quick.probed.insert(id.clone()) {
            let (tx, wake, gen) = (st.quick.tx.clone(), st.wake.clone(), st.quick.gen);
            let dir = root.join("runs").join(&id).join("fields");
            spawn("rcfd-studio-store-probe", move || {
                let snapshots = if dir.join(rcfd_tsstore::STORE_FILE).is_file() {
                    rcfd_tsstore::Reader::open(&dir)
                        .map(|r| r.snapshots().len())
                        .map_err(|e| format!("{}: {e}", dir.display()))
                } else {
                    Ok(0)
                };
                if tx.send(Msg::Probe { gen, id, snapshots }).is_ok() {
                    wake();
                }
            });
        } else if !st.quick.switches.no_reexplain {
            // ADR 0144: the store was opened again (the Mesh / Run toggle): the same explanation again
            if let Some((_, text)) = st.quick.notice.clone().filter(|(nid, _)| *nid == id) {
                let status = notice_status(&row, text);
                if st.viewport.status != status {
                    st.viewport.status = status;
                }
            }
        }
    } else if matches!(st.viewport.status, Status::NoStore(_)) {
        let hint = Status::NoStore(waiting_text(st, &id, &row));
        if st.viewport.status != hint {
            st.viewport.status = hint;
        }
    }
}

fn store_text(plan: &StartPlan) -> String {
    match plan.store.as_ref().filter(|s| s.on) {
        Some(s) => match s.every {
            StoreEvery::Steps(k) => format!("its store writes a snapshot every {k} steps"),
            StoreEvery::Time(dt) => format!("its store writes a snapshot every {dt} s of simulated time"),
        },
        None => format!(
            "this run stores no fields{}, so there is no picture to show; the plots follow it",
            plan.note.as_ref().map_or(String::new(), |n| format!(" ({n})"))
        ),
    }
}

/// The viewport's text while the selected run has no stored snapshot yet.
pub fn waiting_text(st: &Studio, id: &str, row: &RunRow) -> String {
    let store = st.quick.plan_of(id).map_or_else(
        || "its first stored snapshot appears here when it is written".into(),
        store_text,
    );
    match row.state {
        RunState::Queued => format!(
            "Run {id} is {}{}; {store}.",
            row.state_label(),
            row.queue_pos.map_or(String::new(), |q| format!(" (#{q} in the queue)"))
        ),
        RunState::Paused => format!(
            "Run {id} is paused at step {} before its first stored snapshot; {store}.",
            row.step
        ),
        _ => format!(
            "Run {id} is running (step {}), no stored snapshot yet; {store}.",
            row.step
        ),
    }
}

/// The last non-empty line of the shown run's log, when `id` is the shown run.
fn last_log_line<'a>(st: &'a Studio, id: &str) -> Option<&'a str> {
    if st.runs.shown.as_deref() != Some(id) {
        return None;
    }
    st.runs.log.iter().rev().map(|l| l.trim()).find(|l| !l.is_empty())
}

/// A field-store warning among the shown run's events (`field store disabled: ...`).
fn store_warning(st: &Studio, id: &str) -> Option<String> {
    if st.runs.shown.as_deref() != Some(id) {
        return None;
    }
    st.runs.events.iter().rev().find_map(|e| match &e.kind {
        EventKind::Warning { message } if message.contains("field store") => Some(message.clone()),
        EventKind::Other { body, .. } => {
            let m = body["solver"]["message"]
                .as_str()
                .or_else(|| body["message"].as_str())?;
            m.contains("field store").then(|| m.to_string())
        }
        _ => None,
    })
}

/// The message of a selected run that ended before its first stored snapshot.
pub fn end_text(st: &Studio, id: &str, row: &RunRow, store_error: Option<&str>) -> String {
    let reason = row.reason.as_deref().unwrap_or("no reason recorded");
    let mut t = match row.state {
        RunState::Failed => format!("Run {id} failed before writing its first stored snapshot: {reason}"),
        RunState::Cancelled => format!("Run {id} was cancelled before writing its first stored snapshot: {reason}"),
        _ => format!(
            "Run {id} finished without stored fields{}",
            match (store_warning(st, id), st.quick.plan_of(id)) {
                (Some(w), _) => format!(" ({w})"),
                (None, Some(p)) => format!(" ({})", store_text(p)),
                (None, None) => {
                    " (its settings set no output.store_interval or output.store_time_interval; start it with \
                     \"Store fields\" on to scrub it)"
                        .to_string()
                }
            }
        ),
    };
    if let Some(e) = store_error {
        t.push_str(&format!(". Its store could not be read: {e}"));
    }
    if row.state != RunState::Finished {
        if let Some(l) = last_log_line(st, id) {
            t.push_str(&format!(". Last line of its log: {l}"));
        }
    }
    t.push('.');
    t
}

fn explain_end(st: &mut Studio, id: &str, snapshots: Result<usize, String>) {
    if st.viewport.run.as_deref() != Some(id) || st.viewport.frame.is_some() || !st.viewport.snaps.is_empty() {
        return;
    }
    if matches!(snapshots, Ok(n) if n > 0) {
        return; // the viewport opens the store on its next poll
    }
    let Some(row) = st.runs.row(id).cloned() else { return };
    let text = end_text(st, id, &row, snapshots.as_ref().err().map(String::as_str));
    st.viewport.status = notice_status(&row, text.clone());
    st.quick.notice = Some((id.to_string(), text));
}

/// How the viewport shows the explanation of a run that ended before its first stored snapshot.
fn notice_status(row: &RunRow, text: String) -> Status {
    if row.state == RunState::Finished {
        Status::NoStore(text)
    } else {
        Status::Failed(text)
    }
}

// ---------------------------------------------------------------- the status strip

/// How a strip line is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Plain.
    Normal,
    /// Secondary.
    Weak,
    /// A warning (a cancelled run, a run that waits).
    Warn,
    /// An error.
    Error,
}

/// One line of the strip.
#[derive(Debug, Clone, PartialEq)]
pub struct StripLine {
    /// Text.
    pub text: String,
    /// Tone.
    pub tone: Tone,
}

fn line(tone: Tone, text: impl Into<String>) -> StripLine {
    StripLine {
        text: text.into(),
        tone,
    }
}

/// A run control of the strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// Hold the run.
    Pause,
    /// Release it.
    Resume,
    /// Cancel it.
    Cancel,
}

impl Control {
    /// The button label.
    pub fn label(self) -> &'static str {
        match self {
            Control::Pause => "Pause",
            Control::Resume => "Resume",
            Control::Cancel => "Cancel",
        }
    }
}

/// What the strip shows: the selected run's line, the start in flight and its errors, and the controls with whether
/// each is enabled.
#[derive(Debug, Clone, PartialEq)]
pub struct StripView {
    /// The selected run.
    pub run: Option<String>,
    /// Lines, top first.
    pub lines: Vec<StripLine>,
    /// Pause, Resume, Cancel and whether each is enabled.
    pub controls: [(Control, bool); 3],
}

impl StripView {
    /// Every line, joined by newlines (tests).
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Whether control `c` is enabled.
    pub fn enabled(&self, c: Control) -> bool {
        self.controls.iter().any(|(x, on)| *x == c && *on)
    }
}

fn res(r: &RunRow, k: &str) -> String {
    r.last_residuals.get(k).map_or("-".into(), |v| format!("{v:.2e}"))
}

/// The run line: id, case, state, step and time in words (ADR 0148: `time step N  simulated time T s` of a transient
/// run, `iteration N` of a steady one), last residuals and the viewport's snapshot.
pub fn run_line(st: &Studio, r: &RunRow) -> String {
    let when = if st.viewport.switches.bare_time {
        format!("step {}  t {:.4e}", r.step, r.time)
    } else {
        crate::viewport::step_time_text(
            crate::viewport::run_kind_of_time(r.time),
            &r.step.to_string(),
            r.time,
            false,
        )
    };
    let mut s = format!(
        "run {}  {}  {}  {when}  res u {}  p {}  continuity {}",
        r.id,
        r.case,
        r.state_label(),
        res(r, "res_u"),
        res(r, "res_p"),
        res(r, "continuity")
    );
    if let Some(q) = r.queue_pos {
        s.push_str(&format!("  #{q} in the queue"));
    }
    let vp = &st.viewport;
    if vp.run.as_deref() == Some(r.id.as_str()) {
        match &vp.frame {
            Some(f) => s.push_str(&format!(
                "  snapshot {}/{}{}",
                f.index + 1,
                vp.snaps.len(),
                if vp.follow { " (following the latest)" } else { "" }
            )),
            None if vp.snaps.is_empty() => s.push_str("  no stored snapshot yet"),
            None => {}
        }
    }
    s
}

/// The strip of the current state.
pub fn strip_view(st: &Studio) -> StripView {
    let mut lines = Vec::new();
    let run = st.selected_run.clone();
    let row = run.as_deref().and_then(|id| st.runs.row(id));
    match (&run, row) {
        (Some(_), Some(r)) => {
            lines.push(line(Tone::Normal, run_line(st, r)));
            if !st.quick.switches.no_explain && matches!(r.state, RunState::Failed | RunState::Cancelled) {
                let tone = if r.state == RunState::Failed {
                    Tone::Error
                } else {
                    Tone::Warn
                };
                lines.push(line(
                    tone,
                    format!(
                        "{}: {}",
                        r.state_label(),
                        r.reason.as_deref().unwrap_or("no reason recorded")
                    ),
                ));
                if r.state == RunState::Failed {
                    if let Some(l) = last_log_line(st, &r.id) {
                        lines.push(line(Tone::Weak, format!("last line of its log: {l}")));
                    }
                }
            }
        }
        (Some(id), None) => lines.push(line(Tone::Weak, format!("run {id}: waiting for the run list"))),
        (None, None) | (None, Some(_)) => {}
    }
    if let Some(s) = &st.quick.starting {
        lines.push(line(
            Tone::Weak,
            format!("Run {}: {} ({:.1} s)", s.case, s.stage, s.since.elapsed().as_secs_f64()),
        ));
    }
    if let Some(e) = &st.quick.error {
        lines.push(line(Tone::Error, e.clone()));
    }
    if let Some(e) = &st.quick.runner_error {
        lines.push(line(Tone::Error, e.clone()));
    }
    if lines.is_empty() {
        lines.push(line(
            Tone::Weak,
            "No run selected: select a case in the Project panel and press Run (F5), or select a run in the Runs \
             panel.",
        ));
    }
    let live = row.is_some_and(|r| !r.state.is_terminal());
    let paused = row.is_some_and(|r| r.held || r.state == RunState::Paused);
    StripView {
        run,
        lines,
        controls: [
            (Control::Pause, live && !paused),
            (Control::Resume, live && paused),
            (Control::Cancel, live),
        ],
    }
}

/// A control of the strip on the selected run (the run worker's request files, ADR 0133).
pub fn control(st: &mut Studio, c: Control) -> Result<(), String> {
    let id = st.selected_run.clone().ok_or("no run is selected")?;
    match c {
        Control::Pause => st.pause_run(&id),
        Control::Resume => st.resume_run(&id),
        Control::Cancel => st.cancel_run(&id),
    }
}
