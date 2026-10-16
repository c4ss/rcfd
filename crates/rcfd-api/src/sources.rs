//! Case-level momentum sources (F-031): `sources.<name>` with a `type` of `porous`,
//! `rotating_frame` or `acceleration`, restricted to a cell zone (`all`, an axis-aligned `box`, or a
//! named mesh cell zone). The physics is in `rcfd_fv::sources`.

use crate::case::{Case, Diagnostic};
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::actuator::{ActuatorDisk, DiskProfile};
use rcfd_fv::lorentz::{Lorentz, LorentzResult, PatchElectric, PotentialMode};
use rcfd_fv::simple::BodyForce;
use rcfd_fv::sources::{self as fvs, MrfZone, RotatingFrame};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use std::sync::Mutex;

pub(crate) fn gf(c: &Case, p: &str) -> f64 {
    c.tree.get(p).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

pub(crate) fn gs(c: &Case, p: &str) -> String {
    c.tree
        .get(p)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

/// A 3-vector setting; `None` when the list is empty, `Some(Err)` when it has the wrong length.
pub(crate) fn gv(c: &Case, p: &str) -> Option<Option<Vec3>> {
    let v = c.tree.get(p).ok()?;
    let a = v.as_array()?;
    match a.len() {
        0 => None,
        3 => Some(Some([
            a[0].as_f64().unwrap_or(0.0),
            a[1].as_f64().unwrap_or(0.0),
            a[2].as_f64().unwrap_or(0.0),
        ])),
        _ => Some(None),
    }
}

/// Names of the configured sources.
pub fn names(case: &Case) -> Vec<String> {
    case.tree.names("sources").unwrap_or_default()
}

/// Is any source configured?
pub fn enabled(case: &Case) -> bool {
    !names(case).is_empty()
}

/// Settings checks (values, vector lengths, algorithm restrictions).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut bad = |path: String, message: String| {
        out.push(Diagnostic {
            path,
            line: None,
            message,
        })
    };
    let alg = gs(case, "solver.algorithm");
    for n in names(case) {
        let p = |f: &str| format!("sources.{n}.{f}");
        let kind = gs(case, &p("type"));
        if !matches!(alg.as_str(), "simple" | "simplec") {
            bad(
                p("type"),
                format!("momentum sources are available with the steady algorithms (simple, simplec) only, not with \"{alg}\""),
            );
        }
        let vec3_of = |f: &str| match gv(case, &p(f)) {
            Some(None) => Err(String::from(
                "expected 3 components [x, y, z] (or an empty list for the default)",
            )),
            Some(Some(v)) => Ok(Some(v)),
            None => Ok(None),
        };
        for d in check_zone(case, &format!("sources.{n}")) {
            bad(d.path, d.message);
        }
        match kind.as_str() {
            "porous" => {
                let model = gs(case, &p("model"));
                if model == "ergun" {
                    let e = gf(case, &p("porosity"));
                    if !(e > 0.0 && e < 1.0) {
                        bad(
                            p("porosity"),
                            "the Ergun model needs a porosity strictly between 0 and 1".into(),
                        );
                    }
                    if gf(case, &p("particle_diameter")) <= 0.0 {
                        bad(
                            p("particle_diameter"),
                            "the Ergun model needs a positive particle diameter [m]".into(),
                        );
                    }
                } else {
                    let (k, c2) = (gf(case, &p("permeability")), gf(case, &p("inertial_resistance")));
                    if k <= 0.0 && c2 <= 0.0 {
                        bad(
                            p("permeability"),
                            "darcy_forchheimer needs a positive permeability [m^2] and/or inertial_resistance [1/m]"
                                .into(),
                        );
                    }
                    if k < 0.0 {
                        bad(
                            p("permeability"),
                            "must not be negative (0 = no viscous resistance)".into(),
                        );
                    }
                    if c2 < 0.0 {
                        bad(p("inertial_resistance"), "must not be negative".into());
                    }
                }
            }
            "rotating_frame" | "mrf" => {
                match vec3_of("rotation_axis") {
                    Ok(Some(a)) if vec3::norm(a) > 0.0 => {}
                    Ok(None) => {}
                    Ok(_) => bad(
                        p("rotation_axis"),
                        "a rotating frame or MRF zone needs a non-zero rotation_axis".into(),
                    ),
                    Err(m) => bad(p("rotation_axis"), m),
                }
                if let Err(m) = vec3_of("rotation_origin") {
                    bad(p("rotation_origin"), m);
                }
                if !gf(case, &p("angular_velocity")).is_finite() {
                    bad(p("angular_velocity"), "must be finite".into());
                }
            }
            "actuator_disk" => {
                if !gf(case, &p("thrust")).is_finite() {
                    bad(p("thrust"), "must be finite".into());
                }
                let dir = vec3_of("direction");
                match &dir {
                    Ok(Some(d)) if vec3::norm(*d) > 0.0 && d.iter().all(|x| x.is_finite()) => {}
                    Ok(_) => bad(
                        p("direction"),
                        "an actuator disk needs a non-zero direction [x, y, z]".into(),
                    ),
                    Err(m) => bad(p("direction"), m.clone()),
                }
                let (centre, axis) = (vec3_of("disk_centre"), vec3_of("disk_axis"));
                for (f, r) in [("disk_centre", &centre), ("disk_axis", &axis)] {
                    if let Err(m) = r {
                        bad(p(f), m.clone());
                    }
                }
                let centre_given = matches!(centre, Ok(Some(_)));
                let axis_ok = match &axis {
                    Ok(Some(a)) => vec3::norm(*a) > 0.0,
                    _ => matches!(&dir, Ok(Some(d)) if vec3::norm(*d) > 0.0),
                };
                if matches!(&axis, Ok(Some(a)) if vec3::norm(*a) == 0.0) {
                    bad(p("disk_axis"), "must be non-zero (or empty to use direction)".into());
                }
                if gs(case, &p("profile")) == "linear_r" && !centre_given {
                    bad(
                        p("disk_centre"),
                        "profile = \"linear_r\" needs disk_centre (a point on the disk axis)".into(),
                    );
                }
                let torque = gf(case, &p("torque"));
                if !torque.is_finite() {
                    bad(p("torque"), "must be finite".into());
                } else if torque != 0.0 {
                    if !axis_ok {
                        bad(
                            p("disk_axis"),
                            "a swirl torque needs a non-zero disk_axis (or direction)".into(),
                        );
                    }
                    if !centre_given {
                        bad(
                            p("disk_centre"),
                            "a swirl torque needs disk_centre (a point on the disk axis)".into(),
                        );
                    }
                }
                let rho = gf(case, &p("density"));
                if rho < 0.0 {
                    bad(p("density"), "must not be negative (0 = physics.rho)".into());
                }
                if rho == 0.0 && gf(case, "physics.rho") <= 0.0 {
                    bad(p("density"), "set a positive density or physics.rho".into());
                }
            }
            "lorentz" => {
                match vec3_of("magnetic_field") {
                    Ok(Some(b)) if vec3::norm(b) > 0.0 && b.iter().all(|x| x.is_finite()) => {}
                    Ok(_) => bad(
                        p("magnetic_field"),
                        "a Lorentz source needs a finite non-zero magnetic_field [Bx, By, Bz] (T)".into(),
                    ),
                    Err(m) => bad(p("magnetic_field"), m),
                }
                match vec3_of("applied_electric_field") {
                    Ok(Some(e)) if !e.iter().all(|x| x.is_finite()) => {
                        bad(p("applied_electric_field"), "must be finite".into())
                    }
                    Ok(_) => {}
                    Err(m) => bad(p("applied_electric_field"), m),
                }
                let sigma = gf(case, &p("conductivity"));
                if !(sigma > 0.0 && sigma.is_finite()) {
                    bad(
                        p("conductivity"),
                        format!("set the electrical conductivity [S/m] (> 0); found {sigma} (-1 means unset)"),
                    );
                }
                if gf(case, "physics.rho") <= 0.0 {
                    bad(p("type"), "a Lorentz source needs a positive physics.rho".into());
                }
                if case
                    .tree
                    .get("electric.enabled")
                    .ok()
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    bad(
                        p("type"),
                        "a Lorentz source cannot be combined with electric.enabled (the Joule heat sigma |grad phi|^2 of the electric model is not J^2/sigma when u x B is present; ADR 0039)".into(),
                    );
                }
            }
            _ => {
                if let Err(m) = vec3_of("acceleration") {
                    bad(p("acceleration"), m);
                }
            }
        }
    }
    out
}

