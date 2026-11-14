//! High-cycle fatigue: stress-life curves, mean-stress corrections, counting, damage. Declared by ADR 0440 (F-408, the scaffold
//! of ADR 0311); filled by ADR 0449 (F-413, stub ADR 0316).
//!
//! * Curves (D1): [`SnCurve`], Basquin `sigma_a = sigma_f (2 N)^b` (so `N = (sigma_a/sigma_f)^(1/b)/2`), a power law
//!   `N = c S^-m`, or a table interpolated in log-log, each with an optional endurance limit.
//! * Mean stress (D2): [`MeanStressCorrection`] of a [`MeanStress`] kind, Goodman `sigma_a/(1 - sigma_m/sigma_u)`, Gerber
//!   `sigma_a/(1 - (sigma_m/sigma_u)^2)`, Soderberg `sigma_a/(1 - sigma_m/sigma_y)`; a mean at or above the strength is refused;
//!   a compressive mean gets no credit unless asked. [`safety_factor`] on the whole stress against a design life.
//! * Equivalent stresses (D2): [`harmonic_von_mises_amplitude`] (the closed-form maximum over the phase of the von Mises stress
//!   of `Re((A + i B) e^{i w t})`), [`harmonic_max_principal`], [`signed_von_mises`], [`equivalent_history`].
//! * Fields (D3): [`FatigueSpec`], [`PointFatigue`], [`FatigueField`] (life, damage per cycle and per second, safety factor at
//!   every point of a mean and a harmonic stress field).
//! * Counting (D4): [`turning_points`], [`rainflow`] (the four-point closing of hysteresis loops; residue reported),
//!   [`rainflow_periodic`] (rotated to the turning point of largest modulus; no residue), [`miner`] (Palmgren-Miner sum).
//! * Narrow band (D5): [`narrow_band_damage_rate`] `nu0 (sqrt 2 s)^m Gamma(1 + m/2)/c`, [`gamma`], [`ln_gamma`].
//!
//! Voigt order xx, yy, zz, xy, yz, zx with tensor shear (as [`crate::recover`]). Every loop is sequential in a fixed order.

use crate::error::{FemError, Result};
use crate::recover::{principal, von_mises};
use rcfd_core::vec3::Vec3;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

fn invalid(key: &'static str, reason: String) -> FemError {
    FemError::InvalidValue { key, reason }
}

// ------------------------------------------------------------------------------------------------------------- curves

#[derive(Debug, Clone, PartialEq)]
enum Law {
    Basquin { sigma_f: f64, b: f64 },
    Power { m: f64, c: f64 },
    Table { ln_n: Vec<f64>, ln_s: Vec<f64> },
}

/// A stress-life curve: the life in cycles of a fully reversed stress amplitude (ADR 0449 D1).
#[derive(Debug, Clone, PartialEq)]
pub struct SnCurve {
    law: Law,
    endurance: Option<f64>,
}

const CURVE: &str = "stress-life curve";

fn positive(key: &'static str, name: &str, x: f64) -> Result<()> {
    if x.is_finite() && x > 0.0 {
        Ok(())
    } else {
        Err(invalid(key, format!("{name} {x:e} must be finite and positive")))
    }
}

impl SnCurve {
    /// Basquin's curve `sigma_a = sigma_f (2 N)^b`: `sigma_f > 0` the fatigue strength coefficient, `b < 0` the exponent.
    pub fn basquin(sigma_f: f64, b: f64) -> Result<SnCurve> {
        positive(CURVE, "fatigue strength coefficient", sigma_f)?;
        if !(b.is_finite() && b < 0.0) {
            return Err(invalid(
                CURVE,
                format!("Basquin exponent {b:e} must be finite and negative"),
            ));
        }
        Ok(SnCurve {
            law: Law::Basquin { sigma_f, b },
            endurance: None,
        })
    }

    /// The power law `N = c S^-m` (`m > 0`, `c > 0`).
    pub fn power(m: f64, c: f64) -> Result<SnCurve> {
        positive(CURVE, "exponent m", m)?;
        positive(CURVE, "constant c", c)?;
        Ok(SnCurve {
            law: Law::Power { m, c },
            endurance: None,
        })
    }

