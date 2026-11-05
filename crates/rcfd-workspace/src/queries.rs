//! The read queries (moved from `rcfd-mcp/src/project.rs`, ADR 0185 E2; the text of each is unchanged): the project, its
//! runs, their series, metrics and stored fields, the changes since a token, and a project file in pages.

use crate::series::{decimate_minmax, residual_series, series_json};
use crate::{
    err, project_error as perr, str_arg, text_len, u64_arg, Answer, RpcError, Workspace, FILE_PAGE, INTERNAL,
    NOT_FOUND, PARAMS, TEXT_BUDGET, VALIDATION,
};
use rcfd_api::service::base64;
use rcfd_project::records::{hash_bytes, RunState};
use rcfd_project::{atomic, RunSummary, Watcher};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub(crate) fn state_name(s: RunState) -> String {
    rcfd_runner::client::state_name(s)
}

/// `s` with its metrics taken from the `reports` object of `metrics.json` (final report values, written by the run
/// service, ADR 0135 amendment 3) when it has one; otherwise the numeric top-level entries as `rcfd-project` reads them.
fn with_reports(root: &Path, mut s: RunSummary) -> RunSummary {
    let p = root.join("runs").join(&s.id).join("metrics.json");
    if let Ok(v) = atomic::read_json::<Value>(&p) {
        if let Some(r) = v.get("reports").filter(|r| r.is_object()) {
            s.metrics = rcfd_project::records::metrics_from_json(r);
        }
    }
    s
}

fn summary_json(s: &RunSummary) -> Value {
    json!({"id": s.id, "case": s.case, "created": s.created, "state": state_name(s.state), "step": s.step,
        "time": s.time, "metrics": s.metrics})
}

/// Neumaier-compensated sum.
fn neumaier(xs: impl Iterator<Item = f64>) -> f64 {
    let (mut s, mut c) = (0.0f64, 0.0f64);
    for x in xs {
        let t = s + x;
        c += if s.abs() >= x.abs() { (s - t) + x } else { (x - t) + s };
        s = t;
    }
    s + c
}

/// The last `n` complete lines of a file, read from its last `max_bytes` bytes (missing file: none).
fn tail_lines(path: &Path, n: usize, max_bytes: u64) -> Vec<String> {
    let Ok(mut f) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(max_bytes);
    let mut buf = Vec::new();
    if f.seek(SeekFrom::Start(start)).is_err() || f.read_to_end(&mut buf).is_err() {
        return Vec::new();
    }
    let committed = buf.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    let text = String::from_utf8_lossy(&buf[..committed]);
    let mut lines: Vec<&str> = text.split('\n').collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0); // a partial first line
    }
    let lines: Vec<String> = lines
        .into_iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect();
    lines[lines.len().saturating_sub(n)..].to_vec()
}

