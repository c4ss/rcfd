//! Mixture-fraction approach for non-premixed combustion (roadmap F-129, ADR 0053 D5): unity-Lewis adiabatic mixing of two streams,
//! equilibrium state tables over the mixture fraction `Z`, beta presumed PDF integration (exact for the piecewise-linear table
//! interpolant), the Burke-Schumann closed form, the variance source terms, and the special functions they need.
//!
//! `Z` is the mass fraction of fuel-stream material: `Z = 0` is the oxidiser stream, `Z = 1` the fuel stream. At equal diffusivities
//! the species mass fractions of an inert mixture and the enthalpy are linear in `Z` (derived in the ADR); the chemistry only moves
//! mass between species and so leaves the element mass fractions linear in `Z`.

use crate::equil::{equilibrium, equilibrium_tp, Constraint, Equilibrium};
use crate::mech::Mechanism;
use crate::mech::R_GAS;
use std::f64::consts::PI;

/// Error of the mixture-fraction tools; the text names the offending argument and its value.
#[derive(Debug, Clone, PartialEq)]
pub struct MixError(pub String);

impl std::fmt::Display for MixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for MixError {}

fn err<T>(s: String) -> Result<T, MixError> {
    Err(MixError(s))
}

// ------------------------------------------------------------------------------------------------ special functions

/// `ln Gamma(x)` for `x > 0`: upward recurrence to `x >= 15` and the Stirling series.
pub fn ln_gamma(x: f64) -> f64 {
    assert!(x > 0.0, "ln_gamma needs x > 0");
    let mut x = x;
    let mut acc = 0.0;
    while x < 15.0 {
        acc += x.ln();
        x += 1.0;
    }
    let ix = 1.0 / x;
    let ix2 = ix * ix;
    let series = ix * (1.0 / 12.0 - ix2 * (1.0 / 360.0 - ix2 * (1.0 / 1260.0 - ix2 * (1.0 / 1680.0 - ix2 / 1188.0))));
    (x - 0.5) * x.ln() - x + 0.5 * (2.0 * PI).ln() + series - acc
}

fn betacf(x: f64, a: f64, b: f64) -> f64 {
    const FPMIN: f64 = 1e-300;
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < FPMIN {
        d = FPMIN;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..200_000 {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = 1.0 + aa / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = 1.0 + aa / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 3e-16 {
            break;
        }
    }
    h
}

/// Regularised incomplete beta function `I_x(a, b)` (`a, b > 0`), continued fraction by the modified Lentz method.
pub fn reg_inc_beta(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let lbeta = ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b);
    let bt = (lbeta + a * x.ln() + b * (-x).ln_1p()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        bt * betacf(x, a, b) / a
    } else {
        1.0 - bt * betacf(1.0 - x, b, a) / b
    }
}

/// Error function: positive-term series `erf x = 2/sqrt(pi) e^{-x^2} sum 2^n x^{2n+1}/(1 3 ... (2n+1))` for `|x| < 2`, else `1 - erfc`.
pub fn erf(x: f64) -> f64 {
    if x < 0.0 {
        return -erf(-x);
    }
    if x < 2.0 {
        let mut term = x;
        let mut sum = x;
        let mut n = 0.0;
        while term.abs() > 1e-18 * sum.abs() {
            n += 1.0;
            term *= 2.0 * x * x / (2.0 * n + 1.0);
            sum += term;
        }
        2.0 / PI.sqrt() * (-x * x).exp() * sum
    } else {
        1.0 - erfc(x)
    }
}

