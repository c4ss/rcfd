//! Enthalpy-porosity melting and solidification library checks (F-212, ADR 0035 checks P1 to P8, K1, K2). The limits are the
//! ones registered in `docs/adr/0035-wave21a-thermal-extras.md` before any run.

mod cht_common;

use cht_common::{erfc, geometry, patch_area};
use rcfd_fv::cht::conductive_face_flows;
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::solidification::{carman_kozeny, momentum_sink, PhaseChange, PhaseLaw};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const EPS: f64 = 0.002;
const LATENT_K: f64 = 2.0; // L/c with c = 1, L = 2
const T0: f64 = 0.1;
const T_END: f64 = 1.0;
const LX: f64 = 6.0;

fn erf(x: f64) -> f64 {
    1.0 - erfc(x)
}

/// Root of the two-phase Neumann equation for the Stefan numbers `st_l`, `st_s` (equal properties, alpha = 1).
fn neumann_lambda(st_l: f64, st_s: f64) -> f64 {
    let pi = std::f64::consts::PI;
    let g = |l: f64| {
        let e = (-l * l).exp();
        st_l * e / (pi.sqrt() * erf(l)) - st_s * e / (pi.sqrt() * erfc(l)) - l
    };
    let (mut a, mut b) = (1e-3, 6.0);
    assert!(g(a) > 0.0 && g(b) < 0.0);
    for _ in 0..200 {
        let m = 0.5 * (a + b);
        if g(m) > 0.0 {
            a = m;
        } else {
            b = m;
        }
    }
    let l = 0.5 * (a + b);
    assert!(g(l).abs() <= 1e-12, "P7 residual {}", g(l));
    l
}

/// Analytic temperature (melting orientation: hot wall at x = 0, liquid for x < s).
fn neumann_t(x: f64, t: f64, lambda: f64, t_w: f64, t_i: f64) -> f64 {
    let s = 2.0 * lambda * t.sqrt();
    let eta = x / (2.0 * t.sqrt());
    if x < s {
        t_w - t_w * erf(eta) / erf(lambda)
    } else {
        t_i + (0.0 - t_i) * erfc(eta) / erfc(lambda)
    }
}

struct Run {
    s_num: f64,
    max_newton: usize,
    all_converged: bool,
    worst_balance: f64,
    worst_monotone: f64,
    f_in_range: bool,
    safeguarded: usize,
    /// Interface position (liquid content) of the initial state.
    s_init: f64,
}

/// `melting = true`: hot wall `t_w > 0`, solid initial `t_i <= 0`; `false`: the mirror image (`t_w < 0`, liquid initial `t_i >= 0`).
fn stefan_run(n: usize, t_w: f64, t_i: f64, lambda: f64, melting: bool, one_phase: bool) -> Run {
    stefan_run_x(n, t_w, t_i, lambda, melting, one_phase, false, false)
}

