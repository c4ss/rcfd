//! Spalart-Allmaras one-equation eddy-viscosity model, SA-noft2 (F-041).
//!
//! Sources: P. R. Spalart and S. R. Allmaras, "A one-equation turbulence model for aerodynamic flows",
//! La Recherche Aerospatiale 1 (1994) 5-21; the NASA Turbulence Modeling Resource page
//! (https://tmbwg.github.io/turbmodels/spalart.html, fetched, see docs/reference-data.md and ADR 0014) for the
//! SA-noft2 form (no `f_t2`), the `S~` limiters and the negative-`nuT` treatment of Allmaras, Johnson and Spalart
//! (2012). Incompressible, kinematic form:
//!
//! ```text
//! D nuT/Dt = cb1 S~ nuT - cw1 fw (nuT/d)^2 + 1/sigma [ div((nu + nuT) grad nuT) + cb2 |grad nuT|^2 ]
//! nut = nuT fv1,  chi = nuT/nu,  fv1 = chi^3/(chi^3 + cv1^3),  fv2 = 1 - chi/(1 + chi fv1)
//! S~ = Omega + nuT fv2/(kappa^2 d^2),  Omega = sqrt(2 W_ij W_ij)
//! fw = g [(1 + cw3^6)/(g^6 + cw3^6)]^(1/6),  g = r + cw2 (r^6 - r),  r = min(nuT/(S~ kappa^2 d^2), 10)
//! cw1 = cb1/kappa^2 + (1 + cb2)/sigma
//! ```
//!
//! Negative `nuT` (SA-neg): production `cb1 Omega nuT`, destruction `+cw1 (nuT/d)^2`, diffusion `(nu + fn nuT)`,
//! `fn = (cn1 + chi^3)/(cn1 - chi^3)`, `nut = 0`.
//!
//! Numerics: the production and the cb2 term are explicit, the destruction is linearised on the diagonal, convection and
//! diffusion go through the shared finite-volume operators. Wall treatment: see [`SaWallMode`].

use crate::eddy::{effective_viscosity, patch_faces};
use crate::kepsilon::velocity_gradient;
use crate::wall::{wall_distance, SpaldingLaw};
use rcfd_core::vec3;
use rcfd_core::Result;
use rcfd_fv::convection::{convection, ConvScheme};
use rcfd_fv::diffusion::laplacian;
use rcfd_fv::gradient::{green_gauss, least_squares};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use std::sync::Arc;

/// Model coefficients (defaults: the TMR values).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SaCoefficients {
    /// `sigma`.
    pub sigma: f64,
    /// `cb1`.
    pub cb1: f64,
    /// `cb2`.
    pub cb2: f64,
    /// von Karman constant used inside the model.
    pub kappa: f64,
    /// `cw2`.
    pub cw2: f64,
    /// `cw3`.
    pub cw3: f64,
    /// `cv1`.
    pub cv1: f64,
    /// `cn1` of the negative-`nuT` branch.
    pub cn1: f64,
    /// `c2` of the Allmaras-Johnson-Spalart `S~` limiter.
    pub c2: f64,
    /// `c3` of the Allmaras-Johnson-Spalart `S~` limiter.
    pub c3: f64,
}

impl Default for SaCoefficients {
    fn default() -> Self {
        SaCoefficients {
            sigma: 2.0 / 3.0,
            cb1: 0.1355,
            cb2: 0.622,
            kappa: 0.41,
            cw2: 0.3,
            cw3: 2.0,
            cv1: 7.1,
            cn1: 16.0,
            c2: 0.7,
            c3: 0.9,
        }
    }
}

impl SaCoefficients {
    /// `cw1 = cb1/kappa^2 + (1 + cb2)/sigma`.
    pub fn cw1(&self) -> f64 {
        self.cb1 / (self.kappa * self.kappa) + (1.0 + self.cb2) / self.sigma
    }
}

/// Limiter of `S~` (TMR options a, b, c).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SLimiter {
    /// Allmaras, Johnson and Spalart (2012), `c2 = 0.7`, `c3 = 0.9` (default).
    Allmaras,
    /// `S~ >= 0.3 Omega` (Spalart's recommendation).
    Min03,
    /// `S~ > 0` only.
    Positive,
}

