//! `rcfd duct-modes`: the modal amplitudes of a hard-walled duct at one frequency from pressure time series on duct
//! sections (`rcfd-surface-series`, one file per section), written as the `rcfd-duct-modes` export of
//! `rcfd_post::duct_modes` (F-451, ADR 0783). One section gives the export of kind `section`, two or more the axial
//! split into the waves towards `+x` and `-x` (kind `axial`). The chain is the library's (ADR 0351 DR7, DR8).

use rcfd_post::duct_modes::{
    axial_wavenumber, decompose_section, split_axial, DuctShape, HarmonicDft, ModalExport, ModeBasis, C64,
};
use rcfd_post::surface_series::{Motion, SeriesSpec};
use std::f64::consts::PI;

const OK: u8 = 0;
const FAIL: u8 = 1;
const USAGE_ERR: u8 = 2;
const INVALID: u8 = 3;

const VALUE_OPTS: [&str; 6] = ["--duct", "--frequency", "--k-max", "--x-ref", "--origin", "--out"];

/// A failure with its exit code; the message is one line of standard error.
enum Stop {
    Usage(String),
    Io(String),
    Invalid(String),
}

fn usage<T>(msg: impl Into<String>) -> Result<T, Stop> {
    Err(Stop::Usage(msg.into()))
}

/// A located error of a series file: `<path>: invalid value at '<json path>': <reason>` (ADR 0783 D3).
fn located<T>(path: &str, json_path: &str, reason: impl AsRef<str>) -> Result<T, Stop> {
    Err(Stop::Invalid(format!(
        "{path}: invalid value at '{json_path}': {}",
        reason.as_ref()
    )))
}

fn number(flag: &str, s: &str) -> Result<f64, Stop> {
    match s.trim().parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(v),
        _ => usage(format!("{flag} '{s}': not a finite number")),
    }
}

fn numbers(flag: &str, s: &str, count: usize, form: &str) -> Result<Vec<f64>, Stop> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != count {
        return usage(format!("{flag} '{s}': expected {form}"));
    }
    parts.iter().map(|p| number(flag, p)).collect()
}

fn parse_duct(s: &str) -> Result<DuctShape, Stop> {
    let form = "rect:a,b or circ:radius";
    let (shape, sizes) = match s.split_once(':') {
        Some(("rect", rest)) => {
            let v = numbers("--duct", rest, 2, form)?;
            (DuctShape::Rectangular { a: v[0], b: v[1] }, v)
        }
        Some(("circ", rest)) => {
            let v = numbers("--duct", rest, 1, form)?;
            (DuctShape::Circular { radius: v[0] }, v)
        }
        _ => return usage(format!("--duct '{s}': expected {form}")),
    };
    if sizes.iter().any(|v| *v <= 0.0) {
        return usage(format!("--duct '{s}': the sizes must be > 0"));
    }
    Ok(shape)
}

/// The options of one call.
struct Options<'a> {
    files: Vec<&'a str>,
    shape: DuctShape,
    frequency: f64,
    k_max: Option<f64>,
    x_ref: Option<f64>,
    origin: [f64; 2],
    out: Option<&'a str>,
}

fn parse_args(args: &[String]) -> Result<Options<'_>, Stop> {
    let mut files = Vec::new();
    let mut values: Vec<(&str, &str)> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if VALUE_OPTS.contains(&a) {
            let Some(v) = args.get(i + 1) else {
                return usage(format!("{a} needs a value"));
            };
            values.push((a, v.as_str()));
            i += 2;
        } else if a.starts_with("--") {
            return usage(format!("duct-modes: unknown option '{a}'"));
        } else {
            files.push(a);
            i += 1;
        }
    }
    let opt = |name: &str| values.iter().find(|(n, _)| *n == name).map(|(_, v)| *v);
    if files.is_empty() {
        return usage("duct-modes needs at least one surface-series file (rcfd-surface-series JSON)");
    }
    let Some(duct) = opt("--duct") else {
        return usage("duct-modes needs --duct rect:a,b or --duct circ:radius");
    };
    let shape = parse_duct(duct)?;
    let Some(freq) = opt("--frequency") else {
        return usage("duct-modes needs --frequency hz");
    };
    let frequency = number("--frequency", freq)?;
    if frequency <= 0.0 {
        return usage(format!("--frequency '{freq}': must be > 0"));
    }
    let k_max = match opt("--k-max") {
        Some(s) => {
            let v = number("--k-max", s)?;
            if v < 0.0 {
                return usage(format!("--k-max '{s}': must be >= 0"));
            }
            Some(v)
        }
        None => None,
    };
    let x_ref = match opt("--x-ref") {
        Some(_) if files.len() < 2 => {
            return usage("--x-ref needs two or more sections (one section gives no axial waves)");
        }
        Some(s) => Some(number("--x-ref", s)?),
        None => None,
    };
    let origin = match opt("--origin") {
        Some(s) => {
            let v = numbers("--origin", s, 2, "y,z")?;
            [v[0], v[1]]
        }
        None => [0.0, 0.0],
    };
    Ok(Options {
        files,
        shape,
        frequency,
        k_max,
        x_ref,
        origin,
        out: opt("--out"),
    })
}

