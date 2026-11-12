//! Characteristic boundary conditions of the density-based compressible solver
//! (ADR 0006): total pressure / total temperature inlet, static-pressure
//! outlet with subsonic / supersonic switching, partially non-reflecting
//! outlet and a Riemann-invariant far field; and the impedance wall of F-243
//! (ADR 0233).
//!
//! Each condition maps the interior cell state and the outward unit normal to a
//! boundary *face state*, whose physical flux the solver uses directly. The
//! analysis is one-dimensional along the face normal (Thompson, J. Comput.
//! Phys. 68 (1987) 1-24; Poinsot and Lele, J. Comput. Phys. 101 (1992)
//! 104-129; Blazek, *Computational Fluid Dynamics*, ch. 8).
//!
//! [`MsdWall`] is the frequency-dependent wall of F-332 (ADR 0243): a mass-spring-damper wall with its own state per
//! face, which the solver advances with its Runge-Kutta stages.

use crate::riemann::{Gas, Prim};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};

/// Subsonic inlet from stagnation conditions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TotalInlet {
    /// Total pressure.
    pub p0: f64,
    /// Total temperature.
    pub t0: f64,
    /// Specific gas constant.
    pub r_gas: f64,
    /// Unit flow direction (pointing into the domain).
    pub dir: Vec3,
}

/// Face state of a total pressure / total temperature inlet. The characteristic leaving
/// the domain, `J = u_n + 2a/(gamma-1)`, is taken from the interior.
pub fn total_inlet(gas: Gas, bc: &TotalInlet, interior: &Prim, n: Vec3) -> Prim {
    let g = gas.gamma;
    let a_i = interior.sound_speed(gas);
    let jinv = vec3::dot(interior.u, n) + 2.0 * a_i / (g - 1.0);
    let c = vec3::dot(bc.dir, n);
    if c >= -1e-12 {
        return *interior;
    }
    let a0sq = g * bc.r_gas * bc.t0;
    let vmax = (2.0 * a0sq / (g - 1.0)).sqrt();
    let a_of = |v: f64| (a0sq - 0.5 * (g - 1.0) * v * v).max(0.0).sqrt();
    let f = |v: f64| v * c + 2.0 * a_of(v) / (g - 1.0) - jinv;
    let v = if f(0.0) <= 0.0 {
        0.0
    } else {
        let (mut lo, mut hi) = (0.0, vmax);
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if f(mid) > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    };
    let a = a_of(v);
    let p = bc.p0 * (a * a / a0sq).powf(g / (g - 1.0));
    Prim {
        rho: g * p / (a * a),
        u: vec3::scale(bc.dir, v),
        p,
    }
}

/// Subsonic outlet. The incoming acoustic characteristic is changed so that the face
/// pressure becomes `p_i + sigma (p_target - p_i)`; entropy, tangential velocity and the
/// outgoing invariant `u_n + 2a/(gamma-1)` are kept (an exact simple-wave relation).
/// `sigma = 1` is a fixed static pressure, `sigma = 0` a non-reflecting zero-gradient
/// outlet. A supersonic outflow (normal Mach number >= 1) takes the interior state.
pub fn pressure_outlet(gas: Gas, p_target: f64, sigma: f64, interior: &Prim, n: Vec3) -> Prim {
    let g = gas.gamma;
    let a_i = interior.sound_speed(gas);
    let un = vec3::dot(interior.u, n);
    if un >= a_i {
        return *interior;
    }
    let p_f = interior.p + sigma * (p_target - interior.p);
    let ratio = p_f / interior.p;
    let rho_f = interior.rho * ratio.powf(1.0 / g);
    let a_f = a_i * ratio.powf((g - 1.0) / (2.0 * g));
    let un_f = un - 2.0 * (a_f - a_i) / (g - 1.0);
    Prim {
        rho: rho_f,
        u: vec3::add(interior.u, vec3::scale(n, un_f - un)),
        p: p_f,
    }
}

/// Largest number of Newton steps of [`impedance_wall`].
const IMPEDANCE_ITERATIONS: usize = 100;

