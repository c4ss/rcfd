//! The Viewport panel's Mesh / Run toggle (ADR 0144; the maintainer's answer to ADR 0138 question 1). The Viewport shows
//! either the mesh of the open case (the preview of ADR 0138) or the stored fields of the selected run (ADR 0132), and
//! the toggle says which:
//!
//! * opening a case (a click in the Project panel, [`Studio::open_case`], or a case the case starter makes and opens)
//!   switches to Mesh;
//! * selecting a run (a click in the Runs panel, [`Studio::select_run`], also of the run already selected; the run a
//!   Run click starts, ADR 0141, which is selected the same way, unless another case was selected after the click,
//!   ADR 0148; a selection written to [`Studio::selected_run`] directly, seen at the next [`Studio::drain`]) switches
//!   to Run;
//! * a click on the toggle switches to that side, unless it has nothing to show (no case opened, no run selected): then
//!   it is disabled and says why.
//!
//! The selected run stays selected on the Mesh side: the plots, the log and the run strip keep following it. The
//! viewport is synchronised at once on every switch, so no frame shows one side's picture under the other side's
//! label. No drawing here ([`crate::viewport_ui`] draws the toggle).
//!
//! ADR 0149 (the maintainer's answer to ADR 0148 question 1) links the two selections so that the toggle, Run and the
//! Project panel name one case: a click on a case ([`Studio::open_case`]) also selects that case's newest run (none:
//! the Run side is disabled and says that the case has no run yet) and stays on the Mesh side; a click on a run in the
//! Runs panel ([`Studio::open_run`]) also selects its case and goes to the Run side (a run whose case file is gone keeps
//! the selected case, and the Run side's caption says so); a run that Run queued after another case was selected is
//! not selected.

use crate::studio::Studio;
use std::collections::BTreeMap;

/// What the Viewport panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    /// The mesh of the open case (ADR 0138).
    #[default]
    Mesh,
    /// The stored fields of the selected run (ADR 0132).
    Run,
}

impl Side {
    /// Both sides, in the toggle's order.
    pub const BOTH: [Side; 2] = [Side::Mesh, Side::Run];

    /// The toggle's label.
    pub fn label(self) -> &'static str {
        match self {
            Side::Mesh => "Mesh",
            Side::Run => "Run",
        }
    }

    /// The toggle's hover text (when it can be picked).
    pub fn hover(self) -> &'static str {
        match self {
            Side::Mesh => "the mesh of the open case, built from the case (the solver is not run)",
            Side::Run => "the stored fields of the selected run",
        }
    }
}

/// The Run side's readout for a run whose case is no longer among the project's case files (ADR 0149).
pub fn gone_case_text(case: &str) -> String {
    format!("case {case} (not in the project any more: deleted or renamed; the selected case is unchanged)")
}

/// Why the Mesh side cannot be shown (the toggle's disabled hover text).
pub const NO_CASE: &str = "No case is open: click a case in the Project panel to see its mesh.";
/// Why the Run side cannot be shown (the toggle's disabled hover text).
pub const NO_RUN: &str = "No run is selected: select a run in the Runs panel, or press Run (F5) to start one.";

/// Negative-control switches (tests only; all off in the app).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Switches {
    /// ADR 0138's rule: the viewport shows the selected run whatever the toggle says, so the mesh cannot be reached
    /// while a run is selected.
    pub run_always_wins: bool,
    /// The case starter's route before ADR 0144: a case it makes and opens in the editor is not handed to the preview
    /// (and the view does not switch).
    pub starter_skips_preview: bool,
    /// The case starter's route before ADR 0147: a case is written alone, without the data files of its example.
    pub starter_no_companions: bool,
    /// The route before ADR 0148 (the maintainer's answer to ADR 0147 question 3): every file under `cases/` is listed
    /// and offered as a case (no "Data files" list).
    pub data_files_as_cases: bool,
}

/// The toggle's state, a field of [`Studio`].
#[derive(Debug, Clone, Default)]
pub struct Toggle {
    /// The side shown.
    pub side: Side,
    /// Negative-control switches.
    pub switches: Switches,
    /// Where the two buttons were drawn last (by label), so tests can click them.
    pub rects: BTreeMap<&'static str, egui::Rect>,
    /// Whether each button was enabled when drawn last (by label).
    pub enabled: BTreeMap<&'static str, bool>,
    seen_run: Option<String>,
}

