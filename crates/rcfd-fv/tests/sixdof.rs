//! Six-degree-of-freedom rigid-body checks R1-R6 of ADR 0033 against closed forms. The limits are those registered in
//! the ADR before the code ran; where a registered closed form turned out to carry a sign error the corrected check is
//! a new test and the original is kept ignored.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::sixdof::{Body, BodySpec, BodyState, Integrator, Rotation};

const NO_FLUID: (Vec3, Vec3) = (vec3::ZERO, vec3::ZERO);

fn translating(mass: f64, locked: [bool; 3]) -> BodySpec {
    let mut s = BodySpec::new(mass, [1.0, 1.0, 1.0]);
    s.translation_locked = locked;
    s
}

#[test]
fn r1_free_fall_with_linear_drag() {
    let (m, c) = (2.0, 3.0);
    let g = -9.81;
    let tau = m / c;
    let mut spec = translating(m, [true, true, false]);
    spec.gravity = [0.0, 0.0, g];
    spec.drag = c;
    let mut b = Body::new(spec, BodyState::at([0.0, 0.0, 0.0])).unwrap();
    let dt = tau / 40.0;
    for _ in 0..200 {
        b.advance(dt, Integrator::Rk4, NO_FLUID).unwrap();
    }
    let t = b.state.t;
    assert!((t - 5.0 * tau).abs() < 1e-12);
    let vt = m * g / c;
    let v_ex = vt * (1.0 - (-t / tau).exp());
    let z_ex = vt * (t - tau * (1.0 - (-t / tau).exp()));
    let ev = ((b.state.v[2] - v_ex) / v_ex).abs();
    let ez = ((b.state.x[2] - z_ex) / z_ex).abs();
    println!("R1: relative error v {ev:.2e}, z {ez:.2e} (limit 1e-8)");
    assert!(ev <= 1e-8 && ez <= 1e-8);
    assert_eq!(b.state.x[0], 0.0);
    assert_eq!(b.state.x[1], 0.0);
    assert_eq!(b.state.v[0], 0.0);
}

/// Period from the upward zero crossings 1 and 11 of `x`, linear interpolation.
fn oscillator_period(integrator: Integrator, steps_per_period: usize, damping: f64) -> f64 {
    let k = (2.0 * std::f64::consts::PI).powi(2);
    let mut spec = translating(1.0, [false, true, true]);
    spec.stiffness = [k, 0.0, 0.0];
    spec.damping = [damping, 0.0, 0.0];
    let mut s = BodyState::at([1.0, 0.0, 0.0]);
    s.v = [0.0; 3];
    let mut b = Body::new(spec, s).unwrap();
    let dt = 1.0 / steps_per_period as f64;
    let mut crossings = Vec::new();
    let mut prev = b.state;
    while crossings.len() < 11 {
        b.advance(dt, integrator, NO_FLUID).unwrap();
        let cur = b.state;
        if prev.x[0] < 0.0 && cur.x[0] >= 0.0 {
            crossings.push(prev.t + dt * (-prev.x[0]) / (cur.x[0] - prev.x[0]));
        }
        prev = cur;
    }
    (crossings[10] - crossings[0]) / 10.0
}

#[test]
fn r2_spring_mass_period_newmark_and_rk4() {
    let err = |i: Integrator, n: usize| (oscillator_period(i, n, 0.0) - 1.0).abs();
    let (n25, n50) = (err(Integrator::Newmark, 25), err(Integrator::Newmark, 50));
    let ratio_n = n25 / n50;
    println!("R2 Newmark: period error T/50 {n50:.3e} (limit 2e-3), ratio T/25 : T/50 {ratio_n:.3} (limit [3.5, 4.5])");
    assert!(n50 <= 2e-3, "{n50}");
    assert!((3.5..=4.5).contains(&ratio_n), "{ratio_n}");
    let (r25, r50) = (err(Integrator::Rk4, 25), err(Integrator::Rk4, 50));
    let ratio_r = r25 / r50;
    println!("R2 RK4: period error T/50 {r50:.3e} (limit 1e-5), ratio {ratio_r:.2} (limit >= 12)");
    assert!(r50 <= 1e-5, "{r50}");
    assert!(ratio_r >= 12.0, "{ratio_r}");
}

