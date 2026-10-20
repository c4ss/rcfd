//! Case hook of the parametric axial-fan periodic-sector mesh (`mesh.generator = "axial_fan_sector"`, ADR 0065).
//!
//! The settings live in the nested section `mesh.fan`. The blade is described by a hub and a tip station with a linear
//! variation in between (the library [`rcfd_mesh::fan`] accepts any number of stations). Angles are given in degrees.
//! Patches of the mesh: `inlet`, `outlet`, `hub`, `shroud`, `blade`, and the rotational pair `seam_lo` / `seam_hi`
//! (cyclic, angle 2 pi / n_blades, applied by the mesh). The cell zone `rotor` covers all cells.
//!
//! `mesh.fan.topology = "h_slit"` (default) is the lower-fidelity hex mesh with blunt (axially truncated) blade edges
//! (ADR 0065). `"c_grid"` selects the body-fitted C-grid passage mesh with the true blade edges (ADR 0068,
//! [`rcfd_mesh::fan_cgrid`]); it uses `n_up`, `n_chord`, `n_down`, `n_side`, `n_span`, `n_gap`, `n_bl`, `growth` and the
//! first-cell controls like the default topology, plus `n_nose`, `n_core` and `front_gap`; `n_mid`, `trunc_le` and `trunc_te`
//! are ignored there. Neither mesh is validated for accurate fan performance prediction (docs/meshing.md).

use crate::case::{Case, Diagnostic};
use crate::sim::{get_b, get_f, get_i, get_s};
use rcfd_core::settings::Schema;
use rcfd_core::units::Dimension;
use rcfd_mesh::fan::{AxialFan, SectionFamily, Station};
use rcfd_mesh::fan_cgrid::{build_cgrid_mesh_with, CGridOptions, CGridSpec};
use rcfd_mesh::fan_mesh::{build_sector_mesh, FirstCell, SectorSpec};
use rcfd_mesh::Mesh;

