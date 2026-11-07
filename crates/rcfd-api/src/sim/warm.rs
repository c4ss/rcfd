//! The extended warm start of the design loop (ADR 0209 D4): the state a converged steady SIMPLE/SIMPLEC run hands to the next
//! run of a sweep, with the temperature of the energy equation and the fields of the k-epsilon and k-omega SST models, and its
//! seeding into a freshly built simulation. [`Simulation::seed_fields`] (velocity and pressure only) is unchanged.

use super::{Engine, Simulation};
use rcfd_core::vec3::Vec3;

/// Turbulence models whose state a [`WarmState`] carries.
const WARM_MODELS: [&str; 2] = ["k_epsilon", "k_omega_sst"];

/// The turbulence part of a [`WarmState`].
#[derive(Debug, Clone, PartialEq)]
pub struct TurbWarm {
    /// Model key (`k_epsilon` or `k_omega_sst`).
    pub model: String,
    /// Stored eddy viscosity per cell.
    pub nut: Vec<f64>,
    /// The model's transported fields (`k` and `epsilon` or `omega`) per cell.
    pub fields: Vec<(String, Vec<f64>)>,
}

/// The fields a converged steady run hands to the next run of a sweep (ADR 0209 D4).
#[derive(Debug, Clone, PartialEq)]
pub struct WarmState {
    /// Cell velocity.
    pub u: Vec<Vec3>,
    /// Cell (kinematic) pressure.
    pub p: Vec<f64>,
    /// Cell temperature, when the energy equation is solved.
    pub t: Option<Vec<f64>>,
    /// The k-epsilon or k-omega SST state, when the case uses one of them.
    pub turbulence: Option<TurbWarm>,
}

impl Simulation {
    /// The warm state of a steady SIMPLE/SIMPLEC run (`None` for other engines): velocity, pressure, the temperature when the
    /// energy equation is solved, and the k-epsilon or k-omega SST fields with the eddy viscosity (none for other models).
    pub fn warm_state(&self) -> Option<WarmState> {
        if !matches!(self.engine, Engine::Steady(_)) {
            return None;
        }
        let t = self.energy.as_ref().map(|en| en.solver.t.values.clone());
        let turbulence = self
            .turb
            .as_ref()
            .filter(|t| WARM_MODELS.contains(&t.model.key()))
            .map(|t| TurbWarm {
                model: t.model.key().to_string(),
                nut: t.model.nut().to_vec(),
                fields: t
                    .model
                    .scalars()
                    .into_iter()
                    .map(|(n, f)| (n.to_string(), f.values.clone()))
                    .collect(),
            });
        Some(WarmState {
            u: self.velocity().values.clone(),
            p: self.pressure().values.clone(),
            t,
            turbulence,
        })
    }

    /// Start the run from `st` (ADR 0209 D4): velocity and pressure (the face flux is rebuilt, the iteration counter restarts
    /// at zero), the temperature, and the k-epsilon / k-omega SST fields with the eddy viscosity. Everything is checked before
    /// anything is written, so an `Err` leaves the simulation as it was and the caller runs cold.
    pub fn seed_state(&mut self, st: &WarmState) -> std::result::Result<(), String> {
        if !matches!(self.engine, Engine::Steady(_)) {
            return Err("warm start is available for the steady SIMPLE and SIMPLEC algorithms".into());
        }
        let other: Vec<&str> = [
            (self.scalars.is_some(), "scalars or species"),
            (self.chem.is_some(), "chemistry"),
            (self.cht.is_some(), "conjugate heat transfer"),
            (self.radiation.is_some(), "radiation"),
            (self.electric.is_some(), "an electric potential"),
            (self.phase.is_some(), "phase change"),
            (self.two_way.is_some(), "two-way particle coupling"),
            (self.mixing.is_some(), "a mixing plane"),
            (self.dynmesh.is_some(), "a dynamic mesh"),
        ]
        .into_iter()
        .filter_map(|(on, what)| on.then_some(what))
        .collect();
        if !other.is_empty() {
            return Err(format!(
                "warm start carries velocity, pressure, temperature and the k-epsilon / k-omega SST fields; this case also has {}",
                other.join(", ")
            ));
        }
        match (&st.t, self.energy.is_some()) {
            (Some(_), false) => {
                return Err("the warm state has a temperature field and this case solves no energy equation".into())
            }
            (None, true) => {
                return Err("this case solves the energy equation and the warm state has no temperature field".into())
            }
            _ => {}
        }
        let key = self.turb.as_ref().map(|t| t.model.key().to_string());
        match (&key, &st.turbulence) {
            (None, None) => {}
            (Some(k), _) if !WARM_MODELS.contains(&k.as_str()) => {
                return Err(format!(
                "warm start carries the k-epsilon and k-omega SST fields only; this case uses the turbulence model {k}"
            ))
            }
            (Some(k), None) => {
                return Err(format!(
                    "this case uses the turbulence model {k} and the warm state has no turbulence fields"
                ))
            }
            (None, Some(w)) => {
                return Err(format!(
                    "the warm state has {} turbulence fields and this case has no turbulence model",
                    w.model
                ))
            }
            (Some(k), Some(w)) if *k != w.model => {
                return Err(format!(
                    "the warm state is of the turbulence model {} and this case uses {k}",
                    w.model
                ))
            }
            _ => {}
        }
        let nc = self.geom.n_cells();
        let size = |what: &str, n: usize| -> std::result::Result<(), String> {
            if n == nc {
                Ok(())
            } else {
                Err(format!(
                    "warm state field '{what}': {n} values for {nc} cells (another mesh)"
                ))
            }
        };
        size("u", st.u.len())?;
        size("p", st.p.len())?;
        if let Some(t) = &st.t {
            size("T", t.len())?;
        }
        if let (Some(tb), Some(w)) = (&self.turb, &st.turbulence) {
            size("nut", w.nut.len())?;
            let mut want: Vec<String> = tb.model.scalars().iter().map(|(n, _)| (*n).to_string()).collect();
            let mut have: Vec<String> = w.fields.iter().map(|(n, _)| n.clone()).collect();
            want.sort();
            have.sort();
            if want != have {
                return Err(format!(
                    "the warm state's turbulence fields {have:?} are not the model's {want:?}"
                ));
            }
            for (n, v) in &w.fields {
                size(n, v.len())?;
            }
        }
        let Engine::Steady(s) = &mut self.engine else {
            return Err("warm start is available for the steady SIMPLE and SIMPLEC algorithms".into());
        };
        s.set_fields(st.u.clone(), st.p.clone()).map_err(|e| e.to_string())?;
        if let (Some(en), Some(t)) = (&mut self.energy, &st.t) {
            en.solver.t.values = t.clone();
        }
        if let (Some(tb), Some(w)) = (&mut self.turb, &st.turbulence) {
            tb.model.initialize_from_nut(&w.nut);
            for (n, v) in &w.fields {
                if let Some(f) = tb.model.scalar_mut(n) {
                    f.values = v.clone();
                }
            }
        }
        self.done = false;
        self.converged = false;
        self.last = None;
        self.stop_reason = None;
        Ok(())
    }
}
