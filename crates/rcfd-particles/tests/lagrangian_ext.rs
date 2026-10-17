//! Registered checks L1-L13 of ADR 0057 (wave 29C): two-way exchange, discrete random walk, injection distributions.

use rcfd_core::vec3::{dot, norm, scale, sub, unit};
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::Mesh;
use rcfd_particles::dispersion::{crossing_time, eddy_length, sigma, t_lagrangian, Eddy, C_MU};
use rcfd_particles::drag::tau_p;
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

fn quiet_box(n: [usize; 3], l: [f64; 3]) -> Arc<Mesh> {
    Arc::new(box_mesh(n, l, [Grading::Uniform; 3]).unwrap())
}

fn particle(tr: &Tracker, id: u64, x: [f64; 3], v: [f64; 3], d: f64, parcel: f64) -> Particle {
    Particle {
        id,
        x,
        v,
        d,
        rho: RHO_P,
        parcel,
        cell: tr.locator().locate(x, None).expect("inside"),
        age: 0.0,
        state: State::Active,
    }
}

fn mass(p: &Particle) -> f64 {
    p.parcel * p.rho * std::f64::consts::PI * p.d.powi(3) / 6.0
}

fn opts(drag: DragLaw, scheme: Scheme, g: [f64; 3]) -> Options {
    Options {
        drag,
        scheme,
        interp: Interp::CellCentre,
        gravity: g,
        ..Options::default()
    }
}

fn flow_of<'a>(u: &'a [[f64; 3]]) -> Flow<'a> {
    Flow {
        u,
        grad: None,
        rho: RHO_F,
        mu: MU,
    }
}

/// Non-uniform carrier velocity from the cell centres.
fn carrier(mesh: &Mesh) -> Vec<[f64; 3]> {
    mesh.cell_centres()
        .iter()
        .map(|c| [1.0 * c[1] + 0.2, -0.5 * c[0], 0.3 + 0.1 * c[2]])
        .collect()
}

fn parcels(tr: &Tracker, n: usize, seed: u64) -> Vec<Particle> {
    let mut rng = SplitMix64::new(seed);
    (0..n)
        .map(|i| {
            let x = [
                0.3 + 0.4 * rng.uniform(),
                0.3 + 0.4 * rng.uniform(),
                0.3 + 0.4 * rng.uniform(),
            ];
            let v = [rng.uniform() - 0.5, rng.uniform() - 0.5, rng.uniform() - 0.5];
            let d = 5e-5 + 1e-4 * rng.uniform();
            particle(tr, i as u64, x, v, d, 1e3 + 1e3 * rng.uniform())
        })
        .collect()
}

fn thermal_for(mesh: &Mesh) -> Thermal {
    Thermal {
        cp_p: 900.0,
        k_f: 0.026,
        cp_f: 1005.0,
        t_fluid: mesh
            .cell_centres()
            .iter()
            .map(|c| 300.0 + 20.0 * c[0] + 10.0 * c[1])
            .collect(),
    }
}

