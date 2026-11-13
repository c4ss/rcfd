//! F-473 (ADR 0371): conservation of the integral of p in the acoustic wave solver on the closed circular O-grid duct, with an
//! initial pressure of non-zero mean on every patch (the field that ADR 0366's check U8 lacked). Every limit is the registered one.
//!
//! F0: the field has a non-zero boundary mean on every patch. C1: all patches walls, `sum p V` kept to 1e-12 over 100 steps.
//! O1 to O4: one patch open, `sum p V` changes by more than 1e-3, and decreases.

use rcfd_fv::acoustic_wave::{AcousticWaveSolver, Medium, WaveBc};
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::circular_duct_ogrid;
use std::sync::Arc;

const R: f64 = 0.06;
const LEN: f64 = 0.1;
const AIR: Medium = Medium { rho0: 1.2, c: 340.0 };

/// The registered field (ADR 0371): `1 + 0.4 x/R + 0.3 y/R + 0.5 z/L`.
fn field(x: [f64; 3]) -> f64 {
    1.0 + 0.4 * x[0] / R + 0.3 * x[1] / R + 0.5 * x[2] / LEN
}

/// The U8 field of ADR 0366 (zero mean on the wall), used only by the mutation control MU-F.
#[allow(dead_code)]
fn field_u8(x: [f64; 3]) -> f64 {
    (1.0 + x[0] / R + 0.5 * x[1] / R) * (std::f64::consts::PI * x[2] / LEN).cos()
}

fn geom() -> FvGeometry {
    let mesh = circular_duct_ogrid(6, 3, 6, R, 0.5, LEN).unwrap();
    FvGeometry::new(Arc::new(mesh)).unwrap()
}

/// Area-weighted mean of the owner cells' value over the faces of a patch.
fn patch_mean(geom: &FvGeometry, p: &[f64], name: &str) -> f64 {
    let mesh = &geom.mesh;
    let sf = mesh.face_area_vectors();
    let own = mesh.owner();
    let (mut s, mut a) = (0.0, 0.0);
    for f in mesh.patch(name).unwrap().faces() {
        let af = (sf[f][0].powi(2) + sf[f][1].powi(2) + sf[f][2].powi(2)).sqrt();
        s += af * p[own[f]];
        a += af;
    }
    s / a
}

/// 100 steps of `max_dt(0.48)` with the given conditions; returns `(signed change, relative change)` of `sum p V`.
fn run(bcs: [(&str, WaveBc); 3], f: fn([f64; 3]) -> f64) -> (f64, f64) {
    let g = geom();
    let cc = g.mesh.cell_centres().to_vec();
    let vol = g.mesh.cell_volumes().to_vec();
    let mut s = AcousticWaveSolver::new(g, AIR, &bcs).unwrap();
    for (c, x) in cc.iter().enumerate() {
        s.p[c] = f(*x);
    }
    let i0 = s.pressure_integral();
    let scale: f64 = s.p.iter().zip(&vol).map(|(p, v)| p.abs() * v).sum();
    let dt = s.max_dt(0.48);
    for _ in 0..100 {
        s.step(dt).unwrap();
    }
    let d = s.pressure_integral() - i0;
    (d, d.abs() / scale)
}

fn one_open(name: &str) -> [(&'static str, WaveBc); 3] {
    let bc = |n: &'static str| (n, if n == name { WaveBc::Open } else { WaveBc::Wall });
    [bc("wall"), bc("inlet"), bc("outlet")]
}

/// F0: the field has a non-zero mean on every patch and is positive.
#[test]
fn f0_field_has_non_zero_boundary_mean_on_every_patch() {
    let g = geom();
    let p: Vec<f64> = g.mesh.cell_centres().iter().map(|x| field(*x)).collect();
    let min = p.iter().cloned().fold(f64::INFINITY, f64::min);
    println!("F0: min p0 {min:.4}");
    assert!(min > 0.0, "F0 positive");
    for name in ["wall", "inlet", "outlet"] {
        let m = patch_mean(&g, &p, name);
        println!("F0: mean of p0 over {name}: {m:.4}");
        assert!(m >= 0.5, "F0 {name}");
    }
}

/// C1: every patch a wall: `sum p V` is kept to 1e-12 (relative) over 100 steps.
#[test]
fn c1_closed_duct_keeps_the_pressure_integral() {
    let (d, e) = run(
        [
            ("wall", WaveBc::Wall),
            ("inlet", WaveBc::Wall),
            ("outlet", WaveBc::Wall),
        ],
        field,
    );
    println!("C1: change of sum p V {d:.3e}, relative {e:.3e}");
    assert!(e <= 1e-12, "C1");
}

/// O1 to O4: one patch open, `sum p V` changes by more than 1e-3 (relative) and decreases.
#[test]
fn o1_o4_one_open_patch_changes_the_pressure_integral() {
    let mut bad = Vec::new();
    for (id, name) in [("O1", "inlet"), ("O2", "outlet"), ("O3", "wall")] {
        let (d, e) = run(one_open(name), field);
        println!("{id}: {name} open: change of sum p V {d:.3e}, relative {e:.3e}");
        if !(e > 1e-3) {
            bad.push(format!("{id} change"));
        }
        if !(d < 0.0) {
            bad.push(format!("O4 sign {name}"));
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}

/// MU-F control (ADR 0371): U8's field under the closed condition; F0 must reject it. Ignored: it records that the old field
/// does not satisfy F0, and runs only when asked.
#[test]
#[ignore = "control: the U8 field of ADR 0366 must fail F0 (zero wall mean)"]
fn mu_f_u8_field_fails_f0() {
    let g = geom();
    let p: Vec<f64> = g.mesh.cell_centres().iter().map(|x| field_u8(*x)).collect();
    for name in ["wall", "inlet", "outlet"] {
        println!(
            "MU-F: mean of the U8 field over {name}: {:.4}",
            patch_mean(&g, &p, name)
        );
    }
    assert!(
        ["wall", "inlet", "outlet"].iter().all(|n| patch_mean(&g, &p, n) >= 0.5),
        "F0 (U8 field)"
    );
}
