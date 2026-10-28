//! Drawing of the Claude Code sidebar (ADR 0134): the Chat panel and the folder confirmation dialog. State and decisions
//! are in [`crate::chat`]; this module only draws and forwards clicks.

use crate::chat::{ChatModel, Folder, ItemKind, TurnState, FLOOR_CAVEAT};
use crate::studio::Studio;
use egui::{Color32, RichText};
use rcfd_claude::allow::AllowRule;
use rcfd_claude::trust::ControlKind;

const ERR: Color32 = Color32::from_rgb(200, 60, 60);
const WARN: Color32 = Color32::from_rgb(200, 140, 0);
const USER: Color32 = Color32::from_rgb(70, 130, 200);
const GREY: Color32 = Color32::from_rgb(128, 128, 128);

/// Denied calls listed (the last ones of the turn).
const SHOWN_DENIALS: usize = 10;

/// Height kept below the transcript for the turn, approvals and input.
const BELOW_FEED: f32 = 230.0;

/// The Chat panel.
pub fn chat_panel(ui: &mut egui::Ui, st: &mut Studio) {
    let mut msgs: Vec<String> = Vec::new();
    draw(ui, &mut st.chat, &mut msgs);
    for m in msgs {
        st.say(m);
    }
}

fn draw(ui: &mut egui::Ui, c: &mut ChatModel, msgs: &mut Vec<String>) {
    if let Err(e) = &c.bridge {
        ui.colored_label(WARN, e.as_str());
        return;
    }
    if c.project.is_none() {
        ui.weak("Open a project to chat with Claude Code about it.");
        return;
    }
    header(ui, c);
    sessions(ui, c, msgs);
    ui.separator();
    feed(ui, c);
    turn_status(ui, c);
    approvals(ui, c, msgs);
    input(ui, c, msgs);
}

fn header(ui: &mut egui::Ui, c: &mut ChatModel) {
    ui.horizontal_wrapped(|ui| {
        match &c.cli_version {
            Some(v) => ui.weak(format!("Claude Code {v}")),
            None => ui.weak("Claude Code"),
        };
        if let Some(Err(e)) = &c.mcp {
            ui.colored_label(ERR, format!("MCP config: {e}"));
        }
    });
    if let Some(w) = c.version_warning() {
        ui.colored_label(WARN, w);
    }
    let (mut review, mut again) = (false, false);
    match &c.folder {
        Folder::Checking => {
            ui.weak("checking the project folder's Claude Code files...");
        }
        Folder::Confirmed => {}
        Folder::Pending { pending, .. } => {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    WARN,
                    format!(
                        "{} Claude Code file(s) in this folder are not confirmed ({} changed); no chat starts until you confirm them.",
                        pending.files.len(),
                        pending.changed.len()
                    ),
                );
                review = ui.button("Review...").clicked();
            });
        }
        Folder::Error(e) => {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(ERR, format!("folder check failed: {e}"));
                again = ui.small_button("check again").clicked();
            });
        }
    }
    if review {
        c.dialog_open = true;
    }
    if again {
        c.recheck_folder();
    }
}

fn sessions(ui: &mut egui::Ui, c: &mut ChatModel, msgs: &mut Vec<String>) {
    let rows = c.session_rows();
    let live = c.turn.as_ref().is_some_and(|t| t.state.is_live());
    let mut pick: Option<String> = None;
    let mut new = false;
    egui::CollapsingHeader::new(format!("Sessions ({})", rows.len()))
        .id_salt("chat-sessions")
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                new = ui
                    .add_enabled(!live, egui::Button::new("New chat"))
                    .on_hover_text("the next message starts a new session")
                    .clicked();
                if ui.small_button("refresh").clicked() {
                    c.refresh();
                }
            });
            for w in &c.warnings {
                ui.colored_label(WARN, w);
            }
            egui::ScrollArea::vertical()
                .id_salt("chat-session-list")
                .max_height(110.0)
                .show(ui, |ui| {
                    for r in &rows {
                        let sel = c.selected.as_deref() == Some(r.id.as_str());
                        let text = if r.input_enabled {
                            RichText::new(&r.label)
                        } else {
                            RichText::new(&r.label).color(GREY)
                        };
                        let resp = ui.add_enabled(!live || sel, egui::Button::selectable(sel, text));
                        let resp = match &r.reason {
                            Some(why) => resp.on_hover_text(why),
                            None => resp.on_hover_text(&r.id),
                        };
                        if resp.clicked() {
                            pick = Some(r.id.clone());
                        }
                    }
                    if rows.is_empty() {
                        ui.weak(if c.discoveries == 0 {
                            "listing sessions..."
                        } else {
                            "no Claude Code session in this folder yet"
                        });
                    }
                });
        });
    if new {
        if let Err(e) = c.new_chat() {
            msgs.push(e);
        }
    }
    if let Some(id) = pick {
        if let Err(e) = c.select(&id) {
            msgs.push(e);
        }
    }
}

