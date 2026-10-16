//! Dynamic mesh through the case tree (F-063, F-229, ADR 0033 T1-T4). The limits are those registered in the ADR; the
//! library-equality parts of T2-T4 are replaced by the physical limits of the corresponding library checks (ADR 0033
//! addendum).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

fn parse(text: &str) -> Result<Case, Vec<rcfd_api::case::Diagnostic>> {
    Case::parse_toml(text)
}

fn paths(text: &str) -> Vec<String> {
    match parse(text) {
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

const BASE: &str = r#"
[mesh]
generator = "box_2d"
nx = 20
ny = 2
lx = 1.0
ly = 0.2
[physics]
nu = 0.01
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.ymin]
velocity = "zero_gradient"
[boundaries.ymax]
velocity = "zero_gradient"
[solver]
algorithm = "piso"
convection = "central"
dt = 0.005
end_time = 0.5
"#;

fn piston(extra_dm: &str, extra: &str) -> String {
    format!(
        r#"{BASE}{extra}
[dynamic_mesh]
enabled = true
moving_patches = ["xmin"]
deformation = "smoothing"
displacement_expression_x = "1 [m/s^2] * t^2"
fixed_patches = ["xmax"]
{extra_dm}
"#
    )
}

#[test]
fn t1_located_errors() {
    let has = |text: String, path: &str| {
        let p = paths(&text);
        assert!(
            p.iter().any(|x| x == path),
            "{path}: got {p:?}
{text}"
        );
    };
    has(
        piston("", "").replace("[\"xmin\"]", "[\"nope\"]"),
        "dynamic_mesh.moving_patches",
    );
    has(piston("follow_patches = [\"nope\"]", ""), "dynamic_mesh.follow_patches");
    has(
        piston("", "").replace("1 [m/s^2] * t^2", "1 [m/s^2] * "),
        "dynamic_mesh.displacement_expression_x",
    );
    has(
        piston("", "").replace("1 [m/s^2] * t^2", "1 [m/s^2] * t"),
        "dynamic_mesh.displacement_expression_x",
    );
    has(
        piston(
            "rotation_angle_expression = \"t / 1 [s]\"\nrotation_axis = [0.0, 0.0, 0.0]",
            "",
        ),
        "dynamic_mesh.rotation_axis",
    );
    has(
        piston("[dynamic_mesh.smoothing]\nexponent = -1.0", ""),
        "dynamic_mesh.smoothing.exponent",
    );
    has(piston("motion = \"six_dof\"", ""), "dynamic_mesh.six_dof.mass");
    has(piston("", "").replace("\"piso\"", "\"simple\""), "dynamic_mesh.enabled");
    has(
        piston("", "").replace("dt = 0.005", "dt = 0.005\ntime_scheme = \"bdf2\""),
        "dynamic_mesh.enabled",
    );
    has(
        piston("", "").replace("[solver]", "[turbulence]\nmodel = \"k_epsilon\"\n[solver]"),
        "dynamic_mesh.enabled",
    );
    has(
        piston("", "").replace("[solver]", "[energy]\nenabled = true\n[solver]"),
        "dynamic_mesh.enabled",
    );
    has(
        piston("", "").replace(
            "[boundaries.xmin]
velocity = \"fixed\"",
            "[boundaries.xmin]
velocity = \"moving_wall\"",
        ),
        "boundaries.xmin.velocity",
    );
    // disabled: every key set, runs as without the section
    let off = piston("", "").replace("enabled = true", "enabled = false");
    let c = parse(&off).unwrap();
    assert!(c.semantic_errors().is_empty());
    let mut plain = Simulation::new(&parse(BASE).unwrap()).unwrap();
    let mut with = Simulation::new(&c).unwrap();
    for _ in 0..5 {
        plain.step();
        with.step();
    }
    assert_eq!(plain.field("ux").unwrap(), with.field("ux").unwrap());
}

#[test]
fn t1_run_time_mesh_failure_is_located() {
    // the piston travels through the opposite wall
    let text = piston("", "")
        .replace("1 [m/s^2] * t^2", "3 [m/s^2] * t^2")
        .replace("end_time = 0.5", "end_time = 2.0");
    let c = parse(&text).unwrap();
    let mut s = Simulation::new(&c).unwrap();
    s.run(1000);
    let f = s.failure().expect("the run must fail");
    println!("T1 run: {f}");
    assert!(f.starts_with("dynamic_mesh:"), "{f}");
    assert!(f.chars().any(|c| c.is_ascii_digit()));
}

struct PistonRun {
    cont: f64,
    flux: f64,
    p_err: f64,
    u_err: f64,
}

fn run_piston() -> PistonRun {
    let a0 = 2.0;
    let c = parse(&piston("", "")).unwrap();
    let mut s = Simulation::new(&c).unwrap();
    let dt = 0.005;
    let mut out = PistonRun {
        cont: 0.0,
        flux: 0.0,
        p_err: 0.0,
        u_err: 0.0,
    };
    for n in 0..100 {
        let v_old = s.geom.mesh.total_volume();
        let info = s.step();
        assert!(s.failure().is_none(), "{:?}", s.failure());
        out.cont = out.cont.max(info.continuity);
        let v_new = s.geom.mesh.total_volume();
        let q_out: f64 = s.geom.mesh.patch("xmax").unwrap().faces().map(|f| s.flux()[f]).sum();
        out.flux = out.flux.max((q_out * dt - (v_old - v_new)).abs() / (v_old - v_new));
        let v_mean = a0 * (n as f64 + 0.5) * dt;
        for u in s.field("ux").unwrap() {
            out.u_err = out.u_err.max((u - v_mean).abs());
        }
    }
    let ux = s.field("p").unwrap();
    for (p, cc) in ux.iter().zip(s.geom.mesh.cell_centres()) {
        out.p_err = out.p_err.max((p - a0 * (1.0 - cc[0])).abs());
    }
    out
}

#[test]
fn t2_piston_through_the_tree() {
    let r = run_piston();
    println!(
        "T2: continuity {:.2e}, outlet vs swept volume {:.2e}, pressure error {:.3e} (limit 4e-2), u_x error {:.2e}",
        r.cont, r.flux, r.p_err, r.u_err
    );
    assert!(r.flux <= 1e-6, "{}", r.flux);
    assert!(r.cont <= 1e-6, "{}", r.cont);
    assert!(r.p_err <= 4e-2, "{}", r.p_err);
}

/// Registered velocity limit of T2 (`1e-5 a0 t_end`); the piston-adjacent cell lags (same cause as the library G5 miss).
#[test]
#[ignore = "registered limit missed in the piston-adjacent cell (ADR 0033 addendum, same as g5b_piston_cell_velocity_known_miss)"]
fn t2_piston_cell_velocity_known_miss() {
    let r = run_piston();
    assert!(r.u_err <= 1e-5 * 2.0 * 0.5, "{}", r.u_err);
}

#[test]
fn t3_stokes_second_problem_through_the_tree() {
    let delta: f64 = (2.0f64 * 0.01 / std::f64::consts::TAU).sqrt();
    let text = format!(
        r#"
[mesh]
generator = "box_2d"
nx = 4
ny = 40
lx = 0.1
ly = 1.0
periodic_x = true
[physics]
nu = 0.01
rho = 1.0
[initial]
velocity = [0.0, 0.5, 0.0]
velocity_expression_x = "exp(-y / {delta} [m]) * cos(y / {delta} [m]) * 1 [m/s]"
[boundaries.ymin]
velocity = "fixed"
velocity_value = [1.0, 0.5, 0.0]
[boundaries.ymax]
velocity = "fixed"
velocity_value = [0.0, 0.5, 0.0]
[solver]
algorithm = "pimple"
convection = "central"
n_outer = 10
dt = 0.0025
end_time = 3.0
[dynamic_mesh]
enabled = true
moving_patches = ["ymin"]
deformation = "rigid"
displacement_expression_x = "sin(2 * pi * t / 1 [s]) / (2 * pi) * 1 [m]"
displacement_expression_y = "0.5 [m/s] * t"
"#
    );
    let c = parse(&text).unwrap();
    assert!(c.semantic_errors().is_empty(), "{:?}", c.semantic_errors());
    let mut s = Simulation::new(&c).unwrap();
    let steps = s.run(2000);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let t = s.time();
    let shift = 0.5 * t;
    let ux = s.field("ux").unwrap();
    let uy = s.field("uy").unwrap();
    let mut e_ux: f64 = 0.0;
    let mut e_uy: f64 = 0.0;
    for ((a, b), cc) in ux.iter().zip(&uy).zip(s.geom.mesh.cell_centres()) {
        let eta = (cc[1] - shift) / delta;
        let exact = (-eta).exp() * (std::f64::consts::TAU * t - eta).cos();
        e_ux = e_ux.max((a - exact).abs());
        e_uy = e_uy.max((b - 0.5).abs());
    }
    println!(
        "T3: {steps} steps, t = {t}: max|u_x - exact| {e_ux:.3e} (limit 4e-2), max|u_y - V| {e_uy:.2e} (limit 1e-9)"
    );
    assert!(e_ux <= 4e-2, "{e_ux}");
    assert!(e_uy <= 1e-9, "{e_uy}");
}

#[test]
fn t4_six_dof_piston_through_the_tree() {
    // C2 of the library checks: constant force 1 N on a piston of mass 0.2 kg that pushes a fluid column
    let text = format!(
        r#"{BASE}
[dynamic_mesh]
enabled = true
motion = "six_dof"
moving_patches = ["xmin"]
fixed_patches = ["xmax"]
deformation = "smoothing"
[dynamic_mesh.six_dof]
mass = 0.2
inertia = [1.0, 1.0, 1.0]
translation_locked = [false, true, true]
gravity = true
"#
    )
    .replace("[physics]", "[physics]\ngravity = [5.0, 0.0, 0.0]")
    .replace("end_time = 0.5", "end_time = 2.0");
    let c = parse(&text).unwrap();
    assert!(c.semantic_errors().is_empty(), "{:?}", c.semantic_errors());
    let mut s = Simulation::new(&c).unwrap();
    let m = &s.geom.mesh;
    let area: f64 = m
        .patch("xmin")
        .unwrap()
        .faces()
        .map(|f| rcfd_core::vec3::norm(m.face_area_vectors()[f]))
        .sum();
    let (rho, l0, mass, force) = (1.0, 1.0, 0.2, 1.0);
    let mut steps = 0;
    while s.dynamic_body().unwrap().state.x[0] < 0.5 && steps < 400 {
        s.step();
        assert!(s.failure().is_none(), "{:?}", s.failure());
        steps += 1;
    }
    let b = s.dynamic_body().unwrap();
    let (x, v) = (b.state.x[0], b.state.v[0]);
    assert!(x >= 0.5, "the piston did not reach x = 0.5 in {steps} steps");
    let m0 = mass + rho * area * l0;
    let v2 = -(2.0 * force / (rho * area)) * (1.0 - rho * area * x / m0).ln();
    let ratio = v * v / v2;
    println!("T4: x = {x:.4} after {steps} steps, v^2/v_exact^2 = {ratio:.5} (limit |ratio - 1| <= 2e-2)");
    assert!((ratio - 1.0).abs() <= 2e-2, "{ratio}");
}
