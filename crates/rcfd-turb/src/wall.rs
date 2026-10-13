//! Wall distance and standard wall functions (F-040).
//!
//! Wall distance is the exact Euclidean distance from a cell centre to the
//! nearest wall face (every face is split into a triangle fan). The wall
//! functions are the equilibrium log-law functions of Launder and Spalding,
//! "The numerical computation of turbulent flows", Comput. Methods Appl. Mech.
//! Eng. 3 (1974) 269-289: `u+ = ln(E y+) / kappa` above the intersection
//! `y+ = y+_lam` with the viscous sublayer law `u+ = y+`, expressed as an
//! eddy viscosity on the wall faces so that the wall shear stress is
//! `(nu + nu_t,w) u_P / y_P`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;

/// Constants of the log law and of the k-epsilon family.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallConstants {
    /// von Karman constant.
    pub kappa: f64,
    /// Log-law roughness parameter `E` (smooth wall).
    pub e: f64,
    /// Eddy-viscosity constant `C_mu`.
    pub cmu: f64,
}

impl Default for WallConstants {
    fn default() -> Self {
        // Launder and Spalding (1974): kappa = 0.4187 and E = 9.793 are often rounded to 0.41 / 9.8.
        WallConstants {
            kappa: 0.41,
            e: 9.8,
            cmu: 0.09,
        }
    }
}

impl WallConstants {
    /// Additive constant `B = ln(E) / kappa` of the log law `u+ = ln(y+) / kappa + B`.
    pub fn b(&self) -> f64 {
        self.e.ln() / self.kappa
    }

    /// `y+` at which the log law and the viscous sublayer law `u+ = y+` intersect.
    pub fn y_plus_lam(&self) -> f64 {
        let mut y = 11.0;
        for _ in 0..50 {
            y = (self.e * y).ln() / self.kappa;
        }
        y
    }

    /// Log-law velocity `u+` for a given `y+` (viscous sublayer below the intersection).
    pub fn u_plus(&self, y_plus: f64) -> f64 {
        if y_plus < self.y_plus_lam() {
            y_plus
        } else {
            (self.e * y_plus).ln() / self.kappa
        }
    }

    /// Friction velocity from the velocity `u_p` at distance `y` from the wall, by solving
    /// `u_p / u_tau = u+(y u_tau / nu)` with Newton's method.
    pub fn u_tau_from_velocity(&self, nu: f64, u_p: f64, y: f64) -> f64 {
        let u = u_p.abs();
        if u == 0.0 {
            return 0.0;
        }
        let lam = (nu * u / y).sqrt();
        if lam * y / nu < self.y_plus_lam() {
            return lam;
        }
        // f(ut) = ut ln(E y ut / nu) / kappa - u = 0
        let mut ut = lam;
        for _ in 0..60 {
            let yp = y * ut / nu;
            let f = ut * (self.e * yp).ln() / self.kappa - u;
            let df = ((self.e * yp).ln() + 1.0) / self.kappa;
            let step = f / df;
            ut = (ut - step).max(1e-12);
            if step.abs() < 1e-14 * ut {
                break;
            }
        }
        ut
    }

    /// Wall eddy viscosity from the first-cell velocity (equilibrium log law):
    /// `nu_t,w = u_tau^2 y / u_p - nu`, zero in the viscous sublayer.
    pub fn nut_wall_from_velocity(&self, nu: f64, u_p: f64, y: f64) -> f64 {
        let u = u_p.abs();
        if u == 0.0 {
            return 0.0;
        }
        let ut = self.u_tau_from_velocity(nu, u, y);
        (ut * ut * y / u - nu).max(0.0)
    }

    /// Wall eddy viscosity from the turbulent kinetic energy of the first cell (k-based wall
    /// function): `y* = C_mu^(1/4) sqrt(k) y / nu`, `nu_t,w = nu (y* kappa / ln(E y*) - 1)` for
    /// `y*` above the intersection and zero below it.
    pub fn nut_wall_from_k(&self, nu: f64, k: f64, y: f64) -> f64 {
        let ystar = self.cmu.powf(0.25) * k.max(0.0).sqrt() * y / nu;
        if ystar < self.y_plus_lam() {
            0.0
        } else {
            nu * (ystar * self.kappa / (self.e * ystar).ln() - 1.0)
        }
    }
}

