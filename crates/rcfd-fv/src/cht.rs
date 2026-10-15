//! Conjugate heat transfer helpers (F-051, ADR 0024): solid cell zones with their own conductivity and heat capacity,
//! coupled fluid-solid and solid-solid interfaces with optional contact resistance, and discrete heat balances.
//!
//! The temperature equation is kept in the conductive form normalised by one reference capacity (the fluid
//! `rho cp`): `w_c dT/dt + div(phi T) = div(gamma grad T) + S/(rho cp)_ref` with `gamma_f = k_f/(rho cp)_ref` and the
//! capacity weight `w_c = (rho cp)_c/(rho cp)_ref`, so heat-flux continuity across a capacity jump is exact. The
//! existing [`EnergySolver`] solves it once it carries the per-face `gamma` (its `alpha_face`) and the weight (its
//! `ddt_weight`); [`ChtMaterials::configure`] sets both.

use crate::energy::EnergySolver;
use crate::field::ScalarField;
use crate::geometry::FvGeometry;
use crate::gradient::least_squares;
use rcfd_core::vec3::{self, Vec3};

/// Contact (or fouling-layer) resistance on one internal face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    /// Internal face index.
    pub face: usize,
    /// Resistance per unit area (m^2 K/W).
    pub resistance: f64,
}

/// Material data of a mesh with solid zones.
#[derive(Debug, Clone)]
pub struct ChtMaterials {
    /// Reference capacity `rho cp` (J/m^3/K) the equation is normalised by (the fluid value).
    pub rho_cp_ref: f64,
    /// Conductivity of every cell (W/m/K).
    pub k_cell: Vec<f64>,
    /// Volumetric heat capacity of every cell (J/m^3/K).
    pub rho_cp_cell: Vec<f64>,
    /// Contact resistances on internal faces.
    pub contacts: Vec<Contact>,
}

impl ChtMaterials {
    /// Conductivity at every face (W/m/K): the distance-weighted harmonic mean
    /// `1/k_f = (1 - w)/k_o + w/k_n` on internal faces (`w` the owner interpolation weight), the owner value on boundary
    /// faces; contact resistances are added in series, `1/k_eff = 1/k_f + R''/Delta` with `Delta` the normal distance
    /// between the cell centres.
    pub fn face_conductivity(&self, geom: &FvGeometry) -> Vec<f64> {
        let own = geom.mesh.owner();
        let nei = geom.mesh.neighbour();
        let mut k = vec![0.0; geom.mesh.n_faces()];
        for (f, kf) in k.iter_mut().enumerate() {
            *kf = if f < geom.n_internal {
                let w = geom.weights[f];
                1.0 / ((1.0 - w) / self.k_cell[own[f]] + w / self.k_cell[nei[f]])
            } else {
                self.k_cell[own[f]]
            };
        }
        let sf = geom.mesh.face_area_vectors();
        for c in &self.contacts {
            let f = c.face;
            let delta = vec3::dot(sf[f], geom.delta[f]) / vec3::norm(sf[f]);
            k[f] = 1.0 / (1.0 / k[f] + c.resistance / delta);
        }
        k
    }

    /// Diffusivity-like coefficient `gamma_f = k_f/(rho cp)_ref` of the normalised equation.
    pub fn gamma(&self, geom: &FvGeometry) -> Vec<f64> {
        self.face_conductivity(geom)
            .into_iter()
            .map(|k| k / self.rho_cp_ref)
            .collect()
    }

    /// Capacity weights `(rho cp)_c/(rho cp)_ref`.
    pub fn capacity_weights(&self) -> Vec<f64> {
        self.rho_cp_cell.iter().map(|c| c / self.rho_cp_ref).collect()
    }

    /// Hand the per-face coefficient and the capacity weight to the energy solver.
    pub fn configure(&self, geom: &FvGeometry, solver: &mut EnergySolver) {
        solver.alpha_face = Some(self.gamma(geom));
        solver.ddt_weight = Some(self.capacity_weights());
    }

    /// Volumetric source `q'''` (W/m^3 per cell) converted to the K/s of the normalised equation.
    pub fn source_in_kelvin_per_second(&self, q: &[f64]) -> Vec<f64> {
        q.iter().map(|x| x / self.rho_cp_ref).collect()
    }
}

