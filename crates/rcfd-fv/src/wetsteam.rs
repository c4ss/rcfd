//! Non-equilibrium condensing (wet) steam in transonic flow (F-221, ADR 0217): a single-fluid mixture of steam and
//! monodisperse water droplets with classical homogeneous nucleation and Young's droplet growth law, and an explicit
//! density-based Euler solver for it on polyhedral meshes.
//!
//! Model (ADR 0217 M1-M8; equations from the published sources listed there):
//!
//! * mixture without velocity slip, liquid volume neglected: `p = (1 - y) rho R T` (Afzalifar et al. 2016, Eqs. 5-8);
//! * vapour a perfect gas with constant `c_pv`; liquid enthalpy `h_l(T) = h_v(T) - L(T)` with the Kirchhoff-linear latent heat
//!   `L(T) = L_r + (c_pv - c_pl)(T - T_r)`; mixture `h = (1 - y) h_v + y h_l` (Dykas et al. 2013, Eq. 2). For frozen `y` the
//!   mixture is a perfect gas in the shifted energy `e + q(y)` with ratio of specific heats `Gamma(y)`;
//! * nucleation rate per unit mass of mixture (Starzmann et al. 2016, Eq. 1) with the Kantrowitz correction (Dykas et al.
//!   2013, Eq. 5) and the Kelvin-Helmholtz critical radius (Afzalifar et al. 2016, Eq. 1);
//! * droplet growth of Young (Starzmann et al. 2016, Eqs. 2-3; mean free path of Starzmann et al. 2011, Eq. 9);
//! * saturation line, liquid density and vapour transport from IAPWS-IF97 and the IAPWS 2008/2011 correlations
//!   ([`rcfd_core::iapws`]), surface tension from IAPWS R1-76(2014).
//!
//! Solver (ADR 0217 W1-W5): conserved `(rho, rho u, rho v, rho w, rho E, rho y, rho n)`; HLLC for a general equation of state
//! with the wave-speed estimates of [`crate::riemann`]; MUSCL on primitives with the unstructured TVD form and the
//! least-squares gradients of [`crate::compressible`]; SSP-RK3 with the sources evaluated at every sub-step and an optional
//! point-implicit treatment of the stiff wetness relaxation. With `y = n = 0` and condensation off the solver reproduces
//! [`crate::compressible::CompressibleSolver`] with `gamma = gamma_v` (check D1).

use crate::compressible::{ls_inverse, safe, RkScheme};
use crate::compressible_bc::{self, TotalInlet};
use crate::convection::TvdLimiter;
use crate::geometry::FvGeometry;
use crate::riemann::{Gas, Prim};
use rayon::prelude::*;
use rcfd_core::iapws::{self, R_WATER};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_mesh::PatchKind;
use std::f64::consts::PI;

/// Boltzmann constant (J/K), exact SI value (NIST CODATA).
pub const BOLTZMANN: f64 = 1.380649e-23;
/// Lowest admissible temperature of a state (K), ADR 0217 M8.
pub const T_MIN: f64 = 200.0;
/// Highest admissible temperature of a state (K), ADR 0217 M8.
pub const T_MAX: f64 = 640.0;
/// Upper bound (exclusive) of the wetness, ADR 0217 M8.
pub const Y_MAX: f64 = 0.5;

/// Conserved variables `(rho, rho u, rho v, rho w, rho E, rho y, rho n)`.
pub type WetCons = [f64; 7];

/// Primitive state of the mixture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WetPrim {
    /// Mixture density (kg/m^3).
    pub rho: f64,
    /// Velocity (m/s).
    pub u: Vec3,
    /// Pressure (Pa).
    pub p: f64,
    /// Wetness: liquid mass fraction.
    pub y: f64,
    /// Droplet number per unit mass of mixture (1/kg).
    pub n: f64,
}

impl WetPrim {
    /// A dry state from a perfect-gas primitive state.
    pub fn dry(p: &Prim) -> Self {
        Self {
            rho: p.rho,
            u: p.u,
            p: p.p,
            y: 0.0,
            n: 0.0,
        }
    }

    /// The flow part `(rho, u, p)`.
    pub fn flow(&self) -> Prim {
        Prim {
            rho: self.rho,
            u: self.u,
            p: self.p,
        }
    }

    /// Mirror in a wall with unit normal `n` (slip-wall ghost state), as [`Prim::mirrored`].
    pub fn mirrored(&self, n: Vec3) -> Self {
        let un = vec3::dot(self.u, n);
        Self {
            u: vec3::sub(self.u, vec3::scale(n, 2.0 * un)),
            ..*self
        }
    }

    /// Temperature `p / ((1 - y) rho R)`.
    pub fn temperature(&self) -> f64 {
        self.p / ((1.0 - self.y) * self.rho * R_WATER)
    }
}

/// Surface tension of water (N/m), IAPWS R1-76(2014): `B tau^mu (1 + b tau)`, `tau = 1 - T/T_c`.
pub fn surface_tension(t: f64) -> f64 {
    let tau = 1.0 - t / iapws::T_CRITICAL;
    0.2358 * tau.powf(1.256) * (1.0 - 0.625 * tau)
}

