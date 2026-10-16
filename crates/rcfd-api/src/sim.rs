//! A runnable simulation built from a [`Case`]: mesh, fields, solver,
//! monitors and reports (F-082). This is the programmatic API used by the
//! CLI, the solver service and the MCP tools.

use crate::case::{Case, Diagnostic};
use crate::chemistry::ChemState;
use crate::cht::ChtModel;
use crate::comp::{self, CompState};
use crate::exprs;
use crate::lumped::Components;
use crate::properties::Properties;
use crate::radiation::Radiation;
use crate::rheology::Rheology;
use crate::scalars::{ScalarState, StepCtx};
use crate::sources::Sources;
use crate::stats::{self, Statistics};
use crate::turb::{self, TurbState};
use crate::udf::Udf;
use crate::vd::{self, VdState};
use crate::wall_motion;
use crate::zone_sources::ZoneSources;
use rcfd_core::expr::Compiled;
use rcfd_core::units::Dimension;
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_fv::bc::{Bc, FixedGradient, FixedValue, Freestream, Profile, Symmetry, ZeroGradient};
use rcfd_fv::convection::{ConvScheme, TvdLimiter};
use rcfd_fv::coupled::{CoupledSettings, CoupledSolver};
use rcfd_fv::ddt::{CflController, TimeScheme};
use rcfd_fv::energy::{boussinesq_force, EnergySolver};
use rcfd_fv::gradient::green_gauss;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::pressure::{boundary_values, pressure_bc};
use rcfd_fv::simple::{
    BodyForce, Coupling, MeanVelocityDriver, PseudoMode, PseudoTransient, SimpleSettings, SimpleSolver, SimpleState,
};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{self, Grading};
use rcfd_mesh::{Mesh, PatchKind};
use rcfd_post::convergence::{self, Combine, Condition, ConditionKind};
use rcfd_post::integrals as ig;
use rcfd_post::interpolate::{InterpMode, Interpolator};
use rcfd_post::monitors::{MonitorStore, ReportValue};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod restart;

pub(crate) fn diag(path: &str, message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }]
}

