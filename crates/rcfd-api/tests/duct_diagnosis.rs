//! Wave 24 duct diagnosis (ADR 0042): plug flow between symmetry walls, and the pressure-pressure duct.
//!
//! The acceptance limits are those registered in docs/adr/0042-wave24-duct-diagnosis.md (section "Registered acceptance limits")
//! before any solver change. Tests marked `known_miss` are ignored and keep their assertion; they record the behaviour of the
//! default solver settings.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::lumped::network_pressure;

// ---------------------------------------------------------------------------------------------------------------
// Symmetry-wall duct

fn sym_duct(walls: &str, algo: &str, steps: usize, extra: &str) -> Simulation {
    let src = format!(
        r#"
[mesh]
generator = "channel_2d"
nx = 60
ny = 10
lx = 2.0
ly = 0.2
depth = 0.05
[physics]
nu = 1e-3
rho = 1.2
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.inlet]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
{walls}
[solver]
algorithm = "{algo}"
convection = "upwind"
h_operator = "from_solution"
momentum_rel_tol = 1e-8
pressure_rel_tol = 1e-8
pressure_max_iter = 1000
max_iterations = 3000
tolerance_u = 1e-10
tolerance_p = 1e-10
{extra}
"#
    );
    let case = Case::parse_toml(&src).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..steps {
        sim.step();
    }
    sim
}

/// (max |ux - 1|, max |uy|, max |uz|, max |p|, relative mass imbalance over all boundary faces).
fn plug_errors(sim: &Simulation) -> (f64, f64, f64, f64, f64) {
    let max_abs = |name: &str, shift: f64| {
        sim.field(name)
            .unwrap()
            .iter()
            .map(|v| {
                if v.is_finite() {
                    (v - shift).abs()
                } else {
                    f64::INFINITY
                }
            })
            .fold(0.0, f64::max)
    };
    let phi = sim.flux();
    let g = &sim.geom;
    let (mut net, mut inflow) = (0.0, 0.0);
    for f in g.n_internal..phi.len() {
        if g.empty[f] {
            continue;
        }
        net += phi[f];
        if phi[f] < 0.0 {
            inflow -= phi[f];
        }
    }
    (
        max_abs("ux", 1.0),
        max_abs("uy", 0.0),
        max_abs("uz", 0.0),
        max_abs("p", 0.0),
        net.abs() / inflow,
    )
}

fn check_plug(tag: &str, sim: &Simulation) {
    let (eu, ev, ew, ep, mass) = plug_errors(sim);
    eprintln!(
        "{tag}: max|ux-1| {eu:.3e}, max|uy| {ev:.3e}, max|uz| {ew:.3e}, max|p| {ep:.3e}, mass imbalance {mass:.3e}"
    );
    assert!(sim.failure().is_none(), "{tag}: {:?}", sim.failure());
    assert!(eu <= 1e-10, "{tag}: A1 max |ux - 1| = {eu:e}");
    assert!(ev <= 1e-10 && ew <= 1e-10, "{tag}: A2 max |uy|, |uz| = {ev:e}, {ew:e}");
    assert!(ep <= 1e-10, "{tag}: A3 max |p| = {ep:e}");
    assert!(mass <= 1e-12, "{tag}: A4 mass imbalance {mass:e}");
}

const SYM: &str = "velocity = \"symmetry\"";
const FIXED_STREAM: &str = "velocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]";
const CORRECTED: &str = "component_diagonal = \"corrected\"";

/// Control: walls moving with the stream (plug flow exact for the default settings too).
#[test]
fn a_control_walls_at_stream_velocity_hold_plug_flow() {
    for algo in ["simple", "simplec"] {
        check_plug(&format!("fixed walls {algo}"), &sym_duct(FIXED_STREAM, algo, 300, ""));
    }
}

