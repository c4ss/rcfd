//! F-599 (ADR 0513): the through-thickness variation of the ring-on-shaft interference fit of ADR 0507 D7 I, its diagnosis
//! and I3 re-registered. Every limit is the one registered in ADR 0513; the model is ADR 0507's with the parameters of
//! ADR 0513 D4 (layers through the thickness, the ring's radial cells, the `u_z` constraints, `nu`, `d`).

use rcfd_core::vec3::Vec3;
use rcfd_fem::contact::{solve, ContactOptions, ContactPair, ContactResult, Target};
use rcfd_fem::nonlinear::NewtonOptions;
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::{Constraint, ElementKind, FaceRef, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

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
    fn greater(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value > limit;
        println!(
            "{name}: {value:.6e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
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

// ------------------------------------------------------------------------------------------------------------ the fit

const E_FIT: f64 = 2.0e11;
const A_FIT: f64 = 1.0;
const B_FIT: f64 = 2.0;
const T_FIT: f64 = 0.1;
const N_FIT: usize = 64;

/// The `u_z` constraints of ADR 0513's probe.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Zbc {
    /// ADR 0507: `u_z = 0` on every node of `z = 0`.
    Bottom,
    /// Plane strain: `u_z = 0` on every node of `z = 0` and of `z = T`.
    Both,
    /// `u_z` free: `u_z = 0` at three bottom nodes of each body only.
    Minimal,
    /// `u_z = 0` on the mid-plane nodes (an even number of layers).
    Mid,
}

#[derive(Clone, Copy, Debug)]
struct Setup {
    nz: usize,
    nr: usize,
    zbc: Zbc,
    nu: f64,
    d: f64,
}

const ADR_0507: Setup = Setup {
    nz: 1,
    nr: 8,
    zbc: Zbc::Bottom,
    nu: 0.3,
    d: 1e-3,
};

struct Fit {
    setup: Setup,
    model: SolidModel,
    ring: Vec<usize>,
    shaft: Vec<usize>,
    /// Nodes per plane.
    n2: usize,
}

/// ADR 0507's `fit_model` with the parameters of `s` (ADR 0513 D4); `ADR_0507` is ADR 0507's model.
fn fit_model(s: Setup) -> Fit {
    let (nz, nr, d) = (s.nz, s.nr, s.d);
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
        let t = l as f64 / layers as f64;
        let mut row = Vec::new();
        for (p, &q) in per.iter().enumerate() {
            let th = (p as f64 + 0.5) * dth;
            let c = pts[q];
            pts.push([
                (1.0 - t) * c[0] + t * av * th.cos(),
                (1.0 - t) * c[1] + t * av * th.sin(),
            ]);
            row.push(pts.len() - 1);
        }
        band.push(row);
    }
    let ring0 = pts.len();
    let rk = |k: usize| (A_FIT - d) + k as f64 * (B_FIT - A_FIT + d) / nr as f64;
    let ring = |j: usize, k: usize| ring0 + (j % N_FIT) + N_FIT * k;
    for k in 0..=nr {
        for j in 0..N_FIT {
            let th = j as f64 * dth;
            pts.push([rk(k) * th.cos(), rk(k) * th.sin()]);
        }
    }
    let n2 = pts.len();
    let mut nodes: Vec<Vec3> = Vec::new();
    for l in 0..=nz {
        let z = T_FIT * l as f64 / nz as f64;
        nodes.extend(pts.iter().map(|p| [p[0], p[1], z]));
    }
    let mut conn = Vec::new();
    let (mut shaft_el, mut ring_el, mut shaft_out, mut ring_in) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut e = 0;
    for lz in 0..nz {
        let o = lz * n2;
        let hex = |q: [usize; 4]| {
            [
                q[0] + o,
                q[1] + o,
                q[2] + o,
                q[3] + o,
                q[0] + o + n2,
                q[1] + o + n2,
                q[2] + o + n2,
                q[3] + o + n2,
            ]
        };
        for j in 0..m {
            for i in 0..m {
                conn.extend_from_slice(&hex([core(i, j), core(i + 1, j), core(i + 1, j + 1), core(i, j + 1)]));
                shaft_el.push(e);
                e += 1;
            }
        }
        for l in 0..layers {
            for p in 0..N_FIT {
                let p1 = (p + 1) % N_FIT;
                conn.extend_from_slice(&hex([band[l][p], band[l + 1][p], band[l + 1][p1], band[l][p1]]));
                shaft_el.push(e);
                if l == layers - 1 {
                    shaft_out.push(FaceRef { element: e, face: 3 });
                }
                e += 1;
            }
        }
        for k in 0..nr {
            for j in 0..N_FIT {
                conn.extend_from_slice(&hex([ring(j, k), ring(j, k + 1), ring(j + 1, k + 1), ring(j + 1, k)]));
                ring_el.push(e);
                if k == 0 {
                    ring_in.push(FaceRef { element: e, face: 5 });
                }
                e += 1;
            }
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, conn).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_surface_set("ring_in", ring_in).unwrap();
    mesh.add_surface_set("shaft_out", shaft_out).unwrap();
    let column = |v: usize| (0..=nz).map(|l| v + l * n2).collect::<Vec<_>>();
    let zset: Vec<usize> = match s.zbc {
        Zbc::Bottom => (0..n2).collect(),
        Zbc::Both => (0..n2).chain(nz * n2..(nz + 1) * n2).collect(),
        Zbc::Minimal => vec![
            core(8, 8),
            core(m, 8),
            core(8, m),
            ring(0, nr),
            ring(16, nr),
            ring(32, nr),
        ],
        Zbc::Mid => {
            assert!(nz % 2 == 0, "Mid needs an even number of layers");
            (nz / 2 * n2..(nz / 2 + 1) * n2).collect()
        }
    };
    mesh.add_node_set("zfix", zset).unwrap();
    mesh.add_node_set("centre", column(core(8, 8))).unwrap();
    mesh.add_node_set("shaft_x", column(core(m, 8))).unwrap();
    mesh.add_node_set("ring_0", column(ring(0, nr))).unwrap();
    let mut r90 = column(ring(16, nr));
    r90.extend(column(ring(48, nr)));
    mesh.add_node_set("ring_90_270", r90).unwrap();
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(E_FIT, s.nu, 1.0, 0.0).unwrap())
        .unwrap();
    model.add_constraint(Constraint::displacement("zfix", 2, 0.0)).unwrap();
    for (set, c) in [
        ("centre", 0),
        ("centre", 1),
        ("shaft_x", 1),
        ("ring_0", 1),
        ("ring_90_270", 0),
    ] {
        model.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
    Fit {
        setup: s,
        model,
        ring: ring_el,
        shaft: shaft_el,
        n2,
    }
}

/// The contactor nodes of a row with their pressures over the mean.
type Row = Vec<(usize, f64)>;

struct Solved {
    fit: Fit,
    r: ContactResult,
    /// The mean node pressure.
    mean: f64,
}

/// ADR 0507's solve: penalty `100 E/(1/8)`, one step, tolerance `1e-6 d`, Newton 30 iterations.
fn solve_fit(s: Setup) -> Solved {
    let fit = fit_model(s);
    let pair = ContactPair::new("ring_in", Target::Surface("shaft_out".into()), 100.0 * E_FIT / 0.125);
    let o = ContactOptions::new(1, 1e-6 * s.d).newton(NewtonOptions::new(1).max_iterations(30));
    let r = solve(&fit.model, &[], &[], &[pair], &o).unwrap();
    let mean = r.nodes.iter().map(|n| n.pressure).sum::<f64>() / r.nodes.len() as f64;
    println!(
        "{s:?}: {} contactor nodes, mean pressure {mean:.6e}, penetration {:.3e}",
        r.nodes.len(),
        r.steps[0].solves.last().unwrap().penetration
    );
    Solved { fit, r, mean }
}

impl Solved {
    fn z(&self, node: usize) -> f64 {
        self.fit.model.mesh().nodes()[node][2]
    }
    /// The node pressures over the mean of the bottom row (`z = 0`) and of the top row (`z = T`), by node.
    fn rows(&self) -> (Row, Row) {
        let (mut bot, mut top) = (Vec::new(), Vec::new());
        for n in &self.r.nodes {
            let z = self.z(n.node);
            if z.abs() < 1e-12 {
                bot.push((n.node, n.pressure / self.mean));
            } else if (z - T_FIT).abs() < 1e-12 {
                top.push((n.node, n.pressure / self.mean));
            }
        }
        assert_eq!(bot.len(), N_FIT);
        assert_eq!(top.len(), N_FIT);
        (bot, top)
    }
    fn row_means(&self) -> (f64, f64) {
        let (b, t) = self.rows();
        let m = |v: &[(usize, f64)]| v.iter().map(|x| x.1).sum::<f64>() / v.len() as f64;
        (m(&b), m(&t))
    }
    /// The split `s` of ADR 0513: the top-row mean over the bottom-row mean, minus one.
    fn split(&self) -> f64 {
        let (b, t) = self.row_means();
        t / b - 1.0
    }
    /// The shaft's stress at every integration point against `-p` in `xx, yy`, `szz` in `zz` and 0 in the shears, over `p`.
    fn shaft_error(&self, p: f64, szz: f64) -> f64 {
        let u = self.r.result.displacement_values();
        let f = IntegrationPointField::compute(&self.fit.model, u).unwrap();
        let off = f.offsets();
        let reference = [-p, -p, szz, 0.0, 0.0, 0.0];
        worst(self.fit.shaft.iter().flat_map(|&e| {
            f.stress()[off[e]..off[e + 1]]
                .iter()
                .flat_map(|s| (0..6).map(|i| (s[i] - reference[i]).abs() / p).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        }))
    }
    /// ADR 0513 D1: the shaft's uniform pressure of the measured node forces, `mean (1 - d/a) cos(Delta/2)`.
    fn p_shaft(&self) -> f64 {
        self.mean * (1.0 - self.fit.setup.d / A_FIT) * (PI / N_FIT as f64).cos()
    }
    /// ADR 0507 I2's measure: every ring element's centre hoop stress against Lame's with the mean pressure.
    fn hoop_error(&self) -> f64 {
        let m = &self.fit.model;
        let f = IntegrationPointField::compute(m, self.r.result.displacement_values()).unwrap();
        let off = f.offsets();
        worst(self.fit.ring.iter().map(|&e| {
            let s = &f.stress()[off[e]..off[e + 1]];
            let np = s.len() as f64;
            let mut avg = [0.0; 6];
            for p in s {
                for i in 0..6 {
                    avg[i] += p[i] / np;
                }
            }
            let (mut cx, mut cy) = (0.0, 0.0);
            for &v in m.mesh().element_nodes(e) {
                cx += m.mesh().nodes()[v][0] / 8.0;
                cy += m.mesh().nodes()[v][1] / 8.0;
            }
            let rc = (cx * cx + cy * cy).sqrt();
            let (co, si) = (cx / rc, cy / rc);
            let st = avg[0] * si * si - 2.0 * avg[3] * si * co + avg[1] * co * co;
            let lame = self.mean * A_FIT * A_FIT * (1.0 + B_FIT * B_FIT / (rc * rc)) / (B_FIT * B_FIT - A_FIT * A_FIT);
            (st / lame - 1.0).abs()
        }))
    }
    fn penetration(&self) -> f64 {
        self.r.steps[0].solves.last().unwrap().penetration
    }
}

// ------------------------------------------------------------------------------------------------------------ diagnosis

/// DG1, I3ps, I3ps-r and NPS: ADR 0507's plane-stress set-up.
#[test]
fn dg1_i3ps_adr_0507_plane_stress_fit_rows_and_shaft() {
    let s = solve_fit(ADR_0507);
    let mut c = Checks::new();
    let (b, t) = s.row_means();
    c.within("DG1 bottom-row mean p/mean", b, 0.9950, 0.9960);
    c.within("DG1 top-row mean p/mean", t, 1.0040, 1.0050);
    let e = s.shaft_error(s.p_shaft(), 0.0);
    c.at_most("I3ps shaft against D1's reference", e, 1e-2);
    let (bot, top) = s.rows();
    let spread = |v: &[(usize, f64)]| {
        let m = v.iter().map(|x| x.1).sum::<f64>() / v.len() as f64;
        worst(v.iter().map(|x| (x.1 / m - 1.0).abs()))
    };
    c.at_most("I3ps-r around the bottom row", spread(&bot), 1e-3);
    c.at_most("I3ps-r around the top row", spread(&top), 1e-3);
    c.greater(
        "NPS I3pe's measure on the plane-stress set-up (negative control)",
        e,
        1e-6,
    );
    println!("split {:.6e}", s.split());
    c.finish();
}

/// DG2: without Poisson's ratio the variation is absent and D1's reference exact.
#[test]
fn dg2_no_poisson_ratio_uniform() {
    let s = solve_fit(Setup { nu: 0.0, ..ADR_0507 });
    let mut c = Checks::new();
    c.at_most(
        "DG2 every node pressure against the mean",
        worst(s.r.nodes.iter().map(|n| (n.pressure / s.mean - 1.0).abs())),
        1e-6,
    );
    c.at_most(
        "DG2 shaft against D1's reference",
        s.shaft_error(s.p_shaft(), 0.0),
        1e-6,
    );
    c.finish();
}

/// DG3: the split falls with the ring's radial cell at least linearly.
#[test]
fn dg3_radial_refinement() {
    let sp: Vec<f64> = [8, 16, 32]
        .iter()
        .map(|&nr| solve_fit(Setup { nr, ..ADR_0507 }).split())
        .collect();
    println!(
        "DG3 splits at nr 8, 16, 32: {:.6e}, {:.6e}, {:.6e}",
        sp[0], sp[1], sp[2]
    );
    let mut c = Checks::new();
    c.greater("DG3 s_8 - s_16", sp[0] - sp[1], 0.0);
    c.greater("DG3 s_16 - s_32", sp[1] - sp[2], 0.0);
    c.at_most("DG3 s_32", sp[2].abs(), 2e-3);
    c.at_least("DG3 order 8 to 16", (sp[0] / sp[1]).log2(), 1.0);
    c.at_least("DG3 order 16 to 32", (sp[1] / sp[2]).log2(), 1.0);
    c.finish();
}

/// DG4: the split is linear in the interference.
#[test]
fn dg4_linear_in_the_interference() {
    let s3 = solve_fit(ADR_0507).split();
    let s4 = solve_fit(Setup { d: 1e-4, ..ADR_0507 }).split();
    println!("DG4 splits at d 1e-3, 1e-4: {s3:.6e}, {s4:.6e}");
    let mut c = Checks::new();
    c.at_most("DG4 |s(1e-4)/s(1e-3) - 1|", (s4 / s3 - 1.0).abs(), 0.1);
    c.finish();
}

/// DG5: with `u_z` free (three bottom nodes per body) the split nearly vanishes.
#[test]
fn dg5_uz_free() {
    let s = solve_fit(Setup {
        zbc: Zbc::Minimal,
        ..ADR_0507
    });
    let mut c = Checks::new();
    c.at_most("DG5 |s|", s.split().abs(), 1e-3);
    c.finish();
}

/// DG6 and DG7: two layers, the bottom held (the split stays) and the mid-plane held (the rows equal).
#[test]
fn dg6_dg7_two_layers() {
    let mut c = Checks::new();
    let b = solve_fit(Setup { nz: 2, ..ADR_0507 });
    c.at_least("DG6 two layers, bottom held: s", b.split(), 5e-3);
    let m = solve_fit(Setup {
        nz: 2,
        zbc: Zbc::Mid,
        ..ADR_0507
    });
    let (bot, top) = m.rows();
    let n2 = m.fit.n2;
    let ratio = worst(bot.iter().map(|&(v, pb)| {
        let pt = top.iter().find(|x| x.0 == v + 2 * n2).map_or(f64::NAN, |x| x.1);
        (pt / pb - 1.0).abs()
    }));
    c.at_most("DG7 two layers, mid-plane held: |p_top/p_bottom - 1|", ratio, 1e-6);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ I3 re-registered

/// I3pe, I3pe-n, I1pe, I2pe, I4pe and NI3: the plane-strain fit (`u_z = 0` on both faces).
#[test]
fn i3pe_plane_strain_fit_uniform_shaft() {
    let s = solve_fit(Setup {
        zbc: Zbc::Both,
        ..ADR_0507
    });
    let nu = s.fit.setup.nu;
    let d = s.fit.setup.d;
    let mut c = Checks::new();
    let ps = s.p_shaft();
    c.at_most(
        "I3pe shaft against -p_s, -2 nu p_s and 0",
        s.shaft_error(ps, -2.0 * nu * ps),
        1e-6,
    );
    c.at_most(
        "I3pe-n every node pressure against the mean",
        worst(s.r.nodes.iter().map(|n| (n.pressure / s.mean - 1.0).abs())),
        1e-6,
    );
    let p_pe = E_FIT * d * (B_FIT * B_FIT - A_FIT * A_FIT) / (2.0 * A_FIT * B_FIT * B_FIT * (1.0 - nu * nu));
    println!("plane-strain closed form {p_pe:.6e}");
    c.at_most("I1pe mean against p_pe", (s.mean / p_pe - 1.0).abs(), 2e-2);
    c.at_most(
        "I1pe every node against p_pe",
        worst(s.r.nodes.iter().map(|n| (n.pressure / p_pe - 1.0).abs())),
        2e-2,
    );
    c.at_most("I2pe ring hoop stress against Lame", s.hoop_error(), 2e-2);
    c.at_most("I4pe penetration", s.penetration(), 1e-9);
    let ni3 = s.shaft_error(s.mean, -2.0 * nu * s.mean);
    c.at_least("NI3 ADR 0507's reference -mean (negative control)", ni3, 1e-3);
    c.finish();
}
