//! In-process fluid face forces on the solid and a static solid as a co-simulation participant (one-way coupling 2).
//! Declared by ADR 0278; filled by ADR 0286.
//!
//! The interface nodes of a solid are the nodes of one surface set of a [`SolidModel`] ([`SolidInterface`]). The fluid's
//! face forces reach them through a given [`InterfaceMap`] (`forces_to_nodes`, the transpose of the consistent
//! displacement map, so that resultant and work are those of the face forces, and the moment too when the map reproduces
//! the wall points' coordinates) and are solved statically with the model's own constraints and loads
//! ([`solve_face_forces`]).
//!
//! [`StaticSolid`] is the same solve as a [`Structure`] of the co-simulation protocol (`docs/cosimulation-protocol.md`):
//! `advance` is a static solve under the nodal forces, `checkpoint` and `restore` keep and give back its state bit for bit.
//! It has no mass: against an incompressible flow the added-mass ratio is unbounded and the coupled iteration cannot be
//! stabilised, so it is for mock fluids and quasi-static use (ADR 0286 arithmetic 9). The dynamic participant is the
//! intermediate tier's.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::solve::{solve_static_with, StaticOptions};
use rcfd_fem::{SolidModel, SolidResult};
use rcfd_fv::cosim::{InterfaceMap, MapReport, MapSettings, Participant, Structure};

/// The interface nodes of a surface set of a solid model: the set's nodes in ascending order, their coordinates, and as
/// interface elements the corner nodes of each face of the set (in interface numbering, the local face order of ADR 0271).
#[derive(Debug, Clone, PartialEq)]
pub struct SolidInterface {
    set: String,
    nodes: Vec<usize>,
    coordinates: Vec<Vec3>,
    elements: Vec<Vec<usize>>,
}

impl SolidInterface {
    /// The interface of `surface_set` of `model`. Refuses an unknown set, naming it. The mid-edge nodes of a quadratic face
    /// are interface nodes but not corners of an interface element.
    pub fn new(model: &SolidModel, surface_set: &str) -> Result<Self, String> {
        let mesh = model.mesh();
        let faces = mesh
            .surface_set(surface_set)
            .ok_or_else(|| format!("solid interface: no surface set '{surface_set}'"))?;
        let nodes = mesh.surface_set_nodes(surface_set).map_err(|e| e.to_string())?;
        let coordinates = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
        let elements = faces
            .iter()
            .map(|&f| {
                let corners = mesh.element_kind(f.element).face_corners();
                mesh.face_nodes(f)[..corners]
                    .iter()
                    .map(|n| nodes.binary_search(n).expect("a face node is a node of its set"))
                    .collect()
            })
            .collect();
        Ok(SolidInterface {
            set: surface_set.to_string(),
            nodes,
            coordinates,
            elements,
        })
    }

    /// The surface set's name.
    pub fn surface_set(&self) -> &str {
        &self.set
    }

    /// Number of interface nodes.
    pub fn n_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Mesh node of every interface node, ascending.
    pub fn nodes(&self) -> &[usize] {
        &self.nodes
    }

    /// Coordinates of the interface nodes (reference configuration) [m].
    pub fn coordinates(&self) -> &[Vec3] {
        &self.coordinates
    }

    /// The corner nodes of each face of the set, in interface numbering (elements of [`InterfaceMap::build`]).
    pub fn elements(&self) -> &[Vec<usize>] {
        &self.elements
    }

    /// [`InterfaceMap::build`] from these nodes and elements to a fluid's wall `points` and the points of its wall faces.
    pub fn build_map(
        &self,
        points: &[Vec3],
        face_points: Vec<Vec<usize>>,
        settings: &MapSettings,
    ) -> Result<(InterfaceMap, MapReport), String> {
        InterfaceMap::build(&self.coordinates, &self.elements, points, face_points, settings)
    }

    /// One force per interface node [N] as nodal loads on the mesh nodes. Refuses a wrong count.
    pub fn nodal_forces(&self, forces: &[Vec3]) -> Result<Vec<NodalForceAt>, String> {
        if forces.len() != self.nodes.len() {
            return Err(format!(
                "solid interface '{}': {} forces for {} interface nodes",
                self.set,
                forces.len(),
                self.nodes.len()
            ));
        }
        Ok(self
            .nodes
            .iter()
            .zip(forces)
            .map(|(&n, &f)| NodalForceAt::new(n, f))
            .collect())
    }

    /// The displacement of every interface node [m] from a result of the model.
    pub fn displacements(&self, result: &SolidResult) -> Vec<Vec3> {
        self.nodes
            .iter()
            .map(|&n| {
                let u = result.displacement(n);
                [u[0], u[1], u[2]]
            })
            .collect()
    }
}

