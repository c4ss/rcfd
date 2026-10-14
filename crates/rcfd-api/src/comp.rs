//! The compressible solver in the case schema: settings, validation of
//! unsupported combinations and the runtime state of a compressible run.
//!
//! The density-based explicit Euler solver of `rcfd-fv` (HLLC / Roe / Rusanov /
//! AUSM+ fluxes, MUSCL reconstruction, SSP Runge-Kutta; Toro 2009, Shu and
//! Osher 1988) is selected with `solver.algorithm = "compressible"`.

use crate::case::{Case, Diagnostic};
use crate::sim::{conv_scheme, get_b, get_f, get_i, get_s, get_v};
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::compressible::{to_prim, CompBc, CompSettings, CompressibleSolver, Gas, Prim, RiemannSolver, RkScheme};
use rcfd_fv::compressible_bc::TotalInlet;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::PatchKind;
use std::sync::Arc;

/// Is the compressible solver selected?
pub fn enabled(case: &Case) -> bool {
    get_s(case, "solver.algorithm") == "compressible"
}

fn diag(path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

/// Riemann solver of a `compressible.flux` name.
pub fn flux_of(name: &str) -> RiemannSolver {
    match name {
        "rusanov" => RiemannSolver::Rusanov,
        "roe" => RiemannSolver::Roe,
        "ausm_plus" => RiemannSolver::AusmPlus,
        _ => RiemannSolver::Hllc,
    }
}

/// Settings of the solver from the case.
pub fn settings_of(case: &Case) -> CompSettings {
    let limiter = match conv_scheme(&get_s(case, "compressible.reconstruction")) {
        Some(ConvScheme::Tvd(l)) => Some(l),
        _ => None,
    };
    CompSettings {
        solver: flux_of(&get_s(case, "compressible.flux")),
        limiter,
        rk: match get_i(case, "compressible.rk_order") {
            1 => RkScheme::Euler,
            2 => RkScheme::Rk2,
            _ => RkScheme::Rk3,
        },
        cfl: get_f(case, "compressible.cfl"),
    }
}

fn state_of(case: &Case, which: &str) -> Prim {
    Prim {
        rho: get_f(case, &format!("compressible.initial.{which}.density")),
        u: get_v(case, &format!("compressible.initial.{which}.velocity"), [0.0; 3]),
        p: get_f(case, &format!("compressible.initial.{which}.pressure")),
    }
}

/// Fixed state of a `compressible_bc = fixed` boundary.
pub fn boundary_state(case: &Case, patch: &str) -> Prim {
    let b = format!("boundaries.{patch}");
    let p = get_f(case, &format!("{b}.pressure_value"));
    let rho_set = get_f(case, &format!("{b}.density_value"));
    let rho = if rho_set > 0.0 {
        rho_set
    } else {
        p / (get_f(case, "compressible.gas_constant") * get_f(case, &format!("{b}.temperature_value")))
    };
    Prim {
        rho,
        u: get_v(case, &format!("{b}.velocity_value"), [0.0; 3]),
        p,
    }
}

/// Semantic checks of the compressible settings and of combinations the solver does not support.
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let names = case.tree.names("boundaries").unwrap_or_default();
    if !enabled(case) {
        for n in &names {
            let p = format!("boundaries.{n}.compressible_bc");
            if get_s(case, &p) != "auto" {
                out.push(diag(&p, "compressible_bc needs solver.algorithm = \"compressible\""));
            }
        }
        return out;
    }
    if get_s(case, "turbulence.model") != "laminar" {
        out.push(diag(
            "turbulence.model",
            "turbulence models are not available with the compressible solver (inviscid Euler equations)",
        ));
    }
    if get_b(case, "energy.enabled") {
        out.push(diag(
            "energy.enabled",
            "the Boussinesq energy equation cannot be combined with the compressible solver (the energy is part of the Euler system)",
        ));
    }
    if get_s(case, "solver.pseudo_mode") != "off" {
        out.push(diag(
            "solver.pseudo_mode",
            "pseudo-transient stepping applies to SIMPLE / SIMPLEC only; use compressible.mode = \"steady\" to march to a steady state",
        ));
    }
    for f in ["mesh.periodic_x", "mesh.periodic_y"] {
        if get_b(case, f) {
            out.push(diag(f, "periodic meshes are not supported by the compressible solver"));
        }
    }
    if get_i(case, "compressible.rk_order") == 1 && get_s(case, "compressible.reconstruction") != "first_order" {
        out.push(diag(
            "compressible.rk_order",
            "forward Euler (rk_order = 1) with a limited second-order reconstruction is not supported: use rk_order 2 or 3, or compressible.reconstruction = \"first_order\"",
        ));
    }
    if get_s(case, "compressible.mode") == "transient" && get_f(case, "solver.end_time") <= 0.0 {
        out.push(diag(
            "solver.end_time",
            "a transient compressible run needs solver.end_time > 0",
        ));
    }
    let riemann = get_s(case, "compressible.initial.mode") == "riemann";
    for which in if riemann { vec!["left", "right"] } else { vec!["left"] } {
        for f in ["density", "pressure"] {
            let p = format!("compressible.initial.{which}.{f}");
            if get_f(case, &p) <= 0.0 {
                out.push(diag(&p, format!("the initial {f} must be positive")));
            }
        }
        let p = format!("compressible.initial.{which}.velocity");
        if !get_v_ok(case, &p) {
            out.push(diag(&p, "expected 3 components [x, y, z] (or an empty list for zero)"));
        }
    }
    for n in &names {
        let b = format!("boundaries.{n}");
        if get_s(case, &format!("{b}.compressible_bc")) == "fixed" {
            if get_f(case, &format!("{b}.pressure_value")) <= 0.0 {
                out.push(diag(
                    &format!("{b}.pressure_value"),
                    "a fixed compressible state needs a positive static pressure_value [Pa]",
                ));
            }
            if get_f(case, &format!("{b}.density_value")) <= 0.0
                && get_f(case, &format!("{b}.temperature_value")) <= 0.0
            {
                out.push(diag(
                    &format!("{b}.density_value"),
                    "a fixed compressible state needs density_value > 0, or a positive temperature_value to derive it",
                ));
            }
        }
        check_characteristic(case, &b, &mut out);
        if get_s(case, &format!("{b}.turbulence")) != "auto" {
            out.push(diag(
                &format!("{b}.turbulence"),
                "turbulence roles do not apply to the compressible solver",
            ));
        }
        for c in ["x", "y", "z"] {
            let p = format!("{b}.velocity_expression_{c}");
            if !get_s(case, &p).trim().is_empty() {
                out.push(diag(
                    &p,
                    "velocity expressions are not used by the compressible solver: use compressible_bc = \"fixed\" with velocity_value",
                ));
            }
        }
    }
    for r in case.tree.names("reports").unwrap_or_default() {
        let ty = get_s(case, &format!("reports.{r}.type"));
        if matches!(
            ty.as_str(),
            "force" | "moment" | "force_coefficient" | "heat_flow" | "y_plus"
        ) {
            out.push(diag(
                &format!("reports.{r}.type"),
                format!("report type '{ty}' is not available with the compressible solver (use mass_flow, area/volume averages, probes or expressions)"),
            ));
        }
    }
    out
}

