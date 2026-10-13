//! Expression support for case settings (F-104): named expressions
//! (`expressions.<name>.definition`), expression-valued boundary conditions
//! (`velocity_expression_x/y/z`, `temperature_expression`,
//! `heat_flux_expression`) and expression reports (`type = "expression"`).
//!
//! Boundary expressions are functions of the face position `x, y, z` and the
//! time `t` and may use the physical constants `rho`, `nu`, `pr`, `beta`,
//! `g_x`, `g_y`, `g_z`, `cp`, `conductivity`, `t_ref` (and `pi`, `e`), other named expressions and the
//! builtin functions of [`rcfd_core::expr`]. Report expressions may
//! additionally refer to other reports by name.

use crate::case::{Case, Diagnostic};
use rcfd_core::expr::{Compiled, Context, Expr, ExprError};
use rcfd_core::units::{Dimension, Quantity};
use serde_json::Value;

fn get_s(c: &Case, path: &str) -> String {
    c.tree
        .get(path)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

fn get_f(c: &Case, path: &str) -> f64 {
    c.tree.get(path).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn diag(path: &str, e: &ExprError, src: &str) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: format!("{e}\n{}", e.render(src)),
    }
}

/// SI dimension of a report of the given type and field.
pub fn report_dimension(kind: &str, field: &str) -> Dimension {
    let field_dim = match field {
        "ux" | "uy" | "uz" | "speed" => Dimension::VELOCITY,
        "p" => Dimension::VELOCITY * Dimension::VELOCITY,
        "T" => Dimension::TEMPERATURE,
        "vorticity_z" => Dimension::ANGULAR_VELOCITY,
        "k" => Dimension::VELOCITY * Dimension::VELOCITY,
        "epsilon" => Dimension::VELOCITY * Dimension::VELOCITY * Dimension::ANGULAR_VELOCITY,
        "nut" => Dimension::KINEMATIC_VISCOSITY,
        "wall_distance" => Dimension::LENGTH,
        "rho" => Dimension::DENSITY,
        _ => Dimension::NONE,
    };
    match kind {
        "force" => Dimension::FORCE,
        "moment" => Dimension::FORCE * Dimension::LENGTH,
        "mass_flow" => Dimension::MASS / Dimension::TIME,
        "heat_flow" => Dimension::POWER,
        "volume_integral" => field_dim * Dimension::LENGTH.powi(3),
        "area_integral" => field_dim * Dimension::LENGTH.powi(2),
        "volume_average" | "area_average" | "probe" | "minimum" | "maximum" => field_dim,
        _ => Dimension::NONE,
    }
}

/// Context holding the physical constants of the case and its named expressions
/// (no variables yet).
pub fn constants_context(case: &Case) -> Context {
    let mut ctx = Context::new();
    ctx.add_const("rho", Quantity::from_si(get_f(case, "physics.rho"), Dimension::DENSITY));
    ctx.add_const(
        "nu",
        Quantity::from_si(get_f(case, "physics.nu"), Dimension::KINEMATIC_VISCOSITY),
    );
    let (rho, nu) = (get_f(case, "physics.rho"), get_f(case, "physics.nu"));
    let (cp, kth) = (get_f(case, "physics.cp"), get_f(case, "physics.conductivity"));
    ctx.add_const("cp", Quantity::from_si(cp, Dimension::SPECIFIC_HEAT));
    ctx.add_const("conductivity", Quantity::from_si(kth, Dimension::CONDUCTIVITY));
    ctx.add_const(
        "t_ref",
        Quantity::from_si(get_f(case, "physics.reference_temperature"), Dimension::TEMPERATURE),
    );
    ctx.add_const(
        "pr",
        Quantity::from_si(if kth > 0.0 { nu * rho * cp / kth } else { 0.0 }, Dimension::NONE),
    );
    ctx.add_const(
        "beta",
        Quantity::from_si(
            get_f(case, "physics.thermal_expansion"),
            Dimension::TEMPERATURE.powi(-1),
        ),
    );
    let g = case
        .tree
        .get("physics.gravity")
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    for (i, name) in ["g_x", "g_y", "g_z"].iter().enumerate() {
        let v = g.get(i).and_then(Value::as_f64).unwrap_or(0.0);
        ctx.add_const(name, Quantity::from_si(v, Dimension::ACCELERATION));
    }
    if let Ok(names) = case.tree.names("expressions") {
        for n in names {
            let src = get_s(case, &format!("expressions.{n}.definition"));
            if let Ok(e) = Expr::parse(&src) {
                ctx.add_named(&n, e);
            }
        }
    }
    ctx
}

/// Constants context plus the spatial variables `x, y, z` (slots 0..3) and time `t` (slot 3).
pub fn spatial_context(case: &Case) -> Context {
    let mut ctx = constants_context(case);
    for n in ["x", "y", "z"] {
        ctx.add_var(n, Dimension::LENGTH);
    }
    ctx.add_var("t", Dimension::TIME);
    ctx
}

