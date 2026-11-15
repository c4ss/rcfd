//! F-649 (ADR 0576): the mean-dilatation (BBar) Hex8 sphere of ADR 0541 D6 under follower pressure traced by the arc-length
//! step (`arclength::trace` with the load direction of the contributions, ADR 0560 D3) through its pressure maximum near
//! `l_a = 1.83` and down the descending branch to `l_a = 2.5`: T1, M1, D1c, M2, A0, C1, I1 and the report R1 (heavy,
//! ignored, run once), the negative control NC1 (heavy, ignored), and the fast checks F1 to F3 on a coarse sphere. The
//! helpers are copies of `sa_hyper_lock.rs` (ADR 0557), which is not edited; the bulk modulus is a parameter. Every measure
//! is required finite before it is compared.

use rcfd_core::vec3::Vec3;
use rcfd_fem::arclength::{trace, ArcLengthOptions, ArcLengthPath, PathPoint};
use rcfd_fem::constraint::{prescribed_equations, Prescribed};
use rcfd_fem::error::FemError;
use rcfd_fem::follower::FollowerPressure;
use rcfd_fem::material::HyperelasticEnergy;
use rcfd_fem::nonlinear::{solve_with, Contribution, DriverExtensions, NewtonOptions};
use rcfd_fem::{BlockProperty, ElementKind, FaceRef, Material, SolidFormulation, SolidMesh, SolidModel};

