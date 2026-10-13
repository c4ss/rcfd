//! Run-time statistics and output policy (F-116).
//!
//! Checks fixed before running:
//! * the accumulated mean, rms, std, min and max of ux, uy and p of a transient run with adaptive
//!   time steps equal the same quantities computed afterwards from the stored per-step fields
//!   (same definitions: end-of-step values weighted by the step width after `start_time`), to a
//!   relative 1e-12 of the field scale;
//! * the long vortex-shedding variant (cylinder, Re = 100, ignored test) does the same for a wake
//!   probe cell and the whole mean field, and the probe velocity must change sign several times
//!   (shedding present);
//! * `output.write_interval`/`retain_snapshots` leave exactly the newest N numbered snapshots;
//!   `retain_checkpoints` leaves the newest N periodic restart files plus the end-of-run file;
//!   `output.fields` removes unselected variables from the VTK file;
//! * statistics are refused for steady algorithms and unknown field names are diagnosed.

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, exit, RunOptions};
use rcfd_api::sim::Simulation;
use std::path::PathBuf;
use std::sync::Mutex;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_stats_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const TRANSIENT: &str = r#"
[mesh]
generator = "cavity_2d"
nx = 14
ny = 14
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "piso"
dt = 0.1
max_courant = 0.2
end_time = 1.0
[statistics]
enabled = true
start_time = 0.15
fields = ["ux", "uy", "p", "speed"]
"#;

#[test]
fn statistics_equal_the_post_computed_moments() {
    let case = Case::parse_toml(TRANSIENT).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    let names = ["ux", "uy", "p", "speed"];
    let mut snaps: Vec<(f64, Vec<Vec<f64>>)> = Vec::new();
    while !sim.is_done() {
        sim.step();
        snaps.push((sim.time(), names.iter().map(|n| sim.field(n).unwrap()).collect()));
    }
    assert!(snaps.len() > 8);
    // adaptive steps: the widths differ
    let widths: Vec<f64> = snaps.windows(2).map(|w| w[1].0 - w[0].0).collect();
    assert!(
        widths.iter().any(|w| (w - widths[0]).abs() > 1e-6),
        "time steps were uniform"
    );
    let start = 0.15;
    let mut t_prev: f64 = 0.0;
    let mut w = Vec::new();
    for (t, _) in &snaps {
        w.push((t - t_prev.max(start)).max(0.0));
        t_prev = *t;
    }
    let total: f64 = w.iter().sum();
    assert!((sim.statistics_time() - total).abs() < 1e-12);
    let mut worst: f64 = 0.0;
    for (k, n) in names.iter().enumerate() {
        let nc = snaps[0].1[k].len();
        let (mut mean, mut msq, mut var, mut lo, mut hi) = (
            vec![0.0; nc],
            vec![0.0; nc],
            vec![0.0; nc],
            vec![f64::INFINITY; nc],
            vec![f64::NEG_INFINITY; nc],
        );
        for (s, (_, v)) in snaps.iter().enumerate() {
            for c in 0..nc {
                mean[c] += w[s] * v[k][c] / total;
                msq[c] += w[s] * v[k][c] * v[k][c] / total;
                if w[s] > 0.0 {
                    lo[c] = lo[c].min(v[k][c]);
                    hi[c] = hi[c].max(v[k][c]);
                }
            }
        }
        for (s, (_, v)) in snaps.iter().enumerate() {
            for c in 0..nc {
                var[c] += w[s] * (v[k][c] - mean[c]).powi(2) / total;
            }
        }
        let scale = |x: &[f64]| x.iter().fold(1e-12_f64, |m, v| m.max(v.abs()));
        let cmp = |got: Vec<f64>, want: Vec<f64>| -> f64 {
            let sc = scale(&want);
            got.iter()
                .zip(&want)
                .map(|(a, b)| (a - b).abs() / sc)
                .fold(0.0, f64::max)
        };
        worst = worst
            .max(cmp(sim.field(&format!("{n}_mean")).unwrap(), mean))
            .max(cmp(
                sim.field(&format!("{n}_rms")).unwrap(),
                msq.iter().map(|v| v.sqrt()).collect(),
            ))
            .max(cmp(
                sim.field(&format!("{n}_std")).unwrap(),
                var.iter().map(|v| v.sqrt()).collect(),
            ))
            .max(cmp(sim.field(&format!("{n}_min")).unwrap(), lo))
            .max(cmp(sim.field(&format!("{n}_max")).unwrap(), hi));
    }
    eprintln!("statistics vs post-computed moments: worst relative difference {worst:e}");
    assert!(worst < 1e-12, "{worst:e}");
    // the statistics fields are written to the VTK file
    let d = tmp("vtu");
    let path = sim.export_vtu(&d, "solution").unwrap();
    let text = std::fs::read_to_string(path).unwrap();
    for n in ["ux_mean", "p_std", "speed_max", "uy_rms", "p_min"] {
        assert!(text.contains(&format!("Name=\"{n}\"")), "{n} missing");
    }
}

