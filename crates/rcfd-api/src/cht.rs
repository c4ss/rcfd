//! Conjugate heat transfer in the settings tree (F-051, ADR 0024): solid cell zones with their own conductivity, density
//! and specific heat, thermal contact resistance on interfaces, convective / thin-wall / radiating wall conditions and a
//! prescribed (frozen) flow field.
//!
//! The numerics live in `rcfd_fv::cht` and `rcfd_fv::thermal_bc`; this module reads and checks the settings, builds the
//! material arrays and the frozen face flux, and hands them to the energy solver. Every new key has an explicit unset
//! sentinel (`-1`, `""` or an empty list) with a located error when it is needed and unset (ADR 0024 D3, D4).

use crate::case::{Case, Diagnostic};
use crate::exprs;
use crate::sources::{check_zone, gf, gs, zone_cells};
use rcfd_core::settings::Schema;
use rcfd_core::units::Dimension;
use rcfd_core::vec3;
use rcfd_fv::bc::{Bc, FixedGradient, Profile};
use rcfd_fv::cht::{self, ChtMaterials};
use rcfd_fv::energy::EnergySolver;
use rcfd_fv::thermal_bc::ConvectiveWall;
use rcfd_fv::FvGeometry;
use rcfd_linalg::SolveControl;
use rcfd_mesh::PatchKind;

/// Sentinel of the numeric keys that are not set.
pub const UNSET: f64 = -1.0;

fn d(path: impl Into<String>, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

fn list_f(case: &Case, path: &str) -> Vec<f64> {
    case.tree
        .get(path)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect())
        .unwrap_or_default()
}

fn flag(case: &Case, path: &str) -> bool {
    case.tree.get(path).ok().and_then(|v| v.as_bool()).unwrap_or(false)
}

/// Names of the configured solids.
pub fn solid_names(case: &Case) -> Vec<String> {
    case.tree.names("solids").unwrap_or_default()
}

/// Names of the configured thermal interfaces.
pub fn interface_names(case: &Case) -> Vec<String> {
    case.tree.names("thermal_interfaces").unwrap_or_default()
}

/// Is the flow prescribed (`solver.frozen_flow`)?
pub fn frozen(case: &Case) -> bool {
    flag(case, "solver.frozen_flow")
}

fn convection_patches(case: &Case) -> Vec<String> {
    case.tree
        .names("boundaries")
        .unwrap_or_default()
        .into_iter()
        .filter(|b| gs(case, &format!("boundaries.{b}.temperature")) == "convection")
        .collect()
}

/// Does the case use any conjugate-heat-transfer feature?
pub fn enabled(case: &Case) -> bool {
    !solid_names(case).is_empty() || frozen(case) || !convection_patches(case).is_empty()
}

/// Schema of the `solids` collection.
pub fn solids_schema() -> Schema {
    Schema::named(
        Schema::object()
            .doc("A solid cell zone with its own conductivity, density and specific heat (conjugate heat transfer, ADR 0024). Needs energy.enabled and solver.frozen_flow = true")
            .field(
                "zone",
                Schema::enumeration("unset", &["unset", "all", "box", "cell_zone", "register"])
                    .doc("Which cells are solid: the whole mesh, the cells whose centres lie in the box [box_min, box_max], a named mesh cell zone or a cell register. \"unset\" is an error (there is no default zone)"),
            )
            .field("register", Schema::string("").doc("Name of the register for zone = \"register\""))
            .field("box_min", Schema::list(Schema::float(0.0)).doc("Lower corner [m] of the box zone"))
            .field("box_max", Schema::list(Schema::float(0.0)).doc("Upper corner [m] of the box zone"))
            .field("cell_zone", Schema::string("").doc("Name of the mesh cell zone"))
            .field(
                "conductivity",
                Schema::float(UNSET).doc("Thermal conductivity [W/m/K], constant and isotropic; must be set (> 0), -1 = unset"),
            )
            .field(
                "density",
                Schema::float(UNSET).doc("Density [kg/m^3]; must be set (> 0), -1 = unset"),
            )
            .field(
                "specific_heat",
                Schema::float(UNSET).doc("Specific heat [J/kg/K]; must be set (> 0), -1 = unset"),
            )
            .field(
                "initial_temperature",
                Schema::float(UNSET).doc("Initial temperature [K] of the solid cells; -1 = unset, the cells start at initial.temperature"),
            ),
    )
    .doc("Solid cell zones of a conjugate heat-transfer case (ADR 0024). Zones must not overlap; the fluid is every cell in no solid")
}

