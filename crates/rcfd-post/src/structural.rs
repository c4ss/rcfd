//! One-way structural load export: wall pressure (face field) and temperature (cell field) mapped onto a structural mesh and
//! written as a finite-element load deck (F-237, ADR 0051 D9).
//!
//! * Pressure: [`overlap_weights`] gives conservative overlap weights between two tensor-product surface parametrisations
//!   (`(x, y)` for planes, `(theta, z)` for cylinders; the area element is constant in these parameters), [`map_face_field`]
//!   applies them, [`shell_deck`] writes the element pressures as `PLOAD4` cards on shell elements.
//! * Temperature: the cell field is transferred with [`crate::remap::Remapper`] to the structural volume mesh and recovered at
//!   the structural nodes ([`node_values`], exact for linear fields), written as `TEMP` cards.
//! * [`fluid_force_moment`] and [`deck_force_moment`] give the resultant force and moment about a point for the check of the
//!   transfer.
//!
//! Sign convention (unverified against a solver, ADR 0051 D8): shell elements are oriented with their normal pointing TOWARD the
//! fluid side; a positive `PLOAD4` value then presses on the structure along the fluid-outward face normal, i.e. the force is
//! `-p A n_shell`.

use crate::remap::{RemapMethod, Remapper};
use rcfd_core::vec3::{self, Vec3};
use rcfd_io::nastran::{Deck, ElemKind, Element, Pload4};
use rcfd_mesh::Mesh;

/// Rectangle in parameter space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamRect {
    /// Lower and upper `u`.
    pub u: [f64; 2],
    /// Lower and upper `v`.
    pub v: [f64; 2],
}

fn err<T>(m: String) -> Result<T, String> {
    Err(m)
}

/// Overlap weights `w_ts = |s cap t| / |t|` of source rectangles `s` in target rectangles `t`. `period_u` is the period of the
/// `u` parameter (2 pi for an angle) or `None`. Every target must be covered (weights sum to 1 within `1e-9`).
pub fn overlap_weights(
    src: &[ParamRect],
    tgt: &[ParamRect],
    period_u: Option<f64>,
) -> Result<Vec<Vec<(usize, f64)>>, String> {
    if src.is_empty() || tgt.is_empty() {
        return err("overlap_weights: empty source or target surface".into());
    }
    let shifts: Vec<f64> = match period_u {
        Some(p) => vec![-p, 0.0, p],
        None => vec![0.0],
    };
    let mut out = Vec::with_capacity(tgt.len());
    for (ti, t) in tgt.iter().enumerate() {
        let area = (t.u[1] - t.u[0]) * (t.v[1] - t.v[0]);
        if !(area > 0.0 && area.is_finite()) {
            return err(format!(
                "overlap_weights: target element {ti} has zero or invalid parameter area"
            ));
        }
        let mut row = Vec::new();
        let mut sum = 0.0;
        for (si, s) in src.iter().enumerate() {
            let ov_v = (t.v[1].min(s.v[1]) - t.v[0].max(s.v[0])).max(0.0);
            if ov_v == 0.0 {
                continue;
            }
            let mut ov_u = 0.0;
            for sh in &shifts {
                ov_u += (t.u[1].min(s.u[1] + sh) - t.u[0].max(s.u[0] + sh)).max(0.0);
            }
            if ov_u > 0.0 {
                let w = ov_u * ov_v / area;
                sum += w;
                row.push((si, w));
            }
        }
        if row.is_empty() {
            return err(format!(
                "overlap_weights: target element {ti} does not intersect the source surface (zero total weight)"
            ));
        }
        if (sum - 1.0).abs() > 1e-9 {
            return err(format!(
                "overlap_weights: target element {ti} is covered only to {sum:.6} by the source surface (weights must sum to 1)"
            ));
        }
        out.push(row);
    }
    Ok(out)
}

