//! F-301 (ADR 0223) (b) and (c): the case key `[solver.consistent_correctors] enabled` (E5) and the benchmark set, part 2
//! (rcfd-api).
//!
//! * C1 to C4: the key maps to the library option C (bit for bit against ADR 0154's harness replica), its located errors, its
//!   default (hash and written text) and a dynamic mesh with it.
//! * BK: ADR 0154's K1 and K2 with the key; BY: ADR 0069's BDF2 cylinder (128 x 100) with C; BX: the Studio examples that run
//!   transient (`cylinder-re100.toml`, `cylinder-re200.toml`) to t = 80 with and without the key. Deliverable checks (BKc, BYc,
//!   BXc) and default-decision criteria (DC6 to DC8) are separate tests over one cached computation per benchmark.
//!
//! The harness replica and the M_hf metric are copied from `w48t_cylinder_spokes.rs` (ADR 0154), the BDF2 cylinder from
//! `validation_cylinder_re100_w33b.rs` (ADR 0069). The limits were registered in ADR 0223 before this file existed.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::pressure::{boundary_values, pressure_bc};
use rcfd_fv::{FixedValue, Freestream, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{cylinder_ogrid, Grading};
use rcfd_post::integrals::wall_force_and_moment;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, OnceLock};

const KEY: &str = "solver.consistent_correctors.enabled";

