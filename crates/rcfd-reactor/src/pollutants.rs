//! Thermal NO from the extended Zeldovich mechanism (ADR 0027 D5, roadmap F-222): post-processing on prescribed fields.
//!
//! Reactions `O + N2 <=> NO + N` (1) and `N + O2 <=> NO + O` (2). With N in steady state,
//! `d[NO]/dt = 2 (k1f k2f [O][N2][O2] - k1r k2r [NO]^2 [O]) / (k1r [NO] + k2f [O2])` (derivation in ADR 0027 K10).
//! The rate constants are inputs; no literature set is bundled because none could be verified from a non-vendor
//! source in this wave (named gap). No prompt-NO, fuel-NO or N2O paths.

/// Modified-Arrhenius rate constant `k = A T^b exp(-Ta / T)` in SI units (m3/mol/s).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arrhenius {
    /// Pre-exponential factor.
    pub a: f64,
    /// Temperature exponent.
    pub b: f64,
    /// Activation temperature, K.
    pub ta: f64,
}

impl Arrhenius {
    /// Value at temperature `t` (K).
    pub fn at(&self, t: f64) -> f64 {
        self.a * t.powf(self.b) * (-self.ta / t).exp()
    }
}

/// The four rate constants of reactions 1 and 2.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZeldovichRates {
    /// `O + N2 -> NO + N`.
    pub k1f: Arrhenius,
    /// `NO + N -> O + N2`.
    pub k1r: Arrhenius,
    /// `N + O2 -> NO + O`.
    pub k2f: Arrhenius,
    /// `NO + O -> N + O2`.
    pub k2r: Arrhenius,
}

/// Rate constants at one temperature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZeldovichK {
    /// k1f.
    pub k1f: f64,
    /// k1r.
    pub k1r: f64,
    /// k2f.
    pub k2f: f64,
    /// k2r.
    pub k2r: f64,
}

impl ZeldovichRates {
    /// Evaluate at `t` (K).
    pub fn at(&self, t: f64) -> ZeldovichK {
        ZeldovichK {
            k1f: self.k1f.at(t),
            k1r: self.k1r.at(t),
            k2f: self.k2f.at(t),
            k2r: self.k2r.at(t),
        }
    }
}

/// Steady-state N concentration (mol/m3) for concentrations `o`, `n2`, `o2`, `no` (mol/m3).
pub fn n_steady(k: &ZeldovichK, o: f64, n2: f64, o2: f64, no: f64) -> f64 {
    (k.k1f * o * n2 + k.k2r * no * o) / (k.k1r * no + k.k2f * o2)
}

/// Thermal NO production rate `d[NO]/dt` (mol/m3/s) with N in steady state.
pub fn thermal_no_rate(k: &ZeldovichK, o: f64, n2: f64, o2: f64, no: f64) -> f64 {
    let den = k.k1r * no + k.k2f * o2;
    if !(den > 0.0) {
        return 0.0;
    }
    2.0 * (k.k1f * k.k2f * o * n2 * o2 - k.k1r * k.k2r * no * no * o) / den
}

/// NO concentration at which the rate vanishes (detailed balance of the two reactions): `k1f k2f [O2][N2] = k1r k2r [NO]^2`.
pub fn no_balance(k: &ZeldovichK, n2: f64, o2: f64) -> f64 {
    (k.k1f * k.k2f * o2 * n2 / (k.k1r * k.k2r)).sqrt()
}
