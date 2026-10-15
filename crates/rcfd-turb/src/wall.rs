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

/// Spalding's single-formula law of the wall (J. Appl. Mech. 28 (1961) 455-458), valid from the
/// viscous sublayer through the log layer without a switch:
/// `y+ = u+ + (1/E) [exp(kappa u+) - 1 - kappa u+ - (kappa u+)^2/2 - (kappa u+)^3/6]`, `E = exp(kappa B)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpaldingLaw {
    /// von Karman constant.
    pub kappa: f64,
    /// `E = exp(kappa B)` (B = 5.5 gives 9.5; rcfd uses the wall-function value 9.8).
    pub e: f64,
}

impl Default for SpaldingLaw {
    fn default() -> Self {
        SpaldingLaw { kappa: 0.41, e: 9.8 }
    }
}

impl SpaldingLaw {
    /// `y+` as a function of `u+`.
    pub fn y_plus(&self, u_plus: f64) -> f64 {
        let ku = self.kappa * u_plus;
        u_plus + (ku.exp() - 1.0 - ku - 0.5 * ku * ku - ku * ku * ku / 6.0) / self.e
    }

    /// `d y+ / d u+` (`d u+ / d y+` is its inverse).
    pub fn dy_du(&self, u_plus: f64) -> f64 {
        let ku = self.kappa * u_plus;
        1.0 + self.kappa / self.e * (ku.exp() - 1.0 - ku - 0.5 * ku * ku)
    }

    /// Friction velocity from the tangential speed `u` at distance `y` (bisection on `u_tau`; the
    /// law is monotone). Zero speed gives zero.
    pub fn u_tau(&self, nu: f64, u: f64, y: f64) -> f64 {
        let u = u.abs();
        if u == 0.0 {
            return 0.0;
        }
        // g(ut) = y+(u/ut) - y ut / nu is decreasing in ut
        let g = |ut: f64| self.y_plus(u / ut) - y * ut / nu;
        let mut hi = 2.0 * u.max((nu * u / y).sqrt());
        let mut lo = hi * 1e-8;
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if g(mid) > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
            if hi - lo < 1e-14 * hi {
                break;
            }
        }
        0.5 * (lo + hi)
    }
}

/// Equivalent-sand-grain roughness with a velocity-shift law (F-203, ADR 0015):
/// `u+ = ln(E y+)/kappa - Delta B(k_s+)`, `k_s+ = k_s u_tau / nu` (`C_mu^(1/4) sqrt(k)` replaces `u_tau` in the
/// k-based k-epsilon wall functions).
///
/// `Delta B` after Cebeci and Bradshaw (1977), recalled from memory (not re-fetched): zero for `k_s+ <= 2.25`,
/// `(1/kappa) ln[(k_s+ - 2.25)/87.75 + C_s k_s+] sin[0.4258 (ln k_s+ - 0.811)]` for `2.25 < k_s+ < 90` (implemented with
/// the exact constants `0.4258 -> (pi/2)/ln 40`, `0.811 -> ln 2.25`, see [`Roughness::delta_b`]), and
/// `(1/kappa) ln(1 + C_s k_s+)` in the fully rough regime. `C_s` defaults to the value that makes the fully rough
/// law reproduce Nikuradse's `u+ = ln(y/k_s)/kappa + 8.5` for the smooth law's own constants
/// ([`Roughness::nikuradse_cs`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Roughness {
    /// Equivalent sand-grain height (length units); 0 = smooth wall.
    pub ks: f64,
    /// Roughness constant `C_s`.
    pub cs: f64,
}

impl Roughness {
    /// Smooth wall.
    pub const SMOOTH: Roughness = Roughness { ks: 0.0, cs: 0.0 };

    /// `C_s = exp(-kappa (8.5 - B))`, `B = ln(E)/kappa`.
    pub fn nikuradse_cs(kappa: f64, e: f64) -> f64 {
        (-kappa * (8.5 - e.ln() / kappa)).exp()
    }

    /// Rough wall of height `ks` with the Nikuradse-consistent `C_s` of the given smooth log law.
    pub fn new(ks: f64, kappa: f64, e: f64) -> Self {
        Roughness {
            ks,
            cs: Self::nikuradse_cs(kappa, e),
        }
    }

    /// Whether the wall is rough.
    pub fn is_rough(&self) -> bool {
        self.ks > 0.0
    }

