//! The mesh transform of a case (ADR 0155, job 48U): `[mesh.transform]`, a rotation of the mesh about an axis parallel
//! to z. [`crate::sim::build_mesh`] applies it to the mesh of every source, after the mesh is built or read (and after
//! `mesh.scale`), so `rcfd run`, the run service, `rcfd check`, the service and the Studio's preview all see the same
//! rotated mesh. Only the mesh turns: no setting of the case is rotated. Flow and force directions, gravity and body
//! forces are world quantities by intent; settings that place something relative to the geometry (points, boxes, axes,
//! expressions in x, y, z, ...) stay in world coordinates too and get the warning [`MT101`].
//!
//! Sign convention: positive angles turn counter-clockwise seen from +z (x to the right, y up). An angle of attack
//! alpha with the flow along +x and lift along +y turns an aerofoil nose up, clockwise: `rotate_z_deg = -alpha`, about
//! the quarter chord (`rotate_origin = [0.25, 0, 0]` for a unit chord).

use crate::case::{Case, Diagnostic};
use crate::rules::{RuleDiagnostic, Severity};
use rcfd_core::expr::Expr;
use rcfd_core::settings::Schema;
use rcfd_core::units::Dimension;
use rcfd_core::vec3::Vec3;
use rcfd_mesh::Mesh;
use serde_json::Value;
use std::collections::BTreeMap;

/// Rule id of the warning for a setting that stays in world coordinates while the mesh is rotated.
pub const MT101: &str = "MT101";

/// Path of the angle.
pub const ANGLE: &str = "mesh.transform.rotate_z_deg";
/// Path of the axis point.
pub const ORIGIN: &str = "mesh.transform.rotate_origin";

const ANGLE_IS_A_NUMBER: &str =
    "give the angle as a plain number of degrees (a unit string such as \"-4 deg\" would be read in radians)";

/// The optional table `mesh.transform` (absent from a case that does not write it, so every existing case keeps its
/// settings data, text and hash).
pub fn schema() -> Schema {
    Schema::object()
        .optional()
        .doc("Rigid rotation of the mesh (ADR 0155), applied to the mesh of every source after it is built or read (after mesh.scale). Only the mesh turns: every other setting (flow and force directions, points, boxes, axes, expressions) stays in world coordinates; settings that place something relative to the geometry get the warning MT101. Absent = no transform")
        .field(
            "rotate_z_deg",
            Schema::float(0.0)
                .range(-360.0, 360.0)
                .unit("deg")
                .doc("Rotation of the mesh about the axis parallel to z through rotate_origin, in degrees (a plain number, not a unit string); positive = counter-clockwise seen from +z (x to the right, y up). Angle of attack alpha with the flow along +x and lift along +y: rotate_z_deg = -alpha (nose up). 0 (or a multiple of 360) = no rotation: the mesh is bit-identical"),
        )
        .field(
            "rotate_origin",
            Schema::list(Schema::float(0.0).dim(Dimension::LENGTH))
                .unit("m")
                .doc("A point [x, y, z] of the rotation axis, in m, in the coordinates after mesh.scale; empty = the origin. For an aerofoil of unit chord with its leading edge at the origin: [0.25, 0, 0], the quarter chord"),
        )
}

/// A rotation of the mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rotation {
    /// Angle in degrees, as written (its reduction modulo 360 is not 0).
    pub deg: f64,
    /// A point of the axis (parallel to z).
    pub origin: Vec3,
}

