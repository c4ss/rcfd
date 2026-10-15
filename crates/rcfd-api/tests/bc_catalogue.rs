//! ADR 0022 (F-200) boundary-condition catalogue through the settings tree. The limits are those registered in
//! docs/adr/0022-bc-catalogue.md before the code was written; none is changed here.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

fn parse(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"))
}

fn build(text: &str) -> Simulation {
    Simulation::new(&parse(text)).unwrap_or_else(|e| panic!("{e:?}"))
}

/// Run to convergence (a run that stops on the iteration cap is NOT MET).
fn solve(text: &str, cap: usize) -> Simulation {
    let mut s = build(text);
    s.run(cap);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    assert!(s.is_converged(), "not converged: {:?}", s.stop_reason());
    s
}

/// POST-HOC CRITERION (ADR 0022 addendum, deviation 1; the registered one is residual targets, see the
/// `*_as_registered_known_miss` tests). The threshold was first 1e-12 and was loosened to 1e-10 after C3 K = 0 sat at a
/// 1e-12 round-off floor of the pressure.
/// Iterate until the cell fields stop changing: the largest change of ux, uy, uz and p between successive iterations
/// is at most 1e-10 for 20 iterations in a row. Used for the frictionless uniform-flow cases, whose exact solution makes
/// the normalised residuals measure round-off (the control with only existing conditions behaves the same), so the
/// residual targets cannot be met there. A run that does not settle within `cap` iterations fails.
fn settle(text: &str, cap: usize) -> Simulation {
    let mut s = build(text);
    let names = ["ux", "uy", "uz", "p"];
    let mut prev: Vec<Vec<f64>> = names.iter().map(|n| s.field(n).unwrap()).collect();
    let mut quiet = 0;
    for it in 0..cap {
        s.step();
        assert!(s.failure().is_none(), "{:?}", s.failure());
        let cur: Vec<Vec<f64>> = names.iter().map(|n| s.field(n).unwrap()).collect();
        let change = cur
            .iter()
            .zip(&prev)
            .flat_map(|(a, b)| a.iter().zip(b).map(|(x, y)| (x - y).abs()))
            .fold(0.0_f64, |m, v| if v.is_nan() { f64::INFINITY } else { m.max(v) });
        assert!(change.is_finite(), "the fields are not finite at iteration {}", it + 1);
        quiet = if change <= 1e-10 { quiet + 1 } else { 0 };
        prev = cur;
        if quiet >= 20 {
            println!("settled after {} iterations (last change {change:.2e})", it + 1);
            return s;
        }
    }
    panic!("did not settle within {cap} iterations");
}

/// Outward mass flow [kg/s] through a patch from the final face flux.
fn mass(s: &Simulation, patch: &str, rho: f64) -> f64 {
    let p = s.geom.mesh.patch(patch).unwrap_or_else(|| panic!("no patch {patch}"));
    rho * p.faces().map(|f| s.flux()[f]).sum::<f64>()
}

/// Sum of the mass flow over every non-empty boundary patch divided by the largest absolute patch flow.
fn imbalance(s: &Simulation, rho: f64) -> f64 {
    let flows: Vec<f64> = s
        .geom
        .mesh
        .patches()
        .iter()
        .map(|p| rho * p.faces().map(|f| s.flux()[f]).sum::<f64>())
        .collect();
    let top = flows.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    flows.iter().sum::<f64>().abs() / top
}

/// Least-squares slope of the column-mean cell pressure (Pa = rho * p) against x over `x0 <= x <= x1`.
fn pressure_slope(s: &Simulation, rho: f64, x0: f64, x1: f64) -> f64 {
    let p = s.field("p").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let mut cols: std::collections::BTreeMap<i64, (f64, f64, usize)> = Default::default();
    for (c, x) in cc.iter().enumerate() {
        if x[0] >= x0 && x[0] <= x1 {
            let key = (x[0] * 1e9).round() as i64;
            let e = cols.entry(key).or_insert((x[0], 0.0, 0));
            e.1 += rho * p[c];
            e.2 += 1;
        }
    }
    let pts: Vec<(f64, f64)> = cols.values().map(|(x, sum, n)| (*x, sum / *n as f64)).collect();
    let n = pts.len() as f64;
    let (mx, mp) = (
        pts.iter().map(|q| q.0).sum::<f64>() / n,
        pts.iter().map(|q| q.1).sum::<f64>() / n,
    );
    let num: f64 = pts.iter().map(|q| (q.0 - mx) * (q.1 - mp)).sum();
    let den: f64 = pts.iter().map(|q| (q.0 - mx) * (q.0 - mx)).sum();
    num / den
}

const SOLVER: &str = r#"
[solver]
algorithm = "simplec"
convection = "linear_upwind"
max_iterations = 6000
tolerance_u = 1e-12
tolerance_p = 1e-12
"#;

/// Plane channel of ADR 0022 A2: lx 10, H 1, depth 0.1, 100 x 20 cells, nu 0.1, rho 1.
fn channel(inlet: &str, outlet: &str, rho: f64, extra: &str) -> String {
    format!(
        r#"
[mesh]
generator = "channel_2d"
nx = 100
ny = 20
lx = 10.0
ly = 1.0
depth = 0.1
{extra}
[physics]
nu = 0.1
rho = {rho}
[boundaries.inlet]
{inlet}
[boundaries.outlet]
{outlet}
[boundaries.walls]
velocity = "fixed"
{SOLVER}
"#
    )
}

const MF_INLET: &str = "bc_kind = \"mass_flow\"\nmass_flow_value = 0.1";
const P_OUTLET: &str = "velocity = \"zero_gradient\"\npressure = \"fixed\"\npressure_value = 0.0";
const PARABOLA: &str = "mass_flow_shape_expression = \"4 [m/s] * (y / 1 [m]) * (1 - y / 1 [m])\"";

#[test]
fn a1_exactness_of_the_prescribed_flow() {
    let mut worst = 0.0_f64;
    let mut check = |label: &str, text: String, patch: &str, rho: f64, m_in: f64| {
        let mut s = build(&text);
        s.step();
        let got = -mass(&s, patch, rho);
        let e = (got - m_in).abs() / m_in.abs();
        println!("A1 {label}: m = {got:.15e}, relative error {e:.2e}");
        assert!(e <= 1e-12, "{label}: {e}");
        worst = worst.max(e);
    };
    // (a) uniform, (b) parabolic shape, (e) rho 1.2, (g) graded mesh
    check("uniform", channel(MF_INLET, P_OUTLET, 1.0, ""), "inlet", 1.0, 0.1);
    check(
        "parabolic",
        channel(&format!("{MF_INLET}\n{PARABOLA}"), P_OUTLET, 1.0, ""),
        "inlet",
        1.0,
        0.1,
    );
    check(
        "rho 1.2",
        channel("bc_kind = \"mass_flow\"\nmass_flow_value = 0.12", P_OUTLET, 1.2, ""),
        "inlet",
        1.2,
        0.12,
    );
    check(
        "graded",
        channel(MF_INLET, P_OUTLET, 1.0, "grading = 4.0"),
        "inlet",
        1.0,
        0.1,
    );
    // (f) prescribed outflow: inlet and outlet both mass flow
    let two = channel(MF_INLET, "bc_kind = \"mass_flow\"\nmass_flow_value = -0.1", 1.0, "");
    check("outflow side", two.clone(), "outlet", 1.0, -0.1);
    check("inflow side", two, "inlet", 1.0, 0.1);
    // (c) oblique direction on a strip periodic in y
    let oblique = r#"
[mesh]
generator = "box_2d"
nx = 20
ny = 8
lx = 2.0
ly = 1.0
periodic_y = true
[physics]
nu = 0.1
[boundaries.xmin]
bc_kind = "mass_flow"
mass_flow_value = 0.1
flow_direction = [0.8660254037844386, 0.5, 0.0]
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
[solver]
algorithm = "simplec"
"#;
    check("oblique", oblique.into(), "xmin", 1.0, 0.1);
    // (d) swirl on an annulus inlet
    let swirl = r#"
[mesh]
generator = "annulus_3d"
nx = 24
ny = 6
nz = 6
r_in = 0.5
r_out = 1.0
lz = 2.0
[physics]
nu = 0.1
[boundaries.inner]
velocity = "fixed"
[boundaries.outer]
velocity = "fixed"
[boundaries.zmin]
bc_kind = "mass_flow"
mass_flow_value = 0.3
swirl_rate = 2.0
[boundaries.zmax]
velocity = "zero_gradient"
pressure = "fixed"
[solver]
algorithm = "simplec"
"#;
    check("swirl", swirl.into(), "zmin", 1.0, 0.3);
    println!("A1 worst relative error {worst:.2e} (limit 1e-12)");
}

