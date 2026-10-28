//! The Studio's state and decisions, without drawing (ADR 0131 Design). Every method is callable from tests; the UI calls
//! the same methods. File work is sent to the project worker ([`crate::watch`]); [`Studio::drain`] applies its results.

use crate::case_edit::{schema_tree, CaseEditor, EditorState, External};
use crate::layout::{self, Panel};
use crate::monitors::MonitorStore;
use crate::poll::{self, RunWorker, Switches, Update};
use crate::prefs::Prefs;
use crate::project_view::Browser;
use crate::runs::{RunsView, StartPlan};
use crate::schema_model::Node;
use crate::theme::Theme;
use crate::watch::{Command, Event, ProjectWorker, Start, Wake, POLL};
use egui_dock::DockState;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_runner::runner::Config;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Messages kept for the log panel.
pub const MAX_MESSAGES: usize = 200;

/// One open (or opening) project.
pub struct OpenProject {
    worker: ProjectWorker,
    /// The folder as requested.
    pub requested: PathBuf,
    /// The lists, once the worker opened the project.
    pub browser: Option<Browser>,
    /// Paths changed on disk, with the time the model received them, latest last (at most 50; paths under `runs/` are
    /// left out, the run panels show them).
    pub changes: Vec<(Instant, String)>,
    /// The run worker (ADR 0133 D2), started once the project is open.
    runs: Option<RunWorker>,
}

/// The application model.
pub struct Studio {
    /// In-process solver service used by the case editor (validation only; the Studio never runs the solver in process).
    pub svc: Arc<Service>,
    /// Settings schema tree.
    pub schema: Node,
    /// Preferences (theme, layout text, recent projects).
    pub prefs: Prefs,
    /// Where preferences are saved (`None`: not saved, as in tests).
    pub prefs_dir: Option<PathBuf>,
    /// Panel layout.
    pub dock: DockState<Panel>,
    /// Theme.
    pub theme: Theme,
    /// The open project.
    pub project: Option<OpenProject>,
    /// The case file open in the editor.
    pub editor: Option<CaseEditor>,
    /// A case file asked for and not loaded yet.
    pub pending_case: Option<String>,
    /// Selected run id.
    pub selected_run: Option<String>,
    /// Residual series of the selected run (`res_u`, `res_p`, `continuity`; ADR 0133 D3).
    pub plots: MonitorStore,
    /// Monitor (report) series of the selected run, by report name.
    pub monitors: MonitorStore,
    /// The run worker's view of the open project's runs.
    pub runs: RunsView,
    /// The viewport's current step, marked in the plots (set by the viewport, ADR 0132).
    pub plot_marker: Option<f64>,
    /// The viewport of the selected run (ADR 0132; its GPU is attached by the app, none in `new`).
    pub viewport: crate::viewport::Viewport,
    /// Status and error messages, latest last.
    pub messages: Vec<String>,
    /// Watcher period for projects opened from now on (`None`: watcher off).
    pub poll: Option<Duration>,
    /// Run worker period for projects opened from now on (`None`: no run worker).
    pub run_poll: Option<Duration>,
    /// Negative-control switches of the run worker (tests only; all off in the app).
    pub run_switches: Switches,
    /// The runner program the run worker starts (`RCFD_RUNNER_BIN`, else `rcfd-runner` next to this program).
    pub runner_exe: PathBuf,
    shown_run: Option<String>,
    wake: Wake,
}

