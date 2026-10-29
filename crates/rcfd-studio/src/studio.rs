//! The Studio's state and decisions, without drawing (ADR 0131 Design). Every method is callable from tests; the UI calls
//! the same methods. File work is sent to the project worker ([`crate::watch`]); [`Studio::drain`] applies its results.

use crate::case_edit::{schema_tree, CaseEditor, EditorState, External};
use crate::chat::ChatModel;
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
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Messages kept for the log panel.
pub const MAX_MESSAGES: usize = 200;

/// The most case editors the Studio keeps with their unsaved state while other cases are selected (ADR 0149, the
/// maintainer's answer to ADR 0148 question 2). Each holds the settings of one case and an in-process session of it
/// (tens of kB to a few hundred kB); a switch that would keep one more is refused with a message.
pub const MAX_PARKED: usize = 32;

/// What follows the close of the project once the question about unsaved edits is answered (ADR 0149).
#[derive(Debug, Clone, PartialEq)]
pub enum AfterClose {
    /// Nothing more (Project > Close project).
    Nothing,
    /// Open this project folder (Project > Open project, Recent, Open typed path).
    Open(PathBuf),
    /// Create a project folder, then open it (Project > New project).
    Create {
        /// Folder.
        root: PathBuf,
        /// Manifest name.
        name: String,
        /// Solver version recorded in the manifest.
        solver_version: String,
    },
    /// Close the window.
    Exit,
}

/// The question asked before the project closes while cases hold unsaved state (ADR 0149): save all, discard, or
/// cancel.
#[derive(Debug, Clone, PartialEq)]
pub struct CloseAsk {
    /// What follows the close.
    pub then: AfterClose,
    /// The cases with unsaved state when it was asked.
    pub cases: Vec<String>,
    /// The cases whose save "Save all and close" is waiting for.
    pub saving: BTreeSet<String>,
    /// Why "Save all and close" stopped (shown in the question; nothing closed).
    pub error: Option<String>,
}

/// The answers to [`CloseAsk`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseAnswer {
    /// Save every case with unsaved state, then close (a typed field is committed first).
    SaveAll,
    /// Close; the unsaved edits are dropped.
    Discard,
    /// Do not close.
    Cancel,
}

/// True when a case editor holds something not in its file (ADR 0149): an edit not saved, a field being typed, a
/// conflict, or a save in flight.
pub fn has_unsaved(ed: &CaseEditor) -> bool {
    ed.is_dirty() || ed.draft.is_some() || ed.in_conflict() || ed.saving
}

/// Why the Run side has nothing to show while `case` is selected and has no run (ADR 0149: the Run side shows the
/// selected case's runs).
pub fn no_run_of(case: &str) -> String {
    format!("{case} has no run yet: press Run (F5) to start one, or select a run in the Runs panel.")
}

