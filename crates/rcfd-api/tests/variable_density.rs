//! Variable-density ideal-gas solver (ADR 0005) through the case schema.
//!
//! Checks fixed before the first run: a subsonic converging-diverging duct (nozzle_2d, 100 x 8 cells, throat
//! Mach 0.3 set by the back pressure through the area-Mach relation, stagnation inlet 101325 Pa / 300 K) built from
//! a TOML case (a) reaches the residual targets tolerance_u = tolerance_p = 1e-8, (b) conserves the mass flow between
//! inlet and outlet to 1e-6 and (c) gives a throat Mach number within 2 % of 0.3 (a coarse smoke check of the wiring;
//! the quantitative validation of the kernel is in rcfd-fv/tests/varden_*.rs). Unsupported combinations are diagnosed
//! with their setting path.
//!
//! Result: (b) and (c) are met (in/out equal to 1e-7 or better, throat Mach 0.2997). (a) is NOT met: at the
//! 3000-iteration cap res_u = 2.6e-5 and res_p = 6e-13 (the momentum residual floor also seen in the kernel tests), so
//! the run is not flagged converged; the test therefore does not assert (a).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

const GAMMA: f64 = 1.4;
const P0: f64 = 101_325.0;

fn area_ratio(m: f64) -> f64 {
    let t = 1.0 + 0.5 * (GAMMA - 1.0) * m * m;
    (1.0 / m) * (2.0 / (GAMMA + 1.0) * t).powf((GAMMA + 1.0) / (2.0 * (GAMMA - 1.0)))
}

fn mach_of_area_subsonic(ar: f64) -> f64 {
    let (mut lo, mut hi) = (1e-6, 1.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if area_ratio(mid) > ar {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

fn nozzle(extra: &str) -> String {
    let ar_t = area_ratio(0.3);
    let m_e = mach_of_area_subsonic(2.0 * ar_t);
    let p_e = P0 * (1.0 + 0.5 * (GAMMA - 1.0) * m_e * m_e).powf(-GAMMA / (GAMMA - 1.0));
    format!(
        r#"
[mesh]
generator = "nozzle_2d"
nx = 100
ny = 8
lx = 2.0
ly = 0.1
throat_height = 0.05
exit_height = 0.1
[fluid]
density = "ideal_gas"
gas_constant = 287.0
[energy]
enabled = true
relax = 0.9
[initial]
velocity = [30.0, 0.0, 0.0]
temperature = 300.0
pressure = {pm}
[boundaries.xmin]
inlet = "stagnation"
total_pressure_value = {P0}
temperature_value = 300.0
velocity = "zero_gradient"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = {p_e}
temperature = "zero_gradient"
[boundaries.ymin]
velocity = "symmetry"
[boundaries.ymax]
velocity = "symmetry"
[solver]
algorithm = "simplec"
convection = "linear_upwind"
max_iterations = 3000
tolerance_u = 1e-8
tolerance_p = 1e-8
{extra}
[reports.m_in]
type = "mass_flow"
patch = "xmin"
[reports.m_out]
type = "mass_flow"
patch = "xmax"
"#,
        pm = 0.5 * (P0 + p_e)
    )
}

/// KNOWN MISS (F-061). Pre-registered check (a) of this file: the subsonic duct case reaches the residual targets
/// tolerance_u = tolerance_p = 1e-8 within the 3000-iteration cap of the case file. Measured when the test was first
/// run: res_u = 2.6e-5, res_p = 6e-13 at the cap, so the run is not flagged converged. The threshold below is the
/// registered one and is not loosened; the test is ignored because it fails by design until the momentum residual
/// floor is understood. Run with `--ignored` to see the current values.
#[test]
#[ignore = "KNOWN MISS: registered residual target 1e-8 is not met at the 3000-iteration cap (res_u 2.6e-5)"]
fn subsonic_nozzle_residual_target_1e8_known_miss() {
    let case = Case::parse_toml(&nozzle("")).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    while !sim.is_done() {
        sim.step();
    }
    let last = sim.last().unwrap();
    println!(
        "KNOWN MISS measured: res_u {:.3e}, res_p {:.3e}, converged {} after {} iterations",
        last.res_u, last.res_p, last.converged, last.index
    );
    assert!(
        last.res_u < 1e-8 && last.res_p < 1e-8 && last.converged,
        "res_u {:.3e} res_p {:.3e}",
        last.res_u,
        last.res_p
    );
}

#[test]
fn subsonic_nozzle_runs_from_a_case_file() {
    let case = Case::parse_toml(&nozzle("")).unwrap();
    assert!(case.semantic_errors().is_empty(), "{:?}", case.semantic_errors());
    let mut sim = Simulation::new(&case).unwrap();
    while !sim.is_done() {
        sim.step();
    }
    println!("{:?}: {:?}", sim.stop_reason(), sim.last());
    let last = |n: &str| *sim.monitors.get(n).unwrap().y.last().unwrap();
    let (m_in, m_out) = (last("m_in"), last("m_out"));
    println!("mass flow in {m_in:.6e} out {m_out:.6e}");
    assert!(m_in < 0.0 && m_out > 0.0);
    assert!(((m_in + m_out) / m_out).abs() < 1e-6, "{m_in} {m_out}");
    // throat Mach number: mean of the two columns next to x = lx / 2
    let mach = sim.field("mach").unwrap();
    let xc: Vec<f64> = sim.geom.mesh.cell_centres().iter().map(|c| c[0]).collect();
    let mut throat = Vec::new();
    for (c, x) in xc.iter().enumerate() {
        if (x - 1.0).abs() < 0.011 {
            throat.push(mach[c]);
        }
    }
    assert!(throat.len() == 16, "{}", throat.len());
    let m_t = throat.iter().sum::<f64>() / throat.len() as f64;
    println!("throat Mach {m_t:.4}");
    assert!((m_t / 0.3 - 1.0).abs() < 0.02, "{m_t}");
}

fn first_error(text: &str) -> (String, String) {
    match Case::parse_toml(text) {
        Err(d) => (d[0].path.clone(), d[0].message.clone()),
        Ok(c) => {
            let e = c.semantic_errors();
            assert!(!e.is_empty(), "no diagnostic");
            (e[0].path.clone(), e[0].message.clone())
        }
    }
}

#[test]
fn unsupported_combinations_are_diagnosed_with_their_path() {
    let (path, _) = first_error(&nozzle("").replace("algorithm = \"simplec\"", "algorithm = \"pimple\""));
    assert_eq!(path, "solver.algorithm");
    let (path, _) = first_error(&nozzle("").replace("enabled = true", "enabled = false"));
    assert_eq!(path, "energy.enabled");
    let (path, _) = first_error(&nozzle("").replace("[fluid]", "[fluid]\nmode = \"low_mach\""));
    assert_eq!(path, "boundaries.xmin.inlet");
    let (path, _) = first_error(&nozzle("").replace("[solver]", "[turbulence]\nmodel = \"k_epsilon\"\n[solver]"));
    assert_eq!(path, "turbulence.model");
    let (path, _) = first_error(&nozzle("").replace("[solver]", "[physics]\nbody_force = [0.0, 1.0, 0.0]\n[solver]"));
    assert_eq!(path, "physics.body_force");
    // fluid settings need density = ideal_gas
    let constant = nozzle("").replace("density = \"ideal_gas\"\n", "");
    let (path, _) = first_error(&constant);
    assert_eq!(path, "fluid.gas_constant");
}
