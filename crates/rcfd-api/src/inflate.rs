//! Inflation report surface (F-108, ADR 0021 D5): `rcfd inflate` and the service method `mesh.inflate` call
//! `rcfd_mesh::inflate::inflate_patch` and report its `InflateReport`.

use rcfd_mesh::inflate::{inflate_patch, LayerSizing};
use rcfd_mesh::layers::first_cell_height_from_y_plus;
use rcfd_mesh::Mesh;
use serde_json::{json, Value};

/// Why an inflation request failed.
#[derive(Debug, Clone, PartialEq)]
pub enum InflateError {
    /// The request is inconsistent or incomplete (usage error).
    Usage(String),
    /// The library refused it (unknown patch, periodic mesh, invalid sizing).
    Library(String),
}

impl std::fmt::Display for InflateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InflateError::Usage(m) | InflateError::Library(m) => f.write_str(m),
        }
    }
}

/// An inflation request. Exactly one sizing family must be given (see [`InflateRequest::sizing`]).
#[derive(Debug, Clone, Default)]
pub struct InflateRequest {
    /// Boundary patch.
    pub patch: String,
    /// Number of layers.
    pub layers: usize,
    /// Growth ratio (first-growth and aspect-ratio sizing); default 1.2.
    pub growth: Option<f64>,
    /// Height of the first layer.
    pub first: Option<f64>,
    /// Total thickness of equal layers.
    pub total: Option<f64>,
    /// Surface size over first-layer height.
    pub aspect: Option<f64>,
    /// Last-layer height over local surface size (needs `first`).
    pub last_ratio: Option<f64>,
    /// y+ target as (y+, friction velocity, kinematic viscosity); gives `first`.
    pub y_plus: Option<(f64, f64, f64)>,
}

impl InflateRequest {
    /// The layer sizing described by the request.
    pub fn sizing(&self) -> Result<LayerSizing, InflateError> {
        let u = |m: &str| InflateError::Usage(m.to_string());
        if self.layers == 0 {
            return Err(u("--layers must be at least 1"));
        }
        if self.first.is_some() && self.y_plus.is_some() {
            return Err(u("give either a first-layer height or a y+ target, not both"));
        }
        let first = match (self.first, self.y_plus) {
            (Some(h), None) => Some(h),
            (None, Some((y, ut, nu))) => Some(first_cell_height_from_y_plus(y, ut, nu)),
            _ => None,
        };
        let growth = self.growth.unwrap_or(1.2);
        let layers = self.layers;
        match (first, self.total, self.aspect, self.last_ratio) {
            (Some(first), None, None, None) => Ok(LayerSizing::FirstGrowth { first, growth, layers }),
            (None, Some(total), None, None) => Ok(LayerSizing::Uniform { total, layers }),
            (None, None, Some(aspect), None) => Ok(LayerSizing::AspectRatio { aspect, growth, layers }),
            (Some(first), None, None, Some(last_ratio)) => Ok(LayerSizing::LastRatio {
                first,
                last_ratio,
                layers,
            }),
            (None, None, None, None) => Err(u(
                "choose a sizing: --first, --y-plus (with --u-tau and --nu), --uniform, --aspect or --last-ratio with --first",
            )),
            _ => Err(u("inconsistent sizing options (one family at a time; --last-ratio needs --first)")),
        }
    }

    /// Parse the request from JSON-RPC parameters (`patch`, `layers`, and the optional sizing keys).
    pub fn from_json(p: &Value) -> Result<Self, InflateError> {
        let f = |k: &str| p.get(k).and_then(Value::as_f64);
        let y_plus = match p.get("y_plus") {
            Some(Value::Object(o)) => {
                let g = |k: &str| o.get(k).and_then(Value::as_f64);
                match (g("y_plus"), g("u_tau"), g("nu")) {
                    (Some(a), Some(b), Some(c)) => Some((a, b, c)),
                    _ => return Err(InflateError::Usage("y_plus needs y_plus, u_tau and nu".into())),
                }
            }
            _ => None,
        };
        Ok(InflateRequest {
            patch: p
                .get("patch")
                .and_then(Value::as_str)
                .ok_or_else(|| InflateError::Usage("missing parameter 'patch'".into()))?
                .to_string(),
            layers: p.get("layers").and_then(Value::as_u64).unwrap_or(0) as usize,
            growth: f("growth"),
            first: f("first"),
            total: f("total"),
            aspect: f("aspect"),
            last_ratio: f("last_ratio"),
            y_plus,
        })
    }
}

/// Inflate the patch; returns the new mesh and the report document (keys `patch`, `sizing`, `cells_before`,
/// `cells_after`, `report`).
pub fn inflate(mesh: &Mesh, req: &InflateRequest) -> Result<(Mesh, Value), InflateError> {
    let sizing = req.sizing()?;
    let (new, report) = inflate_patch(mesh, &req.patch, sizing).map_err(|e| InflateError::Library(e.to_string()))?;
    let doc = json!({
        "patch": req.patch,
        "sizing": serde_json::to_value(sizing).unwrap_or(Value::Null),
        "cells_before": mesh.n_cells(),
        "cells_after": new.n_cells(),
        "report": serde_json::to_value(&report).unwrap_or(Value::Null),
    });
    Ok((new, doc))
}
