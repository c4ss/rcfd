//! F-601 (ADR 0540 D1): the bucket search against the all-faces loop. EQ1 on the models of the existing contact tests
//! (solved), EQ2 at the states of D5, BT the speed on ADR 0517's hub at `n = 32`, NS the faces visited. Every limit is the one
//! registered in ADR 0540.

mod f601_common;

use f601_common::*;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::contact::{
    solve, ContactOptions, ContactPair, ContactResult, ContactSet, Kinematics, SearchMethod, Target,
};
use rcfd_fem::nonlinear::NewtonOptions;
use rcfd_fem::{Load, SolidModel};
use std::time::Instant;

fn newton() -> NewtonOptions {
    NewtonOptions::new(1).max_iterations(30)
}

/// Solves with both methods; whether every word is equal and every value finite.
fn both(
    c: &mut Checks,
    name: &str,
    m: &SolidModel,
    prescribed: &[Prescribed],
    pair: &ContactPair,
    o: &ContactOptions,
) -> ContactResult {
    let ra = solve(
        m,
        prescribed,
        &[],
        std::slice::from_ref(pair),
        &o.clone().search_method(SearchMethod::AllFaces),
    )
    .unwrap();
    let rb = solve(
        m,
        prescribed,
        &[],
        std::slice::from_ref(pair),
        &o.clone().search_method(SearchMethod::Buckets),
    )
    .unwrap();
    let (wa, wb) = (result_words(&ra), result_words(&rb));
    let equal = wa == wb;
    let finite = result_finite(&ra) && result_finite(&rb);
    c.holds(
        &format!("EQ1 {name}"),
        equal && finite,
        &format!("{} words, equal {equal}, finite {finite}", wa.len()),
    );
    rb
}

#[test]
fn eq1_existing_models_buckets_against_all_faces() {
    let mut c = Checks::new();
    // ADR 0507 B1.
    let mut m = blocks(0.0);
    m.add_load(Load::pressure("upper.zmax", 1.0e8)).unwrap();
    let block_pair = ContactPair::new("upper.zmin", Target::Surface("lower.zmax".into()), 100.0 * E_BL / 0.5);
    both(
        &mut c,
        "B1 blocks",
        &m,
        &[],
        &block_pair,
        &ContactOptions::new(2, 1e-12).newton(newton()),
    );
    // ADR 0507 S1 (the a3 set-up).
    let d = 1e-3;
    let m = blocks(d);
    let top: Vec<Prescribed> = m
        .mesh()
        .node_set("upper.zmax")
        .unwrap()
        .iter()
        .map(|&n| Prescribed::new(n, 2, 2.0 * d))
        .collect();
    both(
        &mut c,
        "S1 blocks",
        &m,
        &top,
        &block_pair,
        &ContactOptions::new(8, 1e-13).newton(newton()),
    );
    // ADR 0507 I (the a3 set-up).
    let m = ring_fit_model();
    let pair = ContactPair::new("ring_in", Target::Surface("shaft_out".into()), 100.0 * E_FIT / 0.125);
    both(
        &mut c,
        "I fit",
        &m,
        &[],
        &pair,
        &ContactOptions::new(1, 1e-9).newton(newton()),
    );
    // ADR 0507 T2, total Lagrangian.
    let m = punch_model(0.01);
    let pair = ContactPair::new("punch.zmin", Target::Surface("plate.zmax".into()), 800.0);
    let tl = ContactOptions::new(2, 1e-8)
        .newton(newton())
        .kinematics(Kinematics::TotalLagrangian);
    both(&mut c, "T2 punch", &m, &[], &pair, &tl);
    // ADR 0519 M3: the punch with an initial gap on part of it.
    let mut pair3 = pair.clone();
    for id in 0..m.mesh().nodes().len() {
        let x = m.mesh().nodes()[id];
        if x[0] > 1.0 && x[0] <= 1.2 + 1e-9 && x[1] <= 0.2 + 1e-9 && (x[2] - 0.25).abs() < 1e-12 {
            pair3 = pair3.initial_gap(id, -3e-4);
        }
    }
    both(&mut c, "M3 punch", &m, &[], &pair3, &tl);
    // ADR 0517's hub at n = 4.
    let m = hub_model(4);
    let pair = ContactPair::new("bore", Target::Surface("shaft_out".into()), 100.0 * E_HUB / hub_h(4));
    both(
        &mut c,
        "hub n = 4",
        &m,
        &[],
        &pair,
        &ContactOptions::new(1, 1e-6 * hub_d()).newton(newton()),
    );
    c.finish();
}

