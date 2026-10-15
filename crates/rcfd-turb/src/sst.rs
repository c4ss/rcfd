//! k-omega SST eddy-viscosity model, Menter 2003 formulation (F-043).
//!
//! Source: F. R. Menter, M. Kuntz and R. Langtry, "Ten years of industrial experience with the SST
//! turbulence model", Turbulence, Heat and Mass Transfer 4 (2003) 625-632, as tabulated on the NASA
//! Turbulence Modeling Resource SST page (https://tmbwg.github.io/turbmodels/sst.html, fetched in
//! wave 7; see docs/reference-data.md). Incompressible, kinematic form (`rho = 1`):
//!
//! ```text
//! div(phi k)     - div((nu + sigma_k nu_t) grad k)     = P~ - beta* omega k
//! div(phi omega) - div((nu + sigma_w nu_t) grad omega) = gamma P~/nu_t - beta omega^2 + (1 - F1) CD
//! P = nu_t S^2,  P~ = min(P, 10 beta* k omega),  S = sqrt(2 S_ij S_ij)
//! CD = 2 sigma_w2 / omega  grad k . grad omega
//! nu_t = a1 k / max(a1 omega, S F2)
//! F1 = tanh(arg1^4), arg1 = min(max(sqrt(k)/(beta* omega d), 500 nu/(d^2 omega)),
//!                               4 sigma_w2 k/(max(CD, 1e-10) d^2))
//! F2 = tanh(arg2^2), arg2 = max(2 sqrt(k)/(beta* omega d), 500 nu/(d^2 omega))
//! phi = F1 phi_1 + (1 - F1) phi_2 for sigma_k, sigma_w, beta, gamma
//! ```
//!
//! Constants (SST-2003): `beta* = 0.09`, `a1 = 0.31`, set 1 `sigma_k = 0.85`, `sigma_w = 0.5`,
//! `beta = 0.075`, `gamma = 5/9`; set 2 `sigma_k = 1`, `sigma_w = 0.856`, `beta = 0.0828`,
//! `gamma = 0.44`.
//!
//! Wall treatment (rcfd, automatic in the sense of Menter and Esch, "Elements of industrial heat
//! transfer predictions", 16th Brazilian Congress of Mechanical Engineering, 2001): the wall shear
//! comes from Spalding's single-formula law of the wall ([`crate::wall::SpaldingLaw`]), valid from
//! `y+ ~ 1` to the log layer. In the wall-adjacent cell `omega = sqrt(omega_vis^2 + omega_log^2)`
//! with `omega_vis = 6 nu / (beta_1 y^2)` and `omega_log = u_tau / (sqrt(beta*) kappa y)`, the strain is
//! the Spalding velocity gradient, and the production is the turbulent share of the wall shear
//! stress times that gradient. `k` has zero gradient at walls.

use crate::eddy::{effective_viscosity, patch_faces};
use crate::kepsilon::velocity_gradient;
use crate::wall::{wall_distance, Roughness, SpaldingLaw};
use rcfd_core::vec3;
use rcfd_core::Result;
use rcfd_fv::convection::{convection, ConvScheme};
use rcfd_fv::diffusion::laplacian;
use rcfd_fv::gradient::{green_gauss, least_squares};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use std::sync::Arc;

/// `beta*` (and `C_mu` of the k-epsilon family).
pub const BETA_STAR: f64 = 0.09;
const A1: f64 = 0.31;
const SIGMA_K: [f64; 2] = [0.85, 1.0];
const SIGMA_W: [f64; 2] = [0.5, 0.856];
const BETA: [f64; 2] = [0.075, 0.0828];
const GAMMA: [f64; 2] = [5.0 / 9.0, 0.44];

/// Model variant (F-043, ADR 0015).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SstVariant {
    /// Menter 2003 SST (the NASA TMR form), default.
    Sst,
    /// Menter 1994 baseline model: `nu_t = k/omega` (no a1 limiter, no F2), `sigma_k1 = 0.5`, `gamma_i` from the
    /// consistency relation `beta_i/beta* - sigma_wi kappa^2/sqrt(beta*)` with `kappa = 0.41`.
    Bsl,
}

/// Wall omega / wall-shear options (F-203 wall-omega options, ADR 0015).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SstWallMode {
    /// Spalding wall law for the shear and strain, cell omega fixed for `y+ > 5` (default, see the module docs).
    Automatic,
    /// Plain low-Re boundary conditions of the TMR page: `k = 0` and `omega = 10 * 6 nu/(beta1 y1^2)` on the wall
    /// face, laminar wall shear, no wall law, no cell fixing. Needs `y+` of order 1.
    LowRe,
}