impl Toggle {
    /// Why `side` has nothing to show (`None`: it can be shown), from whether a case is open and a run selected.
    pub fn unavailable(side: Side, has_case: bool, has_run: bool) -> Option<&'static str> {
        match side {
            Side::Mesh if !has_case => Some(NO_CASE),
            Side::Run if !has_run => Some(NO_RUN),
            _ => None,
        }
    }

    /// The run the viewport shows: the selected run on the Run side, none on the Mesh side.
    pub fn run_shown<'a>(&self, selected: Option<&'a str>) -> Option<&'a str> {
        if self.switches.run_always_wins || self.side == Side::Run {
            selected
        } else {
            None
        }
    }

    /// Forget the selection (the project closed): back to the Mesh side.
    pub fn close(&mut self) {
        self.side = Side::Mesh;
        self.seen_run = None;
    }
}

impl Studio {
    /// Why `side` has nothing to show (`None`: it can be shown).
    pub fn view_unavailable(&self, side: Side) -> Option<&'static str> {
        Toggle::unavailable(side, self.preview.case.is_some(), self.selected_run.is_some())
    }

    /// The words of [`Studio::view_unavailable`] as the toggle says them: with a case selected that has no run, the Run
    /// side says so for that case (ADR 0149: the Run side shows the selected case's runs), else [`NO_CASE`] or
    /// [`NO_RUN`].
    pub fn view_refusal(&self, side: Side) -> Option<String> {
        let why = self.view_unavailable(side)?;
        match (&self.selected_case, side) {
            (Some(c), Side::Run) if self.linked() => Some(crate::studio::no_run_of(c)),
            _ => Some(why.to_string()),
        }
    }

    /// The toggle: show the open case's mesh or the selected run's fields. Refused, with the reason
    /// ([`Studio::view_refusal`]), when that side has nothing to show; the selection is not changed either way.
    pub fn set_view(&mut self, side: Side) -> Result<(), String> {
        if let Some(why) = self.view_refusal(side) {
            return Err(why);
        }
        self.view.side = side;
        self.sync_viewport();
        Ok(())
    }

    /// What the Run side shows, in words (beside the toggle; the Mesh side's panel names its case itself).
    pub fn view_caption(&self) -> Option<String> {
        match (self.view.side, self.viewport.run.as_deref()) {
            (Side::Run, Some(id)) => Some(format!("stored fields of run {id}")),
            _ => None,
        }
    }

    /// The Run side's readout beside its caption (ADR 0148): the run's case, the size of its domain and its cells, from
    /// the run's own stored mesh (not from the selected case); `None` on the Mesh side. A run whose case is no longer
    /// among the project's case files says so (ADR 0149: the selected case did not change for it).
    pub fn run_readout(&self) -> Option<String> {
        if self.viewport.switches.no_scale {
            return None;
        }
        let id = self.viewport.run.as_deref()?;
        let case = self.run_case(id);
        let mut parts = Vec::new();
        if let Some(c) = case {
            if self.linked() && self.browser().is_some() && !self.case_listed(&c) {
                parts.push(gone_case_text(&c));
            } else {
                parts.push(format!("case {c}"));
            }
        }
        if let Some(d) = self.viewport.domain() {
            parts.push(d);
        }
        if let Some(n) = self.viewport.cells {
            parts.push(format!("{} cells", crate::preview::with_commas(n as u64)));
        }
        (!parts.is_empty()).then(|| parts.join(", "))
    }

    /// A case was asked for (a Project panel click, or a case the starter made and opened): the preview builds it and
    /// the Viewport switches to its mesh. A selection made before does not switch the view back later.
    pub(crate) fn show_case_mesh(&mut self, rel: &str) {
        self.preview.select(rel);
        self.view.side = Side::Mesh;
        self.view.seen_run = self.selected_run.clone();
        self.sync_viewport();
    }

    /// A run was selected without a switch of the side shown (ADR 0148: a run that Run queued after another case was
    /// selected): the selection is taken as seen, the side stays.
    pub(crate) fn keep_view_side(&mut self) {
        self.view.seen_run = self.selected_run.clone();
        self.sync_viewport();
    }

    /// A run was selected (also the one already selected): the Viewport shows it.
    pub(crate) fn show_run_fields(&mut self) {
        self.view.side = Side::Run;
        self.view.seen_run = self.selected_run.clone();
        self.sync_viewport();
    }

    /// The viewport follows the run of the shown side ([`Toggle::run_shown`]); a selection written to
    /// [`Studio::selected_run`] directly since the last call switches to the Run side first.
    pub(crate) fn sync_viewport(&mut self) {
        if self.selected_run != self.view.seen_run {
            if self.selected_run.is_some() {
                self.view.side = Side::Run;
            }
            self.view.seen_run = self.selected_run.clone();
        }
        let root = self.browser().map(|b| b.root.clone());
        let run = self.view.run_shown(self.selected_run.as_deref()).map(String::from);
        let wake = self.wake.clone();
        self.viewport
            .sync(root.as_deref(), run.as_deref(), crate::poll::PERIOD, &wake);
    }
}
