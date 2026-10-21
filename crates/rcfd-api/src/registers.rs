//! Cell registers in the settings tree (F-124, ADR 0011 section 3): `registers.<name>` defines a named set of cells
//! (box, expression of the cell centre, boundary adjacency, mesh cell zone, or a combination of other registers);
//! `sources.<n>.zone = "register"` restricts a source to a register and `initial.patches.<n>` overwrites an initial
//! field inside one. A `field` register thresholds an initial field (see [`InitialFields`]). The set operations are in
//! `rcfd_post::registers`.

use crate::case::{Case, Diagnostic};
use crate::exprs;
use rcfd_core::expr::Expr;
use rcfd_core::settings::Schema;
use rcfd_core::vec3::Vec3;
use rcfd_mesh::Mesh;
use rcfd_post::registers::Register;
use std::collections::BTreeMap;

/// Value of `above` / `below` meaning "unbounded".
pub const UNBOUNDED: f64 = 1e300;

const TYPES: [&str; 6] = [
    "box",
    "expression",
    "field",
    "boundary_adjacent",
    "cell_zone",
    "combine",
];
const FIELDS: [&str; 6] = ["velocity_magnitude", "ux", "uy", "uz", "pressure", "temperature"];

/// The initial fields a `field` register thresholds: taken in `Simulation::new` after the uniform values, the velocity
/// expressions and the user-function initialise hook, before `initial.patches` (ADR 0095). Pressure is kinematic.
#[derive(Clone, Debug)]
pub struct InitialFields {
    /// Cell velocity.
    pub u: Vec<Vec3>,
    /// Cell kinematic pressure.
    pub p: Vec<f64>,
    /// Cell temperature.
    pub t: Vec<f64>,
}

/// True when some register of the case is a `field` register (the initial fields are then kept for them).
pub fn uses_fields(case: &Case) -> bool {
    names(case)
        .iter()
        .any(|n| gs(case, &format!("registers.{n}.type")) == "field")
}
const OPS: [&str; 4] = ["union", "intersection", "difference", "complement"];

/// Schema of the `registers` collection.
pub fn schema() -> Schema {
    Schema::named(
        Schema::object()
            .doc("A named set of cells (a register) for source restriction, initial patches and post-processing")
            .field(
                "type",
                Schema::enumeration("box", &TYPES).doc("box: cells whose centres lie in [box_min, box_max] (inclusive); expression: cells where above <= expression(x, y, z) <= below at the cell centre; field: cells where above <= the initial value of `field` <= below (initial fields after uniform values, velocity expressions and the initialise hook, before initial patches; evaluated once, not during the run; usable by sources and initial patches of velocity, pressure, temperature only); boundary_adjacent: cells that own a face on the listed patches, grown to `layers` face-neighbour layers; cell_zone: a named mesh cell zone; combine: set algebra on other registers"),
            )
            .field("box_min", Schema::list(Schema::float(0.0)).doc("Lower corner [m] of the box"))
            .field("box_max", Schema::list(Schema::float(0.0)).doc("Upper corner [m] of the box (every component must exceed box_min)"))
            .field(
                "expression",
                Schema::string("").doc("Expression of the cell centre x, y, z (and the physical constants / named expressions of the case); lengths need units, for example (x - 0.5 [m])^2; the SI value is compared with above and below"),
            )
            .field(
                "field",
                Schema::enumeration("velocity_magnitude", &FIELDS).doc("field: the initial field to threshold (pressure is the kinematic pressure)"),
            )
            .field(
                "above",
                Schema::float(-UNBOUNDED).doc("Lower limit (inclusive) of the expression or field value; -1e300 or below means unbounded"),
            )
            .field(
                "below",
                Schema::float(UNBOUNDED).doc("Upper limit (inclusive) of the expression or field value; 1e300 or above means unbounded"),
            )
            .field(
                "patches",
                Schema::list(Schema::string("")).doc("Patches whose adjacent cells are selected (boundary_adjacent)"),
            )
            .field(
                "layers",
                Schema::int(1).range(1.0, 1000.0).doc("boundary_adjacent: number of cell layers from the patches (1 = only cells owning a patch face)"),
            )
            .field("cell_zone", Schema::string("").doc("Name of the mesh cell zone (cell_zone)"))
            .field(
                "operation",
                Schema::enumeration("union", &OPS).doc("combine: union, intersection, difference (first operand minus all others) or complement (exactly one operand)"),
            )
            .field(
                "operands",
                Schema::list(Schema::string("")).doc("combine: names of the registers to combine"),
            )
            .field(
                "grow",
                Schema::int(0).range(0.0, 1000.0).doc("Dilate the resulting register by this many layers (applied before shrink)"),
            )
            .field(
                "shrink",
                Schema::int(0).range(0.0, 1000.0).doc("Erode the resulting register by this many layers"),
            )
            .field(
                "neighbours",
                Schema::enumeration("faces", &["faces", "points"]).doc("Adjacency used by grow and shrink: faces (cells sharing a face) or points (cells sharing a vertex)"),
            ),
    )
    .doc("Cell registers: named regions defined by a box, an expression, an initial-field threshold, boundary adjacency, a cell zone or set algebra (ADR 0011)")
}

