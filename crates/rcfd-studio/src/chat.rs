//! The Claude Code sidebar's model and worker (ADR 0134, integration part). No drawing: [`crate::chat_ui`] draws it.
//!
//! * Sessions of the open project from `Claude::discover` every [`DISCOVER_PERIOD`] on the chat worker: OPEN ones are
//!   mirrored read-only (input refused here, before any process), CLOSED ones are resumed with `claude -p --resume`,
//!   UNKNOWN ones are not written to (user decision 1 of plan section 8).
//! * The selected session's transcript is mirrored: the last [`TAIL`] entries, then the new complete lines every
//!   [`MIRROR_PERIOD`].
//! * A message starts one `claude -p` turn on a short-lived thread (`Claude::resume` or `Claude::new_session`, which
//!   check the folder against the per-user trust record and refuse an unconfirmed one); its events arrive here through
//!   the same channel as the worker's.
//! * The folder confirmation shows `rcfd_claude::trust::Pending` (files, what changed, the allow file's rules) and
//!   records the user's confirmation in the trust record in the Studio's config folder (never inside the project).
//! * Approvals: a denied call shows its input and the exact suggested rule; "allow" writes the project's allow-list file
//!   through `TrustStore::save_allow_list`, so that change alone does not ask for a confirmation again.
//!
//! Every file, process and listing call runs on the worker or a turn thread; the UI thread only drains a channel.

use rcfd_claude::allow::{pinned_mcp_config, AllowList, AllowRule};
use rcfd_claude::discover::{project_dir_name, Discovery, OpenState, SessionInfo};
use rcfd_claude::stream::StreamEvent;
use rcfd_claude::transcript::{parse_entry, read_new, Role, TranscriptEntry};
use rcfd_claude::trust::{ControlFile, ControlKind, Pending, TrustState, TrustStore};
use rcfd_claude::{Claude, Error};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::watch::Wake;

/// Session discovery period (ADR 0134 Design).
pub const DISCOVER_PERIOD: Duration = Duration::from_secs(2);
/// Transcript mirror period (ADR 0134 Design).
pub const MIRROR_PERIOD: Duration = Duration::from_millis(500);
/// Entries read from the end of a transcript when a session is selected (ADR 0134 Design: `tail(50)`).
pub const TAIL: usize = 50;
/// The Claude Code version the stream and transcript fixtures were recorded with (ADR 0126); another version is shown
/// with a warning (ADR 0134 Risks).
pub const PINNED_CLI_VERSION: &str = "2.1.310";
/// The trust record, a file in the Studio's config folder (ADR 0134 Amendment 2).
pub const TRUST_FILE: &str = "claude-trust.json";
/// Folder (in the Studio's config folder) of the app-owned MCP config files, one per project (W48-10 (a)).
pub const MCP_DIR: &str = "claude-mcp";
/// Bytes of each control file shown in the confirmation dialog.
pub const PREVIEW_BYTES: usize = 4096;
/// Characters of a tool result or a tool input kept for display.
pub const SHOWN_CHARS: usize = 2000;
/// What the panel says about the allow-list's reach (ADR 0126 observed limits; ADR 0134 Design "Approvals").
pub const FLOOR_CAVEAT: &str = "The allow-list is the floor this app controls, not the whole gate: Claude Code also \
     approves some read-only shell commands by itself, and your user and project settings can add rules.";

/// Test switches for the registered negative controls (ADR 0134 Amendment 3); all off in the app.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Switches {
    /// E7 control: read the whole transcript every period, the unterminated last line included.
    pub whole_file_mirror: bool,
    /// E10 control: draw the whole feed as one text each frame.
    pub whole_text: bool,
}

/// How the sidebar reaches Claude Code and where it keeps its per-user files.
#[derive(Debug, Clone)]
pub struct Bridge {
    /// The CLI (program and Claude Code home). `trust_store` and `mcp_config` are set per project by the model.
    pub claude: Claude,
    /// The Studio's per-user config folder (trust record, MCP config files).
    pub config_dir: PathBuf,
    /// The `rcfd-mcp` program named in the app-owned MCP config.
    pub rcfd_mcp: PathBuf,
}

impl Bridge {
    /// The bridge of the app: `Claude::from_env()`, `claude` on PATH, the config folder, `rcfd-mcp` from `RCFD_MCP_BIN`
    /// or next to this program. `Err` says why the sidebar cannot chat; the rest of the Studio works without it.
    pub fn from_env(config_dir: Option<PathBuf>) -> Result<Bridge, String> {
        let config_dir = config_dir.ok_or(
            "no configuration directory (APPDATA, XDG_CONFIG_HOME or HOME): the chat needs it for the folder \
             confirmations and the MCP config, so it is off",
        )?;
        let claude = Claude::from_env().ok_or(
            "Claude Code's home folder is not known (CLAUDE_CONFIG_DIR, USERPROFILE or HOME unset), so its sessions \
             cannot be listed",
        )?;
        let exe = format!("claude{}", std::env::consts::EXE_SUFFIX);
        if find_on_path(&exe).is_none() {
            return Err(format!(
                "Claude Code was not found: no {exe} on PATH. Install Claude Code (code.claude.com/docs), make sure \
                 `claude` starts from a terminal, and start the Studio again. Everything else works without it."
            ));
        }
        let rcfd_mcp = std::env::var_os("RCFD_MCP_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| rcfd_runner::runner::sibling_exe("rcfd-mcp"));
        Ok(Bridge {
            claude,
            config_dir,
            rcfd_mcp,
        })
    }

    /// The trust record file.
    pub fn trust_file(&self) -> PathBuf {
        self.config_dir.join(TRUST_FILE)
    }

    /// The app-owned MCP config file of `project` (named after the hash of its path).
    pub fn mcp_file(&self, project: &Path) -> PathBuf {
        let h = rcfd_project::records::hash_bytes(project.to_string_lossy().as_bytes());
        let hex = h.rsplit(':').next().unwrap_or(&h).to_string();
        self.config_dir.join(MCP_DIR).join(format!("{hex}.json"))
    }
}

/// The first `name` file in a folder of PATH.
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(name)).find(|p| p.is_file())
}

// ---------------------------------------------------------------- feed (what the transcript area shows)

