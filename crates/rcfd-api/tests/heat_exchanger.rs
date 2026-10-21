//! Heat-exchanger macro cells (F-217, ADR 0039 D3). Limits are those registered in
//! docs/adr/0039-wave23a-mhd-heat-exchanger.md before the first run: X1 counterflow ntu mode, X2 loss closure,
//! X3 dual cell, X4 fan boundary against a porous jump, X5 located diagnostics. X6 (fan at three operating points) and X7 (vent
//! at six flows) are registered in docs/adr/0095-wave39d-field-registers-monitor-frames-fan-vent-curves.md.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::hx::{solve_exchanger, Arrangement};
use rcfd_fv::lumped::{bisect, porous_jump_dp, FanCurve};

const RHO: f64 = 1.2;
const CP: f64 = 1005.0;
const MU: f64 = 1.8e-5;

/// Duct in x, periodic in y (a true plug flow, no side wall condition). Depth of `box_2d` is lx/nx = 0.02.
fn case_text(ly: f64, inlet: &str, extra: &str, energy: bool) -> String {
    let en = if energy {
        "[energy]\nenabled = true\nbuoyancy = false\nrelax = 1.0\n"
    } else {
        ""
    };
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 100
ny = 4
lx = 2.0
ly = {ly}
periodic_y = true
[physics]
nu = {nu}
rho = {RHO}
cp = {CP}
conductivity = 1e-6
{en}
[initial]
velocity = [2.0, 0.0, 0.0]
temperature = 300.0
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
"#,
        nu = MU / RHO
    )
}

const HOT_INLET: &str = "[boundaries.xmin]\nvelocity = \"fixed\"\nvelocity_value = [2.0, 0.0, 0.0]\ntemperature = \"fixed\"\ntemperature_value = 300.0\n";

fn hx_block(extra: &str) -> String {
    format!(
        "[components.hx]\ntype = \"heat_exchanger\"\nzone = \"box\"\nbox_min = [0.8, -1.0, -1.0]\nbox_max = [1.0, 1.0, 1.0]\ndirection = [1.0, 0.0, 0.0]\n{extra}"
    )
}

/// Run `n` iterations then 300 more; require stationarity (relative change of ux, p, T below 1e-8). The pressure change is scaled by
/// `max(max|p|, 1)` (see the ADR 0039 addendum, deviation 4).
fn run(text: &str, n: usize) -> Simulation {
    run_floor(text, n, 1.0)
}

/// As `run` with the pressure-scale floor `p_floor` (1e-12 is the originally registered helper).
fn run_floor(text: &str, n: usize, p_floor: f64) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..n {
        sim.step();
    }
    let fields: Vec<&str> = ["ux", "p", "T"].into_iter().filter(|f| sim.field(f).is_ok()).collect();
    let before: Vec<Vec<f64>> = fields.iter().map(|f| sim.field(f).unwrap()).collect();
    for _ in 0..300 {
        sim.step();
    }
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    for (f, old) in fields.iter().zip(&before) {
        let new = sim.field(f).unwrap();
        let scale = new.iter().map(|v| v.abs()).fold(1e-12, f64::max);
        let scale = if *f == "p" { scale.max(p_floor) } else { scale };
        let change = new.iter().zip(old).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max) / scale;
        assert!(
            change < 1e-8,
            "{f} is not stationary after {n} iterations: change {change:.3e}"
        );
    }
    sim
}

/// Mean of a field over the cells of column `x_target` with `y` in `[y0, y1]`.
fn column(sim: &Simulation, name: &str, x_target: f64, y0: f64, y1: f64) -> f64 {
    let c = sim.geom.mesh.cell_centres();
    let f = sim.field(name).unwrap();
    let idx: Vec<usize> = (0..c.len())
        .filter(|&i| (c[i][0] - x_target).abs() < 1e-9 && c[i][1] >= y0 && c[i][1] <= y1)
        .collect();
    assert!(!idx.is_empty(), "no cells at x = {x_target}");
    idx.iter().map(|&i| f[i]).sum::<f64>() / idx.len() as f64
}

fn mon(sim: &Simulation, name: &str) -> f64 {
    sim.monitors.last(name).unwrap_or_else(|| panic!("no monitor {name}"))
}

