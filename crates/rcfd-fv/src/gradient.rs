//! Gradient schemes (cell/node Green-Gauss, weighted least squares) and
//! slope limiters (Barth-Jespersen, Venkatakrishnan).
//!
//! All schemes take cell values and the boundary face values produced by the
//! field's boundary conditions (`ScalarField::boundary_values`), indexed
//! `face - n_internal`. Faces on empty patches are skipped and the
//! gradient component along the empty direction is zero.

use crate::geometry::FvGeometry;
use crate::ScalarField;
use rcfd_core::vec3::{self, Vec3};

/// Cell-based Green-Gauss gradient with linear face interpolation.
///
/// `skew_corrections` extra passes add the face-gradient correction
/// `grad_f . (C_f - C_f')` where `C_f'` is the point where the line between
/// the cell centres meets the face plane's interpolation point; this removes
/// the skewness error for smooth fields.
pub fn green_gauss(geom: &FvGeometry, cells: &[f64], bvals: &[f64], skew_corrections: usize) -> Vec<Vec3> {
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut grad = gg_pass(geom, cells, bvals, None);
    for _ in 0..skew_corrections {
        let cc = mesh.cell_centres();
        let cf = mesh.face_centres();
        let corr: Vec<f64> = (0..geom.n_internal)
            .map(|f| {
                let w = geom.weights[f];
                let gf = [0, 1, 2].map(|k| w * grad[own[f]][k] + (1.0 - w) * grad[nei[f]][k]);
                let interp_pt = vec3::add(vec3::scale(cc[own[f]], w), vec3::scale(cc[nei[f]], 1.0 - w));
                vec3::dot(gf, vec3::sub(cf[f], interp_pt))
            })
            .collect();
        grad = gg_pass(geom, cells, bvals, Some(&corr));
    }
    grad
}

fn gg_pass(geom: &FvGeometry, cells: &[f64], bvals: &[f64], corr: Option<&[f64]>) -> Vec<Vec3> {
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let sf = mesh.face_area_vectors();
    let mut g = vec![[0.0; 3]; mesh.n_cells()];
    for f in 0..geom.n_internal {
        let w = geom.weights[f];
        let mut phi = w * cells[own[f]] + (1.0 - w) * cells[nei[f]];
        if let Some(c) = corr {
            phi += c[f];
        }
        vec3::axpy(&mut g[own[f]], phi, sf[f]);
        vec3::axpy(&mut g[nei[f]], -phi, sf[f]);
    }
    for f in geom.n_internal..mesh.n_faces() {
        if !geom.empty[f] {
            vec3::axpy(&mut g[own[f]], bvals[f - geom.n_internal], sf[f]);
        }
    }
    for (gc, v) in g.iter_mut().zip(mesh.cell_volumes()) {
        *gc = vec3::scale(*gc, 1.0 / v);
    }
    g
}

fn solve3(m: [[f64; 3]; 3], b: Vec3) -> Vec3 {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() < 1e-300 {
        return [0.0; 3];
    }
    let inv = |r: usize, c: usize| -> f64 {
        let (r1, r2) = ((r + 1) % 3, (r + 2) % 3);
        let (c1, c2) = ((c + 1) % 3, (c + 2) % 3);
        // cofactor of (c, r) transposed
        m[c1][r1] * m[c2][r2] - m[c1][r2] * m[c2][r1]
    };
    let mut x = [0.0; 3];
    for (r, xr) in x.iter_mut().enumerate() {
        *xr = (0..3).map(|c| inv(r, c) * b[c]).sum::<f64>() / det;
    }
    x
}

/// Solve a symmetric 3x3 system, restricting to the active directions.
fn solve_masked(mut m: [[f64; 3]; 3], b: Vec3, active: [bool; 3]) -> Vec3 {
    let mut b = b;
    for d in 0..3 {
        if !active[d] {
            for k in 0..3 {
                m[d][k] = 0.0;
                m[k][d] = 0.0;
            }
            m[d][d] = 1.0;
            b[d] = 0.0;
        }
    }
    solve3(m, b)
}

