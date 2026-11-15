//! F-617 (ADR 0539): the thermal predictor of the attempts of the nonlinear paths with a temperature field. Checks P1 to P5
//! (the start of an attempt: free expansion, prescribed values kept, no change without a change of the field, nonuniform fields
//! on three supports, the multiplicative form on a stretched state), FE1p, FE1c-p, FE1s-p, FE2p, FE0p, NC0, HB1p (the driver),
//! CONV, CONVp (the converged results do not depend on the start), INT1, INT2, INTn (the integrator), UBp and NCp (the bits
//! without a change, the predictor acts) and RFp (refusals) of the registration. Every aggregated error is required finite.

#[path = "pred_common/mod.rs"]
mod common;

use common::*;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{prescribed_equations, Prescribed};
use rcfd_fem::mesh::SolidFormulation;
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions};
use rcfd_fem::nonlinear::{solve_in_time_with, TimeHistory, TimeLoading, TimeSteppingOptions};
use rcfd_fem::thermal::{free_expansion_start, TemperatureFieldHistory};
use rcfd_fem::transient::{LoadTable, Rayleigh};
use rcfd_fem::{BlockProperty, Constraint, FemError, Material, SolidModel};

fn key_of(e: &FemError) -> Option<&'static str> {
    match e {
        FemError::InvalidValue { key, .. }
        | FemError::FieldLength { key, .. }
        | FemError::InvalidMaterial { key, .. } => Some(key),
        FemError::Element { source, .. } => key_of(source),
        _ => None,
    }
}

fn refused_with<T>(r: rcfd_fem::Result<T>, key: &str) -> (bool, String) {
    match r {
        Ok(_) => (false, "accepted".to_string()),
        Err(e) => (key_of(&e) == Some(key), format!("{e}")),
    }
}

fn run_driver(
    model: &SolidModel,
    field: TemperatureFieldHistory,
    predictor: bool,
    times: &[f64],
) -> rcfd_fem::Result<TimeHistory> {
    let opts = TimeSteppingOptions::new(f64::INFINITY)
        .newton(tight())
        .temperature_field(field.with_thermal_predictor(predictor));
    solve_in_time_with(model, &[], &TimeLoading::new(), times, &opts)
}

/// `max |u_p - u_n| / max |u_n|` over the entries of two histories.
fn relative_difference(p: &TimeHistory, n: &TimeHistory) -> f64 {
    assert_eq!(p.steps.len(), n.steps.len());
    let mut diff: f64 = 0.0;
    let mut scale: f64 = 0.0;
    for (a, b) in p.steps.iter().zip(&n.steps) {
        for (x, y) in a.u.iter().zip(&b.u) {
            worse(&mut diff, (x - y).abs());
            worse(&mut scale, y.abs());
        }
    }
    diff / scale
}

fn cube_with(sets: &[(&str, usize)]) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap();
    let a = node_at(&mesh, [0.0, 0.0, 0.0]);
    mesh.add_node_set("A", vec![a]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(ALPHA)).unwrap();
    for &(set, comp) in sets {
        m.add_constraint(Constraint::displacement(set, comp, 0.0)).unwrap();
    }
    m
}

// ------------------------------------------------------------------------------------------------- the start of an attempt

