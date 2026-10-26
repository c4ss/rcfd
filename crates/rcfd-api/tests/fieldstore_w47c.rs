//! ADR 0124 checks H1-H4: the opt-in field store of the run path (`output.store_interval`,
//! `output.store_time_interval`, `output.store_fields`), written by `runctl::drive` into `<out_dir>/fields/`.

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::sim::Simulation;
use rcfd_tsstore::{IndexSource, Reader};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

fn cavity(alg: &str, extra: &str) -> String {
    // smoke (ADR 0124): the steady run diverged at iteration 14 with the default relaxation; 0.7 / 0.3 settles
    let relax = if alg == "simple" {
        "alpha_u = 0.7\nalpha_p = 0.3\n"
    } else {
        ""
    };
    format!(
        "[mesh]\ngenerator = \"cavity_2d\"\nnx = 12\nny = 12\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"{alg}\"\n{relax}max_iterations = 60\nend_time = 0.3\ndt = 0.01\n[output]\n{extra}\n"
    )
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd-w47c-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// Drive the case into `out` (or without an output directory); returns the simulation and the events.
fn drive(text: &str, out: Option<&Path>) -> (Simulation, Vec<Value>) {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let m = Mutex::new(Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}")));
    let opts = RunOptions {
        out_dir: out.map(Path::to_path_buf),
        ..Default::default()
    };
    let mut events = Vec::new();
    let o = runctl::drive(&m, &opts, &|| false, &mut |e| events.push(e.clone()));
    assert!(o.exit_code == 0 || o.exit_code == 5, "{o:?}");
    (m.into_inner().unwrap(), events)
}

const NAMES: [&str; 4] = ["ux", "uy", "uz", "p"];

fn bits(sim: &Simulation) -> Vec<Vec<u64>> {
    NAMES
        .iter()
        .map(|n| sim.field(n).unwrap().iter().map(|x| x.to_bits()).collect())
        .collect()
}

fn stored_bits(r: &mut Reader, i: usize) -> Vec<Vec<u64>> {
    NAMES
        .iter()
        .map(|n| r.read_field(i, n).unwrap().iter().map(|x| x.to_bits()).collect())
        .collect()
}

fn warnings(ev: &[Value]) -> Vec<String> {
    ev.iter()
        .filter(|e| e["event"] == "warning")
        .map(|e| e["message"].to_string())
        .collect()
}

#[test]
fn h1_store_off_writes_nothing_and_on_does_not_change_the_run() {
    for alg in ["piso", "simple"] {
        let off_dir = tmp(&format!("h1-off-{alg}"));
        let on_dir = tmp(&format!("h1-on-{alg}"));
        let (plain, _) = drive(&cavity(alg, ""), None);
        let (off, ev_off) = drive(&cavity(alg, ""), Some(&off_dir));
        let (on, ev_on) = drive(&cavity(alg, "store_interval = 7"), Some(&on_dir));
        // off: no store, no extra events; on: the same solution bit for bit
        assert!(!off_dir.join("fields").exists());
        assert_eq!(bits(&plain), bits(&off));
        assert_eq!(bits(&off), bits(&on));
        assert_eq!(off.iteration(), on.iteration());
        assert_eq!(ev_off.len(), ev_on.len());
        assert!(warnings(&ev_on).is_empty(), "{:?}", warnings(&ev_on));
        let mut r = Reader::open(&on_dir.join("fields")).unwrap();
        assert_eq!(r.index_source(), &IndexSource::Footer);
        let last = on.iteration() as u64;
        let mut want: Vec<u64> = (1..=last).filter(|s| s % 7 == 0).collect();
        if want.last() != Some(&last) {
            want.push(last);
        }
        let steps: Vec<u64> = r.snapshots().iter().map(|s| s.step).collect();
        assert_eq!(steps, want, "{alg}");
        let k = steps.len() - 1;
        assert_eq!(r.snapshots()[k].time.to_bits(), on.time().to_bits());
        // the stored final snapshot is the solver's final state, bit for bit
        assert_eq!(stored_bits(&mut r, k), bits(&on), "{alg}");
        // negative control: the previous snapshot is a different state
        assert_ne!(stored_bits(&mut r, k - 1), bits(&on), "{alg}");
        assert_eq!(r.n_cells(), on.geom.mesh.n_cells());
        assert_eq!(r.mesh_hash(), on.geom.mesh.content_hash());
        let units: Vec<String> = r.fields(k).unwrap().into_iter().map(|f| f.unit).collect();
        assert_eq!(units, ["m/s", "m/s", "m/s", "unknown"]);
        println!("H1 {alg}: {} iterations, stored steps {steps:?}", on.iteration());
        let _ = std::fs::remove_dir_all(&off_dir);
        let _ = std::fs::remove_dir_all(&on_dir);
    }
}

#[test]
fn h2_time_interval_stores_the_first_step_at_or_after_each_multiple() {
    let dir = tmp("h2");
    let (plain, _) = drive(&cavity("piso", ""), None);
    let (on, ev) = drive(
        &cavity("piso", "store_time_interval = 0.05\nstore_fields = [\"p\", \"speed\"]"),
        Some(&dir),
    );
    assert_eq!(bits(&plain), bits(&on));
    assert!(warnings(&ev).is_empty());
    let mut r = Reader::open(&dir.join("fields")).unwrap();
    let times: Vec<f64> = r.snapshots().iter().map(|s| s.time).collect();
    println!("H2 stored times {times:?}");
    assert_eq!(times.len(), 6);
    for (k, t) in times.iter().enumerate() {
        assert!((t / 0.05 - (k + 1) as f64).abs() < 1e-6, "{t}");
    }
    let names: Vec<String> = r.fields(0).unwrap().into_iter().map(|f| f.name).collect();
    assert_eq!(names, ["p", "speed"]);
    let last = times.len() - 1;
    assert_eq!(
        r.read_field(last, "speed")
            .unwrap()
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>(),
        on.field("speed")
            .unwrap()
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn h3_located_errors_and_warnings() {
    let e = Case::parse_toml(&cavity("simple", "store_time_interval = 0.1"))
        .err()
        .unwrap();
    assert!(e.iter().any(|d| d.path == "output.store_time_interval"), "{e:?}");
    // negative control: the same key on a transient run is accepted
    assert!(Case::parse_toml(&cavity("piso", "store_time_interval = 0.1")).is_ok());
    let dyn_text = format!(
        "{}[dynamic_mesh]\nenabled = true\n",
        cavity("piso", "store_interval = 5")
    );
    let e = Case::parse_toml(&dyn_text).err().unwrap();
    assert!(e.iter().any(|d| d.path == "output.store_interval"), "{e:?}");
    // an unknown field name: a warning, no store, the run itself unchanged
    let dir = tmp("h3");
    let (sim, ev) = drive(
        &cavity("piso", "store_interval = 5\nstore_fields = [\"nut\"]"),
        Some(&dir),
    );
    let w = warnings(&ev);
    assert!(
        w.iter()
            .any(|m| m.contains("field store disabled") && m.contains("turbulence")),
        "{w:?}"
    );
    assert!(!dir.join("fields").exists());
    assert_eq!(bits(&sim), bits(&drive(&cavity("piso", ""), None).0));
    // the store needs an output directory: without one nothing is written and nothing is reported
    let (_, ev) = drive(&cavity("piso", "store_interval = 5"), None);
    assert!(warnings(&ev).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn h4_fresh_run_replaces_an_earlier_store_in_the_same_directory() {
    let dir = tmp("h4");
    drive(&cavity("piso", "store_interval = 10"), Some(&dir));
    let (on, _) = drive(&cavity("piso", "store_interval = 15"), Some(&dir));
    let mut r = Reader::open(&dir.join("fields")).unwrap();
    let steps: Vec<u64> = r.snapshots().iter().map(|s| s.step).collect();
    assert_eq!(steps, [15, 30]);
    assert_eq!(stored_bits(&mut r, 1), bits(&on));
    let _ = std::fs::remove_dir_all(&dir);
}
