//! F-546 (ADR 0550): the angle-weighted shell pressure (`Load::shell_pressure_angle_weighted`) on an unstructured
//! triangulation of a sphere octant under internal pressure. The checks of the registration, in its order; every limit is the
//! registered one and every prototype number is that of `f546-scratch/est2.log`, `est3_n64.log` and `mesh.log`.

use rcfd_core::vec3::{cross, dot, norm, Vec3};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::solve::{solve_static_with, StaticOptions};
use rcfd_fem::{BlockProperty, ElementKind, Load, Material, SolidMesh, SolidModel};
use std::collections::HashMap;

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

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

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

#[derive(Clone, Copy)]
enum Loading {
    Angle,
    Default,
    AreaGradient,
}

/// The octant model of ADR 0550 design 1 on `m`; returns `u_r / ref` at every node.
fn octant(m: &Mesh, loading: Loading) -> Vec<f64> {
    let nn = m.nodes.len();
    let mut mesh = SolidMesh::new(m.nodes.clone()).unwrap();
    mesh.add_block(ElementKind::Shell3, m.tris.iter().flatten().copied().collect())
        .unwrap();
    mesh.set_block_property(0, BlockProperty::shell(T).unwrap()).unwrap();
    mesh.add_element_set("all", (0..m.tris.len()).collect()).unwrap();
    mesh.add_node_set("all", (0..nn).collect()).unwrap();
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(E, NU, 7850.0, 0.0).unwrap())
        .unwrap();
    let mut forces = Vec::new();
    match loading {
        Loading::Angle => model.add_load(Load::shell_pressure_angle_weighted("all", P)).unwrap(),
        Loading::Default => model.add_load(Load::shell_pressure("all", P)).unwrap(),
        Loading::AreaGradient => {
            let mut f = vec![[0.0; 3]; nn];
            for tri in &m.tris {
                let x = tri.map(|q| m.nodes[q]);
                let nv = cross(sub(x[1], x[0]), sub(x[2], x[0]));
                let l = norm(nv);
                let ne = [nv[0] / l, nv[1] / l, nv[2] / l];
                for i in 0..3 {
                    let (j, k) = ((i + 1) % 3, (i + 2) % 3);
                    let g = cross(ne, sub(x[k], x[j]));
                    for c in 0..3 {
                        f[tri[i]][c] += 0.5 * P * R * 0.5 * g[c];
                    }
                }
            }
            forces = f
                .into_iter()
                .enumerate()
                .map(|(q, v)| NodalForceAt::new(q, v))
                .collect();
        }
    }
    let mut pres = Vec::new();
    for (q, x) in m.nodes.iter().enumerate() {
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
    let r = solve_static_with(&model, &pres, &forces, &StaticOptions::default()).unwrap();
    let reference = P * R * R * (1.0 - NU) / (2.0 * E * T);
    m.nodes
        .iter()
        .enumerate()
        .map(|(q, x)| {
            let u = r.displacement(q);
            (u[0] * x[0] + u[1] * x[1] + u[2] * x[2]) / R / reference
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

/// G1 for one level: counts against the prototype, smallest angle, symmetry-edge nodes on their planes, outward triangles.
fn g1(c: &mut Checks, n: usize, expect: (usize, usize, usize)) -> Mesh {
    let m = unstructured(n);
    let got = (m.nodes.len(), m.tris.len(), m.flips);
    c.holds(
        &format!("G1 n = {n}: nodes, triangles, flips"),
        got == expect,
        &format!("{got:?} (prototype {expect:?})"),
    );
    let ma = m
        .tris
        .iter()
        .map(|t| min_angle(t.map(|q| m.nodes[q])))
        .fold(f64::INFINITY, f64::min);
    c.at_least(&format!("G1 n = {n}: smallest angle (deg)"), ma, 20.0);
    // a node of a symmetry edge is one whose grid coordinate is zero; it lies on the plane when that coordinate is below 1e-15
    let mut off = 0.0f64;
    let mut q = 0;
    for i in 0..=n {
        for j in 0..=(n - i) {
            let k = n - i - j;
            let x = m.nodes[q];
            for (zero, comp) in [(i == 0, 0), (j == 0, 1), (k == 0, 2)] {
                if zero {
                    off = off.max(x[comp].abs());
                }
            }
            q += 1;
        }
    }
    c.at_most(&format!("G1 n = {n}: symmetry-edge nodes off their plane"), off, 1e-15);
    let all_out = m.tris.iter().all(|t| outward(t.map(|q| m.nodes[q])));
    c.holds(
        &format!("G1 n = {n}: every triangle outward"),
        all_out,
        &format!("{all_out}"),
    );
    m
}

#[test]
fn sphere_octant_unstructured() {
    let mut c = Checks::new();

    let m16 = g1(&mut c, 16, (153, 256, 72));
    let m32 = g1(&mut c, 32, (561, 1024, 286));
    let m64 = g1(&mut c, 64, (2145, 4096, 1185));
    let _ = g1(&mut c, 128, (8385, 16384, 4764));

    // G2 at n = 64
    let mut val = vec![0usize; m64.nodes.len()];
    for t in &m64.tris {
        for &q in t {
            val[q] += 1;
        }
    }
    let mut distinct: Vec<usize> = m64
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, x)| x.iter().all(|v| v.abs() > 1e-12))
        .map(|(q, _)| val[q])
        .collect();
    distinct.sort_unstable();
    distinct.dedup();
    println!("G2 n = 64 interior valences: {distinct:?}");
    c.at_least("G2 n = 64: distinct interior valences", distinct.len() as f64, 5.0);
    c.at_least(
        "G2 n = 64: share of triangles flipped",
        2.0 * m64.flips as f64 / m64.tris.len() as f64,
        0.25,
    );

    // P1, P2 against the prototype; R1 at 32, 64; NC1; A2 first order
    let proto_angle = [
        (16, 0.7975032605, 1.5564788140),
        (32, 0.8598630675, 1.1257115733),
        (64, 0.9666577934, 1.0303006206),
    ];
    let proto_default = [
        (16, 0.6349865117, 1.7877475664),
        (32, 0.7738809759, 1.2075570046),
        (64, 0.9465929416, 1.0464244510),
    ];
    let meshes = [&m16, &m32, &m64];
    let mut e_angle = Vec::new();
    let mut e_default = Vec::new();
    for (k, m) in meshes.iter().enumerate() {
        let ur = octant(m, Loading::Angle);
        let (lo, hi) = range(&ur);
        let (n, plo, phi) = proto_angle[k];
        println!("n = {n} angle: u_r / ref in [{lo:.10}, {hi:.10}], e = {:.4e}", err(&ur));
        c.at_most(
            &format!("P1 n = {n} angle min against the prototype"),
            rel(lo, plo),
            1e-5,
        );
        c.at_most(
            &format!("P1 n = {n} angle max against the prototype"),
            rel(hi, phi),
            1e-5,
        );
        e_angle.push(err(&ur));
        let ur = octant(m, Loading::Default);
        let (lo, hi) = range(&ur);
        let (n, plo, phi) = proto_default[k];
        println!(
            "n = {n} default: u_r / ref in [{lo:.10}, {hi:.10}], e = {:.4e}",
            err(&ur)
        );
        c.at_most(
            &format!("P2 n = {n} default min against the prototype"),
            rel(lo, plo),
            1e-5,
        );
        c.at_most(
            &format!("P2 n = {n} default max against the prototype"),
            rel(hi, phi),
            1e-5,
        );
        e_default.push(err(&ur));
    }
    println!("A2 e(32) = {:.4e}, e(64) = {:.4e}", e_angle[1], e_angle[2]);
    c.at_least(
        "A2 observed order log2(e(32) / e(64))",
        (e_angle[1] / e_angle[2]).log2(),
        1.5,
    );
    c.at_most("R1 n = 32: e / e_default", e_angle[1] / e_default[1], 0.8);
    c.at_most("R1 n = 64: e / e_default", e_angle[2] / e_default[2], 0.8);
    c.above("NC1 n = 64: e(64)", e_angle[2], 2e-2);

    // P3, S1, NC2 on the structured grid
    let s16 = structured(16);
    let ur = octant(&s16, Loading::Angle);
    let (lo, hi) = range(&ur);
    println!("structured n = 16 angle: u_r / ref in [{lo:.10}, {hi:.10}]");
    c.at_most(
        "P3 n = 16 structured min against the prototype",
        rel(lo, 0.9891279349),
        1e-5,
    );
    c.at_most(
        "P3 n = 16 structured max against the prototype",
        rel(hi, 1.0061449536),
        1e-5,
    );
    c.at_most("S1 structured n = 16 angle: e", err(&ur), 2e-2);
    let ur = octant(&structured(32), Loading::Angle);
    let (lo, hi) = range(&ur);
    println!("structured n = 32 angle: u_r / ref in [{lo:.10}, {hi:.10}]");
    c.at_most(
        "P3 n = 32 structured min against the prototype",
        rel(lo, 0.9970766504),
        1e-5,
    );
    c.at_most(
        "P3 n = 32 structured max against the prototype",
        rel(hi, 1.0016982460),
        1e-5,
    );
    let ur = octant(&s16, Loading::Default);
    c.above("NC2 structured n = 16 default: e", err(&ur), 5e-2);

    // D1 the area-gradient load
    let ur = octant(&m32, Loading::AreaGradient);
    c.at_most("D1 n = 32 area-gradient load: max |u_r / ref - 1|", err(&ur), 1e-8);

    c.finish();
}

/// The heavy record (ADR 0550 design 4): n = 128, 50310 unknowns, run once with `--ignored`.
#[test]
#[ignore]
fn sphere_octant_unstructured_n128_heavy() {
    let mut c = Checks::new();
    let e64 = err(&octant(&unstructured(64), Loading::Angle));
    let m = unstructured(128);
    let ur = octant(&m, Loading::Angle);
    let (lo, hi) = range(&ur);
    let e128 = err(&ur);
    println!("n = 128 angle: u_r / ref in [{lo:.10}, {hi:.10}]");
    c.at_most("A1 e(128)", e128, 2e-2);
    println!("A2 e(64) = {e64:.4e}, e(128) = {e128:.4e}");
    c.at_least("A2 observed order log2(e(64) / e(128))", (e64 / e128).log2(), 1.5);
    let ur = octant(&m, Loading::Default);
    let (lo, hi) = range(&ur);
    let ed = err(&ur);
    println!("O1 n = 128 default: u_r / ref in [{lo:.10}, {hi:.10}], e = {ed:.4e} (observation, no limit)");
    c.at_most("R1 n = 128: e / e_default", e128 / ed, 0.8);
    c.finish();
}
