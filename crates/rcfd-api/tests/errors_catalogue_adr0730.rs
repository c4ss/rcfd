//! ADR 0730 T8 (open question 7, answer (a)): the error catalogue holds every code the engine can give on every surface
//! (the service, the project tools, the MCP server, and the exit codes of rcfd, rcfd-runner and rcfd-mcp), each with
//! one recovery line, and agrees with the tables of the docs and of `rcfd --help`.
//!
//! The codes are read from the sources, so a code added there without a catalogue entry fails here.

use rcfd_api::errors::{lookup, lookup_rpc, Surface, CATALOGUE};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn crates() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(crates().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn sources(dir: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![crates().join(dir)];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                let name = p.display().to_string();
                // test code asserts codes, it does not give them
                if name.contains("tests") {
                    continue;
                }
                out.push((name, std::fs::read_to_string(&p).unwrap()));
            }
        }
    }
    out
}

/// Integers after `pattern` (e.g. `const X: i64 = ` or `ExitCode::from(`), outside `#[cfg(test)]` modules.
fn numbers_after(text: &str, pattern: &str) -> Vec<i64> {
    let body = text.split("#[cfg(test)]").next().unwrap_or(text);
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(i) = rest.find(pattern) {
        rest = &rest[i + pattern.len()..];
        let digits: String = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '-' || *c == '_')
            .filter(|c| *c != '_')
            .collect();
        if let Ok(n) = digits.parse() {
            out.push(n);
        }
    }
    out
}

#[test]
fn every_service_code_is_in_the_catalogue() {
    let text = read("rcfd-api/src/service.rs");
    let module = text
        .split("pub mod code {")
        .nth(1)
        .unwrap()
        .split("\n}")
        .next()
        .unwrap();
    let codes = numbers_after(module, ": i64 = ");
    // 11, and ADR 0736 P2's CONFLICT (1007)
    assert_eq!(codes.len(), 12, "the constants of service::code");
    for c in codes {
        let e = lookup(Surface::Service, c).unwrap_or_else(|| panic!("service code {c} is not in the catalogue"));
        assert!(!e.recovery.is_empty());
    }
}

#[test]
fn every_rpc_code_of_any_crate_is_in_the_catalogue() {
    let mut found = BTreeSet::new();
    for dir in [
        "rcfd-api/src",
        "rcfd-workspace/src",
        "rcfd-mcp/src",
        "rcfd-runner/src",
        "rcfd-project/src",
    ] {
        for (name, text) in sources(dir) {
            for pat in [": i64 = ", "RpcError::new(", "code: ", "fail(", "\"code\": "] {
                for c in numbers_after(&text, pat) {
                    // JSON-RPC codes and the engine's 1000-range codes; other integers after these patterns are not codes
                    if c <= -32000 || (1000..2000).contains(&c) {
                        found.insert((c, name.clone()));
                    }
                }
            }
        }
    }
    assert!(found.len() > 10, "the scan found the codes");
    let missing: Vec<String> = found
        .iter()
        .filter(|(c, _)| lookup_rpc(*c).is_none())
        .map(|(c, f)| format!("{c} ({f})"))
        .collect();
    assert!(
        missing.is_empty(),
        "JSON-RPC codes given by the engine but not in the catalogue: {missing:?}"
    );
    for c in [1200, 1201, 1202] {
        assert_eq!(lookup_rpc(c).unwrap().surface, Surface::Workspace);
    }
    assert_eq!(lookup_rpc(1100).unwrap().surface, Surface::Mcp);
}

