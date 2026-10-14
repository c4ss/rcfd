//! Rosseland and P1 radiation (F-130, ADR 0012). Limits are those registered in
//! docs/adr/0012-lumped-zones-radiation.md before the first run:
//! R1 P1 radiative equilibrium flux vs the analytic P1/Marshak solution 1e-3 (tau 0.1, 1, 10, emissivities 0.8/0.5,
//! scattering), R2 P1 vs the Milne transport asymptote 1 % at tau = 20, R3 Rosseland Kirchhoff variable linear 1e-3 and
//! wall flux 1e-3, R4 P1 energy balance 1e-4 / absorbed power 1e-6 / P1 vs Rosseland at tau = 50 within 5 %,
//! R5 located diagnostics.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::radiation::SIGMA;

const L: f64 = 1.0;
const T1: f64 = 1000.0;
const T2: f64 = 500.0;

/// Plates at x = 0 (inlet patch, T1) and x = L (outlet patch, T2); symmetry side walls.
#[allow(clippy::too_many_arguments)]
fn slab(len: f64, a: f64, s: f64, k: f64, model: &str, eps1: f64, eps2: f64, nx: usize) -> String {
    format!(
        r#"
[mesh]
generator = "channel_2d"
nx = {nx}
ny = 2
lx = {len}
ly = 0.1
depth = 0.1
[physics]
nu = 0.01
rho = 1.2
cp = 1005.0
conductivity = {k}
[energy]
enabled = true
buoyancy = false
relax = 1.0
[initial]
temperature = 750.0
[boundaries.inlet]
velocity = "fixed"
temperature = "fixed"
temperature_value = {T1}
emissivity = {eps1}
[boundaries.outlet]
velocity = "fixed"
temperature = "fixed"
temperature_value = {T2}
emissivity = {eps2}
[boundaries.walls]
velocity = "symmetry"
[radiation]
model = "{model}"
absorption_coefficient = {a}
scattering_coefficient = {s}
[reports.q_in]
type = "heat_flow"
patch = "inlet"
conductivity = {k}
[reports.q_out]
type = "heat_flow"
patch = "outlet"
conductivity = {k}
[solver]
algorithm = "simple"
convection = "upwind"
max_iterations = 100000
tolerance_u = 1e-14
tolerance_p = 1e-14
"#
    )
}

fn run(text: &str, n: usize) -> Simulation {
    let case = Case::parse_toml(text).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    for _ in 0..n {
        sim.step();
    }
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    sim
}

fn cond(sim: &Simulation, name: &str) -> f64 {
    sim.report_values()
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap()
        .1
        .unwrap()
        .value
}

/// Run until the radiative wall flux changes by less than 1e-10 relative over 200 iterations (or the cap).
fn run_converged(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    let mut last = f64::NAN;
    for k in 0..60 {
        for _ in 0..200 {
            sim.step();
        }
        let q = sim.radiative_heat_flow("outlet").unwrap();
        if ((q - last) / q).abs() < 1e-10 {
            eprintln!("  converged after {} iterations", 200 * (k + 1));
            break;
        }
        last = q;
    }
    sim
}

fn area() -> f64 {
    0.1 * 0.1
}

fn p1_flux(tau: f64, e1: f64, e2: f64) -> f64 {
    SIGMA * (T1.powi(4) - T2.powi(4)) / (1.0 / e1 + 1.0 / e2 - 1.0 + 0.75 * tau)
}

#[test]
fn r1_p1_radiative_equilibrium_matches_the_analytic_solution() {
    let mut worst: f64 = 0.0;
    for (tau, s_frac, e1, e2) in [
        (0.1, 0.0, 1.0, 1.0),
        (1.0, 0.0, 1.0, 1.0),
        (10.0, 0.0, 1.0, 1.0),
        (1.0, 0.0, 0.8, 0.5),
        (1.0, 0.5, 1.0, 1.0),
    ] {
        let beta = tau / L;
        let sim = run_converged(&slab(L, beta * (1.0 - s_frac), beta * s_frac, 1e-9, "p1", e1, e2, 100));
        // heat leaving the hot plate (inlet) is negative of the radiative flux into the medium
        let q_hot = -sim.radiative_heat_flow("inlet").unwrap() / area();
        let q_cold = sim.radiative_heat_flow("outlet").unwrap() / area();
        let exact = p1_flux(tau, e1, e2);
        let err = ((q_hot / exact) - 1.0).abs().max(((q_cold / exact) - 1.0).abs());
        eprintln!(
            "R1 tau {tau} scat {s_frac} eps {e1}/{e2}: q {q_hot:.3} / {q_cold:.3} vs {exact:.3} ({:.3e})",
            err
        );
        worst = worst.max(err);
        assert!(err < 1e-3, "tau {tau}: q {q_hot} {q_cold} vs {exact}");
    }
    eprintln!("R1 worst {worst:.3e} (limit 1e-3)");
}

#[test]
fn r2_p1_against_the_transport_asymptote() {
    let tau = 20.0;
    let sim = run_converged(&slab(L, tau / L, 0.0, 1e-9, "p1", 1.0, 1.0, 200));
    let q = sim.radiative_heat_flow("outlet").unwrap() / area();
    let psi = q / (SIGMA * (T1.powi(4) - T2.powi(4)));
    let psi_milne = 4.0 / (3.0 * tau + 6.0 * 0.7104);
    let psi_p1 = 1.0 / (1.0 + 0.75 * tau);
    let err = (psi / psi_milne - 1.0).abs();
    eprintln!(
        "R2 tau 20: Psi {psi:.6} (P1 analytic {psi_p1:.6}) vs Milne asymptote {psi_milne:.6}: {:.3}%",
        100.0 * err
    );
    assert!(err < 0.01, "Psi {psi} vs {psi_milne}");
}