/// Does a Lorentz source with `potential = "solve"` exist (it reads the `boundaries.<p>.electric` keys)?
pub fn lorentz_potential(case: &Case) -> bool {
    names(case).iter().any(|n| {
        gs(case, &format!("sources.{n}.type")) == "lorentz" && gs(case, &format!("sources.{n}.potential")) == "solve"
    })
}

/// Checks of the `zone` / `box_min` / `box_max` / `cell_zone` settings under `prefix` (shared with the lumped components).
pub(crate) fn check_zone(case: &Case, prefix: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut bad = |path: String, message: String| {
        out.push(Diagnostic {
            path,
            line: None,
            message,
        })
    };
    let p = |f: &str| format!("{prefix}.{f}");
    let vec3_of = |f: &str| match gv(case, &p(f)) {
        Some(None) => Err(String::from(
            "expected 3 components [x, y, z] (or an empty list for the default)",
        )),
        Some(Some(v)) => Ok(Some(v)),
        None => Ok(None),
    };
    match gs(case, &p("zone")).as_str() {
        "box" => {
            let (lo, hi) = (vec3_of("box_min"), vec3_of("box_max"));
            for (f, r) in [("box_min", &lo), ("box_max", &hi)] {
                if let Err(m) = r {
                    bad(p(f), m.clone());
                }
            }
            match (lo, hi) {
                (Ok(Some(l)), Ok(Some(h))) => {
                    if (0..3).any(|k| l[k] >= h[k]) {
                        bad(p("box_max"), "box_max must exceed box_min in every component".into());
                    }
                }
                (Ok(_), Ok(_)) => bad(p("box_min"), "zone = \"box\" needs box_min and box_max".into()),
                _ => {}
            }
        }
        "register" if !crate::registers::names(case).contains(&gs(case, &p("register"))) => {
            bad(
                p("register"),
                format!(
                    "zone = \"register\" needs the name of a register (registers: {})",
                    crate::registers::names(case).join(", ")
                ),
            );
        }
        "cell_zone" if gs(case, &p("cell_zone")).is_empty() => {
            bad(
                p("cell_zone"),
                "zone = \"cell_zone\" needs the name of a mesh cell zone".into(),
            );
        }
        _ => {}
    }
    out
}