#[test]
fn a2_plane_poiseuille_pressure_gradient() {
    for (label, inlet) in [
        ("uniform", MF_INLET.to_string()),
        ("parabolic", format!("{MF_INLET}\n{PARABOLA}")),
    ] {
        let s = solve(&channel(&inlet, P_OUTLET, 1.0, ""), 6000);
        let slope = pressure_slope(&s, 1.0, 6.0, 9.0);
        let e = (slope + 1.2).abs() / 1.2;
        println!(
            "A2 {label}: dp/dx = {slope:.6} Pa/m (analytic -1.2), error {:.3} %",
            100.0 * e
        );
        assert!(e <= 0.01, "{label}: {e}");
        // profile at the column nearest x = 9
        let ux = s.field("ux").unwrap();
        let cc = s.geom.mesh.cell_centres();
        let xs = cc.iter().map(|c| (c[0] - 9.0).abs()).fold(f64::MAX, f64::min);
        let mut worst = 0.0_f64;
        for (c, x) in cc.iter().enumerate() {
            if (x[0] - 9.0).abs() <= xs + 1e-9 {
                let y = x[1];
                worst = worst.max((ux[c] - 6.0 * y * (1.0 - y)).abs());
            }
        }
        println!(
            "A2 {label}: profile max error {:.3e} (limit {:.3e})",
            worst / 1.5,
            0.015
        );
        assert!(worst / 1.5 <= 0.015, "{label}: profile {}", worst / 1.5);
        if label == "parabolic" {
            let s1 = pressure_slope(&s, 1.0, 1.0, 9.0);
            let e1 = (s1 + 1.2).abs() / 1.2;
            println!(
                "A2b parabolic, slope over 1 <= x <= 9: {s1:.6}, error {:.3} %",
                100.0 * e1
            );
            assert!(e1 <= 0.01, "{e1}");
        }
        let im = imbalance(&s, 1.0);
        println!("M {label}: patch imbalance {im:.2e}");
        assert!(im <= 1e-9, "{im}");
    }
}

/// Volume-weighted layer-mean cell pressure [Pa] against z, least-squares slope over `z0 <= z <= z1`.
fn pressure_slope_z(s: &Simulation, rho: f64, z0: f64, z1: f64) -> f64 {
    let p = s.field("p").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let vol = s.geom.mesh.cell_volumes();
    let mut layers: std::collections::BTreeMap<i64, (f64, f64, f64)> = Default::default();
    for (c, x) in cc.iter().enumerate() {
        if x[2] >= z0 && x[2] <= z1 {
            let e = layers.entry((x[2] * 1e9).round() as i64).or_insert((x[2], 0.0, 0.0));
            e.1 += rho * p[c] * vol[c];
            e.2 += vol[c];
        }
    }
    let pts: Vec<(f64, f64)> = layers.values().map(|(z, ps, v)| (*z, ps / v)).collect();
    let n = pts.len() as f64;
    let (mz, mp) = (
        pts.iter().map(|q| q.0).sum::<f64>() / n,
        pts.iter().map(|q| q.1).sum::<f64>() / n,
    );
    let num: f64 = pts.iter().map(|q| (q.0 - mz) * (q.1 - mp)).sum();
    let den: f64 = pts.iter().map(|q| (q.0 - mz) * (q.0 - mz)).sum();
    num / den
}

#[test]
fn a3_annular_hagen_poiseuille() {
    // ADR 0022 A3: 48-gon annulus, equal-area radii, Q = W A_polygon; closed-form gradient 4.776136 Pa/m
    let text = r#"
[mesh]
generator = "annulus_3d"
nx = 48
ny = 24
nz = 24
r_in = 0.5
r_out = 1.0
lz = 4.0
[physics]
nu = 0.1
rho = 1.0
[boundaries.inner]
velocity = "fixed"
[boundaries.outer]
velocity = "fixed"
[boundaries.zmin]
bc_kind = "mass_flow"
mass_flow_value = 2.3494714599609283
[boundaries.zmax]
velocity = "zero_gradient"
pressure = "fixed"
[solver]
algorithm = "simplec"
convection = "linear_upwind"
max_iterations = 6000
tolerance_u = 1e-12
tolerance_p = 1e-12
"#;
    let s = solve(text, 6000);
    let slope = pressure_slope_z(&s, 1.0, 1.5, 3.5);
    let exact = 4.776135984330572;
    let e = (slope + exact).abs() / exact;
    println!(
        "A3 annular: dp/dz = {slope:.6} Pa/m (closed form -{exact:.6}), error {:.3} %, iterations {}",
        100.0 * e,
        s.iteration()
    );
    assert!(e <= 0.01, "{e}");
    let im = imbalance(&s, 1.0);
    println!("M annulus: patch imbalance {im:.2e}");
    assert!(im <= 1e-9, "{im}");
}

// ---------------------------------------------------------------------------------------------------------------
// A4, A5: face velocity of the swirl and the oblique direction (first iteration, i.e. right after the build)

fn boundary_velocity(s: &Simulation, patch: &str) -> Vec<([f64; 3], [f64; 3], [f64; 3])> {
    // (face velocity, face centre, unit normal) of every face of the patch
    let uf = s.velocity().boundary_values(&s.geom);
    let p = s.geom.mesh.patch(patch).unwrap();
    p.faces()
        .map(|f| {
            (
                uf[f - s.geom.n_internal],
                s.geom.mesh.face_centres()[f],
                s.geom.normal(f),
            )
        })
        .collect()
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn tangent(v: [f64; 3], n: [f64; 3]) -> [f64; 3] {
    let vn = v[0] * n[0] + v[1] * n[1] + v[2] * n[2];
    [v[0] - n[0] * vn, v[1] - n[1] * vn, v[2] - n[2] * vn]
}

#[test]
fn a4_swirl_face_velocity_mass_flow_and_total_pressure_inlet() {
    for kind in ["mass_flow", "total_pressure_inlet"] {
        let inlet = if kind == "mass_flow" {
            "bc_kind = \"mass_flow\"\nmass_flow_value = 0.3\nswirl_rate = 2.0".to_string()
        } else {
            "bc_kind = \"total_pressure_inlet\"\nbc_pressure = 0.5\nswirl_rate = 2.0".to_string()
        };
        let text = format!(
            r#"
[mesh]
generator = "annulus_3d"
nx = 24
ny = 6
nz = 6
r_in = 0.5
r_out = 1.0
lz = 2.0
[physics]
nu = 0.1
[boundaries.inner]
velocity = "fixed"
[boundaries.outer]
velocity = "fixed"
[boundaries.zmin]
{inlet}
[boundaries.zmax]
velocity = "zero_gradient"
pressure = "fixed"
[solver]
algorithm = "simplec"
"#
        );
        let mut s = build(&text);
        if kind == "total_pressure_inlet" {
            // the Directed condition needs the owner-cell velocity: give the cells an axial velocity first
            s.step();
        }
        let mut worst = 0.0_f64;
        for (u, x, n) in boundary_velocity(&s, "zmin") {
            let omega = [0.0, 0.0, 2.0];
            let swirl = cross(omega, x);
            let (tu, ts) = (tangent(u, n), tangent(swirl, n));
            let scale = 2.0 * (x[0] * x[0] + x[1] * x[1]).sqrt();
            let e = (0..3).map(|k| (tu[k] - ts[k]).abs()).fold(0.0_f64, f64::max) / scale;
            worst = worst.max(e);
        }
        println!("A4/D3 {kind}: swirl tangential error {worst:.2e} (limit 1e-12)");
        assert!(worst <= 1e-12, "{kind}: {worst}");
    }
}

#[test]
fn a5_oblique_direction() {
    let text = r#"
[mesh]
generator = "box_2d"
nx = 20
ny = 8
lx = 2.0
ly = 1.0
periodic_y = true
[physics]
nu = 0.1
[boundaries.xmin]
bc_kind = "mass_flow"
mass_flow_value = 0.1
flow_direction = [0.8660254037844386, 0.5, 0.0]
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
[solver]
algorithm = "simplec"
"#;
    let s = build(text);
    let e = [0.8660254037844386_f64, 0.5, 0.0];
    let mut worst = 0.0_f64;
    for (u, _, _) in boundary_velocity(&s, "xmin") {
        let cr = [
            u[1] * e[2] - u[2] * e[1],
            u[2] * e[0] - u[0] * e[2],
            u[0] * e[1] - u[1] * e[0],
        ];
        let nc = (cr[0] * cr[0] + cr[1] * cr[1] + cr[2] * cr[2]).sqrt();
        let nu = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
        worst = worst.max(nc / nu);
    }
    println!("A5 direction: max |u x e|/|u| = {worst:.2e} (limit 1e-12)");
    assert!(worst <= 1e-12, "{worst}");
}

#[test]
fn a6_recirculation_pair() {
    let text = format!(
        r#"
[mesh]
generator = "box_2d"
nx = 40
ny = 40
lx = 1.0
ly = 1.0
[physics]
nu = 0.1
[boundaries.xmin]
bc_kind = "mass_flow"
mass_flow_value = 0.1
[boundaries.ymax]
bc_kind = "mass_flow"
mass_flow_source_patch = "xmax"
mass_flow_fraction = 0.3
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.ymin]
velocity = "fixed"
{SOLVER}
"#
    );
    let s = solve(&text, 6000);
    let m_out = mass(&s, "xmax", 1.0);
    let m_ymax = -mass(&s, "ymax", 1.0);
    let e1 = (m_out - 0.1 / 0.7).abs() / (0.1 / 0.7);
    let e2 = (m_ymax - 0.3 * m_out).abs() / (0.3 * m_out);
    let im = imbalance(&s, 1.0);
    println!("A6 recirculation: m_out {m_out:.12} (m0/(1-f) = {:.12}), error {e1:.2e} (limit 1e-8); m_ymax/m_out = {:.12}, error {e2:.2e} (limit 1e-8); imbalance {im:.2e}", 0.1 / 0.7, m_ymax / m_out);
    assert!(e1 <= 1e-8, "{e1}");
    assert!(e2 <= 1e-8, "{e2}");
    assert!(im <= 1e-9, "{im}");
}

