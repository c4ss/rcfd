//! The solid driver of a project run (F-990, ADR 0840 D2, D3): what the runner launches instead of `rcfd` for a run whose
//! `case.snapshot` is a solid case file (`rcfd_runner::mesh::solid_case`, ADR 0840 D1), with the arguments it gives
//! `rcfd run`. It solves the case with the library entry of `rcfd_api::solid_case` (the solver unchanged), writes the
//! `.vtu` and the report into `--out` as `run_solid_case_file` writes them, the displacement and stress of a static result
//! as cell fields into the run's field store (`--store-dir`), and its status stream (`--status-file`, `rcfd-status`
//! events): one `iteration` event whose `monitors` are the report's values, then `finish`.

use rcfd_api::case::Diagnostic;
use rcfd_api::runctl::make_event;
use rcfd_api::solid_case::{modal_vtu_string, run, SolidCase, SolidOutcome, SolidRun};
use rcfd_post::solid_output::{solid_fields, solid_vtu_string};
use rcfd_tsstore::{Location, Precision, WriteOptions, Writer};
use serde_json::{json, Map, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Exit code of a refused case or command line (ADR 0840 D2).
pub const EXIT_INVALID: i32 = 2;
/// Exit code of a failed solve or a file that could not be written.
pub const EXIT_FAIL: i32 = 1;

/// The names of the stored cell fields, in their order (ADR 0840 D2).
pub const FIELDS: [&str; 11] = [
    "ux",
    "uy",
    "uz",
    "u_magnitude",
    "von_mises",
    "sxx",
    "syy",
    "szz",
    "sxy",
    "syz",
    "szx",
];

/// The report values sent as monitors, with their place in the report.
pub const MONITORS: [(&str, &str, &str); 3] = [
    ("displacement_max_magnitude", "displacement", "max_magnitude"),
    ("von_mises_max", "stress", "von_mises_max"),
    ("element_von_mises_max", "stress", "element_von_mises_max"),
];

/// The command line the runner gives (`run <case> --case-dir <dir> ...`).
#[derive(Debug, Default)]
struct Args {
    case: Option<PathBuf>,
    case_dir: Option<PathBuf>,
    check_only: bool,
    out: Option<PathBuf>,
    status_file: Option<PathBuf>,
    store_dir: Option<PathBuf>,
    refused: Vec<String>,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args::default();
    let mut it = args.iter();
    match it.next().map(String::as_str) {
        Some("run") => {}
        _ => return Err("usage: rcfd-solid-run run <case> --case-dir <dir> [options of rcfd run]".into()),
    }
    while let Some(x) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| format!("{x} needs a value"));
        match x.as_str() {
            "--case-dir" => a.case_dir = Some(value()?.into()),
            "--out" => a.out = Some(value()?.into()),
            "--status-file" => a.status_file = Some(value()?.into()),
            "--store-dir" => a.store_dir = Some(value()?.into()),
            // accepted and not read: one solve has no step to stop or hold between (ADR 0840, not covered)
            "--status-every" | "--threads" | "--stop-file" | "--pause-file" => {
                value()?;
            }
            "--check-only" => a.check_only = true,
            "--status-json" | "--quiet" | "--status-monitors" => {}
            "--set" | "--continue-from" | "--max-steps" => {
                value()?;
                a.refused.push(x.clone());
            }
            s if s.starts_with("--") => return Err(format!("{s}: not an option of a solid run")),
            s if a.case.is_none() => a.case = Some(s.into()),
            s => return Err(format!("{s}: a solid run takes one case file")),
        }
    }
    if a.case.is_none() {
        return Err("a solid run takes one case file".into());
    }
    Ok(a)
}

fn diag_json(d: &[Diagnostic]) -> Value {
    json!(d
        .iter()
        .map(|x| json!({"path": x.path, "line": x.line, "message": x.message}))
        .collect::<Vec<_>>())
}

fn diag_text(d: &[Diagnostic]) -> String {
    d.iter()
        .take(3)
        .map(|x| format!("{}: {}", x.path, x.message))
        .collect::<Vec<_>>()
        .join("; ")
}

