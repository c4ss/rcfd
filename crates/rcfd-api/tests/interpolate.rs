//! F-115: start a fine-mesh run from a converged coarse-mesh solution.
//!
//! Checks fixed before running: the first iteration of the interpolated start has a lower
//! momentum residual and a lower pressure residual than the uniform start on the same fine mesh
//! (the acceptance criterion); the conservative mode reproduces the source volume average of u
//! and p to 1e-10 relative to the average of |.|; identical meshes give the identical state; a
//! checkpoint from another mesh is refused.
//!
//! Also measured (added after the first run, recorded, not an acceptance gate): the iteration
//! count to tolerance 1e-6 and the distance of the start from the converged fine solution. The
//! iteration count was hypothesised to drop and did not (see the printed numbers): the normalised
//! residual is relative to the solution's own variation, not to the initial residual.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_post::interpolate::InterpMode;
use std::path::PathBuf;

fn cavity(n: usize, tol: f64) -> String {
    format!(
        r#"
title = "interp cavity"
[mesh]
generator = "cavity_2d"
nx = {n}
ny = {n}
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
max_iterations = 2000
tolerance_u = {tol}
tolerance_p = {tol}
"#
    )
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_interp_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn sim(n: usize, tol: f64) -> (Case, Simulation) {
    let c = Case::parse_toml(&cavity(n, tol)).unwrap();
    let s = Simulation::new(&c).unwrap();
    (c, s)
}

fn converge(s: &mut Simulation) -> usize {
    let mut n = 0;
    while !s.is_done() {
        s.step();
        n += 1;
    }
    assert!(s.is_converged());
    n
}

#[test]
fn coarse_solution_gives_lower_initial_residual_on_fine_mesh() {
    let dir = tmp("accept");
    let (c_coarse, mut coarse) = sim(12, 1e-7);
    converge(&mut coarse);
    let ck = dir.join("coarse.ckpt");
    coarse.save_checkpoint(&ck).unwrap();

    // uniform start on the fine mesh
    let (_, mut uni) = sim(24, 1e-6);
    let start_uni = uni.field("speed").unwrap();
    let first_uni = uni.step();
    let n_uni = 1 + converge(&mut uni);
    let reference = uni.field("speed").unwrap();
    let dist = |a: &[f64]| -> f64 {
        let w = uni.geom.mesh.cell_volumes();
        let num: f64 = a
            .iter()
            .zip(&reference)
            .zip(w)
            .map(|((x, y), v)| v * (x - y).powi(2))
            .sum();
        let den: f64 = reference.iter().zip(w).map(|(y, v)| v * y * y).sum();
        (num / den).sqrt()
    };
    let d_uni = dist(&start_uni);

    for mode in [InterpMode::NonConservative, InterpMode::Conservative] {
        let (_, mut fine) = sim(24, 1e-6);
        let rep = fine.interpolate_from(&c_coarse, &ck, mode).unwrap();
        assert_eq!(rep["source_cells"], 144);
        assert_eq!(rep["target_cells"], 576);
        let d_int = dist(&fine.field("speed").unwrap());
        let first = fine.step();
        let n = 1 + converge(&mut fine);
        println!(
            "{mode:?}: first-iteration res_u {:.4e} res_p {:.4e} (uniform {:.4e} {:.4e}); iterations {n} (uniform {n_uni}); relative L2 distance of the start from the converged speed {d_int:.4} (uniform {d_uni:.4})",
            first.res_u, first.res_p, first_uni.res_u, first_uni.res_p
        );
        assert!(d_int < d_uni);
        assert!(
            first.res_u < first_uni.res_u,
            "res_u {} vs {}",
            first.res_u,
            first_uni.res_u
        );
        assert!(
            first.res_p < first_uni.res_p,
            "res_p {} vs {}",
            first.res_p,
            first_uni.res_p
        );
    }
}

#[test]
fn conservative_mode_matches_source_integrals() {
    let dir = tmp("cons");
    let (c_coarse, mut coarse) = sim(10, 1e-6);
    converge(&mut coarse);
    let ck = dir.join("c.ckpt");
    coarse.save_checkpoint(&ck).unwrap();
    // volume averages (the meshes have different cell depths, so integrals differ by that factor)
    let integral = |s: &Simulation, f: &str| -> (f64, f64) {
        let v = s.field(f).unwrap();
        let vol = s.geom.mesh.cell_volumes();
        let tv: f64 = vol.iter().sum();
        (
            v.iter().zip(vol).map(|(a, b)| a * b).sum::<f64>() / tv,
            v.iter().zip(vol).map(|(a, b)| a.abs() * b).sum::<f64>() / tv,
        )
    };
    let (mut fine_c, mut fine_n) = (sim(20, 1e-6).1, sim(20, 1e-6).1);
    fine_c
        .interpolate_from(&c_coarse, &ck, InterpMode::Conservative)
        .unwrap();
    fine_n
        .interpolate_from(&c_coarse, &ck, InterpMode::NonConservative)
        .unwrap();
    for f in ["p", "ux", "uy"] {
        let (is, abs_s) = integral(&coarse, f);
        let (ic, _) = integral(&fine_c, f);
        let (inc, _) = integral(&fine_n, f);
        println!("{f}: source {is:.6e}, conservative {ic:.6e}, non-conservative {inc:.6e}");
        assert!((ic - is).abs() <= 1e-10 * abs_s.max(1e-30), "{f}: {ic} vs {is}");
    }
}

#[test]
fn identical_mesh_reproduces_state_and_wrong_checkpoint_is_refused() {
    let dir = tmp("same");
    let (c12, mut s12) = sim(12, 1e-6);
    converge(&mut s12);
    let ck = dir.join("a.ckpt");
    s12.save_checkpoint(&ck).unwrap();
    let (_, mut again) = sim(12, 1e-6);
    again.interpolate_from(&c12, &ck, InterpMode::NonConservative).unwrap();
    for f in ["p", "ux", "uy"] {
        let (a, b) = (s12.field(f).unwrap(), again.field(f).unwrap());
        let worst = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
        assert!(worst < 1e-12, "{f}: {worst}");
    }
    // checkpoint of the 12x12 mesh against a 16x16 source description
    let (c16, _) = sim(16, 1e-6);
    let (_, mut t) = sim(24, 1e-6);
    let e = t.interpolate_from(&c16, &ck, InterpMode::NonConservative).unwrap_err();
    assert!(e.to_string().contains("not written for the mesh"), "{e}");
}
