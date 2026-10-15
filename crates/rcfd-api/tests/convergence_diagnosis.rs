//! ADR 0023 Task 1: diagnosis experiments X1-X7 for the registered residual-convergence known misses. The decision
//! rules are those registered in docs/adr/0023-wave17a-convergence.md before the first run; they are evaluated by the
//! `verdict_*` helpers below and printed, nothing is tuned. The experiments are diagnostics (they print a table and
//! the rule outcome); the assertions are only sanity checks on the machinery (finite numbers, the options default to
//! the old behaviour). Run with `cargo test --release -p rcfd-api --test convergence_diagnosis -- --ignored --nocapture`.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::simple::ResidualParts;

fn build(text: &str) -> Simulation {
    let c = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"))
}

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

const SYM_SIDES: &str = "[boundaries.ymin]\nvelocity = \"symmetry\"\n[boundaries.ymax]\nvelocity = \"symmetry\"\n";

fn duct_text(xmin: &str, xmax: &str, nu: &str, solver_extra: &str) -> String {
    box2d(
        20,
        4,
        5.0,
        1.0,
        "",
        &format!("[boundaries.xmin]\n{xmin}\n[boundaries.xmax]\n{xmax}\n{SYM_SIDES}"),
        &format!("component_diagonal = \"corrected\"\n{solver_extra}"),
    )
    .replace("nu = 0.1", &format!("nu = {nu}"))
}

const P_FIXED_0: &str = "velocity = \"zero_gradient\"\npressure = \"fixed\"\npressure_value = 0.0";
const PO: &str =
    "bc_kind = \"pressure_outlet\"\nbc_pressure = 0.5\nbackflow_pressure = \"total\"\nbackflow_direction = \"normal\"";
const TP: &str = "bc_kind = \"total_pressure_inlet\"\nbc_pressure = 0.5";
const PSEUDO: &str = "pseudo_mode = \"physical\"\npseudo_dt = 0.2";

/// The component with the largest reference residual of the last recorded iteration.
fn worst(sim: &Simulation) -> ResidualParts {
    let h = &sim.simple_solver().unwrap().parts_history;
    let last = h.last().expect("no parts recorded");
    *last.iter().max_by(|a, b| a.res_ref().total_cmp(&b.res_ref())).unwrap()
}

/// Parts of the pressure-equation residual of the last iteration.
fn pworst(sim: &Simulation) -> Option<ResidualParts> {
    sim.simple_solver()
        .unwrap()
        .pressure_parts_history
        .last()
        .copied()
        .flatten()
}

fn preport(label: &str, it: usize, sim: &Simulation) {
    if let Some(p) = pworst(sim) {
        report(&format!("{label} [pressure]"), it, &p);
    }
}

fn fields_change(sim: &Simulation, prev: &mut Vec<Vec<f64>>) -> f64 {
    let names = ["ux", "uy", "uz", "p"];
    let cur: Vec<Vec<f64>> = names.iter().map(|n| sim.field(n).unwrap()).collect();
    let mut m = 0.0_f64;
    if prev.len() == cur.len() {
        for (a, b) in cur.iter().zip(prev.iter()) {
            for (x, y) in a.iter().zip(b) {
                m = m.max((x - y).abs());
            }
        }
    } else {
        m = f64::INFINITY;
    }
    *prev = cur;
    m
}

fn report(label: &str, it: usize, p: &ResidualParts) {
    println!(
        "  {label} it {it:5}: num {:.3e}  N_ref {:.3e}  S {:.3e}  res_ref {:.3e}  res_bwd {:.3e}  N_ref/S {:.3e}",
        p.num,
        p.norm_ref,
        p.scale,
        p.res_ref(),
        p.res_bwd(),
        p.norm_ref / p.scale
    );
}

