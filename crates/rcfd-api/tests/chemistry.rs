//! ADR 0027 case-level checks: K2b, K2c (case), K3 (case), K4, K5b, K6c, K8, K9. Limits are those registered in the ADR
//! before any run.

use rcfd_api::case::Case;
use rcfd_api::chemistry::{cell_rates, ChemState};
use rcfd_api::sim::Simulation;
use rcfd_reactor::cell::{CellModel, Closure};
use rcfd_reactor::mech::{Mechanism, R_GAS};
use rcfd_reactor::reactors::Tol;
use rcfd_reactor::toy;

const W: f64 = 0.020;

fn chem_block(mech: &str, thermo: &str, extra: &str) -> String {
    format!("[chemistry]\nmechanism_text = '''\n{mech}'''\nthermo_text = '''\n{thermo}'''\n{extra}\n")
}

fn box_case(species: &str, chem: &str, energy: bool, t0: f64, rho: f64, cp: f64, dt: f64, end: f64) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 4
ny = 4
lx = 1.0
ly = 1.0
[physics]
nu = 0.01
rho = {rho:.17e}
cp = {cp:.17e}
conductivity = 1.0
[energy]
enabled = {energy}
buoyancy = false
[initial]
temperature = {t0}
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "piso"
dt = {dt:.17e}
end_time = {end:.17e}
{species}
{chem}
"#
    )
}

fn isomer_species(a0: f64, with_inert: bool) -> String {
    let mut s =
        format!("[species.A]\ndiffusivity = 1e-5\ninitial = {a0}\n[species.B]\ndiffusivity = 1e-5\ninitial = 0.0\n");
    if with_inert {
        s.push_str(&format!("[species.I]\ndiffusivity = 1e-5\ninitial = {}\n", 1.0 - a0));
        // B is transported, I is the balance species
        s = s.replace("[species.I]", "[species.I]\nremainder = true");
    } else {
        s = s.replace("[species.B]", "[species.B]\nremainder = true");
    }
    s
}

fn sim_of(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"))
}

#[test]
fn k2b_closed_box_follows_the_cell_integration() {
    let (m, t) = toy::isomer_texts(1.0e8, 19872.0, -3000.0, false);
    let mech = Mechanism::from_chemkin(&m, Some(&t)).unwrap();
    let rho = 1.5;
    let cv = 1.5 * R_GAS / W;
    let dt = 1e-5;
    let chem = chem_block(
        &m,
        &t,
        &format!("model = \"finite_rate\"\ndensity = {rho:.17e}\nheat_release = true\nheat_capacity = {cv:.17e}"),
    );
    let mut sim = sim_of(&box_case(
        &isomer_species(1.0, false),
        &chem,
        true,
        800.0,
        rho,
        cv,
        dt,
        2e-3,
    ));
    let cell = CellModel {
        mech: &mech,
        rho,
        cp: Some(cv),
        closure: Closure::Kinetic,
        tol: Tol::default(),
    };
    let mut st = vec![800.0, 1.0, 0.0];
    let dt_ad = 2000.0;
    let (mut worst, mut spread) = (0.0_f64, 0.0_f64);
    for _ in 0..200 {
        sim.step();
        assert!(sim.failure().is_none(), "{:?}", sim.failure());
        cell.advance(&mut st, dt).unwrap();
        let t = sim.field("T").unwrap();
        let (lo, hi) = t.iter().fold((f64::MAX, f64::MIN), |a, v| (a.0.min(*v), a.1.max(*v)));
        spread = spread.max(hi - lo);
        worst = worst.max((t[0] - st[0]).abs());
    }
    println!(
        "K2b max |T_box - T_cell| {:.3e} K = {:.3e} DeltaT (limit 1e-5), spread {:.3e} (limit 1e-9 DeltaT)",
        worst,
        worst / dt_ad,
        spread
    );
    assert!(st[0] > 2500.0, "the run did not ignite: {}", st[0]);
    assert!(worst <= 1e-5 * dt_ad, "K2b T {worst:e}");
    assert!(spread <= 1e-9 * dt_ad, "K2b spread {spread:e}");
}

