//! ADR 0373 (F-476): the field store of a run with `[mesh.adapt]` is opened on the mesh the run solves on. A1, A2: the
//! shipped adaptive cavity with `output.store_interval` set stores the adapted mesh (2799 cells) with no warning. D3: a run
//! stopped before its first step stores the base lattice (576 cells), as before. N1 (negative control): an unknown stored
//! field is refused with the store's warning and no store file.

use rcfd_api::case::Case;
use rcfd_api::chain::apply_set;
use rcfd_api::runctl::{drive, RunOptions};
use rcfd_api::sim::Simulation;
use rcfd_tsstore::{Reader, STORE_FILE};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd-f476-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// The shipped case with `sets`, driven `max_steps` steps with the runner's folders under `dir`; the simulation after
/// the run and the warning messages.
fn run(sets: &[&str], max_steps: usize, dir: &Path) -> (Simulation, Vec<String>) {
    let mut case = Case::load(repo().join("cases/cavity/adapt_re100.toml")).unwrap_or_else(|d| panic!("{d:?}"));
    for s in sets {
        apply_set(&mut case, s).unwrap_or_else(|e| panic!("{s}: {e:?}"));
    }
    let m = Mutex::new(Simulation::new(&case).unwrap_or_else(|d| panic!("{d:?}")));
    let opts = RunOptions {
        max_steps: Some(max_steps),
        out_dir: Some(dir.join("output")),
        store_dir: Some(dir.join("fields")),
        status_monitors: true,
        ..Default::default()
    };
    let mut warnings = Vec::new();
    let o = drive(&m, &opts, &|| false, &mut |e: &Value| {
        if e["event"] == "warning" {
            warnings.push(e["message"].as_str().unwrap_or_default().to_string());
        }
    });
    eprintln!("  drive: {} after {} steps; warnings {warnings:?}", o.state, o.iterations);
    (m.into_inner().unwrap(), warnings)
}

#[test]
fn a1_the_shipped_adaptive_case_stores_its_adapted_mesh() {
    let dir = tmp("a1");
    let (sim, warnings) = run(&["output.store_interval=20", "output.store_precision=f32"], 3, &dir);
    // A2: no warning, of the store or any other
    let store_warnings: Vec<_> = warnings.iter().filter(|w| w.contains("field store")).collect();
    eprintln!(
        "A2: {} store warning(s), {} warning(s) in all",
        store_warnings.len(),
        warnings.len()
    );
    assert!(store_warnings.is_empty(), "A2 {store_warnings:?}");
    assert!(warnings.is_empty(), "A2 {warnings:?}");
    // A1: the store opens on the adapted mesh
    let mut r = Reader::open(&dir.join("fields")).unwrap_or_else(|e| panic!("A1 {e}"));
    let n = sim.geom.n_cells();
    let snaps: Vec<u64> = r.snapshots().iter().map(|s| s.step).collect();
    eprintln!(
        "A1: store n_cells {}, mesh hash {:016x} (run {n} cells, {:016x}), snapshots at steps {snaps:?}",
        r.n_cells(),
        r.mesh_hash(),
        sim.geom.mesh.content_hash()
    );
    assert_eq!(n, 2799, "A1 the run's final mesh");
    assert_eq!(r.n_cells(), 2799, "A1 store cells");
    assert_eq!(r.mesh_hash(), sim.geom.mesh.content_hash(), "A1 store mesh hash");
    assert_eq!(r.mesh().unwrap().n_cells(), 2799, "A1 stored mesh");
    assert!(!snaps.is_empty(), "A1 no snapshot");
    for k in 0..snaps.len() {
        let names: Vec<String> = r.fields(k).unwrap().iter().map(|f| f.name.clone()).collect();
        assert_eq!(names, ["ux", "uy", "uz", "p"], "A1 snapshot {k}");
        for name in &names {
            assert_eq!(r.read_field(k, name).unwrap().len(), 2799, "A1 snapshot {k} {name}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn d3_an_adaptive_run_stopped_before_its_first_step_stores_the_base_lattice() {
    let dir = tmp("d3");
    let (_, warnings) = run(&["output.store_interval=20"], 0, &dir);
    assert!(warnings.is_empty(), "D3 {warnings:?}");
    let r = Reader::open(&dir.join("fields")).unwrap_or_else(|e| panic!("D3 {e}"));
    let snaps: Vec<u64> = r.snapshots().iter().map(|s| s.step).collect();
    eprintln!("D3: store n_cells {}, snapshots at steps {snaps:?}", r.n_cells());
    assert_eq!(r.n_cells(), 576, "D3 store cells");
    assert_eq!(snaps, [0], "D3 snapshots");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn n1_an_unknown_stored_field_is_refused_after_the_first_step() {
    let dir = tmp("n1");
    let (_, warnings) = run(
        &["output.store_interval=20", "output.store_fields=[\"nosuch\"]"],
        3,
        &dir,
    );
    eprintln!("N1: {warnings:?}");
    assert!(warnings.iter().any(|w| w.contains("field store disabled")), "N1 {warnings:?}");
    assert!(!dir.join("fields").join(STORE_FILE).exists(), "N1 store file");
    let _ = std::fs::remove_dir_all(&dir);
}
