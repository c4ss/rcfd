//! The discrete gradient of a stored energy in the energy-momentum step (ADR 0523; O. Gonzalez, J. Nonlinear Sci. 6 (1996)
//! 449 and Comput. Methods Appl. Mech. Engrg. 190 (2000) 1763, cited from memory, the identity derived in the ADR).
//!
//! At a point with the Green-Lagrange strains `E_n`, `E_{n+1}` (Voigt, engineering shear) the algorithmic stress is
//! `S_alg = S(E_m) + (nu / dE:dE) dE`, `E_m = (E_n + E_{n+1})/2`, `dE = E_{n+1} - E_n`, `nu = W(E_{n+1}) - W(E_n) - S(E_m):dE`,
//! so that `S_alg : dE = W(E_{n+1}) - W(E_n)` exactly (D3). The correction is taken when `|dE| > DELTA` and `|nu|` exceeds the
//! rounding bound `64 eps` times the term magnitudes; otherwise `S_alg = S(E_m)` (the midpoint branch, whose energy error is
//! that rounding). For St. Venant-Kirchhoff `W` is quadratic and `nu = 0`: `S_alg = D E_m`, the F-570 average.
//!
//! The compressible neo-Hookean energy of D2, private to this step until F-425 (ADR 0328) lands:
//! `W = (mu/2) (J^{-2/3} I_1 - 3) + (kappa/2) (J - 1)^2`, evaluated in `E` without cancellation at small strain.
//!
//! [`AlgorithmicStress::MaterialDiscreteGradient`] (ADR 0544) takes the same discrete gradient with F-425's point functions
//! (`mat_hyper`: the neo-Hookean and the Mooney-Rivlin energy); the step-local energy above stays on `DiscreteGradient` and
//! `Average` (equal to F-425's neo-Hookean within rounding, ADR 0544 E1).

use super::invalid;
use crate::error::{FemError, Result};
use crate::mat_hyper;
use crate::material::{HyperelasticEnergy, Matrix6, PointContext};
use crate::model::{Material, SolidModel};
use crate::nonlinear;

/// The registration named in the refusals of this module.
pub(super) const ADR: &str = "ADR 0523";

/// The threshold on the tensor norm of `dE` below which the midpoint branch is taken (ADR 0523 D3).
pub(super) const DELTA: f64 = 1e-12;

/// The factor of the rounding bound of `nu` (ADR 0523 D3).
pub(super) const BETA_FACTOR: f64 = 64.0;

/// The algorithmic stress of the energy-momentum step (ADR 0523 D4); read by [`super::StepKind::EnergyMomentum`] only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum AlgorithmicStress {
    /// The F-570 step in every respect (ADR 0514, the same bits): the average `(S_n + S_{n+1})/2` of the isotropic elastic
    /// material; a hyperelastic material refused as before (ADR 0523 amendment 1); the default.
    #[default]
    Auto,
    /// The discrete gradient of Gonzalez for every material (ADR 0523 D3); takes the neo-Hookean material.
    DiscreteGradient,
    /// The average `(S(E_n) + S(E_{n+1}))/2` for every material (takes the neo-Hookean material); it does not conserve the
    /// energy of a non-quadratic energy (the negative control of ADR 0523).
    Average,
    /// The discrete gradient of ADR 0523 D3 with the hyperelastic energies of F-425 (`mat_hyper`, ADR 0541 D1): takes the
    /// neo-Hookean and the Mooney-Rivlin material; the isotropic elastic material as `DiscreteGradient` (ADR 0544 D1).
    MaterialDiscreteGradient,
}

/// The energy, the magnitude of its terms and the stress (Voigt) at one strain.
#[derive(Debug, Clone, Copy)]
pub(super) struct Point {
    pub(super) w: f64,
    pub(super) m: f64,
    pub(super) s: [f64; 6],
}

/// The compressible neo-Hookean energy with the small-strain shear modulus `mu` and bulk modulus `kappa` (ADR 0523 D2).
#[derive(Debug, Clone, Copy)]
pub(super) struct NeoHookean {
    pub(super) mu: f64,
    pub(super) kappa: f64,
}

/// The invariants of one strain.
struct Kin {
    /// `C^{-1}`.
    ci: [[f64; 3]; 3],
    /// `E` as a tensor.
    e: [[f64; 3]; 3],
    t: f64,
    q: f64,
    i3: f64,
    /// `J^{2/3} - 1`.
    g: f64,
    /// `J`.
    j: f64,
    /// `J - 1`.
    jm1: f64,
}

fn tensor(e: &[f64; 6]) -> [[f64; 3]; 3] {
    [
        [e[0], 0.5 * e[3], 0.5 * e[5]],
        [0.5 * e[3], e[1], 0.5 * e[4]],
        [0.5 * e[5], 0.5 * e[4], e[2]],
    ]
}

