//! User transport equations (F-202, ADR 0019 checks U1 to U6). The limits are those of the ADR, written before this file
//! was run; none was changed afterwards.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use std::f64::consts::PI;

fn parse(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"))
}

fn run(text: &str, steps: usize) -> Simulation {
    let mut s = Simulation::new(&parse(text)).unwrap_or_else(|e| panic!("{e:?}"));
    s.run(steps);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    s
}

fn order(coarse: f64, fine: f64) -> f64 {
    (coarse / fine).log2()
}

fn rms(s: &Simulation, values: &[f64], exact: &dyn Fn(f64, f64) -> f64) -> f64 {
    let cc = s.geom.mesh.cell_centres();
    let vol = s.geom.mesh.cell_volumes();
    let tot: f64 = vol.iter().sum();
    let sum: f64 = (0..values.len())
        .map(|i| vol[i] * (values[i] - exact(cc[i][0], cc[i][1])).powi(2))
        .sum();
    (sum / tot).sqrt()
}

const REST: &str = r#"
[physics]
nu = 0.01
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
"#;

fn diffusion_1d(n: usize, diffusivity: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = {n}
ny = 2
lx = 1.0
ly = 0.1
{REST}
[scalars.s]
equation = "diffusive"
{diffusivity}
[scalars.s.boundaries.xmin]
type = "fixed"
value = 0.0
[scalars.s.boundaries.xmax]
type = "fixed"
value = 1.0
[solver]
algorithm = "simplec"
max_iterations = 50
"#
    )
}

#[test]
fn u1_diffusion_with_variable_diffusivity_is_second_order() {
    let mut errs = Vec::new();
    for n in [16, 32, 64, 128] {
        let s = run(
            &diffusion_1d(n, "diffusivity_expression = \"1 [m^2/s] * (1 + x / 1 [m])\""),
            50,
        );
        let v = s.field("s").unwrap();
        errs.push(rms(&s, &v, &|x, _| (1.0 + x).ln() / 2.0_f64.ln()));
    }
    let (o1, o2) = (order(errs[1], errs[2]), order(errs[2], errs[3]));
    println!("U1 RMS errors {errs:?} orders {o1:.3} {o2:.3}");
    assert!(errs[3] <= 8e-6, "U1 error at n = 128: {}", errs[3]);
    assert!(
        (1.9..=2.1).contains(&o1) && (1.9..=2.1).contains(&o2),
        "U1 orders {o1} {o2}"
    );
    // constant diffusivity: the profile is linear
    let s = run(&diffusion_1d(32, "diffusivity = 1.0"), 50);
    let v = s.field("s").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let dev = (0..v.len()).map(|i| (v[i] - cc[i][0]).abs()).fold(0.0, f64::max);
    println!("U1 constant diffusivity: largest deviation from linear {dev:.3e}");
    assert!(dev <= 1e-9, "U1 linear {dev}");
}

fn poisson(n: usize) -> String {
    let exact = "sin(pi * x / 1 [m]) * cos(pi * y / 1 [m])";
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = {n}
ny = {n}
lx = 1.0
ly = 1.0
{REST}
[scalars.s]
equation = "poisson"
diffusivity_expression = "1 [m^2/s] * (1 + x / 1 [m])"
source_expression = "-pi * cos(pi * x / 1 [m]) * cos(pi * y / 1 [m]) + 2 * pi^2 * (1 + x / 1 [m]) * sin(pi * x / 1 [m]) * cos(pi * y / 1 [m])"
[scalars.s.boundaries.xmin]
type = "fixed"
value_expression = "{exact}"
[scalars.s.boundaries.xmax]
type = "fixed"
value_expression = "{exact}"
[scalars.s.boundaries.ymin]
type = "fixed"
value_expression = "{exact}"
[scalars.s.boundaries.ymax]
type = "fixed"
value_expression = "{exact}"
[solver]
algorithm = "simplec"
max_iterations = 50
"#
    )
}

#[test]
fn u2_poisson_manufactured_solution_is_second_order() {
    let mut errs = Vec::new();
    for n in [8, 16, 32, 64] {
        let s = run(&poisson(n), 50);
        let v = s.field("s").unwrap();
        errs.push(rms(&s, &v, &|x, y| (PI * x).sin() * (PI * y).cos()));
    }
    let (o1, o2) = (order(errs[1], errs[2]), order(errs[2], errs[3]));
    println!("U2 RMS errors {errs:?} orders {o1:.3} {o2:.3}");
    assert!(errs[3] <= 2e-3, "U2 error at n = 64: {}", errs[3]);
    assert!(
        (1.8..=2.3).contains(&o1) && (1.8..=2.3).contains(&o2),
        "U2 orders {o1} {o2}"
    );
}

