//! Setup-workflow tools: mesh assessment, boundary-condition suggestion, case
//! validation and a residual divergence explainer.
//!
//! The analysis functions are pure (JSON in, JSON out) so they can be tested on
//! synthetic data. Thresholds are widely used rules of thumb from CFD practice
//! (e.g. non-orthogonality above ~70 degrees and skewness above ~4 are
//! commonly treated as unacceptable), not guarantees; every report says so.

use serde_json::{json, Value};

fn num(v: &Value) -> Option<f64> {
    v.as_f64()
}

fn finding(severity: &str, topic: &str, message: String, advice: &str) -> Value {
    json!({"severity": severity, "topic": topic, "message": message, "advice": advice})
}

/// Interpret a `mesh.check` result.
pub fn assess_mesh(check: &Value) -> Value {
    let r = &check["report"];
    let mut f = vec![];
    for e in r["errors"].as_array().into_iter().flatten() {
        f.push(finding(
            "error",
            "topology",
            e.as_str().unwrap_or("mesh error").to_string(),
            "Fix the mesh; the solver will not run on an invalid mesh.",
        ));
    }
    for w in r["warnings"].as_array().into_iter().flatten() {
        f.push(finding(
            "warning",
            "mesh check",
            w.as_str().unwrap_or("warning").to_string(),
            "Review the cells flagged by the mesh check.",
        ));
    }
    if let Some(v) = num(&r["max_non_orthogonality"]) {
        if v > 70.0 {
            f.push(finding(
                "error",
                "non-orthogonality",
                format!("max non-orthogonality {v:.1} deg"),
                "Above ~70 deg the pressure equation is unreliable; regenerate or smooth the mesh.",
            ));
        } else if v > 40.0 {
            f.push(finding(
                "warning",
                "non-orthogonality",
                format!("max non-orthogonality {v:.1} deg"),
                "Set solver.non_orthogonal_correctors to 1-2 and prefer a more diffusive convection scheme at first.",
            ));
        }
    }
    if let Some(v) = num(&r["max_skewness"]) {
        if v > 4.0 {
            f.push(finding(
                "error",
                "skewness",
                format!("max skewness {v:.2}"),
                "Highly skewed faces degrade accuracy and stability; improve the mesh there.",
            ));
        } else if v > 2.0 {
            f.push(finding(
                "warning",
                "skewness",
                format!("max skewness {v:.2}"),
                "Moderately skewed cells; check they are away from steep gradients.",
            ));
        }
    }
    if let Some(v) = num(&r["max_aspect_ratio"]) {
        if v > 1000.0 {
            f.push(finding(
                "error",
                "aspect ratio",
                format!("max aspect ratio {v:.0}"),
                "Extreme stretching; acceptable only in resolved boundary layers aligned with the flow.",
            ));
        } else if v > 100.0 {
            f.push(finding(
                "warning",
                "aspect ratio",
                format!("max aspect ratio {v:.0}"),
                "Fine inside boundary layers aligned with the flow, risky elsewhere.",
            ));
        }
    }
    if let Some(v) = num(&r["max_volume_ratio"]) {
        if v > 100.0 {
            f.push(finding(
                "warning",
                "volume ratio",
                format!("neighbour volume ratio {v:.0}"),
                "Abrupt size changes reduce accuracy; grade the mesh more smoothly.",
            ));
        }
    }
    let n_err = f.iter().filter(|x| x["severity"] == "error").count();
    let n_warn = f.iter().filter(|x| x["severity"] == "warning").count();
    let verdict = if n_err > 0 {
        "not usable"
    } else if n_warn > 0 {
        "usable with care"
    } else {
        "good"
    };
    json!({
        "verdict": verdict,
        "cells": r["n_cells"],
        "findings": f,
        "patches": check["patches"],
        "note": "Thresholds are common rules of thumb, not guarantees."
    })
}

fn patch_names(patches: &Value) -> Vec<(String, String)> {
    patches
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| {
            (
                p["name"].as_str().unwrap_or_default().to_string(),
                p["kind"].as_str().unwrap_or("Generic").to_string(),
            )
        })
        .collect()
}