/// BSL `gamma_i = beta_i/beta* - sigma_wi kappa^2/sqrt(beta*)`.
pub fn bsl_gamma(i: usize) -> f64 {
    BETA[i] / BETA_STAR - SIGMA_W[i] * 0.41f64.powi(2) / BETA_STAR.sqrt()
}

/// Blending function `F1` from the local state (`d` wall distance, `cd` the cross-diffusion
/// `2 sigma_w2/omega grad k . grad omega`).
pub fn blend_f1(k: f64, omega: f64, d: f64, nu: f64, cd: f64) -> f64 {
    let a = (k.sqrt() / (BETA_STAR * omega * d)).max(500.0 * nu / (d * d * omega));
    let b = 4.0 * SIGMA_W[1] * k / (cd.max(1e-10) * d * d);
    a.min(b).powi(4).tanh()
}

/// Blending function `F2`.
pub fn blend_f2(k: f64, omega: f64, d: f64, nu: f64) -> f64 {
    let a = (2.0 * k.sqrt() / (BETA_STAR * omega * d)).max(500.0 * nu / (d * d * omega));
    (a * a).tanh()
}

/// Steady k-omega SST model state.
pub struct KOmegaSst {
    /// Geometry.
    pub geom: Arc<FvGeometry>,
    /// Turbulent kinetic energy (m^2/s^2).
    pub k: ScalarField,
    /// Specific dissipation rate (1/s).
    pub omega: ScalarField,
    /// Laminar kinematic viscosity.
    pub nu: f64,
    /// Eddy viscosity of every cell (under-relaxed).
    pub nut: Vec<f64>,
    /// Wall eddy viscosity of the wall faces `(global face, nu_t,w)`.
    pub wall_nut: Vec<(usize, f64)>,
    /// Law of the wall used for the wall shear.
    pub law: SpaldingLaw,
    /// Wall-cell `y+` above which omega of the wall cell is fixed to the blended viscous/log value
    /// instead of being solved with the Menter wall value on the wall face.
    pub omega_fix_y_plus: f64,
    /// Convection scheme of k and omega.
    pub convection: ConvScheme,
    /// Implicit under-relaxation of k and omega.
    pub relax_turb: f64,
    /// Explicit under-relaxation of the eddy viscosity.
    pub relax_nut: f64,
    /// Linear solver controls.
    pub solver: SolveControl,
    /// Model variant (default [`SstVariant::Sst`]).
    pub variant: SstVariant,
    /// Wall treatment mode (default [`SstWallMode::Automatic`]); change it with [`KOmegaSst::set_wall_mode`].
    pub wall_mode: SstWallMode,
    /// Equivalent-sand-grain roughness of the wall patches (default smooth; `Automatic` mode only).
    pub roughness: Roughness,
    wall_names: Vec<String>,
    wall_dist: Vec<f64>,
    wall_faces: Vec<usize>,
    cell_wall_face: Vec<usize>,
    f1: Vec<f64>,
    y_plus: Vec<f64>,
    started: bool,
}

impl KOmegaSst {
    /// New model with uniform initial `k` and `omega`; `walls` are the wall patch names.
    pub fn new(geom: Arc<FvGeometry>, nu: f64, walls: &[&str], k0: f64, omega0: f64) -> Result<Self> {
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
        let wall_faces_len = wall_faces.len();
        // Menter's wall value omega_w = 10 * 6 nu / (beta_1 y^2) on the wall faces, y being the
        // distance of the first cell centre
        let mut omega = ScalarField::uniform("omega", &geom, omega0);
        for name in walls {
            if let Some(patch) = geom.mesh.patch(name) {
                let vals: Vec<[f64; 3]> = patch
                    .faces()
                    .map(|f| {
                        let y = geom.dn[f - geom.n_internal];
                        [10.0 * 6.0 * nu / (BETA[0] * y * y), 0.0, 0.0]
                    })
                    .collect();
                omega.set_bc(
                    &geom,
                    name,
                    Box::new(rcfd_fv::bc::FixedValue(rcfd_fv::bc::Profile::PerFace(vals))),
                )?;
            }
        }
        Ok(KOmegaSst {
            k: ScalarField::uniform("k", &geom, k0),
            omega,
            nu,
            nut: vec![k0 / omega0; n],
            wall_nut: Vec::new(),
            law: SpaldingLaw::default(),
            omega_fix_y_plus: 5.0,
            convection: ConvScheme::Upwind,
            relax_turb: 0.7,
            relax_nut: 0.5,
            solver: SolveControl {
                tol_abs: 1e-14,
                tol_rel: 1e-6,
                max_iter: 500,
            },
            variant: SstVariant::Sst,
            wall_mode: SstWallMode::Automatic,
            roughness: Roughness::SMOOTH,
            wall_names: walls.iter().map(|w| w.to_string()).collect(),
            wall_dist,
            wall_faces,
            cell_wall_face,
            f1: vec![1.0; n],
            y_plus: vec![0.0; wall_faces_len],
            geom,
            started: false,
        })
    }

