//! The solid model (ADR 0271 D5): numbering of the unknowns, materials, constraints, loads, the model and its result.
//!
//! Every type is built through a constructor that validates; structs have private fields and enums are non-exhaustive,
//! so that a field or variant added by a later tier breaks no caller (the request of ADR 0290 section 2.4). Loads and
//! constraints are plain data naming sets, readable from a file; there are no closures.

use crate::error::{FemError, Result, SetKind};
use crate::mesh::SolidMesh;
use rcfd_core::vec3::Vec3;

/// Numbering of the unknowns: (node, component) to equation. The only place that knows how many unknowns a node has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DofMap {
    offsets: Vec<usize>,
}

impl DofMap {
    /// `per_node` unknowns at each of `n_nodes` nodes, numbered node by node.
    pub fn uniform(n_nodes: usize, per_node: usize) -> DofMap {
        DofMap { offsets: (0..=n_nodes).map(|n| n * per_node).collect() }
    }

    /// The given number of unknowns at each node, numbered node by node.
    pub fn from_counts(counts: &[usize]) -> DofMap {
        let mut offsets = Vec::with_capacity(counts.len() + 1);
        let mut s = 0;
        offsets.push(0);
        for &c in counts {
            s += c;
            offsets.push(s);
        }
        DofMap { offsets }
    }

    /// The numbering a mesh needs: three translations per node for the solid kinds of the basic tier.
    pub fn for_mesh(mesh: &SolidMesh) -> DofMap {
        DofMap::uniform(mesh.nodes().len(), 3)
    }

    /// Number of nodes.
    pub fn n_nodes(&self) -> usize {
        self.offsets.len() - 1
    }
    /// Number of unknowns.
    pub fn n_equations(&self) -> usize {
        *self.offsets.last().expect("offsets has n_nodes + 1 entries")
    }
    /// Number of unknowns of a node (0 for a node that does not exist).
    pub fn components(&self, node: usize) -> usize {
        if node + 1 < self.offsets.len() {
            self.offsets[node + 1] - self.offsets[node]
        } else {
            0
        }
    }
    /// The equation of (node, component), or `None` if the node or the component does not exist.
    pub fn equation(&self, node: usize, component: usize) -> Option<usize> {
        (component < self.components(node)).then(|| self.offsets[node] + component)
    }
    /// The (node, component) of an equation, or `None` beyond the last.
    pub fn node_component(&self, equation: usize) -> Option<(usize, usize)> {
        if equation >= self.n_equations() {
            return None;
        }
        let node = self.offsets.partition_point(|&o| o <= equation) - 1;
        Some((node, equation - self.offsets[node]))
    }
}

/// A material. Non-exhaustive: later tiers add variants (ADR 0311).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Material {
    /// Isotropic linear elasticity.
    #[non_exhaustive]
    IsotropicElastic {
        /// Young's modulus, Pa, positive.
        e: f64,
        /// Poisson ratio, in the open interval (-1, 0.5).
        nu: f64,
        /// Density, kg/m^3, non-negative.
        density: f64,
        /// Linear expansion coefficient, 1/K.
        expansion: f64,
    },
}

impl Material {
    /// An isotropic linear elastic material. Refuses (naming the key) `e <= 0`, `nu` outside (-1, 0.5), a negative
    /// density and any non-finite value.
    pub fn isotropic_elastic(e: f64, nu: f64, density: f64, expansion: f64) -> Result<Material> {
        if !(e > 0.0 && e.is_finite()) {
            return Err(FemError::InvalidMaterial {
                key: "e",
                value: e,
                reason: "Young's modulus must be positive and finite",
            });
        }
        if !(nu > -1.0 && nu < 0.5) {
            return Err(FemError::InvalidMaterial {
                key: "nu",
                value: nu,
                reason: "Poisson ratio must lie in the open interval (-1, 0.5)",
            });
        }
        if !(density >= 0.0 && density.is_finite()) {
            return Err(FemError::InvalidMaterial {
                key: "density",
                value: density,
                reason: "density must be non-negative and finite",
            });
        }
        if !expansion.is_finite() {
            return Err(FemError::InvalidMaterial {
                key: "expansion",
                value: expansion,
                reason: "expansion coefficient must be finite",
            });
        }
        Ok(Material::IsotropicElastic { e, nu, density, expansion })
    }

