//! Assistant chat model and the worker thread that runs the agent (no UI code).
//!
//! The agent runs on its own thread so slow model calls never block the UI. The
//! UI sends [`Command`]s and drains [`WorkerMsg`]s into a [`ChatState`], which
//! is plain data the sidebar renders.

use rcfd_assistant::{Agent, AgentContext, AgentError, AgentEvent, Decision, PendingCall, Provider, Step};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::Duration;

/// UI to worker.
pub enum Command {
    /// Start a turn with this text and context.
    Send(String, AgentContext),
    /// Answer the pending approval.
    Decide(Decision),
    /// Switch model backend.
    SetProvider(Arc<dyn Provider>, String),
    /// Stop the current turn.
    Cancel,
    /// Exit the worker.
    Shutdown,
}

/// Worker to UI.
pub enum WorkerMsg {
    /// Agent progress.
    Event(AgentEvent),
    /// The turn finished with this final text.
    Done(String),
    /// The turn failed or was cancelled.
    Failed(String),
}

/// Handle to the worker thread.
pub struct Worker {
    tx: Sender<Command>,
    /// Messages from the worker.
    pub rx: Receiver<WorkerMsg>,
    cancel: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl Worker {
    /// Spawn a worker owning `agent`.
    pub fn spawn(agent: Agent) -> Worker {
        let (tx, cmd_rx) = channel::<Command>();
        let (msg_tx, rx) = channel::<WorkerMsg>();
        let cancel = agent.cancel_flag();
        let join = std::thread::spawn(move || worker_loop(agent, cmd_rx, msg_tx));
        Worker {
            tx,
            rx,
            cancel,
            join: Some(join),
        }
    }

    /// Send a command (ignored if the worker is gone).
    pub fn send(&self, c: Command) {
        let _ = self.tx.send(c);
    }

    /// Ask the current turn to stop at the next opportunity.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        let _ = self.tx.send(Command::Cancel);
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        let _ = self.tx.send(Command::Shutdown);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

fn worker_loop(mut agent: Agent, cmds: Receiver<Command>, out: Sender<WorkerMsg>) {
    let mut pending_text: Option<(String, AgentContext)> = None;
    loop {
        let (text, ctx) = match pending_text.take() {
            Some(t) => t,
            None => match cmds.recv() {
                Ok(Command::Send(t, c)) => (t, c),
                Ok(Command::SetProvider(p, m)) => {
                    agent.set_provider(p, m);
                    continue;
                }
                Ok(Command::Shutdown) | Err(_) => return,
                Ok(_) => continue,
            },
        };
        agent.set_context(ctx);
        if let Err(e) = agent.submit(&text) {
            let _ = out.send(WorkerMsg::Failed(e.to_string()));
            continue;
        }
        let mut sink = |e: AgentEvent| {
            let _ = out.send(WorkerMsg::Event(e));
        };
        'turn: loop {
            match agent.step(&mut sink) {
                Ok(Step::Done(t)) => {
                    let _ = out.send(WorkerMsg::Done(t));
                    break 'turn;
                }
                Ok(Step::NeedApproval(_)) => {
                    // Wait for the user's decision; poll so a cancel or shutdown is noticed.
                    loop {
                        match cmds.recv_timeout(Duration::from_millis(100)) {
                            Ok(Command::Decide(d)) => {
                                let _ = agent.resolve(d, &mut sink);
                                break;
                            }
                            Ok(Command::Cancel) => {
                                agent.cancel_flag().store(true, Ordering::SeqCst);
                                let _ = agent.resolve(Decision::Deny("cancelled".into()), &mut sink);
                                break;
                            }
                            Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
                            Ok(_) | Err(RecvTimeoutError::Timeout) => {}
                        }
                    }
                }
                Err(AgentError::Cancelled) => {
                    let _ = out.send(WorkerMsg::Failed("cancelled".into()));
                    break 'turn;
                }
                Err(e) => {
                    let _ = out.send(WorkerMsg::Failed(e.to_string()));
                    break 'turn;
                }
            }
        }
        // Drop commands that arrived during the turn and are meaningless now (stale cancels).
        while let Ok(c) = cmds.try_recv() {
            match c {
                Command::Send(t, c) => {
                    pending_text = Some((t, c));
                    break;
                }
                Command::SetProvider(p, m) => agent.set_provider(p, m),
                Command::Shutdown => return,
                _ => {}
            }
        }
    }
}

/// State of a tool card.
#[derive(Debug, Clone, PartialEq)]
pub enum CardState {
    /// Waiting for approve/deny.
    Pending,
    /// Ran successfully.
    Done,
    /// Ran and reported an error.
    Failed,
    /// The user (or read-only mode) refused it.
    Denied,
}

/// One transcript entry.
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    /// What the user typed.
    User(String),
    /// Assistant text (grows while streaming).
    Assistant(String),
    /// A tool call.
    Tool {
        /// Call id.
        id: String,
        /// Tool name.
        name: String,
        /// Arguments.
        args: Value,
        /// Card state.
        state: CardState,
        /// Result text once available.
        result: Option<String>,
        /// PNG (base64) produced by the tool.
        image: Option<String>,
        /// True if the tool changes state (approval card).
        mutating: bool,
    },
    /// System notice or error.
    Notice(String),
}