fn run_two_way(walls: bool, elapsed_from_age: bool) -> (f64, f64, f64, f64) {
    // returns (momentum residual / scale, energy residual / scale, wall impulse norm, scale)
    let mesh = quiet_box([4, 4, 4], [1.0, 1.0, 1.0]);
    let u = carrier(&mesh);
    let g = [0.0, 0.0, -G];
    let o = if walls {
        opts(DragLaw::SchillerNaumann, Scheme::Rk4, g)
    } else {
        opts(DragLaw::SchillerNaumann, Scheme::Analytic, g)
    };
    let mut tr = Tracker::new(mesh.clone(), o, &[("zmin".into(), WallAction::Reflect(0.8))]).unwrap();
    tr.enable_coupling();
    let th = thermal_for(&mesh);
    if !walls {
        tr.set_thermal(th.clone()).unwrap();
    }
    let ps = parcels(&tr, 40, 7);
    let p0 = ps.clone();
    tr.particles = ps;
    let (dt, steps) = if walls { (0.05, 20) } else { (0.01, 10) };
    for _ in 0..steps {
        tr.advance(&flow_of(&u), dt).unwrap();
    }
    let t = dt * steps as f64;
    let cpl = tr.coupling().unwrap();
    let (sm, se) = cpl.sources(mesh.cell_volumes(), t);
    // independent particle-side sums
    let mut dp = [0.0; 3];
    let mut scale_p = 0.0;
    let mut de = 0.0;
    let mut scale_e = 0.0;
    for (i, (a, b)) in p0.iter().zip(tr.particles.iter()).enumerate() {
        let m = mass(a);
        let geff = scale(g, 1.0 - RHO_F / RHO_P);
        let te = if elapsed_from_age { b.age - a.age } else { t };
        let j = scale(sub(sub(b.v, a.v), scale(geff, te)), m);
        for k in 0..3 {
            dp[k] += j[k];
        }
        scale_p += norm(j);
        if !walls {
            let dke = 0.5 * m * (dot(b.v, b.v) - dot(a.v, a.v));
            let wg = m * dot(geff, sub(b.x, a.x));
            let t0 = th.t_fluid[a.cell];
            let heat = m * th.cp_p * (tr.temperature(i) - t0);
            de += dke - wg + heat;
            scale_e += dke.abs() + wg.abs() + heat.abs();
        }
    }
    let mut ds = [0.0; 3];
    let mut dse = 0.0;
    for c in 0..mesh.n_cells() {
        let v = mesh.cell_volumes()[c] * t;
        for k in 0..3 {
            ds[k] += sm[c][k] * v;
        }
        dse += se[c] * v;
    }
    let w = cpl.wall_impulse;
    // sum J_p (drag part) = (all momentum change - gravity) minus wall impulse = - sum S V t
    let res = norm([dp[0] - w[0] + ds[0], dp[1] - w[1] + ds[1], dp[2] - w[2] + ds[2]]);
    let eres = (de + dse).abs();
    (res / scale_p, eres / scale_e.max(1e-300), norm(w), scale_p)
}

#[test]
fn l1_momentum_balance() {
    let (r, _, w, s) = run_two_way(false, false);
    println!("L1 no walls residual {r:e} (scale {s:e}), wall impulse {w:e}");
    assert!(r <= 1e-12);
    assert_eq!(w, 0.0);
}

/// SET-UP DEFECT, kept as a record (ADR 0057 addendum): the gravity impulse of every parcel was taken over the full tracked time
/// `t`, but parcels that rest on the reflecting floor are declared stuck (Zeno, ADR 0028 A2) and are no longer advanced, so their
/// elapsed time is shorter. The corrected test is `l1b_momentum_balance_with_walls`.
#[test]
#[ignore]
fn l1_walls_full_time_setup_defect() {
    let (r, _, w, s) = run_two_way(true, false);
    println!("L1 walls (full time) residual {r:e} (scale {s:e}), wall impulse {w:e}");
    assert!(r <= 1e-12);
}

#[test]
fn l1b_momentum_balance_with_walls() {
    let (r, _, w, s) = run_two_way(true, true);
    println!("L1b walls residual {r:e} (scale {s:e}), wall impulse {w:e}");
    assert!(w > 0.0, "the walls must be hit in this set-up");
    assert!(r <= 1e-12);
}

#[test]
fn l2_energy_balance() {
    let (_, r, _, _) = run_two_way(false, false);
    println!("L2 energy residual {r:e}");
    assert!(r <= 1e-12);
}

