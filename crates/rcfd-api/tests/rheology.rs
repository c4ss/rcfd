//! Non-Newtonian and temperature-dependent viscosity laws (F-110) through the settings tree.
//!
//! Checks fixed before running:
//! * plane channel flow of a power-law fluid (n = 0.6 and n = 1.4, K = 0.1 Pa s^n, U = 1 m/s, H = 1 m,
//!   rho = 1): the velocity in the last cell column must match u(y) = U (2n+1)/(n+1)
//!   [1 - |1 - 2y/H|^((n+1)/n)] within 2 % of the centreline value, and the pressure gradient between
//!   the columns at 60 % and 90 % of the length must match G = K [U (2n+1)/n]^n (H/2)^-(n+1) within 3 %;
//! * Bingham channel flow (plastic viscosity 0.1, yield stress 0.2 Pa, regularisation 200 s): centreline
//!   (plug) velocity and pressure gradient against the Buckingham-Reiner plane solution within 5 %;
//! * a Carreau fluid with a vanishing time constant, a Cross fluid with mu_inf = mu_0 and a power law
//!   with n = 1 reproduce the Newtonian run (relative field difference < 1e-6);
//! * the same holds for a transient (PISO) run;
//! * a temperature-dependent (Arrhenius) viscosity needs the energy equation and is otherwise
//!   a case diagnostic; invalid parameters are diagnosed.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

const H: f64 = 1.0;
const L: f64 = 6.0;
const U: f64 = 1.0;

