//! Zone sources and fixed values (F-201, ADR 0019 checks E1 to E6). The limits are those of the ADR, written before this
//! file was run; none was changed afterwards.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

const SLAB: &str = r#"
title = "slab"
[mesh]
generator = "box_2d"
nx = 64
ny = 2
lx = 0.1
ly = 0.02
[physics]
nu = 0.01
rho = 1.0
cp = 1000.0
conductivity = 2.0
[energy]
enabled = true
relax = 1.0
[initial]
temperature = 300.0
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 300.0
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_value = 300.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
tolerance_u = 1e-12
max_iterations = 400
[reports.q_min]
type = "heat_flow"
patch = "xmin"
[reports.q_max]
type = "heat_flow"
patch = "xmax"
"#;

fn slab_text(n: usize, extra: &str, report_conductivity: bool) -> String {
    let mut text = SLAB.replace("nx = 64", &format!("nx = {n}"));
    if report_conductivity {
        // the heat_flow report has its own conductivity (default 1.0); the physics value is 2.0
        text = text.replace("type = \"heat_flow\"", "type = \"heat_flow\"\nconductivity = 2.0");
    }
    text + extra
}

fn slab_with(n: usize, extra: &str, report_conductivity: bool) -> Case {
    Case::parse_toml(&slab_text(n, extra, report_conductivity)).unwrap_or_else(|e| panic!("{e:?}"))
}

fn slab(n: usize, extra: &str) -> Case {
    slab_with(n, extra, true)
}

fn solve(c: &Case) -> Simulation {
    let mut s = Simulation::new(c).unwrap_or_else(|e| panic!("{e:?}"));
    s.run(400);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    s
}

fn order(coarse: f64, fine: f64) -> f64 {
    (coarse / fine).log2()
}

const K: f64 = 2.0;
const L: f64 = 0.1;

fn uniform_source(n: usize, report_conductivity: bool) -> (Simulation, Vec<f64>) {
    let c = slab_with(
        n,
        "[zone_sources.q]\ntarget = \"temperature\"\nzone = \"all\"\nvalue = 1e4\n",
        report_conductivity,
    );
    let s = solve(&c);
    let x: Vec<f64> = s.geom.mesh.cell_centres().iter().map(|c| c[0]).collect();
    (s, x)
}

/// Set-up defect, kept as the record: the first version of this test left the `heat_flow` reports at their own default
/// conductivity (1.0) while the case has 2.0, so E1c compared half the heat flow. E1a and E1b passed in this version too
/// (see the ADR results); the corrected test below repeats all three with `reports.*.conductivity = 2.0`.
#[test]
#[ignore = "set-up defect (report conductivity left at its default); superseded by e1_corrected_report_conductivity"]
fn e1_uniform_source_matches_the_discrete_identity_and_converges_at_second_order() {
    e1_body(false);
}

#[test]
fn e1_corrected_report_conductivity() {
    e1_body(true);
}

fn e1_body(report_conductivity: bool) {
    let q = 1e4;
    let dt_max = q * L * L / (8.0 * K);
    // E1a: n = 64, T_h = T_exact + q h^2 / (8 k) to 1e-6 of the maximum rise
    let (s, x) = uniform_source(64, report_conductivity);
    let t = s.field("T").unwrap();
    let h = L / 64.0;
    let ident = (0..t.len())
        .map(|i| (t[i] - (300.0 + q * x[i] * (L - x[i]) / (2.0 * K)) - q * h * h / (8.0 * K)).abs())
        .fold(0.0, f64::max);
    println!("E1a identity deviation {ident:.3e} K (limit {:.3e})", 1e-6 * dt_max);
    assert!(ident <= 1e-6 * dt_max, "E1a {ident}");
    // E1b: error and order
    let mut errs = Vec::new();
    for n in [8, 16, 32, 64] {
        let (s, x) = uniform_source(n, report_conductivity);
        let t = s.field("T").unwrap();
        let e = (0..t.len())
            .map(|i| (t[i] - (300.0 + q * x[i] * (L - x[i]) / (2.0 * K))).abs())
            .fold(0.0, f64::max)
            / dt_max;
        errs.push(e);
    }
    let (o1, o2) = (order(errs[1], errs[2]), order(errs[2], errs[3]));
    println!("E1b errors {errs:?} orders {o1:.3} {o2:.3}");
    assert!(errs[3] <= 3e-4, "E1b error {}", errs[3]);
    assert!(
        (1.9..=2.1).contains(&o1) && (1.9..=2.1).contains(&o2),
        "E1b orders {o1} {o2}"
    );
    // E1c: conservation
    let vol: f64 = s.geom.mesh.cell_volumes().iter().sum();
    let out = s.monitors.last("q_min").unwrap() + s.monitors.last("q_max").unwrap();
    let rel = (out / (q * vol) - 1.0).abs();
    println!(
        "E1c heat out {out:.6e} W against q V {:.6e} W, relative {rel:.3e}",
        q * vol
    );
    assert!(rel <= 1e-6, "E1c {rel}");
}

