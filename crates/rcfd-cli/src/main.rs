//! rcfd command line (F-081): validate / schema / run / check-mesh / convert /
//! post / serve / cases.
//!
//! Exit codes: 0 success, 1 runtime or I/O failure, 2 usage error, 3 invalid
//! case file, 4 mesh failed its quality check, 5 run finished without meeting
//! its convergence targets, 6 run diverged, 7 interrupted (stop file), 8 wall-clock limit
//! reached (see docs/run-control.md).

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::schema::case_schema;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_api::sim::Simulation;
use rcfd_io::cases::Registry;
use rcfd_mesh::quality::check_mesh;
use rcfd_mesh::Mesh;
use rcfd_post::monitors::{html_report, MonitorStore};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;

const OK: u8 = 0;
const FAIL: u8 = 1;
const USAGE_ERR: u8 = 2;
const INVALID: u8 = 3;
const BAD_MESH: u8 = 4;

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
        --check-only             validate the case, build the simulation and check the mesh; do not run
        --status-json            write the machine-readable status stream (JSON lines) to stdout;
                                 human text goes to stderr (format: docs/run-control.md)
        --status-file <file>     also write the status stream to a file
        --status-every <n>       iteration events every n steps (default 10; first 5 and last always)
        --continue-from <file>   restart from a checkpoint (steady SIMPLE/SIMPLEC without turbulence)
        --checkpoint <file>      checkpoint written on interrupt, time limit and at the end
                                 (default: <out>/restart.rcfd)
        --checkpoint-every <n>   also write the checkpoint every n steps
        --stop-file <file>       stop cleanly (exit 7) when this file appears
        --max-elapsed <seconds>  stop cleanly (exit 8) after this wall-clock time
        --threads <n>            worker threads (sets RAYON_NUM_THREADS; results do not depend on it)
        --precision double       only double precision is supported
        --journal <file>         run the case recorded in a service journal (journal.get) instead of a file
    workflow templates                list workflow templates and task kinds
    workflow show [--template <name> | --workflow <file>]
                               print a workflow document (JSON)
    workflow run <case> [--template <name> | --workflow <file>] [--task <id>] [--no-prerequisites]
                        [--max-steps <n>] [--json]
                               run the tasks of a workflow on a case (exit code of the failing task)
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
    7 interrupted (stop file)   8 wall-clock limit reached
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
        "workflow" => cmd_workflow(rest),
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

const RUN_VALUE_OPTS: &[&str] = &[
    "--out",
    "--max-iterations",
    "--status-file",
    "--status-every",
    "--continue-from",
    "--checkpoint",
    "--checkpoint-every",
    "--stop-file",
    "--max-elapsed",
    "--threads",
    "--precision",
    "--hostfile",
    "--journal",
];

fn parse_opt<T: std::str::FromStr>(args: &[String], name: &str) -> Result<Option<T>, String> {
    match opt(args, name) {
        None => Ok(None),
        Some(v) => v
            .parse::<T>()
            .map(Some)
            .map_err(|_| format!("{name} needs a number, got '{v}'")),
    }
}

