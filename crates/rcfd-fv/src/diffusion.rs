//! Diffusion operator `-div(gamma grad phi)` with non-orthogonal correction.
//!
//! Face flux (outward from the owner): `gamma [ g (phi_N - phi_P) + k . grad_f ]`
//! where `d` joins the cell centres, `Delta = (S.S / S.d) d` is the
//! over-relaxed orthogonal part and `k = S - Delta` the non-orthogonal
//! remainder, treated explicitly (deferred) with the interpolated gradient.
//! On fixed-value boundary faces the same correction uses the owner-cell gradient.
//! Assembly is data-parallel and deterministic: face coefficients are
//! independent, the diagonal is gathered per cell in a fixed order.

use crate::bc::FaceCoeffs;
use crate::fvmatrix::FvMatrix;
use crate::geometry::FvGeometry;
use crate::gradient::{limiter, Limiter};
use rayon::prelude::*;
use rcfd_core::vec3::{self, Vec3};

/// Assemble the matrix of `-div(gamma grad phi)` (positive diagonal).
///
/// * `gamma` – diffusivity at every face (length `n_faces`).
/// * `bc` – boundary coefficients (`FaceCoeffs`, indexed `f - n_internal`).
/// * `grad` – cell gradients for the non-orthogonal correction; `None`
///   disables the correction.
pub fn laplacian(geom: &FvGeometry, gamma: &[f64], bc: &[FaceCoeffs], grad: Option<&[Vec3]>) -> FvMatrix {
    laplacian_impl(geom, gamma, bc, grad, None)
}

/// Treatment of the explicit non-orthogonal flux on Dirichlet boundary faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoundaryGradient {
    /// Owner-cell gradient (the behaviour of [`laplacian`]).
    #[default]
    Owner,
    /// Owner-cell gradient with its component along the cell-to-face vector replaced by the one-sided difference
    /// `(phi_b - phi_P)/|d|`.
    Corrected,
    /// No explicit non-orthogonal flux on boundary faces.
    Dropped,
}

/// Options of the explicit non-orthogonal (secondary-gradient) flux of [`laplacian_with`] (ADR 0016, F-208).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GradientOptions {
    /// Replace the component of the interpolated face gradient along the cell-centre line by the compact difference
    /// `(phi_N - phi_P)/|d|` (face-gradient correction).
    pub face_correction: bool,
    /// Limit the cell gradients entering the secondary flux (`None` = unlimited).
    pub secondary_limiter: Option<Limiter>,
    /// Boundary treatment.
    pub boundary: BoundaryGradient,
}

/// [`laplacian`] with [`GradientOptions`]; `cells` and `bvals` are the current cell and boundary-face values (used by
/// the face-gradient correction and the limiter; the corrections are lagged, i.e. deferred).
/// `GradientOptions::default()` reproduces [`laplacian`] to round-off.
pub fn laplacian_with(
    geom: &FvGeometry,
    gamma: &[f64],
    bc: &[FaceCoeffs],
    grad: Option<&[Vec3]>,
    cells: &[f64],
    bvals: &[f64],
    opts: GradientOptions,
) -> FvMatrix {
    laplacian_impl(geom, gamma, bc, grad, Some((cells, bvals, opts)))
}

