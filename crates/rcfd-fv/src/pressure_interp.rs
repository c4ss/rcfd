//! Pressure-interpolation variants of the steady SIMPLE kernel (F-246, ADR 0193): how the kernel turns cell pressures into the
//! face pressures of its Green-Gauss cell gradients and, for the body-force-weighted variant, into the least-squares gradients of
//! its non-orthogonal corrections and the body-force part of the face flux.
//!
//! Notation of ADR 0193: `P` the owner, `N` the neighbour, `d = x_N - x_P`, `S` the face area vector, `w` the owner weight,
//! `f_X` the body acceleration at the centre of cell `X`.

use crate::geometry::FvGeometry;
use crate::pressure::PressureBc;
use crate::simple::BodyForce;
use rcfd_core::vec3::{self, Vec3};

/// The variant of [`crate::simple::SimpleSolver::pressure_interpolation`] (ADR 0193).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PressureInterpolation {
    /// V0 (default): Green-Gauss face values `w p_P + (1 - w) p_N`, the pressure-condition values on boundary faces.
    #[default]
    Linear,
    /// V1: the Green-Gauss face value moved from the centre-line point `x_f'` to the face centre with the interpolated
    /// gradient of a first pass, `p_f + (w g_P + (1 - w) g_N) . (x_f - x_f')` (`gradient::green_gauss(.., 1)`). Identical to
    /// `Linear` where `x_f' = x_f`.
    SkewCorrected,
    /// V2: the default discretisation applied to the pressure relative to a local potential of the body force (trapezoid
    /// line integrals of the cell accelerations). Without a body force it is `Linear` bit for bit.
    BodyForceWeighted,
}

/// The body-force data of the variant `BodyForceWeighted` for one iteration (ADR 0193 V2-a to V2-d).
#[derive(Debug, Clone)]
pub struct BodyForceWeighting {
    /// `f_P` at every cell centre.
    cell: Vec<Vec3>,
    /// Internal faces: `w f_P + (1 - w) f_N`. Boundary faces: `f_b = f_P + (g_b - f_P . n) n` with `g_b` the stored normal
    /// component `a_b . S / |S|`.
    face: Vec<Vec3>,
    /// Per boundary face (`f - n_internal`): its pressure is extrapolated with the body force (V2-b): the normal velocity is
    /// prescribed and the face is not periodic.
    extrapolate: Vec<bool>,
}

impl BodyForceWeighting {
    /// The data for the body force `body` and the pressure conditions `pbc` of the iteration.
    pub fn new(geom: &FvGeometry, body: &BodyForce, pbc: &PressureBc) -> Self {
        let mesh = &geom.mesh;
        let (own, nei, sf) = (mesh.owner(), mesh.neighbour(), mesh.face_area_vectors());
        let n_int = geom.n_internal;
        let cell = body.cell.clone();
        let mut face = vec![[0.0; 3]; mesh.n_faces()];
        for (f, fv) in face.iter_mut().enumerate().take(n_int) {
            let w = geom.weights[f];
            *fv = vec3::add(vec3::scale(cell[own[f]], w), vec3::scale(cell[nei[f]], 1.0 - w));
        }
        for (f, fv) in face.iter_mut().enumerate().skip(n_int) {
            if geom.empty[f] {
                continue;
            }
            let area = vec3::norm(sf[f]).max(1e-300);
            let n = vec3::scale(sf[f], 1.0 / area);
            let fp = cell[own[f]];
            let gb = body.face[f] / area;
            *fv = vec3::add(fp, vec3::scale(n, gb - vec3::dot(fp, n)));
        }
        let extrapolate = (0..pbc.coeffs.len())
            .map(|i| pbc.fixed_u[i] && pbc.coupled[i].is_none() && !geom.empty[n_int + i])
            .collect();
        BodyForceWeighting {
            cell,
            face,
            extrapolate,
        }
    }

