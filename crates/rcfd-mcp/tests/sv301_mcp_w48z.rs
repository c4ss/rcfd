//! ADR 0160 (job 48Z) item 3 through the MCP tools (library server, default policy): P2 (`check_case_headless` gives SV301 on
//! ADR 0154 K1's wall layer) and P4 (`run_headless` returns the SV301 warning among its events). Checks as registered in ADR
//! 0160 before any run; none was changed afterwards. The case is the committed example changed through `set_setting`, so no
//! case text is a literal here.

use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mcp::policy::Policy;
use rcfd_mcp::McpServer;
use serde_json::{json, Value};

fn example_text() -> String {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../example-projects/wind-tunnel/cases/cylinder-re100.toml"
    );
    std::fs::read_to_string(p).expect("example case readable")
}

/// A server with the K1 set-up (64 x 256, dt 1/64) as its current session.
fn k1_server() -> McpServer {
    let s = McpServer::new(Service::new(ServiceConfig::default()), Policy::default());
    tool(&s, "create_case", json!({"toml": example_text()}));
    for (path, value) in [
        ("mesh.nx", json!(64)),
        ("mesh.ny", json!(256)),
        ("solver.dt", json!(0.015625)),
    ] {
        tool(&s, "set_setting", json!({"path": path, "value": value}));
    }
    s
}

/// The structured result of a tool call that must succeed.
fn tool(s: &McpServer, name: &str, args: Value) -> Value {
    let r = s.call_tool(name, &args).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    assert_ne!(r["isError"], true, "{name}: {r}");
    r["structuredContent"].clone()
}

#[test]
fn p2_check_case_headless_gives_sv301() {
    let s = k1_server();
    let r = tool(&s, "check_case_headless", json!({}));
    let found: Vec<&Value> = r["event"]["rules"]
        .as_array()
        .map(|a| a.iter().filter(|x| x["id"] == "SV301").collect())
        .unwrap_or_default();
    println!("P2: exit {}, SV301 {found:?}", r["exit_code"]);
    assert_eq!(r["exit_code"], 0, "{r}");
    assert_eq!(found.len(), 1, "{r}");
    assert_eq!(found[0]["severity"], "warning");
    assert_eq!(found[0]["path"], "solver.corrector_control");
    assert_eq!(found[0]["fix"]["value"], "adaptive");
}

#[test]
fn p4_run_headless_returns_the_sv301_warning() {
    let s = k1_server();
    let r = tool(&s, "run_headless", json!({"max_steps": 1}));
    let events = r["events"].as_array().cloned().unwrap_or_default();
    let kinds: Vec<String> = events
        .iter()
        .map(|e| {
            format!(
                "{}{}",
                e["event"].as_str().unwrap_or("?"),
                if e["rule"] == "SV301" { ":SV301" } else { "" }
            )
        })
        .collect();
    println!("P4: outcome {}, events {kinds:?}", r["outcome"]);
    assert!(
        events.iter().any(|e| e["event"] == "warning" && e["rule"] == "SV301"),
        "P4: no SV301 warning among {kinds:?}"
    );
}