/// Schema of `initial.patches`.
pub fn initial_patches_schema() -> Schema {
    Schema::named(
        Schema::object()
            .doc("Overwrite an initial field inside a register")
            .field("register", Schema::string("").doc("Name of the register (registers.<name>)"))
            .field(
                "variable",
                Schema::enumeration(
                    "velocity",
                    &["velocity", "pressure", "temperature", "k", "epsilon", "omega", "nutilde"],
                )
                .doc("The field to overwrite; k, epsilon, omega and nutilde are fields of the turbulence model (which must have them) and take one number"),
            )
            .field(
                "value",
                Schema::list(Schema::float(0.0)).doc("The value: three components for velocity, one number otherwise"),
            ),
    )
    .doc("Initial patches: field values set inside registers after the uniform and expression initialisation, in name order")
}

/// Variables of `initial.patches` that belong to the turbulence model.
pub const TURBULENCE_VARIABLES: [&str; 4] = ["k", "epsilon", "omega", "nutilde"];

/// Names of the configured registers.
pub fn names(case: &Case) -> Vec<String> {
    case.tree.names("registers").unwrap_or_default()
}

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

/// A 3-vector setting; `None` unless the list has exactly three numbers.
fn gv(c: &Case, p: &str) -> Option<Option<Vec3>> {
    let a = c.tree.get(p).ok()?.as_array()?.clone();
    if a.len() != 3 {
        return Some(None);
    }
    let n = |i: usize| a[i].as_f64();
    Some(match (n(0), n(1), n(2)) {
        (Some(x), Some(y), Some(z)) => Some([x, y, z]),
        _ => None,
    })
}

fn d(path: String, message: String) -> Diagnostic {
    Diagnostic {
        path,
        line: None,
        message,
    }
}

