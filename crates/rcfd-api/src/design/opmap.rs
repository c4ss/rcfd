//! Operating-map automation (ADR 0067, F-254 candidate): a chain of operating points, each restarting from the previous
//! converged solution, with a metrics table, per-point status and failure handling.
//!
//! The driver is generic over [`PointEvaluator`], so an analytic surrogate (tests) and the case evaluator
//! ([`super::case_eval::CaseMapEvaluator`]) share it. The tables hold no wall-clock times: two runs of the same map give
//! byte-identical `map.csv` and `map.json`.

use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Largest number of points of one map.
pub const MAX_MAP_POINTS: usize = 100_000;

/// Outcome class of one point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointStatus {
    /// Converged (steady) or finished (transient).
    Converged,
    /// Stopped at a step/time limit without meeting the convergence targets.
    NotConverged,
    /// Non-finite state or divergence guard.
    Diverged,
    /// Could not be run (invalid override, mesh, internal error).
    Failed,
}

impl PointStatus {
    /// Stable name.
    pub fn name(self) -> &'static str {
        match self {
            PointStatus::Converged => "converged",
            PointStatus::NotConverged => "not_converged",
            PointStatus::Diverged => "diverged",
            PointStatus::Failed => "failed",
        }
    }
}

/// One operating point: settings overrides applied to the base case.
#[derive(Debug, Clone, PartialEq)]
pub struct MapPoint {
    /// Position in the map.
    pub index: usize,
    /// Label (`path=value path=value`).
    pub label: String,
    /// Overrides `(settings path, value)`.
    pub set: Vec<(String, Value)>,
}

/// Result of running one point.
#[derive(Debug, Clone)]
pub struct PointResult<S> {
    /// Class.
    pub status: PointStatus,
    /// Run-control exit code.
    pub exit_code: u8,
    /// Run-control state name.
    pub state_name: String,
    /// Iterations or steps spent.
    pub iterations: usize,
    /// Metrics `(name, value)` of a usable point.
    pub metrics: Vec<(String, f64)>,
    /// Failure reason or notes.
    pub message: String,
    /// Whether a supplied warm start was applied.
    pub warm_applied: bool,
    /// Carried state for the next point (only for converged points).
    pub state: Option<S>,
}

impl<S> PointResult<S> {
    /// A point that could not be run.
    pub fn failed(message: String) -> PointResult<S> {
        PointResult {
            status: PointStatus::Failed,
            exit_code: 1,
            state_name: "failed".into(),
            iterations: 0,
            metrics: Vec::new(),
            message,
            warm_applied: false,
            state: None,
        }
    }
}

/// Runs one point, optionally from the state of the previous one.
pub trait PointEvaluator {
    /// State carried from point to point.
    type State;
    /// Run `point`; `warm` is the state of the previous converged point (`None` = cold start). If the warm state cannot
    /// be applied the evaluator runs cold and reports `warm_applied = false`.
    fn run(&self, point: &MapPoint, warm: Option<&Self::State>) -> PointResult<Self::State>;
}

/// What to do when a point fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnFailure {
    /// Go on; the next point starts cold from the base case.
    Continue,
    /// End the map after the failed point.
    Stop,
}

/// Options of a map run.
#[derive(Debug, Clone)]
pub struct MapOptions {
    /// Failure handling.
    pub on_failure: OnFailure,
    /// Repeat a failed warm point once from the base case.
    pub retry_cold: bool,
    /// Never restart from the previous solution.
    pub cold: bool,
    /// Directory receiving `map.csv` and `map.json` after every point.
    pub out_dir: Option<PathBuf>,
}

impl Default for MapOptions {
    fn default() -> Self {
        MapOptions {
            on_failure: OnFailure::Continue,
            retry_cold: true,
            cold: false,
            out_dir: None,
        }
    }
}

/// One row of the map table.
#[derive(Debug, Clone, PartialEq)]
pub struct MapRow {
    /// Point index.
    pub index: usize,
    /// Label.
    pub label: String,
    /// Overrides.
    pub set: Vec<(String, Value)>,
    /// Status of the (last) attempt.
    pub status: PointStatus,
    /// `warm`, `cold` or `cold_retry`.
    pub restart: &'static str,
    /// Exit code of the last attempt.
    pub exit_code: u8,
    /// Run-control state of the last attempt.
    pub state: String,
    /// Iterations over all attempts.
    pub iterations: usize,
    /// Metrics by name.
    pub metrics: BTreeMap<String, f64>,
    /// Notes (both attempts of a retried point).
    pub message: String,
}