/// Cells of the zone under `prefix` (`all`, `box`, `cell_zone`); a located diagnostic when it is empty or unknown.
pub(crate) fn zone_cells(case: &Case, prefix: &str, geom: &FvGeometry) -> Result<Vec<bool>, Diagnostic> {
    let p = |f: &str| format!("{prefix}.{f}");
    let n_cells = geom.n_cells();
    let cc = geom.mesh.cell_centres();
    let zone: Vec<bool> = match gs(case, &p("zone")).as_str() {
        "box" => {
            let lo = gv(case, &p("box_min")).flatten().unwrap_or([0.0; 3]);
            let hi = gv(case, &p("box_max")).flatten().unwrap_or([0.0; 3]);
            cc.iter()
                .map(|c| (0..3).all(|k| c[k] >= lo[k] && c[k] <= hi[k]))
                .collect()
        }
        "register" => crate::registers::zone_from_setting(case, &p("register"), &geom.mesh)?,
        "cell_zone" => {
            let name = gs(case, &p("cell_zone"));
            match geom.mesh.cell_zone(&name) {
                Some(z) => {
                    let mut m = vec![false; n_cells];
                    for &i in &z.ids {
                        m[i] = true;
                    }
                    m
                }
                None => {
                    let known: Vec<&str> = geom.mesh.cell_zones().iter().map(|z| z.name.as_str()).collect();
                    return Err(Diagnostic {
                        path: p("cell_zone"),
                        line: None,
                        message: format!("the mesh has no cell zone '{name}' (zones: {})", known.join(", ")),
                    });
                }
            }
        }
        _ => vec![true; n_cells],
    };
    if !zone.iter().any(|&z| z) {
        return Err(Diagnostic {
            path: p("zone"),
            line: None,
            message: "the zone contains no cells of the mesh".into(),
        });
    }
    Ok(zone)
}