    /// Velocity shift `Delta B(k_s+)`.
    pub fn delta_b(&self, ks_plus: f64, kappa: f64) -> f64 {
        if ks_plus <= 2.25 {
            0.0
        } else if ks_plus < 90.0 {
            // The published constants 0.4258 and 0.811 are rounded forms of (pi/2)/ln(90/2.25) and ln(2.25); the exact
            // forms make Delta B continuous at both ends (the rounded ones jump by 6e-5 at 2.25 and 3e-8 at 90).
            let theta = std::f64::consts::FRAC_PI_2 * (ks_plus / 2.25).ln() / 40f64.ln();
            // limited below by zero: with the Nikuradse-consistent C_s (about 0.3) the bracket is below 1 just above
            // 2.25 and the raw formula would give a (non-physical) friction reduction of up to 1e-3 in u+
            (((ks_plus - 2.25) / 87.75 + self.cs * ks_plus).ln() / kappa * theta.sin()).max(0.0)
        } else {
            (1.0 + self.cs * ks_plus).ln() / kappa
        }
    }

    /// Effective log-law constant `E exp(-kappa Delta B)` at the given `k_s+`.
    pub fn e_eff(&self, ks_plus: f64, kappa: f64, e: f64) -> f64 {
        e * (-kappa * self.delta_b(ks_plus, kappa)).exp()
    }
}

impl WallConstants {
    /// Rough-wall version of [`WallConstants::nut_wall_from_k`]: the log-law constant is replaced by
    /// `E exp(-kappa Delta B(k_s*))`, `k_s* = C_mu^(1/4) sqrt(k) k_s/nu`. The logarithm is limited below by 1
    /// (the cell centre must lie above the roughness height; the caller warns otherwise).
    pub fn nut_wall_from_k_rough(&self, nu: f64, k: f64, y: f64, rough: &Roughness) -> f64 {
        let ustar = self.cmu.powf(0.25) * k.max(0.0).sqrt();
        let ystar = ustar * y / nu;
        if ystar < self.y_plus_lam() {
            0.0
        } else {
            let e_eff = rough.e_eff(ustar * rough.ks / nu, self.kappa, self.e);
            let ln = (e_eff * ystar).ln().max(1.0);
            (nu * (ystar * self.kappa / ln - 1.0)).max(0.0)
        }
    }

    /// Scalable wall function (Grotjans and Menter, ECCOMAS 1998, as recalled; not re-fetched): the logarithm uses
    /// `max(y*, y*_lam)`. Returns the EFFECTIVE wall-face viscosity `nu + nu_t,w` (it can be below `nu` when
    /// `y* < y*_lam`) with a floor of `0.05 nu`.
    pub fn nu_eff_wall_scalable(&self, nu: f64, k: f64, y: f64, rough: &Roughness) -> f64 {
        let ustar = self.cmu.powf(0.25) * k.max(0.0).sqrt();
        let ystar = (ustar * y / nu).max(1e-12);
        let yt = ystar.max(self.y_plus_lam());
        let e_eff = if rough.is_rough() {
            rough.e_eff(ustar * rough.ks / nu, self.kappa, self.e)
        } else {
            self.e
        };
        let ln = (e_eff * yt).ln().max(1.0);
        (nu * ystar * self.kappa / ln).max(0.05 * nu)
    }
}

impl SpaldingLaw {
    /// `y+(u+)` with an explicit log-law constant `e` (rough walls: `e` = effective constant).
    pub fn y_plus_with_e(&self, u_plus: f64, e: f64) -> f64 {
        let ku = self.kappa * u_plus;
        u_plus + (ku.exp() - 1.0 - ku - 0.5 * ku * ku - ku * ku * ku / 6.0) / e
    }

    /// `d y+/d u+` with an explicit log-law constant `e`.
    pub fn dy_du_with_e(&self, u_plus: f64, e: f64) -> f64 {
        let ku = self.kappa * u_plus;
        1.0 + self.kappa / e * (ku.exp() - 1.0 - ku - 0.5 * ku * ku)
    }

    /// Effective constant `E exp(-kappa Delta B(k_s u_tau/nu))` of a rough wall at friction velocity `ut`.
    pub fn e_rough(&self, nu: f64, ut: f64, rough: &Roughness) -> f64 {
        rough.e_eff(ut * rough.ks / nu, self.kappa, self.e)
    }

