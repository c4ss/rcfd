//! Rough-wall rebound (F-254, ADR 0194 W1 to W7).
//!
//! A parcel that hits a face of a rough patch reflects about a virtual wall whose normal is the face normal tilted by a
//! random roughness angle `gamma = angle_std xi`, `xi` standard normal, in the plane of incidence (the virtual-wall model
//! of Sommerfeld 1992 as restated by Tian et al., ANZIAM J. 48, 2007, eq. 5). A draw is kept only when the parcel reaches
//! the virtual wall from the fluid side and its rebound leaves the real wall (W4, registered in ADR 0194).

use crate::drag::reflect;
use crate::rng::SplitMix64;
use crate::{err, Result};
use rcfd_core::vec3::{add, cross, dot, norm, scale, sub, Vec3};
use rcfd_mesh::{Mesh, PatchKind};
use std::f64::consts::{FRAC_PI_2, PI};

/// Rejected draws in one hit after which the smooth reflection applies (W4).
pub const MAX_REDRAWS: u64 = 1000;

/// Largest standard deviation of the roughness angle [degrees] (W7).
pub const MAX_ANGLE_STD_DEG: f64 = 30.0;

/// Where the wall is rough and how rough.
#[derive(Debug, Clone, PartialEq)]
pub struct RoughSpec {
    /// Reflecting wall patches that are rough.
    pub patches: Vec<String>,
    /// Standard deviation of the roughness angle [rad], in `(0, 30]` degrees.
    pub angle_std: f64,
    /// Seed of the roughness random numbers.
    pub seed: u64,
}

/// Counts of the rough-wall hits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RoughCounts {
    /// Hits that drew a roughness angle.
    pub collisions: u64,
    /// Draws rejected by the realizability rule.
    pub redraws: u64,
    /// Hits that fell back to the smooth reflection after `MAX_REDRAWS` rejected draws.
    pub fallbacks: u64,
    /// Hits at normal incidence (the plane of incidence drawn at random).
    pub normal_incidence: u64,
    /// Hits with no velocity into the wall (smooth reflection, nothing drawn).
    pub grazing: u64,
}

/// The rough wall of a tracker: its faces, the roughness and the generator.
#[derive(Debug, Clone)]
pub struct RoughWall {
    names: Vec<String>,
    faces: Vec<usize>,
    patch: Vec<usize>,
    angle_std: f64,
    rng: SplitMix64,
    counts: RoughCounts,
}

impl RoughWall {
    /// The rough wall on the faces of the named wall patches of `mesh`. Refuses `angle_std` outside `(0, 30]` degrees, an
    /// empty or repeated patch list, an unknown patch and a patch that is not a wall.
    pub fn new(mesh: &Mesh, spec: &RoughSpec) -> Result<RoughWall> {
        let s = spec.angle_std;
        if !(s.is_finite() && s > 0.0 && s <= MAX_ANGLE_STD_DEG.to_radians()) {
            return err("rough wall: the standard deviation of the roughness angle must be in (0, 30] degrees");
        }
        if spec.patches.is_empty() {
            return err("rough wall: no patch given");
        }
        let (mut faces, mut patch) = (Vec::new(), Vec::new());
        for (k, name) in spec.patches.iter().enumerate() {
            if spec.patches[..k].contains(name) {
                return err(format!("rough wall: patch '{name}' is listed twice"));
            }
            let Some(p) = mesh.patch(name) else {
                return err(format!("rough wall: unknown boundary patch '{name}'"));
            };
            if p.kind != PatchKind::Wall {
                return err(format!("rough wall: patch '{name}' is not a wall"));
            }
            for f in p.faces() {
                faces.push(f);
                patch.push(k);
            }
        }
        if faces.is_empty() {
            return err("rough wall: the patches have no faces");
        }
        Ok(RoughWall {
            names: spec.patches.clone(),
            faces,
            patch,
            angle_std: s,
            rng: SplitMix64::new(spec.seed),
            counts: RoughCounts::default(),
        })
    }

    /// Global mesh face of each rough face.
    pub fn faces(&self) -> &[usize] {
        &self.faces
    }

    /// Patch name of rough face `k`.
    pub fn patch_name(&self, k: usize) -> &str {
        &self.names[self.patch[k]]
    }

    /// Standard deviation of the roughness angle [rad].
    pub fn angle_std(&self) -> f64 {
        self.angle_std
    }

    /// Counts of the hits so far.
    pub fn counts(&self) -> RoughCounts {
        self.counts
    }

    /// The rebound of the velocity `v` from a face of unit normal `n` (out of the domain) with restitution `e`.
    pub(crate) fn rebound(&mut self, v: Vec3, n: Vec3, e: f64) -> Vec3 {
        rough_rebound(v, n, e, self.angle_std, &mut self.rng, &mut self.counts)
    }
}

/// A fixed unit pair `p`, `q` normal to the unit vector `n`: `p = unit(n x e_k)` with `e_k` the axis of the smallest
/// `abs(n_k)` (the first of equal ones), `q = n x p` (W2).
pub fn tangent_pair(n: Vec3) -> (Vec3, Vec3) {
    let mut k = 0;
    for i in 1..3 {
        if n[i].abs() < n[k].abs() {
            k = i;
        }
    }
    let mut ek = [0.0; 3];
    ek[k] = 1.0;
    let c = cross(n, ek);
    let p = scale(c, 1.0 / norm(c));
    (p, cross(n, p))
}

/// The rough-wall rebound of the velocity `v` from a face of unit normal `n` (out of the domain) with restitution `e` and
/// roughness `angle_std` [rad] (ADR 0194 W1 to W4), drawing from `rng` and counting in `counts`.
pub fn rough_rebound(v: Vec3, n: Vec3, e: f64, angle_std: f64, rng: &mut SplitMix64, counts: &mut RoughCounts) -> Vec3 {
    let u = norm(v);
    let vn = dot(v, n);
    let into_wall = u > 0.0 && vn > 0.0;
    if !into_wall {
        counts.grazing += 1;
        return reflect(v, n, e);
    }
    let vt = sub(v, scale(n, vn));
    let st = norm(vt);
    let t = if st > 1e-12 * u {
        scale(vt, 1.0 / st)
    } else {
        counts.normal_incidence += 1;
        let (p, q) = tangent_pair(n);
        let phi = 2.0 * PI * rng.uniform();
        add(scale(p, phi.cos()), scale(q, phi.sin()))
    };
    counts.collisions += 1;
    let mut rejected = 0;
    loop {
        let g = angle_std * rng.gauss();
        if g.abs() < FRAC_PI_2 {
            let nv = add(scale(n, g.cos()), scale(t, g.sin()));
            let vnv = dot(v, nv);
            if vnv > 0.0 {
                let w = sub(v, scale(nv, (1.0 + e) * vnv));
                if dot(w, n) < 0.0 {
                    return w;
                }
            }
        }
        rejected += 1;
        counts.redraws += 1;
        if rejected >= MAX_REDRAWS {
            counts.fallbacks += 1;
            return reflect(v, n, e);
        }
    }
}
