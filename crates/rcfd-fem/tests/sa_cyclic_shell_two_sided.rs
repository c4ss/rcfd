//! F-662 (ADR 0583): two-sided shell edge ties for non-matching cyclic boundaries. Checks J0 to J3, R2, RA, RW, RW0, B4 and the
//! negative controls NC8 and NC9 of the registration (B1 and B2 are the existing tests and `baseline_hashes`).

use rcfd_fem::builders::hex_box;
use rcfd_fem::cyclic::{boundary, modal_analysis_harmonic, the_cyclic_constraint, HarmonicModes, Mat3};
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::rotating::{RotatingOptions, RotatingPart, RotatingSector};
use rcfd_fem::tie::{TieMethod, TieOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;

type Vec3 = [f64; 3];
type Weights = Vec<(usize, f64)>;

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const N: usize = 6;
const T: f64 = 0.05;
const ORIGIN: Vec3 = [0.3, -0.2, 0.5];
const EPSILON: f64 = 1e-6;

fn axis() -> Vec3 {
    [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0]
}

fn mat() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn frame() -> (Vec3, Vec3, Vec3) {
    let n = axis();
    let l = 5.0f64.sqrt();
    let ea = [2.0 / l, -1.0 / l, 0.0];
    let eb = cross(n, ea);
    (n, ea, eb)
}

/// The frustum of ADR 0556 D3 at angle `phi` and generator parameter `s`.
fn frustum(phi: f64, s: f64) -> Vec3 {
    let (n, ea, eb) = frame();
    let r = 1.0 - 0.4 * s;
    let z = 0.8 * s;
    let (sp, cp) = phi.sin_cos();
    [0, 1, 2].map(|k| ORIGIN[k] + r * (cp * ea[k] + sp * eb[k]) + z * n[k])
}

fn modal_options(modes: usize) -> ModalOptions {
    let mut o = ModalOptions::new(modes);
    o.tolerance = 1e-12;
    o.residual_test = ResidualTest::Backward;
    o
}

fn met(ok: bool) -> &'static str {
    if ok {
        "met"
    } else {
        "MISSED"
    }
}

fn shells() -> CyclicOptions {
    CyclicOptions::new().shells(true)
}

fn extended() -> CyclicOptions {
    CyclicOptions::new().shells(true).shell_extensions(true)
}

fn nts() -> TieOptions {
    TieOptions::new(TieMethod::NodeToSurface)
}

fn clamp(m: &mut SolidModel) {
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("clamp", comp, 0.0)).unwrap();
    }
}

// ---------------------------------------------------------------------------------------------- the frustum sectors

/// The frustum sector of ADR 0575 D4 (the same construction as `sa_cyclic_shell_extensions.rs`).
struct Frustum {
    mesh: SolidMesh,
    id: Vec<Vec<usize>>,
}

