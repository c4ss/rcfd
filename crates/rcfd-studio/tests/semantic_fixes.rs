//! Checks of the Studio fixes that came with ADR 0229's branch (asked for by the maintainer on 2026-11-11, not
//! registered): an edit made while a save is in flight stays unsaved; transcript lines read before a turn's pause go in
//! front of that turn's items; a replaced turn's messages do not reach the turn that followed it.

mod common;

use common::CAVITY;
use rcfd_claude::stream::StreamEvent;
use rcfd_claude::transcript::{Role, TranscriptEntry};
use rcfd_studio::case_edit::CaseEditor;
use rcfd_studio::chat::{ChatModel, TurnMsg, TurnState, Update};
use rcfd_studio::Studio;
use serde_json::json;

/// The editor's `saved` is the settings the written bytes hold: an edit made between the save's bytes and its answer
/// leaves the editor dirty. Control: without that edit the same save leaves it clean.
#[test]
fn an_edit_made_while_a_save_is_in_flight_stays_unsaved() {
    let st = Studio::in_memory();
    let dirty_after = |edit_in_flight: bool| {
        let mut ed = CaseEditor::from_bytes(st.svc.clone(), "cases/a.toml", CAVITY.as_bytes()).unwrap();
        ed.set_value("solver.max_iterations", json!(123)).unwrap();
        let bytes = ed.to_bytes().unwrap();
        ed.begin_save();
        if edit_in_flight {
            ed.set_value("solver.max_iterations", json!(456)).unwrap();
        }
        ed.saved(rcfd_project::records::hash_bytes(&bytes));
        (ed.is_dirty(), ed.saving)
    };
    let (with_edit, saving) = dirty_after(true);
    let (control, _) = dirty_after(false);
    eprintln!("save: dirty after a save with an edit in flight {with_edit}; control (no edit in flight) {control}");
    assert!(with_edit, "the edit made while the save was in flight reads as saved");
    assert!(!saving);
    assert!(!control, "control: a save without a later edit left the editor dirty");
}

fn message(role: Role, text: &str) -> TranscriptEntry {
    TranscriptEntry::Message {
        role,
        text: text.into(),
        tools: Vec::new(),
        timestamp: None,
    }
}

fn texts(chat: &ChatModel) -> Vec<String> {
    chat.feed.items().iter().map(|i| i.text.clone()).collect()
}

/// A running turn of session S with one streamed text item.
fn running_turn() -> ChatModel {
    let mut chat = ChatModel::default();
    chat.apply(Update::Turn(TurnMsg::Started {
        session_id: "S".into(),
        pid: 1,
        args: Vec::new(),
    }));
    chat.apply(Update::Turn(TurnMsg::Event(StreamEvent::TextDelta("hello".into()))));
    chat
}

/// The session's history, read before the worker saw the turn's pause, arrives while the turn streams: it goes in front
/// of the turn's items, and the next streamed text is appended to the turn's own item, not to a history item.
#[test]
fn history_that_arrives_during_a_turn_goes_in_front_of_the_turns_items() {
    let mut chat = running_turn();
    assert_eq!(chat.selected.as_deref(), Some("S"));
    chat.apply(Update::Transcript {
        session: "S".into(),
        reset: true,
        entries: vec![
            message(Role::User, "earlier question"),
            message(Role::Assistant, "earlier answer"),
        ],
    });
    chat.apply(Update::Turn(TurnMsg::Event(StreamEvent::TextDelta(" world".into()))));
    let got = texts(&chat);
    eprintln!("feed after history arrived during the turn: {got:?}");
    assert_eq!(
        got,
        ["you> earlier question", "claude> earlier answer", "claude> hello world"]
    );
    // the turn ends: its transcript lines replace its streamed items, the history stays
    chat.apply(Update::Turn(TurnMsg::Event(StreamEvent::Exited {
        code: Some(0),
        stderr_tail: String::new(),
    })));
    chat.apply(Update::Transcript {
        session: "S".into(),
        reset: false,
        entries: vec![message(Role::Assistant, "hello world")],
    });
    let got = texts(&chat);
    eprintln!("feed after the turn's transcript lines: {got:?}");
    assert_eq!(
        got,
        ["you> earlier question", "claude> earlier answer", "claude> hello world"]
    );
}

/// What a turn thread sends carries its turn's number: a message of another number (a replaced turn's exit) changes
/// nothing. Control: the same message without a number is applied.
#[test]
fn a_replaced_turns_exit_does_not_reach_the_current_turn() {
    let exit = || {
        TurnMsg::Event(StreamEvent::Exited {
            code: Some(1),
            stderr_tail: "old".into(),
        })
    };
    let mut chat = running_turn();
    chat.apply(Update::TurnOf(41, exit()));
    let state = chat.turn.as_ref().map(|t| t.state.clone());
    let mut control = running_turn();
    control.apply(Update::Turn(exit()));
    let control_state = control.turn.as_ref().map(|t| t.state.clone());
    eprintln!("turn state after another turn's exit {state:?}; control (unnumbered) {control_state:?}");
    assert_eq!(state, Some(TurnState::Running { pid: 1 }));
    assert_eq!(control_state, Some(TurnState::Finished { code: Some(1) }));
}