/// Independent marched solution of two streams exchanging `UA` along a unit length: stream 1 (capacity rate `c1`, inlet `t1`)
/// flows in +x; stream 2 (`c2`, `t2`; 0 = infinite capacity) flows in -x (counter) or +x (parallel). Heat INTO stream 1 [W]. RK4,
/// 4000 steps, shooting by bisection for the counterflow.
fn marched_heat_into_1(counter: bool, c1: f64, c2: f64, ua: f64, t1: f64, t2: f64) -> f64 {
    if c2 <= 0.0 {
        // constant secondary temperature
        let n = 4000;
        let h = 1.0 / n as f64;
        let mut t = t1;
        let rhs = |t: f64| ua / c1 * (t2 - t);
        for _ in 0..n {
            let k1 = rhs(t);
            let k2 = rhs(t + 0.5 * h * k1);
            let k3 = rhs(t + 0.5 * h * k2);
            let k4 = rhs(t + h * k3);
            t += h / 6.0 * (k1 + 2.0 * k2 + 2.0 * k3 + k4);
        }
        return c1 * (t - t1);
    }
    // state (T1, T2) along x; dT1/dx = ua/c1 (T2 - T1); parallel: dT2/dx = ua/c2 (T1 - T2); counter: dT2/dx = ua/c2 (T2 - T1)
    let sgn = if counter { -1.0 } else { 1.0 };
    let march = |t2_start: f64| -> (f64, f64) {
        let n = 4000;
        let h = 1.0 / n as f64;
        let (mut a, mut b) = (t1, t2_start);
        let rhs = |a: f64, b: f64| (ua / c1 * (b - a), sgn * ua / c2 * (a - b));
        for _ in 0..n {
            let (k1a, k1b) = rhs(a, b);
            let (k2a, k2b) = rhs(a + 0.5 * h * k1a, b + 0.5 * h * k1b);
            let (k3a, k3b) = rhs(a + 0.5 * h * k2a, b + 0.5 * h * k2b);
            let (k4a, k4b) = rhs(a + h * k3a, b + h * k3b);
            a += h / 6.0 * (k1a + 2.0 * k2a + 2.0 * k3a + k4a);
            b += h / 6.0 * (k1b + 2.0 * k2b + 2.0 * k3b + k4b);
        }
        (a, b)
    };
    if !counter {
        let (a, _) = march(t2);
        return c1 * (a - t1);
    }
    // counterflow: T2(1) = t2 (inlet at x = 1); shoot on T2(0)
    let (lo, hi) = (t1.min(t2), t1.max(t2));
    let (mut lo_s, mut hi_s) = (lo, hi);
    for _ in 0..200 {
        let mid = 0.5 * (lo_s + hi_s);
        if march(mid).1 - t2 > 0.0 {
            hi_s = mid;
        } else {
            lo_s = mid;
        }
    }
    let guess = 0.5 * (lo_s + hi_s);
    let (a, _) = march(guess);
    c1 * (a - t1)
}

/// X1 run: returns (duty, rise, eps-NTU duty with the reported inlet, marched duty with the reported inlet, flow).
fn x1_run(arr: &str, arrangement: Arrangement, c_sec: f64) -> (f64, f64, f64, f64, f64) {
    let ua = 40.0;
    let block = hx_block(&format!(
        "mode = \"ntu\"\narrangement = \"{arr}\"\nua = {ua}\nsecondary_temperature = 350.0\nsecondary_capacity_rate = {c_sec}\n"
    ));
    let sim = run(&case_text(0.2, HOT_INLET, &block, true), 3000);
    let (t_in, t_out) = (column(&sim, "T", 0.01, 0.0, 1.0), column(&sim, "T", 1.99, 0.0, 1.0));
    let q = mon(&sim, "hx_flow");
    let duty = mon(&sim, "hx_duty");
    let rise = RHO * CP * q * (t_out - t_in);
    let t_rep = mon(&sim, "hx_t_in");
    let lib = solve_exchanger(arrangement, RHO * CP * q, c_sec, ua, t_rep, 350.0).q;
    let marched = marched_heat_into_1(arr == "counterflow", RHO * CP * q, c_sec, ua, t_rep, 350.0);
    eprintln!(
        "X1 {arr} C_sec {c_sec}: Q {q:.6e} m3/s, duty {duty:.6} W; stream enthalpy rise {rise:.6} (rel {:.2e}, limit 1e-6); eps-NTU lib {lib:.6} (rel {:.2e}, limit 1e-9); marched {marched:.6} (rel {:.2e}, limits 1e-4 and 2e-2); eps {:.6} NTU {:.4}",
        (rise / duty - 1.0).abs(),
        (duty / lib - 1.0).abs(),
        (duty / marched - 1.0).abs(),
        mon(&sim, "hx_eps"),
        mon(&sim, "hx_ntu")
    );
    (duty, rise, lib, marched, q)
}