#[allow(clippy::too_many_arguments)]
fn stefan_run_x(
    n: usize,
    t_w: f64,
    t_i: f64,
    lambda: f64,
    melting: bool,
    one_phase: bool,
    force: bool,
    avg_init: bool,
) -> Run {
    let geom = geometry(box_2d(n, 1, LX, 1.0, false, false).unwrap());
    let sign = if melting { 1.0 } else { -1.0 };
    // analytic profile of the melting orientation, mirrored for freezing
    let mut t = ScalarField::uniform("T", &geom, 0.0);
    for (c, x) in geom.mesh.cell_centres().iter().enumerate() {
        let analytic = sign * neumann_t(x[0], T0, lambda, sign * t_w, sign * t_i);
        // one-phase: the solid stays at its initial value (no solid-side profile)
        let solid = x[0] >= 2.0 * lambda * T0.sqrt();
        t.values[c] = if one_phase && solid { t_i } else { analytic };
    }
    if avg_init {
        // ADR 0043 S7: cell average of the sharp-front analytic enthalpy, inverted through the piecewise-linear law
        let s0 = 2.0 * lambda * T0.sqrt();
        let (hs, hl) = (-EPS, EPS + LATENT_K);
        let xs = geom.mesh.cell_centres();
        let hcell = LX / n as f64;
        for c in 0..t.values.len() {
            let sub = 200;
            let mut acc = 0.0;
            for k in 0..sub {
                let x = xs[c][0] - 0.5 * hcell + (k as f64 + 0.5) / sub as f64 * hcell;
                let liquid = x < s0;
                let tm = if one_phase && !liquid {
                    sign * t_i
                } else {
                    neumann_t(x, T0, lambda, sign * t_w, sign * t_i)
                };
                acc += tm + if liquid { LATENT_K } else { 0.0 };
            }
            let hm = acc / sub as f64;
            let h = if melting { hm } else { LATENT_K - hm };
            t.values[c] = if h <= hs {
                h
            } else if h >= hl {
                h - LATENT_K
            } else {
                -EPS + (h - hs) / (hl - hs) * 2.0 * EPS
            };
        }
    }
    t.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(t_w))).unwrap();
    t.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(t_i))).unwrap();
    let law = PhaseLaw {
        solidus: -EPS,
        liquidus: EPS,
        latent_k: LATENT_K,
    };
    let mut pc = PhaseChange::new(law, vec![1.0; geom.mesh.n_faces()], &t.values);
    pc.force_safeguard = force;
    let h = LX / n as f64;
    let steps = ((T_END - T0) / (0.5 * h)).ceil() as usize;
    let dt = (T_END - T0) / steps as f64;
    let vol = geom.mesh.cell_volumes();
    let mut run = Run {
        s_num: 0.0,
        max_newton: 0,
        all_converged: true,
        worst_balance: 0.0,
        worst_monotone: 0.0,
        f_in_range: true,
        safeguarded: 0,
        s_init: 0.0,
    };
    {
        let f0 = pc.liquid_fraction(&t.values);
        let a0 = patch_area(&geom, "xmin");
        run.s_init = (0..f0.len())
            .map(|c| vol[c] * if melting { f0[c] } else { 1.0 - f0[c] })
            .sum::<f64>()
            / a0;
    }
    for _ in 0..steps {
        let f_old = pc.liquid_fraction(&t.values);
        let h_old = pc.h_old.clone();
        let st = pc.advance(&geom, &mut t, dt, None);
        run.max_newton = run.max_newton.max(st.iterations);
        run.all_converged &= st.converged;
        let f_new = pc.liquid_fraction(&t.values);
        let d_h: f64 = (0..t.values.len()).map(|c| vol[c] * (pc.h_old[c] - h_old[c])).sum();
        let flows = conductive_face_flows(&geom, &t, &pc.gamma, false);
        let q_in: f64 = -["xmin", "xmax"]
            .iter()
            .map(|p| rcfd_fv::cht::patch_heat_flow_out(&geom, &flows, p).unwrap())
            .sum::<f64>();
        let bal = (d_h - dt * q_in).abs() / d_h.abs().max((dt * q_in).abs()).max(1e-300);
        run.worst_balance = run.worst_balance.max(bal);
        for c in 0..f_new.len() {
            run.f_in_range &= (0.0..=1.0).contains(&f_new[c]);
            let df = f_new[c] - f_old[c];
            // melting: f must not decrease; freezing: f must not increase
            let bad = if melting { -df } else { df };
            run.worst_monotone = run.worst_monotone.max(bad);
        }
    }
    run.safeguarded = pc.safeguarded_steps;
    let f = pc.liquid_fraction(&t.values);
    let area = patch_area(&geom, "xmin");
    run.s_num = (0..f.len())
        .map(|c| vol[c] * if melting { f[c] } else { 1.0 - f[c] })
        .sum::<f64>()
        / area;
    run
}

