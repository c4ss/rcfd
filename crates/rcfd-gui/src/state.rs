//! Application state shared by all panels (no drawing code).

use crate::assistant_setup::AssistantSetup;
use crate::chat::{CardState, ChatState, Command, Entry, Worker, WorkerMsg};
use crate::controller::Controller;
use crate::theme::Theme;
use rcfd_assistant::{Agent, AgentContext, AgentEvent, ApprovalMode, Decision, Provider};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Which file prompt is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prompt {
    /// Open a case file.
    Open,
    /// Save the case as.
    SaveAs,
}

/// No-model actions on the assistant tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickAction {
    /// `assess_mesh`.
    AssessMesh,
    /// `validate_case`.
    ValidateCase,
    /// `suggest_boundary_conditions`.
    SuggestBcs,
    /// `explain_residuals`.
    ExplainResiduals,
}

impl QuickAction {
    /// All actions.
    pub const ALL: [QuickAction; 4] = [
        QuickAction::AssessMesh,
        QuickAction::ValidateCase,
        QuickAction::SuggestBcs,
        QuickAction::ExplainResiduals,
    ];

    /// Button label.
    pub fn label(self) -> &'static str {
        match self {
            QuickAction::AssessMesh => "Assess mesh",
            QuickAction::ValidateCase => "Validate case",
            QuickAction::SuggestBcs => "Suggest BCs",
            QuickAction::ExplainResiduals => "Explain residuals",
        }
    }

    /// Tool name.
    pub fn tool(self) -> &'static str {
        match self {
            QuickAction::AssessMesh => "assess_mesh",
            QuickAction::ValidateCase => "validate_case",
            QuickAction::SuggestBcs => "suggest_boundary_conditions",
            QuickAction::ExplainResiduals => "explain_residuals",
        }
    }
}

/// Everything the panels read and change.
pub struct AppState {
    /// Case, run and results.
    pub ctrl: Controller,
    /// Theme.
    pub theme: Theme,
    /// Assistant transcript.
    pub chat: ChatState,
    /// Assistant configuration.
    pub setup: AssistantSetup,
    /// Agent thread, once a provider is installed.
    pub worker: Option<Worker>,
    /// Name of the model the worker uses.
    pub active_model: Option<String>,
    /// Settings editor filter text.
    pub settings_filter: String,
    /// Text buffers of fields being edited.
    pub buffers: HashMap<String, String>,
    /// Text boxes for adding named entries, by collection.
    pub new_names: HashMap<String, String>,
    /// Open file prompt.
    pub prompt: Option<Prompt>,
    /// Path typed into the prompt.
    pub prompt_path: String,
    /// Plot residuals on a log axis.
    pub log_plot: bool,
    /// Request to close the application.
    pub quit: bool,
    /// Preferences path.
    pub prefs_path: Option<PathBuf>,
    last_poll: Instant,
    last_sync: Instant,
}

/// How often the solver is polled while the app is idle.
pub const POLL_EVERY: Duration = Duration::from_millis(250);

impl AppState {
    /// New state over a controller and assistant setup.
    pub fn new(ctrl: Controller, setup: AssistantSetup, theme: Theme) -> AppState {
        AppState {
            ctrl,
            theme,
            chat: ChatState::default(),
            setup,
            worker: None,
            active_model: None,
            settings_filter: String::new(),
            buffers: HashMap::new(),
            new_names: HashMap::new(),
            prompt: None,
            prompt_path: String::new(),
            log_plot: true,
            quit: false,
            prefs_path: None,
            last_poll: Instant::now(),
            last_sync: Instant::now(),
        }
    }

    /// Context shown as chips and sent to the model.
    pub fn context(&self) -> AgentContext {
        let has = self.ctrl.session.is_some();
        AgentContext {
            case_title: has.then(|| self.ctrl.title()),
            session: self.ctrl.session.clone(),
            run_state: has.then(|| self.ctrl.status.state.clone()),
            selection: self.ctrl.selected.clone(),
        }
    }