#[test]
fn x1_ntu_exchanger_matches_effectiveness_ntu_and_marched_solution() {
    for (arr, a, c_sec) in [
        ("counterflow", Arrangement::Counterflow, 20.0),
        ("counterflow", Arrangement::Counterflow, 0.0),
        ("parallel", Arrangement::Parallel, 20.0),
    ] {
        let (duty, rise, lib, marched, _) = x1_run(arr, a, c_sec);
        assert!(duty > 0.0);
        assert!(
            (rise / duty - 1.0).abs() <= 1e-6,
            "{arr} {c_sec}: enthalpy rise {rise} vs duty {duty}"
        );
        assert!(
            (duty / lib - 1.0).abs() <= 1e-9,
            "{arr} {c_sec}: duty {duty} vs eps-NTU {lib}"
        );
        assert!(
            (duty / marched - 1.0).abs() <= 1e-4,
            "{arr} {c_sec}: duty {duty} vs marched {marched}"
        );
        assert!((duty / marched - 1.0).abs() <= 2e-2);
    }
}

#[test]
fn x2_loss_closure_gives_the_prescribed_pressure_drop() {
    let u = 2.0;
    let dp_of = |extra: &str| -> f64 {
        let block = hx_block(&format!("mode = \"fixed_duty\"\nduty = 0.0\n{extra}"));
        let sim = run(&case_text(0.2, HOT_INLET, &block, true), 2500);
        let p = |x: f64| RHO * column(&sim, "p", x, 0.0, 1.0);
        p(0.99) - p(1.01)
    };
    let reference = dp_of("");
    let mut worst: f64 = 0.0;
    for (extra, want) in [
        ("loss_coefficient = 5.0\n", 5.0 * RHO * u * u / 2.0),
        ("loss_linear = 2.0\n", 2.0 * u),
    ] {
        let dp = dp_of(extra) - reference;
        let e = (dp / want - 1.0).abs();
        eprintln!(
            "X2 {}: dp {dp:.6} Pa vs {want:.6} (rel {e:.2e}, limit 1e-3), reference dp {reference:.2e}",
            extra.trim()
        );
        worst = worst.max(e);
    }
    assert!(worst <= 1e-3, "worst relative error {worst}");
}

#[test]
fn x3_dual_cell_exchanges_equal_and_opposite_heat() {
    let ua = 60.0;
    let block = format!(
        "[components.hx]\ntype = \"heat_exchanger\"\nzone = \"box\"\nbox_min = [0.8, -1.0, -1.0]\nbox_max = [1.0, 0.1, 1.0]\ndirection = [1.0, 0.0, 0.0]\nmode = \"ntu\"\narrangement = \"counterflow\"\nua = {ua}\ndual_cell = true\n[components.hx.secondary]\nzone = \"box\"\nbox_min = [0.8, 0.1, -1.0]\nbox_max = [1.0, 1.0, 1.0]\ndirection = [1.0, 0.0, 0.0]\n"
    );
    let inlet = "[boundaries.xmin]\nvelocity = \"fixed\"\nvelocity_value = [2.0, 0.0, 0.0]\ntemperature = \"fixed\"\ntemperature_expression = \"if(y < 0.1 [m], 350 [K], 300 [K])\"\n";
    let sim = run(&case_text(0.2, inlet, &block, true), 4000);
    let (q1, q2) = (mon(&sim, "hx_flow"), mon(&sim, "hx_sec_flow"));
    let duty = mon(&sim, "hx_duty");
    let rise =
        |y0: f64, y1: f64, q: f64| RHO * CP * q * (column(&sim, "T", 1.99, y0, y1) - column(&sim, "T", 0.01, y0, y1));
    let (r1, r2) = (rise(0.0, 0.1, q1), rise(0.1, 0.2, q2));
    let sec_duty = mon(&sim, "hx_sec_duty");
    let t1 = mon(&sim, "hx_t_in");
    let t2 = mon(&sim, "hx_sec_t_in");
    let marched = marched_heat_into_1(true, RHO * CP * q1, RHO * CP * q2, ua, t1, t2);
    eprintln!(
        "X3: flows {q1:.6e} {q2:.6e}; duty {duty:.6} W; primary rise {r1:.6} (rel {:.2e}), secondary rise {r2:.6} (rel {:.2e}), sum/|Q| {:.2e} (limit 1e-6), sec duty {sec_duty:.6}; marched {marched:.6} (rel {:.2e}, limits 1e-4 and 2e-2); inlets {t1:.4} {t2:.4}",
        (r1 / duty - 1.0).abs(),
        (r2 / -duty - 1.0).abs(),
        (r1 + r2).abs() / duty.abs(),
        (duty / marched - 1.0).abs()
    );
    assert!(duty < 0.0, "heat flows from the hot (primary) stream: {duty}");
    assert!((r1 / duty - 1.0).abs() <= 1e-6, "primary rise {r1} vs {duty}");
    assert!((r2 / -duty - 1.0).abs() <= 1e-6, "secondary rise {r2} vs {}", -duty);
    assert!((r1 + r2).abs() <= 1e-6 * duty.abs());
    assert!((duty / marched - 1.0).abs() <= 1e-4, "duty {duty} vs marched {marched}");
    assert!((duty / marched - 1.0).abs() <= 2e-2);
}

