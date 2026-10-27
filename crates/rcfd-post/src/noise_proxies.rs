//! Steady-RANS broadband noise SOURCE PROXIES (ADR 0066).
//!
//! **These are not predictive of absolute sound levels.** They map quantities a steady RANS solution can supply
//! (k, epsilon, wall shear, surface pressure statistics modelled from them) onto the scaling laws of three source
//! mechanisms. Their constants are either taken from secondary sources (Proudman alpha) or left to the caller (Curle
//! correlation area and rate, trailing-edge constant). They are only meaningful for ranking designs against each
//! other at fixed assumptions, and even that needs validation (see `docs/acoustics.md`). Tests check identities
//! and scaling exponents only.

/// Error of the proxy functions; messages name the offending argument.
#[derive(Debug, Clone, PartialEq)]
pub struct ProxyError(pub String);

impl std::fmt::Display for ProxyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ProxyError {}

fn pos(name: &str, v: f64) -> Result<(), ProxyError> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        Err(ProxyError(format!("{name}: must be finite and positive (got {v})")))
    }
}

fn nonneg(name: &str, v: f64) -> Result<(), ProxyError> {
    if v.is_finite() && v >= 0.0 {
        Ok(())
    } else {
        Err(ProxyError(format!("{name}: must be finite and >= 0 (got {v})")))
    }
}

/// Proudman constant, attributed to Sarkar and Hussaini's DNS calibration (UNVERIFIED: secondary sources only, the original
/// papers were not read; see docs/provenance.md).
pub const PROUDMAN_ALPHA: f64 = 0.1;
/// Reference acoustic power density for the dB value, W/m^3.
pub const POWER_DENSITY_REF: f64 = 1e-12;

/// Proudman acoustic power per unit volume of isotropic turbulence, `alpha rho0 eps M_t^5`, `M_t = sqrt(2k)/c0`.
/// Valid in the derivation for high Reynolds number, low Mach number, isotropic turbulence, no mean flow.
pub fn proudman_power_density(k: f64, eps: f64, rho0: f64, c0: f64, alpha: f64) -> Result<f64, ProxyError> {
    nonneg("k", k)?;
    nonneg("eps", eps)?;
    pos("rho0", rho0)?;
    pos("c0", c0)?;
    nonneg("alpha", alpha)?;
    let mt = (2.0 * k).sqrt() / c0;
    Ok(alpha * rho0 * eps * mt.powi(5))
}

/// Level of a power density in dB re 1e-12 W/m^3; `None` for zero.
pub fn power_density_level_db(p: f64) -> Option<f64> {
    if p > 0.0 && p.is_finite() {
        Some(10.0 * (p / POWER_DENSITY_REF).log10())
    } else {
        None
    }
}

/// Total Proudman power of cells `(k, eps, volume)`.
pub fn proudman_total_power(cells: &[(f64, f64, f64)], rho0: f64, c0: f64, alpha: f64) -> Result<f64, ProxyError> {
    let mut w = 0.0;
    for (i, &(k, eps, vol)) in cells.iter().enumerate() {
        nonneg(&format!("cells[{i}].volume"), vol)?;
        w +=
            vol * proudman_power_density(k, eps, rho0, c0, alpha).map_err(|e| ProxyError(format!("cells[{i}].{e}")))?;
    }
    Ok(w)
}

/// Velocity and length scale of a k-epsilon pair, `u = sqrt(2k/3)`, `l = k^{3/2}/eps` (dimensional estimates).
pub fn turbulence_scales(k: f64, eps: f64) -> Result<(f64, f64), ProxyError> {
    nonneg("k", k)?;
    pos("eps", eps)?;
    Ok(((2.0 * k / 3.0).sqrt(), k.powf(1.5) / eps))
}

/// One surface patch of the Curle-type dipole proxy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DipoleFace {
    /// Area, m^2.
    pub area: f64,
    /// Unit normal (direction of the force the pressure exerts on the fluid).
    pub normal: [f64; 3],
    /// RMS of the time derivative of the surface pressure, Pa/s (`2 pi f_c p'_rms` for a narrowband estimate).
    pub dpdt_rms: f64,
    /// Correlation area of the pressure fluctuation, m^2 (patches are statistically independent).
    pub corr_area: f64,
}