fn eigenmode(nx: usize, dt: f64, scheme: &str) -> f64 {
    let text = format!(
        r#"
[mesh]
generator = "box_2d"
nx = {nx}
ny = 2
lx = 1.0
ly = 0.1
{REST}
[scalars.s]
equation = "diffusive"
diffusivity = 0.1
initial_expression = "sin(pi * x / 1 [m])"
[scalars.s.boundaries.xmin]
type = "fixed"
value = 0.0
[scalars.s.boundaries.xmax]
type = "fixed"
value = 0.0
[solver]
algorithm = "pimple"
dt = {dt}
end_time = 1.0
time_scheme = "{scheme}"
"#
    );
    let mut s = Simulation::new(&parse(&text)).unwrap_or_else(|e| panic!("{e:?}"));
    let steps = s.run(10_000);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    assert_eq!(steps, (1.0 / dt).round() as usize);
    let v = s.field("s").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let amp = (-PI * PI * 0.1).exp();
    (0..v.len())
        .map(|i| (v[i] - (PI * cc[i][0]).sin() * amp).abs())
        .fold(0.0, f64::max)
}

#[test]
fn u3_transient_diffusive_eigenmode_has_the_time_orders_of_the_scheme() {
    let euler: Vec<f64> = [0.1, 0.05, 0.025]
        .iter()
        .map(|&dt| eigenmode(128, dt, "euler"))
        .collect();
    let (o1, o2) = (order(euler[0], euler[1]), order(euler[1], euler[2]));
    println!("U3 Euler errors {euler:?} orders {o1:.3} {o2:.3}");
    println!("U3 Euler nx = 256, dt = 0.025: {:.4e}", eigenmode(256, 0.025, "euler"));
    assert!(
        (0.9..=1.15).contains(&o1) && (0.9..=1.15).contains(&o2),
        "U3 Euler orders {o1} {o2}"
    );
    let bdf: Vec<f64> = [0.2, 0.1, 0.05].iter().map(|&dt| eigenmode(128, dt, "bdf2")).collect();
    let (p1, p2) = (order(bdf[0], bdf[1]), order(bdf[1], bdf[2]));
    println!("U3 BDF2 errors {bdf:?} orders {p1:.3} {p2:.3}");
    println!("U3 BDF2 nx = 256, dt = 0.05: {:.4e}", eigenmode(256, 0.05, "bdf2"));
    assert!(
        (1.8..=2.3).contains(&p1) && (1.8..=2.3).contains(&p2),
        "U3 BDF2 orders {p1} {p2}"
    );
}

fn channel(n: usize, scheme: &str, extra: &str) -> String {
    format!(
        r#"
[mesh]
generator = "channel_2d"
nx = {n}
ny = 2
lx = 1.0
ly = 0.1
depth = 0.01
[physics]
nu = 0.01
rho = 1.0
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.inlet]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "symmetry"
[scalars.s]
diffusivity = 0.1
initial = 0.0
[scalars.s.boundaries.inlet]
type = "fixed"
value = 1.0
{extra}
[solver]
algorithm = "simplec"
convection = "{scheme}"
max_iterations = 800
tolerance_u = 1e-10
tolerance_p = 1e-10
pressure_rel_tol = 1e-13
pressure_abs_tol = 1e-22
pressure_max_iter = 5000
"#
    )
}

fn reaction_exact(x: f64) -> f64 {
    let (u, d, k) = (1.0_f64, 0.1_f64, 0.5_f64);
    let r = (u * u + 4.0 * k * d).sqrt();
    let (m1, m2) = ((u + r) / (2.0 * d), (u - r) / (2.0 * d));
    let a = -m2 * m2.exp() / (m1 * m1.exp() - m2 * m2.exp());
    a * (m1 * x).exp() + (1.0 - a) * (m2 * x).exp()
}

fn reaction_error(n: usize, scheme: &str) -> f64 {
    let s = run(
        &channel(
            n,
            scheme,
            "[zone_sources.react]\ntarget = \"s\"\nlinear_coefficient = 0.5\n",
        ),
        800,
    );
    let v = s.field("s").unwrap();
    let cc = s.geom.mesh.cell_centres();
    (v.iter()
        .zip(cc)
        .map(|(a, c)| (a - reaction_exact(c[0])).powi(2))
        .sum::<f64>()
        / v.len() as f64)
        .sqrt()
}