enum Kind {
    Porous { inv_k: f64, c2: f64 },
    Rotating(RotatingFrame),
    Mrf(MrfZone),
    Acceleration(Vec3),
    Disk(ActuatorDisk),
    Lorentz(Box<LorentzItem>),
}

/// A Lorentz source with its persistent potential field and the last evaluation (for the output fields).
struct LorentzItem {
    law: Lorentz,
    phi: Mutex<ScalarField>,
    last: Mutex<Option<LorentzResult>>,
}

struct Item {
    name: String,
    kind: Kind,
    zone: Vec<bool>,
}

/// Configured sources with their cell zones.
pub struct Sources {
    items: Vec<Item>,
    /// Kinematic viscosity used by the viscous porous term.
    nu: f64,
    /// Frame flux `(Omega x r) . S` per face of the MRF zones (`None` without an MRF source).
    pub frame_flux: Option<Vec<f64>>,
    /// Use the body-force wall-pressure construction of the SIMPLE kernel (`wall_pressure_trapezoid`): needed by the
    /// relative-velocity rotating frame and by accelerations, not by MRF zones (it forces the wall-normal pressure
    /// gradient of fixed-velocity faces, inlets included, to the body force, which distorts through-flow; ADR 0008).
    pub wall_pressure_trapezoid: bool,
}

