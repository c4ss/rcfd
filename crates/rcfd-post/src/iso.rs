//! Iso-surfaces of a stored cell field (ADR 0738 P1, F-809 row E-809a).
//!
//! Marching tetrahedra (Doi and Koide 1991) on the decomposition [`crate::surfaces::cut_implicit`] uses (cell centre,
//! face centre, two consecutive face points): unlike marching cubes (Lorensen and Cline 1987) it has no ambiguous case
//! and needs no lattice. A cell field is known at the cell centres only; the values at the other tet vertices are
//! reconstructed (ADR 0738 Q2 (a)): the mean of the two cells at an interior face centre, the owner's value at a boundary
//! face centre, and the mean of the cells that share a mesh point. A field linear in one coordinate on a mesh uniform
//! along it is reproduced exactly (the means of cells symmetric about a point), so the iso-surface is then the plane.

use crate::surfaces::{cut_values_edges, Surface};
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;

/// The reconstructed values of a cell field at the tet vertices of the decomposition: `n_points` point values (the mean
/// of the cells sharing the point), `n_faces` face-centre values (the mean of owner and neighbour, the owner's on a
/// boundary face), then the `n_cells` cell values.
pub fn tet_vertex_values(mesh: &Mesh, cells: &[f64]) -> Result<Vec<f64>> {
    let (np, nf, nc) = (mesh.n_points(), mesh.n_faces(), mesh.n_cells());
    if cells.len() != nc {
        return Err(Error::Other(format!("{} values for {nc} cells", cells.len())));
    }
    let mut sum = vec![0.0f64; np];
    let mut count = vec![0u32; np];
    let mut last = vec![usize::MAX; np];
    for (c, &v) in cells.iter().enumerate() {
        for &f in mesh.cell_faces(c) {
            for &p in mesh.face_points(f) {
                if last[p] != c {
                    last[p] = c;
                    sum[p] += v;
                    count[p] += 1;
                }
            }
        }
    }
    let mut out = Vec::with_capacity(np + nf + nc);
    out.extend((0..np).map(|p| if count[p] > 0 { sum[p] / count[p] as f64 } else { 0.0 }));
    let (own, nb, ni) = (mesh.owner(), mesh.neighbour(), mesh.n_internal_faces());
    out.extend((0..nf).map(|f| {
        if f < ni {
            0.5 * (cells[own[f]] + cells[nb[f]])
        } else {
            cells[own[f]]
        }
    }));
    out.extend_from_slice(cells);
    Ok(out)
}

/// An iso-surface with a value at each vertex.
#[derive(Debug, Clone, Default)]
pub struct IsoSurface {
    /// The surface (`vertex_cell` the cell of the tet each vertex was made in; facet normals toward larger values).
    pub surface: Surface,
    /// The colour field at each vertex (reconstructed as the iso field and interpolated along the same tet edge), or the
    /// level when no colour field was given.
    pub colour: Vec<f64>,
}

/// The surface where the reconstructed `cells` field equals `level`, coloured by the field `colour_by` (cell values) when
/// given. Errors: a non-finite value, a field of the wrong length.
pub fn iso_surface(mesh: &Mesh, cells: &[f64], level: f64, colour_by: Option<&[f64]>) -> Result<IsoSurface> {
    if !level.is_finite() {
        return Err(Error::Other(format!("iso_surface: level {level} is not finite")));
    }
    if cells.iter().any(|x| !x.is_finite()) {
        return Err(Error::Other("iso_surface: the field has a non-finite value".into()));
    }
    let gv: Vec<f64> = tet_vertex_values(mesh, cells)?.into_iter().map(|v| v - level).collect();
    let (surface, edges) = cut_values_edges(mesh, &gv)?;
    let colour = match colour_by {
        None => vec![level; surface.points.len()],
        Some(cb) => {
            if cb.iter().any(|x| !x.is_finite()) {
                return Err(Error::Other(
                    "iso_surface: the colour field has a non-finite value".into(),
                ));
            }
            let cv = tet_vertex_values(mesh, cb)?;
            edges.iter().map(|&(a, b, t)| cv[a] + t * (cv[b] - cv[a])).collect()
        }
    };
    Ok(IsoSurface { surface, colour })
}

/// Surface area (sum of triangle areas).
pub fn area(s: &Surface) -> f64 {
    use rcfd_core::vec3;
    s.tris
        .iter()
        .map(|t| {
            let n = vec3::cross(
                vec3::sub(s.points[t[1]], s.points[t[0]]),
                vec3::sub(s.points[t[2]], s.points[t[0]]),
            );
            0.5 * vec3::norm(n)
        })
        .sum()
}