fn laplacian_impl(
    geom: &FvGeometry,
    gamma: &[f64],
    bc: &[FaceCoeffs],
    grad: Option<&[Vec3]>,
    ext: Option<(&[f64], &[f64], GradientOptions)>,
) -> FvMatrix {
    let limited: Option<Vec<Vec3>> = match (grad, ext) {
        (Some(gr), Some((cells, bvals, o))) => o.secondary_limiter.map(|kind| {
            let psi = limiter(geom, cells, bvals, gr, kind);
            gr.iter().zip(&psi).map(|(g, p)| vec3::scale(*g, *p)).collect()
        }),
        _ => None,
    };
    let grad: Option<&[Vec3]> = match &limited {
        Some(l) => Some(l.as_slice()),
        None => grad,
    };
    let mesh = &geom.mesh;
    let n_int = geom.n_internal;
    let sf = mesh.face_area_vectors();
    let (own, _nei) = (mesh.owner(), mesh.neighbour());
    let mut m = FvMatrix::zeros(geom);

    let face: Vec<(f64, f64)> = (0..n_int)
        .into_par_iter()
        .map(|f| {
            let g = gamma[f] * geom.lap_coeff[f];
            let corr = match grad {
                Some(gr) => {
                    let d = geom.delta[f];
                    let delta = vec3::scale(d, vec3::dot(sf[f], sf[f]) / vec3::dot(sf[f], d));
                    let k = vec3::sub(sf[f], delta);
                    let w = geom.weights[f];
                    let (o, n) = (own[f], mesh.neighbour()[f]);
                    let mut gf = [0, 1, 2].map(|i| w * gr[o][i] + (1.0 - w) * gr[n][i]);
                    if let Some((cells, _, opts)) = ext {
                        if opts.face_correction {
                            let len = vec3::norm(d);
                            let e = vec3::scale(d, 1.0 / len);
                            let c = (cells[n] - cells[o]) / len - vec3::dot(gf, e);
                            vec3::axpy(&mut gf, c, e);
                        }
                    }
                    gamma[f] * vec3::dot(k, gf)
                }
                None => 0.0,
            };
            (g, corr)
        })
        .collect();
    m.a.lower
        .par_iter_mut()
        .zip(m.a.upper.par_iter_mut())
        .zip(face.par_iter())
        .for_each(|((l, u), (g, _))| {
            *l = -g;
            *u = -g;
        });

    let rows: Vec<(f64, f64)> = (0..mesh.n_cells())
        .into_par_iter()
        .map(|c| {
            let (mut diag, mut rhs) = (0.0, 0.0);
            for &f in mesh.cell_faces(c) {
                if f < n_int {
                    diag += face[f].0;
                    if own[f] == c {
                        rhs += face[f].1;
                    } else {
                        rhs -= face[f].1;
                    }
                } else if !geom.empty[f] {
                    let a = vec3::norm(sf[f]);
                    let co = &bc[f - n_int];
                    diag += -gamma[f] * a * co.c;
                    rhs += gamma[f] * a * co.d;
                    // Dirichlet faces: the cell-to-face vector is not normal to the face on
                    // skewed boundary cells, so add the non-orthogonal part of the flux.
                    if let (Some(gr), true) = (grad, co.a == 0.0) {
                        let d = geom.delta[f];
                        let k = vec3::sub(sf[f], vec3::scale(d, a * a / vec3::dot(sf[f], d)));
                        match ext.map(|(_, _, o)| o.boundary).unwrap_or_default() {
                            BoundaryGradient::Owner => rhs += gamma[f] * vec3::dot(k, gr[c]),
                            BoundaryGradient::Dropped => {}
                            BoundaryGradient::Corrected => {
                                let (cells, bvals, _) = ext.expect("corrected boundary needs values");
                                let len = vec3::norm(d);
                                let e = vec3::scale(d, 1.0 / len);
                                let mut gb = gr[c];
                                let corr = (bvals[f - n_int] - cells[c]) / len - vec3::dot(gb, e);
                                vec3::axpy(&mut gb, corr, e);
                                rhs += gamma[f] * vec3::dot(k, gb);
                            }
                        }
                    }
                }
            }
            (diag, rhs)
        })
        .collect();
    for (c, (d, r)) in rows.into_iter().enumerate() {
        m.a.diag[c] = d;
        m.b[c] = r;
    }
    m
}