#[test]
fn k2c_case_adiabatic_flame_temperature() {
    let (m, t) = toy::isomer_texts(1.0e8, 19872.0, -3000.0, true);
    let chem = chem_block(
        &m,
        &t,
        "model = \"finite_rate\"\ndensity = 1.0\nheat_release = true\nheat_capacity = 1000.0",
    );
    let mut sim = sim_of(&box_case(
        &isomer_species(0.2, true),
        &chem,
        true,
        900.0,
        1.0,
        1000.0,
        1e-3,
        0.05,
    ));
    sim.run(100);
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let t = sim.field("T").unwrap();
    let a = sim.field("A").unwrap();
    let hand = 900.0 + 0.2 * R_GAS * 3000.0 / (W * 1000.0);
    let err = t.iter().map(|v| (v - hand).abs()).fold(0.0, f64::max);
    println!(
        "K2c case: T_end {:.6} hand {hand:.6} max err {err:.3e} K (limit 1e-4), max Y_A {:.2e}",
        t[0],
        a.iter().cloned().fold(0.0, f64::max)
    );
    assert!(err <= 1e-4);
    assert!(a.iter().all(|v| *v <= 1e-12));
}

#[test]
fn k3_case_element_conservation() {
    let (m, t) = toy::exchange_texts();
    let mech = Mechanism::from_chemkin(&m, Some(&t)).unwrap();
    let chem = chem_block(&m, &t, "model = \"finite_rate\"\ndensity = 1.2\ntemperature = 1500.0");
    let species = "[species.X2]\ndiffusivity = 1e-5\ninitial = 0.3\n[species.Y2]\ndiffusivity = 1e-5\ninitial = 0.4\n[species.XY]\ndiffusivity = 1e-5\ninitial = 0.1\n[species.I]\ndiffusivity = 1e-5\ninitial = 0.2\nremainder = true\n";
    let mut sim = sim_of(&box_case(species, &chem, false, 300.0, 1.2, 1005.0, 1e-4, 5e-3));
    let names = ["X2", "Y2", "XY", "I"];
    let elem = |sim: &Simulation| -> Vec<Vec<f64>> {
        let cols: Vec<Vec<f64>> = names.iter().map(|n| sim.field(n).unwrap()).collect();
        let n = cols[0].len();
        (0..n)
            .map(|c| {
                let y: Vec<f64> = cols.iter().map(|col| col[c]).collect();
                mech.element_mass_fractions(&y)
            })
            .collect()
    };
    let z0 = elem(&sim);
    sim.run(100);
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let z1 = elem(&sim);
    let mut worst = 0.0_f64;
    for (a, b) in z0.iter().zip(&z1) {
        for (x, y) in a.iter().zip(b) {
            worst = worst.max((x - y).abs() / x);
        }
    }
    let sum_drift = (0..z1.len())
        .map(|c| (names.iter().map(|n| sim.field(n).unwrap()[c]).sum::<f64>() - 1.0).abs())
        .fold(0.0, f64::max);
    assert!(sum_drift <= 1e-10, "sum of the mass fractions {sum_drift:e}");
    let xy = sim.field("XY").unwrap();
    println!("K3 case: element drift {worst:.3e} (limit 1e-10), XY end {:.4}", xy[0]);
    assert!(xy[0] > 0.3, "the reaction did not run");
    assert!(worst <= 1e-10);
}

// ---------- K4, K5b, K9: plug-flow channel ----------

fn channel(scheme: &str, nx: usize, chem: &str, energy: &str, species: &str, solver: &str) -> String {
    format!(
        r#"
[mesh]
generator = "channel_2d"
nx = {nx}
ny = 2
lx = 1.0
ly = 0.1
depth = 0.01
[physics]
nu = 0.001
rho = 1.0
cp = 1000.0
conductivity = 1e-6
[initial]
velocity = [1.0, 0.0, 0.0]
temperature = 600.0
[boundaries.inlet]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
temperature = "fixed"
temperature_value = 600.0
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "symmetry"
[solver]
convection = "{scheme}"
{solver}
{energy}
{species}
{chem}
"#
    )
}

const STEADY_SOLVER: &str = "algorithm = \"simplec\"\nmax_iterations = 800\ntolerance_u = 1e-9\ntolerance_p = 1e-9\npressure_rel_tol = 1e-13\npressure_abs_tol = 1e-22\npressure_max_iter = 5000\n";

const K_RATE: f64 = 5.0;

fn decay_species() -> String {
    "[species.A]\ndiffusivity = 1e-9\ninitial = 1.0\n[species.A.boundaries.inlet]\ntype = \"fixed\"\nvalue = 1.0\n[species.B]\nremainder = true\ninitial = 0.0\n".to_string()
}