impl Sources {
    /// Build from the case; `None` when no source is configured.
    pub fn from_case(case: &Case, geom: &FvGeometry) -> Result<Option<Sources>, Vec<Diagnostic>> {
        if !enabled(case) {
            return Ok(None);
        }
        let mut items = Vec::new();
        let mut errs = Vec::new();
        for n in names(case) {
            let p = |f: &str| format!("sources.{n}.{f}");
            let zone = match zone_cells(case, &format!("sources.{n}"), geom) {
                Ok(z) => z,
                Err(d) => {
                    errs.push(d);
                    continue;
                }
            };
            let kind = match gs(case, &p("type")).as_str() {
                "porous" => {
                    let (inv_k, c2) = if gs(case, &p("model")) == "ergun" {
                        fvs::ergun_coefficients(gf(case, &p("porosity")), gf(case, &p("particle_diameter")))
                    } else {
                        let k = gf(case, &p("permeability"));
                        (if k > 0.0 { 1.0 / k } else { 0.0 }, gf(case, &p("inertial_resistance")))
                    };
                    Kind::Porous { inv_k, c2 }
                }
                "rotating_frame" => Kind::Rotating(RotatingFrame {
                    origin: gv(case, &p("rotation_origin")).flatten().unwrap_or([0.0; 3]),
                    axis: vec3::unit(gv(case, &p("rotation_axis")).flatten().unwrap_or([0.0, 0.0, 1.0])),
                    omega: gf(case, &p("angular_velocity")),
                    centrifugal: case
                        .tree
                        .get(&p("centrifugal"))
                        .ok()
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                }),
                "mrf" => {
                    let m = MrfZone {
                        origin: gv(case, &p("rotation_origin")).flatten().unwrap_or([0.0; 3]),
                        axis: vec3::unit(gv(case, &p("rotation_axis")).flatten().unwrap_or([0.0, 0.0, 1.0])),
                        omega: gf(case, &p("angular_velocity")),
                    };
                    let worst = m.worst_non_axisymmetry(geom, &zone);
                    if worst > 1e-6 {
                        errs.push(Diagnostic {
                            path: p("zone"),
                            line: None,
                            message: format!(
                                "the boundary of an MRF zone (walls, symmetry planes and the interface to other cells) must be a surface of revolution about the rotation axis; the frame flux (Omega x r).S is {worst:.2e} of |Omega||r||S| on a zone boundary face (limit 1e-6)"
                            ),
                        });
                        continue;
                    }
                    Kind::Mrf(m)
                }
                "actuator_disk" => {
                    let direction = vec3::unit(gv(case, &p("direction")).flatten().unwrap_or([1.0, 0.0, 0.0]));
                    let d = ActuatorDisk {
                        thrust: gf(case, &p("thrust")),
                        direction,
                        profile: if gs(case, &p("profile")) == "linear_r" {
                            DiskProfile::LinearR
                        } else {
                            DiskProfile::Uniform
                        },
                        centre: gv(case, &p("disk_centre")).flatten().unwrap_or([0.0; 3]),
                        axis: gv(case, &p("disk_axis")).flatten().map_or(direction, vec3::unit),
                        torque: gf(case, &p("torque")),
                        smooth: gs(case, &p("distribution")) != "cell",
                        density: if gf(case, &p("density")) > 0.0 {
                            gf(case, &p("density"))
                        } else {
                            gf(case, "physics.rho")
                        },
                    };
                    if let Err(m) = d.cell_forces(geom, &zone) {
                        errs.push(Diagnostic {
                            path: p("zone"),
                            line: None,
                            message: m,
                        });
                        continue;
                    }
                    Kind::Disk(d)
                }
                "lorentz" => {
                    let mut patches = Vec::new();
                    for patch in geom.mesh.patches() {
                        let bp = |f: &str| format!("boundaries.{}.{f}", patch.name);
                        match gs(case, &bp("electric")).as_str() {
                            "potential" => patches.push((
                                patch.name.clone(),
                                PatchElectric::Potential(gf(case, &bp("electric_potential"))),
                            )),
                            "current" => {
                                let sf = geom.mesh.face_area_vectors();
                                let area: f64 = patch
                                    .faces()
                                    .filter(|&f| !geom.empty[f])
                                    .map(|f| vec3::norm(sf[f]))
                                    .sum();
                                if area <= 0.0 {
                                    errs.push(Diagnostic {
                                        path: bp("electric_current"),
                                        line: None,
                                        message: "the patch has no area".into(),
                                    });
                                    continue;
                                }
                                patches.push((
                                    patch.name.clone(),
                                    PatchElectric::Current(gf(case, &bp("electric_current")) / area),
                                ))
                            }
                            _ => {}
                        }
                    }
                    Kind::Lorentz(Box::new(LorentzItem {
                        law: Lorentz {
                            b: gv(case, &p("magnetic_field")).flatten().unwrap_or([0.0; 3]),
                            sigma: gf(case, &p("conductivity")),
                            e_ext: gv(case, &p("applied_electric_field")).flatten().unwrap_or([0.0; 3]),
                            rho: gf(case, "physics.rho"),
                            mode: if gs(case, &p("potential")) == "solve" {
                                PotentialMode::Solve
                            } else {
                                PotentialMode::None
                            },
                            patches,
                            zone: zone.clone(),
                        },
                        phi: Mutex::new(ScalarField::uniform("phi_mhd", geom, 0.0)),
                        last: Mutex::new(None),
                    }))
                }
                _ => Kind::Acceleration(gv(case, &p("acceleration")).flatten().unwrap_or([0.0; 3])),
            };
            items.push(Item {
                name: n.clone(),
                kind,
                zone,
            });
        }
        // MRF zones must not overlap each other or a relative-velocity rotating frame
        for (i, a) in items.iter().enumerate() {
            for b in &items[i + 1..] {
                let rot = |k: &Kind| matches!(k, Kind::Mrf(_) | Kind::Rotating(_));
                let any_mrf = matches!(a.kind, Kind::Mrf(_)) || matches!(b.kind, Kind::Mrf(_));
                if any_mrf && rot(&a.kind) && rot(&b.kind) && a.zone.iter().zip(&b.zone).any(|(x, y)| *x && *y) {
                    let culprit = if matches!(b.kind, Kind::Mrf(_)) { b } else { a };
                    errs.push(Diagnostic {
                        path: format!("sources.{}.zone", culprit.name),
                        line: None,
                        message: format!(
                            "an MRF zone must not share cells with another MRF zone or a rotating_frame source ('{}' and '{}' overlap)",
                            a.name, b.name
                        ),
                    });
                }
            }
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        let mut frame_flux: Option<Vec<f64>> = None;
        for it in &items {
            if let Kind::Mrf(m) = &it.kind {
                let off = frame_flux.get_or_insert_with(|| vec![0.0; geom.mesh.n_faces()]);
                m.add_frame_flux(geom, &it.zone, off);
            }
        }
        let wall_pressure_trapezoid = items.iter().any(|i| {
            matches!(
                i.kind,
                Kind::Rotating(_) | Kind::Acceleration(_) | Kind::Disk(_) | Kind::Lorentz(_)
            )
        });
        Ok(Some(Sources {
            wall_pressure_trapezoid,
            items,
            nu: gf(case, "physics.nu"),
            frame_flux,
        }))
    }

    /// Body acceleration and implicit sink for the current velocity field.
    pub fn evaluate(&self, geom: &FvGeometry, u: &VectorField) -> (Option<BodyForce>, Option<Vec<f64>>) {
        let ub = u.boundary_values(geom);
        let mut force: Option<BodyForce> = None;
        let mut sink: Option<Vec<f64>> = None;
        for it in &self.items {
            match &it.kind {
                Kind::Porous { inv_k, c2 } => {
                    let s = sink.get_or_insert_with(|| vec![0.0; geom.n_cells()]);
                    fvs::porous_sink(self.nu, *inv_k, *c2, &u.values, &it.zone, s);
                }
                Kind::Rotating(r) => {
                    let f = force.get_or_insert_with(|| BodyForce::uniform(geom, [0.0; 3]));
                    r.add_force(geom, &u.values, &ub, &it.zone, f);
                }
                Kind::Mrf(m) => {
                    let f = force.get_or_insert_with(|| BodyForce::uniform(geom, [0.0; 3]));
                    m.add_force(geom, &u.values, &ub, &it.zone, f);
                }
                Kind::Acceleration(a) => {
                    let f = force.get_or_insert_with(|| BodyForce::uniform(geom, [0.0; 3]));
                    fvs::add_constant_acceleration(geom, *a, &it.zone, f);
                }
                Kind::Disk(d) => {
                    let f = force.get_or_insert_with(|| BodyForce::uniform(geom, [0.0; 3]));
                    // the zone was checked for force-carrying cells when the source was built
                    let _ = d.add_force(geom, &it.zone, f);
                }
                Kind::Lorentz(l) => {
                    let mut phi = l.phi.lock().expect("lorentz potential lock");
                    let r = l.law.evaluate(geom, &mut phi, u);
                    let f = force.get_or_insert_with(|| BodyForce::uniform(geom, [0.0; 3]));
                    f.add(&r.body_force(geom));
                    let s = sink.get_or_insert_with(|| vec![0.0; geom.n_cells()]);
                    for (a, b) in s.iter_mut().zip(&r.sink) {
                        *a += b;
                    }
                    *l.last.lock().expect("lorentz result lock") = Some(r);
                }
            }
        }
        (force, sink)
    }

    /// Output field of the Lorentz sources (`lorentz_force_x/y/z` in N/m^3, `current_density_x/y/z` in A/m^2) from the last
    /// evaluation, summed over the sources; `None` when the name is not one of them or no Lorentz source has been evaluated.
    pub fn lorentz_field(&self, name: &str) -> Option<Vec<f64>> {
        let (force, comp) = match name {
            "lorentz_force_x" => (true, 0),
            "lorentz_force_y" => (true, 1),
            "lorentz_force_z" => (true, 2),
            "current_density_x" => (false, 0),
            "current_density_y" => (false, 1),
            "current_density_z" => (false, 2),
            _ => return None,
        };
        let mut out: Option<Vec<f64>> = None;
        for it in &self.items {
            if let Kind::Lorentz(l) = &it.kind {
                let guard = l.last.lock().expect("lorentz result lock");
                let Some(r) = guard.as_ref() else { continue };
                let v = if force { &r.force } else { &r.current };
                let o = out.get_or_insert_with(|| vec![0.0; v.len()]);
                for (a, b) in o.iter_mut().zip(v) {
                    *a += b[comp];
                }
            }
        }
        out
    }
}