/// The Voigt pairs of the driver's order `[00, 11, 22, 01, 12, 20]`.
const PAIRS: [(usize, usize); 6] = [(0, 0), (1, 1), (2, 2), (0, 1), (1, 2), (2, 0)];

fn det3(a: &[[f64; 3]; 3]) -> f64 {
    a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0])
}

fn kin(strain: &[f64; 6]) -> Result<Kin> {
    let e = tensor(strain);
    let t = e[0][0] + e[1][1] + e[2][2];
    let mut q = 0.0;
    for row in &e {
        for x in row {
            q += x * x;
        }
    }
    let i3 = det3(&e);
    let d = 2.0 * t + 2.0 * (t * t - q) + 8.0 * i3;
    let det_c = 1.0 + d;
    if !(det_c > 0.0 && det_c.is_finite()) {
        return Err(FemError::InvalidValue {
            key: nonlinear::INVERSION_KEY,
            reason: format!("det C = {det_c:e} at a point of the neo-Hookean material (inverted; {ADR})"),
        });
    }
    let g = (d.ln_1p() / 3.0).exp_m1();
    let j = det_c.sqrt();
    let jm1 = d / (1.0 + j);
    let mut c = [[0.0; 3]; 3];
    for a in 0..3 {
        for b in 0..3 {
            c[a][b] = 2.0 * e[a][b];
        }
        c[a][a] += 1.0;
    }
    let mut ci = [[0.0; 3]; 3];
    for a in 0..3 {
        for b in 0..3 {
            let (a1, a2) = ((b + 1) % 3, (b + 2) % 3);
            let (b1, b2) = ((a + 1) % 3, (a + 2) % 3);
            ci[a][b] = (c[a1][b1] * c[a2][b2] - c[a1][b2] * c[a2][b1]) / det_c;
        }
    }
    Ok(Kin {
        ci,
        e,
        t,
        q,
        i3,
        g,
        j,
        jm1,
    })
}

impl NeoHookean {
    /// The point functions of a hyperelastic energy; refuses the Mooney-Rivlin energy (ADR 0523 D6).
    pub(super) fn of(energy: &HyperelasticEnergy) -> Result<NeoHookean> {
        match energy {
            HyperelasticEnergy::NeoHookean { mu, kappa } => Ok(NeoHookean { mu: *mu, kappa: *kappa }),
            other => Err(invalid(
                "nl_transient",
                format!(
                    "hyperelastic energy {other:?}: the energy-momentum step takes the neo-Hookean energy only ({ADR})"
                ),
            )),
        }
    }

    /// `W`, `m(W)` and `S` at a strain (ADR 0523 D2).
    pub(super) fn point(&self, strain: &[f64; 6]) -> Result<Point> {
        let k = kin(strain)?;
        let (mu, kappa) = (self.mu, self.kappa);
        let (t, q, i3, g) = (k.t, k.q, k.i3, k.g);
        let vol = 0.5 * kappa * k.jm1 * k.jm1;
        let w = 0.5 * mu * (2.0 * q - 2.0 * t * t - 8.0 * i3 + 3.0 * g * g + g * g * g) / (1.0 + g) + vol;
        let m = 0.5 * mu * (2.0 * q + 2.0 * t * t + 8.0 * i3.abs() + 3.0 * g * g + (g * g * g).abs()) / (1.0 + g) + vol;
        let mut dev = k.e;
        for (a, row) in dev.iter_mut().enumerate() {
            row[a] -= t / 3.0;
        }
        let iso = mu / (1.0 + g);
        let volc = kappa * k.jm1 * k.j;
        let mut st = [[0.0; 3]; 3];
        for a in 0..3 {
            for b in 0..3 {
                let mut p = 0.0;
                for (c, devc) in dev[a].iter().enumerate() {
                    p += devc * k.ci[c][b];
                }
                st[a][b] = iso * 2.0 * p + volc * k.ci[a][b];
            }
        }
        let mut s = [0.0; 6];
        for (i, &(a, b)) in PAIRS.iter().enumerate() {
            s[i] = 0.5 * (st[a][b] + st[b][a]);
        }
        Ok(Point { w, m, s })
    }

    /// The energy alone.
    pub(super) fn energy(&self, strain: &[f64; 6]) -> Result<f64> {
        Ok(self.point(strain)?.w)
    }

