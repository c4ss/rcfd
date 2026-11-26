//! ADR 0812 (F-823): `case.facts` refuses a generator mesh over its cell limit by the counts' estimate
//! (`mesh_preview::estimate`) before it builds it, with the reply the build would give (1004, "mesh has N cells, limit is
//! L"); a mesh file, the generators the estimate does not name and callers without a limit build as before.
//!
//! E1: the estimate is the built count (two routes). E2: the over-limit cavity is refused in a fraction of its build's
//! time, with the built count's message. E3: the refusal alone (the memory row is read by a sampler outside the test).
//! E4: at and over a small limit. E5: without a limit the mesh is built. E6: mesh files and other generators are built,
//! then refused.

use rcfd_api::case::Case;
use rcfd_api::facts::{self, FactsError, InText};
use rcfd_api::mesh_preview::estimate;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_api::sim;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// ADR 0737's transient cavity, 16 x 16 cells.
const PISO: &str = r#"title = "piso cavity"
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

/// `PISO` with `nx` x `ny` cells.
fn cavity(nx: u64, ny: u64) -> String {
    PISO.replace("nx = 16\nny = 16", &format!("nx = {nx}\nny = {ny}"))
}

/// The cavity of 1500 x 1500 cells, steady (no `Simulation` is built for its facts).
fn big() -> String {
    cavity(1500, 1500).replace("algorithm = \"piso\"", "algorithm = \"simple\"")
}

fn gen_case(g: &str, extra: &[(&str, i64)]) -> Case {
    let mut case = Case::new();
    case.set("mesh.generator", json!(g)).unwrap();
    case.set("mesh.nx", json!(8)).unwrap();
    case.set("mesh.ny", json!(4)).unwrap();
    for (k, v) in extra {
        case.set(k, json!(v)).unwrap();
    }
    case
}

/// The eleven generators `estimate` names, with ADR 0737 C3's settings and right counts that differ from the left.
fn gens() -> Vec<(&'static str, Case)> {
    let set: Vec<(&str, Vec<(&str, i64)>)> = vec![
        ("cavity_2d", vec![]),
        ("box_2d", vec![]),
        ("channel_2d", vec![]),
        ("channel_2d_interface", vec![("mesh.nx_right", 5), ("mesh.ny_right", 3)]),
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
        (
            "annulus_3d_interface",
            vec![
                ("mesh.r_in", 1),
                ("mesh.r_out", 2),
                ("mesh.nz", 2),
                ("mesh.ny_right", 3),
                ("mesh.nz_right", 3),
            ],
        ),
        (
            "annulus_3d_stage",
            vec![
                ("mesh.r_in", 1),
                ("mesh.r_out", 2),
                ("mesh.nz", 2),
                ("mesh.nx_right", 6),
                ("mesh.ny_right", 3),
                ("mesh.nz_right", 3),
            ],
        ),
        (
            "annulus_3d_cylindrical_interface",
            vec![
                ("mesh.r_in", 1),
                ("mesh.r_out", 3),
                ("mesh.r_interface", 2),
                ("mesh.nz", 2),
                ("mesh.ny_right", 3),
            ],
        ),
    ];
    set.into_iter().map(|(g, e)| (g, gen_case(g, &e))).collect()
}

/// The generators of ADR 0737 C3 that `estimate` does not name, with C3's settings.
fn others() -> Vec<(&'static str, Case)> {
    let set: Vec<(&str, Vec<(&str, i64)>)> = vec![
        ("cylinder_ogrid", vec![("mesh.r_in", 1), ("mesh.r_out", 4)]),
        ("pipe_sweep", vec![("mesh.r_out", 1), ("mesh.nz", 2)]),
        ("box_3d", vec![("mesh.nz", 2)]),
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
    set.into_iter().map(|(g, e)| (g, gen_case(g, &e))).collect()
}

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

fn service(max_cells: Option<usize>) -> Arc<Service> {
    let mut cfg = ServiceConfig::default();
    if let Some(m) = max_cells {
        cfg.max_cells = m;
    }
    let s = Service::new(cfg);
    s.enable_ui();
    s
}

fn call(svc: &Arc<Service>, method: &str, params: Value) -> Value {
    svc.handle_request(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}))
        .unwrap()
}

fn session(svc: &Arc<Service>, source: Value) -> String {
    let r = call(svc, "session.create", source);
    assert!(r.get("error").is_none(), "session.create: {r}");
    r["result"]["session"].as_str().unwrap().to_string()
}

fn toml_session(svc: &Arc<Service>, toml: &str) -> String {
    session(svc, json!({"toml": toml}))
}

fn case_session(svc: &Arc<Service>, case: &Case) -> String {
    let settings: Value = serde_json::from_str(&case.to_json_string()).unwrap();
    session(svc, json!({"settings": settings}))
}

fn facts_of(svc: &Arc<Service>, id: &str) -> Value {
    call(svc, "case.facts", json!({"session": id}))
}

