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

/// Strain-rate magnitude `S = sqrt(2 S_ij S_ij)` of cell `c` from the velocity gradient `gr[i][c][j] = d u_i / d x_j`
/// (NASA TMR SST page; the vorticity magnitude is NOT used by the 2003 form).
pub(crate) fn strain_magnitude(gr: &[Vec<[f64; 3]>], c: usize) -> f64 {
    let mut s2 = 0.0;
    for i in 0..3 {
        for j in 0..3 {
            let sij = 0.5 * (gr[i][c][j] + gr[j][c][i]);
            s2 += sij * sij;
        }
    }
    (2.0 * s2).sqrt()
}

/// Limited production `P~ = min(P, 10 beta* k omega)` of both equations (SST-2003, TMR SST page).
pub fn limited_production(p: f64, k: f64, omega: f64) -> f64 {
    p.min(10.0 * BETA_STAR * k * omega)
}

/// Cross-diffusion `2 sigma_w2 / omega  grad k . grad omega` (without the `(1 - F1)` factor and without the 1e-10 floor of F1).
pub fn cross_diffusion(grad_k: [f64; 3], grad_w: [f64; 3], omega: f64) -> f64 {
    2.0 * SIGMA_W[1] / omega * vec3::dot(grad_k, grad_w)
}

/// Splits the omega-equation cross-diffusion term `cdc = (1 - F1) CD` into an explicit source and an implicit
/// (always non-negative) diagonal coefficient; on the right-hand side `source - coefficient * omega = cdc` for either sign
/// of `cdc` (the coefficient is added to the matrix diagonal, i.e. it is a sink when `cdc < 0`).
pub fn cross_diffusion_split(cdc: f64, omega: f64) -> (f64, f64) {
    if cdc >= 0.0 {
        (cdc, 0.0)
    } else {
        (0.0, -cdc / omega.max(1e-30))
    }
}

/// Blend `phi = F1 phi_1 + (1 - F1) phi_2` of an inner/outer constant pair.
pub fn blend_constant(f1: f64, a: [f64; 2]) -> f64 {
    f1 * a[0] + (1.0 - f1) * a[1]
}

