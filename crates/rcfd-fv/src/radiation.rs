//! Grey radiation in participating media for the steady energy equation (F-130 / F-052, ADR 0012): the Rosseland
//! diffusion approximation (an effective radiative conductivity) and the P1 model with Marshak boundary conditions.
//!
//! P1 (isotropic scattering): `div(Gamma grad G) - a G = -4 a sigma n^2 T^4`, `Gamma = 1 / (3 (a + sigma_s))`, radiative
//! flux `q = -Gamma grad G`, energy source `-div q = a (G - 4 sigma n^2 T^4)`. Marshak condition at a wall of emissivity
//! `eps`: `q.n = -(eps / (2 (2 - eps))) (4 sigma n^2 T_w^4 - G_w)` with `n` the outward normal.
//! Rosseland: `q = -(16 sigma n^2 T^3 / (3 beta)) grad T`, `beta = a + sigma_s`.

use crate::bc::{Bc, BcCtx, FaceCoeffs};
use crate::convection::convection;
use crate::diffusion::{laplacian, uniform_gamma};
use crate::energy::EnergySolver;
use crate::field::ScalarField;
use crate::fvmatrix::FvMatrix;
use crate::geometry::FvGeometry;
use crate::gradient::least_squares;
use crate::simple::normalised_residual;
use rcfd_core::vec3;
use rcfd_linalg::SolveControl;
use std::sync::Arc;

/// Stefan-Boltzmann constant (W/m^2/K^4, CODATA 2018).
pub const SIGMA: f64 = 5.670374419e-8;

/// Marshak coefficient `eps / (2 (2 - eps))`.
pub fn marshak_coefficient(emissivity: f64) -> f64 {
    emissivity / (2.0 * (2.0 - emissivity))
}

/// Robin condition of the P1 equation: `Gamma dG/dn = c (G_b - G_w)` with `G_b = 4 sigma n^2 T_w^4` (`c = 0` is a
/// zero normal gradient).
#[derive(Debug, Clone)]
pub struct MarshakWall {
    /// `Gamma` at the patch faces.
    pub gamma: f64,
    /// Marshak coefficient per patch face.
    pub c: Vec<f64>,
    /// Black-body value `4 sigma n^2 T_w^4` per patch face.
    pub g_b: Vec<f64>,
}

impl Bc for MarshakWall {
    fn type_name(&self) -> &'static str {
        "marshakWall"
    }
    fn coeffs(&self, ctx: &BcCtx, _: usize) -> FaceCoeffs {
        let i = ctx.local_face;
        let k = self.gamma / ctx.dn;
        let den = k + self.c[i];
        FaceCoeffs::from_value(k / den, self.c[i] * self.g_b[i] / den, ctx.dn)
    }
    fn clone_box(&self) -> Box<dyn Bc> {
        Box::new(self.clone())
    }
}

/// Radiative role of a boundary patch.
#[derive(Debug, Clone)]
pub struct WallRadiation {
    /// Patch name.
    pub patch: String,
    /// Emissivity in (0, 1]; `None` is a symmetry / adiabatic reflecting patch (zero normal gradient of G).
    pub emissivity: Option<f64>,
}

/// P1 incident-radiation solver.
pub struct P1Solver {
    /// Geometry.
    pub geom: Arc<FvGeometry>,
    /// Incident radiation `G` (W/m^2).
    pub g: ScalarField,
    /// Absorption coefficient (1/m).
    pub absorption: f64,
    /// Scattering coefficient (1/m).
    pub scattering: f64,
    /// Refractive index.
    pub index: f64,
    /// Patch conditions.
    pub walls: Vec<WallRadiation>,
    /// Linear solver controls (tight: the equation is linear for a given temperature).
    pub solver: SolveControl,
}

impl P1Solver {
    /// New solver; `G` starts at the black-body value of `t`.
    pub fn new(
        geom: Arc<FvGeometry>,
        t: &[f64],
        absorption: f64,
        scattering: f64,
        index: f64,
        walls: Vec<WallRadiation>,
    ) -> Self {
        let mut g = ScalarField::uniform("G", &geom, 0.0);
        for (gv, tv) in g.values.iter_mut().zip(t) {
            *gv = 4.0 * SIGMA * index * index * tv.powi(4);
        }
        P1Solver {
            geom,
            g,
            absorption,
            scattering,
            index,
            walls,
            solver: SolveControl {
                tol_abs: 1e-300,
                tol_rel: 1e-12,
                max_iter: 5000,
            },
        }
    }

