//! Hyperelastic energies (neo-Hookean, Mooney-Rivlin). Declared by ADR 0440 (F-408, the scaffold of ADR 0311); filled by ADR
//! 0541 (the registration of ADR 0328, F-425).
//!
//! * Energies (ADR 0541 D1, arithmetic 1): with `C = I + 2E`, `J = (det C)^(1/2)`, `Ibar1 = J^(-2/3) tr C`, `Ibar2 = J^(-4/3)
//!   I2(C)` and the volumetric function `U(J) = (kappa/2)(J - 1)^2`: neo-Hookean `W = (mu/2)(Ibar1 - 3) + U(J)` (the form of
//!   ADR 0523 D2), Mooney-Rivlin `W = c10 (Ibar1 - 3) + c01 (Ibar2 - 3) + U(J)`. At rest the tangent is linear elasticity with
//!   the shear modulus `mu` (`2 (c10 + c01)`) and the bulk modulus `kappa` ([`small_strain_moduli`]).
//! * [`energy`], [`energy_parts`], [`stress`] (the second Piola-Kirchhoff stress `S = 2 dW/dC`, Voigt, tensor shear) and
//!   [`stress_and_tangent`] (with `D = dS/dE` in the driver's layout, `S_I = D_IJ e_J`, engineering shear strains) of a strain
//!   (Voigt, engineering shear), evaluated so that no terms of order one cancel at small strain. A strain with `det C <= 0` is
//!   refused with the key [`crate::nonlinear::INVERSION_KEY`].
//! * Paths: [`crate::model::Material::stress`] gives the stress and the tangent; the nonlinear driver takes the material
//!   (Full and BBar Hex8, the BBar block being the mean-dilatation element, [`crate::nonlinear_md`]); the linear paths and the
//!   nonlinear transient Method step refuse it naming [`FOLLOW_UP`].

use crate::error::{FemError, Result};
use crate::material::{HyperelasticEnergy, Matrix6};

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// The package named by the paths that do not take the hyperelastic material (ADR 0541 D2 to D4).
pub const FOLLOW_UP: &str = "a follow-up of ADR 0541";

/// The Voigt pairs of the driver: `(0,0), (1,1), (2,2), (0,1), (1,2), (2,0)`.
const PAIRS: [(usize, usize); 6] = [(0, 0), (1, 1), (2, 2), (0, 1), (1, 2), (2, 0)];

/// `(c10, c01, kappa)` of an energy (neo-Hookean: `c10 = mu/2`, `c01 = 0`).
fn constants(energy: &HyperelasticEnergy) -> (f64, f64, f64) {
    match *energy {
        HyperelasticEnergy::NeoHookean { mu, kappa } => (0.5 * mu, 0.0, kappa),
        HyperelasticEnergy::MooneyRivlin { c10, c01, kappa } => (c10, c01, kappa),
    }
}

/// The small-strain shear and bulk moduli `(G, K)` of an energy: `(mu, kappa)`, Mooney-Rivlin `(2 (c10 + c01), kappa)`.
pub fn small_strain_moduli(energy: &HyperelasticEnergy) -> (f64, f64) {
    let (c10, c01, kappa) = constants(energy);
    (2.0 * (c10 + c01), kappa)
}

/// The kinematic quantities of a strain.
struct Kin {
    /// `E` (tensor).
    e: [[f64; 3]; 3],
    /// `C = I + 2E`.
    c: [[f64; 3]; 3],
    /// `C^(-1)`.
    ci: [[f64; 3]; 3],
    /// `tr E`.
    t: f64,
    /// `tr E^2`.
    q: f64,
    /// `ln J`.
    ln_j: f64,
    /// `J`.
    j: f64,
    /// `J - 1`.
    jm1: f64,
}

fn det3(a: &[[f64; 3]; 3]) -> f64 {
    a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0])
}