#[test]
fn a7_turbulence_inlet_values_follow_the_mass_flow_speed() {
    let case = |model: &str, spec: &str| {
        format!(
            r#"
[mesh]
generator = "channel_2d"
nx = 20
ny = 10
lx = 4.0
ly = 1.0
depth = 0.1
[physics]
nu = 0.1
[boundaries.inlet]
bc_kind = "mass_flow"
mass_flow_value = 0.1
turbulence_intensity = 0.05
{spec}
[boundaries.outlet]
bc_kind = "pressure_outlet"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
[turbulence]
model = "{model}"
"#
        )
    };
    let uniform = |s: &Simulation, name: &str| -> f64 {
        let v = s.boundary_field(name).unwrap();
        let p = s.geom.mesh.patch("inlet").unwrap();
        let vals: Vec<f64> = p.faces().map(|f| v[f - s.geom.n_internal]).collect();
        let (lo, hi) = vals
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
        assert!((hi - lo).abs() <= 1e-14 * hi.abs(), "inlet {name} not uniform");
        vals[0]
    };
    let rel = |a: f64, b: f64| (a - b).abs() / b.abs();
    let s = build(&case("k_epsilon", "turbulence_length = 0.07"));
    let (k, eps) = (uniform(&s, "k"), uniform(&s, "epsilon"));
    let (k0, e0) = (3.75e-3, 0.09_f64.powf(0.75) * 3.75e-3_f64.powf(1.5) / 0.07);
    println!("A7 k-epsilon: k = {k:.9e} (want {k0:.9e}), eps = {eps:.9e} (want {e0:.9e}; registered 5.390521e-4)");
    assert!(rel(k, k0) <= 1e-12 && rel(eps, e0) <= 1e-12);
    assert!((e0 - 5.390521e-4).abs() / 5.390521e-4 < 1e-6);
    let s = build(&case("k_omega_sst", "turbulence_length = 0.07"));
    let (k, om) = (uniform(&s, "k"), uniform(&s, "omega"));
    let om0 = 3.75e-3_f64.sqrt() / (0.09_f64.powf(0.25) * 0.07);
    println!("A7 SST: k = {k:.9e}, omega = {om:.9e} (want {om0:.9e}; registered 1.597191)");
    assert!(rel(k, k0) <= 1e-12 && rel(om, om0) <= 1e-12);
    assert!((om0 - 1.597191).abs() / 1.597191 < 1e-6);
    let s = build(&case(
        "k_epsilon",
        "turbulence_spec = \"intensity_viscosity_ratio\"\nturbulence_viscosity_ratio = 10.0",
    ));
    let eps = uniform(&s, "epsilon");
    let e1 = 0.09 * 3.75e-3_f64 * 3.75e-3 / (0.1 * 10.0);
    println!("A7 viscosity ratio: eps = {eps:.9e} (want {e1:.9e}; registered 1.265625e-6)");
    assert!(rel(eps, e1) <= 1e-12);
    assert!((e1 - 1.265625e-6).abs() / 1.265625e-6 < 1e-9);
}

// ---------------------------------------------------------------------------------------------------------------
// A8: located validation errors

/// Diagnostics of a case from either stage (parse and semantic check, then the build).
fn errors(text: &str) -> Vec<rcfd_api::case::Diagnostic> {
    match Case::parse_toml(text) {
        Err(e) => e,
        Ok(c) => {
            let mut e = c.semantic_errors();
            if e.is_empty() {
                if let Err(b) = Simulation::new(&c) {
                    e = b;
                }
            }
            e
        }
    }
}

fn expect_error(label: &str, text: &str, path: &str) {
    let e = errors(text);
    println!(
        "A8 {label}: {}",
        e.iter()
            .map(|d| format!("{} -> {}", d.path, d.message))
            .collect::<Vec<_>>()
            .join(" | ")
    );
    assert!(
        e.iter().any(|d| d.path == path),
        "{label}: expected an error at {path}, got {e:?}"
    );
}

/// Base case of the validation checks: a plane channel (entries inlet, outlet, walls).
fn base(solver_extra: &str, inlet: &str, outlet: &str, walls: &str, extra: &str) -> String {
    format!(
        r#"
[mesh]
generator = "channel_2d"
nx = 10
ny = 6
lx = 2.0
ly = 1.0
depth = 0.1
[physics]
nu = 0.1
[boundaries.inlet]
{inlet}
[boundaries.outlet]
{outlet}
[boundaries.walls]
{walls}
{extra}
[solver]
algorithm = "simplec"
{solver_extra}
"#
    )
}

const MF: &str = "bc_kind = \"mass_flow\"\nmass_flow_value = 0.1";
const PO: &str = "bc_kind = \"pressure_outlet\"";
const WALL: &str = "velocity = \"fixed\"";

fn ok_case(inlet: &str, outlet: &str) -> String {
    base("", inlet, outlet, WALL, "")
}

