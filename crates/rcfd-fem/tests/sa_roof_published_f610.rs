//! F-610 (ADR 0716): the hinged cylindrical roof of ADR 0522 against the published limit load of Sze, Liu and Lo (2004),
//! Table 9a (`t = 12.7`, isotropic): the largest tabulated `P/Pmax = 0.7421` before the snap, `Pmax = 3000`. Checks R0, B1,
//! B2, B3, B5 and the controls NC1, NC2. The models and the limit-load extraction are those of ADR 0522 D5 (copied: a test
//! file cannot import another). Each registered run is made once; every value is printed.

use rcfd_fem::arclength::{trace, ArcLengthOptions, PathPoint};
use rcfd_fem::nonlinear::NewtonOptions;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
use std::time::Instant;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed (a NaN misses).
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
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.4e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not > {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn record(&mut self, name: &str, value: f64) {
        println!("{name}: {value:.6e} (recorded)");
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

// ------------------------------------------------------------------------------------ the published reference (R0)

/// `Pmax` of Sze, Liu and Lo (2004), Figure 9a.
const P_MAX_SZE: f64 = 3000.0;
/// The largest tabulated `P/Pmax` before the snap, Table 9a (`t = 12.7`, isotropic, at `-W_C = 11.293`).
const RATIO_LIMIT_SZE: f64 = 0.7421;
/// The deflection of the loaded point at that tabulated point, Table 9a.
const W_LIMIT_SZE: f64 = 11.293;
/// The vertex of the parabola through the three tabulated points around the limit (ADR 0716 arithmetic 1), a record.
const P_VERTEX_SZE: f64 = 2231.2316;
/// The rising branch of Table 9a below the limit: `(P/Pmax, -W_C)`.
const RISING_SZE: [(f64, f64); 7] = [
    (0.1980, 1.638),
    (0.3473, 3.087),
    (0.4686, 4.477),
    (0.5647, 5.802),
    (0.6381, 7.057),
    (0.6908, 8.237),
    (0.7246, 9.339),
];

/// The registered reference, `P_ref = 0.7421 x 3000 = 2226.3` N.
fn p_ref() -> f64 {
    RATIO_LIMIT_SZE * P_MAX_SZE
}

// ------------------------------------------------------------------------------------------ the roof (ADR 0522 D5)

const R_R: f64 = 2540.0;
const HALF_LENGTH: f64 = 254.0;
const THETA_0: f64 = 0.1;
const T_R: f64 = 12.7;
const E_R: f64 = 3102.75;
const NU_R: f64 = 0.3;
const P_QUARTER: f64 = 250.0;
const P_0: f64 = 1000.0;

fn grid_conn(ni: usize, nj: usize, tri: bool) -> Vec<usize> {
    let idx = |i: usize, j: usize| j * (ni + 1) + i;
    let mut conn = Vec::new();
    for j in 0..nj {
        for i in 0..ni {
            let (a, b, c, d) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
            if tri {
                conn.extend_from_slice(&[a, b, c, a, c, d]);
            } else {
                conn.extend_from_slice(&[a, b, c, d]);
            }
        }
    }
    conn
}

/// The quarter roof of Shell4 (or Shell3) `n x n` with Young's modulus `e`; `clamped` also holds the hinge rotations
/// (NC2). Returns the model and the apex node.
fn roof_shell(n: usize, tri: bool, e: f64, clamped: bool) -> (SolidModel, usize) {
    let mut nodes = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            let s = THETA_0 * i as f64 / n as f64;
            let y = HALF_LENGTH * j as f64 / n as f64;
            nodes.push([R_R * s.sin(), y, R_R * s.cos() - R_R]);
        }
    }
    let idx = |i: usize, j: usize| j * (n + 1) + i;
    let conn = grid_conn(n, n, tri);
    let ne = conn.len() / if tri { 3 } else { 4 };
    let kind = if tri { ElementKind::Shell3 } else { ElementKind::Shell4 };
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(T_R).unwrap()).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("sym_x", (0..=n).map(|j| idx(0, j)).collect())
        .unwrap();
    mesh.add_node_set("sym_y", (0..=n).map(|i| idx(i, 0)).collect())
        .unwrap();
    mesh.add_node_set("hinge", (0..=n).map(|j| idx(n, j)).collect())
        .unwrap();
    mesh.add_node_set("apex", vec![idx(0, 0)]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(e, NU_R, 1.0, 0.0).unwrap())
        .unwrap();
    for c in [0, 4, 5] {
        m.add_constraint(Constraint::displacement("sym_x", c, 0.0)).unwrap();
    }
    for c in [1, 3, 5] {
        m.add_constraint(Constraint::displacement("sym_y", c, 0.0)).unwrap();
    }
    let held = if clamped { 6 } else { 3 };
    for c in 0..held {
        m.add_constraint(Constraint::displacement("hinge", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("apex", [0.0, 0.0, -P_QUARTER])).unwrap();
    (m, idx(0, 0))
}

/// What one roof trace gives (ADR 0522 D5).
struct Limit {
    p_lim: f64,
    vertex: bool,
    w_at_limit: f64,
    stopped: bool,
    negative_after: bool,
    points: usize,
    halvings: usize,
    seconds: f64,
    /// `(w, P)` of every point, `P` in N for the whole roof.
    path: Vec<(f64, f64)>,
}

/// The vertex of the parabola through three points, if it is concave.
fn vertex(p: [(f64, f64); 3]) -> Option<(f64, f64)> {
    let [(x0, y0), (x1, y1), (x2, y2)] = p;
    let d01 = (y1 - y0) / (x1 - x0);
    let d12 = (y2 - y1) / (x2 - x1);
    let a = (d12 - d01) / (x2 - x0);
    if a.is_nan() || a >= 0.0 {
        return None;
    }
    let b = d01 - a * (x0 + x1);
    let xv = -b / (2.0 * a);
    let yv = y0 + d01 * (xv - x0) + a * (xv - x0) * (xv - x1);
    Some((xv, yv))
}

fn roof_limit(name: &str, model: &SolidModel, apex: usize) -> Limit {
    let start = Instant::now();
    let lin = solve_static(model, &StaticOptions::default()).unwrap();
    let c: f64 = lin.displacement_values().iter().map(|x| x * x).sum();
    assert!(c > 0.0 && c.is_finite(), "{name}: load scale {c:e}");
    let options = ArcLengthOptions::new(0.03 * (2.0 * c).sqrt())
        .load_scale(c)
        .max_points(300)
        .newton(NewtonOptions::new(1));
    let mut lam_max = f64::NEG_INFINITY;
    let mut stop = |p: &PathPoint| {
        let past = lam_max > 0.0 && p.load_factor <= 0.8 * lam_max;
        lam_max = lam_max.max(p.load_factor);
        past
    };
    let path = trace(model, &[], &[], &options, &[], &mut stop).unwrap();
    let qz = model.dof_map().equation(apex, 2).expect("apex u_z");
    let pts: Vec<(f64, f64)> = path
        .points
        .iter()
        .map(|p| (-p.displacement[qz], p.load_factor))
        .collect();
    let mut imax = 0;
    for (i, p) in pts.iter().enumerate() {
        if p.1.is_nan() || p.1 > pts[imax].1 {
            imax = i;
        }
    }
    let fit = if imax >= 1 && imax + 1 < pts.len() {
        vertex([pts[imax - 1], pts[imax], pts[imax + 1]])
    } else {
        None
    };
    let (w_at, lam) = fit.unwrap_or(pts[imax]);
    let negative_after = path.points[imax + 1..].iter().any(|p| p.negative_pivots >= 1);
    let l = Limit {
        p_lim: P_0 * lam,
        vertex: fit.is_some(),
        w_at_limit: w_at,
        stopped: path.stopped,
        negative_after,
        points: path.points.len(),
        halvings: path.points.iter().map(|p| p.halvings).sum(),
        seconds: start.elapsed().as_secs_f64(),
        path: pts.iter().map(|&(w, lam)| (w, P_0 * lam)).collect(),
    };
    println!(
        "{name}: P_lim {:.6e} N (vertex {}), apex deflection at the limit {:.4e}, {} points, {} halvings, stopped {}, \
         negative pivot after the limit {}, {:.1} s",
        l.p_lim, l.vertex, l.w_at_limit, l.points, l.halvings, l.stopped, l.negative_after, l.seconds
    );
    for (i, (p, pp)) in pts.iter().zip(&path.points).enumerate() {
        println!(
            "  {name} point {i}: w {:.6e} P {:.6e} negative pivots {}",
            p.0,
            P_0 * p.1,
            pp.negative_pivots
        );
    }
    l
}

/// The deflection of the rising branch at the load `p` (N), by linear interpolation between the first two points that
/// bracket it; NaN if none does.
fn w_at_load(l: &Limit, p: f64) -> f64 {
    for k in 1..l.path.len() {
        let (w0, p0) = l.path[k - 1];
        let (w1, p1) = l.path[k];
        if p1 < p0 {
            break;
        }
        if p0 <= p && p <= p1 {
            return w0 + (w1 - w0) * (p - p0) / (p1 - p0);
        }
    }
    f64::NAN
}

/// The relative error of a limit load against `P_ref`, NaN if not finite.
fn rel(p: f64) -> f64 {
    let r = (p / p_ref() - 1.0).abs();
    if r.is_finite() {
        r
    } else {
        f64::NAN
    }
}

fn b3(ck: &mut Checks, name: &str, l: &Limit) {
    ck.holds(
        &format!("B3 {name}: stopped by the rule past the limit, a negative pivot after it"),
        l.stopped && l.negative_after && l.p_lim.is_finite(),
        &format!(
            "stopped {}, negative after {}, P_lim {:e}",
            l.stopped, l.negative_after, l.p_lim
        ),
    );
}

fn records(ck: &mut Checks, name: &str, l: &Limit) {
    ck.record(&format!("{name}: P_lim, N"), l.p_lim);
    ck.record(&format!("{name}: P_lim/P_ref - 1 (signed)"), l.p_lim / p_ref() - 1.0);
    ck.record(
        &format!("{name}: P_lim/P_vertex(Sze) - 1"),
        l.p_lim / P_VERTEX_SZE - 1.0,
    );
    ck.record(&format!("{name}: apex deflection at the limit"), l.w_at_limit);
    ck.record(
        &format!("{name}: deflection at the limit / Table 9a 11.293 - 1"),
        l.w_at_limit / W_LIMIT_SZE - 1.0,
    );
    for (ratio, w_sze) in RISING_SZE {
        let w = w_at_load(l, ratio * P_MAX_SZE);
        ck.record(
            &format!("{name}: w at P/Pmax {ratio} / Table 9a {w_sze} - 1"),
            w / w_sze - 1.0,
        );
    }
    ck.record(&format!("{name}: points"), l.points as f64);
    ck.record(&format!("{name}: halvings"), l.halvings as f64);
    ck.record(&format!("{name}: wall time, s"), l.seconds);
}

fn r0(ck: &mut Checks) {
    ck.holds(
        "R0 P_ref = 0.7421 x 3000 = 2226.3 N",
        (p_ref() - 2226.3).abs() < 1e-9,
        &format!("P_ref {:.4}", p_ref()),
    );
}

/// The registered comparison (B1, B2, B3): Shell4 and Shell3 16 x 16. The heavy record, run once with --ignored.
#[test]
#[ignore = "ADR 0716 registered record (B1, B2: Shell4 and Shell3 16 x 16 against Sze et al. 2004); run once with --ignored"]
fn b1_b2_roof_16_against_sze_2004() {
    let mut ck = Checks::new();
    r0(&mut ck);
    let (m, apex) = roof_shell(16, false, E_R, false);
    let s16 = roof_limit("Shell4 16 x 16", &m, apex);
    let (m, apex) = roof_shell(16, true, E_R, false);
    let t16 = roof_limit("Shell3 16 x 16", &m, apex);
    ck.at_most("B1 |P_lim(Shell4 16)/P_ref - 1|", rel(s16.p_lim), 2e-2);
    ck.at_most("B2 |P_lim(Shell3 16)/P_ref - 1|", rel(t16.p_lim), 2e-2);
    b3(&mut ck, "Shell4 16", &s16);
    b3(&mut ck, "Shell3 16", &t16);
    records(&mut ck, "Shell4 16", &s16);
    records(&mut ck, "Shell3 16", &t16);
    ck.finish();
}

/// The fast test (B5, B3): Shell4 and Shell3 8 x 8 against the same reference and limit.
#[test]
fn b5_fast_roof_8_against_sze_2004() {
    let mut ck = Checks::new();
    r0(&mut ck);
    let (m, apex) = roof_shell(8, false, E_R, false);
    let s8 = roof_limit("Shell4 8 x 8", &m, apex);
    let (m, apex) = roof_shell(8, true, E_R, false);
    let t8 = roof_limit("Shell3 8 x 8", &m, apex);
    ck.at_most("B5 |P_lim(Shell4 8)/P_ref - 1|", rel(s8.p_lim), 2e-2);
    ck.at_most("B5 |P_lim(Shell3 8)/P_ref - 1|", rel(t8.p_lim), 2e-2);
    b3(&mut ck, "Shell4 8", &s8);
    b3(&mut ck, "Shell3 8", &t8);
    records(&mut ck, "Shell4 8", &s8);
    records(&mut ck, "Shell3 8", &t8);
    ck.finish();
}

/// The negative controls: NC1 (Young's modulus times 1.05) and NC2 (clamped straight edges) on Shell4 8 x 8.
#[test]
fn nc_roof_negative_controls() {
    let mut ck = Checks::new();
    let (m, apex) = roof_shell(8, false, E_R, false);
    let base = roof_limit("NC base Shell4 8 x 8", &m, apex);
    let (m, apex) = roof_shell(8, false, 1.05 * E_R, false);
    let stiff = roof_limit("NC1 Shell4 8 x 8, E x 1.05", &m, apex);
    ck.above("NC1 |P_lim(1.05 E)/P_ref - 1|", rel(stiff.p_lim), 2e-2);
    let scale = (stiff.p_lim / (1.05 * base.p_lim) - 1.0).abs();
    ck.at_most(
        "NC1 |P_lim(1.05 E)/(1.05 P_lim(E)) - 1|",
        if scale.is_finite() { scale } else { f64::NAN },
        1e-3,
    );
    let (m, apex) = roof_shell(8, false, E_R, true);
    let clamped = roof_limit("NC2 Shell4 8 x 8, clamped", &m, apex);
    ck.above("NC2 |P_lim(clamped)/P_ref - 1|", rel(clamped.p_lim), 2e-2);
    ck.record("NC2 P_lim(clamped), N", clamped.p_lim);
    ck.record(
        "NC2 stopped by the rule (1) or the cap (0)",
        f64::from(u8::from(clamped.stopped)),
    );
    ck.record("NC2 vertex fit used (1)", f64::from(u8::from(clamped.vertex)));
    ck.finish();
}