    /// Use `provider`/`model` for the assistant (starts the agent thread on first use).
    pub fn install_provider(&mut self, provider: Arc<dyn Provider>, model: &str) {
        match &self.worker {
            Some(w) => w.send(Command::SetProvider(provider, model.to_string())),
            None => {
                let agent = Agent::new(provider, self.ctrl.toolbox.clone(), model);
                self.worker = Some(Worker::spawn(agent));
            }
        }
        self.active_model = Some(model.to_string());
    }

    /// Build the selected provider from the setup and install it.
    pub fn connect_selected(&mut self) {
        match self.setup.build(None) {
            Ok((p, m)) => {
                self.install_provider(p, &m);
                self.setup.status = Some(format!("using {m}"));
            }
            Err(e) => self.setup.status = Some(e.to_string()),
        }
    }

    /// Approval mode.
    pub fn mode(&self) -> ApprovalMode {
        self.ctrl.toolbox.mode()
    }

    /// Change the approval mode (drops outstanding grants).
    pub fn set_mode(&mut self, m: ApprovalMode) {
        self.ctrl.toolbox.set_mode(m);
    }

    /// Can a message be sent now?
    pub fn can_send(&self) -> bool {
        self.worker.is_some() && !self.chat.busy && !self.chat.input.trim().is_empty()
    }

    /// Send the typed message.
    pub fn send_message(&mut self) {
        if !self.can_send() {
            return;
        }
        let text = std::mem::take(&mut self.chat.input);
        self.chat.begin_turn(text.trim());
        let ctx = self.context();
        if let Some(w) = &self.worker {
            w.send(Command::Send(text.trim().to_string(), ctx));
        }
    }

    /// Approve the pending tool call.
    pub fn approve(&mut self) {
        if let Some(w) = &self.worker {
            w.send(Command::Decide(Decision::Approve));
        }
    }

    /// Deny the pending tool call.
    pub fn deny(&mut self) {
        if let Some(w) = &self.worker {
            w.send(Command::Decide(Decision::Deny("denied by the user".into())));
        }
    }

    /// Stop the current assistant turn.
    pub fn cancel_turn(&mut self) {
        if let Some(w) = &self.worker {
            w.cancel();
        }
    }

    /// Drain worker messages into the transcript. Returns true if anything arrived.
    pub fn pump_worker(&mut self) -> bool {
        let mut msgs = vec![];
        if let Some(w) = &self.worker {
            while let Ok(m) = w.rx.try_recv() {
                msgs.push(m);
            }
        }
        let any = !msgs.is_empty();
        let mut touched = false;
        for m in msgs {
            if matches!(&m, WorkerMsg::Event(AgentEvent::ToolResult { .. })) {
                touched = true;
            }
            self.chat.apply(m);
        }
        if touched {
            self.ctrl.sync_with_assistant();
        }
        any
    }

    /// Run a setup tool directly, with no model involved.
    pub fn quick_action(&mut self, a: QuickAction) {
        let out = self.ctrl.toolbox.execute(a.tool(), &json!({}));
        let pretty = serde_json::from_str::<Value>(&out.text)
            .ok()
            .and_then(|v| serde_json::to_string_pretty(&v).ok())
            .unwrap_or(out.text.clone());
        self.chat.entries.push(Entry::Tool {
            id: format!("quick-{}", self.chat.entries.len()),
            name: a.tool().to_string(),
            args: json!({}),
            state: if out.is_error {
                CardState::Failed
            } else {
                CardState::Done
            },
            result: Some(pretty),
            image: None,
            mutating: false,
        });
    }

    /// Periodic work: poll the solver and follow the assistant. Returns true if the UI should repaint soon.
    pub fn tick(&mut self) -> bool {
        let mut active = self.pump_worker();
        let running = self.ctrl.status.is_running();
        if self.last_poll.elapsed() >= POLL_EVERY {
            self.ctrl.poll(false);
            self.last_poll = Instant::now();
        }
        if self.chat.busy && self.last_sync.elapsed() >= Duration::from_millis(500) {
            self.ctrl.sync_with_assistant();
            self.last_sync = Instant::now();
        }
        active |= running || self.chat.busy;
        active
    }
}
