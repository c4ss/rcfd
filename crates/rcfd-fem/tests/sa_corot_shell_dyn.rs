//! F-585 (ADR 0522): the corotated Shell4 and Shell3 of ADR 0490 in the arc-length continuation, `solve_from` and the
//! nonlinear transient. Checks A1, A2, H0 and the records A1c, A3 (the hinged cylindrical roof through its limit point against
//! the Hex20 reference), F1, F1b, F1c, E1, EZ and their companions (the free vibration of a cantilever strip of shells), SF1
//! (`solve_from` against `solve`), R2 (the refusals kept) and the record P1. Each registered run is made once; every value is
//! printed. Every aggregated error uses a NaN-propagating maximum.

use rcfd_fem::arclength::{trace, ArcLengthOptions, PathPoint};
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nl_transient::{Integrator, NlTransientOptions};
use rcfd_fem::nonlinear::{initial_states, solve, solve_from, solve_in_time, NewtonOptions, TimeSteppingOptions};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;
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

/// The larger of two values, NaN if either is NaN.
fn nmax(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

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

fn kind_of(tri: bool) -> ElementKind {
    if tri {
        ElementKind::Shell3
    } else {
        ElementKind::Shell4
    }
}

// ------------------------------------------------------------------------------------------------ the roof (D5)

const R_R: f64 = 2540.0;
const HALF_LENGTH: f64 = 254.0;
const THETA_0: f64 = 0.1;
const T_R: f64 = 12.7;
const E_R: f64 = 3102.75;
const NU_R: f64 = 0.3;
const P_QUARTER: f64 = 250.0;
const P_0: f64 = 1000.0;

fn roof_material() -> Material {
    Material::isotropic_elastic(E_R, NU_R, 1.0, 0.0).unwrap()
}

/// The quarter roof of Shell4 (or Shell3) `n x n`; returns the model and the apex node.
fn roof_shell(n: usize, tri: bool) -> (SolidModel, usize) {
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
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind_of(tri), conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(T_R).unwrap()).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("sym_x", (0..=n).map(|j| idx(0, j)).collect()).unwrap();
    mesh.add_node_set("sym_y", (0..=n).map(|i| idx(i, 0)).collect()).unwrap();
    mesh.add_node_set("hinge", (0..=n).map(|j| idx(n, j)).collect()).unwrap();
    mesh.add_node_set("apex", vec![idx(0, 0)]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", roof_material()).unwrap();
    for c in [0, 4, 5] {
        m.add_constraint(Constraint::displacement("sym_x", c, 0.0)).unwrap();
    }
    for c in [1, 3, 5] {
        m.add_constraint(Constraint::displacement("sym_y", c, 0.0)).unwrap();
    }
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("hinge", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("apex", [0.0, 0.0, -P_QUARTER])).unwrap();
    (m, idx(0, 0))
}

/// The quarter roof of Hex20 on the Hex8 grid `n x n x 2` of `(s, y, zeta)`; returns the model and the apex node.
fn roof_hex20(n: usize) -> (SolidModel, usize) {
    let grid = hex_box([0.0, 0.0, -0.5 * T_R], [THETA_0, HALF_LENGTH, T_R], [n, n, 2]).unwrap();
    let mesh = upgrade_to_hex20_mapped(&grid, |p| {
        let r = R_R + p[2];
        [r * p[0].sin(), p[1], r * p[0].cos() - R_R]
    })
    .unwrap()
    .0;
    let (hx, hz) = (R_R * THETA_0.sin(), R_R * THETA_0.cos() - R_R);
    let nodes = mesh.nodes().to_vec();
    let pick = |f: &dyn Fn(&[f64; 3]) -> bool| -> Vec<usize> { (0..nodes.len()).filter(|&v| f(&nodes[v])).collect() };
    let sym_x = pick(&|p| p[0] == 0.0);
    let sym_y = pick(&|p| p[1] == 0.0);
    let hinge = pick(&|p| (p[0] - hx).abs() < 1e-9 && (p[2] - hz).abs() < 1e-9);
    let apex = pick(&|p| p[0] == 0.0 && p[1] == 0.0 && p[2].abs() < 1e-9);
    assert_eq!(hinge.len(), 2 * n + 1, "the hinge line of the mid-surface");
    assert_eq!(apex.len(), 1, "one apex node");
    let apex_node = apex[0];
    let mut mesh = mesh;
    mesh.add_node_set("sym_x_f585", sym_x).unwrap();
    mesh.add_node_set("sym_y_f585", sym_y).unwrap();
    mesh.add_node_set("hinge_f585", hinge).unwrap();
    mesh.add_node_set("apex_f585", apex).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", roof_material()).unwrap();
    m.add_constraint(Constraint::displacement("sym_x_f585", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("sym_y_f585", 1, 0.0)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("hinge_f585", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("apex_f585", [0.0, 0.0, -P_QUARTER])).unwrap();
    (m, apex_node)
}

/// What one roof trace gives (D5).
struct Limit {
    p_lim: f64,
    vertex: bool,
    w_at_limit: f64,
    stopped: bool,
    negative_after: bool,
    points: usize,
    halvings: usize,
    last_load_factor: f64,
    seconds: f64,
}

/// The vertex of the parabola through three points, if it is concave.
fn vertex(p: [(f64, f64); 3]) -> Option<(f64, f64)> {
    let [(x0, y0), (x1, y1), (x2, y2)] = p;
    let d01 = (y1 - y0) / (x1 - x0);
    let d12 = (y2 - y1) / (x2 - x1);
    let a = (d12 - d01) / (x2 - x0);
    if !(a < 0.0) {
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
    let pts: Vec<(f64, f64)> = path.points.iter().map(|p| (-p.displacement[qz], p.load_factor)).collect();
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
        last_load_factor: path.points.last().map_or(f64::NAN, |p| p.load_factor),
        seconds: start.elapsed().as_secs_f64(),
    };
    println!(
        "{name}: P_lim {:.6e} N (vertex {}), apex deflection at the limit {:.4e}, {} points, {} halvings, stopped {}, \
         negative pivot after the limit {}, last load factor {:.4e}, {:.1} s",
        l.p_lim, l.vertex, l.w_at_limit, l.points, l.halvings, l.stopped, l.negative_after, l.last_load_factor, l.seconds
    );
    for (i, (p, pp)) in pts.iter().zip(&path.points).enumerate() {
        println!(
            "  {name} point {i}: w {:.6e} P {:.6e} negative pivots {} iterations {}",
            p.0,
            P_0 * p.1,
            pp.negative_pivots,
            pp.residuals.len() - 1
        );
    }
    l
}

fn a2(ck: &mut Checks, name: &str, l: &Limit) {
    ck.holds(
        &format!("A2 {name}: stopped by the rule past the limit, a negative pivot after it"),
        l.stopped && l.negative_after && l.p_lim.is_finite(),
        &format!("stopped {}, negative after {}, P_lim {:e}", l.stopped, l.negative_after, l.p_lim),
    );
}

/// The registered roof run (D5): A1, A2, H0 and the records A1c, A3. Heavy (the Hex20 16 x 16 x 2 reference).
#[test]
#[ignore = "ADR 0522 roof run record: A1 2.53e-2 missed, the point-load solid reference has no mesh limit (amendment 3)"]
fn a1_a2_h0_hinged_roof_through_the_limit_point_known_miss() {
    let mut ck = Checks::new();
    let (m, apex) = roof_shell(16, false);
    let s16 = roof_limit("Shell4 16 x 16", &m, apex);
    let (m, apex) = roof_hex20(16);
    let h16 = roof_limit("Hex20 16 x 16 x 2", &m, apex);
    let (m, apex) = roof_hex20(8);
    let h8 = roof_limit("Hex20 8 x 8 x 2", &m, apex);
    let (m, apex) = roof_shell(8, false);
    let s8 = roof_limit("Shell4 8 x 8", &m, apex);
    let (m, apex) = roof_shell(16, true);
    let t16 = roof_limit("Shell3 16 x 16", &m, apex);
    let p_ref = h16.p_lim;
    ck.record("P_ref (Hex20 16 x 16 x 2), N", p_ref);
    let h0 = (h8.p_lim / p_ref - 1.0).abs();
    ck.at_most("H0 |P_lim(Hex20 8)/P_ref - 1|", if h0.is_finite() { h0 } else { f64::NAN }, 2e-2);
    let a1 = (s16.p_lim / p_ref - 1.0).abs();
    ck.at_most("A1 |P_lim(Shell4 16)/P_ref - 1|", if a1.is_finite() { a1 } else { f64::NAN }, 2e-2);
    for (name, l) in [
        ("Shell4 16", &s16),
        ("Hex20 16", &h16),
        ("Hex20 8", &h8),
        ("Shell4 8", &s8),
        ("Shell3 16", &t16),
    ] {
        a2(&mut ck, name, l);
    }
    ck.record("A1c P_lim(Shell4 8)/P_ref", s8.p_lim / p_ref);
    ck.record("A3 P_lim(Shell3 16)/P_ref", t16.p_lim / p_ref);
    ck.record("A1 signed P_lim(Shell4 16)/P_ref - 1", s16.p_lim / p_ref - 1.0);
    for (name, l) in [
        ("P1 Shell4 16 s", &s16),
        ("P1 Hex20 16 s", &h16),
        ("P1 Hex20 8 s", &h8),
        ("P1 Shell4 8 s", &s8),
        ("P1 Shell3 16 s", &t16),
    ] {
        ck.record(name, l.seconds);
    }
    ck.finish();
}

/// The sum of the `z` components of the consistent nodal load of a model's loads.
fn load_z(m: &SolidModel) -> f64 {
    let f = nodal_load_vector(m, &[]).unwrap();
    let dofs = m.dof_map();
    (0..m.mesh().nodes().len())
        .filter_map(|v| dofs.equation(v, 2))
        .map(|q| f[q])
        .sum()
}

/// A copy of a roof model with its loads replaced by `load(pressure)`, the pressure scaled so that the `z` forces sum to
/// `-P_QUARTER` (amendment 3).
fn with_patch_load(model: &SolidModel, load: &dyn Fn(f64) -> Load) -> SolidModel {
    let build = |p: f64| {
        let mut m = SolidModel::new(model.mesh().clone());
        for (set, mat) in model.materials() {
            m.add_material(set, mat.clone()).unwrap();
        }
        for c in model.constraints() {
            m.add_constraint(c.clone()).unwrap();
        }
        m.add_load(load(p)).unwrap();
        m
    };
    let unit = load_z(&build(1.0));
    assert!(unit != 0.0 && unit.is_finite(), "patch load {unit:e}");
    let m = build(-P_QUARTER / unit);
    let total = load_z(&m);
    assert!((total / -P_QUARTER - 1.0).abs() < 1e-12, "patch total {total:e}");
    m
}

/// The quarter roof of shells under the patch load of amendment 3.
fn roof_shell_patch(n: usize, tri: bool) -> (SolidModel, usize) {
    let (m, apex) = roof_shell(n, tri);
    let k = n / 8;
    let per = if tri { 2 } else { 1 };
    let mut mesh = m.mesh().clone();
    let elements: Vec<usize> = (0..k)
        .flat_map(|j| (0..k).flat_map(move |i| (0..per).map(move |t| per * (j * n + i) + t)))
        .collect();
    mesh.add_element_set("patch_f585", elements).unwrap();
    let mut base = SolidModel::new(mesh);
    for (set, mat) in m.materials() {
        base.add_material(set, mat.clone()).unwrap();
    }
    for c in m.constraints() {
        base.add_constraint(c.clone()).unwrap();
    }
    (with_patch_load(&base, &|p| Load::shell_pressure("patch_f585", p)), apex)
}

/// The quarter roof of Hex20 under the patch load of amendment 3 (the outer faces of the patch).
fn roof_hex20_patch(n: usize) -> (SolidModel, usize) {
    let (m, apex) = roof_hex20(n);
    let mut mesh = m.mesh().clone();
    let (sp, yp) = (THETA_0 / 8.0, HALF_LENGTH / 8.0);
    let faces: Vec<_> = mesh
        .surface_set("zmax")
        .unwrap()
        .iter()
        .copied()
        .filter(|f| {
            let nodes = mesh.element_nodes(f.element);
            let mut c = [0.0; 3];
            for &v in nodes {
                for k in 0..3 {
                    c[k] += mesh.nodes()[v][k] / nodes.len() as f64;
                }
            }
            let s = c[0].atan2(c[2] + R_R);
            s < sp && c[1] < yp
        })
        .collect();
    assert_eq!(faces.len(), (n / 8) * (n / 8), "the patch faces");
    mesh.add_surface_set("patch_f585", faces).unwrap();
    let mut base = SolidModel::new(mesh);
    for (set, mat) in m.materials() {
        base.add_material(set, mat.clone()).unwrap();
    }
    for c in m.constraints() {
        base.add_constraint(c.clone()).unwrap();
    }
    (with_patch_load(&base, &|p| Load::pressure("patch_f585", -p)), apex)
}

/// The corrected roof comparison of amendment 3: A1p, H0p, A2, records. Heavy (the Hex20 16 x 16 x 2 reference).
#[test]
#[ignore = "heavy registered record of ADR 0522 amendment 3 (the patch load); run once with --ignored"]
fn a1p_h0p_roof_patch_load() {
    let mut ck = Checks::new();
    let (m, apex) = roof_shell_patch(16, false);
    let s16 = roof_limit("patch Shell4 16 x 16", &m, apex);
    let (m, apex) = roof_hex20_patch(16);
    let h16 = roof_limit("patch Hex20 16 x 16 x 2", &m, apex);
    let (m, apex) = roof_hex20_patch(8);
    let h8 = roof_limit("patch Hex20 8 x 8 x 2", &m, apex);
    let (m, apex) = roof_shell_patch(8, false);
    let s8 = roof_limit("patch Shell4 8 x 8", &m, apex);
    let p_ref = h16.p_lim;
    ck.record("P_ref,p (Hex20 16 x 16 x 2, patch), N", p_ref);
    let h0 = (h8.p_lim / p_ref - 1.0).abs();
    ck.at_most("H0p |P_lim(Hex20 8)/P_ref,p - 1|", if h0.is_finite() { h0 } else { f64::NAN }, 2e-2);
    let a1 = (s16.p_lim / p_ref - 1.0).abs();
    ck.at_most("A1p |P_lim(Shell4 16)/P_ref,p - 1|", if a1.is_finite() { a1 } else { f64::NAN }, 2e-2);
    for (name, l) in [
        ("patch Shell4 16", &s16),
        ("patch Hex20 16", &h16),
        ("patch Hex20 8", &h8),
        ("patch Shell4 8", &s8),
    ] {
        a2(&mut ck, name, l);
    }
    ck.record("A1p signed P_lim(Shell4 16)/P_ref,p - 1", s16.p_lim / p_ref - 1.0);
    ck.record("patch P_lim(Shell4 8)/P_ref,p", s8.p_lim / p_ref);
    ck.record("patch P_lim(Hex20 8)/P_ref,p", h8.p_lim / p_ref);
    ck.finish();
}

/// The fast roof test (ADR 0522 amendment 1): A2 on the Shell4 8 x 8 and the Hex20 4 x 4 x 2 quarter, their limit loads
/// recorded.
#[test]
fn a2_fast_roof_shell4_and_hex20() {
    let mut ck = Checks::new();
    let (m, apex) = roof_shell(8, false);
    let s8 = roof_limit("fast Shell4 8 x 8", &m, apex);
    a2(&mut ck, "fast Shell4 8", &s8);
    let (m, apex) = roof_hex20(4);
    let h4 = roof_limit("fast Hex20 4 x 4 x 2", &m, apex);
    a2(&mut ck, "fast Hex20 4", &h4);
    ck.record("fast P_lim(Shell4 8), N", s8.p_lim);
    ck.record("fast P_lim(Hex20 4), N", h4.p_lim);
    ck.finish();
}

// ------------------------------------------------------------------------------------------------ the strip (D6)

const N_S: usize = 16;
const L_S: f64 = 12.0;
const B_S: f64 = 1.0;
const T_S: f64 = 0.1;
const E_S: f64 = 1.2e6;
const RHO_S: f64 = 1.0;
const BETA_1: f64 = 1.875_104_068_711_961;

fn ei() -> f64 {
    E_S * B_S * T_S.powi(3) / 12.0
}

/// The cantilever strip, clamped at `x = 0`, nothing else held; the tip node of `y = 0` is node `N_S`.
fn strip(tri: bool) -> SolidModel {
    let mut nodes = Vec::new();
    for j in 0..=1 {
        for i in 0..=N_S {
            nodes.push([L_S * i as f64 / N_S as f64, B_S * j as f64, 0.0]);
        }
    }
    let conn = grid_conn(N_S, 1, tri);
    let ne = conn.len() / if tri { 3 } else { 4 };
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind_of(tri), conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(T_S).unwrap()).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("root", vec![0, N_S + 1]).unwrap();
    mesh.add_node_set("tip", vec![N_S, 2 * N_S + 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_S, 0.0, RHO_S, 0.0).unwrap())
        .unwrap();
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("root", c, 0.0)).unwrap();
    }
    m
}