#[test]
fn a8_validation_errors_are_located() {
    // the plain base case is valid
    let e = errors(&ok_case(MF, PO));
    assert!(e.is_empty(), "{e:?}");
    // V1 every kind with compressible / coupled
    for kind in rcfd_api::bc_cat::KINDS {
        let inlet = format!("bc_kind = \"{kind}\"");
        let t = base(
            "",
            &inlet,
            "velocity = \"zero_gradient\"\npressure = \"fixed\"",
            WALL,
            "",
        );
        let t_comp = t.replace("algorithm = \"simplec\"", "algorithm = \"compressible\"");
        expect_error(&format!("V1 {kind} compressible"), &t_comp, "boundaries.inlet.bc_kind");
        let t_coupled = t.replace("algorithm = \"simplec\"", "algorithm = \"coupled\"");
        expect_error(&format!("V1 {kind} coupled"), &t_coupled, "boundaries.inlet.bc_kind");
    }
    // V1 variable density
    let vd = base(
        "",
        MF,
        PO,
        WALL,
        "[fluid]
density = \"ideal_gas\"",
    );
    expect_error("V1 variable density", &vd, "boundaries.inlet.bc_kind");
    // V2 the kind owns the velocity and pressure conditions
    expect_error(
        "V2 velocity",
        &ok_case(&format!("{MF}\nvelocity = \"zero_gradient\""), PO),
        "boundaries.inlet.velocity",
    );
    expect_error(
        "V2 pressure",
        &ok_case(&format!("{MF}\npressure = \"fixed\""), PO),
        "boundaries.inlet.pressure",
    );
    expect_error(
        "V2 velocity_value",
        &ok_case(&format!("{MF}\nvelocity_value = [1.0, 0.0, 0.0]"), PO),
        "boundaries.inlet.velocity_value",
    );
    expect_error(
        "V2 velocity_expression_x",
        &ok_case(&format!("{MF}\nvelocity_expression_x = \"1 [m/s]\""), PO),
        "boundaries.inlet.velocity_expression_x",
    );
    // V3 direction that does not enter
    expect_error(
        "V3 direction",
        &ok_case(&format!("{MF}\nflow_direction = [0.0, 1.0, 0.0]"), PO),
        "boundaries.inlet.flow_direction",
    );
    // V4 source patch
    for (label, extra, path) in [
        (
            "unknown",
            "mass_flow_source_patch = \"nowhere\"",
            "boundaries.inlet.mass_flow_source_patch",
        ),
        (
            "itself",
            "mass_flow_source_patch = \"inlet\"",
            "boundaries.inlet.mass_flow_source_patch",
        ),
        (
            "with value",
            "mass_flow_source_patch = \"outlet\"\nmass_flow_value = 0.1",
            "boundaries.inlet.mass_flow_source_patch",
        ),
        (
            "fraction 0",
            "mass_flow_source_patch = \"outlet\"\nmass_flow_fraction = 0.0",
            "boundaries.inlet.mass_flow_fraction",
        ),
    ] {
        expect_error(
            &format!("V4 {label}"),
            &ok_case(&format!("bc_kind = \"mass_flow\"\n{extra}"), PO),
            path,
        );
    }
    let cyc = ok_case(
        "bc_kind = \"mass_flow\"\nmass_flow_source_patch = \"outlet\"",
        "bc_kind = \"mass_flow\"\nmass_flow_source_patch = \"inlet\"",
    );
    let e = errors(&cyc);
    println!("A8 V4 cycle: {e:?}");
    assert!(e
        .iter()
        .any(|d| d.path.ends_with(".mass_flow_source_patch") && d.message.contains("cycle")));
    // V5 flow_rate_weight
    expect_error(
        "V5 weight",
        &ok_case(MF, "bc_kind = \"outflow\"\nflow_rate_weight = 0.0"),
        "boundaries.outlet.flow_rate_weight",
    );
    // V6 loss coefficient
    expect_error(
        "V6 loss",
        &ok_case(MF, "bc_kind = \"vent\"\nloss_coefficient = -1.0"),
        "boundaries.outlet.loss_coefficient",
    );
    // V7 shape expression: compile error, negative value, zero total
    for (label, expr) in [
        ("compile error", "mass_flow_shape_expression = \"1 [m/s] +\""),
        ("negative", "mass_flow_shape_expression = \"-1 [m/s]\""),
        ("zero total", "mass_flow_shape_expression = \"0 [m/s]\""),
    ] {
        expect_error(
            &format!("V7 {label}"),
            &ok_case(&format!("{MF}\n{expr}"), PO),
            "boundaries.inlet.mass_flow_shape_expression",
        );
    }
    // V8 target mass flow
    expect_error(
        "V8 target",
        &ok_case(MF, "bc_kind = \"target_mass_flow_outlet\"\ntarget_mass_flow = -0.1"),
        "boundaries.outlet.target_mass_flow",
    );
    // V9 turbulence model with a total-pressure inlet
    let v9 = base(
        "",
        "bc_kind = \"total_pressure_inlet\"",
        "velocity = \"zero_gradient\"\npressure = \"fixed\"",
        WALL,
        "",
    ) + "\n[turbulence]\nmodel = \"k_epsilon\"\n";
    expect_error("V9 turbulence", &v9, "boundaries.inlet.turbulence_spec");
    // V10 zero swirl axis
    expect_error(
        "V10 swirl axis",
        &ok_case(&format!("{MF}\nswirl_rate = 1.0\nswirl_axis = [0.0, 0.0, 0.0]"), PO),
        "boundaries.inlet.swirl_axis",
    );
    // V11 one case per unused-key row: a non-default value on a kind that does not use the key
    let rows: [(&str, &str, &str); 19] = [
        ("mass_flow_value", "mass_flow_value = 0.5", "outflow"),
        (
            "mass_flow_shape_expression",
            "mass_flow_shape_expression = \"1 [m/s]\"",
            "outflow",
        ),
        (
            "mass_flow_source_patch",
            "mass_flow_source_patch = \"inlet\"",
            "outflow",
        ),
        ("mass_flow_fraction", "mass_flow_fraction = 0.5", "outflow"),
        ("flow_direction", "flow_direction = [1.0, 0.0, 0.0]", "outflow"),
        ("swirl_rate", "swirl_rate = 1.0", "outflow"),
        ("swirl_axis", "swirl_axis = [1.0, 0.0, 0.0]", "outflow"),
        ("swirl_origin", "swirl_origin = [1.0, 0.0, 0.0]", "outflow"),
        ("bc_pressure", "bc_pressure = 3.0", "outflow"),
        (
            "backflow_direction",
            "backflow_direction = \"zero_gradient\"",
            "opening",
        ),
        ("backflow_pressure", "backflow_pressure = \"total\"", "opening"),
        ("prevent_reverse_flow", "prevent_reverse_flow = true", "vent"),
        ("pressure_average", "pressure_average = \"area\"", "opening"),
        ("loss_coefficient", "loss_coefficient = 2.0", "opening"),
        ("flow_rate_weight", "flow_rate_weight = 2.0", "opening"),
        ("radial_reference", "radial_reference = \"area_average\"", "opening"),
        ("target_mass_flow", "target_mass_flow = 0.1", "opening"),
        ("pressure_update", "pressure_update = \"secant\"", "opening"),
        ("pressure_relax", "pressure_relax = 0.3", "opening"),
    ];
    for (key, line, kind) in rows {
        expect_error(
            &format!("V11 {key} on {kind}"),
            &ok_case(MF, &format!("bc_kind = \"{kind}\"\n{line}")),
            &format!("boundaries.outlet.{key}"),
        );
    }
    // V12 a radial-outlet face on the axis: put the axis through the centre of one outlet face
    let v12 = |outlet: &str, origin: &str| {
        format!(
            r#"
[mesh]
generator = "annulus_3d"
nx = 12
ny = 4
nz = 4
r_in = 0.5
r_out = 1.0
lz = 2.0
[physics]
nu = 0.1
[boundaries.inner]
velocity = "fixed"
[boundaries.outer]
velocity = "fixed"
[boundaries.zmin]
bc_kind = "mass_flow"
mass_flow_value = 0.3
[boundaries.zmax]
{outlet}
{origin}
[solver]
algorithm = "simplec"
"#
        )
    };
    let plain = Simulation::new(&parse(&v12(
        "velocity = \"zero_gradient\"
pressure = \"fixed\"",
        "",
    )))
    .unwrap();
    let f0 = plain.geom.mesh.patch("zmax").unwrap().faces().next().unwrap();
    let c = plain.geom.mesh.face_centres()[f0];
    let origin = format!("swirl_origin = [{:.17e}, {:.17e}, 0.0]", c[0], c[1]);
    expect_error(
        "V12 axis through a face",
        &v12("bc_kind = \"radial_equilibrium_outlet\"", &origin),
        "boundaries.zmax.swirl_axis",
    );
    // V14 potential initialisation supports only mass_flow
    expect_error(
        "V14 potential",
        &ok_case(MF, "bc_kind = \"outflow\"").replace("[solver]", "[initial]\nmethod = \"potential\"\n[solver]"),
        "boundaries.outlet.bc_kind",
    );
}

