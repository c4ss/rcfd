//! The mesh of a solid model (ADR 0271 D2, D3): nodes, blocks of elements of one kind, named node, element and surface
//! sets, their validation and the geometry the checks and later packages need.
//!
//! Node orders (fixed for all tiers): Hex8 on the reference cube `[-1, 1]^3` with nodes 0 `(-,-,-)`, 1 `(+,-,-)`, 2
//! `(+,+,-)`, 3 `(-,+,-)` and 4 to 7 the same at `zeta = +1`; Tet4 positively oriented (`det(x1 - x0, x2 - x0, x3 - x0) >
//! 0`); Tet10 with the corners of Tet4 and the mid-edge nodes 4 `(0,1)`, 5 `(1,2)`, 6 `(0,2)`, 7 `(0,3)`, 8 `(1,3)`, 9
//! `(2,3)`. Local faces list their nodes counter-clockwise seen from outside, so the right-hand normal points out of the
//! element.

use crate::error::{FemError, Result, SetKind};
use rcfd_core::vec3::{cross, sub, Vec3};
use std::collections::{BTreeMap, BTreeSet};

/// The kind of a finite element. Non-exhaustive: the intermediate tier adds kinds (ADR 0290).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ElementKind {
    /// Linear tetrahedron, 4 nodes.
    Tet4,
    /// Trilinear hexahedron, 8 nodes.
    Hex8,
    /// Quadratic tetrahedron, 10 nodes (4 corners, 6 mid-edge nodes).
    Tet10,
    /// Quadratic (serendipity) hexahedron, 20 nodes: the corners of Hex8, then the mid-edge nodes 8 to 19 on the edges
    /// (0,1), (1,2), (2,3), (3,0), (4,5), (5,6), (6,7), (7,4), (0,4), (1,5), (2,6), (3,7) (the order of VTK type 25;
    /// ADR 0291 D2, confirmed by ADR 0292). Filled by [`crate::hex20`].
    Hex20,
    /// Two-node beam with six unknowns per node. Filled by [`crate::beam`] (ADR 0294).
    Beam2,
    /// Four-node shell with six unknowns per node, nodes counter-clockwise. Filled by [`crate::shell`] (ADR 0295).
    Shell4,
    /// Three-node shell with six unknowns per node, nodes counter-clockwise. Declared by ADR 0440; filled by
    /// [`crate::shell3`] (ADR 0322, F-419).
    Shell3,
}

const HEX_REF: [[f64; 3]; 8] = [
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
    [1.0, 1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, 1.0],
    [-1.0, 1.0, 1.0],
];
const TET_REF: [[f64; 3]; 10] = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
    [0.5, 0.0, 0.0],
    [0.5, 0.5, 0.0],
    [0.0, 0.5, 0.0],
    [0.0, 0.0, 0.5],
    [0.5, 0.0, 0.5],
    [0.0, 0.5, 0.5],
];
/// Local edges of a tetrahedron; edge `k` carries the mid-edge node `4 + k` of a Tet10.
pub const TET_EDGES: [[usize; 2]; 6] = [[0, 1], [1, 2], [0, 2], [0, 3], [1, 3], [2, 3]];
const HEX_EDGES: [[usize; 2]; 12] = [
    [0, 1],
    [1, 2],
    [2, 3],
    [3, 0],
    [4, 5],
    [5, 6],
    [6, 7],
    [7, 4],
    [0, 4],
    [1, 5],
    [2, 6],
    [3, 7],
];
const HEX_FACES: [&[usize]; 6] = [
    &[0, 3, 2, 1],
    &[4, 5, 6, 7],
    &[0, 1, 5, 4],
    &[1, 2, 6, 5],
    &[2, 3, 7, 6],
    &[3, 0, 4, 7],
];
const QUAD_EDGES: [[usize; 2]; 4] = [[0, 1], [1, 2], [2, 3], [3, 0]];
const TRI_EDGES: [[usize; 2]; 3] = [[0, 1], [1, 2], [2, 0]];
const LINE_EDGES: [[usize; 2]; 1] = [[0, 1]];
const TET4_FACES: [&[usize]; 4] = [&[0, 2, 1], &[0, 1, 3], &[1, 2, 3], &[0, 3, 2]];
const TET10_FACES: [&[usize]; 4] = [
    &[0, 2, 1, 6, 5, 4],
    &[0, 1, 3, 4, 8, 7],
    &[1, 2, 3, 5, 9, 8],
    &[0, 3, 2, 7, 9, 6],
];