fn repo_file(rel: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{rel}", env!("CARGO_MANIFEST_DIR")))
        .unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn parse(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"))
}

fn with(mut case: Case, overrides: &[(&str, Value)]) -> Case {
    for (k, v) in overrides {
        case.tree.set(k, v.clone()).unwrap_or_else(|e| panic!("{k}: {e:?}"));
    }
    case
}

/// The committed cylinder example (64 x 48, PISO, dt 1/16) with `overrides`.
fn cylinder(overrides: &[(&str, Value)]) -> Case {
    with(
        parse(&repo_file("example-projects/wind-tunnel/cases/cylinder-re100.toml")),
        overrides,
    )
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn option_c(s: &mut PimpleSettings) {
    s.consistent_correctors = true;
    s.consistent_periodic = true;
    s.consistent_body_force = true;
}

/// ADR 0154's harness replica (`w48t_cylinder_spokes.rs` `harness`, PISO with two correctors, no lag check), with the option C when
/// `consistent`.
fn harness(case: &Case, consistent: bool) -> (PimpleSolver, f64) {
    let sim = Simulation::new(case).unwrap_or_else(|e| panic!("{e:?}"));
    let geom = sim.geom.clone();
    let (ux, uy, p0) = (
        sim.field("ux").unwrap(),
        sim.field("uy").unwrap(),
        sim.field("p").unwrap(),
    );
    let n = geom.n_cells();
    let mut u = VectorField::from_fn("U", &geom, |_| [0.0; 3]);
    u.values = (0..n).map(|i| [ux[i], uy[i], 0.0]).collect();
    let uinf = [1.0, 0.0, 0.0];
    u.set_bc(&geom, "cylinder", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    u.set_bc(
        &geom,
        "farfield",
        Box::new(Freestream {
            u_ref: uinf,
            value: uinf,
            fixed_on_inflow: true,
        }),
    )
    .unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.values = p0;
    p.set_bc(
        &geom,
        "farfield",
        Box::new(Freestream {
            u_ref: uinf,
            value: [0.0; 3],
            fixed_on_inflow: false,
        }),
    )
    .unwrap();
    let mut s = PimpleSettings::piso(0.01);
    s.convection = ConvScheme::LinearUpwind;
    s.n_correctors = 2;
    (s.alpha_u, s.alpha_p) = (0.9, 1.0);
    s.amg_pressure = n > 2000;
    if consistent {
        option_c(&mut s);
    }
    let dt = case.tree.get("solver.dt").unwrap().as_f64().unwrap();
    (PimpleSolver::new(geom, u, p, s), dt)
}

// ------------------------------------------------------------------------------------------------ C1 to C4

/// ADR 0223 C1: the key maps to the library option C, bit for bit against ADR 0154's harness replica; and it acts.
#[test]
fn c1_the_key_is_the_library_option() {
    let case = cylinder(&[(KEY, json!(true))]);
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let mut plain = Simulation::new(&cylinder(&[])).unwrap();
    let (mut h, dt) = harness(&case, true);
    for _ in 0..16 {
        sim.step();
        plain.step();
        h.step(dt);
    }
    let mine = [
        h.p.values.clone(),
        h.u.values.iter().map(|v| v[0]).collect::<Vec<_>>(),
        h.u.values.iter().map(|v| v[1]).collect::<Vec<_>>(),
    ];
    let mut equal = true;
    let mut differs = false;
    for (name, hv) in ["p", "ux", "uy"].iter().zip(&mine) {
        let sv = sim.field(name).unwrap();
        let pv = plain.field(name).unwrap();
        let e = bits(hv) == bits(&sv);
        let d = bits(&pv) != bits(&sv);
        println!("C1 {name}: the key against the replica with C bit-identical {e}; against the run without the key different {d}");
        equal &= e;
        differs |= d;
    }
    assert!(!sim.diverged() && !plain.diverged(), "C1 stopped");
    assert!(equal, "C1: the key is not the library option");
    assert!(differs, "C1: the key does not act");
}

/// A transient channel with a lumped porous jump (`transient_sources.rs` T7's channel, shortened).
const LUMPED: &str = r#"
[mesh]
generator = "channel_2d"
nx = 20
ny = 2
lx = 2.0
ly = 0.2
depth = 0.1
[physics]
nu = 0.000015
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
velocity = "symmetry"
[solver]
algorithm = "pimple"
convection = "upwind"
n_outer = 3
dt = 0.01
end_time = 0.1
[components.jump]
type = "porous_jump"
zone = "box"
box_min = [1.2, -1.0, -1.0]
box_max = [1.3, 1.0, 1.0]
direction = [1.0, 0.0, 0.0]
thickness = 0.02
permeability = 1e-8
inertial_resistance = 1000.0
"#;

/// ADR 0156's low-Mach closed box (`w48v_adaptive_correctors.rs` `LOW_MACH`) without the adaptive key.
const LOW_MACH: &str = r#"
[mesh]
generator = "box_2d"
nx = 6
ny = 6
lx = 0.1
ly = 0.1
[fluid]
density = "ideal_gas"
gas_constant = 287.0
gamma = 1.4
mode = "low_mach"
reference_pressure = 101325.0
inner_iterations = 60
inner_tolerance = 1e-12
[energy]
enabled = true
relax = 1.0
heat_source = 84437.5
[initial]
temperature = 300.0
pressure = 101325.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "pimple"
time_scheme = "bdf2"
dt = 0.01
end_time = 0.1
alpha_u = 0.9
alpha_p = 1.0
momentum_rel_tol = 1e-10
momentum_max_iter = 2000
pressure_rel_tol = 1e-10
pressure_max_iter = 20000
"#;

/// ADR 0156's piston case (box_2d, piso, moving xmin) without the adaptive key.
const PISTON: &str = r#"
[mesh]
generator = "box_2d"
nx = 20
ny = 2
lx = 1.0
ly = 0.2
[physics]
nu = 0.01
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.ymin]
velocity = "zero_gradient"
[boundaries.ymax]
velocity = "zero_gradient"
[solver]
algorithm = "piso"
convection = "central"
dt = 0.005
end_time = 0.5
[dynamic_mesh]
enabled = true
moving_patches = ["xmin"]
deformation = "smoothing"
displacement_expression_x = "1 [m/s^2] * t^2"
fixed_patches = ["xmax"]
"#;

/// ADR 0223 C2: the located errors of the key; each case builds without it.
#[test]
fn c2_located_errors_of_the_key() {
    let cavity = parse(&repo_file("example-projects/quick-start/cases/cavity-re100.toml"));
    let arms: Vec<(&str, Case)> = vec![
        ("steady simplec (the quick-start cavity)", cavity),
        (
            "fractional step",
            cylinder(&[("solver.time_advance", json!("fractional_step"))]),
        ),
        (
            "non-orthogonal correctors",
            cylinder(&[("solver.non_orthogonal_correctors", json!(1))]),
        ),
        (
            "component diagonal",
            cylinder(&[("solver.component_diagonal", json!("corrected"))]),
        ),
        ("variable density (low Mach)", parse(LOW_MACH)),
        ("lumped components", parse(LUMPED)),
    ];
    for (name, case) in arms {
        assert!(
            Simulation::new(&case).is_ok(),
            "C2 {name}: does not build without the key"
        );
        let keyed = with(case, &[(KEY, json!(true))]);
        let r = Simulation::new(&keyed);
        let msg = match &r {
            Ok(_) => "built".to_string(),
            Err(d) => d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "),
        };
        println!("C2 {name}: {msg}");
        let Err(d) = r else {
            panic!("C2 {name}: built with the key")
        };
        assert!(d.iter().any(|x| x.path == KEY), "C2 {name}: {msg}");
    }
}

/// ADR 0223 C3: the key's default leaves the case hash and the written text unchanged; `enabled = true` changes the hash.
#[test]
fn c3_the_default_of_the_key() {
    let plain = cylinder(&[]);
    let off = cylinder(&[(KEY, json!(false))]);
    let on = cylinder(&[(KEY, json!(true))]);
    let (h0, h_off, h_on) = (
        plain.tree.content_hash(),
        off.tree.content_hash(),
        on.tree.content_hash(),
    );
    let (toml, js) = (plain.to_toml(), plain.to_json_string());
    println!(
        "C3: hash without the table {h0:016x}, with enabled = false {h_off:016x}, with enabled = true {h_on:016x}; the written TOML \
         names the key {}, the JSON {}",
        toml.contains("consistent_correctors"),
        js.contains("consistent_correctors")
    );
    assert_eq!(h0, h_off, "C3: enabled = false changes the hash");
    assert_ne!(h0, h_on, "C3: enabled = true leaves the hash");
    assert!(
        !toml.contains("consistent_correctors") && !js.contains("consistent_correctors"),
        "C3: the written text names the key"
    );
}

/// ADR 0223 C4: a dynamic mesh with the key builds and runs; the key acts.
#[test]
fn c4_a_dynamic_mesh_with_the_key() {
    let run = |case: &Case| {
        let mut sim = Simulation::new(case).unwrap_or_else(|e| panic!("{e:?}"));
        for _ in 0..10 {
            sim.step();
        }
        assert!(!sim.diverged(), "C4 stopped");
        ["p", "ux", "uy"]
            .iter()
            .flat_map(|n| bits(&sim.field(n).unwrap()))
            .collect::<Vec<_>>()
    };
    let plain = run(&parse(PISTON));
    let keyed = run(&with(parse(PISTON), &[(KEY, json!(true))]));
    println!(
        "C4: 10 steps with the key, fields different from the run without it {}",
        plain != keyed
    );
    assert_ne!(plain, keyed, "C4: the key does not act on the moving mesh");
}

// ------------------------------------------------------------------------------------------------ BK: ADR 0154's K1 and K2

const Q: f64 = 0.5;

/// Cells grouped into rings by centre radius and ordered by angle (copied from `w48t_cylinder_spokes.rs`).
fn rings(centres: &[[f64; 3]]) -> Vec<(f64, Vec<usize>)> {
    let r: Vec<f64> = centres.iter().map(|c| c[0].hypot(c[1])).collect();
    let mut order: Vec<usize> = (0..r.len()).collect();
    order.sort_by(|&a, &b| r[a].total_cmp(&r[b]));
    let mut groups: Vec<Vec<usize>> = vec![vec![order[0]]];
    for w in order.windows(2) {
        if r[w[1]] - r[w[0]] > 1e-9 * r[w[1]] {
            groups.push(Vec::new());
        }
        groups.last_mut().unwrap().push(w[1]);
    }
    let theta = |c: usize| {
        centres[c][1]
            .atan2(centres[c][0])
            .rem_euclid(2.0 * std::f64::consts::PI)
    };
    groups
        .into_iter()
        .map(|mut g| {
            g.sort_by(|&a, &b| theta(a).total_cmp(&theta(b)));
            (g.iter().map(|&c| r[c]).sum::<f64>() / g.len() as f64, g)
        })
        .collect()
}

/// M_hf of ADR 0154 (copied from `w48t_cylinder_spokes.rs`).
fn m_hf(p: &[f64], centres: &[[f64; 3]]) -> f64 {
    let (mut sum, mut cnt) = (0.0, 0usize);
    for (rj, g) in rings(centres) {
        if !(0.55..=0.70).contains(&rj) {
            continue;
        }
        let n = g.len();
        assert!(n % 8 == 0, "ring of {n} cells");
        let mut hp = 0.0;
        for k in n / 8..=n / 2 {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, &c) in g.iter().enumerate() {
                let a = -2.0 * std::f64::consts::PI * (k * i % n) as f64 / n as f64;
                re += p[c] * a.cos();
                im += p[c] * a.sin();
            }
            hp += if k == n / 2 { 1.0 } else { 2.0 } * (re * re + im * im);
        }
        sum += hp / n as f64;
        cnt += n;
    }
    assert!(cnt > 0, "no ring in [0.55, 0.70]");
    (sum / cnt as f64).sqrt() / Q
}

const K_LIMIT: f64 = 0.0114;

/// One K run with the key: (stopped by the guard, time reached, M_hf at the end, steps, seconds).
type KRun = (bool, f64, f64, usize, f64);

fn k_run(n_correctors: u32) -> KRun {
    let case = cylinder(&[
        ("mesh.nx", json!(64)),
        ("mesh.ny", json!(256)),
        ("solver.dt", json!(1.0 / 64.0)),
        ("solver.n_correctors", json!(n_correctors)),
        ("solver.end_time", json!(3.0)),
        (KEY, json!(true)),
    ]);
    let t0 = std::time::Instant::now();
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let mut steps = 0;
    while !sim.is_done() {
        sim.step();
        steps += 1;
    }
    let cc = sim.geom.mesh.cell_centres().to_vec();
    let p = sim.field("p").unwrap();
    let m = if p.iter().all(|x| x.is_finite()) {
        m_hf(&p, &cc)
    } else {
        f64::INFINITY
    };
    (sim.diverged(), sim.time(), m, steps, t0.elapsed().as_secs_f64())
}

fn bk() -> &'static [KRun; 2] {
    static BK: OnceLock<[KRun; 2]> = OnceLock::new();
    BK.get_or_init(|| {
        let r = [k_run(2), k_run(3)];
        for (name, k) in ["K1", "K2"].iter().zip(&r) {
            println!(
                "BK {name} with the key: {} steps, stopped by the guard {}, t = {}, M_hf {:.4e} (limit {K_LIMIT}), {:.0} s",
                k.3, k.0, k.1, k.2, k.4
            );
        }
        r
    })
}