/// Apply overlap weights to a source face field.
pub fn map_face_field(weights: &[Vec<(usize, f64)>], src: &[f64]) -> Result<Vec<f64>, String> {
    if src.iter().any(|v| !v.is_finite()) {
        return err("map_face_field: the source pressure field contains a non-finite value".into());
    }
    let mut out = Vec::with_capacity(weights.len());
    for (i, row) in weights.iter().enumerate() {
        // Shifted average: the normalised weights sum to 1 only up to rounding, so averaging p directly turns a uniform field
        // into p(1 + O(n eps)). Averaging the differences to a reference value returns a uniform field exactly.
        let mut reference = 0.0;
        let mut s = 0.0;
        for (k, &(j, w)) in row.iter().enumerate() {
            let v = *src.get(j).ok_or_else(|| {
                format!(
                    "map_face_field: target element {i} refers to source face {j}, the field has {} values",
                    src.len()
                )
            })?;
            if k == 0 {
                reference = v;
            }
            s += w * (v - reference);
        }
        out.push(reference + s);
    }
    Ok(out)
}

fn patch_faces(mesh: &Mesh, patch: &str, field_len: usize) -> Result<std::ops::Range<usize>, String> {
    let p = mesh.patch(patch).ok_or_else(|| format!("unknown patch '{patch}'"))?;
    if p.size != field_len {
        return Err(format!(
            "patch '{patch}' has {} faces, the pressure array has {field_len} values",
            p.size
        ));
    }
    Ok(p.faces())
}

/// Parameter rectangles of the faces of a planar patch in the coordinate pair `axes` (for example `[0, 1]` for `(x, y)`).
pub fn patch_rects_plane(mesh: &Mesh, patch: &str, axes: [usize; 2]) -> Result<Vec<ParamRect>, String> {
    let p = mesh.patch(patch).ok_or_else(|| format!("unknown patch '{patch}'"))?;
    Ok(p.faces()
        .map(|f| {
            let mut r = ParamRect {
                u: [f64::INFINITY, f64::NEG_INFINITY],
                v: [f64::INFINITY, f64::NEG_INFINITY],
            };
            for &q in mesh.face_points(f) {
                let x = mesh.points()[q];
                r.u = [r.u[0].min(x[axes[0]]), r.u[1].max(x[axes[0]])];
                r.v = [r.v[0].min(x[axes[1]]), r.v[1].max(x[axes[1]])];
            }
            r
        })
        .collect())
}

/// Parameter rectangles `(theta in [0, 2 pi) shifted, z)` of the faces of a patch on a cylinder about the z axis.
pub fn patch_rects_cylinder(mesh: &Mesh, patch: &str) -> Result<Vec<ParamRect>, String> {
    let p = mesh.patch(patch).ok_or_else(|| format!("unknown patch '{patch}'"))?;
    let two_pi = 2.0 * std::f64::consts::PI;
    Ok(p.faces()
        .map(|f| {
            let c = mesh.face_centres()[f];
            let tc = c[1].atan2(c[0]);
            let mut r = ParamRect {
                u: [f64::INFINITY, f64::NEG_INFINITY],
                v: [f64::INFINITY, f64::NEG_INFINITY],
            };
            for &q in mesh.face_points(f) {
                let x = mesh.points()[q];
                let mut d = x[1].atan2(x[0]) - tc;
                while d > std::f64::consts::PI {
                    d -= two_pi;
                }
                while d < -std::f64::consts::PI {
                    d += two_pi;
                }
                r.u = [r.u[0].min(tc + d), r.u[1].max(tc + d)];
                r.v = [r.v[0].min(x[2]), r.v[1].max(x[2])];
            }
            r
        })
        .collect())
}

/// A structural shell surface: nodes, quads (normal toward the fluid) and the parameter rectangle of each quad.
#[derive(Debug, Clone)]
pub struct ShellSurface {
    /// Node coordinates.
    pub nodes: Vec<Vec3>,
    /// Quad connectivity (0-based node indices).
    pub quads: Vec<[usize; 4]>,
    /// Parameter rectangles.
    pub rects: Vec<ParamRect>,
}

fn check_grid(nu: usize, nv: usize) -> Result<(), String> {
    if nu < 1 || nv < 1 {
        return Err(format!(
            "shell grid needs at least 1 x 1 elements (got {nu} x {nv}), i.e. at least 2 grid lines"
        ));
    }
    Ok(())
}

