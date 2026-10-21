//! Zone sources and fixed values for the temperature and the user scalars (F-201, ADR 0019 D2).
//!
//! `zone_sources.<name>` adds a volumetric source (optionally with an implicit linear part
//! `S = value - k (phi - reference)`) to the energy equation or to a `scalars` / transported `species` entry in a
//! cell zone, or holds the cells of the zone at a value (`kind = "fixed_value"`). The solvers take the result
//! through `rcfd_fv::scalar::SweepExtras`.

use crate::case::{Case, Diagnostic};
use crate::exprs;
use rcfd_core::expr::Compiled;
use rcfd_core::settings::Schema;
use rcfd_core::Dimension;
use rcfd_fv::energy::EnergySolver;
use rcfd_fv::FvGeometry;

/// Names of the configured zone sources.
pub fn names(case: &Case) -> Vec<String> {
    case.tree.names("zone_sources").unwrap_or_default()
}

/// Is any zone source configured?
pub fn enabled(case: &Case) -> bool {
    !names(case).is_empty()
}

/// Schema of the `zone_sources` collection.
pub fn schema() -> Schema {
    Schema::named(
        Schema::object()
            .doc("A volumetric source or a fixed value in a cell zone for the temperature or a scalar / species (steady SIMPLE/SIMPLEC and transient PISO/PIMPLE; ADR 0019)")
            .field(
                "target",
                Schema::string("temperature")
                    .doc("temperature, or the name of a scalars.<s> / transported species.<s> entry (the balance species follows from the closure and cannot be a target)"),
            )
            .field(
                "kind",
                Schema::enumeration("source", &["source", "fixed_value"])
                    .doc("source: add a volumetric source; fixed_value: hold the cells of the zone at value"),
            )
            .field(
                "zone",
                Schema::enumeration("all", &["all", "box", "cell_zone", "register"])
                    .doc("Where it acts: the whole mesh, the cells whose centres lie in the axis-aligned box [box_min, box_max], a named mesh cell zone or a cell register (registers.<name>)"),
            )
            .field("register", Schema::string("").doc("Name of the register for zone = \"register\""))
            .field("box_min", Schema::list(Schema::float(0.0)).doc("Lower corner [m] of the box zone"))
            .field("box_max", Schema::list(Schema::float(0.0)).doc("Upper corner [m] of the box zone"))
            .field("cell_zone", Schema::string("").doc("Name of the mesh cell zone"))
            .field(
                "value",
                Schema::float(0.0)
                    .doc("source: volumetric source, W/m^3 for the temperature (divided by physics.rho * physics.cp) and value per second for a scalar; fixed_value: the value in K or in the unit of the scalar"),
            )
            .field(
                "value_expression",
                Schema::string("").doc("Expression of x, y, z and t at the cell centres, re-evaluated every step; overrides value. Dimension: temperature source W/m^3, temperature fixed value K, scalar fixed value dimensionless, scalar source dimensionless or 1/s"),
            )
            .field(
                "linear_coefficient",
                Schema::float(0.0).doc("source only: k >= 0 [1/s], treated implicitly: S = value - k (phi - reference)"),
            )
            .field(
                "reference",
                Schema::float(0.0).doc("source only: reference level of the linear part (K for the temperature)"),
            ),
    )
    .doc("Zone sources and fixed values for the temperature and the user scalars (ADR 0019)")
}

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

fn d(path: String, message: String) -> Diagnostic {
    Diagnostic {
        path,
        line: None,
        message,
    }
}

/// Compile `path` as an expression whose dimension is one of `dims`.
pub(crate) fn compile_any(
    case: &Case,
    path: &str,
    dims: &[Dimension],
    ctx: &rcfd_core::expr::Context,
) -> Result<Option<Compiled>, Vec<Diagnostic>> {
    let mut last = None;
    for dim in dims {
        match exprs::compile_bc(case, path, *dim, ctx) {
            Ok(c) => return Ok(c),
            Err(e) => last = Some(e),
        }
    }
    let mut e = last.unwrap_or_default();
    if dims.len() > 1 {
        if let Some(first) = e.first_mut() {
            let list: Vec<String> = dims.iter().map(|x| format!("'{x}'")).collect();
            first.message = format!("{} (accepted dimensions: {})", first.message, list.join(" or "));
        }
    }
    Err(e)
}