/// What a feed item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    /// The user's message.
    User,
    /// Model text.
    Assistant,
    /// Tool calls (names).
    Tool,
    /// A tool result fed back to the model.
    ToolResult,
    /// A denied tool call.
    Denied,
    /// A note of the app (aborted, refused, cancelled).
    Note,
    /// A transcript line of unknown shape.
    Unrecognised,
}

/// One feed item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Kind.
    pub kind: ItemKind,
    /// Text, starting with its label (`you> `, `claude> `, ...).
    pub text: String,
}

/// One display row: a byte range of one item's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    /// Item index.
    pub item: u32,
    /// Start byte.
    pub start: u32,
    /// End byte (exclusive).
    pub end: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dirty {
    Clean,
    /// Items from this index on have no rows yet.
    From(usize),
    /// Text was appended to this (last) item: only its last row is redone.
    Tail(usize),
}

/// The items of the transcript area and their rows, hard-wrapped at a width in characters. Appending text redoes only
/// the last row of the last item (greedy word wrap is stable for closed rows), so a streaming turn costs per frame the
/// text that arrived, not the whole feed. The panel draws only the visible rows.
#[derive(Debug, Clone)]
pub struct Feed {
    items: Vec<Item>,
    rows: Vec<Row>,
    width: usize,
    dirty: Dirty,
}

impl Default for Feed {
    fn default() -> Self {
        Feed {
            items: Vec::new(),
            rows: Vec::new(),
            width: 0,
            dirty: Dirty::Clean,
        }
    }
}

impl Feed {
    /// Items.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// No items?
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn mark(&mut self, d: Dirty) {
        self.dirty = match (self.dirty, d) {
            (Dirty::Clean, d) => d,
            (Dirty::Tail(a), Dirty::Tail(b)) if a == b => Dirty::Tail(a),
            (Dirty::From(a) | Dirty::Tail(a), Dirty::From(b) | Dirty::Tail(b)) => Dirty::From(a.min(b)),
            (cur, Dirty::Clean) => cur,
        };
    }

    /// Add an item.
    pub fn push(&mut self, kind: ItemKind, text: String) {
        self.items.push(Item { kind, text });
        self.mark(Dirty::From(self.items.len() - 1));
    }

    /// Append text to the last item.
    pub fn append(&mut self, text: &str) {
        if let Some(last) = self.items.last_mut() {
            last.text.push_str(text);
            let i = self.items.len() - 1;
            self.mark(Dirty::Tail(i));
        }
    }

    /// Keep the first `n` items.
    pub fn truncate(&mut self, n: usize) {
        if n < self.items.len() {
            self.items.truncate(n);
            self.mark(Dirty::From(n));
        }
    }

    /// Remove everything.
    pub fn clear(&mut self) {
        self.items.clear();
        self.rows.clear();
        self.dirty = Dirty::Clean;
    }

    /// The whole feed as one text without line breaks, items separated by `" | "` (the E10 negative control draws
    /// this). Without `\n` the text is one paragraph, so egui's galley cache, which caches each paragraph of a text
    /// separately, lays the whole text out again whenever it changes (ADR 0134 Amendment 4).
    pub fn whole_text(&self) -> String {
        let mut s = String::new();
        for it in &self.items {
            s.extend(it.text.chars().map(|c| if c == '\n' { ' ' } else { c }));
            s.push_str(" | ");
        }
        s
    }

    /// The rows of the last [`Feed::layout`].
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Bring the rows up to date for `width` characters (at least 8).
    pub fn layout(&mut self, width: usize) {
        let width = width.max(8);
        if width != self.width {
            self.width = width;
            self.rows.clear();
            self.dirty = if self.items.is_empty() {
                Dirty::Clean
            } else {
                Dirty::From(0)
            };
        }
        match self.dirty {
            Dirty::Clean => {}
            Dirty::From(first) => {
                let keep = self.rows.partition_point(|r| (r.item as usize) < first);
                self.rows.truncate(keep);
                for i in first..self.items.len() {
                    wrap(&mut self.rows, i, &self.items[i].text, 0, width);
                }
            }
            Dirty::Tail(i) => {
                let from = match self.rows.last() {
                    Some(r) if r.item as usize == i => {
                        let s = r.start as usize;
                        self.rows.pop();
                        s
                    }
                    _ => 0,
                };
                wrap(&mut self.rows, i, &self.items[i].text, from, width);
            }
        }
        self.dirty = Dirty::Clean;
    }

    /// The text of a row.
    pub fn row_text(&self, r: &Row) -> &str {
        self.items
            .get(r.item as usize)
            .and_then(|it| it.text.get(r.start as usize..r.end as usize))
            .unwrap_or("")
    }

    /// The kind of a row's item.
    pub fn row_kind(&self, r: &Row) -> ItemKind {
        self.items.get(r.item as usize).map_or(ItemKind::Note, |it| it.kind)
    }
}

/// Greedy word wrap of `text[from..]` (a row start) into rows of at most `w` characters; a `\n` ends a row; the last row
/// stays open (it may grow when text is appended).
fn wrap(rows: &mut Vec<Row>, item: usize, text: &str, from: usize, w: usize) {
    let row = |s: usize, e: usize| Row {
        item: item as u32,
        start: s as u32,
        end: e as u32,
    };
    let (mut start, mut count, mut space) = (from, 0usize, None::<usize>);
    for (ci, c) in text[from..].char_indices() {
        let pos = from + ci;
        if c == '\n' {
            rows.push(row(start, pos));
            (start, count, space) = (pos + 1, 0, None);
            continue;
        }
        if count == w {
            let brk = space.filter(|&s| s > start).unwrap_or(pos);
            rows.push(row(start, brk));
            count = text[brk..pos].chars().count();
            (start, space) = (brk, None);
        }
        if c == ' ' {
            space = Some(pos + 1);
        }
        count += 1;
    }
    rows.push(row(start, text.len()));
}

fn clip(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}... ({} more bytes)", &s[..i], s.len() - i),
        None => s.to_string(),
    }
}

// ---------------------------------------------------------------- turns

