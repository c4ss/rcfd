//! F-601 (ADR 0540): the models shared by its test files. Copies of the set-ups of `tests/sa_contact.rs` (ADR 0507 D7: B1's
//! blocks, T1's warped pair, T2's punch), `tests/sa_contact_hub_f604.rs` (ADR 0517 D3: the hub) and the state generator of
//! ADR 0540 D5.
#![allow(dead_code)]

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::contact::ContactNode;
use rcfd_fem::contact::ContactResult;
use rcfd_fem::tie::assemble_parts;
use rcfd_fem::{Constraint, ElementKind, FaceRef, Load, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

// ------------------------------------------------------------------------------------------------------------ checks

pub struct Checks(pub Vec<String>);

impl Checks {
    pub fn new() -> Checks {
        Checks(Vec::new())
    }
    pub fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    pub fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
        }
    }
    pub fn less(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value < limit;
        println!(
            "{name}: {value:.6e} (limit < {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} >= {limit:e}"));
        }
    }
    pub fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    pub fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

/// The largest of the values; infinite if any is not finite (a NaN must not hide behind `f64::max`).
pub fn worst(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut w: f64 = 0.0;
    for v in values {
        if !v.is_finite() {
            return f64::INFINITY;
        }
        w = w.max(v);
    }
    w
}

pub fn elastic(e: f64, nu: f64) -> Material {
    Material::isotropic_elastic(e, nu, 1.0, 0.0).unwrap()
}

// ------------------------------------------------------------------------------------------------------------ bits