    /// Select the wall treatment. `LowRe` sets `k = 0` on the wall faces; `Automatic` restores zero gradient.
    pub fn set_wall_mode(&mut self, mode: SstWallMode) -> Result<()> {
        self.wall_mode = mode;
        for name in self.wall_names.clone() {
            let bc: Box<dyn rcfd_fv::bc::Bc> = match mode {
                SstWallMode::LowRe => Box::new(rcfd_fv::bc::FixedValue::scalar(0.0)),
                SstWallMode::Automatic => Box::new(rcfd_fv::ZeroGradient),
            };
            self.k.set_bc(&self.geom, &name, bc)?;
        }
        Ok(())
    }

    /// Model coefficients `(sigma_k, sigma_w, beta, gamma)` (sets 1 and 2) of the active variant.
    fn coeffs(&self) -> ([f64; 2], [f64; 2], [f64; 2], [f64; 2]) {
        match self.variant {
            SstVariant::Sst => (SIGMA_K, SIGMA_W, BETA, GAMMA),
            SstVariant::Bsl => ([0.5, 1.0], SIGMA_W, BETA, [bsl_gamma(0), bsl_gamma(1)]),
        }
    }

    /// Eddy viscosity from the local state (`s` strain magnitude, `f2` blending function).
    fn nut_of(&self, k: f64, w: f64, s: f64, f2: f64) -> f64 {
        match self.variant {
            SstVariant::Sst => A1 * k / (A1 * w).max(s * f2).max(1e-30),
            SstVariant::Bsl => k / w.max(1e-30),
        }
    }

    /// Wall distance of every cell centre.
    pub fn wall_distance(&self) -> &[f64] {
        &self.wall_dist
    }

    /// `y+ = u_tau y / nu` (Spalding `u_tau`) of every wall face, in the order of the wall patches'
    /// faces, from the last sweep.
    pub fn y_plus(&self) -> &[f64] {
        &self.y_plus
    }

    /// Blending function `F1` of the last sweep.
    pub fn f1(&self) -> &[f64] {
        &self.f1
    }

    /// Effective face viscosity `nu + nu_t` (wall faces from the wall law) for the momentum equation.
    pub fn nu_face(&self) -> Vec<f64> {
        effective_viscosity(&self.geom, self.nu, &self.nut, &self.wall_nut)
    }

    fn tangential_speed(&self, u: &VectorField, f: usize) -> f64 {
        let c = self.geom.mesh.owner()[f];
        let n = self.geom.normal(f);
        let v = u.values[c];
        vec3::norm(vec3::sub(v, vec3::scale(n, vec3::dot(v, n))))
    }

    fn transport(&self, fld: &ScalarField, phi: &[f64], gamma_f: &[f64]) -> rcfd_fv::FvMatrix {
        let g = &self.geom;
        let cells = fld.values.clone();
        let bc = fld.boundary_coeffs(g);
        let bvals = fld.boundary_values(g);
        let grad = if self.convection.needs_gradient() {
            Some(least_squares(g, &cells, &bvals))
        } else {
            None
        };
        let mut m = convection(g, phi, &cells, &bc, self.convection, grad.as_deref());
        m.add(&laplacian(g, gamma_f, &bc, grad.as_deref()));
        m
    }