/// ADR 0223 BKc: both K runs made with the key.
#[test]
#[ignore = "heavy (ADR 0223 BK: two 16,384-cell runs to t = 3)"]
fn bkc_the_k_runs_with_the_key() {
    let r = bk();
    assert!(r.iter().all(|k| k.3 > 0), "BKc");
}

/// ADR 0223 DC6 (default-decision criterion): K1 and K2 of ADR 0154 are clean with the key.
#[test]
#[ignore = "heavy (ADR 0223 BK: two 16,384-cell runs to t = 3)"]
fn dc6_k1_k2_with_the_key_are_clean() {
    for (name, k) in ["K1", "K2"].iter().zip(bk()) {
        assert!(!k.0 && (k.1 - 3.0).abs() < 1e-9, "DC6 {name}: stopped at t = {}", k.1);
        assert!(k.2 <= K_LIMIT, "DC6 {name}: M_hf {:.4e} > {K_LIMIT}", k.2);
    }
}

// ------------------------------------------------------------------------------------------------ BY: ADR 0069's BDF2 cylinder

const DEPTH: f64 = 0.1;
const NU: f64 = 0.01;

/// The set-up of `validation_cylinder_re100_w33b.rs` (128 x 100, r_out 20, Expansion(150), BDF2, PIMPLE n_outer 2, alpha 0.9,
/// linear upwind) with the option C.
fn by_setup() -> PimpleSolver {
    let mesh = Arc::new(cylinder_ogrid(128, 100, 0.5, 20.0, Grading::Expansion(150.0), DEPTH).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let uinf = [1.0, 0.0, 0.0];
    let mut u = VectorField::from_fn("U", &geom, |c| {
        let blob = 0.1 * (-((c[0] - 2.0).powi(2) + (c[1] - 0.3).powi(2))).exp();
        [1.0, blob, 0.0]
    });
    u.set_bc(&geom, "cylinder", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    u.set_bc(
        &geom,
        "farfield",
        Box::new(Freestream {
            u_ref: uinf,
            value: uinf,
            fixed_on_inflow: true,
        }),
    )
    .unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(
        &geom,
        "farfield",
        Box::new(Freestream {
            u_ref: uinf,
            value: [0.0; 3],
            fixed_on_inflow: false,
        }),
    )
    .unwrap();
    let mut st = PimpleSettings::piso(NU);
    st.time_scheme = TimeScheme::Bdf2;
    st.convection = ConvScheme::LinearUpwind;
    st.n_outer = 2;
    st.amg_pressure = true;
    st.alpha_u = 0.9;
    st.alpha_p = 0.9;
    option_c(&mut st);
    PimpleSolver::new(geom, u, p, st)
}

fn nearest(s: &PimpleSolver, x: [f64; 3]) -> usize {
    let c = s.geom.mesh.cell_centres();
    (0..c.len())
        .min_by(|&a, &b| {
            let d = |i: usize| (c[i][0] - x[0]).powi(2) + (c[i][1] - x[1]).powi(2);
            d(a).total_cmp(&d(b))
        })
        .unwrap()
}

fn drag(s: &PimpleSolver) -> f64 {
    let pbc = pressure_bc(&s.geom, &s.u, &s.p);
    let pf = boundary_values(&s.geom, &pbc.coeffs, &s.p.values);
    let (f, _) = wall_force_and_moment(&s.geom, "cylinder", &s.u, &pf, NU, 1.0, vec3::ZERO).unwrap();
    f.total()[0] / (0.5 * DEPTH)
}

/// Upward zero crossings (linear interpolation) of `v - mean`.
fn crossings(t: &[f64], v: &[f64]) -> Vec<f64> {
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    let mut cross = Vec::new();
    for i in 1..v.len() {
        if v[i - 1] - mean < 0.0 && v[i] - mean >= 0.0 {
            let w = (mean - v[i - 1]) / (v[i] - v[i - 1]);
            cross.push(t[i - 1] + w * (t[i] - t[i - 1]));
        }
    }
    cross
}

/// The trapezoid mean of `v(t)` over `[ta, tb]`.
fn window_mean(t: &[f64], v: &[f64], ta: f64, tb: f64) -> f64 {
    let (mut integral, mut span) = (0.0, 0.0);
    for i in 1..t.len() {
        let (a, b) = (t[i - 1].max(ta), t[i].min(tb));
        if b > a {
            integral += 0.5 * (v[i - 1] + v[i]) * (b - a);
            span += b - a;
        }
    }
    integral / span
}

/// BY's outcome: Ok((crossings, St, C_D)) or the stop.
type ByRun = Result<(usize, f64, f64), String>;

fn by() -> &'static ByRun {
    static BY: OnceLock<ByRun> = OnceLock::new();
    BY.get_or_init(|| {
        let dt = 0.05;
        let mut s = by_setup();
        let probe = nearest(&s, [3.0, 0.0, DEPTH / 2.0]);
        let (mut t, mut v, mut tc, mut cd) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let t0 = std::time::Instant::now();
        let mut p_iters = 0usize;
        while s.time < 160.0 {
            let info = s.step(dt);
            p_iters += info.p_iters;
            if s.diverged.is_some() || !(info.courant.is_finite() && info.courant < 1e4) {
                let r = Err(format!(
                    "stopped at t = {:.2}: {:?}",
                    s.time,
                    s.diverged.as_ref().map(|d| d.message(None))
                ));
                println!("BY: {r:?}");
                return r;
            }
            if s.time > 60.0 {
                t.push(s.time);
                v.push(s.u.values[probe][1]);
            }
            if s.time > 85.0 {
                tc.push(s.time);
                cd.push(drag(&s));
            }
        }
        let cross = crossings(&t, &v);
        let k = cross.len();
        if k <= 8 {
            return Err(format!("only {k} crossings"));
        }
        let (ta, tb) = (cross[k - 9], cross[k - 1]);
        let st = 8.0 / (tb - ta);
        let cd_mean = window_mean(&tc, &cd, ta, tb);
        println!(
            "BY C: {k} crossings, St {st:.5}, C_D {cd_mean:.5} (window {ta:.1}..{tb:.1}), pressure iterations {p_iters} over {} steps, \
             {:.0} s (ADR 0069 default: St 0.16480, C_D 1.35247)",
            (160.0 / dt) as usize,
            t0.elapsed().as_secs_f64()
        );
        Ok((k, st, cd_mean))
    })
}

/// ADR 0223 BYc: the BDF2 cylinder run with C made, its numbers or its stop recorded.
#[test]
#[ignore = "heavy (ADR 0223 BY: 12,800 cells to t = 160, about 5 to 15 minutes)"]
fn byc_the_bdf2_cylinder_with_the_option() {
    println!("BYc: {:?}", by());
}

/// ADR 0223 DC7 (default-decision criterion): ADR 0069's K1b, K2b and K4 with the option; T1 printed.
#[test]
#[ignore = "heavy (ADR 0223 BY: 12,800 cells to t = 160, about 5 to 15 minutes)"]
fn dc7_the_bdf2_cylinder_meets_k1b_k2b_k4_with_the_option() {
    let (k, st, cd) = by().clone().unwrap_or_else(|e| panic!("DC7: {e}"));
    let t1 = (0.16339..=0.16733).contains(&st) && (cd - 1.3437).abs() <= 0.0101;
    println!(
        "DC7: K1b {} K2b {} K4 {} (T1 {t1})",
        (0.16005..=0.16995).contains(&st),
        (1.2883..=1.3957).contains(&cd),
        k > 9
    );
    assert!(k > 9, "DC7 K4: {k} crossings");
    assert!((0.16005..=0.16995).contains(&st), "DC7 K1b: St {st}");
    assert!((1.2883..=1.3957).contains(&cd), "DC7 K2b: C_D {cd}");
}

// ------------------------------------------------------------------------------------------------ BX: the Studio examples

/// One BX run: (example, with the key, time reached, stopped, upward crossings of cl after t = 50, St, mean cd, seconds).
type BxRun = (&'static str, bool, f64, bool, usize, f64, f64, f64);

fn bx_run(example: &'static str, keyed: bool) -> BxRun {
    bx_run_from(example, keyed, 50.0)
}

/// [`bx_run`] with the window from `t_from` (ADR 0223 amendment 3: 40).
fn bx_run_from(example: &'static str, keyed: bool, t_from: f64) -> BxRun {
    let mut case = parse(&repo_file(&format!(
        "example-projects/wind-tunnel/cases/{example}.toml"
    )));
    if keyed {
        case = with(case, &[(KEY, json!(true))]);
    }
    let t0 = std::time::Instant::now();
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let (mut t, mut cl, mut cd) = (Vec::new(), Vec::new(), Vec::new());
    while !sim.is_done() {
        sim.step();
        if sim.diverged() {
            break;
        }
        let vals = sim.step_report_values();
        let get = |n: &str| vals.iter().find(|(k, _)| k == n).map(|(_, v)| *v);
        if let (Some(l), Some(d)) = (get("cl"), get("cd")) {
            if sim.time() >= t_from {
                t.push(sim.time());
                cl.push(l);
                cd.push(d);
            }
        }
    }
    let cross = if t.len() > 1 { crossings(&t, &cl) } else { Vec::new() };
    let k = cross.len();
    let (st, cd_mean) = if k >= 5 {
        let (ta, tb) = (cross[k - 5], cross[k - 1]);
        (4.0 / (tb - ta), window_mean(&t, &cd, ta, tb))
    } else {
        (f64::NAN, f64::NAN)
    };
    let r = (
        example,
        keyed,
        sim.time(),
        sim.diverged(),
        k,
        st,
        cd_mean,
        t0.elapsed().as_secs_f64(),
    );
    println!(
        "BX {example} {} (window from t = {t_from}): t = {:.4}, stopped {}, {k} crossings of cl, St {st:.5}, mean cd {cd_mean:.5}, {:.0} s",
        if keyed { "C (key)" } else { "D" },
        r.2,
        r.3,
        r.7
    );
    r
}

fn bx() -> &'static Vec<BxRun> {
    static BX: OnceLock<Vec<BxRun>> = OnceLock::new();
    BX.get_or_init(|| bx_all(50.0))
}

/// BX2 of ADR 0223 amendment 3: the window from t = 40.
fn bx2() -> &'static Vec<BxRun> {
    static BX2: OnceLock<Vec<BxRun>> = OnceLock::new();
    BX2.get_or_init(|| bx_all(40.0))
}

