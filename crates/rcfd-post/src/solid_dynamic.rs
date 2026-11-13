//! The dynamic solid as a co-simulation participant (`DynamicSolid` behind `cosim::Structure`). Declared by ADR 0291;
//! filled by ADR 0410 (the registration of F-400; design stub ADR 0303).
//!
//! The interface is a surface set of a [`SolidModel`] ([`SolidInterface`]: its nodes ascending, its faces' corners as
//! interface elements, so that `InterfaceMap::build` makes the weights from the two surfaces). `advance(dt, forces)`
//! integrates the model with the transient integrator of ADR 0389/0393 over `sub_steps` steps of the integrator, the
//! interface forces held over them (`Integrator::run_with_force`), and returns the interface nodes' displacement at the new
//! time. `checkpoint` and `restore` keep and give back the integrator's state and the last held forces bit for bit.
//!
//! With Newmark's method only the force at the end of the step enters its equilibrium, so a converged coupled step is the
//! implicit step of the coupled system (ADR 0410 D6). With the generalised-alpha method the held force is weighed at both
//! ends of the step (first order in the coupling, ADR 0410 D2).

use crate::solid_coupling::SolidInterface;
use rcfd_core::vec3::{self, Vec3};
use rcfd_fem::transient::{Integrator, LoadHistory, TransientOptions, TransientState};
use rcfd_fem::SolidModel;
use rcfd_fv::cosim::{Participant, Structure};

#[derive(Debug, Clone, PartialEq)]
struct DynamicState {
    transient: TransientState,
    forces: Vec<Vec3>,
}

/// A linear elastic solid with mass as a structural participant of a coupled step (ADR 0410).
#[derive(Debug, Clone)]
pub struct DynamicSolid {
    name: String,
    interface: SolidInterface,
    /// Equation of each component of every interface node (`None`: the component has no equation).
    equations: Vec<[Option<usize>; 3]>,
    integrator: Integrator,
    sub_steps: u64,
    forces: Vec<Vec3>,
    saved: Option<DynamicState>,
}

impl DynamicSolid {
    /// The participant `name` for `model` with the interface `surface_set`, the load histories `loads` (full patterns, as
    /// for [`Integrator::new`]) and the integrator `options`; one coupling step is `sub_steps` steps of the integrator. It
    /// starts at rest at time 0. Refuses a model or options the integrator refuses, an unknown set and `sub_steps = 0`,
    /// each message naming the participant.
    pub fn new(
        name: &str,
        model: &SolidModel,
        surface_set: &str,
        loads: &[LoadHistory],
        options: &TransientOptions,
        sub_steps: usize,
    ) -> Result<Self, String> {
        let fail = |e: String| format!("dynamic solid '{name}': {e}");
        if sub_steps == 0 {
            return Err(fail("at least one sub-step is needed".into()));
        }
        model.validate().map_err(|e| fail(e.to_string()))?;
        let interface = SolidInterface::new(model, surface_set).map_err(fail)?;
        let dofs = model.dof_map();
        let equations = interface
            .nodes()
            .iter()
            .map(|&n| [dofs.equation(n, 0), dofs.equation(n, 1), dofs.equation(n, 2)])
            .collect();
        let integrator = Integrator::new(model, loads, options).map_err(|e| fail(e.to_string()))?;
        let m = interface.n_nodes();
        let mut solid = DynamicSolid {
            name: name.to_string(),
            interface,
            equations,
            integrator,
            sub_steps: sub_steps as u64,
            forces: vec![vec3::ZERO; m],
            saved: None,
        };
        let zero = vec![0.0; solid.integrator.n_unknowns()];
        solid.start(0.0, &zero, &zero)?;
        Ok(solid)
    }

    /// Restarts at `t0` from the full displacement and velocity ([`Integrator::start`]; the initial acceleration solves
    /// `M a0 = f(t0) - C v0 - K u0`, with no interface force); the held forces are reset to zero and the checkpoint dropped.
    pub fn start(&mut self, t0: f64, u0: &[f64], v0: &[f64]) -> Result<(), String> {
        self.integrator
            .start(t0, u0, v0)
            .map_err(|e| format!("dynamic solid '{}': {e}", self.name))?;
        self.forces = vec![vec3::ZERO; self.interface.n_nodes()];
        self.saved = None;
        Ok(())
    }