#[test]
fn p1_p5_the_start_of_an_attempt() {
    let mut c = Checks::new();
    let a = ALPHA * DT;
    let model = free_cube(steel(ALPHA));
    let n = model.dof_map().n_equations();
    let nn = model.mesh().nodes().len();
    let cold = uniform(&model, REF);
    let hot = uniform(&model, REF + DT);

    // P1: the free expansion of a uniform field from u = 0.
    let start = free_expansion_start(&model, &[], &vec![0.0; n], &cold, &hot, REF).unwrap();
    c.at_most("P1 start against alpha dT X", expansion_error(&model, &start, a), 1e-14);

    // P2: prescribed values kept (a constraint with a value and an extra prescribed value).
    let mut pm = free_cube(steel(ALPHA));
    pm.add_constraint(Constraint::displacement("B", 0, 3e-3)).unwrap();
    let dofs = pm.dof_map();
    let node_c = node_at(pm.mesh(), [0.0, 1.0, 0.0]);
    let extra = [Prescribed::new(node_c, 0, 1e-3)];
    let mut u_c = vec![0.0; n];
    for &(eq, v) in &prescribed_equations(&pm, &extra).unwrap() {
        u_c[eq] = v;
    }
    let start = free_expansion_start(&pm, &extra, &u_c, &cold, &hot, REF).unwrap();
    let fixed = prescribed_equations(&pm, &extra).unwrap();
    let differing = fixed
        .iter()
        .filter(|&&(eq, _)| start[eq].to_bits() != u_c[eq].to_bits())
        .count();
    let free_finite = start.iter().all(|v| v.is_finite());
    c.holds(
        "P2 prescribed values kept bit for bit, free entries finite",
        differing == 0 && free_finite && fixed.len() == 8 && dofs.n_equations() == n,
        &format!(
            "{differing} differing of {} fixed equations, finite {free_finite}",
            fixed.len()
        ),
    );

    // P3: no change of the field, or no expansion, leaves the start bit for bit.
    let mut u_x = expanded(&model, 0.37e-2);
    u_x[3] = -0.0;
    let nonuniform_a: Vec<f64> = (0..nn).map(|b| REF + 50.0 * r(b)).collect();
    let mut differing = 0usize;
    for (old, new) in [(&cold, &cold), (&hot, &hot), (&nonuniform_a, &nonuniform_a)] {
        let s = free_expansion_start(&model, &[], &u_x, old, new, REF).unwrap();
        differing += s.iter().zip(&u_x).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
    }
    let no_alpha = free_cube(steel(0.0));
    let s = free_expansion_start(&no_alpha, &[], &u_x, &cold, &hot, REF).unwrap();
    differing += s.iter().zip(&u_x).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
    c.holds(
        "P3 old = new and alpha = 0 leave the start unchanged",
        differing == 0,
        &format!("{differing} differing entries"),
    );

    // P4: nonuniform fields on three supports against the formula of D2 to D4 written out.
    let supports: Vec<(&str, SolidModel, Vec<(usize, usize)>)> = vec![
        (
            "3-2-1",
            free_cube(steel(ALPHA)),
            vec![(0, 0), (0, 1), (0, 2), (1, 1), (1, 2), (2, 2)],
        ),
        (
            "one pinned node",
            cube_with(&[("A", 0), ("A", 1), ("A", 2)]),
            vec![(0, 0), (0, 1), (0, 2)],
        ),
        ("x of one node", cube_with(&[("A", 0)]), vec![(0, 0)]),
    ];
    let t_old: Vec<f64> = (0..nn).map(|b| REF + 50.0 * r(b)).collect();
    let t_new: Vec<f64> = (0..nn).map(|b| REF + 50.0 * r(2 * b + 1)).collect();
    let mut p4: f64 = 0.0;
    for (name, m, fixed_nodes) in &supports {
        let dofs = m.dof_map();
        let mesh = m.mesh();
        let corners = [
            node_at(mesh, [0.0, 0.0, 0.0]),
            node_at(mesh, [1.0, 0.0, 0.0]),
            node_at(mesh, [0.0, 1.0, 0.0]),
        ];
        let mut u_c = vec![0.0; n];
        for (k, v) in u_c.iter_mut().enumerate() {
            *v = 1e-3 * r(k + 3);
        }
        // the fixed equations carry zero (the constraints are homogeneous)
        let fixed: Vec<usize> = fixed_nodes
            .iter()
            .map(|&(corner, comp)| dofs.equation(corners[corner], comp).unwrap())
            .collect();
        for &q in &fixed {
            u_c[q] = 0.0;
        }
        let start = free_expansion_start(m, &[], &u_c, &t_old, &t_new, REF).unwrap();
        // the formula written out
        let theta = |t: f64| 1.0 + ALPHA * (t - REF);
        let mut origin = [0.0; 3];
        for (node, x) in mesh.nodes().iter().enumerate() {
            for k in 0..3 {
                origin[k] += x[k] + u_c[dofs.equation(node, k).unwrap()];
            }
        }
        for o in &mut origin {
            *o /= nn as f64;
        }
        let d = |node: usize, k: usize| {
            let x = mesh.nodes()[node][k] + u_c[dofs.equation(node, k).unwrap()];
            (theta(t_new[node]) / theta(t_old[node]) - 1.0) * (x - origin[k])
        };
        let mut shift = [0.0; 3];
        for k in 0..3 {
            let mut s = 0.0;
            let mut cnt = 0;
            for &q in &fixed {
                let (node, comp) = dofs.node_component(q).unwrap();
                if comp == k {
                    s -= d(node, comp);
                    cnt += 1;
                }
            }
            if cnt > 0 {
                shift[k] = s / cnt as f64;
            }
        }
        let mut err: f64 = 0.0;
        for node in 0..nn {
            for k in 0..3 {
                let q = dofs.equation(node, k).unwrap();
                let expect = if fixed.contains(&q) {
                    u_c[q]
                } else {
                    u_c[q] + d(node, k) + shift[k]
                };
                worse(&mut err, (start[q] - expect).abs());
            }
        }
        println!("P4 {name}: {err:e}");
        worse(&mut p4, err);
    }
    c.at_most("P4 nonuniform start against the formula, three supports", p4, 1e-14);

    // P5: the multiplicative form on a stretched committed state.
    let e = 1e-2;
    let stretched = expanded(&model, e);
    let start = free_expansion_start(&model, &[], &stretched, &cold, &hot, REF).unwrap();
    let multiplicative: Vec<f64> = expanded(&model, e + a * (1.0 + e));
    let additive: Vec<f64> = expanded(&model, e + a);
    let mut m_err: f64 = 0.0;
    let mut a_dist: f64 = 0.0;
    for k in 0..n {
        worse(&mut m_err, (start[k] - multiplicative[k]).abs());
        worse(&mut a_dist, (start[k] - additive[k]).abs());
    }
    c.at_most("P5 start against u_c + a (1 + e) X", m_err, 1e-14);
    c.at_least("P5 distance to the additive start", a_dist, 1e-5);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- the driver

#[test]
fn fe1p_fe2p_fe0p_hb1p_the_driver() {
    let mut c = Checks::new();
    let a = ALPHA * DT;
    let model = free_cube(steel(ALPHA));

    // FE1p, FE1c-p, FE1s-p.
    let run = run_driver(&model, step_field(&model), true, &[1.0, 2.0, 3.0]).unwrap();
    let mut fe1: f64 = 0.0;
    let mut fe1s: f64 = 0.0;
    let mut halvings = 0;
    let mut corrections = 0;
    for s in &run.steps[1..] {
        worse(&mut fe1, expansion_error(&model, &s.u, a));
    }
    for s in &run.steps {
        worse(&mut fe1s, inf_norm(&s.internal_force) / (E_S * a));
        halvings = halvings.max(s.halvings);
        corrections = corrections.max(s.iterations);
    }
    let at0 = inf_norm(&run.steps[0].u) / a;
    println!(
        "FE1p entries {}, halvings {halvings}, corrections {corrections}, u(0)/a {at0:e}",
        run.steps.len()
    );
    c.at_most("FE1p free expansion with the predictor, |u - alpha dT X|/L", fe1, 1e-12);
    c.holds(
        "FE1c-p no halving, 4 entries, u(0) = 0, at most 2 corrections per entry",
        halvings == 0 && run.steps.len() == 4 && at0.is_finite() && at0 <= 1e-12 && corrections <= 2,
        &format!(
            "halvings {halvings}, entries {}, u(0)/a {at0:e}, corrections {corrections}",
            run.steps.len()
        ),
    );
    c.at_most("FE1s-p internal forces / (E alpha dT L^2)", fe1s, 1e-10);

    // NCp (driver): the predictor acts.
    let plain = run_driver(&model, step_field(&model), false, &[1.0, 2.0, 3.0]).unwrap();
    c.holds(
        "NCp driver: the run with the predictor differs from the run without",
        flat_history(&run)
            .iter()
            .map(|x| x.to_bits())
            .ne(flat_history(&plain).iter().map(|x| x.to_bits())),
        &format!(
            "halvings without: {}",
            plain.steps.iter().map(|s| s.halvings).max().unwrap()
        ),
    );

    // FE2p.
    let table = LoadTable::new(vec![(0.0, REF), (4.0, REF + DT)]).unwrap();
    let run = run_driver(&model, ramp_field(&model), true, &[1.0, 2.0, 3.0, 4.0]).unwrap();
    let mut fe2: f64 = 0.0;
    let mut h2 = 0;
    for s in &run.steps {
        let an = ALPHA * (table.value(s.time).unwrap() - REF);
        worse(&mut fe2, expansion_error(&model, &s.u, an));
        h2 = h2.max(s.halvings);
    }
    c.at_most("FE2p the ramp, |u - a_n X|/L at the step ends", fe2, 1e-12);
    c.holds("FE2p no halving", h2 == 0, &format!("halvings {h2}"));

    // FE0p and NC0: the table hot from t = 0.
    let hot = run_driver(&model, hot_field(&model), true, &[1.0, 2.0]);
    match &hot {
        Ok(run) => {
            let mut e0: f64 = 0.0;
            let mut h0 = 0;
            for s in &run.steps {
                worse(&mut e0, expansion_error(&model, &s.u, a));
                h0 = h0.max(s.halvings);
            }
            c.holds(
                "FE0p the hot start with the predictor, 3 entries, no halving",
                run.steps.len() == 3 && h0 == 0,
                &format!("entries {}, halvings {h0}", run.steps.len()),
            );
            c.at_most("FE0p |u - alpha dT X|/L including t = 0", e0, 1e-12);
        }
        Err(e) => c.holds("FE0p the hot start with the predictor", false, &format!("refused: {e}")),
    }
    let (refused, why) = match run_driver(&model, hot_field(&model), false, &[1.0, 2.0]) {
        Ok(_) => (false, "accepted".to_string()),
        Err(e) => (true, e.to_string()),
    };
    c.holds("NC0 the hot start without the predictor is refused", refused, &why);

    // HB1p: the held cube.
    let held = held_cube(steel(ALPHA));
    let run = run_driver(&held, step_field(&held), true, &[1.0, 2.0, 3.0]).unwrap();
    let theta = 1.0 + a;
    let sxx = -E_S / (1.0 - 2.0 * NU_S) * (theta * theta - 1.0) / (2.0 * theta);
    let dofs = held.dof_map();
    let xmax: Vec<usize> = held
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&n| dofs.equation(n, 0).unwrap())
        .collect();
    let mut hb1: f64 = 0.0;
    let mut hb1u: f64 = 0.0;
    for s in &run.steps[1..] {
        let sum: f64 = xmax.iter().map(|&q| s.internal_force[q]).sum();
        worse(&mut hb1, (sum - sxx).abs() / sxx.abs());
        worse(&mut hb1u, inf_norm(&s.u) / a);
    }
    c.at_most("HB1p held cube, the face force against the closed form", hb1, 1e-10);
    c.at_most("HB1p held cube, max |u| / (alpha dT L)", hb1u, 1e-12);
    c.finish();
}

