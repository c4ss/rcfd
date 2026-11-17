//! ADR 0737 (F-837 E-837a to E-837c): the service methods `case.facts`, `mesh.refine` and `case.save` behind
//! `rcfd serve --ui`, and the library rules under them (`rcfd_api::facts`, `rcfd_api::refine`, `rcfd_api::case_save`).
//!
//! As ADR 0730's tests: a service without the switch answers as the same build's plain service (the methods are the same
//! -32601 as a name that never existed; `--journal-dir` alone does not turn them on), never compared with a frozen
//! recording. C1 holds the run-time rule to the egui Studio's own unit tests (ported into `facts.rs` with their numbers)
//! and to hand-computed facts of a cavity; C3 compares each refinement with the mesh the refined case builds.

use rcfd_api::case::Case;
use rcfd_api::facts::{self, InText, RunTime};
use rcfd_api::refine::{count_keys, count_refinement, Refusal};
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_api::ui::UI_METHODS;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_adr0737_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A transient lid-driven cavity, 16 x 16 cells of 1/16 m, lid speed 1 m/s.
const PISO: &str = r#"title = "piso cavity" # kept by a lines save
[mesh]
generator = "cavity_2d"
nx = 16
ny = 16
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "piso"
dt = 0.0625
end_time = 8.0
"#;

fn call(svc: &Arc<Service>, method: &str, params: Value) -> Value {
    svc.handle_request(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}))
        .unwrap()
}

fn ok(svc: &Arc<Service>, method: &str, params: Value) -> Value {
    let r = call(svc, method, params.clone());
    assert!(r.get("error").is_none(), "{method} {params}: {r}");
    r["result"].clone()
}

fn error(svc: &Arc<Service>, method: &str, params: Value) -> Value {
    call(svc, method, params)["error"].clone()
}

fn with_ui(cfg: ServiceConfig) -> Arc<Service> {
    let s = Service::new(cfg);
    s.enable_ui();
    s
}

fn session(svc: &Arc<Service>, toml: &str) -> String {
    ok(svc, "session.create", json!({"toml": toml}))["session"]
        .as_str()
        .unwrap()
        .to_string()
}

