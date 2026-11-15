//! F-624 (ADR 0566): the Newton driver's supernodal factor reuses its nested dissection order and symbolic analysis while
//! the reduced tangent's pattern is unchanged. V0 (ADR 0542's hub at `n = 4`, its recorded bits), A1 and A1b (the counts of
//! analyses), A2 (the bits against the path without reuse), the negative controls N1 and N3, and the heavy record at `n =
//! 16` (T1, V16, D, R; Q is sampled by `f624-scratch/heavy.ps1`). Every limit is the one registered in ADR 0566.

mod f624_common;

use f624_common::*;
use rcfd_fem::element::ElementState;
use rcfd_fem::error::Result as FemResult;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::nonlinear::{
    initial_states, solve_from, solve_from_reusing, Contribution, Equilibrium, FactorReuse, NewtonOptions,
};
use rcfd_fem::solve::DirectFactor;
use rcfd_fem::SolidModel;

/// V0 of ADR 0566: ADR 0542's hub at `n = 4`, `d = 1e-4 a`, by both routes: ADR 0542 C1's digest.
#[test]
fn f624_v0_tl_hub_n4_adr0542_bits() {
    let fit = fit_model(Setup { n: 4, d: D_TL, ..BASE });
    let mut c = Checks::new();
    for route in [Route::Contact, Route::Newton] {
        let s = solve_fit(&fit, route);
        let d = digest(&flat(&s.result));
        println!("V0 {route:?}: {d:#018x} (ADR 0542 C1: 0xecd8f40c5721a14f)");
        c.holds(&format!("V0 {route:?}: ADR 0542's bits"), d == 0xecd8_f40c_5721_a14f);
    }
    c.finish();
}

/// ADR 0566 A1b: no force; the tangent entries `(p, q)` and `(q, p)` of value 0, off the pattern.
struct OffPattern {
    p: usize,
    q: usize,
}

