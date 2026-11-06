//! F-254 (ADR 0194): particle wall interaction, erosion and rough-wall rebound, in the case tree and the run path. This
//! file holds the record R4 of the particle output files of a steady run with per-patch wall actions (made on the base
//! code main 54ec840 before any code of F-254).

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::sim::Simulation;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// ADR 0194 R4: FNV-1a 64 of the names and bytes of the particle output files of the R4 run on main 54ec840 (Windows;
/// the flow and the particles call `exp`, whose last bits may differ elsewhere, ADR 0137).
const R4: u64 = 0x5514_d633_2741_d3a4;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f254_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The R4 case: a steady run on a prescribed shear flow in air (`solver.frozen_flow`, `u = 2 y` [1/s] as in the Couette
/// case of `particles_case.rs`, box 12 x 12), particles tracked once on that field by the run path, with gravity, per-patch
/// wall actions (`ymin` sticks, `ymax` reflects with restitution 0.5, `xmax` lets them escape, `xmin` reflects with 0.9),
/// two injections (Rosin-Rammler sizes with a given velocity; fluid-velocity parcels), the track log and numbered VTP
/// files.
const R4_CASE: &str = r#"
title = "particle run R4"
[mesh]
generator = "box_2d"
nx = 12
ny = 12
lx = 1.0
ly = 1.0
[physics]
nu = 1.5e-5
rho = 1.2
gravity = [0.0, -9.81, 0.0]
[energy]
enabled = true
[initial]
temperature = 300.0
velocity_expression_x = "2 [1/s] * y"
[boundaries.xmin]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.xmax]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
velocity_value = [2.0, 0.0, 0.0]
[solver]
algorithm = "simple"
frozen_flow = true
max_iterations = 2
dt = 0.01
[particles]
enabled = true
density = 1500.0
drag = "schiller_naumann"
integration = "rk4"
end_time = 1.0
dt = 0.01
restitution = 0.9
tracks = true
track_every = 5
output_every = 25
[particles.walls.ymin]
interaction = "stick"
[particles.walls.ymax]
interaction = "reflect"
restitution = 0.5
[particles.walls.xmax]
interaction = "escape"
[particles.injection.a]
kind = "point"
position = [0.5, 0.8, 0.04]
rate = 120.0
end_time = 0.6
velocity = [0.8, 2.5, 0.0]
distribution = "rosin_rammler"
rr_d_bar = 2.0e-4
rr_n = 3.0
[particles.injection.b]
kind = "point"
position = [0.2, 0.3, 0.04]
rate = 60.0
velocity_mode = "fluid"
diameter = 1.5e-4
parcel_count = 10.0
"#;

/// The run path of the library (`drive` plus `write_results`) into `dir`.
fn run_path(t: &str, dir: &Path) -> Simulation {
    let case = Case::parse_toml(t).unwrap_or_else(|e| panic!("{e:?}"));
    let sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let m = Mutex::new(sim);
    let opts = RunOptions {
        out_dir: Some(dir.to_path_buf()),
        ..Default::default()
    };
    let out = runctl::drive(&m, &opts, &|| false, &mut |_| {});
    assert!(out.exit_code == 0 || out.exit_code == 5, "{out:?}");
    let sim = m.into_inner().unwrap();
    runctl::write_results(&sim, dir).unwrap();
    sim
}

/// FNV-1a 64 of the particle output files of `dir` (names sorted, each name then its bytes).
fn particle_files_hash(dir: &Path) -> (u64, Vec<String>) {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("particle"))
        .collect();
    names.sort();
    let mut h = rcfd_core::hash::Fnv64::new();
    for n in &names {
        h.write(n.as_bytes());
        h.write(&std::fs::read(dir.join(n)).unwrap());
    }
    (h.finish(), names)
}

/// ADR 0194 R4 (check E2): a steady particle run with per-patch wall actions and no F-254 model writes the same files as on
/// the base code. Compared on Windows, printed elsewhere (ADR 0137). `RCFD_F254_RECORD=1` prints the value only.
#[test]
fn e2_wall_actions_run_outputs_match_the_base_record() {
    let dir = tmp("e2");
    run_path(R4_CASE, &dir);
    let (h, names) = particle_files_hash(&dir);
    let report = std::fs::read_to_string(dir.join("particle_report.json")).unwrap_or_default();
    println!("R4 {h:016x} over {names:?}\nR4 report {report}");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(names.len() >= 4, "set-up: {names:?}");
    let rep: serde_json::Value = serde_json::from_str(&report).unwrap();
    let c = &rep["counts"];
    assert!(
        c["deposited"].as_u64().unwrap_or(0) > 0 && c["escaped"].as_u64().unwrap_or(0) > 0,
        "R4 set-up: {c}"
    );
    if std::env::var("RCFD_F254_RECORD").as_deref() == Ok("1") || !cfg!(windows) {
        return;
    }
    assert_eq!(h, R4, "R4 {h:016x} differs from the base record {R4:016x}");
}