    /// The tangent `2 dS/dC` in the Voigt layout of the driver (`S_I = D_IJ e_J`, ADR 0523 D2).
    pub(super) fn tangent(&self, strain: &[f64; 6]) -> Result<Matrix6> {
        let k = kin(strain)?;
        let (mu, kappa) = (self.mu, self.kappa);
        let jj = 1.0 / (1.0 + k.g);
        let i1 = 3.0 + 2.0 * k.t;
        let ci = &k.ci;
        let delta = |a: usize, b: usize| if a == b { 1.0 } else { 0.0 };
        let a_iso = -2.0 * mu * jj / 3.0;
        let c_cc = kappa * k.j * (2.0 * k.j - 1.0);
        let c_ii = -2.0 * kappa * k.j * k.jm1;
        let mut d = [[0.0; 6]; 6];
        for (r, &(i, j)) in PAIRS.iter().enumerate() {
            for (c, &(kk, l)) in PAIRS.iter().enumerate() {
                let ii = 0.5 * (ci[i][kk] * ci[j][l] + ci[i][l] * ci[j][kk]);
                let cc = ci[i][j] * ci[kk][l];
                let iso = a_iso * (delta(i, j) * ci[kk][l] + ci[i][j] * delta(kk, l) - i1 / 3.0 * cc - i1 * ii);
                d[r][c] = iso + c_cc * cc + c_ii * ii;
            }
        }
        Ok(d)
    }
}

/// The St. Venant-Kirchhoff point functions of an isotropic elastic material: `S = D E`, `W = S . E/2`, `m = sum |S_i e_i|/2`.
pub(super) fn svk_point(material: &Material, context: &PointContext, strain: &[f64; 6]) -> Result<(Point, Matrix6)> {
    let (s, d) = material.stress(strain, context, &[])?;
    let (mut w, mut m) = (0.0, 0.0);
    for i in 0..6 {
        w += s[i] * strain[i];
        m += (s[i] * strain[i]).abs();
    }
    Ok((
        Point {
            w: 0.5 * w,
            m: 0.5 * m,
            s,
        },
        d,
    ))
}

/// The discrete gradient (ADR 0523 D3): `S_alg`, the midpoint strain `E_m` and whether the correction branch was taken.
pub(super) fn discrete_gradient<P>(e0: &[f64; 6], e1: &[f64; 6], point: P) -> Result<([f64; 6], [f64; 6], bool)>
where
    P: Fn(&[f64; 6]) -> Result<Point>,
{
    let mut em = [0.0; 6];
    let mut de = [0.0; 6];
    for i in 0..6 {
        em[i] = 0.5 * (e0[i] + e1[i]);
        de[i] = e1[i] - e0[i];
    }
    let p0 = point(e0)?;
    let p1 = point(e1)?;
    let pm = point(&em)?;
    let n2 = de[0] * de[0] + de[1] * de[1] + de[2] * de[2] + 0.5 * (de[3] * de[3] + de[4] * de[4] + de[5] * de[5]);
    let (mut work, mut work_abs) = (0.0, 0.0);
    for i in 0..6 {
        work += pm.s[i] * de[i];
        work_abs += (pm.s[i] * de[i]).abs();
    }
    let nu = p1.w - p0.w - work;
    let beta = BETA_FACTOR * f64::EPSILON * (p0.m + p1.m + work_abs);
    let mut s = pm.s;
    let corrected = n2.sqrt() > DELTA && nu.abs() > beta;
    if corrected {
        let c = nu / n2;
        for i in 0..3 {
            s[i] += c * de[i];
        }
        for i in 3..6 {
            s[i] += c * 0.5 * de[i];
        }
    }
    Ok((s, em, corrected))
}

/// The algorithmic stress and the material tangent of a neo-Hookean point (ADR 0523 D3, D4).
pub(super) fn hyperelastic_stress(
    nh: &NeoHookean,
    e0: &[f64; 6],
    e1: &[f64; 6],
    mode: AlgorithmicStress,
) -> Result<([f64; 6], Matrix6)> {
    if mode == AlgorithmicStress::Average {
        let s0 = nh.point(e0)?.s;
        let s1 = nh.point(e1)?.s;
        let mut sa = [0.0; 6];
        for m in 0..6 {
            sa[m] = 0.5 * (s0[m] + s1[m]);
        }
        return Ok((sa, nh.tangent(e1)?));
    }
    let (sa, em, _) = discrete_gradient(e0, e1, |e| nh.point(e))?;
    Ok((sa, nh.tangent(&em)?))
}

/// The discrete gradient of the St. Venant-Kirchhoff energy of an isotropic elastic material, with its `D` (ADR 0523 D3).
pub(super) fn elastic_discrete_gradient(
    material: &Material,
    context: &PointContext,
    e0: &[f64; 6],
    e1: &[f64; 6],
) -> Result<([f64; 6], Matrix6)> {
    let (sa, em, _) = discrete_gradient(e0, e1, |e| Ok(svk_point(material, context, e)?.0))?;
    let (_, d) = svk_point(material, context, &em)?;
    Ok((sa, d))
}

/// The point functions of F-425's energy (ADR 0544 D1, D2): `W`, its term magnitude and `S` from `mat_hyper`.
pub(super) fn material_point(energy: &HyperelasticEnergy, strain: &[f64; 6]) -> Result<Point> {
    Ok(Point {
        w: mat_hyper::energy(energy, strain)?,
        m: mat_hyper::energy_magnitude(energy, strain)?,
        s: mat_hyper::stress(energy, strain)?,
    })
}