/// State of a turn (one `claude -p` process).
#[derive(Debug, Clone, PartialEq)]
pub enum TurnState {
    /// The thread is checking and starting the process.
    Starting,
    /// The process runs.
    Running {
        /// Its pid.
        pid: u32,
    },
    /// The process ended.
    Finished {
        /// Exit code (`None`: killed).
        code: Option<i32>,
    },
    /// The driver stopped it (e.g. the CLI did not apply `dontAsk`).
    Aborted(String),
    /// Nothing was started (folder not confirmed, session open, ...).
    Refused(String),
    /// The user cancelled it.
    Cancelled,
}

impl TurnState {
    /// Still running or starting?
    pub fn is_live(&self) -> bool {
        matches!(self, TurnState::Starting | TurnState::Running { .. })
    }
}

/// A denied tool call as the approvals list shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct DenialView {
    /// Tool name.
    pub tool: String,
    /// Its input as JSON text (from the matching tool call), clipped.
    pub input: String,
    /// The CLI's message.
    pub message: String,
    /// The exact rule that allows it; `None`: no rule may (control files, unsupported tools), so no button.
    pub suggestion: Option<AllowRule>,
}

/// A turn of this Studio.
#[derive(Debug, Clone)]
pub struct Turn {
    /// Session id (`None` until a new session's process started).
    pub session_id: Option<String>,
    /// Started as a new session?
    pub new_session: bool,
    /// The message.
    pub prompt: String,
    /// State.
    pub state: TurnState,
    /// Assistant text assembled from the deltas.
    pub text: String,
    /// Tool calls in order (names).
    pub tools: Vec<String>,
    /// Denied calls.
    pub denials: Vec<DenialView>,
    /// `total_cost_usd` (summed if the stream has several results).
    pub cost: Option<f64>,
    /// Final text of the result event and `is_error`.
    pub result: Option<(bool, Option<String>)>,
    /// Permission mode, CLI version and model from `system/init`.
    pub init: Option<(Option<String>, Option<String>, Option<String>)>,
    /// The argument list of the process.
    pub args: Vec<String>,
    /// Last part of stderr, once exited.
    pub stderr: String,
    inputs: HashMap<String, serde_json::Value>,
    feed_start: usize,
    text_open: bool,
    stopped: bool,
    cancel: Arc<AtomicBool>,
}

impl Turn {
    fn new(prompt: &str, session_id: Option<String>, feed_start: usize) -> Turn {
        Turn {
            new_session: session_id.is_none(),
            session_id,
            prompt: prompt.to_string(),
            state: TurnState::Starting,
            text: String::new(),
            tools: Vec::new(),
            denials: Vec::new(),
            cost: None,
            result: None,
            init: None,
            args: Vec::new(),
            stderr: String::new(),
            inputs: HashMap::new(),
            feed_start,
            text_open: false,
            stopped: false,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The cost label the panel shows (`"$0.003819"`, or `"cost not reported"`).
    pub fn cost_label(&self) -> String {
        match self.cost {
            Some(c) => format!("${c:.6}"),
            None => "cost not reported".into(),
        }
    }
}

// ---------------------------------------------------------------- folder confirmation

/// The first bytes of a listed file, for the confirmation dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    /// Project-relative path.
    pub path: String,
    /// Text (lossy UTF-8) of the first [`PREVIEW_BYTES`].
    pub text: String,
    /// The file is longer than the preview.
    pub truncated: bool,
    /// The bytes read now have the hash in the list (`false`: the file changed after the scan).
    pub matches: bool,
}

/// The folder's state for the chat.
#[derive(Debug, Clone, PartialEq)]
pub enum Folder {
    /// Not checked yet, or a check is in progress.
    Checking,
    /// No control files, or exactly the confirmed ones.
    Confirmed,
    /// The user must confirm these before a chat starts.
    Pending {
        /// What the library reports.
        pending: Pending,
        /// The first bytes of each listed file.
        previews: Vec<Preview>,
    },
    /// The folder or the record could not be read: no chat starts.
    Error(String),
}

// ---------------------------------------------------------------- worker messages

/// What a turn thread sends.
#[derive(Debug, Clone)]
pub enum TurnMsg {
    /// The process started.
    Started {
        /// Session id.
        session_id: String,
        /// Pid.
        pid: u32,
        /// Arguments.
        args: Vec<String>,
    },
    /// Nothing was started.
    Refused {
        /// Why.
        reason: String,
        /// The folder files to confirm, when that was the reason.
        pending: Option<Pending>,
    },
    /// A stream event (the last one is `Exited`).
    Event(StreamEvent),
}

/// What the worker and the turn threads send to the model.
#[derive(Debug, Clone)]
pub enum Update {
    /// A discovery result.
    Sessions(Discovery),
    /// Mirrored transcript entries of `session`. `reset`: replace the feed.
    Transcript {
        /// Session id.
        session: String,
        /// Start over.
        reset: bool,
        /// New entries.
        entries: Vec<TranscriptEntry>,
    },
    /// Folder check result.
    Folder(Folder),
    /// The project's allow-list (`Err`: the file is refused) and whether the file exists.
    Allow(Result<AllowList, String>, bool),
    /// The MCP config file was written (or why not).
    McpConfig(Result<PathBuf, String>),
    /// A message for the user.
    Notice(String),
    /// A turn's progress.
    Turn(TurnMsg),
}

enum Command {
    Discover,
    Mirror {
        target: Option<(String, PathBuf)>,
        paused: bool,
    },
    PauseMirror(bool),
    CheckFolder,
    Confirm(Vec<ControlFile>),
    SaveAllow(AllowList),
}

struct Worker {
    tx: Sender<Command>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        let (tx, _) = mpsc::channel();
        drop(std::mem::replace(&mut self.tx, tx)); // disconnect: the loop ends
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

// ---------------------------------------------------------------- the model

/// A session row as the panel shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRow {
    /// Session id.
    pub id: String,
    /// What the row says.
    pub label: String,
    /// May the user type into it?
    pub input_enabled: bool,
    /// Why not, when not.
    pub reason: Option<String>,
}

/// The sidebar's state and decisions.
pub struct ChatModel {
    /// How Claude Code is reached (`Err`: why the chat is off; the default is off, `main` configures it).
    pub bridge: Result<Bridge, String>,
    /// `--model` for new turns (empty: the user's default).
    pub model_name: String,
    /// Negative-control switches (tests only).
    pub switches: Switches,
    /// Discovery period for projects opened from now on.
    pub discover_period: Duration,
    /// The open project's folder (as `rcfd-project` reports it).
    pub project: Option<PathBuf>,
    /// Sessions of the last discovery.
    pub sessions: Vec<SessionInfo>,
    /// Why a discovery source failed.
    pub warnings: Vec<String>,
    /// Number of discoveries received.
    pub discoveries: u64,
    /// The selected session.
    pub selected: Option<String>,
    /// The transcript area.
    pub feed: Feed,
    /// The current or last turn.
    pub turn: Option<Turn>,
    /// The folder confirmation state.
    pub folder: Folder,
    /// The confirmation dialog is shown.
    pub dialog_open: bool,
    /// The allow-list (`None`: not read yet; `Err`: the file is refused).
    pub allow: Option<Result<AllowList, String>>,
    /// The allow-list file exists (else the first list is used and written on the first change).
    pub allow_on_disk: bool,
    /// The MCP config file (`None`: not written yet).
    pub mcp: Option<Result<PathBuf, String>>,
    /// Costs of the turns run here, per session.
    pub costs: BTreeMap<String, Vec<f64>>,
    /// CLI version from the last `system/init`.
    pub cli_version: Option<String>,
    /// The message being typed.
    pub input: String,
    own_pids: BTreeSet<u32>,
    own_sessions: BTreeSet<String>,
    provisional_from: Option<usize>,
    worker: Option<Worker>,
    tx: Option<Sender<Update>>,
    rx: Option<Receiver<Update>>,
    wake: Option<Wake>,
}

impl Default for ChatModel {
    fn default() -> Self {
        ChatModel {
            bridge: Err("the Claude Code chat is not configured in this Studio".into()),
            model_name: String::new(),
            switches: Switches::default(),
            discover_period: DISCOVER_PERIOD,
            project: None,
            sessions: Vec::new(),
            warnings: Vec::new(),
            discoveries: 0,
            selected: None,
            feed: Feed::default(),
            turn: None,
            folder: Folder::Checking,
            dialog_open: false,
            allow: None,
            allow_on_disk: false,
            mcp: None,
            costs: BTreeMap::new(),
            cli_version: None,
            input: String::new(),
            own_pids: BTreeSet::new(),
            own_sessions: BTreeSet::new(),
            provisional_from: None,
            worker: None,
            tx: None,
            rx: None,
            wake: None,
        }
    }
}

impl ChatModel {
    /// The `Claude` of the open project: trust record and MCP config in the config folder, the model field.
    fn claude(&self) -> Option<Claude> {
        let (Ok(b), Some(p)) = (&self.bridge, &self.project) else {
            return None;
        };
        let mut c = b.claude.clone();
        c.trust_store = Some(b.trust_file());
        c.mcp_config = Some(b.mcp_file(p));
        let m = self.model_name.trim();
        c.model = (!m.is_empty()).then(|| m.to_string());
        Some(c)
    }

