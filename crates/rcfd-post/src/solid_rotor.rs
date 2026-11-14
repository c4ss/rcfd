//! Conversion to and from rotordyn; discs from a solid model. Declared by ADR 0440 (F-408, the scaffold of ADR 0311); filled by
//! ADR 0476 (F-416, design stub ADR 0319).
//!
//! The flexible rotor of [`rcfd_fem::rotor`] and the rigid rotor of [`crate::rotordyn`] (ADR 0120) describe the same machine at
//! two levels (ADR 0476 D10): [`flexible_from_rigid`] puts the rigid rotor's mass and inertias on a disc at the centre of mass of
//! a shaft between its two bearings (the stiff-shaft limit when the shaft is stiff), [`rigid_from_flexible`] lumps a flexible
//! rotor on two isotropic bearings into the rigid one, [`nodal_load`] turns a `rotordyn` harmonic load into station phasors, and
//! [`disc_from_solid`] takes a rigid disc's mass and inertias from the consistent mass of a solid part. `rotordyn.rs` is not
//! edited.

use crate::rotordyn::{self, HarmonicLoad};
use rcfd_core::vec3::Vec3;
use rcfd_fem::mass::{consistent, mass_properties};
use rcfd_fem::rotor::{Bearing, Disc, RotorModel, C64};
use rcfd_fem::{BeamSection, FemError, Material, Result, SolidModel};

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// The relative tolerance of the axisymmetry and centring tests of [`disc_from_solid`] (ADR 0476 D10).
pub const AXISYMMETRY_TOLERANCE: f64 = 1e-9;

fn invalid(key: &'static str, reason: String) -> FemError {
    FemError::InvalidValue { key, reason }
}

/// The shaft used by [`flexible_from_rigid`]: an axisymmetric section and an isotropic elastic material.
#[derive(Debug, Clone, PartialEq)]
pub struct Shaft {
    /// The section.
    pub section: BeamSection,
    /// The material (density 0 for a massless shaft).
    pub material: Material,
}

/// A rigid disc taken from a solid part (ADR 0476 D10).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolidDisc {
    /// Mass, transverse inertia `(I_xx + I_yy)/2` and polar inertia `I_zz` about the centre.
    pub disc: Disc,
    /// The centre of mass.
    pub centre: Vec3,
    /// `|I_xx - I_yy| / (I_xx + I_yy)`.
    pub asymmetry: f64,
}

fn c64(v: rotordyn::C64) -> C64 {
    C64::new(v.re, v.im)
}

/// A flexible rotor for a rigid one (ADR 0476 D10): stations at the two bearings and at the centre of mass (`z = 0`, merged
/// with a bearing at the same position), the disc `(mass, it, ip)` at the centre, the isotropic bearings `(k, c)`, shaft
/// elements of `shaft` between consecutive stations. Refuses a rotor that `rotordyn` refuses (`"rotor"`), a bearing with a
/// hysteretic loss factor (`"eta"`) and the shaft's refusals.
pub fn flexible_from_rigid(rotor: &rotordyn::Rotor, shaft: &Shaft) -> Result<RotorModel> {
    rotor.validate().map_err(|e| invalid("rotor", e))?;
    for (j, b) in rotor.bearings.iter().enumerate() {
        if b.eta != 0.0 {
            return Err(invalid(
                "eta",
                format!(
                    "bearing {j} has the hysteretic loss factor {}: the flexible rotor has no hysteretic term",
                    b.eta
                ),
            ));
        }
    }
    let mut z = vec![rotor.bearings[0].z, rotor.bearings[1].z, 0.0];
    z.sort_by(|a, b| a.total_cmp(b));
    z.dedup();
    let at = |v: f64| z.iter().position(|w| *w == v).expect("a station");
    let mut model = RotorModel::new(z.clone())?;
    for i in 0..z.len() - 1 {
        model.add_shaft(i, i + 1, shaft.section, &shaft.material)?;
    }
    model.add_disc(
        at(0.0),
        Disc {
            mass: rotor.mass,
            transverse: rotor.it,
            polar: rotor.ip,
        },
    )?;
    for b in &rotor.bearings {
        model.add_bearing(at(b.z), Bearing::isotropic(b.k, b.c))?;
    }
    Ok(model)
}

fn isotropic(m: &[[f64; 2]; 2]) -> Option<f64> {
    (m[0][0] == m[1][1] && m[0][1] == 0.0 && m[1][0] == 0.0).then_some(m[0][0])
}