#[test]
fn statistics_settings_are_checked() {
    let steady = TRANSIENT.replace("piso", "simplec");
    let e = Case::parse_toml(&steady).expect_err("steady + statistics");
    assert!(e.iter().any(|d| d.path == "statistics.enabled"), "{e:?}");
    let bad = TRANSIENT.replace("\"speed\"", "\"vorticity\"");
    let e = Case::parse_toml(&bad).expect_err("unknown field");
    assert!(e.iter().any(|d| d.message.contains("vorticity")), "{e:?}");
    let bad = format!("{TRANSIENT}\n[output]\nfields = [\"U\", \"bogus\"]\n");
    let e = Case::parse_toml(&bad).expect_err("unknown output variable");
    assert!(e.iter().any(|d| d.path == "output.fields"), "{e:?}");
}

const STEADY: &str = r#"
[mesh]
generator = "cavity_2d"
nx = 12
ny = 12
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 400
tolerance_u = 1e-7
tolerance_p = 1e-7
"#;

fn names_in(dir: &std::path::Path, prefix: &str) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.starts_with(prefix))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

#[test]
fn snapshots_and_checkpoints_are_retained_by_count() {
    let d = tmp("retain");
    let text = format!("{STEADY}[output]\nwrite_interval = 10\nretain_snapshots = 3\nretain_checkpoints = 2\n");
    let sim = Simulation::new(&Case::parse_toml(&text).unwrap()).unwrap();
    let m = Mutex::new(sim);
    let ck = d.join("restart.rcfd");
    let opts = RunOptions {
        checkpoint: Some(ck.clone()),
        checkpoint_every: 15,
        out_dir: Some(d.clone()),
        ..Default::default()
    };
    let mut events = Vec::new();
    let out = runctl::drive(&m, &opts, &|| false, &mut |e| events.push(e.clone()));
    assert_eq!(out.exit_code, exit::OK, "{out:?}");
    let n = out.iterations;
    assert!(n >= 60, "run too short for the test: {n}");
    // newest three multiples of 10
    let mut want: Vec<String> = (1..=n / 10).map(|k| format!("snapshot_{:08}.vtu", k * 10)).collect();
    let want: Vec<String> = want.split_off(want.len() - 3);
    assert_eq!(names_in(&d.join("snapshots"), "snapshot_"), want);
    assert!(events.iter().filter(|e| e["event"] == "snapshot").count() >= 3);
    // newest two multiples of 15, plus the end-of-run file at the requested path
    let mut want: Vec<String> = (1..=n / 15).map(|k| format!("restart.rcfd.{}", k * 15)).collect();
    let mut want = want.split_off(want.len() - 2);
    want.sort();
    let mut got = names_in(&d, "restart.rcfd.");
    got.sort();
    assert_eq!(got, want);
    assert!(ck.exists(), "end-of-run checkpoint missing");
}

#[test]
fn output_fields_select_the_written_variables() {
    let d = tmp("fields");
    let text = format!("{STEADY}[output]\nfields = [\"U\"]\n");
    let mut sim = Simulation::new(&Case::parse_toml(&text).unwrap()).unwrap();
    sim.run(5);
    let path = sim.export_vtu(&d, "solution").unwrap();
    let t = std::fs::read_to_string(path).unwrap();
    assert!(t.contains("Name=\"U\""));
    assert!(!t.contains("Name=\"p\"") && !t.contains("Name=\"speed\""));
    // default: everything
    let mut sim = Simulation::new(&Case::parse_toml(STEADY).unwrap()).unwrap();
    sim.run(5);
    let t = std::fs::read_to_string(sim.export_vtu(&d, "all").unwrap()).unwrap();
    assert!(t.contains("Name=\"U\"") && t.contains("Name=\"p\"") && t.contains("Name=\"speed\""));
}