struct Vibration {
    f1: f64,
    fm: f64,
    r: f64,
    energy_error: f64,
    ez: f64,
    crossings: usize,
    halvings: usize,
    iterations: usize,
    seconds: f64,
}

/// The free vibration of D6 over `periods` periods at 40 steps per period.
fn vibrate(name: &str, tri: bool, periods: u64) -> Vibration {
    let start = Instant::now();
    let m = strip(tri);
    let modal = modal_analysis(&m, &ModalOptions::new(4)).unwrap();
    let f1 = modal.frequencies[0];
    let dofs = m.dof_map();
    let qz = dofs.equation(N_S, 2).expect("tip u_z");
    let phi = &modal.modes[0];
    let a = 1e-3 * L_S;
    let u0: Vec<f64> = phi.iter().map(|x| a * x / phi[qz]).collect();
    let n = u0.len();
    let omega = 2.0 * PI * f1;
    let h = 1.0 / (40.0 * f1);
    let mut it = Integrator::new(&m, &[], &NlTransientOptions::new(h)).unwrap();
    it.start(0.0, &u0, &vec![0.0; n]).unwrap();
    // EZ: the energy at the start against the linear quadratic form.
    let k = stiffness(&m).unwrap();
    let mut ku = vec![0.0; n];
    k.matvec(&u0, &mut ku);
    let quad = 0.5 * u0.iter().zip(&ku).map(|(x, y)| x * y).sum::<f64>();
    let pi0 = it.strain_energy().unwrap();
    let ez = (pi0 / quad - 1.0).abs();
    let h0 = it.kinetic_energy().unwrap() + pi0;
    let mut energy_error = 0.0;
    let mut prev = (0.0, u0[qz]);
    let mut cross = Vec::new();
    let (mut halvings, mut iterations) = (0, 0);
    for step in 1..=40 * periods {
        let rep = it.step().unwrap();
        halvings += rep.halvings;
        iterations += rep.iterations;
        let t = step as f64 * h;
        let uz = it.displacement().unwrap()[qz];
        let hn = it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
        energy_error = nmax(energy_error, ((hn - h0) / h0).abs());
        if (prev.1 > 0.0) != (uz > 0.0) {
            cross.push(prev.0 + h * prev.1 / (prev.1 - uz));
        }
        prev = (t, uz);
    }
    let kc = cross.len();
    let fm = if kc >= 2 {
        (kc - 1) as f64 / (2.0 * (cross[kc - 1] - cross[0]))
    } else {
        f64::NAN
    };
    let r = 2.0 * (0.5 * omega * h).atan() / (omega * h);
    let v = Vibration {
        f1,
        fm,
        r,
        energy_error,
        ez,
        crossings: kc,
        halvings,
        iterations,
        seconds: start.elapsed().as_secs_f64(),
    };
    println!(
        "{name}: f_1 {:.10e} Hz, f_m {:.10e} Hz, r {:.8}, {} crossings, H_0 {:.6e}, max |dH|/H_0 {:.4e}, EZ {:.4e}, \
         {} halvings, {} iterations, {:.1} s",
        v.f1, v.fm, v.r, v.crossings, h0, v.energy_error, v.ez, v.halvings, v.iterations, v.seconds
    );
    v
}