#[test]
fn r3_damped_oscillator_rk4() {
    let k = (2.0 * std::f64::consts::PI).powi(2);
    let w = k.sqrt();
    let zeta = 0.2;
    let c = zeta * 2.0 * k.sqrt();
    let mut spec = translating(1.0, [false, true, true]);
    spec.stiffness = [k, 0.0, 0.0];
    spec.damping = [c, 0.0, 0.0];
    let mut b = Body::new(spec, BodyState::at([1.0, 0.0, 0.0])).unwrap();
    let wd = w * (1.0 - zeta * zeta).sqrt();
    let dt = 1.0 / 100.0;
    let mut worst: f64 = 0.0;
    for _ in 0..300 {
        b.advance(dt, Integrator::Rk4, NO_FLUID).unwrap();
        let t = b.state.t;
        let ex = (-zeta * w * t).exp() * ((wd * t).cos() + zeta * w / wd * (wd * t).sin());
        worst = worst.max((b.state.x[0] - ex).abs());
    }
    println!("R3: max |x - exact| {worst:.3e} (limit 1e-5)");
    assert!(worst <= 1e-5);
}

fn tops(i: [f64; 3], w: Vec3) -> Body {
    let mut spec = BodySpec::new(1.0, i);
    spec.rotation = Rotation::Free;
    let mut s = BodyState::at(vec3::ZERO);
    s.w = w;
    Body::new(spec, s).unwrap()
}

#[test]
fn r4_torque_free_rotation() {
    // (i) asymmetric body: world angular momentum and energy are conserved
    let mut b = tops([1.0, 2.0, 3.0], [1.0, 0.1, 0.5]);
    let (l0, e0) = (b.angular_momentum_world(), b.kinetic_energy());
    let (mut dl, mut de, mut dm): (f64, f64, f64) = (0.0, 0.0, 0.0);
    for _ in 0..4000 {
        b.advance(0.005, Integrator::Rk4, NO_FLUID).unwrap();
        let l = b.angular_momentum_world();
        dl = dl.max(vec3::norm(vec3::sub(l, l0)) / vec3::norm(l0));
        dm = dm.max((vec3::norm(l) - vec3::norm(l0)).abs() / vec3::norm(l0));
        de = de.max((b.kinetic_energy() - e0).abs() / e0);
    }
    println!("R4 (i): drift of L {dl:.2e}, |L| {dm:.2e}, E {de:.2e} (limit 1e-7)");
    assert!(dl <= 1e-7 && dm <= 1e-7 && de <= 1e-7);

    // (iii) isotropic: constant to 1e-14
    let mut iso = tops([2.0, 2.0, 2.0], [1.0, 0.1, 0.5]);
    for _ in 0..1000 {
        iso.advance(0.005, Integrator::Rk4, NO_FLUID).unwrap();
    }
    let dw = vec3::norm(vec3::sub(iso.state.w, [1.0, 0.1, 0.5]));
    println!("R4 (iii): isotropic w drift {dw:.2e} (limit 1e-14)");
    assert!(dw <= 1e-14);
}

fn axisymmetric_error(sign: f64) -> f64 {
    let (i1, i3) = (1.0, 2.0);
    let w0 = [1.0, 0.1, 0.5];
    let mut b = tops([i1, i1, i3], w0);
    let omega = (i3 - i1) * w0[2] / i1;
    let mut worst: f64 = 0.0;
    for _ in 0..4000 {
        b.advance(0.005, Integrator::Rk4, NO_FLUID).unwrap();
        let t = b.state.t;
        let (c, s) = ((sign * omega * t).cos(), (sign * omega * t).sin());
        // (w1 + i w2) = (w1(0) + i w2(0)) e^{i sign Omega t}
        let w1 = w0[0] * c - w0[1] * s;
        let w2 = w0[0] * s + w0[1] * c;
        worst = worst
            .max((b.state.w[0] - w1).abs())
            .max((b.state.w[1] - w2).abs())
            .max((b.state.w[2] - w0[2]).abs());
    }
    worst
}

