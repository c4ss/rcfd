//! ADR 0823 (F-980): `export_view` through the `rcfd-mcp` binary, on the f980 project (a copy of quick-start with
//! `meshes/box.msh`, the cylinder's case and run of wind-tunnel and the fan's case and run of axial-fan). R1: the pages of
//! the registered set X1 to X5 are written under `exports/` as the reply says. R2: each page's values equal the engine's
//! tools (X1 to X4). R3: no page refers to anything outside it, each carries the CSP and the Studio's styles. R4: every
//! panel names the files it read with their hashes, which `hash_project_file` gives. R7: one `view_exported` journal line
//! per export. R5: `export_view {}` exports the view shown last. R6: four refusals write nothing.

mod common;

use common::Mcp;
use rcfd_workspace::view::{page_references, EXPORT_CSP, EXPORT_STYLES, VIEW_EXPORTED};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const RUN: &str = "r-20261028T152641-3900";
const OTHER: &str = "r-20261028T152641-5cc1";
const D5CB: &str = "r-20261028T152128-d5cb";
const FAN: &str = "r-20261030T223519-0489";
const RE100: &str = "cases/cavity-re100.toml";
/// ADR 0823 D3: a number agrees to a relative difference of at most 1e-12 (tolerances.md, "Same discrete problem by
/// two routes", where only rounding separates the two).
const REL: f64 = 1e-12;

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let t = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &t);
        } else {
            std::fs::copy(e.path(), t).unwrap();
        }
    }
}

/// ADR 0823 D3: the f980 project.
fn f980(tag: &str) -> PathBuf {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = common::scratch(&format!("f980-{tag}")).join("quick-start");
    copy_dir(&repo.join("example-projects/quick-start"), &dir);
    std::fs::write(dir.join("meshes/box.msh"), "$MeshFormat\n2.2 0 8\n$EndMeshFormat\n").unwrap();
    for (p, kase, run) in [
        ("wind-tunnel", "cylinder-re100.toml", D5CB),
        ("axial-fan", "fan01-passage-upwind.toml", FAN),
    ] {
        let from = repo.join("example-projects").join(p);
        std::fs::copy(from.join("cases").join(kase), dir.join("cases").join(kase)).unwrap();
        copy_dir(&from.join("runs").join(run), &dir.join("runs").join(run));
    }
    dir
}

/// X1: ADR 0805 V2's view.
fn x1() -> Value {
    json!({"version": 1, "title": "Cavity", "layout": {"columns": 2}, "panels": [
        {"kind": "case", "case": RE100},
        {"kind": "run", "run": RUN},
        {"kind": "field", "run": RUN, "field": "p"},
        {"kind": "mesh", "mesh": "meshes/box.msh"},
        {"kind": "plot", "run": RUN},
        {"kind": "monitor", "run": RUN, "monitor": "nu_hot"},
        {"kind": "table", "runs": [RUN, OTHER]}
    ]})
}

/// X2: ADR 0807 V3's view.
fn x2() -> Value {
    let mut v = x1();
    let p = v["panels"].as_array_mut().unwrap();
    p.remove(6);
    p.remove(4);
    p.remove(1);
    p.push(json!({"kind": "mesh", "case": RE100}));
    p.push(json!({"kind": "setting", "case": RE100, "setting": "physics.nu"}));
    p.push(json!({"kind": "monitor", "case": RE100, "monitor": "lid_drag"}));
    p.push(json!({"kind": "table", "runs": [RUN, OTHER]}));
    v
}

fn registered() -> Vec<(&'static str, Value, usize)> {
    vec![
        ("X1", x1(), 7),
        ("X2", x2(), 8),
        ("X3", json!({"version": 1, "run": D5CB}), 6),
        ("X4", json!({"version": 1, "run": RUN}), 6),
        ("X5", json!({"version": 1, "run": FAN}), 7),
    ]
}

fn d(dir: &Path) -> String {
    dir.display().to_string()
}

fn ok(r: &Value) -> Value {
    assert_eq!(
        r["isError"],
        false,
        "{}",
        &r.to_string()[..r.to_string().len().min(800)]
    );
    r["structuredContent"].clone()
}

fn error_of(r: &Value) -> Value {
    assert_eq!(r["isError"], true, "{r}");
    r["structuredContent"]["error"].clone()
}

fn journal(dir: &Path) -> Vec<u8> {
    std::fs::read(dir.join(".rcfd/edits.jsonl")).unwrap_or_default()
}