/// Propose boundary conditions for patches that have none. Only proposes;
/// applying the `settings` entries goes through `set_setting` and approval.
pub fn suggest_boundary_conditions(patches: &Value, boundaries: &Value) -> Value {
    let mut out = vec![];
    let mut any_fixed_pressure = boundaries
        .as_object()
        .map(|m| m.values().any(|b| b["pressure"] == "fixed"))
        .unwrap_or(false);
    for (name, kind) in patch_names(patches) {
        let lname = name.to_lowercase();
        let configured = boundaries.get(&name).is_some();
        let has = |words: &[&str]| words.iter().any(|w| lname.contains(w));
        let (role, settings, why): (&str, Vec<(&str, Value)>, String) =
            if kind == "Empty" || has(&["frontandback", "empty"]) {
                ("empty", vec![], "2D reduction patch: no condition is applied".into())
            } else if kind == "Cyclic" {
                ("periodic", vec![], "periodic pair is defined by the mesh".into())
            } else if kind == "Symmetry" || has(&["symm"]) {
                (
                    "symmetry",
                    vec![("velocity", json!("symmetry"))],
                    "symmetry plane: zero normal velocity, zero shear".into(),
                )
            } else if kind == "Inlet" || has(&["inlet", "inflow"]) || lname == "in" {
                (
                    "inlet",
                    vec![
                        ("velocity", json!("fixed")),
                        ("velocity_value", json!([1.0, 0.0, 0.0])),
                        ("pressure", json!("zero_gradient")),
                    ],
                    "inflow: fixed velocity. The vector [1,0,0] is a PLACEHOLDER; set the real magnitude and direction"
                        .into(),
                )
            } else if kind == "Outlet" || has(&["outlet", "outflow"]) || lname == "out" {
                any_fixed_pressure = true;
                (
                    "outlet",
                    vec![
                        ("velocity", json!("zero_gradient")),
                        ("pressure", json!("fixed")),
                        ("pressure_value", json!(0.0)),
                    ],
                    "outflow: fixed reference pressure, zero-gradient velocity".into(),
                )
            } else if has(&["lid", "moving"]) {
                (
                    "moving wall",
                    vec![
                        ("velocity", json!("fixed")),
                        ("velocity_value", json!([1.0, 0.0, 0.0])),
                        ("pressure", json!("zero_gradient")),
                    ],
                    "moving wall: fixed tangential velocity. [1,0,0] is a PLACEHOLDER; set the real wall speed".into(),
                )
            } else if kind == "Wall" || has(&["wall", "cylinder", "body", "airfoil", "plate"]) {
                (
                    "wall",
                    vec![
                        ("velocity", json!("fixed")),
                        ("velocity_value", json!([0.0, 0.0, 0.0])),
                        ("pressure", json!("zero_gradient")),
                    ],
                    "no-slip wall: zero velocity, zero-gradient pressure".into(),
                )
            } else {
                (
                    "unknown",
                    vec![],
                    "role could not be inferred from the name or kind; ask the user".into(),
                )
            };
        let status = if configured { "configured" } else { "missing" };
        let apply: Vec<Value> = if configured {
            vec![]
        } else {
            settings
                .into_iter()
                .map(|(k, v)| json!({"path": format!("boundaries.{name}.{k}"), "value": v}))
                .collect()
        };
        out.push(
            json!({"patch": name, "kind": kind, "role": role, "status": status, "rationale": why, "settings": apply}),
        );
    }
    let mut notes = vec![
        "These are heuristics based on patch names and kinds. Review them, then apply with set_setting (requires approval).".to_string(),
    ];
    if !any_fixed_pressure {
        notes.push("No patch fixes the pressure. That is fine for a closed domain (e.g. a cavity); with an outlet, fix its pressure.".into());
    }
    json!({"suggestions": out, "notes": notes})
}

