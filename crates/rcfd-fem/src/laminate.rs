//! Laminated shell: layup stiffness, ply stresses, failure indices. Declared by ADR 0440 (F-408, the scaffold of ADR 0311);
//! filled by ADR 0496 (F-428, the package of ADR 0331).
//!
//! A [`crate::BlockProperty::Layup`] lists its plies bottom first (the `-e_3` side). Each ply has a material
//! (`IsotropicElastic` or `Orthotropic`), a thickness and the angle of its material axis 1 from the element's first axis
//! `e_1` about the normal (ADR 0496 D1). Classical lamination theory gives the membrane, coupling and bending stiffness `A`,
//! `B`, `D`, the transverse shear stiffness `H` (factor 5/6), the thermal resultants of a temperature uniform through the
//! thickness and the mass moments ([`laminate`], ADR 0496 D2). [`section_stiffness`] is the laminate's side of the
//! section-stiffness hook of the shell ([`crate::shell::section_of`]); one isotropic ply takes the homogeneous section, so
//! that it gives the bits of a `Shell` block of that material and thickness (ADR 0496 D3). [`ply_stresses_at`] and
//! [`layup_response`] give the strains and stresses at the bottom and top of each ply in its material axes, and
//! [`failure_index`] the maximum-stress, maximum-strain, Tsai-Hill and Tsai-Wu indices with their reserve factors (D6).
//!
//! ADR 0502 (F-591): a layup also takes the body loads (gravity and the centrifugal load with the mass moments `m0`, `m1`;
//! the nodal body force over the total thickness), the pre-stress and geometric stiffness of its membrane resultants
//! (buckling, pre-stressed modes) and the spin-softening mass of its mass moments (the rotating part); one isotropic ply
//! takes the calls of its `Shell` twin on each. The corotated Newton path still refuses a layup naming [`FOLLOW_UP`].

use crate::error::{FemError, Result};
use crate::mesh::{BlockProperty, ElementKind, Ply};
use crate::model::{Material, SolidModel};
use crate::shell::{homogeneous_section, SectionStiffness, ShearStiffness, SHEAR_FACTOR};

/// The work package that fills this module; `None` once it has landed (ADR 0496).
pub const PENDING: Option<&'static str> = None;

/// The package named when a path that does not take a layup is given one (ADR 0496 D7).
pub const FOLLOW_UP: &str = "a follow-up of ADR 0496";

/// A 3 x 3 matrix, rows.
pub type Matrix3 = [[f64; 3]; 3];

/// The in-plane constants of a ply in its material axes (ADR 0496 D1).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct PlyConstants {
    /// The reduced stiffness `Q` (plane stress, engineering shear), Pa.
    pub q: Matrix3,
    /// `E1`, `E2`, Pa.
    pub e: [f64; 2],
    /// `G12`, `G13`, `G23`, Pa.
    pub g: [f64; 3],
    /// `alpha1`, `alpha2`, 1/K.
    pub alpha: [f64; 2],
    /// Density, kg/m^3.
    pub density: f64,
}

fn ply_error(k: usize, reason: String) -> FemError {
    FemError::InvalidValue {
        key: "ply",
        reason: format!("ply {k}: {reason}"),
    }
}

/// The constants of ply `k` (its index for the refusal), or the refusal of a material that is not a ply material.
pub fn ply_constants(ply: &Ply, k: usize) -> Result<PlyConstants> {
    let (e1, e2, nu12, g, alpha, density) = match &ply.material {
        Material::IsotropicElastic {
            e,
            nu,
            density,
            expansion,
        } => {
            let g = e / (2.0 * (1.0 + nu));
            (*e, *e, *nu, [g, g, g], [*expansion, *expansion], *density)
        }
        Material::Orthotropic {
            e,
            nu,
            g,
            density,
            expansion,
            ..
        } => (e[0], e[1], nu[0], *g, [expansion[0], expansion[1]], *density),
        other => {
            return Err(ply_error(
                k,
                format!(
                    "a {} material is not a ply material (IsotropicElastic or Orthotropic; ADR 0496 D1)",
                    other.name()
                ),
            ))
        }
    };
    let nu21 = nu12 * e2 / e1;
    let d = 1.0 - nu12 * nu21;
    let q = [
        [e1 / d, nu12 * e2 / d, 0.0],
        [nu12 * e2 / d, e2 / d, 0.0],
        [0.0, 0.0, g[0]],
    ];
    Ok(PlyConstants {
        q,
        e: [e1, e2],
        g,
        alpha,
        density,
    })
}