fn unescape(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// The value of the first attribute `name` in `s`.
fn attr(s: &str, name: &str) -> Option<String> {
    let key = format!(" {name}=\"");
    let at = s.find(&key)? + key.len();
    let end = s[at..].find('"')?;
    Some(unescape(&s[at..at + end]))
}

/// Every value of attribute `name` in `s`, in order.
fn attrs(s: &str, name: &str) -> Vec<String> {
    let key = format!(" {name}=\"");
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(at) = rest.find(&key) {
        let from = at + key.len();
        let end = rest[from..].find('"').unwrap();
        out.push(unescape(&rest[from..from + end]));
        rest = &rest[from + end..];
    }
    out
}

/// The markup of each panel of a page.
fn panels(page: &str) -> Vec<String> {
    page.split("<div class=\"cv-panel\"")
        .skip(1)
        .map(String::from)
        .collect()
}

fn close(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= REL * a.abs().max(b.abs())
}

fn f(s: &Option<String>) -> f64 {
    s.as_deref().and_then(|x| x.parse::<f64>().ok()).unwrap_or(f64::NAN)
}

/// The series of `name` of `run` as the Studio reads it (`max_points` 800).
fn series(c: &mut Mcp, run: &str, name: &str) -> (Vec<f64>, Vec<f64>) {
    let r = ok(&c.call(
        "read_residuals",
        json!({"run": run, "names": [name], "max_points": 800}),
    ));
    let s = &r["series"][name];
    let arr = |k: &str| -> Vec<f64> {
        s[k].as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap_or(f64::NAN))
            .collect()
    };
    (arr("x"), arr("y"))
}

/// ADR 0823 R2: the values of one page against the engine's tools; returns the panels checked.
fn check_values(c: &mut Mcp, tag: &str, page: &str, settings: &Value) -> usize {
    let runs = ok(&c.call("list_runs", json!({"limit": 1000})))["runs"].clone();
    let step_of = |id: &str| -> Value {
        runs.as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == json!(id))
            .map(|r| r["step"].clone())
            .unwrap()
    };
    let mut n = 0;
    for (i, p) in panels(page).iter().enumerate() {
        let kind = attr(p, "data-kind").unwrap();
        match kind.as_str() {
            "field" => {
                let run = attr(p, "data-sources").unwrap();
                let run = run.split('/').nth(1).unwrap().to_string();
                let field = attr(p, "data-field").unwrap();
                let s = ok(&c.call("field_summary", json!({"run": run, "field": field})));
                let (lo, hi) = (
                    s["min"].as_f64().unwrap() as f32 as f64,
                    s["max"].as_f64().unwrap() as f32 as f64,
                );
                let (plo, phi) = (f(&attr(p, "data-lo")), f(&attr(p, "data-hi")));
                eprintln!("R2 {tag} panel {i} field {field} of {run}: page {plo} {phi}, field_summary {lo} {hi}, snapshot {:?} step {:?} against {}", attr(p, "data-snapshot"), attr(p, "data-step"), s["snapshot"]);
                assert!(close(plo, lo) && close(phi, hi), "R2 {tag} panel {i}: the range");
                assert_eq!(
                    attr(p, "data-snapshot").unwrap(),
                    s["snapshot"]["index"].to_string(),
                    "R2 {tag} panel {i}: the snapshot"
                );
                assert_eq!(
                    attr(p, "data-step").unwrap(),
                    s["snapshot"]["step"].to_string(),
                    "R2 {tag} panel {i}: the step"
                );
            }
            "monitor" => {
                let run = attr(p, "data-run").unwrap();
                let title = attr(p, "data-sources").unwrap();
                assert!(title.contains(&run));
                let m = p
                    .split("<h3 class=\"cv-panel-title\">")
                    .nth(1)
                    .map(|t| t.split('<').next().unwrap().to_string());
                // the monitor's name: from the facts line "<m> at step"
                let name = p
                    .split("<p class=\"xv-facts\">")
                    .nth(1)
                    .unwrap()
                    .split(" at step")
                    .next()
                    .unwrap()
                    .to_string();
                let (x, y) = series(c, &run, &format!("monitors.{}", unescape(&name)));
                let (ps, pv) = (f(&attr(p, "data-last-step")), f(&attr(p, "data-last-value")));
                eprintln!(
                    "R2 {tag} panel {i} monitor {name} ({m:?}) of {run}: page {ps} {pv}, read_residuals {} {}",
                    x[x.len() - 1],
                    y[y.len() - 1]
                );
                assert!(
                    close(ps, x[x.len() - 1]) && close(pv, y[y.len() - 1]),
                    "R2 {tag} panel {i}: the last sample"
                );
            }
            "plot" => {
                let run = attr(p, "data-run").unwrap();
                let names = ok(&c.call("read_residuals", json!({"run": run, "max_points": 2})))["series"].clone();
                let (mut drawn, mut skipped) = (0usize, 0usize);
                for k in names
                    .as_object()
                    .unwrap()
                    .keys()
                    .filter(|k| k.starts_with("residuals."))
                {
                    let (x, y) = series(c, &run, k);
                    for (a, b) in x.iter().zip(&y) {
                        if a.is_finite() && *b > 0.0 && b.log10().is_finite() {
                            drawn += 1;
                        } else {
                            skipped += 1;
                        }
                    }
                }
                eprintln!(
                    "R2 {tag} panel {i} plot of {run}: page {:?} {:?}, read_residuals {drawn} {skipped}",
                    attr(p, "data-drawn"),
                    attr(p, "data-skipped")
                );
                assert_eq!(
                    attr(p, "data-drawn").unwrap(),
                    drawn.to_string(),
                    "R2 {tag} panel {i}: drawn"
                );
                assert_eq!(
                    attr(p, "data-skipped").unwrap(),
                    skipped.to_string(),
                    "R2 {tag} panel {i}: skipped"
                );
            }
            "run" => {
                let run = attr(p, "data-run").unwrap();
                eprintln!(
                    "R2 {tag} panel {i} run {run}: page step {:?}, list_runs {}",
                    attr(p, "data-step"),
                    step_of(&run)
                );
                assert_eq!(
                    attr(p, "data-step").unwrap(),
                    step_of(&run).to_string(),
                    "R2 {tag} panel {i}: the step"
                );
            }
            "table" => {
                let ids = attrs(p, "data-run");
                let steps = attrs(p, "data-step");
                eprintln!("R2 {tag} panel {i} table: {ids:?} {steps:?}");
                assert_eq!(ids.len(), 2);
                for (id, s) in ids.iter().zip(&steps) {
                    assert_eq!(*s, step_of(id).to_string(), "R2 {tag} panel {i}: the step of {id}");
                }
            }
            "setting" => {
                let want = settings
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|s| s["panel"] == json!(i))
                    .unwrap()["value"]
                    .to_string();
                eprintln!(
                    "R2 {tag} panel {i} setting: page {:?}, read_view {want}",
                    attr(p, "data-value")
                );
                assert_eq!(attr(p, "data-value").unwrap(), want, "R2 {tag} panel {i}: the setting");
            }
            _ => continue,
        }
        n += 1;
    }
    n
}