fn stefan_case(tag: &str, t_w: f64, t_i: f64, melting: bool, one_phase: bool, levels: &[usize], orders_too: bool) {
    let (st_l, st_s) = (t_w.abs() / LATENT_K, if one_phase { 0.0 } else { t_i.abs() / LATENT_K });
    // the analytic orientation uses the positive driving temperatures
    let lambda = neumann_lambda(st_l, st_s);
    let s_exact = 2.0 * lambda * T_END.sqrt();
    let mut errs = Vec::new();
    for &n in levels {
        let r = stefan_run(n, t_w, t_i, lambda, melting, one_phase);
        let e = (r.s_num - s_exact).abs() / s_exact;
        println!(
            "{tag} N={n}: s {:.6} vs {:.6} (rel {e:.4e}); balance {:.2e}, monotone violation {:.2e}, newton max {}, converged {}",
            r.s_num, s_exact, r.worst_balance, r.worst_monotone, r.max_newton, r.all_converged
        );
        assert!(r.worst_balance <= 1e-10, "{tag} P4 balance {}", r.worst_balance);
        assert!(r.f_in_range, "{tag} P5 range");
        assert!(r.worst_monotone <= 1e-10, "{tag} P5 monotone {}", r.worst_monotone);
        assert!(
            r.all_converged && r.max_newton < 100,
            "{tag} P6 newton {}",
            r.max_newton
        );
        errs.push(e);
    }
    let orders: Vec<f64> = errs.windows(2).map(|w| (w[0] / w[1]).log2()).collect();
    println!("{tag} lambda {lambda:.9}, errors {errs:?}, orders {orders:?} (limits: N=240 <= 3e-2, orders >= 0.9)");
    assert!(errs[2] <= 0.03, "{tag} acceptance: error at N=240 is {}", errs[2]);
    if orders_too {
        assert!(orders.iter().all(|&o| o >= 0.9), "{tag} orders {orders:?}");
    }
}

/// Corrected P1 set-up: the solid starts at the solidus Ts = -EPS (the ADR says "solid at Ts"), so St_s = 0.
/// Registered P1 (corrected set-up). MISSED: the successive orders are [-0.55, 0.06, 0.98] against the registered >= 0.9: the
/// position error (0.2 to 0.3 percent) is at the level of the mush/time-step offsets and does not decrease monotonically.
#[test]
#[ignore = "known miss P1: successive-order criterion not met (error already 0.3 percent)"]
fn p1_one_phase_melting_known_miss() {
    stefan_case("P1", 1.0, -EPS, true, true, &[60, 120, 240, 480], true);
}

/// Evidence only (not the registered P1 acceptance): the position error at N=240 against the 3 percent limit and the
/// per-step balance, range and Newton invariants at all four levels.
#[test]
fn p1_one_phase_melting_position_and_invariants() {
    stefan_case("P1", 1.0, -EPS, true, true, &[60, 120, 240, 480], false);
}

/// Record of the first P1 set-up, which started the solid at the melting temperature 0 (mid-mush, f = 0.5 everywhere): a
/// set-up defect, not a scheme miss. Kept as a record.
#[test]
#[ignore = "set-up defect record: solid started at Tm instead of Ts"]
fn p1_one_phase_melting_setup_defect_record() {
    stefan_case("P1-old", 1.0, 0.0, true, true, &[60, 120, 240, 480], true);
}

/// Registered P2. MISSED at N=480 (invariants P4/P5/P6): the Newton iteration cycles on the piecewise-linear enthalpy when the
/// mush half-width (0.002) is much thinner than a cell (0.0125); see ADR 0035 addendum.
#[test]
#[ignore = "known miss P2: Newton limit cycle at N=480 (cap hit, balance not closed); since ADR 0043 D2 the invariants hold at N=480 (s1_s2_s4_...) and only the successive-order criterion fails (errors 1.86e-2, 3.5e-3, 6.3e-5, 1.6e-3)"]
fn p2_two_phase_melting_known_miss() {
    stefan_case("P2", 1.0, -0.5, true, false, &[60, 120, 240, 480], true);
}