/// Schema of the `mesh.fan` section.
pub fn schema() -> Schema {
    let len = |v: f64, doc: &str| Schema::float(v).at_least(0.0).dim(Dimension::LENGTH).doc(doc);
    let ang = |v: f64, doc: &str| Schema::float(v).range(-85.0, 85.0).doc(doc);
    let cnt = |v: i64, lo: f64, doc: &str| Schema::int(v).range(lo, 100000.0).doc(doc);
    Schema::object()
        .doc("Axial-fan periodic single-passage sector (mesh.generator = axial_fan_sector, ADR 0065): hub and tip station of a blade with linear variation in between, and the hex mesh controls. Lower-fidelity mesh with blunt axially truncated blade edges; not validated for accurate fan performance prediction")
        .field("n_blades", cnt(5, 2.0, "Blade count N; the sector angle is 2 pi / N"))
        .field("r_hub", len(0.1, "Hub radius [m]"))
        .field("r_tip", len(0.3, "Blade tip radius [m]"))
        .field("tip_clearance", len(0.01, "Tip clearance [m]; the shroud radius is r_tip + tip_clearance"))
        .field("axial_extent", len(0.7, "Axial length of the domain [m] (inlet at z = 0)"))
        .field("z_le_root", len(0.3, "Axial position of the hub leading edge [m]"))
        .field(
            "family",
            Schema::enumeration("naca4", &["naca4", "circular_arc"])
                .doc("Section family: NACA 4-digit (closed trailing edge) or double circular arc"),
        )
        .field("hub_chord", len(0.10, "Chord at the hub [m]"))
        .field("tip_chord", len(0.07, "Chord at the tip [m]"))
        .field("hub_stagger", ang(35.0, "Stagger at the hub [deg]: angle of the chord from the axis, positive turns the trailing edge toward -theta"))
        .field("tip_stagger", ang(20.0, "Stagger at the tip [deg]"))
        .field("hub_camber", Schema::float(0.04).range(0.0, 0.2).doc("Maximum camber at the hub (chord fraction)"))
        .field("tip_camber", Schema::float(0.04).range(0.0, 0.2).doc("Maximum camber at the tip (chord fraction)"))
        .field("camber_pos", Schema::float(0.4).range(0.05, 0.95).doc("Position of the maximum camber (chord fraction; NACA 4-digit only)"))
        .field("hub_thickness", Schema::float(0.10).range(0.005, 0.4).doc("Maximum thickness at the hub (chord fraction)"))
        .field("tip_thickness", Schema::float(0.06).range(0.005, 0.4).doc("Maximum thickness at the tip (chord fraction)"))
        .field("hub_sweep", Schema::float(0.0).dim(Dimension::LENGTH).doc("Axial shift of the leading edge at the hub [m]"))
        .field("tip_sweep", Schema::float(0.01).dim(Dimension::LENGTH).doc("Axial shift of the leading edge at the tip [m]"))
        .field("hub_lean", Schema::float(0.0).dim(Dimension::LENGTH).doc("Tangential shift of the leading edge at the hub [m]"))
        .field("tip_lean", Schema::float(0.005).dim(Dimension::LENGTH).doc("Tangential shift of the leading edge at the tip [m]"))
        .field("n_up", cnt(22, 1.0, "Axial cells from the inlet to the leading-edge plane"))
        .field("n_chord", cnt(24, 1.0, "Axial cells along the truncated chord"))
        .field("n_down", cnt(22, 1.0, "Axial cells from the trailing-edge plane to the outlet"))
        .field("n_side", cnt(20, 1.0, "Pitchwise cells from each seam to the blade (including the boundary layers)"))
        .field("n_mid", cnt(3, 1.0, "Pitchwise cells across the blunt edge faces"))
        .field("n_span", cnt(30, 1.0, "Radial cells from the hub to the blade tip"))
        .field("n_gap", cnt(18, 1.0, "Radial cells in the tip gap"))
        .field("n_bl", cnt(5, 0.0, "Boundary-layer cells with exact geometric growth on every wall"))
        .field("growth", Schema::float(1.2).range(1.0, 2.0).doc("Growth ratio of the boundary-layer cells"))
        .field("first_cell_height", Schema::float(0.0).at_least(0.0).dim(Dimension::LENGTH).doc("First cell height [m]; 0 = take it from y_plus"))
        .field("y_plus", Schema::float(30.0).at_least(0.0).doc("Target y+ of the first cell centre when first_cell_height = 0 (flat-plate estimate at the mid-span chord)"))
        .field("ref_speed", Schema::float(90.0).at_least(0.0).doc("Relative speed [m/s] for the y+ estimate"))
        .field("nu", Schema::float(1.5e-5).at_least(0.0).doc("Kinematic viscosity [m^2/s] for the y+ estimate"))
        .field("trunc_le", Schema::float(0.02).range(0.0, 0.3).doc("Leading-edge truncation (chord fraction)"))
        .field("trunc_te", Schema::float(0.01).range(0.0, 0.3).doc("Trailing-edge truncation (chord fraction)"))
        .field(
            "topology",
            Schema::enumeration("h_slit", &["h_slit", "c_grid"])
                .doc("Mesh topology: h_slit (ADR 0065, blunt truncated blade edges) or c_grid (ADR 0068, body-fitted C-grid with the true blade edges; ignores n_mid, trunc_le and trunc_te)"),
        )
        .field("n_nose", cnt(8, 1.0, "c_grid only: cells from the leading edge to each seam corner (n_nose + 2 must not exceed n_chord)"))
        .field("n_core", cnt(2, 1.0, "c_grid only: cells across the blade thickness in the tip gap above the blade"))
        .field("front_gap", Schema::float(0.3).at_least(0.0).doc("c_grid only: axial distance of the front edge of the leading-edge fan ahead of the leading edge, in local chords"))
        .field("include_blade", Schema::boolean(true).doc("false leaves the blade footprint filled: an empty passage with the same nodes (verification)"))
        .field(
            "c_grid_construction",
            Schema::enumeration("pursuit", &["pursuit", "smoothed", "camber_seam", "camber_seam_h"])
                .doc("c_grid only: pursuit (ADR 0068, the default), smoothed (ADR 0072: elliptic in-plane smoothing with periodic seams and the radial remap of the far grid; better orthogonality, more robust against the controls) or camber_seam (ADR 0079: smoothed, plus seams that follow the blade where straight seams leave no margin, a fitted tip-gap block, the accepted ADR 0079 quality changes and a checked control range) or camber_seam_h (ADR 0083: camber_seam with the changes accepted by ADR 0083: a post-smoothing untangling and lean-following wall stacks)"),
        )
        .field("station_radius", flist("Optional radial stations [m] of the blade (ADR 0075): when this list is not empty, the station_* lists (all of the same length, first radius r_hub, last radius r_tip, strictly increasing) replace the hub_* / tip_* values and the blade varies linearly between neighbouring stations"))
        .field("station_chord", flist("Chord [m] at each station_radius"))
        .field("station_stagger", flist("Stagger [deg] at each station_radius (angle of the chord from the axis, as hub_stagger)"))
        .field("station_camber", flist("Maximum camber (chord fraction) at each station_radius"))
        .field("station_camber_pos", flist("Position of the maximum camber (chord fraction) at each station_radius; empty = camber_pos everywhere"))
        .field("station_thickness", flist("Maximum thickness (chord fraction) at each station_radius"))
        .field("station_sweep", flist("Axial shift [m] of the leading edge at each station_radius; empty = 0"))
        .field("station_lean", flist("Tangential shift [m] of the leading edge at each station_radius; empty = 0"))
}