impl Contribution for OffPattern {
    fn name(&self) -> &str {
        "off-pattern"
    }
    fn add(
        &self,
        _u: &[f64],
        _load_factor: f64,
        _force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> FemResult<()> {
        tangent.push((self.p, self.q, 0.0));
        tangent.push((self.q, self.p, 0.0));
        Ok(())
    }
}

/// The two calls of A1: `0.5 f` from 0, then `f` from the first result (the second call with `second`), by `call`.
fn two_calls(
    m: &SolidModel,
    second: &[&dyn Contribution],
    call: &dyn Fn(&[f64], &[f64], &[ElementState], &[&dyn Contribution]) -> Equilibrium,
) -> [Equilibrium; 2] {
    let f = nodal_load_vector(m, &[]).unwrap();
    let half: Vec<f64> = f.iter().map(|v| 0.5 * v).collect();
    let s0 = initial_states(m).unwrap();
    let u0 = vec![0.0; f.len()];
    let e1 = call(&half, &u0, &s0, &[]);
    let e2 = call(&f, &e1.u, &e1.states, second);
    [e1, e2]
}

fn eq_bits(x: &Equilibrium) -> Vec<u64> {
    x.u.iter()
        .chain(&x.internal_force)
        .chain(&x.residuals)
        .chain(&x.energies)
        .map(|v| v.to_bits())
        .collect()
}

fn corrections(e: &[Equilibrium; 2]) -> usize {
    e.iter().map(|x| x.residuals.len() - 1).sum()
}

/// A1, A1b, A2, N1 and N3 of ADR 0566 on ADR 0542 Q5's cantilever.
#[test]
fn f624_a_counts_and_bits_cantilever() {
    let (m, tip) = cantilever();
    let mut c = Checks::new();
    let sn = NewtonOptions::new(1).factor(DirectFactor::SupernodalNestedDissection);
    // a free equation of a node near x = 1, far from the tip's elements
    let near = (0..m.mesh().nodes().len())
        .find(|&n| {
            let p = m.mesh().nodes()[n];
            (p[0] - 1.0).abs() < 1e-9 && (p[1] - 0.5).abs() < 1e-9 && (p[2] - 0.5).abs() < 1e-9
        })
        .unwrap();
    let q = m.dof_map().equation(near, 1).unwrap();
    let off = OffPattern { p: tip, q };
    let run = |o: &NewtonOptions, reuse: Option<&FactorReuse>, second: &[&dyn Contribution]| {
        two_calls(&m, second, &|load, start, states, list| match reuse {
            Some(r) => solve_from_reusing(&m, &[], load, start, states, o, list, r).unwrap(),
            None => solve_from(&m, &[], load, start, states, o, list).unwrap(),
        })
    };
    // A1
    let r1 = FactorReuse::new();
    let a1 = run(&sn, Some(&r1), &[]);
    println!(
        "A1: analyses {}, factorisations {}, corrections {}",
        r1.analyses(),
        r1.factorisations(),
        corrections(&a1)
    );
    c.holds("A1: one analysis", r1.analyses() == 1);
    c.holds(
        "A1: a factorisation per correction",
        r1.factorisations() == corrections(&a1),
    );
    // A1b
    let r2 = FactorReuse::new();
    let a1b = run(&sn, Some(&r2), &[&off]);
    println!(
        "A1b: analyses {}, factorisations {}, corrections {}",
        r2.analyses(),
        r2.factorisations(),
        corrections(&a1b)
    );
    c.holds("A1b: two analyses (the pattern changed)", r2.analyses() == 2);
    c.holds("N3: A1b's count is not A1's", r2.analyses() != r1.analyses());
    // A2
    let plain = run(&sn, None, &[]);
    let plain_b = run(&sn, None, &[&off]);
    for k in 0..2 {
        c.holds(
            &format!("A2 A1 call {}: bit for bit without the reuse", k + 1),
            eq_bits(&a1[k]) == eq_bits(&plain[k]),
        );
        c.holds(
            &format!("A2 A1b call {}: bit for bit without the reuse", k + 1),
            eq_bits(&a1b[k]) == eq_bits(&plain_b[k]),
        );
    }
    // N1
    let r3 = FactorReuse::new();
    let _ = run(&NewtonOptions::new(1), Some(&r3), &[]);
    println!(
        "N1: envelope analyses {}, factorisations {}",
        r3.analyses(),
        r3.factorisations()
    );
    c.holds(
        "N1: the envelope factor does not use the reuse",
        r3.analyses() == 0 && r3.factorisations() == 0,
    );
    c.finish();
}

/// The heavy record of ADR 0566: ADR 0542 D4's hub at `n = 16`, present and supernodal alternating five times in this
/// process: T1 (the ratio of the minima), V16 (ADR 0542's bits), D (a pool of 1 thread) and R. Run once by
/// `f624-scratch/heavy.ps1`, which samples Q.
#[test]
#[ignore = "heavy record of ADR 0566 (about 4 minutes), run once by f624-scratch/heavy.ps1"]
fn f624_tl_hub_n16_reuse_against_present_heavy() {
    let fit = fit_model(Setup { n: 16, d: D_TL, ..BASE });
    println!("R: rayon threads {}", rayon::current_num_threads());
    let mut ps = Vec::new();
    let mut ss = Vec::new();
    for _ in 0..5 {
        ps.push(solve_fit(&fit, Route::Present));
        ss.push(solve_fit(&fit, Route::Contact));
    }
    let times = |v: &[Solve]| {
        let mut t: Vec<f64> = v.iter().map(|x| x.seconds).collect();
        t.sort_by(|a, b| a.total_cmp(b));
        t
    };
    let (tp, ts) = (times(&ps), times(&ss));
    for (k, (p, s)) in ps.iter().zip(&ss).enumerate() {
        println!(
            "R pair {}: t_p = {:.2} s, t_s = {:.2} s, ratio {:.4}",
            k + 1,
            p.seconds,
            s.seconds,
            s.seconds / p.seconds
        );
    }
    println!(
        "R: min t_p = {:.2} s, min t_s = {:.2} s; median t_p = {:.2} s, median t_s = {:.2} s, ratio of medians {:.4}",
        tp[0],
        ts[0],
        tp[2],
        ts[2],
        ts[2] / tp[2]
    );
    let mut c = Checks::new();
    below(&mut c, "T1 n = 16: min(t_s)/min(t_p)", ts[0] / tp[0], 1.0 / 3.0);
    let dp: Vec<u64> = ps.iter().map(|x| digest(&flat(&x.result))).collect();
    let ds: Vec<u64> = ss.iter().map(|x| digest(&flat(&x.result))).collect();
    println!("V16 present digests {dp:x?}, supernodal {ds:x?}");
    c.holds(
        "V16 the five supernodal solves: ADR 0542's 0xfc936c7553ff35d5",
        ds.iter().all(|&x| x == 0xfc93_6c75_53ff_35d5),
    );
    c.holds(
        "V16 the five present solves: ADR 0542's 0xa5faff8918362a96",
        dp.iter().all(|&x| x == 0xa5fa_ff89_1836_2a96),
    );
    let pool = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap();
    let t0 = std::time::Instant::now();
    let one = pool.install(|| digest(&flat(&solve_fit(&fit, Route::Contact).result)));
    println!("D: 1 thread {one:#018x} in {:.2} s", t0.elapsed().as_secs_f64());
    c.holds(
        "D n = 16: the 1-thread supernodal solve has ADR 0542's bits",
        one == 0xfc93_6c75_53ff_35d5,
    );
    c.finish();
}