#[test]
fn x4_fan_boundary_reproduces_its_pressure_flow_curve() {
    x4_run(true, 1e-6);
}

/// Diagnosis (wave 24, ADR 0042): the set-up is well-posed (fan curve against the jump has an operating point); the divergence
/// is the missing momentum diagonal of the fixed-pressure inlet cell (`duct_diagnosis.rs`), not ill-posed physics.
/// KNOWN MISS: the registered X4 set-up (lossless duct, fan boundary at the inlet, porous jump, outlet pressure 0, initial speed 2 m/s)
/// diverges (flow NaN within about 8 iterations, patch pressure stays near 200 Pa); see ADR 0039 addendum, deviation 2. The corrected set-up
/// with a distributed Darcy bed is `x4_fan_boundary_reproduces_its_pressure_flow_curve`.
#[test]
#[ignore = "KNOWN MISS: the registered lossless pressure-pressure duct of X4 diverges in the segregated iteration (flow NaN)"]
fn x4_registered_lossless_setup_known_miss() {
    x4_run(false, 1e-6);
}

/// Originally registered stationarity helper (pressure scale floor 1e-12): the pressure of a lossless periodic duct is zero to round-off,
/// so its relative change is noise (measured 0.42 for X1, 0.30 for X2, 0.13 for X3). Corrected helper: `run`.
#[test]
#[ignore = "KNOWN MISS: the registered stationarity test on the round-off pressure of a lossless duct fails (relative change 0.42)"]
fn x1_registered_stationarity_known_miss() {
    let block = hx_block(
        "mode = \"ntu\"\narrangement = \"counterflow\"\nua = 40.0\nsecondary_temperature = 350.0\nsecondary_capacity_rate = 20.0\n",
    );
    run_floor(&case_text(0.2, HOT_INLET, &block, true), 3000, 1e-12);
}

