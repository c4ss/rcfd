//! F-451 (ADR 0783): `rcfd duct-modes` against the library on the X1 field of ADR 0351. A1, A1s, A1x: the axial export
//! of the seven X1 sections equals the library's text byte for byte (file, standard output, another reference
//! position); A2: one section gives the library's section export; E1, E1c: a section outside the duct is refused with
//! the registered text T1; E2: a non-integer number of periods with T2; E2n: one whole period is accepted; U1: usage
//! errors; H1: the help lines. The X1 field and the reference text are built by the statements of
//! `x1_rectangular_chain_export_and_recovery` (crates/rcfd-post/tests/duct_modes_f333.rs), copied.

use rcfd_post::duct_modes::{
    axial_wavenumber, decompose_section, split_axial, DuctShape, HarmonicDft, ModalExport, Mode, ModeBasis, ModeValues,
    SectionFit, C64,
};
use rcfd_post::fwh::SourceMode;
use rcfd_post::surface_series::{FaceSpec, Motion, SeriesSpec};
use std::f64::consts::PI;
use std::path::PathBuf;
use std::process::{Command, Output};

const RHO0: f64 = 1.2;
const C: f64 = 340.0;

/// The X1 field of ADR 0351 as seven series files' contents, with the library's exports of it.
struct X1 {
    /// The seven sections, `x = 0, 0.05, .., 0.30 m`.
    series: Vec<SeriesSpec>,
    /// The frequency as the command line gets it (`{}` of `w / (2 pi)`).
    frequency: String,
    /// The library's axial export with the waves at `x_ref = 0` (the X1 export).
    axial: String,
    /// The same with the axial split at `x_ref = 0.1`.
    axial_at_01: String,
    /// The library's section export of the first station.
    section0: String,
}