fn vibration_checks(ck: &mut Checks, name: &str, v: &Vibration) {
    ck.at_most(&format!("F1 {name} |f_m/f_1 - 1|"), (v.fm / v.f1 - 1.0).abs(), 1e-2);
    ck.at_most(&format!("F1b {name} |f_m/(f_1 r) - 1|"), (v.fm / (v.f1 * v.r) - 1.0).abs(), 1e-3);
    ck.at_most(&format!("E1 {name} max |H_n - H_0|/H_0"), v.energy_error, 1e-5);
    ck.record(&format!("{name} signed f_m/f_1 - 1"), v.fm / v.f1 - 1.0);
    ck.record(&format!("{name} signed f_m/(f_1 r) - 1"), v.fm / (v.f1 * v.r) - 1.0);
}

fn f_eb() -> f64 {
    BETA_1 * BETA_1 / (2.0 * PI * L_S * L_S) * (ei() / (RHO_S * B_S * T_S)).sqrt()
}

/// The registered 100-period run (D6): F1, F1b, F1c, E1, EZ. Heavy (4000 steps).
#[test]
#[ignore = "superseded before its run by ADR 0522 amendment 2 (the start state, the modal options); not run"]
fn f1_e1_strip_free_vibration_100_periods() {
    let mut ck = Checks::new();
    let v = vibrate("Shell4 strip 100 periods", false, 100);
    vibration_checks(&mut ck, "100 periods", &v);
    ck.at_most("F1c |f_1/f_EB - 1|", (v.f1 / f_eb() - 1.0).abs(), 1e-2);
    ck.record("F1c signed f_1/f_EB - 1", v.f1 / f_eb() - 1.0);
    ck.at_most("EZ |Pi(u_0)/(u_0^T K u_0/2) - 1|", v.ez, 1e-4);
    ck.record("P1 100 periods s", v.seconds);
    ck.finish();
}

