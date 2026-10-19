//! Particle tracking driven by the run loop (F-056, ADR 0037 D1; checks W1 to W8). The limits are those registered in the ADR
//! before this file was written; none was changed afterwards.

use rcfd_api::case::Case;
use rcfd_api::particle_run;
use rcfd_api::particles;
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_api::sim::Simulation;
use rcfd_particles::State;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_prun_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// 20 x 20 lid-driven box, `algorithm` piso (40 steps of 0.01) or simple (60 iterations), with a point injection of 10
/// fluid-velocity parcels (rate 50/s until t = 0.2) tracked to t = 0.4 (piso) or 0.3 (simple).
fn text(algorithm: &str, particles_on: bool, extra_particles: &str) -> String {
    let (solver, end) = if algorithm == "piso" {
        ("algorithm = \"piso\"\ndt = 0.01\nend_time = 0.4\n", 0.4)
    } else {
        (
            // this SIMPLE set-up (schema relaxation 0.9/1.0) diverges; ADR 0077: keep the run going to its cap as before the steady guard
            "algorithm = \"simple\"\nmax_iterations = 60\ntolerance_u = 1e-14\ntolerance_p = 1e-14\ndt = 0.01\nsteady_divergence_guard = \"off\"\n",
            0.3,
        )
    };
    format!(
        r#"
title = "particle run"
[mesh]
generator = "box_2d"
nx = 20
ny = 20
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
velocity_value = [1.0, 0.0, 0.0]
[solver]
{solver}
[particles]
enabled = {particles_on}
density = 1000.0
drag = "schiller_naumann"
integration = "analytic"
gravity = false
end_time = {end}
dt = 0.01
{extra_particles}
[particles.injection.p]
kind = "point"
position = [0.3, 0.7, 0.0]
rate = 50.0
end_time = 0.2
velocity_mode = "fluid"
diameter = 1.0e-3
"#
    )
}

fn parse(t: &str) -> Case {
    Case::parse_toml(t).unwrap_or_else(|e| panic!("{e:?}"))
}