#[test]
fn l3_observation_only_and_closed_forms() {
    // bit-identical trajectories with the exchange recorded
    let mesh = quiet_box([4, 4, 4], [1.0, 1.0, 1.0]);
    let u = carrier(&mesh);
    let g = [0.0, 0.0, -G];
    let fp = |rec: bool| {
        let mut tr = Tracker::new(mesh.clone(), opts(DragLaw::SchillerNaumann, Scheme::Rk4, g), &[]).unwrap();
        if rec {
            tr.enable_coupling();
        }
        tr.particles = parcels(&tr, 20, 3);
        for _ in 0..15 {
            tr.advance(&flow_of(&u), 0.03).unwrap();
        }
        tr.fingerprint()
    };
    assert_eq!(fp(false), fp(true));
    // terminal velocity with coupling on
    let mesh = quiet_box([2, 2, 12], [1.0, 1.0, 3.0]);
    let uq = vec![[0.0; 3]; mesh.n_cells()];
    let mut tr = Tracker::new(mesh.clone(), opts(DragLaw::Stokes, Scheme::Analytic, g), &[]).unwrap();
    tr.enable_coupling();
    tr.particles = vec![particle(&tr, 0, [0.5, 0.5, 2.8], [0.0; 3], D, 1.0)];
    for _ in 0..150 {
        tr.advance(&flow_of(&uq), 0.01).unwrap();
    }
    let vt = D * D * (RHO_P - RHO_F) * G / (18.0 * MU);
    let e = (tr.particles[0].v[2] + vt).abs() / vt;
    println!("L3 terminal velocity error {e:e}");
    assert!(e <= 1e-6);
    // relaxation in a uniform flow
    let tau = tau_p(D, RHO_P, MU);
    let uu = vec![[1.0, 0.0, 0.0]; mesh.n_cells()];
    let mut tr = Tracker::new(mesh.clone(), opts(DragLaw::Stokes, Scheme::Analytic, [0.0; 3]), &[]).unwrap();
    tr.enable_coupling();
    tr.particles = vec![particle(&tr, 0, [0.1, 0.5, 1.5], [0.0; 3], D, 1.0)];
    let n = 10;
    for _ in 0..n {
        tr.advance(&flow_of(&uu), tau / n as f64).unwrap();
    }
    let ex = 1.0 - (-1.0f64).exp();
    let e = (tr.particles[0].v[0] - ex).abs() / ex;
    println!("L3 relaxation error {e:e}");
    assert!(e <= 1e-12);
}

/// Single-cell coupled loop; returns (v, u, w) at t = tau for the loading phi (None = one-way).
fn single_cell(phi: Option<f64>, dt_div: usize) -> (f64, f64, f64, f64) {
    let mesh = quiet_box([1, 1, 1], [1.0, 1.0, 1.0]);
    let tau = tau_p(D, RHO_P, MU);
    let m_f = RHO_F * 1.0;
    let ph = phi.unwrap_or(1e-30);
    let m_p = ph * m_f;
    let parcel = m_p / (RHO_P * std::f64::consts::PI * D.powi(3) / 6.0);
    let mut tr = Tracker::new(mesh.clone(), opts(DragLaw::Stokes, Scheme::Analytic, [0.0; 3]), &[]).unwrap();
    tr.enable_coupling();
    let v0 = 0.01;
    tr.particles = vec![particle(&tr, 0, [0.5, 0.5, 0.5], [v0, 0.0, 0.0], D, parcel)];
    let mut u = vec![[0.0; 3]];
    let dt = tau / dt_div as f64;
    for _ in 0..dt_div {
        tr.advance(&flow_of(&u), dt).unwrap();
        if phi.is_some() {
            let cpl = tr.coupling_mut().unwrap();
            let imp = cpl.impulse[0];
            for k in 0..3 {
                u[0][k] += imp[k] / (RHO_F * mesh.cell_volumes()[0]);
            }
            cpl.reset();
        }
    }
    let v = tr.particles[0].v[0];
    let mom = m_p * v + m_f * u[0][0];
    (v, u[0][0], v - u[0][0], (mom - m_p * v0).abs() / (m_p * v0))
}

#[test]
fn l4_single_cell_closed_form() {
    let phi: f64 = 1.0;
    let exact = (-(1.0 + phi)).exp();
    let mut errs = Vec::new();
    for div in [100, 200, 400] {
        let (_, _, w, mom) = single_cell(Some(phi), div);
        let e = (w / 0.01 - exact).abs();
        println!(
            "L4 dt = tau/{div}: w/w0 error {:e} (relative {:e}), momentum {mom:e}",
            e,
            e / exact
        );
        assert!(mom <= 1e-13, "momentum {mom:e}");
        if div == 100 {
            assert!(e <= 0.02 * exact);
        }
        errs.push(e);
    }
    let (r1, r2) = (errs[0] / errs[1], errs[1] / errs[2]);
    println!("L4 convergence ratios {r1:.4} {r2:.4}");
    assert!((1.8..=2.2).contains(&r1) && (1.8..=2.2).contains(&r2));
}