/// Face state of a locally reacting wall with the real specific acoustic impedance `z` [Pa s/m], `z >= 0` and finite
/// (F-243, ADR 0233 DR5): the face keeps the entropy, the tangential velocity and the outgoing invariant
/// `u_n + 2a/(gamma-1)` of the interior and satisfies `p_f - p_ref = z u_n,f`, with `u_n` the normal velocity relative to
/// the face. A weak plane wave at normal incidence is reflected with `R = (z - rho c)/(z + rho c)`: `z = 0` is the pressure
/// outlet ([`pressure_outlet`] with `sigma = 1`, bit for bit), `z = rho c` of the ambient state reflects nothing and holds
/// `p_ref`, a large `z` approaches a rigid wall. A supersonic outflow (normal Mach number >= 1) takes the interior state.
pub fn impedance_wall(gas: Gas, p_ref: f64, z: f64, interior: &Prim, n: Vec3) -> Prim {
    if z == 0.0 {
        return pressure_outlet(gas, p_ref, 1.0, interior, n);
    }
    let g = gas.gamma;
    let a_i = interior.sound_speed(gas);
    let un = vec3::dot(interior.u, n);
    if un >= a_i {
        return *interior;
    }
    let expo = 2.0 * g / (g - 1.0);
    // sound speed and pressure of the simple wave that brings the normal velocity to `u`
    let wave = |u: f64| {
        let a = (a_i - 0.5 * (g - 1.0) * (u - un)).max(0.0);
        (a, interior.p * (a / a_i).powf(expo))
    };
    let resid = |u: f64| p_ref + z * u - wave(u).1;
    // bracket: the residual grows with u; the upper end is the vacuum speed
    let mut hi = un + 2.0 * a_i / (g - 1.0);
    if resid(hi) <= 0.0 {
        return *interior;
    }
    let mut lo = un - a_i;
    let mut k = 0;
    while resid(lo) >= 0.0 && k < 64 {
        k += 1;
        lo = un - a_i * 2f64.powi(k);
    }
    let mut u = (un + (interior.p - p_ref - z * un) / (z + interior.rho * a_i)).clamp(lo, hi);
    for _ in 0..IMPEDANCE_ITERATIONS {
        let (a, p) = wave(u);
        let r = p_ref + z * u - p;
        if r == 0.0 {
            break;
        }
        if r > 0.0 {
            hi = u;
        } else {
            lo = u;
        }
        // d(resid)/du = z + rho_f a_f = z + gamma p_f / a_f
        let slope = z + if a > 0.0 { g * p / a } else { 0.0 };
        let mut next = u - r / slope;
        if !(next > lo && next < hi) {
            next = 0.5 * (lo + hi);
        }
        let step = (next - u).abs();
        u = next;
        if step <= 4.0 * f64::EPSILON * (a_i + u.abs()) {
            break;
        }
    }
    let (_, p_f) = wave(u);
    Prim {
        rho: interior.rho * (p_f / interior.p).powf(1.0 / g),
        u: vec3::add(interior.u, vec3::scale(n, u - un)),
        p: p_f,
    }
}

/// Face state of a wall that moves with the normal velocity `v` relative to the face, as a transpiration velocity
/// (F-332, ADR 0243 DR2): the face keeps the entropy, the tangential velocity and the outgoing invariant
/// `u_n + 2a/(gamma-1)` of the interior, as [`impedance_wall`] does, and has the normal velocity `v`. For a weak wave the
/// face pressure is `p_i + rho c (u_n,i - v)`. A supersonic outflow (normal Mach number >= 1) takes the interior state.
pub fn piston_face(gas: Gas, interior: &Prim, n: Vec3, v: f64) -> Prim {
    let g = gas.gamma;
    let a_i = interior.sound_speed(gas);
    let un = vec3::dot(interior.u, n);
    if un >= a_i {
        return *interior;
    }
    let a_f = (a_i - 0.5 * (g - 1.0) * (v - un)).max(0.0);
    let p_f = interior.p * (a_f / a_i).powf(2.0 * g / (g - 1.0));
    Prim {
        rho: interior.rho * (p_f / interior.p).powf(1.0 / g),
        u: vec3::add(interior.u, vec3::scale(n, v - un)),
        p: p_f,
    }
}

/// Safety factor of [`MsdWall::dt_limit`] against the edge of the stability disc (ADR 0243 DR5).
const MSD_DT_SAFETY: f64 = 0.8;

