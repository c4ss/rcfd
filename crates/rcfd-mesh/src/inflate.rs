//! Prismatic inflation layers on arbitrary (unstructured) boundary patches (F-108, ADR 0020 part B).
//!
//! The wall patch is extruded outward along the per-point averaged normals in `layers` steps whose
//! heights follow a [`LayerSizing`]: uniform, geometric from a first height (for example the one
//! given by a `y+` target through [`crate::layers::first_cell_height_from_y_plus`]), geometric from
//! a first height that follows the local surface size through an aspect ratio, or geometric to a
//! last layer of a given multiple of the local surface size. Each patch face yields one prism or
//! hexahedron per layer. This builds the layer shell only; merging it with a bulk volume mesh is
//! not part of this module.

use crate::edit::{extrude_core, Displacement};
use crate::mesh::Mesh;
use crate::quality::{check_mesh, QualityReport};
use rcfd_core::{Error, Result};
use serde::{Deserialize, Serialize};

/// How the layer heights at a patch point are chosen. `s` is the local surface size of the point:
/// the mean over its incident patch faces of the square root of the face area.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LayerSizing {
    /// `layers` equal layers of total thickness `total`.
    Uniform {
        /// Total thickness.
        total: f64,
        /// Number of layers.
        layers: usize,
    },
    /// Heights `first * growth^i`.
    FirstGrowth {
        /// Height of the layer next to the wall.
        first: f64,
        /// Ratio of successive heights.
        growth: f64,
        /// Number of layers.
        layers: usize,
    },
    /// Heights `(s / aspect) * growth^i`: the first layer has the aspect ratio `s / h` to the local surface size.
    AspectRatio {
        /// Surface size over first-layer height.
        aspect: f64,
        /// Ratio of successive heights.
        growth: f64,
        /// Number of layers.
        layers: usize,
    },
    /// Heights `first * g^i` with `g` chosen so that the last layer is `last_ratio * s` high (`layers >= 2`).
    LastRatio {
        /// Height of the layer next to the wall.
        first: f64,
        /// Last-layer height over local surface size.
        last_ratio: f64,
        /// Number of layers.
        layers: usize,
    },
}

impl LayerSizing {
    /// Number of layers.
    pub fn layers(&self) -> usize {
        match *self {
            LayerSizing::Uniform { layers, .. }
            | LayerSizing::FirstGrowth { layers, .. }
            | LayerSizing::AspectRatio { layers, .. }
            | LayerSizing::LastRatio { layers, .. } => layers,
        }
    }

    /// Check the parameters.
    pub fn validate(&self) -> Result<()> {
        let pos = |name: &str, v: f64| -> Result<()> {
            if v.is_finite() && v > 0.0 {
                Ok(())
            } else {
                Err(Error::Mesh(format!(
                    "inflate: {name} must be positive and finite (got {v})"
                )))
            }
        };
        if self.layers() == 0 {
            return Err(Error::Mesh("inflate: at least one layer is required".into()));
        }
        match *self {
            LayerSizing::Uniform { total, .. } => pos("total thickness", total),
            LayerSizing::FirstGrowth { first, growth, .. } => {
                pos("first height", first).and(pos("growth ratio", growth))
            }
            LayerSizing::AspectRatio { aspect, growth, .. } => {
                pos("aspect ratio", aspect).and(pos("growth ratio", growth))
            }
            LayerSizing::LastRatio {
                first,
                last_ratio,
                layers,
            } => {
                if layers < 2 {
                    return Err(Error::Mesh(
                        "inflate: last-ratio sizing needs at least two layers".into(),
                    ));
                }
                pos("first height", first).and(pos("last-layer ratio", last_ratio))
            }
        }
    }

    /// Layer heights at a point of local surface size `s`.
    pub fn heights(&self, s: f64) -> Vec<f64> {
        match *self {
            LayerSizing::Uniform { total, layers } => vec![total / layers as f64; layers],
            LayerSizing::FirstGrowth { first, growth, layers } => {
                (0..layers).map(|i| first * growth.powi(i as i32)).collect()
            }
            LayerSizing::AspectRatio { aspect, growth, layers } => {
                (0..layers).map(|i| s / aspect * growth.powi(i as i32)).collect()
            }
            LayerSizing::LastRatio {
                first,
                last_ratio,
                layers,
            } => {
                let g = (last_ratio * s / first).powf(1.0 / (layers - 1) as f64);
                (0..layers).map(|i| first * g.powi(i as i32)).collect()
            }
        }
    }
}

/// Summary of an inflation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InflateReport {
    /// Number of layers.
    pub layers: usize,
    /// Smallest and largest first-layer height over the patch points.
    pub first_height_min: f64,
    /// See `first_height_min`.
    pub first_height_max: f64,
    /// Smallest last-layer height.
    pub last_height_min: f64,
    /// Largest last-layer height.
    pub last_height_max: f64,
    /// Smallest total layer thickness.
    pub thickness_min: f64,
    /// Largest total layer thickness.
    pub thickness_max: f64,
    /// Smallest ratio of successive layer heights (1 for a single layer).
    pub growth_min: f64,
    /// Largest ratio of successive layer heights (1 for a single layer).
    pub growth_max: f64,
    /// Quality check of the resulting mesh.
    pub quality: QualityReport,
}

/// Add prismatic layers to the outward side of boundary patch `patch`. The patch is replaced by the far
/// cap (same name and kind) and a `Generic` patch `<patch>_side` with the lateral faces (see
/// [`crate::edit::extrude_patch`]).
pub fn inflate_patch(mesh: &Mesh, patch: &str, sizing: LayerSizing) -> Result<(Mesh, InflateReport)> {
    sizing.validate()?;
    let layers = sizing.layers();
    let cum = |s: f64, k: usize| -> f64 { sizing.heights(s)[..k].iter().sum() };
    let (out, sizes) = extrude_core(mesh, patch, layers, Displacement::Layered(&cum))?;
    let mut rep = InflateReport {
        layers,
        first_height_min: f64::INFINITY,
        first_height_max: 0.0,
        last_height_min: f64::INFINITY,
        last_height_max: 0.0,
        thickness_min: f64::INFINITY,
        thickness_max: 0.0,
        growth_min: if layers > 1 { f64::INFINITY } else { 1.0 },
        growth_max: if layers > 1 { 0.0 } else { 1.0 },
        quality: check_mesh(&out),
    };
    for &s in &sizes {
        let h = sizing.heights(s);
        rep.first_height_min = rep.first_height_min.min(h[0]);
        rep.first_height_max = rep.first_height_max.max(h[0]);
        let last = h[layers - 1];
        rep.last_height_min = rep.last_height_min.min(last);
        rep.last_height_max = rep.last_height_max.max(last);
        let t: f64 = h.iter().sum();
        rep.thickness_min = rep.thickness_min.min(t);
        rep.thickness_max = rep.thickness_max.max(t);
        for w in h.windows(2) {
            let r = w[1] / w[0];
            rep.growth_min = rep.growth_min.min(r);
            rep.growth_max = rep.growth_max.max(r);
        }
    }
    Ok((out, rep))
}