#[test]
fn u4_advection_diffusion_reaction_has_the_orders_of_the_schemes() {
    let central: Vec<f64> = [40, 80, 160, 320]
        .iter()
        .map(|&n| reaction_error(n, "central"))
        .collect();
    let (o1, o2) = (order(central[1], central[2]), order(central[2], central[3]));
    println!("U4 central RMS errors {central:?} orders {o1:.3} {o2:.3}");
    assert!(
        (1.9..=2.1).contains(&o1) && (1.9..=2.1).contains(&o2),
        "U4 central orders {o1} {o2}"
    );
    let upwind: Vec<f64> = [80, 160, 320].iter().map(|&n| reaction_error(n, "upwind")).collect();
    let (p1, p2) = (order(upwind[0], upwind[1]), order(upwind[1], upwind[2]));
    println!("U4 upwind RMS errors {upwind:?} orders {p1:.3} {p2:.3}");
    assert!(
        (0.9..=1.15).contains(&p1) && (0.9..=1.15).contains(&p2),
        "U4 upwind orders {p1} {p2}"
    );
}

#[test]
fn u5_algebraic_variable_follows_its_expression() {
    let mut text = channel(40, "upwind", "");
    // scalar a is the transported one; no reaction
    text = text.replace("[scalars.s", "[scalars.a");
    text = text.replace(
        "[solver]",
        "[scalars.b]\nequation = \"algebraic\"\nexpression = \"2 * a + x / 1 [m] + T / 1 [K]\"\n[solver]",
    );
    text = text.replace("[boundaries.outlet]", "[energy]\nenabled = true\n[boundaries.outlet]");
    text = text.replace("[initial]", "[initial]\ntemperature = 300.0");
    text = text.replace(
        "[boundaries.inlet]",
        "[boundaries.inlet]\ntemperature = \"fixed\"\ntemperature_value = 300.0",
    );
    let s = run(&text, 800);
    let (a, b) = (s.field("a").unwrap(), s.field("b").unwrap());
    let t = s.field("T").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let dev = (0..a.len())
        .map(|i| (b[i] - (2.0 * a[i] + cc[i][0] + t[i])).abs())
        .fold(0.0, f64::max);
    println!(
        "U5 largest deviation of b from its expression {dev:.3e} (T = {:.1}, a(0) = {:.4})",
        t[0], a[0]
    );
    assert!(dev <= 1e-12, "U5 {dev}");
    assert!(b.iter().all(|v| *v > 300.0), "b carries T");
}

fn errors_of(text: &str) -> Vec<(String, String)> {
    let c = match Case::parse_toml(text) {
        Ok(c) => c,
        Err(d) => return d.into_iter().map(|d| (d.path, d.message)).collect(),
    };
    let mut out: Vec<(String, String)> = c.semantic_errors().into_iter().map(|d| (d.path, d.message)).collect();
    if out.is_empty() {
        if let Err(e) = Simulation::new(&c) {
            out.extend(e.into_iter().map(|d| (d.path, d.message)));
        }
    }
    out
}

#[test]
fn u6_diagnostics_are_located() {
    let with = |body: &str| {
        let text = diffusion_1d(8, "").replace("equation = \"diffusive\"\n", body);
        errors_of(&text)
    };
    let has = |d: &[(String, String)], path: &str, word: &str| {
        assert!(
            d.iter().any(|(p, m)| p == path && m.contains(word)),
            "expected {path} / '{word}' in {d:?}"
        );
    };
    let d = with("equation = \"diffusive\"\nschmidt = 1.0\ndiffusivity_expression = \"1 [m^2/s]\"\n");
    has(&d, "scalars.s.diffusivity_expression", "schmidt");
    let d = with("equation = \"diffusive\"\ndiffusivity_expression = \"1 [m]\"\n");
    has(&d, "scalars.s.diffusivity_expression", "dimension");
    let d = with("equation = \"diffusive\"\ndiffusivity = 1.0\nexpression = \"x / 1 [m]\"\n");
    has(&d, "scalars.s.expression", "algebraic");
    let d = with("equation = \"algebraic\"\nexpression = \"s + 1\"\n");
    assert!(
        d.iter().any(|(p, _)| p == "scalars.s.expression"),
        "self reference: {d:?}"
    );
    let d = with("equation = \"algebraic\"\nexpression = \"q + 1\"\n");
    assert!(
        d.iter().any(|(p, _)| p == "scalars.s.expression"),
        "unknown symbol: {d:?}"
    );
    let d = with("equation = \"nonsense\"\ndiffusivity = 1.0\n");
    assert!(
        d.iter().any(|(p, _)| p.starts_with("scalars.s.equation")),
        "unknown equation: {d:?}"
    );
    // an algebraic variable with boundary conditions
    let d = with("equation = \"algebraic\"\nexpression = \"x / 1 [m]\"\n");
    assert!(d.iter().any(|(p, _)| p.starts_with("scalars.s.boundaries")), "{d:?}");
    // transient scalar on a compressible case is still rejected
    let text = diffusion_1d(8, "diffusivity = 1.0").replace("algorithm = \"simplec\"", "algorithm = \"compressible\"");
    let d = errors_of(&text);
    assert!(d.iter().any(|(p, _)| p == "scalars.s"), "compressible: {d:?}");
}
