//! ADR 0128 finding 10: the project root carries no Windows verbatim prefix (`\\?\`), so path strings derived from
//! it (Claude Code's project folder name, `same_path`) match the folder the user sees.

mod common;

use rcfd_project::paths::strip_verbatim;
use rcfd_project::Project;
use std::path::Path;

#[test]
fn root_has_no_verbatim_prefix() {
    let dir = common::scratch("root");
    let plain = dir.join("proj");
    Project::create(&plain, "root", "0").unwrap();
    let p = Project::open(&plain).unwrap();
    let root = p.root().to_string_lossy().into_owned();
    eprintln!("root {root} (opened as {})", plain.display());
    assert!(!root.starts_with(r"\\?\"), "{root}");
    assert_eq!(
        Path::new(&root).canonicalize().unwrap(),
        plain.canonicalize().unwrap(),
        "the stripped root names another folder"
    );
    // the stripped root is still usable: a project-relative path resolves under it
    assert!(p.resolve("cases/a.json").unwrap().starts_with(p.root()));
}

#[test]
fn verbatim_strings_are_stripped_only_when_safe() {
    let long = format!(r"\\?\C:\{}", "a".repeat(300));
    let cases: [(&str, Option<&str>); 9] = [
        (r"\\?\C:\Users\x\proj", Some(r"C:\Users\x\proj")),
        (r"\\?\d:\a b\c", Some(r"d:\a b\c")),
        (r"\\?\UNC\server\share\proj", Some(r"\\server\share\proj")),
        (r"C:\Users\x\proj", None),
        (r"\\?\C:\Users\x\proj.", None),
        (r"\\?\C:\Users\x\proj \y", None),
        (r"\\?\C:\Users\CON\y", None),
        (r"\\?\C:\Users\nul.txt", None),
        (r"\\?\Volume{1234}\x", None),
    ];
    for (input, want) in cases {
        assert_eq!(strip_verbatim(input).as_deref(), want, "{input}");
    }
    assert_eq!(strip_verbatim(&long), None, "260 characters or more stay verbatim");
}