/// Distance from `p` to the triangle `(a, b, c)`.
fn point_triangle_distance(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> f64 {
    // Ericson, Real-Time Collision Detection, section 5.1.5 (closest point on a triangle).
    let ab = vec3::sub(b, a);
    let ac = vec3::sub(c, a);
    let ap = vec3::sub(p, a);
    let d1 = vec3::dot(ab, ap);
    let d2 = vec3::dot(ac, ap);
    let closest = if d1 <= 0.0 && d2 <= 0.0 {
        a
    } else {
        let bp = vec3::sub(p, b);
        let d3 = vec3::dot(ab, bp);
        let d4 = vec3::dot(ac, bp);
        if d3 >= 0.0 && d4 <= d3 {
            b
        } else {
            let vc = d1 * d4 - d3 * d2;
            if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
                vec3::add(a, vec3::scale(ab, d1 / (d1 - d3)))
            } else {
                let cp = vec3::sub(p, c);
                let d5 = vec3::dot(ab, cp);
                let d6 = vec3::dot(ac, cp);
                if d6 >= 0.0 && d5 <= d6 {
                    c
                } else {
                    let vb = d5 * d2 - d1 * d6;
                    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
                        vec3::add(a, vec3::scale(ac, d2 / (d2 - d6)))
                    } else {
                        let va = d3 * d6 - d5 * d4;
                        if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
                            let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
                            vec3::add(b, vec3::scale(vec3::sub(c, b), w))
                        } else {
                            let denom = 1.0 / (va + vb + vc);
                            let v = vb * denom;
                            let w = vc * denom;
                            vec3::add(a, vec3::add(vec3::scale(ab, v), vec3::scale(ac, w)))
                        }
                    }
                }
            }
        }
    };
    vec3::norm(vec3::sub(p, closest))
}