/// The refusals of a list of plies (ADR 0496 D1): an empty list; for each ply in order a thickness that is not positive and
/// finite, an angle that is not finite, a material that is not `IsotropicElastic` or `Orthotropic`. The error is
/// `InvalidValue { key: "ply" }` naming the ply.
pub fn check_plies(plies: &[Ply]) -> Result<()> {
    if plies.is_empty() {
        return Err(FemError::InvalidValue {
            key: "plies",
            reason: "a layup needs at least one ply".to_string(),
        });
    }
    for (k, ply) in plies.iter().enumerate() {
        if !(ply.thickness > 0.0 && ply.thickness.is_finite()) {
            return Err(ply_error(
                k,
                format!("the thickness {} must be positive and finite", ply.thickness),
            ));
        }
        if !ply.angle.is_finite() {
            return Err(ply_error(k, format!("the angle {} must be finite", ply.angle)));
        }
        ply_constants(ply, k)?;
    }
    Ok(())
}

/// `T_s(theta)`: the stress in material axes from element axes, `sigma_m = T_s sigma_e` (ADR 0496 D1).
pub fn stress_rotation(theta: f64) -> Matrix3 {
    let (s, c) = theta.sin_cos();
    [
        [c * c, s * s, 2.0 * c * s],
        [s * s, c * c, -2.0 * c * s],
        [-c * s, c * s, c * c - s * s],
    ]
}

/// `T_e(theta)`: the engineering strain in material axes from element axes, `eps_m = T_e eps_e` (ADR 0496 D1).
pub fn strain_rotation(theta: f64) -> Matrix3 {
    let (s, c) = theta.sin_cos();
    [
        [c * c, s * s, c * s],
        [s * s, c * c, -c * s],
        [-2.0 * c * s, 2.0 * c * s, c * c - s * s],
    ]
}

fn mat3(a: &Matrix3, b: &Matrix3) -> Matrix3 {
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    r
}

fn mul3(a: &Matrix3, v: [f64; 3]) -> [f64; 3] {
    [
        a[0][0] * v[0] + a[0][1] * v[1] + a[0][2] * v[2],
        a[1][0] * v[0] + a[1][1] * v[1] + a[1][2] * v[2],
        a[2][0] * v[0] + a[2][1] * v[1] + a[2][2] * v[2],
    ]
}

/// `Qbar = T_s(-theta) Q T_e(theta)`, the ply stiffness in element axes (ADR 0496 D1).
pub fn rotated_stiffness(q: &Matrix3, theta: f64) -> Matrix3 {
    mat3(&stress_rotation(-theta), &mat3(q, &strain_rotation(theta)))
}

/// `alphabar = T_e(-theta) (alpha1, alpha2, 0)`, the expansion in element axes, engineering shear (ADR 0496 D1).
pub fn rotated_expansion(alpha: [f64; 2], theta: f64) -> [f64; 3] {
    mul3(&strain_rotation(-theta), [alpha[0], alpha[1], 0.0])
}

/// `Hbar = R^T diag(G13, G23) R`, `R = [[c, s], [-s, c]]` (ADR 0496 D1).
pub fn rotated_shear(g13: f64, g23: f64, theta: f64) -> [[f64; 2]; 2] {
    let (s, c) = theta.sin_cos();
    [
        [c * c * g13 + s * s * g23, c * s * (g13 - g23)],
        [c * s * (g13 - g23), s * s * g13 + c * c * g23],
    ]
}

/// The stiffness, thermal resultants and mass moments of a layup by classical lamination theory (ADR 0496 D2).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Laminate {
    /// `A`, N/m.
    pub a: Matrix3,
    /// `B`, N.
    pub b: Matrix3,
    /// `D`, N m.
    pub d: Matrix3,
    /// `H = (5/6) sum Hbar_k t_k`, N/m.
    pub h: [[f64; 2]; 2],
    /// `N_th = sum Qbar_k alphabar_k t_k`, per kelvin, N/(m K).
    pub n_th: [f64; 3],
    /// `M_th = sum Qbar_k alphabar_k t_k zbar_k`, per kelvin, N/K.
    pub m_th: [f64; 3],
    /// The total thickness, m.
    pub thickness: f64,
    /// `(m0, m1, m2) = sum rho_k t_k (1, zbar_k, zbar_k^2 + t_k^2/12)`.
    pub mass: [f64; 3],
    /// `z_0 = -t/2, ..., z_n`, the ply interfaces bottom first.
    pub z: Vec<f64>,
}

