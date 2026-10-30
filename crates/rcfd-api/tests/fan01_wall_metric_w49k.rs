//! Job 49K (ADR 0174): the ADR 0171 wall-cell metric `aspect x tan(non-orthogonality)` (the SV302 metric of ADR 0172) on the FAN-01
//! meshes of ADRs 0086 and 0110: is the FAN-01 stall the trailing-edge mechanism of ADR 0171? Mesh properties and `Simulation::new` only;
//! no solver iteration. Everything below was registered in ADR 0174 before any computation. The mesh and case construction is
//! `fan01_stall_w43c.rs`'s, unchanged. The report: `cargo test --release -p rcfd-api --test fan01_wall_metric_w49k -- --nocapture`;
//! with `W49K_OUT=<file>` it is also written as JSON.

use rcfd_api::case::Case;
use rcfd_api::rules;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::OnceLock;

const R_HUB: f64 = 0.124;
const R_DUCT: f64 = 0.250;
const RHO: f64 = 1.2; // ASSUMED (ADR 0075), UNVERIFIED
const Q_DESIGN: f64 = 1.4032;

fn omega() -> f64 {
    1486.0 * 2.0 * PI / 60.0
}

fn annulus_area() -> f64 {
    PI * (R_DUCT * R_DUCT - R_HUB * R_HUB)
}

/// The 16 registered FAN-01 sections (ADR 0075 amendment 1).
fn sections() -> Vec<Vec<f64>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../validation-data/reference/w34d_fan01_cad_sections.csv");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let rows: Vec<Vec<f64>> = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("r_mm"))
        .map(|l| {
            l.split(',')
                .take(8)
                .map(|v| v.trim().parse::<f64>().unwrap())
                .collect::<Vec<f64>>()
        })
        .filter(|r| r[0] != 247.0)
        .collect();
    assert_eq!(rows.len(), 16);
    rows
}

fn list(rows: &[Vec<f64>], f: impl Fn(&[f64]) -> f64) -> String {
    let v: Vec<String> = rows.iter().map(|s| format!("{:?}", f(s))).collect();
    format!("[{}]", v.join(", "))
}

/// Counts (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) of the ADR 0086 family.
type Counts = (usize, usize, usize, usize, usize, usize, usize);
const F1: Counts = (12, 24, 12, 15, 6, 21, 10);
const F2: Counts = (8, 17, 8, 11, 4, 15, 7);
const F3: Counts = (6, 12, 6, 8, 3, 10, 5);

/// The ADR 0086 mesh (construction `camber_seam_h`), as `fan01_stall_w43c.rs`.
fn mesh_toml(c: Counts, blade: bool) -> String {
    let rows = sections();
    let (x0, t0) = (rows[0][6], rows[0][7]);
    let ref_speed = (Q_DESIGN / annulus_area()).hypot(omega() * 0.186);
    let (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) = c;
    format!(
        "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\nn_blades = 9\nr_hub = 0.124\nr_tip = 0.2475\ntip_clearance = 0.0025\naxial_extent = 0.4\nz_le_root = 0.1\nfamily = \"naca4\"\n\
station_radius = {}\nstation_chord = {}\nstation_stagger = {}\nstation_camber = {}\nstation_camber_pos = {}\nstation_thickness = {}\nstation_sweep = {}\nstation_lean = {}\n\
topology = \"c_grid\"\nc_grid_construction = \"camber_seam_h\"\nn_up = {n_up}\nn_chord = {n_chord}\nn_down = {n_down}\nn_side = {n_side}\nn_nose = {n_nose}\nn_core = 1\nfront_gap = 0.3\nn_span = {n_span}\nn_gap = {n_gap}\nn_bl = 2\ngrowth = 1.2\n\
first_cell_height = 0.0\ny_plus = 30.0\nref_speed = {ref_speed:?}\nnu = 1.5e-5\n{}",
        list(&rows, |s| s[0] * 1e-3),
        list(&rows, |s| s[1] * 1e-3),
        list(&rows, |s| s[2]),
        list(&rows, |s| s[3]),
        list(&rows, |s| s[4]),
        list(&rows, |s| s[5]),
        list(&rows, |s| (s[6] - x0) * 1e-3),
        list(&rows, |s| s[0] * 1e-3 * (s[7] - t0).to_radians()),
        if blade { "" } else { "include_blade = false\n" },
    )
}