/// Schema of the `thermal_interfaces` collection.
pub fn interfaces_schema() -> Schema {
    Schema::named(
        Schema::object()
            .doc("Thermal contact resistance on the faces between two materials (ADR 0024 D2)")
            .field(
                "solid_a",
                Schema::string("").doc("First material: the name of a solids entry or \"fluid\" (all cells in no solid); empty is an error"),
            )
            .field(
                "solid_b",
                Schema::string("").doc("Second material: the name of a solids entry or \"fluid\"; empty is an error"),
            )
            .field(
                "resistance",
                Schema::float(UNSET).doc("Contact (or fouling-layer) resistance per unit area [m^2 K/W] added in series on the faces between the two materials; 0 is a perfect contact, -1 = unset (an error)"),
            ),
    )
    .doc("Thermal contact resistances between materials (ADR 0024)")
}

/// Settings checks (no mesh needed).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let solids = solid_names(case);
    let ifaces = interface_names(case);
    let alg = gs(case, "solver.algorithm");
    let frozen_flow = frozen(case);
    let energy_on = flag(case, "energy.enabled");
    // a solved flow with the block-coupled algorithm may share the domain with solid zones (ADR 0029)
    let solved_coupled = alg == "coupled" && !frozen_flow;
    if frozen_flow {
        if !energy_on {
            out.push(d(
                "solver.frozen_flow",
                "a prescribed flow field is for the energy equation: set energy.enabled = true",
            ));
        }
        if !matches!(alg.as_str(), "simple" | "simplec" | "piso" | "pimple") {
            out.push(d(
                "solver.frozen_flow",
                format!("solver.frozen_flow is available with the simple, simplec, piso and pimple algorithms, not \"{alg}\""),
            ));
        }
    }
    // a convection condition takes the constant physics.conductivity (or the conductivity of its solid) for the half-cell
    // resistance, so it is not combined with a temperature-dependent conductivity law or with solvers that bypass it
    if !convection_patches(case).is_empty() {
        for b in convection_patches(case) {
            for (present, path, what) in [
                (
                    crate::properties::enabled(case),
                    "properties",
                    "temperature-dependent property laws",
                ),
                (
                    crate::comp::enabled(case) || crate::vd::enabled(case),
                    "compressible",
                    "the compressible / variable-density solvers",
                ),
            ] {
                if present {
                    out.push(d(
                        format!("boundaries.{b}.temperature"),
                        format!("temperature = \"convection\" cannot be combined with {what} (see {path}); use fixed or heat_flux"),
                    ));
                }
            }
        }
    }
    if !solids.is_empty() || frozen_flow {
        let scalar_present = ["scalars", "species"]
            .iter()
            .any(|c| !case.tree.names(c).unwrap_or_default().is_empty());
        if scalar_present {
            out.push(d(
                "scalars",
                "user scalars and species are not advanced with solid zones or a prescribed flow (ADR 0024 D5); remove them",
            ));
        }
        let conflicts: [(bool, &str, &str); 9] = [
            (
                crate::properties::enabled(case),
                "properties",
                "temperature-dependent property laws",
            ),
            (
                crate::radiation::enabled(case),
                "radiation.model",
                "the radiation model",
            ),
            (
                !matches!(gs(case, "turbulence.model").as_str(), "" | "laminar"),
                "turbulence.model",
                "a turbulence model",
            ),
            (
                crate::comp::enabled(case) || crate::vd::enabled(case),
                "compressible",
                "the compressible / variable-density solvers",
            ),
            (crate::lumped::enabled(case), "components", "lumped components"),
            // the coupled algorithm solves the flow with the solid cells excluded (ADR 0029 D3); with a prescribed flow it
            // has nothing to prescribe
            (
                alg == "coupled" && frozen_flow,
                "solver.algorithm",
                "the coupled algorithm",
            ),
            (
                !solved_coupled
                    && flag(case, "energy.buoyancy")
                    && gf(case, "physics.thermal_expansion") != 0.0
                    && list_f(case, "physics.gravity").iter().any(|g| *g != 0.0),
                "energy.buoyancy",
                "buoyancy (physics.thermal_expansion with physics.gravity)",
            ),
            (
                crate::rheology::enabled(case),
                "rheology.model",
                "non-Newtonian rheology",
            ),
            (
                !case.tree.names("udf").unwrap_or_default().is_empty(),
                "udf",
                "user functions",
            ),
        ];
        for (present, path, what) in conflicts {
            if present {
                out.push(d(
                    path,
                    format!("{what} cannot be combined with solid zones or a prescribed flow (ADR 0024 D5)"),
                ));
            }
        }
        for n in crate::zone_sources::names(case) {
            if gs(case, &format!("zone_sources.{n}.kind")) == "fixed_value" {
                out.push(d(
                    format!("zone_sources.{n}.kind"),
                    "fixed_value zone sources are not combined with solid zones or a prescribed flow (use a source)",
                ));
            }
        }
    }
    if !solids.is_empty() {
        if !energy_on {
            out.push(d("solids", "solid zones need energy.enabled = true"));
        }
        if !frozen_flow && !solved_coupled {
            out.push(d(
                "solver.frozen_flow",
                "solid zones need either solver.frozen_flow = true (prescribed flow, ADR 0024 D5) or solver.algorithm = \"coupled\" (solved flow with the solid cells excluded, ADR 0029); the segregated algorithms do not solve a flow next to solid zones",
            ));
        }
    }
    for n in &solids {
        let p = |f: &str| format!("solids.{n}.{f}");
        match gs(case, &p("zone")).as_str() {
            "unset" | "" => out.push(d(
                p("zone"),
                "set zone to all, box, cell_zone or register (a solid has no default zone)",
            )),
            _ => out.extend(check_zone(case, &format!("solids.{n}"))),
        }
        for (key, what) in [
            ("conductivity", "thermal conductivity [W/m/K]"),
            ("density", "density [kg/m^3]"),
            ("specific_heat", "specific heat [J/kg/K]"),
        ] {
            let v = gf(case, &p(key));
            if !(v > 0.0 && v.is_finite()) {
                out.push(d(
                    p(key),
                    format!("the {what} of a solid must be set and positive (found {v}; -1 means unset)"),
                ));
            }
        }
        let ti = gf(case, &p("initial_temperature"));
        if ti != UNSET && !(ti > 0.0 && ti.is_finite()) {
            out.push(d(
                p("initial_temperature"),
                format!("must be a positive temperature in K, or -1 for the initial.temperature (found {ti})"),
            ));
        }
    }
    for n in &ifaces {
        let p = |f: &str| format!("thermal_interfaces.{n}.{f}");
        if solids.is_empty() && !ifaces.is_empty() {
            out.push(d(
                p("solid_a"),
                "thermal interfaces join materials: define solids first",
            ));
        }
        let mut names_ok = Vec::new();
        for f in ["solid_a", "solid_b"] {
            let v = gs(case, &p(f));
            if v.is_empty() {
                out.push(d(p(f), "name a solids entry or \"fluid\""));
            } else if v != "fluid" && !solids.contains(&v) {
                out.push(d(
                    p(f),
                    format!("unknown material '{v}' (solids: {}; or \"fluid\")", solids.join(", ")),
                ));
            } else {
                names_ok.push(v);
            }
        }
        if names_ok.len() == 2 && names_ok[0] == names_ok[1] {
            out.push(d(p("solid_b"), "the two materials of an interface must differ"));
        }
        let r = gf(case, &p("resistance"));
        if r == UNSET {
            out.push(d(
                p("resistance"),
                "set the contact resistance [m^2 K/W] (0 = perfect contact); -1 means unset",
            ));
        } else if !(r >= 0.0 && r.is_finite()) {
            out.push(d(p("resistance"), format!("must not be negative (found {r})")));
        }
    }
    // thermal wall conditions
    for b in case.tree.names("boundaries").unwrap_or_default() {
        let p = |f: &str| format!("boundaries.{b}.{f}");
        let kind = gs(case, &p("temperature"));
        let h = gf(case, &p("heat_transfer_coefficient"));
        let tinf = gf(case, &p("free_stream_temperature"));
        let th = list_f(case, &p("wall_thickness"));
        let kw = list_f(case, &p("wall_conductivity"));
        let eps = gf(case, &p("wall_emissivity"));
        let trad = gf(case, &p("radiation_temperature"));
        if kind != "convection" {
            for (set, key) in [
                (h != UNSET, "heat_transfer_coefficient"),
                (tinf != UNSET, "free_stream_temperature"),
                (!th.is_empty(), "wall_thickness"),
                (!kw.is_empty(), "wall_conductivity"),
                (eps != 0.0, "wall_emissivity"),
                (trad != UNSET, "radiation_temperature"),
            ] {
                if set {
                    out.push(d(p(key), "only used with temperature = \"convection\""));
                }
            }
            continue;
        }
        if !energy_on {
            out.push(d(
                p("temperature"),
                "a convection condition needs energy.enabled = true",
            ));
        }
        if h == UNSET {
            out.push(d(
                p("heat_transfer_coefficient"),
                "set the heat-transfer coefficient [W/m^2/K] (0 = none); -1 means unset",
            ));
        } else if !(h >= 0.0 && h.is_finite()) {
            out.push(d(
                p("heat_transfer_coefficient"),
                format!("must not be negative (found {h})"),
            ));
        }
        if h > 0.0 && !(tinf > 0.0 && tinf.is_finite()) {
            out.push(d(
                p("free_stream_temperature"),
                "set the free-stream temperature [K] (> 0) for a convection coefficient above zero; -1 means unset",
            ));
        }
        if th.len() != kw.len() {
            out.push(d(
                p("wall_conductivity"),
                format!(
                    "wall_thickness has {} entries but wall_conductivity has {}; give one conductivity per layer",
                    th.len(),
                    kw.len()
                ),
            ));
        }
        for (key, v) in [("wall_thickness", &th), ("wall_conductivity", &kw)] {
            if v.iter().any(|x| !(*x > 0.0 && x.is_finite())) {
                out.push(d(p(key), "every layer entry must be positive"));
            }
        }
        if !(0.0..=1.0).contains(&eps) {
            out.push(d(p("wall_emissivity"), format!("must lie in [0, 1] (found {eps})")));
        }
        if eps > 0.0 && !(trad > 0.0 && trad.is_finite()) {
            out.push(d(
                p("radiation_temperature"),
                "set the radiation (ambient) temperature [K] (> 0) for a wall emissivity above zero; -1 means unset",
            ));
        }
        if eps == 0.0 && trad != UNSET {
            out.push(d(
                p("radiation_temperature"),
                "has no effect with wall_emissivity = 0; remove it or set an emissivity",
            ));
        }
    }
    out
}

