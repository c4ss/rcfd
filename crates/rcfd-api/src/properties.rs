//! Temperature-dependent fluid properties (F-053): `properties.viscosity`, `properties.conductivity`
//! and `properties.specific_heat`, each a constant (the `physics` value), a polynomial in T or
//! Sutherland's law (see `rcfd_core::materials`).
//!
//! * The dynamic viscosity enters the momentum diffusion term through the face viscosity of the flow
//!   solver (`nu_face = mu(T_f) / rho`, the same path as the non-Newtonian rheology and the user-function
//!   viscosity, with which it therefore cannot be combined).
//! * Conductivity and specific heat give the thermal diffusivity at the faces
//!   `alpha_f = k(T_f) / (rho cp(T_f))` of the energy equation.
//!
//! The density stays constant (incompressible flow); a variable-density ideal gas is not part of this
//! model. `T_f` is the linear interpolation of the cell temperatures (boundary value on boundary faces).

use crate::case::{Case, Diagnostic};
use rcfd_core::materials::PropertyLaw;
use rcfd_core::tabular::{Extrapolation, PiecewiseLinear, PiecewisePoly, Vogel};
use rcfd_fv::{FvGeometry, ScalarField};

const NAMES: [&str; 3] = ["viscosity", "conductivity", "specific_heat"];

fn gf(c: &Case, p: &str) -> f64 {
    c.tree.get(p).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn gs(c: &Case, p: &str) -> String {
    c.tree
        .get(p)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

fn coefficients(c: &Case, p: &str) -> Vec<f64> {
    c.tree
        .get(p)
        .ok()
        .and_then(|v| {
            v.as_array()
                .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect())
        })
        .unwrap_or_default()
}

fn law(case: &Case, name: &str) -> Option<PropertyLaw> {
    let p = |f: &str| format!("properties.{name}.{f}");
    match gs(case, &p("law")).as_str() {
        "polynomial" => Some(PropertyLaw::Polynomial(coefficients(case, &p("coefficients")))),
        "sutherland" => Some(PropertyLaw::Sutherland {
            reference_value: gf(case, &p("reference_value")),
            reference_temperature: gf(case, &p("reference_temperature")),
            constant: gf(case, &p("sutherland_constant")),
        }),
        "piecewise_linear" => {
            let ex = Extrapolation::parse(&gs(case, &p("extrapolation"))).unwrap_or_default();
            // an invalid table still selects the law; `check` reports the located error
            Some(
                PiecewiseLinear::new(
                    coefficients(case, &p("temperatures")),
                    coefficients(case, &p("values")),
                    ex,
                )
                .map(PropertyLaw::PiecewiseLinear)
                .unwrap_or(PropertyLaw::Constant(f64::NAN)),
            )
        }
        "piecewise_polynomial" => {
            let ex = Extrapolation::parse(&gs(case, &p("extrapolation"))).unwrap_or_default();
            Some(
                piece_lists(case, name)
                    .and_then(|(b, c)| PiecewisePoly::new(b, c, ex).map_err(|e| e.to_string()))
                    .map(PropertyLaw::PiecewisePolynomial)
                    .unwrap_or(PropertyLaw::Constant(f64::NAN)),
            )
        }
        "vogel" => Some(PropertyLaw::Vogel(Vogel {
            a: gf(case, &p("vogel_a")),
            b: gf(case, &p("vogel_b")),
            c: gf(case, &p("vogel_c")),
        })),
        _ => None,
    }
}

/// Breakpoints and the per-piece coefficient lists of a `piecewise_polynomial` law.
fn piece_lists(case: &Case, name: &str) -> Result<(Vec<f64>, Vec<Vec<f64>>), String> {
    let p = |f: &str| format!("properties.{name}.{f}");
    let order = case
        .tree
        .get(&p("piece_order"))
        .ok()
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if order < 1 {
        return Err("piece_order must be at least 1 (coefficients per piece)".into());
    }
    let order = order as usize;
    let flat = coefficients(case, &p("piece_coefficients"));
    if flat.is_empty() || flat.len() % order != 0 {
        return Err(format!(
            "piece_coefficients has {} entries, which is not a positive multiple of piece_order = {order}",
            flat.len()
        ));
    }
    Ok((
        coefficients(case, &p("breakpoints")),
        flat.chunks(order).map(|c| c.to_vec()).collect(),
    ))
}

/// Located message of an invalid table law (`None` when the law is valid or not tabular).
fn table_error(case: &Case, name: &str) -> Option<(String, String)> {
    let p = |f: &str| format!("properties.{name}.{f}");
    let ex = Extrapolation::parse(&gs(case, &p("extrapolation"))).unwrap_or_default();
    match gs(case, &p("law")).as_str() {
        "piecewise_linear" => {
            let (x, y) = (coefficients(case, &p("temperatures")), coefficients(case, &p("values")));
            PiecewiseLinear::new(x.clone(), y, ex).err().map(|e| {
                let (k, msg) = match &e {
                    rcfd_core::Error::InvalidValue { path, reason } => (path.clone(), reason.clone()),
                    e => (String::new(), e.to_string()),
                };
                let key = if k.starts_with('y') || k == "values" {
                    "values"
                } else {
                    "temperatures"
                };
                (p(key), msg)
            })
        }
        "piecewise_polynomial" => match piece_lists(case, name) {
            Err(m) => Some((p("piece_coefficients"), m)),
            Ok((b, c)) => PiecewisePoly::new(b, c, ex).err().map(|e| {
                let msg = e.to_string();
                let key = if msg.contains("breakpoints") || msg.contains("breaks") {
                    "breakpoints"
                } else {
                    "piece_coefficients"
                };
                (p(key), msg)
            }),
        },
        _ => None,
    }
}

/// Is any property law temperature-dependent?
pub fn enabled(case: &Case) -> bool {
    NAMES.iter().any(|n| law(case, n).is_some())
}

/// Settings checks.
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut bad = |path: String, message: String| {
        out.push(Diagnostic {
            path,
            line: None,
            message,
        })
    };
    if !enabled(case) {
        return out;
    }
    let alg = gs(case, "solver.algorithm");
    let energy = case
        .tree
        .get("energy.enabled")
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    for n in NAMES {
        let Some(l) = law(case, n) else { continue };
        let p = |f: &str| format!("properties.{n}.{f}");
        if !matches!(alg.as_str(), "simple" | "simplec") {
            bad(
                p("law"),
                format!("temperature-dependent properties need the steady algorithms (simple, simplec) with the energy equation, not \"{alg}\""),
            );
        }
        if !energy {
            bad(
                p("law"),
                "a temperature-dependent property needs the temperature field: set energy.enabled = true".into(),
            );
        }
        if let Some((path, message)) = table_error(case, n) {
            bad(path, message);
        } else if let Err(e) = l.validate() {
            let field = match &l {
                PropertyLaw::Polynomial(_) => "coefficients",
                PropertyLaw::Vogel(_) => "vogel_a",
                _ => "reference_value",
            };
            bad(p(field), e.to_string());
        }
    }
    if law(case, "viscosity").is_some() {
        if gf(case, "physics.rho") <= 0.0 {
            bad(
                "physics.rho".into(),
                "a temperature-dependent dynamic viscosity needs a positive physics.rho".into(),
            );
        }
        if crate::rheology::enabled(case) {
            bad(
                "properties.viscosity.law".into(),
                "cannot be combined with rheology.model / rheology.shift: both set the face viscosity".into(),
            );
        }
        if crate::turb::enabled(case) {
            bad(
                "properties.viscosity.law".into(),
                "cannot be combined with a turbulence model in this version".into(),
            );
        }
        for n in case.tree.names("udf").unwrap_or_default() {
            if case
                .tree
                .get(&format!("udf.{n}.viscosity"))
                .ok()
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                bad(
                    format!("udf.{n}.viscosity"),
                    "a user-function viscosity cannot be combined with properties.viscosity".into(),
                );
            }
        }
    }
    for n in ["conductivity", "specific_heat"] {
        if law(case, n).is_some() && (gf(case, "physics.rho") <= 0.0) {
            bad(
                "physics.rho".into(),
                "temperature-dependent conductivity or specific heat needs a positive physics.rho".into(),
            );
            break;
        }
    }
    out
}