/// Result of a map run.
#[derive(Debug, Clone, PartialEq)]
pub struct MapOutcome {
    /// Rows of the points run.
    pub rows: Vec<MapRow>,
    /// `complete` or `stopped` (a failed point ended the map under `on_failure = stop`).
    pub state: &'static str,
    /// Points planned.
    pub planned: usize,
}

impl MapOutcome {
    /// Number of converged rows.
    pub fn converged(&self) -> usize {
        self.rows.iter().filter(|r| r.status == PointStatus::Converged).count()
    }
}

/// One axis of a grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Axis {
    /// Settings path.
    pub path: String,
    /// Values in order.
    pub values: Vec<Value>,
}

fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn label_of(set: &[(String, Value)]) -> String {
    set.iter()
        .map(|(p, v)| format!("{p}={}", value_text(v)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Points of a grid: row-major, the first axis slowest. With `serpentine` the last axis runs backwards on every second row
/// (boustrophedon: successive points are neighbours, which helps a warm start).
pub fn grid_points(axes: &[Axis], serpentine: bool) -> Result<Vec<MapPoint>, String> {
    if axes.is_empty() {
        return Err("axes: a map needs at least one axis".into());
    }
    let mut total = 1usize;
    for a in axes {
        if a.path.is_empty() {
            return Err("axes: an axis has an empty path".into());
        }
        if a.values.is_empty() {
            return Err(format!("axis '{}': no values", a.path));
        }
        total = total.saturating_mul(a.values.len());
    }
    if total > MAX_MAP_POINTS {
        return Err(format!("the map has {total} points, the limit is {MAX_MAP_POINTS}"));
    }
    let n = axes.len();
    let last_len = axes[n - 1].values.len();
    let mut points = Vec::with_capacity(total);
    for i in 0..total {
        let mut rest = i;
        let mut idx = vec![0usize; n];
        for k in (0..n).rev() {
            idx[k] = rest % axes[k].values.len();
            rest /= axes[k].values.len();
        }
        if serpentine && n >= 2 && (i / last_len) % 2 == 1 {
            idx[n - 1] = last_len - 1 - idx[n - 1];
        }
        let set: Vec<(String, Value)> = axes
            .iter()
            .zip(&idx)
            .map(|(a, &k)| (a.path.clone(), a.values[k].clone()))
            .collect();
        points.push(MapPoint {
            index: i,
            label: label_of(&set),
            set,
        });
    }
    Ok(points)
}

/// Points from an explicit list of override sets.
pub fn list_points(sets: Vec<Vec<(String, Value)>>) -> Result<Vec<MapPoint>, String> {
    if sets.is_empty() {
        return Err("points: the map has no points".into());
    }
    if sets.len() > MAX_MAP_POINTS {
        return Err(format!(
            "the map has {} points, the limit is {MAX_MAP_POINTS}",
            sets.len()
        ));
    }
    Ok(sets
        .into_iter()
        .enumerate()
        .map(|(index, set)| MapPoint {
            index,
            label: label_of(&set),
            set,
        })
        .collect())
}

/// Parse an axis argument `path=v1,v2,...` (numbers or strings) or `path=start:stop:count` (linspace, both ends exact).
pub fn parse_axis(text: &str) -> Result<Axis, String> {
    let (path, vals) = text
        .split_once('=')
        .ok_or_else(|| format!("axis '{text}': expected path=v1,v2,... or path=start:stop:count"))?;
    let path = path.trim();
    if path.is_empty() || vals.trim().is_empty() {
        return Err(format!(
            "axis '{text}': expected path=v1,v2,... or path=start:stop:count"
        ));
    }
    let parts: Vec<&str> = vals.split(':').collect();
    if parts.len() == 3 && !vals.contains(',') && !vals.contains('[') {
        if let (Ok(a), Ok(b), Ok(n)) = (
            parts[0].trim().parse::<f64>(),
            parts[1].trim().parse::<f64>(),
            parts[2].trim().parse::<usize>(),
        ) {
            if n == 0 || n > MAX_MAP_POINTS || !a.is_finite() || !b.is_finite() {
                return Err(format!("axis '{path}': bad range {vals}"));
            }
            let values = (0..n)
                .map(|i| {
                    let x = if n == 1 {
                        a
                    } else if i + 1 == n {
                        b
                    } else {
                        a + (b - a) * i as f64 / (n - 1) as f64
                    };
                    json!(x)
                })
                .collect();
            return Ok(Axis {
                path: path.to_string(),
                values,
            });
        }
    }
    // split at commas outside brackets, so that vector values like [0.5,0,0] stay whole
    let mut parts_v: Vec<&str> = Vec::new();
    let (mut depth, mut from) = (0i32, 0usize);
    for (i, ch) in vals.char_indices() {
        match ch {
            '[' | '{' => depth += 1,
            ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts_v.push(&vals[from..i]);
                from = i + 1;
            }
            _ => {}
        }
    }
    parts_v.push(&vals[from..]);
    let values = parts_v
        .into_iter()
        .map(|s| {
            let s = s.trim();
            serde_json::from_str::<Value>(s).unwrap_or_else(|_| Value::String(s.to_string()))
        })
        .collect();
    Ok(Axis {
        path: path.to_string(),
        values,
    })
}

/// Run the map. `on_row` is called with all rows after every point (to write the tables incrementally).
pub fn run_map<E: PointEvaluator>(
    points: &[MapPoint],
    eval: &E,
    opts: &MapOptions,
    on_row: &mut dyn FnMut(&[MapRow]),
) -> MapOutcome {
    let mut rows: Vec<MapRow> = Vec::with_capacity(points.len());
    let mut prev: Option<E::State> = None;
    let mut state = "complete";
    for p in points {
        let warm = if opts.cold { None } else { prev.as_ref() };
        let had_warm = warm.is_some();
        let mut notes: Vec<String> = Vec::new();
        let mut r = eval.run(p, warm);
        let mut iterations = r.iterations;
        let mut restart = if had_warm && r.warm_applied { "warm" } else { "cold" };
        if had_warm && r.warm_applied && r.status != PointStatus::Converged && opts.retry_cold {
            notes.push(format!("warm attempt {}: {}", r.status.name(), r.message));
            r = eval.run(p, None);
            iterations += r.iterations;
            restart = "cold_retry";
        }
        if !r.message.is_empty() {
            notes.push(std::mem::take(&mut r.message));
        }
        let converged = r.status == PointStatus::Converged;
        let row = MapRow {
            index: p.index,
            label: p.label.clone(),
            set: p.set.clone(),
            status: r.status,
            restart,
            exit_code: r.exit_code,
            state: r.state_name.clone(),
            iterations,
            metrics: r.metrics.iter().cloned().collect(),
            message: notes.join("; "),
        };
        prev = if converged { r.state } else { None };
        rows.push(row);
        on_row(&rows);
        if !converged && opts.on_failure == OnFailure::Stop {
            state = "stopped";
            break;
        }
    }
    MapOutcome {
        rows,
        state,
        planned: points.len(),
    }
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// The CSV table: point, label, one column per overridden path, status, restart, exit_code, state, iterations, one column per
/// metric (sorted), message. Numbers use the shortest text that parses back to the same bits.
pub fn render_csv(rows: &[MapRow]) -> String {
    let mut paths: Vec<String> = Vec::new();
    let mut metrics: Vec<String> = Vec::new();
    for r in rows {
        for (p, _) in &r.set {
            if !paths.contains(p) {
                paths.push(p.clone());
            }
        }
        for k in r.metrics.keys() {
            if !metrics.contains(k) {
                metrics.push(k.clone());
            }
        }
    }
    metrics.sort();
    let mut head: Vec<String> = vec!["point".into(), "label".into()];
    head.extend(paths.iter().cloned());
    head.extend(["status", "restart", "exit_code", "state", "iterations"].map(String::from));
    head.extend(metrics.iter().cloned());
    head.push("message".into());
    let mut out = head.iter().map(|h| csv_cell(h)).collect::<Vec<_>>().join(",");
    out.push('\n');
    for r in rows {
        let mut cells: Vec<String> = vec![r.index.to_string(), csv_cell(&r.label)];
        for p in &paths {
            cells.push(
                r.set
                    .iter()
                    .find(|(q, _)| q == p)
                    .map_or(String::new(), |(_, v)| csv_cell(&value_text(v))),
            );
        }
        cells.push(r.status.name().into());
        cells.push(r.restart.into());
        cells.push(r.exit_code.to_string());
        cells.push(csv_cell(&r.state));
        cells.push(r.iterations.to_string());
        for m in &metrics {
            cells.push(r.metrics.get(m).map_or(String::new(), |v| format!("{v}")));
        }
        cells.push(csv_cell(&r.message));
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

/// Row as a JSON object.
pub fn row_json(r: &MapRow) -> Value {
    let set: Map<String, Value> = r.set.iter().cloned().collect();
    let metrics: Map<String, Value> = r
        .metrics
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                serde_json::Number::from_f64(*v).map_or(Value::Null, Value::Number),
            )
        })
        .collect();
    json!({
        "point": r.index, "label": r.label, "set": set, "status": r.status.name(), "restart": r.restart,
        "exit_code": r.exit_code, "state": r.state, "iterations": r.iterations, "metrics": metrics, "message": r.message,
    })
}

/// The JSON table.
pub fn render_json(rows: &[MapRow], planned: usize, state: &str) -> String {
    let doc = json!({
        "format": "rcfd-operating-map", "version": 1, "planned": planned, "state": state,
        "points": rows.iter().map(row_json).collect::<Vec<_>>(),
    });
    let mut text = serde_json::to_string_pretty(&doc).unwrap_or_default();
    text.push('\n');
    text
}

fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Write `map.csv` and `map.json` into `dir` (created when missing).
pub fn write_tables(dir: &Path, rows: &[MapRow], planned: usize, state: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    write_atomic(&dir.join("map.csv"), &render_csv(rows))?;
    write_atomic(&dir.join("map.json"), &render_json(rows, planned, state))
}

/// Run a map and write the tables after every point when `opts.out_dir` is set.
pub fn run_and_write<E: PointEvaluator>(
    points: &[MapPoint],
    eval: &E,
    opts: &MapOptions,
) -> Result<MapOutcome, String> {
    let planned = points.len();
    let mut err: Option<String> = None;
    let out = run_map(points, eval, opts, &mut |rows| {
        if let Some(d) = &opts.out_dir {
            if let Err(e) = write_tables(d, rows, planned, "running") {
                err.get_or_insert(format!("cannot write the tables to {}: {e}", d.display()));
            }
        }
    });
    if let Some(d) = &opts.out_dir {
        write_tables(d, &out.rows, planned, out.state)
            .map_err(|e| format!("cannot write the tables to {}: {e}", d.display()))?;
    }
    match err {
        Some(e) => Err(e),
        None => Ok(out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    const A: f64 = 2.0;
    const B: f64 = 0.5;
    const C: f64 = 3.0;

    /// Surrogate pump/fan curve; state = the index of the point that produced it.
    struct Surrogate {
        received: RefCell<Vec<Option<usize>>>,
        fail_warm_at: Option<usize>,
        fail_always_at: Option<usize>,
        calls: Cell<usize>,
    }

    fn q_n(p: &MapPoint) -> (f64, f64) {
        let get = |name: &str| p.set.iter().find(|(k, _)| k == name).unwrap().1.as_f64().unwrap();
        (get("q"), get("n"))
    }

    impl PointEvaluator for Surrogate {
        type State = usize;
        fn run(&self, p: &MapPoint, warm: Option<&usize>) -> PointResult<usize> {
            self.calls.set(self.calls.get() + 1);
            self.received.borrow_mut().push(warm.copied());
            let fails = self.fail_always_at == Some(p.index) || (self.fail_warm_at == Some(p.index) && warm.is_some());
            if fails {
                let mut r = PointResult::failed("surrogate failure".into());
                r.warm_applied = warm.is_some();
                r.iterations = 7;
                return r;
            }
            let (q, n) = q_n(p);
            // independent closed form
            let dp = A * n * n - B * n * q - C * q * q;
            let power = 0.1 * n * n * n + 0.25 * n * q * q;
            let eta = q * dp / power;
            PointResult {
                status: PointStatus::Converged,
                exit_code: 0,
                state_name: "converged".into(),
                iterations: 10,
                metrics: vec![("dp".into(), dp), ("power".into(), power), ("eta".into(), eta)],
                message: String::new(),
                warm_applied: warm.is_some(),
                state: Some(p.index),
            }
        }
    }

    fn surrogate(fail_warm_at: Option<usize>, fail_always_at: Option<usize>) -> Surrogate {
        Surrogate {
            received: RefCell::new(Vec::new()),
            fail_warm_at,
            fail_always_at,
            calls: Cell::new(0),
        }
    }

    fn axes() -> Vec<Axis> {
        vec![
            Axis {
                path: "n".into(),
                values: [1.0, 1.5, 2.0].iter().map(|v| json!(v)).collect(),
            },
            Axis {
                path: "q".into(),
                values: [0.1, 0.2, 0.3, 0.4, 0.5].iter().map(|v| json!(v)).collect(),
            },
        ]
    }

    #[test]
    fn m1_surrogate_table_is_exact_ordered_and_chained() {
        let pts = grid_points(&axes(), false).unwrap();
        assert_eq!(pts.len(), 15);
        let s = surrogate(None, None);
        let out = run_map(&pts, &s, &MapOptions::default(), &mut |_| {});
        assert_eq!(out.rows.len(), 15);
        assert_eq!(out.state, "complete");
        let mut i = 0;
        for &n in &[1.0f64, 1.5, 2.0] {
            for &q in &[0.1f64, 0.2, 0.3, 0.4, 0.5] {
                let row = &out.rows[i];
                let want_dp = 2.0 * n * n - 0.5 * n * q - 3.0 * q * q;
                let want_p = 0.1 * n * n * n + 0.25 * n * q * q;
                assert_eq!(row.index, i);
                assert_eq!(row.metrics["dp"].to_bits(), want_dp.to_bits());
                assert_eq!(row.metrics["power"].to_bits(), want_p.to_bits());
                assert_eq!(row.metrics["eta"].to_bits(), (q * want_dp / want_p).to_bits());
                assert_eq!(row.status, PointStatus::Converged);
                assert_eq!(row.restart, if i == 0 { "cold" } else { "warm" });
                i += 1;
            }
        }
        let rec = s.received.borrow();
        assert_eq!(rec[0], None);
        for (k, r) in rec.iter().enumerate().skip(1) {
            assert_eq!(*r, Some(k - 1), "point {k} received the previous state");
        }
        // CSV round trip to the bit
        let csv = render_csv(&out.rows);
        let mut lines = csv.lines();
        let head: Vec<&str> = lines.next().unwrap().split(',').collect();
        let col = |n: &str| head.iter().position(|h| *h == n).unwrap();
        for (row, line) in out.rows.iter().zip(lines) {
            let cells: Vec<&str> = line.split(',').collect();
            for m in ["dp", "power", "eta"] {
                let v: f64 = cells[col(m)].parse().unwrap();
                assert_eq!(v.to_bits(), row.metrics[m].to_bits());
            }
        }
        // JSON round trip
        let j: Value = serde_json::from_str(&render_json(&out.rows, 15, out.state)).unwrap();
        for (row, pj) in out.rows.iter().zip(j["points"].as_array().unwrap()) {
            assert_eq!(
                pj["metrics"]["eta"].as_f64().unwrap().to_bits(),
                row.metrics["eta"].to_bits()
            );
        }
    }

    #[test]
    fn m2_failure_handling() {
        let pts = grid_points(&axes(), false).unwrap();
        // flow index 3 of speed 1.5 is point 5 + 3 = 8; it fails always (cold too)
        let s = surrogate(None, Some(8));
        let out = run_map(&pts, &s, &MapOptions::default(), &mut |_| {});
        assert_eq!(out.rows.len(), 15);
        let failed: Vec<usize> = out
            .rows
            .iter()
            .filter(|r| r.status == PointStatus::Failed)
            .map(|r| r.index)
            .collect();
        assert_eq!(failed, vec![8]);
        assert_eq!(out.rows[9].restart, "cold");
        assert_eq!(out.rows[9].status, PointStatus::Converged);
        assert_eq!(out.rows[10].restart, "warm");
        // retried point: fails warm, passes cold
        let s = surrogate(Some(8), None);
        let out = run_map(&pts, &s, &MapOptions::default(), &mut |_| {});
        assert_eq!(out.rows[8].restart, "cold_retry");
        assert_eq!(out.rows[8].status, PointStatus::Converged);
        assert!(out.rows[8].message.contains("warm attempt failed"));
        assert_eq!(out.rows[8].iterations, 7 + 10);
        assert!(out.rows.iter().all(|r| r.status == PointStatus::Converged));
        // without retry the warm failure stays a failure
        let s = surrogate(Some(8), None);
        let out = run_map(
            &pts,
            &s,
            &MapOptions {
                retry_cold: false,
                ..Default::default()
            },
            &mut |_| {},
        );
        assert_eq!(out.rows[8].status, PointStatus::Failed);
        assert_eq!(out.rows[9].restart, "cold");
        // stop ends after the failing point
        let s = surrogate(None, Some(8));
        let out = run_map(
            &pts,
            &s,
            &MapOptions {
                on_failure: OnFailure::Stop,
                ..Default::default()
            },
            &mut |_| {},
        );
        assert_eq!(out.rows.len(), 9);
        assert_eq!(out.state, "stopped");
        assert_eq!(out.rows[8].status, PointStatus::Failed);
        // cold: nothing is ever warm
        let s = surrogate(None, None);
        let out = run_map(
            &pts,
            &s,
            &MapOptions {
                cold: true,
                ..Default::default()
            },
            &mut |_| {},
        );
        assert!(out.rows.iter().all(|r| r.restart == "cold"));
        assert!(s.received.borrow().iter().all(Option::is_none));
    }

    #[test]
    fn m4_two_runs_are_byte_identical_on_disk() {
        let dir = std::env::temp_dir().join(format!("rcfd_map_m4_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let pts = grid_points(&axes(), true).unwrap();
        let mut texts = Vec::new();
        for k in 0..2 {
            let d = dir.join(format!("run{k}"));
            let s = surrogate(None, Some(8));
            let opts = MapOptions {
                out_dir: Some(d.clone()),
                ..Default::default()
            };
            run_and_write(&pts, &s, &opts).unwrap();
            texts.push((
                std::fs::read(d.join("map.csv")).unwrap(),
                std::fs::read(d.join("map.json")).unwrap(),
            ));
        }
        assert_eq!(texts[0], texts[1]);
        assert!(!texts[0].0.is_empty());
    }

    #[test]
    fn grid_order_serpentine_and_axis_parsing() {
        let pts = grid_points(&axes(), true).unwrap();
        let qs: Vec<f64> = pts.iter().map(|p| q_n(p).0).collect();
        assert_eq!(&qs[..10], &[0.1, 0.2, 0.3, 0.4, 0.5, 0.5, 0.4, 0.3, 0.2, 0.1]);
        let a = parse_axis("boundaries.inlet.velocity_value=0.5,0.6").unwrap();
        assert_eq!(a.values, vec![json!(0.5), json!(0.6)]);
        let a = parse_axis("x=1:2:5").unwrap();
        assert_eq!(
            a.values,
            vec![json!(1.0), json!(1.25), json!(1.5), json!(1.75), json!(2.0)]
        );
        let a = parse_axis("solver.convection=upwind,linear").unwrap();
        assert_eq!(a.values, vec![json!("upwind"), json!("linear")]);
        let a = parse_axis("boundaries.inlet.velocity_value=[0.5,0,0],[0.6,0,0]").unwrap();
        assert_eq!(a.values, vec![json!([0.5, 0, 0]), json!([0.6, 0, 0])]);
        assert!(parse_axis("novalue").unwrap_err().contains("expected"));
        assert!(grid_points(&[], false).is_err());
        assert!(list_points(vec![]).is_err());
    }
}
