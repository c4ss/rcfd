//! F-684 (ADR 0615): the line search of ADR 0607 on the other total Lagrangian contact cases, with and without the
//! curvature fallback of ADR 0570: ADR 0542 P (the hub, `d = 1e-3 a`; fast) and ADR 0551 HS20t and HS10t (heavy records,
//! run once). A passive recorder (ADR 0615 D2) locates every correction, its trials and its step length, also in a failed
//! run. Every limit is the one registered in ADR 0615.

mod f601_common;
mod f624_common;

use f601_common::*;
use rcfd_fem::builders::upgrade_to_tet10;
use rcfd_fem::contact::{
    solve, solve_with, ContactOptions, ContactPair, ContactResult, Kinematics, QuadraticContactor, Target,
};
use rcfd_fem::convert::split_hexahedra;
use rcfd_fem::error::Result as FemResult;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::nonlinear::{Contribution, NewtonOptions};
use rcfd_fem::solve::DirectFactor;
use rcfd_fem::{FemError, SolidMesh, SolidModel};
use std::cell::RefCell;

// ------------------------------------------------------------------------------------------------------------ recorder

/// One correction seen by the recorder: the trial evaluations after its evaluation, the reconstructed step length
/// `(u_{k+1} - u_k) . d/(d . d)` with `d` the first trial's step (1 when there was no trial), and `|d|_inf/|u_k|_inf`.
#[derive(Debug, Clone, Copy)]
struct Correction {
    trials: usize,
    alpha: f64,
    scale: f64,
}

/// One Newton solve seen by the recorder: its evaluations and its corrections.
#[derive(Debug, Clone, Default)]
struct Segment {
    evaluations: usize,
    corrections: Vec<Correction>,
}

#[derive(Default)]
struct RecState {
    last: Option<Vec<f64>>,
    first_trial: Option<Vec<f64>>,
    trials: usize,
    segments: Vec<Segment>,
}

/// ADR 0615 D2: a contribution that adds nothing and records the evaluations and the trials of the line search.
#[derive(Default)]
struct Recorder(RefCell<RecState>);

