//! F-602 (ADR 0518): the partly plastic thick sphere of ADR 0475 on a 4 x 4-per-face cubed-sphere octant with 16 radial Hex20
//! layers: the residual hoop stress at the bore after unloading from p(1.25 a), the elastic Lame hoop stress at every
//! integration point of the inner layer, S1 and S2 of ADR 0475; the 8-layer misses of ADR 0512 as the negative control N1.
//! The generator and the readings are copied from `sa_j2_f561.rs` (ADR 0512).

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::element_dofs;
use rcfd_fem::element::element_for;
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mat_j2;
use rcfd_fem::mesh::FaceRef;
use rcfd_fem::nonlinear::{
    element_second_piola_kirchhoff, solve, solve_with, Contribution, DriverExtensions, NewtonOptions, NonlinearResult,
};
use rcfd_fem::{Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
use std::collections::HashMap;

const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const SY: f64 = 2.0e8;
const RHO: f64 = 7850.0;
/// The layer count of the item; the controls of ADR 0512 use 8.
const LAYERS: usize = 16;
const OLD_LAYERS: usize = 8;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
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
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {} {detail}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

/// The largest of the values; a NaN or an infinity among them is an error, not skipped.
fn max_finite(name: &str, v: impl Iterator<Item = f64>) -> f64 {
    let mut m = f64::NEG_INFINITY;
    for x in v {
        assert!(x.is_finite(), "{name}: a value is not finite");
        m = m.max(x);
    }
    assert!(m.is_finite(), "{name}: no value");
    m
}

fn base() -> Material {
    Material::isotropic_elastic(E_MOD, NU, RHO, 0.0).unwrap()
}

/// Perfectly plastic J2 (`sy = SY`) or the never-yielding control (`sy = 1e30`).
fn j2(sy: f64) -> Material {
    Material::j2(base(), sy, 0.0, 0.0).unwrap()
}

/// The bore pressure of the closed form for the plastic radius `c` (`a = 1`, `b = 2`), in units of `sigma_y`.
fn p_of_c(c: f64) -> f64 {
    2.0 * c.ln() + 2.0 / 3.0 * (1.0 - c * c * c / 8.0)
}

fn per_layer(n: usize) -> usize {
    3 * n * n
}

/// The octant of the sphere `a = 1`, `b = 2`: a cubed sphere (the faces `x = 1`, `y = 1`, `z = 1` of the unit cube, each
/// `n x n` cells, carried to the radius by `p -> p |p|_inf / |p|_2`), 8 radial layers, Hex20 on the spheres, element
/// `layer * 3 n^2 + cell`; the bore pressure `p` (Pa) if given.
fn sphere(n: usize, layers: usize, mat: Material, p: Option<f64>) -> SolidModel {
    let m = layers;
    let mut index: HashMap<([usize; 3], usize), usize> = HashMap::new();
    let mut params: Vec<Vec3> = Vec::new();
    let mut node = |q: [usize; 3], k: usize, params: &mut Vec<Vec3>| -> usize {
        *index.entry((q, k)).or_insert_with(|| {
            let r = 1.0 + k as f64 / m as f64;
            params.push([
                r * q[0] as f64 / n as f64,
                r * q[1] as f64 / n as f64,
                r * q[2] as f64 / n as f64,
            ]);
            params.len() - 1
        })
    };
    let mut conn = Vec::new();
    for k in 0..m {
        for a in 0..3 {
            for j in 0..n {
                for i in 0..n {
                    let q = |i: usize, j: usize| {
                        let mut v = [0; 3];
                        v[a] = n;
                        v[(a + 1) % 3] = i;
                        v[(a + 2) % 3] = j;
                        v
                    };
                    for kk in [k, k + 1] {
                        for (ii, jj) in [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)] {
                            let id = node(q(ii, jj), kk, &mut params);
                            conn.push(id);
                        }
                    }
                }
            }
        }
    }
    let nn = params.len();
    let mut mesh = SolidMesh::new(params).unwrap();
    mesh.add_block(ElementKind::Hex8, conn).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    for (c, name) in ["x0", "y0", "z0"].iter().enumerate() {
        let set: Vec<usize> = (0..nn).filter(|&v| mesh.nodes()[v][c] == 0.0).collect();
        mesh.add_node_set(name, set).unwrap();
    }
    mesh.add_surface_set(
        "bore",
        (0..per_layer(n)).map(|e| FaceRef { element: e, face: 0 }).collect(),
    )
    .unwrap();
    let mesh = upgrade_to_hex20_mapped(&mesh, |p| {
        let inf = p[0].abs().max(p[1].abs()).max(p[2].abs());
        let two = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        [p[0] * inf / two, p[1] * inf / two, p[2] * inf / two]
    })
    .unwrap()
    .0;
    let mut model = SolidModel::new(mesh);
    model.add_material("all", mat).unwrap();
    for (c, name) in ["x0", "y0", "z0"].iter().enumerate() {
        model.add_constraint(Constraint::displacement(name, c, 0.0)).unwrap();
    }
    if let Some(p) = p {
        model.add_load(Load::pressure("bore", p)).unwrap();
    }
    model
}