/// A boundary expression compiled to a given dimension.
pub fn compile_bc(
    case: &Case,
    path: &str,
    expected: Dimension,
    ctx: &Context,
) -> Result<Option<Compiled>, Vec<Diagnostic>> {
    let src = get_s(case, path);
    if src.trim().is_empty() {
        return Ok(None);
    }
    let one = |e: &ExprError| vec![diag(path, e, &src)];
    let ex = Expr::parse(&src).map_err(|e| one(&e))?;
    let c = ex.compile(ctx).map_err(|e| one(&e))?;
    if c.dimension() != expected {
        return Err(vec![Diagnostic {
            path: path.into(),
            line: None,
            message: format!(
                "expression has dimension '{}' but '{}' is required",
                c.dimension(),
                expected
            ),
        }]);
    }
    Ok(Some(c))
}

/// Names and dimensions of the reports a report expression may refer to
/// (every non-expression report of the case).
pub fn report_symbols(case: &Case) -> Vec<(String, Dimension)> {
    case.tree
        .names("reports")
        .unwrap_or_default()
        .into_iter()
        .filter(|n| get_s(case, &format!("reports.{n}.type")) != "expression")
        .map(|n| {
            let field = get_s(case, &format!("reports.{n}.field"));
            let kind = get_s(case, &format!("reports.{n}.type"));
            let mut d = report_dimension(&kind, &field);
            if get_s(case, "solver.algorithm") == "compressible"
                && field == "p"
                && matches!(
                    kind.as_str(),
                    "volume_average"
                        | "area_average"
                        | "probe"
                        | "minimum"
                        | "maximum"
                        | "volume_integral"
                        | "area_integral"
                )
            {
                // static pressure [Pa] instead of the kinematic pressure of the incompressible solvers
                d = d * Dimension::DENSITY;
            }
            (n, d)
        })
        .collect()
}

/// Context for report expressions: constants, named expressions and one variable per other report.
pub fn report_context(case: &Case) -> (Context, Vec<String>) {
    let mut ctx = constants_context(case);
    let syms = report_symbols(case);
    for (n, d) in &syms {
        ctx.add_var(n, *d);
    }
    (ctx, syms.into_iter().map(|s| s.0).collect())
}

/// Check every expression of the case (named, boundary, report) and return all problems.
pub fn check_case_expressions(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    // named expressions: syntax and dimensional consistency in the union context
    let (mut all, _) = report_context(case);
    for n in ["x", "y", "z"] {
        all.add_var(n, Dimension::LENGTH);
    }
    all.add_var("t", Dimension::TIME);
    for n in case.tree.names("expressions").unwrap_or_default() {
        let p = format!("expressions.{n}.definition");
        let src = get_s(case, &p);
        if src.trim().is_empty() {
            out.push(Diagnostic {
                path: p,
                line: None,
                message: "empty expression".into(),
            });
            continue;
        }
        match Expr::parse(&src) {
            Err(e) => out.push(diag(&p, &e, &src)),
            Ok(ex) => {
                if let Err(e) = ex.compile(&all) {
                    out.push(diag(&p, &e, &src));
                }
            }
        }
    }
    let ctx = spatial_context(case);
    for n in case.tree.names("boundaries").unwrap_or_default() {
        let has_vel_expr = ["x", "y", "z"].iter().any(|c| {
            !get_s(case, &format!("boundaries.{n}.velocity_expression_{c}"))
                .trim()
                .is_empty()
        });
        if has_vel_expr && get_s(case, &format!("boundaries.{n}.velocity")) != "fixed" {
            out.push(Diagnostic {
                path: format!("boundaries.{n}.velocity"),
                line: None,
                message: "velocity expressions require velocity = \"fixed\"".into(),
            });
        }
        for (f, mode, want) in [
            ("temperature_expression", "temperature", "fixed"),
            ("heat_flux_expression", "temperature", "heat_flux"),
        ] {
            let has = !get_s(case, &format!("boundaries.{n}.{f}")).trim().is_empty();
            if has && get_s(case, &format!("boundaries.{n}.{mode}")) != want {
                out.push(Diagnostic {
                    path: format!("boundaries.{n}.{mode}"),
                    line: None,
                    message: format!("{f} requires {mode} = \"{want}\""),
                });
            }
        }
        for (f, dim) in [
            ("velocity_expression_x", Dimension::VELOCITY),
            ("velocity_expression_y", Dimension::VELOCITY),
            ("velocity_expression_z", Dimension::VELOCITY),
            ("temperature_expression", Dimension::TEMPERATURE),
            ("heat_flux_expression", Dimension::HEAT_FLUX),
        ] {
            if let Err(e) = compile_bc(case, &format!("boundaries.{n}.{f}"), dim, &ctx) {
                out.extend(e);
            }
        }
    }
    let (rctx, _) = report_context(case);
    for n in case.tree.names("reports").unwrap_or_default() {
        if get_s(case, &format!("reports.{n}.type")) != "expression" {
            continue;
        }
        let p = format!("reports.{n}.expression");
        let src = get_s(case, &p);
        match Expr::parse(&src) {
            Err(e) => out.push(diag(&p, &e, &src)),
            Ok(ex) => {
                if let Err(e) = ex.compile(&rctx) {
                    out.push(diag(&p, &e, &src));
                }
            }
        }
    }
    out
}