/// Validation of the characteristic boundary conditions of one patch (ADR 0006).
fn check_characteristic(case: &Case, b: &str, out: &mut Vec<Diagnostic>) {
    let bc = get_s(case, &format!("{b}.compressible_bc"));
    let positive = |key: &str, what: &str, out: &mut Vec<Diagnostic>| {
        let p = format!("{b}.{key}");
        if get_f(case, &p).partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            out.push(diag(&p, format!("{bc} needs a positive {what} ({key})")));
        }
    };
    match bc.as_str() {
        "total_inlet" => {
            positive("total_pressure_value", "total pressure [Pa]", out);
            positive("temperature_value", "total temperature [K]", out);
            let p = format!("{b}.flow_direction");
            let d = get_v(case, &p, [0.0; 3]);
            if !get_v_ok(case, &p) {
                out.push(diag(&p, "expected 3 components [x, y, z]"));
            } else if d != [0.0; 3] && !(vec3::norm(d) > 0.0 && vec3::norm(d).is_finite()) {
                out.push(diag(&p, "flow_direction must have a finite non-zero length"));
            }
        }
        "pressure_outlet" => positive("pressure_value", "static pressure [Pa]", out),
        "nonreflecting" => {
            positive("pressure_value", "target static pressure [Pa]", out);
            let p = format!("{b}.reflection_value");
            let s = get_f(case, &p);
            if !(0.0..=1.0).contains(&s) {
                out.push(diag(&p, format!("reflection_value must lie in [0, 1], got {s}")));
            }
        }
        "farfield" => {
            positive("pressure_value", "free-stream static pressure [Pa]", out);
            positive("temperature_value", "free-stream static temperature [K]", out);
            let p = format!("{b}.mach_value");
            if get_f(case, &p).partial_cmp(&0.0) == Some(std::cmp::Ordering::Less) {
                out.push(diag(&p, "mach_value must be >= 0"));
            }
        }
        _ => {}
    }
}