/// True when the settings select a RANS turbulence model (k-epsilon or k-omega SST); the inlet
/// helpers and the y+ checks apply to both.
fn is_k_epsilon(settings: &Value) -> bool {
    matches!(
        settings["turbulence"]["model"].as_str(),
        Some("k_epsilon" | "k_omega_sst")
    )
}

/// True when the settings select k-omega SST.
fn is_sst(settings: &Value) -> bool {
    settings["turbulence"]["model"] == "k_omega_sst"
}

/// Estimated cell-centre `y+` of the wall-adjacent cell from the settings alone: flat-plate
/// friction velocity `u_tau = U sqrt(Cf/2)`, `Cf = 0.455 / ln^2(0.06 Re_x)` (White) at the
/// downstream end `lx`, and the first-cell height from `mesh.first_layer_height`, or `ly / ny`
/// (half of it for a channel-like domain is not assumed: the uniform cell height is used).
/// Returns `None` when speed, viscosity or the mesh sizes are not available.
fn estimate_first_cell_y_plus(settings: &Value, speed: f64, nu: f64) -> Option<f64> {
    let mesh = &settings["mesh"];
    let lx = mesh["lx"].as_f64()?;
    let re = speed * lx / nu;
    if re.is_nan() || re <= 1e3 {
        return None;
    }
    let cf = 0.455 / (0.06 * re).ln().powi(2);
    let u_tau = speed * (0.5 * cf).sqrt();
    let h1 =
        if mesh["wall_layers"].as_i64().unwrap_or(0) > 0 && mesh["first_layer_height"].as_f64().unwrap_or(0.0) > 0.0 {
            mesh["first_layer_height"].as_f64()?
        } else {
            mesh["ly"].as_f64()? / mesh["ny"].as_f64()?.max(1.0)
        };
    Some(0.5 * h1 * u_tau / nu)
}

/// True when the settings select the compressible solver.
fn is_compressible(settings: &Value) -> bool {
    settings["solver"]["algorithm"] == "compressible"
}

/// [`suggest_boundary_conditions`] that also knows the physics selected in `settings`: with the
/// k-epsilon model, inlets get turbulence-helper entries; with the compressible solver, every
/// patch gets `compressible_bc` entries (the incompressible velocity and pressure entries are
/// not used by that solver and are left out). Without either, the result equals
/// [`suggest_boundary_conditions`].
pub fn suggest_boundary_conditions_for(patches: &Value, boundaries: &Value, settings: &Value) -> Value {
    let mut out = suggest_boundary_conditions(patches, boundaries);
    let (ke, comp) = (is_k_epsilon(settings), is_compressible(settings));
    if !ke && !comp {
        return out;
    }
    if let Some(list) = out["suggestions"].as_array_mut() {
        for s in list.iter_mut() {
            let name = s["patch"].as_str().unwrap_or_default().to_string();
            let role = s["role"].as_str().unwrap_or_default().to_string();
            let missing = s["status"] == "missing";
            let rationale = s["rationale"].as_str().unwrap_or_default().to_string();
            let mut extra: Vec<(&str, Value)> = vec![];
            if comp {
                match role.as_str() {
                    "inlet" => {
                        extra.push(("compressible_bc", json!("fixed")));
                        extra.push(("velocity_value", json!([1.0, 0.0, 0.0])));
                        extra.push(("density_value", json!(1.0)));
                        extra.push(("pressure_value", json!(1.0)));
                        s["rationale"] = json!("inflow state: fixed density, velocity and pressure. The values are PLACEHOLDERS (non-dimensional); set the real free-stream state");
                    }
                    "outlet" => {
                        extra.push(("compressible_bc", json!("extrapolate")));
                        s["rationale"] = json!(format!("{rationale} (compressible solver: zeroth-order extrapolation, valid for supersonic outflow)"));
                    }
                    "wall" | "symmetry" | "moving wall" => {
                        extra.push(("compressible_bc", json!("slip")));
                        s["rationale"] = json!(format!("{rationale} (compressible solver: inviscid slip wall)"));
                    }
                    _ => {}
                }
                s["settings"] = json!([]);
            } else if role == "inlet" {
                extra.push(("turbulence_spec", json!("intensity_length")));
                extra.push(("turbulence_intensity", json!(0.05)));
                extra.push(("turbulence_length", json!(0.01)));
                s["rationale"] = json!(format!(
                    "{rationale}. Turbulence: 5 % intensity and a 0.01 m length scale are PLACEHOLDERS (a length scale of about 5 % of the inlet height is a common first guess)"
                ));
            }
            if !missing {
                continue;
            }
            if let Some(arr) = s["settings"].as_array_mut() {
                for (k, v) in extra {
                    arr.push(json!({"path": format!("boundaries.{name}.{k}"), "value": v}));
                }
            }
        }
    }
    if let Some(notes) = out["notes"].as_array_mut() {
        if comp {
            notes.push(json!("Compressible solver: only compressible_bc, density_value, pressure_value and velocity_value apply; the incompressible pressure reference notes above do not."));
            notes.push(json!("Compressible solver characteristic conditions (compressible_bc): total_inlet (total_pressure_value, temperature_value = total temperature, flow_direction) for a subsonic inlet; pressure_outlet or nonreflecting (pressure_value, reflection_value) for a subsonic outlet; farfield (mach_value, flow_angle, pressure_value, temperature_value) for an open external boundary."));
        }
        if ke {
            notes.push(json!("k-epsilon: walls need no extra entries (turbulence = auto gives them wall functions); at least one wall patch is required."));
        }
    }
    out
}