fn built(case: &Case) -> u64 {
    sim::build_mesh(case).unwrap().n_cells() as u64
}

fn assert_refused(reply: &Value, cells: u64, limit: u64, what: &str) {
    let e = &reply["error"];
    assert_eq!(e["code"], 1004, "{what}: {reply}");
    assert_eq!(
        e["message"].as_str(),
        Some(format!("mesh has {cells} cells, limit is {limit}").as_str()),
        "{what}: {reply}"
    );
}

/// E1: for every shipped case with an estimate and each of the eleven generators it names, the estimate is the count of
/// the mesh `build_mesh` builds.
#[test]
fn e1_the_estimate_is_the_built_count() {
    let mut n = 0;
    for file in shipped() {
        let case = Case::load(&file).unwrap();
        if let Some(e) = estimate(&case) {
            let b = built(&case);
            assert_eq!(e, b, "{}", file.display());
            eprintln!("E1 {}: estimate {e}, built {b}", file.display());
            n += 1;
        }
    }
    for (g, case) in gens() {
        let e = estimate(&case);
        let b = built(&case);
        assert_eq!(e, Some(b), "{g}");
        eprintln!("E1 {g}: estimate {e:?}, built {b}");
        n += 1;
    }
    eprintln!("E1: {n} cases, all equal");
}

/// E2: the 2,250,000-cell cavity is refused with the message of its built count, in at most 1 s and at most half the
/// time of building its mesh.
#[test]
fn e2_over_the_limit_is_refused_before_a_build() {
    let svc = service(None);
    let id = toml_session(&svc, &big());
    let t = Instant::now();
    let reply = facts_of(&svc, &id);
    let t_r = t.elapsed().as_secs_f64();
    let case = Case::parse_toml(&big()).unwrap();
    let t = Instant::now();
    let n_built = built(&case);
    let t_b = t.elapsed().as_secs_f64();
    eprintln!(
        "E2: T_r {t_r:.6} s, T_b {t_b:.3} s, ratio {:.6}; built {n_built}; {reply}",
        t_r / t_b
    );
    assert_refused(&reply, n_built, 2_000_000, "E2");
    assert!(t_r <= 1.0, "T_r {t_r} s");
    assert!(t_r / t_b <= 0.5, "T_r / T_b = {}", t_r / t_b);
}

/// E3: the refusal of E2 alone (for the peak working set read outside the test).
#[test]
fn e3_the_refusal_alone() {
    let svc = service(None);
    let id = toml_session(&svc, &big());
    let reply = facts_of(&svc, &id);
    assert_refused(&reply, 2_250_000, 2_000_000, "E3");
}

/// E4: at the limit the facts are given; over it the reply is the built count's refusal, for the cavity and for each
/// generator the estimate names.
#[test]
fn e4_at_and_over_a_small_limit() {
    let svc = service(Some(100));
    let id = toml_session(&svc, &cavity(10, 10));
    let r = facts_of(&svc, &id);
    assert!(r.get("error").is_none(), "10 x 10 at 100: {r}");
    assert_eq!(r["result"]["cells"], 100, "{r}");
    let id = toml_session(&svc, &cavity(101, 1));
    assert_refused(&facts_of(&svc, &id), 101, 100, "101 x 1 at 100");
    let svc = service(Some(20));
    for (g, case) in gens() {
        let id = case_session(&svc, &case);
        assert_refused(&facts_of(&svc, &id), built(&case), 20, g);
    }
}

/// E5: without a limit `case_facts` builds the 2,250,000-cell mesh as before.
#[test]
fn e5_without_a_limit_the_mesh_is_built() {
    let case = Case::parse_toml(&big()).unwrap();
    let f = facts::case_facts(&case, InText::NotDefault, None).unwrap();
    assert_eq!(f.cells, 2_250_000);
}

/// E6: a mesh file (its counts ignored) and the generators the estimate does not name are built, then refused by their
/// built count.
#[test]
fn e6_a_mesh_file_and_other_generators_are_built_then_refused() {
    let msh = repo().join("crates/rcfd-io/tests/data/hex_v2.2_ascii.msh");
    let mut file = Case::new();
    file.set("mesh.source", json!("file")).unwrap();
    file.set("mesh.file", json!(msh.display().to_string())).unwrap();
    file.set("mesh.nx", json!(1_000_000)).unwrap();
    file.set("mesh.ny", json!(1_000_000)).unwrap();
    assert_eq!(estimate(&file), None);
    let mut cases = vec![("mesh file", file)];
    cases.extend(others());
    for (what, case) in cases {
        let b = built(&case);
        let r = facts::case_facts(&case, InText::NotDefault, Some(10));
        assert_eq!(r, Err(FactsError::TooLarge { cells: b, limit: 10 }), "{what}");
        eprintln!("E6 {what}: built {b}, refused at 10");
    }
}
