//! k-epsilon eddy-viscosity models with standard wall functions (F-042).
//!
//! Three variants share the transport equations
//!
//! ```text
//! div(phi k)   - div((nu + nu_t/sigma_k) grad k)   = G - eps
//! div(phi eps) - div((nu + nu_t/sigma_e) grad eps) = S_eps
//! nu_t = C_mu k^2 / eps,   G = nu_t S^2,   S = sqrt(2 S_ij S_ij)
//! ```
//!
//! * `Standard`: Launder and Spalding (1974): `C_mu = 0.09`, `C1 = 1.44`, `C2 = 1.92`,
//!   `sigma_k = 1.0`, `sigma_e = 1.3`, `S_eps = (C1 G - C2 eps) eps / k`.
//! * `Rng`: Yakhot and Orszag (1986), Yakhot et al. (1992): `C_mu = 0.0845`, `C1 = 1.42`,
//!   `C2 = 1.68`, `sigma_k = sigma_e = 0.7194`, with the strain-dependent correction
//!   `C2* = C2 + C_mu eta^3 (1 - eta/eta0) / (1 + beta eta^3)`, `eta = S k / eps`,
//!   `eta0 = 4.38`, `beta = 0.012`.
//! * `Realizable`: Shih et al. (1995): variable `C_mu = 1 / (A0 + A_s U* k/eps)`,
//!   `S_eps = C1 S eps - C2 eps^2 / (k + sqrt(nu eps))`, `C1 = max(0.43, eta/(eta+5))`,
//!   `C2 = 1.9`, `sigma_k = 1`, `sigma_e = 1.2`.
//!
//! The RNG and realizable constants were checked against published model documentation, the
//! OpenFOAM class documentation and an arXiv RNG table (see docs/reference-data.md). Note that
//! Shih et al.'s `A0` appears as 4.04 (used here) and as 4.0 (OpenFOAM default).
//!
//! Walls use the equilibrium log-law wall functions of [`crate::wall`]: in a wall cell
//! `eps = C_mu^(3/4) k^(3/2) / (kappa y)` is imposed and the production is replaced by the
//! wall-shear value `G = tau_w C_mu^(1/4) sqrt(k) / (kappa y)`; `k` has zero gradient at the wall
//! and the wall-face eddy viscosity follows from `k` (`nu_t,w`). A cell with several wall faces
//! uses the nearest one.

use crate::eddy::{effective_viscosity, patch_faces};
use crate::wall::{wall_distance, WallConstants};
use rcfd_core::vec3;
use rcfd_core::Result;
use rcfd_fv::convection::{convection, ConvScheme};
use rcfd_fv::diffusion::laplacian;
use rcfd_fv::gradient::{green_gauss, least_squares};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use std::sync::Arc;

/// Model variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KEpsilonVariant {
    /// Launder-Spalding standard model.
    Standard,
    /// Renormalisation-group model.
    Rng,
    /// Realizable model of Shih et al.
    Realizable,
}

/// Treatment of the wall-cell production and dissipation around the viscous/log intersection
/// `y* = y*_lam`.
///
/// The sharp switch changes the wall-cell epsilon by a factor of about four (`2 nu k / y^2` below,
/// `C_mu^(3/4) k^(3/2) / (kappa y)` above, i.e. about 8.4 nu k / y^2 at the intersection) and the
/// production from zero to the log-law value, so a cell whose `y*` sits near the intersection can
/// flip between the two branches from one iteration to the next and `nu_t` never settles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WallTreatment {
    /// Sharp switch at `y*_lam` (the Launder-Spalding two-layer form; may chatter).
    Switch,
    /// Sharp switch with a dead band: a wall cell changes branch only when `y*` leaves
    /// `y*_lam (1 -/+ band)`; the branch of each cell is remembered between sweeps.
    Hysteresis {
        /// Relative half width of the dead band (0.1 = 10 percent).
        band: f64,
    },
    /// Smooth blend: `eps_w = sqrt(eps_v^2 + eps_l^2)` (the compound form of Popovac and Hanjalic,
    /// Flow Turbul. Combust. 78 (2007) 177-202, reduces to the two limits) and the log-law
    /// production multiplied by `w = r^n / (1 + r^n)`, `r = y*/y*_lam` (rcfd's own smooth step,
    /// n = 6).
    Smooth,
}

