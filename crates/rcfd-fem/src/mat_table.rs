//! Elastic properties against temperature from tables. Declared by ADR 0440 (F-408, the scaffold of ADR 0311); filled by ADR 0312 (F-409).
//!
//! ADR 0452 (F-409) D2: the columns of a [`Material::Tabulated`] (named properties of its isotropic or orthotropic base),
//! piecewise-linear interpolation inside the table (a temperature outside it is refused, nothing is extrapolated), and the
//! thermal strain as the integral of the tabulated expansion coefficient from the reference temperature (F9).

use crate::error::{FemError, Result};
use crate::model::Material;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// The column names of a table on an isotropic base.
pub const ISOTROPIC_COLUMNS: [&str; 3] = ["e", "nu", "expansion"];

/// The column names of a table on an orthotropic base.
pub const ORTHOTROPIC_COLUMNS: [&str; 12] = [
    "e_1",
    "e_2",
    "e_3",
    "nu_12",
    "nu_13",
    "nu_23",
    "g_12",
    "g_13",
    "g_23",
    "expansion_1",
    "expansion_2",
    "expansion_3",
];

fn outside(t: f64, temperatures: &[f64]) -> FemError {
    FemError::InvalidValue {
        key: "temperature",
        reason: format!(
            "temperature {t:.6} K is outside the table [{:.6}, {:.6}] K",
            temperatures[0],
            temperatures[temperatures.len() - 1]
        ),
    }
}

fn segment(temperatures: &[f64], t: f64) -> Result<usize> {
    let n = temperatures.len();
    if !(t.is_finite() && t >= temperatures[0] && t <= temperatures[n - 1]) {
        return Err(outside(t, temperatures));
    }
    Ok((0..n - 1).find(|&i| t <= temperatures[i + 1]).unwrap_or(n - 2))
}

fn on_segment(temperatures: &[f64], values: &[f64], i: usize, t: f64) -> f64 {
    let w = (t - temperatures[i]) / (temperatures[i + 1] - temperatures[i]);
    values[i] + (values[i + 1] - values[i]) * w
}

/// Piecewise-linear interpolation of `values` over `temperatures` (strictly increasing) at `t`: on the first segment with
/// `t_i <= t <= t_(i+1)`, `v_i + (v_(i+1) - v_i) (t - t_i)/(t_(i+1) - t_i)`. A constant column gives its value bit for bit.
/// Refuses a `t` outside the table or not finite (key `"temperature"`).
pub fn interpolate(temperatures: &[f64], values: &[f64], t: f64) -> Result<f64> {
    let i = segment(temperatures, t)?;
    Ok(on_segment(temperatures, values, i, t))
}

/// The integral of the interpolated `values` from `from` to `to` (signed): the trapezoid on each segment piece, exact for
/// the piecewise-linear function. Both ends must lie inside the table.
pub fn integrate(temperatures: &[f64], values: &[f64], from: f64, to: f64) -> Result<f64> {
    segment(temperatures, from)?;
    segment(temperatures, to)?;
    let (lo, hi, sign) = if to >= from { (from, to, 1.0) } else { (to, from, -1.0) };
    let mut sum = 0.0;
    for i in 0..temperatures.len() - 1 {
        let x0 = lo.max(temperatures[i]);
        let x1 = hi.min(temperatures[i + 1]);
        if x1 > x0 {
            let v0 = on_segment(temperatures, values, i, x0);
            let v1 = on_segment(temperatures, values, i, x1);
            sum += (x1 - x0) * (v0 + v1) / 2.0;
        }
    }
    Ok(sign * sum)
}

fn parts(material: &Material) -> Option<(&Material, &[f64], &[(String, Vec<f64>)])> {
    match material {
        Material::Tabulated {
            base,
            temperatures,
            columns,
        } => Some((base, temperatures, columns)),
        _ => None,
    }
}

fn column<'a>(columns: &'a [(String, Vec<f64>)], name: &str) -> Option<&'a [f64]> {
    columns.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_slice())
}

/// Refuses a column whose name is not a property of the base and a repeated column (key `"columns"`; ADR 0452 D2).
pub fn check_columns(material: &Material) -> Result<()> {
    let Some((base, _, columns)) = parts(material) else {
        return Ok(());
    };
    let allowed: &[&str] = match base {
        Material::Orthotropic { .. } => &ORTHOTROPIC_COLUMNS,
        _ => &ISOTROPIC_COLUMNS,
    };
    for (k, (name, _)) in columns.iter().enumerate() {
        if !allowed.contains(&name.as_str()) {
            return Err(FemError::InvalidValue {
                key: "columns",
                reason: format!(
                    "column '{name}' is not a property of a {} base (one of {allowed:?})",
                    base.name()
                ),
            });
        }
        if columns[..k].iter().any(|(n, _)| n == name) {
            return Err(FemError::InvalidValue {
                key: "columns",
                reason: format!("column '{name}' is given twice"),
            });
        }
    }
    Ok(())
}