    /// A table of lives (strictly ascending, positive) against amplitudes (strictly descending, positive), at least two
    /// points; `log N` is linear in `log S` between points and the end segments are extended beyond the table.
    pub fn table(lives: &[f64], amplitudes: &[f64]) -> Result<SnCurve> {
        if lives.len() != amplitudes.len() {
            return Err(FemError::FieldLength {
                key: CURVE,
                expected: lives.len(),
                found: amplitudes.len(),
            });
        }
        if lives.len() < 2 {
            return Err(invalid(
                CURVE,
                format!("a table needs at least two points, got {}", lives.len()),
            ));
        }
        for (i, (&n, &s)) in lives.iter().zip(amplitudes).enumerate() {
            positive(CURVE, "table life", n)?;
            positive(CURVE, "table amplitude", s)?;
            if i > 0 && n <= lives[i - 1] {
                return Err(invalid(
                    CURVE,
                    format!("table lives must ascend strictly (point {i}: {n:e})"),
                ));
            }
            if i > 0 && s >= amplitudes[i - 1] {
                return Err(invalid(
                    CURVE,
                    format!("table amplitudes must descend strictly (point {i}: {s:e})"),
                ));
            }
        }
        Ok(SnCurve {
            law: Law::Table {
                ln_n: lives.iter().map(|x| x.ln()).collect(),
                ln_s: amplitudes.iter().map(|x| x.ln()).collect(),
            },
            endurance: None,
        })
    }

    /// The same curve with an endurance limit: an amplitude at or below `sigma_e` has an infinite life.
    pub fn with_endurance(mut self, sigma_e: f64) -> Result<SnCurve> {
        positive(CURVE, "endurance limit", sigma_e)?;
        self.endurance = Some(sigma_e);
        Ok(self)
    }

    /// The endurance limit, if any.
    pub fn endurance(&self) -> Option<f64> {
        self.endurance
    }

    /// The life in cycles of a fully reversed amplitude `s >= 0`; `f64::INFINITY` at zero or at or below the endurance limit.
    pub fn life(&self, s: f64) -> Result<f64> {
        if !(s.is_finite() && s >= 0.0) {
            return Err(invalid(CURVE, format!("amplitude {s:e} must be finite and >= 0")));
        }
        if s == 0.0 || self.endurance.is_some_and(|e| s <= e) {
            return Ok(f64::INFINITY);
        }
        Ok(match &self.law {
            Law::Basquin { sigma_f, b } => (s / sigma_f).powf(1.0 / b) / 2.0,
            Law::Power { m, c } => c * s.powf(-m),
            Law::Table { ln_n, ln_s } => {
                let x = s.ln();
                let last = ln_s.len() - 1;
                // the segment i..i+1 with ln_s[i] >= x >= ln_s[i+1], or an end segment
                let mut i = 0;
                while i + 1 < last && x < ln_s[i + 1] {
                    i += 1;
                }
                let slope = (ln_n[i + 1] - ln_n[i]) / (ln_s[i + 1] - ln_s[i]);
                (ln_n[i] + (x - ln_s[i]) * slope).exp()
            }
        })
    }

    /// The amplitude whose life is `n` cycles (the inverse of [`SnCurve::life`]), not below the endurance limit.
    pub fn strength_at(&self, n: f64) -> Result<f64> {
        positive(CURVE, "life", n)?;
        let s = match &self.law {
            Law::Basquin { sigma_f, b } => sigma_f * (2.0 * n).powf(*b),
            Law::Power { m, c } => (c / n).powf(1.0 / m),
            Law::Table { ln_n, ln_s } => {
                let x = n.ln();
                let last = ln_n.len() - 1;
                let mut i = 0;
                while i + 1 < last && x > ln_n[i + 1] {
                    i += 1;
                }
                let slope = (ln_s[i + 1] - ln_s[i]) / (ln_n[i + 1] - ln_n[i]);
                (ln_s[i] + (x - ln_n[i]) * slope).exp()
            }
        };
        Ok(match self.endurance {
            Some(e) => s.max(e),
            None => s,
        })
    }

