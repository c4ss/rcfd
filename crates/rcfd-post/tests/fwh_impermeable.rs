//! ADR 0066 FW-H checks W1-W6: impermeable surfaces, compact dipole, rotating point force vs the Gutin-type formula,
//! interpolation order, uniform flow, sound power. Limits are those registered in the ADR (not changed after a run).

use rcfd_post::acoustics::{mean_square, sound_power_sphere, sphere_quadrature};
use rcfd_post::fwh::{medium_frame_observer, Face, SourceMode, Surface};
use rcfd_post::rotor::Rotor;
use std::f64::consts::PI;

const RHO0: f64 = 1.2;
const C0: f64 = 340.0;
type V3 = [f64; 3];

fn rms(v: &[f64]) -> f64 {
    (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt()
}

/// Amplitude and phase of `A cos(w t + phi)` from samples spaced by exactly 1/256 of the period.
fn fundamental(ts: &[f64], vals: &[f64], w: f64) -> (f64, f64) {
    let (mut re, mut im) = (0.0, 0.0);
    for (v, &t) in vals.iter().zip(ts) {
        re += v * (w * t).cos();
        im -= v * (w * t).sin();
    }
    let n = vals.len() as f64;
    let (re, im) = (2.0 * re / n, 2.0 * im / n);
    ((re * re + im * im).sqrt(), im.atan2(re))
}

fn wrap(mut d: f64) -> f64 {
    d -= 2.0 * PI * (d / (2.0 * PI)).round();
    d
}

// ---------------- complex helpers ----------------
type C = (f64, f64);
fn cmul(a: C, b: C) -> C {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}
fn cdiv(a: C, b: C) -> C {
    let d = b.0 * b.0 + b.1 * b.1;
    ((a.0 * b.0 + a.1 * b.1) / d, (a.1 * b.0 - a.0 * b.1) / d)
}
fn cexp(a: C) -> C {
    let e = a.0.exp();
    (e * a.1.cos(), e * a.1.sin())
}

// ---------------- pulsating sphere ----------------
struct Pulsating {
    a: f64,
    eps: f64,
    k: f64,
}
impl Pulsating {
    fn w(&self) -> f64 {
        self.k * C0
    }
    fn u0(&self) -> f64 {
        self.eps * self.w()
    }
    fn period(&self) -> f64 {
        2.0 * PI / self.w()
    }
    /// Exact radiating-sphere pressure at radius r (time harmonic, u_r(a) = U0 cos wt).
    fn exact_p(&self, r: f64, t: f64) -> f64 {
        let (k, a, w) = (self.k, self.a, self.w());
        // A = U0 a^2 e^{-ika}/(ika - 1)
        let amp = cdiv(cmul((self.u0() * a * a, 0.0), cexp((0.0, -k * a))), (-1.0, k * a));
        // p = Re[i w rho0 A e^{i(kr - wt)}/r]
        let z = cmul(cmul((0.0, w * RHO0), amp), cexp((0.0, k * r - w * t)));
        z.0 / r
    }
    fn exact_surface_p(&self, t: f64) -> f64 {
        let (k, a, w) = (self.k, self.a, self.w());
        // Re[i w rho0 U0 a/(ika - 1) e^{-iwt}]
        let z = cmul(
            cdiv((0.0, w * RHO0 * self.u0() * a), (-1.0, k * a)),
            cexp((0.0, -w * t)),
        );
        z.0
    }
    /// Thickness-only closed form: -rho0 w U0 a^2 sinc(ka) sin(w(t - R/c))/R.
    fn thickness_p(&self, r: f64, t: f64) -> f64 {
        let ka = self.k * self.a;
        -RHO0 * self.w() * self.u0() * self.a * self.a * (ka.sin() / ka) * (self.w() * (t - r / C0)).sin() / r
    }
    fn surface(&self, with_loading: bool, nth: usize, nph: usize) -> Surface {
        let dt = self.period() / 64.0;
        let ns = 64 * 14;
        let mut faces = Vec::new();
        for (nrm, wq) in sphere_quadrature(nth, nph).unwrap() {
            let mut f = Face {
                area: wq * self.a * self.a,
                x: vec![],
                n: vec![],
                v: vec![],
                p: vec![],
            };
            for i in 0..ns {
                let t = i as f64 * dt;
                let rr = self.a + self.eps * (self.w() * t).sin();
                let vr = self.u0() * (self.w() * t).cos();
                f.x.push([rr * nrm[0], rr * nrm[1], rr * nrm[2]]);
                f.n.push(nrm);
                f.v.push([vr * nrm[0], vr * nrm[1], vr * nrm[2]]);
                f.p.push(if with_loading { self.exact_surface_p(t) } else { 0.0 });
            }
            faces.push(f);
        }
        Surface::impermeable(RHO0, C0, 0.0, dt, faces, SourceMode::Impermeable)
    }
}

/// Observer times: 256 samples over one period starting half a period inside the window.
fn period_times(lo: f64, period: f64) -> Vec<f64> {
    (0..256)
        .map(|i| lo + 0.5 * period + period * i as f64 / 256.0)
        .collect()
}

fn check_series(s: &Surface, obs: V3, period: f64, exact: &dyn Fn(f64) -> f64, label: &str) {
    let (lo, _) = s.valid_window(obs).unwrap();
    let ts = period_times(lo, period);
    let sig = s.signal(obs, &ts).unwrap();
    let got: Vec<f64> = sig.iter().map(|g| g.total()).collect();
    let ex: Vec<f64> = ts.iter().map(|&t| exact(t)).collect();
    let diff: Vec<f64> = got.iter().zip(&ex).map(|(a, b)| a - b).collect();
    let e = rms(&diff) / rms(&ex);
    let w = 2.0 * PI / period;
    let (ag, pg) = fundamental(&ts, &got, w);
    let (ae, pe) = fundamental(&ts, &ex, w);
    let (da, dp) = ((ag / ae - 1.0).abs(), wrap(pg - pe).abs());
    println!("{label}: e = {e:.3e}, amplitude {da:.3e}, phase {dp:.3e} rad");
    assert!(e <= 1e-3, "{label}: e = {e:.3e} > 1e-3");
    assert!(da <= 1e-3, "{label}: amplitude {da:.3e} > 1e-3");
    assert!(dp <= 1e-3, "{label}: phase {dp:.3e} > 1e-3");
}

#[test]
fn w1a_pulsating_sphere_thickness_only() {
    let ps = Pulsating {
        a: 1.0,
        eps: 1e-5,
        k: PI / 2.0,
    };
    let s = ps.surface(false, 16, 32);
    for (name, obs) in [
        ("R=10 x", [10.0, 0.0, 0.0]),
        ("R=100 oblique", [60.0, 0.0, 80.0]),
        ("R=10 y", [0.0, 10.0, 0.0]),
    ] {
        let obs: V3 = obs;
        let r = (obs[0] * obs[0] + obs[1] * obs[1] + obs[2] * obs[2]).sqrt();
        check_series(&s, obs, ps.period(), &|t| ps.thickness_p(r, t), &format!("W1a {name}"));
    }
}

#[test]
fn w1b_pulsating_sphere_full_field() {
    let ps = Pulsating {
        a: 1.0,
        eps: 1e-5,
        k: PI / 2.0,
    };
    let s = ps.surface(true, 16, 32);
    for (name, obs) in [
        ("R=10 x", [10.0, 0.0, 0.0]),
        ("R=100 oblique", [60.0, 0.0, 80.0]),
        ("R=10 y", [0.0, 10.0, 0.0]),
    ] {
        let obs: V3 = obs;
        let r = (obs[0] * obs[0] + obs[1] * obs[1] + obs[2] * obs[2]).sqrt();
        check_series(&s, obs, ps.period(), &|t| ps.exact_p(r, t), &format!("W1b {name}"));
    }
}

// ---------------- compact dipole ----------------
#[test]
fn w2_compact_dipole() {
    let (a, k) = (0.01, PI / 2.0);
    let w = k * C0;
    let period = 2.0 * PI / w;
    let dt = period / 64.0;
    let ns = 64 * 14;
    let p0 = 1000.0;
    let f0 = p0 * 4.0 * PI * a * a / 3.0;
    let mut faces = Vec::new();
    for (nrm, wq) in sphere_quadrature(8, 16).unwrap() {
        let mut f = Face {
            area: wq * a * a,
            x: vec![],
            n: vec![],
            v: vec![],
            p: vec![],
        };
        for i in 0..ns {
            let t = i as f64 * dt;
            f.x.push([a * nrm[0], a * nrm[1], a * nrm[2]]);
            f.n.push(nrm);
            f.v.push([0.0; 3]);
            f.p.push(p0 * (w * t).sin() * nrm[0]);
        }
        faces.push(f);
    }
    let s = Surface::impermeable(RHO0, C0, 0.0, dt, faces, SourceMode::LoadingOnly);
    let exact = |x: V3, t: f64| {
        let r = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
        let tau = t - r / C0;
        let f = f0 * (w * tau).sin();
        let fd = f0 * w * (w * tau).cos();
        x[0] / (4.0 * PI * r * r) * (f / r + fd / C0)
    };
    let r = 100.0;
    let th = 45f64.to_radians();
    let obs = [r * th.cos(), r * th.sin(), 0.0];
    let (lo, _) = s.valid_window(obs).unwrap();
    let ts = period_times(lo, period);
    let sig = s.signal(obs, &ts).unwrap();
    let diff: Vec<f64> = sig.iter().zip(&ts).map(|(g, &t)| g.total() - exact(obs, t)).collect();
    let ex: Vec<f64> = ts.iter().map(|&t| exact(obs, t)).collect();
    let e = rms(&diff) / rms(&ex);
    println!("W2 series e = {e:.3e}");
    assert!(e <= 1e-3, "W2 e = {e:.3e}");
    assert!(s.signal(obs, &ts).unwrap().iter().all(|g| g.p_thickness == 0.0));
    let amp = |deg: f64| -> f64 {
        let th = deg.to_radians();
        let o = [r * th.cos(), r * th.sin(), 0.0];
        let (lo, _) = s.valid_window(o).unwrap();
        let ts = period_times(lo, period);
        let sg: Vec<f64> = s.signal(o, &ts).unwrap().iter().map(|g| g.total()).collect();
        fundamental(&ts, &sg, w).0
    };
    let a0 = amp(0.0);
    for deg in [0.0_f64, 30.0, 60.0, 90.0, 120.0, 150.0, 180.0] {
        let err = (amp(deg) / a0 - deg.to_radians().cos().abs()).abs();
        println!("W2 theta {deg}: directivity error {err:.3e}");
        assert!(err <= 1e-3, "W2 theta {deg}: {err:.3e}");
    }
}

// ---------------- rotating point force ----------------
const MT: f64 = 0.3;
const RT: f64 = 1.0;

/// B blades, each a compact force T_b e_z + D_b e_phi at radius RT, `spp` samples per revolution, `revs` revolutions.
fn rotor_surface(b: usize, tb: f64, db: f64, spp: usize, revs: usize) -> (Surface, f64) {
    let omega = MT * C0 / RT;
    let dt = 2.0 * PI / omega / spp as f64;
    let ns = spp * revs;
    let fmag = (tb * tb + db * db).sqrt();
    let mut faces = Vec::new();
    for blade in 0..b {
        let phi_b = 2.0 * PI * blade as f64 / b as f64;
        let mut f = Face {
            area: 1.0,
            x: vec![],
            n: vec![],
            v: vec![],
            p: vec![],
        };
        for i in 0..ns {
            let ang = omega * i as f64 * dt + phi_b;
            let (s, c) = ang.sin_cos();
            f.x.push([RT * c, RT * s, 0.0]);
            f.v.push([-omega * RT * s, omega * RT * c, 0.0]);
            f.n.push([-db * s / fmag, db * c / fmag, tb / fmag]);
            f.p.push(fmag);
        }
        faces.push(f);
    }
    (
        Surface::impermeable(RHO0, C0, 0.0, dt, faces, SourceMode::LoadingOnly),
        omega,
    )
}

/// Harmonic amplitudes (peak) at n Omega, n = 1..=nmax, from one revolution of observer samples.
fn rotor_harmonics(s: &Surface, omega: f64, obs: V3, nmax: usize) -> Vec<f64> {
    let rev = 2.0 * PI / omega;
    let (lo, _) = s.valid_window(obs).unwrap();
    let n = 256;
    let ts: Vec<f64> = (0..n).map(|i| lo + 1.0 * rev + rev * i as f64 / n as f64).collect();
    let sig: Vec<f64> = s.signal(obs, &ts).unwrap().iter().map(|g| g.p_loading).collect();
    (1..=nmax).map(|h| fundamental(&ts, &sig, h as f64 * omega).0).collect()
}

#[test]
fn w3_rotating_force_matches_gutin_type_formula() {
    let r = 2000.0;
    for b in [3usize, 1] {
        let (tb, db) = (30.0 / b as f64, 9.0 / b as f64);
        let (s, omega) = rotor_surface(b, tb, db, 256, 4);
        let rotor = Rotor {
            blades: b,
            omega,
            r_tip: RT,
            thrust: 30.0,
            torque: 9.0 * RT,
            c0: C0,
        };
        for deg in [45.0_f64, 90.0, 135.0] {
            let th = deg.to_radians();
            let obs = [r * th.sin(), 0.0, r * th.cos()];
            let nmax = 3 * b + 2;
            let h = rotor_harmonics(&s, omega, obs, nmax);
            let refs: Vec<f64> = (1..=3).map(|m| rotor.harmonic_amplitude(m, r, th).unwrap()).collect();
            let top = refs.iter().cloned().fold(0.0, f64::max);
            for m in 1..=3 {
                let got = h[m * b - 1];
                let exp = refs[m - 1];
                if exp >= 1e-3 * top {
                    let e = (got / exp - 1.0).abs();
                    println!("W3 B={b} theta={deg} m={m}: fwh {got:.5e} formula {exp:.5e} rel {e:.3e}");
                    assert!(e <= 1e-2, "W3 B={b} theta={deg} m={m}: {e:.3e} > 1e-2");
                }
            }
            if b == 3 {
                for n in [1usize, 2, 4, 5] {
                    let ratio = h[n - 1] / h[2];
                    println!("W3 B=3 theta={deg}: n={n} / n=3 = {ratio:.3e}");
                    assert!(ratio <= 1e-3, "W3 non-multiple harmonic n={n}: {ratio:.3e}");
                }
            }
        }
    }
}

#[test]
fn w3_formula_sanity_limits() {
    let rotor = Rotor {
        blades: 3,
        omega: 100.0,
        r_tip: 1.0,
        thrust: 30.0,
        torque: 9.0,
        c0: C0,
    };
    // thrust term vanishes at 90 degrees; amplitude equals the torque-only value there
    let a90 = rotor.harmonic_amplitude(1, 100.0, PI / 2.0).unwrap();
    let r2 = Rotor { thrust: 0.0, ..rotor };
    let a90b = r2.harmonic_amplitude(1, 100.0, PI / 2.0).unwrap();
    assert!((a90 / a90b - 1.0).abs() < 1e-12);
    // on the axis (theta = 0, pi) J_n(0) = 0 for n >= 1: silent
    assert!(rotor.harmonic_amplitude(1, 100.0, 0.0).unwrap().abs() < 1e-30);
}

#[test]
fn w4_observer_time_interpolation_order() {
    let r = 100.0;
    let th = 60f64.to_radians();
    let obs = [r * th.sin(), 0.0, r * th.cos()];
    let revs = 4;
    let run = |spp: usize, ts: &[f64]| -> Vec<f64> {
        let (s, _) = rotor_surface(1, 30.0, 9.0, spp, revs);
        s.signal(obs, ts).unwrap().iter().map(|g| g.p_loading).collect()
    };
    let omega = MT * C0 / RT;
    let rev = 2.0 * PI / omega;
    let (s128, _) = rotor_surface(1, 30.0, 9.0, 128, revs);
    let (lo, hi) = s128.valid_window(obs).unwrap();
    let t_a = lo + 0.5 * rev;
    assert!(t_a + rev < hi - 0.5 * rev, "test window must be interior");
    let ts: Vec<f64> = (0..200).map(|i| t_a + 1.07 * rev * i as f64 / 200.0).collect();
    let reference = run(2048, &ts);
    let err = |spp: usize| -> f64 {
        let s = run(spp, &ts);
        let d: Vec<f64> = s.iter().zip(&reference).map(|(a, b)| a - b).collect();
        rms(&d) / rms(&reference)
    };
    let (e1, e2, e3) = (err(128), err(256), err(512));
    let (pa, pb) = ((e1 / e2).log2(), (e2 / e3).log2());
    println!("W4 errors {e1:.3e} {e2:.3e} {e3:.3e}; orders {pa:.3} {pb:.3}");
    assert!(pa >= 3.5, "W4 order {pa:.3} < 3.5");
    assert!(pb >= 3.5, "W4 order {pb:.3} < 3.5");
}

// ---------------- uniform flow ----------------
fn small_pulsating(k: f64) -> Pulsating {
    Pulsating { a: 0.01, eps: 1e-6, k }
}

#[test]
#[ignore = "registered limit missed, ADR 0066 addendum 1 (reference is a point mass source, the pulsating sphere in a stream is not)"]
fn w5a_uniform_flow_on_axis_convective_amplification_known_miss() {
    let ps = small_pulsating(PI / 2.0);
    let w = ps.w();
    let q0 = 4.0 * PI * ps.a * ps.a * ps.u0();
    let u_inf = 0.3 * C0;
    let mach = u_inf / C0;
    let body = ps.surface(false, 6, 12).into_medium_frame([u_inf, 0.0, 0.0]).unwrap();
    for (name, xb, sign) in [("downstream", 1000.0, 1.0), ("upstream", -1000.0, -1.0)] {
        let path = medium_frame_observer([xb, 0.0, 0.0], [u_inf, 0.0, 0.0]);
        let tau_x = 1000.0 / (C0 + sign * u_inf);
        let (lo, hi) = body
            .valid_window_moving(&path, 0.9 * tau_x, tau_x + 0.2, ps.period() / 64.0)
            .unwrap();
        let ts = period_times(lo, ps.period());
        assert!(ts[255] < hi);
        let sig: Vec<f64> = body
            .signal_moving(&path, &ts)
            .unwrap()
            .iter()
            .map(|g| g.total())
            .collect();
        // Q = Q0 cos(w t): p' = rho0 Qdot(t - |X|/(c +- U)) / (4 pi |X| (1 +- M))
        let ex: Vec<f64> = ts
            .iter()
            .map(|&t| {
                -RHO0 * q0 * w * (w * (t - 1000.0 / (C0 + sign * u_inf))).sin()
                    / (4.0 * PI * 1000.0 * (1.0 + sign * mach))
            })
            .collect();
        let (ag, pg) = fundamental(&ts, &sig, w);
        let (ae, pe) = fundamental(&ts, &ex, w);
        let (da, dp) = ((ag / ae - 1.0).abs(), wrap(pg - pe).abs());
        println!("W5a {name}: amplitude {da:.3e}, phase {dp:.3e} rad");
        assert!(da <= 1e-2, "W5a {name}: amplitude {da:.3e}");
        assert!(dp <= 5e-3, "W5a {name}: phase {dp:.3e}");
    }
}

/// Velocity potential of a monopole Q(tau) = Q0 cos(w tau) moving at -U e_x (Lienard-Wiechert), medium frame.
fn phi_lw(x: V3, t: f64, q0: f64, w: f64, u: f64) -> f64 {
    let y = |tau: f64| [-u * tau, 0.0, 0.0];
    let v = [-u, 0.0, 0.0];
    let dist = |tau: f64| {
        let yy = y(tau);
        let r = [x[0] - yy[0], x[1] - yy[1], x[2] - yy[2]];
        ((r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt(), r)
    };
    let mut tau = t - dist(0.0).0 / C0;
    for _ in 0..100 {
        let (rn, r) = dist(tau);
        let g = tau + rn / C0 - t;
        let gp = 1.0 - (v[0] * r[0] + v[1] * r[1] + v[2] * r[2]) / (C0 * rn);
        tau -= g / gp;
        if (g / gp).abs() < 1e-15 {
            break;
        }
    }
    let (rn, r) = dist(tau);
    let mr = (v[0] * r[0] + v[1] * r[1] + v[2] * r[2]) / (C0 * rn);
    -q0 * (w * tau).cos() / (4.0 * PI * rn * (1.0 - mr))
}

#[test]
#[ignore = "registered limit missed, ADR 0066 addendum 1"]
fn w5b_uniform_flow_off_axis_against_lienard_wiechert_known_miss() {
    let ps = small_pulsating(PI / 2.0);
    let w = ps.w();
    let q0 = 4.0 * PI * ps.a * ps.a * ps.u0();
    let u_inf = 0.3 * C0;
    let body = ps.surface(false, 6, 12).into_medium_frame([u_inf, 0.0, 0.0]).unwrap();
    let xb = [50.0 * 45f64.to_radians().cos(), 50.0 * 45f64.to_radians().sin(), 0.0];
    let path = medium_frame_observer(xb, [u_inf, 0.0, 0.0]);
    let (lo, _) = body.valid_window_moving(&path, 0.05, 0.4, ps.period() / 64.0).unwrap();
    let ts = period_times(lo, ps.period());
    let sig: Vec<f64> = body
        .signal_moving(&path, &ts)
        .unwrap()
        .iter()
        .map(|g| g.total())
        .collect();
    // p' = -rho0 d phi / dt at the fixed medium point x_m = x_b - U t0 e_x (8th-order central differences)
    const FD8: [f64; 9] = [
        1.0 / 280.0,
        -4.0 / 105.0,
        1.0 / 5.0,
        -4.0 / 5.0,
        0.0,
        4.0 / 5.0,
        -1.0 / 5.0,
        4.0 / 105.0,
        -1.0 / 280.0,
    ];
    let h = ps.period() * 4e-4;
    let ex: Vec<f64> = ts
        .iter()
        .map(|&t| {
            let xm = path(t);
            let d: f64 = FD8
                .iter()
                .enumerate()
                .map(|(k, c)| c * phi_lw(xm, t + (k as f64 - 4.0) * h, q0, w, u_inf))
                .sum::<f64>()
                / h;
            -RHO0 * d
        })
        .collect();
    let diff: Vec<f64> = sig.iter().zip(&ex).map(|(a, b)| a - b).collect();
    let e = rms(&diff) / rms(&ex);
    println!("W5b e = {e:.3e}");
    assert!(e <= 1e-3, "W5b e = {e:.3e}");
}

#[test]
fn w5c_zero_flow_moving_observer_reproduces_fixed_observer() {
    let ps = Pulsating {
        a: 1.0,
        eps: 1e-5,
        k: PI / 2.0,
    };
    let s = ps.surface(true, 8, 16);
    let obs = [20.0, 5.0, 3.0];
    let (lo, _) = s.valid_window(obs).unwrap();
    let ts = period_times(lo, ps.period());
    let a = s.signal(obs, &ts).unwrap();
    let b = s.signal_moving(&|_| obs, &ts).unwrap();
    let scale = a.iter().map(|g| g.total().abs()).fold(0.0, f64::max);
    let worst = a
        .iter()
        .zip(&b)
        .map(|(x, y)| ((x.total() - y.total()).abs()).max((x.p_thickness - y.p_thickness).abs()))
        .fold(0.0, f64::max);
    println!("W5c worst difference {:.3e} (scale {scale:.3e})", worst / scale);
    assert!(worst / scale <= 1e-12);
}

// ---------------- sound power ----------------
fn sphere_series(s: &Surface, radius: f64, period: f64, nth: usize, nph: usize) -> (Vec<f64>, Vec<Vec<f64>>) {
    let quad = sphere_quadrature(nth, nph).unwrap();
    let mut weights = Vec::new();
    let mut series = Vec::new();
    for (dir, wq) in quad {
        let obs = [radius * dir[0], radius * dir[1], radius * dir[2]];
        let (lo, _) = s.valid_window(obs).unwrap();
        let n = 128; // two whole periods at 64 per period
        let ts: Vec<f64> = (0..n).map(|i| lo + period + i as f64 * period / 64.0).collect();
        series.push(s.signal(obs, &ts).unwrap().iter().map(|g| g.total()).collect());
        weights.push(wq);
    }
    (weights, series)
}

#[test]
fn w6b_monopole_sound_power_through_fwh() {
    let ps = small_pulsating(PI / 2.0);
    let s = ps.surface(false, 6, 12);
    let r = 10.0;
    let (w, ser) = sphere_series(&s, r, ps.period(), 8, 16);
    let power = sound_power_sphere(r, &w, &ser, RHO0, C0).unwrap();
    let q0 = 4.0 * PI * ps.a * ps.a * ps.u0();
    let exact = RHO0 * ps.w() * ps.w() * q0 * q0 / (8.0 * PI * C0);
    let e = (power / exact - 1.0).abs();
    println!("W6b power {power:.6e} exact {exact:.6e} rel {e:.3e}");
    assert!(e <= 1e-3, "W6b {e:.3e}");
}

#[test]
fn w6c_dipole_sound_power_through_fwh() {
    let (a, k) = (0.01, PI / 2.0);
    let w = k * C0;
    let period = 2.0 * PI / w;
    let dt = period / 64.0;
    let ns = 64 * 14;
    let p0 = 1000.0;
    let f0 = p0 * 4.0 * PI * a * a / 3.0;
    let mut faces = Vec::new();
    for (nrm, wq) in sphere_quadrature(8, 16).unwrap() {
        let mut f = Face {
            area: wq * a * a,
            x: vec![],
            n: vec![],
            v: vec![],
            p: vec![],
        };
        for i in 0..ns {
            let t = i as f64 * dt;
            f.x.push([a * nrm[0], a * nrm[1], a * nrm[2]]);
            f.n.push(nrm);
            f.v.push([0.0; 3]);
            f.p.push(p0 * (w * t).sin() * nrm[0]);
        }
        faces.push(f);
    }
    let s = Surface::impermeable(RHO0, C0, 0.0, dt, faces, SourceMode::LoadingOnly);
    let r = 500.0;
    let (wt, ser) = sphere_series(&s, r, period, 8, 16);
    let power = sound_power_sphere(r, &wt, &ser, RHO0, C0).unwrap();
    let exact = w * w * f0 * f0 / (24.0 * PI * RHO0 * C0.powi(3));
    let e = (power / exact - 1.0).abs();
    println!("W6c power {power:.6e} exact {exact:.6e} rel {e:.3e}");
    assert!(e <= 1e-3, "W6c {e:.3e}");
    assert!(mean_square(&ser[0]).unwrap() > 0.0);
}

// ---------------- W5d (ADR 0066 addendum 1): permeable data surface carrying the exact convected-monopole field ----------------
// Body frame: a sphere of radius 0.3 m fixed about the source, stream U e_x past it. Data on the surface are what a
// body-frame CFD run would deliver: p', rho' = p'/c^2 and the TOTAL body-frame velocity u_b = u_m + U, v = 0.

const FD8C: [f64; 9] = [
    1.0 / 280.0,
    -4.0 / 105.0,
    1.0 / 5.0,
    -4.0 / 5.0,
    0.0,
    4.0 / 5.0,
    -1.0 / 5.0,
    4.0 / 105.0,
    -1.0 / 280.0,
];

fn d8(h: f64, f: &dyn Fn(f64) -> f64) -> f64 {
    FD8C.iter()
        .enumerate()
        .map(|(k, c)| c * f((k as f64 - 4.0) * h))
        .sum::<f64>()
        / h
}

fn flow_w() -> f64 {
    PI / 2.0 * C0
}
fn flow_q0() -> f64 {
    4.0 * PI * 0.01 / (RHO0 * flow_w())
}

fn permeable_flow_surface() -> &'static Surface {
    static S: std::sync::OnceLock<Surface> = std::sync::OnceLock::new();
    S.get_or_init(|| {
        let u_inf = 0.3 * C0;
        let w = flow_w();
        let period = 2.0 * PI / w;
        let dt = period / 64.0;
        let ns = 64 * 14;
        let a = 0.3;
        let mut elements = Vec::new();
        for (nrm, wq) in sphere_quadrature(6, 12).unwrap() {
            let xb = [a * nrm[0], a * nrm[1], a * nrm[2]];
            let (mut p, mut rho, mut u) = (vec![], vec![], vec![]);
            for i in 0..ns {
                let t = i as f64 * dt;
                let xm = [xb[0] - u_inf * t, xb[1], xb[2]];
                let phi = |x: V3, tt: f64| phi_lw(x, tt, flow_q0(), w, u_inf);
                let pp = -RHO0 * d8(period * 4e-4, &|d| phi(xm, t + d));
                let mut um = [0.0; 3];
                for (c, uc) in um.iter_mut().enumerate() {
                    *uc = d8(0.01, &|d| {
                        let mut y = xm;
                        y[c] += d;
                        phi(y, t)
                    });
                }
                p.push(pp);
                rho.push(pp / (C0 * C0));
                u.push([um[0] + u_inf, um[1], um[2]]);
            }
            elements.push(rcfd_post::fwh::Element {
                area: wq * a * a,
                x: vec![xb; ns],
                n: vec![nrm; ns],
                v: vec![[0.0; 3]; ns],
                p,
                rho,
                u,
            });
        }
        let body = Surface {
            rho0: RHO0,
            c0: C0,
            t0: 0.0,
            dt,
            elements,
        };
        body.into_medium_frame([u_inf, 0.0, 0.0]).unwrap()
    })
}

#[test]
fn w5d_permeable_flow_on_axis_closed_form() {
    let u_inf = 0.3 * C0;
    let mach = 0.3;
    let w = flow_w();
    let period = 2.0 * PI / w;
    let body = permeable_flow_surface();
    for (name, xb, sign) in [("downstream", 1000.0, 1.0), ("upstream", -1000.0, -1.0)] {
        let path = medium_frame_observer([xb, 0.0, 0.0], [u_inf, 0.0, 0.0]);
        let tau_x = 1000.0 / (C0 + sign * u_inf);
        let (lo, hi) = body
            .valid_window_moving(&path, 0.9 * tau_x, tau_x + 0.2, period / 64.0)
            .unwrap();
        let ts = period_times(lo, period);
        assert!(ts[255] < hi);
        let sig: Vec<f64> = body
            .signal_moving(&path, &ts)
            .unwrap()
            .iter()
            .map(|g| g.total())
            .collect();
        let ex: Vec<f64> = ts
            .iter()
            .map(|&t| {
                -RHO0 * flow_q0() * w * (w * (t - 1000.0 / (C0 + sign * u_inf))).sin()
                    / (4.0 * PI * 1000.0 * (1.0 + sign * mach))
            })
            .collect();
        let (ag, pg) = fundamental(&ts, &sig, w);
        let (ae, pe) = fundamental(&ts, &ex, w);
        let (da, dp) = ((ag / ae - 1.0).abs(), wrap(pg - pe).abs());
        println!("W5d {name}: amplitude {da:.3e}, phase {dp:.3e} rad");
        assert!(da <= 1e-2, "W5d {name}: amplitude {da:.3e}");
        assert!(dp <= 5e-3, "W5d {name}: phase {dp:.3e}");
    }
}

#[test]
fn w5d_permeable_flow_off_axis_against_lienard_wiechert() {
    let u_inf = 0.3 * C0;
    let w = flow_w();
    let period = 2.0 * PI / w;
    let body = permeable_flow_surface();
    let xb = [50.0 * 45f64.to_radians().cos(), 50.0 * 45f64.to_radians().sin(), 0.0];
    let path = medium_frame_observer(xb, [u_inf, 0.0, 0.0]);
    let (lo, _) = body.valid_window_moving(&path, 0.05, 0.4, period / 64.0).unwrap();
    let ts = period_times(lo, period);
    let sig: Vec<f64> = body
        .signal_moving(&path, &ts)
        .unwrap()
        .iter()
        .map(|g| g.total())
        .collect();
    let ex: Vec<f64> = ts
        .iter()
        .map(|&t| -RHO0 * d8(period * 4e-4, &|d| phi_lw(path(t), t + d, flow_q0(), w, u_inf)))
        .collect();
    let diff: Vec<f64> = sig.iter().zip(&ex).map(|(a, b)| a - b).collect();
    let e = rms(&diff) / rms(&ex);
    println!("W5d off axis e = {e:.3e}");
    assert!(e <= 1e-3, "W5d e = {e:.3e}");
}