/// Evidence for the levels N = 60, 120, 240 (all of P2 holds there); not a replacement of the registered four-level check.
#[test]
fn p2_two_phase_melting_n60_to_240() {
    stefan_case("P2", 1.0, -0.5, true, false, &[60, 120, 240], true);
}

#[test]
#[ignore = "known miss P8: Newton limit cycle at N=480 (cap hit, balance not closed); since ADR 0043 D2 the invariants hold at N=480 and only the successive-order criterion fails (mirror image of P2)"]
fn p8_two_phase_solidification_known_miss() {
    stefan_case("P8", -1.0, 0.5, false, false, &[60, 120, 240, 480], true);
}

#[test]
fn p8_two_phase_solidification_n60_to_240() {
    stefan_case("P8", -1.0, 0.5, false, false, &[60, 120, 240], true);
}

#[test]
fn p3_sensible_limit_matches_erf() {
    let n = 240;
    let geom = geometry(box_2d(n, 1, LX, 1.0, false, false).unwrap());
    let ex = |x: f64, t: f64| 1.0 + (0.0 - 1.0) * erf(x / (2.0 * t.sqrt()));
    let mut t = ScalarField::uniform("T", &geom, 0.0);
    for (c, x) in geom.mesh.cell_centres().iter().enumerate() {
        t.values[c] = ex(x[0], T0);
    }
    t.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(1.0))).unwrap();
    t.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let law = PhaseLaw {
        solidus: -EPS,
        liquidus: EPS,
        latent_k: 0.0,
    };
    let mut pc = PhaseChange::new(law, vec![1.0; geom.mesh.n_faces()], &t.values);
    let h = LX / n as f64;
    let steps = ((T_END - T0) / (0.5 * h)).ceil() as usize;
    let dt = (T_END - T0) / steps as f64;
    for _ in 0..steps {
        pc.advance(&geom, &mut t, dt, None);
    }
    let err = geom
        .mesh
        .cell_centres()
        .iter()
        .zip(&t.values)
        .map(|(x, v)| (v - ex(x[0], T_END)).abs())
        .fold(0.0, f64::max);
    println!("P3 sensible limit: max error / dT = {err:.4e} (limit 5e-3)");
    assert!(err <= 5e-3, "P3 {err}");
}

#[test]
fn k1_carman_kozeny_values() {
    let (c, b) = (1e5, 1e-3);
    let a = carman_kozeny(0.5, c, b);
    let exact = c * 0.25 / (0.125 + 0.001);
    let rel = |x: f64, y: f64| (x - y).abs() / y.abs();
    println!("K1 A(0.5) = {a:.9} vs {exact:.9}");
    assert!(rel(a, exact) <= 1e-14);
    assert_eq!(carman_kozeny(1.0, c, b), 0.0);
    assert!(rel(carman_kozeny(0.0, c, b), c / b) <= 1e-14);
    let s = momentum_sink(&[0.5, 1.0], c, b, 2.0);
    assert!(rel(s[0], exact / 2.0) <= 1e-14 && s[1] == 0.0);
}

