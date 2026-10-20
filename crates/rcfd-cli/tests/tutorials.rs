//! F-096 (ADR 0089): the CLI tutorials in docs/tutorials are executed, so they cannot rot. MCP tutorials run in
//! crates/rcfd-mcp/tests/tutorials.rs (the only place where the rcfd-mcp binary is available).

mod tutorial;

use std::path::Path;
use std::process::Command;
use tutorial::{Block, Tag};

/// Runs every `$ rcfd ...` line of a block with the real binary; the exit code of each must be the block's.
fn exec(b: &Block, dir: &Path) -> Result<String, String> {
    let Tag::Run { exit, .. } = b.tag else { unreachable!() };
    let mut all = String::new();
    for l in b.text.lines().filter(|l| !l.trim().is_empty()) {
        let Some(cmd) = l.strip_prefix("$ ") else {
            return Err(format!("'{l}' is not a command (commands start with '$ ')"));
        };
        let args = tutorial::split_args(cmd);
        if args[0] != "rcfd" {
            return Err(format!("only rcfd commands run here, got '{}'", args[0]));
        }
        let o = Command::new(env!("CARGO_BIN_EXE_rcfd"))
            .args(&args[1..])
            .current_dir(dir)
            .output()
            .map_err(|e| e.to_string())?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        if o.status.code() != Some(exit) {
            return Err(format!(
                "'{cmd}' exited with {:?}, expected {exit}:\n{text}",
                o.status.code()
            ));
        }
        all.push_str(&text);
    }
    Ok(all)
}

fn run_all(slow: bool) {
    let t0 = std::time::Instant::now();
    let mut n = 0;
    for p in tutorial::tutorials() {
        let md = std::fs::read_to_string(&p).unwrap();
        if tutorial::is_mcp(&md) {
            continue;
        }
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        let dir = tutorial::workdir(&name);
        let r = tutorial::run(&md, &dir, slow, &mut exec).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        println!(
            "{name}: {} blocks executed, {} slow skipped, {} not executed, {} outputs checked",
            r.executed, r.skipped_slow, r.not_executed, r.outputs_checked
        );
        assert!(r.executed > 0, "{name}: no executed block (T3)");
        let _ = std::fs::remove_dir_all(&dir);
        n += 1;
    }
    println!("{n} CLI tutorials in {:.1} s", t0.elapsed().as_secs_f64());
    assert!(n >= 4);
}

/// T1, T3, T4, T7: the default (fast) blocks of every CLI tutorial.
#[test]
fn cli_tutorials_run() {
    run_all(false);
}

/// T8: the run-slow blocks too.
#[test]
#[ignore = "slow: runs every run-slow block of the CLI tutorials (ADR 0089 T8)"]
fn cli_tutorials_run_slow() {
    run_all(true);
}

/// T1, T2: the registered tutorial set exists and no block is untagged.
#[test]
fn tutorial_set_and_tags() {
    let names: Vec<String> = tutorial::tutorials()
        .iter()
        .map(|p| p.file_stem().unwrap().to_string_lossy().to_string())
        .collect();
    for want in [
        "quick-start-cli",
        "cavity",
        "mcp-design-loop",
        "gallery",
        "desktop-assistant",
    ] {
        assert!(names.iter().any(|n| n == want), "tutorial {want} missing (T1)");
    }
    for p in tutorial::tutorials() {
        let md = std::fs::read_to_string(&p).unwrap();
        tutorial::parse(&md).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    }
}

/// T5: every example case file is checked by the gallery and every case directory is named in it.
#[test]
fn gallery_covers_every_case() {
    let md = std::fs::read_to_string(tutorial::repo().join("docs/tutorials/gallery.md")).unwrap();
    let checked: Vec<String> = tutorial::parse(&md)
        .unwrap()
        .iter()
        .filter(|b| b.tag == Tag::Run { exit: 0, slow: false })
        .flat_map(|b| b.text.lines().map(String::from).collect::<Vec<_>>())
        .filter_map(|l| l.strip_prefix("$ rcfd check ").map(|s| s.trim().to_string()))
        .collect();
    for d in std::fs::read_dir(tutorial::repo().join("cases")).unwrap() {
        let d = d.unwrap().path();
        let name = d.file_name().unwrap().to_string_lossy().to_string();
        assert!(
            md.contains(&format!("`{name}`")),
            "gallery does not name case directory {name}"
        );
        for f in std::fs::read_dir(&d).unwrap() {
            let f = f.unwrap().path();
            if f.extension().is_some_and(|x| x == "toml") {
                let rel = format!("cases/{name}/{}", f.file_name().unwrap().to_string_lossy());
                assert!(checked.contains(&rel), "gallery does not check {rel}");
            }
        }
    }
}

/// T6: the runner is not vacuous; each defect makes it fail.
#[test]
fn runner_detects_defects() {
    let case = "```toml file=c.toml\n[mesh]\ngenerator = \"cavity_2d\"\nnx = 4\nny = 4\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n```\n";
    let good = format!("{case}```console run\n$ rcfd validate c.toml\n```\n```text output\nok\n```\n");
    let dir = tutorial::workdir("mutation");
    tutorial::run(&good, &dir, false, &mut exec).expect("the good fixture passes");
    let bad = [
        ("untagged block", format!("{case}```\n$ rcfd validate c.toml\n```\n")),
        (
            "wrong exit code",
            format!("{case}```console run exit=3\n$ rcfd validate c.toml\n```\n"),
        ),
        (
            "missing output line",
            format!("{case}```console run\n$ rcfd validate c.toml\n```\n```text output\nno such text\n```\n"),
        ),
        (
            "missing input file",
            "```console run\n$ rcfd validate nofile.toml\n```\n".to_string(),
        ),
    ];
    for (what, md) in bad {
        assert!(
            tutorial::run(&md, &dir, false, &mut exec).is_err(),
            "{what} not detected"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn split_args_quotes() {
    assert_eq!(
        tutorial::split_args(r#"rcfd run a.toml --set 'solver.dt=0.5' --x "a b" ''"#),
        ["rcfd", "run", "a.toml", "--set", "solver.dt=0.5", "--x", "a b", ""]
    );
}