fn frustum_mesh(kind: ElementKind, m: &[usize], shift: &dyn Fn(usize, usize) -> f64) -> Frustum {
    let nc = m.len() - 1;
    let mut nodes = Vec::new();
    let mut id = Vec::new();
    for (i, &mi) in m.iter().enumerate() {
        let phi = (PI / 3.0) * i as f64 / nc as f64;
        let mut col = Vec::new();
        for a in 0..=mi {
            col.push(nodes.len());
            nodes.push(frustum(phi, (a as f64 + shift(i, a)) / mi as f64));
        }
        id.push(col);
    }
    let mut conn = Vec::new();
    for i in 0..nc {
        let (l, r) = (&id[i], &id[i + 1]);
        if m[i] == m[i + 1] {
            for a in 0..m[i] {
                let q = [l[a], r[a], r[a + 1], l[a + 1]];
                match kind {
                    ElementKind::Shell4 => conn.extend(q),
                    _ => conn.extend([q[0], q[1], q[2], q[0], q[2], q[3]]),
                }
            }
        } else {
            assert_eq!(kind, ElementKind::Shell3, "a merged strip is Shell3");
            let (ml, mr) = (m[i], m[i + 1]);
            let (mut a, mut b) = (0, 0);
            while a < ml || b < mr {
                let sl = if a < ml {
                    (a + 1) as f64 / ml as f64
                } else {
                    f64::INFINITY
                };
                let sr = if b < mr {
                    (b + 1) as f64 / mr as f64
                } else {
                    f64::INFINITY
                };
                if sl <= sr {
                    conn.extend([l[a], r[b], l[a + 1]]);
                    a += 1;
                } else {
                    conn.extend([l[a], r[b], r[b + 1]]);
                    b += 1;
                }
            }
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(T).unwrap()).unwrap();
    let ne = mesh.blocks()[0].len();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("clamp", id.iter().map(|c| c[0]).collect()).unwrap();
    mesh.add_node_set("low", id[0].clone()).unwrap();
    mesh.add_node_set("high", id[nc].clone()).unwrap();
    Frustum { mesh, id }
}

fn frustum_model(f: Frustum, options: CyclicOptions) -> SolidModel {
    let mut model = SolidModel::new(f.mesh);
    model.add_material("all", mat()).unwrap();
    clamp(&mut model);
    model
        .add_constraint(Constraint::cyclic_with("low", "high", axis(), ORIGIN, N, options))
        .unwrap();
    model
}

fn no_shift(_: usize, _: usize) -> f64 {
    0.0
}

fn columns(nc: usize, coarse: usize, fine: usize, which: Option<usize>) -> Vec<usize> {
    (0..=nc).map(|i| if Some(i) == which { fine } else { coarse }).collect()
}

/// The four lowest eigenvalues of the real problem of each harmonic `k = 0 .. 3`.
fn harmonics(m: &SolidModel) -> Vec<HarmonicModes> {
    (0..=N / 2)
        .map(|k| modal_analysis_harmonic(m, &[], k, &modal_options(4)).unwrap())
        .collect()
}

/// The I1 sector of ADR 0575 (Shell4, `m = 6`, the high column shifted by `EPSILON`).
fn i1_sector() -> SolidModel {
    let shift = |i: usize, a: usize| if i == 4 && (1..6).contains(&a) { EPSILON } else { 0.0 };
    frustum_model(
        frustum_mesh(ElementKind::Shell4, &[6; 5], &shift),
        extended().interpolation(nts()),
    )
}

/// The I2a (`high = true`) or I2b sector of ADR 0575 at `level`.
fn i2_sector(level: usize, high: bool) -> (Frustum, SolidModel) {
    let (nc, coarse, fine) = (4 * level, 8 * level, 12 * level);
    let which = if high { nc } else { 0 };
    let f = frustum_mesh(ElementKind::Shell3, &columns(nc, coarse, fine, Some(which)), &no_shift);
    let id = f.id.clone();
    let model = frustum_model(f, extended().interpolation(nts()));
    (
        Frustum {
            mesh: model.mesh().clone(),
            id,
        },
        model,
    )
}

fn fnv(bits: &[u64]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bits {
        for byte in b.to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

fn apply(r: &Mat3, v: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| r[i][0] * v[0] + r[i][1] * v[1] + r[i][2] * v[2])
}

fn transpose(r: &Mat3) -> Mat3 {
    let mut t = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            t[i][j] = r[j][i];
        }
    }
    t
}

/// `d_k = max |f / f_ref - 1|` over the sorted four lowest eigenvalues of each harmonic; returns `max_k d_k`.
fn sector_against_sector(name: &str, m: &[HarmonicModes], r: &[HarmonicModes]) -> f64 {
    let mut worst = 0.0f64;
    for (hm, hr) in m.iter().zip(r) {
        let mut a = hm.eigenvalues.clone();
        let mut b = hr.eigenvalues.clone();
        a.sort_by(f64::total_cmp);
        b.sort_by(f64::total_cmp);
        assert_eq!(a.len(), b.len());
        let d = a
            .iter()
            .zip(&b)
            .map(|(x, y)| ((x / y).sqrt() - 1.0).abs())
            .fold(0.0f64, f64::max);
        println!(
            "{name} k = {}: d_k {d:.4e}; f {:?} against {:?}",
            hm.harmonic, hm.frequencies, hr.frequencies
        );
        worst = worst.max(d);
    }
    worst
}

