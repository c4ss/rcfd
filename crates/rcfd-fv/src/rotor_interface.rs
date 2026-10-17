//! Frames and frozen-rotor interfaces of turbomachinery (F-126, ADR 0055).
//!
//! A [`RotorFrame`] is a reference frame rotating with angular velocity `omega` (rad/s, right-handed) about the line through
//! `origin` along the unit `axis`. Its frame velocity is `u_f(x) = omega axis x (x - origin)`. A velocity relative to the
//! frame is `w = u - u_f(x)` (`u` absolute); a velocity relative to frame `a` becomes one relative to frame `b` through
//! `w_b = w_a + u_fa(x) - u_fb(x)`. These are exact vector sums, the error is round-off.
//!
//! A frozen-rotor interface is the set of internal faces between a rotating and another cell zone (the planar non-conformal
//! join of `rcfd-mesh::non_conformal` makes ordinary internal faces). [`FrozenRotorInterface`] stores them with the frame-flux
//! jump `(u_fa - u_fb) . S`, which is zero when the interface is a surface of revolution about the axis, and evaluates flux
//! closure in the absolute and in the relative form. The steady solver (`sources.mrf`, ADR 0008) works in the absolute
//! velocity, where the frame change at the interface is the identity for the solved variable; this module is the library for
//! reporting in relative frames, for checking the interface, and for the mixing plane ([`crate::mixing_plane`]).

use crate::geometry::FvGeometry;
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};

fn bad(path: &str, reason: impl Into<String>) -> Error {
    Error::InvalidValue {
        path: path.to_string(),
        reason: reason.into(),
    }
}

/// Reference frame rotating about an axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotorFrame {
    /// A point on the rotation axis.
    pub origin: Vec3,
    /// Unit axis direction.
    pub axis: Vec3,
    /// Angular velocity (rad/s, right-handed about `axis`).
    pub omega: f64,
}

impl RotorFrame {
    /// New frame; the axis is normalised. A zero or non-finite axis, origin or angular velocity is a located error.
    pub fn new(origin: Vec3, axis: Vec3, omega: f64) -> Result<RotorFrame> {
        if !omega.is_finite() {
            return Err(bad(
                "rotor_frame.omega",
                format!("angular velocity {omega} is not finite"),
            ));
        }
        if origin.iter().any(|v| !v.is_finite()) {
            return Err(bad("rotor_frame.origin", "origin has a non-finite component"));
        }
        let n = vec3::norm(axis);
        if !n.is_finite() || n < 1e-300 {
            return Err(bad(
                "rotor_frame.axis",
                "rotation axis must be a finite non-zero vector",
            ));
        }
        Ok(RotorFrame {
            origin,
            axis: vec3::scale(axis, 1.0 / n),
            omega,
        })
    }

    /// Frame velocity `omega axis x (x - origin)`.
    pub fn frame_velocity(&self, x: Vec3) -> Vec3 {
        vec3::cross(vec3::scale(self.axis, self.omega), vec3::sub(x, self.origin))
    }

    /// Velocity relative to the frame from the absolute velocity `u` at `x`.
    pub fn to_relative(&self, u: Vec3, x: Vec3) -> Vec3 {
        vec3::sub(u, self.frame_velocity(x))
    }

    /// Absolute velocity from the velocity `w` relative to the frame at `x`.
    pub fn to_absolute(&self, w: Vec3, x: Vec3) -> Vec3 {
        vec3::add(w, self.frame_velocity(x))
    }

    /// Velocity `w` relative to this frame, expressed relative to the frame `to`.
    pub fn change_frame(&self, to: &RotorFrame, w: Vec3, x: Vec3) -> Vec3 {
        vec3::add(w, vec3::sub(self.frame_velocity(x), to.frame_velocity(x)))
    }

    /// Frame-flux jump `(u_f(this) - u_f(to)) . S` of a face with centre `x` and area vector `s`.
    pub fn frame_flux_jump(&self, to: &RotorFrame, x: Vec3, s: Vec3) -> f64 {
        vec3::dot(vec3::sub(self.frame_velocity(x), to.frame_velocity(x)), s)
    }

    /// Distance of `x` from the axis.
    pub fn radius(&self, x: Vec3) -> f64 {
        let d = vec3::sub(x, self.origin);
        vec3::norm(vec3::sub(d, vec3::scale(self.axis, vec3::dot(d, self.axis))))
    }
}

/// Flux closure over the interface faces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InterfaceClosure {
    /// Net flux from zone A to zone B (zero for closure).
    pub net: f64,
    /// Sum of the absolute face fluxes.
    pub abs_sum: f64,
    /// Interface area.
    pub area: f64,
}