/// The algorithmic stress (ADR 0523 D3) and the tangent at `E_m` of a point of F-425's energy (ADR 0544 D1).
pub(super) fn material_stress(
    energy: &HyperelasticEnergy,
    e0: &[f64; 6],
    e1: &[f64; 6],
) -> Result<([f64; 6], Matrix6)> {
    let (sa, em, _) = discrete_gradient(e0, e1, |e| material_point(energy, e))?;
    let (_, d) = mat_hyper::stress_and_tangent(energy, &em)?;
    Ok((sa, d))
}

/// The surrogate of a model with a hyperelastic material for the energy-momentum step (ADR 0523 D5): each neo-Hookean
/// material replaced by the isotropic elastic material of its small-strain moduli; `None` without a hyperelastic material;
/// a Mooney-Rivlin material refused (key `"nl_transient"`) unless the stress is `MaterialDiscreteGradient`, which takes
/// either energy through `mat_hyper::small_strain_moduli` (ADR 0544 D3).
pub(super) fn surrogate(model: &SolidModel, mode: AlgorithmicStress) -> Result<Option<SolidModel>> {
    if !model
        .materials()
        .iter()
        .any(|(_, m)| matches!(m, Material::Hyperelastic { .. }))
    {
        return Ok(None);
    }
    let mut materials = Vec::with_capacity(model.materials().len());
    for (set, m) in model.materials() {
        let replaced = match m {
            Material::Hyperelastic { energy, density } if mode == AlgorithmicStress::MaterialDiscreteGradient => {
                let (mu, kappa) = mat_hyper::small_strain_moduli(energy);
                let e = 9.0 * kappa * mu / (3.0 * kappa + mu);
                let nu = (3.0 * kappa - 2.0 * mu) / (2.0 * (3.0 * kappa + mu));
                Material::isotropic_elastic(e, nu, *density, 0.0)?
            }
            Material::Hyperelastic { energy, density } => {
                let nh = NeoHookean::of(energy).map_err(|e| match e {
                    FemError::InvalidValue { key, reason } => FemError::InvalidValue {
                        key,
                        reason: format!("material of the set '{set}': {reason}"),
                    },
                    other => other,
                })?;
                let (mu, kappa) = (nh.mu, nh.kappa);
                let e = 9.0 * kappa * mu / (3.0 * kappa + mu);
                let nu = (3.0 * kappa - 2.0 * mu) / (2.0 * (3.0 * kappa + mu));
                Material::isotropic_elastic(e, nu, *density, 0.0)?
            }
            other => other.clone(),
        };
        materials.push((set.clone(), replaced));
    }
    Ok(Some(model.with_materials(materials)))
}

/// The neo-Hookean energy of a hyperelastic material at a strain (for `Integrator::strain_energy`, ADR 0523 D5).
pub(super) fn energy(energy: &HyperelasticEnergy, strain: &[f64; 6]) -> Result<f64> {
    NeoHookean::of(energy)?.energy(strain)
}

#[cfg(test)]
mod tests {
    //! Checks P1 to P6 of ADR 0523 (the point functions). Every aggregated value is required finite.
    use super::*;

    /// A deterministic generator of standard normal numbers (xorshift64* and Box-Muller).
    struct Normal(u64);

    impl Normal {
        fn uniform(&mut self) -> f64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            let x = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
            ((x >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        }
        fn next(&mut self) -> f64 {
            let (a, b) = (self.uniform(), self.uniform());
            (-2.0 * a.ln()).sqrt() * (2.0 * std::f64::consts::PI * b).cos()
        }
        fn strain(&mut self, scale: f64) -> [f64; 6] {
            let mut e = [0.0; 6];
            for x in e.iter_mut() {
                *x = scale * self.next();
            }
            e
        }
    }

    fn worst(acc: &mut f64, v: f64, what: &str) {
        assert!(v.is_finite(), "{what}: non-finite {v}");
        *acc = acc.max(v);
    }

    fn inf(x: &[f64]) -> f64 {
        x.iter().fold(0.0f64, |m, v| m.max(v.abs()))
    }

    fn tensor_norm(e: &[f64; 6]) -> f64 {
        (e[0] * e[0] + e[1] * e[1] + e[2] * e[2] + 0.5 * (e[3] * e[3] + e[4] * e[4] + e[5] * e[5])).sqrt()
    }

    fn bar_nh() -> NeoHookean {
        let (e, nu) = (2.0e9, 0.3);
        NeoHookean {
            mu: e / (2.0 * (1.0 + nu)),
            kappa: e / (3.0 * (1.0 - 2.0 * nu)),
        }
    }

    fn cube_nh() -> NeoHookean {
        NeoHookean { mu: 1e5, kappa: 1e6 }
    }