    /// The interface.
    pub fn interface(&self) -> &SolidInterface {
        &self.interface
    }

    /// The integrator.
    pub fn integrator(&self) -> &Integrator {
        &self.integrator
    }

    /// Integrator steps per coupling step.
    pub fn n_sub_steps(&self) -> usize {
        self.sub_steps as usize
    }

    /// The coupling step this participant takes: `sub_steps` times the integrator's step [s].
    pub fn coupling_step(&self) -> f64 {
        self.sub_steps as f64 * self.integrator.time_step()
    }

    /// The integrator's state (a clone).
    pub fn state(&self) -> TransientState {
        self.integrator.state().expect("a dynamic solid always has a state")
    }

    /// Displacement of every unknown [m] ([`Integrator::displacement`]).
    pub fn displacement(&self) -> Vec<f64> {
        self.integrator.displacement().expect("a dynamic solid always has a state")
    }

    /// Velocity of every unknown [m/s].
    pub fn velocity(&self) -> Vec<f64> {
        self.integrator.velocity().expect("a dynamic solid always has a state")
    }

    /// Acceleration of every unknown [m/s^2].
    pub fn acceleration(&self) -> Vec<f64> {
        self.integrator.acceleration().expect("a dynamic solid always has a state")
    }

    /// The interface forces held over the last advance [N] (zero before the first).
    pub fn forces(&self) -> &[Vec3] {
        &self.forces
    }

    /// The interface nodes' displacement [m] from a full displacement.
    fn interface_values(&self, full: &[f64]) -> Vec<Vec3> {
        self.equations
            .iter()
            .map(|eq| {
                let at = |c: usize| eq[c].map_or(0.0, |e| full[e]);
                [at(0), at(1), at(2)]
            })
            .collect()
    }
}

impl Participant for DynamicSolid {
    fn name(&self) -> &str {
        &self.name
    }

    fn time(&self) -> f64 {
        self.integrator.time().expect("a dynamic solid always has a state")
    }

    fn checkpoint(&mut self) -> Result<(), String> {
        self.saved = Some(DynamicState {
            transient: self.state(),
            forces: self.forces.clone(),
        });
        Ok(())
    }

    fn restore(&mut self) -> Result<(), String> {
        let saved = self
            .saved
            .as_ref()
            .ok_or_else(|| format!("dynamic solid '{}': restore without a checkpoint", self.name))?;
        self.integrator
            .restore(saved.transient.clone())
            .map_err(|e| format!("dynamic solid '{}': {e}", self.name))?;
        self.forces.clone_from(&saved.forces);
        Ok(())
    }
}

impl Structure for DynamicSolid {
    fn node_coordinates(&self) -> Vec<Vec3> {
        self.interface.coordinates().to_vec()
    }

    fn elements(&self) -> Vec<Vec<usize>> {
        self.interface.elements().to_vec()
    }

    fn advance(&mut self, dt: f64, forces: &[Vec3]) -> Result<Vec<Vec3>, String> {
        let fail = |e: String| format!("dynamic solid '{}': {e}", self.name);
        let step = self.coupling_step();
        if !(dt.is_finite() && dt > 0.0) {
            return Err(fail(format!("the step must be positive, got {dt}")));
        }
        if (dt - step).abs() > 1e-9 * dt {
            return Err(fail(format!(
                "the coupling step {dt} is not {} sub-steps of {}",
                self.sub_steps,
                self.integrator.time_step()
            )));
        }
        if forces.len() != self.equations.len() {
            return Err(fail(format!(
                "{} forces for {} interface nodes",
                forces.len(),
                self.equations.len()
            )));
        }
        if !forces.iter().flatten().all(|x| x.is_finite()) {
            return Err(fail("a non-finite interface force".into()));
        }
        let mut full = vec![0.0; self.integrator.n_unknowns()];
        for (eq, f) in self.equations.iter().zip(forces) {
            for c in 0..3 {
                if let Some(e) = eq[c] {
                    full[e] += f[c];
                }
            }
        }
        self.integrator
            .run_with_force(self.sub_steps, &full)
            .map_err(|e| fail(e.to_string()))?;
        self.forces = forces.to_vec();
        let u = self.displacement();
        Ok(self.interface_values(&u))
    }
}