/// The localisation `rho` of ADR 0583 of every mode of `hs`: the largest translation amplitude on the boundary columns over
/// the largest on the interior columns; returns the largest `rho`.
fn localisation(name: &str, m: &SolidModel, id: &[Vec<usize>], hs: &[HarmonicModes]) -> f64 {
    let dofs = m.dof_map();
    let nc = id.len() - 1;
    let amp = |v: &[f64], w: &[f64], p: usize| -> f64 {
        (0..3)
            .map(|c| {
                let e = dofs.equation(p, c).unwrap();
                let a = v[e];
                let b = if w.is_empty() { 0.0 } else { w[e] };
                a * a + b * b
            })
            .sum::<f64>()
            .sqrt()
    };
    let mut worst = 0.0f64;
    for h in hs {
        let mut per = Vec::new();
        for mode in &h.modes {
            let edge = id[0]
                .iter()
                .chain(&id[nc])
                .map(|&p| amp(&mode.cos, &mode.sin, p))
                .fold(0.0f64, f64::max);
            let inner = id[1..nc]
                .iter()
                .flatten()
                .map(|&p| amp(&mode.cos, &mode.sin, p))
                .fold(0.0f64, f64::max);
            per.push(edge / inner);
        }
        println!(
            "{name} k = {}: rho {:?}",
            h.harmonic,
            per.iter().map(|x| format!("{x:.3}")).collect::<Vec<_>>()
        );
        worst = per.iter().copied().fold(worst, f64::max);
    }
    worst
}

// ---------------------------------------------------------------------------------------------- J0

#[test]
fn j0_the_tied_low_nodes_and_their_weights() {
    let mut fails = Vec::new();
    let (f, m) = i2_sector(1, false);
    let ci = the_cyclic_constraint(&m).unwrap();
    let bd = boundary(&m, ci).unwrap();
    let (low, high) = (&f.id[0], &f.id[4]);
    let want_f: Vec<usize> = [1, 2, 4, 5, 7, 8, 10, 11].iter().map(|&a| low[a]).collect();
    let got_f: Vec<usize> = bd.two_sided().iter().map(|t| t.0).collect();
    println!(
        "J0 I2b: two_sided {got_f:?} (expected {want_f:?}) {}",
        met(got_f == want_f)
    );
    if got_f != want_f {
        fails.push(format!("F {got_f:?}"));
    }
    let want_kept: Vec<usize> = [0, 2, 4, 6, 8].iter().map(|&b| high[b]).collect();
    let got_kept: Vec<usize> = bd.interpolated().iter().map(|t| t.0).collect();
    println!(
        "J0 I2b: interpolated {got_kept:?} (expected {want_kept:?}) {}",
        met(got_kept == want_kept)
    );
    if got_kept != want_kept {
        fails.push(format!("kept {got_kept:?}"));
    }
    let mut worst = 0.0f64;
    for (fnode, gl, gh) in bd.two_sided() {
        let Some(a) = low.iter().position(|p| p == fnode) else {
            continue;
        };
        let g = a / 3;
        let (wl, wh): (Weights, Weights) = if a % 3 == 1 {
            (vec![(low[3 * g], 1.0 / 3.0)], vec![(high[2 * g + 1], 2.0 / 3.0)])
        } else {
            (vec![(low[3 * g + 3], 1.0 / 3.0)], vec![(high[2 * g + 1], 2.0 / 3.0)])
        };
        for (got, want) in [(gl, &wl), (gh, &wh)] {
            let keys: std::collections::BTreeSet<usize> = got.iter().chain(want.iter()).map(|e| e.0).collect();
            for k in keys {
                let x = got.iter().filter(|e| e.0 == k).map(|e| e.1).sum::<f64>();
                let y = want.iter().filter(|e| e.0 == k).map(|e| e.1).sum::<f64>();
                worst = worst.max((x - y).abs());
            }
        }
    }
    println!(
        "J0 I2b: G against the arithmetic {worst:.3e} (limit <= 1e-12) {}",
        met(worst <= 1e-12)
    );
    if worst > 1e-12 || bd.two_sided().is_empty() {
        fails.push(format!("G {worst:e}"));
    }
    for (name, m, n) in [("I1", i1_sector(), 7), ("I2a", i2_sector(1, true).1, 13)] {
        let bd = boundary(&m, the_cyclic_constraint(&m).unwrap()).unwrap();
        let ok = bd.two_sided().is_empty() && bd.interpolated().len() == n;
        println!(
            "J0 {name}: two_sided {}, interpolated {} (expected 0, {n}) {}",
            bd.two_sided().len(),
            bd.interpolated().len(),
            met(ok)
        );
        if !ok {
            fails.push(name.to_string());
        }
    }
    assert!(fails.is_empty(), "{fails:?}");
}

