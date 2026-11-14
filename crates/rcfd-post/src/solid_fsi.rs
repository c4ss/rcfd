//! The two-way coupling helper of an elastic solid with the flow solver. Declared by ADR 0291; filled by ADR 0434 (the
//! registration of F-401; design stub ADR 0304).
//!
//! [`ChannelMotion`] turns the displacement of the wall points of a flow's moving wall (in the order of
//! `cosim::FlowWall::point_ids`) into the position of every mesh point of a straight channel whose wall is the end face: every
//! point moves along the channel's axis by the axial displacement of the wall point on its line (the same transverse
//! coordinates), times a weight that falls linearly from 1 on the wall to 0 at a given distance from it. It is the
//! `cosim::MeshMotion` of the piston on an elastic bar (ADR 0434 D5). Only the axial component is applied: the transverse
//! displacement of the interface is not followed (it would move the points of the side walls off their planes).

use rcfd_core::vec3::Vec3;
use rcfd_fv::cosim::{FlowWall, MeshMotion};
use rcfd_mesh::Mesh;

/// The mesh motion of a straight channel whose wall patch is its end face (ADR 0434 D5).
#[derive(Debug, Clone)]
pub struct ChannelMotion {
    reference: Vec<Vec3>,
    axis: usize,
    /// For every mesh point: the wall point on its line and the point's weight.
    lines: Vec<(usize, f64)>,
    n_wall: usize,
}

impl ChannelMotion {
    /// The motion of `mesh` (the reference configuration) whose moving wall is `patch`: the points of every line along
    /// `axis` (0, 1 or 2) follow the wall point with the same transverse coordinates (within `tolerance`) with the weight
    /// `max(0, 1 - |x_a - x_wall|/length)`. Refuses an unknown or empty patch, an axis above 2, a `length` or `tolerance`
    /// that is not positive and finite, and a mesh point with no wall point on its line.
    pub fn new(mesh: &Mesh, patch: &str, axis: usize, length: f64, tolerance: f64) -> Result<Self, String> {
        if axis > 2 {
            return Err(format!("channel motion: the axis must be 0, 1 or 2, got {axis}"));
        }
        if !(length.is_finite() && length > 0.0) {
            return Err(format!(
                "channel motion: the length must be positive and finite, got {length}"
            ));
        }
        if !(tolerance.is_finite() && tolerance > 0.0) {
            return Err(format!(
                "channel motion: the tolerance must be positive and finite, got {tolerance}"
            ));
        }
        let wall = FlowWall::point_ids(mesh, patch).ok_or_else(|| format!("channel motion: no patch '{patch}'"))?;
        if wall.is_empty() {
            return Err(format!("channel motion: the patch '{patch}' has no points"));
        }
        let points = mesh.points();
        let (b, c) = ((axis + 1) % 3, (axis + 2) % 3);
        let mut lines = Vec::with_capacity(points.len());
        for (i, p) in points.iter().enumerate() {
            let on_line = wall.iter().position(|&q| {
                let w = points[q];
                (p[b] - w[b]).abs() <= tolerance && (p[c] - w[c]).abs() <= tolerance
            });
            let Some(k) = on_line else {
                return Err(format!(
                    "channel motion: mesh point {i} at {p:?} has no point of the wall '{patch}' on its line along axis {axis}"
                ));
            };
            let distance = (p[axis] - points[wall[k]][axis]).abs();
            lines.push((k, (1.0 - distance / length).max(0.0)));
        }
        Ok(ChannelMotion {
            reference: points.to_vec(),
            axis,
            lines,
            n_wall: wall.len(),
        })
    }

    /// Number of wall points.
    pub fn n_wall_points(&self) -> usize {
        self.n_wall
    }

    /// The weight of every mesh point.
    pub fn weights(&self) -> Vec<f64> {
        self.lines.iter().map(|l| l.1).collect()
    }

    /// The position of every mesh point for the wall points displaced by `displacements` (one per wall point, from the
    /// reference configuration). Refuses a wrong count and a non-finite value.
    pub fn positions(&self, displacements: &[Vec3]) -> Result<Vec<Vec3>, String> {
        if displacements.len() != self.n_wall {
            return Err(format!(
                "channel motion: {} displacements for {} wall points",
                displacements.len(),
                self.n_wall
            ));
        }
        if !displacements.iter().flatten().all(|x| x.is_finite()) {
            return Err("channel motion: a non-finite wall displacement".into());
        }
        Ok(self
            .reference
            .iter()
            .zip(&self.lines)
            .map(|(p, &(k, w))| {
                let mut q = *p;
                if w > 0.0 {
                    q[self.axis] += w * displacements[k][self.axis];
                }
                q
            })
            .collect())
    }

    /// The motion as the closure `cosim::FlowWall` takes.
    pub fn into_motion(self) -> MeshMotion {
        Box::new(move |d| self.positions(d))
    }
}
