//! F-743 (ADR 0664): the rotated partner of the periodic pairs implicit in the momentum predictor
//! (`PimpleSettings::rotational_periodic_implicit`) on the lockstep of ADR 0661 (the sector SS through
//! `periodic_sliding_step_coupled` against the full annulus P): `V*(1, 6)` (one outer iteration, the setting on) against
//! ADR 0661's `V(6, 6)` (six outer iterations, the setting off). Checks J1, K1J, D0, D2, NC1, NC2, G0 (ADR 0641's start, 16
//! steps; fast) and K1s, K1, A1s, G0 (the start `u0` consistent with the inlet, 400 steps; heavy record). The limits are those
//! registered in ADR 0664 before this code ran; each test prints every measured number and then asserts the checks it carries.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::sliding::sliding_step;
use rcfd_fv::sliding_periodic::periodic_sliding_step_coupled;
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{annulus_sector, cylinder_ogrid, Grading};
use rcfd_mesh::sliding::{SlidingInterface, SlidingMesh};
use rcfd_mesh::sliding_periodic::{PeriodicSlidingInterface, PeriodicSlidingMesh};
use rcfd_mesh::Mesh;
use std::f64::consts::{FRAC_PI_3, PI};
use std::sync::Arc;
use std::time::Instant;

const L: f64 = 0.1;
const Z: Vec3 = [0.0, 0.0, 1.0];
const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};
const U_IN: f64 = 1.0;
const RI: f64 = 1.0;
/// ADR 0641's LS: the rotor pitch over 8, 400 steps.
const DT: f64 = PI / 576.0;
const STEPS: usize = 400;
/// J: ADR 0641's start, the first two coincidence steps.
const J_STEPS: usize = 16;

fn pair_ss() -> PeriodicSlidingInterface {
    let a = annulus_sector(24, 8, 1.0, 1.5, FRAC_PI_3, Grading::Uniform, L).unwrap();
    let b = annulus_sector(20, 8, 1.5, 2.0, FRAC_PI_3, Grading::Uniform, L).unwrap();
    PeriodicSlidingInterface::sector(a, "outer", b, "inner", vec3::ZERO, Z, FRAC_PI_3, 1e-8).unwrap()
}

/// ADR 0216's pair P (the full annulus).
fn pair_p() -> SlidingInterface {
    let a = cylinder_ogrid(144, 8, 1.0, 1.5, Grading::Uniform, L).unwrap();
    let b = cylinder_ogrid(120, 8, 1.5, 2.0, Grading::Uniform, L).unwrap();
    SlidingInterface::cylindrical(a, "farfield", b, "cylinder", vec3::ZERO, Z, 1e-8).unwrap()
}

/// PS6 of ADR 0629 with `n_outer = k`, `non_orth_correctors = n` and the opt-in of ADR 0664 `implicit`.
fn settings(k: usize, n: usize, implicit: bool) -> PimpleSettings {
    let mut s = PimpleSettings::piso(0.1);
    s.rotational_periodic_implicit = implicit;
    s.convection = ConvScheme::Central;
    s.non_orth_correctors = n;
    s.n_outer = k;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s
}

fn geom_of(m: &Mesh) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(m.clone())).unwrap())
}

fn xhat(x: Vec3) -> Vec3 {
    vec3::unit([x[0], x[1], 0.0])
}

/// The inlet of ADR 0641, `U_in (1 + 0.5 sin 6 theta) x_hat`.
fn modulation(x: Vec3) -> f64 {
    1.0 + 0.5 * (6.0 * x[1].atan2(x[0])).sin()
}

fn inlet(x: Vec3) -> Vec3 {
    vec3::scale(xhat(x), U_IN * modulation(x))
}

/// The start: `u0 = U_in (1 + 0.5 sin 6 theta) (r_i / r) x_hat` (consistent with the inlet, divergence-free; ADR 0661) or
/// ADR 0641's `U_in (r_i / r) x_hat`.
fn start(x: Vec3, consistent: bool) -> Vec3 {
    let m = if consistent { modulation(x) } else { 1.0 };
    vec3::scale(xhat(x), U_IN * m * RI / x[0].hypot(x[1]))
}

fn per_face(g: &FvGeometry, patch: &str) -> Box<FixedValue> {
    let cf = g.mesh.face_centres();
    Box::new(FixedValue(Profile::PerFace(
        g.mesh.patch(patch).unwrap().faces().map(|k| inlet(cf[k])).collect(),
    )))
}

