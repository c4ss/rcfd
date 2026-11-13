//! Co-simulation participants from the case (F-444, ADR 0380): the optional table `[cosim]` names a moving wall patch of
//! an incompressible PISO/PIMPLE case as the fluid participant of `rcfd_fv::cosim` (`FlowWall`), which `rcfd serve
//! --cosim <case>` hosts under protocol `rcfd-cosim/1` ([`host`], `crate::cosim_service`).
//!
//! The case's mesh is the reference configuration; the flow starts at rest on that mesh moved by
//! `cosim.initial_displacement` (the smoothing of the interior), and every `advance` moves the wall to the displacement
//! it is given, smooths the interior (moving patch prescribed, `fixed_patches` fixed, every other patch sliding) and makes
//! one flow step. The participant advances the flow solver alone: a case whose simulation would also advance another
//! model is refused, naming it. A case without the table is not affected in any way.

use crate::case::{Case, Diagnostic};
use crate::service::Service;
use crate::sim::{build_mesh, get_b, get_f, get_s, Simulation};
use rcfd_core::settings::Schema;
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::cosim::{FlowWall, Fluid, MeshMotion};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::FvGeometry;
use rcfd_linalg::SolveControl;
use rcfd_mesh::motion::moved_mesh;
use rcfd_mesh::Mesh;
use serde_json::Value;
use std::sync::Arc;

/// Schema of the optional table `[cosim]`.
pub fn schema() -> Schema {
    Schema::object()
        .optional()
        .doc("Co-simulation fluid participant (F-444, ADR 0380): the moving wall patch of an incompressible piso / pimple case, hosted by rcfd serve --cosim <case> under protocol rcfd-cosim/1 (docs/cosimulation-protocol.md) and driven by a coupler (rcfd_fv::cosim). The case is not run on its own. Absent = no participant")
        .field("patch", Schema::string("").doc("The moving wall patch: the participant's wall (required)"))
        .field(
            "participant",
            Schema::string("fluid").doc("Name under which rcfd serve hosts the participant (the participant parameter of cosim.open)"),
        )
        .field(
            "fixed_patches",
            Schema::list(Schema::string("")).doc("Smoothing: patches whose points do not move; every other patch except the moving one slides in its plane"),
        )
        .field(
            "smoothing_method",
            Schema::enumeration("laplacian", &["laplacian", "inverse_distance", "inverse_volume"])
                .doc("Diffusivity of the point-displacement equation, as dynamic_mesh.smoothing.method"),
        )
        .field(
            "smoothing_exponent",
            Schema::float(1.0).range(0.0, 10.0).doc("Exponent of the inverse-distance or inverse-volume diffusivity"),
        )
        .field(
            "smoothing_tolerance",
            Schema::float(1e-12).range(0.0, 1.0).doc("Relative tolerance of the displacement solve (absolute floor 1e-16, at most 5000 iterations)"),
        )
        .field(
            "initial_displacement",
            Schema::list(Schema::float(0.0)).doc("Displacement of the wall from the mesh at time 0 [m]: three numbers per wall point, the points of the patch in ascending mesh point index (the order of cosim.describe); empty = 0. The flow starts at rest on the mesh moved by it"),
        )
}

/// True when the case has the table `[cosim]`.
pub fn present(case: &Case) -> bool {
    case.tree.to_json().get("cosim").is_some()
}

fn d(path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

fn strings(case: &Case, path: &str) -> Vec<String> {
    case.tree
        .get(path)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
}

fn floats(case: &Case, path: &str) -> Vec<f64> {
    case.tree
        .get(path)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_f64)
        .collect()
}

/// The checks that need no mesh (located errors; empty without the table).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if !present(case) {
        return out;
    }
    let patch = get_s(case, "cosim.patch");
    if patch.trim().is_empty() {
        out.push(d("cosim.patch", "name the moving wall patch of the fluid participant"));
    }
    if strings(case, "cosim.fixed_patches").contains(&patch) {
        out.push(d(
            "cosim.fixed_patches",
            format!("'{patch}' is the moving patch; it cannot also be fixed"),
        ));
    }
    if get_s(case, "cosim.participant").trim().is_empty() {
        out.push(d("cosim.participant", "the hosted participant needs a name"));
    }
    if floats(case, "cosim.initial_displacement")
        .iter()
        .any(|x| !x.is_finite())
    {
        out.push(d("cosim.initial_displacement", "the displacements must be finite"));
    }
    let alg = get_s(case, "solver.algorithm");
    if alg != "piso" && alg != "pimple" {
        out.push(d(
            "solver.algorithm",
            format!("a co-simulation fluid participant needs \"piso\" or \"pimple\", not \"{alg}\""),
        ));
    }
    if get_s(case, "solver.corrector_control") == "adaptive" {
        out.push(d(
            "solver.corrector_control",
            "the adaptive corrector count is not supported by the co-simulation fluid participant (the repeated steps would reset its schedule)",
        ));
    }
    if get_b(case, "dynamic_mesh.enabled") {
        out.push(d(
            "dynamic_mesh.enabled",
            "the co-simulation fluid participant moves the mesh itself; switch the dynamic mesh off",
        ));
    }
    if case.tree.get("mesh").ok().is_some_and(|m| m.get("adapt").is_some()) {
        out.push(d(
            "mesh.adapt",
            "mesh adaptation is not available for a co-simulation fluid participant",
        ));
    }
    out
}

/// The mesh motion of the participant: from the displacement of the wall points to every mesh point (the smoothing of
/// the reference mesh).
struct Smoothing {
    mesh0: Mesh,
    geom0: FvGeometry,
    classes: Vec<PatchClass>,
    wall: Vec<usize>,
    method: SmoothingMethod,
    control: SolveControl,
}