impl ElementKind {
    /// Name of the kind.
    pub fn name(self) -> &'static str {
        match self {
            ElementKind::Tet4 => "Tet4",
            ElementKind::Hex8 => "Hex8",
            ElementKind::Tet10 => "Tet10",
            ElementKind::Hex20 => "Hex20",
            ElementKind::Beam2 => "Beam2",
            ElementKind::Shell4 => "Shell4",
            ElementKind::Shell3 => "Shell3",
        }
    }

    /// Whether the kind is a solid (a volume element with three translations per node): Tet4, Hex8, Tet10, Hex20.
    pub fn is_solid(self) -> bool {
        match self {
            ElementKind::Tet4 | ElementKind::Hex8 | ElementKind::Tet10 | ElementKind::Hex20 => true,
            ElementKind::Beam2 | ElementKind::Shell4 | ElementKind::Shell3 => false,
        }
    }

    /// The work package that adds the kind, while it has not landed (`None` for an available kind; ADR 0291 D2).
    pub fn pending(self) -> Option<&'static str> {
        match self {
            ElementKind::Tet4 | ElementKind::Hex8 | ElementKind::Tet10 => None,
            ElementKind::Hex20 => crate::hex20::PENDING,
            ElementKind::Beam2 => crate::beam::PENDING,
            ElementKind::Shell4 => crate::shell::PENDING,
            ElementKind::Shell3 => crate::shell3::PENDING,
        }
    }

    /// Number of nodes of one element.
    pub fn nodes_per_element(self) -> usize {
        match self {
            ElementKind::Tet4 => 4,
            ElementKind::Hex8 => 8,
            ElementKind::Tet10 => 10,
            ElementKind::Hex20 => 20,
            ElementKind::Beam2 => 2,
            ElementKind::Shell4 => 4,
            ElementKind::Shell3 => 3,
        }
    }

    /// Number of corner nodes (the first nodes of the element).
    pub fn corners(self) -> usize {
        match self {
            ElementKind::Tet4 | ElementKind::Tet10 | ElementKind::Shell4 => 4,
            ElementKind::Hex8 | ElementKind::Hex20 => 8,
            ElementKind::Beam2 => 2,
            ElementKind::Shell3 => 3,
        }
    }

    /// Number of local faces.
    pub fn n_faces(self) -> usize {
        self.faces().len()
    }

    /// The local faces: local node lists, counter-clockwise seen from outside (corners first, then mid-edge nodes).
    pub fn faces(self) -> &'static [&'static [usize]] {
        match self {
            ElementKind::Tet4 => &TET4_FACES,
            ElementKind::Hex8 => &HEX_FACES,
            ElementKind::Tet10 => &TET10_FACES,
            ElementKind::Hex20 => crate::hex20::faces(),
            ElementKind::Beam2 => crate::beam::faces(),
            ElementKind::Shell4 => crate::shell::faces(),
            ElementKind::Shell3 => crate::shell3::faces(),
        }
    }

    /// Number of corner nodes of each face (3 for the tetrahedra, 4 for the hexahedra, 0 for the structural kinds).
    pub fn face_corners(self) -> usize {
        match self {
            ElementKind::Tet4 | ElementKind::Tet10 => 3,
            ElementKind::Hex8 | ElementKind::Hex20 => 4,
            ElementKind::Beam2 | ElementKind::Shell4 | ElementKind::Shell3 => 0,
        }
    }

    /// The edges between corner nodes, as local node pairs.
    pub fn edges(self) -> &'static [[usize; 2]] {
        match self {
            ElementKind::Tet4 | ElementKind::Tet10 => &TET_EDGES,
            ElementKind::Hex8 | ElementKind::Hex20 => &HEX_EDGES,
            ElementKind::Beam2 => &LINE_EDGES,
            ElementKind::Shell4 => &QUAD_EDGES,
            ElementKind::Shell3 => &TRI_EDGES,
        }
    }

    /// Reference coordinates of the nodes.
    pub fn reference_nodes(self) -> &'static [[f64; 3]] {
        match self {
            ElementKind::Tet4 => &TET_REF[..4],
            ElementKind::Hex8 => &HEX_REF,
            ElementKind::Tet10 => &TET_REF,
            ElementKind::Hex20 => crate::hex20::reference_nodes(),
            ElementKind::Beam2 => crate::beam::reference_nodes(),
            ElementKind::Shell4 => crate::shell::reference_nodes(),
            ElementKind::Shell3 => crate::shell3::reference_nodes(),
        }
    }

    /// Derivatives of the shape functions with respect to the reference coordinates at `xi`.
    pub fn shape_gradients(self, xi: [f64; 3]) -> Vec<[f64; 3]> {
        match self {
            ElementKind::Hex8 => HEX_REF
                .iter()
                .map(|a| {
                    let f = [1.0 + xi[0] * a[0], 1.0 + xi[1] * a[1], 1.0 + xi[2] * a[2]];
                    [
                        a[0] * f[1] * f[2] / 8.0,
                        f[0] * a[1] * f[2] / 8.0,
                        f[0] * f[1] * a[2] / 8.0,
                    ]
                })
                .collect(),
            ElementKind::Tet4 => vec![[-1.0, -1.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            ElementKind::Tet10 => {
                let l = [1.0 - xi[0] - xi[1] - xi[2], xi[0], xi[1], xi[2]];
                let dl = [[-1.0, -1.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
                let mut g = Vec::with_capacity(10);
                for i in 0..4 {
                    let s = 4.0 * l[i] - 1.0;
                    g.push([s * dl[i][0], s * dl[i][1], s * dl[i][2]]);
                }
                for [i, j] in TET_EDGES {
                    g.push([
                        4.0 * (l[i] * dl[j][0] + l[j] * dl[i][0]),
                        4.0 * (l[i] * dl[j][1] + l[j] * dl[i][1]),
                        4.0 * (l[i] * dl[j][2] + l[j] * dl[i][2]),
                    ]);
                }
                g
            }
            ElementKind::Hex20 => crate::hex20::shape_gradients(xi),
            ElementKind::Beam2 => crate::beam::shape_gradients(xi),
            ElementKind::Shell4 => crate::shell::shape_gradients(xi),
            ElementKind::Shell3 => crate::shell3::shape_gradients(xi),
        }
    }
}

/// The formulation of a solid element block (ADR 0290 section 2.3). `Full` is the element of the basic tier; `BBar` and
/// `Incompatible` are Hex8 formulations added by ADR 0293 ([`crate::hex8x`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SolidFormulation {
    /// Full integration with the kind's stiffness rule (the default).
    #[default]
    Full,
    /// Mean-dilatation (B-bar) Hex8.
    BBar,
    /// Hex8 with incompatible modes.
    Incompatible,
}

impl SolidFormulation {
    /// Name of the formulation.
    pub fn name(self) -> &'static str {
        match self {
            SolidFormulation::Full => "Full",
            SolidFormulation::BBar => "BBar",
            SolidFormulation::Incompatible => "Incompatible",
        }
    }

    /// The work package that adds the formulation, while it has not landed (`None` for `Full`).
    pub fn pending(self) -> Option<&'static str> {
        match self {
            SolidFormulation::Full => None,
            SolidFormulation::BBar | SolidFormulation::Incompatible => crate::hex8x::PENDING,
        }
    }
}

