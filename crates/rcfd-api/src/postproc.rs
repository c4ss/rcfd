//! Derived fields and sampling through the case tree (F-072, ADR 0021 D4): the `postprocessing` section.
//!
//! Derived arrays are opt-in (`postprocessing.derived`): cell arrays are added to the VTK volume files, wall
//! quantities are patch-face data written as `wall_<patch>.vtp`, and `postprocessing.samples.<name>` are line or
//! plane samples written as `sample_<name>.csv`. All numerics are the `rcfd_post::derived` functions.

use crate::case::{Case, Diagnostic};
use crate::sim::{get_b, get_f, get_i, get_s, get_v, Simulation};
use crate::{comp, rheology, vd};
use rcfd_core::settings::Schema;
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::gradient::least_squares;
use rcfd_mesh::{Mesh, PatchKind};
use rcfd_post::derived::{self, SampleSet};
use std::path::{Path, PathBuf};

/// Names accepted by `postprocessing.derived`.
pub const DERIVED: [&str; 6] = [
    "vorticity",
    "q_criterion",
    "total_pressure",
    "mach",
    "wall_shear",
    "wall_y_plus",
];

fn d(path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

fn vec3s(doc: &str) -> Schema {
    Schema::list(Schema::float(0.0)).doc(doc)
}

/// Schema of the `postprocessing` section.
pub fn schema() -> Schema {
    let sample = Schema::object()
        .doc("A line or plane sample of the final solution, written as sample_<name>.csv")
        .field(
            "type",
            Schema::enumeration("line", &["line", "plane"]).doc("line: `points` points from p0 to p1 inclusive; plane: n1 x n2 cell-midpoint samples of the rectangle origin + a e1 + b e2"),
        )
        .field("p0", vec3s("line: start point [m]"))
        .field("p1", vec3s("line: end point [m]"))
        .field("points", Schema::int(50).range(2.0, 1e7).doc("line: number of points (at least 2)"))
        .field("origin", vec3s("plane: corner [m]"))
        .field("e1", vec3s("plane: first direction (default [1, 0, 0])"))
        .field("e2", vec3s("plane: second direction, orthogonal to e1 (default [0, 1, 0])"))
        .field("extent1", Schema::float(1.0).at_least(1e-300).doc("plane: length [m] along e1"))
        .field("extent2", Schema::float(1.0).at_least(1e-300).doc("plane: length [m] along e2"))
        .field("n1", Schema::int(10).range(1.0, 1e5).doc("plane: samples along e1"))
        .field("n2", Schema::int(10).range(1.0, 1e5).doc("plane: samples along e2"))
        .field(
            "fields",
            Schema::list(Schema::string("")).doc("Fields to sample (empty = speed, p): ux, uy, uz, speed, p (kinematic for incompressible runs), T, scalars, vorticity_x / _y / _z, q_criterion, and the other names Simulation::field knows"),
        )
        .field(
            "gradient",
            Schema::boolean(true).doc("Reconstruct with the least-squares cell gradient, phi_c + grad_c . (x - x_c); false uses the cell value"),
        );
    Schema::object()
        .doc("Derived fields and line / plane sampling written at the end of a run (ADR 0021). Opt-in: the defaults add nothing to the result files")
        .field(
            "derived",
            Schema::list(Schema::string("")).doc("Derived outputs: vorticity (cell vector), q_criterion, total_pressure (cell scalar p_total: rho p + 0.5 rho |u|^2 for incompressible runs, isentropic from the engine's mach and absolute pressure for the ideal-gas variable-density solver), mach (density-based and variable-density solvers: the engine's mach array), wall_shear and wall_y_plus (laminar constant-viscosity incompressible runs; patch-face arrays in wall_<patch>.vtp)"),
        )
        .field(
            "wall_patches",
            Schema::list(Schema::string("")).doc("Patches of wall_shear / wall_y_plus (empty = every wall patch)"),
        )
        .field("samples", Schema::named(sample).doc("Line and plane samples by name"))
}

fn list_of(case: &Case, path: &str) -> Vec<String> {
    case.tree
        .get(path)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|x| x.as_str().map(String::from))
        .collect()
}

/// Requested derived outputs.
pub fn derived_list(case: &Case) -> Vec<String> {
    list_of(case, "postprocessing.derived")
}

/// Does the case request any postprocessing output beyond the standard files?
pub fn is_active(case: &Case) -> bool {
    !derived_list(case).is_empty() || !case.tree.names("postprocessing.samples").unwrap_or_default().is_empty()
}