/// One open (or opening) project.
pub struct OpenProject {
    pub(crate) worker: ProjectWorker,
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
    /// The selected case (project-relative): set by a click in the Project panel ([`Studio::open_case`]), by a click on a
    /// run of it in the Runs panel ([`Studio::open_run`], ADR 0149) or by a case the case starter made and opened. Run,
    /// F5, the toolbar label, the Project panel's highlight, the Mesh side and the case editor follow it (ADR 0148), and
    /// the selected run is one of its runs (ADR 0149); a run uses its file on disk.
    pub selected_case: Option<String>,
    /// The case file open in the editor: the selected case once loaded, else none (ADR 0148).
    pub editor: Option<CaseEditor>,
    /// The selected case when the case editor could not open it, and why (ADR 0148): the editor shows this for that
    /// case, while the Mesh side and Run use its file on disk.
    pub editor_error: Option<(String, String)>,
    /// Case editors kept with their unsaved state (an edit not saved, a field being typed, a conflict, a save in
    /// flight) while another case is selected, by case (ADR 0149, the maintainer's answer to ADR 0148 question 2). The
    /// case editor takes one back, as it was, when its case is selected again; at most [`Studio::max_parked`].
    pub parked: BTreeMap<String, CaseEditor>,
    /// The most editors [`Studio::parked`] keeps ([`MAX_PARKED`]; tests may lower it).
    pub max_parked: usize,
    /// The selected case when its read showed a benchmark descriptor, with ADR 0147's one line (ADR 0149): Run is
    /// greyed out with that line.
    pub not_a_case: Option<(String, String)>,
    /// The question asked before the project closes while cases hold unsaved state (ADR 0149).
    pub close_ask: Option<CloseAsk>,
    /// Set when the question answered for a window close let the window go: the UI closes it.
    pub exit_now: bool,
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
    /// The mesh of the open case, shown by the viewport while no run is selected (ADR 0138).
    pub preview: crate::preview::Preview,
    /// Status and error messages, latest last.
    pub messages: Vec<String>,
    /// The Claude Code sidebar (ADR 0134); off until `main` configures its bridge.
    pub chat: ChatModel,
    /// Watcher period for projects opened from now on (`None`: watcher off).
    pub poll: Option<Duration>,
    /// Run worker period for projects opened from now on (`None`: no run worker).
    pub run_poll: Option<Duration>,
    /// Negative-control switches of the run worker (tests only; all off in the app).
    pub run_switches: Switches,
    /// The runner program the run worker starts (`RCFD_RUNNER_BIN`, else `rcfd-runner` next to this program).
    pub runner_exe: PathBuf,
    /// The one-click path: Run button, status strip, binaries search (ADR 0141).
    pub quick: crate::quick_run::QuickRun,
    /// Stop in Run's place (ADR 0161).
    pub stop: crate::stop::StopState,
    /// The Viewport's Mesh / Run toggle (ADR 0144).
    pub view: crate::view_toggle::Toggle,
    shown_run: Option<String>,
    pub(crate) wake: Wake,
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
            selected_case: None,
            editor: None,
            editor_error: None,
            parked: BTreeMap::new(),
            max_parked: MAX_PARKED,
            not_a_case: None,
            close_ask: None,
            exit_now: false,
            pending_case: None,
            selected_run: None,
            plots: MonitorStore::default(),
            monitors: MonitorStore::default(),
            runs: RunsView::default(),
            plot_marker: None,
            viewport: Default::default(),
            preview: Default::default(),
            messages: Vec::new(),
            chat: ChatModel::default(),
            poll: Some(POLL),
            run_poll: Some(poll::PERIOD),
            run_switches: Switches::default(),
            runner_exe: default_runner_exe(),
            quick: Default::default(),
            stop: Default::default(),
            view: Default::default(),
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

    /// Close the project (and the case open in the editor; unsaved edits, also those kept for other cases, are dropped).
    /// The window's Project menu, opening another project and the window's close ask first ([`Studio::request_close`],
    /// ADR 0149); this call does not.
    pub fn close_project(&mut self) {
        self.selected_case = None;
        self.editor = None;
        self.editor_error = None;
        self.parked.clear();
        self.not_a_case = None;
        self.close_ask = None;
        self.pending_case = None;
        self.selected_run = None;
        self.project = None;
        self.chat.close();
        self.runs = RunsView::default();
        self.plots.clear();
        self.monitors.clear();
        self.shown_run = None;
        self.viewport.close();
        self.preview.close();
        self.plot_marker = None;
        self.quick.close();
        self.stop.close();
        self.view.close();
    }

    /// Select a case (a click in the Project panel): it is the selected case from now on (ADR 0148); the case editor
    /// lets go of the case it held, keeping its unsaved state in memory (ADR 0149), and holds this case: as it was left,
    /// with its unsaved state, else from its file once the worker has read it; the selected run is this case's newest
    /// run, or none (ADR 0149); the preview builds the case and the Viewport switches to its mesh (ADR 0144). Refused,
    /// with a message and nothing changed, when keeping the unsaved state of the case left would keep more than
    /// [`Studio::max_parked`] editors.
    pub fn open_case(&mut self, rel: &str) {
        if self.project.is_none() {
            return;
        }
        if let Err(m) = self.select_case(rel) {
            self.say(m);
            return;
        }
        if self.linked() {
            // ADR 0149: the Run side shows the selected case's newest run (the side stays on Mesh: show_case_mesh)
            self.selected_run = self.newest_run_of(rel);
            self.sync_shown();
        }
        self.show_case_mesh(rel);
    }