/// The section of a beam block (ADR 0294): area, second moments about the section axes y and z, torsion constant and
/// shear factors. Every value positive and finite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeamSection {
    area: f64,
    i_y: f64,
    i_z: f64,
    torsion: f64,
    shear_y: f64,
    shear_z: f64,
}

fn positive(key: &'static str, v: f64) -> Result<()> {
    if v > 0.0 && v.is_finite() {
        Ok(())
    } else {
        Err(FemError::InvalidValue {
            key,
            reason: format!("{v} must be positive and finite"),
        })
    }
}

impl BeamSection {
    /// A section: area (m^2), `I_y`, `I_z` (m^4), torsion constant `J` (m^4), shear factors `kappa_y`, `kappa_z`.
    /// Refuses (naming the key) a value that is not positive and finite.
    pub fn new(area: f64, i_y: f64, i_z: f64, torsion: f64, shear_y: f64, shear_z: f64) -> Result<BeamSection> {
        positive("area", area)?;
        positive("i_y", i_y)?;
        positive("i_z", i_z)?;
        positive("torsion", torsion)?;
        positive("shear_y", shear_y)?;
        positive("shear_z", shear_z)?;
        Ok(BeamSection {
            area,
            i_y,
            i_z,
            torsion,
            shear_y,
            shear_z,
        })
    }
    /// Area.
    pub fn area(&self) -> f64 {
        self.area
    }
    /// Second moment about the section axis y.
    pub fn i_y(&self) -> f64 {
        self.i_y
    }
    /// Second moment about the section axis z.
    pub fn i_z(&self) -> f64 {
        self.i_z
    }
    /// Torsion constant.
    pub fn torsion(&self) -> f64 {
        self.torsion
    }
    /// Shear factor for shear along y.
    pub fn shear_y(&self) -> f64 {
        self.shear_y
    }
    /// Shear factor for shear along z.
    pub fn shear_z(&self) -> f64 {
        self.shear_z
    }
}

/// The property of an element block (ADR 0290 section 2.3): plain data, set by [`SolidMesh::set_block_property`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum BlockProperty {
    /// A solid block and its formulation.
    #[non_exhaustive]
    Solid {
        /// The formulation.
        formulation: SolidFormulation,
    },
    /// A beam block: its section and the orientation vector that fixes the section axis y (with the beam axis).
    #[non_exhaustive]
    Beam {
        /// The section.
        section: BeamSection,
        /// A vector not parallel to the beam axis; the section axis y lies in their plane.
        orientation: Vec3,
    },
    /// A shell block and its thickness.
    #[non_exhaustive]
    Shell {
        /// Thickness, m.
        thickness: f64,
        /// The membrane of a Shell3 block (ADR 0466); a Shell4 block takes only the default.
        membrane: crate::shell3::Shell3Membrane,
        /// The membrane of a Shell4 block (ADR 0470); a Shell3 block takes only the default.
        shell4_membrane: crate::shell::Shell4Membrane,
    },
    /// A laminated shell block: its plies from the bottom (`-e_3` side) to the top (ADR 0440 D5). Refused by every solve
    /// until [`crate::laminate`] lands (ADR 0331, F-428).
    #[non_exhaustive]
    Layup {
        /// The plies, bottom first.
        plies: Vec<Ply>,
    },
}