/// Wall treatment of the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaWallMode {
    /// `nuT = 0` on the wall face and no wall function: needs a wall-resolved mesh (y+ of order 1).
    Resolved,
    /// Spalding law of the wall for the wall shear and strain; for a wall cell with y+ > 5 the cell `nuT` is fixed to the
    /// value whose `nut` equals the Spalding eddy viscosity `nu (dy+/du+ - 1)`. Works from y+ about 1 into the log layer.
    Auto,
}

/// `fv1`.
pub fn fv1(chi: f64, cv1: f64) -> f64 {
    let c3 = chi * chi * chi;
    c3 / (c3 + cv1 * cv1 * cv1)
}

/// `fv2`.
pub fn fv2(chi: f64, cv1: f64) -> f64 {
    1.0 - chi / (1.0 + chi * fv1(chi, cv1))
}

/// `fw` from `r`.
pub fn fw(r: f64, cw2: f64, cw3: f64) -> f64 {
    let g = r + cw2 * (r.powi(6) - r);
    let c6 = cw3.powi(6);
    g * ((1.0 + c6) / (g.powi(6) + c6)).powf(1.0 / 6.0)
}

/// Eddy viscosity `nut = nuT fv1(nuT/nu)` (zero for `nuT <= 0`).
pub fn nut_from_nutilde(nutilde: f64, nu: f64, cv1: f64) -> f64 {
    if nutilde <= 0.0 {
        0.0
    } else {
        nutilde * fv1(nutilde / nu, cv1)
    }
}

/// Inverse of [`nut_from_nutilde`]: the `nuT >= 0` with `nuT fv1 = nut`.
pub fn nutilde_from_nut(nut: f64, nu: f64, cv1: f64) -> f64 {
    if nut <= 0.0 {
        return 0.0;
    }
    // f(chi) = chi fv1(chi) increases from 0 and f(chi) <= chi, so chi >= nut/nu
    let target = nut / nu;
    let f = |chi: f64| chi * fv1(chi, cv1);
    let (mut lo, mut hi) = (target, target + 2.0 * cv1 + 1.0);
    while f(hi) < target {
        hi *= 2.0;
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi) * nu
}

/// `S~` for the vorticity `omega`, the term `sbar = nuT fv2/(kappa^2 d^2)` and a limiter.
pub fn s_tilde(omega: f64, sbar: f64, limiter: SLimiter, c2: f64, c3: f64) -> f64 {
    match limiter {
        SLimiter::Allmaras => {
            if sbar >= -c2 * omega {
                omega + sbar
            } else {
                omega + omega * (c2 * c2 * omega + c3 * sbar) / ((c3 - 2.0 * c2) * omega - sbar)
            }
        }
        SLimiter::Min03 => (omega + sbar).max(0.3 * omega),
        SLimiter::Positive => (omega + sbar).max(0.0),
    }
}

/// `r = min(nuT/(S~ kappa^2 d^2), 10)`, 10 when `S~ = 0`.
pub fn r_function(nutilde: f64, s_t: f64, kappa: f64, d: f64) -> f64 {
    if s_t <= 0.0 {
        10.0
    } else {
        (nutilde / (s_t * kappa * kappa * d * d)).min(10.0)
    }
}

/// Steady Spalart-Allmaras model state.
pub struct SpalartAllmaras {
    /// Geometry.
    pub geom: Arc<FvGeometry>,
    /// The working variable `nuT` (m^2/s).
    pub nutilde: ScalarField,
    /// Laminar kinematic viscosity.
    pub nu: f64,
    /// Eddy viscosity of every cell (under-relaxed).
    pub nut: Vec<f64>,
    /// Wall eddy viscosity of the wall faces `(global face, nu_t,w)`.
    pub wall_nut: Vec<(usize, f64)>,
    /// Coefficients.
    pub coef: SaCoefficients,
    /// `S~` limiter.
    pub limiter: SLimiter,
    /// Negative-`nuT` treatment (SA-neg); off: `nuT` is clipped at zero.
    pub negative_nutilde: bool,
    /// Wall treatment.
    pub wall_mode: SaWallMode,
    /// Law of the wall (wall mode `Auto`, and the reported y+).
    pub law: SpaldingLaw,
    /// Wall-cell y+ above which the cell `nuT` is fixed in wall mode `Auto`.
    pub fix_y_plus: f64,
    /// Convection scheme.
    pub convection: ConvScheme,
    /// Implicit under-relaxation of the `nuT` equation.
    pub relax_turb: f64,
    /// Explicit under-relaxation of the eddy viscosity.
    pub relax_nut: f64,
    /// Linear solver controls.
    pub solver: SolveControl,
    wall_dist: Vec<f64>,
    wall_faces: Vec<usize>,
    cell_wall_face: Vec<usize>,
    y_plus: Vec<f64>,
    started: bool,
}