fn read_series(path: &str) -> Result<SeriesSpec, Stop> {
    let text = std::fs::read_to_string(path).map_err(|e| Stop::Io(format!("error: cannot read {path}: {e}")))?;
    let doc: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| Stop::Invalid(format!("{path}:{}:{}: invalid JSON: {e}", e.line(), e.column())))?;
    let spec = SeriesSpec::from_json(&doc, None).map_err(|e| Stop::Invalid(format!("{path}: {e}")))?;
    spec.validate().map_err(|e| Stop::Invalid(format!("{path}: {e}")))?;
    Ok(spec)
}

/// One section: its station and the amplitudes of the basis modes there.
struct Section {
    station: f64,
    amplitudes: Vec<C64>,
    residual: f64,
}

/// The checks of ADR 0783 D3 on one file, then the DFT at `omega` and the section fit (ADR 0351 DR7).
fn section(path: &str, spec: &SeriesSpec, o: &Options, basis: &ModeBasis, medium: (f64, f64)) -> Result<Section, Stop> {
    if spec.flow != [0.0; 3] {
        return located(
            path,
            "$.flow",
            "the medium of a duct-modes export is at rest; a uniform flow is not supported",
        );
    }
    if let Motion::Rotation { .. } = spec.motion {
        return located(
            path,
            "$.motion",
            "a moving surface is not a duct section; only static is supported",
        );
    }
    for (json_path, file, first) in [("$.rho0", spec.rho0, medium.0), ("$.c0", spec.c0, medium.1)] {
        if (file / first - 1.0).abs() > 1e-9 {
            return located(path, json_path, format!("{file} differs from the first file's {first}"));
        }
    }
    // Whole periods (T2).
    let n = spec.samples();
    let r = n as f64 * spec.dt * o.frequency;
    let periods = r.round();
    if periods < 1.0 || (r - periods).abs() > 1e-6 * r.max(1.0) {
        return located(
            path,
            "$.faces[0].p",
            format!(
                "{n} samples of dt = {} s cover {r} periods of {} Hz, not a whole number of periods",
                spec.dt, o.frequency
            ),
        );
    }
    // Geometry: normals along x, one section plane, inside the duct (T1).
    let size = match o.shape {
        DuctShape::Rectangular { a, b } => a.max(b),
        DuctShape::Circular { radius } => radius,
    };
    let tol = 1e-6 * size;
    let station = spec.faces[0].x[0];
    let mut points = Vec::with_capacity(spec.faces.len());
    for (k, f) in spec.faces.iter().enumerate() {
        if f.n[0].abs() < 1.0 - 1e-6 {
            return located(
                path,
                &format!("$.faces[{k}].n"),
                "the face normal is not along the duct axis x",
            );
        }
        let off = f.x[0] - station;
        if off.abs() > tol {
            return located(
                path,
                &format!("$.faces[{k}].x"),
                format!("the face lies {off} m off the section plane"),
            );
        }
        let s = [f.x[1] - o.origin[0], f.x[2] - o.origin[1]];
        let inside = match o.shape {
            DuctShape::Rectangular { a, b } => s[0] >= -tol && s[0] <= a + tol && s[1] >= -tol && s[1] <= b + tol,
            DuctShape::Circular { radius } => s[0].hypot(s[1]) <= radius + tol,
        };
        if !inside {
            return located(
                path,
                &format!("$.faces[{k}].x"),
                format!("section point ({}, {}) lies outside the duct", s[0], s[1]),
            );
        }
        points.push(s);
    }
    if basis.len() > spec.faces.len() {
        return located(
            path,
            "$.faces",
            format!(
                "{} modes need at least as many faces, got {}",
                basis.len(),
                spec.faces.len()
            ),
        );
    }
    let lib = |e: rcfd_core::Error| Stop::Invalid(format!("{path}: {e}"));
    let weights: Vec<f64> = spec.faces.iter().map(|f| f.area).collect();
    let mut dft = HarmonicDft::new(2.0 * PI * o.frequency, spec.faces.len()).map_err(lib)?;
    let mut row = vec![0.0; spec.faces.len()];
    for i in 0..n {
        for (v, f) in row.iter_mut().zip(&spec.faces) {
            *v = f.p[i];
        }
        dft.add(spec.t0 + i as f64 * spec.dt, &row).map_err(lib)?;
    }
    let fit = decompose_section(basis, &points, &weights, &dft.amplitudes().map_err(lib)?).map_err(lib)?;
    Ok(Section {
        station,
        amplitudes: fit.amplitudes,
        residual: fit.residual,
    })
}