/// Iterate with parts recording until settled (change <= 1e-10 for 20 iterations) or `cap`; returns iterations used.
fn run_settle(sim: &mut Simulation, cap: usize, every: usize, label: &str) -> usize {
    sim.simple_solver_mut().unwrap().record_parts = true;
    let mut prev = Vec::new();
    let mut quiet = 0;
    for it in 1..=cap {
        sim.step();
        assert!(sim.failure().is_none(), "{label}: {:?}", sim.failure());
        let ch = fields_change(sim, &mut prev);
        assert!(ch.is_finite() || it == 1, "{label}: non-finite fields");
        if it % every == 0 || it == 1 {
            report(label, it, &worst(sim));
        }
        quiet = if ch <= 1e-10 { quiet + 1 } else { 0 };
        if quiet >= 20 {
            println!("  {label}: settled after {it} iterations (last change {ch:.2e})");
            return it;
        }
    }
    println!("  {label}: did NOT settle within {cap} iterations");
    cap
}

/// X1 rule.
fn verdict_x1(p: &ResidualParts) -> &'static str {
    if p.res_ref() >= 1e-3 && p.res_bwd() <= 1e-13 && p.norm_ref / p.scale <= 1e-9 {
        "H1 HOLDS (denominator vanishes with the iterate, numerator at round-off)"
    } else if p.res_ref() <= 1e-9 || p.res_bwd() > 1e-11 {
        "H1 REJECTED"
    } else {
        "H1 inconclusive"
    }
}

/// X2 rule.
fn verdict_x2(p: &ResidualParts) -> &'static str {
    if p.res_bwd() <= 1e-13 && p.res_ref() / p.res_bwd() >= 30.0 {
        "H1' HOLDS (round-off in the residual evaluation amplified by N_ref << S)"
    } else if p.res_bwd() >= 1e-11 {
        "H1' REJECTED (not at round-off level: genuine slow or non-contracting iteration)"
    } else {
        "inconclusive (1e-13 < res_bwd < 1e-11)"
    }
}

fn e2_text() -> String {
    r#"
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
"#
    .to_string()
}

/// X1: frictionless uniform flow, control and settled C2 / D1 / D2 set-ups.
#[test]
#[ignore = "diagnostic X1 (ADR 0023)"]
fn x1_normalisation_denominator_on_uniform_flow() {
    println!("X1 control: velocity inlet 1, static outlet 0, frictionless duct, plain simplec (ADR 0022 control)");
    let control = duct_text(
        "velocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]",
        P_FIXED_0,
        "1e-3",
        "",
    );
    let mut s = build(&control);
    s.simple_solver_mut().unwrap().record_parts = true;
    for it in 1..=1500 {
        s.step();
        if matches!(it, 1 | 10 | 100 | 500 | 1000 | 1500) {
            report("control", it, &worst(&s));
        }
    }
    let ux = s.field("ux").unwrap();
    let dev = ux.iter().map(|v| (v - 1.0).abs()).fold(0.0_f64, f64::max);
    println!("  control: max |ux - 1| = {dev:.2e}");
    println!("  X1 control verdict: {}", verdict_x1(&worst(&s)));
    for (name, xmin, xmax) in [("C2", P_FIXED_0, PO), ("D1", TP, P_FIXED_0)] {
        let mut s = build(&duct_text(xmin, xmax, "1e-3", PSEUDO));
        run_settle(&mut s, 6000, 200, name);
        let p = worst(&s);
        println!("  X1 {name} verdict: {}", verdict_x1(&p));
    }
    // D2 (30 degree inlet direction, periodic y)
    let c = (30.0_f64).to_radians().cos();
    let sn = (30.0_f64).to_radians().sin();
    let d2 = box2d(
        20,
        8,
        2.0,
        1.0,
        "periodic_y = true",
        &format!(
            "[boundaries.xmin]\nbc_kind = \"total_pressure_inlet\"\nbc_pressure = 0.5\nflow_direction = [{c:.17}, {sn:.17}, 0.0]\n[boundaries.xmax]\n{P_FIXED_0}\n"
        ),
        &format!("component_diagonal = \"corrected\"\n{PSEUDO}"),
    )
    .replace("nu = 0.1", "nu = 1e-3");
    let mut s = build(&d2);
    run_settle(&mut s, 6000, 200, "D2");
    println!("  X1 D2 verdict: {}", verdict_x1(&worst(&s)));
}