#[test]
fn l5_dilute_limit() {
    let (v_one, ..) = single_cell(None, 100);
    let mut diffs = Vec::new();
    for phi in [1e-2, 1e-4, 1e-6] {
        let (v2, ..) = single_cell(Some(phi), 100);
        let d = (v2 - v_one).abs() / 0.01;
        println!("L5 phi {phi:e}: |v_two - v_one|/|v0| = {d:e} ({:.4} phi)", d / phi);
        assert!(d <= 0.5 * phi);
        diffs.push(d);
    }
    let r = diffs[0] / diffs[1];
    println!("L5 ratio of differences {r:.3}");
    assert!((90.0..=110.0).contains(&r));
}

#[test]
fn l6_heat_transfer_closed_form() {
    let mesh = quiet_box([1, 1, 1], [1.0, 1.0, 1.0]);
    let u = vec![[0.0; 3]];
    let mut tr = Tracker::new(mesh.clone(), opts(DragLaw::None, Scheme::Analytic, [0.0; 3]), &[]).unwrap();
    let th = Thermal {
        cp_p: 900.0,
        k_f: 0.026,
        cp_f: 1005.0,
        t_fluid: vec![300.0],
    };
    tr.set_thermal(th).unwrap();
    tr.enable_coupling();
    tr.add_injector(InjectorSpec {
        name: "one".into(),
        kind: InjKind::Point([0.5, 0.5, 0.5]),
        rate: 1000.0,
        t_start: 0.0,
        t_end: 1e-3,
        velocity: VelMode::Given([0.0; 3]),
        dist: SizeDist::Constant(D),
        density: RHO_P,
        parcel_count: 1.0e3,
        seed: 1,
    })
    .unwrap();
    tr.set_injector_ext(
        "one",
        InjectorExt {
            temperature: Some(400.0),
            ..InjectorExt::default()
        },
    )
    .unwrap();
    tr.advance(&flow_of(&u), 1e-3).unwrap();
    assert_eq!(tr.particles.len(), 1);
    for _ in 0..10 {
        tr.advance(&flow_of(&u), 0.01).unwrap();
    }
    let tau_t = RHO_P * 900.0 * D * D / (12.0 * 0.026);
    let exact = 300.0 + 100.0 * (-0.1 / tau_t).exp();
    let tp = tr.temperature(0);
    let e = (tp - exact).abs() / (exact - 300.0);
    println!("L6 T_p {tp}, exact {exact}, relative error of the excess {e:e}");
    assert!(e <= 1e-12);
    let m = mass(&tr.particles[0]);
    let q = m * 900.0 * (tp - 400.0);
    let got = tr.coupling().unwrap().energy_heat[0];
    let e2 = (got + q).abs() / q.abs();
    println!("L6 heat bookkeeping error {e2:e}");
    assert!(e2 <= 1e-12);
}

#[test]
fn l7_dispersion_constants_and_crossing() {
    let (k, eps) = (1.7, 0.9);
    assert!((sigma(k) - (2.0 * k / 3.0f64).sqrt()).abs() <= 1e-14);
    assert!((t_lagrangian(0.15, k, eps) - 0.15 * k / eps).abs() <= 1e-14);
    let le = C_MU.powf(0.75) * k.powf(1.5) / eps;
    assert!((eddy_length(k, eps) - le).abs() <= 1e-14 * le);
    // a Stokes particle at rest in a stream of speed w: the relative distance at t_cross equals L_e
    let tau = tau_p(D, RHO_P, MU);
    let w = 2.0;
    let l = 0.5 * tau * w;
    let tc = crossing_time(tau, l, w);
    let (x, _) = step_particle(
        Scheme::Analytic,
        DragLaw::Stokes,
        fl(),
        D,
        RHO_P,
        [0.0; 3],
        [0.0; 3],
        [0.0; 3],
        tc,
        &mut |_| [w, 0.0, 0.0],
    );
    let rel = w * tc - x[0];
    let e = (rel - l).abs() / l;
    println!("L7 crossing relative-distance error {e:e}");
    assert!(e <= 1e-10);
    assert!(crossing_time(tau, tau * w * 1.0001, w).is_infinite());
    assert!(crossing_time(tau, 1.0, 0.0).is_infinite());
}

