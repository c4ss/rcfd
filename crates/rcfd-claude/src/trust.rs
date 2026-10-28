//! The files of a project folder that change what `claude -p` runs or reads (ADR 0134 W48-10 (b)): `.mcp.json`,
//! everything under `.claude/` and `.claude-plugin/`, and `CLAUDE.md`, at any depth, plus the app's own allow-list file
//! `.rcfd/claude-allow.json` (Amendment 2, option (c)). The Studio lists them with their hashes, and the allow file's
//! rules, before the first chat in a folder and after any change.
//!
//! The confirmed lists live in a [`TrustStore`]: one file in the Studio's per-user config folder, keyed by the
//! canonical project path, never inside the project folder (Amendment 2, option (a)); a record shipped inside a
//! received folder therefore confirms nothing. `Claude::new_session` and `Claude::resume` refuse a folder whose scan is
//! not empty and does not equal its record ([`crate::Error::FolderNotConfirmed`]).
//!
//! Known limits: the hash is `fnv64` (not collision-resistant; open in ADR 0134); the store is read, changed and
//! replaced without a lock, so two Studio processes confirming at the same moment can lose one confirmation (which
//! then asks again); `CLAUDE.md` files in folders above the project are not scanned.

use crate::allow::{AllowList, ALLOW_FILE};
use crate::Error;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Names that change Claude Code's behaviour, lowercase. Matched per path component, ASCII case and trailing dots and
/// spaces ignored (as Windows resolves names).
const NAMES: [&str; 4] = [".mcp.json", "claude.md", ".claude", ".claude-plugin"];

/// What a listed entry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ControlKind {
    /// A regular file; its contents are hashed.
    File,
    /// A symbolic link or junction (never followed); the text of its target is hashed. Every link in the folder is
    /// listed, because the scan cannot see control files behind it and Claude Code may follow it.
    Link,
}

/// One entry of a scan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlFile {
    /// Project-relative path with `/` separators.
    pub path: String,
    /// File or link.
    pub kind: ControlKind,
    /// `rcfd_project::records::hash_bytes` of the contents (file) or of the target text (link).
    pub hash: String,
    /// Size in bytes of what was hashed.
    pub len: u64,
}

fn fold(name: &str) -> String {
    name.trim_end_matches(['.', ' ']).to_ascii_lowercase()
}

fn is_control_name(name: &str) -> bool {
    NAMES.contains(&fold(name).as_str())
}

/// Is the project-relative `rel` the allow-list file ([`ALLOW_FILE`]), ignoring ASCII case and trailing dots and spaces?
pub fn is_allow_file(rel: &str) -> bool {
    rel.split('/').map(fold).eq(ALLOW_FILE.split('/').map(String::from))
}

