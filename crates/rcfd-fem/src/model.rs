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
        DofMap {
            offsets: (0..=n_nodes).map(|n| n * per_node).collect(),
        }
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

    /// The numbering a mesh needs: three translations per node, and six unknowns (the translations, then the small
    /// rotations about x, y, z) at each node of a Beam2 element (ADR 0417 D1). A mesh without a Beam2 block gets
    /// `DofMap::uniform(n, 3)`.
    pub fn for_mesh(mesh: &SolidMesh) -> DofMap {
        if mesh
            .blocks()
            .iter()
            .all(|b| b.kind() != crate::mesh::ElementKind::Beam2)
        {
            return DofMap::uniform(mesh.nodes().len(), 3);
        }
        let mut counts = vec![3; mesh.nodes().len()];
        for e in 0..mesh.n_elements() {
            if mesh.element_kind(e) == crate::mesh::ElementKind::Beam2 {
                for &n in mesh.element_nodes(e) {
                    counts[n] = 6;
                }
            }
        }
        DofMap::from_counts(&counts)
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
        Ok(Material::IsotropicElastic {
            e,
            nu,
            density,
            expansion,
        })
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
    /// A linear constraint `sum_i a_i u_i = b` over (node, component) pairs (ADR 0296; refused until it lands).
    #[non_exhaustive]
    Linear {
        /// The terms `a_i u_i`.
        terms: Vec<LinearTerm>,
        /// The right-hand side `b`, m.
        value: f64,
    },
    /// The nodes of a node set follow a reference point rigidly, with small rotations (ADR 0296; refused until it lands).
    #[non_exhaustive]
    RigidSet {
        /// Node set name.
        set: String,
        /// The reference point.
        reference: Vec3,
    },
    /// Cyclic symmetry of one sector: the surface set `high` is `low` rotated by `2 pi / sectors` about the axis (ADR
    /// 0297; refused until it lands).
    #[non_exhaustive]
    Cyclic {
        /// Surface set of the low boundary.
        low: String,
        /// Surface set of the high boundary.
        high: String,
        /// Direction of the axis (non-zero).
        axis: Vec3,
        /// A point on the axis.
        origin: Vec3,
        /// Number of sectors of the full rotor, at least 2.
        sectors: usize,
    },
}

/// One term `a u` of a [`Constraint::Linear`]: the coefficient of one component of one node.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct LinearTerm {
    /// The node.
    pub node: usize,
    /// The component.
    pub component: usize,
    /// The coefficient.
    pub coefficient: f64,
}

impl LinearTerm {
    /// A term.
    pub fn new(node: usize, component: usize, coefficient: f64) -> LinearTerm {
        LinearTerm {
            node,
            component,
            coefficient,
        }
    }
}

impl Constraint {
    /// A prescribed displacement component on a node set.
    pub fn displacement(set: &str, component: usize, value: f64) -> Constraint {
        Constraint::Displacement {
            set: set.to_string(),
            component,
            value,
        }
    }
    /// A linear constraint `sum a_i u_i = value`.
    pub fn linear(terms: Vec<LinearTerm>, value: f64) -> Constraint {
        Constraint::Linear { terms, value }
    }
    /// A rigid set of nodes following a reference point.
    pub fn rigid_set(set: &str, reference: Vec3) -> Constraint {
        Constraint::RigidSet {
            set: set.to_string(),
            reference,
        }
    }
    /// The cyclic condition between two boundary surface sets of a sector.
    pub fn cyclic(low: &str, high: &str, axis: Vec3, origin: Vec3, sectors: usize) -> Constraint {
        Constraint::Cyclic {
            low: low.to_string(),
            high: high.to_string(),
            axis,
            origin,
            sectors,
        }
    }
    /// Name of the variant.
    pub fn name(&self) -> &'static str {
        match self {
            Constraint::Displacement { .. } => "Displacement",
            Constraint::Linear { .. } => "Linear",
            Constraint::RigidSet { .. } => "RigidSet",
            Constraint::Cyclic { .. } => "Cyclic",
        }
    }
}

