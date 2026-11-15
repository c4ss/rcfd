//! F-656 (ADR 0579): the curvature shell pressure on a non-quadric shell. A pressurised torus segment (Shell3, symmetry cuts,
//! the edge `phi = pi/4` clamped at the membrane state) against the membrane solution of the torus at two refinements, with
//! `Load::shell_pressure_curvature` (the ADR 0567 quadric) and with the opt-in boundary estimator `CurvatureBoundary::Jet`;
//! the boundary-node curvature error of the ADR 0567 torus patch; the bits of the default. The checks of the registration;
//! every limit is the registered one and every prototype number is that of `f656-scratch/proto`. The octant generator, the
//! cylinder quarter and the torus patch are copied from `sa_shell_pressure_curvature.rs` (ADR 0567).

use rcfd_core::vec3::{cross, dot, norm, Vec3};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::shell3_curvature::{curvature_estimates, curvature_estimates_with, CurvatureBoundary};
use rcfd_fem::solve::{solve_static_with, StaticOptions};
use rcfd_fem::{BlockProperty, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use std::collections::HashMap;
use std::f64::consts::PI;

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{name}: {value:.4} (limit >= {limit}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value} < {limit}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.4e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const T: f64 = 0.01;
const P: f64 = 1e5;
const R: f64 = 1.0;

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// The 64-bit linear congruential generator of ADR 0550 design 2.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn min_angle(x: [Vec3; 3]) -> f64 {
    let mut m = 180.0f64;
    for i in 0..3 {
        let u = sub(x[(i + 1) % 3], x[i]);
        let v = sub(x[(i + 2) % 3], x[i]);
        m = m.min(norm(cross(u, v)).atan2(dot(u, v)).to_degrees());
    }
    m
}

fn outward(x: [Vec3; 3]) -> bool {
    let c = [
        (x[0][0] + x[1][0] + x[2][0]) / 3.0,
        (x[0][1] + x[1][1] + x[2][1]) / 3.0,
        (x[0][2] + x[1][2] + x[2][2]) / 3.0,
    ];
    dot(cross(sub(x[1], x[0]), sub(x[2], x[0])), c) > 0.0
}

struct Mesh {
    nodes: Vec<Vec3>,
    tris: Vec<[usize; 3]>,
    #[allow(dead_code)]
    flips: usize,
}

/// The octant mesh of ADR 0550 design 2: the geodesic grid of level `n`, nodes jittered by `jitter` grid units, one pass of
/// random flips with probability `flip` (smallest angle 20 degrees), seed `546 + n`.
fn mesh(n: usize, jitter: f64, flip: f64) -> Mesh {
    let mut rng = Lcg(546 + n as u64);
    let mut idx = HashMap::new();
    let mut nodes = Vec::new();
    for i in 0..=n {
        for j in 0..=(n - i) {
            let k = n - i - j;
            let a = (2.0 * rng.next() - 1.0) * jitter;
            let b = (2.0 * rng.next() - 1.0) * jitter;
            let (mut bi, mut bj, mut bk) = (i as f64, j as f64, k as f64);
            let zeros = (i == 0) as usize + (j == 0) as usize + (k == 0) as usize;
            if zeros == 0 {
                bi = i as f64 + a;
                bj = j as f64 + b;
                bk = k as f64 - a - b;
            } else if zeros == 1 {
                if k == 0 {
                    bi = i as f64 + a;
                    bj = j as f64 - a;
                } else if i == 0 {
                    bj = j as f64 + a;
                    bk = k as f64 - a;
                } else {
                    bi = i as f64 + a;
                    bk = k as f64 - a;
                }
            }
            let v = [bi, bj, bk];
            let s = norm(v);
            idx.insert((i, j), nodes.len());
            nodes.push([R * v[0] / s, R * v[1] / s, R * v[2] / s]);
        }
    }
    let mut tris = Vec::new();
    for i in 0..n {
        for j in 0..(n - i) {
            tris.push([idx[&(i, j)], idx[&(i + 1, j)], idx[&(i, j + 1)]]);
            if i + j + 1 < n {
                tris.push([idx[&(i + 1, j)], idx[&(i + 1, j + 1)], idx[&(i, j + 1)]]);
            }
        }
    }
    let mut edges = HashMap::new();
    for (t, tri) in tris.iter().enumerate() {
        for e in 0..3 {
            edges.insert((tri[e], tri[(e + 1) % 3]), t);
        }
    }
    let mut used = vec![false; tris.len()];
    let mut flips = 0;
    for t in 0..tris.len() {
        for e in 0..3 {
            if used[t] {
                break;
            }
            let (a, b) = (tris[t][e], tris[t][(e + 1) % 3]);
            let t2 = match edges.get(&(b, a)) {
                Some(&t2) if t2 > t => t2,
                _ => continue,
            };
            let u = rng.next();
            if used[t2] || u >= flip {
                continue;
            }
            let c = tris[t][(e + 2) % 3];
            let d = *tris[t2].iter().find(|&&q| q != a && q != b).unwrap();
            let (n1, n2) = ([a, d, c], [d, b, c]);
            let x1 = n1.map(|q| nodes[q]);
            let x2 = n2.map(|q| nodes[q]);
            if !(outward(x1) && outward(x2)) {
                continue;
            }
            if min_angle(x1).min(min_angle(x2)) < 20.0 {
                continue;
            }
            tris[t] = n1;
            tris[t2] = n2;
            used[t] = true;
            used[t2] = true;
            flips += 1;
        }
    }
    Mesh { nodes, tris, flips }
}

