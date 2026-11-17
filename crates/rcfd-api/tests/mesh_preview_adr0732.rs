//! ADR 0732 (F-836): `mesh.preview`, the mesh a session's case runs on, built without solving and written as a mesh-only
//! field store. M1 (replaces the egui Studio's `preview_g1` G1): for every shipped case the stored mesh is the one
//! `Simulation::new` sets the run up on, bit for bit. M2 (replaces `adaptive_preview_f475`): a `[mesh.adapt]` case
//! previews its base mesh with `pending: true`, and with `adapt: "final"` the mesh the run solves on after its first
//! step. M3: without `--ui` the method is -32601 and `service.info` is the plain reply of the same build. M4: every
//! error, with nothing left behind. M5: the store opens with `rcfd_tsstore::Reader` and its mesh decodes to the same
//! mesh. Also: the cell estimate against the built mesh, reuse of a store of the same mesh, unsaved settings preview.
//!
//! No frozen recording: every comparison is with what the same build computes (ADR 0730's rule).

use rcfd_api::case::Case;
use rcfd_api::mesh_preview::{estimate, ESTIMATE_LIMIT};
use rcfd_api::service::{Service, ServiceConfig, METHODS};
use rcfd_api::sim::{self, Simulation};
use rcfd_api::ui::UI_METHODS;
use rcfd_mesh::Mesh;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd-adr0732-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn svc(ui: bool) -> Arc<Service> {
    svc_with(ServiceConfig::default(), ui)
}

fn svc_with(cfg: ServiceConfig, ui: bool) -> Arc<Service> {
    let s = Service::new(cfg);
    if ui {
        s.enable_ui();
    }
    s
}

fn call(s: &Arc<Service>, method: &str, p: Value) -> Result<Value, Value> {
    let r = s
        .handle_request(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": p}))
        .unwrap();
    match r.get("error") {
        Some(e) => Err(e.clone()),
        None => Ok(r["result"].clone()),
    }
}

fn session(s: &Arc<Service>, p: Value) -> String {
    call(s, "session.create", p).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string()
}

/// The shipped cases, `cases/**/*.toml`, sorted.
fn shipped() -> Vec<PathBuf> {
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "toml") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo().join("cases"), &mut out);
    out.sort();
    out
}

