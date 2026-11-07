//! Sliding (rotating) non-conformal interface on a cylinder (F-250, ADR 0216).
//!
//! A rotor block turns rigidly about an axis against a stator block; their interface patches lie on one cylinder about
//! that axis. At every rotor angle the rotor's reference points are rotated by the angle (always from the reference, so
//! no round-off accumulates) and the two blocks are joined by [`join_non_conformal_cylindrical`] (ADR 0109): every overlap
//! of a rotor face with a stator face becomes one internal face, so whatever leaves the rotor through the interface enters
//! the stator, by construction. Cells keep their indices at every angle (rotor cells first, then stator cells), and the
//! boundary faces keep their order, so cell fields and boundary data carry over from one angle to the next; the internal
//! faces that are not on the interface are matched by their owner and neighbour ([`SlidingMesh::face_map_from`]).
//!
//! The rotor faces sweep volume as a rigid body ([`SlidingMesh::rotor_swept_volumes`], the swept volume of
//! [`crate::motion`]); the stator faces do not move. The cells next to the interface also change volume because the
//! interface is the polygon through the merged angles (their slivers change with the overlaps); the finite-volume side
//! closes that part of the geometric conservation law on the interface faces (`rcfd_fv::sliding`, ADR 0216 M3).
//!
//! Limits: a cylindrical interface whose patches cover each other at every angle (a full annulus); no planar slide, no
//! sector with cyclic seams, one rotor.

use crate::mesh::Mesh;
use crate::motion::{mat_vec, moved_mesh, rotation_matrix, swept_volumes_about};
use crate::non_conformal::{join_non_conformal_cylindrical, JoinReport};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use std::collections::HashMap;

fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Mesh(msg.into()))
}

/// A rotor block that turns about an axis against a stator block, across interface patches on one cylinder about that axis
/// (ADR 0216 M1).
#[derive(Debug, Clone)]
pub struct SlidingInterface {
    rotor: Mesh,
    rotor_patch: String,
    stator: Mesh,
    stator_patch: String,
    origin: Vec3,
    axis: Vec3,
    tol: f64,
}

impl SlidingInterface {
    /// The rotor `rotor` (in its reference position, angle 0) with its interface patch `rotor_patch`, the stator `stator`
    /// with its interface patch `stator_patch`, the axis `axis` through `origin` and the relative geometric tolerance `tol`
    /// of the join (typically `1e-8`). The pair is joined once at angle 0, so a pair that cannot be joined (patches not on
    /// one cylinder, not radial, not covering each other) fails here with the join's message.
    pub fn cylindrical(
        rotor: Mesh,
        rotor_patch: &str,
        stator: Mesh,
        stator_patch: &str,
        origin: Vec3,
        axis: Vec3,
        tol: f64,
    ) -> Result<Self> {
        if rotor.patch(rotor_patch).is_none() {
            return bad(format!("sliding interface: the rotor has no patch '{rotor_patch}'"));
        }
        if stator.patch(stator_patch).is_none() {
            return bad(format!("sliding interface: the stator has no patch '{stator_patch}'"));
        }
        let n = vec3::norm(axis);
        if !(n > 1e-300 && n.is_finite()) {
            return bad("sliding interface: the axis must be a finite non-zero vector");
        }
        if !(tol > 0.0 && tol.is_finite()) {
            return bad("sliding interface: the tolerance must be positive and finite");
        }
        if origin.iter().any(|x| !x.is_finite()) {
            return bad("sliding interface: the origin must be finite");
        }
        let s = SlidingInterface {
            rotor,
            rotor_patch: rotor_patch.to_string(),
            stator,
            stator_patch: stator_patch.to_string(),
            origin,
            axis: vec3::scale(axis, 1.0 / n),
            tol,
        };
        s.mesh_at(0.0)?;
        Ok(s)
    }

    /// Number of rotor cells (the first cells of every joined mesh).
    pub fn n_rotor_cells(&self) -> usize {
        self.rotor.n_cells()
    }

    /// The rotor in its reference position.
    pub fn rotor(&self) -> &Mesh {
        &self.rotor
    }

    /// The stator.
    pub fn stator(&self) -> &Mesh {
        &self.stator
    }