/// Production and dissipation imposed in a wall-adjacent cell with turbulent kinetic energy `k`,
/// wall distance `y` and tangential speed `u_t`, as `(G, epsilon)`. `in_log` is the remembered
/// branch used by [`WallTreatment::Hysteresis`].
pub fn wall_cell_sources(
    treatment: WallTreatment,
    in_log: &mut bool,
    wc: &WallConstants,
    nu: f64,
    k: f64,
    y: f64,
    u_t: f64,
) -> (f64, f64) {
    let k = k.max(0.0);
    let c25 = wc.cmu.powf(0.25);
    let yst = c25 * k.sqrt() * y / nu;
    let ylam = wc.y_plus_lam();
    let eps_log = c25.powi(3) * k.powf(1.5) / (wc.kappa * y);
    let eps_vis = 2.0 * nu * k / (y * y);
    let log_prod = (nu + wc.nut_wall_from_k(nu, k, y)) * u_t / y * c25 * k.sqrt() / (wc.kappa * y);
    match treatment {
        WallTreatment::Smooth => {
            let r = (yst / ylam).powi(6);
            (r / (1.0 + r) * log_prod, (eps_vis * eps_vis + eps_log * eps_log).sqrt())
        }
        WallTreatment::Switch | WallTreatment::Hysteresis { .. } => {
            if let WallTreatment::Hysteresis { band } = treatment {
                if *in_log && yst < ylam * (1.0 - band) {
                    *in_log = false;
                } else if !*in_log && yst > ylam * (1.0 + band) {
                    *in_log = true;
                }
            } else {
                *in_log = yst > ylam;
            }
            if *in_log {
                (log_prod, eps_log)
            } else {
                // viscous sublayer: laminar production vanishes, epsilon = 2 nu k / y^2
                (0.0, eps_vis)
            }
        }
    }
}

/// Velocity gradient `g[i][c][j] = d u_i / d x_j` by Green-Gauss.
pub(crate) fn velocity_gradient(geom: &FvGeometry, u: &VectorField) -> Vec<Vec<[f64; 3]>> {
    let ub = u.boundary_values(geom);
    (0..3)
        .map(|k| {
            let cells: Vec<f64> = u.values.iter().map(|v| v[k]).collect();
            let bv: Vec<f64> = ub.iter().map(|v| v[k]).collect();
            green_gauss(geom, &cells, &bv, 0)
        })
        .collect()
}

/// Steady k-epsilon model state.
pub struct KEpsilon {
    /// Variant.
    pub variant: KEpsilonVariant,
    /// Geometry.
    pub geom: Arc<FvGeometry>,
    /// Turbulent kinetic energy (m^2/s^2).
    pub k: ScalarField,
    /// Dissipation rate (m^2/s^3).
    pub eps: ScalarField,
    /// Laminar kinematic viscosity.
    pub nu: f64,
    /// Eddy viscosity of every cell (under-relaxed).
    pub nut: Vec<f64>,
    /// Wall-function eddy viscosity of the wall faces `(global face, nu_t,w)`.
    pub wall_nut: Vec<(usize, f64)>,
    /// Wall constants.
    pub wc: WallConstants,
    /// Convection scheme of k and epsilon.
    pub convection: ConvScheme,
    /// Implicit under-relaxation of k and epsilon.
    pub relax_turb: f64,
    /// Explicit under-relaxation of the eddy viscosity.
    pub relax_nut: f64,
    /// Linear solver controls.
    pub solver: SolveControl,
    /// Wall-cell treatment around the viscous/log intersection.
    pub wall_treatment: WallTreatment,
    /// Model constants.
    pub c_mu: f64,
    /// `C1` (standard and RNG).
    pub c1: f64,
    /// `C2`.
    pub c2: f64,
    /// Prandtl number of k.
    pub sigma_k: f64,
    /// Prandtl number of epsilon.
    pub sigma_e: f64,
    wall_dist: Vec<f64>,
    wall_faces: Vec<usize>,
    /// Wall face of every wall-adjacent cell (nearest wall face), `usize::MAX` otherwise.
    cell_wall_face: Vec<usize>,
    /// Branch remembered by the hysteresis treatment (`true` = log layer).
    wall_log: Vec<bool>,
    started: bool,
}

