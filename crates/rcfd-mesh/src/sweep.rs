//! Sweep, thin-volume and multizone hexahedral meshing (F-235, ADR 0047).
//!
//! A [`BaseMesh`] (a quadrilateral surface mesh with named boundary edge groups) is swept along a [`Path`] (a straight line, a
//! revolution about an axis, or a vertex-normal offset for thin volumes) in a prescribed number of layers with a prescribed
//! distribution ([`Grading`]). Every base quadrilateral gives one hexahedron per layer, so the result is an all-hex mesh with
//! the layer count exactly as requested. [`disc_base`] is the multizone O-grid + core decomposition of a circular cross
//! section, [`annulus_base`] the polar mesh of an annulus, so pipes and annuli are all-hex without degenerate cells.

use crate::gen::{axis_coords, Grading};
use crate::mesh::{BoundarySpec, Mesh, PatchKind};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use std::collections::HashMap;

fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Mesh(msg.into()))
}

/// A named group of boundary edges of a base mesh (becomes one lateral patch of the sweep).
#[derive(Debug, Clone)]
pub struct EdgeGroup {
    /// Patch name.
    pub name: String,
    /// Patch kind.
    pub kind: PatchKind,
    /// Boundary edges as point-id pairs.
    pub edges: Vec<[usize; 2]>,
}

/// Quadrilateral surface mesh to be swept. Quads are point-id loops; boundary edges must all belong to a group.
#[derive(Debug, Clone)]
pub struct BaseMesh {
    /// Point coordinates (3-D; a planar base lies in a plane).
    pub points: Vec<Vec3>,
    /// Quadrilaterals.
    pub quads: Vec<[usize; 4]>,
    /// Named boundary edge groups.
    pub groups: Vec<EdgeGroup>,
}

fn ekey(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

impl BaseMesh {
    /// Check indices, manifoldness and that the boundary edges are exactly the group edges (each in one group).
    pub fn check(&self) -> Result<()> {
        let mut count: HashMap<(usize, usize), usize> = HashMap::new();
        for (qi, q) in self.quads.iter().enumerate() {
            for (i, &p) in q.iter().enumerate() {
                if p >= self.points.len() {
                    return bad(format!("base quad {qi} references point {p} out of range"));
                }
                if q[(i + 1) % 4] == p || q[(i + 2) % 4] == p {
                    return bad(format!("base quad {qi} repeats a point"));
                }
            }
            for i in 0..4 {
                *count.entry(ekey(q[i], q[(i + 1) % 4])).or_insert(0) += 1;
            }
        }
        if let Some((e, _)) = count.iter().find(|(_, &c)| c > 2) {
            return bad(format!("base edge {e:?} is used by more than two quads"));
        }
        let mut owner: HashMap<(usize, usize), &str> = HashMap::new();
        for g in &self.groups {
            for e in &g.edges {
                let k = ekey(e[0], e[1]);
                match count.get(&k) {
                    Some(1) => {}
                    _ => {
                        return bad(format!(
                            "edge group '{}': edge {k:?} is not a boundary edge of the base",
                            g.name
                        ))
                    }
                }
                if owner.insert(k, &g.name).is_some() {
                    return bad(format!("base edge {k:?} is in more than one edge group"));
                }
            }
        }
        for (k, &c) in &count {
            if c == 1 && !owner.contains_key(k) {
                return bad(format!(
                    "base boundary edge {k:?} is not assigned to a named patch (every boundary edge needs an edge group)"
                ));
            }
        }
        Ok(())
    }

    /// Area of the base (sum of quad areas).
    pub fn area(&self) -> f64 {
        self.quads.iter().map(|q| vec3::norm(self.quad_area_vector(q))).sum()
    }

    fn quad_area_vector(&self, q: &[usize; 4]) -> Vec3 {
        let p = |i: usize| self.points[q[i]];
        vec3::scale(vec3::cross(vec3::sub(p(2), p(0)), vec3::sub(p(3), p(1))), 0.5)
    }

    /// Unit vertex normals (area-weighted mean of the incident quad normals).
    pub fn vertex_normals(&self) -> Result<Vec<Vec3>> {
        let mut acc = vec![vec3::ZERO; self.points.len()];
        for q in &self.quads {
            let a = self.quad_area_vector(q);
            for &p in q {
                acc[p] = vec3::add(acc[p], a);
            }
        }
        for (i, a) in acc.iter_mut().enumerate() {
            let n = vec3::norm(*a);
            if n == 0.0 {
                return bad(format!(
                    "base point {i} has no normal (no incident quad or zero net area)"
                ));
            }
            *a = vec3::scale(*a, 1.0 / n);
        }
        Ok(acc)
    }

    fn size(&self) -> f64 {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for p in &self.points {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        vec3::norm(vec3::sub(hi, lo))
    }
}

/// Rectangle `[0,lx] x [0,ly]` in the z = 0 plane, `nx x ny` quads, edge groups `xmin, xmax, ymin, ymax` (walls).
pub fn rect_base(nx: usize, ny: usize, lx: f64, ly: f64) -> Result<BaseMesh> {
    if nx < 1 || ny < 1 || lx <= 0.0 || ly <= 0.0 {
        return bad("rect_base needs nx, ny >= 1 and positive lengths");
    }
    let id = |i: usize, j: usize| i + (nx + 1) * j;
    let mut points = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            points.push([lx * i as f64 / nx as f64, ly * j as f64 / ny as f64, 0.0]);
        }
    }
    let mut quads = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            quads.push([id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)]);
        }
    }
    let grp = |name: &str, edges: Vec<[usize; 2]>| EdgeGroup {
        name: name.into(),
        kind: PatchKind::Wall,
        edges,
    };
    let groups = vec![
        grp("xmin", (0..ny).map(|j| [id(0, j), id(0, j + 1)]).collect()),
        grp("xmax", (0..ny).map(|j| [id(nx, j), id(nx, j + 1)]).collect()),
        grp("ymin", (0..nx).map(|i| [id(i, 0), id(i + 1, 0)]).collect()),
        grp("ymax", (0..nx).map(|i| [id(i, ny), id(i + 1, ny)]).collect()),
    ];
    Ok(BaseMesh { points, quads, groups })
}