/// Uniform diffusivity on all faces.
pub fn uniform_gamma(geom: &FvGeometry, value: f64) -> Vec<f64> {
    vec![value; geom.mesh.n_faces()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bc::{FixedValue, Profile};
    use crate::gradient::least_squares;
    use crate::{ScalarField, ZeroGradient};
    use rcfd_linalg::SolveControl;
    use rcfd_mesh::gen::{box_mesh, warp, Grading};
    use std::sync::Arc;

    fn dirichlet_field(geom: &FvGeometry, f: impl Fn(Vec3) -> f64 + Copy) -> ScalarField {
        let mut s = ScalarField::uniform("phi", geom, 0.0);
        for p in geom.mesh.patches() {
            if p.kind == rcfd_mesh::PatchKind::Empty {
                continue;
            }
            let vals = p
                .faces()
                .map(|face| [f(geom.mesh.face_centres()[face]), 0.0, 0.0])
                .collect();
            s.set_bc(geom, &p.name, Box::new(FixedValue(Profile::PerFace(vals))))
                .unwrap();
        }
        s
    }

    #[test]
    fn matches_hand_assembled_1d_reference() {
        // 1D bar of 4 cells (h = 0.25), phi(0) = 1, phi(1) = 3 (Dirichlet), gamma = 2
        let m = Arc::new(box_mesh([4, 1, 1], [1.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap());
        let g = FvGeometry::new(m.clone()).unwrap();
        let mut phi = ScalarField::uniform("T", &g, 0.0);
        for p in m.patches() {
            let n = g.normal(p.start);
            if n[0] < -0.9 {
                phi.set_bc(&g, &p.name, Box::new(FixedValue::scalar(1.0))).unwrap();
            } else if n[0] > 0.9 {
                phi.set_bc(&g, &p.name, Box::new(FixedValue::scalar(3.0))).unwrap();
            } else {
                phi.set_bc(&g, &p.name, Box::new(ZeroGradient)).unwrap();
            }
        }
        let a = laplacian(&g, &uniform_gamma(&g, 2.0), &phi.boundary_coeffs(&g), None);
        // interior face coefficient: gamma * area / h = 2 * 0.25*... area = 1*1 = 1 -> 2/0.25 = 8
        let int_faces: Vec<usize> = (0..g.n_internal).collect();
        assert!(int_faces.iter().all(|&f| (a.a.upper[f] + 8.0).abs() < 1e-12));
        // end cells: one internal face (8) + boundary half-cell (2 * 1 / 0.125 = 16)
        assert!((a.a.diag[0] - 24.0).abs() < 1e-12 && (a.a.diag[3] - 24.0).abs() < 1e-12);
        assert!((a.a.diag[1] - 16.0).abs() < 1e-12);
        assert!((a.b[0] - 16.0).abs() < 1e-12 && (a.b[3] - 48.0).abs() < 1e-12 && a.b[1] == 0.0);
        // exact solution is linear: 1 + 2 x
        let mut x = vec![0.0; 4];
        let st = a.solve(
            &mut x,
            &SolveControl {
                tol_abs: 1e-14,
                tol_rel: 1e-12,
                max_iter: 100,
            },
        );
        assert!(st.converged);
        for (c, v) in x.iter().enumerate() {
            let xc = 0.125 + 0.25 * c as f64;
            assert!((v - (1.0 + 2.0 * xc)).abs() < 1e-10);
        }
    }

    #[test]
    fn assembly_is_thread_count_independent() {
        let m = Arc::new(box_mesh([20, 20, 20], [1.0; 3], [Grading::Uniform; 3]).unwrap());
        let g = FvGeometry::new(m).unwrap();
        let phi = dirichlet_field(&g, |p| p[0] + 2.0 * p[1]);
        let gam = uniform_gamma(&g, 1.3);
        let grad = least_squares(&g, &phi.values, &phi.boundary_values(&g));
        let build = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| laplacian(&g, &gam, &phi.boundary_coeffs(&g), Some(&grad)))
        };
        let (a, b) = (build(1), build(6));
        assert!(a.a.diag.iter().zip(&b.a.diag).all(|(x, y)| x.to_bits() == y.to_bits()));
        assert!(a.b.iter().zip(&b.b).all(|(x, y)| x.to_bits() == y.to_bits()));
    }

    /// Solve -lap(phi) = f with Dirichlet data on a smoothly warped mesh and
    /// return the L2 error against the manufactured solution.
    fn manufactured_error(n: usize, correct: bool) -> f64 {
        let pi = std::f64::consts::PI;
        let m = box_mesh([n, n, 1], [1.0, 1.0, 0.2], [Grading::Uniform; 3]).unwrap();
        let m = warp(&m, |p| {
            let s = (pi * p[0]).sin() * (pi * p[1]).sin();
            [p[0] + 0.08 * s, p[1] + 0.08 * s, p[2]]
        })
        .unwrap();
        let g = FvGeometry::new(Arc::new(m)).unwrap();
        let exact = |p: Vec3| (pi * p[0]).sin() * (pi * p[1]).cos() + p[0] * p[1];
        // lap(exact) = -2 pi^2 sin(pi x) cos(pi y)
        let lap_exact = |p: Vec3| -2.0 * pi * pi * (pi * p[0]).sin() * (pi * p[1]).cos();
        let mut phi = dirichlet_field(&g, exact);
        let gam = uniform_gamma(&g, 1.0);
        let vol = g.mesh.cell_volumes();
        let cc = g.mesh.cell_centres();
        let ctl = SolveControl {
            tol_abs: 1e-14,
            tol_rel: 1e-12,
            max_iter: 2000,
        };
        for _ in 0..if correct { 12 } else { 1 } {
            let grad = if correct {
                Some(least_squares(&g, &phi.values, &phi.boundary_values(&g)))
            } else {
                None
            };
            let mut a = laplacian(&g, &gam, &phi.boundary_coeffs(&g), grad.as_deref());
            for c in 0..a.b.len() {
                a.b[c] += -lap_exact(cc[c]) * vol[c];
            }
            let st = a.solve(&mut phi.values, &ctl);
            assert!(st.converged);
        }
        let mut e2 = 0.0;
        let mut t = 0.0;
        for c in 0..phi.values.len() {
            let d = phi.values[c] - exact(cc[c]);
            e2 += d * d * vol[c];
            t += vol[c];
        }
        (e2 / t).sqrt()
    }

    #[test]
    fn manufactured_solution_converges_at_second_order_on_skewed_mesh() {
        let e: Vec<f64> = [8usize, 16, 32, 64]
            .iter()
            .map(|&n| manufactured_error(n, true))
            .collect();
        let orders: Vec<f64> = e.windows(2).map(|w| (w[0] / w[1]).log2()).collect();
        println!("corrected: errors {e:?} orders {orders:?}");
        // boundary-cell effects make the pre-asymptotic orders slightly below 2
        assert!(
            orders.iter().all(|&o| o > 1.7) && *orders.last().unwrap() > 1.9,
            "{orders:?}"
        );
        // without the correction the scheme is inconsistent on the skewed mesh
        let u: Vec<f64> = [16usize, 32].iter().map(|&n| manufactured_error(n, false)).collect();
        println!("uncorrected: errors {u:?}");
        assert!(
            u[1] > 3.0 * e[2],
            "uncorrected error {} should exceed corrected {}",
            u[1],
            e[2]
        );
    }
}