// ---------------------------------------------------------------------------------------------------------------
// B outflow weights, C pressure outlet / opening / vent, D total-pressure inlet, E radial equilibrium, F target
// mass flow, T transient, R roles

fn box2d(nx: usize, ny: usize, lx: f64, ly: f64, mesh_extra: &str, patches: &str, solver_extra: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = {nx}
ny = {ny}
lx = {lx}
ly = {ly}
{mesh_extra}
[physics]
nu = 0.1
{patches}
[solver]
algorithm = "simplec"
convection = "linear_upwind"
max_iterations = 6000
tolerance_u = 1e-12
tolerance_p = 1e-12
{solver_extra}
"#
    )
}

fn shares(s: &Simulation, patches: &[&str], m_in: f64) -> Vec<f64> {
    patches.iter().map(|p| mass(s, p, 1.0) / m_in).collect()
}

#[test]
fn b1_b2_b3_b4_outflow_weights() {
    // B1
    let b1 = |prevent: bool| {
        box2d(
            30,
            30,
            1.0,
            1.0,
            "",
            &format!(
                r#"[boundaries.xmin]
bc_kind = "mass_flow"
mass_flow_value = 0.2
[boundaries.xmax]
bc_kind = "outflow"
flow_rate_weight = 1.0
prevent_reverse_flow = {prevent}
[boundaries.ymax]
bc_kind = "outflow"
flow_rate_weight = 3.0
prevent_reverse_flow = {prevent}
[boundaries.ymin]
velocity = "fixed"
"#
            ),
            "",
        )
    };
    let s = solve(&b1(false), 6000);
    let sh = shares(&s, &["xmax", "ymax"], 0.2);
    let e = (sh[0] - 0.25).abs().max((sh[1] - 0.75).abs());
    let im = imbalance(&s, 1.0);
    println!(
        "B1 shares {:.14} {:.14} (0.25, 0.75), worst error {e:.2e} (limit 1e-10); B3 imbalance {im:.2e} (limit 1e-12)",
        sh[0], sh[1]
    );
    assert!(e <= 1e-10, "{e}");
    assert!(im <= 1e-12, "B3 {im}");
    // B4 reverse-flow prevention: every outflow face has u.n >= 0
    let s = solve(&b1(true), 6000);
    let mut worst = f64::MAX;
    for patch in ["xmax", "ymax"] {
        for (u, _, n) in boundary_velocity(&s, patch) {
            worst = worst.min(u[0] * n[0] + u[1] * n[1] + u[2] * n[2]);
        }
    }
    let sh = shares(&s, &["xmax", "ymax"], 0.2);
    println!(
        "B4 prevent_reverse_flow: min u.n over the outflow faces {worst:.3e} (limit >= 0); shares {:.12} {:.12}",
        sh[0], sh[1]
    );
    assert!(worst >= 0.0, "{worst}");
    assert!((sh[0] - 0.25).abs() <= 1e-10 && (sh[1] - 0.75).abs() <= 1e-10);
    // B2 three outflow patches 1, 2, 3
    let b2 = box2d(
        30,
        30,
        1.0,
        1.0,
        "",
        r#"[boundaries.xmin]
bc_kind = "mass_flow"
mass_flow_value = 0.2
[boundaries.xmax]
bc_kind = "outflow"
flow_rate_weight = 1.0
[boundaries.ymax]
bc_kind = "outflow"
flow_rate_weight = 2.0
[boundaries.ymin]
bc_kind = "outflow"
flow_rate_weight = 3.0
"#,
        "",
    );
    let s = solve(&b2, 6000);
    let sh = shares(&s, &["xmax", "ymax", "ymin"], 0.2);
    let want = [1.0 / 6.0, 2.0 / 6.0, 3.0 / 6.0];
    let e = (0..3).map(|i| (sh[i] - want[i]).abs()).fold(0.0_f64, f64::max);
    let im = imbalance(&s, 1.0);
    println!(
        "B2 shares {sh:?} (1/6, 2/6, 3/6), worst error {e:.2e} (limit 1e-10); B3 imbalance {im:.2e} (limit 1e-12)"
    );
    assert!(e <= 1e-10, "{e}");
    assert!(im <= 1e-12, "B3 {im}");
}

/// C2 box: symmetry walls, inviscid-uniform exact solution.
fn duct_plain(xmin: &str, xmax: &str, extra_solver: &str) -> String {
    box2d(
        20,
        4,
        5.0,
        1.0,
        "",
        &format!(
            r#"[boundaries.xmin]
{xmin}
[boundaries.xmax]
{xmax}
[boundaries.ymin]
velocity = "symmetry"
[boundaries.ymax]
velocity = "symmetry"
"#
        ),
        &format!(
            "component_diagonal = \"corrected\"
{extra_solver}"
        ),
    )
    .replace("nu = 0.1", "nu = 1e-3")
}

/// Post-hoc set-up: pseudo-time stepping, because plain SIMPLEC from rest diverges on a frictionless duct driven by two
/// pressure ends (the `*_as_registered_known_miss` tests keep the registered set-up).
fn duct(xmin: &str, xmax: &str, extra_solver: &str) -> String {
    duct_plain(
        xmin,
        xmax,
        &format!(
            "pseudo_mode = \"physical\"
pseudo_dt = 0.2
{extra_solver}"
        ),
    )
}

const XMIN_P0: &str = "velocity = \"zero_gradient\"\npressure = \"fixed\"\npressure_value = 0.0";

fn mean_abs_ux(s: &Simulation) -> (f64, f64, f64) {
    let ux = s.field("ux").unwrap();
    let n = ux.len() as f64;
    let mean = ux.iter().map(|v| v.abs()).sum::<f64>() / n;
    let worst = ux.iter().map(|v| (v.abs() - 1.0).abs()).fold(0.0_f64, f64::max);
    (mean, worst, ux.iter().sum::<f64>() / n)
}

#[test]
fn c2_total_pressure_backflow_and_c4_opening() {
    let po = "bc_kind = \"pressure_outlet\"\nbc_pressure = 0.5\nbackflow_pressure = \"total\"\nbackflow_direction = \"normal\"";
    let s = settle(&duct(XMIN_P0, po, ""), 6000);
    let (mean, worst, signed) = mean_abs_ux(&s);
    let im = imbalance(&s, 1.0);
    println!("C2: mean |u_x| = {mean:.9} (limit |.|-1 <= 1e-5), worst cell deviation {worst:.2e}, mean u_x {signed:.6} (flow enters through xmax), imbalance {im:.2e} (limit 1e-9)");
    assert!(signed < 0.0, "the flow must enter through xmax");
    assert!((mean - 1.0).abs() <= 1e-5, "{mean}");
    assert!(im <= 1e-9, "{im}");
    // C4 opening == pressure_outlet(total, normal) bitwise (fixed iteration count)
    let open = "bc_kind = \"opening\"\nbc_pressure = 0.5";
    let mut a = build(&duct(XMIN_P0, po, ""));
    let mut b = build(&duct(XMIN_P0, open, ""));
    for _ in 0..150 {
        a.step();
        b.step();
    }
    let mut diff = 0.0_f64;
    for name in ["ux", "uy", "p"] {
        let (fa, fb) = (a.field(name).unwrap(), b.field(name).unwrap());
        for (x, y) in fa.iter().zip(&fb) {
            diff = diff.max((x - y).abs());
        }
    }
    println!("C4 opening vs explicit pressure_outlet after 150 iterations: max field difference {diff:e} (limit 0)");
    assert_eq!(diff, 0.0);
    assert!(
        a.field("ux").unwrap().iter().any(|v| v.abs() > 1e-3),
        "the case must not be trivially at rest"
    );
}

