//! Dockable panel layout: which panels exist, where they sit, show/hide/float.
//! Pure `egui_dock` state manipulation, no drawing.
//!
//! Copied from `crates/rcfd-gui/src/layout.rs` at main 15b00ef (last changed in f71f6e2) by ADR 0131 W48-9 (a). Changed: the
//! Studio's panels and default arrangement, and egui_dock 0.21's `TabPath`. The copy goes away with `rcfd-gui`.

use egui_dock::{DockState, NodeIndex};
use serde::{Deserialize, Serialize};

/// The Studio's panels. Viewport, Plots, Log and Chat get their content in 48C, 48D and 48E (ADRs 0132-0134).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Panel {
    /// Cases, meshes, designs and skipped run folders of the open project.
    Browser,
    /// Schema-driven editor of the selected case file.
    CaseEditor,
    /// Run list with state, step, time and metrics.
    Runs,
    /// 3D view (ADR 0132).
    Viewport,
    /// Residual and monitor plots (ADR 0133).
    Plots,
    /// Solver log and run events (ADR 0133).
    Log,
    /// Claude Code chat (ADR 0134).
    Chat,
    /// Metrics table and report link of the selected run.
    Outputs,
}

impl Panel {
    /// All panels.
    pub const ALL: [Panel; 8] = [
        Panel::Browser,
        Panel::CaseEditor,
        Panel::Runs,
        Panel::Viewport,
        Panel::Plots,
        Panel::Log,
        Panel::Chat,
        Panel::Outputs,
    ];

    /// Tab title.
    pub fn title(self) -> &'static str {
        match self {
            Panel::Browser => "Project",
            Panel::CaseEditor => "Case editor",
            Panel::Runs => "Runs",
            Panel::Viewport => "Viewport",
            Panel::Plots => "Plots",
            Panel::Log => "Log",
            Panel::Chat => "Chat",
            Panel::Outputs => "Outputs",
        }
    }
}

/// The default arrangement: project browser and runs left, viewport centre, case editor and chat right, plots, log and
/// outputs below.
pub fn default_layout() -> DockState<Panel> {
    let mut s = DockState::new(vec![Panel::Viewport]);
    let t = s.main_surface_mut();
    let [centre, _right] = t.split_right(NodeIndex::root(), 0.70, vec![Panel::CaseEditor, Panel::Chat]);
    let [centre, left] = t.split_left(centre, 0.26, vec![Panel::Browser]);
    let [_top, _bottom] = t.split_below(left, 0.5, vec![Panel::Runs]);
    let [_top, _bottom] = t.split_below(centre, 0.66, vec![Panel::Plots, Panel::Log, Panel::Outputs]);
    s
}

/// Is the panel present (docked or floating)?
pub fn is_open(s: &DockState<Panel>, p: Panel) -> bool {
    s.find_tab(&p).is_some()
}

/// How many instances of `p` exist (should be 0 or 1).
pub fn count(s: &DockState<Panel>, p: Panel) -> usize {
    s.iter_all_tabs().filter(|(_, t)| **t == p).count()
}

/// Hide the panel if it is open.
pub fn hide(s: &mut DockState<Panel>, p: Panel) {
    while let Some(path) = s.find_tab(&p) {
        s.remove_tab(path);
    }
}

/// Show the panel if hidden (docked into the focused area).
pub fn show(s: &mut DockState<Panel>, p: Panel) {
    if !is_open(s, p) {
        s.push_to_focused_leaf(p);
    }
}

/// Toggle visibility.
pub fn toggle(s: &mut DockState<Panel>, p: Panel) {
    if is_open(s, p) {
        hide(s, p);
    } else {
        show(s, p);
    }
}

/// Move the panel into its own floating window.
pub fn float(s: &mut DockState<Panel>, p: Panel) {
    hide(s, p);
    s.add_window(vec![p]);
}

/// Serialise the layout. Positions that are not laid out yet (NaN or infinite, written as `null`) are written as 0, as
/// [`from_json`] reads them, so a saved layout reads back to the same text (Studio change to the copied module).
pub fn to_json(s: &DockState<Panel>) -> Result<String, String> {
    let mut v = serde_json::to_value(s).map_err(|e| e.to_string())?;
    repair_positions(&mut v);
    serde_json::to_string(&v).map_err(|e| e.to_string())
}

/// Restore a layout. Rejects layouts that are empty, contain a panel twice, or fail to parse;
/// panels missing from a restored layout are simply hidden (the user hid them).
pub fn from_json(text: &str) -> Result<DockState<Panel>, String> {
    let mut v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    repair_positions(&mut v);
    let s: DockState<Panel> = serde_json::from_value(v).map_err(|e| e.to_string())?;
    if s.iter_all_tabs().next().is_none() {
        return Err("layout has no panels".into());
    }
    if Panel::ALL.iter().any(|p| count(&s, *p) > 1) {
        return Err("layout lists a panel twice".into());
    }
    Ok(s)
}

/// Unlaid-out panels carry NaN or infinite positions, which JSON writes as `null` and cannot read
/// back as numbers. Replace those with 0; the dock recomputes every rectangle on the next frame.
fn repair_positions(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(m) => {
            let is_pos = m.len() == 2 && m.contains_key("x") && m.contains_key("y");
            for (_, c) in m.iter_mut() {
                if is_pos && c.is_null() {
                    *c = serde_json::json!(0.0);
                } else {
                    repair_positions(c);
                }
            }
        }
        serde_json::Value::Array(a) => a.iter_mut().for_each(repair_positions),
        _ => {}
    }
}