/// The run path of the library: `drive` plus `write_results` into `dir`.
fn run_path(t: &str, dir: &Path) -> Simulation {
    let sim = Simulation::new(&parse(t)).unwrap_or_else(|e| panic!("{e:?}"));
    let m = Mutex::new(sim);
    let opts = RunOptions {
        out_dir: Some(dir.to_path_buf()),
        ..Default::default()
    };
    let out = runctl::drive(&m, &opts, &|| false, &mut |_| {});
    assert!(out.exit_code == 0 || out.exit_code == 5, "{out:?}");
    let sim = m.into_inner().unwrap();
    runctl::write_results(&sim, dir).unwrap();
    sim
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn flow_bits(sim: &Simulation) -> Vec<u64> {
    let mut out = Vec::new();
    for v in &sim.velocity().values {
        out.extend(v.iter().map(|x| x.to_bits()));
    }
    out.extend(bits(&sim.pressure().values));
    out.extend(bits(sim.flux()));
    out
}

fn sorted_names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn w1_transient_run_path_equals_library_hook_bit_for_bit() {
    let dir = tmp("w1");
    let sim_a = run_path(&text("piso", true, "output_every = 7\n"), &dir);
    let pr = sim_a.particle_run().expect("transient run owns a particle run");
    // library hook on a second simulation of the same case
    let out_b = tmp("w1b").join("p.vtp");
    let extra = format!("output_file = {:?}\noutput_every = 7\n", out_b.to_string_lossy());
    let tb = text("piso", true, &extra);
    let case_b = parse(&tb);
    let mut sim_b = Simulation::new(&case_b).unwrap();
    let trk = particles::track_transient(&case_b, &mut sim_b).unwrap();
    let (fa, fb) = (pr.tracker.fingerprint(), trk.fingerprint());
    println!(
        "W1 fingerprint words {} / {}, injected {}",
        fa.len(),
        fb.len(),
        trk.injected()
    );
    assert_eq!(fa, fb, "tracker fingerprints differ");
    let a = std::fs::read(dir.join("particles.vtp")).unwrap();
    let b = std::fs::read(&out_b).unwrap();
    assert_eq!(a, b, "particles.vtp differs");
    // the numbered files too (every 7th tracking step; 40 steps -> 5 files)
    for n in [7, 14, 21, 28, 35] {
        let fa = std::fs::read(dir.join(format!("particles_{n}.vtp"))).unwrap();
        let fb = std::fs::read(out_b.with_file_name(format!("p_{n}.vtp"))).unwrap();
        assert_eq!(fa, fb, "particles_{n}.vtp differs");
    }
    assert!(!dir.join("particles_42.vtp").exists());
    // the flow of the library run equals the flow of the run path (the hook is not run twice on a detached simulation)
    assert_eq!(flow_bits(&sim_a), flow_bits(&sim_b));
}

#[test]
fn w2_steady_run_path_equals_track_frozen() {
    let dir = tmp("w2");
    let sim_a = run_path(&text("simple", true, ""), &dir);
    assert!(
        sim_a.particle_run().is_none(),
        "steady runs are tracked at result-writing time"
    );
    let out_b = tmp("w2b").join("p.vtp");
    let extra = format!("output_file = {:?}\n", out_b.to_string_lossy());
    let case_b = parse(&text("simple", true, &extra));
    let mut sim_b = Simulation::new(&case_b).unwrap();
    sim_b.run(10_000);
    let trk = particles::track_frozen(&case_b, &sim_b).unwrap();
    let run = particle_run::steady_run(&sim_a, None).unwrap();
    assert_eq!(run.tracker.fingerprint(), trk.fingerprint());
    assert_eq!(
        std::fs::read(dir.join("particles.vtp")).unwrap(),
        std::fs::read(&out_b).unwrap()
    );
    assert_eq!(trk.injected(), 10);
}

#[test]
fn w3_flow_untouched_and_baseline_files_unchanged() {
    for alg in ["piso", "simple"] {
        let d1 = tmp(&format!("w3a_{alg}"));
        let d2 = tmp(&format!("w3b_{alg}"));
        let on = run_path(&text(alg, true, ""), &d1);
        let off = run_path(&text(alg, false, ""), &d2);
        assert_eq!(
            flow_bits(&on),
            flow_bits(&off),
            "{alg}: flow differs with particles enabled"
        );
        // W4: the disabled run holds exactly the default file list
        assert_eq!(
            sorted_names(&d2),
            vec!["monitors.csv", "monitors.json", "report.html", "solution.vtu"],
            "{alg}"
        );
        let names = sorted_names(&d1);
        assert!(names.contains(&"particles.vtp".to_string()) && names.contains(&"particle_report.json".to_string()));
        assert!(!names.contains(&"particle_tracks.csv".to_string()));
    }
}

fn diag_paths(t: &str) -> Vec<String> {
    match Case::parse_toml(t) {
        Err(d) => d.iter().map(|x| x.path.clone()).collect(),
        Ok(c) => {
            let mut d = c.semantic_errors();
            if d.is_empty() {
                if let Err(e) = Simulation::new(&c) {
                    d = e;
                }
            }
            for x in &d {
                assert!(!x.message.is_empty());
            }
            d.iter().map(|x| x.path.clone()).collect()
        }
    }
}

/// The dynamic-mesh case: the conflict with particles is reported at `dynamic_mesh.enabled` by the earlier ADR 0033 check
/// (registered here as `particles.enabled`; deviation recorded in the ADR addendum).
fn error_cases() -> Vec<(&'static str, String)> {
    let dyn_mesh = r#"
[dynamic_mesh]
enabled = true
moving_patches = ["xmin"]
deformation = "smoothing"
displacement_expression_x = "1 [m/s^2] * t^2"
fixed_patches = ["xmax"]
"#;
    vec![
        (
            "dynamic_mesh.enabled",
            format!("{}{}", text("piso", true, ""), dyn_mesh),
        ),
        (
            "particles.injection.p.position",
            text("piso", true, "").replace("[0.3, 0.7, 0.0]", "[5.0, 5.0, 0.0]"),
        ),
        (
            "particles.injection.p.patch",
            text("piso", true, "")
                .replace("kind = \"point\"", "kind = \"surface\"\npatch = \"nope\"")
                .replace("position = [0.3, 0.7, 0.0]\n", ""),
        ),
        (
            "particles.walls.nope",
            format!(
                "{}\n[particles.walls.nope]\ninteraction = \"stick\"\n",
                text("piso", true, "")
            ),
        ),
        (
            "particles.track_every",
            text("piso", true, "tracks = true\ntrack_every = 0\n"),
        ),
    ]
}

#[test]
fn w5_located_errors() {
    for (path, t) in error_cases() {
        let p = diag_paths(&t);
        assert!(p.iter().any(|x| x == path), "{path}: got {p:?}");
    }
}

/// Used by the CLI test through `include!`-free duplication: the error cases are also written to files there.
#[test]
fn w6_report_and_tracks_are_exact() {
    let dir = tmp("w6");
    let sim = run_path(&text("piso", true, "tracks = true\ntrack_every = 3\n"), &dir);
    let pr = sim.particle_run().unwrap();
    let rep: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("particle_report.json")).unwrap()).unwrap();
    let c = &rep["counts"];
    let n = |k: &str| c[k].as_u64().unwrap();
    let injected = rep["injected"].as_u64().unwrap();
    assert_eq!(n("active") + n("deposited") + n("escaped") + n("stuck"), injected);
    assert_eq!(injected, (50.0f64 * (0.2 - 0.0)).floor() as u64);
    assert_eq!(injected, 10);
    assert_eq!(n("active"), 10, "{rep}");
    assert_eq!(rep["steps"].as_u64().unwrap(), 40);
    // tracks
    let csv = std::fs::read_to_string(dir.join("particle_tracks.csv")).unwrap();
    let mut lines = csv.lines();
    assert_eq!(lines.next().unwrap(), "id,time,x,y,z,state");
    let rows: Vec<(u64, f64, [f64; 3], String)> = lines
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            (
                f[0].parse().unwrap(),
                f[1].parse().unwrap(),
                [f[2].parse().unwrap(), f[3].parse().unwrap(), f[4].parse().unwrap()],
                f[5].to_string(),
            )
        })
        .collect();
    let mut total_expected = 0usize;
    for p in pr.particles() {
        let mine: Vec<_> = rows.iter().filter(|r| r.0 == p.id).collect();
        assert!(!mine.is_empty(), "parcel {} has no row", p.id);
        // injection time of the registered rule: parcel m (1-based) is due at m / rate
        let m = (p.id + 1) as f64;
        let due = m / 50.0;
        let s_m = (mine[0].1 / 0.01).round() as usize;
        assert!((mine[0].1 - 0.01 * s_m as f64).abs() < 1e-9);
        assert!(
            0.01 * s_m as f64 >= due - 1e-9 && 0.01 * (s_m as f64 - 1.0) < due + 1e-9,
            "parcel {}: first row at {}",
            p.id,
            mine[0].1
        );
        // last row = final position, bit for bit of the printed shortest round-trip numbers
        let last = mine.last().unwrap();
        assert_eq!(bits(&last.2), bits(&p.x), "parcel {} last row", p.id);
        assert_eq!(last.1, pr.tracker.time);
        // expected rows: the injection step, every third later step, and a closing row at step 40
        let mut expect = 1;
        for s in (s_m + 1)..=40 {
            if s % 3 == 0 || s == 40 {
                expect += 1;
            }
        }
        assert_eq!(mine.len(), expect, "parcel {} row count", p.id);
        total_expected += expect;
    }
    assert_eq!(rows.len(), total_expected);
    // service-style report with tracks carries the same rows
    let with = pr.report(true);
    assert_eq!(with["tracks"].as_array().unwrap().len(), rows.len());
    assert_eq!(pr.tracker.count(State::Active), 10);
}