/// The materials of a conjugate case on a mesh, and the prescribed flux.
pub struct ChtModel {
    /// Material arrays and contact resistances for the energy solver.
    pub mats: ChtMaterials,
    /// Per cell: 0 fluid, `i + 1` the `i`-th solid of `names`.
    pub label: Vec<usize>,
    /// Solid names (sorted).
    pub names: Vec<String>,
    /// Initial temperatures per solid (`None` = `initial.temperature`).
    pub init_t: Vec<Option<f64>>,
    /// Conductivity at every face, including contact resistances (W/m/K).
    pub face_k: Vec<f64>,
    /// Prescribed face flux when the flow is frozen.
    pub frozen_flux: Option<Vec<f64>>,
    /// Whether any solid or interface exists (otherwise the energy solver keeps its uniform coefficients).
    pub has_solids: bool,
    /// Whether the convection scheme needs gradients (selects the explicit non-orthogonal part in flow reports).
    pub non_orthogonal: bool,
}

impl ChtModel {
    /// Build from the case; `None` when no conjugate feature is used. Located errors for empty or overlapping zones,
    /// interfaces without faces and a flux through solid cells.
    pub fn from_case(
        case: &Case,
        geom: &FvGeometry,
        non_orthogonal: bool,
    ) -> Result<Option<ChtModel>, Vec<Diagnostic>> {
        if !enabled(case) {
            return Ok(None);
        }
        let mut errs = Vec::new();
        let names = solid_names(case);
        let n = geom.n_cells();
        let rho_cp_ref = gf(case, "physics.rho") * gf(case, "physics.cp");
        let k_fluid = gf(case, "physics.conductivity");
        let mut label = vec![0usize; n];
        let mut k_cell = vec![k_fluid; n];
        let mut rho_cp_cell = vec![rho_cp_ref; n];
        let mut init_t = Vec::new();
        for (i, name) in names.iter().enumerate() {
            let p = |f: &str| format!("solids.{name}.{f}");
            let mask = match zone_cells(case, &format!("solids.{name}"), geom) {
                Ok(m) => m,
                Err(e) => {
                    errs.push(e);
                    init_t.push(None);
                    continue;
                }
            };
            let (k, rho, cp) = (
                gf(case, &p("conductivity")),
                gf(case, &p("density")),
                gf(case, &p("specific_heat")),
            );
            for c in 0..n {
                if !mask[c] {
                    continue;
                }
                if label[c] != 0 {
                    errs.push(d(
                        p("zone"),
                        format!(
                            "overlaps solids.{} (cell {c} belongs to both); solid zones must not overlap",
                            names[label[c] - 1]
                        ),
                    ));
                    break;
                }
                label[c] = i + 1;
                k_cell[c] = k;
                rho_cp_cell[c] = rho * cp;
            }
            let ti = gf(case, &p("initial_temperature"));
            init_t.push((ti != UNSET).then_some(ti));
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        let mut contacts = Vec::new();
        for name in interface_names(case) {
            let p = |f: &str| format!("thermal_interfaces.{name}.{f}");
            let find = |m: String| {
                if m == "fluid" {
                    Some(0)
                } else {
                    names.iter().position(|x| *x == m).map(|i| i + 1)
                }
            };
            let (Some(a), Some(b)) = (find(gs(case, &p("solid_a"))), find(gs(case, &p("solid_b")))) else {
                continue; // reported by the settings check
            };
            let found = cht::contact_faces(geom, &label, a, b, gf(case, &p("resistance")));
            if found.is_empty() {
                errs.push(d(
                    p("solid_a"),
                    "the two materials share no internal face of the mesh, so the contact resistance has nothing to act on",
                ));
            }
            contacts.extend(found);
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        let has_solids = !names.is_empty();
        let mats = ChtMaterials {
            rho_cp_ref,
            k_cell,
            rho_cp_cell,
            contacts,
        };
        let face_k = mats.face_conductivity(geom);
        let frozen_flux = if frozen(case) {
            Some(frozen_face_flux(case, geom, &label)?)
        } else {
            None
        };
        Ok(Some(ChtModel {
            mats,
            label,
            names,
            init_t,
            face_k,
            frozen_flux,
            has_solids,
            non_orthogonal,
        }))
    }

    /// Hand the material data to the energy solver (per-face coefficient, capacity weights, tight linear solves) and set
    /// the initial temperatures of the solids. Does nothing for a case without solids and interfaces.
    pub fn configure(&self, geom: &FvGeometry, solver: &mut EnergySolver) {
        if !self.has_solids {
            return;
        }
        self.mats.configure(geom, solver);
        solver.solver = SolveControl {
            tol_abs: 1e-300,
            tol_rel: 1e-13,
            max_iter: 5000,
        };
        for (c, &l) in self.label.iter().enumerate() {
            if let Some(Some(t)) = l.checked_sub(1).map(|i| self.init_t[i]) {
                solver.t.values[c] = t;
            }
        }
        // the time-history of the solver was built from the uniform initial field
        solver.history = rcfd_fv::ddt::History::new(solver.t.values.clone());
    }

    /// Conductive heat flow out of the domain through `patch` (W), with the conductivity of the owner cell of every face.
    pub fn heat_flow_out(&self, geom: &FvGeometry, solver: &EnergySolver, patch: &str) -> Option<f64> {
        let flows = cht::conductive_face_flows(geom, &solver.t, &self.face_k, self.non_orthogonal);
        cht::patch_heat_flow_out(geom, &flows, patch)
    }

    /// Heat flux into the domain `q` as a per-face gradient profile `q / k_owner` (heat-flux boundary condition).
    pub fn flux_profile(&self, geom: &FvGeometry, patch: &str, q: &[f64]) -> Profile {
        let p = geom.mesh.patch(patch).expect("known patch");
        Profile::PerFace(p.faces().zip(q).map(|(f, q)| [q / self.face_k[f], 0.0, 0.0]).collect())
    }

    /// Owner-cell conductivity at the faces of `patch`.
    pub fn patch_k(&self, geom: &FvGeometry, patch: &str) -> Vec<f64> {
        geom.mesh
            .patch(patch)
            .map(|p| p.faces().map(|f| self.face_k[f]).collect())
            .unwrap_or_default()
    }
}

/// Heat-flux boundary condition `FixedGradient(q/k)` with the per-face conductivity (`q` INTO the domain, per face).
pub fn heat_flux_bc(model: &ChtModel, geom: &FvGeometry, patch: &str, q: &[f64]) -> Box<dyn Bc> {
    Box::new(FixedGradient(model.flux_profile(geom, patch, q)))
}

/// The convection / thin-wall / radiation condition of a patch from `boundaries.<patch>`.
pub fn convective_bc(case: &Case, model: &ChtModel, geom: &FvGeometry, patch: &str) -> Box<dyn Bc> {
    let p = |f: &str| format!("boundaries.{patch}.{f}");
    let h = gf(case, &p("heat_transfer_coefficient")).max(0.0);
    let th = list_f(case, &p("wall_thickness"));
    let kw = list_f(case, &p("wall_conductivity"));
    let wall_resistance: f64 = th.iter().zip(&kw).map(|(t, k)| t / k).sum();
    Box::new(ConvectiveWall {
        k: model.patch_k(geom, patch),
        h,
        t_inf: gf(case, &p("free_stream_temperature")),
        wall_resistance,
        emissivity: gf(case, &p("wall_emissivity")),
        t_rad: gf(case, &p("radiation_temperature")),
    })
}

/// Prescribed face flux `u(x_f) . S_f` from `initial.velocity` and `initial.velocity_expression_x/y/z` at the face centres;
/// patches whose velocity condition is a wall, symmetry or fixed zero carry no flux. Located errors when the flux touches a solid cell or violates continuity.
fn frozen_face_flux(case: &Case, geom: &FvGeometry, label: &[usize]) -> Result<Vec<f64>, Vec<Diagnostic>> {
    let ctx = exprs::spatial_context(case);
    let base = match case
        .tree
        .get("initial.velocity")
        .ok()
        .as_ref()
        .and_then(|v| v.as_array().cloned())
    {
        Some(a) if a.len() == 3 => [
            a[0].as_f64().unwrap_or(0.0),
            a[1].as_f64().unwrap_or(0.0),
            a[2].as_f64().unwrap_or(0.0),
        ],
        _ => [0.0; 3],
    };
    let mut comps = Vec::new();
    let mut errs = Vec::new();
    for c in ["x", "y", "z"] {
        match exprs::compile_bc(
            case,
            &format!("initial.velocity_expression_{c}"),
            Dimension::VELOCITY,
            &ctx,
        ) {
            Ok(e) => comps.push(e),
            Err(e) => {
                errs.extend(e);
                comps.push(None);
            }
        }
    }
    if !errs.is_empty() {
        return Err(errs);
    }
    let fc = geom.mesh.face_centres();
    let sf = geom.mesh.face_area_vectors();
    let nf = geom.mesh.n_faces();
    let mut no_flux = vec![false; nf];
    for patch in geom.mesh.patches() {
        let b = format!("boundaries.{}", patch.name);
        // a patch carries no prescribed flux when it is empty, a symmetry plane, a wall of any kind, or a fixed zero velocity
        let none = patch.kind == PatchKind::Empty
            || match gs(case, &format!("{b}.velocity")).as_str() {
                "symmetry" | "moving_wall" | "shear" | "partial_slip" | "marangoni" => true,
                "fixed" => {
                    let zero = list_f(case, &format!("{b}.velocity_value")).iter().all(|v| *v == 0.0);
                    let exprs = ["x", "y", "z"]
                        .iter()
                        .any(|c| !gs(case, &format!("{b}.velocity_expression_{c}")).is_empty());
                    zero && !exprs && gs(case, &format!("{b}.velocity_udf")).is_empty()
                }
                _ => false,
            };
        if none {
            for f in patch.faces() {
                no_flux[f] = true;
            }
        }
    }
    let mut phi = vec![0.0; nf];
    for f in 0..nf {
        if no_flux[f] || geom.empty[f] {
            continue;
        }
        let u = [0, 1, 2].map(|k| match &comps[k] {
            Some(e) => e.eval(&[fc[f][0], fc[f][1], fc[f][2], 0.0]),
            None => base[k],
        });
        phi[f] = vec3::dot(u, sf[f]);
    }
    let scale = phi.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    let (own, nei) = (geom.mesh.owner(), geom.mesh.neighbour());
    let mut net = vec![0.0; geom.n_cells()];
    for f in 0..nf {
        net[own[f]] += phi[f];
        if f < geom.n_internal {
            net[nei[f]] -= phi[f];
        }
    }
    if scale > 0.0 {
        if let Some(f) = (0..nf).find(|&f| {
            let touches = label[own[f]] != 0 || (f < geom.n_internal && label[nei[f]] != 0);
            touches && phi[f].abs() > 1e-9 * scale
        }) {
            errs.push(d(
                "initial.velocity_expression_x",
                format!(
                    "the prescribed flux through face {f} touching a solid cell is {:.3e} (largest face flux {scale:.3e}); the velocity must vanish on solids",
                    phi[f]
                ),
            ));
        }
        let worst = net.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
        if worst > 1e-6 * scale {
            errs.push(d(
                "initial.velocity",
                format!(
                    "the prescribed flux is not divergence free: the largest cell imbalance is {worst:.3e} against a largest face flux {scale:.3e} (limit 1e-6 relative)"
                ),
            ));
        }
    }
    if errs.is_empty() {
        Ok(phi)
    } else {
        Err(errs)
    }
}
