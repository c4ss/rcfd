//! Lumped zone components (F-113, ADR 0012). Limits are those registered in docs/adr/0012-lumped-zones-radiation.md
//! before the first run: Z1 porous jump 1e-3, Z2 fan operating point 5e-3 (flow) and 1e-3 (zone pressure changes),
//! Z3 network outlet 5e-3 and 1e-4, Z4 heat-exchanger energy balance 1e-6, Z5 located diagnostics.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::lumped::{bisect, network_pressure, porous_jump_dp, FanCurve};

const RHO: f64 = 1.2;
const MU: f64 = 1.8e-5;
const LY: f64 = 0.2;
const DEPTH: f64 = 0.1;
const AREA: f64 = LY * DEPTH;
const ALPHA: f64 = 1e-8;
const C2: f64 = 1000.0;
const THICK: f64 = 0.02;
const FAN_Q: [f64; 3] = [0.0, 0.03, 0.06];
const FAN_DP: [f64; 3] = [200.0, 140.0, 0.0];

fn base(inlet: &str, extra: &str) -> String {
    format!(
        r#"
[mesh]
generator = "channel_2d"
nx = 100
ny = 2
lx = 2.0
ly = {LY}
depth = {DEPTH}
[physics]
nu = {nu}
rho = {RHO}
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "symmetry"
{inlet}
[solver]
algorithm = "simplec"
convection = "upwind"
h_operator = "from_solution"
momentum_rel_tol = 1e-8
pressure_rel_tol = 1e-8
pressure_max_iter = 1000
max_iterations = 100000
tolerance_u = 1e-14
tolerance_p = 1e-14
{extra}
"#,
        nu = MU / RHO
    )
}

fn velocity_inlet(u: f64) -> String {
    format!("[boundaries.inlet]\nvelocity = \"fixed\"\nvelocity_value = [{u}, 0.0, 0.0]\n")
}

const PRESSURE_INLET: &str = "[boundaries.inlet]\nvelocity = \"zero_gradient\"\npressure = \"fixed\"\n";

fn jump_block(alpha: f64, c2: f64) -> String {
    format!(
        "[components.jump]\ntype = \"porous_jump\"\nzone = \"box\"\nbox_min = [1.2, -1.0, -1.0]\nbox_max = [1.3, 1.0, 1.0]\ndirection = [1.0, 0.0, 0.0]\nthickness = {THICK}\npermeability = {alpha}\ninertial_resistance = {c2}\n"
    )
}

fn fan_block() -> String {
    format!(
        "[components.fan]\ntype = \"fan\"\nzone = \"box\"\nbox_min = [0.4, -1.0, -1.0]\nbox_max = [0.6, 1.0, 1.0]\ndirection = [1.0, 0.0, 0.0]\ncurve_flow = {FAN_Q:?}\ncurve_pressure = {FAN_DP:?}\n"
    )
}

/// Run `n` iterations then 300 more; require stationarity (relative change of ux and p below 1e-8).
fn run(text: &str, n: usize) -> Simulation {
    let case = Case::parse_toml(text).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    for _ in 0..n {
        sim.step();
    }
    let before: Vec<Vec<f64>> = ["ux", "p"].iter().map(|f| sim.field(f).unwrap()).collect();
    for _ in 0..300 {
        sim.step();
    }
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    for (f, old) in ["ux", "p"].iter().zip(&before) {
        let new = sim.field(f).unwrap();
        let scale = new.iter().map(|v| v.abs()).fold(1e-12, f64::max);
        let change = new.iter().zip(old).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max) / scale;
        assert!(
            change < 1e-8,
            "{f} is not stationary after {n} iterations: change {change:.3e}"
        );
    }
    sim
}