/// Stationary sample of a Stokes particle in synthetic homogeneous turbulence (pure lifetime model).
/// Returns (mean of v_i^2 averaged over components, mean of v_i, mean eddy lifetime, variance of u').
fn drw_stats(tau_over_t: f64, n: usize, seed: u64) -> (f64, f64, f64, f64, f64) {
    let t_l = 0.5;
    let k = 1.5; // sigma = 1
    let eps = 0.15 * k / t_l;
    let tau = tau_over_t * t_l;
    let d = (18.0 * MU * tau / RHO_P).sqrt();
    assert!((tau_p(d, RHO_P, MU) - tau).abs() <= 1e-12 * tau);
    let model = Dispersion {
        c_l: 0.15,
        crossing: false,
    };
    let t_end = 12.0 * t_l.max(tau);
    let mut rng = SplitMix64::new(seed);
    let (mut s2, mut s1, mut life, mut up2) = (0.0, 0.0, 0.0, 0.0);
    for _ in 0..n {
        let mut v = [0.0; 3];
        let mut x = [0.0; 3];
        let mut t = 0.0;
        let mut first = true;
        while t < t_end {
            let mut e = Eddy::default();
            e.draw(&mut rng, &model, k, eps, tau, [0.0; 3], v);
            if first {
                life += e.t_left;
                up2 += e.uprime.iter().map(|a| a * a).sum::<f64>();
                first = false;
            }
            let h = e.t_left.min(t_end - t);
            let up = e.uprime;
            let (xn, vn) = step_particle(
                Scheme::Analytic,
                DragLaw::Stokes,
                fl(),
                d,
                RHO_P,
                [0.0; 3],
                x,
                v,
                h,
                &mut |_| up,
            );
            x = xn;
            v = vn;
            t += h;
        }
        s2 += v.iter().map(|a| a * a).sum::<f64>() / 3.0;
        s1 += v.iter().sum::<f64>() / 3.0;
    }
    let nf = n as f64;
    (s2 / nf, s1 / nf, life / nf, up2 / (3.0 * nf), t_l)
}

#[test]
fn l8_drw_statistics() {
    let n = 40_000;
    for r in [0.1, 1.0, 5.0] {
        let (var, mean, life, up2, t_l) = drw_stats(r, n, 20260329);
        let expect = t_l / (t_l + r * t_l);
        let e = (var - expect).abs() / expect;
        let mean_lim = 4.0 * expect.sqrt() / (n as f64).sqrt();
        println!(
            "L8 tau/T_L = {r}: <v^2> = {var:.5} (closed form {expect:.5}, rel err {e:.4}), <v> = {mean:.2e} (limit {mean_lim:.2e}), \
             mean lifetime {:.4} T_L, var(u') = {up2:.4}",
            life / t_l
        );
        assert!(e <= 0.04);
        assert!(mean.abs() <= mean_lim);
        assert!((life / t_l - 1.0).abs() <= 0.04);
        assert!((up2 - 1.0).abs() <= 0.025);
    }
    // determinism
    let a = drw_stats(1.0, 500, 11);
    let b = drw_stats(1.0, 500, 11);
    let c = drw_stats(1.0, 500, 12);
    assert_eq!(a.0.to_bits(), b.0.to_bits());
    assert_ne!(a.0.to_bits(), c.0.to_bits());
}

#[test]
fn l9_drw_in_the_tracker() {
    let big = 1.0e4;
    let mesh = quiet_box([1, 1, 1], [big, big, big]);
    let u = vec![[0.0; 3]];
    let t_l = 0.5;
    let (k, eps) = (1.5, 0.15 * 1.5 / 0.5);
    let tau = t_l; // tau / T_L = 1
    let d = (18.0 * MU * tau / RHO_P).sqrt();
    let n = 4000;
    let run = |seed: u64| {
        let mut tr = Tracker::new(mesh.clone(), opts(DragLaw::Stokes, Scheme::Analytic, [0.0; 3]), &[]).unwrap();
        tr.set_dispersion(
            Dispersion {
                c_l: 0.15,
                crossing: false,
            },
            vec![k],
            vec![eps],
            seed,
        )
        .unwrap();
        tr.particles = (0..n)
            .map(|i| particle(&tr, i as u64, [big / 2.0; 3], [0.0; 3], d, 1.0))
            .collect();
        for _ in 0..12 {
            tr.advance(&flow_of(&u), 0.5).unwrap();
        }
        tr
    };
    let tr = run(5);
    let var: f64 = tr.particles.iter().map(|p| dot(p.v, p.v) / 3.0).sum::<f64>() / n as f64;
    let expect = 0.5;
    let e = (var - expect).abs() / expect;
    println!("L9 tracker <v^2> = {var:.5}, closed form {expect}, relative error {e:.4}");
    assert!(e <= 0.08);
    assert_eq!(run(5).fingerprint(), tr.fingerprint());
    assert_ne!(run(6).fingerprint(), tr.fingerprint());
}