/// Mean outward unit normal of a patch.
fn patch_normal(geom: &FvGeometry, patch: &str) -> Vec3 {
    let mesh = &geom.mesh;
    let sf = mesh.face_area_vectors();
    let mut sum = [0.0; 3];
    if let Some(p) = mesh.patches().iter().find(|p| p.name == patch) {
        for f in p.faces() {
            sum = vec3::add(sum, sf[f]);
        }
    }
    let n = vec3::norm(sum);
    if n > 0.0 {
        vec3::scale(sum, 1.0 / n)
    } else {
        [1.0, 0.0, 0.0]
    }
}

/// Characteristic boundary condition of a patch, if `compressible_bc` selects one.
fn characteristic_bc(case: &Case, geom: &FvGeometry, patch: &str) -> Option<CompBc> {
    let b = format!("boundaries.{patch}");
    let r = get_f(case, "compressible.gas_constant");
    let gamma = get_f(case, "compressible.gamma");
    let p = get_f(case, &format!("{b}.pressure_value"));
    Some(match get_s(case, &format!("{b}.compressible_bc")).as_str() {
        "total_inlet" => {
            let d = get_v(case, &format!("{b}.flow_direction"), [0.0; 3]);
            let dir = if vec3::norm(d) > 0.0 {
                vec3::scale(d, 1.0 / vec3::norm(d))
            } else {
                vec3::scale(patch_normal(geom, patch), -1.0)
            };
            CompBc::TotalInlet(TotalInlet {
                p0: get_f(case, &format!("{b}.total_pressure_value")),
                t0: get_f(case, &format!("{b}.temperature_value")),
                r_gas: r,
                dir,
            })
        }
        "pressure_outlet" => CompBc::PressureOutlet { p },
        "nonreflecting" => CompBc::NonReflecting {
            p,
            sigma: get_f(case, &format!("{b}.reflection_value")),
        },
        "farfield" => {
            let t = get_f(case, &format!("{b}.temperature_value"));
            let a = (gamma * r * t).sqrt();
            let m = get_f(case, &format!("{b}.mach_value"));
            let ang = get_f(case, &format!("{b}.flow_angle")).to_radians();
            CompBc::FarField(Prim {
                rho: p / (r * t),
                u: [m * a * ang.cos(), m * a * ang.sin(), 0.0],
                p,
            })
        }
        _ => return None,
    })
}

fn get_v_ok(case: &Case, path: &str) -> bool {
    case.tree
        .get(path)
        .ok()
        .and_then(|v| v.as_array().map(|a| a.is_empty() || a.len() == 3))
        .unwrap_or(true)
}

/// Outcome of one compressible step.
pub(crate) struct Advance {
    /// Largest relative density change in the step.
    pub res_rho: f64,
    /// Largest relative pressure change in the step.
    pub res_p: f64,
    /// Relative change of the total mass since the start.
    pub mass_drift: f64,
    /// Step size taken.
    pub dt: f64,
    /// Failure message (non-positive state, ...).
    pub error: Option<String>,
}

/// Runtime state of a compressible simulation.
pub(crate) struct CompState {
    /// Solver.
    pub solver: CompressibleSolver,
    /// Specific gas constant.
    pub r_gas: f64,
    /// Cell velocity (shadow field refreshed after each step).
    pub u: VectorField,
    /// Static pressure (shadow field).
    pub p: ScalarField,
    /// Mass flux `rho u . S` through every face (boundary faces from the adjacent cell).
    pub phi: Vec<f64>,
    /// March to a steady state.
    pub steady: bool,
    /// Steady-mode tolerance on the relative density change.
    pub tol: f64,
    mass0: f64,
}

