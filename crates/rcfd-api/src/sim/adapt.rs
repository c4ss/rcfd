//! The case key `[mesh.adapt]` (F-342, ADR 0262): a steady incompressible run on a uniform box mesh adapted by the loop of
//! `rcfd_fv::adaptive` (ADR 0242) with the criteria of ADR 0247.
//!
//! A case without the table never reaches this module beyond [`present`]: `Simulation::new` builds it as before.

use super::{diag, get_b, get_f, get_i, get_s, get_v, Engine, Simulation, StepSummary};
use crate::case::{Case, Diagnostic};
use rcfd_core::settings::Schema;
use rcfd_core::vec3;
use rcfd_core::{Error, Result};
use rcfd_fv::adaptive::{
    children_per_split, fixed_fraction_marks, register_marks, restrict_marks, run_adaptive_simple,
    run_adaptive_simple_marked, run_adaptive_simple_target, run_adaptive_simple_with, target_count_marks,
    target_schedule, velocity_edge_length_indicator, velocity_gradient_indicator, AdaptiveRun, AdaptiveSettings,
    CycleRecord, Register,
};
use rcfd_fv::simple::{SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::adapt::{AmrMesh, RefineTransfer};
use rcfd_mesh::gen::{axis_coords, Grading};
use rcfd_mesh::PatchKind;
use std::sync::Arc;

/// Path of the table.
pub(crate) const TABLE: &str = "mesh.adapt";

/// The generators whose mesh the lattice reproduces (D2).
const GENERATORS: [&str; 3] = ["cavity_2d", "box_2d", "box_3d"];

/// Schema of `[mesh.adapt]` (D1): an optional table whose defaults are ADR 0242's cavity run A.
pub fn schema() -> Schema {
    Schema::object()
        .optional()
        .doc("Solution-adaptive steady run (ADR 0262): the steady simple / simplec solve is repeated on a lattice refined where the indicator of the last solution is largest (rcfd_fv::adaptive, ADR 0242; criteria of ADR 0247), with the velocity and pressure transferred to each new mesh. For mesh.generator = cavity_2d, box_2d or box_3d (uniform, mesh.source = generator); laminar, constant properties, no sources or models. The defaults are ADR 0242's adapted cavity. Absent = no adaptation")
        .field(
            "cycles",
            Schema::int(2).range(0.0, 20.0).doc("Adaptation steps; the run makes cycles + 1 solves (0 = one solve on the base lattice)"),
        )
        .field(
            "max_level",
            Schema::int(2).range(1.0, 10.0).doc("Finest refinement level: a base cell is split at most max_level times along each refined axis"),
        )
        .field(
            "marking",
            Schema::enumeration("fixed_fraction", &["fixed_fraction", "target_count"]).doc("fixed_fraction: each step refines the ceil(refine_fraction N) cells with the largest indicator; target_count: each step refines the cells needed to reach a geometric schedule towards target_cells (ADR 0247 D2)"),
        )
        .field(
            "refine_fraction",
            Schema::float(0.4).range(0.0, 1.0).doc("Fraction of the cells marked per step (marking = fixed_fraction)"),
        )
        .field(
            "target_cells",
            Schema::int(0).range(0.0, 1e7).doc("Cell count of the last step (marking = target_count; required there)"),
        )
        .field(
            "indicator",
            Schema::enumeration("velocity_gradient", &["velocity_gradient", "velocity_gradient_unscaled", "edge_length"]).doc("Refinement indicator per cell: velocity_gradient = |grad U| h (h the geometric-mean cell length, ADR 0242); velocity_gradient_unscaled = |grad U|; edge_length = the variation of U along the cell edges times the cell measure to the power (r - 1)/d, r = edge_length_exponent (ADR 0247 D3)"),
        )
        .field(
            "edge_length_exponent",
            Schema::float(1.0).at_least(0.0).doc("Exponent r of indicator = edge_length (1: the variation across the cell; 1 + d/2: its contribution to the L2 norm)"),
        )
        .field(
            "register",
            Schema::string("").doc("Name of a register (registers.<name>, not of type field) that confines or drives the refinement; empty = the whole mesh"),
        )
        .field(
            "register_use",
            Schema::enumeration("confine", &["confine", "refine"]).doc("confine: the indicator's marks outside the register are dropped; refine: every register cell below max_level is refined, the indicator is not used (ADR 0247 D4)"),
        )
        .field(
            "balance",
            Schema::boolean(true).doc("Refine neighbours so that face neighbours differ by at most one level (2:1 balance)"),
        )
        .field(
            "transfer",
            Schema::enumeration("linear", &["linear", "injection"]).doc("How refined cells take the velocity and pressure of their parent: linear (parent value plus its least-squares gradient) or injection (the parent value)"),
        )
        .field(
            "max_iterations",
            Schema::int(20000).range(1.0, 1e7).doc("Iterations per solve (the case's solver.max_iterations and convergence conditions are not used by the loop)"),
        )
        .field(
            "tolerance",
            Schema::float(1e-7).range(1e-14, 1.0).doc("A solve stops when the largest relaxed velocity change of an iteration is below tolerance and the continuity error below 1e-6 (SimpleSolver::run)"),
        )
}

/// Whether the case writes the table (an absent optional table reads its defaults, so the data is looked at).
/// Public as `rcfd_api::sim::has_mesh_adapt` (F-475, ADR 0377).
pub fn present(case: &Case) -> bool {
    case.tree.get("mesh").ok().is_some_and(|m| m.get("adapt").is_some())
}

fn at(path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

fn refused(path: &str, what: &str) -> Diagnostic {
    at(
        path,
        format!("{what} is not available with [mesh.adapt] (the adaptive steady loop runs laminar constant-property simple / simplec on a uniform box mesh, ADR 0262); remove the table [mesh.adapt] or this setting"),
    )
}

/// Static checks of the table (D5 V3 to V8); empty without it.
pub(crate) fn check(case: &Case) -> Vec<Diagnostic> {
    if !present(case) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let generator = get_s(case, "mesh.generator");
    if get_s(case, "mesh.source") == "file" {
        out.push(refused("mesh.source", "a mesh file"));
    } else if !GENERATORS.contains(&generator.as_str()) {
        out.push(refused(
            "mesh.generator",
            &format!("mesh.generator = \"{generator}\" (cavity_2d, box_2d and box_3d are)"),
        ));
    }
    if generator == "box_2d" {
        if (get_f(case, "mesh.grading") - 1.0).abs() >= 1e-12 {
            out.push(refused("mesh.grading", "a graded mesh"));
        }
        if get_i(case, "mesh.wall_layers") > 0 {
            out.push(refused("mesh.wall_layers", "boundary-layer inflation"));
        }
        for k in ["mesh.periodic_x", "mesh.periodic_y"] {
            if get_b(case, k) {
                out.push(refused(k, "a periodic pair"));
            }
        }
    }
    let section = get_s(case, "mesh.box_3d.section");
    if generator == "box_3d" && !section.is_empty() && section != "box" {
        out.push(refused("mesh.box_3d.section", &format!("section = \"{section}\"")));
    }
    if crate::mesh_transform::rotation(case).is_some() {
        out.push(refused("mesh.transform.rotate_z_deg", "a mesh rotation"));
    }
    let alg = get_s(case, "solver.algorithm");
    if !matches!(alg.as_str(), "simple" | "simplec") {
        out.push(refused(
            "solver.algorithm",
            &format!("solver.algorithm = \"{alg}\" (simple and simplec are)"),
        ));
    }
    if get_s(case, "mesh.adapt.marking") == "target_count" && get_i(case, "mesh.adapt.target_cells") <= 0 {
        out.push(at(
            "mesh.adapt.target_cells",
            "marking = \"target_count\" needs target_cells > 0 (the cell count of the last step)",
        ));
    }
    let reg = get_s(case, "mesh.adapt.register");
    if reg.is_empty() {
        if get_s(case, "mesh.adapt.register_use") == "refine" {
            out.push(at(
                "mesh.adapt.register_use",
                "register_use = \"refine\" needs mesh.adapt.register (the name of a register)",
            ));
        }
    } else if !crate::registers::names(case).contains(&reg) {
        out.push(at(
            "mesh.adapt.register",
            format!(
                "unknown register '{reg}' (registers: {})",
                crate::registers::names(case).join(", ")
            ),
        ));
    } else if get_s(case, &format!("registers.{reg}.type")) == "field" {
        out.push(at(
            "mesh.adapt.register",
            format!("register '{reg}' is of type field, whose initial fields do not follow the adapted mesh; use a geometric register"),
        ));
    }
    let models: [(bool, &str, &str); 19] = [
        (crate::turb::enabled(case), "turbulence.model", "a turbulence model"),
        (get_b(case, "energy.enabled"), "energy.enabled", "the energy equation"),
        (crate::scalars::enabled(case), "scalars", "transported scalars"),
        (crate::chemistry::enabled(case), "chemistry.model", "chemistry"),
        (crate::cht::enabled(case), "solids", "conjugate heat transfer"),
        (
            crate::electric::enabled(case),
            "electric.enabled",
            "the electric potential",
        ),
        (
            crate::phase_change::enabled(case),
            "phase_change.enabled",
            "phase change",
        ),
        (
            crate::properties::enabled(case),
            "properties",
            "temperature-dependent properties",
        ),
        (crate::radiation::enabled(case), "radiation.model", "radiation"),
        (
            crate::rheology::enabled(case),
            "rheology.model",
            "non-Newtonian rheology",
        ),
        (crate::sources::enabled(case), "sources", "momentum sources"),
        (crate::lumped::enabled(case), "components", "lumped components"),
        (crate::zone_sources::enabled(case), "zone_sources", "zone sources"),
        (
            crate::particles::enabled(case),
            "particles.enabled",
            "particle tracking",
        ),
        (
            crate::mixing_run::enabled(case),
            "mixing_plane.enabled",
            "a mixing plane",
        ),
        (crate::stats::enabled(case), "statistics.enabled", "statistics"),
        (crate::dynmesh::enabled(case), "dynamic_mesh.enabled", "a moving mesh"),
        (crate::vd::enabled(case), "fluid.density", "a variable density"),
        (crate::vof::enabled(case), "multiphase.enabled", "multiphase flow"),
    ];
    for (on, path, what) in models {
        if on {
            out.push(refused(path, what));
        }
    }
    if !case.tree.names("udf").unwrap_or_default().is_empty() {
        out.push(refused("udf", "a user function"));
    }
    let options: [(bool, &str, &str); 11] = [
        (
            vec3::norm(get_v(case, "physics.body_force", [0.0; 3])) > 0.0,
            "physics.body_force",
            "a body force",
        ),
        (
            get_f(case, "solver.bulk_velocity") != 0.0,
            "solver.bulk_velocity",
            "a bulk-velocity driver",
        ),
        (
            get_s(case, "solver.linear_upwind_limiter") != "none",
            "solver.linear_upwind_limiter",
            "a linear-upwind limiter",
        ),
        (
            get_s(case, "solver.periodic_gradient") == "partner_cell",
            "solver.periodic_gradient",
            "the partner-cell periodic gradient",
        ),
        (
            get_s(case, "solver.velocity_correction") == "face_flux",
            "solver.velocity_correction",
            "the face-flux velocity correction",
        ),
        (
            get_b(case, "solver.simplec_boundary_compensation"),
            "solver.simplec_boundary_compensation",
            "the SIMPLEC boundary compensation",
        ),
        (
            get_s(case, "solver.pressure_interpolation") != "linear",
            "solver.pressure_interpolation",
            "a pressure interpolation other than linear",
        ),
        (
            get_s(case, "solver.pseudo_mode") != "off",
            "solver.pseudo_mode",
            "pseudo-time stepping",
        ),
        (
            get_s(case, "solver.pseudo_control") != "off",
            "solver.pseudo_control",
            "the pseudo-time-step controller",
        ),
        (
            get_s(case, "solver.residual_norm") != "reference",
            "solver.residual_norm",
            "a residual norm other than reference",
        ),
        (
            get_s(case, "solver.component_diagonal") == "corrected",
            "solver.component_diagonal",
            "the corrected component diagonal",
        ),
    ];
    for (on, path, what) in options {
        if on {
            out.push(refused(path, what));
        }
    }
    if crate::init::method(case) != "uniform" {
        out.push(refused("initial.method", "an initial method other than uniform"));
    }
    for n in case.tree.names("convergence").unwrap_or_default() {
        out.push(refused(
            &format!("convergence.{n}"),
            "a convergence condition (a solve of the loop stops by mesh.adapt.tolerance)",
        ));
    }
    for n in case.tree.names("boundaries").unwrap_or_default() {
        let b = format!("boundaries.{n}");
        for c in ["x", "y", "z"] {
            let k = format!("{b}.velocity_expression_{c}");
            if !get_s(case, &k).is_empty() {
                out.push(refused(&k, "a velocity expression"));
            }
        }
        if !get_s(case, &format!("{b}.velocity_udf")).is_empty() {
            out.push(refused(&format!("{b}.velocity_udf"), "a user-function velocity"));
        }
        if get_s(case, &format!("{b}.velocity")) == "marangoni" {
            out.push(refused(&format!("{b}.velocity"), "a Marangoni stress"));
        }
        if crate::bc_cat::kind(case, &n) != "none" {
            out.push(refused(&format!("{b}.bc_kind"), "a boundary-condition catalogue entry"));
        }
    }
    out
}

/// The base lattice of the case (D2): the generator's coordinates and patches, `mesh.adapt.max_level`.
pub(crate) fn base_lattice(case: &Case) -> std::result::Result<AmrMesh, Vec<Diagnostic>> {
    let me = |e: Error| diag(TABLE, e.to_string());
    let (nx, ny) = (get_i(case, "mesh.nx") as usize, get_i(case, "mesh.ny") as usize);
    let (lx, ly) = (get_f(case, "mesh.lx"), get_f(case, "mesh.ly"));
    let max_level = get_i(case, "mesh.adapt.max_level").clamp(0, 10) as u8;
    let generator = get_s(case, "mesh.generator");
    let named = |n: &str, k: PatchKind| (n.to_string(), k);
    let wall = |n: &str| (n.to_string(), PatchKind::Wall);
    let (coords, axes, patches) = match generator.as_str() {
        "cavity_2d" | "box_2d" => {
            let coords = [
                axis_coords(nx, lx, Grading::Uniform).map_err(me)?,
                axis_coords(ny, ly, Grading::Uniform).map_err(me)?,
                // the slab of the generator: one cell of lx / nx (gen::cavity_2d_rect, gen::box_2d_graded)
                axis_coords(1, lx / nx as f64, Grading::Uniform).map_err(me)?,
            ];
            let sides = if generator == "cavity_2d" {
                [wall("walls"), wall("walls"), wall("walls"), wall("lid")]
            } else {
                [wall("xmin"), wall("xmax"), wall("ymin"), wall("ymax")]
            };
            let [a, b, c, d] = sides;
            (
                coords,
                [true, true, false],
                [
                    a,
                    b,
                    c,
                    d,
                    named("frontAndBack", PatchKind::Empty),
                    named("frontAndBack", PatchKind::Empty),
                ],
            )
        }
        "box_3d" => {
            let nz = get_i(case, "mesh.nz").max(1) as usize;
            let coords = [
                axis_coords(nx, lx, Grading::Uniform).map_err(me)?,
                axis_coords(ny, ly, Grading::Uniform).map_err(me)?,
                axis_coords(nz, get_f(case, "mesh.lz"), Grading::Uniform).map_err(me)?,
            ];
            (
                coords,
                [true; 3],
                [
                    wall("xmin"),
                    wall("xmax"),
                    wall("ymin"),
                    wall("ymax"),
                    wall("zmin"),
                    wall("zmax"),
                ],
            )
        }
        other => {
            return Err(diag(
                "mesh.generator",
                format!("[mesh.adapt] needs mesh.generator = cavity_2d, box_2d or box_3d, not \"{other}\""),
            ))
        }
    };
    AmrMesh::new(coords, axes, max_level, patches).map_err(me)
}

/// The library settings of the table (D1).
pub(crate) fn settings(case: &Case) -> AdaptiveSettings {
    AdaptiveSettings {
        cycles: get_i(case, "mesh.adapt.cycles").max(0) as usize,
        refine_fraction: get_f(case, "mesh.adapt.refine_fraction"),
        balance: get_b(case, "mesh.adapt.balance"),
        transfer: if get_s(case, "mesh.adapt.transfer") == "injection" {
            RefineTransfer::Injection
        } else {
            RefineTransfer::Linear
        },
        scale_by_length: get_s(case, "mesh.adapt.indicator") != "velocity_gradient_unscaled",
        max_iter: get_i(case, "mesh.adapt.max_iterations").max(1) as usize,
        tol: get_f(case, "mesh.adapt.tolerance"),
    }
}

/// State of an adaptive simulation.
pub(crate) struct AdaptState {
    /// The loop has not run yet (it runs at the first step).
    pending: bool,
    /// One record per solve, after the loop.
    pub(crate) cycles: Vec<CycleRecord>,
    /// The final lattice, after the loop.
    pub(crate) lattice: Option<AmrMesh>,
}

fn joined(d: &[Diagnostic]) -> String {
    d.iter()
        .map(|x| format!("{}: {}", x.path, x.message))
        .collect::<Vec<_>>()
        .join("; ")
}

/// What of the base simulation the loop would drop (D4), `None` when it is plain.
fn not_plain(sim: &Simulation) -> Option<&'static str> {
    let Engine::Steady(s) = &sim.engine else {
        return Some("an engine other than the steady simple / simplec solver");
    };
    let model = [
        (sim.turb.is_some(), "a turbulence model"),
        (sim.energy.is_some(), "the energy equation"),
        (sim.scalars.is_some(), "transported scalars"),
        (sim.chem.is_some(), "chemistry"),
        (sim.electric.is_some(), "the electric potential"),
        (sim.phase.is_some(), "phase change"),
        (sim.cht.is_some(), "conjugate heat transfer"),
        (sim.zone_sources.is_some(), "zone sources"),
        (sim.sources.is_some(), "momentum sources"),
        (sim.components.is_some(), "lumped components"),
        (sim.radiation.is_some(), "radiation"),
        (sim.rheology.is_some(), "rheology"),
        (sim.props.is_some(), "temperature-dependent properties"),
        (!sim.udfs.is_empty(), "user functions"),
        (
            !sim.expr_bcs.is_empty(),
            "a velocity expression or user function on a patch",
        ),
        (sim.catalogue.is_some(), "a boundary-condition catalogue entry"),
        (!sim.marangoni.is_empty(), "a Marangoni stress"),
        (sim.base_body.is_some(), "a body force"),
        (sim.dynmesh.is_some(), "a moving mesh"),
        (sim.stats.is_some(), "statistics"),
        (sim.particles.is_some() || sim.two_way.is_some(), "particles"),
        (sim.mixing.is_some(), "a mixing plane"),
        (!sim.conditions.is_empty(), "convergence conditions"),
    ];
    if let Some((_, what)) = model.iter().find(|(on, _)| *on) {
        return Some(what);
    }
    let fresh = SimpleSolver::new(s.geom.clone(), s.u.clone(), s.p.clone(), s.settings.clone());
    let solver = [
        (s.nu_face.is_some(), "a face viscosity"),
        (
            s.body_force.is_some() || s.driver.is_some(),
            "a body force or bulk-velocity driver",
        ),
        (s.sink.is_some() || s.immersed.is_some(), "a sink or immersed forcing"),
        (s.pseudo.is_some() || s.pseudo_control.is_some(), "pseudo-time stepping"),
        (s.linear_upwind_limiter.is_some(), "a linear-upwind limiter"),
        (s.periodic_gradient_partner, "the partner-cell periodic gradient"),
        (s.reconstructed_correction, "the face-flux velocity correction"),
        (s.simplec_boundary_compensation, "the SIMPLEC boundary compensation"),
        (
            s.gradient_momentum.is_some() || s.gradient_pressure.is_some(),
            "gradient options of the momentum or pressure equation",
        ),
        (
            s.residual_norm != fresh.residual_norm,
            "a residual norm other than reference",
        ),
        (
            s.pressure_interpolation != fresh.pressure_interpolation,
            "a pressure interpolation other than linear",
        ),
        (s.phi != fresh.phi, "a prescribed face flux"),
    ];
    solver.iter().find(|(on, _)| *on).map(|(_, what)| *what)
}

/// `Simulation::new` of a case with the table (D2, D3): the base lattice's simulation with the loop pending.
pub(super) fn new_adaptive(case: &Case) -> std::result::Result<Simulation, Vec<Diagnostic>> {
    let errs = check(case);
    if !errs.is_empty() {
        return Err(errs);
    }
    let amr = base_lattice(case)?;
    let mesh = amr.to_mesh().map_err(|e| diag(TABLE, e.to_string()))?;
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).map_err(|e| diag("mesh", e.to_string()))?);
    let mut sim = Simulation::from_geometry(case, geom)?;
    if let Some(what) = not_plain(&sim) {
        return Err(vec![refused(TABLE, what)]);
    }
    sim.adapt = Some(AdaptState {
        pending: true,
        cycles: Vec::new(),
        lattice: None,
    });
    Ok(sim)
}

