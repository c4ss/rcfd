//! The case starter (ADR 0139): new case files of a project made from the example cases shipped in the repository's
//! `cases/` folder. The examples are compiled in (D2; since ADR 0186 G1 by the project workspace,
//! `rcfd_workspace::examples`), so the list is the same from any folder and in any install; reference and validation data
//! are never embedded.
//!
//! Model only, no drawing ([`crate::case_starter_ui`] draws). [`check_name`] decides whether a typed name may become
//! `cases/<name>.toml`; [`create`] and [`create_with`] are the write the project worker runs (they never replace a
//! file); [`Studio::create_case_from_example`] ties them to the worker and the editor.
//!
//! An example that reads a data file by a relative path in its own folder (`channel/profile_inlet.toml` reads
//! `profile_inlet.csv`) has it as a [`Companion`]: embedded too, and written next to the new case under the same file
//! name, so the case's text stays byte-equal to the example's (ADR 0147, the maintainer's answer to ADR 0139
//! question 3 and ADR 0144 question 11).

use crate::studio::Studio;
use crate::watch::Command;
use rcfd_project::Project;
use std::sync::OnceLock;

// The examples, their data files and their descriptions moved to the project workspace (ADR 0186 G1), unchanged, so
// that the MCP server makes new cases from the same list.
pub use rcfd_workspace::examples::{is_reference, Companion, Description, Example, EXAMPLES, EXCLUDED};

/// The descriptions of [`EXAMPLES`], in order, built once: on the UI thread at the first open of the dialog (ADR 0139
/// CS13 measures the cost).
pub fn descriptions() -> &'static [Description] {
    static D: OnceLock<Vec<Description>> = OnceLock::new();
    D.get_or_init(build_descriptions)
}

/// Build the descriptions without the cache.
pub fn build_descriptions() -> Vec<Description> {
    EXAMPLES.iter().map(Example::describe).collect()
}

// The name rule and the never-replace write moved to the project workspace (ADR 0185 E6), unchanged.
pub use rcfd_workspace::cases::{check_name, MAX_NAME};

/// A free name for an example: its stem, else `<stem>-2`, `<stem>-3`, ... (the stem itself when none of the first
/// thousand is free; the dialog then shows the refusal).
pub fn suggest_name(stem: &str, ext: &str, cases: &[String]) -> String {
    if check_name(stem, ext, cases).is_ok() {
        return stem.to_string();
    }
    (2..1000)
        .map(|k| format!("{stem}-{k}"))
        .find(|n| check_name(n, ext, cases).is_ok())
        .unwrap_or_else(|| stem.to_string())
}

/// The write of a new case file, run by the project worker (ADR 0139 D5). `rel` must be `cases/<name>.toml` or
/// `.json` with a name [`check_name`] accepts (collisions are left to the write); `cases/` is created when it is
/// missing; the file is created atomically and an existing file is never replaced (`rcfd_project::atomic::write_new`,
/// a hard link that fails when the target exists). Errors are located.
pub fn create(project: &Project, rel: &str, bytes: &[u8]) -> Result<(), String> {
    create_with(project, rel, bytes, &[]).map(|_| ())
}

/// [`create`] with the data files of the example (its [`Companion`]s, `(cases/<file name>, bytes)`), written next to the
/// case first (ADR 0147). A data file that is there already with the same bytes is used as it is (a second case made from
/// the same example shares it); one with other bytes refuses the creation before anything is written, because the new
/// case would read it; none is ever replaced. When the case file cannot be written, the data files this call wrote are
/// removed again. Returns each data file with whether this call wrote it. The write of the project workspace
/// (`rcfd_workspace::cases::create_with`, ADR 0185 E6).
pub fn create_with(
    project: &Project,
    rel: &str,
    bytes: &[u8],
    companions: &[(String, Vec<u8>)],
) -> Result<Vec<(String, bool)>, String> {
    rcfd_workspace::cases::create_with(project, rel, bytes, companions)
}

/// The line under the dialog's name field: the file the case becomes and the data files written with it (ADR 0147);
/// `cases` is the browser's list of the project's files under `cases/`.
pub fn target_line(rel: &str, example: &Example, cases: &[String]) -> String {
    let mut s = format!("creates {rel}");
    let (new, there): (Vec<String>, Vec<String>) = example
        .companions
        .iter()
        .map(|c| format!("cases/{}", c.name()))
        .partition(|c| !cases.iter().any(|x| x.eq_ignore_ascii_case(c)));
    for c in &new {
        s.push_str(&format!(" and its data file {c}"));
    }
    s.push_str(&format!(" from the example {}", example.source));
    for c in &there {
        s.push_str(&format!(
            "; {c} is in the project already: used when it holds the example's bytes, else the creation is refused"
        ));
    }
    s
}