/// Nodal forces on the interface nodes from the fluid's face forces [N], through `map` (its nodes are the interface
/// nodes, in their order).
pub fn face_forces_to_nodes(
    interface: &SolidInterface,
    map: &InterfaceMap,
    face_forces: &[Vec3],
) -> Result<Vec<Vec3>, String> {
    if map.n_nodes() != interface.n_nodes() {
        return Err(format!(
            "solid interface '{}': the map is for {} nodes, the interface has {}",
            interface.surface_set(),
            map.n_nodes(),
            interface.n_nodes()
        ));
    }
    map.forces_to_nodes(face_forces)
}

/// The static solve of `model` (its own constraints and loads) with, in addition, the fluid's face forces mapped to the
/// interface nodes by `map`.
pub fn solve_face_forces(
    model: &SolidModel,
    interface: &SolidInterface,
    map: &InterfaceMap,
    face_forces: &[Vec3],
    options: &StaticOptions,
) -> Result<SolidResult, String> {
    let nodal = face_forces_to_nodes(interface, map, face_forces)?;
    let loads = interface.nodal_forces(&nodal)?;
    solve_static_with(model, &[], &loads, options).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, PartialEq)]
struct SolidState {
    time: f64,
    displacement: Vec<f64>,
    forces: Vec<Vec3>,
}

/// A static solid as a structural participant of a coupled step: `advance` solves the model statically under its own
/// loads and the interface forces it is given; the state is the time, the displacement of every unknown and the interface
/// forces of the last advance. No mass: for mock fluids and quasi-static use only.
#[derive(Debug, Clone)]
pub struct StaticSolid {
    /// Options of the static solve of every advance.
    pub options: StaticOptions,
    name: String,
    model: SolidModel,
    interface: SolidInterface,
    state: SolidState,
    saved: Option<SolidState>,
}

impl StaticSolid {
    /// The participant `name` for `model` with the interface `surface_set`, at rest (zero displacement) at time 0.
    /// Refuses a model that does not validate and an unknown set.
    pub fn new(name: &str, model: SolidModel, surface_set: &str, options: StaticOptions) -> Result<Self, String> {
        model
            .validate()
            .map_err(|e| format!("static solid '{name}': {e}"))?;
        let interface =
            SolidInterface::new(&model, surface_set).map_err(|e| format!("static solid '{name}': {e}"))?;
        let n = model.dof_map().n_equations();
        let m = interface.n_nodes();
        Ok(StaticSolid {
            options,
            name: name.to_string(),
            model,
            interface,
            state: SolidState {
                time: 0.0,
                displacement: vec![0.0; n],
                forces: vec![vec3::ZERO; m],
            },
            saved: None,
        })
    }

    /// The model.
    pub fn model(&self) -> &SolidModel {
        &self.model
    }

    /// The interface.
    pub fn interface(&self) -> &SolidInterface {
        &self.interface
    }

    /// Displacement of every unknown of the model [m] at the participant's time.
    pub fn displacement(&self) -> &[f64] {
        &self.state.displacement
    }

    /// The interface forces of the last advance [N] (zero before the first).
    pub fn forces(&self) -> &[Vec3] {
        &self.state.forces
    }
}

impl Participant for StaticSolid {
    fn name(&self) -> &str {
        &self.name
    }

    fn time(&self) -> f64 {
        self.state.time
    }

    fn checkpoint(&mut self) -> Result<(), String> {
        self.saved = Some(self.state.clone());
        Ok(())
    }

    fn restore(&mut self) -> Result<(), String> {
        let saved = self
            .saved
            .as_ref()
            .ok_or_else(|| format!("static solid '{}': restore without a checkpoint", self.name))?;
        self.state.clone_from(saved);
        Ok(())
    }
}

impl Structure for StaticSolid {
    fn node_coordinates(&self) -> Vec<Vec3> {
        self.interface.coordinates().to_vec()
    }

    fn advance(&mut self, dt: f64, forces: &[Vec3]) -> Result<Vec<Vec3>, String> {
        let fail = |e: String| format!("static solid '{}': {e}", self.name);
        if !(dt.is_finite() && dt > 0.0) {
            return Err(fail(format!("the step must be positive, got {dt}")));
        }
        if !forces.iter().flatten().all(|x| x.is_finite()) {
            return Err(fail("a non-finite interface force".into()));
        }
        let loads = self.interface.nodal_forces(forces).map_err(fail)?;
        let result = solve_static_with(&self.model, &[], &loads, &self.options).map_err(|e| fail(e.to_string()))?;
        let q = self.interface.displacements(&result);
        self.state.time += dt;
        self.state.displacement = result.displacement_values().to_vec();
        self.state.forces = forces.to_vec();
        Ok(q)
    }
}
