//! Derived fields and line/plane sampling (F-072, ADR 0020 part C).
//!
//! * Velocity-gradient based fields: [`velocity_gradient`], [`vorticity`], [`q_criterion`].
//! * Wall quantities: [`wall_shear`] (force per area of the fluid on the wall, tangential) and
//!   [`y_plus`], from the one-sided wall-normal derivative `(u_c - u_wall) / dn`.
//! * Compressible quantities: [`mach`], [`speed_of_sound`], [`total_pressure_incompressible`],
//!   [`total_pressure_isentropic`].
//! * Sampling on a line or a plane with gradient reconstruction and CSV output: [`sample_line`],
//!   [`sample_plane`], [`SampleSet`].
//!
//! Boundary arrays are indexed `f - n_internal`, like everywhere in the finite-volume crate.

use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_fv::gradient::least_squares;
use rcfd_fv::FvGeometry;

/// Velocity gradient of every cell: `g[c][k]` is the gradient of component `k`, so
/// `d u_k / d x_j = g[c][k][j]`. Least-squares reconstruction, exact for linear fields when the boundary
/// values are exact.
pub fn velocity_gradient(geom: &FvGeometry, u: &[Vec3], u_bnd: &[Vec3]) -> Vec<[Vec3; 3]> {
    let comp = |k: usize| -> Vec<Vec3> {
        let cells: Vec<f64> = u.iter().map(|v| v[k]).collect();
        let bvals: Vec<f64> = u_bnd.iter().map(|v| v[k]).collect();
        least_squares(geom, &cells, &bvals)
    };
    let (gx, gy, gz) = (comp(0), comp(1), comp(2));
    (0..u.len()).map(|c| [gx[c], gy[c], gz[c]]).collect()
}

/// Vorticity `(d w/dy - d v/dz, d u/dz - d w/dx, d v/dx - d u/dy)` of every cell.
pub fn vorticity(g: &[[Vec3; 3]]) -> Vec<Vec3> {
    g.iter()
        .map(|a| [a[2][1] - a[1][2], a[0][2] - a[2][0], a[1][0] - a[0][1]])
        .collect()
}

/// Q criterion `0.5 (|Omega|^2 - |S|^2) = -0.5 sum_kj A_kj A_jk` of every cell.
pub fn q_criterion(g: &[[Vec3; 3]]) -> Vec<f64> {
    g.iter()
        .map(|a| {
            let mut t = 0.0;
            for k in 0..3 {
                for j in 0..3 {
                    t += a[k][j] * a[j][k];
                }
            }
            -0.5 * t
        })
        .collect()
}

fn patch_faces(geom: &FvGeometry, patch: &str) -> Result<std::ops::Range<usize>> {
    geom.mesh
        .patch(patch)
        .map(|p| p.faces())
        .ok_or_else(|| Error::PathNotFound(format!("patch {patch}")))
}

/// Wall shear stress vector (force per unit area of the fluid on the wall) on every face of `patch`:
/// `rho nu (u_c - u_wall) / dn` with the wall-normal component removed. `u_wall` is the face velocity
/// (boundary array). The result is in patch face order.
pub fn wall_shear(geom: &FvGeometry, patch: &str, u: &[Vec3], u_wall: &[Vec3], rho: f64, nu: f64) -> Result<Vec<Vec3>> {
    let own = geom.mesh.owner();
    Ok(patch_faces(geom, patch)?
        .map(|f| {
            let i = f - geom.n_internal;
            let n = geom.normal(f);
            let du = vec3::sub(u[own[f]], u_wall[i]);
            let v = vec3::scale(du, rho * nu / geom.dn[i]);
            vec3::sub(v, vec3::scale(n, vec3::dot(v, n)))
        })
        .collect())
}