fn value_dims(temperature: bool, fixed: bool) -> Vec<Dimension> {
    match (temperature, fixed) {
        (true, true) => vec![Dimension::TEMPERATURE],
        (true, false) => vec![Dimension::POWER / Dimension::LENGTH.powi(3)],
        (false, true) => vec![Dimension::NONE],
        (false, false) => vec![Dimension::NONE, Dimension::TIME.powi(-1)],
    }
}

/// Settings checks (no mesh needed).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let all = names(case);
    if all.is_empty() {
        return out;
    }
    let alg = gs(case, "solver.algorithm");
    let scalar_names: Vec<String> = ["scalars", "species"]
        .iter()
        .flat_map(|s| case.tree.names(s).unwrap_or_default())
        .collect();
    let balance = crate::scalars::balance_of(case);
    let ctx = exprs::spatial_context(case);
    for n in &all {
        let p = |f: &str| format!("zone_sources.{n}.{f}");
        if crate::comp::enabled(case) || crate::vd::enabled(case) || alg == "coupled" {
            out.push(d(
                format!("zone_sources.{n}"),
                format!("zone sources act in the incompressible steady (simple, simplec) and transient (piso, pimple) algorithms, not with \"{alg}\" / the variable-density and compressible solvers"),
            ));
            continue;
        }
        let target = gs(case, &p("target"));
        let temperature = target == "temperature";
        if temperature {
            if !case
                .tree
                .get("energy.enabled")
                .ok()
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                out.push(d(
                    p("target"),
                    "target = \"temperature\" needs energy.enabled = true".into(),
                ));
            }
            if crate::radiation::enabled(case) {
                out.push(d(
                    p("target"),
                    "zone sources on the temperature are not combined with the radiation model".into(),
                ));
            }
        } else if target == balance && !balance.is_empty() {
            out.push(d(
                p("target"),
                format!("'{target}' is the balance species, closed by difference; put the source on another species"),
            ));
        } else if !scalar_names.contains(&target) {
            let mut known = vec!["temperature".to_string()];
            known.extend(scalar_names.iter().cloned());
            out.push(d(
                p("target"),
                format!("unknown target '{target}' (known: {})", known.join(", ")),
            ));
        }
        let fixed = gs(case, &p("kind")) == "fixed_value";
        let k = gf(case, &p("linear_coefficient"));
        if k < 0.0 {
            out.push(d(
                p("linear_coefficient"),
                "must not be negative (a negative coefficient is an implicit source and destabilises the solve)".into(),
            ));
        }
        if fixed && k != 0.0 {
            out.push(d(
                p("linear_coefficient"),
                "kind = \"fixed_value\" has no linear part; use kind = \"source\"".into(),
            ));
        }
        if fixed && gf(case, &p("reference")) != 0.0 {
            out.push(d(
                p("reference"),
                "kind = \"fixed_value\" has no reference level".into(),
            ));
        }
        out.extend(crate::sources::check_zone(case, &format!("zone_sources.{n}")));
        if let Err(mut e) = compile_any(case, &p("value_expression"), &value_dims(temperature, fixed), &ctx) {
            out.append(&mut e);
        }
    }
    out
}

struct Item {
    name: String,
    /// `None` is the temperature.
    scalar: Option<String>,
    fixed: bool,
    mask: Vec<bool>,
    value: f64,
    expr: Option<Compiled>,
    k: f64,
    reference: f64,
}

/// Evaluated terms of a target at one time.
#[derive(Debug, Clone, Default)]
pub struct ZoneTerms {
    /// Source per cell in the units of the equation (K/s for the temperature, `[phi]/s` for a scalar), including the
    /// constant part `k * reference` of the linear sources.
    pub source: Vec<f64>,
    /// Implicit linear coefficient per cell (1/s) when any item has one.
    pub sink: Option<Vec<f64>>,
    /// Fixed cells with their values when any item fixes cells.
    pub fixed: Option<Vec<Option<f64>>>,
}

/// The zone sources of a case with their cells.
pub struct ZoneSources {
    items: Vec<Item>,
    rho_cp: f64,
    centres: Vec<[f64; 3]>,
}

