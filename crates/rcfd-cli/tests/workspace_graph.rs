//! F-001: the crate dependency graph must be acyclic and follow the layering
//! documented in docs/architecture.md.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

fn graph() -> BTreeMap<String, BTreeSet<String>> {
    let out = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo metadata runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: Value = serde_json::from_slice(&out.stdout).expect("metadata is JSON");
    let mut g = BTreeMap::new();
    for p in v["packages"].as_array().expect("packages") {
        let name = p["name"].as_str().expect("name").to_string();
        let deps = p["dependencies"]
            .as_array()
            .expect("deps")
            .iter()
            .filter_map(|d| d["name"].as_str())
            .filter(|n| n.starts_with("rcfd-"))
            .map(String::from)
            .collect();
        g.insert(name, deps);
    }
    g
}

#[test]
fn all_required_crates_exist() {
    let g = graph();
    for c in [
        "core",
        "mesh",
        "linalg",
        "fv",
        "turb",
        "io",
        "post",
        "cli",
        "api",
        "mcp",
        "assistant",
        "gui",
    ] {
        assert!(g.contains_key(&format!("rcfd-{c}")), "missing crate rcfd-{c}");
    }
}

#[test]
fn graph_is_acyclic() {
    let g = graph();
    // Kahn's algorithm: repeatedly remove nodes with no remaining dependencies.
    let mut remaining: BTreeMap<&String, BTreeSet<&String>> = g.iter().map(|(k, v)| (k, v.iter().collect())).collect();
    while !remaining.is_empty() {
        let ready: Vec<&String> = remaining
            .iter()
            .filter(|(_, d)| d.is_empty())
            .map(|(k, _)| *k)
            .collect();
        assert!(
            !ready.is_empty(),
            "dependency cycle among {:?}",
            remaining.keys().collect::<Vec<_>>()
        );
        for r in &ready {
            remaining.remove(r);
        }
        for d in remaining.values_mut() {
            for r in &ready {
                d.remove(r);
            }
        }
    }
}

#[test]
fn layering_matches_architecture_doc() {
    let g = graph();
    let layer = |n: &str| match n {
        "rcfd-core" => 0,
        "rcfd-mesh" | "rcfd-linalg" | "rcfd-reactor" | "rcfd-project" => 1,
        "rcfd-gpu" | "rcfd-dist" | "rcfd-runner" | "rcfd-claude" => 2,
        "rcfd-fem" => 2,
        "rcfd-io" | "rcfd-fv" | "rcfd-particles" => 3,
        "rcfd-turb" | "rcfd-post" | "rcfd-tsstore" => 4,
        "rcfd-api" | "rcfd-render" => 5,
        // ADR 0173 D1 (a), ADR 0185 E8: the project workspace, above api and below mcp
        "rcfd-workspace" => 6,
        "rcfd-mcp" | "rcfd-cli" => 7,
        "rcfd-assistant" => 8,
        "rcfd-gui" => 9,
        "rcfd-studio" => 10,
        other => panic!("unknown crate {other}"),
    };
    for (c, deps) in &g {
        for d in deps {
            assert!(layer(d) < layer(c), "{c} must not depend on {d}");
        }
    }
}

/// ADR 0173 C6, ADR 0185 A6: the Studio does not depend on the MCP server, the frozen GUI or the assistant (the
/// architecture page's rule; the layer order alone would allow it).
#[test]
fn studio_does_not_depend_on_mcp_gui_or_assistant() {
    let g = graph();
    let studio = g.get("rcfd-studio").expect("rcfd-studio is a workspace crate");
    for c in ["rcfd-mcp", "rcfd-gui", "rcfd-assistant"] {
        assert!(!studio.contains(c), "rcfd-studio must not depend on {c}: {studio:?}");
    }
}
