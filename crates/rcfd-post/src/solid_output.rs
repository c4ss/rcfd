//! VTK output of a solid result (F-377, ADR 0278): the finite-element `.vtu` writer of `rcfd_io::vtk` fed with a
//! [`SolidResult`] of `rcfd_fem`.
//!
//! Point arrays: `displacement` (3), the nodal-averaged `stress` (6, Voigt xx yy zz xy yz zx), `von_mises` (1) and
//! `principal_stress` (3, descending) of that averaged tensor, `principal_direction_1`, `_2`, `_3` (3 each), and `reaction`
//! (3, when the result carries reactions). Cell array: `stress` (6), the volume mean over each element's integration points.
//! The stresses come from `rcfd_fem::recover` (ADR 0277); a node of no element gets NaN in the nodal stress arrays. The node
//! orders of Tet4, Hex8 and Tet10 are VTK's (ADR 0278 D2, from the public VTK class documentation); the writer still maps
//! through [`vtk_from_fem`] so that a kind with a different order is mapped in one place.

use rcfd_core::{Error, Result};
use rcfd_fem::mesh::ElementKind;
use rcfd_fem::recover::{nodal_average, principal, von_mises, IntegrationPointField};
use rcfd_fem::{FemError, SolidModel, SolidResult};
use rcfd_io::vtk::{fe_vtu_string, FeArray, FeCellBlock, VTK_HEXAHEDRON, VTK_QUADRATIC_TETRA, VTK_TETRA};
use std::path::Path;

const IDENTITY_4: [usize; 4] = [0, 1, 2, 3];
const IDENTITY_8: [usize; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
const IDENTITY_10: [usize; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

/// The VTK cell type of an element kind and its node permutation: VTK local node `a` is rcfd-fem local node `perm[a]`.
/// Refuses a kind without a VTK mapping (named).
pub fn vtk_from_fem(kind: ElementKind) -> Result<(u8, &'static [usize])> {
    match kind {
        ElementKind::Tet4 => Ok((VTK_TETRA, &IDENTITY_4)),
        ElementKind::Hex8 => Ok((VTK_HEXAHEDRON, &IDENTITY_8)),
        ElementKind::Tet10 => Ok((VTK_QUADRATIC_TETRA, &IDENTITY_10)),
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

/// The `.vtu` text of a solid result (see the module documentation for the arrays).
pub fn solid_vtu_string(model: &SolidModel, result: &SolidResult) -> Result<String> {
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