/// The fields with their conditions and the solver settings of the case on `geom` (the `setup` of the loop, D3).
fn setup(case: &Case, geom: &Arc<FvGeometry>) -> Result<(VectorField, ScalarField, SimpleSettings)> {
    let sim = Simulation::from_geometry(case, geom.clone()).map_err(|d| Error::Other(joined(&d)))?;
    match sim.engine {
        Engine::Steady(s) => {
            let s = *s;
            Ok((s.u, s.p, s.settings))
        }
        _ => Err(Error::Other("the case does not build the steady solver".into())),
    }
}

#[derive(Clone, Copy)]
enum Indicator {
    Gradient { scaled: bool },
    EdgeLength(f64),
}

/// The adaptive loop of the case (D3), with the library's functions.
pub(crate) fn run(case: &Case) -> Result<AdaptiveRun> {
    run_until(case, &|| false)
}

/// The mesh the run of a `[mesh.adapt]` case solves on after its first step (F-475, ADR 0377): the final mesh of the
/// run's loop, with one record per solve.
pub struct AdaptedMesh {
    /// The mesh of the last solve (the one the first step puts in `Simulation::geom`).
    pub mesh: Arc<rcfd_mesh::Mesh>,
    /// One record per solve.
    pub cycles: Vec<CycleRecord>,
}