    /// `Gamma = 1 / (3 (a + sigma_s))`.
    pub fn gamma(&self) -> f64 {
        1.0 / (3.0 * (self.absorption + self.scattering))
    }

    fn black_body(&self, t: f64) -> f64 {
        4.0 * SIGMA * self.index * self.index * t.powi(4)
    }

    fn install_walls(&mut self, t_boundary: &[f64]) {
        let geom = self.geom.clone();
        let gamma = self.gamma();
        for w in &self.walls {
            let Some(p) = geom.mesh.patch(&w.patch) else { continue };
            let c = w.emissivity.map_or(0.0, marshak_coefficient);
            let faces: Vec<usize> = p.faces().collect();
            let bc = MarshakWall {
                gamma,
                c: vec![c; faces.len()],
                g_b: faces
                    .iter()
                    .map(|&f| self.black_body(t_boundary[f - geom.n_internal]))
                    .collect(),
            };
            let _ = self.g.set_bc(&geom, &w.patch, Box::new(bc));
        }
    }

    /// Solve for `G` with the temperature `t` (cells) and `t_boundary` (boundary-face values, index
    /// `f - n_internal`); returns the normalised residual before the solve.
    pub fn solve(&mut self, t: &[f64], t_boundary: &[f64]) -> f64 {
        self.install_walls(t_boundary);
        let geom = self.geom.clone();
        let bc = self.g.boundary_coeffs(&geom);
        let gamma = uniform_gamma(&geom, self.gamma());
        let mut m: FvMatrix = laplacian(&geom, &gamma, &bc, None);
        let vol = geom.mesh.cell_volumes();
        for c in 0..t.len() {
            m.a.diag[c] += self.absorption * vol[c];
            m.b[c] += self.absorption * self.black_body(t[c]) * vol[c];
        }
        let cells = self.g.values.clone();
        let res = normalised_residual(&m, &cells);
        let mut x = cells;
        m.solve(&mut x, &self.solver);
        self.g.values = x;
        res
    }

    /// Linearised energy source for the temperature `t_star`: returns `(implicit, explicit)` per cell, in K/s per unit
    /// `rho cp` already divided: `S = explicit - implicit * T` with `implicit = 16 a sigma n^2 T*^3 / (rho cp)` and
    /// `explicit = a (G + 12 sigma n^2 T*^4) / (rho cp)`.
    pub fn energy_terms(&self, t_star: &[f64], rho_cp: f64) -> (Vec<f64>, Vec<f64>) {
        let (a, n2) = (self.absorption, self.index * self.index);
        let imp = t_star
            .iter()
            .map(|t| 16.0 * a * SIGMA * n2 * t.powi(3) / rho_cp)
            .collect();
        let exp = t_star
            .iter()
            .zip(&self.g.values)
            .map(|(t, g)| a * (g + 12.0 * SIGMA * n2 * t.powi(4)) / rho_cp)
            .collect();
        (imp, exp)
    }

    /// Net radiative heat flow (W) out of the domain through `patch`: `sum c (G_w - G_b) |S|`.
    pub fn wall_flow_out(&self, patch: &str, t_boundary: &[f64]) -> f64 {
        let g = &self.geom;
        let Some(p) = g.mesh.patch(patch) else { return 0.0 };
        let Some(w) = self.walls.iter().find(|w| w.patch == patch) else {
            return 0.0;
        };
        let c = w.emissivity.map_or(0.0, marshak_coefficient);
        let coeffs = self.g.boundary_coeffs(g);
        let own = g.mesh.owner();
        let sf = g.mesh.face_area_vectors();
        p.faces()
            .filter(|&f| !g.empty[f])
            .map(|f| {
                let gw = coeffs[f - g.n_internal].value(self.g.values[own[f]]);
                c * (gw - self.black_body(t_boundary[f - g.n_internal])) * vec3::norm(sf[f])
            })
            .sum()
    }