/// X4 duct with bed permeability `k_bed` (the bed resistance sets the operating point); returns (Q, patch pressure).
fn x4_run(with_bed: bool, k_bed: f64) -> (f64, f64) {
    let (ly, depth) = (1.0, 0.02);
    let area = ly * depth;
    let (fq, fp) = ([0.0, 0.03, 0.06], [200.0, 140.0, 0.0]);
    let fan = FanCurve::new(fq.to_vec(), fp.to_vec()).unwrap();
    let (alpha, c2, thick) = (1e-8, 1000.0, 0.02);
    // A lossless duct between a pressure inlet and a pressure outlet is unstable in the segregated iteration (measured: divergence
    // within 2 iterations with the jump alone), so the whole duct carries a linear Darcy bed `dp = mu/K v L` (L = 2 m); it enters the
    // analytic operating point.
    let length = 2.0;
    let bed_on = if with_bed { 1.0 } else { 0.0 };
    let q_op = bisect(
        |q| {
            let v = q / area;
            fan.eval(q) - porous_jump_dp(MU, RHO, alpha, c2, thick, v) - bed_on * MU / k_bed * v * length
        },
        0.0,
        0.06,
    );
    let bed = if with_bed {
        format!("[sources.bed]\ntype = \"porous\"\nmodel = \"darcy_forchheimer\"\npermeability = {k_bed}\ninertial_resistance = 0.0\nzone = \"all\"\n")
    } else {
        String::new()
    };
    let comps = format!(
        "[components.fan]\ntype = \"fan_boundary\"\npatch = \"xmin\"\ncurve_flow = {fq:?}\ncurve_pressure = {fp:?}\nreference_pressure = 0.0\nrelax = 0.02\n[components.jump]\ntype = \"porous_jump\"\nzone = \"box\"\nbox_min = [1.2, -1.0, -1.0]\nbox_max = [1.3, 2.0, 1.0]\ndirection = [1.0, 0.0, 0.0]\nthickness = {thick}\npermeability = {alpha}\ninertial_resistance = {c2}\n{bed}"
    );
    let inlet = "[boundaries.xmin]\nvelocity = \"zero_gradient\"\npressure = \"fixed\"\n";
    let mut text = case_text(ly, inlet, &comps, false);
    if with_bed {
        text = text.replace(
            "velocity = [2.0, 0.0, 0.0]
temperature",
            "velocity = [1.0, 0.0, 0.0]
temperature",
        );
    }
    let sim = run(&text, if with_bed { 2000 } else { 8000 });
    let p = sim.geom.mesh.patch("xmin").unwrap();
    let phi = sim.flux();
    let q: f64 = -p.faces().filter(|&f| !sim.geom.empty[f]).map(|f| phi[f]).sum::<f64>();
    let dp = mon(&sim, "fan_dp");
    let eq = (q / q_op - 1.0).abs();
    let ec = (dp / fan.eval(q) - 1.0).abs();
    eprintln!(
        "X4: Q {q:.6e} vs operating point {q_op:.6e} (rel {eq:.2e}, limit 5e-3); patch pressure {dp:.4} vs curve {:.4} (rel {ec:.2e}, limit 2e-3); jump dp {:.4}",
        fan.eval(q),
        mon(&sim, "jump_dp")
    );
    assert!(q > 0.0 && eq <= 5e-3, "Q {q} vs {q_op}");
    assert!(ec <= 2e-3, "patch pressure {dp} vs curve {}", fan.eval(q));
    (q, dp)
}

/// X6 (ADR 0095): the fan boundary at three operating points, one on each linear segment of the curve and one near the knee,
/// set by the bed permeability on the X4 set-up; each point must meet the X4 limits and the three flows must be distinct.
#[test]
fn x6_fan_boundary_at_three_operating_points() {
    let mut qs = Vec::new();
    for k_bed in [3e-7, 1e-6, 1e-5] {
        let (q, dp) = x4_run(true, k_bed);
        eprintln!("X6 k_bed {k_bed:e}: Q {q:.6e}, patch pressure {dp:.4}");
        qs.push(q);
    }
    assert!(qs[0] < 0.03 && qs[2] > 0.03, "points not on both segments: {qs:?}");
    assert!(
        qs.windows(2).all(|w| w[1] > w[0] * 1.05),
        "operating points not distinct: {qs:?}"
    );
}

/// X7 (ADR 0095) vent on its own set-up, independent of the F-200 channel: the X1 periodic duct (no side walls, plug flow) with a
/// fixed inlet velocity `u` at xmin (negative: the flow enters through the vent), a vent with loss coefficient `k` and external
/// pressure 0 at xmax, physical pseudo-time step `dt` (plain SIMPLEC diverges for forward flow, see the ADR). Returns the mean
/// kinematic pressure of the outlet column.
fn vent_run(u: f64, k: f64, dt: f64) -> f64 {
    let inlet = format!("[boundaries.xmin]\nvelocity = \"fixed\"\nvelocity_value = [{u:?}, 0.0, 0.0]\n");
    let pseudo = format!("pseudo_mode = \"physical\"\npseudo_dt = {dt:?}\n");
    let vent = format!("[boundaries.xmax]\nbc_kind = \"vent\"\nloss_coefficient = {k:?}\nbc_pressure = 0.0\n");
    let text = case_text(0.2, &inlet, &pseudo, false)
        .replace(
            "[boundaries.xmax]\nvelocity = \"zero_gradient\"\npressure = \"fixed\"\n",
            &vent,
        )
        .replace("velocity = [2.0, 0.0, 0.0]", &format!("velocity = [{u:?}, 0.0, 0.0]"));
    assert!(text.contains("bc_kind = \"vent\""), "vent block not substituted");
    let sim = run(&text, 2000);
    column(&sim, "p", 1.99, -1.0, 1.0)
}

