//! Linear viscoelasticity: Prony series, time-temperature shift, complex modulus. Declared by ADR 0440 (F-408, the scaffold of
//! ADR 0311); filled by ADR 0485 (F-410, the registration of ADR 0313).
//!
//! The generalised Maxwell solid on an isotropic instantaneous base (ADR 0485 arithmetic 1): `sigma_0(eps) = 2 G_0 dev eps +
//! K_0 tr eps 1`; branch `i` has the relative shear and bulk weights `g_i`, `k_i` and the relaxation time `tau_i`, and carries
//! the internal stress `h_i`; the stress is `sigma = (1 - sum g_i) dev sigma_0 + (1 - sum k_i) (tr sigma_0/3) 1 + sum h_i`.
//! The update over a step of reduced length `dxi = dt / a_T`, exact for a strain linear in the step:
//!
//! * `x_i = dxi / tau_i`, `e_i = exp(-x_i)`, `gamma_i = (1 - e_i)/x_i` (`-expm1(-x_i)/x_i`; 1 at `x_i = 0`);
//! * `h_i^{n+1} = e_i h_i^n + gamma_i P_i (sigma_0(eps^{n+1}) - sigma_0(eps^n))`, `P_i s = g_i dev s + k_i (tr s/3) 1`;
//! * tangent `2 G_t I_dev + K_t 1 (x) 1`, `G_t = G_0 (1 - sum g_i (1 - gamma_i))`, `K_t = K_0 (1 - sum k_i (1 - gamma_i))`.
//!
//! The state of a point is `[eps^n (6, engineering shear), h_1 (6, tensor shear), ..., h_N]`, `6 (N + 1)` values, none for a
//! material without branches (which gives the bits of its base). The time increment and the temperature come from the point
//! context: no increment is an increment of zero length (the instantaneous response from the committed state); a material
//! with a shift needs a temperature. On the total Lagrangian driver the law acts on the Green-Lagrange strain and the second
//! Piola-Kirchhoff stress (ADR 0485 Limits). Only the nonlinear driver at a point context ([`crate::nonlinear::solve_from_at`])
//! and the nonlinear transient integrator take the material; every other path refuses it naming [`FOLLOW_UP`].

use crate::error::{FemError, Result};
use crate::harmonic_direct::ModulusTable;
use crate::material::{apply, isotropic_elasticity, Matrix6, PointContext, PronyTerm, TimeTemperatureShift};
use crate::model::Material;
use rcfd_linalg::complex::C64;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// The package named where a path that does not take the viscoelastic material is given one (ADR 0485 D2).
pub const FOLLOW_UP: &str = "a follow-up of ADR 0485";

/// The universal gas constant, J/(mol K) (CODATA 2018: the product of the defined Avogadro and Boltzmann constants).
pub const GAS_CONSTANT: f64 = 8.314_462_618;

/// The state per integration point (ADR 0485 D1): the strain of the last committed step (six values) and the internal
/// stress (six values) of each Prony term; none without terms.
pub fn state_size(terms: usize) -> usize {
    if terms == 0 {
        0
    } else {
        6 * (terms + 1)
    }
}

/// The result of one update.
#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    /// The stress (Voigt, tensor shear).
    pub stress: [f64; 6],
    /// The tangent `d sigma / d eps` (strain with engineering shear).
    pub tangent: Matrix6,
    /// The state after the step (`state_size` values).
    pub state: Vec<f64>,
}

/// The complex moduli at one circular frequency (ADR 0485 arithmetic 2): storage modulus in the real part, loss modulus in
/// the imaginary part.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComplexModuli {
    /// `G*(omega)`, Pa.
    pub shear: C64,
    /// `K*(omega)`, Pa.
    pub bulk: C64,
    /// `E*(omega) = 9 K* G* / (3 K* + G*)`, Pa.
    pub young: C64,
}

fn temperature_error(reason: String) -> FemError {
    FemError::InvalidValue {
        key: "temperature",
        reason,
    }
}

/// The shift factor `a_T` (ADR 0485 arithmetic 4): WLF `log10 a_T = -c1 (T - T_ref)/(c2 + T - T_ref)` (refused for `c2 + T -
/// T_ref <= 0`), Arrhenius `ln a_T = (E_a/R)(1/T - 1/T_ref)` (refused for `T <= 0` or `T_ref <= 0`); a non-finite temperature
/// or result is refused (key `"temperature"`).
pub fn shift_factor(shift: &TimeTemperatureShift, temperature: f64) -> Result<f64> {
    if !temperature.is_finite() {
        return Err(temperature_error(format!("{temperature:e} K is not finite")));
    }
    let a = match *shift {
        TimeTemperatureShift::Wlf { reference, c1, c2 } => {
            let d = temperature - reference;
            if c2 + d <= 0.0 {
                return Err(temperature_error(format!(
                    "{temperature:e} K: the WLF shift needs c2 + T - T_ref > 0 (c2 = {c2:e} K, T_ref = {reference:e} K)"
                )));
            }
            10f64.powf(-c1 * d / (c2 + d))
        }
        TimeTemperatureShift::Arrhenius { reference, activation } => {
            if !(temperature > 0.0 && reference > 0.0) {
                return Err(temperature_error(format!(
                    "{temperature:e} K with T_ref = {reference:e} K: the Arrhenius shift needs positive temperatures"
                )));
            }
            ((activation / GAS_CONSTANT) * (1.0 / temperature - 1.0 / reference)).exp()
        }
    };
    if !(a.is_finite() && a > 0.0) {
        return Err(temperature_error(format!(
            "{temperature:e} K gives the shift factor {a:e} (must be positive and finite)"
        )));
    }
    Ok(a)
}