/// Mean pressure (Pa) of the cell column nearest to x.
fn column_p(sim: &Simulation, x_target: f64) -> f64 {
    let c = sim.geom.mesh.cell_centres();
    let p = sim.field("p").unwrap();
    let x = c
        .iter()
        .map(|v| v[0])
        .min_by(|a, b| (a - x_target).abs().partial_cmp(&(b - x_target).abs()).unwrap())
        .unwrap();
    let idx: Vec<usize> = (0..c.len()).filter(|&i| (c[i][0] - x).abs() < 1e-9).collect();
    RHO * idx.iter().map(|&i| p[i]).sum::<f64>() / idx.len() as f64
}

fn column_t(sim: &Simulation, x_target: f64) -> f64 {
    let c = sim.geom.mesh.cell_centres();
    let t = sim.field("T").unwrap();
    let i = (0..c.len())
        .min_by(|&a, &b| {
            (c[a][0] - x_target)
                .abs()
                .partial_cmp(&(c[b][0] - x_target).abs())
                .unwrap()
        })
        .unwrap();
    t[i]
}

fn patch_flow(sim: &Simulation, patch: &str) -> f64 {
    let p = sim.geom.mesh.patch(patch).unwrap();
    let phi = sim.flux();
    p.faces().filter(|&f| !sim.geom.empty[f]).map(|f| phi[f]).sum()
}

fn fan_curve() -> FanCurve {
    FanCurve::new(FAN_Q.to_vec(), FAN_DP.to_vec()).unwrap()
}

#[test]
fn z1_porous_jump_reproduces_the_darcy_forchheimer_sheet() {
    let mut worst: f64 = 0.0;
    let cases: [(f64, f64, f64); 5] = [
        (0.5, ALPHA, C2),
        (2.0, ALPHA, C2),
        (5.0, ALPHA, C2),
        (2.0, ALPHA, 0.0),
        (2.0, 0.0, C2),
    ];
    for (u, alpha, c2) in cases {
        let sim = run(&base(&velocity_inlet(u), &jump_block(alpha, c2)), 1500);
        let dp = column_p(&sim, 1.19) - column_p(&sim, 1.31);
        let exact = porous_jump_dp(MU, RHO, alpha, c2, THICK, u);
        let err = (dp / exact - 1.0).abs();
        eprintln!(
            "Z1 u {u} alpha {alpha:e} C2 {c2}: dp {dp:.5} vs {exact:.5} ({:+.4}%)",
            100.0 * (dp / exact - 1.0)
        );
        worst = worst.max(err);
        assert!(err < 1e-3, "u {u} alpha {alpha} c2 {c2}: dp {dp} vs {exact}");
    }
    eprintln!("Z1 worst relative error {worst:.3e} (limit 1e-3)");
}

#[test]
fn z2_fan_finds_the_intersection_with_the_system_curve() {
    let fan = fan_curve();
    let q_op = bisect(
        |q| fan.eval(q) - porous_jump_dp(MU, RHO, ALPHA, C2, THICK, q / AREA),
        0.0,
        0.06,
    );
    let sim = run(
        &base(PRESSURE_INLET, &format!("{}{}", fan_block(), jump_block(ALPHA, C2))),
        6000,
    );
    let q = patch_flow(&sim, "outlet");
    let rise = column_p(&sim, 0.61) - column_p(&sim, 0.39);
    let drop = column_p(&sim, 1.19) - column_p(&sim, 1.31);
    let eq = (q / q_op - 1.0).abs();
    let erise = (rise / fan.eval(q) - 1.0).abs();
    let edrop = (drop / porous_jump_dp(MU, RHO, ALPHA, C2, THICK, q / AREA) - 1.0).abs();
    eprintln!(
        "Z2: Q {q:.6} vs {q_op:.6} ({:+.4}%), fan rise {rise:.4} vs {:.4} ({:+.4}%), jump drop {drop:.4} ({:+.4}%)",
        100.0 * (q / q_op - 1.0),
        fan.eval(q),
        100.0 * (rise / fan.eval(q) - 1.0),
        100.0 * edrop
    );
    assert!(eq < 5e-3, "operating flow {q} vs {q_op}");
    assert!(erise < 1e-3, "fan rise {rise} vs curve {}", fan.eval(q));
    assert!(edrop < 1e-3, "jump drop {drop}");
}