/// Saturated-liquid density (kg/m^3) at temperature `t`, IF97 region 1 on the extended saturation line (ADR 0217 M3).
pub fn liquid_density(t: f64) -> Result<f64> {
    Ok(iapws::saturated_liquid(t)?.density)
}

/// Nucleation result at a state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nucleation {
    /// Nucleation rate per unit mass of mixture (1/(kg s)); zero for `S <= 1`.
    pub rate: f64,
    /// Kelvin-Helmholtz critical radius (m); infinite for `S <= 1`.
    pub r_crit: f64,
    /// Kantrowitz non-isothermal correction `phi`.
    pub phi: f64,
    /// Supersaturation `p / p_s(T)`.
    pub supersaturation: f64,
}

/// Source rates of a state per unit mass of mixture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rates {
    /// `dy/dt` (1/s).
    pub dy_dt: f64,
    /// `dn/dt` (1/(kg s)).
    pub dn_dt: f64,
    /// Temperature (K).
    pub temperature: f64,
    /// Pressure (Pa).
    pub pressure: f64,
}

/// The wet-steam model of ADR 0217 (M1-M8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WetSteamModel {
    /// Vapour isobaric specific heat `c_pv` (J/(kg K)).
    pub cp_v: f64,
    /// Liquid specific heat `c_pl` (J/(kg K)).
    pub cp_l: f64,
    /// Reference temperature `T_r` of the latent heat (K).
    pub t_ref: f64,
    /// Latent heat `L_r` at `T_r` (J/kg).
    pub latent_ref: f64,
    /// Young's empirical growth coefficient `alpha`.
    pub alpha: f64,
    /// Condensation coefficient `q_c`.
    pub q_c: f64,
}

impl WetSteamModel {
    /// The model with `c_pl` and `L_r` from IF97 at `(T_r, p_s(T_r))` (ADR 0217 M2) and `q_c = 1`.
    pub fn from_if97(cp_v: f64, t_ref: f64, alpha: f64) -> Result<Self> {
        if !(cp_v > R_WATER && cp_v.is_finite() && alpha.is_finite()) {
            return Err(Error::Numerical(format!(
                "wet steam: c_pv must exceed R = {R_WATER} J/(kg K) and alpha must be finite (c_pv {cp_v}, alpha {alpha})"
            )));
        }
        let liq = iapws::saturated_liquid(t_ref)?;
        let vap = iapws::saturated_vapour(t_ref)?;
        Ok(Self {
            cp_v,
            cp_l: liq.cp,
            t_ref,
            latent_ref: vap.h - liq.h,
            alpha,
            q_c: 1.0,
        })
    }

    /// The registered parameter rule of ADR 0217 M2: `c_pv` = IF97 region-2 `c_p` at the inlet stagnation state
    /// `(p0, T0)` (which must be superheated vapour), `T_r = 300 K`.
    pub fn for_inlet(p0: f64, t0: f64, alpha: f64) -> Result<Self> {
        let st = iapws::water_state(t0, p0)?;
        if st.region != 2 {
            return Err(Error::Numerical(format!(
                "wet steam: the inlet stagnation state ({p0} Pa, {t0} K) is not vapour"
            )));
        }
        Self::from_if97(st.cp, 300.0, alpha)
    }

    /// `A(y) = c_v + y (R - (c_pv - c_pl))`.
    pub fn a_coef(&self, y: f64) -> f64 {
        (self.cp_v - R_WATER) + y * (R_WATER - (self.cp_v - self.cp_l))
    }

    /// Energy offset `q(y) = y (L_r - (c_pv - c_pl) T_r)`.
    pub fn offset(&self, y: f64) -> f64 {
        y * (self.latent_ref - (self.cp_v - self.cp_l) * self.t_ref)
    }

    /// Ratio of specific heats of the frozen mixture `Gamma(y) = 1 + (1 - y) R / A(y)`.
    pub fn gamma_mix(&self, y: f64) -> f64 {
        1.0 + (1.0 - y) * R_WATER / self.a_coef(y)
    }

    /// The vapour's ratio of specific heats `Gamma(0)`.
    pub fn gamma_v(&self) -> f64 {
        self.gamma_mix(0.0)
    }

    /// The perfect gas of the dry vapour.
    pub fn gas(&self) -> Gas {
        Gas { gamma: self.gamma_v() }
    }

    /// Latent heat `L(T)` (J/kg).
    pub fn latent(&self, t: f64) -> f64 {
        self.latent_ref + (self.cp_v - self.cp_l) * (t - self.t_ref)
    }

    /// Temperature from the specific internal energy and the wetness, `(e + q)/A`.
    pub fn temperature(&self, e: f64, y: f64) -> f64 {
        (e + self.offset(y)) / self.a_coef(y)
    }

    /// Specific internal energy at temperature `t` and wetness `y`.
    pub fn internal_energy(&self, t: f64, y: f64) -> f64 {
        self.a_coef(y) * t - self.offset(y)
    }

    /// Density from pressure, temperature and wetness.
    pub fn density(&self, p: f64, t: f64, y: f64) -> f64 {
        p / ((1.0 - y) * R_WATER * t)
    }

