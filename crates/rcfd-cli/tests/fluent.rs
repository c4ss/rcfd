//! Fluent meshes through the CLI and the case loader (F-118): `check-mesh`, `convert` and a
//! laminar channel run on a 2D Fluent-format mesh written by this test.
//!
//! Criteria fixed up front: check-mesh exits 0 on a 3D and a 2D file; convert writes a .vtu;
//! the 2D channel run converges (exit 0); unsupported zone types (periodic) fail with exit 1 and
//! name the problem; skipped sections are reported on stderr, not dropped silently.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_fluent_cli_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// ASCII Fluent mesh of an `nx` x `ny` quad channel, `lx` x `ly`.
fn channel_msh(nx: usize, ny: usize, lx: f64, ly: f64, extra: &str, wall_type: &str) -> String {
    let node = |i: usize, j: usize| 1 + i + (nx + 1) * j;
    let cell = |i: usize, j: usize| 1 + i + nx * j;
    let nn = (nx + 1) * (ny + 1);
    let mut s = format!("(0 \"channel\")\n(1 \"rcfd test\")\n(2 2)\n(10 (0 1 {nn:x} 0))\n(10 (1 1 {nn:x} 1 2)(\n");
    for j in 0..=ny {
        for i in 0..=nx {
            s += &format!("{} {}\n", lx * i as f64 / nx as f64, ly * j as f64 / ny as f64);
        }
    }
    s += &format!("))\n(12 (0 1 {:x} 0))\n(12 (9 1 {:x} 1 3))\n", nx * ny, nx * ny);
    let mut zones: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut interior = Vec::new();
    for j in 0..ny {
        for i in 1..nx {
            interior.push(format!(
                "{:x} {:x} {:x} {:x}",
                node(i, j),
                node(i, j + 1),
                cell(i - 1, j),
                cell(i, j)
            ));
        }
    }
    for j in 1..ny {
        for i in 0..nx {
            interior.push(format!(
                "{:x} {:x} {:x} {:x}",
                node(i, j),
                node(i + 1, j),
                cell(i, j - 1),
                cell(i, j)
            ));
        }
    }
    zones.push((3, 2, interior));
    let mut walls = Vec::new();
    for i in 0..nx {
        walls.push(format!("{:x} {:x} {:x} 0", node(i, 0), node(i + 1, 0), cell(i, 0)));
        walls.push(format!(
            "{:x} {:x} {:x} 0",
            node(i, ny),
            node(i + 1, ny),
            cell(i, ny - 1)
        ));
    }
    zones.push((4, 3, walls));
    zones.push((
        5,
        10,
        (0..ny)
            .map(|j| format!("{:x} {:x} {:x} 0", node(0, j), node(0, j + 1), cell(0, j)))
            .collect(),
    ));
    zones.push((
        6,
        5,
        (0..ny)
            .map(|j| format!("{:x} {:x} {:x} 0", node(nx, j), node(nx, j + 1), cell(nx - 1, j)))
            .collect(),
    ));
    let total: usize = zones.iter().map(|z| z.2.len()).sum();
    s += &format!("(13 (0 1 {total:x} 0))\n");
    let mut first = 1;
    for (id, bc, faces) in &zones {
        let last = first + faces.len() - 1;
        s += &format!("(13 ({id:x} {first:x} {last:x} {bc:x} 2)(\n{}\n))\n", faces.join("\n"));
        first = last + 1;
    }
    s += &format!(
        "(45 (3 interior interior-3)())\n(45 (4 {wall_type} walls)())\n(45 (5 velocity-inlet inlet)())\n(45 (6 pressure-outlet outlet)())\n(45 (9 fluid fluid)())\n{extra}"
    );
    s
}

fn write(dir: &Path, name: &str, text: &str) -> String {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p.display().to_string()
}

#[test]
fn check_mesh_convert_and_reporting_of_skipped_sections() {
    let d = tmp("convert");
    let f = write(
        &d,
        "chan.msh",
        &channel_msh(8, 4, 2.0, 1.0, "(33 (1 2 3))\n(37 ())\n", "wall"),
    );
    let o = rcfd(&["check-mesh", &f]);
    let out = String::from_utf8_lossy(&o.stdout);
    assert_eq!(o.status.code(), Some(0), "{out} {}", String::from_utf8_lossy(&o.stderr));
    assert!(out.contains("cells 32"), "{out}");
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("Fluent sections not read: 33 x1, 37 x1"), "{err}");
    assert!(err.contains("extruded"), "{err}");
    let vtu = d.join("chan.vtu").display().to_string();
    let c = rcfd(&["convert", &f, &vtu]);
    assert_eq!(c.status.code(), Some(0), "{}", String::from_utf8_lossy(&c.stderr));
    assert!(std::fs::metadata(&vtu).unwrap().len() > 1000);

    let p = write(&d, "per.msh", &channel_msh(4, 2, 1.0, 1.0, "", "periodic"));
    let o = rcfd(&["check-mesh", &p]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("periodic"),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn laminar_channel_runs_on_a_fluent_2d_mesh() {
    let d = tmp("run");
    let _ = write(&d, "chan.msh", &channel_msh(16, 8, 2.0, 1.0, "", "wall"));
    let case = write(
        &d,
        "case.toml",
        r#"
title = "channel from a Fluent mesh"
[mesh]
source = "file"
file = "chan.msh"
[physics]
nu = 0.1
[boundaries.inlet]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 3000
tolerance_u = 1e-8
tolerance_p = 1e-8
"#,
    );
    let out = d.join("out").display().to_string();
    let o = rcfd(&["run", &case, "--quiet", "--out", &out]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(d.join("out").join("solution.vtu").exists());
}