fn cmd_run(args: &[String]) -> u8 {
    let pos = positionals(args, RUN_VALUE_OPTS);
    let journal = opt(args, "--journal");
    if (journal.is_none() && pos.len() != 1) || (journal.is_some() && !pos.is_empty()) {
        return usage_error("run takes exactly one case file (or --journal <file> instead of a case)");
    }
    if opt(args, "--hostfile").is_some() {
        return usage_error("--hostfile: distributed runs are not supported (single-process shared-memory solver)");
    }
    if let Some(p) = opt(args, "--precision") {
        if p != "double" {
            return usage_error(&format!(
                "--precision {p}: only double precision is supported (use --precision double)"
            ));
        }
    }
    let (threads, max_elapsed, status_every, ckpt_every, max_iter) = match (
        parse_opt::<usize>(args, "--threads"),
        parse_opt::<f64>(args, "--max-elapsed"),
        parse_opt::<usize>(args, "--status-every"),
        parse_opt::<usize>(args, "--checkpoint-every"),
        parse_opt::<i64>(args, "--max-iterations"),
    ) {
        (Ok(a), Ok(b), Ok(c), Ok(d), Ok(e)) => (a, b, c, d, e),
        (Err(m), ..) | (_, Err(m), ..) | (_, _, Err(m), ..) | (_, _, _, Err(m), _) | (_, _, _, _, Err(m)) => {
            return usage_error(&m)
        }
    };
    if threads == Some(0) || max_elapsed.is_some_and(|m| m.is_nan() || m <= 0.0) {
        return usage_error("--threads and --max-elapsed must be positive");
    }
    if let Some(n) = threads {
        // the global worker pool reads this on first use, which is after this point
        std::env::set_var("RAYON_NUM_THREADS", n.to_string());
    }
    let status_json = flag(args, "--status-json");
    let say = |s: &str| {
        if status_json {
            eprintln!("{s}");
        } else {
            println!("{s}");
        }
    };
    let (mut case, label) = if let Some(jp) = journal {
        let text = match std::fs::read_to_string(jp) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("error: cannot read {jp}: {e}");
                return FAIL;
            }
        };
        let doc: serde_json::Value = match serde_json::from_str(&text) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("error: {jp}: {e}");
                return INVALID;
            }
        };
        let base = Path::new(jp)
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        match runctl::case_from_journal(&doc, &base, None) {
            Ok(c) => (c, jp.to_string()),
            Err(diags) => {
                for d in diags {
                    eprintln!("{jp}: {d}");
                }
                return INVALID;
            }
        }
    } else {
        match load_case(pos[0]) {
            Ok(c) => (c, pos[0].to_string()),
            Err(code) => return code,
        }
    };
    if let Some(n) = max_iter {
        if let Err(e) = case.set("solver.max_iterations", n.into()) {
            eprintln!("error: {e}");
            return INVALID;
        }
    }
    if flag(args, "--check-only") {
        let (code, ev) = runctl::check_only(&case);
        if status_json || flag(args, "--json") {
            println!("{ev}");
        }
        if let Some(ds) = ev["diagnostics"].as_array() {
            for d in ds {
                eprintln!(
                    "{label}: {}: {}",
                    d["path"].as_str().unwrap_or(""),
                    d["message"].as_str().unwrap_or("")
                );
            }
        }
        say(&match code {
            runctl::exit::OK => format!("{label}: ok, {} cells", ev["cells"]),
            runctl::exit::BAD_MESH => format!("{label}: mesh failed its quality check"),
            _ => format!("{label}: invalid"),
        });
        return code;
    }
    let sim = match Simulation::new(&case) {
        Ok(s) => s,
        Err(diags) => {
            for d in diags {
                eprintln!("{label}: {d}");
            }
            return INVALID;
        }
    };
    let q = check_mesh(&sim.geom.mesh);
    if !q.is_ok() {
        for e in &q.errors {
            eprintln!("{label}: mesh error: {e}");
        }
        return BAD_MESH;
    }
    let mut sim = sim;
    let quiet = flag(args, "--quiet");
    let out_dir = opt(args, "--out")
        .map(PathBuf::from)
        .or_else(|| {
            case.get("output.directory")
                .ok()
                .and_then(|v| v.as_str().map(PathBuf::from))
        })
        .unwrap_or_else(|| PathBuf::from("results"));
    let cont = opt(args, "--continue-from");
    if let Some(c) = cont {
        if let Err(e) = sim.load_checkpoint(Path::new(c)) {
            eprintln!("error: --continue-from {c}: {e}");
            return FAIL;
        }
    }
    let mut status_file = match opt(args, "--status-file") {
        Some(f) => match std::fs::File::create(f) {
            Ok(h) => Some(h),
            Err(e) => {
                eprintln!("error: cannot create {f}: {e}");
                return FAIL;
            }
        },
        None => None,
    };
    let opts = RunOptions {
        max_steps: None,
        max_elapsed,
        stop_file: opt(args, "--stop-file").map(PathBuf::from),
        status_every: status_every.unwrap_or(0),
        checkpoint: Some(
            opt(args, "--checkpoint")
                .map(PathBuf::from)
                .unwrap_or_else(|| out_dir.join("restart.rcfd")),
        ),
        checkpoint_every: ckpt_every.unwrap_or(0),
        info: serde_json::json!({
            "driver": "cli", "case": label, "continue_from": cont,
            "threads": threads.or_else(|| std::thread::available_parallelism().ok().map(|n| n.get())),
            "precision": "double",
        }),
    };
    say(&format!(
        "{} cells, algorithm {}",
        sim.geom.n_cells(),
        case.get("solver.algorithm")
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default()
    ));
    let sim = Mutex::new(sim);
    let outcome = runctl::drive(&sim, &opts, &|| false, &mut |ev| {
        if status_json {
            println!("{ev}");
        }
        if let Some(f) = status_file.as_mut() {
            use std::io::Write;
            let _ = writeln!(f, "{ev}");
        }
        match ev["event"].as_str() {
            Some("iteration") if !quiet && !status_json => println!(
                "{:>6}  t={:<10.4e} res_u {:.3e}  res_p {:.3e}  continuity {:.2e}",
                ev["index"],
                ev["time"].as_f64().unwrap_or(0.0),
                ev["res_u"].as_f64().unwrap_or(f64::NAN),
                ev["res_p"].as_f64().unwrap_or(f64::NAN),
                ev["continuity"].as_f64().unwrap_or(f64::NAN)
            ),
            Some("checkpoint") => eprintln!("checkpoint written to {}", ev["path"].as_str().unwrap_or("")),
            Some("warning") => eprintln!("warning: {}", ev["message"].as_str().unwrap_or("")),
            _ => {}
        }
    });
    let sim = sim.into_inner().unwrap_or_else(|e| e.into_inner());
    if let Err(e) = runctl::write_results(&sim, &out_dir) {
        eprintln!("error: writing results to {}: {e}", out_dir.display());
        return FAIL;
    }
    say(&format!("results written to {}", out_dir.display()));
    match outcome.exit_code {
        runctl::exit::OK => say(&format!(
            "finished after {} iterations/steps ({}: {})",
            outcome.iterations, outcome.state, outcome.reason
        )),
        runctl::exit::NOT_CONVERGED => eprintln!(
            "warning: stopped after {} iterations without meeting the tolerances",
            outcome.iterations
        ),
        _ => eprintln!("{}: {} (exit {})", outcome.state, outcome.reason, outcome.exit_code),
    }
    outcome.exit_code
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