/// Annulus `r_in <= r <= r_out` in the z = 0 plane, `n_theta` chords around (closed) and `n_r` radial quads graded by
/// `grading` from the inner radius. Edge groups `inner`, `outer`.
pub fn annulus_base(n_theta: usize, n_r: usize, r_in: f64, r_out: f64, grading: Grading) -> Result<BaseMesh> {
    if n_theta < 3 || n_r < 1 || r_in <= 0.0 || r_out <= r_in {
        return bad("annulus_base needs n_theta >= 3, n_r >= 1 and 0 < r_in < r_out");
    }
    let rs: Vec<f64> = axis_coords(n_r, r_out - r_in, grading)?
        .into_iter()
        .map(|r| r + r_in)
        .collect();
    let id = |it: usize, ir: usize| (it % n_theta) + n_theta * ir;
    let mut points = Vec::new();
    for &r in &rs {
        for it in 0..n_theta {
            let th = 2.0 * std::f64::consts::PI * it as f64 / n_theta as f64;
            points.push([r * th.cos(), r * th.sin(), 0.0]);
        }
    }
    let mut quads = Vec::new();
    for ir in 0..n_r {
        for it in 0..n_theta {
            quads.push([id(it, ir), id(it + 1, ir), id(it + 1, ir + 1), id(it, ir + 1)]);
        }
    }
    let groups = vec![
        EdgeGroup {
            name: "inner".into(),
            kind: PatchKind::Wall,
            edges: (0..n_theta).map(|it| [id(it, 0), id(it + 1, 0)]).collect(),
        },
        EdgeGroup {
            name: "outer".into(),
            kind: PatchKind::Wall,
            edges: (0..n_theta).map(|it| [id(it, n_r), id(it + 1, n_r)]).collect(),
        },
    ];
    Ok(BaseMesh { points, quads, groups })
}