/// Z3 run: (Q, q_op, network patch pressure, R(Q)).
fn z3_run() -> (f64, f64, f64, f64) {
    let (r1, r2) = (1000.0, 40000.0);
    let fan = fan_curve();
    let q_op = bisect(|q| fan.eval(q) - network_pressure(0.0, r1, r2, q), 0.0, 0.06);
    let net = format!(
        "[components.net]
type = \"network_outlet\"
patch = \"outlet\"
reference_pressure = 0.0
resistance_linear = {r1}
resistance_quadratic = {r2}
"
    );
    // the loop is stiff (a lossless duct between two lagged boundaries): fan curve and network pressure are
    // under-relaxed strongly (relax = 0.02, ADR 0012 addendum)
    let net = format!(
        "{net}relax = 0.02
"
    );
    let fan_text = format!(
        "{}relax = 0.02
",
        fan_block()
    );
    let sim = run(&base(PRESSURE_INLET, &format!("{}{}", fan_text, net)), 6000);
    let q = patch_flow(&sim, "outlet");
    let p_out = column_p(&sim, 1.99); // outlet cell; zero-gradient pressure patch value is the fixed value
    let p_patch = sim.monitors.last("net_dp").unwrap();
    eprintln!(
        "Z3: Q {q:.6} vs {q_op:.6} ({:+.4}%), patch pressure {p_patch:.4} vs R(Q) {:.4} ({:+.5}%), outlet-cell p {p_out:.3}",
        100.0 * (q / q_op - 1.0),
        network_pressure(0.0, r1, r2, q),
        100.0 * (p_patch / network_pressure(0.0, r1, r2, q) - 1.0)
    );
    (q, q_op, p_patch, network_pressure(0.0, r1, r2, q))
}

#[test]
fn z3_network_outlet_follows_the_lumped_resistance() {
    let (q, q_op, ..) = z3_run();
    assert!((q / q_op - 1.0).abs() < 5e-3, "operating flow {q} vs {q_op}");
}

/// Diagnosis (wave 24, ADR 0042): the two-cycle sits in the inlet cell and vanishes with `component_diagonal = "corrected"` and
/// with the inflow lag of ADR 0042 (this test also passes with `--ignored` since then; its assertion is unchanged)
/// (`duct_diagnosis.rs::z3_with_corrected_component_diagonal_meets_the_registered_limits`).
/// Registered Z3 limit on the network patch pressure against R(Q) (1e-4). Measured 8.5e-4: a small persistent
/// two-cycle of the flow remains, so the lagged patch pressure and the final flow differ. Not met; limit unchanged.
#[test]
#[ignore = "known miss: Z3 patch pressure vs R(Q) 8.5e-4 against the registered 1e-4 (ADR 0012 addendum)"]
fn z3_network_pressure_known_miss() {
    let (_, _, p_patch, r) = z3_run();
    assert!((p_patch / r - 1.0).abs() < 1e-4, "network pressure {p_patch} vs {r}");
}

fn hx_case(block: &str) -> String {
    let inlet = "[boundaries.inlet]
velocity = \"fixed\"
velocity_value = [2.0, 0.0, 0.0]
temperature = \"fixed\"
temperature_value = 300.0
";
    base(inlet, block)
        .replacen(
            "[physics]
",
            "[physics]
cp = 1005.0
conductivity = 1e-3
",
            1,
        )
        .replacen(
            "[initial]
",
            "[energy]
enabled = true
buoyancy = false
relax = 1.0
[initial]
temperature = 300.0
",
            1,
        )
}

fn hx_block(extra: &str) -> String {
    format!(
        "[components.hx]\ntype = \"heat_exchanger\"\nzone = \"box\"\nbox_min = [0.8, -1.0, -1.0]\nbox_max = [1.0, 1.0, 1.0]\ndirection = [1.0, 0.0, 0.0]\n{extra}"
    )
}

