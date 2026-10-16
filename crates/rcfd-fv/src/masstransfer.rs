//! Interfacial mass-transfer closures (F-226, ADR 0040): evaporation-condensation (temperature driven Lee form, kinetic
//! Hertz-Knudsen-Schrage, pressure driven bubble-dynamics form), species equilibrium transfer (Raoult, Henry with van 't Hoff
//! temperature dependence) and Nusselt-film wall condensation.
//!
//! Library only: the multiphase host (F-055, F-127) does not exist yet. Every closure is a per-cell evaluator that takes phase
//! fractions, temperatures and pressures as inputs. Sign convention: `mdot` (kg/(m^3 s)) is positive for evaporation
//! (liquid to vapour); a [`PhaseSource`] carries `liquid = -mdot`, `vapour = +mdot`, `energy = -mdot h_fg`.

/// Universal gas constant, J/(mol K) (exact since 2019).
pub const R_UNIVERSAL: f64 = 8.314_462_618;

/// Result alias: errors are text naming the offending parameter.
pub type Result<T> = std::result::Result<T, String>;

/// Phase-change sources of one cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseSource {
    /// Liquid mass source, kg/(m^3 s).
    pub liquid: f64,
    /// Vapour mass source, kg/(m^3 s).
    pub vapour: f64,
    /// Source in the mixture enthalpy equation, W/m^3 (`-mdot h_fg`).
    pub energy: f64,
}

impl PhaseSource {
    /// Build the sources from the evaporation rate `mdot` (kg/(m^3 s), positive evaporation) and the latent heat `h_fg` (J/kg).
    pub fn from_rate(mdot: f64, h_fg: f64) -> Self {
        Self {
            liquid: -mdot,
            vapour: mdot,
            energy: -mdot * h_fg,
        }
    }
    /// The evaporation rate this source encodes.
    pub fn rate(&self) -> f64 {
        self.vapour
    }
}

fn finite_pos(name: &str, v: f64) -> Result<()> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        Err(format!("{name} must be finite and > 0, got {v}"))
    }
}

fn finite_nonneg(name: &str, v: f64) -> Result<()> {
    if v.is_finite() && v >= 0.0 {
        Ok(())
    } else {
        Err(format!("{name} must be finite and >= 0, got {v}"))
    }
}

