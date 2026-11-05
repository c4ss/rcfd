//! Job 49R (ADR 0181): the FAN-01 z 0.17 hub site against the C-grid construction (`camber_seam_h`), on B3 and B2.
//!
//! Step 1 (diagnosis, report only): the per-cell geometry, the generator's per-line state around the site and the cycle shape at
//! iteration 1000 of the default (stalled) run. Mesh and case text as `fan01_second_order_w49q.rs`.
//!
//! `W49R_OUT=<absolute dir> cargo test --release -p rcfd-api --test fan01_hub_mesh_w49r <test> -- --ignored --exact --nocapture`.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::fan_case::{cgrid_spec_of, fan_of};
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::fan_cgrid::{build_cgrid_mesh_with, CGridMesh, CGridOptions};
use serde_json::{json, Value};
use std::f64::consts::PI;
use std::time::Instant;

#[path = "../../rcfd-core/tests/support/expected_hashes.rs"]
mod expected_hashes;

/// The registered hashes of this file (ADR 0190); their provenance is in the table.
fn hash_table() -> expected_hashes::Table {
    expected_hashes::Table::load(
        "fan01_hub_mesh_w49r",
        include_str!("fixtures/expected_hashes/fan01_hub_mesh_w49r.tsv"),
    )
}

const R_HUB: f64 = 0.124;
const R_DUCT: f64 = 0.250;
const RHO: f64 = 1.2; // ASSUMED (ADR 0075), UNVERIFIED
const Q_DESIGN: f64 = 1.4032;
const N_BLADES: f64 = 9.0;
const NU: f64 = 1.5e-5;

fn omega() -> f64 {
    1486.0 * 2.0 * PI / 60.0
}

fn annulus_area() -> f64 {
    PI * (R_DUCT * R_DUCT - R_HUB * R_HUB)
}

// ---------------------------------------------------------------- mesh and case (copied from fan01_second_order_w49q.rs)

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

type Counts = (usize, usize, usize, usize, usize, usize, usize);
const F3: Counts = (6, 12, 6, 8, 3, 10, 5);
const F2: Counts = (8, 17, 8, 11, 4, 15, 7);

fn mesh_toml(c: Counts) -> String {
    let rows = sections();
    let (x0, t0) = (rows[0][6], rows[0][7]);
    let ref_speed = (Q_DESIGN / annulus_area()).hypot(omega() * 0.186);
    let (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) = c;
    format!(
        "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\nn_blades = 9\nr_hub = 0.124\nr_tip = 0.2475\ntip_clearance = 0.0025\naxial_extent = 0.4\nz_le_root = 0.1\nfamily = \"naca4\"\n\
station_radius = {}\nstation_chord = {}\nstation_stagger = {}\nstation_camber = {}\nstation_camber_pos = {}\nstation_thickness = {}\nstation_sweep = {}\nstation_lean = {}\n\
topology = \"c_grid\"\nc_grid_construction = \"camber_seam_h\"\nn_up = {n_up}\nn_chord = {n_chord}\nn_down = {n_down}\nn_side = {n_side}\nn_nose = {n_nose}\nn_core = 1\nfront_gap = 0.3\nn_span = {n_span}\nn_gap = {n_gap}\nn_bl = 2\ngrowth = 1.2\n\
first_cell_height = 0.0\ny_plus = 30.0\nref_speed = {ref_speed:?}\nnu = 1.5e-5\n",
        list(&rows, |s| s[0] * 1e-3),
        list(&rows, |s| s[1] * 1e-3),
        list(&rows, |s| s[2]),
        list(&rows, |s| s[3]),
        list(&rows, |s| s[4]),
        list(&rows, |s| s[5]),
        list(&rows, |s| (s[6] - x0) * 1e-3),
        list(&rows, |s| s[0] * 1e-3 * (s[7] - t0).to_radians()),
    )
}

fn case_toml(c: Counts) -> String {
    let outlet = "velocity = \"zero_gradient\"
pressure = \"fixed\"
";
    let w = omega();
    let u_in = Q_DESIGN / annulus_area();
    let k = 1.5 * (0.01 * u_in) * (0.01 * u_in);
    let om = k.sqrt() / (0.09f64.powf(0.25) * 0.0176);
    let wall_t = "turbulence = \"wall\"\n";
    let casing = format!(
        "velocity = \"moving_wall\"\nwall_rotation_rate = {:?}\nwall_velocity_projection = \"tangential\"\n{wall_t}",
        -w
    );
    let hub = "velocity = \"fixed\"\n".to_string();
    let blade_t = "[boundaries.blade]\nvelocity = \"fixed\"\n[reports.m_blade]\ntype = \"moment\"\npatch = \"blade\"\ndirection = [0.0, 0.0, 1.0]\n";
    let turb = "[turbulence]\nmodel = \"k_omega_sst\"\n".to_string();
    let inlet_t =
        format!("turbulence = \"inlet\"\nturbulence_spec = \"values\"\nk_value = {k:?}\nomega_value = {om:?}\n");
    let rel = format!("velocity_expression_x = \"{w:?} [1/s] * y\"\nvelocity_expression_y = \"-{w:?} [1/s] * x\"\n");
    let scheme = "convection = \"linear_upwind\"\nlinear_upwind_limiter = \"barth_jespersen\"";
    format!(
        r#"{}
[physics]
nu = 1.5e-5
rho = {RHO:?}
{turb}[initial]
velocity = [0.0, 0.0, {u_in:?}]
{rel}[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.0, 0.0, {u_in:?}]
{rel}{inlet_t}[boundaries.outlet]
{outlet}[boundaries.hub]
{hub}[boundaries.shroud]
{casing}[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = {w:?}
centrifugal = true
{blade_t}[solver]
algorithm = "simplec"
{scheme}
max_iterations = 100000
tolerance_u = 1e-5
tolerance_p = 1e-5

[reports.m_in]
type = "mass_flow"
patch = "inlet"
[reports.m_out]
type = "mass_flow"
patch = "outlet"
[reports.p_in]
type = "area_average"
patch = "inlet"
field = "p"
[reports.p_out]
type = "area_average"
patch = "outlet"
field = "p"
[reports.m_hub]
type = "moment"
patch = "hub"
direction = [0.0, 0.0, 1.0]
[reports.m_shroud]
type = "moment"
patch = "shroud"
direction = [0.0, 0.0, 1.0]
"#,
        mesh_toml(c)
    )
}

/// Bladed (torque [N m], Delta p_ts [Pa]) as ADR 0086.
fn functionals(sim: &Simulation) -> (f64, f64) {
    let u_in = Q_DESIGN / annulus_area();
    let mon = |k: &str| sim.monitors.last(k).unwrap_or(f64::NAN);
    (
        -N_BLADES * (mon("m_blade") + mon("m_hub")),
        RHO * (mon("p_out") - (mon("p_in") + 0.5 * u_in * u_in)),
    )
}

fn speed_change(a: &[Vec3], b: &[Vec3]) -> Vec<f64> {
    a.iter().zip(b).map(|(x, y)| vec3::norm(vec3::sub(*x, *y))).collect()
}

fn l2(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if v.is_empty() {
        f64::NAN
    } else {
        v[v.len() / 2]
    }
}

/// The `n` cells with the largest values of `d`, largest first.
fn top(d: &[f64], n: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..d.len()).collect();
    idx.sort_by(|a, b| d[*b].partial_cmp(&d[*a]).unwrap_or(std::cmp::Ordering::Equal));
    idx.truncate(n);
    idx
}

