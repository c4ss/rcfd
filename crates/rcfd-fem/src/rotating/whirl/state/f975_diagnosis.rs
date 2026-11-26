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

// ------------------------------------------------------------------------------------------------------------------------
// ADR 0814 (F-975): the controls C1 to C4 of the stalled set of DG1 on two platforms.

use super::super::{cholesky, dense_of, doubled, lower_solve, transpose};
use super::{hessenberg, real_eigenvalues, schur_sweep};

/// FNV-1a over the little-endian bits of `xs`.
fn fnv(xs: impl IntoIterator<Item = f64>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for x in xs {
        for b in x.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// The arguments of `sin` and `cos` on the path of W (ADR 0814 C3): the builder's angles, computed as `wheel` computes them,
/// and `2 pi j / N`, computed as the library computes them.
fn libm_args() -> Vec<(String, f64)> {
    let a = 2.0 * PI / N as f64;
    let mut out: Vec<(String, f64)> = (1..=NR)
        .map(|i| (format!("builder {i} a / {NR}"), i as f64 * a / NR as f64))
        .collect();
    out.push(("builder a / 2".to_string(), a / 2.0));
    out.extend((1..N).map(|j| (format!("library 2 pi {j} / {N}"), 2.0 * PI * j as f64 / N as f64)));
    out
}

/// The input of the QR iteration of `whirl_state(k, 0)`: a reproduction of `whirl_state_at` (sector.rs) and of the
/// assembly of the state matrix in `state_solve`, then `hessenberg` (ADR 0814 C1).
struct Stage {
    kd: Vec<f64>,
    md: Vec<f64>,
    a: Vec<f64>,
    h0: Vec<f64>,
    mm: usize,
}

fn stage(sec: &RotatingSector, k: usize) -> Stage {
    let omega = 0.0f64;
    let model = sec.part.model;
    let multiplicity = if k == 0 || 2 * k == sec.sectors { 1 } else { 2 };
    let tr = if k == 0 {
        crate::mpc::transformation(model, &[]).unwrap()
    } else {
        crate::mpc::harmonic_transformation(model, &[], k).unwrap()
    };
    let c = sec.part.coriolis().unwrap();
    let keff = sec.part.operator(omega);
    let (k2, m2, c2) = if multiplicity == 2 {
        (doubled(&keff), doubled(&sec.part.m), doubled(&c))
    } else {
        (keff, sec.part.m.clone(), c)
    };
    let zero = vec![0.0; tr.n_unknowns()];
    let (kr, _) = tr.reduce(&k2, &zero).unwrap();
    let (mr, _) = tr.reduce(&m2, &zero).unwrap();
    let (cr, _) = tr.reduce(&c2, &zero).unwrap();
    let n = kr.n;
    let kd = dense_of(&kr);
    let md = dense_of(&mr);
    let cd = dense_of(&cr);
    let gd: Vec<f64> = if sec.part.options.coriolis {
        cd.iter().map(|v| 2.0 * omega * v).collect()
    } else {
        vec![0.0; n * n]
    };
    let lm = cholesky(&md, n).unwrap();
    let scaled = |x: &[f64]| -> Vec<f64> {
        let mut a = x.to_vec();
        lower_solve(&lm, n, &mut a, n);
        let mut y = transpose(&a, n, n);
        lower_solve(&lm, n, &mut y, n);
        y
    };
    let yk = scaled(&kd);
    let yg = scaled(&gd);
    let mut kh = vec![0.0; n * n];
    let mut gh = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            kh[i * n + j] = 0.5 * (yk[j * n + i] + yk[i * n + j]);
            gh[i * n + j] = 0.5 * (yg[j * n + i] - yg[i * n + j]);
        }
    }
    let kmax = kh.iter().map(|v| v.abs()).fold(0.0f64, f64::max);
    let gmax = gh.iter().map(|v| v.abs()).fold(0.0f64, f64::max);
    let mut cc = (kmax + gmax * gmax).sqrt();
    if !(cc.is_finite() && cc > 0.0) {
        cc = 1.0;
    }
    let mm = 2 * n;
    let mut a = vec![0.0; mm * mm];
    for i in 0..n {
        a[i * mm + n + i] = cc;
        for j in 0..n {
            a[(n + i) * mm + j] = -kh[i * n + j] / cc;
            a[(n + i) * mm + n + j] = -gh[i * n + j];
        }
    }
    let mut h0 = a.clone();
    hessenberg(&mut h0, mm);
    Stage { kd, md, a, h0, mm }
}

/// The stalled first iterations of the 12 calls of DG1, as the test of ADR 0806 collects them.
fn stall_set(sec: &RotatingSector) -> Vec<(usize, f64)> {
    let mut failed = Vec::new();
    for k in 0..4usize {
        for w in [0.0, 30.0, 60.0] {
            QR_FAILURES.with(|r| r.borrow_mut().clear());
            let _ = sec.whirl_state(k, w);
            let n = QR_FAILURES.with(|r| r.borrow().len());
            failed.extend(std::iter::repeat((k, w)).take(n));
        }
    }
    QR_FAILURES.with(|r| r.borrow_mut().clear());
    failed
}

/// The hashes L2 (the part's `K`, `M`, `C`) and, for k = 1, 2, L3 (the reduced `K`, `M`) and L4 (`A`, `h0`).
fn stage_hashes(sec: &RotatingSector) -> Vec<u64> {
    let c = sec.part.coriolis().unwrap();
    let l2 = fnv(sec.part.k.val.iter().chain(&sec.part.m.val).chain(&c.val).copied());
    let mut out = vec![l2];
    for k in [1, 2] {
        let st = stage(sec, k);
        out.push(fnv(st.kd.iter().chain(&st.md).copied()));
        out.push(fnv(st.a.iter().chain(&st.h0).copied()));
    }
    out
}

