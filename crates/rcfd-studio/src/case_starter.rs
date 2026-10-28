//! The case starter (ADR 0139): new case files of a project made from the example cases shipped in the repository's
//! `cases/` folder. The examples are compiled into the Studio (D2), so the list is the same from any folder and in any
//! install; reference and validation data are never embedded.
//!
//! Model only, no drawing ([`crate::case_starter_ui`] draws). [`check_name`] decides whether a typed name may become
//! `cases/<name>.toml`; [`create`] is the write the project worker runs (it never replaces a file);
//! [`Studio::create_case_from_example`] ties them to the worker and the editor.

use crate::studio::Studio;
use crate::watch::Command;
use rcfd_api::case::Case;
use rcfd_project::Project;
use std::sync::OnceLock;

/// One example case shipped in `cases/` and embedded in the Studio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Example {
    /// Path under the repository's `cases/` folder (`sod/sod.toml`).
    pub source: &'static str,
    /// The file's text, embedded at build time; a new case gets these bytes unchanged.
    pub text: &'static str,
    /// What it shows (curated from `docs/tutorials/gallery.md` and the file's own comments).
    pub shows: &'static str,
    /// `2D` (the mesh has a patch of kind `Empty`) or `3D`. Curated: deriving it builds the mesh; ADR 0139 CS7 checks it.
    pub dim: &'static str,
    /// Cells of the mesh. Curated for the same reason; CS7 checks it against the mesh build.
    pub cells: u64,
}

macro_rules! example {
    ($source:literal, $dim:literal, $cells:literal, $shows:literal) => {
        Example {
            source: $source,
            text: include_str!(concat!("../../../cases/", $source)),
            shows: $shows,
            dim: $dim,
            cells: $cells,
        }
    };
}

/// The offered examples, cheapest first (by cell count): the runnable case files of `cases/` that open and validate in
/// the case editor (ADR 0139 D1). A test keeps this list, [`EXCLUDED`] and `cases/` in step (CS5).
pub const EXAMPLES: &[Example] = &[
    example!(
        "channel/kepsilon_channel.toml",
        "2D",
        56,
        "Periodic plane channel at Re_tau = 395, standard k-epsilon with wall functions, coarse on purpose; stops when the wall y+ settles"
    ),
    example!(
        "sod/sod.toml",
        "2D",
        100,
        "Shock tube with the compressible Euler solver (HLLC flux, limited reconstruction, SSP Runge-Kutta 3), one cell high"
    ),
    example!(
        "rotating_channel/rotating_channel.toml",
        "2D",
        160,
        "Laminar channel rotating about the spanwise axis; the Coriolis force is balanced by a cross-stream pressure gradient"
    ),
    example!(
        "channel/sst_channel.toml",
        "2D",
        240,
        "Wall-resolved channel at Re_tau = 395, k-omega SST on an inflated y+ = 1 mesh"
    ),
    example!(
        "channel/kepsilon_inlet.toml",
        "2D",
        288,
        "Developing channel with inlet turbulence from an intensity and a length scale, RNG k-epsilon"
    ),
    example!(
        "packed_bed/packed_bed.toml",
        "2D",
        320,
        "Air through a packed bed of spheres (Ergun porous resistance); stops at its iteration cap without converging"
    ),
    example!(
        "wedge/wedge.toml",
        "2D",
        1200,
        "Mach 2 flow over a 10 degree compression ramp; an oblique shock leaves the corner"
    ),
    example!(
        "axial_fan_cgrid/axial_fan_cgrid.toml",
        "3D",
        11536,
        "One blade passage of an axial fan on a body-fitted C-grid in a rotating frame; a laminar smoke case, not a fan prediction"
    ),
    example!(
        "axial_fan_sector/axial_fan_sector.toml",
        "3D",
        13332,
        "One blade passage of an axial fan (sector mesh, blunt blade edges) in a rotating frame; a laminar smoke case, not a fan prediction"
    ),
];

const DESCRIPTOR: &str =
    "benchmark descriptor (Reynolds numbers, meshes, reference data and tolerances of a validation case, \
     read by `rcfd cases`), not a case file: the case editor refuses its keys and `rcfd run` refuses it";