/// Internal faces between two cell zones with the frames of the zones.
#[derive(Debug, Clone)]
pub struct FrozenRotorInterface {
    /// Face indices (internal faces whose cells lie in different zones).
    pub faces: Vec<usize>,
    /// `+1` when the owner lies in zone A (the face normal points from A to B), else `-1`.
    pub sign: Vec<f64>,
    /// Frame-flux jump `(u_fa - u_fb) . S` of every face (zero on a surface of revolution).
    pub flux_jump: Vec<f64>,
    /// Worst `|jump| / (|omega_a - omega_b| r |S|)`; zero when the frames agree.
    pub worst_non_axisymmetry: f64,
    /// Interface area.
    pub area: f64,
    /// Frame of zone A.
    pub frame_a: RotorFrame,
    /// Frame of zone B.
    pub frame_b: RotorFrame,
}

impl FrozenRotorInterface {
    /// Interface between the cell zones `zone_a` and `zone_b` (boolean per cell). Wrong mask lengths, equal masks, no
    /// interface face, and an interface that is not a surface of revolution about the frame axis (relative non-axisymmetry
    /// above `1e-9`, which would make the frozen-rotor fluxes of the two sides disagree) are located errors.
    pub fn new(
        geom: &FvGeometry,
        zone_a: &[bool],
        zone_b: &[bool],
        frame_a: RotorFrame,
        frame_b: RotorFrame,
    ) -> Result<FrozenRotorInterface> {
        let mesh = &geom.mesh;
        let nc = mesh.n_cells();
        if zone_a.len() != nc || zone_b.len() != nc {
            return Err(bad(
                "frozen_rotor.zone",
                format!(
                    "zone masks have {} and {} entries for {nc} cells",
                    zone_a.len(),
                    zone_b.len()
                ),
            ));
        }
        if zone_a == zone_b {
            return Err(bad("frozen_rotor.zone", "the two zone masks are identical"));
        }
        let (fc, sf) = (mesh.face_centres(), mesh.face_area_vectors());
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let mut out = FrozenRotorInterface {
            faces: vec![],
            sign: vec![],
            flux_jump: vec![],
            worst_non_axisymmetry: 0.0,
            area: 0.0,
            frame_a,
            frame_b,
        };
        let dom = (frame_a.omega - frame_b.omega).abs();
        for f in 0..geom.n_internal {
            if geom.empty[f] {
                continue;
            }
            let (o, n) = (own[f], nei[f]);
            let s = if zone_a[o] && zone_b[n] {
                1.0
            } else if zone_b[o] && zone_a[n] {
                -1.0
            } else {
                continue;
            };
            let jump = frame_a.frame_flux_jump(&frame_b, fc[f], sf[f]);
            let r = frame_a.radius(fc[f]);
            let sn = vec3::norm(sf[f]);
            if dom > 0.0 && r > 0.0 && sn > 0.0 {
                let rel = jump.abs() / (dom * r * sn);
                out.worst_non_axisymmetry = out.worst_non_axisymmetry.max(rel);
                if rel > 1e-9 {
                    return Err(bad(
                        &format!("frozen_rotor.face[{f}]"),
                        format!(
                            "the interface is not a surface of revolution about the rotation axis (relative frame-flux jump {rel:.3e}); the frozen-rotor fluxes of the two sides would not close"
                        ),
                    ));
                }
            }
            out.faces.push(f);
            out.sign.push(s);
            out.flux_jump.push(jump);
            out.area += sn;
        }
        if out.faces.is_empty() {
            return Err(bad("frozen_rotor.zone", "the two zones share no interface face"));
        }
        Ok(out)
    }

    /// Closure of a face-flux field `phi` (owner to neighbour, absolute volume flux) over the interface, positive from A to B.
    pub fn closure(&self, phi: &[f64]) -> InterfaceClosure {
        let mut c = InterfaceClosure {
            net: 0.0,
            abs_sum: 0.0,
            area: self.area,
        };
        for (k, &f) in self.faces.iter().enumerate() {
            let v = self.sign[k] * phi[f];
            c.net += v;
            c.abs_sum += v.abs();
        }
        c
    }

    /// Closure of the flux relative to the frame of zone A (`phi - u_fa . S`).
    pub fn closure_relative_a(&self, geom: &FvGeometry, phi: &[f64]) -> InterfaceClosure {
        self.closure_relative(geom, phi, true)
    }

    /// Closure of the flux relative to the frame of zone B (`phi - u_fb . S`).
    pub fn closure_relative_b(&self, geom: &FvGeometry, phi: &[f64]) -> InterfaceClosure {
        self.closure_relative(geom, phi, false)
    }