/// The laminate of a list of plies by the general sums (ADR 0496 D2), after [`check_plies`].
pub fn laminate(plies: &[Ply]) -> Result<Laminate> {
    check_plies(plies)?;
    let t: f64 = plies.iter().map(|p| p.thickness).sum();
    let mut out = Laminate {
        a: [[0.0; 3]; 3],
        b: [[0.0; 3]; 3],
        d: [[0.0; 3]; 3],
        h: [[0.0; 2]; 2],
        n_th: [0.0; 3],
        m_th: [0.0; 3],
        thickness: t,
        mass: [0.0; 3],
        z: Vec::with_capacity(plies.len() + 1),
    };
    let mut z = -0.5 * t;
    out.z.push(z);
    for (k, ply) in plies.iter().enumerate() {
        let pc = ply_constants(ply, k)?;
        let tk = ply.thickness;
        let zb = z + 0.5 * tk;
        let qb = rotated_stiffness(&pc.q, ply.angle);
        let ab = rotated_expansion(pc.alpha, ply.angle);
        let qa = mul3(&qb, ab);
        let hb = rotated_shear(pc.g[1], pc.g[2], ply.angle);
        let i2 = tk * zb * zb + tk * tk * tk / 12.0;
        for i in 0..3 {
            for j in 0..3 {
                out.a[i][j] += qb[i][j] * tk;
                out.b[i][j] += qb[i][j] * tk * zb;
                out.d[i][j] += qb[i][j] * i2;
            }
            out.n_th[i] += qa[i] * tk;
            out.m_th[i] += qa[i] * tk * zb;
        }
        for i in 0..2 {
            for j in 0..2 {
                out.h[i][j] += SHEAR_FACTOR * hb[i][j] * tk;
            }
        }
        out.mass[0] += pc.density * tk;
        out.mass[1] += pc.density * tk * zb;
        out.mass[2] += pc.density * i2;
        z += tk;
        out.z.push(z);
    }
    Ok(out)
}

/// `Some((material, thickness))` for a layup of one `IsotropicElastic` ply: the route of ADR 0496 D3 to the homogeneous
/// functions of the shell.
pub fn single_isotropic(plies: &[Ply]) -> Option<(&Material, f64)> {
    match plies {
        [ply] if matches!(ply.material, Material::IsotropicElastic { .. }) => Some((&ply.material, ply.thickness)),
        _ => None,
    }
}

/// The section stiffness of a layup (ADR 0496 D2, D3): one isotropic ply gives [`homogeneous_section`] of its material and
/// thickness; any other layup `A`, `Some(B)`, `D`, `Matrix(H)` and the drilling pair `(A66/t, t)`. Refuses as
/// [`check_plies`].
pub fn section_stiffness(plies: &[Ply]) -> Result<SectionStiffness> {
    check_plies(plies)?;
    if let Some((material, t)) = single_isotropic(plies) {
        return Ok(homogeneous_section(material, t));
    }
    let l = laminate(plies)?;
    Ok(SectionStiffness::new(
        l.a,
        Some(l.b),
        l.d,
        ShearStiffness::Matrix(l.h),
        [l.a[2][2] / l.thickness, l.thickness],
    ))
}

/// The strains and stresses at the bottom and top of a ply in its material axes (ADR 0496 D6).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct PlyStress {
    /// The ply, 0-based, bottom first.
    pub ply: usize,
    /// `z` of its bottom and top, m.
    pub z: [f64; 2],
    /// The mechanical strain `(eps1, eps2, gamma12)` at the bottom and the top.
    pub strain: [[f64; 3]; 2],
    /// The stress `(sigma1, sigma2, tau12)` at the bottom and the top, Pa.
    pub stress: [[f64; 3]; 2],
}