fn field_known(case: &Case, name: &str) -> bool {
    let base = crate::schema::FIELD_NAMES.contains(&name)
        || matches!(
            name,
            "vorticity_x" | "vorticity_y" | "vorticity_z" | "q_criterion" | "p_abs" | "nuTilda"
        )
        || case.tree.names("scalars").unwrap_or_default().iter().any(|n| n == name)
        || case.tree.names("species").unwrap_or_default().iter().any(|n| n == name);
    base
}

/// Settings checks (no mesh needed).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let vd_on = vd::enabled(case);
    let comp_on = comp::enabled(case);
    let dens_based = vd_on || comp_on;
    for name in derived_list(case) {
        let p = "postprocessing.derived";
        if !DERIVED.contains(&name.as_str()) {
            out.push(d(
                p,
                format!("unknown derived output '{name}' (known: {})", DERIVED.join(", ")),
            ));
            continue;
        }
        match name.as_str() {
            "wall_shear" | "wall_y_plus" => {
                if get_s(case, "turbulence.model") != "laminar" {
                    out.push(d(p, format!("'{name}' is the laminar-viscosity estimate and is not available with a turbulence model (wall-function shear is not reconstructed)")));
                }
                if dens_based {
                    out.push(d(p, format!("'{name}' is available for the incompressible solvers only (the density-based and variable-density engines have no wall-normal state at the boundary)")));
                }
                if rheology::enabled(case)
                    || !matches!(get_s(case, "properties.viscosity.law").as_str(), "" | "constant")
                {
                    out.push(d(p, format!("'{name}' uses the constant physics.nu; it is not available with a rheology law or a temperature-dependent viscosity")));
                }
            }
            "mach" => {
                if !dens_based {
                    out.push(d(p, "'mach' needs the density-based solver (solver.algorithm = \"compressible\") or fluid.density = \"ideal_gas\"; incompressible runs have no speed of sound"));
                }
            }
            "total_pressure" => {
                if comp_on {
                    out.push(d(p, "'total_pressure' is not available with the density-based solver"));
                } else if vd::real_gas(case) {
                    out.push(d(p, "'total_pressure' uses the ideal-gas isentropic relation and is not available with fluid.density = \"real_gas\""));
                }
            }
            _ => {}
        }
    }
    for n in case.tree.names("postprocessing.samples").unwrap_or_default() {
        let b = format!("postprocessing.samples.{n}");
        let ty = get_s(case, &format!("{b}.type"));
        if ty == "plane" {
            let e1 = get_v(case, &format!("{b}.e1"), [1.0, 0.0, 0.0]);
            let e2 = get_v(case, &format!("{b}.e2"), [0.0, 1.0, 0.0]);
            let (n1, n2) = (vec3::norm(e1), vec3::norm(e2));
            if n1 < 1e-300 {
                out.push(d(&format!("{b}.e1"), "the direction must not be zero"));
            }
            if n2 < 1e-300 {
                out.push(d(&format!("{b}.e2"), "the direction must not be zero"));
            } else if n1 >= 1e-300 && vec3::dot(e1, e2).abs() > 1e-9 * n1 * n2 {
                out.push(d(&format!("{b}.e2"), "e2 must be orthogonal to e1"));
            }
        } else {
            let p0 = get_v(case, &format!("{b}.p0"), [0.0; 3]);
            let p1 = get_v(case, &format!("{b}.p1"), [0.0; 3]);
            if vec3::norm(vec3::sub(p1, p0)) < 1e-300 {
                out.push(d(&format!("{b}.p1"), "p1 coincides with p0"));
            }
        }
        for f in list_of(case, &format!("{b}.fields")) {
            if !field_known(case, &f) {
                out.push(d(&format!("{b}.fields"), format!("unknown field '{f}'")));
            } else if f == "T" && !(get_b(case, "energy.enabled") || dens_based) {
                out.push(d(
                    &format!("{b}.fields"),
                    "field 'T' needs energy.enabled or a density-based solver",
                ));
            }
        }
    }
    out
}