// ---------------------------------------------------------------------------------------------- J1, J2, J3

/// The registered run of J1, J2 and J3: I2b at levels 1 and 2 against the matched sectors.
#[test]
fn j1_j2_j3_low_side_finer_against_the_matched_sector() {
    let mut fails = Vec::new();
    let mut d = [0.0f64; 2];
    for level in [1, 2] {
        let (nc, coarse) = (4 * level, 8 * level);
        let matched = frustum_model(
            frustum_mesh(ElementKind::Shell3, &columns(nc, coarse, coarse, None), &no_shift),
            shells(),
        );
        let (f, nm) = i2_sector(level, false);
        let hs = harmonics(&nm);
        let name = format!("J level {level}");
        d[level - 1] = sector_against_sector(&name, &hs, &harmonics(&matched));
        let rho = localisation(&name, &nm, &f.id, &hs);
        println!(
            "J2 level {level}: largest rho {rho:.4} (limit <= 2) {}",
            met(rho <= 2.0)
        );
        if rho > 2.0 {
            fails.push(format!("J2 level {level}: {rho}"));
        }
    }
    println!("J1: max_k d_k {:.4e} (limit <= 2e-2) {}", d[0], met(d[0] <= 2e-2));
    if d[0] > 2e-2 {
        fails.push(format!("J1: {:e}", d[0]));
    }
    let ratio = d[1] / d[0];
    println!(
        "J3: level 2 {:.4e}, ratio {ratio:.4} (limit <= 0.7) {}",
        d[1],
        met(ratio <= 0.7)
    );
    if ratio > 0.7 {
        fails.push(format!("J3: {ratio}"));
    }
    assert!(fails.is_empty(), "{fails:?}");
}

// ---------------------------------------------------------------------------------------------- R2

