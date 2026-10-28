//! The files of a project folder that change what `claude -p` runs or reads (ADR 0134 W48-10 (b)): `.mcp.json`,
//! everything under `.claude/` and `.claude-plugin/`, and `CLAUDE.md`, at any depth. The Studio lists them with their
//! hashes before the first chat in a folder.
//!
//! This module only scans. Where the confirmed hashes are kept, and so whether a chat may start, is not implemented
//! here: a record kept inside the received folder could be shipped with it (see the ADR 0134 addendum, open question).

use crate::Error;
use std::path::Path;

/// Names that change Claude Code's behaviour, lowercase. Matched per path component, ASCII case and trailing dots and
/// spaces ignored (as Windows resolves names).
const NAMES: [&str; 4] = [".mcp.json", "claude.md", ".claude", ".claude-plugin"];

/// What a listed entry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlKind {
    /// A regular file; its contents are hashed.
    File,
    /// A symbolic link or junction (never followed); the text of its target is hashed. Every link in the folder is
    /// listed, because the scan cannot see control files behind it and Claude Code may follow it.
    Link,
}

/// One entry of a scan.
#[derive(Clone, Debug, PartialEq, Eq)]
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

fn is_control_name(name: &str) -> bool {
    let n = name.trim_end_matches(['.', ' ']).to_ascii_lowercase();
    NAMES.contains(&n.as_str())
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
            } else if control {
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