fn options(threads: Option<usize>) -> RotatingOptions {
    let mut mo = ModalOptions::new(4);
    mo.tolerance = 1e-12;
    mo.residual_test = ResidualTest::Backward;
    mo.threads = threads;
    RotatingOptions::new(4).modal(mo).coriolis(true)
}

/// One-ulp moves of the nonzero entries on or above the subdiagonal of `h0`, copy `j` (ADR 0814 C4).
fn perturbed(h0: &[f64], mm: usize, j: u64) -> Vec<f64> {
    let mut h = h0.to_vec();
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15 ^ (j + 1);
    for r in 0..mm {
        for c in 0..mm {
            let v = h[r * mm + c];
            if c + 1 >= r && v != 0.0 {
                x = x
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                h[r * mm + c] = match (x >> 33) % 3 {
                    1 => f64::from_bits(v.to_bits() + 1),
                    2 => f64::from_bits(v.to_bits() - 1),
                    _ => v,
                };
            }
        }
    }
    h
}

/// C1 to C4 of ADR 0814: asserts C1 and C2 (bits within this platform); prints C3 and C4, compared across platforms.
#[test]
fn dg1_controls_library_order_threshold_f975_0814() {
    let m = wheel();
    let sec = RotatingSector::new(&m, &options(None)).unwrap();

    // C3: the library's values (L0) and the node coordinates (L1).
    let args = libm_args();
    for (label, x) in &args {
        let (s, c) = (x.sin(), x.cos());
        println!(
            "C3 L0 {label}: arg {:#018x} ({x:.17e}), sin {:#018x} ({s:.17e}), cos {:#018x} ({c:.17e})",
            x.to_bits(),
            s.to_bits(),
            c.to_bits()
        );
    }
    let l0 = fnv(args.iter().flat_map(|(_, x)| [x.sin(), x.cos()]));
    let l1 = fnv(m.mesh().nodes().iter().flatten().copied());
    let hs = stage_hashes(&sec);
    println!("C3 L0 {l0:#018x}");
    println!("C3 L1 {l1:#018x}");
    println!("C3 L2 {:#018x}", hs[0]);
    for (i, k) in [1, 2].iter().enumerate() {
        println!("C3 k = {k}: L3 {:#018x}, L4 {:#018x}", hs[1 + 2 * i], hs[2 + 2 * i]);
    }

    // C1: the reproduction against the library, k = 1, 2 at 0 rad/s.
    let mut c1 = true;
    for k in [1, 2] {
        QR_FAILURES.with(|r| r.borrow_mut().clear());
        let r = sec.whirl_state(k, 0.0).unwrap();
        let recs = QR_FAILURES.with(|r| r.borrow().clone());
        let st = stage(&sec, k);
        let stalls = schur_sweep(&mut st.h0.clone(), st.mm, None).is_err();
        let fro = st.h0.iter().map(|v| v * v).sum::<f64>().sqrt();
        let same_fro = recs.iter().all(|f| f.fro.to_bits() == fro.to_bits());
        QR_FAILURES.with(|r| r.borrow_mut().clear());
        let mut ev = real_eigenvalues(st.a.clone(), st.mm).unwrap();
        QR_FAILURES.with(|r| r.borrow_mut().clear());
        ev.sort_by(|p, q| p[1].total_cmp(&q[1]).then(p[0].total_cmp(&q[0])));
        let lib = &r.state.eigenvalues;
        let same_ev = ev.len() == lib.len()
            && ev
                .iter()
                .zip(lib)
                .all(|(a, b)| a[0].to_bits() == b[0].to_bits() && a[1].to_bits() == b[1].to_bits());
        let ok = stalls == !recs.is_empty() && same_fro && same_ev;
        c1 &= ok;
        println!(
            "C1 k = {k}: reproduction stalls {stalls}, library records {}, Frobenius norm {fro:.6e} bits equal {same_fro}, {} eigenvalues bits equal {same_ev}: {}",
            recs.len(),
            ev.len(),
            met(ok)
        );
    }

    // C2: a second build and a build with one thread.
    let set = stall_set(&sec);
    let m2 = wheel();
    let sec2 = RotatingSector::new(&m2, &options(None)).unwrap();
    let m3 = wheel();
    let sec3 = RotatingSector::new(&m3, &options(Some(1))).unwrap();
    let (hs2, set2) = (stage_hashes(&sec2), stall_set(&sec2));
    let (hs3, set3) = (stage_hashes(&sec3), stall_set(&sec3));
    let c2 = hs == hs2 && hs == hs3 && set == set2 && set == set3;
    println!("C2 stall sets: default {set:?}, second build {set2:?}, threads 1 {set3:?}");
    println!(
        "C2 hashes L2..L4 equal: second build {}, threads 1 {}: {}",
        hs == hs2,
        hs == hs3,
        met(c2)
    );

    // C4: 16 one-ulp perturbations of h0, k = 1, 2.
    for k in [1, 2] {
        let st = stage(&sec, k);
        let runs: Vec<bool> = (0..16u64)
            .map(|j| schur_sweep(&mut perturbed(&st.h0, st.mm, j), st.mm, None).is_err())
            .collect();
        let s = runs.iter().filter(|b| **b).count();
        let pattern: String = runs.iter().map(|b| if *b { 'S' } else { 'c' }).collect();
        println!(
            "C4 k = {k}: stalls in {s} of 16 perturbed copies ({pattern}; S stalled, c converged); near the threshold {}",
            s > 0 && s < 16
        );
    }
    println!("C1: {}", met(c1));
    println!("C2: {}", met(c2));
    assert!(c1, "C1");
    assert!(c2, "C2");
}