    /// Young's modulus.
    pub fn youngs_modulus(&self) -> f64 {
        match self {
            Material::IsotropicElastic { e, .. } => *e,
        }
    }
    /// Poisson ratio.
    pub fn poisson_ratio(&self) -> f64 {
        match self {
            Material::IsotropicElastic { nu, .. } => *nu,
        }
    }
    /// Density.
    pub fn density(&self) -> f64 {
        match self {
            Material::IsotropicElastic { density, .. } => *density,
        }
    }
    /// Linear expansion coefficient.
    pub fn expansion(&self) -> f64 {
        match self {
            Material::IsotropicElastic { expansion, .. } => *expansion,
        }
    }
}

/// A constraint. Non-exhaustive: the intermediate tier adds multi-point, rigid and cyclic constraints (ADR 0290).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Constraint {
    /// Prescribed value of one component of the displacement at every node of a node set.
    #[non_exhaustive]
    Displacement {
        /// Node set name.
        set: String,
        /// Component (0, 1, 2 for the translations x, y, z).
        component: usize,
        /// Prescribed value, m.
        value: f64,
    },
}

impl Constraint {
    /// A prescribed displacement component on a node set.
    pub fn displacement(set: &str, component: usize, value: f64) -> Constraint {
        Constraint::Displacement { set: set.to_string(), component, value }
    }
}

/// A load. Non-exhaustive; plain data naming sets (ADR 0270 section 2.4).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Load {
    /// The same force on each node of a node set, N.
    #[non_exhaustive]
    NodalForce {
        /// Node set name.
        set: String,
        /// Force on each node.
        force: Vec3,
    },
    /// A uniform traction on a surface set, Pa.
    #[non_exhaustive]
    Traction {
        /// Surface set name.
        set: String,
        /// Traction vector.
        traction: Vec3,
    },
    /// A uniform pressure on a surface set, Pa, positive when it presses on the solid.
    #[non_exhaustive]
    Pressure {
        /// Surface set name.
        set: String,
        /// Pressure.
        pressure: f64,
    },
    /// Gravity on the whole model, m/s^2.
    #[non_exhaustive]
    Gravity {
        /// Acceleration of gravity.
        acceleration: Vec3,
    },
    /// The centrifugal load of the whole model rotating at constant speed about an axis.
    #[non_exhaustive]
    Centrifugal {
        /// Direction of the axis (non-zero; not necessarily of unit length).
        axis: Vec3,
        /// A point on the axis.
        origin: Vec3,
        /// Angular speed, rad/s.
        omega: f64,
    },
    /// A body force per unit volume given at the nodes, N/m^3.
    #[non_exhaustive]
    BodyForceNodal {
        /// One vector per node.
        values: Vec<Vec3>,
    },
    /// A nodal temperature field for the thermal strain, K.
    #[non_exhaustive]
    Temperature {
        /// One temperature per node.
        nodal: Vec<f64>,
        /// Reference (strain-free) temperature.
        reference: f64,
    },
}