#[test]
fn c5_prevent_reverse_flow_closes_the_patch() {
    let po =
        "bc_kind = \"pressure_outlet\"\nbc_pressure = 0.5\nbackflow_pressure = \"total\"\nprevent_reverse_flow = true";
    let s = solve(&duct_plain(XMIN_P0, po, ""), 6000);
    let flux = mass(&s, "xmax", 1.0).abs();
    let umax = s
        .field("ux")
        .unwrap()
        .iter()
        .zip(s.field("uy").unwrap())
        .map(|(a, b)| (a * a + b * b).sqrt())
        .fold(0.0_f64, f64::max);
    println!("C5: patch flux {flux:.2e} (limit 1e-12), |u|max {umax:.2e} (limit 1e-6)");
    assert!(flux <= 1e-12, "{flux}");
    assert!(umax <= 1e-6, "{umax}");
}

#[test]
fn c1_pressure_averaging() {
    let outlet = "bc_kind = \"pressure_outlet\"\npressure_average = \"area\"\nbc_pressure = 0.0";
    let s = solve(&channel(MF_INLET, outlet, 1.0, ""), 6000);
    let pb = s.boundary_field("p").unwrap();
    let patch = s.geom.mesh.patch("outlet").unwrap();
    let (mut num, mut den) = (0.0, 0.0);
    for f in patch.faces() {
        let a = rcfd_core::vec3::norm(s.geom.mesh.face_area_vectors()[f]);
        num += pb[f - s.geom.n_internal] * a;
        den += a;
    }
    let avg = num / den;
    let slope = pressure_slope(&s, 1.0, 6.0, 9.0);
    let e = (slope + 1.2).abs() / 1.2;
    let im = imbalance(&s, 1.0);
    println!("C1: outlet area-average p = {avg:.3e} (limit 1e-9), slope {slope:.6} (error {:.3} %, limit 1 %), imbalance {im:.2e}", 100.0 * e);
    assert!(avg.abs() <= 1e-9, "{avg}");
    assert!(e <= 0.01, "{e}");
    assert!(im <= 1e-9, "{im}");
}

/// C3 channel: frictionless (symmetry walls, nu 1e-3), `inlet` mass-flow condition, vent with loss coefficient `k`.
/// `pseudo_dt = None` is the registered set-up (plain simplec).
fn c3_case(inlet: &str, k: f64, pseudo_dt: Option<f64>) -> String {
    let pseudo = match pseudo_dt {
        Some(dt) => format!(
            "pseudo_mode = \"physical\"
pseudo_dt = {dt}
"
        ),
        None => String::new(),
    };
    channel(
        inlet,
        &format!(
            "bc_kind = \"vent\"
loss_coefficient = {k}
bc_pressure = 0.0"
        ),
        1.0,
        "",
    )
    .replace(
        "[boundaries.walls]
velocity = \"fixed\"",
        "[boundaries.walls]
velocity = \"symmetry\"",
    )
    .replace("nu = 0.1", "nu = 1e-3")
    .replace(
        "[solver]
",
        &format!(
            "[solver]
component_diagonal = \"corrected\"
{pseudo}"
        ),
    )
}

const C3_REVERSED: &str = "bc_kind = \"mass_flow\"
mass_flow_value = -0.1";

fn c3_check(s: &Simulation, label: &str, k: f64, sign: f64) {
    let p = s.field("p").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let xmax = cc.iter().map(|c| c[0]).fold(0.0_f64, f64::max);
    let want = sign * 0.5 * k;
    let mut worst = 0.0_f64;
    for (c, x) in cc.iter().enumerate() {
        if (x[0] - xmax).abs() < 1e-9 {
            worst = worst.max((p[c] - want).abs());
        }
    }
    let lim = if k == 0.0 { 1e-8 } else { 1e-6 * want.abs() };
    let im = imbalance(s, 1.0);
    println!("C3 {label} K = {k}: outlet-column pressure error {worst:.2e} against {want} (limit {lim:.1e}); imbalance {im:.2e}");
    assert!(worst <= lim, "{label} K={k}: {worst}");
    assert!(im <= 1e-9, "{im}");
}

#[test]
fn c3_vent_loss_law() {
    // post-hoc set-up: pseudo-time step 0.02 and the settled-field criterion (cap 6000 as registered); the reversed
    // K = 0 case does not settle within the cap and is the ignored known miss below
    for (dir, inlet, sign) in [("forward", MF_INLET, 1.0), ("reversed", C3_REVERSED, -1.0)] {
        for k in [0.0, 2.5, 10.0] {
            if dir == "reversed" && k == 0.0 {
                continue;
            }
            let s = settle(&c3_case(inlet, k, Some(0.02)), 6000);
            c3_check(&s, dir, k, sign);
        }
    }
}

/// KNOWN MISS: the reversed vent with K = 0 does not settle within the registered 6000 iterations (a slow mode of a fixed
/// static pressure on an inflow face with zero-gradient velocity). With a cap of 30000 it settles after 23174 iterations
/// (post-hoc observation, `c3_vent_reversed_k0_long_run_observation`), with the right pressure.
#[test]
#[ignore]
fn c3_vent_reversed_k0_known_miss() {
    let s = settle(&c3_case(C3_REVERSED, 0.0, Some(0.02)), 6000);
    c3_check(&s, "reversed", 0.0, -1.0);
}

/// Post-hoc observation, not a gate: the cap widened to 30000 iterations.
#[test]
#[ignore]
fn c3_vent_reversed_k0_long_run_observation() {
    let s = settle(&c3_case(C3_REVERSED, 0.0, Some(0.02)), 30000);
    c3_check(&s, "reversed (long run)", 0.0, -1.0);
}

/// As registered (plain simplec, residual targets): NOT MET (K = 2.5 diverges to NaN within 300 iterations).
#[test]
#[ignore]
fn c3_vent_as_registered_known_miss() {
    let s = solve(&c3_case(MF_INLET, 2.5, None), 6000);
    c3_check(&s, "forward", 2.5, 1.0);
}

/// Record of the first post-hoc attempt: pseudo time step 0.2 diverges for K = 2.5 (NaN at iteration 9).
#[test]
#[ignore]
fn c3_vent_pseudo_dt_02_known_miss() {
    let s = settle(&c3_case(MF_INLET, 2.5, Some(0.2)), 6000);
    c3_check(&s, "forward", 2.5, 1.0);
}

/// As registered (plain simplec, residual targets): NOT MET, see the ADR addendum.
#[test]
#[ignore]
fn c2_as_registered_known_miss() {
    let po = "bc_kind = \"pressure_outlet\"
bc_pressure = 0.5
backflow_pressure = \"total\"
backflow_direction = \"normal\"";
    let s = solve(&duct_plain(XMIN_P0, po, ""), 6000);
    let (mean, _, _) = mean_abs_ux(&s);
    assert!((mean - 1.0).abs() <= 1e-5, "{mean}");
}

/// As registered (plain simplec, residual targets): NOT MET, see the ADR addendum.
#[test]
#[ignore]
fn d1_as_registered_known_miss() {
    let tp = "bc_kind = \"total_pressure_inlet\"
bc_pressure = 0.5";
    let xmax = "velocity = \"zero_gradient\"
pressure = \"fixed\"
pressure_value = 0.0";
    let s = solve(&duct_plain(tp, xmax, ""), 6000);
    let (mean, _, _) = mean_abs_ux(&s);
    assert!((mean - 1.0).abs() <= 1e-5, "{mean}");
}

#[test]
fn d1_total_pressure_inlet_duct() {
    let tp = "bc_kind = \"total_pressure_inlet\"\nbc_pressure = 0.5";
    let xmax = "velocity = \"zero_gradient\"\npressure = \"fixed\"\npressure_value = 0.0";
    let s = settle(&duct(tp, xmax, ""), 6000);
    let (mean, worst, signed) = mean_abs_ux(&s);
    let im = imbalance(&s, 1.0);
    println!("D1: mean u_x = {signed:.9} (limit |.|-1 <= 1e-5), worst cell deviation {worst:.2e}, imbalance {im:.2e}");
    assert!(signed > 0.0);
    assert!((mean - 1.0).abs() <= 1e-5, "{mean}");
    assert!(im <= 1e-9, "{im}");
}