/// Percentile rank (0-100) of `v` in `all`.
fn pct(all: &[f64], v: f64) -> f64 {
    100.0 * all.iter().filter(|x| **x < v).count() as f64 / all.len().max(1) as f64
}

fn out_dir() -> Option<std::path::PathBuf> {
    std::env::var("W49R_OUT").ok().map(|d| {
        let p = std::path::PathBuf::from(d);
        assert!(
            p.is_absolute(),
            "W49R_OUT must be an absolute path (cargo runs tests from the crate directory)"
        );
        std::fs::create_dir_all(&p).unwrap();
        p
    })
}

fn write(name: &str, v: &Value) {
    if let Some(d) = out_dir() {
        std::fs::write(d.join(name), serde_json::to_string_pretty(v).unwrap()).unwrap();
    }
}

// ---------------------------------------------------------------- step 1: per-cell geometry (definitions in ADR 0181)

/// Per-cell geometry of a mesh (all report only).
struct Geo {
    /// largest face non-orthogonality [deg] (`compute_metrics`)
    nonorth: Vec<f64>,
    /// largest face skewness (`compute_metrics`)
    skew: Vec<f64>,
    /// aspect ratio (`compute_metrics`)
    aspect: Vec<f64>,
    /// largest volume ratio over the cell's internal faces (`compute_metrics`, >= 1)
    volratio: Vec<f64>,
    /// largest area ratio of a pair of opposite faces of the cell (>= 1): the stream-tube expansion through the cell
    area_jump: Vec<f64>,
    /// largest ratio of the centre distances across a pair of opposite faces (>= 1): the local spacing growth
    growth: Vec<f64>,
    /// largest linear-interpolation weight over the cell's internal faces, `max(w, 1 - w)` (1/2 on a uniform mesh)
    lambda_max: Vec<f64>,
}

fn geometry(sim: &Simulation) -> Geo {
    let g = &sim.geom;
    let mesh = &g.mesh;
    let n = mesh.n_cells();
    let q = rcfd_mesh::quality::compute_metrics(mesh);
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let (cc, cf, sf) = (mesh.cell_centres(), mesh.face_centres(), mesh.face_area_vectors());
    let mut nonorth = vec![0.0f64; n];
    let mut skew = vec![0.0f64; n];
    let mut volratio = vec![1.0f64; n];
    let mut lambda_max = vec![0.0f64; n];
    for f in 0..mesh.n_faces() {
        let o = own[f];
        nonorth[o] = nonorth[o].max(q.non_orthogonality[f]);
        skew[o] = skew[o].max(q.skewness[f]);
        if f < g.n_internal {
            let nb = nei[f];
            nonorth[nb] = nonorth[nb].max(q.non_orthogonality[f]);
            skew[nb] = skew[nb].max(q.skewness[f]);
            let vr = q.volume_ratio[f].max(1.0 / q.volume_ratio[f].max(1e-300));
            volratio[o] = volratio[o].max(vr);
            volratio[nb] = volratio[nb].max(vr);
            let w = g.weights[f].max(1.0 - g.weights[f]);
            lambda_max[o] = lambda_max[o].max(w);
            lambda_max[nb] = lambda_max[nb].max(w);
        }
    }
    let mut area_jump = vec![1.0f64; n];
    let mut growth = vec![1.0f64; n];
    for c in 0..n {
        let fs = mesh.cell_faces(c);
        // outward unit normal, area and centre distance across each face
        let data: Vec<(Vec3, f64, f64)> = fs
            .iter()
            .map(|&f| {
                let s = if own[f] == c { sf[f] } else { vec3::scale(sf[f], -1.0) };
                let a = vec3::norm(s);
                let d = if f < g.n_internal {
                    vec3::norm(vec3::sub(cc[nei[f]], cc[own[f]]))
                } else {
                    2.0 * vec3::norm(vec3::sub(cf[f], cc[c]))
                };
                (vec3::scale(s, 1.0 / a.max(1e-300)), a, d)
            })
            .collect();
        for (a_i, (na, aa, da)) in data.iter().enumerate() {
            // the face most nearly opposite
            let (b_i, _) = data
                .iter()
                .enumerate()
                .filter(|(b, _)| *b != a_i)
                .map(|(b, (nb, _, _))| (b, vec3::dot(*na, *nb)))
                .fold((usize::MAX, f64::INFINITY), |m, x| if x.1 < m.1 { x } else { m });
            if b_i == usize::MAX {
                continue;
            }
            let (_, ab, db) = data[b_i];
            area_jump[c] = area_jump[c].max(aa / ab.max(1e-300)).max(ab / aa.max(1e-300));
            growth[c] = growth[c].max(da / db.max(1e-300)).max(db / da.max(1e-300));
        }
    }
    Geo {
        nonorth,
        skew,
        aspect: q.aspect_ratio.clone(),
        volratio,
        area_jump,
        growth,
        lambda_max,
    }
}