impl Load {
    /// The same force on each node of a node set.
    pub fn nodal_force(set: &str, force: Vec3) -> Load {
        Load::NodalForce { set: set.to_string(), force }
    }
    /// A uniform traction on a surface set.
    pub fn traction(set: &str, traction: Vec3) -> Load {
        Load::Traction { set: set.to_string(), traction }
    }
    /// A uniform pressure on a surface set (positive pressing on the solid).
    pub fn pressure(set: &str, pressure: f64) -> Load {
        Load::Pressure { set: set.to_string(), pressure }
    }
    /// Gravity on the whole model.
    pub fn gravity(acceleration: Vec3) -> Load {
        Load::Gravity { acceleration }
    }
    /// The centrifugal load of a rotation about `axis` through `origin` at `omega`.
    pub fn centrifugal(axis: Vec3, origin: Vec3, omega: f64) -> Load {
        Load::Centrifugal { axis, origin, omega }
    }
    /// A body force per unit volume at the nodes.
    pub fn body_force_nodal(values: Vec<Vec3>) -> Load {
        Load::BodyForceNodal { values }
    }
    /// A nodal temperature field with its reference temperature.
    pub fn temperature(nodal: Vec<f64>, reference: f64) -> Load {
        Load::Temperature { nodal, reference }
    }
}

fn finite_vec(key: &'static str, v: Vec3) -> Result<()> {
    if v.iter().all(|c| c.is_finite()) {
        Ok(())
    } else {
        Err(FemError::InvalidValue { key, reason: format!("{v:?} is not finite") })
    }
}

fn finite(key: &'static str, v: f64) -> Result<()> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(FemError::InvalidValue { key, reason: format!("{v} is not finite") })
    }
}

/// A solid model: mesh, materials on element sets, constraints and loads.
#[derive(Debug, Clone, PartialEq)]
pub struct SolidModel {
    mesh: SolidMesh,
    materials: Vec<(String, Material)>,
    constraints: Vec<Constraint>,
    loads: Vec<Load>,
}

impl SolidModel {
    /// A model on a mesh, with no material, constraint or load yet.
    pub fn new(mesh: SolidMesh) -> SolidModel {
        SolidModel { mesh, materials: Vec::new(), constraints: Vec::new(), loads: Vec::new() }
    }