fn material() -> Material {
    Material::isotropic_elastic(E, NU, 7850.0, 0.0).unwrap()
}

/// A model of one Shell3 block, element set and node set "all".
fn shell3_model(nodes: &[Vec3], tris: &[[usize; 3]], t: f64) -> SolidModel {
    let mut mesh = SolidMesh::new(nodes.to_vec()).unwrap();
    mesh.add_block(ElementKind::Shell3, tris.iter().flatten().copied().collect())
        .unwrap();
    mesh.set_block_property(0, BlockProperty::shell(t).unwrap()).unwrap();
    mesh.add_element_set("all", (0..tris.len()).collect()).unwrap();
    mesh.add_node_set("all", (0..nodes.len()).collect()).unwrap();
    let mut model = SolidModel::new(mesh);
    model.add_material("all", material()).unwrap();
    model
}

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum Pattern {
    One,
    Other,
    Checker,
}

#[allow(dead_code)]
struct Cylinder {
    nodes: Vec<Vec3>,
    tris: Vec<[usize; 3]>,
    nq: usize,
    nz: usize,
}

/// The cylinder quarter of ADR 0461 clause (a1) (R = 1, H = 0.5) on `nq x nz` cells, nodes numbered `(nz + 1) i + j`, each cell
/// cut by `pattern` (`One`: `q0-q2`), the interior rows moved axially by `warp` cells (ADR 0461 design 5).
fn cylinder_mesh(nq: usize, nz: usize, pattern: Pattern, warp: f64) -> Cylinder {
    let lh = 0.5;
    let idx = |i: usize, j: usize| (nz + 1) * i + j;
    let dz = lh / nz as f64;
    let mut nodes = Vec::new();
    for i in 0..=nq {
        let ph = PI / 2.0 * i as f64 / nq as f64;
        for j in 0..=nz {
            let mut z = lh * j as f64 / nz as f64;
            if warp != 0.0 {
                z += warp * dz * (3.0 * PI * i as f64 / nq as f64).sin() * (PI * j as f64 / nz as f64).sin();
            }
            nodes.push([R * ph.cos(), R * ph.sin(), z]);
        }
    }
    let mut tris = Vec::new();
    for i in 0..nq {
        for j in 0..nz {
            let q = [idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)];
            let d = match pattern {
                Pattern::One => 0,
                Pattern::Other => 1,
                Pattern::Checker => (i + j) % 2,
            };
            if d == 0 {
                tris.push([q[0], q[1], q[2]]);
                tris.push([q[0], q[2], q[3]]);
            } else {
                tris.push([q[0], q[1], q[3]]);
                tris.push([q[1], q[2], q[3]]);
            }
        }
    }
    Cylinder { nodes, tris, nq, nz }
}

