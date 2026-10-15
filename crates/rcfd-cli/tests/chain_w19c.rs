//! Command-line half of C1 and the chain/restart part of R5 (ADR 0031): `rcfd run chain.toml`
//! writes a transient solution byte-identical to the manual procedure, independent of the thread
//! count.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_chain_cli_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const BASE: &str = r#"
title = "chain cavity"
[mesh]
generator = "cavity_2d"
nx = 16
ny = 16
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 2000
tolerance_u = 1e-8
tolerance_p = 1e-8
"#;

const CHAIN: &str = r#"
[chain]
sequence = ["steady", "transient"]
[chain.stages.steady]
[chain.stages.transient]
set = ["solver.algorithm=\"piso\"", "solver.dt=0.05", "solver.end_time=1.0"]
"#;

fn write(dir: &Path, name: &str, text: &str) -> String {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p.display().to_string()
}

fn ok(o: &Output) {
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn c1_cli_chain_output_is_byte_identical_to_run_then_restart() {
    let d = tmp("c1");
    let chain = write(&d, "chain.toml", &format!("{BASE}{CHAIN}"));
    let steady = write(&d, "steady.toml", BASE);
    let transient = write(
        &d,
        "transient.toml",
        &BASE.replace(
            "algorithm = \"simplec\"",
            "algorithm = \"piso\"\ndt = 0.05\nend_time = 1.0",
        ),
    );
    let (oc, os, ot) = (d.join("chain_out"), d.join("steady_out"), d.join("transient_out"));
    ok(&rcfd(&["run", &chain, "--out", oc.to_str().unwrap(), "--quiet"]));
    ok(&rcfd(&["run", &steady, "--out", os.to_str().unwrap(), "--quiet"]));
    let ck = os.join("restart.rcfd");
    ok(&rcfd(&[
        "restart",
        &transient,
        "--from",
        ck.to_str().unwrap(),
        "--out",
        ot.to_str().unwrap(),
        "--quiet",
    ]));
    let a = std::fs::read(oc.join("transient/solution.vtu")).unwrap();
    let b = std::fs::read(ot.join("solution.vtu")).unwrap();
    assert!(
        a == b,
        "transient solution.vtu differs ({} vs {} bytes)",
        a.len(),
        b.len()
    );
    let a1 = std::fs::read(oc.join("steady/solution.vtu")).unwrap();
    let b1 = std::fs::read(os.join("solution.vtu")).unwrap();
    assert!(a1 == b1, "steady solution.vtu differs");
    println!(
        "C1 (CLI): chain transient/solution.vtu is byte-identical to run + restart ({} bytes)",
        a.len()
    );
}

#[test]
fn r5_restart_and_chain_do_not_depend_on_the_thread_count() {
    let d = tmp("r5");
    // coarse-to-fine restart
    let coarse = write(
        &d,
        "coarse.toml",
        &BASE.replace("nx = 16", "nx = 12").replace("ny = 16", "ny = 12"),
    );
    let fine = write(
        &d,
        "fine.toml",
        &BASE.replace("nx = 16", "nx = 24").replace("ny = 16", "ny = 24"),
    );
    let oc = d.join("coarse_out");
    ok(&rcfd(&["run", &coarse, "--out", oc.to_str().unwrap(), "--quiet"]));
    let ck = oc.join("restart.rcfd");
    let mut files = Vec::new();
    for t in ["1", "3"] {
        let out = d.join(format!("fine_t{t}"));
        ok(&rcfd(&[
            "restart",
            &fine,
            "--from",
            ck.to_str().unwrap(),
            "--method",
            "least_squares",
            "--threads",
            t,
            "--out",
            out.to_str().unwrap(),
            "--quiet",
        ]));
        files.push(std::fs::read(out.join("solution.vtu")).unwrap());
    }
    assert!(
        files[0] == files[1],
        "restart onto a finer mesh depends on the thread count"
    );
    // chain
    let chain = write(&d, "chain.toml", &format!("{BASE}{CHAIN}"));
    let mut chain_files = Vec::new();
    for t in ["1", "3"] {
        let out = d.join(format!("chain_t{t}"));
        ok(&rcfd(&[
            "run",
            &chain,
            "--threads",
            t,
            "--out",
            out.to_str().unwrap(),
            "--quiet",
        ]));
        chain_files.push(std::fs::read(out.join("transient/solution.vtu")).unwrap());
    }
    assert!(chain_files[0] == chain_files[1], "chain depends on the thread count");
    println!(
        "R5: threads 1 vs 3 byte-identical for restart ({} bytes) and chain ({} bytes)",
        files[0].len(),
        chain_files[0].len()
    );
}
