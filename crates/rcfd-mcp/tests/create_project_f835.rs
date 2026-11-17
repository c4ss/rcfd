//! ADR 0731 P3 (F-835): the MCP tool `create_project` on a server without `--project`. P-T5: with `--root` a path outside
//! it is refused with 1004 and nothing is made; `--read-only` refuses it with 1100; with `--project` it is neither listed
//! nor callable. The list without `--project` is checked by `default_f1.rs` (P-T6), the list with it, unedited, by
//! `project_list_f270.rs` and `tool_list_f275.rs` (P-T7).

mod common;

use common::*;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mcp::policy::Policy;
use rcfd_mcp::McpServer;
use rcfd_project::Project;
use serde_json::json;
use std::path::Path;

fn rooted(root: &Path, policy: Policy) -> McpServer {
    McpServer::new(
        Service::new(ServiceConfig {
            root: Some(root.to_path_buf()),
            ..Default::default()
        }),
        policy,
    )
}

#[test]
fn p_t5_create_project_inside_the_root_makes_a_project() {
    let root = scratch("f835-in");
    let s = rooted(&root, Policy::default());
    let v = ok(&call(&s, "create_project", json!({"path": "fan", "name": "Fan study"})));
    assert_eq!(v["name"], "Fan study");
    assert_eq!(v["existing"], json!([]));
    assert_eq!(
        v["solver_version_created"],
        format!("rcfd {}", env!("CARGO_PKG_VERSION"))
    );
    let p = Project::open(&root.join("fan")).unwrap();
    assert_eq!(p.manifest().name, "Fan study");
    assert_eq!(v["created"], p.manifest().created);
    assert_eq!(v["root"], p.root().display().to_string());
    // a second call refuses (1200) and names the folder
    let r = call(&s, "create_project", json!({"path": "fan"}));
    assert_eq!(err_code(&r), 1200, "{r}");
    assert!(r["structuredContent"]["error"]["data"]["root"].is_string(), "{r}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn p_t5_a_path_outside_the_root_is_refused_and_nothing_is_made() {
    let root = scratch("f835-root");
    let outside = scratch("f835-outside").join("p");
    let s = rooted(&root, Policy::default());
    let r = call(&s, "create_project", json!({"path": outside.display().to_string()}));
    assert_eq!(err_code(&r), 1004, "{r}");
    assert!(!outside.exists());
    let r = call(&s, "create_project", json!({"path": "../escape"}));
    assert_eq!(err_code(&r), 1004, "{r}");
    assert!(!root.parent().unwrap().join("escape").join("rcfd-project.json").exists());
    assert_eq!(
        std::fs::read_dir(&root).unwrap().count(),
        0,
        "nothing made under the root"
    );
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(outside.parent().unwrap());
}

#[test]
fn p_t5_read_only_refuses_create_project() {
    let root = scratch("f835-ro");
    let s = rooted(&root, Policy::read_only());
    let r = call(&s, "create_project", json!({"path": "p"}));
    assert_eq!(err_code(&r), 1100, "{r}");
    assert!(!root.join("p").exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn p_t5_with_a_project_create_project_is_not_listed_or_callable() {
    let dir = project("f835-proj");
    let s = server(&dir, Policy::default());
    let list = s.tool_list();
    assert!(
        !list["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "create_project"),
        "create_project is listed with --project"
    );
    let r = rpc_call(&s, "create_project", json!({"path": "other"}));
    assert_eq!(r["error"]["code"], -32602, "{r}");
    assert!(!dir.join("other").exists());
}

#[test]
fn p_t3_bad_names_are_argument_errors() {
    let root = scratch("f835-names");
    let s = rooted(&root, Policy::default());
    for name in ["", "   ", "a\u{7}b", &"x".repeat(201)] {
        let r = call(&s, "create_project", json!({"path": "p", "name": name}));
        assert_eq!(err_code(&r), -32602, "{name:?}: {r}");
    }
    assert!(!root.join("p").exists());
    let _ = std::fs::remove_dir_all(&root);
}