    /// Frozen sound speed `sqrt(Gamma p / rho)`.
    pub fn sound_speed(&self, w: &WetPrim) -> f64 {
        (self.gamma_mix(w.y) * w.p / w.rho).sqrt()
    }

    /// Conserved variables of a primitive state.
    pub fn to_cons(&self, w: &WetPrim) -> WetCons {
        let g = self.gamma_mix(w.y);
        let ke = 0.5 * w.rho * vec3::dot(w.u, w.u);
        [
            w.rho,
            w.rho * w.u[0],
            w.rho * w.u[1],
            w.rho * w.u[2],
            w.p / (g - 1.0) - w.rho * self.offset(w.y) + ke,
            w.rho * w.y,
            w.rho * w.n,
        ]
    }

    /// Primitive state of conserved variables (no validity test; see [`Self::check`]).
    pub fn to_prim(&self, c: &WetCons) -> WetPrim {
        let rho = c[0];
        let u = [c[1] / rho, c[2] / rho, c[3] / rho];
        let y = c[5] / rho;
        let n = c[6] / rho;
        let g = self.gamma_mix(y);
        let p = (g - 1.0) * (c[4] - 0.5 * rho * vec3::dot(u, u) + rho * self.offset(y));
        WetPrim { rho, u, p, y, n }
    }

    /// The validity guard of ADR 0217 M8: `None` for an admissible state, else the reason.
    pub fn check(&self, w: &WetPrim) -> Option<String> {
        if !(w.rho > 0.0 && w.p > 0.0 && w.rho.is_finite() && w.p.is_finite()) {
            return Some(format!(
                "non-positive or non-finite density {} or pressure {}",
                w.rho, w.p
            ));
        }
        if !(w.y >= 0.0 && w.y < Y_MAX) || !(w.n >= 0.0 && w.n.is_finite()) {
            return Some(format!(
                "wetness {} outside [0, {Y_MAX}) or droplet number {} invalid",
                w.y, w.n
            ));
        }
        let t = w.temperature();
        if !(T_MIN..=T_MAX).contains(&t) {
            return Some(format!("temperature {t} K outside [{T_MIN}, {T_MAX}] K"));
        }
        None
    }

    /// Kantrowitz non-isothermal correction `phi(T)` (Dykas et al. 2013, Eq. 5, with `h_v - h_l = L(T)`).
    pub fn kantrowitz_phi(&self, t: f64) -> f64 {
        let g = self.gamma_v();
        let lrt = self.latent(t) / (R_WATER * t);
        2.0 * (g - 1.0) / (g + 1.0) * lrt * (lrt - 0.5)
    }

    /// Nucleation at temperature `t`, pressure `p`, wetness `y`, mixture density `rho` (ADR 0217 M4).
    pub fn nucleation(&self, t: f64, p: f64, y: f64, rho: f64) -> Result<Nucleation> {
        let ps = iapws::saturation_pressure_extended(t)?;
        self.nucleation_with(t, p, y, rho, ps, None)
    }

    fn nucleation_with(&self, t: f64, p: f64, y: f64, rho: f64, ps: f64, rl: Option<f64>) -> Result<Nucleation> {
        let s = p / ps;
        let phi = self.kantrowitz_phi(t);
        if s <= 1.0 {
            return Ok(Nucleation {
                rate: 0.0,
                r_crit: f64::INFINITY,
                phi,
                supersaturation: s,
            });
        }
        let rl = match rl {
            Some(v) => v,
            None => liquid_density(t)?,
        };
        let sig = surface_tension(t);
        let r_crit = 2.0 * sig / (rl * R_WATER * t * s.ln());
        let mm = BOLTZMANN / R_WATER;
        let rv = (1.0 - y) * rho;
        let rate = self.q_c / (1.0 + phi) * (2.0 * sig / (PI * mm.powi(3))).sqrt() * rv * rv / (rho * rl)
            * (-4.0 * PI * r_crit * r_crit * sig / (3.0 * BOLTZMANN * t)).exp();
        Ok(Nucleation {
            rate,
            r_crit,
            phi,
            supersaturation: s,
        })
    }

    /// Droplet growth rate `dr/dt` (m/s) of a droplet of radius `r` (ADR 0217 M5).
    pub fn growth_rate(&self, t: f64, p: f64, y: f64, rho: f64, r: f64) -> Result<f64> {
        let ps = iapws::saturation_pressure_extended(t)?;
        let rl = liquid_density(t)?;
        self.growth_with(t, p, y, rho, r, ps, rl)
    }

