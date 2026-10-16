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
use rcfd_core::table::{CsvSpec, Extrapolation, Table};
use rcfd_core::units::{Dimension, Quantity};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Message used when a boundary expression or profile is not finite on some faces.
pub const NONFINITE_BC: &str = "the boundary value is not finite on some faces: a lookup table or profile does not cover the patch (extend its data or set tables.<name>.extrapolation = \"clamp\")";

/// Default units of the column names that profile files use.
const DEFAULT_UNITS: [(&str, &str); 9] = [
    ("x", "m"),
    ("y", "m"),
    ("z", "m"),
    ("t", "s"),
    ("ux", "m/s"),
    ("uy", "m/s"),
    ("uz", "m/s"),
    ("T", "K"),
    ("heat_flux", "W/m^2"),
];

/// Names that a table may not take (they would shadow built-ins).
const RESERVED: [&str; 31] = [
    "sqrt", "abs", "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "exp", "ln", "log10", "pow", "min", "max",
    "step", "if", "hypot", "clamp", "sign", "x", "y", "z", "t", "pi", "e", "rho", "nu", "pr", "beta", "cp",
];

/// Read the case's lookup tables (`tables.<name>`): the loaded tables and one diagnostic per
/// table that cannot be built (missing file, bad CSV, bad settings).
pub fn load_tables(case: &Case) -> (BTreeMap<String, Arc<Table>>, Vec<Diagnostic>) {
    let mut out = BTreeMap::new();
    let mut diags = Vec::new();
    for n in case.tree.names("tables").unwrap_or_default() {
        let p = |f: &str| format!("tables.{n}.{f}");
        let bad = |path: String, message: String| Diagnostic {
            path,
            line: None,
            message,
        };
        if RESERVED.contains(&n.as_str())
            || !n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            || n.starts_with(|c: char| c.is_ascii_digit())
        {
            diags.push(bad(
                format!("tables.{n}"),
                format!(
                    "'{n}' cannot be used as a table name (use letters, digits and underscores, not a built-in name)"
                ),
            ));
            continue;
        }
        let strings = |f: &str| -> Vec<String> {
            case.tree
                .get(&p(f))
                .ok()
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default()
                .iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        };
        let file = get_s(case, &p("file"));
        let text = if !file.is_empty() {
            let path = case.base_dir.join(&file);
            if let Some(root) = &case.sandbox {
                let inside = !std::path::Path::new(&file)
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
                    && path
                        .canonicalize()
                        .ok()
                        .zip(root.canonicalize().ok())
                        .map_or(true, |(p, r)| p.starts_with(r));
                if !inside {
                    diags.push(bad(p("file"), format!("table file '{file}' is outside the sandbox")));
                    continue;
                }
            }
            match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(e) => {
                    diags.push(bad(
                        p("file"),
                        format!("cannot read table file {}: {e}", path.display()),
                    ));
                    continue;
                }
            }
        } else {
            let d = get_s(case, &p("data"));
            if d.trim().is_empty() {
                diags.push(bad(p("file"), "a table needs a file or inline data".into()));
                continue;
            }
            d
        };
        let inputs = strings("inputs");
        if inputs.is_empty() || inputs.len() > 3 {
            diags.push(bad(p("inputs"), "name 1 to 3 input columns".into()));
            continue;
        }
        let mut units: Vec<(String, String)> = Vec::new();
        let mut unit_err = false;
        for e in strings("units") {
            match e.split_once('=') {
                Some((c, u)) => units.push((c.trim().to_string(), u.trim().to_string())),
                None => {
                    diags.push(bad(p("units"), format!("'{e}' is not of the form \"column = unit\"")));
                    unit_err = true;
                }
            }
        }
        if unit_err {
            continue;
        }
        for (c, u) in DEFAULT_UNITS {
            if !units.iter().any(|(k, _)| k == c) {
                units.push((c.to_string(), u.to_string()));
            }
        }
        let spec = CsvSpec {
            inputs,
            outputs: strings("outputs"),
            units,
            extrapolation: Extrapolation::parse(&get_s(case, &p("extrapolation"))).unwrap_or_default(),
        };
        match Table::from_csv(&n, &text, &spec) {
            Ok(t) => {
                out.insert(n, Arc::new(t));
            }
            Err(e) => diags.push(bad(if file.is_empty() { p("data") } else { p("file") }, e.to_string())),
        }
    }
    (out, diags)
}