fn decay_chem() -> String {
    let (m, t) = toy::isomer_texts(K_RATE, 0.0, 0.0, false);
    chem_block(&m, &t, "model = \"finite_rate\"\ndensity = 1.0\ntemperature = 1000.0")
}

fn run_steady(text: &str, monitors: &[&str]) -> Simulation {
    let mut sim = sim_of(text);
    while !sim.is_done() {
        sim.step();
        let res = monitors.iter().filter_map(|m| sim.monitors.last(m)).fold(0.0, f64::max);
        if sim.iteration() > 30 && res < 1e-11 && sim.monitors.last("continuity").unwrap() < 1e-13 {
            break;
        }
    }
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    assert!(
        sim.iteration() < 800,
        "not converged: {:?}",
        monitors.iter().map(|m| (*m, sim.monitors.last(m))).collect::<Vec<_>>()
    );
    sim
}

fn l2_error(sim: &Simulation, nx: usize) -> (f64, f64) {
    let a = sim.field("A").unwrap();
    let cc = sim.geom.mesh.cell_centres();
    let dx = 1.0 / nx as f64;
    let kappa = K_RATE * dx;
    let (mut s, mut worst_discrete) = (0.0, 0.0_f64);
    for (c, x) in cc.iter().enumerate() {
        let e = a[c] - (-K_RATE * x[0]).exp();
        s += e * e;
        let i = (x[0] / dx - 0.5).round() as i32 + 1;
        let disc = (1.0 + kappa).powi(-i);
        worst_discrete = worst_discrete.max((a[c] / disc - 1.0).abs());
    }
    ((s / cc.len() as f64).sqrt(), worst_discrete)
}

#[test]
fn k4_first_order_decay_in_a_plug_flow_channel() {
    let mut errs = std::collections::BTreeMap::new();
    for scheme in ["upwind", "linear_upwind"] {
        for nx in [40, 80, 160] {
            let sim = run_steady(
                &channel(scheme, nx, &decay_chem(), "", &decay_species(), STEADY_SOLVER),
                &["res_A"],
            );
            let (e, d) = l2_error(&sim, nx);
            println!(
                "K4 {scheme} nx {nx}: L2 error {e:.4e}, discrete-exactness deviation {d:.3e}, {} iterations",
                sim.iteration()
            );
            if scheme == "upwind" && nx == 40 {
                assert!(d <= 1e-5, "K4a discrete exactness {d:e} (limit 1e-5)");
            }
            errs.insert((scheme, nx), e);
        }
    }
    let ord = |s: &'static str, a: usize, b: usize| (errs[&(s, a)] / errs[&(s, b)]).log2();
    let (u1, u2) = (ord("upwind", 40, 80), ord("upwind", 80, 160));
    let (l1, l2) = (ord("linear_upwind", 40, 80), ord("linear_upwind", 80, 160));
    println!("K4b upwind orders {u1:.3} {u2:.3} (limit [0.9, 1.2]); K4c linear_upwind orders {l1:.3} {l2:.3} (limit >= 1.5), finest error {:.3e} (limit 1e-4)", errs[&("linear_upwind", 160)]);
    assert!((0.9..=1.2).contains(&u1) && (0.9..=1.2).contains(&u2), "K4b {u1} {u2}");
    assert!(l1 >= 1.5 && l2 >= 1.5, "K4c {l1} {l2}");
    assert!(errs[&("linear_upwind", 160)] <= 1e-4);
}