    /// Friction velocity over a rough wall (bisection; `k_s+` follows `u_tau` inside the iteration).
    pub fn u_tau_rough(&self, nu: f64, u: f64, y: f64, rough: &Roughness) -> f64 {
        let u = u.abs();
        if u == 0.0 {
            return 0.0;
        }
        let g = |ut: f64| self.y_plus_with_e(u / ut, self.e_rough(nu, ut, rough)) - y * ut / nu;
        let mut hi = 2.0 * u.max((nu * u / y).sqrt());
        let mut lo = hi * 1e-8;
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if g(mid) > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
            if hi - lo < 1e-14 * hi {
                break;
            }
        }
        0.5 * (lo + hi)
    }
}

/// Kader's (1981) smooth blend of the viscous and logarithmic laws, used by the enhanced wall treatment
/// (constants recalled from memory, not re-fetched): `u+ = exp(Gamma) y+ + exp(1/Gamma) ln(E y+)/kappa`,
/// `Gamma = -0.01 y+^4/(1 + 5 y+)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KaderLaw {
    /// von Karman constant.
    pub kappa: f64,
    /// Log-law constant `E`.
    pub e: f64,
}

impl Default for KaderLaw {
    fn default() -> Self {
        KaderLaw { kappa: 0.41, e: 9.8 }
    }
}

impl KaderLaw {
    /// `u+` as a function of `y+`.
    pub fn u_plus(&self, y_plus: f64) -> f64 {
        if y_plus <= 0.0 {
            return 0.0;
        }
        let gamma = -0.01 * y_plus.powi(4) / (1.0 + 5.0 * y_plus);
        let turb = (self.e * y_plus).ln() / self.kappa;
        gamma.exp() * y_plus + (1.0 / gamma).exp() * turb
    }

    /// `d u+/d y+` (central difference of the closed form).
    pub fn du_dy(&self, y_plus: f64) -> f64 {
        let h = 1e-5 * y_plus.max(1e-3);
        (self.u_plus(y_plus + h) - self.u_plus((y_plus - h).max(0.0))) / (y_plus + h - (y_plus - h).max(0.0))
    }