    /// A click on run `id` in the Runs panel (ADR 0149, the maintainer's answer to ADR 0148 question 1): its case (from
    /// its `run.json`) becomes the selected case, as a click in the Project panel makes it, but the Viewport goes to the
    /// Run side and shows this run (not the case's newest). When the run's case is no longer among the project's case
    /// files (deleted or renamed; the listing says so), the selected case is not changed, the status bar says why and
    /// the Run side's caption names the missing case. Refused like [`Studio::open_case`] when the case left's unsaved
    /// state cannot be kept. With [`crate::quick_run::Switches::case_run_unlinked`], [`Studio::select_run`] (ADR 0144).
    pub fn open_run(&mut self, id: &str) {
        if self.linked() {
            match self.run_case(id) {
                Some(c) if self.case_listed(&c) => {
                    if self.selected_case.as_deref() != Some(c.as_str()) {
                        if let Err(m) = self.select_case(&c) {
                            self.say(m);
                            return;
                        }
                        // the Mesh side shows the run's case when toggled; the side switches to Run below
                        self.preview.select(&c);
                    }
                }
                Some(c) => self.say(format!(
                    "run {id}: its case {c} is not in the project any more (deleted or renamed), so the selected case \
                     is unchanged"
                )),
                None => {}
            }
        }
        self.select_run(id);
    }

    /// True while a click on a case selects its newest run and a click on a run selects its case (ADR 0149).
    pub fn linked(&self) -> bool {
        !self.quick.switches.case_run_unlinked && !self.quick.switches.selection_from_editor
    }

    /// True while the unsaved state of a case left is kept (ADR 0149).
    pub fn keeps_drafts(&self) -> bool {
        !self.quick.switches.drop_drafts && !self.quick.switches.selection_from_editor
    }

    /// The newest run of `case` by `created`, then id (ADR 0147's order), among the runs listed and the runs the run
    /// worker knows; the run Run queued last counts as the newest of its case until it is listed (the run worker lists a
    /// new run at its next tick); `None` when the case has none.
    pub fn newest_run_of(&self, case: &str) -> Option<String> {
        let listed = self
            .browser()
            .into_iter()
            .flat_map(|b| b.runs.iter())
            .filter(|r| r.case == case)
            .map(|r| (r.created.as_str(), r.id.as_str()));
        let rows = self
            .runs
            .rows
            .values()
            .filter(|r| r.case == case)
            .map(|r| (r.created.as_str(), r.id.as_str()));
        let known = listed.chain(rows).max().map(|(_, id)| id.to_string());
        match self
            .quick
            .run
            .as_ref()
            .filter(|(id, plan)| plan.case == case && !self.run_listed(id))
        {
            Some((id, _)) => Some(id.clone()),
            None => known,
        }
    }

    /// True when run `id` is listed or known to the run worker.
    fn run_listed(&self, id: &str) -> bool {
        self.runs.rows.contains_key(id) || self.browser().is_some_and(|b| b.runs.iter().any(|r| r.id == id))
    }

    /// The case of run `id` as its `run.json` names it (the run worker's row, else the listing, else the plan of the run
    /// Run queued last).
    pub fn run_case(&self, id: &str) -> Option<String> {
        self.runs
            .row(id)
            .map(|r| r.case.clone())
            .or_else(|| {
                self.browser()
                    .and_then(|b| b.runs.iter().find(|r| r.id == id))
                    .map(|r| r.case.clone())
            })
            .or_else(|| self.quick.plan_of(id).map(|p| p.case.clone()))
    }

    /// True when `case` is one of the project's case files as last listed (ADR 0148's `.toml` / `.json` rule). The
    /// listing follows the watcher (or Project > Refresh): a case removed while the watcher is off is listed until then.
    pub fn case_listed(&self, case: &str) -> bool {
        crate::project_view::is_case_file(case) && self.browser().is_some_and(|b| b.cases.iter().any(|c| c == case))
    }

    /// The case editor of `rel`, wherever it is: the open one, or one kept with its unsaved state (ADR 0149).
    pub fn case_editor(&self, rel: &str) -> Option<&CaseEditor> {
        match &self.editor {
            Some(e) if e.rel == rel => Some(e),
            _ => self.parked.get(rel),
        }
    }