/// EQ2 on one model: every word of every state equal; the number of changes of a point's face across the states.
fn states(c: &mut Checks, name: &str, m: &SolidModel, pair: &ContactPair, amp: f64, reach: Option<f64>) -> usize {
    let mut o = ContactOptions::new(1, 0.0);
    if let Some(d) = reach {
        o = o.search_distance(d);
    }
    let sa = ContactSet::new(
        m,
        std::slice::from_ref(pair),
        &o.clone().search_method(SearchMethod::AllFaces),
    )
    .unwrap();
    let sb = ContactSet::new(m, std::slice::from_ref(pair), &o.search_method(SearchMethod::Buckets)).unwrap();
    let n = m.dof_map().n_equations();
    let (mut equal, mut finite, mut words) = (true, true, 0);
    let mut faces: Option<Vec<Option<rcfd_fem::FaceRef>>> = None;
    let mut changes = 0;
    let mut found = 0;
    for seed in 0..20u64 {
        let u = state(n, amp, seed);
        let (na, nb) = (sa.evaluate(&u).unwrap(), sb.evaluate(&u).unwrap());
        let (mut wa, mut wb) = (Vec::new(), Vec::new());
        for v in &na {
            node_words(v, &mut wa);
            finite &= v.force.is_finite() && v.gap.map_or(true, |g| g.is_finite());
            found += v.face.is_some() as usize;
        }
        for v in &nb {
            node_words(v, &mut wb);
        }
        words += wa.len();
        equal &= wa == wb;
        let f: Vec<_> = nb.iter().map(|v| v.face).collect();
        if let Some(prev) = &faces {
            changes += prev.iter().zip(&f).filter(|(a, b)| a != b).count();
        }
        faces = Some(f);
    }
    c.holds(
        &format!("EQ2 {name}"),
        equal && finite,
        &format!(
            "{words} words, equal {equal}, finite {finite}; {found} point states with a face, {changes} face changes"
        ),
    );
    changes
}

#[test]
fn eq2_states_buckets_against_all_faces() {
    let mut c = Checks::new();
    let m = hub_model(8);
    let h = hub_h(8);
    let pair = ContactPair::new("bore", Target::Surface("shaft_out".into()), 100.0 * E_HUB / h);
    let mut hub_changes = states(&mut c, "hub n = 8, default reach", &m, &pair, 0.3 * h, None);
    hub_changes += states(&mut c, "hub n = 8, reach 0.1 h", &m, &pair, 0.3 * h, Some(0.1 * h));
    c.at_least("EQ2 hub face changes", hub_changes as f64, 10.0);
    // NS: the faces visited by the buckets at the hub's states (default reach) against all faces.
    let sb = ContactSet::new(&m, std::slice::from_ref(&pair), &ContactOptions::new(1, 0.0)).unwrap();
    let mut frac = Vec::new();
    for seed in 0..20u64 {
        let (visited, points, faces) = sb
            .visited_faces(&state(m.dof_map().n_equations(), 0.3 * h, seed))
            .unwrap();
        if seed == 0 {
            println!("NS n = 8, state 0: {visited} faces visited for {points} points ({faces} in all)");
        }
        frac.push(visited as f64 / faces as f64);
    }
    c.less("NS visited fraction", worst(frac), 0.25);
    let m = punch_model(0.01);
    let pair = ContactPair::new("punch.zmin", Target::Surface("plate.zmax".into()), 800.0);
    states(&mut c, "punch, default reach", &m, &pair, 0.05, None);
    states(&mut c, "punch, reach 0.02", &m, &pair, 0.05, Some(0.02));
    let m = two_hex(T1_CONTACTOR);
    let pair = ContactPair::new("contactor", Target::Surface("target".into()), 10.0);
    states(&mut c, "T1 pair, default reach", &m, &pair, 0.05, None);
    states(&mut c, "T1 pair, reach 0.02", &m, &pair, 0.05, Some(0.02));
    c.finish();
}

#[test]
fn bt_ns_hub_n32_speed_and_visited_faces() {
    let mut c = Checks::new();
    let m = hub_model(32);
    let h = hub_h(32);
    let pair = ContactPair::new("bore", Target::Surface("shaft_out".into()), 100.0 * E_HUB / h);
    let o = ContactOptions::new(1, 1e-6 * hub_d());
    let sa = ContactSet::new(
        &m,
        std::slice::from_ref(&pair),
        &o.clone().search_method(SearchMethod::AllFaces),
    )
    .unwrap();
    let sb = ContactSet::new(&m, std::slice::from_ref(&pair), &o.search_method(SearchMethod::Buckets)).unwrap();
    let n = m.dof_map().n_equations();
    let us = [vec![0.0; n], state(n, 0.3 * h, 1)];
    let (mut ta, mut tb) = (0.0, 0.0);
    let mut equal = true;
    for u in &us {
        for _ in 0..5 {
            let t0 = Instant::now();
            let na = sa.evaluate(u).unwrap();
            ta += t0.elapsed().as_secs_f64();
            let t0 = Instant::now();
            let nb = sb.evaluate(u).unwrap();
            tb += t0.elapsed().as_secs_f64();
            let (mut wa, mut wb) = (Vec::new(), Vec::new());
            na.iter().for_each(|v| node_words(v, &mut wa));
            nb.iter().for_each(|v| node_words(v, &mut wb));
            equal &= wa == wb;
        }
    }
    println!(
        "BT hub n = 32: {} contactor nodes, all faces {ta:.4} s, buckets {tb:.4} s (10 evaluations each)",
        sb.nodes(0).len()
    );
    c.less("BT t_b / t_a", tb / ta, 0.5);
    c.holds("BT results", equal, &format!("bit for bit {equal}"));
    for u in &us {
        let (visited, points, faces) = sb.visited_faces(u).unwrap();
        let (va, _, fa) = sa.visited_faces(u).unwrap();
        println!("NS record, n = 32: buckets visit {visited} faces for {points} points ({faces} in all); all faces {va} of {fa}");
    }
    c.finish();
}
