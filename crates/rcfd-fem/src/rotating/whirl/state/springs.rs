//! Nodal springs of any sign on a rotating part (ADR 0424, F-510): a negative support (for example the axial support that
//! makes the count of negative eigenvalues of `K_eff` odd) cannot be a model support, which must be positive; it is added to
//! the part's stiffness after the pre-stress, so it never reaches a static solve.

use crate::error::{FemError, Result, SetKind};
use crate::rotating::RotatingPart;

impl<'a> RotatingPart<'a> {
    /// The same part with `stiffness` (N/m, any finite value, negative too) added to the diagonal of its stiffness `K` at the
    /// equation of `component` of every node of the node set `set` (ADR 0424 D2). The springs are added after the static
    /// solve of the pre-stress and take no part in it; every analysis of the part ([`RotatingPart::operator`] and all that
    /// start from it) sees them. Refuses an unknown node set, a node without the component and a non-finite stiffness
    /// (`"stiffness"`).
    pub fn with_nodal_springs(mut self, set: &str, component: usize, stiffness: f64) -> Result<RotatingPart<'a>> {
        if !stiffness.is_finite() {
            return Err(FemError::InvalidValue {
                key: "stiffness",
                reason: format!("{stiffness} is not finite"),
            });
        }
        let model = self.model;
        let nodes = model.mesh().node_set(set).ok_or_else(|| FemError::UnknownSet {
            kind: SetKind::Node,
            name: set.to_string(),
        })?;
        let dofs = model.dof_map();
        let mut equations = Vec::with_capacity(nodes.len());
        for &node in nodes {
            let eq = dofs.equation(node, component).ok_or_else(|| FemError::ComponentOutOfRange {
                set: set.to_string(),
                node,
                component,
                available: dofs.components(node),
            })?;
            equations.push(eq);
        }
        let k = &mut self.k;
        for eq in equations {
            let row = &k.col[k.row_ptr[eq]..k.row_ptr[eq + 1]];
            let pos = row.binary_search(&eq).map_err(|_| FemError::InvalidValue {
                key: "stiffness",
                reason: format!("the diagonal entry of equation {eq} is outside the stiffness pattern"),
            })?;
            k.val[k.row_ptr[eq] + pos] += stiffness;
        }
        Ok(self)
    }
}