fn solver(g: Arc<FvGeometry>, inlet_patch: &str, outlet: &str, consistent: bool, s: PimpleSettings) -> PimpleSolver {
    let mut u = VectorField::from_fn("U", &g, |c| start(c, consistent));
    u.set_bc(&g, inlet_patch, per_face(&g, inlet_patch)).unwrap();
    u.set_bc(&g, outlet, Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &g, 0.0);
    p.set_bc(&g, outlet, Box::new(FixedValue::scalar(0.0))).unwrap();
    PimpleSolver::new(g, u, p, s)
}

fn finite_or_inf(x: f64) -> f64 {
    if x.is_nan() {
        f64::INFINITY
    } else {
        x
    }
}

type Fails = Vec<(&'static str, String)>;

fn check(fails: &mut Fails, id: &'static str, ok: bool, what: String) {
    println!("{id}: {what} -> {}", if ok { "met" } else { "MISSED" });
    if !ok {
        fails.push((id, what));
    }
}

/// For every point of `pts`, the nearest cell of `annulus`; the largest distance; whether one-to-one.
fn match_points(pts: &[Vec3], annulus: &Mesh) -> (Vec<usize>, f64, bool) {
    let ca = annulus.cell_centres();
    let mut worst: f64 = 0.0;
    let idx: Vec<usize> = pts
        .iter()
        .map(|c| {
            let (mut best, mut d) = (0, f64::INFINITY);
            for (j, a) in ca.iter().enumerate() {
                let e = vec3::norm(vec3::sub(*c, *a));
                if e < d {
                    d = e;
                    best = j;
                }
            }
            worst = worst.max(d);
            best
        })
        .collect();
    let mut seen = std::collections::HashSet::new();
    let one = idx.iter().all(|j| seen.insert(*j));
    (idx, worst, one)
}

/// One variant: the coupled sector C and the annulus A, both with `settings(k, n, implicit)`.
struct Variant<'a> {
    name: String,
    sc: &'a PeriodicSlidingInterface,
    c_cur: PeriodicSlidingMesh,
    c: PimpleSolver,
    a_sc: SlidingInterface,
    a_cur: SlidingMesh,
    a: PimpleSolver,
}

impl<'a> Variant<'a> {
    fn new(sc: &'a PeriodicSlidingInterface, k: usize, n: usize, implicit: bool, consistent: bool) -> Self {
        let c_cur = sc.mesh_turned(0.0).unwrap();
        let c = solver(
            geom_of(&c_cur.mesh),
            "inner",
            "outer",
            consistent,
            settings(k, n, implicit),
        );
        let a_sc = pair_p();
        let a_cur = a_sc.mesh_at(0.0).unwrap();
        let a = solver(
            geom_of(&a_cur.mesh),
            "cylinder",
            "farfield",
            consistent,
            settings(k, n, implicit),
        );
        Variant {
            name: format!("V{}({k}, {n})", if implicit { "*" } else { "" }),
            sc,
            c_cur,
            c,
            a_sc,
            a_cur,
            a,
        }
    }

    /// One step of both solvers: `u_C - u_A` at every sector cell (against its nearest annulus cell), the match, and C's
    /// remapped flux residual.
    fn step(&mut self, step_no: usize) -> (Vec<Vec3>, f64, bool, f64) {
        let theta = step_no as f64 * DT;
        let new_c = self.sc.mesh_turned(theta).unwrap();
        let st_c = periodic_sliding_step_coupled(&self.c.geom, &self.c_cur, &new_c, &self.c.phi, &self.c.u.values, DT)
            .unwrap();
        let new_a = self.a_sc.mesh_at(theta).unwrap();
        let st_a = sliding_step(&self.a.geom, &self.a_cur, &new_a, &self.a.phi, &self.a.u.values, DT).unwrap();
        let flux_residual = finite_or_inf(st_c.flux_residual);
        let gc = st_c.geom.clone();
        self.c.u.set_bc(&gc, "inner", per_face(&gc, "inner")).unwrap();
        self.c.step_sliding_periodic(DT, st_c, Vec::new()).unwrap();
        self.c_cur = new_c;
        let ga = st_a.geom.clone();
        self.a.u.set_bc(&ga, "cylinder", per_face(&ga, "cylinder")).unwrap();
        self.a.step_sliding(DT, st_a, Vec::new()).unwrap();
        self.a_cur = new_a;
        let (idx, match_d, match_one) = match_points(self.c.geom.mesh.cell_centres(), &self.a.geom.mesh);
        let delta = idx
            .iter()
            .enumerate()
            .map(|(c, &j)| vec3::sub(self.c.u.values[c], self.a.u.values[j]))
            .collect();
        (delta, match_d, match_one, flux_residual)
    }
}