/// Checks that need the built simulation (patch names, fields the engine really has). Run by `Simulation::new`.
pub fn check_sim(sim: &Simulation) -> Vec<Diagnostic> {
    let case = &sim.case;
    let mut out = Vec::new();
    for p in list_of(case, "postprocessing.wall_patches") {
        match sim.geom.mesh.patch(&p) {
            None => out.push(d(
                "postprocessing.wall_patches",
                format!("'{p}' is not a patch of the mesh"),
            )),
            Some(pa) if pa.kind != PatchKind::Wall => {
                out.push(d("postprocessing.wall_patches", format!("patch '{p}' is not a wall")))
            }
            _ => {}
        }
    }
    for n in case.tree.names("postprocessing.samples").unwrap_or_default() {
        for f in sample_fields(case, &format!("postprocessing.samples.{n}")) {
            if !f.starts_with("vorticity_") && f != "q_criterion" {
                if let Err(e) = sim.field(&f) {
                    out.push(d(&format!("postprocessing.samples.{n}.fields"), e.to_string()));
                }
            }
        }
    }
    out
}

fn sample_fields(case: &Case, base: &str) -> Vec<String> {
    let f = list_of(case, &format!("{base}.fields"));
    if f.is_empty() {
        vec!["speed".into(), "p".into()]
    } else {
        f
    }
}

