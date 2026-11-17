//! `rcfd project init <dir> [--name <name>]` (ADR 0731 P2): make a project folder with
//! `rcfd_workspace::create_project`, which calls `rcfd_project::Project::create` unchanged. `rcfd project hash <path>`
//! (ADR 0734 P4): the hash of a mesh file or folder by `rcfd_project::filehash` (re-exported by the workspace), the rule
//! of `hash_project_file`.

use crate::{usage_error, FAIL, OK};
use std::path::Path;

/// `rcfd project ...`.
pub(crate) fn cmd_project(args: &[String]) -> u8 {
    match args.first().map(String::as_str) {
        Some("init") => cmd_init(&args[1..]),
        Some("hash") => cmd_hash(&args[1..]),
        Some(other) => usage_error(&format!(
            "unknown project command '{other}' (project init <dir> [--name <name>], project hash <path>)"
        )),
        None => usage_error("project needs a command: project init <dir> [--name <name>], project hash <path>"),
    }
}

/// Prints `{path, kind, hash, bytes, files}` as one JSON line; exit 0, 1 when the path cannot be hashed (missing, a
/// symbolic link, over the limit named in the message), 2 on a usage error.
fn cmd_hash(args: &[String]) -> u8 {
    let [path] = args else {
        return usage_error("project hash takes exactly one file or folder: project hash <path>");
    };
    if path.starts_with("--") {
        return usage_error(&format!("unknown option '{path}' for project hash"));
    }
    match rcfd_workspace::filehash::hash_path(Path::new(path)) {
        Ok(h) => {
            println!(
                "{}",
                serde_json::json!({"path": path, "kind": h.kind, "hash": h.hash, "bytes": h.bytes, "files": h.files})
            );
            OK
        }
        Err(e) => {
            eprintln!("error: {e}");
            FAIL
        }
    }
}

/// Prints the result as one JSON line; exit 0, 1 on a refusal or failure, 2 on a usage error (a bad name included).
fn cmd_init(args: &[String]) -> u8 {
    let mut dir: Option<&str> = None;
    let mut name: Option<&str> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--name" => {
                let Some(v) = args.get(i + 1) else {
                    return usage_error("--name needs a value");
                };
                name = Some(v);
                i += 2;
                continue;
            }
            a if a.starts_with("--") => return usage_error(&format!("unknown option '{a}' for project init")),
            a if dir.is_none() => dir = Some(a),
            _ => return usage_error("project init takes exactly one folder"),
        }
        i += 1;
    }
    let Some(dir) = dir else {
        return usage_error("project init needs a folder: project init <dir> [--name <name>]");
    };
    match rcfd_workspace::create_project(Path::new(dir), name) {
        Ok(v) => {
            println!("{v}");
            OK
        }
        Err(e) if e.code == -32602 => usage_error(&format!("{dir}: {}", e.message)),
        Err(e) => {
            eprintln!("error: {dir}: {}", e.message);
            FAIL
        }
    }
}
