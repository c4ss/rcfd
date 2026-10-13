//! rcfd command line (F-081): validate / schema / run / check-mesh / convert /
//! post / serve / cases.
//!
//! Exit codes: 0 success, 1 runtime or I/O failure, 2 usage error, 3 invalid
//! case file, 4 mesh failed its quality check, 5 run finished without meeting
//! its convergence targets, 6 run diverged.

use rcfd_api::case::Case;
use rcfd_api::schema::case_schema;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_api::sim::Simulation;
use rcfd_io::cases::Registry;
use rcfd_mesh::quality::check_mesh;
use rcfd_mesh::Mesh;
use rcfd_post::monitors::{html_report, MonitorStore};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const OK: u8 = 0;
const FAIL: u8 = 1;
const USAGE_ERR: u8 = 2;
const INVALID: u8 = 3;
const BAD_MESH: u8 = 4;
const NOT_CONVERGED: u8 = 5;
const DIVERGED: u8 = 6;

const USAGE: &str = "rcfd - clean-room open source CFD

USAGE:
    rcfd <command> [options]

COMMANDS:
    validate <case>            check a case file (.toml or .json); prints diagnostics with line numbers
    schema [--out <file>]      print (or write) the JSON schema of the case file
    run <case> [options]       run a case
        --out <dir>              output directory (default: output.directory of the case)
        --max-iterations <n>     override solver.max_iterations
        --quiet                  no per-iteration progress
    check-mesh <mesh|case>     quality report for a .msh file, an OpenFOAM polyMesh directory or a case
        --json                   machine-readable report
    convert <in> <out>         convert a mesh: in = .msh or OpenFOAM dir; out = .vtu or an OpenFOAM dir
    post <results-dir> [--format html|csv|json] [--out <file>]
                               re-create reports from <results-dir>/monitors.json written by `run`
    serve [--addr <host:port>] [--root <dir>] [--stdio]
                               start the solver service (JSON-RPC 2.0, newline-delimited)
    call [--addr <host:port>] <method> [<json-params>]
                               call one method of a running service; prints the JSON result
                               (exit 1 and the JSON error on stderr if the call fails)
    cases [list] [--dir <cases-dir>]
                               list benchmark/validation cases

OPTIONS:
    -h, --help       print this help (also valid after a command)
    -V, --version    print the version

EXIT CODES:
    0 success   1 runtime/I-O failure   2 usage error   3 invalid case
    4 mesh quality check failed   5 not converged   6 diverged
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(dispatch(&args))
}

fn usage_error(msg: &str) -> u8 {
    eprintln!("error: {msg}\n\nrun `rcfd --help` for usage");
    USAGE_ERR
}

fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

/// Positional arguments: everything that is not an option or an option value.
fn positionals<'a>(args: &'a [String], value_opts: &[&str]) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
        } else if value_opts.contains(&a.as_str()) {
            skip = true;
        } else if !a.starts_with("--") {
            out.push(a.as_str());
        }
    }
    out
}

fn dispatch(args: &[String]) -> u8 {
    let Some(cmd) = args.first().map(String::as_str) else {
        print!("{USAGE}");
        return OK;
    };
    let rest = &args[1..];
    if matches!(cmd, "-h" | "--help" | "help") || (flag(rest, "--help") || flag(rest, "-h")) {
        print!("{USAGE}");
        return OK;
    }
    match cmd {
        "--version" | "-V" => {
            println!("rcfd {}", env!("CARGO_PKG_VERSION"));
            OK
        }
        "validate" => cmd_validate(rest),
        "schema" => cmd_schema(rest),
        "run" => cmd_run(rest),
        "check-mesh" => cmd_check_mesh(rest),
        "convert" => cmd_convert(rest),
        "post" => cmd_post(rest),
        "serve" => cmd_serve(rest),
        "call" => cmd_call(rest),
        "cases" => cmd_cases(rest),
        other => usage_error(&format!("unknown command '{other}'")),
    }
}

fn load_case(path: &str) -> Result<Case, u8> {
    match Case::load(path) {
        Ok(c) => Ok(c),
        Err(diags) => {
            for d in diags {
                eprintln!("{path}: {d}");
            }
            Err(INVALID)
        }
    }
}

fn cmd_validate(args: &[String]) -> u8 {
    let pos = positionals(args, &[]);
    let [path] = pos[..] else {
        return usage_error("validate takes exactly one case file");
    };
    let case = match load_case(path) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let mut diags = case.semantic_errors();
    if diags.is_empty() {
        if let Err(d) = Simulation::new(&case) {
            diags = d;
        }
    }
    if diags.is_empty() {
        println!("{path}: ok");
        OK
    } else {
        // diagnostics raised while building the simulation carry no line yet: locate them in the TOML text
        let text = path
            .ends_with(".toml")
            .then(|| std::fs::read_to_string(path).ok())
            .flatten();
        for mut d in diags {
            if d.line.is_none() {
                if let Some(t) = &text {
                    d.line = rcfd_api::case::locate_line(t, &d.path);
                }
            }
            eprintln!("{path}: {d}");
        }
        INVALID
    }
}