#[test]
fn w7_service_headless_and_interactive_agree_with_the_run_path() {
    let dir = tmp("w7ref");
    let sim_a = run_path(&text("piso", true, ""), &dir);
    let ref_report = sim_a.particle_run().unwrap().report(false);
    let ref_vtp = std::fs::read(dir.join("particles.vtp")).unwrap();
    let root = tmp("w7root");
    let svc = Service::new(ServiceConfig {
        root: Some(root.clone()),
        ..Default::default()
    });
    let sid = svc
        .call("session.create", &json!({"toml": text("piso", true, "")}))
        .unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    let r = svc
        .call("run.headless", &json!({"session": sid, "out_dir": "res"}))
        .unwrap();
    assert_eq!(r["outcome"]["state"], "finished", "{r}");
    assert_eq!(std::fs::read(root.join("res/particles.vtp")).unwrap(), ref_vtp);
    let rep = svc.call("particles.report", &json!({"session": sid})).unwrap();
    assert_eq!(rep, ref_report);
    // interactive worker
    let sid2 = svc
        .call("session.create", &json!({"toml": text("piso", true, "")}))
        .unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    svc.call("run.start", &json!({"session": sid2})).unwrap();
    for _ in 0..200 {
        let st = svc
            .call("run.wait", &json!({"session": sid2, "timeout_ms": 5000}))
            .unwrap();
        if st["state"] != "running" {
            break;
        }
    }
    let rep2 = svc.call("particles.report", &json!({"session": sid2})).unwrap();
    assert_eq!(rep2["counts"], ref_report["counts"]);
    assert_eq!(rep2["time"], ref_report["time"]);
    assert_eq!(rep2, ref_report);
    // a session without particles is a state error
    let sid3 = svc
        .call("session.create", &json!({"toml": text("piso", false, "")}))
        .unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(svc.call("particles.report", &json!({"session": sid3})).is_err());
    let methods = svc.call("service.info", &json!({})).unwrap()["methods"].to_string();
    assert!(methods.contains("particles.report"));
}

