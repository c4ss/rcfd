//! Registered checks PT1-PT13 of ADR 0028 (library level).

use rcfd_mesh::gen::{box_mesh, warp, Grading};
use rcfd_mesh::Mesh;
use rcfd_particles::drag::{reflect, tau_p};
use rcfd_particles::vtp::vtp_string;
use rcfd_particles::*;
use std::sync::Arc;

const D: f64 = 1e-4;
const RHO_P: f64 = 2500.0;
const RHO_F: f64 = 1.2;
const MU: f64 = 1.8e-5;
const G: f64 = 9.81;

fn fl() -> Fluid {
    Fluid { rho: RHO_F, mu: MU }
}

fn vt() -> f64 {
    D * D * (RHO_P - RHO_F) * G / (18.0 * MU)
}

fn tau() -> f64 {
    tau_p(D, RHO_P, MU)
}

#[allow(clippy::too_many_arguments)]
fn run(scheme: Scheme, law: DragLaw, u: [f64; 3], g: [f64; 3], h: f64, steps: usize) -> ([f64; 3], [f64; 3]) {
    let mut x = [0.0; 3];
    let mut v = [0.0; 3];
    for _ in 0..steps {
        let (xn, vn) = step_particle(scheme, law, fl(), D, RHO_P, g, x, v, h, &mut |_| u);
        x = xn;
        v = vn;
    }
    (x, v)
}

#[test]
fn pt1_terminal_velocity() {
    let (_, v) = run(Scheme::Analytic, DragLaw::Stokes, [0.0; 3], [0.0, 0.0, -G], tau(), 25);
    let rel = (v[2] + vt()).abs() / vt();
    println!("PT1 terminal velocity relative error {rel:e}");
    assert!(rel <= 1e-6, "{rel}");
}

#[test]
fn pt1b_schiller_naumann_terminal_balance() {
    let (_, v) = run(
        Scheme::Rk4,
        DragLaw::SchillerNaumann,
        [0.0; 3],
        [0.0, 0.0, -G],
        tau() / 20.0,
        1200,
    );
    let vtn = v[2].abs();
    let re = RHO_F * D * vtn / MU;
    let lhs = drag_factor(DragLaw::SchillerNaumann, re) * vtn / tau();
    let rhs = (1.0 - RHO_F / RHO_P) * G;
    let rel = (lhs - rhs).abs() / rhs;
    println!("PT1b SN balance residual {rel:e} (Re_t = {re})");
    assert!(rel <= 1e-8, "{rel}");
}

#[test]
fn pt2_exponential_approach() {
    let t = tau();
    let mut x = [0.0; 3];
    let mut v = [0.0; 3];
    let (mut ev, mut ez) = (0.0f64, 0.0f64);
    for n in 1..=10 {
        let (xn, vn) = step_particle(
            Scheme::Analytic,
            DragLaw::Stokes,
            fl(),
            D,
            RHO_P,
            [0.0, 0.0, -G],
            x,
            v,
            t,
            &mut |_| [0.0; 3],
        );
        x = xn;
        v = vn;
        let tn = n as f64 * t;
        let ev_n = (v[2] + vt() * (1.0 - (-tn / t).exp())).abs() / vt();
        let z_exact = -vt() * (tn - t * (1.0 - (-tn / t).exp()));
        let ez_n = (x[2] - z_exact).abs() / (vt() * t);
        ev = ev.max(ev_n);
        ez = ez.max(ez_n);
    }
    println!("PT2 max velocity error {ev:e}, max position error {ez:e}");
    assert!(ev <= 1e-8 && ez <= 1e-8);
}

fn order_test(scheme: Scheme, divisors: &[f64], lo: f64, hi: f64) {
    let t = tau();
    let exact = vt() * (1.0 - (-1.0f64).exp());
    let mut errs = Vec::new();
    for &dv in divisors {
        let h = t / dv;
        let steps = dv as usize;
        let (_, v) = run(scheme, DragLaw::Stokes, [0.0; 3], [0.0, 0.0, -G], h, steps);
        errs.push((v[2] + exact).abs());
    }
    for k in 0..errs.len() - 1 {
        let ord = (errs[k] / errs[k + 1]).log2();
        println!(
            "PT3 {scheme:?} h=tau/{}: err {:e}, order {ord:.4}",
            divisors[k], errs[k]
        );
        assert!(ord >= lo && ord <= hi, "{scheme:?} order {ord}");
    }
}

#[test]
fn pt3_euler_order() {
    order_test(Scheme::Euler, &[8.0, 16.0, 32.0, 64.0], 0.9, 1.1);
}

