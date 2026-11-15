//! F-618 (ADR 0542): the supernodal factor in the Newton driver (`NewtonOptions::factor`), for the total Lagrangian contact
//! body. ADR 0517's hub with the total Lagrangian body and `d = 1e-4 a` (ADR 0542 D4) solved by the present driver (the
//! envelope factor) and with `DirectFactor::SupernodalNestedDissection`: the readings of ADR 0517 D4 against each other, the
//! Newton record, the pass-through of the contact driver, the halvings of a cantilever with ADR 0542 D5's device, the refused
//! pivot of ADR 0517's own interference, the refusals, and the heavy record at `n = 16` with the wall times. Every limit is the
//! one registered in ADR 0542; the model and `read` are copies of `tests/sa_contact_hub_f604.rs` (ADR 0517 D3, D4).

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

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool) {
        println!("{name}: {}", if ok { "holds" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name} does not hold"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

/// The largest of the values; infinite if any is not finite (a NaN must not hide behind `f64::max`).
fn worst(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut w: f64 = 0.0;
    for v in values {
        if !v.is_finite() {
            return f64::INFINITY;
        }
        w = w.max(v);
    }
    w
}

fn rel(x: f64, r: f64) -> f64 {
    (x / r - 1.0).abs()
}

// ------------------------------------------------------------------------------------------------------------ the model

const E: f64 = 2.0e11;
const A: f64 = 1.0;
const B: f64 = 2.0;
/// Cells over 90 degrees (ADR 0517 D3).
const NTH: usize = 15;
/// Cells per side of the shaft's square core.
const NC: usize = 8;
/// Side of the shaft's square core.
const CORE: f64 = 0.4 * A;
/// The axial offset of the shaft's node planes, as a fraction of the next cell (ADR 0517 D3).
const SHIFT: f64 = 0.05;

#[derive(Clone, Copy, Debug)]
struct Setup {
    /// Cells through `b - a` (the bore cell `h = (b - a)/n`).
    n: usize,
    /// Hub length.
    l: f64,
    /// Shaft extension beyond each face.
    e: f64,
    nu: f64,
    d: f64,
    /// `u_z = 0` at every node (control CPE).
    all_uz: bool,
    /// The axial offset of the shaft's planes.
    shift: f64,
}

const BASE: Setup = Setup {
    n: 8,
    l: 4.0 * (B - A),
    e: 2.0 * A,
    nu: 0.3,
    d: 1e-3 * A,
    all_uz: false,
    shift: SHIFT,
};

/// `p_0 = E d (b^2 - a^2)/(2 a b^2)` (ADR 0517 D1, `sigma_zz = 0`).
fn p_free(s: &Setup) -> f64 {
    E * s.d * (B * B - A * A) / (2.0 * A * B * B)
}

/// `p_pe = p_0/(1 - nu^2)` (ADR 0517 D1, plane strain).
fn p_plane_strain(s: &Setup) -> f64 {
    p_free(s) / (1.0 - s.nu * s.nu)
}

/// ADR 0517 D3's axial positions from a face (0) to `len`: cells of `h` over `0.5 (b - a)`, `m = round(1/(h + 0.25))` cells
/// rising linearly from `h` to `0.25 (b - a)` over the next `0.5 (b - a)`, then cells of `0.25 (b - a)`.
fn axial(len: f64, h: f64) -> Vec<f64> {
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

struct Fit {
    setup: Setup,
    model: SolidModel,
    /// The hub's bore nodes by row (plane `k`, `z = zh[k]`) and angle `j`.
    bore: Vec<Vec<usize>>,
    zh: Vec<f64>,
}

fn fit_model(s: Setup) -> Fit {
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
struct Read {
    #[allow(dead_code)]
    setup: Setup,
    p_mid: f64,
    b025: f64,
    b05: f64,
    /// The face row's resultant.
    p_face: f64,
    q025: f64,
    q05: f64,
    /// The face row has the largest row resultant and holds the largest node pressure.
    peak_at_face: bool,
}

fn read(fit: &Fit, r: &ContactResult) -> Read {
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
const D_TL: f64 = 1e-4 * A;

/// Strictly below the limit (T1's `< 1/3`).
fn below(c: &mut Checks, name: &str, value: f64, limit: f64) {
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
fn rel_vec(x: &[f64], y: &[f64]) -> f64 {
    if x.len() != y.len() {
        return f64::INFINITY;
    }
    let d = worst(x.iter().zip(y).map(|(a, b)| (a - b).abs()));
    let m = worst(y.iter().map(|a| a.abs()));
    d / m
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in v.iter().flat_map(|x| x.to_bits().to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// ADR 0533's `flat`: the displacement, the reactions, every contactor node's force, pressure, gap and local coordinates,
/// and every Newton residual, energy, penetration, convex count and active set.
fn flat(r: &ContactResult) -> Vec<f64> {
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
enum Route {
    /// The present driver: every option at its default.
    Present,
    /// `ContactOptions::factor(SupernodalNestedDissection)`.
    Contact,
    /// `ContactOptions::newton(NewtonOptions::new(1).max_iterations(30).factor(SupernodalNestedDissection))`.
    Newton,
}

/// ADR 0517's options with the total Lagrangian body and the route's factor.
fn options(s: &Setup, route: Route) -> (ContactPair, ContactOptions) {
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
fn record(r: &ContactResult) -> Vec<(usize, usize)> {
    r.steps
        .iter()
        .flat_map(|s| s.solves.iter().map(|x| (x.residuals.len() - 1, x.convex_iterations)))
        .collect()
}

/// One solve and everything ADR 0542 compares.
struct Solve {
    read: Read,
    /// Every row's `F_k/A_k` over `p_0`.
    rows: Vec<f64>,
    /// The largest node pressure over `p_0` (NaN if any bore node's pressure is not finite) and its row.
    node_max: f64,
    node_row: usize,
    result: ContactResult,
    seconds: f64,
}

/// The total Lagrangian solve of `fit` by `route`, timed, and its readings.
fn solve_fit(fit: &Fit, route: Route) -> Solve {
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
fn compare(c: &mut Checks, tag: &str, p: &Solve, s: &Solve) {
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

/// V2, V3, U1, N1 at `n = 4` and `n = 8`, and C1 at `n = 4` (ADR 0542).
#[test]
fn f618_v2_tl_hub_n4_n8() {
    let mut c = Checks::new();
    for n in [4usize, 8] {
        let fit = fit_model(Setup { n, d: D_TL, ..BASE });
        let p = solve_fit(&fit, Route::Present);
        let s = solve_fit(&fit, Route::Contact);
        compare(&mut c, &format!("V2 n = {n}"), &p, &s);
        c.holds(
            &format!("N1 n = {n}: the supernodal displacement not bit-equal to the present one"),
            bits(s.result.result.displacement_values()) != bits(p.result.result.displacement_values()),
        );
        if n == 4 {
            let s2 = solve_fit(&fit, Route::Newton);
            let (d1, d2) = (digest(&flat(&s.result)), digest(&flat(&s2.result)));
            println!("C1 n = 4: ContactOptions::factor {d1:#018x}, NewtonOptions::factor {d2:#018x}");
            c.holds("C1 n = 4: the two routes give the same bits", d1 == d2);
        }
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ halvings

/// ADR 0542 D5: no force; the tangent entry `-kick` at `eq` once `|u_eq - u_eq,start| > tau`, `u_eq,start` taken at the first
/// call with the attempt's load factor.
struct Spoiler {
    eq: usize,
    tau: f64,
    kick: f64,
    start: RefCell<Option<(u64, f64)>>,
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
fn cantilever() -> (SolidModel, usize) {
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
fn increments(r: &NonlinearResult) -> Vec<(usize, u64, usize, usize)> {
    r.steps
        .iter()
        .map(|s| (s.step, s.load_factor.to_bits(), s.halvings, s.residuals.len() - 1))
        .collect()
}

/// H1 to H3, N1 and NH of ADR 0542: the cantilever with the device, present against supernodal.
#[test]
fn f618_h_halvings_cantilever() {
    let (m, eq) = cantilever();
    let plain = nl_solve(&m, &[], &[], &NewtonOptions::new(2)).unwrap();
    let tau = 0.35 * plain.result.displacement_values()[eq];
    let kick = 10.0 * rcfd_fem::assemble::stiffness(&m).unwrap().diag()[eq];
    println!(
        "H: tip deflection {:e}, tau {tau:e}, kick {kick:e}",
        plain.result.displacement_values()[eq]
    );
    let run = |o: &NewtonOptions| {
        let dev = Spoiler {
            eq,
            tau,
            kick,
            start: RefCell::new(None),
        };
        nl_solve_with(
            &m,
            &[],
            &[],
            o,
            DriverExtensions {
                contributions: vec![&dev],
                ..Default::default()
            },
        )
    };
    let present = NewtonOptions::new(2);
    let supernodal = NewtonOptions::new(2).factor(DirectFactor::SupernodalNestedDissection);
    let p = run(&present).unwrap();
    let s = run(&supernodal).unwrap();
    let (ip, is) = (increments(&p), increments(&s));
    println!("H1 present {ip:?}");
    println!("H1 supernodal {is:?}");
    let mut c = Checks::new();
    c.holds(
        "H1 the increments (step, load factor, halvings, iterations) equal",
        ip == is,
    );
    c.at_most(
        "H2 max |u_s - u_p| / max |u_p|",
        rel_vec(s.result.displacement_values(), p.result.displacement_values()),
        1e-10,
    );
    c.holds(
        "N1 cantilever: the supernodal displacement not bit-equal to the present one",
        bits(s.result.displacement_values()) != bits(p.result.displacement_values()),
    );
    c.holds(
        "NH the present run halves at least once",
        p.steps.iter().any(|x| x.halvings > 0),
    );
    for (name, o) in [("present", present), ("supernodal", supernodal)] {
        let mut o = o;
        o.max_halvings = 0;
        let e = run(&o);
        println!("H3 {name}, 0 halvings: {:?}", e.as_ref().err());
        c.holds(
            &format!("H3 {name}: 0 halvings ends with the singular tangent"),
            matches!(&e, Err(FemError::Solver { solver, reason })
                if *solver == "newton" && reason.contains("after 0 halvings: singular tangent: pivot")),
        );
    }
    c.finish();
}

/// P of ADR 0542: ADR 0517's own interference `d = 1e-3 a` with the total Lagrangian body at `n = 4`, refused by both
/// factors at the first tangent.
#[test]
fn f618_p_refused_pivot_tl_hub() {
    let s = Setup { n: 4, ..BASE };
    let fit = fit_model(s);
    let mut c = Checks::new();
    for route in [Route::Present, Route::Contact] {
        let (pair, o) = options(&s, route);
        let e = solve(&fit.model, &[], &[], &[pair], &o);
        println!("P {route:?}: {:?}", e.as_ref().err());
        c.holds(
            &format!("P {route:?}: the first tangent refused"),
            matches!(&e, Err(FemError::Solver { solver, reason }) if *solver == "contact"
                && reason.starts_with("load step 1 of 1, augmentation 0: solver newton: singular tangent: pivot")),
        );
    }
    c.finish();
}

fn is_solver_refusal<T>(e: &Result<T, FemError>) -> bool {
    matches!(e, Err(FemError::InvalidValue { key: "solver", .. }))
}

/// N2 of ADR 0542: the refusals with the key "solver".
#[test]
fn f618_n2_refusals() {
    let s = Setup { n: 4, ..BASE };
    let fit = fit_model(s);
    let mut c = Checks::new();
    // (a) the envelope in the contact driver (ADR 0533 N2, its first half).
    let (pair, o) = options(&s, Route::Present);
    let e = solve(&fit.model, &[], &[], &[pair], &o.factor(DirectFactor::Envelope));
    println!("N2a: {:?}", e.as_ref().err());
    c.holds(
        "N2a the envelope refused by ContactOptions (key solver)",
        is_solver_refusal(&e),
    );
    // (b) SparseMinimumDegree in the Newton driver.
    let (m, _) = cantilever();
    let e = nl_solve(
        &m,
        &[],
        &[],
        &NewtonOptions::new(2).factor(DirectFactor::SparseMinimumDegree),
    );
    println!("N2b: {:?}", e.as_ref().err());
    c.holds(
        "N2b SparseMinimumDegree refused by NewtonOptions (key solver)",
        is_solver_refusal(&e),
    );
    // (c) the linear contact body with a supernodal newton.factor.
    let (pair, o) = options(&s, Route::Newton);
    let e = solve(&fit.model, &[], &[], &[pair], &o.kinematics(Kinematics::Linear));
    println!("N2c: {:?}", e.as_ref().err());
    c.holds(
        "N2c newton.factor with the linear contact body refused (key solver)",
        is_solver_refusal(&e),
    );
    // (d) the arc-length path.
    let ao = ArcLengthOptions::new(0.1).newton(NewtonOptions::new(1).factor(DirectFactor::SupernodalNestedDissection));
    let e = trace(&m, &[], &[], &ao, &[], &mut |_| true);
    println!("N2d: {:?}", e.as_ref().err());
    c.holds(
        "N2d the supernodal factor refused by the arc-length path (key solver)",
        is_solver_refusal(&e),
    );
    c.finish();
}

/// The heavy record (ADR 0542): D4's hub at `n = 16`, present and supernodal alternating three times in this process: T1
/// (the ratio of the medians), V1, V1e, V3, U1 on the first pair, R2 (repeats bit-equal), D (a pool of 1 thread) and R.
#[test]
#[ignore = "known miss T1 of ADR 0542 (heavy record, run of fc7a5560, f618-scratch/run-heavy.log): median(t_s)/median(t_p) = 13.46 s / 37.96 s = 0.3546 against the limit < 1/3 (pair ratios 0.283, 0.419, 0.353); V1 (worst 3.5e-11), V1e, V3, U1 (7.4e-14), R2 and D met; about 3 minutes, run once, alone"]
fn f618_tl_hub_n16_supernodal_against_present_heavy_known_miss() {
    let fit = fit_model(Setup { n: 16, d: D_TL, ..BASE });
    println!("R: rayon threads {}", rayon::current_num_threads());
    let mut ps = Vec::new();
    let mut ss = Vec::new();
    for _ in 0..3 {
        ps.push(solve_fit(&fit, Route::Present));
        ss.push(solve_fit(&fit, Route::Contact));
    }
    let median = |v: &[Solve]| {
        let mut t: Vec<f64> = v.iter().map(|x| x.seconds).collect();
        t.sort_by(|a, b| a.total_cmp(b));
        t[1]
    };
    let (tp, ts) = (median(&ps), median(&ss));
    for (k, (p, s)) in ps.iter().zip(&ss).enumerate() {
        println!(
            "R pair {}: t_p = {:.2} s, t_s = {:.2} s, ratio {:.4}",
            k + 1,
            p.seconds,
            s.seconds,
            s.seconds / p.seconds
        );
    }
    println!("R: median t_p = {tp:.2} s (ADR 0542 Q1: 34.30 s), median t_s = {ts:.2} s");
    let mut c = Checks::new();
    below(&mut c, "T1 n = 16: median(t_s)/median(t_p)", ts / tp, 1.0 / 3.0);
    compare(&mut c, "V1 n = 16", &ps[0], &ss[0]);
    let dp: Vec<u64> = ps.iter().map(|x| digest(&flat(&x.result))).collect();
    let ds: Vec<u64> = ss.iter().map(|x| digest(&flat(&x.result))).collect();
    println!("R2 present digests {dp:x?}, supernodal {ds:x?}");
    c.holds(
        "R2 the three present solves bit-equal, and the three supernodal ones",
        dp.iter().all(|&x| x == dp[0]) && ds.iter().all(|&x| x == ds[0]),
    );
    let pool = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap();
    let one = pool.install(|| digest(&flat(&solve_fit(&fit, Route::Contact).result)));
    println!(
        "D: 1 thread {one:#018x}, {} threads {:#018x}",
        rayon::current_num_threads(),
        ds[0]
    );
    c.holds(
        "D n = 16: the supernodal solve in a pool of 1 thread bit-equal",
        one == ds[0],
    );
    c.finish();
}