#[test]
fn r2_tied_low_nodes_follow_the_high_edge_rotated_back() {
    let (f, m) = i2_sector(1, false);
    let ci = the_cyclic_constraint(&m).unwrap();
    let bd = boundary(&m, ci).unwrap();
    let rt = transpose(bd.rotation());
    let h = modal_analysis_harmonic(&m, &[], 1, &modal_options(4)).unwrap();
    let dofs = m.dof_map();
    let (s, c) = (2.0 * PI / N as f64).sin_cos();
    let triple =
        |v: &[f64], p: usize, base: usize| -> Vec3 { [0, 1, 2].map(|k| v[dofs.equation(p, base + k).unwrap()]) };
    let (low, high) = (&f.id[0], &f.id[4]);
    let mut worst = 0.0f64;
    let mut checked = 0;
    for mode in &h.modes {
        let umax = mode.cos.iter().chain(&mode.sin).fold(0.0f64, |a, x| a.max(x.abs()));
        for (fnode, _, _) in bd.two_sided() {
            let a = low.iter().position(|p| p == fnode).unwrap();
            // s_f = a / 12 on the high column of 8 segments: b = floor(2a/3), t = (2a mod 3)/3.
            let (b, t) = ((2 * a) / 3, ((2 * a) % 3) as f64 / 3.0);
            for base in [0, 3] {
                let mut aa = [0.0; 3];
                let mut bb = [0.0; 3];
                for (node, w) in [(high[b], 1.0 - t), (high[b + 1], t)] {
                    let (ta, tb) = (triple(&mode.cos, node, base), triple(&mode.sin, node, base));
                    for k in 0..3 {
                        aa[k] += w * ta[k];
                        bb[k] += w * tb[k];
                    }
                }
                let ea = apply(&rt, [0, 1, 2].map(|k| c * aa[k] + s * bb[k]));
                let eb = apply(&rt, [0, 1, 2].map(|k| -s * aa[k] + c * bb[k]));
                let (fa, fb) = (triple(&mode.cos, *fnode, base), triple(&mode.sin, *fnode, base));
                for k in 0..3 {
                    worst = worst.max((fa[k] - ea[k]).abs() / umax);
                    worst = worst.max((fb[k] - eb[k]).abs() / umax);
                }
            }
            checked += 1;
        }
    }
    println!(
        "R2: {checked} node-modes, max residual {worst:.4e} (limit <= 1e-13) {}",
        met(worst <= 1e-13)
    );
    assert!(checked > 0 && worst <= 1e-13, "R2: {worst:e}");
}

// ---------------------------------------------------------------------------------------------- RA, RW, RW0: the cap

const CAP_NC: usize = 2;
const CAP_NA: usize = 3;

fn cap_point(phi: f64, psi: f64) -> Vec3 {
    let (n, ea, eb) = frame();
    let (sp, cp) = phi.sin_cos();
    let (ss, cs) = psi.sin_cos();
    [0, 1, 2].map(|k| ORIGIN[k] + ss * (cp * ea[k] + sp * eb[k]) + cs * n[k])
}

/// The cap of ADR 0575 D4 (the same construction as `sa_cyclic_shell_extensions.rs`).
fn cap_mesh(quads: bool, full: bool) -> SolidMesh {
    let columns = if full { N * CAP_NC } else { CAP_NC + 1 };
    let id = |i: usize, a: usize| 1 + (a - 1) * columns + i % columns;
    let mut nodes = vec![cap_point(0.0, 0.0)];
    for a in 1..=CAP_NA {
        for i in 0..columns {
            let phi = (PI / 3.0) * i as f64 / CAP_NC as f64;
            nodes.push(cap_point(phi, (PI / 3.0) * a as f64 / CAP_NA as f64));
        }
    }
    let strips = if full { columns } else { columns - 1 };
    let mut tri = Vec::new();
    let mut quad = Vec::new();
    for i in 0..strips {
        tri.extend([0, id(i, 1), id(i + 1, 1)]);
        for a in 1..CAP_NA {
            let q = [id(i, a), id(i + 1, a), id(i + 1, a + 1), id(i, a + 1)];
            if quads {
                quad.extend(q);
            } else {
                tri.extend([q[0], q[1], q[2], q[0], q[2], q[3]]);
            }
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell3, tri).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(T).unwrap()).unwrap();
    if quads {
        mesh.add_block(ElementKind::Shell4, quad).unwrap();
        mesh.set_block_property(1, BlockProperty::shell(T).unwrap()).unwrap();
    }
    let ne: usize = mesh.blocks().iter().map(|b| b.len()).sum();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("clamp", (0..columns).map(|i| id(i, CAP_NA)).collect())
        .unwrap();
    if !full {
        let set = |i: usize| -> Vec<usize> { std::iter::once(0).chain((1..=CAP_NA).map(|a| id(i, a))).collect() };
        mesh.add_node_set("low", set(0)).unwrap();
        mesh.add_node_set("high", set(CAP_NC)).unwrap();
    }
    mesh
}

fn cap_sector(quads: bool) -> SolidModel {
    let mut m = SolidModel::new(cap_mesh(quads, false));
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m.add_constraint(Constraint::cyclic_with(
        "low",
        "high",
        axis(),
        ORIGIN,
        N,
        extended().axis_nodes(true),
    ))
    .unwrap();
    m
}

