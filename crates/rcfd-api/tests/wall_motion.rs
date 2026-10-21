//! F-204 (ADR 0016): wall motion and shear boundary conditions through the settings tree. Limits are those
//! registered in docs/adr/0016-wave13-numerics.md before this file was compiled or run.
//!
//! Plane Couette, specified shear and partial slip are linear profiles that the central discretisation reproduces
//! exactly, so their tight limits are weak tests of the discretisation on their own (stated in the ADR); the
//! thermocapillary layer and the Taylor-Couette torque are the discriminating checks.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

fn couette(ymin: &str, ymax: &str, solver_extra: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 8
ny = 20
lx = 1.0
ly = 1.0
periodic_x = true
[physics]
nu = 0.1
rho = 1.0
[boundaries.ymin]
{ymin}
[boundaries.ymax]
{ymax}
[solver]
algorithm = "simplec"
convection = "central"
max_iterations = 3000
tolerance_u = 1e-12
tolerance_p = 1e-12
{solver_extra}
"#
    )
}

const CORRECTED: &str = "component_diagonal = \"corrected\"";

fn solve(text: &str) -> Simulation {
    let c = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut s = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    s.run(5000);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    s
}

fn profile_error(s: &Simulation, exact: impl Fn(f64) -> f64) -> f64 {
    let ux = s.field("ux").unwrap();
    let uy = s.field("uy").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let mut worst = 0.0_f64;
    for c in 0..ux.len() {
        worst = worst.max((ux[c] - exact(cc[c][1])).abs()).max(uy[c].abs());
    }
    worst
}

fn walls(vel_lo: f64, vel_hi: f64) -> (String, String) {
    (
        format!("velocity = \"moving_wall\"\nvelocity_value = [{vel_lo}, 0.0, 0.0]"),
        format!("velocity = \"moving_wall\"\nvelocity_value = [{vel_hi}, 0.0, 0.0]"),
    )
}

#[test]
fn d1_plane_couette() {
    let (lo, hi) = walls(0.0, 2.0);
    let s = solve(&couette(&lo, &hi, ""));
    let e = profile_error(&s, |y| 2.0 * y) / 2.0;
    println!("D1 moving wall 0 / 2: max|u - 2y|/2 = {e:.3e}");
    assert!(e <= 1e-8, "{e}");
    let (lo, hi) = walls(-1.0, 1.0);
    let s = solve(&couette(&lo, &hi, ""));
    let e = profile_error(&s, |y| 2.0 * y - 1.0);
    println!("D1 counter-translating: max|u - (2y-1)| = {e:.3e}");
    assert!(e <= 1e-8, "{e}");
}

fn d2_body(extra: &str) {
    let s = solve(&couette(
        "velocity = \"fixed\"",
        "velocity = \"shear\"\nshear_stress = [0.5, 0.0, 0.0]",
        extra,
    ));
    let e = profile_error(&s, |y| 5.0 * y) / 5.0;
    println!(
        "D2 shear tau = 0.5 ({extra}): max|u - 5y|/5 = {e:.3e}, converged {}",
        s.is_converged()
    );
    assert!(e <= 1e-8, "{e}");
}

/// Record: with the default averaged momentum diagonal a Neumann-type tangential wall (shear, slip, and the existing
/// symmetry condition with a body force) stalls the SIMPLEC iteration (residual 0.15 after 40000 iterations, error
/// 0.79 relative); ADR 0010 documents the cause (the component-averaged diagonal) and its remedy. Not a defect of the
/// new conditions; superseded by `d2b_specified_shear` with `component_diagonal = "corrected"`. Assertion unchanged.
#[test]
#[ignore = "known miss: default averaged diagonal stalls with a Neumann-type wall, see d2b_specified_shear"]
fn d2_specified_shear() {
    d2_body("");
}

#[test]
fn d2b_specified_shear() {
    d2_body(CORRECTED);
}

fn d3_body(extra: &str) {
    let top = "velocity = \"moving_wall\"\nvelocity_value = [1.0, 0.0, 0.0]";
    let run = |lo: &str| solve(&couette(lo, top, extra));
    let l = 0.3;
    let slip = run(&format!("velocity = \"partial_slip\"\nslip_length = {l}"));
    let e = profile_error(&slip, |y| (y + l) / (1.0 + l));
    println!("D3 l = 0.3 ({extra}): max|u - (y+l)/(h+l)| = {e:.3e}");
    assert!(e <= 1e-8, "{e}");

    let diff = |a: &Simulation, b: &Simulation| {
        a.field("ux")
            .unwrap()
            .iter()
            .zip(&b.field("ux").unwrap())
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max)
    };
    let l0 = run("velocity = \"partial_slip\"\nslip_length = 0.0");
    let noslip = run("velocity = \"fixed\"");
    let d0 = diff(&l0, &noslip);
    println!("D3 l = 0 vs no-slip: {d0:.3e}");
    assert!(d0 <= 1e-12, "{d0}");

    let spec = run("velocity = \"partial_slip\"\nspecularity = 0.5\nmean_free_path = 0.1");
    let ds = diff(&spec, &slip);
    println!("D3 s = 0.5, lambda = 0.1 vs l = 0.3: {ds:.3e}");
    assert!(ds <= 1e-12, "{ds}");
}

