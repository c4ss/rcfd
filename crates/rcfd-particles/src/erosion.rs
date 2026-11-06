//! Erosion of wall patches by parcel impacts (F-254, ADR 0194 E1 to E5).
//!
//! At every hit of a face of an erosion patch, the mass `m K U^n F(alpha)` eroded by the parcel is booked on the face
//! (DNV RP O501, Revision 4.2-2007, eq. 7.1 to 7.3 and Table 7-1), with `m` the parcel mass, `U` the impact speed and
//! `alpha` the angle between the path and the face. Erosion is an observation: it never changes a parcel.
//!
//! [`EROSION_PRESETS`] names the steel row of DNV Table 7-2 (ADR 0197), for [`ErosionSpec::from_preset`]; see
//! [`PRESET_PROVENANCE`] for what those values are worth.

use crate::{err, Result};
use rcfd_core::vec3::{dot, norm, scale, sub, Vec3};
use rcfd_mesh::{Mesh, PatchKind};
use std::collections::HashMap;
use std::fmt::Write as _;

/// The constants `A_1` to `A_8` of the angle function of steel grades, DNV RP O501 Table 7-1 (Revision 4.2-2007).
pub const DNV_A: [f64; 8] = [9.370, 42.295, 110.864, 175.804, 170.137, 98.398, 31.211, 4.170];

/// Angle function `F(alpha) = sum_{i=1..8} (-1)^(i+1) A_i alpha^i` of DNV RP O501 eq. 7.2, with `alpha` in radians
/// (the source writes `alpha pi/180` with `alpha` in degrees; ADR 0194 E2).
pub fn dnv_angle_function(alpha: f64) -> f64 {
    // Horner form of A1 a - A2 a^2 + A3 a^3 - ... - A8 a^8 = a (A1 - a (A2 - a (A3 - ...)))
    let mut s = 0.0;
    for a in DNV_A.iter().rev() {
        s = a - alpha * s;
    }
    alpha * s
}

/// Impact angle `atan2(max(v . n, 0), abs(v - (v . n) n))` in `[0, pi/2]` of the velocity `v` on a face with the unit
/// normal `n` out of the domain: the angle between the path and the face (ADR 0194 E1).
pub fn impact_angle(v: Vec3, n: Vec3) -> f64 {
    let vn = dot(v, n);
    let vt = norm(sub(v, scale(n, vn)));
    vn.max(0.0).atan2(vt)
}

/// Where erosion is booked and the constants of the correlation.
#[derive(Debug, Clone, PartialEq)]
pub struct ErosionSpec {
    /// Wall patches where hits are booked.
    pub patches: Vec<String>,
    /// `K` of DNV eq. 7.1 [(m/s)^-n].
    pub k: f64,
    /// Velocity exponent `n`.
    pub n: f64,
    /// Target material density [kg/m^3] (for the depth rate).
    pub density: f64,
}

/// Provenance of the erosion presets (ADR 0197 P7), to be shown wherever a preset is used.
pub const PRESET_PROVENANCE: &str = "The Table 7-2 values come from a third-party copy of DNV RP O501 Rev 4.2-2007 that has not been checked against the maintainer's edition; a preset gives an indicative, not certified, erosion rate.";

/// A named set of erosion constants: one row of DNV RP O501 Table 7-2 (ADR 0197 P2). The values come from a third-party
/// copy of DNV RP O501 Rev 4.2-2007 that has not been checked against the maintainer's edition; a preset gives an
/// indicative, not certified, erosion rate ([`PRESET_PROVENANCE`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ErosionPreset {
    /// Name of the preset (`particles.erosion.material` in a case).
    pub name: &'static str,
    /// The row of the table.
    pub row: &'static str,
    /// The table and its edition.
    pub source: &'static str,
    /// `K` of DNV eq. 7.1 [(m/s)^-n].
    pub k: f64,
    /// Velocity exponent `n`.
    pub n: f64,
    /// Density of the wall material [kg/m^3].
    pub density: f64,
}

