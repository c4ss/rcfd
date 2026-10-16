//! Lorentz-force MHD through the case tree (F-214, ADR 0039 checks A1 to A5). The limits are the ones registered in
//! `docs/adr/0039-wave23a-mhd-heat-exchanger.md` before any run.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::lorentz::hartmann;

const SIGMA: f64 = 1.0;
const A: f64 = 0.5;
const G: f64 = 400.0;

/// Hartmann channel: periodic in x, no-slip plates at y = 0 and y = 1 (half width a = 0.5), fixed body force `g`.
fn channel(ny: usize, g: f64, source: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 4
ny = {ny}
lx = 1.0
ly = 1.0
periodic_x = true
[physics]
nu = 1.0
rho = 1.0
body_force = [{g}, 0.0, 0.0]
[initial]
velocity = [0.0, 0.0, 0.0]
[boundaries.ymin]
velocity = "fixed"
velocity_value = [0.0, 0.0, 0.0]
[boundaries.ymax]
velocity = "fixed"
velocity_value = [0.0, 0.0, 0.0]
[solver]
algorithm = "simplec"
convection = "upwind"
h_operator = "from_solution"
momentum_rel_tol = 1e-10
pressure_rel_tol = 1e-10
pressure_max_iter = 1000
max_iterations = 100000
tolerance_u = 1e-14
tolerance_p = 1e-14
{source}
"#
    )
}

fn lorentz(b: [f64; 3], extra: &str) -> String {
    format!(
        "[sources.mhd]\ntype = \"lorentz\"\nzone = \"all\"\nmagnetic_field = [{}, {}, {}]\nconductivity = {SIGMA}\n{extra}",
        b[0], b[1], b[2]
    )
}

/// Iterate until the largest change of ux over 200 iterations is below 1e-9 of max ux (cap 30000 iterations).
fn run(text: &str) -> (Simulation, usize, f64) {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let mut n = 0;
    loop {
        let before = sim.field("ux").unwrap();
        for _ in 0..200 {
            sim.step();
        }
        n += 200;
        assert!(sim.failure().is_none(), "{:?}", sim.failure());
        let after = sim.field("ux").unwrap();
        let scale = after.iter().fold(1e-300_f64, |m, v| m.max(v.abs()));
        let change = after
            .iter()
            .zip(&before)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max)
            / scale;
        if change < 1e-9 || n >= 30000 {
            return (sim, n, change);
        }
    }
}

/// (profile error vs the exact profile, mean velocity, cell-volume mean)
fn compare(sim: &Simulation, exact: impl Fn(f64) -> f64) -> (f64, f64) {
    let ux = sim.field("ux").unwrap();
    let cc = sim.geom.mesh.cell_centres();
    let vol = sim.geom.mesh.cell_volumes();
    let umax = (0..ux.len()).map(|c| exact(cc[c][1] - A)).fold(0.0_f64, f64::max);
    let err = (0..ux.len())
        .map(|c| (ux[c] - exact(cc[c][1] - A)).abs())
        .fold(0.0, f64::max)
        / umax;
    let v: f64 = vol.iter().sum();
    let mean = ux.iter().zip(vol).map(|(u, v)| u * v).sum::<f64>() / v;
    (err, mean)
}

fn hartmann_short(ha: f64, ny: usize) -> (Simulation, f64, f64, f64) {
    let b0 = 2.0 * ha;
    let (sim, n, change) = run(&channel(ny, G, &lorentz([0.0, b0, 0.0], "")));
    assert!(
        change < 1e-9,
        "not stationary after {n} iterations: change {change:.2e}"
    );
    let (err, mean) = compare(&sim, |y| hartmann::u_short(G, SIGMA, b0, ha, A, y));
    let exact = hartmann::mean_short(G, SIGMA, b0, ha);
    let flow_err = (mean / exact - 1.0).abs();
    let poiseuille = G * A * A / 3.0;
    eprintln!(
        "A1 Ha {ha} ny {ny}: {n} iterations, profile error {err:.3e} (limit 2e-2), flow error {flow_err:.3e} (limit 1e-2), U/U_P {:.5} vs 3(Ha-tanh Ha)/Ha^3 {:.5}",
        mean / poiseuille,
        hartmann::flow_ratio(ha)
    );
    (sim, err, flow_err, b0)
}

#[test]
fn a1_hartmann_short_circuit_profile_and_flow_rate() {
    for (ha, ny) in [(2.0, 40), (10.0, 80), (20.0, 160)] {
        let (_, err, flow_err, _) = hartmann_short(ha, ny);
        assert!(err <= 2e-2, "Ha {ha}: profile error {err}");
        assert!(flow_err <= 1e-2, "Ha {ha}: flow-rate error {flow_err}");
    }
}

