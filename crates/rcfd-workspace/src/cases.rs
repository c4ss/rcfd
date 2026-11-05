//! New case files in `cases/`, never replacing a file (ADR 0139 D4, D5; data files of ADR 0147). Moved from the Studio's
//! case starter (ADR 0185 E6) with the same checks, order and messages, so that every client creates a case the same
//! way.

use rcfd_project::Project;
use std::path::Path;

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

/// Create the case file `rel` with `bytes` and the data files it reads (`(cases/<file name>, bytes)`), written next to
/// it first (ADR 0139 D5, ADR 0147). `rel` must be `cases/<name>.toml` or `.json` with a name [`check_name`] accepts
/// (collisions are left to the write); `cases/` is created when it is missing; every file is created atomically and an
/// existing file is never replaced (`rcfd_project::atomic::write_new`, a hard link that fails when the target exists).
/// A data file that is there already with the same bytes is used as it is (a second case made from the same example
/// shares it); one with other bytes refuses the creation before anything is written, because the new case would read
/// it. When the case file cannot be written, the data files this call wrote are removed again. Returns each data file
/// with whether this call wrote it. Errors are located.
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
