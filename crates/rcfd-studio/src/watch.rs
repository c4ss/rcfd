//! The project worker (ADR 0131 Watching, Threads): one thread per open project owns the `Project` and its poll `Watcher`,
//! does every file operation of the browser and the editor, and sends the results to the UI by channel. The UI thread only
//! drains the channel.

use crate::case_edit::{save_if_base, SaveOutcome};
use crate::project_view::{self, Browser};
use rcfd_project::{Project, Watcher};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Poll period of the watcher (ADR 0131: 500 ms).
pub const POLL: Duration = Duration::from_millis(500);

/// How the worker gets its project.
#[derive(Debug, Clone)]
pub enum Start {
    /// Open an existing project folder.
    Open(PathBuf),
    /// Create a project folder, then open it.
    Create {
        /// Folder.
        root: PathBuf,
        /// Manifest name.
        name: String,
        /// Solver version recorded in the manifest.
        solver_version: String,
    },
}

/// Requests from the UI.
#[derive(Debug, Clone)]
pub enum Command {
    /// Re-read the browser lists now.
    Refresh,
    /// Read a project file (project-relative) and report its changes from now on.
    Load(String),
    /// Stop reporting changes of a file.
    Untrack(String),
    /// Write a file if its hash is still `base` ([`save_if_base`]).
    Save {
        /// Project-relative path.
        rel: String,
        /// Hash the editor loaded or last saved.
        base: String,
        /// New content.
        bytes: Vec<u8>,
    },
}

/// Results for the UI.
#[derive(Debug, Clone)]
pub enum Event {
    /// The project is open; its lists.
    Opened(Browser),
    /// Open or create failed (located message). The worker has stopped.
    OpenFailed(String),
    /// The lists after a change on disk or a refresh.
    Listing(Browser),
    /// Paths (project-relative) that changed on disk since the previous poll.
    Changed(Vec<String>),
    /// A tracked file changed on disk; its new bytes (`None`: removed or unreadable).
    FileChanged {
        /// Project-relative path.
        rel: String,
        /// New content.
        bytes: Option<Vec<u8>>,
    },
    /// A file was read for [`Command::Load`].
    Loaded {
        /// Project-relative path.
        rel: String,
        /// Content.
        bytes: Vec<u8>,
    },
    /// A file operation failed (located message).
    Failed(String),
    /// A save was written; the new base hash.
    Saved {
        /// Project-relative path.
        rel: String,
        /// New hash.
        hash: String,
    },
    /// A save found another hash and wrote nothing.
    SaveConflict {
        /// Project-relative path.
        rel: String,
        /// Current content (`None`: no file).
        disk: Option<Vec<u8>>,
    },
}

/// Handle of a running worker. Dropping it stops and joins the thread.
pub struct ProjectWorker {
    tx: Option<Sender<Command>>,
    rx: Receiver<Event>,
    handle: Option<JoinHandle<()>>,
}

/// Called by the worker after it sent something, so the UI can repaint (only on new data, no busy loop).
pub type Wake = Arc<dyn Fn() + Send + Sync>;

impl ProjectWorker {
    /// Start a worker. `poll`: the watcher period, or `None` to run with the watcher off (commands only).
    pub fn start(start: Start, poll: Option<Duration>, wake: Wake) -> ProjectWorker {
        let (ctx, crx) = mpsc::channel();
        let (etx, erx) = mpsc::channel();
        let handle = std::thread::Builder::new()
            .name("rcfd-studio-project".into())
            .spawn(move || run(start, poll, crx, etx, wake))
            .expect("spawn the project worker thread");
        ProjectWorker {
            tx: Some(ctx),
            rx: erx,
            handle: Some(handle),
        }
    }

    /// Send a request (ignored once the worker stopped).
    pub fn send(&self, c: Command) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(c);
        }
    }

    /// Everything the worker sent since the last call; never blocks.
    pub fn drain(&self) -> Vec<Event> {
        self.rx.try_iter().collect()
    }
}

impl Drop for ProjectWorker {
    fn drop(&mut self) {
        // Closing the channel ends the worker's wait at once.
        self.tx = None;
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn read_opt(p: &Project, rel: &str) -> Option<Vec<u8>> {
    p.resolve(rel).ok().and_then(|path| std::fs::read(path).ok())
}

fn run(start: Start, poll: Option<Duration>, rx: Receiver<Command>, tx: Sender<Event>, wake: Wake) {
    let send = |e: Event| {
        let ok = tx.send(e).is_ok();
        wake();
        ok
    };
    let opened = match start {
        Start::Open(root) => project_view::open(&root),
        Start::Create {
            root,
            name,
            solver_version,
        } => project_view::create(&root, &name, &solver_version),
    };
    let project = match opened {
        Ok(p) => p,
        Err(m) => {
            send(Event::OpenFailed(m));
            return;
        }
    };
    let mut watcher = match poll.map(|_| Watcher::new(project.root())).transpose() {
        Ok(w) => w,
        Err(e) => {
            send(Event::Failed(format!("watcher: {e}")));
            None
        }
    };
    // the lists as last sent; a change under runs/ re-reads only the runs it touches (ADR 0133 R12)
    let mut current = Browser::load(&project);
    if !send(Event::Opened(current.clone())) {
        return;
    }
    let mut tracked: BTreeSet<String> = BTreeSet::new();
    let period = poll.unwrap_or(Duration::from_secs(3600));
    let mut next_poll = Instant::now() + period;
    loop {
        let wait = next_poll.saturating_duration_since(Instant::now());
        match rx.recv_timeout(wait) {
            Ok(cmd) => {
                let ev = match cmd {
                    Command::Refresh => {
                        current = Browser::load(&project);
                        Event::Listing(current.clone())
                    }
                    Command::Load(rel) => match project
                        .resolve(&rel)
                        .map_err(|e| e.to_string())
                        .and_then(|p| std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display())))
                    {
                        Ok(bytes) => {
                            tracked.insert(rel.clone());
                            Event::Loaded { rel, bytes }
                        }
                        Err(m) => Event::Failed(m),
                    },
                    Command::Untrack(rel) => {
                        tracked.remove(&rel);
                        continue;
                    }
                    Command::Save { rel, base, bytes } => match project
                        .resolve(&rel)
                        .map_err(|e| e.to_string())
                        .and_then(|p| save_if_base(&p, &base, &bytes))
                    {
                        Ok(SaveOutcome::Saved(hash)) => {
                            tracked.insert(rel.clone());
                            Event::Saved { rel, hash }
                        }
                        Ok(SaveOutcome::Conflict(disk)) => Event::SaveConflict { rel, disk },
                        Err(m) => Event::Failed(m),
                    },
                };
                if !send(ev) {
                    return;
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {
                next_poll = Instant::now() + period;
                let Some(w) = watcher.as_mut() else { continue };
                let before = w.token();
                match w.poll() {
                    Ok(t) if t != before => {
                        let changed = w.changes_since(before);
                        let files: Vec<Event> = changed
                            .iter()
                            .filter(|c| tracked.contains(*c))
                            .map(|rel| Event::FileChanged {
                                rel: rel.clone(),
                                bytes: read_opt(&project, rel),
                            })
                            .collect();
                        let read = current.refresh(&project, &changed);
                        let mut ok = send(Event::Changed(changed));
                        for f in files {
                            ok &= send(f);
                        }
                        if read {
                            ok &= send(Event::Listing(current.clone()));
                        }
                        if !ok {
                            return;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        if !send(Event::Failed(format!("watcher: {e}"))) {
                            return;
                        }
                    }
                }
            }
        }
    }
}