/// Source of a boundary expression: the explicit setting, else a call of the patch's profile table
/// (`boundaries.<patch>.profile`) for the matching column.
pub fn bc_source(case: &Case, path: &str, ctx: &Context) -> String {
    let src = get_s(case, path);
    if !src.trim().is_empty() {
        return src;
    }
    let mut it = path.splitn(3, '.');
    let (Some("boundaries"), Some(patch), Some(field)) = (it.next(), it.next(), it.next()) else {
        return src;
    };
    let col = match field {
        "velocity_expression_x" => "ux",
        "velocity_expression_y" => "uy",
        "velocity_expression_z" => "uz",
        "temperature_expression" => "T",
        "heat_flux_expression" => "heat_flux",
        _ => return src,
    };
    let prof = get_s(case, &format!("boundaries.{patch}.profile"));
    let func = format!("{prof}_{col}");
    match ctx.table(&func) {
        Some(t) if !prof.is_empty() => {
            let args: Vec<&str> = t.inputs.iter().map(|c| c.name.as_str()).collect();
            format!("{func}({})", args.join(", "))
        }
        _ => src,
    }
}

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
        "omega" => Dimension::ANGULAR_VELOCITY,
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
        "vector_flux" => Dimension::FORCE,
        "volume_integral" => field_dim * Dimension::LENGTH.powi(3),
        "area_integral" => field_dim * Dimension::LENGTH.powi(2),
        "volume_average"
        | "area_average"
        | "probe"
        | "minimum"
        | "maximum"
        | "volume_std"
        | "area_std"
        | "mass_weighted_average"
        | "area_min"
        | "area_max" => field_dim,
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
    for (n, t) in load_tables(case).0 {
        ctx.add_table(&n, t.clone(), 0);
        for (i, c) in t.outputs.iter().enumerate() {
            ctx.add_table(&format!("{n}_{}", c.name), t.clone(), i);
        }
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
    let src = bc_source(case, path, ctx);
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
                        | "volume_std"
                        | "area_std"
                        | "mass_weighted_average"
                        | "area_min"
                        | "area_max"
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
    let mut out = load_tables(case).1;
    let load_failed: Vec<String> = out.iter().map(|d| d.path.clone()).collect();
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
        let prof = get_s(case, &format!("boundaries.{n}.profile"));
        if !prof.is_empty() && !load_failed.iter().any(|p| p.starts_with(&format!("tables.{prof}"))) {
            let pp = format!("boundaries.{n}.profile");
            match ctx.table(&prof) {
                None => out.push(Diagnostic {
                    path: pp,
                    line: None,
                    message: format!("no usable table '{prof}' (define tables.{prof} with a file or data)"),
                }),
                Some(t) => {
                    if let Some(c) = t
                        .inputs
                        .iter()
                        .find(|c| !["x", "y", "z", "t"].contains(&c.name.as_str()))
                    {
                        out.push(Diagnostic {
                            path: pp,
                            line: None,
                            message: format!(
                                "profile table '{prof}' has input column '{}': a boundary profile can only depend on x, y, z and t",
                                c.name
                            ),
                        });
                    } else if !["ux", "uy", "uz", "T", "heat_flux"]
                        .iter()
                        .any(|c| ctx.table(&format!("{prof}_{c}")).is_some())
                    {
                        out.push(Diagnostic {
                            path: pp,
                            line: None,
                            message: format!("profile table '{prof}' has none of the columns ux, uy, uz, T, heat_flux"),
                        });
                    }
                }
            }
        }
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
