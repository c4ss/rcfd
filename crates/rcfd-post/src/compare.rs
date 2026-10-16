//! Case comparison and difference fields (F-238, ADR 0048 D4).
//!
//! `difference(reference, field, ...)` is `field - remap(reference)` on the mesh of `field`. The transfer is
//! [`crate::remap::Remapper`], so identical meshes (equal content hash) are copied bit for bit and the difference of
//! identical solutions is exactly zero; across meshes it is exact for fields the chosen method reproduces (constants for
//! all methods, linear fields for `least_squares` and `conservative_overlap`).

use crate::remap::{RemapMethod, Remapper};
use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;

/// A difference field with volume-weighted norms.
#[derive(Debug, Clone)]
pub struct Difference {
    /// `field - remapped reference`, per cell of the compared mesh.
    pub values: Vec<f64>,
    /// Largest absolute value.
    pub max_abs: f64,
    /// Volume-weighted mean of the absolute value.
    pub l1: f64,
    /// Volume-weighted root mean square.
    pub l2: f64,
}

fn check(name: &str, v: &[f64], n: usize, mesh: &str) -> Result<()> {
    if v.len() != n {
        return Err(Error::Other(format!(
            "difference: {name} has {} entries, the {mesh} mesh has {n} cells",
            v.len()
        )));
    }
    if let Some(i) = v.iter().position(|x| !x.is_finite()) {
        return Err(Error::Other(format!("difference: {name}[{i}] is not finite")));
    }
    Ok(())
}

fn norms(mesh: &Mesh, values: Vec<f64>) -> Difference {
    let vol = mesh.cell_volumes();
    let tv: f64 = vol.iter().sum();
    let max_abs = values.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    let l1 = vol.iter().zip(&values).map(|(v, x)| v * x.abs()).sum::<f64>() / tv;
    let l2 = (vol.iter().zip(&values).map(|(v, x)| v * x * x).sum::<f64>() / tv).sqrt();
    Difference {
        values,
        max_abs,
        l1,
        l2,
    }
}

/// Difference of a scalar cell field against a reference solution on a (possibly different) mesh.
pub fn difference(
    ref_mesh: &Mesh,
    ref_values: &[f64],
    mesh: &Mesh,
    values: &[f64],
    method: RemapMethod,
) -> Result<Difference> {
    check("ref_values", ref_values, ref_mesh.n_cells(), "reference")?;
    check("values", values, mesh.n_cells(), "compared")?;
    let r = Remapper::new(ref_mesh, mesh, method)?;
    let m = r.scalar(ref_values)?;
    let d: Vec<f64> = values.iter().zip(&m).map(|(a, b)| a - b).collect();
    Ok(norms(mesh, d))
}

/// Difference of a vector field; returns one [`Difference`] per component and the magnitude difference norms
/// (`values` of the last entry hold `|d|`).
pub fn difference_vector(
    ref_mesh: &Mesh,
    ref_values: &[Vec3],
    mesh: &Mesh,
    values: &[Vec3],
    method: RemapMethod,
) -> Result<[Difference; 3]> {
    let comp = |v: &[Vec3], k: usize| v.iter().map(|x| x[k]).collect::<Vec<f64>>();
    let mut out = Vec::new();
    for k in 0..3 {
        out.push(difference(
            ref_mesh,
            &comp(ref_values, k),
            mesh,
            &comp(values, k),
            method,
        )?);
    }
    let mut it = out.into_iter();
    Ok([it.next().expect("3"), it.next().expect("3"), it.next().expect("3")])
}