/// A1 to A4 with the opt-in corrected component diagonal (ADR 0010): the symmetry-wall duct keeps the plug flow to round-off.
/// Together with the failing default (below) this isolates the averaged diagonal as the cause of the 0.159 deviation.
#[test]
fn a_symmetry_walls_hold_plug_flow_with_corrected_component_diagonal() {
    for algo in ["simple", "simplec"] {
        check_plug(
            &format!("symmetry corrected {algo}"),
            &sym_duct(SYM, algo, 300, CORRECTED),
        );
    }
}

/// A5: the default (averaged) diagonal. KNOWN MISS: SIMPLEC deviates from the plug flow by 0.159 in the wall cells (SIMPLE
/// diverges). Root cause: ADR 0042 (a_k / a_p < 1 in wall cells, the documented price of the baseline-preserving default).
#[test]
#[ignore = "KNOWN MISS: default component_diagonal = averaged multiplies the tangential wall-cell velocity by a_k/a_p (0.159, ADR 0042)"]
fn a5_symmetry_walls_default_diagonal_known_miss() {
    check_plug("symmetry default simplec", &sym_duct(SYM, "simplec", 300, ""));
}

/// Diagnostic (asserts the measured mechanism, not a quality limit): with the default diagonal the plug-flow error is of the
/// size recorded in ADR 0022/0033 and sits in the wall cells (first row of cells next to a symmetry plane).
#[test]
fn a6_default_deviation_is_located_in_the_wall_cells() {
    let sim = sym_duct(SYM, "simplec", 50, "");
    let c = sim.geom.mesh.cell_centres();
    let ux = sim.field("ux").unwrap();
    let (mut wall, mut inner) = (0.0_f64, 0.0_f64);
    for (i, v) in ux.iter().enumerate() {
        let e = (v - 1.0).abs();
        let y = c[i][1];
        if y < 0.02 + 1e-9 || y > 0.18 - 1e-9 {
            wall = wall.max(e);
        } else {
            inner = inner.max(e);
        }
    }
    eprintln!("A6 default simplec: wall-cell max |ux - 1| {wall:.3e}, interior max {inner:.3e}");
    assert!(wall > 0.1 && wall < 0.2, "wall-cell deviation {wall}");
    assert!(inner < wall, "interior deviation {inner} vs wall {wall}");
}

// ---------------------------------------------------------------------------------------------------------------
// Pressure-pressure duct (periodic in y: no side walls)

fn pp_text(inlet: &str, extra: &str, u0: f64, nu: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 100
ny = 4
lx = 2.0
ly = 1.0
periodic_y = true
[physics]
nu = {nu}
rho = 1.2
[initial]
velocity = [{u0}, 0.0, 0.0]
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
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
"#
    )
}

fn p_in(v: &str) -> String {
    format!("[boundaries.xmin]\nvelocity = \"zero_gradient\"\npressure = \"fixed\"\npressure_value = {v}\n")
}

const JUMP: &str = "[components.jump]\ntype = \"porous_jump\"\nzone = \"box\"\nbox_min = [1.2, -1.0, -1.0]\nbox_max = [1.3, 2.0, 1.0]\ndirection = [1.0, 0.0, 0.0]\nthickness = 0.02\npermeability = 1e-8\ninertial_resistance = 1000.0\n";

fn bed(k: &str, zone: &str) -> String {
    format!("[sources.bed]\ntype = \"porous\"\nmodel = \"darcy_forchheimer\"\npermeability = {k}\ninertial_resistance = 0.0\n{zone}\n")
}

const WHOLE: &str = "zone = \"all\"";
const FIRST_COLUMN: &str = "zone = \"box\"\nbox_min = [-1.0, -1.0, -1.0]\nbox_max = [0.015, 2.0, 1.0]";

fn sim_of(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"))
}

/// Largest |ux| after each of `steps` iterations (NaN once the field is not finite).
fn max_ux_history(sim: &mut Simulation, steps: usize) -> Vec<f64> {
    (0..steps)
        .map(|_| {
            sim.step();
            let ux = sim.field("ux").unwrap();
            if ux.iter().any(|v| !v.is_finite()) {
                f64::NAN
            } else {
                ux.iter().map(|v| v.abs()).fold(0.0, f64::max)
            }
        })
        .collect()
}