/// An elastic support to ground (ADR 0290 section 2.3; ADR 0296, refused until it lands). Non-exhaustive.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ElasticSupport {
    /// A spring to ground on one component at every node of a node set, N/m per node.
    #[non_exhaustive]
    NodalSpring {
        /// Node set name.
        set: String,
        /// Component.
        component: usize,
        /// Stiffness, positive.
        stiffness: f64,
    },
    /// A foundation on a surface set: a normal stiffness per area, Pa/m.
    #[non_exhaustive]
    Foundation {
        /// Surface set name.
        set: String,
        /// Stiffness per area, positive.
        stiffness: f64,
    },
}

impl ElasticSupport {
    /// A spring per node and component on a node set.
    pub fn nodal_spring(set: &str, component: usize, stiffness: f64) -> ElasticSupport {
        ElasticSupport::NodalSpring {
            set: set.to_string(),
            component,
            stiffness,
        }
    }
    /// A foundation with a normal stiffness per area on a surface set.
    pub fn foundation(set: &str, stiffness: f64) -> ElasticSupport {
        ElasticSupport::Foundation {
            set: set.to_string(),
            stiffness,
        }
    }
    /// Name of the variant.
    pub fn name(&self) -> &'static str {
        match self {
            ElasticSupport::NodalSpring { .. } => "NodalSpring",
            ElasticSupport::Foundation { .. } => "Foundation",
        }
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
    /// The same moment on each node of a node set, N m; every node must carry rotations (ADR 0417 D2).
    #[non_exhaustive]
    NodalMoment {
        /// Node set name.
        set: String,
        /// Moment on each node, about the global axes.
        moment: Vec3,
    },
    /// A uniform force per unit length, N/m, about the global axes, on every element of an element set; every element of
    /// the set must be a Beam2 element (ADR 0422 D1). Its nodal loads are the consistent loads of
    /// [`crate::beam::line_load_vector`].
    #[non_exhaustive]
    LineLoad {
        /// Element set name.
        set: String,
        /// Force per unit length on each element.
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
        Load::NodalForce {
            set: set.to_string(),
            force,
        }
    }
    /// The same moment on each node of a node set (nodes with rotations only).
    pub fn nodal_moment(set: &str, moment: Vec3) -> Load {
        Load::NodalMoment {
            set: set.to_string(),
            moment,
        }
    }
    /// A uniform force per unit length on every element of an element set of Beam2 elements (ADR 0422).
    pub fn line_load(set: &str, force: Vec3) -> Load {
        Load::LineLoad {
            set: set.to_string(),
            force,
        }
    }
    /// A uniform traction on a surface set.
    pub fn traction(set: &str, traction: Vec3) -> Load {
        Load::Traction {
            set: set.to_string(),
            traction,
        }
    }
    /// A uniform pressure on a surface set (positive pressing on the solid).
    pub fn pressure(set: &str, pressure: f64) -> Load {
        Load::Pressure {
            set: set.to_string(),
            pressure,
        }
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
        Err(FemError::InvalidValue {
            key,
            reason: format!("{v:?} is not finite"),
        })
    }
}

fn finite(key: &'static str, v: f64) -> Result<()> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(FemError::InvalidValue {
            key,
            reason: format!("{v} is not finite"),
        })
    }
}

fn positive_finite(key: &'static str, v: f64) -> Result<()> {
    if v > 0.0 && v.is_finite() {
        Ok(())
    } else {
        Err(FemError::InvalidValue {
            key,
            reason: format!("{v} must be positive and finite"),
        })
    }
}

/// A solid model: mesh, materials on element sets, constraints, loads and elastic supports.
#[derive(Debug, Clone, PartialEq)]
pub struct SolidModel {
    mesh: SolidMesh,
    materials: Vec<(String, Material)>,
    constraints: Vec<Constraint>,
    loads: Vec<Load>,
    supports: Vec<ElasticSupport>,
}