fn diag(path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

/// The axis point: `[]` is the origin, else 3 finite components.
fn origin(case: &Case) -> Result<Vec3, String> {
    let v = case.get(ORIGIN).unwrap_or(Value::Array(Vec::new()));
    let a = v.as_array().cloned().unwrap_or_default();
    match a.len() {
        0 => Ok([0.0; 3]),
        3 => {
            let x: Vec<f64> = a.iter().filter_map(Value::as_f64).filter(|x| x.is_finite()).collect();
            if x.len() == 3 {
                Ok([x[0], x[1], x[2]])
            } else {
                Err("expected 3 finite numbers [x, y, z]".into())
            }
        }
        n => Err(format!(
            "expected 3 components [x, y, z] (or an empty list for the origin), got {n}"
        )),
    }
}

/// The case's rotation; `None` when the table is absent or its angle reduces to 0 modulo 360 (no transform).
pub fn rotation(case: &Case) -> Option<Rotation> {
    let deg = case.get(ANGLE).ok()?.as_f64()?;
    if rcfd_mesh::transform::reduce_deg(deg) == 0.0 {
        return None;
    }
    Some(Rotation {
        deg,
        origin: origin(case).unwrap_or([0.0; 3]),
    })
}

/// Settings checks of the table (no mesh needed): the axis point's components.
pub fn check(case: &Case) -> Vec<Diagnostic> {
    match origin(case) {
        Ok(_) => Vec::new(),
        Err(m) => vec![diag(ORIGIN, m)],
    }
}

/// A string angle in a case document, before coercion turns it into a number (D7).
pub fn check_raw(value: &Value) -> Vec<Diagnostic> {
    match value.pointer("/mesh/transform/rotate_z_deg") {
        Some(Value::String(_)) => vec![diag(ANGLE, ANGLE_IS_A_NUMBER)],
        _ => Vec::new(),
    }
}

/// The message when `value`, given for the settings path `path` (`Case::set`), holds a string angle (D7).
pub fn string_angle(path: &str, value: &Value) -> Option<String> {
    let pointer = match path {
        "" => "/mesh/transform/rotate_z_deg",
        "mesh" => "/transform/rotate_z_deg",
        "mesh.transform" => "/rotate_z_deg",
        ANGLE => "",
        _ => return None,
    };
    matches!(value.pointer(pointer), Some(Value::String(_))).then(|| format!("{ANGLE}: {ANGLE_IS_A_NUMBER}"))
}

/// The mesh of `case` after its transform: the mesh itself (bit for bit) without one, else rotated. A rotation that
/// would break a rotationally periodic pair of the mesh is refused at `mesh.transform.rotate_origin` (D5).
pub fn apply(case: &Case, mesh: Mesh) -> Result<Mesh, Vec<Diagnostic>> {
    let Some(r) = rotation(case) else {
        return Ok(mesh);
    };
    let o = origin(case).map_err(|m| vec![diag(ORIGIN, m)])?;
    if let Some(m) = rcfd_mesh::transform::periodic_conflict(&mesh, o) {
        return Err(vec![diag(ORIGIN, m)]);
    }
    rcfd_mesh::transform::rotate_z(&mesh, r.deg, o).map_err(|e| vec![diag("mesh.transform", e.to_string())])
}

/// Last path segments of the settings that hold a point, a box corner or an axis (D6).
const POSITION_KEYS: [&str; 12] = [
    "point",
    "origin",
    "position",
    "centre",
    "box_min",
    "box_max",
    "rotation_origin",
    "rotation_axis",
    "wall_rotation_origin",
    "wall_rotation_axis",
    "axis",
    "reference_point",
];

fn get_s(case: &Case, path: &str) -> String {
    case.get(path)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

/// True when the expression `src` reads x, y or z.
fn uses_xyz(src: &str) -> bool {
    Expr::parse(src).is_ok_and(|e| e.identifiers().iter().any(|i| matches!(i.as_str(), "x" | "y" | "z")))
}

/// Walk the settings below `v` (at `path`): explicit positions and axes (a non-empty list that is not the schema's
/// default), expressions in x, y, z and table inputs x, y, z.
fn walk(case: &Case, v: &Value, path: &str, out: &mut BTreeMap<String, String>) {
    let Value::Object(m) = v else { return };
    for (k, x) in m {
        let p = if path.is_empty() {
            k.clone()
        } else {
            format!("{path}.{k}")
        };
        if path.is_empty() && k == "mesh" {
            continue; // the mesh's own settings describe the mesh before it turns
        }
        match x {
            Value::Object(_) => walk(case, x, &p, out),
            Value::Array(a) if POSITION_KEYS.contains(&k.as_str()) && !a.is_empty() => {
                let default = case.tree.schema_at(&p).map(Schema::default_value).ok();
                if default.as_ref() == Some(x) {
                    continue;
                }
                if k == "box_max"
                    && m.get("box_min")
                        .and_then(Value::as_array)
                        .is_some_and(|b| !b.is_empty())
                {
                    continue; // one warning per box, at box_min
                }
                let what = if k == "box_min" {
                    "the box [box_min, box_max]".to_string()
                } else {
                    format!("{k} {x}")
                };
                out.insert(p, what);
            }
            Value::Array(a) if k == "inputs" && path.starts_with("tables.") => {
                if a.iter().any(|i| matches!(i.as_str(), Some("x" | "y" | "z"))) {
                    out.insert(p, "a table read at x, y or z".into());
                }
            }
            Value::String(s)
                if (k.contains("expression") || (k == "definition" && path.starts_with("expressions.")))
                    && uses_xyz(s) =>
            {
                out.insert(p, format!("the expression \"{s}\" in x, y or z"));
            }
            _ => {}
        }
    }
}

/// MT101 (D6): one warning per setting that places something relative to the geometry, while the mesh is rotated;
/// none without a rotation. Emitted by [`crate::rules::check_case`] after the data rules.
pub fn world_coordinate_warnings(case: &Case) -> Vec<RuleDiagnostic> {
    let Some(r) = rotation(case) else {
        return Vec::new();
    };
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    walk(case, &case.tree.to_json(), "", &mut out);
    let names = |c: &str| case.tree.names(c).unwrap_or_default();
    for n in names("reports") {
        match get_s(case, &format!("reports.{n}.type")).as_str() {
            "probe" => {
                out.entry(format!("reports.{n}.point"))
                    .or_insert_with(|| "the probe point".into());
            }
            "moment" => {
                out.entry(format!("reports.{n}.origin"))
                    .or_insert_with(|| "the moment origin and axis".into());
            }
            _ => {}
        }
    }
    for n in names("sources") {
        if matches!(
            get_s(case, &format!("sources.{n}.type")).as_str(),
            "rotating_frame" | "mrf"
        ) {
            out.entry(format!("sources.{n}.rotation_origin"))
                .or_insert_with(|| "the axis of the rotating frame".into());
        }
    }
    for n in names("boundaries") {
        let rate = case
            .get(&format!("boundaries.{n}.wall_rotation_rate"))
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        if rate != 0.0 {
            out.entry(format!("boundaries.{n}.wall_rotation_origin"))
                .or_insert_with(|| "the axis of the wall rotation".into());
        }
    }
    let on = |p: &str| case.get(p).ok().and_then(|v| v.as_bool()) == Some(true);
    if on("dynamic_mesh.enabled") {
        out.entry("dynamic_mesh.enabled".into())
            .or_insert_with(|| "the motion of the dynamic mesh".into());
    }
    if on("mixing_plane.enabled") {
        out.entry("mixing_plane.origin".into())
            .or_insert_with(|| "the axis of the mixing plane".into());
    }
    let method = get_s(case, "decomposition.method");
    if !method.is_empty() && method != "none" {
        out.entry("decomposition.method".into())
            .or_insert_with(|| format!("the geometric partitioning \"{method}\""));
    }
    let o = r.origin;
    out.into_iter()
        .map(|(path, what)| RuleDiagnostic {
            id: MT101.into(),
            severity: Severity::Warning,
            category: "mesh".into(),
            path,
            message: format!(
                "{what} stays in world coordinates while mesh.transform rotates the mesh by {} deg about the z axis through ({}, {}, {}); give it for the rotated geometry",
                r.deg, o[0], o[1], o[2]
            ),
            fix: None,
        })
        .collect()
}