/// The companion F1/E1-4 (D6): Shell4, 4 periods; with EZ and F1c.
#[test]
#[ignore = "ADR 0522 run 1 record: the modal solve did not converge (amendment 2); corrected by f1_e1_4_corrected"]
fn f1_e1_4_strip_shell4_four_periods_known_miss() {
    let mut ck = Checks::new();
    let v = vibrate("Shell4 strip 4 periods", false, 4);
    vibration_checks(&mut ck, "Shell4 4 periods", &v);
    ck.at_most("F1c (companion) |f_1/f_EB - 1|", (v.f1 / f_eb() - 1.0).abs(), 1e-2);
    ck.at_most("EZ (companion) |Pi(u_0)/(u_0^T K u_0/2) - 1|", v.ez, 1e-4);
    ck.finish();
}

/// The companion F1/E1-3 (D6): the strip of Shell3, 4 periods.
#[test]
#[ignore = "ADR 0522 run 1 record: E1 4.71e-4 and EZ 2.52e-2 missed, the unrelaxed start (amendment 2); corrected by f1_e1_3_corrected"]
fn f1_e1_3_strip_shell3_four_periods_known_miss() {
    let mut ck = Checks::new();
    let v = vibrate("Shell3 strip 4 periods", true, 4);
    vibration_checks(&mut ck, "Shell3 4 periods", &v);
    ck.at_most("EZ (Shell3) |Pi(u_0)/(u_0^T K u_0/2) - 1|", v.ez, 1e-4);
    ck.finish();
}

