//! ADR 0081 part L: the opt-in Barth-Jespersen-limited gradient of `linear_upwind` (`solver.linear_upwind_limiter`). Checks L0a, L0c, L1a, L1b,
//! L2a-L2c and L3; limits are the registered ones. L1a, L1b and L2c run the 13 332-cell FAN-01 / fan-sector meshes for 300 iterations and are
//! ignored as heavy (run once for the record, ADR 0081 addendum).

mod w35b_common;

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3;
use std::time::Instant;

const BJ: &str = "linear_upwind_limiter = \"barth_jespersen\"\n";

/// Insert `lines` at the top of the `[solver]` table of `text`.
fn with_solver(text: &str, lines: &str) -> String {
    text.replacen("[solver]\n", &format!("[solver]\n{lines}"), 1)
}

fn sim_of(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"))
}

fn max_speed(v: &[[f64; 3]]) -> f64 {
    v.iter()
        .map(|u| vec3::norm(*u))
        .fold(0.0f64, |a, s| if s.is_nan() { f64::INFINITY } else { a.max(s) })
}

fn boundary_speed(sim: &Simulation) -> f64 {
    let g = &sim.geom;
    let ub = sim.velocity().boundary_values(g);
    (0..ub.len())
        .filter(|&i| !g.empty[g.n_internal + i])
        .map(|i| vec3::norm(ub[i]))
        .fold(0.0, f64::max)
}

/// The ADR 0077 part B criterion: 300 iterations; STABLE iff no guard trip, max cell speed over 10..300 <= 3 x the largest boundary-face speed,
/// continuity finite with its maximum over 200..300 not above its value at 10.
fn classify(name: &str, text: &str) -> bool {
    let t0 = Instant::now();
    let mut sim = sim_of(text);
    let ub = boundary_speed(&sim);
    let (mut umax, mut c10, mut c_late, mut cont_ok) = (0.0f64, f64::NAN, 0.0f64, true);
    let mut why = String::new();
    for it in 1..=300 {
        let s = sim.step();
        if let Some(d) = sim.steady_divergence() {
            why = format!("guard trip at {it}: {} {}", d.kind, d.quantity);
            break;
        }
        if it >= 10 {
            umax = umax.max(max_speed(&sim.velocity().values));
        }
        if !s.continuity.is_finite() {
            cont_ok = false;
        }
        if it == 10 {
            c10 = s.continuity;
        }
        if it >= 200 {
            c_late = c_late.max(s.continuity);
        }
    }
    let stable = why.is_empty() && umax <= 3.0 * ub && cont_ok && c_late <= c10;
    println!(
        "{name}: {} cells, {} (max speed 10..300 {umax:.4e} vs 3 x {ub:.4e}; continuity at 10 {c10:.3e}, max 200..300 {c_late:.3e}; last {:?}){} [{:.1} s]",
        sim.geom.n_cells(),
        if stable { "STABLE" } else { "UNSTABLE" },
        sim.last(),
        if why.is_empty() { String::new() } else { format!("; {why}") },
        t0.elapsed().as_secs_f64()
    );
    stable
}

const CAVITY_SIMPLE: &str = "[mesh]\ngenerator = \"cavity_2d\"\nnx = 20\nny = 20\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"simple\"\nconvection = \"linear_upwind\"\nmax_iterations = 60\n";

#[test]
fn l0a_key_absent_and_none_are_bit_identical() {
    let run = |text: &str| {
        let mut sim = sim_of(text);
        for _ in 0..60 {
            sim.step();
        }
        let bits = |v: Vec<f64>| v.into_iter().map(f64::to_bits).collect::<Vec<_>>();
        (
            bits(sim.field("ux").unwrap()),
            bits(sim.field("uy").unwrap()),
            bits(sim.field("p").unwrap()),
            bits(sim.flux().to_vec()),
        )
    };
    assert!(
        run(CAVITY_SIMPLE) == run(&with_solver(CAVITY_SIMPLE, "linear_upwind_limiter = \"none\"\n")),
        "L0a: fields differ"
    );
}