/// Exact distance of every cell centre to the nearest face of the named wall patches.
pub fn wall_distance(mesh: &Mesh, wall_patches: &[&str]) -> Result<Vec<f64>> {
    // collect the triangles of all wall faces (fan around the face centre)
    let pts = mesh.points();
    let mut tris: Vec<[Vec3; 3]> = Vec::new();
    for name in wall_patches {
        let patch = mesh
            .patch(name)
            .ok_or_else(|| Error::Mesh(format!("wall distance: mesh has no patch '{name}'")))?;
        for f in patch.faces() {
            let ids = mesh.face_points(f);
            let c = mesh.face_centres()[f];
            for i in 0..ids.len() {
                tris.push([pts[ids[i]], pts[ids[(i + 1) % ids.len()]], c]);
            }
        }
    }
    if tris.is_empty() {
        return Err(Error::Mesh("wall distance: no wall faces".into()));
    }
    // bounding box of every triangle for cheap rejection
    let boxes: Vec<([f64; 3], [f64; 3])> = tris
        .iter()
        .map(|t| {
            let mut lo = t[0];
            let mut hi = t[0];
            for p in t {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            (lo, hi)
        })
        .collect();
    Ok(mesh
        .cell_centres()
        .iter()
        .map(|&p| {
            let mut best = f64::INFINITY;
            for (t, (lo, hi)) in tris.iter().zip(&boxes) {
                // distance from p to the box is a lower bound of the distance to the triangle
                let mut d2 = 0.0;
                for k in 0..3 {
                    let e = (lo[k] - p[k]).max(p[k] - hi[k]).max(0.0);
                    d2 += e * e;
                }
                if d2 >= best * best {
                    continue;
                }
                best = best.min(point_triangle_distance(p, t[0], t[1], t[2]));
            }
            best
        })
        .collect())
}

/// `y+ = u_tau y / nu` of every given wall face from the wall-adjacent cell velocity (log-law `u_tau`),
/// `y` being the normal distance of the cell centre from the face.
pub fn wall_y_plus(
    geom: &rcfd_fv::FvGeometry,
    u: &rcfd_fv::VectorField,
    wc: &WallConstants,
    nu: f64,
    faces: &[usize],
) -> Vec<f64> {
    faces
        .iter()
        .map(|&f| {
            let c = geom.mesh.owner()[f];
            let y = geom.dn[f - geom.n_internal];
            let n = geom.normal(f);
            let v = u.values[c];
            let t = vec3::sub(v, vec3::scale(n, vec3::dot(v, n)));
            wc.u_tau_from_velocity(nu, vec3::norm(t), y) * y / nu
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_and_log_law_are_self_consistent() {
        let c = WallConstants::default();
        let yl = c.y_plus_lam();
        // intersection of u+ = y+ and u+ = ln(E y+)/kappa (11.2 for kappa 0.4187, E 9.793; 11.5 for 0.41, 9.8)
        assert!((yl - (c.e * yl).ln() / c.kappa).abs() < 1e-12);
        assert!((11.0..12.0).contains(&yl), "{yl}");
        assert!((c.u_plus(yl) - yl).abs() < 1e-9);
        // continuity of u+ at the intersection; log branch above
        assert!((c.u_plus(100.0) - ((c.e * 100.0).ln() / c.kappa)).abs() < 1e-12);
        assert!((c.b() - 5.5).abs() < 0.1, "B = {}", c.b());
    }

    #[test]
    fn friction_velocity_inverts_the_log_law() {
        let c = WallConstants::default();
        let nu = 1.5e-5;
        for &(ut, y) in &[(0.5, 0.01), (1.0, 0.002), (0.05, 0.0008)] {
            let yp: f64 = y * ut / nu;
            let u = ut * c.u_plus(yp);
            let got = c.u_tau_from_velocity(nu, u, y);
            assert!((got - ut).abs() < 1e-9 * ut, "{got} vs {ut} (y+ {yp})");
            // wall eddy viscosity reproduces the shear: (nu + nu_t) u / y = u_tau^2
            let nutw = c.nut_wall_from_velocity(nu, u, y);
            assert!(((nu + nutw) * u / y - ut * ut).abs() < 1e-9 * ut * ut);
        }
        // viscous sublayer: nu_t,w = 0 and tau = nu u / y
        let (nu, y, u) = (1e-3, 1e-3, 0.002);
        assert_eq!(c.nut_wall_from_velocity(nu, u, y), 0.0);
        assert_eq!(c.nut_wall_from_k(nu, 1e-12, y), 0.0);
        // k-based: y* = cmu^(1/4) sqrt(k) y / nu = 100 -> nut_w = nu (100 kappa / ln(100 E) - 1)
        let k = (100.0 * nu / (c.cmu.powf(0.25) * y)).powi(2);
        let want = nu * (100.0 * c.kappa / (100.0 * c.e).ln() - 1.0);
        assert!((c.nut_wall_from_k(nu, k, y) - want).abs() < 1e-12);
    }

    #[test]
    fn triangle_distance_covers_face_edge_and_vertex_regions() {
        let (a, b, c) = ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert!((point_triangle_distance([0.2, 0.2, 0.5], a, b, c) - 0.5).abs() < 1e-14); // face
        assert!((point_triangle_distance([0.5, -1.0, 0.0], a, b, c) - 1.0).abs() < 1e-14); // edge ab
        assert!((point_triangle_distance([-1.0, -1.0, 0.0], a, b, c) - 2f64.sqrt()).abs() < 1e-14); // vertex a
        assert!((point_triangle_distance([1.0, 1.0, 0.0], a, b, c) - (0.5f64).sqrt()).abs() < 1e-14); // edge bc
        assert!(point_triangle_distance([0.25, 0.25, 0.0], a, b, c) < 1e-14);
    }
}