    /// Random pairs that do not invert (a pair with `det C <= 0` at an end is drawn again).
    fn pair(rng: &mut Normal, nh: &NeoHookean, scale: f64, change: f64) -> ([f64; 6], [f64; 6]) {
        loop {
            let e0 = rng.strain(scale);
            let d = rng.strain(scale * change);
            let mut e1 = e0;
            for i in 0..6 {
                e1[i] += d[i];
            }
            if nh.point(&e0).is_ok() && nh.point(&e1).is_ok() {
                return (e0, e1);
            }
        }
    }

    #[test]
    fn p1_p2_identity_and_svk_reduction() {
        let mut rng = Normal(0x9E37_79B9_7F4A_7C15);
        let nh = bar_nh();
        let material = Material::isotropic_elastic(2.0e9, 0.3, 7850.0, 0.0).unwrap();
        let ctx = PointContext::default();
        let (mut p1, mut p2) = (0.0, 0.0);
        let mut corrected = 0;
        for scale in [1e-6, 1e-4, 1e-2, 1e-1] {
            for _ in 0..200 {
                let (e0, e1) = pair(&mut rng, &nh, scale, 0.3);
                let (sa, _, c) = discrete_gradient(&e0, &e1, |e| nh.point(e)).unwrap();
                corrected += usize::from(c);
                let (a, b) = (nh.point(&e0).unwrap(), nh.point(&e1).unwrap());
                let (mut work, mut work_abs) = (0.0, 0.0);
                for i in 0..6 {
                    work += sa[i] * (e1[i] - e0[i]);
                    work_abs += (sa[i] * (e1[i] - e0[i])).abs();
                }
                worst(&mut p1, (work - (b.w - a.w)).abs() / (work_abs + a.m + b.m), "P1");
                let (sdg, _) = elastic_discrete_gradient(&material, &ctx, &e0, &e1).unwrap();
                let s0 = material.stress(&e0, &ctx, &[]).unwrap().0;
                let s1 = material.stress(&e1, &ctx, &[]).unwrap().0;
                let avg: Vec<f64> = (0..6).map(|i| 0.5 * (s0[i] + s1[i])).collect();
                let diff: Vec<f64> = (0..6).map(|i| sdg[i] - avg[i]).collect();
                worst(&mut p2, inf(&diff) / inf(&avg), "P2");
            }
        }
        println!("P1 identity: {p1:e} (limit 1e-12); corrected {corrected} of 800");
        println!("P2 St. Venant-Kirchhoff reduction: {p2:e} (limit 1e-12)");
        assert!(p1 <= 1e-12, "P1 {p1:e}");
        assert!(p2 <= 1e-12, "P2 {p2:e}");
    }

    fn with_change(rng: &mut Normal, e0: &[f64; 6], size: f64) -> [f64; 6] {
        let mut d = rng.strain(1.0);
        let n = tensor_norm(&d);
        let mut e1 = *e0;
        for i in 0..6 {
            d[i] *= size / n;
            e1[i] += d[i];
        }
        e1
    }

    #[test]
    fn p3_branch_at_small_changes() {
        let mut rng = Normal(0xD1B5_4A32_D192_ED03);
        let nh = cube_nh();
        // P3a: dE = 0 and |dE| = 1e-13 < DELTA: the midpoint stress bit for bit.
        let mut p3a = true;
        for size in [0.0, 1e-13] {
            for _ in 0..20 {
                let e0 = rng.strain(0.1);
                let e1 = if size == 0.0 {
                    e0
                } else {
                    with_change(&mut rng, &e0, size)
                };
                let (sa, em, c) = discrete_gradient(&e0, &e1, |e| nh.point(e)).unwrap();
                let sm = nh.point(&em).unwrap().s;
                p3a &= !c && sa.iter().zip(&sm).all(|(a, b)| a.to_bits() == b.to_bits());
            }
        }
        println!("P3a midpoint branch at dE = 0 and |dE| = 1e-13: {p3a}");
        // P3b: |dE| = 1e-8 at the base 0.1 (nu at rounding): the midpoint stress bit for bit.
        let mut p3b = true;
        for _ in 0..100 {
            let e0 = rng.strain(0.1);
            let e1 = with_change(&mut rng, &e0, 1e-8);
            let (sa, em, c) = discrete_gradient(&e0, &e1, |e| nh.point(e)).unwrap();
            let sm = nh.point(&em).unwrap().s;
            p3b &= !c && sa.iter().zip(&sm).all(|(a, b)| a.to_bits() == b.to_bits());
        }
        println!("P3b midpoint branch at |dE| = 1e-8, base 0.1: {p3b}");
        // P3c: the correction is of second order, |S_alg - S(E_m)| / |S(E_m)| <= 5 |dE|^2 / |E_m|.
        let mut p3c = 0.0;
        for (base, sizes) in [(0.1, [1e-5, 1e-4, 1e-3]), (1e-3, [1e-6, 1e-5, 1e-4])] {
            for size in sizes {
                for _ in 0..100 {
                    let e0 = rng.strain(base);
                    let e1 = with_change(&mut rng, &e0, size);
                    let (sa, em, _) = discrete_gradient(&e0, &e1, |e| nh.point(e)).unwrap();
                    let sm = nh.point(&em).unwrap().s;
                    let diff: Vec<f64> = (0..6).map(|i| sa[i] - sm[i]).collect();
                    let ratio = inf(&diff) / inf(&sm) / (size * size / tensor_norm(&em));
                    worst(&mut p3c, ratio, "P3c");
                }
            }
        }
        println!("P3c second order: {p3c:e} (limit 5)");
        assert!(p3a && p3b, "P3a {p3a}, P3b {p3b}");
        assert!(p3c <= 5.0, "P3c {p3c:e}");
    }