impl Smoothing {
    fn points(&self, displacement: &[Vec3]) -> Result<Vec<Vec3>, String> {
        let mut presc = vec![vec3::ZERO; self.mesh0.n_points()];
        for (&q, v) in self.wall.iter().zip(displacement) {
            presc[q] = *v;
        }
        let sm = smooth_displacement(&self.geom0, &self.classes, &presc, self.method, &self.control)
            .map_err(|e| e.to_string())?;
        Ok(self
            .mesh0
            .points()
            .iter()
            .zip(&sm.displacement)
            .map(|(p, dd)| vec3::add(*p, *dd))
            .collect())
    }
}

/// The fluid participant of the case: checks, the reference mesh, the mesh at the initial displacement, the flow solver
/// of the case on it and `FlowWall` with the smoothing as its mesh motion.
pub fn fluid_from_case(case: &Case) -> Result<FlowWall, Vec<Diagnostic>> {
    if !present(case) {
        return Err(vec![d("cosim", "the case has no table [cosim]")]);
    }
    let errs = check(case);
    if !errs.is_empty() {
        return Err(errs);
    }
    let patch = get_s(case, "cosim.patch");
    let mesh0 = build_mesh(case)?;
    let known: Vec<&str> = mesh0.patches().iter().map(|p| p.name.as_str()).collect();
    if mesh0.patch(&patch).is_none() {
        return Err(vec![d(
            "cosim.patch",
            format!("mesh has no patch '{patch}' (patches: {})", known.join(", ")),
        )]);
    }
    let fixed = strings(case, "cosim.fixed_patches");
    if let Some(f) = fixed.iter().find(|f| mesh0.patch(f).is_none()) {
        return Err(vec![d(
            "cosim.fixed_patches",
            format!("mesh has no patch '{f}' (patches: {})", known.join(", ")),
        )]);
    }
    let wall = FlowWall::point_ids(&mesh0, &patch).ok_or_else(|| vec![d("cosim.patch", "no points")])?;
    let initial = floats(case, "cosim.initial_displacement");
    if !initial.is_empty() && initial.len() != 3 * wall.len() {
        return Err(vec![d(
            "cosim.initial_displacement",
            format!(
                "{} numbers for {} wall points (three per point, or none)",
                initial.len(),
                wall.len()
            ),
        )]);
    }
    let geom0 = FvGeometry::new(Arc::new(mesh0.clone())).map_err(|e| vec![d("mesh", e.to_string())])?;
    let classes = mesh0
        .patches()
        .iter()
        .map(|p| {
            if p.name == patch {
                PatchClass::Prescribed
            } else if fixed.contains(&p.name) {
                PatchClass::Fixed
            } else {
                PatchClass::Slip
            }
        })
        .collect();
    let exponent = get_f(case, "cosim.smoothing_exponent");
    let method = match get_s(case, "cosim.smoothing_method").as_str() {
        "inverse_distance" => SmoothingMethod::InverseDistance(exponent),
        "inverse_volume" => SmoothingMethod::InverseVolume(exponent),
        _ => SmoothingMethod::Laplacian,
    };
    let smoothing = Arc::new(Smoothing {
        mesh0: mesh0.clone(),
        geom0,
        classes,
        wall,
        method,
        control: SolveControl {
            tol_abs: 1e-16,
            tol_rel: get_f(case, "cosim.smoothing_tolerance").max(1e-14),
            max_iter: 5000,
        },
    });
    let geom = if initial.is_empty() {
        Arc::new(FvGeometry::new(Arc::new(mesh0.clone())).map_err(|e| vec![d("mesh", e.to_string())])?)
    } else {
        let d0: Vec<Vec3> = initial.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
        let points = smoothing
            .points(&d0)
            .map_err(|e| vec![d("cosim.initial_displacement", e)])?;
        let mesh = moved_mesh(&mesh0, points).map_err(|e| vec![d("cosim.initial_displacement", e.to_string())])?;
        Arc::new(FvGeometry::new(Arc::new(mesh)).map_err(|e| vec![d("cosim.initial_displacement", e.to_string())])?)
    };
    let flow = Simulation::from_geometry(case, geom)?
        .into_cosim_flow()
        .map_err(|e| vec![d("cosim", e)])?;
    let motion: MeshMotion = {
        let s = smoothing.clone();
        Box::new(move |dd| s.points(dd))
    };
    FlowWall::new(
        flow,
        &patch,
        &mesh0,
        get_f(case, "physics.rho"),
        get_f(case, "physics.nu"),
        motion,
    )
    .map_err(|e| vec![d("cosim", e)])
}

/// Host the fluid participant of `case` in `svc` under the name `cosim.participant` (built on the participant's own
/// thread, ADR 0345 D1). Returns the name.
pub fn host(svc: &Service, case: &Case) -> Result<String, Vec<Diagnostic>> {
    if !present(case) {
        return Err(vec![d("cosim", "the case has no table [cosim]")]);
    }
    let errs = check(case);
    if !errs.is_empty() {
        return Err(errs);
    }
    let name = get_s(case, "cosim.participant");
    let case = case.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let result = svc.host_fluid(&name, move || match fluid_from_case(&case) {
        Ok(f) => Ok(Box::new(f) as Box<dyn Fluid>),
        Err(e) => {
            let text = e.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("; ");
            let _ = tx.send(e);
            Err(text)
        }
    });
    match result {
        Ok(()) => Ok(name),
        Err(m) => Err(rx.try_recv().unwrap_or_else(|_| vec![d("cosim", m)])),
    }
}