    /// Start the chat for an opened project (`root` as `rcfd-project` reports it). Does nothing when the bridge is off.
    pub fn open(&mut self, root: &Path, wake: Wake) {
        self.close();
        self.project = Some(root.to_path_buf());
        let (Ok(b), Some(claude)) = (&self.bridge, self.claude()) else {
            return;
        };
        let (utx, urx) = mpsc::channel();
        let (ctx, crx) = mpsc::channel();
        let cfg = WorkerCfg {
            claude,
            project: root.to_path_buf(),
            mcp_file: b.mcp_file(root),
            rcfd_mcp: b.rcfd_mcp.clone(),
            period: self.discover_period,
            switches: self.switches,
            tx: utx.clone(),
            wake: wake.clone(),
        };
        let handle = std::thread::Builder::new()
            .name("rcfd-studio-chat".into())
            .spawn(move || worker_loop(cfg, crx))
            .expect("spawn the chat worker");
        self.worker = Some(Worker {
            tx: ctx,
            handle: Some(handle),
        });
        self.tx = Some(utx);
        self.rx = Some(urx);
        self.wake = Some(wake);
    }

    /// Stop the chat of the project (a running turn is cancelled: its thread ends when the process does).
    pub fn close(&mut self) {
        if let Some(t) = &self.turn {
            t.cancel.store(true, Ordering::SeqCst);
        }
        self.worker = None;
        let keep = (
            std::mem::replace(&mut self.bridge, Err(String::new())),
            std::mem::take(&mut self.model_name),
            self.switches,
            self.discover_period,
        );
        *self = ChatModel {
            bridge: keep.0,
            model_name: keep.1,
            switches: keep.2,
            discover_period: keep.3,
            ..ChatModel::default()
        };
    }

    fn send_cmd(&self, c: Command) {
        if let Some(w) = &self.worker {
            let _ = w.tx.send(c);
        }
    }

    /// Ask for a discovery now.
    pub fn refresh(&self) {
        self.send_cmd(Command::Discover);
    }

    /// Check the folder again.
    pub fn recheck_folder(&mut self) {
        self.folder = Folder::Checking;
        self.send_cmd(Command::CheckFolder);
    }

    /// Apply everything that arrived. Returns the number of updates and the messages for the status bar.
    pub fn drain(&mut self) -> (usize, Vec<String>) {
        let mut ups = Vec::new();
        if let Some(rx) = &self.rx {
            while let Ok(u) = rx.try_recv() {
                ups.push(u);
            }
        }
        let n = ups.len();
        let mut msgs = Vec::new();
        for u in ups {
            if let Some(m) = self.apply(u) {
                msgs.push(m);
            }
        }
        (n, msgs)
    }