/// Per-cell flow numbers at the current iterate: largest face Peclet `|F_f| / ((nu + nu_t)_f |S|^2 / (S . d))` and the residence
/// time `V / sum(outflow)` relative to the median cell. There is no physical time step in the steady SIMPLEC iteration, so no
/// Courant number is defined; the residence-time ratio is the closest flux-based measure (ADR 0181).
fn flow_numbers(sim: &Simulation) -> (Vec<f64>, Vec<f64>) {
    let g = &sim.geom;
    let mesh = &g.mesh;
    let n = mesh.n_cells();
    let s = sim.simple_solver().unwrap();
    let flux: Vec<f64> = match &s.conv_flux_offset {
        Some(o) => s.phi.iter().zip(o).map(|(p, o)| p - o).collect(),
        None => s.phi.clone(),
    };
    let nut = sim.field("nut").unwrap_or_else(|_| vec![0.0; n]);
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut pe = vec![0.0f64; n];
    let mut out = vec![0.0f64; n];
    for f in 0..mesh.n_faces() {
        if g.empty[f] {
            continue;
        }
        let o = own[f];
        let (nuf, coeff) = if f < g.n_internal {
            (NU + 0.5 * (nut[o] + nut[nei[f]]), g.lap_coeff[f])
        } else {
            (NU + nut[o], g.lap_coeff[f])
        };
        let p = flux[f].abs() / (nuf * coeff).max(1e-300);
        pe[o] = pe[o].max(p);
        if flux[f] > 0.0 {
            out[o] += flux[f];
        }
        if f < g.n_internal {
            let nb = nei[f];
            pe[nb] = pe[nb].max(p);
            if flux[f] < 0.0 {
                out[nb] -= flux[f];
            }
        }
    }
    let vol = mesh.cell_volumes();
    let tau: Vec<f64> = (0..n).map(|c| vol[c] / out[c].max(1e-300)).collect();
    let tm = median(tau.clone());
    (pe, tau.iter().map(|t| t / tm).collect())
}

/// The generator mesh of the case (same construction as `fan_case::build`), with its bookkeeping.
fn generator_mesh(case: &Case) -> CGridMesh {
    build_cgrid_mesh_with(
        &fan_of(case),
        &cgrid_spec_of(case),
        &CGridOptions::camber_seam_h_construction(),
    )
    .expect("C-grid builds")
}

/// `(z, s = r theta)` of a grid node.
fn zs(p: Vec3) -> (f64, f64) {
    let r = p[0].hypot(p[1]);
    (p[2], r * p[1].atan2(p[0]))
}

/// Generator tag of a cell: (block, a, b, k).
type Tag = (u8, usize, usize, usize);

fn tag_json(t: Tag) -> Value {
    let blk = match t.0 {
        0 => "C",
        1 => "F",
        _ => "core",
    };
    json!([blk, t.1, t.2, t.3])
}

/// The generator's per-line state around the trailing edge at layers `ks` (step 1, "what the generator does there").
fn line_state(cg: &CGridMesh, nd: usize, n_i: usize, ns: usize, ks: &[usize]) -> Value {
    let p = |i: usize, j: usize, k: usize| zs(cg.grid[cg.c_node(i, j, k)]);
    let dist = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).hypot(a.1 - b.1);
    let mut layers = Vec::new();
    for &k in ks {
        // wall-and-wake curve spacing (j = 0) from line nd - 3 to nd + 3 (lower side) and the upper-side counterpart
        let mut lines = Vec::new();
        for i in nd.saturating_sub(3)..=(nd + 3) {
            let up = n_i - i;
            let seg = dist(p(i, 0, k), p(i + 1, 0, k));
            let st0 = dist(p(i, 0, k), p(i, 1, k));
            let st1 = dist(p(i, 1, k), p(i, 2, k));
            let st0u = dist(p(up, 0, k), p(up, 1, k));
            // in-plane line direction j = 0 -> 1 and the angle to the next line's direction
            let dir = |ii: usize| {
                let (a, b) = (p(ii, 0, k), p(ii, 1, k));
                (b.1 - a.1).atan2(b.0 - a.0)
            };
            let turn = (dir(i + 1) - dir(i)).to_degrees();
            // seam row node turning angle at line i (j = ns)
            let seam_turn = if i >= 1 {
                let (a, b, c) = (p(i - 1, ns, k), p(i, ns, k), p(i + 1, ns, k));
                ((c.1 - b.1).atan2(c.0 - b.0) - (b.1 - a.1).atan2(b.0 - a.0)).to_degrees()
            } else {
                f64::NAN
            };
            lines.push(json!({
                "i": i, "z": p(i, 0, k).0, "stack_cells": cg.stack_cells[k][i], "stack_cells_upper": cg.stack_cells[k][up],
                "wall_segment_to_next": seg, "first_height": st0, "second_height": st1, "first_height_upper": st0u,
                "line_dir_turn_to_next_deg": turn, "seam_turn_deg": seam_turn,
            }));
        }
        layers.push(json!({"k": k, "r": cg.radii[k], "lines": lines}));
    }
    let dr: Vec<f64> = cg.radii.windows(2).map(|w| w[1] - w[0]).collect();
    json!({
        "construction_step": cg.construction_step, "h1": cg.h1, "radii": cg.radii,
        "radial_thickness": dr,
        "radial_growth": dr.windows(2).map(|w| w[1] / w[0]).collect::<Vec<f64>>(),
        "z_te_hub": p(nd, 0, 0).0, "z_le_hub": p(nd + cg.n_chord(), 0, 0).0,
        "layers": layers,
    })
}

/// The B3 hub seeds of ADR 0178 (the F-R(0.25) hub cycle, 19 cells).
const B3_HUB_SEEDS: [usize; 19] = [
    1234, 365, 401, 1250, 1233, 907, 906, 581, 905, 582, 149, 437, 150, 438, 402, 113, 114, 77, 257,
];

/// The B2 cycle cells of ADR 0180 (TVD arms and the Barth-Jespersen stall at the switch).
const B2_SITE: [usize; 6] = [157, 621, 1235, 107, 7, 57];