/// The ADR 0086 case (SST, the fixed-pressure outlet of `fan01_stall_w43c.rs::case_toml`), with `limiter` as
/// `solver.linear_upwind_limiter`.
fn case_toml(c: Counts, blade: bool, limiter: &str) -> String {
    let w = omega();
    let u_in = Q_DESIGN / annulus_area();
    let k = 1.5 * (0.01 * u_in) * (0.01 * u_in);
    let om = k.sqrt() / (0.09f64.powf(0.25) * 0.0176);
    let casing = format!(
        "velocity = \"moving_wall\"\nwall_rotation_rate = {:?}\nwall_velocity_projection = \"tangential\"\nturbulence = \"wall\"\n",
        -w
    );
    let hub = if blade {
        "velocity = \"fixed\"\n".to_string()
    } else {
        casing.clone()
    };
    let blade_t = if blade {
        "[boundaries.blade]\nvelocity = \"fixed\"\n"
    } else {
        ""
    };
    let rel = format!("velocity_expression_x = \"{w:?} [1/s] * y\"\nvelocity_expression_y = \"-{w:?} [1/s] * x\"\n");
    format!(
        r#"{}
[physics]
nu = 1.5e-5
rho = {RHO:?}
[turbulence]
model = "k_omega_sst"
[initial]
velocity = [0.0, 0.0, {u_in:?}]
{rel}[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.0, 0.0, {u_in:?}]
{rel}turbulence = "inlet"
turbulence_spec = "values"
k_value = {k:?}
omega_value = {om:?}
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.hub]
{hub}[boundaries.shroud]
{casing}[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = {w:?}
centrifugal = true
{blade_t}[solver]
algorithm = "simplec"
convection = "linear_upwind"
linear_upwind_limiter = "{limiter}"
max_iterations = 100000
tolerance_u = 1e-5
tolerance_p = 1e-5
"#,
        mesh_toml(c, blade)
    )
}

// ---------------------------------------------------------------- per-cell quantities (ADR 0174 definitions)

/// One mesh of the family with its per-cell metric.
struct MeshData {
    n: usize,
    /// `m_c`, the formula of `rules::wall_cell_risk` on every cell.
    m: Vec<f64>,
    aspect: Vec<f64>,
    /// largest face `non_orthogonality` of `compute_metrics` (the metric's angle) [deg]
    theta: Vec<f64>,
    /// the `fan01_stall_w43c.rs::cell_attributes` angle (owner-neighbour vector against the face normal) [deg]
    harness_angle: Vec<f64>,
    tags: Vec<String>,
    wall: Vec<bool>,
    r: Vec<f64>,
    z: Vec<f64>,
    x: Vec<vec3::Vec3>,
    /// SV302 diagnostics of the unlimited and of the Barth-Jespersen simulation
    sv302_unlimited: usize,
    sv302_bj: usize,
    risk: Option<rules::WallRisk>,
}

