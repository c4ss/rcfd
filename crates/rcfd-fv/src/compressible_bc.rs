//! Characteristic boundary conditions of the density-based compressible solver
//! (ADR 0006): total pressure / total temperature inlet, static-pressure
//! outlet with subsonic / supersonic switching, partially non-reflecting
//! outlet and a Riemann-invariant far field.
//!
//! Each condition maps the interior cell state and the outward unit normal to a
//! boundary *face state*, whose physical flux the solver uses directly. The
//! analysis is one-dimensional along the face normal (Thompson, J. Comput.
//! Phys. 68 (1987) 1-24; Poinsot and Lele, J. Comput. Phys. 101 (1992)
//! 104-129; Blazek, *Computational Fluid Dynamics*, ch. 8).

use crate::riemann::{Gas, Prim};
use rcfd_core::vec3::{self, Vec3};

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
}
