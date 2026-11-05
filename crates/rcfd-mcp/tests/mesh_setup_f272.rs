//! ADR 0187 C8-C and C-M (ADR 0173 C8, the MCP part): through the `rcfd-mcp` binary, `check_case_mesh` gives the report
//! `rcfd check-mesh <case> --json` prints (the `rcfd` binary next to `rcfd-mcp`) and the same `ok`, `report` and
//! `patches` as the session `check_mesh` for the same case (a session server without a sandbox root: with one, the
//! service refuses a `mesh.file` with a `..` component, so the session tools cannot load a case that names `meshes/` the
//! usual way); a case's mesh file outside the project is refused. Each new
//! tool answers through the binary, the instructions name them, and a read-only server refuses `import_mesh` with
//! nothing written. Needs `rcfd` next to `rcfd-mcp` (one cargo build profile).

mod common;

use common::*;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mcp::policy::Policy;
use rcfd_mcp::McpServer;
use rcfd_project::Project;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

fn sample(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rcfd-io/tests/data")
        .join(name)
}

/// `rcfd check-mesh <case> --json`: the printed report.
fn cli_report(case: &Path) -> Value {
    let rcfd = binaries().with_file_name(format!("rcfd{}", std::env::consts::EXE_SUFFIX));
    let o = Command::new(rcfd)
        .arg("check-mesh")
        .arg(case)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        o.status.code() == Some(0) || o.status.code() == Some(4),
        "rcfd check-mesh {}: {:?} {}",
        case.display(),
        o.status,
        String::from_utf8_lossy(&o.stderr)
    );
    serde_json::from_slice(&o.stdout).unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&o.stdout)))
}

/// Sorted relative paths of every file and folder under `dir`.
fn tree(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            out.push(p.strip_prefix(dir).unwrap().to_string_lossy().into_owned());
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(p);
            }
        }
    }
    out.sort();
    out
}

fn file_case(file: &str) -> String {
    format!(
        "[mesh]\nsource = \"file\"\nfile = {}\n",
        serde_json::to_string(file).unwrap()
    )
}