fn ks(sorted: &[f64], cdf: &dyn Fn(f64) -> f64) -> f64 {
    let n = sorted.len() as f64;
    let mut dmax: f64 = 0.0;
    for (i, &x) in sorted.iter().enumerate() {
        let f = cdf(x);
        dmax = dmax
            .max((((i + 1) as f64) / n - f).abs())
            .max((f - (i as f64) / n).abs());
    }
    dmax
}

fn inject_n(dist: SizeDist, ext: InjectorExt, n: usize, big: f64, kind: InjKind, seed: u64) -> Tracker {
    let mesh = if let InjKind::Surface(_) = kind {
        quiet_box([4, 4, 4], [2.0, 1.0, 1.0])
    } else {
        quiet_box([1, 1, 1], [big, big, big])
    };
    let u = vec![[0.0; 3]; mesh.n_cells()];
    let mut tr = Tracker::new(mesh, opts(DragLaw::None, Scheme::Analytic, [0.0; 3]), &[]).unwrap();
    tr.add_injector(InjectorSpec {
        name: "j".into(),
        kind,
        rate: n as f64,
        t_start: 0.0,
        t_end: 1.0,
        velocity: VelMode::Given([0.0; 3]),
        dist,
        density: RHO_P,
        parcel_count: 1.0,
        seed,
    })
    .unwrap();
    tr.set_injector_ext("j", ext).unwrap();
    tr.advance(&flow_of(&u), 1.0).unwrap();
    assert_eq!(tr.particles.len(), n);
    tr
}

#[test]
fn l10_l11_size_distributions_and_mass_flow() {
    let n = 20_000;
    let bound = 1.95 / (n as f64).sqrt();
    let (db, nn) = (1.0e-4, 2.5);
    let f_rr = |d: f64| 1.0 - (-(d / db).powf(nn)).exp();
    let (lo, hi) = (3.0e-5, 3.0e-4);
    let f_tr = |d: f64| (f_rr(d) - f_rr(lo)) / (f_rr(hi) - f_rr(lo));
    let (ua, ub) = (2.0e-5, 2.0e-4);
    let f_u = |d: f64| (d - ua) / (ub - ua);
    let mdot = 0.05;
    type Case<'a> = (&'static str, SizeDist, Box<dyn Fn(f64) -> f64 + 'a>);
    let cases: Vec<Case<'_>> = vec![
        (
            "rosin_rammler",
            SizeDist::RosinRammler {
                d_bar: db,
                n: nn,
                d_min: None,
                d_max: None,
            },
            Box::new(f_rr),
        ),
        (
            "rosin_rammler truncated",
            SizeDist::RosinRammler {
                d_bar: db,
                n: nn,
                d_min: Some(lo),
                d_max: Some(hi),
            },
            Box::new(f_tr),
        ),
        ("uniform", SizeDist::Uniform { d_min: ua, d_max: ub }, Box::new(f_u)),
    ];
    for (name, dist, cdf) in cases {
        let ext = InjectorExt {
            mass_flow: Some(mdot),
            ..InjectorExt::default()
        };
        let tr = inject_n(dist, ext, n, 10.0, InjKind::Point([5.0; 3]), 99);
        let mut ds: Vec<f64> = tr.particles.iter().map(|p| p.d).collect();
        ds.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let dk = ks(&ds, &*cdf);
        // mass fractions from the weights at five quantiles of the sample CDF target
        let total: f64 = tr.particles.iter().map(mass).sum();
        let mut worst: f64 = 0.0;
        for q in [0.1, 0.3, 0.5, 0.7, 0.9] {
            // diameter with CDF q by bisection
            let (mut a, mut b) = (1e-7, 1e-2);
            for _ in 0..200 {
                let m = 0.5 * (a + b);
                if cdf(m) < q {
                    a = m
                } else {
                    b = m
                }
            }
            let dq = 0.5 * (a + b);
            let mf: f64 = tr.particles.iter().filter(|p| p.d <= dq).map(mass).sum::<f64>() / total;
            worst = worst.max((mf - q).abs());
        }
        let e_mass = (total - n as f64 * mdot / n as f64).abs() / mdot;
        println!(
            "L10/L11 {name}: KS {dk:.5} (bound {bound:.5}), worst mass-fraction deviation {worst:.5}, mass-flow relative error {e_mass:e}"
        );
        assert!(dk <= bound);
        assert!(worst <= bound);
        assert!(e_mass <= 1e-12);
    }
}