fn outlet_flux(sim: &Simulation, patch: &str) -> f64 {
    let p = sim.geom.mesh.patch(patch).unwrap();
    let phi = sim.flux();
    p.faces().filter(|&f| !sim.geom.empty[f]).map(|f| phi[f]).sum()
}

/// Well-posedness of the lossless duct: with `p_in != p_out`, constant area and no loss the steady momentum balance requires
/// `p_in = p_out` (ADR 0042), so there is no steady state; the iteration must not settle. Run at a viscosity large enough for the
/// iteration to be well behaved so that the absence of a steady state is the property seen, not a numerical failure of the
/// inlet cell (see the inlet-cell test below).
#[test]
fn b0_lossless_duct_with_a_pressure_difference_has_no_steady_state() {
    let mut sim = sim_of(&pp_text(&p_in("100.0"), "", 1.0, "15.0"));
    let h = max_ux_history(&mut sim, 400);
    let last = h[399];
    eprintln!(
        "B0 lossless dp=100, nu=15: max|ux| at iterations 50/100/200/400: {:.3e} {:.3e} {:.3e} {last:.3e}",
        h[49], h[99], h[199]
    );
    let grows = !last.is_finite() || (last > 1.2 * h[199] && h[199] > 1.2 * h[99] && h[99] > 1.2 * h[49]);
    assert!(
        grows,
        "the lossless pressure-pressure duct settled: {:?}",
        [h[49], h[99], h[199], last]
    );
}

/// Control for B0: with a pressure difference of zero the same duct keeps its uniform flow (any uniform u is steady).
#[test]
fn b0_lossless_duct_without_pressure_difference_keeps_uniform_flow() {
    let mut sim = sim_of(&pp_text(&p_in("0.0"), "", 1.0, "1.5e-5"));
    let h = max_ux_history(&mut sim, 200);
    assert!((h[199] - 1.0).abs() <= 1e-6, "{}", h[199]);
}

/// B (registered): pressure-pressure duct with a distributed Darcy bed has the closed-form flow
/// `u = dp K rho / (mu L)` (dp kinematic), identical flux at both ends.
#[test]
fn b_pressure_pressure_duct_with_darcy_bed_matches_closed_form() {
    let mut sim = sim_of(&pp_text(&p_in("100.0"), &bed("1e-6", WHOLE), 1.0, "1.5e-5"));
    for _ in 0..3000 {
        sim.step();
    }
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let area = 1.0 * 0.02;
    let q_out = outlet_flux(&sim, "xmax");
    let q_in = -outlet_flux(&sim, "xmin");
    let u_exact = 100.0 * 1e-6 * 1.2 / (1.8e-5 * 2.0);
    let e = (q_out / area / u_exact - 1.0).abs();
    let em = (q_in - q_out).abs() / q_out.abs();
    eprintln!("B: outlet u {:.6} vs closed form {u_exact:.6} (rel {e:.2e}, limit 1e-2); end flux mismatch {em:.2e} (limit 1e-12)", q_out / area);
    assert!(e <= 1e-2, "{e}");
    assert!(em <= 1e-12, "{em}");
}

/// D (registered): jump only, `p_in = p_out = 0`, initial flow 1 m/s: the flow decays. KNOWN MISS: diverged at molecular viscosity
/// before the inflow lag; with it the flow decays but too slowly for the registered 1500 iterations.
#[test]
#[ignore = "KNOWN MISS (registered limit D): jump-only pressure-pressure duct; diverged before the F2 inflow lag, after it decays but max|ux| is 4.96e-6 after 1500 iterations against 1e-6 (ADR 0042)"]
fn d_jump_only_decay_known_miss() {
    let mut sim = sim_of(&pp_text(&p_in("0.0"), JUMP, 1.0, "1.5e-5"));
    let h = max_ux_history(&mut sim, 1500);
    eprintln!("D: max|ux| after 1500 iterations {:e}", h[1499]);
    assert!(h[1499].is_finite() && h[1499] <= 1e-6, "{}", h[1499]);
}