/// One ply of a [`BlockProperty::Layup`]: its material, thickness and the angle of its first material axis from the
/// element's first axis about the normal (ADR 0440 D5). Plain data.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Ply {
    /// The material of the ply.
    pub material: crate::model::Material,
    /// Thickness, m, positive.
    pub thickness: f64,
    /// Angle about the normal, rad.
    pub angle: f64,
}

impl Ply {
    /// A ply; refuses a thickness that is not positive and finite and a non-finite angle.
    pub fn new(material: crate::model::Material, thickness: f64, angle: f64) -> Result<Ply> {
        positive("thickness", thickness)?;
        if !angle.is_finite() {
            return Err(FemError::InvalidValue {
                key: "angle",
                reason: format!("{angle} must be finite"),
            });
        }
        Ok(Ply {
            material,
            thickness,
            angle,
        })
    }
}

impl BlockProperty {
    /// A solid block with a formulation.
    pub fn solid(formulation: SolidFormulation) -> BlockProperty {
        BlockProperty::Solid { formulation }
    }
    /// A beam block; refuses a non-finite or zero orientation.
    pub fn beam(section: BeamSection, orientation: Vec3) -> Result<BlockProperty> {
        if !orientation.iter().all(|c| c.is_finite()) || orientation.iter().all(|&c| c == 0.0) {
            return Err(FemError::InvalidValue {
                key: "orientation",
                reason: format!("{orientation:?} must be finite and non-zero"),
            });
        }
        Ok(BlockProperty::Beam { section, orientation })
    }
    /// A shell block; refuses a thickness that is not positive and finite.
    pub fn shell(thickness: f64) -> Result<BlockProperty> {
        BlockProperty::shell_with_membrane(thickness, crate::shell3::Shell3Membrane::ConstantStrain)
    }
    /// A shell block with a membrane for Shell3 (ADR 0466); refuses a non-positive or non-finite thickness.
    pub fn shell_with_membrane(thickness: f64, membrane: crate::shell3::Shell3Membrane) -> Result<BlockProperty> {
        positive("thickness", thickness)?;
        Ok(BlockProperty::Shell {
            thickness,
            membrane,
            shell4_membrane: crate::shell::Shell4Membrane::Bilinear,
        })
    }
    /// A shell block with a membrane for Shell4 (ADR 0470); refuses a non-positive or non-finite thickness.
    pub fn shell4_with_membrane(thickness: f64, shell4_membrane: crate::shell::Shell4Membrane) -> Result<BlockProperty> {
        positive("thickness", thickness)?;
        Ok(BlockProperty::Shell {
            thickness,
            membrane: crate::shell3::Shell3Membrane::ConstantStrain,
            shell4_membrane,
        })
    }
    /// A laminated shell block; refuses an empty list of plies.
    pub fn layup(plies: Vec<Ply>) -> Result<BlockProperty> {
        if plies.is_empty() {
            return Err(FemError::InvalidValue {
                key: "plies",
                reason: "a layup needs at least one ply".to_string(),
            });
        }
        Ok(BlockProperty::Layup { plies })
    }
    /// Name of the property.
    pub fn name(&self) -> &'static str {
        match self {
            BlockProperty::Solid { .. } => "Solid",
            BlockProperty::Beam { .. } => "Beam",
            BlockProperty::Shell { .. } => "Shell",
            BlockProperty::Layup { .. } => "Layup",
        }
    }
}

/// A block of elements of one kind with a flat connectivity in the element's node order.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementBlock {
    kind: ElementKind,
    connectivity: Vec<usize>,
    property: Option<BlockProperty>,
}

impl ElementBlock {
    /// Element kind of the block.
    pub fn kind(&self) -> ElementKind {
        self.kind
    }
    /// Flat connectivity, `nodes_per_element` entries per element.
    pub fn connectivity(&self) -> &[usize] {
        &self.connectivity
    }
    /// Number of elements of the block.
    pub fn len(&self) -> usize {
        self.connectivity.len() / self.kind.nodes_per_element()
    }
    /// Whether the block has no element (never true for a block of a validated mesh).
    pub fn is_empty(&self) -> bool {
        self.connectivity.is_empty()
    }
    /// The property set by [`SolidMesh::set_block_property`] (`None`: the default of the kind, `Full` for a solid).
    pub fn property(&self) -> Option<&BlockProperty> {
        self.property.as_ref()
    }
    /// The solid formulation of the block (`Full` unless a solid property says otherwise).
    pub fn formulation(&self) -> SolidFormulation {
        match &self.property {
            Some(BlockProperty::Solid { formulation }) => *formulation,
            _ => SolidFormulation::Full,
        }
    }
}