impl SolidModel {
    /// A model on a mesh, with no material, constraint or load yet.
    pub fn new(mesh: SolidMesh) -> SolidModel {
        SolidModel {
            mesh,
            materials: Vec::new(),
            constraints: Vec::new(),
            loads: Vec::new(),
            supports: Vec::new(),
        }
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
    /// The elastic supports, in the order added.
    pub fn supports(&self) -> &[ElasticSupport] {
        &self.supports
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
            Err(FemError::UnknownSet {
                kind,
                name: name.to_string(),
            })
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
            Constraint::Linear { terms, value } => {
                if terms.is_empty() {
                    return Err(FemError::InvalidValue {
                        key: "terms",
                        reason: "a linear constraint needs at least one term".into(),
                    });
                }
                let dofs = self.dof_map();
                for t in terms {
                    if t.node >= dofs.n_nodes() {
                        return Err(FemError::MissingNode {
                            role: "linear constraint",
                            node: t.node,
                            n_nodes: dofs.n_nodes(),
                        });
                    }
                    if dofs.equation(t.node, t.component).is_none() {
                        return Err(FemError::MissingComponent {
                            role: "linear constraint",
                            node: t.node,
                            component: t.component,
                            available: dofs.components(t.node),
                        });
                    }
                    finite("coefficient", t.coefficient)?;
                }
                if terms.iter().all(|t| t.coefficient == 0.0) {
                    return Err(FemError::InvalidValue {
                        key: "coefficient",
                        reason: "every coefficient of the linear constraint is zero".into(),
                    });
                }
                finite("value", *value)?;
            }
            Constraint::RigidSet { set, reference } => {
                self.need_set(SetKind::Node, set)?;
                finite_vec("reference", *reference)?;
            }
            Constraint::Cyclic {
                low,
                high,
                axis,
                origin,
                sectors,
            } => {
                self.need_set(SetKind::Surface, low)?;
                self.need_set(SetKind::Surface, high)?;
                finite_vec("axis", *axis)?;
                if axis.iter().all(|&c| c == 0.0) {
                    return Err(FemError::InvalidValue {
                        key: "axis",
                        reason: "the axis is the zero vector".into(),
                    });
                }
                finite_vec("origin", *origin)?;
                if *sectors < 2 {
                    return Err(FemError::InvalidValue {
                        key: "sectors",
                        reason: format!("{sectors} sectors: a cyclic rotor has at least 2"),
                    });
                }
            }
        }
        self.constraints.push(c);
        Ok(())
    }

    /// Adds an elastic support; its set must exist with the right kind, its component exist at every node and its
    /// stiffness be positive and finite.
    pub fn add_support(&mut self, s: ElasticSupport) -> Result<()> {
        match &s {
            ElasticSupport::NodalSpring {
                set,
                component,
                stiffness,
            } => {
                self.need_set(SetKind::Node, set)?;
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
                positive_finite("stiffness", *stiffness)?;
            }
            ElasticSupport::Foundation { set, stiffness } => {
                self.need_set(SetKind::Surface, set)?;
                positive_finite("stiffness", *stiffness)?;
            }
        }
        self.supports.push(s);
        Ok(())
    }

    /// Refuses an element kind or a solid formulation whose work package has not landed, naming it and its block (ADR
    /// 0291 D6), and a Beam2 block, which only the paths of ADR 0417 D4 take, naming [`crate::beam::FOLLOW_UP`]. A model of
    /// the basic kinds with the default formulation passes.
    pub fn check_elements_supported(&self) -> Result<()> {
        self.check_elements(false)
    }

    /// As [`SolidModel::check_elements_supported`] for a path that takes beams (ADR 0417 D4): a Beam2 block passes, and the
    /// model's beams are checked by [`crate::beam::check_beam_model`] (a Beam property on every Beam2 block, no node shared
    /// with a solid element, a frame for every beam element).
    pub fn check_elements_supported_with_beams(&self) -> Result<()> {
        self.check_elements(true)?;
        crate::beam::check_beam_model(self)
    }

