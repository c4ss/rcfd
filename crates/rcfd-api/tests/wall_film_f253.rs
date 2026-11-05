//! F-253 (ADR 0192): the particle wall film in the case tree and the run path. This first part holds the record R2 of
//! ADR 0192: the output files of a particle run without a film, made on the base code (main 201ea17) before any code of
//! F-253, whose bytes must stay the same.

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::sim::Simulation;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// ADR 0192 R2: FNV-1a 64 of the names and bytes of the output files of the R2 run on main 201ea17 (Windows; the flow
/// and the particles call `exp`, whose last bits may differ elsewhere, ADR 0137).
const R2: u64 = 0xce9d_f10b_3a88_5c96;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f253_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The R2 case: the 20 x 20 lid-driven box of `particle_run.rs` (piso, 40 steps of 0.01 s) with a point injection of
/// fluid-velocity parcels, reflecting walls, the track log and numbered VTP files.
const R2_CASE: &str = r#"
title = "particle run R2"
[mesh]
generator = "box_2d"
nx = 20
ny = 20
lx = 1.0
ly = 1.0
[physics]
nu = 0.01
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[solver]
algorithm = "piso"
dt = 0.01
end_time = 0.4
[particles]
enabled = true
density = 1000.0
drag = "schiller_naumann"
integration = "analytic"
gravity = false
end_time = 0.4
dt = 0.01
tracks = true
track_every = 3
output_every = 10
[particles.injection.p]
kind = "point"
position = [0.3, 0.7, 0.0]
rate = 50.0
end_time = 0.2
velocity_mode = "fluid"
diameter = 1.0e-3
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

/// ADR 0192 R2 (check D2): a particle run without a film writes the same files as on the base code. Compared on
/// Windows, printed elsewhere (ADR 0137). `RCFD_F253_RECORD=1` prints the value only.
#[test]
fn d2_no_film_run_outputs_match_the_base_record() {
    let dir = tmp("d2");
    run_path(R2_CASE, &dir);
    let (h, names) = particle_files_hash(&dir);
    println!("R2 {h:016x} over {names:?}");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(names.len() >= 4, "set-up: {names:?}");
    if std::env::var("RCFD_F253_RECORD").as_deref() == Ok("1") || !cfg!(windows) {
        return;
    }
    assert_eq!(h, R2, "R2 {h:016x} differs from the base record {R2:016x}");
}
