//! Assembly of the global matrices of a solid model (ADR 0275 D1, D2). Declared by ADR 0271; filled by ADR 0275.
//!
//! The pattern couples every unknown of a node with every unknown of each node it shares an element with; columns are
//! sorted within rows because [`DofMap`] numbers the unknowns node by node. Element matrices are computed in batches by
//! worker threads (`std::thread::scope`) and scattered sequentially in element order, so every entry is the sum of the
//! same terms in the same order for any thread count: the result is bit-identical for 1 and for 8 threads.

use crate::element::{element_for, ElementMatrix};
use crate::error::{FemError, Result};
use crate::mesh::{ElementKind, SolidMesh};
use crate::model::{DofMap, Material, SolidModel};
use rcfd_core::vec3::Vec3;
use rcfd_linalg::CsrMatrix;

/// Elements per batch of the parallel computation of element matrices.
const BATCH: usize = 1024;

/// What an element-matrix function is given: the element, its kind, the coordinates of its nodes and its material.
#[derive(Debug, Clone, Copy)]
pub struct ElementContext<'a> {
    /// Global element index.
    pub element: usize,
    /// Element kind.
    pub kind: ElementKind,
    /// Node indices of the element, in its node order.
    pub nodes: &'a [usize],
    /// Coordinates of its nodes, in its node order.
    pub coords: &'a [Vec3],
    /// Its material.
    pub material: &'a Material,
}

/// The equations of the unknowns of the given nodes, node-major (all components of the first node, then the second...).
pub fn element_dofs(dofs: &DofMap, nodes: &[usize]) -> Vec<usize> {
    let mut out = Vec::with_capacity(3 * nodes.len());
    for &n in nodes {
        for c in 0..dofs.components(n) {
            out.push(dofs.equation(n, c).expect("component of an existing node"));
        }
    }
    out
}

/// The sparsity pattern of the assembled matrices of a mesh (all values zero). A node of no element has empty rows.
pub fn pattern(mesh: &SolidMesh, dofs: &DofMap) -> CsrMatrix {
    let n_nodes = mesh.nodes().len();
    let mut adjacent: Vec<Vec<usize>> = vec![Vec::new(); n_nodes];
    for e in 0..mesh.n_elements() {
        let nodes = mesh.element_nodes(e);
        for &a in nodes {
            adjacent[a].extend_from_slice(nodes);
        }
    }
    for adj in adjacent.iter_mut() {
        adj.sort_unstable();
        adj.dedup();
    }
    let n = dofs.n_equations();
    let mut row_ptr = Vec::with_capacity(n + 1);
    row_ptr.push(0usize);
    let mut col = Vec::new();
    for (a, adj) in adjacent.iter().enumerate() {
        for _ in 0..dofs.components(a) {
            for &b in adj {
                for j in 0..dofs.components(b) {
                    col.push(dofs.equation(b, j).expect("component of an existing node"));
                }
            }
            row_ptr.push(col.len());
        }
    }
    let val = vec![0.0; col.len()];
    CsrMatrix::rect(n, n, row_ptr, col, val)
}

fn worker_count(threads: Option<usize>, len: usize) -> usize {
    let t = threads
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
        .max(1);
    t.min(len.max(1))
}

/// Assembles a global matrix from one dense element matrix per element (the generic hook; ADR 0284
/// assembles the mass matrix with it). `f` must return a square matrix with one row per element unknown, node-major.
/// `threads` workers compute the element matrices (`None`: the available parallelism); the result does not depend on it.
/// An error of `f`, or a matrix of the wrong size, is returned as [`FemError::Element`] for the first element (in element
/// order) that fails.
pub fn assemble_with<F>(model: &SolidModel, threads: Option<usize>, f: F) -> Result<CsrMatrix>
where
    F: Fn(&ElementContext) -> Result<ElementMatrix> + Sync,
{
    let materials = model.material_indices()?;
    let mesh = model.mesh();
    let dofs = model.dof_map();
    let mut k = pattern(mesh, &dofs);
    let ne = mesh.n_elements();
    let compute = |e: usize| -> Result<ElementMatrix> {
        let nodes = mesh.element_nodes(e);
        let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
        let ctx = ElementContext {
            element: e,
            kind: mesh.element_kind(e),
            nodes,
            coords: &coords,
            material: &model.materials()[materials[e]].1,
        };
        f(&ctx)
    };
    let mut start = 0;
    while start < ne {
        let end = (start + BATCH).min(ne);
        let len = end - start;
        let workers = worker_count(threads, len);
        let mut results: Vec<Result<ElementMatrix>> = Vec::with_capacity(len);
        if workers == 1 {
            results.extend((start..end).map(compute));
        } else {
            let per = len.div_ceil(workers);
            let parts: Vec<Vec<Result<ElementMatrix>>> = std::thread::scope(|s| {
                let handles: Vec<_> = (0..workers)
                    .map(|w| {
                        let (lo, hi) = ((start + w * per).min(end), (start + (w + 1) * per).min(end));
                        let compute = &compute;
                        s.spawn(move || (lo..hi).map(compute).collect::<Vec<_>>())
                    })
                    .collect();
                handles
                    .into_iter()
                    .map(|h| h.join().expect("element worker panicked"))
                    .collect()
            });
            for p in parts {
                results.extend(p);
            }
        }
        for (offset, r) in results.into_iter().enumerate() {
            let element = start + offset;
            let ke = r.map_err(|source| FemError::Element {
                element,
                source: Box::new(source),
            })?;
            scatter(&mut k, &dofs, mesh.element_nodes(element), &ke).map_err(|source| FemError::Element {
                element,
                source: Box::new(source),
            })?;
        }
        start = end;
    }
    Ok(k)
}