/// The element coordinates and displacement of element `e`.
fn element_data(model: &SolidModel, r: &NonlinearResult, e: usize) -> (Vec<Vec3>, Vec<f64>) {
    let mesh = model.mesh();
    let nodes = mesh.element_nodes(e);
    let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
    let d = element_dofs(&model.dof_map(), nodes);
    let u = r.result.displacement_values();
    (coords, d.iter().map(|&q| u[q]).collect())
}

/// One integration point: reference radius, von Mises stress, hoop stress, equivalent plastic strain.
struct Point {
    r: f64,
    eq: f64,
    hoop: f64,
    alpha: f64,
}

/// The stresses of the integration points (27 per Hex20 element) of the elements `< elements`, in element order.
fn sphere_points(model: &SolidModel, r: &NonlinearResult, elements: usize) -> Vec<Point> {
    let mat = &model.materials()[0].1;
    let el = element_for(ElementKind::Hex20);
    let mut out = Vec::new();
    for e in 0..elements {
        let (x, ue) = element_data(model, r, e);
        let s = element_second_piola_kirchhoff(el, &x, mat, &ue, &r.states[e]).unwrap();
        let ips = el.integration_points(&x, el.stiffness_rule()).unwrap();
        for (q, (ip, s)) in ips.iter().zip(&s).enumerate() {
            let p = ip.geometry.x;
            let rad = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            let n = [p[0] / rad, p[1] / rad, p[2] / rad];
            let t = [[s[0], s[3], s[5]], [s[3], s[1], s[4]], [s[5], s[4], s[2]]];
            let mut srr = 0.0;
            for i in 0..3 {
                for j in 0..3 {
                    srr += n[i] * t[i][j] * n[j];
                }
            }
            let tr = s[0] + s[1] + s[2];
            let mean = tr / 3.0;
            let dev2 = (s[0] - mean).powi(2)
                + (s[1] - mean).powi(2)
                + (s[2] - mean).powi(2)
                + 2.0 * (s[3] * s[3] + s[4] * s[4] + s[5] * s[5]);
            out.push(Point {
                r: rad,
                eq: (1.5 * dev2).sqrt(),
                hoop: 0.5 * (tr - srr),
                alpha: r.states[e].values()[q * mat_j2::STATE_SIZE + 12],
            });
        }
    }
    out
}

/// The bore pressure as a driver contribution `-g(lambda) f1`; `g = 1 - |2 lambda - 1|` (load and unload) or `g = lambda`
/// (load only).
struct Ramp {
    f1: Vec<f64>,
    up_only: bool,
}

impl Contribution for Ramp {
    fn name(&self) -> &str {
        "bore pressure, loaded and unloaded"
    }
    fn add(
        &self,
        _u: &[f64],
        load_factor: f64,
        force: &mut [f64],
        _tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        let g = if self.up_only {
            load_factor
        } else {
            1.0 - (2.0 * load_factor - 1.0).abs()
        };
        for (f, v) in force.iter_mut().zip(&self.f1) {
            *f = -g * v;
        }
        Ok(())
    }
}