    #[allow(clippy::too_many_arguments)]
    fn growth_with(&self, t: f64, p: f64, y: f64, rho: f64, r: f64, ps: f64, rl: f64) -> Result<f64> {
        let ls = (p / ps).ln();
        if ls.abs() < 1e-14 {
            return Ok(0.0);
        }
        let r_crit = 2.0 * surface_tension(t) / (rl * R_WATER * t * ls);
        let ts = iapws::saturation_temperature(p)?;
        let l = self.latent(ts);
        let rv = (1.0 - y) * rho;
        let mu = iapws::viscosity(t, rv);
        let lam = iapws::conductivity(t, rv);
        let pr = mu * self.cp_v / lam;
        let lbar = 1.5 * mu * (R_WATER * t).sqrt() / p;
        let kn = lbar / (2.0 * r);
        let g = self.gamma_v();
        let nu = R_WATER * ts / l
            * (self.alpha - 0.5 - (2.0 - self.q_c) / (2.0 * self.q_c) * ((g + 1.0) / (2.0 * g)) * (self.cp_v * ts / l));
        Ok(lam / (rl * l * r) * (ts - t) * (1.0 - r_crit / r) / (1.0 + 3.78 * (1.0 - nu) * kn / pr))
    }

    /// Monodisperse droplet radius `(3 y / (4 pi rho_l(T) n))^(1/3)` (m); zero without liquid or droplets.
    pub fn droplet_radius(&self, y: f64, n: f64, t: f64) -> Result<f64> {
        if !(y > 0.0 && n > 0.0) {
            return Ok(0.0);
        }
        Ok((3.0 * y / (4.0 * PI * liquid_density(t)? * n)).cbrt())
    }

    /// Source rates per unit mass at mixture density `rho`, specific internal energy `e`, wetness `y` and droplet number `n`
    /// (ADR 0217 M6).
    pub fn rates(&self, rho: f64, e: f64, y: f64, n: f64) -> Result<Rates> {
        let t = self.temperature(e, y);
        let p = (1.0 - y) * rho * R_WATER * t;
        let ps = iapws::saturation_pressure_extended(t)?;
        let has_drops = y > 0.0 && n > 0.0;
        let rl = if p > ps || has_drops {
            Some(liquid_density(t)?)
        } else {
            None
        };
        let nuc = self.nucleation_with(t, p, y, rho, ps, rl)?;
        let mut dy = 0.0;
        if nuc.rate > 0.0 {
            dy += nuc.rate * 4.0 / 3.0 * PI * nuc.r_crit.powi(3) * rl.unwrap_or(0.0);
        }
        if has_drops {
            let rl = rl.unwrap_or(0.0);
            let r = (3.0 * y / (4.0 * PI * rl * n)).cbrt();
            dy += n * 4.0 * PI * r * r * rl * self.growth_with(t, p, y, rho, r, ps, rl)?;
        }
        Ok(Rates {
            dy_dt: dy,
            dn_dt: nuc.rate,
            temperature: t,
            pressure: p,
        })
    }
}

/// Boundary treatment of a patch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WetBc {
    /// Slip wall / symmetry plane (mirror ghost state).
    Slip,
    /// Zero-gradient outflow (supersonic outflow).
    Extrapolate,
    /// Fixed state (Riemann problem against it).
    Fixed(WetPrim),
    /// Subsonic dry inflow from stagnation conditions: the face state of [`compressible_bc::total_inlet`] with the vapour's
    /// `gamma_v`, `y = n = 0`.
    TotalInlet(TotalInlet),
}

/// Solver settings.
#[derive(Debug, Clone, Copy)]
pub struct WetSettings {
    /// Slope limiter; `None` gives first order.
    pub limiter: Option<TvdLimiter>,
    /// Time integrator.
    pub rk: RkScheme,
    /// CFL number.
    pub cfl: f64,
    /// Nucleation and droplet growth on (off: frozen wetness, a passive mixture).
    pub condensation: bool,
    /// Point-implicit wetness increment (ADR 0217 W4b).
    pub point_implicit: bool,
}

impl Default for WetSettings {
    fn default() -> Self {
        Self {
            limiter: Some(TvdLimiter::VanLeer),
            rk: RkScheme::Rk3,
            cfl: 0.4,
            condensation: true,
            point_implicit: true,
        }
    }
}

/// Explicit density-based solver for the wet-steam mixture.
#[derive(Debug, Clone)]
pub struct WetSteamSolver {
    /// Mesh geometry.
    pub geom: FvGeometry,
    /// The model.
    pub model: WetSteamModel,
    /// Settings.
    pub settings: WetSettings,
    /// Conserved variables per cell.
    pub u: Vec<WetCons>,
    /// Boundary treatment per mesh patch.
    pub bcs: Vec<WetBc>,
    ls_inv: Vec<[[f64; 3]; 3]>,
    /// Physical time.
    pub time: f64,
    /// Steps taken.
    pub steps: usize,
    /// Number of `rho y` or `rho n` values clipped at zero so far (ADR 0217 W4).
    pub clips: usize,
}

impl WetSteamSolver {
    /// New solver with initial state `init(x)`; patches default to slip walls, outlets to extrapolation.
    pub fn new(
        geom: FvGeometry,
        model: WetSteamModel,
        settings: WetSettings,
        init: impl Fn(Vec3) -> WetPrim,
    ) -> Result<Self> {
        let cc = geom.mesh.cell_centres();
        let mut u = Vec::with_capacity(cc.len());
        for (c, &x) in cc.iter().enumerate() {
            let w = init(x);
            if let Some(why) = model.check(&w) {
                return Err(Error::Numerical(format!("wet steam: initial state of cell {c}: {why}")));
            }
            u.push(model.to_cons(&w));
        }
        let bcs = geom
            .mesh
            .patches()
            .iter()
            .map(|p| match p.kind {
                PatchKind::Outlet => WetBc::Extrapolate,
                _ => WetBc::Slip,
            })
            .collect();
        let ls_inv = ls_inverse(&geom);
        Ok(Self {
            geom,
            model,
            settings,
            u,
            bcs,
            ls_inv,
            time: 0.0,
            steps: 0,
            clips: 0,
        })
    }