    /// V2-b: the boundary values `bvals` (index `f - n_internal`) with `p_P + 1/2 (f_P + f_b) . (x_b - x_P)` on the faces whose
    /// pressure is extrapolated.
    pub fn boundary_values(&self, geom: &FvGeometry, p: &[f64], bvals: &[f64]) -> Vec<f64> {
        let mesh = &geom.mesh;
        let (own, cc, cf) = (mesh.owner(), mesh.cell_centres(), mesh.face_centres());
        let mut out = bvals.to_vec();
        for (i, v) in out.iter_mut().enumerate() {
            if self.extrapolate[i] {
                let f = geom.n_internal + i;
                let c = own[f];
                *v = p[c] + 0.5 * vec3::dot(vec3::add(self.cell[c], self.face[f]), vec3::sub(cf[f], cc[c]));
            }
        }
        out
    }

    /// V2-a: the Green-Gauss cell gradient with the internal face values
    /// `w (p_P + 1/2 (f_P + f_f) . (x_f - x_P)) + (1 - w) (p_N + 1/2 (f_N + f_f) . (x_f - x_N))` and the boundary values `bvals`
    /// (already passed through [`boundary_values`](Self::boundary_values)).
    pub fn green_gauss(&self, geom: &FvGeometry, p: &[f64], bvals: &[f64]) -> Vec<Vec3> {
        let mesh = &geom.mesh;
        let (own, nei, sf) = (mesh.owner(), mesh.neighbour(), mesh.face_area_vectors());
        let (cc, cf) = (mesh.cell_centres(), mesh.face_centres());
        let mut g = vec![[0.0; 3]; mesh.n_cells()];
        for f in 0..geom.n_internal {
            let (o, nb) = (own[f], nei[f]);
            let w = geom.weights[f];
            let po = p[o] + 0.5 * vec3::dot(vec3::add(self.cell[o], self.face[f]), vec3::sub(cf[f], cc[o]));
            let pn = p[nb] + 0.5 * vec3::dot(vec3::add(self.cell[nb], self.face[f]), vec3::sub(cf[f], cc[nb]));
            let pf = w * po + (1.0 - w) * pn;
            vec3::axpy(&mut g[o], pf, sf[f]);
            vec3::axpy(&mut g[nb], -pf, sf[f]);
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

    /// V2-c: `f_P + LS_P(e)`, the weighted least-squares fit of the reduced increments `e = (p_N - p_P) - 1/2 (f_P + f_N) . d` and
    /// `e = (p_b - p_P) - 1/2 (f_P + f_b) . (x_b - x_P)` (`p_b` from `bvals`, already passed through
    /// [`boundary_values`](Self::boundary_values)), in the solved directions.
    pub fn least_squares(&self, geom: &FvGeometry, p: &[f64], bvals: &[f64]) -> Vec<Vec3> {
        let mesh = &geom.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let n_int = geom.n_internal;
        let internal: Vec<f64> = (0..n_int)
            .map(|f| {
                let (o, nb) = (own[f], nei[f]);
                (p[nb] - p[o]) - 0.5 * vec3::dot(vec3::add(self.cell[o], self.cell[nb]), geom.delta[f])
            })
            .collect();
        let boundary: Vec<f64> = (n_int..mesh.n_faces())
            .map(|f| {
                let o = own[f];
                (bvals[f - n_int] - p[o]) - 0.5 * vec3::dot(vec3::add(self.cell[o], self.face[f]), geom.delta[f])
            })
            .collect();
        let mut g = crate::gradient::least_squares_increments(geom, &internal, &boundary);
        for (gc, fc) in g.iter_mut().zip(&self.cell) {
            for k in 0..3 {
                if geom.active[k] {
                    gc[k] += fc[k];
                }
            }
        }
        g
    }

    /// V2-d: the body-force part of the flux of internal face `f` without the factor `rAU_f`,
    /// `lap_f 1/2 (f_P + f_N) . d + k_f . f_f` (the `k_f` term only with `non_orth`).
    pub fn face_flux(&self, geom: &FvGeometry, f: usize, non_orth: bool) -> f64 {
        let mesh = &geom.mesh;
        let (o, nb) = (mesh.owner()[f], mesh.neighbour()[f]);
        let d = geom.delta[f];
        let mut a = geom.lap_coeff[f] * 0.5 * vec3::dot(vec3::add(self.cell[o], self.cell[nb]), d);
        if non_orth {
            let s = mesh.face_area_vectors()[f];
            let k = vec3::sub(s, vec3::scale(d, vec3::dot(s, s) / vec3::dot(s, d)));
            a += vec3::dot(k, self.face[f]);
        }
        a
    }
}