    fn closure_relative(&self, geom: &FvGeometry, phi: &[f64], a: bool) -> InterfaceClosure {
        let (fc, sf) = (geom.mesh.face_centres(), geom.mesh.face_area_vectors());
        let fr = if a { &self.frame_a } else { &self.frame_b };
        let mut c = InterfaceClosure {
            net: 0.0,
            abs_sum: 0.0,
            area: self.area,
        };
        for (k, &f) in self.faces.iter().enumerate() {
            let v = self.sign[k] * (phi[f] - vec3::dot(fr.frame_velocity(fc[f]), sf[f]));
            c.net += v;
            c.abs_sum += v.abs();
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcfd_mesh::gen::{annulus_3d, Grading};
    use rcfd_mesh::non_conformal::join_non_conformal;
    use std::sync::Arc;

    /// Deterministic pseudo-random numbers in [-1, 1).
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
        }
        fn vec(&mut self) -> Vec3 {
            [self.next(), self.next(), self.next()]
        }
    }

    fn frame(omega: f64) -> RotorFrame {
        RotorFrame::new([0.3, -0.2, 0.1], [1.0, 2.0, -2.0], omega).unwrap()
    }

    #[test]
    fn r1_frame_transformation_is_exact_for_rigid_rotation() {
        let w = 3.0;
        let abs = frame(w);
        let mut rng = Lcg(7);
        let (fa, fb, fc) = (frame(1.0), frame(-2.0), frame(0.0));
        let mut worst: [f64; 4] = [0.0; 4];
        for _ in 0..200 {
            let x = vec3::scale(rng.vec(), 2.0);
            let big_r = abs.radius(x).max(1e-3);
            let scale = w * big_r;
            let u = abs.frame_velocity(x); // rigid rotation W axis x r
                                           // in the co-rotating frame the field vanishes
            worst[0] = worst[0].max(vec3::norm(abs.to_relative(u, x)) / scale);
            // relative to a, transformed to b equals (W - Omega_b) axis x r
            let wa = fa.to_relative(u, x);
            let wb = fa.change_frame(&fb, wa, x);
            let exact = vec3::scale(vec3::cross(abs.axis, vec3::sub(x, abs.origin)), w - fb.omega);
            worst[1] = worst[1].max(vec3::norm(vec3::sub(wb, exact)) / scale);
            // round trip and composition on arbitrary vectors
            let v = rng.vec();
            let back = fb.change_frame(&fa, fa.change_frame(&fb, v, x), x);
            worst[2] = worst[2].max(vec3::norm(vec3::sub(back, v)) / (vec3::norm(v) + scale));
            let direct = fa.change_frame(&fc, v, x);
            let via = fb.change_frame(&fc, fa.change_frame(&fb, v, x), x);
            worst[3] = worst[3].max(vec3::norm(vec3::sub(direct, via)) / (vec3::norm(v) + scale));
        }
        println!("R1 worst errors {worst:?} (limit 1e-14)");
        for w in worst {
            assert!(w <= 1e-14, "{worst:?}");
        }
    }

    #[test]
    fn r2_frame_flux_jump_closed_form() {
        let (fa, fb) = (
            RotorFrame::new([0.0; 3], [0.0, 0.0, 1.0], 5.0).unwrap(),
            RotorFrame::new([0.0; 3], [0.0, 0.0, 1.0], 1.5).unwrap(),
        );
        // planar annulus face (normal along the axis): no jump
        let (x, s) = ([0.7, 0.2, 0.4], [0.0, 0.0, 0.13]);
        let r = fa.radius(x);
        assert!(fa.frame_flux_jump(&fb, x, s).abs() <= 1e-14 * 3.5 * r * 0.13);
        // face in a plane through the axis (normal tangential): jump = (Oa - Ob) r_c (n_theta . S)
        let x = [0.6, 0.0, 0.3];
        let s = [0.0, 0.2, 0.0]; // n_theta at (x > 0, y = 0) is +y
        let jump = fa.frame_flux_jump(&fb, x, s);
        let exact = 3.5 * 0.6 * 0.2;
        assert!((jump - exact).abs() <= 1e-14 * exact, "{jump} {exact}");
        // reversed rotation about a tilted axis keeps the sign convention
        let fc = RotorFrame::new([0.0; 3], [0.0, 0.0, -1.0], 5.0).unwrap();
        assert!((fc.frame_velocity(x)[1] + 5.0 * 0.6).abs() < 1e-14);
    }