/// A local face of an element: global element index and local face index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FaceRef {
    /// Global element index (elements are numbered across blocks in block order).
    pub element: usize,
    /// Local face index of the element's kind.
    pub face: usize,
}

/// The finite-element mesh of a solid model. Built through [`SolidMesh::new`] and the `add_*` methods, each of which
/// validates what it adds.
#[derive(Debug, Clone, PartialEq)]
pub struct SolidMesh {
    nodes: Vec<Vec3>,
    blocks: Vec<ElementBlock>,
    block_start: Vec<usize>,
    n_elements: usize,
    node_sets: BTreeMap<String, Vec<usize>>,
    element_sets: BTreeMap<String, Vec<usize>>,
    surface_sets: BTreeMap<String, Vec<FaceRef>>,
}

fn det3(c0: Vec3, c1: Vec3, c2: Vec3) -> f64 {
    c0[0] * (c1[1] * c2[2] - c1[2] * c2[1]) - c1[0] * (c0[1] * c2[2] - c0[2] * c2[1])
        + c2[0] * (c0[1] * c1[2] - c0[2] * c1[1])
}

const G: f64 = 0.577_350_269_189_625_8; // 1/sqrt(3)

// The 5-point rule exact to degree 3 on the tetrahedron (ADR 0271 D3, arithmetic 2): (point, weight as a fraction of the
// volume).
const TET_CUBIC_RULE: [([f64; 3], f64); 5] = [
    ([0.25, 0.25, 0.25], -0.8),
    ([1.0 / 6.0, 1.0 / 6.0, 1.0 / 6.0], 0.45),
    ([0.5, 1.0 / 6.0, 1.0 / 6.0], 0.45),
    ([1.0 / 6.0, 0.5, 1.0 / 6.0], 0.45),
    ([1.0 / 6.0, 1.0 / 6.0, 0.5], 0.45),
];

impl SolidMesh {
    /// A mesh with the given nodes and no element. Refuses a non-finite coordinate.
    pub fn new(nodes: Vec<Vec3>) -> Result<SolidMesh> {
        if let Some(node) = nodes.iter().position(|p| p.iter().any(|c| !c.is_finite())) {
            return Err(FemError::NonFiniteNode { node });
        }
        Ok(SolidMesh {
            nodes,
            blocks: Vec::new(),
            block_start: Vec::new(),
            n_elements: 0,
            node_sets: BTreeMap::new(),
            element_sets: BTreeMap::new(),
            surface_sets: BTreeMap::new(),
        })
    }

    /// Adds a block of elements of one kind; returns the block index. Each element is checked: nodes in range, no node
    /// twice, `det J > 0` at every node.
    pub fn add_block(&mut self, kind: ElementKind, connectivity: Vec<usize>) -> Result<usize> {
        let block = self.blocks.len();
        let npe = kind.nodes_per_element();
        if connectivity.is_empty() {
            return Err(FemError::EmptyBlock { block });
        }
        if connectivity.len() % npe != 0 {
            return Err(FemError::ConnectivityLength {
                block,
                len: connectivity.len(),
                nodes_per_element: npe,
            });
        }
        for (local, conn) in connectivity.chunks(npe).enumerate() {
            let element = self.n_elements + local;
            for &node in conn {
                if node >= self.nodes.len() {
                    return Err(FemError::NodeOutOfRange {
                        element,
                        node,
                        n_nodes: self.nodes.len(),
                    });
                }
            }
            for (i, &a) in conn.iter().enumerate() {
                if conn[..i].contains(&a) {
                    return Err(FemError::RepeatedNode { element, node: a });
                }
            }
            // A structural kind (Beam2, Shell4) has no volume map: its element checks come with its package.
            if !kind.is_solid() {
                continue;
            }
            for (local_node, &jacobian) in self.jacobians_of(kind, conn).iter().enumerate() {
                if jacobian.is_nan() || jacobian <= 0.0 {
                    return Err(FemError::InvertedElement {
                        element,
                        local_node,
                        jacobian,
                    });
                }
            }
        }
        let n = connectivity.len() / npe;
        self.block_start.push(self.n_elements);
        self.blocks.push(ElementBlock {
            kind,
            connectivity,
            property: None,
        });
        self.n_elements += n;
        Ok(block)
    }

