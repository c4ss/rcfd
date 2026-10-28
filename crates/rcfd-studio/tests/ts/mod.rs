//! Helpers of ADR 0153's checks (job 48S): the example cases as the checks use them, a run's monitor series, the
//! Strouhal number of its lift, and `rcfd run` of a case with the Studio's time settings.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The repository's root.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The wind tunnel's `cases/` folder.
pub fn wind_tunnel_cases() -> PathBuf {
    repo_root().join("example-projects/wind-tunnel/cases")
}

/// The quick start's `cases/` folder.
pub fn quick_start_cases() -> PathBuf {
    repo_root().join("example-projects/quick-start/cases")
}

/// An example file's text with LF line ends (the example projects keep their bytes, `* -text`; a CRLF copy reads the
/// same here).
fn read(path: PathBuf) -> String {
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .replace("\r\n", "\n")
}

/// The wind tunnel's `cylinder-re100.toml` with `mesh.nx` and `mesh.ny` set (64 x 48: the file as shipped).
pub fn cylinder(nx: usize, ny: usize) -> String {
    let t = read(wind_tunnel_cases().join("cylinder-re100.toml"));
    assert!(
        t.contains("\nnx = 64\n") && t.contains("\nny = 48\n"),
        "the example's mesh counts moved"
    );
    t.replace("\nnx = 64\n", &format!("\nnx = {nx}\n"))
        .replace("\nny = 48\n", &format!("\nny = {ny}\n"))
}

/// A steady SIMPLEC example made transient: `algorithm = "piso"` and `end_time` in place of `algorithm = "simplec"`
/// (the rest of the file as shipped; no `solver.dt`, so the schema default).
fn piso(text: &str, end_time: f64) -> String {
    assert!(text.contains("algorithm = \"simplec\""), "not a simplec example");
    text.replace(
        "algorithm = \"simplec\"",
        &format!("algorithm = \"piso\"\nend_time = {end_time:?}"),
    )
}

/// The quick start's `cavity-re100.toml` as a PISO run to `end_time`, `nx` x `nx` cells (32: as shipped).
pub fn cavity_piso(nx: usize, end_time: f64) -> String {
    let t = read(quick_start_cases().join("cavity-re100.toml"));
    assert!(t.contains("nx = 32\nny = 32"), "the example's mesh counts moved");
    piso(
        &t.replace("nx = 32\nny = 32", &format!("nx = {nx}\nny = {nx}")),
        end_time,
    )
}

/// The wind tunnel's `step-re400.toml` as a PISO run to `end_time`.
pub fn step_piso(end_time: f64) -> String {
    piso(&read(wind_tunnel_cases().join("step-re400.toml")), end_time)
}

/// The quick start's cavity as PISO to `end_time` with its lid ramped from rest: `velocity_expression_x = "min(1 [m/s],
/// 1 [m/s] * t / 1 [s])"` in place of its fixed `velocity_value` (no speed at t = 0, no gravity: no speed scale).
pub fn cavity_ramp_piso(end_time: f64) -> String {
    let t = cavity_piso(32, end_time);
    assert!(t.contains("velocity_value = [1.0, 0.0, 0.0]"), "the lid moved");
    t.replace(
        "velocity_value = [1.0, 0.0, 0.0]",
        "velocity_expression_x = \"min(1 [m/s], 1 [m/s] * t / 1 [s])\"",
    )
}

/// The quick start's `heated-cavity-ra1e4.toml` as a PISO run to `end_time`: a flow at rest, driven by buoyancy only
/// (no inflow, moving wall or initial velocity; gravity 7100 m/s^2 in a 1 m box).
pub fn heated_piso(end_time: f64) -> String {
    piso(&read(quick_start_cases().join("heated-cavity-ra1e4.toml")), end_time)
}