fn export(o: &Options) -> Result<String, Stop> {
    let lib = |e: rcfd_core::Error| Stop::Invalid(format!("error: {e}"));
    let first = read_series(o.files[0])?;
    let (rho0, c0) = (first.rho0, first.c0);
    let k = 2.0 * PI * o.frequency / c0;
    let basis = ModeBasis::below(o.shape, o.k_max.unwrap_or(k)).map_err(lib)?;
    let mut sections = Vec::with_capacity(o.files.len());
    for (i, path) in o.files.iter().enumerate() {
        let spec = if i == 0 { first.clone() } else { read_series(path)? };
        sections.push(section(path, &spec, o, &basis, (rho0, c0))?);
    }
    if sections.len() == 1 {
        let s = sections.remove(0);
        let fit = rcfd_post::duct_modes::SectionFit {
            amplitudes: s.amplitudes,
            residual: s.residual,
        };
        return Ok(ModalExport::section(&basis, o.frequency, c0, rho0, &fit)
            .map_err(lib)?
            .to_json_string());
    }
    let xs: Vec<f64> = sections.iter().map(|s| s.station).collect();
    let x_ref = o.x_ref.unwrap_or(xs[0]);
    let residual = sections.iter().fold(0.0, |r: f64, s| r.max(s.residual));
    let mut pairs = Vec::with_capacity(basis.len());
    for q in 0..basis.len() {
        let amps: Vec<C64> = sections.iter().map(|s| s.amplitudes[q]).collect();
        let f = split_axial(&xs, &amps, axial_wavenumber(k, basis.k_cut[q]), x_ref).map_err(lib)?;
        pairs.push((f.plus, f.minus));
    }
    Ok(
        ModalExport::axial(&basis, o.frequency, c0, rho0, x_ref, &pairs, residual)
            .map_err(lib)?
            .to_json_string(),
    )
}

/// `rcfd duct-modes <series.json> [<series.json> ...] --duct rect:a,b|circ:radius --frequency hz [--k-max k]
/// [--x-ref x] [--origin y,z] [--out file.json]`
pub fn cmd_duct_modes(args: &[String]) -> u8 {
    let result = parse_args(args).and_then(|o| {
        let text = export(&o)?;
        match o.out {
            Some(f) => {
                std::fs::write(f, &text).map_err(|e| Stop::Io(format!("error: cannot write {f}: {e}")))?;
                eprintln!("wrote {f}");
            }
            None => println!("{text}"),
        }
        Ok(())
    });
    match result {
        Ok(()) => OK,
        Err(Stop::Usage(m)) => {
            eprintln!("error: {m}\n\nrun `rcfd --help` for usage");
            USAGE_ERR
        }
        Err(Stop::Io(m)) => {
            eprintln!("{m}");
            FAIL
        }
        Err(Stop::Invalid(m)) => {
            eprintln!("{m}");
            INVALID
        }
    }
}