const WF_VALUE_OPTS: &[&str] = &["--template", "--workflow", "--task", "--max-steps"];

fn cmd_workflow(args: &[String]) -> u8 {
    use rcfd_api::workflow::{Workflow, KINDS, TEMPLATES};
    let Some(sub) = args.first().map(String::as_str) else {
        return usage_error("workflow needs a subcommand: templates, show or run");
    };
    let rest = &args[1..];
    let load = |rest: &[String]| -> Result<Workflow, String> {
        if let Some(f) = opt(rest, "--workflow") {
            let text = std::fs::read_to_string(f).map_err(|e| format!("cannot read {f}: {e}"))?;
            let doc: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{f}: {e}"))?;
            Workflow::from_json(&doc).map_err(|e| format!("{f}: {e}"))
        } else {
            let t = opt(rest, "--template").unwrap_or("mesh_to_results");
            Workflow::template(t).ok_or_else(|| format!("unknown template '{t}'"))
        }
    };
    match sub {
        "templates" => {
            for (n, d) in TEMPLATES {
                println!("{n:<18} {d}");
            }
            println!("\ntask kinds:");
            for (n, d) in KINDS {
                println!("  {n:<12} {d}");
            }
            OK
        }
        "show" => match load(rest) {
            Ok(w) => {
                println!("{}", serde_json::to_string_pretty(&w.to_json()).unwrap_or_default());
                OK
            }
            Err(e) => usage_error(&e),
        },
        "run" => {
            let pos = positionals(rest, WF_VALUE_OPTS);
            let [path] = pos[..] else {
                return usage_error("workflow run takes exactly one case file");
            };
            let w = match load(rest) {
                Ok(w) => w,
                Err(e) => return usage_error(&e),
            };
            let abs = std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
            let svc = Service::new(ServiceConfig::default());
            let fail = |e: rcfd_api::service::RpcError| {
                eprintln!("error: {}", e.message);
                if let Some(d) = &e.data {
                    eprintln!("{d}");
                }
                if e.code == rcfd_api::service::code::VALIDATION {
                    INVALID
                } else {
                    FAIL
                }
            };
            let sid = match svc.call(
                "session.create",
                &serde_json::json!({"path": abs.display().to_string()}),
            ) {
                Ok(v) => v["session"].as_str().unwrap_or_default().to_string(),
                Err(e) => return fail(e),
            };
            if let Err(e) = svc.call(
                "workflow.create",
                &serde_json::json!({"session": sid, "workflow": w.to_json()}),
            ) {
                return fail(e);
            }
            let mut params = serde_json::json!({
                "session": sid,
                "with_prerequisites": !flag(rest, "--no-prerequisites"),
            });
            if let Some(t) = opt(rest, "--task") {
                params["task"] = t.into();
            }
            match parse_opt::<u64>(rest, "--max-steps") {
                Ok(Some(n)) => params["max_steps"] = n.into(),
                Ok(None) => {}
                Err(m) => return usage_error(&m),
            }
            let r = match svc.call("workflow.run", &params) {
                Ok(r) => r,
                Err(e) => return fail(e),
            };
            if flag(rest, "--json") {
                println!("{}", serde_json::to_string_pretty(&r).unwrap_or_default());
            } else {
                for t in r["ran"].as_array().into_iter().flatten() {
                    println!(
                        "{:<16} {:<7} {}",
                        t["id"].as_str().unwrap_or(""),
                        if t["ok"] == true { "done" } else { "FAILED" },
                        t["message"].as_str().unwrap_or("")
                    );
                }
            }
            if r["ok"] == true {
                return OK;
            }
            // exit code of the task that failed
            let bad = r["ran"].as_array().and_then(|a| a.iter().find(|t| t["ok"] == false));
            let kind = bad
                .and_then(|t| w.task(t["id"].as_str().unwrap_or("")))
                .map_or("", |t| t.kind.as_str());
            match kind {
                "check_mesh" => BAD_MESH,
                "validate" => INVALID,
                "solve" => bad
                    .and_then(|t| t["result"]["outcome"]["exit_code"].as_u64())
                    // a solve that stopped at the step cap without finishing is "not converged"
                    .map_or(FAIL, |c| if c == 0 { runctl::exit::NOT_CONVERGED } else { c as u8 }),
                _ => FAIL,
            }
        }
        other => usage_error(&format!("unknown workflow subcommand '{other}'")),
    }
}
