//! Whirl stability of a cyclic sector at any speed (ADR 0420, F-511): each harmonic index solved by the state matrix of ADR
//! 0416 on its real reduced problem (the doubled cos/sin matrices of ADR 0411 D5 for a pair index, the real 2 x 2-block form
//! of the complex Hermitian problem of the harmonic, whose spectrum is that problem's together with its conjugate, D1), with
//! the stability flag and the growth rate per index; the rotor is stable iff every index is (D2).

use super::super::{check_size, dense_of, dot, doubled, inertia, mass_refused, speed_refused, whirl_number, Whirl};
use super::{state_solve, Stability, StateWhirl, StateWhirlMode};
use crate::error::{FemError, Result};
use crate::mpc::Transformation;
use crate::rotating::{check_harmonics, check_speeds, LossOfStiffness, RotatingSector};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;
use std::fmt::Write as _;

impl Stability {
    /// The flag of a rotor from the flags of its harmonic indices (ADR 0420 D2): `Stable` iff every flag is (an empty list
    /// is `Stable`), otherwise `Unstable` with the largest growth rate of the unstable ones.
    pub fn combined(flags: &[Stability]) -> Stability {
        let mut growth: Option<f64> = None;
        for f in flags {
            if let Stability::Unstable { growth: g } = *f {
                growth = Some(growth.map_or(g, |h| h.max(g)));
            }
        }
        match growth {
            None => Stability::Stable,
            Some(growth) => Stability::Unstable { growth },
        }
    }
}

/// The whirl analysis of one harmonic index of a sector at one speed by the state matrix (ADR 0420 D4).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct HarmonicStateWhirl {
    /// The harmonic index `k`.
    pub harmonic: usize,
    /// 1 for `k = 0` and `2 k = N`, 2 otherwise.
    pub multiplicity: usize,
    /// The analysis of the index's real reduced problem: `loss` with its count (even for a pair), all its eigenvalues, the
    /// flag, and the modes on the sector's equations (cosine then sine part for a pair).
    pub state: StateWhirl,
    /// The wave sense `tau` of each mode (ADR 0411 D5), `None` for a single index.
    pub waves: Vec<Option<f64>>,
}

/// The whirl stability of a sector over harmonic indices and speeds (ADR 0420 D4).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CyclicWhirlStability {
    /// The harmonic indices.
    pub harmonics: Vec<usize>,
    /// The speeds, rad/s, increasing.
    pub speeds: Vec<f64>,
    /// `results[h][j]`: index `harmonics[h]` at `speeds[j]`.
    pub results: Vec<Vec<HarmonicStateWhirl>>,
}

impl CyclicWhirlStability {
    /// The flag of the rotor at speed `j`: [`Stability::combined`] over the indices (the full rotor's flag when the indices
    /// cover `0 .. N/2`).
    pub fn rotor(&self, j: usize) -> Stability {
        let flags: Vec<Stability> = self.results.iter().map(|r| r[j].state.stability).collect();
        Stability::combined(&flags)
    }

    /// The table as CSV: `harmonic,speed_rad_s,negative_stiffness,stable,growth_rate_1_s` and `f_<j>_hz` per reported mode,
    /// one row per index and speed (index-major); `negative_stiffness` 0 when `K_eff` of the index is positive definite and
    /// empty when its factorisation met a zero pivot, `growth_rate_1_s` 0 for a stable index, a missing mode an empty field.
    pub fn to_csv(&self) -> String {
        let width = self
            .results
            .iter()
            .flat_map(|r| r.iter().map(|x| x.state.modes.len()))
            .max()
            .unwrap_or(0);
        let mut s = String::from("harmonic,speed_rad_s,negative_stiffness,stable,growth_rate_1_s");
        for j in 0..width {
            let _ = write!(s, ",f_{j}_hz");
        }
        s.push('\n');
        for (h, row) in self.harmonics.iter().zip(&self.results) {
            for (w, x) in self.speeds.iter().zip(row) {
                let r = &x.state;
                let negative = match r.loss {
                    None => "0".to_string(),
                    Some(l) => l.negative.map_or_else(String::new, |n| n.to_string()),
                };
                let growth = match r.stability {
                    Stability::Stable => 0.0,
                    Stability::Unstable { growth } => growth,
                };
                let _ = write!(s, "{h},{w},{negative},{},{growth}", r.stability.is_stable());
                for j in 0..width {
                    s.push(',');
                    if let Some(m) = r.modes.get(j) {
                        let _ = write!(s, "{}", m.frequency);
                    }
                }
                s.push('\n');
            }
        }
        s
    }
}

