//! Eddy-viscosity plumbing shared by the turbulence models: assembling the
//! effective face viscosity of the momentum equation (with wall-function
//! values on wall faces) and the algebraic mixing-length model (Prandtl, 1925).

use rcfd_core::vec3;
use rcfd_fv::gradient::green_gauss;
use rcfd_fv::{FvGeometry, VectorField};

/// Faces of the named patches (global face indices).
pub fn patch_faces(geom: &FvGeometry, patches: &[&str]) -> Vec<usize> {
    patches
        .iter()
        .filter_map(|n| geom.mesh.patch(n))
        .flat_map(|p| p.faces())
        .collect()
}

/// Effective kinematic viscosity at every face: `nu + nu_t` with `nu_t` linearly interpolated to internal
/// faces, taken from the owner cell on boundary faces, and replaced by `wall_nut` on the given wall
/// faces (`(global face, nu_t,w)`, e.g. from the wall functions).
pub fn effective_viscosity(geom: &FvGeometry, nu: f64, nut: &[f64], wall_nut: &[(usize, f64)]) -> Vec<f64> {
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut out: Vec<f64> = (0..mesh.n_faces())
        .map(|f| {
            if f < geom.n_internal {
                let w = geom.weights[f];
                nu + w * nut[own[f]] + (1.0 - w) * nut[nei[f]]
            } else {
                nu + nut[own[f]]
            }
        })
        .collect();
    for &(f, v) in wall_nut {
        out[f] = nu + v;
    }
    out
}

/// Magnitude of the strain rate `sqrt(2 S_ij S_ij)` in every cell.
pub fn strain_rate(geom: &FvGeometry, u: &VectorField) -> Vec<f64> {
    let ub = u.boundary_values(geom);
    let n = geom.n_cells();
    let g: Vec<Vec<[f64; 3]>> = (0..3)
        .map(|k| {
            let cells: Vec<f64> = u.values.iter().map(|v| v[k]).collect();
            let bv: Vec<f64> = ub.iter().map(|v| v[k]).collect();
            green_gauss(geom, &cells, &bv, 0)
        })
        .collect();
    (0..n)
        .map(|c| {
            let mut s2 = 0.0;
            for i in 0..3 {
                for j in 0..3 {
                    let sij = 0.5 * (g[i][c][j] + g[j][c][i]);
                    s2 += sij * sij;
                }
            }
            (2.0 * s2).sqrt()
        })
        .collect()
}

/// Prandtl mixing-length model: `nu_t = l^2 |S|` with `l = min(kappa d, l_max)`.
#[derive(Debug, Clone)]
pub struct MixingLength {
    /// von Karman constant.
    pub kappa: f64,
    /// Upper limit of the mixing length (e.g. `0.09 * half height`); `f64::INFINITY` for none.
    pub l_max: f64,
    /// Wall distance of every cell centre.
    pub wall_distance: Vec<f64>,
}

impl MixingLength {
    /// Eddy viscosity of every cell for the velocity field `u`.
    pub fn nut(&self, geom: &FvGeometry, u: &VectorField) -> Vec<f64> {
        let s = strain_rate(geom, u);
        s.iter()
            .zip(&self.wall_distance)
            .map(|(&s, &d)| {
                let l = (self.kappa * d).min(self.l_max);
                l * l * s
            })
            .collect()
    }
}

impl MixingLength {
    /// Like [`nut`](Self::nut), but cells next to the given wall faces use the log-law value
    /// `nu_t = kappa y u_tau` (with `u_tau` from the wall function) instead of the strain of the
    /// cell, which would include the no-slip face value and so overestimate the strain in the log layer.
    pub fn nut_with_walls(
        &self,
        geom: &FvGeometry,
        u: &VectorField,
        wc: &crate::wall::WallConstants,
        nu: f64,
        wall_faces: &[usize],
    ) -> Vec<f64> {
        let mut nut = self.nut(geom, u);
        for &f in wall_faces {
            let c = geom.mesh.owner()[f];
            let y = geom.dn[f - geom.n_internal];
            let n = geom.normal(f);
            let v = u.values[c];
            let vn = vec3::dot(v, n);
            let tangential = vec3::norm(vec3::sub(v, vec3::scale(n, vn)));
            let ut = wc.u_tau_from_velocity(nu, tangential, y);
            if ut * y / nu >= wc.y_plus_lam() {
                nut[c] = self.kappa * y * ut;
            }
        }
        nut
    }
}

/// Scalar helper: vector magnitude of a cell velocity.
pub fn speed(v: [f64; 3]) -> f64 {
    vec3::norm(v)
}
