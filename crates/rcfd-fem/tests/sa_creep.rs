//! F-411 (ADR 0489, the registration of ADR 0314): power-law creep with time hardening, the backward Euler radial return with
//! its consistent tangent, on the quasi-static time-stepping driver `nonlinear::solve_in_time`. The checks T1, T2, U1, U1e,
//! U2, U2e, R1 to R3, C1, C2, C2d (C2r reported), B1, H1, H2, NC1, Q1, N2 and N3 of the registration (seed 0xF411_0489). N1
//! is `sa_creep_default_bits.rs`.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::element::{element_for, ElementState};
use rcfd_fem::hex20::{reference_nodes, upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::mat_creep::{self, update, ITERATION_KEY};
use rcfd_fem::material::{MaterialAxes, PointContext, PronyTerm};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nonlinear::{
    element_force_and_tangent_at, element_second_piola_kirchhoff, initial_states, solve, solve_from, solve_from_at,
    solve_in_time, NewtonOptions, TimeHistory, TimeSteppingOptions,
};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::transient::{Integrator, TransientOptions};
use rcfd_fem::{Constraint, ElementKind, FemError, Load, Material, SolidModel};

const SEED: u64 = 0xF411_0489;
const E: f64 = 2.0e9;
const NU: f64 = 0.3;
const RHO: f64 = 1200.0;

/// SplitMix64, a deterministic source only.
struct Rng(u64);

impl Rng {
    fn new(tag: u64) -> Rng {
        Rng(SEED.wrapping_add(tag))
    }
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
    fn sym(&mut self, r: f64) -> f64 {
        r * (2.0 * self.uniform() - 1.0)
    }
}

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{name}: {value:.6} (limit >= {limit}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value} < {limit}"));
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

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn max_abs(v: impl Iterator<Item = f64>) -> f64 {
    v.fold(0.0, |a, x| a.max(x.abs()))
}

fn key_of(e: &FemError) -> Option<&'static str> {
    match e {
        FemError::InvalidValue { key, .. }
        | FemError::FieldLength { key, .. }
        | FemError::InvalidMaterial { key, .. } => Some(key),
        _ => None,
    }
}

fn base() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

fn creep(a: f64, n: f64, m: f64) -> Material {
    Material::creep(base(), a, n, m).unwrap()
}

fn ctx(t: f64, dt: f64) -> PointContext {
    PointContext::new().with_time(t, dt).unwrap()
}

/// `x = a^-1 b` by Gaussian elimination with partial pivoting.
fn solve_dense<const N: usize>(mut a: [[f64; N]; N], mut b: [f64; N]) -> [f64; N] {
    for c in 0..N {
        let p = (c..N)
            .max_by(|&i, &j| a[i][c].abs().partial_cmp(&a[j][c].abs()).unwrap())
            .unwrap();
        a.swap(c, p);
        b.swap(c, p);
        for r in c + 1..N {
            let f = a[r][c] / a[c][c];
            for k in c..N {
                a[r][k] -= f * a[c][k];
            }
            b[r] -= f * b[c];
        }
    }
    let mut x = [0.0; N];
    for r in (0..N).rev() {
        let mut s = b[r];
        for k in r + 1..N {
            s -= a[r][k] * x[k];
        }
        x[r] = s / a[r][r];
    }
    x
}

// ------------------------------------------------------------------------------------------------- T1