/// The parts of a viscoelastic material the update needs.
struct Parts<'a> {
    e: f64,
    nu: f64,
    terms: &'a [PronyTerm],
    shift: Option<&'a TimeTemperatureShift>,
}

fn parts(material: &Material) -> Result<Parts<'_>> {
    match material {
        Material::Viscoelastic { base, terms, shift } => match base.as_ref() {
            Material::IsotropicElastic { e, nu, .. } => Ok(Parts {
                e: *e,
                nu: *nu,
                terms,
                shift: shift.as_ref(),
            }),
            _ => Err(FemError::NotYetSupported {
                item: "material",
                variant: material.name(),
                location: "an orthotropic base".to_string(),
                package: FOLLOW_UP,
            }),
        },
        other => Err(FemError::InvalidValue {
            key: "material",
            reason: format!(
                "a {} material: the Prony update of ADR 0485 takes a Viscoelastic material",
                other.name()
            ),
        }),
    }
}

/// The shift factor of a material at the point context: 1 without a shift; with one, the point's temperature is needed
/// (key `"temperature"`).
fn point_shift(shift: Option<&TimeTemperatureShift>, point: &PointContext) -> Result<f64> {
    match shift {
        None => Ok(1.0),
        Some(s) => {
            let t = point.temperature.ok_or_else(|| {
                temperature_error(
                    "a viscoelastic material with a time-temperature shift needs a temperature".to_string(),
                )
            })?;
            shift_factor(s, t)
        }
    }
}

/// The deviatoric (tensor shear) and the mean part of the base's elastic stress of a strain.
fn split(g0: f64, k0: f64, eps: &[f64; 6]) -> ([f64; 6], f64) {
    let tr = eps[0] + eps[1] + eps[2];
    let mut dev = [0.0; 6];
    for k in 0..3 {
        dev[k] = 2.0 * g0 * (eps[k] - tr / 3.0);
    }
    for k in 3..6 {
        dev[k] = g0 * eps[k];
    }
    (dev, k0 * tr)
}

/// The Prony update of `material` (a viscoelastic material) at the strain `strain` from the committed `state`, over the
/// step of the point context (ADR 0485 D1). A material without branches returns `apply(&isotropic_elasticity(e, nu),
/// strain)`, the bits of its base. Refuses another material (key `"material"`), an orthotropic base (`NotYetSupported`), a
/// state of the wrong length or with a non-finite value (key `"state"`) and a shifted material without a temperature (key
/// `"temperature"`).
pub fn update(material: &Material, strain: &[f64; 6], state: &[f64], point: &PointContext) -> Result<Update> {
    let p = parts(material)?;
    let n = p.terms.len();
    let size = state_size(n);
    if state.len() != size {
        return Err(FemError::FieldLength {
            key: "state",
            expected: size,
            found: state.len(),
        });
    }
    if let Some(i) = state.iter().position(|v| !v.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "state",
            reason: format!("viscoelastic state value {i} is not finite"),
        });
    }
    let d0 = isotropic_elasticity(p.e, p.nu);
    if n == 0 {
        return Ok(Update {
            stress: apply(&d0, strain),
            tangent: d0,
            state: Vec::new(),
        });
    }
    let a_t = point_shift(p.shift, point)?;
    let dxi = point.time_increment.unwrap_or(0.0) / a_t;
    let g0 = p.e / (2.0 * (1.0 + p.nu));
    let k0 = p.e / (3.0 * (1.0 - 2.0 * p.nu));
    let mut eps_n = [0.0; 6];
    eps_n.copy_from_slice(&state[..6]);
    let (dev1, vol1) = split(g0, k0, strain);
    let mut deps = [0.0; 6];
    for k in 0..6 {
        deps[k] = strain[k] - eps_n[k];
    }
    let (ddev, dvol) = split(g0, k0, &deps);
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(strain);
    let (mut sum_g, mut sum_k) = (0.0, 0.0);
    let (mut rel_g, mut rel_k) = (0.0, 0.0);
    let mut hsum = [0.0; 6];
    for (i, t) in p.terms.iter().enumerate() {
        let x = dxi / t.tau;
        let e = (-x).exp();
        let gamma = if x == 0.0 { 1.0 } else { -(-x).exp_m1() / x };
        let h = &state[6 * (i + 1)..6 * (i + 2)];
        for k in 0..6 {
            let inc = t.shear * ddev[k] + if k < 3 { t.bulk * dvol } else { 0.0 };
            let v = e * h[k] + gamma * inc;
            out.push(v);
            hsum[k] += v;
        }
        sum_g += t.shear;
        sum_k += t.bulk;
        rel_g += t.shear * (1.0 - gamma);
        rel_k += t.bulk * (1.0 - gamma);
    }
    let mut stress = [0.0; 6];
    for k in 0..6 {
        let vol = if k < 3 { (1.0 - sum_k) * vol1 } else { 0.0 };
        stress[k] = (1.0 - sum_g) * dev1[k] + vol + hsum[k];
    }
    let gt = g0 * (1.0 - rel_g);
    let kt = k0 * (1.0 - rel_k);
    let mut c = [[0.0; 6]; 6];
    for (i, row) in c.iter_mut().enumerate().take(3) {
        for (j, v) in row.iter_mut().enumerate().take(3) {
            *v = kt + 2.0 * gt * (if i == j { 1.0 } else { 0.0 } - 1.0 / 3.0);
        }
    }
    for (k, row) in c.iter_mut().enumerate().skip(3) {
        row[k] = gt;
    }
    Ok(Update {
        stress,
        tangent: c,
        state: out,
    })
}