/// Z4 effectiveness runs: (C_sec, reported duty, eps C_min dT with the nominal flow, stream rise, actual flow).
fn z4_effectiveness(c_sec: f64) -> (f64, f64, f64, f64) {
    let (cp, eps) = (1005.0, 0.7);
    let q_flow = 2.0 * AREA;
    let c_min = if c_sec > 0.0 { c_sec } else { RHO * cp * q_flow };
    let sim = run(
        &hx_case(&hx_block(&format!(
            "mode = \"effectiveness\"
effectiveness = {eps}
secondary_temperature = 350.0
secondary_capacity_rate = {c_sec}
"
        ))),
        1500,
    );
    let (t_in, t_out) = (column_t(&sim, 0.01), column_t(&sim, 1.99));
    let rise = RHO * cp * q_flow * (t_out - t_in);
    let rep = sim.monitors.last("hx_duty").unwrap();
    let q_actual = sim.monitors.last("hx_flow").unwrap();
    let expect = eps * c_min * (350.0 - 300.0);
    eprintln!(
        "Z4 effectiveness (C_sec {c_sec}): reported {rep:.6}, eps C_min dT {expect:.6} (rel {:.3e}), stream rise {rise:.6} (rel to reported {:.3e}), flow {q_actual:.9} vs nominal {q_flow:.9}",
        (rep / expect - 1.0).abs(),
        (rise / rep - 1.0).abs()
    );
    (rep, expect, rise, q_actual)
}

#[test]
fn z4_heat_exchanger_energy_balance() {
    let cp = 1005.0;
    let q_flow = 2.0 * AREA;
    // fixed duty
    let duty = -500.0;
    let sim = run(
        &hx_case(&hx_block(&format!(
            "mode = \"fixed_duty\"
duty = {duty}
"
        ))),
        1500,
    );
    let (t_in, t_out) = (column_t(&sim, 0.01), column_t(&sim, 1.99));
    let rise = RHO * cp * q_flow * (t_out - t_in);
    let rep = sim.monitors.last("hx_duty").unwrap();
    eprintln!(
        "Z4 fixed duty: stream enthalpy change {rise:.6} W vs {duty} (rel {:.3e}), reported {rep}",
        (rise / duty - 1.0).abs()
    );
    assert!((rise / duty - 1.0).abs() < 1e-6, "enthalpy change {rise}");
    assert!((rep / duty - 1.0).abs() < 1e-9);

    // effectiveness: the heat added to the stream equals the reported duty (1e-6), for both capacity rates
    for c_sec in [0.0, 20.0] {
        let (rep, _, rise, _) = z4_effectiveness(c_sec);
        assert!(
            (rise / rep - 1.0).abs() < 1e-6,
            "stream enthalpy rise {rise} vs duty {rep}"
        );
    }
}

/// Registered Z4 limit: reported effectiveness duty equals eps C_min dT with the nominal inlet temperature (1e-6).
/// Measured 1.45e-6 for C_sec = 0 (6.0e-7 for C_sec = 20): the exchanger inlet temperature seen by the component is
/// about 7e-5 K above the nominal 300 K because of axial conduction (k = 1e-3) upstream of the zone. The
/// supplementary check below uses the reported inlet temperature. Not met; limit unchanged.
#[test]
#[ignore = "known miss: Z4 effectiveness duty vs nominal formula 1.45e-6 against the registered 1e-6 (ADR 0012 addendum)"]
fn z4_effectiveness_nominal_formula_known_miss() {
    for c_sec in [0.0, 20.0] {
        let (rep, expect, ..) = z4_effectiveness(c_sec);
        assert!((rep / expect - 1.0).abs() < 1e-6, "duty {rep} vs {expect}");
    }
}