/// The columns of a `monitors.csv` (`time`, `dt`, `courant`, `cd`, `cl`, residuals, reports) by name. A missing file (a
/// run that wrote no results) gives the columns `time`, `dt`, `courant`, `cd`, `cl` empty, so that the limits fail with
/// their own messages.
pub fn monitors(csv: &Path) -> BTreeMap<String, Vec<f64>> {
    let Ok(text) = std::fs::read_to_string(csv) else {
        eprintln!("{}: not found", csv.display());
        return ["time", "dt", "courant", "cd", "cl"]
            .iter()
            .map(|n| (n.to_string(), Vec::new()))
            .collect();
    };
    let mut lines = text.lines();
    let names: Vec<String> = lines.next().unwrap_or_default().split(',').map(String::from).collect();
    let mut cols: BTreeMap<String, Vec<f64>> = names.iter().map(|n| (n.clone(), Vec::new())).collect();
    for l in lines.filter(|l| !l.trim().is_empty()) {
        for (n, v) in names.iter().zip(l.split(',')) {
            cols.get_mut(n).unwrap().push(v.trim().parse().unwrap_or(f64::NAN));
        }
    }
    cols
}

/// The shedding of a lift series in the window `[from, to]` (ADR 0142 amendment 1's rule): the upward crossings of `cl`
/// through its mean over the window (times by linear interpolation), the Strouhal number `(k - 1) / (t_k - t_1)` over the
/// whole periods between the first and the last crossing (D = U = 1), and half the range of `cl` in the window.
#[derive(Debug, Clone, PartialEq)]
pub struct Shedding {
    /// Upward crossings.
    pub crossings: usize,
    /// Strouhal number (`None` below two crossings).
    pub strouhal: Option<f64>,
    /// Half the range of `cl`.
    pub amplitude: f64,
    /// The mean of `cl` over the window.
    pub mean: f64,
}

/// [`Shedding`] of `cl(t)` over `[from, to]`.
pub fn shedding(t: &[f64], cl: &[f64], from: f64, to: f64) -> Shedding {
    let w: Vec<(f64, f64)> = t
        .iter()
        .zip(cl)
        .filter(|(x, _)| **x >= from && **x <= to)
        .map(|(x, y)| (*x, *y))
        .collect();
    let mean = w.iter().map(|p| p.1).sum::<f64>() / w.len().max(1) as f64;
    let mut times = Vec::new();
    for p in w.windows(2) {
        let ((t0, a), (t1, b)) = (p[0], p[1]);
        if a < mean && b >= mean {
            times.push(t0 + (mean - a) / (b - a) * (t1 - t0));
        }
    }
    let (lo, hi) = w
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), p| (l.min(p.1), h.max(p.1)));
    Shedding {
        crossings: times.len(),
        strouhal: (times.len() >= 2).then(|| (times.len() - 1) as f64 / (times[times.len() - 1] - times[0])),
        amplitude: 0.5 * (hi - lo),
        mean,
    }
}

/// `rcfd run` of `text` (written to `<out>.toml`, its relative files against `dir`) with `--set` entries, one thread,
/// output to `out`: the exit code and the wall time.
pub fn run_cli(rcfd: &Path, text: &str, dir: &Path, out: &Path, set: &[String]) -> (i32, f64) {
    let _ = std::fs::remove_dir_all(out);
    std::fs::create_dir_all(out).unwrap();
    let case = out.join("case.toml");
    std::fs::write(&case, text).unwrap();
    let mut cmd = std::process::Command::new(rcfd);
    cmd.arg("run")
        .arg(&case)
        .arg("--case-dir")
        .arg(dir)
        .arg("--out")
        .arg(out)
        .args(["--quiet", "--threads", "1"]);
    for s in set {
        cmd.arg("--set").arg(s);
    }
    let t0 = Instant::now();
    let o = cmd.output().unwrap();
    std::fs::write(out.join("stderr.txt"), &o.stderr).unwrap();
    (o.status.code().unwrap_or(-1), t0.elapsed().as_secs_f64())
}

/// The largest of `v[from..]` (NaN when empty).
pub fn max_from(v: &[f64], from: usize) -> f64 {
    v.iter().skip(from).copied().fold(f64::NAN, f64::max)
}