/// The run's adaptive loop of `case` (the first step's, [`run`]) for its final mesh (F-475, ADR 0377). `cancelled` is
/// polled once before each of the `cycles + 1` solves and nowhere else; `Ok(None)` when it returned true (the loop
/// stopped before that solve; a solve in progress is not interrupted).
pub fn adapted_mesh(case: &Case, cancelled: &dyn Fn() -> bool) -> Result<Option<AdaptedMesh>> {
    let stopped = std::cell::Cell::new(false);
    let poll = || {
        let c = cancelled();
        if c {
            stopped.set(true);
        }
        c
    };
    match run_until(case, &poll) {
        _ if stopped.get() => Ok(None),
        Ok(r) => Ok(Some(AdaptedMesh {
            mesh: r.solver.geom.mesh.clone(),
            cycles: r.cycles,
        })),
        Err(e) => Err(e),
    }
}

/// [`run`] with `cancelled` polled before each solve (in `setup`); a true poll stops the loop with an error.
fn run_until(case: &Case, cancelled: &dyn Fn() -> bool) -> Result<AdaptiveRun> {
    let amr = base_lattice(case).map_err(|d| Error::Other(joined(&d)))?;
    let set = settings(case);
    let axes = amr.refined_axes();
    let indicator = match get_s(case, "mesh.adapt.indicator").as_str() {
        "edge_length" => Indicator::EdgeLength(get_f(case, "mesh.adapt.edge_length_exponent")),
        other => Indicator::Gradient {
            scaled: other != "velocity_gradient_unscaled",
        },
    };
    let eta = move |s: &SimpleSolver| -> Result<Vec<f64>> {
        match indicator {
            Indicator::Gradient { scaled } => velocity_gradient_indicator(s, axes, scaled),
            Indicator::EdgeLength(r) => velocity_edge_length_indicator(s, axes, r),
        }
    };
    let target = get_s(case, "mesh.adapt.marking") == "target_count";
    let n_target = get_i(case, "mesh.adapt.target_cells").max(0) as usize;
    let reg = get_s(case, "mesh.adapt.register");
    let cell_setup = |g: &Arc<FvGeometry>| {
        if cancelled() {
            return Err(Error::Other("cancelled".into()));
        }
        setup(case, g)
    };
    if reg.is_empty() {
        return match (target, indicator) {
            (true, _) => run_adaptive_simple_target(amr, cell_setup, &set, n_target, eta),
            (false, Indicator::Gradient { .. }) => run_adaptive_simple(amr, cell_setup, &set),
            (false, Indicator::EdgeLength(_)) => run_adaptive_simple_with(amr, cell_setup, &set, eta),
        };
    }
    let refine = get_s(case, "mesh.adapt.register_use") == "refine";
    let (n0, cycles, fraction) = (amr.n_cells(), set.cycles, set.refine_fraction);
    let children = children_per_split(axes);
    let marker = move |s: &SimpleSolver, lattice: &AmrMesh, k: usize| {
        let mask = crate::registers::zone_from_setting(case, "mesh.adapt.register", &s.geom.mesh, None)
            .map_err(|d| Error::Other(format!("{}: {}", d.path, d.message)))?;
        let register = Register::from_mask(mask);
        let levels = lattice.levels();
        if refine {
            return register_marks(&register, &levels, lattice.max_level());
        }
        let ind = eta(s)?;
        let marks = if target {
            target_count_marks(
                &ind,
                &levels,
                lattice.max_level(),
                target_schedule(n0, n_target, cycles, k),
                children,
            )?
        } else {
            fixed_fraction_marks(&ind, &levels, lattice.max_level(), fraction)?
        };
        restrict_marks(&marks, &register)
    };
    run_adaptive_simple_marked(amr, cell_setup, &set, marker)
}