fn x1() -> X1 {
    let shape = DuctShape::Rectangular { a: 0.12, b: 0.08 };
    let k = 60.0;
    let w = k * C;
    let basis = ModeBasis::below(shape, 80.0).unwrap();
    assert_eq!(basis.len(), 8, "X1 basis {:?}", basis.modes);
    let truth_of = |md: Mode| -> (C64, C64) {
        let c = |r: f64, a: f64| C64::cis(a).scale(r);
        match (md.m, md.n) {
            (0, 0) => (c(1.0, 0.0), c(0.4, 1.1)),
            (1, 0) => (c(0.8, -0.7), c(0.3, 2.0)),
            (0, 1) => (c(0.6, 2.5), c(0.25, -1.4)),
            (1, 1) => (c(0.5, 0.9), c(0.2, 0.3)),
            (2, 0) => (c(0.35, -2.2), C64::default()),
            (2, 1) => (c(0.3, 1.7), C64::default()),
            _ => (C64::default(), C64::default()),
        }
    };
    let truth: Vec<(C64, C64)> = basis.modes.iter().map(|&md| truth_of(md)).collect();
    let kz: Vec<C64> = basis.k_cut.iter().map(|&kc| axial_wavenumber(k, kc)).collect();
    let (ny, nz) = (24, 16);
    let mut points = Vec::new();
    let mut weights = Vec::new();
    for i in 0..ny {
        for j in 0..nz {
            points.push([0.12 * (i as f64 + 0.5) / ny as f64, 0.08 * (j as f64 + 0.5) / nz as f64]);
            weights.push(0.12 / ny as f64 * 0.08 / nz as f64);
        }
    }
    let xs: Vec<f64> = (0..7).map(|s| 0.05 * s as f64).collect();
    let period = 2.0 * PI / w;
    let mut station_amps: Vec<Vec<C64>> = vec![Vec::new(); basis.len()];
    let mut residual: f64 = 0.0;
    let mut series = Vec::new();
    let mut first_fit: Option<SectionFit> = None;
    for &x in &xs {
        let field: Vec<C64> = points
            .iter()
            .map(|&pt| {
                let mut f = C64::default();
                for q in 0..basis.len() {
                    let (ap, am) = truth[q];
                    let ep = (C64::new(0.0, -1.0) * kz[q]).scale(x).exp();
                    let em = (C64::new(0.0, 1.0) * kz[q]).scale(x).exp();
                    f += (ap * ep + am * em) * basis.eval(q, pt);
                }
                f
            })
            .collect();
        let mut dft = HarmonicDft::new(w, points.len()).unwrap();
        let mut p: Vec<Vec<f64>> = vec![Vec::with_capacity(128); points.len()];
        for n in 0..128 {
            let t = n as f64 * period / 64.0;
            let e = C64::cis(w * t);
            let values: Vec<f64> = field.iter().map(|f| (*f * e).re).collect();
            dft.add(t, &values).unwrap();
            for (col, v) in p.iter_mut().zip(&values) {
                col.push(*v);
            }
        }
        let fit = decompose_section(&basis, &points, &weights, &dft.amplitudes().unwrap()).unwrap();
        residual = residual.max(fit.residual);
        for q in 0..basis.len() {
            station_amps[q].push(fit.amplitudes[q]);
        }
        if first_fit.is_none() {
            first_fit = Some(fit);
        }
        // The saved series of this section: the same samples on the clock t0 + n dt (ADR 0783 D5).
        series.push(SeriesSpec {
            rho0: RHO0,
            c0: C,
            t0: 0.0,
            dt: period / 64.0,
            mode: SourceMode::Impermeable,
            flow: [0.0; 3],
            motion: Motion::Static,
            faces: points
                .iter()
                .zip(&weights)
                .zip(p)
                .map(|((pt, wt), p)| FaceSpec {
                    area: *wt,
                    x: [x, pt[0], pt[1]],
                    n: [1.0, 0.0, 0.0],
                    p,
                })
                .collect(),
        });
    }
    let frequency = w / (2.0 * PI);
    let axial_text = |x_ref: f64| {
        let pairs: Vec<(C64, C64)> = (0..basis.len())
            .map(|q| {
                let f = split_axial(&xs, &station_amps[q], kz[q], x_ref).unwrap();
                (f.plus, f.minus)
            })
            .collect();
        ModalExport::axial(&basis, frequency, C, RHO0, x_ref, &pairs, residual)
            .unwrap()
            .to_json_string()
    };
    let axial = axial_text(0.0);
    // Guard (ADR 0783 D5): the reference is the X1 chain, by ADR 0351's own X1 limit on what it reads back.
    let back = ModalExport::from_json_str(&axial).unwrap();
    let big = truth.iter().flat_map(|t| [t.0.abs(), t.1.abs()]).fold(0.0, f64::max);
    let mut err: f64 = 0.0;
    for (q, rec) in back.modes.iter().enumerate() {
        assert_eq!(rec.mode, basis.modes[q]);
        let ModeValues::Axial { plus, minus, .. } = rec.values else {
            panic!("X1: not an axial record")
        };
        for (t, g) in [(truth[q].0, plus), (truth[q].1, minus)] {
            err = err.max(if t.abs() > 0.0 {
                (g - t).abs() / t.abs()
            } else {
                g.abs() / big
            });
        }
    }
    assert!(err <= 1e-2, "the reference is not the X1 chain: recovery error {err:e}");
    let section0 = ModalExport::section(&basis, frequency, C, RHO0, &first_fit.unwrap())
        .unwrap()
        .to_json_string();
    X1 {
        series,
        frequency: format!("{frequency}"),
        axial,
        axial_at_01: axial_text(0.1),
        section0,
    }
}

fn dir(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("duct_modes_f451_{name}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write_series(dir: &std::path::Path, name: &str, s: &SeriesSpec) -> String {
    let file = dir.join(name);
    std::fs::write(&file, serde_json::to_string(&s.to_json()).unwrap()).unwrap();
    file.to_str().unwrap().to_string()
}

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

/// Positions at which the two byte strings differ, plus the difference of their lengths.
fn differing_bytes(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).filter(|(x, y)| x != y).count() + a.len().abs_diff(b.len())
}

fn stderr_line(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).trim_end().to_string()
}

/// The seven files of the X1 field in `d` and the arguments of the registered run (without `--out`).
fn x1_run(d: &std::path::Path, x: &X1) -> Vec<String> {
    let mut args = vec!["duct-modes".to_string()];
    for (s, spec) in x.series.iter().enumerate() {
        args.push(write_series(d, &format!("s{s}.json"), spec));
    }
    for a in ["--duct", "rect:0.12,0.08", "--frequency", &x.frequency, "--k-max", "80"] {
        args.push(a.to_string());
    }
    args
}

fn run_owned(args: &[String]) -> Output {
    rcfd(&args.iter().map(String::as_str).collect::<Vec<_>>())
}