    /// Sets the property of a block (ADR 0291 D3). Refuses a block that does not exist and a property that does not fit
    /// the kind: `Solid` on a non-solid kind, `Beam` on any kind but Beam2, `Shell` and `Layup` on any kind but Shell4 and
    /// Shell3.
    pub fn set_block_property(&mut self, block: usize, property: BlockProperty) -> Result<()> {
        let n_blocks = self.blocks.len();
        let Some(b) = self.blocks.get_mut(block) else {
            return Err(FemError::InvalidValue {
                key: "block",
                reason: format!("block {block} does not exist (the mesh has {n_blocks} blocks)"),
            });
        };
        let (fits, operation) = match &property {
            BlockProperty::Solid { .. } => (b.kind.is_solid(), "the property Solid"),
            BlockProperty::Beam { .. } => (b.kind == ElementKind::Beam2, "the property Beam"),
            // ADR 0470 D2: a Shell4 membrane other than the bilinear one is a Shell4 choice.
            BlockProperty::Shell {
                membrane,
                shell4_membrane,
                ..
            } if *shell4_membrane != crate::shell::Shell4Membrane::Bilinear => (
                b.kind == ElementKind::Shell4 && *membrane == crate::shell3::Shell3Membrane::ConstantStrain,
                "a Shell4 membrane",
            ),
            BlockProperty::Shell { membrane, .. } => match membrane {
                // ADR 0466 D2: a drilling membrane is a Shell3 choice.
                crate::shell3::Shell3Membrane::ConstantStrain => (
                    matches!(b.kind, ElementKind::Shell4 | ElementKind::Shell3),
                    "the property Shell",
                ),
                _ => (b.kind == ElementKind::Shell3, "a Shell3 membrane"),
            },
            BlockProperty::Layup { .. } => (
                matches!(b.kind, ElementKind::Shell4 | ElementKind::Shell3),
                "the property Layup",
            ),
        };
        if !fits {
            return Err(FemError::UnsupportedKind {
                block,
                kind: b.kind.name(),
                operation,
            });
        }
        b.property = Some(property);
        Ok(())
    }

    fn check_name(kind: SetKind, name: &str, exists: bool) -> Result<()> {
        if name.is_empty() {
            return Err(FemError::EmptySetName { kind });
        }
        if exists {
            return Err(FemError::DuplicateSet {
                kind,
                name: name.to_string(),
            });
        }
        Ok(())
    }

    fn check_members(kind: SetKind, name: &str, members: &[usize], bound: usize) -> Result<()> {
        if members.is_empty() {
            return Err(FemError::EmptySet {
                kind,
                name: name.to_string(),
            });
        }
        let mut seen = BTreeSet::new();
        for &m in members {
            if m >= bound {
                return Err(FemError::SetMemberOutOfRange {
                    kind,
                    name: name.to_string(),
                    member: m,
                });
            }
            if !seen.insert(m) {
                return Err(FemError::DuplicateSetMember {
                    kind,
                    name: name.to_string(),
                    member: m,
                });
            }
        }
        Ok(())
    }

    /// Adds a named node set.
    pub fn add_node_set(&mut self, name: &str, nodes: Vec<usize>) -> Result<()> {
        Self::check_name(SetKind::Node, name, self.node_sets.contains_key(name))?;
        Self::check_members(SetKind::Node, name, &nodes, self.nodes.len())?;
        self.node_sets.insert(name.to_string(), nodes);
        Ok(())
    }

    /// Adds a named element set (global element indices).
    pub fn add_element_set(&mut self, name: &str, elements: Vec<usize>) -> Result<()> {
        Self::check_name(SetKind::Element, name, self.element_sets.contains_key(name))?;
        Self::check_members(SetKind::Element, name, &elements, self.n_elements)?;
        self.element_sets.insert(name.to_string(), elements);
        Ok(())
    }

    /// Adds a named surface set (element faces, normals out of the solid).
    pub fn add_surface_set(&mut self, name: &str, faces: Vec<FaceRef>) -> Result<()> {
        let kind = SetKind::Surface;
        Self::check_name(kind, name, self.surface_sets.contains_key(name))?;
        if faces.is_empty() {
            return Err(FemError::EmptySet {
                kind,
                name: name.to_string(),
            });
        }
        let mut seen = BTreeSet::new();
        for f in &faces {
            if f.element >= self.n_elements {
                return Err(FemError::SetMemberOutOfRange {
                    kind,
                    name: name.to_string(),
                    member: f.element,
                });
            }
            if f.face >= self.element_kind(f.element).n_faces() {
                return Err(FemError::FaceOutOfRange {
                    set: name.to_string(),
                    element: f.element,
                    face: f.face,
                });
            }
            if !seen.insert(*f) {
                return Err(FemError::DuplicateSetMember {
                    kind,
                    name: name.to_string(),
                    member: f.element,
                });
            }
        }
        self.surface_sets.insert(name.to_string(), faces);
        Ok(())
    }

    /// The nodes.
    pub fn nodes(&self) -> &[Vec3] {
        &self.nodes
    }
    /// The blocks.
    pub fn blocks(&self) -> &[ElementBlock] {
        &self.blocks
    }
    /// Number of elements over all blocks.
    pub fn n_elements(&self) -> usize {
        self.n_elements
    }

    fn locate(&self, element: usize) -> (usize, usize) {
        assert!(element < self.n_elements, "element {element} does not exist");
        let b = self.block_start.partition_point(|&s| s <= element) - 1;
        (b, element - self.block_start[b])
    }