/// Wall value of omega on a wall face, `10 * 6 nu / (beta_1 y^2)` with `y` the first-cell centre distance (TMR SST page).
pub fn wall_omega(nu: f64, y: f64) -> f64 {
    10.0 * 6.0 * nu / (BETA[0] * y * y)
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
                        [wall_omega(nu, y), 0.0, 0.0]
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

    /// Tangential speed of the wall cell relative to the wall (ADR 0075: the cell velocity minus the face boundary velocity, so a
    /// moving wall is treated in its own frame; a wall at rest has the boundary velocity 0 exactly and is unchanged).
    fn tangential_speed(&self, u: &VectorField, ub: &[vec3::Vec3], f: usize) -> f64 {
        let c = self.geom.mesh.owner()[f];
        let n = self.geom.normal(f);
        let v = vec3::sub(u.values[c], ub[f - self.geom.n_internal]);
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
        let ub = u.boundary_values(&g);
        let mut s_mag: Vec<f64> = (0..n).map(|c| strain_magnitude(&gr, c)).collect();
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
            let uu = self.tangential_speed(u, &ub, f);
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
            cd[c] = cross_diffusion(gk[c], gw[c], wc);
            f1[c] = blend_f1(kc, wc, d, nu, cd[c]);
            f2[c] = blend_f2(kc, wc, d, nu);
        }
        self.f1 = f1.clone();
        let mix = blend_constant;
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
            ptil[c] = limited_production(p, kv[c], wv[c]);
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
            let (src, coef) = cross_diffusion_split(cdc, wv[c]);
            mw.b[c] += src * vol[c];
            mw.a.diag[c] += coef * vol[c];
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
                let uu = self.tangential_speed(u, &ub, f);
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

    // ---- Wave 33C (ADR 0070): term-by-term audit against the NASA TMR SST page, hand values computed independently.

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-12 * b.abs().max(1e-300)
    }

    #[test]
    fn w33c_constants_and_model_kappa() {
        assert_eq!(BETA_STAR, 0.09);
        assert_eq!(A1, 0.31);
        assert_eq!(SIGMA_K, [0.85, 1.0]);
        assert_eq!(SIGMA_W, [0.5, 0.856]);
        assert_eq!(BETA, [0.075, 0.0828]);
        assert_eq!(GAMMA, [5.0 / 9.0, 0.44]);
        // kappa^2 = sqrt(beta*) (beta/beta* - gamma) / sigma_w for both sets (TMR: kappa = 0.41 in the consistency relation)
        for (i, want) in [(0usize, 0.408248290463863), (1, 0.410151556214798)] {
            let k = (BETA_STAR.sqrt() * (BETA[i] / BETA_STAR - GAMMA[i]) / SIGMA_W[i]).sqrt();
            assert!(close(k, want), "set {i}: {k}");
        }
        assert!(close(bsl_gamma(0), 0.5531666666666668) && close(bsl_gamma(1), 0.4403546666666667));
    }

    #[test]
    fn w33c_constant_blending() {
        let f = 0.3;
        assert!(close(blend_constant(f, SIGMA_K), 0.955));
        assert!(close(blend_constant(f, SIGMA_W), 0.7492));
        assert!(close(blend_constant(f, BETA), 0.08046));
        assert!(close(blend_constant(f, GAMMA), 0.4746666666666667));
        assert_eq!(blend_constant(1.0, BETA), BETA[0]);
        assert_eq!(blend_constant(0.0, BETA), BETA[1]);
    }

    #[test]
    fn w33c_f1_branches_and_cd_floor() {
        let nu = 1.5e-5;
        // (k, omega, d, CD) -> F1; the active branch of the min/max is named
        assert!(close(blend_f1(0.006561, 10.0, 0.1, nu, 0.01), 0.5757621252496801)); // sqrt(k)/(beta* w d)
        assert!(close(blend_f1(1e-6, 100.0, 0.01, nu, 0.01), 0.3062538284406168)); // 500 nu/(d^2 w)
        assert!(close(blend_f1(0.01, 10.0, 0.1, nu, 10.0), 0.013743826901846232)); // 4 sigma_w2 k/(CD d^2)
        assert!(close(blend_f1(0.01, 10.0, 0.1, nu, -5.0), 0.90941951863331)); // CD floored at 1e-10
    }

    #[test]
    fn w33c_f2_branches() {
        let nu = 1.5e-5;
        assert!(close(blend_f2(0.006561, 10.0, 0.1, nu), 0.9969370765859934));
        assert!(close(blend_f2(1e-6, 100.0, 0.01, nu), 0.5098299737352566));
        assert!(close(blend_f2(0.01, 10.0, 0.1, nu), 0.9998972742141851));
    }

    #[test]
    fn w33c_cross_diffusion_value_and_split() {
        let cd = cross_diffusion([0.2, -0.1, 0.0], [3.0, 4.0, 0.0], 25.0);
        assert!(close(cd, 0.013696));
        for &t in &[0.013696, -0.013696, 0.0] {
            let (src, coef) = cross_diffusion_split(t, 25.0);
            assert!(coef >= 0.0, "implicit coefficient must not be negative");
            assert!((src - coef * 25.0 - t).abs() < 1e-15);
        }
    }

    #[test]
    fn w33c_strain_magnitude_is_not_vorticity() {
        let g = 3.0;
        let one = |m: [[f64; 3]; 3]| -> Vec<Vec<[f64; 3]>> { (0..3).map(|i| vec![m[i]]).collect() };
        // simple shear du/dy = g
        let shear = one([[0.0, g, 0.0], [0.0; 3], [0.0; 3]]);
        assert!(close(strain_magnitude(&shear, 0), g));
        // plane strain (a x, -a y): S = 2 a
        let strain = one([[g, 0.0, 0.0], [0.0, -g, 0.0], [0.0; 3]]);
        assert!(close(strain_magnitude(&strain, 0), 2.0 * g));
        // solid-body rotation u = (-W y, W x): S = 0 although the vorticity is 2 W
        let rot = one([[0.0, -g, 0.0], [g, 0.0, 0.0], [0.0; 3]]);
        assert!(strain_magnitude(&rot, 0) < 1e-15);
    }

    #[test]
    fn w33c_eddy_viscosity_limiter_branches() {
        let mesh = rcfd_mesh::gen::box_2d(4, 4, 1.0, 1.0, false, false).unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
        let sst = KOmegaSst::new(geom, 1.5e-5, &["ymin"], 1e-3, 10.0).unwrap();
        // strain-limited: a1 w = 3.1 < S F2 = 4
        assert!(close(sst.nut_of(0.01, 10.0, 5.0, 0.8), 7.75e-4));
        // omega-limited: a1 w = 3.1 > S F2 = 1.8 -> k / omega
        assert!(close(sst.nut_of(0.01, 10.0, 2.0, 0.9), 1.0e-3));
    }

    #[test]
    fn w33c_production_limiter_both_branches() {
        // P = nu_t S^2 = 1e-3 * 100^2 = 10 is limited to 10 beta* k w = 0.45; 0.1 stays
        assert!(close(limited_production(1e-3 * 100.0 * 100.0, 0.01, 50.0), 0.45));
        assert!(close(limited_production(0.1, 0.01, 50.0), 0.1));
    }

    #[test]
    fn w33c_wall_omega_on_the_wall_face() {
        // the helper: 10 * 6 nu/(beta1 y^2); nu 2e-7, y 1e-5 -> 1.6e6
        assert!(close(wall_omega(2e-7, 1e-5), 1.6e6));
        // and the field really carries it: first cell height 2e-5 -> centre distance 1e-5
        let mesh = rcfd_mesh::gen::box_2d_graded(
            4,
            10,
            1.0,
            1.0,
            [
                rcfd_mesh::gen::Grading::Uniform,
                rcfd_mesh::gen::Grading::WallLayers {
                    first: 2e-5,
                    growth: 1.2,
                    layers: 5,
                    both: false,
                },
            ],
            false,
            false,
        )
        .unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
        let sst = KOmegaSst::new(geom.clone(), 2e-7, &["ymin"], 1e-6, 100.0).unwrap();
        let wb = sst.omega.boundary_values(&geom);
        let patch = geom.mesh.patch("ymin").unwrap();
        for f in patch.faces() {
            assert!(close(wb[f - geom.n_internal], 1.6e6), "{}", wb[f - geom.n_internal]);
        }
    }

    #[test]
    fn w33c_wall_modes_set_k_boundary() {
        let mesh = rcfd_mesh::gen::box_2d(4, 4, 1.0, 1.0, false, false).unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
        let mut sst = KOmegaSst::new(geom.clone(), 1e-3, &["ymin"], 0.5, 10.0).unwrap();
        let patch = geom.mesh.patch("ymin").unwrap();
        let f0 = patch.faces().next().unwrap() - geom.n_internal;
        sst.set_wall_mode(SstWallMode::LowRe).unwrap();
        assert_eq!(sst.k.boundary_values(&geom)[f0], 0.0);
        sst.set_wall_mode(SstWallMode::Automatic).unwrap();
        assert_eq!(
            sst.k.boundary_values(&geom)[f0],
            0.5,
            "zero gradient takes the cell value"
        );
    }
}