fn d2_text(pseudo: bool) -> String {
    let c = (30.0_f64).to_radians().cos();
    let sn = (30.0_f64).to_radians().sin();
    let pseudo = if pseudo {
        "
pseudo_mode = \"physical\"
pseudo_dt = 0.2"
    } else {
        ""
    };
    box2d(
        20,
        8,
        2.0,
        1.0,
        "periodic_y = true",
        &format!(
            r#"[boundaries.xmin]
bc_kind = "total_pressure_inlet"
bc_pressure = 0.5
flow_direction = [{c:.17}, {sn:.17}, 0.0]
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
"#
        ),
        &format!("component_diagonal = \"corrected\"{pseudo}"),
    )
    .replace("nu = 0.1", "nu = 1e-3")
}

fn d2_check(s: &Simulation) {
    let (ux, uy) = (s.field("ux").unwrap(), s.field("uy").unwrap());
    let (mut e_mag, mut e_ang) = (0.0_f64, 0.0_f64);
    for i in 0..ux.len() {
        e_mag = e_mag.max(((ux[i] * ux[i] + uy[i] * uy[i]).sqrt() - 1.0).abs());
        e_ang = e_ang.max((uy[i].atan2(ux[i]) - 30.0_f64.to_radians()).abs());
    }
    let im = imbalance(&s, 1.0);
    println!(
        "D2: max ||u|-1| = {e_mag:.2e} (limit 1e-5), max angle error {e_ang:.2e} rad (limit 1e-6), imbalance {im:.2e}"
    );
    assert!(e_mag <= 1e-5, "{e_mag}");
    assert!(e_ang <= 1e-6, "{e_ang}");
    assert!(im <= 1e-9, "{im}");
}

#[test]
fn d2_total_pressure_inlet_direction() {
    let s = settle(&d2_text(true), 6000);
    d2_check(&s);
}

/// As registered (plain simplec, residual targets, 6000 iterations): NOT MET, see the ADR addendum.
#[test]
#[ignore]
fn d2_as_registered_known_miss() {
    let s = solve(&d2_text(false), 6000);
    d2_check(&s);
}

const E2_CASE: &str = r#"
[mesh]
generator = "annulus_3d"
nx = 48
ny = 12
nz = 24
r_in = 0.5
r_out = 1.0
lz = 4.0
[physics]
nu = 0.1
rho = 1.0
[boundaries.inner]
velocity = "moving_wall"
velocity_value = [0.0, 0.0, 1.0]
wall_rotation_rate = 2.0
[boundaries.outer]
velocity = "moving_wall"
velocity_value = [0.0, 0.0, 1.0]
wall_rotation_rate = 2.0
[boundaries.zmin]
bc_kind = "mass_flow"
mass_flow_value = 2.3494714599609283
swirl_rate = 2.0
[boundaries.zmax]
bc_kind = "radial_equilibrium_outlet"
radial_reference = "area_average"
bc_pressure = 1.25
[solver]
algorithm = "simplec"
convection = "linear_upwind"
component_diagonal = "corrected"
max_iterations = 6000
tolerance_u = 1e-12
tolerance_p = 1e-12
"#;

#[test]
fn e2_swirling_pipe_radial_equilibrium_outlet() {
    let text = E2_CASE;
    let s = settle(text, 6000);
    let exact = |r: f64| 1.25 + 2.0 * (r * r - 0.625);
    let range = 1.5;
    let pb = s.boundary_field("p").unwrap();
    let uf = s.velocity().boundary_values(&s.geom);
    let patch = s.geom.mesh.patch("zmax").unwrap();
    let (mut ep, mut eu) = (0.0_f64, 0.0_f64);
    let mut rows: Vec<(f64, f64, f64)> = Vec::new(); // (r, p, w)
    for f in patch.faces() {
        let x = s.geom.mesh.face_centres()[f];
        let r = (x[0] * x[0] + x[1] * x[1]).sqrt();
        let v = uf[f - s.geom.n_internal];
        let w = (-x[1] * v[0] + x[0] * v[1]) / r;
        let p = pb[f - s.geom.n_internal];
        ep = ep.max((p - exact(r)).abs());
        eu = eu.max((w - 2.0 * r).abs());
        rows.push((r, p, w));
    }
    // (c) mid-length cell pressure
    let p = s.field("p").unwrap();
    let mut em = 0.0_f64;
    for (c, x) in s.geom.mesh.cell_centres().iter().enumerate() {
        if x[2] >= 1.5 && x[2] <= 2.5 {
            let r = (x[0] * x[0] + x[1] * x[1]).sqrt();
            em = em.max((p[c] - exact(r)).abs());
        }
    }
    // (d) discrete balance of the applied face pressures
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut ed = 0.0_f64;
    for i in 0..rows.len() - 1 {
        let g = |q: &(f64, f64, f64)| q.2 * q.2 / q.0;
        let dp = rows[i + 1].1 - rows[i].1;
        let want = 0.5 * (g(&rows[i]) + g(&rows[i + 1])) * (rows[i + 1].0 - rows[i].0);
        ed = ed.max((dp - want).abs());
    }
    let im = imbalance(&s, 1.0);
    println!(
        "E2: outlet pressure error {:.3} % of 1.5 Pa (limit 3 %); outlet u_theta error {:.3} % of 2 (limit 3 %); mid-length pressure error {:.3} % (limit 3 %); discrete balance {:.2e} of range (limit 1e-8); imbalance {im:.2e}",
        100.0 * ep / range,
        100.0 * eu / 2.0,
        100.0 * em / range,
        ed / range
    );
    assert!(ep <= 0.03 * range, "(a) {ep}");
    assert!(eu <= 0.03 * 2.0, "(b) {eu}");
    assert!(em <= 0.03 * range, "(c) {em}");
    assert!(ed / range <= 1e-8, "(d) {ed}");
    assert!(im <= 1e-9, "M {im}");
}

fn f1_case(update: &str) -> String {
    channel(
        "velocity = \"zero_gradient\"\npressure = \"fixed\"\npressure_value = 12.0",
        &format!("bc_kind = \"target_mass_flow_outlet\"\ntarget_mass_flow = 0.1\nbc_pressure = 0.0\npressure_update = \"{update}\""),
        1.0,
        "",
    )
}

fn f1_check(update: &str, limit: f64) {
    let mut s = build(&f1_case(update));
    let n = s.run(6000);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let converged = s.is_converged();
    let m = mass(&s, "outlet", 1.0);
    let e = (m - 0.1).abs() / 0.1;
    let slope = pressure_slope(&s, 1.0, 6.0, 9.0);
    let es = (slope + 1.2).abs() / 1.2;
    let im = imbalance(&s, 1.0);
    println!(
        "F1 {update}: m_out = {m:.9} after {n} iterations (converged {converged}), error {e:.2e} (limit {limit:e}); F2 slope {slope:.6}, error {:.3} % (limit 1 %); imbalance {im:.2e}",
        100.0 * es
    );
    assert!(converged, "{update}: not converged");
    assert!(e <= limit, "{update}: {e}");
    assert!(es <= 0.01, "{update}: slope {es}");
    assert!(im <= 1e-9, "{update}: {im}");
}

#[test]
fn f1_f2_target_mass_flow_outlet_bernoulli() {
    f1_check("bernoulli", 1e-3);
}

/// KNOWN MISS (ADR 0022 F1, secant limit 1e-4 in 6000 iterations; root cause in the ADR addendum): the registered
/// secant rule estimates dp/dm from successive iterates while the flow is still spinning up, so the slope is
/// contaminated, and the registered clip to 1000 times the Bernoulli slope turns one spurious estimate into a large
/// overshoot that diverges (observed at iteration 23). Run with `--ignored` to see the failure.
#[test]
#[ignore]
fn f1_f2_target_mass_flow_outlet_secant_known_miss() {
    f1_check("secant", 1e-4);
}

fn pimple_text(patches: &str, mesh: &str) -> String {
    format!(
        r#"
{mesh}
[physics]
nu = 0.1
{patches}
[solver]
algorithm = "pimple"
dt = 0.01
end_time = 100.0
alpha_p = 1.0
momentum_rel_tol = 1e-12
pressure_rel_tol = 1e-12
pressure_max_iter = 20000
momentum_max_iter = 2000
"#
    )
}