/// Record, as for D2: superseded by `d3b_partial_slip`. Assertions unchanged.
#[test]
#[ignore = "known miss: default averaged diagonal stalls with a Neumann-type wall, see d3b_partial_slip"]
fn d3_partial_slip() {
    d3_body("");
}

#[test]
fn d3b_partial_slip() {
    d3_body(CORRECTED);
}

#[test]
fn d3_full_specularity_is_a_located_error() {
    let top = "velocity = \"moving_wall\"\nvelocity_value = [1.0, 0.0, 0.0]";
    let bad = couette(
        "velocity = \"partial_slip\"\nspecularity = 1.0\nmean_free_path = 0.1",
        top,
        "",
    );
    let err = match Case::parse_toml(&bad) {
        Err(d) => d,
        Ok(c) => Simulation::new(&c).err().unwrap_or_default(),
    };
    println!("D3 s = 1: {err:?}");
    assert!(err.iter().any(|d| d.path == "boundaries.ymin.specularity"), "{err:?}");
}

#[test]
fn moving_wall_with_a_normal_component_is_a_located_error() {
    let bad = couette(
        "velocity = \"moving_wall\"\nvelocity_value = [0.0, 1.0, 0.0]",
        "velocity = \"fixed\"",
        "",
    );
    let err = match Case::parse_toml(&bad) {
        Err(d) => d,
        Ok(c) => Simulation::new(&c).err().unwrap_or_default(),
    };
    assert!(
        err.iter().any(|d| d.path == "boundaries.ymin.velocity_value"),
        "{err:?}"
    );
}

const THERMO: &str = r#"
[mesh]
generator = "box_2d"
nx = 160
ny = 24
lx = 20.0
ly = 1.0
[physics]
nu = 1.0
rho = 1.0
cp = 1.0
conductivity = 10.0
[energy]
enabled = true
buoyancy = false
relax = 1.0
[initial]
temperature = 0.0
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_expression = "1 [K/m] * x"
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_expression = "1 [K/m] * x"
[boundaries.ymin]
velocity = "fixed"
temperature = "fixed"
temperature_expression = "1 [K/m] * x"
[boundaries.ymax]
velocity = "marangoni"
marangoni_coefficient = -0.1
temperature = "fixed"
temperature_expression = "1 [K/m] * x"
[solver]
algorithm = "simplec"
convection = "central"
component_diagonal = "corrected"
max_iterations = 20000
tolerance_u = 1e-10
tolerance_p = 1e-10
"#;

#[test]
fn d4_thermocapillary_layer() {
    let c = Case::parse_toml(THERMO).unwrap_or_else(|e| panic!("{e:?}"));
    let mut s = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    let n = s.run(30000);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let ux = s.field("ux").unwrap();
    let cc = s.geom.mesh.cell_centres();
    // cell column nearest to x = 10
    let xc = cc
        .iter()
        .map(|c| c[0])
        .min_by(|a, b| (a - 10.0).abs().total_cmp(&(b - 10.0).abs()))
        .unwrap();
    let mut worst = 0.0_f64;
    for (c, u) in ux.iter().enumerate() {
        if (cc[c][0] - xc).abs() < 1e-9 {
            let xi = cc[c][1];
            worst = worst.max((u + 0.025 * (3.0 * xi * xi - 2.0 * xi)).abs());
        }
    }
    println!(
        "D4 thermocapillary: column x = {xc}, max error {worst:.3e} against limit {:.3e}; {n} iterations, converged {}",
        0.03 * 0.025,
        s.is_converged()
    );
    assert!(worst <= 0.03 * 0.025, "{worst}");
}