/// Files of `cases/` that are not offered, with the reason (ADR 0139 D1). Files inside a folder named `reference` are
/// reference data and are left out by rule ([`is_reference`]).
pub const EXCLUDED: &[(&str, &str)] = &[
    ("backward_step/case.json", DESCRIPTOR),
    ("cavity/case.json", DESCRIPTOR),
    ("channel/case.json", DESCRIPTOR),
    ("flat_plate/case.json", DESCRIPTOR),
    ("heated_cavity/case.json", DESCRIPTOR),
    ("naca0012/case.json", DESCRIPTOR),
    (
        "channel/profile_inlet.toml",
        "reads its inlet profile from profile_inlet.csv by a relative path, which the starter does not copy: a copy of \
         the one file does not open in the case editor (the table is looked for in the case's folder since ADR 0144)",
    ),
    (
        "channel/profile_inlet.csv",
        "input table of channel/profile_inlet.toml, not a case",
    ),
];

/// True for a path relative to `cases/` (`/` separators) inside a folder named `reference`: reference data, never offered
/// or embedded.
pub fn is_reference(source: &str) -> bool {
    let mut parts: Vec<&str> = source.split('/').collect();
    parts.pop();
    parts.contains(&"reference")
}

/// What the dialog shows of an example (ADR 0139 D6).
#[derive(Debug, Clone, PartialEq)]
pub struct Description {
    /// The file's `title` (its file stem when it has none).
    pub title: String,
    /// How long it runs, from the case: `<algorithm>, at most N iterations` or `<algorithm>, to t = T`.
    pub run: String,
    /// The line under the title: what it shows, dimension, cells and run length.
    pub line: String,
}

impl Example {
    /// File stem of the source (`sod`): the name the dialog proposes.
    pub fn stem(&self) -> &'static str {
        let file = self.source.rsplit('/').next().unwrap_or(self.source);
        file.rsplit_once('.').map_or(file, |(s, _)| s)
    }

    /// Extension of the source (`toml`); the new file keeps it.
    pub fn ext(&self) -> &'static str {
        self.source.rsplit_once('.').map_or("", |(_, e)| e)
    }

    /// The bytes a new case gets.
    pub fn bytes(&self) -> &'static [u8] {
        self.text.as_bytes()
    }

    /// Describe it: the title and the run length are read from the text (the defaults of keys it does not set come from
    /// the schema); [`descriptions`] keeps the result for the dialog.
    pub fn describe(&self) -> Description {
        let (title, run) = match Case::parse_toml(self.text) {
            Ok(c) => {
                let text = |k: &str| c.get(k).ok().and_then(|v| v.as_str().map(str::to_string));
                let title = text("title")
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or_else(|| self.stem().to_string());
                let algorithm = text("solver.algorithm").unwrap_or_default();
                let transient = match algorithm.as_str() {
                    "piso" | "pimple" => true,
                    "compressible" => text("compressible.mode").as_deref() != Some("steady"),
                    _ => false,
                };
                let run = if transient {
                    let t = c.get("solver.end_time").ok().and_then(|v| v.as_f64());
                    format!("{algorithm}, to t = {}", t.map_or("?".to_string(), |t| t.to_string()))
                } else {
                    let n = c.get("solver.max_iterations").ok().and_then(|v| v.as_u64());
                    format!(
                        "{algorithm}, at most {} iterations",
                        n.map_or("?".to_string(), |n| n.to_string())
                    )
                };
                (title, run)
            }
            Err(_) => (self.stem().to_string(), "run length unknown".to_string()),
        };
        let line = format!("{}. {}, {} cells, {}.", self.shows, self.dim, self.cells, run);
        Description { title, run, line }
    }
}

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

/// Longest accepted case name (ADR 0139 D4).
pub const MAX_NAME: usize = 64;

/// Windows device names: refused as case names in any letter case (with any extension they still name the device).
const RESERVED: [&str; 24] = [
    "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT0",
    "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn char_name(c: char) -> String {
    match c {
        ' ' => "a space".to_string(),
        c if c.is_control() => format!("U+{:04X}", c as u32),
        c => format!("'{c}'"),
    }
}