fn bx_all(t_from: f64) -> Vec<BxRun> {
    {
        let jobs: Vec<(&'static str, bool)> = vec![
            ("cylinder-re100", false),
            ("cylinder-re100", true),
            ("cylinder-re200", false),
            ("cylinder-re200", true),
        ];
        let out = Mutex::new(Vec::new());
        std::thread::scope(|sc| {
            for (e, k) in jobs {
                let out = &out;
                sc.spawn(move || {
                    let r = bx_run_from(e, k, t_from);
                    out.lock().unwrap().push(r);
                });
            }
        });
        let mut v = out.into_inner().unwrap();
        v.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        v
    }
}

/// ADR 0223 BXc as registered: every BX run reaches t = 80 with at least 5 upward crossings of cl after t = 50. Defective set-up
/// (ADR 0223 amendment 3): the Re 100 example has 4 crossings in that window in both arms. Corrected by `bxc2_...`.
#[test]
#[ignore = "defective set-up: the window t >= 50 holds 4 crossings of the Re 100 example in both arms (ADR 0223 amendment 3)"]
fn bxc_the_studio_examples_ran_with_and_without_the_key_defective_setup() {
    bxc_check(bx());
}

/// ADR 0223 amendment 3, BXc2: BXc with the window from t = 40.
#[test]
#[ignore = "heavy (ADR 0223 BX2: four 1280-step runs of the examples)"]
fn bxc2_the_studio_examples_ran_with_and_without_the_key() {
    bxc_check(bx2());
}

