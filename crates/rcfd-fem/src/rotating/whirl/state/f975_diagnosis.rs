//! F-975 (ADR 0806): the diagnosis of the stalled QR iteration on the wheel sector W of ADR 0595, rows DG1 to DG3. The record
//! of a stalled first iteration (ADR 0806 D4) exists in test builds only. The builder of W is that of
//! `tests/sa_beam_opt_ins_default_f672.rs`, copied with `crate::` paths.

use super::QR_FAILURES;
use crate::modal::ModalOptions;
use crate::model::CyclicOptions;
use crate::rotating::{RotatingOptions, RotatingSector};
use crate::{BeamSection, BlockProperty, Constraint, ElementKind, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;

type Vec3 = [f64; 3];

const N: usize = 6;
const NR: usize = 4;
const ORIGIN: Vec3 = [0.3, -0.2, 0.5];
const LOADED: [usize; 5] = [0, 1, 2, 3, 5];

fn axis() -> Vec3 {
    [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn ea() -> Vec3 {
    let l = 5.0f64.sqrt();
    [2.0 / l, -1.0 / l, 0.0]
}

fn eb() -> Vec3 {
    cross(axis(), ea())
}

fn comb(a: f64, x: Vec3, b: f64, y: Vec3) -> Vec3 {
    [0, 1, 2].map(|k| a * x[k] + b * y[k])
}

fn p(phi: f64, r: f64, z: f64) -> Vec3 {
    let (n, a, b) = (axis(), ea(), eb());
    let (s, c) = phi.sin_cos();
    [0, 1, 2].map(|k| ORIGIN[k] + r * (c * a[k] + s * b[k]) + z * n[k])
}

fn wheel() -> SolidModel {
    let a = 2.0 * PI / N as f64;
    let mut nodes: Vec<Vec3> = (0..=NR).map(|i| p(i as f64 * a / NR as f64, 1.0, 0.0)).collect();
    nodes.push(p(a / 2.0, 0.65, 0.125));
    nodes.push(p(a / 2.0, 0.3, 0.25));
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2, 2, 3, 3, 4])
        .unwrap();
    mesh.add_block(ElementKind::Beam2, vec![6, 5, 5, 2]).unwrap();
    let ring = BeamSection::new(1.8e-3, 1.35e-7, 5.4e-7, 3.7e-7, 5.0 / 6.0, 5.0 / 6.0).unwrap();
    let spoke = BeamSection::new(1.2e-3, 1.0e-7, 2.5e-7, 2.4e-7, 0.85, 0.75).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(ring, comb(0.8, axis(), 0.6, ea())).unwrap())
        .unwrap();
    mesh.set_block_property(1, BlockProperty::beam(spoke, comb(0.6, eb(), 0.8, axis())).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..6).collect()).unwrap();
    mesh.add_node_set("low", vec![0]).unwrap();
    mesh.add_node_set("high", vec![NR]).unwrap();
    mesh.add_node_set("hub", vec![6]).unwrap();
    for q in LOADED {
        mesh.add_node_set(&format!("p{q}"), vec![q]).unwrap();
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("hub", comp, 0.0)).unwrap();
    }
    let o = CyclicOptions::new().shells(true);
    m.add_constraint(Constraint::cyclic_with("low", "high", axis(), ORIGIN, N, o))
        .unwrap();
    m
}

fn met(ok: bool) -> &'static str {
    if ok {
        "met"
    } else {
        "MISSED"
    }
}

/// DG1, DG2, DG3 of ADR 0806.
#[test]
fn dg1_dg2_dg3_the_stalled_window_of_the_wheel_sector() {
    let m = wheel();
    let mut mo = ModalOptions::new(4);
    mo.tolerance = 1e-12;
    mo.residual_test = ResidualTest::Backward;
    let o = RotatingOptions::new(4).modal(mo).coriolis(true);
    let sec = RotatingSector::new(&m, &o).unwrap();
    let mut failed = Vec::new();
    let mut dg2 = true;
    let mut dg3 = true;
    for k in 0..4usize {
        for w in [0.0, 30.0, 60.0] {
            QR_FAILURES.with(|r| r.borrow_mut().clear());
            let r = sec.whirl_state(k, w);
            let recs = QR_FAILURES.with(|r| r.borrow().clone());
            println!(
                "k = {k}, speed {w}: result Ok {}, stalled first iterations {}",
                r.is_ok(),
                recs.len()
            );
            for f in &recs {
                failed.push((k, w));
                let smin = f.sub.iter().copied().fold(f64::INFINITY, f64::min);
                let refused = f.sub.iter().zip(&f.local).all(|(s, l)| *s > f64::EPSILON * l);
                let ok = smin <= f.tau && refused;
                dg2 &= ok;
                println!(
                    "  DG2 m = {}, window {:?}, ||h0||_F = {:.6e}, tau = {:.6e}, fallback converged {}",
                    f.m, f.window, f.fro, f.tau, f.fallback_ok
                );
                for (i, (s, l)) in f.sub.iter().zip(&f.local).enumerate() {
                    println!(
                        "    row {}: |sub| = {s:.6e} ({:.3} eps ||h0||_F), local sum = {l:.6e}, |sub| / (eps local) = {:.3e}",
                        f.window.0 + 1 + i,
                        s / (f64::EPSILON * f.fro),
                        s / (f64::EPSILON * l)
                    );
                }
                println!(
                    "  DG2: smallest |sub| {smin:.6e} <= tau {}, every subdiagonal refused by the local test {refused}: {}",
                    smin <= f.tau,
                    met(ok)
                );
            }
            if (k == 1 || k == 2) && w == 0.0 {
                match &r {
                    Ok(h) => {
                        let ev = &h.state.eigenvalues;
                        let mut worst = 0.0f64;
                        for pair in ev.chunks(2) {
                            if pair.len() == 2 {
                                let a = pair[0][0].hypot(pair[0][1]);
                                let d = (pair[0][1] - pair[1][1]).abs();
                                worst = worst.max(if a > 0.0 { d / a } else { d });
                            }
                        }
                        let ok = ev.len() % 2 == 0 && worst <= 1e-8;
                        dg3 &= ok;
                        println!(
                            "  DG3 k = {k}: {} eigenvalues, largest |Im s_2i - Im s_2i+1| / |s_2i| = {worst:.3e} (limit 1e-8): {}",
                            ev.len(),
                            met(ok)
                        );
                    }
                    Err(e) => {
                        dg3 = false;
                        println!("  DG3 k = {k}: not computed, {e:?}: MISSED");
                    }
                }
            }
        }
    }
    let want = failed.contains(&(1, 0.0)) && failed.contains(&(2, 0.0));
    let none_single = !failed.iter().any(|(k, _)| *k == 0 || *k == 3);
    let none_30 = !failed.iter().any(|(_, w)| *w == 30.0);
    let dg1 = want && none_single && none_30 && !failed.is_empty();
    println!(
        "DG1: stalled at {failed:?}; at (1, 0) and (2, 0) {want}, at no single index {none_single}, at no index at 30 rad/s {none_30}: {}",
        met(dg1)
    );
    println!("DG2: {}", met(dg2 && !failed.is_empty()));
    println!("DG3: {}", met(dg3));
    assert!(dg1, "DG1");
    assert!(dg2 && !failed.is_empty(), "DG2");
    assert!(dg3, "DG3");
}
