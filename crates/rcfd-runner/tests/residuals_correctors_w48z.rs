//! ADR 0160 (job 48Z) R2: through `rcfd-runner serve`, the `residuals.jsonl` lines of a run with
//! `solver.corrector_control = "adaptive"` carry the step's `correctors` and `corrector_cap`; those of the same run with the
//! default (`fixed`) have neither key and exactly the keys they had before. Registered in ADR 0160 before any run; not changed
//! afterwards. The case text is a `format!` template (ADR 0156 B2a's literal scan skips it). Processes are killed only by pids
//! the test recorded.

use rcfd_project::Project;
use rcfd_runner::client::Client;
use rcfd_runner::records::{RunSettings, RunState};
use rcfd_runner::sys;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const RUNNER: &str = env!("CARGO_BIN_EXE_rcfd-runner");

/// ADR 0145's PISO cavity (20 x 20) for `dt`, to 5 steps.
fn cavity(dt: f64) -> String {
    format!(
        r#"
title = "w48z residuals cavity"
[mesh]
generator = "cavity_2d"
nx = 20
ny = 20
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "piso"
convection = "linear_upwind"
dt = {dt}
end_time = {end}
"#,
        end = 5.0 * dt
    )
}

fn rcfd_bin() -> PathBuf {
    let p = Path::new(RUNNER).with_file_name(format!("rcfd{}", std::env::consts::EXE_SUFFIX));
    assert!(p.exists(), "{} missing (cargo build -p rcfd-cli)", p.display());
    p
}

struct Fixture {
    dir: PathBuf,
    client: Client,
    serve: Option<Child>,
}

impl Fixture {
    fn new(name: &str) -> Fixture {
        rcfd_bin();
        let dir = std::env::temp_dir().join(format!("rcfd_runner_w48z_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Project::create(&dir, name, "test").unwrap();
        std::fs::write(dir.join("cases/cavity.toml"), cavity(0.01)).unwrap();
        let config = json!({"max_runs": 1, "cpu_threads": 8, "mem_budget_mb": 100000, "poll_ms": 50});
        std::fs::write(dir.join(".rcfd/runner-config.json"), config.to_string()).unwrap();
        Fixture {
            client: Client::new(&dir, RUNNER),
            dir,
            serve: None,
        }
    }

    fn submit(&self, set: &[&str]) -> String {
        let s = RunSettings {
            mem_mb: Some(10),
            set: set.iter().map(|x| x.to_string()).collect(),
            ..RunSettings::default()
        };
        self.client.submit(&self.dir.join("cases/cavity.toml"), &s).unwrap()
    }

    fn serve(&mut self) {
        let c = Command::new(RUNNER)
            .args(["serve", "--project"])
            .arg(&self.dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        self.serve = Some(c);
    }

    fn finished(&self, id: &str) -> RunState {
        let t0 = Instant::now();
        loop {
            let s = self.client.status(id).unwrap();
            if s.state.is_terminal() {
                return s.state;
            }
            assert!(
                t0.elapsed() < Duration::from_secs(180),
                "{id}: not finished in 180 s: {s:?}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn lines(&self, id: &str) -> Vec<Value> {
        std::fs::read_to_string(self.dir.join("runs").join(id).join("residuals.jsonl"))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn launched_pids(&self) -> Vec<u32> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(self.dir.join("runs")).into_iter().flatten().flatten() {
            let text = std::fs::read_to_string(e.path().join("events.jsonl")).unwrap_or_default();
            for l in text.lines() {
                if let Ok(v) = serde_json::from_str::<Value>(l) {
                    if v["event"] == "launched" {
                        out.extend(v["pid"].as_u64().map(|p| p as u32));
                    }
                }
            }
        }
        out
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::write(self.dir.join(".rcfd/shutdown"), b"");
        if let Some(mut c) = self.serve.take() {
            let t0 = Instant::now();
            while c.try_wait().ok().flatten().is_none() && t0.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(50));
            }
            let _ = c.kill();
            let _ = c.wait();
        }
        for p in self.launched_pids() {
            if sys::pid_alive(p, "rcfd") {
                sys::kill(p);
            }
        }
        std::thread::sleep(Duration::from_millis(300));
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn r2_adaptive_lines_carry_the_corrector_count_and_fixed_lines_are_unchanged() {
    let mut f = Fixture::new("r2");
    let adaptive = f.submit(&["solver.corrector_control=\"adaptive\""]);
    let fixed = f.submit(&[]);
    f.serve();
    let (sa, sf) = (f.finished(&adaptive), f.finished(&fixed));
    let (la, lf) = (f.lines(&adaptive), f.lines(&fixed));
    eprintln!("R2 adaptive: {sa:?}, {} lines, first {:?}", la.len(), la.first());
    eprintln!("R2 fixed: {sf:?}, {} lines, first {:?}", lf.len(), lf.first());
    let mut problems = Vec::new();
    if sa != RunState::Finished || sf != RunState::Finished || la.len() != 5 || lf.len() != 5 {
        problems.push(format!("states {sa:?} {sf:?}, lines {} {}", la.len(), lf.len()));
    }
    for l in &la {
        let ok = l["correctors"].as_u64().is_some_and(|n| n >= 2) && l["corrector_cap"] == 0;
        if !ok {
            problems.push(format!("adaptive line without the count: {l}"));
        }
    }
    for l in &lf {
        let keys: Vec<&str> = l
            .as_object()
            .map(|o| o.keys().map(String::as_str).collect())
            .unwrap_or_default();
        if keys != ["monitors", "residuals", "step", "time"] {
            problems.push(format!("fixed line keys {keys:?}: {l}"));
        }
    }
    assert!(problems.is_empty(), "R2: {problems:#?}");
}