/// FNV-1a 64 over the little-endian bytes of the words.
pub fn digest(v: &[u64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in v.iter().flat_map(|x| x.to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// Every field of a contactor node as words (floats by their bits).
pub fn node_words(n: &ContactNode, out: &mut Vec<u64>) {
    out.push(n.pair as u64);
    out.push(n.node as u64);
    out.push(n.area.to_bits());
    match n.gap {
        Some(g) => {
            out.push(1);
            out.push(g.to_bits());
        }
        None => out.push(0),
    }
    out.push(n.force.to_bits());
    out.push(n.pressure.to_bits());
    out.push(n.active as u64);
    match n.face {
        Some(f) => {
            out.push(1);
            out.push(f.element as u64);
            out.push(f.face as u64);
        }
        None => out.push(0),
    }
    out.extend(n.local.iter().map(|v| v.to_bits()));
    out.extend(n.normal.iter().map(|v| v.to_bits()));
}

/// Every value of a contact result as words: displacement, reactions, the nodes at the end and of every step, every Newton
/// residual, energy, penetration, convex count and active set.
pub fn result_words(r: &ContactResult) -> Vec<u64> {
    let mut v: Vec<u64> = r.result.displacement_values().iter().map(|x| x.to_bits()).collect();
    if let Some(re) = r.result.reactions() {
        v.extend(re.iter().map(|x| x.to_bits()));
    }
    for n in &r.nodes {
        node_words(n, &mut v);
    }
    for s in &r.steps {
        v.push(s.step as u64);
        v.push(s.load_factor.to_bits());
        v.extend(s.displacement.iter().map(|x| x.to_bits()));
        v.extend(s.reactions.iter().map(|x| x.to_bits()));
        for n in &s.nodes {
            node_words(n, &mut v);
        }
        for x in &s.solves {
            v.extend(x.residuals.iter().map(|y| y.to_bits()));
            v.extend(x.energies.iter().map(|y| y.to_bits()));
            v.push(x.penetration.to_bits());
            v.push(x.convex_iterations as u64);
            for a in &x.active_sets {
                v.push(a.len() as u64);
                v.extend(a.iter().map(|&i| i as u64));
            }
        }
    }
    v
}

/// Whether every float of a contact result is finite (gaps of nodes without a face excepted).
pub fn result_finite(r: &ContactResult) -> bool {
    let fin = |s: &[f64]| s.iter().all(|x| x.is_finite());
    let nodes_ok = |ns: &[ContactNode]| {
        ns.iter().all(|n| {
            n.area.is_finite()
                && n.force.is_finite()
                && n.pressure.is_finite()
                && n.gap.map_or(true, |g| g.is_finite())
                && fin(&n.local)
                && fin(&n.normal)
        })
    };
    fin(r.result.displacement_values())
        && r.result.reactions().map_or(true, fin)
        && nodes_ok(&r.nodes)
        && r.steps.iter().all(|s| {
            nodes_ok(&s.nodes)
                && s.solves
                    .iter()
                    .all(|x| fin(&x.residuals) && fin(&x.energies) && x.penetration.is_finite())
        })
}

// ------------------------------------------------------------------------------------------------------------ states

/// ADR 0540 D5: the state `u_i = amplitude s_i`, `s_i` uniform in `[-1, 1]` from a linear congruential generator.
pub fn state(n: usize, amplitude: f64, seed: u64) -> Vec<f64> {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1);
    (0..n)
        .map(|_| {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let s = (x >> 11) as f64 / (1u64 << 53) as f64;
            amplitude * (2.0 * s - 1.0)
        })
        .collect()
}

// ------------------------------------------------------------------------------------------------------------ ADR 0507 models

pub const E_BL: f64 = 2.0e11;

/// ADR 0507 D7 B1: two `[2, 2, 2]` blocks, the upper lowered by `drop`.
pub fn blocks(drop: f64) -> SolidModel {
    let lower = hex_box([0.0; 3], [1.0, 1.0, 1.0], [2, 2, 2]).unwrap();
    let upper = hex_box([0.0, 0.0, 1.0 - drop], [1.0, 1.0, 1.0], [2, 2, 2]).unwrap();
    let mesh = assemble_parts(&[("lower", &lower), ("upper", &upper)]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("lower.all", elastic(E_BL, 0.3)).unwrap();
    m.add_material("upper.all", elastic(E_BL, 0.3)).unwrap();
    m.add_constraint(Constraint::displacement("lower.zmin", 2, 0.0))
        .unwrap();
    for p in ["lower", "upper"] {
        m.add_constraint(Constraint::displacement(&format!("{p}.xmin"), 0, 0.0))
            .unwrap();
        m.add_constraint(Constraint::displacement(&format!("{p}.ymin"), 1, 0.0))
            .unwrap();
    }
    m
}

/// ADR 0507 D7 T2: the punch on the clamped plate (the punch's top pressed by `p`).
pub fn punch_model(p: f64) -> SolidModel {
    let plate = hex_box([0.0; 3], [2.0, 0.5, 0.25], [8, 2, 2]).unwrap();
    let punch = hex_box([0.8, 0.0, 0.25], [0.4, 0.2, 0.2], [3, 1, 1]).unwrap();
    let mesh = assemble_parts(&[("plate", &plate), ("punch", &punch)]).unwrap();
    punch_constraints(mesh, p)
}

pub fn punch_constraints(mesh: SolidMesh, p: f64) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("plate.all", elastic(1.0, 0.3)).unwrap();
    m.add_material("punch.all", elastic(1.0, 0.3)).unwrap();
    for s in ["plate.xmin", "plate.xmax"] {
        for cc in 0..3 {
            m.add_constraint(Constraint::displacement(s, cc, 0.0)).unwrap();
        }
    }
    m.add_constraint(Constraint::displacement("plate.ymin", 1, 0.0))
        .unwrap();
    m.add_constraint(Constraint::displacement("punch.ymin", 1, 0.0))
        .unwrap();
    m.add_constraint(Constraint::displacement("punch.zmax", 0, 0.0))
        .unwrap();
    m.add_load(Load::pressure("punch.zmax", p)).unwrap();
    m
}

pub const T1_CONTACTOR: [Vec3; 4] = [[0.25, 0.3, 0.98], [0.8, 0.2, 0.99], [0.75, 0.7, 0.97], [0.2, 0.75, 1.0]];

/// ADR 0507 D7 T1: the warped Hex8 target (element 0, face 1, set "target") and a Hex8 contactor above (element 1, face 0,
/// set "contactor").
pub fn two_hex(contactor: [Vec3; 4]) -> SolidModel {
    let mut nodes: Vec<Vec3> = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.05],
        [1.0, 0.0, 0.96],
        [1.0, 1.0, 1.06],
        [0.0, 1.0, 0.97],
    ];
    nodes.extend_from_slice(&contactor);
    nodes.extend(contactor.iter().map(|p| [p[0], p[1], p[2] + 1.0]));
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, (0..16).collect()).unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    mesh.add_surface_set("target", vec![FaceRef { element: 0, face: 1 }])
        .unwrap();
    mesh.add_surface_set("contactor", vec![FaceRef { element: 1, face: 0 }])
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", elastic(1.0, 0.3)).unwrap();
    m
}

