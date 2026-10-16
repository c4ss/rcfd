//! ADR 0034 check P7: the Python package tests (`python/tests`) run with the system Python as part of the
//! workspace tests. Without an interpreter the test prints `SKIPPED: ...` (visible with `--nocapture` and in
//! the failure-free summary line of the harness output) and returns; it never passes silently.

use std::path::PathBuf;
use std::process::Command;

fn find_python() -> Option<String> {
    for name in ["python", "python3", "py"] {
        if let Ok(o) = Command::new(name).arg("--version").output() {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            if o.status.success() && text.contains("Python 3") {
                return Some(name.to_string());
            }
        }
    }
    None
}

#[test]
fn python_package_tests() {
    let Some(py) = find_python() else {
        eprintln!(
            "SKIPPED: no python interpreter found (python, python3 or py); the python/ package tests were not run"
        );
        return;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let runner = root.join("python").join("tests").join("run_tests.py");
    assert!(runner.exists(), "missing {}", runner.display());
    let out = Command::new(&py)
        .arg("-I")
        .arg(&runner)
        .env("RCFD_BIN", env!("CARGO_BIN_EXE_rcfd"))
        .current_dir(&root)
        .output()
        .expect("cannot start python");
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "python tests failed:\n{text}");
    assert!(text.contains("Ran 11 tests"), "unexpected test count:\n{text}");
}