/// Inverse-distance weighted least-squares gradient (exact for linear fields
/// on any mesh when the boundary values are exact).
pub fn least_squares(geom: &FvGeometry, cells: &[f64], bvals: &[f64]) -> Vec<Vec3> {
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let n = mesh.n_cells();
    let mut mat = vec![[[0.0; 3]; 3]; n];
    let mut rhs = vec![[0.0; 3]; n];
    let mut add = |c: usize, dr: Vec3, dphi: f64| {
        let w = 1.0 / vec3::dot(dr, dr);
        for i in 0..3 {
            for j in 0..3 {
                mat[c][i][j] += w * dr[i] * dr[j];
            }
            rhs[c][i] += w * dr[i] * dphi;
        }
    };
    for f in 0..geom.n_internal {
        let (o, nb) = (own[f], nei[f]);
        let d = geom.delta[f];
        add(o, d, cells[nb] - cells[o]);
        add(nb, vec3::scale(d, -1.0), cells[o] - cells[nb]);
    }
    for f in geom.n_internal..mesh.n_faces() {
        if !geom.empty[f] {
            add(own[f], geom.delta[f], bvals[f - geom.n_internal] - cells[own[f]]);
        }
    }
    (0..n).map(|c| solve_masked(mat[c], rhs[c], geom.active)).collect()
}

/// Node interpolation stencil with Holmes-Connell pseudo-Laplacian weights.
///
/// Each node value is a weighted mean of the surrounding cell values and
/// boundary-face values; the weights reproduce linear fields exactly.
#[derive(Debug, Clone)]
pub struct NodeStencil {
    /// Per node: `(source, weight)` with source `< n_cells` a cell and
    /// otherwise `n_cells + boundary_face_index`.
    pub entries: Vec<Vec<(usize, f64)>>,
}

impl NodeStencil {
    /// Build the stencil for every mesh point.
    pub fn new(geom: &FvGeometry) -> Self {
        let mesh = &geom.mesh;
        let np = mesh.n_points();
        let nc = mesh.n_cells();
        let mut src: Vec<Vec<usize>> = vec![Vec::new(); np];
        for f in 0..mesh.n_faces() {
            for &p in mesh.face_points(f) {
                src[p].push(mesh.owner()[f]);
                if f < geom.n_internal {
                    src[p].push(mesh.neighbour()[f]);
                } else {
                    src[p].push(nc + f - geom.n_internal);
                }
            }
        }
        let pos = |s: usize| -> Vec3 {
            if s < nc {
                mesh.cell_centres()[s]
            } else {
                mesh.face_centres()[geom.n_internal + s - nc]
            }
        };
        let entries = src
            .into_iter()
            .enumerate()
            .map(|(p, mut s)| {
                s.sort_unstable();
                s.dedup();
                let x = mesh.points()[p];
                let dr: Vec<Vec3> = s.iter().map(|&i| vec3::sub(pos(i), x)).collect();
                let mut r = [0.0; 3];
                let mut ii = [[0.0; 3]; 3];
                for d in &dr {
                    for a in 0..3 {
                        r[a] += d[a];
                        for b in 0..3 {
                            ii[a][b] += d[a] * d[b];
                        }
                    }
                }
                let lam = solve_masked(ii, r, geom.active);
                let mut w: Vec<f64> = dr.iter().map(|d| 1.0 - vec3::dot(lam, *d)).collect();
                let sum: f64 = w.iter().sum();
                if sum <= 1e-12 {
                    // fall back to inverse-distance weights (not exact for linear fields)
                    w = dr.iter().map(|d| 1.0 / vec3::norm(*d).max(1e-300)).collect();
                }
                let sum: f64 = w.iter().sum();
                s.into_iter().zip(w).map(|(i, wi)| (i, wi / sum)).collect()
            })
            .collect();
        NodeStencil { entries }
    }