/// Internal faces between cells whose material labels are `{a, b}` (either order), with resistance `r`.
pub fn contact_faces(geom: &FvGeometry, material: &[usize], a: usize, b: usize, r: f64) -> Vec<Contact> {
    let own = geom.mesh.owner();
    let nei = geom.mesh.neighbour();
    (0..geom.n_internal)
        .filter(|&f| {
            let (m, n) = (material[own[f]], material[nei[f]]);
            (m == a && n == b) || (m == b && n == a)
        })
        .map(|face| Contact { face, resistance: r })
        .collect()
}

/// Discrete conductive heat flow through every face, in W, positive out of the owner cell, for the face coefficient `k`
/// (W/m/K, [`ChtMaterials::face_conductivity`]). Uses the same operator as the solver: the over-relaxed orthogonal part
/// plus, when `non_orthogonal`, the explicit correction with the least-squares cell gradient (a scheme that needs a
/// gradient, as `ConvScheme::LinearUpwind`, assembles it; `Upwind` does not). Boundary faces: `-k A dT/dn` with the
/// boundary condition's gradient, plus the Dirichlet skewness term of the operator when the face is fixed-value.
pub fn conductive_face_flows(geom: &FvGeometry, t: &ScalarField, k: &[f64], non_orthogonal: bool) -> Vec<f64> {
    let own = geom.mesh.owner();
    let nei = geom.mesh.neighbour();
    let sf = geom.mesh.face_area_vectors();
    let bc = t.boundary_coeffs(geom);
    let bvals = t.boundary_values(geom);
    let grad = non_orthogonal.then(|| least_squares(geom, &t.values, &bvals));
    let mut out = vec![0.0; geom.mesh.n_faces()];
    for f in 0..geom.n_internal {
        let (o, n) = (own[f], nei[f]);
        let mut flow = k[f] * geom.lap_coeff[f] * (t.values[o] - t.values[n]);
        if let Some(gr) = &grad {
            let d = geom.delta[f];
            let delta = vec3::scale(d, vec3::dot(sf[f], sf[f]) / vec3::dot(sf[f], d));
            let kv = vec3::sub(sf[f], delta);
            let w = geom.weights[f];
            let gf: Vec3 = [0, 1, 2].map(|i| w * gr[o][i] + (1.0 - w) * gr[n][i]);
            flow -= k[f] * vec3::dot(kv, gf);
        }
        out[f] = flow;
    }
    for f in geom.n_internal..geom.mesh.n_faces() {
        if geom.empty[f] {
            continue;
        }
        let a = vec3::norm(sf[f]);
        let co = &bc[f - geom.n_internal];
        let mut flow = -k[f] * a * co.gradient(t.values[own[f]]);
        if let (Some(gr), true) = (&grad, co.a == 0.0) {
            let d = geom.delta[f];
            let kv = vec3::sub(sf[f], vec3::scale(d, a * a / vec3::dot(sf[f], d)));
            flow -= k[f] * vec3::dot(kv, gr[own[f]]);
        }
        out[f] = flow;
    }
    out
}

/// Net conductive heat leaving each cell group (W): `group[c]` labels the cell, the result has `n_groups` entries. Flows
/// between cells of the same group cancel; only the group boundary (to other groups and the mesh boundary) counts.
pub fn group_net_outflow(geom: &FvGeometry, flows: &[f64], group: &[usize], n_groups: usize) -> Vec<f64> {
    let own = geom.mesh.owner();
    let nei = geom.mesh.neighbour();
    let mut out = vec![0.0; n_groups];
    for f in 0..geom.n_internal {
        let (go, gn) = (group[own[f]], group[nei[f]]);
        if go != gn {
            out[go] += flows[f];
            out[gn] -= flows[f];
        }
    }
    for f in geom.n_internal..geom.mesh.n_faces() {
        if !geom.empty[f] {
            out[group[own[f]]] += flows[f];
        }
    }
    out
}

/// Conductive heat flow out of the domain through the faces of `patch` (W), with the per-face conductivity `k`.
pub fn patch_heat_flow_out(geom: &FvGeometry, flows: &[f64], patch: &str) -> Option<f64> {
    let p = geom.mesh.patch(patch)?;
    Some(p.faces().filter(|&f| !geom.empty[f]).map(|f| flows[f]).sum())
}

/// Volume-integrated heat content `sum rho cp_c V_c T_c` (J).
pub fn enthalpy(geom: &FvGeometry, rho_cp_cell: &[f64], t: &[f64]) -> f64 {
    let vol = geom.mesh.cell_volumes();
    (0..t.len()).map(|c| rho_cp_cell[c] * vol[c] * t[c]).sum()
}