/// Rectangular plane grid `origin + u e_u + v e_v`, `u` in `[0, lu]`, `v` in `[0, lv]`; `e_u x e_v` points toward the fluid
/// when `normal_toward_fluid_is_cross` is true, otherwise the quads are flipped. Parameter rectangles are in `(u, v)` plus the
/// origin offsets `u0`, `v0` (so that they live in the coordinates of the source patch).
#[allow(clippy::too_many_arguments)]
pub fn shell_plane(
    origin: Vec3,
    e_u: Vec3,
    e_v: Vec3,
    u_range: [f64; 2],
    v_range: [f64; 2],
    nu: usize,
    nv: usize,
    flip: bool,
) -> Result<ShellSurface, String> {
    check_grid(nu, nv)?;
    let mut nodes = Vec::new();
    for j in 0..=nv {
        for i in 0..=nu {
            let u = u_range[0] + (u_range[1] - u_range[0]) * i as f64 / nu as f64;
            let v = v_range[0] + (v_range[1] - v_range[0]) * j as f64 / nv as f64;
            nodes.push(vec3::add(origin, vec3::add(vec3::scale(e_u, u), vec3::scale(e_v, v))));
        }
    }
    let id = |i: usize, j: usize| i + (nu + 1) * j;
    let (mut quads, mut rects) = (Vec::new(), Vec::new());
    for j in 0..nv {
        for i in 0..nu {
            let q = [id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)];
            quads.push(if flip { [q[0], q[3], q[2], q[1]] } else { q });
            let u0 = u_range[0] + (u_range[1] - u_range[0]) * i as f64 / nu as f64;
            let u1 = u_range[0] + (u_range[1] - u_range[0]) * (i + 1) as f64 / nu as f64;
            let v0 = v_range[0] + (v_range[1] - v_range[0]) * j as f64 / nv as f64;
            let v1 = v_range[0] + (v_range[1] - v_range[0]) * (j + 1) as f64 / nv as f64;
            rects.push(ParamRect {
                u: [u0, u1],
                v: [v0, v1],
            });
        }
    }
    Ok(ShellSurface { nodes, quads, rects })
}

/// Cylinder (axis z) of radius `r`, `n_theta` chords, `n_z` axial elements between `z0` and `z1`, normals pointing to the axis
/// (the fluid is inside the tube).
pub fn shell_tube(r: f64, z0: f64, z1: f64, n_theta: usize, n_z: usize) -> Result<ShellSurface, String> {
    check_grid(n_theta, n_z)?;
    if n_theta < 3 {
        return Err("shell tube needs n_theta >= 3".into());
    }
    if !(r > 0.0 && z1 > z0) {
        return Err("shell tube needs r > 0 and z1 > z0".into());
    }
    let two_pi = 2.0 * std::f64::consts::PI;
    let mut nodes = Vec::new();
    for j in 0..=n_z {
        for i in 0..n_theta {
            let th = two_pi * i as f64 / n_theta as f64;
            nodes.push([r * th.cos(), r * th.sin(), z0 + (z1 - z0) * j as f64 / n_z as f64]);
        }
    }
    let id = |i: usize, j: usize| (i % n_theta) + n_theta * j;
    let (mut quads, mut rects) = (Vec::new(), Vec::new());
    for j in 0..n_z {
        for i in 0..n_theta {
            // (theta_i, z_j) (theta_i+1, z_j) (theta_i+1, z_j+1) (theta_i, z_j+1) has the outward normal; flipped: toward the axis
            let q = [id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)];
            quads.push([q[0], q[3], q[2], q[1]]);
            rects.push(ParamRect {
                u: [
                    two_pi * i as f64 / n_theta as f64,
                    two_pi * (i + 1) as f64 / n_theta as f64,
                ],
                v: [
                    z0 + (z1 - z0) * j as f64 / n_z as f64,
                    z0 + (z1 - z0) * (j + 1) as f64 / n_z as f64,
                ],
            });
        }
    }
    Ok(ShellSurface { nodes, quads, rects })
}

/// Area vector (normal by node order) and centroid of a quad.
fn quad_geom(p: &[Vec3; 4]) -> (Vec3, Vec3) {
    let a = vec3::scale(vec3::cross(vec3::sub(p[2], p[0]), vec3::sub(p[3], p[1])), 0.5);
    // centroid: area weighted centroids of the two triangles (0,1,2) and (0,2,3)
    let tri = |a: Vec3, b: Vec3, c: Vec3| {
        let ar = 0.5 * vec3::norm(vec3::cross(vec3::sub(b, a), vec3::sub(c, a)));
        (ar, vec3::scale(vec3::add(vec3::add(a, b), c), 1.0 / 3.0))
    };
    let (a1, c1) = tri(p[0], p[1], p[2]);
    let (a2, c2) = tri(p[0], p[2], p[3]);
    let w = a1 + a2;
    let c = if w > 0.0 {
        vec3::scale(vec3::add(vec3::scale(c1, a1), vec3::scale(c2, a2)), 1.0 / w)
    } else {
        c1
    };
    (a, c)
}