    /// Set the boundary treatment of a named patch.
    pub fn set_bc(&mut self, patch: &str, bc: WetBc) -> Result<()> {
        let i = self
            .geom
            .mesh
            .patches()
            .iter()
            .position(|p| p.name == patch)
            .ok_or_else(|| Error::PathNotFound(format!("patch {patch}")))?;
        self.bcs[i] = bc;
        Ok(())
    }

    fn prims_of(&self, u: &[WetCons]) -> Result<Vec<WetPrim>> {
        let mut out = Vec::with_capacity(u.len());
        for (c, cons) in u.iter().enumerate() {
            let w = self.model.to_prim(cons);
            if let Some(why) = self.model.check(&w) {
                return Err(Error::Numerical(format!(
                    "wet steam: cell {c} at step {}: {why}",
                    self.steps
                )));
            }
            out.push(w);
        }
        Ok(out)
    }

    /// Primitive state of every cell (an error names the first inadmissible cell).
    pub fn primitives(&self) -> Result<Vec<WetPrim>> {
        self.prims_of(&self.u)
    }

    /// Monodisperse droplet radius of every cell (m).
    pub fn droplet_radii(&self) -> Result<Vec<f64>> {
        self.primitives()?
            .iter()
            .map(|w| self.model.droplet_radius(w.y, w.n, w.temperature()))
            .collect()
    }

    /// Largest stable explicit step (`cfl` times `2V / sum_f (|u.n| + c)|S|`, frozen sound speed).
    pub fn max_dt(&self) -> Result<f64> {
        let mesh = &self.geom.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let sf = mesh.face_area_vectors();
        let prim = self.primitives()?;
        let mut sum = vec![0.0; mesh.n_cells()];
        for f in 0..mesh.n_faces() {
            if self.geom.empty[f] {
                continue;
            }
            let a = vec3::norm(sf[f]);
            let n = vec3::scale(sf[f], 1.0 / a);
            let lam = |p: &WetPrim| (vec3::dot(p.u, n).abs() + self.model.sound_speed(p)) * a;
            sum[own[f]] += lam(&prim[own[f]]);
            if f < self.geom.n_internal {
                sum[nei[f]] += lam(&prim[nei[f]]);
            }
        }
        let vol = mesh.cell_volumes();
        Ok((0..mesh.n_cells())
            .map(|c| 2.0 * vol[c] / sum[c])
            .fold(f64::INFINITY, f64::min)
            * self.settings.cfl)
    }

    /// Weighted least-squares gradients of the seven primitive variables (the arithmetic of [`crate::compressible`]).
    fn gradients(&self, prim: &[WetPrim], bface: &[WetPrim]) -> Vec<[Vec3; 7]> {
        let mesh = &self.geom.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let mut rhs = vec![[[0.0; 3]; 7]; mesh.n_cells()];
        let mut add = |c: usize, dr: Vec3, dphi: [f64; 7]| {
            let w = 1.0 / vec3::dot(dr, dr);
            for k in 0..7 {
                for i in 0..3 {
                    rhs[c][k][i] += w * dr[i] * dphi[k];
                }
            }
        };
        for f in 0..self.geom.n_internal {
            let (o, nb) = (own[f], nei[f]);
            let d = self.geom.delta[f];
            let dphi: [f64; 7] = std::array::from_fn(|k| var(&prim[nb], k) - var(&prim[o], k));
            add(o, d, dphi);
            add(nb, vec3::scale(d, -1.0), dphi.map(|x| -x));
        }
        for f in self.geom.n_internal..mesh.n_faces() {
            if !self.geom.empty[f] {
                let o = own[f];
                let b = &bface[f - self.geom.n_internal];
                add(
                    o,
                    self.geom.delta[f],
                    std::array::from_fn(|k| var(b, k) - var(&prim[o], k)),
                );
            }
        }
        rhs.iter()
            .zip(&self.ls_inv)
            .map(|(r, inv)| {
                std::array::from_fn(|k| [0, 1, 2].map(|i| (0..3).map(|j| inv[i][j] * r[k][j]).sum::<f64>()))
            })
            .collect()
    }