/// Cell arrays to add to the VTK volume files.
#[derive(Debug, Default)]
pub struct DerivedCells {
    /// Vector arrays.
    pub vectors: Vec<(&'static str, Vec<Vec3>)>,
    /// Scalar arrays.
    pub scalars: Vec<(&'static str, Vec<f64>)>,
}

/// Velocity gradient of the current solution.
fn gradient(sim: &Simulation) -> Vec<[Vec3; 3]> {
    let ub = sim.velocity().boundary_values(&sim.geom);
    derived::velocity_gradient(&sim.geom, &sim.velocity().values, &ub)
}

/// Cell arrays requested by `postprocessing.derived` (empty when none).
pub fn derived_cells(sim: &Simulation) -> Result<DerivedCells, String> {
    let case = &sim.case;
    let list = derived_list(case);
    let mut out = DerivedCells::default();
    if list.iter().any(|n| n == "vorticity" || n == "q_criterion") {
        let g = gradient(sim);
        if list.iter().any(|n| n == "vorticity") {
            out.vectors.push(("vorticity", derived::vorticity(&g)));
        }
        if list.iter().any(|n| n == "q_criterion") {
            out.scalars.push(("q_criterion", derived::q_criterion(&g)));
        }
    }
    if list.iter().any(|n| n == "mach") {
        out.scalars
            .push(("mach", sim.field("mach").map_err(|e| e.to_string())?));
    }
    if list.iter().any(|n| n == "total_pressure") {
        let u = &sim.velocity().values;
        let v: Vec<f64> = if vd::enabled(case) {
            let pa = sim.field("p_abs").map_err(|e| e.to_string())?;
            let m = sim.field("mach").map_err(|e| e.to_string())?;
            let gamma = get_f(case, "fluid.gamma");
            pa.iter()
                .zip(&m)
                .map(|(&p, &m)| derived::total_pressure_isentropic(p, m, gamma))
                .collect()
        } else {
            let rho = get_f(case, "physics.rho");
            sim.pressure()
                .values
                .iter()
                .zip(u)
                .map(|(&p, &u)| derived::total_pressure_incompressible(rho * p, rho, u))
                .collect()
        };
        out.scalars.push(("p_total", v));
    }
    Ok(out)
}

/// Patch-face arrays of one wall patch: shear vectors and y+.
pub fn wall_arrays(sim: &Simulation, patch: &str) -> Result<(Vec<Vec3>, Vec<f64>), String> {
    let case = &sim.case;
    let (rho, nu) = (get_f(case, "physics.rho"), get_f(case, "physics.nu"));
    let ub = sim.velocity().boundary_values(&sim.geom);
    let tau = derived::wall_shear(&sim.geom, patch, &sim.velocity().values, &ub, rho, nu).map_err(|e| e.to_string())?;
    let yp = derived::y_plus(&sim.geom, patch, &tau, rho, nu).map_err(|e| e.to_string())?;
    Ok((tau, yp))
}

/// Wall patches of `wall_shear` / `wall_y_plus`.
pub fn wall_patch_names(case: &Case, mesh: &Mesh) -> Vec<String> {
    let l = list_of(case, "postprocessing.wall_patches");
    if l.is_empty() {
        mesh.patches()
            .iter()
            .filter(|p| p.kind == PatchKind::Wall)
            .map(|p| p.name.clone())
            .collect()
    } else {
        l
    }
}

/// Write `wall_<patch>.vtp` for each wall patch when `wall_shear` or `wall_y_plus` is requested.
pub fn write_wall_files(sim: &Simulation, dir: &Path) -> Result<Vec<PathBuf>, String> {
    use rcfd_io::vtk::{VtkField, VtkValues};
    let list = derived_list(&sim.case);
    let (want_tau, want_yp) = (
        list.iter().any(|n| n == "wall_shear"),
        list.iter().any(|n| n == "wall_y_plus"),
    );
    let mut written = Vec::new();
    if !(want_tau || want_yp) {
        return Ok(written);
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for patch in wall_patch_names(&sim.case, &sim.geom.mesh) {
        let (tau, yp) = wall_arrays(sim, &patch)?;
        let mut fields = Vec::new();
        if want_tau {
            fields.push(VtkField {
                name: "wall_shear",
                values: VtkValues::Vector(&tau),
            });
        }
        if want_yp {
            fields.push(VtkField {
                name: "wall_y_plus",
                values: VtkValues::Scalar(&yp),
            });
        }
        let path = dir.join(format!("wall_{patch}.vtp"));
        rcfd_io::vtk::write_vtp(&path, &sim.geom.mesh, &patch, &fields).map_err(|e| e.to_string())?;
        written.push(path);
    }
    Ok(written)
}

/// Geometry of a sample.
#[derive(Debug, Clone)]
pub enum SampleKind {
    /// Points from `p0` to `p1` inclusive.
    Line {
        /// Start.
        p0: Vec3,
        /// End.
        p1: Vec3,
        /// Number of points.
        points: usize,
    },
    /// Cell-midpoint samples of a rectangle.
    Plane {
        /// Corner.
        origin: Vec3,
        /// First direction.
        e1: Vec3,
        /// Second direction.
        e2: Vec3,
        /// Length along `e1`.
        extent1: f64,
        /// Length along `e2`.
        extent2: f64,
        /// Samples along `e1`.
        n1: usize,
        /// Samples along `e2`.
        n2: usize,
    },
}

/// A sample request.
#[derive(Debug, Clone)]
pub struct SampleSpec {
    /// Geometry.
    pub kind: SampleKind,
    /// Field names (empty = speed, p).
    pub fields: Vec<String>,
    /// Gradient reconstruction.
    pub gradient: bool,
}

/// The sample `postprocessing.samples.<name>` of a case.
pub fn spec_from_case(case: &Case, name: &str) -> SampleSpec {
    let b = format!("postprocessing.samples.{name}");
    let kind = if get_s(case, &format!("{b}.type")) == "plane" {
        SampleKind::Plane {
            origin: get_v(case, &format!("{b}.origin"), [0.0; 3]),
            e1: get_v(case, &format!("{b}.e1"), [1.0, 0.0, 0.0]),
            e2: get_v(case, &format!("{b}.e2"), [0.0, 1.0, 0.0]),
            extent1: get_f(case, &format!("{b}.extent1")),
            extent2: get_f(case, &format!("{b}.extent2")),
            n1: get_i(case, &format!("{b}.n1")).max(1) as usize,
            n2: get_i(case, &format!("{b}.n2")).max(1) as usize,
        }
    } else {
        SampleKind::Line {
            p0: get_v(case, &format!("{b}.p0"), [0.0; 3]),
            p1: get_v(case, &format!("{b}.p1"), [0.0; 3]),
            points: get_i(case, &format!("{b}.points")).max(2) as usize,
        }
    };
    SampleSpec {
        kind,
        fields: sample_fields(case, &b),
        gradient: get_b(case, &format!("{b}.gradient")),
    }
}

/// Parse a sample request from JSON-RPC parameters: `type` (`line` default | `plane`), the geometry keys of the
/// schema (`p0`, `p1`, `points`; `origin`, `e1`, `e2`, `extent1`, `extent2`, `n1`, `n2`), `fields`, `gradient`.
pub fn spec_from_json(p: &serde_json::Value) -> Result<SampleSpec, String> {
    let vec = |k: &str, default: Vec3| -> Result<Vec3, String> {
        match p.get(k) {
            None | Some(serde_json::Value::Null) => Ok(default),
            Some(v) => {
                let a = v
                    .as_array()
                    .filter(|a| a.len() == 3)
                    .ok_or(format!("'{k}' must be a list of 3 numbers"))?;
                let mut out = [0.0; 3];
                for (o, x) in out.iter_mut().zip(a) {
                    *o = x.as_f64().ok_or(format!("'{k}' must be a list of 3 numbers"))?;
                }
                Ok(out)
            }
        }
    };
    let num = |k: &str, default: f64| p.get(k).and_then(serde_json::Value::as_f64).unwrap_or(default);
    let int = |k: &str, default: usize| {
        p.get(k)
            .and_then(serde_json::Value::as_u64)
            .map_or(default, |v| v as usize)
    };
    let kind = if p.get("type").and_then(serde_json::Value::as_str) == Some("plane") {
        SampleKind::Plane {
            origin: vec("origin", [0.0; 3])?,
            e1: vec("e1", [1.0, 0.0, 0.0])?,
            e2: vec("e2", [0.0, 1.0, 0.0])?,
            extent1: num("extent1", 1.0),
            extent2: num("extent2", 1.0),
            n1: int("n1", 10),
            n2: int("n2", 10),
        }
    } else {
        SampleKind::Line {
            p0: vec("p0", [0.0; 3])?,
            p1: vec("p1", [0.0; 3])?,
            points: int("points", 50),
        }
    };
    let fields = p
        .get("fields")
        .and_then(serde_json::Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default();
    Ok(SampleSpec {
        kind,
        fields,
        gradient: p.get("gradient").and_then(serde_json::Value::as_bool).unwrap_or(true),
    })
}

/// Cell values of a field and, when requested and available, its reconstruction gradient.
fn field_and_gradient(sim: &Simulation, name: &str, gradient: bool) -> Result<(Vec<f64>, Option<Vec<Vec3>>), String> {
    match name {
        "vorticity_x" | "vorticity_y" | "vorticity_z" => {
            let w = derived::vorticity(&gradient_of(sim));
            let k = (name.as_bytes()[10] - b'x') as usize;
            Ok((w.iter().map(|v| v[k]).collect(), None))
        }
        "q_criterion" => Ok((derived::q_criterion(&gradient_of(sim)), None)),
        _ => {
            let cells = sim.field(name).map_err(|e| e.to_string())?;
            let grad = if gradient {
                sim.boundary_field(name)
                    .ok()
                    .map(|b| least_squares(&sim.geom, &cells, &b))
            } else {
                None
            };
            Ok((cells, grad))
        }
    }
}

fn gradient_of(sim: &Simulation) -> Vec<[Vec3; 3]> {
    gradient(sim)
}

/// Evaluate a sample on the current solution.
pub fn sample(sim: &Simulation, spec: &SampleSpec) -> Result<SampleSet, String> {
    let geom = &sim.geom;
    let mut set = match &spec.kind {
        SampleKind::Line { p0, p1, points } => derived::sample_line(geom, *p0, *p1, *points),
        SampleKind::Plane {
            origin,
            e1,
            e2,
            extent1,
            extent2,
            n1,
            n2,
        } => derived::sample_plane(geom, *origin, *e1, *e2, *extent1, *extent2, *n1, *n2),
    }
    .map_err(|e| e.to_string())?;
    let names: Vec<String> = if spec.fields.is_empty() {
        vec!["speed".into(), "p".into()]
    } else {
        spec.fields.clone()
    };
    for n in &names {
        let (cells, grad) = field_and_gradient(sim, n, spec.gradient)?;
        set.add_field(geom, n, &cells, grad.as_deref());
    }
    Ok(set)
}

/// CSV text of a sample.
pub fn sample_csv(sim: &Simulation, spec: &SampleSpec) -> Result<String, String> {
    Ok(sample(sim, spec)?.to_csv())
}

/// Write `sample_<name>.csv` for every `postprocessing.samples.<name>` of the case.
pub fn write_samples(sim: &Simulation, dir: &Path) -> Result<Vec<PathBuf>, String> {
    let names = sim.case.tree.names("postprocessing.samples").unwrap_or_default();
    let mut written = Vec::new();
    if names.is_empty() {
        return Ok(written);
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for n in names {
        let csv = sample_csv(sim, &spec_from_case(&sim.case, &n))?;
        let path = dir.join(format!("sample_{n}.csv"));
        std::fs::write(&path, csv).map_err(|e| e.to_string())?;
        written.push(path);
    }
    Ok(written)
}

/// Everything `write_results` adds: wall files and sample CSVs.
pub fn write_outputs(sim: &Simulation, dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut v = write_wall_files(sim, dir)?;
    v.extend(write_samples(sim, dir)?);
    Ok(v)
}