    /// Interpolate cell and boundary values to the nodes.
    pub fn interpolate(&self, geom: &FvGeometry, cells: &[f64], bvals: &[f64]) -> Vec<f64> {
        let nc = geom.n_cells();
        self.entries
            .iter()
            .map(|e| {
                e.iter()
                    .map(|&(i, w)| w * if i < nc { cells[i] } else { bvals[i - nc] })
                    .sum()
            })
            .collect()
    }
}

/// Node-based Green-Gauss gradient: face integrals use the mesh's triangle-fan
/// decomposition with Holmes-Connell node values, so linear fields are
/// reproduced exactly on distorted meshes with planar faces.
pub fn node_green_gauss(geom: &FvGeometry, stencil: &NodeStencil, cells: &[f64], bvals: &[f64]) -> Vec<Vec3> {
    let mesh = &geom.mesh;
    let nv = stencil.interpolate(geom, cells, bvals);
    let pts = mesh.points();
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut g = vec![[0.0; 3]; mesh.n_cells()];
    for f in 0..mesh.n_faces() {
        if geom.empty[f] {
            continue;
        }
        let ids = mesh.face_points(f);
        let flux: Vec3 = if f >= geom.n_internal {
            vec3::scale(mesh.face_area_vectors()[f], bvals[f - geom.n_internal])
        } else {
            let k = ids.len() as f64;
            let mut c = [0.0; 3];
            let mut pc = 0.0;
            for &i in ids {
                vec3::axpy(&mut c, 1.0 / k, pts[i]);
                pc += nv[i] / k;
            }
            let mut acc = [0.0; 3];
            for j in 0..ids.len() {
                let (a, b) = (ids[j], ids[(j + 1) % ids.len()]);
                let st = vec3::scale(vec3::cross(vec3::sub(pts[a], c), vec3::sub(pts[b], c)), 0.5);
                vec3::axpy(&mut acc, (nv[a] + nv[b] + pc) / 3.0, st);
            }
            acc
        };
        vec3::axpy(&mut g[own[f]], 1.0, flux);
        if f < geom.n_internal {
            vec3::axpy(&mut g[nei[f]], -1.0, flux);
        }
    }
    for (gc, v) in g.iter_mut().zip(mesh.cell_volumes()) {
        *gc = vec3::scale(*gc, 1.0 / v);
    }
    g
}

/// Slope limiter families.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Limiter {
    /// Barth-Jespersen (strictly monotone, non-differentiable).
    BarthJespersen,
    /// Venkatakrishnan with the smoothing constant `k`.
    Venkatakrishnan(f64),
}

