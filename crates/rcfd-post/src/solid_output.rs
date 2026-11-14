//! VTK output of a solid result (F-377, ADR 0278): the finite-element `.vtu` writer of `rcfd_io::vtk` fed with a
//! [`SolidResult`] of `rcfd_fem`.
//!
//! Point arrays: `displacement` (3), the nodal-averaged `stress` (6, Voigt xx yy zz xy yz zx), `von_mises` (1) and
//! `principal_stress` (3, descending) of that averaged tensor, `principal_direction_1`, `_2`, `_3` (3 each), and `reaction`
//! (3, when the result carries reactions). Cell array: `stress` (6), the volume mean over each element's integration points.
//! The stresses come from `rcfd_fem::recover` (ADR 0277); a node of no element gets NaN in the nodal stress arrays. The node
//! orders of Tet4, Hex8 and Tet10 are VTK's (ADR 0278 D2, from the public VTK class documentation); the writer still maps
//! through [`vtk_from_fem`] so that a kind with a different order is mapped in one place.
//!
//! A model with Beam2 blocks (F-512, ADR 0423) is written by [`beam_fields`]: point arrays `displacement` (3), `rotation`
//! (3, the small rotations about x, y, z; NaN at a node with three unknowns), and, when the result carries reactions,
//! `reaction` (3) and `reaction_moment` (3, NaN at a node with three unknowns); every element a `VTK_LINE` cell; cell arrays
//! `section_force_start` and `section_force_end` (6 each: `N, V_y, V_z, T, M_y, M_z` in the element's local axes, from
//! `rcfd_fem::beam::section_forces`). A model mixing Beam2 with other kinds is refused. A model without Beam2 is written as
//! before, bit for bit.

use rcfd_core::{Error, Result};
use rcfd_fem::beam::section_forces;
use rcfd_fem::mesh::ElementKind;
use rcfd_fem::recover::{nodal_average, principal, von_mises, IntegrationPointField};
use rcfd_fem::{FemError, SolidModel, SolidResult};
use rcfd_io::vtk::{
    fe_vtu_string, FeArray, FeCellBlock, VTK_HEXAHEDRON, VTK_LINE, VTK_QUAD, VTK_QUADRATIC_HEXAHEDRON,
    VTK_QUADRATIC_TETRA, VTK_TETRA,
};
use std::path::Path;

