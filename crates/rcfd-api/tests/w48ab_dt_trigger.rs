//! ADR 0162 (job 48AB): the harness of the registered runs A (AB256), B (AB256-off, the control) and C (C64, C128, C256).
//!
//! Each run is the shipped `cylinder-re100.toml` with the Studio plan's full `--set` list (ADR 0153, 0158) as tree overrides:
//! `solver.end_time=N`, `solver.dt=dt0`, `solver.max_courant=0.5`, `solver.max_dt=N/100`, `solver.corrector_control=adaptive`,
//! plus the mesh, driven through `Simulation` in-process (the control needs `Simulation::set_corrector_dt_factor`). Every run is
//! an ignored test of its own; it writes `<W48AB_WORK>/runs/<id>/solver-status.jsonl` (`iteration` events that
//! `scripts/w48v_shedding.py` reads, `store` and `mark` events with `M_hf`, `lag_trial` events, a `cost` and a `finish` event),
//! refuses to overwrite an existing run folder, and asserts its registered limits after writing everything.
//!
//! `cargo test --release -p rcfd-api --test w48ab_dt_trigger -- --ignored --exact w48ab_ab256 --nocapture`
//! (`RAYON_NUM_THREADS` sets the threads; results do not depend on it).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use serde_json::{json, Value};
use std::io::Write;
use std::time::Instant;

/// The shipped example case with `overrides` applied.
fn case_with(overrides: &[(&str, Value)]) -> Case {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../example-projects/wind-tunnel/cases/cylinder-re100.toml"
    );
    let mut case = Case::parse_toml(&std::fs::read_to_string(path).expect("example case readable"))
        .unwrap_or_else(|e| panic!("{e:?}"));
    for (k, v) in overrides {
        case.tree.set(k, v.clone()).unwrap_or_else(|e| panic!("{k}: {e:?}"));
    }
    case
}

// ------------------------------------------------------------------------- M_hf (copied from w48t_cylinder_spokes.rs, ADR 0154)

const Q: f64 = 0.5;

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

