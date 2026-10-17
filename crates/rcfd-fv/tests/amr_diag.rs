//! F-102 (ADR 0054 check A4 and addendum 1): a steady diffusion problem with a manufactured tanh-front solution on uniform
//! meshes and on a mesh adapted with the gradient criterion (hanging-node polyhedra). Limits registered before the first run.

use rcfd_core::vec3::Vec3;
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::diffusion::{laplacian, uniform_gamma};
use rcfd_fv::gradient::least_squares;
use rcfd_fv::{FvGeometry, ScalarField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::adapt::{gradient_indicator, AmrMesh, Mark};
use rcfd_mesh::gen::{axis_coords, Grading};
use rcfd_mesh::{Mesh, PatchKind};
use std::sync::Arc;

const W: f64 = 0.06;
const AXES: [bool; 3] = [true, true, false];

fn exact(p: Vec3) -> f64 {
    ((p[0] - 0.5) / W).tanh() + 0.3 * p[1]
}

/// `-lap(exact)` (gamma = 1).
fn source(p: Vec3) -> f64 {
    let t = ((p[0] - 0.5) / W).tanh();
    2.0 * t * (1.0 - t * t) / (W * W)
}

fn base(n: usize, max_level: u8) -> AmrMesh {
    let w = |s: &str| (s.to_string(), PatchKind::Wall);
    let c = [
        axis_coords(n, 1.0, Grading::Uniform).unwrap(),
        axis_coords(n, 1.0, Grading::Uniform).unwrap(),
        vec![0.0, 0.1],
    ];
    AmrMesh::new(
        c,
        [true, true, false],
        max_level,
        [
            w("walls"),
            w("walls"),
            w("walls"),
            w("lid"),
            ("frontAndBack".into(), PatchKind::Empty),
            ("frontAndBack".into(), PatchKind::Empty),
        ],
    )
    .unwrap()
}

/// Returns (cell values, L2 error, converged).
fn solve(mesh: &Mesh) -> (Vec<f64>, f64, bool) {
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh.clone())).unwrap());
    let cc = geom.mesh.cell_centres().to_vec();
    let vol = geom.mesh.cell_volumes().to_vec();
    let mut phi = ScalarField::from_fn("phi", &geom, exact);
    for p in geom.mesh.patches() {
        if p.kind == PatchKind::Empty {
            continue;
        }
        let vals = p
            .faces()
            .map(|f| [exact(geom.mesh.face_centres()[f]), 0.0, 0.0])
            .collect();
        phi.set_bc(&geom, &p.name, Box::new(FixedValue(Profile::PerFace(vals))))
            .unwrap();
    }
    phi.values.iter_mut().for_each(|v| *v = 0.0);
    let gam = uniform_gamma(&geom, 1.0);
    let ctl = SolveControl {
        tol_abs: 1e-300,
        tol_rel: 1e-13,
        max_iter: 4000,
    };
    let mut converged = false;
    for _ in 0..200 {
        let cells = phi.values.clone();
        let bc = phi.boundary_coeffs(&geom);
        let bvals = phi.boundary_values(&geom);
        let grad = least_squares(&geom, &cells, &bvals);
        let mut m = laplacian(&geom, &gam, &bc, Some(&grad));
        for c in 0..cells.len() {
            m.b[c] += source(cc[c]) * vol[c];
        }
        let mut x = cells.clone();
        m.solve(&mut x, &ctl);
        let change = x.iter().zip(&cells).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
        phi.values = x;
        if change <= 1e-12 {
            converged = true;
            break;
        }
    }
    let (mut e2, mut tot) = (0.0, 0.0);
    for c in 0..cc.len() {
        e2 += (phi.values[c] - exact(cc[c])).powi(2) * vol[c];
        tot += vol[c];
    }
    (phi.values, (e2 / tot).sqrt(), converged)
}

/// Diagnostic for the A4 miss: error contributions by refinement level and by interface cells.
#[test]
#[ignore = "diagnostic of the A4 known miss"]
fn a4_diag_error_by_level() {
    let mut amr = base(16, 2);
    for _ in 0..2 {
        let mesh = amr.to_mesh().unwrap();
        let (phi, _, _) = solve(&mesh);
        let ind = gradient_indicator(&mesh, &phi, AXES, false).unwrap();
        let peak = ind.iter().cloned().fold(0.0, f64::max);
        let marks: Vec<Mark> = ind
            .iter()
            .map(|&g| if g >= 0.25 * peak { Mark::Refine } else { Mark::Keep })
            .collect();
        amr = amr.adapt(&marks, true).unwrap().0;
    }
    let mesh = amr.to_mesh().unwrap();
    let (phi, e, _) = solve(&mesh);
    let lv = amr.levels();
    let vol = mesh.cell_volumes();
    let cc = mesh.cell_centres();
    let mut by: std::collections::BTreeMap<u8, (usize, f64)> = Default::default();
    let mut by_if: [(usize, f64); 2] = [(0, 0.0); 2];
    for c in 0..mesh.n_cells() {
        let d = (phi[c] - exact(cc[c])).powi(2) * vol[c];
        let ent = by.entry(lv[c]).or_default();
        ent.0 += 1;
        ent.1 += d;
        let nb_diff = mesh.cell_neighbours(c).iter().any(|&n| lv[n] != lv[c]);
        by_if[nb_diff as usize].0 += 1;
        by_if[nb_diff as usize].1 += d;
    }
    let tot: f64 = by.values().map(|v| v.1).sum();
    println!("total L2 {e:.4e}");
    for (l, (n, d)) in by {
        println!("level {l}: {n} cells, share of squared error {:.3}", d / tot);
    }
    println!(
        "interior cells {} share {:.3}; cells next to a level change {} share {:.3}",
        by_if[0].0,
        by_if[0].1 / tot,
        by_if[1].0,
        by_if[1].1 / tot
    );
    // worst cells
    let mut idx: Vec<usize> = (0..mesh.n_cells()).collect();
    idx.sort_by(|&a, &b| {
        (phi[b] - exact(cc[b]))
            .abs()
            .partial_cmp(&(phi[a] - exact(cc[a])).abs())
            .unwrap()
    });
    for &c in idx.iter().take(8) {
        println!(
            "cell {c} level {} x={:.4} y={:.4} err {:.3e}",
            lv[c],
            cc[c][0],
            cc[c][1],
            phi[c] - exact(cc[c])
        );
    }
    // uniform meshes at the same level
    for n in [16usize, 32, 64] {
        let m = base(n, 0).to_mesh().unwrap();
        let (_, e, _) = solve(&m);
        println!("uniform {n}: {e:.4e}");
    }
}