/// Multizone disc of radius `r` in the z = 0 plane: an `n x n` Cartesian core of half-width `core_fraction * r` and four ring
/// sectors of `n x n_ring` quads each (`4 n` cells around, `n^2 + 4 n n_ring` quads in all, no degenerate cell). `grading` acts
/// on the ring radial direction starting at the core interface. Edge group `wall`.
///
/// Sector 0 has the nodes `P_m + s_l (Q_m - P_m)` with `P_m = (a, -a + 2 a m / n)` on the core boundary, `Q_m = r (cos phi_m,
/// sin phi_m)`, `phi_m = (pi/4)(-1 + 2 m / n)` on the circle; sectors 1 to 3 are exact quarter-turn rotations. Shared nodes are
/// identified structurally. The core corners have valence three with angles 90, 135 and 135 degrees.
pub fn disc_base(n: usize, n_ring: usize, r: f64, core_fraction: f64, grading: Grading) -> Result<BaseMesh> {
    if n < 1 || n_ring < 1 || r <= 0.0 || !(0.05..=0.95).contains(&core_fraction) {
        return bad("disc_base needs n >= 1, n_ring >= 1, r > 0 and 0.05 <= core_fraction <= 0.95");
    }
    let a = core_fraction * r;
    let s = axis_coords(n_ring, 1.0, grading)?;
    let core = |i: usize, j: usize| i + (n + 1) * j;
    let n_core = (n + 1) * (n + 1);
    // canonical node id of (sector k, m in 0..=n, l in 0..=n_ring)
    let node = |k: usize, m: usize, l: usize| -> usize {
        if l == 0 {
            return match k {
                0 => core(n, m),
                1 => core(n - m, n),
                2 => core(0, n - m),
                _ => core(m, 0),
            };
        }
        let (k, m) = if m == n { ((k + 1) % 4, 0) } else { (k, m) };
        n_core + (k * n + m) * n_ring + (l - 1)
    };
    let n_pts = n_core + 4 * n * n_ring;
    let mut points = vec![[0.0; 3]; n_pts];
    for j in 0..=n {
        for i in 0..=n {
            points[core(i, j)] = [
                -a + 2.0 * a * i as f64 / n as f64,
                -a + 2.0 * a * j as f64 / n as f64,
                0.0,
            ];
        }
    }
    for k in 0..4 {
        for m in 0..n {
            for l in 1..=n_ring {
                let t = m as f64 / n as f64;
                let phi = std::f64::consts::FRAC_PI_4 * (-1.0 + 2.0 * t);
                let p = [a, -a + 2.0 * a * t];
                let q = [r * phi.cos(), r * phi.sin()];
                let mut x = [p[0] + s[l] * (q[0] - p[0]), p[1] + s[l] * (q[1] - p[1])];
                for _ in 0..k {
                    x = [-x[1], x[0]];
                }
                points[node(k, m, l)] = [x[0], x[1], 0.0];
            }
        }
    }
    let mut quads = Vec::new();
    for j in 0..n {
        for i in 0..n {
            quads.push([core(i, j), core(i + 1, j), core(i + 1, j + 1), core(i, j + 1)]);
        }
    }
    let mut wall = Vec::new();
    for k in 0..4 {
        for m in 0..n {
            for l in 0..n_ring {
                quads.push([
                    node(k, m, l),
                    node(k, m, l + 1),
                    node(k, m + 1, l + 1),
                    node(k, m + 1, l),
                ]);
            }
            wall.push([node(k, m, n_ring), node(k, m + 1, n_ring)]);
        }
    }
    Ok(BaseMesh {
        points,
        quads,
        groups: vec![EdgeGroup {
            name: "wall".into(),
            kind: PatchKind::Wall,
            edges: wall,
        }],
    })
}

/// Cylindrical shell surface of radius `r` about the z axis, `n_theta` chords around (closed), `n_z` rows over `height`
/// (a curved base for thin volumes). The quad normals point outward. Edge groups `zmin`, `zmax`.
pub fn cylinder_shell_base(n_theta: usize, n_z: usize, r: f64, height: f64) -> Result<BaseMesh> {
    if n_theta < 3 || n_z < 1 || r <= 0.0 || height <= 0.0 {
        return bad("cylinder_shell_base needs n_theta >= 3, n_z >= 1, r > 0 and height > 0");
    }
    let id = |it: usize, iz: usize| (it % n_theta) + n_theta * iz;
    let mut points = Vec::new();
    for iz in 0..=n_z {
        for it in 0..n_theta {
            let th = 2.0 * std::f64::consts::PI * it as f64 / n_theta as f64;
            points.push([r * th.cos(), r * th.sin(), height * iz as f64 / n_z as f64]);
        }
    }
    let mut quads = Vec::new();
    for iz in 0..n_z {
        for it in 0..n_theta {
            quads.push([id(it, iz), id(it + 1, iz), id(it + 1, iz + 1), id(it, iz + 1)]);
        }
    }
    let groups = vec![
        EdgeGroup {
            name: "zmin".into(),
            kind: PatchKind::Wall,
            edges: (0..n_theta).map(|it| [id(it, 0), id(it + 1, 0)]).collect(),
        },
        EdgeGroup {
            name: "zmax".into(),
            kind: PatchKind::Wall,
            edges: (0..n_theta).map(|it| [id(it, n_z), id(it + 1, n_z)]).collect(),
        },
    ];
    Ok(BaseMesh { points, quads, groups })
}