// ------------------------------------------------------------------------------------------------------------ ADR 0517 hub

pub const E_HUB: f64 = 2.0e11;
pub const A_HUB: f64 = 1.0;
pub const B_HUB: f64 = 2.0;
const NTH: usize = 15;
const NC: usize = 8;
const CORE: f64 = 0.4 * A_HUB;
const SHIFT: f64 = 0.05;
const D_HUB: f64 = 1e-3 * A_HUB;

/// ADR 0517 D3's axial positions from a face (0) to `len`.
fn axial(len: f64, h: f64) -> Vec<f64> {
    let (a, b) = (A_HUB, B_HUB);
    let w = 0.5 * (b - a);
    let hc = 0.25 * (b - a);
    let rest = len - 2.0 * w;
    let nrest = (rest / hc).round() as usize;
    assert!(
        rest >= -1e-12 && (nrest as f64 * hc - rest).abs() < 1e-12,
        "length {len}"
    );
    let nz = (w / h).round() as usize;
    let mut x: Vec<f64> = (0..=nz).map(|i| w * i as f64 / nz as f64).collect();
    let m = (1.0 / (h / (b - a) + 0.25)).round() as usize;
    let sizes: Vec<f64> = (0..m).map(|i| h + (hc - h) * (i as f64 + 0.5) / m as f64).collect();
    let sum: f64 = sizes.iter().sum();
    let mut z = w;
    for (i, sz) in sizes.iter().enumerate() {
        z += sz * w / sum;
        x.push(if i == m - 1 { 2.0 * w } else { z });
    }
    for i in 1..=nrest {
        x.push(2.0 * w + i as f64 * hc);
    }
    x
}