/// `y+ = dn sqrt(|tau| / rho) / nu` on every face of `patch`, from the wall shear vectors of [`wall_shear`].
pub fn y_plus(geom: &FvGeometry, patch: &str, tau: &[Vec3], rho: f64, nu: f64) -> Result<Vec<f64>> {
    let range = patch_faces(geom, patch)?;
    if tau.len() != range.len() {
        return Err(Error::Other(format!(
            "y_plus: {} shear vectors for {} faces of patch {patch}",
            tau.len(),
            range.len()
        )));
    }
    Ok(range
        .zip(tau)
        .map(|(f, t)| geom.dn[f - geom.n_internal] * (vec3::norm(*t) / rho).sqrt() / nu)
        .collect())
}

/// Speed of sound of an ideal gas, `sqrt(gamma R T)`.
pub fn speed_of_sound(gamma: f64, r_gas: f64, t: f64) -> f64 {
    (gamma * r_gas * t).sqrt()
}

/// Mach number `|u| / c` per cell.
pub fn mach(u: &[Vec3], c: &[f64]) -> Vec<f64> {
    u.iter().zip(c).map(|(v, c)| vec3::norm(*v) / c).collect()
}

/// Total pressure of an incompressible flow, `p + 0.5 rho |u|^2` (`p` in Pa, dynamic).
pub fn total_pressure_incompressible(p: f64, rho: f64, u: Vec3) -> f64 {
    p + 0.5 * rho * vec3::dot(u, u)
}

/// Isentropic total pressure `p (1 + (gamma - 1)/2 M^2)^(gamma/(gamma - 1))`.
pub fn total_pressure_isentropic(p: f64, mach: f64, gamma: f64) -> f64 {
    p * (1.0 + 0.5 * (gamma - 1.0) * mach * mach).powf(gamma / (gamma - 1.0))
}

/// Cell containing `point` (convex cells: the point lies on the inner side of every face plane).
/// The cell with the nearest centre is tried first, then all cells.
pub fn locate_cell(geom: &FvGeometry, point: Vec3) -> Option<usize> {
    let mesh = &geom.mesh;
    let (own, cf, sf, cc) = (
        mesh.owner(),
        mesh.face_centres(),
        mesh.face_area_vectors(),
        mesh.cell_centres(),
    );
    let contains = |c: usize| -> bool {
        mesh.cell_faces(c).iter().all(|&f| {
            let s = if own[f] == c { sf[f] } else { vec3::scale(sf[f], -1.0) };
            let len = vec3::norm(vec3::sub(cf[f], cc[c]));
            vec3::dot(vec3::sub(point, cf[f]), s) <= 1e-12 * vec3::norm(s) * len
        })
    };
    let d2 = |c: usize| {
        let r = vec3::sub(cc[c], point);
        vec3::dot(r, r)
    };
    let nearest = (0..mesh.n_cells()).min_by(|&a, &b| d2(a).total_cmp(&d2(b)))?;
    if contains(nearest) {
        return Some(nearest);
    }
    (0..mesh.n_cells()).find(|&c| contains(c))
}

/// Sample points with their cells and, per added field, the sampled values (`NaN` outside the mesh).
#[derive(Debug, Clone)]
pub struct SampleSet {
    /// Arc length from the first point for a line; empty for a plane.
    pub s: Vec<f64>,
    /// Sample points.
    pub points: Vec<Vec3>,
    /// Containing cell, `None` outside the mesh.
    pub cells: Vec<Option<usize>>,
    /// Field names.
    pub names: Vec<String>,
    /// Values, `values[field][sample]`.
    pub values: Vec<Vec<f64>>,
    /// Area represented by one sample of a plane (0 for a line).
    pub area_element: f64,
}

impl SampleSet {
    fn new(geom: &FvGeometry, s: Vec<f64>, points: Vec<Vec3>, area_element: f64) -> Self {
        let cells = points.iter().map(|&p| locate_cell(geom, p)).collect();
        SampleSet {
            s,
            points,
            cells,
            names: Vec::new(),
            values: Vec::new(),
            area_element,
        }
    }

    /// Number of samples.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Whether there are no samples.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Number of samples inside the mesh.
    pub fn n_inside(&self) -> usize {
        self.cells.iter().filter(|c| c.is_some()).count()
    }