/// Adds an element matrix into the global matrix, row by row and column by column.
fn scatter(k: &mut CsrMatrix, dofs: &DofMap, nodes: &[usize], ke: &ElementMatrix) -> Result<()> {
    let d = element_dofs(dofs, nodes);
    let m = d.len();
    if ke.size() != m {
        return Err(FemError::FieldLength {
            key: "element matrix",
            expected: m,
            found: ke.size(),
        });
    }
    let values = ke.values();
    for (i, &row) in d.iter().enumerate() {
        let (s, e) = (k.row_ptr[row], k.row_ptr[row + 1]);
        for (j, &c) in d.iter().enumerate() {
            let pos = s + k.col[s..e]
                .binary_search(&c)
                .expect("the pattern holds every pair of unknowns of an element");
            k.val[pos] += values[i * m + j];
        }
    }
    Ok(())
}

/// The global stiffness matrix of a model, with the available parallelism.
pub fn stiffness(model: &SolidModel) -> Result<CsrMatrix> {
    stiffness_with(model, None)
}

/// The global stiffness matrix with `threads` workers (`None`: the available parallelism); the result is bit-identical for
/// any value. Each element gives `element_for(kind).stiffness(coords, D)` with the elasticity matrix of its material. An
/// element kind or a formulation whose package has not landed is refused (ADR 0291 D6), never assembled as another. A Beam2
/// element gives [`crate::beam::element_stiffness`] with its block's section and orientation (ADR 0417 D6) and a Shell4
/// element [`crate::shell::element_stiffness`] with its block's thickness (ADR 0432 D6), a Shell3 element
/// [`crate::shell3::element_stiffness`] (ADR 0454 D2), after the checks of
/// [`SolidModel::check_elements_supported_with_structures`].
pub fn stiffness_with(model: &SolidModel, threads: Option<usize>) -> Result<CsrMatrix> {
    model.check_elements_supported_with_structures()?;
    assemble_stiffness(model, threads, None)
}

/// As [`stiffness_with`] for the static solve (ADR 0452 D6): the checks of
/// [`SolidModel::check_elements_supported_with_materials`], and an element of the orthotropic or tabulated material takes
/// [`crate::mat_ortho::element_stiffness`] with the elasticity at each integration point.
pub(crate) fn stiffness_with_materials(model: &SolidModel, threads: Option<usize>) -> Result<CsrMatrix> {
    model.check_elements_supported_with_materials()?;
    if crate::mat_ortho::uses_point_materials(model) {
        let points = crate::mat_ortho::MaterialPoints::new(model)?;
        assemble_stiffness(model, threads, Some(&points))
    } else {
        assemble_stiffness(model, threads, None)
    }
}

fn assemble_stiffness(
    model: &SolidModel,
    threads: Option<usize>,
    points: Option<&crate::mat_ortho::MaterialPoints>,
) -> Result<CsrMatrix> {
    assemble_with(model, threads, |ctx| {
        if ctx.kind == ElementKind::Beam2 {
            return crate::beam::model_element_stiffness(model.mesh(), ctx.element, ctx.coords, ctx.material);
        }
        if ctx.kind == ElementKind::Shell4 {
            return crate::shell::model_element_stiffness(model.mesh(), ctx.element, ctx.coords, ctx.material);
        }
        if ctx.kind == ElementKind::Shell3 {
            return crate::shell3::model_element_stiffness(model.mesh(), ctx.element, ctx.coords, ctx.material);
        }
        if let Some(points) = points {
            if crate::mat_ortho::is_point_material(ctx.material) {
                return crate::mat_ortho::element_stiffness(points, ctx);
            }
        }
        let d = ctx.material.elasticity_matrix()?;
        element_for(ctx.kind).stiffness(ctx.coords, &d)
    })
}
