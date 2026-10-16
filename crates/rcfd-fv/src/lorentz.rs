//! Lorentz-force coupling of a liquid conductor in an imposed magnetic field (F-214, ADR 0039 D1).
//!
//! Low magnetic Reynolds number approximation: the applied field `B` is given and not changed by the flow. Ohm's law
//! `J = sigma (E + u x B)` with `E = E_ext - grad phi` and `div J = 0`; the body force on the fluid is `J x B`, handed to
//! the momentum equation as the kinematic acceleration `J x B / rho` through the explicit force plus implicit sink path of
//! the momentum sources: for uniform `B`, `a = J x B / rho = (J_c x B / rho + s u*) - s u` with `s = sigma B^2 / rho`, so
//! the explicit part carries the full force plus `s u*` (the lagged iterate) and the implicit sink `s` damps the iteration
//! (stable for large Hartmann numbers); at convergence the two `s u` terms cancel exactly.

use crate::bc::{FixedGradient, FixedValue, Profile};
use crate::electric as el;
use crate::field::{ScalarField, VectorField};
use crate::geometry::FvGeometry;
use crate::simple::BodyForce;
use rcfd_core::vec3::{self, Vec3};
use rcfd_linalg::SolveControl;

/// Electric condition of one boundary patch for the potential solve.
#[derive(Debug, Clone, PartialEq)]
pub enum PatchElectric {
    /// Zero normal current (default).
    Insulated,
    /// Fixed potential (V).
    Potential(f64),
    /// Uniform current density INTO the domain (A/m^2).
    Current(f64),
}

/// How charge conservation is closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PotentialMode {
    /// `grad phi = 0`: `J = sigma (E_ext + u x B)` in every cell.
    None,
    /// Solve `-div(sigma grad phi) = -div(sigma (E_ext + u x B))`.
    Solve,
}

/// One Lorentz-force zone with uniform `B` and `sigma`.
#[derive(Debug, Clone)]
pub struct Lorentz {
    /// Applied magnetic flux density (T).
    pub b: Vec3,
    /// Electrical conductivity of the zone cells (S/m).
    pub sigma: f64,
    /// Imposed uniform electric field (V/m).
    pub e_ext: Vec3,
    /// Density (kg/m^3) converting force to acceleration.
    pub rho: f64,
    /// Charge-conservation closure.
    pub mode: PotentialMode,
    /// Electric condition per patch name (patches not listed are insulated).
    pub patches: Vec<(String, PatchElectric)>,
    /// Cells of the zone.
    pub zone: Vec<bool>,
}

/// Result of one evaluation.
#[derive(Debug, Clone)]
pub struct LorentzResult {
    /// Current density per cell (A/m^2).
    pub current: Vec<Vec3>,
    /// Force density `J x B` per cell (N/m^3).
    pub force: Vec<Vec3>,
    /// Explicit part of the kinematic acceleration `J x B / rho + s u*` (zone cells).
    pub accel: Vec<Vec3>,
    /// Implicit sink `s = sigma B^2 / rho` (1/s) in the zone cells, zero elsewhere.
    pub sink: Vec<f64>,
}

impl LorentzResult {
    /// The explicit acceleration as the solver's body-force object.
    pub fn body_force(&self, geom: &FvGeometry) -> BodyForce {
        BodyForce::from_cells(geom, self.accel.clone())
    }
}

impl Lorentz {
    fn sigma_cells(&self) -> Vec<f64> {
        self.zone.iter().map(|&z| if z { self.sigma } else { 0.0 }).collect()
    }

    /// Unit normal of the `empty` faces of a 2-D mesh, if any.
    fn empty_normal(geom: &FvGeometry) -> Option<Vec3> {
        let sf = geom.mesh.face_area_vectors();
        (0..geom.mesh.n_faces())
            .find(|&f| geom.empty[f])
            .map(|f| vec3::unit(sf[f]))
    }