    /// Undivided flux residual `-sum_f F_f |S_f|` of the state `u`.
    fn flux_sum(&self, u: &[WetCons]) -> Result<Vec<WetCons>> {
        let mesh = &self.geom.mesh;
        let n_cells = mesh.n_cells();
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let sf = mesh.face_area_vectors();
        let n_int = self.geom.n_internal;
        let model = self.model;
        let gas = model.gas();
        let prim = self.prims_of(u)?;
        let patch_of: Vec<usize> = {
            let mut v = vec![usize::MAX; mesh.n_faces()];
            for (pi, p) in mesh.patches().iter().enumerate() {
                for f in p.faces() {
                    v[f] = pi;
                }
            }
            v
        };
        let nb = mesh.n_faces() - n_int;
        let mut ghost = vec![prim[0]; nb];
        let mut bface = vec![prim[0]; nb];
        let mut direct = vec![false; nb];
        for f in n_int..mesh.n_faces() {
            if self.geom.empty[f] {
                continue;
            }
            let n = vec3::unit(sf[f]);
            let pr = &prim[own[f]];
            let (g, b, d) = match self.bcs[patch_of[f]] {
                WetBc::Slip => (
                    pr.mirrored(n),
                    WetPrim {
                        u: vec3::sub(pr.u, vec3::scale(n, vec3::dot(pr.u, n))),
                        ..*pr
                    },
                    false,
                ),
                WetBc::Extrapolate => (*pr, *pr, false),
                WetBc::Fixed(s) => (s, s, false),
                WetBc::TotalInlet(t) => {
                    let fs = WetPrim::dry(&compressible_bc::total_inlet(gas, &t, &pr.flow(), n));
                    (fs, fs, true)
                }
            };
            ghost[f - n_int] = g;
            bface[f - n_int] = b;
            direct[f - n_int] = d;
        }
        let grads: Option<Vec<[Vec3; 7]>> = self.settings.limiter.map(|_| self.gradients(&prim, &bface));
        let limiter = self.settings.limiter;
        let geom = &self.geom;
        let fluxes: Vec<(f64, WetCons)> = (0..mesh.n_faces())
            .into_par_iter()
            .map(|f| {
                if geom.empty[f] {
                    return (0.0, [0.0; 7]);
                }
                let a = vec3::norm(sf[f]);
                let n = vec3::scale(sf[f], 1.0 / a);
                let o = own[f];
                let flux = if f < n_int {
                    let nbr = nei[f];
                    let (mut pl, mut pr) = (prim[o], prim[nbr]);
                    if let (Some(lim), Some(gr)) = (limiter, grads.as_ref()) {
                        let d = geom.delta[f];
                        let mut left = [0.0; 7];
                        let mut right = [0.0; 7];
                        for k in 0..7 {
                            let (vp, vn) = (var(&prim[o], k), var(&prim[nbr], k));
                            let dv = vn - vp;
                            let rp = 2.0 * vec3::dot(gr[o][k], d) / safe(dv) - 1.0;
                            let rn = 2.0 * vec3::dot(gr[nbr][k], vec3::scale(d, -1.0)) / safe(-dv) - 1.0;
                            left[k] = vp + 0.5 * lim.psi(rp) * dv;
                            right[k] = vn - 0.5 * lim.psi(rn) * dv;
                        }
                        let l = from_vars(&left);
                        let r = from_vars(&right);
                        if admissible(&l) && admissible(&r) {
                            pl = l;
                            pr = r;
                        }
                    }
                    hllc(&model, &pl, &pr, n)
                } else if direct[f - n_int] {
                    physical_flux(&model, &ghost[f - n_int], n)
                } else {
                    hllc(&model, &prim[o], &ghost[f - n_int], n)
                };
                (a, flux)
            })
            .collect();
        let mut res = vec![[0.0; 7]; n_cells];
        for (f, (a, flux)) in fluxes.iter().enumerate() {
            if geom.empty[f] {
                continue;
            }
            let o = own[f];
            for k in 0..7 {
                res[o][k] -= flux[k] * a;
                if f < n_int {
                    res[nei[f]][k] += flux[k] * a;
                }
            }
        }
        Ok(res)
    }

    /// Per-cell source `(rho dy/dt, rho dn/dt)` and the point-implicit slope `lambda <= 0` of the `rho y` source (ADR 0217
    /// W4b; zero when off or without droplets).
    fn sources(&self, u: &[WetCons]) -> Result<Vec<[f64; 3]>> {
        if !self.settings.condensation {
            return Ok(vec![[0.0; 3]; u.len()]);
        }
        let model = self.model;
        let pi = self.settings.point_implicit;
        let steps = self.steps;
        u.par_iter()
            .enumerate()
            .map(|(c, s)| {
                let rho = s[0];
                let uu = [s[1] / rho, s[2] / rho, s[3] / rho];
                let e = (s[4] - 0.5 * rho * vec3::dot(uu, uu)) / rho;
                let (y, n) = (s[5] / rho, s[6] / rho);
                let at = |y: f64| {
                    model.rates(rho, e, y, n).map_err(|err| {
                        Error::Numerical(format!("wet steam: source of cell {c} at step {steps}: {err}"))
                    })
                };
                let r0 = at(y)?;
                let sy = rho * r0.dy_dt;
                let mut lam = 0.0;
                if pi && y > 0.0 && n > 0.0 {
                    let delta = 1e-6 * s[5].max(1e-9 * rho);
                    let r1 = at((s[5] + delta) / rho)?;
                    lam = ((rho * r1.dy_dt - sy) / delta).min(0.0);
                }
                Ok([sy, rho * r0.dn_dt, lam])
            })
            .collect()
    }