impl SpalartAllmaras {
    /// New model with uniform `nuT = nutilde0`; `walls` are the wall patch names.
    pub fn new(geom: Arc<FvGeometry>, nu: f64, walls: &[&str], nutilde0: f64) -> Result<Self> {
        let wall_dist = wall_distance(&geom.mesh, walls)?;
        let wall_faces = patch_faces(&geom, walls);
        let own = geom.mesh.owner();
        let mut cell_wall_face = vec![usize::MAX; geom.n_cells()];
        for &f in &wall_faces {
            let c = own[f];
            if cell_wall_face[c] == usize::MAX
                || geom.dn[f - geom.n_internal] < geom.dn[cell_wall_face[c] - geom.n_internal]
            {
                cell_wall_face[c] = f;
            }
        }
        let n = geom.n_cells();
        let mut nutilde = ScalarField::uniform("nuTilda", &geom, nutilde0);
        for name in walls {
            if geom.mesh.patch(name).is_some() {
                nutilde.set_bc(&geom, name, Box::new(rcfd_fv::bc::FixedValue::scalar(0.0)))?;
            }
        }
        let coef = SaCoefficients::default();
        let nut0 = nut_from_nutilde(nutilde0, nu, coef.cv1);
        Ok(SpalartAllmaras {
            nutilde,
            nu,
            nut: vec![nut0; n],
            wall_nut: Vec::new(),
            coef,
            limiter: SLimiter::Allmaras,
            negative_nutilde: true,
            wall_mode: SaWallMode::Auto,
            law: SpaldingLaw::default(),
            fix_y_plus: 5.0,
            convection: ConvScheme::Upwind,
            relax_turb: 0.7,
            relax_nut: 0.5,
            solver: SolveControl {
                tol_abs: 1e-14,
                tol_rel: 1e-6,
                max_iter: 500,
            },
            wall_dist,
            y_plus: vec![0.0; wall_faces.len()],
            wall_faces,
            cell_wall_face,
            geom,
            started: false,
        })
    }

    /// Wall distance of every cell centre.
    pub fn wall_distance(&self) -> &[f64] {
        &self.wall_dist
    }

    /// Wall faces (global indices) of the wall patches.
    pub fn wall_faces(&self) -> &[usize] {
        &self.wall_faces
    }

    /// `y+ = u_tau y/nu` (Spalding `u_tau`) of every wall face from the last sweep.
    pub fn y_plus(&self) -> &[f64] {
        &self.y_plus
    }

    /// Effective face viscosity `nu + nu_t` for the momentum equation.
    pub fn nu_face(&self) -> Vec<f64> {
        effective_viscosity(&self.geom, self.nu, &self.nut, &self.wall_nut)
    }

    /// Start from an eddy-viscosity field (e.g. the one of another model): `nuT` by inverting `nut = nuT fv1`.
    pub fn initialize_from_nut(&mut self, nut: &[f64]) {
        for (c, &v) in nut.iter().enumerate() {
            self.nutilde.values[c] = nutilde_from_nut(v, self.nu, self.coef.cv1);
        }
        self.nut = nut.to_vec();
        self.started = false;
    }

    fn tangential_speed(&self, u: &VectorField, f: usize) -> f64 {
        let c = self.geom.mesh.owner()[f];
        let n = self.geom.normal(f);
        let v = u.values[c];
        vec3::norm(vec3::sub(v, vec3::scale(n, vec3::dot(v, n))))
    }

