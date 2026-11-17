//! ADR 0730 through the `rcfd` binary: `rcfd schema` without `--ui` is the untouched export byte for byte (T1 for the
//! CLI: `to_string_pretty` of `case_schema().to_json_schema()`, as every earlier version printed it; no frozen
//! recording, ADR 0730 "Result"), `rcfd schema --ui` carries the metadata, `rcfd help glossary|errors` print JSON, `rcfd help` alone is the
//! usage as before, and `rcfd serve --stdio` answers `help.*` only with `--ui`.

use serde_json::Value;
use std::io::Write;
use std::process::{Command, Output, Stdio};

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
fn schema_without_ui_is_the_untouched_export() {
    let out = rcfd(&["schema"]);
    assert!(out.status.success());
    let want = serde_json::to_string_pretty(&rcfd_api::schema::case_schema().to_json_schema()).unwrap()
        + "
";
    assert!(out.stdout == want.as_bytes(), "rcfd schema differs from the export");
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

/// P4 through the binary: `service.info` with `--ui` is the one without plus `ui` and the methods of the switch at the
/// end: the two help methods and, since ADR 0732 (F-836, restated 2026-11-17), `mesh.preview`.
#[test]
fn serve_info_with_ui_is_the_plain_one_plus_the_additions() {
    let req = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"service.info\"}
";
    let off = serve(&["serve", "--stdio"], req).remove(0)["result"].clone();
    let mut on = serve(&["serve", "--stdio", "--ui"], req).remove(0)["result"].clone();
    assert!(off.get("ui").is_none());
    assert_eq!(on["ui"], "rcfd-ui/1");
    let m = on["methods"].as_array_mut().unwrap();
    assert_eq!(
        m.split_off(m.len() - 3),
        vec![
            Value::from("help.glossary"),
            Value::from("help.errors"),
            Value::from("mesh.preview")
        ]
    );
    on.as_object_mut().unwrap().remove("ui");
    assert_eq!(on, off);
}
