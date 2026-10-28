//! ADR 0141 G3: where the Studio finds `rcfd-runner` and `rcfd` (`rcfd_studio::bins`), on fixture folders (the search
//! takes its folders as inputs, so neither PATH nor the machine matters), plus the solver check on real and stale
//! programs. Control: the pre-change rule (the runner next to the Studio only) misses programs on PATH.

mod common;

use rcfd_studio::bins::{check_solver, exe_name, Place, Search, BUILD, RUNNER_ENV};
use std::path::{Path, PathBuf};

struct Fixture {
    studio: PathBuf,
    p1: PathBuf,
    p2: PathBuf,
    ws: PathBuf,
    dev: PathBuf,
    cwd: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Fixture {
        let t = common::temp(tag);
        let f = Fixture {
            studio: t.join("studio"),
            p1: t.join("p1"),
            p2: t.join("p 2"),
            ws: t.join("ws"),
            dev: t.join("ws").join("target").join("release"),
            cwd: t.join("ws").join("sub"),
        };
        for d in [&f.studio, &f.p1, &f.p2, &f.dev, &f.cwd] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(f.ws.join("Cargo.toml"), "[workspace]\n").unwrap();
        f
    }

    fn search(&self) -> Search {
        Search {
            exe_dir: Some(self.studio.clone()),
            path: Some(std::env::join_paths([&self.p1, &self.p2]).unwrap()),
            cwd: Some(self.cwd.clone()),
            ..Search::default()
        }
    }
}

fn put(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(exe_name(name));
    std::fs::write(&p, b"fixture").unwrap();
    p
}

fn both(dir: &Path) {
    put(dir, "rcfd-runner");
    put(dir, "rcfd");
}

/// The pre-change rule (`Client::with_default_runner`, the Studio's `default_runner_exe` before ADR 0141): `rcfd-runner`
/// next to the Studio's executable, nothing else.
fn pre_change(s: &Search) -> Option<PathBuf> {
    let r = s.exe_dir.as_ref()?.join(exe_name("rcfd-runner"));
    r.is_file().then_some(r)
}

#[test]
fn g3_nothing_found_lists_every_folder_and_the_build_command() {
    let f = Fixture::new("g3-none");
    let s = f.search();
    let folders: Vec<(PathBuf, Place)> = s.folders();
    eprintln!("G3 folders: {folders:?}");
    let e = s.find().unwrap_err();
    eprintln!("G3 message:\n{e}");
    let want = [
        format!("next to the Studio: {}", f.studio.display()),
        format!("PATH: {}", f.p1.display()),
        format!("PATH: {}", f.p2.display()),
        format!("development checkout: {}", f.dev.display()),
    ];
    let mut at = 0;
    for w in &want {
        let i = e[at..]
            .find(w.as_str())
            .unwrap_or_else(|| panic!("'{w}' missing or out of order in:\n{e}"));
        at += i + w.len();
    }
    assert!(e.contains(BUILD), "no build command");
    assert_eq!(
        BUILD,
        "cargo build --release -p rcfd-studio -p rcfd-runner -p rcfd-cli -p rcfd-mcp"
    );
    assert!(e.contains(RUNNER_ENV));
    assert!(!e.contains("Found alone"), "nothing was found alone");
}