/// RMS pressure rate from an rms pressure and a characteristic frequency, `2 pi f_c p'_rms`.
pub fn dpdt_from_frequency(p_rms: f64, f_c: f64) -> Result<f64, ProxyError> {
    nonneg("p_rms", p_rms)?;
    nonneg("f_c", f_c)?;
    Ok(2.0 * std::f64::consts::PI * f_c * p_rms)
}

fn check_faces(faces: &[DipoleFace]) -> Result<(), ProxyError> {
    for (i, f) in faces.iter().enumerate() {
        pos(&format!("faces[{i}].area"), f.area)?;
        pos(&format!("faces[{i}].corr_area"), f.corr_area)?;
        nonneg(&format!("faces[{i}].dpdt_rms"), f.dpdt_rms)?;
        let n2 = f.normal.iter().map(|c| c * c).sum::<f64>();
        if !n2.is_finite() || (n2.sqrt() - 1.0).abs() > 1e-6 {
            return Err(ProxyError(format!(
                "faces[{i}].normal: must be a unit vector (norm {})",
                n2.sqrt()
            )));
        }
    }
    Ok(())
}

/// Curle-type dipole power, `W = sum <p_dot^2> A_c S / (12 pi rho0 c0^3)` (independent patches of correlation area
/// `A_c`; derivation in `docs/acoustics.md`).
pub fn curle_power(faces: &[DipoleFace], rho0: f64, c0: f64) -> Result<f64, ProxyError> {
    pos("rho0", rho0)?;
    pos("c0", c0)?;
    check_faces(faces)?;
    let s: f64 = faces
        .iter()
        .map(|f| f.dpdt_rms * f.dpdt_rms * f.corr_area * f.area)
        .sum();
    Ok(s / (12.0 * std::f64::consts::PI * rho0 * c0.powi(3)))
}

/// Far-field mean-square pressure of the Curle proxy at distance `r` in the unit direction `rhat`:
/// `sum <p_dot^2> A_c S (n . rhat)^2 / (16 pi^2 c0^2 r^2)`.
pub fn curle_far_field_ms(faces: &[DipoleFace], c0: f64, r: f64, rhat: [f64; 3]) -> Result<f64, ProxyError> {
    pos("c0", c0)?;
    pos("r", r)?;
    let n2 = rhat.iter().map(|c| c * c).sum::<f64>();
    if !n2.is_finite() || (n2.sqrt() - 1.0).abs() > 1e-6 {
        return Err(ProxyError(format!("rhat: must be a unit vector (norm {})", n2.sqrt())));
    }
    check_faces(faces)?;
    let s: f64 = faces
        .iter()
        .map(|f| {
            let nr = f.normal[0] * rhat[0] + f.normal[1] * rhat[1] + f.normal[2] * rhat[2];
            f.dpdt_rms * f.dpdt_rms * f.corr_area * f.area * nr * nr
        })
        .sum();
    Ok(s / (16.0 * std::f64::consts::PI.powi(2) * c0 * c0 * r * r))
}

/// Trailing-edge scaling, `W = K rho0 u_t^5 L_e l_t / c0^2` (power ~ U^5, Ffowcs Williams and Hall 1970 exponent from a
/// secondary source; the constant `k_te` is uncalibrated and defaults to 1 so that only ratios are meaningful).
pub fn trailing_edge_power(k_te: f64, rho0: f64, u_t: f64, l_edge: f64, l_t: f64, c0: f64) -> Result<f64, ProxyError> {
    nonneg("k_te", k_te)?;
    pos("rho0", rho0)?;
    nonneg("u_t", u_t)?;
    nonneg("l_edge", l_edge)?;
    nonneg("l_t", l_t)?;
    pos("c0", c0)?;
    Ok(k_te * rho0 * u_t.powi(5) * l_edge * l_t / (c0 * c0))
}
