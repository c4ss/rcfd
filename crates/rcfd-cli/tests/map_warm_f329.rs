//! ADR 0235 B (F-329): `rcfd map --warm-start flow|extended`, the opt-in extended warm start of ADR 0209 D4 on the command
//! line. Checks B1 to B4 as registered; the case is HE of ADR 0209 (steady SIMPLEC with energy).

use rcfd_api::case::Case;
use rcfd_api::design::case_eval::{CaseMapEvaluator, ExtendedMapEvaluator, RunLimits};
use rcfd_api::design::opmap::{self, MapOptions, OnFailure, PointEvaluator};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// ADR 0209's tolerance between a warm and a cold run of the same point.
const REL_WARM: f64 = 1e-4;

const AXIS: &str = "physics.nu=0.05,0.055,0.06";

/// Case HE of ADR 0209, unchanged.
const HE: &str = r#"[mesh]
generator = "channel_2d"
nx = 40
ny = 10
lx = 4.0
ly = 1.0
depth = 0.1
[physics]
nu = 0.05
rho = 1.0
cp = 1.0
conductivity = 0.05
[energy]
enabled = true
buoyancy = false
[initial]
temperature = 0.0
[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.5, 0.0, 0.0]
temperature = "fixed"
temperature_value = 0.0
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
temperature = "zero_gradient"
[boundaries.walls]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 4000
tolerance_u = 1e-6
tolerance_p = 1e-6
[reports.dp]
type = "area_average"
patch = "inlet"
field = "p"
[reports.t_out]
type = "area_average"
patch = "outlet"
field = "T"
[reports.q_walls]
type = "heat_flow"
patch = "walls"
"#;

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("map_warm_f329")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The two tables of a map folder.
fn tables(dir: &Path) -> (String, String) {
    let r = |f: &str| std::fs::read_to_string(dir.join(f)).unwrap_or_else(|e| panic!("{}: {e}", dir.join(f).display()));
    (r("map.csv"), r("map.json"))
}

fn points(json: &str) -> Vec<Value> {
    let j: Value = serde_json::from_str(json).unwrap();
    j["points"].as_array().unwrap().clone()
}