#[test]
fn g3_found_on_path_in_search_order_and_only_together() {
    let f = Fixture::new("g3-found");
    let s = f.search();
    // (b) both in the second PATH folder only
    both(&f.p2);
    let got = s.find().unwrap();
    eprintln!("G3 (b): {got:?}");
    assert_eq!(got.runner, f.p2.join(exe_name("rcfd-runner")));
    assert_eq!(got.solver, f.p2.join(exe_name("rcfd")));
    assert_eq!(got.place, Place::Path);
    // control: the pre-change rule finds nothing there
    assert_eq!(pre_change(&s), None, "the control found a runner next to the Studio");
    // (c) both next to the Studio too: the Studio's folder comes first
    both(&f.studio);
    let got = s.find().unwrap();
    assert_eq!(
        (got.runner.parent(), got.place),
        (Some(f.studio.as_path()), Place::Studio)
    );
    // (d) the runner alone next to the Studio (no rcfd), both in the checkout's target/release
    std::fs::remove_file(f.studio.join(exe_name("rcfd"))).unwrap();
    std::fs::remove_file(f.p2.join(exe_name("rcfd-runner"))).unwrap();
    std::fs::remove_file(f.p2.join(exe_name("rcfd"))).unwrap();
    both(&f.dev);
    let got = s.find().unwrap();
    eprintln!("G3 (d): {got:?}");
    assert_eq!((got.runner.parent(), got.place), (Some(f.dev.as_path()), Place::Dev));
    // (e) without the checkout's pair only the lone runner is left: an error naming it
    std::fs::remove_file(f.dev.join(exe_name("rcfd-runner"))).unwrap();
    let e = s.find().unwrap_err();
    eprintln!("G3 (e):\n{e}");
    let lone = format!(
        "{} (no {} next to it)",
        f.studio.join(exe_name("rcfd-runner")).display(),
        exe_name("rcfd")
    );
    assert!(e.contains(&lone), "the lone runner is not named: {e}");
    assert!(e.contains(&format!(
        "{} (no {} next to it)",
        f.dev.join(exe_name("rcfd")).display(),
        exe_name("rcfd-runner")
    )));
    assert!(e.contains(BUILD));
    // the checkout is found from the Studio's folder too (a Studio in target/debug)
    let from_exe = Search {
        exe_dir: Some(f.ws.join("target").join("debug")),
        ..Search::default()
    };
    assert!(from_exe.folders().contains(&(f.dev.clone(), Place::Dev)));
}

#[test]
fn g3_environment_variables_are_taken_as_given() {
    let f = Fixture::new("g3-env");
    both(&f.p1);
    // (f) RCFD_RUNNER_BIN with rcfd next to it
    let r = put(&f.studio, "rcfd-runner");
    put(&f.studio, "rcfd");
    let got = Search {
        runner_env: Some(r.clone()),
        ..f.search()
    }
    .find()
    .unwrap();
    assert_eq!((got.runner, got.place), (r.clone(), Place::Env));
    // a missing RCFD_RUNNER_BIN is an error, not a silent search elsewhere (p1 holds both)
    let missing = f.studio.join("nope").join(exe_name("rcfd-runner"));
    let e = Search {
        runner_env: Some(missing.clone()),
        ..f.search()
    }
    .find()
    .unwrap_err();
    eprintln!("G3 (f): {e}");
    assert!(e.contains(RUNNER_ENV) && e.contains(&missing.display().to_string()) && e.contains(BUILD));
    // RCFD_BIN names the solver: a folder with the runner alone counts
    let lone = f.p2.join(exe_name("rcfd-runner"));
    std::fs::write(&lone, b"fixture").unwrap();
    let solver = put(&f.cwd, "rcfd");
    let got = Search {
        path: Some(std::env::join_paths([&f.p2]).unwrap()),
        exe_dir: None,
        cwd: None,
        solver_env: Some(solver.clone()),
        ..f.search()
    }
    .find()
    .unwrap();
    assert_eq!((got.runner, got.solver, got.place), (lone, solver, Place::Path));
}

#[test]
fn g3_solver_check_refuses_a_stale_or_broken_rcfd() {
    let (runner, rcfd) = common::binaries();
    check_solver(&rcfd).unwrap_or_else(|e| panic!("the built solver is refused: {e}"));
    let f = Fixture::new("g3-stale");
    // a program whose --help lacks the runner's flags (the runner itself, renamed)
    let stale = f.p1.join(exe_name("rcfd"));
    std::fs::copy(&runner, &stale).unwrap();
    let e = check_solver(&stale).unwrap_err();
    eprintln!("G3 stale: {e}");
    assert!(e.contains(&stale.display().to_string()) && e.contains("--store-dir and --status-monitors"));
    assert!(e.contains(BUILD));
    // a file that is not a program
    let junk = f.p2.join(exe_name("rcfd"));
    std::fs::write(&junk, b"not a program").unwrap();
    let e = check_solver(&junk).unwrap_err();
    eprintln!("G3 junk: {e}");
    assert!(e.contains(&junk.display().to_string()) && e.contains("cannot run it"));
}