impl KEpsilon {
    /// New model with uniform initial `k` and `eps`; `walls` are the wall patch names.
    pub fn new(
        variant: KEpsilonVariant,
        geom: Arc<FvGeometry>,
        nu: f64,
        walls: &[&str],
        k0: f64,
        eps0: f64,
    ) -> Result<Self> {
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
        let (c_mu, c1, c2, sigma_k, sigma_e) = match variant {
            KEpsilonVariant::Standard => (0.09, 1.44, 1.92, 1.0, 1.3),
            KEpsilonVariant::Rng => (0.0845, 1.42, 1.68, 0.7194, 0.7194),
            KEpsilonVariant::Realizable => (0.09, 1.44, 1.9, 1.0, 1.2),
        };
        let n = geom.n_cells();
        let wc = WallConstants {
            cmu: c_mu,
            ..WallConstants::default()
        };
        Ok(KEpsilon {
            variant,
            k: ScalarField::uniform("k", &geom, k0),
            eps: ScalarField::uniform("epsilon", &geom, eps0),
            nu,
            nut: (0..n).map(|_| c_mu * k0 * k0 / eps0).collect(),
            wall_nut: Vec::new(),
            wc,
            convection: ConvScheme::Upwind,
            relax_turb: 0.7,
            relax_nut: 0.5,
            solver: SolveControl {
                tol_abs: 1e-14,
                tol_rel: 1e-6,
                max_iter: 500,
            },
            wall_treatment: WallTreatment::Smooth,
            c_mu,
            c1,
            c2,
            sigma_k,
            sigma_e,
            wall_log: vec![false; geom.n_cells()],
            wall_dist,
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

    /// Effective face viscosity `nu + nu_t` (wall faces from the wall function) for the momentum equation.
    pub fn nu_face(&self) -> Vec<f64> {
        effective_viscosity(&self.geom, self.nu, &self.nut, &self.wall_nut)
    }

    /// Tangential speed of the cell next to a wall face relative to the wall.
    fn tangential_speed(&self, u: &VectorField, f: usize) -> f64 {
        let c = self.geom.mesh.owner()[f];
        let n = self.geom.normal(f);
        let v = u.values[c];
        vec3::norm(vec3::sub(v, vec3::scale(n, vec3::dot(v, n))))
    }

    fn transport(&self, fld: &ScalarField, phi: &[f64], gamma_t: f64, nut_f: &[f64]) -> rcfd_fv::FvMatrix {
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
        let gamma: Vec<f64> = nut_f.iter().map(|&nt| self.nu + nt / gamma_t).collect();
        m.add(&laplacian(g, &gamma, &bc, grad.as_deref()));
        m
    }

    /// One sweep: solve k and epsilon on the flux `phi` for the current velocity, then update
    /// `nut` and `wall_nut`. Returns the largest relative change of `nut`.
    pub fn update(&mut self, u: &VectorField, phi: &[f64]) -> f64 {
        let g = self.geom.clone();
        let n = g.n_cells();
        let vol = g.mesh.cell_volumes().to_vec();
        let gr = velocity_gradient(&g, u);
        // strain, rotation invariants
        let mut s_mag = vec![0.0; n]; // sqrt(2 S_ij S_ij)
        let mut u_star = vec![0.0; n]; // sqrt(S_ij S_ij + W_ij W_ij)
        let mut w_cos = vec![0.0; n]; // W = S_ij S_jk S_ki / (S_ij S_ij)^1.5
        for c in 0..n {
            let (mut s2, mut o2) = (0.0, 0.0);
            let mut sm = [[0.0; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    let sij = 0.5 * (gr[i][c][j] + gr[j][c][i]);
                    let oij = 0.5 * (gr[i][c][j] - gr[j][c][i]);
                    sm[i][j] = sij;
                    s2 += sij * sij;
                    o2 += oij * oij;
                }
            }
            s_mag[c] = (2.0 * s2).sqrt();
            u_star[c] = (s2 + o2).sqrt();
            let mut tr3 = 0.0;
            for i in 0..3 {
                for j in 0..3 {
                    for l in 0..3 {
                        tr3 += sm[i][j] * sm[j][l] * sm[l][i];
                    }
                }
            }
            w_cos[c] = if s2 > 1e-300 { tr3 / s2.powf(1.5) } else { 0.0 };
        }
        let kv = self.k.values.clone();
        let ev = self.eps.values.clone();
        let cmu = self.c_mu;
        let c_mu_cell = |c: usize| -> f64 {
            if self.variant == KEpsilonVariant::Realizable {
                let phi_a = (6f64.sqrt() * w_cos[c].clamp(-1.0, 1.0)).clamp(-1.0, 1.0).acos() / 3.0;
                let a_s = 6f64.sqrt() * phi_a.cos();
                1.0 / (4.04 + a_s * u_star[c] * kv[c] / ev[c])
            } else {
                cmu
            }
        };
        let nut_f = effective_viscosity(&g, 0.0, &self.nut, &[]);

        // wall-cell production and dissipation (wall functions)
        let mut prod = vec![0.0; n];
        for (c, p) in prod.iter_mut().enumerate() {
            *p = self.nut[c] * s_mag[c] * s_mag[c];
        }
        let mut wall_eps: Vec<Option<f64>> = vec![None; n];
        let wall_cells: Vec<usize> = (0..n).filter(|&c| self.cell_wall_face[c] != usize::MAX).collect();
        for &c in &wall_cells {
            let f = self.cell_wall_face[c];
            let y = g.dn[f - g.n_internal];
            let u_t = self.tangential_speed(u, f);
            let (pc, ec) = wall_cell_sources(
                self.wall_treatment,
                &mut self.wall_log[c],
                &self.wc,
                self.nu,
                kv[c],
                y,
                u_t,
            );
            prod[c] = pc;
            wall_eps[c] = Some(ec);
        }

        // ---- k equation
        let mut mk = self.transport(&self.k, phi, self.sigma_k, &nut_f);
        for c in 0..n {
            mk.b[c] += prod[c] * vol[c];
            let e = wall_eps[c].unwrap_or(ev[c]);
            mk.a.diag[c] += e / kv[c].max(1e-30) * vol[c];
        }
        mk.relax(self.relax_turb, &kv);
        let mut knew = kv.clone();
        mk.solve(&mut knew, &self.solver);
        knew.iter_mut().for_each(|x| *x = x.max(1e-10));
        self.k.values = knew;

        // ---- epsilon equation (uses the updated k)
        let kv2 = self.k.values.clone();
        let mut me = self.transport(&self.eps, phi, self.sigma_e, &nut_f);
        for c in 0..n {
            let (kc, ec) = (kv2[c].max(1e-30), ev[c].max(1e-30));
            let eta = s_mag[c] * kc / ec;
            match self.variant {
                KEpsilonVariant::Standard => {
                    me.b[c] += self.c1 * prod[c] * ec / kc * vol[c];
                    me.a.diag[c] += self.c2 * ec / kc * vol[c];
                }
                KEpsilonVariant::Rng => {
                    let r = cmu * eta.powi(3) * (1.0 - eta / 4.38) / (1.0 + 0.012 * eta.powi(3));
                    me.b[c] += self.c1 * prod[c] * ec / kc * vol[c];
                    // C2* may turn negative for large eta; keep the sink implicit-positive part only
                    let c2s = self.c2 + r;
                    if c2s >= 0.0 {
                        me.a.diag[c] += c2s * ec / kc * vol[c];
                    } else {
                        me.b[c] += -c2s * ec * ec / kc * vol[c];
                    }
                }
                KEpsilonVariant::Realizable => {
                    let c1 = (eta / (eta + 5.0)).max(0.43);
                    me.b[c] += c1 * s_mag[c] * ec * vol[c];
                    me.a.diag[c] += self.c2 * ec / (kc + (self.nu * ec).sqrt()) * vol[c];
                }
            }
        }
        me.relax(self.relax_turb, &ev);
        // wall cells: fix epsilon with a dominant diagonal
        for &c in &wall_cells {
            if let Some(e) = wall_eps[c] {
                let big = 1e8 * me.a.diag[c].abs().max(1e-30);
                me.a.diag[c] = big;
                me.b[c] = big * e;
            }
        }
        let mut enew = ev.clone();
        me.solve(&mut enew, &self.solver);
        enew.iter_mut().for_each(|x| *x = x.max(1e-10));
        self.eps.values = enew;

        // ---- eddy viscosity
        let (kc, ec) = (self.k.values.clone(), self.eps.values.clone());
        let cap = 1e5 * self.nu;
        let mut change: f64 = 0.0;
        let alpha = if self.started { self.relax_nut } else { 1.0 };
        for c in 0..n {
            let new = (c_mu_cell(c) * kc[c] * kc[c] / ec[c]).min(cap);
            let upd = alpha * new + (1.0 - alpha) * self.nut[c];
            change = change.max((upd - self.nut[c]).abs() / (self.nut[c] + self.nu));
            self.nut[c] = upd;
        }
        self.started = true;
        let old = std::mem::take(&mut self.wall_nut);
        self.wall_nut = self
            .wall_faces
            .iter()
            .enumerate()
            .map(|(i, &f)| {
                let c = g.mesh.owner()[f];
                let y = g.dn[f - g.n_internal];
                let new = self.wc.nut_wall_from_k(self.nu, kc[c], y);
                let v = match old.get(i) {
                    Some(&(_, o)) => alpha * new + (1.0 - alpha) * o,
                    None => new,
                };
                (f, v)
            })
            .collect();
        change
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// k giving a requested `y*` at wall distance `y`.
    fn k_for(wc: &WallConstants, nu: f64, y: f64, yst: f64) -> f64 {
        (yst * nu / (wc.cmu.powf(0.25) * y)).powi(2)
    }

    #[test]
    fn smooth_treatment_has_no_jump_at_the_intersection() {
        let wc = WallConstants::default();
        let (nu, y, ut) = (1e-3, 1e-2, 0.5);
        let yl = wc.y_plus_lam();
        let at = |t, st: &mut bool, ys: f64| wall_cell_sources(t, st, &wc, nu, k_for(&wc, nu, y, ys), y, ut);
        // sharp switch: epsilon jumps by a factor of more than three across y*_lam
        let (mut a, mut b) = (false, false);
        let lo = at(WallTreatment::Switch, &mut a, yl * 0.999);
        let hi = at(WallTreatment::Switch, &mut b, yl * 1.001);
        assert!(hi.1 / lo.1 > 3.0, "switch eps jump {}", hi.1 / lo.1);
        assert_eq!(lo.0, 0.0);
        assert!(hi.0 > 0.0);
        // smooth blend: relative change over a 0.2 percent step in y* is tiny for both quantities
        let (mut a, mut b) = (false, false);
        let lo = at(WallTreatment::Smooth, &mut a, yl * 0.999);
        let hi = at(WallTreatment::Smooth, &mut b, yl * 1.001);
        assert!((hi.1 / lo.1 - 1.0).abs() < 0.01, "eps {} {}", lo.1, hi.1);
        assert!((hi.0 / lo.0 - 1.0).abs() < 0.05, "G {} {}", lo.0, hi.0);
        // limits: viscous sublayer gives eps = 2 nu k / y^2 and G -> 0; deep log layer the log law
        let (mut a, mut b) = (false, false);
        let deep = at(WallTreatment::Smooth, &mut a, 0.02);
        let k = k_for(&wc, nu, y, 0.02);
        assert!((deep.1 / (2.0 * nu * k / (y * y)) - 1.0).abs() < 1e-3);
        assert!(deep.0 < 1e-9 * at(WallTreatment::Switch, &mut b, 100.0).0);
        let mut c = false;
        let log = at(WallTreatment::Smooth, &mut c, 100.0);
        let k = k_for(&wc, nu, y, 100.0);
        let eps_l = wc.cmu.powf(0.75) * k.powf(1.5) / (wc.kappa * y);
        assert!((log.1 / eps_l - 1.0).abs() < 1e-3);
    }

    #[test]
    fn hysteresis_keeps_its_branch_inside_the_dead_band() {
        let wc = WallConstants::default();
        let (nu, y, ut) = (1e-3, 1e-2, 0.5);
        let yl = wc.y_plus_lam();
        let t = WallTreatment::Hysteresis { band: 0.1 };
        let mut st = false;
        let mut run = |ys: f64| {
            wall_cell_sources(t, &mut st, &wc, nu, k_for(&wc, nu, y, ys), y, ut);
            st
        };
        assert!(!run(yl * 1.05)); // inside the band: stays viscous
        assert!(run(yl * 1.2)); // above the band: log
        assert!(run(yl * 0.95)); // inside the band: stays log
        assert!(!run(yl * 0.8)); // below the band: viscous
        assert!(!run(yl * 1.05));
    }
}