    fn check_elements(&self, beams: bool) -> Result<()> {
        for (b, block) in self.mesh.blocks().iter().enumerate() {
            if let Some(package) = block.kind().pending() {
                return Err(FemError::NotYetSupported {
                    item: "element kind",
                    variant: block.kind().name(),
                    location: format!("block {b}"),
                    package,
                });
            }
            let formulation = block.formulation();
            if let Some(package) = formulation.pending() {
                return Err(FemError::NotYetSupported {
                    item: "formulation",
                    variant: formulation.name(),
                    location: format!("block {b}"),
                    package,
                });
            }
            // ADR 0417 D4: a path that does not take beams refuses them.
            if !beams && block.kind() == crate::mesh::ElementKind::Beam2 {
                return Err(FemError::NotYetSupported {
                    item: "element kind",
                    variant: block.kind().name(),
                    location: format!("block {b}"),
                    package: crate::beam::FOLLOW_UP,
                });
            }
        }
        Ok(())
    }

    /// [`SolidModel::check_elements_supported`], then refuses an elastic support whose work package has not landed,
    /// naming it and its set (ADR 0291 D6). Constraints are refused where they are read
    /// ([`crate::constraint::prescribed_equations`]).
    pub fn check_supported(&self) -> Result<()> {
        self.check_elements_supported()?;
        self.check_supports()
    }

    /// As [`SolidModel::check_supported`] for a path that takes beams (ADR 0417 D4).
    pub fn check_supported_with_beams(&self) -> Result<()> {
        self.check_elements_supported_with_beams()?;
        self.check_supports()
    }

    fn check_supports(&self) -> Result<()> {
        for (i, s) in self.supports.iter().enumerate() {
            if let Some(package) = crate::mpc::PENDING {
                let location = match s {
                    ElasticSupport::NodalSpring { set, .. } => format!("support {i}, node set '{set}'"),
                    ElasticSupport::Foundation { set, .. } => format!("support {i}, surface set '{set}'"),
                };
                return Err(FemError::NotYetSupported {
                    item: "support",
                    variant: s.name(),
                    location,
                    package,
                });
            }
        }
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
            Load::NodalMoment { set, moment } => {
                self.need_set(SetKind::Node, set)?;
                finite_vec("moment", *moment)?;
                let dofs = self.dof_map();
                for &node in self.mesh.node_set(set).expect("checked") {
                    if dofs.components(node) < 6 {
                        return Err(FemError::ComponentOutOfRange {
                            set: set.clone(),
                            node,
                            component: 3,
                            available: dofs.components(node),
                        });
                    }
                }
            }
            Load::LineLoad { set, force } => {
                self.need_set(SetKind::Element, set)?;
                finite_vec("force per length", *force)?;
                for &e in self.mesh.element_set(set).expect("checked") {
                    let kind = self.mesh.element_kind(e);
                    if kind != crate::mesh::ElementKind::Beam2 {
                        return Err(FemError::InvalidValue {
                            key: "line load",
                            reason: format!(
                                "element {e} of the set '{set}' is a {} element: a line load acts on Beam2 elements",
                                kind.name()
                            ),
                        });
                    }
                }
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
                    return Err(FemError::InvalidValue {
                        key: "axis",
                        reason: "the axis is the zero vector".into(),
                    });
                }
                finite_vec("origin", *origin)?;
                finite("omega", *omega)?;
            }
            Load::BodyForceNodal { values } => {
                if values.len() != n {
                    return Err(FemError::FieldLength {
                        key: "values",
                        expected: n,
                        found: values.len(),
                    });
                }
                for v in values {
                    finite_vec("values", *v)?;
                }
            }
            Load::Temperature { nodal, reference } => {
                if nodal.len() != n {
                    return Err(FemError::FieldLength {
                        key: "nodal",
                        expected: n,
                        found: nodal.len(),
                    });
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
        SolverStats {
            solver: solver.to_string(),
            unknowns,
            iterations,
            relative_residual,
        }
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
            return Err(FemError::FieldLength {
                key: "strain",
                expected: np,
                found: strain.len(),
            });
        }
        if stress.len() != np {
            return Err(FemError::FieldLength {
                key: "stress",
                expected: np,
                found: stress.len(),
            });
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
