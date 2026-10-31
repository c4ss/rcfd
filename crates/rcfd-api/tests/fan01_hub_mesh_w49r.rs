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

fn tag_json(t: (u8, usize, usize, usize)) -> Value {
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
    let shifted = |cells: &[usize], f: &dyn Fn((u8, usize, usize, usize)) -> (u8, usize, usize, usize)| -> Vec<usize> {
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
    for (id, c, cells, hash) in [
        ("B3", F3, 4920, 0xbe64f588a252d0b4u64),
        ("B2", F2, 13627, 0xcae52c4997c9463fu64),
    ] {
        let case = Case::parse_toml(&case_toml(c)).unwrap();
        let m = rcfd_api::sim::build_mesh(&case).unwrap();
        println!("W49R {id} cells {} hash {:#018x}", m.n_cells(), m.content_hash());
        assert_eq!(
            (m.n_cells(), m.content_hash()),
            (cells, hash),
            "{id}: default mesh changed"
        );
    }
}