    /// The mesh.
    pub fn mesh(&self) -> &SolidMesh {
        &self.mesh
    }
    /// Materials with the element sets they are assigned to, in the order added.
    pub fn materials(&self) -> &[(String, Material)] {
        &self.materials
    }
    /// The constraints, in the order added.
    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }
    /// The loads, in the order added.
    pub fn loads(&self) -> &[Load] {
        &self.loads
    }
    /// The numbering of the unknowns of this model.
    pub fn dof_map(&self) -> DofMap {
        DofMap::for_mesh(&self.mesh)
    }

    fn need_set(&self, kind: SetKind, name: &str) -> Result<()> {
        let exists = match kind {
            SetKind::Node => self.mesh.node_set(name).is_some(),
            SetKind::Element => self.mesh.element_set(name).is_some(),
            SetKind::Surface => self.mesh.surface_set(name).is_some(),
        };
        if exists {
            Ok(())
        } else {
            Err(FemError::UnknownSet { kind, name: name.to_string() })
        }
    }

    /// Assigns a material to an element set.
    pub fn add_material(&mut self, element_set: &str, material: Material) -> Result<()> {
        self.need_set(SetKind::Element, element_set)?;
        self.materials.push((element_set.to_string(), material));
        Ok(())
    }

    /// Adds a constraint; its set must exist and every node of it must have the component.
    pub fn add_constraint(&mut self, c: Constraint) -> Result<()> {
        match &c {
            Constraint::Displacement { set, component, value } => {
                self.need_set(SetKind::Node, set)?;
                finite("value", *value)?;
                let dofs = self.dof_map();
                for &node in self.mesh.node_set(set).expect("checked") {
                    if dofs.equation(node, *component).is_none() {
                        return Err(FemError::ComponentOutOfRange {
                            set: set.clone(),
                            node,
                            component: *component,
                            available: dofs.components(node),
                        });
                    }
                }
            }
        }
        self.constraints.push(c);
        Ok(())
    }

    /// Adds a load; the sets it names must exist with the right kind, its values be finite and its fields of the node
    /// count.
    pub fn add_load(&mut self, l: Load) -> Result<()> {
        let n = self.mesh.nodes().len();
        match &l {
            Load::NodalForce { set, force } => {
                self.need_set(SetKind::Node, set)?;
                finite_vec("force", *force)?;
            }
            Load::Traction { set, traction } => {
                self.need_set(SetKind::Surface, set)?;
                finite_vec("traction", *traction)?;
            }
            Load::Pressure { set, pressure } => {
                self.need_set(SetKind::Surface, set)?;
                finite("pressure", *pressure)?;
            }
            Load::Gravity { acceleration } => finite_vec("acceleration", *acceleration)?,
            Load::Centrifugal { axis, origin, omega } => {
                finite_vec("axis", *axis)?;
                if axis.iter().all(|&c| c == 0.0) {
                    return Err(FemError::InvalidValue { key: "axis", reason: "the axis is the zero vector".into() });
                }
                finite_vec("origin", *origin)?;
                finite("omega", *omega)?;
            }
            Load::BodyForceNodal { values } => {
                if values.len() != n {
                    return Err(FemError::FieldLength { key: "values", expected: n, found: values.len() });
                }
                for v in values {
                    finite_vec("values", *v)?;
                }
            }
            Load::Temperature { nodal, reference } => {
                if nodal.len() != n {
                    return Err(FemError::FieldLength { key: "nodal", expected: n, found: nodal.len() });
                }
                for &t in nodal {
                    finite("nodal", t)?;
                }
                finite("reference", *reference)?;
            }
        }
        self.loads.push(l);
        Ok(())
    }

    /// Index into [`SolidModel::materials`] of each element; refuses an element without material or with two.
    pub fn material_indices(&self) -> Result<Vec<usize>> {
        let mut of: Vec<Option<usize>> = vec![None; self.mesh.n_elements()];
        for (k, (set, _)) in self.materials.iter().enumerate() {
            for &e in self.mesh.element_set(set).expect("checked when added") {
                if let Some(first) = of[e] {
                    return Err(FemError::MaterialOverlap {
                        element: e,
                        first: self.materials[first].0.clone(),
                        second: set.clone(),
                    });
                }
                of[e] = Some(k);
            }
        }
        of.into_iter()
            .enumerate()
            .map(|(element, m)| m.ok_or(FemError::MaterialUnassigned { element }))
            .collect()
    }

    /// Checks the model as a whole: every element has exactly one material.
    pub fn validate(&self) -> Result<()> {
        self.material_indices().map(|_| ())
    }

    /// The material of an element, if it has exactly one.
    pub fn material_of(&self, element: usize) -> Option<&Material> {
        let mut found = None;
        for (set, m) in &self.materials {
            if self.mesh.element_set(set).is_some_and(|s| s.contains(&element)) {
                if found.is_some() {
                    return None;
                }
                found = Some(m);
            }
        }
        found
    }
}

/// Statistics of a solve.
#[derive(Debug, Clone, PartialEq)]
pub struct SolverStats {
    solver: String,
    unknowns: usize,
    iterations: usize,
    relative_residual: f64,
}

impl SolverStats {
    /// Statistics of a solve by the named solver.
    pub fn new(solver: &str, unknowns: usize, iterations: usize, relative_residual: f64) -> SolverStats {
        SolverStats { solver: solver.to_string(), unknowns, iterations, relative_residual }
    }
    /// Solver name.
    pub fn solver(&self) -> &str {
        &self.solver
    }
    /// Number of unknowns solved for.
    pub fn unknowns(&self) -> usize {
        self.unknowns
    }
    /// Iterations (0 for a direct solve).
    pub fn iterations(&self) -> usize {
        self.iterations
    }
    /// Final relative residual.
    pub fn relative_residual(&self) -> f64 {
        self.relative_residual
    }
}