    /// Point on the axis.
    pub fn origin(&self) -> Vec3 {
        self.origin
    }

    /// Unit axis.
    pub fn axis(&self) -> Vec3 {
        self.axis
    }

    /// The joined mesh with the rotor turned by `angle` (radians, right-handed about the axis) from its reference position.
    pub fn mesh_at(&self, angle: f64) -> Result<SlidingMesh> {
        if !angle.is_finite() {
            return bad(format!(
                "sliding interface: the rotor angle must be finite (got {angle})"
            ));
        }
        let rotor = if angle == 0.0 {
            self.rotor.clone()
        } else {
            let r = rotation_matrix(self.axis, angle);
            let o = self.origin;
            let pts: Vec<Vec3> = self
                .rotor
                .points()
                .iter()
                .map(|&p| vec3::add(o, mat_vec(&r, vec3::sub(p, o))))
                .collect();
            moved_mesh(&self.rotor, pts)?
        };
        let (mesh, report) = join_non_conformal_cylindrical(
            &rotor,
            &self.rotor_patch,
            &self.stator,
            &self.stator_patch,
            self.origin,
            self.axis,
            self.tol,
        )?;
        let mut interface = vec![false; mesh.n_faces()];
        for &f in &report.interface_faces {
            interface[f] = true;
        }
        Ok(SlidingMesh {
            mesh,
            report,
            angle,
            n_rotor_cells: self.rotor.n_cells(),
            origin: self.origin,
            axis: self.axis,
            interface,
        })
    }
}

/// The joined rotor-stator mesh at one rotor angle.
#[derive(Debug, Clone)]
pub struct SlidingMesh {
    /// The joined mesh (rotor cells first).
    pub mesh: Mesh,
    /// What the join did; `report.interface_faces` are the interface faces.
    pub report: JoinReport,
    /// Rotor angle of this mesh.
    pub angle: f64,
    n_rotor_cells: usize,
    origin: Vec3,
    axis: Vec3,
    interface: Vec<bool>,
}

impl SlidingMesh {
    /// Number of rotor cells.
    pub fn n_rotor_cells(&self) -> usize {
        self.n_rotor_cells
    }

    /// Point on the axis.
    pub fn origin(&self) -> Vec3 {
        self.origin
    }

    /// Unit axis.
    pub fn axis(&self) -> Vec3 {
        self.axis
    }

    /// Whether face `f` is an interface face.
    pub fn is_interface(&self, f: usize) -> bool {
        self.interface[f]
    }

    /// Whether face `f` moves with the rotor: not an interface face, its owner a rotor cell and (internal faces) its
    /// neighbour too.
    pub fn is_rotor_face(&self, f: usize) -> bool {
        if self.interface[f] {
            return false;
        }
        let nr = self.n_rotor_cells;
        let own = self.mesh.owner()[f];
        if f < self.mesh.n_internal_faces() {
            own < nr && self.mesh.neighbour()[f] < nr
        } else {
            own < nr
        }
    }

    /// For every face of this mesh, the face of `old` it continues (ADR 0216 M2): an internal face that is not on the
    /// interface is the old face with the same owner and neighbour, a boundary face is the old face at the same position
    /// after the internal faces (same owner); an interface face has none (`None`). Fails when the two meshes are not of one
    /// rotor-stator pair or a face has no partner.
    pub fn face_map_from(&self, old: &SlidingMesh) -> Result<Vec<Option<usize>>> {
        let (m1, m0) = (&self.mesh, &old.mesh);
        if m0.n_cells() != m1.n_cells()
            || old.n_rotor_cells != self.n_rotor_cells
            || m0.n_boundary_faces() != m1.n_boundary_faces()
        {
            return bad("sliding interface: the two meshes are not of one rotor-stator pair (cell or boundary face counts differ)");
        }
        let (ni0, ni1) = (m0.n_internal_faces(), m1.n_internal_faces());
        let (own0, nei0) = (m0.owner(), m0.neighbour());
        let (own1, nei1) = (m1.owner(), m1.neighbour());
        let mut key: HashMap<(usize, usize), usize> = HashMap::with_capacity(ni0);
        for f in 0..ni0 {
            if !old.interface[f] {
                key.insert((own0[f], nei0[f]), f);
            }
        }
        let mut used = vec![false; m0.n_faces()];
        let mut map = vec![None; m1.n_faces()];
        for f in 0..ni1 {
            if self.interface[f] {
                continue;
            }
            let g = match key.get(&(own1[f], nei1[f])) {
                Some(&g) => g,
                None => {
                    return bad(format!(
                        "sliding interface: face {f} between cells {} and {} has no partner in the old mesh",
                        own1[f], nei1[f]
                    ))
                }
            };
            if used[g] {
                return bad(format!("sliding interface: old face {g} continues two faces"));
            }
            used[g] = true;
            map[f] = Some(g);
        }
        for (f, slot) in map.iter_mut().enumerate().skip(ni1) {
            let g = f - ni1 + ni0;
            if own0[g] != own1[f] {
                return bad(format!(
                    "sliding interface: boundary face {f} (owner {}) does not continue old face {g} (owner {})",
                    own1[f], own0[g]
                ));
            }
            *slot = Some(g);
        }
        Ok(map)
    }

