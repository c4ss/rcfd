//! ADR 0144 J16 (job 48J): `session.create`'s optional `base_dir`. With a `toml` or `settings` case, relative data
//! paths resolve against `base_dir` (the Studio's case editor gives the case's folder); without it they resolve as
//! before (the sandbox root, else the working folder), and the journal is as before.

use rcfd_api::case::Case;
use rcfd_api::service::{code, Service, ServiceConfig};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

/// A case on the 4 x 4 x 4 Gmsh hex box, by a mesh file relative to the case's folder (`cases/`).
const HEX_CASE: &str = r#"title = "box from a Gmsh file (ADR 0144 J16)"
[mesh]
source = "file"
file = "../meshes/hex.msh"
[physics]
nu = 0.01
[boundaries.inlet]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 5
"#;

/// A fresh project-like folder: `cases/` (empty) and `meshes/hex.msh`. Returns the canonical root.
fn project(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir()
        .join("rcfd-api-tests")
        .join(format!("{name}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(root.join("cases")).unwrap();
    std::fs::create_dir_all(root.join("meshes")).unwrap();
    let msh = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rcfd-io/tests/data/hex_v2.2_ascii.msh");
    std::fs::copy(msh, root.join("meshes/hex.msh")).unwrap();
    root.canonicalize().unwrap()
}

/// `session.create` with `p`, then `settings.validate`: the session (when created), validity, diagnostics or the error.
fn create_validate(svc: &Arc<Service>, p: &Value) -> (Option<String>, bool, String) {
    match svc.call("session.create", p) {
        Ok(s) => {
            let id = s["session"].as_str().unwrap().to_string();
            let v = svc.call("settings.validate", &json!({ "session": id })).unwrap();
            (Some(id), v["ok"] == true, v["diagnostics"].to_string())
        }
        Err(e) => (None, false, format!("error {}: {}", e.code, e.message)),
    }
}

/// J16: (i) the hex case as text with `base_dir = <root>/cases` validates, and as settings too; (ii) its journal's
/// create entry holds `base_dir` and `journal.replay` of it makes a session that validates; (iii) with a sandbox root a
/// `base_dir` inside it resolves (the case naming `hex.msh` in `<root>/meshes`: the sandbox refuses `..`) and one
/// outside it is refused (code `LIMIT`). Negative controls (the defaults, kept):
/// without `base_dir` the same text opens and fails at `mesh.file` (the working folder), and its journal's create entry
/// has no `base_dir` key.
#[test]
fn j16_base_dir_resolves_relative_paths_of_a_text_case() {
    let root = project("j16");
    let cases = root.join("cases");
    let svc = Service::new(ServiceConfig::default());
    // (i)
    let with = json!({ "toml": HEX_CASE, "base_dir": cases.to_string_lossy() });
    let (sid, ok, diag) = create_validate(&svc, &with);
    println!("J16 (i) text with base_dir: valid {ok} {diag}");
    assert!(sid.is_some() && ok, "text with base_dir: {diag}");
    let settings: Value =
        serde_json::from_str(&Case::parse_toml_in(HEX_CASE, &cases).unwrap().to_json_string()).unwrap();
    let (_, ok_s, diag_s) = create_validate(
        &svc,
        &json!({ "settings": settings, "base_dir": cases.to_string_lossy() }),
    );
    println!("J16 (i) settings with base_dir: valid {ok_s} {diag_s}");
    assert!(ok_s, "settings with base_dir: {diag_s}");
    // (ii)
    let journal = svc.call("journal.get", &json!({ "session": sid.unwrap() })).unwrap();
    let source = journal["entries"][0]["source"].clone();
    assert_eq!(source["base_dir"], json!(cases.to_string_lossy()), "journal: {source}");
    let replay = svc.call("journal.replay", &json!({ "journal": journal })).unwrap();
    let v = svc
        .call("settings.validate", &json!({ "session": replay["session"] }))
        .unwrap();
    println!(
        "J16 (ii) journal source {source}; replay valid {} matches {}",
        v["ok"], replay["matches"]
    );
    assert_eq!((v["ok"].clone(), replay["matches"].clone()), (json!(true), json!(true)));
    // (iii) the sandbox refuses `..` in any path, so here the case names `hex.msh` in `base_dir = <root>/meshes`
    let boxed = Service::new(ServiceConfig {
        root: Some(root.clone()),
        ..ServiceConfig::default()
    });
    let local = HEX_CASE.replace("../meshes/hex.msh", "hex.msh");
    let inside = json!({ "toml": local, "base_dir": root.join("meshes").to_string_lossy() });
    let (_, ok_in, diag_in) = create_validate(&boxed, &inside);
    let outside = root.parent().unwrap().to_string_lossy().to_string();
    let err = boxed
        .call("session.create", &json!({ "toml": local, "base_dir": outside }))
        .unwrap_err();
    println!(
        "J16 (iii) sandboxed: inside valid {ok_in} {diag_in}; outside: error {} {}",
        err.code, err.message
    );
    assert!(ok_in, "sandboxed, inside: {diag_in}");
    assert_eq!(err.code, code::LIMIT, "{}", err.message);
    // controls: the defaults
    let without = json!({ "toml": HEX_CASE });
    let (sid0, ok0, diag0) = create_validate(&svc, &without);
    let journal0 = svc
        .call("journal.get", &json!({ "session": sid0.clone().unwrap() }))
        .unwrap();
    let source0 = journal0["entries"][0]["source"].clone();
    println!(
        "J16 control: without base_dir: opened {}, valid {ok0} {diag0}; journal source {source0}",
        sid0.is_some()
    );
    assert!(
        sid0.is_some() && !ok0 && diag0.contains("mesh.file"),
        "control: {diag0}"
    );
    assert!(source0.get("base_dir").is_none(), "control: journal {source0}");
}
