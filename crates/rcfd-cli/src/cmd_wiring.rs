//! `rcfd partition`, `rcfd sample` and `rcfd inflate` (ADR 0021): the command line surface of the partitioning,
//! sampling and boundary-layer inflation library features.

use super::{flag, load_case, opt, parse_opt, positionals, read_mesh, usage_error, BAD_MESH, FAIL, INVALID, OK};
use rcfd_api::case::Case;
use rcfd_api::decomp;
use rcfd_api::inflate::{inflate, InflateError, InflateRequest};
use rcfd_api::postproc::{self, SampleKind, SampleSpec};
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::Vec3;
use rcfd_io::vtk::{write_vtu, VtkField, VtkValues};
use serde_json::{json, Value};
use std::path::Path;

fn print_diags(label: &str, diags: &[rcfd_api::case::Diagnostic]) {
    for d in diags {
        eprintln!("{label}: {d}");
    }
}

fn parse_vec3(s: &str) -> Result<Vec3, String> {
    let v: Vec<f64> = s
        .split(',')
        .map(|x| x.trim().parse::<f64>().map_err(|_| format!("'{s}' is not x,y,z")))
        .collect::<Result<_, _>>()?;
    match v[..] {
        [a, b, c] => Ok([a, b, c]),
        _ => Err(format!("'{s}' is not x,y,z")),
    }
}

const PARTITION_VALUES: [&str; 13] = [
    "--method",
    "--parts",
    "--rule",
    "--sector-kind",
    "--tolerance",
    "--max-passes",
    "--origin",
    "--axis",
    "--reference",
    "--weights-box",
    "--out",
    "--block",
    "--patch",
];

/// `rcfd partition <case|mesh> [options]`.
pub fn cmd_partition(args: &[String]) -> u8 {
    let pos = positionals(args, &PARTITION_VALUES);
    let [path] = pos[..] else {
        return usage_error("partition takes exactly one case or mesh");
    };
    let is_case = path.ends_with(".toml") || path.ends_with(".json");
    let case = if is_case {
        match load_case(path) {
            Ok(c) => c,
            Err(code) => return code,
        }
    } else {
        match Case::parse_toml("") {
            Ok(c) => c,
            Err(_) => return FAIL,
        }
    };
    let mut over: Vec<(String, Value)> = Vec::new();
    let mut set = |k: &str, v: Value| over.push((k.to_string(), v));
    for (flag_name, key) in [
        ("--method", "method"),
        ("--rule", "rule"),
        ("--sector-kind", "sector_kind"),
    ] {
        if let Some(v) = opt(args, flag_name) {
            set(key, json!(v));
        }
    }
    match parse_opt::<i64>(args, "--parts") {
        Ok(Some(n)) => set("parts", json!(n)),
        Ok(None) => {}
        Err(m) => return usage_error(&m),
    }
    match parse_opt::<f64>(args, "--tolerance") {
        Ok(Some(t)) => set("tolerance", json!(t)),
        Ok(None) => {}
        Err(m) => return usage_error(&m),
    }
    match parse_opt::<i64>(args, "--max-passes") {
        Ok(Some(t)) => set("max_passes", json!(t)),
        Ok(None) => {}
        Err(m) => return usage_error(&m),
    }
    for (flag_name, key) in [("--origin", "origin"), ("--axis", "axis"), ("--reference", "reference")] {
        if let Some(v) = opt(args, flag_name) {
            match parse_vec3(v) {
                Ok(x) => set(key, json!(x)),
                Err(m) => return usage_error(&format!("{flag_name}: {m}")),
            }
        }
    }
    if let Some(w) = opt(args, "--weights-box") {
        let parts: Vec<&str> = w.split(':').collect();
        let [lo, hi, inside, outside] = parts[..] else {
            return usage_error("--weights-box needs x0,y0,z0:x1,y1,z1:inside:outside");
        };
        let (Ok(lo), Ok(hi), Ok(inside), Ok(outside)) = (
            parse_vec3(lo),
            parse_vec3(hi),
            inside.parse::<f64>(),
            outside.parse::<f64>(),
        ) else {
            return usage_error("--weights-box needs x0,y0,z0:x1,y1,z1:inside:outside");
        };
        set("weights.mode", json!("box"));
        set("weights.box_min", json!(lo));
        set("weights.box_max", json!(hi));
        set("weights.inside", json!(inside));
        set("weights.outside", json!(outside));
    }
    if flag(args, "--smooth") {
        set("smooth", json!(true));
    }
    if flag(args, "--reorder") {
        set("reorder", json!(true));
    }
    let case = match decomp::with_overrides(&case, &over) {
        Ok(c) => c,
        Err(d) => {
            print_diags(path, &d);
            return INVALID;
        }
    };
    let mesh = match read_mesh(path, args) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            return FAIL;
        }
    };
    let report = match decomp::run(&case, &mesh) {
        Ok(r) => r,
        Err(d) => {
            print_diags(path, &d);
            return INVALID;
        }
    };
    if let Some(out) = opt(args, "--out") {
        let field = decomp::part_field(&report);
        let r = write_vtu(
            out,
            &mesh,
            &[VtkField {
                name: "partition",
                values: VtkValues::Scalar(&field),
            }],
        );
        if let Err(e) = r {
            eprintln!("error: {out}: {e}");
            return FAIL;
        }
    }
    if flag(args, "--json") {
        let doc = decomp::report_json(&report, flag(args, "--parts-vector"));
        println!("{}", serde_json::to_string_pretty(&doc).unwrap_or_default());
    } else {
        println!(
            "method {}  parts {}  cells {}",
            report.method, report.n_parts, report.n_cells
        );
        for (i, ((c, w), g)) in report
            .part_cells
            .iter()
            .zip(&report.part_weights)
            .zip(&report.ghost_cells)
            .enumerate()
        {
            println!("  part {i}: cells {c}  weight {w}  ghost cells {g}");
        }
        println!("imbalance {:.4}  edge cut {}", report.imbalance, report.edge_cut);
        if let Some(m) = report.smoothed_moves {
            println!("smoothing moved {m} cells");
        }
        if let Some(b) = report.bandwidth {
            println!("bandwidth natural {}  reverse Cuthill-McKee {}", b.natural, b.rcm);
        }
        if let Some(out) = opt(args, "--out") {
            println!("wrote {out}");
        }
    }
    OK
}