fn diagnose(mesh_id: &str) -> Value {
    let t0 = Instant::now();
    let c = if mesh_id == "B3" { F3 } else { F2 };
    let case = Case::parse_toml(&case_toml(c)).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    let cg = generator_mesh(&case);
    // the generator mesh is the mesh of the simulation (same hash, same cell order)
    assert_eq!(
        cg.mesh.content_hash(),
        sim.geom.mesh.content_hash(),
        "generator mesh differs from the case mesh"
    );
    assert_eq!(cg.cell_tags.len(), sim.geom.mesh.n_cells());
    let n = sim.geom.mesh.n_cells();
    let (n_up, n_chord, nd, ns, n_nose, n_span, _n_gap) = c;
    let n_i = cg.n_i();
    let _ = (n_up, n_nose);
    let tags = &cg.cell_tags;
    let geo = geometry(&sim);

    // the default run to iteration 1000 (the stalled reference of ADRs 0175-0180)
    let mut hist = Vec::new();
    let mut f_ref = Vec::new();
    let mut prev = sim.velocity().values.clone();
    for it in 1..=1000 {
        prev = sim.velocity().values.clone();
        let st = sim.step();
        hist.push((st.res_u, st.res_p));
        if it > 500 {
            f_ref.push(functionals(&sim));
        }
    }
    let du = speed_change(&sim.velocity().values, &prev);
    let du_l2 = l2(&du);
    let med_u = median(hist[500..].iter().map(|h| h.0).collect());
    let med_p = median(hist[500..].iter().map(|h| h.1).collect());
    let t_mean = f_ref.iter().map(|f| f.0).sum::<f64>() / f_ref.len() as f64;
    let p_mean = f_ref.iter().map(|f| f.1).sum::<f64>() / f_ref.len() as f64;
    let t_1000 = functionals(&sim).0;
    println!("W49R {mesh_id} cells {n} medians {med_u:.3e} / {med_p:.2e} torque@1000 {t_1000:.4} means {t_mean:.4} / {p_mean:.3} |du| {du_l2:.3}");
    let (pe, tau) = flow_numbers(&sim);

    // the site: the 20 largest-|du| cells at 1000, and the registered sets of earlier jobs
    let top20 = top(&du, 20);
    let site: Vec<usize> = if mesh_id == "B3" {
        B3_HUB_SEEDS.to_vec()
    } else {
        B2_SITE.to_vec()
    };
    let metrics: [(&str, &Vec<f64>); 10] = [
        ("nonorth_deg", &geo.nonorth),
        ("skewness", &geo.skew),
        ("aspect", &geo.aspect),
        ("volume_ratio", &geo.volratio),
        ("area_jump", &geo.area_jump),
        ("growth", &geo.growth),
        ("lambda_max", &geo.lambda_max),
        ("peclet_max", &pe),
        ("residence_ratio", &tau),
        ("du_1000", &du),
    ];
    let cell_rec = |c: usize| -> Value {
        let x = sim.geom.mesh.cell_centres()[c];
        let mut m = serde_json::Map::new();
        m.insert("cell".into(), json!(c));
        m.insert("tag".into(), tag_json(tags[c]));
        m.insert("r".into(), json!(x[0].hypot(x[1])));
        m.insert("z".into(), json!(x[2]));
        // relative velocity at iteration 1000: axial component and magnitude (a negative axial component is reversed flow)
        let u = sim.velocity().values[c];
        m.insert("u_z".into(), json!(u[2]));
        m.insert("u_mag".into(), json!(vec3::norm(u)));
        for (name, v) in &metrics {
            m.insert((*name).into(), json!(v[c]));
            m.insert(format!("{name}_pct"), json!(pct(v, v[c])));
        }
        Value::Object(m)
    };
    // shares of sum |du|^2 by location class
    let d2: f64 = du.iter().map(|x| x * x).sum();
    let h = nd + n_chord;
    let class = |t: (u8, usize, usize, usize)| -> &'static str {
        if t.0 != 0 {
            return if t.0 == 1 { "F block" } else { "core" };
        }
        let i = t.1;
        let hub = t.3 <= 2;
        let te_wake = i == nd - 1 || i == n_i - nd;
        let te_chord = i == nd || i == n_i - nd - 1;
        let le = i == h - 1 || i == h;
        match (te_wake, te_chord, le, hub) {
            (true, _, _, true) => "TE first wake column, k<=2",
            (true, _, _, false) => "TE first wake column, k>2",
            (_, true, _, true) => "TE last chord column, k<=2",
            (_, true, _, false) => "TE last chord column, k>2",
            (_, _, true, true) => "LE columns, k<=2",
            (_, _, true, false) => "LE columns, k>2",
            _ if hub => "other C, k<=2",
            _ => "other C, k>2",
        }
    };
    let mut shares: std::collections::BTreeMap<&str, (usize, f64)> = Default::default();
    for cc in 0..n {
        let e = shares.entry(class(tags[cc])).or_insert((0, 0.0));
        e.0 += 1;
        e.1 += du[cc] * du[cc] / d2;
    }
    // contrast (ADR 0181): the metric at the site tags, at the same (i, j) at mid-span, and at the same (j, k) at mid-chord and
    // at the leading-edge column
    let id_of = |t: (u8, usize, usize, usize)| tags.iter().position(|x| *x == t);
    let k_mid = n_span / 2;
    let i_mid = nd + n_chord / 2;
    let shifted = |cells: &[usize], f: &dyn Fn(Tag) -> Tag| -> Vec<usize> {
        cells.iter().filter_map(|&c| id_of(f(tags[c]))).collect()
    };
    let site_c: Vec<usize> = site.iter().copied().filter(|&c| tags[c].0 == 0).collect();
    let groups: Vec<(&str, Vec<usize>)> = vec![
        ("site", site_c.clone()),
        ("same (i, j) at k_mid", shifted(&site_c, &|t| (0, t.1, t.2, k_mid))),
        (
            "same (j, k) at mid-chord i (lower side)",
            shifted(&site_c, &|t| (0, i_mid, t.2, t.3)),
        ),
        (
            "same (j, k) at LE column h-1",
            shifted(&site_c, &|t| (0, h - 1, t.2, t.3)),
        ),
    ];
    let mut contrast = serde_json::Map::new();
    for (gname, cells) in &groups {
        let mut m = serde_json::Map::new();
        m.insert("n".into(), json!(cells.len()));
        for (name, v) in &metrics {
            let vals: Vec<f64> = cells.iter().map(|&c| v[c]).collect();
            let mean = vals.iter().sum::<f64>() / vals.len().max(1) as f64;
            let max = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            m.insert((*name).into(), json!({"mean": mean, "max": max}));
        }
        contrast.insert((*gname).into(), Value::Object(m));
    }
    // mesh-wide percentiles of each metric
    let mut dist = serde_json::Map::new();
    for (name, v) in &metrics {
        let mut s = (*v).clone();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let q = |p: f64| s[((s.len() - 1) as f64 * p) as usize];
        dist.insert(
            (*name).into(),
            json!({"p50": q(0.5), "p90": q(0.9), "p99": q(0.99), "max": q(1.0)}),
        );
    }
    let rec = json!({
        "adr": "0181", "id": format!("D-{mesh_id}"), "mesh": mesh_id, "n_cells": n,
        "ref_median_res_u": med_u, "ref_median_res_p": med_p, "torque_1000": t_1000,
        "torque_ref_mean": t_mean, "dpts_ref_mean": p_mean, "du_l2_1000": du_l2,
        "generator": line_state(&cg, nd, n_i, ns, &[0, 1, 2, k_mid]),
        "top20_1000": top20.iter().map(|&c| cell_rec(c)).collect::<Vec<Value>>(),
        "site_cells": site.iter().map(|&c| cell_rec(c)).collect::<Vec<Value>>(),
        "du2_share_by_class": shares.iter().map(|(k, (n, s))| json!({"class": k, "cells": n, "share": s})).collect::<Vec<Value>>(),
        "contrast": contrast, "distribution": dist,
        "seconds": t0.elapsed().as_secs_f64(),
    });
    println!("W49R D-{mesh_id} RESULT {}", serde_json::to_string(&rec).unwrap());
    write(&format!("D-{mesh_id}.json"), &rec);
    rec
}