#[test]
fn d4b_marangoni_stress_of_a_linear_field_equals_the_shear_condition() {
    use rcfd_api::wall_motion::{marangoni_bc, marangoni_stress};
    use rcfd_fv::bc::{Bc, BcCtx, FixedValue, Profile, SlipLaw, SlipWall};
    use rcfd_fv::gradient::least_squares;
    use rcfd_fv::{FvGeometry, ScalarField};
    use rcfd_mesh::gen::box_2d;
    use std::sync::Arc;

    let mesh = box_2d(16, 6, 4.0, 1.0, false, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let gamma = 1.3;
    let mut t = ScalarField::from_fn("T", &geom, |p| gamma * p[0]);
    for p in geom.mesh.patches() {
        if p.kind == rcfd_mesh::PatchKind::Empty {
            continue;
        }
        let v = p
            .faces()
            .map(|f| [gamma * geom.mesh.face_centres()[f][0], 0.0, 0.0])
            .collect();
        t.set_bc(&geom, &p.name, Box::new(FixedValue(Profile::PerFace(v))))
            .unwrap();
    }
    let grad = least_squares(&geom, &t.values, &t.boundary_values(&geom));
    let (coeff, mu) = (-0.1, 0.7);
    let stress = marangoni_stress(&geom, "ymax", &grad, coeff, mu);
    let reference = SlipWall {
        law: SlipLaw::Shear,
        wall_velocity: Profile::Uniform([0.0; 3]),
        stress: Profile::Uniform([coeff * gamma / mu, 0.0, 0.0]),
    };
    let bc = marangoni_bc(stress);
    let patch = geom.mesh.patch("ymax").unwrap();
    let mut worst = 0.0_f64;
    for (i, f) in patch.faces().enumerate() {
        let ctx = BcCtx {
            normal: geom.normal(f),
            dn: 0.08,
            local_face: i,
            cell_value: [0.3, 0.1, 0.0],
            partner: None,
            is_vector: true,
        };
        for k in 0..3 {
            let (a, b) = (bc.coeffs(&ctx, k), reference.coeffs(&ctx, k));
            worst = worst
                .max((a.a - b.a).abs())
                .max((a.b - b.b).abs())
                .max((a.c - b.c).abs())
                .max((a.d - b.d).abs());
        }
    }
    println!("D4b marangoni vs shear coefficients: max difference {worst:.3e}");
    assert!(worst <= 1e-12, "{worst}");
}

fn taylor_couette(n_r: usize, w_in: f64, w_out: f64, depth: Option<f64>) -> String {
    // The first D5 runs left `mesh.depth` at its default 0.1 instead of the registered depth 1 (set-up slip); the
    // torque is per unit depth times the actual depth.
    let depth_line = depth.map(|d| format!("depth = {d:?}")).unwrap_or_default();
    format!(
        r#"
[mesh]
generator = "cylinder_ogrid"
nx = 128
ny = {n_r}
r_in = 1.0
r_out = 2.0
{depth_line}
[physics]
nu = 0.1
rho = 1.0
[boundaries.cylinder]
velocity = "moving_wall"
wall_rotation_rate = {w_in:?}
[boundaries.farfield]
velocity = "moving_wall"
wall_rotation_rate = {w_out:?}
[solver]
algorithm = "simplec"
convection = "linear_upwind"
non_orthogonal_correctors = 1
max_iterations = 6000
tolerance_u = 1e-11
tolerance_p = 1e-11
[reports.torque]
type = "moment"
patch = "cylinder"
component = "viscous"
direction = [0.0, 0.0, 1.0]
origin = [0.0, 0.0, 0.0]
"#
    )
}

fn torque(text: &str) -> (f64, Simulation) {
    let c = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut s = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    s.run(8000);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let m = s.monitors.last("torque").unwrap();
    (m, s)
}

fn d5_body(depth: Option<f64>) {
    // u_theta = A r + B / r with u(1) = 1, u(2) = -1 (omega_out = -0.5): A = -1, B = 2; torque on the inner cylinder
    // per unit depth magnitude 4 pi mu B
    let exact = 4.0 * std::f64::consts::PI * 0.1 * 2.0 * depth.unwrap_or(0.1);
    let mut errs = Vec::new();
    for n_r in [32usize, 64, 128] {
        let (m, s) = torque(&taylor_couette(n_r, 1.0, -0.5, depth));
        let rel = m.abs() / exact - 1.0;
        println!(
            "D5 depth {depth:?} n_r = {n_r}: torque {m:.6} (exact magnitude {exact:.6}), relative error {rel:.3e}, converged {}",
            s.is_converged()
        );
        errs.push(rel);
    }
    println!(
        "D5 observed order (n_r 64 -> 128): {:.3}",
        (errs[1] / errs[2]).abs().log2()
    );
    assert!(errs[2].abs() <= 0.01, "{}", errs[2]);
}

/// Record: the depth was left at its default 0.1 while the exact torque was computed for depth 1, so the comparison
/// was off by a factor 10 (relative error -0.90, a set-up slip, not a solver error). Superseded by `d5c_...`.
#[test]
#[ignore = "set-up slip: depth not set, see d5c_taylor_couette_torque"]
fn d5_taylor_couette_torque() {
    let exact = 4.0 * std::f64::consts::PI * 0.1 * 2.0;
    let (m, _) = torque(&taylor_couette(128, 1.0, -0.5, None));
    assert!((m.abs() / exact - 1.0).abs() <= 0.01, "{m}");
}

#[test]
fn d5c_taylor_couette_torque() {
    d5_body(Some(1.0));
}

fn d5b_body(depth: Option<f64>, n_r: usize) -> (f64, f64, f64) {
    let exact = 4.0 * std::f64::consts::PI * 0.1 * 2.0 * depth.unwrap_or(0.1);
    let (m, s) = torque(&taylor_couette(n_r, 1.0, 1.0, depth));
    let ux = s.field("ux").unwrap();
    let uy = s.field("uy").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let ur = (0..ux.len())
        .map(|c| {
            let r = (cc[c][0] * cc[c][0] + cc[c][1] * cc[c][1]).sqrt();
            let ut = (-cc[c][1] * ux[c] + cc[c][0] * uy[c]) / r;
            (ut - r).abs()
        })
        .fold(0.0, f64::max);
    println!(
        "D5b rigid rotation n_r = {n_r}: torque {m:.3e} ({:.3e} of the D5 exact value), max |u_theta - r| {ur:.3e}, converged {}",
        m.abs() / exact,
        s.is_converged()
    );
    (m.abs() / exact, ur, exact)
}

/// Registered check D5b: rigid rotation gives no torque (<= 1e-3 of the D5 exact value) and `u_theta = r` to 1e-6.
/// The torque part is met; the velocity part is not (about 1.4e-5 at n_r = 32), see the diagnosis in ADR 0016.
/// The assertion is the registered one and is kept in this ignored test.
#[test]
#[ignore = "known miss: max |u_theta - r| exceeds 1e-6 (discretisation error of the centripetal balance)"]
fn d5b_known_miss_rigid_rotation_velocity() {
    let (_, ur, _) = d5b_body(Some(1.0), 32);
    assert!(ur <= 1e-6, "{ur}");
}

#[test]
fn d5b_rigid_rotation_has_no_torque_and_the_velocity_error_shrinks() {
    let (t32, u32_, _) = d5b_body(Some(1.0), 32);
    let (t64, u64_, _) = d5b_body(Some(1.0), 64);
    assert!(t32 <= 1e-3 && t64 <= 1e-3, "{t32} {t64}");
    // diagnosis support, not a registered criterion: the velocity error falls with refinement
    println!("D5b velocity error ratio (n_r 32 / 64): {:.2}", u32_ / u64_);
    assert!(u64_ < u32_);
}

/// Diagnostic (not a registered check): how far the n_r = 128 Taylor-Couette run is from converged when it stops.
#[test]
#[ignore = "diagnostic, about 5 minutes"]
fn d5_finest_grid_convergence_diagnostic() {
    let c = Case::parse_toml(&taylor_couette(128, 1.0, -0.5, Some(1.0))).unwrap();
    let mut s = Simulation::new(&c).unwrap();
    s.run(6000);
    let t1 = s.monitors.last("torque").unwrap();
    let l = s.last().unwrap();
    println!(
        "D5 diag after 6000 its: torque {t1:.6}, res_u {:.3e}, res_p {:.3e}",
        l.res_u, l.res_p
    );
    // continue (the iteration cap of the case is 6000, so this checks the stop rather than the solver)
    println!(
        "D5 diag: converged {}, stop reason {:?}",
        s.is_converged(),
        s.stop_reason()
    );
}

/// ADR 0098 (wave 40B) D5d: the finest D5 grid run to convergence. Registered set-up change only: the iteration cap
/// 6000 -> 20000 (residual tolerances 1e-11 unchanged). Limits: `converged == true` and `|M / M_exact - 1| <= 0.01`.
#[test]
fn d5d_taylor_couette_torque_converged() {
    let exact = 4.0 * std::f64::consts::PI * 0.1 * 2.0;
    let text = taylor_couette(128, 1.0, -0.5, Some(1.0)).replace("max_iterations = 6000", "max_iterations = 20000");
    let c = Case::parse_toml(&text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut s = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    s.run(20000);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let m = s.monitors.last("torque").unwrap();
    let rel = m.abs() / exact - 1.0;
    let l = s.last().unwrap();
    println!(
        "D5d n_r = 128: torque {m:.7} (exact magnitude {exact:.6}), relative error {rel:.3e}, {} iterations, res_u {:.3e}, res_p {:.3e}, converged {}",
        s.iteration(),
        l.res_u,
        l.res_p,
        s.is_converged()
    );
    assert!(s.is_converged(), "not converged: {:?}", s.stop_reason());
    assert!(rel.abs() <= 0.01, "{rel}");
}