fn build(c: Counts, blade: bool) -> MeshData {
    let unl = Case::parse_toml(&case_toml(c, blade, "none")).unwrap();
    let sim = Simulation::new(&unl).unwrap();
    let bj = Simulation::new(&Case::parse_toml(&case_toml(c, blade, "barth_jespersen")).unwrap()).unwrap();
    let count = |s: &Simulation| {
        rules::dynamic_setup_diagnostics(s)
            .iter()
            .filter(|d| d.id == "SV302")
            .count()
    };
    let (sv302_unlimited, sv302_bj) = (count(&sim), count(&bj));
    drop(bj);
    let g = &sim.geom;
    let mesh = &g.mesh;
    let n = mesh.n_cells();
    let q = rcfd_mesh::quality::compute_metrics(mesh);
    let theta: Vec<f64> = (0..n)
        .map(|c| {
            mesh.cell_faces(c)
                .iter()
                .map(|&f| q.non_orthogonality[f])
                .fold(0.0_f64, f64::max)
                .min(89.9)
        })
        .collect();
    let m: Vec<f64> = (0..n)
        .map(|c| {
            let v = q.aspect_ratio[c] * theta[c].to_radians().tan();
            if v.is_nan() {
                0.0
            } else {
                v
            }
        })
        .collect();
    let mut wall = vec![false; n];
    for p in mesh.patches().iter().filter(|p| rules::is_wall_patch(&sim.case, p)) {
        for f in p.faces() {
            wall[mesh.owner()[f]] = true;
        }
    }
    // the harness angle and patch tags, as `fan01_stall_w43c.rs::cell_attributes`
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let (cc, cf, sf) = (mesh.cell_centres(), mesh.face_centres(), mesh.face_area_vectors());
    let mut ang = vec![0.0f64; n];
    let mut tags = vec![String::new(); n];
    let mut patch_of = vec![String::new(); mesh.n_faces()];
    for p in mesh.patches() {
        for f in p.faces() {
            patch_of[f] = p.name.clone();
        }
    }
    for f in 0..mesh.n_faces() {
        if g.empty[f] {
            continue;
        }
        let o = own[f];
        let d = if f < g.n_internal {
            vec3::sub(cc[nei[f]], cc[o])
        } else {
            vec3::sub(cf[f], cc[o])
        };
        let cosv = vec3::dot(d, sf[f]) / (vec3::norm(d) * vec3::norm(sf[f])).max(1e-300);
        let a = cosv.clamp(-1.0, 1.0).acos().to_degrees();
        ang[o] = ang[o].max(a);
        if f < g.n_internal {
            ang[nei[f]] = ang[nei[f]].max(a);
        } else {
            let name = if g.coupled_face[f - g.n_internal] {
                "seam".to_string()
            } else {
                patch_of[f].clone()
            };
            if !tags[o].split('+').any(|t| t == name) {
                if !tags[o].is_empty() {
                    tags[o].push('+');
                }
                tags[o].push_str(&name);
            }
        }
    }
    MeshData {
        n,
        aspect: q.aspect_ratio.clone(),
        theta,
        harness_angle: ang,
        tags,
        wall,
        r: cc.iter().map(|x| x[0].hypot(x[1])).collect(),
        z: cc.iter().map(|x| x[2]).collect(),
        x: cc.to_vec(),
        m,
        sv302_unlimited,
        sv302_bj,
        risk: rules::wall_cell_risk(&sim.case, mesh),
    }
}

/// The meshes, built once per test binary.
fn data(id: &str) -> &'static MeshData {
    static CACHE: OnceLock<std::sync::Mutex<HashMap<String, &'static MeshData>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(d) = cache.lock().unwrap().get(id) {
        return d;
    }
    let (c, blade) = match id {
        "B3" => (F3, true),
        "B2" => (F2, true),
        "B1" => (F1, true),
        "E3" => (F3, false),
        "E2" => (F2, false),
        "E1" => (F1, false),
        _ => panic!("{id}"),
    };
    let d: &'static MeshData = Box::leak(Box::new(build(c, blade)));
    cache.lock().unwrap().insert(id.to_string(), d);
    d
}

/// Percentile rank of `m_c` in the population `pop` (`#{d : m_d <= m_c} / |pop|`).
fn rank(d: &MeshData, c: usize, wall_only: bool) -> f64 {
    let pop: Vec<usize> = (0..d.n).filter(|&k| !wall_only || d.wall[k]).collect();
    pop.iter().filter(|&&k| d.m[k] <= d.m[c]).count() as f64 / pop.len() as f64
}

fn quantile(d: &MeshData, wall_only: bool, q: f64) -> f64 {
    let mut v: Vec<f64> = (0..d.n).filter(|&k| !wall_only || d.wall[k]).map(|k| d.m[k]).collect();
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() - 1) as f64 * q).round() as usize]
}

fn cell_record(d: &MeshData, c: usize) -> Value {
    json!({
        "cell": c, "r": d.r[c], "z": d.z[c], "patches": d.tags[c], "wall": d.wall[c], "m": d.m[c], "aspect": d.aspect[c],
        "theta_metric_deg": d.theta[c], "harness_angle_deg": d.harness_angle[c],
        "rank_all": rank(d, c, false), "rank_wall": if d.wall[c] { json!(rank(d, c, true)) } else { Value::Null },
    })
}

