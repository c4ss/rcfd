//! Actuator-disk (virtual rotor) momentum source and momentum-theory helpers (F-218, ADR 0036).
//!
//! The disk is a cell zone. A thrust `T` along `n` and an optional torque `Q` about an axis are distributed over the zone cells with
//! the weights `w_c = V_c g(r_c)` (`g = 1`, uniform, or `g = r`, the distance from the axis), so the cell forces sum to `T n` and
//! `Q` exactly. The solver sees the kinematic acceleration `F_c / (rho V_c)`.
//!
//! Momentum theory (Rankine-Froude; incompressible, uniform, steady, no swirl, see ADR 0036 D3): `u_d = (u0 + u_w)/2`,
//! `T = rho A u_d (u_w - u0)`, `P = T u_d`; hover `u_d = sqrt(T/(2 rho A))`; Betz `C_P = 4a(1-a)^2`, maximum 16/27 at `a = 1/3`.

use crate::geometry::FvGeometry;
use crate::simple::BodyForce;
use crate::sources::face_mask;
use rcfd_core::vec3::{self, Vec3};

/// Radial distribution of the force density.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskProfile {
    /// Constant force per volume.
    Uniform,
    /// Force per volume proportional to the distance from the disk axis.
    LinearR,
}

/// Actuator-disk settings.
#[derive(Debug, Clone, Copy)]
pub struct ActuatorDisk {
    /// Total force exerted on the fluid (N, signed along `direction`).
    pub thrust: f64,
    /// Unit direction of the force on the fluid.
    pub direction: Vec3,
    /// Radial profile.
    pub profile: DiskProfile,
    /// A point on the disk axis.
    pub centre: Vec3,
    /// Unit axis (swirl and radial coordinate).
    pub axis: Vec3,
    /// Torque on the fluid about `axis` (N m, right-handed).
    pub torque: f64,
    /// Density that converts force to kinematic acceleration.
    pub density: f64,
    /// Face-consistent smoothing (default in the case layer): face values are the mean of the two cell values (zone edge faces
    /// get half), the cell values are reconstructed from the face fluxes, and thrust and torque are renormalised. A sharp
    /// top-hat of cell forces is not balanced by the discrete pressure gradient in the edge cells (a residual of half the force
    /// density, ADR 0036 addendum); smoothing spreads the force over one extra cell layer and removes it.
    pub smooth: bool,
}

impl ActuatorDisk {
    /// Radial distance of `x` from the axis.
    fn radial(&self, x: Vec3) -> (f64, Vec3) {
        let rel = vec3::sub(x, self.centre);
        let perp = vec3::sub(rel, vec3::scale(self.axis, vec3::dot(rel, self.axis)));
        (vec3::norm(perp), perp)
    }

    /// Force (N) on every cell, zero outside the zone.
    pub fn cell_forces(&self, geom: &FvGeometry, zone: &[bool]) -> Result<Vec<Vec3>, String> {
        let (cc, vol) = (geom.mesh.cell_centres(), geom.mesh.cell_volumes());
        let g = |r: f64| match self.profile {
            DiskProfile::Uniform => 1.0,
            DiskProfile::LinearR => r,
        };
        let mut sum_w = 0.0;
        let mut sum_wr = 0.0;
        for c in 0..zone.len() {
            if zone[c] {
                let (r, _) = self.radial(cc[c]);
                let w = vol[c] * g(r);
                sum_w += w;
                sum_wr += w * r;
            }
        }
        if sum_w.is_nan() || sum_w <= 0.0 {
            return Err("the disk zone has no force-carrying cells (zero total weight)".into());
        }
        if self.torque != 0.0 && (sum_wr.is_nan() || sum_wr <= 0.0) {
            return Err("a swirl torque needs cells away from the disk axis".into());
        }
        let mut out = vec![[0.0; 3]; zone.len()];
        for c in 0..zone.len() {
            if !zone[c] {
                continue;
            }
            let (r, perp) = self.radial(cc[c]);
            let w = vol[c] * g(r);
            let mut f = vec3::scale(self.direction, self.thrust * w / sum_w);
            if self.torque != 0.0 && r > 0.0 {
                let e_theta = vec3::scale(vec3::cross(self.axis, perp), 1.0 / r);
                f = vec3::add(f, vec3::scale(e_theta, self.torque * w / sum_wr));
            }
            out[c] = f;
        }
        Ok(out)
    }