const IDENTITY_4: [usize; 4] = [0, 1, 2, 3];
const IDENTITY_8: [usize; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
const IDENTITY_10: [usize; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
const IDENTITY_2: [usize; 2] = [0, 1];
const IDENTITY_20: [usize; 20] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19];

/// The VTK cell type of an element kind and its node permutation: VTK local node `a` is rcfd-fem local node `perm[a]`.
/// Refuses a kind without a VTK mapping (named).
pub fn vtk_from_fem(kind: ElementKind) -> Result<(u8, &'static [usize])> {
    match kind {
        ElementKind::Tet4 => Ok((VTK_TETRA, &IDENTITY_4)),
        ElementKind::Hex8 => Ok((VTK_HEXAHEDRON, &IDENTITY_8)),
        ElementKind::Tet10 => Ok((VTK_QUADRATIC_TETRA, &IDENTITY_10)),
        // The node orders of the intermediate tier are VTK's (ADR 0291 D2, D7).
        ElementKind::Hex20 => Ok((VTK_QUADRATIC_HEXAHEDRON, &IDENTITY_20)),
        ElementKind::Beam2 => Ok((VTK_LINE, &IDENTITY_2)),
        ElementKind::Shell4 => Ok((VTK_QUAD, &IDENTITY_4)),
        other => Err(Error::InvalidValue {
            path: "element kind".into(),
            reason: format!("no VTK cell mapping for element kind {}", other.name()),
        }),
    }
}

/// The arrays written for a solid result, flat and row-major (exposed so that a test can compare the file's bits).
#[derive(Debug, Clone, PartialEq)]
pub struct SolidFields {
    /// 3 per node.
    pub displacement: Vec<f64>,
    /// 6 per node: the volume-weighted nodal average of the stress (NaN for a node of no element).
    pub stress: Vec<f64>,
    /// 1 per node: von Mises of the nodal stress.
    pub von_mises: Vec<f64>,
    /// 3 per node: principal values of the nodal stress, descending.
    pub principal_stress: Vec<f64>,
    /// 9 per node: the directions of the three principal values, one 3-vector each.
    pub principal_directions: Vec<f64>,
    /// 3 per node, when the result carries reactions.
    pub reaction: Option<Vec<f64>>,
    /// 6 per element: the element mean of the stress.
    pub element_stress: Vec<f64>,
}

fn length_error(key: &'static str, expected: usize, found: usize) -> Error {
    FemError::FieldLength { key, expected, found }.into()
}

/// The arrays of a solid result. Refuses a result whose unknowns do not match the model (`displacement` named), a node
/// with other than 3 components, and a nodal average across a material interface (ADR 0277).
pub fn solid_fields(model: &SolidModel, result: &SolidResult) -> Result<SolidFields> {
    let mesh = model.mesh();
    let n = mesh.nodes().len();
    let dofs = model.dof_map();
    if result.dofs().n_nodes() != n || result.displacement_values().len() != dofs.n_equations() {
        return Err(length_error(
            "displacement",
            dofs.n_equations(),
            result.displacement_values().len(),
        ));
    }
    for node in 0..n {
        if result.dofs().components(node) != 3 {
            return Err(Error::InvalidValue {
                path: "displacement".into(),
                reason: format!(
                    "node {node} has {} components; the solid output writes 3",
                    result.dofs().components(node)
                ),
            });
        }
    }
    let mut displacement = Vec::with_capacity(3 * n);
    for node in 0..n {
        displacement.extend_from_slice(result.displacement(node));
    }
    let reaction = match result.reactions() {
        None => None,
        Some(r) => {
            let mut v = Vec::with_capacity(3 * n);
            for node in 0..n {
                let start = result.dofs().equation(node, 0).expect("node has 3 components");
                v.extend_from_slice(&r[start..start + 3]);
            }
            Some(v)
        }
    };
    let field = IntegrationPointField::from_result(model, result)?;
    let nodal = nodal_average(model, &field, field.stress(), None)?;
    let mut stress = Vec::with_capacity(6 * n);
    let mut vm = Vec::with_capacity(n);
    let mut ps = Vec::with_capacity(3 * n);
    let mut pd = Vec::with_capacity(9 * n);
    for s in &nodal {
        match s {
            Some(s) => {
                stress.extend_from_slice(s);
                vm.push(von_mises(s));
                let p = principal(s);
                ps.extend_from_slice(&p.values);
                for d in &p.directions {
                    pd.extend_from_slice(d);
                }
            }
            None => {
                stress.extend_from_slice(&[f64::NAN; 6]);
                vm.push(f64::NAN);
                ps.extend_from_slice(&[f64::NAN; 3]);
                pd.extend_from_slice(&[f64::NAN; 9]);
            }
        }
    }
    let element_stress = field.element_means(field.stress())?.into_iter().flatten().collect();
    Ok(SolidFields {
        displacement,
        stress,
        von_mises: vm,
        principal_stress: ps,
        principal_directions: pd,
        reaction,
        element_stress,
    })
}

/// The arrays written for the result of a model with Beam2 elements, flat and row-major (ADR 0423 D2).
#[derive(Debug, Clone, PartialEq)]
pub struct BeamFields {
    /// 3 per node: the translations.
    pub displacement: Vec<f64>,
    /// 3 per node: the small rotations about x, y, z (NaN at a node with three unknowns).
    pub rotation: Vec<f64>,
    /// 3 per node, when the result carries reactions: the reaction forces.
    pub reaction: Option<Vec<f64>>,
    /// 3 per node, when the result carries reactions: the reaction moments (NaN at a node with three unknowns).
    pub reaction_moment: Option<Vec<f64>>,
    /// 6 per element: the section forces `N, V_y, V_z, T, M_y, M_z` in local axes just after node 0.
    pub section_force_start: Vec<f64>,
    /// 6 per element: the same just before node 1.
    pub section_force_end: Vec<f64>,
}

fn has_beam(model: &SolidModel) -> bool {
    model.mesh().blocks().iter().any(|b| b.kind() == ElementKind::Beam2)
}

/// The arrays of the result of a beam model. Refuses a model without Beam2 or with a block of another kind (named), and a
/// result whose numbering of the unknowns is not the model's (`displacement` named).
pub fn beam_fields(model: &SolidModel, result: &SolidResult) -> Result<BeamFields> {
    let mesh = model.mesh();
    if !has_beam(model) {
        return Err(Error::InvalidValue {
            path: "element kind".into(),
            reason: "the model has no Beam2 block; use solid_fields".into(),
        });
    }
    if let Some((b, block)) = mesh
        .blocks()
        .iter()
        .enumerate()
        .find(|(_, b)| b.kind() != ElementKind::Beam2)
    {
        return Err(Error::InvalidValue {
            path: "element kind".into(),
            reason: format!(
                "block {b} is {} in a model with Beam2 blocks; the output of a model mixing Beam2 with other kinds is \
                 a follow-up of ADR 0423",
                block.kind().name()
            ),
        });
    }
    let dofs = model.dof_map();
    if result.displacement_values().len() != dofs.n_equations() {
        return Err(length_error(
            "displacement",
            dofs.n_equations(),
            result.displacement_values().len(),
        ));
    }
    // The model's number of unknowns in another layout (ADR 0430): name the first node and both counts.
    let rd = result.dofs();
    if rd.n_nodes() != dofs.n_nodes() {
        return Err(Error::InvalidValue {
            path: "displacement".into(),
            reason: format!(
                "the result numbers {} nodes and the model {}; the unknowns of the result are laid out for another model",
                rd.n_nodes(),
                dofs.n_nodes()
            ),
        });
    }
    if let Some(k) = (0..dofs.n_nodes()).find(|&k| rd.components(k) != dofs.components(k)) {
        return Err(Error::InvalidValue {
            path: "displacement".into(),
            reason: format!(
                "node {k} has {} unknowns in the result and {} in the model; the result has the model's number of unknowns ({}) in another layout",
                rd.components(k),
                dofs.components(k),
                dofs.n_equations()
            ),
        });
    }
    let n = mesh.nodes().len();
    let mut displacement = Vec::with_capacity(3 * n);
    let mut rotation = Vec::with_capacity(3 * n);
    for node in 0..n {
        let u = result.displacement(node);
        displacement.extend_from_slice(&u[0..3]);
        if u.len() == 6 {
            rotation.extend_from_slice(&u[3..6]);
        } else {
            rotation.extend_from_slice(&[f64::NAN; 3]);
        }
    }
    let (reaction, reaction_moment) = match result.reactions() {
        None => (None, None),
        Some(r) => {
            let mut f = Vec::with_capacity(3 * n);
            let mut m = Vec::with_capacity(3 * n);
            for node in 0..n {
                let start = dofs.equation(node, 0).expect("a node has at least 3 components");
                f.extend_from_slice(&r[start..start + 3]);
                if dofs.components(node) == 6 {
                    m.extend_from_slice(&r[start + 3..start + 6]);
                } else {
                    m.extend_from_slice(&[f64::NAN; 3]);
                }
            }
            (Some(f), Some(m))
        }
    };
    let sf = section_forces(model, result.displacement_values())?;
    let mut section_force_start = Vec::with_capacity(6 * sf.len());
    let mut section_force_end = Vec::with_capacity(6 * sf.len());
    for (_, s) in &sf {
        section_force_start.extend_from_slice(&s.start);
        section_force_end.extend_from_slice(&s.end);
    }
    Ok(BeamFields {
        displacement,
        rotation,
        reaction,
        reaction_moment,
        section_force_start,
        section_force_end,
    })
}

/// The connectivity of every block in VTK order, with its cell type.
fn vtk_connectivity(model: &SolidModel) -> Result<Vec<(u8, Vec<usize>)>> {
    let mesh = model.mesh();
    let mut conns: Vec<(u8, Vec<usize>)> = Vec::with_capacity(mesh.blocks().len());
    for block in mesh.blocks() {
        let (ty, perm) = vtk_from_fem(block.kind())?;
        let npe = block.kind().nodes_per_element();
        let mut c = Vec::with_capacity(block.connectivity().len());
        for element in block.connectivity().chunks(npe) {
            c.extend(perm.iter().map(|&a| element[a]));
        }
        conns.push((ty, c));
    }
    Ok(conns)
}

/// The `.vtu` text of the result of a beam model (ADR 0423 D3).
fn beam_vtu_string(model: &SolidModel, result: &SolidResult) -> Result<String> {
    let f = beam_fields(model, result)?;
    let conns = vtk_connectivity(model)?;
    let blocks: Vec<FeCellBlock> = conns
        .iter()
        .map(|(ty, c)| FeCellBlock {
            cell_type: *ty,
            connectivity: c,
        })
        .collect();
    let arr = |name, components, values| FeArray {
        name,
        components,
        values,
    };
    let mut point = vec![
        arr("displacement", 3, &f.displacement[..]),
        arr("rotation", 3, &f.rotation[..]),
    ];
    if let (Some(r), Some(m)) = (&f.reaction, &f.reaction_moment) {
        point.push(arr("reaction", 3, &r[..]));
        point.push(arr("reaction_moment", 3, &m[..]));
    }
    let cell = [
        arr("section_force_start", 6, &f.section_force_start[..]),
        arr("section_force_end", 6, &f.section_force_end[..]),
    ];
    fe_vtu_string(model.mesh().nodes(), &blocks, &point, &cell)
}

/// The `.vtu` text of a solid result (see the module documentation for the arrays). A model with Beam2 blocks is written by
/// the beam arrays (ADR 0423); a model without is written as before.
pub fn solid_vtu_string(model: &SolidModel, result: &SolidResult) -> Result<String> {
    if has_beam(model) {
        return beam_vtu_string(model, result);
    }
    let f = solid_fields(model, result)?;
    let mesh = model.mesh();
    let mut conns: Vec<(u8, Vec<usize>)> = Vec::with_capacity(mesh.blocks().len());
    for block in mesh.blocks() {
        let (ty, perm) = vtk_from_fem(block.kind())?;
        let npe = block.kind().nodes_per_element();
        let mut c = Vec::with_capacity(block.connectivity().len());
        for element in block.connectivity().chunks(npe) {
            c.extend(perm.iter().map(|&a| element[a]));
        }
        conns.push((ty, c));
    }
    let blocks: Vec<FeCellBlock> = conns
        .iter()
        .map(|(ty, c)| FeCellBlock {
            cell_type: *ty,
            connectivity: c,
        })
        .collect();
    let n = mesh.nodes().len();
    let arr = |name, components, values| FeArray {
        name,
        components,
        values,
    };
    let mut point = vec![
        arr("displacement", 3, &f.displacement[..]),
        arr("stress", 6, &f.stress[..]),
        arr("von_mises", 1, &f.von_mises[..]),
        arr("principal_stress", 3, &f.principal_stress[..]),
    ];
    // The directions are stored per node as d1 d2 d3; each array takes one of them.
    let dirs: Vec<Vec<f64>> = (0..3)
        .map(|k| {
            (0..n)
                .flat_map(|node| f.principal_directions[9 * node + 3 * k..9 * node + 3 * k + 3].to_vec())
                .collect()
        })
        .collect();
    point.push(arr("principal_direction_1", 3, &dirs[0][..]));
    point.push(arr("principal_direction_2", 3, &dirs[1][..]));
    point.push(arr("principal_direction_3", 3, &dirs[2][..]));
    if let Some(r) = &f.reaction {
        point.push(arr("reaction", 3, &r[..]));
    }
    let cell = [arr("stress", 6, &f.element_stress[..])];
    fe_vtu_string(mesh.nodes(), &blocks, &point, &cell)
}

/// Write a solid result as a `.vtu` file.
pub fn write_solid_vtu(path: impl AsRef<Path>, model: &SolidModel, result: &SolidResult) -> Result<()> {
    std::fs::write(path, solid_vtu_string(model, result)?).map_err(|e| Error::Io(e.to_string()))
}
