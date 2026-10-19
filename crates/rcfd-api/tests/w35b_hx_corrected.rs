//! ADR 0077 part D: the `lumped` z4 heat-exchanger channel with `solver.component_diagonal = "corrected"`. Hypothesis H-D1: the exact solution
//! is the plug flow u = (2, 0, 0), p = 0 (symmetry planes on both walls, no pressure loss, no buoyancy); `corrected` reaches it to round-off and the
//! wave-34C "not stationary" verdict is a relative criterion applied to a round-off pressure field. The set-up is the one of `lumped.rs` (ADR 0012).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

const RHO: f64 = 1.2;
const CP: f64 = 1005.0;
const U: f64 = 2.0;
const AREA: f64 = 0.2 * 0.1;

fn hx_case(diag: &str, block: &str) -> String {
    format!(
        "[mesh]\ngenerator = \"channel_2d\"\nnx = 100\nny = 2\nlx = 2.0\nly = 0.2\ndepth = 0.1\n[physics]\ncp = {CP:?}\nconductivity = 1e-3\nnu = {nu:?}\nrho = {RHO:?}\n\
[energy]\nenabled = true\nbuoyancy = false\nrelax = 1.0\n[initial]\ntemperature = 300.0\nvelocity = [1.0, 0.0, 0.0]\n\
[boundaries.outlet]\nvelocity = \"zero_gradient\"\npressure = \"fixed\"\n[boundaries.walls]\nvelocity = \"symmetry\"\n\
[boundaries.inlet]\nvelocity = \"fixed\"\nvelocity_value = [2.0, 0.0, 0.0]\ntemperature = \"fixed\"\ntemperature_value = 300.0\n\
[solver]\nalgorithm = \"simplec\"\nconvection = \"upwind\"\nh_operator = \"from_solution\"\nmomentum_rel_tol = 1e-8\npressure_rel_tol = 1e-8\npressure_max_iter = 1000\n\
max_iterations = 100000\ntolerance_u = 1e-14\ntolerance_p = 1e-14\ncomponent_diagonal = \"{diag}\"\n\
[components.hx]\ntype = \"heat_exchanger\"\nzone = \"box\"\nbox_min = [0.8, -1.0, -1.0]\nbox_max = [1.0, 1.0, 1.0]\ndirection = [1.0, 0.0, 0.0]\n{block}",
        nu = 1.8e-5 / RHO
    )
}

const FIXED: &str = "mode = \"fixed_duty\"\nduty = -500.0\n";

fn effectiveness(c_sec: f64) -> String {
    format!("mode = \"effectiveness\"\neffectiveness = 0.7\nsecondary_temperature = 350.0\nsecondary_capacity_rate = {c_sec:?}\n")
}

fn max_abs(v: &[f64], f: impl Fn(f64) -> f64) -> f64 {
    v.iter().fold(0.0f64, |a, &x| {
        if f(x).is_nan() {
            f64::INFINITY
        } else {
            a.max(f(x).abs())
        }
    })
}

/// Errors against the exact plug flow: (max|ux - 2|/U, max|uy|/U, max|p|/(U^2/2)).
fn errors(sim: &Simulation) -> [f64; 3] {
    [
        max_abs(&sim.field("ux").unwrap(), |x| x - U) / U,
        max_abs(&sim.field("uy").unwrap(), |x| x) / U,
        max_abs(&sim.field("p").unwrap(), |x| x) / (0.5 * U * U),
    ]
}

fn column_t(sim: &Simulation, x: f64) -> f64 {
    let c = sim.geom.mesh.cell_centres();
    let t = sim.field("T").unwrap();
    let i = (0..c.len())
        .min_by(|&a, &b| (c[a][0] - x).abs().total_cmp(&(c[b][0] - x).abs()))
        .unwrap();
    t[i]
}

