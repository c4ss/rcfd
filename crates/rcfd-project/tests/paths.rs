//! ADR 0122 P1: path-escape attempts are refused with located errors; the negative control shows each
//! escape attempt really leaves the root under a naive `root.join(rel)`.

mod common;

use rcfd_project::{Project, ProjectError};
use std::path::{Component, Path, PathBuf};

fn lexical(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c.as_os_str()),
        }
    }
    out
}

/// Link `link` -> `target` (a directory). Returns false if this machine cannot make one.
fn dir_link(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    {
        // A junction needs no privilege, unlike a directory symlink.
        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[test]
fn p1_escape_attempts_are_refused_and_located() {
    let base = common::scratch("p1");
    let outside = base.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret.txt"), b"secret").unwrap();
    let p = Project::create(&base.join("proj"), "p1", "0").unwrap();
    let root = p.root().to_path_buf();
    let out_abs = outside.canonicalize().unwrap().join("secret.txt");
    let link_ok = dir_link(&outside, &root.join("cases").join("out"));
    assert!(
        link_ok,
        "set-up: could not create a directory link (symlink or junction)"
    );

    let mut escapes: Vec<String> = vec![
        "../outside/secret.txt".into(),
        "cases/../../outside/secret.txt".into(),
        "./cases/./../../outside/secret.txt".into(),
        "runs/r-20260101T000000-0000/../../../outside/secret.txt".into(),
        out_abs.to_string_lossy().into_owned(),
        "/outside/secret.txt".into(),
        "cases/out/secret.txt".into(),
        "cases/out".into(),
    ];
    if cfg!(windows) {
        let drive = out_abs
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .chars()
            .next()
            .unwrap();
        escapes.push(r"cases\..\..\outside\secret.txt".into());
        escapes.push(format!("{drive}:secret.txt"));
        escapes.push(r"\\localhost\C$\Windows".into());
        escapes.push(out_abs.to_string_lossy().trim_start_matches(r"\\?\").to_string());
        escapes.push(format!(
            r"\\?\{}",
            out_abs.to_string_lossy().trim_start_matches(r"\\?\")
        ));
    }
    let refusals_only = ["", "  ", "cases/a.json:stream", "cases/\0x"];

    // Negative control: under a naive join every escape attempt leaves the root, lexically or by
    // reading the outside file through the link.
    for a in &escapes {
        let naive = root.join(a);
        let lexical_out = !lexical(&naive).starts_with(&root);
        let reads_secret = std::fs::read(&naive).map(|b| b == b"secret").unwrap_or(false);
        let link_out = naive.canonicalize().map(|c| !c.starts_with(&root)).unwrap_or(false);
        assert!(
            lexical_out || reads_secret || link_out,
            "control: {a:?} does not escape naively ({})",
            naive.display()
        );
    }
    let mut n = 0;
    for a in escapes.iter().map(String::as_str).chain(refusals_only) {
        match p.resolve(a) {
            Err(ProjectError::PathRefused {
                input,
                component,
                reason,
            }) => {
                assert_eq!(input, a);
                assert!(!reason.is_empty());
                let msg = ProjectError::PathRefused {
                    input,
                    component,
                    reason,
                }
                .to_string();
                assert!(msg.contains("refused at"), "{msg}");
                n += 1;
            }
            other => panic!("{a:?} not refused: {other:?}"),
        }
    }
    eprintln!(
        "P1: {n} attempts refused ({} escapes, link_ok={link_ok})",
        escapes.len()
    );

    // Control against over-refusal: ordinary paths resolve under the root.
    for ok in [
        "cases/a.json",
        "./cases/new/deep.json",
        "notes/x y.txt",
        "cases/ünï.json",
        "runs",
    ] {
        let r = p.resolve(ok).unwrap_or_else(|e| panic!("{ok}: {e}"));
        assert!(r.starts_with(&root), "{}", r.display());
    }
}
