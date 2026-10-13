//! Boundary-condition framework.
//!
//! Every condition is reduced to implicit coefficients per boundary face:
//! `phi_f = a * phi_P + b` and outward normal gradient
//! `dphi/dn = c * phi_P + d`. Operators use `a, b` for convection and
//! `c, d` for diffusion, so a new condition only has to implement [`Bc`].

use rcfd_core::vec3::Vec3;

/// Implicit face coefficients (see module docs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceCoeffs {
    /// Face value = `a * phi_P + b`.
    pub a: f64,
    /// Explicit part of the face value.
    pub b: f64,
    /// Normal gradient = `c * phi_P + d`.
    pub c: f64,
    /// Explicit part of the normal gradient.
    pub d: f64,
}

impl FaceCoeffs {
    /// Coefficients from a face-value law, with the gradient taken as
    /// `(phi_f - phi_P) / dn`.
    pub fn from_value(a: f64, b: f64, dn: f64) -> Self {
        FaceCoeffs {
            a,
            b,
            c: (a - 1.0) / dn,
            d: b / dn,
        }
    }

    /// Face value for a given cell value.
    pub fn value(&self, phi_p: f64) -> f64 {
        self.a * phi_p + self.b
    }

    /// Normal gradient for a given cell value.
    pub fn gradient(&self, phi_p: f64) -> f64 {
        self.c * phi_p + self.d
    }
}

/// Information a condition may use when producing coefficients.
#[derive(Debug, Clone, Copy)]
pub struct BcCtx {
    /// Outward unit normal.
    pub normal: Vec3,
    /// Normal distance from the owner cell centre to the face.
    pub dn: f64,
    /// Index of the face inside its patch.
    pub local_face: usize,
    /// Current (lagged) owner cell value; scalars use component 0.
    pub cell_value: Vec3,
    /// For coupled patches: partner cell value and its normal distance.
    pub partner: Option<(Vec3, f64)>,
    /// True when the field is a vector (affects symmetry handling).
    pub is_vector: bool,
}

/// A boundary condition on one patch.
pub trait Bc: Send + Sync + std::fmt::Debug {
    /// Short type name.
    fn type_name(&self) -> &'static str;
    /// Coefficients for component `comp` (0 for scalars).
    fn coeffs(&self, ctx: &BcCtx, comp: usize) -> FaceCoeffs;
    /// Faces of this condition are excluded from all operators (2D reduction).
    fn is_empty(&self) -> bool {
        false
    }
    /// The condition couples to the periodic partner patch.
    fn is_coupled(&self) -> bool {
        false
    }
    /// Clone into a box.
    fn clone_box(&self) -> Box<dyn Bc>;
}

impl Clone for Box<dyn Bc> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Uniform or per-face prescribed values.
#[derive(Debug, Clone, PartialEq)]
pub enum Profile {
    /// One value on every face.
    Uniform(Vec3),
    /// One value per face of the patch.
    PerFace(Vec<Vec3>),
}

impl Profile {
    /// Scalar uniform profile.
    pub fn scalar(v: f64) -> Self {
        Profile::Uniform([v, 0.0, 0.0])
    }

    fn get(&self, i: usize) -> Vec3 {
        match self {
            Profile::Uniform(v) => *v,
            Profile::PerFace(v) => v[i],
        }
    }
}

/// Dirichlet condition.
#[derive(Debug, Clone)]
pub struct FixedValue(pub Profile);

impl FixedValue {
    /// Uniform scalar value.
    pub fn scalar(v: f64) -> Self {
        FixedValue(Profile::scalar(v))
    }
    /// Uniform vector value.
    pub fn vector(v: Vec3) -> Self {
        FixedValue(Profile::Uniform(v))
    }
}

impl Bc for FixedValue {
    fn type_name(&self) -> &'static str {
        "fixedValue"
    }
    fn coeffs(&self, ctx: &BcCtx, comp: usize) -> FaceCoeffs {
        FaceCoeffs::from_value(0.0, self.0.get(ctx.local_face)[comp], ctx.dn)
    }
    fn clone_box(&self) -> Box<dyn Bc> {
        Box::new(self.clone())
    }
}

/// Homogeneous Neumann condition.
#[derive(Debug, Clone, Copy)]
pub struct ZeroGradient;

impl Bc for ZeroGradient {
    fn type_name(&self) -> &'static str {
        "zeroGradient"
    }
    fn coeffs(&self, _: &BcCtx, _: usize) -> FaceCoeffs {
        FaceCoeffs {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 0.0,
        }
    }
    fn clone_box(&self) -> Box<dyn Bc> {
        Box::new(*self)
    }
}

/// Prescribed outward normal gradient.
#[derive(Debug, Clone)]
pub struct FixedGradient(pub Profile);