    /// The power law `(m, c)` of `N = c S^-m` of a Basquin or power curve without an endurance limit (Basquin: `m = -1/b`,
    /// `c = sigma_f^m/2`); `None` for a table or with an endurance limit.
    pub fn power_law(&self) -> Option<(f64, f64)> {
        if self.endurance.is_some() {
            return None;
        }
        match &self.law {
            Law::Basquin { sigma_f, b } => {
                let m = -1.0 / b;
                Some((m, sigma_f.powf(m) / 2.0))
            }
            Law::Power { m, c } => Some((*m, *c)),
            Law::Table { .. } => None,
        }
    }
}

// --------------------------------------------------------------------------------------------------------- mean stress

/// The kind of mean-stress correction (ADR 0449 D2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeanStress {
    /// No correction: the amplitude is used as it is.
    None,
    /// Goodman `sigma_a/(1 - sigma_m/sigma_u)`.
    Goodman {
        /// Ultimate tensile strength `sigma_u`.
        ultimate: f64,
    },
    /// Gerber `sigma_a/(1 - (sigma_m/sigma_u)^2)`.
    Gerber {
        /// Ultimate tensile strength `sigma_u`.
        ultimate: f64,
    },
    /// Soderberg `sigma_a/(1 - sigma_m/sigma_y)`.
    Soderberg {
        /// Yield strength `sigma_y`.
        yield_strength: f64,
    },
}

/// A mean-stress correction: the kind, and whether a compressive mean changes the amplitude (`compressive_credit`; default
/// false: a compressive mean gives the amplitude unchanged).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeanStressCorrection {
    /// The kind.
    pub kind: MeanStress,
    /// Apply the formula to a compressive mean too (Goodman and Soderberg credit it, Gerber penalises it).
    pub compressive_credit: bool,
}

const MEAN: &str = "mean stress";

impl MeanStressCorrection {
    /// The correction of `kind` without compressive credit.
    pub fn new(kind: MeanStress) -> MeanStressCorrection {
        MeanStressCorrection {
            kind,
            compressive_credit: false,
        }
    }

    /// No correction.
    pub fn none() -> MeanStressCorrection {
        MeanStressCorrection::new(MeanStress::None)
    }

    /// The same correction with compressive credit on or off.
    pub fn with_compressive_credit(mut self, on: bool) -> MeanStressCorrection {
        self.compressive_credit = on;
        self
    }

    fn strength(&self) -> Result<Option<(f64, &'static str)>> {
        let (s, name) = match self.kind {
            MeanStress::None => return Ok(None),
            MeanStress::Goodman { ultimate } => (ultimate, "Goodman"),
            MeanStress::Gerber { ultimate } => (ultimate, "Gerber"),
            MeanStress::Soderberg { yield_strength } => (yield_strength, "Soderberg"),
        };
        positive(MEAN, "strength", s)?;
        Ok(Some((s, name)))
    }

    /// The fully reversed amplitude equivalent to amplitude `sa >= 0` at mean `sm`. Refuses a mean at or above the strength
    /// (with credit, Gerber also refuses `sm <= -sigma_u`).
    pub fn equivalent_amplitude(&self, sa: f64, sm: f64) -> Result<f64> {
        if !(sa.is_finite() && sa >= 0.0) {
            return Err(invalid(MEAN, format!("amplitude {sa:e} must be finite and >= 0")));
        }
        if !sm.is_finite() {
            return Err(invalid(MEAN, format!("mean {sm:e} must be finite")));
        }
        let Some((strength, name)) = self.strength()? else {
            return Ok(sa);
        };
        let refuse = || {
            if sm < 0.0 {
                invalid(
                    MEAN,
                    format!(
                        "mean stress {sm:e} at or above the strength {strength:e} in modulus ({name}, with compressive credit): no equivalent amplitude"
                    ),
                )
            } else {
                invalid(
                    MEAN,
                    format!(
                        "mean stress {sm:e} at or above the strength {strength:e} ({name}): no equivalent amplitude"
                    ),
                )
            }
        };
        if sm >= strength {
            return Err(refuse());
        }
        if sm < 0.0 && !self.compressive_credit {
            return Ok(sa);
        }
        let r = sm / strength;
        Ok(match self.kind {
            MeanStress::Gerber { .. } => {
                if r <= -1.0 {
                    return Err(refuse());
                }
                sa / (1.0 - r * r)
            }
            _ => sa / (1.0 - r),
        })
    }
}