impl Simulation {
    /// Whether the adaptive loop is still to run.
    pub(crate) fn adapt_pending(&self) -> bool {
        self.adapt.as_ref().is_some_and(|a| a.pending)
    }

    /// The one step of an adaptive run (D3): the whole loop, then the final solver and its mesh; one monitor row.
    pub(super) fn step_adaptive(&mut self) -> StepSummary {
        let case = self.case.clone();
        let outcome = run(&case);
        let mut summary = StepSummary {
            index: 0,
            time: 0.0,
            res_u: 0.0,
            res_p: 0.0,
            continuity: 0.0,
            done: true,
            converged: false,
        };
        match outcome {
            Ok(AdaptiveRun {
                amr,
                solver,
                cycles: records,
            }) => {
                if let Some(i) = solver.history.last() {
                    summary.index = i.iteration;
                    summary.res_u = i.res_u;
                    summary.res_p = i.res_p;
                    summary.continuity = i.continuity;
                }
                summary.converged = records.last().is_some_and(|c| c.converged);
                self.geom = solver.geom.clone();
                self.engine = Engine::Steady(Box::new(solver));
                let x = summary.index as f64;
                self.monitors.push("res_u", "", x, summary.res_u);
                self.monitors.push("res_p", "", x, summary.res_p);
                self.monitors.push("continuity", "", x, summary.continuity);
                for (rep, v) in self.reports.clone().iter().zip(self.eval_all_reports()) {
                    if let Ok(v) = v {
                        self.monitors.push(&rep.name, &rep.unit(), x, v);
                    }
                }
                let solves: Vec<String> = records
                    .iter()
                    .map(|c| {
                        format!(
                            "{} cells, {} iterations{}",
                            c.cells,
                            c.iterations,
                            if c.converged { "" } else { ", not converged" }
                        )
                    })
                    .collect();
                self.stop_reason = Some(format!(
                    "mesh.adapt: {} solves ({}); the last {}",
                    records.len(),
                    solves.join("; "),
                    if summary.converged {
                        "met mesh.adapt.tolerance"
                    } else {
                        "did not meet mesh.adapt.tolerance"
                    }
                ));
                if let Some(a) = self.adapt.as_mut() {
                    a.cycles = records;
                    a.lattice = Some(amr);
                }
            }
            Err(e) => {
                let m = format!("mesh.adapt: {e}");
                self.stop_reason = Some(m.clone());
                self.failure = Some(m);
            }
        }
        if let Some(a) = self.adapt.as_mut() {
            a.pending = false;
        }
        self.done = true;
        self.converged = summary.converged;
        self.last = Some(summary);
        summary
    }
}