fn flist(doc: &str) -> Schema {
    Schema::list(Schema::float(0.0)).doc(doc)
}

/// The numbers of the list `mesh.fan.<key>` (empty when unset).
fn list_of(case: &Case, key: &str) -> Vec<f64> {
    case.tree
        .get(&format!("mesh.fan.{key}"))
        .ok()
        .as_ref()
        .and_then(|v| {
            v.as_array()
                .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect())
        })
        .unwrap_or_default()
}

/// Lists of the optional station form, in the order of [`Station`]'s fields after the radius.
const STATION_LISTS: [&str; 7] = [
    "station_chord",
    "station_stagger",
    "station_camber",
    "station_camber_pos",
    "station_thickness",
    "station_sweep",
    "station_lean",
];

/// Located problems of the optional station lists (ADR 0075 H2): every list given must have the length of `station_radius`
/// (`station_camber_pos`, `station_sweep` and `station_lean` may be empty), the others are required.
pub fn station_problems(case: &Case) -> Vec<Diagnostic> {
    let r = list_of(case, "station_radius");
    let mut out = Vec::new();
    if r.is_empty() {
        for k in STATION_LISTS {
            if !list_of(case, k).is_empty() {
                out.push(Diagnostic {
                    path: format!("mesh.fan.{k}"),
                    line: None,
                    message: "station lists need mesh.fan.station_radius".into(),
                });
            }
        }
        return out;
    }
    for k in STATION_LISTS {
        let v = list_of(case, k);
        let optional = matches!(k, "station_camber_pos" | "station_sweep" | "station_lean");
        if (v.is_empty() && !optional) || (!v.is_empty() && v.len() != r.len()) {
            out.push(Diagnostic {
                path: format!("mesh.fan.{k}"),
                line: None,
                message: format!("needs {} values (one per station_radius), has {}", r.len(), v.len()),
            });
        }
    }
    out
}

/// The stations of the optional list form, or `None` when `station_radius` is empty.
fn stations_of(case: &Case) -> Option<Vec<Station>> {
    let r = list_of(case, "station_radius");
    if r.is_empty() {
        return None;
    }
    let get = |k: &str, i: usize, default: f64| list_of(case, k).get(i).copied().unwrap_or(default);
    let cp = get_f(case, "mesh.fan.camber_pos");
    Some(
        (0..r.len())
            .map(|i| Station {
                radius: r[i],
                chord: get("station_chord", i, f64::NAN),
                stagger: get("station_stagger", i, f64::NAN).to_radians(),
                camber: get("station_camber", i, f64::NAN),
                camber_pos: get("station_camber_pos", i, cp),
                thickness: get("station_thickness", i, f64::NAN),
                sweep: get("station_sweep", i, 0.0),
                lean: get("station_lean", i, 0.0),
            })
            .collect(),
    )
}

/// Library path `stations[i].field` as the case path `station_field[i]`.
fn station_path(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("stations[") {
        if let Some((i, field)) = rest.split_once("].") {
            return format!("station_{field}[{i}]");
        }
    }
    path.to_string()
}