/// Sweep path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Path {
    /// Translation along `dir` (any length, normalised) by `length`.
    Line {
        /// Direction.
        dir: Vec3,
        /// Sweep length.
        length: f64,
    },
    /// Rotation about the axis through `origin` along `axis` by `angle` (radians). `|angle| >= 2 pi` closes the sweep (no cap
    /// patches, point ids wrap modulo the layer count).
    Revolve {
        /// Point on the axis.
        origin: Vec3,
        /// Axis direction.
        axis: Vec3,
        /// Total angle.
        angle: f64,
    },
    /// Offset along the unit vertex normals of the base by `thickness` (thin volumes on possibly curved bases).
    NormalOffset {
        /// Total thickness.
        thickness: f64,
    },
}

/// Sweep specification.
#[derive(Debug, Clone)]
pub struct SweepSpec {
    /// Path.
    pub path: Path,
    /// Number of hexahedra through the sweep (>= 1).
    pub layers: usize,
    /// Distribution of the layer stations along the path.
    pub grading: Grading,
    /// Name of the patch at the start of the sweep (the base).
    pub bottom: String,
    /// Name of the patch at the end of the sweep.
    pub top: String,
}

fn rotate(p: Vec3, o: Vec3, k: Vec3, ang: f64) -> Vec3 {
    let v = vec3::sub(p, o);
    let (c, s) = (ang.cos(), ang.sin());
    let kv = vec3::cross(k, v);
    let kd = vec3::dot(k, v) * (1.0 - c);
    vec3::add(
        o,
        vec3::add(vec3::add(vec3::scale(v, c), vec3::scale(kv, s)), vec3::scale(k, kd)),
    )
}