#[test]
fn t1_material_tangent_central_differences() {
    let mut c = Checks::new();
    let mut rng = Rng::new(1);
    for (case, (n, m)) in [(3.0, 0.0), (4.0, -0.5), (5.0, 0.3), (3.0, 0.0), (1.5, 0.0)]
        .iter()
        .enumerate()
    {
        let mat = creep(1e-26, *n, *m);
        let mut eps = [0.0; 6];
        for v in eps.iter_mut() {
            *v = rng.sym(1e-3);
        }
        let mut st = [0.0; 7];
        for v in st.iter_mut().take(6) {
            *v = rng.sym(2e-4);
        }
        st[6] = 3e-4;
        let p = ctx(2.0, 0.5);
        let u0 = update(&mat, &eps, &st, &p).unwrap();
        c.holds(
            &format!("T1 case {case} (n = {n}, m = {m}): set-up (creeping)"),
            u0.creeping && u0.state[6] > st[6],
            &format!(
                "{} local iterations, d eps_bar {:e}",
                u0.iterations,
                u0.state[6] - st[6]
            ),
        );
        let h = 1e-10;
        let mut worst: f64 = 0.0;
        for j in 0..6 {
            let mut a = eps;
            a[j] += h;
            let mut b = eps;
            b[j] -= h;
            let sa = update(&mat, &a, &st, &p).unwrap().stress;
            let sb = update(&mat, &b, &st, &p).unwrap().stress;
            for i in 0..6 {
                worst = worst.max((u0.tangent[i][j] - (sa[i] - sb[i]) / (2.0 * h)).abs());
            }
        }
        let scale = max_abs(u0.tangent.iter().flatten().copied());
        c.at_most(
            &format!("T1 case {case} (n = {n}, m = {m}): max |C - C_fd| / max |C|"),
            worst / scale,
            1e-6,
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- T2

fn distorted(kind: ElementKind, rng: &mut Rng) -> Vec<Vec3> {
    let unit: Vec<Vec3> = match kind {
        ElementKind::Hex8 => vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ],
        _ => reference_nodes()
            .iter()
            .map(|p| [(p[0] + 1.0) / 2.0, (p[1] + 1.0) / 2.0, (p[2] + 1.0) / 2.0])
            .collect(),
    };
    let amp = if kind == ElementKind::Hex8 { 0.1 } else { 0.05 };
    unit.iter()
        .map(|p| [p[0] + rng.sym(amp), p[1] + rng.sym(amp), p[2] + rng.sym(amp)])
        .collect()
}

/// The Green-Lagrange strains (Voigt, engineering shear) at the points of the stiffness rule.
fn point_strains(kind: ElementKind, x: &[Vec3], u: &[f64]) -> Vec<[f64; 6]> {
    let el = element_for(kind);
    let ips = el.integration_points(x, el.stiffness_rule()).unwrap();
    ips.iter()
        .map(|ip| {
            let mut h = [[0.0; 3]; 3];
            for (a, g) in ip.geometry.grad.iter().enumerate() {
                for (i, row) in h.iter_mut().enumerate() {
                    for (j, v) in row.iter_mut().enumerate() {
                        *v += u[3 * a + i] * g[j];
                    }
                }
            }
            let e = |i: usize, j: usize| h[i][j] + h[j][i] + h[0][i] * h[0][j] + h[1][i] * h[1][j] + h[2][i] * h[2][j];
            [0.5 * e(0, 0), 0.5 * e(1, 1), 0.5 * e(2, 2), e(0, 1), e(1, 2), e(2, 0)]
        })
        .collect()
}

#[test]
fn t2_element_tangent_central_differences() {
    let mut c = Checks::new();
    let mut rng = Rng::new(2);
    let mat = creep(1e-25, 4.0, -0.5);
    let p = ctx(2.0, 0.5);
    for kind in [ElementKind::Hex8, ElementKind::Hex20] {
        let el = element_for(kind);
        let x = distorted(kind, &mut rng);
        let n = 3 * el.n_nodes();
        let np = el.stiffness_rule().len();
        let mut v = Vec::new();
        for _ in 0..np {
            for _ in 0..6 {
                v.push(rng.sym(2e-4));
            }
            v.push(3e-4);
        }
        let st = ElementState::new(v);
        let u: Vec<f64> = (0..n).map(|_| rng.sym(1e-3)).collect();
        let crept = point_strains(kind, &x, &u)
            .iter()
            .enumerate()
            .filter(|(q, eps)| {
                let s = &st.values()[q * 7..(q + 1) * 7];
                let r = update(&mat, eps, s, &p).unwrap();
                r.creeping && r.state[6] - s[6] > 1e-8
            })
            .count();
        c.holds(
            &format!("T2 {}: set-up (points creeping in the step)", kind.name()),
            crept == np,
            &format!("{crept} of {np}"),
        );
        let (_, k) = element_force_and_tangent_at(el, &x, &mat, &u, &st, &p).unwrap();
        let h = 1e-9;
        let mut worst: f64 = 0.0;
        for j in 0..n {
            let mut a = u.clone();
            a[j] += h;
            let mut b = u.clone();
            b[j] -= h;
            let fa = element_force_and_tangent_at(el, &x, &mat, &a, &st, &p).unwrap().0;
            let fb = element_force_and_tangent_at(el, &x, &mat, &b, &st, &p).unwrap().0;
            for i in 0..n {
                worst = worst.max((k.values()[i * n + j] - (fa[i] - fb[i]) / (2.0 * h)).abs());
            }
        }
        let scale = max_abs(k.values().iter().copied());
        c.at_most(
            &format!("T2 {}: max |K - K_fd| / max |K|", kind.name()),
            worst / scale,
            1e-6,
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- U1, U2

/// Creep at the material point under the uniaxial stress `sigma` (stress control, Newton on the strain with the tangent):
/// the largest relative error of `eps_c,xx` against `A sigma^n t^(m+1)/(m+1)` over the step ends of `steps` steps of `dt`.
fn uniaxial_point(a: f64, n: f64, m: f64, sigma: f64, steps: usize, dt: f64) -> f64 {
    let mat = creep(a, n, m);
    let mut st = [0.0; 7];
    let mut eps = [0.0; 6];
    let target = [sigma, 0.0, 0.0, 0.0, 0.0, 0.0];
    let mut worst: f64 = 0.0;
    for k in 0..=steps {
        let (t, h) = if k == 0 { (0.0, 0.0) } else { (k as f64 * dt, dt) };
        let p = ctx(t, h);
        let mut it = 0;
        let r = loop {
            let r = update(&mat, &eps, &st, &p).unwrap();
            let res: Vec<f64> = (0..6).map(|i| r.stress[i] - target[i]).collect();
            if it > 0 && max_abs(res.iter().copied()) <= 1e-13 * sigma {
                break r;
            }
            assert!(it < 50, "stress control not converged");
            let d = solve_dense(r.tangent, [res[0], res[1], res[2], res[3], res[4], res[5]]);
            for i in 0..6 {
                eps[i] -= d[i];
            }
            it += 1;
        };
        st = r.state;
        if k > 0 {
            let want = a * sigma.powf(n) * t.powf(m + 1.0) / (m + 1.0);
            worst = worst.max((st[0] - want).abs() / want);
        }
    }
    worst
}

/// The unit cube of one Hex8 with three symmetry planes.
fn cube(material: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m
}

fn times(steps: usize, dt: f64) -> Vec<f64> {
    (1..=steps).map(|k| k as f64 * dt).collect()
}

/// The one-element creep under the dead traction `sigma0` on `x = 1` by the driver: the largest relative error of the
/// `eps_c,xx` of every point against the closed form.
fn uniaxial_element(a: f64, n: f64, m: f64, sigma0: f64, steps: usize, dt: f64) -> f64 {
    let mut model = cube(creep(a, n, m));
    model.add_load(Load::traction("xmax", [sigma0, 0.0, 0.0])).unwrap();
    let mut newton = NewtonOptions::new(1);
    newton.residual_tolerance = 1e-12;
    let opts = TimeSteppingOptions::new(f64::INFINITY).newton(newton);
    let h = solve_in_time(&model, &[], &times(steps, dt), &opts).unwrap();
    let mut worst: f64 = 0.0;
    for s in h.steps.iter().skip(1) {
        let want = a * sigma0.powf(n) * s.time.powf(m + 1.0) / (m + 1.0);
        for q in s.states[0].values().chunks(7) {
            worst = worst.max((q[0] - want).abs() / want);
        }
    }
    worst
}

#[test]
fn u1_u2_uniaxial_constant_stress() {
    let mut c = Checks::new();
    c.at_most(
        "U1 material point, m = 0: max |eps_c - A sigma^n t| / (A sigma^n t)",
        uniaxial_point(1e-25, 3.0, 0.0, 2e6, 100, 0.1),
        1e-8,
    );
    c.at_most(
        "U1e one Hex8, dead traction, m = 0",
        uniaxial_element(1.25e-9, 3.0, 0.0, 0.2, 100, 0.1),
        1e-8,
    );
    c.at_most(
        "U2 material point, m = -0.5: max relative error against A sigma^n t^(m+1)/(m+1)",
        uniaxial_point(1e-25, 3.0, -0.5, 2e6, 100, 0.1),
        1e-3,
    );
    c.at_most(
        "U2e one Hex8, dead traction, m = -0.5",
        uniaxial_element(1.25e-9, 3.0, -0.5, 0.2, 100, 0.1),
        1e-3,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- R1 to R3

const EPS0: f64 = 1e-3;

/// `A` of a relaxation time scale of 1 s at `sigma0 = E eps0` (ADR 0489 arithmetic 3).
fn relax_a(n: f64) -> f64 {
    1.0 / ((n - 1.0) * E * (E * EPS0).powf(n - 1.0))
}

fn stretched_cube(material: Material) -> SolidModel {
    let mut m = cube(material);
    m.add_constraint(Constraint::displacement("xmax", 0, EPS0)).unwrap();
    m
}

/// The second Piola-Kirchhoff stresses at the points of element `e` at a step end (default context: the committed stress).
fn stresses(model: &SolidModel, u: &[f64], states: &[ElementState], e: usize) -> Vec<[f64; 6]> {
    let mesh = model.mesh();
    let dofs = model.dof_map();
    let nodes = mesh.element_nodes(e);
    let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
    let ue: Vec<f64> = nodes
        .iter()
        .flat_map(|&n| (0..3).map(move |c| (n, c)))
        .map(|(n, c)| u[dofs.equation(n, c).unwrap()])
        .collect();
    let material = model.material_of(e).unwrap();
    element_second_piola_kirchhoff(element_for(mesh.element_kind(e)), &coords, material, &ue, &states[e]).unwrap()
}

/// The relaxation of the stretched cube by the driver at `steps` uniform steps to 10 s: the largest relative error of
/// `S_xx` over the step ends and points against the closed form with `sigma0 = S_xx(0)`.
fn relaxation(n: f64, steps: usize) -> (f64, TimeHistory) {
    let a = relax_a(n);
    let model = stretched_cube(creep(a, n, 0.0));
    let h = solve_in_time(
        &model,
        &[],
        &times(steps, 10.0 / steps as f64),
        &TimeSteppingOptions::new(f64::INFINITY),
    )
    .unwrap();
    let s0: Vec<f64> = stresses(&model, &h.steps[0].u, &h.steps[0].states, 0)
        .iter()
        .map(|s| s[0])
        .collect();
    let mut worst: f64 = 0.0;
    for s in &h.steps {
        for (q, st) in stresses(&model, &s.u, &s.states, 0).iter().enumerate() {
            let want = (s0[q].powf(1.0 - n) + (n - 1.0) * E * a * s.time).powf(1.0 / (1.0 - n));
            worst = worst.max((st[0] - want).abs() / want);
        }
    }
    (worst, h)
}

#[test]
fn r1_to_r3_relaxation_at_fixed_strain() {
    let mut c = Checks::new();
    for (id, n) in [("R1", 3.0), ("R2", 5.0)] {
        let (e800, h) = relaxation(n, 800);
        let halvings: usize = h.steps.iter().map(|s| s.halvings).sum();
        c.holds(
            &format!("{id} n = {n}: set-up (no halving)"),
            halvings == 0,
            &format!("{halvings}"),
        );
        c.at_most(
            &format!("{id} n = {n}, 800 steps: max |S_xx - sigma(t)| / sigma(t)"),
            e800,
            5e-3,
        );
        let (e400, _) = relaxation(n, 400);
        println!("R3 n = {n}: 400 steps {e400:.6e}, 800 steps {e800:.6e}");
        c.at_least(&format!("R3 n = {n}: log2(e_400/e_800)"), (e400 / e800).log2(), 0.9);
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- C1, C2, C2d

const CYL_A: f64 = 1.0;
const CYL_B: f64 = 2.0;
const CYL_P: f64 = 2e5;
const CYL_N: f64 = 4.0;

/// The plane-strain quarter of the cylinder of D6: 12 radial x 8 circumferential Hex20 on their circles, one layer.
fn cylinder() -> SolidModel {
    let hz = 1.0 / 12.0;
    let mesh = hex_box(
        [CYL_A, 0.0, 0.0],
        [CYL_B - CYL_A, std::f64::consts::FRAC_PI_2, hz],
        [12, 8, 1],
    )
    .unwrap();
    let mut mesh = upgrade_to_hex20_mapped(&mesh, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0;
    let nn = mesh.nodes().len();
    mesh.add_node_set("every", (0..nn).collect()).unwrap();
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", creep(1.0 / (E * CYL_P.powi(3)), CYL_N, 0.0))
        .unwrap();
    model.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    model.add_constraint(Constraint::displacement("ymax", 0, 0.0)).unwrap();
    model.add_constraint(Constraint::displacement("every", 2, 0.0)).unwrap();
    model.add_load(Load::pressure("xmin", CYL_P)).unwrap();
    model
}

fn lame(r: f64) -> (f64, f64) {
    let d = (CYL_B / CYL_A).powi(2) - 1.0;
    let q = (CYL_B / r).powi(2);
    (-CYL_P * (q - 1.0) / d, CYL_P * (q + 1.0) / d)
}

fn stationary(r: f64) -> (f64, f64) {
    let d = (CYL_B / CYL_A).powf(2.0 / CYL_N) - 1.0;
    let q = (CYL_B / r).powf(2.0 / CYL_N);
    (-CYL_P * (q - 1.0) / d, CYL_P * ((2.0 - CYL_N) / CYL_N * q + 1.0) / d)
}

/// Per element, per point: (weight, radius, sigma_r, sigma_theta) at a step end.
fn polar_points(model: &SolidModel, u: &[f64], states: &[ElementState]) -> Vec<Vec<(f64, f64, f64, f64)>> {
    let mesh = model.mesh();
    let el = element_for(ElementKind::Hex20);
    (0..mesh.n_elements())
        .map(|e| {
            let coords: Vec<Vec3> = mesh.element_nodes(e).iter().map(|&n| mesh.nodes()[n]).collect();
            let ips = el.integration_points(&coords, el.stiffness_rule()).unwrap();
            stresses(model, u, states, e)
                .iter()
                .zip(&ips)
                .map(|(s, ip)| {
                    let x = ip.geometry.x;
                    let r = x[0].hypot(x[1]);
                    let (cs, sn) = (x[0] / r, x[1] / r);
                    let srr = cs * cs * s[0] + sn * sn * s[1] + 2.0 * cs * sn * s[3];
                    let stt = sn * sn * s[0] + cs * cs * s[1] - 2.0 * cs * sn * s[3];
                    (ip.weight, r, srr, stt)
                })
                .collect()
        })
        .collect()
}

/// (point-wise error, element-mean error, point-wise deviatoric error), relative to `p`, against `field`.
fn cylinder_errors(pts: &[Vec<(f64, f64, f64, f64)>], field: fn(f64) -> (f64, f64)) -> (f64, f64, f64) {
    let (mut point, mut mean, mut dev): (f64, f64, f64) = (0.0, 0.0, 0.0);
    for e in pts {
        let (mut w, mut a, mut b, mut ea, mut eb) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for &(wq, r, srr, stt) in e {
            let (fr, ft) = field(r);
            point = point.max((srr - fr).abs()).max((stt - ft).abs());
            dev = dev.max(((stt - srr) - (ft - fr)).abs());
            w += wq;
            a += wq * srr;
            b += wq * stt;
            ea += wq * fr;
            eb += wq * ft;
        }
        mean = mean.max(((a - ea) / w).abs()).max(((b - eb) / w).abs());
    }
    (point / CYL_P, mean / CYL_P, dev / CYL_P)
}

#[test]
fn c1_c2_thick_cylinder_plane_strain() {
    let mut c = Checks::new();
    let model = cylinder();
    let dt0 = 5.0 * 0.05 / (1.05f64.powi(40) - 1.0);
    let ts: Vec<f64> = (1..=40).map(|k| dt0 * (1.05f64.powi(k) - 1.0) / 0.05).collect();
    let ts: Vec<f64> = ts.iter().take(39).copied().chain(std::iter::once(5.0)).collect();
    let h = solve_in_time(&model, &[], &ts, &TimeSteppingOptions::new(5e-4)).unwrap();
    let halvings: usize = h.steps.iter().map(|s| s.halvings).sum();
    let largest = h.steps.iter().map(|s| s.max_creep_increment).fold(0.0, f64::max);
    println!(
        "C set-up: {halvings} halvings, largest creep-strain increment {largest:e}, t_end {}",
        h.steps[40].time
    );
    let p0 = polar_points(&model, &h.steps[0].u, &h.steps[0].states);
    let (lp, lm, _) = cylinder_errors(&p0, lame);
    println!("C1 element means against Lame (reported): {lm:.6e}");
    c.at_most("C1 t = 0: point-wise max |sigma - Lame| / p", lp, 1e-2);
    for s in h.steps.iter().skip(1) {
        let pts = polar_points(&model, &s.u, &s.states);
        let (pp, pm, pd) = cylinder_errors(&pts, stationary);
        println!(
            "C2r t = {:.4e} t*: point-wise {pp:.4e}, element means {pm:.4e}, deviatoric {pd:.4e}",
            s.time
        );
    }
    let last = &h.steps[40];
    let pts = polar_points(&model, &last.u, &last.states);
    let (pp, pm, pd) = cylinder_errors(&pts, stationary);
    c.at_most(
        "C2 t = 5 t*: element means, max |mean sigma - mean closed| / p",
        pm,
        2e-2,
    );
    c.at_most(
        "C2d t = 5 t*: point-wise |(sigma_theta - sigma_r) - closed| / p",
        pd,
        2e-2,
    );
    println!("C2r t = 5 t*: point-wise sigma_r, sigma_theta error (reported, no limit): {pp:.6e}");
    c.finish();
}

// ------------------------------------------------------------------------------------------------- B1

#[test]
fn b1_rotating_radial_bar() {
    let mut c = Checks::new();
    let (l, w, omega, n) = (0.05, 5e-4, 250.0, 3.0);
    let s_root = RHO * omega * omega * l * l / 2.0;
    let a = 1.0 / (E * s_root * s_root);
    let mesh = upgrade_to_hex20(&hex_box([0.0; 3], [l, w, w], [10, 1, 1]).unwrap())
        .unwrap()
        .0;
    let tip = (0..mesh.nodes().len())
        .find(|&v| {
            let p = mesh.nodes()[v];
            (p[0] - l).abs() < 1e-12 && p[1] == 0.0 && p[2] == 0.0
        })
        .unwrap();
    let mut model = SolidModel::new(mesh);
    model.add_material("all", creep(a, n, 0.0)).unwrap();
    model.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    model.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    model.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    model
        .add_load(Load::centrifugal([0.0, 0.0, 1.0], [0.0, 0.0, 0.0], omega))
        .unwrap();
    let h = solve_in_time(&model, &[], &times(50, 0.1), &TimeSteppingOptions::new(f64::INFINITY)).unwrap();
    let q = model.dof_map().equation(tip, 0).unwrap();
    let rate = (h.steps[50].u[q] - h.steps[40].u[q]) / (h.steps[50].time - h.steps[40].time);
    let exact = a * (RHO * omega * omega / 2.0).powf(n) * 16.0 / 35.0 * l.powi(7);
    println!(
        "B1: tip u_x elastic {:e}, at 5 s {:e}; rate {rate:e} against {exact:e}",
        h.steps[0].u[q], h.steps[50].u[q]
    );
    c.at_most(
        "B1 |rate / (A (rho Omega^2/2)^3 (16/35) L^7) - 1|",
        (rate / exact - 1.0).abs(),
        5e-3,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- H1, H2, NC1

fn flat(u: &[f64], states: &[ElementState], f: &[f64]) -> Vec<u64> {
    let mut v = u.to_vec();
    for s in states {
        v.extend_from_slice(s.values());
    }
    v.extend_from_slice(f);
    bits(&v)
}

#[test]
fn h1_h2_nc1_halving_and_the_located_local_error() {
    let mut c = Checks::new();
    let model = stretched_cube(creep(relax_a(3.0), 3.0, 0.0));
    let h = solve_in_time(&model, &[], &[1.0], &TimeSteppingOptions::new(1e-4)).unwrap();
    let s = &h.steps[1];
    let r = solve_in_time(
        &model,
        &[],
        &[0.25, 0.5, 0.75, 1.0],
        &TimeSteppingOptions::new(f64::INFINITY),
    )
    .unwrap();
    let e = &r.steps[4];
    c.holds(
        "H1 halved twice, four sub-steps, increment within the limit",
        s.halvings == 2 && s.substeps == 4 && s.max_creep_increment <= 1e-4,
        &format!(
            "halvings {}, sub-steps {}, largest increment {:e}",
            s.halvings, s.substeps, s.max_creep_increment
        ),
    );
    c.holds(
        "H1 bits of the explicitly halved steps",
        flat(&s.u, &s.states, &s.internal_force) == flat(&e.u, &e.states, &e.internal_force),
        "",
    );
    let refused = solve_in_time(&model, &[], &[1.0], &TimeSteppingOptions::new(1e-4).max_halvings(1));
    c.holds(
        "H2 refused after the halvings",
        matches!(&refused, Err(FemError::Solver { solver: "creep time stepping", reason })
            if reason.contains("step 1 of 1") && reason.contains("creep-strain increment")),
        &format!("{:?}", refused.as_ref().err()),
    );

    // NC1: element 1 creeps, element 0 is elastic; one step of 1e40 s.
    let mut mesh = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    mesh.add_element_set("e0", vec![0]).unwrap();
    mesh.add_element_set("e1", vec![1]).unwrap();
    let mut nm = SolidModel::new(mesh);
    let a5 = 1.0 / (4.0 * E * (2e6f64).powi(4));
    nm.add_material("e0", base()).unwrap();
    nm.add_material("e1", creep(a5, 5.0, 0.0)).unwrap();
    nm.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    nm.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    nm.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    nm.add_constraint(Constraint::displacement("xmax", 0, 2e-3)).unwrap();
    let r = solve_in_time(&nm, &[], &[1e40], &TimeSteppingOptions::new(f64::INFINITY));
    let located = matches!(&r, Err(FemError::Element { element: 1, source })
        if matches!(source.as_ref(), FemError::InvalidValue { key, .. } if *key == ITERATION_KEY));
    let text = format!("{:?}", r.as_ref().err());
    c.holds(
        "NC1 located error with element 1, not halved",
        located && !text.contains("halving"),
        &text,
    );
    let point = update(
        &creep(a5, 5.0, 0.0),
        &[1e-3, -3e-4, -3e-4, 0.0, 0.0, 0.0],
        &[0.0; 7],
        &ctx(1e40, 1e40),
    );
    c.holds(
        "NC1 material point refused with the key",
        point.as_ref().err().and_then(key_of) == Some(ITERATION_KEY),
        &format!("{:?}", point.err()),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- Q1

/// A loop of `solve_from_at` over the step ends written here: the instantaneous step at `with_time(0, 0)`, then each step at
/// `with_time(t_k, t_k - t_{k-1})`; the displacement, states and internal force of every step end.
fn loop_of_solve_from_at(model: &SolidModel, ts: &[f64], newton: &NewtonOptions) -> Vec<Vec<u64>> {
    let n = model.dof_map().n_equations();
    let mut u = vec![0.0; n];
    for &q in model.mesh().node_set("xmax").unwrap() {
        u[model.dof_map().equation(q, 0).unwrap()] = EPS0;
    }
    let load = vec![0.0; n];
    let mut states = initial_states(model).unwrap();
    let mut out = Vec::new();
    let mut previous = 0.0;
    for (k, &t) in std::iter::once(&0.0).chain(ts).enumerate() {
        let p = if k == 0 { ctx(0.0, 0.0) } else { ctx(t, t - previous) };
        let eq = solve_from_at(model, &[], &load, &u, &states, newton, &[], &p).unwrap();
        u = eq.u;
        states = eq.states;
        out.push(flat(&u, &states, &eq.internal_force));
        previous = t;
    }
    out
}

#[test]
fn q1_the_driver_is_the_loop() {
    let mut c = Checks::new();
    let creep_model = stretched_cube(creep(relax_a(3.0), 3.0, 0.0));
    let ts = times(20, 0.0125);
    let newton = NewtonOptions::new(1);
    let h = solve_in_time(&creep_model, &[], &ts, &TimeSteppingOptions::new(f64::INFINITY)).unwrap();
    let got: Vec<Vec<u64>> = h
        .steps
        .iter()
        .map(|s| flat(&s.u, &s.states, &s.internal_force))
        .collect();
    c.holds(
        "Q1 creep element, 20 steps",
        got == loop_of_solve_from_at(&creep_model, &ts, &newton),
        "",
    );
    let terms = [(0.3, 0.01), (0.2, 0.1), (0.1, 1.0)]
        .iter()
        .map(|&(w, t)| PronyTerm::new(w, w, t).unwrap())
        .collect();
    let visco = Material::viscoelastic(Material::isotropic_elastic(E, 0.35, RHO, 0.0).unwrap(), terms, None).unwrap();
    let vm = stretched_cube(visco);
    let ts = times(20, 0.02);
    let mut tight = NewtonOptions::new(1);
    tight.residual_tolerance = 1e-13;
    tight.energy_tolerance = 1.0;
    let h = solve_in_time(
        &vm,
        &[],
        &ts,
        &TimeSteppingOptions::new(f64::INFINITY).newton(tight.clone()),
    )
    .unwrap();
    let got: Vec<Vec<u64>> = h
        .steps
        .iter()
        .map(|s| flat(&s.u, &s.states, &s.internal_force))
        .collect();
    c.holds(
        "Q1 viscoelastic element, 20 steps",
        got == loop_of_solve_from_at(&vm, &ts, &tight),
        "",
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- N2, N3

fn not_yet<T>(r: &rcfd_fem::Result<T>, location: &str) -> (bool, String) {
    match r {
        Err(
            e @ FemError::NotYetSupported {
                item,
                variant,
                location: l,
                package,
            },
        ) => (
            *item == "material" && *variant == "Creep" && l.contains(location) && *package == mat_creep::FOLLOW_UP,
            format!("{e}"),
        ),
        Err(e) => (false, format!("{e}")),
        Ok(_) => (false, "accepted".to_string()),
    }
}

#[test]
fn n2_n3_refusals_and_no_time_no_creep() {
    let mut c = Checks::new();
    let mat = creep(relax_a(3.0), 3.0, 0.0);
    let cm = stretched_cube(mat.clone());
    let rows: Vec<(&str, (bool, String))> = vec![
        (
            "solve_static",
            not_yet(&solve_static(&cm, &StaticOptions::default()), "element set 'all'"),
        ),
        (
            "modal_analysis",
            not_yet(&modal_analysis(&cm, &ModalOptions::new(1)), "element set 'all'"),
        ),
        (
            "transient",
            not_yet(
                &Integrator::new(&cm, &[], &TransientOptions::new(1e-3)),
                "element set 'all'",
            ),
        ),
        ("stiffness", not_yet(&stiffness(&cm), "element set 'all'")),
        (
            "elasticity_matrix",
            not_yet(&mat.elasticity_matrix(), "the element-constant elasticity matrix"),
        ),
        (
            "nonlinear::solve",
            not_yet(
                &solve(&cm, &[], &[], &NewtonOptions::new(1)),
                "the nonlinear static solve (no time)",
            ),
        ),
    ];
    for (name, (ok, detail)) in rows {
        c.holds(&format!("N2 {name} refused naming the follow-up"), ok, &detail);
    }
    let ortho = Material::orthotropic(
        [1.4e11, 1.0e10, 1.0e10],
        [0.3, 0.3, 0.45],
        [5.0e9, 5.0e9, 3.5e9],
        1600.0,
        [0.0; 3],
        MaterialAxes::fixed([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]).unwrap(),
    )
    .unwrap();
    let oc = Material::creep(ortho, 1e-26, 3.0, 0.0).unwrap();
    let r = update(&oc, &[1e-4; 6], &[0.0; 7], &ctx(1.0, 0.5));
    let (ok, detail) = match &r {
        Err(e @ FemError::NotYetSupported { location, package, .. }) => (
            location == "an orthotropic base" && *package == mat_creep::FOLLOW_UP,
            format!("{e}"),
        ),
        other => (false, format!("{other:?}")),
    };
    c.holds("N2 orthotropic base refused at the update", ok, &detail);
    let r = update(&creep(1e-26, 0.5, 0.0), &[1e-4; 6], &[0.0; 7], &ctx(1.0, 0.5));
    c.holds(
        "N2 n = 0.5 refused (key n)",
        r.as_ref().err().and_then(key_of) == Some("n"),
        &format!("{:?}", r.err()),
    );
    let mut no_time = PointContext::new();
    no_time.time_increment = Some(0.5);
    let r = update(&mat, &[1e-4; 6], &[0.0; 7], &no_time);
    c.holds(
        "N2 time increment without a time refused (key time)",
        r.as_ref().err().and_then(key_of) == Some("time"),
        &format!("{:?}", r.err()),
    );
    let r = solve_in_time(&cm, &[], &[1.0, 1.0], &TimeSteppingOptions::new(1e-3));
    c.holds(
        "N2 times not increasing refused (key times)",
        r.as_ref().err().and_then(key_of) == Some("times"),
        &format!("{:?}", r.err()),
    );

    // N3: through solve_from (default context, no time) the step and five more attempts: no creep at all.
    let n = cm.dof_map().n_equations();
    let mut u = vec![0.0; n];
    for &q in cm.mesh().node_set("xmax").unwrap() {
        u[cm.dof_map().equation(q, 0).unwrap()] = EPS0;
    }
    let mut tight = NewtonOptions::new(1);
    tight.residual_tolerance = 1e-13;
    tight.energy_tolerance = 1.0;
    let mut states = initial_states(&cm).unwrap();
    let mut zero = true;
    for _ in 0..6 {
        let eq = solve_from(&cm, &[], &vec![0.0; n], &u, &states, &tight, &[]).unwrap();
        u = eq.u;
        states = eq.states;
        zero &= states.iter().all(|s| s.values().iter().all(|v| v.to_bits() == 0));
    }
    c.holds("N3 no time, no creep: every state value 0.0", zero, "");
    c.finish();
}