/// The parsed case of `args`: its text read from `--case-dir`'s point of view.
fn load(a: &Args) -> Result<SolidCase, Vec<Diagnostic>> {
    let case = a.case.as_ref().expect("checked by parse");
    let text = std::fs::read_to_string(case).map_err(|e| {
        vec![Diagnostic {
            path: String::new(),
            line: None,
            message: format!("cannot read {}: {e}", case.display()),
        }]
    })?;
    let dir = a
        .case_dir
        .clone()
        .or_else(|| case.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    SolidCase::parse_toml_in(&text, &dir)
}

/// The status stream: one JSON line per event, appended.
struct Stream {
    file: Option<std::fs::File>,
    seq: u64,
    t0: Instant,
}

impl Stream {
    fn open(p: Option<&Path>) -> Stream {
        let file = p.and_then(|p| std::fs::OpenOptions::new().create(true).append(true).open(p).ok());
        Stream {
            file,
            seq: 0,
            t0: Instant::now(),
        }
    }
    fn put(&mut self, kind: &str, body: Value) {
        let ev = make_event(kind, &mut self.seq, self.t0.elapsed().as_secs_f64(), body);
        if let Some(f) = &mut self.file {
            let _ = writeln!(f, "{ev}");
        }
    }
    fn finish(&mut self, code: i32, reason: &str) -> i32 {
        let state = if code == 0 { "finished" } else { "failed" };
        self.put(
            "finish",
            json!({"state": state, "exit_code": code, "reason": reason, "ok": code == 0, "steps": 1}),
        );
        code
    }
}

/// `rcfd-solid-run` with `args` (without the program name); returns the exit code.
pub fn main_args(args: &[String]) -> i32 {
    let a = match parse(args) {
        Ok(a) => a,
        Err(m) => {
            eprintln!("error: {m}");
            return EXIT_INVALID;
        }
    };
    if a.check_only {
        let line = match (a.refused.first(), load(&a).and_then(|c| c.build().map(|s| (c, s)))) {
            (Some(r), _) => json!({"ok": false, "diagnostics": [{"path": "", "line": null,
                "message": format!("{r}: a solid case file takes no {r}")}]}),
            (None, Ok((_, s))) => json!({"ok": true, "cells": s.model.mesh().n_elements()}),
            (None, Err(d)) => json!({"ok": false, "diagnostics": diag_json(&d)}),
        };
        println!("{line}");
        return if line["ok"] == true { 0 } else { EXIT_INVALID };
    }
    let mut s = Stream::open(a.status_file.as_deref());
    if let Some(r) = a.refused.first() {
        eprintln!("error: {r}: a solid case file takes no {r}");
        return s.finish(EXIT_INVALID, &format!("{r}: a solid case file takes no {r}"));
    }
    let setup = match load(&a).and_then(|c| c.build()) {
        Ok(x) => x,
        Err(d) => {
            eprintln!("error: {}", diag_text(&d));
            return s.finish(EXIT_INVALID, &diag_text(&d));
        }
    };
    if setup.reduction.is_some() || !setup.superelements.is_empty() {
        return s.finish(
            EXIT_INVALID,
            "a solid case with [reduction] or [superelements] is not run as a project run (ADR 0840, not covered)",
        );
    }
    let mut r = match run(setup) {
        Ok(r) => r,
        Err(d) => return s.finish(EXIT_FAIL, &diag_text(&d)),
    };
    let out = a.out.clone().unwrap_or_else(|| PathBuf::from("."));
    if let Err(m) = write_outputs(&mut r, &out) {
        return s.finish(EXIT_FAIL, &m);
    }
    if let (Some(dir), SolidOutcome::Static { .. }) = (&a.store_dir, &r.outcome) {
        if let Err(m) = store_fields(&r, dir) {
            return s.finish(EXIT_FAIL, &format!("field store: {m}"));
        }
    }
    let mut monitors = Map::new();
    for (name, group, key) in MONITORS {
        if let Some(v) = r.report.get(group).and_then(|g| g.get(key)) {
            monitors.insert(name.into(), v.clone());
        }
    }
    s.put(
        "iteration",
        json!({"index": 1, "time": 0.0, "monitors": Value::Object(monitors)}),
    );
    let what = match &r.outcome {
        SolidOutcome::Static { .. } => "static solve",
        SolidOutcome::Modal(_) => "modal analysis",
    };
    s.finish(0, &format!("{what} of the solid case done"))
}

/// The `.vtu` and the report of `r` in `dir`, as `run_solid_case_file` writes them (ADR 0438 D7).
pub fn write_outputs(r: &mut SolidRun, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut outputs = Map::new();
    if !r.setup.vtu.is_empty() {
        let p = dir.join(&r.setup.vtu);
        let text = match &r.outcome {
            SolidOutcome::Static { result, .. } => solid_vtu_string(&r.setup.model, result),
            SolidOutcome::Modal(m) => modal_vtu_string(&r.setup.model, m),
        }
        .map_err(|e| format!("output.vtu: {e}"))?;
        std::fs::write(&p, text).map_err(|e| format!("output.vtu: {}: {e}", p.display()))?;
        outputs.insert("vtu".into(), json!(p.to_string_lossy()));
        r.written.push(p);
    }
    let report = (!r.setup.report.is_empty()).then(|| dir.join(&r.setup.report));
    if let Some(p) = &report {
        outputs.insert("report".into(), json!(p.to_string_lossy()));
    }
    if let Value::Object(m) = &mut r.report {
        m.insert("outputs".into(), Value::Object(outputs));
    }
    if let Some(p) = report {
        let text = serde_json::to_string_pretty(&r.report).expect("a JSON value serialises");
        std::fs::write(&p, text).map_err(|e| format!("output.report: {}: {e}", p.display()))?;
        r.written.push(p);
    }
    Ok(())
}

/// ADR 0840 D3: the cell values of a static result, in the order of [`FIELDS`], one per element; `None` for a modal
/// result.
pub fn cell_fields(r: &SolidRun) -> Result<Option<Vec<Vec<f64>>>, String> {
    let SolidOutcome::Static { result, .. } = &r.outcome else {
        return Ok(None);
    };
    let model = &r.setup.model;
    let mesh = model.mesh();
    let f = solid_fields(model, result).map_err(|e| e.to_string())?;
    let n = mesh.n_elements();
    let mut out = vec![Vec::with_capacity(n); FIELDS.len()];
    for e in 0..n {
        let corners = element_kind_corners(mesh, e);
        let nodes = &mesh.element_nodes(e)[..corners];
        let mut u = [0.0; 3];
        for &a in nodes {
            for (c, uc) in u.iter_mut().enumerate() {
                *uc += f.displacement[3 * a + c];
            }
        }
        for uc in &mut u {
            *uc /= nodes.len() as f64;
        }
        let s: [f64; 6] = f.element_stress[6 * e..6 * e + 6].try_into().expect("6 per element");
        let mag = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
        let vm = rcfd_fem::recover::von_mises(&s);
        for (k, v) in [u[0], u[1], u[2], mag, vm, s[0], s[1], s[2], s[3], s[4], s[5]]
            .into_iter()
            .enumerate()
        {
            out[k].push(v);
        }
    }
    Ok(Some(out))
}

/// The number of corner nodes of element `e`.
fn element_kind_corners(mesh: &rcfd_fem::SolidMesh, e: usize) -> usize {
    mesh.element_kind(e).corners()
}

/// ADR 0840 D3: the FE mesh of `r` as a finite-volume mesh: one cell per element, its faces the kind's local faces by
/// their corner nodes, its points the corner nodes in node order.
pub fn cell_mesh(r: &SolidRun) -> Result<rcfd_mesh::Mesh, String> {
    let mesh = r.setup.model.mesh();
    let n = mesh.nodes().len();
    let mut used = vec![false; n];
    for e in 0..mesh.n_elements() {
        for &a in &mesh.element_nodes(e)[..element_kind_corners(mesh, e)] {
            used[a] = true;
        }
    }
    let mut map = vec![usize::MAX; n];
    let mut points = Vec::new();
    for a in 0..n {
        if used[a] {
            map[a] = points.len();
            points.push(mesh.nodes()[a]);
        }
    }
    let mut cells = Vec::with_capacity(mesh.n_elements());
    for e in 0..mesh.n_elements() {
        let kind = mesh.element_kind(e);
        let nodes = mesh.element_nodes(e);
        let corners = kind.face_corners();
        if corners == 0 {
            return Err(format!("element {e} is a {} (not a volume element)", kind.name()));
        }
        cells.push(
            kind.faces()
                .iter()
                .map(|f| f[..corners].iter().map(|&l| map[nodes[l]]).collect())
                .collect::<Vec<Vec<usize>>>(),
        );
    }
    rcfd_mesh::Mesh::from_cell_faces(points, &cells, &[]).map_err(|e| e.to_string())
}

/// The field store of a static result `r` at `dir`: the mesh of [`cell_mesh`], one snapshot (step 1, time 0) of the cell
/// fields of [`cell_fields`], f64 (ADR 0840 D2).
pub fn store_fields(r: &SolidRun, dir: &Path) -> Result<(), String> {
    let Some(values) = cell_fields(r)? else {
        return Ok(());
    };
    let mesh = cell_mesh(r)?;
    let mut w = Writer::create(dir, &mesh, WriteOptions::default()).map_err(|e| e.to_string())?;
    {
        let mut snap = w.snapshot(1, 0.0).map_err(|e| e.to_string())?;
        for (name, v) in FIELDS.iter().zip(&values) {
            let unit = if name.starts_with('u') { "m" } else { "Pa" };
            snap.field(name, unit, Location::Cell, v, Precision::F64)
                .map_err(|e| e.to_string())?;
        }
        snap.commit().map_err(|e| e.to_string())?;
    }
    w.finish().map_err(|e| e.to_string())
}
