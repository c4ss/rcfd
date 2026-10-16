//! Per-region model assignment (F-216, ADR 0036): `regions.<name>` with a cell zone and `turbulence = "model" | "laminar"`.
//!
//! A laminar region switches the eddy viscosity off in the momentum equation: every face that touches a cell of the region (boundary
//! faces by their owner cell) gets the molecular viscosity `physics.nu`. The turbulence transport equations are still solved there.
//! Only this one assignment is implemented; per-region energy, radiation and species models are named gaps of the roadmap item.

use crate::case::{Case, Diagnostic};
use rcfd_core::settings::Schema;
use rcfd_fv::FvGeometry;

fn gs(c: &Case, p: &str) -> String {
    c.tree
        .get(p)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

/// Names of the configured regions.
pub fn names(case: &Case) -> Vec<String> {
    case.tree.names("regions").unwrap_or_default()
}

/// Schema of the `regions` collection.
pub fn schema() -> Schema {
    let vec3 = |doc: &str| Schema::list(Schema::float(0.0)).doc(doc);
    Schema::named(
        Schema::object()
            .doc("A region: a cell zone with its own model assignment (ADR 0036)")
            .field(
                "zone",
                Schema::enumeration("all", &["all", "box", "cell_zone", "register"])
                    .doc("Cells of the region: the whole mesh, the cells whose centres lie in [box_min, box_max], a named mesh cell zone or a cell register"),
            )
            .field("register", Schema::string("").doc("Name of the register for zone = \"register\""))
            .field("box_min", vec3("Lower corner [m] of the box zone"))
            .field("box_max", vec3("Upper corner [m] of the box zone"))
            .field("cell_zone", Schema::string("").doc("Name of the mesh cell zone"))
            .field(
                "turbulence",
                Schema::enumeration("model", &["model", "laminar"]).doc(
                    "model: the global turbulence.model acts in the region (default); laminar: the momentum equation uses the molecular viscosity on every face touching the region (the turbulence equations are still solved there)",
                ),
            ),
    )
    .doc("Per-region model assignment (turbulence on/off in a cell zone); steady and transient incompressible runs")
}

/// Settings checks.
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for n in names(case) {
        let p = |f: &str| format!("regions.{n}.{f}");
        out.extend(crate::sources::check_zone(case, &format!("regions.{n}")));
        if gs(case, &p("turbulence")) != "laminar" {
            continue;
        }
        let mut bad = |path: String, message: &str| {
            out.push(Diagnostic {
                path,
                line: None,
                message: message.into(),
            })
        };
        if !crate::turb::enabled(case) {
            bad(
                p("turbulence"),
                "a laminar region needs a turbulence model to switch off (turbulence.model is laminar)",
            );
        } else if crate::turb::is_rsm(case) || gs(case, "turbulence.model") == "plugin" {
            bad(
                p("turbulence"),
                "per-region laminar assignment is not available with the Reynolds stress model or plugin models (their momentum coupling is not a face viscosity)",
            );
        }
        if crate::rheology::enabled(case)
            || case
                .tree
                .get("properties.viscosity.law")
                .ok()
                .and_then(|v| v.as_str().map(|s| s != "constant"))
                .unwrap_or(false)
        {
            bad(
                p("turbulence"),
                "cannot be combined with rheology or a temperature-dependent viscosity (both set the face viscosity)",
            );
        }
    }
    out
}

/// Union of the cells of all laminar regions; `None` when there is none.
pub fn laminar_cells(case: &Case, geom: &FvGeometry) -> Result<Option<Vec<bool>>, Diagnostic> {
    let mut mask: Option<Vec<bool>> = None;
    for n in names(case) {
        if gs(case, &format!("regions.{n}.turbulence")) != "laminar" {
            continue;
        }
        let zone = crate::sources::zone_cells(case, &format!("regions.{n}"), geom)?;
        let m = mask.get_or_insert_with(|| vec![false; zone.len()]);
        for (a, b) in m.iter_mut().zip(&zone) {
            *a |= *b;
        }
    }
    Ok(mask)
}

/// Set the face viscosity to the molecular value `nu` on every face that touches a cell of `cells` (boundary faces: the owner cell).
pub fn mask_faces(geom: &FvGeometry, cells: &[bool], nu: f64, nu_face: &mut [f64]) {
    let mesh = &geom.mesh;
    for (f, v) in nu_face.iter_mut().enumerate() {
        let o = mesh.owner()[f];
        let touches = cells[o] || (f < geom.n_internal && cells[mesh.neighbour()[f]]);
        if touches {
            *v = nu;
        }
    }
}