    fn case_editor_mut(&mut self, rel: &str) -> Option<&mut CaseEditor> {
        match &mut self.editor {
            Some(e) if e.rel == rel => Some(e),
            _ => self.parked.get_mut(rel),
        }
    }

    /// The cases whose editor holds unsaved state (the open case's and the kept ones), sorted (ADR 0149; the Project
    /// panel marks them).
    pub fn unsaved_cases(&self) -> Vec<String> {
        let mut v: Vec<String> = self.parked.keys().cloned().collect();
        if let Some(e) = self.editor.as_ref().filter(|e| has_unsaved(e)) {
            if !v.contains(&e.rel) {
                v.push(e.rel.clone());
            }
        }
        v.sort();
        v
    }

    /// Make `rel` the selected case (ADR 0148) and the case editor's, without touching the selected run or the
    /// Viewport's side: the editor of the case left is kept with its unsaved state ([`Studio::parked`], ADR 0149) or
    /// let go when clean; the editor of `rel` is taken back as it was, else `rel` is read (the editor is empty until it
    /// arrives). The file is read in both cases; for an editor taken back, the bytes are compared when they arrive
    /// ([`CaseEditor::returned`]). With [`crate::quick_run::Switches::drop_drafts`], ADR 0148's rule (dropped).
    fn select_case(&mut self, rel: &str) -> Result<(), String> {
        let route_before = self.quick.switches.selection_from_editor;
        if self.keeps_drafts() {
            self.leave_case(rel)?;
        } else if let (Some(p), Some(old)) = (&self.project, &self.editor) {
            p.worker.send(Command::Untrack(old.rel.clone()));
        }
        self.selected_case = Some(rel.to_string());
        self.editor_error = None;
        self.not_a_case = None;
        if !route_before {
            // ADR 0148: the editor follows the selection at once ("Loading..." until the file arrives); ADR 0149: an
            // editor kept with its unsaved state comes back at once
            self.editor = if self.keeps_drafts() { self.take_back(rel) } else { None };
        }
        self.pending_case = Some(rel.to_string());
        if let Some(p) = &self.project {
            p.worker.send(Command::Load(rel.to_string()));
        }
        Ok(())
    }

    /// Let go of the case editor before `next` is selected (ADR 0149): an editor with unsaved state is kept in
    /// [`Studio::parked`] (refused, the editor left as it is, when that would keep more than [`Studio::max_parked`]); a
    /// clean one is dropped. Its file is no longer watched either way; the editor of `next` itself stays open when it has
    /// unsaved state ([`Studio::take_back`] keeps it).
    fn leave_case(&mut self, next: &str) -> Result<(), String> {
        let Some(ed) = self.editor.take() else {
            return Ok(());
        };
        if ed.rel == next && has_unsaved(&ed) {
            self.editor = Some(ed);
            return Ok(());
        }
        if has_unsaved(&ed) && ed.rel != next && !self.parked.contains_key(next) && self.parked.len() >= self.max_parked
        {
            let n = self.parked.len();
            self.editor = Some(ed);
            return Err(format!(
                "{next} was not opened: {n} other cases hold unsaved edits, the most the Studio keeps; save or discard \
                 the edits of one of them first (marked * in the Project panel)"
            ));
        }
        if let Some(p) = &self.project {
            p.worker.send(Command::Untrack(ed.rel.clone()));
        }
        if has_unsaved(&ed) && ed.rel != next {
            self.parked.insert(ed.rel.clone(), ed);
        }
        Ok(())
    }

    /// The editor of `rel` with its unsaved state: the open one (the same case selected again) or the one kept for it.
    fn take_back(&mut self, rel: &str) -> Option<CaseEditor> {
        match self.editor.take() {
            Some(ed) if ed.rel == rel => Some(ed),
            _ => self.parked.remove(rel),
        }
    }

    /// "Discard edits" in the case editor (ADR 0149): the unsaved edits, the field being typed and the conflict of the
    /// open case are dropped and its file is read again. Refused while a save of it is in flight.
    pub fn discard_edits(&mut self) -> Result<(), String> {
        let ed = self.editor.as_ref().ok_or("no case is open in the case editor")?;
        if ed.saving {
            return Err(format!("{}: a save is in progress", ed.rel));
        }
        let rel = ed.rel.clone();
        self.editor = None;
        self.parked.remove(&rel);
        self.pending_case = Some(rel.clone());
        if let Some(p) = &self.project {
            p.worker.send(Command::Load(rel.clone()));
        }
        // the mesh shown may be the edits': build the file again
        self.preview.rebuild();
        self.say(format!("{rel}: edits discarded; read again from the file"));
        Ok(())
    }

