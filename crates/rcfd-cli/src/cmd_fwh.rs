//! `rcfd fwh`: FW-H far field (formulation 1A) of an impermeable moving surface from a surface-data time series
//! (`rcfd-surface-series`), with optional uniform mean flow and spectra (ADR 0066).

use rcfd_post::acoustics::{spectrum_report, Window};
use rcfd_post::fwh::{medium_frame_observer, Signal};
use rcfd_post::surface_series::{pressure_csv_path, SeriesSpec};
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

fn parse_v3(flag: &str, s: &str) -> Result<[f64; 3], String> {
    let v: Vec<&str> = s.split(',').collect();
    if v.len() != 3 {
        return Err(format!("{flag} '{s}': expected x,y,z"));
    }
    let mut r = [0.0; 3];
    for (i, t) in v.iter().enumerate() {
        r[i] = t
            .trim()
            .parse::<f64>()
            .map_err(|_| format!("{flag} '{s}': component {} is not a number", i + 1))?;
    }
    Ok(r)
}

const VALUE_OPTS: [&str; 8] = [
    "--observer",
    "--times",
    "--out",
    "--spectrum",
    "--narrowband",
    "--flow",
    "--fmin",
    "--fmax",
];

/// `rcfd fwh <series.json> --observer x,y,z [--observer ...] [--flow ux,uy,uz] [--times t0:t1:n] [--out file.csv]
/// [--spectrum bands.csv|spectrum.json] [--narrowband file.csv] [--fmin hz] [--fmax hz] [--window hann|rect]`
pub fn cmd_fwh(args: &[String]) -> u8 {
    let mut value_opts = VALUE_OPTS.to_vec();
    value_opts.push("--window");
    let Some(path) = first_positional(args, &value_opts) else {
        eprintln!("error: fwh needs a surface-series file (rcfd-surface-series JSON)\n\nrun `rcfd --help` for usage");
        return USAGE_ERR;
    };
    let mut obs = Vec::new();
    for o in opts(args, "--observer") {
        match parse_v3("--observer", o) {
            Ok(v) => obs.push(v),
            Err(e) => {
                eprintln!("error: {e}");
                return USAGE_ERR;
            }
        }
    }
    if obs.is_empty() {
        eprintln!("error: fwh needs at least one --observer x,y,z");
        return USAGE_ERR;
    }
    let flow_override = match opt(args, "--flow").map(|s| parse_v3("--flow", s)) {
        Some(Ok(v)) => Some(v),
        Some(Err(e)) => {
            eprintln!("error: {e}");
            return USAGE_ERR;
        }
        None => None,
    };
    let window = match opt(args, "--window") {
        None | Some("hann") => Window::Hann,
        Some("rect") => Window::Rectangular,
        Some(o) => {
            eprintln!("error: --window '{o}': expected hann or rect");
            return USAGE_ERR;
        }
    };
    let num = |name: &str, default: f64| -> Result<f64, String> {
        match opt(args, name) {
            None => Ok(default),
            Some(s) => s.parse::<f64>().map_err(|_| format!("{name} '{s}': not a number")),
        }
    };
    let (fmin, fmax) = match (num("--fmin", 20.0), num("--fmax", 20000.0)) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("error: {e}");
            return USAGE_ERR;
        }
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
    let csv_text = match pressure_csv_path(&doc) {
        Some(rel) => {
            let p = Path::new(path).parent().unwrap_or(Path::new(".")).join(rel);
            match std::fs::read_to_string(&p) {
                Ok(t) => Some(t),
                Err(e) => {
                    eprintln!("error: cannot read {}: {e}", p.display());
                    return FAIL;
                }
            }
        }
        None => None,
    };
    let mut spec = match SeriesSpec::from_json(&doc, csv_text.as_deref()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{path}: {e}");
            return INVALID;
        }
    };
    if let Some(f) = flow_override {
        spec.flow = f;
    }
    let case = match spec.build() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{path}: {e}");
            return INVALID;
        }
    };
    let surf = match case.medium_frame() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{path}: {e}");
            return INVALID;
        }
    };
    let flow = case.flow;
    let uf = (flow[0] * flow[0] + flow[1] * flow[1] + flow[2] * flow[2]).sqrt();
    let ns = surf.samples();
    let record_end = surf.t0 + ns as f64 * surf.dt;
    let mut csv = String::from("observer,t,p_thickness,p_loading,p_total\n");
    let mut band_csv = String::from("observer,nominal_hz,centre_hz,lo_hz,hi_hz,ms_pa2,level_db,level_dba\n");
    let mut narrow_csv = String::from("observer,f_hz,ms_pa2,level_db\n");
    let mut json_obs = Vec::new();
    for (i, x) in obs.iter().enumerate() {
        let path_fn = medium_frame_observer(*x, flow);
        let dmax = case
            .surface
            .elements
            .iter()
            .flat_map(|e| e.x.iter())
            .map(|y| ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt())
            .fold(0.0, f64::max);
        let window_t = if uf == 0.0 {
            surf.valid_window(*x)
        } else {
            surf.valid_window_moving(&path_fn, surf.t0, record_end + dmax / (surf.c0 - uf), 8.0 * surf.dt)
        };
        let (lo, hi) = match window_t {
            Ok(w) => w,
            Err(e) => {
                eprintln!("error: observer {}: {e}", i + 1);
                return INVALID;
            }
        };
        let times: Vec<f64> = match opt(args, "--times") {
            Some(s) => {
                let p: Vec<&str> = s.split(':').collect();
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
                        eprintln!("error: --times '{s}': expected t0:t1:n with n >= 1");
                        return USAGE_ERR;
                    }
                }
            }
            None => {
                let n = ((hi - lo) / surf.dt).floor() as usize;
                (0..=n).map(|k| lo + k as f64 * surf.dt).collect()
            }
        };
        let sig: Vec<Signal> = match surf.signal_moving(&path_fn, &times) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: observer {}: {e}", i + 1);
                return INVALID;
            }
        };
        for s in &sig {
            csv.push_str(&format!(
                "{},{:e},{:e},{:e},{:e}\n",
                i + 1,
                s.t,
                s.p_thickness,
                s.p_loading,
                s.total()
            ));
        }
        if opt(args, "--spectrum").is_some() || opt(args, "--narrowband").is_some() {
            if times.len() < 8 {
                eprintln!("error: observer {}: spectra need at least 8 observer times", i + 1);
                return USAGE_ERR;
            }
            let dt_obs = (times[times.len() - 1] - times[0]) / (times.len() - 1) as f64;
            let p: Vec<f64> = sig.iter().map(Signal::total).collect();
            match spectrum_report(&p, dt_obs, window, fmin, fmax) {
                Ok(rep) => {
                    for line in rep.bands_csv().lines().skip(1) {
                        band_csv.push_str(&format!("{},{}\n", i + 1, line));
                    }
                    for line in rep.narrowband_csv().lines().skip(1) {
                        narrow_csv.push_str(&format!("{},{}\n", i + 1, line));
                    }
                    let fmt = |v: Option<f64>| v.map_or("undefined".to_string(), |x| format!("{x:.2}"));
                    eprintln!(
                        "observer {}: OASPL {} dB, {} dB(A)",
                        i + 1,
                        fmt(rep.oaspl),
                        fmt(rep.oaspl_a)
                    );
                    json_obs.push(serde_json::json!({"observer": i + 1, "position": x, "report": rep.to_json()}));
                }
                Err(e) => {
                    eprintln!("error: observer {}: {e}", i + 1);
                    return INVALID;
                }
            }
        }
    }
    let write = |f: &str, text: &str| -> bool {
        match std::fs::write(f, text) {
            Ok(()) => {
                eprintln!("wrote {f}");
                true
            }
            Err(e) => {
                eprintln!("error: cannot write {f}: {e}");
                false
            }
        }
    };
    match opt(args, "--out") {
        Some(f) => {
            if !write(f, &csv) {
                return FAIL;
            }
        }
        None => print!("{csv}"),
    }
    if let Some(f) = opt(args, "--spectrum") {
        let ok = if f.ends_with(".json") {
            let doc = serde_json::json!({"format": "rcfd-spectra", "version": 1, "observers": json_obs});
            write(f, &serde_json::to_string_pretty(&doc).unwrap_or_default())
        } else {
            write(f, &band_csv)
        };
        if !ok {
            return FAIL;
        }
    }
    if let Some(f) = opt(args, "--narrowband") {
        if !write(f, &narrow_csv) {
            return FAIL;
        }
    }
    OK
}
