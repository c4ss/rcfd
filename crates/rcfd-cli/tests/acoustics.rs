//! ADR 0013 check A7 (CLI part): `rcfd acoustics` reproduces the library result from a surface file to 1e-12 relative
//! and reports located errors for malformed files. The surface here is a coarse monopole-like sphere, so the check
//! compares CLI against library on the same data (the accuracy of the formulation itself is checked in rcfd-post).

use rcfd_post::fwh::{Element, Surface};
use std::process::Command;

fn surface() -> Surface {
    let (rho0, c0) = (1.2, 340.0);
    let w = 2.0 * std::f64::consts::PI * 85.0;
    let ns = 200;
    let dt = 1.0 / (85.0 * 32.0);
    let mut elements = Vec::new();
    let (nth, nph) = (6, 12);
    for i in 0..nth {
        let th = std::f64::consts::PI * (i as f64 + 0.5) / nth as f64;
        for j in 0..nph {
            let ph = 2.0 * std::f64::consts::PI * (j as f64 + 0.5) / nph as f64;
            let n = [th.sin() * ph.cos(), th.sin() * ph.sin(), th.cos()];
            let area = 4.0 * std::f64::consts::PI * th.sin() * (std::f64::consts::PI / nth as f64) / nph as f64
                * (nth as f64 / std::f64::consts::PI)
                * (std::f64::consts::PI / nth as f64);
            let mut e = Element {
                area,
                x: vec![],
                n: vec![],
                v: vec![],
                p: vec![],
                rho: vec![],
                u: vec![],
            };
            for k in 0..ns {
                let t = k as f64 * dt;
                let p = 0.02 * (w * t - 0.7).cos();
                e.x.push(n);
                e.n.push(n);
                e.v.push([0.0; 3]);
                e.p.push(p);
                e.rho.push(p / (c0 * c0));
                e.u.push([p / (rho0 * c0) * n[0], p / (rho0 * c0) * n[1], p / (rho0 * c0) * n[2]]);
            }
            elements.push(e);
        }
    }
    Surface {
        rho0,
        c0,
        t0: 0.0,
        dt,
        elements,
    }
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

#[test]
fn a7_cli_matches_library_and_reports_located_errors() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let s = surface();
    let file = dir.join("a7_surface.json");
    std::fs::write(&file, serde_json::to_string(&s.to_json()).unwrap()).unwrap();
    let obs = [10.0, 0.0, 0.0];
    let (lo, hi) = s.valid_window(obs).unwrap();
    let times: Vec<f64> = (0..20).map(|k| lo + (hi - lo) * k as f64 / 19.0).collect();
    let lib = s.signal(obs, &times).unwrap();
    let spec = format!("{:e}:{:e}:20", times[0], times[19]);
    let out = run(&[
        "acoustics",
        file.to_str().unwrap(),
        "--observer",
        "10,0,0",
        "--times",
        &spec,
    ]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    let rows: Vec<Vec<f64>> = text
        .lines()
        .skip(1)
        .map(|l| l.split(',').map(|v| v.parse().unwrap()).collect())
        .collect();
    assert_eq!(rows.len(), 20);
    let scale = lib.iter().map(|g| g.total().abs()).fold(0.0, f64::max);
    let mut worst = 0.0_f64;
    for (r, g) in rows.iter().zip(&lib) {
        worst = worst.max((r[4] - g.total()).abs() / scale);
    }
    println!("A7 CLI vs library: worst relative difference {worst:.3e}");
    assert!(worst <= 1e-12, "A7 CLI: {worst:e}");

    // malformed file: located error, exit code 3
    let mut doc = s.to_json();
    doc["elements"][2]["p"].as_array_mut().unwrap().pop();
    let bad = dir.join("a7_bad.json");
    std::fs::write(&bad, serde_json::to_string(&doc).unwrap()).unwrap();
    let out = run(&["acoustics", bad.to_str().unwrap(), "--observer", "10,0,0"]);
    assert_eq!(out.status.code(), Some(3));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("elements[2].p"), "{err}");
}