    /// Apply one update (public so tests can feed a recorded stream without a process). Returns a status message.
    pub fn apply(&mut self, u: Update) -> Option<String> {
        match u {
            Update::Sessions(d) => {
                self.sessions = d.sessions;
                self.warnings = d.warnings;
                self.discoveries += 1;
                None
            }
            Update::Transcript {
                session,
                reset,
                entries,
            } => {
                if self.selected.as_deref() != Some(session.as_str()) {
                    return None;
                }
                if entries.is_empty() {
                    return None;
                }
                if reset {
                    self.feed.clear();
                } else if let Some(k) = self.provisional_from {
                    self.feed.truncate(k);
                }
                self.provisional_from = None;
                for e in entries {
                    push_entry(&mut self.feed, e);
                }
                None
            }
            Update::Folder(f) => {
                if matches!(f, Folder::Confirmed) {
                    self.dialog_open = false;
                }
                self.folder = f;
                None
            }
            Update::Allow(list, on_disk) => {
                self.allow = Some(list);
                self.allow_on_disk = on_disk;
                None
            }
            Update::McpConfig(r) => {
                let msg = r.as_ref().err().map(|e| format!("chat: {e}"));
                self.mcp = Some(r);
                msg
            }
            Update::Notice(m) => Some(m),
            Update::Turn(t) => self.apply_turn(t),
        }
    }

    fn apply_turn(&mut self, m: TurnMsg) -> Option<String> {
        if self.turn.is_none() {
            // a recorded stream fed by a test, or a stray message after close
            self.turn = Some(Turn::new("", None, self.feed.len()));
        }
        match m {
            TurnMsg::Started { session_id, pid, args } => {
                let turn = self.turn.as_mut()?;
                turn.state = TurnState::Running { pid };
                turn.args = args;
                let new = turn.new_session;
                if new {
                    turn.session_id = Some(session_id.clone());
                }
                self.own_pids.insert(pid);
                self.own_sessions.insert(session_id.clone());
                if new {
                    self.selected = Some(session_id.clone());
                    if let Some(path) = self.transcript_path(&session_id) {
                        self.send_cmd(Command::Mirror {
                            target: Some((session_id, path)),
                            paused: true,
                        });
                    }
                }
                None
            }
            TurnMsg::Refused { reason, pending } => {
                self.turn.as_mut()?.state = TurnState::Refused(reason.clone());
                self.feed.push(ItemKind::Note, format!("-- not sent: {reason}"));
                self.send_cmd(Command::PauseMirror(false));
                if let Some(p) = pending {
                    self.folder = Folder::Pending {
                        pending: p,
                        previews: Vec::new(),
                    };
                    self.dialog_open = true;
                    self.send_cmd(Command::CheckFolder); // fills the previews
                }
                Some(format!("chat: not sent: {reason}"))
            }
            TurnMsg::Event(ev) => {
                let exited = matches!(ev, StreamEvent::Exited { .. });
                let r = self.apply_event(ev);
                if exited {
                    self.send_cmd(Command::PauseMirror(false));
                    self.send_cmd(Command::Discover);
                }
                r
            }
        }
    }

    fn apply_event(&mut self, ev: StreamEvent) -> Option<String> {
        let turn = self.turn.as_mut()?;
        // after the driver aborted the turn, nothing the CLI still printed is shown (only the exit)
        if turn.stopped && !matches!(ev, StreamEvent::Exited { .. }) {
            return None;
        }
        match ev {
            StreamEvent::Init {
                permission_mode,
                version,
                model,
                ..
            } => {
                self.cli_version = version.clone().or(self.cli_version.take());
                turn.init = Some((permission_mode, version, model));
            }
            StreamEvent::TextDelta(t) => {
                turn.text.push_str(&t);
                if turn.text_open && self.feed.len() > turn.feed_start {
                    self.feed.append(&t);
                } else {
                    self.feed.push(ItemKind::Assistant, format!("claude> {t}"));
                    turn.text_open = true;
                }
            }
            StreamEvent::ToolCall { id, name, input } => {
                turn.inputs.insert(id, input);
                turn.tools.push(name.clone());
                turn.text_open = false;
                self.feed.push(ItemKind::Tool, format!("[tool] {name}"));
            }
            StreamEvent::PermissionDenied {
                tool_name,
                tool_use_id,
                message,
                suggestion,
            } => {
                let input = turn
                    .inputs
                    .get(&tool_use_id)
                    .map(|v| clip(&v.to_string(), SHOWN_CHARS))
                    .unwrap_or_else(|| "(input not seen)".into());
                turn.text_open = false;
                self.feed
                    .push(ItemKind::Denied, format!("[denied] {tool_name}: {message}"));
                turn.denials.push(DenialView {
                    tool: tool_name,
                    input,
                    message,
                    suggestion,
                });
            }
            StreamEvent::Result {
                is_error,
                text,
                cost_usd,
                ..
            } => {
                if let Some(c) = cost_usd {
                    turn.cost = Some(turn.cost.unwrap_or(0.0) + c);
                }
                turn.result = Some((is_error, text));
                turn.text_open = false;
            }
            StreamEvent::Aborted(why) => {
                turn.state = TurnState::Aborted(why.clone());
                turn.stopped = true;
                self.feed.push(ItemKind::Note, format!("-- aborted: {why}"));
            }
            StreamEvent::Exited { code, stderr_tail } => {
                turn.stderr = stderr_tail;
                if turn.cancel.load(Ordering::SeqCst) && !matches!(turn.state, TurnState::Aborted(_)) {
                    turn.state = TurnState::Cancelled;
                    self.feed.push(ItemKind::Note, "-- cancelled".into());
                } else if turn.state.is_live() {
                    turn.state = TurnState::Finished { code };
                }
                if let (Some(id), Some(c)) = (&turn.session_id, turn.cost) {
                    self.costs.entry(id.clone()).or_default().push(c);
                }
                self.provisional_from = Some(turn.feed_start);
            }
            StreamEvent::Unrecognised { .. } => {}
        }
        None
    }

    /// The expected transcript file of a session of the open project.
    fn transcript_path(&self, id: &str) -> Option<PathBuf> {
        if let Some(t) = self
            .sessions
            .iter()
            .find(|s| s.session_id == id)
            .and_then(|s| s.transcript.clone())
        {
            return Some(t);
        }
        let (Ok(b), Some(p)) = (&self.bridge, &self.project) else {
            return None;
        };
        Some(
            b.claude
                .claude_home
                .join("projects")
                .join(project_dir_name(p))
                .join(format!("{id}.jsonl")),
        )
    }

