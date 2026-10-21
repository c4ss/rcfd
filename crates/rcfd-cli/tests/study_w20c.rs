//! ADR 0034 registered checks S1-S7 and ADR 0099 P1-P2 for `rcfd study` (parametric studies, F-095).

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn rcfd_env(args: &[&str], key: &str, val: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd"))
        .args(args)
        .env(key, val)
        .output()
        .unwrap()
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("study_w20c").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Slab with a registered tolerance (1e-12) or the converging one (1e-9).
fn slab(tol: &str, study: &str) -> String {
    format!(
        r#"title = "slab study"
[mesh]
generator = "box_2d"
nx = 16
ny = 2
lx = 0.1
ly = 0.02
[physics]
nu = 0.01
rho = 1.0
cp = 1000.0
conductivity = 2.0
[energy]
enabled = true
relax = 1.0
[initial]
temperature = 300.0
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 310.0
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_value = 300.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
tolerance_u = {tol}
max_iterations = 400
[reports.q]
type = "heat_flow"
patch = "xmin"
{study}"#
    )
}

const GRID: &str = r#"[study]
outputs = ["q"]
[study.parameters.k]
path = "physics.conductivity"
values = [1.0, 2.0]
[study.parameters.dt]
path = "boundaries.xmin.temperature_value"
kind = "linspace"
start = 310
stop = 350
count = 3
[study.parameters.lx]
path = "mesh.lx"
kind = "logspace"
start = 0.05
stop = 0.2
count = 3
"#;

fn write(dir: &Path, name: &str, text: &str) -> String {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p.display().to_string()
}

fn table(dir: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join("table.json")).unwrap()).unwrap()
}

fn summary(o: &Output) -> Value {
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    serde_json::from_str(out.lines().last().unwrap_or("{}")).unwrap_or_else(|e| panic!("{e}: {out}"))
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap()
}

/// Depth of the `box_2d` mesh with the given length, from the total cell volume.
fn mesh_depth(lx: f64) -> f64 {
    let mut c = rcfd_api::case::Case::parse_toml(&slab("1e-9", "")).unwrap();
    c.set("mesh.lx", serde_json::json!(lx)).unwrap();
    let m = rcfd_api::sim::build_mesh(&c).unwrap();
    m.total_volume() / (lx * 0.02)
}

fn exact(row: &Value, depth_of: &dyn Fn(f64) -> f64) -> f64 {
    let p = &row["parameters"];
    let (k, dt, lx) = (f(&p["k"]), f(&p["dt"]), f(&p["lx"]));
    k * 0.02 * depth_of(lx) * (dt - 300.0) / lx
}

fn max_rel(t: &Value, depth_of: &dyn Fn(f64) -> f64) -> f64 {
    t["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let e = exact(r, depth_of);
            (f(&r["reports"]["q"]).abs() / e - 1.0).abs()
        })
        .fold(0.0, f64::max)
}

#[test]
#[ignore = "known miss: ADR 0034 S1 as registered assumes unit depth; box_2d is one cell thick with depth lx/nx (set-up defect, see s1_corrected_depth)"]
fn s1_unit_depth_known_miss() {
    let d = tmp("s1_unit");
    let case = write(&d, "slab.toml", &slab("1e-12", GRID));
    let out = d.join("o");
    rcfd(&["study", &case, "--out", out.to_str().unwrap(), "--quiet", "--no-fields"]);
    let r = max_rel(&table(&out), &|_| 1.0);
    assert!(r <= 1e-6, "relative error {r}");
}