/// Every control file and link under `project`, sorted by path. Links are not followed. Any directory or control
/// file that cannot be read fails the whole scan, so a caller never mistakes an unreadable folder for a clean one.
pub fn scan(project: &Path) -> Result<Vec<ControlFile>, Error> {
    let io = |path: &Path| {
        let path = path.to_path_buf();
        move |source| Error::Io { path, source }
    };
    let mut out = Vec::new();
    let mut stack = vec![(project.to_path_buf(), String::new(), false)];
    while let Some((dir, rel, under_control)) = stack.pop() {
        for entry in std::fs::read_dir(&dir).map_err(io(&dir))? {
            let entry = entry.map_err(io(&dir))?;
            let p = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let r = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            let ty = entry.file_type().map_err(io(&p))?; // of the entry itself, links not followed
            let control = under_control || is_control_name(&name);
            if ty.is_symlink() {
                let target = std::fs::read_link(&p).map_err(io(&p))?;
                let t = target.to_string_lossy();
                out.push(ControlFile {
                    path: r,
                    kind: ControlKind::Link,
                    hash: rcfd_project::records::hash_bytes(t.as_bytes()),
                    len: t.len() as u64,
                });
            } else if ty.is_dir() {
                stack.push((p, r, control));
            } else if control || is_allow_file(&r) {
                let bytes = rcfd_project::atomic::read(&p).map_err(|e| crate::allow::project_err(&p, e))?;
                out.push(ControlFile {
                    path: r,
                    kind: ControlKind::File,
                    hash: rcfd_project::records::hash_bytes(&bytes),
                    len: bytes.len() as u64,
                });
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// What the user must confirm before a chat starts in a folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    /// The current scan: every control file and link, as it is to be shown and then passed to [`TrustStore::confirm`].
    pub files: Vec<ControlFile>,
    /// Paths added, changed or removed since the confirmed record (all paths when there is no record), sorted.
    pub changed: Vec<String>,
    /// The allow-list file's rules as flag values, or why the file is refused; `None` when there is no allow file.
    pub allow_rules: Option<Result<Vec<String>, String>>,
}

/// Whether a chat may start in a folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrustState {
    /// The folder holds no control file, or exactly the confirmed ones.
    Confirmed,
    /// The user has to confirm these files first.
    Pending(Pending),
}

/// Format version of the trust record.
const STORE_VERSION: u64 = 1;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoreFile {
    version: u64,
    projects: BTreeMap<String, Vec<ControlFile>>,
}

/// The per-user record of confirmed folders: one JSON file in the Studio's config folder (never inside a project),
/// `{"version": 1, "projects": {<canonical project path>: [entries]}}`, replaced atomically.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustStore {
    path: PathBuf,
}

impl TrustStore {
    /// A record kept in `path` (the file need not exist yet).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The record file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The key of `project`: its canonical path, without the Windows verbatim prefix where that is safe.
    pub fn key(project: &Path) -> Result<String, Error> {
        rcfd_project::paths::canonicalize(project)
            .map(|p| p.to_string_lossy().into_owned())
            .map_err(|source| Error::Io {
                path: project.to_path_buf(),
                source,
            })
    }

    /// Refuses a record file that lies inside `project` (it would travel with the folder).
    fn outside(&self, project: &Path) -> Result<(), Error> {
        use crate::allow::{inside, lexical};
        let mut bases = vec![lexical(project)];
        if let Ok(c) = rcfd_project::paths::canonicalize(project) {
            bases.push(lexical(&c));
        }
        let mut forms = vec![lexical(&self.path)];
        if let (Some(dir), Some(name)) = (self.path.parent(), self.path.file_name()) {
            if let Ok(c) = rcfd_project::paths::canonicalize(dir) {
                forms.push(lexical(&c.join(name)));
            }
        }
        if bases.iter().any(|b| forms.iter().any(|f| inside(b, f))) {
            return Err(Error::Io {
                path: self.path.clone(),
                source: std::io::Error::other("the trust record must not be inside the project folder"),
            });
        }
        Ok(())
    }

    fn load(&self) -> Result<StoreFile, Error> {
        let bytes = match rcfd_project::atomic::read(&self.path) {
            Ok(b) => b,
            Err(rcfd_project::ProjectError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(StoreFile {
                    version: STORE_VERSION,
                    projects: BTreeMap::new(),
                })
            }
            Err(e) => return Err(crate::allow::project_err(&self.path, e)),
        };
        let bad = |msg: String| Error::Io {
            path: self.path.clone(),
            source: std::io::Error::other(msg),
        };
        let s: StoreFile = serde_json::from_slice(&bytes).map_err(|e| bad(format!("trust record unreadable: {e}")))?;
        if s.version != STORE_VERSION {
            return Err(bad(format!(
                "trust record version {} is not {STORE_VERSION}",
                s.version
            )));
        }
        Ok(s)
    }

    fn save(&self, s: &StoreFile) -> Result<(), Error> {
        if let Some(dir) = self.path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|source| Error::Io {
                path: dir.to_path_buf(),
                source,
            })?;
        }
        let mut v = serde_json::to_vec_pretty(s).expect("record serializes");
        v.push(b'\n');
        rcfd_project::atomic::atomic_write(&self.path, &v).map_err(|e| crate::allow::project_err(&self.path, e))
    }

    /// The confirmed entries of `project`, if any.
    pub fn record(&self, project: &Path) -> Result<Option<Vec<ControlFile>>, Error> {
        self.outside(project)?;
        let key = Self::key(project)?;
        Ok(self.load()?.projects.remove(&key))
    }

    /// Records `files` (the [`Pending::files`] the user was shown and accepted) as the confirmed state of `project`.
    /// If the folder changed after it was shown, the next check differs and asks again.
    pub fn confirm(&self, project: &Path, files: &[ControlFile]) -> Result<(), Error> {
        self.outside(project)?;
        let key = Self::key(project)?;
        let mut s = self.load()?;
        let mut files = files.to_vec();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        s.projects.insert(key, files);
        self.save(&s)
    }

    /// The "allow" button: writes `list` to the project's allow-list file and updates only that file's entry of the
    /// record, so the change made by the user does not ask again while other unconfirmed changes still do. If the file
    /// on disk is not the one written here when it is scanned again (another writer), the entry is not updated.
    pub fn save_allow_list(&self, project: &Path, list: &AllowList) -> Result<(), Error> {
        self.outside(project)?;
        let key = Self::key(project)?;
        list.to_file(&AllowList::file_of(project))?;
        let written = rcfd_project::records::hash_bytes(&list.to_file_bytes());
        let entry = scan(project)?
            .into_iter()
            .find(|c| is_allow_file(&c.path) && c.kind == ControlKind::File && c.hash == written);
        let mut s = self.load()?;
        let rec = s.projects.entry(key).or_default();
        rec.retain(|c| !is_allow_file(&c.path));
        rec.extend(entry);
        rec.sort_by(|a, b| a.path.cmp(&b.path));
        self.save(&s)
    }

    /// Whether a chat may start in `project` (see [`check_folder`]).
    pub fn check(&self, project: &Path) -> Result<TrustState, Error> {
        check_folder(project, Some(self))
    }
}