fn cap_full(quads: bool) -> SolidModel {
    let mut m = SolidModel::new(cap_mesh(quads, true));
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m
}

/// The ADR 0561 D4 list protocol: sector lists per harmonic against the full list; returns the per-harmonic maxima.
fn compare(name: &str, sector: &[Vec<f64>], full: &[f64], fails: &mut Vec<String>) -> f64 {
    let cut = (1.0 - 1e-6)
        * sector
            .iter()
            .map(|l| l.iter().copied().fold(0.0f64, f64::max))
            .fold(f64::INFINITY, f64::min);
    let mut s: Vec<(f64, usize)> = sector
        .iter()
        .enumerate()
        .flat_map(|(k, l)| l.iter().copied().filter(|&v| v < cut).map(move |v| (v, k)))
        .collect();
    s.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut r: Vec<f64> = full.iter().copied().filter(|&v| v < cut).collect();
    r.sort_by(f64::total_cmp);
    let largest = full.iter().copied().fold(0.0f64, f64::max);
    println!(
        "{name}: largest full {largest:.6e}, cut-off {cut:.6e} (complete) {}; counts sector {} full {} {}",
        met(largest >= cut),
        s.len(),
        r.len(),
        met(s.len() == r.len())
    );
    if largest < cut {
        fails.push(format!("{name}: full list incomplete"));
    }
    if s.len() != r.len() {
        fails.push(format!("{name}: counts {} {}", s.len(), r.len()));
    }
    let mut per = vec![0.0f64; sector.len()];
    for ((v, k), w) in s.iter().zip(&r) {
        per[*k] = per[*k].max((v / w - 1.0).abs());
    }
    let worst = per.iter().copied().fold(0.0f64, f64::max);
    println!(
        "{name}: per harmonic [{}] (limit <= 1e-8) {}",
        per.iter().map(|x| format!("{x:.3e}")).collect::<Vec<_>>().join(", "),
        met(worst <= 1e-8)
    );
    if worst > 1e-8 {
        fails.push(format!("{name}: {worst:e}"));
    }
    worst
}

fn rotating_options(modes: usize) -> RotatingOptions {
    RotatingOptions::new(modes).modal(modal_options(modes))
}