    /// Volume integral of `a (G - 4 sigma n^2 T^4)` (W): the net radiative absorption.
    pub fn absorbed_power(&self, t: &[f64]) -> f64 {
        let vol = self.geom.mesh.cell_volumes();
        (0..t.len())
            .map(|c| self.absorption * (self.g.values[c] - self.black_body(t[c])) * vol[c])
            .sum()
    }
}

/// Rosseland radiative diffusivity `16 sigma n^2 T^3 / (3 beta rho cp)` (m^2/s) at every face; the face temperature is
/// the linear interpolation of the cells (internal) or `t_boundary` (index `f - n_internal`).
pub fn rosseland_alpha_faces(
    geom: &FvGeometry,
    t: &[f64],
    t_boundary: &[f64],
    beta: f64,
    index: f64,
    rho_cp: f64,
) -> Vec<f64> {
    let mesh = &geom.mesh;
    (0..mesh.n_faces())
        .map(|f| {
            let tf = if f < geom.n_internal {
                let w = geom.weights[f];
                w * t[mesh.owner()[f]] + (1.0 - w) * t[mesh.neighbour()[f]]
            } else {
                t_boundary[f - geom.n_internal]
            };
            16.0 * SIGMA * index * index * tf.powi(3) / (3.0 * beta * rho_cp)
        })
        .collect()
}

/// One implicit sweep of the steady energy equation with an extra linearised sink: the equation solved is
/// `div(phi T) - div(alpha grad T) + implicit T = explicit (+ e.source)`. Used for the P1 coupling; returns the
/// normalised residual before the solve.
pub fn iterate_with_implicit(e: &mut EnergySolver, phi: &[f64], implicit: &[f64], explicit: &[f64]) -> f64 {
    let g = e.geom.clone();
    let cells = e.t.values.clone();
    let bc = e.t.boundary_coeffs(&g);
    let bvals = e.t.boundary_values(&g);
    let gamma = match &e.alpha_face {
        Some(a) => a.clone(),
        None => uniform_gamma(&g, e.alpha),
    };
    let grad = e.convection.needs_gradient().then(|| least_squares(&g, &cells, &bvals));
    let mut m = convection(&g, phi, &cells, &bc, e.convection, grad.as_deref());
    m.add(&laplacian(&g, &gamma, &bc, grad.as_deref()));
    let vol = g.mesh.cell_volumes();
    for c in 0..cells.len() {
        m.a.diag[c] += implicit[c] * vol[c];
        m.b[c] += explicit[c] * vol[c];
        if let Some(s) = &e.source {
            m.b[c] += s[c] * vol[c];
        }
    }
    let res = normalised_residual(&m, &cells);
    m.relax(e.relax, &cells);
    let mut x = cells;
    m.solve(&mut x, &e.solver);
    e.t.values = x;
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marshak_coefficient_limits() {
        assert!((marshak_coefficient(1.0) - 0.5).abs() < 1e-15);
        assert!((marshak_coefficient(0.5) - 1.0 / 6.0).abs() < 1e-15);
    }

    #[test]
    fn marshak_wall_at_black_body_value_has_no_flux() {
        // G_b equal to the cell value: face value equals the cell value, zero gradient
        let bc = MarshakWall {
            gamma: 0.1,
            c: vec![0.5],
            g_b: vec![300.0],
        };
        let ctx = BcCtx {
            normal: [1.0, 0.0, 0.0],
            dn: 0.01,
            local_face: 0,
            cell_value: [300.0, 0.0, 0.0],
            partner: None,
            is_vector: false,
        };
        let co = bc.coeffs(&ctx, 0);
        assert!((co.value(300.0) - 300.0).abs() < 1e-12);
        assert!(co.gradient(300.0).abs() < 1e-9);
        // radiative flux q.n = -Gamma dG/dn = -c (G_b - G_w): for G_P = 0 the wall value is below G_b
        let gw = co.value(0.0);
        assert!(gw > 0.0 && gw < 300.0);
        let q_n = -0.1 * co.gradient(0.0);
        assert!((q_n + 0.5 * (300.0 - gw)).abs() < 1e-9);
    }
}
