//! The case starter (ADR 0139): new case files of a project made from the example cases shipped in the repository's
//! `cases/` folder. The examples are compiled into the Studio (D2), so the list is the same from any folder and in any
//! install; reference and validation data are never embedded.
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
use rcfd_api::case::Case;
use rcfd_project::Project;
use serde_json::Value;
use std::path::Path;
use std::sync::OnceLock;

/// A data file an example reads by its file name from its own folder, embedded with it and written next to a case made
/// from it (ADR 0147).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Companion {
    /// Path under the repository's `cases/` folder, in the example's folder (`channel/profile_inlet.csv`).
    pub source: &'static str,
    /// The file's text, embedded at build time; written unchanged.
    pub text: &'static str,
}

impl Companion {
    /// The file name (`profile_inlet.csv`): the name the example's text gives, and the file written next to the case.
    pub fn name(&self) -> &'static str {
        self.source.rsplit('/').next().unwrap_or(self.source)
    }

    /// The bytes written.
    pub fn bytes(&self) -> &'static [u8] {
        self.text.as_bytes()
    }
}

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
    /// Data files it reads from its own folder, written next to a new case made from it (ADR 0147).
    pub companions: &'static [Companion],
}

macro_rules! example {
    ($source:literal, $dim:literal, $cells:literal, $shows:literal) => {
        example!($source, $dim, $cells, $shows, [])
    };
    ($source:literal, $dim:literal, $cells:literal, $shows:literal, [$($companion:literal),*]) => {
        Example {
            source: $source,
            text: include_str!(concat!("../../../cases/", $source)),
            shows: $shows,
            dim: $dim,
            cells: $cells,
            companions: &[$(Companion {
                source: $companion,
                text: include_str!(concat!("../../../cases/", $companion)),
            }),*],
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
        "channel/profile_inlet.toml",
        "2D",
        480,
        "Laminar channel whose inlet velocity is read from a CSV profile, copied next to the case as profile_inlet.csv; fully developed plane Poiseuille flow",
        ["channel/profile_inlet.csv"]
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
/// reference data and are left out by rule ([`is_reference`]); a [`Companion`] of an offered example is written with it
/// and is not offered by itself (ADR 0147).
pub const EXCLUDED: &[(&str, &str)] = &[
    ("backward_step/case.json", DESCRIPTOR),
    ("cavity/case.json", DESCRIPTOR),
    ("channel/case.json", DESCRIPTOR),
    ("flat_plate/case.json", DESCRIPTOR),
    ("heated_cavity/case.json", DESCRIPTOR),
    ("naca0012/case.json", DESCRIPTOR),
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

    /// The example's text parsed as a case, without files: a table it reads from a [`Companion`] gets that companion's
    /// text inline (`data` in place of `file`), so no file is read; `None` when it does not parse.
    pub fn parse(&self) -> Option<Case> {
        if self.companions.is_empty() {
            return Case::parse_toml(self.text).ok();
        }
        let mut v: Value = toml::from_str(self.text).ok()?;
        if let Some(tables) = v.get_mut("tables").and_then(Value::as_object_mut) {
            for t in tables.values_mut().filter_map(Value::as_object_mut) {
                let file = t.get("file").and_then(Value::as_str).unwrap_or_default().to_string();
                if let Some(c) = self.companions.iter().find(|c| c.name() == file) {
                    t.remove("file");
                    t.insert("data".into(), Value::String(c.text.to_string()));
                }
            }
        }
        Case::from_json_in(&v, Path::new(".")).ok()
    }

    /// Describe it: the title and the run length are read from the text (the defaults of keys it does not set come from
    /// the schema); [`descriptions`] keeps the result for the dialog.
    pub fn describe(&self) -> Description {
        let (title, run) = match self.parse().ok_or(()) {
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
    create_with(project, rel, bytes, &[]).map(|_| ())
}

/// [`create`] with the data files of the example (its [`Companion`]s, `(cases/<file name>, bytes)`), written next to the
/// case first (ADR 0147). A data file that is there already with the same bytes is used as it is (a second case made from
/// the same example shares it); one with other bytes refuses the creation before anything is written, because the new
/// case would read it; none is ever replaced. When the case file cannot be written, the data files this call wrote are
/// removed again. Returns each data file with whether this call wrote it.
pub fn create_with(
    project: &Project,
    rel: &str,
    bytes: &[u8],
    companions: &[(String, Vec<u8>)],
) -> Result<Vec<(String, bool)>, String> {
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
    if companions.is_empty() {
        rcfd_project::atomic::write_new(&path, bytes).map_err(|e| e.to_string())?;
        return Ok(Vec::new());
    }
    // checked before anything is written: the case file is new, every data file new or equal
    if std::fs::symlink_metadata(&path).is_ok() {
        return Err(format!("{}: already exists", path.display()));
    }
    let mut plan = Vec::new();
    for (crel, cbytes) in companions {
        let plain = crel
            .strip_prefix("cases/")
            .is_some_and(|f| !f.is_empty() && !f.contains(['/', '\\']));
        if !plain || crel.eq_ignore_ascii_case(rel) {
            return Err(format!(
                "{crel}: a data file of a new case is written directly in cases/, under another name than the case"
            ));
        }
        let cpath = project.resolve(crel).map_err(|e| e.to_string())?;
        match std::fs::read(&cpath) {
            Ok(old) if old == *cbytes => plan.push((crel.clone(), cpath, false)),
            Ok(_) => {
                return Err(format!(
                    "{crel} exists with other content: {rel} would read its table from that file; rename or remove \
                     {crel}, then create the case again"
                ))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => plan.push((crel.clone(), cpath, true)),
            Err(e) => return Err(format!("{}: {e}", cpath.display())),
        }
    }
    let mut written: Vec<&Path> = Vec::new();
    let undo = |written: &[&Path]| {
        for p in written {
            let _ = std::fs::remove_file(p);
        }
    };
    for ((crel, cpath, new), (_, cbytes)) in plan.iter().zip(companions) {
        if *new {
            if let Err(e) = rcfd_project::atomic::write_new(cpath, cbytes) {
                undo(&written);
                return Err(format!("{crel}: {e}"));
            }
            written.push(cpath.as_path());
        }
    }
    if let Err(e) = rcfd_project::atomic::write_new(&path, bytes) {
        undo(&written);
        return Err(e.to_string());
    }
    Ok(plan.into_iter().map(|(crel, _, new)| (crel, new)).collect())
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
        if !busy {
            self.pending_case = Some(rel.clone());
        }
        Ok(rel)
    }
}
