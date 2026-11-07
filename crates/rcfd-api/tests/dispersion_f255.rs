//! F-255 (ADR 0208): particle dispersion from the turbulence model's fields, in the case tree and the run path. This file
//! starts with the records R8 and R9 of ADR 0208 (checks D2, D3), made on main d953cd6 before any code of F-255; the
//! registered checks CT1 to CT8 and the parts of D2 and D3 that need the code follow them. The limits are those of ADR 0208,
//! fixed before the code ran.

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::sim::Simulation;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Case TC of ADR 0208: the box of case T (`box_3d`, 5 x 5 x 5 cells of 80 m, every wall at rest), the k-epsilon model at
/// its uniform initial state (k 1.5, epsilon 0.225), a cloud of 20 000 tracer parcels (d 1.8e-5 m, tau 1e-3 s) released at
/// the centre, with the random walk on the constants `dispersion_k`, `dispersion_epsilon` of ADR 0057.
const TC_CASE: &str = r#"
title = "dispersion TC"
[mesh]
generator = "box_3d"
nx = 5
ny = 5
nz = 5
lx = 400.0
ly = 400.0
lz = 400.0
[physics]
nu = 1.5e-5
rho = 1.2
[turbulence]
model = "k_epsilon"
k_initial = 1.5
epsilon_initial = 0.225
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[boundaries.zmin]
velocity = "fixed"
[boundaries.zmax]
velocity = "fixed"
[solver]
algorithm = "simple"
max_iterations = 2
dt = 0.01
[particles]
enabled = true
density = 1000.0
drag = "stokes"
integration = "analytic"
gravity = false
end_time = 10.0
dt = 0.05
dispersion = "drw"
dispersion_k = 1.5
dispersion_epsilon = 0.225
dispersion_seed = 20261107
[particles.injection.c]
kind = "point"
position = [200.0, 200.0, 200.0]
rate = 1.0e8
end_time = 2.0e-4
diameter = 1.8e-5
"#;

/// Case TCR of ADR 0208: TC with 2000 parcels tracked to 2 s (the run-path record R9).
fn tcr_text() -> String {
    TC_CASE
        .replace("rate = 1.0e8", "rate = 1.0e7")
        .replace("end_time = 10.0", "end_time = 2.0")
}

fn parse(t: &str) -> Case {
    Case::parse_toml(t).unwrap_or_else(|e| panic!("{e:?}"))
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f255_{tag}_{}", std::process::id()));
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

/// ADR 0208 R8: the settings content hash of case TC on main d953cd6 (a settings hash has no platform value).
const R8: u64 = 0xfcb3_9f54_9396_9989;

/// ADR 0208 R9: FNV-1a 64 of the names and bytes of the particle output files of case TCR through the run path on main
/// d953cd6 (Windows; the flow and the particles call `exp`, whose last bits may differ elsewhere, ADR 0137).
const R9: u64 = 0x9897_7560_c22d_623c;

/// ADR 0208 R8 (check D2): the settings hash of case TC is that of the base code. `RCFD_F255_RECORD=1` prints the value
/// only.
#[test]
fn d2_dispersion_case_settings_hash_matches_the_base_record() {
    let h = parse(TC_CASE).tree.content_hash();
    println!("R8 {h:016x}");
    if std::env::var("RCFD_F255_RECORD").as_deref() == Ok("1") {
        return;
    }
    assert_eq!(h, R8, "R8 {h:016x} differs from the base record {R8:016x}");
}

/// ADR 0208 R9 (check D3): a random-walk run on the constants writes the same particle files as on the base code. Compared
/// on Windows, printed elsewhere (ADR 0137). `RCFD_F255_RECORD=1` prints the value only.
#[test]
fn d3_dispersion_run_outputs_match_the_base_record() {
    let dir = tmp("d3");
    run_path(&tcr_text(), &dir);
    let (h, names) = particle_files_hash(&dir);
    println!("R9 {h:016x} over {names:?}");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(names.iter().any(|n| n == "particles.vtp"), "set-up: {names:?}");
    if std::env::var("RCFD_F255_RECORD").as_deref() == Ok("1") || !cfg!(windows) {
        return;
    }
    assert_eq!(h, R9, "R9 {h:016x} differs from the base record {R9:016x}");
}
