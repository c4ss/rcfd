//! Lumped zone components of steady incompressible flow (F-113, ADR 0012): zone frontal geometry, fan curves,
//! the porous-jump scaling of the Darcy-Forchheimer sink, effectiveness-based heat duty and the lumped network
//! pressure. The momentum forces reuse [`crate::sources`] (`add_constant_acceleration`, `porous_sink`).

use crate::geometry::FvGeometry;
use rcfd_core::vec3::{self, Vec3};

/// Piecewise-linear fan curve: total pressure rise `dp` [Pa] against volume flow `q` [m^3/s]. Outside the range the
/// end values are held.
#[derive(Debug, Clone, PartialEq)]
pub struct FanCurve {
    q: Vec<f64>,
    dp: Vec<f64>,
}

impl FanCurve {
    /// Build from points; `q` must be strictly increasing with at least two points of equal length, all finite.
    pub fn new(q: Vec<f64>, dp: Vec<f64>) -> Result<Self, String> {
        if q.len() != dp.len() {
            return Err(format!(
                "curve_flow has {} entries but curve_pressure has {}",
                q.len(),
                dp.len()
            ));
        }
        if q.len() < 2 {
            return Err("a fan curve needs at least two points".into());
        }
        if q.iter().chain(&dp).any(|v| !v.is_finite()) {
            return Err("fan curve values must be finite".into());
        }
        if q.windows(2).any(|w| w[1] <= w[0]) {
            return Err("curve_flow must be strictly increasing".into());
        }
        Ok(FanCurve { q, dp })
    }

    /// Pressure rise at flow `q`.
    pub fn eval(&self, q: f64) -> f64 {
        let n = self.q.len();
        if q.is_nan() || q <= self.q[0] {
            return self.dp[0];
        }
        if q >= self.q[n - 1] {
            return self.dp[n - 1];
        }
        let i = self.q.partition_point(|&x| x <= q) - 1;
        let t = (q - self.q[i]) / (self.q[i + 1] - self.q[i]);
        self.dp[i] + t * (self.dp[i + 1] - self.dp[i])
    }
}

/// Entry/exit faces and size of a flow-through zone for a flow direction.
#[derive(Debug, Clone)]
pub struct ZoneFlow {
    /// Exit faces `(face, sign)`: the flux out of the zone through the face is `sign * phi[face]`.
    pub exit: Vec<(usize, f64)>,
    /// Entry faces (same convention, `S . n < 0` on the zone boundary).
    pub entry: Vec<(usize, f64)>,
    /// Zone volume (m^3).
    pub volume: f64,
    /// Exit area `sum S . n` (m^2).
    pub area: f64,
    /// Zone length `volume / area` along the direction (m).
    pub length: f64,
}

impl ZoneFlow {
    /// Faces on the boundary of `zone` (a boolean per cell) classified by `S . dir`. Errors when the zone has no exit
    /// face for the direction.
    pub fn new(geom: &FvGeometry, zone: &[bool], dir: Vec3) -> Result<Self, String> {
        let mesh = &geom.mesh;
        let (own, sf) = (mesh.owner(), mesh.face_area_vectors());
        let dir = vec3::unit(dir);
        let (mut exit, mut entry) = (Vec::new(), Vec::new());
        let mut area = 0.0;
        for f in 0..mesh.n_faces() {
            if geom.empty[f] {
                continue;
            }
            let o_in = zone[own[f]];
            let n_in = f < geom.n_internal && zone[mesh.neighbour()[f]];
            if o_in == n_in {
                continue; // interior of the zone (both) or outside (neither; a boundary face has no neighbour)
            }
            // outward normal of the zone through this face
            let sign = if o_in { 1.0 } else { -1.0 };
            let sn = sign * vec3::dot(sf[f], dir);
            if sn > 1e-12 * vec3::norm(sf[f]) {
                exit.push((f, sign));
                area += sn;
            } else if sn < -1e-12 * vec3::norm(sf[f]) {
                entry.push((f, sign));
            }
        }
        if exit.is_empty() || area <= 0.0 {
            return Err("the zone has no face on its downstream side for this flow direction".into());
        }
        let vol = mesh.cell_volumes();
        let volume: f64 = (0..zone.len()).filter(|&c| zone[c]).map(|c| vol[c]).sum();
        Ok(ZoneFlow {
            exit,
            entry,
            volume,
            area,
            length: volume / area,
        })
    }

    /// Volume flow out of the zone through the exit faces for the face flux `phi`.
    pub fn flow(&self, phi: &[f64]) -> f64 {
        self.exit.iter().map(|&(f, s)| s * phi[f]).sum()
    }

    /// Flux-weighted upstream temperature of the entry faces that carry inflow. `t_cells` are cell values and
    /// `t_boundary` the boundary-face values (index `f - n_internal`). `None` without inflow.
    pub fn inlet_temperature(
        &self,
        geom: &FvGeometry,
        phi: &[f64],
        t_cells: &[f64],
        t_boundary: &[f64],
    ) -> Option<f64> {
        let mesh = &geom.mesh;
        let (mut num, mut den) = (0.0, 0.0);
        for &(f, s) in &self.entry {
            let inflow = -s * phi[f];
            if inflow <= 0.0 {
                continue;
            }
            let t_up = if f < geom.n_internal {
                // the non-zone cell of the face
                if s > 0.0 {
                    t_cells[mesh.neighbour()[f]]
                } else {
                    t_cells[mesh.owner()[f]]
                }
            } else {
                t_boundary[f - geom.n_internal]
            };
            num += inflow * t_up;
            den += inflow;
        }
        (den > 0.0).then(|| num / den)
    }
}

