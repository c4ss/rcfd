//! `rcfd project init <dir> [--name <name>]` (ADR 0731 P2): make a project folder with
//! `rcfd_workspace::create_project`, which calls `rcfd_project::Project::create` unchanged.

use crate::{usage_error, FAIL, OK};
use std::path::Path;

/// `rcfd project ...`.
pub(crate) fn cmd_project(args: &[String]) -> u8 {
    match args.first().map(String::as_str) {
        Some("init") => cmd_init(&args[1..]),
        Some(other) => usage_error(&format!(
            "unknown project command '{other}' (project init <dir> [--name <name>])"
        )),
        None => usage_error("project needs a command: project init <dir> [--name <name>]"),
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