/// Diagnostic of the mechanism (pre-fix behaviour, measured at commit 8989cdd): the first-iteration velocity of a lossless
/// pressure-pressure duct scales as 1/nu (the inlet cell has only the viscous diagonal), and it sits in the inlet cell. With the
/// inflow lag of ADR 0042 (F2) this no longer holds; see `b1f_`.
#[test]
#[ignore = "pre-fix mechanism record (ADR 0042, 8989cdd): fails once the inlet-cell inflow term is lagged (F2); see b1f_"]
fn b1_first_iteration_velocity_is_inversely_proportional_to_viscosity() {
    let first = |nu: &str| {
        let mut sim = sim_of(&pp_text(&p_in("100.0"), "", 1.0, nu));
        sim.step();
        let ux = sim.field("ux").unwrap();
        let c = sim.geom.mesh.cell_centres();
        let i = (0..ux.len())
            .max_by(|&a, &b| ux[a].abs().partial_cmp(&ux[b].abs()).unwrap())
            .unwrap();
        (ux[i].abs(), c[i][0])
    };
    let (u1, x1) = first("1.5e-5");
    let (u2, x2) = first("1.5e-4");
    let (u3, _) = first("1.5e-3");
    eprintln!("B1: first-iteration max|ux| {u1:.3e} (x {x1:.3}), {u2:.3e} (x {x2:.3}), {u3:.3e}");
    assert!(
        x1 < 0.02 && x2 < 0.02,
        "the maximum is not in the inlet cell: {x1} {x2}"
    );
    assert!(
        (8.0..12.0).contains(&(u1 / u2)) && (8.0..12.0).contains(&(u2 / u3)),
        "{u1} {u2} {u3}"
    );
}

/// Diagnostic of the mechanism (cause test, pre-fix behaviour, measured at commit 8989cdd): the jump-only pressure-pressure duct diverges, and a Darcy sink restricted to the
/// inlet cell column (nothing else changed) makes it converge, so the missing diagonal of the inlet cell is what is wrong.
#[test]
#[ignore = "pre-fix mechanism record (ADR 0042, 8989cdd): the jump-only duct no longer diverges once the inflow term is lagged (F2); see b2f_"]
fn b2_sink_in_the_inlet_cell_column_removes_the_divergence() {
    let mut bad = sim_of(&pp_text(&p_in("100.0"), JUMP, 1.0, "1.5e-5"));
    let hb = max_ux_history(&mut bad, 60);
    assert!(
        !hb[59].is_finite() || hb[59] > 1e3,
        "the jump-only duct did not diverge: {}",
        hb[59]
    );
    let extra = format!("{JUMP}{}", bed("1e-6", FIRST_COLUMN));
    let mut good = sim_of(&pp_text(&p_in("100.0"), &extra, 1.0, "1.5e-5"));
    let hg = max_ux_history(&mut good, 400);
    eprintln!(
        "B2: jump only after 60 its {:e}; with inlet-column sink max|ux| at 200/400 its {:.6} {:.6}",
        hb[59], hg[199], hg[399]
    );
    assert!(
        hg[399].is_finite() && (hg[399] - hg[199]).abs() <= 1e-6 * hg[399],
        "{hg:?}"
    );
}

/// Post-fix counterpart of b1: with the lagged inflow term the first-iteration velocity of the lossless pressure-pressure duct
/// no longer grows as 1/nu (measured 30.0 and 25.5 m/s for nu 1.5e-5 and 1.5e-3, ratio 1.2; before: 1.3e6 and 1.2e4).
#[test]
fn b1f_first_iteration_velocity_is_bounded_independently_of_viscosity() {
    let first = |nu: &str| {
        let mut sim = sim_of(&pp_text(&p_in("100.0"), "", 1.0, nu));
        sim.step();
        sim.field("ux").unwrap().iter().map(|v| v.abs()).fold(0.0, f64::max)
    };
    let (u1, u3) = (first("1.5e-5"), first("1.5e-3"));
    eprintln!("B1f: first-iteration max|ux| {u1:.3e} and {u3:.3e}");
    assert!(u1 < 100.0 && u3 < 100.0 && (0.5..2.0).contains(&(u1 / u3)), "{u1} {u3}");
}

