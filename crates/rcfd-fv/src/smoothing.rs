//! Mesh smoothing by displacement diffusion (F-063, ADR 0033 D5): the boundary-driven deformation of the interior
//! mesh.
//!
//! The cell displacement `d` of every active direction solves `div(gamma grad d) = 0` (the existing FV Laplacian
//! with the deferred non-orthogonal correction). `gamma = 1` is the Laplacian method, `gamma_f = 1/l_f^k` with
//! `l_f` the distance of the face centre to the nearest prescribed-patch face centre is the inverse-distance
//! method (variable diffusivity; Jasak and Tukovic, from memory, unverified).
//!
//! Boundary treatment, by patch class: [`PatchClass::Prescribed`] Dirichlet with the face mean of the prescribed point
//! displacements; [`PatchClass::Fixed`] Dirichlet zero; [`PatchClass::Slip`] zero normal displacement and
//! zero-gradient tangential displacement (`Symmetry` condition; exact for coordinate-aligned planes). Empty patches
//! (2-D reduction) are skipped and the displacement of the empty direction is zero. Cyclic patches are not
//! supported. Point displacements are reconstructed from the cell values by a local linear least-squares fit
//! (exact for linear fields); points of prescribed patches take their prescribed value (prescribed wins over
//! fixed wins over slip where patches share a point) and points of slip patches are projected onto the slip planes.

use crate::bc::{FixedValue, Profile, Symmetry};
use crate::diffusion::laplacian;
use crate::field::VectorField;
use crate::geometry::FvGeometry;
use crate::gradient::least_squares;
use rcfd_core::vec3::{self, Vec3};
use rcfd_linalg::SolveControl;
use rcfd_mesh::PatchKind;

/// How the displacement of a patch is determined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchClass {
    /// Moves with the prescribed point displacements.
    Prescribed,
    /// Does not move.
    Fixed,
    /// Slides in its plane.
    Slip,
}

/// Diffusivity of the displacement equation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SmoothingMethod {
    /// `gamma = 1`.
    Laplacian,
    /// `gamma = 1/l^k`, `l` the distance to the nearest prescribed face (brute-force search).
    InverseDistance(f64),
}

/// Result of a smoothing call.
#[derive(Debug, Clone)]
pub struct SmoothingResult {
    /// Displacement of every mesh point.
    pub displacement: Vec<Vec3>,
    /// Linear-solver iterations spent in total.
    pub iterations: usize,
}

/// Solve the displacement of all points. `classes` has one entry per patch (empty patches ignore theirs);
/// `prescribed[p]` is the displacement of mesh point `p` (only the points of `Prescribed` patches are read).
pub fn smooth_displacement(
    geom: &FvGeometry,
    classes: &[PatchClass],
    prescribed: &[Vec3],
    method: SmoothingMethod,
    control: &SolveControl,
) -> Result<SmoothingResult, String> {
    let mesh = &geom.mesh;
    let patches = mesh.patches();
    if classes.len() != patches.len() {
        return Err("smoothing: one patch class per patch is required".into());
    }
    if prescribed.len() != mesh.n_points() {
        return Err("smoothing: one prescribed displacement per mesh point is required".into());
    }
    if geom.periodic_rot.iter().any(|r| r.is_some()) {
        return Err("smoothing: rotationally periodic patches are not supported".into());
    }
    let n = geom.n_cells();
    let n_int = geom.n_internal;
    let nf = mesh.n_faces();
    // boundary conditions of the displacement field
    let mut d = VectorField::uniform("d", geom, vec3::ZERO);
    let mut presc_faces: Vec<usize> = Vec::new();
    for (pi, p) in patches.iter().enumerate() {
        if p.kind == PatchKind::Empty || p.kind == PatchKind::Cyclic {
            continue; // empty: no flux; cyclic: periodic coupling (the default condition of the field)
        }
        match classes[pi] {
            PatchClass::Prescribed => {
                let vals: Vec<Vec3> = p
                    .faces()
                    .map(|f| {
                        let ids = mesh.face_points(f);
                        let mut s = vec3::ZERO;
                        for &q in ids {
                            s = vec3::add(s, prescribed[q]);
                        }
                        vec3::scale(s, 1.0 / ids.len() as f64)
                    })
                    .collect();
                presc_faces.extend(p.faces());
                d.set_bc(geom, &p.name, Box::new(FixedValue(Profile::PerFace(vals))))
                    .map_err(|e| e.to_string())?;
            }
            PatchClass::Fixed => d
                .set_bc(geom, &p.name, Box::new(FixedValue::vector(vec3::ZERO)))
                .map_err(|e| e.to_string())?,
            PatchClass::Slip => d.set_bc(geom, &p.name, Box::new(Symmetry)).map_err(|e| e.to_string())?,
        }
    }
    // diffusivity
    let gamma: Vec<f64> = match method {
        SmoothingMethod::Laplacian => vec![1.0; nf],
        SmoothingMethod::InverseDistance(k) => {
            if presc_faces.is_empty() {
                vec![1.0; nf]
            } else {
                let cf = mesh.face_centres();
                let targets: Vec<Vec3> = presc_faces.iter().map(|&f| cf[f]).collect();
                (0..nf)
                    .map(|f| {
                        let l = targets
                            .iter()
                            .map(|t| vec3::norm(vec3::sub(cf[f], *t)))
                            .fold(f64::INFINITY, f64::min)
                            .max(1e-12);
                        l.powf(-k)
                    })
                    .collect()
            }
        }
    };
    // solve each active component (non-orthogonal passes re-using the least-squares gradient)
    let mut iterations = 0;
    for k in 0..3 {
        if !geom.active[k] {
            continue;
        }
        let bc = d.boundary_coeffs(geom, k);
        let mut x: Vec<f64> = vec![0.0; n];
        let mut grad: Option<Vec<Vec3>> = None;
        for _pass in 0..6 {
            let m = laplacian(geom, &gamma, &bc, grad.as_deref());
            let before = x.clone();
            let stats = m.solve(&mut x, control);
            iterations += stats.iterations;
            let bv: Vec<f64> = bc
                .iter()
                .enumerate()
                .map(|(i, c)| c.value(x[mesh.owner()[n_int + i]]))
                .collect();
            grad = Some(least_squares(geom, &x, &bv));
            let change = x.iter().zip(&before).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
            let scale = x.iter().fold(0.0f64, |a, b| a.max(b.abs())).max(1e-300);
            if change <= 1e-14 * scale {
                break;
            }
        }
        for c in 0..n {
            d.values[c][k] = x[c];
        }
    }
    let displacement = reconstruct_points(geom, &d, classes, prescribed)?;
    Ok(SmoothingResult {
        displacement,
        iterations,
    })
}