/// The entries of a folder (names), sorted.
fn entries(d: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(d)
        .map(|r| {
            r.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// G1's comparison (ADR 0138): the reply and the stored mesh against `mesh`, bit for bit.
fn assert_same(reply: &Value, mesh: &Mesh, what: &str) {
    let hash = format!("{:016x}", mesh.content_hash());
    assert_eq!(reply["mesh_hash"], hash, "{what}: mesh hash");
    assert_eq!(
        (&reply["cells"], &reply["faces"], &reply["points"]),
        (&json!(mesh.n_cells()), &json!(mesh.n_faces()), &json!(mesh.n_points())),
        "{what}: counts"
    );
    let want: Vec<Value> = mesh
        .patches()
        .iter()
        .map(|p| json!({"name": p.name, "kind": format!("{:?}", p.kind), "faces": p.size}))
        .collect();
    assert_eq!(reply["patches"], json!(want), "{what}: patches");
    // M5: the store opens with the Rust reader; its mesh decodes, rehashes and equals `mesh`
    let path = PathBuf::from(reply["path"].as_str().unwrap());
    assert_eq!(
        path.file_name().unwrap().to_string_lossy(),
        hash,
        "{what}: folder named by the hash"
    );
    let mut r = rcfd_tsstore::Reader::open(&path).unwrap_or_else(|e| panic!("{what}: open: {e}"));
    assert_eq!(r.mesh_hash(), mesh.content_hash(), "{what}: recorded hash");
    assert!(r.snapshots().is_empty(), "{what}: a mesh-only store");
    let stored = r.mesh().unwrap_or_else(|e| panic!("{what}: mesh: {e}"));
    assert_eq!(stored.content_hash(), mesh.content_hash(), "{what}: decoded hash");
    assert_eq!(stored.points(), mesh.points(), "{what}: points bit for bit");
    assert_eq!(stored.patches(), mesh.patches(), "{what}: patches of the store");
    let two_d = mesh.patches().iter().any(|p| p.kind == rcfd_mesh::PatchKind::Empty);
    assert_eq!(reply["two_d"], two_d, "{what}: 2D");
}

/// M1: every shipped case previews the mesh `Simulation::new` sets its run up on; where it refuses the case, the mesh
/// of `build_mesh` (said in the output). The cell estimate, where there is one, is the built mesh's count.
#[test]
fn m1_every_shipped_case_previews_the_solvers_mesh() {
    let s = svc(true);
    let out = tmp("m1");
    let cases = shipped();
    assert!(cases.len() >= 10, "{} shipped cases", cases.len());
    for file in &cases {
        let rel = file.strip_prefix(repo()).unwrap().display().to_string();
        let case = Case::load(file).unwrap_or_else(|d| panic!("{rel}: {d:?}"));
        let id = session(&s, json!({"path": file.display().to_string()}));
        let reply = call(
            &s,
            "mesh.preview",
            json!({"session": id, "dir": out.display().to_string()}),
        )
        .unwrap_or_else(|e| panic!("{rel}: {e}"));
        let (mesh, source) = match Simulation::new(&case) {
            Ok(sim) => ((*sim.geom.mesh).clone(), "Simulation::new".to_string()),
            Err(d) => (
                sim::build_mesh(&case).unwrap(),
                format!("build_mesh (Simulation::new refuses: {})", d[0]),
            ),
        };
        assert_same(&reply, &mesh, &rel);
        if let Some(n) = estimate(&case) {
            assert_eq!(n, mesh.n_cells() as u64, "{rel}: estimate");
        }
        eprintln!(
            "M1 {rel}: {} cells {}, against {source}; estimate {:?}; adapt {}",
            mesh.n_cells(),
            reply["mesh_hash"],
            estimate(&case),
            reply["adapt"]
        );
    }
    // only the stores, no temporary folder left
    assert!(entries(&out).iter().all(|e| e.len() == 16), "{:?}", entries(&out));
    let _ = std::fs::remove_dir_all(&out);
}

const ADAPT: &str = "cases/cavity/adapt_re100.toml";

/// M2: the base mesh with `pending: true`, and with `adapt: "final"` the mesh of the run after its first step.
#[test]
fn m2_an_adaptive_case_previews_its_base_and_on_request_its_final_mesh() {
    let s = svc(true);
    let out = tmp("m2");
    let file = repo().join(ADAPT);
    let case = Case::load(&file).unwrap();
    let id = session(&s, json!({"path": file.display().to_string()}));
    let dir = out.display().to_string();
    let base = call(&s, "mesh.preview", json!({"session": id, "dir": dir})).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    assert_same(&base, &sim.geom.mesh, "M2 base");
    assert_eq!(base["adapt"], json!({"pending": true, "cycles": 2, "solves": 0}));
    let explicit = call(&s, "mesh.preview", json!({"session": id, "dir": dir, "adapt": "base"})).unwrap();
    assert_eq!(explicit["mesh_hash"], base["mesh_hash"]);
    assert_eq!(explicit["reused"], true, "the same mesh: the store is used again");
    let fin = call(&s, "mesh.preview", json!({"session": id, "dir": dir, "adapt": "final"})).unwrap();
    sim.step();
    let run = (*sim.geom.mesh).clone();
    assert_same(&fin, &run, "M2 final");
    assert_eq!(fin["adapt"], json!({"pending": false, "cycles": 2, "solves": 3}));
    assert_eq!(run.n_cells(), 2799, "ADR 0242's adapted mesh");
    assert_ne!(fin["mesh_hash"], base["mesh_hash"]);
    eprintln!(
        "M2: base {} cells {}, final {} cells {}",
        base["cells"], base["mesh_hash"], fin["cells"], fin["mesh_hash"]
    );
    // a case without the table: adapt null, base and final the same mesh
    let plain = session(&s, json!({}));
    let a = call(&s, "mesh.preview", json!({"session": plain, "dir": dir})).unwrap();
    let b = call(
        &s,
        "mesh.preview",
        json!({"session": plain, "dir": dir, "adapt": "final"}),
    )
    .unwrap();
    assert_eq!(a["adapt"], Value::Null);
    assert_eq!(a["mesh_hash"], b["mesh_hash"]);
    let _ = std::fs::remove_dir_all(&out);
}

/// M3: without the switch `mesh.preview` is -32601 like any unknown method, and `service.info` is the plain reply; with
/// it, the plain reply plus `ui` and the names of `UI_METHODS`, `mesh.preview` among them.
#[test]
fn m3_without_ui_the_method_does_not_exist() {
    let off = svc(false);
    let id = session(&off, json!({}));
    let e = call(&off, "mesh.preview", json!({"session": id, "dir": "x"})).unwrap_err();
    assert_eq!(e, json!({"code": -32601, "message": "unknown method 'mesh.preview'"}));
    assert!(!METHODS.contains(&"mesh.preview"));
    assert!(UI_METHODS.contains(&"mesh.preview"));
    let plain = call(&off, "service.info", json!({})).unwrap();
    assert_eq!(plain["methods"], json!(METHODS));
    assert!(plain.get("ui").is_none());
    let mut on = call(&svc(true), "service.info", json!({})).unwrap();
    let m = on["methods"].as_array_mut().unwrap();
    assert_eq!(
        m.split_off(METHODS.len()),
        UI_METHODS.iter().map(|n| json!(n)).collect::<Vec<_>>()
    );
    on.as_object_mut().unwrap().remove("ui");
    assert_eq!(on, plain);
}

/// M4: each error of the ADR, with nothing left in `dir` afterwards.
#[test]
fn m4_errors_leave_nothing_behind() {
    let out = tmp("m4");
    let dir = out.join("previews");
    let d = dir.display().to_string();
    let s = svc(true);
    let id = session(&s, json!({}));
    let code = |r: Result<Value, Value>| r.unwrap_err()["code"].as_i64().unwrap();
    // -32602: shape of the parameters
    for p in [
        json!({"session": id}),
        json!({"session": id, "dir": ""}),
        json!({"session": id, "dir": 3}),
        json!({"dir": d}),
        json!({"session": id, "dir": d, "adapt": "last"}),
        json!({"session": id, "dir": d, "adapt": 1}),
        json!({"session": id, "dir": d, "force": "yes"}),
        json!({"session": id, "dir": d, "case": "cases/a.toml"}),
        json!([id, d]),
        Value::Null,
    ] {
        assert_eq!(code(call(&s, "mesh.preview", p.clone())), -32602, "{p}");
    }
    // 1001: unknown session
    assert_eq!(
        code(call(&s, "mesh.preview", json!({"session": "s999", "dir": d}))),
        1001
    );
    // 1003: the mesh cannot be built, located as mesh.check locates it
    let bad = session(
        &s,
        json!({"settings": {"mesh": {"source": "file", "file": "nowhere.msh"}}}),
    );
    let e = call(&s, "mesh.preview", json!({"session": bad, "dir": d})).unwrap_err();
    let check = call(&s, "mesh.check", json!({"session": bad})).unwrap_err();
    assert_eq!(e["code"], 1003);
    assert_eq!(e, check, "the same located error as mesh.check");
    // 1004: the estimate, unless forced (then max_cells after the build)
    // a typing slip: 100 000 by 100 cells
    call(
        &s,
        "settings.set",
        json!({"session": id, "path": "mesh.nx", "value": 100_000}),
    )
    .unwrap();
    call(
        &s,
        "settings.set",
        json!({"session": id, "path": "mesh.ny", "value": 100}),
    )
    .unwrap();
    let e = call(&s, "mesh.preview", json!({"session": id, "dir": d})).unwrap_err();
    assert_eq!(e["code"], 1004);
    assert!(
        e["message"].as_str().unwrap().contains(&ESTIMATE_LIMIT.to_string()),
        "{e}"
    );
    let small = svc_with(
        ServiceConfig {
            max_cells: 100,
            ..ServiceConfig::default()
        },
        true,
    );
    let sid = session(&small, json!({}));
    call(
        &small,
        "settings.set",
        json!({"session": sid, "path": "mesh.nx", "value": 20}),
    )
    .unwrap();
    call(
        &small,
        "settings.set",
        json!({"session": sid, "path": "mesh.ny", "value": 20}),
    )
    .unwrap();
    for force in [false, true] {
        let e = call(
            &small,
            "mesh.preview",
            json!({"session": sid, "dir": d, "force": force}),
        )
        .unwrap_err();
        assert_eq!(e["code"], 1004);
        assert_eq!(e["message"], "mesh has 400 cells, limit is 100");
    }
    // 1004: a dir outside the sandbox
    let root = out.join("root");
    std::fs::create_dir_all(&root).unwrap();
    let boxed = svc_with(
        ServiceConfig {
            root: Some(root.clone()),
            ..ServiceConfig::default()
        },
        true,
    );
    let bid = session(&boxed, json!({}));
    for p in ["../previews", out.join("elsewhere").display().to_string().as_str()] {
        assert_eq!(
            code(call(&boxed, "mesh.preview", json!({"session": bid, "dir": p}))),
            1004,
            "{p}"
        );
    }
    assert_eq!(entries(&dir), Vec::<String>::new(), "nothing written by a refused call");
    assert_eq!(entries(&root), Vec::<String>::new());
    // inside the sandbox, a relative dir is the root's
    let r = call(&boxed, "mesh.preview", json!({"session": bid, "dir": ".rcfd/previews"})).unwrap();
    assert!(PathBuf::from(r["path"].as_str().unwrap()).starts_with(root.join(".rcfd").join("previews")));
    // -32000: the store cannot be written (dir is a file): no temporary folder, no store
    let file = out.join("a-file");
    std::fs::write(&file, b"x").unwrap();
    let e = call(
        &s,
        "mesh.preview",
        json!({"session": sid_default(&s), "dir": file.join("sub").display().to_string()}),
    )
    .unwrap_err();
    assert_eq!(e["code"], -32000, "{e}");
    assert_eq!(std::fs::read(&file).unwrap(), b"x");
    let _ = std::fs::remove_dir_all(&out);
}

fn sid_default(s: &Arc<Service>) -> String {
    session(s, json!({}))
}

/// The session's settings are previewed, saved or not; a changed setting is a new store, the old one stays; a folder of
/// the mesh's name that holds no store of that mesh is replaced.
#[test]
fn unsaved_settings_preview_and_reuse() {
    let out = tmp("reuse");
    let d = out.display().to_string();
    let s = svc(true);
    let id = session(&s, json!({}));
    let a = call(&s, "mesh.preview", json!({"session": id, "dir": d})).unwrap();
    assert_eq!(a["reused"], false);
    let again = call(&s, "mesh.preview", json!({"session": id, "dir": d})).unwrap();
    assert_eq!(again["reused"], true);
    assert_eq!(again["path"], a["path"]);
    call(
        &s,
        "settings.set",
        json!({"session": id, "path": "mesh.nx", "value": 7}),
    )
    .unwrap();
    let b = call(&s, "mesh.preview", json!({"session": id, "dir": d})).unwrap();
    assert_ne!(a["mesh_hash"], b["mesh_hash"]);
    assert_eq!(b["cells"].as_u64().unwrap() % 7, 0);
    assert_eq!(entries(&out).len(), 2);
    // a damaged store of that name is written again
    let p = PathBuf::from(b["path"].as_str().unwrap());
    std::fs::write(p.join("store.rts"), b"not a store").unwrap();
    let c = call(&s, "mesh.preview", json!({"session": id, "dir": d})).unwrap();
    assert_eq!(c["reused"], false);
    assert!(rcfd_tsstore::Reader::open(&p).is_ok());
    let bounds = &c["bounds"];
    assert_eq!(bounds["min"], json!([0.0, 0.0, 0.0]));
    assert!(bounds["max"][0].as_f64().unwrap() > 0.0);
    let _ = std::fs::remove_dir_all(&out);
}