#[test]
fn l0c_the_key_outside_its_scope_is_a_located_error() {
    let located = |text: &str, what: &str| {
        let err = match Case::parse_toml(text) {
            Err(e) => e,
            Ok(case) => Simulation::new(&case).map(|_| ()).expect_err(what),
        };
        println!("L0c {what}: {err:?}");
        assert!(
            err.iter().any(|d| d.path == "solver.linear_upwind_limiter"),
            "L0c {what}: {err:?}"
        );
    };
    let base = with_solver(CAVITY_SIMPLE, BJ);
    located(
        &base.replace("convection = \"linear_upwind\"", "convection = \"upwind\""),
        "upwind",
    );
    located(
        &base.replace(
            "algorithm = \"simple\"",
            "algorithm = \"pimple\"\ndt = 0.01\nend_time = 0.05",
        ),
        "pimple",
    );
    located(
        &base.replace("algorithm = \"simple\"", "algorithm = \"coupled\""),
        "coupled",
    );
    located(&base.replace("barth_jespersen", "minmod"), "unknown value");
    // in scope: accepted
    sim_of(&base);
}

#[test]
#[ignore = "heavy (FAN-01, 13 332 cells, 300 iterations); ADR 0081 L1a"]
fn l1a_fan01_laminar_limited_linear_upwind_is_stable() {
    assert!(classify(
        "L1a FAN-01 laminar linear_upwind + BJ",
        &with_solver(&w35b_common::laminar("linear_upwind"), BJ)
    ));
}

#[test]
#[ignore = "heavy (FAN-01 SST, 13 332 cells, 300 iterations); ADR 0081 L1b"]
fn l1b_fan01_sst_limited_linear_upwind_is_stable() {
    assert!(classify(
        "L1b FAN-01 SST linear_upwind + BJ",
        &with_solver(&w35b_common::sst("linear_upwind"), BJ)
    ));
}

const CAV32: &str = "[mesh]\ngenerator = \"cavity_2d\"\nnx = 32\nny = 32\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"simple\"\nalpha_u = 0.7\nalpha_p = 0.3\nconvection = \"linear_upwind\"\nmax_iterations = 3000\ntolerance_u = 1e-6\ntolerance_p = 1e-6\n";

/// Run to convergence or the cap; (simulation, iterations).
fn converge(name: &str, text: &str, cap: usize) -> Simulation {
    let t0 = Instant::now();
    let mut sim = sim_of(text);
    let n = sim.run(cap);
    println!(
        "{name}: {} cells, {n} iterations, converged {}, trip {:?}, last {:?} [{:.1} s]",
        sim.geom.n_cells(),
        sim.is_converged(),
        sim.steady_divergence().map(|d| (d.iteration, d.kind)),
        sim.last(),
        t0.elapsed().as_secs_f64()
    );
    assert!(sim.steady_divergence().is_none(), "{name}: guard trip");
    assert!(sim.is_converged(), "{name}: not converged within {cap}");
    sim
}

#[test]
fn l2a_cavity_converges_with_the_limiter() {
    converge("L2a cavity 32 + BJ", &with_solver(CAV32, BJ), 3000);
}

/// The ADR 0008 M4 set-up (`mrf.rs`): Taylor-Couette across the non-conformal interface, MRF lower block.
fn m4_text() -> String {
    let wall = |name: &str, w: f64| {
        format!(
            "[boundaries.{name}]\nvelocity = \"fixed\"\nvelocity_expression_x = \"-{w:?} [m/s] * y / 1 [m]\"\nvelocity_expression_y = \"{w:?} [m/s] * x / 1 [m]\"\n"
        )
    };
    format!(
        "[mesh]\ngenerator = \"annulus_3d_interface\"\nnx = 48\nny = 16\nnz = 2\nny_right = 11\nnz_right = 3\nr_in = 0.5\nr_out = 1.0\nlz = 0.4\n[physics]\nnu = 0.1\nrho = 1.0\n{}{}[boundaries.zmin]\nvelocity = \"symmetry\"\n[boundaries.zmax]\nvelocity = \"symmetry\"\n[solver]\nalgorithm = \"simplec\"\nconvection = \"linear_upwind\"\nnon_orthogonal_correctors = 1\nmax_iterations = 4000\ntolerance_u = 1e-10\ntolerance_p = 1e-10\n[sources.rotor]\ntype = \"mrf\"\nrotation_axis = [0.0, 0.0, 1.0]\nangular_velocity = 1.0\nzone = \"box\"\nbox_min = [-2.0, -2.0, -1.0]\nbox_max = [2.0, 2.0, 0.2]\n",
        wall("inner", 1.0),
        wall("outer", 0.0)
    )
}