/// Solve a small dense system by Gaussian elimination with partial pivoting; `None` when singular.
fn solve_small(a: &mut [[f64; 4]; 4], b: &mut [[f64; 3]; 4], n: usize) -> Option<()> {
    for col in 0..n {
        let piv = (col..n).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[piv][col].abs() < 1e-14 * (1.0 + a[0][0].abs()) {
            return None;
        }
        a.swap(col, piv);
        b.swap(col, piv);
        for r in col + 1..n {
            let f = a[r][col] / a[col][col];
            for c in col..n {
                a[r][c] -= f * a[col][c];
            }
            for c in 0..3 {
                b[r][c] -= f * b[col][c];
            }
        }
    }
    for col in (0..n).rev() {
        for c in 0..3 {
            let mut s = b[col][c];
            for j in col + 1..n {
                s -= a[col][j] * b[j][c];
            }
            b[col][c] = s / a[col][col];
        }
    }
    Some(())
}

fn reconstruct_points(
    geom: &FvGeometry,
    d: &VectorField,
    classes: &[PatchClass],
    prescribed: &[Vec3],
) -> Result<Vec<Vec3>, String> {
    let mesh = &geom.mesh;
    let np = mesh.n_points();
    let n_int = geom.n_internal;
    let (cc, cf, pts) = (mesh.cell_centres(), mesh.face_centres(), mesh.points());
    // point classification: 0 interior/none, 1 slip, 2 fixed, 3 prescribed
    let mut level = vec![0u8; np];
    let mut slip_normals: Vec<Vec<Vec3>> = vec![Vec::new(); np];
    for (pi, p) in mesh.patches().iter().enumerate() {
        if p.kind == PatchKind::Empty || p.kind == PatchKind::Cyclic {
            continue;
        }
        let lv = match classes[pi] {
            PatchClass::Prescribed => 3,
            PatchClass::Fixed => 2,
            PatchClass::Slip => 1,
        };
        for f in p.faces() {
            for &q in mesh.face_points(f) {
                level[q] = level[q].max(lv);
                if lv == 1 {
                    let nrm = geom.normal(f);
                    if !slip_normals[q].iter().any(|m| vec3::dot(*m, nrm) > 1.0 - 1e-9) {
                        slip_normals[q].push(nrm);
                    }
                }
            }
        }
    }
    // samples around every point: cells and boundary faces (non-empty)
    let mut cell_samples: Vec<Vec<usize>> = vec![Vec::new(); np];
    let mut face_samples: Vec<Vec<usize>> = vec![Vec::new(); np];
    for c in 0..mesh.n_cells() {
        for &f in mesh.cell_faces(c) {
            for &q in mesh.face_points(f) {
                if !cell_samples[q].contains(&c) {
                    cell_samples[q].push(c);
                }
            }
        }
    }
    let bv = d.boundary_values(geom);
    for f in n_int..mesh.n_faces() {
        if geom.empty[f] {
            continue;
        }
        for &q in mesh.face_points(f) {
            face_samples[q].push(f);
        }
    }
    // periodic faces: the cell across the seam, shifted by the periodic translation, completes the stencil
    let mut extra: Vec<Vec<(Vec3, Vec3)>> = vec![Vec::new(); np];
    for (pi, p) in mesh.patches().iter().enumerate() {
        if p.kind != PatchKind::Cyclic {
            continue;
        }
        if let Some(map) = &geom.periodic_partner[pi] {
            for (local, f) in p.faces().enumerate() {
                let g = map[local];
                let shift = vec3::sub(cf[f], cf[g]);
                let c = mesh.owner()[g];
                for &q in mesh.face_points(f) {
                    extra[q].push((vec3::add(cc[c], shift), d.values[c]));
                }
            }
        }
    }
    let active: Vec<usize> = (0..3).filter(|&k| geom.active[k]).collect();
    let na = active.len();
    let mut out = vec![vec3::ZERO; np];
    for q in 0..np {
        if level[q] == 3 {
            out[q] = prescribed[q];
            continue;
        }
        if level[q] == 2 {
            continue;
        }
        // weighted linear fit  d(x) = a + G (x - x_q)
        let mut m = [[0.0; 4]; 4];
        let mut rhs = [[0.0; 3]; 4];
        let mut add = |x: Vec3, v: Vec3| {
            let dx = vec3::sub(x, pts[q]);
            let w = 1.0 / vec3::dot(dx, dx).max(1e-300);
            let mut phi = [0.0; 4];
            phi[0] = 1.0;
            for (j, &k) in active.iter().enumerate() {
                phi[j + 1] = dx[k];
            }
            for i in 0..=na {
                for j in 0..=na {
                    m[i][j] += w * phi[i] * phi[j];
                }
                for c in 0..3 {
                    rhs[i][c] += w * phi[i] * v[c];
                }
            }
        };
        for &c in &cell_samples[q] {
            add(cc[c], d.values[c]);
        }
        for &f in &face_samples[q] {
            add(cf[f], bv[f - n_int]);
        }
        for &(x, v) in &extra[q] {
            add(x, v);
        }
        let mut val = match solve_small(&mut m, &mut rhs, na + 1) {
            Some(()) => rhs[0],
            None => {
                // degenerate stencil: inverse-distance weighted mean of the samples
                let (mut s, mut ws) = (vec3::ZERO, 0.0);
                for &c in &cell_samples[q] {
                    let w = 1.0 / vec3::norm(vec3::sub(cc[c], pts[q])).max(1e-300);
                    s = vec3::add(s, vec3::scale(d.values[c], w));
                    ws += w;
                }
                vec3::scale(s, 1.0 / ws.max(1e-300))
            }
        };
        for k in 0..3 {
            if !geom.active[k] {
                val[k] = 0.0;
            }
        }
        if level[q] == 1 {
            for nrm in &slip_normals[q] {
                let s = vec3::dot(val, *nrm);
                val = vec3::sub(val, vec3::scale(*nrm, s));
            }
        }
        out[q] = val;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcfd_mesh::gen::{box_mesh, Grading};
    use std::sync::Arc;

    fn tight() -> SolveControl {
        SolveControl {
            tol_abs: 1e-16,
            tol_rel: 1e-14,
            max_iter: 5000,
        }
    }

    #[test]
    fn affine_displacement_is_reproduced() {
        let m = box_mesh(
            [4, 3, 3],
            [1.2, 1.0, 0.8],
            [Grading::Expansion(3.0), Grading::Expansion(2.0), Grading::Uniform],
        )
        .unwrap();
        let g = FvGeometry::new(Arc::new(m)).unwrap();
        let a = [[0.10, -0.05, 0.02], [0.03, 0.08, -0.04], [-0.02, 0.06, 0.05]];
        let b = [0.1, -0.2, 0.05];
        let aff = |p: Vec3| -> Vec3 { [0, 1, 2].map(|i| a[i][0] * p[0] + a[i][1] * p[1] + a[i][2] * p[2] + b[i]) };
        let presc: Vec<Vec3> = g.mesh.points().iter().map(|&p| aff(p)).collect();
        let classes = vec![PatchClass::Prescribed; g.mesh.patches().len()];
        let r = smooth_displacement(&g, &classes, &presc, SmoothingMethod::Laplacian, &tight()).unwrap();
        let err = r
            .displacement
            .iter()
            .zip(g.mesh.points())
            .map(|(d, &p)| vec3::norm(vec3::sub(*d, aff(p))))
            .fold(0.0, f64::max);
        assert!(err < 1e-12, "{err}");
    }
}