/// The safety factor: the factor `lambda` on the whole stress (amplitude `sa` and mean `sm` together) at which the life is
/// `design_life` cycles (ADR 0449 D2); `f64::INFINITY` when no factor reaches it.
pub fn safety_factor(
    curve: &SnCurve,
    correction: &MeanStressCorrection,
    sa: f64,
    sm: f64,
    design_life: f64,
) -> Result<f64> {
    if !(design_life.is_finite() && design_life > 0.0) {
        return Err(invalid(
            "design life",
            format!("{design_life:e} must be finite and positive"),
        ));
    }
    // validates the arguments and the strength
    correction.equivalent_amplitude(sa, sm)?;
    let s = curve.strength_at(design_life)?;
    let plain = s / sa;
    let Some((strength, _)) = correction.strength()? else {
        return Ok(plain);
    };
    if sm < 0.0 && !correction.compressive_credit {
        return Ok(plain);
    }
    Ok(match correction.kind {
        MeanStress::Gerber { .. } => {
            let q = s * sm / strength;
            2.0 * s / (sa + (sa * sa + 4.0 * q * q).sqrt())
        }
        _ => {
            let d = sa + s * sm / strength;
            if d <= 0.0 {
                f64::INFINITY
            } else {
                s / d
            }
        }
    })
}

// ------------------------------------------------------------------------------------------------- equivalent stresses

fn vm_form(s: &[f64; 6], r: &[f64; 6]) -> f64 {
    0.5 * ((s[0] - s[1]) * (r[0] - r[1]) + (s[1] - s[2]) * (r[1] - r[2]) + (s[2] - s[0]) * (r[2] - r[0]))
        + 3.0 * (s[3] * r[3] + s[4] * r[4] + s[5] * r[5])
}

/// The amplitude of the von Mises stress of `sigma(t) = Re((re + i im) e^{i w t})`: its maximum over the phase,
/// `sqrt((q_A + q_B)/2 + sqrt(((q_A - q_B)/2)^2 + p_AB^2))` with `q` the von Mises quadratic form and `p` its bilinear form.
pub fn harmonic_von_mises_amplitude(re: &[f64; 6], im: &[f64; 6]) -> f64 {
    let qa = vm_form(re, re);
    let qb = vm_form(im, im);
    let p = vm_form(re, im);
    (0.5 * (qa + qb) + (0.5 * (qa - qb)).hypot(p)).max(0.0).sqrt()
}

fn combine(re: &[f64; 6], im: &[f64; 6], th: f64) -> [f64; 6] {
    let (s, c) = th.sin_cos();
    std::array::from_fn(|k| re[k] * c - im[k] * s)
}

/// The largest principal value of `+-sigma(th)` at phase `th`, with its direction.
fn principal_extreme(re: &[f64; 6], im: &[f64; 6], th: f64) -> (f64, Vec3) {
    let p = principal(&combine(re, im, th));
    if p.values[0] >= -p.values[2] {
        (p.values[0], p.directions[0])
    } else {
        (-p.values[2], p.directions[2])
    }
}

/// The amplitude of the largest principal stress of `Re((re + i im) e^{i w t})`, `max_th lambda_1(re cos th - im sin th)`
/// (720 phases on `[0, pi)` of `max(lambda_1, -lambda_3)`, then golden-section refinement to 1e-12 rad), with the principal
/// direction at the maximising phase.
pub fn harmonic_max_principal(re: &[f64; 6], im: &[f64; 6]) -> (f64, Vec3) {
    const STEPS: usize = 720;
    let h = std::f64::consts::PI / STEPS as f64;
    let mut best = (f64::NEG_INFINITY, [1.0, 0.0, 0.0], 0.0);
    for j in 0..STEPS {
        let th = j as f64 * h;
        let (v, d) = principal_extreme(re, im, th);
        if v > best.0 {
            best = (v, d, th);
        }
    }
    let (mut a, mut b) = (best.2 - h, best.2 + h);
    let g = 0.5 * (5f64.sqrt() - 1.0);
    let mut x1 = b - g * (b - a);
    let mut x2 = a + g * (b - a);
    let mut f1 = principal_extreme(re, im, x1).0;
    let mut f2 = principal_extreme(re, im, x2).0;
    while b - a > 1e-12 {
        if f1 >= f2 {
            b = x2;
            x2 = x1;
            f2 = f1;
            x1 = b - g * (b - a);
            f1 = principal_extreme(re, im, x1).0;
        } else {
            a = x1;
            x1 = x2;
            f1 = f2;
            x2 = a + g * (b - a);
            f2 = principal_extreme(re, im, x2).0;
        }
    }
    let (v, d) = principal_extreme(re, im, 0.5 * (a + b));
    if v > best.0 {
        (v, d)
    } else {
        (best.0, best.1)
    }
}