/// Compute per-cell limiter values in `[0, 1]` for gradient `grad`.
pub fn limiter(geom: &FvGeometry, cells: &[f64], bvals: &[f64], grad: &[Vec3], kind: Limiter) -> Vec<f64> {
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let n = mesh.n_cells();
    let mut lo = cells.to_vec();
    let mut hi = cells.to_vec();
    for f in 0..geom.n_internal {
        let (o, nb) = (own[f], nei[f]);
        lo[o] = lo[o].min(cells[nb]);
        hi[o] = hi[o].max(cells[nb]);
        lo[nb] = lo[nb].min(cells[o]);
        hi[nb] = hi[nb].max(cells[o]);
    }
    for f in geom.n_internal..mesh.n_faces() {
        if !geom.empty[f] {
            let (o, v) = (own[f], bvals[f - geom.n_internal]);
            lo[o] = lo[o].min(v);
            hi[o] = hi[o].max(v);
        }
    }
    let cc = mesh.cell_centres();
    let cf = mesh.face_centres();
    let mut psi = vec![1.0f64; n];
    let mut update = |c: usize, f: usize| {
        let d = vec3::dot(grad[c], vec3::sub(cf[f], cc[c]));
        let (dmax, dmin) = (hi[c] - cells[c], lo[c] - cells[c]);
        let r = match kind {
            Limiter::BarthJespersen => {
                if d > 1e-300 {
                    (dmax / d).min(1.0)
                } else if d < -1e-300 {
                    (dmin / d).min(1.0)
                } else {
                    1.0
                }
            }
            Limiter::Venkatakrishnan(k) => {
                let eps2 = (k * mesh.cell_volumes()[c].cbrt()).powi(3);
                let dm = if d >= 0.0 { dmax } else { dmin };
                // smooth function: (dm^2 + eps^2) d + 2 d^2 dm over dm^2 + 2 d^2 + dm d + eps^2, divided by d
                if d.abs() < 1e-300 {
                    1.0
                } else {
                    let num = (dm * dm + eps2) * d + 2.0 * d * d * dm;
                    let den = dm * dm + 2.0 * d * d + dm * d + eps2;
                    (num / den / d).min(1.0)
                }
            }
        };
        psi[c] = psi[c].min(r.max(0.0));
    };
    for f in 0..geom.n_internal {
        update(own[f], f);
        update(nei[f], f);
    }
    for f in geom.n_internal..mesh.n_faces() {
        if !geom.empty[f] {
            update(own[f], f);
        }
    }
    psi
}

/// Gradient of a [`ScalarField`] with the chosen scheme.
pub fn field_gradient(geom: &FvGeometry, field: &ScalarField, scheme: Scheme) -> Vec<Vec3> {
    let b = field.boundary_values(geom);
    match scheme {
        Scheme::GreenGauss(k) => green_gauss(geom, &field.values, &b, k),
        Scheme::LeastSquares => least_squares(geom, &field.values, &b),
        Scheme::NodeGreenGauss => node_green_gauss(geom, &NodeStencil::new(geom), &field.values, &b),
    }
}