/// Transcript and turn state rendered by the sidebar.
#[derive(Debug, Default)]
pub struct ChatState {
    /// Entries, oldest first.
    pub entries: Vec<Entry>,
    /// A turn is in progress.
    pub busy: bool,
    /// Call waiting for approval.
    pub pending: Option<PendingCall>,
    /// Text box content.
    pub input: String,
    streaming: bool,
}

impl ChatState {
    /// Record a user message and mark the turn busy.
    pub fn begin_turn(&mut self, text: &str) {
        self.entries.push(Entry::User(text.to_string()));
        self.busy = true;
        self.streaming = false;
    }

    /// Fold a worker message into the state.
    pub fn apply(&mut self, m: WorkerMsg) {
        match m {
            WorkerMsg::Event(AgentEvent::Text(t)) => {
                if self.streaming {
                    if let Some(Entry::Assistant(s)) = self.entries.last_mut() {
                        s.push_str(&t);
                        return;
                    }
                }
                self.entries.push(Entry::Assistant(t));
                self.streaming = true;
            }
            WorkerMsg::Event(AgentEvent::ApprovalRequested(p)) => {
                self.streaming = false;
                self.entries.push(Entry::Tool {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    args: p.args.clone(),
                    state: CardState::Pending,
                    result: None,
                    image: None,
                    mutating: true,
                });
                self.pending = Some(p);
            }
            WorkerMsg::Event(AgentEvent::ToolResult {
                id,
                name,
                args,
                output,
                denied,
            }) => {
                self.streaming = false;
                let state = if denied {
                    CardState::Denied
                } else if output.is_error {
                    if output.text.contains("permission denied") {
                        CardState::Denied
                    } else {
                        CardState::Failed
                    }
                } else {
                    CardState::Done
                };
                if self.pending.as_ref().is_some_and(|p| p.id == id) {
                    self.pending = None;
                }
                let existing = self.entries.iter_mut().rev().find_map(|e| match e {
                    Entry::Tool {
                        id: eid,
                        state,
                        result,
                        image,
                        ..
                    } if *eid == id => Some((state, result, image)),
                    _ => None,
                });
                match existing {
                    Some((s, r, i)) => {
                        *s = state;
                        *r = Some(output.text);
                        *i = output.image_png_base64;
                    }
                    None => self.entries.push(Entry::Tool {
                        id,
                        name,
                        args,
                        state,
                        result: Some(output.text),
                        image: output.image_png_base64,
                        mutating: false,
                    }),
                }
            }
            WorkerMsg::Done(t) => {
                // Streamed text is already in the transcript; add it only if nothing streamed.
                let has_text = matches!(self.entries.last(), Some(Entry::Assistant(_)));
                if !has_text && !t.is_empty() {
                    self.entries.push(Entry::Assistant(t));
                }
                self.finish();
            }
            WorkerMsg::Failed(e) => {
                self.entries.push(Entry::Notice(format!("assistant stopped: {e}")));
                self.finish();
            }
        }
    }

    fn finish(&mut self) {
        self.busy = false;
        self.streaming = false;
        self.pending = None;
    }

    /// Add a notice line.
    pub fn notice(&mut self, text: impl Into<String>) {
        self.entries.push(Entry::Notice(text.into()));
    }
}