/// ADR 0517 D3's hub on its shaft (BASE: `L = 4 (b - a)`, `e = 2 a`, `nu = 0.3`, `d = 1e-3 a`) with `n` cells through `b - a`;
/// surface sets "bore" (contactor) and "shaft_out" (target).
pub fn hub_model(n: usize) -> SolidModel {
    let (a, b) = (A_HUB, B_HUB);
    let (l, e_ext, nu, d) = (4.0 * (b - a), 2.0 * a, 0.3, D_HUB);
    let h = (b - a) / n as f64;
    let half = 0.5 * l;
    let mut zh: Vec<f64> = axial(half, h).iter().rev().map(|v| half - v).collect();
    zh[0] = 0.0;
    let last = zh.len() - 1;
    zh[last] = half;
    let mut zm = zh.clone();
    zm.extend(axial(e_ext, h).iter().skip(1).map(|v| half + v));
    let nzm = zm.len();
    zm[nzm - 1] = half + e_ext;
    let zs: Vec<f64> = (0..nzm)
        .map(|k| {
            if k == 0 || k == nzm - 1 {
                zm[k]
            } else {
                zm[k] + SHIFT * (zm[k + 1] - zm[k])
            }
        })
        .collect();
    let dth = 0.5 * PI / NTH as f64;
    let mut sp: Vec<[f64; 2]> = Vec::new();
    let core = |i: usize, j: usize| i + (NC + 1) * j;
    for j in 0..=NC {
        for i in 0..=NC {
            sp.push([CORE * i as f64 / NC as f64, CORE * j as f64 / NC as f64]);
        }
    }
    let nper = NTH + 2;
    let cb: Vec<usize> = (0..nper)
        .map(|k| if k <= NC { core(NC, k) } else { core(2 * NC - k, NC) })
        .collect();
    let per: Vec<[f64; 2]> = (0..nper)
        .map(|k| {
            if k == 0 {
                [a, 0.0]
            } else if k == nper - 1 {
                [0.0, a]
            } else {
                let th = (k as f64 - 0.5) * dth;
                let rv = a / (0.5 * dth).cos();
                [rv * th.cos(), rv * th.sin()]
            }
        })
        .collect();
    let nb = ((0.6 * n as f64).ceil() as usize).max(3);
    let mut band = vec![cb.clone()];
    for li in 1..=nb {
        let t = li as f64 / nb as f64;
        let mut row = Vec::new();
        for k in 0..nper {
            let c = sp[cb[k]];
            let mut p = [(1.0 - t) * c[0] + t * per[k][0], (1.0 - t) * c[1] + t * per[k][1]];
            if k == 0 {
                p[1] = 0.0;
            }
            if k == nper - 1 {
                p[0] = 0.0;
            }
            sp.push(p);
            row.push(sp.len() - 1);
        }
        band.push(row);
    }
    let ns2 = sp.len();
    let mut hp: Vec<[f64; 2]> = Vec::new();
    for k in 0..=n {
        let r = (a - d) + k as f64 * (b - a + d) / n as f64;
        for j in 0..=NTH {
            let th = j as f64 * dth;
            hp.push(if j == 0 {
                [r, 0.0]
            } else if j == NTH {
                [0.0, r]
            } else {
                [r * th.cos(), r * th.sin()]
            });
        }
    }
    let nh2 = hp.len();
    let hub = |k: usize, j: usize| k * (NTH + 1) + j;
    let mut nodes: Vec<Vec3> = Vec::new();
    for &z in &zs {
        nodes.extend(sp.iter().map(|p| [p[0], p[1], z]));
    }
    let hoff = nodes.len();
    for &z in &zh {
        nodes.extend(hp.iter().map(|p| [p[0], p[1], z]));
    }
    let sid = |pl: usize, i: usize| pl * ns2 + i;
    let hid = |pl: usize, i: usize| hoff + pl * nh2 + i;
    let mut conn = Vec::new();
    let (mut shaft_out, mut bore_in) = (Vec::new(), Vec::new());
    let mut e = 0;
    for pl in 0..zs.len() - 1 {
        let hx = |q: [usize; 4]| {
            [
                sid(pl, q[0]),
                sid(pl, q[1]),
                sid(pl, q[2]),
                sid(pl, q[3]),
                sid(pl + 1, q[0]),
                sid(pl + 1, q[1]),
                sid(pl + 1, q[2]),
                sid(pl + 1, q[3]),
            ]
        };
        for j in 0..NC {
            for i in 0..NC {
                conn.extend_from_slice(&hx([core(i, j), core(i + 1, j), core(i + 1, j + 1), core(i, j + 1)]));
                e += 1;
            }
        }
        for li in 0..nb {
            for k in 0..nper - 1 {
                conn.extend_from_slice(&hx([
                    band[li][k],
                    band[li + 1][k],
                    band[li + 1][k + 1],
                    band[li][k + 1],
                ]));
                if li == nb - 1 {
                    shaft_out.push(FaceRef { element: e, face: 3 });
                }
                e += 1;
            }
        }
    }
    for pl in 0..zh.len() - 1 {
        let hx = |q: [usize; 4]| {
            [
                hid(pl, q[0]),
                hid(pl, q[1]),
                hid(pl, q[2]),
                hid(pl, q[3]),
                hid(pl + 1, q[0]),
                hid(pl + 1, q[1]),
                hid(pl + 1, q[2]),
                hid(pl + 1, q[3]),
            ]
        };
        for k in 0..n {
            for j in 0..NTH {
                conn.extend_from_slice(&hx([hub(k, j), hub(k + 1, j), hub(k + 1, j + 1), hub(k, j + 1)]));
                if k == 0 {
                    bore_in.push(FaceRef { element: e, face: 5 });
                }
                e += 1;
            }
        }
    }
    let (mut xs, mut ys, mut zset) = (Vec::new(), Vec::new(), Vec::new());
    for (v, p) in nodes.iter().enumerate() {
        if p[0] == 0.0 {
            xs.push(v);
        }
        if p[1] == 0.0 {
            ys.push(v);
        }
        if p[2] == 0.0 {
            zset.push(v);
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, conn).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_surface_set("bore", bore_in).unwrap();
    mesh.add_surface_set("shaft_out", shaft_out).unwrap();
    mesh.add_node_set("x0", xs).unwrap();
    mesh.add_node_set("y0", ys).unwrap();
    mesh.add_node_set("z0", zset).unwrap();
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(E_HUB, nu, 1.0, 0.0).unwrap())
        .unwrap();
    model.add_constraint(Constraint::displacement("x0", 0, 0.0)).unwrap();
    model.add_constraint(Constraint::displacement("y0", 1, 0.0)).unwrap();
    model.add_constraint(Constraint::displacement("z0", 2, 0.0)).unwrap();
    model
}

