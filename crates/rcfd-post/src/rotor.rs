//! Rotor tonal noise: Gutin-type steady-loading harmonics of a rotating compact force (closed form) and the Bessel
//! functions it needs (ADR 0066, derivation in `docs/acoustics.md`).
//!
//! The formula is derived from the far field of a moving point force, not copied from Gutin (1936) or Lowson (1965),
//! whose papers were not read. It is an independent check of the time-domain FW-H code, not a design tool by itself.

/// Error of the rotor module; messages name the offending argument.
#[derive(Debug, Clone, PartialEq)]
pub struct RotorError(pub String);

impl std::fmt::Display for RotorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RotorError {}

/// Bessel function of the first kind `J_n(x)` for integer `n >= 0` and `0 <= x <= 1000`, by Miller's backward
/// recurrence normalised with `J_0 + 2 sum J_{2k} = 1` (stable for `n > x`, where forward recurrence is not).
pub fn bessel_j(n: usize, x: f64) -> Result<f64, RotorError> {
    if !x.is_finite() || !(0.0..=1000.0).contains(&x) {
        return Err(RotorError(format!("x: must be finite and in [0, 1000] (got {x})")));
    }
    if n > 100_000 {
        return Err(RotorError(format!("n: must be <= 100000 (got {n})")));
    }
    if x == 0.0 {
        return Ok(if n == 0 { 1.0 } else { 0.0 });
    }
    let start = n.max(x.ceil() as usize) + 40 + (x.sqrt() as usize) * 4;
    let start = start + (start % 2); // even
    let (mut jp1, mut j) = (0.0_f64, 1e-300_f64);
    let mut sum = 0.0;
    let mut result = 0.0;
    for k in (1..=start).rev() {
        // j = J_k (unnormalised), jp1 = J_{k+1}; compute J_{k-1}
        let jm1 = 2.0 * k as f64 / x * j - jp1;
        jp1 = j;
        j = jm1;
        // now j = J_{k-1}, jp1 = J_k
        if k - 1 == n {
            result = j;
        }
        if (k - 1) % 2 == 0 && k - 1 > 0 {
            sum += 2.0 * j;
        }
        if j.abs() > 1e250 {
            jp1 *= 1e-250;
            j *= 1e-250;
            sum *= 1e-250;
            result *= 1e-250;
        }
    }
    // j = J_0 here
    let norm = j + sum;
    Ok(result / norm)
}

/// Geometry and loading of a rotor for the Gutin-type formula. Conventions (ADR 0066): `theta` is measured from the
/// rotor axis towards the direction in which the thrust force acts on the fluid; `thrust` is the total axial force on
/// the fluid, `torque` the magnitude of the total torque, i.e. the total tangential force on the fluid (in the direction
/// of rotation) times `r_tip`; the loading is a compact force acting at radius `r_tip`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rotor {
    /// Number of equally spaced blades.
    pub blades: usize,
    /// Angular velocity, rad/s.
    pub omega: f64,
    /// Radius of the effective force, m.
    pub r_tip: f64,
    /// Total thrust (force on the fluid along the axis), N.
    pub thrust: f64,
    /// Total torque, N m.
    pub torque: f64,
    /// Speed of sound, m/s.
    pub c0: f64,
}

impl Rotor {
    fn check(&self) -> Result<(), RotorError> {
        let bad = |m: String| Err(RotorError(m));
        if self.blades == 0 {
            return bad("blades: must be >= 1".into());
        }
        for (name, v) in [("omega", self.omega), ("r_tip", self.r_tip), ("c0", self.c0)] {
            if !(v.is_finite() && v > 0.0) {
                return bad(format!("{name}: must be finite and positive (got {v})"));
            }
        }
        for (name, v) in [("thrust", self.thrust), ("torque", self.torque)] {
            if !v.is_finite() {
                return bad(format!("{name}: must be finite (got {v})"));
            }
        }
        if self.omega * self.r_tip >= self.c0 {
            return bad(format!(
                "omega, r_tip: tip speed {} is not subsonic (c0 = {})",
                self.omega * self.r_tip,
                self.c0
            ));
        }
        Ok(())
    }

    /// Tip Mach number `omega r_tip / c0`.
    pub fn tip_mach(&self) -> f64 {
        self.omega * self.r_tip / self.c0
    }

    /// Peak pressure amplitude (Pa) of harmonic `m >= 1` (frequency `m B Omega`) at distance `r` and polar angle
    /// `theta` (rad), far field and no flow:
    /// `|p_m| = (m B Omega/(2 pi c r)) |J_{mB}(m B M_t sin theta)| |T cos theta + Q c/(Omega R_t^2)|`.
    /// The rms value is this divided by `sqrt 2`.
    pub fn harmonic_amplitude(&self, m: usize, r: f64, theta: f64) -> Result<f64, RotorError> {
        Ok(self.harmonic_pressure(m, r, theta)?.abs())
    }

    /// Signed form of [`Rotor::harmonic_amplitude`], `(m B Omega/(2 pi c r)) J_{mB}(...) (T cos theta + Q c/(Omega R_t^2))`:
    /// the contributions of rings at different radii that share one blade azimuth add with these signs (ADR 0119).
    pub fn harmonic_pressure(&self, m: usize, r: f64, theta: f64) -> Result<f64, RotorError> {
        self.check()?;
        if m == 0 {
            return Err(RotorError("m: harmonic number must be >= 1".into()));
        }
        if !(r.is_finite() && r > 0.0) {
            return Err(RotorError(format!("r: must be finite and positive (got {r})")));
        }
        if !theta.is_finite() {
            return Err(RotorError(format!("theta: must be finite (got {theta})")));
        }
        let n = m * self.blades;
        let z = n as f64 * self.tip_mach() * theta.sin().abs();
        let j = bessel_j(n, z)?;
        let load = self.thrust * theta.cos() + self.torque * self.c0 / (self.omega * self.r_tip * self.r_tip);
        Ok(n as f64 * self.omega / (2.0 * std::f64::consts::PI * self.c0 * r) * j * load)
    }

    /// Frequency of harmonic `m`, Hz.
    pub fn harmonic_frequency(&self, m: usize) -> f64 {
        (m * self.blades) as f64 * self.omega / (2.0 * std::f64::consts::PI)
    }
}
