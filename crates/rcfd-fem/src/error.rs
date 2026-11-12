//! The error type of the crate: every refusal names the element, node, block, set or key it concerns (ADR 0270 section
//! 2.2, ADR 0271 D6).

use std::fmt;

/// Result alias of the crate.
pub type Result<T> = std::result::Result<T, FemError>;

/// The kind of a named set of a [`crate::SolidMesh`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SetKind {
    /// A set of nodes.
    Node,
    /// A set of elements.
    Element,
    /// A set of element faces.
    Surface,
}

impl fmt::Display for SetKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SetKind::Node => "node set",
            SetKind::Element => "element set",
            SetKind::Surface => "surface set",
        })
    }
}

/// A refusal of the data model, located.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum FemError {
    /// A node coordinate is not finite.
    NonFiniteNode {
        /// Node index.
        node: usize,
    },
    /// A block with no element.
    EmptyBlock {
        /// Block index.
        block: usize,
    },
    /// The connectivity length is not a multiple of the node count of the kind.
    ConnectivityLength {
        /// Block index.
        block: usize,
        /// Length given.
        len: usize,
        /// Nodes per element of the block's kind.
        nodes_per_element: usize,
    },
    /// An element refers to a node that does not exist.
    NodeOutOfRange {
        /// Global element index.
        element: usize,
        /// The node index given.
        node: usize,
        /// Number of nodes of the mesh.
        n_nodes: usize,
    },
    /// A node appears twice in one element.
    RepeatedNode {
        /// Global element index.
        element: usize,
        /// The repeated node.
        node: usize,
    },
    /// The Jacobian determinant is not positive at a node of the element.
    InvertedElement {
        /// Global element index.
        element: usize,
        /// Local node at which `det J <= 0` (the first one found).
        local_node: usize,
        /// The value of `det J` there.
        jacobian: f64,
    },
    /// A set name is empty.
    EmptySetName {
        /// Kind of set.
        kind: SetKind,
    },
    /// A set of this name and kind exists already.
    DuplicateSet {
        /// Kind of set.
        kind: SetKind,
        /// Name.
        name: String,
    },
    /// A set has no member.
    EmptySet {
        /// Kind of set.
        kind: SetKind,
        /// Name.
        name: String,
    },
    /// A set member (node or element index) does not exist.
    SetMemberOutOfRange {
        /// Kind of set.
        kind: SetKind,
        /// Name.
        name: String,
        /// The member given.
        member: usize,
    },
    /// A set lists a member twice.
    DuplicateSetMember {
        /// Kind of set.
        kind: SetKind,
        /// Name.
        name: String,
        /// The repeated member (for a surface set, the element).
        member: usize,
    },
    /// A surface set names a local face the element does not have.
    FaceOutOfRange {
        /// Surface set name.
        set: String,
        /// Global element index.
        element: usize,
        /// The local face given.
        face: usize,
    },
    /// A set referred to by name does not exist.
    UnknownSet {
        /// Kind of set expected.
        kind: SetKind,
        /// Name.
        name: String,
    },
    /// A material value is outside its range.
    InvalidMaterial {
        /// The key (`e`, `nu`, `density`, `expansion`).
        key: &'static str,
        /// The value given.
        value: f64,
        /// Why it is refused.
        reason: &'static str,
    },
    /// An element has no material.
    MaterialUnassigned {
        /// Global element index.
        element: usize,
    },
    /// An element is in two element sets that carry a material.
    MaterialOverlap {
        /// Global element index.
        element: usize,
        /// The first set.
        first: String,
        /// The second set.
        second: String,
    },
    /// A constraint names a component that a node of its set does not have.
    ComponentOutOfRange {
        /// Node set name.
        set: String,
        /// The node.
        node: usize,
        /// The component given.
        component: usize,
        /// Components the node has.
        available: usize,
    },
    /// A value of a load, constraint, builder or result is refused.
    InvalidValue {
        /// The key.
        key: &'static str,
        /// Why it is refused.
        reason: String,
    },
    /// A field has the wrong length.
    FieldLength {
        /// The key.
        key: &'static str,
        /// Expected length.
        expected: usize,
        /// Length given.
        found: usize,
    },
    /// An operation does not support the element kind of a block.
    UnsupportedKind {
        /// Block index.
        block: usize,
        /// The kind's name.
        kind: &'static str,
        /// The operation.
        operation: &'static str,
    },
    /// An error of the element routines, located at the element by the assembly (ADR 0275 D1, D7).
    Element {
        /// Global element index.
        element: usize,
        /// The error of the element routine.
        source: Box<FemError>,
    },
    /// A per-node constraint or load names a node that does not exist (ADR 0275 D3, D4).
    MissingNode {
        /// What names the node (`constraint`, `nodal force`).
        role: &'static str,
        /// The node given.
        node: usize,
        /// Number of nodes of the mesh.
        n_nodes: usize,
    },
    /// A per-node constraint or load names a component the node does not have.
    MissingComponent {
        /// What names the component.
        role: &'static str,
        /// The node.
        node: usize,
        /// The component given.
        component: usize,
        /// Components the node has.
        available: usize,
    },
    /// One unknown is prescribed twice with different values.
    ConflictingConstraint {
        /// The node.
        node: usize,
        /// The component.
        component: usize,
        /// The first value.
        first: f64,
        /// The second value.
        second: f64,
    },
    /// The constraints leave a rigid-body motion of a connected part free (ADR 0275 D5).
    RigidBodyMotionFree {
        /// The smallest node of the part.
        part_node: usize,
        /// Number of nodes of the part.
        part_nodes: usize,
        /// Number of free rigid-body motions.
        count: usize,
        /// One free motion, described.
        motion: String,
    },
    /// A node of no element has a component that is not prescribed.
    FreeNode {
        /// The node.
        node: usize,
        /// The component.
        component: usize,
    },
    /// The reduced stiffness has a pivot that fails the test of the static solve (ADR 0275 D6).
    SingularStiffness {
        /// Node of the unknown.
        node: usize,
        /// Component of the unknown.
        component: usize,
        /// The pivot.
        pivot: f64,
        /// The diagonal of the reduced matrix at that unknown.
        diagonal: f64,
    },
    /// A linear solver failed.
    Solver {
        /// The solver.
        solver: &'static str,
        /// Why.
        reason: String,
    },
    /// A load variant the static solve does not support yet.
    UnsupportedLoad {
        /// The variant.
        load: &'static str,
        /// The work package that adds it.
        package: &'static str,
    },
}