/// The von Mises stress with the sign of the trace (`+` for a zero trace): the equivalent mean stress.
pub fn signed_von_mises(s: &[f64; 6]) -> f64 {
    let v = von_mises(s);
    if s[0] + s[1] + s[2] < 0.0 {
        -v
    } else {
        v
    }
}

/// The equivalent stress used for amplitude and mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Equivalent {
    /// Von Mises: the harmonic amplitude maximised over the phase, the mean the signed von Mises stress.
    VonMises,
    /// Largest principal: the harmonic amplitude maximised over the phase, the mean the normal stress of the mean tensor on
    /// the principal direction of the maximising phase.
    MaxPrincipal,
}

/// The equivalent stress of each tensor of a proportional history: the signed von Mises stress, or the principal value of
/// largest modulus with its sign.
pub fn equivalent_history(history: &[[f64; 6]], equivalent: Equivalent) -> Vec<f64> {
    history
        .iter()
        .map(|s| match equivalent {
            Equivalent::VonMises => signed_von_mises(s),
            Equivalent::MaxPrincipal => {
                let p = principal(s);
                if p.values[0].abs() >= p.values[2].abs() {
                    p.values[0]
                } else {
                    p.values[2]
                }
            }
        })
        .collect()
}

// ------------------------------------------------------------------------------------------------------------- fields

/// What a fatigue evaluation uses (ADR 0449 D3).
#[derive(Debug, Clone, PartialEq)]
pub struct FatigueSpec {
    /// The stress-life curve.
    pub curve: SnCurve,
    /// The mean-stress correction.
    pub mean: MeanStressCorrection,
    /// The equivalent stress.
    pub equivalent: Equivalent,
    /// A design life in cycles for the safety factor, if wanted.
    pub design_life: Option<f64>,
}

/// The fatigue result at one point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointFatigue {
    /// The equivalent stress amplitude.
    pub amplitude: f64,
    /// The equivalent mean stress.
    pub mean: f64,
    /// The fully reversed amplitude after the mean-stress correction.
    pub equivalent_amplitude: f64,
    /// The life in cycles (`f64::INFINITY` for an infinite life).
    pub life: f64,
    /// The damage per cycle, `1/life` (0 for an infinite life).
    pub damage_per_cycle: f64,
    /// The safety factor against the design life, if one was given.
    pub safety_factor: Option<f64>,
}

impl FatigueSpec {
    /// A specification without a design life.
    pub fn new(curve: SnCurve, mean: MeanStressCorrection, equivalent: Equivalent) -> FatigueSpec {
        FatigueSpec {
            curve,
            mean,
            equivalent,
            design_life: None,
        }
    }

    /// The same specification with a design life for the safety factor.
    pub fn with_design_life(mut self, n: f64) -> FatigueSpec {
        self.design_life = Some(n);
        self
    }

    /// The fatigue result of a static stress `mean` and a harmonic stress `Re((re + i im) e^{i w t})` at one point.
    pub fn evaluate_harmonic(&self, mean: &[f64; 6], re: &[f64; 6], im: &[f64; 6]) -> Result<PointFatigue> {
        let (amplitude, sm) = match self.equivalent {
            Equivalent::VonMises => (harmonic_von_mises_amplitude(re, im), signed_von_mises(mean)),
            Equivalent::MaxPrincipal => {
                let (a, n) = harmonic_max_principal(re, im);
                let t = [
                    mean[0] * n[0] + mean[3] * n[1] + mean[5] * n[2],
                    mean[3] * n[0] + mean[1] * n[1] + mean[4] * n[2],
                    mean[5] * n[0] + mean[4] * n[1] + mean[2] * n[2],
                ];
                (a, t[0] * n[0] + t[1] * n[1] + t[2] * n[2])
            }
        };
        let equivalent_amplitude = self.mean.equivalent_amplitude(amplitude, sm)?;
        let life = self.curve.life(equivalent_amplitude)?;
        let damage_per_cycle = if life.is_infinite() { 0.0 } else { 1.0 / life };
        let safety_factor = match self.design_life {
            Some(n) => Some(safety_factor(&self.curve, &self.mean, amplitude, sm, n)?),
            None => None,
        };
        Ok(PointFatigue {
            amplitude,
            mean: sm,
            equivalent_amplitude,
            life,
            damage_per_cycle,
            safety_factor,
        })
    }
}