    /// Evaluate current and force for the velocity `u`; `phi` is the persistent potential field (used and updated in
    /// `Solve` mode, ignored otherwise).
    pub fn evaluate(&self, geom: &FvGeometry, phi: &mut ScalarField, u: &VectorField) -> LorentzResult {
        let n = geom.n_cells();
        let sigma = self.sigma_cells();
        let b2 = vec3::dot(self.b, self.b);
        let mut current = vec![[0.0; 3]; n];
        match self.mode {
            PotentialMode::None => {
                for c in 0..n {
                    if self.zone[c] {
                        let d = vec3::add(self.e_ext, vec3::cross(u.values[c], self.b));
                        current[c] = vec3::scale(d, self.sigma);
                    }
                }
            }
            PotentialMode::Solve => self.solve_current(geom, phi, u, &sigma, &mut current),
        }
        let mut force = vec![[0.0; 3]; n];
        let mut accel = vec![[0.0; 3]; n];
        let mut sink = vec![0.0; n];
        let s = self.sigma * b2 / self.rho;
        for c in 0..n {
            if !self.zone[c] {
                continue;
            }
            let f = vec3::cross(current[c], self.b);
            force[c] = f;
            accel[c] = vec3::add(vec3::scale(f, 1.0 / self.rho), vec3::scale(u.values[c], s));
            sink[c] = s;
        }
        LorentzResult {
            current,
            force,
            accel,
            sink,
        }
    }

    fn solve_current(
        &self,
        geom: &FvGeometry,
        phi: &mut ScalarField,
        u: &VectorField,
        sigma: &[f64],
        current: &mut [Vec3],
    ) {
        let mesh = &geom.mesh;
        let (own, nei, sf, cf, cc, vol) = (
            mesh.owner(),
            mesh.neighbour(),
            mesh.face_area_vectors(),
            mesh.face_centres(),
            mesh.cell_centres(),
            mesh.cell_volumes(),
        );
        let ub = u.boundary_values(geom);
        let sig_f = el::face_conductivity(geom, sigma);
        // driving field D = E_ext + u x B at the faces
        let drive = |f: usize| -> Vec3 {
            let uf = if f < geom.n_internal {
                let w = geom.weights[f];
                vec3::add(vec3::scale(u.values[own[f]], w), vec3::scale(u.values[nei[f]], 1.0 - w))
            } else {
                ub[f - geom.n_internal]
            };
            vec3::add(self.e_ext, vec3::cross(uf, self.b))
        };
        let mut source = vec![0.0; geom.n_cells()];
        for f in 0..mesh.n_faces() {
            if geom.empty[f] {
                continue;
            }
            let flux = sig_f[f] * vec3::dot(drive(f), sf[f]);
            source[own[f]] -= flux / vol[own[f]];
            if f < geom.n_internal {
                source[nei[f]] += flux / vol[nei[f]];
            }
        }
        // boundary conditions of the potential
        let mut has_fixed = false;
        for (i, p) in mesh.patches().iter().enumerate() {
            if geom.periodic_partner.get(i).is_some_and(|x| x.is_some()) || (p.size > 0 && geom.empty[p.start]) {
                continue;
            }
            let kind = self
                .patches
                .iter()
                .find(|(n, _)| *n == p.name)
                .map_or(&PatchElectric::Insulated, |(_, k)| k);
            match kind {
                PatchElectric::Potential(v) => {
                    has_fixed = true;
                    let _ = phi.set_bc(geom, &p.name, Box::new(FixedValue::scalar(*v)));
                }
                PatchElectric::Insulated | PatchElectric::Current(_) => {
                    let j_in = if let PatchElectric::Current(j) = kind { *j } else { 0.0 };
                    let g: Vec<Vec3> = p
                        .faces()
                        .map(|f| {
                            if geom.empty[f] {
                                return [0.0; 3];
                            }
                            let nhat = vec3::unit(sf[f]);
                            let s = sigma[own[f]];
                            let gr = vec3::dot(drive(f), nhat) + if s > 0.0 { j_in / s } else { 0.0 };
                            [gr, 0.0, 0.0]
                        })
                        .collect();
                    let _ = phi.set_bc(geom, &p.name, Box::new(FixedGradient(Profile::PerFace(g))));
                }
            }
        }
        let ctl = SolveControl {
            tol_abs: 1e-300,
            tol_rel: 1e-13,
            max_iter: 5000,
        };
        let (sf_cond, _) = el::solve_with_source(geom, phi, sigma, Some(&source), !has_fixed, &ctl, 20);
        // face currents leaving the owner: -sigma A dphi/dn plus the driven part
        let flows = el::face_currents(geom, phi, &sf_cond, true);
        let mut jf = vec![0.0; mesh.n_faces()];
        for f in 0..mesh.n_faces() {
            if geom.empty[f] {
                continue;
            }
            jf[f] = flows[f] + sf_cond[f] * vec3::dot(drive(f), sf[f]);
        }
        // cell reconstruction from the face currents (exact for a uniform J), in the directions the mesh resolves
        let mut acc = vec![[0.0; 3]; geom.n_cells()];
        for f in 0..mesh.n_faces() {
            if geom.empty[f] {
                continue;
            }
            let o = own[f];
            vec3::axpy(&mut acc[o], jf[f] / vol[o], vec3::sub(cf[f], cc[o]));
            if f < geom.n_internal {
                let m = nei[f];
                vec3::axpy(&mut acc[m], -jf[f] / vol[m], vec3::sub(cf[f], cc[m]));
            }
        }
        let en = Self::empty_normal(geom);
        for c in 0..geom.n_cells() {
            if sigma[c] <= 0.0 {
                current[c] = [0.0; 3];
                continue;
            }
            let mut j = acc[c];
            if let Some(n) = en {
                let d = vec3::add(self.e_ext, vec3::cross(u.values[c], self.b));
                let jn = self.sigma * vec3::dot(d, n);
                // the reconstruction has no component along the empty direction
                let corr = jn - vec3::dot(j, n);
                vec3::axpy(&mut j, corr, n);
            }
            current[c] = j;
        }
    }
}