/// ADR 0823 R1, R2, R3, R4, R7: the pages of the registered set.
#[test]
#[ignore = "ADR 0823 amendment 1: a record; its list_runs limit 1000 is above the tool maximum of 100"]
fn r1_r2_r3_r4_r7_the_pages_of_the_registered_set() {
    let dir = f980("r1");
    let mut c = Mcp::start(&["--project", &d(&dir)]);
    // the settings of X2 as read_view gives them (R2), from a Studio's reading server
    let mut studio = Mcp::start(&["--project", &d(&dir), "--client", "studio", "--read-only", "--canvas"]);
    studio.request("tools/list", json!({}));
    let mut lines = 0;
    for (tag, view, n) in registered() {
        let before = journal(&dir);
        let t0 = std::time::Instant::now();
        let r = ok(&c.call("export_view", json!({"view": view, "reason": format!("F-980 {tag}")})));
        let ms = t0.elapsed().as_millis();
        let path = dir.join(r["path"].as_str().unwrap());
        let bytes = std::fs::read(&path).unwrap();
        eprintln!(
            "R1 {tag}: {} in {ms} ms, {} bytes, {} panels",
            r["path"],
            bytes.len(),
            r["panels"]
        );
        // R1
        assert_eq!(r["exported"], true);
        assert!(r["path"].as_str().unwrap().starts_with("exports/"));
        assert_eq!(r["bytes"], json!(bytes.len()));
        assert_eq!(r["hash"], json!(rcfd_project::records::hash_bytes(&bytes)));
        assert_eq!(r["panels"], json!(n));
        let page = String::from_utf8(bytes).unwrap();
        assert!(page.starts_with("<!doctype html>"));
        assert_eq!(panels(&page).len(), n, "R1 {tag}: the panels");
        // R3
        let refs = page_references(&page);
        eprintln!("R3 {tag}: references {refs:?}");
        assert!(refs.is_empty(), "R3 {tag}: {refs:?}");
        assert!(page.contains(&format!(
            "<meta http-equiv=\"Content-Security-Policy\" content=\"{EXPORT_CSP}\">"
        )));
        for (name, css) in &EXPORT_STYLES[..3] {
            assert!(page.contains(css), "R3 {tag}: {name} is not in the page");
        }
        // R4
        assert!(page.contains(&format!("data-engine=\"{}\"", env!("CARGO_PKG_VERSION"))));
        assert_eq!(attr(&page, "data-export-view").unwrap(), r["id"].as_str().unwrap());
        let mut files = 0;
        for p in panels(&page) {
            let sources = attr(&p, "data-sources").unwrap();
            let hashes = attr(&p, "data-hashes").unwrap();
            assert!(!sources.is_empty());
            for (s, h) in sources.split(' ').zip(hashes.split(' ')) {
                assert!(dir.join(s).is_file(), "R4 {tag}: {s} is not a file");
                let got = ok(&c.call("hash_project_file", json!({"path": s})));
                assert_eq!(got["hash"].as_str().unwrap(), h, "R4 {tag}: the hash of {s}");
                files += 1;
            }
        }
        eprintln!("R4 {tag}: {files} files named with their hashes");
        // R2 (X1 to X4)
        if tag != "X5" {
            let settings = if tag == "X2" {
                ok(&c.call("show_view", json!({"view": view})));
                ok(&studio.call("read_view", json!({})))["settings"].clone()
            } else {
                Value::Null
            };
            let checked = check_values(&mut c, tag, &page, &settings);
            eprintln!("R2 {tag}: {checked} panels with values checked");
        }
        // R7
        let after = journal(&dir);
        let new: Vec<Value> = String::from_utf8_lossy(&after[before.len()..])
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .filter(|l: &Value| l["kind"] == VIEW_EXPORTED)
            .collect();
        assert_eq!(new.len(), 1, "R7 {tag}");
        assert_eq!(new[0]["path"], r["path"]);
        assert_eq!(new[0]["hash"], r["hash"]);
        assert_eq!(new[0]["bytes"], r["bytes"]);
        lines += 1;
    }
    eprintln!("R7: {lines} view_exported lines");
}

