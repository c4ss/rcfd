//! Wave 19C command line additions (ADR 0031): `rcfd restart`, `rcfd check`, the `--set` /
//! `--restart-from` / `--print-settings` options of `rcfd run`, setup rules and chained runs.

use super::{flag, opt, parse_opt, positionals, usage_error, BAD_MESH, FAIL, INVALID, OK, USAGE_ERR};
use rcfd_api::case::Case;
use rcfd_api::chain::{self, SetError};
use rcfd_api::rules::{self, RuleDiagnostic};
use rcfd_api::runctl::{self, RunOptions};
use rcfd_mesh::quality::QualityReport;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Value options of `rcfd run` that this module adds.
pub const EXTRA_VALUE_OPTS: &[&str] = &["--set", "--restart-from", "--source-case", "--method", "--mesh"];

/// All values of a repeatable option, in order.
fn opt_all<'a>(args: &'a [String], name: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == name {
            if let Some(v) = args.get(i + 1) {
                out.push(v.as_str());
            }
            i += 1;
        }
        i += 1;
    }
    out
}

fn absolute(p: &str) -> String {
    let path = Path::new(p);
    if path.is_absolute() {
        return p.to_string();
    }
    std::env::current_dir()
        .map(|d| d.join(path).display().to_string())
        .unwrap_or_else(|_| p.to_string())
}

/// Apply `--set`, `--mesh`, `--restart-from`, `--source-case` and `--method` to the case. Returns the
/// exit code on failure (2 for a malformed `--set`, 3 for an unknown path or invalid value).
pub fn apply_overrides(case: &mut Case, args: &[String], label: &str) -> Result<(), u8> {
    let mut changed = false;
    for entry in opt_all(args, "--set") {
        match chain::apply_set(case, entry) {
            Ok(()) => changed = true,
            Err(SetError::Malformed(m)) => {
                eprintln!("error: --set {m}");
                return Err(USAGE_ERR);
            }
            Err(SetError::Invalid(m)) => {
                eprintln!("{label}: --set {m}");
                return Err(INVALID);
            }
        }
    }
    let mut set = |case: &mut Case, path: &str, v: Value| -> Result<(), u8> {
        case.set(path, v).map_err(|e| {
            eprintln!("{label}: {path}: {e}");
            INVALID
        })?;
        changed = true;
        Ok(())
    };
    if let Some(m) = opt(args, "--mesh") {
        set(case, "mesh.source", json!("file"))?;
        set(case, "mesh.file", json!(absolute(m)))?;
    }
    if let Some(f) = opt(args, "--restart-from") {
        set(case, "restart.from", json!(absolute(f)))?;
    }
    if let Some(f) = opt(args, "--source-case") {
        set(case, "restart.source_case", json!(absolute(f)))?;
    }
    if let Some(m) = opt(args, "--method") {
        set(case, "restart.method", json!(m))?;
    }
    if changed {
        let diags = case.semantic_errors();
        if !diags.is_empty() {
            for d in diags {
                eprintln!("{label}: {d}");
            }
            return Err(INVALID);
        }
    }
    Ok(())
}

/// Print the resolved settings (`--print-settings`).
pub fn print_settings(case: &Case) -> u8 {
    println!(
        "{}",
        serde_json::to_string_pretty(&case.tree.to_json()).unwrap_or_default()
    );
    OK
}

fn print_rules(label: &str, d: &[RuleDiagnostic]) {
    for x in d {
        eprintln!("{label}: {x}");
    }
}

/// Evaluate the setup rules for `case` with the mesh quality `q`; print them to stderr. Returns the
/// warning objects for the status stream, or the exit code 3 when a rule reports an error.
pub fn rules_gate(case: &Case, q: &QualityReport, label: &str) -> Result<Value, u8> {
    let d = rules::check_case(case, Some(&rules::mesh_context(q)));
    print_rules(label, &d);
    if rules::has_errors(&d) {
        return Err(INVALID);
    }
    Ok(rules::warnings_json(&d))
}

/// Rules without a built simulation, for `rcfd validate`: errors give exit 3.
pub fn validate_rules(case: &Case, label: &str) -> u8 {
    let mesh = rcfd_api::sim::build_mesh(case).ok();
    let ctx = mesh
        .as_ref()
        .map(|m| rules::mesh_context(&rcfd_mesh::quality::check_mesh(m)));
    let d = rules::check_case(case, ctx.as_ref());
    print_rules(label, &d);
    if rules::has_errors(&d) {
        INVALID
    } else {
        OK
    }
}

/// `rcfd restart <case> --from <file> ...`: `rcfd run` starting from a restart file.
pub fn cmd_restart(args: &[String]) -> u8 {
    let Some(from) = opt(args, "--from") else {
        return usage_error("restart needs --from <restart file>");
    };
    let mut a: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--from" {
            a.push("--restart-from".into());
        } else {
            a.push(args[i].clone());
        }
        i += 1;
    }
    let _ = from;
    super::cmd_run(&a)
}