impl fmt::Display for FemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use FemError::*;
        match self {
            NonFiniteNode { node } => write!(f, "node {node}: coordinate is not finite"),
            EmptyBlock { block } => write!(f, "block {block}: has no element"),
            ConnectivityLength {
                block,
                len,
                nodes_per_element,
            } => write!(
                f,
                "block {block}: connectivity length {len} is not a multiple of {nodes_per_element} nodes per element"
            ),
            NodeOutOfRange { element, node, n_nodes } => {
                write!(
                    f,
                    "element {element}: node {node} does not exist (the mesh has {n_nodes} nodes)"
                )
            }
            RepeatedNode { element, node } => write!(f, "element {element}: node {node} appears twice"),
            InvertedElement {
                element,
                local_node,
                jacobian,
            } => {
                write!(
                    f,
                    "element {element}: inverted (det J = {jacobian:e} at local node {local_node})"
                )
            }
            EmptySetName { kind } => write!(f, "{kind} with an empty name"),
            DuplicateSet { kind, name } => write!(f, "{kind} '{name}' is defined twice"),
            EmptySet { kind, name } => write!(f, "{kind} '{name}' is empty"),
            SetMemberOutOfRange { kind, name, member } => {
                write!(f, "{kind} '{name}': member {member} does not exist")
            }
            DuplicateSetMember { kind, name, member } => write!(f, "{kind} '{name}': member {member} is listed twice"),
            FaceOutOfRange { set, element, face } => {
                write!(f, "surface set '{set}': element {element} has no local face {face}")
            }
            UnknownSet { kind, name } => write!(f, "{kind} '{name}' does not exist"),
            InvalidMaterial { key, value, reason } => write!(f, "material key '{key}' = {value}: {reason}"),
            MaterialUnassigned { element } => write!(f, "element {element}: has no material"),
            MaterialOverlap { element, first, second } => {
                write!(
                    f,
                    "element {element}: has a material from element set '{first}' and from '{second}'"
                )
            }
            ComponentOutOfRange {
                set,
                node,
                component,
                available,
            } => write!(
                f,
                "node set '{set}': node {node} has {available} components, component {component} does not exist"
            ),
            InvalidValue { key, reason } => write!(f, "key '{key}': {reason}"),
            FieldLength { key, expected, found } => {
                write!(f, "key '{key}': length {found}, expected {expected}")
            }
            UnsupportedKind { block, kind, operation } => {
                write!(f, "block {block}: element kind {kind} is not supported by {operation}")
            }
            Element { element, source } => write!(f, "element {element}: {source}"),
            MissingNode { role, node, n_nodes } => {
                write!(
                    f,
                    "{role} on node {node}: the node does not exist (the mesh has {n_nodes} nodes)"
                )
            }
            MissingComponent {
                role,
                node,
                component,
                available,
            } => write!(
                f,
                "{role} on node {node} component {component}: the node has {available} components"
            ),
            ConflictingConstraint {
                node,
                component,
                first,
                second,
            } => write!(
                f,
                "node {node} component {component} is prescribed twice with different values ({first:e} and {second:e})"
            ),
            RigidBodyMotionFree {
                part_node,
                part_nodes,
                count,
                motion,
            } => write!(
                f,
                "the model leaves a rigid-body motion free: the part containing node {part_node} ({part_nodes} nodes) \
                 has {count} unconstrained rigid-body motion(s), e.g. {motion}; prescribe displacements that hold it"
            ),
            FreeNode { node, component } => write!(
                f,
                "node {node} belongs to no element and its component {component} is not prescribed: a free motion"
            ),
            SingularStiffness {
                node,
                component,
                pivot,
                diagonal,
            } => write!(
                f,
                "the reduced stiffness is singular at node {node} component {component} (pivot {pivot:e}, {:e} of its \
                 diagonal {diagonal:e}): a mechanism or rigid-body motion is free, or the model is too ill-conditioned \
                 for the direct solver",
                pivot / diagonal
            ),
            Solver { solver, reason } => write!(f, "solver {solver}: {reason}"),
            UnsupportedLoad { load, package } => write!(
                f,
                "load {load} is not supported by the static solve yet (it is added by {package})"
            ),
        }
    }
}

impl std::error::Error for FemError {}

impl From<FemError> for rcfd_core::Error {
    fn from(e: FemError) -> Self {
        match &e {
            FemError::InvalidMaterial { key, .. }
            | FemError::InvalidValue { key, .. }
            | FemError::FieldLength { key, .. } => rcfd_core::Error::InvalidValue {
                path: (*key).to_string(),
                reason: e.to_string(),
            },
            _ => rcfd_core::Error::Mesh(e.to_string()),
        }
    }
}