/// Configured laws.
#[derive(Debug, Clone)]
pub struct Properties {
    mu: Option<PropertyLaw>,
    k: Option<PropertyLaw>,
    cp: Option<PropertyLaw>,
    rho: f64,
    k0: f64,
    cp0: f64,
}

/// Linear face interpolation of a cell field, boundary values on boundary faces.
fn face_values(geom: &FvGeometry, t: &ScalarField) -> Vec<f64> {
    let mesh = &geom.mesh;
    let bv = t.boundary_values(geom);
    (0..mesh.n_faces())
        .map(|f| {
            if f < geom.n_internal {
                let w = geom.weights[f];
                w * t.values[mesh.owner()[f]] + (1.0 - w) * t.values[mesh.neighbour()[f]]
            } else {
                bv[f - geom.n_internal]
            }
        })
        .collect()
}

impl Properties {
    /// Build from the case; `None` when every property is constant.
    pub fn from_case(case: &Case) -> Option<Properties> {
        enabled(case).then(|| Properties {
            mu: law(case, "viscosity"),
            k: law(case, "conductivity"),
            cp: law(case, "specific_heat"),
            rho: gf(case, "physics.rho"),
            k0: gf(case, "physics.conductivity"),
            cp0: gf(case, "physics.cp"),
        })
    }

    /// Does the viscosity depend on temperature?
    pub fn has_viscosity(&self) -> bool {
        self.mu.is_some()
    }

    /// Does the thermal diffusivity depend on temperature?
    pub fn has_diffusivity(&self) -> bool {
        self.k.is_some() || self.cp.is_some()
    }

    /// Kinematic viscosity at every face.
    pub fn nu_faces(&self, geom: &FvGeometry, t: &ScalarField) -> Option<Vec<f64>> {
        let mu = self.mu.as_ref()?;
        Some(face_values(geom, t).iter().map(|&tf| mu.eval(tf) / self.rho).collect())
    }

    /// Thermal diffusivity `k / (rho cp)` at every face.
    pub fn alpha_faces(&self, geom: &FvGeometry, t: &ScalarField) -> Option<Vec<f64>> {
        if !self.has_diffusivity() {
            return None;
        }
        Some(
            face_values(geom, t)
                .iter()
                .map(|&tf| self.conductivity(tf) / (self.rho * self.specific_heat(tf)))
                .collect(),
        )
    }

    /// Conductivity at temperature `t`.
    pub fn conductivity(&self, t: f64) -> f64 {
        self.k.as_ref().map_or(self.k0, |l| l.eval(t))
    }

    /// Specific heat at temperature `t`.
    pub fn specific_heat(&self, t: f64) -> f64 {
        self.cp.as_ref().map_or(self.cp0, |l| l.eval(t))
    }

    /// Does the conductivity depend on temperature?
    pub fn has_conductivity(&self) -> bool {
        self.k.is_some()
    }
}