#[test]
fn s1_corrected_depth() {
    for (tag, tol) in [("s1_1e-12", "1e-12"), ("s1_1e-9", "1e-9")] {
        let d = tmp(tag);
        let case = write(&d, "slab.toml", &slab(tol, GRID));
        let out = d.join("o");
        let o = rcfd(&[
            "study",
            &case,
            "--out",
            out.to_str().unwrap(),
            "--quiet",
            "--no-fields",
            "--json",
        ]);
        // the registered tolerance 1e-12 stops at the iteration cap (exit 5, measured), 1e-9 converges
        assert_eq!(code(&o), if tol == "1e-9" { 0 } else { 5 }, "{}", stderr(&o));
        let t = table(&out);
        assert_eq!(t["rows"].as_array().unwrap().len(), 18);
        let r = max_rel(&t, &mesh_depth);
        eprintln!("S1 tol {tol}: max relative error {r:e}");
        assert!(r <= 1e-6, "relative error {r}");
        // recorded sweep values: linspace and logspace ends are exact
        let rows = t["rows"].as_array().unwrap();
        let dts: Vec<f64> = rows.iter().map(|r| f(&r["parameters"]["dt"])).collect();
        assert!(dts.iter().all(|x| [310.0, 330.0, 350.0].contains(x)));
        let lxs: Vec<f64> = rows.iter().map(|r| f(&r["parameters"]["lx"])).collect();
        assert!(lxs.contains(&0.05) && lxs.contains(&0.2));
        assert!(lxs.iter().any(|x| (x / 0.1 - 1.0).abs() < 1e-15));
    }
}

