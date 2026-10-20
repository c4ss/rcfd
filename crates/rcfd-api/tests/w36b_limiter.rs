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