/// The sign of the registered closed form was `e^{-i Omega t}`; Euler's equations give `e^{+i Omega t}` for
/// `I_3 > I_1` (`w1' = -Omega w2`, `w2' = Omega w1`). Kept as registered, ignored.
#[test]
#[ignore = "registered closed form has the wrong sign of the precession (ADR 0033 addendum); superseded by r4b"]
fn r4_axisymmetric_registered_sign_known_miss() {
    let e = axisymmetric_error(-1.0);
    println!("R4 (ii) registered sign: error {e:.2e}");
    assert!(e <= 1e-7, "{e}");
}

#[test]
fn r4b_axisymmetric_closed_form() {
    let e = axisymmetric_error(1.0);
    println!("R4 (ii): body angular velocity vs closed form {e:.2e} (limit 1e-7)");
    assert!(e <= 1e-7, "{e}");
}

/// Complete elliptic integral of the first kind by the arithmetic-geometric mean.
fn elliptic_k(k: f64) -> f64 {
    let (mut a, mut b) = (1.0, (1.0 - k * k).sqrt());
    for _ in 0..40 {
        let (an, bn) = (0.5 * (a + b), (a * b).sqrt());
        a = an;
        b = bn;
    }
    std::f64::consts::FRAC_PI_2 / a
}