/// The runner program: `RCFD_RUNNER_BIN`, else `rcfd-runner` next to the current executable (the rule of
/// `rcfd_runner::client::Client::with_default_runner`).
pub fn default_runner_exe() -> PathBuf {
    std::env::var_os("RCFD_RUNNER_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| rcfd_runner::runner::sibling_exe("rcfd-runner"))
}

impl Studio {
    /// A model with the given preferences and layout. `prefs_dir` is where changes are saved.
    pub fn new(prefs: Prefs, dock: DockState<Panel>, prefs_dir: Option<PathBuf>) -> Studio {
        let svc = Service::new(ServiceConfig::default());
        let schema = schema_tree(&svc);
        Studio {
            svc,
            schema,
            theme: prefs.theme,
            prefs,
            prefs_dir,
            dock,
            project: None,
            editor: None,
            pending_case: None,
            selected_run: None,
            plots: MonitorStore::default(),
            monitors: MonitorStore::default(),
            runs: RunsView::default(),
            plot_marker: None,
            viewport: Default::default(),
            messages: Vec::new(),
            poll: Some(POLL),
            run_poll: Some(poll::PERIOD),
            run_switches: Switches::default(),
            runner_exe: default_runner_exe(),
            shown_run: None,
            wake: Arc::new(|| {}),
        }
    }

    /// A model with default preferences and layout that saves nothing.
    pub fn in_memory() -> Studio {
        Studio::new(Prefs::default(), layout::default_layout(), None)
    }

    /// What the worker calls after sending (the UI passes a repaint request).
    pub fn set_wake(&mut self, wake: Wake) {
        self.wake = wake;
    }

    /// Add a message (shown in the status bar and the log panel).
    pub fn say(&mut self, m: impl Into<String>) {
        self.messages.push(m.into());
        if self.messages.len() > MAX_MESSAGES {
            self.messages.remove(0);
        }
    }

    /// The latest message.
    pub fn last_message(&self) -> Option<&str> {
        self.messages.last().map(String::as_str)
    }

    /// The open project's lists, once open.
    pub fn browser(&self) -> Option<&Browser> {
        self.project.as_ref().and_then(|p| p.browser.as_ref())
    }

    fn start(&mut self, start: Start, requested: PathBuf) {
        self.close_project();
        let worker = ProjectWorker::start(start, self.poll, self.wake.clone());
        self.project = Some(OpenProject {
            worker,
            requested,
            browser: None,
            changes: Vec::new(),
            runs: None,
        });
    }

    /// Open a project folder (on the worker; the result arrives through [`Studio::drain`]).
    pub fn open_project(&mut self, root: &Path) {
        self.start(Start::Open(root.to_path_buf()), root.to_path_buf());
    }

    /// Create a project folder and open it.
    pub fn create_project(&mut self, root: &Path, name: &str, solver_version: &str) {
        self.start(
            Start::Create {
                root: root.to_path_buf(),
                name: name.to_string(),
                solver_version: solver_version.to_string(),
            },
            root.to_path_buf(),
        );
    }

    /// Close the project (and the case open in the editor; unsaved edits are dropped).
    pub fn close_project(&mut self) {
        self.editor = None;
        self.pending_case = None;
        self.selected_run = None;
        self.project = None;
        self.runs = RunsView::default();
        self.plots.clear();
        self.monitors.clear();
        self.shown_run = None;
        self.viewport.close();
        self.plot_marker = None;
    }

    /// Ask the worker for a case file; it opens in the editor when it arrives.
    pub fn open_case(&mut self, rel: &str) {
        let Some(p) = &self.project else { return };
        if let Some(old) = &self.editor {
            p.worker.send(Command::Untrack(old.rel.clone()));
        }
        self.pending_case = Some(rel.to_string());
        p.worker.send(Command::Load(rel.to_string()));
    }

    /// Hand the editor's content to the worker for a save with the base-hash rule. Refused in the conflict state (resolve
    /// it first), while a save is in flight, and when there is nothing to save.
    pub fn save_case(&mut self) -> Result<(), String> {
        let (Some(p), Some(ed)) = (&self.project, &mut self.editor) else {
            return Err("no case is open".into());
        };
        if ed.in_conflict() {
            return Err(format!("{}: resolve the conflict first", ed.rel));
        }
        if ed.saving {
            return Err(format!("{}: a save is in progress", ed.rel));
        }
        if !ed.is_dirty() {
            return Err(format!("{}: nothing to save", ed.rel));
        }
        let bytes = ed.to_bytes()?;
        ed.saving = true;
        p.worker.send(Command::Save {
            rel: ed.rel.clone(),
            base: ed.base_hash.clone(),
            bytes,
        });
        Ok(())
    }

    /// Re-read the browser lists.
    pub fn refresh(&self) {
        if let Some(p) = &self.project {
            p.worker.send(Command::Refresh);
        }
    }

    /// Apply everything the workers sent. Returns the number of events and updates applied. Never blocks.
    pub fn drain(&mut self) -> usize {
        let Some(p) = &self.project else { return 0 };
        let events = p.worker.drain();
        let mut n = events.len();
        for e in events {
            self.apply(e);
        }
        self.sync_shown();
        let updates = self
            .project
            .as_ref()
            .and_then(|p| p.runs.as_ref())
            .map(RunWorker::drain)
            .unwrap_or_default();
        n += updates.len();
        let mut rows_changed = false;
        for u in updates {
            rows_changed |= self.apply_update(u);
        }
        if rows_changed || n > 0 {
            self.update_extra();
        }
        // the viewport follows the selected run (ADR 0132) and the plots mark its step
        let root = self.browser().map(|b| b.root.clone());
        let selected = self.selected_run.clone();
        self.viewport
            .sync(root.as_deref(), selected.as_deref(), poll::PERIOD, &self.wake);
        n += self.viewport.drain();
        self.plot_marker = self.viewport.marker();
        n
    }

    fn run_worker(&self) -> Option<&RunWorker> {
        self.project.as_ref().and_then(|p| p.runs.as_ref())
    }

    fn send_runs(&mut self, c: poll::Command) -> Result<(), String> {
        match self.run_worker() {
            Some(w) => {
                w.send(c);
                Ok(())
            }
            None => Err("no project is open (or it is still opening)".into()),
        }
    }

    /// The run whose series, events and log the worker reads follows [`Studio::selected_run`]; a new selection
    /// clears the plots.
    fn sync_shown(&mut self) {
        if self.shown_run == self.selected_run || self.run_worker().is_none() {
            return;
        }
        self.shown_run = self.selected_run.clone();
        self.plots.clear();
        self.monitors.clear();
        self.runs.events.clear();
        self.runs.log.clear();
        self.runs.metrics = None;
        self.runs.warnings.clear();
        self.runs.shown = self.shown_run.clone();
        let ids: Vec<String> = self.shown_run.iter().cloned().collect();
        let _ = self.send_runs(poll::Command::Show(ids));
    }

    /// Select a run (its plots, events and log are read from now on).
    pub fn select_run(&mut self, id: &str) {
        self.selected_run = Some(id.to_string());
        self.sync_shown();
    }

    /// Queue a run of the plan's case and make sure a runner is alive (`Client::start`, on a control thread). The id
    /// arrives as a message.
    pub fn start_run(&mut self, plan: StartPlan) -> Result<(), String> {
        self.send_runs(poll::Command::Start(plan))
    }

    /// Queue a run without starting a runner (`Client::submit`).
    pub fn queue_run(&mut self, plan: StartPlan) -> Result<(), String> {
        self.send_runs(poll::Command::Queue(plan))
    }

    /// Start the project's runner (`Client::ensure_runner`, on a control thread; not retried on failure).
    pub fn start_runner(&mut self) -> Result<(), String> {
        if self.runs.starting_runner {
            return Err("the runner is being started".into());
        }
        self.send_runs(poll::Command::EnsureRunner)?;
        self.runs.starting_runner = true;
        self.runs.runner_error = None;
        Ok(())
    }

    /// Hold a run.
    pub fn pause_run(&mut self, id: &str) -> Result<(), String> {
        self.send_runs(poll::Command::Pause(id.to_string()))
    }

    /// Release a held run.
    pub fn resume_run(&mut self, id: &str) -> Result<(), String> {
        self.send_runs(poll::Command::Resume(id.to_string()))
    }

    /// Cancel a run.
    pub fn cancel_run(&mut self, id: &str) -> Result<(), String> {
        self.send_runs(poll::Command::Cancel(id.to_string()))
    }

    /// Ask for the start plan of a case (it arrives in [`RunsView::plan`]).
    pub fn plan_start(&mut self, rel: &str) -> Result<(), String> {
        self.runs.plan = None;
        self.send_runs(poll::Command::Plan(rel.to_string()))
    }

    /// Re-read the runner budgets.
    pub fn load_budgets(&mut self) -> Result<(), String> {
        self.send_runs(poll::Command::LoadConfig)
    }

    /// Write the runner budgets (checked first; the runner reads them when it starts).
    pub fn save_budgets(&mut self, c: Config) -> Result<(), String> {
        crate::runs::check_config(&c)?;
        self.send_runs(poll::Command::SaveConfig(c))
    }

    /// Recompute [`RunsView::extra`]: runs the worker knows that the browser does not list yet.
    fn update_extra(&mut self) {
        let listed: BTreeSet<&str> = self
            .browser()
            .map(|b| b.runs.iter().map(|r| r.id.as_str()).collect())
            .unwrap_or_default();
        let mut extra: Vec<&crate::runs::RunRow> = self
            .runs
            .rows
            .values()
            .filter(|r| !listed.contains(r.id.as_str()))
            .collect();
        extra.sort_by(|a, b| (&a.created, &a.id).cmp(&(&b.created, &b.id)));
        self.runs.extra = extra.into_iter().map(|r| r.id.clone()).collect();
    }

    /// Apply one update of the run worker; true when the rows changed.
    fn apply_update(&mut self, u: Update) -> bool {
        let shown = |id: &str, s: &Option<String>| s.as_deref() == Some(id);
        match u {
            Update::Tick(t) => {
                self.runs.last_tick = Some((t.n, t.started));
                self.runs.runner_running = Some(t.runner_running);
                self.runs.status_errors = t.status_errors;
                if let Some(rows) = t.rows {
                    self.runs.rows = rows.into_iter().map(|r| (r.id.clone(), r)).collect();
                    return true;
                }
            }
            Update::Series {
                id,
                reset,
                residuals,
                monitors,
            } => {
                if !shown(&id, &self.shown_run) {
                    return false;
                }
                if reset {
                    self.plots.clear();
                    self.monitors.clear();
                }
                for (name, (x, y)) in residuals {
                    for (a, b) in x.into_iter().zip(y) {
                        self.plots.push(&name, a, b);
                    }
                }
                for (name, (x, y)) in monitors {
                    for (a, b) in x.into_iter().zip(y) {
                        self.monitors.push(&name, a, b);
                    }
                }
            }
            Update::Events { id, reset, items } => {
                if shown(&id, &self.shown_run) {
                    if reset {
                        self.runs.events.clear();
                    }
                    self.runs.events.extend(items);
                }
            }
            Update::Log { id, lines } => {
                if shown(&id, &self.shown_run) {
                    self.runs.log = lines;
                }
            }
            Update::Metrics { id, value } => {
                if shown(&id, &self.shown_run) {
                    self.runs.metrics = Some(value);
                }
            }
            Update::Warning { id, text } => {
                if shown(&id, &self.shown_run) {
                    self.runs.warnings.push(text.clone());
                }
                self.say(text);
            }
            Update::Config(c) => self.runs.config = Some(c),
            Update::Plan(p) => self.runs.plan = Some(p),
            Update::Started(r) => self.say(match r {
                Ok(id) => format!("queued run {id}"),
                Err(e) => format!("the run was not started: {e}"),
            }),
            Update::RunnerStarted(r) => {
                self.runs.starting_runner = false;
                match r {
                    Ok(()) => self.say("the runner is running"),
                    Err(e) => {
                        self.say(format!("the runner did not start: {e}"));
                        self.runs.runner_error = Some(e);
                    }
                }
            }
            Update::Message(m) => self.say(m),
        }
        false
    }

    /// Drain until `done` holds or `timeout` passes (tests and batch use; never from the UI thread).
    pub fn drain_until(&mut self, timeout: Duration, mut done: impl FnMut(&Studio) -> bool) -> bool {
        let end = Instant::now() + timeout;
        loop {
            self.drain();
            if done(self) {
                return true;
            }
            if Instant::now() >= end {
                return false;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn apply(&mut self, e: Event) {
        match e {
            Event::Opened(b) => {
                if let Some(n) = b.notice() {
                    self.say(format!("{}: {n}", b.root.display()));
                } else {
                    self.say(format!("opened {}", b.root.display()));
                }
                self.prefs.remember(&b.root);
                self.save_prefs();
                let runs = self.run_poll.map(|period| {
                    let w = RunWorker::start(&b.root, &self.runner_exe, period, self.run_switches, self.wake.clone());
                    w.send(poll::Command::LoadConfig);
                    w
                });
                self.shown_run = None;
                if let Some(p) = &mut self.project {
                    p.browser = Some(b);
                    p.runs = runs;
                }
            }
            Event::OpenFailed(m) => {
                self.say(m);
                self.project = None;
            }
            Event::Listing(b) => {
                if let Some(p) = &mut self.project {
                    p.browser = Some(b);
                }
            }
            Event::Changed(paths) => {
                if let Some(p) = &mut self.project {
                    let now = Instant::now();
                    // streaming runs change files under runs/ every poll; the run panels show those
                    p.changes
                        .extend(paths.into_iter().filter(|c| !c.starts_with("runs/")).map(|c| (now, c)));
                    let excess = p.changes.len().saturating_sub(50);
                    p.changes.drain(..excess);
                }
            }
            Event::FileChanged { rel, bytes } => {
                let Some(ed) = self.editor.as_mut().filter(|ed| ed.rel == rel) else {
                    return;
                };
                match ed.external_change(bytes) {
                    External::Same => {}
                    External::Reloaded => self.say(format!("{rel}: changed on disk, reloaded")),
                    External::Conflict => {
                        self.say(format!("{rel}: changed on disk while edited; resolve the conflict"))
                    }
                }
            }
            Event::Loaded { rel, bytes } => {
                if self.pending_case.as_deref() != Some(rel.as_str()) {
                    return;
                }
                self.pending_case = None;
                match CaseEditor::from_bytes(self.svc.clone(), &rel, &bytes) {
                    Ok(ed) => self.editor = Some(ed),
                    Err(m) => self.say(m),
                }
            }
            Event::Failed(m) => {
                self.pending_case = None;
                if let Some(ed) = &mut self.editor {
                    ed.saving = false;
                }
                self.say(m);
            }
            Event::Saved { rel, hash } => {
                if let Some(ed) = self.editor.as_mut().filter(|ed| ed.rel == rel) {
                    ed.saved(hash);
                }
                self.say(format!("saved {rel}"));
            }
            Event::SaveConflict { rel, disk } => {
                if let Some(ed) = self.editor.as_mut().filter(|ed| ed.rel == rel) {
                    ed.save_conflict(disk);
                }
                self.say(format!(
                    "{rel}: not saved, the file changed on disk; resolve the conflict"
                ));
            }
        }
    }

    /// Editor state for the status bar.
    pub fn editor_state(&self) -> Option<EditorState> {
        self.editor.as_ref().map(CaseEditor::state)
    }

    /// Preferences of the current state.
    pub fn capture_prefs(&self) -> Prefs {
        Prefs::capture(self.theme, &self.dock, &self.prefs.recent)
    }

    /// Save preferences in the background when a directory is set (the UI thread does no file I/O).
    pub fn save_prefs(&mut self) {
        self.prefs = self.capture_prefs();
        let Some(dir) = self.prefs_dir.clone() else { return };
        let prefs = self.prefs.clone();
        std::thread::spawn(move || {
            if let Err(e) = prefs.save(&dir) {
                eprintln!("rcfd-studio: could not save preferences: {e}");
            }
        });
    }

    /// Save preferences now, on the calling thread. Used when the window closes: a background thread could be cut off by
    /// the process exit, and tab moves are only captured here. One small file write at exit.
    pub fn save_prefs_now(&mut self) -> Result<(), String> {
        self.prefs = self.capture_prefs();
        match &self.prefs_dir {
            Some(dir) => self.prefs.save(dir),
            None => Ok(()),
        }
    }
}