/// Post-fix counterpart of b2: the jump-only pressure-pressure duct (`p_in = 100` kinematic = 120 Pa, no sink anywhere) is
/// finite and reaches the closed-form velocity: `dp = mu/alpha v t + rho c2 v^2 t / 2` gives `12 v^2 + 36 v - 120 = 0`,
/// `v = 2` m/s exactly. Limit 1e-3 relative (set after a first run gave 7.8e-5 at 1200 iterations; the tail is still decaying).
#[test]
fn b2f_jump_only_pressure_pressure_duct_reaches_the_closed_form_velocity() {
    let mut sim = sim_of(&pp_text(&p_in("100.0"), JUMP, 1.0, "1.5e-5"));
    let h = max_ux_history(&mut sim, 1200);
    let e = (h[1199] / 2.0 - 1.0).abs();
    eprintln!(
        "B2f: jump only, max|ux| at 600/1200 iterations {:.6} {:.6} (closed form 2, rel {e:.2e})",
        h[599], h[1199]
    );
    assert!(h[1199].is_finite() && e <= 1e-3, "{} {}", h[599], h[1199]);
}

// ---------------------------------------------------------------------------------------------------------------
// Z3 of ADR 0012 under the corrected diagonal (symmetry-wall duct with a fan and a network outlet)

fn z3_text(extra_solver: &str) -> String {
    format!(
        r#"
[mesh]
generator = "channel_2d"
nx = 100
ny = 2
lx = 2.0
ly = 0.2
depth = 0.1
[physics]
nu = {nu}
rho = 1.2
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "symmetry"
[boundaries.inlet]
velocity = "zero_gradient"
pressure = "fixed"
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
{extra_solver}
[components.fan]
type = "fan"
zone = "box"
box_min = [0.4, -1.0, -1.0]
box_max = [0.6, 1.0, 1.0]
direction = [1.0, 0.0, 0.0]
curve_flow = [0.0, 0.03, 0.06]
curve_pressure = [200.0, 140.0, 0.0]
relax = 0.02
[components.net]
type = "network_outlet"
patch = "outlet"
reference_pressure = 0.0
resistance_linear = 1000.0
resistance_quadratic = 40000.0
relax = 0.02
"#,
        nu = 1.8e-5 / 1.2
    )
}

fn z3_check(tag: &str, extra_solver: &str) {
    let mut sim = sim_of(&z3_text(extra_solver));
    for _ in 0..6000 {
        sim.step();
    }
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let q = outlet_flux(&sim, "outlet");
    let r = network_pressure(0.0, 1000.0, 40000.0, q);
    let p_patch = sim.monitors.last("net_dp").unwrap();
    let fan = rcfd_fv::lumped::FanCurve::new(vec![0.0, 0.03, 0.06], vec![200.0, 140.0, 0.0]).unwrap();
    let q_op = rcfd_fv::lumped::bisect(|q| fan.eval(q) - network_pressure(0.0, 1000.0, 40000.0, q), 0.0, 0.06);
    let (eq, ep) = ((q / q_op - 1.0).abs(), (p_patch / r - 1.0).abs());
    eprintln!("{tag}: Q {q:.6e} vs {q_op:.6e} (rel {eq:.2e}, limit 5e-3); patch pressure {p_patch:.5} vs R(Q) {r:.5} (rel {ep:.2e}, limit 1e-4)");
    assert!(eq < 5e-3, "{eq}");
    assert!(ep < 1e-4, "{ep}");
}