/// The outer quarter-patch of a torus (ADR 0567 design 3.4): centre radius 1, tube radius 0.4, `theta` in `[0, pi/2]`, `phi`
/// in `[-pi/4, pi/4]`, `N x N` cells cut along `q0-q2`, nodes numbered `(N + 1) i + j`, the interior nodes jittered by
/// `jitter` cells (LCG seed `633 + N`, two draws per node). Returns the mesh and `phi` per node.
fn torus(n: usize, jitter: f64) -> (Vec<Vec3>, Vec<[usize; 3]>, Vec<f64>) {
    let (r0, r) = (1.0, 0.4);
    let mut rng = Lcg(633 + n as u64);
    let idx = |i: usize, j: usize| (n + 1) * i + j;
    let mut nodes = Vec::new();
    let mut phis = Vec::new();
    for i in 0..=n {
        for j in 0..=n {
            let (mut a, mut b) = (i as f64 / n as f64, j as f64 / n as f64);
            let da = (2.0 * rng.next() - 1.0) * jitter;
            let db = (2.0 * rng.next() - 1.0) * jitter;
            if i > 0 && i < n && j > 0 && j < n {
                a += da / n as f64;
                b += db / n as f64;
            }
            let (th, ph) = (a * PI / 2.0, b * PI / 2.0 - PI / 4.0);
            let rr = r0 + r * ph.cos();
            nodes.push([rr * th.cos(), rr * th.sin(), r * ph.sin()]);
            phis.push(ph);
        }
    }
    let mut tris = Vec::new();
    for i in 0..n {
        for j in 0..n {
            let q = [idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)];
            tris.push([q[0], q[1], q[2]]);
            tris.push([q[0], q[2], q[3]]);
        }
    }
    (nodes, tris, phis)
}

fn unstructured(n: usize) -> Mesh {
    mesh(n, 0.2, 0.5)
}

/// The 3 x 2-cell torus patch of RF1 (ADR 0579): the ADR 0567 generator with `nt` cells in `theta` and `nf` in `phi`, every
/// node's `rho` changed by `0.01 ((k^2 mod 7) - 3) / 3`, `k` the node number.
fn perturbed_patch(nt: usize, nf: usize) -> (Vec<Vec3>, Vec<[usize; 3]>) {
    let (r0, r) = (1.0, 0.4);
    let idx = |i: usize, j: usize| (nf + 1) * i + j;
    let mut nodes = Vec::new();
    for i in 0..=nt {
        for j in 0..=nf {
            let (th, ph) = (
                i as f64 / nt as f64 * PI / 2.0,
                j as f64 / nf as f64 * PI / 2.0 - PI / 4.0,
            );
            let k = idx(i, j);
            let rr = r0 + r * ph.cos() + 0.01 * (((k * k) % 7) as f64 - 3.0) / 3.0;
            nodes.push([rr * th.cos(), rr * th.sin(), r * ph.sin()]);
        }
    }
    let mut tris = Vec::new();
    for i in 0..nt {
        for j in 0..nf {
            let q = [idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)];
            tris.push([q[0], q[1], q[2]]);
            tris.push([q[0], q[2], q[3]]);
        }
    }
    (nodes, tris)
}