/// File name part made of `[A-Za-z0-9_-]` only.
fn file_part(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

impl Workspace {
    pub(crate) fn project_info(&self) -> Result<Value, RpcError> {
        let p = &self.project;
        let m = p.manifest();
        let runs = p.runs().map_err(perr)?;
        let mut by_state: BTreeMap<String, usize> = BTreeMap::new();
        for r in &runs.runs {
            *by_state.entry(state_name(r.state)).or_default() += 1;
        }
        let issues = p.validate().map_err(perr)?;
        let notes = count_files(&p.root().join("notes"));
        let mut v = json!({
            "name": m.name, "root": p.root().display().to_string(), "schema_version": m.schema_version,
            "created": m.created, "solver_version_created": m.solver_version_created, "migrated_from": p.migrated_from(),
            "counts": {"cases": p.cases().map_err(perr)?.len(), "meshes": p.meshes().map_err(perr)?.len(),
                "designs": p.designs().map_err(perr)?.len(), "notes": notes, "runs": runs.runs.len(),
                "skipped_runs": runs.skipped.len()},
            "runs_by_state": by_state,
            "issues": [],
            "issues_total": issues.len(),
        });
        let mut list = Vec::new();
        for i in issues.iter().take(50) {
            list.push(json!({"path": self.rel(&i.path), "message": i.message}));
            v["issues"] = json!(list);
            if text_len(&v) > TEXT_BUDGET {
                list.pop();
                v["issues"] = json!(list);
                break;
            }
        }
        Ok(v)
    }

    pub(crate) fn list_runs(&self, args: &Value) -> Result<Value, RpcError> {
        let all = self.project.runs().map_err(perr)?;
        let norm = |s: &str| {
            s.replace('\\', "/")
                .split('/')
                .filter(|c| !c.is_empty() && *c != ".")
                .collect::<Vec<_>>()
                .join("/")
        };
        let state = args.get("state").and_then(Value::as_str);
        let case = args.get("case").and_then(Value::as_str).map(norm);
        let rows: Vec<&RunSummary> = all
            .runs
            .iter()
            .filter(|r| state.is_none_or(|s| state_name(r.state) == s))
            .filter(|r| case.as_ref().is_none_or(|c| &norm(&r.case) == c))
            .collect();
        let offset = u64_arg(args, "offset").unwrap_or(0) as usize;
        let limit = u64_arg(args, "limit").unwrap_or(50) as usize;
        let skipped: Vec<Value> = all
            .skipped
            .iter()
            .take(20)
            .map(|i| json!({"path": self.rel(&i.path), "message": i.message}))
            .collect();
        let mut v = json!({"runs": [], "total": rows.len(), "offset": offset, "next_offset": null,
            "skipped": skipped, "skipped_total": all.skipped.len()});
        let mut size = text_len(&v);
        let mut out = Vec::new();
        for r in rows.iter().skip(offset).take(limit) {
            let row = summary_json(&with_reports(self.project.root(), (*r).clone()));
            let n = text_len(&row) + 1;
            if size + n > TEXT_BUDGET {
                break;
            }
            size += n;
            out.push(row);
        }
        let end = offset + out.len();
        v["runs"] = json!(out);
        if end < rows.len() {
            v["next_offset"] = json!(end);
        }
        Ok(v)
    }

    pub(crate) fn run_status(&self, args: &Value) -> Result<Value, RpcError> {
        let (id, dir) = self.run_arg(args, "run")?;
        let rec = self.project.read_run(&id).map_err(perr)?;
        let st = self.project.read_status(&id).map_err(perr)?;
        let last = tail_lines(&dir.join("residuals.jsonl"), 1, 64 * 1024)
            .pop()
            .and_then(|l| serde_json::from_str::<Value>(&l).ok());
        let n = u64_arg(args, "events").unwrap_or(10) as usize;
        let mut events: Vec<Value> = tail_lines(&dir.join("events.jsonl"), n, 256 * 1024)
            .iter()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        let mut v = json!({"run": id, "case": rec.case, "created": rec.created, "solver_version": rec.solver_version,
            "settings": rec.settings, "status": serde_json::to_value(&st).unwrap_or(Value::Null),
            "last_residual": last, "events": events});
        while text_len(&v) > TEXT_BUDGET && !events.is_empty() {
            events.remove(0);
            v["events"] = json!(events);
        }
        if text_len(&v) > TEXT_BUDGET {
            v["settings"] = json!("(omitted: over the size budget; read runs/<id>/run.json)");
        }
        Ok(v)
    }

    fn run_list_arg(&self, args: &Value, lo: usize, hi: usize) -> Result<Vec<(String, PathBuf)>, RpcError> {
        let ids = args["runs"].as_array().cloned().unwrap_or_default();
        if ids.len() < lo || ids.len() > hi {
            return Err(err(
                PARAMS,
                format!("runs: give {lo} to {hi} run ids, got {}", ids.len()),
            ));
        }
        ids.iter().map(|i| self.run_arg(&json!({"run": i}), "run")).collect()
    }

    pub(crate) fn read_metrics(&self, args: &Value) -> Result<Value, RpcError> {
        let runs = self.run_list_arg(args, 1, 50)?;
        let mut metrics = Map::new();
        let mut omitted = Vec::new();
        let mut size = 64;
        for (id, dir) in &runs {
            let p = dir.join("metrics.json");
            let m = if p.is_file() {
                atomic::read_json::<Value>(&p).map_err(perr)?
            } else {
                Value::Null
            };
            let n = text_len(&m) + id.len() + 4;
            if size + n > TEXT_BUDGET {
                omitted.push(id.clone());
                continue;
            }
            size += n;
            metrics.insert(id.clone(), m);
        }
        Ok(json!({"metrics": metrics, "omitted": omitted}))
    }

    pub(crate) fn read_residuals(&self, args: &Value) -> Result<Value, RpcError> {
        let (id, dir) = self.run_arg(args, "run")?;
        let bytes = match std::fs::read(dir.join("residuals.jsonl")) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(err(INTERNAL, format!("runs/{id}/residuals.jsonl: {e}"))),
        };
        let max_points = u64_arg(args, "max_points").unwrap_or(200) as usize;
        let since = u64_arg(args, "since_step").unwrap_or(0);
        let (mut series, lines, bad) = residual_series(&bytes, since, None);
        let longest = series.values().map(Vec::len).max().unwrap_or(0);
        let mut unknown = Vec::new();
        if let Some(names) = args.get("names").and_then(Value::as_array) {
            let want: BTreeSet<&str> = names.iter().filter_map(Value::as_str).collect();
            unknown = want
                .iter()
                .filter(|n| !series.contains_key(**n))
                .map(|n| n.to_string())
                .collect();
            series.retain(|k, _| want.contains(k.as_str()));
        }
        for s in series.values_mut() {
            *s = decimate_minmax(s, max_points);
        }
        let mut v = json!({"run": id, "lines": lines, "skipped_lines": bad, "max_points": max_points,
            "decimated": longest > max_points, "unknown_names": unknown, "series": series_json(&series)});
        if text_len(&v) > TEXT_BUDGET {
            let rel = format!("runs/{id}/mcp/residuals-{max_points}-from{since}.json");
            let path = self.project.root().join(&rel);
            std::fs::create_dir_all(path.parent().expect("has a parent"))
                .map_err(|e| err(INTERNAL, format!("{rel}: {e}")))?;
            let text = v.to_string();
            atomic::atomic_write(&path, text.as_bytes()).map_err(perr)?;
            v = json!({"run": id, "lines": lines, "skipped_lines": bad, "max_points": max_points,
                "decimated": longest > max_points, "unknown_names": unknown,
                "series_names": series.keys().collect::<Vec<_>>(), "path": rel, "bytes": text.len(),
                "note": "the series are over the size budget and were written to path; lower max_points or pick names to get them inline"});
        }
        Ok(v)
    }

    /// The stored field a snapshot query asks for: (reader, values, field info, snapshot json).
    fn sample(&self, args: &Value) -> Result<Sampled, RpcError> {
        use rcfd_tsstore::{Mode, Pick, Reader, STORE_FILE};
        let (id, rundir) = self.run_arg(args, "run")?;
        let store = rundir.join("fields");
        if !store.join(STORE_FILE).is_file() {
            return Err(err(
                NOT_FOUND,
                format!("run {id} has no stored fields (runs/{id}/fields/{STORE_FILE}); runs store fields when the case sets output.store_interval or output.store_time_interval"),
            ));
        }
        let ts = |e: &dyn std::fmt::Display| err(INTERNAL, format!("runs/{id}/fields: {e}"));
        let mut r = Reader::open(&store).map_err(|e| ts(&e))?;
        let n_snaps = r.snapshots().len();
        if n_snaps == 0 {
            return Err(err(NOT_FOUND, format!("runs/{id}/fields holds no snapshot yet")));
        }
        let field = str_arg(args, "field").to_string();
        let pick = match args.get("pick").and_then(Value::as_str).unwrap_or("nearest") {
            "floor" => Some(Pick::Floor),
            "linear" => None,
            _ => Some(Pick::Nearest),
        };
        let none = |what: &str| err(NOT_FOUND, format!("runs/{id}/fields: no snapshot for {what}"));
        let (values, lower, upper, weight, interpolated) =
            match (u64_arg(args, "step"), args.get("time").and_then(Value::as_f64)) {
                (Some(_), Some(_)) => return Err(err(PARAMS, "give step or time, not both")),
                (Some(s), None) => {
                    let Some(p) = pick else {
                        return Err(err(PARAMS, "pick linear needs a time, not a step"));
                    };
                    let i = r.by_step(s, p).ok_or_else(|| none(&format!("step {s}")))?;
                    (None, i, i, 0.0, false)
                }
                (None, Some(t)) => {
                    let mode = pick.map_or(Mode::Linear, Mode::Pick);
                    let i = match mode {
                        Mode::Pick(p) => r.by_time(t, p).ok_or_else(|| none(&format!("time {t}")))?,
                        Mode::Linear => r.by_time(t, Pick::Floor).unwrap_or(0),
                    };
                    self.check_field(&mut r, i, &id, &field)?;
                    let s = r
                        .field_at_time(&field, t, mode)
                        .map_err(|e| err(NOT_FOUND, format!("runs/{id}/fields: {e}")))?;
                    (Some(s.values), s.lower, s.upper, s.weight, s.interpolated)
                }
                (None, None) => (None, n_snaps - 1, n_snaps - 1, 0.0, false),
            };
        let info = self.check_field(&mut r, lower, &id, &field)?;
        let values = match values {
            Some(v) => v,
            None => r.read_field(lower, &field).map_err(|e| ts(&e))?,
        };
        let snaps = r.snapshots();
        let snap = |i: usize| json!({"index": i, "step": snaps[i].step, "time": snaps[i].time});
        let mut snapshot = snap(lower);
        if interpolated {
            snapshot["upper"] = snap(upper);
            snapshot["weight"] = json!(weight);
        }
        let step = snaps[lower].step;
        Ok(Sampled {
            id,
            reader: r,
            values,
            info,
            snapshot,
            interpolated,
            step,
        })
    }

    fn check_field(
        &self,
        r: &mut rcfd_tsstore::Reader,
        i: usize,
        id: &str,
        field: &str,
    ) -> Result<rcfd_tsstore::FieldInfo, RpcError> {
        let fields = r
            .fields(i)
            .map_err(|e| err(INTERNAL, format!("runs/{id}/fields: {e}")))?;
        let names: Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
        fields.into_iter().find(|f| f.name == field).ok_or_else(|| RpcError {
            code: NOT_FOUND,
            message: format!("runs/{id}/fields: snapshot {i} has no field '{field}'"),
            data: Some(json!({"fields": names})),
        })
    }

    pub(crate) fn field_summary(&self, args: &Value) -> Result<Value, RpcError> {
        let mut s = self.sample(args)?;
        let finite: Vec<usize> = (0..s.values.len()).filter(|&i| s.values[i].is_finite()).collect();
        let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
        for &i in &finite {
            min = min.min(s.values[i]);
            max = max.max(s.values[i]);
        }
        let cell = s.info.location == rcfd_tsstore::Location::Cell;
        let vols = if cell {
            let m = s
                .reader
                .mesh()
                .map_err(|e| err(INTERNAL, format!("runs/{}/fields: {e}", s.id)))?;
            Some(m.cell_volumes().to_vec()).filter(|v| v.len() == s.values.len())
        } else {
            None
        };
        let (mean, weighting) = match &vols {
            Some(v) => (
                neumaier(finite.iter().map(|&i| s.values[i] * v[i])) / neumaier(finite.iter().map(|&i| v[i])),
                "cell_volume",
            ),
            None => (
                neumaier(finite.iter().map(|&i| s.values[i])) / finite.len() as f64,
                "none",
            ),
        };
        let none = finite.is_empty();
        Ok(json!({
            "run": s.id, "field": s.info.name, "unit": s.info.unit, "location": s.info.location,
            "precision": s.info.precision, "snapshot": s.snapshot, "interpolated": s.interpolated,
            "n": s.values.len(), "finite": finite.len(),
            "min": if none { Value::Null } else { json!(min) }, "max": if none { Value::Null } else { json!(max) },
            "mean": if none { Value::Null } else { json!(mean) }, "weighting": weighting,
        }))
    }

    pub(crate) fn render_field(&self, args: &Value) -> Result<Answer, RpcError> {
        use rcfd_post::render::{render_scalar, RenderOptions};
        let mut s = self.sample(args)?;
        if s.info.location != rcfd_tsstore::Location::Cell {
            return Err(err(
                VALIDATION,
                format!("render_field draws cell fields; '{}' is a point field", s.info.name),
            ));
        }
        let mesh = s
            .reader
            .mesh()
            .map_err(|e| err(INTERNAL, format!("runs/{}/fields: {e}", s.id)))?;
        let width = u64_arg(args, "width").unwrap_or(640).clamp(64, 4096) as usize;
        let opt = RenderOptions {
            width,
            ..Default::default()
        };
        let r = render_scalar(&mesh, &s.values, &opt).map_err(|e| err(VALIDATION, e.to_string()))?;
        if r.cells_drawn != mesh.n_cells() {
            return Err(err(
                VALIDATION,
                format!(
                    "render_field draws 2D cases only (one cell layer): the view plane cuts {} of {} cells of runs/{}",
                    r.cells_drawn,
                    mesh.n_cells(),
                    s.id
                ),
            ));
        }
        let png = r.image.to_png().map_err(|e| err(INTERNAL, e.to_string()))?;
        let b64 = base64(&png);
        let mut meta = json!({"run": s.id, "field": s.info.name, "snapshot": s.snapshot, "interpolated": s.interpolated,
            "min": r.vmin, "max": r.vmax, "width": r.image.width, "height": r.image.height, "png_bytes": png.len()});
        if b64.len() + text_len(&meta) > TEXT_BUDGET {
            let rel = format!(
                "runs/{}/renders/{}-step{}-w{width}{}.png",
                s.id,
                file_part(&s.info.name),
                s.step,
                if s.interpolated { "-interp" } else { "" }
            );
            let path = self.project.root().join(&rel);
            std::fs::create_dir_all(path.parent().expect("has a parent"))
                .map_err(|e| err(INTERNAL, format!("{rel}: {e}")))?;
            atomic::atomic_write(&path, &png).map_err(perr)?;
            meta["path"] = json!(rel);
            meta["note"] = json!("the image is over the size budget and was written to path");
            return Ok(Answer::Value(meta));
        }
        Ok(Answer::Image { png_base64: b64, meta })
    }

    pub(crate) fn compare_runs(&self, args: &Value) -> Result<Value, RpcError> {
        let runs = self.run_list_arg(args, 2, 10)?;
        let mut rows = Vec::new();
        let mut sums = Vec::new();
        for (id, dir) in &runs {
            let s = self.project.run_summary(id).map_err(perr)?;
            let lines = std::fs::read(dir.join("residuals.jsonl"))
                .map(|b| b.iter().filter(|&&c| c == b'\n').count())
                .unwrap_or(0);
            rows.push(
                json!({"id": id, "case": s.case, "state": state_name(s.state), "step": s.step, "time": s.time,
                "residual_lines": lines}),
            );
            sums.push(with_reports(self.project.root(), s));
        }
        let names: BTreeSet<&String> = sums.iter().flat_map(|s| s.metrics.keys()).collect();
        let base = &sums[0];
        let mut metrics = Map::new();
        let mut diffs = Map::new();
        let mut v = json!({"baseline": base.id, "runs": rows, "metrics": {}, "differences": {}, "metrics_total": names.len(),
            "metrics_omitted": 0});
        let mut size = text_len(&v);
        let mut omitted = 0;
        for name in &names {
            let vals: Map<String, Value> = sums
                .iter()
                .filter_map(|s| s.metrics.get(*name).map(|x| (s.id.clone(), json!(x))))
                .collect();
            let d: Map<String, Value> = match base.metrics.get(*name) {
                Some(b) => sums[1..]
                    .iter()
                    .filter_map(|s| s.metrics.get(*name).map(|x| (s.id.clone(), json!(x - b))))
                    .collect(),
                None => Map::new(),
            };
            let n = text_len(&Value::Object(vals.clone())) + text_len(&Value::Object(d.clone())) + 2 * name.len() + 8;
            if size + n > TEXT_BUDGET {
                omitted += 1;
                continue;
            }
            size += n;
            metrics.insert((*name).clone(), Value::Object(vals));
            diffs.insert((*name).clone(), Value::Object(d));
        }
        v["metrics"] = Value::Object(metrics);
        v["differences"] = Value::Object(diffs);
        v["metrics_omitted"] = json!(omitted);
        Ok(v)
    }

    pub(crate) fn list_changes(&self, args: &Value) -> Result<Value, RpcError> {
        let mut g = self.watcher.lock().unwrap_or_else(|e| e.into_inner());
        if g.is_none() {
            *g = Some(Watcher::new(self.project.root()).map_err(perr)?);
        }
        let w = g.as_mut().expect("made above");
        let token = w.poll().map_err(perr)?;
        let Some(since) = u64_arg(args, "since") else {
            return Ok(json!({"token": token, "changes": [], "total": 0, "truncated": false}));
        };
        let all = w.changes_since(since);
        let mut out = Vec::new();
        let mut size = 128;
        for p in &all {
            size += p.len() + 3;
            if size > TEXT_BUDGET {
                break;
            }
            out.push(p.clone());
        }
        Ok(json!({"since": since, "token": token, "total": all.len(),
            "truncated": out.len() < all.len(), "changes": out}))
    }

    pub(crate) fn read_project_file(&self, args: &Value) -> Result<Value, RpcError> {
        let rel = str_arg(args, "path");
        let path = self.project.resolve(rel).map_err(perr)?;
        if path.is_dir() {
            return Err(err(NOT_FOUND, format!("{rel} is a directory")));
        }
        let bytes = atomic::read(&path).map_err(perr)?;
        let text =
            std::str::from_utf8(&bytes).map_err(|e| err(VALIDATION, format!("{rel} is not UTF-8 text ({e})")))?;
        let offset = u64_arg(args, "offset").unwrap_or(0) as usize;
        if offset > text.len() || !text.is_char_boundary(offset) {
            return Err(err(
                PARAMS,
                format!(
                    "offset {offset} is not a character boundary of {rel} ({} bytes)",
                    text.len()
                ),
            ));
        }
        let limit = u64_arg(args, "limit").unwrap_or(FILE_PAGE as u64) as usize;
        let mut end = (offset + limit.min(FILE_PAGE)).min(text.len());
        loop {
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            // JSON escapes can grow the text (a control character takes 6 bytes): halve the page until it fits
            if serde_json::to_string(&text[offset..end]).map_or(0, |s| s.len()) <= TEXT_BUDGET - 1000 || end == offset {
                break;
            }
            end = offset + (end - offset) / 2;
        }
        Ok(json!({
            "path": self.rel(&path), "hash": hash_bytes(&bytes), "size": bytes.len(), "offset": offset,
            "text": &text[offset..end], "next_offset": if end < text.len() { json!(end) } else { Value::Null },
        }))
    }
}

struct Sampled {
    id: String,
    reader: rcfd_tsstore::Reader,
    values: Vec<f64>,
    info: rcfd_tsstore::FieldInfo,
    snapshot: Value,
    interpolated: bool,
    step: u64,
}

fn count_files(dir: &Path) -> usize {
    let mut n = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() && !atomic::is_temp_name(&name) => n += 1,
                _ => {}
            }
        }
    }
    n
}