#[test]
fn w8_checkpoint_honesty() {
    let on = Simulation::new(&parse(&text("piso", true, ""))).unwrap();
    let off = Simulation::new(&parse(&text("piso", false, ""))).unwrap();
    assert!(!on.supports_restart());
    assert!(off.supports_restart());
    let dir = tmp("w8");
    let m = Mutex::new(on);
    let opts = RunOptions {
        checkpoint: Some(dir.join("ck.rcfd")),
        checkpoint_every: 10,
        ..Default::default()
    };
    let mut warned = false;
    runctl::drive(&m, &opts, &|| false, &mut |ev: &Value| {
        if ev["event"] == "warning"
            && ev["message"]
                .as_str()
                .is_some_and(|s| s.contains("cannot be restarted"))
        {
            warned = true;
        }
    });
    assert!(warned, "no 'cannot be restarted' warning");
    assert!(!dir.join("ck.rcfd").exists());
}

#[test]
fn error_case_texts_for_the_binary_are_exported() {
    // The binary test (rcfd-cli) writes the same error cases; keep a copy of their paths here so a change shows up in both.
    let paths: Vec<&str> = error_cases().iter().map(|c| c.0).collect();
    assert_eq!(
        paths,
        [
            "dynamic_mesh.enabled",
            "particles.injection.p.position",
            "particles.injection.p.patch",
            "particles.walls.nope",
            "particles.track_every"
        ]
    );
}

#[test]
fn w4_write_rates_is_inert_without_chemistry() {
    let d = tmp("w4");
    let t = format!("{}\n[chemistry]\nwrite_rates = true\n", text("piso", false, ""));
    let _ = run_path(&t, &d);
    assert_eq!(
        sorted_names(&d),
        vec!["monitors.csv", "monitors.json", "report.html", "solution.vtu"]
    );
    let vtu = std::fs::read_to_string(d.join("solution.vtu")).unwrap();
    assert!(!vtu.contains("wdot_") && !vtu.contains("heat_release_rate"));
}
