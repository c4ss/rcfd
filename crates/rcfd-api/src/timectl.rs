//! Case-tree front end of the adaptive time-step control (F-209, ADR 0050 D7): reads the additive `solver.*` / `output.*` keys
//! (all with unset sentinels), validates them with located errors and builds the library driver
//! [`rcfd_fv::stepctl::ControlledRun`]. With every key at its sentinel nothing is built and the existing fixed-dt /
//! `solver.max_courant` code path runs unchanged.

use crate::case::{Case, Diagnostic};
use rcfd_fv::stepctl::{ControlledRun, StepController, StepSettings};

fn num(case: &Case, path: &str) -> f64 {
    case.get(path).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn err(path: &str, message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }]
}

/// Whether any of the new keys is set (the legacy `max_courant` alone does not activate the new path).
pub(crate) fn requested(case: &Case) -> bool {
    [
        "solver.max_courant_rms",
        "solver.dt_growth",
        "solver.dt_shrink",
        "solver.dt_safety",
        "solver.min_dt",
        "solver.max_dt",
        "solver.error_tolerance",
        "solver.period",
        "output.write_time_interval",
    ]
    .iter()
    .any(|p| num(case, p) != 0.0)
        || num(case, "solver.steps_per_period") != 0.0
        || case.get("solver.extrapolate_initial").ok().and_then(|v| v.as_bool()) == Some(true)
        || case
            .get("output.write_times")
            .ok()
            .and_then(|v| v.as_array().map(|a| !a.is_empty()))
            == Some(true)
}

/// Build the driver. `transient` says the run is an incompressible transient PISO/PIMPLE one; `plain` that it carries no
/// additional physics that must be advanced with the step (needed by the step-doubling error control).
pub(crate) fn from_case(
    case: &Case,
    transient: bool,
    plain: bool,
    dt: f64,
    end_time: f64,
) -> Result<Option<ControlledRun>, Vec<Diagnostic>> {
    if !requested(case) {
        return Ok(None);
    }
    let first = [
        "solver.max_courant_rms",
        "solver.dt_growth",
        "solver.dt_shrink",
        "solver.dt_safety",
        "solver.min_dt",
        "solver.max_dt",
        "solver.error_tolerance",
        "solver.period",
        "solver.steps_per_period",
        "solver.extrapolate_initial",
        "output.write_times",
        "output.write_time_interval",
    ]
    .into_iter()
    .find(|p| match case.get(p) {
        Ok(v) => {
            v.as_f64().is_some_and(|x| x != 0.0)
                || v.as_bool() == Some(true)
                || v.as_array().is_some_and(|a| !a.is_empty())
        }
        Err(_) => false,
    })
    .unwrap_or("solver.max_courant_rms");
    if !transient {
        return Err(err(
            first,
            "adaptive time stepping needs a transient incompressible run (solver.algorithm = piso or pimple)",
        ));
    }
    let times: Vec<f64> = case
        .get("output.write_times")
        .ok()
        .and_then(|v| v.as_array().map(|a| a.iter().filter_map(|x| x.as_f64()).collect()))
        .unwrap_or_default();
    let or = |x: f64, d: f64| if x == 0.0 { d } else { x };
    let st = StepSettings {
        co_max: num(case, "solver.max_courant"),
        co_rms: num(case, "solver.max_courant_rms"),
        safety: or(num(case, "solver.dt_safety"), 0.9),
        growth: or(num(case, "solver.dt_growth"), 1.2),
        shrink: or(num(case, "solver.dt_shrink"), 0.5),
        min_dt: or(num(case, "solver.min_dt"), dt * 1e-3),
        max_dt: or(num(case, "solver.max_dt"), dt),
        error_tol: num(case, "solver.error_tolerance"),
        period: num(case, "solver.period"),
        steps_per_period: num(case, "solver.steps_per_period") as usize,
        write_times: times,
        write_interval: num(case, "output.write_time_interval"),
    };
    if let Err((key, message)) = st.validate() {
        let path = match key {
            "max_courant" => "solver.max_courant".to_string(),
            "write_times" | "write_time_interval" => format!("output.{key}"),
            k => format!("solver.{k}"),
        };
        return Err(err(&path, message));
    }
    if st.write_times.iter().any(|t| *t > end_time) {
        return Err(err("output.write_times", "a write time lies beyond solver.end_time"));
    }
    if st.error_tol > 0.0 {
        if !plain {
            return Err(err(
                "solver.error_tolerance",
                "step-doubling error control needs a laminar constant-density run without energy, scalars, turbulence, rheology, \
                 sources, lumped components, dynamic mesh, statistics or expression boundary conditions",
            ));
        }
        if case
            .get("solver.time_scheme")
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .as_deref()
            != Some("euler")
        {
            return Err(err(
                "solver.error_tolerance",
                "step-doubling error control is implemented for solver.time_scheme = \"euler\" only",
            ));
        }
    }
    let extrapolate = case.get("solver.extrapolate_initial").ok().and_then(|v| v.as_bool()) == Some(true);
    let dt0 = if st.period > 0.0 {
        st.period / st.steps_per_period as f64
    } else {
        dt
    };
    Ok(Some(ControlledRun::new(StepController::new(st, dt0), extrapolate)))
}