/// Run to 10 000 iterations; at 1 500, 5 000 and 10 000 return the errors and the changes over the previous 500 iterations.
fn run(name: &str, text: &str) -> (Simulation, f64) {
    let mut sim = Simulation::new(&Case::parse_toml(text).unwrap()).unwrap();
    let mut worst: f64 = 0.0;
    let (mut ux0, mut p0) = (Vec::new(), Vec::new());
    for it in 1..=10_000 {
        sim.step();
        if [1000, 4500, 9500].contains(&it) {
            ux0 = sim.field("ux").unwrap();
            p0 = sim.field("p").unwrap();
        }
        if [1500, 5000, 10_000].contains(&it) {
            let e = errors(&sim);
            let (ux, p) = (sim.field("ux").unwrap(), sim.field("p").unwrap());
            let dux = ux.iter().zip(&ux0).fold(0.0f64, |a, (x, y)| a.max((x - y).abs())) / U;
            let dp = p.iter().zip(&p0).fold(0.0f64, |a, (x, y)| a.max((x - y).abs())) / (0.5 * U * U);
            println!(
                "{name} it {it}: |ux-2|/U {:.3e}, |uy|/U {:.3e}, |p|/(U^2/2) {:.3e}; change over 500: ux {dux:.3e}, p {dp:.3e}",
                e[0], e[1], e[2]
            );
            worst = worst.max(e[0]).max(e[1]).max(e[2]).max(dux).max(dp);
        }
    }
    (sim, worst)
}

#[test]
fn z4_corrected_reaches_the_exact_plug_flow() {
    let q = U * AREA;
    let mut worst: f64 = 0.0;
    // fixed duty
    let (sim, w) = run("fixed duty", &hx_case("corrected", FIXED));
    worst = worst.max(w);
    let rise = RHO * CP * q * (column_t(&sim, 1.99) - column_t(&sim, 0.01));
    let rep = sim.monitors.last("hx_duty").unwrap();
    println!(
        "fixed duty: enthalpy change {rise:.6} W vs -500 (rel {:.3e}), reported {rep} (rel {:.3e})",
        (rise / -500.0 - 1.0).abs(),
        (rep / -500.0 - 1.0).abs()
    );
    assert!((rise / -500.0 - 1.0).abs() < 1e-6, "enthalpy change {rise}");
    assert!((rep / -500.0 - 1.0).abs() < 1e-9, "reported duty {rep}");
    for c_sec in [0.0, 20.0] {
        let (sim, w) = run(
            &format!("effectiveness C_sec {c_sec}"),
            &hx_case("corrected", &effectiveness(c_sec)),
        );
        worst = worst.max(w);
        let rise = RHO * CP * q * (column_t(&sim, 1.99) - column_t(&sim, 0.01));
        let rep = sim.monitors.last("hx_duty").unwrap();
        let t_in = sim.monitors.last("hx_t_in").unwrap();
        let c_min = if c_sec > 0.0 { c_sec } else { RHO * CP * q };
        let expect = 0.7 * c_min * (350.0 - t_in);
        println!(
            "effectiveness C_sec {c_sec}: rise {rise:.6} vs duty {rep:.6} (rel {:.3e}); duty vs eps C_min (T_sec - T_in) {expect:.6} (rel {:.3e})",
            (rise / rep - 1.0).abs(),
            (rep / expect - 1.0).abs()
        );
        assert!((rise / rep - 1.0).abs() < 1e-6, "rise {rise} vs duty {rep}");
        assert!((rep / expect - 1.0).abs() < 1e-9, "duty {rep} vs {expect}");
    }
    println!("worst error / change against the exact plug flow {worst:.3e} (limit 1e-10)");
    assert!(worst <= 1e-10, "H-D1 falsified: {worst:e}");
}

/// Reported (no limit): the same measures with the `averaged` default, and the wave-34C relative reading of the `corrected` run.
#[test]
fn z4_averaged_error_and_the_34c_reading_reported() {
    let mut sim = Simulation::new(&Case::parse_toml(&hx_case("averaged", FIXED)).unwrap()).unwrap();
    for _ in 0..1500 {
        sim.step();
    }
    let e = errors(&sim);
    println!(
        "averaged it 1500: |ux-2|/U {:.3e}, |uy|/U {:.3e}, |p|/(U^2/2) {:.3e}",
        e[0], e[1], e[2]
    );
    let mut sim = Simulation::new(&Case::parse_toml(&hx_case("corrected", FIXED)).unwrap()).unwrap();
    for _ in 0..1500 {
        sim.step();
    }
    let before = sim.field("p").unwrap();
    for _ in 0..300 {
        sim.step();
    }
    let p = sim.field("p").unwrap();
    let scale = p.iter().map(|v| v.abs()).fold(1e-12, f64::max);
    let change = p.iter().zip(&before).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    println!(
        "corrected, 34C reading: p change over 300 iterations after 1500 = {change:.3e}, max|p| = {:.3e}, change / max(max|p|, 1e-12) = {:.3e}",
        p.iter().map(|v| v.abs()).fold(0.0, f64::max),
        change / scale
    );
}