/// `rcfd check <case> [--json] [--set ...]`.
pub fn cmd_check(args: &[String]) -> u8 {
    let pos = positionals(args, EXTRA_VALUE_OPTS);
    let [path] = pos[..] else {
        return usage_error("check takes exactly one case file");
    };
    let json_out = flag(args, "--json");
    let report = |code: u8, diags: Value, rules_v: Value, mesh: Value| {
        if json_out {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "ok": code == 0, "exit_code": code, "diagnostics": diags,
                    "rules": rules_v, "mesh": mesh,
                }))
                .unwrap_or_default()
            );
        }
        code
    };
    let mut case = match Case::load(path) {
        Ok(c) => c,
        Err(d) => {
            for x in &d {
                eprintln!("{path}: {x}");
            }
            return report(
                INVALID,
                serde_json::to_value(&d).unwrap_or(Value::Null),
                json!([]),
                Value::Null,
            );
        }
    };
    if let Err(code) = apply_overrides(&mut case, args, path) {
        return code;
    }
    let (code, ev) = runctl::check_only(&case);
    if let Some(ds) = ev["diagnostics"].as_array() {
        for d in ds {
            eprintln!(
                "{path}: {}: {}",
                d["path"].as_str().unwrap_or(""),
                d["message"].as_str().unwrap_or("")
            );
        }
    }
    let rules_v = ev["rules"].clone();
    if !json_out {
        if let Some(rs) = rules_v.as_array() {
            for r in rs {
                println!(
                    "{path}: {} [{}/{}] {}: {}",
                    r["id"].as_str().unwrap_or(""),
                    r["severity"].as_str().unwrap_or(""),
                    r["category"].as_str().unwrap_or(""),
                    r["path"].as_str().unwrap_or(""),
                    r["message"].as_str().unwrap_or("")
                );
            }
        }
        println!(
            "{path}: {}",
            match code {
                0 => format!("ok, {} cells", ev["cells"]),
                BAD_MESH => "mesh failed its quality check".to_string(),
                _ => "invalid".to_string(),
            }
        );
    }
    report(code, ev["diagnostics"].clone(), rules_v, ev["mesh"].clone())
}

/// True when `rcfd run` should run the chain of the case.
pub fn chain_requested(case: &Case, args: &[String]) -> bool {
    chain::is_chain(case) && !flag(args, "--no-chain")
}

/// Run the chain of `case` (`rcfd run` with a non-empty `chain.sequence`).
pub fn run_chain_cli(case: &Case, args: &[String], label: &str) -> u8 {
    let status_json = flag(args, "--status-json");
    let quiet = flag(args, "--quiet");
    let say = |s: &str| {
        if status_json {
            eprintln!("{s}");
        } else {
            println!("{s}");
        }
    };
    let out_dir = opt(args, "--out")
        .map(PathBuf::from)
        .or_else(|| {
            case.get("output.directory")
                .ok()
                .and_then(|v| v.as_str().map(PathBuf::from))
        })
        .unwrap_or_else(|| PathBuf::from("results"));
    let (max_elapsed, status_every, ckpt_every) = match (
        parse_opt::<f64>(args, "--max-elapsed"),
        parse_opt::<usize>(args, "--status-every"),
        parse_opt::<usize>(args, "--checkpoint-every"),
    ) {
        (Ok(a), Ok(b), Ok(c)) => (a, b, c),
        (Err(m), ..) | (_, Err(m), _) | (_, _, Err(m)) => return usage_error(&m),
    };
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
        checkpoint: None,
        checkpoint_every: ckpt_every.unwrap_or(0),
        out_dir: None,
        info: json!({"driver": "cli", "case": label, "chain": true}),
    };
    say(&format!(
        "chain of {} stages: {}",
        chain::sequence(case).len(),
        chain::sequence(case).join(", ")
    ));
    let outcome = chain::run_chain(case, &opts, &out_dir, &|| false, &mut |ev| {
        if status_json {
            println!("{ev}");
        }
        if let Some(f) = status_file.as_mut() {
            use std::io::Write;
            let _ = writeln!(f, "{ev}");
        }
        let stage = ev["stage"].as_str().unwrap_or("");
        match ev["event"].as_str() {
            Some("iteration") if !quiet && !status_json => println!(
                "[{stage}] {:>6}  t={:<10.4e} res_u {:.3e}  res_p {:.3e}",
                ev["index"],
                ev["time"].as_f64().unwrap_or(0.0),
                ev["res_u"].as_f64().unwrap_or(f64::NAN),
                ev["res_p"].as_f64().unwrap_or(f64::NAN),
            ),
            Some("stage_start") => say(&format!(
                "stage {stage} ({}/{})",
                ev["index"].as_u64().unwrap_or(0) + 1,
                ev["of"]
            )),
            Some("stage_finish") => say(&format!(
                "stage {stage}: {} (exit {})",
                ev["state"].as_str().unwrap_or(""),
                ev["exit_code"]
            )),
            Some("restart") => eprintln!("[{stage}] restart: {}", ev["report"]),
            Some("warning") => eprintln!(
                "warning{}: {}",
                ev["rule"].as_str().map(|r| format!(" {r}")).unwrap_or_default(),
                ev["message"].as_str().unwrap_or("")
            ),
            Some("error") => eprintln!("error: {}", ev["message"].as_str().unwrap_or("")),
            _ => {}
        }
    });
    say(&format!(
        "chain {} (exit {}), results in {}",
        outcome.state,
        outcome.exit_code,
        Path::new(&out_dir).display()
    ));
    outcome.exit_code
}