/// Locally reacting mass-spring-damper wall (F-332, ADR 0243): per unit area a plate of mass `m` on a spring `k` and a
/// damper `r`, driven by the face pressure: `m dv/dt + r v + k xi = p_face - p`, `d xi/dt = v`, with `xi` the
/// displacement along the outward normal and the fluid's normal velocity at the face equal to `v`. For a tone
/// `e^{i omega t}` this is the specific impedance `Z(omega) = r + i (omega m - k/omega)`, and a plane wave at normal
/// incidence is reflected with `(Z - rho c)/(Z + rho c)`. `m = 0` is the algebraic form, whose face state is
/// [`impedance_wall`] with the reference pressure `p + k xi`; `m = 0`, `k = 0` is [`impedance_wall`] with `z = r`, bit
/// for bit. Set on a patch with `CompressibleSolver::set_msd_wall`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MsdWall {
    /// Reference (ambient) static pressure, at which the wall is at rest with `xi = 0`.
    pub p: f64,
    /// Damper [Pa s/m], finite and `>= 0`.
    pub r: f64,
    /// Mass per area [kg/m^2], finite and `>= 0`.
    pub m: f64,
    /// Spring [Pa/m], finite and `>= 0`.
    pub k: f64,
}

/// The face state and the rates of the wall state of one face ([`MsdWall::rates`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MsdRates {
    /// The boundary face state, whose physical flux the solver uses.
    pub face: Prim,
    /// `d xi/dt`.
    pub xi_dot: f64,
    /// `dv/dt` (zero for `m = 0`, where the velocity is not a state).
    pub v_dot: f64,
}

impl MsdWall {
    /// An error unless `r`, `m`, `k` are finite and `>= 0` and `p` is finite and positive.
    pub fn validate(&self) -> Result<()> {
        let ok = |x: f64| x.is_finite() && x >= 0.0;
        if ok(self.r) && ok(self.m) && ok(self.k) && self.p.is_finite() && self.p > 0.0 {
            return Ok(());
        }
        Err(Error::InvalidValue {
            path: "msd_wall".into(),
            reason: format!(
                "a mass-spring-damper wall needs finite r, m, k >= 0 and a positive pressure, got r = {}, m = {}, k = {}, p = {}",
                self.r, self.m, self.k, self.p
            ),
        })
    }

    /// The largest step for which the explicit integration of the wall equation is stable (ADR 0243 DR5), for the
    /// impedance `z0 = rho a` of the gas at the face: `0.8 min(2m/(r + z0), (r + z0)/k)`, each term only when `m > 0` or
    /// `k > 0`; infinite for `m = 0`, `k = 0`. With this step both eigenvalues of the wall equation (with the radiation
    /// damping `z0`) lie in the disc `|1 + dt lambda| <= 1`, which is inside the stability region of the solver's three
    /// Runge-Kutta schemes.
    pub fn dt_limit(&self, z0: f64) -> f64 {
        let d = self.r + z0;
        let mut lim = f64::INFINITY;
        if self.m > 0.0 {
            lim = lim.min(2.0 * self.m / d);
        }
        if self.k > 0.0 {
            lim = lim.min(d / self.k);
        }
        MSD_DT_SAFETY * lim
    }

    /// The face state and the rates of the wall state `(xi, v)` for the interior cell state `interior` and the outward
    /// unit normal `n` (ADR 0243 DR2, DR3). `m > 0`: the face is [`piston_face`] at `v` and
    /// `dv/dt = (p_face - p - r v - k xi)/m`, `d xi/dt = v`. `m = 0`: the face is [`impedance_wall`] with the reference
    /// `p + k xi` and `d xi/dt` is its normal velocity; `v` is not used. `None` when `p + k xi` is not positive (`m = 0`).
    pub fn rates(&self, gas: Gas, interior: &Prim, n: Vec3, xi: f64, v: f64) -> Option<MsdRates> {
        if self.m > 0.0 {
            let face = piston_face(gas, interior, n, v);
            return Some(MsdRates {
                face,
                xi_dot: v,
                v_dot: (face.p - self.p - self.r * v - self.k * xi) / self.m,
            });
        }
        let p_ref = self.p + self.k * xi;
        if p_ref.is_nan() || p_ref <= 0.0 {
            return None;
        }
        let face = impedance_wall(gas, p_ref, self.r, interior, n);
        Some(MsdRates {
            face,
            xi_dot: vec3::dot(face.u, n),
            v_dot: 0.0,
        })
    }
}

