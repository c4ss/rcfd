//! `rcfd study` (ADR 0034): parametric studies from the `study` section of a case.

use super::{cmd_w19c, flag, load_case, opt, parse_opt, positionals, usage_error, FAIL, INVALID, OK};
use rcfd_api::study::{self, StudyOptions};
use std::path::PathBuf;

const VALUE_OPTS: &[&str] = &["--out", "--parallel", "--max-new-points", "--set"];

/// `rcfd study <case> [--out dir] [--parallel n] [--resume] [--max-new-points n] [--no-fields]
/// [--list] [--json] [--set path=json]...`
pub fn cmd_study(args: &[String]) -> u8 {
    let pos = positionals(args, VALUE_OPTS);
    let [path] = pos[..] else {
        return usage_error("study takes exactly one case file");
    };
    let (parallel, max_new) = match (
        parse_opt::<usize>(args, "--parallel"),
        parse_opt::<usize>(args, "--max-new-points"),
    ) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(m), _) | (_, Err(m)) => return usage_error(&m),
    };
    if parallel == Some(0) {
        return usage_error("--parallel must be at least 1");
    }
    let mut case = match load_case(path) {
        Ok(c) => c,
        Err(code) => return code,
    };
    if let Err(code) = cmd_w19c::apply_overrides(&mut case, args, path) {
        return code;
    }
    let json_out = flag(args, "--json");
    let plan = match study::plan(&case, true) {
        Ok(p) => p,
        Err(diags) => {
            for d in diags {
                eprintln!("{path}: {d}");
            }
            return INVALID;
        }
    };
    if flag(args, "--list") {
        let points = plan.points();
        if json_out {
            let v: Vec<serde_json::Value> = points
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "point": p.index,
                        "parameters": p.values.iter().cloned().collect::<serde_json::Map<_, _>>(),
                    })
                })
                .collect();
            println!("{}", serde_json::Value::Array(v));
        } else {
            for p in &points {
                let vals: Vec<String> = p.values.iter().map(|(n, v)| format!("{n}={v}")).collect();
                println!("{:>5}  {}", p.index, vals.join("  "));
            }
        }
        return OK;
    }
    let opts = StudyOptions {
        out_dir: opt(args, "--out").map_or_else(|| PathBuf::from("study"), PathBuf::from),
        parallel,
        resume: flag(args, "--resume"),
        max_new_points: max_new,
        no_fields: flag(args, "--no-fields"),
    };
    let quiet = flag(args, "--quiet");
    let out = study::run_study(&case, &opts, &mut |ev| {
        if !quiet && !json_out {
            eprintln!(
                "point {} {} (exit {})",
                ev["point"],
                ev["status"].as_str().unwrap_or(""),
                ev["exit_code"]
            );
        }
    });
    match out {
        Ok(o) => {
            if json_out {
                println!("{}", o.to_json(&opts.out_dir));
            } else {
                println!(
                    "study {}: {} points, {} run, {} skipped, {} failed; table in {}",
                    o.state,
                    o.points,
                    o.run,
                    o.skipped,
                    o.failed,
                    opts.out_dir.join("table.csv").display()
                );
                for r in o.rows.iter().filter(|r| r.status != "ok") {
                    eprintln!("point {} failed (exit {}): {}", r.point, r.exit_code, r.message);
                }
            }
            o.exit_code
        }
        Err(diags) => {
            for d in diags {
                eprintln!("{path}: {d}");
            }
            if opts.out_dir.as_os_str().is_empty() {
                FAIL
            } else {
                INVALID
            }
        }
    }
}