    /// Friction velocity from the tangential speed `u` at distance `y` (bisection; `u_tau u+(y u_tau/nu)` is
    /// monotone in `u_tau`).
    pub fn u_tau(&self, nu: f64, u: f64, y: f64) -> f64 {
        let u = u.abs();
        if u == 0.0 {
            return 0.0;
        }
        let g = |ut: f64| ut * self.u_plus(y * ut / nu) - u;
        let mut hi = 2.0 * u.max((nu * u / y).sqrt());
        let mut lo = hi * 1e-8;
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if g(mid) > 0.0 {
                hi = mid;
            } else {
                lo = mid;
            }
            if hi - lo < 1e-14 * hi {
                break;
            }
        }
        0.5 * (lo + hi)
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
    fn spalding_law_recovers_both_limits() {
        let sp = SpaldingLaw::default();
        // viscous limit u+ = y+ (1 percent at y+ = 1)
        assert!((sp.y_plus(1.0) - 1.0).abs() < 1e-3);
        assert!((sp.dy_du(1.0) - 1.0).abs() < 1e-3);
        // log layer: u+ = ln(E y+)/kappa within 2 percent at y+ = 100..1000
        for &yp in &[100.0, 300.0, 1000.0] {
            let (mut lo, mut hi) = (1.0, 40.0);
            for _ in 0..100 {
                let mid = 0.5 * (lo + hi);
                if sp.y_plus(mid) < yp {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let log = (sp.e * yp).ln() / sp.kappa;
            assert!((lo / log - 1.0).abs() < 0.02, "y+ {yp}: {lo} vs {log}");
        }
        // inversion for u_tau, including the viscous regime
        let nu = 1.5e-5;
        for &(ut, y) in &[(0.5, 0.01), (1.0, 0.002), (0.05, 0.0008), (0.3, 3e-5)] {
            let yp: f64 = y * ut / nu;
            let up = {
                let (mut lo, mut hi) = (0.0, 60.0);
                for _ in 0..200 {
                    let mid = 0.5 * (lo + hi);
                    if sp.y_plus(mid) < yp {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                lo
            };
            let got = sp.u_tau(nu, ut * up, y);
            assert!((got / ut - 1.0).abs() < 1e-9, "{got} vs {ut} (y+ {yp})");
        }
        assert_eq!(sp.u_tau(nu, 0.0, 0.01), 0.0);
    }

    /// ADR 0015 C4: velocity-shift law of the equivalent sand grain.
    #[test]
    fn roughness_shift_is_continuous_monotone_and_fully_rough_is_nikuradse() {
        let c = WallConstants::default();
        let r = Roughness::new(1.0, c.kappa, c.e);
        // continuity at 2.25 and 90
        assert!((r.delta_b(2.25 + 1e-12, c.kappa) - r.delta_b(2.25, c.kappa)).abs() < 1e-9);
        let (lo, hi) = (r.delta_b(90.0 - 1e-9, c.kappa), r.delta_b(90.0, c.kappa));
        assert!((lo - hi).abs() < 1e-8, "{lo} {hi}");
        // non-decreasing
        let mut prev = 0.0;
        for i in 0..4000 {
            let ks = 0.01 * 1.003f64.powi(i);
            let d = r.delta_b(ks, c.kappa);
            assert!(d >= prev - 1e-12, "k_s+ {ks}: {d} < {prev}");
            prev = d;
        }
        // fully rough: u+ = ln(E_eff y+)/kappa equals ln(y/k_s)/kappa + 8.5 within 0.05 at k_s+ = 1000
        let (ks_plus, y_over_ks) = (1000.0, 30.0);
        let u_plus = (r.e_eff(ks_plus, c.kappa, c.e) * (y_over_ks * ks_plus)).ln() / c.kappa;
        let nik = y_over_ks.ln() / c.kappa + 8.5;
        assert!((u_plus - nik).abs() < 0.05, "{u_plus} vs {nik}");
        // smooth limit: rough friction velocity equals the smooth one for k_s+ << 2.25
        let sp = SpaldingLaw::default();
        let nu = 1.5e-5;
        let r_small = Roughness::new(1e-9, c.kappa, c.e);
        let (u, y) = (10.0, 0.01);
        assert!((sp.u_tau_rough(nu, u, y, &r_small) / sp.u_tau(nu, u, y) - 1.0).abs() < 1e-9);
        // rough wall gives a larger friction velocity at the same velocity
        let r_big = Roughness::new(2e-3, c.kappa, c.e);
        assert!(sp.u_tau_rough(nu, u, y, &r_big) > 1.2 * sp.u_tau(nu, u, y));
    }

    /// ADR 0015 C4: scalable wall function and Kader blend limits.
    #[test]
    fn scalable_and_kader_limits() {
        let c = WallConstants::default();
        let nu = 1e-3;
        let y = 1e-2;
        let k_for = |yst: f64| (yst * nu / (c.cmu.powf(0.25) * y)).powi(2);
        // identical to the standard wall function where y* >= y*_lam
        for &ys in &[12.0, 30.0, 100.0, 1000.0] {
            let k = k_for(ys);
            let std_eff = nu + c.nut_wall_from_k(nu, k, y);
            let sc = c.nu_eff_wall_scalable(nu, k, y, &Roughness::SMOOTH);
            assert!((sc / std_eff - 1.0).abs() < 1e-14, "y* {ys}: {sc} {std_eff}");
        }
        // below y*_lam the standard wall function gives nu (viscous), the scalable one a smaller viscosity
        let k = k_for(3.0);
        let sc = c.nu_eff_wall_scalable(nu, k, y, &Roughness::SMOOTH);
        assert!(sc < nu && sc > 0.0, "{sc}");
        // Kader: viscous and log limits
        let kd = KaderLaw::default();
        for &yp in &[0.1, 0.5, 1.0] {
            assert!((kd.u_plus(yp) / yp - 1.0).abs() < 0.01, "y+ {yp}: {}", kd.u_plus(yp));
        }
        for &yp in &[50.0, 100.0, 1000.0] {
            let log = (kd.e * yp).ln() / kd.kappa;
            assert!((kd.u_plus(yp) / log - 1.0).abs() < 0.01, "y+ {yp}");
        }
        // inversion
        let nu = 1.5e-5;
        for &(ut, y) in &[(0.5, 0.01), (1.0, 0.002), (0.05, 0.0008), (0.3, 3e-5)] {
            let u = ut * kd.u_plus(ut * y / nu);
            let got = kd.u_tau(nu, u, y);
            assert!((got / ut - 1.0).abs() < 1e-9, "{got} vs {ut}");
        }
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