    /// Cell forces of the thrust part and the swirl part separately (N).
    fn parts(&self, geom: &FvGeometry, zone: &[bool]) -> Result<(Vec<Vec3>, Vec<Vec3>), String> {
        let total = self.cell_forces(geom, zone)?;
        let mut thrust_only = *self;
        thrust_only.torque = 0.0;
        let t = thrust_only.cell_forces(geom, zone)?;
        let q: Vec<Vec3> = total.iter().zip(&t).map(|(a, b)| vec3::sub(*a, *b)).collect();
        Ok((t, q))
    }

    /// Kinematic acceleration per cell and per face (`a_f . S_f`) of a force field, sharp (zone cells only; faces whose two cells
    /// are in the zone) or smoothed (see [`ActuatorDisk::smooth`]).
    fn distribute(&self, geom: &FvGeometry, zone: &[bool], force: &[Vec3], scale: f64) -> (Vec<Vec3>, Vec<f64>) {
        let mesh = &geom.mesh;
        let (sf, vol) = (mesh.face_area_vectors(), mesh.cell_volumes());
        let acc: Vec<Vec3> = (0..zone.len())
            .map(|c| vec3::scale(force[c], 1.0 / (self.density * vol[c])))
            .collect();
        let nf = mesh.n_faces();
        if !self.smooth {
            let fz = face_mask(geom, zone);
            let face = (0..nf)
                .map(|f| {
                    if !fz[f] {
                        return 0.0;
                    }
                    let o = mesh.owner()[f];
                    let a = if f < geom.n_internal {
                        vec3::scale(vec3::add(acc[o], acc[mesh.neighbour()[f]]), 0.5)
                    } else {
                        acc[o]
                    };
                    vec3::dot(a, sf[f])
                })
                .collect();
            return (acc, face);
        }
        // smoothed: face flux = mean of the two cell accelerations (outside cells are zero), cell value reconstructed from the
        // face fluxes of the cell: a_c = (sum S S / |S|)^-1 sum (S / |S|) flux
        let face: Vec<f64> = (0..nf)
            .map(|f| {
                let o = mesh.owner()[f];
                let a = if f < geom.n_internal {
                    vec3::scale(vec3::add(acc[o], acc[mesh.neighbour()[f]]), 0.5)
                } else {
                    acc[o]
                };
                vec3::dot(a, sf[f])
            })
            .collect();
        let mut cell = vec![[0.0; 3]; zone.len()];
        for (c, out) in cell.iter_mut().enumerate() {
            let mut m = [[0.0; 3]; 3];
            let mut r = [0.0; 3];
            for &f in mesh.cell_faces(c) {
                let area = vec3::norm(sf[f]);
                if area == 0.0 {
                    continue;
                }
                for i in 0..3 {
                    for j in 0..3 {
                        m[i][j] += sf[f][i] * sf[f][j] / area;
                    }
                    r[i] += sf[f][i] / area * face[f];
                }
            }
            if let Some(inv) = invert3(&m) {
                *out = [
                    scale * (inv[0][0] * r[0] + inv[0][1] * r[1] + inv[0][2] * r[2]),
                    scale * (inv[1][0] * r[0] + inv[1][1] * r[1] + inv[1][2] * r[2]),
                    scale * (inv[2][0] * r[0] + inv[2][1] * r[1] + inv[2][2] * r[2]),
                ];
            }
        }
        let face = face.iter().map(|v| v * scale).collect();
        (cell, face)
    }