impl ZoneSources {
    /// Build from the case; `None` when none is configured. Located errors for empty zones and overlapping fixed zones.
    pub fn from_case(case: &Case, geom: &FvGeometry) -> Result<Option<ZoneSources>, Vec<Diagnostic>> {
        if !enabled(case) {
            return Ok(None);
        }
        let mut errs = Vec::new();
        let ctx = exprs::spatial_context(case);
        let mut items: Vec<Item> = Vec::new();
        let mut owners: std::collections::HashMap<Option<String>, Vec<Option<String>>> = Default::default();
        let rho_cp = gf(case, "physics.rho") * gf(case, "physics.cp");
        let centres: Vec<[f64; 3]> = geom.mesh.cell_centres().to_vec();
        for n in names(case) {
            let p = |f: &str| format!("zone_sources.{n}.{f}");
            let target = gs(case, &p("target"));
            let scalar = (target != "temperature").then_some(target);
            let fixed = gs(case, &p("kind")) == "fixed_value";
            let mask = match crate::sources::zone_cells(case, &format!("zone_sources.{n}"), geom, None) {
                Ok(m) => m,
                Err(e) => {
                    errs.push(e);
                    continue;
                }
            };
            if fixed {
                let slot = owners
                    .entry(scalar.clone())
                    .or_insert_with(|| vec![None; geom.n_cells()]);
                for (c, &m) in mask.iter().enumerate() {
                    if !m {
                        continue;
                    }
                    if let Some(other) = &slot[c] {
                        errs.push(d(
                            p("zone"),
                            format!("the fixed-value zones '{other}' and '{n}' share cell {c}; fixed-value zones of one target must not overlap"),
                        ));
                        break;
                    }
                    slot[c] = Some(n.clone());
                }
            }
            let temperature = scalar.is_none();
            let expr = match compile_any(case, &p("value_expression"), &value_dims(temperature, fixed), &ctx) {
                Ok(e) => e,
                Err(mut e) => {
                    errs.append(&mut e);
                    None
                }
            };
            items.push(Item {
                name: n.clone(),
                scalar,
                fixed,
                mask,
                value: gf(case, &p("value")),
                expr,
                k: gf(case, &p("linear_coefficient")),
                reference: gf(case, &p("reference")),
            });
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        let zs = ZoneSources { items, rho_cp, centres };
        // a value expression that is not finite at t = 0 is a located error
        for it in &zs.items {
            if let Some(e) = &it.expr {
                let bad = (0..zs.centres.len()).any(|c| it.mask[c] && !zs.eval_cell(e, c, 0.0).is_finite());
                if bad {
                    errs.push(d(
                        format!("zone_sources.{}.value_expression", it.name),
                        "the expression is not finite at some cells of the zone".into(),
                    ));
                }
            }
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        Ok(Some(zs))
    }

    fn eval_cell(&self, e: &Compiled, c: usize, t: f64) -> f64 {
        let x = self.centres[c];
        e.eval(&[x[0], x[1], x[2], t])
    }

    /// Does any item act on the target (`None` is the temperature)?
    pub fn has_target(&self, scalar: Option<&str>) -> bool {
        self.items.iter().any(|i| i.scalar.as_deref() == scalar)
    }

    /// Set the source, sink and fixed cells of the energy solver for time `t`; `base` is a source that exists anyway
    /// (lumped components). Without a temperature item the solver keeps `base`.
    pub fn apply_temperature(&self, solver: &mut EnergySolver, base: Option<Vec<f64>>, t: f64) {
        match self.terms(None, t) {
            None => solver.source = base,
            Some(z) => {
                let mut s = z.source;
                if let Some(b) = base {
                    for (x, y) in s.iter_mut().zip(&b) {
                        *x += y;
                    }
                }
                solver.source = Some(s);
                solver.sink = z.sink;
                solver.fixed = z.fixed;
            }
        }
    }

    /// Terms for the target (`None` is the temperature) at time `t`; `None` when no item acts on it.
    pub fn terms(&self, scalar: Option<&str>, t: f64) -> Option<ZoneTerms> {
        if !self.has_target(scalar) {
            return None;
        }
        let n = self.centres.len();
        let mut out = ZoneTerms {
            source: vec![0.0; n],
            sink: None,
            fixed: None,
        };
        let scale = if scalar.is_none() { 1.0 / self.rho_cp } else { 1.0 };
        for it in self.items.iter().filter(|i| i.scalar.as_deref() == scalar) {
            for c in 0..n {
                if !it.mask[c] {
                    continue;
                }
                let v = it.expr.as_ref().map_or(it.value, |e| self.eval_cell(e, c, t));
                if it.fixed {
                    out.fixed.get_or_insert_with(|| vec![None; n])[c] = Some(v);
                } else {
                    out.source[c] += v * scale + it.k * it.reference;
                    if it.k != 0.0 {
                        out.sink.get_or_insert_with(|| vec![0.0; n])[c] += it.k;
                    }
                }
            }
        }
        Some(out)
    }
}