/// Combine service diagnostics with consistency checks between mesh patches and settings.
pub fn validate_case(service_validation: &Value, mesh_check: Option<&Value>, settings: &Value) -> Value {
    let mut errors: Vec<Value> = service_validation["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut warnings = vec![];
    let boundaries = &settings["boundaries"];
    if let Some(mc) = mesh_check {
        let patches = patch_names(&mc["patches"]);
        for (name, kind) in &patches {
            if kind != "Empty" && kind != "Cyclic" && boundaries.get(name).is_none() {
                warnings.push(json!({"path": format!("boundaries.{name}"), "message": format!("patch '{name}' ({kind}) has no boundary condition; defaults apply"), "fix": "run suggest_boundary_conditions"}));
            }
        }
        if let Some(b) = boundaries.as_object() {
            for k in b.keys() {
                if !patches.iter().any(|(n, _)| n == k) {
                    errors.push(json!({"path": format!("boundaries.{k}"), "message": format!("boundary '{k}' does not match any mesh patch ({})", patches.iter().map(|p| p.0.as_str()).collect::<Vec<_>>().join(", "))}));
                }
            }
        }
    }
    // Reynolds-number plausibility for the laminar solver.
    let nu = settings["physics"]["nu"].as_f64().unwrap_or(0.0);
    let speed = boundaries
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(_, b)| b["velocity_value"].as_array())
        .map(|a| a.iter().filter_map(Value::as_f64).map(|x| x * x).sum::<f64>().sqrt())
        .fold(0.0, f64::max);
    let len = settings["mesh"]["lx"]
        .as_f64()
        .unwrap_or(1.0)
        .max(settings["mesh"]["ly"].as_f64().unwrap_or(1.0));
    let (ke, comp) = (is_k_epsilon(settings), is_compressible(settings));
    if ke && !is_sst(settings) {
        warnings.push(json!({"path": "turbulence.model", "message": "k-epsilon with standard wall functions needs the first cell centre in the log layer (y+ about 30 to 300); a y_plus report on each wall patch shows the value", "fix": "add reports.<name> with type y_plus, then coarsen the wall-adjacent cells if y+ < 11 or refine if y+ > 300"}));
    }
    if is_sst(settings) {
        let mut w = json!({"path": "turbulence.model", "message": "k-omega SST with its automatic wall treatment is accurate for a wall-resolved first cell (y+ about 1, at most 2) and for the log layer (y+ about 30 to 300); y+ between about 3 and 20 is less accurate (the wall shear then relies on the blended law of the wall)", "fix": "add reports.<name> with type y_plus on each wall patch; for a wall-resolved mesh set mesh.wall_layers with mesh.target_y_plus (or mesh.first_layer_height) and mesh.layer_growth about 1.1 to 1.2"});
        if nu > 0.0 && speed > 0.0 {
            if let Some(yp) = estimate_first_cell_y_plus(settings, speed, nu) {
                w["y_plus_estimate"] = json!((yp * 100.0).round() / 100.0);
                let note = if yp > 2.0 && yp < 20.0 {
                    format!(" Estimated first-cell y+ = {yp:.1} (flat-plate friction at lx): in the less accurate range, refine the wall-adjacent cells to y+ <= 1 (mesh.target_y_plus = 1) or coarsen them to y+ > 30")
                } else {
                    format!(" Estimated first-cell y+ = {yp:.1} (flat-plate friction at lx)")
                };
                w["message"] = json!(format!("{}{note}", w["message"].as_str().unwrap_or_default()));
            }
        }
        warnings.push(w);
    }
    if comp {
        let cfl = settings["compressible"]["cfl"].as_f64().unwrap_or(0.4);
        if cfl > 0.8 {
            warnings.push(json!({"path": "compressible.cfl", "message": format!("CFL {cfl} is close to the explicit stability limit of the Runge-Kutta schemes"), "fix": "use cfl <= 0.5 for rk_order 3, lower if the run fails"}));
        }
    }
    if nu > 0.0 && speed > 0.0 && !comp && !ke {
        let re = speed * len / nu;
        if re > 2000.0 {
            warnings.push(json!({"path": "physics.nu", "message": format!("estimated Re = U L / nu = {re:.0}; rcfd currently solves laminar flow only, so the result may be unsteady or physically wrong"), "fix": "increase nu, reduce speed, or expect an unconverged steady run"}));
        }
    }
    if nu == 0.0 {
        warnings.push(json!({"path": "physics.nu", "message": "nu = 0 (inviscid); stability depends entirely on numerical diffusion"}));
    }
    json!({"ok": errors.is_empty(), "errors": errors, "warnings": warnings})
}