/// X2 + X3 + X4 on E2.
#[test]
#[ignore = "diagnostic X2/X3/X4 on E2 (ADR 0023), minutes"]
fn x2_x3_x4_e2_stall() {
    println!("X2 E2 as registered (linear solver controls at their defaults), 6000 iterations");
    let mut s = build(&e2_text());
    s.simple_solver_mut().unwrap().record_parts = true;
    let mut prev = Vec::new();
    let mut hist_ref: Vec<f64> = Vec::new();
    let mut changes: Vec<f64> = Vec::new();
    for it in 1..=6000 {
        s.step();
        let ch = fields_change(&s, &mut prev);
        let w = worst(&s);
        hist_ref.push(w.res_ref());
        changes.push(ch);
        if matches!(it, 1 | 100 | 500 | 1000 | 2000 | 3000 | 4000 | 5000 | 6000) {
            report("E2", it, &w);
            preport("E2", it, &s);
            println!(
                "      max field change {ch:.3e}, last summary res_u {:.3e} res_p {:.3e}",
                s.last().unwrap().res_u,
                s.last().unwrap().res_p
            );
        }
    }
    let w = worst(&s);
    println!("  X2 E2 momentum verdict: {}", verdict_x2(&w));
    if let Some(pp) = pworst(&s) {
        println!("  X2 E2 pressure verdict: {}", verdict_x2(&pp));
    }
    // X4: the change per iteration over the last 500 iterations
    let tail = &changes[changes.len() - 500..];
    let (first, last) = (
        tail[..50].iter().cloned().fold(0.0_f64, f64::max),
        tail[450..].iter().cloned().fold(0.0_f64, f64::max),
    );
    println!(
        "  X4 E2 field change per iteration: max over iterations 5501-5550 {first:.3e}, over 5951-6000 {last:.3e}, overall max in the last 500 {:.3e}",
        tail.iter().cloned().fold(0.0_f64, f64::max)
    );
    println!(
        "  X4 rule: round-off noise iff change <= 1e-13 and not monotonically decreasing; slow mode iff it decreases by > 2x: {}",
        if last <= 1e-13 && first / last.max(1e-300) <= 2.0 {
            "NOISE BAND"
        } else if first / last.max(1e-300) > 2.0 {
            "SLOW MODE (decaying)"
        } else {
            "neither (change above noise level and not decaying)"
        }
    );
    // continuity imbalance
    let rec = s.last().unwrap();
    println!("  continuity (max cell imbalance / area scale) {:.3e}", rec.continuity);
    let g0 = hist_ref[4999];
    let g1 = hist_ref[5999];
    println!("  res_ref iteration 5000 {g0:.3e} -> 6000 {g1:.3e}");

    // X3: linear solver floor
    println!("X3 E2 with tol_abs = 0, tol_rel = 1e-9, momentum cap 5000 / pressure cap 5000, 6000 iterations");
    let mut t = build(&e2_text());
    {
        let sv = t.simple_solver_mut().unwrap();
        sv.record_parts = true;
        sv.settings.momentum_solver = rcfd_linalg::SolveControl {
            tol_abs: 0.0,
            tol_rel: 1e-9,
            max_iter: 5000,
        };
        sv.settings.pressure_solver = rcfd_linalg::SolveControl {
            tol_abs: 0.0,
            tol_rel: 1e-9,
            max_iter: 5000,
        };
    }
    let mut hist3: Vec<f64> = Vec::new();
    for it in 1..=6000 {
        t.step();
        hist3.push(worst(&t).res_ref());
        if matches!(it, 1000 | 3000 | 5000 | 5500 | 6000) {
            report("E2 tight", it, &worst(&t));
        }
    }
    let (a, b) = (hist_ref[5999], hist3[5999]);
    let slope = (hist3[5999].max(1e-300).log10() - hist3[5499].max(1e-300).log10()) / 500.0;
    println!(
        "  X3: final res_ref default {a:.3e}, tight {b:.3e}, ratio default/tight {:.2}; tight last-500 log10 slope {slope:.3e}/iteration",
        a / b
    );
    println!(
        "  X3 rule: {}",
        if a / b >= 10.0 && slope < -1e-3 {
            "H2 CONFIRMED (linear-solver tolerance floor)"
        } else if (1.0 / 3.0..=3.0).contains(&(a / b)) {
            "H2 REJECTED (final residuals within a factor 3)"
        } else {
            "H2 inconclusive"
        }
    );
}