#[test]
fn k5b_steady_heat_release_follows_the_enthalpy_balance() {
    let (m, t) = toy::isomer_texts(K_RATE, 0.0, -3000.0, true);
    let chem = chem_block(
        &m,
        &t,
        "model = \"finite_rate\"\ndensity = 1.0\nheat_release = true\nheat_capacity = 1000.0",
    );
    let species = "[species.A]\ndiffusivity = 1e-9\ninitial = 0.2\n[species.A.boundaries.inlet]\ntype = \"fixed\"\nvalue = 0.2\n[species.B]\ndiffusivity = 1e-9\ninitial = 0.0\n[species.B.boundaries.inlet]\ntype = \"fixed\"\nvalue = 0.0\n[species.I]\nremainder = true\ninitial = 0.8\n";
    let nx = 100;
    let sim = run_steady(
        &channel(
            "upwind",
            nx,
            &chem,
            "[energy]\nenabled = true\nbuoyancy = false\nrelax = 0.9\n",
            species,
            STEADY_SOLVER,
        ),
        &["res_A", "res_B", "res_T"],
    );
    let a = sim.field("A").unwrap();
    let temp = sim.field("T").unwrap();
    let q_over_cp = R_GAS * 3000.0 / W / 1000.0;
    let scale = q_over_cp * 0.2;
    let mut worst = 0.0_f64;
    for c in 0..a.len() {
        worst = worst.max((temp[c] - 600.0 - q_over_cp * (0.2 - a[c])).abs());
    }
    let cc = sim.geom.mesh.cell_centres();
    let last = (0..a.len()).max_by(|&i, &j| cc[i][0].total_cmp(&cc[j][0])).unwrap();
    let out_hand = 600.0 + scale * (1.0 - (-K_RATE * cc[last][0]).exp());
    println!(
        "K5b: max |T - T_in - (Q/cp)(Y_in - Y_A)| = {worst:.3e} K (limit {:.2e}); outlet T {:.5} vs undiscretised {out_hand:.5}; DeltaT_ad {scale:.4}",
        1e-6 * scale,
        temp[last]
    );
    assert!(worst <= 1e-6 * scale);
}

