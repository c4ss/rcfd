//! Case-level momentum sources (F-031): `sources.<name>` with a `type` of `porous`,
//! `rotating_frame` or `acceleration`, restricted to a cell zone (`all`, an axis-aligned `box`, or a
//! named mesh cell zone). The physics is in `rcfd_fv::sources`.

use crate::case::{Case, Diagnostic};
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::simple::BodyForce;
use rcfd_fv::sources::{self as fvs, RotatingFrame};
use rcfd_fv::{FvGeometry, VectorField};

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

/// A 3-vector setting; `None` when the list is empty, `Some(Err)` when it has the wrong length.
fn gv(c: &Case, p: &str) -> Option<Option<Vec3>> {
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
            "cell_zone" if gs(case, &p("cell_zone")).is_empty() => {
                bad(
                    p("cell_zone"),
                    "zone = \"cell_zone\" needs the name of a mesh cell zone".into(),
                );
            }
            _ => {}
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
            "rotating_frame" => {
                match vec3_of("rotation_axis") {
                    Ok(Some(a)) if vec3::norm(a) > 0.0 => {}
                    Ok(None) => {}
                    Ok(_) => bad(
                        p("rotation_axis"),
                        "a rotating frame needs a non-zero rotation_axis".into(),
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
            _ => {
                if let Err(m) = vec3_of("acceleration") {
                    bad(p("acceleration"), m);
                }
            }
        }
    }
    out
}

enum Kind {
    Porous { inv_k: f64, c2: f64 },
    Rotating(RotatingFrame),
    Acceleration(Vec3),
}

struct Item {
    kind: Kind,
    zone: Vec<bool>,
}

/// Configured sources with their cell zones.
pub struct Sources {
    items: Vec<Item>,
    /// Kinematic viscosity used by the viscous porous term.
    nu: f64,
}

impl Sources {
    /// Build from the case; `None` when no source is configured.
    pub fn from_case(case: &Case, geom: &FvGeometry) -> Result<Option<Sources>, Vec<Diagnostic>> {
        if !enabled(case) {
            return Ok(None);
        }
        let mut items = Vec::new();
        let mut errs = Vec::new();
        let n_cells = geom.n_cells();
        let cc = geom.mesh.cell_centres();
        for n in names(case) {
            let p = |f: &str| format!("sources.{n}.{f}");
            let zone: Vec<bool> = match gs(case, &p("zone")).as_str() {
                "box" => {
                    let lo = gv(case, &p("box_min")).flatten().unwrap_or([0.0; 3]);
                    let hi = gv(case, &p("box_max")).flatten().unwrap_or([0.0; 3]);
                    cc.iter()
                        .map(|c| (0..3).all(|k| c[k] >= lo[k] && c[k] <= hi[k]))
                        .collect()
                }
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
                            errs.push(Diagnostic {
                                path: p("cell_zone"),
                                line: None,
                                message: format!("the mesh has no cell zone '{name}' (zones: {})", known.join(", ")),
                            });
                            continue;
                        }
                    }
                }
                _ => vec![true; n_cells],
            };
            if !zone.iter().any(|&z| z) {
                errs.push(Diagnostic {
                    path: p("zone"),
                    line: None,
                    message: "the zone contains no cells of the mesh".into(),
                });
                continue;
            }
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
                _ => Kind::Acceleration(gv(case, &p("acceleration")).flatten().unwrap_or([0.0; 3])),
            };
            items.push(Item { kind, zone });
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        Ok(Some(Sources {
            items,
            nu: gf(case, "physics.nu"),
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
                Kind::Acceleration(a) => {
                    let f = force.get_or_insert_with(|| BodyForce::uniform(geom, [0.0; 3]));
                    fvs::add_constant_acceleration(geom, *a, &it.zone, f);
                }
            }
        }
        (force, sink)
    }
}