/// What the Studio does with [`crate::watch::Event::Created`] (called from `Studio::apply`): a message. When `opened`,
/// the `Loaded` event that follows opens the file in the editor, and the preview shows it on the Viewport's Mesh side
/// (ADR 0144: the file exists now; `Studio::open_case` itself would load it a second time).
pub(crate) fn on_created(st: &mut Studio, rel: &str, opened: bool, companions: &[(String, bool)]) {
    let mut m = format!("created {rel}");
    for (c, written) in companions {
        if *written {
            m.push_str(&format!(" and its data file {c}"));
        } else {
            m.push_str(&format!(
                "; its data file {c} was in the project already, with the example's bytes"
            ));
        }
    }
    if opened {
        // ADR 0149: the case left keeps its unsaved state (the starter opens over a field being typed only, ADR 0139
        // D5: not over unsaved edits or a conflict); the worker no longer watches its file
        if st.keeps_drafts() {
            if let Some(ed) = st.editor.take() {
                if crate::studio::has_unsaved(&ed) && ed.rel != rel {
                    st.parked.insert(ed.rel.clone(), ed);
                }
            }
        }
        // ADR 0148: the new case is the selected case; the editor follows it (the `Loaded` that follows opens it)
        st.selected_case = Some(rel.to_string());
        st.editor_error = None;
        st.not_a_case = None;
        st.pending_case = Some(rel.to_string());
        if !st.quick.switches.selection_from_editor {
            st.editor = None;
        }
        // ADR 0149: the Run side shows the new case's runs (normally none)
        if st.linked() {
            st.selected_run = st.newest_run_of(rel);
        }
        if !st.view.switches.starter_skips_preview {
            st.show_case_mesh(rel);
        }
        st.say(m);
    } else {
        st.say(format!(
            "{m}; not opened, because the open case has unsaved edits or a conflict (save or resolve them, then select \
             {rel} in the Project panel)"
        ));
    }
}

impl Studio {
    /// Create `cases/<name>.<ext>` in the open project from `example`, its bytes unchanged (ADR 0139 D5), with the
    /// example's data files next to it ([`create_with`], ADR 0147). The name is checked against the browser's list
    /// ([`check_name`]); the project worker writes the files without replacing one, sends the lists that hold them and,
    /// unless the editor holds unsaved edits, a conflict or a save in flight, opens the case in the editor. Returns the
    /// path asked for; the outcome arrives through [`Studio::drain`] as a message ("created ..." or the located
    /// failure).
    pub fn create_case_from_example(&mut self, example: &Example, name: &str) -> Result<String, String> {
        let rel = {
            let b = self
                .browser()
                .ok_or_else(|| "no project is open (or it is still opening)".to_string())?;
            check_name(name, example.ext(), &b.cases)?
        };
        // ADR 0149: a field being typed is kept for its case when the new case opens, unless as many cases as the Studio
        // keeps hold unsaved edits already
        let full = self.keeps_drafts() && self.parked.len() >= self.max_parked;
        let busy = self
            .editor
            .as_ref()
            .is_some_and(|e| e.is_dirty() || e.in_conflict() || e.saving || (full && e.draft.is_some()));
        let untrack = if busy {
            None
        } else {
            self.editor.as_ref().map(|e| e.rel.clone())
        };
        let Some(p) = &self.project else {
            return Err("no project is open".into());
        };
        // ADR 0147: its data files go next to it, under the names its text gives (the route before: the case alone)
        let companions = if self.view.switches.starter_no_companions {
            Vec::new()
        } else {
            example
                .companions
                .iter()
                .map(|c| (format!("cases/{}", c.name()), c.bytes().to_vec()))
                .collect()
        };
        p.worker.send(Command::Create {
            rel: rel.clone(),
            bytes: example.bytes().to_vec(),
            companions,
            open: !busy,
            untrack,
        });
        if !busy && self.quick.switches.selection_from_editor {
            // the route before ADR 0148 waited for the load from here; now `Event::Created` asks for it
            self.pending_case = Some(rel.clone());
        }
        Ok(rel)
    }
}