/// The ramp solve of the registration (16 steps, 8 up and 8 down; or 8 steps up only) of `mat` from `p1`.
fn ramp_solve(n: usize, layers: usize, mat: Material, p1: f64, up_only: bool) -> (SolidModel, NonlinearResult) {
    let f1 = nodal_load_vector(&sphere(n, layers, mat.clone(), Some(p1)), &[]).unwrap();
    let model = sphere(n, layers, mat, None);
    let ramp = Ramp { f1, up_only };
    let ext = DriverExtensions {
        contributions: vec![&ramp],
        step_control: None,
    };
    let steps = if up_only { 8 } else { 16 };
    let r = solve_with(&model, &[], &[], &NewtonOptions::new(steps), ext).unwrap();
    (model, r)
}

/// Least squares `hoop = c0 + c1 t + c2 t^2`, `t = r - 1.125`, over the points, evaluated at `r = at`.
fn fit_at(points: &[Point], at: f64) -> f64 {
    let mut a = [[0.0; 3]; 3];
    let mut b = [0.0; 3];
    for q in points {
        let t = q.r - 1.125;
        let phi = [1.0, t, t * t];
        for i in 0..3 {
            for j in 0..3 {
                a[i][j] += phi[i] * phi[j];
            }
            b[i] += phi[i] * q.hoop;
        }
    }
    for col in 0..3 {
        for row in col + 1..3 {
            let f = a[row][col] / a[col][col];
            for k in col..3 {
                a[row][k] -= f * a[col][k];
            }
            b[row] -= f * b[col];
        }
    }
    let mut x = [0.0; 3];
    for row in (0..3).rev() {
        let mut v = b[row];
        for k in row + 1..3 {
            v -= a[row][k] * x[k];
        }
        x[row] = v / a[row][row];
    }
    let t = at - 1.125;
    x[0] + x[1] * t + x[2] * t * t
}

/// The elastic Lame hoop stress `(p/7)(1 + 4/r^3)` of the sphere `a = 1`, `b = 2`.
fn lame_hoop(p: f64, r: f64) -> f64 {
    p / 7.0 * (1.0 + 4.0 / (r * r * r))
}

/// The closed residual hoop field after the elastic unloading from `p1` (plastic zone of the loading).
fn residual_hoop(p1: f64, r: f64) -> f64 {
    SY - p1 + 2.0 * SY * r.ln() - lame_hoop(p1, r)
}

/// R1: (the residual hoop stress at the bore of the fit, the closed `sigma_y - 12/7 p1`, the points) of the unloading run.
fn residual_run(n: usize, layers: usize, mat: Material) -> (f64, Vec<Point>, usize) {
    let p1 = p_of_c(1.25) * SY;
    let (model, r) = ramp_solve(n, layers, mat, p1, false);
    let pts: Vec<Point> = sphere_points(&model, &r, per_layer(n));
    (fit_at(&pts, 1.0), pts, r.steps.len())
}

/// E1: (largest, mean) relative hoop error against Lame over the inner layer of the elastic material under `p1`, and the
/// elastic fit at the bore.
fn elastic_run(n: usize, layers: usize) -> (f64, f64, f64) {
    let p1 = p_of_c(1.25) * SY;
    let model = sphere(n, layers, j2(1e30), Some(p1));
    let r = solve(&model, &[], &[], &NewtonOptions::new(1)).unwrap();
    let pts = sphere_points(&model, &r, per_layer(n));
    let errs: Vec<f64> = pts
        .iter()
        .map(|q| {
            let l = lame_hoop(p1, q.r);
            (q.hoop - l).abs() / l
        })
        .collect();
    let worst = max_finite("E1", errs.iter().copied());
    let mean = errs.iter().sum::<f64>() / errs.len() as f64;
    assert!(mean.is_finite());
    (worst, mean, fit_at(&pts, 1.0))
}