/// Classification of one residual history.
fn classify(y: &[f64], tol: f64) -> (&'static str, String) {
    if y.len() < 5 {
        return ("insufficient_data", format!("only {} samples", y.len()));
    }
    if let Some(i) = y.iter().position(|v| !v.is_finite()) {
        return ("non_finite", format!("NaN/inf at sample {} (iteration ~{})", i, i + 1));
    }
    let last = *y.last().unwrap();
    if last <= tol {
        return (
            "converged",
            format!("last value {last:.2e} is below the tolerance {tol:.0e}"),
        );
    }
    let n = y.len();
    let min_so_far = y[..n - 1].iter().cloned().fold(f64::INFINITY, f64::min);
    let w = (n / 4).clamp(4, 50).min(n - 1);
    let tail = &y[n - w - 1..];
    let ups = tail.windows(2).filter(|p| p[1] > p[0]).count();
    let rising = ups as f64 / w as f64;
    if last > 1e3 || (last > 10.0 * min_so_far && rising > 0.6 && last > 1.0) {
        return (
            "diverging",
            format!("last residual {last:.2e} is {:.1e} times the best value {min_so_far:.2e} and rose in {:.0}% of the last {w} steps", last / min_so_far, rising * 100.0),
        );
    }
    let flips = tail.windows(3).filter(|p| (p[1] - p[0]) * (p[2] - p[1]) < 0.0).count();
    let (lo, hi) = tail
        .iter()
        .fold((f64::INFINITY, 0.0f64), |(l, h), v| (l.min(*v), h.max(*v)));
    if flips as f64 > 0.5 * (w as f64 - 1.0) && hi > 2.0 * lo && last > tol {
        return (
            "oscillating",
            format!("residual alternates direction {flips} times in the last {w} steps (range {lo:.1e}..{hi:.1e})"),
        );
    }
    let a = y[n - w - 1].max(1e-300);
    let change = (last / a).ln().abs();
    if change < 0.05 {
        return ("stagnating", format!("residual {last:.2e} changed by less than 5% over the last {w} steps and is above the tolerance {tol:.0e}"));
    }
    if last < a {
        (
            "converging",
            format!("residual fell from {a:.2e} to {last:.2e} over the last {w} steps"),
        )
    } else {
        (
            "rising",
            format!("residual rose from {a:.2e} to {last:.2e} over the last {w} steps"),
        )
    }
}