#[test]
#[ignore = "ADR 0181 step 1 diagnosis on B3 (report only; 1000 default iterations, about 1 min in release)"]
fn diagnose_b3() {
    diagnose("B3");
}

#[test]
#[ignore = "ADR 0181 step 1 diagnosis on B2 (report only; 1000 default iterations, about 3 min in release)"]
fn diagnose_b2() {
    diagnose("B2");
}

/// Content hashes of the default B3 and B2 meshes recorded on the unchanged generator (ADR 0181 bit-identity check).
#[test]
fn w49r_default_meshes_are_bit_identical() {
    let table = hash_table();
    for (id, c, cells) in [("B3", F3, 4920), ("B2", F2, 13627)] {
        // the case without the key and with the explicit default value
        for text in [case_toml(c), case_toml_g(c, Some("capped"))] {
            let case = Case::parse_toml(&text).unwrap();
            let m = rcfd_api::sim::build_mesh(&case).unwrap();
            println!("W49R {id} cells {} hash {:#018x}", m.n_cells(), m.content_hash());
            match table.hash(id) {
                Some(hash) => assert_eq!(
                    (m.n_cells(), m.content_hash()),
                    (cells, hash),
                    "{id}: default mesh changed"
                ),
                None => eprintln!("{id}: no recorded hash for this platform (printed above)"),
            }
        }
    }
}

/// ADR 0181 addendum (2026-11-05) F3: the default `"capped"` leaves the case's settings and content hash unchanged (absent,
/// written or set); every other value changes the hash, is written in the TOML and JSON text and round-trips through the TOML.
#[test]
fn w49r_c_grid_grading_hash_and_text() {
    let parse = |t: &str| Case::parse_toml(t).unwrap_or_else(|e| panic!("{e:?}"));
    let c0 = parse(&case_toml(F3));
    assert_eq!(c0.get("mesh.fan.c_grid_grading").unwrap(), json!("capped"));
    let h0 = c0.tree.content_hash();
    let written = parse(&case_toml_g(F3, Some("capped")));
    let mut set = parse(&case_toml(F3));
    set.set("mesh.fan.c_grid_grading", json!("capped")).unwrap();
    println!(
        "W49R F3 capped: absent {h0:016x}, written {:016x}, set {:016x}",
        written.tree.content_hash(),
        set.tree.content_hash()
    );
    assert_eq!(written.tree.to_json(), c0.tree.to_json());
    assert_eq!(set.tree.to_json(), c0.tree.to_json());
    assert_eq!(written.tree.content_hash(), h0);
    assert_eq!(set.tree.content_hash(), h0);
    let mut hashes = vec![h0];
    for g in ["radial_geometric", "wake_geometric", "radial_wake_geometric"] {
        let c = parse(&case_toml_g(F3, Some(g)));
        let h = c.tree.content_hash();
        let toml = c.to_toml();
        let back = parse(&toml);
        println!(
            "W49R F3 {g}: hash {h:016x}, parsed back {:016x}",
            back.tree.content_hash()
        );
        assert!(
            !hashes.contains(&h),
            "{g}: hash {h:016x} equals the default's or another value's"
        );
        hashes.push(h);
        assert!(
            toml.contains(&format!("c_grid_grading = \"{g}\"")),
            "{g}: not in the TOML"
        );
        assert!(
            c.to_json_string().contains(&format!("\"c_grid_grading\": \"{g}\"")),
            "{g}: not in the JSON"
        );
        assert_eq!(back.get("mesh.fan.c_grid_grading").unwrap(), json!(g));
        assert_eq!(back.tree.to_json(), c.tree.to_json(), "{g}: TOML round trip");
        assert_eq!(back.tree.content_hash(), h, "{g}: hash after the TOML round trip");
    }
}

// ---------------------------------------------------------------- arms (ADR 0181, fixed before any arm mesh was built)