/// The ply strains and stresses of a layup at the mid-plane strain `eps0`, curvature `kappa` (element axes, engineering
/// shear) and a temperature change `dt` uniform through the thickness (ADR 0496 D6).
pub fn ply_stresses_at(plies: &[Ply], eps0: [f64; 3], kappa: [f64; 3], dt: f64) -> Result<Vec<PlyStress>> {
    let l = laminate(plies)?;
    let mut out = Vec::with_capacity(plies.len());
    for (k, ply) in plies.iter().enumerate() {
        let pc = ply_constants(ply, k)?;
        let ab = rotated_expansion(pc.alpha, ply.angle);
        let te = strain_rotation(ply.angle);
        let z = [l.z[k], l.z[k + 1]];
        let mut strain = [[0.0; 3]; 2];
        let mut stress = [[0.0; 3]; 2];
        for s in 0..2 {
            let e = [
                eps0[0] + z[s] * kappa[0] - ab[0] * dt,
                eps0[1] + z[s] * kappa[1] - ab[1] * dt,
                eps0[2] + z[s] * kappa[2] - ab[2] * dt,
            ];
            strain[s] = mul3(&te, e);
            stress[s] = mul3(&pc.q, strain[s]);
        }
        out.push(PlyStress {
            ply: k,
            z,
            strain,
            stress,
        });
    }
    Ok(out)
}

/// The response of a Shell4 element of a Layup block at its centre (ADR 0496 D6).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct LayupResponse {
    /// The element.
    pub element: usize,
    /// The mid-plane strain `(eps_xx, eps_yy, gamma_xy)` in the element's local axes.
    pub strain: [f64; 3],
    /// The curvature `(kappa_xx, kappa_yy, kappa_xy)` in the element's local axes, 1/m.
    pub curvature: [f64; 3],
    /// The temperature change at the centre, K (0 without a temperature field).
    pub delta_t: f64,
    /// The plies' strains and stresses.
    pub plies: Vec<PlyStress>,
}

/// The centre strains, curvatures and ply stresses of every Shell4 element of a Layup block of a model for a displacement
/// vector (one value per unknown of `model.dof_map()`), in element order (ADR 0496 D6).
pub fn layup_response(model: &SolidModel, displacement: &[f64]) -> Result<Vec<LayupResponse>> {
    let mesh = model.mesh();
    let mut out = Vec::new();
    for (e, eps, kappa, dt) in crate::shell::centre_strains(model, displacement)? {
        if mesh.element_kind(e) != ElementKind::Shell4 {
            continue;
        }
        let Some(BlockProperty::Layup { plies }) = mesh.blocks()[mesh.block_of(e)].property() else {
            continue;
        };
        let plies_out = ply_stresses_at(plies, eps, kappa, dt).map_err(|source| FemError::Element {
            element: e,
            source: Box::new(source),
        })?;
        out.push(LayupResponse {
            element: e,
            strain: eps,
            curvature: kappa,
            delta_t: dt,
            plies: plies_out,
        });
    }
    Ok(out)
}

/// The strengths of a ply in its material axes, Pa, the compressive ones as magnitudes (ADR 0496 D6).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct PlyStrengths {
    /// Tensile strength along 1.
    pub xt: f64,
    /// Compressive strength along 1.
    pub xc: f64,
    /// Tensile strength along 2.
    pub yt: f64,
    /// Compressive strength along 2.
    pub yc: f64,
    /// In-plane shear strength.
    pub s: f64,
}

impl PlyStrengths {
    /// Strengths; refuses one that is not positive and finite (key `"strength"`).
    pub fn new(xt: f64, xc: f64, yt: f64, yc: f64, s: f64) -> Result<PlyStrengths> {
        for (name, v) in [("xt", xt), ("xc", xc), ("yt", yt), ("yc", yc), ("s", s)] {
            if !(v > 0.0 && v.is_finite()) {
                return Err(FemError::InvalidValue {
                    key: "strength",
                    reason: format!("{name} = {v} must be positive and finite"),
                });
            }
        }
        Ok(PlyStrengths { xt, xc, yt, yc, s })
    }
}

/// A failure criterion (ADR 0496 D6).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Criterion {
    /// The largest ratio of a stress component to its strength.
    MaxStress,
    /// The largest ratio of a strain component to its linear-elastic allowable (strength over modulus).
    MaxStrain,
    /// `(sigma1/X)^2 - sigma1 sigma2/X^2 + (sigma2/Y)^2 + (tau12/S)^2`, `X`, `Y` by the signs of the stresses.
    TsaiHill,
    /// Tsai-Wu with the interaction `F12 = f12_star sqrt(F11 F22)`.
    #[non_exhaustive]
    TsaiWu {
        /// The normalised interaction, in `(-1, 1)`.
        f12_star: f64,
    },
}