    #[test]
    fn p4_p5_p6_tangent_small_strain_and_stress() {
        let mut rng = Normal(0x94D0_49BB_1331_11EB);
        let (mut p4, mut p6) = (0.0, 0.0);
        for nh in [bar_nh(), cube_nh()] {
            for scale in [1e-3, 0.1] {
                for _ in 0..50 {
                    let (e, _) = pair(&mut rng, &nh, scale, 0.0);
                    let d = nh.tangent(&e).unwrap();
                    let s = nh.point(&e).unwrap().s;
                    let mut dmax = 0.0f64;
                    let mut ddiff = 0.0f64;
                    let mut sdiff = 0.0f64;
                    for c in 0..6 {
                        let hd = 1e-7 * scale;
                        let hw = 1e-6 * scale;
                        let (mut ep, mut em) = (e, e);
                        ep[c] += hd;
                        em[c] -= hd;
                        let (sp, sm) = (nh.point(&ep).unwrap().s, nh.point(&em).unwrap().s);
                        for r in 0..6 {
                            let fd = (sp[r] - sm[r]) / (2.0 * hd);
                            dmax = dmax.max(d[r][c].abs());
                            ddiff = ddiff.max((fd - d[r][c]).abs());
                        }
                        let (mut wp, mut wm) = (e, e);
                        wp[c] += hw;
                        wm[c] -= hw;
                        let fdw = (nh.energy(&wp).unwrap() - nh.energy(&wm).unwrap()) / (2.0 * hw);
                        sdiff = sdiff.max((fdw - s[c]).abs());
                    }
                    worst(&mut p4, ddiff / dmax, "P4");
                    worst(&mut p6, sdiff / inf(&s), "P6");
                }
            }
        }
        let nh = bar_nh();
        let lambda = nh.kappa - 2.0 * nh.mu / 3.0;
        let mut p5 = 0.0;
        for _ in 0..200 {
            let e = rng.strain(1e-6);
            let s = nh.point(&e).unwrap().s;
            let tr = e[0] + e[1] + e[2];
            let lin = [
                lambda * tr + 2.0 * nh.mu * e[0],
                lambda * tr + 2.0 * nh.mu * e[1],
                lambda * tr + 2.0 * nh.mu * e[2],
                nh.mu * e[3],
                nh.mu * e[4],
                nh.mu * e[5],
            ];
            let diff: Vec<f64> = (0..6).map(|i| s[i] - lin[i]).collect();
            worst(&mut p5, inf(&diff) / inf(&lin), "P5");
        }
        println!("P4 tangent against central differences: {p4:e} (limit 1e-6)");
        println!("P5 small strain against linear elasticity: {p5:e} (limit 1e-4)");
        println!("P6 stress against central differences of W: {p6:e} (limit 1e-6)");
        assert!(p4 <= 1e-6, "P4 {p4:e}");
        assert!(p5 <= 1e-4, "P5 {p5:e}");
        assert!(p6 <= 1e-6, "P6 {p6:e}");
    }
}

#[cfg(test)]
mod tests_adr0544 {
    //! Checks E1a, E1b, the record E1r (the step-local energy against F-425's at a point) and Q1 to Q4 (the discrete
    //! gradient with F-425's point functions) of ADR 0544. Every aggregated value is required finite.
    use super::*;

    /// A deterministic generator of standard normal numbers (xorshift64* and Box-Muller), as the tests of ADR 0523.
    struct Normal(u64);

    impl Normal {
        fn uniform(&mut self) -> f64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            let x = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
            ((x >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        }
        fn next(&mut self) -> f64 {
            let (a, b) = (self.uniform(), self.uniform());
            (-2.0 * a.ln()).sqrt() * (2.0 * std::f64::consts::PI * b).cos()
        }
        fn strain(&mut self, scale: f64) -> [f64; 6] {
            let mut e = [0.0; 6];
            for x in e.iter_mut() {
                *x = scale * self.next();
            }
            e
        }
    }