fn rotating_cap(quads: bool, tag: &str) {
    let mut fails = Vec::new();
    let m = cap_sector(quads);
    let full = cap_full(quads);
    let w1 = harmonics(&m)
        .iter()
        .map(|h| h.eigenvalues[0])
        .fold(f64::INFINITY, f64::min)
        .sqrt();
    let speeds = [0.2 * w1, 0.4 * w1];
    println!("{tag}: omega_1 {w1:.6e} rad/s, speeds {speeds:?}");
    // RA: set A (stress stiffening and spin softening).
    let sec = RotatingSector::new(&m, &rotating_options(4)).unwrap();
    let part = RotatingPart::new(&full, axis(), ORIGIN, &rotating_options(24).shells(true)).unwrap();
    for (si, &w) in speeds.iter().enumerate() {
        let name = format!("RA{tag} speed {si} ({w:.4e} rad/s)");
        let hs: Vec<_> = (0..=N / 2).map(|k| sec.modes(k, w).unwrap()).collect();
        let rr = part.modes(w).unwrap();
        let (Some(rr), true) = (rr.modes(), hs.iter().all(|h| h.modes().is_some())) else {
            fails.push(format!("{name}: loss of stiffness"));
            continue;
        };
        let lists: Vec<Vec<f64>> = hs
            .iter()
            .map(|h| h.modes().unwrap().eigenvalues.iter().map(|l| l.sqrt()).collect())
            .collect();
        let full_list: Vec<f64> = rr.eigenvalues.iter().map(|l| l.sqrt()).collect();
        compare(&name, &lists, &full_list, &mut fails);
    }
    // RW: the whirl with the Coriolis matrix.
    let secw = RotatingSector::new(&m, &rotating_options(4).coriolis(true)).unwrap();
    let partw = RotatingPart::new(&full, axis(), ORIGIN, &rotating_options(24).coriolis(true).shells(true)).unwrap();
    for (si, &w) in speeds.iter().enumerate() {
        let name = format!("RW{tag} speed {si} ({w:.4e} rad/s)");
        let hs: Vec<_> = (0..=N / 2).map(|k| secw.whirl_modes(k, w).unwrap()).collect();
        let rr = partw.whirl_modes(w).unwrap();
        let (Some(rr), true) = (rr.modes(), hs.iter().all(|h| h.modes().is_some())) else {
            fails.push(format!("{name}: loss of stiffness"));
            continue;
        };
        let lists: Vec<Vec<f64>> = hs
            .iter()
            .map(|h| h.modes().unwrap().modes.iter().map(|x| x.omega).collect())
            .collect();
        let full_list: Vec<f64> = rr.modes.iter().map(|x| x.omega).collect();
        compare(&name, &lists, &full_list, &mut fails);
    }
    // RW0 (set-up, A1 only): the Coriolis term is felt at k = 1, top speed.
    if quads {
        let w2 = speeds[1];
        let mut a: Vec<f64> = sec
            .modes(1, w2)
            .unwrap()
            .modes()
            .unwrap()
            .eigenvalues
            .iter()
            .map(|l| l.sqrt())
            .collect();
        let mut b: Vec<f64> = secw
            .whirl_modes(1, w2)
            .unwrap()
            .modes()
            .unwrap()
            .modes
            .iter()
            .map(|x| x.omega)
            .collect();
        a.sort_by(f64::total_cmp);
        b.sort_by(f64::total_cmp);
        let cut = (1.0 - 1e-6) * a.last().unwrap().min(*b.last().unwrap());
        let w0 = a
            .iter()
            .zip(&b)
            .filter(|(x, y)| **x < cut && **y < cut)
            .map(|(x, y)| (y / x - 1.0).abs())
            .fold(0.0f64, f64::max);
        println!(
            "RW0: Coriolis effect at k = 1, top speed {w0:.4e} (limit >= 1e-4) {}",
            met(w0 >= 1e-4)
        );
        if w0 < 1e-4 {
            fails.push(format!("RW0: {w0:e}"));
        }
    }
    assert!(fails.is_empty(), "{tag}: {fails:?}");
}

#[test]
fn ra4_rw4_rotating_cap_shell4_with_shell3_at_the_pole_against_the_full_model() {
    rotating_cap(true, "4");
}

#[test]
fn ra3_rw3_rotating_cap_shell3_against_the_full_model() {
    rotating_cap(false, "3");
}

// ---------------------------------------------------------------------------------------------- B4

/// The FNV-1a 64 of the bits of the `k = 0 .. 3` frequencies of I1 and of I2a level 1.
fn b4_hash() -> u64 {
    let mut bits = Vec::new();
    for m in [i1_sector(), i2_sector(1, true).1] {
        for h in harmonics(&m) {
            bits.extend(h.frequencies.iter().map(|v| v.to_bits()));
        }
    }
    fnv(&bits)
}

/// Recorded by the set-up run of ADR 0583 on the unchanged library (32b95ac2 with this file's B4 part only;
/// `f662-scratch/setup_b4.log`: 9407375498072338308).
const B4_HASH: u64 = 0x828d_b62c_4f50_af84;

#[test]
fn b4_i1_and_i2a_keep_their_bits() {
    let h = b4_hash();
    println!("B4: hash {h:#018x} (recorded {B4_HASH:#018x}) {}", met(h == B4_HASH));
    assert_eq!(h, B4_HASH);
}

// ---------------------------------------------------------------------------------------------- NC8, NC9