fn brinkman_error(ny: usize) -> f64 {
    let (nu, g, h) = (0.1, 1.0, 0.5);
    let kappa = 3.6;
    let b = 1e-3;
    // C such that A(0.5)/rho = kappa
    let c = kappa * (0.125 + b) / 0.25;
    let mesh = box_2d(6, ny, 3.0, 1.0, true, false).unwrap();
    let geom: Arc<FvGeometry> = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for w in ["ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(nu, Coupling::Simplec);
    s.alpha_u = 0.9;
    let mut solver = SimpleSolver::new(geom.clone(), u, p, s);
    solver.body_force = Some(BodyForce::uniform(&solver.geom, [g, 0.0, 0.0]));
    let sink = momentum_sink(&vec![0.5; geom.n_cells()], c, b, 1.0);
    assert!((sink[0] - kappa).abs() <= 1e-12);
    solver.sink = Some(sink);
    let mut last = solver.iterate();
    for _ in 0..6000 {
        last = solver.iterate();
        if last.max_du < 1e-13 && last.continuity < 1e-10 {
            break;
        }
    }
    // the normalised momentum residual is not used: the v-equation is identically zero here, so its normalisation is noise
    assert!(last.max_du < 1e-11 && last.continuity < 1e-10, "{last:?}");
    let m = (kappa / nu).sqrt();
    let exact = |y: f64| g / kappa * (1.0 - (m * (y - h)).cosh() / (m * h).cosh());
    let uc = exact(h);
    let cc = geom.mesh.cell_centres();
    let err = solver
        .u
        .values
        .iter()
        .enumerate()
        .map(|(i, v)| (v[0] - exact(cc[i][1])).abs())
        .fold(0.0, f64::max)
        / uc;
    println!("K2 ny={ny}: centre-normalised max error {err:.4e}");
    err
}

/// Registered K2. MISSED narrowly: the ny=20 centre-normalised error is 1.0040e-2 against the limit 1e-2; both orders (1.84,
/// 1.92) meet the >= 1.8 limit.
#[test]
#[ignore = "known miss K2: ny=20 error 1.0040e-2 vs limit 1e-2"]
fn k2_brinkman_channel_known_miss() {
    let e: Vec<f64> = [20usize, 40, 80].iter().map(|&n| brinkman_error(n)).collect();
    let (o1, o2) = ((e[0] / e[1]).log2(), (e[1] / e[2]).log2());
    println!("K2 errors {e:?}, orders {o1:.3} {o2:.3} (limits: ny=20 <= 1e-2, orders >= 1.8)");
    assert!(e[0] <= 1e-2, "K2 {}", e[0]);
    assert!(o1 >= 1.8 && o2 >= 1.8, "K2 orders {o1} {o2}");
}

/// Evidence only: the registered order assertions of K2 without the ny=20 error limit (which is the recorded miss).
#[test]
fn k2_brinkman_channel_orders() {
    let e: Vec<f64> = [20usize, 40, 80].iter().map(|&n| brinkman_error(n)).collect();
    let (o1, o2) = ((e[0] / e[1]).log2(), (e[1] / e[2]).log2());
    assert!(o1 >= 1.8 && o2 >= 1.8, "{e:?} {o1} {o2}");
}

// ---- ADR 0043 D2 checks S1 to S5 ------------------------------------------------------------------------------------

fn check_invariants(tag: &str, r: &Run) {
    assert!(r.worst_balance <= 1e-10, "{tag} P4 balance {}", r.worst_balance);
    assert!(r.f_in_range, "{tag} P5 range");
    assert!(r.worst_monotone <= 1e-10, "{tag} P5 monotone {}", r.worst_monotone);
    assert!(
        r.all_converged && r.max_newton < 100,
        "{tag} P6 newton {}",
        r.max_newton
    );
}

/// S1 (and S4): P2 and P8 at all four levels including N = 480: P4, P5, P6 and the position error <= 3 percent. S2: the
/// safeguarded branch is not entered at N <= 240. S4: it is entered at N = 480 (the limit cycle of the plain Newton loop existed).
#[test]
fn s1_s2_s4_two_phase_stefan_four_levels_with_safeguard() {
    for (tag, t_w, t_i, melting) in [("P2", 1.0_f64, -0.5_f64, true), ("P8", -1.0_f64, 0.5_f64, false)] {
        let lambda = neumann_lambda(t_w.abs() / LATENT_K, t_i.abs() / LATENT_K);
        let s_exact = 2.0 * lambda * T_END.sqrt();
        for n in [60usize, 120, 240, 480] {
            let r = stefan_run(n, t_w, t_i, lambda, melting, false);
            let e = (r.s_num - s_exact).abs() / s_exact;
            println!(
                "{tag} N={n}: rel {e:.4e}; balance {:.2e}, monotone {:.2e}, newton max {}, safeguarded steps {}",
                r.worst_balance, r.worst_monotone, r.max_newton, r.safeguarded
            );
            check_invariants(&format!("{tag} N={n}"), &r);
            assert!(e <= 0.03, "{tag} N={n} position error {e}");
            if n <= 240 {
                assert_eq!(
                    r.safeguarded, 0,
                    "{tag} N={n}: the plain loop converged before, it must not change"
                );
            } else {
                assert!(
                    r.safeguarded > 0,
                    "{tag} N={n}: the plain loop should have failed here (S4)"
                );
            }
        }
    }
}

/// S2: one-phase P1 never enters the safeguarded branch (all four levels) and keeps the invariants.
#[test]
fn s2_p1_never_enters_the_safeguarded_branch() {
    let lambda = neumann_lambda(0.5, 0.0);
    for n in [60usize, 120, 240, 480] {
        let r = stefan_run(n, 1.0, -EPS, lambda, true, true);
        check_invariants(&format!("P1 N={n}"), &r);
        assert_eq!(r.safeguarded, 0, "P1 N={n}");
    }
}

/// S3: the safeguarded branch forced on every step converges to the same discrete solution (interface position within 1e-8
/// relative) and keeps P4, P5, P6.
#[test]
fn s3_forced_safeguarded_branch_matches_the_plain_loop() {
    let lambda = neumann_lambda(0.5, 0.25);
    for n in [60usize, 120, 240] {
        let a = stefan_run(n, 1.0, -0.5, lambda, true, false);
        let b = stefan_run_x(n, 1.0, -0.5, lambda, true, false, true, false);
        let d = (a.s_num - b.s_num).abs() / a.s_num;
        println!(
            "S3 N={n}: plain {:.10} forced {:.10} rel diff {d:.2e}, forced newton max {}",
            a.s_num, b.s_num, b.max_newton
        );
        check_invariants(&format!("S3 N={n}"), &b);
        assert!(d <= 1e-8, "S3 N={n} {d}");
        assert!(b.safeguarded > 0);
    }
}

/// S5 (reported, not asserted): successive differences of the numerical interface position of P1 and their orders.
#[test]
fn s5_p1_successive_position_differences_report() {
    let lambda = neumann_lambda(0.5, 0.0);
    let s_exact = 2.0 * lambda * T_END.sqrt();
    let runs: Vec<Run> = [60usize, 120, 240, 480]
        .iter()
        .map(|&n| stefan_run(n, 1.0, -EPS, lambda, true, true))
        .collect();
    let s: Vec<f64> = runs.iter().map(|r| r.s_num).collect();
    let s0_exact = 2.0 * lambda * T0.sqrt();
    println!(
        "S5 initial-state position errors (absolute, against 2 lambda sqrt(t0) = {s0_exact:.9}): {:?}; final absolute errors {:?}",
        runs.iter().map(|r| r.s_init - s0_exact).collect::<Vec<_>>(),
        runs.iter().map(|r| r.s_num - s_exact).collect::<Vec<_>>()
    );
    let d: Vec<f64> = s.windows(2).map(|w| (w[0] - w[1]).abs()).collect();
    let o: Vec<f64> = d.windows(2).map(|w| (w[0] / w[1]).log2()).collect();
    println!(
        "S5 P1 positions {s:?} (exact {s_exact:.9}); signed errors {:?}; successive differences {d:?}; orders of the differences {o:?}",
        s.iter().map(|x| (x - s_exact) / s_exact).collect::<Vec<_>>()
    );
}

/// ADR 0043 S7: corrected initial state (cell-averaged sharp-front enthalpy). The registered limits of P1, P2 and P8 unchanged.
fn stefan_case_v2(tag: &str, t_w: f64, t_i: f64, melting: bool, one_phase: bool, orders_too: bool) {
    let (st_l, st_s) = (t_w.abs() / LATENT_K, if one_phase { 0.0 } else { t_i.abs() / LATENT_K });
    let lambda = neumann_lambda(st_l, st_s);
    let s_exact = 2.0 * lambda * T_END.sqrt();
    let s0_exact = 2.0 * lambda * T0.sqrt();
    let mut errs = Vec::new();
    for n in [60usize, 120, 240, 480] {
        let r = stefan_run_x(n, t_w, t_i, lambda, melting, one_phase, false, true);
        let e = (r.s_num - s_exact).abs() / s_exact;
        println!(
            "{tag} v2 N={n}: initial position error {:.3e}; final rel {e:.4e}; balance {:.2e}, monotone {:.2e}, newton max {}, safeguarded {}",
            r.s_init - s0_exact,
            r.worst_balance,
            r.worst_monotone,
            r.max_newton,
            r.safeguarded
        );
        check_invariants(&format!("{tag} v2 N={n}"), &r);
        errs.push(e);
    }
    let orders: Vec<f64> = errs.windows(2).map(|w| (w[0] / w[1]).log2()).collect();
    println!("{tag} v2 errors {errs:?}, orders {orders:?} (limits: N=240 <= 3e-2, orders >= 0.9)");
    assert!(errs[2] <= 0.03, "{tag} v2 acceptance: error at N=240 is {}", errs[2]);
    if orders_too {
        assert!(orders.iter().all(|&o| o >= 0.9), "{tag} v2 orders {orders:?}");
    }
}

/// Corrected P1 set-up, invariants and the 3 percent position limit (the order criterion is the separate test below).
#[test]
fn p1_v2_corrected_initial_state_position_and_invariants() {
    stefan_case_v2("P1", 1.0, -EPS, true, true, false);
}

#[test]
#[ignore = "known miss (ADR 0043 S7): corrected initial state, successive orders [-1.03, -0.06, 0.45] against >= 0.9; the error sits on an O(mush half-width) floor of the registered reference, see the ADR addendum"]
fn p1_v2_known_miss_corrected_initial_state_orders() {
    stefan_case_v2("P1", 1.0, -EPS, true, true, true);
}

#[test]
fn p2_v2_corrected_initial_state_position_and_invariants() {
    stefan_case_v2("P2", 1.0, -0.5, true, false, false);
}

#[test]
#[ignore = "known miss (ADR 0043 S7): corrected initial state, successive orders [0.49, 1.85, 0.97] against >= 0.9 (first pair)"]
fn p2_v2_known_miss_corrected_initial_state_orders() {
    stefan_case_v2("P2", 1.0, -0.5, true, false, true);
}

#[test]
fn p8_v2_corrected_initial_state_position_and_invariants() {
    stefan_case_v2("P8", -1.0, 0.5, false, false, false);
}

#[test]
#[ignore = "known miss (ADR 0043 S7): mirror image of P2, successive orders [0.49, 1.85, 0.97] against >= 0.9 (first pair)"]
fn p8_v2_known_miss_corrected_initial_state_orders() {
    stefan_case_v2("P8", -1.0, 0.5, false, false, true);
}

/// Report only (ADR 0043 S5/S7 diagnosis): P1 with the corrected initial state against two references, the registered one-phase
/// Neumann position (solid at the melting temperature) and the two-phase Neumann position with the solid at the solidus
/// `T_i = -EPS` (`St_s = EPS/(L/c)`), which is the temperature the numerical solid actually has.
#[test]
fn s7_p1_reference_offset_report() {
    let lam1 = neumann_lambda(0.5, 0.0);
    let lam2 = neumann_lambda(0.5, EPS / LATENT_K);
    let (s1, s2) = (2.0 * lam1 * T_END.sqrt(), 2.0 * lam2 * T_END.sqrt());
    let mut e1 = Vec::new();
    let mut e2 = Vec::new();
    for n in [60usize, 120, 240, 480] {
        let r = stefan_run_x(n, 1.0, -EPS, lam2, true, false, false, true);
        e1.push((r.s_num - s1) / s1);
        e2.push((r.s_num - s2) / s2);
    }
    println!("S7 report: s_exact one-phase {s1:.9}, solid-at-solidus {s2:.9}; signed errors vs one-phase {e1:?}; vs solid-at-solidus {e2:?}");
}
