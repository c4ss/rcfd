//! `rcfd project init <dir> [--name <name>]` (ADR 0731 P2): make a project folder with
//! `rcfd_workspace::create_project`, which calls `rcfd_project::Project::create` unchanged.
//!
//! `rcfd project trash|trash-list|restore|empty-trash <dir> ...` (ADR 0736 P1, G16): the CLI twins of the MCP project
//! tools `trash_run`, `trash_results`, `trash_case`, `list_trash`, `restore_trash` and `empty_trash`, through the same
//! workspace commands (`rcfd_workspace::Workspace::call`), their journal lines naming the actor `cli` of this process.

use crate::{usage_error, FAIL, OK};
use std::path::Path;

/// `rcfd project ...`.
pub(crate) fn cmd_project(args: &[String]) -> u8 {
    match args.first().map(String::as_str) {
        Some("init") => cmd_init(&args[1..]),
        Some(c @ ("trash" | "trash-list" | "restore" | "empty-trash")) => cmd_trash(c, &args[1..]),
        Some(other) => usage_error(&format!(
            "unknown project command '{other}' (project init|trash|trash-list|restore|empty-trash; see rcfd --help)"
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

/// The usage of the trash commands.
const TRASH_USAGE: &str = "project trash <dir> (--run <id> | --results <id> [--parts fields,output] | --case <path> --base-hash <hash>) [--reason <text>]; project trash-list <dir>; project restore <dir> <token> [--reason <text>]; project empty-trash <dir> [--token <token>]... [--older-than-days <n>] [--reason <text>]";

/// The client name of this CLI's journal lines.
const CLI: &str = "cli";

/// `project trash`, `trash-list`, `restore`, `empty-trash`: the workspace command's result as one JSON line (exit 0);
/// a refusal as `{code, message, data}` on stderr (exit 1); a usage error exit 2.
fn cmd_trash(cmd: &str, args: &[String]) -> u8 {
    use serde_json::{json, Map, Value};
    let mut pos: Vec<&str> = Vec::new();
    let mut opts: Map<String, Value> = Map::new();
    let mut tokens: Vec<Value> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if let Some(key) = a.strip_prefix("--") {
            let known = match cmd {
                "trash" => ["run", "results", "parts", "case", "base-hash", "reason"].contains(&key),
                "restore" => key == "reason",
                "empty-trash" => ["token", "older-than-days", "reason"].contains(&key),
                _ => false,
            };
            if !known {
                return usage_error(&format!("unknown option '{a}' for project {cmd} ({TRASH_USAGE})"));
            }
            let Some(v) = args.get(i + 1) else {
                return usage_error(&format!("{a} needs a value"));
            };
            if key == "token" {
                tokens.push(json!(v));
            } else if opts.insert(key.to_string(), json!(v)).is_some() {
                return usage_error(&format!("{a} given twice"));
            }
            i += 2;
            continue;
        }
        pos.push(a);
        i += 1;
    }
    let want = if cmd == "restore" { 2 } else { 1 };
    if pos.len() != want {
        let what = if want == 2 {
            "a folder and a token"
        } else {
            "one folder"
        };
        return usage_error(&format!("project {cmd} takes {what} ({TRASH_USAGE})"));
    }
    let dir = pos[0];
    let reason = opts.remove("reason");
    let mut a = Map::new();
    let tool = match cmd {
        "trash-list" => "list_trash",
        "restore" => {
            a.insert("token".into(), json!(pos[1]));
            "restore_trash"
        }
        "empty-trash" => {
            if !tokens.is_empty() {
                a.insert("tokens".into(), Value::Array(tokens));
            }
            if let Some(d) = opts.remove("older-than-days") {
                let days = d.as_str().and_then(|s| s.parse::<f64>().ok());
                match days.filter(|d| d.is_finite() && *d >= 0.0) {
                    Some(d) => a.insert("older_than_days".into(), json!(d)),
                    None => return usage_error("--older-than-days needs a number of days, 0 or more"),
                };
            }
            "empty_trash"
        }
        _ => {
            let (run, results, case) = (opts.remove("run"), opts.remove("results"), opts.remove("case"));
            let parts = opts.remove("parts");
            let base = opts.remove("base-hash");
            match (run, results, case) {
                (Some(r), None, None) if parts.is_none() && base.is_none() => {
                    a.insert("run".into(), r);
                    "trash_run"
                }
                (None, Some(r), None) if base.is_none() => {
                    a.insert("run".into(), r);
                    let list: Vec<Value> = match &parts {
                        Some(p) => p
                            .as_str()
                            .unwrap_or_default()
                            .split(',')
                            .map(|s| json!(s.trim()))
                            .collect(),
                        None => vec![json!("fields"), json!("output")],
                    };
                    a.insert("parts".into(), Value::Array(list));
                    "trash_results"
                }
                (None, None, Some(c)) if parts.is_none() => {
                    let Some(b) = base else {
                        return usage_error(
                            "project trash --case needs --base-hash (the hash the file must still have)",
                        );
                    };
                    a.insert("path".into(), c);
                    a.insert("base_hash".into(), b);
                    "trash_case"
                }
                _ => {
                    return usage_error(&format!(
                        "project trash takes one of --run, --results or --case ({TRASH_USAGE})"
                    ))
                }
            }
        }
    };
    if let Some(r) = reason {
        a.insert("reason".into(), r);
    }
    let ws = match rcfd_workspace::Workspace::open(Path::new(dir)) {
        Ok(w) => w.with_actor(rcfd_workspace::journal::Actor::this_process(CLI)),
        Err(e) => {
            eprintln!("error: {e}");
            return FAIL;
        }
    };
    match ws.call(tool, &Value::Object(a)) {
        Some(Ok(rcfd_workspace::Answer::Value(v))) => {
            println!("{v}");
            OK
        }
        Some(Ok(rcfd_workspace::Answer::Image { meta, .. })) => {
            println!("{meta}");
            OK
        }
        Some(Err(e)) if e.code == -32602 => usage_error(&format!("{dir}: {}", e.message)),
        Some(Err(e)) => {
            eprintln!("{}", json!({"code": e.code, "message": e.message, "data": e.data}));
            FAIL
        }
        None => usage_error(&format!("project {cmd}: no such command")),
    }
}