/// The erosion presets (ADR 0197 P2): the steel grades of DNV RP O501 Rev 4.2-2007 Table 7-2, the row whose angle function
/// is the one of [`dnv_angle_function`]. The values come from a third-party copy of DNV RP O501 Rev 4.2-2007 that has not
/// been checked against the maintainer's edition; a preset gives an indicative, not certified, erosion rate.
pub const EROSION_PRESETS: [ErosionPreset; 1] = [ErosionPreset {
    name: "dnv_o501_2007_steel",
    row: "steel grades",
    source: "DNV RP O501 Rev 4.2-2007 Table 7-2",
    k: 2.0e-9,
    n: 2.6,
    density: 7800.0,
}];

/// The preset of a name, if there is one (ADR 0197 P2).
pub fn erosion_preset(name: &str) -> Option<&'static ErosionPreset> {
    EROSION_PRESETS.iter().find(|p| p.name == name)
}

impl ErosionSpec {
    /// Erosion on `patches` with the three constants of `preset` (ADR 0197). The preset values come from a third-party
    /// copy of DNV RP O501 Rev 4.2-2007 that has not been checked against the maintainer's edition; a preset gives an
    /// indicative, not certified, erosion rate ([`PRESET_PROVENANCE`]).
    pub fn from_preset(patches: Vec<String>, preset: &ErosionPreset) -> ErosionSpec {
        ErosionSpec {
            patches,
            k: preset.k,
            n: preset.n,
            density: preset.density,
        }
    }
}

/// Erosion booked on the faces of the erosion patches.
#[derive(Debug, Clone)]
pub struct Erosion {
    names: Vec<String>,
    faces: Vec<usize>,
    patch: Vec<usize>,
    local: HashMap<usize, usize>,
    area: Vec<f64>,
    centre: Vec<Vec3>,
    k: f64,
    n: f64,
    density: f64,
    eroded: Vec<f64>,
    impact: Vec<f64>,
    hits: Vec<u64>,
}

fn positive(x: f64) -> bool {
    x.is_finite() && x > 0.0
}

impl Erosion {
    /// Erosion booking on the faces of the named wall patches of `mesh`. Refuses `k`, `n` or `density` not above 0, an
    /// empty or repeated patch list, an unknown patch and a patch that is not a wall.
    pub fn new(mesh: &Mesh, spec: &ErosionSpec) -> Result<Erosion> {
        if !positive(spec.k) {
            return err("erosion: K must be positive");
        }
        if !positive(spec.n) {
            return err("erosion: the velocity exponent n must be positive");
        }
        if !positive(spec.density) {
            return err("erosion: the target density must be positive");
        }
        if spec.patches.is_empty() {
            return err("erosion: no patch given");
        }
        let (mut faces, mut patch) = (Vec::new(), Vec::new());
        for (k, name) in spec.patches.iter().enumerate() {
            if spec.patches[..k].contains(name) {
                return err(format!("erosion: patch '{name}' is listed twice"));
            }
            let Some(p) = mesh.patch(name) else {
                return err(format!("erosion: unknown boundary patch '{name}'"));
            };
            if p.kind != PatchKind::Wall {
                return err(format!("erosion: patch '{name}' is not a wall"));
            }
            for f in p.faces() {
                faces.push(f);
                patch.push(k);
            }
        }
        if faces.is_empty() {
            return err("erosion: the patches have no faces");
        }
        let n = faces.len();
        let local = faces.iter().enumerate().map(|(k, &f)| (f, k)).collect();
        let area = faces.iter().map(|&f| mesh.face_area(f)).collect();
        let centre = faces.iter().map(|&f| mesh.face_centres()[f]).collect();
        Ok(Erosion {
            names: spec.patches.clone(),
            faces,
            patch,
            local,
            area,
            centre,
            k: spec.k,
            n: spec.n,
            density: spec.density,
            eroded: vec![0.0; n],
            impact: vec![0.0; n],
            hits: vec![0; n],
        })
    }

    /// Number of erosion faces.
    pub fn len(&self) -> usize {
        self.faces.len()
    }

    /// True when there is no erosion face (never, once built).
    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// Global mesh face of each erosion face.
    pub fn faces(&self) -> &[usize] {
        &self.faces
    }

    /// Erosion face of a global mesh face, if it is one.
    pub fn local(&self, face: usize) -> Option<usize> {
        self.local.get(&face).copied()
    }