fn kind_colour(k: ItemKind) -> Option<Color32> {
    match k {
        ItemKind::User => Some(USER),
        ItemKind::Assistant => None,
        ItemKind::Tool | ItemKind::ToolResult | ItemKind::Unrecognised => Some(GREY),
        ItemKind::Denied => Some(WARN),
        ItemKind::Note => Some(ERR),
    }
}

fn feed(ui: &mut egui::Ui, c: &mut ChatModel) {
    let h = (ui.available_height() - BELOW_FEED).max(100.0);
    match &c.selected {
        Some(id) => ui.weak(format!("session {id}")),
        None => ui.weak("new chat: the next message starts a new session"),
    };
    if c.switches.whole_text {
        // E10 negative control: the whole feed laid out as one text every frame
        let text = c.feed.whole_text();
        egui::ScrollArea::vertical()
            .id_salt("chat-feed")
            .max_height(h)
            .stick_to_bottom(true)
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.label(RichText::new(text).monospace());
            });
        return;
    }
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let glyph = ui.ctx().fonts_mut(|f| f.glyph_width(&font, '0')).max(1.0);
    let row_h = ui.text_style_height(&egui::TextStyle::Monospace);
    let width = ((ui.available_width() - 24.0) / glyph).floor().max(8.0) as usize;
    c.feed.layout(width);
    let feed = &c.feed;
    let n = feed.rows().len();
    egui::ScrollArea::vertical()
        .id_salt("chat-feed")
        .max_height(h)
        .stick_to_bottom(true)
        .auto_shrink([false; 2])
        .show_rows(ui, row_h, n, |ui, range| {
            for i in range {
                let r = feed.rows()[i];
                let mut t = RichText::new(feed.row_text(&r)).monospace();
                if let Some(col) = kind_colour(feed.row_kind(&r)) {
                    t = t.color(col);
                }
                ui.add(egui::Label::new(t).wrap_mode(egui::TextWrapMode::Extend));
            }
        });
}

fn turn_status(ui: &mut egui::Ui, c: &mut ChatModel) {
    let Some(t) = &c.turn else { return };
    let mut cancel = false;
    ui.horizontal_wrapped(|ui| {
        match &t.state {
            TurnState::Starting => {
                ui.label("starting...");
            }
            TurnState::Running { pid } => {
                ui.label(format!("running (pid {pid})"));
                cancel = ui.button("Cancel").clicked();
            }
            TurnState::Finished { code } => {
                let ok = t.result.as_ref().is_some_and(|r| !r.0);
                match (ok, code) {
                    (true, _) => ui.label("turn finished"),
                    (false, c) => ui.colored_label(WARN, format!("turn ended (exit code {c:?})")),
                };
            }
            TurnState::Aborted(why) => {
                ui.colored_label(ERR, format!("aborted: {why}"));
            }
            TurnState::Refused(why) => {
                ui.colored_label(ERR, format!("not sent: {why}"));
            }
            TurnState::Cancelled => {
                ui.colored_label(WARN, "cancelled");
            }
        }
        ui.label(format!("cost {}", t.cost_label()));
        if let Some(total) = t.session_id.as_deref().and_then(|s| c.session_cost(s)) {
            ui.weak(format!("session total here ${total:.6}"));
        }
    });
    // for the by-hand checks (ADR 0134 H1-H5): what was started, and what the CLI reported
    egui::CollapsingHeader::new("turn details")
        .id_salt("chat-turn-details")
        .show(ui, |ui| {
            if let Some((mode, version, model)) = &t.init {
                ui.monospace(format!(
                    "system/init: permissionMode {}, version {}, model {}",
                    mode.as_deref().unwrap_or("-"),
                    version.as_deref().unwrap_or("-"),
                    model.as_deref().unwrap_or("-")
                ));
            }
            if !t.args.is_empty() {
                ui.monospace(format!("claude {}", t.args.join(" ")));
            }
            if !t.stderr.trim().is_empty() {
                ui.monospace(format!("stderr: {}", t.stderr.trim()));
            }
        });
    if cancel {
        c.cancel();
    }
}

