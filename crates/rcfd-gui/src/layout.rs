//! Dockable panel layout: which panels exist, where they sit, show/hide/float.
//! Pure `egui_dock` state manipulation, no drawing.

use egui_dock::{DockState, NodeIndex};
use serde::{Deserialize, Serialize};

/// The application's panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Panel {
    /// Settings tree editor.
    Settings,
    /// Mesh / field viewport.
    Viewport,
    /// Residual plots and run controls.
    Monitors,
    /// Solver log.
    Log,
    /// AI assistant sidebar.
    Assistant,
}

impl Panel {
    /// All panels.
    pub const ALL: [Panel; 5] = [
        Panel::Settings,
        Panel::Viewport,
        Panel::Monitors,
        Panel::Log,
        Panel::Assistant,
    ];

    /// Tab title.
    pub fn title(self) -> &'static str {
        match self {
            Panel::Settings => "Settings",
            Panel::Viewport => "Viewport",
            Panel::Monitors => "Monitors",
            Panel::Log => "Log",
            Panel::Assistant => "Assistant",
        }
    }
}

/// The default arrangement: settings left, viewport centre, assistant right, monitors and log below.
pub fn default_layout() -> DockState<Panel> {
    let mut s = DockState::new(vec![Panel::Viewport]);
    let t = s.main_surface_mut();
    let [centre, _right] = t.split_right(NodeIndex::root(), 0.72, vec![Panel::Assistant]);
    let [centre, _left] = t.split_left(centre, 0.22, vec![Panel::Settings]);
    let [_top, _bottom] = t.split_below(centre, 0.62, vec![Panel::Monitors, Panel::Log]);
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
    while let Some(idx) = s.find_tab(&p) {
        s.remove_tab(idx);
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

/// Serialise the layout.
pub fn to_json(s: &DockState<Panel>) -> Result<String, String> {
    serde_json::to_string(s).map_err(|e| e.to_string())
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