/// Gradient scheme selector.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scheme {
    /// Cell Green-Gauss with the given number of skewness corrections.
    GreenGauss(usize),
    /// Weighted least squares.
    LeastSquares,
    /// Node-based Green-Gauss.
    NodeGreenGauss,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bc::{FixedValue, Profile};
    use rcfd_mesh::gen::{box_mesh, warp, Grading};
    use std::sync::Arc;

    /// A genuinely distorted mesh with planar faces: trapezoidal compression
    /// plus shear, with graded spacing.
    fn distorted(n: usize) -> Arc<rcfd_mesh::Mesh> {
        let m = box_mesh(
            [n, n, n],
            [1.0, 1.0, 1.0],
            [Grading::Expansion(2.0), Grading::Uniform, Grading::Uniform],
        )
        .unwrap();
        let w = warp(&m, |p| {
            let y = p[1] * (1.0 + 0.4 * p[0]);
            [p[0] + 0.15 * y + 0.1 * p[2], y, p[2]]
        })
        .unwrap();
        Arc::new(w)
    }

    /// Field with exact linear values on all boundary faces.
    fn linear_field(geom: &FvGeometry, f: impl Fn(Vec3) -> f64 + Copy) -> ScalarField {
        let mut s = ScalarField::from_fn("phi", geom, f);
        for p in geom.mesh.patches().iter() {
            let vals = p
                .faces()
                .map(|face| [f(geom.mesh.face_centres()[face]), 0.0, 0.0])
                .collect();
            s.set_bc(geom, &p.name, Box::new(FixedValue(Profile::PerFace(vals))))
                .unwrap();
        }
        s
    }

    fn max_err(g: &[Vec3], exact: Vec3) -> f64 {
        g.iter().map(|v| vec3::norm(vec3::sub(*v, exact))).fold(0.0, f64::max)
    }

    #[test]
    fn least_squares_and_node_gg_exact_on_distorted_mesh() {
        let geom = FvGeometry::new(distorted(5)).unwrap();
        let lin = |p: Vec3| 1.5 + 2.0 * p[0] - 3.0 * p[1] + 0.7 * p[2];
        let phi = linear_field(&geom, lin);
        let exact = [2.0, -3.0, 0.7];
        let ls = field_gradient(&geom, &phi, Scheme::LeastSquares);
        assert!(max_err(&ls, exact) < 1e-11, "LS {}", max_err(&ls, exact));
        let ng = field_gradient(&geom, &phi, Scheme::NodeGreenGauss);
        assert!(max_err(&ng, exact) < 1e-10, "node GG {}", max_err(&ng, exact));
    }

    #[test]
    fn cell_green_gauss_exact_only_without_skewness() {
        // orthogonal graded mesh: linear weights are exact -> GG exact
        let m = Arc::new(
            box_mesh(
                [4, 4, 4],
                [1.0; 3],
                [Grading::Expansion(3.0), Grading::Symmetric(2.0), Grading::Uniform],
            )
            .unwrap(),
        );
        let geom = FvGeometry::new(m).unwrap();
        let lin = |p: Vec3| 1.0 + 2.0 * p[0] - 3.0 * p[1] + 0.5 * p[2];
        let phi = linear_field(&geom, lin);
        let g = field_gradient(&geom, &phi, Scheme::GreenGauss(0));
        assert!(
            max_err(&g, [2.0, -3.0, 0.5]) < 1e-11,
            "{}",
            max_err(&g, [2.0, -3.0, 0.5])
        );

        // distorted mesh: uncorrected GG is NOT exact, skewness correction removes the error
        let geom = FvGeometry::new(distorted(5)).unwrap();
        let phi = linear_field(&geom, lin);
        let raw = max_err(&field_gradient(&geom, &phi, Scheme::GreenGauss(0)), [2.0, -3.0, 0.5]);
        assert!(raw > 1e-3, "expected visible skewness error, got {raw}");
        let mut prev = raw;
        for k in [2usize, 6, 20] {
            let e = max_err(&field_gradient(&geom, &phi, Scheme::GreenGauss(k)), [2.0, -3.0, 0.5]);
            assert!(
                e < prev || e < 1e-10,
                "corrections must reduce the error: {e} vs {prev}"
            );
            prev = e;
        }
        assert!(prev < raw * 1e-3, "after corrections {prev} vs raw {raw}");
    }

    fn l2_gradient_error(n: usize, scheme: Scheme) -> f64 {
        let m = box_mesh([n, n, 1], [1.0, 1.0, 0.1], [Grading::Uniform; 3]).unwrap();
        let m = warp(&m, |p| {
            let a = 0.05;
            let s = (std::f64::consts::PI * p[0]).sin() * (std::f64::consts::PI * p[1]).sin();
            [p[0] + a * s, p[1] + a * s, p[2]]
        })
        .unwrap();
        let geom = FvGeometry::new(Arc::new(m)).unwrap();
        let f = |p: Vec3| (2.0 * p[0]).sin() * (1.0 + p[1]).cos();
        let phi = linear_field(&geom, f);
        let g = field_gradient(&geom, &phi, scheme);
        let cc = geom.mesh.cell_centres();
        let vol = geom.mesh.cell_volumes();
        let mut e2 = 0.0;
        let mut tot = 0.0;
        for c in 0..g.len() {
            let ex = [
                2.0 * (2.0 * cc[c][0]).cos() * (1.0 + cc[c][1]).cos(),
                -(2.0 * cc[c][0]).sin() * (1.0 + cc[c][1]).sin(),
                0.0,
            ];
            let d = vec3::sub(g[c], [ex[0], ex[1], g[c][2]]);
            e2 += vec3::dot(d, d) * vol[c];
            tot += vol[c];
        }
        (e2 / tot).sqrt()
    }

    #[test]
    fn gradient_convergence_order_on_smoothly_warped_meshes() {
        // Global L2 order is ~1.5, not 2: cells next to the boundary have one-sided
        // stencils (first-order local error) and their share of the volume shrinks like h.
        for (name, scheme, min_order) in [
            ("least squares", Scheme::LeastSquares, 1.3),
            ("node GG", Scheme::NodeGreenGauss, 1.3),
            ("cell GG + 3 corrections", Scheme::GreenGauss(3), 1.3),
        ] {
            let e: Vec<f64> = [8usize, 16, 32].iter().map(|&n| l2_gradient_error(n, scheme)).collect();
            let orders: Vec<f64> = e.windows(2).map(|w| (w[0] / w[1]).log2()).collect();
            println!("{name}: errors {e:?} orders {orders:?}");
            assert!(orders.iter().all(|&o| o > min_order), "{name}: {orders:?}");
        }
    }

    #[test]
    fn two_d_mesh_has_zero_out_of_plane_gradient() {
        let m = Arc::new(rcfd_mesh::gen::cavity_2d(6, 1.0).unwrap());
        let geom = FvGeometry::new(m).unwrap();
        let phi = ScalarField::from_fn("p", &geom, |p| 3.0 * p[0] + p[1]);
        for scheme in [Scheme::GreenGauss(0), Scheme::LeastSquares, Scheme::NodeGreenGauss] {
            let g = field_gradient(&geom, &phi, scheme);
            assert!(g.iter().all(|v| v[2].abs() < 1e-14), "{scheme:?}");
            // interior cells see the exact linear gradient (zero-gradient BCs distort only wall cells)
            let c = 2 + 6 * 3;
            assert!(
                (g[c][0] - 3.0).abs() < 1e-12 && (g[c][1] - 1.0).abs() < 1e-12,
                "{scheme:?} {:?}",
                g[c]
            );
        }
    }

    #[test]
    fn limiters_bound_reconstruction() {
        let m = Arc::new(box_mesh([40, 1, 1], [1.0, 0.1, 0.1], [Grading::Uniform; 3]).unwrap());
        let geom = FvGeometry::new(m).unwrap();
        // step profile
        let phi = ScalarField::from_fn("s", &geom, |p| if p[0] < 0.5 { 1.0 } else { 0.0 });
        let b = phi.boundary_values(&geom);
        let grad = least_squares(&geom, &phi.values, &b);
        for kind in [Limiter::BarthJespersen, Limiter::Venkatakrishnan(5.0)] {
            let psi = limiter(&geom, &phi.values, &b, &grad, kind);
            assert!(psi.iter().all(|&p| (0.0..=1.0).contains(&p)));
            // reconstructed face values stay inside the local extrema (BJ strictly, VK approximately)
            let cc = geom.mesh.cell_centres();
            let cf = geom.mesh.face_centres();
            for f in 0..geom.n_internal {
                for c in [geom.mesh.owner()[f], geom.mesh.neighbour()[f]] {
                    let v = phi.values[c] + psi[c] * vec3::dot(grad[c], vec3::sub(cf[f], cc[c]));
                    let tol = if kind == Limiter::BarthJespersen { 1e-12 } else { 0.08 };
                    assert!(v >= -tol && v <= 1.0 + tol, "{kind:?} overshoot {v}");
                }
            }
            // limiter acts at the step, not in the flat regions
            assert!(psi[5] == 1.0 || psi[5] > 0.99);
        }
        // a linear field in the interior is left untouched
        let lin = ScalarField::from_fn("l", &geom, |p| 2.0 * p[0]);
        let b = lin.boundary_values(&geom);
        let grad = least_squares(&geom, &lin.values, &b);
        let psi = limiter(&geom, &lin.values, &b, &grad, Limiter::BarthJespersen);
        assert!(psi[5..35].iter().all(|&p| (p - 1.0).abs() < 1e-12));
    }
}
