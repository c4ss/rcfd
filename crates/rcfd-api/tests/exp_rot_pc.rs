//! ADR 0183 amendment P-C, case-tree checks of the EXP-ROT prerequisite P-C (`rotating_frame` with `pimple` on a static
//! mesh): PC-1 the Coriolis term re-evaluated every outer iteration (with PC-1b and the negative control PC-1N), PC-2 the
//! rest state in a rotating frame with the transient wall pressure (PC-2N without it), PC-2b the rotating channel, PC-3 the
//! seam pair conservation with the coupled mean `rAU` (PC-3N without it), PC-4 the casing `moving_wall` with the tangential
//! projection on the FAN-01 empty passage E3 (PC-4N unprojected), PC-5 scope and located errors, PC-6 the flags of the
//! built solver. The limits are those registered in the amendment before this code ran.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use std::f64::consts::PI;
use std::time::Instant;

/// Paths of the diagnostics of a case: parse errors, then static errors, then the errors of `Simulation::new`.
fn paths(text: &str) -> Vec<String> {
    match Case::parse_toml(text) {
        Err(d) => d.iter().map(|x| x.path.clone()).collect(),
        Ok(c) => {
            let mut d: Vec<String> = c.semantic_errors().iter().map(|x| x.path.clone()).collect();
            if d.is_empty() {
                if let Err(e) = Simulation::new(&c) {
                    d = e.iter().map(|x| x.path.clone()).collect();
                }
            }
            d
        }
    }
}

fn has(text: &str, path: &str) {
    let p = paths(text);
    assert!(p.iter().any(|x| x == path), "{path}: got {p:?}\n{text}");
}

fn sim_of(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    let errs = case.semantic_errors();
    assert!(errs.is_empty(), "{errs:?}\n{text}");
    Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}\n{text}"))
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0_f64, |m, x| m.max(x.abs()))
}

// ---------------------------------------------------------------------------------------------------------------
// PC-1 inertial oscillation in a doubly periodic box (Coriolis only)
// ---------------------------------------------------------------------------------------------------------------

const PC1_OMEGA: f64 = 0.5;
const PC1_DT: f64 = 0.1;

fn pc1_text(n_outer: usize) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 8
ny = 8
lx = 1.0
ly = 1.0
periodic_x = true
periodic_y = true
[physics]
nu = 1.0
rho = 1.0
[initial]
velocity = [1.0, 0.0, 0.0]
[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = {PC1_OMEGA:?}
centrifugal = false
[solver]
algorithm = "pimple"
convection = "upwind"
time_scheme = "euler"
n_outer = {n_outer}
alpha_u = 1.0
alpha_p = 1.0
dt = {PC1_DT:?}
end_time = 100.0
momentum_rel_tol = 1e-12
pressure_rel_tol = 1e-12
pressure_max_iter = 1000
"#
    )
}

/// 50 steps; (max error against the implicit-Euler recursion, max error against the explicit recursion, max abs(p)).
fn pc1_run(n_outer: usize) -> (f64, f64, f64) {
    let mut sim = sim_of(&pc1_text(n_outer));
    let a = 2.0 * PC1_OMEGA * PC1_DT;
    let (mut imp, mut exp) = ([1.0_f64, 0.0_f64], [1.0_f64, 0.0_f64]);
    let (mut e_imp, mut e_exp, mut e_p) = (0.0_f64, 0.0_f64, 0.0_f64);
    for _ in 0..50 {
        sim.step();
        assert!(sim.failure().is_none(), "{:?}", sim.failure());
        imp = [
            (imp[0] + a * imp[1]) / (1.0 + a * a),
            (imp[1] - a * imp[0]) / (1.0 + a * a),
        ];
        exp = [exp[0] + a * exp[1], exp[1] - a * exp[0]];
        let (ux, uy) = (sim.field("ux").unwrap(), sim.field("uy").unwrap());
        for c in 0..ux.len() {
            e_imp = e_imp.max((ux[c] - imp[0]).hypot(uy[c] - imp[1]));
            e_exp = e_exp.max((ux[c] - exp[0]).hypot(uy[c] - exp[1]));
        }
        e_p = e_p.max(max_abs(&sim.field("p").unwrap()));
    }
    (e_imp, e_exp, e_p)
}

