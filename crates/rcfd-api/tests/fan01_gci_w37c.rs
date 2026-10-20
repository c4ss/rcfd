//! Wave 37C (ADR 0086): the first solution-verification study of FAN-01. A three-mesh family of the C-grid passage (registered counts and
//! refinement ratio), FAN-01 at the design point with k-omega SST and the Barth-Jespersen-limited `linear_upwind` (ADR 0081) on each mesh, and the
//! same family for the empty passage (the X2a-type check of ADR 0078, exact absolute swirl zero). Richardson extrapolation, observed order and the
//! fine-grid convergence index (Roache's GCI with the factor of safety 1.25, in the three-grid form of Celik et al. 2008 for unequal ratios).
//! Everything was registered in ADR 0086 before the runs. No agreement with the experiment is claimed.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::{build_mesh, Simulation};
use rcfd_core::vec3;
use rcfd_mesh::quality::check_mesh;
use std::f64::consts::PI;
use std::time::Instant;

const N_BLADES: f64 = 9.0;
const R_HUB: f64 = 0.124;
const R_DUCT: f64 = 0.250;
const RHO: f64 = 1.2; // ASSUMED (ADR 0075), UNVERIFIED
const Q_DESIGN: f64 = 1.4032;
/// Measured design point (ADR 0075 amendment 1, `w34d_fan01_characteristic_n1ug.csv`): Delta p_ts [Pa], shaft power [W].
const DP_TS_MEAS: f64 = 126.46;
const POWER_MEAS: f64 = 333.30;
const CAP: usize = 8000;
const WALL_CLOCK_MIN: f64 = 60.0;
/// Roache's factor of safety for a three-grid study with an observed order.
const FS: f64 = 1.25;

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

/// Counts (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap); n_core 1, n_bl 2, growth 1.2, front_gap 0.3 and the y+ = 30 first cell fixed.
type Counts = (usize, usize, usize, usize, usize, usize, usize);

/// FAN-01-C (ADR 0075 amendment 1, ADR 0079).
const X1: Counts = (8, 16, 8, 10, 4, 14, 7);

/// Every count of X1 times `s`, rounded to the nearest integer (halves away from zero).
fn scaled(s: f64) -> Counts {
    let r = |n: usize| (n as f64 * s).round() as usize;
    (r(X1.0), r(X1.1), r(X1.2), r(X1.3), r(X1.4), r(X1.5), r(X1.6))
}

/// Candidate count sets of the smoke builds (ADR 0086 disclosure): the x0.75, x1.25 and x1.5 sets of ADR 0083 and the nominal sqrt(2) steps.
fn candidates() -> Vec<(&'static str, Counts)> {
    vec![
        ("x0.75", (6, 12, 6, 8, 3, 10, 5)),
        ("x0.75r2", scaled(0.75 * 2f64.sqrt())),
        ("x1", X1),
        ("x1.25", (10, 20, 10, 12, 5, 18, 9)),
        ("x1.5", (12, 24, 12, 15, 6, 21, 10)),
    ]
}

