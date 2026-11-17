//! The `site` commands, shared by `rcfd-runner site ...` and `rcfd site ...` (ADR 0739 P3):
//!
//! - `site --stdio --work <dir>`: the agent (what `ssh` starts on a site);
//! - `site list [--json]`: the sites of the user's file; nothing is started;
//! - `site check <name> [--confirm]`: start the site's agent once, show its facts, the host key fingerprint and the
//!   engine beside this machine's, and whether a run may go there; with `--confirm`, record the user's yes in the file
//!   (`confirmed`, and the engine in `accept_engines` when it differs from this machine's).

use super::link::AgentLink;
use super::sites::{self, SiteEntry};
use super::Site;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Stdio;

fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

/// The host key fingerprints OpenSSH holds for an ssh site (`ssh -G` for the host and port it resolves to, then
/// `ssh-keygen -F ... -l`), joined by `,`. An exec site's is `exec`.
pub fn fingerprint(entry: &SiteEntry) -> Result<String, String> {
    if entry.kind == "exec" {
        return Ok("exec".into());
    }
    let host = entry.host.as_deref().unwrap_or_default();
    let (mut g, _) = crate::sys::console("ssh", 0);
    g.args(["-G", "--", host]).stdin(Stdio::null()).stderr(Stdio::null());
    let out = g.output().map_err(|e| format!("cannot run ssh -G: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let field = |k: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(&format!("{k} ")).map(|v| v.trim().to_string()))
    };
    let name = field("hostname").unwrap_or_else(|| host.rsplit('@').next().unwrap_or(host).to_string());
    let port = field("port").unwrap_or_else(|| "22".into());
    let key = if port == "22" {
        name.clone()
    } else {
        format!("[{name}]:{port}")
    };
    let (mut k, _) = crate::sys::console("ssh-keygen", 0);
    k.args(["-l", "-F", &key]).stdin(Stdio::null()).stderr(Stdio::null());
    let out = k.output().map_err(|e| format!("cannot run ssh-keygen: {e}"))?;
    let prints: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .filter(|w| w.starts_with("SHA256:"))
        .map(String::from)
        .collect();
    if prints.is_empty() {
        return Err(format!(
            "the host key of {key} is not in known_hosts: connect once with `ssh {host}` so that OpenSSH checks and \
             records it, then check the site again"
        ));
    }
    Ok(prints.join(","))
}

/// `site check`: the report, and whether a run may go to the site now.
pub fn check(name: &str, confirm: bool) -> Result<(Value, bool), String> {
    let (file, entry) = sites::site(name)?;
    let print = fingerprint(&entry);
    let mut link = AgentLink::open(name, &entry).map_err(|e| e.to_string())?;
    let facts = link.hello().map_err(|e| e.to_string())?;
    drop(link);
    let local = super::remote::local_engine(&crate::runner::sibling_exe("rcfd"));
    let theirs = facts.engine_id();
    let same_protocol = facts.major() == super::protocol_major(super::PROTOCOL);
    let mut report = json!({
        "site": name, "file": file.display().to_string(), "kind": entry.kind, "host": entry.host,
        "facts": facts, "fingerprint": print.as_ref().ok(), "fingerprint_error": print.as_ref().err(),
        "engine": theirs, "local_engine": local, "same_engine": theirs == local, "protocol_ok": same_protocol,
        "assistant": entry.assistant, "confirmed": entry.confirmed,
    });
    if confirm {
        if !same_protocol {
            return Err(format!(
                "site {name} speaks {}, this rcfd-runner speaks {}: nothing was confirmed",
                facts.protocol,
                super::PROTOCOL
            ));
        }
        let fp = print?;
        let engine = (theirs != local).then_some(theirs.as_str());
        sites::record_confirmation(&file, name, &fp, engine)?;
        report["confirmed"] = json!(fp);
    }
    let (_, entry) = sites::site(name)?;
    let admitted = match (
        &entry.confirmed,
        super::remote::admit(name, &file, &entry, &facts, &local),
    ) {
        (None, _) => Err(format!("not confirmed: rcfd site check {name} --confirm")),
        (Some(_), r) => r.map(|_| ()),
    };
    report["admitted"] = json!(admitted.is_ok());
    if let Err(e) = &admitted {
        report["reason"] = json!(e);
    }
    Ok((report, admitted.is_ok()))
}

/// `site <args>`: the exit code, or the error to print.
pub fn run(args: &[String]) -> Result<u8, String> {
    if args.iter().any(|a| a == "--stdio") {
        let work = opt(args, "--work").ok_or("site --stdio needs --work <dir>")?;
        return super::agent::serve_stdio(Path::new(work)).map(|c| c as u8);
    }
    match args.first().map(String::as_str) {
        Some("list") => {
            let (file, list) = sites::load()?;
            if args.iter().any(|a| a == "--json") {
                let v: Vec<Value> = list.iter().map(|(n, e)| e.listing(n)).collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({"file": file.display().to_string(), "sites": v}))
                        .unwrap_or_default()
                );
            } else {
                println!("sites file: {}", file.display());
                for (n, e) in &list {
                    let state = match e.check(n) {
                        Ok(()) if e.confirmed.is_some() => "confirmed".to_string(),
                        Ok(()) => "not confirmed".to_string(),
                        Err(m) => format!("refused: {m}"),
                    };
                    println!(
                        "{n}\t{}\t{}\tassistant={}\t{state}",
                        e.kind,
                        e.host.as_deref().unwrap_or("-"),
                        e.assistant
                    );
                }
            }
            Ok(0)
        }
        Some("check") => {
            let name = args
                .get(1)
                .filter(|a| !a.starts_with("--"))
                .ok_or("site check needs a site name")?;
            let (report, ok) = check(name, args.iter().any(|a| a == "--confirm"))?;
            println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
            Ok(u8::from(!ok))
        }
        Some("gc") => Err("site gc comes with phase 2 of ADR 0739; nothing was removed".into()),
        _ => Err("site: list, check <name> [--confirm], or --stdio --work <dir> (the agent)".into()),
    }
}