fn rebuilt(f: &Frustum, nodes: Vec<Vec3>, extra: Option<(Vec<usize>, Vec<usize>)>) -> SolidMesh {
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell3, f.mesh.blocks()[0].connectivity().to_vec())
        .unwrap();
    mesh.set_block_property(0, BlockProperty::shell(T).unwrap()).unwrap();
    let high = match extra {
        Some((hex, high)) => {
            mesh.add_block(ElementKind::Hex8, hex).unwrap();
            high
        }
        None => f.id[4].clone(),
    };
    let ne: usize = mesh.blocks().iter().map(|b| b.len()).sum();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("clamp", f.id.iter().map(|c| c[0]).collect()).unwrap();
    mesh.add_node_set("low", f.id[0].clone()).unwrap();
    mesh.add_node_set("high", high).unwrap();
    mesh
}

#[test]
fn nc8_nc9_refusals() {
    let mut fails = Vec::new();
    let modal_k1 = |m: &SolidModel| modal_analysis_harmonic(m, &[], 1, &modal_options(4)).map(|_| ());
    // NC8: the low node (0, 1) moved along the radius by 0.2 of the mean low segment, tolerance 0.15 of it.
    let f = i2_sector(1, false).0;
    let l = f.id[0][1];
    let x0: Vec<Vec3> = f.id[0].iter().map(|&p| f.mesh.nodes()[p]).collect();
    let mean = x0
        .windows(2)
        .map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2) + (w[1][2] - w[0][2]).powi(2)).sqrt())
        .sum::<f64>()
        / (x0.len() - 1) as f64;
    let mut nodes = f.mesh.nodes().to_vec();
    let (n, _, _) = frame();
    let p = nodes[l];
    let d = [p[0] - ORIGIN[0], p[1] - ORIGIN[1], p[2] - ORIGIN[2]];
    let along = d[0] * n[0] + d[1] * n[1] + d[2] * n[2];
    let radial = [0, 1, 2].map(|k| d[k] - along * n[k]);
    let rl = (radial[0].powi(2) + radial[1].powi(2) + radial[2].powi(2)).sqrt();
    nodes[l] = [0, 1, 2].map(|k| p[k] + 0.2 * mean * radial[k] / rl);
    let m8 = frustum_model(
        Frustum {
            mesh: rebuilt(&f, nodes, None),
            id: f.id.clone(),
        },
        extended().interpolation(nts().tolerance(0.15 * mean)),
    );
    let r = modal_k1(&m8);
    let ok = matches!(&r, Err(FemError::TieGap { node, .. }) if *node == l);
    println!("NC8 a tied low node off the high edge: {r:?} {}", met(ok));
    if !ok {
        fails.push(format!("NC8: {r:?}"));
    }
    // NC9: the NC6 model of ADR 0575, a three-unknown Hex8 node in place of the high node (4, 3).
    let f = i2_sector(1, false).0;
    let h = f.id[4][3];
    let q = f.mesh.nodes()[h];
    let mut nodes = f.mesh.nodes().to_vec();
    let base = nodes.len();
    let hb = hex_box([0.0; 3], [0.05; 3], [1, 1, 1]).unwrap();
    for x in hb.nodes() {
        nodes.push([q[0] + x[0], q[1] + x[1], q[2] + x[2]]);
    }
    let corner = base + hb.nodes().iter().position(|x| *x == [0.0; 3]).unwrap();
    let hex: Vec<usize> = hb.blocks()[0].connectivity().iter().map(|&p| p + base).collect();
    let mut high: Vec<usize> = f.id[4].iter().copied().filter(|&p| p != h).collect();
    high.push(corner);
    let m9 = frustum_model(
        Frustum {
            mesh: rebuilt(&f, nodes, Some((hex, high))),
            id: f.id.clone(),
        },
        extended().interpolation(nts()),
    );
    let r = modal_k1(&m9);
    let text = format!("high node {corner} has 3 unknowns");
    let ok = matches!(&r, Err(e @ FemError::InvalidValue { key: "cyclic", .. }) if e.to_string().contains(&text) && e.to_string().contains("has 6"));
    println!("NC9 a three-unknown high node: {r:?} {}", met(ok));
    if !ok {
        fails.push(format!("NC9: {r:?}"));
    }
    assert!(fails.is_empty(), "{fails:#?}");
}