/// The registered family (ADR 0086): (label, counts), fine first.
fn family() -> [(&'static str, Counts); 3] {
    [
        ("F1 x1.5", (12, 24, 12, 15, 6, 21, 10)),
        ("F2 x0.75r2", scaled(0.75 * 2f64.sqrt())),
        ("F3 x0.75", (6, 12, 6, 8, 3, 10, 5)),
    ]
}

/// The registered C-grid construction.
const CONSTRUCTION: &str = "camber_seam_h";

fn mesh_toml(c: Counts, construction: &str, blade: bool) -> String {
    let rows = sections();
    let (x0, t0) = (rows[0][6], rows[0][7]);
    let ref_speed = (Q_DESIGN / annulus_area()).hypot(omega() * 0.186);
    let (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) = c;
    format!(
        "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\nn_blades = 9\nr_hub = 0.124\nr_tip = 0.2475\ntip_clearance = 0.0025\naxial_extent = 0.4\nz_le_root = 0.1\nfamily = \"naca4\"\n\
station_radius = {}\nstation_chord = {}\nstation_stagger = {}\nstation_camber = {}\nstation_camber_pos = {}\nstation_thickness = {}\nstation_sweep = {}\nstation_lean = {}\n\
topology = \"c_grid\"\nc_grid_construction = \"{construction}\"\nn_up = {n_up}\nn_chord = {n_chord}\nn_down = {n_down}\nn_side = {n_side}\nn_nose = {n_nose}\nn_core = 1\nfront_gap = 0.3\nn_span = {n_span}\nn_gap = {n_gap}\nn_bl = 2\ngrowth = 1.2\n\
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

/// The registered case (ADR 0086): design point, `rotating_frame`, k-omega SST, SIMPLEC, `linear_upwind` + Barth-Jespersen limiter. `blade =
/// false` is the empty passage (X2a type): hub and shroud at rest in the absolute frame, exact absolute swirl zero.
fn case_toml(c: Counts, construction: &str, blade: bool) -> String {
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
        "[boundaries.blade]\nvelocity = \"fixed\"\n[reports.m_blade]\ntype = \"moment\"\npatch = \"blade\"\ndirection = [0.0, 0.0, 1.0]\n"
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
linear_upwind_limiter = "barth_jespersen"
max_iterations = {CAP}
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
        mesh_toml(c, construction, blade)
    )
}

// ---------------------------------------------------------------- Richardson extrapolation and GCI (registered in ADR 0086)

/// Three-grid result for one functional; index 1 = fine, 3 = coarse.
#[derive(Debug, Clone, Copy)]
struct Gci {
    /// sign of eps32 / eps21 (+1 monotone, -1 oscillatory)
    s: f64,
    /// observed order (NaN if the fixed point does not converge)
    p: f64,
    /// Richardson-extrapolated value with the observed order
    f_ext: f64,
    /// governing band half-width (absolute): Fs |eps21| / (r21^p_band - 1), p_band = min(p, 1), Fs 1.25 if p in [0.9, 2.2] else 3
    band: f64,
    /// reported alternative: Fs 1.25, p_band = min(p, 2)
    band_alt: f64,
    fs: f64,
}

/// Observed order from three solutions on grids with ratios r21 = h2/h1, r32 = h3/h2 (Celik et al. 2008, fixed-point form; recalled, UNVERIFIED).
fn observed_order(f1: f64, f2: f64, f3: f64, r21: f64, r32: f64) -> (f64, f64) {
    let (e21, e32) = (f2 - f1, f3 - f2);
    let s = (e32 / e21).signum();
    let mut p = 1.0f64;
    for _ in 0..500 {
        let q = ((r21.powf(p) - s) / (r32.powf(p) - s)).ln();
        let pn = ((e32 / e21).abs().ln() + q).abs() / r21.ln();
        if !pn.is_finite() {
            return (s, f64::NAN);
        }
        if (pn - p).abs() < 1e-12 {
            return (s, pn);
        }
        p = pn;
    }
    (s, f64::NAN)
}

fn gci(f1: f64, f2: f64, f3: f64, r21: f64, r32: f64) -> Gci {
    let (s, p) = observed_order(f1, f2, f3, r21, r32);
    let f_ext = (r21.powf(p) * f1 - f2) / (r21.powf(p) - 1.0);
    // formal order of the coupled scheme is 1 (SST convected with upwind), of the momentum convection 2
    let fs = if (0.9..=2.2).contains(&p) { FS } else { 3.0 };
    Gci {
        s,
        p,
        f_ext,
        band: fs * (f1 - f2).abs() / (r21.powf(p.min(1.0)) - 1.0),
        band_alt: FS * (f1 - f2).abs() / (r21.powf(p.min(2.0)) - 1.0),
        fs,
    }
}

/// Self-check of the order solver on an exact power law with unequal ratios (and its oscillatory flag).
#[test]
fn gci_recovers_a_known_power_law() {
    for &(p_true, r21, r32) in &[(1.6, 1.49, 1.33), (2.0, 1.41, 1.41), (0.9, 1.2, 1.6)] {
        let f = |h: f64| 3.0 + 0.7 * h.powf(p_true);
        let (h1, h2) = (0.01, 0.01 * r21);
        let h3 = h2 * r32;
        let g = gci(f(h1), f(h2), f(h3), r21, r32);
        println!("p_true {p_true}: {g:?}");
        assert!(g.s > 0.0);
        assert!((g.p - p_true).abs() < 1e-8, "{g:?}");
        assert!((g.f_ext - 3.0).abs() < 1e-10, "{g:?}");
    }
    let g = gci(1.0, 1.1, 1.05, 1.4, 1.4);
    assert!(g.s < 0.0);
}

// ---------------------------------------------------------------- smoke builds (ADR 0086 rule (d) disclosure)

/// Construction smoke of every candidate count set with both C-grid constructions, bladed and empty: cells, quality maxima, `check_mesh`.
#[test]
#[ignore = "construction smoke of ADR 0086 (about 1-2 min in release): cargo test --release -p rcfd-api --test fan01_gci_w37c smoke_build -- --ignored --nocapture"]
fn smoke_build_candidates() {
    for construction in ["camber_seam", "camber_seam_h"] {
        for (label, c) in candidates() {
            for blade in [true, false] {
                let t0 = Instant::now();
                let case = Case::parse_toml(&case_toml(c, construction, blade)).unwrap_or_else(|e| panic!("{e:?}"));
                match build_mesh(&case) {
                    Ok(m) => {
                        let q = check_mesh(&m);
                        println!(
                            "BUILD {construction} {label} {c:?} blade {blade}: {} cells, blade faces {}, non-orth {:.2}, skewness {:.3}, aspect {:.1}, check_mesh ok {} ({:.1} s)",
                            m.n_cells(),
                            m.patch("blade").map(|p| p.faces().count()).unwrap_or(0),
                            q.max_non_orthogonality,
                            q.max_skewness,
                            q.max_aspect_ratio,
                            q.is_ok(),
                            t0.elapsed().as_secs_f64()
                        );
                    }
                    Err(e) => println!("BUILD {construction} {label} {c:?} blade {blade}: REFUSED {e:?}"),
                }
            }
        }
    }
}

/// The registered family builds with the registered construction, bladed and empty, and `check_mesh` accepts every mesh (ADR 0086 S1).
#[test]
fn s1_family_builds_and_is_accepted() {
    let mut cells = Vec::new();
    for (label, c) in family() {
        let mut n = [0usize; 2];
        for (i, blade) in [true, false].into_iter().enumerate() {
            let case =
                Case::parse_toml(&case_toml(c, CONSTRUCTION, blade)).unwrap_or_else(|e| panic!("{label}: {e:?}"));
            let m = build_mesh(&case).unwrap_or_else(|e| panic!("{label} blade {blade}: {e:?}"));
            let q = check_mesh(&m);
            let blade_faces = m.patch("blade").map(|p| p.faces().count()).unwrap_or(0);
            println!(
                "S1 {label} blade {blade}: {} cells, blade faces {blade_faces}, non-orth {:.2}, skewness {:.3}, aspect {:.1}",
                m.n_cells(),
                q.max_non_orthogonality,
                q.max_skewness,
                q.max_aspect_ratio
            );
            assert!(q.is_ok(), "{label} blade {blade}: {:?}", q.errors);
            assert_eq!(
                blade_faces > 0,
                blade,
                "{label}: the empty passage must have no blade wall"
            );
            n[i] = m.n_cells();
        }
        assert!(n[1] > n[0], "{label}: the empty passage fills the blade footprint");
        cells.push(n[0] as f64);
    }
    let (r21, r32) = ((cells[0] / cells[1]).cbrt(), (cells[1] / cells[2]).cbrt());
    println!("S1 effective ratios r21 {r21:.4}, r32 {r32:.4}");
    assert!(r21 >= 1.3 && r32 >= 1.3, "ratios {r21} {r32}");
}

/// 300-iteration solver smoke of every registered run with the ADR 0077 part B / ADR 0081 L1 criterion (ADR 0086 rule (d)): STABLE iff no
/// guard trip, max cell speed over 10..300 <= 3 x the largest boundary-face speed, continuity finite with its maximum over 200..300 not above its
/// value at 10.
#[test]
#[ignore = "solver smoke of ADR 0086 (about 10 min in release): cargo test --release -p rcfd-api --test fan01_gci_w37c smoke_solve -- --ignored --nocapture"]
fn smoke_solve_family() {
    let only = std::env::var("W37C_SMOKE").unwrap_or_default();
    for (label, c) in family() {
        for blade in [true, false] {
            if !only.is_empty() && !label.starts_with(&only) {
                continue;
            }
            let t0 = Instant::now();
            let case = Case::parse_toml(&case_toml(c, CONSTRUCTION, blade)).unwrap_or_else(|e| panic!("{e:?}"));
            let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
            let g = &sim.geom;
            let ub = sim.velocity().boundary_values(g);
            let u_bnd = (0..ub.len())
                .filter(|&i| !g.empty[g.n_internal + i])
                .map(|i| vec3::norm(ub[i]))
                .fold(0.0, f64::max);
            let (mut umax, mut c10, mut late, mut finite, mut why) = (0.0f64, f64::NAN, 0.0f64, true, String::new());
            let mut line = String::new();
            for it in 1..=300 {
                let st = sim.step();
                if it == 1 || it == 10 || it % 50 == 0 {
                    let (a, b) = functionals(&sim, blade);
                    line += &format!(
                        " it {it} cont {:.3e} res_u {:.2e} f_a {a:.4} f_b {b:.3};",
                        st.continuity, st.res_u
                    );
                }
                if let Some(d) = sim.steady_divergence() {
                    why = format!(" guard trip at {it}: {} {}", d.kind, d.quantity);
                    break;
                }
                if !st.continuity.is_finite() {
                    finite = false;
                }
                if it >= 10 {
                    let s = sim
                        .velocity()
                        .values
                        .iter()
                        .map(|u| vec3::norm(*u))
                        .fold(0.0f64, |a, s| if s.is_nan() { f64::INFINITY } else { a.max(s) });
                    umax = umax.max(s);
                }
                if it == 10 {
                    c10 = st.continuity;
                }
                if it >= 200 {
                    late = late.max(st.continuity);
                }
            }
            let stable = why.is_empty() && finite && umax <= 3.0 * u_bnd && late <= c10;
            println!(
                "SMOKE {label} blade {blade}: {} cells, {} (max speed {umax:.2} vs 3 x {u_bnd:.2}; continuity at 10 {c10:.3e}, max 200..300 {late:.3e}){why} ({:.1} s, {:.3} s/it);{line}",
                sim.geom.n_cells(),
                if stable { "STABLE" } else { "UNSTABLE" },
                t0.elapsed().as_secs_f64(),
                t0.elapsed().as_secs_f64() / 300.0
            );
        }
    }
}

// ---------------------------------------------------------------- surface statistics (the ADR 0078 definitions)

struct Sample {
    x: [f64; 3],
    c: [f64; 3],
    p: f64,
    phi: f64,
    a: f64,
}

fn absolute(w: [f64; 3], x: [f64; 3], om: f64) -> [f64; 3] {
    [w[0] - om * x[1], w[1] + om * x[0], w[2]]
}

/// (radial, tangential, r)
fn polar(c: [f64; 3], x: [f64; 3]) -> (f64, f64, f64) {
    let r = x[0].hypot(x[1]);
    ((x[0] * c[0] + x[1] * c[1]) / r, (-x[1] * c[0] + x[0] * c[1]) / r, r)
}

fn patch_samples(sim: &Simulation, name: &str, om: f64) -> Vec<Sample> {
    let g = &sim.geom;
    let ni = g.n_internal;
    let ub = sim.velocity().boundary_values(g);
    let pb = sim.boundary_field("p").unwrap();
    let phi = sim.flux();
    let (cf, sf) = (g.mesh.face_centres(), g.mesh.face_area_vectors());
    g.mesh
        .patch(name)
        .unwrap()
        .faces()
        .map(|f| {
            let s = if sf[f][2] >= 0.0 { 1.0 } else { -1.0 };
            Sample {
                x: cf[f],
                c: absolute(ub[f - ni], cf[f], om),
                p: pb[f - ni],
                phi: s * phi[f],
                a: sf[f][2].abs(),
            }
        })
        .collect()
}

/// Full-annulus surface statistics: (Q, area-averaged p [Pa], mass-averaged absolute total pressure [Pa], rho <c_theta^2/2>_m [Pa],
/// angular-momentum flux L [N m]).
fn stats(s: &[Sample]) -> (f64, f64, f64, f64, f64) {
    let q: f64 = s.iter().map(|v| v.phi).sum();
    let area: f64 = s.iter().map(|v| v.a).sum();
    let m = |f: &dyn Fn(&Sample) -> f64| s.iter().map(|v| f(v) * v.phi).sum::<f64>() / q;
    let kth = m(&|v| 0.5 * polar(v.c, v.x).1.powi(2));
    let pt = m(&|v| v.p + 0.5 * (v.c[0] * v.c[0] + v.c[1] * v.c[1] + v.c[2] * v.c[2]));
    let l = N_BLADES
        * RHO
        * s.iter()
            .map(|v| polar(v.c, v.x).2 * polar(v.c, v.x).1 * v.phi)
            .sum::<f64>();
    (
        N_BLADES * q,
        RHO * s.iter().map(|v| v.p * v.a).sum::<f64>() / area,
        RHO * pt,
        RHO * kth,
        l,
    )
}

// ---------------------------------------------------------------- one registered run

/// Functionals of one run and their last-window iteration changes.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // printed through Debug
struct Outcome {
    converged: bool,
    how: String,
    cells: usize,
    iterations: usize,
    minutes: f64,
    /// bladed: torque [N m]; empty: outlet angular-momentum flux L [N m]
    f_a: f64,
    /// bladed: Delta p_ts [Pa]; empty: outlet rho <c_theta^2/2>_m [Pa]
    f_b: f64,
    nu_a: f64,
    nu_b: f64,
}

/// Bladed: (torque, Delta p_ts) with the ADR 0075 definitions; empty: (outlet L, outlet swirl energy). `om` is the frame rate.
fn functionals(sim: &Simulation, blade: bool) -> (f64, f64) {
    let w = omega();
    let u_in = Q_DESIGN / annulus_area();
    let mon = |k: &str| sim.monitors.last(k).unwrap_or(f64::NAN);
    if blade {
        (
            -N_BLADES * (mon("m_blade") + mon("m_hub")),
            RHO * (mon("p_out") - (mon("p_in") + 0.5 * u_in * u_in)),
        )
    } else {
        let (_, _, _, kth, l) = stats(&patch_samples(sim, "outlet", w));
        (l, kth)
    }
}

fn run(label: &str, c: Counts, blade: bool) -> Outcome {
    let case = Case::parse_toml(&case_toml(c, CONSTRUCTION, blade)).unwrap_or_else(|e| panic!("{label}: {e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{label}: {e:?}"));
    let cells = sim.geom.mesh.n_cells();
    let t0 = Instant::now();
    let mut hist: Vec<(usize, f64, f64)> = Vec::new();
    let (mut n, mut stop) = (0usize, None);
    while n < CAP && !sim.is_done() {
        let st = sim.step();
        n += 1;
        if n % 50 == 0 || sim.is_done() {
            let (a, b) = functionals(&sim, blade);
            hist.push((n, a, b));
        }
        if n % 500 == 0 || n <= 3 || n == 10 || n == 30 {
            let (a, b) = functionals(&sim, blade);
            println!(
                "W37C {label} it {n}: continuity {:.3e} res_u {:.3e} res_p {:.3e} f_a {a:.6} f_b {b:.4} ({:.1} min)",
                st.continuity,
                st.res_u,
                st.res_p,
                t0.elapsed().as_secs_f64() / 60.0
            );
        }
        if let Some(d) = sim.steady_divergence() {
            stop = Some(format!("steady guard trip at {n}: {} {}", d.kind, d.quantity));
            break;
        }
        if !(st.continuity.is_finite() && st.res_u.is_finite() && st.res_p.is_finite()) || st.continuity > 1e6 {
            stop = Some(format!("diverged: continuity {:.3e} at iteration {n}", st.continuity));
            break;
        }
        if t0.elapsed().as_secs_f64() > 60.0 * WALL_CLOCK_MIN {
            stop = Some(format!("NOT CONVERGED (time): wall-clock cap at iteration {n}"));
            break;
        }
    }
    let minutes = t0.elapsed().as_secs_f64() / 60.0;
    let (f_a, f_b) = functionals(&sim, blade);
    if hist.last().map(|h| h.0) != Some(n) {
        hist.push((n, f_a, f_b));
    }
    // iteration change over the registered window: the last 1000 iterations, or the last half of a shorter run (sampled every 50)
    let window = 1000.min(n / 2);
    let back = hist
        .iter()
        .rev()
        .find(|h| h.0 + window <= n)
        .copied()
        .unwrap_or(hist[0]);
    let (nu_a, nu_b) = ((f_a - back.1).abs(), (f_b - back.2).abs());
    let (m_in, m_out) = (
        sim.monitors.last("m_in").unwrap_or(f64::NAN),
        sim.monitors.last("m_out").unwrap_or(f64::NAN),
    );
    let mb = (m_in + m_out).abs() / m_in.abs();
    let (converged, how) = if let Some(s) = stop {
        (false, s)
    } else if sim.failure().is_some() {
        (false, format!("failure after {n} iterations: {:?}", sim.failure()))
    } else if sim.is_converged() {
        (true, format!("residual tolerance reached in {n} iterations"))
    } else {
        // at the cap (ADR 0075 rule; empty passage: the ADR 0086 absolute floors for functionals whose exact value is zero)
        let ok_a = if blade {
            nu_a < 5e-3 * f_a.abs()
        } else {
            nu_a <= (5e-3 * f_a.abs()).max(1e-4)
        };
        let ok_b = if blade {
            nu_b < 5e-3 * f_b.abs()
        } else {
            nu_b <= (5e-3 * f_b.abs()).max(1e-2)
        };
        let ok = n >= CAP && ok_a && ok_b && mb <= 1e-3;
        (
            ok,
            format!(
                "at {n} iterations (cap rule): change f_a {nu_a:.3e}, f_b {nu_b:.3e} over {window}, mass balance {mb:.3e}; last {:?}",
                sim.last()
            ),
        )
    };
    let w = omega();
    let (qi, pi_area, pt_in, _, l_in) = stats(&patch_samples(&sim, "inlet", w));
    let (qo, po_area, pt_out, kth_out, l_out) = stats(&patch_samples(&sim, "outlet", w));
    let mon = |k: &str| sim.monitors.last(k).unwrap_or(f64::NAN);
    let wall = -N_BLADES * (mon("m_hub") + mon("m_shroud") + if blade { mon("m_blade") } else { 0.0 });
    println!(
        "W37C {label} RESULT: {cells} cells, {n} iterations, {minutes:.1} min, converged {converged} ({how}); f_a {f_a:.6} (nu {nu_a:.3e}), f_b {f_b:.5} (nu {nu_b:.3e}); mass balance {mb:.2e}"
    );
    println!(
        "W37C {label} BALANCE: Q in {qi:.6} out {qo:.6}; p_area in {pi_area:.3} out {po_area:.3} Pa; Delta p_tt (mass-averaged absolute) {:.3} Pa; outlet rho<cth2/2> {kth_out:.3} Pa; L in {l_in:.5} out {l_out:.5} N m; wall torque on the fluid (blade+hub+shroud) {wall:.5} N m; L excess {:.5} N m; torque blade {:.5} hub {:.5} shroud {:.5} N m",
        pt_out - pt_in,
        (l_out - l_in) - wall,
        if blade { -N_BLADES * mon("m_blade") } else { 0.0 },
        -N_BLADES * mon("m_hub"),
        -N_BLADES * mon("m_shroud"),
    );
    Outcome {
        converged,
        how,
        cells,
        iterations: n,
        minutes,
        f_a,
        f_b,
        nu_a,
        nu_b,
    }
}

/// One registered run, selected by `W37C_RUN` = `B1`, `B2`, `B3` (bladed F1-F3) or `E1`, `E2`, `E3` (empty passage F1-F3).
#[test]
#[ignore = "ADR 0086 long run (up to 60 min each): W37C_RUN=B1 cargo test --release -p rcfd-api --test fan01_gci_w37c registered_run -- --ignored --nocapture"]
fn registered_run() {
    let sel = std::env::var("W37C_RUN").expect("set W37C_RUN");
    let blade = sel.starts_with('B');
    let i: usize = sel[1..].parse().unwrap();
    let (label, c) = family()[i - 1];
    let o = run(&format!("{sel} {label}"), c, blade);
    println!("W37C {sel} OUTCOME {o:?}");
}

/// The registered evaluation (ADR 0086 parts G and E) from the printed outcomes, given as `W37C_VALUES` = "cells1,cells2,cells3;a1,a2,a3;b1,b2,b3;
/// nua1,nua2,nua3;nub1,nub2,nub3;blade" (fine first; `blade` = 1 or 0; convergence is checked by hand from the run logs).
#[test]
#[ignore = "ADR 0086 evaluation of the printed run values: W37C_VALUES=... cargo test --release -p rcfd-api --test fan01_gci_w37c evaluate -- --ignored --nocapture"]
fn evaluate() {
    let v = std::env::var("W37C_VALUES").expect("set W37C_VALUES");
    let parts: Vec<Vec<f64>> = v
        .split(';')
        .map(|p| p.split(',').map(|x| x.trim().parse::<f64>().unwrap()).collect())
        .collect();
    let (cells, a, b, nua, nub, blade) = (&parts[0], &parts[1], &parts[2], &parts[3], &parts[4], parts[5][0] > 0.5);
    let (r21, r32) = ((cells[0] / cells[1]).cbrt(), (cells[1] / cells[2]).cbrt());
    println!("EVAL r21 {r21:.4} r32 {r32:.4}");
    let meas_torque = POWER_MEAS / omega();
    for (name, f, nu, meas) in [
        (
            if blade { "torque" } else { "outlet L" },
            a,
            nua,
            if blade { meas_torque } else { 0.0 },
        ),
        (
            if blade { "Delta p_ts" } else { "outlet swirl energy" },
            b,
            nub,
            if blade { DP_TS_MEAS } else { 0.0 },
        ),
    ] {
        assess(name, [f[0], f[1], f[2]], [nu[0], nu[1], nu[2]], r21, r32, meas, true);
    }
}

/// The registered validity class (ADR 0086 part G) of one functional's study, printed with the band and the reading against `meas`.
fn assess(
    name: &str,
    f: [f64; 3],
    nu: [f64; 3],
    r21: f64,
    r32: f64,
    meas: f64,
    converged: bool,
) -> (&'static str, Gci) {
    {
        let g = gci(f[0], f[1], f[2], r21, r32);
        let (e21, e32) = (f[1] - f[0], f[2] - f[1]);
        let nu_max = nu.iter().cloned().fold(0.0, f64::max);
        let iter_ok = nu_max <= 0.1 * e21.abs().min(e32.abs());
        let class = if !converged {
            "NOT CONVERGED"
        } else if !iter_ok {
            "ITERATION ERROR"
        } else if !(g.s > 0.0) || !g.p.is_finite() {
            "NOT MONOTONE"
        } else if !(g.p >= 0.5 && g.p <= 4.0) {
            "NOT ASYMPTOTIC"
        } else {
            "VALID"
        };
        let d = (meas - f[0]).abs();
        println!(
            "EVAL {name}: f1 {:.6} f2 {:.6} f3 {:.6}; eps21 {e21:.4e} eps32 {e32:.4e}; s {} p {:.4}; f_ext {:.6}; Fs {}; band f1 +- {:.4e} = [{:.6}, {:.6}] (relative {:.4e}); alternative (Fs 1.25, order min(p, 2)) +- {:.4e}; max iteration change {nu_max:.3e} (limit {:.3e}); {class}; reference {meas:.4}: |ref - f1| = {d:.4e}, {} the band, {} the alternative band; |ref - f_ext| = {:.4e}",
            f[0],
            f[1],
            f[2],
            g.s,
            g.p,
            g.f_ext,
            g.fs,
            g.band,
            f[0] - g.band,
            f[0] + g.band,
            g.band / f[0].abs(),
            g.band_alt,
            0.1 * e21.abs().min(e32.abs()),
            if d <= g.band { "inside" } else { "outside" },
            if d <= g.band_alt { "inside" } else { "outside" },
            (meas - g.f_ext).abs(),
        );
        (class, g)
    }
}

/// Runs F1-F3 of one part (fine first) and assesses both functionals: (class, Gci, f1) for f_a and f_b.
fn study(blade: bool) -> [(&'static str, Gci, f64); 2] {
    let o: Vec<Outcome> = family()
        .iter()
        .enumerate()
        .map(|(i, (label, c))| {
            run(
                &format!("{}{} {label}", if blade { 'B' } else { 'E' }, i + 1),
                *c,
                blade,
            )
        })
        .collect();
    let conv = o.iter().all(|x| x.converged);
    let n: Vec<f64> = o.iter().map(|x| x.cells as f64).collect();
    let (r21, r32) = ((n[0] / n[1]).cbrt(), (n[1] / n[2]).cbrt());
    let ma = if blade { POWER_MEAS / omega() } else { 0.0 };
    let mb = if blade { DP_TS_MEAS } else { 0.0 };
    let a = assess(
        "f_a",
        [o[0].f_a, o[1].f_a, o[2].f_a],
        [o[0].nu_a, o[1].nu_a, o[2].nu_a],
        r21,
        r32,
        ma,
        conv,
    );
    let b = assess(
        "f_b",
        [o[0].f_b, o[1].f_b, o[2].f_b],
        [o[0].nu_b, o[1].nu_b, o[2].nu_b],
        r21,
        r32,
        mb,
        conv,
    );
    [(a.0, a.1, o[0].f_a), (b.0, b.1, o[0].f_b)]
}

/// ADR 0086 E-L, E-S (R-E credibility): the empty-passage studies of the outlet L and swirl energy are VALID and their bands contain the exact 0.
/// KNOWN MISS: E1 stopped at the 60-minute wall clock at iteration 7918 (NOT CONVERGED); with its settled values the swirl energy is not monotone
/// (5.48, 9.23, 6.27 Pa) and L extrapolates to 0.197 N m, away from the exact 0 (ADR 0086 addendum).
#[test]
#[ignore = "known miss, ADR 0086 E-L/E-S: part E NOT CONVERGED (E1 time cap at 7918 of 8000), swirl energy not monotone; three runs, about 1.5 h"]
fn e_l_e_s_empty_passage_band_contains_zero_known_miss() {
    for (class, g, f1) in study(false) {
        assert_eq!(class, "VALID", "{g:?}");
        assert!(f1.abs() <= g.band, "band {} does not contain 0 (f1 {f1})", g.band);
    }
}

/// ADR 0086 B-M, B-P: the bladed studies of torque and Delta p_ts are VALID (met: p 1.51 and 1.74, ADR 0086 addendum). The fine run took 58.8 of
/// the 60 wall-clock minutes on a shared machine.
#[test]
#[ignore = "heavy, ADR 0086 part B: three runs, about 1.5 h in release (met in the addendum)"]
fn b_m_b_p_bladed_studies_are_valid() {
    for (class, g, _) in study(true) {
        assert_eq!(class, "VALID", "{g:?}");
    }
}