/// Sweep `base` along the path (see the module documentation). Patches: `bottom` and `top` (not for a closed revolution) and
/// one per base edge group.
pub fn sweep(base: &BaseMesh, spec: &SweepSpec) -> Result<Mesh> {
    if spec.layers < 1 {
        return bad("sweep needs at least one layer");
    }
    base.check()?;
    let t = axis_coords(spec.layers, 1.0, spec.grading)?;
    let nb = base.points.len();
    let closed = matches!(spec.path, Path::Revolve { angle, .. } if angle.abs() >= 2.0 * std::f64::consts::PI - 1e-12);
    let n_st = if closed { spec.layers } else { spec.layers + 1 };
    if closed && spec.layers < 3 {
        return bad("a closed revolution needs at least three layers");
    }
    let normals = if matches!(spec.path, Path::NormalOffset { .. }) {
        Some(base.vertex_normals()?)
    } else {
        None
    };
    let axis_unit = match spec.path {
        Path::Line { dir, length } => {
            if (length.is_nan() || length <= 0.0) || vec3::norm(dir) == 0.0 {
                return bad("sweep line needs a positive length and a non-zero direction");
            }
            Some(vec3::unit(dir))
        }
        Path::Revolve { origin, axis, angle } => {
            if angle == 0.0 || vec3::norm(axis) == 0.0 {
                return bad("sweep revolution needs a non-zero angle and axis");
            }
            let k = vec3::unit(axis);
            let tol = 1e-9 * base.size();
            for (i, p) in base.points.iter().enumerate() {
                let v = vec3::sub(*p, origin);
                let d = vec3::norm(vec3::sub(v, vec3::scale(k, vec3::dot(v, k))));
                if d < tol {
                    return bad(format!(
                        "base point {i} lies on the revolution axis: a base touching the axis gives degenerate wedge cells \
                         and the mesh would not be all-hex (keep the base off the axis)"
                    ));
                }
            }
            Some(k)
        }
        Path::NormalOffset { thickness } => {
            if thickness.is_nan() || thickness <= 0.0 {
                return bad("normal offset needs a positive thickness");
            }
            None
        }
    };
    let mut points = Vec::with_capacity(nb * n_st);
    for k in 0..n_st {
        for (b, &p) in base.points.iter().enumerate() {
            let q = match spec.path {
                Path::Line { length, .. } => vec3::add(p, vec3::scale(axis_unit.expect("line"), length * t[k])),
                Path::Revolve { origin, angle, .. } => rotate(p, origin, axis_unit.expect("axis"), angle * t[k]),
                Path::NormalOffset { thickness } => {
                    vec3::add(p, vec3::scale(normals.as_ref().expect("normals")[b], thickness * t[k]))
                }
            };
            points.push(q);
        }
    }
    let pid = |b: usize, k: usize| b + nb * (k % n_st);
    let mut edge_group: HashMap<(usize, usize), usize> = HashMap::new();
    for (gi, g) in base.groups.iter().enumerate() {
        for e in &g.edges {
            edge_group.insert(ekey(e[0], e[1]), gi);
        }
    }
    let mut cells = Vec::with_capacity(base.quads.len() * spec.layers);
    let mut bottom = Vec::new();
    let mut top = Vec::new();
    let mut lateral: Vec<Vec<Vec<usize>>> = vec![Vec::new(); base.groups.len()];
    for k in 0..spec.layers {
        for q in &base.quads {
            let lo = [pid(q[0], k), pid(q[1], k), pid(q[2], k), pid(q[3], k)];
            let hi = [pid(q[0], k + 1), pid(q[1], k + 1), pid(q[2], k + 1), pid(q[3], k + 1)];
            let mut faces = vec![lo.to_vec(), hi.to_vec()];
            for i in 0..4 {
                let j = (i + 1) % 4;
                let side = vec![lo[i], lo[j], hi[j], hi[i]];
                if let Some(&gi) = edge_group.get(&ekey(q[i], q[j])) {
                    lateral[gi].push(side.clone());
                }
                faces.push(side);
            }
            if k == 0 && !closed {
                bottom.push(lo.to_vec());
            }
            if k + 1 == spec.layers && !closed {
                top.push(hi.to_vec());
            }
            cells.push(faces);
        }
    }
    let mut specs = Vec::new();
    if !closed {
        specs.push(BoundarySpec {
            name: spec.bottom.clone(),
            kind: PatchKind::Wall,
            faces: bottom,
        });
        specs.push(BoundarySpec {
            name: spec.top.clone(),
            kind: PatchKind::Wall,
            faces: top,
        });
    }
    for (g, faces) in base.groups.iter().zip(lateral) {
        specs.push(BoundarySpec {
            name: g.name.clone(),
            kind: g.kind.clone(),
            faces,
        });
    }
    specs.retain(|s| !s.faces.is_empty());
    Mesh::from_cell_faces(points, &cells, &specs)
}

/// Thin volume: one hexahedron through the thickness, offset along the vertex normals of `base`. Patches `base`, `top` and
/// the base edge groups.
pub fn thin_volume(base: &BaseMesh, thickness: f64) -> Result<Mesh> {
    sweep(
        base,
        &SweepSpec {
            path: Path::NormalOffset { thickness },
            layers: 1,
            grading: Grading::Uniform,
            bottom: "base".into(),
            top: "top".into(),
        },
    )
}

/// Circular pipe along z, all hexahedra: multizone [`disc_base`] (`n x n` core, `n_ring` ring cells) swept in `nz` uniform
/// layers over `length`. Patches `wall`, `zmin`, `zmax`; `periodic_z` makes `zmin`/`zmax` a cyclic pair.
#[allow(clippy::too_many_arguments)]
pub fn pipe_mesh(
    n: usize,
    n_ring: usize,
    nz: usize,
    radius: f64,
    length: f64,
    core_fraction: f64,
    ring_grading: Grading,
    periodic_z: bool,
) -> Result<Mesh> {
    let base = disc_base(n, n_ring, radius, core_fraction, ring_grading)?;
    let mut m = sweep(
        &base,
        &SweepSpec {
            path: Path::Line {
                dir: [0.0, 0.0, 1.0],
                length,
            },
            layers: nz,
            grading: Grading::Uniform,
            bottom: "zmin".into(),
            top: "zmax".into(),
        },
    )?;
    if periodic_z {
        m.set_periodic("zmin", "zmax")?;
        m.set_periodic("zmax", "zmin")?;
    }
    Ok(m)
}