pub(crate) fn get_f(c: &Case, path: &str) -> f64 {
    c.tree.get(path).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

pub(crate) fn get_i(c: &Case, path: &str) -> i64 {
    c.tree.get(path).ok().and_then(|v| v.as_i64()).unwrap_or(0)
}

pub(crate) fn get_s(c: &Case, path: &str) -> String {
    c.tree
        .get(path)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

pub(crate) fn get_b(c: &Case, path: &str) -> bool {
    c.tree.get(path).ok().and_then(|v| v.as_bool()).unwrap_or(false)
}

/// A 3-vector setting; an empty list yields `default`.
pub(crate) fn get_v(c: &Case, path: &str, default: Vec3) -> Vec3 {
    match c.tree.get(path).ok().as_ref().and_then(Value::as_array) {
        Some(a) if a.len() == 3 => [
            a[0].as_f64().unwrap_or(0.0),
            a[1].as_f64().unwrap_or(0.0),
            a[2].as_f64().unwrap_or(0.0),
        ],
        _ => default,
    }
}

/// Convection scheme from its settings name.
pub fn conv_scheme(name: &str) -> Option<ConvScheme> {
    Some(match name {
        "upwind" => ConvScheme::Upwind,
        "central" => ConvScheme::Central,
        "linear_upwind" => ConvScheme::LinearUpwind,
        "quick" => ConvScheme::Quick,
        "tvd_minmod" => ConvScheme::Tvd(TvdLimiter::Minmod),
        "tvd_van_leer" => ConvScheme::Tvd(TvdLimiter::VanLeer),
        "tvd_van_albada" => ConvScheme::Tvd(TvdLimiter::VanAlbada),
        "tvd_superbee" => ConvScheme::Tvd(TvdLimiter::Superbee),
        "tvd_muscl" => ConvScheme::Tvd(TvdLimiter::Muscl),
        "tvd_koren" => ConvScheme::Tvd(TvdLimiter::Koren),
        "blended" => ConvScheme::Blended(0.75),
        "high_resolution" => ConvScheme::Tvd(TvdLimiter::Gamma(0.2)),
        "compressive" => ConvScheme::Tvd(TvdLimiter::Compressive(1.5)),
        "bounded_central" => ConvScheme::Tvd(TvdLimiter::BoundedCentral),
        _ => return None,
    })
}

/// Scheme of `solver.convection` with its parameters (`solver.blend_factor`, `solver.high_resolution_beta`,
/// `solver.compressive_k`) from the case, upwind if the name is unknown.
pub fn solver_conv_scheme(case: &Case) -> ConvScheme {
    match get_s(case, "solver.convection").as_str() {
        "blended" => ConvScheme::Blended(get_f(case, "solver.blend_factor")),
        "high_resolution" => ConvScheme::Tvd(TvdLimiter::Gamma(get_f(case, "solver.high_resolution_beta"))),
        "compressive" => ConvScheme::Tvd(TvdLimiter::Compressive(get_f(case, "solver.compressive_k"))),
        name => conv_scheme(name).unwrap_or(ConvScheme::Upwind),
    }
}

/// Time scheme of `solver.time_scheme`.
pub fn solver_time_scheme(case: &Case) -> TimeScheme {
    match get_s(case, "solver.time_scheme").as_str() {
        "bdf2" => TimeScheme::Bdf2,
        "bdf2_bounded" => TimeScheme::Bdf2Bounded,
        _ => TimeScheme::ImplicitEuler,
    }
}

/// y grading from the boundary-layer inflation settings (`mesh.wall_layers`), or `None` when off.
fn wall_layer_grading(case: &Case) -> std::result::Result<Option<Grading>, Vec<Diagnostic>> {
    let layers = get_i(case, "mesh.wall_layers").max(0) as usize;
    if layers == 0 {
        return Ok(None);
    }
    let mut first = get_f(case, "mesh.first_layer_height");
    if first <= 0.0 {
        let yp = get_f(case, "mesh.target_y_plus");
        if yp <= 0.0 {
            return Err(diag(
                "mesh.wall_layers",
                "wall layers need mesh.first_layer_height > 0 or mesh.target_y_plus > 0",
            ));
        }
        let nu = get_f(case, "physics.nu");
        if nu <= 0.0 {
            return Err(diag("physics.nu", "mesh.target_y_plus needs a positive physics.nu"));
        }
        let mut ut = get_f(case, "mesh.layer_u_tau");
        if ut <= 0.0 {
            let (u, x) = (
                get_f(case, "mesh.layer_reference_velocity"),
                get_f(case, "mesh.layer_reference_length"),
            );
            if u <= 0.0 || x <= 0.0 {
                return Err(diag(
                    "mesh.target_y_plus",
                    "set mesh.layer_u_tau, or mesh.layer_reference_velocity and mesh.layer_reference_length, to turn a target y+ into a first layer height",
                ));
            }
            ut = rcfd_mesh::layers::flat_plate_u_tau(u, nu, x);
        }
        first = rcfd_mesh::layers::first_cell_height_from_y_plus(yp, ut, nu);
    }
    let both = get_s(case, "mesh.layer_walls") != "lower";
    let ny = get_i(case, "mesh.ny").max(1) as usize;
    let g = Grading::WallLayers {
        first,
        growth: get_f(case, "mesh.layer_growth"),
        layers,
        both,
    };
    // reject impossible layer stacks with a located message instead of a mesh error
    gen::axis_coords(ny, get_f(case, "mesh.ly"), g).map_err(|e| diag("mesh.wall_layers", e.to_string()))?;
    Ok(Some(g))
}

/// Reject mesh options the chosen generator does not apply (instead of silently ignoring them).
fn check_mesh_options(case: &Case, generator: &str) -> std::result::Result<(), Vec<Diagnostic>> {
    let mut out = Vec::new();
    let mut bad = |path: &str, what: &str| {
        out.push(
            diag(
                path,
                format!("mesh.generator = \"{generator}\" does not use {what}; remove it"),
            )
            .remove(0),
        );
    };
    if get_i(case, "mesh.wall_layers") > 0 && !matches!(generator, "box_2d" | "channel_2d") {
        bad(
            "mesh.wall_layers",
            "boundary-layer inflation (box_2d and channel_2d only)",
        );
    }
    let graded = (get_f(case, "mesh.grading") - 1.0).abs() > 1e-12;
    if graded && matches!(generator, "cavity_2d" | "wedge_2d" | "nozzle_2d") {
        bad("mesh.grading", "grading");
    }
    if generator != "box_2d" {
        for p in ["mesh.periodic_x", "mesh.periodic_y"] {
            if get_b(case, p) {
                bad(p, "periodic pairs (box_2d only)");
            }
        }
    }
    if out.is_empty() {
        Ok(())
    } else {
        Err(out)
    }
}

/// Build the mesh described by the case.
pub fn build_mesh(case: &Case) -> std::result::Result<Mesh, Vec<Diagnostic>> {
    let me = |e: Error| diag("mesh", e.to_string());
    if get_s(case, "mesh.source") == "file" {
        let file = get_s(case, "mesh.file");
        let path = if Path::new(&file).is_absolute() {
            PathBuf::from(&file)
        } else {
            case.base_dir.join(&file)
        };
        let mesh = if path.is_dir() {
            rcfd_io::openfoam::read_dir(&path)
        } else {
            rcfd_io::read_msh_file(&path)
        }
        .map_err(|e| diag("mesh.file", format!("{}: {e}", path.display())))?;
        let s = get_f(case, "mesh.scale");
        return if (s - 1.0).abs() > 1e-15 {
            gen::warp(&mesh, |p| vec3::scale(p, s)).map_err(me)
        } else {
            Ok(mesh)
        };
    }
    let (nx, ny) = (get_i(case, "mesh.nx") as usize, get_i(case, "mesh.ny") as usize);
    let (lx, ly, depth) = (
        get_f(case, "mesh.lx"),
        get_f(case, "mesh.ly"),
        get_f(case, "mesh.depth"),
    );
    let grading = get_f(case, "mesh.grading");
    check_mesh_options(case, &get_s(case, "mesh.generator"))?;
    match get_s(case, "mesh.generator").as_str() {
        "cavity_2d" => gen::cavity_2d_rect(nx, ny, lx, ly).map_err(me),
        "box_2d" => {
            // grading != 1 clusters cells towards both walls of each direction (symmetric grading)
            let g = if (grading - 1.0).abs() < 1e-12 {
                Grading::Uniform
            } else {
                Grading::Symmetric(grading)
            };
            let gy = wall_layer_grading(case)?.unwrap_or(g);
            gen::box_2d_graded(
                nx,
                ny,
                lx,
                ly,
                [g, gy],
                get_b(case, "mesh.periodic_x"),
                get_b(case, "mesh.periodic_y"),
            )
            .map_err(me)
        }
        "channel_2d" => {
            let gy = if (grading - 1.0).abs() < 1e-12 {
                Grading::Uniform
            } else {
                Grading::Symmetric(grading)
            };
            let gy = wall_layer_grading(case)?.unwrap_or(gy);
            gen::channel(
                [nx, ny, 1],
                [lx, ly, depth],
                [Grading::Uniform, gy, Grading::Uniform],
                true,
            )
            .map_err(me)
        }
        "channel_2d_interface" => {
            // two channel blocks meeting at x = lx/2 through a non-conformal interface (F-114)
            let (nxr, nyr) = (
                get_i(case, "mesh.nx_right") as usize,
                get_i(case, "mesh.ny_right") as usize,
            );
            let (nxr, nyr) = (if nxr == 0 { nx } else { nxr }, if nyr == 0 { ny } else { nyr });
            let gy = if (grading - 1.0).abs() < 1e-12 {
                Grading::Uniform
            } else {
                Grading::Symmetric(grading)
            };
            let half = 0.5 * lx;
            let block = |nx: usize, ny: usize, x0: f64| {
                let m = gen::channel(
                    [nx, ny, 1],
                    [half, ly, depth],
                    [Grading::Uniform, gy, Grading::Uniform],
                    true,
                )?;
                gen::warp(&m, |p| [p[0] + x0, p[1], p[2]])
            };
            let (a, b) = (block(nx, ny, 0.0).map_err(me)?, block(nxr, nyr, half).map_err(me)?);
            rcfd_mesh::non_conformal::join_non_conformal(&a, "outlet", &b, "inlet", 1e-8)
                .map(|(m, _)| m)
                .map_err(me)
        }
        "wedge_2d" => {
            // box_2d whose lower wall turns upward by ramp_angle at ramp_start (the top boundary stays flat)
            let base = gen::box_2d(nx, ny, lx, ly, false, false).map_err(me)?;
            let (x0, th) = (
                get_f(case, "mesh.ramp_start"),
                get_f(case, "mesh.ramp_angle").to_radians(),
            );
            gen::warp(&base, |p| {
                let rise = (p[0] - x0).max(0.0) * th.tan();
                [p[0], p[1] + rise * (1.0 - p[1] / ly), p[2]]
            })
            .map_err(me)
        }
        "nozzle_2d" => {
            // upper half of a converging-diverging duct: cosine wall from the inlet half-height ly through the
            // throat half-height to the exit half-height (the flat-top box_2d scaled in y)
            let ht = get_f(case, "mesh.throat_height");
            let he = Some(get_f(case, "mesh.exit_height")).filter(|h| *h > 0.0).unwrap_or(ly);
            let base = gen::box_2d(nx, ny, lx, 1.0, false, false).map_err(me)?;
            gen::warp(&base, |p| {
                let s = p[0] / lx;
                let h_end = if s <= 0.5 { ly } else { he };
                let h = ht + (h_end - ht) * (1.0 + (2.0 * std::f64::consts::PI * s).cos()) / 2.0;
                [p[0], p[1] * h, p[2]]
            })
            .map_err(me)
        }
        "annulus_3d" | "annulus_3d_interface" => {
            // cylindrical annulus about z (F-031 MRF); the interface variant joins two blocks at z = lz/2
            let g = if (grading - 1.0).abs() < 1e-12 {
                Grading::Uniform
            } else {
                Grading::Expansion(grading)
            };
            let (r_in, r_out, lz) = (
                get_f(case, "mesh.r_in"),
                get_f(case, "mesh.r_out"),
                get_f(case, "mesh.lz"),
            );
            let nz = get_i(case, "mesh.nz").max(1) as usize;
            if get_s(case, "mesh.generator") == "annulus_3d" {
                gen::annulus_3d(nx, ny, nz, r_in, r_out, lz, 0.0, g).map_err(me)
            } else {
                let (nyr, nzr) = (
                    get_i(case, "mesh.ny_right") as usize,
                    get_i(case, "mesh.nz_right") as usize,
                );
                let (nyr, nzr) = (if nyr == 0 { ny } else { nyr }, if nzr == 0 { nz } else { nzr });
                let half = 0.5 * lz;
                let a = gen::annulus_3d(nx, ny, nz, r_in, r_out, half, 0.0, g).map_err(me)?;
                let b = gen::annulus_3d(nx, nyr, nzr, r_in, r_out, half, half, g).map_err(me)?;
                rcfd_mesh::non_conformal::join_non_conformal(&a, "zmax", &b, "zmin", 1e-8)
                    .map(|(m, _)| m)
                    .map_err(me)
            }
        }
        "cylinder_ogrid" => {
            let g = if (grading - 1.0).abs() < 1e-12 {
                Grading::Uniform
            } else {
                Grading::Expansion(grading)
            };
            gen::cylinder_ogrid(nx, ny, get_f(case, "mesh.r_in"), get_f(case, "mesh.r_out"), g, depth).map_err(me)
        }
        other => Err(diag("mesh.generator", format!("unknown generator '{other}'"))),
    }
}

/// What a report computes.
#[derive(Debug, Clone, PartialEq)]
pub enum ReportKind {
    /// Force of the fluid on a patch along `direction`.
    Force,
    /// Moment about `origin` along the axis `direction`.
    Moment,
    /// Force coefficient along `direction`.
    ForceCoefficient,
    /// Volume average of a field.
    VolumeAverage,
    /// Volume integral of a field.
    VolumeIntegral,
    /// Area average of a field over a patch.
    AreaAverage,
    /// Area integral of a field over a patch.
    AreaIntegral,
    /// Mass flow out of the domain through a patch.
    MassFlow,
    /// Conductive heat flow out of the domain through a patch.
    HeatFlow,
    /// Value at a point (nearest cell centre).
    Probe,
    /// Minimum of a field.
    Minimum,
    /// Maximum of a field.
    Maximum,
    /// Expression of other reports (evaluated after all other reports).
    Expression,
    /// Wall y+ statistic over a wall patch (k-epsilon runs).
    YPlus,
    /// Net outflow of a passive scalar or species through a patch (conservation imbalance without a patch).
    ScalarFlux,
    /// Volume average of a passive scalar or species.
    ScalarAverage,
}

/// A report definition read from the settings tree.
#[derive(Debug, Clone)]
pub struct ReportDef {
    /// Report name (key in `reports`).
    pub name: String,
    /// Kind.
    pub kind: ReportKind,
    /// Patch name.
    pub patch: String,
    /// Field name.
    pub field: String,
    /// `total`, `pressure` or `viscous`.
    pub component: String,
    /// Direction or axis.
    pub direction: Vec3,
    /// Moment origin.
    pub origin: Vec3,
    /// Probe point.
    pub point: Vec3,
    /// Reference area.
    pub a_ref: f64,
    /// Reference velocity.
    pub u_ref: f64,
    /// Reference length.
    pub l_ref: f64,
    /// Thermal conductivity.
    pub conductivity: f64,
    /// Expression source (`type = expression`).
    pub expression: String,
    /// Unit label of an expression report (its dimension, set when compiled).
    pub expr_unit: String,
    /// `average`, `min` or `max` (`y_plus` reports).
    pub statistic: String,
    /// Passive scalar or species name (`scalar_flux`, `scalar_average`).
    pub scalar: String,
}

impl ReportDef {
    /// Read all report definitions of a case.
    pub fn from_case(case: &Case) -> Vec<ReportDef> {
        let names = case.tree.names("reports").unwrap_or_default();
        names
            .into_iter()
            .map(|n| {
                let p = |f: &str| format!("reports.{n}.{f}");
                let kind = match get_s(case, &p("type")).as_str() {
                    "force" => ReportKind::Force,
                    "moment" => ReportKind::Moment,
                    "force_coefficient" => ReportKind::ForceCoefficient,
                    "volume_integral" => ReportKind::VolumeIntegral,
                    "area_average" => ReportKind::AreaAverage,
                    "area_integral" => ReportKind::AreaIntegral,
                    "mass_flow" => ReportKind::MassFlow,
                    "heat_flow" => ReportKind::HeatFlow,
                    "probe" => ReportKind::Probe,
                    "minimum" => ReportKind::Minimum,
                    "maximum" => ReportKind::Maximum,
                    "expression" => ReportKind::Expression,
                    "y_plus" => ReportKind::YPlus,
                    "scalar_flux" => ReportKind::ScalarFlux,
                    "scalar_average" => ReportKind::ScalarAverage,
                    _ => ReportKind::VolumeAverage,
                };
                ReportDef {
                    kind,
                    patch: get_s(case, &p("patch")),
                    field: get_s(case, &p("field")),
                    component: get_s(case, &p("component")),
                    direction: get_v(case, &p("direction"), [1.0, 0.0, 0.0]),
                    origin: get_v(case, &p("origin"), [0.0; 3]),
                    point: get_v(case, &p("point"), [0.0; 3]),
                    a_ref: get_f(case, &p("a_ref")),
                    u_ref: get_f(case, &p("u_ref")),
                    l_ref: get_f(case, &p("l_ref")),
                    conductivity: get_f(case, &p("conductivity")),
                    expression: get_s(case, &p("expression")),
                    expr_unit: String::new(),
                    statistic: get_s(case, &p("statistic")),
                    scalar: get_s(case, &p("scalar")),
                    name: n,
                }
            })
            .collect()
    }

    fn unit(&self) -> String {
        match self.kind {
            ReportKind::Force => "N".into(),
            ReportKind::Moment => "N m".into(),
            ReportKind::MassFlow => "kg/s".into(),
            ReportKind::HeatFlow => "W".into(),
            ReportKind::Expression => self.expr_unit.clone(),
            _ => String::new(),
        }
    }
}

/// A fixed-velocity boundary whose components are expressions of position and time.
struct ExprBc {
    patch: String,
    comps: [Option<Compiled>; 3],
    base: Vec3,
    /// User function that supplies the components without an expression.
    udf: Option<String>,
}

impl ExprBc {
    /// Per-face velocity at time `t`.
    fn profile(
        &self,
        geom: &FvGeometry,
        t: f64,
        udfs: &mut BTreeMap<String, Udf>,
    ) -> std::result::Result<Profile, String> {
        let patch = geom.mesh.patch(&self.patch).expect("patch checked at build time");
        let cf = geom.mesh.face_centres();
        let mut out = Vec::new();
        for f in patch.faces() {
            let slots = [cf[f][0], cf[f][1], cf[f][2], t];
            let from_udf = match self.udf.as_ref().and_then(|n| udfs.get_mut(n)) {
                Some(u) => Some(u.velocity_profile(cf[f], t)?),
                None => None,
            };
            out.push(std::array::from_fn(|k| match (&self.comps[k], from_udf) {
                (Some(c), _) => c.eval(&slots),
                (None, Some(v)) => v[k],
                (None, None) => self.base[k],
            }));
        }
        Ok(Profile::PerFace(out))
    }
}

/// Energy equation state of a steady buoyant simulation.
struct EnergyState {
    solver: EnergySolver,
    beta: f64,
    gravity: Vec3,
    t_ref: f64,
    buoyancy: bool,
    /// Thermal conductivity (W/m/K).
    k: f64,
    res_t: f64,
}

enum Engine {
    Steady(Box<SimpleSolver>),
    Coupled(Box<CoupledSolver>),
    Transient(Box<PimpleSolver>),
    Compressible(Box<CompState>),
    VarDensity(Box<VdState>),
}

/// Outcome of one iteration or time step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepSummary {
    /// Iteration / step number.
    pub index: usize,
    /// Simulation time (transient; 0 for steady).
    pub time: f64,
    /// Normalised momentum residual.
    pub res_u: f64,
    /// Normalised pressure residual.
    pub res_p: f64,
    /// Continuity error.
    pub continuity: f64,
    /// Run finished (converged, iteration cap or end time reached).
    pub done: bool,
    /// Steady residual targets met.
    pub converged: bool,
}

/// A configured, runnable simulation.
pub struct Simulation {
    /// Settings used to build it.
    pub case: Case,
    /// Finite-volume geometry.
    pub geom: Arc<FvGeometry>,
    dynmesh: Option<crate::dynmesh::DynMesh>,
    engine: Engine,
    /// Recorded residuals, courant numbers and report histories.
    pub monitors: MonitorStore,
    reports: Vec<ReportDef>,
    expr_bcs: Vec<ExprBc>,
    /// Patches with a thermocapillary stress (`marangoni`): patch name and d sigma/d T.
    marangoni: Vec<(String, f64)>,
    /// Boundary-condition catalogue patches (ADR 0022), refreshed before every iteration or step.
    catalogue: Option<crate::bc_cat::Catalogue>,
    energy: Option<EnergyState>,
    turb: Option<TurbState>,
    /// Uniform body acceleration of the momentum equation (steady runs).
    base_body: Option<BodyForce>,
    /// Sandboxed user functions (F-122).
    udfs: BTreeMap<String, Udf>,
    udf_momentum: Option<String>,
    udf_viscosity: Option<String>,
    udf_monitors: Vec<String>,
    /// Generalised-Newtonian viscosity law (F-110).
    rheology: Option<Rheology>,
    /// Porous, rotating-frame and acceleration sources (F-031).
    sources: Option<Sources>,
    /// Lumped zone components: fans, porous jumps, heat exchangers, network outlets (F-113).
    components: Option<Components>,
    /// Rosseland / P1 radiation (F-130).
    radiation: Option<Radiation>,
    scalars: Option<ScalarState>,
    /// Chemical reactions of the species (F-054, ADR 0027).
    chem: Option<ChemState>,
    /// Zone sources and fixed values of the temperature and the scalars (F-201).
    zone_sources: Option<ZoneSources>,
    /// Conjugate heat transfer: solid zones, contact resistances, prescribed flow (ADR 0024).
    cht: Option<ChtModel>,
    /// Iterations performed with a prescribed flow (the flow solver is not advanced).
    frozen_iter: usize,
    props: Option<Properties>,
    t_init: f64,
    /// Run-time statistics of transient runs (F-116).
    stats: Option<Statistics>,
    /// Compiled expression of each report (aligned with `reports`; `Some` for expression reports).
    report_exprs: Vec<Option<Compiled>>,
    /// Names of the non-expression reports, in expression-slot order.
    report_syms: Vec<String>,
    nu: f64,
    rho: f64,
    max_iter: usize,
    end_time: f64,
    dt: f64,
    cfl: Option<CflController>,
    tol_u: f64,
    tol_p: f64,
    done: bool,
    converged: bool,
    last: Option<StepSummary>,
    conditions: Vec<Condition>,
    combine: Combine,
    min_iterations: usize,
    stop_reason: Option<String>,
    failure: Option<String>,
    /// Solution limits (`limits.*`, ADR 0016): `(enabled, min, max)` for temperature and transported scalars.
    limit_t: (bool, f64, f64),
    limit_s: (bool, f64, f64),
    /// Cells clipped by the solution limits so far (counted per cell and step).
    clipped: usize,
}

impl Simulation {
    /// Build mesh, fields and solver from a validated case.
    pub fn new(case: &Case) -> std::result::Result<Self, Vec<Diagnostic>> {
        let mesh = build_mesh(case)?;
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).map_err(|e| diag("mesh", e.to_string()))?);
        let uinit = get_v(case, "initial.velocity", [0.0; 3]);
        let mut u = VectorField::uniform("U", &geom, uinit);
        let mut p = ScalarField::uniform("p", &geom, get_f(case, "initial.pressure"));
        let energy_on = get_b(case, "energy.enabled");
        let limit_of = |k: &str| {
            (
                get_b(case, &format!("limits.{k}.enabled")),
                get_f(case, &format!("limits.{k}.min")),
                get_f(case, &format!("limits.{k}.max")),
            )
        };
        let (limit_t, limit_s) = (limit_of("temperature"), limit_of("scalar"));
        for (k, l) in [("temperature", limit_t), ("scalar", limit_s)] {
            if l.0 && l.1 >= l.2 {
                return Err(diag(
                    &format!("limits.{k}.min"),
                    format!("min {} must be below max {}", l.1, l.2),
                ));
            }
        }
        if limit_t.0 && !get_b(case, "energy.enabled") {
            return Err(diag("limits.temperature.enabled", "needs energy.enabled = true"));
        }
        let compressible = comp::enabled(case);
        let vdens = vd::enabled(case);
        let mut temp = ScalarField::uniform("T", &geom, get_f(case, "initial.temperature"));
        let mut errs = Vec::new();
        let sctx = exprs::spatial_context(case);
        // initial velocity expressions of x, y, z (cell centres, t = 0); components without one keep initial.velocity
        for (k, c) in ["x", "y", "z"].iter().enumerate() {
            match exprs::compile_bc(
                case,
                &format!("initial.velocity_expression_{c}"),
                Dimension::VELOCITY,
                &sctx,
            ) {
                Ok(Some(ex)) => {
                    for (v, p) in u.values.iter_mut().zip(geom.mesh.cell_centres()) {
                        v[k] = ex.eval(&[p[0], p[1], p[2], 0.0]);
                    }
                }
                Ok(None) => {}
                Err(e) => errs.extend(e),
            }
        }
        errs.extend(crate::registers::apply_initial_patches(
            case,
            &geom.mesh,
            &mut u.values,
            &mut p.values,
            &mut temp.values,
        ));
        let cht = if compressible || vdens {
            None
        } else {
            ChtModel::from_case(case, &geom, solver_conv_scheme(case).needs_gradient())?
        };
        let mut expr_bcs: Vec<ExprBc> = Vec::new();
        let mut marangoni: Vec<(String, f64)> = Vec::new();
        let (mut udfs, udf_diags) = crate::udf::load_case_udfs(case);
        errs.extend(udf_diags);
        let flagged = |flag: &str| udfs.keys().find(|n| get_b(case, &format!("udf.{n}.{flag}"))).cloned();
        let (udf_momentum, udf_viscosity) = (flagged("momentum_source"), flagged("viscosity"));
        let udf_monitors: Vec<String> = udfs
            .keys()
            .filter(|n| get_b(case, &format!("udf.{n}.monitor")))
            .cloned()
            .collect();
        for patch in geom.mesh.patches() {
            if compressible || matches!(patch.kind, PatchKind::Empty | PatchKind::Cyclic) {
                continue;
            }
            let b = format!("boundaries.{}", patch.name);
            if case.tree.get(&b).is_err() {
                errs.push(Diagnostic {
                    path: b,
                    line: None,
                    message: format!("no boundary condition for mesh patch '{}'", patch.name),
                });
                continue;
            }
            let vval = get_v(case, &format!("{b}.velocity_value"), [0.0; 3]);
            let mut comps: [Option<Compiled>; 3] = [None, None, None];
            if get_s(case, &format!("{b}.velocity")) == "fixed" {
                for (k, c) in ["x", "y", "z"].iter().enumerate() {
                    match exprs::compile_bc(
                        case,
                        &format!("{b}.velocity_expression_{c}"),
                        Dimension::VELOCITY,
                        &sctx,
                    ) {
                        Ok(v) => comps[k] = v,
                        Err(e) => errs.extend(e),
                    }
                }
            }
            let udf_name =
                Some(get_s(case, &format!("{b}.velocity_udf"))).filter(|n| !n.is_empty() && udfs.contains_key(n));
            let ubc: Box<dyn Bc> = if comps.iter().any(Option::is_some) || udf_name.is_some() {
                let eb = ExprBc {
                    patch: patch.name.clone(),
                    comps,
                    base: vval,
                    udf: udf_name,
                };
                let prof0 = match eb.profile(&geom, 0.0, &mut udfs) {
                    Ok(p) => p,
                    Err(e) => {
                        errs.push(Diagnostic {
                            path: format!("{b}.velocity_udf"),
                            line: None,
                            message: e,
                        });
                        Profile::Uniform(vval)
                    }
                };
                if let Profile::PerFace(v) = &prof0 {
                    if v.iter().any(|f| f.iter().any(|c| !c.is_finite())) {
                        errs.push(Diagnostic {
                            path: format!("{b}.velocity"),
                            line: None,
                            message: exprs::NONFINITE_BC.into(),
                        });
                    }
                }
                let bc = FixedValue(prof0);
                expr_bcs.push(eb);
                Box::new(bc)
            } else {
                match get_s(case, &format!("{b}.velocity")).as_str() {
                    "zero_gradient" => Box::new(ZeroGradient),
                    "symmetry" => Box::new(Symmetry),
                    "freestream" => Box::new(Freestream {
                        u_ref: vval,
                        value: vval,
                        fixed_on_inflow: true,
                    }),
                    k if wall_motion::KINDS.contains(&k) => {
                        if k == "marangoni" {
                            if !matches!(
                                get_s(case, "solver.algorithm").as_str(),
                                "simple" | "simplec" | "piso" | "pimple"
                            ) {
                                errs.push(Diagnostic {
                                    path: format!("{b}.velocity"),
                                    line: None,
                                    message:
                                        "marangoni is available with the simple, simplec, piso and pimple algorithms"
                                            .into(),
                                });
                            }
                            marangoni.push((patch.name.clone(), get_f(case, &format!("{b}.marangoni_coefficient"))));
                        }
                        match wall_motion::build(
                            case,
                            &geom,
                            &patch.name,
                            &b,
                            get_f(case, "physics.rho") * get_f(case, "physics.nu"),
                        ) {
                            Ok(bc) => bc,
                            Err(e) => {
                                errs.extend(e);
                                Box::new(FixedValue::vector([0.0; 3]))
                            }
                        }
                    }
                    _ => Box::new(FixedValue::vector(vval)),
                }
            };
            let pval = get_f(case, &format!("{b}.pressure_value"));
            let pbc: Box<dyn Bc> = match get_s(case, &format!("{b}.pressure")).as_str() {
                "fixed" => Box::new(FixedValue::scalar(pval)),
                "freestream" => Box::new(Freestream {
                    u_ref: vval,
                    value: [pval, 0.0, 0.0],
                    fixed_on_inflow: false,
                }),
                _ => Box::new(ZeroGradient),
            };
            if energy_on {
                let kth = get_f(case, "physics.conductivity");
                let tval = get_f(case, &format!("{b}.temperature_value"));
                let qval = get_f(case, &format!("{b}.heat_flux_value"));
                let mut nonfinite = false;
                let mut per_face = |c: &Compiled, scale: f64| {
                    let cf = geom.mesh.face_centres();
                    let v: Vec<[f64; 3]> = patch
                        .faces()
                        .map(|f| [scale * c.eval(&[cf[f][0], cf[f][1], cf[f][2], 0.0]), 0.0, 0.0])
                        .collect();
                    nonfinite |= v.iter().any(|x| !x[0].is_finite());
                    Profile::PerFace(v)
                };
                let tbc: Box<dyn Bc> = match get_s(case, &format!("{b}.temperature")).as_str() {
                    "fixed" => {
                        match exprs::compile_bc(
                            case,
                            &format!("{b}.temperature_expression"),
                            Dimension::TEMPERATURE,
                            &sctx,
                        ) {
                            Ok(Some(c)) => Box::new(FixedValue(per_face(&c, 1.0))),
                            Ok(None) => Box::new(FixedValue::scalar(tval)),
                            Err(e) => {
                                errs.extend(e);
                                Box::new(ZeroGradient)
                            }
                        }
                    }
                    "convection" if cht.is_some() => {
                        crate::cht::convective_bc(case, cht.as_ref().expect("checked"), &geom, &patch.name)
                    }
                    "heat_flux" => {
                        if kth <= 0.0 {
                            errs.push(Diagnostic {
                                path: "physics.conductivity".into(),
                                line: None,
                                message: "a heat_flux boundary needs a positive conductivity".into(),
                            });
                        }
                        let inv_k = if kth > 0.0 { 1.0 / kth } else { 0.0 };
                        match exprs::compile_bc(case, &format!("{b}.heat_flux_expression"), Dimension::HEAT_FLUX, &sctx)
                        {
                            // heat flux into the domain q = k dT/dn_out
                            Ok(Some(c)) => match &cht {
                                Some(m) => {
                                    let q: Vec<f64> = match per_face(&c, 1.0) {
                                        Profile::PerFace(v) => v.iter().map(|x| x[0]).collect(),
                                        Profile::Uniform(v) => vec![v[0]; patch.faces().count()],
                                    };
                                    crate::cht::heat_flux_bc(m, &geom, &patch.name, &q)
                                }
                                None => Box::new(FixedGradient(per_face(&c, inv_k))),
                            },
                            Ok(None) => match &cht {
                                Some(m) => {
                                    let q = vec![qval; patch.faces().count()];
                                    crate::cht::heat_flux_bc(m, &geom, &patch.name, &q)
                                }
                                None => Box::new(FixedGradient(Profile::scalar(qval * inv_k))),
                            },
                            Err(e) => {
                                errs.extend(e);
                                Box::new(ZeroGradient)
                            }
                        }
                    }
                    _ => Box::new(ZeroGradient),
                };
                if nonfinite {
                    errs.push(Diagnostic {
                        path: format!("{b}.temperature"),
                        line: None,
                        message: exprs::NONFINITE_BC.into(),
                    });
                }
                let _ = temp.set_bc(&geom, &patch.name, tbc);
            }
            let _ = u.set_bc(&geom, &patch.name, ubc);
            let _ = p.set_bc(&geom, &patch.name, pbc);
        }
        let mut catalogue = if compressible || vdens {
            None
        } else {
            match crate::bc_cat::Catalogue::from_case(case, &geom) {
                Ok(c) => c,
                Err(e) => {
                    errs.extend(e);
                    None
                }
            }
        };
        if let Some(c) = &mut catalogue {
            c.refresh(&geom, &mut u, &mut p, &vec![0.0; geom.mesh.n_faces()]);
        }
        if let Ok(names) = case.tree.names("boundaries") {
            for n in names {
                if geom.mesh.patch(&n).is_none() {
                    let known: Vec<&str> = geom.mesh.patches().iter().map(|p| p.name.as_str()).collect();
                    errs.push(Diagnostic {
                        path: format!("boundaries.{n}"),
                        line: None,
                        message: format!("mesh has no patch '{n}' (patches: {})", known.join(", ")),
                    });
                }
            }
        }
        let mut conditions = Vec::new();
        for n in case.tree.names("convergence").unwrap_or_default() {
            let p = |f: &str| format!("convergence.{n}.{f}");
            let monitor = get_s(case, &p("monitor"));
            let known = [
                "res_u",
                "res_p",
                "continuity",
                "courant",
                "dt",
                "res_T",
                "res_nut",
                "res_rho",
            ]
            .contains(&monitor.as_str())
                || case.tree.names("reports").unwrap_or_default().contains(&monitor)
                || ["scalars", "species"].iter().any(|s| {
                    case.tree
                        .names(s)
                        .unwrap_or_default()
                        .iter()
                        .any(|n| format!("res_{n}") == monitor)
                });
            if !known {
                errs.push(Diagnostic {
                    path: p("monitor"),
                    line: None,
                    message: format!("unknown monitor '{monitor}'"),
                });
            }
            let c = Condition {
                kind: ConditionKind::parse(&get_s(case, &p("type"))).unwrap_or(ConditionKind::WindowRange),
                tolerance: get_f(case, &p("tolerance")),
                window: get_i(case, &p("window")).max(0) as usize,
                relative: get_b(case, &p("relative")),
                monitor,
                name: n.clone(),
            };
            if let Err(e) = c.validate() {
                errs.push(Diagnostic {
                    path: match &e {
                        Error::InvalidValue { path, .. } => path.clone(),
                        _ => p("tolerance"),
                    },
                    line: None,
                    message: e.to_string(),
                });
            }
            conditions.push(c);
        }
        let mut reports = ReportDef::from_case(case);
        let (rctx, report_syms) = exprs::report_context(case);
        let mut report_exprs: Vec<Option<Compiled>> = vec![None; reports.len()];
        for (i, r) in reports.iter_mut().enumerate() {
            if r.kind == ReportKind::Expression {
                let p = format!("reports.{}.expression", r.name);
                match rcfd_core::expr::Expr::parse(&r.expression).and_then(|e| e.compile(&rctx)) {
                    Ok(c) => {
                        r.expr_unit = match c.dimension().to_string().as_str() {
                            "1" => String::new(),
                            u => u.to_string(),
                        };
                        report_exprs[i] = Some(c);
                    }
                    Err(e) => errs.push(Diagnostic {
                        path: p,
                        line: None,
                        message: format!("{e}\n{}", e.render(&r.expression)),
                    }),
                }
            }
        }
        for r in &reports {
            let needs_patch = matches!(
                r.kind,
                ReportKind::Force
                    | ReportKind::Moment
                    | ReportKind::ForceCoefficient
                    | ReportKind::AreaAverage
                    | ReportKind::AreaIntegral
                    | ReportKind::MassFlow
                    | ReportKind::HeatFlow
                    | ReportKind::YPlus
            );
            if needs_patch && geom.mesh.patch(&r.patch).is_none() {
                errs.push(Diagnostic {
                    path: format!("reports.{}.patch", r.name),
                    line: None,
                    message: format!("mesh has no patch '{}'", r.patch),
                });
            }
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        let nu = get_f(case, "physics.nu");
        let alg = get_s(case, "solver.algorithm");
        let conv = solver_conv_scheme(case);
        let coupled_alg = alg == "coupled";
        let steady_alg = matches!(alg.as_str(), "simple" | "simplec");
        let n_cells = geom.n_cells();
        let p_ref_cell = get_i(case, "solver.p_ref_cell").max(0) as usize;
        if !compressible && p_ref_cell >= n_cells {
            return Err(diag(
                "solver.p_ref_cell",
                format!("cell {p_ref_cell} does not exist (the mesh has {n_cells} cells)"),
            ));
        }
        let turb = if turb::enabled(case) {
            let mut speeds: Vec<(String, Vec<f64>)> = Vec::new();
            for n in case.tree.names("boundaries").unwrap_or_default() {
                if turb::role(case, &n) == turb::Role::Inlet {
                    if let Some(patch) = geom.mesh.patch(&n) {
                        let nf = patch.faces().count();
                        let v = match expr_bcs
                            .iter()
                            .find(|e| e.patch == n)
                            .and_then(|e| e.profile(&geom, 0.0, &mut udfs).ok())
                        {
                            // catalogue kinds (ADR 0022): the speed of the refreshed boundary velocity
                            _ if crate::bc_cat::kind(case, &n) != "none" => {
                                let bv = u.boundary_values(&geom);
                                patch.faces().map(|f| vec3::norm(bv[f - geom.n_internal])).collect()
                            }
                            Some(Profile::PerFace(v)) => v.iter().map(|x| vec3::norm(*x)).collect(),
                            Some(Profile::Uniform(v)) => vec![vec3::norm(v); nf],
                            None => {
                                vec![vec3::norm(get_v(case, &format!("boundaries.{n}.velocity_value"), [0.0; 3])); nf]
                            }
                        };
                        speeds.push((n, v));
                    }
                }
            }
            Some(TurbState::new(case, &geom, &speeds)?)
        } else {
            None
        };
        // linear solver controls (0 keeps the algorithm default)
        macro_rules! linear_controls {
            ($s:expr) => {{
                let (mt, mi) = (
                    get_f(case, "solver.momentum_rel_tol"),
                    get_i(case, "solver.momentum_max_iter"),
                );
                let (pt, pi) = (
                    get_f(case, "solver.pressure_rel_tol"),
                    get_i(case, "solver.pressure_max_iter"),
                );
                if mt > 0.0 {
                    $s.momentum_solver.tol_rel = mt;
                }
                if mi > 0 {
                    $s.momentum_solver.max_iter = mi as usize;
                }
                if pt > 0.0 {
                    $s.pressure_solver.tol_rel = pt;
                }
                let pa = get_f(case, "solver.pressure_abs_tol");
                if pa > 0.0 {
                    $s.pressure_solver.tol_abs = pa;
                }
                if pi > 0 {
                    $s.pressure_solver.max_iter = pi as usize;
                }
            }};
        }
        let mut base_body: Option<BodyForce> = None;
        let engine = if compressible {
            Engine::Compressible(Box::new(CompState::new(case, &geom)?))
        } else if vdens {
            Engine::VarDensity(Box::new(VdState::new(case, &geom, u, p, temp.clone())?))
        } else if coupled_alg {
            let mut s = CoupledSettings::new(nu);
            s.convection = conv;
            s.alpha_u = get_f(case, "solver.alpha_u");
            s.p_ref = (p_ref_cell, get_f(case, "solver.p_ref_value"));
            let (mt, mi) = (
                get_f(case, "solver.pressure_rel_tol"),
                get_i(case, "solver.pressure_max_iter"),
            );
            if mt > 0.0 {
                s.linear.tol_rel = mt;
            }
            if mi > 0 {
                s.linear.max_iter = mi as usize;
            }
            // solid zones are excluded from the pressure-velocity system (ADR 0029 D3)
            let mask = cht
                .as_ref()
                .filter(|m| m.has_solids)
                .map(|m| m.label.iter().map(|&l| l > 0).collect::<Vec<bool>>());
            let first_solid = cht.as_ref().and_then(|m| m.names.first().cloned());
            let mut solver = CoupledSolver::with_solids(geom.clone(), u, p, s, mask).map_err(|e| match e {
                rcfd_core::Error::InvalidValue { path, reason } => {
                    let path = match (path.as_str(), &first_solid) {
                        ("solids", Some(n)) => format!("solids.{n}.zone"),
                        _ => path,
                    };
                    diag(&path, reason)
                }
                e => diag("solver.algorithm", e.to_string()),
            })?;
            let bf = get_v(case, "physics.body_force", [0.0; 3]);
            if vec3::norm(bf) > 0.0 {
                let b = BodyForce::uniform(&geom, bf);
                solver.body_force = Some(b.clone());
                base_body = Some(b);
            }
            Engine::Coupled(Box::new(solver))
        } else if steady_alg {
            let coupling = if alg == "simple" {
                Coupling::Simple
            } else {
                Coupling::Simplec
            };
            let mut s = SimpleSettings::new(nu, coupling);
            s.convection = conv;
            s.alpha_u = get_f(case, "solver.alpha_u");
            s.alpha_p = get_f(case, "solver.alpha_p");
            s.non_orth_correctors = get_i(case, "solver.non_orthogonal_correctors") as usize;
            s.p_ref = (p_ref_cell, get_f(case, "solver.p_ref_value"));
            match get_s(case, "solver.h_operator").as_str() {
                "from_solution" => s.h_from_solution = true,
                "previous_iterate" => s.h_from_solution = false,
                _ if turb.is_some() => s.h_from_solution = true,
                _ => {}
            }
            linear_controls!(s);
            let mut solver = SimpleSolver::new(geom.clone(), u, p, s);
            let bf = get_v(case, "physics.body_force", [0.0; 3]);
            if vec3::norm(bf) > 0.0 {
                let b = BodyForce::uniform(&geom, bf);
                solver.body_force = Some(b.clone());
                base_body = Some(b);
            }
            let bulk = get_f(case, "solver.bulk_velocity");
            if bulk != 0.0 {
                let dir = get_v(case, "solver.bulk_direction", [1.0, 0.0, 0.0]);
                if vec3::norm(dir) == 0.0 {
                    return Err(diag(
                        "solver.bulk_direction",
                        "the direction of the bulk velocity must not be zero",
                    ));
                }
                solver.driver = Some(MeanVelocityDriver {
                    dir: vec3::unit(dir),
                    target: bulk,
                    accel: 0.0,
                });
            }
            solver.pseudo = match get_s(case, "solver.pseudo_mode").as_str() {
                "auto" => Some(PseudoMode::Auto),
                "local" => Some(PseudoMode::Local {
                    cfl: get_f(case, "solver.pseudo_cfl"),
                }),
                "physical" => Some(PseudoMode::Physical {
                    dt: get_f(case, "solver.pseudo_dt"),
                }),
                _ => None,
            }
            .map(|mode| PseudoTransient {
                mode,
                factor: get_f(case, "solver.pseudo_factor"),
            });
            let ctl = get_s(case, "solver.pseudo_control");
            if ctl == "ser" || ctl == "ser2" {
                if solver.pseudo.is_none() {
                    return Err(diag(
                        "solver.pseudo_control",
                        "the pseudo-time-step controller needs solver.pseudo_mode = \"auto\", \"local\" or \"physical\"",
                    ));
                }
                let cap = get_f(case, "solver.pseudo_scale_max");
                solver.pseudo_control = Some(if ctl == "ser2" {
                    rcfd_fv::simple::PseudoController::new_symmetric(cap)
                } else {
                    rcfd_fv::simple::PseudoController::new(cap)
                });
            }
            match get_s(case, "solver.residual_norm").as_str() {
                "backward" => solver.residual_norm = rcfd_fv::simple::ResidualNorm::Backward,
                "scaled" => solver.residual_norm = rcfd_fv::simple::ResidualNorm::Scaled,
                _ => {}
            }
            Engine::Steady(Box::new(solver))
        } else {
            let mut s = if alg == "pimple" {
                PimpleSettings::pimple(nu, get_i(case, "solver.n_outer") as usize)
            } else {
                PimpleSettings::piso(nu)
            };
            s.convection = conv;
            s.n_correctors = get_i(case, "solver.n_correctors") as usize;
            s.non_orth_correctors = get_i(case, "solver.non_orthogonal_correctors") as usize;
            s.alpha_u = get_f(case, "solver.alpha_u");
            s.alpha_p = get_f(case, "solver.alpha_p");
            s.p_ref = (p_ref_cell, get_f(case, "solver.p_ref_value"));
            s.ddt_correction = get_b(case, "solver.ddt_correction");
            s.full_viscous_stress = get_s(case, "solver.viscous_stress") == "full";
            linear_controls!(s);
            s.time_scheme = solver_time_scheme(case);
            s.amg_pressure = geom.n_cells() > 2000;
            Engine::Transient(Box::new(PimpleSolver::new(geom.clone(), u, p, s)))
        };
        let steady = steady_alg || coupled_alg || matches!(&engine, Engine::Compressible(c) if c.steady);
        let transient_alg = matches!(alg.as_str(), "piso" | "pimple");
        let energy = if energy_on && !vdens && (steady_alg || transient_alg || coupled_alg) {
            let rho = get_f(case, "physics.rho");
            let (cp, kth) = (get_f(case, "physics.cp"), get_f(case, "physics.conductivity"));
            let mut solver = EnergySolver::new(geom.clone(), temp, kth / (rho * cp));
            solver.convection = conv;
            solver.relax = get_f(case, "energy.relax");
            if transient_alg {
                // linear for the frozen flux, solved tightly every step
                solver.solver.tol_rel = 1e-10;
                solver.solver.tol_abs = 1e-300;
                solver.solver.max_iter = 2000;
            }
            if let Some(m) = &cht {
                m.configure(&geom, &mut solver);
            }
            Some(EnergyState {
                solver,
                beta: get_f(case, "physics.thermal_expansion"),
                gravity: get_v(case, "physics.gravity", [0.0; 3]),
                t_ref: get_f(case, "physics.reference_temperature"),
                buoyancy: get_b(case, "energy.buoyancy"),
                k: kth,
                res_t: 1.0,
            })
        } else {
            None
        };
        let mut engine = engine;
        if let Some(phi) = cht.as_ref().and_then(|m| m.frozen_flux.as_ref()) {
            match &mut engine {
                Engine::Steady(s) => s.phi = phi.clone(),
                Engine::Transient(s) => s.phi = phi.clone(),
                _ => {}
            }
        }
        let dt = get_f(case, "solver.dt");
        let max_cfl = get_f(case, "solver.max_courant");
        for (name, role, flag) in [
            (&udf_momentum, "a momentum source", "momentum_source"),
            (&udf_viscosity, "a viscosity", "viscosity"),
        ] {
            if let Some(n) = name {
                if !steady_alg {
                    return Err(diag(
                        &format!("udf.{n}.{flag}"),
                        format!("a user function can supply {role} only in steady SIMPLE/SIMPLEC runs"),
                    ));
                }
            }
        }
        if let (Some(n), true) = (&udf_viscosity, turb.is_some()) {
            return Err(diag(
                &format!("udf.{n}.viscosity"),
                "a user-function viscosity cannot be combined with a turbulence model",
            ));
        }
        let stats = Statistics::from_case(case, n_cells);
        if stats.is_some() && !matches!(engine, Engine::Transient(_)) {
            return Err(diag(
                "statistics.enabled",
                "statistics are collected by the transient incompressible algorithms (piso, pimple)",
            ));
        }
        let rheology = Rheology::from_case(case);
        if let (Some(_), Some(n)) = (&rheology, &udf_viscosity) {
            return Err(diag(
                &format!("udf.{n}.viscosity"),
                "a user-function viscosity cannot be combined with rheology.model / rheology.shift",
            ));
        }
        if (compressible || vdens) && !udfs.is_empty() {
            return Err(diag(
                "udf",
                "user functions are not supported by the compressible solver",
            ));
        }
        if coupled_alg {
            for (present, path, what) in [
                (turb.is_some(), "turbulence.model", "a turbulence model"),
                (rheology.is_some(), "rheology.model", "non-Newtonian rheology"),
                (
                    udf_momentum.is_some() || udf_viscosity.is_some(),
                    "udf",
                    "user functions",
                ),
                (crate::sources::enabled(case), "sources", "momentum sources"),
                (crate::lumped::enabled(case), "components", "lumped components"),
                (crate::radiation::enabled(case), "radiation.model", "radiation"),
            ] {
                if present {
                    return Err(diag(
                        path,
                        format!("{what} is not supported by solver.algorithm = \"coupled\" (closed-domain laminar flow only)"),
                    ));
                }
            }
        }
        let scalars = if compressible || vdens {
            None
        } else {
            ScalarState::from_case(case, &geom, conv, nu)?
        };
        let chem = if compressible || vdens {
            None
        } else {
            ChemState::from_case(case)?
        };
        let zone_sources = if compressible || vdens {
            None
        } else {
            ZoneSources::from_case(case, &geom)?
        };
        let sources = if compressible || vdens {
            None
        } else {
            Sources::from_case(case, &geom)?
        };
        let components = if compressible || vdens {
            None
        } else {
            Components::from_case(case, &geom)?
        };
        let radiation = match (&energy, compressible || vdens) {
            (Some(en), false) => Radiation::from_case(case, &geom, &en.solver.t.values),
            _ => None,
        };
        let dynmesh = crate::dynmesh::DynMesh::from_case(case, &geom)?;
        if dynmesh.is_some() && !matches!(engine, Engine::Transient(_)) {
            return Err(diag(
                "dynamic_mesh.enabled",
                "needs a transient incompressible run (solver.algorithm = piso or pimple)",
            ));
        }
        let mut sim = Simulation {
            case: case.clone(),
            geom,
            dynmesh,
            engine,
            monitors: MonitorStore::new(if steady { "iteration" } else { "time" }),
            reports,
            expr_bcs,
            marangoni,
            catalogue,
            energy,
            turb,
            base_body,
            udfs,
            udf_momentum,
            udf_viscosity,
            udf_monitors,
            rheology,
            sources,
            components,
            radiation,
            scalars,
            chem,
            zone_sources,
            cht,
            frozen_iter: 0,
            props: Properties::from_case(case),
            t_init: get_f(case, "initial.temperature"),
            stats,
            report_exprs,
            report_syms,
            nu,
            rho: get_f(case, "physics.rho"),
            max_iter: get_i(case, "solver.max_iterations") as usize,
            end_time: get_f(case, "solver.end_time"),
            dt,
            cfl: (!steady && !compressible && max_cfl > 0.0).then_some(CflController {
                max_cfl,
                max_dt: dt,
                min_dt: dt * 1e-3,
                growth: 1.2,
            }),
            tol_u: get_f(case, "solver.tolerance_u"),
            tol_p: get_f(case, "solver.tolerance_p"),
            done: false,
            converged: false,
            last: None,
            conditions,
            combine: if get_s(case, "solver.stop_when") == "all" {
                Combine::All
            } else {
                Combine::Any
            },
            min_iterations: get_i(case, "solver.min_iterations").max(0) as usize,
            stop_reason: None,
            failure: None,
            limit_t,
            limit_s,
            clipped: 0,
        };
        if let Some(sc) = &mut sim.scalars {
            let ctx = StepCtx {
                zone: None,
                temperature: sim.energy.as_ref().map(|e| e.solver.t.values.as_slice()),
                time: 0.0,
            };
            sc.update_algebraic(&ctx);
        }
        sim.apply_initial_method(case)?;
        let mut post_diags = crate::postproc::check_sim(&sim);
        post_diags.extend(crate::decomp::check_mesh(case, &sim.geom.mesh));
        if !post_diags.is_empty() {
            return Err(post_diags);
        }
        Ok(sim)
    }

    /// Localised turbulence initialisation (`initial.patches` with a turbulence variable) and `initial.method`
    /// (potential flow, coarse pre-solve); ADR 0019 D1.
    fn apply_initial_method(&mut self, case: &Case) -> std::result::Result<(), Vec<Diagnostic>> {
        let mut errs = Vec::new();
        for n in case.tree.names("initial.patches").unwrap_or_default() {
            let path = |f: &str| format!("initial.patches.{n}.{f}");
            let var = get_s(case, &path("variable"));
            if !crate::registers::TURBULENCE_VARIABLES.contains(&var.as_str()) {
                continue;
            }
            let vals: Vec<f64> = case
                .tree
                .get(&path("value"))
                .ok()
                .and_then(|v| v.as_array().cloned())
                .map(|a| a.iter().filter_map(|x| x.as_f64()).collect())
                .unwrap_or_default();
            if vals.len() != 1 {
                errs.push(
                    diag(
                        &path("value"),
                        format!("expected 1 number for {var}, found {}", vals.len()),
                    )[0]
                    .clone(),
                );
                continue;
            }
            let Some(t) = self.turb.as_mut() else {
                errs.push(
                    diag(
                        &path("variable"),
                        format!("{var} needs a turbulence model (turbulence.model)"),
                    )[0]
                    .clone(),
                );
                continue;
            };
            let mask = match crate::registers::zone_from_setting(case, &path("register"), &self.geom.mesh) {
                Ok(m) => m,
                Err(e) => {
                    errs.push(e);
                    continue;
                }
            };
            let field = if var == "nutilde" { "nuTilda" } else { var.as_str() };
            match t.model.scalar_mut(field) {
                Some(f) => {
                    for (v, m) in f.values.iter_mut().zip(&mask) {
                        if *m {
                            *v = vals[0];
                        }
                    }
                }
                None => {
                    let have: Vec<&str> = t.model.scalars().iter().map(|(n, _)| *n).collect();
                    errs.push(
                        diag(
                            &path("variable"),
                            format!(
                                "the turbulence model '{}' has no field '{var}' (fields: {})",
                                t.model.key(),
                                have.join(", ")
                            ),
                        )[0]
                        .clone(),
                    );
                }
            }
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        let method = crate::init::method(case);
        if method == "uniform" {
            return Ok(());
        }
        if !matches!(self.engine, Engine::Steady(_)) {
            return Err(diag(
                "initial.method",
                format!("initial.method = \"{method}\" is available with the steady algorithms simple and simplec"),
            ));
        }
        if method == "potential" {
            let Engine::Steady(s) = &mut self.engine else {
                unreachable!()
            };
            let (u, p) = crate::init::potential_fields(case, &self.geom, &s.u)?;
            s.set_fields(u, p).map_err(|e| diag("initial.method", e.to_string()))?;
            return Ok(());
        }
        // coarse pre-solve
        let coarse_case = crate::init::coarse_case(case)?;
        let mut coarse = Simulation::new(&coarse_case)?;
        let its = get_i(case, "initial.coarse_iterations").max(1) as usize;
        coarse.run(its);
        let me = |e: Error| diag("initial.method", e.to_string());
        let ip = Interpolator::new(&coarse.geom.mesh, &self.geom.mesh).map_err(me)?;
        let u = ip
            .vector(&coarse.velocity().values, InterpMode::NonConservative)
            .map_err(me)?;
        let p = ip
            .scalar(&coarse.pressure().values, InterpMode::NonConservative)
            .map_err(me)?;
        let t = match (&coarse.energy, &self.energy) {
            (Some(c), Some(_)) => Some(ip.scalar(&c.solver.t.values, InterpMode::NonConservative).map_err(me)?),
            _ => None,
        };
        if u.iter().any(|v| v.iter().any(|x| !x.is_finite())) || p.iter().any(|x| !x.is_finite()) {
            return Err(diag(
                "initial.coarse_iterations",
                "the coarse pre-solve produced non-finite values; lower the relaxation factors or the iteration count",
            ));
        }
        let Engine::Steady(s) = &mut self.engine else {
            unreachable!()
        };
        s.set_fields(u, p).map_err(me)?;
        if let (Some(en), Some(t)) = (self.energy.as_mut(), t) {
            en.solver.t.values = t;
        }
        Ok(())
    }

    /// True for SIMPLE/SIMPLEC runs.
    pub fn is_steady(&self) -> bool {
        match &self.engine {
            Engine::Steady(_) | Engine::Coupled(_) => true,
            Engine::Transient(_) => false,
            Engine::Compressible(c) => c.steady,
            Engine::VarDensity(v) => v.solver.settings.transient.is_none(),
        }
    }

    /// Run finished?
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Steady residual targets reached?
    pub fn is_converged(&self) -> bool {
        self.converged
    }

    /// The SIMPLE/SIMPLEC solver of a steady run (diagnostics and tests), if that is the engine.
    pub fn simple_solver(&self) -> Option<&SimpleSolver> {
        match &self.engine {
            Engine::Steady(s) => Some(s),
            _ => None,
        }
    }

    /// Mutable access to the SIMPLE/SIMPLEC solver of a steady run (diagnostics and tests).
    pub fn simple_solver_mut(&mut self) -> Option<&mut SimpleSolver> {
        match &mut self.engine {
            Engine::Steady(s) => Some(s),
            _ => None,
        }
    }

    /// Iterations or steps performed.
    pub fn iteration(&self) -> usize {
        self.last.map_or(0, |l| l.index)
    }

    /// Current simulation time (0 for steady runs).
    pub fn time(&self) -> f64 {
        match &self.engine {
            Engine::Steady(_) | Engine::Coupled(_) => 0.0,
            Engine::VarDensity(v) => v.solver.time,
            Engine::Transient(s) => s.time,
            Engine::Compressible(c) => {
                if c.steady {
                    0.0
                } else {
                    c.solver.time
                }
            }
        }
    }

    /// Why the run stopped (`None` while it is still running).
    pub fn stop_reason(&self) -> Option<&str> {
        self.stop_reason.as_deref()
    }

    /// The six-degree-of-freedom body of a `dynamic_mesh` run (`motion = six_dof`), if any.
    pub fn dynamic_body(&self) -> Option<&rcfd_fv::sixdof::Body> {
        self.dynmesh.as_ref().and_then(|d| d.body())
    }

    /// Set when the run stopped because a user function failed (trap, fuel, bad value).
    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    /// Registry key and label of the active turbulence model (`None` for laminar runs).
    pub fn turbulence_model(&self) -> Option<(String, String)> {
        self.turb
            .as_ref()
            .map(|t| (t.model.key().to_string(), turb::model_label(&self.case)))
    }

    /// Coefficients (name, value) of the active turbulence model.
    pub fn turbulence_coefficients(&self) -> Vec<(String, f64)> {
        self.turb.as_ref().map_or_else(Vec::new, |t| {
            t.model
                .coefficients()
                .into_iter()
                .map(|c| (c.name.to_string(), c.value))
                .collect()
        })
    }

    /// Change the turbulence model or its settings in a live simulation (F-044): the turbulence state is rebuilt from
    /// `new_case` while the velocity, pressure, flux, iteration counter and monitors are kept. The new model starts from
    /// the current eddy viscosity. Only RANS-to-RANS changes are possible (the laminar and RANS engines differ), and the
    /// rest of `new_case` must equal the case the simulation was built from. Errors leave the simulation unchanged.
    pub fn reconfigure_turbulence(&mut self, new_case: &Case) -> std::result::Result<(), Vec<Diagnostic>> {
        let Some(old) = self.turb.as_ref() else {
            return Err(diag(
                "turbulence.model",
                "this simulation was built without a RANS turbulence model: rebuild it to switch from laminar",
            ));
        };
        if !turb::enabled(new_case) {
            return Err(diag(
                "turbulence.model",
                "a running RANS simulation cannot be switched to laminar: rebuild it",
            ));
        }
        let errs = turb::check(new_case);
        if !errs.is_empty() {
            return Err(errs);
        }
        let mut st = TurbState::new(new_case, &self.geom, &old.inlet_speeds)?;
        st.model.initialize_from_nut(old.model.nut());
        self.turb = Some(st);
        self.case = new_case.clone();
        self.done = false;
        self.converged = false;
        self.stop_reason = None;
        Ok(())
    }

    /// Evaluate the user-function monitors on the current fields; returns an error text on failure.
    fn udf_monitor_step(&mut self, x: f64) -> Option<String> {
        if self.udf_monitors.is_empty() || matches!(self.engine, Engine::Compressible(_) | Engine::VarDensity(_)) {
            return None;
        }
        let mut udfs = std::mem::take(&mut self.udfs);
        let mut sums = Vec::new();
        let mut error = None;
        {
            let geom = self.geom.clone();
            let (cc, vol) = (geom.mesh.cell_centres(), geom.mesh.cell_volumes());
            let (u, p) = (&self.velocity().values, &self.pressure().values);
            let tc = self.energy.as_ref().map(|e| &e.solver.t.values);
            'outer: for name in &self.udf_monitors {
                let m = udfs.get_mut(name).expect("checked at build time");
                let mut sum = 0.0;
                for c in 0..cc.len() {
                    match m.monitor(cc[c], vol[c], u[c], p[c], tc.map_or(0.0, |t| t[c])) {
                        Ok(v) => sum += v * vol[c],
                        Err(e) => {
                            error = Some(e);
                            break 'outer;
                        }
                    }
                }
                sums.push((format!("udf_{name}"), sum));
            }
        }
        self.udfs = udfs;
        for (n, v) in sums {
            self.monitors.push(&n, "", x, v);
        }
        error
    }

    /// Last step summary.
    pub fn last(&self) -> Option<StepSummary> {
        self.last
    }

    /// Velocity field.
    pub fn velocity(&self) -> &VectorField {
        match &self.engine {
            Engine::Steady(s) => &s.u,
            Engine::Coupled(s) => &s.u,
            Engine::Transient(s) => &s.u,
            Engine::Compressible(c) => &c.u,
            Engine::VarDensity(v) => &v.solver.u,
        }
    }

    /// Kinematic pressure field.
    pub fn pressure(&self) -> &ScalarField {
        match &self.engine {
            Engine::Steady(s) => &s.p,
            Engine::Coupled(s) => &s.p,
            Engine::Transient(s) => &s.p,
            Engine::Compressible(c) => &c.p,
            Engine::VarDensity(v) => &v.solver.p,
        }
    }

    /// Face flux (volumetric for the incompressible algorithms, mass flux for the compressible solver).
    pub fn flux(&self) -> &[f64] {
        match &self.engine {
            Engine::Steady(s) => &s.phi,
            Engine::Coupled(s) => &s.phi,
            Engine::Transient(s) => &s.phi,
            Engine::Compressible(c) => &c.phi,
            Engine::VarDensity(v) => &v.solver.phi,
        }
    }

    /// Advance one iteration (steady) or time step (transient) and record monitors.
    pub fn step(&mut self) -> StepSummary {
        self.update_marangoni();
        let mut reason_override: Option<String> = None;
        let mut udf_error: Option<String> = None;
        let frozen = self.cht.as_ref().is_some_and(|c| c.frozen_flux.is_some());
        let summary = match &mut self.engine {
            Engine::Steady(s) if frozen => {
                // prescribed flow (ADR 0024 D5): only the energy equation is iterated
                self.frozen_iter += 1;
                let iteration = self.frozen_iter;
                if let Some(en) = &mut self.energy {
                    if let Some(zs) = &self.zone_sources {
                        zs.apply_temperature(&mut en.solver, None, 0.0);
                    }
                    en.res_t = en.solver.iterate(&s.phi);
                }
                let converged = self.energy.as_ref().map_or(true, |e| e.res_t < self.tol_u);
                StepSummary {
                    index: iteration,
                    time: 0.0,
                    res_u: 0.0,
                    res_p: 0.0,
                    continuity: 0.0,
                    done: converged || iteration >= self.max_iter,
                    converged,
                }
            }
            Engine::Transient(s) if frozen => {
                let dt = self.dt.min((self.end_time - s.time).max(1e-12));
                if let Some(en) = &mut self.energy {
                    if let Some(zs) = &self.zone_sources {
                        zs.apply_temperature(&mut en.solver, None, s.time + dt);
                    }
                    en.res_t = en.solver.advance(&s.phi, dt, s.settings.time_scheme);
                }
                s.time += dt;
                let step = s.history.len() + 1;
                s.history.push(rcfd_fv::pimple::StepInfo {
                    step,
                    time: s.time,
                    dt,
                    courant: 0.0,
                    continuity: 0.0,
                    res_u: 0.0,
                    res_p: 0.0,
                    p_iters: 0,
                });
                StepSummary {
                    index: step,
                    time: s.time,
                    res_u: 0.0,
                    res_p: 0.0,
                    continuity: 0.0,
                    done: s.time >= self.end_time - 1e-12,
                    converged: false,
                }
            }
            Engine::Steady(s) => {
                if let Some(t) = &mut self.turb {
                    // eddy viscosity from the current flux, then the momentum step with it
                    t.change = t.model.update(&s.u, &s.phi);
                    s.nu_face = Some(t.model.nu_face());
                }
                let temp_cells: Option<&[f64]> = self.energy.as_ref().map(|en| en.solver.t.values.as_slice());
                if let Some(r) = &mut self.rheology {
                    match r.nu_faces(&self.geom, &s.u, temp_cells, self.t_init) {
                        Ok(v) => s.nu_face = Some(v),
                        Err(e) => udf_error = Some(e),
                    }
                }
                let mut udf_force = None;
                if let Some(name) = &self.udf_momentum {
                    let (cc, u) = (self.geom.mesh.cell_centres(), &s.u.values);
                    let u0 = self.udfs.get_mut(name).expect("checked at build time");
                    let cells: std::result::Result<Vec<Vec3>, String> =
                        (0..cc.len()).map(|c| u0.momentum_source(cc[c], 0.0, u[c])).collect();
                    match cells {
                        Ok(v) => udf_force = Some(BodyForce::from_cells(&self.geom, v)),
                        Err(e) => udf_error = Some(e),
                    }
                }
                if let (Some(name), None) = (&self.udf_viscosity, &udf_error) {
                    let g = &self.geom;
                    let (fc, u) = (g.mesh.face_centres(), &s.u.values);
                    let u0 = self.udfs.get_mut(name).expect("checked at build time");
                    let mut nu = Vec::with_capacity(g.mesh.n_faces());
                    for f in 0..g.mesh.n_faces() {
                        let (o, w) = (g.mesh.owner()[f], if f < g.n_internal { g.weights[f] } else { 1.0 });
                        let n = if f < g.n_internal { g.mesh.neighbour()[f] } else { o };
                        let blend = |a: f64, b: f64| w * a + (1.0 - w) * b;
                        let speed = blend(vec3::norm(u[o]), vec3::norm(u[n]));
                        let temperature = temp_cells.map_or(0.0, |t| blend(t[o], t[n]));
                        match u0.kinematic_viscosity(fc[f], 0.0, temperature, speed) {
                            Ok(v) => nu.push(v),
                            Err(e) => {
                                udf_error = Some(e);
                                break;
                            }
                        }
                    }
                    if udf_error.is_none() {
                        s.nu_face = Some(nu);
                    }
                }
                if let (Some(pr), Some(en)) = (&self.props, &self.energy) {
                    if let Some(nu) = pr.nu_faces(&self.geom, &en.solver.t) {
                        s.nu_face = Some(nu);
                    }
                }
                let (src_force, src_sink) = match &self.sources {
                    Some(src) => src.evaluate(&self.geom, &s.u),
                    None => (None, None),
                };
                if let Some(cm) = &mut self.components {
                    s.face_jump = cm.face_jump(&self.geom, &s.phi);
                    for (patch, pk) in cm.network_pressures(&self.geom, &s.phi) {
                        let _ = s.p.set_bc(&self.geom, &patch, Box::new(FixedValue::scalar(pk)));
                    }
                }
                s.sink = src_sink;
                s.conv_flux_offset = self.sources.as_ref().and_then(|x| x.frame_flux.clone());
                s.component_diag_correction =
                    s.conv_flux_offset.is_some() || get_s(&self.case, "solver.component_diagonal") == "corrected";
                s.wall_pressure_trapezoid =
                    src_force.is_some() && self.sources.as_ref().is_some_and(|x| x.wall_pressure_trapezoid);
                if self.energy.is_some() || self.udf_momentum.is_some() || src_force.is_some() {
                    let bous = self.energy.as_ref().and_then(|en| {
                        en.buoyancy
                            .then(|| boussinesq_force(&self.geom, &en.solver.t, en.beta, en.gravity, en.t_ref))
                    });
                    let mut total = self.base_body.clone();
                    for part in [bous, udf_force, src_force].into_iter().flatten() {
                        match &mut total {
                            Some(t) => t.add(&part),
                            None => total = Some(part),
                        }
                    }
                    s.body_force = total;
                }
                if let Some(e) = &udf_error {
                    reason_override = Some(e.clone());
                }
                if let Some(cat) = &mut self.catalogue {
                    cat.refresh(&self.geom, &mut s.u, &mut s.p, &s.phi);
                }
                let i = s.iterate();
                let mut chem_heat: Option<Vec<f64>> = None;
                if let Some(ch) = &self.chem {
                    match crate::chemistry::apply_steady(
                        ch,
                        self.scalars.as_mut(),
                        self.energy.as_ref().map(|e| e.solver.t.values.as_slice()),
                        self.turb.as_ref(),
                    ) {
                        Ok(h) => chem_heat = h,
                        Err(e) => udf_error = Some(e),
                    }
                }
                if let Some(en) = &mut self.energy {
                    let mut base_alpha = None;
                    if let Some(pr) = &self.props {
                        if let Some(a) = pr.alpha_faces(&self.geom, &en.solver.t) {
                            base_alpha = Some(a.clone());
                            en.solver.alpha_face = Some(a);
                        }
                    }
                    if let Some(cm) = &mut self.components {
                        en.solver.source = cm.heat_sources(&self.geom, &s.phi, &en.solver.t);
                    }
                    if let Some(zs) = &self.zone_sources {
                        let base = if self.components.is_some() {
                            en.solver.source.clone()
                        } else {
                            None
                        };
                        zs.apply_temperature(&mut en.solver, base, 0.0);
                    }
                    if let Some(mut h) = chem_heat.take() {
                        // the other sources of this iteration were assigned above; add the heat release
                        if self.components.is_some() || self.zone_sources.is_some() {
                            if let Some(b) = &en.solver.source {
                                for (x, y) in h.iter_mut().zip(b) {
                                    *x += y;
                                }
                            }
                        }
                        en.solver.source = Some(h);
                    }
                    en.res_t = match &mut self.radiation {
                        Some(r) => r.iterate_energy(&mut en.solver, &s.phi, base_alpha),
                        None => en.solver.iterate(&s.phi),
                    };
                }
                if let Some(cm) = &self.components {
                    for (name, v) in cm.values() {
                        self.monitors.push(&name, "", i.iteration as f64, v);
                    }
                }
                let sctx = StepCtx {
                    zone: self.zone_sources.as_ref(),
                    temperature: self.energy.as_ref().map(|e| e.solver.t.values.as_slice()),
                    time: 0.0,
                };
                let res_sc = self
                    .scalars
                    .as_mut()
                    .map_or(0.0, |sc| sc.iterate(&s.phi, s.nu_face.as_deref(), &sctx));
                let converged = i.res_u < self.tol_u
                    && i.res_p < self.tol_p
                    && res_sc < self.tol_u
                    && self.energy.as_ref().map_or(true, |e| e.res_t < self.tol_u)
                    && self.turb.as_ref().map_or(true, |t| t.change < self.tol_u);
                StepSummary {
                    index: i.iteration,
                    time: 0.0,
                    res_u: i.res_u,
                    res_p: i.res_p,
                    continuity: i.continuity,
                    done: converged || i.iteration >= self.max_iter,
                    converged,
                }
            }
            Engine::Coupled(s) => {
                if self.energy.is_some() {
                    // uniform body force plus Boussinesq buoyancy of the current temperature (ADR 0029 D2, D5)
                    let bous = self.energy.as_ref().and_then(|en| {
                        en.buoyancy
                            .then(|| boussinesq_force(&self.geom, &en.solver.t, en.beta, en.gravity, en.t_ref))
                    });
                    let mut total = self.base_body.clone();
                    if let Some(part) = bous {
                        match &mut total {
                            Some(t) => t.add(&part),
                            None => total = Some(part),
                        }
                    }
                    s.body_force = total;
                }
                let i = s.iterate();
                if let Some(ch) = &self.chem {
                    if let Err(e) = crate::chemistry::apply_steady(ch, self.scalars.as_mut(), None, self.turb.as_ref())
                    {
                        udf_error = Some(e);
                    }
                }
                if let Some(en) = &mut self.energy {
                    en.res_t = en.solver.iterate(&s.phi);
                }
                let res_sc = self
                    .scalars
                    .as_mut()
                    .map_or(0.0, |sc| sc.iterate(&s.phi, None, &StepCtx::default()));
                let converged = i.res_u < self.tol_u
                    && i.res_p < self.tol_p
                    && res_sc < self.tol_u
                    && self.energy.as_ref().map_or(true, |e| e.res_t < self.tol_u);
                StepSummary {
                    index: i.iteration,
                    time: 0.0,
                    res_u: i.res_u,
                    res_p: i.res_p,
                    continuity: i.continuity,
                    done: converged || i.iteration >= self.max_iter,
                    converged,
                }
            }
            Engine::Compressible(c) => {
                let t_end = (!c.steady).then_some(self.end_time);
                let a = c.advance(t_end);
                c.refresh();
                let index = c.solver.steps;
                let (done, converged) = if let Some(e) = &a.error {
                    reason_override = Some(format!("compressible solver failed at step {index}: {e}"));
                    (true, false)
                } else if c.steady {
                    let conv = a.res_rho < c.tol;
                    if conv {
                        reason_override = Some(format!(
                            "steady state: largest relative density change per step {:.3e} < {:.3e}",
                            a.res_rho, c.tol
                        ));
                    }
                    (conv || index >= self.max_iter, conv)
                } else {
                    (c.solver.time >= self.end_time - 1e-12 * self.end_time.max(1.0), false)
                };
                self.monitors.push("dt", "s", index as f64, a.dt);
                self.monitors
                    .push("courant", "", index as f64, get_f(&self.case, "compressible.cfl"));
                self.monitors.push("res_rho", "", index as f64, a.res_rho);
                StepSummary {
                    index,
                    time: if c.steady { 0.0 } else { c.solver.time },
                    res_u: a.res_rho,
                    res_p: a.res_p,
                    continuity: a.mass_drift,
                    done,
                    converged,
                }
            }
            Engine::VarDensity(v) if v.solver.settings.transient.is_some() => {
                // transient variable density (ADR 0009): one time step of SIMPLE-inner iterations
                let dt = self.dt.min((self.end_time - v.solver.time).max(1e-12));
                match v.solver.advance(dt) {
                    Ok(si) => {
                        v.steps += 1;
                        v.last = Some(si.last);
                        v.max_du = si.last.max_du;
                        let l = si.last;
                        let finite =
                            si.change.is_finite() && l.res_u.is_finite() && l.res_p.is_finite() && l.res_h.is_finite();
                        if !finite {
                            reason_override = Some(format!(
                                "variable-density solver diverged at time {} (non-finite inner iteration); lower solver.dt or the relaxation factors",
                                si.time
                            ));
                        }
                        self.monitors.push("res_T", "", si.time, l.res_h);
                        self.monitors.push("inner_iterations", "", si.time, si.inner as f64);
                        self.monitors.push("dt", "s", si.time, dt);
                        StepSummary {
                            index: v.steps,
                            time: si.time,
                            res_u: l.res_u,
                            res_p: l.res_p,
                            continuity: l.continuity,
                            done: !finite || si.time >= self.end_time - 1e-12,
                            converged: false,
                        }
                    }
                    Err(e) => {
                        reason_override = Some(e.to_string());
                        StepSummary {
                            index: v.steps,
                            time: v.solver.time,
                            res_u: f64::NAN,
                            res_p: f64::NAN,
                            continuity: f64::NAN,
                            done: true,
                            converged: false,
                        }
                    }
                }
            }
            Engine::VarDensity(v) => {
                let i = v.solver.iterate();
                v.max_du = i.max_du;
                v.last = Some(i);
                let finite = i.max_du.is_finite() && i.res_u.is_finite() && i.res_p.is_finite() && i.res_h.is_finite();
                if !finite {
                    reason_override = Some(format!(
                        "variable-density solver diverged at iteration {} (non-finite residuals); lower solver.alpha_u / alpha_p / energy.relax",
                        i.iteration
                    ));
                }
                let converged = finite && i.res_u < self.tol_u && i.res_p < self.tol_p && i.res_h < self.tol_u;
                self.monitors.push("res_T", "", i.iteration as f64, i.res_h);
                StepSummary {
                    index: i.iteration,
                    time: 0.0,
                    res_u: i.res_u,
                    res_p: i.res_p,
                    continuity: i.continuity,
                    done: !finite || converged || i.iteration >= self.max_iter,
                    converged,
                }
            }
            Engine::Transient(s) => {
                let dt_prev = s.history.last().map_or(self.dt, |h| h.dt);
                let mut dt = match &self.cfl {
                    Some(c) if !s.history.is_empty() => c.next_dt(&s.geom, &s.phi, dt_prev),
                    _ => self.dt,
                };
                dt = dt.min((self.end_time - s.time).max(1e-12));
                for eb in &self.expr_bcs {
                    match eb.profile(&self.geom, s.time + dt, &mut self.udfs) {
                        Ok(prof) => {
                            let _ = s.u.set_bc(&self.geom, &eb.patch, Box::new(FixedValue(prof)));
                        }
                        Err(e) => {
                            udf_error = Some(e);
                            break;
                        }
                    }
                }
                if let Some(e) = &udf_error {
                    reason_override = Some(e.clone());
                }
                if let Some(t) = &mut self.turb {
                    // algebraic models (subgrid scale, mixing length; ADR 0026): evaluated explicitly from the
                    // velocity and flux of the old time level, no relaxation
                    t.change = t.model.update(&s.u, &s.phi);
                    s.nu_face = Some(t.model.nu_face());
                }
                if let Some(r) = &mut self.rheology {
                    match r.nu_faces(&self.geom, &s.u, None, self.t_init) {
                        Ok(v) => s.nu_face = Some(v),
                        Err(e) => udf_error = Some(e),
                    }
                }
                if let Some(en) = &mut self.energy {
                    // temperature first, on the flux of the previous step; the flow then sees the new buoyancy
                    if let Some(zs) = &self.zone_sources {
                        zs.apply_temperature(&mut en.solver, None, s.time + dt);
                    }
                    en.res_t = en.solver.advance(&s.phi, dt, s.settings.time_scheme);
                    let bous = en
                        .buoyancy
                        .then(|| boussinesq_force(&self.geom, &en.solver.t, en.beta, en.gravity, en.t_ref));
                    let mut total = self.base_body.clone();
                    if let Some(b) = bous {
                        match &mut total {
                            Some(t) => t.add(&b),
                            None => total = Some(b),
                        }
                    }
                    s.body_force = total;
                }
                if let Some(cat) = &mut self.catalogue {
                    cat.refresh(&self.geom, &mut s.u, &mut s.p, &s.phi);
                }
                let i = match &mut self.dynmesh {
                    Some(dm) => match dm.step(s, dt) {
                        Ok(i) => {
                            self.geom = s.geom.clone();
                            i
                        }
                        Err(e) => {
                            udf_error = Some(crate::dynmesh::failure_message(s.history.len() + 1, s.time + dt, &e));
                            rcfd_fv::pimple::StepInfo {
                                step: s.history.len() + 1,
                                time: s.time,
                                dt,
                                courant: 0.0,
                                continuity: 0.0,
                                res_u: 0.0,
                                res_p: 0.0,
                                p_iters: 0,
                            }
                        }
                    },
                    None => s.step(dt),
                };
                if let Some(sc) = &mut self.scalars {
                    let sctx = StepCtx {
                        zone: self.zone_sources.as_ref(),
                        temperature: self.energy.as_ref().map(|e| e.solver.t.values.as_slice()),
                        time: s.time,
                    };
                    sc.advance(&s.phi, dt, s.settings.time_scheme, s.nu_face.as_deref(), &sctx);
                }
                if let Some(ch) = &self.chem {
                    if let Err(e) = crate::chemistry::apply_split(
                        ch,
                        self.scalars.as_mut(),
                        self.energy.as_mut().map(|e| &mut e.solver),
                        self.turb.as_ref(),
                        dt,
                    ) {
                        udf_error = Some(e);
                    }
                }
                self.monitors.push("courant", "", i.time, i.courant);
                self.monitors.push("dt", "s", i.time, i.dt);
                StepSummary {
                    index: i.step,
                    time: i.time,
                    res_u: i.res_u,
                    res_p: i.res_p,
                    continuity: i.continuity,
                    done: i.time >= self.end_time - 1e-12,
                    converged: false,
                }
            }
        };
        let x = if self.is_steady() {
            summary.index as f64
        } else {
            summary.time
        };
        self.monitors.push("res_u", "", x, summary.res_u);
        self.monitors.push("res_p", "", x, summary.res_p);
        self.monitors.push("continuity", "", x, summary.continuity);
        if let Some(en) = &self.energy {
            self.monitors.push("res_T", "", x, en.res_t);
        }
        if let Some(t) = &self.turb {
            self.monitors.push("res_nut", "", x, t.change);
        }
        if let Some(sc) = &self.scalars {
            for (n, v) in sc.residuals() {
                self.monitors.push(&format!("res_{n}"), "", x, v);
            }
        }
        for (r, v) in self.reports.clone().iter().zip(self.eval_all_reports()) {
            if let Ok(v) = v {
                self.monitors.push(&r.name, &r.unit(), x, v);
            }
        }
        if let Some(names) = self.stats.as_ref().map(|s| s.names().to_vec()) {
            if matches!(self.engine, Engine::Transient(_)) {
                let vals: Vec<Vec<f64>> = names.iter().filter_map(|n| self.field(n).ok()).collect();
                if let Some(st) = self.stats.as_mut() {
                    st.accumulate(summary.time, &vals);
                }
            }
        }
        self.apply_solution_limits();
        let mut summary = summary;
        if udf_error.is_none() {
            udf_error = self.udf_monitor_step(x);
        }
        if let Some(e) = udf_error {
            summary.done = true;
            summary.converged = false;
            self.stop_reason = Some(e.clone());
            self.failure = Some(e);
        } else if let Some(r) = reason_override {
            self.stop_reason = Some(r);
        } else if summary.converged {
            self.stop_reason = Some(format!(
                "residual targets met: res_u {:.3e} < {:.3e}, res_p {:.3e} < {:.3e}",
                summary.res_u, self.tol_u, summary.res_p, self.tol_p
            ));
        } else if summary.done {
            self.stop_reason = Some(if self.is_steady() {
                format!(
                    "iteration limit {} reached without meeting the residual targets",
                    self.max_iter
                )
            } else {
                format!("end time {} reached", self.end_time)
            });
        } else if summary.index >= self.min_iterations {
            let mon = &self.monitors;
            if let Some(reason) =
                convergence::evaluate(&self.conditions, self.combine, |n| mon.get(n).map(|s| s.y.as_slice()))
            {
                summary.done = true;
                summary.converged = !reason.starts_with("interrupt");
                self.stop_reason = Some(reason);
            }
        }
        self.done = summary.done;
        self.converged = summary.converged;
        self.last = Some(summary);
        summary
    }

    /// Refresh the thermocapillary stress of every `marangoni` patch from the current temperature field (the
    /// tangential part of the boundary-cell least-squares gradient times d sigma/d T over the viscosity).
    fn update_marangoni(&mut self) {
        if self.marangoni.is_empty() {
            return;
        }
        let Some(en) = &self.energy else { return };
        let t = &en.solver.t;
        let grad = rcfd_fv::gradient::least_squares(&self.geom, &t.values, &t.boundary_values(&self.geom));
        let mu = get_f(&self.case, "physics.rho") * get_f(&self.case, "physics.nu");
        let u = match &mut self.engine {
            Engine::Steady(s) => &mut s.u,
            Engine::Transient(s) => &mut s.u,
            _ => return,
        };
        for (patch, coeff) in &self.marangoni {
            let stress = wall_motion::marangoni_stress(&self.geom, patch, &grad, *coeff, mu);
            let _ = u.set_bc(&self.geom, patch, wall_motion::marangoni_bc(stress));
        }
    }

    /// Clip temperature and transported scalars to the configured bands (off by default). Clipping after the solve is
    /// a robustness measure that breaks conservation locally; the count is reported in the status.
    fn apply_solution_limits(&mut self) {
        let (on_t, lo_t, hi_t) = self.limit_t;
        if on_t {
            if let Some(en) = &mut self.energy {
                for v in en.solver.t.values.iter_mut() {
                    if *v < lo_t || *v > hi_t {
                        *v = v.clamp(lo_t, hi_t);
                        self.clipped += 1;
                    }
                }
            }
        }
        let (on_s, lo_s, hi_s) = self.limit_s;
        if on_s {
            if let Some(sc) = &mut self.scalars {
                self.clipped += sc.clip(lo_s, hi_s);
            }
        }
    }

    /// Cells clipped by the solution limits so far.
    pub fn clipped_cells(&self) -> usize {
        self.clipped
    }

    /// Step until done or `max_steps` have been taken; returns steps taken.
    pub fn run(&mut self, max_steps: usize) -> usize {
        let mut n = 0;
        while !self.done && n < max_steps {
            self.step();
            n += 1;
        }
        n
    }

    /// Cell values of a named field.
    pub fn field(&self, name: &str) -> Result<Vec<f64>> {
        let u = &self.velocity().values;
        if let Engine::Compressible(c) = &self.engine {
            if let Some(v) = c.field(name) {
                return Ok(v);
            }
        }
        if let Engine::VarDensity(v) = &self.engine {
            if name == "T" {
                return Ok(v.solver.t.values.clone());
            }
            if let Some(f) = v.field(name) {
                return Ok(f);
            }
        }
        if let Some(t) = &self.turb {
            match name {
                "k" | "epsilon" | "omega" | "nuTilda" if t.scalar(name).is_some() => {
                    return Ok(t.scalar(name).map(|f| f.values.clone()).unwrap_or_default())
                }
                "nut" => return Ok(t.model.nut().to_vec()),
                "wall_distance" => return Ok(t.model.wall_distance().to_vec()),
                "y_plus" => return Ok(t.y_plus_cells()),
                _ => {}
            }
        } else if matches!(name, "k" | "epsilon" | "omega" | "nuTilda" | "nut" | "y_plus") {
            return Err(Error::InvalidValue {
                path: name.into(),
                reason: format!(
                    "field '{name}' needs a turbulence model (turbulence.model = \"k_epsilon\", \"k_omega_sst\", \"spalart_allmaras\", \"smagorinsky\", \"wale\", \"mixing_length\" or \"plugin\")"
                ),
            });
        }
        if let Some(v) = self.stats.as_ref().and_then(|s| s.get(name)) {
            return Ok(v);
        }
        if let Some(v) = self.scalars.as_ref().and_then(|s| s.field(name)) {
            return Ok(v);
        }
        Ok(match name {
            "ux" => u.iter().map(|v| v[0]).collect(),
            "uy" => u.iter().map(|v| v[1]).collect(),
            "uz" => u.iter().map(|v| v[2]).collect(),
            "speed" => u.iter().map(|v| vec3::norm(*v)).collect(),
            "p" => self.pressure().values.clone(),
            "T" => match &self.energy {
                Some(en) => en.solver.t.values.clone(),
                None => {
                    return Err(Error::InvalidValue {
                        path: "T".into(),
                        reason: "the energy equation is not enabled (energy.enabled)".into(),
                    })
                }
            },
            "G" => match self.radiation.as_ref().and_then(|r| r.incident()) {
                Some(g) => g.to_vec(),
                None => {
                    return Err(Error::InvalidValue {
                        path: "G".into(),
                        reason: "the incident radiation needs radiation.model = \"p1\"".into(),
                    })
                }
            },
            "vorticity_z" => {
                let g = &self.geom;
                let ub = self.velocity().boundary_values(g);
                let comp = |k: usize| {
                    let cells: Vec<f64> = u.iter().map(|v| v[k]).collect();
                    let bv: Vec<f64> = ub.iter().map(|v| v[k]).collect();
                    green_gauss(g, &cells, &bv, 0)
                };
                let (gx, gy) = (comp(0), comp(1));
                (0..u.len()).map(|c| gy[c][0] - gx[c][1]).collect()
            }
            other => {
                return Err(Error::InvalidValue {
                    path: other.into(),
                    reason: format!("field '{other}' is not available in this simulation"),
                })
            }
        })
    }

    /// Boundary-face values (indexed `f - n_internal`) of a named field.
    pub fn boundary_field(&self, name: &str) -> Result<Vec<f64>> {
        let g = &self.geom;
        if let Some(t) = &self.turb {
            match name {
                "y_plus" => return Ok(t.y_plus_boundary()),
                "k" | "epsilon" | "omega" | "nuTilda" => {
                    if let Some(f) = t.scalar(name) {
                        return Ok(f.boundary_values(g));
                    }
                }
                _ => {}
            }
        }
        if let Engine::VarDensity(v) = &self.engine {
            if name == "T" {
                return Ok(v.solver.t.boundary_values(g));
            }
        }
        if matches!(self.engine, Engine::Compressible(_) | Engine::VarDensity(_)) {
            // first-order face values: the adjacent cell state
            let cells = self.field(name)?;
            let own = g.mesh.owner();
            return Ok((g.n_internal..g.mesh.n_faces()).map(|f| cells[own[f]]).collect());
        }
        let ub = self.velocity().boundary_values(g);
        Ok(match name {
            "ux" => ub.iter().map(|v| v[0]).collect(),
            "uy" => ub.iter().map(|v| v[1]).collect(),
            "uz" => ub.iter().map(|v| v[2]).collect(),
            "speed" => ub.iter().map(|v| vec3::norm(*v)).collect(),
            "p" => self.pressure_face_values(),
            "T" if self.energy.is_some() => self.energy.as_ref().map_or(vec![], |e| e.solver.t.boundary_values(g)),
            other => {
                return Err(Error::InvalidValue {
                    path: other.into(),
                    reason: format!("field '{other}' has no boundary values here"),
                })
            }
        })
    }

    /// Kinematic pressure on boundary faces (with the velocity-boundary logic applied).
    pub fn pressure_face_values(&self) -> Vec<f64> {
        let pbc = pressure_bc(&self.geom, self.velocity(), self.pressure());
        boundary_values(&self.geom, &pbc.coeffs, &self.pressure().values)
    }

    /// Minimum, maximum, mean (volume-weighted) and RMS of a field.
    pub fn field_stats(&self, name: &str) -> Result<Value> {
        let f = self.field(name)?;
        let mean = ig::volume_average(&self.geom, &f);
        let rms = (ig::volume_average(&self.geom, &f.iter().map(|x| x * x).collect::<Vec<_>>())).sqrt();
        Ok(json!({
            "field": name,
            "min": f.iter().cloned().fold(f64::INFINITY, f64::min),
            "max": f.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            "mean": mean,
            "rms": rms,
            "cells": f.len(),
        }))
    }

    /// Net radiative heat flow (W) out of the domain through a patch (`radiation.model` of `p1` or `rosseland`); the
    /// conductive part is the `heat_flow` report.
    pub fn radiative_heat_flow(&self, patch: &str) -> Result<f64> {
        let (Some(r), Some(en)) = (&self.radiation, &self.energy) else {
            return Err(Error::Other("no radiation model is active (radiation.model)".into()));
        };
        if self.geom.mesh.patch(patch).is_none() {
            return Err(Error::PathNotFound(format!("patch '{patch}'")));
        }
        Ok(r.wall_flow_out(patch, &en.solver))
    }

    /// Net radiative absorption `integral a (G - 4 sigma n^2 T^4) dV` (W) of the P1 model.
    pub fn radiative_absorption(&self) -> Result<f64> {
        match (&self.radiation, &self.energy) {
            (Some(r), Some(en)) => r
                .absorbed_power(&en.solver.t.values)
                .ok_or_else(|| Error::Other("the absorbed power needs radiation.model = \"p1\"".into())),
            _ => Err(Error::Other("no radiation model is active (radiation.model)".into())),
        }
    }

    /// Evaluate one report definition on the current solution.
    pub fn eval_report(&self, r: &ReportDef) -> Result<f64> {
        let g = &self.geom;
        if matches!(self.engine, Engine::Compressible(_)) {
            match r.kind {
                ReportKind::Force | ReportKind::Moment | ReportKind::ForceCoefficient | ReportKind::HeatFlow => {
                    return Err(Error::Other(
                        "force, moment and heat-flow reports are not available with the compressible solver".into(),
                    ))
                }
                ReportKind::MassFlow => return ig::mass_flux(g, &r.patch, self.flux(), 1.0),
                _ => {}
            }
        }
        if let Engine::VarDensity(v) = &self.engine {
            match r.kind {
                ReportKind::Force | ReportKind::Moment | ReportKind::ForceCoefficient | ReportKind::YPlus => {
                    return Err(Error::Other(
                        "force, moment and y_plus reports are not available with the variable-density solver".into(),
                    ))
                }
                ReportKind::MassFlow => return v.solver.mass_flow(&r.patch),
                ReportKind::HeatFlow => return v.solver.heat_flow_out(&r.patch),
                _ => {}
            }
        }
        let wall = |origin: Vec3| {
            ig::wall_force_and_moment(
                g,
                &r.patch,
                self.velocity(),
                &self.pressure_face_values(),
                self.nu,
                self.rho,
                origin,
            )
        };
        let pick = |f: ig::PatchForce| match r.component.as_str() {
            "pressure" => f.pressure,
            "viscous" => f.viscous,
            _ => f.total(),
        };
        Ok(match r.kind {
            ReportKind::Force => vec3::dot(pick(wall(r.origin)?.0), vec3::unit(r.direction)),
            ReportKind::ForceCoefficient => {
                ig::force_coefficient(pick(wall(r.origin)?.0), r.direction, self.rho, r.u_ref, r.a_ref)
            }
            ReportKind::Moment => vec3::dot(wall(r.origin)?.1, vec3::unit(r.direction)),
            ReportKind::VolumeAverage => ig::volume_average(g, &self.field(&r.field)?),
            ReportKind::VolumeIntegral => ig::volume_integral(g, &self.field(&r.field)?),
            ReportKind::AreaAverage => ig::patch_average(g, &r.patch, &self.boundary_field(&r.field)?)?,
            ReportKind::AreaIntegral => ig::patch_integral(g, &r.patch, &self.boundary_field(&r.field)?)?,
            ReportKind::MassFlow => ig::mass_flux(g, &r.patch, self.flux(), self.rho)?,
            ReportKind::HeatFlow => {
                let en = self.energy.as_ref().ok_or_else(|| {
                    Error::Other("heat_flow needs the energy equation (set energy.enabled = true)".into())
                })?;
                if g.mesh.patch(&r.patch).is_none() {
                    return Err(Error::PathNotFound(format!("patch '{}'", r.patch)));
                }
                if let Some(m) = self.cht.as_ref().filter(|m| m.has_solids) {
                    if r.conductivity > 0.0 {
                        return Err(Error::Other(
                            "heat_flow: a report-level conductivity is not used with solid zones (each face takes the conductivity of its cell)".into(),
                        ));
                    }
                    return m
                        .heat_flow_out(g, &en.solver, &r.patch)
                        .ok_or_else(|| Error::PathNotFound(format!("patch '{}'", r.patch)));
                }
                // conductive heat flow out of the domain; a report-level conductivity overrides physics.conductivity
                match &self.props {
                    Some(pr) if pr.has_conductivity() => en.solver.heat_flow_out_var(&r.patch, &|t| pr.conductivity(t)),
                    _ => en
                        .solver
                        .heat_flow_out(&r.patch, if r.conductivity > 0.0 { r.conductivity } else { en.k }),
                }
            }
            ReportKind::ScalarFlux => {
                let sc = self.scalars.as_ref().ok_or_else(|| {
                    Error::Other(
                        "scalar_flux needs a passive scalar or species (scalars.<name>, species.<name>)".into(),
                    )
                })?;
                sc.flux(&r.scalar, &r.patch, self.flux())?
            }
            ReportKind::ScalarAverage => ig::volume_average(g, &self.field(&r.scalar)?),
            ReportKind::Probe => ig::Probe::new(g, r.point).sample(g, &self.field(&r.field)?, None),
            ReportKind::Minimum => self.field(&r.field)?.into_iter().fold(f64::INFINITY, f64::min),
            ReportKind::Maximum => self.field(&r.field)?.into_iter().fold(f64::NEG_INFINITY, f64::max),
            ReportKind::YPlus => {
                let t = self.turb.as_ref().ok_or_else(|| {
                    Error::Other("y_plus needs a RANS turbulence model (turbulence.model other than laminar)".into())
                })?;
                let patch = g
                    .mesh
                    .patch(&r.patch)
                    .ok_or_else(|| Error::PathNotFound(format!("patch '{}'", r.patch)))?;
                let all = t.y_plus_boundary();
                let vals: Vec<f64> = patch.faces().map(|f| all[f - g.n_internal]).collect();
                if vals.is_empty() {
                    return Err(Error::Other(format!("patch '{}' has no faces", r.patch)));
                }
                match r.statistic.as_str() {
                    "min" => vals.iter().cloned().fold(f64::INFINITY, f64::min),
                    "max" => vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                    _ => ig::patch_average(g, &r.patch, &all)?,
                }
            }
            ReportKind::Expression => {
                let i = self
                    .reports
                    .iter()
                    .position(|d| d.name == r.name)
                    .ok_or_else(|| Error::PathNotFound(format!("reports.{}", r.name)))?;
                self.eval_all_reports().swap_remove(i)?
            }
        })
    }

    /// Evaluate every report: first the plain ones, then the expression reports
    /// that combine them. An expression whose input report cannot be evaluated fails.
    pub fn eval_all_reports(&self) -> Vec<Result<f64>> {
        let mut out: Vec<Result<f64>> = self
            .reports
            .iter()
            .map(|r| {
                if r.kind == ReportKind::Expression {
                    Ok(f64::NAN)
                } else {
                    self.eval_report(r)
                }
            })
            .collect();
        // slot order of the report context equals `report_syms` order
        let slots: Vec<f64> = self
            .report_syms
            .iter()
            .map(|n| {
                self.reports
                    .iter()
                    .position(|r| &r.name == n)
                    .and_then(|i| out[i].as_ref().ok().copied())
                    .unwrap_or(f64::NAN)
            })
            .collect();
        for (i, c) in self.report_exprs.iter().enumerate() {
            if let Some(c) = c {
                let v = c.eval(&slots);
                out[i] = if v.is_nan() {
                    Err(Error::Other(
                        "expression is not finite (an input report may be unavailable)".into(),
                    ))
                } else {
                    Ok(v)
                };
            }
        }
        out
    }

    /// Evaluate every report of the case.
    pub fn report_values(&self) -> Vec<(String, std::result::Result<ReportValue, String>)> {
        self.reports
            .iter()
            .zip(self.eval_all_reports())
            .map(|(r, v)| {
                (
                    r.name.clone(),
                    v.map(|v| ReportValue {
                        name: r.name.clone(),
                        value: v,
                        unit: r.unit(),
                    })
                    .map_err(|e| e.to_string()),
                )
            })
            .collect()
    }

    /// Self-contained HTML report (summary, residual plots, report table, monitor plots).
    pub fn html_report(&self) -> String {
        let summary = vec![
            ("Case".to_string(), get_s(&self.case, "title")),
            ("Algorithm".to_string(), get_s(&self.case, "solver.algorithm")),
            (
                "Turbulence".to_string(),
                match &self.turb {
                    Some(_) => turb::model_label(&self.case),
                    None => "laminar".into(),
                },
            ),
            ("Cells".to_string(), self.geom.n_cells().to_string()),
            ("Iterations / steps".to_string(), self.iteration().to_string()),
            ("Time".to_string(), format!("{}", self.time())),
            ("Converged".to_string(), self.converged.to_string()),
            (
                "Stopped because".to_string(),
                self.stop_reason.clone().unwrap_or_else(|| "still running".into()),
            ),
        ];
        let values: Vec<ReportValue> = self.report_values().into_iter().filter_map(|(_, v)| v.ok()).collect();
        rcfd_post::monitors::html_report(&get_s(&self.case, "title"), &summary, &self.monitors, &values)
    }

    /// Short JSON status.
    pub fn status_json(&self) -> Value {
        let l = self.last;
        json!({
            "algorithm": get_s(&self.case, "solver.algorithm"),
            "steady": self.is_steady(),
            "cells": self.geom.n_cells(),
            "iteration": self.iteration(),
            "time": self.time(),
            "done": self.done,
            "converged": self.converged,
            "stop_reason": self.stop_reason,
            "clipped_cells": self.clipped,
            "res_u": l.map(|s| s.res_u),
            "res_p": l.map(|s| s.res_p),
            "continuity": l.map(|s| s.continuity),
            "turbulence": self.turb.as_ref().map(|t| {
                let y = t.y_plus_faces();
                json!({
                    "model": get_s(&self.case, "turbulence.model"),
                    "key": turb::model_key(&self.case),
                    "variant": match get_s(&self.case, "turbulence.model").as_str() {
                        "k_omega_sst" => "menter_2003".to_string(),
                        "spalart_allmaras" => "sa_noft2".to_string(),
                        m @ ("smagorinsky" | "wale" | "mixing_length") => m.to_string(),
                        "plugin" => get_s(&self.case, "turbulence.plugin"),
                        _ => get_s(&self.case, "turbulence.variant"),
                    },
                    "wall_patches": t.walls,
                    "y_plus_min": y.iter().cloned().fold(f64::INFINITY, f64::min),
                    "y_plus_max": y.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                    "nut_change": t.change,
                })
            }),
        })
    }

    /// Whether [`save_checkpoint`](Self::save_checkpoint) works for this run (steady SIMPLE/SIMPLEC,
    /// laminar, with or without the energy equation).
    pub fn supports_checkpoint(&self) -> bool {
        matches!(self.engine, Engine::Steady(_)) && self.turb.is_none()
    }

    /// Write a restart file (F-121 `continue-from`): velocity, pressure, face flux, temperature
    /// and the iteration counter in the checksummed container, tied to the mesh by its hash.
    pub fn save_checkpoint(&self, path: &Path) -> Result<()> {
        let Engine::Steady(s) = &self.engine else {
            return Err(Error::Other(
                "checkpoints are available for steady SIMPLE/SIMPLEC runs only".into(),
            ));
        };
        if self.turb.is_some() {
            return Err(Error::Other(
                "the k-epsilon / k-omega SST state is not part of a checkpoint yet".into(),
            ));
        }
        let st = s.snapshot();
        let mut c = rcfd_io::container::Container::new();
        c.put(
            "meta",
            json!({
                "kind": "rcfd-sim-checkpoint", "version": 1,
                "iteration": st.iteration,
                "mesh_hash": format!("{:016x}", self.geom.mesh.content_hash()),
                "n_cells": st.p.len(), "n_faces": st.phi.len(),
                "energy": self.energy.is_some(),
            })
            .to_string()
            .into_bytes(),
        );
        c.put_f64s("u", &st.u.iter().flatten().copied().collect::<Vec<_>>());
        c.put_f64s("p", &st.p);
        c.put_f64s("phi", &st.phi);
        if let Some(en) = &self.energy {
            c.put_f64s("T", &en.solver.t.values);
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        c.write_file(path)
    }

    /// Restore a restart file written by [`save_checkpoint`](Self::save_checkpoint) into this
    /// freshly built simulation (same mesh and physics); the run continues from its iteration.
    pub fn load_checkpoint(&mut self, path: &Path) -> Result<()> {
        let c = rcfd_io::container::Container::read_file(path)?;
        let meta: Value = serde_json::from_slice(
            c.get("meta")
                .ok_or_else(|| Error::Io("checkpoint: no meta section".into()))?,
        )
        .map_err(|e| Error::Io(format!("checkpoint meta: {e}")))?;
        if meta["kind"] != "rcfd-sim-checkpoint" {
            return Err(Error::Io("not an rcfd simulation checkpoint".into()));
        }
        if meta["mesh_hash"].as_str() != Some(format!("{:016x}", self.geom.mesh.content_hash()).as_str()) {
            return Err(Error::Io("checkpoint was written for a different mesh".into()));
        }
        if meta["energy"].as_bool().unwrap_or(false) != self.energy.is_some() {
            return Err(Error::Io(
                "checkpoint and case disagree about the energy equation".into(),
            ));
        }
        if self.turb.is_some() {
            return Err(Error::Other(
                "the k-epsilon / k-omega SST state is not part of a checkpoint yet".into(),
            ));
        }
        let Engine::Steady(s) = &mut self.engine else {
            return Err(Error::Other(
                "checkpoints are available for steady SIMPLE/SIMPLEC runs only".into(),
            ));
        };
        let u = c.get_f64s("u")?;
        let state = SimpleState {
            iteration: meta["iteration"].as_u64().unwrap_or(0) as usize,
            u: u.chunks_exact(3).map(|v| [v[0], v[1], v[2]]).collect(),
            p: c.get_f64s("p")?,
            phi: c.get_f64s("phi")?,
        };
        s.restore(&state)?;
        if let Some(en) = &mut self.energy {
            let t = c.get_f64s("T")?;
            if t.len() != en.solver.t.values.len() {
                return Err(Error::Io("checkpoint temperature size does not match the mesh".into()));
            }
            en.solver.t.values = t;
        }
        self.done = false;
        self.converged = false;
        self.last = None;
        self.stop_reason = None;
        Ok(())
    }

    /// Initialise this simulation from a solution computed on another mesh (F-115, the
    /// solution transfer between meshes for initialisation. `src_case`
    /// describes the source mesh and `checkpoint` is a checkpoint written for it; velocity,
    /// pressure and (when both cases solve the energy equation) temperature are interpolated onto
    /// this mesh, the face flux is rebuilt, and the iteration counter restarts at zero.
    pub fn interpolate_from(&mut self, src_case: &Case, checkpoint: &Path, mode: InterpMode) -> Result<Value> {
        if self.turb.is_some() {
            return Err(Error::Other(
                "interpolation initialises velocity, pressure and temperature only; turbulence runs are not supported yet".into(),
            ));
        }
        let src_mesh = build_mesh(src_case).map_err(|d| {
            Error::Mesh(format!(
                "source case mesh: {}",
                d.iter().map(|x| x.message.clone()).collect::<Vec<_>>().join("; ")
            ))
        })?;
        let c = rcfd_io::container::Container::read_file(checkpoint)?;
        let meta: Value = serde_json::from_slice(
            c.get("meta")
                .ok_or_else(|| Error::Io("checkpoint: no meta section".into()))?,
        )
        .map_err(|e| Error::Io(format!("checkpoint meta: {e}")))?;
        if meta["kind"] != "rcfd-sim-checkpoint" {
            return Err(Error::Io("not an rcfd simulation checkpoint".into()));
        }
        if meta["mesh_hash"].as_str() != Some(format!("{:016x}", src_mesh.content_hash()).as_str()) {
            return Err(Error::Io(
                "the checkpoint was not written for the mesh of the given source case".into(),
            ));
        }
        let ip = Interpolator::new(&src_mesh, &self.geom.mesh)?;
        let u_src: Vec<Vec3> = c.get_f64s("u")?.chunks_exact(3).map(|v| [v[0], v[1], v[2]]).collect();
        let p_src = c.get_f64s("p")?;
        let u = ip.vector(&u_src, mode)?;
        let p = ip.scalar(&p_src, mode)?;
        let mut with_t = false;
        let t_new = if self.energy.is_some() && meta["energy"].as_bool().unwrap_or(false) {
            with_t = true;
            Some(ip.scalar(&c.get_f64s("T")?, mode)?)
        } else {
            None
        };
        let Engine::Steady(s) = &mut self.engine else {
            return Err(Error::Other(
                "interpolated initialisation is available for steady SIMPLE/SIMPLEC runs only".into(),
            ));
        };
        s.set_fields(u, p)?;
        if let (Some(en), Some(t)) = (&mut self.energy, t_new) {
            en.solver.t.values = t;
        }
        self.done = false;
        self.converged = false;
        self.last = None;
        self.stop_reason = None;
        Ok(json!({
            "mode": if mode == InterpMode::Conservative { "conservative" } else { "nonconservative" },
            "source_cells": src_mesh.n_cells(), "target_cells": self.geom.mesh.n_cells(),
            "temperature": with_t,
        }))
    }

    /// Averaging time accumulated by the run-time statistics (0 when they are off).
    pub fn statistics_time(&self) -> f64 {
        self.stats.as_ref().map_or(0.0, Statistics::time)
    }

    /// Write the current solution as `<dir>/<name>.vtu`; `output.fields` selects the variables.
    pub fn export_vtu(&self, dir: &Path, name: &str) -> Result<PathBuf> {
        use rcfd_io::vtk::{VtkField, VtkValues};
        std::fs::create_dir_all(dir)?;
        let speed = self.field("speed")?;
        let mut extra: Vec<(&'static str, Vec<f64>)> = Vec::new();
        if let Some(en) = &self.energy {
            extra.push(("T", en.solver.t.values.clone()));
        }
        let scalar_fields: Vec<(String, Vec<f64>)> = self
            .scalars
            .as_ref()
            .map(|s| {
                s.field_names()
                    .into_iter()
                    .filter_map(|n| s.field(&n).map(|v| (n, v)))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(t) = &self.turb {
            for (name, f) in t.model.scalars() {
                extra.push((name, f.values.clone()));
            }
            extra.push(("nut", t.model.nut().to_vec()));
            extra.push(("wall_distance", t.model.wall_distance().to_vec()));
            extra.push(("y_plus", t.y_plus_cells()));
        }
        if let Engine::Compressible(c) = &self.engine {
            for n in ["rho", "T", "mach"] {
                if let Some(v) = c.field(n) {
                    extra.push((n, v));
                }
            }
        }
        if let Engine::VarDensity(v) = &self.engine {
            extra.push(("T", v.solver.t.values.clone()));
            for n in ["rho", "mach", "p_abs"] {
                if let Some(f) = v.field(n) {
                    extra.push((n, f));
                }
            }
        }
        let sel = stats::output_selection(&self.case);
        let stat_fields = self.stats.as_ref().map(Statistics::fields).unwrap_or_default();
        let mut fields = Vec::new();
        if stats::keeps(&sel, "U", false) {
            fields.push(VtkField {
                name: "U",
                values: VtkValues::Vector(&self.velocity().values),
            });
        }
        if stats::keeps(&sel, "p", false) {
            fields.push(VtkField {
                name: "p",
                values: VtkValues::Scalar(&self.pressure().values),
            });
        }
        if stats::keeps(&sel, "speed", false) {
            fields.push(VtkField {
                name: "speed",
                values: VtkValues::Scalar(&speed),
            });
        }
        for (n, v) in &extra {
            if stats::keeps(&sel, n, false) {
                fields.push(VtkField {
                    name: n,
                    values: VtkValues::Scalar(v),
                });
            }
        }
        for (n, v) in &scalar_fields {
            if stats::keeps(&sel, n, false) {
                fields.push(VtkField {
                    name: n,
                    values: VtkValues::Scalar(v),
                });
            }
        }
        for (n, v) in &stat_fields {
            if stats::keeps(&sel, n, true) {
                fields.push(VtkField {
                    name: n,
                    values: VtkValues::Scalar(v),
                });
            }
        }
        let derived = crate::postproc::derived_cells(self).map_err(Error::Other)?;
        for (n, v) in &derived.vectors {
            fields.push(VtkField {
                name: n,
                values: VtkValues::Vector(v),
            });
        }
        for (n, v) in &derived.scalars {
            fields.push(VtkField {
                name: n,
                values: VtkValues::Scalar(v),
            });
        }
        let partition = if get_b(&self.case, "decomposition.write_field") {
            let report = crate::decomp::run(&self.case, &self.geom.mesh)
                .map_err(|d| Error::Other(d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")))?;
            Some(crate::decomp::part_field(&report))
        } else {
            None
        };
        if let Some(p) = &partition {
            fields.push(VtkField {
                name: "partition",
                values: VtkValues::Scalar(p),
            });
        }
        let path = dir.join(format!("{name}.vtu"));
        rcfd_io::vtk::write_vtu(&path, &self.geom.mesh, &fields)?;
        Ok(path)
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    pub(crate) const CAVITY_TOML: &str = r#"
title = "cavity"
[mesh]
generator = "cavity_2d"
nx = 12
ny = 12
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 400
tolerance_u = 1e-6
tolerance_p = 1e-6
[reports.lid_drag]
type = "force"
patch = "lid"
direction = [1.0, 0.0, 0.0]
component = "viscous"
[reports.mean_speed]
type = "volume_average"
field = "speed"
[reports.centre]
type = "probe"
field = "ux"
point = [0.5, 0.5, 0.05]
"#;
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::tests_support::CAVITY_TOML as CAVITY;

    #[test]
    fn cavity_case_runs_converges_and_records_monitors() {
        let case = Case::parse_toml(CAVITY).unwrap();
        let mut sim = Simulation::new(&case).unwrap();
        let n = sim.run(1000);
        assert!(sim.is_done() && sim.is_converged(), "{:?} after {n}", sim.last());
        assert!(n < 400);
        let res = sim.monitors.get("res_u").unwrap();
        assert_eq!(res.x.len(), n);
        assert!(res.y[0] > 1e3 * res.y[n - 1]);
        // reports: lid drag opposes the lid motion (the cavity fluid decelerates the lid) and is non-zero
        let drag = sim.monitors.last("lid_drag").unwrap();
        assert!(drag < 0.0, "{drag}");
        assert!(sim.monitors.last("mean_speed").unwrap() > 0.0);
        let stats = sim.field_stats("ux").unwrap();
        assert!(stats["max"].as_f64().unwrap() <= 1.0 + 1e-9);
        let vals = sim.report_values();
        assert_eq!(vals.len(), 3);
        assert!(vals.iter().all(|(_, v)| v.is_ok()));
        assert!(sim.field("vorticity_z").unwrap().iter().any(|w| w.abs() > 0.1));
        assert!(sim.field("T").is_err());
    }

    #[test]
    fn pseudo_transient_modes_are_selectable_from_the_settings_tree() {
        let mut steps = vec![];
        for extra in [
            "",
            "pseudo_mode = \"local\"\npseudo_cfl = 5.0\nalpha_u = 1.0\n",
            "pseudo_mode = \"physical\"\npseudo_dt = 0.2\nalpha_u = 1.0\n",
            "pseudo_mode = \"auto\"\npseudo_factor = 2.0\nalpha_u = 1.0\n",
        ] {
            let toml = CAVITY.replace("[solver]\n", &format!("[solver]\n{extra}"));
            let case = Case::parse_toml(&toml).unwrap();
            let mut sim = Simulation::new(&case).unwrap();
            let n = sim.run(2000);
            assert!(sim.is_converged(), "{extra:?}: not converged after {n}");
            steps.push(n);
        }
        println!("iterations (off, local, physical, auto): {steps:?}");
        let bad = CAVITY.replace("[solver]\n", "[solver]\npseudo_mode = \"fast\"\n");
        assert!(Case::parse_toml(&bad).is_err());
    }

    /// Differentially heated cavity through the settings tree (nondimensional units: alpha = 1,
    /// H = 1, g = Ra Pr), Ra = 1e3, 32x32 cells graded 4:1: Nu from the heat_flow reports against
    /// de Vahl Davis (1983), Nu = 1.118.
    #[test]
    fn heated_cavity_case_reproduces_the_benchmark_nusselt_number() {
        let toml = r#"
title = "heated cavity"
[mesh]
generator = "box_2d"
nx = 32
ny = 32
lx = 1.0
ly = 1.0
grading = 4.0
[physics]
nu = 0.71
rho = 1.0
cp = 1.0
conductivity = 1.0
reference_temperature = 0.5
thermal_expansion = 1.0
gravity = [0.0, -710.0, 0.0]
[energy]
enabled = true
[initial]
temperature = 0.5
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_value = 0.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "tvd_van_leer"
alpha_u = 0.8
max_iterations = 4000
tolerance_u = 1e-9
tolerance_p = 1e-9
[reports.q_hot]
type = "heat_flow"
patch = "xmin"
[reports.q_cold]
type = "heat_flow"
patch = "xmax"
[reports.nu]
type = "expression"
expression = "-q_hot / (conductivity * 1 [K] * 0.03125 [m])"
[reports.t_mid]
type = "probe"
field = "T"
point = [0.5, 0.5, 0.015]
"#;
        let case = Case::parse_toml(toml).unwrap();
        let mut sim = Simulation::new(&case).unwrap();
        let n = sim.run(4000);
        assert!(sim.is_converged(), "not converged after {n}: {:?}", sim.last());
        let q_hot = sim.monitors.last("q_hot").unwrap();
        let q_cold = sim.monitors.last("q_cold").unwrap();
        // depth of the box_2d mesh is lx / nx = 0.03125; heat flows in at the hot wall and out at the cold wall
        let nu = -q_hot / 0.03125;
        println!("{n} iterations, Nu = {nu:.4}, q_hot {q_hot:.5}, q_cold {q_cold:.5}");
        assert!((q_hot + q_cold).abs() < 1e-6 * q_hot.abs(), "{q_hot} {q_cold}");
        assert!((nu / 1.118 - 1.0).abs() < 0.02, "Nu {nu}");
        let nu_expr = sim.monitors.last("nu").unwrap();
        assert!((nu_expr - nu).abs() < 1e-9 * nu, "{nu_expr} vs {nu}");
        assert!((sim.monitors.last("t_mid").unwrap() - 0.5).abs() < 0.01);
        assert!(sim.monitors.get("res_T").is_some());
        let t = sim.field("T").unwrap();
        assert!(t.iter().all(|&v| (-1e-9..=1.0 + 1e-9).contains(&v)));
    }

    /// Temperature and heat-flux boundary expressions on a conduction problem (no flow, no gravity):
    /// the hot wall injects q(y) = 2 (1 + y) W/m^2, the cold wall is held at an expression-defined
    /// temperature; the integrated heat flow in equals the flow out and matches the exact integral.
    #[test]
    fn heat_flux_and_temperature_expressions_drive_conduction() {
        let toml = r#"
title = "conduction"
[mesh]
generator = "box_2d"
nx = 16
ny = 16
lx = 1.0
ly = 1.0
[physics]
conductivity = 1.0
cp = 1.0
rho = 1.0
[energy]
enabled = true
[initial]
temperature = 10.0
[boundaries.xmin]
velocity = "fixed"
temperature = "heat_flux"
heat_flux_expression = "2 [W/m^2] * (1 + y / 1 [m])"
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_expression = "300 [K] + 0 [K/m] * y"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
max_iterations = 3000
tolerance_u = 1e-10
tolerance_p = 1e-10
[reports.q_in]
type = "heat_flow"
patch = "xmin"
[reports.q_out]
type = "heat_flow"
patch = "xmax"
[reports.t_hot]
type = "area_average"
patch = "xmin"
field = "T"
"#;
        let case = Case::parse_toml(toml).unwrap();
        let mut sim = Simulation::new(&case).unwrap();
        let n = sim.run(3000);
        assert!(sim.is_converged(), "not converged after {n}: {:?}", sim.last());
        let depth = 1.0 / 16.0;
        let (qin, qout) = (sim.monitors.last("q_in").unwrap(), sim.monitors.last("q_out").unwrap());
        println!(
            "{n} its: q_in {qin:.8}, q_out {qout:.8}, expected {:.8}, last {:?}, res_T {:?}",
            3.0 * depth,
            sim.last(),
            sim.monitors.last("res_T")
        );
        // exact integral of 2 (1 + y) over y in [0, 1] is 3, times the span 1/16 of the mesh
        assert!((qin + 3.0 * depth).abs() < 1e-9, "{qin}");
        assert!((qout - 3.0 * depth).abs() < 1e-6, "{qout}");
        // the hot wall is hotter than the 300 K cold wall by roughly q L / k = 3 K
        let th = sim.monitors.last("t_hot").unwrap();
        assert!(th > 300.0 && (th - 303.0).abs() < 0.01, "{th}");
    }

    #[test]
    fn energy_settings_are_validated() {
        // energy with piso/pimple is accepted since the transient energy equation exists (F-050)
        let bad = CAVITY
            .replace("algorithm = \"simplec\"", "algorithm = \"piso\"")
            .replace("[solver]\n", "[energy]\nenabled = true\n[solver]\n");
        assert!(Case::parse_toml(&bad).is_ok());
        let e2 = CAVITY.replace(
            "[boundaries.lid]",
            "[boundaries.lid]\ntemperature = \"zero_gradient\"\ntemperature_expression = \"300 [K]\"",
        );
        assert!(Case::parse_toml(&e2).is_err());
        // heat_flow without the energy equation is a clear error, not a number
        let case = Case::parse_toml(CAVITY).unwrap();
        let sim = Simulation::new(&case).unwrap();
        assert!(sim.field("T").is_err());
    }

    #[test]
    fn html_report_from_a_real_run_has_residuals_and_integrals() {
        let mut sim = Simulation::new(&Case::parse_toml(CAVITY).unwrap()).unwrap();
        sim.run(60);
        let html = sim.html_report();
        assert!(html.contains("Normalised residuals") && html.contains("<polyline"));
        assert!(html.contains("lid_drag") && html.contains("mean_speed"));
        // the plotted residual series are the recorded ones
        let n = sim.monitors.get("res_u").unwrap().x.len();
        assert_eq!(n, 60);
        assert!(html.contains("<h1>cavity</h1>"));
        // monitor JSON and CSV exports describe the same run
        let j = sim.monitors.to_json();
        rcfd_core::settings::validate_json_schema(&rcfd_post::monitors::monitor_json_schema(), &j).unwrap();
        assert_eq!(sim.monitors.to_csv().lines().count(), 61);
    }

    #[test]
    fn missing_or_unknown_boundaries_are_reported_with_paths() {
        let mut case = Case::parse_toml(CAVITY).unwrap();
        case.tree.remove_named("boundaries", "walls").unwrap();
        case.tree.add_named("boundaries", "bogus").unwrap();
        let errs = Simulation::new(&case).err().unwrap();
        assert!(errs.iter().any(|e| e.path == "boundaries.walls"), "{errs:?}");
        assert!(errs.iter().any(|e| e.path == "boundaries.bogus"), "{errs:?}");
    }

    #[test]
    fn report_on_missing_patch_is_rejected() {
        let mut case = Case::parse_toml(CAVITY).unwrap();
        case.tree.set("reports.lid_drag.patch", json!("nowhere")).unwrap();
        let errs = Simulation::new(&case).err().unwrap();
        assert_eq!(errs[0].path, "reports.lid_drag.patch");
    }

    #[test]
    fn transient_cavity_approaches_the_steady_solution() {
        let mut steady = Simulation::new(&Case::parse_toml(CAVITY).unwrap()).unwrap();
        steady.run(1000);
        assert!(steady.is_converged());
        let mut case = Case::parse_toml(CAVITY).unwrap();
        for (k, v) in [
            ("solver.algorithm", json!("piso")),
            ("solver.dt", json!(0.05)),
            ("solver.end_time", json!(30.0)),
        ] {
            case.tree.set(k, v).unwrap();
        }
        let mut sim = Simulation::new(&case).unwrap();
        let n = sim.run(10_000);
        assert_eq!(n, 600);
        assert!(sim.is_done() && (sim.time() - 30.0).abs() < 1e-9);
        assert_eq!(sim.monitors.x_label, "time");
        assert!(sim.monitors.get("courant").is_some());
        let (a, b) = (steady.field("ux").unwrap(), sim.field("ux").unwrap());
        let err = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
        assert!(err < 5e-3, "max |ux_transient - ux_steady| = {err}");
        // kinetic energy rises monotonically while the cavity spins up
        let ke = &sim.monitors.get("mean_speed").unwrap().y;
        assert!(ke.windows(2).take(100).all(|w| w[1] >= w[0] - 1e-12));
    }

    #[test]
    fn adaptive_time_step_respects_the_courant_target() {
        let mut case = Case::parse_toml(CAVITY).unwrap();
        for (k, v) in [
            ("solver.algorithm", json!("piso")),
            ("solver.dt", json!(0.5)),
            ("solver.end_time", json!(3.0)),
            ("solver.max_courant", json!(0.4)),
        ] {
            case.tree.set(k, v).unwrap();
        }
        let mut sim = Simulation::new(&case).unwrap();
        sim.run(10_000);
        let co = sim.monitors.get("courant").unwrap();
        assert!(co.y.iter().skip(5).all(|&c| c < 0.5), "{:?}", co.y);
        assert!((sim.time() - 3.0).abs() < 1e-9);
    }

    #[test]
    fn reports_defined_in_the_settings_tree_return_analytic_values() {
        let text = r#"
[mesh]
generator = "channel_2d"
nx = 10
ny = 8
lx = 2.0
ly = 0.5
depth = 0.2
[physics]
nu = 0.01
rho = 2.0
[initial]
velocity = [1.5, 0.0, 0.0]
[boundaries.inlet]
velocity = "fixed"
velocity_value = [1.5, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
"#;
        let mut case = Case::parse_toml(text).unwrap();
        // reports are plain tree entries; define them through tree.set like any other setting
        let defs = [
            ("m_in", json!({"type": "mass_flow", "patch": "inlet"})),
            ("m_out", json!({"type": "mass_flow", "patch": "outlet"})),
            ("u_in", json!({"type": "area_average", "patch": "inlet", "field": "ux"})),
            ("u_avg", json!({"type": "volume_average", "field": "ux"})),
            ("u_max", json!({"type": "maximum", "field": "speed"})),
            ("vol", json!({"type": "volume_integral", "field": "ux"})),
            ("heat", json!({"type": "heat_flow", "patch": "walls"})),
        ];
        for (n, d) in defs {
            case.tree.add_named("reports", n).unwrap();
            for (k, v) in d.as_object().unwrap() {
                case.tree.set(&format!("reports.{n}.{k}"), v.clone()).unwrap();
            }
        }
        let sim = Simulation::new(&case).unwrap();
        let get = |n: &str| sim.report_values().into_iter().find(|(k, _)| k == n).unwrap().1;
        let area = 0.5 * 0.2;
        assert!((get("m_in").unwrap().value + 2.0 * 1.5 * area).abs() < 1e-12);
        assert!((get("m_in").unwrap().value + get("m_out").unwrap().value).abs() < 1e-12);
        assert!((get("u_in").unwrap().value - 1.5).abs() < 1e-12);
        assert!((get("u_avg").unwrap().value - 1.5).abs() < 1e-12);
        assert!((get("u_max").unwrap().value - 1.5).abs() < 1e-12);
        assert!((get("vol").unwrap().value - 1.5 * 2.0 * 0.5 * 0.2).abs() < 1e-12);
        assert_eq!(get("m_in").unwrap().unit, "kg/s");
        assert!(get("heat").unwrap_err().contains("energy"));
    }

    const CHANNEL_EXPR: &str = r#"
[mesh]
generator = "channel_2d"
nx = 10
ny = 8
lx = 2.0
ly = 0.5
depth = 0.2
[physics]
nu = 0.01
rho = 2.0
[expressions.u_peak]
definition = "1.5 [m/s]"
[boundaries.inlet]
velocity = "fixed"
velocity_expression_x = "u_peak * (1 - ((y - 0.25 [m]) / 0.25 [m])^2)"
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
[reports.m_in]
type = "mass_flow"
patch = "inlet"
[reports.u_in]
type = "area_average"
patch = "inlet"
field = "ux"
[reports.q_in]
type = "expression"
expression = "m_in / rho"
[reports.ratio]
type = "expression"
expression = "u_in / u_peak"
"#;

    #[test]
    fn expression_inlet_profile_and_expression_reports() {
        let case = Case::parse_toml(CHANNEL_EXPR).unwrap();
        // expressions serialise in the settings tree and survive a round trip
        let again = Case::parse_toml(&case.to_toml()).unwrap();
        assert_eq!(again.tree.to_json(), case.tree.to_json());
        assert_eq!(case.get("expressions.u_peak.definition").unwrap(), json!("1.5 [m/s]"));
        let sim = Simulation::new(&case).unwrap();
        let get = |n: &str| {
            sim.report_values()
                .into_iter()
                .find(|(k, _)| k == n)
                .unwrap()
                .1
                .unwrap()
        };
        // exact face-centre values of the parabola on the 8 inlet faces
        let h = 0.5 / 8.0;
        let mid: f64 = (0..8)
            .map(|j| {
                let y = (j as f64 + 0.5) * h;
                1.5 * (1.0 - ((y - 0.25) / 0.25f64).powi(2))
            })
            .sum::<f64>()
            / 8.0;
        assert!(
            (get("u_in").value - mid).abs() < 1e-12,
            "{} vs {mid}",
            get("u_in").value
        );
        assert!(
            (mid - 1.0).abs() < 0.01,
            "midpoint rule is within 1 percent of the analytic mean"
        );
        // mass flow is minus rho * mean * area (inflow is negative)
        assert!((get("m_in").value + 2.0 * mid * 0.5 * 0.2).abs() < 1e-12);
        let q = get("q_in");
        assert!((q.value + mid * 0.5 * 0.2).abs() < 1e-12, "{}", q.value);
        assert_eq!(q.unit, "m^3 s^-1");
        let r = get("ratio");
        assert!((r.value - mid / 1.5).abs() < 1e-12 && r.unit.is_empty(), "{r:?}");
    }

    #[test]
    fn time_dependent_inlet_expression_is_updated_every_step() {
        let text = r#"
[mesh]
generator = "channel_2d"
nx = 6
ny = 4
lx = 1.0
ly = 0.5
depth = 0.2
[boundaries.inlet]
velocity = "fixed"
velocity_expression_x = "2 [m/s] * t / 1 [s]"
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "piso"
dt = 0.05
end_time = 0.5
[reports.u_in]
type = "area_average"
patch = "inlet"
field = "ux"
"#;
        let case = Case::parse_toml(text).unwrap();
        let mut sim = Simulation::new(&case).unwrap();
        sim.run(100);
        assert!((sim.time() - 0.5).abs() < 1e-9);
        let u = sim.monitors.get("u_in").unwrap();
        assert!((u.y.last().unwrap() - 1.0).abs() < 1e-12, "{:?}", u.y);
        assert!((u.y[0] - 0.1).abs() < 1e-12, "first step is at t = 0.05: {:?}", u.y);
    }

    #[test]
    fn bad_expressions_are_rejected_with_located_errors() {
        let bad = |extra: &str| {
            Case::parse_toml(&format!(
                "{}
{extra}",
                "[boundaries.lid]
velocity = \"fixed\""
            ))
            .unwrap_err()
        };
        // dimensionally inconsistent sum
        let e = bad("velocity_expression_x = \"1 [m/s] + 2 [K]\"");
        let d = e
            .iter()
            .find(|d| d.path == "boundaries.lid.velocity_expression_x")
            .unwrap();
        assert!(
            d.message.contains("dimension") || d.message.contains("mismatch"),
            "{}",
            d.message
        );
        assert!(d.message.contains('^'), "located: {}", d.message);
        assert_eq!(d.line, Some(3));
        // wrong result dimension
        let e = bad("velocity_expression_y = \"2 [m]\"");
        assert!(e
            .iter()
            .any(|d| d.path.ends_with("velocity_expression_y") && d.message.contains("required")));
        // syntax error
        let e = bad("velocity_expression_z = \"3 * (y + \"");
        assert!(e.iter().any(|d| d.path.ends_with("velocity_expression_z")));
        // unknown identifier in a report expression
        let e = Case::parse_toml(
            "[reports.r]
type = \"expression\"
expression = \"nothere * 2\"
",
        )
        .unwrap_err();
        assert!(e.iter().any(|d| d.path == "reports.r.expression"), "{e:?}");
        // expression velocity needs a fixed condition
        let e = Case::parse_toml(
            "[boundaries.lid]
velocity = \"zero_gradient\"
velocity_expression_x = \"1 [m/s]\"
",
        )
        .unwrap_err();
        assert!(e.iter().any(|d| d.path == "boundaries.lid.velocity"));
    }

    #[test]
    fn report_driven_convergence_condition_stops_the_run_and_records_why() {
        let mut case = Case::parse_toml(CAVITY).unwrap();
        // make the residual targets unreachable so only the drag condition can stop the run
        case.tree.set("solver.tolerance_u", json!(1e-14)).unwrap();
        case.tree.set("solver.tolerance_p", json!(1e-14)).unwrap();
        case.tree.set("solver.max_iterations", json!(3000)).unwrap();
        case.tree.add_named("convergence", "drag_flat").unwrap();
        for (k, v) in [
            ("monitor", json!("lid_drag")),
            ("type", json!("window_range")),
            ("tolerance", json!(1e-5)),
            ("window", json!(10)),
            ("relative", json!(true)),
        ] {
            case.tree.set(&format!("convergence.drag_flat.{k}"), v).unwrap();
        }
        let mut sim = Simulation::new(&case).unwrap();
        assert!(sim.stop_reason().is_none());
        let n = sim.run(5000);
        assert!(sim.is_done() && sim.is_converged());
        assert!(n < 3000, "stopped by the condition, not by the iteration cap: {n}");
        let reason = sim.stop_reason().unwrap();
        assert!(reason.contains("drag_flat") && reason.contains("lid_drag"), "{reason}");
        // the condition really holds on the recorded drag history
        let y = &sim.monitors.get("lid_drag").unwrap().y;
        let w = &y[y.len() - 10..];
        let (lo, hi) = w.iter().fold((f64::MAX, f64::MIN), |(l, h), &v| (l.min(v), h.max(v)));
        assert!((hi - lo) / hi.abs().max(lo.abs()) <= 1e-5, "range {}", hi - lo);
        // ... and it did not hold one step earlier (no premature stop)
        let w0 = &y[y.len() - 11..y.len() - 1];
        let (lo, hi) = w0.iter().fold((f64::MAX, f64::MIN), |(l, h), &v| (l.min(v), h.max(v)));
        assert!((hi - lo) / hi.abs().max(lo.abs()) > 1e-5);
        assert_eq!(sim.status_json()["stop_reason"], json!(reason));
        assert!(sim.html_report().contains("Stopped because"));
        println!("F-109: stopped after {n} iterations: {reason}");
    }

    #[test]
    fn interrupt_condition_and_builtin_reasons_are_recorded() {
        // interrupt: stop when the momentum residual exceeds a threshold (immediately, here)
        let mut case = Case::parse_toml(CAVITY).unwrap();
        case.tree.add_named("convergence", "guard").unwrap();
        case.tree.set("convergence.guard.monitor", json!("res_u")).unwrap();
        case.tree.set("convergence.guard.type", json!("above")).unwrap();
        case.tree.set("convergence.guard.tolerance", json!(0.0)).unwrap();
        let mut sim = Simulation::new(&case).unwrap();
        sim.run(100);
        assert!(sim.is_done() && !sim.is_converged());
        assert_eq!(sim.iteration(), 1);
        assert!(sim.stop_reason().unwrap().starts_with("interrupt condition 'guard'"));
        // min_iterations delays the check
        case.tree.set("solver.min_iterations", json!(7)).unwrap();
        let mut sim = Simulation::new(&case).unwrap();
        sim.run(100);
        assert_eq!(sim.iteration(), 7);
        // plain residual convergence and the iteration cap also say why
        let mut sim = Simulation::new(&Case::parse_toml(CAVITY).unwrap()).unwrap();
        sim.run(1000);
        assert!(sim.stop_reason().unwrap().starts_with("residual targets met"));
        let mut capped = Case::parse_toml(CAVITY).unwrap();
        capped.tree.set("solver.max_iterations", json!(5)).unwrap();
        let mut sim = Simulation::new(&capped).unwrap();
        sim.run(100);
        assert!(sim.stop_reason().unwrap().starts_with("iteration limit 5"));
        // validation: unknown monitor names are rejected with the dotted path
        let e = Case::parse_toml(
            "[convergence.c]
monitor = \"nope\"
",
        )
        .unwrap_err();
        assert!(e.iter().any(|d| d.path == "convergence.c.monitor"), "{e:?}");
    }

    #[test]
    fn all_combination_waits_for_every_condition() {
        let mut case = Case::parse_toml(CAVITY).unwrap();
        case.tree.set("solver.tolerance_u", json!(1e-14)).unwrap();
        case.tree.set("solver.tolerance_p", json!(1e-14)).unwrap();
        for (n, mon, tol) in [("a", "res_u", 1e-3), ("b", "res_p", 1e-5)] {
            case.tree.add_named("convergence", n).unwrap();
            case.tree.set(&format!("convergence.{n}.monitor"), json!(mon)).unwrap();
            case.tree.set(&format!("convergence.{n}.type"), json!("below")).unwrap();
            case.tree
                .set(&format!("convergence.{n}.tolerance"), json!(tol))
                .unwrap();
        }
        let first_any = {
            let mut sim = Simulation::new(&case).unwrap();
            sim.run(2000);
            sim.iteration()
        };
        case.tree.set("solver.stop_when", json!("all")).unwrap();
        let mut sim = Simulation::new(&case).unwrap();
        sim.run(2000);
        assert!(sim.iteration() >= first_any, "{} vs {first_any}", sim.iteration());
        let r = sim.monitors.last("res_p").unwrap();
        assert!(r <= 1e-5 && sim.monitors.last("res_u").unwrap() <= 1e-3);
        let reason = sim.stop_reason().unwrap();
        assert!(reason.contains("'a'") && reason.contains("'b'"), "{reason}");
    }

    #[test]
    fn export_vtu_writes_a_file() {
        let case = Case::parse_toml(CAVITY).unwrap();
        let mut sim = Simulation::new(&case).unwrap();
        sim.run(3);
        let dir = std::env::temp_dir().join(format!("rcfd_sim_{}", std::process::id()));
        let p = sim.export_vtu(&dir, "out").unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.contains("<VTKFile") && text.contains("Name=\"speed\""));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