/// A1, A1s, A1x (ADR 0783): the axial export of the X1 field, byte for byte the library's.
#[test]
fn a1_axial_export_of_x1_equals_the_library_bit_for_bit() {
    let x = x1();
    let d = dir("a1");
    let base = x1_run(&d, &x);
    // A1: the file.
    let out = d.join("modes.json");
    let mut args = base.clone();
    args.extend(["--out".to_string(), out.to_str().unwrap().to_string()]);
    let o = run_owned(&args);
    assert!(o.status.success(), "A1: {}", stderr_line(&o));
    let a1 = differing_bytes(&std::fs::read(&out).unwrap(), x.axial.as_bytes());
    assert_eq!(stderr_line(&o), format!("wrote {}", out.to_str().unwrap()));
    assert!(o.stdout.is_empty(), "A1: standard output with --out");
    // A1s: standard output.
    let o = run_owned(&base);
    assert!(o.status.success(), "A1s: {}", stderr_line(&o));
    let a1s = differing_bytes(&o.stdout, format!("{}\n", x.axial).as_bytes());
    // A1x: another reference position.
    let mut args = base.clone();
    args.extend(["--x-ref".to_string(), "0.1".to_string()]);
    let o = run_owned(&args);
    assert!(o.status.success(), "A1x: {}", stderr_line(&o));
    let a1x = differing_bytes(&o.stdout, format!("{}\n", x.axial_at_01).as_bytes());
    println!(
        "A1: {a1} differing bytes of {}; A1s: {a1s} of {}; A1x: {a1x} of {}",
        x.axial.len(),
        x.axial.len() + 1,
        x.axial_at_01.len() + 1
    );
    assert_ne!(x.axial, x.axial_at_01, "A1x: the two references must differ");
    assert_eq!(a1, 0, "A1");
    assert_eq!(a1s, 0, "A1s");
    assert_eq!(a1x, 0, "A1x");
}

/// A2 (ADR 0783): one section gives the library's section export, byte for byte.
#[test]
fn a2_section_export_of_one_x1_station_equals_the_library() {
    let x = x1();
    let d = dir("a2");
    let file = write_series(&d, "s0.json", &x.series[0]);
    let out = d.join("section.json");
    let o = rcfd(&[
        "duct-modes",
        &file,
        "--duct",
        "rect:0.12,0.08",
        "--frequency",
        &x.frequency,
        "--k-max",
        "80",
        "--out",
        out.to_str().unwrap(),
    ]);
    assert!(o.status.success(), "A2: {}", stderr_line(&o));
    let a2 = differing_bytes(&std::fs::read(&out).unwrap(), x.section0.as_bytes());
    println!("A2: {a2} differing bytes of {}", x.section0.len());
    assert!(x.section0.contains("\"kind\": \"section\""));
    assert_eq!(a2, 0, "A2");
}

/// E1, E1c (ADR 0783): a section outside the duct is refused with the text T1, exit 3, and nothing is written.
#[test]
fn e1_section_outside_the_duct_is_refused_with_its_face() {
    let x = x1();
    let d = dir("e1");
    // E1: face 5 moved to z = 0.09 (b = 0.08).
    let mut moved = x.series[0].clone();
    moved.faces[5].x[2] = 0.09;
    let y = moved.faces[5].x[1];
    let file = write_series(&d, "moved.json", &moved);
    let out = d.join("e1.json");
    let o = rcfd(&[
        "duct-modes",
        &file,
        "--duct",
        "rect:0.12,0.08",
        "--frequency",
        &x.frequency,
        "--k-max",
        "80",
        "--out",
        out.to_str().unwrap(),
    ]);
    let t1 = format!("{file}: invalid value at '$.faces[5].x': section point ({y}, 0.09) lies outside the duct");
    println!("E1: exit {:?}; {}", o.status.code(), stderr_line(&o));
    assert_eq!(o.status.code(), Some(3), "E1");
    assert_eq!(stderr_line(&o), t1, "E1");
    assert!(!out.exists(), "E1: an output file was written");
    assert!(o.stdout.is_empty(), "E1: standard output");
    // E1c: the unchanged section against a circular duct of radius 0.06.
    let file = write_series(&d, "s0.json", &x.series[0]);
    let (k, f) = x.series[0]
        .faces
        .iter()
        .enumerate()
        .find(|(_, f)| f.x[1].hypot(f.x[2]) > 0.06 + 6e-8)
        .unwrap();
    let out = d.join("e1c.json");
    let o = rcfd(&[
        "duct-modes",
        &file,
        "--duct",
        "circ:0.06",
        "--frequency",
        &x.frequency,
        "--out",
        out.to_str().unwrap(),
    ]);
    let t1 = format!(
        "{file}: invalid value at '$.faces[{k}].x': section point ({}, {}) lies outside the duct",
        f.x[1], f.x[2]
    );
    println!("E1c: exit {:?}; {}", o.status.code(), stderr_line(&o));
    assert_eq!(o.status.code(), Some(3), "E1c");
    assert_eq!(stderr_line(&o), t1, "E1c");
    assert!(!out.exists(), "E1c: an output file was written");
}