    /// Right-hand side `L(U)` of a forward-Euler sub-step of size `dt` (flux residual per volume plus sources, the `rho y`
    /// component divided by `1 - dt lambda` when point-implicit), and the source per cell.
    fn rhs(&self, u: &[WetCons], dt: f64) -> Result<(Vec<WetCons>, Vec<[f64; 2]>)> {
        let mut res = self.flux_sum(u)?;
        let vol = self.geom.mesh.cell_volumes();
        for (r, v) in res.iter_mut().zip(vol) {
            for x in r.iter_mut() {
                *x /= v;
            }
        }
        let src = self.sources(u)?;
        let mut applied = vec![[0.0; 2]; u.len()];
        if self.settings.condensation {
            for c in 0..u.len() {
                res[c][5] += src[c][0];
                res[c][6] += src[c][1];
                applied[c] = [src[c][0], src[c][1]];
                if src[c][2] < 0.0 {
                    res[c][5] /= 1.0 - dt * src[c][2];
                }
            }
        }
        Ok((res, applied))
    }

    fn clip(&mut self, u: &mut [WetCons]) {
        for c in u.iter_mut() {
            for k in [5, 6] {
                if c[k] < 0.0 {
                    c[k] = 0.0;
                    self.clips += 1;
                }
            }
        }
    }

    /// Advance one step of size `dt`. Returns the source integral of the step, `dt sum_c V_c sum_s b_s S_s` for `rho y` and
    /// `rho n` (ADR 0217 W4).
    pub fn step(&mut self, dt: f64) -> Result<[f64; 2]> {
        let n = self.u.len();
        let axpy = |a: f64, x: &[WetCons], b: f64, y: &[WetCons], c: f64, r: &[WetCons]| -> Vec<WetCons> {
            (0..n)
                .map(|i| std::array::from_fn(|k| a * x[i][k] + b * y[i][k] + c * r[i][k]))
                .collect()
        };
        let vol = self.geom.mesh.cell_volumes().to_vec();
        let integral = |s: &[[f64; 2]], w: f64| -> [f64; 2] {
            let mut t = [0.0; 2];
            for (c, v) in s.iter().zip(&vol) {
                t[0] += w * dt * c[0] * v;
                t[1] += w * dt * c[1] * v;
            }
            t
        };
        let u0 = self.u.clone();
        let mut total = [0.0; 2];
        let mut acc = |t: [f64; 2]| {
            total[0] += t[0];
            total[1] += t[1];
        };
        match self.settings.rk {
            RkScheme::Euler => {
                let (r, s) = self.rhs(&u0, dt)?;
                acc(integral(&s, 1.0));
                let mut u1 = axpy(1.0, &u0, 0.0, &u0, dt, &r);
                self.clip(&mut u1);
                self.u = u1;
            }
            RkScheme::Rk2 => {
                let (r0, s0) = self.rhs(&u0, dt)?;
                acc(integral(&s0, 0.5));
                let mut u1 = axpy(1.0, &u0, 0.0, &u0, dt, &r0);
                self.clip(&mut u1);
                let (r1, s1) = self.rhs(&u1, dt)?;
                acc(integral(&s1, 0.5));
                let mut u2 = axpy(0.5, &u0, 0.5, &u1, 0.5 * dt, &r1);
                self.clip(&mut u2);
                self.u = u2;
            }
            RkScheme::Rk3 => {
                let (r0, s0) = self.rhs(&u0, dt)?;
                acc(integral(&s0, 1.0 / 6.0));
                let mut u1 = axpy(1.0, &u0, 0.0, &u0, dt, &r0);
                self.clip(&mut u1);
                let (r1, s1) = self.rhs(&u1, dt)?;
                acc(integral(&s1, 1.0 / 6.0));
                let mut u2 = axpy(0.75, &u0, 0.25, &u1, 0.25 * dt, &r1);
                self.clip(&mut u2);
                let (r2, s2) = self.rhs(&u2, dt)?;
                acc(integral(&s2, 2.0 / 3.0));
                let mut u3 = axpy(1.0 / 3.0, &u0, 2.0 / 3.0, &u2, 2.0 / 3.0 * dt, &r2);
                self.clip(&mut u3);
                self.u = u3;
            }
        }
        self.time += dt;
        self.steps += 1;
        Ok(total)
    }

    /// Advance with steps of [`Self::max_dt`] until the largest relative density change in a step falls below `tol` or
    /// `max_steps` is reached. Returns `(steps, final_change)`.
    pub fn run_steady(&mut self, tol: f64, max_steps: usize) -> Result<(usize, f64)> {
        let mut change = f64::INFINITY;
        for i in 0..max_steps {
            let before: Vec<f64> = self.u.iter().map(|c| c[0]).collect();
            let dt = self.max_dt()?;
            self.step(dt)?;
            change = self
                .u
                .iter()
                .zip(&before)
                .map(|(c, b)| (c[0] - b).abs() / b)
                .fold(0.0, f64::max);
            if change < tol {
                return Ok((i + 1, change));
            }
        }
        Ok((max_steps, change))
    }