/// Resultant force and moment about `about` of the wall pressure on a patch of the fluid mesh (force on the wall, `p S_f` with
/// `S_f` outward from the fluid).
pub fn fluid_force_moment(mesh: &Mesh, patch: &str, p: &[f64], about: Vec3) -> Result<(Vec3, Vec3), String> {
    let faces = patch_faces(mesh, patch, p.len())?;
    let (mut f, mut m) = ([0.0; 3], [0.0; 3]);
    for (k, face) in faces.enumerate() {
        let df = vec3::scale(mesh.face_area_vectors()[face], p[k]);
        f = vec3::add(f, df);
        m = vec3::add(m, vec3::cross(vec3::sub(mesh.face_centres()[face], about), df));
    }
    Ok((f, m))
}

/// Resultant force and moment about `about` of the `PLOAD4` cards of load set `sid` on the quad shells of a deck (sign
/// convention of the module doc: force `-p A n_shell`). Corner pressures are averaged.
pub fn deck_force_moment(deck: &Deck, sid: i64, about: Vec3) -> Result<(Vec3, Vec3), String> {
    let pos: std::collections::HashMap<i64, Vec3> = deck.grids.iter().cloned().collect();
    let el: std::collections::HashMap<i64, &Element> = deck.elements.iter().map(|e| (e.id, e)).collect();
    let (mut f, mut m) = ([0.0; 3], [0.0; 3]);
    for l in deck.pload4.iter().filter(|l| l.sid == sid) {
        let e = el
            .get(&l.eid)
            .ok_or_else(|| format!("PLOAD4 on missing element {}", l.eid))?;
        if e.kind != ElemKind::Quad4 {
            return Err(format!(
                "element {} is not a CQUAD4 (only quad shells are summed)",
                e.id
            ));
        }
        let q: Vec<Vec3> = e
            .nodes
            .iter()
            .map(|n| {
                pos.get(n)
                    .copied()
                    .ok_or_else(|| format!("element {} refers to missing grid {n}", e.id))
            })
            .collect::<Result<_, _>>()?;
        let (a, c) = quad_geom(&[q[0], q[1], q[2], q[3]]);
        let given: Vec<f64> = l.p.iter().copied().filter(|v| !v.is_nan()).collect();
        let pm = if given.len() == 1 {
            given[0]
        } else {
            given.iter().sum::<f64>() / given.len() as f64
        };
        let df = vec3::scale(a, -pm);
        f = vec3::add(f, df);
        m = vec3::add(m, vec3::cross(vec3::sub(c, about), df));
    }
    Ok((f, m))
}

/// Build the shell load deck: grids from 1, quads from 1, one `PLOAD4` per element in load set `sid`.
pub fn shell_deck(surface: &ShellSurface, pressure: &[f64], sid: i64) -> Result<Deck, String> {
    if pressure.len() != surface.quads.len() {
        return Err(format!(
            "shell_deck: {} element pressures for {} elements",
            pressure.len(),
            surface.quads.len()
        ));
    }
    let mut d = Deck::default();
    for (i, x) in surface.nodes.iter().enumerate() {
        d.grids.push((i as i64 + 1, *x));
    }
    for (k, q) in surface.quads.iter().enumerate() {
        let (a, _) = quad_geom(&[
            surface.nodes[q[0]],
            surface.nodes[q[1]],
            surface.nodes[q[2]],
            surface.nodes[q[3]],
        ]);
        if vec3::norm(a) <= 0.0 {
            return Err(format!("shell_deck: element {} has zero area", k + 1));
        }
        d.elements.push(Element {
            kind: ElemKind::Quad4,
            id: k as i64 + 1,
            pid: 1,
            nodes: q.iter().map(|&n| n as i64 + 1).collect(),
        });
        d.pload4.push(Pload4 {
            sid,
            eid: k as i64 + 1,
            p: [pressure[k], f64::NAN, f64::NAN, f64::NAN],
        });
    }
    Ok(d)
}