/// Largest |u_theta - analytic| over the cells (Taylor-Couette, r_i 0.5, r_o 1, inner wall speed 0.5, outer at rest).
fn tc_error(sim: &Simulation) -> f64 {
    let (ri, ro) = (0.5f64, 1.0f64);
    let d = ro * ro - ri * ri;
    let exact = |r: f64| -ri * ri / d * r + ri * ri * ro * ro / d / r;
    let cc = sim.geom.mesh.cell_centres();
    let (ux, uy) = (sim.field("ux").unwrap(), sim.field("uy").unwrap());
    (0..cc.len())
        .map(|c| {
            let (x, y) = (cc[c][0], cc[c][1]);
            let r = x.hypot(y);
            ((-y * ux[c] + x * uy[c]) / r - exact(r)).abs()
        })
        .fold(0.0, f64::max)
}

#[test]
fn l2b_mrf_taylor_couette_interface_converges_and_meets_m4() {
    let lim = converge("L2b M4 annulus + BJ", &with_solver(&m4_text(), BJ), 4000);
    let e = tc_error(&lim) / 0.5;
    println!("L2b: max |u_theta - analytic| = {e:.4e} V_i (limit 2e-2)");
    assert!(e <= 2e-2, "L2b: {e}");
}

#[test]
#[ignore = "heavy (fan sector, 13 332 cells, 300 iterations); ADR 0081 L2c"]
fn l2c_axial_fan_sector_is_stable_with_the_limiter() {
    let fan = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../cases/axial_fan_sector/axial_fan_sector.toml"
    ))
    .unwrap()
    .replace(
        "convection = \"upwind\"",
        &format!("convection = \"linear_upwind\"\n{BJ}"),
    );
    assert!(classify("L2c axial_fan_sector linear_upwind + BJ", &fan));
}

fn lambda() -> f64 {
    let re = 40.0f64;
    re / 2.0 - (re * re / 4.0 + 4.0 * std::f64::consts::PI.powi(2)).sqrt()
}

/// Kovasznay flow, Re 40, on [-0.5, 1] x [-0.5, 1.5] (`box_2d` 1.5 x 2 shifted by 0.5): exact velocity on all sides and as the initial field.
fn kovasznay(nx: usize, extra: &str) -> String {
    let l = lambda();
    let c = l / (2.0 * std::f64::consts::PI);
    let ux = format!("1 [m/s] - 1 [m/s] * exp({l:?} [1/m] * (x - 0.5 [m])) * cos(2 * pi * (y - 0.5 [m]) / 1 [m])");
    let uy = format!("{c:?} [m/s] * exp({l:?} [1/m] * (x - 0.5 [m])) * sin(2 * pi * (y - 0.5 [m]) / 1 [m])");
    let mut b = String::new();
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        b.push_str(&format!(
            "[boundaries.{p}]\nvelocity = \"fixed\"\nvelocity_expression_x = \"{ux}\"\nvelocity_expression_y = \"{uy}\"\n"
        ));
    }
    format!(
        "[mesh]\ngenerator = \"box_2d\"\nnx = {nx}\nny = {}\nlx = 1.5\nly = 2.0\n[physics]\nnu = 0.025\n[initial]\nvelocity_expression_x = \"{ux}\"\nvelocity_expression_y = \"{uy}\"\n{b}[solver]\n{extra}algorithm = \"simple\"\nalpha_u = 0.7\nalpha_p = 0.3\nconvection = \"linear_upwind\"\nmax_iterations = 20000\ntolerance_u = 1e-10\ntolerance_p = 1e-10\n",
        nx * 4 / 3
    )
}

/// (volume-weighted RMS, max) of the velocity-vector error against the exact Kovasznay field at the cell centres.
fn kovasznay_error(sim: &Simulation) -> (f64, f64) {
    let l = lambda();
    let tp = 2.0 * std::f64::consts::PI;
    let cc = sim.geom.mesh.cell_centres();
    let vol = sim.geom.mesh.cell_volumes();
    let (ux, uy) = (sim.field("ux").unwrap(), sim.field("uy").unwrap());
    let (mut s, mut v, mut linf) = (0.0, 0.0, 0.0f64);
    for c in 0..cc.len() {
        let (x, y) = (cc[c][0] - 0.5, cc[c][1] - 0.5);
        let e = [
            1.0 - (l * x).exp() * (tp * y).cos(),
            l / tp * (l * x).exp() * (tp * y).sin(),
        ];
        let d2 = (ux[c] - e[0]).powi(2) + (uy[c] - e[1]).powi(2);
        s += vol[c] * d2;
        v += vol[c];
        linf = linf.max(d2.sqrt());
    }
    ((s / v).sqrt(), linf)
}

