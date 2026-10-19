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
use rcfd_mesh::fan_cgrid::{build_cgrid_mesh, CGridSpec};
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
    AxialFan::linear(
        get_i(case, "mesh.fan.n_blades").max(0) as usize,
        f("r_hub"),
        f("r_tip"),
        f("tip_clearance"),
        f("axial_extent"),
        f("z_le_root"),
        family,
        st("hub"),
        st("tip"),
    )
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
    let fan = fan_of(case);
    let to_diag = |errs: Vec<rcfd_mesh::fan::FanError>| -> Vec<Diagnostic> {
        errs.into_iter()
            .map(|e| Diagnostic {
                path: format!("mesh.fan.{}", e.path),
                line: None,
                message: e.message,
            })
            .collect()
    };
    if get_s(case, "mesh.fan.topology") == "c_grid" {
        return build_cgrid_mesh(&fan, &cgrid_spec_of(case))
            .map(|s| s.mesh)
            .map_err(to_diag);
    }
    let spec = spec_of(case);
    build_sector_mesh(&fan, &spec).map(|s| s.mesh).map_err(to_diag)
}