impl CompState {
    /// Build the solver from the case on `geom`.
    pub fn new(case: &Case, geom: &Arc<FvGeometry>) -> Result<Self, Vec<Diagnostic>> {
        let gamma = get_f(case, "compressible.gamma");
        let riemann = get_s(case, "compressible.initial.mode") == "riemann";
        let (left, right) = (state_of(case, "left"), state_of(case, "right"));
        let axis = match get_s(case, "compressible.initial.split_axis").as_str() {
            "y" => 1,
            "z" => 2,
            _ => 0,
        };
        let split = get_f(case, "compressible.initial.split_position");
        let init = move |x: Vec3| if riemann && x[axis] >= split { right } else { left };
        let mut solver = CompressibleSolver::new((**geom).clone(), Gas { gamma }, settings_of(case), init)
            .map_err(|e| vec![diag("compressible.initial", e.to_string())])?;
        for p in geom.mesh.patches() {
            if p.kind == PatchKind::Cyclic {
                return Err(vec![diag(
                    "mesh",
                    format!(
                        "patch '{}' is periodic; the compressible solver has no periodic boundaries",
                        p.name
                    ),
                )]);
            }
        }
        for n in case.tree.names("boundaries").unwrap_or_default() {
            let b = format!("boundaries.{n}");
            let bc = match get_s(case, &format!("{b}.compressible_bc")).as_str() {
                "slip" => CompBc::Slip,
                "extrapolate" => CompBc::Extrapolate,
                "fixed" => CompBc::Fixed(boundary_state(case, &n)),
                _ => match characteristic_bc(case, geom, &n) {
                    Some(bc) => bc,
                    None => continue,
                },
            };
            // unknown patch names are reported by the caller
            let _ = solver.set_bc(&n, bc);
        }
        let n = geom.n_cells();
        let mut st = CompState {
            r_gas: get_f(case, "compressible.gas_constant"),
            u: VectorField::uniform("U", geom, [0.0; 3]),
            p: ScalarField::uniform("p", geom, 0.0),
            phi: vec![0.0; geom.mesh.n_faces()],
            steady: get_s(case, "compressible.mode") == "steady",
            tol: get_f(case, "compressible.steady_tolerance"),
            mass0: 0.0,
            solver,
        };
        let _ = n;
        st.mass0 = st.solver.totals()[0];
        st.refresh();
        Ok(st)
    }

    /// Update the shadow fields and the face mass flux from the conserved state.
    pub fn refresh(&mut self) {
        let prim = self.solver.primitives();
        for (i, p) in prim.iter().enumerate() {
            self.u.values[i] = p.u;
            self.p.values[i] = p.p;
        }
        let g = &self.solver.geom;
        let mesh = &g.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let sf = mesh.face_area_vectors();
        for f in 0..mesh.n_faces() {
            let mf = |p: &Prim| p.rho * vec3::dot(p.u, sf[f]);
            self.phi[f] = if g.empty[f] {
                0.0
            } else if f < g.n_internal {
                0.5 * (mf(&prim[own[f]]) + mf(&prim[nei[f]]))
            } else {
                mf(&prim[own[f]])
            };
        }
    }

    /// Advance one CFL-limited step, clipped to `t_end` for transient runs.
    pub fn advance(&mut self, t_end: Option<f64>) -> Advance {
        let gas = self.solver.gas;
        let before = self.solver.u.clone();
        let mut dt = self.solver.max_dt();
        if let Some(te) = t_end {
            dt = dt.min((te - self.solver.time).max(1e-300));
        }
        let fail = |msg: String, dt: f64| Advance {
            res_rho: f64::NAN,
            res_p: f64::NAN,
            mass_drift: f64::NAN,
            dt,
            error: Some(msg),
        };
        if !(dt.is_finite() && dt > 0.0) {
            return fail(format!("the stable time step is not positive and finite ({dt})"), dt);
        }
        if let Err(e) = self.solver.step(dt) {
            return fail(e.to_string(), dt);
        }
        let mut res_rho: f64 = 0.0;
        let mut res_p: f64 = 0.0;
        for (a, b) in self.solver.u.iter().zip(&before) {
            let (pa, pb) = (to_prim(gas, a), to_prim(gas, b));
            if !(pa.rho.is_finite() && pa.p.is_finite() && pa.rho > 0.0 && pa.p > 0.0) {
                return fail(
                    "non-positive or non-finite density / pressure after the step".into(),
                    dt,
                );
            }
            res_rho = res_rho.max((a[0] - b[0]).abs() / b[0]);
            res_p = res_p.max((pa.p - pb.p).abs() / pb.p);
        }
        let mass = self.solver.totals()[0];
        Advance {
            res_rho,
            res_p,
            mass_drift: (mass - self.mass0).abs() / self.mass0,
            dt,
            error: None,
        }
    }

    /// Named cell field (`rho`, `p`, `T`, `mach`), if it is one of the compressible ones.
    pub fn field(&self, name: &str) -> Option<Vec<f64>> {
        let gas = self.solver.gas;
        let prim = self.solver.primitives();
        Some(match name {
            "rho" => prim.iter().map(|p| p.rho).collect(),
            "T" => prim.iter().map(|p| p.p / (p.rho * self.r_gas)).collect(),
            "mach" => prim.iter().map(|p| vec3::norm(p.u) / p.sound_speed(gas)).collect(),
            _ => return None,
        })
    }
}