// ------------------------------------------------------------------------------- the corrected strip (amendment 2)

/// The tip force of amendment 2 on each tip node (the linear tip deflection `1e-3 L`).
const TIP_FORCE_A2: f64 = -1.042e-3;

/// EZ of amendment 2: the energy at the static equilibrium under the small tip force against the linear quadratic form.
fn ez_at_equilibrium(tri: bool) -> f64 {
    let mut m = strip(tri);
    m.add_load(Load::nodal_force("tip", [0.0, 0.0, TIP_FORCE_A2])).unwrap();
    let r = solve(&m, &[], &[], &NewtonOptions::new(1)).unwrap();
    let us = r.result.displacement_values().to_vec();
    let n = us.len();
    let mut it = Integrator::new(&m, &[], &NlTransientOptions::new(1.0)).unwrap();
    it.start(0.0, &us, &vec![0.0; n]).unwrap();
    let pi = it.strain_energy().unwrap();
    let k = stiffness(&m).unwrap();
    let mut ku = vec![0.0; n];
    k.matvec(&us, &mut ku);
    let quad = 0.5 * us.iter().zip(&ku).map(|(x, y)| x * y).sum::<f64>();
    println!(
        "EZ set-up {}: tip u_z {:.6e}, Pi {:.10e}, u^T K u/2 {:.10e}",
        kind_of(tri).name(),
        us[m.dof_map().equation(N_S, 2).unwrap()],
        pi,
        quad
    );
    (pi / quad - 1.0).abs()
}