    /// Block index of an element.
    pub fn block_of(&self, element: usize) -> usize {
        self.locate(element).0
    }
    /// Kind of an element. Panics if the element does not exist.
    pub fn element_kind(&self, element: usize) -> ElementKind {
        self.blocks[self.locate(element).0].kind
    }
    /// Nodes of an element in its node order. Panics if the element does not exist.
    pub fn element_nodes(&self, element: usize) -> &[usize] {
        let (b, l) = self.locate(element);
        let npe = self.blocks[b].kind.nodes_per_element();
        &self.blocks[b].connectivity[l * npe..(l + 1) * npe]
    }

    /// A node set by name.
    pub fn node_set(&self, name: &str) -> Option<&[usize]> {
        self.node_sets.get(name).map(|v| v.as_slice())
    }
    /// An element set by name.
    pub fn element_set(&self, name: &str) -> Option<&[usize]> {
        self.element_sets.get(name).map(|v| v.as_slice())
    }
    /// A surface set by name.
    pub fn surface_set(&self, name: &str) -> Option<&[FaceRef]> {
        self.surface_sets.get(name).map(|v| v.as_slice())
    }
    /// Names of the node sets, sorted.
    pub fn node_set_names(&self) -> impl Iterator<Item = &str> {
        self.node_sets.keys().map(|s| s.as_str())
    }
    /// Names of the element sets, sorted.
    pub fn element_set_names(&self) -> impl Iterator<Item = &str> {
        self.element_sets.keys().map(|s| s.as_str())
    }
    /// Names of the surface sets, sorted.
    pub fn surface_set_names(&self) -> impl Iterator<Item = &str> {
        self.surface_sets.keys().map(|s| s.as_str())
    }

    fn jacobian_at(&self, kind: ElementKind, conn: &[usize], xi: [f64; 3]) -> f64 {
        let g = kind.shape_gradients(xi);
        let mut c = [[0.0; 3]; 3]; // c[j] = dx/dxi_j
        for (a, &n) in conn.iter().enumerate() {
            let x = self.nodes[n];
            for j in 0..3 {
                for i in 0..3 {
                    c[j][i] += x[i] * g[a][j];
                }
            }
        }
        det3(c[0], c[1], c[2])
    }

    fn jacobians_of(&self, kind: ElementKind, conn: &[usize]) -> Vec<f64> {
        kind.reference_nodes()
            .iter()
            .map(|&xi| self.jacobian_at(kind, conn, xi))
            .collect()
    }

    /// `det J` at every node of an element, in its node order.
    pub fn jacobians_at_nodes(&self, element: usize) -> Vec<f64> {
        self.jacobians_of(self.element_kind(element), self.element_nodes(element))
    }

    /// Volume of an element: exact for Tet4, for Hex8 (2 x 2 x 2 Gauss) and for Tet10 (a rule exact to degree 3); NaN for
    /// a Hex20 until ADR 0292 and for the structural kinds.
    pub fn element_volume(&self, element: usize) -> f64 {
        let kind = self.element_kind(element);
        let conn = self.element_nodes(element);
        match kind {
            ElementKind::Tet4 => self.jacobian_at(kind, conn, [0.25; 3]) / 6.0,
            ElementKind::Hex8 => {
                let mut v = 0.0;
                for &z in &[-G, G] {
                    for &y in &[-G, G] {
                        for &x in &[-G, G] {
                            v += self.jacobian_at(kind, conn, [x, y, z]);
                        }
                    }
                }
                v
            }
            ElementKind::Tet10 => {
                let mut v = 0.0;
                for (xi, w) in TET_CUBIC_RULE {
                    v += w * self.jacobian_at(kind, conn, xi);
                }
                v / 6.0
            }
            ElementKind::Hex20 => crate::hex20::volume(&|xi| self.jacobian_at(kind, conn, xi)),
            // A structural kind has no volume without its section or thickness: undefined until its package.
            ElementKind::Beam2 | ElementKind::Shell4 | ElementKind::Shell3 => f64::NAN,
        }
    }

    /// Global nodes of a face, in the local face order.
    pub fn face_nodes(&self, f: FaceRef) -> Vec<usize> {
        let conn = self.element_nodes(f.element);
        self.element_kind(f.element).faces()[f.face]
            .iter()
            .map(|&l| conn[l])
            .collect()
    }