#[test]
fn e2_linear_source_expression_converges_at_second_order() {
    let q0 = 2e4;
    let mut errs = Vec::new();
    for n in [8, 16, 32, 64] {
        let c = slab(
            n,
            "[zone_sources.q]\ntarget = \"temperature\"\nzone = \"all\"\nvalue_expression = \"2e4 [W/m^3] * x / 0.1 [m]\"\n",
        );
        let s = solve(&c);
        let t = s.field("T").unwrap();
        let x: Vec<f64> = s.geom.mesh.cell_centres().iter().map(|c| c[0]).collect();
        let exact = |x: f64| q0 * x * (L * L - x * x) / (6.0 * K * L);
        let max_exact = (0..2000).map(|i| exact(L * i as f64 / 2000.0)).fold(0.0, f64::max);
        let e = (0..t.len())
            .map(|i| (t[i] - 300.0 - exact(x[i])).abs())
            .fold(0.0, f64::max)
            / max_exact;
        errs.push(e);
    }
    let (o1, o2) = (order(errs[1], errs[2]), order(errs[2], errs[3]));
    println!("E2 relative errors {errs:?} orders {o1:.3} {o2:.3}");
    assert!(errs[3] <= 6e-4, "E2 error {}", errs[3]);
    assert!(
        (1.9..=2.1).contains(&o1) && (1.9..=2.1).contains(&o2),
        "E2 orders {o1} {o2}"
    );
}

const HOLD: &str = "[zone_sources.hold]\ntarget = \"temperature\"\nkind = \"fixed_value\"\nvalue = 500.0\n";

fn hold_case(register: bool) -> Case {
    let mut text = String::from(HOLD);
    if register {
        text.push_str("zone = \"register\"\nregister = \"hold\"\n[registers.hold]\ntype = \"box\"\n");
    } else {
        text.push_str("zone = \"box\"\n");
    }
    text.push_str("box_min = [0.045, -1.0, -1.0]\nbox_max = [0.055, 1.0, 1.0]\n");
    let mut c = slab(40, &text);
    c.set("boundaries.xmax.temperature_value", serde_json::json!(400.0))
        .unwrap();
    c
}

#[test]
fn e3_fixed_value_cells_hold_exactly_and_the_rest_is_piecewise_linear() {
    let s = solve(&hold_case(false));
    let t = s.field("T").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let fixed: Vec<usize> = (0..t.len())
        .filter(|&i| cc[i][0] >= 0.045 && cc[i][0] <= 0.055)
        .collect();
    assert_eq!(fixed.len(), 8, "four columns of two cells are fixed");
    let columns: std::collections::BTreeSet<i64> = fixed.iter().map(|&i| (cc[i][0] * 1e6).round() as i64).collect();
    assert_eq!(columns.len(), 4);
    let dev_fixed = fixed.iter().map(|&i| (t[i] - 500.0).abs()).fold(0.0, f64::max);
    println!("E3a largest deviation of a fixed cell {dev_fixed:e}");
    assert!(dev_fixed <= 0.0, "E3a {dev_fixed}");
    let h = L / 40.0;
    let (x_first, x_last) = (18.5 * h, 21.5 * h);
    let mut dev: f64 = 0.0;
    for i in 0..t.len() {
        if fixed.contains(&i) {
            continue;
        }
        let x = cc[i][0];
        let pl = if x < x_first {
            300.0 + 200.0 * x / x_first
        } else {
            400.0 + 100.0 * (L - x) / (L - x_last)
        };
        dev = dev.max((t[i] - pl).abs());
    }
    println!("E3b largest deviation from the piecewise-linear profile {dev:.3e} K (limit 2e-4)");
    assert!(dev <= 1e-6 * 200.0, "E3b {dev}");
    // E3c: the zone given as a register is identical
    let r = solve(&hold_case(true));
    let tr = r.field("T").unwrap();
    let diff = t.iter().zip(&tr).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    println!("E3c register against box: largest difference {diff:e}");
    assert!(diff <= 0.0, "E3c {diff}");
}