fn kin(strain: &[f64; 6]) -> Result<Kin> {
    if let Some(i) = strain.iter().position(|v| !v.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "strain",
            reason: format!("component {i} is not finite"),
        });
    }
    let mut e = [[0.0; 3]; 3];
    for (m, &(a, b)) in PAIRS.iter().enumerate() {
        let v = if m < 3 { strain[m] } else { 0.5 * strain[m] };
        e[a][b] = v;
        e[b][a] = v;
    }
    let t = e[0][0] + e[1][1] + e[2][2];
    let mut q = 0.0;
    for row in &e {
        for v in row {
            q += v * v;
        }
    }
    // det(I + 2E) - 1 = 2 tr E + 4 i2(E) + 8 det E, without the subtraction (ADR 0541 D1).
    let dcm1 = 2.0 * t + 2.0 * (t * t - q) + 8.0 * det3(&e);
    let det_c = 1.0 + dcm1;
    if !(det_c > 0.0 && det_c.is_finite()) {
        return Err(FemError::InvalidValue {
            key: crate::nonlinear::INVERSION_KEY,
            reason: format!("det C = {det_c:e} (inverted): the hyperelastic energy needs det C > 0"),
        });
    }
    let ln_j = 0.5 * dcm1.ln_1p();
    let mut c = e;
    for (i, row) in c.iter_mut().enumerate() {
        for v in row.iter_mut() {
            *v *= 2.0;
        }
        row[i] += 1.0;
    }
    let mut ci = [[0.0; 3]; 3];
    for (r, row) in ci.iter_mut().enumerate() {
        for (k, v) in row.iter_mut().enumerate() {
            let (r1, r2) = ((k + 1) % 3, (k + 2) % 3);
            let (c1, c2) = ((r + 1) % 3, (r + 2) % 3);
            *v = (c[r1][c1] * c[r2][c2] - c[r1][c2] * c[r2][c1]) / det_c;
        }
    }
    Ok(Kin {
        e,
        c,
        ci,
        t,
        q,
        ln_j,
        j: ln_j.exp(),
        jm1: ln_j.exp_m1(),
    })
}

/// The isochoric and the volumetric part of the energy at a strain (ADR 0541 D1): `(c10 (Ibar1 - 3) + c01 (Ibar2 - 3),
/// (kappa/2)(J - 1)^2)`.
pub fn energy_parts(energy: &HyperelasticEnergy, strain: &[f64; 6]) -> Result<(f64, f64)> {
    let k = kin(strain)?;
    let (c10, c01, kappa) = constants(energy);
    let (t, q) = (k.t, k.q);
    let a = (-2.0 * k.ln_j / 3.0).exp();
    let am1 = (-2.0 * k.ln_j / 3.0).exp_m1();
    let b = (-4.0 * k.ln_j / 3.0).exp();
    let bm1 = (-4.0 * k.ln_j / 3.0).exp_m1();
    let ibar1m3 = 3.0 * am1 + 2.0 * t * a;
    let ibar2m3 = 3.0 * bm1 + b * (4.0 * t + 2.0 * t * t - 2.0 * q);
    let mut iso = c10 * ibar1m3;
    if c01 != 0.0 {
        iso += c01 * ibar2m3;
    }
    Ok((iso, 0.5 * kappa * k.jm1 * k.jm1))
}

/// The energy per unit reference volume at a strain (Voigt, engineering shear).
pub fn energy(energy: &HyperelasticEnergy, strain: &[f64; 6]) -> Result<f64> {
    let (iso, vol) = energy_parts(energy, strain)?;
    Ok(iso + vol)
}