#[test]
fn l3_kovasznay_order_with_the_limiter() {
    let study = |extra: &str, label: &str| -> Vec<(f64, f64)> {
        [12usize, 24, 48]
            .iter()
            .map(|&nx| {
                let t0 = Instant::now();
                let mut sim = sim_of(&kovasznay(nx, extra));
                let n = sim.run(20000);
                let e = kovasznay_error(&sim);
                println!(
                    "L3 {label} {nx} x {}: {n} iterations, converged {}, last {:?}, L2 {:.4e}, Linf {:.4e} [{:.1} s]",
                    nx * 4 / 3,
                    sim.is_converged(),
                    sim.last(),
                    e.0,
                    e.1,
                    t0.elapsed().as_secs_f64()
                );
                e
            })
            .collect()
    };
    let lim = study(BJ, "limited");
    let unl = study("", "unlimited");
    let order = |e: &[(f64, f64)], i: usize, k: usize| {
        let f = |p: (f64, f64)| if k == 0 { p.0 } else { p.1 };
        (f(e[i]) / f(e[i + 1])).log2()
    };
    println!(
        "L3: limited L2 orders {:.3} (coarse pair), {:.3} (fine pair); Linf {:.3}, {:.3}; unlimited L2 {:.3}, {:.3}; Linf {:.3}, {:.3}; fine-mesh L2 ratio limited/unlimited {:.3}",
        order(&lim, 0, 0),
        order(&lim, 1, 0),
        order(&lim, 0, 1),
        order(&lim, 1, 1),
        order(&unl, 0, 0),
        order(&unl, 1, 0),
        order(&unl, 0, 1),
        order(&unl, 1, 1),
        lim[2].0 / unl[2].0
    );
    assert!(order(&lim, 1, 0) >= 1.8, "L3: fine-pair L2 order {}", order(&lim, 1, 0));
}

// ================================================================ ADR 0115 (wave 45A): the range-scaled Venkatakrishnan limiter
// Everything below this line belongs to ADR 0115.

// The case key, its located-error test and P6 were removed under the registered decision rule (Z1 missed at every k); their results
// are in the ADR 0115 addendum. The checks below set the library variant through `simple_solver_mut`.

/// Build `text` (with the Barth-Jespersen key) and switch the momentum limiter to `lim` before iteration 1 (`None` keeps Barth-Jespersen).
fn w45a_sim(text: &str, lim: Option<rcfd_fv::gradient::Limiter>) -> Simulation {
    let mut sim = sim_of(text);
    if lim.is_some() {
        sim.simple_solver_mut().unwrap().linear_upwind_limiter = lim;
    }
    sim
}

fn w45a_order(e: &[(f64, f64)], i: usize, linf: bool) -> f64 {
    let f = |p: (f64, f64)| if linf { p.1 } else { p.0 };
    (f(e[i]) / f(e[i + 1])).log2()
}

/// ADR 0115 P3: Kovasznay (the ADR 0081 L3 set-up) with the range-scaled limiter at each registered k; limit: fine-pair L2 order >= 1.8.
#[test]
fn w45a_p3_kovasznay_order() {
    let mut fails = Vec::new();
    for k in [0.002, 0.01, 0.05] {
        let e: Vec<(f64, f64)> = [12usize, 24, 48]
            .iter()
            .map(|&nx| {
                let t0 = Instant::now();
                let mut sim = w45a_sim(
                    &kovasznay(nx, BJ),
                    Some(rcfd_fv::gradient::Limiter::VenkatakrishnanRange(k)),
                );
                let n = sim.run(20000);
                let e = kovasznay_error(&sim);
                println!(
                    "W45A P3 k {k} {nx}: {n} iterations, converged {}, L2 {:.4e}, Linf {:.4e} [{:.1} s]",
                    sim.is_converged(),
                    e.0,
                    e.1,
                    t0.elapsed().as_secs_f64()
                );
                e
            })
            .collect();
        let o = w45a_order(&e, 1, false);
        println!(
            "W45A P3 k {k}: L2 orders {:.3} / {o:.3}, Linf {:.3} / {:.3}",
            w45a_order(&e, 0, false),
            w45a_order(&e, 0, true),
            w45a_order(&e, 1, true)
        );
        if o.is_nan() || o < 1.8 {
            fails.push((k, o));
        }
    }
    assert!(fails.is_empty(), "P3: {fails:?}");
}