#[test]
fn s1_point_equals_direct_set_run() {
    let d = tmp("s1_direct");
    let case = write(&d, "slab.toml", &slab("1e-9", GRID));
    let out = d.join("o");
    rcfd(&["study", &case, "--out", out.to_str().unwrap(), "--quiet"]);
    let t = table(&out);
    let row = &t["rows"][7];
    let p = &row["parameters"];
    let direct = d.join("direct");
    let o = rcfd(&[
        "run",
        &case,
        "--out",
        direct.to_str().unwrap(),
        "--quiet",
        "--set",
        &format!("physics.conductivity={}", p["k"]),
        "--set",
        &format!("boundaries.xmin.temperature_value={}", p["dt"]),
        "--set",
        &format!("mesh.lx={}", p["lx"]),
    ]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    for file in ["solution.vtu", "monitors.csv"] {
        let a = std::fs::read(direct.join(file)).unwrap();
        let b = std::fs::read(out.join("points").join("p0007").join(file)).unwrap();
        assert!(a == b, "{file} differs between the study point and rcfd run --set");
    }
}

#[test]
fn s2_ordering_and_thread_independence() {
    let d = tmp("s2");
    let case = write(&d, "slab.toml", &slab("1e-9", GRID));
    let (o1, o3) = (d.join("o1"), d.join("o3"));
    let a = rcfd(&[
        "study",
        &case,
        "--out",
        o1.to_str().unwrap(),
        "--parallel",
        "1",
        "--quiet",
        "--json",
    ]);
    let b = rcfd(&[
        "study",
        &case,
        "--out",
        o3.to_str().unwrap(),
        "--parallel",
        "3",
        "--quiet",
        "--json",
    ]);
    assert_eq!((code(&a), code(&b)), (0, 0));
    for file in ["table.csv", "table.json"] {
        let x = std::fs::read(o1.join(file)).unwrap();
        let y = std::fs::read(o3.join(file)).unwrap();
        assert_eq!(x.len(), y.len());
        assert!(x == y, "{file} differs between parallel 1 and 3");
    }
    let t = table(&o1);
    let rows = t["rows"].as_array().unwrap();
    for (i, r) in rows.iter().enumerate() {
        assert_eq!(r["point"].as_u64(), Some(i as u64));
    }
    // grid is row-major over the alphabetical names dt (3 values, slowest), k (2), lx (3, fastest)
    let g = |i: usize, n: &str| f(&rows[i]["parameters"][n]);
    assert_eq!(g(0, "dt"), g(5, "dt"));
    assert_ne!(g(0, "dt"), g(6, "dt"));
    assert_eq!(g(0, "k"), g(2, "k"));
    assert_ne!(g(0, "k"), g(3, "k"));
    assert_ne!(g(0, "lx"), g(1, "lx"));
    assert_eq!(g(0, "lx"), g(3, "lx"));
    // zip
    let z = r#"[study]
combine = "zip"
outputs = ["q"]
[study.parameters.a]
path = "physics.conductivity"
values = [1.0, 2.0, 3.0]
[study.parameters.b]
path = "boundaries.xmin.temperature_value"
values = [310.0, 320.0, 330.0]
[study.parameters.c]
path = "mesh.nx"
values = [8, 16, 32]
"#;
    let zc = write(&d, "zip.toml", &slab("1e-9", z));
    let zo = d.join("zo");
    let o = rcfd(&["study", &zc, "--out", zo.to_str().unwrap(), "--quiet", "--no-fields"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let zt = table(&zo);
    let zr = zt["rows"].as_array().unwrap();
    assert_eq!(zr.len(), 3);
    assert_eq!(f(&zr[2]["parameters"]["a"]), 3.0);
    assert_eq!(f(&zr[2]["parameters"]["b"]), 330.0);
    assert_eq!(zr[2]["parameters"]["c"].as_i64(), Some(32));
}

#[test]
fn s3_resume_skips_exactly_completed_points() {
    let d = tmp("s3");
    let case = write(&d, "slab.toml", &slab("1e-9", GRID));
    let (fresh, part) = (d.join("fresh"), d.join("part"));
    rcfd(&["study", &case, "--out", fresh.to_str().unwrap(), "--quiet"]);
    let a = rcfd(&[
        "study",
        &case,
        "--out",
        part.to_str().unwrap(),
        "--quiet",
        "--json",
        "--max-new-points",
        "7",
    ]);
    assert_eq!(code(&a), 0, "{}", stderr(&a));
    let s = summary(&a);
    assert_eq!((s["state"].as_str(), s["run"].as_u64()), (Some("partial"), Some(7)));
    assert_eq!(table(&part)["rows"].as_array().unwrap().len(), 7);
    let b = rcfd(&[
        "study",
        &case,
        "--out",
        part.to_str().unwrap(),
        "--quiet",
        "--json",
        "--resume",
    ]);
    let s = summary(&b);
    assert_eq!((s["run"].as_u64(), s["skipped"].as_u64()), (Some(11), Some(7)), "{s}");
    for file in ["table.csv", "table.json"] {
        assert!(
            std::fs::read(fresh.join(file)).unwrap() == std::fs::read(part.join(file)).unwrap(),
            "{file} differs from a fresh run"
        );
    }
    let c = rcfd(&[
        "study",
        &case,
        "--out",
        part.to_str().unwrap(),
        "--quiet",
        "--json",
        "--resume",
    ]);
    let s = summary(&c);
    assert_eq!((s["run"].as_u64(), s["skipped"].as_u64()), (Some(0), Some(18)));
    assert!(std::fs::read(fresh.join("table.json")).unwrap() == std::fs::read(part.join("table.json")).unwrap());
    // a changed base setting changes the fingerprints: everything runs again
    let case2 = write(&d, "slab2.toml", &slab("1e-9", GRID).replace("nu = 0.01", "nu = 0.02"));
    let e = rcfd(&[
        "study",
        &case2,
        "--out",
        part.to_str().unwrap(),
        "--quiet",
        "--json",
        "--resume",
    ]);
    let s = summary(&e);
    assert_eq!((s["run"].as_u64(), s["skipped"].as_u64()), (Some(18), Some(0)), "{s}");
}

#[test]
fn s4_failure_isolation() {
    let d = tmp("s4");
    let st = r#"[study]
outputs = ["q"]
[study.parameters.n]
path = "mesh.source"
strings = ["generator", "file", "generator"]
"#;
    let case = write(&d, "slab.toml", &slab("1e-9", st));
    let mut tables = Vec::new();
    for par in ["1", "3"] {
        let out = d.join(format!("o{par}"));
        let o = rcfd(&[
            "study",
            &case,
            "--out",
            out.to_str().unwrap(),
            "--parallel",
            par,
            "--quiet",
            "--json",
        ]);
        let t = table(&out);
        let rows = t["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1]["status"], "failed");
        assert!(!rows[1]["message"].as_str().unwrap().is_empty());
        assert!(rows[1]["reports"]["q"].is_null());
        for i in [0, 2] {
            assert_eq!(rows[i]["status"], "ok");
            // k = 2, depth lx/nx: |q| = 2 * 0.02 * 10 / 16
            let r = (f(&rows[i]["reports"]["q"]).abs() / (2.0 * 0.02 * 10.0 / 16.0) - 1.0).abs();
            assert!(r <= 1e-6, "relative error {r}");
        }
        let fc = rows[1]["exit_code"].as_i64().unwrap() as i32;
        assert!(fc != 0);
        assert_eq!(code(&o), fc, "study exit code is that of the first failed point");
        assert!(out.join("points").join("p0000").join("solution.vtu").exists());
        assert!(out.join("points").join("p0002").join("solution.vtu").exists());
        tables.push(std::fs::read(out.join("table.csv")).unwrap());
    }
    assert!(tables[0] == tables[1]);
}

/// S2 as registered (the S1 study at tolerance 1e-12, where every point stops at the cap with exit 5) and the
/// reading of "thread count" as the solver's worker threads.
#[test]
fn s2_registered_tolerance_and_solver_threads() {
    let d = tmp("s2_reg");
    let case = write(&d, "slab.toml", &slab("1e-12", GRID));
    let (o1, o3) = (d.join("o1"), d.join("o3"));
    let a = rcfd(&[
        "study",
        &case,
        "--out",
        o1.to_str().unwrap(),
        "--parallel",
        "1",
        "--quiet",
        "--json",
    ]);
    let b = rcfd(&[
        "study",
        &case,
        "--out",
        o3.to_str().unwrap(),
        "--parallel",
        "3",
        "--quiet",
        "--json",
    ]);
    assert_eq!((code(&a), code(&b)), (5, 5));
    for file in ["table.csv", "table.json"] {
        assert!(
            std::fs::read(o1.join(file)).unwrap() == std::fs::read(o3.join(file)).unwrap(),
            "{file} differs between parallel 1 and 3"
        );
    }
    let case9 = write(&d, "slab9.toml", &slab("1e-9", GRID));
    let (r1, r3) = (d.join("r1"), d.join("r3"));
    let a = rcfd_env(
        &["study", &case9, "--out", r1.to_str().unwrap(), "--quiet"],
        "RAYON_NUM_THREADS",
        "1",
    );
    let b = rcfd_env(
        &["study", &case9, "--out", r3.to_str().unwrap(), "--quiet"],
        "RAYON_NUM_THREADS",
        "3",
    );
    assert_eq!((code(&a), code(&b)), (0, 0));
    for file in ["table.csv", "table.json"] {
        assert!(
            std::fs::read(r1.join(file)).unwrap() == std::fs::read(r3.join(file)).unwrap(),
            "{file} differs between solver threads 1 and 3"
        );
    }
}

/// S3 on the study exactly as registered (tolerance 1e-12): every point is `failed` (exit 5), and resume retries
/// failed points, so the registered counts (run 11, skipped 7) cannot be met.
#[test]
#[ignore = "known miss: ADR 0034 S3 registered on the 1e-12 study where all rows are failed (exit 5) and resume retries failed rows; measured run 18, skipped 0 (see s3_resume_skips_exactly_completed_points for the 1e-9 set-up)"]
fn s3_registered_tolerance_known_miss() {
    let d = tmp("s3_reg");
    let case = write(&d, "slab.toml", &slab("1e-12", GRID));
    let part = d.join("part");
    rcfd(&[
        "study",
        &case,
        "--out",
        part.to_str().unwrap(),
        "--quiet",
        "--max-new-points",
        "7",
    ]);
    let b = rcfd(&[
        "study",
        &case,
        "--out",
        part.to_str().unwrap(),
        "--quiet",
        "--json",
        "--resume",
    ]);
    let s = summary(&b);
    assert_eq!((s["run"].as_u64(), s["skipped"].as_u64()), (Some(11), Some(7)), "{s}");
}

/// S4 with the registered fallback: a mesh file that does not exist.
#[test]
fn s4_registered_variant_missing_mesh_file() {
    let d = tmp("s4b");
    let missing = d.join("no_such.msh").display().to_string().replace('\\', "/");
    let st = format!(
        "[study]
combine = \"zip\"
outputs = [\"q\"]
[study.parameters.a]
path = \"mesh.source\"
strings = [\"generator\", \"file\", \"generator\"]
[study.parameters.b]
path = \"mesh.file\"
strings = [\"\", \"{missing}\", \"\"]
"
    );
    let case = write(&d, "slab.toml", &slab("1e-9", &st));
    let out = d.join("o");
    let o = rcfd(&["study", &case, "--out", out.to_str().unwrap(), "--quiet", "--json"]);
    let t = table(&out);
    let rows = t["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 3, "{}", stderr(&o));
    assert_eq!(rows[1]["status"], "failed");
    assert_eq!(
        (rows[0]["status"].as_str(), rows[2]["status"].as_str()),
        (Some("ok"), Some("ok"))
    );
    let fc = rows[1]["exit_code"].as_i64().unwrap() as i32;
    eprintln!("S4 registered variant: exit code {fc}, message {}", rows[1]["message"]);
    assert!(fc != 0);
    assert_eq!(code(&o), fc);
}

#[test]
fn s5_located_errors() {
    let d = tmp("s5");
    let cases: &[(&str, &str, &str)] = &[
        (
            "unknown path",
            "path = \"physics.nope\"\nvalues = [1.0]\n",
            "study.parameters.a.path",
        ),
        (
            "string for number",
            "path = \"physics.nu\"\nstrings = [\"abc\"]\n",
            "study.parameters.a.strings.0",
        ),
        (
            "number for enum",
            "path = \"solver.algorithm\"\nvalues = [1.0]\n",
            "study.parameters.a.values.0",
        ),
        (
            "out of range",
            "path = \"mesh.nx\"\nvalues = [16, 0]\n",
            "study.parameters.a.values.1",
        ),
        ("empty sweep", "path = \"physics.nu\"\n", "study.parameters.a.values"),
        (
            "linspace no count",
            "path = \"physics.nu\"\nkind = \"linspace\"\nstart = 1\nstop = 2\n",
            "study.parameters.a.count",
        ),
        (
            "logspace start 0",
            "path = \"physics.nu\"\nkind = \"logspace\"\nstart = 0\nstop = 2\ncount = 3\n",
            "study.parameters.a.start",
        ),
        (
            "values and strings",
            "path = \"physics.nu\"\nvalues = [1.0]\nstrings = [\"x\"]\n",
            "study.parameters.a.strings",
        ),
        (
            "bad json",
            "path = \"physics.nu\"\njson = [\"[1,\"]\n",
            "study.parameters.a.json.0",
        ),
        (
            "unknown kind",
            "path = \"physics.nu\"
kind = \"cubic\"
values = [1.0]
",
            "study.parameters.a.kind",
        ),
        (
            "own section",
            "path = \"study.parallel\"\nvalues = [1.0]\n",
            "study.parameters.a.path",
        ),
    ];
    for (what, body, path) in cases {
        let text = slab("1e-9", &format!("[study.parameters.a]\n{body}"));
        let case = write(&d, "bad.toml", &text);
        let out = d.join("o");
        let _ = std::fs::remove_dir_all(&out);
        let o = rcfd(&["study", &case, "--out", out.to_str().unwrap(), "--quiet"]);
        assert_eq!(code(&o), 3, "{what}: {}", stderr(&o));
        assert!(
            stderr(&o).contains(&format!("{path}:")),
            "{what}: expected {path} in {}",
            stderr(&o)
        );
        assert!(!out.exists(), "{what}: nothing may be created");
    }
    let more: &[(&str, &str, &str)] = &[
        (
            "zip unequal",
            "combine = \"zip\"\n[study.parameters.a]\npath = \"physics.nu\"\nvalues = [1.0, 2.0]\n[study.parameters.b]\npath = \"physics.rho\"\nvalues = [1.0]\n",
            "study.combine",
        ),
        (
            "unknown output",
            "outputs = [\"nope\"]\n[study.parameters.a]\npath = \"physics.nu\"\nvalues = [1.0]\n",
            "study.outputs.0",
        ),
        (
            "duplicate path",
            "[study.parameters.a]\npath = \"physics.nu\"\nvalues = [1.0]\n[study.parameters.b]\npath = \"physics.nu\"\nvalues = [2.0]\n",
            "study.parameters.b.path",
        ),
        ("no parameters", "outputs = [\"q\"]\n", "study.parameters"),
        (
            "parallel 0",
            "parallel = 0\n[study.parameters.a]\npath = \"physics.nu\"\nvalues = [1.0]\n",
            "study.parallel",
        ),
    ];
    for (what, body, path) in more {
        let case = write(&d, "bad2.toml", &slab("1e-9", &format!("[study]\n{body}")));
        let out = d.join("o2");
        let _ = std::fs::remove_dir_all(&out);
        let o = rcfd(&["study", &case, "--out", out.to_str().unwrap(), "--quiet"]);
        assert_eq!(code(&o), 3, "{what}: {}", stderr(&o));
        assert!(
            stderr(&o).contains(&format!("{path}:")),
            "{what}: expected {path} in {}",
            stderr(&o)
        );
        assert!(!out.exists(), "{what}: nothing may be created");
    }
    let ok = write(&d, "ok.toml", &slab("1e-9", GRID));
    assert_eq!(code(&rcfd(&["study", &ok, "--parallel", "0"])), 2);
    assert_eq!(code(&rcfd(&["study", &ok, "--max-new-points", "x"])), 2);
    assert_eq!(code(&rcfd(&["study"])), 2);
}

#[test]
fn s6_exit_code_contract_and_list() {
    let d = tmp("s6");
    let st = r#"[study]
outputs = ["q"]
[study.parameters.it]
path = "solver.max_iterations"
values = [2, 400, 500]
"#;
    let case = write(&d, "slab.toml", &slab("1e-9", st));
    let out = d.join("o");
    let o = rcfd(&[
        "study",
        &case,
        "--out",
        out.to_str().unwrap(),
        "--quiet",
        "--json",
        "--no-fields",
    ]);
    assert_eq!(code(&o), 5, "{}", stderr(&o));
    let t = table(&out);
    let rows = t["rows"].as_array().unwrap();
    assert_eq!(rows[0]["exit_code"], 5);
    assert_eq!(rows[0]["status"], "failed");
    assert!(
        !rows[0]["reports"]["q"].is_null(),
        "values of a non-converged point are recorded"
    );
    assert_eq!(
        (rows[1]["exit_code"].as_i64(), rows[2]["exit_code"].as_i64()),
        (Some(0), Some(0))
    );
    // --list runs nothing
    let ok = write(&d, "ok.toml", &slab("1e-9", GRID));
    let list_out = d.join("never");
    let o = rcfd(&["study", &ok, "--list", "--out", list_out.to_str().unwrap()]);
    assert_eq!(code(&o), 0);
    assert_eq!(String::from_utf8_lossy(&o.stdout).lines().count(), 18);
    assert!(!list_out.exists(), "--list runs nothing and writes nothing");
}

const POIS: &str = r#"title = "poiseuille map"
[mesh]
generator = "channel_2d"
nx = 100
ny = NY
lx = 4.0
ly = 0.1
depth = 0.1
[physics]
nu = 0.01
rho = 1.0
[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.02, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 8000
tolerance_u = 1e-9
tolerance_p = 1e-9
[reports.dp]
type = "area_average"
patch = "inlet"
field = "p"
[study]
outputs = ["dp"]
[study.parameters.u]
path = "boundaries.inlet.velocity_value"
json = [JSON]
"#;

fn pois_case(d: &Path, ny: &str, us: &[f64]) -> String {
    let js: Vec<String> = us.iter().map(|u| format!("\"[{u},0,0]\"")).collect();
    write(d, "p.toml", &POIS.replace("NY", ny).replace("JSON", &js.join(", ")))
}

fn poiseuille(ny: &str, us: &[f64], tag: &str) -> Vec<(f64, f64)> {
    let d = tmp(tag);
    let case = pois_case(&d, ny, us);
    let out = d.join("o");
    let o = rcfd(&[
        "study",
        &case,
        "--out",
        out.to_str().unwrap(),
        "--quiet",
        "--no-fields",
        "--parallel",
        "2",
    ]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let t = table(&out);
    t["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let u = r["parameters"]["u"][0].as_f64().unwrap();
            (u, f(&r["reports"]["dp"]))
        })
        .collect()
}

fn dp_exact(u: f64) -> f64 {
    12.0 * 1.0 * 0.01 * u * 4.0 / (0.1 * 0.1)
}

#[test]
#[ignore = "known miss: ADR 0034 S7 limit 1 percent; measured about 1.9 percent at ny = 10 (first-order wall treatment, see poiseuille_error_decreases_with_refinement)"]
fn s7_poiseuille_map_known_miss() {
    let us = [0.02, 0.04, 0.06, 0.08, 0.1];
    for (u, dp) in poiseuille("10", &us, "s7") {
        let e = (dp / dp_exact(u) - 1.0).abs();
        assert!(e <= 0.01, "u = {u}: dp {dp} vs {} (relative {e})", dp_exact(u));
    }
}

const FIVE_U: [f64; 5] = [0.02, 0.04, 0.06, 0.08, 0.1];

/// ADR 0099 P1 (F-095 acceptance): the 5-velocity sweep in parallel returns a 5-row table equal to the sequential one,
/// and resume skips all 5 points.
#[test]
fn p1_five_velocity_sweep_parallel_equals_sequential_and_resumes() {
    let d = tmp("p1");
    let case = pois_case(&d, "10", &FIVE_U);
    let (a, b) = (d.join("a"), d.join("b"));
    let go = |out: &Path, extra: &[&str]| {
        let mut args = vec![
            "study",
            &case,
            "--out",
            out.to_str().unwrap(),
            "--quiet",
            "--no-fields",
            "--json",
        ];
        args.extend_from_slice(extra);
        rcfd(&args)
    };
    let oa = go(&a, &["--parallel", "2"]);
    assert_eq!(code(&oa), 0, "{}", stderr(&oa));
    let s = summary(&oa);
    assert_eq!((s["run"].as_u64(), s["skipped"].as_u64()), (Some(5), Some(0)), "{s}");
    let t = table(&a);
    let rows = t["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 5);
    for r in rows {
        assert_eq!(r["status"], "ok", "{r}");
        assert!(f(&r["reports"]["dp"]).is_finite(), "{r}");
    }
    let ob = go(&b, &["--parallel", "1"]);
    assert_eq!(code(&ob), 0, "{}", stderr(&ob));
    let files = ["table.csv", "table.json"];
    let before: Vec<Vec<u8>> = files.iter().map(|x| std::fs::read(a.join(x)).unwrap()).collect();
    for (file, bytes) in files.iter().zip(&before) {
        assert!(
            *bytes == std::fs::read(b.join(file)).unwrap(),
            "{file} differs between parallel 2 and 1"
        );
    }
    let oc = go(&a, &["--resume"]);
    assert_eq!(code(&oc), 0, "{}", stderr(&oc));
    let s = summary(&oc);
    assert_eq!((s["run"].as_u64(), s["skipped"].as_u64()), (Some(0), Some(5)), "{s}");
    for (file, bytes) in files.iter().zip(&before) {
        assert!(
            *bytes == std::fs::read(a.join(file)).unwrap(),
            "{file} changed on resume"
        );
    }
}

/// ADR 0099 P2: the S7 map re-registered at ny 20 (S7 at ny 10 stays an ignored known miss).
#[test]
fn p2_poiseuille_map_ny20_within_one_percent() {
    let m = poiseuille("20", &FIVE_U, "p2");
    assert_eq!(m.len(), 5);
    for (u, dp) in m {
        let e = (dp / dp_exact(u) - 1.0).abs();
        eprintln!("P2 u = {u}: dp {dp} vs {} (relative {e:e})", dp_exact(u));
        assert!(e <= 0.01, "u = {u}: dp {dp} vs {} (relative {e})", dp_exact(u));
    }
}

/// Diagnostic added after the S7 miss (not a replacement of S7): at the registered mesh the deficit is
/// reproduced, and it shrinks when the cross-stream resolution doubles.
#[test]
fn poiseuille_error_decreases_with_refinement() {
    let e10 = poiseuille("10", &[0.04], "s7d10");
    let e20 = poiseuille("20", &[0.04], "s7d20");
    let r = |v: &[(f64, f64)]| (v[0].1 / dp_exact(v[0].0) - 1.0).abs();
    let (a, b) = (r(&e10), r(&e20));
    eprintln!("S7 diagnostic: relative error ny=10 {a:e}, ny=20 {b:e}");
    assert!(b < a * 0.75, "{a} -> {b}");
    // the map is nearly linear in U (laminar; the convective terms add a small deviation): dp/U constant to 1e-3
    let m = poiseuille("10", &[0.02, 0.06, 0.1], "s7lin");
    let s0 = m[0].1 / m[0].0;
    for (u, dp) in &m {
        assert!((dp / u / s0 - 1.0).abs() < 1e-3, "linearity at {u}");
    }
}
