//! ADR 0133 R6 (set-up: amendment 1): the run worker's tick with 4 running runs, each with a `residuals.jsonl` of at
//! least 100 000 000 bytes already read and 10 new lines per tick, has a median of at most 5 ms (release). The tick is
//! `RunPoller::tick` called directly. Negative control: a tick that re-reads whole files exceeds 5 ms.

mod common;

use common::{temp, CAVITY};
use rcfd_project::{Project, RunState, RunStatus};
use rcfd_studio::poll::RunPoller;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

const MIN_BYTES: u64 = 100_000_000;
const LIMIT_MS: f64 = 5.0;

fn line(step: u64) -> String {
    let r = 1.0 / (step as f64 + 1.0);
    format!(
        "{{\"step\":{step},\"time\":{},\"residuals\":{{\"u\":{r:e},\"p\":{},\"continuity\":{}}},\"monitors\":{{\"lid_drag\":{}}}}}\n",
        step as f64 * 1e-3,
        r * 0.5,
        r * 0.25,
        -1e-3 - r * 1e-4
    )
}

struct Fixture {
    root: PathBuf,
    ids: Vec<String>,
    next: Vec<u64>,
}

impl Fixture {
    fn new(name: &str) -> Fixture {
        let root = temp(name);
        let p = Project::create(&root, "r6", "rcfd test").unwrap();
        std::fs::write(root.join("cases/a.toml"), CAVITY).unwrap();
        let mut ids = Vec::new();
        let mut next = Vec::new();
        for _ in 0..4 {
            let rec = p
                .create_run("cases/a.toml", None, "rcfd test", serde_json::json!({}))
                .unwrap();
            p.write_status(&rec.id, &RunStatus::new(RunState::Running)).unwrap();
            let path = root.join("runs").join(&rec.id).join("residuals.jsonl");
            let mut f = std::io::BufWriter::with_capacity(1 << 20, std::fs::File::create(&path).unwrap());
            let (mut bytes, mut step) = (0u64, 0u64);
            while bytes < MIN_BYTES {
                step += 1;
                let l = line(step);
                bytes += l.len() as u64;
                f.write_all(l.as_bytes()).unwrap();
            }
            f.flush().unwrap();
            ids.push(rec.id);
            next.push(step + 1);
        }
        Fixture { root, ids, next }
    }

    fn path(&self, i: usize) -> PathBuf {
        self.root.join("runs").join(&self.ids[i]).join("residuals.jsonl")
    }

    fn append(&mut self, n: u64) {
        for i in 0..self.ids.len() {
            let mut f = std::fs::OpenOptions::new().append(true).open(self.path(i)).unwrap();
            let mut s = String::new();
            for k in self.next[i]..self.next[i] + n {
                s.push_str(&line(k));
            }
            f.write_all(s.as_bytes()).unwrap();
            self.next[i] += n;
        }
    }
}

fn poller(fx: &Fixture, root: &Path) -> RunPoller {
    let mut p = RunPoller::new(root, &root.join("no-runner-here"));
    p.show(&fx.ids);
    p
}

/// Untimed ticks until every shown file is read to its end.
fn catch_up(p: &mut RunPoller, fx: &Fixture) -> usize {
    let mut ticks = 0;
    loop {
        let _ = p.tick();
        ticks += 1;
        let done = (0..fx.ids.len())
            .all(|i| p.residual_offset(&fx.ids[i]) == Some(std::fs::metadata(fx.path(i)).unwrap().len()));
        if done {
            return ticks;
        }
        assert!(ticks < 100, "the poller did not catch up");
    }
}

fn timed(p: &mut RunPoller, fx: &mut Fixture, n: usize) -> Vec<f64> {
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        fx.append(10);
        let t = Instant::now();
        let (_, updates) = p.tick();
        v.push(t.elapsed().as_secs_f64() * 1e3);
        drop(updates);
    }
    v.sort_by(f64::total_cmp);
    v
}

fn median(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

#[test]
fn r6_tick_with_four_large_runs_is_within_5_ms() {
    if cfg!(debug_assertions) {
        eprintln!("SKIP r6 (crates/rcfd-studio/tests/tick_cost_r6.rs): the limit is registered for release builds");
        return;
    }
    let t0 = Instant::now();
    let mut fx = Fixture::new("r6");
    let sizes: Vec<u64> = (0..4).map(|i| std::fs::metadata(fx.path(i)).unwrap().len()).collect();
    let made = t0.elapsed().as_secs_f64();
    let lines0 = fx.next[0] - 1;
    let root = fx.root.clone();
    let mut p = poller(&fx, &root);
    let t1 = Instant::now();
    let ticks = catch_up(&mut p, &fx);
    let first = t1.elapsed().as_secs_f64();
    let v = timed(&mut p, &mut fx, 50);
    let m = median(&v);
    eprintln!(
        "R6: files {sizes:?} bytes ({lines0} lines each) written in {made:.1} s, read in {ticks} untimed ticks ({first:.1} s); 50 ticks: median {m:.3} ms, min {:.3}, max {:.3}",
        v[0],
        v[49]
    );
    assert!(m <= LIMIT_MS, "median {m:.3} ms");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn r6_control_a_tick_that_rereads_whole_files_exceeds_5_ms() {
    if cfg!(debug_assertions) {
        eprintln!("SKIP r6 control (crates/rcfd-studio/tests/tick_cost_r6.rs): release builds only");
        return;
    }
    let mut fx = Fixture::new("r6ctl");
    let root = fx.root.clone();
    let mut p = poller(&fx, &root);
    catch_up(&mut p, &fx);
    p.switches.reread_whole = true;
    let v = timed(&mut p, &mut fx, 5);
    let m = median(&v);
    eprintln!(
        "R6 control: 5 whole-file ticks, median {m:.1} ms, min {:.1}, max {:.1}",
        v[0], v[4]
    );
    assert!(m > LIMIT_MS, "median {m:.3} ms");
    let _ = std::fs::remove_dir_all(&root);
}