/// Complementary error function: `1 - erf x` for `x < 2`, the Laplace continued fraction (modified Lentz) for `x >= 2`.
pub fn erfc(x: f64) -> f64 {
    if x < 2.0 {
        return 1.0 - erf(x);
    }
    // erfc x = e^{-x^2}/sqrt(pi) * 1/(x + (1/2)/(x + 1/(x + (3/2)/(x + ...))))
    const FPMIN: f64 = 1e-300;
    let mut f = x;
    let mut c = x;
    let mut d = 0.0;
    for n in 1..2000 {
        let a = 0.5 * n as f64;
        d = x + a * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = x + a / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        let del = c * d;
        f *= del;
        if (del - 1.0).abs() < 1e-16 {
            break;
        }
    }
    (-x * x).exp() / PI.sqrt() / f
}

/// Inverse of `erfc` on `(0, 2)`: bisection to a bracket, then Newton polishing.
pub fn erfc_inv(y: f64) -> f64 {
    assert!(y > 0.0 && y < 2.0, "erfc_inv needs 0 < y < 2");
    let (mut lo, mut hi) = (-6.0_f64, 27.0_f64); // erfc decreasing
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if erfc(mid) > y {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let mut x = 0.5 * (lo + hi);
    for _ in 0..4 {
        let d = -2.0 / PI.sqrt() * (-x * x).exp();
        if d == 0.0 {
            break;
        }
        x -= (erfc(x) - y) / d;
    }
    x
}

// ------------------------------------------------------------------------------------------------ tables and the beta PDF

fn check_grid(z: &[f64]) -> Result<(), MixError> {
    if z.len() < 2 {
        return err(format!("z grid has {} points, at least 2 are needed", z.len()));
    }
    for i in 1..z.len() {
        if !(z[i] > z[i - 1]) {
            return err(format!(
                "z grid is not strictly increasing at index {i}: z[{}] = {} then z[{i}] = {}",
                i - 1,
                z[i - 1],
                z[i]
            ));
        }
    }
    Ok(())
}

/// Piecewise-linear interpolation of the table `(z, f)` at `x` (clamped at the ends).
pub fn interp(z: &[f64], f: &[f64], x: f64) -> f64 {
    let n = z.len();
    if x <= z[0] {
        return f[0];
    }
    if x >= z[n - 1] {
        return f[n - 1];
    }
    let j = match z.binary_search_by(|v| v.partial_cmp(&x).unwrap()) {
        Ok(j) => return f[j],
        Err(j) => j - 1,
    };
    f[j] + (f[j + 1] - f[j]) * (x - z[j]) / (z[j + 1] - z[j])
}

/// Validate the beta PDF arguments (mean `zm`, variance `v`).
pub fn check_pdf(zm: f64, v: f64) -> Result<(), MixError> {
    if !(0.0..=1.0).contains(&zm) {
        return err(format!("mean mixture fraction zm = {zm} is outside [0, 1]"));
    }
    if !(v >= 0.0) {
        return err(format!("variance v = {v} is negative"));
    }
    if v > zm * (1.0 - zm) * (1.0 + 1e-12) {
        return err(format!(
            "variance v = {v:e} exceeds the maximum zm (1 - zm) = {:e} for zm = {zm}",
            zm * (1.0 - zm)
        ));
    }
    Ok(())
}

/// Parameters `(a, b)` of the beta PDF of mean `zm` and variance `v` (`0 < v < zm (1 - zm)`).
pub fn beta_params(zm: f64, v: f64) -> (f64, f64) {
    let gamma = zm * (1.0 - zm) / v - 1.0;
    (zm * gamma, (1.0 - zm) * gamma)
}

/// Mean of the piecewise-linear interpolant of `(z, f)` under the beta PDF of mean `zm` and variance `v`, integrated exactly with the
/// regularised incomplete beta function (ADR 0053 D5). `v = 0` (or `zm` at an end) is the table lookup; the maximum variance is the
/// two-point limit `(1 - zm) f(z_0) + zm f(z_n)`.
pub fn integrate_beta(z: &[f64], f: &[f64], zm: f64, v: f64) -> Result<f64, MixError> {
    check_grid(z)?;
    if f.len() != z.len() {
        return err(format!(
            "table length mismatch: z has {} entries, f has {}",
            z.len(),
            f.len()
        ));
    }
    check_pdf(zm, v)?;
    let n = z.len();
    if v == 0.0 || zm == 0.0 || zm == 1.0 {
        return Ok(interp(z, f, zm));
    }
    let vmax = zm * (1.0 - zm);
    if v >= vmax * (1.0 - 1e-15) {
        return Ok((1.0 - zm) * f[0] + zm * f[n - 1]);
    }
    let (a, b) = beta_params(zm, v);
    let i0: Vec<f64> = z.iter().map(|&x| reg_inc_beta(x, a, b)).collect();
    let i1: Vec<f64> = z.iter().map(|&x| reg_inc_beta(x, a + 1.0, b)).collect();
    let mut sum = 0.0;
    for j in 0..n - 1 {
        let w0 = i0[j + 1] - i0[j];
        let w1 = zm * (i1[j + 1] - i1[j]);
        let slope = (f[j + 1] - f[j]) / (z[j + 1] - z[j]);
        sum += f[j] * w0 + slope * (w1 - z[j] * w0);
    }
    // probability mass outside the table range (only when the table does not span [0, 1]) sits at the end values
    if z[0] > 0.0 {
        sum += f[0] * i0[0];
    }
    if z[n - 1] < 1.0 {
        sum += f[n - 1] * (1.0 - i0[n - 1]);
    }
    Ok(sum)
}

// ------------------------------------------------------------------------------------------------ streams

/// A feed stream: mass fractions and temperature.
#[derive(Debug, Clone, PartialEq)]
pub struct Stream {
    /// Mass fractions.
    pub y: Vec<f64>,
    /// Temperature, K.
    pub t: f64,
}

impl Stream {
    /// Stream from `(species, amount)` mole pairs (normalised). Unknown names and an all-zero composition are errors naming the argument.
    pub fn from_moles(mech: &Mechanism, pairs: &[(&str, f64)], t: f64) -> Result<Stream, MixError> {
        let mut x = vec![0.0; mech.species.len()];
        for (name, v) in pairs {
            let Some(k) = mech.species_index(name) else {
                return err(format!("stream species '{name}' is not in the mechanism"));
            };
            if !(*v >= 0.0) {
                return err(format!("stream amount of '{name}' = {v} is negative"));
            }
            x[k] += v;
        }
        if !(x.iter().sum::<f64>() > 0.0) {
            return err("stream composition sums to zero (all amounts are 0)".into());
        }
        if !(t > 0.0) {
            return err(format!("stream temperature t = {t} must be positive"));
        }
        Ok(Stream { y: mech.x_to_y(&x), t })
    }

    /// Specific enthalpy, J/kg.
    pub fn h(&self, mech: &Mechanism) -> f64 {
        mech.h_mass(self.t, &self.y)
    }
}

/// Temperature from the specific enthalpy `h` (J/kg) at mass fractions `y` by Newton on the NASA polynomials.
pub fn temperature_from_h(mech: &Mechanism, h: f64, y: &[f64], t_guess: f64) -> Result<f64, MixError> {
    let mut t = t_guess.clamp(200.0, 5500.0);
    for _ in 0..100 {
        let f = mech.h_mass(t, y) - h;
        let cp = mech.cp_mass(t, y);
        let dt = (f / cp).clamp(-500.0, 500.0);
        t -= dt;
        t = t.clamp(150.0, 6500.0);
        if dt.abs() < 1e-10 * t.max(1.0) {
            return Ok(t);
        }
    }
    err(format!(
        "temperature from enthalpy h = {h:e} J/kg did not converge (last t = {t})"
    ))
}

/// Unity-Lewis mixture of the two streams at mixture fraction `z`: mass fractions and enthalpy.
pub fn mix_streams(mech: &Mechanism, fuel: &Stream, oxid: &Stream, z: f64) -> (Vec<f64>, f64) {
    let y: Vec<f64> = fuel.y.iter().zip(&oxid.y).map(|(f, o)| z * f + (1.0 - z) * o).collect();
    (y, z * fuel.h(mech) + (1.0 - z) * oxid.h(mech))
}

/// Stoichiometric mixture fraction of the two streams by the Bilger element balance `beta = 2 n_C + n_H/2 - n_O` (moles of atoms
/// per kg): `beta(Z_st) = 0`. Needs the elements C, H, O in the mechanism (missing ones count as absent).
pub fn stoichiometric_z(mech: &Mechanism, fuel: &Stream, oxid: &Stream) -> Result<f64, MixError> {
    let coef: Vec<f64> = mech
        .elements
        .iter()
        .map(|e| match e.to_ascii_uppercase().as_str() {
            "C" => 2.0,
            "H" => 0.5,
            "O" => -1.0,
            _ => 0.0,
        })
        .collect();
    let beta = |y: &[f64]| -> f64 {
        mech.element_mass_fractions(y)
            .iter()
            .zip(&coef)
            .map(|(n, c)| n * c)
            .sum()
    };
    let (bf, bo) = (beta(&fuel.y), beta(&oxid.y));
    if bf == bo {
        return err("the streams have equal Bilger beta: no stoichiometric mixture fraction".into());
    }
    let z = -bo / (bf - bo);
    if !(z > 0.0 && z < 1.0) {
        return err(format!("stoichiometric mixture fraction z_st = {z} is outside (0, 1)"));
    }
    Ok(z)
}

/// A state table over the mixture fraction: temperature and mass fractions at the nodes of a `Z` grid.
#[derive(Debug, Clone, PartialEq)]
pub struct MixtureTable {
    /// Mixture-fraction nodes, strictly increasing.
    pub z: Vec<f64>,
    /// Temperature at the nodes.
    pub t: Vec<f64>,
    /// Mass fractions: `y[k][node]`.
    pub y: Vec<Vec<f64>>,
}

impl MixtureTable {
    /// Table from per-node states (checks the grid and the row lengths).
    pub fn new(z: Vec<f64>, t: Vec<f64>, y: Vec<Vec<f64>>) -> Result<MixtureTable, MixError> {
        check_grid(&z)?;
        if t.len() != z.len() {
            return err(format!(
                "table row count mismatch: z has {} nodes, t has {}",
                z.len(),
                t.len()
            ));
        }
        for (k, row) in y.iter().enumerate() {
            if row.len() != z.len() {
                return err(format!(
                    "table row count mismatch: z has {} nodes, species {k} has {}",
                    z.len(),
                    row.len()
                ));
            }
        }
        Ok(MixtureTable { z, t, y })
    }

    /// Linear lookup at `z`: `(T, Y)`.
    pub fn lookup(&self, z: f64) -> (f64, Vec<f64>) {
        (
            interp(&self.z, &self.t, z),
            self.y.iter().map(|row| interp(&self.z, row, z)).collect(),
        )
    }

    /// Beta-PDF mean of `(T, Y)` for the mixture fraction mean `zm` and variance `v`.
    pub fn mean(&self, zm: f64, v: f64) -> Result<(f64, Vec<f64>), MixError> {
        let t = integrate_beta(&self.z, &self.t, zm, v)?;
        let mut y = Vec::with_capacity(self.y.len());
        for row in &self.y {
            y.push(integrate_beta(&self.z, row, zm, v)?);
        }
        Ok((t, y))
    }

    /// Mass fractions of node `i` as a vector over species.
    pub fn y_at(&self, i: usize) -> Vec<f64> {
        self.y.iter().map(|row| row[i]).collect()
    }
}

fn enthalpy_of(mech: &Mechanism, n: &[f64], t: f64) -> f64 {
    mech.species.iter().zip(n).map(|(s, n)| n * R_GAS * t * s.h_rt(t)).sum()
}

/// HP equilibrium; the library routine first. Its temperature bracket starts at 250 K, where a nearly complete exothermic reaction
/// (equilibrium constant above `e^100`) cannot be solved, so on failure the root is bracketed by stepping down from 6000 K and found by
/// bisection on the TP equilibria (ADR 0053 addendum).
fn hp_equilibrium(mech: &Mechanism, x0: &[f64], t0: f64, p: f64) -> Result<Equilibrium, MixError> {
    if let Ok(eq) = equilibrium(mech, x0, t0, p, Constraint::HP) {
        return Ok(eq);
    }
    let h0 = enthalpy_of(mech, x0, t0);
    let scale = h0.abs() + 3.5 * R_GAS * t0;
    let f = |t: f64| -> Result<f64, MixError> {
        let eq = equilibrium_tp(mech, x0, t, p).map_err(|e| MixError(format!("equilibrium at T = {t}: {e}")))?;
        Ok((enthalpy_of(mech, &eq.n, t) - h0) / scale)
    };
    let mut hi = 6000.0_f64;
    if f(hi)? <= 0.0 {
        return err("HP equilibrium: the enthalpy residual is not positive at 6000 K".into());
    }
    let mut lo = hi;
    loop {
        lo *= 0.9;
        if lo < 250.0 {
            return err("HP equilibrium: no temperature with a negative enthalpy residual found above 250 K".into());
        }
        // below the root the library cannot always solve the TP equilibrium of a nearly complete reaction; h_eq(T) - h0 increases
        // with T, so a failure below the lowest tried temperature is read as "below the root" and ends the scan
        match f(lo) {
            Ok(v) if v >= 0.0 => hi = lo,
            _ => break,
        }
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        match f(mid) {
            Ok(v) if v >= 0.0 => hi = mid,
            _ => lo = mid,
        }
        if hi - lo < 1e-11 * hi {
            break;
        }
    }
    let tr = 0.5 * (lo + hi);
    let res = f(tr)?;
    if res.abs() > 1e-9 {
        return err(format!(
            "HP equilibrium: the enthalpy root was not reached (T = {tr}, scaled residual {res:e}); the TP equilibria below it did not converge"
        ));
    }
    equilibrium_tp(mech, x0, tr, p).map_err(|e| MixError(format!("equilibrium: {e}")))
}

/// Equilibrium (HP) table over the given `Z` grid for unity-Lewis adiabatic mixing of the two streams at pressure `p`. The end nodes
/// `Z = 0` and `Z = 1` are the pure streams (they do not react: frozen mixing at the ends).
pub fn equilibrium_table(
    mech: &Mechanism,
    fuel: &Stream,
    oxid: &Stream,
    p: f64,
    zgrid: &[f64],
) -> Result<MixtureTable, MixError> {
    check_grid(zgrid)?;
    if fuel.y.len() != mech.species.len() || oxid.y.len() != mech.species.len() {
        return err(format!(
            "stream length mismatch: mechanism has {} species, fuel {} oxidiser {}",
            mech.species.len(),
            fuel.y.len(),
            oxid.y.len()
        ));
    }
    let ns = mech.species.len();
    let mut t = Vec::with_capacity(zgrid.len());
    let mut y = vec![Vec::with_capacity(zgrid.len()); ns];
    let mut tg = 0.5 * (fuel.t + oxid.t);
    for &z in zgrid {
        if !(0.0..=1.0).contains(&z) {
            return err(format!("z grid value z = {z} is outside [0, 1]"));
        }
        let (ym, h) = if z == 0.0 {
            (oxid.y.clone(), oxid.h(mech))
        } else if z == 1.0 {
            (fuel.y.clone(), fuel.h(mech))
        } else {
            mix_streams(mech, fuel, oxid, z)
        };
        let (tt, yy) = if z == 0.0 {
            (oxid.t, oxid.y.clone())
        } else if z == 1.0 {
            (fuel.t, fuel.y.clone())
        } else {
            let tf = temperature_from_h(mech, h, &ym, tg)?;
            let x0 = mech.y_to_x(&ym);
            let eq = hp_equilibrium(mech, &x0, tf, p)
                .map_err(|e| MixError(format!("equilibrium at z = {z} failed: {e}")))?;
            (eq.t, eq.y)
        };
        tg = tt;
        t.push(tt);
        for (k, row) in y.iter_mut().enumerate() {
            row.push(yy[k]);
        }
    }
    MixtureTable::new(zgrid.to_vec(), t, y)
}

// ------------------------------------------------------------------------------------------------ Burke-Schumann

/// Burke-Schumann solution of the single irreversible step `F + s O -> products` (mass stoichiometric ratio `s`) with constant specific
/// heat: infinitely fast chemistry, so fuel and oxidiser never coexist.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BurkeSchumann {
    /// Mass of oxidiser per mass of fuel.
    pub s: f64,
    /// Fuel mass fraction of the fuel stream.
    pub yf1: f64,
    /// Oxidiser mass fraction of the oxidiser stream.
    pub yo2: f64,
    /// Constant specific heat, J/(kg K).
    pub cp: f64,
    /// Heat of reaction, J per kg of fuel.
    pub q: f64,
    /// Oxidiser-stream temperature, K.
    pub t_o: f64,
    /// Fuel-stream temperature, K.
    pub t_f: f64,
}