impl Criterion {
    /// Tsai-Wu with `f12_star = -1/2`.
    pub fn tsai_wu() -> Criterion {
        Criterion::TsaiWu { f12_star: -0.5 }
    }
    /// Tsai-Wu with a given `f12_star`; refuses one outside `(-1, 1)` (key `"f12_star"`).
    pub fn tsai_wu_with(f12_star: f64) -> Result<Criterion> {
        if !(f12_star > -1.0 && f12_star < 1.0) {
            return Err(FemError::InvalidValue {
                key: "f12_star",
                reason: format!("{f12_star} must lie in (-1, 1) for a closed failure surface"),
            });
        }
        Ok(Criterion::TsaiWu { f12_star })
    }
}

/// The value of a criterion at a ply's stress (failure at 1) and the reserve factor on that stress at failure (ADR 0496 D6).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct FailureIndex {
    /// The criterion's value.
    pub index: f64,
    /// The factor on the stress at which the criterion reaches failure; infinite for a zero stress.
    pub reserve: f64,
}

fn signed(v: f64, t: f64, c: f64) -> f64 {
    if v >= 0.0 {
        v / t
    } else {
        -v / c
    }
}

/// The failure index of a criterion for the stress `(sigma1, sigma2, tau12)` and mechanical strain `(eps1, eps2, gamma12)` of
/// a ply in its material axes (ADR 0496 D6). `constants` gives the moduli of the maximum-strain allowables.
pub fn failure_index(
    criterion: Criterion,
    strengths: &PlyStrengths,
    constants: &PlyConstants,
    stress: [f64; 3],
    strain: [f64; 3],
) -> FailureIndex {
    let st = strengths;
    let linear = |index: f64| FailureIndex {
        index,
        reserve: 1.0 / index,
    };
    match criterion {
        Criterion::MaxStress => linear(
            signed(stress[0], st.xt, st.xc)
                .max(signed(stress[1], st.yt, st.yc))
                .max(stress[2].abs() / st.s),
        ),
        Criterion::MaxStrain => {
            let (e1, e2, g12) = (constants.e[0], constants.e[1], constants.g[0]);
            linear(
                signed(strain[0], st.xt / e1, st.xc / e1)
                    .max(signed(strain[1], st.yt / e2, st.yc / e2))
                    .max(strain[2].abs() / (st.s / g12)),
            )
        }
        Criterion::TsaiHill => {
            let x = if stress[0] >= 0.0 { st.xt } else { st.xc };
            let y = if stress[1] >= 0.0 { st.yt } else { st.yc };
            let index = (stress[0] / x) * (stress[0] / x) - stress[0] * stress[1] / (x * x)
                + (stress[1] / y) * (stress[1] / y)
                + (stress[2] / st.s) * (stress[2] / st.s);
            FailureIndex {
                index,
                reserve: 1.0 / index.sqrt(),
            }
        }
        Criterion::TsaiWu { f12_star } => {
            let f1 = 1.0 / st.xt - 1.0 / st.xc;
            let f2 = 1.0 / st.yt - 1.0 / st.yc;
            let f11 = 1.0 / (st.xt * st.xc);
            let f22 = 1.0 / (st.yt * st.yc);
            let f66 = 1.0 / (st.s * st.s);
            let f12 = f12_star * (f11 * f22).sqrt();
            let (s1, s2, s6) = (stress[0], stress[1], stress[2]);
            let a = f11 * s1 * s1 + f22 * s2 * s2 + f66 * s6 * s6 + 2.0 * f12 * s1 * s2;
            let b = f1 * s1 + f2 * s2;
            let reserve = if a > 0.0 {
                let sq = (b * b + 4.0 * a).sqrt();
                if b >= 0.0 {
                    2.0 / (b + sq)
                } else {
                    (sq - b) / (2.0 * a)
                }
            } else if b > 0.0 {
                1.0 / b
            } else {
                f64::INFINITY
            };
            FailureIndex { index: a + b, reserve }
        }
    }
}