impl Bc for FixedGradient {
    fn type_name(&self) -> &'static str {
        "fixedGradient"
    }
    fn coeffs(&self, ctx: &BcCtx, comp: usize) -> FaceCoeffs {
        let g = self.0.get(ctx.local_face)[comp];
        FaceCoeffs {
            a: 1.0,
            b: g * ctx.dn,
            c: 0.0,
            d: g,
        }
    }
    fn clone_box(&self) -> Box<dyn Bc> {
        Box::new(self.clone())
    }
}

/// Blend of Dirichlet and Neumann: `phi_f = f * ref + (1-f) (phi_P + grad * dn)`.
/// `fraction = 1` is fixed value, `0` is fixed gradient.
#[derive(Debug, Clone, Copy)]
pub struct Mixed {
    /// Reference value.
    pub value: f64,
    /// Reference normal gradient.
    pub gradient: f64,
    /// Value fraction in `[0, 1]`.
    pub fraction: f64,
}

impl Bc for Mixed {
    fn type_name(&self) -> &'static str {
        "mixed"
    }
    fn coeffs(&self, ctx: &BcCtx, _: usize) -> FaceCoeffs {
        let f = self.fraction;
        FaceCoeffs::from_value(1.0 - f, f * self.value + (1.0 - f) * self.gradient * ctx.dn, ctx.dn)
    }
    fn clone_box(&self) -> Box<dyn Bc> {
        Box::new(*self)
    }
}

/// Symmetry plane: zero normal gradient for scalars; for vectors the normal
/// component vanishes and tangential components have zero gradient. The
/// cross-component coupling is treated explicitly with the lagged cell value.
#[derive(Debug, Clone, Copy)]
pub struct Symmetry;

impl Bc for Symmetry {
    fn type_name(&self) -> &'static str {
        "symmetry"
    }
    fn coeffs(&self, ctx: &BcCtx, comp: usize) -> FaceCoeffs {
        if !ctx.is_vector {
            return ZeroGradient.coeffs(ctx, comp);
        }
        let n = ctx.normal;
        let others: f64 = (0..3).filter(|&j| j != comp).map(|j| n[j] * ctx.cell_value[j]).sum();
        FaceCoeffs::from_value(1.0 - n[comp] * n[comp], -n[comp] * others, ctx.dn)
    }
    fn clone_box(&self) -> Box<dyn Bc> {
        Box::new(*self)
    }
}

/// Patch removed from the discretisation (2D reduction).
#[derive(Debug, Clone, Copy)]
pub struct Empty;

impl Bc for Empty {
    fn type_name(&self) -> &'static str {
        "empty"
    }
    fn coeffs(&self, ctx: &BcCtx, comp: usize) -> FaceCoeffs {
        ZeroGradient.coeffs(ctx, comp)
    }
    fn is_empty(&self) -> bool {
        true
    }
    fn clone_box(&self) -> Box<dyn Bc> {
        Box::new(*self)
    }
}

/// Far-field condition that switches on the flow direction relative to a
/// reference velocity `u_ref`: where `n . u_ref < 0` (inflow) the face is
/// fixed to `value` when `fixed_on_inflow` and zero-gradient otherwise;
/// on outflow faces the roles swap. Use `fixed_on_inflow = true` for the
/// velocity and `false` for the pressure of a free-stream boundary.
#[derive(Debug, Clone, Copy)]
pub struct Freestream {
    /// Reference (free-stream) velocity defining inflow and outflow.
    pub u_ref: Vec3,
    /// Prescribed value (vector for velocity, first component for scalars).
    pub value: Vec3,
    /// Fix the value on inflow faces (true) or on outflow faces (false).
    pub fixed_on_inflow: bool,
}

impl Bc for Freestream {
    fn type_name(&self) -> &'static str {
        "freestream"
    }
    fn coeffs(&self, ctx: &BcCtx, comp: usize) -> FaceCoeffs {
        let inflow =
            ctx.normal[0] * self.u_ref[0] + ctx.normal[1] * self.u_ref[1] + ctx.normal[2] * self.u_ref[2] < 0.0;
        if inflow == self.fixed_on_inflow {
            FaceCoeffs::from_value(0.0, self.value[comp], ctx.dn)
        } else {
            FaceCoeffs::from_value(1.0, 0.0, ctx.dn)
        }
    }
    fn clone_box(&self) -> Box<dyn Bc> {
        Box::new(*self)
    }
}

/// Translational periodicity. The face value is the distance-weighted mean
/// of the owner and partner cell values; the partner value is lagged
/// (taken from the current field), so steady problems converge through the
/// outer iterations.
#[derive(Debug, Clone, Copy)]
pub struct Periodic;