/// X5: first-iteration over-shoot from rest versus viscosity.
#[test]
#[ignore = "diagnostic X5 (ADR 0023)"]
fn x5_first_iteration_overshoot_scales_with_inverse_viscosity() {
    let mut prod = Vec::new();
    for nu in ["1e-3", "1e-2", "1e-1"] {
        let xmin = "velocity = \"zero_gradient\"\npressure = \"fixed\"\npressure_value = 0.5";
        let mut s = build(&duct_text(xmin, P_FIXED_0, nu, "alpha_u = 0.9"));
        s.step();
        let u1 = s.field("ux").unwrap().iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        let nuf: f64 = nu.parse().unwrap();
        println!(
            "X5 nu {nu}: max |u| after iteration 1 = {u1:.4e}, u1 * nu = {:.4e}",
            u1 * nuf
        );
        prod.push(u1 * nuf);
    }
    let (lo, hi) = prod
        .iter()
        .fold((f64::INFINITY, 0.0_f64), |(l, h), &v| (l.min(v), h.max(v)));
    println!(
        "  X5 rule (u1*nu constant within 15 % and |u1| >> 1 at nu 1e-3): spread {:.1} % -> {}",
        100.0 * (hi - lo) / hi,
        if (hi - lo) / hi <= 0.15 {
            "diagonal-limited step (no inertial scale), not a code defect"
        } else {
            "NOT 1/nu scaling"
        }
    );
}

/// X6: closed cavity, pressure level by p_ref, residual targets 1e-12.
#[test]
#[ignore = "diagnostic X6 (ADR 0023)"]
fn x6_closed_domain_reference_cell_has_no_floor() {
    let text = box2d(
        32,
        32,
        1.0,
        1.0,
        "",
        "[boundaries.xmin]
velocity = \"fixed\"
[boundaries.xmax]
velocity = \"fixed\"
[boundaries.ymin]
velocity = \"fixed\"
[boundaries.ymax]
velocity = \"fixed\"
velocity_value = [1.0, 0.0, 0.0]
",
        "alpha_u = 0.9",
    )
    .replace("nu = 0.1", "nu = 0.01");
    let mut s = build(&text);
    s.simple_solver_mut().unwrap().record_parts = true;
    for it in 1..=6000 {
        s.step();
        if s.is_done() {
            report("cavity", it, &worst(&s));
            preport("cavity", it, &s);
            break;
        }
        if it % 500 == 0 {
            report("cavity", it, &worst(&s));
            preport("cavity", it, &s);
        }
    }
    if let Some(pp) = pworst(&s) {
        println!("  X6 cavity pressure-equation verdict (X2 rule): {}", verdict_x2(&pp));
    }
    let l = s.last().unwrap();
    println!(
        "  X6 cavity Re 100 closed: converged {} after {} iterations, res_u {:.3e}, res_p {:.3e}; rule: H5 rejected iff res_ref < 1e-12 reached -> {}",
        s.is_converged(),
        l.index,
        l.res_u,
        l.res_p,
        if s.is_converged() { "H5 REJECTED" } else { "H5 not rejected" }
    );
}

fn nozzle_text(extra: &str) -> String {
    const GAMMA: f64 = 1.4;
    const P0: f64 = 101_325.0;
    let area_ratio = |m: f64| {
        let t = 1.0 + 0.5 * (GAMMA - 1.0) * m * m;
        (1.0 / m) * (2.0 / (GAMMA + 1.0) * t).powf((GAMMA + 1.0) / (2.0 * (GAMMA - 1.0)))
    };
    let mach_of_area_subsonic = |ar: f64| {
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
    };
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
"#,
        pm = 0.5 * (P0 + p_e)
    )
}

