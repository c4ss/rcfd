//! The Studio's state and decisions, without drawing (ADR 0131 Design). Every method is callable from tests; the UI calls
//! the same methods. File work is sent to the project worker ([`crate::watch`]); [`Studio::drain`] applies its results.

use crate::case_edit::{schema_tree, CaseEditor, EditorState, External};
use crate::layout::{self, Panel};
use crate::monitors::MonitorStore;
use crate::prefs::Prefs;
use crate::project_view::Browser;
use crate::schema_model::Node;
use crate::theme::Theme;
use crate::watch::{Command, Event, ProjectWorker, Start, Wake, POLL};
use egui_dock::DockState;
use rcfd_api::service::{Service, ServiceConfig};
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
    /// Paths changed on disk, with the time the model received them, latest last (at most 50).
    pub changes: Vec<(Instant, String)>,
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
    /// Monitor series of the selected run (filled in 48D, ADR 0133).
    pub plots: MonitorStore,
    /// Status and error messages, latest last.
    pub messages: Vec<String>,
    /// Watcher period for projects opened from now on (`None`: watcher off).
    pub poll: Option<Duration>,
    wake: Wake,
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
            messages: Vec::new(),
            poll: Some(POLL),
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

    /// Apply everything the worker sent. Returns the number of events applied. Never blocks.
    pub fn drain(&mut self) -> usize {
        let Some(p) = &self.project else { return 0 };
        let events = p.worker.drain();
        let n = events.len();
        for e in events {
            self.apply(e);
        }
        n
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
                if let Some(p) = &mut self.project {
                    p.browser = Some(b);
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
                    p.changes.extend(paths.into_iter().map(|c| (now, c)));
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
}