fn stress_of(k: &Kin, c10: f64, c01: f64, kappa: f64) -> [f64; 6] {
    let (t, q) = (k.t, k.q);
    let a = (-2.0 * k.ln_j / 3.0).exp();
    let b = (-4.0 * k.ln_j / 3.0).exp();
    // M1 = C - (I1/3) I = 2 dev E; M2 = I1 C - C^2 - (2/3) I2 I = 2 dev E + 4 (t E - E^2) - (4/3)(t^2 - q) I.
    let mut m1 = [[0.0; 3]; 3];
    let mut m2 = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let delta = if i == j { 1.0 } else { 0.0 };
            let dev = k.e[i][j] - delta * t / 3.0;
            let e2: f64 = (0..3).map(|m| k.e[i][m] * k.e[m][j]).sum();
            m1[i][j] = 2.0 * dev;
            m2[i][j] = 2.0 * dev + 4.0 * (t * k.e[i][j] - e2) - delta * (4.0 / 3.0) * (t * t - q);
        }
    }
    let vol = kappa * k.jm1 * k.j;
    let mut st = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let mut p1 = 0.0;
            let mut p2 = 0.0;
            for m in 0..3 {
                p1 += k.ci[i][m] * m1[m][j];
                p2 += k.ci[i][m] * m2[m][j];
            }
            let mut v = 2.0 * c10 * a * p1 + vol * k.ci[i][j];
            if c01 != 0.0 {
                v += 2.0 * c01 * b * p2;
            }
            st[i][j] = v;
        }
    }
    let mut s = [0.0; 6];
    for (m, &(i, j)) in PAIRS.iter().enumerate() {
        s[m] = 0.5 * (st[i][j] + st[j][i]);
    }
    s
}

/// The second Piola-Kirchhoff stress `S = 2 dW/dC` (Voigt, tensor shear) at a strain (Voigt, engineering shear).
pub fn stress(energy: &HyperelasticEnergy, strain: &[f64; 6]) -> Result<[f64; 6]> {
    let k = kin(strain)?;
    let (c10, c01, kappa) = constants(energy);
    Ok(stress_of(&k, c10, c01, kappa))
}

/// The stress and the tangent `D = dS/dE = 4 d^2W/dC dC` in the driver's Voigt layout (ADR 0541 arithmetic 1).
pub fn stress_and_tangent(energy: &HyperelasticEnergy, strain: &[f64; 6]) -> Result<([f64; 6], Matrix6)> {
    let k = kin(strain)?;
    let (c10, c01, kappa) = constants(energy);
    let s = stress_of(&k, c10, c01, kappa);
    let (t, q) = (k.t, k.q);
    let a = (-2.0 * k.ln_j / 3.0).exp();
    let b = (-4.0 * k.ln_j / 3.0).exp();
    let i1 = 3.0 + 2.0 * t;
    let i2 = 3.0 + 4.0 * t + 2.0 * t * t - 2.0 * q;
    let (ci, c) = (&k.ci, &k.c);
    let delta = |i: usize, j: usize| if i == j { 1.0 } else { 0.0 };
    let f1 = 4.0 * c10 * a;
    let f2 = 4.0 * c01 * b;
    let v_cc = kappa * k.j * (1.0 + 2.0 * k.jm1);
    let v_ii = -2.0 * kappa * k.j * k.jm1;
    let mut d = [[0.0; 6]; 6];
    for (r, &(i, j)) in PAIRS.iter().enumerate() {
        for (col, &(kk, l)) in PAIRS.iter().enumerate() {
            let ici = delta(i, j) * ci[kk][l] + ci[i][j] * delta(kk, l);
            let cici = ci[i][j] * ci[kk][l];
            let sym_ci = 0.5 * (ci[i][kk] * ci[j][l] + ci[i][l] * ci[j][kk]);
            let mut v = f1 * (-ici / 3.0 + i1 / 9.0 * cici + i1 / 3.0 * sym_ci) + v_cc * cici + v_ii * sym_ci;
            if c01 != 0.0 {
                let ii = delta(i, j) * delta(kk, l);
                let sym_i = 0.5 * (delta(i, kk) * delta(j, l) + delta(i, l) * delta(j, kk));
                let cci = c[i][j] * ci[kk][l] + ci[i][j] * c[kk][l];
                v += f2
                    * (ii - sym_i - (2.0 / 3.0) * i1 * ici
                        + (2.0 / 3.0) * cci
                        + (4.0 / 9.0) * i2 * cici
                        + (2.0 / 3.0) * i2 * sym_ci);
            }
            d[r][col] = v;
        }
    }
    Ok((s, d))
}