    /// The session rows of the panel.
    pub fn session_rows(&self) -> Vec<SessionRow> {
        self.sessions
            .iter()
            .map(|s| {
                let name = s
                    .name
                    .clone()
                    .unwrap_or_else(|| s.session_id.get(..8).unwrap_or(&s.session_id).to_string());
                let (state, reason) = self.state_of(s);
                SessionRow {
                    id: s.session_id.clone(),
                    label: format!("{name}  {state}"),
                    input_enabled: reason.is_none(),
                    reason,
                }
            })
            .collect()
    }

    /// What a session's state allows: the label and, when typing is refused, why.
    fn state_of(&self, s: &SessionInfo) -> (String, Option<String>) {
        match s.state {
            OpenState::Open { pid } if self.own_pids.contains(&pid) => ("this Studio's chat".into(), None),
            OpenState::Open { pid } => (
                format!("open in process {pid} (mirror only)"),
                Some(format!(
                    "this session is open in process {pid}; it is mirrored read-only until that process ends"
                )),
            ),
            OpenState::Closed => ("closed".into(), None),
            OpenState::Unknown => (
                "state unknown".into(),
                Some("whether another process has this session open cannot be established (see the warnings); it is not written to".into()),
            ),
        }
    }

    /// Select a session: its transcript is mirrored from now on. Refused while a turn runs.
    pub fn select(&mut self, id: &str) -> Result<(), String> {
        if self.turn.as_ref().is_some_and(|t| t.state.is_live()) {
            return Err("a turn is running; wait for it or cancel it".into());
        }
        if self.selected.as_deref() == Some(id) {
            return Ok(());
        }
        self.selected = Some(id.to_string());
        self.feed.clear();
        self.provisional_from = None;
        let target = self.transcript_path(id).map(|p| (id.to_string(), p));
        self.send_cmd(Command::Mirror { target, paused: false });
        Ok(())
    }

    /// Start a new chat: deselect so the next message starts a new session.
    pub fn new_chat(&mut self) -> Result<(), String> {
        if self.turn.as_ref().is_some_and(|t| t.state.is_live()) {
            return Err("a turn is running; wait for it or cancel it".into());
        }
        self.selected = None;
        self.feed.clear();
        self.provisional_from = None;
        self.send_cmd(Command::Mirror {
            target: None,
            paused: false,
        });
        Ok(())
    }

    /// Whether a message may be sent now; `Err` says why not. No file or process is touched.
    pub fn can_send(&self) -> Result<(), String> {
        self.bridge.as_ref().map_err(Clone::clone)?;
        if self.project.is_none() || self.worker.is_none() {
            return Err("no project is open".into());
        }
        if self.turn.as_ref().is_some_and(|t| t.state.is_live()) {
            return Err("a turn is running; wait for it or cancel it".into());
        }
        match &self.mcp {
            None => return Err("the MCP config is being written".into()),
            Some(Err(e)) => return Err(e.clone()),
            Some(Ok(_)) => {}
        }
        match &self.folder {
            Folder::Pending { .. } => {
                return Err("this folder's Claude Code files are not confirmed: review and confirm them first".into())
            }
            Folder::Error(e) => return Err(format!("the folder cannot be checked: {e}")),
            Folder::Checking | Folder::Confirmed => {} // the driver checks again before any process
        }
        if let Some(Err(e)) = &self.allow {
            return Err(format!(
                "the allow-list file is refused ({e}); reset it to the first list or fix it"
            ));
        }
        if let Some(id) = &self.selected {
            match self.sessions.iter().find(|s| &s.session_id == id) {
                Some(s) => {
                    if let (_, Some(why)) = self.state_of(s) {
                        return Err(why);
                    }
                }
                None if self.own_sessions.contains(id) => {}
                None => return Err("the session is no longer listed".into()),
            }
        }
        Ok(())
    }

    /// Send `prompt` to the selected session (resume) or as a new session when none is selected. Refused here, with no
    /// process, when [`ChatModel::can_send`] says so (an OPEN session, a pending folder, ...).
    pub fn send(&mut self, prompt: &str) -> Result<(), String> {
        if prompt.trim().is_empty() {
            return Err("type a message first".into());
        }
        if let Err(e) = self.can_send() {
            if matches!(self.folder, Folder::Pending { .. }) {
                self.dialog_open = true;
            }
            return Err(e);
        }
        let claude = self.claude().ok_or("no project is open")?;
        let project = self.project.clone().ok_or("no project is open")?;
        let tx = self.tx.clone().ok_or("no project is open")?;
        let wake = self.wake.clone().unwrap_or_else(|| Arc::new(|| {}));
        let session = self.selected.clone();
        if session.is_some() {
            self.send_cmd(Command::PauseMirror(true)); // the turn's own lines are shown from the stream
        }
        self.provisional_from = None;
        let feed_start = self.feed.len();
        self.feed.push(ItemKind::User, format!("you> {prompt}"));
        let turn = Turn::new(prompt, session.clone(), feed_start);
        let cancel = turn.cancel.clone();
        self.turn = Some(turn);
        let prompt = prompt.to_string();
        std::thread::Builder::new()
            .name("rcfd-studio-chat-turn".into())
            .spawn(move || run_turn(claude, project, session, prompt, cancel, tx, wake))
            .map_err(|e| format!("cannot start the turn thread: {e}"))?;
        Ok(())
    }

    /// Cancel the running turn (the process and its children are killed by the turn thread).
    pub fn cancel(&mut self) {
        if let Some(t) = &self.turn {
            t.cancel.store(true, Ordering::SeqCst);
        }
    }

    /// Accept the files shown in the confirmation dialog (recorded in the per-user trust record).
    pub fn confirm(&mut self) -> Result<(), String> {
        let Folder::Pending { pending, .. } = &self.folder else {
            return Err("nothing to confirm".into());
        };
        let files = pending.files.clone();
        self.folder = Folder::Checking;
        self.dialog_open = false;
        self.send_cmd(Command::Confirm(files));
        Ok(())
    }

    /// "Allow": add `rule` to the project's allow-list file. It applies from the next message.
    pub fn allow_rule(&mut self, rule: AllowRule) -> Result<(), String> {
        let mut list = match &self.allow {
            Some(Ok(l)) => l.clone(),
            Some(Err(e)) => return Err(format!("the allow-list file is refused: {e}")),
            None => return Err("the allow-list is not read yet".into()),
        };
        list.add(rule).map_err(|e| e.to_string())?;
        self.send_cmd(Command::SaveAllow(list));
        Ok(())
    }