const BOX4: &str = r#"
title = "decay"
[mesh]
generator = "box_2d"
nx = 4
ny = 4
lx = 1.0
ly = 1.0
[physics]
nu = 0.01
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[species.a]
initial = 1.0
diffusivity = 1e-3
[species.b]
initial = 0.0
diffusivity = 1e-3
[zone_sources.decay]
target = "a"
value = 0.0
linear_coefficient = 1.0
[solver]
algorithm = "pimple"
end_time = 1.0
"#;

fn decay(dt: f64, scheme: &str) -> (f64, f64, f64) {
    let text = BOX4.to_string() + &format!("dt = {dt}\ntime_scheme = \"{scheme}\"\n");
    // the keys belong to [solver], which is the last table of the text
    let c = Case::parse_toml(&text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut s = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    let steps = s.run(1000);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    assert_eq!(steps, (1.0 / dt).round() as usize);
    let a = s.field("a").unwrap();
    let b = s.field("b").unwrap();
    let spread = a.iter().map(|v| (v - a[0]).abs()).fold(0.0, f64::max);
    let closure = a.iter().zip(&b).map(|(x, y)| (x + y - 1.0).abs()).fold(0.0, f64::max);
    (a[0], spread, closure)
}

#[test]
fn e4_species_decay_is_exp_minus_t_with_the_expected_orders() {
    let exact = (-1.0_f64).exp();
    // Euler
    let mut errs = Vec::new();
    for dt in [0.1, 0.05, 0.025] {
        let (a, spread, closure) = decay(dt, "euler");
        let n = (1.0 / dt).round() as i32;
        let identity = (a - (1.0 + dt).powi(-n)).abs();
        println!(
            "E4a Euler dt {dt}: a {a:.12}, |a - (1+dt)^-N| {identity:.3e}, spread {spread:.2e}, closure {closure:.2e}"
        );
        assert!(identity <= 1e-10, "E4a identity at dt {dt}: {identity}");
        assert!(
            closure <= 1e-12 && spread <= 1e-12,
            "E4c at dt {dt}: {closure} {spread}"
        );
        errs.push((a - exact).abs());
    }
    let (o1, o2) = (order(errs[0], errs[1]), order(errs[1], errs[2]));
    println!("E4a Euler errors {errs:?} orders {o1:.3} {o2:.3}");
    assert!(
        (0.9..=1.15).contains(&o1) && (0.9..=1.15).contains(&o2),
        "E4a orders {o1} {o2}"
    );
    // BDF2
    let mut errs = Vec::new();
    for dt in [0.1, 0.05, 0.025] {
        let (a, spread, closure) = decay(dt, "bdf2");
        println!(
            "E4b BDF2 dt {dt}: a {a:.12}, error {:.3e}, spread {spread:.2e}, closure {closure:.2e}",
            (a - exact).abs()
        );
        assert!(
            closure <= 1e-12 && spread <= 1e-12,
            "E4c at dt {dt}: {closure} {spread}"
        );
        errs.push((a - exact).abs());
    }
    let (o1, o2) = (order(errs[0], errs[1]), order(errs[1], errs[2]));
    println!("E4b BDF2 errors {errs:?} orders {o1:.3} {o2:.3}");
    assert!(
        (1.8..=2.3).contains(&o1) && (1.8..=2.3).contains(&o2),
        "E4b orders {o1} {o2}"
    );
    assert!(errs[2] <= 2e-4, "E4b error at dt 0.025: {}", errs[2]);
}

/// Set-up defect, kept as the record (see E1): the report conductivity was left at 1.0.
#[test]
#[ignore = "set-up defect (report conductivity left at its default); superseded by e5_corrected_report_conductivity"]
fn e5_source_in_a_register_conserves_energy_and_stays_symmetric() {
    e5_body(false);
}

#[test]
fn e5_corrected_report_conductivity() {
    e5_body(true);
}

fn e5_body(report_conductivity: bool) {
    let c = slab_with(
        40,
        "[registers.mid]\ntype = \"box\"\nbox_min = [0.03, -1.0, -1.0]\nbox_max = [0.07, 1.0, 1.0]\n[zone_sources.q]\ntarget = \"temperature\"\nzone = \"register\"\nregister = \"mid\"\nvalue = 1e4\n",
        report_conductivity,
    );
    let s = solve(&c);
    let cc = s.geom.mesh.cell_centres();
    let vol = s.geom.mesh.cell_volumes();
    let v_mid: f64 = (0..cc.len())
        .filter(|&i| cc[i][0] >= 0.03 && cc[i][0] <= 0.07)
        .map(|i| vol[i])
        .sum();
    let out = s.monitors.last("q_min").unwrap() + s.monitors.last("q_max").unwrap();
    let rel = (out / (1e4 * v_mid) - 1.0).abs();
    println!(
        "E5 heat out {out:.6e} W against q V_mid {:.6e} W, relative {rel:.3e}",
        1e4 * v_mid
    );
    assert!(rel <= 1e-6, "E5 conservation {rel}");
    let t = s.field("T").unwrap();
    let dt_max = t.iter().cloned().fold(0.0, f64::max) - 300.0;
    let mut asym: f64 = 0.0;
    for i in 0..t.len() {
        // mirror cell: same row, mirrored column
        let mirror = (0..t.len())
            .find(|&j| (cc[j][0] - (L - cc[i][0])).abs() < 1e-9 && (cc[j][1] - cc[i][1]).abs() < 1e-9)
            .unwrap();
        asym = asym.max((t[i] - t[mirror]).abs());
    }
    println!("E5 asymmetry {asym:.3e} K against 1e-9 * dT = {:.3e}", 1e-9 * dt_max);
    assert!(asym <= 1e-9 * dt_max, "E5 symmetry {asym}");
}

fn errors_of(text: &str) -> Vec<(String, String)> {
    let c = match Case::parse_toml(text) {
        Ok(c) => c,
        Err(d) => return d.into_iter().map(|d| (d.path, d.message)).collect(),
    };
    let mut out: Vec<(String, String)> = c.semantic_errors().into_iter().map(|d| (d.path, d.message)).collect();
    if out.is_empty() {
        if let Err(e) = Simulation::new(&c) {
            out.extend(e.into_iter().map(|d| (d.path, d.message)));
        }
    }
    out
}

#[test]
fn e6_diagnostics_are_located() {
    let with = |extra: &str| errors_of(&slab_text(16, extra, true));
    let has = |d: &[(String, String)], path: &str, word: &str| {
        assert!(
            d.iter().any(|(p, m)| p == path && m.contains(word)),
            "expected {path} / '{word}' in {d:?}"
        );
    };
    let d = with("[zone_sources.q]\ntarget = \"nonsense\"\nvalue = 1.0\n");
    has(&d, "zone_sources.q.target", "unknown target");
    // the balance species
    let d = errors_of(&BOX4.replace("target = \"a\"", "target = \"b\""));
    has(&d, "zone_sources.decay.target", "balance species");
    let d = with("[zone_sources.q]\ntarget = \"temperature\"\nvalue = 1.0\nlinear_coefficient = -1.0\n");
    has(&d, "zone_sources.q.linear_coefficient", "negative");
    let d = with(
        "[zone_sources.q]\ntarget = \"temperature\"\nkind = \"fixed_value\"\nvalue = 1.0\nlinear_coefficient = 1.0\n",
    );
    has(&d, "zone_sources.q.linear_coefficient", "fixed_value");
    let d = with(&format!(
        "{HOLD}zone = \"box\"\nbox_min = [0.045, -1.0, -1.0]\nbox_max = [0.055, 1.0, 1.0]\n[zone_sources.hold2]\ntarget = \"temperature\"\nkind = \"fixed_value\"\nvalue = 450.0\nzone = \"box\"\nbox_min = [0.04, -1.0, -1.0]\nbox_max = [0.06, 1.0, 1.0]\n"
    ));
    has(&d, "zone_sources.hold2.zone", "share cell");
    let d = with("[zone_sources.q]\ntarget = \"temperature\"\nvalue = 1.0\nzone = \"box\"\nbox_min = [5.0, 5.0, -1.0]\nbox_max = [6.0, 6.0, 1.0]\n");
    has(&d, "zone_sources.q.zone", "no cells");
    let d = with("[zone_sources.q]\ntarget = \"temperature\"\nvalue_expression = \"300 [K]\"\n");
    has(&d, "zone_sources.q.value_expression", "dimension");
    // compressible and coupled
    for alg in ["coupled", "compressible"] {
        let text = slab_text(16, "[zone_sources.q]\ntarget = \"temperature\"\nvalue = 1.0\n", true)
            .replace("algorithm = \"simplec\"", &format!("algorithm = \"{alg}\""));
        let d = errors_of(&text);
        assert!(d.iter().any(|(p, _)| p == "zone_sources.q"), "{alg}: {d:?}");
    }
}