impl<'a> RotatingSector<'a> {
    /// The whirl analysis of harmonic index `harmonic` at the speed `omega`, rad/s, by the state matrix (ADR 0420): valid
    /// at any speed, also where the index's reduced `K_eff` is not positive definite (then `state.loss` is set and the index
    /// may still be stable). `K_eff`, `M` and the Coriolis matrix of the sector, doubled for a pair, reduced by the rows of
    /// the index (ADR 0411 D5); `G = 2 omega C` if `options.coriolis`, 0 otherwise; `options.modal.modes x multiplicity`
    /// reported modes. Refuses a harmonic above `N / 2` (`"harmonic"`), a negative or non-finite speed (`"speed"`), a mode
    /// count of 0 or not below the unknowns (`"modes"`) and more than [`super::super::MAX_WHIRL_UNKNOWNS`] reduced unknowns
    /// (`"unknowns"`).
    pub fn whirl_state(&self, harmonic: usize, omega: f64) -> Result<HarmonicStateWhirl> {
        self.check_harmonic(harmonic)?;
        if !(omega.is_finite() && omega >= 0.0) {
            return Err(speed_refused(omega));
        }
        let c = self.part.coriolis()?;
        self.whirl_state_at(harmonic, omega, &c)
    }

    /// [`RotatingSector::whirl_state`] over the harmonic indices `harmonics` (as [`RotatingSector::campbell`]; key
    /// `"harmonics"`) and the speeds `speeds`, rad/s (as [`crate::rotating::RotatingPart::campbell`]; key `"speeds"`).
    pub fn whirl_stability(&self, harmonics: &[usize], speeds: &[f64]) -> Result<CyclicWhirlStability> {
        check_harmonics(harmonics, self.sectors)?;
        check_speeds(speeds)?;
        let c = self.part.coriolis()?;
        let mut results = Vec::with_capacity(harmonics.len());
        for &k in harmonics {
            let mut row = Vec::with_capacity(speeds.len());
            for &w in speeds {
                row.push(self.whirl_state_at(k, w, &c)?);
            }
            results.push(row);
        }
        Ok(CyclicWhirlStability {
            harmonics: harmonics.to_vec(),
            speeds: speeds.to_vec(),
            results,
        })
    }

    fn check_harmonic(&self, harmonic: usize) -> Result<()> {
        let sectors = self.sectors;
        if 2 * harmonic > sectors {
            return Err(FemError::InvalidValue {
                key: "harmonic",
                reason: format!(
                    "harmonic index {harmonic} is above N/2 for a rotor of {sectors} sectors (the largest is {})",
                    sectors / 2
                ),
            });
        }
        Ok(())
    }

