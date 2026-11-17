//! ADR 0730 through the `rcfd` binary: `rcfd schema` without `--ui` is the base commit's output byte for byte (T1 for
//! the CLI), `rcfd schema --ui` carries the metadata, `rcfd help glossary|errors` print JSON, `rcfd help` alone is the
//! usage as before, and `rcfd serve --stdio` answers `help.*` only with `--ui`.

use rcfd_core::hash::fnv64;
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Output, Stdio};

const BASE: &str = include_str!("../../rcfd-api/tests/fixtures/adr0730_base_replies.json");

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn serve(args: &[&str], requests: &str) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rcfd"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(requests.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn schema_without_ui_is_the_base_commit_s() {
    let out = rcfd(&["schema"]);
    assert!(out.status.success());
    let base: Value = serde_json::from_str(BASE).unwrap();
    let want = &base["rcfd_schema_stdout"];
    assert_eq!(out.stdout.len() as u64, want["bytes"].as_u64().unwrap());
    assert_eq!(format!("{:#018x}", fnv64(&out.stdout)), want["fnv64"].as_str().unwrap());
}

#[test]
fn schema_with_ui_carries_the_metadata() {
    let out = rcfd(&["schema", "--ui"]);
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("\"x-rcfd-ui\""));
    assert!(text.contains("\"x-rcfd-label\""));
    assert!(text.contains("\"x-rcfd-advanced\""));
    assert!(v.is_object());
}

#[test]
fn help_glossary_and_errors_print_json() {
    let g = rcfd(&["help", "glossary"]);
    assert!(g.status.success());
    let g: Value = serde_json::from_slice(&g.stdout).unwrap();
    assert!(g["terms"].as_array().unwrap().len() >= 20);
    let e = rcfd(&["help", "errors"]);
    assert!(e.status.success());
    let e: Value = serde_json::from_slice(&e.stdout).unwrap();
    assert!(e["errors"]
        .as_array()
        .unwrap()
        .iter()
        .all(|x| x["recovery"].as_str().is_some_and(|r| !r.is_empty())));
}

#[test]
fn help_alone_is_the_usage() {
    let plain = rcfd(&["--help"]);
    let help = rcfd(&["help"]);
    assert!(help.status.success());
    assert_eq!(help.stdout, plain.stdout);
}

#[test]
fn serve_answers_help_only_with_ui() {
    let req = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"help.errors\"}\n";
    let off = serve(&["serve", "--stdio"], req);
    assert_eq!(off[0]["error"]["code"], -32601);
    let on = serve(&["serve", "--stdio", "--ui"], req);
    assert!(on[0]["result"]["errors"].is_array(), "{}", on[0]);
}
