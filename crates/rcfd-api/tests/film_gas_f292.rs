//! F-292 (ADR 0206): gas buoyancy and gas shear in the particle wall film, in the case tree and the run path. This file
//! starts with the records R6 and R7 of ADR 0206 (check D2, D3), made on main 8db45fb before any code of F-292. The limits
//! are those of ADR 0206, fixed before the code ran.

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::sim::Simulation;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Case K of ADR 0192 C2 as corrected by its amendment 1 (with `[energy]` and `[initial]`): a box_2d incline (gravity 9.81
/// at 30 degrees from -y), frozen flow at rest, the film on `ymin`, one point injection that lands on the centre of the
/// first `ymin` face.
const K_CASE: &str = r#"
title = "wall film K"
[mesh]
generator = "box_2d"
nx = 10
ny = 2
lx = 0.05
ly = 0.01
[physics]
nu = 1.5e-5
rho = 1.2
gravity = [4.905, -8.495709211125343, 0.0]
[energy]
enabled = true
[initial]
temperature = 300.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simple"
frozen_flow = true
max_iterations = 2
dt = 0.01
[particles]
enabled = true
density = 1000.0
drag = "none"
integration = "analytic"
end_time = 20.0
dt = 0.01
[particles.film]
enabled = true
patches = ["ymin"]
viscosity = 1.0e-3
[particles.injection.f]
kind = "point"
position = [1.3452994616207485e-3, 2.0e-3, 2.5e-3]
rate = 730.0
diameter = 1.0e-4
mass_flow = 2.5e-6
"#;

fn parse(t: &str) -> Case {
    Case::parse_toml(t).unwrap_or_else(|e| panic!("{e:?}"))
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f292_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The run path of the library (`drive` plus `write_results`) into `dir`.
fn run_path(t: &str, dir: &Path) -> Simulation {
    let case = parse(t);
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

/// FNV-1a 64 of the particle output files of `dir` (names sorted, each name then its bytes), as ADR 0192 R2.
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

/// ADR 0206 R6: FNV-1a 64 of the names and bytes of the particle output files of case K through the run path on main
/// 8db45fb (Windows; the flow and the particles call `exp`, whose last bits may differ elsewhere, ADR 0137).
const R6: u64 = 0x2a6c_2d99_f94d_77dc;

/// ADR 0206 R7: the settings content hash of case K on main 8db45fb (a settings hash has no platform value).
const R7: u64 = 0x9517_5a46_b994_1edc;

/// ADR 0206 R6 (check D2): a film run without the gas keys writes the same particle files as on the base code. Compared
/// on Windows, printed elsewhere (ADR 0137). `RCFD_F292_RECORD=1` prints the value only.
#[test]
fn d2_film_run_outputs_match_the_base_record() {
    let dir = tmp("d2");
    run_path(K_CASE, &dir);
    let (h, names) = particle_files_hash(&dir);
    println!("R6 {h:016x} over {names:?}");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(names.iter().any(|n| n == "particle_film.csv"), "set-up: {names:?}");
    if std::env::var("RCFD_F292_RECORD").as_deref() == Ok("1") || !cfg!(windows) {
        return;
    }
    assert_eq!(h, R6, "R6 {h:016x} differs from the base record {R6:016x}");
}

/// ADR 0206 R7 (check D3): the settings hash of case K is that of the base code. `RCFD_F292_RECORD=1` prints the value
/// only.
#[test]
fn d3_film_case_settings_hash_matches_the_base_record() {
    let h = parse(K_CASE).tree.content_hash();
    println!("R7 {h:016x}");
    if std::env::var("RCFD_F292_RECORD").as_deref() == Ok("1") {
        return;
    }
    assert_eq!(h, R7, "R7 {h:016x} differs from the base record {R7:016x}");
}