    fn whirl_state_at(&self, harmonic: usize, omega: f64, c: &CsrMatrix) -> Result<HarmonicStateWhirl> {
        self.check_harmonic(harmonic)?;
        let sectors = self.sectors;
        let model = self.part.model;
        let options = &self.part.options.modal;
        let multiplicity = if harmonic == 0 || 2 * harmonic == sectors { 1 } else { 2 };
        let n = model.dof_map().n_equations();
        let tr: Transformation = if harmonic == 0 {
            crate::mpc::transformation(model, &[])?
        } else {
            crate::mpc::harmonic_transformation(model, &[], harmonic)?
        };
        let keff = self.part.operator(omega);
        let (k2, m2, c2) = if multiplicity == 2 {
            (doubled(&keff), doubled(&self.part.m), doubled(c))
        } else {
            (keff, self.part.m.clone(), c.clone())
        };
        let zero = vec![0.0; tr.n_unknowns()];
        let (kr, _) = tr.reduce(&k2, &zero)?;
        let (mr, _) = tr.reduce(&m2, &zero)?;
        let (cr, _) = tr.reduce(&c2, &zero)?;
        let nq = kr.n;
        let count = options.modes * multiplicity;
        if options.modes == 0 || count >= nq {
            return Err(FemError::InvalidValue {
                key: "modes",
                reason: format!(
                    "{} modes ({count} whirl modes) requested of harmonic {harmonic} with {nq} free unknowns (at least 1 \
                     and fewer than the unknowns)",
                    options.modes
                ),
            });
        }
        for (a, d) in mr.diag().iter().enumerate() {
            if d.is_nan() || *d <= 0.0 {
                let i = tr.independent()[a];
                return Err(mass_refused(crate::cyclic::unknown_label(&tr, model, i), *d));
            }
        }
        let loss = inertia(&kr, &mr)?.map(|negative| LossOfStiffness { speed: omega, negative });
        check_size(nq)?;
        let kd = dense_of(&kr);
        let md = dense_of(&mr);
        let cd = dense_of(&cr);
        let gd: Vec<f64> = if self.part.options.coriolis {
            cd.iter().map(|v| 2.0 * omega * v).collect()
        } else {
            vec![0.0; nq * nq]
        };
        let out = state_solve(&kd, &md, &gd, nq, count)?;
        let width = multiplicity * n;
        let m = &self.part.m;
        let bil = |x: &[f64], y: &[f64]| -> f64 {
            let mut t = vec![0.0; n];
            m.matvec(y, &mut t);
            dot(x, &t)
        };
        let mut waves = Vec::with_capacity(out.modes.len());
        let mut modes = Vec::with_capacity(out.modes.len());
        for (s, re, im, res) in &out.modes {
            let kappa = whirl_number(&cd, &md, nq, re, im);
            let xr = tr.expand(re)[..width].to_vec();
            let xi = tr.expand(im)[..width].to_vec();
            waves.push(if multiplicity == 2 {
                let (ar, br) = (&xr[..n], &xr[n..]);
                let (ai, bi) = (&xi[..n], &xi[n..]);
                let num = 2.0 * (bil(ar, bi) - bil(ai, br));
                let den = bil(ar, ar) + bil(ai, ai) + bil(br, br) + bil(bi, bi);
                Some(num / den)
            } else {
                None
            });
            modes.push(StateWhirlMode {
                growth: s[0],
                omega: s[1],
                frequency: s[1] / (2.0 * PI),
                re: xr,
                im: xi,
                whirl: kappa,
                direction: Whirl::of(kappa),
                residual: *res,
            });
        }
        Ok(HarmonicStateWhirl {
            harmonic,
            multiplicity,
            state: StateWhirl {
                speed: omega,
                loss,
                eigenvalues: out.eigenvalues,
                stability: out.stability,
                real_defect: out.real_defect,
                modes,
                unknowns: nq,
            },
            waves,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// U2 of ADR 0420: the rotor's flag from its indices.
    #[test]
    fn u2_combined_flags() {
        let un = |growth: f64| Stability::Unstable { growth };
        assert_eq!(Stability::combined(&[]), Stability::Stable);
        assert_eq!(
            Stability::combined(&[Stability::Stable, Stability::Stable]),
            Stability::Stable
        );
        assert_eq!(
            Stability::combined(&[Stability::Stable, un(3.0), un(5.0), Stability::Stable]),
            un(5.0)
        );
        assert_eq!(Stability::combined(&[un(5.0), un(3.0)]), un(5.0));
    }
}