/// The free vibration of amendment 2 over `periods` periods at 40 steps per period: from `u_0 = 0` with the velocity of the
/// first mode.
fn vibrate_corrected(name: &str, tri: bool, periods: u64) -> Vibration {
    let start = Instant::now();
    let m = strip(tri);
    let mut mo = ModalOptions::new(1);
    mo.max_iterations = 2000;
    let modal = modal_analysis(&m, &mo).unwrap();
    let f1 = modal.frequencies[0];
    let dofs = m.dof_map();
    let qz = dofs.equation(N_S, 2).expect("tip u_z");
    let phi = &modal.modes[0];
    let a = 1e-3 * L_S;
    let omega = 2.0 * PI * f1;
    let v0: Vec<f64> = phi.iter().map(|x| omega * a * x / phi[qz]).collect();
    let n = v0.len();
    let h = 1.0 / (40.0 * f1);
    let mut it = Integrator::new(&m, &[], &NlTransientOptions::new(h)).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    let h0 = it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
    let mut energy_error = 0.0;
    let mut prev = (0.0, 0.0);
    let mut cross = vec![0.0];
    let (mut halvings, mut iterations) = (0, 0);
    for step in 1..=40 * periods {
        let rep = it.step().unwrap();
        halvings += rep.halvings;
        iterations += rep.iterations;
        let t = step as f64 * h;
        let uz = it.displacement().unwrap()[qz];
        let hn = it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
        energy_error = nmax(energy_error, ((hn - h0) / h0).abs());
        if step > 1 && (prev.1 > 0.0) != (uz > 0.0) {
            cross.push(prev.0 + h * prev.1 / (prev.1 - uz));
        }
        prev = (t, uz);
    }
    let kc = cross.len();
    let fm = if kc >= 2 {
        (kc - 1) as f64 / (2.0 * (cross[kc - 1] - cross[0]))
    } else {
        f64::NAN
    };
    let r = 2.0 * (0.5 * omega * h).atan() / (omega * h);
    let v = Vibration {
        f1,
        fm,
        r,
        energy_error,
        ez: ez_at_equilibrium(tri),
        crossings: kc,
        halvings,
        iterations,
        seconds: start.elapsed().as_secs_f64(),
    };
    println!(
        "{name}: f_1 {:.10e} Hz, f_m {:.10e} Hz, r {:.8}, {} crossings, H_0 {:.6e}, max |dH|/H_0 {:.4e}, EZ {:.4e}, \
         {} halvings, {} iterations, {:.1} s",
        v.f1, v.fm, v.r, v.crossings, h0, v.energy_error, v.ez, v.halvings, v.iterations, v.seconds
    );
    v
}

/// EZ of amendment 4: the energy at the static equilibrium under the small tip force against the work `(1/2) f . u`.
fn ez_clapeyron(tri: bool) -> f64 {
    let mut m = strip(tri);
    m.add_load(Load::nodal_force("tip", [0.0, 0.0, TIP_FORCE_A2])).unwrap();
    let r = solve(&m, &[], &[], &NewtonOptions::new(1)).unwrap();
    let us = r.result.displacement_values().to_vec();
    let n = us.len();
    let mut it = Integrator::new(&m, &[], &NlTransientOptions::new(1.0)).unwrap();
    it.start(0.0, &us, &vec![0.0; n]).unwrap();
    let pi = it.strain_energy().unwrap();
    let f = nodal_load_vector(&m, &[]).unwrap();
    let work = 0.5 * f.iter().zip(&us).map(|(x, y)| x * y).sum::<f64>();
    println!("EZ (amendment 4) {}: Pi {:.10e}, f . u/2 {:.10e}", kind_of(tri).name(), pi, work);
    (pi / work - 1.0).abs()
}

