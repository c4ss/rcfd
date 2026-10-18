//! ADR 0066 check S3: `rcfd fwh` reproduces the library result (with and without mean flow) to 1e-12 relative, the
//! spectrum outputs parse and the OASPL equals the library value, and malformed files give located errors (exit 3).

use rcfd_post::acoustics::{spectrum_report, Window};
use rcfd_post::fwh::{medium_frame_observer, SourceMode};
use rcfd_post::surface_series::{FaceSpec, Motion, SeriesSpec};
use std::process::Command;

fn spec() -> SeriesSpec {
    let faces = (0..6)
        .map(|k| {
            let a = 1.0 * k as f64;
            FaceSpec {
                area: 2e-3,
                x: [0.3 + 0.02 * a, 0.0, 0.01 * a],
                n: [0.0, 0.6, 0.8],
                p: (0..400)
                    .map(|i| 40.0 * (2.0 * std::f64::consts::PI * 0.011 * i as f64 + 0.4 * a).sin() + 5.0)
                    .collect(),
            }
        })
        .collect();
    SeriesSpec {
        rho0: 1.2,
        c0: 340.0,
        t0: 0.0,
        dt: 1e-4,
        mode: SourceMode::Impermeable,
        flow: [0.0, 0.0, 0.0],
        motion: Motion::Rotation {
            axis: [0.0, 0.0, 1.0],
            origin: [0.0, 0.0, 0.0],
            omega: 200.0,
            angle0: 0.0,
            velocity: [0.0, 0.0, 0.0],
        },
        faces,
    }
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

/// Library signal at the observer over its full valid window (the same grid the CLI uses by default).
fn library(s: &SeriesSpec, obs: [f64; 3]) -> (Vec<f64>, Vec<f64>) {
    let case = s.build().unwrap();
    let surf = case.medium_frame().unwrap();
    let flow = case.flow;
    let uf = (flow[0] * flow[0] + flow[1] * flow[1] + flow[2] * flow[2]).sqrt();
    let f = medium_frame_observer(obs, flow);
    let end = surf.t0 + surf.samples() as f64 * surf.dt;
    let dmax = case
        .surface
        .elements
        .iter()
        .flat_map(|e| e.x.iter())
        .map(|y| ((obs[0] - y[0]).powi(2) + (obs[1] - y[1]).powi(2) + (obs[2] - y[2]).powi(2)).sqrt())
        .fold(0.0, f64::max);
    let (lo, hi) = if uf == 0.0 {
        surf.valid_window(obs).unwrap()
    } else {
        surf.valid_window_moving(&f, surf.t0, end + dmax / (surf.c0 - uf), 8.0 * surf.dt)
            .unwrap()
    };
    let n = ((hi - lo) / surf.dt).floor() as usize;
    let times: Vec<f64> = (0..=n).map(|k| lo + k as f64 * surf.dt).collect();
    let sig = surf.signal_moving(&f, &times).unwrap();
    (times, sig.iter().map(|g| g.total()).collect())
}

fn parse_rows(text: &str) -> Vec<Vec<f64>> {
    text.lines()
        .skip(1)
        .map(|l| l.split(',').map(|v| v.parse().unwrap()).collect())
        .collect()
}

#[test]
fn s3_cli_matches_library_with_and_without_flow() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let s = spec();
    let file = dir.join("s3_series.json");
    std::fs::write(&file, serde_json::to_string(&s.to_json()).unwrap()).unwrap();
    let obs = [6.0, 3.0, 4.0];
    for flow in [None, Some("0,0,25")] {
        let mut args = vec!["fwh", file.to_str().unwrap(), "--observer", "6,3,4"];
        let mut sl = s.clone();
        if let Some(f) = flow {
            args.push("--flow");
            args.push(f);
            sl.flow = [0.0, 0.0, 25.0];
        }
        let (times, total) = library(&sl, obs);
        let out = run(&args);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let rows = parse_rows(&String::from_utf8(out.stdout).unwrap());
        assert_eq!(rows.len(), times.len());
        let scale = total.iter().map(|v| v.abs()).fold(0.0, f64::max);
        assert!(scale > 0.0);
        let mut worst = 0.0_f64;
        for (r, g) in rows.iter().zip(&total) {
            worst = worst.max((r[4] - g).abs() / scale);
        }
        println!("S3 CLI vs library (flow {flow:?}): worst relative difference {worst:.3e}");
        assert!(worst <= 1e-12, "S3 CLI: {worst:e}");
    }
}

#[test]
fn s3_spectrum_outputs_and_located_errors() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let s = spec();
    let file = dir.join("s3b_series.json");
    std::fs::write(&file, serde_json::to_string(&s.to_json()).unwrap()).unwrap();
    let obs = [6.0, 3.0, 4.0];
    let (times, total) = library(&s, obs);
    let dt = (times[times.len() - 1] - times[0]) / (times.len() - 1) as f64;
    let rep = spectrum_report(&total, dt, Window::Hann, 20.0, 20000.0).unwrap();
    let js = dir.join("s3b_spectrum.json");
    let bands = dir.join("s3b_bands.csv");
    let out = run(&[
        "fwh",
        file.to_str().unwrap(),
        "--observer",
        "6,3,4",
        "--spectrum",
        js.to_str().unwrap(),
        "--out",
        dir.join("s3b_p.csv").to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let doc: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&js).unwrap()).unwrap();
    assert_eq!(doc["format"], "rcfd-spectra");
    let r = &doc["observers"][0]["report"];
    let cli = r["oaspl_db"].as_f64().unwrap();
    let lib = rep.oaspl.unwrap();
    println!("S3 OASPL CLI {cli} library {lib}");
    assert!((cli - lib).abs() <= 1e-12 * lib.abs().max(1.0));
    assert!(!r["third_octave"].as_array().unwrap().is_empty());
    let out = run(&[
        "fwh",
        file.to_str().unwrap(),
        "--observer",
        "6,3,4",
        "--spectrum",
        bands.to_str().unwrap(),
        "--out",
        dir.join("s3b_p2.csv").to_str().unwrap(),
    ]);
    assert!(out.status.success());
    let text = std::fs::read_to_string(&bands).unwrap();
    assert!(text.starts_with("observer,nominal_hz"));
    assert_eq!(text.lines().count() - 1, rep.bands.len());

    // malformed series: located error, exit code 3
    let mut doc = s.to_json();
    doc["faces"][2]["p"].as_array_mut().unwrap().pop();
    let bad = dir.join("s3_bad.json");
    std::fs::write(&bad, serde_json::to_string(&doc).unwrap()).unwrap();
    let out = run(&["fwh", bad.to_str().unwrap(), "--observer", "6,3,4"]);
    assert_eq!(out.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&out.stderr).contains("faces[2].p"));
    // usage errors
    assert_eq!(run(&["fwh", file.to_str().unwrap()]).status.code(), Some(2));
    assert_eq!(run(&["fwh"]).status.code(), Some(2));
}