/// Refuses a reference temperature outside a table that has an expansion column (key `"temperature"`).
pub fn check_reference(material: &Material, reference: f64) -> Result<()> {
    let Some((_, temperatures, columns)) = parts(material) else {
        return Ok(());
    };
    if columns.iter().any(|(n, _)| n.starts_with("expansion")) {
        segment(temperatures, reference).map_err(|_| FemError::InvalidValue {
            key: "temperature",
            reason: format!(
                "the reference temperature {reference:.6} K is outside the table [{:.6}, {:.6}] K of an expansion column",
                temperatures[0],
                temperatures[temperatures.len() - 1]
            ),
        })?;
    }
    Ok(())
}

/// The elastic constants of a material at a temperature (ADR 0452 D2, D3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Constants {
    /// `E`, `nu`.
    Isotropic(f64, f64),
    /// `E_i`, `nu_ij`, `G_ij` in the order of [`Material::Orthotropic`].
    Orthotropic([f64; 3], [f64; 3], [f64; 3]),
}

fn value(temperatures: &[f64], columns: &[(String, Vec<f64>)], name: &str, base: f64, t: f64) -> Result<f64> {
    match column(columns, name) {
        Some(v) => interpolate(temperatures, v, t),
        None => Ok(base),
    }
}

/// The elastic constants of the table's base with its tabulated properties at `t`, validated as the base's constructor and
/// [`crate::mat_ortho::check_positive_definite`] do. Refuses `t` outside the table.
pub(crate) fn constants_at(material: &Material, t: f64) -> Result<Constants> {
    let Some((base, temperatures, columns)) = parts(material) else {
        return Err(FemError::InvalidValue {
            key: "base",
            reason: format!("a {} material has no table", material.name()),
        });
    };
    segment(temperatures, t)?;
    match base {
        Material::IsotropicElastic { e, nu, .. } => {
            let e = value(temperatures, columns, "e", *e, t)?;
            let nu = value(temperatures, columns, "nu", *nu, t)?;
            if !(e > 0.0 && e.is_finite()) {
                return Err(FemError::InvalidMaterial {
                    key: "e",
                    value: e,
                    reason: "Young's modulus must be positive and finite",
                });
            }
            if !(nu > -1.0 && nu < 0.5) {
                return Err(FemError::InvalidMaterial {
                    key: "nu",
                    value: nu,
                    reason: "Poisson ratio must lie in the open interval (-1, 0.5)",
                });
            }
            Ok(Constants::Isotropic(e, nu))
        }
        Material::Orthotropic { e, nu, g, .. } => {
            let mut ev = *e;
            let mut nuv = *nu;
            let mut gv = *g;
            for k in 0..3 {
                ev[k] = value(temperatures, columns, ORTHOTROPIC_COLUMNS[k], e[k], t)?;
                nuv[k] = value(temperatures, columns, ORTHOTROPIC_COLUMNS[3 + k], nu[k], t)?;
                gv[k] = value(temperatures, columns, ORTHOTROPIC_COLUMNS[6 + k], g[k], t)?;
            }
            crate::mat_ortho::check_positive_definite(ev, nuv, gv)?;
            Ok(Constants::Orthotropic(ev, nuv, gv))
        }
        other => Err(FemError::InvalidValue {
            key: "base",
            reason: format!("a {} base", other.name()),
        }),
    }
}

/// The thermal strains along the material directions of a table at the temperature `t` with the change `delta_t` from
/// `reference` (ADR 0452 D2, F9): per direction the base's `alpha delta_t` without a column, `v delta_t` for a column whose
/// entries are all bit-equal to `v`, else the integral of the column from `reference` to `t`. An isotropic base gives the same
/// value in the three entries.
pub fn expansion_strain(material: &Material, reference: f64, t: f64, delta_t: f64) -> Result<[f64; 3]> {
    let Some((base, temperatures, columns)) = parts(material) else {
        return Err(FemError::InvalidValue {
            key: "base",
            reason: format!("a {} material has no table", material.name()),
        });
    };
    let one = |name: &str, alpha: f64| -> Result<f64> {
        match column(columns, name) {
            None => Ok(alpha * delta_t),
            Some(v) if v.iter().all(|x| x.to_bits() == v[0].to_bits()) => Ok(v[0] * delta_t),
            Some(v) => integrate(temperatures, v, reference, t),
        }
    };
    match base {
        Material::IsotropicElastic { expansion, .. } => {
            let s = one("expansion", *expansion)?;
            Ok([s, s, s])
        }
        Material::Orthotropic { expansion, .. } => Ok([
            one("expansion_1", expansion[0])?,
            one("expansion_2", expansion[1])?,
            one("expansion_3", expansion[2])?,
        ]),
        other => Err(FemError::InvalidValue {
            key: "base",
            reason: format!("a {} base", other.name()),
        }),
    }
}