const BBAR: SolidFormulation = SolidFormulation::BBar;
const FULL: SolidFormulation = SolidFormulation::Full;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed. A value that is
/// not finite misses.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
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
    fn report(&self, name: &str, value: f64) {
        println!("{name}: {value:.6e} (reported, no limit)");
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

const MU: f64 = 1e6;
/// The bulk modulus of ADR 0541 D6 (`kappa/mu = 1e4`).
const K_HIGH: f64 = 1e10;
/// The bulk modulus of the fast test (`kappa/mu = 100`, arithmetic 7).
const K_FAST: f64 = 1e8;
const A_IN: f64 = 1.0;
const B_OUT: f64 = 2.0;
/// The maximum of the closed form over `l_a` (`p / mu`) and its place (arithmetic 1, `probe_f649.log`).
const P_MAX: f64 = 0.81642365;
const L_MAX: f64 = 1.828147;
/// The end of the descending branch (arithmetic 3).
const L_END: f64 = 2.5;
/// The arc length of D1 (arithmetic 4).
const DS: f64 = 0.65;

/// `l_b` of the inner stretch `l_a` (ADR 0541 arithmetic 5).
fn l_b(l_a: f64) -> f64 {
    (1.0 + (A_IN / B_OUT).powi(3) * (l_a.powi(3) - 1.0)).cbrt()
}

/// The closed form `p / mu`.
fn p_closed(l_a: f64) -> f64 {
    let lb = l_b(l_a);
    0.5 * ((1.0 / lb.powi(4) + 4.0 / lb) - (1.0 / l_a.powi(4) + 4.0 / l_a))
}

/// The octant mesh of ADR 0541 D6 with the surface set `"inner"` (face 0 of every layer-0 element): `(model, inner nodes,
/// inner faces as mesh-node quads)`.
fn sphere_model(n: usize, m: usize, f: SolidFormulation, kappa: f64) -> (SolidModel, Vec<usize>, Vec<[usize; 4]>) {
    let mut keys: Vec<[i64; 3]> = Vec::new();
    let mut dirs: Vec<Vec3> = Vec::new();
    let mut patch_ids: Vec<Vec<Vec<usize>>> = Vec::new();
    for face in 0..3 {
        let mut ids = vec![vec![0usize; n + 1]; n + 1];
        for (i, row) in ids.iter_mut().enumerate() {
            for (j, id) in row.iter_mut().enumerate() {
                let (s, t) = (i as f64 / n as f64, j as f64 / n as f64);
                let p = match face {
                    0 => [1.0, s, t],
                    1 => [t, 1.0, s],
                    _ => [s, t, 1.0],
                };
                let key = [
                    (p[0] * n as f64).round() as i64,
                    (p[1] * n as f64).round() as i64,
                    (p[2] * n as f64).round() as i64,
                ];
                *id = match keys.iter().position(|k| *k == key) {
                    Some(q) => q,
                    None => {
                        let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
                        keys.push(key);
                        dirs.push([p[0] / r, p[1] / r, p[2] / r]);
                        keys.len() - 1
                    }
                };
            }
        }
        patch_ids.push(ids);
    }
    let radii: Vec<f64> = (0..=m)
        .map(|k| A_IN * (B_OUT / A_IN).powf(k as f64 / m as f64))
        .collect();
    let node = |s: usize, k: usize| s * (m + 1) + k;
    let mut nodes = Vec::with_capacity(dirs.len() * (m + 1));
    for d in &dirs {
        for &r in &radii {
            nodes.push([r * d[0], r * d[1], r * d[2]]);
        }
    }
    let mut conn = Vec::new();
    let mut inner_faces = Vec::new();
    let mut inner_refs = Vec::new();
    for ids in &patch_ids {
        for i in 0..n {
            for j in 0..n {
                let mut quad = [ids[i][j], ids[i + 1][j], ids[i + 1][j + 1], ids[i][j + 1]];
                let x = |s: usize, k: usize| nodes[node(s, k)];
                let sub = |a: Vec3, b: Vec3| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
                let (x0, x1, x3, x4) = (x(quad[0], 0), x(quad[1], 0), x(quad[3], 0), x(quad[0], 1));
                let (e1, e3, e4) = (sub(x1, x0), sub(x3, x0), sub(x4, x0));
                let vol = e1[0] * (e3[1] * e4[2] - e3[2] * e4[1]) - e1[1] * (e3[0] * e4[2] - e3[2] * e4[0])
                    + e1[2] * (e3[0] * e4[1] - e3[1] * e4[0]);
                if vol < 0.0 {
                    quad = [quad[0], quad[3], quad[2], quad[1]];
                }
                inner_refs.push(FaceRef {
                    element: conn.len() / 8,
                    face: 0,
                });
                for k in 0..m {
                    for &s in &quad {
                        conn.push(node(s, k));
                    }
                    for &s in &quad {
                        conn.push(node(s, k + 1));
                    }
                }
                inner_faces.push([node(quad[0], 0), node(quad[1], 0), node(quad[2], 0), node(quad[3], 0)]);
            }
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let ne = conn.len() / 8;
    mesh.add_block(ElementKind::Hex8, conn).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_surface_set("inner", inner_refs).unwrap();
    if f != FULL {
        mesh.set_block_property(0, BlockProperty::solid(f)).unwrap();
    }
    let mut model = SolidModel::new(mesh);
    model
        .add_material(
            "all",
            Material::hyperelastic(HyperelasticEnergy::neo_hookean(MU, kappa).unwrap(), 1000.0).unwrap(),
        )
        .unwrap();
    let inner = (0..dirs.len()).map(|s| node(s, 0)).collect();
    (model, inner, inner_faces)
}

/// The symmetry conditions (`u_i = 0` where `X_i = 0`) and, with `inner_stretch`, the inner nodes moved to `l_a X`.
fn boundary(model: &SolidModel, inner: &[usize], inner_stretch: Option<f64>) -> Vec<Prescribed> {
    let is_inner: std::collections::BTreeSet<usize> = inner.iter().copied().collect();
    let mut prescribed = Vec::new();
    for (k, p) in model.mesh().nodes().iter().enumerate() {
        for c in 0..3 {
            match inner_stretch {
                Some(l_a) if is_inner.contains(&k) => prescribed.push(Prescribed::new(k, c, (l_a - 1.0) * p[c])),
                _ if p[c] == 0.0 => prescribed.push(Prescribed::new(k, c, 0.0)),
                _ => {}
            }
        }
    }
    prescribed
}

fn deformed(model: &SolidModel, u: &[f64], k: usize) -> Vec3 {
    let dofs = model.dof_map();
    let p = model.mesh().nodes()[k];
    std::array::from_fn(|c| p[c] + u[dofs.equation(k, c).unwrap()])
}

/// `|(1/3) sum_faces int x . n dA|` over the inner faces at the displacement `u` (bilinear, `2 x 2` Gauss).
fn cavity_volume(model: &SolidModel, u: &[f64], faces: &[[usize; 4]]) -> f64 {
    let g = 1.0 / 3.0f64.sqrt();
    let mut flux = 0.0;
    for face in faces {
        let xs: Vec<Vec3> = face.iter().map(|&k| deformed(model, u, k)).collect();
        for xi in [-g, g] {
            for eta in [-g, g] {
                let nshape = [
                    0.25 * (1.0 - xi) * (1.0 - eta),
                    0.25 * (1.0 + xi) * (1.0 - eta),
                    0.25 * (1.0 + xi) * (1.0 + eta),
                    0.25 * (1.0 - xi) * (1.0 + eta),
                ];
                let dxi = [
                    -0.25 * (1.0 - eta),
                    0.25 * (1.0 - eta),
                    0.25 * (1.0 + eta),
                    -0.25 * (1.0 + eta),
                ];
                let deta = [
                    -0.25 * (1.0 - xi),
                    -0.25 * (1.0 + xi),
                    0.25 * (1.0 + xi),
                    0.25 * (1.0 - xi),
                ];
                let mut p = [0.0; 3];
                let mut a = [0.0; 3];
                let mut b = [0.0; 3];
                for q in 0..4 {
                    for c in 0..3 {
                        p[c] += nshape[q] * xs[q][c];
                        a[c] += dxi[q] * xs[q][c];
                        b[c] += deta[q] * xs[q][c];
                    }
                }
                let nrm = [
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                ];
                flux += p[0] * nrm[0] + p[1] * nrm[1] + p[2] * nrm[2];
            }
        }
    }
    (flux / 3.0).abs()
}

/// The mean of `|x_a|` over the inner nodes and the spread `max - min`.
fn inner_stretch(model: &SolidModel, u: &[f64], inner: &[usize]) -> (f64, f64) {
    let radii: Vec<f64> = inner
        .iter()
        .map(|&k| {
            let x = deformed(model, u, k);
            (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt()
        })
        .collect();
    let mean = radii.iter().sum::<f64>() / radii.len() as f64;
    let spread = radii.iter().cloned().fold(f64::MIN, f64::max) - radii.iter().cloned().fold(f64::MAX, f64::min);
    (mean, spread)
}

/// The free equations (not prescribed), in increasing order.
fn free_equations(model: &SolidModel, prescribed: &[Prescribed]) -> Vec<usize> {
    let n = model.dof_map().n_equations();
    let fixed = prescribed_equations(model, prescribed).unwrap();
    let mut is_fixed = vec![false; n];
    for &(eq, _) in &fixed {
        is_fixed[eq] = true;
    }
    (0..n).filter(|&i| !is_fixed[i]).collect()
}

/// The constraint of every point from the stored points: `| |Du|^2 + c Dlambda^2 - Ds^2 | / Ds^2`, the largest.
fn constraint_error(model: &SolidModel, prescribed: &[Prescribed], path: &ArcLengthPath) -> f64 {
    let free = free_equations(model, prescribed);
    let n = model.dof_map().n_equations();
    let mut u_prev = vec![0.0; n];
    let mut l_prev = 0.0;
    let mut worst: f64 = 0.0;
    for p in &path.points {
        let du2: f64 = free.iter().map(|&q| (p.displacement[q] - u_prev[q]).powi(2)).sum();
        let dl = p.load_factor - l_prev;
        let e = (du2 + path.load_scale * dl * dl - p.length * p.length).abs() / (p.length * p.length);
        worst = if e.is_finite() && !worst.is_nan() {
            worst.max(e)
        } else {
            f64::NAN
        };
        u_prev = p.displacement.clone();
        l_prev = p.load_factor;
    }
    worst
}

fn newton(max_iterations: usize, residual_tolerance: Option<f64>) -> NewtonOptions {
    let mut o = NewtonOptions::new(1).threads(4).max_iterations(max_iterations);
    if let Some(t) = residual_tolerance {
        o.residual_tolerance = t;
    }
    o
}

/// T1, M1, D1c, M2, A0, C1, I1 and the report R1 (D1 of ADR 0576): the D6 BBar sphere traced by arc-length under the
/// follower pressure `p_ref = P_MAX mu` to `l_mean >= 2.5`. Heavy (about 64 points of the 864-element model): run once.
#[test]
#[ignore = "heavy (ADR 0576 D1): run once with --ignored"]
fn t1_m1_d1c_sphere_traced_through_its_pressure_maximum() {
    let mut c = Checks::new();
    let (model, inner, faces) = sphere_model(6, 8, BBAR, K_HIGH);
    let prescribed = boundary(&model, &inner, None);
    let p_ref = P_MAX * MU;
    let pressure = FollowerPressure::new(&model, "inner", p_ref).unwrap();
    let contributions: Vec<&dyn Contribution> = vec![&pressure];
    let options = ArcLengthOptions::new(DS)
        .max_points(200)
        .newton(newton(40, Some(1e-8)))
        .contribution_load(true);
    let zero = vec![0.0; model.dof_map().n_equations()];
    let v0 = cavity_volume(&model, &zero, &faces);
    let mut rows: Vec<(f64, f64, f64)> = Vec::new();
    let path = trace(
        &model,
        &prescribed,
        &[],
        &options,
        &contributions,
        &mut |p: &PathPoint| {
            let (l, spread) = inner_stretch(&model, &p.displacement, &inner);
            let lv = (cavity_volume(&model, &p.displacement, &faces) / v0).cbrt();
            println!(
                "  point {:3}: lambda {:.10} l_mean {:.8} spread {:.2e} volume stretch {:.8} p_c(l) {:.8} rel {:+.3e} \
             iterations {} halvings {} negative pivots {} Ds {:.4}",
                rows.len() + 1,
                p.load_factor,
                l,
                spread,
                lv,
                p_closed(l),
                (p.load_factor * p_ref / MU - p_closed(l)) / p_closed(l),
                p.residuals.len(),
                p.halvings,
                p.negative_pivots,
                p.length
            );
            rows.push((p.load_factor, l, spread));
            l >= L_END
        },
    );
    let path = match path {
        Ok(p) => p,
        Err(e) => {
            c.holds("T1 the trace", false, &format!("error {e}"));
            for id in ["M1", "D1c", "M2", "A0", "C1", "I1"] {
                c.holds(id, false, "not measured");
            }
            c.finish();
            return;
        }
    };
    println!(
        "  {} points, load scale {:e}, stopped {}",
        path.points.len(),
        path.load_scale,
        path.stopped
    );
    let finite = rows.iter().all(|r| r.0.is_finite() && r.1.is_finite());
    let k_max = (0..rows.len())
        .max_by(|&a, &b| rows[a].0.total_cmp(&rows[b].0))
        .unwrap_or(0);
    let after = rows.len().saturating_sub(k_max + 1);
    c.holds(
        "T1 the trace passes the maximum and stops at l_mean >= 2.5 within 200 points",
        finite && path.stopped && k_max > 0 && after >= 3,
        &format!(
            "{} points, the largest load factor at point {}, {after} after it",
            rows.len(),
            k_max + 1
        ),
    );
    let rel = |k: usize| -> f64 {
        let (lam, l, _) = rows[k];
        (lam * p_ref / MU - p_closed(l)).abs() / p_closed(l)
    };
    let worst = |ks: std::ops::Range<usize>| -> f64 {
        let mut w = if ks.is_empty() { f64::NAN } else { 0.0f64 };
        for k in ks {
            let r = rel(k);
            w = if r.is_finite() && !w.is_nan() {
                w.max(r)
            } else {
                f64::NAN
            };
        }
        w
    };
    let p_fe_max = rows[k_max].0 * p_ref / MU;
    c.at_most(
        "M1 |max lambda p_ref - p_c,max| / p_c,max",
        (p_fe_max - P_MAX).abs() / P_MAX,
        1e-2,
    );
    c.at_most(
        "D1c the descending branch: max |p - p_c(l_mean)| / p_c(l_mean)",
        worst(k_max + 1..rows.len()),
        1e-2,
    );
    c.at_most(
        "M2 |l_mean at the maximum - 1.828147|",
        (rows[k_max].1 - L_MAX).abs(),
        0.05,
    );
    c.at_most(
        "A0 the ascending branch: max |p - p_c(l_mean)| / p_c(l_mean)",
        worst(0..k_max),
        1e-2,
    );
    c.at_most("C1 the constraint", constraint_error(&model, &prescribed, &path), 1e-10);
    let it = path
        .points
        .iter()
        .map(|p| p.residuals.len())
        .max()
        .unwrap_or(usize::MAX);
    c.at_most("I1 the largest number of iterations of a point", it as f64, 20.0);
    // R1: the parabola vertex through the largest point and its neighbours, in (l_mean, p).
    if k_max > 0 && k_max + 1 < rows.len() {
        let (x0, x1, x2) = (rows[k_max - 1].1, rows[k_max].1, rows[k_max + 1].1);
        let (y0, y1, y2) = (rows[k_max - 1].0, rows[k_max].0, rows[k_max + 1].0);
        let d01 = (y1 - y0) / (x1 - x0);
        let d12 = (y2 - y1) / (x2 - x1);
        let a = (d12 - d01) / (x2 - x0);
        let b = d01 - a * (x0 + x1);
        let xv = -b / (2.0 * a);
        let yv = y1 + d01 * (xv - x1) + a * (xv - x0) * (xv - x1);
        c.report("R1 the parabola vertex: l", xv);
        c.report("R1 the parabola vertex: p / p_c,max - 1", yv * p_ref / MU / P_MAX - 1.0);
    }
    c.report("R1 the raw maximum: p / p_c,max - 1", p_fe_max / P_MAX - 1.0);
    let np: Vec<usize> = path.points.iter().map(|p| p.negative_pivots).collect();
    println!("R1 negative pivots per point: {np:?}");
    let hv: usize = path.points.iter().map(|p| p.halvings).sum();
    c.report("R1 halvings in all", hv as f64);
    c.finish();
}

/// NC1 (D2 of ADR 0576): load control with the follower pressure 1.02 p_c,max fails (no equilibrium above the maximum).
/// Heavy: run once.
#[test]
#[ignore = "heavy (ADR 0576 D2): run once with --ignored"]
fn nc1_load_control_above_the_maximum_fails() {
    let mut c = Checks::new();
    let (model, inner, _) = sphere_model(6, 8, BBAR, K_HIGH);
    let prescribed = boundary(&model, &inner, None);
    let pressure = FollowerPressure::new(&model, "inner", 1.02 * P_MAX * MU).unwrap();
    let ext = DriverExtensions {
        contributions: vec![&pressure],
        step_control: None,
    };
    let mut o = NewtonOptions::new(10).threads(4).max_iterations(40);
    o.residual_tolerance = 1e-8;
    match solve_with(&model, &prescribed, &[], &o, ext) {
        Ok(r) => {
            let (l, _) = inner_stretch(&model, r.result.displacement_values(), &inner);
            c.holds(
                "NC1 load control at 1.02 p_c,max fails",
                false,
                &format!("converged, l_mean {l}"),
            );
        }
        Err(e) => c.holds("NC1 load control at 1.02 p_c,max fails", true, &format!("{e}")),
    }
    c.finish();
}

/// F1 to F3 (D3 of ADR 0576): the coarse sphere traced by 3 points, each against load control at its load factor; the
/// default route's refusal; the constraint.
#[test]
fn f1_f2_f3_coarse_sphere_against_load_control() {
    let mut c = Checks::new();
    let (model, inner, _) = sphere_model(3, 4, BBAR, K_FAST);
    let prescribed = boundary(&model, &inner, None);
    let p_ref = 0.5 * MU;
    let pressure = FollowerPressure::new(&model, "inner", p_ref).unwrap();
    let contributions: Vec<&dyn Contribution> = vec![&pressure];
    let e = trace(
        &model,
        &prescribed,
        &[],
        &ArcLengthOptions::new(0.24).newton(newton(20, None)),
        &contributions,
        &mut |_: &PathPoint| false,
    );
    c.holds(
        "F2 the default route refuses (key arc length)",
        matches!(&e, Err(FemError::InvalidValue { key: "arc length", .. })),
        &format!("{:?}", e.as_ref().err()),
    );
    let options = ArcLengthOptions::new(0.24)
        .max_points(3)
        .newton(newton(20, None))
        .contribution_load(true);
    let path = match trace(
        &model,
        &prescribed,
        &[],
        &options,
        &contributions,
        &mut |_: &PathPoint| false,
    ) {
        Ok(p) => p,
        Err(e) => {
            c.holds("F1 the trace", false, &format!("error {e}"));
            c.holds("F3", false, "not measured");
            c.finish();
            return;
        }
    };
    let mut worst: f64 = 0.0;
    for (k, p) in path.points.iter().enumerate() {
        let (l, _) = inner_stretch(&model, &p.displacement, &inner);
        let fp = FollowerPressure::new(&model, "inner", p.load_factor * p_ref).unwrap();
        let ext = DriverExtensions {
            contributions: vec![&fp],
            step_control: None,
        };
        let o = NewtonOptions::new(5).max_iterations(20);
        match solve_with(&model, &prescribed, &[], &o, ext) {
            Ok(r) => {
                let u = r.result.displacement_values();
                let scale = u.iter().fold(0.0f64, |m, x| m.max(x.abs()));
                let diff = u
                    .iter()
                    .zip(&p.displacement)
                    .fold(0.0f64, |m, (a, b)| m.max((a - b).abs()));
                println!(
                    "  point {}: lambda {:.8} l_mean {l:.8} iterations {} |u_al - u_lc| / |u_lc| {:.3e}",
                    k + 1,
                    p.load_factor,
                    p.residuals.len(),
                    diff / scale
                );
                let r = diff / scale;
                worst = if r.is_finite() && !worst.is_nan() {
                    worst.max(r)
                } else {
                    f64::NAN
                };
            }
            Err(e) => {
                println!("  point {}: load control failed: {e}", k + 1);
                worst = f64::NAN;
            }
        }
    }
    c.holds(
        "F1 three points",
        path.points.len() == 3,
        &format!("{}", path.points.len()),
    );
    c.at_most("F1 max |u_al - u_lc| / max |u_lc|", worst, 1e-6);
    c.at_most("F3 the constraint", constraint_error(&model, &prescribed, &path), 1e-10);
    c.finish();
}