/// Forced steady Taylor-Green flow on [0, 2 pi]^2 (U = 1): u = (sin x cos y, -cos x sin y), body force 2 nu u, started from the exact
/// velocity; doubly periodic (the drafted P4, infeasible at nu = 0.1 by the smoke) or with the exact velocity on all four sides (the
/// registered P4, ADR 0115).
fn w45a_tg(n: usize, lim: Option<rcfd_fv::gradient::Limiter>, bj: bool, nu: f64, periodic: bool) -> Simulation {
    let tp = 2.0 * std::f64::consts::PI;
    let key = if bj { BJ } else { "" };
    let ux = "1 [m/s] * sin(x / 1 [m]) * cos(y / 1 [m])";
    let uy = "-1 [m/s] * cos(x / 1 [m]) * sin(y / 1 [m])";
    let mut b = String::new();
    if !periodic {
        for p in ["xmin", "xmax", "ymin", "ymax"] {
            b.push_str(&format!(
                "[boundaries.{p}]
velocity = \"fixed\"
velocity_expression_x = \"{ux}\"
velocity_expression_y = \"{uy}\"
"
            ));
        }
    }
    let text = format!(
        "[mesh]
generator = \"box_2d\"
nx = {n}
ny = {n}
lx = {tp:?}
ly = {tp:?}
periodic_x = {periodic}
periodic_y = {periodic}
[physics]
nu = {nu:?}
[initial]
velocity_expression_x = \"{ux}\"
velocity_expression_y = \"{uy}\"
{b}[solver]
algorithm = \"simple\"
alpha_u = 0.7
alpha_p = 0.3
convection = \"linear_upwind\"
{key}max_iterations = 20000
tolerance_u = 1e-10
tolerance_p = 1e-10
"
    );
    let mut sim = w45a_sim(&text, lim);
    let f = |x: [f64; 3]| {
        [
            2.0 * nu * x[0].sin() * x[1].cos(),
            -2.0 * nu * x[0].cos() * x[1].sin(),
            0.0,
        ]
    };
    let m = &sim.geom.mesh;
    let body = rcfd_fv::simple::BodyForce {
        cell: m.cell_centres().iter().map(|&x| f(x)).collect(),
        face: m
            .face_centres()
            .iter()
            .zip(m.face_area_vectors())
            .map(|(&x, s)| vec3::dot(f(x), *s))
            .collect(),
    };
    sim.simple_solver_mut().unwrap().body_force = Some(body);
    sim
}

/// (volume-weighted RMS, max) of the velocity error against the exact Taylor-Green field, and the share of cells with a limiter factor
/// below 0.99 in either component (the factors of `kind` on the final state).
fn w45a_tg_error(sim: &Simulation, kind: Option<rcfd_fv::gradient::Limiter>) -> (f64, f64, f64) {
    let g = &sim.geom;
    let cc = g.mesh.cell_centres();
    let vol = g.mesh.cell_volumes();
    let u = &sim.velocity().values;
    let (mut s, mut v, mut linf) = (0.0, 0.0, 0.0f64);
    for c in 0..cc.len() {
        let (x, y) = (cc[c][0], cc[c][1]);
        let d2 = (u[c][0] - x.sin() * y.cos()).powi(2) + (u[c][1] + x.cos() * y.sin()).powi(2);
        s += vol[c] * d2;
        v += vol[c];
        linf = linf.max(d2.sqrt());
    }
    let share = kind.map_or(0.0, |kind| {
        let ub = sim.velocity().boundary_values(g);
        let psi: Vec<Vec<f64>> = (0..2)
            .map(|k| {
                let cells: Vec<f64> = u.iter().map(|w| w[k]).collect();
                let b: Vec<f64> = ub.iter().map(|w| w[k]).collect();
                let gr = rcfd_fv::gradient::least_squares(g, &cells, &b);
                rcfd_fv::gradient::limiter(g, &cells, &b, &gr, kind)
            })
            .collect();
        (0..cc.len()).filter(|&c| psi[0][c] < 0.99 || psi[1][c] < 0.99).count() as f64 / cc.len() as f64
    });
    ((s / v).sqrt(), linf, share)
}