#[test]
fn conv_the_converged_results_do_not_depend_on_the_start() {
    let mut c = Checks::new();
    // CONV: the nonuniform beam.
    let bm = beam(1e-5);
    let times = [1.0, 2.0, 3.0];
    let with = run_driver(&bm, beam_field(&bm), true, &times).unwrap();
    let without = run_driver(&bm, beam_field(&bm), false, &times).unwrap();
    c.at_most(
        "CONV nonuniform beam, relative difference of u with and without",
        relative_difference(&with, &without),
        1e-9,
    );
    // CONVp: the cube with a prescribed displacement.
    let mut pm = free_cube(steel(1e-5));
    pm.add_constraint(Constraint::displacement("B", 0, 1e-3)).unwrap();
    let times = [1.0, 2.0, 3.0, 4.0];
    let with = run_driver(&pm, ramp_field(&pm), true, &times).unwrap();
    let without = run_driver(&pm, ramp_field(&pm), false, &times).unwrap();
    c.at_most(
        "CONVp prescribed displacement, relative difference of u with and without",
        relative_difference(&with, &without),
        1e-9,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- the integrator

#[test]
fn int1_int2_intn_the_integrator() {
    let mut c = Checks::new();
    let a = ALPHA * DT;
    let model = free_cube(steel(ALPHA));
    let n = model.dof_map().n_equations();
    let dt = 0.05;
    let ramp =
        TemperatureFieldHistory::new(REF, vec![(0.0, uniform(&model, REF)), (1.0, uniform(&model, REF + DT))]).unwrap();
    let rate = expanded(&model, a);
    let opts = NlTransientOptions::new(dt).newton(tight());
    let with = vibrate(
        &model,
        &opts
            .clone()
            .temperature_field(ramp.clone().with_thermal_predictor(true)),
        &vec![0.0; n],
        &rate,
        20,
    );
    let without = vibrate(&model, &opts.temperature_field(ramp), &vec![0.0; n], &rate, 20);
    let mut int1: f64 = 0.0;
    for (k, u) in with.u.iter().enumerate() {
        worse(&mut int1, expansion_error(&model, u, a * dt * (k + 1) as f64));
    }
    let per_step = with.corrections.iter().copied().max().unwrap();
    c.at_most("INT1 free expansion under the ramp with the predictor", int1, 1e-12);
    c.holds(
        "INT1 no halving, at most 1 correction per step",
        with.halvings == 0 && per_step <= 1 && with.u.len() == 20,
        &format!("halvings {}, corrections per step {per_step}", with.halvings),
    );
    let (tw, tn): (usize, usize) = (with.corrections.iter().sum(), without.corrections.iter().sum());
    c.holds(
        "INT2 the corrections with the predictor are fewer than without",
        tw < tn,
        &format!("with {tw}, without {tn}"),
    );
    c.holds(
        "NCp integrator: the corrections differ",
        with.corrections != without.corrections,
        &format!("{:?} against {:?}", with.corrections, without.corrections),
    );

    // INTn: the bar of V3T with the nonuniform step.
    let b = bar(steel(1e-5));
    let dt = 2e-6;
    let o = NlTransientOptions::new(dt)
        .damping(Rayleigh {
            mass: 20.0,
            stiffness: 2.0e-8,
        })
        .newton(tight());
    let zero = vec![0.0; b.dof_map().n_equations()];
    let vb = v3t_velocity(&b);
    let field = v3t_field(&b, dt);
    let with = vibrate(
        &b,
        &o.clone().temperature_field(field.clone().with_thermal_predictor(true)),
        &zero,
        &vb,
        40,
    );
    let without = vibrate(&b, &o.temperature_field(field), &zero, &vb, 40);
    let mut diff: f64 = 0.0;
    let mut scale: f64 = 0.0;
    for (x, y) in with.u.iter().zip(&without.u) {
        for (p, q) in x.iter().zip(y) {
            worse(&mut diff, (p - q).abs());
            worse(&mut scale, q.abs());
        }
    }
    c.at_most(
        "INTn V3T bar, relative difference of u with and without",
        diff / scale,
        1e-9,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- bits and refusals

#[test]
fn ubp_the_bits_without_a_change_and_rfp_refusals() {
    let mut c = Checks::new();
    // UBp, driver: alpha = 0 under a ramp, and the reference-constant field with an expansion.
    let m0 = free_cube(steel(0.0));
    let times = [1.0, 2.0, 3.0, 4.0];
    let on = run_driver(&m0, ramp_field(&m0), true, &times).unwrap();
    let off = run_driver(&m0, ramp_field(&m0), false, &times).unwrap();
    let same0 = flat_history(&on)
        .iter()
        .map(|x| x.to_bits())
        .eq(flat_history(&off).iter().map(|x| x.to_bits()));
    let m1 = free_cube(steel(ALPHA));
    let cons = TemperatureFieldHistory::constant(REF, uniform(&m1, REF), 0.0, 4.0).unwrap();
    let on = run_driver(&m1, cons.clone(), true, &times).unwrap();
    let off = run_driver(&m1, cons, false, &times).unwrap();
    let same1 = flat_history(&on)
        .iter()
        .map(|x| x.to_bits())
        .eq(flat_history(&off).iter().map(|x| x.to_bits()));
    c.holds(
        "UBp driver: alpha = 0 ramp and the reference-constant field give the bits without the predictor",
        same0 && same1,
        &format!("alpha 0 {same0}, constant {same1}"),
    );

    // UBp, integrator.
    let n = m0.dof_map().n_equations();
    let ramp =
        TemperatureFieldHistory::new(REF, vec![(0.0, uniform(&m0, REF)), (1.0, uniform(&m0, REF + DT))]).unwrap();
    let o = NlTransientOptions::new(0.05).newton(tight());
    let v0 = expanded(&m0, 1e-3);
    let on = vibrate(
        &m0,
        &o.clone().temperature_field(ramp.clone().with_thermal_predictor(true)),
        &vec![0.0; n],
        &v0,
        10,
    );
    let off = vibrate(&m0, &o.clone().temperature_field(ramp), &vec![0.0; n], &v0, 10);
    let same_i0 = on.bits == off.bits && on.bytes == off.bytes;
    let cons = TemperatureFieldHistory::constant(REF, uniform(&m1, REF), 0.0, 1.0).unwrap();
    let on = vibrate(
        &m1,
        &o.clone().temperature_field(cons.clone().with_thermal_predictor(true)),
        &vec![0.0; n],
        &v0,
        10,
    );
    let off = vibrate(&m1, &o.temperature_field(cons), &vec![0.0; n], &v0, 10);
    let same_i1 = on.bits == off.bits && on.bytes == off.bytes;
    c.holds(
        "UBp integrator: alpha = 0 ramp and the reference-constant field give the bits without the predictor",
        same_i0 && same_i1,
        &format!("alpha 0 {same_i0}, constant {same_i1}"),
    );

    // RFp.
    let model = free_cube(steel(ALPHA));
    let nn = model.mesh().nodes().len();
    let default_off = !TemperatureFieldHistory::constant(REF, uniform(&model, REF), 0.0, 1.0)
        .unwrap()
        .thermal_predictor();
    let f_off = TemperatureFieldHistory::constant(REF, uniform(&model, REF), 0.0, 1.0).unwrap();
    let f_on = f_off.clone().with_thermal_predictor(true);
    c.holds(
        "RFp default off, the flag takes part in the comparison",
        default_off && f_on.thermal_predictor() && f_on != f_off && f_on.clone().with_thermal_predictor(false) == f_off,
        "",
    );
    let mut mesh = hex_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap();
    mesh.set_block_property(0, BlockProperty::solid(SolidFormulation::BBar))
        .unwrap();
    let bb = three_two_one(mesh, 1.0, 1.0, steel(ALPHA));
    let field = TemperatureFieldHistory::constant(REF, uniform(&bb, REF + DT), 0.0, 2.0).unwrap();
    let (ok, d) = refused_with(run_driver(&bb, field, true, &[1.0]), "temperature");
    c.holds("RFp driver with a BBar block and the predictor", ok, &d);
    let wrong = TemperatureFieldHistory::constant(REF, vec![REF; nn - 1], 0.0, 2.0).unwrap();
    let (ok, d) = refused_with(run_driver(&model, wrong.clone(), true, &[1.0]), "temperature");
    c.holds("RFp driver with a field of the wrong length and the predictor", ok, &d);
    let (ok, d) = refused_with(
        NlIntegrator::new(
            &model,
            &[],
            &NlTransientOptions::new(1e-5).temperature_field(wrong.with_thermal_predictor(true)),
        ),
        "temperature",
    );
    c.holds(
        "RFp integrator with a field of the wrong length and the predictor",
        ok,
        &d,
    );
    let n = model.dof_map().n_equations();
    let (ok, d) = refused_with(
        free_expansion_start(&model, &[], &vec![0.0; n], &vec![REF; nn - 1], &vec![REF; nn], REF),
        "temperature",
    );
    c.holds("RFp free_expansion_start with an old field of the wrong length", ok, &d);
    let cold_model = free_cube(Material::isotropic_elastic(E_S, NU_S, 7850.0, 1e-2).unwrap());
    let (ok, d) = refused_with(
        free_expansion_start(
            &cold_model,
            &[],
            &vec![0.0; n],
            &vec![REF; nn],
            &vec![REF - 150.0; nn],
            REF,
        ),
        "temperature",
    );
    c.holds("RFp free_expansion_start with a non-positive stretch", ok, &d);
    let (ok, d) = refused_with(
        free_expansion_start(&model, &[], &vec![0.0; n - 1], &vec![REF; nn], &vec![REF; nn], REF),
        "displacement",
    );
    c.holds(
        "RFp free_expansion_start with a displacement of the wrong length",
        ok,
        &d,
    );
    c.finish();
}