fn n_above(d: &MeshData, level: f64) -> usize {
    d.m.iter().filter(|&&v| v >= level).count()
}

fn argmax(d: &MeshData) -> usize {
    (0..d.n).max_by(|a, b| d.m[*a].total_cmp(&d.m[*b])).unwrap()
}

const S_RU: [usize; 3] = [4971, 4935, 4970];
const S_LC: [usize; 5] = [17, 665, 701, 341, 53];
const S_LM: [usize; 5] = [1234, 365, 401, 1250, 1233];

/// At least `k` of `cells` in the top `pct` % of all cells.
fn in_top(d: &MeshData, cells: &[usize], pct: f64) -> usize {
    cells.iter().filter(|&&c| rank(d, c, false) > 1.0 - pct / 100.0).count()
}

// ---------------------------------------------------------------- validity (V1-V3)

#[test]
fn v1_cell_counts() {
    assert_eq!(data("B3").n, 4920);
    assert_eq!(data("E3").n, 5040);
}

#[test]
#[ignore = "ADR 0174 V1 on F2 and F1 (builds four meshes up to 38 184 cells; run in release)"]
fn v1_cell_counts_f2_f1() {
    assert_eq!(data("B2").n, 13627);
    assert_eq!(data("B1").n, 38184);
    println!("E2 {} cells, E1 {} cells (not on record)", data("E2").n, data("E1").n);
}

#[test]
fn v2_v3_recorded_positions_and_angles() {
    let at = |id: &str, c: usize, r: f64, z: f64, ang: Option<f64>| {
        let d = data(id);
        assert!(
            (d.r[c] - r).abs() <= 6e-4 && (d.z[c] - z).abs() <= 6e-4,
            "{id} c{c}: r {} z {}",
            d.r[c],
            d.z[c]
        );
        if let Some(a) = ang {
            assert!(
                (d.harness_angle[c] - a).abs() <= 0.6,
                "{id} c{c}: angle {}",
                d.harness_angle[c]
            );
        }
    };
    at("B3", 665, 0.1256, 0.1003, Some(28.0));
    at("B3", 17, 0.1243, 0.0998, Some(26.0));
    at("B3", 341, 0.1249, 0.1000, None);
    at("B3", 1234, 0.1408, 0.149, Some(59.0));
    at("B3", 365, 0.1249, 0.172, None);
    at("B3", 401, 0.1249, 0.172, None);
    at("E3", 4971, 0.2477, 0.104, Some(77.0));
}

// ---------------------------------------------------------------- limits (K1-K4, D1, D2)

#[test]
#[ignore = "ADR 0174 known miss K1: c4971 is the largest-m cell of E3 but m = 750 < 3000, and c4935 ranks 0.955"]
fn k1_unlimited_failure_locus_is_high_metric_known_miss() {
    let d = data("E3");
    for c in S_RU {
        let (ra, rw, m) = (rank(d, c, false), rank(d, c, true), d.m[c]);
        assert!(d.wall[c], "c{c} is not a wall cell");
        assert!(
            ra > 0.99 && rw > 0.99 && m >= 3000.0,
            "K1 c{c}: rank_all {ra}, rank_wall {rw}, m {m}"
        );
    }
}

#[test]
fn k2_limit_cycle_cells_are_not_high_metric() {
    // registered prediction: FALSIFIED (fewer than 3 of 5 in the top 5 %)
    let n = in_top(data("B3"), &S_LC, 5.0);
    assert!(n < 3, "K2 STANDS: {n} of 5 limit-cycle cells in the top 5 %");
}

#[test]
fn k3_leading_mode_cells_are_not_high_metric() {
    // registered prediction: FALSIFIED (fewer than 3 of 5 in the top 5 %)
    let n = in_top(data("B3"), &S_LM, 5.0);
    assert!(n < 3, "K3 STANDS: {n} of 5 leading-mode cells in the top 5 %");
}