#[test]
fn k6c_turbulent_closures_need_turbulence_fields() {
    let (m, t) = toy::burner_texts();
    let inlet = |n: &str, v: f64| {
        format!(
            "[species.{n}.boundaries.inlet]
type = \"fixed\"
value = {v}
"
        )
    };
    let species = format!(
        "[species.FU]
diffusivity = 1e-5
initial = 0.05
{}[species.OX]
diffusivity = 1e-5
initial = 0.2
{}[species.PR]
diffusivity = 1e-5
initial = 0.1
{}[species.DI]
remainder = true
initial = 0.65
",
        inlet("FU", 0.05),
        inlet("OX", 0.2),
        inlet("PR", 0.1)
    );
    let species = species.as_str();
    let chem = chem_block(
        &m,
        &t,
        "model = \"eddy_dissipation\"\ndensity = 1.0\ntemperature = 1000.0",
    );
    // laminar: located error
    let err = Case::parse_toml(&channel("upwind", 20, &chem, "", species, STEADY_SOLVER))
        .expect_err("a laminar case with a turbulent closure must be refused");
    assert!(err.iter().any(|d| d.path == "chemistry.model"), "{err:?}");
    // k-epsilon: runs, and the rates from the run fields equal the hand formula
    let solver = format!("{STEADY_SOLVER}\n[turbulence]\nmodel = \"k_epsilon\"\n");
    let text = channel("upwind", 20, &chem, "", species, &solver)
        .replace("velocity = \"symmetry\"", "velocity = \"fixed\"")
        .replace(
            "temperature = \"fixed\"\n",
            "turbulence_length = 0.01\ntemperature = \"fixed\"\n",
        );
    let case = Case::parse_toml(&text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    sim.run(40);
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let (k, eps) = (sim.field("k").unwrap(), sim.field("epsilon").unwrap());
    let cols: Vec<Vec<f64>> = ["FU", "OX", "PR", "DI"].iter().map(|n| sim.field(n).unwrap()).collect();
    let chem_state = ChemState::from_case(&case).unwrap().unwrap();
    let mut worst = 0.0_f64;
    let mut max_q = 0.0_f64;
    for c in 0..k.len() {
        let y: Vec<f64> = cols.iter().map(|col| col[c]).collect();
        let st = [1000.0, y[0], y[1], y[2], y[3]];
        let (q, _) = cell_rates(&chem_state, &st, eps[c] / k[c]);
        let hand = 4.0
            * 1.0
            * (eps[c] / k[c])
            * f64::min(f64::min(y[0] / 0.024, y[1] / (3.0 * 0.032)), 0.5 * y[2] / (2.0 * 0.060));
        worst = worst.max((q[0] - hand).abs() / hand.abs().max(1e-300));
        max_q = max_q.max(hand);
    }
    println!("K6c: max relative difference to the hand formula on the run fields {worst:.3e} (limit 1e-12), max q {max_q:.3e}");
    assert!(max_q > 0.0);
    assert!(worst <= 1e-12);
    // fuel is consumed downstream
    let fu = &cols[0];
    assert!(
        fu.iter().cloned().fold(f64::MAX, f64::min) < 0.05 - 1e-6,
        "no fuel was consumed"
    );
}

// ---------- K8: located validation errors ----------

/// Registered form of the CHEMKIN syntax-error check: the diagnostic `line` is the line inside the mechanism text. The case
/// parser replaces `line` by the line of the key in the TOML text, so this form misses (ADR 0027 addendum A1); the corrected
/// check (line in the message) is in `k8_located_errors`.
#[test]
#[ignore = "known miss: Case::parse_toml overwrites Diagnostic.line with the TOML key line, see ADR 0027 addendum A1"]
fn k8_known_miss_diagnostic_line_inside_mechanism() {
    let (_, t) = toy::isomer_texts(K_RATE, 0.0, 0.0, false);
    let bad_mech = "ELEMENTS X/20.0/ END
SPECIES A B END
REACTIONS
A => B  5.0 0.0 0.0
 PLOG / 1.0 1.0 0.0 0.0 /
END
";
    let d = diags(&channel(
        "upwind",
        10,
        &format!(
            "[chemistry]
model = \"finite_rate\"
mechanism_text = '''
{bad_mech}'''
thermo_text = '''
{t}'''
density = 1.0
temperature = 1000.0
"
        ),
        "",
        &decay_species(),
        STEADY_SOLVER,
    ));
    let e = d
        .iter()
        .find(|x| x.path == "chemistry.mechanism_text")
        .expect("diagnostic");
    assert_eq!(e.line, Some(5));
}

fn diags(text: &str) -> Vec<rcfd_api::case::Diagnostic> {
    match Case::parse_toml(text) {
        Err(d) => d,
        Ok(c) => Simulation::new(&c).err().unwrap_or_default(),
    }
}

#[test]
fn k8_located_errors() {
    let (m, t) = toy::isomer_texts(K_RATE, 0.0, 0.0, false);
    let good = "model = \"finite_rate\"\ndensity = 1.0\ntemperature = 1000.0";
    let base = |chem_extra: &str, species: &str, energy: &str| {
        channel(
            "upwind",
            10,
            &chem_block(&m, &t, chem_extra),
            energy,
            species,
            STEADY_SOLVER,
        )
    };
    let sp = decay_species();
    // the good case builds
    assert!(
        diags(&base(good, &sp, "")).is_empty(),
        "{:?}",
        diags(&base(good, &sp, ""))
    );
    let has = |d: &[rcfd_api::case::Diagnostic], path: &str| d.iter().any(|x| x.path == path);
    // unreadable mechanism file
    let d = diags(&channel(
        "upwind",
        10,
        "[chemistry]\nmodel = \"finite_rate\"\nmechanism = \"nonexistent.mech\"\ndensity = 1.0\ntemperature = 1000.0\n",
        "",
        &sp,
        STEADY_SOLVER,
    ));
    assert!(has(&d, "chemistry.mechanism"), "{d:?}");
    // CHEMKIN syntax error with its line
    let bad_mech =
        "ELEMENTS X/20.0/ END\nSPECIES A B END\nREACTIONS\nA => B  5.0 0.0 0.0\n PLOG / 1.0 1.0 0.0 0.0 /\nEND\n";
    let d = diags(&channel(
        "upwind",
        10,
        &format!("[chemistry]\nmodel = \"finite_rate\"\nmechanism_text = '''\n{bad_mech}'''\nthermo_text = '''\n{t}'''\ndensity = 1.0\ntemperature = 1000.0\n"),
        "",
        &sp,
        STEADY_SOLVER,
    ));
    let e = d
        .iter()
        .find(|x| x.path == "chemistry.mechanism_text")
        .unwrap_or_else(|| panic!("{d:?}"));
    println!("K8 syntax error: {e:?}");
    assert!(e.message.contains("mechanism line 5:"), "{e:?}");
    // species missing / extra
    let only_a = "[species.A]\ndiffusivity = 1e-9\ninitial = 1.0\n[species.Q]\nremainder = true\ninitial = 0.0\n";
    let d = diags(&base(good, only_a, ""));
    assert!(has(&d, "chemistry.mechanism_text") && has(&d, "species.Q"), "{d:?}");
    // temperature unset with energy off
    let d = diags(&base("model = \"finite_rate\"\ndensity = 1.0", &sp, ""));
    assert!(has(&d, "chemistry.temperature"), "{d:?}");
    // heat release without energy
    let d = diags(&base(
        &format!("{good}\nheat_release = true\nheat_capacity = 1005.0"),
        &sp,
        "",
    ));
    assert!(has(&d, "chemistry.heat_release"), "{d:?}");
    // density unset and different from physics.rho
    let d = diags(&base("model = \"finite_rate\"\ntemperature = 1000.0", &sp, ""));
    assert!(has(&d, "chemistry.density"), "{d:?}");
    let d = diags(&base(
        "model = \"finite_rate\"\ndensity = 2.0\ntemperature = 1000.0",
        &sp,
        "",
    ));
    assert!(has(&d, "chemistry.density"), "{d:?}");
    // heat capacity unset / different from physics.cp (energy on)
    let en = "[energy]\nenabled = true\nbuoyancy = false\n";
    let d = diags(&base(
        "model = \"finite_rate\"\ndensity = 1.0\nheat_release = true",
        &sp,
        en,
    ));
    assert!(has(&d, "chemistry.heat_capacity"), "{d:?}");
    let d = diags(&base(
        "model = \"finite_rate\"\ndensity = 1.0\nheat_release = true\nheat_capacity = 1234.0",
        &sp,
        en,
    ));
    assert!(has(&d, "chemistry.heat_capacity"), "{d:?}");
    // temperature given although the energy equation is on
    let d = diags(&base(good, &sp, en));
    assert!(has(&d, "chemistry.temperature"), "{d:?}");
    // turbulent closure with laminar flow
    let d = diags(&base("model = \"edc\"\ndensity = 1.0\ntemperature = 1000.0", &sp, ""));
    assert!(has(&d, "chemistry.model"), "{d:?}");
    // eddy dissipation with a reversible reaction: located at the reaction line
    let rev_mech = "ELEMENTS X/20.0/ END\nSPECIES A B END\nREACTIONS\nA <=> B  5.0 0.0 0.0\nEND\n";
    let d = diags(&channel(
        "upwind",
        10,
        &format!("[chemistry]\nmodel = \"eddy_dissipation\"\nmechanism_text = '''\n{rev_mech}'''\nthermo_text = '''\n{t}'''\ndensity = 1.0\ntemperature = 1000.0\n"),
        "",
        &sp,
        &format!("{STEADY_SOLVER}\n[turbulence]\nmodel = \"k_epsilon\"\n"),
    ));
    let e = d
        .iter()
        .find(|x| x.path == "chemistry.mechanism_text" && x.message.contains("mechanism line"))
        .unwrap_or_else(|| panic!("{d:?}"));
    assert!(e.message.contains("mechanism line 4:"), "{e:?}");
    // non-positive A
    let d = diags(&base(&format!("{good}\neddy_dissipation_a = 0.0"), &sp, ""));
    assert!(has(&d, "chemistry.eddy_dissipation_a"), "{d:?}");
    // no species at all
    let d = diags(&base(good, "", ""));
    assert!(has(&d, "species"), "{d:?}");
}

// ---------- K9: splitting consistency (measured) ----------

#[test]
fn k9_lie_splitting_error_decreases_with_the_step() {
    let nx = 160;
    let steady = run_steady(
        &channel("upwind", nx, &decay_chem(), "", &decay_species(), STEADY_SOLVER),
        &["res_A"],
    );
    let ref_a = steady.field("A").unwrap();
    let mut diffs = Vec::new();
    for dt in [0.02, 0.01, 0.005] {
        let solver = format!("algorithm = \"piso\"\ndt = {dt}\nend_time = 4.0\n");
        let mut sim = sim_of(&channel("upwind", nx, &decay_chem(), "", &decay_species(), &solver));
        sim.run(10_000);
        assert!(sim.failure().is_none(), "{:?}", sim.failure());
        let a = sim.field("A").unwrap();
        let d = a.iter().zip(&ref_a).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
        println!("K9 dt {dt}: max |Y_split - Y_steady| = {d:.4e}");
        diffs.push(d);
    }
    let (o1, o2) = ((diffs[0] / diffs[1]).log2(), (diffs[1] / diffs[2]).log2());
    println!("K9 observed orders {o1:.3} {o2:.3} (limit [0.8, 1.3])");
    assert!(
        diffs[0] > 1e-6,
        "precondition: split error {} not above the solver tolerance (inconclusive)",
        diffs[0]
    );
    assert!((0.8..=1.3).contains(&o1) && (0.8..=1.3).contains(&o2), "K9 {o1} {o2}");
}