/// The free vibration of amendment 4: as amendment 2, the modal tolerance `1e-8`, EZ by the work of the tip force.
fn vibrate_corrected2(name: &str, tri: bool, periods: u64) -> Vibration {
    let start = Instant::now();
    let m = strip(tri);
    let mut mo = ModalOptions::new(1);
    mo.max_iterations = 2000;
    mo.tolerance = 1e-8;
    let modal = modal_analysis(&m, &mo).unwrap();
    let f1 = modal.frequencies[0];
    let dofs = m.dof_map();
    let qz = dofs.equation(N_S, 2).expect("tip u_z");
    let phi = &modal.modes[0];
    let a = 1e-3 * L_S;
    let omega = 2.0 * PI * f1;
    let v0: Vec<f64> = phi.iter().map(|x| omega * a * x / phi[qz]).collect();
    let n = v0.len();
    let h = 1.0 / (40.0 * f1);
    let mut it = Integrator::new(&m, &[], &NlTransientOptions::new(h)).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    let h0 = it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
    let mut energy_error = 0.0;
    let mut prev = (0.0, 0.0);
    let mut cross = vec![0.0];
    let (mut halvings, mut iterations) = (0, 0);
    for step in 1..=40 * periods {
        let rep = it.step().unwrap();
        halvings += rep.halvings;
        iterations += rep.iterations;
        let t = step as f64 * h;
        let uz = it.displacement().unwrap()[qz];
        let hn = it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
        energy_error = nmax(energy_error, ((hn - h0) / h0).abs());
        if step > 1 && (prev.1 > 0.0) != (uz > 0.0) {
            cross.push(prev.0 + h * prev.1 / (prev.1 - uz));
        }
        prev = (t, uz);
    }
    let kc = cross.len();
    let fm = if kc >= 2 {
        (kc - 1) as f64 / (2.0 * (cross[kc - 1] - cross[0]))
    } else {
        f64::NAN
    };
    let r = 2.0 * (0.5 * omega * h).atan() / (omega * h);
    let v = Vibration {
        f1,
        fm,
        r,
        energy_error,
        ez: ez_clapeyron(tri),
        crossings: kc,
        halvings,
        iterations,
        seconds: start.elapsed().as_secs_f64(),
    };
    println!(
        "{name}: f_1 {:.10e} Hz, f_m {:.10e} Hz, r {:.8}, {} crossings, H_0 {:.6e}, max |dH|/H_0 {:.4e}, EZ {:.4e}, \
         {} halvings, {} iterations, {:.1} s",
        v.f1, v.fm, v.r, v.crossings, h0, v.energy_error, v.ez, v.halvings, v.iterations, v.seconds
    );
    v
}

/// The registered 100-period run (D6, amendment 4): F1, F1b, F1c, E1, EZ. Heavy (4000 steps).
#[test]
#[ignore = "heavy registered record of ADR 0522 D6 and amendment 4 (100 periods); run once with --ignored"]
fn f1_e1_100_periods_corrected2() {
    let mut ck = Checks::new();
    let v = vibrate_corrected2("Shell4 strip 100 periods (amendment 4)", false, 100);
    vibration_checks(&mut ck, "100 periods", &v);
    ck.at_most("F1c |f_1/f_EB - 1|", (v.f1 / f_eb() - 1.0).abs(), 1e-2);
    ck.record("F1c signed f_1/f_EB - 1", v.f1 / f_eb() - 1.0);
    ck.at_most("EZ |Pi(u_s)/(f . u_s/2) - 1|", v.ez, 1e-4);
    ck.record("P1 100 periods s", v.seconds);
    ck.finish();
}

/// The companion F1/E1-4 (D6, amendment 4): Shell4, 4 periods; with EZ and F1c.
#[test]
fn f1_e1_4_corrected2() {
    let mut ck = Checks::new();
    let v = vibrate_corrected2("Shell4 strip 4 periods (amendment 4)", false, 4);
    vibration_checks(&mut ck, "Shell4 4 periods", &v);
    ck.at_most("F1c (companion) |f_1/f_EB - 1|", (v.f1 / f_eb() - 1.0).abs(), 1e-2);
    ck.at_most("EZ (companion) |Pi(u_s)/(f . u_s/2) - 1|", v.ez, 1e-4);
    ck.finish();
}

/// The companion F1/E1-3 (D6, amendment 4): the strip of Shell3, 4 periods.
#[test]
fn f1_e1_3_corrected2() {
    let mut ck = Checks::new();
    let v = vibrate_corrected2("Shell3 strip 4 periods (amendment 4)", true, 4);
    vibration_checks(&mut ck, "Shell3 4 periods", &v);
    ck.at_most("EZ (Shell3) |Pi(u_s)/(f . u_s/2) - 1|", v.ez, 1e-4);
    ck.finish();
}

/// The registered 100-period run (D6, amendment 2): F1, F1b, F1c, E1, EZ. Heavy (4000 steps).
#[test]
#[ignore = "superseded before its run by ADR 0522 amendment 4 (the modal floor, the EZ comparator); not run"]
fn f1_e1_100_periods_corrected() {
    let mut ck = Checks::new();
    let v = vibrate_corrected("Shell4 strip 100 periods (corrected)", false, 100);
    vibration_checks(&mut ck, "100 periods", &v);
    ck.at_most("F1c |f_1/f_EB - 1|", (v.f1 / f_eb() - 1.0).abs(), 1e-2);
    ck.record("F1c signed f_1/f_EB - 1", v.f1 / f_eb() - 1.0);
    ck.at_most("EZ |Pi(u_s)/(u_s^T K u_s/2) - 1|", v.ez, 1e-4);
    ck.record("P1 100 periods s", v.seconds);
    ck.finish();
}