/// PC-1: with converged outer iterations the Coriolis term is implicit: the run follows the implicit-Euler recursion.
#[test]
fn pc1_converged_outer_iterations_follow_the_implicit_recursion() {
    let (e_imp, e_exp, e_p) = pc1_run(30);
    println!("PC-1 n_outer 30: max |u - u_implicit| {e_imp:.3e} (limit 1e-9), max |p| {e_p:.3e} (limit 1e-9); against the explicit recursion {e_exp:.3e}");
    assert!(e_imp <= 1e-9, "PC-1 velocity {e_imp:.3e}");
    assert!(e_p <= 1e-9, "PC-1 pressure {e_p:.3e}");
}

/// PC-1b: one outer iteration evaluates the source once, from the old-time velocity (explicit recursion); PC-1N: that run
/// misses PC-1's limit against the implicit recursion (the check can fail).
#[test]
fn pc1b_one_outer_iteration_follows_the_explicit_recursion_and_pc1n_misses() {
    let (e_imp, e_exp, e_p) = pc1_run(1);
    println!("PC-1b n_outer 1: max |u - u_explicit| {e_exp:.3e} (limit 1e-9), max |p| {e_p:.3e}; PC-1N: max |u - u_implicit| {e_imp:.3e} (must be >= 1e-3)");
    assert!(e_exp <= 1e-9, "PC-1b velocity {e_exp:.3e}");
    assert!(e_imp >= 1e-3, "PC-1N did not fire: {e_imp:.3e}");
}

// ---------------------------------------------------------------------------------------------------------------
// PC-2 rest state in a rotating frame (wave 9 set-up) and PC-2b rotating channel
// ---------------------------------------------------------------------------------------------------------------

const PC2_OMEGA: f64 = 3.0;

fn pc2_text() -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 24
ny = 24
lx = 1.0
ly = 1.0
[physics]
nu = 0.05
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[sources.frame]
type = "rotating_frame"
rotation_origin = [0.5, 0.5, 0.0]
angular_velocity = {PC2_OMEGA:?}
[solver]
algorithm = "pimple"
convection = "upwind"
time_scheme = "euler"
n_outer = 2
n_correctors = 2
dt = 0.05
end_time = 1000.0
momentum_rel_tol = 1e-9
pressure_rel_tol = 1e-9
pressure_max_iter = 2000
"#
    )
}

/// (speed / (Omega R_max), pressure error / range) of the current state (the wave-9 measures of `sources.rs`).
fn rest_errors(sim: &Simulation) -> (f64, f64) {
    let rmax = 0.5 * 2f64.sqrt();
    let c = sim.geom.mesh.cell_centres();
    let umax = sim.field("speed").unwrap().iter().copied().fold(0.0, f64::max);
    let p = sim.field("p").unwrap();
    let r2: Vec<f64> = c.iter().map(|v| (v[0] - 0.5).powi(2) + (v[1] - 0.5).powi(2)).collect();
    let head = |i: usize, j: usize| 0.5 * PC2_OMEGA * PC2_OMEGA * (r2[i] - r2[j]);
    let i_min = (0..r2.len())
        .min_by(|&a, &b| r2[a].partial_cmp(&r2[b]).unwrap())
        .unwrap();
    let i_max = (0..r2.len())
        .max_by(|&a, &b| r2[a].partial_cmp(&r2[b]).unwrap())
        .unwrap();
    let range = head(i_max, i_min);
    let perr = (0..p.len())
        .map(|i| ((p[i] - p[i_min]) - head(i, i_min)).abs())
        .fold(0.0, f64::max)
        / range;
    (umax / (PC2_OMEGA * rmax), perr)
}