    /// Add the kinematic acceleration `F_c / (rho V_c)` to `out`. Sharp distribution: cell values in the zone and face values
    /// `a_f . S_f` on faces whose two cells are in the zone (mean of the two cell accelerations; boundary faces the owner value).
    /// Smoothed distribution: see [`ActuatorDisk::smooth`].
    pub fn add_force(&self, geom: &FvGeometry, zone: &[bool], out: &mut BodyForce) -> Result<(), String> {
        let (ft, fq) = self.parts(geom, zone)?;
        let vol = geom.mesh.cell_volumes();
        let cc = geom.mesh.cell_centres();
        let mut cell_sum = vec![[0.0; 3]; zone.len()];
        let mut face_sum = vec![0.0; geom.mesh.n_faces()];
        for (idx, part) in [ft, fq].iter().enumerate() {
            let mut scale = 1.0;
            if self.smooth {
                // renormalise so that the smoothed part carries exactly the requested thrust (idx 0) or torque (idx 1)
                let (probe, _) = self.distribute(geom, zone, part, 1.0);
                let got: f64 = if idx == 0 {
                    (0..zone.len())
                        .map(|c| self.density * vol[c] * vec3::dot(probe[c], self.direction))
                        .sum()
                } else {
                    (0..zone.len())
                        .map(|c| {
                            let fc = vec3::scale(probe[c], self.density * vol[c]);
                            vec3::dot(vec3::cross(vec3::sub(cc[c], self.centre), fc), self.axis)
                        })
                        .sum()
                };
                let want = if idx == 0 { self.thrust } else { self.torque };
                if want != 0.0 && got != 0.0 {
                    scale = want / got;
                }
            }
            let (c, f) = self.distribute(geom, zone, part, scale);
            for i in 0..zone.len() {
                cell_sum[i] = vec3::add(cell_sum[i], c[i]);
            }
            for (a, b) in face_sum.iter_mut().zip(&f) {
                *a += b;
            }
        }
        for c in 0..zone.len() {
            out.cell[c] = vec3::add(out.cell[c], cell_sum[c]);
        }
        for (a, b) in out.face.iter_mut().zip(&face_sum) {
            *a += b;
        }
        Ok(())
    }
}

fn invert3(m: &[[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let c00 = m[1][1] * m[2][2] - m[1][2] * m[2][1];
    let c01 = m[1][2] * m[2][0] - m[1][0] * m[2][2];
    let c02 = m[1][0] * m[2][1] - m[1][1] * m[2][0];
    let det = m[0][0] * c00 + m[0][1] * c01 + m[0][2] * c02;
    let scale = m[0][0].abs() + m[1][1].abs() + m[2][2].abs();
    if det.abs() <= 1e-14 * scale * scale * scale {
        return None;
    }
    let id = 1.0 / det;
    Some([
        [
            c00 * id,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * id,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * id,
        ],
        [
            c01 * id,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * id,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * id,
        ],
        [
            c02 * id,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * id,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * id,
        ],
    ])
}

/// Hover induced velocity at the disk, `sqrt(T / (2 rho A))`.
pub fn hover_induced_velocity(thrust: f64, rho: f64, area: f64) -> f64 {
    (thrust / (2.0 * rho * area)).sqrt()
}

/// Velocity at the disk in an axial stream `u0`: the root `u_d >= u0 / 2` of `T = 2 rho A u_d (u_d - u0)`.
pub fn axial_disk_velocity(u0: f64, thrust: f64, rho: f64, area: f64) -> f64 {
    0.5 * u0 + (0.25 * u0 * u0 + thrust / (2.0 * rho * area)).sqrt()
}

/// Far-wake velocity `2 u_d - u0`.
pub fn wake_velocity(u0: f64, u_disk: f64) -> f64 {
    2.0 * u_disk - u0
}

/// Power delivered to the flow, `T u_d`.
pub fn disk_power(thrust: f64, u_disk: f64) -> f64 {
    thrust * u_disk
}

/// Propulsive efficiency `T u0 / P = u0 / u_d`.
pub fn propulsive_efficiency(u0: f64, u_disk: f64) -> f64 {
    u0 / u_disk
}

/// Betz power coefficient `4 a (1-a)^2` for the axial induction factor `a`.
pub fn betz_cp(a: f64) -> f64 {
    4.0 * a * (1.0 - a) * (1.0 - a)
}

/// Thrust coefficient `4 a (1-a)`.
pub fn betz_ct(a: f64) -> f64 {
    4.0 * a * (1.0 - a)
}
