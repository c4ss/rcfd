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

/// First-iteration (momentum, pressure) residual numerators `sum|b - A x|`; needs `record_parts`.
fn first_nums(s: &mut Simulation) -> (Vec<f64>, f64) {
    let sv = s.simple_solver_mut().unwrap();
    let u = sv.parts_history[0].iter().map(|p| p.num).collect();
    (u, sv.pressure_parts_history[0].unwrap().num)
}

/// ADR 0099 F1: the initial residuals of both starts divided by the same denominators, the reference norms of the
/// converged uniform-start solution (the default normalisation divides by each run's own variation instead).
#[test]
#[ignore = "known miss: ADR 0099 F1; R_u passes (ratio 0.59) but R_p of the interpolated start is 1.18 (non-conservative) and 1.18 (conservative) times the uniform one, see the ADR 0099 addendum"]
fn f1_fixed_normalisation_initial_residual_known_miss() {
    let dir = tmp("f1");
    let (c_coarse, mut coarse) = sim(12, 1e-7);
    converge(&mut coarse);
    let ck = dir.join("coarse.ckpt");
    coarse.save_checkpoint(&ck).unwrap();

    let (_, mut uni) = sim(24, 1e-6);
    uni.simple_solver_mut().unwrap().record_parts = true;
    uni.step();
    converge(&mut uni);
    let (nu_uni, np_uni) = first_nums(&mut uni);
    let sv = uni.simple_solver_mut().unwrap();
    let d_u: Vec<f64> = sv.parts_history.last().unwrap().iter().map(|p| p.norm_ref).collect();
    let d_p = sv.pressure_parts_history.last().unwrap().unwrap().norm_ref;
    let r = |nu: &[f64], np: f64| -> (f64, f64) {
        let ru = nu.iter().zip(&d_u).map(|(n, d)| n / d).fold(0.0, f64::max);
        (ru, np / d_p)
    };
    let (ru_uni, rp_uni) = r(&nu_uni, np_uni);
    println!("F1 denominators D_u {d_u:?} D_p {d_p:.4e}; uniform R_u {ru_uni:.4e} R_p {rp_uni:.4e}");

    let mut bad = Vec::new();
    for mode in [InterpMode::NonConservative, InterpMode::Conservative] {
        let (_, mut fine) = sim(24, 1e-6);
        fine.interpolate_from(&c_coarse, &ck, mode).unwrap();
        fine.simple_solver_mut().unwrap().record_parts = true;
        fine.step();
        let (nu, np) = first_nums(&mut fine);
        let (ru, rp) = r(&nu, np);
        println!(
            "F1 {mode:?}: R_u {ru:.4e} R_p {rp:.4e} (ratio to uniform {:.4} {:.4})",
            ru / ru_uni,
            rp / rp_uni
        );
        if ru >= ru_uni {
            bad.push(format!("{mode:?} R_u {ru} vs uniform {ru_uni}"));
        }
        if rp >= rp_uni {
            bad.push(format!("{mode:?} R_p {rp} vs uniform {rp_uni}"));
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
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