fn solve3(m: [[f64; 3]; 3], b: Vec3) -> Option<Vec3> {
    let det = |a: [[f64; 3]; 3]| {
        a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
            + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0])
    };
    let d = det(m);
    let scale = m.iter().flatten().fold(0.0f64, |s, v| s.max(v.abs()));
    if d.abs() <= 1e-12 * scale * scale * scale {
        return None;
    }
    let mut x = [0.0; 3];
    for (k, xk) in x.iter_mut().enumerate() {
        let mut a = m;
        for i in 0..3 {
            a[i][k] = b[i];
        }
        *xk = det(a) / d;
    }
    Some(x)
}

/// Values at the nodes of `mesh` from cell values: mean over the cells around a node of `T_c + g_c . (x_node - x_c)`, `g_c` the
/// least-squares gradient over the face neighbours (exact for linear fields, boundary nodes included; a cell without a full-rank
/// stencil uses a zero gradient).
pub fn node_values(mesh: &Mesh, cells: &[f64]) -> Result<Vec<f64>, String> {
    if cells.len() != mesh.n_cells() {
        return Err(format!(
            "node_values: {} cell values for {} cells",
            cells.len(),
            mesh.n_cells()
        ));
    }
    if cells.iter().any(|v| !v.is_finite()) {
        return Err("node_values: non-finite cell value".into());
    }
    let cc = mesh.cell_centres();
    let mut grad = vec![[0.0; 3]; mesh.n_cells()];
    for (c, g) in grad.iter_mut().enumerate() {
        let (mut a, mut b) = ([[0.0; 3]; 3], [0.0; 3]);
        for nb in mesh.cell_neighbours(c) {
            let d = vec3::sub(cc[nb], cc[c]);
            let w = 1.0 / vec3::dot(d, d);
            for i in 0..3 {
                for j in 0..3 {
                    a[i][j] += w * d[i] * d[j];
                }
                b[i] += w * d[i] * (cells[nb] - cells[c]);
            }
        }
        if let Some(x) = solve3(a, b) {
            *g = x;
        }
    }
    let mut sum = vec![0.0; mesh.n_points()];
    let mut cnt = vec![0usize; mesh.n_points()];
    for c in 0..mesh.n_cells() {
        let mut pts: Vec<usize> = mesh
            .cell_faces(c)
            .iter()
            .flat_map(|&f| mesh.face_points(f).iter().copied())
            .collect();
        pts.sort_unstable();
        pts.dedup();
        for n in pts {
            let x = mesh.points()[n];
            sum[n] += cells[c] + vec3::dot(grad[c], vec3::sub(x, cc[c]));
            cnt[n] += 1;
        }
    }
    Ok(sum
        .iter()
        .zip(&cnt)
        .map(|(s, &n)| if n > 0 { s / n as f64 } else { f64::NAN })
        .collect())
}

/// Temperature at the nodes of a structural volume mesh from a fluid-side cell temperature (remap method `method`).
pub fn temperature_at_nodes(
    fluid: &Mesh,
    t_fluid: &[f64],
    structure: &Mesh,
    method: RemapMethod,
) -> Result<Vec<f64>, String> {
    if t_fluid.len() != fluid.n_cells() {
        return Err(format!(
            "temperature: {} values for {} fluid cells",
            t_fluid.len(),
            fluid.n_cells()
        ));
    }
    if t_fluid.iter().any(|v| !v.is_finite()) {
        return Err("temperature: non-finite fluid value".into());
    }
    let r = Remapper::new(fluid, structure, method).map_err(|e| e.to_string())?;
    let cells = r.scalar(t_fluid).map_err(|e| e.to_string())?;
    node_values(structure, &cells)
}

/// Deck with the nodes of a structural volume mesh and one `TEMP` set (no elements).
pub fn temperature_deck(structure: &Mesh, node_t: &[f64], sid: i64) -> Result<Deck, String> {
    if node_t.len() != structure.n_points() {
        return Err(format!(
            "temperature_deck: {} nodal values for {} nodes",
            node_t.len(),
            structure.n_points()
        ));
    }
    let mut d = Deck::default();
    for (i, x) in structure.points().iter().enumerate() {
        d.grids.push((i as i64 + 1, *x));
        d.temps.push((sid, i as i64 + 1, node_t[i]));
    }
    Ok(d)
}
