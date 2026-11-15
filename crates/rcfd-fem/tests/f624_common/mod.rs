//! F-624 (ADR 0566): ADR 0542's hub model, `read`, `flat`, `digest`, the routes and the cantilever with ADR 0542 D5's
//! device, copied from `tests/sa_newton_supernodal_f618.rs` (ADR 0542; its model and `read` are ADR 0517 D3, D4).
#![allow(dead_code, unused_imports)]

use rcfd_core::vec3::Vec3;
use rcfd_fem::arclength::{trace, ArcLengthOptions};
use rcfd_fem::builders::hex_box;
use rcfd_fem::contact::{solve, ContactOptions, ContactPair, ContactResult, Kinematics, Target};
use rcfd_fem::error::FemError;
use rcfd_fem::nonlinear::NewtonOptions;
use rcfd_fem::nonlinear::{
    solve as nl_solve, solve_with as nl_solve_with, Contribution, DriverExtensions, NonlinearResult,
};
use rcfd_fem::solve::DirectFactor;
use rcfd_fem::Load;
use rcfd_fem::{Constraint, ElementKind, FaceRef, Material, SolidMesh, SolidModel};
use std::cell::RefCell;
use std::f64::consts::PI;
use std::time::Instant;

// ------------------------------------------------------------------------------------------------------------ helpers

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
    pub fn holds(&mut self, name: &str, ok: bool) {
        println!("{name}: {}", if ok { "holds" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name} does not hold"));
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

pub fn rel(x: f64, r: f64) -> f64 {
    (x / r - 1.0).abs()
}

// ------------------------------------------------------------------------------------------------------------ the model

pub const E: f64 = 2.0e11;
pub const A: f64 = 1.0;
pub const B: f64 = 2.0;
/// Cells over 90 degrees (ADR 0517 D3).
pub const NTH: usize = 15;
/// Cells per side of the shaft's square core.
pub const NC: usize = 8;
/// Side of the shaft's square core.
pub const CORE: f64 = 0.4 * A;
/// The axial offset of the shaft's node planes, as a fraction of the next cell (ADR 0517 D3).
pub const SHIFT: f64 = 0.05;

#[derive(Clone, Copy, Debug)]
pub struct Setup {
    /// Cells through `b - a` (the bore cell `h = (b - a)/n`).
    pub n: usize,
    /// Hub length.
    pub l: f64,
    /// Shaft extension beyond each face.
    pub e: f64,
    pub nu: f64,
    pub d: f64,
    /// `u_z = 0` at every node (control CPE).
    pub all_uz: bool,
    /// The axial offset of the shaft's planes.
    pub shift: f64,
}

pub const BASE: Setup = Setup {
    n: 8,
    l: 4.0 * (B - A),
    e: 2.0 * A,
    nu: 0.3,
    d: 1e-3 * A,
    all_uz: false,
    shift: SHIFT,
};

/// `p_0 = E d (b^2 - a^2)/(2 a b^2)` (ADR 0517 D1, `sigma_zz = 0`).
pub fn p_free(s: &Setup) -> f64 {
    E * s.d * (B * B - A * A) / (2.0 * A * B * B)
}

/// `p_pe = p_0/(1 - nu^2)` (ADR 0517 D1, plane strain).
pub fn p_plane_strain(s: &Setup) -> f64 {
    p_free(s) / (1.0 - s.nu * s.nu)
}

/// ADR 0517 D3's axial positions from a face (0) to `len`: cells of `h` over `0.5 (b - a)`, `m = round(1/(h + 0.25))` cells
/// rising linearly from `h` to `0.25 (b - a)` over the next `0.5 (b - a)`, then cells of `0.25 (b - a)`.
pub fn axial(len: f64, h: f64) -> Vec<f64> {
    let w = 0.5 * (B - A);
    let hc = 0.25 * (B - A);
    let rest = len - 2.0 * w;
    let nrest = (rest / hc).round() as usize;
    assert!(
        rest >= -1e-12 && (nrest as f64 * hc - rest).abs() < 1e-12,
        "length {len}"
    );
    let nz = (w / h).round() as usize;
    let mut x: Vec<f64> = (0..=nz).map(|i| w * i as f64 / nz as f64).collect();
    let m = (1.0 / (h / (B - A) + 0.25)).round() as usize;
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

pub struct Fit {
    pub setup: Setup,
    pub model: SolidModel,
    /// The hub's bore nodes by row (plane `k`, `z = zh[k]`) and angle `j`.
    pub bore: Vec<Vec<usize>>,
    pub zh: Vec<f64>,
}

pub fn fit_model(s: Setup) -> Fit {
    let h = (B - A) / s.n as f64;
    let half = 0.5 * s.l;
    let mut zh: Vec<f64> = axial(half, h).iter().rev().map(|v| half - v).collect();
    zh[0] = 0.0;
    let last = zh.len() - 1;
    zh[last] = half;
    let mut zm = zh.clone();
    zm.extend(axial(s.e, h).iter().skip(1).map(|v| half + v));
    let nzm = zm.len();
    zm[nzm - 1] = half + s.e;
    let zs: Vec<f64> = (0..nzm)
        .map(|k| {
            if k == 0 || k == nzm - 1 {
                zm[k]
            } else {
                zm[k] + s.shift * (zm[k + 1] - zm[k])
            }
        })
        .collect();
    let dth = 0.5 * PI / NTH as f64;
    // The shaft's cross-section: the core, then the band onto the polygon.
    let mut sp: Vec<[f64; 2]> = Vec::new();
    let core = |i: usize, j: usize| i + (NC + 1) * j;
    for j in 0..=NC {
        for i in 0..=NC {
            sp.push([CORE * i as f64 / NC as f64, CORE * j as f64 / NC as f64]);
        }
    }
    let nper = NTH + 2;
    assert_eq!(nper - 1, 2 * NC);
    let cb: Vec<usize> = (0..nper)
        .map(|k| if k <= NC { core(NC, k) } else { core(2 * NC - k, NC) })
        .collect();
    let per: Vec<[f64; 2]> = (0..nper)
        .map(|k| {
            if k == 0 {
                [A, 0.0]
            } else if k == nper - 1 {
                [0.0, A]
            } else {
                let th = (k as f64 - 0.5) * dth;
                let rv = A / (0.5 * dth).cos();
                [rv * th.cos(), rv * th.sin()]
            }
        })
        .collect();
    let nb = ((0.6 * s.n as f64).ceil() as usize).max(3);
    let mut band = vec![cb.clone()];
    for l in 1..=nb {
        let t = l as f64 / nb as f64;
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
    // The hub's cross-section: n uniform cells from a - d to b, angles j Delta.
    let mut hp: Vec<[f64; 2]> = Vec::new();
    for k in 0..=s.n {
        let r = (A - s.d) + k as f64 * (B - A + s.d) / s.n as f64;
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
        for l in 0..nb {
            for k in 0..nper - 1 {
                conn.extend_from_slice(&hx([band[l][k], band[l + 1][k], band[l + 1][k + 1], band[l][k + 1]]));
                if l == nb - 1 {
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
        for k in 0..s.n {
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
        if p[2] == 0.0 || s.all_uz {
            zset.push(v);
        }
    }
    let bore: Vec<Vec<usize>> = (0..zh.len())
        .map(|pl| (0..=NTH).map(|j| hid(pl, hub(0, j))).collect())
        .collect();
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
        .add_material("all", Material::isotropic_elastic(E, s.nu, 1.0, 0.0).unwrap())
        .unwrap();
    model.add_constraint(Constraint::displacement("x0", 0, 0.0)).unwrap();
    model.add_constraint(Constraint::displacement("y0", 1, 0.0)).unwrap();
    model.add_constraint(Constraint::displacement("z0", 2, 0.0)).unwrap();
    println!(
        "{s:?}: {} nodes, {ne} elements, {} hub planes, {} shaft planes, {nb} band layers",
        model.mesh().nodes().len(),
        zh.len(),
        zs.len()
    );
    Fit {
        setup: s,
        model,
        bore,
        zh,
    }
}

/// ADR 0517 D4's readings of one solve, every pressure over `p_0`.
pub struct Read {
    #[allow(dead_code)]
    pub setup: Setup,
    pub p_mid: f64,
    pub b025: f64,
    pub b05: f64,
    /// The face row's resultant.
    pub p_face: f64,
    pub q025: f64,
    pub q05: f64,
    /// The face row has the largest row resultant and holds the largest node pressure.
    pub peak_at_face: bool,
}

pub fn read(fit: &Fit, r: &ContactResult) -> Read {
    let s = fit.setup;
    let p0 = p_free(&s);
    let by: std::collections::HashMap<usize, (f64, f64, f64)> = r
        .nodes
        .iter()
        .map(|n| (n.node, (n.pressure, n.force, n.area)))
        .collect();
    let fa: Vec<(f64, f64)> = fit
        .bore
        .iter()
        .map(|row| {
            let f: f64 = row.iter().map(|v| by.get(v).map_or(f64::NAN, |x| x.1)).sum();
            let a: f64 = row.iter().map(|v| by.get(v).map_or(f64::NAN, |x| x.2)).sum();
            (f, a)
        })
        .collect();
    let rows: Vec<(f64, f64)> = fa.iter().zip(&fit.zh).map(|(x, &z)| (z, x.0 / x.1 / p0)).collect();
    let row_at = |z: f64| fit.zh.iter().position(|&x| (x - z).abs() < 1e-9).expect("a row at z");
    // The band [z0, z1] of rows: interior rows whole, the rows at its ends inside the bore weighted by half when the band
    // cuts the bore there (a band ending at z = 0 or at the face takes the end row whole: its area is already the half cell).
    let band = |z0: f64, z1: f64| {
        let (mut f, mut a) = (0.0, 0.0);
        for (k, &z) in fit.zh.iter().enumerate() {
            let inside = z >= z0 - 1e-9 && z <= z1 + 1e-9;
            if !inside {
                continue;
            }
            let at_cut = ((z - z0).abs() < 1e-9 && z0 > 1e-9) || ((z - z1).abs() < 1e-9 && z1 < 0.5 * s.l - 1e-9);
            let wgt = if at_cut { 0.5 } else { 1.0 };
            f += wgt * fa[k].0;
            a += wgt * fa[k].1;
        }
        f / a / p0
    };
    let half = 0.5 * s.l;
    let w1 = 0.25 * (B - A);
    let w2 = 0.5 * (B - A);
    let kf = fit.zh.len() - 1;
    let p_face = rows[kf].1;
    let row_max = worst(rows.iter().map(|x| x.1));
    let (mut node_max, mut node_row, mut all_finite) = (f64::NEG_INFINITY, usize::MAX, true);
    for (k, row) in fit.bore.iter().enumerate() {
        for v in row {
            let p = by.get(v).map_or(f64::NAN, |x| x.0);
            if !p.is_finite() {
                all_finite = false;
            } else if p > node_max {
                node_max = p;
                node_row = k;
            }
        }
    }
    let node_max = (node_max, node_row);
    let peak_at_face = all_finite && p_face.is_finite() && p_face == row_max && node_max.1 == kf;
    let out = Read {
        setup: s,
        p_mid: band(0.0, w1),
        b025: band(half - w1, half),
        b05: band(half - w2, half),
        p_face,
        q025: rows[row_at(half - w1)].1,
        q05: rows[row_at(half - w2)].1,
        peak_at_face,
    };
    println!("rows (z, F/A over p_0):");
    for (z, p) in &rows {
        println!("  {z:.5} {p:.6}");
    }
    println!(
        "p_mid {:.6} (over p_pe {:.6}), B_0.25 {:.6}, B_0.5 {:.6}, P_f {:.6}, q_0.25 {:.6}, q_0.5 {:.6}, largest node \
         pressure {:.6} in row {} of {kf}, peak at the face {}",
        out.p_mid,
        out.p_mid * p0 / p_plane_strain(&s),
        out.b025,
        out.b05,
        out.p_face,
        out.q025,
        out.q05,
        node_max.0 / p0,
        node_max.1,
        out.peak_at_face
    );
    out
}

// ------------------------------------------------------------------------------------------------------------ F-618

/// ADR 0542 D4's interference.
pub const D_TL: f64 = 1e-4 * A;

/// Strictly below the limit (T1's `< 1/3`).
pub fn below(c: &mut Checks, name: &str, value: f64, limit: f64) {
    let ok = value.is_finite() && value < limit;
    println!(
        "{name}: {value:.6e} (limit < {limit:e}) {}",
        if ok { "met" } else { "MISSED" }
    );
    if !ok {
        c.0.push(format!("{name} = {value:e} not < {limit:e}"));
    }
}

/// `max |x - y| / max |y|`; infinite if any value is not finite.
pub fn rel_vec(x: &[f64], y: &[f64]) -> f64 {
    if x.len() != y.len() {
        return f64::INFINITY;
    }
    let d = worst(x.iter().zip(y).map(|(a, b)| (a - b).abs()));
    let m = worst(y.iter().map(|a| a.abs()));
    d / m
}

pub fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
pub fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in v.iter().flat_map(|x| x.to_bits().to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// ADR 0533's `flat`: the displacement, the reactions, every contactor node's force, pressure, gap and local coordinates,
/// and every Newton residual, energy, penetration, convex count and active set.
pub fn flat(r: &ContactResult) -> Vec<f64> {
    let mut v = r.result.displacement_values().to_vec();
    if let Some(re) = r.result.reactions() {
        v.extend_from_slice(re);
    }
    for n in &r.nodes {
        v.extend([n.force, n.pressure, n.gap.unwrap_or(f64::NAN), n.local[0], n.local[1]]);
    }
    for s in &r.steps {
        for x in &s.solves {
            v.extend_from_slice(&x.residuals);
            v.extend_from_slice(&x.energies);
            v.push(x.penetration);
            v.push(x.convex_iterations as f64);
            for a in &x.active_sets {
                v.push(a.len() as f64);
                v.extend(a.iter().map(|&i| i as f64));
            }
        }
    }
    v
}

/// How the factor is chosen (ADR 0542 D3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Route {
    /// The present driver: every option at its default.
    Present,
    /// `ContactOptions::factor(SupernodalNestedDissection)`.
    Contact,
    /// `ContactOptions::newton(NewtonOptions::new(1).max_iterations(30).factor(SupernodalNestedDissection))`.
    Newton,
}

/// ADR 0517's options with the total Lagrangian body and the route's factor.
pub fn options(s: &Setup, route: Route) -> (ContactPair, ContactOptions) {
    let h = (B - A) / s.n as f64;
    let pair = ContactPair::new("bore", Target::Surface("shaft_out".into()), 100.0 * E / h);
    let mut newton = NewtonOptions::new(1).max_iterations(30);
    if route == Route::Newton {
        newton = newton.factor(DirectFactor::SupernodalNestedDissection);
    }
    let mut o = ContactOptions::new(1, 1e-6 * s.d)
        .newton(newton)
        .kinematics(Kinematics::TotalLagrangian);
    if route == Route::Contact {
        o = o.factor(DirectFactor::SupernodalNestedDissection);
    }
    (pair, o)
}

/// The Newton record: per solve of every step, the iterations and the convex iterations.
pub fn record(r: &ContactResult) -> Vec<(usize, usize)> {
    r.steps
        .iter()
        .flat_map(|s| s.solves.iter().map(|x| (x.residuals.len() - 1, x.convex_iterations)))
        .collect()
}

/// One solve and everything ADR 0542 compares.
pub struct Solve {
    pub read: Read,
    /// Every row's `F_k/A_k` over `p_0`.
    pub rows: Vec<f64>,
    /// The largest node pressure over `p_0` (NaN if any bore node's pressure is not finite) and its row.
    pub node_max: f64,
    pub node_row: usize,
    pub result: ContactResult,
    pub seconds: f64,
}

/// The total Lagrangian solve of `fit` by `route`, timed, and its readings.
pub fn solve_fit(fit: &Fit, route: Route) -> Solve {
    let s = fit.setup;
    let (pair, o) = options(&s, route);
    let t0 = Instant::now();
    let r: ContactResult = solve(&fit.model, &[], &[], &[pair], &o).unwrap();
    let seconds = t0.elapsed().as_secs_f64();
    println!(
        "{route:?}, n = {}: solve {seconds:.2} s, Newton record (iterations, convex) {:?}",
        s.n,
        record(&r)
    );
    let x = read(fit, &r);
    let p0 = p_free(&s);
    let by: std::collections::HashMap<usize, (f64, f64, f64)> = r
        .nodes
        .iter()
        .map(|n| (n.node, (n.pressure, n.force, n.area)))
        .collect();
    let rows: Vec<f64> = fit
        .bore
        .iter()
        .map(|row| {
            let f: f64 = row.iter().map(|v| by.get(v).map_or(f64::NAN, |x| x.1)).sum();
            let a: f64 = row.iter().map(|v| by.get(v).map_or(f64::NAN, |x| x.2)).sum();
            f / a / p0
        })
        .collect();
    let (mut node_max, mut node_row) = (f64::NEG_INFINITY, usize::MAX);
    for (k, row) in fit.bore.iter().enumerate() {
        for v in row {
            let p = by.get(v).map_or(f64::NAN, |x| x.0) / p0;
            if !p.is_finite() {
                node_max = f64::NAN;
            } else if p > node_max {
                node_max = p;
                node_row = k;
            }
        }
    }
    Solve {
        read: x,
        rows,
        node_max,
        node_row,
        result: r,
        seconds,
    }
}

/// V (V1, V1e or V2), V3 and U1 of ADR 0542: the supernodal solve `s` against the present `p`.
pub fn compare(c: &mut Checks, tag: &str, p: &Solve, s: &Solve) {
    let (a, b) = (&p.read, &s.read);
    for (name, xp, xs) in [
        ("p_mid", a.p_mid, b.p_mid),
        ("B_0.25", a.b025, b.b025),
        ("B_0.5", a.b05, b.b05),
        ("P_f", a.p_face, b.p_face),
        ("q_0.25", a.q025, b.q025),
        ("q_0.5", a.q05, b.q05),
        ("largest node pressure", p.node_max, s.node_max),
    ] {
        c.at_most(&format!("{tag}: |{name}_s/{name}_p - 1|"), rel(xs, xp), 1e-10);
    }
    let rows = if p.rows.len() == s.rows.len() {
        worst(p.rows.iter().zip(&s.rows).map(|(xp, xs)| rel(*xs, *xp)))
    } else {
        f64::INFINITY
    };
    c.at_most(&format!("{tag}: worst row |F/A_s / F/A_p - 1|"), rows, 1e-10);
    println!(
        "{tag}: largest node pressure in row {} (present) and {} (supernodal); E1 {} and {}",
        p.node_row, s.node_row, a.peak_at_face, b.peak_at_face
    );
    c.holds(
        &format!("{tag}: the row of the largest node pressure equal"),
        p.node_row == s.node_row,
    );
    c.holds(&format!("{tag}: E1's flag equal"), a.peak_at_face == b.peak_at_face);
    let (rp, rs) = (record(&p.result), record(&s.result));
    println!("{tag}: V3 Newton record present {rp:?}, supernodal {rs:?}");
    c.holds(&format!("{tag}: V3 the Newton record equal"), rp == rs);
    c.at_most(
        &format!("{tag}: U1 max |u_s - u_p| / max |u_p|"),
        rel_vec(
            s.result.result.displacement_values(),
            p.result.result.displacement_values(),
        ),
        1e-10,
    );
}

// ------------------------------------------------------------------------------------------------------------ halvings

/// ADR 0542 D5: no force; the tangent entry `-kick` at `eq` once `|u_eq - u_eq,start| > tau`, `u_eq,start` taken at the first
/// call with the attempt's load factor.
pub struct Spoiler {
    pub eq: usize,
    pub tau: f64,
    pub kick: f64,
    pub start: RefCell<Option<(u64, f64)>>,
}

impl Contribution for Spoiler {
    fn name(&self) -> &str {
        "spoiler"
    }
    fn add(
        &self,
        u: &[f64],
        load_factor: f64,
        _force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::error::Result<()> {
        let mut s = self.start.borrow_mut();
        if s.map(|x| x.0) != Some(load_factor.to_bits()) {
            *s = Some((load_factor.to_bits(), u[self.eq]));
        }
        let start = s.expect("set").1;
        if (u[self.eq] - start).abs() > self.tau {
            tangent.push((self.eq, self.eq, -self.kick));
        }
        Ok(())
    }
}

/// ADR 0542 Q5: the Hex8 cantilever `10 x 1 x 1` of `40 x 4 x 4` cells, clamped at `x = 0`, a dead traction on `x = 10` with
/// `P L^2/(E I) = 1`; and the equation of the tip's `u_y` at `(10, 0.5, 0.5)`.
pub fn cantilever() -> (SolidModel, usize) {
    let (l, h, e) = (10.0, 1.0, 2.0e11);
    let mesh = hex_box([0.0; 3], [l, h, h], [40, 4, 4]).unwrap();
    let tip = (0..mesh.nodes().len())
        .find(|&n| {
            let p = mesh.nodes()[n];
            (p[0] - l).abs() < 1e-9 && (p[1] - 0.5).abs() < 1e-9 && (p[2] - 0.5).abs() < 1e-9
        })
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(e, 0.3, 1.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let i = h * h * h * h / 12.0;
    let p = e * i / (l * l);
    m.add_load(Load::traction("xmax", [0.0, p / (h * h), 0.0])).unwrap();
    let eq = m.dof_map().equation(tip, 1).unwrap();
    (m, eq)
}

/// Per converged increment: the step, the load factor's bits, the halvings and the iterations.
pub fn increments(r: &NonlinearResult) -> Vec<(usize, u64, usize, usize)> {
    r.steps
        .iter()
        .map(|s| (s.step, s.load_factor.to_bits(), s.halvings, s.residuals.len() - 1))
        .collect()
}