/// The fatigue result at every point of a mean and a harmonic stress field (ADR 0449 D3).
#[derive(Debug, Clone, PartialEq)]
pub struct FatigueField {
    /// The harmonic frequency, Hz.
    pub frequency_hz: f64,
    /// One result per point.
    pub points: Vec<PointFatigue>,
}

impl FatigueField {
    /// Evaluates every point: `mean` the static stress (`None`: zero), `re`, `im` the harmonic stress at the same points (the
    /// fields of a [`crate::harmonic::HarmonicStress`], or nodal values), `frequency_hz` the harmonic frequency.
    pub fn harmonic(
        spec: &FatigueSpec,
        mean: Option<&[[f64; 6]]>,
        re: &[[f64; 6]],
        im: &[[f64; 6]],
        frequency_hz: f64,
    ) -> Result<FatigueField> {
        if !(frequency_hz.is_finite() && frequency_hz >= 0.0) {
            return Err(invalid(
                "frequency",
                format!("{frequency_hz:e} Hz must be finite and >= 0"),
            ));
        }
        let n = re.len();
        if im.len() != n {
            return Err(FemError::FieldLength {
                key: "fatigue field",
                expected: n,
                found: im.len(),
            });
        }
        if let Some(m) = mean {
            if m.len() != n {
                return Err(FemError::FieldLength {
                    key: "fatigue field",
                    expected: n,
                    found: m.len(),
                });
            }
        }
        let zero = [0.0; 6];
        let mut points = Vec::with_capacity(n);
        for p in 0..n {
            let m = mean.map_or(&zero, |m| &m[p]);
            let r = spec.evaluate_harmonic(m, &re[p], &im[p]).map_err(|e| match e {
                FemError::InvalidValue { key, reason } => FemError::InvalidValue {
                    key,
                    reason: format!("point {p}: {reason}"),
                },
                other => other,
            })?;
            points.push(r);
        }
        Ok(FatigueField { frequency_hz, points })
    }

    /// The damage per second at point `p`, `frequency_hz / life`.
    pub fn damage_rate(&self, p: usize) -> f64 {
        self.frequency_hz * self.points[p].damage_per_cycle
    }

    /// The first point of largest damage per cycle (`None` for an empty field).
    pub fn largest_damage(&self) -> Option<usize> {
        let mut best: Option<usize> = None;
        for (p, r) in self.points.iter().enumerate() {
            let better = match best {
                None => true,
                Some(b) => r.damage_per_cycle > self.points[b].damage_per_cycle,
            };
            if better {
                best = Some(p);
            }
        }
        best
    }
}

// ----------------------------------------------------------------------------------------------------------- counting

/// A counted cycle: range, mean and count (1 for a closed cycle, 1/2 for a half cycle).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cycle {
    /// The range, twice the amplitude.
    pub range: f64,
    /// The mean.
    pub mean: f64,
    /// 1 for a closed cycle, 0.5 for a half cycle.
    pub count: f64,
}

/// The result of a count: the closed cycles and the residue (the turning points of the unclosed loops).
#[derive(Debug, Clone, PartialEq)]
pub struct Counts {
    /// Closed cycles, in the order they closed.
    pub cycles: Vec<Cycle>,
    /// The residue.
    pub residue: Vec<f64>,
}

impl Counts {
    /// The closed cycles followed by each range of the residue as a half cycle (ASTM E1049-85 5.4.4 practice).
    pub fn with_half_cycles(&self) -> Vec<Cycle> {
        let mut out = self.cycles.clone();
        for w in self.residue.windows(2) {
            out.push(Cycle {
                range: (w[1] - w[0]).abs(),
                mean: 0.5 * (w[0] + w[1]),
                count: 0.5,
            });
        }
        out
    }
}