fn inf_norm(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

impl Contribution for Recorder {
    fn name(&self) -> &str {
        "recorder"
    }
    fn add(&self, u: &[f64], _lf: f64, _force: &mut [f64], _tangent: &mut Vec<(usize, usize, f64)>) -> FemResult<()> {
        let mut s = self.0.borrow_mut();
        let st = &mut *s;
        match st.last.take() {
            None => st.segments.push(Segment {
                evaluations: 1,
                corrections: Vec::new(),
            }),
            Some(prev) => {
                let same = prev.iter().zip(u).all(|(a, b)| a.to_bits() == b.to_bits());
                if same && st.trials == 0 {
                    st.segments.push(Segment {
                        evaluations: 1,
                        corrections: Vec::new(),
                    });
                } else {
                    let (alpha, scale) = match st.first_trial.as_ref() {
                        Some(x1) => {
                            let d: Vec<f64> = x1.iter().zip(&prev).map(|(a, b)| a - b).collect();
                            let (mut sd, mut dd) = (0.0, 0.0);
                            for i in 0..u.len() {
                                sd += (u[i] - prev[i]) * d[i];
                                dd += d[i] * d[i];
                            }
                            (sd / dd, inf_norm(&d) / inf_norm(&prev).max(f64::MIN_POSITIVE))
                        }
                        None => (1.0, f64::NAN),
                    };
                    let seg = st.segments.last_mut().unwrap();
                    seg.evaluations += 1;
                    seg.corrections.push(Correction {
                        trials: st.trials,
                        alpha,
                        scale,
                    });
                }
            }
        }
        st.last = Some(u.to_vec());
        st.first_trial = None;
        st.trials = 0;
        Ok(())
    }
    fn add_trial_force(&self, u: &[f64], _lf: f64, _force: &mut [f64]) -> FemResult<()> {
        let mut s = self.0.borrow_mut();
        if s.trials == 0 {
            s.first_trial = Some(u.to_vec());
        }
        s.trials += 1;
        Ok(())
    }
}

impl Recorder {
    /// The segments; with `converged`, the last one dropped when it is the reactions' evaluation (1 evaluation, none after).
    fn segments(&self, converged: bool) -> Vec<Segment> {
        let mut v = self.0.borrow().segments.clone();
        if converged && v.last().is_some_and(|s| s.evaluations == 1 && s.corrections.is_empty()) {
            v.pop();
        }
        v
    }
}

// ------------------------------------------------------------------------------------------------------------ common

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn newton_ls() -> NewtonOptions {
    NewtonOptions::new(1).max_iterations(30).line_search(true)
}

fn record(r: &ContactResult) -> Vec<(usize, usize)> {
    r.steps
        .iter()
        .flat_map(|s| s.solves.iter().map(|x| (x.residuals.len() - 1, x.convex_iterations)))
        .collect()
}

fn print_segments(tag: &str, segs: &[Segment]) {
    for (j, s) in segs.iter().enumerate() {
        let a: Vec<String> = s.corrections.iter().map(|c| format!("{:.4}", c.alpha)).collect();
        let t: Vec<usize> = s.corrections.iter().map(|c| c.trials).collect();
        println!(
            "{tag} recorder segment {j}: {} evaluations, {} corrections, alpha [{}], trials {t:?}",
            s.evaluations,
            s.corrections.len(),
            a.join(", ")
        );
    }
}

/// SL, RT, RA of ADR 0615 on a converged run with the line search.
fn step_length_checks(c: &mut Checks, tag: &str, r: &ContactResult, rec: &Recorder) {
    let solves: Vec<_> = r.steps.iter().flat_map(|s| s.solves.iter()).collect();
    let mut sl_ok = true;
    for s in &solves {
        sl_ok &= s.step_lengths.len() == s.energies.len()
            && s.step_lengths.iter().all(|&a| a.is_finite() && a > 0.0 && a <= 1.0);
    }
    for (j, s) in solves.iter().enumerate() {
        println!(
            "{tag} solve {j}: iterations {}, fallback {}, step lengths {:?}, penetration {:.3e}",
            s.residuals.len() - 1,
            s.convex_iterations,
            s.step_lengths,
            s.penetration
        );
    }
    c.holds(&format!("SL {tag}: one length per correction, in (0, 1]"), sl_ok, "");
    let segs = rec.segments(true);
    print_segments(tag, &segs);
    let max_trials = segs
        .iter()
        .flat_map(|s| s.corrections.iter().map(|x| x.trials))
        .max()
        .unwrap_or(0);
    c.holds(
        &format!("RT {tag}: trials per correction <= 9"),
        max_trials <= 9,
        &format!("{max_trials}"),
    );
    let counts_rec: Vec<usize> = segs.iter().map(|s| s.corrections.len()).collect();
    let counts: Vec<usize> = solves.iter().map(|s| s.energies.len()).collect();
    c.holds(
        &format!("RA {tag}: corrections per solve"),
        counts_rec == counts,
        &format!("{counts_rec:?} against {counts:?}"),
    );
    let mut worst_rel = 0.0f64;
    if counts_rec == counts {
        for (seg, s) in segs.iter().zip(&solves) {
            for (x, &a) in seg.corrections.iter().zip(&s.step_lengths) {
                if x.trials > 0 && x.scale >= 1e-6 {
                    let e = (x.alpha / a - 1.0).abs();
                    worst_rel = if e.is_finite() { worst_rel.max(e) } else { f64::INFINITY };
                }
            }
        }
    } else {
        worst_rel = f64::INFINITY;
    }
    c.at_most(
        &format!("RA {tag}: recorder alpha against step_lengths"),
        worst_rel,
        1e-6,
    );
}

// ------------------------------------------------------------------------------------------------------------ P

/// ADR 0615 PL, PLF1 to PLF4, SL, RT, RA on ADR 0542 P (`Route::Contact`, `n = 4`, `d = 1e-3 a`).
#[test]
fn p_hub_with_the_line_search() {
    use f624_common::{fit_model, options, Route, Setup, BASE};
    let s = Setup { n: 4, ..BASE };
    let fit = fit_model(s);
    let (pair, o) = options(&s, Route::Contact);
    let mut c = Checks::new();
    let t = std::time::Instant::now();
    // P-0: the default.
    let p0 = solve(&fit.model, &[], &[], std::slice::from_ref(&pair), &o);
    let p0_text = p0.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
    println!("P-0 ({:?}): {p0_text}", t.elapsed());
    c.holds(
        "PL: P-0 refused at the first tangent",
        matches!(&p0, Err(FemError::Solver { solver, reason }) if *solver == "contact"
            && reason.starts_with("load step 1 of 1, augmentation 0: solver newton: singular tangent: pivot")),
        &p0_text,
    );
    // P-L: the line search alone.
    let t = std::time::Instant::now();
    let rec_l = Recorder::default();
    let pl = solve_with(
        &fit.model,
        &[],
        &[],
        std::slice::from_ref(&pair),
        &o.clone().newton(newton_ls()),
        &[&rec_l],
    );
    let pl_text = pl.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
    println!("P-L ({:?}): {pl_text}", t.elapsed());
    let segs = rec_l.segments(false);
    print_segments("P-L", &segs);
    c.holds(
        "PL: P-L refused with P-0's text",
        pl.is_err() && pl_text == p0_text,
        &pl_text,
    );
    c.holds(
        "PL: the recorder: one segment, 1 evaluation, 0 corrections",
        segs.len() == 1 && segs[0].evaluations == 1 && segs[0].corrections.is_empty(),
        &format!("{segs:?}"),
    );
    // P-F: the fallback alone (ADR 0570 P-f).
    let t = std::time::Instant::now();
    let pf = solve(
        &fit.model,
        &[],
        &[],
        std::slice::from_ref(&pair),
        &o.clone().curvature_fallback(true),
    );
    println!(
        "P-F ({:?}): {:?}",
        t.elapsed(),
        pf.as_ref().map(record).map_err(|e| e.to_string())
    );
    // P-LF: both.
    let t = std::time::Instant::now();
    let rec = Recorder::default();
    let plf = solve_with(
        &fit.model,
        &[],
        &[],
        std::slice::from_ref(&pair),
        &o.clone().newton(newton_ls()).curvature_fallback(true),
        &[&rec],
    );
    println!(
        "P-LF ({:?}): {:?}",
        t.elapsed(),
        plf.as_ref().map(record).map_err(|e| e.to_string())
    );
    match (&plf, &pf) {
        (Ok(r), Ok(f)) => {
            c.holds("PLF1: P-LF converges", true, "");
            let (rl, rf) = (record(r), record(f));
            let first = r.steps[0].solves[0].convex_iterations;
            let total: usize = rl.iter().map(|x| x.1).sum();
            c.holds(
                "PLF2: the record equal to P-F's; first solve 1 fallback, total 1",
                rl == rf && first == 1 && total == 1,
                &format!("{rl:?} against {rf:?}"),
            );
            let solves: Vec<_> = r.steps.iter().flat_map(|s| s.solves.iter()).collect();
            c.holds(
                "PLF3: one length per correction, every length 1",
                solves
                    .iter()
                    .all(|s| s.step_lengths.len() == s.energies.len() && s.step_lengths.iter().all(|&a| a == 1.0)),
                &format!(
                    "{:?}",
                    solves.iter().map(|s| s.step_lengths.clone()).collect::<Vec<_>>()
                ),
            );
            let fs: Vec<_> = f.steps.iter().flat_map(|s| s.solves.iter()).collect();
            let same = bits(r.result.displacement_values()) == bits(f.result.displacement_values())
                && solves.len() == fs.len()
                && solves
                    .iter()
                    .zip(&fs)
                    .all(|(a, b)| bits(&a.residuals) == bits(&b.residuals) && bits(&a.energies) == bits(&b.energies));
            c.holds("PLF4: P-LF against P-F bit for bit", same, "");
            step_length_checks(&mut c, "P-LF", r, &rec);
        }
        (Ok(_), Err(e)) => {
            c.holds("PLF1: P-LF converges", true, "");
            c.holds("PLF2/PLF4: the reference P-F", false, &format!("P-F failed: {e}"));
        }
        (Err(e), _) => {
            print_segments("P-LF", &rec.segments(false));
            c.holds("PLF1: P-LF converges", false, &format!("{e}"));
        }
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ Hertz

fn sphere_pair(h: f64) -> ContactPair {
    ContactPair::new(
        "top",
        Target::RigidSphere {
            centre: [0.0, 0.0, R_H],
            radius: R_H,
        },
        100.0 / h,
    )
}

fn hs20_mesh() -> SolidMesh {
    upgrade_to_hex20(&hertz_block(4, 5, 4)).unwrap().0
}

fn hs10_mesh() -> SolidMesh {
    upgrade_to_tet10(&split_hexahedra(&hertz_block(3, 4, 4)).unwrap())
        .unwrap()
        .0
}

/// ADR 0551 HS's model and options with the total Lagrangian body (ADR 0570 `hertz_setup`, `tl = true`), with the line
/// search and the fallback as asked.
fn hertz_setup(
    mesh: SolidMesh,
    line_search: bool,
    fallback: bool,
) -> (SolidModel, Vec<rcfd_fem::constraint::Prescribed>, ContactOptions) {
    let pres = hertz_prescribed(&mesh);
    let mut model = SolidModel::new(mesh);
    model.add_material("all", elastic(1.0, NU_H)).unwrap();
    let mut o = ContactOptions::new(1, 1e-8)
        .newton(NewtonOptions::new(1).max_iterations(30).line_search(line_search))
        .quadratic_contactor(QuadraticContactor::Mortar)
        .kinematics(Kinematics::TotalLagrangian)
        .factor(DirectFactor::SupernodalNestedDissection);
    if fallback {
        o = o.curvature_fallback(true);
    }
    (model, pres, o)
}

/// ADR 0570 D5: `a_M = sqrt(5/2 M_2/M_0)` over the nodes with `t > 0` at their reference positions.
fn moment_radius(model: &SolidModel, r: &ContactResult) -> f64 {
    let (mut m0, mut m2) = (0.0, 0.0);
    for v in r.nodes.iter().filter(|v| v.force > 0.0) {
        let x = point_reference(model, v);
        m0 += v.force;
        m2 += v.force * (x[0] * x[0] + x[1] * x[1]);
    }
    (2.5 * m2 / m0).sqrt()
}

/// The Hertz readings of a converged run: HLF2 and HLF3 (HS-LF), HLc (HS-L, the same limits).
fn hertz_checks(
    c: &mut Checks,
    tag: &str,
    model: &SolidModel,
    pres: &[rcfd_fem::constraint::Prescribed],
    r: &ContactResult,
    refs: (f64, f64),
) {
    let dofs = model.dof_map();
    let re = r.result.reactions().unwrap();
    let mut eqs: Vec<usize> = pres
        .iter()
        .filter(|p| p.component == 2)
        .map(|p| dofs.equation(p.node, 2).unwrap())
        .collect();
    eqs.sort_unstable();
    eqs.dedup();
    let f_fe = 4.0 * eqs.iter().map(|&e| re[e]).sum::<f64>().abs();
    let hz = hertz_read(model, r);
    let (a_h, p_h, appr) = hertz_theory(f_fe);
    let a_m = moment_radius(model, r);
    let s = r.steps.last().unwrap();
    println!(
        "{tag}: F_fe {f_fe:.6e} (4 sum t |n_z| {:.6e}), a_M {a_m:.6e}, a_H {a_h:.6e}, p_fit {:.6e}, p_H {p_h:.6e}, \
         penetration {:.3e}, {} solves ({} augmentations), contactor nodes {}, active {}",
        hz.force,
        hz.p_fit,
        hz.penetration,
        s.solves.len(),
        s.solves.len() - 1,
        r.nodes.len(),
        r.nodes.iter().filter(|n| n.active).count()
    );
    c.at_most(
        &format!("{tag} contact radius (moments)"),
        (a_m / a_h - 1.0).abs(),
        0.15,
    );
    c.at_most(&format!("{tag} peak pressure"), (hz.p_fit / p_h - 1.0).abs(), 0.20);
    c.at_most(&format!("{tag} approach"), appr, 0.10);
    c.at_most(
        &format!("{tag} F_fe against ADR 0570"),
        (f_fe / refs.0 - 1.0).abs(),
        1e-3,
    );
    c.at_most(&format!("{tag} a_M against ADR 0570"), (a_m / refs.1 - 1.0).abs(), 1e-3);
}

/// One heavy record: `X-0`, `X-L`, `X-LF` of ADR 0615 D3 on one mesh.
fn hertz_case(tag: &str, mesh: fn() -> SolidMesh, h: f64, node: &str, refs: (f64, f64)) {
    let mut c = Checks::new();
    // X-0: the default, with the recorder (DH).
    {
        let (model, pres, o) = hertz_setup(mesh(), false, false);
        let rec = Recorder::default();
        let t = std::time::Instant::now();
        let e = solve_with(&model, &pres, &[], &[sphere_pair(h)], &o, &[&rec]);
        let text = e.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
        println!("{tag}-0 ({:?}): {text}", t.elapsed());
        let segs = rec.segments(false);
        print_segments(&format!("{tag}-0"), &segs);
        c.holds(
            &format!("DH {tag}-0: ADR 0551's refusal"),
            matches!(&e, Err(FemError::Solver { solver, reason }) if *solver == "contact"
                && reason.contains("load step 1 of 1, augmentation 0")
                && reason.contains("singular tangent: pivot")
                && reason.contains(node)),
            &text,
        );
        c.holds(
            &format!("DH {tag}-0: the refusal at iteration 1 of the first solve"),
            segs.len() == 1 && segs[0].corrections.len() == 1,
            &format!("{segs:?}"),
        );
    }
    let mut alpha0 = Vec::new();
    for (run, fallback) in [("L", false), ("LF", true)] {
        let name = format!("{tag}-{run}");
        let (model, pres, o) = hertz_setup(mesh(), true, fallback);
        let rec = Recorder::default();
        let t = std::time::Instant::now();
        let r = solve_with(&model, &pres, &[], &[sphere_pair(h)], &o, &[&rec]);
        let elapsed = t.elapsed();
        let segs = rec.segments(r.is_ok());
        let a0 = segs.first().and_then(|s| s.corrections.first()).map(|x| x.alpha);
        match &r {
            Ok(r) => {
                println!(
                    "{name} ({elapsed:?}): converges; record {:?}; {} unknowns",
                    record(r),
                    model.dof_map().n_equations()
                );
                let first = r.steps[0].solves[0].step_lengths.first().copied();
                println!("{name}: alpha_0 recorder {a0:?}, step_lengths[0] {first:?}");
                alpha0.push((name.clone(), first.or(a0)));
                if run == "L" {
                    c.holds(&format!("HL {name}"), true, "converges");
                } else {
                    c.holds(&format!("HLF1 {name}: converges"), true, "");
                    let fb: usize = record(r).iter().map(|x| x.1).sum();
                    c.holds(
                        &format!("HLF4 {name}: fallback iterations <= 2"),
                        fb <= 2,
                        &format!("{fb}"),
                    );
                }
                hertz_checks(&mut c, &name, &model, &pres, r, refs);
                step_length_checks(&mut c, &name, r, &rec);
            }
            Err(e) => {
                println!("{name} ({elapsed:?}): fails: {e}");
                print_segments(&name, &segs);
                let located = segs.last().map(|s| (segs.len() - 1, s.corrections.len()));
                println!("{name}: the failure at (segment, iteration) {located:?}");
                alpha0.push((name.clone(), a0));
                let max_trials = segs
                    .iter()
                    .flat_map(|s| s.corrections.iter().map(|x| x.trials))
                    .max()
                    .unwrap_or(0);
                c.holds(
                    &format!("RT {name}: trials per correction <= 9"),
                    max_trials <= 9,
                    &format!("{max_trials}"),
                );
                if run == "L" {
                    c.holds(
                        &format!("HL {name}"),
                        located.is_some(),
                        &format!("fails at {located:?}: {e}"),
                    );
                } else {
                    c.holds(
                        &format!("HLF1 {name}: converges"),
                        false,
                        &format!("{e} at {located:?}"),
                    );
                }
            }
        }
    }
    for (name, a) in &alpha0 {
        match a {
            Some(a) => c.at_most(&format!("HK0 {name}: alpha_0"), *a, 0.9),
            None => c.holds(&format!("HK0 {name}: alpha_0"), false, "no first correction"),
        }
    }
    if let [(_, Some(x)), (_, Some(y))] = alpha0.as_slice() {
        println!("{tag}: alpha_0 of L and LF bit for bit: {}", x.to_bits() == y.to_bits());
    }
    c.finish();
}

#[test]
#[ignore = "heavy record of ADR 0615 (run once): HS20-0, HS20-L, HS20-LF"]
fn hs20_line_search_with_and_without_the_fallback() {
    hertz_case(
        "HS20",
        hs20_mesh,
        A_H / 4.0,
        "at node 1129 component 1",
        (1.4088e-3, 0.10118),
    );
}

#[test]
#[ignore = "heavy record of ADR 0615 (run once): HS10-0, HS10-L, HS10-LF"]
fn hs10_line_search_with_and_without_the_fallback() {
    hertz_case(
        "HS10",
        hs10_mesh,
        A_H / 3.0,
        "at node 931 component 1",
        (1.4159e-3, 0.09944),
    );
}