fn approvals(ui: &mut egui::Ui, c: &mut ChatModel, msgs: &mut Vec<String>) {
    let (total, denials) = c
        .turn
        .as_ref()
        .map(|t| {
            let from = t.denials.len().saturating_sub(SHOWN_DENIALS);
            (t.denials.len(), t.denials[from..].to_vec())
        })
        .unwrap_or_default();
    let mut allow: Option<AllowRule> = None;
    let mut remove: Option<AllowRule> = None;
    let mut reset = false;
    if !denials.is_empty() {
        egui::CollapsingHeader::new(format!("Denied tool calls ({total})"))
            .id_salt("chat-denials")
            .default_open(true)
            .show(ui, |ui| {
                if total > denials.len() {
                    ui.weak(format!("the last {} are shown", denials.len()));
                }
                for d in &denials {
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(WARN, RichText::new(&d.tool).strong());
                        ui.monospace(&d.input);
                    });
                    match &d.suggestion {
                        Some(rule) => {
                            ui.horizontal_wrapped(|ui| {
                                ui.monospace(rule.to_flag_value());
                                if ui
                                    .add(egui::Button::new("Allow").small())
                                    .on_hover_text("adds exactly this rule to the project's allow-list; it applies from the next message")
                                    .clicked()
                                {
                                    allow = Some(rule.clone());
                                }
                            });
                        }
                        None => {
                            ui.weak("no rule can allow this call (it reaches a control file or the tool takes no rule)");
                        }
                    }
                }
            });
    }
    egui::CollapsingHeader::new("Allow-list")
        .id_salt("chat-allow")
        .show(ui, |ui| {
            ui.weak(FLOOR_CAVEAT);
            match &c.allow {
                None => {
                    ui.weak("reading...");
                }
                Some(Err(e)) => {
                    ui.colored_label(ERR, e.as_str());
                    reset = ui.button("Reset to the first list").clicked();
                }
                Some(Ok(list)) => {
                    if !c.allow_on_disk {
                        ui.weak("first list (written to .rcfd/claude-allow.json when you change it)");
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("chat-allow-rules")
                        .max_height(100.0)
                        .show(ui, |ui| {
                            for r in list.rules() {
                                ui.horizontal(|ui| {
                                    ui.monospace(r.to_flag_value());
                                    if ui.small_button("remove").clicked() {
                                        remove = Some(r.clone());
                                    }
                                });
                            }
                        });
                }
            }
        });
    for r in [allow.map(|r| c.allow_rule(r)), remove.map(|r| c.remove_rule(&r))]
        .into_iter()
        .flatten()
    {
        if let Err(e) = r {
            msgs.push(e);
        }
    }
    if reset {
        c.reset_allow();
    }
}