    /// Sample a cell field: `phi_c + grad_c . (x - x_c)` with the gradient when given, else the cell value.
    pub fn add_field(&mut self, geom: &FvGeometry, name: &str, values: &[f64], grad: Option<&[Vec3]>) {
        let cc = geom.mesh.cell_centres();
        let v = self
            .points
            .iter()
            .zip(&self.cells)
            .map(|(p, c)| match c {
                Some(c) => match grad {
                    Some(g) => values[*c] + vec3::dot(g[*c], vec3::sub(*p, cc[*c])),
                    None => values[*c],
                },
                None => f64::NAN,
            })
            .collect();
        self.names.push(name.to_string());
        self.values.push(v);
    }

    /// Sum of a sampled field over the samples inside the mesh times the area element (plane integral).
    pub fn plane_integral(&self, field: usize) -> f64 {
        self.values[field].iter().filter(|v| !v.is_nan()).sum::<f64>() * self.area_element
    }

    /// CSV text: header `s,x,y,z,<names>,inside` for a line, `x,y,z,<names>,inside` for a plane; values
    /// use the shortest representation that parses back exactly; samples outside the mesh have empty
    /// value fields and `inside` 0.
    pub fn to_csv(&self) -> String {
        let line = !self.s.is_empty();
        let mut out = String::new();
        if line {
            out.push_str("s,");
        }
        out.push_str("x,y,z");
        for n in &self.names {
            out.push(',');
            out.push_str(n);
        }
        out.push_str(",inside\n");
        for i in 0..self.points.len() {
            if line {
                out.push_str(&format!("{},", self.s[i]));
            }
            let p = self.points[i];
            out.push_str(&format!("{},{},{}", p[0], p[1], p[2]));
            for v in &self.values {
                if v[i].is_nan() {
                    out.push(',');
                } else {
                    out.push_str(&format!(",{}", v[i]));
                }
            }
            out.push_str(if self.cells[i].is_some() { ",1\n" } else { ",0\n" });
        }
        out
    }
}

/// `n` points from `p0` to `p1` inclusive (`n >= 2`).
pub fn sample_line(geom: &FvGeometry, p0: Vec3, p1: Vec3, n: usize) -> Result<SampleSet> {
    if n < 2 {
        return Err(Error::Other("sample_line: at least two points are required".into()));
    }
    let len = vec3::norm(vec3::sub(p1, p0));
    let mut s = Vec::with_capacity(n);
    let mut pts = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f64 / (n - 1) as f64;
        s.push(t * len);
        pts.push(vec3::add(p0, vec3::scale(vec3::sub(p1, p0), t)));
    }
    Ok(SampleSet::new(geom, s, pts, 0.0))
}

/// `n1 x n2` cell-midpoint samples of the rectangle `origin + a e1 + b e2`, `a in [0, extent1]`,
/// `b in [0, extent2]` (`e1`, `e2` need not be unit vectors but must be orthogonal for the area element to be right).
#[allow(clippy::too_many_arguments)]
pub fn sample_plane(
    geom: &FvGeometry,
    origin: Vec3,
    e1: Vec3,
    e2: Vec3,
    extent1: f64,
    extent2: f64,
    n1: usize,
    n2: usize,
) -> Result<SampleSet> {
    if n1 == 0 || n2 == 0 {
        return Err(Error::Other("sample_plane: at least one sample per direction".into()));
    }
    let (e1, e2) = (vec3::unit(e1), vec3::unit(e2));
    let mut pts = Vec::with_capacity(n1 * n2);
    for j in 0..n2 {
        for i in 0..n1 {
            let a = (i as f64 + 0.5) / n1 as f64 * extent1;
            let b = (j as f64 + 0.5) / n2 as f64 * extent2;
            pts.push(vec3::add(origin, vec3::add(vec3::scale(e1, a), vec3::scale(e2, b))));
        }
    }
    Ok(SampleSet::new(
        geom,
        Vec::new(),
        pts,
        extent1 * extent2 / (n1 * n2) as f64,
    ))
}