const SAMPLE_VALUES: [&str; 7] = [
    "--line",
    "--plane",
    "--points",
    "--fields",
    "--max-iterations",
    "--out",
    "--threads",
];

fn ad_hoc_spec(args: &[String]) -> Result<Option<SampleSpec>, String> {
    let fields: Vec<String> = opt(args, "--fields")
        .map(|f| f.split(',').filter(|s| !s.is_empty()).map(String::from).collect())
        .unwrap_or_default();
    let gradient = !flag(args, "--no-gradient");
    if let Some(l) = opt(args, "--line") {
        let ends: Vec<&str> = l.split(':').collect();
        let [a, b] = ends[..] else {
            return Err("--line needs x0,y0,z0:x1,y1,z1".into());
        };
        let points = parse_opt::<usize>(args, "--points")?.unwrap_or(50);
        return Ok(Some(SampleSpec {
            kind: SampleKind::Line {
                p0: parse_vec3(a)?,
                p1: parse_vec3(b)?,
                points,
            },
            fields,
            gradient,
        }));
    }
    if let Some(p) = opt(args, "--plane") {
        let f: Vec<&str> = p.split(':').collect();
        let [o, e1, e2, x1, x2, n1, n2] = f[..] else {
            return Err("--plane needs ox,oy,oz:e1:e2:extent1:extent2:n1:n2".into());
        };
        let num = |s: &str| s.parse::<f64>().map_err(|_| format!("'{s}' is not a number"));
        let cnt = |s: &str| s.parse::<usize>().map_err(|_| format!("'{s}' is not a count"));
        return Ok(Some(SampleSpec {
            kind: SampleKind::Plane {
                origin: parse_vec3(o)?,
                e1: parse_vec3(e1)?,
                e2: parse_vec3(e2)?,
                extent1: num(x1)?,
                extent2: num(x2)?,
                n1: cnt(n1)?,
                n2: cnt(n2)?,
            },
            fields,
            gradient,
        }));
    }
    Ok(None)
}

/// `rcfd sample <case> [options]`: run the case (or sample the initial field with `--max-iterations 0`) and write CSV.
pub fn cmd_sample(args: &[String]) -> u8 {
    let pos = positionals(args, &SAMPLE_VALUES);
    let [path] = pos[..] else {
        return usage_error("sample takes exactly one case file");
    };
    let spec = match ad_hoc_spec(args) {
        Ok(s) => s,
        Err(m) => return usage_error(&m),
    };
    let max_iter = match parse_opt::<usize>(args, "--max-iterations") {
        Ok(v) => v,
        Err(m) => return usage_error(&m),
    };
    let case = match load_case(path) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let diags = case.semantic_errors();
    if !diags.is_empty() {
        print_diags(path, &diags);
        return INVALID;
    }
    let mut sim = match Simulation::new(&case) {
        Ok(s) => s,
        Err(d) => {
            print_diags(path, &d);
            return INVALID;
        }
    };
    if spec.is_none() && case.tree.names("postprocessing.samples").unwrap_or_default().is_empty() {
        return usage_error("give --line or --plane, or define postprocessing.samples in the case");
    }
    let n = sim.run(max_iter.unwrap_or(usize::MAX));
    eprintln!("ran {n} iterations/steps");
    match spec {
        Some(spec) => match postproc::sample_csv(&sim, &spec) {
            Ok(csv) => match opt(args, "--out") {
                Some(out) => {
                    if let Err(e) = std::fs::write(out, csv) {
                        eprintln!("error: {out}: {e}");
                        return FAIL;
                    }
                    println!("wrote {out}");
                    OK
                }
                None => {
                    print!("{csv}");
                    OK
                }
            },
            Err(e) => {
                eprintln!("error: {e}");
                FAIL
            }
        },
        None => {
            let dir = match opt(args, "--out") {
                Some(d) => Path::new(d).to_path_buf(),
                None => case.base_dir.join(
                    case.get("output.directory")
                        .ok()
                        .and_then(|v| v.as_str().map(String::from))
                        .unwrap_or_else(|| "results".into()),
                ),
            };
            match postproc::write_samples(&sim, &dir) {
                Ok(files) => {
                    for f in files {
                        println!("wrote {}", f.display());
                    }
                    OK
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    FAIL
                }
            }
        }
    }
}