#[test]
fn r3_rosseland_slab_has_a_linear_kirchhoff_variable() {
    let (len, beta, k) = (0.1, 100.0, 0.1);
    for (kk, label) in [(k, "k = 0.1"), (1e-9, "k = 1e-9")] {
        let sim = run_converged_ros(&slab(len, beta, 0.0, kk, "rosseland", 1.0, 1.0, 200));
        let x: Vec<f64> = sim.geom.mesh.cell_centres().iter().map(|c| c[0]).collect();
        let t = sim.field("T").unwrap();
        // Kirchhoff variable
        let phi = |t: f64| kk * t + 4.0 * SIGMA * t.powi(4) / (3.0 * beta);
        let (p1, p2) = (phi(T1), phi(T2));
        let dev = (0..t.len())
            .map(|i| (phi(t[i]) - (p1 + (p2 - p1) * x[i] / len)).abs())
            .fold(0.0, f64::max)
            / (p1 - p2);
        let q_exact = (p1 - p2) / len;
        let q = (cond(&sim, "q_out") + sim.radiative_heat_flow("outlet").unwrap()) / area();
        eprintln!(
            "R3 {label}: Kirchhoff deviation {dev:.3e}, wall flux {q:.4} vs {q_exact:.4} ({:+.4}%)",
            100.0 * (q / q_exact - 1.0)
        );
        assert!(dev < 1e-3, "{label}: Kirchhoff deviation {dev}");
        assert!((q / q_exact - 1.0).abs() < 1e-3, "{label}: flux {q} vs {q_exact}");
    }
}

fn run_converged_ros(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    let mut last = f64::NAN;
    for _ in 0..200 {
        for _ in 0..100 {
            sim.step();
        }
        let q = sim.radiative_heat_flow("outlet").unwrap() + cond(&sim, "q_out");
        if ((q - last) / q).abs() < 1e-12 {
            break;
        }
        last = q;
    }
    sim
}

#[test]
fn r4_p1_with_conduction_conserves_energy_and_approaches_rosseland() {
    let (len, a, k) = (0.1, 100.0, 0.1);
    let sim = run_converged(&slab(len, a, 0.0, k, "p1", 1.0, 1.0, 100));
    let (qr_in, qr_out) = (
        sim.radiative_heat_flow("inlet").unwrap(),
        sim.radiative_heat_flow("outlet").unwrap(),
    );
    let (qc_in, qc_out) = (cond(&sim, "q_in"), cond(&sim, "q_out"));
    let total = qr_in + qc_in + qr_out + qc_out;
    let scale = (qr_out + qc_out).abs();
    let absorbed = sim.radiative_absorption().unwrap();
    let net_rad = qr_in + qr_out;
    eprintln!(
        "R4 energy balance: wall sum {total:.4e} W vs flux {scale:.4e} W ({:.3e}); absorbed {absorbed:.6e} vs net radiative wall flow {net_rad:.6e} ({:.3e})",
        (total / scale).abs(),
        ((absorbed + net_rad) / qr_out.abs()).abs()
    );
    assert!((total / scale).abs() < 1e-4, "wall energy balance {total}");
    assert!(
        ((absorbed + net_rad) / qr_out.abs()).abs() < 1e-6,
        "absorbed power {absorbed} vs net radiative wall flow {net_rad}"
    );

    // P1 vs Rosseland at tau = 50 (a = 500 1/m, L = 0.1 m)
    let thick = 500.0;
    let p1 = run_converged(&slab(len, thick, 0.0, k, "p1", 1.0, 1.0, 200));
    let ros = run_converged_ros(&slab(len, thick, 0.0, k, "rosseland", 1.0, 1.0, 200));
    let q_p1 = p1.radiative_heat_flow("outlet").unwrap() + cond(&p1, "q_out");
    let q_ros = ros.radiative_heat_flow("outlet").unwrap() + cond(&ros, "q_out");
    eprintln!(
        "R4 tau 50: total flux P1 {q_p1:.4} W, Rosseland {q_ros:.4} W (ratio {:.4})",
        q_p1 / q_ros
    );
    assert!((q_p1 / q_ros - 1.0).abs() < 0.05);
}

#[test]
fn r5_invalid_radiation_settings_get_located_errors() {
    let diags = |text: &str| -> Vec<String> {
        match Case::parse_toml(text) {
            Err(d) => d.into_iter().map(|d| d.path).collect(),
            Ok(c) => c.semantic_errors().into_iter().map(|d| d.path).collect(),
        }
    };
    let good = slab(L, 1.0, 0.0, 0.1, "p1", 1.0, 1.0, 10);
    assert!(diags(&good).is_empty(), "{:?}", diags(&good));
    let neg = slab(L, -1.0, 0.0, 0.1, "p1", 1.0, 1.0, 10);
    assert!(diags(&neg).contains(&"radiation.absorption_coefficient".to_string()));
    let zero = slab(L, 0.0, 0.0, 0.1, "p1", 1.0, 1.0, 10);
    assert!(diags(&zero).contains(&"radiation.absorption_coefficient".to_string()));
    let noenergy = slab(L, 1.0, 0.0, 0.1, "p1", 1.0, 1.0, 10).replace("enabled = true", "enabled = false");
    assert!(diags(&noenergy).contains(&"radiation.model".to_string()));
    let eps = slab(L, 1.0, 0.0, 0.1, "p1", 1.5, 1.0, 10);
    assert!(diags(&eps).contains(&"boundaries.inlet.emissivity".to_string()));
    let alg = slab(L, 1.0, 0.0, 0.1, "p1", 1.0, 1.0, 10).replace("\"simple\"", "\"piso\"");
    assert!(diags(&alg).contains(&"radiation.model".to_string()));
    let _ = run;
}