/// Check a typed case name for the new file `cases/<name>.<ext>` and return that project-relative path (ADR 0139 D4).
/// `cases`: the project's case files as the browser lists them. A refusal says which rule the name breaks.
pub fn check_name(name: &str, ext: &str, cases: &[String]) -> Result<String, String> {
    if name.is_empty() {
        return Err("a case needs a name".into());
    }
    let n = name.chars().count();
    if n > MAX_NAME {
        return Err(format!("at most {MAX_NAME} characters (this name has {n})"));
    }
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".toml") || lower.ends_with(".json") {
        return Err(format!("give the name without the extension (.{ext} is added)"));
    }
    if name.contains(['/', '\\']) {
        return Err("no path separators: the case is created directly in cases/".into());
    }
    if let Some(c) = name
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '_'))
    {
        return Err(format!(
            "{} is not allowed: use letters, digits, '-' and '_' only (no spaces or dots)",
            char_name(c)
        ));
    }
    if !name.starts_with(|c: char| c.is_ascii_alphanumeric()) {
        return Err("start the name with a letter or a digit".into());
    }
    if RESERVED.contains(&name.to_ascii_uppercase().as_str()) {
        return Err(format!("'{name}' is a reserved device name on Windows"));
    }
    // names that differ only in letter case are one file on Windows: compared without case everywhere
    for other in [ext, "toml", "json"] {
        let rel = format!("cases/{name}.{other}");
        if let Some(c) = cases.iter().find(|c| c.eq_ignore_ascii_case(&rel)) {
            return Err(format!("{c} already exists"));
        }
    }
    Ok(format!("cases/{name}.{ext}"))
}

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
    let file = rel
        .strip_prefix("cases/")
        .filter(|f| !f.contains(['/', '\\']))
        .ok_or_else(|| format!("{rel}: a new case is created directly in cases/"))?;
    let (name, ext) = file
        .rsplit_once('.')
        .filter(|(_, e)| matches!(*e, "toml" | "json"))
        .ok_or_else(|| format!("{rel}: a case file ends in .toml or .json"))?;
    check_name(name, ext, &[]).map_err(|m| format!("{rel}: {m}"))?;
    let path = project.resolve(rel).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    rcfd_project::atomic::write_new(&path, bytes).map_err(|e| e.to_string())
}

/// What the Studio does with [`crate::watch::Event::Created`] (called from `Studio::apply`): a message. When `opened`,
/// the `Loaded` event that follows opens the file in the editor, and the preview shows it on the Viewport's Mesh side
/// (ADR 0144: the file exists now; `Studio::open_case` itself would load it a second time).
pub(crate) fn on_created(st: &mut Studio, rel: &str, opened: bool) {
    if opened {
        if !st.view.switches.starter_skips_preview {
            st.show_case_mesh(rel);
        }
        st.say(format!("created {rel}"));
    } else {
        st.say(format!(
            "created {rel}; not opened, because the open case has unsaved edits or a conflict (save or resolve them, then \
             select {rel} in the Project panel)"
        ));
    }
}

impl Studio {
    /// Create `cases/<name>.<ext>` in the open project from `example`, its bytes unchanged (ADR 0139 D5). The name is
    /// checked against the browser's list ([`check_name`]); the project worker writes the file without replacing one,
    /// sends the lists that hold it and, unless the editor holds unsaved edits, a conflict or a save in flight, opens it
    /// in the editor. Returns the path asked for; the outcome arrives through [`Studio::drain`] as a message
    /// ("created ..." or the located failure).
    pub fn create_case_from_example(&mut self, example: &Example, name: &str) -> Result<String, String> {
        let rel = {
            let b = self
                .browser()
                .ok_or_else(|| "no project is open (or it is still opening)".to_string())?;
            check_name(name, example.ext(), &b.cases)?
        };
        let busy = self
            .editor
            .as_ref()
            .is_some_and(|e| e.is_dirty() || e.in_conflict() || e.saving);
        let untrack = if busy {
            None
        } else {
            self.editor.as_ref().map(|e| e.rel.clone())
        };
        let Some(p) = &self.project else {
            return Err("no project is open".into());
        };
        p.worker.send(Command::Create {
            rel: rel.clone(),
            bytes: example.bytes().to_vec(),
            open: !busy,
            untrack,
        });
        if !busy {
            self.pending_case = Some(rel.clone());
        }
        Ok(rel)
    }
}