    fn interface_mesh() -> (Arc<rcfd_mesh::Mesh>, usize, usize) {
        let g = Grading::Uniform;
        let a = annulus_3d(48, 8, 3, 0.5, 1.0, 0.2, 0.0, g).unwrap();
        let b = annulus_3d(48, 11, 4, 0.5, 1.0, 0.2, 0.2, g).unwrap();
        let (m, rep) = join_non_conformal(&a, "zmax", &b, "zmin", 1e-8).unwrap();
        (Arc::new(m), rep.b_cell_offset, rep.interface_faces.len())
    }

    #[test]
    fn r3_interface_on_annulus_3d_interface() {
        let (mesh, nb, n_if) = interface_mesh();
        let geom = FvGeometry::new(mesh.clone()).unwrap();
        let nc = mesh.n_cells();
        let a: Vec<bool> = (0..nc).map(|c| c < nb).collect();
        let b: Vec<bool> = a.iter().map(|x| !x).collect();
        let (fa, fb) = (
            RotorFrame::new([0.0; 3], [0.0, 0.0, 1.0], 5.0).unwrap(),
            RotorFrame::new([0.0; 3], [0.0, 0.0, 1.0], 0.0).unwrap(),
        );
        let fri = FrozenRotorInterface::new(&geom, &a, &b, fa, fb).unwrap();
        let n = 48.0_f64;
        let exact = 0.5 * n * (2.0 * std::f64::consts::PI / n).sin() * (1.0 - 0.25);
        println!(
            "R3: {} faces ({} from the join), area {:.12} vs {:.12}, non-axisymmetry {:.2e}",
            fri.faces.len(),
            n_if,
            fri.area,
            exact,
            fri.worst_non_axisymmetry
        );
        assert!(fri.faces.len() > 48 * 11);
        assert_eq!(fri.faces.len(), n_if);
        assert!((fri.area - exact).abs() <= 1e-12 * exact);
        assert!(fri.worst_non_axisymmetry <= 1e-12);
        // a flux field that is divergence free across the interface closes: constant axial flux density
        let sf = mesh.face_area_vectors();
        let phi: Vec<f64> = (0..mesh.n_faces()).map(|f| 2.0 * sf[f][2]).collect();
        let c = fri.closure(&phi);
        assert!((c.abs_sum - 2.0 * fri.area).abs() <= 1e-12 * c.abs_sum);
        // the relative flux in either frame equals the absolute one (planar interface)
        let ca = fri.closure_relative_a(&geom, &phi);
        assert!((ca.net - c.net).abs() <= 1e-12 * c.abs_sum && (ca.abs_sum - c.abs_sum).abs() <= 1e-12 * c.abs_sum);
    }

    #[test]
    fn r6_located_errors() {
        assert!(RotorFrame::new([0.0; 3], [0.0; 3], 1.0)
            .unwrap_err()
            .to_string()
            .contains("rotor_frame.axis"));
        assert!(RotorFrame::new([0.0; 3], [0.0, 0.0, 1.0], f64::NAN)
            .unwrap_err()
            .to_string()
            .contains("rotor_frame.omega"));
        assert!(RotorFrame::new([f64::INFINITY, 0.0, 0.0], [0.0, 0.0, 1.0], 1.0)
            .unwrap_err()
            .to_string()
            .contains("rotor_frame.origin"));
        let (mesh, nb, _) = interface_mesh();
        let geom = FvGeometry::new(mesh.clone()).unwrap();
        let nc = mesh.n_cells();
        let a: Vec<bool> = (0..nc).map(|c| c < nb).collect();
        let b: Vec<bool> = a.iter().map(|x| !x).collect();
        let z = |om| RotorFrame::new([0.0; 3], [0.0, 0.0, 1.0], om).unwrap();
        let e = FrozenRotorInterface::new(&geom, &a[..nc - 1], &b, z(1.0), z(0.0)).unwrap_err();
        assert!(e.to_string().contains("frozen_rotor.zone"), "{e}");
        let e = FrozenRotorInterface::new(&geom, &a, &a, z(1.0), z(0.0)).unwrap_err();
        assert!(e.to_string().contains("identical"), "{e}");
        // two zones that do not touch
        let c: Vec<bool> = (0..nc).map(|i| i == 0).collect();
        let d: Vec<bool> = (0..nc).map(|i| i == nc - 1).collect();
        let e = FrozenRotorInterface::new(&geom, &c, &d, z(1.0), z(0.0)).unwrap_err();
        assert!(e.to_string().contains("no interface"), "{e}");
        // an axis that is not the interface's axis makes the planar interface a non-surface of revolution
        let tilted = RotorFrame::new([0.0; 3], [1.0, 0.0, 0.0], 1.0).unwrap();
        let e = FrozenRotorInterface::new(&geom, &a, &b, tilted, z(0.0)).unwrap_err();
        assert!(e.to_string().contains("frozen_rotor.face["), "{e}");
    }
}