fn input(ui: &mut egui::Ui, c: &mut ChatModel, msgs: &mut Vec<String>) {
    let ok = c.can_send();
    ui.horizontal(|ui| {
        ui.label("model");
        ui.add(
            egui::TextEdit::singleline(&mut c.model_name)
                .hint_text("your default")
                .desired_width(90.0),
        );
    });
    ui.add(
        egui::TextEdit::multiline(&mut c.input)
            .hint_text(match &ok {
                Ok(()) => "message (Ctrl+Enter sends)",
                Err(_) => "",
            })
            .desired_rows(3)
            .desired_width(f32::INFINITY),
    );
    let mut send = false;
    ui.horizontal_wrapped(|ui| {
        send = ui.add_enabled(ok.is_ok(), egui::Button::new("Send")).clicked()
            || (ok.is_ok() && ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Enter)));
        if let Err(why) = &ok {
            ui.weak(why.as_str());
        }
    });
    if send {
        let text = c.input.trim().to_string();
        match c.send(&text) {
            Ok(()) => c.input.clear(),
            Err(e) => msgs.push(e),
        }
    }
}

/// The folder confirmation dialog: shown while the model asks for it and the folder is pending. Nothing behind it can
/// be clicked until it is answered.
pub fn confirm_dialog(ctx: &egui::Context, st: &mut Studio) {
    let c = &mut st.chat;
    if !c.dialog_open {
        return;
    }
    let Folder::Pending { pending, previews } = &c.folder else {
        return;
    };
    let project = c.project.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
    let (mut accept, mut close) = (false, false);
    egui::Modal::new(egui::Id::new("rcfd-studio-chat-confirm")).show(ctx, |ui| {
        ui.set_max_width(720.0);
        ui.heading("Confirm Claude Code files in this project");
        ui.label(format!(
            "Claude Code reads these files when the sidebar starts a chat in {project}. They can start programs (MCP \
             servers, hooks) and change what Claude does. Confirm only if you trust where the folder came from. Your \
             confirmation is kept in your own Studio settings, not in the project, and any later change asks again."
        ));
        egui::ScrollArea::vertical()
            .id_salt("chat-confirm-body")
            .max_height(440.0)
            .show(ui, |ui| {
                egui::Grid::new("chat-confirm-files")
                    .striped(true)
                    .num_columns(4)
                    .show(ui, |ui| {
                        ui.strong("file");
                        ui.strong("size");
                        ui.strong("hash");
                        ui.strong("");
                        ui.end_row();
                        for f in &pending.files {
                            let link = if f.kind == ControlKind::Link { " (link)" } else { "" };
                            ui.monospace(format!("{}{link}", f.path));
                            ui.monospace(format!("{} B", f.len));
                            ui.monospace(&f.hash);
                            if pending.changed.contains(&f.path) {
                                ui.colored_label(WARN, "new or changed");
                            } else {
                                ui.label("");
                            }
                            ui.end_row();
                        }
                    });
                let removed: Vec<&String> = pending
                    .changed
                    .iter()
                    .filter(|p| !pending.files.iter().any(|f| &f.path == *p))
                    .collect();
                if !removed.is_empty() {
                    ui.label("Removed since your last confirmation:");
                    for r in removed {
                        ui.monospace(r);
                    }
                }
                match &pending.allow_rules {
                    None => {}
                    Some(Ok(rules)) => {
                        ui.separator();
                        ui.label(format!(
                            "Rules in .rcfd/claude-allow.json ({}): Claude may use these without asking",
                            rules.len()
                        ));
                        for r in rules {
                            ui.monospace(r);
                        }
                    }
                    Some(Err(e)) => {
                        ui.separator();
                        ui.colored_label(ERR, format!("The allow-list file is refused and is not used: {e}"));
                    }
                }
                ui.separator();
                for p in previews {
                    let title = if p.matches {
                        p.path.clone()
                    } else {
                        format!("{} (changed since the list was made)", p.path)
                    };
                    egui::CollapsingHeader::new(title)
                        .id_salt(("chat-preview", &p.path))
                        .show(ui, |ui| {
                            ui.monospace(&p.text);
                            if p.truncated {
                                ui.weak("(only the start of the file is shown)");
                            }
                        });
                }
            });
        ui.horizontal(|ui| {
            accept = ui.button("Confirm these files").clicked();
            close = ui.button("Not now").clicked();
        });
    });
    if accept {
        if let Err(e) = st.chat.confirm() {
            st.say(e);
        }
    } else if close {
        st.chat.dialog_open = false;
    }
}
