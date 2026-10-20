//! Wave 36D (ADR 0083 part A): the cause of the FAN-01 laminar smoke miss of ADR 0079 (P10). The P10 set-up through the case on scaled counts
//! and at ten times the viscosity, with the argmax cell of the cell mass imbalance located every iteration. Measurements only (the verdict rule is
//! registered in ADR 0083); no accuracy and no fan performance claim.

#![allow(clippy::neg_cmp_op_on_partial_ord)]

use rcfd_api::case::Case;
use rcfd_api::fan_case::{cgrid_spec_of, fan_of};
use rcfd_api::rules::{check_case, has_errors, mesh_context, Severity};
use rcfd_api::sim::build_mesh;
use rcfd_api::sim::Simulation;
use rcfd_fv::FvGeometry;
use rcfd_mesh::fan_cgrid::{build_cgrid_mesh_with, CGridOptions};
use rcfd_mesh::quality::check_mesh;
use rcfd_mesh::quality::compute_metrics;
use std::f64::consts::PI;
use std::sync::Arc;

const OMEGA_RPM: f64 = 1486.0;
const Q_DESIGN: f64 = 1.4032;

fn omega() -> f64 {
    OMEGA_RPM * 2.0 * PI / 60.0
}

fn annulus_area() -> f64 {
    PI * (0.25f64.powi(2) - 0.124f64.powi(2))
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

/// Counts (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap); the rest as FAN-01-C (ADR 0079).
type Counts = (usize, usize, usize, usize, usize, usize, usize);

/// FAN-01-C counts (ADR 0075 amendment 1).
const X1: Counts = (8, 16, 8, 10, 4, 14, 7);

/// The P10 case of ADR 0079 with the given counts, construction and laminar viscosity (the mesh keeps the y+ rule at nu = 1.5e-5).
fn smoke_case(c: Counts, construction: &str, nu: f64) -> String {
    let rows = sections();
    let (x0, t0) = (rows[0][6], rows[0][7]);
    let w = omega();
    let u_in = Q_DESIGN / annulus_area();
    let ref_speed = u_in.hypot(w * 0.186);
    let (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) = c;
    let mesh = format!(
        "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\nn_blades = 9\nr_hub = 0.124\nr_tip = 0.2475\ntip_clearance = 0.0025\naxial_extent = 0.4\nz_le_root = 0.1\nfamily = \"naca4\"\n\
station_radius = {}\nstation_chord = {}\nstation_stagger = {}\nstation_camber = {}\nstation_camber_pos = {}\nstation_thickness = {}\nstation_sweep = {}\nstation_lean = {}\n\
topology = \"c_grid\"\nc_grid_construction = \"{construction}\"\nn_up = {n_up}\nn_chord = {n_chord}\nn_down = {n_down}\nn_side = {n_side}\nn_nose = {n_nose}\nn_core = 1\nfront_gap = 0.3\nn_span = {n_span}\nn_gap = {n_gap}\nn_bl = 2\ngrowth = 1.2\n\
first_cell_height = 0.0\ny_plus = 30.0\nref_speed = {ref_speed:?}\nnu = 1.5e-5\n",
        list(&rows, |s| s[0] * 1e-3),
        list(&rows, |s| s[1] * 1e-3),
        list(&rows, |s| s[2]),
        list(&rows, |s| s[3]),
        list(&rows, |s| s[4]),
        list(&rows, |s| s[5]),
        list(&rows, |s| (s[6] - x0) * 1e-3),
        list(&rows, |s| s[0] * 1e-3 * (s[7] - t0).to_radians()),
    );
    format!(
        r#"{mesh}
[physics]
nu = {nu:?}
rho = 1.2
[initial]
velocity = [0.0, 0.0, {u_in:?}]
velocity_expression_x = "{w:?} [1/s] * y"
velocity_expression_y = "-{w:?} [1/s] * x"
[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.0, 0.0, {u_in:?}]
velocity_expression_x = "{w:?} [1/s] * y"
velocity_expression_y = "-{w:?} [1/s] * x"
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.hub]
velocity = "fixed"
[boundaries.blade]
velocity = "fixed"
[boundaries.shroud]
velocity = "fixed"
velocity_expression_x = "{w:?} [1/s] * y"
velocity_expression_y = "-{w:?} [1/s] * x"
[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = {w:?}
centrifugal = true
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 300
tolerance_u = 1e-12
tolerance_p = 1e-12
[reports.m_in]
type = "mass_flow"
patch = "inlet"
[reports.m_out]
type = "mass_flow"
patch = "outlet"
"#
    )
}

/// Options of a case construction value (as `fan_case::build`).
fn options_of(construction: &str) -> CGridOptions {
    match construction {
        "camber_seam" => CGridOptions::camber_seam_construction(),
        "camber_seam_h" => CGridOptions::camber_seam_h_construction(),
        other => panic!("construction {other} not handled by the diagnosis"),
    }
}

/// Result of one diagnosis run.
#[derive(Debug)]
struct P10Run {
    c10: f64,
    late: f64,
    c10_np: f64,
    late_np: f64,
    f_poor: f64,
    distinct: usize,
    failure: bool,
    finite: bool,
}

/// One run of ADR 0083 part A: the P10 criterion plus the argmax localisation.
fn run(tag: &str, c: Counts, construction: &str, nu: f64, iters: usize) -> P10Run {
    run_with(tag, c, construction, nu, iters, "")
}

/// [`run`] with extra lines at the top of the `[solver]` table (ADR 0084: `velocity_correction = "face_flux"`).
fn run_with(tag: &str, c: Counts, construction: &str, nu: f64, iters: usize, solver_lines: &str) -> P10Run {
    let text = smoke_case(c, construction, nu).replacen("[solver]
", &format!("[solver]
{solver_lines}"), 1);
    let case = Case::parse_toml(&text).unwrap_or_else(|e| panic!("{e:?}"));
    let cg = build_cgrid_mesh_with(&fan_of(&case), &cgrid_spec_of(&case), &options_of(construction)).unwrap();
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let mesh = sim.geom.mesh.clone();
    assert_eq!(mesh.n_cells(), cg.mesh.n_cells(), "tags must match the solver mesh");
    let met = compute_metrics(&mesh);
    let n = mesh.n_cells();
    let n_int = mesh.n_internal_faces();
    let mut poor = vec![false; n];
    for f in 0..mesh.n_faces() {
        if met.non_orthogonality[f] > 70.0 || met.skewness[f] > 2.0 {
            poor[mesh.owner()[f]] = true;
            if f < n_int {
                poor[mesh.neighbour()[f]] = true;
            }
        }
    }
    let mut region = poor.clone();
    for f in 0..n_int {
        let (o, nb) = (mesh.owner()[f], mesh.neighbour()[f]);
        if poor[o] || poor[nb] {
            region[o] = true;
            region[nb] = true;
        }
    }
    let area = mesh
        .face_area_vectors()
        .iter()
        .map(|s| (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt())
        .fold(0.0, f64::max);
    let worst_face_no = |c: usize| -> f64 {
        mesh.cell_faces(c)
            .iter()
            .map(|&f| met.non_orthogonality[f])
            .fold(0.0, f64::max)
    };
    println!(
        "A {tag}: {n} cells, {} poor cells, {} in the poor region",
        poor.iter().filter(|&&p| p).count(),
        region.iter().filter(|&&p| p).count()
    );
    let (mut c10, mut late, mut c10_np, mut late_np, mut finite) = (f64::NAN, 0.0f64, f64::NAN, 0.0f64, true);
    let (mut in_poor, mut cnt) = (0usize, 0usize);
    let mut distinct: Vec<usize> = Vec::new();
    let mut below = None;
    for it in 1..=iters {
        let st = sim.step();
        if below.is_none() && st.res_u < 1e-4 && st.res_p < 1e-4 {
            below = Some(it);
            println!("A {tag}: res_u and res_p below 1e-4 first at iteration {it}");
        }
        if !(st.continuity.is_finite() && st.res_u.is_finite() && st.res_p.is_finite()) {
            finite = false;
            println!("A {tag} non-finite at iteration {it}");
            break;
        }
        let phi = sim.flux();
        let (mut best, mut arg, mut best_np) = (0.0f64, 0usize, 0.0f64);
        for c in 0..n {
            let mut s = 0.0;
            for &f in mesh.cell_faces(c) {
                s += if f >= n_int || mesh.owner()[f] == c {
                    phi[f]
                } else {
                    -phi[f]
                };
            }
            let a = s.abs() / area;
            if a > best {
                best = a;
                arg = c;
            }
            if !region[c] {
                best_np = best_np.max(a);
            }
        }
        if it == 10 {
            c10 = st.continuity;
            c10_np = best_np;
        }
        if (200..=300).contains(&it) {
            late = late.max(st.continuity);
            late_np = late_np.max(best_np);
            cnt += 1;
            if region[arg] {
                in_poor += 1;
            }
            if !distinct.contains(&arg) {
                distinct.push(arg);
            }
        }
        if it % 10 == 0 || it <= 3 {
            let t = cg.cell_tags[arg];
            println!(
                "A {tag} it {it}: continuity {:.3e} (recomputed {best:.3e}) argmax cell {arg} tag {t:?} poor {} region {} worst face {:.1} deg; outside the region {best_np:.3e}; res_u {:.3e} res_p {:.3e}",
                st.continuity,
                poor[arg],
                region[arg],
                worst_face_no(arg),
                st.res_u,
                st.res_p
            );
        }
    }
    let (ux, uy, uz) = (
        sim.field("ux").unwrap(),
        sim.field("uy").unwrap(),
        sim.field("uz").unwrap(),
    );
    let (umax, cmax) = (0..ux.len())
        .map(|c| ((ux[c] * ux[c] + uy[c] * uy[c] + uz[c] * uz[c]).sqrt(), c))
        .fold((0.0, 0), |a, b| if b.0 > a.0 { b } else { a });
    // the cells that were the argmax over 200-300: their aspect ratio, worst face and largest volume ratio to a face neighbour, against the
    // mesh-wide maxima of the same measures
    let vol = mesh.cell_volumes();
    let vratio = |c: usize| -> f64 {
        mesh.cell_faces(c)
            .iter()
            .filter(|&&f| f < n_int)
            .map(|&f| {
                let o = if mesh.owner()[f] == c {
                    mesh.neighbour()[f]
                } else {
                    mesh.owner()[f]
                };
                (vol[o] / vol[c]).max(vol[c] / vol[o])
            })
            .fold(1.0, f64::max)
    };
    let all_vr = (0..n).map(vratio).fold(1.0, f64::max);
    let all_ar = met.aspect_ratio.iter().cloned().fold(0.0, f64::max);
    for &c in &distinct {
        println!(
            "A {tag} argmax cell {c} tag {:?}: aspect {:.1} (mesh max {all_ar:.1}), worst face {:.1} deg, volume ratio to a neighbour {:.2} (mesh max {all_vr:.2})",
            cg.cell_tags[c],
            met.aspect_ratio[c],
            worst_face_no(c),
            vratio(c)
        );
    }
    let r = P10Run {
        c10,
        late,
        c10_np,
        late_np,
        f_poor: in_poor as f64 / cnt.max(1) as f64,
        distinct: distinct.len(),
        failure: sim.failure().is_some(),
        finite,
    };
    println!(
        "A {tag} RESULT: c10 {:.3e} L {:.3e} L/c10 {:.3}; outside the poor region c10_np {:.3e} L_np {:.3e} ratio {:.3}; f_poor {:.2}; distinct argmax cells over 200-300 {}; largest relative speed {umax:.2} m/s at cell {cmax} tag {:?} (poor {}); failure {:?}",
        r.c10,
        r.late,
        r.late / r.c10,
        r.c10_np,
        r.late_np,
        r.late_np / r.c10_np,
        r.f_poor,
        r.distinct,
        cg.cell_tags[cmax],
        poor[cmax],
        sim.failure()
    );
    r
}

/// ADR 0083 part A, runs A0-A5 (each once). About 8 minutes in release.
#[test]
#[ignore = "diagnosis of ADR 0083 part A, run by hand: cargo test --release -p rcfd-api --test fan01_p10_w36d part_a -- --ignored --nocapture"]
fn part_a_p10_cause() {
    let only = std::env::var("PART_A_ONLY").unwrap_or_default();
    let runs: [(&str, Counts, f64, usize); 6] = [
        ("A0 x1", X1, 1.5e-5, 300),
        ("A1 x0.75", (6, 12, 6, 8, 3, 10, 5), 1.5e-5, 300),
        ("A2 x1.25", (10, 20, 10, 12, 5, 18, 9), 1.5e-5, 300),
        ("A3 x1.5", (12, 24, 12, 15, 6, 21, 10), 1.5e-5, 300),
        ("A4 x1 nu x10", X1, 1.5e-4, 300),
        ("A5 x1 1500 iterations", X1, 1.5e-5, 1500),
    ];
    for (tag, c, nu, it) in runs {
        if !only.is_empty() && !tag.starts_with(&only) {
            continue;
        }
        run(tag, c, "camber_seam", nu, it);
    }
}

/// ADR 0084 (reported, not a gate): the ADR 0083 A0 set-up (FAN-01-C, `camber_seam`) with `solver.velocity_correction = "face_flux"`, 300 and 1500
/// iterations. Registered reading: P10 met iff L / c10 <= 1 over 300; "converges" iff res_u and res_p fall below 1e-4 within 1500 iterations.
#[test]
#[ignore = "ADR 0084 FAN-01 report, run by hand: cargo test --release -p rcfd-api --test fan01_p10_w36d w37a -- --ignored --nocapture"]
fn w37a_fan01_with_face_flux_velocity_correction() {
    // one 1500-iteration run: c10 and L (200-300) are read from its first 300 iterations, which a 300-iteration run repeats exactly
    run_with("W37A x1 face_flux", X1, "camber_seam", 1.5e-5, 1500, "velocity_correction = \"face_flux\"
");
}

/// ADR 0083 C2 (P9 of ADR 0079 with construction H): `FvGeometry::new` accepts the FAN-01-C mesh, `check_mesh` is ok and the rules raise no
/// error-level finding, through the case with `c_grid_construction = "camber_seam_h"`.
#[test]
fn c2_fan01_h_mesh_is_accepted_by_the_solver_and_the_rules() {
    let case = Case::parse_toml(&smoke_case(X1, "camber_seam_h", 1.5e-5)).unwrap_or_else(|e| panic!("{e:?}"));
    let mesh = build_mesh(&case).unwrap_or_else(|e| panic!("C1 FAN-01 not built with H: {e:?}"));
    let q = check_mesh(&mesh);
    let rules = check_case(&case, Some(&mesh_context(&q)));
    println!(
        "C2 FAN-01 (camber_seam_h): {} cells; check_mesh ok {} (errors {:?}); non-orthogonality {:.2}, skewness {:.3}, aspect {:.1}; {} rule findings",
        q.n_cells,
        q.is_ok(),
        q.errors,
        q.max_non_orthogonality,
        q.max_skewness,
        q.max_aspect_ratio,
        rules.len()
    );
    for r in &rules {
        println!("C2 finding {} {:?} {}: {}", r.id, r.severity, r.path, r.message);
    }
    assert!(
        FvGeometry::new(Arc::new(mesh)).is_ok(),
        "FvGeometry::new refuses the FAN-01 mesh"
    );
    assert!(q.is_ok(), "check_mesh rejects the FAN-01 mesh: {:?}", q.errors);
    assert!(
        !has_errors(&rules) && rules.iter().all(|r| r.severity != Severity::Error),
        "error-level rule finding"
    );
}

/// ADR 0083 C3 (P10 of ADR 0079 with construction H): the laminar smoke solve stays bounded by the ADR 0075 amendment 3 criterion. KNOWN MISS:
/// continuity 7.96e-4 at iteration 10, 7.46e-3 over 200-300 (finite, no failure; the part A oscillation, argmax away from the poor faces).
#[test]
#[ignore = "known miss: FAN-01-C with H laminar smoke continuity 7.96e-4 at 10, 7.46e-3 over 200-300 (finite, no failure), ADR 0083 addendum"]
fn c3_fan01_h_laminar_smoke_solve_stays_bounded_known_miss() {
    let r = run("C3 H", X1, "camber_seam_h", 1.5e-5, 300);
    assert!(!r.failure && r.finite);
    assert!(
        r.late <= r.c10,
        "continuity grew: {:e} over 200-300 against {:e} at iteration 10",
        r.late,
        r.c10
    );
}