/// The case of the harness with `mesh.fan.c_grid_grading = g` (None: the key absent).
fn case_toml_g(c: Counts, g: Option<&str>) -> String {
    let t = case_toml(c);
    match g {
        None => t,
        Some(v) => t.replacen(
            "growth = 1.2\n",
            &format!("growth = 1.2\nc_grid_grading = \"{v}\"\n"),
            1,
        ),
    }
}

fn grading_of(arm: &str) -> &'static str {
    match arm {
        "M-R" => "radial_geometric",
        "M-W" => "wake_geometric",
        "M-RW" => "radial_wake_geometric",
        _ => panic!("unknown arm {arm}"),
    }
}

/// The generator mesh of the case with the arm's flags (as `fan_case::build`), or the build error.
fn try_generator_mesh_g(case: &Case, grading: &str) -> Result<CGridMesh, String> {
    let mut o = CGridOptions::camber_seam_h_construction();
    o.radial_geometric = grading.starts_with("radial");
    o.wake_geometric = grading.contains("wake");
    build_cgrid_mesh_with(&fan_of(case), &cgrid_spec_of(case), &o).map_err(|e| format!("{e:?}"))
}

fn generator_mesh_g(case: &Case, grading: &str) -> CGridMesh {
    try_generator_mesh_g(case, grading).expect("C-grid builds")
}

/// V3 of one arm and mesh: same cell count, the simulation accepts the mesh, the targeted feature changed. Returns the record.
fn v3(arm: &str, mesh_id: &str) -> (Value, bool) {
    let c = if mesh_id == "B3" { F3 } else { F2 };
    let g = grading_of(arm);
    let case = Case::parse_toml(&case_toml_g(c, Some(g))).unwrap();
    let cg = match try_generator_mesh_g(&case, g) {
        Ok(m) => m,
        Err(e) => {
            let rec = json!({"arm": arm, "mesh": mesh_id, "grading": g, "builds": false, "error": e, "V3": false});
            println!("W49R V3 {arm}-{mesh_id} {rec}");
            return (rec, false);
        }
    };
    let sim_mesh = rcfd_api::sim::build_mesh(&case).unwrap();
    assert_eq!(
        cg.mesh.content_hash(),
        sim_mesh.content_hash(),
        "generator mesh differs from the case mesh"
    );
    let n = cg.mesh.n_cells();
    let n_span = c.5;
    let nd = c.2;
    let dr: Vec<f64> = cg.radii[..=n_span].windows(2).map(|w| w[1] - w[0]).collect();
    let radial_max = dr
        .windows(2)
        .map(|w| (w[1] / w[0]).max(w[0] / w[1]))
        .fold(0.0f64, f64::max);
    let first_rel = (dr[0] - cg.h1).abs() / cg.h1;
    // wake axial cells of layer 0, from the trailing edge (node i = nd) downstream (i decreasing)
    let z: Vec<f64> = (0..=nd).rev().map(|i| cg.grid[cg.c_node(i, 0, 0)][2]).collect();
    let dz: Vec<f64> = z.windows(2).map(|w| w[1] - w[0]).collect();
    let wake_max = dz.windows(2).map(|w| w[1] / w[0]).fold(0.0f64, f64::max);
    let q = rcfd_mesh::quality::compute_metrics(&cg.mesh);
    let mx = |v: &[f64]| v.iter().cloned().fold(0.0f64, f64::max);
    let (r_lim, w_lim) = if mesh_id == "B3" { (3.0, 2.25) } else { (1.9, 1.9) };
    let cells = if mesh_id == "B3" { 4920 } else { 13627 };
    let radial_ok = !g.starts_with("radial") || (first_rel < 1e-9 && radial_max <= r_lim);
    let wake_ok = !g.contains("wake") || wake_max <= w_lim;
    let sim_ok = Simulation::new(&case).is_ok();
    let ok = n == cells && sim_ok && radial_ok && wake_ok;
    let rec = json!({
        "arm": arm, "mesh": mesh_id, "grading": g, "n_cells": n, "construction_step": cg.construction_step,
        "hash": format!("{:#018x}", cg.mesh.content_hash()), "simulation_accepts": sim_ok,
        "span_thickness": dr, "radial_max_ratio": radial_max, "first_layer_rel_to_h1": first_rel,
        "wake_dz_hub": dz, "wake_max_ratio_hub": wake_max,
        "max_nonorth_deg": mx(&q.non_orthogonality), "max_skewness": mx(&q.skewness),
        "V3": ok,
    });
    println!("W49R V3 {arm}-{mesh_id} {rec}");
    (rec, ok)
}

#[test]
#[ignore = "ADR 0181 V3: the arm meshes build, keep their cell counts and change the targeted feature (about 1 min in release)"]
fn w49r_v3_arm_meshes() {
    let mut all = Vec::new();
    let mut ok = true;
    for arm in ["M-R", "M-W", "M-RW"] {
        for m in ["B3", "B2"] {
            let (r, o) = v3(arm, m);
            // amendment 1: M-W and M-RW failed V3 and are withdrawn; V3 is asserted for M-R only
            if arm == "M-R" {
                ok &= o;
            }
            all.push(r);
        }
    }
    write("V3.json", &json!(all));
    assert!(ok, "V3 failed (defective set-up: amendment before any run)");
}

const N_RUN: usize = 3000;
const N_REF: usize = 1000;
const W: usize = 500;
const FLOOR_T: f64 = 0.002;
const FLOOR_P: f64 = 0.005;
const SB_T: f64 = 0.02;
const SB_P: f64 = 0.08;
const T_MEAS: f64 = 2.142;
const P_MEAS: f64 = 126.5;

/// The unchanged-mesh reference of step 1 (V1): medians of iterations 501-1000, `|du|_2` at 1000, cycle means.
struct Ref {
    med_u: f64,
    med_p: f64,
    du: f64,
    t_mean: f64,
    p_mean: f64,
    t_1000: f64,
}