    /// One sweep: solve the `nuT` equation on the flux `phi` for the current velocity, then update `nut` and `wall_nut`.
    /// Returns the largest relative change of `nut`.
    pub fn update(&mut self, u: &VectorField, phi: &[f64]) -> f64 {
        let g = self.geom.clone();
        let n = g.n_cells();
        let (nu, cf) = (self.nu, self.coef);
        let cw1 = cf.cw1();
        let vol = g.mesh.cell_volumes().to_vec();
        let gr = velocity_gradient(&g, u);
        let mut vort: Vec<f64> = (0..n)
            .map(|c| {
                let mut w2 = 0.0;
                for i in 0..3 {
                    for j in 0..3 {
                        let wij = 0.5 * (gr[i][c][j] - gr[j][c][i]);
                        w2 += wij * wij;
                    }
                }
                (2.0 * w2).sqrt()
            })
            .collect();
        let nt = self.nutilde.values.clone();

        // ---- wall cells (wall mode Auto): Spalding law, strain and fixed nuT
        let mut fixed: Vec<Option<f64>> = vec![None; n];
        let wall_cells: Vec<usize> = (0..n).filter(|&c| self.cell_wall_face[c] != usize::MAX).collect();
        if self.wall_mode == SaWallMode::Auto {
            for &c in &wall_cells {
                let f = self.cell_wall_face[c];
                let y = g.dn[f - g.n_internal];
                let uu = self.tangential_speed(u, f);
                let ut = self.law.u_tau(nu, uu, y);
                if ut <= 0.0 {
                    continue;
                }
                let dy_du = self.law.dy_du(uu / ut);
                if ut * y / nu > self.fix_y_plus {
                    vort[c] = ut * ut / nu / dy_du;
                    fixed[c] = Some(nutilde_from_nut(nu * (dy_du - 1.0).max(0.0), nu, cf.cv1));
                }
            }
        }

        // ---- gradient of nuT, diffusivity
        let nb = self.nutilde.boundary_values(&g);
        let gnt = green_gauss(&g, &nt, &nb, 0);
        let fnv = |x: f64| -> f64 {
            if x < 0.0 && self.negative_nutilde {
                let chi3 = (x / nu).powi(3);
                (cf.cn1 + chi3) / (cf.cn1 - chi3)
            } else {
                1.0
            }
        };
        let eff: Vec<f64> = nt
            .iter()
            .map(|&x| if self.negative_nutilde { fnv(x) * x } else { x.max(0.0) })
            .collect();
        let mut gam = effective_viscosity(&g, nu, &eff, &[]);
        for (i, &b) in nb.iter().enumerate() {
            let bb = if self.negative_nutilde { b } else { b.max(0.0) };
            gam[g.n_internal + i] = nu + fnv(b) * bb;
        }
        gam.iter_mut().for_each(|x| *x /= cf.sigma);

        // ---- matrix
        let cells = self.nutilde.values.clone();
        let bc = self.nutilde.boundary_coeffs(&g);
        let grad = if self.convection.needs_gradient() {
            Some(least_squares(&g, &cells, &nb))
        } else {
            None
        };
        let mut m = convection(&g, phi, &cells, &bc, self.convection, grad.as_deref());
        m.add(&laplacian(&g, &gam, &bc, grad.as_deref()));
        for c in 0..n {
            let d = self.wall_dist[c].max(1e-30);
            let x = nt[c];
            let g2 = vec3::dot(gnt[c], gnt[c]);
            m.b[c] += cf.cb2 / cf.sigma * g2 * vol[c];
            if x >= 0.0 || !self.negative_nutilde {
                let x = x.max(0.0);
                let chi = x / nu;
                let sb = x * fv2(chi, cf.cv1) / (cf.kappa * cf.kappa * d * d);
                let st = s_tilde(vort[c], sb, self.limiter, cf.c2, cf.c3);
                let r = r_function(x, st, cf.kappa, d);
                let fwv = fw(r, cf.cw2, cf.cw3);
                m.b[c] += cf.cb1 * st.max(0.0) * x * vol[c];
                m.a.diag[c] += cw1 * fwv * x / (d * d) * vol[c];
            } else {
                m.b[c] += (cf.cb1 * vort[c] * x + cw1 * x * x / (d * d)) * vol[c];
            }
        }
        m.relax(self.relax_turb, &nt);
        for &c in &wall_cells {
            if let Some(v) = fixed[c] {
                let big = 1e8 * m.a.diag[c].abs().max(1e-30);
                m.a.diag[c] = big;
                m.b[c] = big * v;
            }
        }
        let mut new = nt.clone();
        m.solve(&mut new, &self.solver);
        if !self.negative_nutilde {
            new.iter_mut().for_each(|x| *x = x.max(0.0));
        }
        self.nutilde.values = new;

        // ---- eddy viscosity and wall values
        let cap = 1e5 * nu;
        let mut change: f64 = 0.0;
        let alpha = if self.started { self.relax_nut } else { 1.0 };
        for c in 0..n {
            let target = nut_from_nutilde(self.nutilde.values[c], nu, cf.cv1).min(cap);
            let upd = alpha * target + (1.0 - alpha) * self.nut[c];
            change = change.max((upd - self.nut[c]).abs() / (self.nut[c] + nu));
            self.nut[c] = upd;
        }
        self.started = true;
        let old = std::mem::take(&mut self.wall_nut);
        let law = self.law;
        let mode = self.wall_mode;
        let mut yp = vec![0.0; self.wall_faces.len()];
        self.wall_nut = self
            .wall_faces
            .iter()
            .enumerate()
            .map(|(i, &f)| {
                let y = g.dn[f - g.n_internal];
                let uu = self.tangential_speed(u, f);
                let ut = if uu > 0.0 { law.u_tau(nu, uu, y) } else { 0.0 };
                yp[i] = ut * y / nu;
                let new = match mode {
                    SaWallMode::Resolved => 0.0,
                    SaWallMode::Auto if uu > 0.0 => (ut * ut * y / uu - nu).max(0.0),
                    SaWallMode::Auto => 0.0,
                };
                let v = match old.get(i) {
                    Some(&(_, o)) => alpha * new + (1.0 - alpha) * o,
                    None => new,
                };
                (f, v)
            })
            .collect();
        self.y_plus = yp;
        change
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_and_functions() {
        let c = SaCoefficients::default();
        assert!((c.cw1() - 3.2390678).abs() < 1e-6, "{}", c.cw1());
        assert!((fv1(c.cv1, c.cv1) - 0.5).abs() < 1e-14);
        assert!((fv2(0.0, c.cv1) - 1.0).abs() < 1e-15);
        assert!((fw(1.0, c.cw2, c.cw3) - 1.0).abs() < 1e-12);
        // r is capped at 10, and 10 when S~ vanishes
        assert_eq!(r_function(1.0, 0.0, 0.41, 1.0), 10.0);
        assert_eq!(r_function(1e9, 1.0, 0.41, 1.0), 10.0);
    }

    #[test]
    fn allmaras_limiter_is_continuous_and_positive() {
        let (c2, c3) = (0.7, 0.9);
        let om = 3.0;
        let at = |sb: f64| s_tilde(om, sb, SLimiter::Allmaras, c2, c3);
        let b = -c2 * om;
        assert!((at(b) - at(b - 1e-13)).abs() < 1e-12);
        assert!((at(b) - 0.3 * om).abs() < 1e-12);
        for e in [0.0, -0.5, -1.0, -10.0, -1e3, -1e6] {
            assert!(at(e * om) > 0.0, "S~ {} at {e}", at(e * om));
        }
        assert!((at(-1e12 * om) / om - 0.1).abs() < 1e-6);
        assert!(s_tilde(om, -100.0, SLimiter::Min03, c2, c3) >= 0.3 * om - 1e-15);
    }

    #[test]
    fn negative_branch_and_inversion() {
        assert!(nut_from_nutilde(-1e-6, 1e-5, 7.1) == 0.0);
        for ratio in [1e-3, 0.1, 1.0, 10.0, 100.0, 1e4] {
            let nu = 2e-7;
            let nt = nutilde_from_nut(ratio * nu, nu, 7.1);
            let back = nut_from_nutilde(nt, nu, 7.1);
            assert!((back / (ratio * nu) - 1.0).abs() < 1e-10, "ratio {ratio}: {back}");
        }
        // fn(chi = 0) = 1
        let c = SaCoefficients::default();
        assert!(((c.cn1 + 0.0) / (c.cn1 - 0.0) - 1.0).abs() < 1e-15);
    }
}