    /// Patch name of erosion face `k`.
    pub fn patch_name(&self, k: usize) -> &str {
        &self.names[self.patch[k]]
    }

    /// Area of erosion face `k` [m^2].
    pub fn area(&self, k: usize) -> f64 {
        self.area[k]
    }

    /// Centre of erosion face `k`.
    pub fn centre(&self, k: usize) -> Vec3 {
        self.centre[k]
    }

    /// `K` [(m/s)^-n].
    pub fn k(&self) -> f64 {
        self.k
    }

    /// Velocity exponent.
    pub fn n(&self) -> f64 {
        self.n
    }

    /// Target material density [kg/m^3].
    pub fn density(&self) -> f64 {
        self.density
    }

    /// Eroded mass of each face [kg].
    pub fn eroded(&self) -> &[f64] {
        &self.eroded
    }

    /// Mass of the parcels that hit each face [kg].
    pub fn impact_mass(&self) -> &[f64] {
        &self.impact
    }

    /// Number of hits of each face.
    pub fn hits(&self) -> &[u64] {
        &self.hits
    }

    /// Eroded mass of all faces [kg].
    pub fn total_eroded(&self) -> f64 {
        self.eroded.iter().sum()
    }

    /// Mass of the parcels that hit any face [kg].
    pub fn total_impact_mass(&self) -> f64 {
        self.impact.iter().sum()
    }

    /// Hits of all faces.
    pub fn total_hits(&self) -> u64 {
        self.hits.iter().sum()
    }

    /// Erosion rate `E_f / (A_f t)` of face `k` over the time `t` [kg/(m^2 s)]; 0 when `t` is not positive (E4).
    pub fn rate(&self, k: usize, t: f64) -> f64 {
        if t > 0.0 {
            self.eroded[k] / (self.area[k] * t)
        } else {
            0.0
        }
    }

    /// Impact mass flux `M_f / (A_f t)` of face `k` over the time `t` [kg/(m^2 s)]; 0 when `t` is not positive.
    pub fn impact_flux(&self, k: usize, t: f64) -> f64 {
        if t > 0.0 {
            self.impact[k] / (self.area[k] * t)
        } else {
            0.0
        }
    }

    /// Depth rate `rate / density` of face `k` over the time `t` [m/s].
    pub fn depth_rate(&self, k: usize, t: f64) -> f64 {
        self.rate(k, t) / self.density
    }

    /// Largest face erosion rate over the time `t`.
    pub fn max_rate(&self, t: f64) -> f64 {
        (0..self.len()).map(|k| self.rate(k, t)).fold(0.0, f64::max)
    }

    /// Largest face depth rate over the time `t`.
    pub fn max_depth_rate(&self, t: f64) -> f64 {
        (0..self.len()).map(|k| self.depth_rate(k, t)).fold(0.0, f64::max)
    }

    /// Book the hit of a parcel of mass `m` with the velocity `v` on the global mesh face `face` of unit normal `n`; false
    /// when it is not an erosion face (E1 to E3).
    pub(crate) fn book(&mut self, face: usize, m: f64, v: Vec3, n: Vec3) -> bool {
        let Some(k) = self.local(face) else { return false };
        let u = norm(v);
        let alpha = impact_angle(v, n);
        self.eroded[k] += m * self.k * u.powf(self.n) * dnv_angle_function(alpha);
        self.impact[k] += m;
        self.hits[k] += 1;
        true
    }

    /// One row per erosion face over the time `t`: `face,patch,x,y,z,area,hits,impact_mass,eroded,rate,depth_rate`
    /// (shortest round-trip numbers).
    pub fn csv(&self, t: f64) -> String {
        let mut s = String::from("face,patch,x,y,z,area,hits,impact_mass,eroded,rate,depth_rate\n");
        for k in 0..self.len() {
            let c = self.centre[k];
            let _ = writeln!(
                s,
                "{},{},{},{},{},{},{},{},{},{},{}",
                self.faces[k],
                self.patch_name(k),
                c[0],
                c[1],
                c[2],
                self.area[k],
                self.hits[k],
                self.impact[k],
                self.eroded[k],
                self.rate(k, t),
                self.depth_rate(k, t)
            );
        }
        s
    }
}