/// The result of a solve: displacement and reactions per unknown, strain and stress at the integration points, solver
/// statistics. Derived fields come from `recover` (ADR 0277).
#[derive(Debug, Clone, PartialEq)]
pub struct SolidResult {
    dofs: DofMap,
    displacement: Vec<f64>,
    reactions: Option<Vec<f64>>,
    ip_offsets: Vec<usize>,
    strain: Vec<[f64; 6]>,
    stress: Vec<[f64; 6]>,
    stats: Option<SolverStats>,
}

impl SolidResult {
    /// A result with the displacement of every unknown of `dofs`.
    pub fn new(dofs: DofMap, displacement: Vec<f64>) -> Result<SolidResult> {
        if displacement.len() != dofs.n_equations() {
            return Err(FemError::FieldLength {
                key: "displacement",
                expected: dofs.n_equations(),
                found: displacement.len(),
            });
        }
        Ok(SolidResult {
            dofs,
            displacement,
            reactions: None,
            ip_offsets: vec![0],
            strain: Vec::new(),
            stress: Vec::new(),
            stats: None,
        })
    }

    /// Adds the reactions, one per unknown.
    pub fn with_reactions(mut self, reactions: Vec<f64>) -> Result<SolidResult> {
        if reactions.len() != self.dofs.n_equations() {
            return Err(FemError::FieldLength {
                key: "reactions",
                expected: self.dofs.n_equations(),
                found: reactions.len(),
            });
        }
        self.reactions = Some(reactions);
        Ok(self)
    }

    /// Adds strain and stress at the integration points: `offsets[e]..offsets[e + 1]` are the points of element `e`
    /// (offsets start at 0, never decrease and end at the point count); six components xx, yy, zz, xy, yz, zx, strains
    /// with engineering shear.
    pub fn with_integration_points(
        mut self,
        offsets: Vec<usize>,
        strain: Vec<[f64; 6]>,
        stress: Vec<[f64; 6]>,
    ) -> Result<SolidResult> {
        let ok = offsets.first() == Some(&0) && offsets.windows(2).all(|w| w[0] <= w[1]);
        if !ok {
            return Err(FemError::InvalidValue {
                key: "offsets",
                reason: "must start at 0 and never decrease".into(),
            });
        }
        let np = *offsets.last().expect("non-empty");
        if strain.len() != np {
            return Err(FemError::FieldLength { key: "strain", expected: np, found: strain.len() });
        }
        if stress.len() != np {
            return Err(FemError::FieldLength { key: "stress", expected: np, found: stress.len() });
        }
        self.ip_offsets = offsets;
        self.strain = strain;
        self.stress = stress;
        Ok(self)
    }

    /// Adds the solver statistics.
    pub fn with_stats(mut self, stats: SolverStats) -> SolidResult {
        self.stats = Some(stats);
        self
    }

    /// The numbering of the unknowns.
    pub fn dofs(&self) -> &DofMap {
        &self.dofs
    }
    /// Displacement of every unknown.
    pub fn displacement_values(&self) -> &[f64] {
        &self.displacement
    }
    /// The values of a node's unknowns (empty for a node that does not exist).
    pub fn displacement(&self, node: usize) -> &[f64] {
        match self.dofs.equation(node, 0) {
            Some(start) => &self.displacement[start..start + self.dofs.components(node)],
            None => &[],
        }
    }
    /// Reactions per unknown, if recovered.
    pub fn reactions(&self) -> Option<&[f64]> {
        self.reactions.as_deref()
    }
    /// Integration-point offsets per element (`[0]` when none were stored).
    pub fn integration_point_offsets(&self) -> &[usize] {
        &self.ip_offsets
    }
    /// Strain at the integration points.
    pub fn strain(&self) -> &[[f64; 6]] {
        &self.strain
    }
    /// Stress at the integration points.
    pub fn stress(&self) -> &[[f64; 6]] {
        &self.stress
    }
    /// Solver statistics, if given.
    pub fn stats(&self) -> Option<&SolverStats> {
        self.stats.as_ref()
    }
}