impl Bc for Periodic {
    fn type_name(&self) -> &'static str {
        "periodic"
    }
    fn coeffs(&self, ctx: &BcCtx, comp: usize) -> FaceCoeffs {
        let (q, dnq) = ctx.partner.expect("periodic condition needs a partner value");
        let span = ctx.dn + dnq;
        let w = dnq / span;
        FaceCoeffs {
            a: w,
            b: (1.0 - w) * q[comp],
            c: -1.0 / span,
            d: q[comp] / span,
        }
    }
    fn is_coupled(&self) -> bool {
        true
    }
    fn clone_box(&self) -> Box<dyn Bc> {
        Box::new(*self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> BcCtx {
        BcCtx {
            normal: [1.0, 0.0, 0.0],
            dn: 0.1,
            local_face: 1,
            cell_value: [2.0, 3.0, 0.0],
            partner: None,
            is_vector: false,
        }
    }

    #[test]
    fn fixed_value() {
        let c = FixedValue(Profile::PerFace(vec![[1.0, 0.0, 0.0], [5.0, 0.0, 0.0]])).coeffs(&ctx(), 0);
        assert_eq!(c.value(2.0), 5.0);
        assert!((c.gradient(2.0) - (5.0 - 2.0) / 0.1).abs() < 1e-12);
    }

    #[test]
    fn zero_and_fixed_gradient() {
        let z = ZeroGradient.coeffs(&ctx(), 0);
        assert_eq!((z.value(2.0), z.gradient(2.0)), (2.0, 0.0));
        let g = FixedGradient(Profile::scalar(4.0)).coeffs(&ctx(), 0);
        assert!((g.value(2.0) - 2.4).abs() < 1e-12);
        assert_eq!(g.gradient(2.0), 4.0);
    }

    #[test]
    fn mixed_limits_and_consistency() {
        let m = |f| {
            Mixed {
                value: 7.0,
                gradient: 3.0,
                fraction: f,
            }
            .coeffs(&ctx(), 0)
        };
        assert!((m(1.0).value(2.0) - 7.0).abs() < 1e-12);
        assert!((m(0.0).value(2.0) - (2.0 + 3.0 * 0.1)).abs() < 1e-12);
        let h = m(0.5);
        // gradient is consistent with the face value
        assert!((h.gradient(2.0) - (h.value(2.0) - 2.0) / 0.1).abs() < 1e-12);
    }

    #[test]
    fn symmetry_scalar_and_vector() {
        let s = Symmetry.coeffs(&ctx(), 0);
        assert_eq!(s.value(2.0), 2.0);
        // vector: u = (2,3,0), n = x -> face velocity (0,3,0)
        let v = BcCtx {
            is_vector: true,
            ..ctx()
        };
        let fx = Symmetry.coeffs(&v, 0);
        let fy = Symmetry.coeffs(&v, 1);
        assert!((fx.value(2.0)).abs() < 1e-12);
        assert!((fy.value(3.0) - 3.0).abs() < 1e-12);
        // oblique normal: face value must be tangential
        let n = [0.6, 0.8, 0.0];
        let o = BcCtx {
            normal: n,
            is_vector: true,
            ..ctx()
        };
        let u = o.cell_value;
        let uf: Vec<f64> = (0..3).map(|k| Symmetry.coeffs(&o, k).value(u[k])).collect();
        assert!((uf[0] * n[0] + uf[1] * n[1]).abs() < 1e-12);
    }

    #[test]
    fn periodic_and_empty() {
        let c = BcCtx {
            partner: Some(([6.0, 0.0, 0.0], 0.3)),
            ..ctx()
        };
        let p = Periodic.coeffs(&c, 0);
        // weights 0.3/0.4 for owner, 0.1/0.4 for partner
        assert!((p.value(2.0) - (0.75 * 2.0 + 0.25 * 6.0)).abs() < 1e-12);
        assert!((p.gradient(2.0) - (6.0 - 2.0) / 0.4).abs() < 1e-12);
        assert!(Periodic.is_coupled() && Empty.is_empty() && !ZeroGradient.is_empty());
    }

    #[test]
    fn custom_condition_via_trait() {
        #[derive(Debug, Clone)]
        struct Robin;
        impl Bc for Robin {
            fn type_name(&self) -> &'static str {
                "robin"
            }
            fn coeffs(&self, ctx: &BcCtx, _: usize) -> FaceCoeffs {
                FaceCoeffs::from_value(0.25, 1.0, ctx.dn)
            }
            fn clone_box(&self) -> Box<dyn Bc> {
                Box::new(self.clone())
            }
        }
        let b: Box<dyn Bc> = Box::new(Robin);
        let b2 = b.clone();
        assert_eq!(b2.coeffs(&ctx(), 0).value(4.0), 2.0);
    }
}