    fn worst(acc: &mut f64, v: f64, what: &str) {
        assert!(v.is_finite(), "{what}: non-finite {v}");
        *acc = acc.max(v);
    }

    fn inf(x: &[f64]) -> f64 {
        x.iter().fold(0.0f64, |m, v| m.max(v.abs()))
    }

    fn tensor_norm(e: &[f64; 6]) -> f64 {
        (e[0] * e[0] + e[1] * e[1] + e[2] * e[2] + 0.5 * (e[3] * e[3] + e[4] * e[4] + e[5] * e[5])).sqrt()
    }

    /// The neo-Hookean moduli of the bar and of the cube (ADR 0523 D7).
    fn moduli() -> [(f64, f64); 2] {
        let (e, nu) = (2.0e9, 0.3);
        [(e / (2.0 * (1.0 + nu)), e / (3.0 * (1.0 - 2.0 * nu))), (1e5, 1e6)]
    }

    /// The energies of Q1: NH and MR (`c10 = 0.35 G`, `c01 = 0.15 G`) at the bar and the cube moduli (ADR 0544 arithmetic 2).
    fn energies() -> Vec<HyperelasticEnergy> {
        let mut out = Vec::new();
        for (mu, kappa) in moduli() {
            out.push(HyperelasticEnergy::neo_hookean(mu, kappa).unwrap());
            out.push(HyperelasticEnergy::mooney_rivlin(0.35 * mu, 0.15 * mu, kappa).unwrap());
        }
        out
    }

    /// The cube's NH and MR energies (ADR 0544 D5).
    fn cube_energies() -> [HyperelasticEnergy; 2] {
        [
            HyperelasticEnergy::neo_hookean(1e5, 1e6).unwrap(),
            HyperelasticEnergy::mooney_rivlin(3.5e4, 1.5e4, 1e6).unwrap(),
        ]
    }

    fn admissible(energy: &HyperelasticEnergy, e: &[f64; 6]) -> bool {
        mat_hyper::energy(energy, e).is_ok()
    }

    #[test]
    fn e1_step_local_energy_equals_f425_at_a_point() {
        let mut rng = Normal(0x6A09_E667_F3BC_C908);
        let mut e1a = [0.0f64; 3];
        let mut e1b = [0.0f64; 3];
        let mut e1r = [0.0f64; 2];
        for (mu, kappa) in moduli() {
            let nh = NeoHookean { mu, kappa };
            let energy = HyperelasticEnergy::neo_hookean(mu, kappa).unwrap();
            for scale in [1e-6, 1e-4, 1e-3, 1e-2, 1e-1, 3e-1] {
                let mut n = 0;
                while n < 200 {
                    let e = rng.strain(scale);
                    if !admissible(&energy, &e) || nh.point(&e).is_err() {
                        continue;
                    }
                    n += 1;
                    let p = nh.point(&e).unwrap();
                    let d_local = nh.tangent(&e).unwrap();
                    let w = mat_hyper::energy(&energy, &e).unwrap();
                    let m = mat_hyper::energy_magnitude(&energy, &e).unwrap();
                    let s = mat_hyper::stress(&energy, &e).unwrap();
                    let (_, d) = mat_hyper::stress_and_tangent(&energy, &e).unwrap();
                    let ds: Vec<f64> = (0..6).map(|i| p.s[i] - s[i]).collect();
                    let rs = inf(&ds) / inf(&s);
                    let (mut dd, mut dmax) = (0.0f64, 0.0f64);
                    for r in 0..6 {
                        for c in 0..6 {
                            dd = dd.max((d_local[r][c] - d[r][c]).abs());
                            dmax = dmax.max(d[r][c].abs());
                        }
                    }
                    let rd = dd / dmax;
                    let rw = (p.w - w).abs() / w.abs();
                    if scale >= 1e-3 {
                        worst(&mut e1a[0], rw, "E1a W");
                        worst(&mut e1a[1], rs, "E1a S");
                        worst(&mut e1a[2], rd, "E1a D");
                    } else {
                        worst(&mut e1b[0], (p.w - w).abs() / m, "E1b W");
                        worst(&mut e1b[1], rs, "E1b S");
                        worst(&mut e1b[2], rd, "E1b D");
                        worst(&mut e1r[usize::from(scale > 1e-5)], rw, "E1r");
                    }
                }
            }
        }
        println!(
            "E1a W, S, D (scales 1e-3 .. 0.3): {:e} {:e} {:e} (limit 1e-12 each)",
            e1a[0], e1a[1], e1a[2]
        );
        println!(
            "E1b W/m_425, S, D (scales 1e-6, 1e-4): {:e} {:e} {:e} (limit 1e-12 each)",
            e1b[0], e1b[1], e1b[2]
        );
        println!("E1r (record) W/|W| at 1e-6, 1e-4: {:e} {:e}", e1r[0], e1r[1]);
        for v in e1a.iter().chain(&e1b) {
            assert!(*v <= 1e-12, "E1a {e1a:?}, E1b {e1b:?}");
        }
    }