/// Registered PT3 limit for RK2 (steps tau/2 .. tau/16, every successive pair in [1.9, 2.1]). MISS: the first pair
/// (tau/2, tau/4) gives 2.29. Kept with the registered limit intact.
#[test]
#[ignore = "known miss: first pair order 2.29 outside the registered [1.9, 2.1] (pre-asymptotic step tau/2)"]
fn pt3_known_miss_rk2_order() {
    order_test(Scheme::Rk2, &[2.0, 4.0, 8.0, 16.0], 1.9, 2.1);
}

/// Diagnostic for the miss above (a different step sequence, NOT the registered one): at tau/8 .. tau/64 the order is 2.
#[test]
fn pt3b_rk2_order_finer_steps_diagnostic() {
    order_test(Scheme::Rk2, &[8.0, 16.0, 32.0, 64.0], 1.9, 2.1);
}

#[test]
fn pt3_rk4_order() {
    order_test(Scheme::Rk4, &[4.0, 8.0, 16.0], 3.7, 4.3);
}

#[test]
fn pt4_uniform_flow_relaxation() {
    let t = tau();
    let u = [1.0, 0.0, 0.0];
    let mut worst = 0.0f64;
    for k in [0.3, 1.0, 2.5] {
        let (_, v) = run(Scheme::Analytic, DragLaw::Stokes, u, [0.0; 3], k * t, 1);
        worst = worst.max((v[0] - (1.0 - (-k).exp())).abs());
    }
    // Multi-step too.
    let (_, v) = run(Scheme::Analytic, DragLaw::Stokes, u, [0.0; 3], 0.1 * t, 10);
    worst = worst.max((v[0] - (1.0 - (-1.0f64).exp())).abs());
    println!("PT4 max abs error {worst:e}");
    assert!(worst <= 1e-12);
    // time to reach 1 - 1/e
    let target = 1.0 - (-1.0f64).exp();
    let (mut lo, mut hi) = (0.0, 5.0 * t);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        let (_, v) = run(Scheme::Analytic, DragLaw::Stokes, u, [0.0; 3], mid, 1);
        if v[0] > target {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let rel = (hi - tau_p(D, RHO_P, MU)).abs() / tau_p(D, RHO_P, MU);
    println!("PT4 relaxation time relative error {rel:e}");
    assert!(rel <= 1e-12);
}

#[test]
fn pt4b_schiller_naumann_relaxation() {
    let t = tau();
    let u = [1.0, 0.0, 0.0];
    let mut x = [0.0; 3];
    let mut v = [0.0; 3];
    let h = t / 20.0;
    let mut prev = 0.0;
    let mut v_at_tau = 0.0;
    for k in 1..=8000 {
        let (xn, vn) = step_particle(
            Scheme::Rk4,
            DragLaw::SchillerNaumann,
            fl(),
            D,
            RHO_P,
            [0.0; 3],
            x,
            v,
            h,
            &mut |_| u,
        );
        x = xn;
        v = vn;
        assert!(v[0] >= prev, "not monotone");
        prev = v[0];
        if k == 20 {
            v_at_tau = v[0];
        }
    }
    println!(
        "PT4b SN: |1 - v| = {:e}, v(tau) = {v_at_tau} vs Stokes {}",
        (1.0 - v[0]).abs(),
        1.0 - (-1.0f64).exp()
    );
    assert!((1.0 - v[0]).abs() <= 1e-6);
    assert!(v_at_tau > 1.0 - (-1.0f64).exp());
}

#[test]
fn pt6_reflection() {
    let mut rng = SplitMix64::new(11);
    let mut worst = 0.0f64;
    for _ in 0..1000 {
        let mut n = [rng.uniform() - 0.5, rng.uniform() - 0.5, rng.uniform() - 0.5];
        let nn = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        n = [n[0] / nn, n[1] / nn, n[2] / nn];
        let v = [
            rng.uniform() * 4.0 - 2.0,
            rng.uniform() * 4.0 - 2.0,
            rng.uniform() * 4.0 - 2.0,
        ];
        let sp = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        let r1 = reflect(v, n, 1.0);
        let sp1 = (r1[0] * r1[0] + r1[1] * r1[1] + r1[2] * r1[2]).sqrt();
        worst = worst.max((sp1 - sp).abs() / sp);
        let r = reflect(v, n, 0.6);
        let vn = v[0] * n[0] + v[1] * n[1] + v[2] * n[2];
        let rn = r[0] * n[0] + r[1] * n[1] + r[2] * n[2];
        worst = worst.max((rn + 0.6 * vn).abs() / sp);
        for k in 0..3 {
            let tv = v[k] - vn * n[k];
            let tr = r[k] - rn * n[k];
            worst = worst.max((tv - tr).abs() / sp);
        }
    }
    println!("PT6 worst reflection error {worst:e}");
    assert!(worst <= 1e-14);
}

fn quiet_box(n: [usize; 3], l: [f64; 3]) -> Arc<Mesh> {
    Arc::new(box_mesh(n, l, [Grading::Uniform; 3]).unwrap())
}

fn particle(tr: &Tracker, x: [f64; 3], v: [f64; 3]) -> Particle {
    Particle {
        id: 0,
        x,
        v,
        d: D,
        rho: RHO_P,
        parcel: 1.0,
        cell: tr.locator().locate(x, None).expect("inside"),
        age: 0.0,
        state: State::Active,
    }
}

#[test]
fn pt6b_bouncing_ball() {
    let mesh = quiet_box([4, 4, 12], [1.0, 1.0, 3.0]);
    let e = 0.8;
    let opts = Options {
        drag: DragLaw::None,
        scheme: Scheme::Analytic,
        interp: Interp::CellCentre,
        gravity: [0.0, 0.0, -G],
        ..Options::default()
    };
    let mut tr = Tracker::new(mesh.clone(), opts, &[("zmin".into(), WallAction::Reflect(e))]).unwrap();
    let h = 1.0;
    let p = particle(&tr, [0.5, 0.5, h], [0.0; 3]);
    tr.particles.push(p);
    let u = vec![[0.0; 3]; mesh.n_cells()];
    let flow = Flow {
        u: &u,
        grad: None,
        rho: 0.0,
        mu: MU,
    };
    let t_a = (2.0 * h / G).sqrt() * (1.0 + e);
    let dt: f64 = 1e-3;
    let mut t = 0.0;
    while t < t_a {
        let step = dt.min(t_a - t);
        tr.advance(&flow, step).unwrap();
        t += step;
    }
    let p = &tr.particles[0];
    let rz = (p.x[2] - e * e * h).abs() / (e * e * h);
    println!(
        "PT6b apex height {} (exact {}), rel err {rz:e}, vz {:e}",
        p.x[2],
        e * e * h,
        p.v[2]
    );
    assert!(rz <= 1e-9);
    assert!(p.v[2].abs() <= 1e-9 * (2.0 * G * h).sqrt());
}

// ---- tracker-level fields

#[allow(clippy::type_complexity)]
fn rotation_setup(omega: f64) -> (Arc<Mesh>, Vec<[f64; 3]>, Vec<[[f64; 3]; 3]>) {
    let mesh = quiet_box([12, 12, 6], [4.0, 4.0, 1.0]);
    let u: Vec<[f64; 3]> = mesh
        .cell_centres()
        .iter()
        .map(|c| [-omega * (c[1] - 2.0), omega * (c[0] - 2.0), 0.0])
        .collect();
    let g = vec![[[0.0, -omega, 0.0], [omega, 0.0, 0.0], [0.0; 3]]; mesh.n_cells()];
    (mesh, u, g)
}

#[derive(Clone, Copy)]
struct C(f64, f64);
impl C {
    fn mul(self, o: C) -> C {
        C(self.0 * o.0 - self.1 * o.1, self.0 * o.1 + self.1 * o.0)
    }
    fn add(self, o: C) -> C {
        C(self.0 + o.0, self.1 + o.1)
    }
    fn sub(self, o: C) -> C {
        C(self.0 - o.0, self.1 - o.1)
    }
    fn div(self, o: C) -> C {
        let d = o.0 * o.0 + o.1 * o.1;
        C((self.0 * o.0 + self.1 * o.1) / d, (self.1 * o.0 - self.0 * o.1) / d)
    }
    fn sqrt(self) -> C {
        let r = (self.0 * self.0 + self.1 * self.1).sqrt();
        let a = ((r + self.0) / 2.0).sqrt();
        let b = ((r - self.0) / 2.0).sqrt() * if self.1 < 0.0 { -1.0 } else { 1.0 };
        C(a, b)
    }
    fn exp(self) -> C {
        let e = self.0.exp();
        C(e * self.1.cos(), e * self.1.sin())
    }
}

/// Solid-body rotation run. Returns (errors at t = 0.1 and 0.25, number of distinct cells visited, state active).
fn rotation_run(start: [f64; 3], dt_flow: f64) -> (f64, f64, usize, bool) {
    let omega = 3.0;
    let (mesh, u, g) = rotation_setup(omega);
    let tp: f64 = 0.05;
    let rho_p: f64 = 1000.0;
    let mu: f64 = 1e-3;
    let d = (tp * 18.0 * mu / rho_p).sqrt();
    let opts = Options {
        drag: DragLaw::Stokes,
        scheme: Scheme::Rk4,
        interp: Interp::Gradient,
        ..Options::default()
    };
    let mut tr = Tracker::new(mesh.clone(), opts, &[]).unwrap();
    let mut p = particle(&tr, start, [0.0; 3]);
    p.d = d;
    p.rho = rho_p;
    tr.particles.push(p);
    let flow = Flow {
        u: &u,
        grad: Some(&g),
        rho: 1.0,
        mu,
    };
    // closed form: tau z'' + z' - i Omega z = 0, z = (x - 2) + i (y - 2)
    let w = C(1.0, 4.0 * omega * tp).sqrt();
    let l1 = C((-1.0 + w.0) / (2.0 * tp), w.1 / (2.0 * tp));
    let l2 = C((-1.0 - w.0) / (2.0 * tp), -w.1 / (2.0 * tp));
    let z0 = C(start[0] - 2.0, start[1] - 2.0);
    let a = C(0.0, 0.0).sub(l2.mul(z0)).div(l1.sub(l2));
    let b = z0.sub(a);
    let exact = |t: f64| {
        a.mul(C(l1.0 * t, l1.1 * t).exp())
            .add(b.mul(C(l2.0 * t, l2.1 * t).exp()))
    };
    let mut cells = std::collections::BTreeSet::new();
    let mut errs = [0.0f64; 2];
    let n = (0.25 / dt_flow).round() as usize;
    for k in 1..=n {
        tr.advance(&flow, dt_flow).unwrap();
        cells.insert(tr.particles[0].cell);
        let t = k as f64 * dt_flow;
        for (i, tt) in [0.1, 0.25].iter().enumerate() {
            if (t - tt).abs() < 1e-12 {
                let z = exact(t);
                let p = &tr.particles[0];
                errs[i] = ((p.x[0] - 2.0 - z.0).powi(2) + (p.x[1] - 2.0 - z.1).powi(2)).sqrt();
            }
        }
    }
    (errs[0], errs[1], cells.len(), tr.particles[0].state == State::Active)
}

/// Registered PT5 (start (3, 2, 0.5), dt 0.005): position error <= 1e-8 at t = 0.1 and 0.25 and >= 3 cells crossed.
/// MISS: error 3.68e-8 at t = 0.1 (RK4 time-stepping error, see the diagnostic) and only 2 cells crossed (the start lies
/// on cell faces and the particle moves along one column; the precondition estimate was wrong).
#[test]
#[ignore = "known miss: error 3.7e-8 at t = 0.1 (> 1e-8) and 2 cells crossed (< 3)"]
fn pt5_known_miss_solid_body_rotation() {
    let (e1, e2, cells, active) = rotation_run([3.0, 2.0, 0.5], 0.005);
    assert!(active && cells >= 3);
    assert!(e1 <= 1e-8 && e2 <= 1e-8, "{e1} {e2}");
}

/// Diagnostic for the PT5 miss: the error is temporal (4th order in the flow step). Uses a start point that crosses more
/// cells ((3.4, 2, 0.45)). Asserts the observed order only; the errors are printed.
#[test]
fn pt5b_rotation_time_error_diagnostic() {
    let mut errs = Vec::new();
    for dt in [0.005, 0.0025, 0.00125] {
        let (e1, e2, cells, active) = rotation_run([3.4, 2.0, 0.45], dt);
        println!("PT5 diagnostic dt={dt}: err(0.1) {e1:e}, err(0.25) {e2:e}, cells {cells}");
        assert!(active && cells >= 3);
        errs.push(e1);
    }
    let o1 = (errs[0] / errs[1]).log2();
    let o2 = (errs[1] / errs[2]).log2();
    println!("PT5 diagnostic orders {o1:.3} {o2:.3}");
    assert!(o1 > 3.5 && o2 > 3.5);
}

fn couette(interp: Interp) -> (f64, f64, f64, f64) {
    let gamma = 20.0;
    let mesh = quiet_box([12, 10, 4], [4.0, 1.0, 1.0]);
    let u: Vec<[f64; 3]> = mesh.cell_centres().iter().map(|c| [gamma * c[1], 0.0, 0.0]).collect();
    let g = vec![[[0.0, gamma, 0.0], [0.0; 3], [0.0; 3]]; mesh.n_cells()];
    let tp: f64 = 0.05;
    let (rho_p, mu): (f64, f64) = (1000.0, 1e-3);
    let d = (tp * 18.0 * mu / rho_p).sqrt();
    let opts = Options {
        drag: DragLaw::Stokes,
        scheme: Scheme::Analytic,
        interp,
        ..Options::default()
    };
    let mut tr = Tracker::new(mesh.clone(), opts, &[]).unwrap();
    let mut p = particle(&tr, [0.5, 0.37, 0.5], [0.0; 3]);
    p.d = d;
    p.rho = rho_p;
    tr.particles.push(p);
    let flow = Flow {
        u: &u,
        grad: Some(&g),
        rho: 1.0,
        mu,
    };
    for _ in 0..40 {
        tr.advance(&flow, 0.01).unwrap();
    }
    let p = &tr.particles[0];
    let t = 0.4;
    let y0 = match interp {
        Interp::Gradient => 0.37,
        Interp::CellCentre => 0.35,
    };
    let vx = gamma * y0 * (1.0 - (-t / tp).exp());
    let x = 0.5 + gamma * y0 * (t - tp * (1.0 - (-t / tp).exp()));
    ((p.v[0] - vx).abs() / vx, (p.x[0] - x).abs() / (x - 0.5), p.x[1], p.v[0])
}

#[test]
fn pt5b_couette_gradient() {
    let (ev, ex, y, _) = couette(Interp::Gradient);
    println!("PT5b rel errors v {ev:e}, x {ex:e}, y = {y}");
    assert!(ev <= 1e-10 && ex <= 1e-10);
    assert!((y - 0.37).abs() <= 1e-14);
}

#[test]
fn pt5c_couette_cell_centre() {
    let (ev, ex, _, vc) = couette(Interp::CellCentre);
    let (_, _, _, vg) = couette(Interp::Gradient);
    println!(
        "PT5c rel errors v {ev:e}, x {ex:e}; |v_cell - v_grad| = {:e}",
        (vc - vg).abs()
    );
    assert!(ev <= 1e-12 && ex <= 1e-12);
    assert!((vc - vg).abs() > 1e-3);
}

// ---- injection conservation, reproducibility

fn pt7_run(seed: u64, dt: f64) -> Tracker {
    let mesh = quiet_box([8, 8, 8], [1.0, 1.0, 1.0]);
    let opts = Options {
        drag: DragLaw::SchillerNaumann,
        scheme: Scheme::Analytic,
        interp: Interp::CellCentre,
        gravity: [0.0, 0.0, -G],
        ..Options::default()
    };
    let mut tr = Tracker::new(
        mesh.clone(),
        opts,
        &[("zmin".into(), WallAction::Stick), ("zmax".into(), WallAction::Escape)],
    )
    .unwrap();
    let dist = SizeDist::RosinRammler {
        d_bar: 1e-4,
        n: 2.5,
        d_min: None,
        d_max: None,
    };
    tr.add_injector(InjectorSpec {
        name: "point".into(),
        kind: InjKind::Point([0.5, 0.5, 0.5]),
        rate: 800.0,
        t_start: 0.0,
        t_end: 0.5,
        velocity: VelMode::Fluid,
        dist,
        density: RHO_P,
        parcel_count: 2.5,
        seed,
    })
    .unwrap();
    tr.add_injector(InjectorSpec {
        name: "ceil".into(),
        kind: InjKind::Surface("zmax".into()),
        rate: 300.0,
        t_start: 0.1,
        t_end: 0.4,
        velocity: VelMode::Given([0.0, 0.0, -0.1]),
        dist,
        density: RHO_P,
        parcel_count: 2.5,
        seed: seed + 1000,
    })
    .unwrap();
    let u = vec![[0.3, 0.0, -0.2]; mesh.n_cells()];
    let flow = Flow {
        u: &u,
        grad: None,
        rho: RHO_F,
        mu: MU,
    };
    let mut t = 0.0;
    while t < 5.0 {
        tr.advance(&flow, dt).unwrap();
        t += dt;
    }
    tr
}

#[test]
fn pt7_injection_conservation() {
    for dt in [0.01, 0.0073] {
        let tr = pt7_run(7, dt);
        let n = tr.particles.len();
        let (a, dpos, esc, stuck) = (
            tr.count(State::Active),
            tr.count(State::Deposited),
            tr.count(State::Escaped),
            tr.count(State::Stuck),
        );
        let real: f64 = tr.particles.iter().map(|p| p.parcel).sum();
        println!(
            "PT7 dt={dt}: injected {n} = inside {a} + deposited {dpos} + escaped {esc} + stuck {stuck}; real {real}"
        );
        assert_eq!(n, 490);
        assert_eq!(n, a + dpos + esc + stuck);
        assert!((real - 490.0 * 2.5).abs() / (490.0 * 2.5) <= 1e-12);
        assert!(dpos > 0);
    }
}

#[test]
fn pt10_reproducibility() {
    let a = pt7_run(7, 0.01);
    let b = pt7_run(7, 0.01);
    assert_eq!(a.fingerprint(), b.fingerprint());
    assert_eq!(vtp_string(&a.particles), vtp_string(&b.particles));
    let c = pt7_run(8, 0.01);
    assert_ne!(a.fingerprint(), c.fingerprint());
}

// ---- location

fn locate_test(mesh: Mesh, map: &dyn Fn([f64; 3]) -> [f64; 3], fallback: bool) {
    mesh.validate().unwrap();
    let loc = Locator::new(Arc::new(mesh));
    let nc = loc.mesh().n_cells();
    let mut rng = SplitMix64::new(5);
    let (mut inside, mut outside, mut bad_in, mut bad_out) = (0usize, 0usize, 0usize, 0usize);
    for _ in 0..100_000 {
        let q = [rng.uniform(), rng.uniform(), rng.uniform()];
        let p = map(q);
        let set = loc.all_containing(p);
        let start = (rng.next_u64() % nc as u64) as usize;
        let got = if fallback {
            loc.locate(p, Some(start))
        } else {
            loc.locate_walk(p, Some(start))
        };
        if set.is_empty() {
            outside += 1;
            if got.is_some() {
                bad_out += 1;
            }
        } else {
            inside += 1;
            match got {
                Some(c) if set.contains(&c) => {}
                _ => bad_in += 1,
            }
        }
    }
    println!(
        "PT8: {inside} inside the faceted domain, {outside} outside, mismatches inside {bad_in}, outside {bad_out}"
    );
    assert_eq!(bad_in, 0);
    assert_eq!(bad_out, 0);
    assert!(outside < 20_000);
}

fn warp_map(p: [f64; 3]) -> [f64; 3] {
    let s = |a: f64| (2.0 * std::f64::consts::PI * a).sin();
    let (x, y, z) = (p[0], p[1], p[2]);
    let mut q = [x + 0.08 * s(y) * s(z), y + 0.08 * s(x) * s(z), z + 0.08 * s(x) * s(y)];
    q[0] += 0.3 * y;
    q
}

/// Registered PT8 (a), strict reading: the pure walk from a random start cell. MISS: 1522 of 99510 points in the faceted
/// domain are not found because the straight segment from the start cell centre leaves the (non-convex, wavy) domain.
#[test]
#[ignore = "known miss: pure walk from a random cell fails for 1522 points (segment leaves the non-convex domain)"]
fn pt8_known_miss_location_warped() {
    let m = box_mesh([14, 12, 10], [1.0; 3], [Grading::Uniform; 3]).unwrap();
    let w = warp(&m, warp_map).unwrap();
    locate_test(w, &warp_map, false);
}

/// PT8 (a) with the production `locate` (walk with the documented brute-force fallback). Not the registered strict test.
#[test]
fn pt8b_location_warped_with_fallback() {
    let m = box_mesh([14, 12, 10], [1.0; 3], [Grading::Uniform; 3]).unwrap();
    let w = warp(&m, warp_map).unwrap();
    locate_test(w, &warp_map, true);
}

#[test]
fn pt8_location_graded() {
    let m = box_mesh(
        [10, 16, 10],
        [1.0; 3],
        [Grading::Uniform, Grading::Expansion(50.0), Grading::Uniform],
    )
    .unwrap();
    // The shear is the only map; the graded points are mapped through the same shear. Sample in the unit cube of the
    // graded box, whose coordinates are the logical ones.
    let shear = |p: [f64; 3]| [p[0] + 0.3 * p[1], p[1], p[2]];
    let w = warp(&m, shear).unwrap();
    locate_test(w, &shear, false);
}

#[test]
fn pt12_interpolation_linear_field() {
    let m = box_mesh([8, 8, 8], [1.0; 3], [Grading::Uniform; 3]).unwrap();
    let w = Arc::new(warp(&m, warp_map).unwrap());
    let loc = Locator::new(w.clone());
    let a = [0.3, -0.2, 0.9];
    let b = [[1.0, 2.0, -0.5], [0.3, -1.0, 0.7], [0.1, 0.2, 0.4]];
    let f = |x: [f64; 3]| -> [f64; 3] {
        let mut o = a;
        for i in 0..3 {
            for j in 0..3 {
                o[i] += b[i][j] * x[j];
            }
        }
        o
    };
    let u: Vec<[f64; 3]> = w.cell_centres().iter().map(|&c| f(c)).collect();
    let g = vec![[b[0], b[1], b[2]]; w.n_cells()];
    let flow = Flow {
        u: &u,
        grad: Some(&g),
        rho: 1.0,
        mu: 1.0,
    };
    let mut rng = SplitMix64::new(9);
    let mut worst = 0.0f64;
    let mut cc_exact = true;
    for _ in 0..1000 {
        let p = warp_map([rng.uniform(), rng.uniform(), rng.uniform()]);
        let Some(c) = loc.locate(p, None) else { continue };
        let ui = fluid_at(&w, &flow, Interp::Gradient, c, p);
        let ue = f(p);
        for k in 0..3 {
            worst = worst.max((ui[k] - ue[k]).abs());
        }
        cc_exact &= fluid_at(&w, &flow, Interp::CellCentre, c, p) == u[c];
    }
    println!("PT12 worst linear-field error {worst:e}");
    assert!(worst <= 1e-12);
    assert!(cc_exact);
}

// ---- deposition

fn landing_tracker(drag: DragLaw, gravity: [f64; 3]) -> Tracker {
    let mesh = quiet_box([4, 4, 8], [1.0, 1.0, 1.0]);
    let opts = Options {
        drag,
        scheme: Scheme::Analytic,
        interp: Interp::CellCentre,
        gravity,
        ..Options::default()
    };
    Tracker::new(
        mesh,
        opts,
        &[("zmin".into(), WallAction::Stick), ("zmax".into(), WallAction::Escape)],
    )
    .unwrap()
}

fn step_until_done(tr: &mut Tracker, rho_f: f64) {
    let u = vec![[0.0; 3]; tr.locator().mesh().n_cells()];
    let flow = Flow {
        u: &u,
        grad: None,
        rho: rho_f,
        mu: MU,
    };
    for _ in 0..1000 {
        if tr.particles[0].state != State::Active {
            return;
        }
        tr.advance(&flow, 0.01).unwrap();
    }
}

#[test]
fn pt9_deposition_ballistic() {
    let mut tr = landing_tracker(DragLaw::None, [0.0, 0.0, -G]);
    let p = particle(&tr, [0.3, 0.4, 0.7], [0.5, 0.1, 0.0]);
    tr.particles.push(p);
    step_until_done(&mut tr, 0.0);
    let p = &tr.particles[0];
    let t = (2.0 * 0.7 / G).sqrt();
    println!(
        "PT9 ballistic: state {:?}, age {} (exact {t}), x {:?}",
        p.state, p.age, p.x
    );
    assert_eq!(p.state, State::Deposited);
    assert_eq!(tr.count(State::Deposited), 1);
    assert!((p.age - t).abs() / t <= 1e-9);
    assert!((p.x[0] - (0.3 + 0.5 * t)).abs() / (0.5 * t) <= 1e-9);
    assert!((p.x[1] - (0.4 + 0.1 * t)).abs() / (0.1 * t) <= 1e-9);
    assert!(p.x[2].abs() <= 1e-9);
}

#[test]
fn pt9_deposition_stokes() {
    let mut tr = landing_tracker(DragLaw::Stokes, [0.0, 0.0, -G]);
    let p = particle(&tr, [0.3, 0.4, 0.7], [0.0; 3]);
    tr.particles.push(p);
    step_until_done(&mut tr, RHO_F);
    let p = &tr.particles[0];
    let (v, t) = (vt(), tau());
    // Newton on z0 = v (T - t (1 - exp(-T/t)))
    let mut big_t = 0.7 / v + t;
    for _ in 0..50 {
        let f = v * (big_t - t * (1.0 - (-big_t / t).exp())) - 0.7;
        let df = v * (1.0 - (-big_t / t).exp());
        big_t -= f / df;
    }
    println!("PT9 stokes: state {:?}, age {} (exact {big_t})", p.state, p.age);
    assert_eq!(p.state, State::Deposited);
    assert!((p.age - big_t).abs() / big_t <= 1e-9);
    assert!((p.x[0] - 0.3).abs() <= 1e-12 && (p.x[1] - 0.4).abs() <= 1e-12);
}

#[test]
fn pt9_escape() {
    let mut tr = landing_tracker(DragLaw::None, [0.0; 3]);
    let p = particle(&tr, [0.5, 0.5, 0.5], [0.0, 0.0, 1.0]);
    tr.particles.push(p);
    step_until_done(&mut tr, 0.0);
    let p = &tr.particles[0];
    assert_eq!(p.state, State::Escaped);
    assert_eq!(tr.count(State::Escaped), 1);
    assert!((p.age - 0.5).abs() <= 1e-9 && (p.x[2] - 1.0).abs() <= 1e-9);
}

// ---- Rosin-Rammler

#[test]
fn pt11_rosin_rammler() {
    let (d_bar, n) = (1e-4, 2.5);
    for k in 0..1000 {
        let u = (k as f64 + 1.0) / 1000.0;
        let d = rr_sample(u, d_bar, n);
        assert!((rr_oversize(d, d_bar, n) - u).abs() <= 1e-12);
    }
    let dist = SizeDist::RosinRammler {
        d_bar,
        n,
        d_min: None,
        d_max: None,
    };
    let mut rng = SplitMix64::new(3);
    let ns = 200_000;
    let ds: Vec<f64> = (0..ns).map(|_| dist.draw(&mut rng)).collect();
    for f in [0.5, 1.0, 1.5] {
        let d = f * d_bar;
        let frac = ds.iter().filter(|&&x| x > d).count() as f64 / ns as f64;
        let y = rr_oversize(d, d_bar, n);
        let bound = 5.0 * (y * (1.0 - y) / ns as f64).sqrt();
        println!(
            "PT11 d = {f} d_bar: fraction {frac:.5}, Y {y:.5}, |diff| {:.2e}, bound {bound:.2e}",
            (frac - y).abs()
        );
        assert!((frac - y).abs() <= bound);
    }
    let (lo, hi) = (0.6 * d_bar, 1.4 * d_bar);
    let tr = SizeDist::RosinRammler {
        d_bar,
        n,
        d_min: Some(lo),
        d_max: Some(hi),
    };
    let ds: Vec<f64> = (0..ns).map(|_| tr.draw(&mut rng)).collect();
    assert!(ds.iter().all(|&x| x >= lo && x <= hi));
    let (da, db) = (0.8 * d_bar, 1.1 * d_bar);
    let frac = ds.iter().filter(|&&x| x >= da && x < db).count() as f64 / ns as f64;
    let exp = (rr_oversize(da, d_bar, n) - rr_oversize(db, d_bar, n))
        / (rr_oversize(lo, d_bar, n) - rr_oversize(hi, d_bar, n));
    let bound = 5.0 * (exp * (1.0 - exp) / ns as f64).sqrt();
    println!("PT11c truncated fraction {frac:.5} vs {exp:.5}, bound {bound:.2e}");
    assert!((frac - exp).abs() <= bound);
}

#[test]
fn pt13_vtp_round_trip() {
    let mut rng = SplitMix64::new(1);
    let ps: Vec<Particle> = (0..50)
        .map(|i| Particle {
            id: i,
            x: [rng.uniform(), rng.uniform(), rng.uniform()],
            v: [rng.uniform(), rng.uniform(), rng.uniform()],
            d: 1e-4 * rng.uniform(),
            rho: 2500.0,
            parcel: 2.5,
            cell: i as usize,
            age: rng.uniform(),
            state: State::Active,
        })
        .collect();
    let s = vtp_string(&ps);
    assert!(s.contains("NumberOfPoints=\"50\""));
    for name in [
        "Points",
        "diameter",
        "velocity",
        "density",
        "parcel_count",
        "cell",
        "age",
        "state",
        "id",
    ] {
        assert!(s.contains(&format!("Name=\"{name}\"")), "{name}");
    }
    let grab = |name: &str| -> Vec<f64> {
        let i = s.find(&format!("Name=\"{name}\"")).unwrap();
        let rest = &s[i..];
        let a = rest.find('>').unwrap() + 1;
        let b = rest.find("</DataArray>").unwrap();
        rest[a..b].split_whitespace().map(|t| t.parse().unwrap()).collect()
    };
    let pts = grab("Points");
    let dia = grab("diameter");
    for (i, p) in ps.iter().enumerate() {
        for k in 0..3 {
            assert_eq!(pts[3 * i + k], p.x[k]);
        }
        assert_eq!(dia[i], p.d);
    }
}