/// The companion F1/E1-4 (D6, amendment 2): Shell4, 4 periods; with EZ and F1c.
#[test]
#[ignore = "ADR 0522 run 2 record: the modal solve stalled at 8.4e-10 (amendment 4); corrected by f1_e1_4_corrected2"]
fn f1_e1_4_corrected_known_miss() {
    let mut ck = Checks::new();
    let v = vibrate_corrected("Shell4 strip 4 periods (corrected)", false, 4);
    vibration_checks(&mut ck, "Shell4 4 periods", &v);
    ck.at_most("F1c (companion) |f_1/f_EB - 1|", (v.f1 / f_eb() - 1.0).abs(), 1e-2);
    ck.at_most("EZ (companion) |Pi(u_s)/(u_s^T K u_s/2) - 1|", v.ez, 1e-4);
    ck.finish();
}

/// The companion F1/E1-3 (D6, amendment 2): the strip of Shell3, 4 periods.
#[test]
#[ignore = "ADR 0522 run 2 record: EZ 2.87e-2 missed, the quadratic form is not the linear limit (amendment 4); corrected by f1_e1_3_corrected2"]
fn f1_e1_3_corrected_known_miss() {
    let mut ck = Checks::new();
    let v = vibrate_corrected("Shell3 strip 4 periods (corrected)", true, 4);
    vibration_checks(&mut ck, "Shell3 4 periods", &v);
    ck.at_most("EZ (Shell3) |Pi(u_s)/(u_s^T K u_s/2) - 1|", v.ez, 1e-4);
    ck.finish();
}

/// SF1 (D7, amendment 2): `solve_from` against `solve` under the small tip force.
#[test]
fn sf1_corrected() {
    let mut ck = Checks::new();
    for tri in [false, true] {
        let mut m = strip(tri);
        m.add_load(Load::nodal_force("tip", [0.0, 0.0, TIP_FORCE_A2])).unwrap();
        let r = solve(&m, &[], &[], &NewtonOptions::new(1)).unwrap();
        let n = m.dof_map().n_equations();
        let f = nodal_load_vector(&m, &[]).unwrap();
        let states = initial_states(&m).unwrap();
        let eq = solve_from(&m, &[], &f, &vec![0.0; n], &states, &NewtonOptions::new(1), &[]).unwrap();
        let u = r.result.displacement_values();
        let same = u.len() == eq.u.len() && u.iter().zip(&eq.u).all(|(a, b)| a.to_bits() == b.to_bits());
        let tip = u[m.dof_map().equation(N_S, 2).unwrap()];
        ck.holds(
            &format!("SF1 {} solve_from = solve bit for bit", kind_of(tri).name()),
            same && r.steps.len() == 1 && r.steps[0].halvings == 0,
            &format!("tip u_z {tip:.6e}, steps {}, iterations {}", r.steps.len(), eq.residuals.len() - 1),
        );
    }
    ck.finish();
}

// ------------------------------------------------------------------------------------------------ SF1, R2 (D7)

#[test]
#[ignore = "ADR 0522 run 1 record: one attempt from zero did not reach the load (amendment 2); corrected by sf1_corrected"]
fn sf1_solve_from_equals_solve_on_shells_known_miss() {
    let mut ck = Checks::new();
    for tri in [false, true] {
        let mut m = strip(tri);
        m.add_load(Load::nodal_force("tip", [0.0, 0.0, -0.1042])).unwrap();
        let r = solve(&m, &[], &[], &NewtonOptions::new(1)).unwrap();
        let n = m.dof_map().n_equations();
        let f = nodal_load_vector(&m, &[]).unwrap();
        let states = initial_states(&m).unwrap();
        let eq = solve_from(&m, &[], &f, &vec![0.0; n], &states, &NewtonOptions::new(1), &[]).unwrap();
        let u = r.result.displacement_values();
        let same = u.len() == eq.u.len() && u.iter().zip(&eq.u).all(|(a, b)| a.to_bits() == b.to_bits());
        let tip = u[m.dof_map().equation(N_S, 2).unwrap()];
        ck.holds(
            &format!("SF1 {} solve_from = solve bit for bit", kind_of(tri).name()),
            same && r.steps.len() == 1 && r.steps[0].halvings == 0,
            &format!("tip u_z {tip:.6e}, steps {}, iterations {}", r.steps.len(), eq.residuals.len() - 1),
        );
    }
    ck.finish();
}

#[test]
fn r2_refusals_kept() {
    let mut ck = Checks::new();
    let m = strip(false);
    let r = Integrator::new(&m, &[], &NlTransientOptions::new(1e-3).energy_momentum());
    let (ok, d) = match &r {
        Ok(_) => (false, "Ok".to_string()),
        Err(e) => (matches!(e, FemError::InvalidValue { key: "nl_transient", .. }), format!("{e:?}")),
    };
    ck.holds("R2 the energy-momentum step refuses Shell4", ok, &d);
    let r = solve_in_time(&m, &[], &[0.0, 1.0], &TimeSteppingOptions::new(f64::INFINITY));
    let (ok, d) = match &r {
        Ok(_) => (false, "Ok".to_string()),
        Err(e) => (
            matches!(
                e,
                FemError::NotYetSupported {
                    item: "element kind",
                    variant: "Shell4",
                    ..
                }
            ),
            format!("{e:?}"),
        ),
    };
    ck.holds("R2 solve_in_time refuses Shell4", ok, &d);
    ck.finish();
}