/// Run one Taylor-Green mesh to convergence or the cap; (L2, Linf, share psi < 0.99, converged).
fn w45a_tg_run(
    label: &str,
    n: usize,
    lim: Option<rcfd_fv::gradient::Limiter>,
    bj: bool,
    periodic: bool,
) -> (f64, f64, f64, bool) {
    let t0 = Instant::now();
    let mut sim = w45a_tg(n, lim, bj, 0.1, periodic);
    let cap = 20000;
    let it = sim.run(cap);
    let kind = lim.or(bj.then_some(rcfd_fv::gradient::Limiter::BarthJespersen));
    let (l2, linf, share) = w45a_tg_error(&sim, kind);
    println!(
        "W45A TG {label} {n}^2: {it} iterations, converged {}, trip {:?}, last {:?}, L2 {l2:.4e}, Linf {linf:.4e}, share psi < 0.99 {share:.3} [{:.1} s]",
        sim.is_converged(),
        sim.steady_divergence().map(|d| (d.iteration, d.kind)),
        sim.last(),
        t0.elapsed().as_secs_f64()
    );
    (l2, linf, share, sim.is_converged())
}

/// ADR 0115 rule-d feasibility smoke of P4 with Barth-Jespersen and unlimited `linear_upwind`: doubly periodic at 16^2 and 32^2 (run
/// first; did not converge), then the exact velocity on all sides at 16^2, 32^2 and 64^2 (the corrected set-up).
#[test]
#[ignore = "ADR 0115 P4 feasibility smoke (several minutes in release)"]
fn w45a_p4_feasibility_smoke() {
    for (per, ns) in [(true, &[16usize, 32][..]), (false, &[16, 32, 64][..])] {
        for &n in ns {
            w45a_tg_run(&format!("smoke BJ periodic {per}"), n, None, true, per);
            w45a_tg_run(&format!("smoke unlimited periodic {per}"), n, None, false, per);
        }
    }
}

/// ADR 0115 P4 (amended before registration: the exact velocity on all sides): with the range-scaled limiter at each registered k, the
/// fine-pair L2 order >= 1.8 and the 64^2 L2 error <= the Barth-Jespersen 64^2 L2 error of the same run. Reported: L-inf orders, the share
/// of cells with psi < 0.99.
#[test]
#[ignore = "ADR 0115 P4 (about 3 min in release)"]
fn w45a_p4_taylor_green_order() {
    let study = |label: &str, lim: Option<rcfd_fv::gradient::Limiter>| -> Vec<(f64, f64)> {
        [16usize, 32, 64]
            .iter()
            .map(|&n| {
                let (l2, linf, _, conv) = w45a_tg_run(label, n, lim, true, false);
                assert!(conv, "P4 {label} {n}: not converged");
                (l2, linf)
            })
            .collect()
    };
    let bj = study("P4 BJ", None);
    let mut fails = Vec::new();
    for k in [0.002, 0.01, 0.05] {
        let e = study(
            &format!("P4 k {k}"),
            Some(rcfd_fv::gradient::Limiter::VenkatakrishnanRange(k)),
        );
        let o = w45a_order(&e, 1, false);
        println!(
            "W45A P4 k {k}: L2 orders {:.3} / {o:.3}, Linf {:.3} / {:.3}; fine L2 {:.4e} against BJ {:.4e} (BJ orders {:.3} / {:.3})",
            w45a_order(&e, 0, false),
            w45a_order(&e, 0, true),
            w45a_order(&e, 1, true),
            e[2].0,
            bj[2].0,
            w45a_order(&bj, 0, false),
            w45a_order(&bj, 1, false)
        );
        if o.is_nan() || o < 1.8 || e[2].0.is_nan() || e[2].0 > bj[2].0 {
            fails.push((k, o, e[2].0, bj[2].0));
        }
    }
    assert!(fails.is_empty(), "P4: {fails:?}");
}

/// ADR 0115 P4 diagnosis of the periodic smoke (16^2, unlimited, 4000 iterations): nu = 1 periodic, nu = 0.1 periodic, nu = 0.1 with the
/// exact velocity on all sides.
#[test]
#[ignore = "ADR 0115 P4 diagnosis (seconds in release)"]
fn w45a_tg_diag() {
    for (nu, per) in [(1.0, true), (0.1, true), (0.1, false)] {
        let mut sim = w45a_tg(16, None, false, nu, per);
        for it in 1..=4000 {
            let s = sim.step();
            if it % 500 == 0 {
                let (l2, _, _) = w45a_tg_error(&sim, None);
                let bf = sim.simple_solver().unwrap().body_force.is_some();
                let pr = sim.pressure().values.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                    - sim.pressure().values.iter().cloned().fold(f64::INFINITY, f64::min);
                println!("DIAG nu {nu} periodic {per} it {it}: res_u {:.3e} res_p {:.3e} L2 {l2:.3e} body {bf} p range {pr:.4} (exact 1.0)", s.res_u, s.res_p);
            }
        }
    }
}