/// Pressure jumps at internal faces (fan and porous-jump planes, ADR 0012), used by [`crate::simple::SimpleSolver`].
///
/// Across face `f` (flux `phi_f` positive from owner to neighbour) the converged relation is
/// `p_N - p_P = R_f + smooth`, `R_f = rise_f - resist_f * phi_f` (kinematic pressure): `rise` is an explicit
/// pressure rise (fan), `resist` a resistance in pressure per unit flux (porous jump, may depend on the previous
/// flux). The pressure equation gets the face flux `phi = [phiHbyA + G rise - G snGrad p] / (1 + G resist)`; the cell
/// gradients are computed from the smooth field (the jump is removed from the linear face interpolation), so that
/// the cell velocities next to the plane are not polluted by the discontinuity.
#[derive(Debug, Clone)]
pub struct FaceJump {
    /// Explicit pressure rise (m^2/s^2) from owner to neighbour side, per face (zero except at jump faces).
    pub rise: Vec<f64>,
    /// Resistance (m^2/s^2 per m^3/s) per face.
    pub resist: Vec<f64>,
}

impl FaceJump {
    /// No jump anywhere.
    pub fn zeros(n_faces: usize) -> Self {
        FaceJump {
            rise: vec![0.0; n_faces],
            resist: vec![0.0; n_faces],
        }
    }

    /// Total pressure rise `R_f` of face `f` for the flux `phi`.
    pub fn total(&self, f: usize, phi: &[f64]) -> f64 {
        self.rise[f] - self.resist[f] * phi[f]
    }

    /// Total pressure rise of every face for the flux `phi`.
    pub fn totals(&self, phi: &[f64]) -> Vec<f64> {
        (0..self.rise.len()).map(|f| self.total(f, phi)).collect()
    }

    /// Additive correction of the Green-Gauss pressure gradient that removes the jump `r` (per face) from the face
    /// interpolation.
    pub fn gradient_fix(&self, geom: &FvGeometry, r: &[f64]) -> Vec<Vec3> {
        let mesh = &geom.mesh;
        let (own, nei, sf, vol) = (
            mesh.owner(),
            mesh.neighbour(),
            mesh.face_area_vectors(),
            mesh.cell_volumes(),
        );
        let mut fix = vec![[0.0; 3]; mesh.n_cells()];
        for f in 0..geom.n_internal {
            if self.rise[f] == 0.0 && self.resist[f] == 0.0 {
                continue;
            }
            let w = geom.weights[f];
            vec3::axpy(&mut fix[own[f]], -(1.0 - w) * r[f] / vol[own[f]], sf[f]);
            vec3::axpy(&mut fix[nei[f]], -w * r[f] / vol[nei[f]], sf[f]);
        }
        fix
    }
}

/// Analytic pressure drop of the porous jump (Pa).
pub fn porous_jump_dp(mu: f64, rho: f64, permeability: f64, c2: f64, thickness: f64, v: f64) -> f64 {
    let visc = if permeability > 0.0 {
        mu / permeability * thickness * v
    } else {
        0.0
    };
    visc + c2 * thickness * rho * v * v.abs() / 2.0
}

/// Heat to the fluid (W) of an effectiveness heat exchanger: `eps C_min (t_secondary - t_in)` with the primary capacity
/// rate `c_primary` and the secondary one `c_secondary` (0 = infinite).
pub fn effectiveness_duty(eps: f64, c_primary: f64, c_secondary: f64, t_secondary: f64, t_in: f64) -> f64 {
    let c_min = if c_secondary > 0.0 {
        c_primary.min(c_secondary)
    } else {
        c_primary
    };
    eps * c_min * (t_secondary - t_in)
}

/// Pressure (Pa) of a lumped series network at flow `q`: `p_ref + R1 q + R2 q |q|`.
pub fn network_pressure(p_ref: f64, r1: f64, r2: f64, q: f64) -> f64 {
    p_ref + r1 * q + r2 * q * q.abs()
}

/// Root of a monotonically decreasing `f` on `[lo, hi]` by bisection (used by tests and checks of operating points).
pub fn bisect(f: impl Fn(f64) -> f64, mut lo: f64, mut hi: f64) -> f64 {
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_curve_interpolates_and_holds_ends() {
        let c = FanCurve::new(vec![0.0, 1.0, 3.0], vec![10.0, 6.0, 0.0]).unwrap();
        assert_eq!(c.eval(-1.0), 10.0);
        assert_eq!(c.eval(0.5), 8.0);
        assert_eq!(c.eval(2.0), 3.0);
        assert_eq!(c.eval(9.0), 0.0);
        assert!(FanCurve::new(vec![0.0, 0.0], vec![1.0, 2.0]).is_err());
        assert!(FanCurve::new(vec![0.0], vec![1.0]).is_err());
        assert!(FanCurve::new(vec![0.0, 1.0], vec![1.0]).is_err());
    }

    #[test]
    fn effectiveness_uses_the_smaller_capacity_rate() {
        assert_eq!(effectiveness_duty(0.5, 40.0, 0.0, 350.0, 300.0), 1000.0);
        assert_eq!(effectiveness_duty(0.5, 40.0, 20.0, 350.0, 300.0), 500.0);
        assert_eq!(network_pressure(1.0, 2.0, 3.0, -2.0), 1.0 - 4.0 - 12.0);
    }
}
