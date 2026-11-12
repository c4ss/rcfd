//! Nodal, surface and body loads (nodal forces: ADR 0275; the others: ADR 0276). Declared by ADR 0271; filled by ADR 0275.
//!
//! [`nodal_load_vector`] is the one function the static solve calls for the right-hand side. ADR 0275 fills the nodal
//! forces of the model ([`Load::NodalForce`]) and per-node forces ([`NodalForceAt`]); every other load variant is refused
//! naming it and the package that adds it, never ignored.

use crate::error::{FemError, Result};
use crate::model::{Load, SolidModel};
use rcfd_core::vec3::Vec3;

/// A force at one node, N.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodalForceAt {
    /// Node index.
    pub node: usize,
    /// The force.
    pub force: Vec3,
}

impl NodalForceAt {
    /// A force at a node.
    pub fn new(node: usize, force: Vec3) -> NodalForceAt {
        NodalForceAt { node, force }
    }
}

/// The global load vector of the model's loads and of `extra` per-node forces, one entry per unknown. Refuses a node that
/// does not exist (naming it), a non-finite force, and every load variant not supported yet.
pub fn nodal_load_vector(model: &SolidModel, extra: &[NodalForceAt]) -> Result<Vec<f64>> {
    let dofs = model.dof_map();
    let mut f = vec![0.0; dofs.n_equations()];
    let mut add = |node: usize, force: &Vec3| -> Result<()> {
        if node >= dofs.n_nodes() {
            return Err(FemError::MissingNode {
                role: "nodal force",
                node,
                n_nodes: dofs.n_nodes(),
            });
        }
        if force.iter().any(|c| !c.is_finite()) {
            return Err(FemError::InvalidValue {
                key: "force",
                reason: format!("{force:?} at node {node} is not finite"),
            });
        }
        for (k, &fk) in force.iter().enumerate() {
            let eq = dofs.equation(node, k).ok_or(FemError::MissingComponent {
                role: "nodal force",
                node,
                component: k,
                available: dofs.components(node),
            })?;
            f[eq] += fk;
        }
        Ok(())
    };
    for load in model.loads() {
        match load {
            Load::NodalForce { set, force } => {
                let nodes = model.mesh().node_set(set).ok_or_else(|| FemError::UnknownSet {
                    kind: crate::error::SetKind::Node,
                    name: set.clone(),
                })?;
                for &node in nodes {
                    add(node, force)?;
                }
            }
            other => return Err(unsupported(other)),
        }
    }
    for p in extra {
        add(p.node, &p.force)?;
    }
    Ok(f)
}

fn unsupported(load: &Load) -> FemError {
    let (load, package) = match load {
        Load::NodalForce { .. } => ("NodalForce", "ADR 0275"),
        Load::Traction { .. } => ("Traction", "ADR 0276"),
        Load::Pressure { .. } => ("Pressure", "ADR 0276"),
        Load::Gravity { .. } => ("Gravity", "ADR 0276"),
        Load::Centrifugal { .. } => ("Centrifugal", "ADR 0276"),
        Load::BodyForceNodal { .. } => ("BodyForceNodal", "ADR 0276"),
        Load::Temperature { .. } => ("Temperature", "ADR 0281"),
    };
    FemError::UnsupportedLoad { load, package }
}
