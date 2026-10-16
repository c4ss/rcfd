//! Weighted statistics, flux-weighted averages, vector fluxes and force projections (F-238, ADR 0048 D1, D2).
//!
//! Conventions follow [`crate::integrals`]: face area vectors point out of the domain, a flux is positive when it
//! leaves the domain, boundary face values are indexed `f - n_internal` and face fluxes `phi` by the global face index.

use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;

/// Weighted statistics of a sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeightedStats {
    /// Number of samples (entries with a positive weight and those with zero weight are both counted).
    pub count: usize,
    /// Sum of the weights.
    pub weight_sum: f64,
    /// Weighted mean.
    pub mean: f64,
    /// Weighted population variance `sum w (x - mean)^2 / sum w`.
    pub variance: f64,
    /// Square root of the variance.
    pub std: f64,
    /// Smallest value.
    pub min: f64,
    /// Largest value.
    pub max: f64,
}

fn bad(msg: String) -> Error {
    Error::Other(msg)
}

/// Weighted mean, variance (two-pass with the mean-correction term, so a large offset does not cancel), minimum and
/// maximum. Errors (located by argument name): length mismatch, empty input, negative / non-finite weights, non-finite
/// values, zero total weight.
pub fn weighted_stats(values: &[f64], weights: &[f64]) -> Result<WeightedStats> {
    if values.len() != weights.len() {
        return Err(bad(format!(
            "weighted_stats: values has {} entries but weights has {}",
            values.len(),
            weights.len()
        )));
    }
    if values.is_empty() {
        return Err(bad("weighted_stats: values is empty".into()));
    }
    let mut wsum = 0.0;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for (i, (&x, &w)) in values.iter().zip(weights).enumerate() {
        if !x.is_finite() {
            return Err(bad(format!("weighted_stats: values[{i}] is not finite")));
        }
        if !w.is_finite() || w < 0.0 {
            return Err(bad(format!(
                "weighted_stats: weights[{i}] = {w} is negative or not finite"
            )));
        }
        wsum += w;
        lo = lo.min(x);
        hi = hi.max(x);
    }
    if wsum <= 0.0 {
        return Err(bad("weighted_stats: the weights sum to zero".into()));
    }
    let m0 = values.iter().zip(weights).map(|(x, w)| w * x).sum::<f64>() / wsum;
    let (mut s1, mut s2) = (0.0, 0.0);
    for (x, w) in values.iter().zip(weights) {
        let d = x - m0;
        s1 += w * d;
        s2 += w * d * d;
    }
    let variance = ((s2 - s1 * s1 / wsum) / wsum).max(0.0);
    Ok(WeightedStats {
        count: values.len(),
        weight_sum: wsum,
        mean: m0 + s1 / wsum,
        variance,
        std: variance.sqrt(),
        min: lo,
        max: hi,
    })
}

fn patch_range(mesh: &Mesh, patch: &str) -> Result<std::ops::Range<usize>> {
    mesh.patch(patch)
        .map(|p| p.faces())
        .ok_or_else(|| Error::PathNotFound(format!("patch {patch}")))
}

/// Volume-weighted statistics of a cell field.
pub fn volume_stats(mesh: &Mesh, values: &[f64]) -> Result<WeightedStats> {
    if values.len() != mesh.n_cells() {
        return Err(bad(format!(
            "volume_stats: values has {} entries, the mesh has {} cells",
            values.len(),
            mesh.n_cells()
        )));
    }
    weighted_stats(values, mesh.cell_volumes())
}

/// Area-weighted statistics of boundary face values over a patch (`face_values` indexed `f - n_internal`).
pub fn patch_stats(mesh: &Mesh, patch: &str, face_values: &[f64]) -> Result<WeightedStats> {
    let r = patch_range(mesh, patch)?;
    let ni = mesh.n_internal_faces();
    if face_values.len() != mesh.n_faces() - ni {
        return Err(bad(format!(
            "patch_stats: face_values has {} entries, the mesh has {} boundary faces",
            face_values.len(),
            mesh.n_faces() - ni
        )));
    }
    let sf = mesh.face_area_vectors();
    let v: Vec<f64> = r.clone().map(|f| face_values[f - ni]).collect();
    let w: Vec<f64> = r.map(|f| vec3::norm(sf[f])).collect();
    weighted_stats(&v, &w)
}

