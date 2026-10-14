//! `rcfd reactor` (0D reactor toolkit, F-112) and `rcfd acoustics` (FW-H far field, F-136).

use rcfd_post::fwh::Surface;
use std::path::Path;

const OK: u8 = 0;
const FAIL: u8 = 1;
const USAGE_ERR: u8 = 2;
const INVALID: u8 = 3;

fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn opts<'a>(args: &'a [String], name: &str) -> Vec<&'a str> {
    args.iter()
        .enumerate()
        .filter(|(_, a)| *a == name)
        .filter_map(|(i, _)| args.get(i + 1).map(String::as_str))
        .collect()
}

fn first_positional<'a>(args: &'a [String], value_opts: &[&str]) -> Option<&'a str> {
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
        } else if value_opts.contains(&a.as_str()) {
            skip = true;
        } else if !a.starts_with("--") {
            return Some(a.as_str());
        }
    }
    None
}

/// `rcfd reactor <case.json> [--out <file.csv>] [--json]`
pub fn cmd_reactor(args: &[String]) -> u8 {
    let Some(path) = first_positional(args, &["--out"]) else {
        eprintln!("error: reactor needs a case file\n\nrun `rcfd --help` for usage");
        return USAGE_ERR;
    };
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: cannot read {path}: {e}");
            return FAIL;
        }
    };
    let doc: serde_json::Value = match serde_json::from_str(&text) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{path}:{}:{}: invalid JSON: {e}", e.line(), e.column());
            return INVALID;
        }
    };
    let base = Path::new(path).parent().unwrap_or(Path::new("."));
    match rcfd_reactor::case::run_case(&doc, base) {
        Ok(res) => {
            if let (Some(out), Some(csv)) = (opt(args, "--out"), &res.csv) {
                if let Err(e) = std::fs::write(out, csv) {
                    eprintln!("error: cannot write {out}: {e}");
                    return FAIL;
                }
                eprintln!("trajectory written to {out}");
            }
            println!("{}", serde_json::to_string_pretty(&res.summary).unwrap_or_default());
            OK
        }
        Err(errs) => {
            for e in &errs {
                eprintln!("{path}: {e}");
            }
            INVALID
        }
    }
}

fn parse_obs(s: &str) -> Result<[f64; 3], String> {
    let v: Vec<&str> = s.split(',').collect();
    if v.len() != 3 {
        return Err(format!("--observer '{s}': expected x,y,z"));
    }
    let mut r = [0.0; 3];
    for (i, t) in v.iter().enumerate() {
        r[i] = t
            .trim()
            .parse::<f64>()
            .map_err(|_| format!("--observer '{s}': component {} is not a number", i + 1))?;
    }
    Ok(r)
}

/// `rcfd acoustics <surface.json> --observer x,y,z [--observer ...] [--times t0:t1:n] [--out file.csv]`
pub fn cmd_acoustics(args: &[String]) -> u8 {
    let Some(path) = first_positional(args, &["--observer", "--times", "--out"]) else {
        eprintln!("error: acoustics needs a surface file (rcfd-fwh-surface JSON)\n\nrun `rcfd --help` for usage");
        return USAGE_ERR;
    };
    let observers: Vec<_> = opts(args, "--observer");
    if observers.is_empty() {
        eprintln!("error: acoustics needs at least one --observer x,y,z");
        return USAGE_ERR;
    }
    let mut obs = Vec::new();
    for o in &observers {
        match parse_obs(o) {
            Ok(v) => obs.push(v),
            Err(e) => {
                eprintln!("error: {e}");
                return USAGE_ERR;
            }
        }
    }
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: cannot read {path}: {e}");
            return FAIL;
        }
    };
    let doc: serde_json::Value = match serde_json::from_str(&text) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{path}:{}:{}: invalid JSON: {e}", e.line(), e.column());
            return INVALID;
        }
    };
    let surf = match Surface::from_json(&doc) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{path}: {e}");
            return INVALID;
        }
    };
    let mut csv = String::from("observer,t,p_thickness,p_loading,p_total\n");
    for (i, x) in obs.iter().enumerate() {
        let (lo, hi) = match surf.valid_window(*x) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("error: observer {}: {e}", i + 1);
                return INVALID;
            }
        };
        let times: Vec<f64> = match opt(args, "--times") {
            Some(spec) => {
                let p: Vec<&str> = spec.split(':').collect();
                let parsed: Option<(f64, f64, usize)> = (p.len() == 3)
                    .then(|| Some((p[0].parse().ok()?, p[1].parse().ok()?, p[2].parse().ok()?)))
                    .flatten();
                match parsed {
                    Some((a, b, n)) if n >= 1 => (0..n)
                        .map(|k| {
                            if n == 1 {
                                a
                            } else {
                                a + (b - a) * k as f64 / (n - 1) as f64
                            }
                        })
                        .collect(),
                    _ => {
                        eprintln!("error: --times '{spec}': expected t0:t1:n with n >= 1");
                        return USAGE_ERR;
                    }
                }
            }
            None => {
                let n = ((hi - lo) / surf.dt).floor() as usize;
                (0..=n).map(|k| lo + k as f64 * surf.dt).collect()
            }
        };
        match surf.signal(*x, &times) {
            Ok(sig) => {
                for s in sig {
                    csv.push_str(&format!(
                        "{},{:e},{:e},{:e},{:e}\n",
                        i + 1,
                        s.t,
                        s.p_thickness,
                        s.p_loading,
                        s.total()
                    ));
                }
            }
            Err(e) => {
                eprintln!("error: observer {}: {e}", i + 1);
                return INVALID;
            }
        }
    }
    match opt(args, "--out") {
        Some(f) => {
            if let Err(e) = std::fs::write(f, csv) {
                eprintln!("error: cannot write {f}: {e}");
                return FAIL;
            }
            eprintln!("wrote {f}");
        }
        None => print!("{csv}"),
    }
    OK
}