fn cmd_schema(args: &[String]) -> u8 {
    let text = serde_json::to_string_pretty(&case_schema().to_json_schema()).unwrap_or_default();
    match opt(args, "--out") {
        Some(p) => match std::fs::write(p, text + "\n") {
            Ok(()) => {
                println!("wrote {p}");
                OK
            }
            Err(e) => {
                eprintln!("error: {p}: {e}");
                FAIL
            }
        },
        None => {
            println!("{text}");
            OK
        }
    }
}

fn cmd_run(args: &[String]) -> u8 {
    let pos = positionals(args, &["--out", "--max-iterations"]);
    let [path] = pos[..] else {
        return usage_error("run takes exactly one case file");
    };
    let mut case = match load_case(path) {
        Ok(c) => c,
        Err(code) => return code,
    };
    if let Some(n) = opt(args, "--max-iterations") {
        let Ok(n) = n.parse::<i64>() else {
            return usage_error("--max-iterations needs an integer");
        };
        if let Err(e) = case.set("solver.max_iterations", n.into()) {
            eprintln!("error: {e}");
            return INVALID;
        }
    }
    let mut sim = match Simulation::new(&case) {
        Ok(s) => s,
        Err(diags) => {
            for d in diags {
                eprintln!("{path}: {d}");
            }
            return INVALID;
        }
    };
    let quiet = flag(args, "--quiet");
    let out_dir = opt(args, "--out")
        .map(PathBuf::from)
        .or_else(|| {
            case.get("output.directory")
                .ok()
                .and_then(|v| v.as_str().map(PathBuf::from))
        })
        .unwrap_or_else(|| PathBuf::from("results"));
    println!(
        "{} cells, algorithm {}",
        sim.geom.n_cells(),
        case.get("solver.algorithm")
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default()
    );
    let mut diverged = false;
    while !sim.is_done() {
        let s = sim.step();
        if !quiet && (s.index <= 5 || s.index % 20 == 0 || s.done) {
            println!(
                "{:>6}  t={:<10.4e} res_u {:.3e}  res_p {:.3e}  continuity {:.2e}",
                s.index, s.time, s.res_u, s.res_p, s.continuity
            );
        }
        if !s.res_u.is_finite() || !s.res_p.is_finite() {
            diverged = true;
            break;
        }
    }
    if let Err(e) = write_results(&sim, &out_dir) {
        eprintln!("error: writing results to {}: {e}", out_dir.display());
        return FAIL;
    }
    println!("results written to {}", out_dir.display());
    if diverged {
        eprintln!("error: residuals are not finite; the run diverged");
        return DIVERGED;
    }
    if sim.is_converged() || !sim.is_steady() {
        println!("finished after {} iterations/steps", sim.iteration());
        if let Some(r) = sim.stop_reason() {
            println!("stopped: {r}");
        }
        OK
    } else {
        eprintln!(
            "warning: stopped after {} iterations without meeting the tolerances",
            sim.iteration()
        );
        NOT_CONVERGED
    }
}

fn write_results(sim: &Simulation, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    sim.export_vtu(dir, "solution")?;
    std::fs::write(dir.join("monitors.json"), sim.monitors.to_json().to_string())?;
    std::fs::write(dir.join("report.html"), sim.html_report())?;
    std::fs::write(dir.join("monitors.csv"), sim.monitors.to_csv())?;
    Ok(())
}

fn read_mesh(path: &str) -> Result<Mesh, String> {
    let p = Path::new(path);
    if p.is_dir() {
        let dir = if p.join("polyMesh").is_dir() {
            p.join("polyMesh")
        } else {
            p.to_path_buf()
        };
        rcfd_io::openfoam::read_dir(dir).map_err(|e| e.to_string())
    } else if p.extension().is_some_and(|e| e == "msh") {
        rcfd_io::gmsh::read_file(p).map_err(|e| e.to_string())
    } else if p.extension().is_some_and(|e| e == "toml" || e == "json") {
        let case = Case::load(p).map_err(|d| d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))?;
        rcfd_api::sim::build_mesh(&case).map_err(|d| d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))
    } else {
        Err(format!(
            "cannot tell the mesh format of '{path}' (expected .msh, .toml/.json case, or an OpenFOAM directory)"
        ))
    }
}

fn cmd_check_mesh(args: &[String]) -> u8 {
    let pos = positionals(args, &[]);
    let [path] = pos[..] else {
        return usage_error("check-mesh takes exactly one mesh or case");
    };
    let mesh = match read_mesh(path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            return FAIL;
        }
    };
    let q = check_mesh(&mesh);
    if flag(args, "--json") {
        println!("{}", serde_json::to_string_pretty(&q).unwrap_or_default());
    } else {
        println!(
            "cells {}  faces {} ({} internal)  patches {}",
            q.n_cells, q.n_faces, q.n_internal_faces, q.n_patches
        );
        println!(
            "volume {:.6e} (min {:.3e}, max {:.3e})",
            q.total_volume, q.min_volume, q.max_volume
        );
        println!(
            "non-orthogonality max {:.1} deg, mean {:.1} deg",
            q.max_non_orthogonality, q.mean_non_orthogonality
        );
        println!(
            "skewness max {:.3}  aspect ratio max {:.2}  volume ratio max {:.2}",
            q.max_skewness, q.max_aspect_ratio, q.max_volume_ratio
        );
        for w in &q.warnings {
            println!("warning: {w}");
        }
        for e in &q.errors {
            println!("error: {e}");
        }
        println!("{}", if q.is_ok() { "mesh ok" } else { "mesh FAILED" });
    }
    if q.is_ok() {
        OK
    } else {
        BAD_MESH
    }
}