/// The fan described by `mesh.fan`.
pub fn fan_of(case: &Case) -> AxialFan {
    let f = |k: &str| get_f(case, &format!("mesh.fan.{k}"));
    let family = if get_s(case, "mesh.fan.family") == "circular_arc" {
        SectionFamily::CircularArc
    } else {
        SectionFamily::Naca4
    };
    let st = |pre: &str| Station {
        radius: 0.0,
        chord: f(&format!("{pre}_chord")),
        stagger: f(&format!("{pre}_stagger")).to_radians(),
        camber: f(&format!("{pre}_camber")),
        camber_pos: f("camber_pos"),
        thickness: f(&format!("{pre}_thickness")),
        sweep: f(&format!("{pre}_sweep")),
        lean: f(&format!("{pre}_lean")),
    };
    let mut fan = AxialFan::linear(
        get_i(case, "mesh.fan.n_blades").max(0) as usize,
        f("r_hub"),
        f("r_tip"),
        f("tip_clearance"),
        f("axial_extent"),
        f("z_le_root"),
        family,
        st("hub"),
        st("tip"),
    );
    if let Some(stations) = stations_of(case) {
        fan.stations = stations;
    }
    fan
}

/// The mesh controls described by `mesh.fan`.
pub fn spec_of(case: &Case) -> SectorSpec {
    let n = |k: &str| get_i(case, &format!("mesh.fan.{k}")).max(0) as usize;
    let f = |k: &str| get_f(case, &format!("mesh.fan.{k}"));
    let h1 = f("first_cell_height");
    SectorSpec {
        n_up: n("n_up"),
        n_chord: n("n_chord"),
        n_down: n("n_down"),
        n_side: n("n_side"),
        n_mid: n("n_mid"),
        n_span: n("n_span"),
        n_gap: n("n_gap"),
        n_bl: n("n_bl"),
        growth: f("growth"),
        first_cell: if h1 > 0.0 {
            FirstCell::Height(h1)
        } else {
            FirstCell::YPlus {
                y_plus: f("y_plus"),
                speed: f("ref_speed"),
                nu: f("nu"),
            }
        },
        trunc_le: f("trunc_le"),
        trunc_te: f("trunc_te"),
        include_blade: get_b(case, "mesh.fan.include_blade"),
    }
}

/// The C-grid controls described by `mesh.fan`.
pub fn cgrid_spec_of(case: &Case) -> CGridSpec {
    let n = |k: &str| get_i(case, &format!("mesh.fan.{k}")).max(0) as usize;
    let f = |k: &str| get_f(case, &format!("mesh.fan.{k}"));
    let h = spec_of(case);
    CGridSpec {
        n_up: n("n_up"),
        n_chord: n("n_chord"),
        n_down: n("n_down"),
        n_side: n("n_side"),
        n_nose: n("n_nose"),
        n_core: n("n_core"),
        n_span: n("n_span"),
        n_gap: n("n_gap"),
        n_bl: n("n_bl"),
        growth: f("growth"),
        first_cell: h.first_cell,
        front_gap: f("front_gap"),
        include_blade: h.include_blade,
    }
}

/// Build the sector mesh of the case; problems are reported with their `mesh.fan.*` path where the library names one.
pub fn build(case: &Case) -> std::result::Result<Mesh, Vec<Diagnostic>> {
    let problems = station_problems(case);
    if !problems.is_empty() {
        return Err(problems);
    }
    let fan = fan_of(case);
    let listed = !list_of(case, "station_radius").is_empty();
    let to_diag = |errs: Vec<rcfd_mesh::fan::FanError>| -> Vec<Diagnostic> {
        errs.into_iter()
            .map(|e| Diagnostic {
                path: format!("mesh.fan.{}", if listed { station_path(&e.path) } else { e.path }),
                line: None,
                message: e.message,
            })
            .collect()
    };
    if get_s(case, "mesh.fan.topology") == "c_grid" {
        let opts = match get_s(case, "mesh.fan.c_grid_construction").as_str() {
            "smoothed" => CGridOptions {
                radial_remap: true,
                smoothing: true,
                ..CGridOptions::default()
            },
            "camber_seam" => CGridOptions::camber_seam_construction(),
            "camber_seam_h" => CGridOptions::camber_seam_h_construction(),
            _ => CGridOptions::default(),
        };
        return build_cgrid_mesh_with(&fan, &cgrid_spec_of(case), &opts)
            .map(|s| s.mesh)
            .map_err(to_diag);
    }
    let spec = spec_of(case);
    build_sector_mesh(&fan, &spec).map(|s| s.mesh).map_err(to_diag)
}
