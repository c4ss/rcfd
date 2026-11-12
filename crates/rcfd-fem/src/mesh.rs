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
const HEX_EDGES: [[usize; 2]; 12] =
    [[0, 1], [1, 2], [2, 3], [3, 0], [4, 5], [5, 6], [6, 7], [7, 4], [0, 4], [1, 5], [2, 6], [3, 7]];
const HEX_FACES: [&[usize]; 6] = [&[0, 3, 2, 1], &[4, 5, 6, 7], &[0, 1, 5, 4], &[1, 2, 6, 5], &[2, 3, 7, 6], &[3, 0, 4, 7]];
const TET4_FACES: [&[usize]; 4] = [&[0, 2, 1], &[0, 1, 3], &[1, 2, 3], &[0, 3, 2]];
const TET10_FACES: [&[usize]; 4] = [&[0, 2, 1, 6, 5, 4], &[0, 1, 3, 4, 8, 7], &[1, 2, 3, 5, 9, 8], &[0, 3, 2, 7, 9, 6]];

impl ElementKind {
    /// Name of the kind.
    pub fn name(self) -> &'static str {
        match self {
            ElementKind::Tet4 => "Tet4",
            ElementKind::Hex8 => "Hex8",
            ElementKind::Tet10 => "Tet10",
        }
    }

    /// Number of nodes of one element.
    pub fn nodes_per_element(self) -> usize {
        match self {
            ElementKind::Tet4 => 4,
            ElementKind::Hex8 => 8,
            ElementKind::Tet10 => 10,
        }
    }

    /// Number of corner nodes (the first nodes of the element).
    pub fn corners(self) -> usize {
        match self {
            ElementKind::Tet4 | ElementKind::Tet10 => 4,
            ElementKind::Hex8 => 8,
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
        }
    }

    /// Number of corner nodes of each face (3 for the tetrahedra, 4 for the hexahedron).
    pub fn face_corners(self) -> usize {
        match self {
            ElementKind::Tet4 | ElementKind::Tet10 => 3,
            ElementKind::Hex8 => 4,
        }
    }

    /// The edges between corner nodes, as local node pairs.
    pub fn edges(self) -> &'static [[usize; 2]] {
        match self {
            ElementKind::Tet4 | ElementKind::Tet10 => &TET_EDGES,
            ElementKind::Hex8 => &HEX_EDGES,
        }
    }

    /// Reference coordinates of the nodes.
    pub fn reference_nodes(self) -> &'static [[f64; 3]] {
        match self {
            ElementKind::Tet4 => &TET_REF[..4],
            ElementKind::Hex8 => &HEX_REF,
            ElementKind::Tet10 => &TET_REF,
        }
    }

    /// Derivatives of the shape functions with respect to the reference coordinates at `xi`.
    pub fn shape_gradients(self, xi: [f64; 3]) -> Vec<[f64; 3]> {
        match self {
            ElementKind::Hex8 => HEX_REF
                .iter()
                .map(|a| {
                    let f = [1.0 + xi[0] * a[0], 1.0 + xi[1] * a[1], 1.0 + xi[2] * a[2]];
                    [a[0] * f[1] * f[2] / 8.0, f[0] * a[1] * f[2] / 8.0, f[0] * f[1] * a[2] / 8.0]
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
        }
    }
}

/// A block of elements of one kind with a flat connectivity in the element's node order.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementBlock {
    kind: ElementKind,
    connectivity: Vec<usize>,
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
    c0[0] * (c1[1] * c2[2] - c1[2] * c2[1]) - c1[0] * (c0[1] * c2[2] - c0[2] * c2[1]) + c2[0] * (c0[1] * c1[2] - c0[2] * c1[1])
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
            return Err(FemError::ConnectivityLength { block, len: connectivity.len(), nodes_per_element: npe });
        }
        for (local, conn) in connectivity.chunks(npe).enumerate() {
            let element = self.n_elements + local;
            for &node in conn {
                if node >= self.nodes.len() {
                    return Err(FemError::NodeOutOfRange { element, node, n_nodes: self.nodes.len() });
                }
            }
            for (i, &a) in conn.iter().enumerate() {
                if conn[..i].contains(&a) {
                    return Err(FemError::RepeatedNode { element, node: a });
                }
            }
            for (local_node, &jacobian) in self.jacobians_of(kind, conn).iter().enumerate() {
                if !(jacobian > 0.0) {
                    return Err(FemError::InvertedElement { element, local_node, jacobian });
                }
            }
        }
        let n = connectivity.len() / npe;
        self.block_start.push(self.n_elements);
        self.blocks.push(ElementBlock { kind, connectivity });
        self.n_elements += n;
        Ok(block)
    }

    fn check_name(kind: SetKind, name: &str, exists: bool) -> Result<()> {
        if name.is_empty() {
            return Err(FemError::EmptySetName { kind });
        }
        if exists {
            return Err(FemError::DuplicateSet { kind, name: name.to_string() });
        }
        Ok(())
    }

    fn check_members(kind: SetKind, name: &str, members: &[usize], bound: usize) -> Result<()> {
        if members.is_empty() {
            return Err(FemError::EmptySet { kind, name: name.to_string() });
        }
        let mut seen = BTreeSet::new();
        for &m in members {
            if m >= bound {
                return Err(FemError::SetMemberOutOfRange { kind, name: name.to_string(), member: m });
            }
            if !seen.insert(m) {
                return Err(FemError::DuplicateSetMember { kind, name: name.to_string(), member: m });
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
            return Err(FemError::EmptySet { kind, name: name.to_string() });
        }
        let mut seen = BTreeSet::new();
        for f in &faces {
            if f.element >= self.n_elements {
                return Err(FemError::SetMemberOutOfRange { kind, name: name.to_string(), member: f.element });
            }
            if f.face >= self.element_kind(f.element).n_faces() {
                return Err(FemError::FaceOutOfRange { set: name.to_string(), element: f.element, face: f.face });
            }
            if !seen.insert(*f) {
                return Err(FemError::DuplicateSetMember { kind, name: name.to_string(), member: f.element });
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
        kind.reference_nodes().iter().map(|&xi| self.jacobian_at(kind, conn, xi)).collect()
    }

    /// `det J` at every node of an element, in its node order.
    pub fn jacobians_at_nodes(&self, element: usize) -> Vec<f64> {
        self.jacobians_of(self.element_kind(element), self.element_nodes(element))
    }

    /// Volume of an element: exact for Tet4, for Hex8 (2 x 2 x 2 Gauss) and for Tet10 (a rule exact to degree 3).
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
        }
    }

    /// Global nodes of a face, in the local face order.
    pub fn face_nodes(&self, f: FaceRef) -> Vec<usize> {
        let conn = self.element_nodes(f.element);
        self.element_kind(f.element).faces()[f.face].iter().map(|&l| conn[l]).collect()
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
            n => unreachable!("a face of {n} nodes"),
        }
    }

    /// Area of a face (exact for planar Tri3, planar parallelogram Quad4 and straight-sided Tri6 faces).
    pub fn face_area(&self, f: FaceRef) -> f64 {
        self.face_tangents(f).into_iter().map(|(w, xu, xv)| w * rcfd_core::vec3::norm(cross(xu, xv))).sum()
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
        let faces = self
            .surface_set(name)
            .ok_or_else(|| FemError::UnknownSet { kind: SetKind::Surface, name: name.to_string() })?;
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