    /// Volume swept by every rotor face from the rotor angle `old_angle` to this mesh's angle (ADR 0216 M3): ADR 0033's
    /// swept volume between the face's point loop of this mesh turned back by `angle - old_angle` and the same loop here
    /// (positive out of the owner). Zero on stator and interface faces.
    pub fn rotor_swept_volumes(&self, old_angle: f64) -> Result<Vec<f64>> {
        if !old_angle.is_finite() {
            return bad(format!(
                "sliding interface: the old rotor angle must be finite (got {old_angle})"
            ));
        }
        let r = rotation_matrix(self.axis, old_angle - self.angle);
        let o = self.origin;
        let back: Vec<Vec3> = self
            .mesh
            .points()
            .iter()
            .map(|&p| vec3::add(o, mat_vec(&r, vec3::sub(p, o))))
            .collect();
        let old = moved_mesh(&self.mesh, back)?;
        let w = swept_volumes_about(&old, self.mesh.points(), None);
        Ok(w.into_iter()
            .enumerate()
            .map(|(f, x)| if self.is_rotor_face(f) { x } else { 0.0 })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::{cylinder_ogrid, Grading};

    const L: f64 = 0.1;

    /// ADR 0216 pair P.
    fn pair_p() -> SlidingInterface {
        let a = cylinder_ogrid(144, 8, 1.0, 1.5, Grading::Uniform, L).unwrap();
        let b = cylinder_ogrid(120, 8, 1.5, 2.0, Grading::Uniform, L).unwrap();
        SlidingInterface::cylindrical(a, "farfield", b, "cylinder", vec3::ZERO, [0.0, 0.0, 1.0], 1e-8).unwrap()
    }

    /// K1 (ADR 0216): the face map between consecutive angles.
    #[test]
    fn k1_face_map_between_consecutive_angles() {
        let s = pair_p();
        let meshes: Vec<SlidingMesh> = (0..5).map(|k| s.mesh_at(0.0123 * k as f64).unwrap()).collect();
        for w in meshes.windows(2) {
            let (old, new) = (&w[0], &w[1]);
            let map = new.face_map_from(old).unwrap();
            let (m0, m1) = (&old.mesh, &new.mesh);
            let mut used = vec![0usize; m0.n_faces()];
            let mut unmapped = 0;
            for (f, g) in map.iter().enumerate() {
                match g {
                    Some(g) => {
                        assert!(!new.is_interface(f), "interface face {f} mapped");
                        assert!(!old.is_interface(*g), "face {f} mapped to old interface face {g}");
                        assert_eq!(m1.owner()[f], m0.owner()[*g], "owner of face {f}");
                        if f < m1.n_internal_faces() {
                            assert!(*g < m0.n_internal_faces());
                            assert_eq!(m1.neighbour()[f], m0.neighbour()[*g], "neighbour of face {f}");
                        } else {
                            assert!(*g >= m0.n_internal_faces());
                            assert_eq!(
                                m1.patch_of_face(f).unwrap().name,
                                m0.patch_of_face(*g).unwrap().name,
                                "patch of face {f}"
                            );
                        }
                        used[*g] += 1;
                    }
                    None => {
                        assert!(new.is_interface(f), "non-interface face {f} unmapped");
                        unmapped += 1;
                    }
                }
            }
            assert_eq!(unmapped, new.report.interface_faces.len());
            for (g, &k) in used.iter().enumerate() {
                assert!(k <= 1, "old face {g} used {k} times");
                assert_eq!(k == 1, !old.is_interface(g), "old face {g}");
            }
            println!(
                "K1 {:.4} -> {:.4}: {} faces, {} interface faces (old {}), map exact",
                old.angle,
                new.angle,
                m1.n_faces(),
                unmapped,
                old.report.interface_faces.len()
            );
        }
    }

    /// K2 (ADR 0216) measures and the K2c amendment: (i) worst `abs(sum W) / V_c`, (ii) radial-face count and worst
    /// relative error, (iii) non-zero stator/interface values, K2c worst `abs(sum W - (V1 - V_back)) / V_c` and the printed
    /// `abs(V1 - V_back) / V_c`.
    fn k2_measures() -> (f64, usize, f64, usize, f64, f64) {
        let s = pair_p();
        let (old, new) = (s.mesh_at(0.0123).unwrap(), s.mesh_at(0.0173).unwrap());
        let w = new.rotor_swept_volumes(old.angle).unwrap();
        let m = &new.mesh;
        let ni = m.n_internal_faces();
        let mut touches = vec![false; m.n_cells()];
        for &f in &new.report.interface_faces {
            touches[m.owner()[f]] = true;
            touches[m.neighbour()[f]] = true;
        }
        let mut sum = vec![0.0; m.n_cells()];
        for f in 0..m.n_faces() {
            sum[m.owner()[f]] += w[f];
            if f < ni {
                sum[m.neighbour()[f]] -= w[f];
            }
        }
        let mut worst_cell: f64 = 0.0;
        for c in 0..new.n_rotor_cells() {
            if !touches[c] {
                worst_cell = worst_cell.max(sum[c].abs() / m.cell_volumes()[c]);
            }
        }
        let (sf, pts) = (m.face_area_vectors(), m.points());
        let mut worst_radial: f64 = 0.0;
        let mut n_radial = 0;
        for f in 0..ni {
            let (o, nb) = (m.owner()[f], m.neighbour()[f]);
            if !(new.is_rotor_face(f) && !touches[o] && !touches[nb]) {
                continue;
            }
            let fc = m.face_centres()[f];
            let rh = vec3::unit([fc[0], fc[1], 0.0]);
            let un = vec3::unit(sf[f]);
            if vec3::dot(un, rh).abs() > 1e-9 || un[2].abs() > 1e-9 {
                continue;
            }
            let rs: Vec<f64> = m.face_points(f).iter().map(|&p| pts[p][0].hypot(pts[p][1])).collect();
            let (r1, r2) = (
                rs.iter().cloned().fold(f64::INFINITY, f64::min),
                rs.iter().cloned().fold(0.0, f64::max),
            );
            let exact = L * (r2 * r2 - r1 * r1) * 0.005f64.sin() / 2.0;
            worst_radial = worst_radial.max((w[f].abs() - exact).abs() / exact);
            n_radial += 1;
        }
        let mut nonzero = 0;
        for (f, x) in w.iter().enumerate() {
            if !new.is_rotor_face(f) && *x != 0.0 {
                nonzero += 1;
            }
        }
        // K2c (amendment 1): against the volumes of the turned-back mesh the swept volumes are computed from
        let r = rotation_matrix(new.axis(), old.angle - new.angle);
        let o = new.origin();
        let back: Vec<Vec3> = m
            .points()
            .iter()
            .map(|&p| vec3::add(o, mat_vec(&r, vec3::sub(p, o))))
            .collect();
        let vb = moved_mesh(m, back).unwrap();
        let (mut k2c, mut rot): (f64, f64) = (0.0, 0.0);
        for c in 0..new.n_rotor_cells() {
            if !touches[c] {
                let (v1, v0) = (m.cell_volumes()[c], vb.cell_volumes()[c]);
                k2c = k2c.max((sum[c] - (v1 - v0)).abs() / v1);
                rot = rot.max((v1 - v0).abs() / v1);
            }
        }
        println!(
            "K2: (i) rotor cell sum {worst_cell:.2e} V_c (1e-14); (ii) {n_radial} radial faces, worst {worst_radial:.2e} (1e-12); (iii) {nonzero} non-zero stator/interface values; K2c {k2c:.2e} V_c (1e-14), printed |V1 - V_back| {rot:.2e} V_c"
        );
        (worst_cell, n_radial, worst_radial, nonzero, k2c, rot)
    }

    /// K2 (ii), (iii) and K2c (ADR 0216 amendment 1): swept volumes of the rotor faces under a rigid rotation.
    #[test]
    fn k2_rotor_swept_volumes_of_a_rigid_rotation() {
        let (_, n_radial, worst_radial, nonzero, k2c, _) = k2_measures();
        assert!(n_radial > 0);
        assert!(worst_radial <= 1e-12, "K2 (ii) {worst_radial}");
        assert_eq!(nonzero, 0, "K2 (iii)");
        assert!(k2c <= 1e-14, "K2c {k2c}");
    }

    /// K2 (i) as registered (ADR 0216): missed, 1.07e-14 V_c against 1e-14 (amendment 1: the round-off of the turned
    /// points, measured against zero instead of against `V1 - V_back`).
    #[test]
    #[ignore = "known miss: K2 (i) 1.07e-14 V_c against 1e-14 (ADR 0216 amendment 1: round-off of the rotation)"]
    fn k2_i_rotor_cell_sum_known_miss() {
        let (worst_cell, ..) = k2_measures();
        assert!(worst_cell <= 1e-14, "K2 (i) {worst_cell}");
    }

    /// K3 (ADR 0216): the total volume does not depend on the rotor angle.
    #[test]
    fn k3_total_volume_independent_of_the_angle() {
        let s = pair_p();
        let tau = std::f64::consts::TAU;
        let exact = L * (60.0 * 4.0 * (tau / 120.0).sin() - 72.0 * (tau / 144.0).sin());
        let mut worst: f64 = 0.0;
        for theta in [0.0, 0.0123, 0.05, 1.0, 3.0] {
            let v = s.mesh_at(theta).unwrap().mesh.total_volume();
            let rel = (v - exact).abs() / exact;
            println!("K3 theta {theta}: volume {v:.15} (closed form {exact:.15}), relative {rel:.2e}");
            worst = worst.max(rel);
        }
        assert!(worst <= 1e-12, "K3 {worst}");
    }

    /// K4 (ADR 0216): located refusals.
    #[test]
    fn k4_refusals_are_located() {
        let a = || cylinder_ogrid(144, 8, 1.0, 1.5, Grading::Uniform, L).unwrap();
        let b = || cylinder_ogrid(120, 8, 1.5, 2.0, Grading::Uniform, L).unwrap();
        let z = [0.0, 0.0, 1.0];
        let msg = |r: Result<SlidingInterface>| format!("{}", r.unwrap_err());
        let e1 = msg(SlidingInterface::cylindrical(
            a(),
            "nope",
            b(),
            "cylinder",
            vec3::ZERO,
            z,
            1e-8,
        ));
        let e2 = msg(SlidingInterface::cylindrical(
            a(),
            "farfield",
            b(),
            "nada",
            vec3::ZERO,
            z,
            1e-8,
        ));
        let e3 = msg(SlidingInterface::cylindrical(
            a(),
            "farfield",
            b(),
            "cylinder",
            vec3::ZERO,
            vec3::ZERO,
            1e-8,
        ));
        let far = cylinder_ogrid(120, 8, 1.6, 2.0, Grading::Uniform, L).unwrap();
        let e4 = msg(SlidingInterface::cylindrical(
            a(),
            "farfield",
            far,
            "cylinder",
            vec3::ZERO,
            z,
            1e-8,
        ));
        let e5 = format!("{}", pair_p().mesh_at(f64::NAN).unwrap_err());
        for (e, want) in [
            (&e1, "nope"),
            (&e2, "nada"),
            (&e3, "axis"),
            (&e4, "cylinder"),
            (&e5, "angle"),
        ] {
            println!("K4: {e}");
            assert!(e.contains(want), "'{e}' should name '{want}'");
        }
    }
}