/// Riemann-invariant far-field face state for the free stream `fs`.
pub fn far_field(gas: Gas, fs: &Prim, interior: &Prim, n: Vec3) -> Prim {
    let g = gas.gamma;
    let (a_i, a_inf) = (interior.sound_speed(gas), fs.sound_speed(gas));
    let (un_i, un_inf) = (vec3::dot(interior.u, n), vec3::dot(fs.u, n));
    if un_i.abs() >= a_i && un_inf.abs() >= a_inf {
        return if un_inf < 0.0 { *fs } else { *interior };
    }
    let r_out = un_i + 2.0 * a_i / (g - 1.0);
    let r_in = un_inf - 2.0 * a_inf / (g - 1.0);
    let un = 0.5 * (r_out + r_in);
    let a = 0.25 * (g - 1.0) * (r_out - r_in);
    let src = if un < 0.0 { fs } else { interior };
    let s = src.p / src.rho.powf(g);
    let rho = (a * a / (g * s)).powf(1.0 / (g - 1.0));
    let un_src = vec3::dot(src.u, n);
    Prim {
        rho,
        u: vec3::add(vec3::sub(src.u, vec3::scale(n, un_src)), vec3::scale(n, un)),
        p: rho * a * a / g,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AIR: Gas = Gas { gamma: 1.4 };

    fn prim(rho: f64, u: Vec3, p: f64) -> Prim {
        Prim { rho, u, p }
    }

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol * b.abs().max(1e-300).max(a.abs())
    }

    /// Deterministic pseudo-random numbers in [0, 1).
    fn rnd(seed: &mut u64) -> f64 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*seed >> 11) as f64) / ((1u64 << 53) as f64)
    }

    #[test]
    fn a1_total_inlet_reproduces_stagnation_state_and_invariant() {
        let (r_gas, g) = (287.0, 1.4);
        let mut seed = 7;
        for _ in 0..20 {
            let (p0, t0) = (1.0e5 * (0.8 + rnd(&mut seed)), 280.0 + 60.0 * rnd(&mut seed));
            let th = (rnd(&mut seed) - 0.5) * 1.0; // flow direction within +-0.5 rad of -n
            let n = [1.0, 0.0, 0.0];
            let dir = [-th.cos(), th.sin(), 0.0];
            let bc = TotalInlet { p0, t0, r_gas, dir };
            let a_int = (g * r_gas * t0).sqrt() * (0.85 + 0.1 * rnd(&mut seed));
            let int = prim(
                1.0 + 0.2 * rnd(&mut seed),
                [-0.3 * a_int * rnd(&mut seed), 10.0 * rnd(&mut seed), 0.0],
                0.0,
            );
            let int = prim(int.rho, int.u, int.rho * a_int * a_int / g);
            let f = total_inlet(AIR, &bc, &int, n);
            let a = f.sound_speed(AIR);
            let v = vec3::norm(f.u);
            let t = f.p / (f.rho * r_gas);
            let t0_f = t + v * v / (2.0 * g / (g - 1.0) * r_gas);
            let p0_f = f.p * (t0_f / t).powf(g / (g - 1.0));
            assert!(close(t0_f, t0, 1e-12), "T0 {t0_f} vs {t0}");
            assert!(close(p0_f, p0, 1e-12), "p0 {p0_f} vs {p0}");
            let d = vec3::scale(f.u, 1.0 / v);
            assert!(vec3::norm(vec3::sub(d, dir)) < 1e-12);
            let j_f = vec3::dot(f.u, n) + 2.0 * a / (g - 1.0);
            let j_i = vec3::dot(int.u, n) + 2.0 * int.sound_speed(AIR) / (g - 1.0);
            assert!(close(j_f, j_i, 1e-12), "J {j_f} vs {j_i}");
        }
    }

    #[test]
    fn a2_far_field_preserves_the_free_stream() {
        let n = [0.6, 0.8, 0.0];
        for mach in [0.3, 0.8, 1.6] {
            for sign in [1.0, -1.0] {
                let a = 1.0;
                let fs = prim(1.0, [sign * mach * a, 0.2 * mach * a, 0.0], a * a / 1.4);
                let f = far_field(AIR, &fs, &fs, n);
                assert!(
                    close(f.rho, fs.rho, 1e-12) && close(f.p, fs.p, 1e-12),
                    "mach {mach} {sign}: {f:?}"
                );
                assert!(vec3::norm(vec3::sub(f.u, fs.u)) < 1e-12);
            }
        }
    }

    #[test]
    fn a3_pressure_outlet_switches_and_preserves_entropy() {
        let n = [0.0, 1.0, 0.0];
        let sup = prim(1.2, [0.3, 2.0, 0.1], 0.9);
        assert_eq!(pressure_outlet(AIR, 0.4, 1.0, &sup, n), sup);
        let sub = prim(1.2, [0.3, 0.5, 0.1], 0.9);
        let f = pressure_outlet(AIR, 0.8, 1.0, &sub, n);
        assert!((f.p - 0.8).abs() < 1e-14);
        assert!((f.u[0] - 0.3).abs() < 1e-12 && (f.u[2] - 0.1).abs() < 1e-12);
        assert!(close(f.p / f.rho.powf(1.4), sub.p / sub.rho.powf(1.4), 1e-12));
    }

    #[test]
    fn a4_non_reflecting_limits() {
        let n = [1.0, 0.0, 0.0];
        let int = prim(1.1, [0.4, 0.1, 0.0], 0.8);
        assert_eq!(
            pressure_outlet(AIR, 0.7, 1.0, &int, n),
            pressure_outlet(AIR, 0.7, 1.0, &int, n)
        );
        let f0 = pressure_outlet(AIR, 0.7, 0.0, &int, n);
        assert!(close(f0.p, int.p, 1e-14) && close(f0.rho, int.rho, 1e-14));
        assert!(vec3::norm(vec3::sub(f0.u, int.u)) < 1e-14);
    }

    /// A pseudo-random subsonic interior state and a unit normal.
    fn random_state(seed: &mut u64) -> (Prim, Vec3) {
        let th = 6.0 * rnd(seed);
        let n = [th.cos(), th.sin(), 0.0];
        let rho = 0.8 + 0.6 * rnd(seed);
        let p = 0.5 + 0.6 * rnd(seed);
        let a = (1.4 * p / rho).sqrt();
        let un = a * (1.6 * rnd(seed) - 0.8);
        let ut = a * (rnd(seed) - 0.5);
        let t = [-n[1], n[0], 0.0];
        let u = vec3::add(vec3::scale(n, un), vec3::scale(t, ut));
        (prim(rho, [u[0], u[1], 0.3 * a * (rnd(seed) - 0.5)], p), n)
    }

    /// A1 of ADR 0233: a zero impedance is the pressure outlet, bit for bit.
    #[test]
    fn f243_a1_zero_impedance_is_the_pressure_outlet() {
        let mut seed = 243;
        for _ in 0..20 {
            let (int, n) = random_state(&mut seed);
            let p_ref = 0.5 + 0.6 * rnd(&mut seed);
            assert_eq!(
                impedance_wall(AIR, p_ref, 0.0, &int, n),
                pressure_outlet(AIR, p_ref, 1.0, &int, n)
            );
        }
    }

    /// A2 of ADR 0233: the impedance relation, the entropy, the outgoing invariant and the tangential velocity.
    #[test]
    fn f243_a2_impedance_relation_and_invariants() {
        let mut seed = 2430;
        let mut worst = [0.0f64; 4];
        for _ in 0..20 {
            let (int, n) = random_state(&mut seed);
            let p_ref = 0.5 + 0.6 * rnd(&mut seed);
            let a_i = int.sound_speed(AIR);
            for zr in [0.1, 1.0, 10.0] {
                let z = zr * int.rho * a_i;
                let f = impedance_wall(AIR, p_ref, z, &int, n);
                let (un_f, un_i) = (vec3::dot(f.u, n), vec3::dot(int.u, n));
                let e = [
                    (f.p - p_ref - z * un_f).abs() / int.p,
                    (f.p / f.rho.powf(1.4) / (int.p / int.rho.powf(1.4)) - 1.0).abs(),
                    ((un_f + 5.0 * f.sound_speed(AIR)) / (un_i + 5.0 * a_i) - 1.0).abs(),
                    vec3::norm(vec3::sub(
                        vec3::sub(f.u, vec3::scale(n, un_f)),
                        vec3::sub(int.u, vec3::scale(n, un_i)),
                    )) / a_i,
                ];
                for (w, e) in worst.iter_mut().zip(e) {
                    *w = w.max(e);
                }
            }
        }
        println!(
            "A2 worst: relation {:.2e}, entropy {:.2e}, invariant {:.2e}, tangential {:.2e} (1e-12)",
            worst[0], worst[1], worst[2], worst[3]
        );
        assert!(worst.iter().all(|&w| w <= 1e-12), "{worst:?}");
    }

    /// A3 of ADR 0233: the linear limit is the plane-wave reflection coefficient `(z - rho c)/(z + rho c)`.
    #[test]
    fn f243_a3_linear_reflection_coefficient() {
        let (rho0, p0) = (1.0, 1.0 / 1.4);
        let n = [1.0, 0.0, 0.0];
        let mut worst = 0.0f64;
        for zr in [1.0 / 3.0, 1.0, 3.0, 19.0] {
            // (p', u_n') in three mixes of size 1e-6; rho' = p'/c^2 (the isentrope to first order)
            for (dp, du) in [(1e-6, 1e-6), (1e-6, 0.0), (-0.4e-6, 1e-6)] {
                let int = prim(rho0 + dp, [du, 0.0, 0.0], p0 + dp);
                let f = impedance_wall(AIR, p0, zr, &int, n);
                let w_out = dp + du;
                let w_in = (f.p - p0) - f.u[0];
                let e = (w_in / w_out - (zr - 1.0) / (zr + 1.0)).abs();
                worst = worst.max(e);
            }
        }
        println!("A3 worst deviation from the closed form: {worst:.2e} (1e-5)");
        assert!(worst <= 1e-5, "{worst}");
    }

    /// A4 and A5 of ADR 0233: a very large impedance stops the normal velocity; a supersonic outflow is left alone.
    #[test]
    fn f243_a4_a5_stiff_limit_and_supersonic_outflow() {
        let n = [0.6, 0.8, 0.0];
        let int = prim(1.1, [0.3, 0.2, 0.1], 0.8);
        let f = impedance_wall(AIR, 0.75, 1e12, &int, n);
        let r = vec3::dot(f.u, n).abs() / int.sound_speed(AIR);
        println!("A4 normal Mach number at z = 1e12: {r:.2e} (1e-10)");
        assert!(r <= 1e-10, "{r}");
        let sup = prim(1.2, [1.2, 1.6, 0.1], 0.9);
        assert!(vec3::dot(sup.u, n) > sup.sound_speed(AIR));
        assert_eq!(impedance_wall(AIR, 0.4, 2.0, &sup, n), sup);
    }

    /// W1 of ADR 0243: the piston face has the wall's normal velocity and keeps the entropy, the outgoing invariant and
    /// the tangential velocity.
    #[test]
    fn f332_w1_piston_face_invariants() {
        let mut seed = 332;
        let mut worst = [0.0f64; 4];
        for _ in 0..20 {
            let (int, n) = random_state(&mut seed);
            let a_i = int.sound_speed(AIR);
            for vr in [-0.3, 0.0, 0.2] {
                let v = vr * a_i;
                let f = piston_face(AIR, &int, n, v);
                let (un_f, un_i) = (vec3::dot(f.u, n), vec3::dot(int.u, n));
                let e = [
                    (un_f - v).abs() / a_i,
                    (f.p / f.rho.powf(1.4) / (int.p / int.rho.powf(1.4)) - 1.0).abs(),
                    ((un_f + 5.0 * f.sound_speed(AIR)) / (un_i + 5.0 * a_i) - 1.0).abs(),
                    vec3::norm(vec3::sub(
                        vec3::sub(f.u, vec3::scale(n, un_f)),
                        vec3::sub(int.u, vec3::scale(n, un_i)),
                    )) / a_i,
                ];
                for (w, e) in worst.iter_mut().zip(e) {
                    *w = w.max(e);
                }
            }
        }
        println!(
            "W1 worst: normal velocity {:.2e}, entropy {:.2e}, invariant {:.2e}, tangential {:.2e} (1e-12)",
            worst[0], worst[1], worst[2], worst[3]
        );
        assert!(worst.iter().all(|&w| w <= 1e-12), "{worst:?}");
    }

    /// W2 of ADR 0243: at the normal velocity of the impedance wall the piston face is the impedance wall and a wall
    /// with that damper is in equilibrium.
    #[test]
    fn f332_w2_equilibrium_is_the_impedance_wall() {
        let mut seed = 3320;
        let (mut worst_face, mut worst_rate) = (0.0f64, 0.0f64);
        for _ in 0..20 {
            let (int, n) = random_state(&mut seed);
            let p_ref = 0.5 + 0.6 * rnd(&mut seed);
            let a_i = int.sound_speed(AIR);
            for zr in [0.1, 1.0, 10.0] {
                let z = zr * int.rho * a_i;
                let f = impedance_wall(AIR, p_ref, z, &int, n);
                let v = vec3::dot(f.u, n);
                let q = piston_face(AIR, &int, n, v);
                let d = ((q.rho - f.rho).abs() / f.rho)
                    .max((q.p - f.p).abs() / f.p)
                    .max(vec3::norm(vec3::sub(q.u, f.u)) / a_i);
                worst_face = worst_face.max(d);
                let wall = MsdWall {
                    p: p_ref,
                    r: z,
                    m: 1.0,
                    k: 0.0,
                };
                let r = wall.rates(AIR, &int, n, 0.0, v).unwrap();
                worst_rate = worst_rate.max((wall.m * r.v_dot).abs() / int.p);
            }
        }
        println!("W2 worst: face {worst_face:.2e}, rate m dv/dt / p_i {worst_rate:.2e} (1e-12)");
        assert!(worst_face <= 1e-12 && worst_rate <= 1e-12, "{worst_face} {worst_rate}");
    }

    /// The wall M of ADR 0243 D8.
    fn wall_m() -> MsdWall {
        MsdWall {
            p: 1.0 / 1.4,
            r: 0.5,
            m: 1.0 / std::f64::consts::PI,
            k: 16.0 * std::f64::consts::PI,
        }
    }

    /// W3 of ADR 0243: the linear limit of the rates is `m dv/dt = w+ - (r + rho c) v - k xi`, `d xi/dt = v`.
    #[test]
    fn f332_w3_linear_limit_of_the_rates() {
        let (rho0, p0) = (1.0, 1.0 / 1.4);
        let n = [1.0, 0.0, 0.0];
        let wall = wall_m();
        let (xi, v) = (1e-8, 3e-7);
        let mut worst = 0.0f64;
        for (dp, du) in [(1e-6, 1e-6), (1e-6, 0.0), (-0.4e-6, 1e-6)] {
            let int = prim(rho0 + dp, [du, 0.0, 0.0], p0 + dp);
            let r = wall.rates(AIR, &int, n, xi, v).unwrap();
            let lin = (dp + du) - (wall.r + 1.0) * v - wall.k * xi;
            worst = worst.max((wall.m * r.v_dot - lin).abs());
            assert_eq!(r.xi_dot, v);
        }
        println!("W3 worst deviation of m dv/dt from the linear form: {worst:.2e} (1e-10)");
        assert!(worst <= 1e-10, "{worst}");
    }

    /// W4 of ADR 0243: without mass the face satisfies the algebraic wall relation, the displacement follows the face,
    /// and without mass and spring the face is the impedance wall bit for bit.
    #[test]
    fn f332_w4_massless_wall() {
        let mut seed = 33204;
        let mut worst = 0.0f64;
        for _ in 0..20 {
            let (int, n) = random_state(&mut seed);
            let p_ref = 0.5 + 0.6 * rnd(&mut seed);
            let z = (0.2 + 3.0 * rnd(&mut seed)) * int.rho * int.sound_speed(AIR);
            let xi = 0.02 * (rnd(&mut seed) - 0.5);
            let wall = MsdWall {
                p: p_ref,
                r: z,
                m: 0.0,
                k: 2.5,
            };
            let r = wall.rates(AIR, &int, n, xi, 123.0).unwrap();
            let un = vec3::dot(r.face.u, n);
            worst = worst.max((r.face.p - p_ref - z * un - wall.k * xi).abs() / int.p);
            assert_eq!(r.xi_dot, un);
            assert_eq!(r.v_dot, 0.0);
            let plain = MsdWall { k: 0.0, ..wall };
            assert_eq!(
                plain.rates(AIR, &int, n, xi, 123.0).unwrap().face,
                impedance_wall(AIR, p_ref, z, &int, n)
            );
        }
        println!("W4 worst residual of the algebraic wall relation: {worst:.2e} (1e-12)");
        assert!(worst <= 1e-12, "{worst}");
        // a displacement that takes the reference pressure to zero or below is not a state
        let wall = MsdWall {
            p: 0.7,
            r: 1.0,
            m: 0.0,
            k: 1.0,
        };
        let (int, n) = random_state(&mut seed);
        assert!(wall.rates(AIR, &int, n, -0.7, 0.0).is_none());
    }

    /// W5 of ADR 0243: the step limit of the wall equation and the disc it rests on.
    #[test]
    fn f332_w5_step_limit() {
        let w = |r: f64, m: f64, k: f64| MsdWall { p: 1.0, r, m, k };
        let close15 = |a: f64, b: f64| (a - b).abs() <= 1e-15 * b.abs();
        assert_eq!(w(0.5, 0.0, 0.0).dt_limit(1.0), f64::INFINITY);
        assert!(close15(w(0.5, 0.3, 0.0).dt_limit(1.2), 0.8 * 2.0 * 0.3 / 1.7));
        assert!(close15(w(0.5, 0.0, 40.0).dt_limit(1.2), 0.8 * 1.7 / 40.0));
        assert!(close15(
            w(0.5, 0.3, 40.0).dt_limit(1.2),
            0.8 * (1.7f64 / 40.0).min(0.6 / 1.7)
        ));
        assert!(close15(w(0.5, 0.001, 40.0).dt_limit(1.2), 0.8 * 0.002 / 1.7));
        // |P(z)| <= 1 on the disc |z + 1| <= 1 for forward Euler, SSP-RK2 and SSP-RK3
        let mul = |a: (f64, f64), b: (f64, f64)| (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0);
        let mut worst = [0.0f64; 3];
        for i in 0..=200 {
            for j in 0..360 {
                let (rad, th) = (i as f64 / 200.0, (j as f64).to_radians());
                let z = (-1.0 + rad * th.cos(), rad * th.sin());
                let z2 = mul(z, z);
                let z3 = mul(z2, z);
                let p1 = (1.0 + z.0, z.1);
                let p2 = (p1.0 + 0.5 * z2.0, p1.1 + 0.5 * z2.1);
                let p3 = (p2.0 + z3.0 / 6.0, p2.1 + z3.1 / 6.0);
                for (w, p) in worst.iter_mut().zip([p1, p2, p3]) {
                    *w = w.max(p.0.hypot(p.1));
                }
            }
        }
        println!(
            "W5 largest |P| on the disc: Euler {:.15}, RK2 {:.15}, RK3 {:.15}",
            worst[0], worst[1], worst[2]
        );
        assert!(worst.iter().all(|&x| x <= 1.0 + 1e-12), "{worst:?}");
        // both eigenvalues of [[0, 1], [-k/m, -(r + z0)/m]] (the single one for m = 0) are inside the disc at the limit
        let mut seed = 33205;
        let mut worst_eig = 0.0f64;
        for i in 0..200 {
            let z0 = 0.5 + rnd(&mut seed);
            let r = 3.0 * rnd(&mut seed);
            let m = if i % 5 == 0 {
                0.0
            } else {
                10f64.powf(4.0 * rnd(&mut seed) - 3.0)
            };
            let k = if i % 7 == 0 {
                0.0
            } else {
                10f64.powf(6.0 * rnd(&mut seed) - 2.0)
            };
            let dt = w(r, m, k).dt_limit(z0) / 0.8;
            if !dt.is_finite() {
                continue;
            }
            let d = r + z0;
            let mods: Vec<f64> = if m == 0.0 {
                vec![(1.0 - dt * k / d).abs()]
            } else {
                let a = d / (2.0 * m);
                let disc = a * a - k / m;
                if disc >= 0.0 {
                    vec![
                        (1.0 - dt * (a + disc.sqrt())).abs(),
                        (1.0 - dt * (a - disc.sqrt())).abs(),
                    ]
                } else {
                    vec![(1.0 - dt * a).hypot(dt * (-disc).sqrt())]
                }
            };
            for x in mods {
                worst_eig = worst_eig.max(x);
            }
        }
        println!("W5 largest |1 + dt lambda| at the unscaled limit: {worst_eig:.15}");
        assert!(worst_eig <= 1.0 + 1e-12, "{worst_eig}");
    }
}