/// FNV-1a 64 over the little-endian bytes of `to_bits()` of every entry (ADR 0579 design 5).
fn fnv(v: &[f64]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for x in v {
        for b in x.to_bits().to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

/// The default load vector of ADR 0567 on a Shell3 set, `t = 0.01`, `p = 1e5`.
fn quadric_load(nodes: &[Vec3], tris: &[[usize; 3]]) -> Vec<f64> {
    let mut m = shell3_model(nodes, tris, T);
    m.add_load(Load::shell_pressure_curvature("all", P)).unwrap();
    nodal_load_vector(&m, &[]).unwrap()
}

/// The load vector with the boundary jet (ADR 0579).
fn jet_load(nodes: &[Vec3], tris: &[[usize; 3]], p: f64) -> Vec<f64> {
    let mut m = shell3_model(nodes, tris, T);
    m.add_load(Load::shell_pressure_curvature_with("all", p, CurvatureBoundary::Jet))
        .unwrap();
    nodal_load_vector(&m, &[]).unwrap()
}

fn bits_equal(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

const R0: f64 = 1.0;
const RT: f64 = 0.4;
/// The thickness of the torus segment (ADR 0579 design 2).
const TS: f64 = 0.001;

/// The membrane solution of the pressurised torus (ADR 0579 design 2): `(w, v)` at `phi`, `v(0) = 0`.
fn membrane(ph: f64) -> (f64, f64) {
    let c0 = P * RT / (2.0 * E * TS);
    let rho = R0 + RT * ph.cos();
    let k = ((R0 - RT) / (R0 + RT)).sqrt();
    let i = RT * ph.sin().atanh()
        - RT * RT * 2.0 / (R0 * R0 - RT * RT).sqrt() * (k * (ph / 2.0).tan()).atan()
        - (1.0 - 2.0 * NU) * R0 * ph.tan();
    let v = ph.cos() * c0 * i;
    let w = (c0 * ((1.0 - NU) * rho - NU * R0) + v * ph.sin()) / ph.cos();
    (w, v)
}

fn normal_at(th: f64, ph: f64) -> Vec3 {
    [ph.cos() * th.cos(), ph.cos() * th.sin(), ph.sin()]
}

fn meridional_at(th: f64, ph: f64) -> Vec3 {
    [-ph.sin() * th.cos(), -ph.sin() * th.sin(), ph.cos()]
}

/// A mesh with the `(theta, phi)` of each node.
type Segment = (Vec<Vec3>, Vec<[usize; 3]>, Vec<(f64, f64)>);

/// The torus segment of ADR 0579 design 2: `6 n x n` cells, nodes `(n + 1) i + j`; returns nodes, triangles and the node
/// parameters `(theta, phi)`.
fn segment(n: usize) -> Segment {
    let nt = 6 * n;
    let idx = |i: usize, j: usize| (n + 1) * i + j;
    let mut nodes = Vec::new();
    let mut par = Vec::new();
    for i in 0..=nt {
        for j in 0..=n {
            let th = i as f64 / nt as f64 * PI / 2.0;
            let ph = j as f64 / n as f64 * PI / 4.0;
            let rr = R0 + RT * ph.cos();
            nodes.push([rr * th.cos(), rr * th.sin(), RT * ph.sin()]);
            par.push((th, ph));
        }
    }
    let mut tris = Vec::new();
    for i in 0..nt {
        for j in 0..n {
            let q = [idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)];
            tris.push([q[0], q[1], q[2]]);
            tris.push([q[0], q[2], q[3]]);
        }
    }
    (nodes, tris, par)
}

#[derive(Clone, Copy)]
enum SegmentLoad {
    Jet,
    Quadric,
    Default,
}

/// The segment at `n` under `loading`: `(e_w, e_v)` (ADR 0579 design 2, measures).
fn segment_errors(n: usize, loading: SegmentLoad) -> (f64, f64) {
    let (nodes, tris, par) = segment(n);
    let mut model = shell3_model(&nodes, &tris, TS);
    model
        .add_load(match loading {
            SegmentLoad::Jet => Load::shell_pressure_curvature_with("all", P, CurvatureBoundary::Jet),
            SegmentLoad::Quadric => Load::shell_pressure_curvature("all", P),
            SegmentLoad::Default => Load::shell_pressure("all", P),
        })
        .unwrap();
    let mut pres: Vec<Prescribed> = Vec::new();
    for (q, x) in nodes.iter().enumerate() {
        let (th, ph) = par[q];
        if q % (n + 1) == n {
            let (w, v) = membrane(ph);
            let (nn, ep) = (normal_at(th, ph), meridional_at(th, ph));
            let hh = 1e-5;
            let dw = (membrane(ph + hh).0 - membrane(ph - hh).0) / (2.0 * hh);
            let beta = (dw - v) / RT;
            let eth = [-th.sin(), th.cos(), 0.0];
            for c in 0..3 {
                pres.push(Prescribed::new(q, c, w * nn[c] + v * ep[c]));
                pres.push(Prescribed::new(q, 3 + c, beta * eth[c]));
            }
            continue;
        }
        for (axis, comps) in [(0, [0, 4, 5]), (1, [1, 3, 5]), (2, [2, 3, 4])] {
            if x[axis].abs() < 1e-12 {
                for comp in comps {
                    if !pres.iter().any(|p| p.node == q && p.component == comp) {
                        pres.push(Prescribed::new(q, comp, 0.0));
                    }
                }
            }
        }
    }
    let r = solve_static_with(&model, &pres, &[], &StaticOptions::default()).unwrap();
    let (mut dw, mut dv, mut mw, mut mv) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for (q, &(th, ph)) in par.iter().enumerate() {
        let u = r.displacement(q);
        let (w, v) = membrane(ph);
        let wh = dot([u[0], u[1], u[2]], normal_at(th, ph));
        let vh = dot([u[0], u[1], u[2]], meridional_at(th, ph));
        dw = dw.max((wh - w).abs());
        dv = dv.max((vh - v).abs());
        mw = mw.max(w.abs());
        mv = mv.max(v.abs());
    }
    (dw / mw, dv / mv)
}

#[test]
fn torus_segment_against_the_membrane_solution() {
    let mut c = Checks::new();
    // M1
    let mut m1 = 0.0f64;
    let mut wmax = 0.0f64;
    for j in 0..=32 {
        let ph = j as f64 * PI / 128.0;
        let (w, v) = membrane(ph);
        let hh = 1e-5;
        let dv = (membrane(ph + hh).1 - membrane(ph - hh).1) / (2.0 * hh);
        let rho = R0 + RT * ph.cos();
        let nt = P * RT / 2.0;
        let nf = P * RT * (rho + R0) / (2.0 * rho);
        let et = (nt - NU * nf) / (E * TS);
        let ef = (nf - NU * nt) / (E * TS);
        m1 = m1
            .max((RT * ef - (dv + w)).abs())
            .max((rho * et - (w * ph.cos() - v * ph.sin())).abs());
        wmax = wmax.max(w.abs());
    }
    c.at_most("M1 membrane solution: strain residuals / max |w|", m1 / wmax, 1e-7);

    let (jw16, jv16) = segment_errors(16, SegmentLoad::Jet);
    let (jw32, jv32) = segment_errors(32, SegmentLoad::Jet);
    println!("segment Jet: n = 16 e_w {jw16:.4e} (prototype 2.36e-3) e_v {jv16:.4e} (1.83e-3)");
    println!("segment Jet: n = 32 e_w {jw32:.4e} (prototype 4.08e-4) e_v {jv32:.4e} (5.26e-4)");
    c.at_most("D1 segment n = 16 Jet: e_w", jw16, 5e-3);
    c.at_most("D1 segment n = 16 Jet: e_v", jv16, 4e-3);
    c.at_most("D2 segment n = 32 Jet: e_w", jw32, 1e-3);
    c.at_most("D2 segment n = 32 Jet: e_v", jv32, 1.2e-3);
    c.at_least("D3 Jet order of e_w", (jw16 / jw32).log2(), 1.5);
    c.at_least("D3 Jet order of e_v", (jv16 / jv32).log2(), 1.5);

    let (qw16, qv16) = segment_errors(16, SegmentLoad::Quadric);
    let (qw32, qv32) = segment_errors(32, SegmentLoad::Quadric);
    println!("segment Quadric: n = 16 e_w {qw16:.4e} (prototype 1.69e-2) e_v {qv16:.4e} (1.14e-2)");
    println!("segment Quadric: n = 32 e_w {qw32:.4e} (prototype 3.72e-3) e_v {qv32:.4e} (2.13e-3)");
    c.at_most("Q1 segment n = 16 Quadric: e_w", qw16, 4e-2);
    c.at_most("Q1 segment n = 16 Quadric: e_v", qv16, 3e-2);
    c.at_most("Q2 segment n = 32 Quadric: e_w", qw32, 8e-3);
    c.at_most("Q2 segment n = 32 Quadric: e_v", qv32, 5e-3);
    c.at_least("Q3 Quadric order of e_w", (qw16 / qw32).log2(), 1.5);
    c.at_least("Q3 Quadric order of e_v", (qv16 / qv32).log2(), 1.5);

    for (tag, j, q) in [
        ("D4 n = 16 e_w", jw16, qw16),
        ("D4 n = 16 e_v", jv16, qv16),
        ("D4 n = 32 e_w", jw32, qw32),
        ("D4 n = 32 e_v", jv32, qv32),
    ] {
        c.holds(
            tag,
            j < q,
            &format!("Jet {j:.4e} < Quadric {q:.4e} (ratio {:.2})", q / j),
        );
    }

    let (dw32, dv32) = segment_errors(32, SegmentLoad::Default);
    println!("segment default: n = 32 e_w {dw32:.4e} (prototype 7.40e-2) e_v {dv32:.4e}");
    c.above("NC1 segment n = 32 default shell_pressure: e_w", dw32, 2e-2);
    c.finish();
}

/// `|kappa / kappa_exact - 1|` per node of the torus patch with the boundary jet, the `jet` flags, the boundary distances
/// and the estimates of both estimators.
fn patch_jet(n: usize, jitter: f64) -> (Vec<f64>, Vec<bool>, Vec<usize>, bool) {
    let (r0, r) = (1.0, 0.4);
    let (nodes, tris, phis) = torus(n, jitter);
    let est = curvature_estimates_with(&nodes, &tris, CurvatureBoundary::Jet).unwrap();
    let base = curvature_estimates(&nodes, &tris).unwrap();
    let mut errs = Vec::new();
    let mut jet = Vec::new();
    let mut dist = Vec::new();
    let mut same = true;
    for ((q, e), (qb, b)) in est.iter().zip(&base) {
        let ph = phis[*q];
        let exact = (r0 + 2.0 * r * ph.cos()) / (2.0 * r * (r0 + r * ph.cos()));
        errs.push((e.kappa / exact - 1.0).abs());
        jet.push(e.jet);
        let (i, j) = (q / (n + 1), q % (n + 1));
        let d = i.min(j).min(n - i).min(n - j);
        dist.push(d);
        if d >= 2 {
            same &= q == qb
                && e.kappa.to_bits() == b.kappa.to_bits()
                && e.rings == b.rings
                && e.normal.iter().zip(&b.normal).all(|(x, y)| x.to_bits() == y.to_bits());
        }
    }
    (errs, jet, dist, same)
}

#[test]
fn boundary_jet_on_the_torus_patch() {
    let mut c = Checks::new();
    let max = |v: &[f64], m: &[bool]| v.iter().zip(m).filter(|(_, &k)| k).map(|(x, _)| *x).fold(0.0, f64::max);
    let (e32, j32, d32, s32) = patch_jet(32, 0.0);
    let all = vec![true; e32.len()];
    let k1 = max(&e32, &all);
    println!("torus patch structured N = 32 Jet: all nodes {k1:.4e} (prototype 9.26e-3; ADR 0567 1.3573e-1)");
    c.at_most("K1 torus patch structured N = 32 Jet: all nodes", k1, 2e-2);
    let (e32j, j32j, d32j, s32j) = patch_jet(32, 0.2);
    let k2 = max(&e32j, &vec![true; e32j.len()]);
    println!("torus patch jittered N = 32 Jet: all nodes {k2:.4e} (prototype 1.08e-2; ADR 0567 1.3742e-1)");
    c.at_most("K2 torus patch jittered N = 32 Jet: all nodes", k2, 2e-2);
    let (e64, j64, _, _) = patch_jet(64, 0.0);
    let k1j = max(&e32, &j32);
    let k3 = max(&e64, &j64);
    println!("torus patch structured Jet nodes: N = 32 {k1j:.4e}, N = 64 {k3:.4e} (prototype 1.14e-3)");
    c.at_most("K3 torus patch structured N = 64 Jet: jet nodes", k3, 3e-3);
    c.at_least("K3 order log2(N32 / N64) at the jet nodes", (k1j / k3).log2(), 2.0);
    for (tag, j, d, s) in [("structured", &j32, &d32, s32), ("jittered", &j32j, &d32j, s32j)] {
        let near = d.iter().filter(|&&x| x <= 1).count();
        let flags = j.iter().zip(d.iter()).all(|(&jj, &dd)| jj == (dd <= 1));
        c.holds(
            &format!(
                "K4 torus patch {tag} N = 32: jet exactly at the {near} nodes of distance <= 1, others bit-identical"
            ),
            near == 248 && flags && s,
            &format!("count {near}, flags {flags}, others identical {s}"),
        );
    }
    c.finish();
}

#[test]
fn boundary_jet_keeps_the_quadric_bits() {
    let mut c = Checks::new();
    let m16 = unstructured(16);
    let m32 = unstructured(32);
    let o1 = cylinder_mesh(32, 16, Pattern::One, 0.0);
    for (tag, nodes, tris) in [
        ("octant n = 16", &m16.nodes, &m16.tris),
        ("octant n = 32", &m32.nodes, &m32.tris),
        ("cylinder O1", &o1.nodes, &o1.tris),
    ] {
        let jets = curvature_estimates_with(nodes, tris, CurvatureBoundary::Jet)
            .unwrap()
            .iter()
            .filter(|(_, e)| e.jet)
            .count();
        let same = bits_equal(&jet_load(nodes, tris, P), &quadric_load(nodes, tris));
        c.holds(
            &format!("K5 {tag}: no jet node and the Jet load bit-identical to the Quadric load"),
            jets == 0 && same,
            &format!("jet nodes {jets}, bits equal {same}"),
        );
    }
    // B1
    let h = fnv(&quadric_load(&m16.nodes, &m16.tris));
    c.holds(
        "B1 octant n = 16 default load digest",
        h == 0xc79b1b660e85edc7,
        &format!("{h:016x}"),
    );
    let h = fnv(&quadric_load(&o1.nodes, &o1.tris));
    c.holds(
        "B1 cylinder O1 default load digest",
        h == 0x7f104d16c739c930,
        &format!("{h:016x}"),
    );
    let (nodes, tris, _) = torus(16, 0.2);
    let h = fnv(&quadric_load(&nodes, &tris));
    c.holds(
        "B1 torus patch N = 16 jittered default load digest",
        h == 0x30fe5e5c181287d3,
        &format!("{h:016x}"),
    );
    let (nodes, tris, _) = torus(32, 0.0);
    let mut v = Vec::new();
    for (_, e) in &curvature_estimates(&nodes, &tris).unwrap() {
        v.push(e.kappa);
        v.extend_from_slice(&e.normal);
        v.push(e.rings as f64);
    }
    let h = fnv(&v);
    c.holds(
        "B1 torus patch N = 32 curvature_estimates digest",
        h == 0x323bfcb68bd39ddf,
        &format!("{h:016x}"),
    );
    c.finish();
}

#[test]
fn s1_boundary_jet_load_is_odd_in_the_pressure() {
    let mut c = Checks::new();
    let (nodes, tris, _) = torus(16, 0.2);
    let fp = jet_load(&nodes, &tris, P);
    let fm = jet_load(&nodes, &tris, -P);
    let same = fp.len() == fm.len()
        && fp
            .iter()
            .zip(&fm)
            .all(|(a, b)| (-a).to_bits() == b.to_bits() || (*a == 0.0 && *b == 0.0));
    let nonzero = fp.iter().filter(|a| **a != 0.0).count();
    c.holds(
        "S1 torus patch N = 16 jittered Jet: the load of -p is the load of p negated (zeros of either sign equal)",
        same,
        &format!("{same} ({nonzero} non-zero entries of {})", fp.len()),
    );
    c.finish();
}

#[test]
fn rf1_boundary_jet_refuses_a_small_stencil() {
    let mut c = Checks::new();
    let (nodes, tris) = perturbed_patch(3, 2);
    let mut m = shell3_model(&nodes, &tris, T);
    m.add_load(Load::shell_pressure_curvature_with("all", P, CurvatureBoundary::Jet))
        .unwrap();
    let r = nodal_load_vector(&m, &[]);
    c.holds(
        "RF1 3 x 2 perturbed patch under Jet: refused with key curvature",
        matches!(&r, Err(FemError::InvalidValue { key: "curvature", .. })),
        &format!("{:?}", r.as_ref().err()),
    );
    let mut m = shell3_model(&nodes, &tris, T);
    m.add_load(Load::shell_pressure_curvature("all", P)).unwrap();
    let r = nodal_load_vector(&m, &[]);
    c.holds(
        "RF1 the same patch under Quadric: accepted",
        r.is_ok(),
        &format!("{:?}", r.as_ref().err()),
    );
    c.finish();
}
