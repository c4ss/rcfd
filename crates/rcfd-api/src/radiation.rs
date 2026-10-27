//! Grey radiation (F-130, ADR 0012): `radiation.model` of `rosseland` or `p1`, coupled to the steady energy equation.
//! The physics is in `rcfd_fv::radiation`.

use crate::case::{Case, Diagnostic};
use crate::sources::{gf, gs};
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::energy::EnergySolver;
use rcfd_fv::radiation::{self as rad, P1Solver, WallRadiation};
use rcfd_fv::FvGeometry;
use std::sync::Arc;

/// Is a radiation model selected?
pub fn enabled(case: &Case) -> bool {
    matches!(gs(case, "radiation.model").as_str(), "rosseland" | "p1")
}

/// Settings checks.
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if !enabled(case) {
        return out;
    }
    let mut bad = |path: &str, message: String| {
        out.push(Diagnostic {
            path: path.into(),
            line: None,
            message,
        })
    };
    let (a, s, n) = (
        gf(case, "radiation.absorption_coefficient"),
        gf(case, "radiation.scattering_coefficient"),
        gf(case, "radiation.refractive_index"),
    );
    if a.is_nan() || a < 0.0 {
        bad("radiation.absorption_coefficient", "must not be negative [1/m]".into());
    }
    if s.is_nan() || s < 0.0 {
        bad("radiation.scattering_coefficient", "must not be negative [1/m]".into());
    }
    if a >= 0.0 && s >= 0.0 && a + s <= 0.0 {
        bad(
            "radiation.absorption_coefficient",
            "the extinction coefficient (absorption plus scattering) must be positive".into(),
        );
    }
    if n.is_nan() || n <= 0.0 {
        bad("radiation.refractive_index", "must be positive".into());
    }
    if !matches!(gs(case, "solver.algorithm").as_str(), "simple" | "simplec") {
        bad(
            "radiation.model",
            format!(
                "radiation is available with the steady algorithms (simple, simplec) only, not with \"{}\"",
                gs(case, "solver.algorithm")
            ),
        );
    }
    if !case
        .tree
        .get("energy.enabled")
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        bad(
            "radiation.model",
            "radiation is coupled through the energy equation (energy.enabled = true)".into(),
        );
    }
    for b in case.tree.names("boundaries").unwrap_or_default() {
        let e = gf(case, &format!("boundaries.{b}.emissivity"));
        if !(e > 0.0 && e <= 1.0) {
            bad(&format!("boundaries.{b}.emissivity"), "must be in (0, 1]".into());
        }
    }
    out
}

enum Model {
    Rosseland,
    P1(Box<P1Solver>),
}

/// Radiation state of a simulation.
pub struct Radiation {
    model: Model,
    beta: f64,
    index: f64,
    rho_cp: f64,
}

impl Radiation {
    /// Build from the case; `None` when no model is selected.
    pub fn from_case(case: &Case, geom: &Arc<FvGeometry>, t: &[f64]) -> Option<Radiation> {
        if !enabled(case) {
            return None;
        }
        let (a, s, n) = (
            gf(case, "radiation.absorption_coefficient"),
            gf(case, "radiation.scattering_coefficient"),
            gf(case, "radiation.refractive_index"),
        );
        let rho_cp = gf(case, "physics.rho") * gf(case, "physics.cp");
        let model = if gs(case, "radiation.model") == "p1" {
            let walls: Vec<WallRadiation> = geom
                .mesh
                .patches()
                .iter()
                .enumerate()
                .filter(|(i, p)| {
                    p.kind != rcfd_mesh::PatchKind::Empty && geom.periodic_partner.get(*i).is_none_or(|x| x.is_none())
                })
                .map(|(_, p)| WallRadiation {
                    patch: p.name.clone(),
                    emissivity: (gs(case, &format!("boundaries.{}.velocity", p.name)) != "symmetry")
                        .then(|| gf(case, &format!("boundaries.{}.emissivity", p.name))),
                })
                .collect();
            Model::P1(Box::new(P1Solver::new(geom.clone(), t, a, s, n, walls)))
        } else {
            Model::Rosseland
        };
        Some(Radiation {
            model,
            beta: a + s,
            index: n,
            rho_cp,
        })
    }

    /// One coupled energy iteration with radiation; `base_alpha` is the non-radiative diffusivity at the faces
    /// (`None` uses the solver's uniform one). Returns the normalised temperature residual.
    pub fn iterate_energy(&mut self, en: &mut EnergySolver, phi: &[f64], base_alpha: Option<Vec<f64>>) -> f64 {
        let geom = en.geom.clone();
        let base = base_alpha.unwrap_or_else(|| uniform_gamma(&geom, en.alpha));
        let tb = en.t.boundary_values(&geom);
        match &mut self.model {
            Model::Rosseland => {
                let kr = rad::rosseland_alpha_faces(&geom, &en.t.values, &tb, self.beta, self.index, self.rho_cp);
                en.alpha_face = Some(base.iter().zip(&kr).map(|(a, b)| a + b).collect());
                en.iterate(phi)
            }
            Model::P1(p1) => {
                en.alpha_face = Some(base);
                p1.solve(&en.t.values, &tb);
                let (imp, exp) = p1.energy_terms(&en.t.values, self.rho_cp);
                rad::iterate_with_implicit(en, phi, &imp, &exp)
            }
        }
    }

    /// Incident radiation `G` (P1 only).
    pub fn incident(&self) -> Option<&[f64]> {
        match &self.model {
            Model::P1(p) => Some(&p.g.values),
            Model::Rosseland => None,
        }
    }

    /// Net radiative heat flow (W) out of the domain through a patch: the Marshak flux of P1, the Rosseland flux
    /// `-sum k_r(T_b) dT/dn |S|` of the diffusion approximation.
    pub fn wall_flow_out(&self, patch: &str, en: &EnergySolver) -> f64 {
        let geom = &en.geom;
        let tb = en.t.boundary_values(geom);
        match &self.model {
            Model::P1(p) => p.wall_flow_out(patch, &tb),
            Model::Rosseland => {
                let Some(pt) = geom.mesh.patch(patch) else { return 0.0 };
                let grad = en.wall_gradient(patch);
                let alpha = rad::rosseland_alpha_faces(geom, &en.t.values, &tb, self.beta, self.index, self.rho_cp);
                let sf = geom.mesh.face_area_vectors();
                -pt.faces()
                    .zip(&grad)
                    .map(|(f, g)| alpha[f] * self.rho_cp * g * rcfd_core::vec3::norm(sf[f]))
                    .sum::<f64>()
            }
        }
    }

    /// Net radiative absorption `integral a (G - 4 sigma n^2 T^4) dV` (W) (P1 only).
    pub fn absorbed_power(&self, t: &[f64]) -> Option<f64> {
        match &self.model {
            Model::P1(p) => Some(p.absorbed_power(t)),
            Model::Rosseland => None,
        }
    }
}