/// S1 or S2: the loading to `p(c)` in `steps` steps: (`|p(c_FE) - p| / p`, points wrong for C1, points).
fn s_run(n: usize, layers: usize, c: f64, steps: usize) -> (f64, usize, usize) {
    let p = p_of_c(c) * SY;
    let model = sphere(n, layers, j2(SY), Some(p));
    let r = solve(&model, &[], &[], &NewtonOptions::new(steps)).unwrap();
    let pts = sphere_points(&model, &r, model.mesh().n_elements());
    let elastic: Vec<&Point> = pts.iter().filter(|q| q.alpha == 0.0).collect();
    let c_fe = elastic.iter().map(|q| q.r * (q.eq / SY).cbrt()).sum::<f64>() / elastic.len() as f64;
    let err = (p_of_c(c_fe) * SY - p).abs() / p;
    let wrong = pts
        .iter()
        .filter(|q| (q.r < c - 0.05 && q.alpha == 0.0) || (q.r > c + 0.05 && q.alpha > 0.0))
        .count();
    (err, wrong, pts.len())
}

fn volume(model: &SolidModel) -> f64 {
    let el = element_for(ElementKind::Hex20);
    let mut v = 0.0;
    for e in 0..model.mesh().n_elements() {
        let x: Vec<Vec3> = model
            .mesh()
            .element_nodes(e)
            .iter()
            .map(|&q| model.mesh().nodes()[q])
            .collect();
        for ip in el.integration_points(&x, el.stiffness_rule()).unwrap() {
            v += ip.weight;
        }
    }
    v
}