fn reference(c: Counts) -> Ref {
    let mut sim = Simulation::new(&Case::parse_toml(&case_toml(c)).unwrap()).unwrap();
    let mut hist = Vec::new();
    let mut f = Vec::new();
    let mut prev = sim.velocity().values.clone();
    for it in 1..=N_REF {
        prev = sim.velocity().values.clone();
        let st = sim.step();
        hist.push((st.res_u, st.res_p));
        if it > N_REF - W {
            f.push(functionals(&sim));
        }
    }
    Ref {
        med_u: median(hist[N_REF - W..].iter().map(|h| h.0).collect()),
        med_p: median(hist[N_REF - W..].iter().map(|h| h.1).collect()),
        du: l2(&speed_change(&sim.velocity().values, &prev)),
        t_mean: f.iter().map(|x| x.0).sum::<f64>() / f.len() as f64,
        p_mean: f.iter().map(|x| x.1).sum::<f64>() / f.len() as f64,
        t_1000: functionals(&sim).0,
    }
}

fn arm_run(arm: &str, mesh_id: &str, budget_s: f64) -> Value {
    let id = format!("{arm}-{mesh_id}");
    let c = if mesh_id == "B3" { F3 } else { F2 };
    let g = grading_of(arm);
    let (v3rec, v3ok) = v3(arm, mesh_id);
    assert!(v3ok, "{id}: V3 failed (defective set-up)");
    // V1: the unchanged-mesh reference reproduces step 1
    let tr = Instant::now();
    let rf = reference(c);
    let ref_s = tr.elapsed().as_secs_f64();
    let v1 = if mesh_id == "B3" {
        format!("{:.3e}", rf.med_u) == "1.089e-3"
            && format!("{:.2e}", rf.med_p) == "4.87e-4"
            && format!("{:.4}", rf.t_1000) == "1.9177"
            && format!("{:.4}", rf.t_mean) == "1.9176"
            && format!("{:.3}", rf.p_mean) == "46.704"
            && format!("{:.3}", rf.du) == "5.742"
    } else {
        format!("{:.3e}", rf.med_u) == "1.004e-3"
            && format!("{:.2e}", rf.med_p) == "6.23e-4"
            && format!("{:.4}", rf.t_mean) == "2.0308"
            && format!("{:.3}", rf.p_mean) == "63.860"
            && format!("{:.3}", rf.du) == "8.248"
    };
    assert!(
        v1,
        "{id}: V1 failed: reference {:.3e} {:.2e} {:.4} {:.4} {:.3} {:.3}",
        rf.med_u, rf.med_p, rf.t_1000, rf.t_mean, rf.p_mean, rf.du
    );
    // the arm, from iteration 1
    let t0 = Instant::now();
    let case = Case::parse_toml(&case_toml_g(c, Some(g))).unwrap();
    let cg = generator_mesh_g(&case, g);
    let mut sim = Simulation::new(&case).unwrap();
    assert_eq!(cg.mesh.content_hash(), sim.geom.mesh.content_hash());
    let tags = cg.cell_tags.clone();
    let n = sim.geom.mesh.n_cells();
    let mut hist: Vec<(f64, f64)> = Vec::new();
    let mut prev = sim.velocity().values.clone();
    let mut du_1000_self = f64::NAN;
    let (mut reached, mut stopped, mut budget_hit) = (None, None, false);
    for it in 1..=N_RUN {
        prev = sim.velocity().values.clone();
        let st = sim.step();
        hist.push((st.res_u, st.res_p));
        if it == N_REF {
            du_1000_self = l2(&speed_change(&sim.velocity().values, &prev));
        }
        if it > N_REF && reached.is_none() && st.res_u < 1e-5 && st.res_p < 1e-5 {
            reached = Some(it);
        }
        if !(st.res_u.is_finite() && st.continuity.is_finite()) || sim.steady_divergence().is_some() {
            stopped = Some(it);
            println!("W49R {id} STOP at {it}");
            break;
        }
        if t0.elapsed().as_secs_f64() > budget_s {
            budget_hit = true;
            stopped = Some(it);
            println!("W49R {id} BUDGET HIT at {it}");
            break;
        }
        if it % 250 == 0 {
            let f = functionals(&sim);
            println!(
                "W49R {id} it {it}: res_u {:.3e} res_p {:.3e} torque {:.4} dpts {:.3} ({:.1} s)",
                st.res_u,
                st.res_p,
                f.0,
                f.1,
                t0.elapsed().as_secs_f64()
            );
        }
    }
    let nh = hist.len();
    let a = nh.saturating_sub(W);
    let arm_u = median(hist[a..].iter().map(|h| h.0).collect());
    let arm_p = median(hist[a..].iter().map(|h| h.1).collect());
    let rho_med = (arm_u / rf.med_u).max(arm_p / rf.med_p);
    let d_end = speed_change(&sim.velocity().values, &prev);
    let du_end = l2(&d_end);
    let rho_du = du_end / rf.du;
    let f_end = functionals(&sim);
    let d_t = (f_end.0 - rf.t_mean).abs() / rf.t_mean.abs();
    let d_p = (f_end.1 - rf.p_mean).abs() / rf.p_mean.abs();
    let converged = sim.is_converged();
    let ok = stopped.is_none();
    // share of sum |du|^2 at the end in the hub trailing-edge columns (first wake and last chord columns, k <= 2)
    let (nd, n_i) = (c.2, cg.n_i());
    let d2: f64 = d_end.iter().map(|x| x * x).sum();
    let te_share = (0..n)
        .filter(|&q| {
            let t = tags[q];
            t.0 == 0 && t.3 <= 2 && [nd - 1, nd, n_i - nd - 1, n_i - nd].contains(&t.1)
        })
        .map(|q| d_end[q] * d_end[q])
        .sum::<f64>()
        / d2.max(1e-300);
    let top_end: Vec<Value> = top(&d_end, 5)
        .into_iter()
        .map(|q| {
            let x = sim.geom.mesh.cell_centres()[q];
            json!({"cell": q, "tag": tag_json(tags[q]), "du": d_end[q], "r": x[0].hypot(x[1]), "z": x[2]})
        })
        .collect();
    let last = hist.last().copied().unwrap_or((f64::NAN, f64::NAN));
    let rec = json!({
        "adr": "0181", "arm": arm, "id": id, "mesh": mesh_id, "grading": g, "n_cells": n, "V3": v3rec,
        "ref": {"median_res_u": rf.med_u, "median_res_p": rf.med_p, "du_1000": rf.du, "torque_mean": rf.t_mean,
                "dpts_mean": rf.p_mean, "torque_1000": rf.t_1000, "seconds": ref_s},
        "arm_median_res_u": arm_u, "arm_median_res_p": arm_p, "end_res_u": last.0, "end_res_p": last.1,
        "res_nut_end": sim.monitors.last("res_nut").unwrap_or(f64::NAN),
        "rho_med": rho_med, "du_1000_self": du_1000_self, "du_end": du_end, "rho_du": rho_du,
        "rho_du_self": du_end / du_1000_self,
        "reached": reached, "stopped": stopped, "budget_hit": budget_hit, "is_converged_end": converged,
        "torque_end": f_end.0, "dpts_end": f_end.1, "d_torque": d_t, "d_dpts": d_p,
        "torque_gap_measured": (f_end.0 - T_MEAS) / T_MEAS, "dpts_gap_measured": (f_end.1 - P_MEAS) / P_MEAS,
        "hub_te_du2_share_end": te_share, "top5_end": top_end,
        "limits": {"C1": ok && reached.is_some(), "C2": ok && converged, "C3": ok && rho_du <= 0.01,
                   "SB": ok && d_t <= SB_T && d_p <= SB_P},
        "seconds": t0.elapsed().as_secs_f64(),
    });
    println!("W49R {id} RESULT {}", serde_json::to_string(&rec).unwrap());
    println!(
        "W49R {id} reached {reached:?} conv {converged} rho_med {rho_med:.3e} rho_du {rho_du:.3e} torque {:.4} (D {d_t:.4}) dpts {:.3} (D {d_p:.4}) te_share {te_share:.3}; limits {}",
        f_end.0, f_end.1, rec["limits"]
    );
    write(&format!("{id}.json"), &rec);
    rec
}

