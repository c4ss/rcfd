//! The real `rcfd` binary drives the particle tracking (F-056, ADR 0037 checks W5 exit codes and W7 binary part).

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::sim::Simulation;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_cliprun_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn text(extra: &str) -> String {
    format!(
        r#"
title = "particle run"
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
integration = "analytic"
gravity = false
end_time = 0.4
dt = 0.01
tracks = true
track_every = 2
[particles.injection.p]
kind = "point"
position = [0.3, 0.7, 0.0]
rate = 50.0
end_time = 0.2
velocity_mode = "fluid"
diameter = 1.0e-3
{extra}
"#
    )
}

fn rcfd(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn run_path(t: &str, dir: &Path) {
    let sim = Simulation::new(&Case::parse_toml(t).unwrap()).unwrap();
    let m = Mutex::new(sim);
    let opts = RunOptions {
        out_dir: Some(dir.to_path_buf()),
        ..Default::default()
    };
    runctl::drive(&m, &opts, &|| false, &mut |_| {});
    runctl::write_results(&m.into_inner().unwrap(), dir).unwrap();
}

#[test]
fn w7_binary_writes_the_same_particle_files_as_the_run_path() {
    let d = tmp("w7");
    let case = d.join("c.toml");
    std::fs::write(&case, text("")).unwrap();
    let out = d.join("out");
    let o = rcfd(&["run", case.to_str().unwrap(), "--out", out.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let reference = d.join("ref");
    run_path(&text(""), &reference);
    for f in ["particles.vtp", "particle_report.json", "particle_tracks.csv"] {
        assert_eq!(
            std::fs::read(out.join(f)).unwrap(),
            std::fs::read(reference.join(f)).unwrap(),
            "{f} differs between the binary and the in-process run path"
        );
    }
    assert!(out.join("solution.vtu").exists());
    // a case without particles has no particle files
    let off = text("").replace("enabled = true", "enabled = false");
    std::fs::write(&case, off).unwrap();
    let out2 = d.join("out2");
    let o = rcfd(&["run", case.to_str().unwrap(), "--out", out2.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0));
    let mut names: Vec<String> = std::fs::read_dir(&out2)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    // the binary additionally writes its restart file for a restartable run (existing behaviour)
    assert_eq!(
        names,
        [
            "monitors.csv",
            "monitors.json",
            "report.html",
            "restart.rcfd",
            "solution.vtu"
        ]
    );
    // with particles the run is not restartable (W8): no checkpoint file is written
    assert!(!out.join("restart.rcfd").exists());
}

#[test]
fn w5_check_only_exits_3_for_mesh_dependent_particle_errors() {
    let d = tmp("w5");
    let cases = [
        (
            "position",
            text("").replace("[0.3, 0.7, 0.0]", "[5.0, 5.0, 0.0]"),
            "particles.injection.p.position",
        ),
        (
            "patch",
            text("")
                .replace("kind = \"point\"", "kind = \"surface\"\npatch = \"nope\"")
                .replace("position = [0.3, 0.7, 0.0]\n", ""),
            "particles.injection.p.patch",
        ),
        (
            "wall",
            format!("{}\n[particles.walls.nope]\ninteraction = \"stick\"\n", text("")),
            "particles.walls.nope",
        ),
        (
            "every",
            text("").replace("track_every = 2", "track_every = 0"),
            "particles.track_every",
        ),
    ];
    for (tag, t, path) in cases {
        let f = d.join(format!("{tag}.toml"));
        std::fs::write(&f, t).unwrap();
        let o = rcfd(&["run", f.to_str().unwrap(), "--check-only"]);
        assert_eq!(o.status.code(), Some(3), "{tag}");
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(err.contains(path), "{tag}: {err}");
    }
    let f = d.join("ok.toml");
    std::fs::write(&f, text("")).unwrap();
    assert_eq!(
        rcfd(&["run", f.to_str().unwrap(), "--check-only"]).status.code(),
        Some(0)
    );
}

/// F11 (binary part, ADR 0037): `rcfd run` of a fractional-step case writes solution.vtu and no checkpoint.
#[test]
fn f11_binary_runs_a_fractional_step_case() {
    let d = tmp("f11");
    let l = 2.0 * std::f64::consts::PI;
    let t = format!(
        "[mesh]\ngenerator = \"box_2d\"\nnx = 16\nny = 16\nlx = {l:.17e}\nly = {l:.17e}\nperiodic_x = true\nperiodic_y = true\n[physics]\nnu = 0.05\n[initial]\nvelocity_expression_x = \"sin(x/(1 [m]))*cos(y/(1 [m])) * 1 [m/s]\"\nvelocity_expression_y = \"-cos(x/(1 [m]))*sin(y/(1 [m])) * 1 [m/s]\"\n[solver]\nalgorithm = \"piso\"\nconvection = \"central\"\ntime_advance = \"fractional_step\"\ndt = 0.02\nend_time = 0.2\n"
    );
    let case = d.join("c.toml");
    std::fs::write(&case, t).unwrap();
    let out = d.join("out");
    let o = rcfd(&["run", case.to_str().unwrap(), "--out", out.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(out.join("solution.vtu").exists());
    assert!(!out.join("restart.rcfd").exists());
}