fn max_norm(x: &[Vec3]) -> f64 {
    x.iter().fold(0.0f64, |m, v| m.max(finite_or_inf(vec3::norm(*v)))) / U_IN
}

fn max_diff(x: &[Vec3], y: &[Vec3]) -> f64 {
    x.iter()
        .zip(y)
        .fold(0.0f64, |m, (a, b)| m.max(finite_or_inf(vec3::norm(vec3::sub(*a, *b)))))
        / U_IN
}

/// The bits of the annulus solver's velocity and pressure.
fn annulus_bits(v: &Variant) -> Vec<u64> {
    let mut b: Vec<u64> =
        v.a.u
            .values
            .iter()
            .flat_map(|u| u.iter().map(|x| x.to_bits()))
            .collect();
    b.extend(v.a.p.values.iter().map(|x| x.to_bits()));
    b
}

/// The lockstep of the variants (`(n_outer, non_orth_correctors, implicit)`) from the start `consistent` over `steps` steps.
struct Run {
    names: Vec<String>,
    /// `G_V(n)` of every variant at every step (index `n - 1`).
    g: Vec<Vec<f64>>,
    /// The largest over the steps of `max_c |delta_V - delta_W|` of every pair of variants.
    pair: Vec<Vec<f64>>,
    /// The largest over the steps of `|G_0(n) - G_1(n)|`, its signed value and its step.
    k1s: (f64, f64, usize),
    match_d: f64,
    match_one: bool,
    /// C's flux residual of variant 0 from step 2.
    a1s: f64,
    /// Whether the annulus of variant 0 and that of variant `annulus_twin` were equal bit for bit on every step.
    annulus_same: Option<bool>,
    elapsed: f64,
}

fn run(vs: &[(usize, usize, bool)], consistent: bool, steps: usize, annulus_twin: Option<usize>) -> Run {
    let t0 = Instant::now();
    let sc = pair_ss();
    let mut var: Vec<Variant> = vs
        .iter()
        .map(|&(k, n, imp)| Variant::new(&sc, k, n, imp, consistent))
        .collect();
    let nv = var.len();
    let mut out = Run {
        names: var.iter().map(|v| v.name.clone()).collect(),
        g: vec![Vec::new(); nv],
        pair: vec![vec![0.0; nv]; nv],
        k1s: (0.0, 0.0, 0),
        match_d: 0.0,
        match_one: true,
        a1s: 0.0,
        annulus_same: annulus_twin.map(|_| true),
        elapsed: 0.0,
    };
    for n in 1..=steps {
        let outs: Vec<(Vec<Vec3>, f64, bool, f64)> = var.iter_mut().map(|v| v.step(n)).collect();
        for (i, o) in outs.iter().enumerate() {
            out.match_d = out.match_d.max(o.1);
            out.match_one &= o.2;
            out.g[i].push(max_norm(&o.0));
            for (j, p) in outs.iter().enumerate().skip(i + 1) {
                let d = max_diff(&o.0, &p.0);
                out.pair[i][j] = out.pair[i][j].max(d);
                out.pair[j][i] = out.pair[i][j];
            }
        }
        if nv > 1 {
            let d = out.g[0][n - 1] - out.g[1][n - 1];
            if finite_or_inf(d.abs()) > out.k1s.0 {
                out.k1s = (finite_or_inf(d.abs()), d, n);
            }
        }
        if n >= 2 {
            out.a1s = out.a1s.max(outs[0].3);
        }
        if let (Some(t), Some(same)) = (annulus_twin, out.annulus_same.as_mut()) {
            *same &= annulus_bits(&var[0]) == annulus_bits(&var[t]);
        }
    }
    out.elapsed = t0.elapsed().as_secs_f64();
    for (i, name) in out.names.iter().enumerate() {
        let (step, gmax) = out.g[i]
            .iter()
            .enumerate()
            .fold((0, 0.0f64), |b, (s, &x)| if x > b.1 { (s + 1, x) } else { b });
        println!(
            "{name}: G at step 1 {:.9e}, largest {gmax:.4e} U_in at step {step}",
            out.g[i][0]
        );
    }
    for i in 0..nv {
        for j in i + 1..nv {
            println!(
                "max |delta_{} - delta_{}| over the steps {:.4e}",
                out.names[i], out.names[j], out.pair[i][j]
            );
        }
    }
    out
}