    /// Total conserved quantities `sum_c U_c V_c`.
    pub fn totals(&self) -> WetCons {
        let vol = self.geom.mesh.cell_volumes();
        let mut t = [0.0; 7];
        for (c, v) in self.u.iter().zip(vol) {
            for k in 0..7 {
                t[k] += c[k] * v;
            }
        }
        t
    }
}

fn var(p: &WetPrim, k: usize) -> f64 {
    match k {
        0 => p.rho,
        1 => p.u[0],
        2 => p.u[1],
        3 => p.u[2],
        4 => p.p,
        5 => p.y,
        _ => p.n,
    }
}

fn from_vars(v: &[f64; 7]) -> WetPrim {
    WetPrim {
        rho: v[0],
        u: [v[1], v[2], v[3]],
        p: v[4],
        y: v[5],
        n: v[6],
    }
}

fn admissible(w: &WetPrim) -> bool {
    w.rho > 0.0 && w.p > 0.0 && w.y >= 0.0 && w.y < Y_MAX && w.n >= 0.0
}

/// Physical flux of a state through the unit normal `n` (the arithmetic of [`Prim::flux`] for the flow part).
pub fn physical_flux(model: &WetSteamModel, w: &WetPrim, n: Vec3) -> WetCons {
    let g = model.gamma_mix(w.y);
    let un = vec3::dot(w.u, n);
    let e = w.p / (g - 1.0) - w.rho * model.offset(w.y) + 0.5 * w.rho * vec3::dot(w.u, w.u);
    [
        w.rho * un,
        w.rho * un * w.u[0] + w.p * n[0],
        w.rho * un * w.u[1] + w.p * n[1],
        w.rho * un * w.u[2] + w.p * n[2],
        (e + w.p) * un,
        w.rho * un * w.y,
        w.rho * un * w.n,
    ]
}

/// HLLC flux for the wet-steam mixture (ADR 0217 W2): the wave speeds of [`crate::riemann`]'s HLLC with the frozen sound
/// speed of each side and a Roe-averaged `Gamma`; the general star-state energy; the scalars carried with the mass. With
/// `y = 0` on both sides this is the perfect-gas HLLC with `gamma = gamma_v`, operation for operation.
pub fn hllc(model: &WetSteamModel, l: &WetPrim, r: &WetPrim, n: Vec3) -> WetCons {
    let (gl, gr) = (model.gamma_mix(l.y), model.gamma_mix(r.y));
    let (unl, unr) = (vec3::dot(l.u, n), vec3::dot(r.u, n));
    let (cl, cr) = ((gl * l.p / l.rho).sqrt(), (gr * r.p / r.rho).sqrt());
    let (sl_, sr_) = (l.rho.sqrt(), r.rho.sqrt());
    let w = 1.0 / (sl_ + sr_);
    let un_roe = (sl_ * unl + sr_ * unr) * w;
    let hl = gl / (gl - 1.0) * l.p / l.rho + 0.5 * vec3::dot(l.u, l.u);
    let hr = gr / (gr - 1.0) * r.p / r.rho + 0.5 * vec3::dot(r.u, r.u);
    let h_roe = (sl_ * hl + sr_ * hr) * w;
    let u_roe = vec3::scale(vec3::add(vec3::scale(l.u, sl_), vec3::scale(r.u, sr_)), w);
    let g_roe = if gl == gr { gl } else { (sl_ * gl + sr_ * gr) * w };
    let c_roe = ((g_roe - 1.0) * (h_roe - 0.5 * vec3::dot(u_roe, u_roe)))
        .max(1e-30)
        .sqrt();
    let s_l = (unl - cl).min(un_roe - c_roe);
    let s_r = (unr + cr).max(un_roe + c_roe);
    let s_star = (r.p - l.p + l.rho * unl * (s_l - unl) - r.rho * unr * (s_r - unr))
        / (l.rho * (s_l - unl) - r.rho * (s_r - unr));
    let fl = physical_flux(model, l, n);
    let fr = physical_flux(model, r, n);
    if s_l >= 0.0 {
        return fl;
    }
    if s_r <= 0.0 {
        return fr;
    }
    let star = |k: &WetPrim, g: f64, un: f64, s: f64| -> WetCons {
        let fac = k.rho * (s - un) / (s - s_star);
        let e_k = k.p / (g - 1.0) - k.rho * model.offset(k.y) + 0.5 * k.rho * vec3::dot(k.u, k.u);
        let mut u = [0.0; 7];
        u[0] = fac;
        for i in 0..3 {
            u[1 + i] = fac * (k.u[i] + (s_star - un) * n[i]);
        }
        u[4] = fac * (e_k / k.rho + (s_star - un) * (s_star + k.p / (k.rho * (s - un))));
        u[5] = fac * k.y;
        u[6] = fac * k.n;
        u
    };
    if s_star >= 0.0 {
        let us = star(l, gl, unl, s_l);
        let ul = model.to_cons(l);
        std::array::from_fn(|k| fl[k] + s_l * (us[k] - ul[k]))
    } else {
        let us = star(r, gr, unr, s_r);
        let ur = model.to_cons(r);
        std::array::from_fn(|k| fr[k] + s_r * (us[k] - ur[k]))
    }
}