/// ADR 0823 amendment 1: R2 with list_runs asked for at most 100 runs (the tool's maximum); otherwise check_values.
fn check_values_a1(c: &mut Mcp, tag: &str, page: &str, settings: &Value) -> usize {
    let runs = ok(&c.call("list_runs", json!({"limit": 100})))["runs"].clone();
    let step_of = |id: &str| -> Value {
        runs.as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == json!(id))
            .map(|r| r["step"].clone())
            .unwrap()
    };
    let mut n = 0;
    for (i, p) in panels(page).iter().enumerate() {
        let kind = attr(p, "data-kind").unwrap();
        match kind.as_str() {
            "field" => {
                let run = attr(p, "data-sources").unwrap();
                let run = run.split('/').nth(1).unwrap().to_string();
                let field = attr(p, "data-field").unwrap();
                let s = ok(&c.call("field_summary", json!({"run": run, "field": field})));
                let (lo, hi) = (
                    s["min"].as_f64().unwrap() as f32 as f64,
                    s["max"].as_f64().unwrap() as f32 as f64,
                );
                let (plo, phi) = (f(&attr(p, "data-lo")), f(&attr(p, "data-hi")));
                eprintln!("R2 {tag} panel {i} field {field} of {run}: page {plo} {phi}, field_summary {lo} {hi}, snapshot {:?} step {:?} against {}", attr(p, "data-snapshot"), attr(p, "data-step"), s["snapshot"]);
                assert!(close(plo, lo) && close(phi, hi), "R2 {tag} panel {i}: the range");
                assert_eq!(
                    attr(p, "data-snapshot").unwrap(),
                    s["snapshot"]["index"].to_string(),
                    "R2 {tag} panel {i}: the snapshot"
                );
                assert_eq!(
                    attr(p, "data-step").unwrap(),
                    s["snapshot"]["step"].to_string(),
                    "R2 {tag} panel {i}: the step"
                );
            }
            "monitor" => {
                let run = attr(p, "data-run").unwrap();
                let title = attr(p, "data-sources").unwrap();
                assert!(title.contains(&run));
                let m = p
                    .split("<h3 class=\"cv-panel-title\">")
                    .nth(1)
                    .map(|t| t.split('<').next().unwrap().to_string());
                // the monitor's name: from the facts line "<m> at step"
                let name = p
                    .split("<p class=\"xv-facts\">")
                    .nth(1)
                    .unwrap()
                    .split(" at step")
                    .next()
                    .unwrap()
                    .to_string();
                let (x, y) = series(c, &run, &format!("monitors.{}", unescape(&name)));
                let (ps, pv) = (f(&attr(p, "data-last-step")), f(&attr(p, "data-last-value")));
                eprintln!(
                    "R2 {tag} panel {i} monitor {name} ({m:?}) of {run}: page {ps} {pv}, read_residuals {} {}",
                    x[x.len() - 1],
                    y[y.len() - 1]
                );
                assert!(
                    close(ps, x[x.len() - 1]) && close(pv, y[y.len() - 1]),
                    "R2 {tag} panel {i}: the last sample"
                );
            }
            "plot" => {
                let run = attr(p, "data-run").unwrap();
                let names = ok(&c.call("read_residuals", json!({"run": run, "max_points": 2})))["series"].clone();
                let (mut drawn, mut skipped) = (0usize, 0usize);
                for k in names
                    .as_object()
                    .unwrap()
                    .keys()
                    .filter(|k| k.starts_with("residuals."))
                {
                    let (x, y) = series(c, &run, k);
                    for (a, b) in x.iter().zip(&y) {
                        if a.is_finite() && *b > 0.0 && b.log10().is_finite() {
                            drawn += 1;
                        } else {
                            skipped += 1;
                        }
                    }
                }
                eprintln!(
                    "R2 {tag} panel {i} plot of {run}: page {:?} {:?}, read_residuals {drawn} {skipped}",
                    attr(p, "data-drawn"),
                    attr(p, "data-skipped")
                );
                assert_eq!(
                    attr(p, "data-drawn").unwrap(),
                    drawn.to_string(),
                    "R2 {tag} panel {i}: drawn"
                );
                assert_eq!(
                    attr(p, "data-skipped").unwrap(),
                    skipped.to_string(),
                    "R2 {tag} panel {i}: skipped"
                );
            }
            "run" => {
                let run = attr(p, "data-run").unwrap();
                eprintln!(
                    "R2 {tag} panel {i} run {run}: page step {:?}, list_runs {}",
                    attr(p, "data-step"),
                    step_of(&run)
                );
                assert_eq!(
                    attr(p, "data-step").unwrap(),
                    step_of(&run).to_string(),
                    "R2 {tag} panel {i}: the step"
                );
            }
            "table" => {
                let ids = attrs(p, "data-run");
                let steps = attrs(p, "data-step");
                eprintln!("R2 {tag} panel {i} table: {ids:?} {steps:?}");
                assert_eq!(ids.len(), 2);
                for (id, s) in ids.iter().zip(&steps) {
                    assert_eq!(*s, step_of(id).to_string(), "R2 {tag} panel {i}: the step of {id}");
                }
            }
            "setting" => {
                let want = settings
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|s| s["panel"] == json!(i))
                    .unwrap()["value"]
                    .to_string();
                eprintln!(
                    "R2 {tag} panel {i} setting: page {:?}, read_view {want}",
                    attr(p, "data-value")
                );
                assert_eq!(attr(p, "data-value").unwrap(), want, "R2 {tag} panel {i}: the setting");
            }
            _ => continue,
        }
        n += 1;
    }
    n
}