/// `rcfd map` of HE with `extra` options, written to `dir/out`; returns the exit code and the tables.
fn cli_map(dir: &Path, out: &str, extra: &[&str]) -> (i32, (String, String)) {
    let case = dir.join("he.toml");
    std::fs::write(&case, HE).unwrap();
    let out = dir.join(out);
    let mut args = vec![
        "map",
        case.to_str().unwrap(),
        "--axis",
        AXIS,
        "--out",
        out.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    args.push("--quiet");
    let o = rcfd(&args);
    let code = o.status.code().unwrap_or(-1);
    assert_eq!(code, 0, "{args:?}: {}", String::from_utf8_lossy(&o.stderr));
    (code, tables(&out))
}

/// The same map in this process through `opmap::run_and_write`.
fn lib_map<E: PointEvaluator>(dir: &Path, ev: &E) -> (String, String) {
    let pts = opmap::grid_points(&[opmap::parse_axis(AXIS).unwrap()], false).unwrap();
    let opts = MapOptions {
        on_failure: OnFailure::Continue,
        retry_cold: true,
        cold: false,
        out_dir: Some(dir.to_path_buf()),
    };
    opmap::run_and_write(&pts, ev, &opts).unwrap();
    tables(dir)
}

fn he() -> Case {
    Case::parse_toml(HE).unwrap_or_else(|e| panic!("{e:?}"))
}

#[test]
fn b1_b2_default_rows_unchanged_and_extended_rows_warm() {
    let d = tmp("b1b2");
    // B1: the default map is the CaseMapEvaluator map, byte for byte; `--warm-start flow` is the default
    let (_, default) = cli_map(&d, "default", &[]);
    let (_, flow) = cli_map(&d, "flow", &["--warm-start", "flow"]);
    let lib_flow = lib_map(
        &d.join("lib_flow"),
        &CaseMapEvaluator {
            base: he(),
            limits: RunLimits::default(),
        },
    );
    eprintln!("B1: default map {} + {} bytes", default.0.len(), default.1.len());
    assert!(
        default == lib_flow,
        "B1: the default map differs from CaseMapEvaluator's"
    );
    assert!(default == flow, "B1: --warm-start flow differs from the default");
    // B2n: the default map does not warm-start HE (it has an energy equation)
    let dp = points(&default.1);
    let restarts: Vec<_> = dp.iter().map(|p| p["restart"].as_str().unwrap().to_string()).collect();
    eprintln!("B2n: default restarts {restarts:?}");
    assert_eq!(dp.len(), 3);
    assert!(
        dp[1]["restart"] != "warm" && dp[2]["restart"] != "warm",
        "B2n: {restarts:?}"
    );
    // B2: the extended map
    let (_, ext) = cli_map(&d, "extended", &["--warm-start", "extended"]);
    let (_, cold) = cli_map(&d, "cold", &["--cold"]);
    let ep = points(&ext.1);
    let cp = points(&cold.1);
    assert_eq!(ep.len(), 3, "B2: {}", ext.1);
    assert!(ep.iter().all(|p| p["status"] == "converged"), "B2: {}", ext.1);
    assert_eq!(ep[1]["restart"], "warm", "B2: {}", ext.1);
    assert_eq!(ep[2]["restart"], "warm", "B2: {}", ext.1);
    assert_eq!(ep[0], dp[0], "B2: point 0 differs from the default map's");
    let mut worst: f64 = 0.0;
    for i in 1..3 {
        let (w, c) = (
            ep[i]["metrics"].as_object().unwrap(),
            cp[i]["metrics"].as_object().unwrap(),
        );
        assert!(!w.is_empty());
        assert_eq!(
            w.keys().collect::<Vec<_>>(),
            c.keys().collect::<Vec<_>>(),
            "B2: point {i}"
        );
        for (k, a) in w {
            let (a, b) = (a.as_f64().unwrap(), c[k].as_f64().unwrap());
            let rel = if a == b {
                0.0
            } else {
                (a - b).abs() / a.abs().max(b.abs())
            };
            eprintln!(
                "B2: point {i} {k} warm {a:e} cold {b:e} rel {rel:.3e}; iterations warm {} cold {}",
                ep[i]["iterations"], cp[i]["iterations"]
            );
            worst = worst.max(rel);
            assert!(rel <= REL_WARM, "B2: point {i} {k}: warm {a:e} cold {b:e} rel {rel:e}");
        }
    }
    eprintln!("B2: largest relative difference {worst:.3e} (limit {REL_WARM:e})");
    // B2x: the extended map is the ExtendedMapEvaluator map, byte for byte
    let lib_ext = lib_map(
        &d.join("lib_ext"),
        &ExtendedMapEvaluator {
            base: he(),
            limits: RunLimits::default(),
        },
    );
    assert!(
        ext == lib_ext,
        "B2x: the extended map differs from ExtendedMapEvaluator's"
    );
}

#[test]
fn b3_bad_warm_start_values_are_refused() {
    let d = tmp("b3");
    let case = d.join("he.toml");
    std::fs::write(&case, HE).unwrap();
    let c = case.to_str().unwrap();
    for (what, args, needle) in [
        ("unknown value", vec!["--warm-start", "bogus", "--quiet"], Some("bogus")),
        ("no value", vec!["--quiet", "--warm-start"], None),
    ] {
        let out = d.join(what.replace(' ', "_"));
        let mut a = vec!["map", c, "--axis", AXIS, "--out", out.to_str().unwrap()];
        a.extend(args);
        let o = rcfd(&a);
        let err = String::from_utf8_lossy(&o.stderr);
        eprintln!("B3 {what}: exit {:?}, stderr {err:?}", o.status.code());
        assert_eq!(o.status.code(), Some(2), "{what}: {err}");
        assert!(err.contains("--warm-start"), "{what}: {err}");
        if let Some(n) = needle {
            assert!(err.contains(n), "{what}: {err}");
        }
        assert!(!out.exists(), "{what}: the out folder was written");
    }
}

#[test]
fn b4_help_names_the_flag() {
    let o = rcfd(&["--help"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&o.stdout).contains("[--warm-start flow|extended]"));
}
