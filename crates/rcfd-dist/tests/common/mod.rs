//! Shared helpers of the rcfd-dist tests.
#![allow(dead_code)]

use rcfd_linalg::CsrMatrix;
use rcfd_mesh::gen::{box_mesh, cavity_2d_rect, Grading};
use rcfd_mesh::mesh::{Mesh, PatchKind};
use rcfd_mesh::partition::{Bisection, BisectionRule, Decomposition, GraphPartitioner, Partitioner, Rcb};

pub fn cavity(n: usize) -> Mesh {
    cavity_2d_rect(n, n, 1.0, 1.0).expect("cavity")
}

pub fn box8() -> Mesh {
    box_mesh(
        [8, 8, 8],
        [1.0, 1.0, 1.0],
        [Grading::Uniform, Grading::Uniform, Grading::Uniform],
    )
    .expect("box")
}

/// Finite-volume `-div(grad phi)` with homogeneous Dirichlet on every non-empty patch (orthogonal coefficients), SPD.
pub fn poisson(mesh: &Mesh) -> CsrMatrix {
    let n = mesh.n_cells();
    let cc = mesh.cell_centres();
    let mut t = Vec::new();
    for f in 0..mesh.n_internal_faces() {
        let (p, q) = (mesh.owner()[f], mesh.neighbour()[f]);
        let d = rcfd_core::vec3::norm(rcfd_core::vec3::sub(cc[q], cc[p]));
        let a = mesh.face_area(f) / d;
        t.push((p, p, a));
        t.push((q, q, a));
        t.push((p, q, -a));
        t.push((q, p, -a));
    }
    for patch in mesh.patches() {
        if patch.kind == PatchKind::Empty {
            continue;
        }
        for f in patch.start..patch.start + patch.size {
            let p = mesh.owner()[f];
            let s = mesh.face_area_vectors()[f];
            let nh = rcfd_core::vec3::scale(s, 1.0 / mesh.face_area(f));
            let d = rcfd_core::vec3::dot(rcfd_core::vec3::sub(mesh.face_centres()[f], cc[p]), nh).abs();
            t.push((p, p, mesh.face_area(f) / d));
        }
    }
    CsrMatrix::from_triplets(n, t)
}

pub fn rhs(mesh: &Mesh) -> Vec<f64> {
    mesh.cell_volumes().to_vec()
}

pub fn lcg(n: usize, seed: u64) -> Vec<f64> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((s >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
        })
        .collect()
}

pub fn partitioners() -> Vec<(&'static str, Box<dyn Partitioner>)> {
    vec![
        ("rcb", Box::new(Rcb)),
        ("bisection-cycle", Box::new(Bisection::new(BisectionRule::Cycle))),
        ("graph", Box::new(GraphPartitioner::default())),
    ]
}

pub fn decompose(mesh: &Mesh, p: usize) -> Decomposition {
    Decomposition::new(mesh, p, &Rcb).expect("decomposition")
}