fn read(id: &str) -> Value {
    let d = out_dir().expect("W49R_OUT");
    let p = d.join(format!("{id}.json"));
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
}

fn assert_limits(r: &Value, keys: &[&str]) {
    let missed: Vec<&&str> = keys.iter().filter(|k| r["limits"][**k] != json!(true)).collect();
    assert!(missed.is_empty(), "{}: missed {missed:?}", r["id"]);
}

/// RN of an arm from its B3 and B2 records (ADR 0181), with the ratio and the implied order (report only).
fn refinement(arm: &str) -> (bool, Value) {
    let (b3, b2) = (read(&format!("{arm}-B3")), read(&format!("{arm}-B2")));
    let gf = |r: &Value, k: &str| r[k].as_f64().unwrap_or(f64::NAN);
    let (t3, t2, p3, p2) = (
        gf(&b3, "d_torque"),
        gf(&b2, "d_torque"),
        gf(&b3, "d_dpts"),
        gf(&b2, "d_dpts"),
    );
    let rn = t2 <= t3.max(FLOOR_T) && p2 <= p3.max(FLOOR_P);
    let r32: f64 = 1.4017;
    let rec = json!({"arm": arm, "D_T_B3": t3, "D_T_B2": t2, "D_P_B3": p3, "D_P_B2": p2,
        "ratio_T": t2 / t3, "ratio_P": p2 / p3, "order_T": (t3 / t2).ln() / r32.ln(), "order_P": (p3 / p2).ln() / r32.ln(),
        "RN": rn, "SB_B3": b3["limits"]["SB"], "SB_B2": b2["limits"]["SB"]});
    println!("W49R refinement {rec}");
    write(&format!("{arm}-refinement.json"), &rec);
    (rn, rec)
}

// B3 arms (calibration); budget 10 min each.

#[test]
#[ignore = "known miss (ADR 0181 C1, C2, C3, SB; P1 held): M-R on B3 keeps the cycle (rho_du 0.80, res_u 5.4e-4), moved to pressure-side cells at k 3 and the LE root; torque D 0.050, Delta p_ts D 0.197 (about 3 min in release with the reference; budget 10 min)"]
fn m_r_b3_known_miss() {
    let r = arm_run("M-R", "B3", 600.0);
    assert_limits(&r, &["C1", "C2", "C3", "SB"]);
}

#[test]
#[ignore = "ADR 0181 arm M-W on B3: WITHDRAWN by amendment 1 (V3 failed: the uncapped wake rule tangles the C-grid); not run"]
fn m_w_b3() {
    let r = arm_run("M-W", "B3", 600.0);
    assert_limits(&r, &["C1", "C2", "C3", "SB"]);
}

#[test]
#[ignore = "ADR 0181 arm M-RW on B3: WITHDRAWN by amendment 1 (V3 failed on B3); not run"]
fn m_rw_b3() {
    let r = arm_run("M-RW", "B3", 600.0);
    assert_limits(&r, &["C1", "C2", "C3", "SB"]);
}

// B2 arms (held out: run only after the three B3 records are committed); budget 30 min each. Each also checks RN.

fn b2_arm(arm: &str) {
    let r = arm_run(arm, "B2", 1800.0);
    let (rn, _) = refinement(arm);
    assert_limits(&r, &["C1", "C2", "C3", "SB"]);
    assert!(rn, "{arm}: RN missed");
}

#[test]
#[ignore = "known miss (ADR 0181 C1, C2, C3, SB; RN met): M-R on B2 keeps the cycle at the hub trailing-edge cells c7, c107, c621 (rho_du 0.76); torque D 0.0228 > 0.02, Delta p_ts D 0.0818 > 0.08 (about 7 min in release with the reference; budget 30 min; needs M-R-B3 in W49R_OUT)"]
fn m_r_b2_known_miss() {
    b2_arm("M-R");
}

#[test]
#[ignore = "ADR 0181 arm M-W on B2: WITHDRAWN by amendment 1 (V3 failed: the uncapped wake rule folds the C-grid); not run"]
fn m_w_b2() {
    b2_arm("M-W");
}

#[test]
#[ignore = "ADR 0181 arm M-RW on B2: WITHDRAWN by amendment 1 (V3 failed on B3, so the arm cannot be applied to the family); not run"]
fn m_rw_b2() {
    b2_arm("M-RW");
}