/// The complex shear, bulk and Young's moduli of a viscoelastic material at the circular frequency `omega` (rad/s), at the
/// temperature `temperature` for a shifted material (ADR 0485 arithmetic 2): `G*(omega) = G_0 (g_inf + sum g_i i x_i/(1 + i
/// x_i))`, `x_i = omega a_T tau_i`, `K*` alike, `E* = 9 K* G*/(3 K* + G*)`. Refuses `omega` negative or not finite (key
/// `"frequency"`) and the refusals of [`update`].
pub fn complex_moduli(material: &Material, omega: f64, temperature: Option<f64>) -> Result<ComplexModuli> {
    let p = parts(material)?;
    if !(omega.is_finite() && omega >= 0.0) {
        return Err(FemError::InvalidValue {
            key: "frequency",
            reason: format!("{omega:e} rad/s (must be finite and >= 0)"),
        });
    }
    let mut point = PointContext::new();
    if let Some(t) = temperature {
        point = point.with_temperature(t)?;
    }
    let a_t = point_shift(p.shift, &point)?;
    let g0 = p.e / (2.0 * (1.0 + p.nu));
    let k0 = p.e / (3.0 * (1.0 - 2.0 * p.nu));
    let (mut gs, mut gl, mut ks, mut kl) = (1.0, 0.0, 1.0, 0.0);
    for t in p.terms {
        let x = omega * a_t * t.tau;
        let q = 1.0 + x * x;
        gs -= t.shear / q;
        ks -= t.bulk / q;
        gl += t.shear * x / q;
        kl += t.bulk * x / q;
    }
    let shear = C64::new(g0 * gs, g0 * gl);
    let bulk = C64::new(k0 * ks, k0 * kl);
    let young = (shear * bulk).scale(9.0) / (bulk.scale(3.0) + shear);
    Ok(ComplexModuli { shear, bulk, young })
}

/// The table of the storage modulus `E'` and the loss factor `E''/E'` of a viscoelastic material at the frequencies
/// `frequencies` in Hz, for the direct harmonic solve with the base material (`LossFactor::Table`, ADR 0485 D7). Only for
/// equal shear and bulk weights in every term (the table scales the whole elasticity matrix; refused otherwise, key
/// `"terms"`); the refusals of [`complex_moduli`] and of [`ModulusTable::new`].
pub fn modulus_table(material: &Material, frequencies: &[f64], temperature: Option<f64>) -> Result<ModulusTable> {
    let p = parts(material)?;
    if let Some(i) = p.terms.iter().position(|t| t.shear != t.bulk) {
        return Err(FemError::InvalidValue {
            key: "terms",
            reason: format!(
                "term {i} has the shear weight {:e} and the bulk weight {:e}: the modulus table scales the whole elasticity \
                 matrix and needs equal weights",
                p.terms[i].shear, p.terms[i].bulk
            ),
        });
    }
    let mut storage = Vec::with_capacity(frequencies.len());
    let mut loss = Vec::with_capacity(frequencies.len());
    for &f in frequencies {
        let y = complex_moduli(material, 2.0 * std::f64::consts::PI * f, temperature)?.young;
        storage.push(y.re);
        loss.push(y.im / y.re);
    }
    ModulusTable::new(frequencies.to_vec(), storage, loss)
}