/// The turning points (reversals) of a history: equal neighbours merged, points on a monotone run dropped, the first and the
/// last point kept. A non-finite value is refused.
pub fn turning_points(history: &[f64]) -> Result<Vec<f64>> {
    let mut out: Vec<f64> = Vec::new();
    for (i, &v) in history.iter().enumerate() {
        if !v.is_finite() {
            return Err(invalid("history", format!("value {v:e} at sample {i} is not finite")));
        }
        push_turning(&mut out, v);
    }
    Ok(out)
}

fn push_turning(out: &mut Vec<f64>, v: f64) {
    let n = out.len();
    if n > 0 && v == out[n - 1] {
        return;
    }
    if n >= 2 {
        let (d1, d2) = (out[n - 1] - out[n - 2], v - out[n - 1]);
        if (d1 > 0.0 && d2 > 0.0) || (d1 < 0.0 && d2 < 0.0) {
            out[n - 1] = v;
            return;
        }
    }
    out.push(v);
}

/// The four-point pass: for the last four points `a b c d` of the stack, `(b, c)` closes a cycle when `|b - c| <= |a - b|`
/// and `|b - c| <= |c - d|`.
fn four_point(points: &[f64]) -> Counts {
    let mut stack: Vec<f64> = Vec::with_capacity(64);
    let mut cycles = Vec::new();
    for &v in points {
        stack.push(v);
        while stack.len() >= 4 {
            let k = stack.len();
            let (a, b, c, d) = (stack[k - 4], stack[k - 3], stack[k - 2], stack[k - 1]);
            let r = (b - c).abs();
            if r <= (a - b).abs() && r <= (c - d).abs() {
                cycles.push(Cycle {
                    range: r,
                    mean: 0.5 * (b + c),
                    count: 1.0,
                });
                stack.drain(k - 3..k - 1);
            } else {
                break;
            }
        }
    }
    Counts { cycles, residue: stack }
}

/// Rainflow count of a history (ADR 0449 D4: the four-point closing of hysteresis loops of Downing and Socie, ASTM E1049-85
/// 5.4.4): the closed cycles and the residue.
pub fn rainflow(history: &[f64]) -> Result<Counts> {
    Ok(four_point(&turning_points(history)?))
}

/// Rainflow count of a periodic history (one period; a closing value equal to the first is dropped): rotated to its first
/// turning point of largest modulus and closed there, so every loop closes and the residue is empty.
pub fn rainflow_periodic(history: &[f64]) -> Result<Counts> {
    let mut tp = turning_points(history)?;
    if tp.len() >= 2 && tp[0] == tp[tp.len() - 1] {
        tp.pop();
    }
    if tp.len() < 2 {
        return Ok(Counts {
            cycles: Vec::new(),
            residue: Vec::new(),
        });
    }
    let mut k = 0;
    for (i, v) in tp.iter().enumerate() {
        if v.abs() > tp[k].abs() {
            k = i;
        }
    }
    let mut rot: Vec<f64> = Vec::with_capacity(tp.len() + 1);
    for &v in tp[k..].iter().chain(&tp[..k]).chain(std::iter::once(&tp[k])) {
        push_turning(&mut rot, v);
    }
    let mut counts = four_point(&rot);
    let res = std::mem::take(&mut counts.residue);
    if res.len() > 3 {
        return Err(invalid(
            "rainflow",
            format!(
                "internal error: the residue of the closed sequence has {} points, more than the three of [M, v, M] (ADR 0453)",
                res.len()
            ),
        ));
    }
    // the residue of a closed sequence started at a global extreme is [M, v, M] (ADR 0449, closed forms (i))
    for pair in res.windows(3).step_by(2) {
        counts.cycles.push(Cycle {
            range: (pair[1] - pair[0]).abs(),
            mean: 0.5 * (pair[0] + pair[1]),
            count: 1.0,
        });
    }
    Ok(counts)
}

/// The Palmgren-Miner damage `sum count/N` of counted cycles, each at amplitude `range/2` and its mean through the
/// correction; an infinite life adds 0. Neumaier's compensated sum in the order of the cycles.
pub fn miner(cycles: &[Cycle], curve: &SnCurve, correction: &MeanStressCorrection) -> Result<f64> {
    let (mut sum, mut comp) = (0.0f64, 0.0f64);
    for c in cycles {
        let sar = correction.equivalent_amplitude(0.5 * c.range, c.mean)?;
        let n = curve.life(sar)?;
        if n.is_infinite() {
            continue;
        }
        let x = c.count / n;
        let t = sum + x;
        if sum.abs() >= x.abs() {
            comp += (sum - t) + x;
        } else {
            comp += (x - t) + sum;
        }
        sum = t;
    }
    Ok(sum + comp)
}