fn fraction(name: &str, v: f64) -> Result<()> {
    if v.is_finite() && (0.0..=1.0).contains(&v) {
        Ok(())
    } else {
        Err(format!("{name} must be in [0, 1], got {v}"))
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Temperature driven (Lee form)
// ---------------------------------------------------------------------------------------------------------------------

/// Temperature-driven evaporation-condensation (Lee form).
#[derive(Debug, Clone, Copy)]
pub struct LeeModel {
    /// Evaporation relaxation frequency, 1/s.
    pub c_evap: f64,
    /// Condensation relaxation frequency, 1/s.
    pub c_cond: f64,
}

impl LeeModel {
    /// Evaporation rate (kg/(m^3 s), negative = condensation) for the liquid fraction `alpha_l`, liquid and vapour densities,
    /// vapour fraction `alpha_v`, cell temperature `t` and saturation temperature `t_sat`.
    pub fn rate(&self, alpha_l: f64, alpha_v: f64, rho_l: f64, rho_v: f64, t: f64, t_sat: f64) -> Result<f64> {
        finite_nonneg("c_evap", self.c_evap)?;
        finite_nonneg("c_cond", self.c_cond)?;
        fraction("alpha_l", alpha_l)?;
        fraction("alpha_v", alpha_v)?;
        finite_pos("rho_l", rho_l)?;
        finite_pos("rho_v", rho_v)?;
        finite_pos("t_sat", t_sat)?;
        if !t.is_finite() {
            return Err(format!("t must be finite, got {t}"));
        }
        let theta = (t - t_sat) / t_sat;
        Ok(if t > t_sat {
            self.c_evap * alpha_l * rho_l * theta
        } else if t < t_sat {
            self.c_cond * alpha_v * rho_v * theta
        } else {
            0.0
        })
    }

    /// Phase and energy sources.
    #[allow(clippy::too_many_arguments)]
    pub fn source(
        &self,
        alpha_l: f64,
        alpha_v: f64,
        rho_l: f64,
        rho_v: f64,
        t: f64,
        t_sat: f64,
        h_fg: f64,
    ) -> Result<PhaseSource> {
        finite_pos("h_fg", h_fg)?;
        Ok(PhaseSource::from_rate(
            self.rate(alpha_l, alpha_v, rho_l, rho_v, t, t_sat)?,
            h_fg,
        ))
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Hertz-Knudsen-Schrage
// ---------------------------------------------------------------------------------------------------------------------

/// Kinetic (Hertz-Knudsen, optionally Schrage-corrected) interfacial mass flux.
#[derive(Debug, Clone, Copy)]
pub struct HertzKnudsen {
    /// Accommodation coefficient, in (0, 1].
    pub sigma: f64,
    /// Molar mass, kg/mol.
    pub molar_mass: f64,
    /// Apply the Schrage factor `2 sigma/(2 - sigma)` instead of `sigma`.
    pub schrage: bool,
}

impl HertzKnudsen {
    /// Interfacial flux, kg/(m^2 s), positive for evaporation. `p_sat_l` is the saturation pressure at the liquid-side
    /// interface temperature `t_l`; `p_v`, `t_v` the vapour-side pressure and temperature.
    pub fn flux(&self, p_sat_l: f64, t_l: f64, p_v: f64, t_v: f64) -> Result<f64> {
        if !(self.sigma.is_finite() && self.sigma > 0.0 && self.sigma <= 1.0) {
            return Err(format!("sigma must be in (0, 1], got {}", self.sigma));
        }
        finite_pos("molar_mass", self.molar_mass)?;
        finite_pos("p_sat_l", p_sat_l)?;
        finite_nonneg("p_v", p_v)?;
        finite_pos("t_l", t_l)?;
        finite_pos("t_v", t_v)?;
        let f = if self.schrage {
            2.0 * self.sigma / (2.0 - self.sigma)
        } else {
            self.sigma
        };
        Ok(
            f * (self.molar_mass / (2.0 * std::f64::consts::PI * R_UNIVERSAL)).sqrt()
                * (p_sat_l / t_l.sqrt() - p_v / t_v.sqrt()),
        )
    }

    /// Volumetric rate `j a_i` with the interfacial area density `a_i` (1/m).
    pub fn rate(&self, p_sat_l: f64, t_l: f64, p_v: f64, t_v: f64, area_density: f64) -> Result<f64> {
        finite_nonneg("area_density", area_density)?;
        Ok(self.flux(p_sat_l, t_l, p_v, t_v)? * area_density)
    }

    /// Phase and energy sources.
    pub fn source(
        &self,
        p_sat_l: f64,
        t_l: f64,
        p_v: f64,
        t_v: f64,
        area_density: f64,
        h_fg: f64,
    ) -> Result<PhaseSource> {
        finite_pos("h_fg", h_fg)?;
        Ok(PhaseSource::from_rate(
            self.rate(p_sat_l, t_l, p_v, t_v, area_density)?,
            h_fg,
        ))
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Pressure driven (bubble dynamics)
// ---------------------------------------------------------------------------------------------------------------------

/// Pressure-driven (cavitation-type) mass transfer derived from the Rayleigh-Plesset growth rate.
#[derive(Debug, Clone, Copy)]
pub struct PressureDriven {
    /// Bubble radius, m.
    pub r_bubble: f64,
    /// Nucleation-site volume fraction.
    pub alpha_nuc: f64,
    /// Vaporisation factor.
    pub f_vap: f64,
    /// Condensation factor.
    pub f_cond: f64,
}

impl Default for PressureDriven {
    fn default() -> Self {
        Self {
            r_bubble: 1e-6,
            alpha_nuc: 5e-4,
            f_vap: 50.0,
            f_cond: 0.01,
        }
    }
}

impl PressureDriven {
    /// Bubble-wall speed `sqrt((2/3)|p_v - p|/rho_l)` (m/s, magnitude).
    pub fn wall_speed(p_v: f64, p: f64, rho_l: f64) -> f64 {
        (2.0 / 3.0 * (p_v - p).abs() / rho_l).sqrt()
    }

    /// Evaporation rate (kg/(m^3 s), negative = condensation). `p_v` is the vapour (saturation) pressure.
    pub fn rate(&self, alpha_v: f64, rho_l: f64, rho_v: f64, p: f64, p_v: f64) -> Result<f64> {
        finite_pos("r_bubble", self.r_bubble)?;
        fraction("alpha_nuc", self.alpha_nuc)?;
        finite_nonneg("f_vap", self.f_vap)?;
        finite_nonneg("f_cond", self.f_cond)?;
        fraction("alpha_v", alpha_v)?;
        finite_pos("rho_l", rho_l)?;
        finite_pos("rho_v", rho_v)?;
        if !(p.is_finite() && p_v.is_finite()) {
            return Err(format!("p and p_v must be finite, got {p} and {p_v}"));
        }
        let speed = Self::wall_speed(p_v, p, rho_l);
        Ok(if p < p_v {
            self.f_vap * 3.0 * self.alpha_nuc * (1.0 - alpha_v) * rho_v / self.r_bubble * speed
        } else if p > p_v {
            -self.f_cond * 3.0 * alpha_v * rho_v / self.r_bubble * speed
        } else {
            0.0
        })
    }

    /// Phase and energy sources.
    pub fn source(&self, alpha_v: f64, rho_l: f64, rho_v: f64, p: f64, p_v: f64, h_fg: f64) -> Result<PhaseSource> {
        finite_pos("h_fg", h_fg)?;
        Ok(PhaseSource::from_rate(self.rate(alpha_v, rho_l, rho_v, p, p_v)?, h_fg))
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Raoult and Henry
// ---------------------------------------------------------------------------------------------------------------------

fn check_composition(name: &str, x: &[f64]) -> Result<()> {
    if x.is_empty() {
        return Err(format!("{name} is empty"));
    }
    let mut sum = 0.0;
    for (i, &v) in x.iter().enumerate() {
        if !(v.is_finite() && v >= 0.0) {
            return Err(format!("{name}[{i}] must be finite and >= 0, got {v}"));
        }
        sum += v;
    }
    if (sum - 1.0).abs() > 1e-9 {
        return Err(format!(
            "{name}: sum of fractions must be 1 (within 1e-9), got sum {sum}"
        ));
    }
    Ok(())
}

fn check_psat(psat: &[f64], n: usize) -> Result<()> {
    if psat.len() != n {
        return Err(format!("length mismatch: fractions {n}, psat {}", psat.len()));
    }
    for (i, &v) in psat.iter().enumerate() {
        if !(v.is_finite() && v > 0.0) {
            return Err(format!("psat[{i}] must be finite and > 0, got {v}"));
        }
    }
    Ok(())
}

/// Bubble-point pressure of an ideal liquid mixture, `sum x_i psat_i`.
pub fn bubble_pressure(x: &[f64], psat: &[f64]) -> Result<f64> {
    check_composition("x", x)?;
    check_psat(psat, x.len())?;
    Ok(x.iter().zip(psat).map(|(a, b)| a * b).sum())
}

/// Dew-point pressure of an ideal vapour mixture, `1/sum(y_i/psat_i)`.
pub fn dew_pressure(y: &[f64], psat: &[f64]) -> Result<f64> {
    check_composition("y", y)?;
    check_psat(psat, y.len())?;
    Ok(1.0 / y.iter().zip(psat).map(|(a, b)| a / b).sum::<f64>())
}

/// Raoult's law vapour mole fractions `y_i = x_i psat_i/p` (not normalised: the sum is 1 at the bubble pressure, above 1 below it).
pub fn raoult_vapour_fractions(x: &[f64], psat: &[f64], p: f64) -> Result<Vec<f64>> {
    check_composition("x", x)?;
    check_psat(psat, x.len())?;
    finite_pos("p", p)?;
    Ok(x.iter().zip(psat).map(|(a, b)| a * b / p).collect())
}

/// Equilibrium K-values `psat_i/p`.
pub fn k_values(psat: &[f64], p: f64) -> Result<Vec<f64>> {
    check_psat(psat, psat.len())?;
    finite_pos("p", p)?;
    Ok(psat.iter().map(|v| v / p).collect())
}

/// Henry's law constant (volatility form `p_i = H x_i`, Pa) with van 't Hoff temperature dependence.
#[derive(Debug, Clone, Copy)]
pub struct Henry {
    /// `H` at the reference temperature, Pa.
    pub h0: f64,
    /// Reference temperature, K.
    pub t0: f64,
    /// `C = -Delta_sol H/R_u`, K (positive: `H` increases with temperature).
    pub c: f64,
}

impl Henry {
    /// `H(T) = H_0 exp(-C (1/T - 1/T_0))`.
    pub fn at(&self, t: f64) -> Result<f64> {
        finite_pos("h0", self.h0)?;
        finite_pos("t0", self.t0)?;
        if !self.c.is_finite() {
            return Err(format!("c must be finite, got {}", self.c));
        }
        finite_pos("t", t)?;
        if t == self.t0 {
            return Ok(self.h0);
        }
        Ok(self.h0 * (-self.c * (1.0 / t - 1.0 / self.t0)).exp())
    }

    /// Equilibrium liquid mole fraction `x = y p/H(T)` of the dissolved gas.
    pub fn liquid_fraction(&self, y: f64, p: f64, t: f64) -> Result<f64> {
        fraction("y", y)?;
        finite_pos("p", p)?;
        Ok(y * p / self.at(t)?)
    }

    /// Equilibrium gas mole fraction `y = H(T) x/p`.
    pub fn gas_fraction(&self, x: f64, p: f64, t: f64) -> Result<f64> {
        fraction("x", x)?;
        finite_pos("p", p)?;
        Ok(self.at(t)? * x / p)
    }
}

/// Mass fractions from mole fractions and molar masses.
pub fn mole_to_mass_fractions(x: &[f64], molar_mass: &[f64]) -> Result<Vec<f64>> {
    if x.len() != molar_mass.len() {
        return Err(format!(
            "length mismatch: fractions {}, molar_mass {}",
            x.len(),
            molar_mass.len()
        ));
    }
    check_composition("x", x)?;
    for (i, &m) in molar_mass.iter().enumerate() {
        finite_pos(&format!("molar_mass[{i}]"), m)?;
    }
    let mix: f64 = x.iter().zip(molar_mass).map(|(a, b)| a * b).sum();
    Ok(x.iter().zip(molar_mass).map(|(a, b)| a * b / mix).collect())
}

/// Mole fractions from mass fractions and molar masses.
pub fn mass_to_mole_fractions(w: &[f64], molar_mass: &[f64]) -> Result<Vec<f64>> {
    if w.len() != molar_mass.len() {
        return Err(format!(
            "length mismatch: fractions {}, molar_mass {}",
            w.len(),
            molar_mass.len()
        ));
    }
    check_composition("w", w)?;
    for (i, &m) in molar_mass.iter().enumerate() {
        finite_pos(&format!("molar_mass[{i}]"), m)?;
    }
    let mix: f64 = w.iter().zip(molar_mass).map(|(a, b)| a / b).sum();
    Ok(w.iter().zip(molar_mass).map(|(a, b)| a / b / mix).collect())
}

/// Species transfer rate into the receiving phase, `k a_i rho_r (w_eq - w)` (kg/(m^3 s)); the donor source is its negative.
pub fn species_transfer_rate(k: f64, area_density: f64, rho_receiver: f64, w_eq: f64, w: f64) -> Result<f64> {
    finite_nonneg("k", k)?;
    finite_nonneg("area_density", area_density)?;
    finite_pos("rho_receiver", rho_receiver)?;
    fraction("w_eq", w_eq)?;
    fraction("w", w)?;
    Ok(k * area_density * rho_receiver * (w_eq - w))
}

// ---------------------------------------------------------------------------------------------------------------------
// Nusselt film condensation
// ---------------------------------------------------------------------------------------------------------------------

/// Laminar Nusselt film on a vertical isothermal plate in saturated stagnant vapour.
#[derive(Debug, Clone, Copy)]
pub struct NusseltFilm {
    /// Liquid density, kg/m^3.
    pub rho_l: f64,
    /// Vapour density, kg/m^3.
    pub rho_v: f64,
    /// Liquid dynamic viscosity, Pa s.
    pub mu_l: f64,
    /// Liquid conductivity, W/(m K).
    pub k_l: f64,
    /// Latent heat, J/kg.
    pub h_fg: f64,
    /// Gravity, m/s^2.
    pub g: f64,
    /// Add the Rohsenow correction `h_fg' = h_fg + 0.68 c_p dT` (needs `cp_l`).
    pub rohsenow: bool,
    /// Liquid specific heat, J/(kg K), used only with `rohsenow`.
    pub cp_l: f64,
}

impl NusseltFilm {
    fn check(&self) -> Result<()> {
        finite_pos("rho_l", self.rho_l)?;
        finite_pos("rho_v", self.rho_v)?;
        if self.rho_l <= self.rho_v {
            return Err(format!("rho_l ({}) must exceed rho_v ({})", self.rho_l, self.rho_v));
        }
        finite_pos("mu_l", self.mu_l)?;
        finite_pos("k_l", self.k_l)?;
        finite_pos("h_fg", self.h_fg)?;
        finite_pos("g", self.g)?;
        if self.rohsenow {
            finite_pos("cp_l", self.cp_l)?;
        }
        Ok(())
    }

    fn hfg_eff(&self, d_t: f64) -> f64 {
        if self.rohsenow {
            self.h_fg + 0.68 * self.cp_l * d_t
        } else {
            self.h_fg
        }
    }

    fn subcooling(t_sat: f64, t_wall: f64) -> Result<f64> {
        finite_pos("t_sat", t_sat)?;
        if !t_wall.is_finite() {
            return Err(format!("t_wall must be finite, got {t_wall}"));
        }
        let d = t_sat - t_wall;
        if d <= 0.0 {
            return Err(format!(
                "t_wall ({t_wall}) must be below t_sat ({t_sat}) for condensation"
            ));
        }
        Ok(d)
    }

    /// Film thickness at distance `x` (m) from the top of the plate.
    pub fn thickness(&self, x: f64, t_sat: f64, t_wall: f64) -> Result<f64> {
        self.check()?;
        finite_pos("x", x)?;
        let d = Self::subcooling(t_sat, t_wall)?;
        let hfg = self.hfg_eff(d);
        Ok((4.0 * self.mu_l * self.k_l * d * x / (self.g * self.rho_l * (self.rho_l - self.rho_v) * hfg)).powf(0.25))
    }

    /// Local heat-transfer coefficient `k_l/delta(x)`.
    pub fn local_h(&self, x: f64, t_sat: f64, t_wall: f64) -> Result<f64> {
        Ok(self.k_l / self.thickness(x, t_sat, t_wall)?)
    }

    /// Mean heat-transfer coefficient over `[0, length]`, `(4/3) h(length)`.
    pub fn mean_h(&self, length: f64, t_sat: f64, t_wall: f64) -> Result<f64> {
        Ok(4.0 / 3.0 * self.local_h(length, t_sat, t_wall)?)
    }

    /// Wall condensation mass flux `k_l dT/(delta h_fg)` at `x`, kg/(m^2 s); zero for a wall at or above the saturation temperature.
    pub fn wall_mass_flux(&self, x: f64, t_sat: f64, t_wall: f64) -> Result<f64> {
        self.check()?;
        finite_pos("x", x)?;
        finite_pos("t_sat", t_sat)?;
        if t_wall.is_finite() && t_wall >= t_sat {
            return Ok(0.0);
        }
        let d = Self::subcooling(t_sat, t_wall)?;
        Ok(self.k_l * d / (self.thickness(x, t_sat, t_wall)? * self.hfg_eff(d)))
    }

    /// Condensate mass flow per unit width at `x`, `rho_l (rho_l - rho_v) g delta^3/(3 mu_l)`, kg/(m s).
    pub fn film_mass_flow(&self, x: f64, t_sat: f64, t_wall: f64) -> Result<f64> {
        let delta = self.thickness(x, t_sat, t_wall)?;
        Ok(self.rho_l * (self.rho_l - self.rho_v) * self.g * delta.powi(3) / (3.0 * self.mu_l))
    }

    /// Condensate Reynolds number `4 Gamma/mu_l` (laminar-wavy limit near 30, informational).
    pub fn film_reynolds(&self, x: f64, t_sat: f64, t_wall: f64) -> Result<f64> {
        Ok(4.0 * self.film_mass_flow(x, t_sat, t_wall)? / self.mu_l)
    }
}

/// Wall-cell source from a wall mass flux: the volumetric condensation rate `m'' A/V` is returned as the (negative)
/// evaporation rate in a [`PhaseSource`] (vapour loses, liquid gains, latent heat released into the cell).
pub fn wall_cell_source(mass_flux: f64, wall_area: f64, cell_volume: f64, h_fg: f64) -> Result<PhaseSource> {
    finite_nonneg("mass_flux", mass_flux)?;
    finite_nonneg("wall_area", wall_area)?;
    finite_pos("cell_volume", cell_volume)?;
    finite_pos("h_fg", h_fg)?;
    Ok(PhaseSource::from_rate(-(mass_flux * wall_area / cell_volume), h_fg))
}

// ---------------------------------------------------------------------------------------------------------------------
// 0-D relaxation cell
// ---------------------------------------------------------------------------------------------------------------------

/// State of the 0-D evaporating cell (per unit volume).
#[derive(Debug, Clone, Copy)]
pub struct LeeCell {
    /// Liquid mass, kg/m^3.
    pub m_liquid: f64,
    /// Vapour mass, kg/m^3.
    pub m_vapour: f64,
    /// Temperature, K.
    pub t: f64,
}

/// Parameters of the 0-D relaxation cell.
#[derive(Debug, Clone, Copy)]
pub struct LeeCellParams {
    /// Evaporation frequency, 1/s (also used for condensation of the vapour mass when `T < T_sat`).
    pub c: f64,
    /// Saturation temperature, K.
    pub t_sat: f64,
    /// Volumetric heat capacity of the cell, J/(m^3 K).
    pub heat_capacity: f64,
    /// Latent heat, J/kg.
    pub h_fg: f64,
    /// Freeze the participating mass (linear ODE with an analytic solution).
    pub frozen: bool,
}

impl LeeCellParams {
    /// The linear relaxation rate `lambda = h_fg c m/(C T_sat)` for the participating mass `m`.
    pub fn lambda(&self, m: f64) -> f64 {
        self.h_fg * self.c * m / (self.heat_capacity * self.t_sat)
    }

    fn deriv(&self, s: &LeeCell, m_frozen: f64) -> [f64; 3] {
        let theta = (s.t - self.t_sat) / self.t_sat;
        let m = if self.frozen {
            m_frozen
        } else if theta > 0.0 {
            s.m_liquid
        } else {
            s.m_vapour
        };
        let mdot = self.c * m * theta;
        [-mdot, mdot, -self.h_fg * mdot / self.heat_capacity]
    }

    /// Advance by `n` classical RK4 steps of size `dt`.
    pub fn integrate(&self, s0: LeeCell, dt: f64, n: usize) -> Result<LeeCell> {
        finite_pos("c", self.c)?;
        finite_pos("t_sat", self.t_sat)?;
        finite_pos("heat_capacity", self.heat_capacity)?;
        finite_pos("h_fg", self.h_fg)?;
        finite_pos("dt", dt)?;
        finite_nonneg("m_liquid", s0.m_liquid)?;
        finite_nonneg("m_vapour", s0.m_vapour)?;
        finite_pos("t", s0.t)?;
        let theta0 = s0.t - self.t_sat;
        let m_frozen = if theta0 >= 0.0 { s0.m_liquid } else { s0.m_vapour };
        let mut s = s0;
        let add = |a: &LeeCell, k: &[f64; 3], f: f64| LeeCell {
            m_liquid: a.m_liquid + f * k[0],
            m_vapour: a.m_vapour + f * k[1],
            t: a.t + f * k[2],
        };
        for _ in 0..n {
            let k1 = self.deriv(&s, m_frozen);
            let k2 = self.deriv(&add(&s, &k1, 0.5 * dt), m_frozen);
            let k3 = self.deriv(&add(&s, &k2, 0.5 * dt), m_frozen);
            let k4 = self.deriv(&add(&s, &k3, dt), m_frozen);
            let k = [
                (k1[0] + 2.0 * k2[0] + 2.0 * k3[0] + k4[0]) / 6.0,
                (k1[1] + 2.0 * k2[1] + 2.0 * k3[1] + k4[1]) / 6.0,
                (k1[2] + 2.0 * k2[2] + 2.0 * k3[2] + k4[2]) / 6.0,
            ];
            s = add(&s, &k, dt);
        }
        Ok(s)
    }

    /// Analytic temperature of the frozen-mass problem after time `t_end` (participating mass `m`).
    pub fn analytic_frozen(&self, t0: f64, m: f64, t_end: f64) -> f64 {
        self.t_sat + (t0 - self.t_sat) * (-self.lambda(m) * t_end).exp()
    }
}