    /// Close the project, asking first while any case holds unsaved state (ADR 0149): `then` follows the close (open or
    /// create another project, close the window). True when nothing had to be asked and `then` was done (for
    /// [`AfterClose::Exit`]: the window may close, nothing else is done); false when the question is asked
    /// ([`Studio::close_ask`], answered by [`Studio::answer_close`]). With
    /// [`crate::quick_run::Switches::drop_drafts`], never asks (the route before: unsaved edits are dropped).
    pub fn request_close(&mut self, then: AfterClose) -> bool {
        let cases = self.unsaved_cases();
        if cases.is_empty() || !self.keeps_drafts() {
            if then != AfterClose::Exit {
                self.close_then(then);
            }
            return true;
        }
        self.close_ask = Some(CloseAsk {
            then,
            cases,
            saving: BTreeSet::new(),
            error: None,
        });
        false
    }

    fn close_then(&mut self, then: AfterClose) {
        self.close_project();
        match then {
            AfterClose::Nothing => {}
            AfterClose::Open(p) => self.open_project(&p),
            AfterClose::Create {
                root,
                name,
                solver_version,
            } => self.create_project(&root, &name, &solver_version),
            AfterClose::Exit => self.exit_now = true,
        }
    }

    /// Answer the close question (ADR 0149). Save all: in every case with unsaved state a field being typed is
    /// committed first (a refused commit stops, with its message, and nothing closes), a case in conflict stops the close
    /// (resolve it first), the others are saved with the base-hash rule; when every save is written the project closes
    /// and the action asked for follows; a save that meets a conflict or fails stops the close (the question stays with
    /// the reason). Discard: the project closes, the edits are dropped. Cancel: nothing closes.
    pub fn answer_close(&mut self, a: CloseAnswer) {
        let Some(ask) = self.close_ask.take() else {
            return;
        };
        match a {
            CloseAnswer::Cancel => self.say("the project stays open (close cancelled)"),
            CloseAnswer::Discard => {
                let n = ask.cases.len();
                self.close_then(ask.then);
                self.say(format!("closed; the unsaved edits of {n} case(s) were discarded"));
            }
            CloseAnswer::SaveAll => {
                let mut ask = ask;
                ask.error = None;
                match self.save_all() {
                    Err(m) => {
                        ask.error = Some(m.clone());
                        self.say(m);
                        self.close_ask = Some(ask);
                    }
                    Ok(saving) if saving.is_empty() => self.close_then(ask.then),
                    Ok(saving) => {
                        ask.saving = saving;
                        self.close_ask = Some(ask);
                    }
                }
            }
        }
    }

    /// Save every case with unsaved state for the close question: the cases whose saves are now in flight, or why it
    /// stopped (nothing is saved after a refusal; a typed field committed before it stays committed).
    fn save_all(&mut self) -> Result<BTreeSet<String>, String> {
        let mut rels: Vec<String> = self.parked.keys().cloned().collect();
        if let Some(e) = self.editor.as_ref().filter(|e| has_unsaved(e)) {
            rels.push(e.rel.clone());
        }
        for r in &rels {
            if self.case_editor(r).is_some_and(CaseEditor::in_conflict) {
                return Err(format!(
                    "{r}: resolve the conflict in the case editor first (Keep my edits or Take the disk version), or \
                     close with Discard"
                ));
            }
        }
        let schema = self.schema.clone();
        for r in &rels {
            let Some(ed) = self.case_editor_mut(r) else { continue };
            if let Some((path, text)) = ed.draft.clone() {
                ed.edit_text(&schema, &path, &text)
                    .map_err(|m| format!("{r}: {m}: correct the field, or close with Discard"))?;
            }
        }
        // the bytes of every save first, so that a refusal leaves no editor waiting for a save never sent
        let mut saving = BTreeSet::new();
        let mut sends = Vec::new();
        for r in &rels {
            let Some(ed) = self.case_editor_mut(r) else { continue };
            if ed.saving {
                saving.insert(r.clone());
            } else if ed.is_dirty() {
                let bytes = ed.to_bytes().map_err(|m| format!("{r}: {m}"))?;
                sends.push((r.clone(), ed.base_hash.clone(), bytes));
            }
        }
        for (rel, base, bytes) in sends {
            if let Some(ed) = self.case_editor_mut(&rel) {
                ed.saving = true;
            }
            if let Some(p) = &self.project {
                p.worker.send(Command::Save {
                    rel: rel.clone(),
                    base,
                    bytes,
                });
            }
            saving.insert(rel);
        }
        // editors whose typed field was the value already are clean now
        self.release_clean();
        Ok(saving)
    }

