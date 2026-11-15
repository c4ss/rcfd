//! F-622 (ADR 0557): the Full Hex8 locking control of the inflated hyperelastic sphere of ADR 0541 with a set-up that
//! converges. A1 (the acceptance, option (a): the inner stretch 1.10 under displacement control), its companions A2, A3, the
//! pressure-control companions P1 to P3 (option (b) by the load-stepping Newton driver with a follower pressure), the set-up
//! report P0 and the diagnostic DX (the lowest eigenvalue of the Full tangent along the path). The mesh, the material and the
//! pressure measure are those of ADR 0541 D6 (the helpers are copies of `sa_hyper.rs`, which is not edited). Every measure
//! is required finite before it is compared.

use rcfd_core::vec3::Vec3;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::follower::{tangent_modes, FollowerPressure};
use rcfd_fem::material::HyperelasticEnergy;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::nonlinear::{
    initial_states, solve, solve_with, Contribution, DriverExtensions, NewtonOptions, NonlinearResult,
};
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
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.4e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
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
const K_HIGH: f64 = 1e10;
const A_IN: f64 = 1.0;
const B_OUT: f64 = 2.0;
/// The inner stretch of A1 to A3 (arithmetic 3).
const L_SMALL: f64 = 1.10;
/// The inner stretch whose closed-form pressure drives P1 to P3 (arithmetic 4).
const L_P: f64 = 1.5;
/// The 1D full-integration model at `L_SMALL`, `p / mu` (arithmetic 3, `probe_lock1d.log`).
const P_1D_FULL: f64 = 4.769507;
/// The 1D full-integration model under the pressure `p_c(1.5)`: the extension ratio (arithmetic 4).
const E_1D_FULL: f64 = 2.2826e-2;

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
fn sphere_model(n: usize, m: usize, f: SolidFormulation) -> (SolidModel, Vec<usize>, Vec<[usize; 4]>) {
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
            Material::hyperelastic(HyperelasticEnergy::neo_hookean(MU, K_HIGH).unwrap(), 1000.0).unwrap(),
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

fn options(max_iterations: usize) -> NewtonOptions {
    let mut o = NewtonOptions::new(10).threads(4).max_iterations(max_iterations);
    o.residual_tolerance = 1e-8;
    o
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

fn iterations(r: &NonlinearResult) -> Vec<usize> {
    r.steps.iter().map(|s| s.residuals.len()).collect()
}

/// Displacement control to `l_a` (D1): the result and `p_FE / mu = sum_a R_a . x_a / (3 V_cav mu)`.
fn displacement_control(
    f: SolidFormulation,
    l_a: f64,
) -> (SolidModel, Vec<Prescribed>, Result<(NonlinearResult, f64), String>) {
    let (model, inner, faces) = sphere_model(6, 8, f);
    let prescribed = boundary(&model, &inner, Some(l_a));
    let out = solve(&model, &prescribed, &[], &options(20))
        .map_err(|e| e.to_string())
        .and_then(|r| {
            let u = r.result.displacement_values().to_vec();
            let reactions = r.result.reactions().ok_or("no reactions")?.to_vec();
            let dofs = model.dof_map();
            let mut work = 0.0;
            for &k in &inner {
                let xk = deformed(&model, &u, k);
                for (c, xc) in xk.iter().enumerate() {
                    work += reactions[dofs.equation(k, c).unwrap()] * xc;
                }
            }
            let v_cav = cavity_volume(&model, &u, &faces);
            let p_fe = work / (3.0 * v_cav) / MU;
            println!(
                "  {} to l_a {l_a}: p_FE / mu {p_fe:.10}, closed {:.10}, V_cav {v_cav:.8}, steps {}, iterations {:?}",
                f.name(),
                p_closed(l_a),
                r.steps.len(),
                iterations(&r)
            );
            Ok((r, p_fe))
        });
    (model, prescribed, out)
}

/// The lowest `omega^2` of the tangent at a state (D3).
fn lowest(model: &SolidModel, prescribed: &[Prescribed], u: &[f64], states: &[rcfd_fem::element::ElementState]) -> f64 {
    match tangent_modes(model, prescribed, u, states, 1.0, &[], &ModalOptions::new(1).threads(4)) {
        Ok(m) => m.eigenvalues[0],
        Err(e) => {
            println!("  tangent_modes failed: {e}");
            f64::NAN
        }
    }
}

/// A1, A2, A3 and the diagnostic DX.
#[test]
fn a1_a2_a3_dx_displacement_control_to_1_10() {
    let mut c = Checks::new();
    let pc = p_closed(L_SMALL);
    let (model, prescribed, full) = displacement_control(FULL, L_SMALL);
    let mut dx = Vec::new();
    match full {
        Ok((r, p)) => {
            c.holds("A1 Full to l_a 1.10: the solve", true, "converged");
            c.at_least("A1 Full to l_a 1.10: |p_FE - p_c| / p_c", (p - pc).abs() / pc, 0.1);
            c.at_most(
                "A3 Full to l_a 1.10: |p_FE / mu - p_1D| / p_1D",
                (p - P_1D_FULL).abs() / P_1D_FULL,
                5e-2,
            );
            c.report("  A1 p_FE / p_c (the locking factor)", p / pc);
            let rest = initial_states(&model).unwrap();
            let zero = vec![0.0; r.result.displacement_values().len()];
            dx.push((1.0, lowest(&model, &prescribed, &zero, &rest)));
            dx.push((
                L_SMALL,
                lowest(&model, &prescribed, r.result.displacement_values(), &r.states),
            ));
        }
        Err(e) => {
            c.holds("A1 Full to l_a 1.10: the solve", false, &e);
            c.holds("A3 Full to l_a 1.10", false, "not measured");
        }
    }
    match displacement_control(BBAR, L_SMALL).2 {
        Ok((_, p)) => c.at_most("A2 BBar to l_a 1.10: |p_FE - p_c| / p_c", (p - pc).abs() / pc, 1e-2),
        Err(e) => c.holds("A2 BBar to l_a 1.10: the solve", false, &e),
    }
    // DX (diagnostic, no limit): a second Full solve to 1.125.
    let (model2, prescribed2, full2) = displacement_control(FULL, 1.125);
    match full2 {
        Ok((r, _)) => dx.push((
            1.125,
            lowest(&model2, &prescribed2, r.result.displacement_values(), &r.states),
        )),
        Err(e) => println!("DX Full to l_a 1.125: the solve failed: {e}"),
    }
    for (l, w2) in &dx {
        c.report(&format!("DX Full omega_1^2 at l_a {l}"), *w2);
    }
    if dx.len() == 3 {
        let ((l1, w1), (l2, w2)) = (dx[1], dx[2]);
        c.report(
            "DX zero crossing of the line through 1.10 and 1.125",
            l2 - w2 * (l2 - l1) / (w2 - w1),
        );
    }
    c.finish();
}

/// Pressure control at `p_c(1.5)` (D2): the result and `(l_mean, spread, cavity-volume stretch)`.
fn pressure_control(f: SolidFormulation) -> Result<(f64, f64, f64), String> {
    let (model, inner, faces) = sphere_model(6, 8, f);
    let prescribed = boundary(&model, &inner, None);
    let pressure = FollowerPressure::new(&model, "inner", p_closed(L_P) * MU).map_err(|e| e.to_string())?;
    let list: Vec<&dyn Contribution> = vec![&pressure];
    let ext = DriverExtensions {
        contributions: list,
        step_control: None,
    };
    let r = solve_with(&model, &prescribed, &[], &options(40), ext).map_err(|e| e.to_string())?;
    let u = r.result.displacement_values();
    let radii: Vec<f64> = inner
        .iter()
        .map(|&k| {
            let x = deformed(&model, u, k);
            (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt()
        })
        .collect();
    let mean = radii.iter().sum::<f64>() / radii.len() as f64;
    let spread = radii.iter().cloned().fold(f64::MIN, f64::max) - radii.iter().cloned().fold(f64::MAX, f64::min);
    let zero = vec![0.0; u.len()];
    let lv = (cavity_volume(&model, u, &faces) / cavity_volume(&model, &zero, &faces)).cbrt();
    println!(
        "  {} at p_c(1.5): l_mean {mean:.10}, spread {spread:.3e}, volume stretch {lv:.10}, steps {}, iterations {:?}",
        f.name(),
        r.steps.len(),
        iterations(&r)
    );
    Ok((mean, spread, lv))
}

/// P0 to P3.
#[test]
fn p0_p1_p2_p3_pressure_control_at_p_c_1_5() {
    let mut c = Checks::new();
    match pressure_control(FULL) {
        Ok((l, spread, lv)) => {
            let e = (l - 1.0) / (L_P - 1.0);
            c.holds("P1 Full at p_c(1.5): the solve", true, "converged");
            c.holds("P1 Full: e_F > 0", e > 0.0, &format!("e_F = {e:e}"));
            c.at_most("P1 Full: e_F", e, 0.1);
            c.at_most("P2 Full: |e_F / e_1D - 1|", (e / E_1D_FULL - 1.0).abs(), 0.1);
            c.report("P0 Full spread / (l_mean - 1)", spread / (l - 1.0));
            c.report("P0 Full cavity-volume stretch", lv);
        }
        Err(e) => {
            c.holds("P1 Full at p_c(1.5): the solve", false, &e);
            c.holds("P2 Full", false, "not measured");
        }
    }
    match pressure_control(BBAR) {
        Ok((l, spread, lv)) => {
            c.holds("P3 BBar at p_c(1.5): the solve", true, "converged");
            c.at_most("P3 BBar: |l_mean - 1.5| / 0.5", (l - L_P).abs() / (L_P - 1.0), 2e-2);
            c.report("P0 BBar spread / (l_mean - 1)", spread / (l - 1.0));
            c.report("P0 BBar cavity-volume stretch", lv);
        }
        Err(e) => c.holds("P3 BBar at p_c(1.5): the solve", false, &e),
    }
    c.finish();
}

/// DX' of amendment 1 (diagnostic, no limit): D3 with the eigen tolerance `1e-8`.
#[test]
fn dx_after_amendment_1() {
    let c = Checks::new();
    let options = ModalOptions::new(1).threads(4).tolerance(1e-8);
    let eig = |model: &SolidModel, prescribed: &[Prescribed], u: &[f64], states: &[rcfd_fem::element::ElementState]| {
        match tangent_modes(model, prescribed, u, states, 1.0, &[], &options) {
            Ok(m) => m.eigenvalues[0],
            Err(e) => {
                println!("  tangent_modes failed: {e}");
                f64::NAN
            }
        }
    };
    let mut dx = Vec::new();
    for l_a in [L_SMALL, 1.125] {
        let (model, prescribed, full) = displacement_control(FULL, l_a);
        match full {
            Ok((r, _)) => {
                if dx.is_empty() {
                    let rest = initial_states(&model).unwrap();
                    let zero = vec![0.0; r.result.displacement_values().len()];
                    dx.push((1.0, eig(&model, &prescribed, &zero, &rest)));
                }
                dx.push((l_a, eig(&model, &prescribed, r.result.displacement_values(), &r.states)));
            }
            Err(e) => println!("DX' Full to l_a {l_a}: the solve failed: {e}"),
        }
    }
    for (l, w2) in &dx {
        c.report(&format!("DX' Full omega_1^2 at l_a {l}"), *w2);
    }
    if dx.len() == 3 {
        let ((l1, w1), (l2, w2)) = (dx[1], dx[2]);
        c.report(
            "DX' zero crossing of the line through 1.10 and 1.125",
            l2 - w2 * (l2 - l1) / (w2 - w1),
        );
    }
    c.finish();
}