/// Zero crossings of `theta` (cubic Hermite interpolation with the rate) of a hinged body released at `theta0`; returns
/// the period from crossings 1 and 11 (ten half periods).
fn pendulum_period(theta0: f64, integrator: Integrator, steps_per_period: usize, t_guess: f64) -> f64 {
    let (m, d) = (3.0, 0.5);
    let mut spec = BodySpec::new(m, [1.0, 0.4, 1.0]);
    spec.rotation = Rotation::Hinge {
        point: [0.0; 3],
        axis: [0.0, 1.0, 0.0],
    };
    spec.gravity = [0.0, 0.0, -9.81];
    // the body hangs below the pivot; theta about +y turns -z toward -x
    let mut s = BodyState::at([0.0, 0.0, -d]);
    s.theta = theta0;
    let mut b = Body::new(spec, s).unwrap();
    let dt = t_guess / steps_per_period as f64;
    let mut cr = Vec::new();
    let mut prev = b.state;
    while cr.len() < 11 {
        b.advance(dt, integrator, NO_FLUID).unwrap();
        let cur = b.state;
        if prev.theta * cur.theta < 0.0 {
            // cubic Hermite on [prev.t, cur.t]
            let h = cur.t - prev.t;
            let hermite = |s: f64| {
                let (h00, h10, h01, h11) = (
                    2.0 * s.powi(3) - 3.0 * s * s + 1.0,
                    s.powi(3) - 2.0 * s * s + s,
                    -2.0 * s.powi(3) + 3.0 * s * s,
                    s.powi(3) - s * s,
                );
                h00 * prev.theta + h10 * h * prev.theta_dot + h01 * cur.theta + h11 * h * cur.theta_dot
            };
            let (mut lo, mut hi) = (0.0, 1.0);
            let flo = hermite(lo);
            for _ in 0..80 {
                let mid = 0.5 * (lo + hi);
                if hermite(mid) * flo > 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            cr.push(prev.t + h * 0.5 * (lo + hi));
        }
        prev = cur;
    }
    2.0 * (cr[10] - cr[0]) / 10.0
}

#[test]
fn r5_compound_pendulum() {
    let (m, g, d): (f64, f64, f64) = (3.0, 9.81, 0.5);
    let ip = 0.4 + m * d * d;
    let t_small = 2.0 * std::f64::consts::PI * (ip / (m * g * d)).sqrt();
    // small amplitude, Newmark dt = T/200
    let t = pendulum_period(0.01, Integrator::Newmark, 200, t_small);
    let e_small = (t / t_small - 1.0).abs();
    println!("R5 small amplitude Newmark: period error {e_small:.2e} (limit 1e-3)");
    assert!(e_small <= 1e-3, "{e_small}");
    // large amplitude 1 rad
    let th0: f64 = 1.0;
    let t_large = 4.0 * (ip / (m * g * d)).sqrt() * elliptic_k((0.5 * th0).sin());
    let t_rk = pendulum_period(th0, Integrator::Rk4, 400, t_large);
    let e_rk = (t_rk / t_large - 1.0).abs();
    let t_nm = pendulum_period(th0, Integrator::Newmark, 200, t_large);
    let e_nm = (t_nm / t_large - 1.0).abs();
    println!("R5 theta0 = 1: RK4 T/400 period error {e_rk:.2e} (limit 1e-5), Newmark T/200 {e_nm:.2e} (limit 2e-3)");
    assert!(e_rk <= 1e-5, "{e_rk}");
    assert!(e_nm <= 2e-3, "{e_nm}");
}

#[test]
fn r6_constraints_and_hooks() {
    // locked translation: nothing moves under a force with all components
    let mut b = Body::new(translating(1.0, [true; 3]), BodyState::at([0.3, 0.2, 0.1])).unwrap();
    for _ in 0..50 {
        b.advance(0.01, Integrator::Rk4, ([1.0, 2.0, 3.0], [1.0, 1.0, 1.0]))
            .unwrap();
        b.advance(0.01, Integrator::Newmark, ([1.0, 2.0, 3.0], [1.0, 1.0, 1.0]))
            .unwrap();
    }
    assert_eq!(b.state.x, [0.3, 0.2, 0.1]);
    assert_eq!(b.state.v, [0.0; 3]);
    // locked rotation: orientation unchanged exactly
    assert_eq!(b.state.q, [1.0, 0.0, 0.0, 0.0]);
    // about-axis rotation with a torque about another axis
    let mut spec = BodySpec::new(1.0, [1.0, 2.0, 3.0]);
    spec.rotation = Rotation::AboutAxis { axis: [0.0, 0.0, 1.0] };
    let mut a = Body::new(spec, BodyState::at(vec3::ZERO)).unwrap();
    for _ in 0..100 {
        a.advance(0.01, Integrator::Rk4, (vec3::ZERO, [1.0, 0.7, 0.5])).unwrap();
    }
    let r = a.rotation_matrix();
    let w_world = [0, 1, 2].map(|i| r[i][0] * a.state.w[0] + r[i][1] * a.state.w[1] + r[i][2] * a.state.w[2]);
    let off = vec3::norm(vec3::cross(w_world, [0.0, 0.0, 1.0]));
    println!("R6: about-axis angular velocity off the axis by {off:.2e} (limit 1e-14)");
    assert!(off <= 1e-14);
    assert!(a.state.theta_dot > 0.0);
    // non-principal axis is rejected with a located message
    let mut bad = BodySpec::new(1.0, [1.0, 2.0, 3.0]);
    bad.inertia[0][1] = 0.3;
    bad.inertia[1][0] = 0.3;
    bad.rotation = Rotation::AboutAxis { axis: [1.0, 0.0, 0.0] };
    let e = Body::new(bad, BodyState::at(vec3::ZERO)).unwrap_err();
    assert!(e.starts_with("six_dof.axis") && e.contains("principal"), "{e}");
    // a hook returning -c v reproduces the built-in drag
    let mut spec1 = translating(2.0, [false; 3]);
    spec1.drag = 0.7;
    spec1.gravity = [0.1, -0.2, -9.81];
    let mut spec2 = translating(2.0, [false; 3]);
    spec2.gravity = spec1.gravity;
    let mut s0 = BodyState::at(vec3::ZERO);
    s0.v = [1.0, 2.0, 3.0];
    let mut b1 = Body::new(spec1, s0).unwrap();
    let mut b2 = Body::new(spec2, s0).unwrap();
    b2.set_hook(Box::new(|s, _t| (vec3::scale(s.v, -0.7), vec3::ZERO)));
    for _ in 0..100 {
        b1.advance(0.01, Integrator::Rk4, NO_FLUID).unwrap();
        b2.advance(0.01, Integrator::Rk4, NO_FLUID).unwrap();
    }
    let dx = vec3::norm(vec3::sub(b1.state.x, b2.state.x));
    println!("R6: hook vs built-in drag {dx:.2e} (limit 1e-14)");
    assert!(dx <= 1e-14);
}
