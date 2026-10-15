//! Block-coupled solver with solid cells excluded from the pressure-velocity system (ADR 0029 D3, checks C1 and C2).
//!
//! The limits are those registered in docs/adr/0029-wave19a-coupled.md before this file was written.

use rcfd_fv::bc::{Profile, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::coupled::{CoupledSettings, CoupledSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

fn geom_of(mesh: rcfd_mesh::Mesh) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
}

fn solve(geom: &Arc<FvGeometry>, u: VectorField, p: ScalarField, nu: f64, mask: Vec<bool>, rel: f64) -> CoupledSolver {
    let mut s = CoupledSettings::new(nu);
    s.convection = ConvScheme::Upwind;
    s.linear = SolveControl {
        tol_abs: 1e-14,
        tol_rel: rel,
        max_iter: 400,
    };
    let mut sol = CoupledSolver::with_solids(geom.clone(), u, p, s, Some(mask)).unwrap();
    for it in 1..=3000 {
        let info = sol.iterate();
        if info.max_du < 1e-11 && it > 2 {
            break;
        }
    }
    sol
}

fn solid_faces(geom: &FvGeometry, mask: &[bool]) -> Vec<usize> {
    let mesh = &geom.mesh;
    (0..mesh.n_faces())
        .filter(|&f| mask[mesh.owner()[f]] || (f < geom.n_internal && mask[mesh.neighbour()[f]]))
        .collect()
}

fn fluid_imbalance(geom: &FvGeometry, mask: &[bool], phi: &[f64]) -> f64 {
    let mesh = &geom.mesh;
    let mut worst: f64 = 0.0;
    for c in 0..geom.n_cells() {
        if mask[c] {
            continue;
        }
        let mut sum = 0.0;
        for &f in mesh.cell_faces(c) {
            if geom.empty[f] {
                continue;
            }
            sum += if f >= geom.n_internal || mesh.owner()[f] == c {
                phi[f]
            } else {
                -phi[f]
            };
        }
        worst = worst.max(sum.abs());
    }
    worst
}

fn max_area(geom: &FvGeometry) -> f64 {
    geom.mesh
        .face_area_vectors()
        .iter()
        .map(|s| rcfd_core::vec3::norm(*s))
        .fold(0.0, f64::max)
}

/// Thomas solution of the discrete fluid layer with walls at both ends, `n` cells of height `h`.
fn layer_profile(n: usize, h: f64, g: f64, nu: f64) -> Vec<f64> {
    let k = nu / (h * h);
    let (mut b, mut d) = (vec![-2.0 * k; n], vec![-g; n]);
    let (a, c) = (k, k);
    b[0] = -3.0 * k;
    b[n - 1] = -3.0 * k;
    for j in 1..n {
        let m = a / b[j - 1];
        b[j] -= m * c;
        d[j] -= m * d[j - 1];
    }
    let mut x = vec![0.0; n];
    x[n - 1] = d[n - 1] / b[n - 1];
    for j in (0..n - 1).rev() {
        x[j] = (d[j] - c * x[j + 1]) / b[j];
    }
    x
}

#[test]
fn c1_solid_slab_in_pressure_driven_channel() {
    let (lx, ny, nu, g, hf) = (4.0, 32usize, 0.1, 3.2, 0.5);
    let geom = geom_of(box_2d(8, ny, lx, 1.0, false, false).unwrap());
    let cc = geom.mesh.cell_centres();
    let mask: Vec<bool> = cc.iter().map(|c| c[1] < 0.5).collect();
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for pch in ["ymin", "ymax"] {
        u.set_bc(&geom, pch, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(&geom, "xmin", Box::new(ZeroGradient)).unwrap();
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(g * lx))).unwrap();
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let sol = solve(&geom, u, p, nu, mask.clone(), 1e-10);
    let umax = g * hf * hf / (8.0 * nu);
    // (i) solid velocity exactly zero
    let solid_u = (0..cc.len()).filter(|&c| mask[c]).all(|c| sol.u.values[c] == [0.0; 3]);
    // (ii) flux exactly zero on every face touching a solid cell
    let sf = solid_faces(&geom, &mask);
    let leak = sf.iter().map(|&f| sol.phi[f].abs()).fold(0.0, f64::max);
    // (iii)-(iv)
    let h = 1.0 / ny as f64;
    let prof = layer_profile(ny / 2, h, g, nu);
    let (mut e_disc, mut e_an, mut ev) = (0.0f64, 0.0f64, 0.0f64);
    for c in 0..cc.len() {
        if mask[c] {
            continue;
        }
        let j = ((cc[c][1] - 0.5) / h) as usize;
        e_disc = e_disc.max((sol.u.values[c][0] - prof[j]).abs());
        e_an = e_an.max((sol.u.values[c][0] - g / (2.0 * nu) * (cc[c][1] - 0.5) * (1.0 - cc[c][1])).abs());
        ev = ev.max(sol.u.values[c][1].abs());
    }
    // (v) flow rate through xmin (per unit depth; box_2d depth 1 is the mesh's own thickness)
    let mesh = &geom.mesh;
    let depth: f64 = mesh
        .patch("xmin")
        .unwrap()
        .faces()
        .map(|f| rcfd_core::vec3::norm(mesh.face_area_vectors()[f]))
        .sum::<f64>()
        / 1.0;
    let q = -mesh.patch("xmin").unwrap().faces().map(|f| sol.phi[f]).sum::<f64>() / depth;
    let q_exact = g * hf.powi(3) / (12.0 * nu);
    let eq = (q - q_exact).abs() / q_exact;
    let imb = fluid_imbalance(&geom, &mask, &sol.phi) / (umax * max_area(&geom));
    println!(
        "C1: {} its; solid u==0: {solid_u}; max |phi| on solid faces {leak:.1e} (== 0); vs discrete {:.2e} (1e-7); |v| {:.2e} (1e-7); vs analytic {:.3e} (1e-2, predicted {:.3e}); Q err {eq:.3e} (1e-2, predicted {:.3e}); continuity {imb:.1e} (1e-10)",
        sol.iterations(),
        e_disc / umax,
        ev / umax,
        e_an / umax,
        (h / hf).powi(2),
        2.0 * (h / hf).powi(2)
    );
    assert!(solid_u);
    assert!(leak == 0.0);
    assert!(e_disc <= 1e-7 * umax && ev <= 1e-7 * umax);
    assert!(e_an <= 1e-2 * umax);
    assert!(eq <= 1e-2);
    assert!(imb <= 1e-10);
}

fn obstacle(rel: f64) -> (CoupledSolver, Arc<FvGeometry>, Vec<bool>) {
    let geom = geom_of(box_2d(40, 20, 4.0, 1.0, false, false).unwrap());
    let cc = geom.mesh.cell_centres();
    let mask: Vec<bool> = cc
        .iter()
        .map(|c| (1.5..=2.0).contains(&c[0]) && (0.25..=0.75).contains(&c[1]))
        .collect();
    let mut u = VectorField::uniform("U", &geom, [1.0, 0.0, 0.0]);
    for pch in ["ymin", "ymax"] {
        u.set_bc(&geom, pch, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(&geom, "xmin", Box::new(FixedValue(Profile::Uniform([1.0, 0.0, 0.0]))))
        .unwrap();
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    (solve(&geom, u, p, 0.05, mask.clone(), rel), geom, mask)
}

#[test]
fn c2_obstacle_in_channel() {
    let (sol, geom, mask) = obstacle(1e-10);
    let (sol2, _, _) = obstacle(1e-12);
    let mesh = &geom.mesh;
    let nblock = mask.iter().filter(|&&m| m).count();
    let sf = solid_faces(&geom, &mask);
    let leak = sf.iter().map(|&f| sol.phi[f].abs()).fold(0.0, f64::max);
    let solid_u = (0..mask.len())
        .filter(|&c| mask[c])
        .all(|c| sol.u.values[c] == [0.0; 3]);
    let flux = |patch: &str| mesh.patch(patch).unwrap().faces().map(|f| sol.phi[f]).sum::<f64>();
    let balance = (flux("xmin") + flux("xmax")).abs();
    let imb = fluid_imbalance(&geom, &mask, &sol.phi) / (1.0 * max_area(&geom));
    let dtol = sol
        .u
        .values
        .iter()
        .zip(&sol2.u.values)
        .map(|(a, b)| (a[0] - b[0]).abs().max((a[1] - b[1]).abs()))
        .fold(0.0, f64::max);
    println!(
        "C2: {nblock} block cells, {} its; max |phi| on block faces {leak:.1e} (== 0); block u==0: {solid_u}; inlet+outlet flux {balance:.2e} (1e-10 U H); continuity {imb:.1e} (1e-10); change with pressure_rel_tol 1e-12: {dtol:.2e} (1e-6)",
        sol.iterations()
    );
    assert!(nblock > 0);
    assert!(leak == 0.0 && solid_u);
    assert!(balance <= 1e-10);
    assert!(imb <= 1e-10);
    assert!(dtol <= 1e-6);
}