/// J1, K1J, D0, D2, NC1, NC2, G0 of ADR 0664: ADR 0641's start over 16 steps, the variants `V*(1, 6)`, `V(6, 6)`, `V(1, 6)`.
#[test]
fn implicit_pair_from_the_start_of_adr_0641() {
    let r = run(&[(1, 6, true), (6, 6, false), (1, 6, false)], false, J_STEPS, Some(2));
    println!("elapsed {:.1} s", r.elapsed);
    let mut fails = Fails::new();
    check(
        &mut fails,
        "J1",
        r.g[0][0] <= 5e-3,
        format!("V*(1, 6) step 1 from ADR 0641's start {:.4e} U_in (5e-3)", r.g[0][0]),
    );
    check(
        &mut fails,
        "K1J",
        r.pair[0][1] <= 2e-3,
        format!(
            "max |delta_* - delta_(6,6)| over {J_STEPS} steps {:.4e} U_in (2e-3)",
            r.pair[0][1]
        ),
    );
    check(
        &mut fails,
        "D0",
        (r.g[2][0] - 3.494135e-2).abs() <= 1e-8,
        format!(
            "V(1, 6) (setting off) step 1 {:.9e} U_in (ADR 0661's 3.494135e-2 within 1e-8)",
            r.g[2][0]
        ),
    );
    check(
        &mut fails,
        "D2",
        r.annulus_same == Some(true),
        format!(
            "the annulus with the setting on equals the annulus with it off bit for bit on every step: {:?}",
            r.annulus_same
        ),
    );
    check(
        &mut fails,
        "NC1",
        r.pair[2][1] >= 1e-2,
        format!(
            "control: max |delta_(1,6) - delta_(6,6)| (setting off) {:.4e} U_in (>= 1e-2)",
            r.pair[2][1]
        ),
    );
    check(
        &mut fails,
        "NC2",
        r.g[2][0] >= 5e-3,
        format!("control: V(1, 6) (setting off) step 1 {:.4e} U_in (>= 5e-3)", r.g[2][0]),
    );
    check(
        &mut fails,
        "G0",
        r.match_d <= 1e-9 && r.match_one,
        format!(
            "nearest annulus cell {:.2e} (1e-9), one-to-one {}",
            r.match_d, r.match_one
        ),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

/// K1s, K1, A1s, G0 of ADR 0664: the start `u0` consistent with the inlet over 400 steps, `V*(1, 6)` against `V(6, 6)`.
#[test]
#[ignore = "heavy record (about 550 s estimated), run once (ADR 0664); implicit_pair_from_the_start_of_adr_0641 covers the same path fast"]
fn implicit_pair_against_six_outer_iterations_from_u0() {
    let r = run(&[(1, 6, true), (6, 6, false)], true, STEPS, None);
    println!("elapsed {:.1} s", r.elapsed);
    let mut fails = Fails::new();
    check(
        &mut fails,
        "K1s",
        r.k1s.0 <= 1.5e-3,
        format!(
            "max over the {STEPS} steps of |G_*(n) - G_(6,6)(n)| {:.4e} U_in (signed {:.4e}, step {}) (1.5e-3 each step)",
            r.k1s.0, r.k1s.1, r.k1s.2
        ),
    );
    check(
        &mut fails,
        "K1",
        r.pair[0][1] <= 1.5e-3,
        format!(
            "max over the steps of max_c |delta_* - delta_(6,6)| {:.4e} U_in (1.5e-3 each step)",
            r.pair[0][1]
        ),
    );
    check(
        &mut fails,
        "A1s",
        r.a1s <= 1e-12,
        format!("C flux_residual of V*(1, 6), steps 2 to {STEPS}: {:.2e} (1e-12)", r.a1s),
    );
    check(
        &mut fails,
        "G0",
        r.match_d <= 1e-9 && r.match_one,
        format!(
            "nearest annulus cell {:.2e} (1e-9), one-to-one {}",
            r.match_d, r.match_one
        ),
    );
    assert!(fails.is_empty(), "{fails:?}");
}