fn cmd_convert(args: &[String]) -> u8 {
    let pos = positionals(args, &[]);
    let [input, output] = pos[..] else {
        return usage_error("convert takes <in> <out>");
    };
    let mesh = match read_mesh(input) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            return FAIL;
        }
    };
    let out = Path::new(output);
    let res = if out.extension().is_some_and(|e| e == "vtu") {
        rcfd_io::vtk::write_vtu(out, &mesh, &[]).map_err(|e| e.to_string())
    } else if out.extension().is_none() {
        rcfd_io::openfoam::write_dir(&mesh, out.join("polyMesh")).map_err(|e| e.to_string())
    } else {
        Err(format!(
            "unsupported output '{output}' (use .vtu or an OpenFOAM case directory without extension)"
        ))
    };
    match res {
        Ok(()) => {
            println!("converted {} cells: {input} -> {output}", mesh.n_cells());
            OK
        }
        Err(e) => {
            eprintln!("error: {e}");
            FAIL
        }
    }
}

fn cmd_post(args: &[String]) -> u8 {
    let pos = positionals(args, &["--format", "--out"]);
    let [dir] = pos[..] else {
        return usage_error("post takes exactly one results directory");
    };
    let file = Path::new(dir).join("monitors.json");
    let store = match std::fs::read_to_string(&file)
        .map_err(|e| e.to_string())
        .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
        .and_then(|v| MonitorStore::from_json(&v).map_err(|e| e.to_string()))
    {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {}: {e}", file.display());
            return FAIL;
        }
    };
    let (default_name, body) = match opt(args, "--format").unwrap_or("html") {
        "html" => ("report.html", html_report(dir, &[], &store, &[])),
        "csv" => ("monitors.csv", store.to_csv()),
        "json" => ("monitors.json", store.to_json().to_string()),
        other => return usage_error(&format!("unknown format '{other}' (html, csv, json)")),
    };
    match opt(args, "--out") {
        Some(p) => match std::fs::write(p, body) {
            Ok(()) => {
                println!("wrote {p}");
                OK
            }
            Err(e) => {
                eprintln!("error: {p}: {e}");
                FAIL
            }
        },
        None => {
            let _ = default_name;
            print!("{body}");
            OK
        }
    }
}

fn cmd_serve(args: &[String]) -> u8 {
    let cfg = ServiceConfig {
        root: opt(args, "--root").map(PathBuf::from),
        ..Default::default()
    };
    let svc = Service::new(cfg);
    if flag(args, "--stdio") {
        let r = svc.serve_stream(std::io::stdin(), std::io::stdout());
        svc.shutdown();
        return if r.is_ok() { OK } else { FAIL };
    }
    let addr = opt(args, "--addr").unwrap_or("127.0.0.1:7878");
    match svc.serve_tcp(addr) {
        Ok(server) => {
            println!("rcfd service listening on {}", server.addr);
            server.wait();
            OK
        }
        Err(e) => {
            eprintln!("error: cannot listen on {addr}: {e}");
            FAIL
        }
    }
}

fn cmd_cases(args: &[String]) -> u8 {
    let dir = opt(args, "--dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cases"));
    match Registry::load(&dir) {
        Ok(reg) => {
            for c in reg.list() {
                println!("{:<16} {}  [{}]", c.id, c.title, c.roadmap.join(", "));
            }
            OK
        }
        Err(e) => {
            eprintln!("error: {e}");
            USAGE_ERR
        }
    }
}

fn cmd_call(args: &[String]) -> u8 {
    let pos = positionals(args, &["--addr"]);
    let (method, params) = match pos[..] {
        [m] => (m, "{}"),
        [m, p] => (m, p),
        _ => return usage_error("call takes <method> [<json-params>]"),
    };
    let params: serde_json::Value = match serde_json::from_str(params) {
        Ok(v) => v,
        Err(e) => return usage_error(&format!("params are not valid JSON: {e}")),
    };
    let addr = opt(args, "--addr").unwrap_or("127.0.0.1:7878");
    let Ok(sock) = addr.parse() else {
        return usage_error(&format!("bad address '{addr}'"));
    };
    let mut client = match rcfd_api::service::Client::connect(sock) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: cannot connect to {addr}: {e}");
            return FAIL;
        }
    };
    match client.call(method, params) {
        Ok(v) => {
            println!("{v}");
            OK
        }
        Err(e) => {
            eprintln!(
                "{}",
                serde_json::json!({"code": e.code, "message": e.message, "data": e.data})
            );
            FAIL
        }
    }
}