/// Compares the scan of `project` with its record in `store`. A folder with no control file is confirmed without a
/// record; otherwise the scan must equal the record exactly. With no store nothing can be confirmed, so only a folder
/// without control files passes. Errors (an unreadable folder or record) mean that no chat may start.
pub fn check_folder(project: &Path, store: Option<&TrustStore>) -> Result<TrustState, Error> {
    let files = scan(project)?;
    if files.is_empty() {
        return Ok(TrustState::Confirmed);
    }
    let record = match store {
        Some(s) => s.record(project)?,
        None => None,
    };
    if record.as_ref() == Some(&files) {
        return Ok(TrustState::Confirmed);
    }
    let old: BTreeMap<&str, &ControlFile> = record.iter().flatten().map(|c| (c.path.as_str(), c)).collect();
    let new: BTreeMap<&str, &ControlFile> = files.iter().map(|c| (c.path.as_str(), c)).collect();
    let mut changed: Vec<String> = new
        .iter()
        .filter(|(p, c)| old.get(*p) != Some(*c))
        .map(|(p, _)| p.to_string())
        .chain(old.keys().filter(|p| !new.contains_key(*p)).map(|p| p.to_string()))
        .collect();
    changed.sort();
    let allow_rules = files
        .iter()
        .find(|c| is_allow_file(&c.path) && c.kind == ControlKind::File)
        .map(|c| {
            AllowList::from_file(&project.join(&c.path))
                .map(|l| l.rules().iter().map(crate::AllowRule::to_flag_value).collect())
                .map_err(|e| e.to_string())
        });
    Ok(TrustState::Pending(Pending {
        files,
        changed,
        allow_rules,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("rcfd-claude-trust-{name}-{}", crate::new_uuid()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    /// Not a registered check (E8 needs the trust record, which is open): the scan lists exactly the control files at
    /// any depth and in any ASCII case, sees a one-byte change, and lists nothing for a plain project.
    #[test]
    fn scan_lists_control_files_only() {
        let root = tmp("scan");
        for (rel, text) in [
            (".mcp.json", "{}"),
            (".claude/settings.json", "{\"hooks\":{}}"),
            (".claude/hooks/a.sh", "echo"),
            (".claude-plugin/plugin.json", "{}"),
            ("CLAUDE.md", "# a"),
            ("cases/sub/claude.MD", "# b"),
            ("cases/x/.mcp.json", "{}"),
            ("cases/a.json", "{}"),
            ("notes/readme.md", "x"),
            ("rcfd-project.json", "{}"),
        ] {
            write(&root, rel, text);
        }
        let got = scan(&root).unwrap();
        let paths: Vec<&str> = got.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                ".claude-plugin/plugin.json",
                ".claude/hooks/a.sh",
                ".claude/settings.json",
                ".mcp.json",
                "CLAUDE.md",
                "cases/sub/claude.MD",
                "cases/x/.mcp.json",
            ]
        );
        assert!(got
            .iter()
            .all(|c| c.kind == ControlKind::File && c.hash.starts_with("fnv64:")));
        write(&root, ".claude/settings.json", "{\"hooks\":{ }}");
        let again = scan(&root).unwrap();
        let changed: Vec<&str> = got
            .iter()
            .zip(&again)
            .filter(|(a, b)| a != b)
            .map(|(a, _)| a.path.as_str())
            .collect();
        assert_eq!(changed, [".claude/settings.json"]);
        // control: a project without those files lists nothing
        let plain = tmp("plain");
        write(&plain, "cases/a.json", "{}");
        write(&plain, "rcfd-project.json", "{}");
        assert!(scan(&plain).unwrap().is_empty());
        // a missing folder is an error, never an empty (clean) list
        assert!(scan(&plain.join("missing")).is_err());
    }

    /// Not registered: a linked folder is listed as a link and not followed. Creating a link can need a privilege on
    /// Windows; the test then prints SKIP and passes.
    #[test]
    fn scan_lists_links_without_following_them() {
        let root = tmp("links");
        let outside = tmp("outside");
        write(&outside, "CLAUDE.md", "# elsewhere");
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_dir(&outside, root.join("cases"));
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(&outside, root.join("cases"));
        if let Err(e) = made {
            println!("SKIP scan_lists_links_without_following_them: cannot create a symbolic link here ({e})");
            return;
        }
        let got = scan(&root).unwrap();
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!((got[0].path.as_str(), got[0].kind), ("cases", ControlKind::Link));
    }
}