/// The bore cell `h = (b - a)/n` and the hub's interference `d`.
pub fn hub_h(n: usize) -> f64 {
    (B_HUB - A_HUB) / n as f64
}

pub fn hub_d() -> f64 {
    D_HUB
}

// ------------------------------------------------------------------------------------------------------------ ADR 0507 I

pub const E_FIT: f64 = 2.0e11;
pub const D_FIT: f64 = 1e-3;
const A_FIT: f64 = 1.0;
const B_FIT: f64 = 2.0;
const T_FIT: f64 = 0.1;
const N_FIT: usize = 64;

/// ADR 0507 D7 I: the ring on the solid shaft (surface sets "ring_in", contactor, and "shaft_out", target).
pub fn ring_fit_model() -> SolidModel {
    let dth = 2.0 * PI / N_FIT as f64;
    let mut pts: Vec<[f64; 2]> = Vec::new();
    let m = 16;
    let core = |i: usize, j: usize| i + (m + 1) * j;
    for j in 0..=m {
        for i in 0..=m {
            pts.push([-0.5 + i as f64 / m as f64, -0.5 + j as f64 / m as f64]);
        }
    }
    let mut per = Vec::new();
    for j in 8..=m {
        per.push(core(m, j));
    }
    for i in (0..m).rev() {
        per.push(core(i, m));
    }
    for j in (0..m).rev() {
        per.push(core(0, j));
    }
    for i in 1..=m {
        per.push(core(i, 0));
    }
    for j in 1..8 {
        per.push(core(m, j));
    }
    assert_eq!(per.len(), N_FIT);
    let av = A_FIT / (dth / 2.0).cos();
    let layers = 6;
    let mut band = vec![per.clone()];
    for l in 1..=layers {
        let s = l as f64 / layers as f64;
        let mut row = Vec::new();
        for (p, &q) in per.iter().enumerate() {
            let th = (p as f64 + 0.5) * dth;
            let c = pts[q];
            pts.push([
                (1.0 - s) * c[0] + s * av * th.cos(),
                (1.0 - s) * c[1] + s * av * th.sin(),
            ]);
            row.push(pts.len() - 1);
        }
        band.push(row);
    }
    let ring0 = pts.len();
    let rk = |k: usize| (A_FIT - D_FIT) + k as f64 * (B_FIT - A_FIT + D_FIT) / 8.0;
    let ring = |j: usize, k: usize| ring0 + (j % N_FIT) + N_FIT * k;
    for k in 0..=8 {
        for j in 0..N_FIT {
            let th = j as f64 * dth;
            pts.push([rk(k) * th.cos(), rk(k) * th.sin()]);
        }
    }
    let n2 = pts.len();
    let mut nodes: Vec<Vec3> = pts.iter().map(|p| [p[0], p[1], 0.0]).collect();
    nodes.extend(pts.iter().map(|p| [p[0], p[1], T_FIT]));
    let hex = |q: [usize; 4]| [q[0], q[1], q[2], q[3], q[0] + n2, q[1] + n2, q[2] + n2, q[3] + n2];
    let mut conn = Vec::new();
    let mut shaft_out = Vec::new();
    let mut ring_in = Vec::new();
    let mut e = 0;
    for j in 0..m {
        for i in 0..m {
            conn.extend_from_slice(&hex([core(i, j), core(i + 1, j), core(i + 1, j + 1), core(i, j + 1)]));
            e += 1;
        }
    }
    for l in 0..layers {
        for p in 0..N_FIT {
            let p1 = (p + 1) % N_FIT;
            conn.extend_from_slice(&hex([band[l][p], band[l + 1][p], band[l + 1][p1], band[l][p1]]));
            if l == layers - 1 {
                shaft_out.push(FaceRef { element: e, face: 3 });
            }
            e += 1;
        }
    }
    for k in 0..8 {
        for j in 0..N_FIT {
            conn.extend_from_slice(&hex([ring(j, k), ring(j, k + 1), ring(j + 1, k + 1), ring(j + 1, k)]));
            if k == 0 {
                ring_in.push(FaceRef { element: e, face: 5 });
            }
            e += 1;
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, conn).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_surface_set("ring_in", ring_in).unwrap();
    mesh.add_surface_set("shaft_out", shaft_out).unwrap();
    mesh.add_node_set("z0", (0..n2).collect()).unwrap();
    let both = |v: usize| vec![v, v + n2];
    mesh.add_node_set("centre", both(core(8, 8))).unwrap();
    mesh.add_node_set("shaft_x", both(core(m, 8))).unwrap();
    mesh.add_node_set("ring_0", both(ring(0, 8))).unwrap();
    let mut r90 = both(ring(16, 8));
    r90.extend(both(ring(48, 8)));
    mesh.add_node_set("ring_90_270", r90).unwrap();
    let mut model = SolidModel::new(mesh);
    model.add_material("all", elastic(E_FIT, 0.3)).unwrap();
    model.add_constraint(Constraint::displacement("z0", 2, 0.0)).unwrap();
    model
        .add_constraint(Constraint::displacement("centre", 0, 0.0))
        .unwrap();
    model
        .add_constraint(Constraint::displacement("centre", 1, 0.0))
        .unwrap();
    model
        .add_constraint(Constraint::displacement("shaft_x", 1, 0.0))
        .unwrap();
    model
        .add_constraint(Constraint::displacement("ring_0", 1, 0.0))
        .unwrap();
    model
        .add_constraint(Constraint::displacement("ring_90_270", 0, 0.0))
        .unwrap();
    model
}

// ------------------------------------------------------------------------------------------------------------ ADR 0507 H

pub const NU_H: f64 = 0.3;
pub const R_H: f64 = 1.0;
pub const A_H: f64 = 0.1;

pub fn e_star() -> f64 {
    1.0 / (1.0 - NU_H * NU_H)
}

/// Boussinesq's displacement for the point force `f` into the surface `z = 0` of the body `z <= 0` (E = 1), ADR 0507
/// arithmetic 3.
pub fn boussinesq(f: f64, x: Vec3) -> Vec3 {
    let zeta = -x[2];
    let r = (x[0] * x[0] + x[1] * x[1]).sqrt();
    let rho = (r * r + zeta * zeta).sqrt();
    let c = f * (1.0 + NU_H) / (2.0 * PI);
    let u_zeta = c * (zeta * zeta / rho.powi(3) + 2.0 * (1.0 - NU_H) / rho);
    let u_r = c * (r * zeta / rho.powi(3) - (1.0 - 2.0 * NU_H) * r / (rho * (rho + zeta)));
    let (cx, cy) = if r > 0.0 { (x[0] / r, x[1] / r) } else { (0.0, 0.0) };
    [u_r * cx, u_r * cy, -u_zeta]
}

/// ADR 0507 D7 H's graded coordinates: `nf` cells of `h`, then `m` cells growing to `l`.
pub fn graded(h: f64, nf: usize, m: usize, l: f64) -> Vec<f64> {
    let xf = h * nf as f64;
    let (mut lo, mut hi) = (1.0000001f64, 3.0f64);
    for _ in 0..200 {
        let q = 0.5 * (lo + hi);
        let s = h * q * (q.powi(m as i32) - 1.0) / (q - 1.0);
        if s > l - xf {
            hi = q
        } else {
            lo = q
        }
    }
    let q = 0.5 * (lo + hi);
    println!("grading factor q = {q:.6}");
    let mut v: Vec<f64> = (0..=nf).map(|i| h * i as f64).collect();
    let mut d = h;
    for _ in 0..m {
        d *= q;
        let last = *v.last().unwrap();
        v.push(last + d);
    }
    *v.last_mut().unwrap() = l;
    v
}

/// ADR 0507 D7 H's quarter block `[0, 5a]^2 x [-5a, 0]` of Hex8 with the surface set "top" (`z = 0`).
pub fn hertz_block(per_a: usize, nf: usize, m: usize) -> SolidMesh {
    let h = A_H / per_a as f64;
    let xs = graded(h, nf, m, 5.0 * A_H);
    let n = xs.len();
    let id = |i: usize, j: usize, k: usize| i + n * (j + n * k);
    let mut nodes = Vec::with_capacity(n * n * n);
    for k in 0..n {
        for j in 0..n {
            for i in 0..n {
                nodes.push([xs[i], xs[j], -xs[k]]);
            }
        }
    }
    let mut conn = Vec::new();
    let mut top = Vec::new();
    for k in 0..n - 1 {
        for j in 0..n - 1 {
            for i in 0..n - 1 {
                conn.extend_from_slice(&[
                    id(i, j, k + 1),
                    id(i + 1, j, k + 1),
                    id(i + 1, j + 1, k + 1),
                    id(i, j + 1, k + 1),
                    id(i, j, k),
                    id(i + 1, j, k),
                    id(i + 1, j + 1, k),
                    id(i, j + 1, k),
                ]);
                if k == 0 {
                    top.push(FaceRef {
                        element: i + (n - 1) * (j + (n - 1) * k),
                        face: 1,
                    });
                }
            }
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, conn).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_surface_set("top", top).unwrap();
    mesh
}

/// The readings of a Hertz solve (ADR 0540 D5 HS).
#[derive(Debug, Clone)]
pub struct Hertz {
    /// `4 sum t |n_z|`.
    pub force: f64,
    pub a_fe: f64,
    /// The centre node's pressure (NaN when no contactor point lies on the axis).
    pub p_centre: f64,
    /// `sqrt(c_0)` of the fit.
    pub p_fit: f64,
    pub axis_count: usize,
    pub penetration: f64,
    pub points: usize,
}

/// The position of a contactor point in the reference configuration (a Gauss point: `sum M_a X_a`).
pub fn point_reference(m: &SolidModel, n: &ContactNode) -> Vec3 {
    match n.point {
        None => m.mesh().nodes()[n.node],
        Some((face, local)) => {
            let fnodes = m.mesh().face_nodes(face);
            let (w, _) = rcfd_fem::loads::face_shape_functions(fnodes.len(), local).unwrap();
            let mut x = [0.0; 3];
            for (&v, &wa) in fnodes.iter().zip(&w) {
                for i in 0..3 {
                    x[i] += wa * m.mesh().nodes()[v][i];
                }
            }
            x
        }
    }
}

/// ADR 0507 D7 H's boundary values on `mesh` (any element kind; by coordinates): `u_x = 0` on `x = 0`, `u_y = 0` on `y =
/// 0`, Boussinesq's field of `F_th` plus `delta` in `z` on the far faces.
pub fn hertz_prescribed(mesh: &SolidMesh) -> Vec<rcfd_fem::constraint::Prescribed> {
    use rcfd_fem::constraint::Prescribed;
    let a = A_H;
    let f_th = 4.0 * e_star() * a.powi(3) / (3.0 * R_H);
    let delta = a * a / R_H;
    let far = 5.0 * a;
    let mut pres = Vec::new();
    for (v, x) in mesh.nodes().iter().enumerate() {
        let (sx, sy) = (x[0] == 0.0, x[1] == 0.0);
        if sx {
            pres.push(Prescribed::new(v, 0, 0.0));
        }
        if sy {
            pres.push(Prescribed::new(v, 1, 0.0));
        }
        if x[0] == far || x[1] == far || x[2] == -far {
            let u = boussinesq(f_th, *x);
            let vals = [u[0], u[1], u[2] + delta];
            for (cc, &val) in vals.iter().enumerate() {
                if (cc == 0 && sx) || (cc == 1 && sy) {
                    continue;
                }
                pres.push(Prescribed::new(v, cc, val));
            }
        }
    }
    pres
}

/// The readings of a solved Hertz model (the fit of ADR 0507 D7 H over the points with `p >= 0.5 max p`).
pub fn hertz_read(model: &SolidModel, r: &ContactResult) -> Hertz {
    let force = 4.0 * r.nodes.iter().map(|v| v.force * v.normal[2].abs()).sum::<f64>();
    let pmax = r.nodes.iter().map(|v| v.pressure).fold(0.0, f64::max);
    let (mut sw, mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let mut p_centre = f64::NAN;
    let mut axis_count = 0;
    for v in &r.nodes {
        let x = point_reference(model, v);
        let r2 = x[0] * x[0] + x[1] * x[1];
        if r2 == 0.0 {
            p_centre = v.pressure;
        }
        if x[1] == 0.0 && v.force > 0.0 {
            axis_count += 1;
        }
        if v.pressure >= 0.5 * pmax {
            let y = v.pressure * v.pressure;
            sw += 1.0;
            sx += r2;
            sy += y;
            sxx += r2 * r2;
            sxy += r2 * y;
        }
    }
    let c1 = (sw * sxy - sx * sy) / (sw * sxx - sx * sx);
    let c0 = (sy - c1 * sx) / sw;
    let s = r.steps.last().unwrap();
    Hertz {
        force,
        a_fe: (-c0 / c1).sqrt(),
        p_centre,
        p_fit: c0.sqrt(),
        axis_count,
        penetration: s.solves.last().unwrap().penetration,
        points: r.nodes.len(),
    }
}

/// `(a_H, p_H, the approach error)` of Hertz's theory at the measured force: `a_H = (3 F R/(4 E*))^(1/3)`, `p_H = 3 F/(2 pi
/// a_H^2)`, `|delta/(a_H^2/R) - 1|` with the prescribed `delta = a^2/R`.
pub fn hertz_theory(force: f64) -> (f64, f64, f64) {
    let a_h = (3.0 * force * R_H / (4.0 * e_star())).cbrt();
    let p_h = 3.0 * force / (2.0 * PI * a_h * a_h);
    let delta = A_H * A_H / R_H;
    (a_h, p_h, (delta / (a_h * a_h / R_H) - 1.0).abs())
}

/// `max |K - K_fd| / max |K|` of a contact set at `u` with every point active and on the same face at every perturbed
/// state (ADR 0507 T1's arrangement, step `1e-6`); also the matrix `K` and the central differences.
pub fn tangent_error(set: &rcfd_fem::contact::ContactSet, u: &[f64]) -> (f64, bool, Vec<f64>, Vec<f64>) {
    use rcfd_fem::nonlinear::Contribution;
    let n = u.len();
    let at = |u: &[f64]| {
        let mut f = vec![0.0; n];
        let mut t = Vec::new();
        set.add(u, 1.0, &mut f, &mut t).unwrap();
        let mut k = vec![0.0; n * n];
        for &(r, c, v) in &t {
            k[n * r + c] += v;
        }
        let st: Vec<(bool, Option<FaceRef>)> = set.evaluate(u).unwrap().iter().map(|v| (v.active, v.face)).collect();
        (f, k, st)
    };
    let (_, k, st0) = at(u);
    let h = 1e-6;
    let mut kfd = vec![0.0; n * n];
    let mut same = st0.iter().all(|s| s.0);
    for j in 0..n {
        let (mut up, mut um) = (u.to_vec(), u.to_vec());
        up[j] += h;
        um[j] -= h;
        let (fp, _, sp) = at(&up);
        let (fm, _, sm) = at(&um);
        same &= sp == st0 && sm == st0;
        for r in 0..n {
            kfd[n * r + j] = (fp[r] - fm[r]) / (2.0 * h);
        }
    }
    let kmax = worst(k.iter().map(|v| v.abs()));
    let err = worst(k.iter().zip(&kfd).map(|(a, b)| (a - b).abs())) / kmax;
    (err, same, k, kfd)
}

/// ADR 0507 T1's state `u_i = 0.01 sin(1.7 i + 0.3)`.
pub fn t1_state(n: usize) -> Vec<f64> {
    (0..n).map(|i| 0.01 * (1.7 * i as f64 + 0.3).sin()).collect()
}