/// X7: nozzle (variable-density kernel). `res_bwd` is not available there (deviation from the registered rule: only the
/// flat/falling part of the rule is evaluated).
#[test]
#[ignore = "diagnostic X7 (ADR 0023)"]
fn x7_nozzle_floor_or_slow_convergence() {
    let mut s = build(&nozzle_text(""));
    let mut hist: Vec<(f64, f64)> = Vec::new();
    while !s.is_done() {
        s.step();
        let l = s.last().unwrap();
        hist.push((l.res_u, l.res_p));
        if l.index % 250 == 0 {
            println!("  nozzle it {:5}: res_u {:.3e} res_p {:.3e}", l.index, l.res_u, l.res_p);
        }
    }
    let n = hist.len();
    let (a, b) = (hist[n - 1001].0, hist[n - 1].0);
    println!(
        "  X7 res_u iteration {} -> {}: {a:.3e} -> {b:.3e} (factor {:.2}); rule: {}",
        n - 1000,
        n,
        a / b,
        if a / b >= 10.0 {
            "SLOW CONVERGENCE (more iterations would help)"
        } else if a / b < 2.0 {
            "FLAT (a floor or limit cycle)"
        } else {
            "neither (between 2x and 10x)"
        }
    );
}

/// D5b at n_r = 128 (ADR 0016 set-up), ~minutes.
#[test]
#[ignore = "diagnostic X2 on D5b n_r = 128 (ADR 0023), about 5 minutes"]
fn x2_d5b_residual_at_the_stall() {
    let text = r#"
[mesh]
generator = "cylinder_ogrid"
nx = 128
ny = 128
r_in = 1.0
r_out = 2.0
depth = 1.0
[physics]
nu = 0.1
rho = 1.0
[boundaries.cylinder]
velocity = "moving_wall"
wall_rotation_rate = 1.0
[boundaries.farfield]
velocity = "moving_wall"
wall_rotation_rate = 1.0
[solver]
algorithm = "simplec"
convection = "linear_upwind"
non_orthogonal_correctors = 1
max_iterations = 6000
tolerance_u = 1e-11
tolerance_p = 1e-11
"#;
    let mut s = build(text);
    s.simple_solver_mut().unwrap().record_parts = true;
    while !s.is_done() {
        s.step();
        let it = s.last().unwrap().index;
        if it % 500 == 0 || it == 1 {
            let l = s.last().unwrap();
            report("D5b", it, &worst(&s));
            preport("D5b", it, &s);
            println!("      summary res_u {:.3e} res_p {:.3e}", l.res_u, l.res_p);
        }
    }
    let w = worst(&s);
    report("D5b final", s.last().unwrap().index, &w);
    println!("  X2 D5b momentum verdict: {}", verdict_x2(&w));
    if let Some(pp) = pworst(&s) {
        println!("  X2 D5b pressure verdict: {}", verdict_x2(&pp));
    }
}

/// The options default to the old behaviour: a run with the keys at their defaults is bitwise the run without them.
#[test]
fn defaults_are_inert() {
    let base = duct_text(
        "velocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]",
        P_FIXED_0,
        "1e-2",
        "",
    );
    let with = duct_text(
        "velocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]",
        P_FIXED_0,
        "1e-2",
        "pseudo_control = \"off\"\nresidual_norm = \"reference\"",
    );
    let (mut a, mut b) = (build(&base), build(&with));
    for _ in 0..60 {
        a.step();
        b.step();
        let (la, lb) = (a.last().unwrap(), b.last().unwrap());
        assert_eq!(la.res_u.to_bits(), lb.res_u.to_bits());
        assert_eq!(la.res_p.to_bits(), lb.res_p.to_bits());
    }
    assert_eq!(a.field("ux").unwrap(), b.field("ux").unwrap());
    // recording the parts does not change the iteration either
    let mut c = build(&base);
    c.simple_solver_mut().unwrap().record_parts = true;
    for _ in 0..60 {
        c.step();
    }
    assert_eq!(a.field("p").unwrap(), c.field("p").unwrap());
    assert_eq!(c.simple_solver().unwrap().parts_history.len(), 60);
}