/// ADR 0823 R1, R2, R3, R4, R7 as amendment 1 corrects them: the pages of the registered set.
#[test]
#[ignore = "ADR 0823 amendment 2: a record; it holds every table to two rows, X3 to X5 have one"]
fn r1_r2_r3_r4_r7_a1_the_pages_of_the_registered_set() {
    let dir = f980("r1");
    let mut c = Mcp::start(&["--project", &d(&dir)]);
    // the settings of X2 as read_view gives them (R2), from a Studio's reading server
    let mut studio = Mcp::start(&["--project", &d(&dir), "--client", "studio", "--read-only", "--canvas"]);
    studio.request("tools/list", json!({}));
    let mut lines = 0;
    for (tag, view, n) in registered() {
        let before = journal(&dir);
        let t0 = std::time::Instant::now();
        let r = ok(&c.call("export_view", json!({"view": view, "reason": format!("F-980 {tag}")})));
        let ms = t0.elapsed().as_millis();
        let path = dir.join(r["path"].as_str().unwrap());
        let bytes = std::fs::read(&path).unwrap();
        eprintln!(
            "R1 {tag}: {} in {ms} ms, {} bytes, {} panels",
            r["path"],
            bytes.len(),
            r["panels"]
        );
        // R1
        assert_eq!(r["exported"], true);
        assert!(r["path"].as_str().unwrap().starts_with("exports/"));
        assert_eq!(r["bytes"], json!(bytes.len()));
        assert_eq!(r["hash"], json!(rcfd_project::records::hash_bytes(&bytes)));
        assert_eq!(r["panels"], json!(n));
        let page = String::from_utf8(bytes).unwrap();
        assert!(page.starts_with("<!doctype html>"));
        assert_eq!(panels(&page).len(), n, "R1 {tag}: the panels");
        // R3
        let refs = page_references(&page);
        eprintln!("R3 {tag}: references {refs:?}");
        assert!(refs.is_empty(), "R3 {tag}: {refs:?}");
        assert!(page.contains(&format!(
            "<meta http-equiv=\"Content-Security-Policy\" content=\"{EXPORT_CSP}\">"
        )));
        for (name, css) in &EXPORT_STYLES[..3] {
            assert!(page.contains(css), "R3 {tag}: {name} is not in the page");
        }
        // R4
        assert!(page.contains(&format!("data-engine=\"{}\"", env!("CARGO_PKG_VERSION"))));
        assert_eq!(attr(&page, "data-export-view").unwrap(), r["id"].as_str().unwrap());
        let mut files = 0;
        for p in panels(&page) {
            let sources = attr(&p, "data-sources").unwrap();
            let hashes = attr(&p, "data-hashes").unwrap();
            assert!(!sources.is_empty());
            for (s, h) in sources.split(' ').zip(hashes.split(' ')) {
                assert!(dir.join(s).is_file(), "R4 {tag}: {s} is not a file");
                let got = ok(&c.call("hash_project_file", json!({"path": s})));
                assert_eq!(got["hash"].as_str().unwrap(), h, "R4 {tag}: the hash of {s}");
                files += 1;
            }
        }
        eprintln!("R4 {tag}: {files} files named with their hashes");
        // R2 (X1 to X4)
        if tag != "X5" {
            let settings = if tag == "X2" {
                ok(&c.call("show_view", json!({"view": view})));
                ok(&studio.call("read_view", json!({})))["settings"].clone()
            } else {
                Value::Null
            };
            let checked = check_values_a1(&mut c, tag, &page, &settings);
            eprintln!("R2 {tag}: {checked} panels with values checked");
        }
        // R7
        let after = journal(&dir);
        let new: Vec<Value> = String::from_utf8_lossy(&after[before.len()..])
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .filter(|l: &Value| l["kind"] == VIEW_EXPORTED)
            .collect();
        assert_eq!(new.len(), 1, "R7 {tag}");
        assert_eq!(new[0]["path"], r["path"]);
        assert_eq!(new[0]["hash"], r["hash"]);
        assert_eq!(new[0]["bytes"], r["bytes"]);
        lines += 1;
    }
    eprintln!("R7: {lines} view_exported lines");
}