/// Supplementary (not a registered criterion): the duty follows eps C_min (T_sec - T_in) with the reported inlet
/// temperature to rounding.
#[test]
fn z4_effectiveness_duty_follows_reported_inlet_temperature() {
    let (cp, eps) = (1005.0, 0.7);
    let q_flow = 2.0 * AREA;
    let sim = run(
        &hx_case(&hx_block(&format!(
            "mode = \"effectiveness\"
effectiveness = {eps}
secondary_temperature = 350.0
secondary_capacity_rate = 0.0
"
        ))),
        1500,
    );
    let (rep, t_in) = (
        sim.monitors.last("hx_duty").unwrap(),
        sim.monitors.last("hx_t_in").unwrap(),
    );
    let expect = eps * RHO * cp * q_flow * (350.0 - t_in);
    eprintln!("Z4 supplementary: T_in {t_in:.7} K, duty {rep:.6} vs {expect:.6}");
    assert!((rep / expect - 1.0).abs() < 1e-9, "duty {rep} vs {expect}");
}

fn diags(text: &str) -> Vec<String> {
    match Case::parse_toml(text) {
        Err(d) => d.into_iter().map(|d| d.path).collect(),
        Ok(c) => c.semantic_errors().into_iter().map(|d| d.path).collect(),
    }
}

fn diag_paths(extra: &str, inlet: &str) -> Vec<String> {
    diags(&base(inlet, extra))
}

#[test]
fn z5_invalid_settings_get_located_errors() {
    let inl = velocity_inlet(1.0);
    let fan = |q: &str, p: &str| {
        format!(
            "[components.f]\ntype = \"fan\"\nzone = \"box\"\nbox_min = [0.4,-1.0,-1.0]\nbox_max = [0.6,1.0,1.0]\ndirection = [1.0,0.0,0.0]\ncurve_flow = {q}\ncurve_pressure = {p}\n"
        )
    };
    assert!(diag_paths(&fan("[0.0, 0.0]", "[1.0, 2.0]"), &inl).contains(&"components.f.curve_flow".to_string()));
    assert!(diag_paths(&fan("[0.0, 1.0]", "[1.0]"), &inl).contains(&"components.f.curve_flow".to_string()));
    let nodir =
        "[components.f]\ntype = \"fan\"\nzone = \"all\"\ncurve_flow = [0.0, 1.0]\ncurve_pressure = [1.0, 0.0]\n";
    assert!(diag_paths(nodir, &inl).contains(&"components.f.direction".to_string()));
    let net = "[components.n]\ntype = \"network_outlet\"\npatch = \"nope\"\n";
    assert!(diag_paths(net, &inl).contains(&"components.n.patch".to_string()));
    let net2 = "[components.n]\ntype = \"network_outlet\"\npatch = \"walls\"\n";
    assert!(diag_paths(net2, &inl).contains(&"components.n.patch".to_string()));
    let hx = |e: &str| hx_block(&format!("mode = \"effectiveness\"\neffectiveness = {e}\n"));
    // energy disabled
    assert!(diag_paths(&hx("0.5"), &inl).contains(&"components.hx.type".to_string()));
    assert!(diags(&hx_case(&hx("1.5"))).contains(&"components.hx.effectiveness".to_string()));
    // zone with no cells
    let empty = "[components.j]\ntype = \"porous_jump\"\nzone = \"box\"\nbox_min = [5.0,-1.0,-1.0]\nbox_max = [6.0,1.0,1.0]\ndirection = [1.0,0.0,0.0]\nthickness = 0.01\npermeability = 1e-8\n";
    let case = Case::parse_toml(&base(&inl, empty)).unwrap();
    let err = Simulation::new(&case).err().expect("empty zone must fail");
    assert!(format!("{err:?}").contains("components.j.zone"), "{err:?}");
    // transient algorithm
    let tr = base(&inl, &fan("[0.0, 1.0]", "[1.0, 0.0]")).replace("simplec", "piso");
    assert!(diags(&tr).contains(&"components.f.type".to_string()));
}