fn bxc_check(runs: &[BxRun]) {
    for r in runs {
        assert!(!r.3 && r.2 >= 80.0 - 1e-9 && r.4 >= 5, "BXc {r:?}");
    }
    assert_eq!(runs.len(), 4, "BXc runs");
}

/// ADR 0223 DC8 as registered (default-decision criterion): the key changes the examples' St and mean cd by at most 1 %. Not
/// evaluable for Re 100 (ADR 0223 amendment 3); corrected by `dc8b_...`.
#[test]
#[ignore = "defective set-up: not evaluable for the Re 100 example in the window t >= 50 (ADR 0223 amendment 3)"]
fn dc8_the_key_does_not_change_the_examples_answers_defective_setup() {
    dc8_check(bx());
}

/// ADR 0223 amendment 3, DC8b (default-decision criterion): DC8 with the window from t = 40.
#[test]
#[ignore = "heavy (ADR 0223 BX2: four 1280-step runs of the examples)"]
fn dc8b_the_key_does_not_change_the_examples_answers() {
    dc8_check(bx2());
}

fn dc8_check(runs: &[BxRun]) {
    for e in ["cylinder-re100", "cylinder-re200"] {
        let d = runs.iter().find(|r| r.0 == e && !r.1).unwrap();
        let c = runs.iter().find(|r| r.0 == e && r.1).unwrap();
        let (ds, dc) = ((c.5 - d.5).abs() / d.5, (c.6 - d.6).abs() / d.6);
        println!(
            "DC8 {e}: St D {:.5} C {:.5} ({ds:.2e}); cd D {:.5} C {:.5} ({dc:.2e}); limits 1e-2",
            d.5, c.5, d.6, c.6
        );
        assert!(ds <= 0.01 && dc <= 0.01, "DC8 {e}: St {ds}, cd {dc}");
    }
}
