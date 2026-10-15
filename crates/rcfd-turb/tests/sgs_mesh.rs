//! ADR 0026 S2 and S3: the subgrid models on a mesh with an exactly linear velocity field, and the van Driest wiring.

mod common;

use common::sgs::{ctx, set_exact_bc};
use rcfd_fv::{FvGeometry, VectorField};
use rcfd_mesh::gen::{box_2d, box_mesh, Grading};
use rcfd_turb::les::{smagorinsky_nu, wale_nu};
use rcfd_turb::{create_model, TurbulenceModel};
use std::sync::Arc;

const G: [[f64; 3]; 3] = [[0.4, -0.7, 0.2], [1.1, -0.3, 0.5], [-0.6, 0.8, -0.1]];
const PATCHES: [&str; 6] = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];

fn apply(g: &[[f64; 3]; 3], x: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|i| (0..3).map(|j| g[i][j] * x[j]).sum())
}

fn linear_case(g: [[f64; 3]; 3]) -> (Arc<FvGeometry>, VectorField) {
    let mesh = box_mesh(
        [6, 5, 4],
        [1.0; 3],
        [Grading::Uniform, Grading::Symmetric(3.0), Grading::Uniform],
    )
    .unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |c| apply(&g, c));
    set_exact_bc(&mut u, &geom, &PATCHES, &|x| apply(&g, x));
    (geom, u)
}

fn max_rel(a: &[f64], b: &[f64]) -> f64 {
    let scale = b.iter().fold(0.0_f64, |m, &v| m.max(v.abs()));
    a.iter().zip(b).fold(0.0_f64, |m, (&x, &y)| m.max((x - y).abs())) / scale
}

#[test]
fn s2a_s2b_model_matches_closed_form_on_a_linear_field() {
    let (geom, u) = linear_case(G);
    let vol = geom.mesh.cell_volumes().to_vec();
    for key in ["smagorinsky", "wale"] {
        let mut m = create_model(key, &ctx(&geom, 0.01, &[])).unwrap();
        m.update(&u, &[]);
        let expect: Vec<f64> = vol
            .iter()
            .map(|&v| {
                let d = v.cbrt();
                if key == "wale" {
                    wale_nu(&G, d, 0.325)
                } else {
                    smagorinsky_nu(&G, d, 0.17)
                }
            })
            .collect();
        let e = max_rel(m.nut(), &expect);
        println!("S2a {key}: max rel error {e:.3e}");
        assert!(e <= 1e-12, "{key}: {e}");
        // S2b face viscosity
        let nuf = m.nu_face();
        let (own, nei) = (geom.mesh.owner(), geom.mesh.neighbour());
        let nut = m.nut();
        let mut worst = 0.0_f64;
        for (f, &nf) in nuf.iter().enumerate() {
            let want = if f < geom.n_internal {
                0.01 + geom.weights[f] * nut[own[f]] + (1.0 - geom.weights[f]) * nut[nei[f]]
            } else {
                0.01 + nut[own[f]]
            };
            worst = worst.max((nf - want).abs());
        }
        println!("S2b {key}: max face viscosity error {worst:.3e}");
        assert!(worst <= 1e-12);
        // with every patch a wall the boundary faces are exactly nu
        let mut mw = create_model(key, &ctx(&geom, 0.01, &PATCHES)).unwrap();
        mw.update(&u, &[]);
        let nufw = mw.nu_face();
        for (f, &nf) in nufw.iter().enumerate().skip(geom.n_internal) {
            assert_eq!(nf, 0.01, "face {f}");
        }
    }
}

#[test]
fn s2c_uniform_flow_and_rigid_rotation() {
    let (geom, u) = linear_case([[0.0; 3]; 3]);
    let mut u = u;
    for v in u.values.iter_mut() {
        *v = [0.7, -0.2, 0.1];
    }
    set_exact_bc(&mut u, &geom, &PATCHES, &|_| [0.7, -0.2, 0.1]);
    for key in ["smagorinsky", "wale"] {
        let mut m = create_model(key, &ctx(&geom, 0.01, &[])).unwrap();
        m.update(&u, &[]);
        let mx = m.nut().iter().fold(0.0_f64, |a, &b| a.max(b.abs()));
        println!("S2c uniform {key}: max {mx:.3e}");
        assert!(mx <= 1e-14);
    }
    let rot = [[0.0, -3.0, 0.0], [3.0, 0.0, 0.0], [0.0; 3]];
    let (geom, u) = linear_case(rot);
    let mut ms = create_model("smagorinsky", &ctx(&geom, 0.01, &[])).unwrap();
    ms.update(&u, &[]);
    let mx = ms.nut().iter().fold(0.0_f64, |a, &b| a.max(b.abs()));
    println!("S2c rotation smagorinsky max {mx:.3e}");
    assert!(mx <= 1e-12);
    let mut mw = create_model("wale", &ctx(&geom, 0.01, &[])).unwrap();
    mw.update(&u, &[]);
    let expect: Vec<f64> = geom
        .mesh
        .cell_volumes()
        .iter()
        .map(|&v| (0.325 * v.cbrt()).powi(2) * (2.0_f64 / 3.0).powf(0.25) * 3.0)
        .collect();
    let e = max_rel(mw.nut(), &expect);
    println!("S2c rotation wale rel error {e:.3e}");
    assert!(e <= 1e-10);
}

#[test]
fn s3_van_driest_wiring() {
    let (nu, gamma) = (0.01, 4.0);
    let mesh = box_2d(8, 16, 1.0, 1.0, true, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |c| [gamma * c[1], 0.0, 0.0]);
    set_exact_bc(&mut u, &geom, &["ymin", "ymax"], &|x| [gamma * x[1], 0.0, 0.0]);
    let mut c = ctx(&geom, nu, &["ymin", "ymax"]);
    c.wall_damping = true;
    let mut m = create_model("smagorinsky", &c).unwrap();
    m.update(&u, &[]);
    let (cc, vol) = (geom.mesh.cell_centres(), geom.mesh.cell_volumes());
    let utau = (nu * gamma).sqrt();
    let mut worst = 0.0_f64;
    let mut peak = 0.0_f64;
    for i in 0..cc.len() {
        let y = cc[i][1];
        let d = y.min(1.0 - y);
        let f = 1.0 - (-(d * utau / nu) / 25.0).exp();
        let want = (0.17 * vol[i].cbrt() * f).powi(2) * gamma;
        worst = worst.max((m.nut()[i] - want).abs() / want);
        peak = peak.max(want);
    }
    println!("S3 van Driest wiring: max rel error {worst:.3e}");
    assert!(worst <= 1e-12, "{worst}");
    // y+ of the wall faces: d_n u_tau / nu with d_n half the first cell height
    let yp = m.y_plus_faces();
    assert_eq!(yp.len(), 16);
    let want = 0.5 * (1.0 / 16.0) * utau / nu;
    assert!(yp.iter().all(|&v| (v - want).abs() < 1e-12 * want), "{yp:?} vs {want}");
    let _ = peak;
    let _: &dyn TurbulenceModel = m.as_ref();
}