/// C5: without --ui (and with --journal-dir alone) the three methods are -32601, word for word as an unknown name.
#[test]
fn c5_methods_are_unknown_without_the_ui_switch() {
    let d = tmp("c5");
    let plain = Service::new(ServiceConfig::default());
    let journal = Service::new(ServiceConfig::default());
    journal.enable_journal_files(&d.join("j").to_string_lossy()).unwrap();
    for svc in [&plain, &journal] {
        let s = session(svc, PISO);
        for m in ["case.facts", "mesh.refine", "case.save"] {
            assert!(UI_METHODS.contains(&m));
            let e = error(svc, m, json!({"session": s}));
            assert_eq!(
                e,
                json!({"code": -32601, "message": format!("unknown method '{m}'")}),
                "{m}"
            );
        }
        let info = ok(svc, "service.info", json!({}));
        assert!(info.get("ui").is_none());
        for m in ["case.facts", "mesh.refine", "case.save"] {
            assert!(!info["methods"].as_array().unwrap().contains(&json!(m)), "{m}");
        }
    }
    // the switch on: listed, in UI_METHODS' order, before journal.save
    journal.enable_ui();
    let info = ok(&journal, "service.info", json!({}));
    let names: Vec<&str> = info["methods"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let at = names.iter().position(|m| *m == "case.facts").unwrap();
    assert_eq!(&names[at..at + 3], ["case.facts", "mesh.refine", "case.save"]);
    assert_eq!(names.last(), Some(&"journal.save"));
    std::fs::remove_dir_all(&d).ok();
}

/// C1, C2: the facts of a transient cavity (thinnest cell 1/16 m, the lid's 1 m/s), the default plan of a run of it,
/// its entries by the egui rule, and the runner's default memory rule; a steady case has no plan; unsaved edits count.
#[test]
fn c1_c2_facts_and_plan_of_a_session() {
    let svc = with_ui(ServiceConfig::default());
    let s = session(&svc, PISO);
    let f = ok(&svc, "case.facts", json!({"session": s}));
    assert_eq!(f["kind"], "transient");
    assert_eq!(f["cells"], 256);
    assert_eq!(f["two_d"], true);
    assert!((f["h_min"].as_f64().unwrap() - 0.0625).abs() < 1e-12, "{f}");
    assert!((f["h_max"].as_f64().unwrap() - 0.0625).abs() < 1e-12, "{f}");
    assert_eq!(f["u0"], 1.0);
    assert_eq!(f["u0_from"], "boundary lid");
    assert_eq!((f["h_unit"].clone(), f["u0_unit"].clone()), (json!("m"), json!("m/s")));
    assert_eq!(f["dt_in_text"], true, "0.0625 is not the schema default");
    assert_eq!(f["correctors"]["not_adaptive"], Value::Null);
    let dt0 = facts::round_down(0.5 * 0.0625 / 1.0);
    assert_eq!(
        f["run_time"]["entries"],
        json!([
            "solver.end_time=8",
            format!("solver.dt={dt0}"),
            "solver.max_courant=0.5",
            "solver.max_dt=0.08",
            facts::ADAPTIVE_ENTRY
        ])
    );
    assert_eq!(f["run_time"]["dt0"], dt0);
    assert_eq!(f["run_time"]["over_dt"], 0.0625);
    assert_eq!(f["run_time"]["warning"], Value::Null);
    assert_eq!(f["memory"]["budgets"], "runner_defaults");
    let d = rcfd_api::facts::Budgets::default();
    assert_eq!(
        f["memory"]["mb"],
        json!((d.mem_base_mb + 256.0 * d.mem_kb_per_cell / 1024.0).ceil() as u64)
    );
    // the case's own step taken as it is, Courant 1 at 0.0625 s: no warning; at 0.25 s, Courant 4: the warning names the fix
    ok(
        &svc,
        "settings.set",
        json!({"session": s, "path": "solver.dt", "value": 0.25}),
    );
    let f = ok(
        &svc,
        "case.facts",
        json!({"session": s, "run_time": {"mode": "case", "simulate_s": 2}}),
    );
    let w = f["run_time"]["warning"].as_str().unwrap();
    assert!(
        w.contains("solver.dt = 0.25 s") && w.contains("estimated Courant number of 4") && w.contains("Fix:"),
        "{w}"
    );
    assert_eq!(
        f["run_time"]["entries"],
        json!(["solver.end_time=2", facts::ADAPTIVE_ENTRY])
    );
    // an unsaved edit of the mesh is in the facts
    ok(
        &svc,
        "settings.set",
        json!({"session": s, "path": "mesh.nx", "value": 32}),
    );
    let f = ok(&svc, "case.facts", json!({"session": s}));
    assert_eq!(f["cells"], 512);
    assert!((f["h_min"].as_f64().unwrap() - 1.0 / 32.0).abs() < 1e-12);
    // a steady case: facts, no plan, the reason
    let st = session(&svc, &PISO.replace("algorithm = \"piso\"", "algorithm = \"simplec\""));
    let f = ok(&svc, "case.facts", json!({"session": st}));
    assert_eq!(f["kind"], "steady");
    assert_eq!(f["run_time"]["refused"], "a steady run has no time step");
    assert_eq!(f["u0"], 0.0);
}

/// P1 errors: a case that does not build is 1003 with diagnostics; over max_cells 1004; bad run_time -32602.
#[test]
fn p1_errors() {
    let svc = with_ui(ServiceConfig {
        max_cells: 100,
        ..ServiceConfig::default()
    });
    let s = session(&svc, PISO);
    let e = error(&svc, "case.facts", json!({"session": s}));
    assert_eq!(e["code"], 1004, "{e}");
    assert!(e["message"].as_str().unwrap().contains("256 cells"), "{e}");
    let svc = with_ui(ServiceConfig::default());
    let s = session(
        &svc,
        &PISO.replace("nx = 16", "nx = 16\nsource = \"file\"\nfile = \"missing.msh\""),
    );
    let e = error(&svc, "case.facts", json!({"session": s}));
    assert_eq!(e["code"], 1003, "{e}");
    assert!(
        e["data"]["diagnostics"].as_array().is_some_and(|d| !d.is_empty()),
        "{e}"
    );
    let s = session(&svc, PISO);
    for rt in [json!({"mode": "auto"}), json!({"x": 1}), json!(3)] {
        assert_eq!(
            error(&svc, "case.facts", json!({"session": s, "run_time": rt}))["code"],
            -32602,
            "{rt}"
        );
    }
    assert_eq!(error(&svc, "case.facts", json!({"session": "nope"}))["code"], 1001);
}

/// C3: each generator of the count list doubles exactly its counts, and the cells after are those of the refined case's
/// own mesh (4x in 2D, 8x in 3D).
#[test]
fn c3_refinement_of_each_generator_is_the_refined_mesh() {
    let gens = [
        ("cavity_2d", vec![]),
        ("box_2d", vec![]),
        ("channel_2d", vec![]),
        ("channel_2d_interface", vec![("mesh.nx_right", 5), ("mesh.ny_right", 3)]),
        ("cylinder_ogrid", vec![("mesh.r_in", 1), ("mesh.r_out", 4)]),
        ("wedge_2d", vec![]),
        ("nozzle_2d", vec![]),
        ("annulus_3d", vec![("mesh.r_in", 1), ("mesh.r_out", 2), ("mesh.nz", 2)]),
        (
            "annulus_3d_sector",
            vec![
                ("mesh.r_in", 1),
                ("mesh.r_out", 2),
                ("mesh.nz", 2),
                ("mesh.sector_angle", 30),
            ],
        ),
        ("pipe_sweep", vec![("mesh.r_out", 1), ("mesh.nz", 2)]),
        ("box_3d", vec![("mesh.nz", 2)]),
        (
            "annulus_3d_interface",
            vec![
                ("mesh.r_in", 1),
                ("mesh.r_out", 2),
                ("mesh.nz", 2),
                ("mesh.nz_right", 3),
            ],
        ),
        (
            "annulus_3d_stage",
            vec![("mesh.r_in", 1), ("mesh.r_out", 2), ("mesh.nz", 2)],
        ),
        (
            "annulus_3d_cylindrical_interface",
            vec![
                ("mesh.r_in", 1),
                ("mesh.r_out", 3),
                ("mesh.r_interface", 2),
                ("mesh.nz", 2),
            ],
        ),
        (
            "annulus_3d_sector_cylindrical_interface",
            vec![
                ("mesh.r_in", 1),
                ("mesh.r_out", 3),
                ("mesh.r_interface", 2),
                ("mesh.nz", 2),
                ("mesh.sector_angle", 30),
            ],
        ),
    ];
    let mut built = 0;
    for (g, extra) in gens {
        let mut case = Case::new();
        case.set("mesh.generator", json!(g)).unwrap();
        case.set("mesh.nx", json!(8)).unwrap();
        case.set("mesh.ny", json!(4)).unwrap();
        for (k, v) in &extra {
            case.set(k, json!(v)).unwrap();
        }
        let r = match count_refinement(&case, 2, None) {
            Ok(r) => r,
            Err(Refusal::Invalid(d)) => {
                eprintln!("C3: {g}: does not build with these settings ({}), skipped", d[0]);
                continue;
            }
            Err(e) => panic!("{g}: {e:?}"),
        };
        let keys = count_keys(g).unwrap();
        assert!(
            r.edits
                .iter()
                .all(|e| keys.contains(&&e.path["mesh.".len()..]) && e.after == 2 * e.before),
            "{g}"
        );
        let mut refined = case.clone();
        for e in &r.edits {
            refined.set(&e.path, json!(e.after)).unwrap();
        }
        let cells = rcfd_api::sim::build_mesh(&refined).unwrap().n_cells() as u64;
        assert_eq!(r.cells_after, cells, "{g}: {r:?}");
        assert_eq!(r.cells_after, r.cells * if r.two_d { 4 } else { 8 }, "{g}");
        built += 1;
        eprintln!("C3: {g}: {} -> {} cells", r.cells, r.cells_after);
    }
    assert!(built >= 13, "{built} generators built");
}

/// C3: a mesh file, axial_fan_sector, a count over its maximum and a factor other than 2 are refused with the reason;
/// nothing is set. `apply: true` sets the edits as unsaved changes of the session.
#[test]
fn c3_refusals_and_apply() {
    let svc = with_ui(ServiceConfig::default());
    let cases = [
        (
            PISO.replace("nx = 16", "nx = 16\nsource = \"file\"\nfile = \"m.msh\""),
            "mesh file",
        ),
        (
            PISO.replace("generator = \"cavity_2d\"", "generator = \"axial_fan_sector\""),
            "mesh.fan.n_*",
        ),
        (PISO.replace("nx = 16", "nx = 60000"), "more than its maximum 100000"),
    ];
    for (text, why) in cases {
        let s = session(&svc, &text);
        let before = ok(&svc, "journal.get", json!({"session": s}));
        let e = error(&svc, "mesh.refine", json!({"session": s, "factor": 2, "apply": true}));
        assert_eq!(e["code"], 1004, "{e}");
        assert!(e["message"].as_str().unwrap().contains(why), "{why}: {e}");
        assert_eq!(
            ok(&svc, "journal.get", json!({"session": s}))["entries"],
            before["entries"]
        );
    }
    let s = session(&svc, PISO);
    for f in [json!(3), json!(1), json!(0.5), json!("2")] {
        assert_eq!(
            error(&svc, "mesh.refine", json!({"session": s, "factor": f}))["code"],
            -32602,
            "{f}"
        );
    }
    let r = ok(&svc, "mesh.refine", json!({"session": s, "factor": 2}));
    assert_eq!(
        (r["cells"].clone(), r["cells_after"].clone(), r["applied"].clone()),
        (json!(256), json!(1024), json!(false))
    );
    assert_eq!(
        ok(&svc, "settings.get", json!({"session": s, "path": "mesh.nx"}))["value"],
        16
    );
    let r = ok(&svc, "mesh.refine", json!({"session": s, "factor": 2, "apply": true}));
    assert_eq!(r["applied"], true);
    assert_eq!(
        r["edits"],
        json!([{"path": "mesh.nx", "before": 16, "after": 32}, {"path": "mesh.ny", "before": 16, "after": 32}])
    );
    assert_eq!(
        ok(&svc, "settings.get", json!({"session": s, "path": "mesh.ny"}))["value"],
        32
    );
    let j = ok(&svc, "journal.get", json!({"session": s}));
    let sets: Vec<&Value> = j["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["op"] == "set")
        .collect();
    assert_eq!(sets.len(), 2);
}

fn hash(p: &Path) -> String {
    rcfd_project::records::hash_bytes(&std::fs::read(p).unwrap())
}

/// C4: case.save writes what Case::load reads to the session's case hash; a stale base is 1008 with the file unchanged;
/// inside a project 1004 naming edit_case; `lines` keeps a comment; the lock file beside the file is honoured and gone
/// after the write.
#[test]
fn c4_save_outside_a_project() {
    let d = tmp("c4");
    let svc = with_ui(ServiceConfig::default());
    let s = session(&svc, PISO);
    ok(
        &svc,
        "settings.set",
        json!({"session": s, "path": "physics.nu", "value": 0.02}),
    );
    let want = ok(&svc, "journal.get", json!({"session": s}))["case_hash"].clone();
    for name in ["new.toml", "new.json"] {
        let f = d.join(name);
        let r = ok(&svc, "case.save", json!({"session": s, "path": f, "base_hash": null}));
        assert_eq!(r["old_hash"], Value::Null);
        assert_eq!(r["new_hash"], json!(hash(&f)));
        assert_eq!(r["form"], "whole");
        let back = Case::load(&f).unwrap();
        assert_eq!(json!(format!("{:016x}", back.tree.content_hash())), want, "{name}");
        assert!(!rcfd_api::case_save::lock_path(&f).exists());
        // the file is there now: null is a conflict, and so is a stale hash; the file is not touched
        let e = error(&svc, "case.save", json!({"session": s, "path": f, "base_hash": null}));
        assert_eq!(
            (e["code"].clone(), e["data"]["current_hash"].clone()),
            (json!(1008), json!(hash(&f)))
        );
        let e = error(
            &svc,
            "case.save",
            json!({"session": s, "path": f, "base_hash": "fnv64:0000000000000000"}),
        );
        assert_eq!(e["code"], 1008);
        assert_eq!(r["new_hash"], json!(hash(&f)));
    }
    // lines: the file's comment is kept, the changed value written in its line
    let f = d.join("own.toml");
    std::fs::write(&f, PISO).unwrap();
    let s2 = ok(&svc, "session.create", json!({"path": f}))["session"]
        .as_str()
        .unwrap()
        .to_string();
    ok(
        &svc,
        "settings.set",
        json!({"session": s2, "path": "physics.nu", "value": 0.005}),
    );
    let base = hash(&f);
    let r = ok(
        &svc,
        "case.save",
        json!({"session": s2, "path": f, "base_hash": base, "form": "lines"}),
    );
    assert_eq!(r["form"], "lines");
    assert_eq!(r["old_hash"], json!(base));
    let text = std::fs::read_to_string(&f).unwrap();
    assert!(
        text.contains("# kept by a lines save") && text.contains("nu = 0.005"),
        "{text}"
    );
    // another writer's lock: refused, the file not touched
    let lock = rcfd_api::case_save::lock_path(&f);
    std::fs::write(&lock, "{}").unwrap();
    let e = error(
        &svc,
        "case.save",
        json!({"session": s2, "path": f, "base_hash": hash(&f)}),
    );
    assert_eq!(e["code"], 1002, "{e}");
    std::fs::remove_file(&lock).unwrap();
    // inside a project: refused, pointing to edit_case
    let p = d.join("proj");
    std::fs::create_dir_all(p.join("cases")).unwrap();
    std::fs::write(p.join("rcfd-project.json"), "{}").unwrap();
    let e = error(
        &svc,
        "case.save",
        json!({"session": s, "path": p.join("cases").join("c.toml"), "base_hash": null}),
    );
    assert_eq!(e["code"], 1004, "{e}");
    assert_eq!(e["data"]["use"], "edit_case");
    assert!(!p.join("cases").join("c.toml").exists());
    // parameters
    for bad in [
        json!({"session": s, "path": d.join("x.txt"), "base_hash": null}),
        json!({"session": s, "path": d.join("x.toml")}),
        json!({"session": s, "path": d.join("x.toml"), "base_hash": 3}),
        json!({"session": s, "path": d.join("x.toml"), "base_hash": null, "form": "all"}),
    ] {
        assert_eq!(error(&svc, "case.save", bad.clone())["code"], -32602, "{bad}");
    }
    std::fs::remove_dir_all(&d).ok();
}

/// C4: with a sandbox root, a path outside it is 1004.
#[test]
fn c4_save_stays_in_the_sandbox() {
    let d = tmp("sandbox");
    let svc = with_ui(ServiceConfig {
        root: Some(d.join("root")),
        ..ServiceConfig::default()
    });
    std::fs::create_dir_all(d.join("root")).unwrap();
    let s = session(&svc, PISO);
    let e = error(
        &svc,
        "case.save",
        json!({"session": s, "path": d.join("out.toml"), "base_hash": null}),
    );
    assert_eq!(e["code"], 1004, "{e}");
    ok(
        &svc,
        "case.save",
        json!({"session": s, "path": "in.toml", "base_hash": null}),
    );
    assert!(d.join("root").join("in.toml").is_file());
    std::fs::remove_dir_all(&d).ok();
}

/// The library facts of a case text: `dt_in_text` follows the text, not the value.
#[test]
fn facts_in_text_follow_the_text() {
    let text = PISO.replace("dt = 0.0625\n", "");
    let case = Case::parse_toml(&text).unwrap();
    let raw = facts::text_tree(&text).unwrap();
    let f = facts::case_facts(&case, InText::Text(&raw), None).unwrap();
    assert!(!f.dt_in_text && f.end_in_text);
    let tp = facts::plan(&f, &RunTime::default()).unwrap();
    assert_eq!(tp.over_dt, None);
}