    /// Let go of kept editors that hold nothing unsaved any more (a save of them written).
    fn release_clean(&mut self) {
        let clean: Vec<String> = self
            .parked
            .iter()
            .filter(|(_, e)| !has_unsaved(e))
            .map(|(r, _)| r.clone())
            .collect();
        for r in clean {
            self.parked.remove(&r);
        }
    }

    /// A save for the close question ended: written (`None`) or not (the reason).
    fn close_save_done(&mut self, rel: &str, error: Option<String>) {
        let Some(ask) = self.close_ask.as_mut() else { return };
        if !ask.saving.remove(rel) {
            return;
        }
        if let Some(e) = error {
            ask.error = Some(e);
        }
        if ask.saving.is_empty() && ask.error.is_none() {
            if let Some(ask) = self.close_ask.take() {
                self.close_then(ask.then);
            }
        }
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
        let Some(p) = &self.project else {
            return crate::quick_run::drain(self);
        };
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
        // the viewport follows the selected run on the Run side of the toggle (ADR 0132, ADR 0144); the plots mark its
        // step
        self.sync_viewport();
        n += self.viewport.drain();
        self.plot_marker = self.viewport.marker();
        // on the Mesh side the viewport shows the open case's mesh (ADR 0138, ADR 0144)
        let wake = self.wake.clone();
        n += crate::preview::follow(self, &wake);
        let (k, msgs) = self.chat.drain();
        for m in msgs {
            self.say(m);
        }
        n + k + crate::quick_run::drain(self) + crate::stop::drain(self)
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

    /// Select a run (its plots, events and log are read from now on); the Viewport switches to it (ADR 0144).
    pub fn select_run(&mut self, id: &str) {
        self.selected_run = Some(id.to_string());
        self.sync_shown();
        self.show_run_fields();
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

    /// Queue a new run of run `id`'s case, unchanged since that run, with the field store on, and make sure a runner
    /// is alive (ADR 0140 D9, on a control thread). The id, or why it was not started, arrives as a message.
    pub fn rerun_with_fields(&mut self, id: &str) -> Result<(), String> {
        self.send_runs(poll::Command::Rerun(id.to_string()))
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
            Update::PlanCells { case, result } => {
                // ADR 0140 D5: only into the plan of the same case (the dialog may have moved on)
                if let Some(Ok(p)) = self.runs.plan.as_mut() {
                    if p.case == case {
                        crate::store_plan::set_cells(p, result);
                    }
                }
            }
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
                self.chat.open(&b.root, self.wake.clone());
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
                // ADR 0149: an editor taken back with its unsaved state keeps it; the file read again is compared with
                // what it was opened from (ADR 0131's rule, a field being typed counted as an edit)
                if self.keeps_drafts() {
                    if let Some(ed) = self.editor.as_mut().filter(|e| e.rel == rel) {
                        match ed.returned(Some(bytes)) {
                            External::Same => {}
                            External::Reloaded => self.say(format!("{rel}: changed on disk, reloaded")),
                            External::Conflict => self.say(format!(
                                "{rel}: changed on disk while its edits were kept; resolve the conflict"
                            )),
                        }
                        return;
                    }
                }
                // ADR 0144: relative data paths against the case's own folder, as a run resolves them
                let dir = self
                    .browser()
                    .map(|b| b.root.join(&rel))
                    .and_then(|f| f.parent().map(Path::to_path_buf));
                let ed = match &dir {
                    Some(d) => CaseEditor::from_bytes_in(self.svc.clone(), &rel, d, &bytes),
                    None => CaseEditor::from_bytes(self.svc.clone(), &rel, &bytes),
                };
                match ed {
                    Ok(ed) => {
                        self.editor = Some(ed);
                        self.editor_error = None;
                    }
                    Err(m) => {
                        // ADR 0147 (the maintainer's answer to ADR 0144 question 4): a benchmark descriptor gets the
                        // preview's one-line "not a case file" message, not the list of its unknown keys
                        let descriptor = std::str::from_utf8(&bytes)
                            .ok()
                            .and_then(|t| crate::preview::descriptor_message(&rel, t));
                        // ADR 0149: a benchmark descriptor is never startable; Run says ADR 0147's one line
                        if let Some(d) = descriptor
                            .as_ref()
                            .filter(|_| self.selected_case.as_deref() == Some(&rel))
                        {
                            self.not_a_case = Some((rel.clone(), d.clone()));
                        }
                        let short = descriptor.filter(|_| !self.preview.switches.editor_lists_descriptor_keys);
                        let m = short.unwrap_or(m);
                        self.editor_cannot_open(&rel, &m);
                        self.say(m);
                    }
                }
            }
            Event::LoadFailed { rel, message } => {
                // ADR 0148: only the failure of the file asked for last ends the wait (the route before: any failure)
                if self.quick.switches.selection_from_editor || self.pending_case.as_deref() == Some(rel.as_str()) {
                    self.pending_case = None;
                    // ADR 0149: an editor taken back with its unsaved state keeps it: the file is gone, ADR 0131's
                    // conflict state (keep the edits to write it again)
                    let kept = self.keeps_drafts() && self.editor.as_ref().is_some_and(|e| e.rel == rel);
                    if kept {
                        if let Some(ed) = self.editor.as_mut() {
                            ed.returned(None);
                        }
                    } else {
                        self.editor_cannot_open(&rel, &message);
                    }
                }
                self.say(message);
            }
            Event::Failed(m) => {
                if self.quick.switches.selection_from_editor {
                    self.pending_case = None; // the route before ADR 0148: a failure of any kind ended the wait
                }
                if let Some(ed) = &mut self.editor {
                    ed.saving = false;
                }
                self.say(m);
            }
            Event::Saved { rel, hash } => {
                // ADR 0149: the save of a kept editor (the close question's "Save all") reaches it too
                let kept = self.parked.contains_key(&rel);
                if let Some(ed) = self.case_editor_mut(&rel) {
                    ed.saved(hash);
                }
                if kept {
                    if let Some(p) = &self.project {
                        p.worker.send(Command::Untrack(rel.clone()));
                    }
                    self.release_clean();
                }
                self.say(format!("saved {rel}"));
                self.close_save_done(&rel, None);
            }
            Event::SaveConflict { rel, disk } => {
                if let Some(ed) = self.case_editor_mut(&rel) {
                    ed.save_conflict(disk);
                }
                let m = format!("{rel}: not saved, the file changed on disk; resolve the conflict");
                self.say(m.clone());
                self.close_save_done(&rel, Some(m));
            }
            Event::SaveFailed { rel, message } => {
                if let Some(ed) = self.case_editor_mut(&rel) {
                    ed.saving = false;
                }
                // the status bar as before ADR 0149 (the message of the failed write); the close question names the case
                self.say(message.clone());
                self.close_save_done(&rel, Some(format!("{rel}: not saved: {message}")));
            }
            Event::Created {
                rel,
                opened,
                companions,
            } => crate::case_starter::on_created(self, &rel, opened, &companions),
        }
    }

    /// The case editor could not open the selected case `rel` (ADR 0148): it holds nothing and shows `why` for that
    /// case (the route before: it kept the case it held).
    fn editor_cannot_open(&mut self, rel: &str, why: &str) {
        if self.quick.switches.selection_from_editor {
            return;
        }
        self.editor = None;
        self.editor_error = Some((rel.to_string(), why.to_string()));
    }

    /// A run was queued by Run after another case was selected (ADR 0148): the run is selected (the plots, the log and
    /// the strip follow it) and the Viewport stays on the selected case's mesh.
    pub(crate) fn select_run_keep_view(&mut self, id: &str) {
        self.selected_run = Some(id.to_string());
        self.sync_shown();
        self.keep_view_side();
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