/// ADR 0823 amendment 2: as check_values_a1, a table with as many rows as it lists (one or more), each with its step.
fn check_values_a2(c: &mut Mcp, tag: &str, page: &str, settings: &Value) -> usize {
    let runs = ok(&c.call("list_runs", json!({"limit": 100})))["runs"].clone();
    let step_of = |id: &str| -> Value {
        runs.as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == json!(id))
            .map(|r| r["step"].clone())
            .unwrap()
    };
    let mut n = 0;
    for (i, p) in panels(page).iter().enumerate() {
        let kind = attr(p, "data-kind").unwrap();
        match kind.as_str() {
            "field" => {
                let run = attr(p, "data-sources").unwrap();
                let run = run.split('/').nth(1).unwrap().to_string();
                let field = attr(p, "data-field").unwrap();
                let s = ok(&c.call("field_summary", json!({"run": run, "field": field})));
                let (lo, hi) = (
                    s["min"].as_f64().unwrap() as f32 as f64,
                    s["max"].as_f64().unwrap() as f32 as f64,
                );
                let (plo, phi) = (f(&attr(p, "data-lo")), f(&attr(p, "data-hi")));
                eprintln!("R2 {tag} panel {i} field {field} of {run}: page {plo} {phi}, field_summary {lo} {hi}, snapshot {:?} step {:?} against {}", attr(p, "data-snapshot"), attr(p, "data-step"), s["snapshot"]);
                assert!(close(plo, lo) && close(phi, hi), "R2 {tag} panel {i}: the range");
                assert_eq!(
                    attr(p, "data-snapshot").unwrap(),
                    s["snapshot"]["index"].to_string(),
                    "R2 {tag} panel {i}: the snapshot"
                );
                assert_eq!(
                    attr(p, "data-step").unwrap(),
                    s["snapshot"]["step"].to_string(),
                    "R2 {tag} panel {i}: the step"
                );
            }
            "monitor" => {
                let run = attr(p, "data-run").unwrap();
                let title = attr(p, "data-sources").unwrap();
                assert!(title.contains(&run));
                let m = p
                    .split("<h3 class=\"cv-panel-title\">")
                    .nth(1)
                    .map(|t| t.split('<').next().unwrap().to_string());
                // the monitor's name: from the facts line "<m> at step"
                let name = p
                    .split("<p class=\"xv-facts\">")
                    .nth(1)
                    .unwrap()
                    .split(" at step")
                    .next()
                    .unwrap()
                    .to_string();
                let (x, y) = series(c, &run, &format!("monitors.{}", unescape(&name)));
                let (ps, pv) = (f(&attr(p, "data-last-step")), f(&attr(p, "data-last-value")));
                eprintln!(
                    "R2 {tag} panel {i} monitor {name} ({m:?}) of {run}: page {ps} {pv}, read_residuals {} {}",
                    x[x.len() - 1],
                    y[y.len() - 1]
                );
                assert!(
                    close(ps, x[x.len() - 1]) && close(pv, y[y.len() - 1]),
                    "R2 {tag} panel {i}: the last sample"
                );
            }
            "plot" => {
                let run = attr(p, "data-run").unwrap();
                let names = ok(&c.call("read_residuals", json!({"run": run, "max_points": 2})))["series"].clone();
                let (mut drawn, mut skipped) = (0usize, 0usize);
                for k in names
                    .as_object()
                    .unwrap()
                    .keys()
                    .filter(|k| k.starts_with("residuals."))
                {
                    let (x, y) = series(c, &run, k);
                    for (a, b) in x.iter().zip(&y) {
                        if a.is_finite() && *b > 0.0 && b.log10().is_finite() {
                            drawn += 1;
                        } else {
                            skipped += 1;
                        }
                    }
                }
                eprintln!(
                    "R2 {tag} panel {i} plot of {run}: page {:?} {:?}, read_residuals {drawn} {skipped}",
                    attr(p, "data-drawn"),
                    attr(p, "data-skipped")
                );
                assert_eq!(
                    attr(p, "data-drawn").unwrap(),
                    drawn.to_string(),
                    "R2 {tag} panel {i}: drawn"
                );
                assert_eq!(
                    attr(p, "data-skipped").unwrap(),
                    skipped.to_string(),
                    "R2 {tag} panel {i}: skipped"
                );
            }
            "run" => {
                let run = attr(p, "data-run").unwrap();
                eprintln!(
                    "R2 {tag} panel {i} run {run}: page step {:?}, list_runs {}",
                    attr(p, "data-step"),
                    step_of(&run)
                );
                assert_eq!(
                    attr(p, "data-step").unwrap(),
                    step_of(&run).to_string(),
                    "R2 {tag} panel {i}: the step"
                );
            }
            "table" => {
                let ids = attrs(p, "data-run");
                let steps = attrs(p, "data-step");
                eprintln!("R2 {tag} panel {i} table: {ids:?} {steps:?}");
                assert!(!ids.is_empty() && ids.len() == steps.len());
                for (id, s) in ids.iter().zip(&steps) {
                    assert_eq!(*s, step_of(id).to_string(), "R2 {tag} panel {i}: the step of {id}");
                }
            }
            "setting" => {
                let want = settings
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|s| s["panel"] == json!(i))
                    .unwrap()["value"]
                    .to_string();
                eprintln!(
                    "R2 {tag} panel {i} setting: page {:?}, read_view {want}",
                    attr(p, "data-value")
                );
                assert_eq!(attr(p, "data-value").unwrap(), want, "R2 {tag} panel {i}: the setting");
            }
            _ => continue,
        }
        n += 1;
    }
    n
}