/// X1b: the post-hoc frictionless set-ups run with both normalisations against the registered targets (ADR 0023 A1).
#[test]
#[ignore = "diagnostic X1b (ADR 0023 addendum A1)"]
fn x1b_reference_versus_backward_normalisation() {
    let c = (30.0_f64).to_radians().cos();
    let sn = (30.0_f64).to_radians().sin();
    let d2 = |extra: &str| {
        box2d(
            20,
            8,
            2.0,
            1.0,
            "periodic_y = true",
            &format!(
                "[boundaries.xmin]
bc_kind = \"total_pressure_inlet\"
bc_pressure = 0.5
flow_direction = [{c:.17}, {sn:.17}, 0.0]
[boundaries.xmax]
{P_FIXED_0}
"
            ),
            &format!(
                "component_diagonal = \"corrected\"
{PSEUDO}
{extra}"
            ),
        )
        .replace("nu = 0.1", "nu = 1e-3")
    };
    for norm in ["reference", "backward"] {
        let x = format!("residual_norm = \"{norm}\"");
        let cases = [
            (
                "C2",
                duct_text(
                    P_FIXED_0,
                    PO,
                    "1e-3",
                    &format!(
                        "{PSEUDO}
{x}"
                    ),
                ),
            ),
            (
                "D1",
                duct_text(
                    TP,
                    P_FIXED_0,
                    "1e-3",
                    &format!(
                        "{PSEUDO}
{x}"
                    ),
                ),
            ),
            ("D2", d2(&x)),
        ];
        for (name, text) in cases {
            let mut s = build(&text);
            while !s.is_done() {
                s.step();
            }
            let l = s.last().unwrap();
            println!(
                "X1b {name} residual_norm {norm}: converged {} after {} iterations, res_u {:.3e}, res_p {:.3e}",
                s.is_converged(),
                l.index,
                l.res_u,
                l.res_p
            );
        }
    }
}

/// X7b: nozzle single-change variants (ADR 0023 addendum A1).
#[test]
#[ignore = "diagnostic X7b (ADR 0023 addendum A1)"]
fn x7b_nozzle_variants() {
    let variants = [
        ("baseline", ""),
        (
            "(a) momentum_rel_tol 1e-8, max_iter 2000",
            "momentum_rel_tol = 1e-8
momentum_max_iter = 2000",
        ),
        ("(b) h_operator previous_iterate", "h_operator = \"previous_iterate\""),
        ("(c) convection upwind", "convection = \"upwind\""),
        ("(d) alpha_u 0.5", "alpha_u = 0.5"),
        ("(e) pressure_rel_tol 1e-8", "pressure_rel_tol = 1e-8"),
    ];
    let mut base = 0.0;
    for (name, extra) in variants {
        // later keys override earlier ones in the toml only if not duplicated, so strip a duplicate convection line
        let mut text = nozzle_text(extra);
        if extra.starts_with("convection") {
            text = text.replacen(
                "convection = \"linear_upwind\"
",
                "",
                1,
            );
        }
        let mut s = build(&text);
        while !s.is_done() {
            s.step();
        }
        let l = s.last().unwrap();
        if name == "baseline" {
            base = l.res_u;
        }
        println!(
            "X7b {name}: converged {} after {} iterations, res_u {:.3e} (ratio to baseline {:.3}), res_p {:.3e}",
            s.is_converged(),
            l.index,
            l.res_u,
            l.res_u / base,
            l.res_p
        );
    }
}

/// Trace of the first iterations of C2 with the physical start step, with and without the controller (debugging the
/// non-finite F1b-F3b runs, ADR 0023 addendum A5).
#[test]
#[ignore = "diagnostic trace (ADR 0023 addendum A5)"]
fn trace_c2_physical_start() {
    for extra in [
        "pseudo_mode = \"physical\"\npseudo_dt = 0.2",
        "pseudo_mode = \"physical\"\npseudo_dt = 0.2\nresidual_norm = \"scaled\"",
        "pseudo_mode = \"physical\"\npseudo_dt = 0.2\npseudo_control = \"ser2\"",
        "pseudo_mode = \"physical\"\npseudo_dt = 0.2\npseudo_control = \"ser2\"\nresidual_norm = \"scaled\"",
    ] {
        println!("trace: {}", extra.replace('\n', " | "));
        let mut s = build(&duct_text(P_FIXED_0, PO, "1e-3", extra));
        for it in 1..=400 {
            s.step();
            let l = s.last().unwrap();
            let scale = s.simple_solver().unwrap().pseudo_scale_history.last().copied();
            if it <= 12 || it % 50 == 0 || !l.res_u.is_finite() || !l.res_p.is_finite() {
                println!(
                    "  it {it:4}: res_u {:.3e} res_p {:.3e} scale {:?}",
                    l.res_u, l.res_p, scale
                );
            }
            if !l.res_u.is_finite() || !l.res_p.is_finite() {
                break;
            }
        }
    }
}