/// The rigid rotor of a flexible one (ADR 0476 D10): total mass, `I_t` about the centre of mass and `I_p` of the discs and the
/// shaft elements (each element a rigid uniform rod with rotary inertia), the two bearings with positions from the centre.
/// Refuses a model without exactly two bearings, or with a bearing that is not isotropic (`"bearing"`), and a rotor `rotordyn`
/// refuses (`"rotor"`).
pub fn rigid_from_flexible(model: &RotorModel) -> Result<rotordyn::Rotor> {
    let z = model.stations();
    let mut mass = 0.0;
    let mut first = 0.0;
    for (n, d) in model.discs() {
        mass += d.mass;
        first += d.mass * z[*n];
    }
    let mut rods = Vec::new();
    for s in model.shafts() {
        let [a, b] = s.nodes;
        let l = z[b] - z[a];
        let rho = s.material.density();
        let ma = rho * s.section.area() * l;
        let mid = 0.5 * (z[a] + z[b]);
        mass += ma;
        first += ma * mid;
        rods.push((
            ma,
            l,
            mid,
            rho * s.section.i_y() * l,
            rho * (s.section.i_y() + s.section.i_z()) * l,
        ));
    }
    if !(mass > 0.0) {
        return Err(invalid("rotor", "the rotor has no mass".to_string()));
    }
    let zc = first / mass;
    let mut it = 0.0;
    let mut ip = 0.0;
    for (n, d) in model.discs() {
        let dz = z[*n] - zc;
        it += d.transverse + d.mass * dz * dz;
        ip += d.polar;
    }
    for (ma, l, mid, rot, pol) in rods {
        let dz = mid - zc;
        it += ma * (l * l / 12.0 + dz * dz) + rot;
        ip += pol;
    }
    let bs = model.bearings();
    if bs.len() != 2 {
        return Err(invalid(
            "bearing",
            format!("the rigid rotor takes exactly two bearings, the model has {}", bs.len()),
        ));
    }
    let mut out = [rotordyn::Bearing {
        z: 0.0,
        k: 0.0,
        c: 0.0,
        eta: 0.0,
    }; 2];
    for (j, (n, b)) in bs.iter().enumerate() {
        let (Some(k), Some(c)) = (isotropic(&b.k), isotropic(&b.c)) else {
            return Err(invalid(
                "bearing",
                format!("bearing {j} at station {n} is not isotropic: {b:?}"),
            ));
        };
        out[j] = rotordyn::Bearing {
            z: z[*n] - zc,
            k,
            c,
            eta: 0.0,
        };
    }
    let r = rotordyn::Rotor {
        mass,
        it,
        ip,
        bearings: out,
    };
    r.validate().map_err(|e| invalid("rotor", e))?;
    Ok(r)
}

/// The station phasors `[F_x, F_y, M_x, M_y]` of a `rotordyn` harmonic load (the axial force and the torque are not part of the
/// lateral model).
pub fn nodal_load(load: &HarmonicLoad) -> [C64; 4] {
    [
        c64(load.force[0]),
        c64(load.force[1]),
        c64(load.moment[0]),
        c64(load.moment[1]),
    ]
}

/// A rigid disc from the consistent mass of a solid part (ADR 0476 D10; the mass properties of ADR 0284). Refuses a centre of
/// mass off the `z` axis by more than [`AXISYMMETRY_TOLERANCE`] of the part's size (`"centre"`) and inertias that are not
/// axisymmetric within that tolerance of `I_xx + I_yy` (`"inertia"`).
pub fn disc_from_solid(model: &SolidModel) -> Result<SolidDisc> {
    let m = consistent(model)?;
    let p = mass_properties(model, &m)?;
    let nodes = model.mesh().nodes();
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for x in nodes {
        for c in 0..3 {
            lo[c] = lo[c].min(x[c]);
            hi[c] = hi[c].max(x[c]);
        }
    }
    let size = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt();
    let off = p.centre[0].hypot(p.centre[1]);
    if off > AXISYMMETRY_TOLERANCE * size {
        return Err(invalid(
            "centre",
            format!(
                "the centre of mass {:?} is {off:e} m off the z axis (part size {size:e} m)",
                p.centre
            ),
        ));
    }
    let i = p.inertia;
    let sum = i[0][0] + i[1][1];
    let asymmetry = (i[0][0] - i[1][1]).abs() / sum;
    let products = i[0][1].abs().max(i[0][2].abs()).max(i[1][2].abs()) / sum;
    if !(asymmetry <= AXISYMMETRY_TOLERANCE && products <= AXISYMMETRY_TOLERANCE) {
        return Err(invalid(
            "inertia",
            format!(
                "the inertia is not axisymmetric about z: I_xx {:e}, I_yy {:e}, products {:?} (relative {asymmetry:e}, \
                 {products:e})",
                i[0][0],
                i[1][1],
                [i[0][1], i[0][2], i[1][2]]
            ),
        ));
    }
    Ok(SolidDisc {
        disc: Disc {
            mass: p.mass,
            transverse: 0.5 * sum,
            polar: i[2][2],
        },
        centre: p.centre,
        asymmetry,
    })
}
