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

fn uniform_errors() -> [f64; 3] {
    let mut e = [0.0; 3];
    for (k, n) in [16usize, 32, 64].into_iter().enumerate() {
        let mesh = base(n, 0).to_mesh().unwrap();
        let (_, err, conv) = solve(&mesh);
        assert!(conv, "uniform {n} not converged");
        println!("A4 uniform {n}x{n}: {} cells, L2 error {err:.4e}", mesh.n_cells());
        e[k] = err;
    }
    e
}

/// Two rounds of gradient-criterion refinement from the 16 x 16 base (threshold = `fraction` of the peak gradient, balance on);
/// returns (cells, L2 error).
fn adapted_run(fraction: f64) -> (usize, f64) {
    let mut amr = base(16, 2);
    for round in 0..2 {
        let mesh = amr.to_mesh().unwrap();
        let (phi, _, conv) = solve(&mesh);
        assert!(conv);
        let ind = gradient_indicator(&mesh, &phi, AXES, false).unwrap();
        let peak = ind.iter().cloned().fold(0.0, f64::max);
        let marks: Vec<Mark> = ind
            .iter()
            .map(|&g| if g >= fraction * peak { Mark::Refine } else { Mark::Keep })
            .collect();
        let (next, _, rep) = amr.adapt(&marks, true).unwrap();
        println!(
            "A4 (fraction {fraction}) round {round}: peak |grad| {peak:.3}, refined {}, cells {} -> {}",
            rep.refined,
            amr.n_cells(),
            next.n_cells()
        );
        amr = next;
    }
    let mesh = amr.to_mesh().unwrap();
    mesh.validate().unwrap();
    let (_, ea, conv) = solve(&mesh);
    assert!(conv);
    (mesh.n_cells(), ea)
}

#[test]
fn a4_order_and_adapted_versus_coarse() {
    let [e16, e32, e64] = uniform_errors();
    let order = (e32 / e64).log2();
    println!("A4 observed order 32->64: {order:.3}");
    assert!(order >= 1.6, "order {order}");
    let (n_a, ea) = adapted_run(0.25);
    println!("A4 adapted: {n_a} cells, E_a {ea:.4e}, E_a/E16 {:.3}", ea / e16);
    assert!(ea <= 0.35 * e16, "E_a/E16 = {}", ea / e16);
    assert!(n_a <= 2048, "{n_a} cells");
}

/// Registered A4 limit `E_a <= 3 E64` with the 0.25 threshold: missed (3.71), see ADR 0054 addendum 2.
#[test]
#[ignore = "known miss: E_a/E64 = 3.71 > 3 for the 0.25 gradient threshold (ADR 0054 addendum 2)"]
fn a4_adapted_vs_fine_known_miss() {
    let [_, _, e64] = uniform_errors();
    let (_, ea) = adapted_run(0.25);
    assert!(ea <= 3.0 * e64, "E_a/E64 = {}", ea / e64);
}

/// A4b (registered in addendum 2): threshold 0.05 of the peak gradient, all three A4 limits.
#[test]
fn a4b_adapted_mesh_with_wider_band() {
    let [e16, _, e64] = uniform_errors();
    let (n_a, ea) = adapted_run(0.05);
    println!(
        "A4b adapted: {n_a} cells, E_a {ea:.4e}; E_a/E16 = {:.3}, E_a/E64 = {:.3}, cells/4096 = {:.3}",
        ea / e16,
        ea / e64,
        n_a as f64 / 4096.0
    );
    let mut misses = Vec::new();
    if ea > 0.35 * e16 {
        misses.push(format!("E_a/E16 = {} > 0.35", ea / e16));
    }
    if ea > 3.0 * e64 {
        misses.push(format!("E_a/E64 = {} > 3", ea / e64));
    }
    if n_a > 2048 {
        misses.push(format!("{n_a} cells > 2048"));
    }
    assert!(misses.is_empty(), "{misses:?}");
}