/// The registered run of ADR 0518: M1, R1, E1, S1, S2, C1, F1 on 16 layers; N1 to N5.
#[test]
#[ignore = "heavy record of the registered run of ADR 0518 (155 s in release, made once from commit of this file before the split): every check met (R1 4.524e-3, E1 9.296e-3, S1 2.75e-4, S2 1.148e-3, C1 0 wrong of 20736 each, N1 8 layers -0.611919 / 2.7005e-2 / 1.1623e-2, N2 0.676, N3 2.4e-182, N4 0, N5 2.891e-2); the fast rows are in f602_fast_rows"]
fn f602_registered_run() {
    let t0 = std::time::Instant::now();
    let mut c = Checks::new();
    let p1 = p_of_c(1.25) * SY;
    let closed = (SY - 12.0 / 7.0 * p1) / SY;
    println!(
        "p1 = {:.7} sigma_y, closed residual at the bore {closed:.6} sigma_y",
        p1 / SY
    );

    // M1
    let model = sphere(4, LAYERS, j2(SY), None);
    c.holds(
        "M1 counts: 768 elements, 3849 nodes",
        model.mesh().n_elements() == 768 && model.mesh().nodes().len() == 3849,
        &format!(
            "{} elements, {} nodes",
            model.mesh().n_elements(),
            model.mesh().nodes().len()
        ),
    );
    let exact_volume = 7.0 * std::f64::consts::PI / 6.0;
    c.at_most(
        "M1 octant volume vs 7 pi/6",
        (volume(&model) - exact_volume).abs() / exact_volume,
        5e-4,
    );

    // R1 and the diagnostics
    let (bore, pts, steps) = residual_run(4, LAYERS, j2(SY));
    let bore = bore / SY;
    let r1 = (bore - closed).abs() / closed.abs();
    println!(
        "  R1: {steps} increments, {} points, residual hoop at the bore {bore:.6} sigma_y, closed {closed:.6} sigma_y",
        pts.len()
    );
    c.holds("R1 point count", pts.len() == 1296, &format!("{} points", pts.len()));
    c.at_most(
        "R1 residual hoop stress at the bore: |FE - closed| / |closed|",
        r1,
        2e-2,
    );
    let pointwise = max_finite(
        "pointwise residual",
        pts.iter().map(|q| (q.hoop - residual_hoop(p1, q.r)).abs() / SY),
    );
    let inner_alpha = max_finite("alpha", pts.iter().map(|q| q.alpha));
    println!("  diagnostic: largest pointwise |hoop - closed residual field| / sigma_y {pointwise:.4e}, largest alpha {inner_alpha:.3e}");

    // N2
    c.at_least(
        "N2 |FE residual - (sigma_y - p1)| / sigma_y (no unloading)",
        (bore - (1.0 - p1 / SY)).abs(),
        0.5,
    );

    // N4 and the loaded fit
    let (up_model, up) = ramp_solve(4, LAYERS, j2(SY), p1, true);
    let up_pts = sphere_points(&up_model, &up, per_layer(4));
    let dalpha = max_finite(
        "alpha difference",
        pts.iter().zip(&up_pts).map(|(a, b)| (a.alpha - b.alpha).abs()),
    );
    c.at_most(
        "N4 largest |alpha(unloaded) - alpha(loaded)| over the inner layer",
        dalpha,
        1e-12,
    );
    println!(
        "  diagnostic: loaded fit at the bore {:.6} sigma_y, closed {:.6}",
        fit_at(&up_pts, 1.0) / SY,
        1.0 - p1 / SY
    );

    // N3
    let (_, el_pts, _) = residual_run(4, LAYERS, j2(1e30));
    let left = max_finite("N3", el_pts.iter().map(|q| q.hoop.abs())) / SY;
    c.at_most(
        "N3 elastic material through the Ramp: largest |hoop| of the inner layer / sigma_y",
        left,
        1e-6,
    );

    // E1
    let (worst, mean, el_fit) = elastic_run(4, LAYERS);
    println!(
        "  diagnostic: E1 mean {mean:.4e}, elastic fit at the bore {:.6} p1 (closed {:.6})",
        el_fit / p1,
        5.0 / 7.0
    );
    c.at_most(
        "E1 elastic hoop vs Lame, largest relative error over the inner layer",
        worst,
        1e-2,
    );

    // S1, S2, C1
    for (id, cc, steps) in [("S1", 1.25, 8), ("S2", 1.5, 10)] {
        let (err, wrong, total) = s_run(4, LAYERS, cc, steps);
        c.at_most(&format!("{id} c = {cc} a: |p(c_FE) - p| / p"), err, 1e-2);
        c.holds(
            &format!("C1 {id}: points wrong"),
            wrong == 0,
            &format!("{wrong} wrong of {total}"),
        );
    }

    // N1: the 8-layer misses of ADR 0512
    let (bore8, _, _) = residual_run(4, OLD_LAYERS, j2(SY));
    let bore8 = bore8 / SY;
    let r1_8 = (bore8 - closed).abs() / closed.abs();
    println!("  N1: 8 layers residual at the bore {bore8:.6} sigma_y, R1 error {r1_8:.4e}");
    c.at_most(
        "N1 8 layers reproduces ADR 0512 (-0.611919): |FE - (-0.611919)|",
        (bore8 + 0.611919).abs(),
        2e-5,
    );
    c.at_least("N1 8 layers R1 error is above the limit", r1_8, 2e-2);
    let (worst8, _, _) = elastic_run(4, OLD_LAYERS);
    c.at_least("N1 8 layers E1 error is above the limit", worst8, 1e-2);

    // N5
    let (worst2, _, _) = elastic_run(2, LAYERS);
    c.at_least("N5 n = 2, 16 layers: E1 error is above the limit", worst2, 1e-2);

    println!("  elapsed {:.1} s", t0.elapsed().as_secs_f64());
    c.finish();
}

/// The fast rows of the registered run (the same code paths: mesh, elastic solve, reading): M1, E1 and N5.
#[test]
fn f602_fast_rows() {
    let mut c = Checks::new();
    let model = sphere(4, LAYERS, j2(SY), None);
    c.holds(
        "M1 counts: 768 elements, 3849 nodes",
        model.mesh().n_elements() == 768 && model.mesh().nodes().len() == 3849,
        &format!(
            "{} elements, {} nodes",
            model.mesh().n_elements(),
            model.mesh().nodes().len()
        ),
    );
    let exact_volume = 7.0 * std::f64::consts::PI / 6.0;
    c.at_most(
        "M1 octant volume vs 7 pi/6",
        (volume(&model) - exact_volume).abs() / exact_volume,
        5e-4,
    );
    let (worst, _, _) = elastic_run(4, LAYERS);
    c.at_most(
        "E1 elastic hoop vs Lame, largest relative error over the inner layer",
        worst,
        1e-2,
    );
    let (worst2, _, _) = elastic_run(2, LAYERS);
    c.at_least("N5 n = 2, 16 layers: E1 error is above the limit", worst2, 1e-2);
    c.finish();
}