const INFLATE_VALUES: [&str; 12] = [
    "--patch",
    "--layers",
    "--growth",
    "--first",
    "--uniform",
    "--aspect",
    "--last-ratio",
    "--y-plus",
    "--u-tau",
    "--nu",
    "--out",
    "--block",
];

/// `rcfd inflate <case|mesh> --patch p --layers n <sizing> [--json] [--out f.vtu]`.
pub fn cmd_inflate(args: &[String]) -> u8 {
    let pos = positionals(args, &INFLATE_VALUES);
    let [path] = pos[..] else {
        return usage_error("inflate takes exactly one case or mesh");
    };
    let Some(patch) = opt(args, "--patch") else {
        return usage_error("inflate needs --patch");
    };
    let f = |name: &str| parse_opt::<f64>(args, name);
    let parsed = (
        parse_opt::<usize>(args, "--layers"),
        f("--growth"),
        f("--first"),
        f("--uniform"),
        f("--aspect"),
        f("--last-ratio"),
        f("--y-plus"),
        f("--u-tau"),
        f("--nu"),
    );
    let (Ok(layers), Ok(growth), Ok(first), Ok(total), Ok(aspect), Ok(last_ratio), Ok(yp), Ok(ut), Ok(nu)) = parsed
    else {
        let m = [
            parsed.0.err(),
            parsed.1.err(),
            parsed.2.err(),
            parsed.3.err(),
            parsed.4.err(),
            parsed.5.err(),
            parsed.6.err(),
            parsed.7.err(),
            parsed.8.err(),
        ]
        .into_iter()
        .flatten()
        .next()
        .unwrap_or_default();
        return usage_error(&m);
    };
    let y_plus = match (yp, ut, nu) {
        (None, None, None) => None,
        (Some(a), Some(b), Some(c)) => Some((a, b, c)),
        _ => return usage_error("--y-plus needs --u-tau and --nu"),
    };
    let req = InflateRequest {
        patch: patch.to_string(),
        layers: layers.unwrap_or(0),
        growth,
        first,
        total,
        aspect,
        last_ratio,
        y_plus,
    };
    let mesh = match read_mesh(path, args) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            return FAIL;
        }
    };
    let (new, doc) = match inflate(&mesh, &req) {
        Ok(r) => r,
        Err(InflateError::Usage(m)) => return usage_error(&m),
        Err(InflateError::Library(m)) => {
            eprintln!("error: {m}");
            return FAIL;
        }
    };
    if let Some(out) = opt(args, "--out") {
        if let Err(e) = write_vtu(out, &new, &[]) {
            eprintln!("error: {out}: {e}");
            return FAIL;
        }
    }
    if flag(args, "--json") {
        println!("{}", serde_json::to_string_pretty(&doc).unwrap_or_default());
    } else {
        let r = &doc["report"];
        println!(
            "patch {}  cells {} -> {}  layers {}",
            patch, doc["cells_before"], doc["cells_after"], r["layers"]
        );
        println!(
            "first height {:.6e} to {:.6e}  last height {:.6e} to {:.6e}  thickness {:.6e} to {:.6e}",
            r["first_height_min"].as_f64().unwrap_or(0.0),
            r["first_height_max"].as_f64().unwrap_or(0.0),
            r["last_height_min"].as_f64().unwrap_or(0.0),
            r["last_height_max"].as_f64().unwrap_or(0.0),
            r["thickness_min"].as_f64().unwrap_or(0.0),
            r["thickness_max"].as_f64().unwrap_or(0.0)
        );
        println!(
            "growth {:.4} to {:.4}  max aspect ratio {:.2}",
            r["growth_min"].as_f64().unwrap_or(0.0),
            r["growth_max"].as_f64().unwrap_or(0.0),
            r["quality"]["max_aspect_ratio"].as_f64().unwrap_or(0.0)
        );
    }
    if doc["report"]["quality"]["errors"]
        .as_array()
        .is_some_and(|e| !e.is_empty())
    {
        return BAD_MESH;
    }
    OK
}