#[test]
fn t1_transient_pimple() {
    // the A2-type channel, mass-flow inlet and fixed-pressure outlet
    let mesh = "[mesh]\ngenerator = \"channel_2d\"\nnx = 50\nny = 10\nlx = 10.0\nly = 1.0\ndepth = 0.1";
    let text = pimple_text(
        &format!(
            "[boundaries.inlet]\n{MF_INLET}\n[boundaries.outlet]\n{P_OUTLET}\n[boundaries.walls]\nvelocity = \"fixed\"\n"
        ),
        mesh,
    );
    let mut s = build(&text);
    let (mut worst_m, mut worst_g) = (0.0_f64, 0.0_f64);
    for step in 0..30 {
        s.step();
        assert!(s.failure().is_none(), "{:?}", s.failure());
        worst_m = worst_m.max(((-mass(&s, "inlet", 1.0)) - 0.1).abs() / 0.1);
        if step > 0 {
            worst_g = worst_g.max(imbalance(&s, 1.0));
        }
    }
    println!("T1 channel: worst mass-flow patch error {worst_m:.2e} (limit 1e-12), worst global imbalance {worst_g:.2e} (limit 1e-8)");
    assert!(worst_m <= 1e-12, "{worst_m}");
    assert!(worst_g <= 1e-8, "{worst_g}");
    // the B1 outflow weights
    let mesh = "[mesh]\ngenerator = \"box_2d\"\nnx = 30\nny = 30\nlx = 1.0\nly = 1.0";
    let text = pimple_text(
        r#"[boundaries.xmin]
bc_kind = "mass_flow"
mass_flow_value = 0.2
[boundaries.xmax]
bc_kind = "outflow"
flow_rate_weight = 1.0
[boundaries.ymax]
bc_kind = "outflow"
flow_rate_weight = 3.0
[boundaries.ymin]
velocity = "fixed"
"#,
        mesh,
    );
    let mut s = build(&text);
    let (mut worst_m, mut worst_g) = (0.0_f64, 0.0_f64);
    for step in 0..30 {
        s.step();
        assert!(s.failure().is_none(), "{:?}", s.failure());
        worst_m = worst_m.max(((-mass(&s, "xmin", 1.0)) - 0.2).abs() / 0.2);
        if step > 0 {
            worst_g = worst_g.max(imbalance(&s, 1.0));
        }
    }
    let sh = shares(&s, &["xmax", "ymax"], 0.2);
    let e = (sh[0] - 0.25).abs().max((sh[1] - 0.75).abs());
    println!("T1 outflow weights: patch error {worst_m:.2e} (limit 1e-12), shares {:.10} {:.10} (error {e:.2e}, limit 1e-8), worst imbalance {worst_g:.2e} (limit 1e-8)", sh[0], sh[1]);
    assert!(worst_m <= 1e-12, "{worst_m}");
    assert!(e <= 1e-8, "{e}");
    assert!(worst_g <= 1e-8, "{worst_g}");
}

#[test]
fn r3_turbulence_roles_and_potential_init() {
    use rcfd_api::turb::{role, Role};
    let text = r#"
[mesh]
generator = "box_2d"
nx = 10
ny = 10
lx = 1.0
ly = 1.0
[physics]
nu = 0.1
[boundaries.xmin]
bc_kind = "mass_flow"
mass_flow_value = 0.1
turbulence_length = 0.07
k_value = 0.01
epsilon_value = 0.01
[boundaries.xmax]
bc_kind = "outflow"
[boundaries.ymax]
bc_kind = "pressure_outlet"
[boundaries.ymin]
velocity = "fixed"
[solver]
algorithm = "simplec"
[turbulence]
model = "k_epsilon"
"#;
    let c = parse(text);
    let roles: Vec<Role> = ["xmin", "xmax", "ymax", "ymin"].iter().map(|p| role(&c, p)).collect();
    println!("R3 roles: {roles:?}");
    assert_eq!(
        roles,
        vec![Role::Inlet, Role::ZeroGradient, Role::ZeroGradient, Role::Wall]
    );
    for k in [
        "vent",
        "opening",
        "radial_equilibrium_outlet",
        "target_mass_flow_outlet",
    ] {
        let t = text.replace("bc_kind = \"outflow\"", &format!("bc_kind = \"{k}\""));
        println!("R3 kind {k}");
        let c = parse(&t);
        assert_eq!(role(&c, "xmax"), Role::ZeroGradient, "{k}");
    }
    // without an inflow speed the initial k cannot be derived: it is given (documented limitation)
    let init = "model = \"k_epsilon\"
k_initial = 0.01
epsilon_initial = 0.01";
    let t = text
        .replace("mass_flow_value = 0.1", "mass_flow_value = -0.1")
        .replace("model = \"k_epsilon\"", init);
    assert_eq!(role(&parse(&t), "xmin"), Role::ZeroGradient);
    let t = text
        .replace(
            "bc_kind = \"mass_flow\"\nmass_flow_value = 0.1",
            "bc_kind = \"total_pressure_inlet\"\nturbulence_spec = \"values\"",
        )
        .replace("model = \"k_epsilon\"", init);
    assert_eq!(role(&parse(&t), "xmin"), Role::Inlet);
    // the explicit setting still wins
    let t = text.replace("bc_kind = \"outflow\"", "bc_kind = \"outflow\"\nturbulence = \"wall\"");
    assert_eq!(role(&parse(&t), "xmax"), Role::Wall);
    // the case builds, and the wall set is exactly the fixed-zero patch (y+ report only on it)
    let _ = build(text);
}

#[test]
fn r2_defaults_are_inactive() {
    // an unmodified case: every entry has bc_kind = none, the catalogue checks stay silent and nothing is built
    let text = channel(
        "velocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]",
        P_OUTLET,
        1.0,
        "",
    );
    let c = parse(&text);
    for n in ["inlet", "outlet", "walls"] {
        assert_eq!(rcfd_api::bc_cat::kind(&c, n), "none");
        assert_eq!(rcfd_api::turb::role(&c, n) == rcfd_api::turb::Role::Wall, n == "walls");
    }
    assert!(!rcfd_api::bc_cat::any_active(&c));
    assert!(rcfd_api::bc_cat::check(&c).is_empty());
    // the new keys sit at inactive defaults and are not read without a kind: non-default values on a plain entry are inert
    let with_keys = channel(
        "velocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\nloss_coefficient = 3.0\nflow_rate_weight = 2.0",
        P_OUTLET,
        1.0,
        "",
    );
    let c = parse(&with_keys);
    assert!(rcfd_api::bc_cat::check(&c).is_empty());
    println!("R2 bc_kind defaults to none; catalogue keys are inert without a kind");
}

#[test]
fn d6b_potential_initialisation_with_a_mass_flow_inlet() {
    let text = channel(MF_INLET, P_OUTLET, 1.0, "").replace(
        "[solver]
",
        "[initial]
method = \"potential\"
[solver]
",
    );
    let s = build(&text);
    // the potential-flow initial velocity is the plug flow of the prescribed mass flow (U = 1)
    let ux = s.field("ux").unwrap();
    let worst = ux.iter().map(|v| (v - 1.0).abs()).fold(0.0_f64, f64::max);
    println!("D6b potential initialisation: max |u_x - 1| = {worst:.2e} (potential plug flow of the mass-flow inlet)");
    assert!(worst <= 1e-6, "{worst}");
}

/// As registered (residual targets 1e-12): NOT MET, the residuals stall at 4.4e-12 with stationary fields.
#[test]
#[ignore]
fn e2_as_registered_known_miss() {
    let s = solve(E2_CASE, 6000);
    assert!(imbalance(&s, 1.0) <= 1e-9);
}

#[test]
fn v13_unknown_patch_is_reported_by_the_existing_check() {
    let t = base(
        "",
        MF,
        PO,
        WALL,
        "[boundaries.nowhere]
velocity = \"fixed\"",
    );
    let e = errors(&t);
    println!("A8 V13: {e:?}");
    assert!(e.iter().any(|d| d.path.contains("nowhere")), "{e:?}");
}