    #[test]
    fn q1_q4_identity_and_magnitude_with_the_f425_energies() {
        let mut rng = Normal(0xBB67_AE85_84CA_A73B);
        let (mut q1, mut corrected, mut pairs) = (0.0, 0, 0);
        let mut q4 = true;
        for energy in energies() {
            for scale in [1e-6, 1e-4, 1e-2, 1e-1] {
                let mut n = 0;
                while n < 200 {
                    let e0 = rng.strain(scale);
                    let d = rng.strain(scale * 0.3);
                    let mut e1 = e0;
                    for i in 0..6 {
                        e1[i] += d[i];
                    }
                    if !admissible(&energy, &e0) || !admissible(&energy, &e1) {
                        continue;
                    }
                    n += 1;
                    pairs += 1;
                    let (sa, _, c) = discrete_gradient(&e0, &e1, |e| material_point(&energy, e)).unwrap();
                    corrected += usize::from(c);
                    let (a, b) = (
                        material_point(&energy, &e0).unwrap(),
                        material_point(&energy, &e1).unwrap(),
                    );
                    for p in [&a, &b] {
                        q4 &= p.m.is_finite() && p.m >= p.w.abs();
                    }
                    let (mut work, mut work_abs) = (0.0, 0.0);
                    for i in 0..6 {
                        work += sa[i] * (e1[i] - e0[i]);
                        work_abs += (sa[i] * (e1[i] - e0[i])).abs();
                    }
                    worst(&mut q1, (work - (b.w - a.w)).abs() / (work_abs + a.m + b.m), "Q1");
                }
            }
        }
        println!("Q1 identity with F-425's energies: {q1:e} (limit 1e-12); corrected {corrected} of {pairs}");
        println!("Q4 m_425 >= |W_425| and finite at every end: {q4}");
        assert!(q1 <= 1e-12, "Q1 {q1:e}");
        assert!(q4, "Q4");
    }

    fn with_change(rng: &mut Normal, e0: &[f64; 6], size: f64) -> [f64; 6] {
        let mut d = rng.strain(1.0);
        let n = tensor_norm(&d);
        let mut e1 = *e0;
        for i in 0..6 {
            d[i] *= size / n;
            e1[i] += d[i];
        }
        e1
    }

    #[test]
    fn q2_q3_branch_and_second_order_with_the_f425_energies() {
        let mut rng = Normal(0x3C6E_F372_FE94_F82B);
        let (mut q2a, mut q2b) = (true, true);
        let mut q3 = [0.0f64; 2];
        for (k, energy) in cube_energies().iter().enumerate() {
            let bits = |e0: &[f64; 6], e1: &[f64; 6]| {
                let (sa, em, c) = discrete_gradient(e0, e1, |e| material_point(energy, e)).unwrap();
                let sm = material_point(energy, &em).unwrap().s;
                !c && sa.iter().zip(&sm).all(|(a, b)| a.to_bits() == b.to_bits())
            };
            for size in [0.0, 1e-13] {
                for _ in 0..20 {
                    let e0 = rng.strain(0.1);
                    let e1 = if size == 0.0 {
                        e0
                    } else {
                        with_change(&mut rng, &e0, size)
                    };
                    q2a &= bits(&e0, &e1);
                }
            }
            for _ in 0..100 {
                let e0 = rng.strain(0.1);
                let e1 = with_change(&mut rng, &e0, 1e-8);
                q2b &= bits(&e0, &e1);
            }
            for (base, sizes) in [(0.1, [1e-5, 1e-4, 1e-3]), (1e-3, [1e-6, 1e-5, 1e-4])] {
                for size in sizes {
                    for _ in 0..100 {
                        let e0 = rng.strain(base);
                        let e1 = with_change(&mut rng, &e0, size);
                        let (sa, em, _) = discrete_gradient(&e0, &e1, |e| material_point(energy, e)).unwrap();
                        let sm = material_point(energy, &em).unwrap().s;
                        let diff: Vec<f64> = (0..6).map(|i| sa[i] - sm[i]).collect();
                        let ratio = inf(&diff) / inf(&sm) / (size * size / tensor_norm(&em));
                        worst(&mut q3[k], ratio, "Q3");
                    }
                }
            }
        }
        println!("Q2 midpoint branch bit for bit at dE = 0, 1e-13: {q2a}; at 1e-8 (base 0.1): {q2b}");
        println!("Q3 second order (NH, MR): {:e} {:e} (limit 5)", q3[0], q3[1]);
        assert!(q2a && q2b, "Q2 {q2a} {q2b}");
        assert!(q3[0] <= 5.0 && q3[1] <= 5.0, "Q3 {q3:?}");
    }
}