/// Net outward flux of a vector field `sum F_f . S_f` through a patch (`face_vectors` indexed `f - n_internal`).
pub fn normal_flux(mesh: &Mesh, patch: &str, face_vectors: &[Vec3]) -> Result<f64> {
    let r = patch_range(mesh, patch)?;
    let ni = mesh.n_internal_faces();
    if face_vectors.len() != mesh.n_faces() - ni {
        return Err(bad(format!(
            "normal_flux: face_vectors has {} entries, the mesh has {} boundary faces",
            face_vectors.len(),
            mesh.n_faces() - ni
        )));
    }
    let sf = mesh.face_area_vectors();
    Ok(r.map(|f| vec3::dot(face_vectors[f - ni], sf[f])).sum())
}

/// Transport of a vector quantity through a patch, `sum phi_f v_f` (component-wise flux; `phi` by global face index,
/// `face_vectors` indexed `f - n_internal`). Positive components leave the domain.
pub fn vector_flux(mesh: &Mesh, patch: &str, phi: &[f64], face_vectors: &[Vec3]) -> Result<Vec3> {
    let r = patch_range(mesh, patch)?;
    let ni = mesh.n_internal_faces();
    if phi.len() != mesh.n_faces() {
        return Err(bad(format!(
            "vector_flux: phi has {} entries, the mesh has {} faces",
            phi.len(),
            mesh.n_faces()
        )));
    }
    if face_vectors.len() != mesh.n_faces() - ni {
        return Err(bad(format!(
            "vector_flux: face_vectors has {} entries, the mesh has {} boundary faces",
            face_vectors.len(),
            mesh.n_faces() - ni
        )));
    }
    let mut out = [0.0; 3];
    for f in r {
        vec3::axpy(&mut out, phi[f], face_vectors[f - ni]);
    }
    Ok(out)
}

/// Flux-weighted average `sum phi q / sum phi` over a patch. The signed flux is the weight, so inflow and outflow
/// patches both give the mean of `q` for a one-signed flux; a net flux that cancels (below `1e-12` of the absolute
/// flux sum) is an error because the ratio is then meaningless.
pub fn mass_weighted_average(mesh: &Mesh, patch: &str, phi: &[f64], face_values: &[f64]) -> Result<f64> {
    let r = patch_range(mesh, patch)?;
    let ni = mesh.n_internal_faces();
    if phi.len() != mesh.n_faces() || face_values.len() != mesh.n_faces() - ni {
        return Err(bad(
            "mass_weighted_average: phi / face_values do not match the mesh face counts".into(),
        ));
    }
    let (mut num, mut den, mut abs) = (0.0, 0.0, 0.0);
    for f in r {
        num += phi[f] * face_values[f - ni];
        den += phi[f];
        abs += phi[f].abs();
    }
    if den.abs() <= 1e-12 * abs || abs == 0.0 {
        return Err(bad(format!(
            "mass_weighted_average: the net flux through patch {patch} is zero"
        )));
    }
    Ok(num / den)
}

/// Force component along `dir` and the remainder: `(F . d, F - (F . d) d)` with `d = dir / |dir|`.
pub fn project_force(force: Vec3, dir: Vec3) -> Result<(f64, Vec3)> {
    let n = vec3::norm(dir);
    if !(n.is_finite() && n > 0.0) || !force.iter().all(|x| x.is_finite()) {
        return Err(bad(
            "project_force: dir must be a non-zero finite vector and force finite".into(),
        ));
    }
    let d = vec3::scale(dir, 1.0 / n);
    let a = vec3::dot(force, d);
    Ok((a, vec3::sub(force, vec3::scale(d, a))))
}

/// Component of a moment vector about the axis `axis` (normalised).
pub fn project_moment(moment: Vec3, axis: Vec3) -> Result<f64> {
    project_force(moment, axis).map(|(a, _)| a)
}