/// Z3 of ADR 0012 with `component_diagonal = "corrected"` (the set-up is otherwise the registered one): the patch pressure
/// agrees with `R(Q)` to the registered 1e-4 and the flow with the operating point to 5e-3.
#[test]
fn z3_with_corrected_component_diagonal_meets_the_registered_limits() {
    z3_check("Z3 corrected", CORRECTED);
}

/// Z3 of ADR 0012 exactly as registered (default settings). Met since the inflow lag of ADR 0042 (before it: patch pressure
/// +0.085 % against R(Q), a persistent two-cycle in the inlet cell; the old ignored test `lumped.rs::z3_network_pressure_known_miss`
/// keeps its assertion and is not edited).
#[test]
fn z3_default_settings_meet_the_registered_limits_with_the_inflow_lag() {
    z3_check("Z3 default", "");
}

// ---------------------------------------------------------------------------------------------------------------
// X4 of ADR 0039 as registered (fan boundary and porous jump in a lossless pressure-pressure duct, no bed)

/// The registered limits of `heat_exchanger.rs::x4_run` (5e-3 on Q, 2e-3 on the patch pressure) on the registered set-up, without
/// the stationarity helper of that test (which compares ux 300 iterations apart at 1e-8 and gives 4.06e-8 at 8000 iterations: the
/// old test `x4_registered_lossless_setup_known_miss` stays ignored and unedited).
#[test]
fn x4_registered_setup_reaches_the_operating_point_with_the_inflow_lag() {
    use rcfd_fv::lumped::{bisect, porous_jump_dp, FanCurve};
    let (rho, mu, ly, depth) = (1.2, 1.8e-5, 1.0, 0.02);
    let area = ly * depth;
    let (fq, fp) = ([0.0, 0.03, 0.06], [200.0, 140.0, 0.0]);
    let fan = FanCurve::new(fq.to_vec(), fp.to_vec()).unwrap();
    let (alpha, c2, thick) = (1e-8, 1000.0, 0.02);
    let q_op = bisect(
        |q| fan.eval(q) - porous_jump_dp(mu, rho, alpha, c2, thick, q / area),
        0.0,
        0.06,
    );
    let comps = format!(
        "[components.fan]
type = \"fan_boundary\"
patch = \"xmin\"
curve_flow = {fq:?}
curve_pressure = {fp:?}
reference_pressure = 0.0
relax = 0.02
[components.jump]
type = \"porous_jump\"
zone = \"box\"
box_min = [1.2, -1.0, -1.0]
box_max = [1.3, 2.0, 1.0]
direction = [1.0, 0.0, 0.0]
thickness = {thick}
permeability = {alpha}
inertial_resistance = {c2}
"
    );
    let inlet = "[boundaries.xmin]
velocity = \"zero_gradient\"
pressure = \"fixed\"
";
    let text = pp_text(inlet, &comps, 2.0, &format!("{}", mu / rho)).replace(
        "[initial]
velocity = [2.0, 0.0, 0.0]",
        "[initial]
velocity = [2.0, 0.0, 0.0]
temperature = 300.0",
    );
    let text = text.replace(
        "rho = 1.2
[initial]",
        "rho = 1.2
cp = 1005.0
conductivity = 1e-6
[initial]",
    );
    let mut sim = sim_of(&text);
    for _ in 0..8000 {
        sim.step();
    }
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let q = -outlet_flux(&sim, "xmin");
    let dp = sim.monitors.last("fan_dp").unwrap();
    let (eq, ec) = ((q / q_op - 1.0).abs(), (dp / fan.eval(q) - 1.0).abs());
    eprintln!("X4 (registered, lag): Q {q:.6e} vs {q_op:.6e} (rel {eq:.2e}, limit 5e-3); patch pressure {dp:.4} vs curve {:.4} (rel {ec:.2e}, limit 2e-3)", fan.eval(q));
    assert!(q > 0.0 && eq <= 5e-3, "Q {q} vs {q_op}");
    assert!(ec <= 2e-3, "patch pressure {dp} vs curve {}", fan.eval(q));
}