/// Vortex shedding behind a cylinder at Re = 100 through the settings tree: the mean field of the
/// statistics equals the average of the stored fields. Run with
/// `cargo test --release -p rcfd-api --test statistics -- --ignored shedding --nocapture`.
#[test]
#[ignore = "heavy: about 3200 PISO steps on a cylinder grid"]
fn shedding_run_mean_field_matches_the_post_computed_average() {
    let text = r#"
[mesh]
generator = "cylinder_ogrid"
nx = 128
ny = 100
r_in = 0.5
r_out = 20.0
grading = 150.0
depth = 0.1
[physics]
nu = 0.01
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.cylinder]
velocity = "fixed"
[boundaries.farfield]
velocity = "freestream"
velocity_value = [1.0, 0.0, 0.0]
pressure = "freestream"
[solver]
algorithm = "piso"
convection = "linear_upwind"
dt = 0.05
end_time = 160.0
[statistics]
enabled = true
start_time = 100.0
fields = ["ux", "p"]
"#;
    let case = Case::parse_toml(text).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    let probe = {
        let c = sim.geom.mesh.cell_centres();
        (0..c.len())
            .min_by(|&a, &b| {
                let d = |i: usize| (c[i][0] - 3.0).powi(2) + (c[i][1]).powi(2);
                d(a).total_cmp(&d(b))
            })
            .unwrap()
    };
    let (mut times, mut vprobe) = (Vec::new(), Vec::new());
    let mut sum_ux: Vec<f64> = Vec::new();
    let mut sum_p: Vec<f64> = Vec::new();
    let (mut t_prev, mut wtot): (f64, f64) = (0.0, 0.0);
    while !sim.is_done() {
        sim.step();
        let t = sim.time();
        let w = t - f64::max(t_prev, 100.0);
        t_prev = t;
        times.push(t);
        vprobe.push(sim.velocity().values[probe][1]);
        if w > 0.0 {
            let (ux, p) = (sim.field("ux").unwrap(), sim.field("p").unwrap());
            if sum_ux.is_empty() {
                sum_ux = vec![0.0; ux.len()];
                sum_p = vec![0.0; p.len()];
            }
            for c in 0..ux.len() {
                sum_ux[c] += w * ux[c];
                sum_p[c] += w * p[c];
            }
            wtot += w;
        }
    }
    let (mu, mp) = (sim.field("ux_mean").unwrap(), sim.field("p_mean").unwrap());
    let scale_u = mu.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let scale_p = mp.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let du = (0..mu.len())
        .map(|c| (mu[c] - sum_ux[c] / wtot).abs())
        .fold(0.0, f64::max)
        / scale_u;
    let dp = (0..mp.len())
        .map(|c| (mp[c] - sum_p[c] / wtot).abs())
        .fold(0.0, f64::max)
        / scale_p;
    let mean_v = vprobe.iter().sum::<f64>() / vprobe.len() as f64;
    let crossings = vprobe
        .windows(2)
        .filter(|w| (w[0] - mean_v) * (w[1] - mean_v) < 0.0)
        .count();
    let std_probe = sim.field("p_std").unwrap()[probe];
    eprintln!(
        "shedding: {} steps, t = {:.1}, averaged over {:.1}, mean-field difference ux {du:e}, p {dp:e}, probe sign changes {crossings}, p_std at probe {std_probe:.4}",
        times.len(),
        sim.time(),
        sim.statistics_time()
    );
    assert!(du < 1e-12 && dp < 1e-12, "{du:e} {dp:e}");
    assert!(
        crossings >= 4,
        "no shedding in the probe signal: {crossings} sign changes"
    );
}

#[test]
fn transient_runs_honour_the_stop_file_and_the_elapsed_limit() {
    let d = tmp("limits");
    let drive = |opts: &RunOptions| {
        let sim = Simulation::new(&Case::parse_toml(TRANSIENT).unwrap()).unwrap();
        let m = Mutex::new(sim);
        let out = runctl::drive(&m, opts, &|| false, &mut |_| {});
        (m.into_inner().unwrap(), out)
    };
    // a stop file that exists from the start: interrupted at once, exit 7
    let stop = d.join("STOP");
    std::fs::write(&stop, "").unwrap();
    let (_, out) = drive(&RunOptions {
        stop_file: Some(stop.clone()),
        ..Default::default()
    });
    assert_eq!(
        (out.state, out.exit_code, out.iterations),
        ("interrupted", exit::INTERRUPTED, 0),
        "{out:?}"
    );
    // a zero-second wall-clock limit: time_limit, exit 8
    let (_, out) = drive(&RunOptions {
        max_elapsed: Some(0.0),
        ..Default::default()
    });
    assert_eq!((out.state, out.exit_code), ("time_limit", exit::TIME_LIMIT), "{out:?}");
    // the statistics of a run cut after 5 steps cover exactly the time reached
    let (sim, out) = drive(&RunOptions {
        max_steps: Some(5),
        ..Default::default()
    });
    assert_eq!(out.state, "step_limit");
    assert!((sim.statistics_time() - (sim.time() - 0.15).max(0.0)).abs() < 1e-12);
}