/// ADR 0823 R1, R2, R3, R4, R7 as amendments 1 and 2 correct them: the pages of the registered set.
#[test]
fn r1_r2_r3_r4_r7_a2_the_pages_of_the_registered_set() {
    let dir = f980("r1");
    let mut c = Mcp::start(&["--project", &d(&dir)]);
    // the settings of X2 as read_view gives them (R2), from a Studio's reading server
    let mut studio = Mcp::start(&["--project", &d(&dir), "--client", "studio", "--read-only", "--canvas"]);
    studio.request("tools/list", json!({}));
    let mut lines = 0;
    for (tag, view, n) in registered() {
        let before = journal(&dir);
        let t0 = std::time::Instant::now();
        let r = ok(&c.call("export_view", json!({"view": view, "reason": format!("F-980 {tag}")})));
        let ms = t0.elapsed().as_millis();
        let path = dir.join(r["path"].as_str().unwrap());
        let bytes = std::fs::read(&path).unwrap();
        eprintln!(
            "R1 {tag}: {} in {ms} ms, {} bytes, {} panels",
            r["path"],
            bytes.len(),
            r["panels"]
        );
        // R1
        assert_eq!(r["exported"], true);
        assert!(r["path"].as_str().unwrap().starts_with("exports/"));
        assert_eq!(r["bytes"], json!(bytes.len()));
        assert_eq!(r["hash"], json!(rcfd_project::records::hash_bytes(&bytes)));
        assert_eq!(r["panels"], json!(n));
        let page = String::from_utf8(bytes).unwrap();
        assert!(page.starts_with("<!doctype html>"));
        assert_eq!(panels(&page).len(), n, "R1 {tag}: the panels");
        // R3
        let refs = page_references(&page);
        eprintln!("R3 {tag}: references {refs:?}");
        assert!(refs.is_empty(), "R3 {tag}: {refs:?}");
        assert!(page.contains(&format!(
            "<meta http-equiv=\"Content-Security-Policy\" content=\"{EXPORT_CSP}\">"
        )));
        for (name, css) in &EXPORT_STYLES[..3] {
            assert!(page.contains(css), "R3 {tag}: {name} is not in the page");
        }
        // R4
        assert!(page.contains(&format!("data-engine=\"{}\"", env!("CARGO_PKG_VERSION"))));
        assert_eq!(attr(&page, "data-export-view").unwrap(), r["id"].as_str().unwrap());
        let mut files = 0;
        for p in panels(&page) {
            let sources = attr(&p, "data-sources").unwrap();
            let hashes = attr(&p, "data-hashes").unwrap();
            assert!(!sources.is_empty());
            for (s, h) in sources.split(' ').zip(hashes.split(' ')) {
                assert!(dir.join(s).is_file(), "R4 {tag}: {s} is not a file");
                let got = ok(&c.call("hash_project_file", json!({"path": s})));
                assert_eq!(got["hash"].as_str().unwrap(), h, "R4 {tag}: the hash of {s}");
                files += 1;
            }
        }
        eprintln!("R4 {tag}: {files} files named with their hashes");
        // R2 (X1 to X4)
        if tag != "X5" {
            let settings = if tag == "X2" {
                ok(&c.call("show_view", json!({"view": view})));
                ok(&studio.call("read_view", json!({})))["settings"].clone()
            } else {
                Value::Null
            };
            let checked = check_values_a2(&mut c, tag, &page, &settings);
            eprintln!("R2 {tag}: {checked} panels with values checked");
        }
        // R7
        let after = journal(&dir);
        let new: Vec<Value> = String::from_utf8_lossy(&after[before.len()..])
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .filter(|l: &Value| l["kind"] == VIEW_EXPORTED)
            .collect();
        assert_eq!(new.len(), 1, "R7 {tag}");
        assert_eq!(new[0]["path"], r["path"]);
        assert_eq!(new[0]["hash"], r["hash"]);
        assert_eq!(new[0]["bytes"], r["bytes"]);
        lines += 1;
    }
    eprintln!("R7: {lines} view_exported lines");
}

