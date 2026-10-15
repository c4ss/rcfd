//! Thermal wall conditions for the temperature equation (F-051, ADR 0024 D3): convection to a free-stream
//! temperature, a thin wall of series layers, and radiation to an ambient temperature.
//!
//! The heat flux out of the domain through a boundary face follows a series chain from the owner cell centre: the
//! half cell `dn/k`, the wall resistance `R_w = sum t_i/k_i` (steady resistance, no thermal capacity), then the external
//! surface where `q_ext(T_s) = h (T_s - T_inf) + eps sigma (T_s^4 - T_a^4)`. With `T_s = T_P - q (dn/k + R_w)` the scalar
//! equation `q = q_ext(T_s)` is monotone and is solved by Newton; the coefficients handed to the operators are its exact
//! linearisation about that solution, so the fixed point of the outer iteration solves the discrete nonlinear boundary
//! equation exactly.

use crate::bc::{Bc, BcCtx, FaceCoeffs};

/// Stefan-Boltzmann constant `2 pi^5 k_B^4 / (15 h^3 c^2)` from the exact SI values of `k_B`, `h` and `c` (W/m^2/K^4).
pub const STEFAN_BOLTZMANN: f64 = {
    use std::f64::consts::PI;
    const KB: f64 = 1.380649e-23;
    const H: f64 = 6.62607015e-34;
    const C: f64 = 299_792_458.0;
    2.0 * PI * PI * PI * PI * PI * KB * KB * KB * KB / (15.0 * H * H * H * C * C)
};

/// Convection / thin wall / radiation-to-ambient condition for the temperature.
#[derive(Debug, Clone)]
pub struct ConvectiveWall {
    /// Conductivity of the owner cell at every face of the patch (W/m/K).
    pub k: Vec<f64>,
    /// Heat-transfer coefficient (W/m^2/K); 0 = none.
    pub h: f64,
    /// Free-stream temperature of the convection (K); unused when `h = 0`.
    pub t_inf: f64,
    /// Series wall resistance `sum t_i/k_i` (m^2 K/W); 0 = no wall.
    pub wall_resistance: f64,
    /// Emissivity of the external surface; 0 = no radiation.
    pub emissivity: f64,
    /// Ambient (radiation) temperature (K); unused when `emissivity = 0`.
    pub t_rad: f64,
}

impl ConvectiveWall {
    /// External heat flux out of the surface and its derivative with respect to the surface temperature.
    fn external(&self, ts: f64) -> (f64, f64) {
        let sg = STEFAN_BOLTZMANN * self.emissivity;
        let q = self.h * (ts - self.t_inf) + sg * (ts.powi(4) - self.t_rad.powi(4));
        let dq = self.h + 4.0 * sg * ts.powi(3);
        (q, dq)
    }

    /// Heat flux out of the domain (W/m^2) and its derivative with respect to the cell temperature `t_p` for a cell centre
    /// at normal distance `dn` of a face with owner conductivity `k`.
    pub fn flux_out(&self, t_p: f64, dn: f64, k: f64) -> (f64, f64) {
        let r = dn / k + self.wall_resistance;
        let mut q = self.external(t_p).0;
        for _ in 0..100 {
            let ts = t_p - q * r;
            let (qe, dqe) = self.external(ts);
            let f = q - qe;
            let df = 1.0 + r * dqe;
            let mut step = f / df;
            // keep the surface temperature positive (the T^4 law is only meaningful there)
            let mut guard = 0;
            while ts + r * step <= 0.0 && self.emissivity > 0.0 && guard < 60 {
                step *= 0.5;
                guard += 1;
            }
            q -= step;
            if step.abs() <= 1e-15 * q.abs().max(1e-300) + 1e-300 {
                break;
            }
        }
        let ts = t_p - q * r;
        let dqe = self.external(ts).1;
        (q, dqe / (1.0 + r * dqe))
    }
}

impl Bc for ConvectiveWall {
    fn type_name(&self) -> &'static str {
        "convectiveWall"
    }

    fn coeffs(&self, ctx: &BcCtx, _comp: usize) -> FaceCoeffs {
        let k = self.k[ctx.local_face];
        let t_p = ctx.cell_value[0];
        let (q, u) = self.flux_out(t_p, ctx.dn, k);
        // q ~ q* + u (T_P - T_P*); gradient = -q/k; face value = T_P - q dn/k
        let q0 = q - u * t_p;
        FaceCoeffs {
            a: 1.0 - u * ctx.dn / k,
            b: -q0 * ctx.dn / k,
            c: -u / k,
            d: -q0 / k,
        }
    }

    fn clone_box(&self) -> Box<dyn Bc> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stefan_boltzmann_value() {
        assert!((STEFAN_BOLTZMANN - 5.670374419e-8).abs() < 1e-16);
    }

    #[test]
    fn linear_case_matches_series_resistance() {
        let w = ConvectiveWall {
            k: vec![2.0],
            h: 50.0,
            t_inf: 400.0,
            wall_resistance: 0.01,
            emissivity: 0.0,
            t_rad: 0.0,
        };
        let (q, u) = w.flux_out(300.0, 0.01, 2.0);
        let r = 0.01 / 2.0 + 0.01 + 1.0 / 50.0;
        assert!((q - (300.0 - 400.0) / r).abs() < 1e-12);
        assert!((u - 1.0 / r).abs() < 1e-12);
    }
}