    /// Remove a rule from the project's allow-list file.
    pub fn remove_rule(&mut self, rule: &AllowRule) -> Result<(), String> {
        let mut list = match &self.allow {
            Some(Ok(l)) => l.clone(),
            _ => return Err("the allow-list is not available".into()),
        };
        list.remove(rule);
        self.send_cmd(Command::SaveAllow(list));
        Ok(())
    }

    /// Replace a refused allow-list file by the first list (the read-class project tools).
    pub fn reset_allow(&mut self) {
        self.send_cmd(Command::SaveAllow(AllowList::project_initial()));
    }

    /// Total cost of the turns run here in `session`.
    pub fn session_cost(&self, session: &str) -> Option<f64> {
        self.costs.get(session).map(|v| v.iter().sum())
    }

    /// A warning when the CLI is not the version the formats were recorded with.
    pub fn version_warning(&self) -> Option<String> {
        self.cli_version
            .as_deref()
            .filter(|v| *v != PINNED_CLI_VERSION)
            .map(|v| {
                format!(
                    "Claude Code {v} is not {PINNED_CLI_VERSION}, the version the sidebar was tested with: lines it does not \
                     know are skipped"
                )
            })
    }
}

fn push_entry(feed: &mut Feed, e: TranscriptEntry) {
    match e {
        TranscriptEntry::Message { role, text, tools, .. } => {
            let (kind, label) = match role {
                Role::User => (ItemKind::User, "you> "),
                Role::Assistant => (ItemKind::Assistant, "claude> "),
                Role::ToolResult => (ItemKind::ToolResult, "[result] "),
            };
            if !text.is_empty() {
                feed.push(kind, format!("{label}{}", clip(&text, SHOWN_CHARS)));
            }
            if !tools.is_empty() {
                feed.push(ItemKind::Tool, format!("[tool] {}", tools.join(", ")));
            }
        }
        TranscriptEntry::Unrecognised { kind } => feed.push(
            ItemKind::Unrecognised,
            format!("[line not recognised: {}]", kind.as_deref().unwrap_or("not JSON")),
        ),
    }
}

// ---------------------------------------------------------------- turn thread

fn run_turn(
    claude: Claude,
    project: PathBuf,
    session: Option<String>,
    prompt: String,
    cancel: Arc<AtomicBool>,
    tx: Sender<Update>,
    wake: Wake,
) {
    let send = |m: TurnMsg| {
        let _ = tx.send(Update::Turn(m));
        wake();
    };
    let file = AllowList::file_of(&project);
    let allow = match AllowList::from_file(&file) {
        Ok(l) => l,
        Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => AllowList::project_initial(),
        Err(e) => {
            return send(TurnMsg::Refused {
                reason: e.to_string(),
                pending: None,
            })
        }
    };
    if cancel.load(Ordering::SeqCst) {
        return send(TurnMsg::Refused {
            reason: "cancelled before it started".into(),
            pending: None,
        });
    }
    let chat = match &session {
        Some(id) => claude.resume(&project, id, &allow, &prompt),
        None => claude.new_session(&project, &allow, &prompt),
    };
    let chat = match chat {
        Ok(c) => c,
        Err(Error::FolderNotConfirmed { pending, .. }) => {
            return send(TurnMsg::Refused {
                reason: "this folder's Claude Code files are not confirmed".into(),
                pending: Some(pending),
            })
        }
        Err(e) => {
            return send(TurnMsg::Refused {
                reason: e.to_string(),
                pending: None,
            })
        }
    };
    send(TurnMsg::Started {
        session_id: chat.session_id.clone(),
        pid: chat.pid(),
        args: chat.args().to_vec(),
    });
    let mut cancelled = false;
    loop {
        if !cancelled && cancel.load(Ordering::SeqCst) {
            chat.cancel();
            cancelled = true;
        }
        match chat.events.recv_timeout(Duration::from_millis(20)) {
            Ok(ev) => {
                let end = matches!(ev, StreamEvent::Exited { .. });
                send(TurnMsg::Event(ev));
                if end {
                    return;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                return send(TurnMsg::Event(StreamEvent::Exited {
                    code: None,
                    stderr_tail: "the chat's event channel closed".into(),
                }))
            }
        }
    }
}

// ---------------------------------------------------------------- worker thread

struct WorkerCfg {
    claude: Claude,
    project: PathBuf,
    mcp_file: PathBuf,
    rcfd_mcp: PathBuf,
    period: Duration,
    switches: Switches,
    tx: Sender<Update>,
    wake: Wake,
}

#[derive(Default)]
struct Mirror {
    target: Option<(String, PathBuf)>,
    paused: bool,
    offset: Option<u64>,
    missing_said: bool,
}

fn worker_loop(cfg: WorkerCfg, rx: Receiver<Command>) {
    let send = |u: Update| {
        let _ = cfg.tx.send(u);
        (cfg.wake)();
    };
    // the app-owned MCP config (checked to exist and lie outside the project before every spawn)
    send(Update::McpConfig(write_mcp_config(&cfg)));
    let (list, on_disk) = read_allow(&cfg.project);
    send(Update::Allow(list, on_disk));
    send(Update::Folder(check(&cfg)));
    let discovering = Arc::new(AtomicBool::new(false));
    let mut mirror = Mirror::default();
    let mut next_discover = Instant::now();
    let mut next_mirror = Instant::now();
    loop {
        let now = Instant::now();
        if now >= next_discover {
            next_discover = now + cfg.period;
            discover(&cfg, &discovering);
        }
        if now >= next_mirror {
            next_mirror = now + MIRROR_PERIOD;
            mirror_tick(&cfg, &mut mirror, &send);
        }
        let wait = next_discover.min(next_mirror).saturating_duration_since(Instant::now());
        match rx.recv_timeout(wait) {
            Ok(Command::Discover) => next_discover = Instant::now(),
            Ok(Command::Mirror { target, paused }) => {
                mirror = Mirror {
                    target,
                    paused,
                    ..Mirror::default()
                };
                next_mirror = Instant::now();
            }
            Ok(Command::PauseMirror(p)) => {
                mirror.paused = p;
                next_mirror = Instant::now();
            }
            Ok(Command::CheckFolder) => send(Update::Folder(check(&cfg))),
            Ok(Command::Confirm(files)) => {
                if let Some(store) = cfg.claude.trust_store.as_ref().map(TrustStore::new) {
                    if let Err(e) = store.confirm(&cfg.project, &files) {
                        send(Update::Notice(format!("chat: the confirmation was not recorded: {e}")));
                    }
                }
                send(Update::Folder(check(&cfg)));
            }
            Ok(Command::SaveAllow(list)) => {
                match cfg.claude.trust_store.as_ref().map(TrustStore::new) {
                    Some(store) => match store.save_allow_list(&cfg.project, &list) {
                        Ok(()) => send(Update::Notice(
                            "chat: allow-list saved; it applies from the next message".into(),
                        )),
                        Err(e) => send(Update::Notice(format!("chat: the allow-list was not saved: {e}"))),
                    },
                    None => send(Update::Notice(
                        "chat: no trust record, the allow-list is not saved".into(),
                    )),
                }
                let (list, on_disk) = read_allow(&cfg.project);
                send(Update::Allow(list, on_disk));
                send(Update::Folder(check(&cfg)));
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn write_mcp_config(cfg: &WorkerCfg) -> Result<PathBuf, String> {
    let dir = cfg.mcp_file.parent().ok_or("the MCP config has no folder")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let text = pinned_mcp_config(&cfg.rcfd_mcp, &cfg.project);
    rcfd_project::atomic::atomic_write(&cfg.mcp_file, text.as_bytes())
        .map_err(|e| format!("{}: {e}", cfg.mcp_file.display()))?;
    Ok(cfg.mcp_file.clone())
}

/// The project's allow-list (the first list when the file does not exist) and whether the file exists.
fn read_allow(project: &Path) -> (Result<AllowList, String>, bool) {
    match AllowList::from_file(&AllowList::file_of(project)) {
        Ok(l) => (Ok(l), true),
        Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            (Ok(AllowList::project_initial()), false)
        }
        Err(e) => (Err(e.to_string()), true),
    }
}

fn check(cfg: &WorkerCfg) -> Folder {
    match cfg.claude.check_folder(&cfg.project) {
        Ok(TrustState::Confirmed) => Folder::Confirmed,
        Ok(TrustState::Pending(p)) => {
            let previews = p
                .files
                .iter()
                .filter(|f| f.kind == ControlKind::File)
                .map(|f| preview(&cfg.project, f))
                .collect();
            Folder::Pending { pending: p, previews }
        }
        Err(e) => Folder::Error(e.to_string()),
    }
}

fn preview(project: &Path, f: &ControlFile) -> Preview {
    let bytes = std::fs::read(project.join(&f.path)).unwrap_or_default();
    let cut = bytes.len().min(PREVIEW_BYTES);
    Preview {
        path: f.path.clone(),
        text: String::from_utf8_lossy(&bytes[..cut]).into_owned(),
        truncated: bytes.len() > PREVIEW_BYTES,
        matches: rcfd_project::records::hash_bytes(&bytes) == f.hash,
    }
}

fn discover(cfg: &WorkerCfg, busy: &Arc<AtomicBool>) {
    if busy.swap(true, Ordering::SeqCst) {
        return; // the last one is still listing
    }
    let (claude, project, tx, wake, busy) = (
        cfg.claude.clone(),
        cfg.project.clone(),
        cfg.tx.clone(),
        cfg.wake.clone(),
        busy.clone(),
    );
    let _ = std::thread::Builder::new()
        .name("rcfd-studio-chat-discover".into())
        .spawn(move || {
            let d = claude.discover(&project);
            let _ = tx.send(Update::Sessions(d));
            busy.store(false, Ordering::SeqCst);
            wake();
        });
}

fn mirror_tick(cfg: &WorkerCfg, m: &mut Mirror, send: &impl Fn(Update)) {
    let Some((id, path)) = m.target.clone() else { return };
    if m.paused {
        return;
    }
    if cfg.switches.whole_file_mirror {
        // E7 negative control: the whole file each time, the unterminated last line included
        if let Ok(bytes) = std::fs::read(&path) {
            let entries: Vec<TranscriptEntry> = bytes
                .split(|&b| b == b'\n')
                .filter(|l| !l.is_empty())
                .filter_map(parse_entry)
                .collect();
            send(Update::Transcript {
                session: id,
                reset: true,
                entries,
            });
        }
        return;
    }
    let r = match m.offset {
        None => first_read(&path).map(|(entries, off)| {
            m.offset = Some(off);
            (true, entries)
        }),
        Some(ref mut off) => read_new(&path, off).map(|e| (false, e)),
    };
    match r {
        Ok((reset, entries)) => {
            m.missing_said = false;
            if reset || !entries.is_empty() {
                send(Update::Transcript {
                    session: id,
                    reset,
                    entries,
                });
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {} // a new session's transcript appears later
        Err(e) => {
            if !m.missing_said {
                m.missing_said = true;
                send(Update::Notice(format!("chat: transcript {}: {e}", path.display())));
            }
        }
    }
}

/// The last [`TAIL`] entries of a transcript and the offset after the last complete line read, so that `read_new`
/// continues exactly there. Reads from the end, doubling the window from 256 KiB (as `transcript::tail`).
fn first_read(path: &Path) -> std::io::Result<(Vec<TranscriptEntry>, u64)> {
    let mut f = std::fs::File::open(path)?;
    let len = f.metadata()?.len();
    let mut window: u64 = 256 * 1024;
    loop {
        let start = len.saturating_sub(window);
        // a line boundary at or after `start`
        let boundary = if start == 0 {
            0
        } else {
            f.seek(SeekFrom::Start(start - 1))?;
            let mut buf = Vec::new();
            f.by_ref().take(len - start + 1).read_to_end(&mut buf)?;
            match buf.iter().position(|&b| b == b'\n') {
                Some(i) => start - 1 + i as u64 + 1,
                None => len, // no complete line in the window
            }
        };
        let mut off = boundary;
        let entries = read_new(path, &mut off)?;
        if entries.len() >= TAIL || start == 0 {
            let skip = entries.len().saturating_sub(TAIL);
            return Ok((entries.into_iter().skip(skip).collect(), off));
        }
        window *= 2;
    }
}