/// K4 classification: TRACKS, DOES NOT TRACK or UNDECIDED.
fn k4_class() -> &'static str {
    let ids = ["E3", "E2", "E1"];
    let n: Vec<usize> = ids.iter().map(|i| n_above(data(i), 3000.0)).collect();
    let m: Vec<f64> = ids.iter().map(|i| data(i).m[argmax(data(i))]).collect();
    if n[0] < n[1] && n[1] < n[2] && m[0] < m[1] && m[1] < m[2] {
        "TRACKS"
    } else if n[0] > n[1] || n[1] > n[2] || m[0] > m[1] || m[1] > m[2] {
        "DOES NOT TRACK"
    } else {
        "UNDECIDED"
    }
}

#[test]
#[ignore = "ADR 0174 K4 (builds E2 and E1; run in release)"]
fn k4_metric_does_not_track_part_l() {
    // registered prediction: DOES NOT TRACK
    assert_eq!(k4_class(), "DOES NOT TRACK");
}

#[test]
#[ignore = "ADR 0174 known miss D1: SV302 does not fire on unlimited E3 or B3 (largest m 750, level 3000)"]
fn d1_sv302_fires_on_unlimited_fan01_only_known_miss() {
    for id in ["E3", "B3"] {
        let d = data(id);
        assert_eq!((d.sv302_unlimited, d.sv302_bj), (1, 0), "D1 {id}");
    }
}

#[test]
fn d2_sv302_points_at_the_failure_locus() {
    let d = data("E3");
    let r = d.risk.expect("a wall cell");
    let dist = vec3::norm(vec3::sub(r.position, d.x[4971]));
    let in_set = S_RU.contains(&r.cell);
    println!(
        "D2: SV302 names c{} at distance {dist:.4} m from c4971, in S_RU {in_set}",
        r.cell
    );
    assert!(in_set || dist <= 0.01, "D2: SV302 names c{}", r.cell);
}

// ---------------------------------------------------------------- report

#[test]
#[ignore = "ADR 0174 report (builds all six meshes; run in release with --nocapture)"]
fn report() {
    let mut meshes = serde_json::Map::new();
    for id in ["B3", "E3", "B2", "E2", "B1", "E1"] {
        let d = data(id);
        let a = argmax(d);
        let mut top: Vec<usize> = (0..d.n).collect();
        top.sort_by(|x, y| d.m[*y].total_cmp(&d.m[*x]));
        let v = json!({
            "cells": d.n, "wall_cells": d.wall.iter().filter(|w| **w).count(),
            "n_m_ge_3000": n_above(d, 3000.0), "max": cell_record(d, a),
            "p99_all": quantile(d, false, 0.99), "p50_all": quantile(d, false, 0.5),
            "p99_wall": quantile(d, true, 0.99), "p50_wall": quantile(d, true, 0.5),
            "sv302_unlimited": d.sv302_unlimited, "sv302_barth_jespersen": d.sv302_bj,
            "sv302_cell": d.risk.map(|r| r.cell), "sv302_metric": d.risk.map(|r| r.metric),
            "top10": top.iter().take(10).map(|&c| cell_record(d, c)).collect::<Vec<_>>(),
        });
        println!("{id}: {}", serde_json::to_string(&v).unwrap());
        meshes.insert(id.into(), v);
    }
    let sets = json!({
        "S_RU_E3": S_RU.iter().map(|&c| cell_record(data("E3"), c)).collect::<Vec<_>>(),
        "S_LC_B3": S_LC.iter().map(|&c| cell_record(data("B3"), c)).collect::<Vec<_>>(),
        "S_LM_B3": S_LM.iter().map(|&c| cell_record(data("B3"), c)).collect::<Vec<_>>(),
    });
    let verdicts = json!({
        "K2_in_top5": in_top(data("B3"), &S_LC, 5.0), "K3_in_top5": in_top(data("B3"), &S_LM, 5.0), "K4": k4_class(),
    });
    println!("sets: {}", serde_json::to_string_pretty(&sets).unwrap());
    println!("verdicts: {verdicts}");
    if let Ok(p) = std::env::var("W49K_OUT") {
        let doc = json!({"adr": "0174", "meshes": meshes, "sets": sets, "verdicts": verdicts});
        std::fs::write(p, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    }
}
