//! Convection operator `div(F phi)` in conservative form.
//!
//! The implicit part always uses first-order upwind (an M-matrix, hence
//! unconditionally bounded); higher-order schemes are added by deferred
//! correction `F (phi_HO - phi_UD)` evaluated from the current iterate and
//! moved to the right-hand side. Repeated assembly/solve iterations
//! converge to the pure higher-order solution.

use crate::bc::FaceCoeffs;
use crate::fvmatrix::FvMatrix;
use crate::geometry::FvGeometry;
use rayon::prelude::*;
use rcfd_core::vec3::{self, Vec3};

/// Flux-limiter functions `psi(r)` for TVD schemes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TvdLimiter {
    /// `max(0, min(1, r))`
    Minmod,
    /// `(r + |r|) / (1 + |r|)`
    VanLeer,
    /// `(r^2 + r) / (r^2 + 1)` for `r > 0`
    VanAlbada,
    /// `max(0, min(2r, 1), min(r, 2))`
    Superbee,
    /// `max(0, min(2r, (1 + r)/2, 2))`
    Muscl,
    /// `max(0, min(2r, (1 + 2r)/3, 2))`
    Koren,
}

impl TvdLimiter {
    /// Evaluate the limiter.
    pub fn psi(self, r: f64) -> f64 {
        match self {
            TvdLimiter::Minmod => r.clamp(0.0, 1.0),
            TvdLimiter::VanLeer => (r + r.abs()) / (1.0 + r.abs()),
            TvdLimiter::VanAlbada => {
                if r > 0.0 {
                    (r * r + r) / (r * r + 1.0)
                } else {
                    0.0
                }
            }
            TvdLimiter::Superbee => (2.0 * r).min(1.0).max(r.min(2.0)).max(0.0),
            TvdLimiter::Muscl => (2.0 * r).min(0.5 * (1.0 + r)).clamp(0.0, 2.0),
            TvdLimiter::Koren => (2.0 * r).min((1.0 + 2.0 * r) / 3.0).clamp(0.0, 2.0),
        }
    }
}

/// Face interpolation scheme for convected quantities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvScheme {
    /// First-order upwind.
    Upwind,
    /// Linear (central) interpolation.
    Central,
    /// Second-order upwind with the upwind-cell gradient.
    LinearUpwind,
    /// QUICK (quadratic upstream interpolation).
    Quick,
    /// Limited second-order (TVD) with the given limiter.
    Tvd(TvdLimiter),
}

impl ConvScheme {
    /// Whether the scheme needs cell gradients.
    pub fn needs_gradient(self) -> bool {
        !matches!(self, ConvScheme::Upwind | ConvScheme::Central)
    }
}

/// Face value of the higher-order scheme between owner `o` and neighbour `n`
/// of internal face `f` for flux direction `forward` (flux from owner to neighbour).
#[allow(clippy::too_many_arguments)]
fn face_value(
    geom: &FvGeometry,
    f: usize,
    forward: bool,
    scheme: ConvScheme,
    cells: &[f64],
    grad: Option<&[Vec3]>,
) -> f64 {
    let mesh = &geom.mesh;
    let (o, n) = (mesh.owner()[f], mesh.neighbour()[f]);
    let w = geom.weights[f];
    let (u, d) = if forward { (o, n) } else { (n, o) };
    let (pu, pd) = (cells[u], cells[d]);
    match scheme {
        ConvScheme::Upwind => pu,
        ConvScheme::Central => w * cells[o] + (1.0 - w) * cells[n],
        ConvScheme::LinearUpwind => {
            let g = grad.expect("gradient required")[u];
            pu + vec3::dot(g, vec3::sub(mesh.face_centres()[f], mesh.cell_centres()[u]))
        }
        ConvScheme::Quick | ConvScheme::Tvd(_) => {
            let g = grad.expect("gradient required")[u];
            let dvec = if forward {
                geom.delta[f]
            } else {
                vec3::scale(geom.delta[f], -1.0)
            };
            let gd = vec3::dot(g, dvec);
            match scheme {
                ConvScheme::Quick => {
                    let p_uu = pd - 2.0 * gd;
                    0.375 * pd + 0.75 * pu - 0.125 * p_uu
                }
                ConvScheme::Tvd(lim) => {
                    let diff = pd - pu;
                    let r = if diff.abs() < 1e-300 {
                        0.0
                    } else {
                        2.0 * gd / diff - 1.0
                    };
                    let lambda = if forward { 1.0 - w } else { w };
                    pu + lim.psi(r) * lambda * diff
                }
                _ => unreachable!(),
            }
        }
    }
}