/// M_hf of ADR 0154 on the rings with centre radius in [0.55, 0.70] (precomputed by [`rings`]).
fn m_hf(p: &[f64], rings: &[(f64, Vec<usize>)]) -> f64 {
    if !p.iter().all(|x| x.is_finite()) {
        return f64::INFINITY;
    }
    let (mut sum, mut cnt) = (0.0, 0usize);
    for (rj, g) in rings {
        if !(0.55..=0.70).contains(rj) {
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

// ------------------------------------------------------------------------------------------------ the run matrix

/// A registered run of ADR 0162.
struct Arm {
    id: &'static str,
    nx: u32,
    ny: u32,
    /// The Studio's first step (ADR 0153's table).
    dt0: f64,
    /// Simulated time N.
    end: f64,
    /// The dt trigger on (A, C) or off (B).
    trigger: bool,
    /// A and B: `M_hf` every 0.1 s and the 60-iteration trials at t = 1, 3, 10.
    dense: bool,
    /// Wall cap of the stepping, s.
    cap_s: f64,
}

const H: f64 = 3600.0;

fn arm(id: &str) -> Arm {
    let a = |id, nx, ny, dt0, end, trigger, dense, cap_h: f64| Arm {
        id,
        nx,
        ny,
        dt0,
        end,
        trigger,
        dense,
        cap_s: cap_h * H,
    };
    match id {
        "AB256" => a("AB256", 256, 256, 0.00116, 10.0, true, true, 6.0),
        "AB256-off" => a("AB256-off", 256, 256, 0.00116, 10.0, false, true, 6.0),
        "C64" => a("C64", 64, 48, 0.00602, 100.0, true, false, 2.0),
        "C128" => a("C128", 128, 96, 0.00307, 100.0, true, false, 6.0),
        "C256" => a("C256", 256, 256, 0.00116, 100.0, true, false, 14.0),
        _ => panic!("unknown run {id}"),
    }
}

/// The Studio plan's `--set` list for the arm (ADR 0153 D2, ADR 0158), plus the mesh.
fn studio_overrides(a: &Arm) -> Vec<(&'static str, Value)> {
    vec![
        ("mesh.nx", json!(a.nx)),
        ("mesh.ny", json!(a.ny)),
        ("solver.end_time", json!(a.end)),
        ("solver.dt", json!(a.dt0)),
        ("solver.max_courant", json!(0.5)),
        ("solver.max_dt", json!(a.end / 100.0)),
        ("solver.corrector_control", json!("adaptive")),
    ]
}

/// The clean limit at every stored time (ADR 0156 C3).
const CLEAN: f64 = 0.0114;

/// What a run measured (for the asserts).
struct Outcome {
    diverged: bool,
    time: f64,
    /// (time, M_hf) at the stored times and marks.
    m: Vec<(f64, f64)>,
    /// (time, count, rho) of the 60-iteration trials.
    trials: Vec<(f64, usize, f64)>,
    summary: Value,
    cost_dt_3: f64,
    cost_dt_10: f64,
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn run(id: &str) -> Outcome {
    let a = arm(id);
    let work = std::env::var("W48AB_WORK").unwrap_or_else(|_| r"E:\projects\cfd\w48ab-work".to_string());
    let dir = std::path::Path::new(&work).join("runs").join(a.id);
    assert!(
        !dir.join("solver-status.jsonl").exists(),
        "{} exists; refusing to overwrite a registered run",
        dir.display()
    );
    std::fs::create_dir_all(&dir).unwrap();
    let mut out = std::io::BufWriter::new(std::fs::File::create(dir.join("solver-status.jsonl")).unwrap());
    let mut emit = |v: Value, flush: bool| {
        writeln!(out, "{v}").unwrap();
        if flush {
            out.flush().unwrap();
        }
    };
    let ov = studio_overrides(&a);
    let mut sim = Simulation::new(&case_with(&ov)).unwrap_or_else(|e| panic!("{e:?}"));
    if !a.trigger {
        assert!(
            sim.set_corrector_dt_factor(0.0),
            "the test switch needs an adaptive run"
        );
    }
    let threads = std::env::var("RAYON_NUM_THREADS").unwrap_or_default();
    emit(
        json!({"event": "start", "id": a.id, "set": ov.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>(),
               "trigger": a.trigger, "cells": sim.geom.n_cells(), "threads": threads}),
        true,
    );
    let rg = rings(sim.geom.mesh.cell_centres());
    let t0 = Instant::now();
    let (mut steps, mut prev_t) = (0usize, 0.0f64);
    let mut step_s: Vec<(f64, f64)> = Vec::new(); // (time, wall s of the step)
    let mut next_store = 1.0;
    let mut next_mark = 0.1;
    let mut trial_at: Vec<f64> = if a.dense { vec![1.0, 3.0, 10.0] } else { Vec::new() };
    let mut m: Vec<(f64, f64)> = Vec::new();
    let mut trials: Vec<(f64, usize, f64)> = Vec::new();
    let mut stepping = 0.0f64;
    let trial = |sim: &Simulation, emit: &mut dyn FnMut(Value, bool), trials: &mut Vec<(f64, usize, f64)>| {
        let ts = Instant::now();
        if let Some(w) = sim.lag_estimate_next_step(60) {
            emit(
                json!({"event": "lag_trial", "time": sim.time(), "correctors": w.n_correctors, "rho": w.rho,
                       "cell": w.cell, "position": w.position, "wall_s": ts.elapsed().as_secs_f64()}),
                true,
            );
            trials.push((sim.time(), w.n_correctors, w.rho));
        }
    };
    while !sim.is_done() && stepping < a.cap_s {
        let ts = Instant::now();
        let sm = sim.step();
        let ds = ts.elapsed().as_secs_f64();
        stepping += ds;
        steps += 1;
        let t = sm.time;
        let dt = t - prev_t;
        prev_t = t;
        step_s.push((t, ds));
        let mon: serde_json::Map<String, Value> = sim
            .step_report_values()
            .into_iter()
            .map(|(k, v)| (k, json!(v)))
            .collect();
        let mut ev = json!({"event": "iteration", "index": sm.index, "time": t, "dt": dt, "res_u": sm.res_u,
                            "res_p": sm.res_p, "monitors": mon, "step_s": ds, "wall_s": t0.elapsed().as_secs_f64()});
        if let Some((n, cap)) = sim.corrector_status() {
            ev["correctors"] = json!(n);
            ev["corrector_cap"] = json!(cap);
        }
        emit(ev, steps % 200 == 0);
        if sim.diverged() {
            break;
        }
        let tol = 1e-9;
        while t >= next_store - tol {
            let p = sim.field("p").unwrap();
            let v = m_hf(&p, &rg);
            emit(json!({"event": "store", "time": t, "at": next_store, "m_hf": v}), true);
            m.push((t, v));
            next_store += 1.0;
        }
        if a.dense {
            while t >= next_mark - tol {
                let p = sim.field("p").unwrap();
                let v = m_hf(&p, &rg);
                emit(json!({"event": "mark", "time": t, "at": next_mark, "m_hf": v}), false);
                m.push((t, v));
                next_mark = ((next_mark * 10.0).round() + 1.0) / 10.0;
            }
            while trial_at.first().is_some_and(|&x| t >= x - tol && !sim.is_done()) {
                trial_at.remove(0);
                trial(&sim, &mut emit, &mut trials);
            }
        }
    }
    if a.dense && !trial_at.is_empty() && sim.is_done() && !sim.diverged() {
        // the trial at t = 10: after the last step
        trial(&sim, &mut emit, &mut trials);
    }
    let summary = sim.corrector_summary().unwrap_or(Value::Null);
    // E: the cost of an estimate step = its wall time minus the median of the 10 nearest steps without an estimate
    let log = summary["log"].as_array().cloned().unwrap_or_default();
    let est_steps: Vec<(usize, bool, u64)> = log
        .iter()
        .map(|e| {
            (
                e[0].as_u64().unwrap() as usize,
                e[6].as_bool().unwrap_or(false),
                e[7].as_u64().unwrap_or(0),
            )
        })
        .collect();
    let is_est = |k: usize| est_steps.iter().any(|e| e.0 == k);
    let mut costs: Vec<Value> = Vec::new();
    let (mut cost_dt_3, mut cost_dt_10, mut cost_all_10) = (0.0, 0.0, 0.0);
    for &(k, dtf, work) in &est_steps {
        if k == 0 || k > step_s.len() {
            continue;
        }
        let mut near: Vec<(usize, f64)> = (1..=step_s.len())
            .filter(|&j| j != k && !is_est(j))
            .map(|j| (j.abs_diff(k), step_s[j - 1].1))
            .collect();
        near.sort_by_key(|x| x.0);
        let base = median(near.iter().take(10).map(|x| x.1).collect());
        let (tk, sk) = step_s[k - 1];
        let c = (sk - base).max(0.0);
        if tk <= 10.0 + 1e-9 {
            cost_all_10 += c;
            if dtf {
                cost_dt_10 += c;
                if tk <= 3.0 + 1e-9 {
                    cost_dt_3 += c;
                }
            }
        }
        costs.push(json!({"step": k, "time": tk, "dt_triggered": dtf, "work": work, "step_s": sk, "base_s": base, "cost_s": c}));
    }
    let wall_to = |x: f64| -> f64 { step_s.iter().filter(|s| s.0 <= x + 1e-9).map(|s| s.1).sum() };
    let (w3, w10) = (wall_to(3.0), wall_to(10.0));
    let n_sum = summary["steps"].as_u64().unwrap_or(0);
    let applications = summary["mean"].as_f64().unwrap_or(0.0) * n_sum as f64;
    let work_dt: u64 = est_steps.iter().filter(|e| e.1).map(|e| e.2).sum();
    let work_all: u64 = est_steps.iter().map(|e| e.2).sum();
    emit(
        json!({"event": "cost", "estimates": costs, "steps_wall_s_to_3": w3, "steps_wall_s_to_10": w10,
               "cost_dt_to_3": cost_dt_3, "cost_dt_to_10": cost_dt_10, "cost_all_to_10": cost_all_10,
               "share_dt_to_3": cost_dt_3 / w3, "share_dt_to_10": cost_dt_10 / w10, "share_all_to_10": cost_all_10 / w10,
               "work_dt": work_dt, "work_all": work_all, "step_corrector_applications": applications}),
        true,
    );
    let max_m = m.iter().fold(0.0f64, |x, y| x.max(y.1));
    emit(
        json!({"event": "finish", "state": if sim.diverged() { "diverged" } else if sim.is_done() { "finished" } else { "wall_cap" },
               "time": sim.time(), "iterations": steps, "stepping_s": stepping, "wall_s": t0.elapsed().as_secs_f64(),
               "max_m_hf": max_m, "correctors": summary}),
        true,
    );
    drop(emit);
    println!(
        "{}: {steps} steps to t = {}, diverged {}, largest M_hf {max_m:.3e}, trials {trials:?}, dt-fired cost {:.1} s of {:.1} s \
         to t = 3, {:.1} s of {:.1} s to t = 10; summary {summary}",
        a.id,
        sim.time(),
        sim.diverged(),
        cost_dt_3,
        w3,
        cost_dt_10,
        w10
    );
    Outcome {
        diverged: sim.diverged(),
        time: sim.time(),
        m,
        trials,
        summary,
        cost_dt_3: cost_dt_3 / w3,
        cost_dt_10: cost_dt_10 / w10,
    }
}

/// C1: not stopped, the end reached, `M_hf <= 0.0114` at every stored time.
fn assert_clean(id: &str, o: &Outcome, end: f64) {
    assert!(!o.diverged, "{id}: stopped by the guard at t = {}", o.time);
    assert!((o.time - end).abs() < 1e-9, "{id}: t = {} (wall cap?)", o.time);
    for &(t, v) in &o.m {
        assert!(v <= CLEAN, "{id}: M_hf {v:.4e} > {CLEAN} at t = {t}");
    }
}

/// A (ADR 0162): 256 x 256 under the Studio plan with the dt trigger, N = 10: A1-A6, E1, E2.
#[test]
#[ignore = "ADR 0162 A (heavy: 256 x 256 to t = 10, about an hour or more on one thread)"]
fn w48ab_ab256() {
    let o = run("AB256");
    assert_clean("AB256", &o, 10.0);
    let log = o.summary["log"].as_array().unwrap();
    let dt_fired: Vec<&Value> = log.iter().filter(|e| e[6] == true).collect();
    assert!(
        dt_fired.iter().any(|e| e[1].as_u64().unwrap() > 2),
        "A3: no dt-fired estimate chose more than 2 correctors: {}",
        o.summary
    );
    for e in log {
        assert!(e[2].as_f64().unwrap() <= 0.5 && e[3] == false, "A4: {e}");
    }
    assert_eq!(o.trials.len(), 3, "A5: trials {:?}", o.trials);
    for &(t, n, rho) in &o.trials {
        assert!(rho < 1.0, "A5: {n} correctors at t = {t}: {rho}");
    }
    assert!(dt_fired.len() <= 6, "A6: {} dt-fired estimates", dt_fired.len());
    assert!(o.cost_dt_3 <= 0.10, "E1: {:.4}", o.cost_dt_3);
    assert!(o.cost_dt_10 <= 0.04, "E2: {:.4}", o.cost_dt_10);
}

/// B (ADR 0162): the control, AB256 with the dt trigger off; reported by its reading rule, no limit.
#[test]
#[ignore = "ADR 0162 B, the control (heavy: 256 x 256 to t = 10)"]
fn w48ab_ab256_off() {
    let o = run("AB256-off");
    let spokes = o.m.iter().any(|x| x.1 > CLEAN);
    let lag = o
        .trials
        .iter()
        .any(|&(t, n, rho)| t <= 3.0 + 1e-9 && n == 2 && rho >= 1.0);
    println!(
        "B: class {} (spokes {spokes}, 2-corrector estimate >= 1 at t <= 3 {lag}); trials {:?}",
        if spokes {
            "B-i"
        } else if lag {
            "B-ii"
        } else {
            "B-iii"
        },
        o.trials
    );
}

/// C (ADR 0162): the resolution sweep under the Studio plan, N = 100; C1 here, C2 by `scripts/w48v_shedding.py`.
#[test]
#[ignore = "ADR 0162 C (64 x 48 to t = 100)"]
fn w48ab_c64() {
    let o = run("C64");
    assert_clean("C64", &o, 100.0);
}

#[test]
#[ignore = "ADR 0162 C (heavy: 128 x 96 to t = 100)"]
fn w48ab_c128() {
    let o = run("C128");
    assert_clean("C128", &o, 100.0);
}

#[test]
#[ignore = "ADR 0162 C (heavy: 256 x 256 to t = 100, several hours)"]
fn w48ab_c256() {
    let o = run("C256");
    assert_clean("C256", &o, 100.0);
}