/// 400 steps from rest; (final speed, final pressure error, max speed over the steps, max pressure error over the steps).
fn pc2_run(trapezoid: bool) -> (f64, f64, f64, f64) {
    let mut sim = sim_of(&pc2_text());
    {
        let s = sim.pimple_solver_mut().expect("pimple engine");
        assert!(
            s.settings.wall_pressure_trapezoid,
            "P-C did not switch the wall pressure on"
        );
        s.settings.wall_pressure_trapezoid = trapezoid;
    }
    let (mut um, mut pm) = (0.0_f64, 0.0_f64);
    let mut last = (0.0, 0.0);
    for _ in 0..400 {
        sim.step();
        assert!(sim.failure().is_none(), "{:?}", sim.failure());
        last = rest_errors(&sim);
        um = um.max(last.0);
        pm = pm.max(last.1);
    }
    (last.0, last.1, um, pm)
}

/// PC-2: the wave-9 rest criterion on the final state (t = 20 s) with the transient wall pressure.
#[test]
fn pc2_rest_state_in_a_rotating_frame_with_the_wall_pressure() {
    let (u, p, um, pm) = pc2_run(true);
    println!("PC-2: final speed/(Omega R) {u:.3e} (limit 1e-3), pressure error {p:.3e} of range (limit 1e-2); max over steps {um:.3e}, {pm:.3e}");
    assert!(u <= 1e-3, "PC-2 speed {u:.3e}");
    assert!(p <= 1e-2, "PC-2 pressure {p:.3e}");
}

/// PC-2N: the same run without the transient wall pressure misses a PC-2 limit.
#[test]
fn pc2n_rest_state_without_the_wall_pressure_misses() {
    let (u, p, um, pm) = pc2_run(false);
    println!("PC-2N: final speed/(Omega R) {u:.3e}, pressure error {p:.3e} of range (a limit must be missed: 1e-3, 1e-2); max over steps {um:.3e}, {pm:.3e}");
    assert!(u > 1e-3 || p > 1e-2, "PC-2N did not fire: {u:.3e}, {p:.3e}");
}