/// Assemble `div(F phi)`.
///
/// * `flux` – volumetric (or mass) flux through every face, positive out of
///   the owner; `bc` – boundary coefficients of `phi`.
/// * `grad` – cell gradients (required for non-trivial schemes).
pub fn convection(
    geom: &FvGeometry,
    flux: &[f64],
    cells: &[f64],
    bc: &[FaceCoeffs],
    scheme: ConvScheme,
    grad: Option<&[Vec3]>,
) -> FvMatrix {
    let mesh = &geom.mesh;
    let n_int = geom.n_internal;
    let mut m = FvMatrix::zeros(geom);
    // explicit correction flux per internal face
    let q: Vec<f64> = (0..n_int)
        .into_par_iter()
        .map(|f| {
            if scheme == ConvScheme::Upwind {
                return 0.0;
            }
            let forward = flux[f] >= 0.0;
            let ud = face_value(geom, f, forward, ConvScheme::Upwind, cells, grad);
            let ho = face_value(geom, f, forward, scheme, cells, grad);
            flux[f] * (ho - ud)
        })
        .collect();
    m.a.lower
        .par_iter_mut()
        .zip(m.a.upper.par_iter_mut())
        .enumerate()
        .for_each(|(f, (l, u))| {
            *u = flux[f].min(0.0);
            *l = -flux[f].max(0.0);
        });
    let own = mesh.owner();
    let rows: Vec<(f64, f64)> = (0..mesh.n_cells())
        .into_par_iter()
        .map(|c| {
            let (mut diag, mut rhs) = (0.0, 0.0);
            for &f in mesh.cell_faces(c) {
                if f < n_int {
                    if own[f] == c {
                        diag += flux[f].max(0.0);
                        rhs -= q[f];
                    } else {
                        diag += (-flux[f]).max(0.0);
                        rhs += q[f];
                    }
                } else if !geom.empty[f] {
                    let co = &bc[f - n_int];
                    if geom.coupled_face[f - n_int] && co.a > 0.0 && co.a < 1.0 {
                        // periodic face: upwind between own and (lagged) partner cell keeps the
                        // matrix an M-matrix; other schemes add the central value by deferred correction
                        let q = co.b / (1.0 - co.a);
                        let fl = flux[f];
                        let ud = if fl >= 0.0 {
                            diag += fl;
                            cells[c]
                        } else {
                            rhs -= fl * q;
                            q
                        };
                        if scheme != ConvScheme::Upwind {
                            rhs -= fl * (co.a * cells[c] + co.b - ud);
                        }
                    } else {
                        diag += flux[f] * co.a;
                        rhs -= flux[f] * co.b;
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

/// Volumetric flux `u_f . S_f` with linear interpolation of the cell
/// velocities and prescribed boundary velocities (empty faces carry no flux).
pub fn face_flux(geom: &FvGeometry, u: &[Vec3], boundary_u: &[Vec3]) -> Vec<f64> {
    let mesh = &geom.mesh;
    let sf = mesh.face_area_vectors();
    (0..mesh.n_faces())
        .into_par_iter()
        .map(|f| {
            if f < geom.n_internal {
                let w = geom.weights[f];
                let (o, n) = (mesh.owner()[f], mesh.neighbour()[f]);
                let uf = [0, 1, 2].map(|k| w * u[o][k] + (1.0 - w) * u[n][k]);
                vec3::dot(uf, sf[f])
            } else if geom.empty[f] {
                0.0
            } else {
                vec3::dot(boundary_u[f - geom.n_internal], sf[f])
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bc::{FixedValue, Profile};
    use crate::gradient::least_squares;
    use crate::ScalarField;
    use rcfd_linalg::SolveControl;
    use rcfd_mesh::gen::{box_mesh, channel, Grading};
    use rcfd_mesh::PatchKind;
    use std::sync::Arc;

    const ALL: [(&str, ConvScheme); 8] = [
        ("upwind", ConvScheme::Upwind),
        ("central", ConvScheme::Central),
        ("linearUpwind", ConvScheme::LinearUpwind),
        ("QUICK", ConvScheme::Quick),
        ("vanLeer", ConvScheme::Tvd(TvdLimiter::VanLeer)),
        ("MUSCL", ConvScheme::Tvd(TvdLimiter::Muscl)),
        ("superbee", ConvScheme::Tvd(TvdLimiter::Superbee)),
        ("minmod", ConvScheme::Tvd(TvdLimiter::Minmod)),
    ];

    #[test]
    fn limiters_are_tvd() {
        for l in [
            TvdLimiter::Minmod,
            TvdLimiter::VanLeer,
            TvdLimiter::VanAlbada,
            TvdLimiter::Superbee,
            TvdLimiter::Muscl,
            TvdLimiter::Koren,
        ] {
            assert_eq!(l.psi(-1.0), 0.0, "{l:?}");
            assert_eq!(l.psi(0.0), 0.0, "{l:?}");
            assert!(
                (l.psi(1.0) - 1.0).abs() < 1e-12,
                "{l:?} second-order consistency at r=1"
            );
            for k in 1..100 {
                let r = k as f64 * 0.1;
                let p = l.psi(r);
                assert!(
                    (0.0..=2.0 + 1e-12).contains(&p) && p <= 2.0 * r + 1e-12,
                    "{l:?} r={r} psi={p}"
                );
            }
        }
    }

    /// 1D convection-diffusion u phi' = G phi'', phi(0)=0, phi(1)=1.
    fn steady_1d_error(n: usize, scheme: ConvScheme, pe: f64) -> f64 {
        let m = Arc::new(box_mesh([n, 1, 1], [1.0, 0.1, 0.1], [Grading::Uniform; 3]).unwrap());
        let g = FvGeometry::new(m).unwrap();
        let mut phi = ScalarField::uniform("phi", &g, 0.0);
        phi.set_bc(&g, "xmin", Box::new(FixedValue::scalar(0.0))).unwrap();
        phi.set_bc(&g, "xmax", Box::new(FixedValue::scalar(1.0))).unwrap();
        let (u, gamma) = (1.0, 1.0 / pe);
        let uvec = vec![[u, 0.0, 0.0]; g.n_cells()];
        let bu = vec![[u, 0.0, 0.0]; g.mesh.n_boundary_faces()];
        let flux = face_flux(&g, &uvec, &bu);
        let gam = crate::diffusion::uniform_gamma(&g, gamma);
        let ctl = SolveControl {
            tol_abs: 1e-14,
            tol_rel: 1e-12,
            max_iter: 1000,
        };
        for _ in 0..40 {
            let bc = phi.boundary_coeffs(&g);
            let grad = least_squares(&g, &phi.values, &phi.boundary_values(&g));
            let mut a = convection(&g, &flux, &phi.values, &bc, scheme, Some(&grad));
            a.add(&crate::diffusion::laplacian(&g, &gam, &bc, None));
            a.solve(&mut phi.values, &ctl);
        }
        let exact = |x: f64| ((pe * x).exp() - 1.0) / (pe.exp() - 1.0);
        let cc = g.mesh.cell_centres();
        let e2: f64 = (0..n).map(|c| (phi.values[c] - exact(cc[c][0])).powi(2)).sum::<f64>() / n as f64;
        e2.sqrt()
    }

    #[test]
    fn steady_1d_convergence_orders() {
        let pe = 4.0;
        for (name, scheme, lo, hi) in [
            ("upwind", ConvScheme::Upwind, 0.85, 1.2),
            ("central", ConvScheme::Central, 1.85, 2.2),
            ("linearUpwind", ConvScheme::LinearUpwind, 1.5, 3.2),
            ("QUICK", ConvScheme::Quick, 1.5, 3.2),
            ("vanLeer", ConvScheme::Tvd(TvdLimiter::VanLeer), 1.3, 3.2),
        ] {
            let e: Vec<f64> = [20usize, 40, 80]
                .iter()
                .map(|&n| steady_1d_error(n, scheme, pe))
                .collect();
            let orders: Vec<f64> = e.windows(2).map(|w| (w[0] / w[1]).log2()).collect();
            println!("{name}: errors {e:?} orders {orders:?}");
            let o = *orders.last().unwrap();
            assert!(o > lo && o < hi, "{name}: orders {orders:?}");
        }
    }

    /// Periodic-free transient advection of a step with implicit Euler plus
    /// deferred correction (inner iterations), returns the profile.
    fn advect_step(scheme: ConvScheme, steps: usize) -> Vec<f64> {
        let n = 100;
        let m = Arc::new(box_mesh([n, 1, 1], [1.0, 0.05, 0.05], [Grading::Uniform; 3]).unwrap());
        let g = FvGeometry::new(m).unwrap();
        let mut phi = ScalarField::from_fn("phi", &g, |p| if p[0] < 0.3 { 1.0 } else { 0.0 });
        phi.set_bc(&g, "xmin", Box::new(FixedValue(Profile::scalar(1.0))))
            .unwrap();
        let uvec = vec![[1.0, 0.0, 0.0]; g.n_cells()];
        let bu = vec![[1.0, 0.0, 0.0]; g.mesh.n_boundary_faces()];
        let flux = face_flux(&g, &uvec, &bu);
        let dt = 0.4 / n as f64; // CFL 0.4
        let vol = g.mesh.cell_volumes().to_vec();
        let ctl = SolveControl {
            tol_abs: 1e-14,
            tol_rel: 1e-12,
            max_iter: 1000,
        };
        for _ in 0..steps {
            let old = phi.values.clone();
            for _ in 0..15 {
                let bc = phi.boundary_coeffs(&g);
                let grad = least_squares(&g, &phi.values, &phi.boundary_values(&g));
                let mut a = convection(&g, &flux, &phi.values, &bc, scheme, Some(&grad));
                for c in 0..n {
                    a.a.diag[c] += vol[c] / dt;
                    a.b[c] += vol[c] / dt * old[c];
                }
                a.solve(&mut phi.values, &ctl);
            }
        }
        phi.values
    }

    #[test]
    fn step_advection_boundedness_and_sharpness() {
        let steps = 100; // front travels 0.4 -> x = 0.7
        let upwind = advect_step(ConvScheme::Upwind, steps);
        let central = advect_step(ConvScheme::Central, steps);
        let width = |v: &[f64]| v.iter().filter(|&&x| x > 0.02 && x < 0.98).count();
        let overshoot = |v: &[f64]| v.iter().fold(0.0f64, |m, &x| m.max(x - 1.0).max(-x));
        println!("upwind width {} overshoot {:e}", width(&upwind), overshoot(&upwind));
        println!("central overshoot {:e}", overshoot(&central));
        assert!(overshoot(&upwind) < 1e-9);
        assert!(overshoot(&central) > 0.01, "central must visibly overshoot");
        for (name, s) in ALL
            .iter()
            .filter(|(n, _)| ["vanLeer", "MUSCL", "superbee", "minmod"].contains(n))
        {
            let v = advect_step(*s, steps);
            let (w, o) = (width(&v), overshoot(&v));
            println!("{name}: width {w} overshoot {o:e}");
            assert!(o < 1e-3, "{name} overshoot {o}");
            assert!(
                w < width(&upwind),
                "{name} should be sharper than upwind: {w} vs {}",
                width(&upwind)
            );
            // front is at the right place
            let mass: f64 = v.iter().sum::<f64>() / 100.0;
            assert!((mass - 0.7).abs() < 0.02, "{name}: mass {mass}");
        }
    }

    #[test]
    fn two_d_oblique_advection_converges() {
        // phi = g(eta), eta = y cos(t) - x sin(t) is constant along the flow direction (cos t, sin t)
        let (c, s) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
        let gfun = |eta: f64| (std::f64::consts::PI * (eta + 0.5)).sin().powi(2);
        let exact = move |p: Vec3| gfun(p[1] * c - p[0] * s);
        let run = |n: usize, scheme: ConvScheme| -> f64 {
            let m = Arc::new(channel([n, n, 1], [1.0, 1.0, 1.0 / n as f64], [Grading::Uniform; 3], true).unwrap());
            let g = FvGeometry::new(m).unwrap();
            let mut phi = ScalarField::uniform("phi", &g, 0.0);
            let u = [c, s, 0.0];
            let bu = vec![u; g.mesh.n_boundary_faces()];
            let flux = face_flux(&g, &vec![u; g.n_cells()], &bu);
            // exact values where the flux enters, free (zero gradient) where it leaves
            #[derive(Debug, Clone)]
            struct InOut(Vec<f64>, Vec<bool>);
            impl crate::Bc for InOut {
                fn type_name(&self) -> &'static str {
                    "inletOutlet"
                }
                fn coeffs(&self, ctx: &crate::BcCtx, _: usize) -> FaceCoeffs {
                    if self.1[ctx.local_face] {
                        FaceCoeffs::from_value(1.0, 0.0, ctx.dn)
                    } else {
                        FaceCoeffs::from_value(0.0, self.0[ctx.local_face], ctx.dn)
                    }
                }
                fn clone_box(&self) -> Box<dyn crate::Bc> {
                    Box::new(self.clone())
                }
            }
            for p in g.mesh.patches() {
                if p.kind == PatchKind::Empty {
                    continue;
                }
                let vals: Vec<f64> = p.faces().map(|f| exact(g.mesh.face_centres()[f])).collect();
                let outflow: Vec<bool> = p.faces().map(|f| flux[f] > 0.0).collect();
                phi.set_bc(&g, &p.name, Box::new(InOut(vals, outflow))).unwrap();
            }
            let ctl = SolveControl {
                tol_abs: 1e-14,
                tol_rel: 1e-12,
                max_iter: 2000,
            };
            for _ in 0..60 {
                let bc = phi.boundary_coeffs(&g);
                let grad = least_squares(&g, &phi.values, &phi.boundary_values(&g));
                let a = convection(&g, &flux, &phi.values, &bc, scheme, Some(&grad));
                a.solve(&mut phi.values, &ctl);
            }
            let cc = g.mesh.cell_centres();
            let e2: f64 = (0..g.n_cells())
                .map(|i| (phi.values[i] - exact(cc[i])).powi(2))
                .sum::<f64>()
                / g.n_cells() as f64;
            e2.sqrt()
        };
        for (name, scheme, min_order) in [
            ("upwind", ConvScheme::Upwind, 0.7),
            ("linearUpwind", ConvScheme::LinearUpwind, 1.3),
            ("QUICK", ConvScheme::Quick, 1.3),
        ] {
            let e: Vec<f64> = [16usize, 32, 64].iter().map(|&n| run(n, scheme)).collect();
            let orders: Vec<f64> = e.windows(2).map(|w| (w[0] / w[1]).log2()).collect();
            println!("2D {name}: errors {e:?} orders {orders:?}");
            assert!(*orders.last().unwrap() > min_order, "{name}: {orders:?}");
        }
    }
}