/// ADR 0823 R5: `export_view {}` exports the view shown last.
#[test]
fn r5_export_view_without_a_view_exports_the_shown_one() {
    let dir = f980("r5");
    let mut studio = Mcp::start(&["--project", &d(&dir), "--client", "studio", "--read-only", "--canvas"]);
    studio.request("tools/list", json!({}));
    let mut c = Mcp::start(&["--project", &d(&dir)]);
    ok(&c.call("show_view", json!({"view": {"version": 1, "run": RUN}})));
    let read = ok(&studio.call("read_view", json!({})));
    let r = ok(&c.call("export_view", json!({})));
    let page = std::fs::read_to_string(dir.join(r["path"].as_str().unwrap())).unwrap();
    let want: Vec<(String, String)> = read["view"]["panels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["kind"].as_str().unwrap().to_string(),
                p["title"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    let got: Vec<(String, String)> = panels(&page)
        .iter()
        .map(|p| {
            let t = p
                .split("<h3 class=\"cv-panel-title\">")
                .nth(1)
                .map(|t| unescape(t.split('<').next().unwrap()))
                .unwrap_or_default();
            (attr(p, "data-kind").unwrap(), t)
        })
        .collect();
    eprintln!(
        "R5: read_view id {}, page {:?}; panels {got:?}",
        read["id"],
        attr(&page, "data-export-view")
    );
    assert_eq!(attr(&page, "data-export-view").unwrap(), read["id"].as_str().unwrap());
    assert_eq!(r["id"], read["id"]);
    assert_eq!(got, want);
}

/// ADR 0823 R6: four refusals, nothing written.
#[test]
fn r6_refusals_write_nothing() {
    let dir = f980("r6");
    let mut c = Mcp::start(&["--project", &d(&dir)]);
    let mut chart = x1();
    chart["panels"][0]["kind"] = json!("chart");
    let cases = [
        (json!({"view": chart}), 1003, "panels[0].kind"),
        (json!({"view": x1(), "out": "../x.html"}), 1003, "out"),
        (json!({"view": x1(), "out": "exports/x.txt"}), 1003, "out"),
        (json!({}), 1202, ""),
    ];
    for (args, code, key) in cases {
        let before = journal(&dir);
        let e = error_of(&c.call("export_view", args.clone()));
        eprintln!("R6 {args}: {e}");
        assert_eq!(e["code"], code);
        if !key.is_empty() {
            assert_eq!(e["data"]["errors"][0]["key"], key);
        }
        assert!(!dir.join("exports").exists(), "R6: exports/ was made");
        assert!(!dir.join("x.html").exists() && !dir.parent().unwrap().join("x.html").exists());
        assert_eq!(journal(&dir), before, "R6: the journal changed");
    }
}