fn channel(physics: &str, rheology: &str, solver_extra: &str) -> String {
    format!(
        r#"
[mesh]
generator = "channel_2d"
nx = 60
ny = 20
lx = {L}
ly = {H}
depth = 0.1
[physics]
rho = 1.0
{physics}
[initial]
velocity = [{U}, 0.0, 0.0]
[boundaries.inlet]
velocity = "fixed"
velocity_value = [{U}, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
h_operator = "from_solution"
momentum_rel_tol = 1e-8
pressure_rel_tol = 1e-8
pressure_max_iter = 1000
max_iterations = 4000
tolerance_u = 1e-9
tolerance_p = 1e-9
{solver_extra}
[rheology]
{rheology}
"#
    )
}

fn run(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    sim.run(5000);
    assert!(sim.failure().is_none());
    sim
}

/// Cell columns: (x of the column, mean pressure, cell indices ordered by y).
fn column(sim: &Simulation, x_target: f64) -> (f64, f64, Vec<usize>) {
    let c = sim.geom.mesh.cell_centres();
    let xs: Vec<f64> = c.iter().map(|v| v[0]).collect();
    let x = xs
        .iter()
        .copied()
        .min_by(|a, b| (a - x_target).abs().partial_cmp(&(b - x_target).abs()).unwrap())
        .unwrap();
    let mut idx: Vec<usize> = (0..c.len()).filter(|&i| (xs[i] - x).abs() < 1e-9).collect();
    idx.sort_by(|&a, &b| c[a][1].partial_cmp(&c[b][1]).unwrap());
    let p = sim.field("p").unwrap();
    let pm = idx.iter().map(|&i| p[i]).sum::<f64>() / idx.len() as f64;
    (x, pm, idx)
}

fn pressure_gradient(sim: &Simulation) -> f64 {
    let (x1, p1, _) = column(sim, 0.6 * L);
    let (x2, p2, _) = column(sim, 0.9 * L);
    (p1 - p2) / (x2 - x1)
}

fn profile_error(sim: &Simulation, exact: &dyn Fn(f64) -> f64, centre: f64) -> f64 {
    let (_, _, idx) = column(sim, L - 0.01);
    let ux = sim.field("ux").unwrap();
    let c = sim.geom.mesh.cell_centres();
    idx.iter()
        .map(|&i| (ux[i] - exact(c[i][1])).abs() / centre)
        .fold(0.0, f64::max)
}

#[test]
fn power_law_channel_matches_the_analytic_profile() {
    for n in [0.6_f64, 1.4] {
        let k = 0.1;
        let g_exact = k * (U * (2.0 * n + 1.0) / n).powf(n) * (H / 2.0).powf(-(n + 1.0));
        let umax = U * (2.0 * n + 1.0) / (n + 1.0);
        let exact = |y: f64| umax * (1.0 - (1.0 - 2.0 * y / H).abs().powf((n + 1.0) / n));
        let sim = run(&channel(
            "",
            &format!("model = \"power_law\"\nconsistency_index = {k}\npower_index = {n}\nmax_viscosity = 100.0\nmin_viscosity = 1e-4\n"),
            "",
        ));
        let perr = profile_error(&sim, &exact, umax);
        let g = pressure_gradient(&sim);
        eprintln!(
            "power law n = {n}: iterations {}, profile error {:.3}% of centreline, dp/dx {g:.5} vs {g_exact:.5} ({:+.2}%)",
            sim.last().unwrap().index,
            100.0 * perr,
            100.0 * (g / g_exact - 1.0)
        );
        assert!(perr < 0.02, "n = {n}: profile error {perr}");
        assert!((g / g_exact - 1.0).abs() < 0.03, "n = {n}: dp/dx {g} vs {g_exact}");
    }
}

/// Buckingham-Reiner plane Poiseuille flow of a Bingham fluid between plates 2h apart, mean speed `u`.
fn bingham(mu: f64, tau0: f64, h: f64, u: f64) -> (f64, impl Fn(f64) -> f64) {
    let mean = |g: f64| {
        let xi = (tau0 / (g * h)).min(1.0);
        g * h * h / (3.0 * mu) * (1.0 - 1.5 * xi + 0.5 * xi.powi(3))
    };
    let (mut lo, mut hi) = (tau0 / h, 1e6);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if mean(mid) < u {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let g = 0.5 * (lo + hi);
    let yp = tau0 / g;
    let prof = move |y: f64| {
        let d = (y - h).abs();
        if d <= yp {
            g * (h - yp).powi(2) / (2.0 * mu)
        } else {
            g / (2.0 * mu) * ((h - yp).powi(2) - (d - yp).powi(2))
        }
    };
    (g, prof)
}

#[test]
fn bingham_channel_matches_buckingham_reiner() {
    let (mu, tau0) = (0.1, 0.2);
    let (g_exact, prof) = bingham(mu, tau0, H / 2.0, U);
    // the analytic profile integrates to the mean speed
    let n = 20000;
    let q: f64 = (0..n).map(|i| prof((i as f64 + 0.5) * H / n as f64)).sum::<f64>() / n as f64;
    assert!((q / U - 1.0).abs() < 1e-6, "analytic mean {q}");
    let centre = prof(H / 2.0);
    let sim = run(&channel(
        "",
        &format!("model = \"bingham\"\nconsistency_index = {mu}\nyield_stress = {tau0}\nregularisation_time = 200.0\nmax_viscosity = 1000.0\n"),
        "",
    ));
    let perr = profile_error(&sim, &prof, centre);
    let g = pressure_gradient(&sim);
    eprintln!(
        "Bingham: iterations {}, profile error {:.3}% of centreline ({centre:.4}), dp/dx {g:.5} vs {g_exact:.5} ({:+.2}%)",
        sim.last().unwrap().index,
        100.0 * perr,
        100.0 * (g / g_exact - 1.0)
    );
    assert!(perr < 0.05, "profile error {perr}");
    assert!((g / g_exact - 1.0).abs() < 0.05, "dp/dx {g} vs {g_exact}");
}

fn max_rel_diff(a: &Simulation, b: &Simulation) -> f64 {
    let mut worst: f64 = 0.0;
    for f in ["ux", "uy", "p"] {
        let (x, y) = (a.field(f).unwrap(), b.field(f).unwrap());
        let scale = x.iter().fold(1e-12_f64, |m, v| m.max(v.abs()));
        for (p, q) in x.iter().zip(&y) {
            worst = worst.max((p - q).abs() / scale);
        }
    }
    worst
}

#[test]
fn newtonian_limits_reproduce_the_plain_run() {
    let plain = run(&channel("nu = 0.1", "", ""));
    for r in [
        "model = \"carreau\"\nzero_shear_viscosity = 0.1\ninfinite_shear_viscosity = 0.0\ntime_constant = 1e-12\npower_index = 0.5\nrelax = 1.0\n",
        "model = \"cross\"\nzero_shear_viscosity = 0.1\ninfinite_shear_viscosity = 0.1\ntime_constant = 1.0\nrelax = 1.0\n",
        "model = \"power_law\"\nconsistency_index = 0.1\npower_index = 1.0\nrelax = 1.0\n",
        "model = \"carreau_yasuda\"\nzero_shear_viscosity = 0.1\ninfinite_shear_viscosity = 0.1\ntime_constant = 3.0\nyasuda_exponent = 1.5\npower_index = 0.4\nrelax = 1.0\n",
    ] {
        let sim = run(&channel("nu = 0.37", r, ""));
        let d = max_rel_diff(&sim, &plain);
        assert!(d < 1e-6, "{r}: {d:e}");
    }
    // sanity: a genuinely shear-thinning law changes the answer
    let thin = run(&channel(
        "nu = 0.1",
        "model = \"carreau\"\nzero_shear_viscosity = 0.1\ninfinite_shear_viscosity = 0.001\ntime_constant = 1.0\npower_index = 0.3\n",
        "",
    ));
    assert!(max_rel_diff(&thin, &plain) > 1e-2);
}

#[test]
fn transient_runs_use_the_law_too() {
    let tr = |physics: &str, rheology: &str| {
        let text = channel(physics, rheology, "").replace(
            "algorithm = \"simplec\"",
            "algorithm = \"piso\"\ndt = 0.02\nend_time = 0.4",
        );
        let text = text.replace("nx = 60", "nx = 20").replace("ny = 20", "ny = 8");
        let case = Case::parse_toml(&text).unwrap();
        let mut sim = Simulation::new(&case).unwrap();
        sim.run(500);
        assert!(sim.failure().is_none());
        sim
    };
    let plain = tr("nu = 0.1", "");
    let limit = tr(
        "nu = 0.9",
        "model = \"carreau\"\nzero_shear_viscosity = 0.1\ninfinite_shear_viscosity = 0.0\ntime_constant = 1e-12\nrelax = 1.0\n",
    );
    assert!(max_rel_diff(&limit, &plain) < 1e-6);
    let thin = tr(
        "nu = 0.1",
        "model = \"power_law\"\nconsistency_index = 0.1\npower_index = 0.4\nmax_viscosity = 10.0\n",
    );
    assert!(max_rel_diff(&thin, &plain) > 1e-2);
}

#[test]
fn bad_settings_are_diagnosed() {
    let diag = |rheology: &str, extra_physics: &str, want: &str| {
        let errs = Case::parse_toml(&channel(extra_physics, rheology, "")).expect_err("must be rejected");
        assert!(
            errs.iter().any(|e| e.message.contains(want)),
            "want '{want}' in {errs:?}"
        );
    };
    diag("model = \"bingham\"\nyield_stress = 0.0\n", "", "positive yield stress");
    diag(
        "model = \"carreau\"\nzero_shear_viscosity = 0.1\ninfinite_shear_viscosity = 0.5\n",
        "",
        "infinite_shear_viscosity",
    );
    diag(
        "model = \"carreau\"\nzero_shear_viscosity = 0.1\ntime_constant = 0.0\n",
        "",
        "must be positive",
    );
    diag(
        "shift = \"arrhenius\"\nactivation_temperature = 2000.0\n",
        "",
        "energy.enabled",
    );
    diag(
        "model = \"power_law\"\nmin_viscosity = 5.0\nmax_viscosity = 1.0\n",
        "",
        "larger than min_viscosity",
    );
}

#[test]
fn arrhenius_shift_scales_the_viscosity_with_the_local_temperature() {
    // Uniform T = 320 K everywhere, Newtonian base viscosity 0.1: a_T = exp(E (1/T - 1/T_ref)).
    let (e, t_ref, t) = (2000.0_f64, 300.0_f64, 320.0_f64);
    let a_t = (e * (1.0 / t - 1.0 / t_ref)).exp();
    let hot = |text: String| {
        text.replace(
            "[boundaries.inlet]\nvelocity = \"fixed\"",
            "[boundaries.inlet]\ntemperature = \"fixed\"\ntemperature_value = 320.0\nvelocity = \"fixed\"",
        )
        .replace(
            "[boundaries.walls]\nvelocity = \"fixed\"",
            "[boundaries.walls]\ntemperature = \"fixed\"\ntemperature_value = 320.0\nvelocity = \"fixed\"",
        )
        .replace("nx = 60", "nx = 20")
        .replace("ny = 20", "ny = 8")
        .replace("max_iterations = 4000", "max_iterations = 300")
        .replace(
            "[initial]",
            "[energy]\nenabled = true\nbuoyancy = false\n[initial]\ntemperature = 320.0",
        )
    };
    let shifted = run(&hot(channel(
        "nu = 0.1",
        &format!("shift = \"arrhenius\"\nactivation_temperature = {e}\nreference_temperature = {t_ref}\nrelax = 1.0\n"),
        "",
    )));
    let expected = run(&hot(channel(&format!("nu = {}", 0.1 * a_t), "", "")));
    let unshifted = run(&hot(channel("nu = 0.1", "", "")));
    let d = max_rel_diff(&shifted, &expected);
    assert!(d < 1e-6, "{d:e}");
    assert!(max_rel_diff(&shifted, &unshifted) > 1e-3);
}

#[test]
fn shift_table_matches_the_equivalent_arrhenius_factor_and_reports_gaps() {
    let (e, t_ref) = (2000.0_f64, 300.0_f64);
    let a = |t: f64| (e * (1.0 / t - 1.0 / t_ref)).exp();
    let table = format!(
        "[tables.shift]\ndata = \"T [K], a_T\\n280, {}\\n320, {}\\n360, {}\\n\"\ninputs = [\"T\"]\n",
        a(280.0),
        a(320.0),
        a(360.0)
    );
    let hot = |text: String, temp: f64| {
        text.replace(
            "[boundaries.inlet]\nvelocity = \"fixed\"",
            &format!("[boundaries.inlet]\ntemperature = \"fixed\"\ntemperature_value = {temp}\nvelocity = \"fixed\""),
        )
        .replace(
            "[boundaries.walls]\nvelocity = \"fixed\"",
            &format!("[boundaries.walls]\ntemperature = \"fixed\"\ntemperature_value = {temp}\nvelocity = \"fixed\""),
        )
        .replace("nx = 60", "nx = 20")
        .replace("ny = 20", "ny = 8")
        .replace("max_iterations = 4000", "max_iterations = 300")
        .replace(
            "[initial]",
            &format!("[energy]\nenabled = true\nbuoyancy = false\n[initial]\ntemperature = {temp}"),
        )
    };
    let rheo = "shift = \"table\"\nshift_table = \"shift\"\nrelax = 1.0\n";
    let tabled = run(&hot(channel("nu = 0.1", rheo, "") + &table, 320.0));
    let expected = run(&hot(channel(&format!("nu = {}", 0.1 * a(320.0)), "", ""), 320.0));
    let d = max_rel_diff(&tabled, &expected);
    assert!(d < 1e-6, "{d:e}");
    // 420 K lies outside the table (extrapolation = error): the run stops with a message, no panic
    let case = Case::parse_toml(&hot(channel("nu = 0.1", rheo, "") + &table, 420.0)).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    sim.run(5);
    let f = sim.failure().expect("the table gap must stop the run");
    assert!(f.contains("shift_table"), "{f}");
    // a table with the wrong columns is a diagnostic
    let bad =
        channel("nu = 0.1", rheo, "") + "[tables.shift]\ndata = \"x [m], q\\n0, 1\\n1, 2\\n\"\ninputs = [\"x\"]\n";
    let errs = Case::parse_toml(&hot(bad, 320.0)).expect_err("rejected");
    assert!(
        errs.iter().any(|e| e.message.contains("single input column T")),
        "{errs:?}"
    );
}