#[test]
fn every_exit_code_is_in_the_catalogue() {
    // rcfd: the u8 constants of every command module and the codes of its usage text
    let mut cli = BTreeSet::new();
    for (_, text) in sources("rcfd-cli/src") {
        cli.extend(numbers_after(&text, ": u8 = "));
        cli.extend(numbers_after(&text, "ExitCode::from("));
    }
    let main = read("rcfd-cli/src/main.rs");
    let block = main.split("EXIT CODES:").nth(1).unwrap().split("\";").next().unwrap();
    let words: Vec<&str> = block.split_whitespace().collect();
    let listed: BTreeSet<i64> = words.iter().filter_map(|w| w.parse().ok()).collect();
    assert_eq!(listed, (0..=8).collect(), "the EXIT CODES block of rcfd --help");
    cli.extend(listed);
    cli.remove(&0);
    for c in &cli {
        assert!(
            lookup(Surface::CliExit, *c).is_some(),
            "rcfd exit code {c} is not in the catalogue"
        );
    }
    // rcfd-runner: its named exit code, ExitCode::from, and the 0/1 of wait
    let mut runner = BTreeSet::from([1]);
    for (_, text) in sources("rcfd-runner/src") {
        runner.extend(numbers_after(&text, ": i32 = "));
        runner.extend(numbers_after(&text, "ExitCode::from("));
    }
    runner.remove(&0);
    for c in &runner {
        assert!(
            lookup(Surface::RunnerExit, *c).is_some(),
            "rcfd-runner exit code {c} is not in the catalogue"
        );
    }
    // rcfd-mcp: the server's exits and the edit hook's
    let main = read("rcfd-mcp/src/main.rs");
    let mut mcp: BTreeSet<i64> = numbers_after(&main, "ExitCode::from(").into_iter().collect();
    mcp.insert(1); // ExitCode::FAILURE
    mcp.remove(&0);
    for c in &mcp {
        assert!(
            lookup(Surface::McpExit, *c).is_some(),
            "rcfd-mcp exit code {c} is not in the catalogue"
        );
    }
    let hook = read("rcfd-mcp/src/edit_hook.rs");
    for c in numbers_after(&hook, ": u8 = ").into_iter().filter(|c| *c != 0) {
        assert!(
            lookup(Surface::McpHookExit, c).is_some(),
            "edit hook exit code {c} is not in the catalogue"
        );
    }
    eprintln!("exit codes: rcfd {cli:?}, rcfd-runner {runner:?}, rcfd-mcp {mcp:?}");
}

/// T8: the catalogue agrees with the error tables of the docs.
#[test]
fn t8_catalogue_agrees_with_the_docs_tables() {
    let docs = crates().join("../docs");
    let proto = std::fs::read_to_string(docs.join("service-protocol.md")).unwrap();
    let table = proto.split("## Errors").nth(1).unwrap();
    let rows: BTreeSet<i64> = table
        .lines()
        .skip(3)
        .take_while(|l| l.starts_with('|'))
        .filter_map(|l| {
            l.trim_matches('|')
                .split('|')
                .next()
                .and_then(|c| c.trim().parse().ok())
        })
        .collect();
    let service: BTreeSet<i64> = CATALOGUE
        .iter()
        .filter(|e| e.surface == Surface::Service)
        .map(|e| e.code)
        .collect();
    assert_eq!(
        rows, service,
        "docs/service-protocol.md, Errors, against the catalogue's service codes"
    );

    let mcp = std::fs::read_to_string(docs.join("mcp-tools.md")).unwrap();
    for c in [1100, 1200, 1201, 1202] {
        assert!(mcp.contains(&c.to_string()), "docs/mcp-tools.md names {c}");
    }
    let workspace: BTreeSet<i64> = CATALOGUE
        .iter()
        .filter(|e| e.surface == Surface::Workspace)
        .map(|e| e.code)
        .collect();
    assert_eq!(workspace, BTreeSet::from([1200, 1201, 1202]));

    let run = std::fs::read_to_string(docs.join("run-control.md")).unwrap();
    let exits = run.split("### Exit codes").nth(1).unwrap();
    let listed: BTreeSet<i64> = exits
        .lines()
        .filter(|l| l.starts_with("| "))
        .filter_map(|l| {
            l.trim_matches('|')
                .split('|')
                .next()
                .and_then(|c| c.trim().parse().ok())
        })
        .filter(|c| *c != 0)
        .collect();
    let cli: BTreeSet<i64> = CATALOGUE
        .iter()
        .filter(|e| e.surface == Surface::CliExit)
        .map(|e| e.code)
        .collect();
    assert_eq!(listed, cli, "docs/run-control.md, Exit codes");
}

/// One entry per code and surface, each line short and one line.
#[test]
fn entries_are_unique_and_one_line() {
    let mut seen = BTreeSet::new();
    for e in CATALOGUE {
        assert!(
            seen.insert((e.surface.name(), e.code)),
            "{} {} twice",
            e.surface.name(),
            e.code
        );
        for t in [e.name, e.meaning, e.recovery] {
            assert!(
                !t.trim().is_empty() && !t.contains('\n'),
                "{} {}",
                e.surface.name(),
                e.code
            );
        }
        assert!(
            e.recovery.chars().count() <= 160,
            "{} {}: recovery over 160 characters",
            e.surface.name(),
            e.code
        );
    }
    // the RPC codes are one number space: no code means two things
    let rpc: Vec<i64> = CATALOGUE
        .iter()
        .filter(|e| e.surface.is_rpc())
        .map(|e| e.code)
        .collect();
    assert_eq!(rpc.len(), rpc.iter().collect::<BTreeSet<_>>().len());
    let v = rcfd_api::errors::catalogue_json();
    assert_eq!(v["errors"].as_array().unwrap().len(), CATALOGUE.len());
}