/// X7: the outlet pressure follows 0.5 K u |u| at six flows (limit 1e-6 relative, ADR 0022 C3); `dt_of(u)` is the pseudo step. At
/// |u| = 0.5 and 2 the linear law 0.5 K u is off by a factor 2 and 0.5, so the quadratic law is distinguished.
fn x7_sweep(dt_of: impl Fn(f64) -> f64) {
    let k = 2.5;
    let mut worst = 0.0_f64;
    for u in [0.5, 1.0, 2.0, -0.5, -1.0, -2.0] {
        let p = vent_run(u, k, dt_of(u));
        let want = 0.5 * k * u * u.abs();
        let e = (p / want - 1.0).abs();
        eprintln!(
            "X7 u {u} dt {}: outlet p {p:.12e} vs 0.5 K u|u| = {want:.6e} (rel {e:.2e}, limit 1e-6); linear law 0.5 K u = {:.4e}",
            dt_of(u),
            0.5 * k * u
        );
        // the `run` helper's max-fold skips NaN, so finiteness is checked here
        assert!(p.is_finite() && e <= 1e-6, "u {u}: p {p} vs {want}");
        worst = worst.max(e);
    }
    eprintln!("X7 worst relative error {worst:.2e}");
}

/// Corrected set-up (ADR 0095 amendment 1): pseudo step 0.02/|u|, a Courant number of 1 at every flow.
#[test]
fn x7_vent_reproduces_its_quadratic_loss_law() {
    x7_sweep(|u| 0.02 / u.abs());
}

/// KNOWN MISS (ADR 0095 amendment 1): the registered fixed pseudo step 0.02 passes at u = 0.5 (2.85e-12) and 1 (1.36e-13) and gives
/// NaN at u = 2 (Courant number 2).
#[test]
#[ignore = "KNOWN MISS: the forward vent at u = 2 with pseudo_dt 0.02 (Courant 2) gives NaN"]
fn x7_vent_at_fixed_pseudo_dt_known_miss() {
    x7_sweep(|_| 0.02);
}

fn diags(text: &str) -> Vec<String> {
    match Case::parse_toml(text) {
        Err(d) => d.into_iter().map(|d| d.path).collect(),
        Ok(c) => c.semantic_errors().into_iter().map(|d| d.path).collect(),
    }
}

#[test]
fn x5_invalid_settings_get_located_errors() {
    let with = |extra: &str| diags(&case_text(0.2, HOT_INLET, &hx_block(extra), true));
    let ok = with("mode = \"ntu\"\nua = 10.0\nsecondary_temperature = 350.0\n");
    assert!(ok.is_empty(), "{ok:?}");
    assert!(with("mode = \"ntu\"\nsecondary_temperature = 350.0\n").contains(&"components.hx.ua".to_string()));
    assert!(
        with("mode = \"ntu\"\nua = 10.0\narrangement = \"nope\"\nsecondary_temperature = 350.0\n")
            .contains(&"components.hx.arrangement".to_string())
    );
    assert!(
        with("mode = \"ntu\"\nua = 10.0\nsecondary_temperature = 350.0\nsecondary_capacity_rate = -1.0\n")
            .contains(&"components.hx.secondary_capacity_rate".to_string())
    );
    assert!(
        with("mode = \"ntu\"\nua = 10.0\nsecondary_temperature = 350.0\nloss_coefficient = -1.0\n")
            .contains(&"components.hx.loss_coefficient".to_string())
    );
    assert!(with("mode = \"ntu\"\nua = 10.0\ndual_cell = true\n")
        .iter()
        .any(|p| p.starts_with("components.hx.secondary")));
    let fan = |patch: &str, q: &str| {
        format!("[components.f]\ntype = \"fan_boundary\"\npatch = \"{patch}\"\ncurve_flow = {q}\ncurve_pressure = [10.0, 0.0]\n")
    };
    let inl = "[boundaries.xmin]\nvelocity = \"zero_gradient\"\npressure = \"fixed\"\n";
    let fd = |c: &str| diags(&case_text(0.2, inl, c, false));
    assert!(
        fd(&fan("xmin", "[0.0, 1.0]")).is_empty(),
        "{:?}",
        fd(&fan("xmin", "[0.0, 1.0]"))
    );
    assert!(fd(&fan("nope", "[0.0, 1.0]")).contains(&"components.f.patch".to_string()));
    assert!(fd(&fan("xmin", "[1.0, 0.0]")).contains(&"components.f.curve_flow".to_string()));
    let not_fixed = diags(&case_text(
        0.2,
        "[boundaries.xmin]\nvelocity = \"fixed\"\nvelocity_value = [2.0, 0.0, 0.0]\n",
        &fan("xmin", "[0.0, 1.0]"),
        false,
    ));
    assert!(not_fixed.contains(&"components.f.patch".to_string()), "{not_fixed:?}");
}