impl BurkeSchumann {
    /// Stoichiometric mixture fraction `Z_st = 1/(1 + s Y_F1/Y_O2)`.
    pub fn z_st(&self) -> f64 {
        1.0 / (1.0 + self.s * self.yf1 / self.yo2)
    }

    /// Fuel mass fraction.
    pub fn y_f(&self, z: f64) -> f64 {
        self.yf1 * ((z - self.z_st()) / (1.0 - self.z_st())).max(0.0)
    }

    /// Oxidiser mass fraction.
    pub fn y_o(&self, z: f64) -> f64 {
        self.yo2 * ((self.z_st() - z) / self.z_st()).max(0.0)
    }

    /// Product mass fraction: fuel-limited `(1 + s) Y_F1 Z` when lean, oxidiser-limited `(1 + 1/s) Y_O2 (1 - Z)` when rich.
    pub fn y_p(&self, z: f64) -> f64 {
        if z <= self.z_st() {
            (1.0 + self.s) * self.yf1 * z
        } else {
            (1.0 + 1.0 / self.s) * self.yo2 * (1.0 - z)
        }
    }

    /// Temperature: piecewise linear with one kink at `Z_st`.
    pub fn t(&self, z: f64) -> f64 {
        let zs = self.z_st();
        let base = self.t_o + z * (self.t_f - self.t_o);
        if z <= zs {
            base + self.q * self.yf1 * z / self.cp
        } else {
            base + self.q * self.yf1 * zs * (1.0 - z) / ((1.0 - zs) * self.cp)
        }
    }

    /// Temperature at the stoichiometric point.
    pub fn t_st(&self) -> f64 {
        self.t(self.z_st())
    }
}

// ------------------------------------------------------------------------------------------------ variance sources

/// Production of the mixture-fraction variance by the mean gradient: `2 (mu_t/Sc_t) |grad Z|^2` (kg/m3/s per unit variance rate).
pub fn variance_production(mu_t: f64, sc_t: f64, grad_z2: f64) -> f64 {
    2.0 * mu_t / sc_t * grad_z2
}

/// Dissipation of the variance: `C_chi rho (eps/k) Z''^2`.
pub fn variance_dissipation(c_chi: f64, rho: f64, eps: f64, k: f64, var: f64) -> f64 {
    c_chi * rho * eps / k * var
}

/// Scalar dissipation rate of the mean field in terms of the variance model: `chi = C_chi (eps/k) Z''^2`.
pub fn scalar_dissipation(c_chi: f64, eps: f64, k: f64, var: f64) -> f64 {
    c_chi * eps / k * var
}
