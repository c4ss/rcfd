//! F-604 (ADR 0517): press fit of a finite hub on a longer shaft, frictionless, same material. The mid-length pressure
//! against the closed forms, the edge pressure peak at the hub faces and what converges under refinement of the bore cells,
//! and a bore-cell rule. Every limit is the one registered in ADR 0517; the model is ADR 0517 D3, the readings D4.

use rcfd_core::vec3::Vec3;
use rcfd_fem::contact::{solve, ContactOptions, ContactPair, ContactResult, Target};
use rcfd_fem::nonlinear::NewtonOptions;
use rcfd_fem::{Constraint, ElementKind, FaceRef, Material, SolidMesh, SolidModel};
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
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
        }
    }
    fn within(&mut self, name: &str, value: f64, lo: f64, hi: f64) {
        let ok = value.is_finite() && value >= lo && value <= hi;
        println!(
            "{name}: {value:.6e} (limit in [{lo}, {hi}]) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not in [{lo}, {hi}]"));
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

fn run(s: Setup) -> Read {
    let fit = fit_model(s);
    let h = (B - A) / s.n as f64;
    let pair = ContactPair::new("bore", Target::Surface("shaft_out".into()), 100.0 * E / h);
    let o = ContactOptions::new(1, 1e-6 * s.d).newton(NewtonOptions::new(1).max_iterations(30));
    let t0 = Instant::now();
    let r: ContactResult = solve(&fit.model, &[], &[], &[pair], &o).unwrap();
    let st = &r.steps[0];
    println!(
        "solve {:.1} s, {} solves, iterations {:?}, penetration {:.3e}",
        t0.elapsed().as_secs_f64(),
        st.solves.len(),
        st.solves.iter().map(|x| x.residuals.len() - 1).collect::<Vec<_>>(),
        st.solves.last().unwrap().penetration
    );
    read(&fit, &r)
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

// ------------------------------------------------------------------------------------------------------------ fast tests

/// A1f, E1, E3f and NA1 at `n = 4, 8` (ADR 0517).
#[test]
fn f604_a1f_e1_hub_on_shaft_n4_n8() {
    let r4 = run(Setup { n: 4, ..BASE });
    let r8 = run(BASE);
    let mut c = Checks::new();
    for r in [&r4, &r8] {
        c.at_most(
            &format!("A1f n = {}: |p_mid/p_0 - 1|", r.setup.n),
            (r.p_mid - 1.0).abs(),
            2e-2,
        );
        c.holds(&format!("E1 n = {}: the peak at the face", r.setup.n), r.peak_at_face);
    }
    c.at_most("E3f |B_0.25(8)/B_0.25(4) - 1|", rel(r8.b025, r4.b025), 5e-2);
    c.at_most("E3f |B_0.5(8)/B_0.5(4) - 1|", rel(r8.b05, r4.b05), 5e-2);
    let pe = p_plane_strain(&BASE) / p_free(&BASE);
    c.at_least(
        "NA1 n = 8: |p_mid/p_pe - 1| (negative control)",
        rel(r8.p_mid, pe),
        5e-2,
    );
    c.finish();
}

/// CPE (`u_z = 0` at every node: plane strain gives `p_pe`) and CNU (`nu = 0`: `p_pe = p_0`), `n = 8`.
#[test]
fn f604_controls_plane_strain_and_nu0() {
    let pe = run(Setup { all_uz: true, ..BASE });
    let nu0 = run(Setup { nu: 0.0, ..BASE });
    let mut c = Checks::new();
    let ratio = p_plane_strain(&BASE) / p_free(&BASE);
    c.at_most("CPE |p_mid/p_pe - 1|", rel(pe.p_mid, ratio), 2e-2);
    c.at_least("CPE |p_mid/p_0 - 1|", (pe.p_mid - 1.0).abs(), 5e-2);
    c.holds("E1 CPE: the peak at the face", pe.peak_at_face);
    c.at_most("CNU |p_mid/p_0 - 1|", (nu0.p_mid - 1.0).abs(), 2e-2);
    c.holds("E1 CNU: the peak at the face", nu0.peak_at_face);
    c.finish();
}

/// S1 (the shaft extension `3 a` against `2 a`) and L8 (`L = 8 (b - a)`), `n = 8`.
#[test]
fn f604_extension_and_long_hub() {
    let r = run(BASE);
    let e3 = run(Setup { e: 3.0 * A, ..BASE });
    let l8 = run(Setup {
        l: 8.0 * (B - A),
        ..BASE
    });
    let mut c = Checks::new();
    c.at_most("S1 p_mid change", rel(e3.p_mid, r.p_mid), 1e-3);
    c.at_most("S1 B_0.25 change", rel(e3.b025, r.b025), 1e-3);
    c.at_most("L8 |p_mid/p_0 - 1|", (l8.p_mid - 1.0).abs(), 2e-2);
    c.at_most("L8 |B_0.25(L 8)/B_0.25(L 4) - 1|", rel(l8.b025, r.b025), 5e-3);
    for x in [&e3, &l8] {
        c.holds(&format!("E1 {:?}: the peak at the face", x.setup), x.peak_at_face);
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ records

/// A1, the acceptance's clause as written: the mid-length pressure against the plane-strain closed form within 2 percent.
/// ADR 0517 D1 predicts the miss: a frictionless hub with free faces is in the `sigma_zz = 0` state at mid-length.
#[test]
#[ignore = "ADR 0517 A1: known miss, measured 8.57e-2 against 2e-2 (D1: the plane-strain closed form does not hold at mid-length)"]
fn f604_a1_plane_strain_closed_form_known_miss() {
    let r = run(BASE);
    let mut c = Checks::new();
    let ratio = p_plane_strain(&BASE) / p_free(&BASE);
    c.at_most("A1 n = 8: |p_mid/p_pe - 1|", rel(r.p_mid, ratio), 2e-2);
    c.finish();
}

/// The heavy record (ADR 0517 D6): `n = 4, 8, 16, 32` at `L = 4 (b - a)`: A1f on every mesh, E1, E2, E3, E4 and the
/// bore-cell rule R.
#[test]
#[ignore = "ADR 0517 D6 heavy record: n = 4, 8, 16 ran; the n = 32 solve failed (active set cycled, the projection defect of amendment 1); superseded by A1-H"]
fn f604_edge_convergence_and_bore_cell_rule_heavy() {
    let rs: Vec<Read> = [4, 8, 16, 32].iter().map(|&n| run(Setup { n, ..BASE })).collect();
    let mut c = Checks::new();
    for r in &rs {
        c.at_most(
            &format!("A1f n = {}: |p_mid/p_0 - 1|", r.setup.n),
            (r.p_mid - 1.0).abs(),
            2e-2,
        );
        c.holds(&format!("E1 n = {}: the peak at the face", r.setup.n), r.peak_at_face);
    }
    // E2: the peak diverges as D2 predicts.
    for i in 0..3 {
        c.within(
            &format!("E2 P_f({})/P_f({})", rs[i + 1].setup.n, rs[i].setup.n),
            rs[i + 1].p_face / rs[i].p_face,
            1.08,
            1.25,
        );
    }
    c.at_most(
        "E2 |log2(P_f(32)/P_f(16)) - 0.226|",
        ((rs[3].p_face / rs[2].p_face).log2() - 0.226).abs(),
        0.05,
    );
    // E3: the band resultants converge.
    for (name, get) in [
        ("B_0.25", (|r: &Read| r.b025) as fn(&Read) -> f64),
        ("B_0.5", |r: &Read| r.b05),
    ] {
        let b: Vec<f64> = rs.iter().map(get).collect();
        let dd = [b[1] - b[0], b[2] - b[1], b[3] - b[2]];
        println!("E3 {name}: {b:?}, differences {dd:?}");
        c.holds(
            &format!("E3 {name}: |D_3| < |D_2| < |D_1|"),
            dd.iter().all(|x| x.is_finite()) && dd[2].abs() < dd[1].abs() && dd[1].abs() < dd[0].abs(),
        );
        c.at_least(&format!("E3 {name}: order 4-8-16"), (dd[0] / dd[1]).abs().log2(), 0.5);
        c.at_least(&format!("E3 {name}: order 8-16-32"), (dd[1] / dd[2]).abs().log2(), 0.5);
        c.at_most(&format!("E3 {name}: |D_3|/B(32)"), (dd[2] / b[3]).abs(), 1e-2);
    }
    // E4: the fixed-distance resultants change little.
    c.at_most("E4 |q_0.25(32)/q_0.25(16) - 1|", rel(rs[3].q025, rs[2].q025), 1e-2);
    c.at_most("E4 |q_0.5(32)/q_0.5(16) - 1|", rel(rs[3].q05, rs[2].q05), 1e-2);
    // R: the bore-cell rule.
    let f = &rs[3];
    let mut rule = None;
    for r in &rs[..3] {
        let e = [
            rel(r.b025, f.b025),
            rel(r.b05, f.b05),
            rel(r.p_mid, f.p_mid),
            (r.p_mid - 1.0).abs(),
        ];
        let ok = e.iter().all(|x| x.is_finite()) && e[0] <= 2e-2 && e[1] <= 2e-2 && e[2] <= 1e-2 && e[3] <= 2e-2;
        println!(
            "R n = {}: B_0.25 {:.3e}, B_0.5 {:.3e}, p_mid {:.3e} against n = 32; p_mid {:.3e} against p_0: {}",
            r.setup.n,
            e[0],
            e[1],
            e[2],
            e[3],
            if ok { "meets" } else { "does not meet" }
        );
        if ok && rule.is_none() {
            rule = Some(r.setup.n);
        }
    }
    println!("R: the bore-cell rule n* = {rule:?} cells through b - a");
    c.holds("R: a rule n* <= 16 exists", rule.is_some());
    c.finish();
}

/// A1-H (ADR 0517 amendment 1): the heavy record run again after the fix of `project`, the same model, checks and limits:
/// `n = 4, 8, 16, 32` at `L = 4 (b - a)`: A1f on every mesh, E1, E2, E3, E4 and the
/// bore-cell rule R.
#[test]
#[ignore = "heavy record (ADR 0517 amendment 1, A1-H): four meshes up to about 100k unknowns, about 25 minutes; run once, alone"]
fn f604_edge_convergence_and_bore_cell_rule_heavy_a1() {
    let rs: Vec<Read> = [4, 8, 16, 32].iter().map(|&n| run(Setup { n, ..BASE })).collect();
    let mut c = Checks::new();
    for r in &rs {
        c.at_most(
            &format!("A1f n = {}: |p_mid/p_0 - 1|", r.setup.n),
            (r.p_mid - 1.0).abs(),
            2e-2,
        );
        c.holds(&format!("E1 n = {}: the peak at the face", r.setup.n), r.peak_at_face);
    }
    // E2: the peak diverges as D2 predicts.
    for i in 0..3 {
        c.within(
            &format!("E2 P_f({})/P_f({})", rs[i + 1].setup.n, rs[i].setup.n),
            rs[i + 1].p_face / rs[i].p_face,
            1.08,
            1.25,
        );
    }
    c.at_most(
        "E2 |log2(P_f(32)/P_f(16)) - 0.226|",
        ((rs[3].p_face / rs[2].p_face).log2() - 0.226).abs(),
        0.05,
    );
    // E3: the band resultants converge.
    for (name, get) in [
        ("B_0.25", (|r: &Read| r.b025) as fn(&Read) -> f64),
        ("B_0.5", |r: &Read| r.b05),
    ] {
        let b: Vec<f64> = rs.iter().map(get).collect();
        let dd = [b[1] - b[0], b[2] - b[1], b[3] - b[2]];
        println!("E3 {name}: {b:?}, differences {dd:?}");
        c.holds(
            &format!("E3 {name}: |D_3| < |D_2| < |D_1|"),
            dd.iter().all(|x| x.is_finite()) && dd[2].abs() < dd[1].abs() && dd[1].abs() < dd[0].abs(),
        );
        c.at_least(&format!("E3 {name}: order 4-8-16"), (dd[0] / dd[1]).abs().log2(), 0.5);
        c.at_least(&format!("E3 {name}: order 8-16-32"), (dd[1] / dd[2]).abs().log2(), 0.5);
        c.at_most(&format!("E3 {name}: |D_3|/B(32)"), (dd[2] / b[3]).abs(), 1e-2);
    }
    // E4: the fixed-distance resultants change little.
    c.at_most("E4 |q_0.25(32)/q_0.25(16) - 1|", rel(rs[3].q025, rs[2].q025), 1e-2);
    c.at_most("E4 |q_0.5(32)/q_0.5(16) - 1|", rel(rs[3].q05, rs[2].q05), 1e-2);
    // R: the bore-cell rule.
    let f = &rs[3];
    let mut rule = None;
    for r in &rs[..3] {
        let e = [
            rel(r.b025, f.b025),
            rel(r.b05, f.b05),
            rel(r.p_mid, f.p_mid),
            (r.p_mid - 1.0).abs(),
        ];
        let ok = e.iter().all(|x| x.is_finite()) && e[0] <= 2e-2 && e[1] <= 2e-2 && e[2] <= 1e-2 && e[3] <= 2e-2;
        println!(
            "R n = {}: B_0.25 {:.3e}, B_0.5 {:.3e}, p_mid {:.3e} against n = 32; p_mid {:.3e} against p_0: {}",
            r.setup.n,
            e[0],
            e[1],
            e[2],
            e[3],
            if ok { "meets" } else { "does not meet" }
        );
        if ok && rule.is_none() {
            rule = Some(r.setup.n);
        }
    }
    println!("R: the bore-cell rule n* = {rule:?} cells through b - a");
    c.holds("R: a rule n* <= 16 exists", rule.is_some());
    c.finish();
}