// --------------------------------------------------------------------------------------------------------- narrow band

/// `ln Gamma(x)`, `x > 0`: Stirling's series with `B_2 .. B_14` at `x + n >= 15` and the recurrence below (ADR 0449 D5).
pub fn ln_gamma(x: f64) -> Result<f64> {
    positive("gamma", "argument", x)?;
    let (z, ln_p) = shift(x);
    Ok(stirling(z) - ln_p)
}

/// `Gamma(x)`, `x > 0` (overflows to infinity above about 171).
pub fn gamma(x: f64) -> Result<f64> {
    positive("gamma", "argument", x)?;
    let mut z = x;
    let mut p = 1.0;
    while z < 15.0 {
        p *= z;
        z += 1.0;
    }
    Ok(stirling(z).exp() / p)
}

fn shift(x: f64) -> (f64, f64) {
    let mut z = x;
    let mut ln_p = 0.0;
    while z < 15.0 {
        ln_p += z.ln();
        z += 1.0;
    }
    (z, ln_p)
}

fn stirling(z: f64) -> f64 {
    // B_2k / (2k (2k - 1)) for k = 1 .. 7
    const C: [f64; 7] = [
        1.0 / 12.0,
        -1.0 / 360.0,
        1.0 / 1260.0,
        -1.0 / 1680.0,
        1.0 / 1188.0,
        -691.0 / 360_360.0,
        1.0 / 156.0,
    ];
    let w = 1.0 / (z * z);
    let mut series = 0.0;
    for c in C.iter().rev() {
        series = series * w + c;
    }
    (z - 0.5) * z.ln() - z + 0.5 * (2.0 * std::f64::consts::PI).ln() + series / z
}

const NARROW: &str = "narrow band";

/// The narrow-band (Rayleigh) damage rate `nu0 (sqrt 2 rms)^m Gamma(1 + m/2)/c` of a Gaussian stress with standard
/// deviation `rms` and zero up-crossing rate `nu0` on the curve `N = c S^-m` (ADR 0449 D5).
pub fn narrow_band_damage_rate(nu0: f64, rms: f64, m: f64, c: f64) -> Result<f64> {
    if !(nu0.is_finite() && nu0 >= 0.0) {
        return Err(invalid(
            NARROW,
            format!("up-crossing rate {nu0:e} must be finite and >= 0"),
        ));
    }
    if !(rms.is_finite() && rms >= 0.0) {
        return Err(invalid(NARROW, format!("rms {rms:e} must be finite and >= 0")));
    }
    positive(NARROW, "exponent m", m)?;
    positive(NARROW, "constant c", c)?;
    Ok(nu0 * (std::f64::consts::SQRT_2 * rms).powf(m) * gamma(1.0 + 0.5 * m)? / c)
}

/// [`narrow_band_damage_rate`] on the power law of a curve; a table or an endurance limit is refused.
pub fn narrow_band_damage_rate_curve(nu0: f64, rms: f64, curve: &SnCurve) -> Result<f64> {
    let (m, c) = curve.power_law().ok_or_else(|| {
        invalid(
            NARROW,
            "the curve must be a power law without an endurance limit (Basquin or power)".to_string(),
        )
    })?;
    narrow_band_damage_rate(nu0, rms, m, c)
}

/// The standard deviation `sqrt(m0)` and the zero up-crossing rate `sqrt(m2/m0)` (Rice) from the one-sided spectral moments
/// `m_k = int f^k G(f) df` in Hz.
pub fn spectral_rate(m0: f64, m2: f64) -> Result<(f64, f64)> {
    positive(NARROW, "moment m0", m0)?;
    if !(m2.is_finite() && m2 >= 0.0) {
        return Err(invalid(NARROW, format!("moment m2 {m2:e} must be finite and >= 0")));
    }
    Ok((m0.sqrt(), (m2 / m0).sqrt()))
}
