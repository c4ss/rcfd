//! Initialisation methods of the case (F-205, ADR 0019 D1): `initial.method` = `uniform` | `potential` | `coarse`.
//!
//! This module holds the settings checks and the pieces that need no access to the simulation's private state:
//! the potential-flow initial fields of the boundary conditions and the coarsened clone of a case for the
//! coarse-level pre-solve. `Simulation::new` applies them.

use crate::case::{Case, Diagnostic};
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::potential::{self, PotentialBc};
use rcfd_fv::{FvGeometry, VectorField};
use rcfd_mesh::PatchKind;
use serde_json::json;

fn gs(c: &Case, p: &str) -> String {
    c.tree
        .get(p)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

fn gf(c: &Case, p: &str) -> f64 {
    c.tree.get(p).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn gi(c: &Case, p: &str) -> i64 {
    c.tree.get(p).ok().and_then(|v| v.as_i64()).unwrap_or(0)
}

fn d(path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

/// Generators whose cell counts the coarse pre-solve can divide.
const COARSENABLE: [&str; 9] = [
    "box_2d",
    "channel_2d",
    "cavity_2d",
    "wedge_2d",
    "nozzle_2d",
    "cylinder_ogrid",
    "annulus_3d",
    "channel_2d_interface",
    "annulus_3d_interface",
];

/// The configured method.
pub fn method(case: &Case) -> String {
    gs(case, "initial.method")
}

/// Settings checks (no mesh needed).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let m = method(case);
    if m != "uniform" && !matches!(gs(case, "solver.algorithm").as_str(), "simple" | "simplec") {
        out.push(d(
            "initial.method",
            format!(
                "initial.method = \"{m}\" is available with the steady algorithms simple and simplec, not with \"{}\"",
                gs(case, "solver.algorithm")
            ),
        ));
    }
    if m == "coarse" {
        let generator = gs(case, "mesh.generator");
        if gs(case, "mesh.source") == "file" {
            out.push(d(
                "initial.method",
                "the coarse pre-solve divides the cell counts of a generated mesh; an imported mesh (mesh.source = \"file\") has none",
            ));
        } else if !COARSENABLE.contains(&generator.as_str()) {
            out.push(d(
                "initial.method",
                format!("the coarse pre-solve divides the cell counts of a generated mesh; mesh.generator = \"{generator}\" has none"),
            ));
        } else if let Err(mut e) = coarse_case(case) {
            out.append(&mut e);
        }
    }
    out
}

/// The case with the cell counts divided by `initial.coarse_factor`, `initial.method = "uniform"` and
/// `solver.max_iterations = initial.coarse_iterations`.
pub fn coarse_case(case: &Case) -> Result<Case, Vec<Diagnostic>> {
    let f = gi(case, "initial.coarse_factor").max(2);
    let generator = gs(case, "mesh.generator");
    if gs(case, "mesh.source") == "file" || !COARSENABLE.contains(&generator.as_str()) {
        return Err(vec![d(
            "initial.method",
            "the coarse pre-solve divides the cell counts of a generated mesh (an imported mesh or this generator has none)",
        )]);
    }
    let mut keys = vec!["mesh.nx", "mesh.ny"];
    if generator.starts_with("annulus_3d") {
        keys.push("mesh.nz");
    }
    if generator.ends_with("_interface") {
        keys.push("mesh.nx_right");
        keys.push("mesh.ny_right");
        if generator.starts_with("annulus_3d") {
            keys.push("mesh.nz_right");
        }
    }
    let mut out = case.clone();
    let mut errs = Vec::new();
    for k in keys {
        let n = gi(case, k);
        if n == 0 && k.ends_with("_right") {
            continue; // follows the left block
        }
        if n % f != 0 || n / f < 2 {
            errs.push(d(
                "initial.coarse_factor",
                format!("{k} = {n} cannot be divided by the factor {f} into at least 2 cells"),
            ));
            continue;
        }
        if let Err(e) = out.set(k, json!(n / f)) {
            errs.push(d(k, e.to_string()));
        }
    }
    if !errs.is_empty() {
        return Err(errs);
    }
    let its = gi(case, "initial.coarse_iterations").max(1);
    for (k, v) in [
        ("initial.method", json!("uniform")),
        ("solver.max_iterations", json!(its)),
    ] {
        if let Err(e) = out.set(k, v) {
            return Err(vec![d(k, e.to_string())]);
        }
    }
    Ok(out)
}

/// Potential-flow velocity and Bernoulli pressure for the boundary conditions of `case`; `u` carries the velocity
/// boundary conditions (and the initial values that some of them read).
pub fn potential_fields(
    case: &Case,
    geom: &FvGeometry,
    u: &VectorField,
) -> Result<(Vec<Vec3>, Vec<f64>), Vec<Diagnostic>> {
    let ub = u.boundary_values(geom);
    let mut patches: Vec<(String, PotentialBc)> = Vec::new();
    let mut fixed_p: Vec<(String, f64)> = Vec::new();
    let mut errs = Vec::new();
    for p in geom.mesh.patches() {
        match p.kind {
            PatchKind::Empty => continue,
            PatchKind::Cyclic => {
                errs.push(d(
                    "initial.method",
                    format!(
                        "the potential-flow initialisation does not support the periodic patch '{}'",
                        p.name
                    ),
                ));
                continue;
            }
            _ => {}
        }
        let b = format!("boundaries.{}", p.name);
        if case.tree.get(&b).is_err() {
            continue;
        }
        if gs(case, &format!("{b}.pressure")) == "fixed" {
            fixed_p.push((p.name.clone(), gf(case, &format!("{b}.pressure_value"))));
            patches.push((p.name.clone(), PotentialBc::Dirichlet));
            continue;
        }
        let kind = gs(case, &format!("{b}.velocity"));
        if kind == "zero_gradient" {
            errs.push(d(
                &format!("{b}.velocity"),
                format!(
                    "the potential-flow initialisation needs the velocity of patch '{}' or a fixed pressure on it (velocity = zero_gradient without pressure = fixed)",
                    p.name
                ),
            ));
            continue;
        }
        let vals: Vec<f64> = p
            .faces()
            .map(|f| {
                let n = geom.normal(f);
                if kind == "freestream" {
                    let r = case
                        .tree
                        .get(&format!("{b}.velocity_value"))
                        .ok()
                        .and_then(|v| v.as_array().cloned())
                        .map(|a| [0, 1, 2].map(|k| a.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0)))
                        .unwrap_or([0.0; 3]);
                    vec3::dot(r, n)
                } else {
                    vec3::dot(ub[f - geom.n_internal], n)
                }
            })
            .collect();
        patches.push((p.name.clone(), PotentialBc::Flux(vals)));
    }
    if !errs.is_empty() {
        return Err(errs);
    }
    let flow = potential::potential_flow(geom, &patches).map_err(|m| vec![d("initial.method", m)])?;
    let head = potential::bernoulli_head(geom, &flow.u, &fixed_p).unwrap_or_else(|| {
        let umax2 = flow.u.iter().map(|v| vec3::dot(*v, *v)).fold(0.0, f64::max);
        gf(case, "initial.pressure") + 0.5 * umax2
    });
    let p = potential::bernoulli_pressure(&flow.u, head);
    Ok((flow.u, p))
}