#[test]
fn a2_hartmann_insulating_walls_through_the_imposed_field() {
    let (ha, ny) = (10.0, 80);
    let b0 = 2.0 * ha;
    let u_mean = 1.0;
    let g = hartmann::g_insulated(u_mean, SIGMA, b0, ha);
    let extra = format!("applied_electric_field = [0.0, 0.0, {}]\n", -u_mean * b0);
    let (sim, n, change) = run(&channel(ny, g, &lorentz([0.0, b0, 0.0], &extra)));
    assert!(
        change < 1e-9,
        "not stationary after {n} iterations: change {change:.2e}"
    );
    let (err, mean) = compare(&sim, |y| hartmann::u_insulated(u_mean, ha, A, y));
    let flow_err = (mean / u_mean - 1.0).abs();
    eprintln!("A2 Ha {ha}: {n} iterations, G {g:.4}, profile error {err:.3e} (limit 2e-2), flow error {flow_err:.3e} (limit 1e-2)");
    assert!(err <= 2e-2, "profile error {err}");
    assert!(flow_err <= 1e-2, "flow-rate error {flow_err}");
}

#[test]
fn a3_lorentz_integral_and_force_balance() {
    let (ha, ny) = (10.0, 80);
    let (sim, _, _, b0) = hartmann_short(ha, ny);
    let ux = sim.field("ux").unwrap();
    let fx = sim.field("lorentz_force_x").unwrap();
    let vol = sim.geom.mesh.cell_volumes();
    let cc = sim.geom.mesh.cell_centres();
    let sum_f: f64 = fx.iter().zip(vol).map(|(f, v)| f * v).sum();
    let sum_u: f64 = ux.iter().zip(vol).map(|(u, v)| u * v).sum();
    let want = -SIGMA * b0 * b0 * sum_u;
    let e_int = (sum_f / want - 1.0).abs();
    let work: f64 = fx.iter().zip(&ux).zip(vol).map(|((f, u), v)| f * u * v).sum();
    let work_want: f64 = -SIGMA * b0 * b0 * ux.iter().zip(vol).map(|(u, v)| u * u * v).sum::<f64>();
    let e_work = (work / work_want - 1.0).abs();
    // wall shear of the cells next to the plates: mu u_c / (dy/2) times the face area (mu = rho nu = 1)
    let dy = 1.0 / ny as f64;
    let sf = sim.geom.mesh.face_area_vectors();
    let mut shear = 0.0;
    for p in ["ymin", "ymax"] {
        for f in sim.geom.mesh.patch(p).unwrap().faces() {
            let c = sim.geom.mesh.owner()[f];
            shear += ux[c] / (0.5 * dy) * (sf[f][0] * sf[f][0] + sf[f][1] * sf[f][1] + sf[f][2] * sf[f][2]).sqrt();
        }
    }
    let vtot: f64 = vol.iter().sum();
    let balance = (G * vtot / (shear + (-sum_f)) - 1.0).abs();
    let _ = cc;
    eprintln!(
        "A3: Lorentz integral vs -sigma B^2 sum u V {e_int:.2e} (limit 1e-9), work identity {e_work:.2e} (1e-9), sum F u V {work:.4e} < 0, momentum balance {balance:.2e} (limit 1e-6)"
    );
    assert!(e_int <= 1e-9);
    assert!(work < 0.0 && e_work <= 1e-9);
    assert!(balance <= 1e-6, "balance {balance}");
}

