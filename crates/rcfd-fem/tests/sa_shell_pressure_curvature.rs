//! F-633 (ADR 0567): the curvature-scaled area-gradient Shell3 pressure (`Load::shell_pressure_curvature`) on the
//! unstructured sphere octant of ADR 0550, the cylinder quarter O1 of ADR 0461 and a torus patch. The checks of the
//! registration, in its order; every limit is the registered one and every prototype number is that of
//! `f633-scratch/proto/est_*.log`. The octant generator is copied from `sa_shell_pressure_sphere.rs` (ADR 0550 design 2).

use rcfd_core::vec3::{cross, dot, norm, Vec3};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::shell3_curvature::curvature_estimates;
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

fn unstructured(n: usize) -> Mesh {
    mesh(n, 0.2, 0.5)
}

fn structured(n: usize) -> Mesh {
    mesh(n, 0.0, 0.0)
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

#[derive(Clone, Copy)]
enum Loading {
    Curvature,
    Default,
    Angle,
}

fn load(loading: Loading, p: f64) -> Load {
    match loading {
        Loading::Curvature => Load::shell_pressure_curvature("all", p),
        Loading::Default => Load::shell_pressure("all", p),
        Loading::Angle => Load::shell_pressure_angle_weighted("all", p),
    }
}

/// The supports of the octant (ADR 0550 design 1): on `x_a = 0` the components of that plane.
fn octant_supports(nodes: &[Vec3]) -> Vec<Prescribed> {
    let mut pres = Vec::new();
    for (q, x) in nodes.iter().enumerate() {
        for (axis, comps) in [(0, [0, 4, 5]), (1, [1, 3, 5]), (2, [2, 3, 4])] {
            if x[axis].abs() < 1e-12 {
                for comp in comps {
                    if !pres.iter().any(|p: &Prescribed| p.node == q && p.component == comp) {
                        pres.push(Prescribed::new(q, comp, 0.0));
                    }
                }
            }
        }
    }
    pres
}

/// The octant model of ADR 0550 design 1 on `m`, thickness `t`; returns `u_r / ref` at every node.
fn octant(m: &Mesh, loading: Loading, t: f64) -> Vec<f64> {
    let mut model = shell3_model(&m.nodes, &m.tris, t);
    model.add_load(load(loading, P)).unwrap();
    let r = solve_static_with(&model, &octant_supports(&m.nodes), &[], &StaticOptions::default()).unwrap();
    let reference = P * R * R * (1.0 - NU) / (2.0 * E * t);
    m.nodes
        .iter()
        .enumerate()
        .map(|(q, x)| {
            let u = r.displacement(q);
            (u[0] * x[0] + u[1] * x[1] + u[2] * x[2]) / R / reference
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq)]
enum Pattern {
    One,
    Other,
    Checker,
}

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

/// The cylinder model of ADR 0461 (t = 0.01, the symmetry conditions of its test, z = H free); `u_r / (p R^2 / (E t))`.
fn cylinder(cy: &Cylinder, loading: Loading) -> Vec<f64> {
    let t = 0.01;
    let mut model = shell3_model(&cy.nodes, &cy.tris, t);
    model.add_load(load(loading, P)).unwrap();
    let idx = |i: usize, j: usize| (cy.nz + 1) * i + j;
    let mut pres = Vec::new();
    for j in 0..=cy.nz {
        for comp in [1, 3, 5] {
            pres.push(Prescribed::new(idx(0, j), comp, 0.0));
        }
        for comp in [0, 4, 5] {
            pres.push(Prescribed::new(idx(cy.nq, j), comp, 0.0));
        }
    }
    for i in 0..=cy.nq {
        for comp in [2, 3, 4] {
            pres.push(Prescribed::new(idx(i, 0), comp, 0.0));
        }
    }
    let r = solve_static_with(&model, &pres, &[], &StaticOptions::default()).unwrap();
    let reference = P * R * R / (E * t);
    cy.nodes
        .iter()
        .enumerate()
        .map(|(k, x)| {
            let u = r.displacement(k);
            (u[0] * x[0] + u[1] * x[1]) / R / reference
        })
        .collect()
}

fn range(v: &[f64]) -> (f64, f64) {
    v.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &x| {
        (lo.min(x), hi.max(x))
    })
}

/// `max |v - 1|`.
fn err(v: &[f64]) -> f64 {
    v.iter().map(|&x| (x - 1.0).abs()).fold(0.0, f64::max)
}

/// The nodal forces of the curvature load through `nodal_load_vector` (six unknowns per node).
fn load_vector(nodes: &[Vec3], tris: &[[usize; 3]]) -> Vec<Vec3> {
    let mut model = shell3_model(nodes, tris, T);
    model.add_load(Load::shell_pressure_curvature("all", P)).unwrap();
    let f = nodal_load_vector(&model, &[]).unwrap();
    assert_eq!(f.len(), 6 * nodes.len());
    (0..nodes.len())
        .map(|q| [f[6 * q], f[6 * q + 1], f[6 * q + 2]])
        .collect()
}

/// `max |F - F_ref| / max |F_ref|` over the components, the components along `e_a` removed at the nodes with `x_a = 0`.
fn load_dev(nodes: &[Vec3], f: &[Vec3], f_ref: &[Vec3]) -> f64 {
    let mut big = 0.0f64;
    let mut d = 0.0f64;
    for (q, x) in nodes.iter().enumerate() {
        for a in 0..3 {
            big = big.max(f_ref[q][a].abs());
            if x[a].abs() >= 1e-12 {
                d = d.max((f[q][a] - f_ref[q][a]).abs());
            }
        }
    }
    d / big
}

fn ring_counts(est: &[(usize, rcfd_fem::shell3_curvature::NodalCurvature)]) -> (usize, usize, usize) {
    let c = |k| est.iter().filter(|(_, e)| e.rings == k).count();
    (c(2), c(3), c(4))
}

#[test]
fn curvature_pressure_sphere_and_cylinder() {
    let mut c = Checks::new();
    let m16 = unstructured(16);
    let m32 = unstructured(32);
    let m64 = unstructured(64);
    let proto = [(16, 9.068e-13), (32, 3.057e-12)];

    // A1, A2, X1
    let mut e_curv = Vec::new();
    for (m, n) in [(&m16, 16), (&m32, 32), (&m64, 64)] {
        let ur = octant(m, Loading::Curvature, T);
        let (lo, hi) = range(&ur);
        let e = err(&ur);
        let p = proto.iter().find(|q| q.0 == n).map(|q| format!("{:.3e}", q.1));
        println!(
            "octant n = {n} curvature: u_r / ref in [{lo:.12}, {hi:.12}], e = {e:.4e} (prototype {})",
            p.unwrap_or_else(|| "not run".into())
        );
        e_curv.push(e);
    }
    c.at_most("A1 octant n = 16: e", e_curv[0], 2e-2);
    c.at_most("A2 octant n = 32: e", e_curv[1], 2e-2);
    c.at_most("X1 octant n = 16: e", e_curv[0], 1e-8);
    c.at_most("X1 octant n = 32: e", e_curv[1], 1e-8);
    c.at_most("X1 octant n = 64: e", e_curv[2], 1e-8);

    // A3, X2
    let o1 = cylinder_mesh(32, 16, Pattern::One, 0.0);
    let ur = cylinder(&o1, Loading::Curvature);
    let (lo, hi) = range(&ur);
    println!("cylinder O1 curvature: u_r / ref in [{lo:.12}, {hi:.12}] (prototype 7.523e-10)");
    c.at_most("A3 cylinder O1: e", err(&ur), 1e-2);
    for (name, cy, proto) in [
        ("other diagonal", cylinder_mesh(32, 16, Pattern::Other, 0.0), 1.626e-10),
        ("checkerboard", cylinder_mesh(32, 16, Pattern::Checker, 0.0), 7.483e-11),
        ("warped 0.3", cylinder_mesh(32, 16, Pattern::One, 0.3), 1.642e-9),
        ("16 x 8", cylinder_mesh(16, 8, Pattern::One, 0.0), 1.361e-11),
    ] {
        let e = err(&cylinder(&cy, Loading::Curvature));
        println!("cylinder {name}: e = {e:.4e} (prototype {proto:.3e})");
        c.at_most(&format!("X2 cylinder {name}: e"), e, 1e-7);
    }

    // X3, X4
    for (m, n, proto) in [(&m16, 16, 1.824e-12), (&m32, 32, 1.604e-11)] {
        let e = err(&octant(m, Loading::Curvature, 0.001));
        println!("octant n = {n}, t = 0.001: e = {e:.4e} (prototype {proto:.3e})");
        c.at_most(&format!("X3 octant n = {n}, t = 0.001: e"), e, 1e-7);
    }
    let s16 = structured(16);
    let e = err(&octant(&s16, Loading::Curvature, T));
    println!("structured octant n = 16: e = {e:.4e} (prototype 6.151e-13)");
    c.at_most("X4 structured octant n = 16: e", e, 1e-8);

    // L1: against (p R / 2) G_i
    let f = load_vector(&m32.nodes, &m32.tris);
    let mut f_ag = vec![[0.0; 3]; m32.nodes.len()];
    for tri in &m32.tris {
        let x = tri.map(|q| m32.nodes[q]);
        let nv = cross(sub(x[1], x[0]), sub(x[2], x[0]));
        let l = norm(nv);
        let ne = [nv[0] / l, nv[1] / l, nv[2] / l];
        for i in 0..3 {
            let (j, k) = ((i + 1) % 3, (i + 2) % 3);
            let g = cross(ne, sub(x[k], x[j]));
            for a in 0..3 {
                f_ag[tri[i]][a] += 0.5 * P * R * 0.5 * g[a];
            }
        }
    }
    c.at_most(
        "L1 octant n = 32: load against the area-gradient load",
        load_dev(&m32.nodes, &f, &f_ag),
        1e-10,
    );
    // L2: against the polygon's tributary radial load
    let f = load_vector(&o1.nodes, &o1.tris);
    let dth = PI / 2.0 / o1.nq as f64;
    let dz = 0.5 / o1.nz as f64;
    let mut f_poly = Vec::new();
    for i in 0..=o1.nq {
        for j in 0..=o1.nz {
            let x = o1.nodes[(o1.nz + 1) * i + j];
            let wi = if i == 0 || i == o1.nq { 0.5 } else { 1.0 };
            let wj = if j == 0 || j == o1.nz { 0.5 } else { 1.0 };
            let s = P * 2.0 * R * (dth / 2.0).sin() * dz * wi * wj / R;
            f_poly.push([s * x[0], s * x[1], 0.0]);
        }
    }
    c.at_most(
        "L2 cylinder O1: load against the polygon's tributary radial load",
        load_dev(&o1.nodes, &f, &f_poly),
        1e-10,
    );

    // K1, K2
    let est = curvature_estimates(&m32.nodes, &m32.tris).unwrap();
    let dk = est.iter().map(|(_, e)| (e.kappa - 1.0 / R).abs()).fold(0.0, f64::max);
    let dn = est
        .iter()
        .map(|(q, e)| {
            let x = m32.nodes[*q];
            (0..3).map(|a| (e.normal[a] - x[a] / R).abs()).fold(0.0, f64::max)
        })
        .fold(0.0, f64::max);
    c.at_most("K1 octant n = 32: max |kappa - 1/R|", dk, 1e-10);
    c.at_most("K1 octant n = 32: max |n - x/R|", dn, 1e-10);
    let rc = ring_counts(&est);
    c.holds(
        "K2 octant n = 32: nodes with 2, 3, 4 rings",
        rc == (361, 104, 96),
        &format!("{rc:?} (prototype (361, 104, 96))"),
    );
    let est = curvature_estimates(&o1.nodes, &o1.tris).unwrap();
    let dk = est
        .iter()
        .map(|(_, e)| (2.0 * R * e.kappa - 1.0).abs())
        .fold(0.0, f64::max);
    let dn = est
        .iter()
        .map(|(q, e)| {
            let x = o1.nodes[*q];
            let er = [x[0] / R, x[1] / R, 0.0];
            (0..3).map(|a| (e.normal[a] - er[a]).abs()).fold(0.0, f64::max)
        })
        .fold(0.0, f64::max);
    c.at_most("K1 cylinder O1: max |2 R kappa - 1|", dk, 1e-10);
    c.at_most("K1 cylinder O1: max |n - e_r|", dn, 1e-10);
    let rc = ring_counts(&est);
    c.holds(
        "K2 cylinder O1: nodes with 2, 3, 4 rings",
        rc == (377, 88, 96),
        &format!("{rc:?} (prototype (377, 88, 96))"),
    );

    // S1
    let mut mp = shell3_model(&m16.nodes, &m16.tris, T);
    mp.add_load(Load::shell_pressure_curvature("all", P)).unwrap();
    let mut mn = shell3_model(&m16.nodes, &m16.tris, T);
    mn.add_load(Load::shell_pressure_curvature("all", -P)).unwrap();
    let fp = nodal_load_vector(&mp, &[]).unwrap();
    let fm = nodal_load_vector(&mn, &[]).unwrap();
    let same = fp.len() == fm.len() && fp.iter().zip(&fm).all(|(a, b)| (-a).to_bits() == b.to_bits());
    c.holds(
        "S1 octant n = 16: the load of -p is the load of p negated, bit for bit",
        same,
        &format!("{same}"),
    );

    // NC1 to NC3
    for (m, n, proto) in [(&m16, 16, 7.8775e-1), (&m32, 32, 2.2612e-1)] {
        let e = err(&octant(m, Loading::Default, T));
        println!("octant n = {n} default: e = {e:.4e} (ADR 0550 {proto:.4e})");
        c.above(&format!("NC1 octant n = {n} default: e"), e, 2e-2);
    }
    let e = err(&octant(&m32, Loading::Angle, T));
    println!("octant n = 32 angle: e = {e:.4e} (ADR 0550 1.4014e-1)");
    c.above("NC2 octant n = 32 angle-weighted: e", e, 2e-2);
    let e = err(&cylinder(&o1, Loading::Default));
    println!("cylinder O1 default: e = {e:.4e} (ADR 0461 9.5e-2)");
    c.above("NC3 cylinder O1 default: e", e, 1e-2);

    c.finish();
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

/// `(max over the 2-ring nodes, max over all nodes)` of `|kappa / kappa_exact - 1|` on the torus patch.
fn torus_errors(n: usize, jitter: f64) -> (f64, f64) {
    let (r0, r) = (1.0, 0.4);
    let (nodes, tris, phis) = torus(n, jitter);
    let est = curvature_estimates(&nodes, &tris).unwrap();
    let (mut inner, mut all) = (0.0f64, 0.0f64);
    for (q, e) in &est {
        let ph = phis[*q];
        let exact = (r0 + 2.0 * r * ph.cos()) / (2.0 * r * (r0 + r * ph.cos()));
        let d = (e.kappa / exact - 1.0).abs();
        all = all.max(d);
        if e.rings == 2 {
            inner = inner.max(d);
        }
    }
    (inner, all)
}

#[test]
fn curvature_estimates_on_a_torus_patch() {
    let mut c = Checks::new();
    let (i32s, a32s) = torus_errors(32, 0.0);
    let (i64s, a64s) = torus_errors(64, 0.0);
    println!("torus structured N = 32: 2-ring {i32s:.4e} (prototype 1.2572e-3), all {a32s:.4e} (1.3573e-1)");
    println!("torus structured N = 64: 2-ring {i64s:.4e} (prototype 4.3065e-4), all {a64s:.4e} (7.8172e-2)");
    c.at_most("T1 torus structured N = 32: 2-ring nodes", i32s, 3e-3);
    c.at_most("T1 torus structured N = 32: all nodes", a32s, 2e-1);
    c.at_least(
        "T1 torus structured: 2-ring order log2(err(32) / err(64))",
        (i32s / i64s).log2(),
        1.5,
    );
    let (i32j, a32j) = torus_errors(32, 0.2);
    println!("torus jittered N = 32: 2-ring {i32j:.4e} (prototype 1.0810e-2), all {a32j:.4e} (1.3742e-1)");
    c.at_most("T2 torus jittered N = 32: 2-ring nodes", i32j, 2e-2);
    c.at_most("T2 torus jittered N = 32: all nodes", a32j, 2e-1);
    c.finish();
}

#[test]
fn curvature_pressure_refusals() {
    let mut c = Checks::new();
    // a Shell4 element in the set
    let nodes = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]];
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, vec![0, 1, 2, 3]).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", vec![0]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    let r = m.add_load(Load::shell_pressure_curvature("all", 1.0));
    c.holds(
        "RF1 a Shell4 element refused",
        matches!(
            &r,
            Err(FemError::InvalidValue {
                key: "shell pressure",
                ..
            })
        ),
        &format!("{r:?}"),
    );
    // a flat 4 x 4 plate
    let k = 4;
    let idx = |i: usize, j: usize| (k + 1) * i + j;
    let mut nodes = Vec::new();
    for i in 0..=k {
        for j in 0..=k {
            nodes.push([i as f64 / k as f64, j as f64 / k as f64, 0.0]);
        }
    }
    let mut tris = Vec::new();
    for i in 0..k {
        for j in 0..k {
            tris.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1)]);
            tris.push([idx(i, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let mut m = shell3_model(&nodes, &tris, 0.01);
    m.add_load(Load::shell_pressure_curvature("all", 1.0)).unwrap();
    let r = nodal_load_vector(&m, &[]);
    c.holds(
        "RF1 a flat plate refused",
        matches!(&r, Err(FemError::InvalidValue { key: "curvature", .. })),
        &format!("{:?}", r.as_ref().err()),
    );
    // the octant n = 8 with one triangle reversed
    let mut m8 = unstructured(8);
    let t = m8.tris.len() / 2;
    m8.tris[t].swap(0, 1);
    let mut m = shell3_model(&m8.nodes, &m8.tris, 0.01);
    m.add_load(Load::shell_pressure_curvature("all", 1.0)).unwrap();
    let r = nodal_load_vector(&m, &[]);
    c.holds(
        "RF1 a reversed triangle refused",
        matches!(&r, Err(FemError::InvalidValue { key: "orientation", .. })),
        &format!("{:?}", r.as_ref().err()),
    );
    // an unknown set; a non-finite pressure
    let m8 = unstructured(8);
    let mut m = shell3_model(&m8.nodes, &m8.tris, 0.01);
    let r = m.add_load(Load::shell_pressure_curvature("nowhere", 1.0));
    c.holds(
        "RF1 an unknown set refused",
        matches!(&r, Err(FemError::UnknownSet { .. })),
        &format!("{r:?}"),
    );
    let r = m.add_load(Load::shell_pressure_curvature("all", f64::NAN));
    c.holds("RF1 a non-finite pressure refused", r.is_err(), &format!("{r:?}"));
    let r = m.add_load(Load::shell_pressure_curvature("all", 1.0));
    c.holds("RF1 the octant n = 8 taken", r.is_ok(), &format!("{r:?}"));
    c.holds(
        "RF1 the octant n = 8 load built",
        nodal_load_vector(&m, &[]).is_ok(),
        "nodal_load_vector",
    );
    c.finish();
}