/// Explain residual histories. `series` maps a monitor name (res_u, res_p, continuity)
/// to its y values; `settings` is the case settings tree.
pub fn explain_residuals(series: &[(String, Vec<f64>)], settings: &Value) -> Value {
    let solver = &settings["solver"];
    let tol_u = solver["tolerance_u"].as_f64().unwrap_or(1e-6);
    let tol_p = solver["tolerance_p"].as_f64().unwrap_or(1e-6);
    let alpha_u = solver["alpha_u"].as_f64().unwrap_or(0.9);
    let alpha_p = solver["alpha_p"].as_f64().unwrap_or(1.0);
    let algo = solver["algorithm"].as_str().unwrap_or("simplec");
    let conv = solver["convection"].as_str().unwrap_or("upwind");
    let comp = is_compressible(settings);
    let steady_comp = comp && settings["compressible"]["mode"] == "steady";
    if comp && !steady_comp {
        // res_u / res_p are per-step changes of a time-accurate run: they do not converge to zero
        let mut v = json!({
            "verdict": "transient compressible run",
            "findings": [],
            "suggested_changes": [],
            "advice": [
                "A transient compressible run has no residual convergence: res_u is the largest relative density change per step and stays finite. Judge it by the conserved totals and the profiles.",
                "If the run stops early with a failure message, lower compressible.cfl, or switch to a first-order reconstruction or the rusanov flux to find out whether the limiter or the flux is the cause."
            ],
            "caveats": "Pattern rules over residual histories; they suggest likely causes, not proof. Suggested changes are not applied; use set_setting (requires approval)."
        });
        if let Some((_, y)) = series.iter().find(|(n, _)| n == "continuity") {
            let (pat, ev) = classify(y, 1e-10);
            v["findings"] = json!([{"series": "continuity", "pattern": pat, "evidence": ev, "samples": y.len()}]);
        }
        return v;
    }
    let mut findings = vec![];
    let mut patterns: Vec<&str> = vec![];
    for (name, y) in series {
        let tol = if name == "res_p" { tol_p } else { tol_u };
        if !(name == "res_u" || name == "res_p" || name == "continuity" || name == "res_nut" || name == "res_rho") {
            continue;
        }
        if name == "res_rho" && !steady_comp {
            continue;
        }
        let tol = if name == "continuity" { tol_p } else { tol };
        let tol = if name == "res_rho" {
            settings["compressible"]["steady_tolerance"].as_f64().unwrap_or(1e-8)
        } else {
            tol
        };
        let (pat, ev) = classify(y, tol);
        patterns.push(pat);
        findings.push(json!({"series": name, "pattern": pat, "evidence": ev, "samples": y.len()}));
    }
    let has = |p: &str| patterns.contains(&p);
    let mut changes: Vec<Value> = vec![];
    let mut advice: Vec<String> = vec![];
    let verdict;
    let high_order = !matches!(conv, "upwind");
    if findings.is_empty() || patterns.iter().all(|p| *p == "insufficient_data") {
        verdict = "not enough data";
        advice.push("Run more iterations (at least ~10) before diagnosing.".into());
    } else if has("non_finite") || has("diverging") {
        verdict = if has("non_finite") {
            "diverged (NaN/inf)"
        } else {
            "diverging"
        };
        advice.push(
            "Check the mesh first (assess_mesh): high non-orthogonality or skewness commonly causes divergence.".into(),
        );
        advice.push("Check boundary conditions (validate_case): a missing pressure reference with an outlet, or an inconsistent inlet velocity, can blow up the solution.".into());
        changes.push(json!({"path": "solver.alpha_u", "value": (((alpha_u * 0.6) * 100.0).round() / 100.0).max(0.05), "reason": "stronger under-relaxation of velocity"}));
        if (algo == "simple" || algo == "piso" || algo == "pimple") && alpha_p > 0.4 {
            changes.push(json!({"path": "solver.alpha_p", "value": 0.3, "reason": "SIMPLE needs pressure relaxation around 0.3 (SIMPLEC tolerates 1.0)"}));
        }
        if high_order {
            changes.push(json!({"path": "solver.convection", "value": "upwind", "reason": "first-order upwind is the most robust scheme; switch back once the run is stable"}));
        }
        if algo == "piso" || algo == "pimple" {
            advice.push("For transient runs also reduce dt (or set solver.max_courant).".into());
        }
    } else if has("oscillating") {
        verdict = "oscillating";
        advice.push("Oscillating residuals usually mean under-relaxation is too weak or the flow is physically unsteady at this Reynolds number.".into());
        changes.push(json!({"path": "solver.alpha_u", "value": ((alpha_u * 0.8) * 100.0).round() / 100.0, "reason": "damp the oscillation"}));
        if high_order {
            changes.push(json!({"path": "solver.convection", "value": "upwind", "reason": "rule out scheme-induced oscillation"}));
        }
        advice.push("If it persists with strong relaxation, the flow may be genuinely unsteady: use a transient algorithm (piso/pimple).".into());
    } else if has("stagnating") {
        verdict = "stagnated above tolerance";
        advice.push("Stagnation can mean the tolerance is unreachable (limit cycle), a pressure level is undetermined, or the mesh is too coarse/poor.".into());
        advice.push("Check that exactly the intended patches fix the pressure and run assess_mesh.".into());
        if solver["non_orthogonal_correctors"].as_i64().unwrap_or(0) == 0 {
            changes.push(json!({"path": "solver.non_orthogonal_correctors", "value": 1, "reason": "non-orthogonal meshes stall without correctors"}));
        }
    } else if patterns.iter().all(|p| *p == "converged") {
        verdict = "converged";
        advice.push(
            "All residuals are below tolerance. Verify the answer with monitors/reports, not residuals alone.".into(),
        );
    } else if has("converging") || has("converged") {
        verdict = "converging";
        advice.push("Residuals are falling; continue the run.".into());
    } else {
        verdict = "inconclusive";
        advice.push("No clear pattern; inspect the monitors visually.".into());
    }
    if steady_comp && (has("diverging") || has("non_finite") || has("oscillating")) {
        advice.push("Compressible steady mode marches in pseudo-time: lower compressible.cfl and use a more dissipative flux (hllc or rusanov) before tuning anything else.".into());
        changes.push(json!({"path": "compressible.cfl", "value": (settings["compressible"]["cfl"].as_f64().unwrap_or(0.4) * 0.5 * 100.0).round() / 100.0, "reason": "smaller explicit time step"}));
    }
    if series.iter().any(|(n, _)| n == "res_nut")
        && (has("diverging") || has("non_finite") || has("oscillating") || has("stagnating"))
    {
        advice.push("res_nut is the change of the eddy viscosity: if it does not fall, lower turbulence.relax / turbulence.relax_nut, check the wall-adjacent y+ (y_plus report) and use turbulence.convection = upwind.".into());
        let relax = settings["turbulence"]["relax"].as_f64().unwrap_or(0.7);
        changes.push(json!({"path": "turbulence.relax", "value": ((relax * 0.7) * 100.0).round() / 100.0, "reason": "stronger under-relaxation of k and epsilon"}));
    }
    json!({
        "verdict": verdict,
        "findings": findings,
        "suggested_changes": changes,
        "advice": advice,
        "caveats": "Pattern rules over residual histories; they suggest likely causes, not proof. Suggested changes are not applied; use set_setting (requires approval)."
    })
}