/// Hartmann-flow closed forms (ADR 0039): `ha = B0 a sqrt(sigma/mu)`; `y` in `[-a, a]`.
pub mod hartmann {
    /// Velocity of the short-circuited channel (`E_z = 0`) driven by the body force `g` (N/m^3):
    /// `u = g/(sigma B0^2) (1 - cosh(Ha y/a)/cosh Ha)`.
    pub fn u_short(g: f64, sigma: f64, b0: f64, ha: f64, a: f64, y: f64) -> f64 {
        g / (sigma * b0 * b0) * (1.0 - (ha * y / a).cosh() / ha.cosh())
    }

    /// Mean velocity of the short-circuited channel, `g/(sigma B0^2) (1 - tanh Ha / Ha)`.
    pub fn mean_short(g: f64, sigma: f64, b0: f64, ha: f64) -> f64 {
        g / (sigma * b0 * b0) * (1.0 - ha.tanh() / ha)
    }

    /// Velocity of the insulated-wall channel with mean velocity `mean`:
    /// `u/U = (1 - cosh(Ha y/a)/cosh Ha)/(1 - tanh Ha/Ha)`.
    pub fn u_insulated(mean: f64, ha: f64, a: f64, y: f64) -> f64 {
        mean * (1.0 - (ha * y / a).cosh() / ha.cosh()) / (1.0 - ha.tanh() / ha)
    }

    /// Driving body force of the insulated-wall channel for the mean velocity `mean`:
    /// `g = sigma B0^2 U (tanh Ha/Ha)/(1 - tanh Ha/Ha)`.
    pub fn g_insulated(mean: f64, sigma: f64, b0: f64, ha: f64) -> f64 {
        let t = ha.tanh() / ha;
        sigma * b0 * b0 * mean * t / (1.0 - t)
    }

    /// Ratio of the Hartmann to the Poiseuille mean velocity at the same `g`: `3 (Ha - tanh Ha)/Ha^3`.
    pub fn flow_ratio(ha: f64) -> f64 {
        3.0 * (ha - ha.tanh()) / (ha * ha * ha)
    }
}