const PC2B_TEXT: &str = r#"
[mesh]
generator = "box_2d"
nx = 4
ny = 40
lx = 0.4
ly = 1.0
periodic_x = true
[physics]
nu = 0.1
rho = 1.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[sources.drive]
type = "acceleration"
acceleration = [1.0, 0.0, 0.0]
zone = "all"
[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = 2.0
centrifugal = false
[solver]
algorithm = "pimple"
convection = "upwind"
time_scheme = "euler"
n_outer = 2
dt = 0.01
end_time = 1000.0
momentum_rel_tol = 1e-9
pressure_rel_tol = 1e-9
pressure_max_iter = 1000
"#;

/// PC-2b: the wave-9 rotating channel on pimple (3000 steps, t = 30 s): the parabola and the geostrophic pressure difference.
#[test]
fn pc2b_rotating_channel_on_pimple() {
    let mut sim = sim_of(PC2B_TEXT);
    for _ in 0..3000 {
        sim.step();
    }
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let (nu, g, omega, h) = (0.1, 1.0, 2.0, 1.0);
    let u_of = |y: f64| g / (2.0 * nu) * y * (h - y);
    let c = sim.geom.mesh.cell_centres();
    let ux = sim.field("ux").unwrap();
    let p = sim.field("p").unwrap();
    let uc = u_of(0.5);
    let perr = (0..c.len())
        .map(|i| (ux[i] - u_of(c[i][1])).abs() / uc)
        .fold(0.0, f64::max);
    let x0 = c.iter().map(|v| v[0]).fold(f64::INFINITY, f64::min);
    let mut col: Vec<usize> = (0..c.len()).filter(|&i| (c[i][0] - x0).abs() < 1e-9).collect();
    col.sort_by(|&a, &b| c[a][1].partial_cmp(&c[b][1]).unwrap());
    let (y0, y1) = (c[col[0]][1], c[*col.last().unwrap()][1]);
    let antider = |y: f64| g / (2.0 * nu) * (h * y * y / 2.0 - y * y * y / 3.0);
    let dp_exact = -2.0 * omega * (antider(y1) - antider(y0));
    let dp = p[*col.last().unwrap()] - p[col[0]];
    let uy = max_abs(&sim.field("uy").unwrap());
    println!(
        "PC-2b: profile error {:.4} % of centre-line (limit 1 %), dp {dp:.5} vs {dp_exact:.5} ({:+.4} %, limit 1 %); max |uy| {:.3e} of centre-line (reported)",
        100.0 * perr,
        100.0 * (dp / dp_exact - 1.0),
        uy / uc
    );
    assert!(perr <= 0.01, "PC-2b profile error {perr}");
    assert!((dp / dp_exact - 1.0).abs() <= 0.01, "PC-2b dp {dp} vs {dp_exact}");
}

// ---------------------------------------------------------------------------------------------------------------
// PC-3 seam pair conservation on a sector
// ---------------------------------------------------------------------------------------------------------------

const PC3_TEXT: &str = r#"
[mesh]
generator = "annulus_3d_sector"
nx = 8
ny = 6
nz = 8
r_in = 0.5
r_out = 1.0
lz = 0.5
sector_angle = 40.0
[physics]
nu = 0.01
rho = 1.0
[initial]
velocity = [0.0, 0.0, 1.0]
[boundaries.zmin]
velocity = "fixed"
velocity_expression_x = "2 [1/s] * y + 0.2 [1/s] * x"
velocity_expression_y = "-2 [1/s] * x + 0.2 [1/s] * y"
velocity_expression_z = "1 [m/s] + 1 [1/s] * y"
[boundaries.zmax]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.inner]
velocity = "fixed"
[boundaries.outer]
velocity = "moving_wall"
wall_rotation_rate = -2.0
wall_velocity_projection = "tangential"
[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = 2.0
centrifugal = true
[solver]
algorithm = "pimple"
convection = "linear_upwind"
linear_upwind_limiter = "barth_jespersen"
time_scheme = "bdf2"
n_outer = 3
n_correctors = 2
dt = 0.05
end_time = 100.0
"#;

/// `max abs(phi_f + phi_g) / max abs(phi_f)` over the faces of the rotationally periodic pairs of the current flux.
fn seam_pair_sum(sim: &Simulation) -> f64 {
    let g = &sim.geom;
    let phi = sim.flux();
    let (mut sum, mut big) = (0.0_f64, 0.0_f64);
    for (pi, patch) in g.mesh.patches().iter().enumerate() {
        let (Some(map), Some(_)) = (&g.periodic_partner[pi], &g.periodic_rot[pi]) else {
            continue;
        };
        for (j, f) in patch.faces().enumerate() {
            sum = sum.max((phi[f] + phi[map[j]]).abs());
            big = big.max(phi[f].abs());
        }
    }
    assert!(big > 0.0, "no rotational seam flux");
    sum / big
}

/// 10 steps; the largest seam pair sum of the steps.
fn pc3_run(pair_mean: bool) -> f64 {
    let mut sim = sim_of(PC3_TEXT);
    {
        let s = sim.pimple_solver_mut().expect("pimple engine");
        assert!(s.settings.coupled_body_pair_mean, "P-C did not switch the pair mean on");
        s.settings.coupled_body_pair_mean = pair_mean;
    }
    let mut worst = 0.0_f64;
    for k in 1..=10 {
        sim.step();
        assert!(sim.failure().is_none(), "{:?}", sim.failure());
        let s = seam_pair_sum(&sim);
        println!("  PC-3 (pair mean {pair_mean}) step {k}: seam pair sum / max seam flux {s:.3e}");
        worst = worst.max(s);
    }
    worst
}

/// PC-3: with the coupled mean rAU (switched on by P-C) the two faces of every seam pair carry opposite fluxes.
#[test]
fn pc3_seam_pairs_carry_opposite_fluxes_with_the_pair_mean() {
    let w = pc3_run(true);
    println!("PC-3: worst seam pair sum {w:.3e} of the largest seam flux (limit 1e-8)");
    assert!(w <= 1e-8, "PC-3 {w:.3e}");
}

/// PC-3N: the same run with the owner rAU breaks the pair conservation (the check can fail).
#[test]
fn pc3n_without_the_pair_mean_the_seam_pairs_do_not_balance() {
    let w = pc3_run(false);
    println!("PC-3N: worst seam pair sum {w:.3e} of the largest seam flux (must exceed 1e-8)");
    assert!(w > 1e-8, "PC-3N did not fire: {w:.3e}");
}

// ---------------------------------------------------------------------------------------------------------------
// PC-4 the casing moving_wall with the tangential projection on the FAN-01 empty passage E3
// ---------------------------------------------------------------------------------------------------------------

const R_HUB: f64 = 0.124;
const R_DUCT: f64 = 0.250;
const Q_DESIGN: f64 = 1.4032;

fn omega() -> f64 {
    1486.0 * 2.0 * PI / 60.0
}

/// The 16 registered FAN-01 sections (ADR 0075 amendment 1), as `fan01_gci_w37c.rs` reads them.
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

/// The E3 mesh of ADR 0086 (F3 x0.75 counts, `camber_seam_h`, no blade).
fn e3_mesh_toml() -> String {
    let rows = sections();
    let (x0, t0) = (rows[0][6], rows[0][7]);
    let u_in = Q_DESIGN / (PI * (R_DUCT * R_DUCT - R_HUB * R_HUB));
    let ref_speed = u_in.hypot(omega() * 0.186);
    let (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) = (6, 12, 6, 8, 3, 10, 5);
    format!(
        "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\nn_blades = 9\nr_hub = 0.124\nr_tip = 0.2475\ntip_clearance = 0.0025\naxial_extent = 0.4\nz_le_root = 0.1\nfamily = \"naca4\"\n\
station_radius = {}\nstation_chord = {}\nstation_stagger = {}\nstation_camber = {}\nstation_camber_pos = {}\nstation_thickness = {}\nstation_sweep = {}\nstation_lean = {}\n\
topology = \"c_grid\"\nc_grid_construction = \"camber_seam_h\"\nn_up = {n_up}\nn_chord = {n_chord}\nn_down = {n_down}\nn_side = {n_side}\nn_nose = {n_nose}\nn_core = 1\nfront_gap = 0.3\nn_span = {n_span}\nn_gap = {n_gap}\nn_bl = 2\ngrowth = 1.2\n\
first_cell_height = 0.0\ny_plus = 30.0\nref_speed = {ref_speed:?}\nnu = 1.5e-5\ninclude_blade = false\n",
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

/// The ADR 0086 empty-passage case, laminar, on pimple (D5's dt, bdf2, 5 outer iterations). `projected`: hub and shroud are
/// `moving_wall` at -Omega with the tangential projection; otherwise `fixed` with the same rigid velocity as expressions.
fn pc4_text(projected: bool) -> String {
    let w = omega();
    let u_in = Q_DESIGN / (PI * (R_DUCT * R_DUCT - R_HUB * R_HUB));
    let rel = format!("velocity_expression_x = \"{w:?} [1/s] * y\"\nvelocity_expression_y = \"-{w:?} [1/s] * x\"\n");
    let wall = if projected {
        format!(
            "velocity = \"moving_wall\"\nwall_rotation_rate = {:?}\nwall_velocity_projection = \"tangential\"\n",
            -w
        )
    } else {
        format!("velocity = \"fixed\"\n{rel}")
    };
    format!(
        r#"{}
[physics]
nu = 1.5e-5
rho = 1.2
[initial]
velocity = [0.0, 0.0, {u_in:?}]
{rel}[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.0, 0.0, {u_in:?}]
{rel}[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.hub]
{wall}[boundaries.shroud]
{wall}[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = {w:?}
centrifugal = true
[solver]
algorithm = "pimple"
convection = "linear_upwind"
linear_upwind_limiter = "barth_jespersen"
time_scheme = "bdf2"
n_outer = 5
dt = 4.605e-5
end_time = 1.0
"#,
        e3_mesh_toml()
    )
}

/// Faces of the hub and shroud.
fn wall_faces(sim: &Simulation) -> Vec<usize> {
    let mut out = Vec::new();
    for name in ["hub", "shroud"] {
        out.extend(sim.geom.mesh.patch(name).expect("patch").faces());
    }
    out
}

/// 10 steps; (max over steps and wall faces of abs(phi_f) / (Omega r_f abs(S_f)), non-vacuity measure, worst seam pair sum,
/// worst continuity, wall seconds per step).
fn pc4_run(projected: bool) -> (f64, f64, f64, f64, f64) {
    let mut sim = sim_of(&pc4_text(projected));
    println!("  PC-4 (projected {projected}): {} cells", sim.geom.n_cells());
    let w = omega();
    let faces = wall_faces(&sim);
    let (fc, sf) = (
        sim.geom.mesh.face_centres().to_vec(),
        sim.geom.mesh.face_area_vectors().to_vec(),
    );
    let scale = |f: usize| w * fc[f][0].hypot(fc[f][1]) * rcfd_core::vec3::norm(sf[f]);
    let vac = faces
        .iter()
        .map(|&f| {
            let n = sim.geom.normal(f);
            let r = fc[f][0].hypot(fc[f][1]);
            (-fc[f][1] * n[0] + fc[f][0] * n[1]).abs() / r
        })
        .fold(0.0, f64::max);
    let (mut flux, mut seam, mut cont) = (0.0_f64, 0.0_f64, 0.0_f64);
    let t0 = Instant::now();
    for k in 1..=10 {
        let st = sim.step();
        assert!(sim.failure().is_none(), "PC-4 step {k}: {:?}", sim.failure());
        let phi = sim.flux();
        let wf = faces.iter().map(|&f| phi[f].abs() / scale(f)).fold(0.0, f64::max);
        let sp = seam_pair_sum(&sim);
        println!("  PC-4 (projected {projected}) step {k}: wall flux {wf:.3e} Omega r abs(S), seam pair sum {sp:.3e}, continuity {:.3e}", st.continuity);
        assert!(st.continuity.is_finite(), "PC-4 step {k} continuity");
        flux = flux.max(wf);
        seam = seam.max(sp);
        cont = cont.max(st.continuity);
    }
    (flux, vac, seam, cont, t0.elapsed().as_secs_f64() / 10.0)
}

/// PC-4: on the pimple path the projected casing and hub stay impermeable on every step, on faces where the rigid velocity
/// is not tangential (non-vacuity), and the run is finite.
#[test]
fn pc4_projected_casing_is_impermeable_on_the_pimple_path() {
    let (flux, vac, seam, cont, secs) = pc4_run(true);
    println!("PC-4: max wall flux {flux:.3e} Omega r abs(S) (limit 1e-12); non-vacuity {vac:.3e} (must be >= 1e-3); reported: seam pair sum {seam:.3e}, continuity {cont:.3e}, {secs:.3} s per step");
    assert!(flux <= 1e-12, "PC-4 wall flux {flux:.3e}");
    assert!(vac >= 1e-3, "PC-4 non-vacuity {vac:.3e}");
}

/// PC-4N: the same walls with the rigid velocity unprojected carry a flux (the check can fail).
#[test]
fn pc4n_unprojected_walls_carry_a_flux() {
    let (flux, vac, seam, cont, secs) = pc4_run(false);
    println!("PC-4N: max wall flux {flux:.3e} Omega r abs(S) (must be >= 1e-3); non-vacuity {vac:.3e}; seam pair sum {seam:.3e}, continuity {cont:.3e}, {secs:.3} s per step");
    assert!(flux >= 1e-3, "PC-4N did not fire: {flux:.3e}");
}

// ---------------------------------------------------------------------------------------------------------------
// PC-5 scope and PC-6 flags
// ---------------------------------------------------------------------------------------------------------------

fn frame_block(kind: &str) -> String {
    format!("[sources.frame]\ntype = \"{kind}\"\nrotation_axis = [0.0, 0.0, 1.0]\nrotation_origin = [0.5, 0.5, 0.0]\nangular_velocity = 3.0\n")
}

/// The PC-2 box with the given algorithm, source block and extra solver lines.
fn scope_text(alg: &str, source: &str, extra_solver: &str) -> String {
    let frame =
        "[sources.frame]\ntype = \"rotating_frame\"\nrotation_origin = [0.5, 0.5, 0.0]\nangular_velocity = 3.0\n";
    let base = pc2_text();
    assert!(base.contains(frame), "the PC-2 text changed");
    let mut t = base.replace(frame, "").replace(
        "algorithm = \"pimple\"",
        &format!("algorithm = \"{alg}\"\n{extra_solver}"),
    );
    t.push_str(source);
    t
}

/// PC-5: the scope of P-C.
#[test]
fn pc5_scope_of_the_rotating_frame_on_the_transient_path() {
    let at = "sources.frame.type";
    let rf = frame_block("rotating_frame");
    let ok = scope_text("pimple", &rf, "");
    assert!(paths(&ok).is_empty(), "PC-5 pimple: {:?}", paths(&ok));
    println!("PC-5: rotating_frame with pimple accepted");
    has(&scope_text("piso", &rf, ""), at);
    has(&scope_text("pimple", &frame_block("mrf"), ""), at);
    has(&scope_text("piso", &frame_block("mrf"), ""), at);
    has(&scope_text("pimple", &rf, "time_advance = \"fractional_step\""), at);
    has(
        &format!("{}[dynamic_mesh]\nenabled = true\n", scope_text("pimple", &rf, "")),
        "dynamic_mesh.enabled",
    );
    has(
        &format!(
            "{}[turbulence]\nmodel = \"k_omega_sst\"\n",
            scope_text("pimple", &rf, "")
        ),
        "turbulence.model",
    );
    println!("PC-5: located errors at {at} (piso; mrf with pimple and piso; fractional step), dynamic_mesh.enabled, turbulence.model (k_omega_sst with pimple)");
}

/// (coupled_body_pair_mean, wall_pressure_trapezoid) of the built pimple solver.
fn flags(source: &str) -> (bool, bool) {
    let sim = sim_of(&scope_text("pimple", source, ""));
    let s = sim.pimple_solver().expect("pimple engine");
    (s.settings.coupled_body_pair_mean, s.settings.wall_pressure_trapezoid)
}

/// PC-6: the rotating frame switches both flags on; no source and the other sources leave both off.
#[test]
fn pc6_flags_of_the_built_pimple_solver() {
    let rf = frame_block("rotating_frame");
    let acc = "[sources.drive]\ntype = \"acceleration\"\nacceleration = [1.0, 0.0, 0.0]\nzone = \"all\"\n";
    let porous = "[sources.bed]\ntype = \"porous\"\nmodel = \"darcy_forchheimer\"\npermeability = 0.5\ninertial_resistance = 0.0\nzone = \"all\"\n";
    let disk = "[sources.disk]\ntype = \"actuator_disk\"\nzone = \"box\"\nbox_min = [0.2, 0.0, -1.0]\nbox_max = [0.4, 1.0, 1.0]\nthrust = 0.1\ndirection = [1.0, 0.0, 0.0]\n";
    let mhd =
        "[sources.mhd]\ntype = \"lorentz\"\nzone = \"all\"\nmagnetic_field = [0.0, 1.0, 0.0]\nconductivity = 1.0\n";
    assert_eq!(flags(&rf), (true, true), "PC-6 rotating_frame");
    assert_eq!(
        flags(&format!("{rf}{acc}")),
        (true, true),
        "PC-6 rotating_frame + acceleration"
    );
    for (name, src) in [
        ("none", ""),
        ("acceleration", acc),
        ("porous", porous),
        ("actuator_disk", disk),
        ("lorentz", mhd),
    ] {
        assert_eq!(flags(src), (false, false), "PC-6 {name}");
    }
    println!("PC-6: rotating_frame (alone and with an acceleration) -> both flags on; none, acceleration, porous, actuator_disk, lorentz -> both off");
}
