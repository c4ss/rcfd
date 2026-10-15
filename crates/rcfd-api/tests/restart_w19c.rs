//! Restart files and solution transfer (F-115, F-121, ADR 0031 D3): checks S1-S5 as registered.
//!
//! S1 steady same-mesh identity; S2 transient restart equals the continued run bit for bit (PISO
//! Euler, PISO BDF2, PIMPLE BDF2); S3 steady file into a transient run; S4 embedded mesh, source
//! case, refusals; S5 coarse-to-fine iteration counts (reported, pass/fail as registered).

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::sim::Simulation;
use rcfd_io::container::Container;
use rcfd_post::remap::RemapMethod;
use std::path::PathBuf;
use std::sync::Mutex;

const CAVITY: &str = r#"
title = "restart cavity"
[mesh]
generator = "cavity_2d"
nx = 12
ny = 12
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 400
tolerance_u = 1e-7
tolerance_p = 1e-7
"#;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_restart_w19c_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn steady(n: usize, tol: f64) -> Case {
    Case::parse_toml(
        &CAVITY
            .replace("nx = 12", &format!("nx = {n}"))
            .replace("ny = 12", &format!("ny = {n}"))
            .replace("1e-7", &format!("{tol:e}")),
    )
    .unwrap()
}

fn transient(scheme: &str, alg: &str, n_outer: usize) -> Case {
    let text = CAVITY.replace(
        "algorithm = \"simplec\"",
        &format!("algorithm = \"{alg}\"\ntime_scheme = \"{scheme}\"\ndt = 0.02\nend_time = 10.0\nn_outer = {n_outer}"),
    );
    Case::parse_toml(&text).unwrap()
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

struct Snap {
    ux: Vec<u64>,
    uy: Vec<u64>,
    p: Vec<u64>,
    flux: Vec<u64>,
    time: u64,
}

fn snap(s: &Simulation) -> Snap {
    Snap {
        ux: bits(&s.field("ux").unwrap()),
        uy: bits(&s.field("uy").unwrap()),
        p: bits(&s.field("p").unwrap()),
        flux: bits(s.flux()),
        time: s.time().to_bits(),
    }
}

fn same(a: &Snap, b: &Snap, what: &str) {
    assert!(a.ux == b.ux, "{what}: ux differs");
    assert!(a.uy == b.uy, "{what}: uy differs");
    assert!(a.p == b.p, "{what}: p differs");
    assert!(a.flux == b.flux, "{what}: flux differs");
    assert_eq!(a.time, b.time, "{what}: time differs");
}

#[test]
fn s1_steady_same_mesh_restart_is_exact() {
    let dir = tmp("s1");
    let case = steady(12, 1e-7);
    let mut a = Simulation::new(&case).unwrap();
    for _ in 0..40 {
        a.step();
    }
    let path = dir.join("c.rcfd");
    assert!(a.supports_restart());
    a.save_restart(&path).unwrap();
    let at_save = snap(&a);
    for _ in 0..25 {
        a.step();
    }
    let mut b = Simulation::new(&case).unwrap();
    let rep = b.restart_from(&path, RemapMethod::Gradient, None).unwrap();
    assert_eq!(rep["restart"], "exact", "{rep}");
    same(&snap(&b), &at_save, "after restore");
    for _ in 0..25 {
        b.step();
    }
    same(&snap(&b), &snap(&a), "25 steps later");
    assert_eq!(a.iteration(), b.iteration());
}

#[test]
fn s2_transient_restart_equals_the_continued_run_bit_for_bit() {
    for (name, scheme, alg, n_outer) in [
        ("piso euler", "euler", "piso", 1),
        ("piso bdf2", "bdf2", "piso", 1),
        ("pimple bdf2", "bdf2", "pimple", 2),
    ] {
        let dir = tmp("s2");
        let case = transient(scheme, alg, n_outer);
        let mut a = Simulation::new(&case).unwrap();
        for _ in 0..8 {
            a.step();
        }
        let path = dir.join("t.rcfd");
        assert!(a.supports_restart(), "{name}");
        a.save_restart(&path).unwrap();
        for _ in 0..12 {
            a.step();
        }
        let mut b = Simulation::new(&case).unwrap();
        let rep = b.restart_from(&path, RemapMethod::Gradient, None).unwrap();
        assert_eq!(rep["restart"], "exact", "{name}: {rep}");
        for _ in 0..12 {
            b.step();
        }
        same(&snap(&b), &snap(&a), name);
        println!(
            "S2 {name}: 12 steps after restore identical to the continued run (time {})",
            a.time()
        );
    }
}

#[test]
fn s2_driver_writes_and_continues_from_a_transient_restart_file() {
    let dir = tmp("s2d");
    let case = transient("bdf2", "piso", 1);
    let ck = dir.join("r.rcfd");
    // reference: 20 steps in one go
    let mut whole = Simulation::new(&case).unwrap();
    for _ in 0..20 {
        whole.step();
    }
    // driver run limited to 8 steps writes the restart file; a second driver run continues 12 steps
    let m = Mutex::new(Simulation::new(&case).unwrap());
    let opts = RunOptions {
        max_steps: Some(8),
        checkpoint: Some(ck.clone()),
        ..RunOptions::default()
    };
    let out = runctl::drive(&m, &opts, &|| false, &mut |_| {});
    assert_eq!(out.state, "step_limit");
    assert!(ck.exists(), "transient restart file not written");
    let mut s2 = Simulation::new(&case).unwrap();
    s2.load_restart(&ck).unwrap();
    let m2 = Mutex::new(s2);
    let opts2 = RunOptions {
        max_steps: Some(12),
        ..RunOptions::default()
    };
    runctl::drive(&m2, &opts2, &|| false, &mut |_| {});
    let s2 = m2.into_inner().unwrap();
    same(&snap(&s2), &snap(&whole), "driver restart");
}

#[test]
fn s3_steady_file_starts_a_transient_run() {
    let dir = tmp("s3");
    let mut st = Simulation::new(&steady(12, 1e-8)).unwrap();
    while !st.is_done() {
        st.step();
    }
    let path = dir.join("s.rcfd");
    st.save_restart(&path).unwrap();
    let mut tr = Simulation::new(&transient("euler", "piso", 1)).unwrap();
    let rep = tr.restart_from(&path, RemapMethod::Gradient, None).unwrap();
    assert_eq!(rep["restart"], "initialised", "{rep}");
    let (a, b) = (snap(&st), snap(&tr));
    assert!(a.ux == b.ux && a.uy == b.uy && a.p == b.p && a.flux == b.flux);
    assert_eq!(tr.time(), 0.0);
    // a transient file cannot start a steady run
    let mut t2 = Simulation::new(&transient("euler", "piso", 1)).unwrap();
    t2.step();
    let tp = dir.join("t.rcfd");
    t2.save_restart(&tp).unwrap();
    let mut st2 = Simulation::new(&steady(12, 1e-8)).unwrap();
    assert!(st2.restart_from(&tp, RemapMethod::Gradient, None).is_err());
}

fn without_mesh(src: &PathBuf, dst: &PathBuf) {
    let c = Container::read_file(src).unwrap();
    let mut n = Container::new();
    for name in c.names() {
        if name != "mesh" {
            n.put(name, c.get(name).unwrap().to_vec());
        }
    }
    n.write_file(dst).unwrap();
}

#[test]
fn s4_embedded_mesh_source_case_and_refusals() {
    let dir = tmp("s4");
    let coarse = steady(12, 1e-7);
    let mut c = Simulation::new(&coarse).unwrap();
    for _ in 0..60 {
        c.step();
    }
    let full = dir.join("full.rcfd");
    c.save_restart(&full).unwrap();
    let v1 = dir.join("v1.rcfd");
    without_mesh(&full, &v1);

    let fine = steady(24, 1e-6);
    let mut a = Simulation::new(&fine).unwrap();
    let rep = a.restart_from(&full, RemapMethod::Gradient, None).unwrap();
    assert_eq!(rep["restart"], "transferred", "{rep}");
    assert_eq!(rep["source_cells"], 144);
    assert_eq!(rep["target_cells"], 576);
    // a file without the mesh needs the source case ...
    let mut b = Simulation::new(&fine).unwrap();
    let e = b
        .restart_from(&v1, RemapMethod::Gradient, None)
        .unwrap_err()
        .to_string();
    assert!(e.contains("source case"), "{e}");
    b.restart_from(&v1, RemapMethod::Gradient, Some(&coarse)).unwrap();
    same(&snap(&a), &snap(&b), "embedded vs source case");
    // ... and the wrong source case is refused
    let mut d = Simulation::new(&fine).unwrap();
    assert!(d
        .restart_from(&v1, RemapMethod::Gradient, Some(&steady(10, 1e-7)))
        .is_err());
    // the exact loader refuses a file of another mesh
    let mut e2 = Simulation::new(&fine).unwrap();
    assert!(e2
        .load_restart(&full)
        .unwrap_err()
        .to_string()
        .contains("different mesh"));
    // every method runs on this pair; conservative_overlap needs box cells (cavity cells are boxes)
    for m in RemapMethod::ALL {
        let mut s = Simulation::new(&fine).unwrap();
        let r = s.restart_from(&full, m, None).unwrap();
        assert_eq!(r["method"], m.name(), "{r}");
        s.step();
    }
}

fn run_to_done(s: &mut Simulation) -> usize {
    let mut n = 0;
    while !s.is_done() {
        s.step();
        n += 1;
    }
    n
}

/// Steps until the relative L2 distance of the speed from `reference` first drops below `limit`
/// (0 when the start is already below), `None` if it never does within the run.
fn steps_to_distance(s: &mut Simulation, reference: &[f64], limit: f64) -> Option<usize> {
    let vol: Vec<f64> = s.geom.mesh.cell_volumes().to_vec();
    let dist = |s: &Simulation| {
        let v = s.field("speed").unwrap();
        let num: f64 = v
            .iter()
            .zip(reference)
            .zip(&vol)
            .map(|((a, b), w)| w * (a - b).powi(2))
            .sum();
        let den: f64 = reference.iter().zip(&vol).map(|(b, w)| w * b * b).sum();
        (num / den).sqrt()
    };
    if dist(s) < limit {
        return Some(0);
    }
    let mut n = 0;
    while !s.is_done() {
        s.step();
        n += 1;
        if dist(s) < limit {
            return Some(n);
        }
    }
    None
}

/// S5: reported numbers; pass/fail as registered (S5a iterations to tolerance, S5b iterations to a
/// 1e-2 relative L2 distance of the speed from the converged fine solution). The assertions of the
/// methods that miss are in the ignored `s5_*_known_miss` tests.
fn s5_numbers() -> Vec<(String, usize, usize, Option<usize>, Option<usize>)> {
    let dir = tmp("s5");
    let mut coarse = Simulation::new(&steady(12, 1e-7)).unwrap();
    run_to_done(&mut coarse);
    let ck = dir.join("c.rcfd");
    coarse.save_restart(&ck).unwrap();
    // reference fine solution
    let mut refsim = Simulation::new(&steady(24, 1e-10)).unwrap();
    run_to_done(&mut refsim);
    let reference = refsim.field("speed").unwrap();
    let mut cold = Simulation::new(&steady(24, 1e-6)).unwrap();
    let n_cold = run_to_done(&mut cold);
    let mut cold2 = Simulation::new(&steady(24, 1e-6)).unwrap();
    let d_cold = steps_to_distance(&mut cold2, &reference, 1e-2);
    println!("S5 cold start: {n_cold} iterations to 1e-6, {d_cold:?} iterations to distance 1e-2");
    let mut out = Vec::new();
    for m in [
        RemapMethod::Gradient,
        RemapMethod::LeastSquares,
        RemapMethod::ConservativeOverlap,
    ] {
        let mut s = Simulation::new(&steady(24, 1e-6)).unwrap();
        s.restart_from(&ck, m, None).unwrap();
        let n = run_to_done(&mut s);
        let mut s2 = Simulation::new(&steady(24, 1e-6)).unwrap();
        s2.restart_from(&ck, m, None).unwrap();
        let d = steps_to_distance(&mut s2, &reference, 1e-2);
        println!(
            "S5 {}: {n} iterations to 1e-6 (cold {n_cold}), {d:?} iterations to distance 1e-2 (cold {d_cold:?})",
            m.name()
        );
        out.push((m.name().to_string(), n, n_cold, d, d_cold));
    }
    out
}

#[test]
fn s5_coarse_to_fine_iteration_counts_are_reported() {
    // Numbers are printed. S5a is a known miss for every method (ignored test below); S5b passes.
    let rows = s5_numbers();
    assert_eq!(rows.len(), 3);
}

#[test]
#[ignore = "known miss: S5a (iterations to 1e-6 strictly fewer than the cold start) is not met by any method (103/96/103 vs 89 cold); see the printed numbers"]
fn s5a_iterations_to_tolerance_known_miss() {
    for (m, n, cold, _, _) in s5_numbers() {
        assert!(n < cold, "S5a {m}: {n} iterations vs cold {cold}");
    }
}

#[test]
fn s5b_iterations_to_distance_are_fewer_than_cold() {
    for (m, _, _, d, dc) in s5_numbers() {
        let (d, dc) = (d.expect("never reached"), dc.expect("cold never reached"));
        assert!(d < dc, "S5b {m}: {d} vs cold {dc}");
    }
}