/// C8-C: the check through the binary equals `rcfd check-mesh --json` and the session `check_mesh`; escapes refused.
#[test]
fn c8c_check_case_mesh_is_rcfd_check_mesh() {
    binaries();
    let dir = project("f272c");
    let root = Project::open(&dir).unwrap().root().to_path_buf();
    std::fs::create_dir_all(root.join("incoming")).unwrap();
    std::fs::copy(sample("hex_v4.1_ascii.msh"), root.join("incoming/hex_v4.1_ascii.msh")).unwrap();
    let d = root.to_string_lossy().to_string();
    let mut m = Mcp::start(&["--project", &d, "--root", &d]);
    let init = m.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    let instructions = init["result"]["instructions"].as_str().unwrap().to_string();
    for t in ["check_case_mesh", "preview_mesh", "import_mesh", "list_meshes"] {
        assert!(instructions.contains(t), "{t}: {instructions}");
    }
    let imp = ok(&m.call("import_mesh", json!({"path": "incoming/hex_v4.1_ascii.msh"})));
    assert_eq!(imp["path"], "meshes/hex_v4.1_ascii.msh");
    assert_eq!(imp["cells"], 64);
    std::fs::write(
        root.join("cases/hex.toml"),
        file_case(imp["mesh_file"].as_str().unwrap()),
    )
    .unwrap();
    let cavity = std::fs::read_to_string(root.join("cases/cavity.toml")).unwrap();
    std::fs::write(
        root.join("cases/rot.toml"),
        format!("{cavity}[mesh.transform]\nrotate_z_deg = 30.0\n"),
    )
    .unwrap();
    // the session tools of a server without a sandbox root: with one (as under --project), a mesh.file with a `..`
    // component is refused (the service's sandbox rule), so cases/hex.toml could not be loaded there (shown below)
    let s = McpServer::new(Service::new(ServiceConfig::default()), Policy::default());
    for case in ["cases/cavity.toml", "cases/hex.toml", "cases/rot.toml"] {
        let r = ok(&m.call("check_case_mesh", json!({"case": case})));
        assert_eq!(r["case"], case);
        let cli = cli_report(&root.join(case));
        assert_eq!(r["report"], cli, "{case}: the report of rcfd check-mesh --json");
        // the session check_mesh of the same case
        let created = call(&s, "create_case", json!({"path": root.join(case).to_string_lossy()}));
        assert_eq!(created["isError"], false, "{case}: {created}");
        let sess = ok(&call(&s, "check_mesh", json!({})));
        for k in ["ok", "report", "patches"] {
            assert_eq!(r[k], sess[k], "{case}: {k} differs from the session check_mesh");
        }
        assert_eq!(r["patches_total"], r["patches"].as_array().unwrap().len());
        eprintln!(
            "C8-C {case}: {} cells, ok {}, {} patches; equal to rcfd check-mesh --json and the session check_mesh",
            r["report"]["n_cells"], r["ok"], r["patches_total"]
        );
    }
    assert_eq!(
        ok(&m.call("check_case_mesh", json!({"case": "cases/hex.toml"})))["mesh"],
        json!({"source": "file", "file": "meshes/hex_v4.1_ascii.msh"})
    );
    // observation: the session tools under the project's sandbox root refuse the case that names meshes/ by `..`
    let sandboxed = server(&root, Policy::default());
    let refused = call(&sandboxed, "create_case", json!({"path": "cases/hex.toml"}));
    assert_eq!(refused["isError"], true, "{refused}");
    let msg = refused["structuredContent"]["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(msg.contains("leaves the sandbox"), "{msg}");
    eprintln!("C8-C observation: session create_case of cases/hex.toml under the project root: {msg}");
    // refusals
    let outside = root
        .parent()
        .unwrap()
        .join(format!("f272-mcp-out-{}.msh", std::process::id()));
    std::fs::copy(sample("hex_v4.1_ascii.msh"), &outside).unwrap();
    std::fs::write(root.join("cases/out.toml"), file_case("../../outside.msh")).unwrap();
    std::fs::write(root.join("cases/abs.toml"), file_case(&outside.to_string_lossy())).unwrap();
    std::fs::write(root.join("cases/missing.toml"), file_case("../meshes/missing.msh")).unwrap();
    for (case, code) in [
        ("cases/out.toml", 1004),
        ("cases/abs.toml", 1004),
        ("cases/missing.toml", 1003),
    ] {
        let r = m.call("check_case_mesh", json!({"case": case}));
        assert_eq!(err_code(&r), code, "{case}: {r}");
        let diags = &r["structuredContent"]["error"]["data"]["diagnostics"];
        assert_eq!(diags[0]["path"], "mesh.file", "{case}: {r}");
    }
    let _ = std::fs::remove_file(&outside);
    // C-M: the other tools answer through the binary
    let p = m.call("preview_mesh", json!({"case": "cases/hex.toml"}));
    assert_eq!(p["isError"], false, "{p}");
    assert_eq!(p["content"][0]["type"], "image");
    assert_eq!(p["content"][0]["mimeType"], "image/png");
    assert!(result_bytes(&p) <= 40_000);
    let l = ok(&m.call("list_meshes", json!({})));
    assert_eq!(l["meshes"][0]["path"], "meshes/hex_v4.1_ascii.msh");
    assert_eq!(l["meshes"][0]["used_by"], json!(["cases/hex.toml"]));
    eprintln!(
        "C-M: preview {} bytes, list_meshes {} entries",
        result_bytes(&p),
        l["total"]
    );
}

/// C-M: a read-only server refuses `import_mesh` with 1100 and writes nothing; the three read tools work.
#[test]
fn cm_read_only_refuses_import_mesh() {
    let dir = project("f272ro");
    std::fs::create_dir_all(dir.join("incoming")).unwrap();
    std::fs::copy(sample("hex_v4.1_ascii.msh"), dir.join("incoming/h.msh")).unwrap();
    let ro = server(&dir, Policy::read_only());
    let before = tree(&dir);
    let r = call(&ro, "import_mesh", json!({"path": "incoming/h.msh"}));
    assert_eq!(err_code(&r), 1100, "{r}");
    assert_eq!(tree(&dir), before, "a refused import changed the project");
    assert_eq!(
        ok(&call(&ro, "check_case_mesh", json!({"case": "cases/cavity.toml"})))["ok"],
        true
    );
    let p = call(&ro, "preview_mesh", json!({"case": "cases/cavity.toml"}));
    assert_eq!(p["content"][0]["type"], "image", "{p}");
    assert_eq!(ok(&call(&ro, "list_meshes", json!({})))["total"], 0);
    // control: the default policy imports
    let s = server(&dir, Policy::default());
    assert_eq!(
        ok(&call(&s, "import_mesh", json!({"path": "incoming/h.msh"})))["copied"],
        true
    );
    eprintln!("C-M read-only: import_mesh 1100 with nothing written; check, preview and list answer");
}
