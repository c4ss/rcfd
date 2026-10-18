//! `rcfd map` (ADR 0067): operating-map automation. Chained operating-point runs over boundary-condition (or any settings)
//! values, each restarting from the previous converged solution, with a metrics table.

use super::{cmd_w19c, flag, load_case, opt, parse_opt, positionals, usage_error, FAIL, INVALID, OK};
use rcfd_api::design::case_eval::{CaseMapEvaluator, RunLimits};
use rcfd_api::design::opmap::{self, MapOptions, OnFailure, PointStatus};
use std::path::PathBuf;

const VALUE_OPTS: &[&str] = &["--axis", "--out", "--max-steps", "--set"];

/// Exit code when some point did not converge.
const NOT_CONVERGED: u8 = 5;

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

/// `rcfd map <case> --axis path=v1,v2,... [--axis path=start:stop:count]... [--out dir] [--cold] [--stop-on-failure]
/// [--no-retry] [--serpentine] [--max-steps n] [--json] [--quiet] [--set path=json]...`
pub fn cmd_map(args: &[String]) -> u8 {
    let pos = positionals(args, VALUE_OPTS);
    let [path] = pos[..] else {
        return usage_error("map takes exactly one case file");
    };
    let max_steps = match parse_opt::<usize>(args, "--max-steps") {
        Ok(v) => v,
        Err(m) => return usage_error(&m),
    };
    let axes: Vec<_> = match opt_all(args, "--axis").into_iter().map(opmap::parse_axis).collect() {
        Ok(a) => a,
        Err(m) => return usage_error(&m),
    };
    if axes.is_empty() {
        return usage_error("map needs at least one --axis path=v1,v2,...");
    }
    let points = match opmap::grid_points(&axes, flag(args, "--serpentine")) {
        Ok(p) => p,
        Err(m) => return usage_error(&m),
    };
    let mut case = match load_case(path) {
        Ok(c) => c,
        Err(code) => return code,
    };
    if let Err(code) = cmd_w19c::apply_overrides(&mut case, args, path) {
        return code;
    }
    // a bad axis path or value is found before the first run
    for (axis, v) in axes.iter().map(|a| (a, &a.values[0])) {
        let mut probe = case.clone();
        if let Err(e) = rcfd_api::study::apply_value(&mut probe, &axis.path, v) {
            eprintln!("{path}: axis {}: {e}", axis.path);
            return INVALID;
        }
    }
    let out_dir = opt(args, "--out").map_or_else(|| PathBuf::from("map"), PathBuf::from);
    let opts = MapOptions {
        on_failure: if flag(args, "--stop-on-failure") {
            OnFailure::Stop
        } else {
            OnFailure::Continue
        },
        retry_cold: !flag(args, "--no-retry"),
        cold: flag(args, "--cold"),
        out_dir: Some(out_dir.clone()),
    };
    let ev = CaseMapEvaluator {
        base: case,
        limits: RunLimits {
            max_steps,
            ..RunLimits::default()
        },
    };
    let quiet = flag(args, "--quiet") || flag(args, "--json");
    if !quiet {
        eprintln!("map: {} points, tables in {}", points.len(), out_dir.display());
    }
    let out = match opmap::run_and_write(&points, &ev, &opts) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            return FAIL;
        }
    };
    if !quiet {
        for r in &out.rows {
            eprintln!(
                "point {:>4} {:<14} {:<10} {:>6} iterations  {}",
                r.index,
                r.status.name(),
                r.restart,
                r.iterations,
                r.label
            );
        }
    }
    if flag(args, "--json") {
        println!(
            "{}",
            serde_json::json!({
                "format": "rcfd-operating-map", "state": out.state, "planned": out.planned, "converged": out.converged(),
                "csv": out_dir.join("map.csv").display().to_string(), "json": out_dir.join("map.json").display().to_string(),
                "points": out.rows.iter().map(opmap::row_json).collect::<Vec<_>>(),
            })
        );
    } else {
        println!(
            "map {}: {} of {} points converged; table in {}",
            out.state,
            out.converged(),
            out.planned,
            out_dir.join("map.csv").display()
        );
    }
    if out.rows.iter().all(|r| r.status == PointStatus::Converged) && out.rows.len() == out.planned {
        OK
    } else {
        NOT_CONVERGED
    }
}