    /// One sweep: solve k and omega on the flux `phi` for the current velocity, then update `nut`
    /// and `wall_nut`. Returns the largest relative change of `nut`.
    pub fn update(&mut self, u: &VectorField, phi: &[f64]) -> f64 {
        let g = self.geom.clone();
        let n = g.n_cells();
        let nu = self.nu;
        let vol = g.mesh.cell_volumes().to_vec();
        let gr = velocity_gradient(&g, u);
        let mut s_mag: Vec<f64> = (0..n)
            .map(|c| {
                let mut s2 = 0.0;
                for i in 0..3 {
                    for j in 0..3 {
                        let sij = 0.5 * (gr[i][c][j] + gr[j][c][i]);
                        s2 += sij * sij;
                    }
                }
                (2.0 * s2).sqrt()
            })
            .collect();
        let kv = self.k.values.clone();
        let wv = self.omega.values.clone();

        // ---- wall cells: law of the wall, strain, production and omega
        let mut wall_g: Vec<Option<f64>> = vec![None; n];
        let mut wall_w: Vec<Option<f64>> = vec![None; n];
        let wall_cells: Vec<usize> = (0..n).filter(|&c| self.cell_wall_face[c] != usize::MAX).collect();
        let low_re = self.wall_mode == SstWallMode::LowRe;
        let rough = self.roughness.is_rough();
        for &c in &wall_cells {
            if low_re {
                break;
            }
            let f = self.cell_wall_face[c];
            let y = g.dn[f - g.n_internal];
            let uu = self.tangential_speed(u, f);
            let ut = if rough {
                self.law.u_tau_rough(nu, uu, y, &self.roughness)
            } else {
                self.law.u_tau(nu, uu, y)
            };
            if ut <= 0.0 {
                continue;
            }
            // u+ at the cell centre from the law (invert y+ = y+(u+) by u+ = U/u_tau)
            let up = uu / ut;
            let dy_du = if rough {
                self.law.dy_du_with_e(up, self.law.e_rough(nu, ut, &self.roughness))
            } else {
                self.law.dy_du(up)
            };
            let dudy = ut * ut / nu / dy_du;
            let f_t = 1.0 - 1.0 / dy_du;
            s_mag[c] = dudy;
            wall_g[c] = Some(ut * ut * f_t * dudy);
            if ut * y / nu > self.omega_fix_y_plus {
                let w_vis = 6.0 * nu / (BETA[0] * y * y);
                let w_log = ut / (BETA_STAR.sqrt() * self.law.kappa * y);
                wall_w[c] = Some((w_vis * w_vis + w_log * w_log).sqrt());
            }
        }

        // ---- cross diffusion, F1 and F2
        let kb = self.k.boundary_values(&g);
        let wb = self.omega.boundary_values(&g);
        let gk = green_gauss(&g, &kv, &kb, 0);
        let gw = green_gauss(&g, &wv, &wb, 0);
        let mut cd = vec![0.0; n]; // 2 sigma_w2 / omega grad k . grad omega
        let mut f1 = vec![0.0; n];
        let mut f2 = vec![0.0; n];
        for c in 0..n {
            let (kc, wc, d) = (kv[c].max(1e-30), wv[c].max(1e-30), self.wall_dist[c]);
            cd[c] = 2.0 * SIGMA_W[1] / wc * vec3::dot(gk[c], gw[c]);
            f1[c] = blend_f1(kc, wc, d, nu, cd[c]);
            f2[c] = blend_f2(kc, wc, d, nu);
        }
        self.f1 = f1.clone();
        let mix = |f: f64, a: [f64; 2]| f * a[0] + (1.0 - f) * a[1];
        let (c_sigma_k, c_sigma_w, c_beta, c_gamma) = self.coeffs();

        // eddy viscosity used in the transport equations (consistent with the strain used here)
        let nut_now: Vec<f64> = (0..n).map(|c| self.nut_of(kv[c], wv[c], s_mag[c], f2[c])).collect();
        let gam_k: Vec<f64> = (0..n).map(|c| mix(f1[c], c_sigma_k) * nut_now[c]).collect();
        let gam_w: Vec<f64> = (0..n).map(|c| mix(f1[c], c_sigma_w) * nut_now[c]).collect();
        let gk_f = effective_viscosity(&g, nu, &gam_k, &[]);
        let gw_f = effective_viscosity(&g, nu, &gam_w, &[]);

        // production (limited)
        let mut ptil = vec![0.0; n];
        for c in 0..n {
            let p = wall_g[c].unwrap_or(nut_now[c] * s_mag[c] * s_mag[c]);
            ptil[c] = p.min(10.0 * BETA_STAR * kv[c] * wv[c]);
        }

        // ---- k equation
        let mut mk = self.transport(&self.k, phi, &gk_f);
        for c in 0..n {
            mk.b[c] += ptil[c] * vol[c];
            mk.a.diag[c] += BETA_STAR * wv[c] * vol[c];
        }
        mk.relax(self.relax_turb, &kv);
        let mut knew = kv.clone();
        mk.solve(&mut knew, &self.solver);
        knew.iter_mut().for_each(|x| *x = x.max(1e-12));
        self.k.values = knew;

        // ---- omega equation
        let mut mw = self.transport(&self.omega, phi, &gw_f);
        for c in 0..n {
            let beta = mix(f1[c], c_beta);
            let gamma = mix(f1[c], c_gamma);
            let nut_c = nut_now[c].max(1e-30);
            mw.b[c] += gamma * ptil[c] / nut_c * vol[c];
            mw.a.diag[c] += beta * wv[c] * vol[c];
            let cdc = (1.0 - f1[c]) * cd[c];
            if cdc >= 0.0 {
                mw.b[c] += cdc * vol[c];
            } else {
                mw.a.diag[c] += -cdc / wv[c].max(1e-30) * vol[c];
            }
        }
        mw.relax(self.relax_turb, &wv);
        for &c in &wall_cells {
            if let Some(w) = wall_w[c] {
                let big = 1e8 * mw.a.diag[c].abs().max(1e-30);
                mw.a.diag[c] = big;
                mw.b[c] = big * w;
            }
        }
        let mut wnew = wv.clone();
        mw.solve(&mut wnew, &self.solver);
        wnew.iter_mut().for_each(|x| *x = x.max(1e-10));
        self.omega.values = wnew;

        // ---- eddy viscosity
        let (kc, wcv) = (self.k.values.clone(), self.omega.values.clone());
        let cap = 1e5 * nu;
        let mut change: f64 = 0.0;
        let alpha = if self.started { self.relax_nut } else { 1.0 };
        for c in 0..n {
            let new = self.nut_of(kc[c], wcv[c], s_mag[c], f2[c]).min(cap);
            let upd = alpha * new + (1.0 - alpha) * self.nut[c];
            change = change.max((upd - self.nut[c]).abs() / (self.nut[c] + nu));
            self.nut[c] = upd;
        }
        self.started = true;
        let old = std::mem::take(&mut self.wall_nut);
        let law = self.law;
        let mut yp = vec![0.0; self.wall_faces.len()];
        self.wall_nut = self
            .wall_faces
            .iter()
            .enumerate()
            .map(|(i, &f)| {
                let y = g.dn[f - g.n_internal];
                let uu = self.tangential_speed(u, f);
                let ut = if uu <= 0.0 {
                    0.0
                } else if low_re {
                    (nu * uu / y).sqrt()
                } else if rough {
                    law.u_tau_rough(nu, uu, y, &self.roughness)
                } else {
                    law.u_tau(nu, uu, y)
                };
                yp[i] = ut * y / nu;
                let new = if uu > 0.0 && !low_re {
                    (ut * ut * y / uu - nu).max(0.0)
                } else {
                    0.0
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

    /// ADR 0015 unit checks: BSL constants and eddy viscosity.
    #[test]
    fn bsl_constants_and_eddy_viscosity() {
        assert!((bsl_gamma(0) - 0.5532).abs() < 5e-4, "{}", bsl_gamma(0));
        assert!((bsl_gamma(1) - 0.4403).abs() < 5e-4, "{}", bsl_gamma(1));
        let mesh = rcfd_mesh::gen::box_2d(4, 4, 1.0, 1.0, false, false).unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
        let mut sst = KOmegaSst::new(geom, 1e-3, &["ymin"], 1e-3, 10.0).unwrap();
        sst.variant = SstVariant::Bsl;
        // BSL: nu_t = k/omega whatever the strain and F2
        assert!((sst.nut_of(2.0, 40.0, 1e6, 1.0) - 0.05).abs() < 1e-15);
        sst.variant = SstVariant::Sst;
        // SST: a1 limiter active for large strain
        let nut = sst.nut_of(2.0, 40.0, 1e6, 1.0);
        assert!((nut - 0.31 * 2.0 / 1e6).abs() < 1e-18);
    }

    #[test]
    fn blending_functions_limits() {
        let nu = 1.5e-5;
        // very close to a wall (small d): F1 -> 1; far from it with weak cross diffusion: F1 -> 0
        assert!(blend_f1(1e-3, 100.0, 1e-5, nu, 0.0) > 0.999);
        assert!(blend_f1(1e-3, 10.0, 10.0, nu, 1e-3) < 1e-6);
        assert!(blend_f2(1e-3, 100.0, 1e-5, nu) > 0.999);
        assert!(blend_f2(1e-3, 10.0, 10.0, nu) < 1e-3);
        // both in [0, 1]
        for &d in &[1e-6, 1e-4, 1e-2, 1.0] {
            let (a, b) = (blend_f1(0.01, 50.0, d, nu, 0.3), blend_f2(0.01, 50.0, d, nu));
            assert!((0.0..=1.0).contains(&a) && (0.0..=1.0).contains(&b));
        }
    }
}