/// E2, E2n (ADR 0783): 127 and 96 samples of the two-period file are refused with the text T2, exit 3; 64 samples (one
/// period) are accepted.
#[test]
fn e2_non_integer_number_of_periods_is_refused() {
    let x = x1();
    let d = dir("e2");
    let f: f64 = x.frequency.parse().unwrap();
    let cut = |n: usize| {
        let mut s = x.series[0].clone();
        for face in &mut s.faces {
            face.p.truncate(n);
        }
        s
    };
    for n in [127usize, 96] {
        let s = cut(n);
        let file = write_series(&d, &format!("cut{n}.json"), &s);
        let out = d.join(format!("cut{n}_modes.json"));
        let o = rcfd(&[
            "duct-modes",
            &file,
            "--duct",
            "rect:0.12,0.08",
            "--frequency",
            &x.frequency,
            "--k-max",
            "80",
            "--out",
            out.to_str().unwrap(),
        ]);
        let r = n as f64 * s.dt * f;
        let t2 = format!(
            "{file}: invalid value at '$.faces[0].p': {n} samples of dt = {} s cover {r} periods of {f} Hz, not a whole \
             number of periods",
            s.dt
        );
        println!(
            "E2 ({n} samples, r = {r}): exit {:?}; {}",
            o.status.code(),
            stderr_line(&o)
        );
        assert_eq!(o.status.code(), Some(3), "E2 {n}");
        assert_eq!(stderr_line(&o), t2, "E2 {n}");
        assert!(!out.exists(), "E2 {n}: an output file was written");
    }
    // E2n: one whole period.
    let file = write_series(&d, "cut64.json", &cut(64));
    let o = rcfd(&[
        "duct-modes",
        &file,
        "--duct",
        "rect:0.12,0.08",
        "--frequency",
        &x.frequency,
        "--k-max",
        "80",
    ]);
    println!("E2n: exit {:?}", o.status.code());
    assert_eq!(o.status.code(), Some(0), "E2n: {}", stderr_line(&o));
    let back = ModalExport::from_json_str(&String::from_utf8(o.stdout).unwrap()).unwrap();
    assert_eq!(back.modes.len(), 8, "E2n");
}

/// U1, H1 (ADR 0783): usage errors give exit 2; the help holds the two usage lines of D4.
#[test]
fn u1_usage_errors_and_h1_help_lines() {
    let x = x1();
    let d = dir("u1");
    let file = write_series(&d, "s0.json", &x.series[0]);
    let f = x.frequency.as_str();
    let cases: [(&str, Vec<&str>); 6] = [
        (
            "no file",
            vec!["duct-modes", "--duct", "rect:0.12,0.08", "--frequency", f],
        ),
        ("no --duct", vec!["duct-modes", &file, "--frequency", f]),
        ("no --frequency", vec!["duct-modes", &file, "--duct", "rect:0.12,0.08"]),
        (
            "--duct oval:1",
            vec!["duct-modes", &file, "--duct", "oval:1", "--frequency", f],
        ),
        (
            "--x-ref with one file",
            vec![
                "duct-modes",
                &file,
                "--duct",
                "rect:0.12,0.08",
                "--frequency",
                f,
                "--x-ref",
                "0",
            ],
        ),
        (
            "an unknown option",
            vec![
                "duct-modes",
                &file,
                "--duct",
                "rect:0.12,0.08",
                "--frequency",
                f,
                "--modes",
                "8",
            ],
        ),
    ];
    for (name, args) in cases {
        let o = rcfd(&args);
        println!("U1 ({name}): exit {:?}", o.status.code());
        assert_eq!(o.status.code(), Some(2), "U1 {name}: {}", stderr_line(&o));
        assert!(o.stdout.is_empty(), "U1 {name}: standard output");
    }
    let help = String::from_utf8(rcfd(&["--help"]).stdout).unwrap();
    for line in [
        "    duct-modes <series.json> [<series.json> ...] --duct <shape> --frequency <hz> [--k-max <k>] [--x-ref <x>]",
        "        [--origin y,z] [--out <file.json>]",
    ] {
        assert!(help.lines().any(|l| l == line), "H1: the help lacks the line {line:?}");
    }
}