fn strings(case: &Case, path: &str) -> Vec<String> {
    case.tree
        .get(path)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

fn int(case: &Case, path: &str) -> i64 {
    case.tree.get(path).ok().and_then(|v| v.as_i64()).unwrap_or(0)
}

fn vec3_setting(case: &Case, path: &str) -> Result<Vec3, String> {
    match gv(case, path) {
        Some(Some(v)) => Ok(v),
        _ => Err("expected 3 components [x, y, z]".into()),
    }
}

/// Static checks of the registers and of every reference to one (no mesh needed).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let known = names(case);
    for n in &known {
        let p = |f: &str| format!("registers.{n}.{f}");
        match gs(case, &p("type")).as_str() {
            "box" => {
                let lo = vec3_setting(case, &p("box_min"));
                let hi = vec3_setting(case, &p("box_max"));
                for (f, r) in [("box_min", &lo), ("box_max", &hi)] {
                    if let Err(m) = r {
                        out.push(d(p(f), m.clone()));
                    }
                }
                if let (Ok(l), Ok(h)) = (lo, hi) {
                    if (0..3).any(|k| l[k] >= h[k]) {
                        out.push(d(p("box_max"), "box_max must exceed box_min in every component".into()));
                    }
                }
            }
            "expression" => {
                let src = gs(case, &p("expression"));
                if src.trim().is_empty() {
                    out.push(d(p("expression"), "type = \"expression\" needs an expression".into()));
                } else if let Err(m) = compile_expression(case, &src) {
                    out.push(d(p("expression"), m));
                }
                let (lo, hi) = (gf(case, &p("above")), gf(case, &p("below")));
                if lo > hi {
                    out.push(d(
                        p("above"),
                        format!("above ({lo}) exceeds below ({hi}): the register is always empty"),
                    ));
                }
            }
            "field" => {
                let (lo, hi) = (gf(case, &p("above")), gf(case, &p("below")));
                if lo > hi {
                    out.push(d(
                        p("above"),
                        format!("above ({lo}) exceeds below ({hi}): the register is always empty"),
                    ));
                }
                let method = gs(case, "initial.method");
                if !method.is_empty() && method != "uniform" {
                    out.push(d(
                        p("type"),
                        format!("a field register thresholds the initial fields, which initial.method = \"{method}\" computes only after the registers are built; use initial.method = \"uniform\""),
                    ));
                }
            }
            "boundary_adjacent" => {
                if strings(case, &p("patches")).is_empty() {
                    out.push(d(p("patches"), "boundary_adjacent needs at least one patch".into()));
                }
                if int(case, &p("layers")) < 1 {
                    out.push(d(p("layers"), "layers must be at least 1".into()));
                }
            }
            "cell_zone" => {
                if gs(case, &p("cell_zone")).is_empty() {
                    out.push(d(
                        p("cell_zone"),
                        "type = \"cell_zone\" needs the name of a mesh cell zone".into(),
                    ));
                }
            }
            "combine" => {
                let ops = strings(case, &p("operands"));
                let op = gs(case, &p("operation"));
                if ops.is_empty() {
                    out.push(d(p("operands"), "combine needs at least one operand".into()));
                }
                if op == "complement" && ops.len() != 1 {
                    out.push(d(p("operands"), "complement takes exactly one operand".into()));
                }
                for o in &ops {
                    if !known.contains(o) {
                        out.push(d(
                            p("operands"),
                            format!("unknown register '{o}' (registers: {})", known.join(", ")),
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    // cycles among combine operands
    for n in &known {
        if let Some(path) = find_cycle(case, n, &mut Vec::new()) {
            out.push(d(
                format!("registers.{n}.operands"),
                format!("registers refer to each other in a cycle: {}", path.join(" -> ")),
            ));
        }
    }
    // references from sources and initial patches
    for s in case.tree.names("sources").unwrap_or_default() {
        if gs(case, &format!("sources.{s}.zone")) == "register" {
            let r = gs(case, &format!("sources.{s}.register"));
            if !known.contains(&r) {
                out.push(d(
                    format!("sources.{s}.register"),
                    format!("unknown register '{r}' (registers: {})", known.join(", ")),
                ));
            }
        }
    }
    for n in case.tree.names("initial.patches").unwrap_or_default() {
        let p = |f: &str| format!("initial.patches.{n}.{f}");
        let r = gs(case, &p("register"));
        if !known.contains(&r) {
            out.push(d(
                p("register"),
                format!("unknown register '{r}' (registers: {})", known.join(", ")),
            ));
        }
        let want = if gs(case, &p("variable")) == "velocity" { 3 } else { 1 };
        let have = case
            .tree
            .get(&p("value"))
            .ok()
            .and_then(|v| v.as_array().map(Vec::len))
            .unwrap_or(0);
        if have != want {
            out.push(d(
                p("value"),
                format!(
                    "expected {want} component(s) for {}, found {have}",
                    gs(case, &p("variable"))
                ),
            ));
        }
    }
    out
}

fn find_cycle(case: &Case, n: &str, stack: &mut Vec<String>) -> Option<Vec<String>> {
    if let Some(i) = stack.iter().position(|s| s == n) {
        let mut c = stack[i..].to_vec();
        c.push(n.into());
        return Some(c);
    }
    if gs(case, &format!("registers.{n}.type")) != "combine" {
        return None;
    }
    stack.push(n.into());
    for o in strings(case, &format!("registers.{n}.operands")) {
        if let Some(c) = find_cycle(case, &o, stack) {
            return Some(c);
        }
    }
    stack.pop();
    None
}

fn compile_expression(case: &Case, src: &str) -> Result<rcfd_core::expr::Compiled, String> {
    let ctx = exprs::spatial_context(case);
    let ex = Expr::parse(src).map_err(|e| format!("{e}\n{}", e.render(src)))?;
    ex.compile(&ctx).map_err(|e| format!("{e}\n{}", e.render(src)))
}

/// Build the register `name` on `mesh`. `fields` are the initial fields for `field` registers (`None` where the caller has
/// none: a `field` register then gives a located error). Errors are located at the setting that is wrong.
pub fn build(case: &Case, name: &str, mesh: &Mesh, fields: Option<&InitialFields>) -> Result<Register, Diagnostic> {
    build_rec(case, name, mesh, fields, &mut Vec::new())
}

fn build_rec(
    case: &Case,
    name: &str,
    mesh: &Mesh,
    fields: Option<&InitialFields>,
    stack: &mut Vec<String>,
) -> Result<Register, Diagnostic> {
    let p = |f: &str| format!("registers.{name}.{f}");
    if stack.iter().any(|s| s == name) {
        return Err(d(
            p("operands"),
            format!(
                "registers refer to each other in a cycle: {} -> {name}",
                stack.join(" -> ")
            ),
        ));
    }
    let known = names(case);
    if !known.iter().any(|k| k == name) {
        return Err(d(
            format!("registers.{name}"),
            format!("unknown register '{name}' (registers: {})", known.join(", ")),
        ));
    }
    // library errors carry the argument name; locate them under this register
    let loc = |e: rcfd_core::Error| match e {
        rcfd_core::Error::InvalidValue { path, reason } => d(p(&path), reason),
        other => d(format!("registers.{name}"), other.to_string()),
    };
    let mut reg = match gs(case, &p("type")).as_str() {
        "box" => {
            let lo = vec3_setting(case, &p("box_min")).map_err(|m| d(p("box_min"), m))?;
            let hi = vec3_setting(case, &p("box_max")).map_err(|m| d(p("box_max"), m))?;
            Register::in_box(mesh, lo, hi).map_err(loc)?
        }
        "expression" => {
            let src = gs(case, &p("expression"));
            let ex = compile_expression(case, &src).map_err(|m| d(p("expression"), m))?;
            let (lo, hi) = (gf(case, &p("above")), gf(case, &p("below")));
            let (lo, hi) = (
                if lo <= -UNBOUNDED { f64::NEG_INFINITY } else { lo },
                if hi >= UNBOUNDED { f64::INFINITY } else { hi },
            );
            let mut values = Vec::with_capacity(mesh.n_cells());
            for (i, c) in mesh.cell_centres().iter().enumerate() {
                let v = ex.eval(&[c[0], c[1], c[2], 0.0]);
                if !v.is_finite() {
                    return Err(d(
                        p("expression"),
                        format!(
                            "the expression is not finite ({v}) at the centre of cell {i} = ({}, {}, {})",
                            c[0], c[1], c[2]
                        ),
                    ));
                }
                values.push(v);
            }
            Register::threshold(mesh, &values, lo, hi).map_err(loc)?
        }
        "field" => {
            let Some(f) = fields else {
                return Err(d(
                    p("type"),
                    "a field register is evaluated on the initial fields and can only be used by sources.<n> (zone = \"register\") and initial.patches of velocity, pressure or temperature".into(),
                ));
            };
            let values: Vec<f64> = match gs(case, &p("field")).as_str() {
                "ux" => f.u.iter().map(|v| v[0]).collect(),
                "uy" => f.u.iter().map(|v| v[1]).collect(),
                "uz" => f.u.iter().map(|v| v[2]).collect(),
                "pressure" => f.p.clone(),
                "temperature" => f.t.clone(),
                _ => f.u.iter().map(|v| rcfd_core::vec3::norm(*v)).collect(),
            };
            let (lo, hi) = (gf(case, &p("above")), gf(case, &p("below")));
            let (lo, hi) = (
                if lo <= -UNBOUNDED { f64::NEG_INFINITY } else { lo },
                if hi >= UNBOUNDED { f64::INFINITY } else { hi },
            );
            Register::threshold(mesh, &values, lo, hi).map_err(loc)?
        }
        "boundary_adjacent" => {
            let patches = strings(case, &p("patches"));
            let refs: Vec<&str> = patches.iter().map(String::as_str).collect();
            let layers = int(case, &p("layers"));
            if layers < 1 {
                return Err(d(p("layers"), "layers must be at least 1".into()));
            }
            Register::boundary_adjacent(mesh, &refs, layers as usize).map_err(loc)?
        }
        "cell_zone" => Register::cell_zone(mesh, &gs(case, &p("cell_zone"))).map_err(loc)?,
        "combine" => {
            let ops = strings(case, &p("operands"));
            stack.push(name.into());
            let mut regs = Vec::new();
            for o in &ops {
                if !known.contains(o) {
                    stack.pop();
                    return Err(d(
                        p("operands"),
                        format!("unknown register '{o}' (registers: {})", known.join(", ")),
                    ));
                }
                regs.push(build_rec(case, o, mesh, fields, stack)?);
            }
            stack.pop();
            let Some(first) = regs.first() else {
                return Err(d(p("operands"), "combine needs at least one operand".into()));
            };
            let mut acc = first.clone();
            match gs(case, &p("operation")).as_str() {
                "complement" => {
                    if regs.len() != 1 {
                        return Err(d(p("operands"), "complement takes exactly one operand".into()));
                    }
                    acc = acc.complement();
                }
                op => {
                    for r in &regs[1..] {
                        acc = match op {
                            "intersection" => acc.intersection(r),
                            "difference" => acc.difference(r),
                            _ => acc.union(r),
                        }
                        .map_err(loc)?;
                    }
                }
            }
            acc
        }
        other => return Err(d(p("type"), format!("unknown register type '{other}'"))),
    };
    let (grow, shrink) = (
        int(case, &p("grow")).max(0) as usize,
        int(case, &p("shrink")).max(0) as usize,
    );
    let points = gs(case, &p("neighbours")) == "points";
    if grow > 0 {
        reg = if points {
            reg.grow_points(mesh, grow)
        } else {
            reg.grow(mesh, grow)
        };
    }
    if shrink > 0 {
        reg = if points {
            reg.shrink_points(mesh, shrink)
        } else {
            reg.shrink(mesh, shrink)
        };
    }
    Ok(reg)
}

/// Every register of the case.
pub fn build_all(
    case: &Case,
    mesh: &Mesh,
    fields: Option<&InitialFields>,
) -> Result<BTreeMap<String, Register>, Vec<Diagnostic>> {
    let mut out = BTreeMap::new();
    let mut errs = Vec::new();
    for n in names(case) {
        match build(case, &n, mesh, fields) {
            Ok(r) => {
                out.insert(n, r);
            }
            Err(e) => errs.push(e),
        }
    }
    if errs.is_empty() {
        Ok(out)
    } else {
        Err(errs)
    }
}

/// Cell flags of the register named by the setting `register_path` (used for `sources.<n>.zone = "register"`);
/// the diagnostic is located at that setting.
pub fn zone_from_setting(
    case: &Case,
    register_path: &str,
    mesh: &Mesh,
    fields: Option<&InitialFields>,
) -> Result<Vec<bool>, Diagnostic> {
    let name = gs(case, register_path);
    let known = names(case);
    if !known.contains(&name) {
        return Err(d(
            register_path.into(),
            format!("unknown register '{name}' (registers: {})", known.join(", ")),
        ));
    }
    build(case, &name, mesh, fields).map(|r| r.mask().to_vec())
}

/// Check every register against the mesh and apply `initial.patches.<n>` (in name order) to the initial velocity, pressure and
/// temperature arrays; `fields` are the unpatched initial fields for `field` registers.
pub fn apply_initial_patches(
    case: &Case,
    mesh: &Mesh,
    fields: Option<&InitialFields>,
    u: &mut [Vec3],
    p: &mut [f64],
    t: &mut [f64],
) -> Vec<Diagnostic> {
    // every register is built once so that errors in unused ones are reported too
    let mut errs = build_all(case, mesh, fields).err().unwrap_or_default();
    for n in case.tree.names("initial.patches").unwrap_or_default() {
        let path = |f: &str| format!("initial.patches.{n}.{f}");
        let rname = gs(case, &path("register"));
        let reg = match zone_from_setting(case, &path("register"), mesh, fields) {
            Ok(m) => Register::from_mask(m),
            Err(e) => {
                errs.push(e);
                continue;
            }
        };
        let vals: Vec<f64> = case
            .tree
            .get(&path("value"))
            .ok()
            .and_then(|v| v.as_array().cloned())
            .map(|a| a.iter().filter_map(|x| x.as_f64()).collect())
            .unwrap_or_default();
        let var = gs(case, &path("variable"));
        if TURBULENCE_VARIABLES.contains(&var.as_str()) {
            // applied to the turbulence model's own fields once it is built (crate::init)
            continue;
        }
        let res = match var.as_str() {
            "velocity" if vals.len() == 3 => reg
                .patch_vector_field(u, [vals[0], vals[1], vals[2]])
                .map(|v| u.copy_from_slice(&v)),
            "pressure" if vals.len() == 1 => reg.patch_field(p, vals[0]).map(|v| p.copy_from_slice(&v)),
            "temperature" if vals.len() == 1 => reg.patch_field(t, vals[0]).map(|v| t.copy_from_slice(&v)),
            _ => {
                errs.push(d(
                    path("value"),
                    format!(
                        "expected {} component(s) for {var}, found {}",
                        if var == "velocity" { 3 } else { 1 },
                        vals.len()
                    ),
                ));
                continue;
            }
        };
        if let Err(e) = res {
            errs.push(d(path("register"), format!("register '{rname}': {e}")));
        }
    }
    errs
}