#[test]
fn l12_cone() {
    let n = 20_000;
    let bound = 1.95 / (n as f64).sqrt();
    let axis = [1.0, 2.0, 2.0];
    let a = unit(axis);
    let (th, thi, speed) = (0.4f64, 0.1f64, 25.0f64);
    let ext = InjectorExt {
        cone: Some(ConeSpec {
            axis,
            half_angle: th,
            inner_half_angle: thi,
            speed,
        }),
        ..InjectorExt::default()
    };
    let tr = inject_n(SizeDist::Constant(D), ext, n, 1.0e6, InjKind::Point([5.0e5; 3]), 4);
    // basis perpendicular to the axis for the azimuth
    let h = [0.0, 0.0, 1.0];
    let e1 = unit(rcfd_core::vec3::cross(a, h));
    let e2 = rcfd_core::vec3::cross(a, e1);
    let (mut amin, mut amax, mut sp_err): (f64, f64, f64) = (10.0, 0.0, 0.0);
    let mut cs = Vec::new();
    let mut az = Vec::new();
    for p in &tr.particles {
        let c = dot(unit(p.v), a).clamp(-1.0, 1.0);
        let ang = c.acos();
        amin = amin.min(ang);
        amax = amax.max(ang);
        sp_err = sp_err.max((norm(p.v) - speed).abs() / speed);
        cs.push(c);
        let psi = dot(p.v, e2).atan2(dot(p.v, e1));
        az.push((psi + std::f64::consts::PI) / (2.0 * std::f64::consts::PI));
    }
    cs.sort_by(|x, y| x.partial_cmp(y).unwrap());
    az.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let (c0, c1) = (th.cos(), thi.cos());
    let kc = ks(&cs, &|c| (c - c0) / (c1 - c0));
    let ka = ks(&az, &|x| x);
    println!(
        "L12 angle range [{amin:.6}, {amax:.6}] (cone [{thi}, {th}]), speed error {sp_err:e}, KS cos {kc:.5}, KS azimuth {ka:.5} (bound {bound:.5})"
    );
    assert!(amin >= thi - 1e-12 && amax <= th + 1e-12);
    assert!(sp_err <= 1e-14);
    assert!(kc <= bound && ka <= bound);
}

#[test]
fn l13_surface_injection() {
    let n = 20_000;
    let tr = inject_n(
        SizeDist::Constant(D),
        InjectorExt::default(),
        n,
        1.0,
        InjKind::Surface("zmin".into()),
        8,
    );
    let mut left = 0usize;
    for p in &tr.particles {
        assert!(p.x[0] > 0.0 && p.x[0] < 2.0 && p.x[1] > 0.0 && p.x[1] < 1.0);
        assert!(p.x[2] > 0.0 && p.x[2] <= 1e-8 * 0.25, "z = {:e}", p.x[2]);
        if p.x[0] < 1.0 {
            left += 1;
        }
    }
    let f = left as f64 / n as f64;
    let lim = 4.0 * (0.25 / n as f64).sqrt();
    println!("L13 area fraction in the left half {f:.5} (limit 0.5 +- {lim:.5})");
    assert!((f - 0.5).abs() <= lim);
}