    // Quadrature points of the face map: weights and tangent vectors (x_u, x_v).
    fn face_tangents(&self, f: FaceRef) -> Vec<(f64, Vec3, Vec3)> {
        let x: Vec<Vec3> = self.face_nodes(f).iter().map(|&n| self.nodes[n]).collect();
        match x.len() {
            3 => vec![(0.5, sub(x[1], x[0]), sub(x[2], x[0]))],
            4 => {
                // Bilinear on [-1, 1]^2, nodes (-,-), (+,-), (+,+), (-,+).
                let r = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
                let mut out = Vec::with_capacity(4);
                for &v in &[-G, G] {
                    for &u in &[-G, G] {
                        let mut xu = [0.0; 3];
                        let mut xv = [0.0; 3];
                        for a in 0..4 {
                            let du = r[a][0] * (1.0 + v * r[a][1]) / 4.0;
                            let dv = (1.0 + u * r[a][0]) * r[a][1] / 4.0;
                            for i in 0..3 {
                                xu[i] += x[a][i] * du;
                                xv[i] += x[a][i] * dv;
                            }
                        }
                        out.push((1.0, xu, xv));
                    }
                }
                out
            }
            6 => {
                // Quadratic triangle: corners c0, c1, c2, mid-edge nodes (c0 c1), (c1 c2), (c2 c0); u = L1, v = L2.
                let pts = [[1.0 / 6.0, 1.0 / 6.0], [2.0 / 3.0, 1.0 / 6.0], [1.0 / 6.0, 2.0 / 3.0]];
                let mut out = Vec::with_capacity(3);
                for [u, v] in pts {
                    let l = [1.0 - u - v, u, v];
                    let du = [-1.0, 1.0, 0.0];
                    let dv = [-1.0, 0.0, 1.0];
                    let mut gu = [0.0; 6];
                    let mut gv = [0.0; 6];
                    for i in 0..3 {
                        gu[i] = (4.0 * l[i] - 1.0) * du[i];
                        gv[i] = (4.0 * l[i] - 1.0) * dv[i];
                    }
                    for (k, [i, j]) in [[0usize, 1usize], [1, 2], [2, 0]].into_iter().enumerate() {
                        gu[3 + k] = 4.0 * (l[i] * du[j] + l[j] * du[i]);
                        gv[3 + k] = 4.0 * (l[i] * dv[j] + l[j] * dv[i]);
                    }
                    let mut xu = [0.0; 3];
                    let mut xv = [0.0; 3];
                    for a in 0..6 {
                        for i in 0..3 {
                            xu[i] += x[a][i] * gu[a];
                            xv[i] += x[a][i] * gv[a];
                        }
                    }
                    out.push((1.0 / 6.0, xu, xv));
                }
                out
            }
            8 => {
                // The 8-node face of Hex20 on [-1, 1]^2 with its 3 x 3 Gauss rule, as hex20::face_area (ADR 0383).
                let mut out = Vec::with_capacity(9);
                for (uv, w) in crate::hex20::face_rule() {
                    let (_, d) = crate::hex20::face_shape_functions(uv);
                    let mut xu = [0.0; 3];
                    let mut xv = [0.0; 3];
                    for (xa, da) in x.iter().zip(&d) {
                        for i in 0..3 {
                            xu[i] += xa[i] * da[0];
                            xv[i] += xa[i] * da[1];
                        }
                    }
                    out.push((w, xu, xv));
                }
                out
            }
            n => unreachable!("a face of {n} nodes"),
        }
    }

    /// Area of a face (exact for planar Tri3, planar parallelogram Quad4 and straight-sided Tri6 faces; an 8-node Hex20 face
    /// by its 3 x 3 Gauss rule, exact for a parallelogram with mid-edge nodes at the midpoints, ADR 0383).
    pub fn face_area(&self, f: FaceRef) -> f64 {
        self.face_tangents(f)
            .into_iter()
            .map(|(w, xu, xv)| w * rcfd_core::vec3::norm(cross(xu, xv)))
            .sum()
    }

    /// Area vector of a face, the integral of `x_u x x_v`: its direction is the outward normal of the element.
    pub fn face_area_vector(&self, f: FaceRef) -> Vec3 {
        let mut a = [0.0; 3];
        for (w, xu, xv) in self.face_tangents(f) {
            let c = cross(xu, xv);
            for i in 0..3 {
                a[i] += w * c[i];
            }
        }
        a
    }

    /// Nodes of the faces of a surface set, sorted and unique.
    pub fn surface_set_nodes(&self, name: &str) -> Result<Vec<usize>> {
        let faces = self.surface_set(name).ok_or_else(|| FemError::UnknownSet {
            kind: SetKind::Surface,
            name: name.to_string(),
        })?;
        let mut s = BTreeSet::new();
        for &f in faces {
            s.extend(self.face_nodes(f));
        }
        Ok(s.into_iter().collect())
    }

    /// Every face of every element, keyed by its sorted corner nodes. A conforming mesh has one member for a boundary face
    /// and two for an interior face.
    pub fn face_incidence(&self) -> BTreeMap<Vec<usize>, Vec<FaceRef>> {
        let mut map: BTreeMap<Vec<usize>, Vec<FaceRef>> = BTreeMap::new();
        for element in 0..self.n_elements {
            let kind = self.element_kind(element);
            for face in 0..kind.n_faces() {
                let f = FaceRef { element, face };
                let mut key = self.face_nodes(f);
                key.truncate(kind.face_corners());
                key.sort_unstable();
                map.entry(key).or_default().push(f);
            }
        }
        map
    }
}