#[test]
fn a5_solved_potential_cancels_the_motional_current() {
    let (ha, ny) = (10.0, 80);
    let b0 = 2.0 * ha;
    // plain Poiseuille reference without the source
    let (plain, _, c0) = run(&channel(ny, G, ""));
    assert!(c0 < 1e-9);
    let u_plain = plain.field("ux").unwrap();
    let umax = u_plain.iter().fold(0.0_f64, |m, v| m.max(*v));
    // B normal to the plane of the flow, potential solved: u x B = -u B e_y is balanced by grad phi, J = 0
    let (sim, n, change) = run(&channel(ny, G, &lorentz([0.0, 0.0, b0], "potential = \"solve\"\n")));
    assert!(change < 1e-9, "not stationary after {n} iterations");
    let ux = sim.field("ux").unwrap();
    let du = ux.iter().zip(&u_plain).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max) / umax;
    let jmax = ["current_density_x", "current_density_y", "current_density_z"]
        .iter()
        .flat_map(|f| sim.field(f).unwrap())
        .fold(0.0_f64, |m, v| m.max(v.abs()))
        / (SIGMA * b0 * umax);
    eprintln!("A5 solved potential: max |J|/(sigma B u_max) {jmax:.2e} (limit 1e-6), max|u - u_plain|/u_max {du:.2e} (limit 1e-6)");
    assert!(jmax <= 1e-6, "{jmax}");
    assert!(du <= 1e-6, "{du}");
    // without the potential solve the same u x B brakes the flow like the Hartmann short circuit
    let (braked, n2, c2) = run(&channel(ny, G, &lorentz([0.0, 0.0, b0], "")));
    assert!(c2 < 1e-9, "not stationary after {n2} iterations");
    let (_, mean) = compare(&braked, |_| 0.0);
    let ratio = mean / (G * A * A / 3.0);
    let want = hartmann::flow_ratio(ha);
    eprintln!(
        "A5 potential = none: U/U_P {ratio:.5} vs {want:.5} (rel {:.2e}, limit 1e-2)",
        (ratio / want - 1.0).abs()
    );
    assert!((ratio / want - 1.0).abs() <= 1e-2);
}

fn diags(text: &str) -> Vec<String> {
    match Case::parse_toml(text) {
        Err(d) => d.into_iter().map(|d| d.path).collect(),
        Ok(c) => c.semantic_errors().into_iter().map(|d| d.path).collect(),
    }
}

#[test]
fn a4_invalid_settings_get_located_errors() {
    let ok = lorentz([0.0, 5.0, 0.0], "");
    assert!(
        diags(&channel(10, 1.0, &ok)).is_empty(),
        "{:?}",
        diags(&channel(10, 1.0, &ok))
    );
    // missing field
    let no_b = "[sources.mhd]\ntype = \"lorentz\"\nzone = \"all\"\nconductivity = 1.0\n";
    assert!(diags(&channel(10, 1.0, no_b)).contains(&"sources.mhd.magnetic_field".to_string()));
    let zero_b = lorentz([0.0; 3], "");
    assert!(diags(&channel(10, 1.0, &zero_b)).contains(&"sources.mhd.magnetic_field".to_string()));
    let bad_len =
        "[sources.mhd]\ntype = \"lorentz\"\nzone = \"all\"\nmagnetic_field = [0.0, 1.0]\nconductivity = 1.0\n";
    assert!(diags(&channel(10, 1.0, bad_len)).contains(&"sources.mhd.magnetic_field".to_string()));
    // conductivity unset / negative
    let no_sigma = "[sources.mhd]\ntype = \"lorentz\"\nzone = \"all\"\nmagnetic_field = [0.0, 1.0, 0.0]\n";
    assert!(diags(&channel(10, 1.0, no_sigma)).contains(&"sources.mhd.conductivity".to_string()));
    // wrong electric field length
    let bad_e = lorentz([0.0, 1.0, 0.0], "applied_electric_field = [1.0]\n");
    assert!(diags(&channel(10, 1.0, &bad_e)).contains(&"sources.mhd.applied_electric_field".to_string()));
    // unsupported algorithms
    // (ADR 0045: piso and pimple now support the Lorentz source, see tests/transient_sources.rs)
    let compressible = channel(10, 1.0, &ok).replace("simplec", "compressible");
    assert!(
        diags(&compressible).contains(&"sources.mhd.type".to_string()),
        "compressible: {:?}",
        diags(&compressible)
    );
    let coupled = channel(10, 1.0, &ok).replace("simplec", "coupled");
    assert!(
        diags(&coupled).contains(&"sources.mhd.type".to_string()),
        "{:?}",
        diags(&coupled)
    );
    // electric model together with a Lorentz source
    let el = format!("[electric]\nenabled = true\nconductivity = 1.0\n{ok}");
    assert!(diags(&channel(10, 1.0, &el)).contains(&"sources.mhd.type".to_string()));
    // boundary electric keys are read with potential = solve (a missing potential value is located) and refused without
    let solve = lorentz(
        [0.0, 0.0, 5.0],
        "potential = \"solve\"
",
    );
    let key = |t: String| {
        t.replace(
            "[boundaries.ymax]
",
            "[boundaries.ymax]
electric = \"potential\"
",
        )
    };
    let d = diags(&key(channel(10, 1.0, &solve)));
    assert!(d.contains(&"boundaries.ymax.electric_potential".to_string()), "{d:?}");
    assert!(!d.contains(&"boundaries.ymax.electric".to_string()), "{d:?}");
    let none = diags(&key(channel(10, 1.0, &ok)));
    assert!(none.contains(&"boundaries.ymax.electric".to_string()), "{none:?}");
}
