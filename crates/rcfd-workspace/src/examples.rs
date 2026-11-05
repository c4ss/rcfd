//! The example cases shipped in the repository's `cases/` folder, compiled in (ADR 0139 D2), from which a client makes a
//! new case of a project: the Studio's case starter and the MCP tool `new_case` (ADR 0173 W3, ADR 0186 G1). Moved
//! unchanged from the Studio's case starter, so the list is the same for every client, from any folder and in any
//! install; reference and validation data are never embedded.
//!
//! An example that reads a data file by a relative path in its own folder (`channel/profile_inlet.toml` reads
//! `profile_inlet.csv`) has it as a [`Companion`]: embedded too, and written next to the new case under the same file
//! name, so the case's text stays byte-equal to the example's (ADR 0147, the maintainer's answer to ADR 0139
//! question 3 and ADR 0144 question 11).

use rcfd_api::case::Case;
use serde_json::Value;
use std::path::Path;

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

/// One example case shipped in `cases/` and compiled in.
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
    /// How long it runs, from the case: `<algorithm>, at most N iterations` or `<algorithm>, to simulated time T s`
    /// (ADR 0149; `to t = T` before).
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
    /// the schema); the Studio's `case_starter::descriptions` keeps the result for its dialog. A transient example runs `to simulated time T s`
    /// (ADR 0149: the words of the Run page, ADR 0148 decision 6).
    pub fn describe(&self) -> Description {
        self.describe_with(false)
    }

    /// [`Example::describe`]; with `bare_time`, the words before ADR 0149 (`to t = T`).
    pub fn describe_with(&self, bare_time: bool) -> Description {
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
                    let t = t.map_or("?".to_string(), |t| t.to_string());
                    if bare_time {
                        format!("{algorithm}, to t = {t}")
                    } else {
                        format!("{algorithm}, to simulated time {t} s")
                    }
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
